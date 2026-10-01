//! The resumable game loop.
//!
//! Drive a game with:
//! ```ignore
//! loop {
//!     match g.advance(&mut sink) {
//!         Step::Decision(d) => { let c = pick(&g, &d); g.apply(c, &mut sink).unwrap(); }
//!         Step::Chance { player } => g.resolve_chance(player, card),   // only in chance_mode
//!         Step::GameOver => break,
//!     }
//! }
//! ```
//! The engine never calls out to players; it stops and returns whenever input is needed.
//! Because `GameState` is `Copy`, any point can be snapshotted, forked, or searched.

use crate::cards::{self, id, CardId, ACTION, NUM_CARDS};
use crate::counts::Counts;
use crate::state::{Act, Dest, DurationHeld, Filter, Frame, FrameKind, GameState, Phase, Then, TurnState, Zone};

/// What kind of input is needed. Deliberately generic: a new card should almost always be
/// expressible as a composition of these, not a new variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecisionKind {
    /// Action phase. Choices: `Card(action in hand)` or `Pass` (go to buy phase).
    PlayAction,
    /// Buy phase. Choices: `Card(affordable supply card)` or `Pass` (end turn).
    Buy,
    /// Buy phase, before any card has been bought this turn: a Treasure whose play involves a
    /// choice (Anvil, Investment, Crystal Ball, Tiara, War Chest, Bank). Choice-free Treasures
    /// (Copper, Silver, Gold, ...) are always auto-played first; this decision is only offered
    /// when at least one has-choice Treasure remains in hand. Choices: `Card(treasure in hand)`
    /// or `Pass` (done playing Treasures; move on to `Buy`). 2nd edition: once any card has been
    /// bought this turn, no more Treasures may be played.
    PlayTreasure,
    /// Gain a card from the supply costing up to `max_cost` (or exactly `max_cost` when `exact`,
    /// e.g. Upgrade). `potion`: the reference cost includes a Potion (see `Frame::potion`): up to
    /// `$max_cost` Potion-cost cards are allowed, or with `exact` the gained card's Potion must
    /// match. `optional`: `Choice::Pass` declines (University). Choices: `Card(..)`.
    Gain { max_cost: u8, filter: Filter, dest: Dest, exact: bool, potion: bool, optional: bool },
    /// Pick ONE card from `from` (matching `filter`) to `act` on. The selection repeats;
    /// `min`/`max` are the picks still required/allowed (including this one). `Pass` is legal
    /// when `min == 0` and ends the selection. When `ordered` is false, picks are offered in
    /// non-decreasing card order (each multiset of picks has exactly one path).
    Select { from: Zone, act: Act, filter: Filter, min: u8, max: u8, ordered: bool },
    /// Apply `act` to `Decision::subject`? Choices: `Yes` / `No`.
    YesNo { act: Act },
    /// Choose one option from `cards::modes(Decision.source)`. `picks` = options still to
    /// choose (a multi-pick decision asks this repeatedly); `distinct` when `picks > 1` (every
    /// pick must be a different table entry, offered in increasing index order). The chooser is
    /// `Decision.player`, which may differ from the card's owner (Torturer's victim). Choices:
    /// `Choice::Mode(index)`.
    Mode { picks: u8, distinct: bool },
    /// Name a card that could be on top of `Decision.player`'s deck (or discard, if their deck
    /// is empty) — Wishing Well. Choices: `Choice::Card(..)`.
    Name,
    /// Put the just-picked card (`Decision.subject`) into the deck: `0` = top, `1..=max_known` =
    /// below the Nth known top card, `255` = the very bottom — Secret Passage. Choices:
    /// `Choice::Position(..)`.
    DeckPosition { max_known: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decision {
    /// Who must submit the choice.
    pub player: u8,
    /// Whose zones/effects this decision concerns, if that differs from `player` (Swindler: the
    /// attacker is `player`, but the gain is for the victim, `for_player`). Equal to `player` for
    /// every other decision.
    pub for_player: u8,
    pub kind: DecisionKind,
    /// The card whose effect caused this decision (`None` for PlayAction/Buy).
    pub source: Option<CardId>,
    /// The card a `YesNo` decision is about.
    pub subject: CardId,
    /// For a selection whose picked card is then "upgraded" (Remodel, Mine, ...): the card you
    /// pick lets you gain a card costing up to `cost(pick) + plus`.
    pub upgrade: Option<Upgrade>,
    /// For a selection of a card to play: how many times it will be played (Throne Room: 2).
    pub play_times: u8,
}

/// "Trash a card, gain a card costing up to $N more" — what a selection leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Upgrade {
    pub plus: u8,
    pub filter: Filter,
    pub dest: Dest,
    /// The gain must cost exactly `cost(trashed) + plus` (Upgrade), not merely up to it.
    pub exact: bool,
    /// The gain's destination (and a possible attack) depends on the gained card's type instead
    /// of `dest` (Replace).
    pub dest_by_type: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Choice {
    Pass,
    Card(CardId),
    Yes,
    No,
    /// Index into `cards::modes(Decision.source)` (see `DecisionKind::Mode`).
    Mode(u8),
    /// A deck position (see `DecisionKind::DeckPosition`): `0` = top, `1..=max_known` = below the
    /// Nth known top card, `255` = the bottom.
    Position(u8),
}

/// `Event::Gain::source` for a gain with no source card (a plain buy).
pub const NO_SOURCE: CardId = u8::MAX;

/// Must cover the largest possible choice list: one entry per supply pile (up to `NUM_CARDS`,
/// currently 113) plus a little headroom, since decisions like Buy or War Chest can offer every
/// affordable/eligible pile at once.
pub const CHOICE_CAP: usize = 128;

/// A fixed-capacity list of choices. Slots past `len` are never read, so they're left
/// uninitialized: a buffer is created for every decision, and filling all `CHOICE_CAP` slots each
/// time was measurable in the game loop.
#[derive(Clone, Copy)]
pub struct ChoiceBuf {
    items: [std::mem::MaybeUninit<Choice>; CHOICE_CAP],
    len: u8,
}

impl Default for ChoiceBuf {
    #[inline]
    fn default() -> Self {
        ChoiceBuf { items: [std::mem::MaybeUninit::uninit(); CHOICE_CAP], len: 0 }
    }
}

impl ChoiceBuf {
    #[inline]
    pub fn push(&mut self, c: Choice) {
        self.items[self.len as usize] = std::mem::MaybeUninit::new(c);
        self.len += 1;
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
    #[inline]
    pub fn as_slice(&self) -> &[Choice] {
        // SAFETY: the first `len` slots were written by `push` (`len` only grows there and
        // `clear` resets it to 0), and `MaybeUninit<Choice>` has the same layout as `Choice`.
        unsafe { std::slice::from_raw_parts(self.items.as_ptr() as *const Choice, self.len as usize) }
    }
    pub fn len(&self) -> usize {
        self.len as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn contains(&self, c: Choice) -> bool {
        self.as_slice().contains(&c)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Decision(Decision),
    /// The top card of `player`'s deck must be revealed. Outcomes: `chance_outcomes(player)`,
    /// each card `c` with probability `count(c) / total`. Answer with `resolve_chance`.
    Chance { player: u8 },
    /// A new turn has just begun (only with `GameState::pause_at_turn_start`). Nothing is
    /// pending; call `advance` again to start playing it.
    TurnStart { player: u8 },
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pending {
    None,
    Decision(Decision),
    Chance(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    TurnStart { player: u8, turn: u16 },
    /// The current player's turn entered `phase` (Action, Buy, or CleanupDraw for cleanup).
    PhaseStart { player: u8, phase: Phase },
    Shuffle { player: u8 },
    Draw { player: u8, card: CardId },
    Play { player: u8, card: CardId },
    /// A card already in play resolves again (e.g. the 2nd play from Throne Room); `nth` >= 2.
    PlayAgain { player: u8, card: CardId, source: CardId, nth: u8 },
    Buy { player: u8, card: CardId },
    /// `source` is the card whose effect (or trigger) caused the gain, or `NO_SOURCE` for a plain
    /// buy.
    Gain { player: u8, card: CardId, to: Dest, source: CardId },
    Trash { player: u8, card: CardId },
    Discard { player: u8, card: CardId },
    Topdeck { player: u8, card: CardId },
    Reveal { player: u8, card: CardId },
    SetAside { player: u8, card: CardId },
    Reaction { player: u8, card: CardId },
    /// Masquerade: `player` passes `card` to `to`, once every passing player has chosen.
    Pass { player: u8, card: CardId, to: u8 },
    /// Vanilla bonuses `source` just gave `player` (+Actions, +Buys, +$, +VP tokens; never
    /// +Cards, which show as Draw events). Emitted once per application, only when something
    /// is non-zero, except for a Duration firing at the start of its owner's turn, which is
    /// always emitted (at depth 0, possibly all zero) to mark the firing.
    Bonus { player: u8, source: CardId, actions: u8, buys: u8, coins: u16, vp: u16 },
    GameOver,
}

pub trait EventSink {
    fn event(&mut self, e: Event);
    /// Nesting depth for the events that follow (0 = top level; effects of a card are one level
    /// below the card's play). Sinks that don't care can ignore it.
    #[inline(always)]
    fn depth(&mut self, _depth: u8) {}
    /// Report vanilla bonuses (see `Event::Bonus`); nothing is emitted when all are zero.
    #[inline(always)]
    fn bonus(&mut self, player: u8, source: CardId, actions: u8, buys: u8, coins: u16, vp: u16) {
        if (actions | buys) != 0 || (coins | vp) != 0 {
            self.event(Event::Bonus { player, source, actions, buys, coins, vp });
        }
    }
}

/// Discards events; compiles to nothing.
pub struct NoEvents;
impl EventSink for NoEvents {
    #[inline(always)]
    fn event(&mut self, _: Event) {}
}

impl EventSink for Vec<Event> {
    fn event(&mut self, e: Event) {
        self.push(e);
    }
}

/// Returned by a frame's `run`.
pub(crate) enum Run {
    Continue,
    /// A new turn just started and `pause_at_turn_start` is set.
    Pause,
    Decide(DecisionKind, CardId),
    Chance(u8),
}

/// A draw/reveal from an unknown deck needs a chance outcome (chance mode only).
pub(crate) struct NeedChance(pub u8);

impl GameState {
    // ------------------------------------------------------------------
    // Public driving API
    // ------------------------------------------------------------------

    /// Run until a decision, a chance event (chance mode), or game over.
    pub fn advance<S: EventSink>(&mut self, sink: &mut S) -> Step {
        match self.pending() {
            Pending::Decision(d) => return Step::Decision(d),
            Pending::Chance(p) => return Step::Chance { player: p },
            Pending::None => {}
        }
        loop {
            if self.turn.phase == Phase::GameOver {
                return Step::GameOver;
            }
            let run = match self.stack.top() {
                Some(f) => self.run_frame(f, sink),
                None => self.run_phase(sink),
            };
            match run {
                Run::Continue => continue,
                Run::Pause => return Step::TurnStart { player: self.turn.player },
                Run::Chance(p) => {
                    self.set_pending(Pending::Chance(p));
                    return Step::Chance { player: p };
                }
                Run::Decide(kind, subject) => {
                    let d = Decision {
                        player: self.decider(),
                        for_player: self.decision_for_player(),
                        kind,
                        source: self.decision_source(),
                        subject,
                        upgrade: self.decision_upgrade(),
                        play_times: self.decision_play_times(),
                    };
                    self.set_pending(Pending::Decision(d));
                    if self.auto_single {
                        let mut buf = ChoiceBuf::default();
                        self.legal_choices(&mut buf);
                        debug_assert!(!buf.is_empty(), "decision {:?} with no legal choices", d);
                        if buf.len() == 1 {
                            self.set_pending(Pending::None);
                            self.apply_unchecked(kind, buf.as_slice()[0], sink);
                            continue;
                        }
                    }
                    return Step::Decision(d);
                }
            }
        }
    }

    /// Resolve the setup draws and start turn 1 (action phase, nothing pending) without
    /// advancing any further, e.g. to show or save the position at the start of the game.
    /// Sampling mode only.
    pub fn deal_opening_hands<S: EventSink>(&mut self, sink: &mut S) {
        while self.turn.phase == Phase::Setup {
            let run = match self.stack.top() {
                Some(f) => self.run_frame(f, sink),
                None => self.run_phase(sink),
            };
            assert!(!matches!(run, Run::Chance(_)), "deal_opening_hands requires sampling mode");
        }
    }

    /// Apply a choice to the pending decision. Rejects illegal choices without changing state.
    pub fn apply<S: EventSink>(&mut self, choice: Choice, sink: &mut S) -> Result<(), &'static str> {
        let Pending::Decision(d) = self.pending() else {
            return Err("no decision pending");
        };
        let mut buf = ChoiceBuf::default();
        self.legal_choices(&mut buf);
        if !buf.contains(choice) {
            return Err("illegal choice");
        }
        self.set_pending(Pending::None);
        self.apply_unchecked(d.kind, choice, sink);
        Ok(())
    }

    /// Chance-mode answer: the top card of `player`'s deck is `card` (must be in `deck_unknown`).
    pub fn resolve_chance(&mut self, player: u8, card: CardId) {
        assert_eq!(self.pending(), Pending::Chance(player), "no chance event pending for player");
        let ps = &mut self.players[player as usize];
        assert!(ps.deck_unknown.remove(card), "chance outcome not in deck");
        ps.deck_known.push_top(card);
        self.set_pending(Pending::None);
    }

    /// Distribution for a pending chance event: card counts in the unknown part of the deck.
    pub fn chance_outcomes(&self, player: u8) -> Counts {
        self.players[player as usize].deck_unknown
    }

    /// Legal choices for the pending decision (empty if none pending).
    pub fn legal_choices(&self, out: &mut ChoiceBuf) {
        out.clear();
        let Pending::Decision(d) = self.pending() else { return };
        match self.stack.top() {
            Some(f) => self.frame_choices(f, out),
            None => self.phase_choices(d.kind, out),
        }
    }

    pub fn pending_decision(&self) -> Option<Decision> {
        match self.pending() {
            Pending::Decision(d) => Some(d),
            _ => None,
        }
    }

    // ------------------------------------------------------------------
    // Phase handling (when the effect stack is empty)
    // ------------------------------------------------------------------

    fn run_phase<S: EventSink>(&mut self, sink: &mut S) -> Run {
        sink.depth(0);
        let p = self.turn.player as usize;
        match self.turn.phase {
            Phase::Setup => {
                self.turn = TurnState::start(0, 1);
                Run::Continue
            }
            Phase::Action => {
                if !self.turn.announced {
                    self.turn.announced = true;
                    sink.event(Event::TurnStart { player: self.turn.player, turn: self.turn.number });
                    sink.event(Event::PhaseStart { player: self.turn.player, phase: Phase::Action });
                }
                // Separate from `announced` (see `TurnState::turn_start_resolved`'s doc comment):
                // a text-loaded state is always `announced: true` but still needs this to run.
                if !self.turn.turn_start_resolved {
                    self.turn.turn_start_resolved = true;
                    self.push_turn_start_frames(sink);
                    if !self.stack.is_empty() {
                        return Run::Continue;
                    }
                }
                if self.turn.actions > 0 && self.players[p].hand.any_type(ACTION) {
                    Run::Decide(DecisionKind::PlayAction, 0)
                } else {
                    self.enter_buy(sink);
                    Run::Continue
                }
            }
            Phase::Buy => {
                // States edited or loaded mid-buy-phase, or a card effect (Crystal Ball, Mine)
                // that puts a fresh choice-free Treasure into hand, may hold unplayed treasures;
                // auto-play them every time we get here. One hand scan decides both what to
                // auto-play and whether a has-choice Treasure remains, so this `Run::Decide`'s
                // `DecisionKind` is authoritative; `apply_phase` reuses it (via `apply_unchecked`)
                // instead of re-deriving the same thing from another hand scan.
                let has_choice_treasure = if self.turn.treasures_done { false } else { self.play_choice_free_treasures(sink) };
                if has_choice_treasure {
                    Run::Decide(DecisionKind::PlayTreasure, 0)
                } else if self.turn.buys > 0 && !self.turn.treasury_offered {
                    // `!treasury_offered`: once the Buy phase has started ending (Treasury's
                    // offer already begun, win or lose — see `end_buy_phase`), stay committed to
                    // ending it even if buys remain unspent (a voluntary Pass while buys>0 is
                    // legal and routes here too, via `apply_phase`'s Buy-decision catch-all);
                    // otherwise, re-entering this branch while a pushed offer's YesNo resolves
                    // would incorrectly re-offer a fresh Buy decision.
                    Run::Decide(DecisionKind::Buy, 0)
                } else {
                    self.end_buy_phase(sink);
                    Run::Continue
                }
            }
            Phase::CleanupDraw => {
                self.players[p].turns_taken += 1;
                if self.end_condition_met() {
                    self.turn.phase = Phase::GameOver;
                    sink.event(Event::GameOver);
                } else {
                    // Outpost: an extra turn keeps the same player instead of advancing; the new
                    // turn's own `is_extra_turn` is set so a 2nd Outpost played during it can't
                    // grant a 3rd turn in a row (see `cleanup`, which computed this).
                    let extra = self.turn.outpost_grants_extra;
                    let next = if extra { p as u8 } else { ((p + 1) % self.num_players as usize) as u8 };
                    self.turn = TurnState::start(next, self.turn.number + 1);
                    self.turn.is_extra_turn = extra;
                    if self.pause_at_turn_start {
                        return Run::Pause;
                    }
                }
                Run::Continue
            }
            Phase::GameOver => Run::Continue,
        }
    }

    fn phase_choices(&self, kind: DecisionKind, out: &mut ChoiceBuf) {
        let p = self.turn.player as usize;
        match kind {
            DecisionKind::PlayAction => {
                for (c, _) in self.players[p].hand.iter() {
                    if cards::is(c, ACTION) {
                        out.push(Choice::Card(c));
                    }
                }
                out.push(Choice::Pass);
            }
            DecisionKind::Buy => {
                for c in self.supply_cards() {
                    if self.supply.get(c) > 0
                        && self.cost(c) as u16 <= self.turn.coins
                        && (self.turn.potions > 0 || !cards::potion_cost(c))
                        && self.may_buy(c)
                    {
                        out.push(Choice::Card(c));
                    }
                }
                out.push(Choice::Pass);
            }
            DecisionKind::PlayTreasure => {
                for (c, _) in self.players[p].hand.iter() {
                    if self.is_treasure(c) {
                        out.push(Choice::Card(c));
                    }
                }
                out.push(Choice::Pass);
            }
            _ => unreachable!(),
        }
    }

    fn apply_unchecked<S: EventSink>(&mut self, kind: DecisionKind, choice: Choice, sink: &mut S) {
        match self.stack.top() {
            Some(f) => self.apply_frame(f, choice, sink),
            None => self.apply_phase(kind, choice, sink),
        }
    }

    /// `kind` is the `DecisionKind` already computed for the pending decision (by `advance`, just
    /// before this was called): reused here, rather than re-deriving "Buy phase: Treasure or
    /// Buy?" from a fresh hand scan, since callers always have it on hand already.
    fn apply_phase<S: EventSink>(&mut self, kind: DecisionKind, choice: Choice, sink: &mut S) {
        sink.depth(0);
        let p = self.turn.player;
        match (self.turn.phase, choice) {
            (Phase::Action, Choice::Card(c)) => {
                self.turn.actions -= 1;
                let ps = &mut self.players[p as usize];
                ps.hand.remove(c);
                ps.in_play.add(c, 1);
                // Not marked held-for-cleanup here: see `push_duration_pending`.
                sink.depth(0);
                sink.event(Event::Play { player: p, card: c });
                self.resolve_effects(c, p, 0, sink);
            }
            (Phase::Action, _) => self.enter_buy(sink),
            (Phase::Buy, Choice::Card(c)) if kind == DecisionKind::PlayTreasure && cards::is_choice_free(c) => {
                self.play_free_treasure(c, 1, sink);
            }
            (Phase::Buy, Choice::Card(c)) if kind == DecisionKind::PlayTreasure => {
                let ps = &mut self.players[p as usize];
                ps.hand.remove(c);
                ps.in_play.add(c, 1);
                sink.depth(0);
                sink.event(Event::Play { player: p, card: c });
                self.resolve_effects(c, p, 0, sink);
            }
            (Phase::Buy, Choice::Pass) if kind == DecisionKind::PlayTreasure => {
                // Done choosing: the remaining choice-free Treasures are played, the remaining
                // has-choice ones are not. Durable for the rest of this Buy phase (the next
                // `run_phase` call offers Buy, even if another has-choice Treasure arrives).
                self.play_all_choice_free_treasures(sink);
                self.turn.treasures_done = true;
            }
            (Phase::Buy, Choice::Card(c)) => {
                self.turn.coins -= self.cost(c) as u16;
                if cards::potion_cost(c) {
                    self.turn.potions -= 1;
                }
                self.turn.buys -= 1;
                self.turn.treasures_done = true;
                sink.depth(0);
                sink.event(Event::Buy { player: p, card: c });
                sink.depth(1);
                self.gain(p, c, Dest::Discard, true, NO_SOURCE, sink);
                self.on_buy_effects(p, c, sink);
            }
            // Ending the Buy phase (a plain Pass on the `Buy` decision, with or without buys
            // still remaining — declining to spend them is legal): goes through the same
            // end-of-phase hook as the "buys naturally hit 0" path in `run_phase`'s `Phase::Buy`
            // arm, so Treasury's offer isn't skipped just because the player passed voluntarily.
            (Phase::Buy, _) => self.end_buy_phase(sink),
            _ => unreachable!(),
        }
    }

    /// Enter the buy phase; treasures are auto-played by the first `run_phase` call for it.
    fn enter_buy<S: EventSink>(&mut self, sink: &mut S) {
        self.turn.phase = Phase::Buy;
        sink.event(Event::PhaseStart { player: self.turn.player, phase: Phase::Buy });
    }

    /// Start of (or return to) the Buy phase. Choice-free Treasures (Copper, Silver, Gold, ...;
    /// see `cards::is_choice_free`) are auto-played, all at once, unless the hand also holds a
    /// Treasure with a choice (Investment, Anvil, Tiara, Crystal Ball, War Chest, Bank): then
    /// order matters (Investment counts and may trash Treasures still in hand, Anvil discards
    /// one, Tiara replays one, Bank counts those already played), so nothing is auto-played and
    /// every Treasure in hand is offered through the `PlayTreasure` decision. Returns whether
    /// that decision is needed.
    fn play_choice_free_treasures<S: EventSink>(&mut self, sink: &mut S) -> bool {
        let p = self.turn.player;
        let hand = self.players[p as usize].hand;
        if hand.iter().any(|(c, _)| self.is_treasure(c) && !cards::is_choice_free(c)) {
            return true;
        }
        self.play_all_choice_free_treasures(sink);
        false
    }

    /// Play every choice-free Treasure in the current player's hand.
    fn play_all_choice_free_treasures<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let hand = self.players[p as usize].hand;
        for (c, n) in hand.iter() {
            if self.is_treasure(c) && cards::is_choice_free(c) {
                self.play_free_treasure(c, n, sink);
            }
        }
    }

    /// Play `n` copies of the choice-free Treasure `c` from the current player's hand.
    fn play_free_treasure<S: EventSink>(&mut self, c: CardId, n: u8, sink: &mut S) {
        let p = self.turn.player;
        let ps = &mut self.players[p as usize];
        ps.hand.set(c, ps.hand.get(c) - n);
        ps.in_play.add(c, n);
        self.turn.played.add(c, n);
        for _ in 0..n {
            sink.event(Event::Play { player: p, card: c });
        }
        self.turn.coins += cards::def(c).coins as u16 * n as u16;
        self.turn.buys += cards::def(c).buys * n;
        self.turn.potions += cards::def(c).potions * n;
        // Philosopher's Stone's value is dynamic, so it is shown like Bank's.
        let stone = if c == id::PHILOSOPHERS_STONE { self.philosophers_stone_coins(p) * n as u16 } else { 0 };
        self.turn.coins += stone;
        // A Treasure's own printed value is not repeated; its extras (+Buy, Duration "now"
        // bonus, Merchant's +$1) are, indented under the play.
        let own_coins = if cards::is(c, cards::DURATION) { cards::def(c).coins as u16 * n as u16 } else { 0 } + stone;
        let merchant = if c == id::SILVER && self.turn.silvers_played == 0 { self.turn.merchants as u16 } else { 0 };
        sink.depth(1);
        sink.bonus(p, c, 0, cards::def(c).buys * n, own_coins, 0);
        sink.bonus(p, id::MERCHANT, 0, 0, merchant, 0);
        sink.depth(0);
        if c == id::SILVER {
            if self.turn.silvers_played == 0 {
                self.turn.coins += self.turn.merchants as u16;
            }
            self.turn.silvers_played += n;
        }
        // Astrolabe (choice-free Duration Treasure): "now" is the vanilla +$1 +1 Buy above;
        // each copy independently schedules its own next-turn +$1 +1 Buy and stays in play
        // (looped so each physical copy is marked held; see `push_duration_pending`).
        if cards::is(c, cards::DURATION) {
            for _ in 0..n {
                self.push_duration_pending(p, c, 1, 0);
            }
        }
        // Corsair (owned by anyone else): the first Silver or Gold played each turn is
        // trashed instead of staying in play. Card ids are fixed in ascending order
        // (Copper < Silver < Gold), so this loop's iteration order already gives Silver
        // priority over Gold when both are in hand, matching "the first ... they play".
        if (c == id::SILVER || c == id::GOLD) && !self.turn.corsair_trashed_first && self.in_supply(id::CORSAIR) {
            let n_players = self.num_players;
            let has_other_corsair =
                (0..n_players).any(|v| v != p && self.players[v as usize].in_play.has(id::CORSAIR));
            if has_other_corsair {
                self.turn.corsair_trashed_first = true;
                let ps = &mut self.players[p as usize];
                ps.in_play.remove(c);
                self.trash.add(c, 1);
                sink.event(Event::Trash { player: p, card: c });
            }
        }
    }

    /// $1 per 5 cards in `p`'s deck and discard pile together (Philosopher's Stone).
    pub(crate) fn philosophers_stone_coins(&self, p: u8) -> u16 {
        let ps = &self.players[p as usize];
        ((ps.deck_size() + ps.discard.total()) / 5) as u16
    }

    /// The offers made at the end of the Buy phase, before cleanup discards everything in play:
    /// Treasury's "put this onto your deck" (not if a Victory card was gained this Buy phase), then
    /// Alchemist's (put onto your deck if a Potion is in play), then Herbalist's (put a Treasure
    /// from play onto your deck). Alchemist and Herbalist trigger "when you discard this from
    /// play", and the player may order such triggers; the fixed order here is Treasury, Alchemist,
    /// Herbalist, so Herbalist can't take away the Potion Alchemist needs. All are the generic
    /// `YesNo` / `Select` frames (`Act::Topdeck` from `Zone::InPlay`), one per copy in play.
    /// Cheaply gated: a game without these cards skips this with three bitmask tests.
    fn push_end_of_turn_offers(&mut self) {
        let p = self.turn.player;
        let play = self.players[p as usize].in_play;
        // Pushed last-first (the stack pops the top first).
        if self.in_supply(id::HERBALIST) {
            for _ in 0..play.get(id::HERBALIST) {
                self.stack.push(Frame {
                    zone: Zone::InPlay, act: Act::Topdeck, filter: self.treasure_filter(), max: 1,
                    ..Frame::new(FrameKind::Select, p, id::HERBALIST)
                });
            }
        }
        if self.in_supply(id::ALCHEMIST) && play.has(id::POTION) {
            for _ in 0..play.get(id::ALCHEMIST) {
                self.stack.push(Frame {
                    zone: Zone::InPlay, act: Act::Topdeck, subject: id::ALCHEMIST,
                    ..Frame::new(FrameKind::YesNo, p, id::ALCHEMIST)
                });
            }
        }
        if self.in_supply(id::TREASURY) && !self.turn.gained_victory_in_buy {
            for _ in 0..play.get(id::TREASURY) {
                self.stack.push(Frame {
                    zone: Zone::InPlay, act: Act::Topdeck, subject: id::TREASURY,
                    ..Frame::new(FrameKind::YesNo, p, id::TREASURY)
                });
            }
        }
    }

    /// End the Buy phase: the end-of-turn offers (once, guarded by `treasury_offered`), then cleanup
    /// once nothing more is pending from it. Reached both when buys run out naturally
    /// (`run_phase`'s `Phase::Buy` arm) and when the player passes with buys still available
    /// (`apply_phase`'s Buy-decision catch-all), so the offer fires exactly once at the true end
    /// of the Buy phase regardless of which path got there.
    fn end_buy_phase<S: EventSink>(&mut self, sink: &mut S) {
        if !self.turn.treasury_offered {
            self.turn.treasury_offered = true;
            self.push_end_of_turn_offers();
            if !self.stack.is_empty() {
                return;
            }
        }
        self.cleanup(sink);
    }

    fn cleanup<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player as usize;
        sink.event(Event::PhaseStart { player: p as u8, phase: Phase::CleanupDraw });
        // Smugglers' gain record: snapshot this turn's gains as "gained on their last turn" for
        // the player whose turn just ended (see `gain`; empty when Smugglers isn't in play, so
        // this is a cheap no-op copy for Base-only games).
        self.players[p].last_turn_gains = self.turn.gained_this_turn;
        // Outpost: "take an extra turn after this one (not a 3rd turn in a row)". Checked
        // against the in-play snapshot before Duration cards are split out below (Outpost has no
        // start-of-turn effect of its own, so it's never in `pending_durations`; only presence in
        // `in_play` marks it played this turn).
        // An Outpost played this turn is in `duration_held` (one held over from the previous
        // turn is only in `in_play`). It makes the next hand 3 cards even when it fails to grant
        // the extra turn ("you only draw 3 cards, even if you know you won't get the extra turn").
        // Failing means this is already an Outpost turn: it then stays in play (with any Throne
        // Room / King's Court that played it, recorded by `MultiplierFinalize`) until the next
        // turn's cleanup, and has nothing left to do at the start of this player's next turn.
        let outpost_played = self.turn.duration_held.has(id::OUTPOST);
        let grant_extra = outpost_played && !self.turn.is_extra_turn;
        self.turn.outpost_grants_extra = grant_extra;
        let draw_n = if outpost_played { 3 } else { 5 };
        // Other players' failed Outposts from the previous turn are discarded now.
        for q in 0..self.num_players as usize {
            if q != p && !self.players[q].discard_next_cleanup.is_empty() {
                let qs = &mut self.players[q];
                for (c, n) in qs.discard_next_cleanup.iter() {
                    let n = n.min(qs.in_play.get(c));
                    qs.in_play.set(c, qs.in_play.get(c) - n);
                    qs.discard.add(c, n);
                }
                qs.discard_next_cleanup = DurationHeld::EMPTY;
            }
        }
        if outpost_played && !grant_extra {
            let ps = &mut self.players[p];
            let n = self.turn.duration_held.get(id::OUTPOST).min(ps.in_play.get(id::OUTPOST));
            ps.discard_next_cleanup.add(id::OUTPOST, n);
            let mut kept = 0;
            for i in 0..ps.pending_durations_len as usize {
                if ps.pending_durations[i].card != id::OUTPOST {
                    ps.pending_durations[kept] = ps.pending_durations[i];
                    kept += 1;
                }
            }
            for i in kept..ps.pending_durations_len as usize {
                ps.pending_durations[i] = Default::default();
            }
            ps.pending_durations_len = kept as u8;
        }

        let ps = &mut self.players[p];
        let hand = ps.hand;
        let play = ps.in_play;
        // Duration cards (and any Throne Room/King's Court that multiplied one) held for next
        // turn stay in play; everything else in play, plus the whole hand, discards as usual.
        let mut kept = Counts::EMPTY;
        for (c, n) in self.turn.duration_held.iter() {
            kept.set(c, n.min(play.get(c)));
        }
        let mut discard_part = play;
        for (c, _) in kept.iter() {
            discard_part.set(c, play.get(c) - kept.get(c));
        }
        ps.discard.add_all(&hand);
        ps.discard.add_all(&discard_part);
        ps.hand.clear();
        ps.in_play = kept;
        self.turn.phase = Phase::CleanupDraw;
        self.stack.push(Frame { max: draw_n, ..Frame::new(FrameKind::Draw, p as u8, 0) });
    }

    // ------------------------------------------------------------------
    // Primitives used by card effects
    // ------------------------------------------------------------------

    /// Remove the top card of `p`'s deck, shuffling the discard in if the deck is empty.
    /// `Ok(None)` when deck and discard are both empty.
    pub(crate) fn take_top<S: EventSink>(&mut self, p: u8, sink: &mut S) -> Result<Option<CardId>, NeedChance> {
        let ps = &mut self.players[p as usize];
        if ps.deck_known.is_empty() && ps.deck_unknown.is_empty() {
            if ps.discard.is_empty() {
                return Ok(None);
            }
            let d = ps.discard;
            ps.deck_unknown.add_all(&d);
            ps.discard.clear();
            sink.event(Event::Shuffle { player: p });
        }
        if let Some(c) = ps.deck_known.pop_top() {
            return Ok(Some(c));
        }
        if self.chance_mode {
            return Err(NeedChance(p));
        }
        let total = ps.deck_unknown.total();
        let c = ps.deck_unknown.nth(self.rng.below(total));
        ps.deck_unknown.remove(c);
        Ok(Some(c))
    }

    /// Gain `c` from the supply if available, to `to`. `on_buy` is true only for the literal
    /// purchase in the Buy phase (never for Workshop/Remodel/War Chest/... gains, even though
    /// some of those happen during a buy): it gates Hoard's "if you bought it" and Mint's on-buy
    /// trash (`on_buy_effects`). Runs every "when you gain" trigger (Watchtower, Hoard,
    /// Collection, Tiara). Also maintains two cheap, narrowly-gated per-turn records: `p`'s
    /// gain record for Smugglers (only while Smugglers is in this game's supply) and the
    /// "gained a Victory card this Buy phase" flag for Treasury (only while in the Buy phase).
    /// Returns whether it was gained.
    pub(crate) fn gain<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, on_buy: bool, source: CardId, sink: &mut S) -> bool {
        if !self.supply.remove(c) {
            return false;
        }
        let ps = &mut self.players[p as usize];
        match to {
            Dest::Hand => ps.hand.add(c, 1),
            Dest::DeckTop => ps.deck_known.push_top(c),
            Dest::Discard => ps.discard.add(c, 1),
        }
        sink.event(Event::Gain { player: p, card: c, to, source });
        // Smugglers' gain record: "a card the player to your right gained on their last turn"
        // means gained during *their own* turn specifically, so this excludes a victim's forced
        // gain during someone else's turn (Witch's Curse, Bandit's Gold...), which is `p !=
        // self.turn.player` here. One bitmask test (plus the player check) skips this for the
        // overwhelming majority of games (no Smugglers in the kingdom at all).
        if self.in_supply(id::SMUGGLERS) && p == self.turn.player {
            self.turn.gained_this_turn.add_or_drop(c, 1);
        }
        if self.turn.phase == Phase::Buy && cards::is(c, cards::VICTORY) {
            self.turn.gained_victory_in_buy = true;
        }
        self.run_gain_triggers(p, c, to, on_buy, sink);
        true
    }

    /// The zone a gain destination corresponds to, for triggers that need to find the gained
    /// card again afterward (Watchtower, Tiara). `None` for `Dest::DeckTop`: there is no zone in
    /// this engine's Select vocabulary for "the known top of the deck" (decks are the
    /// known-stack/unknown-multiset split, not a `Counts` zone), so a card gained straight onto
    /// the deck (e.g. Bureaucrat's Silver) is a documented gap: Watchtower/Tiara can't currently
    /// react to it. Rare in practice.
    fn gain_dest_zone(to: Dest) -> Option<Zone> {
        match to {
            Dest::Discard => Some(Zone::Discard),
            Dest::Hand => Some(Zone::Hand),
            Dest::DeckTop => None,
        }
    }

    /// Run every "when you gain" trigger for `p` gaining `c` (already moved to `to`): the
    /// generic table (`cards::GAIN_TRIGGERS`) for the immediate, decision-free bonuses (Hoard,
    /// Collection), then, if the card landed in a zone we can revisit, optional reactions
    /// (Watchtower, Tiara) as chained optional `Select` frames over the single gained card —
    /// each is skipped automatically once an earlier one has already moved it (`avail == 0`), so
    /// "trash OR topdeck" falls out of the generic Select machinery for free. Order: Watchtower's
    /// trash, then its topdeck, then Tiara's topdeck; a bonus gain a trigger causes (Hoard's
    /// Gold) runs its own triggers first, so this gain's own reactions are asked first.
    fn run_gain_triggers<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, on_buy: bool, sink: &mut S) {
        // Cheap fast path for the overwhelmingly common case (no Prosperity/Seaside gain-watcher
        // card in this game at all): one bitwise AND against the supply mask, instead of walking
        // the trigger table and probing hand/in-play for every gain of every game.
        if self.in_supply & (cards::GAIN_TRIGGER_CARDS_MASK | cards::SEASIDE_GAIN_TRIGGER_MASK) == 0 {
            return;
        }
        if self.in_supply & cards::SEASIDE_GAIN_TRIGGER_MASK != 0 {
            self.run_seaside_gain_triggers(p, c, to, sink);
        }
        if self.in_supply & cards::GAIN_TRIGGER_CARDS_MASK == 0 {
            return;
        }
        let pi = p as usize;
        for &(watcher, zone, trigger) in cards::GAIN_TRIGGERS {
            // Every copy present triggers independently (two Hoards on one bought Victory card
            // gain two Golds).
            let count = match zone {
                cards::GainTriggerZone::Hand => self.players[pi].hand.get(watcher),
                cards::GainTriggerZone::InPlay => self.players[pi].in_play.get(watcher),
            };
            if count == 0 {
                continue;
            }
            match trigger {
                cards::GainTrigger::HoardBoughtVictory => {
                    if on_buy && cards::is(c, cards::VICTORY) {
                        for _ in 0..count {
                            self.gain(p, id::GOLD, Dest::Discard, false, watcher, sink);
                        }
                    }
                }
                cards::GainTrigger::CollectionAction => {
                    if cards::is(c, cards::ACTION) {
                        self.players[pi].vp_tokens += count as u16;
                        sink.bonus(p, watcher, 0, 0, 0, count as u16);
                    }
                }
                // Decision-based: handled below (a frame per copy, not an immediate effect).
                cards::GainTrigger::WatchtowerReact | cards::GainTrigger::TiaraTopdeck => {}
            }
        }
        let Some(zone) = Self::gain_dest_zone(to) else { return };
        let react = |source: CardId, act: Act| Frame {
            zone,
            act,
            filter: Filter::Card(c),
            max: 1,
            ..Frame::new(FrameKind::Select, p, source)
        };
        // A 2nd (or later) copy's reaction naturally finds nothing left to act on once an
        // earlier one has already moved the card (`avail == 0`, auto-skipped), so pushing one
        // frame per copy is both correct and cheap (copy counts are always small).
        for _ in 0..self.players[pi].in_play.get(id::TIARA) {
            self.stack.push(react(id::TIARA, Act::Topdeck));
        }
        for _ in 0..self.players[pi].hand.get(id::WATCHTOWER) {
            // Revealed from hand when used: the pick logs a Reaction event (`reveal_source`).
            self.stack.push(Frame { reveal_source: true, ..react(id::WATCHTOWER, Act::Topdeck) });
            self.stack.push(Frame { reveal_source: true, ..react(id::WATCHTOWER, Act::Trash) });
        }
    }

    /// Cross-player "when [someone] gains a card" reactions from Seaside Durations, which don't
    /// fit `cards::GAIN_TRIGGERS`' shape (a watcher reacting to *its own owner's* gain): Monkey
    /// reacts to the player to its owner's right; Blockade reacts to any other player gaining its
    /// held card; Pirate reacts to any player's Treasure gain; Sailor is the one card here that
    /// *does* react to its own owner's gain, but "once this turn" is tracked per Sailor instance
    /// via its own `PendingDuration::used`, not the generic per-gain table. Gated by the caller on
    /// `cards::SEASIDE_GAIN_TRIGGER_MASK`.
    fn run_seaside_gain_triggers<S: EventSink>(&mut self, g: u8, c: CardId, to: Dest, sink: &mut S) {
        let n = self.num_players;
        // Monkey: "until your next turn, when the player to your right gains a card, +1 Card."
        // The owner is the player to whose right `g` sits, i.e. the next seat after `g` in turn
        // order (turns pass to the left, so "your right" is the player who acted just before you).
        if self.in_supply(id::MONKEY) {
            let owner = (g + 1) % n;
            if self.players[owner as usize].in_play.has(id::MONKEY) {
                self.stack.push(Frame { max: 1, ..Frame::new(FrameKind::Draw, owner, id::MONKEY) });
            }
        }
        // Blockade: "while its gained card is set aside, when another player gains a copy on
        // their turn, they gain a Curse." Checked against every player's live Blockade holds.
        // Edge case: if the blockaded card is itself a Curse, the Curse this grants is *also* "a
        // copy of the blockaded card", which by the letter of the rule keeps cursing the same
        // player again — bounded only by the Curse pile (up to 50, at 6 players), each further
        // gain of which can itself push more reaction frames (Watchtower...) before any of them
        // are resolved. Un-guarded, that can recurse deep enough to overflow the fixed-size
        // effect stack from an entirely ordinary action (simply buying a Curse). `blockading_curse`
        // guards against the *recursive* re-trigger specifically (a Blockade-granted Curse never
        // triggers another Blockade-granted Curse), capping this at one extra Curse per gain of
        // the blockaded card: a deliberate, narrow simplification of this degenerate combo, not
        // exercised by any ordinary game.
        if self.in_supply(id::BLOCKADE) && self.turn.player == g && !self.turn.blockading_curse {
            for owner in 0..n {
                if owner == g {
                    continue;
                }
                let ps = &self.players[owner as usize];
                for i in 0..ps.pending_durations_len as usize {
                    let e = ps.pending_durations[i];
                    if e.card == id::BLOCKADE && e.arg == c {
                        self.turn.blockading_curse = true;
                        self.gain(g, id::CURSE, Dest::Discard, false, id::BLOCKADE, sink);
                        self.turn.blockading_curse = false;
                        break;
                    }
                }
            }
        }
        // Pirate: "When any player gains a Treasure, you may play this from your hand."
        if self.in_supply(id::PIRATE) && self.is_treasure(c) {
            for owner in 0..n {
                if self.players[owner as usize].hand.has(id::PIRATE) {
                    self.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Play, subject: id::PIRATE,
                        ..Frame::new(FrameKind::YesNo, owner, id::PIRATE)
                    });
                }
            }
        }
        // Sailor: "once this turn, when you gain a Duration card, you may play it." Only the
        // gainer's own live, not-yet-used Sailor reacts; needs the gain's zone (Hand/Discard) to
        // offer a `Play` there (a `Dest::DeckTop` gain has no zone to play from: same documented
        // gap as Watchtower/Tiara in `gain_dest_zone`).
        if self.in_supply(id::SAILOR) && cards::is(c, cards::DURATION) {
            if let Some(zone) = Self::gain_dest_zone(to) {
                let ps = &mut self.players[g as usize];
                for i in 0..ps.pending_durations_len as usize {
                    let e = &mut ps.pending_durations[i];
                    if e.card == id::SAILOR && !e.used {
                        e.used = true;
                        self.stack.push(Frame { zone, act: Act::Play, subject: c, ..Frame::new(FrameKind::YesNo, g, id::SAILOR) });
                        break;
                    }
                }
            }
        }
    }

    /// Card-specific effects that happen specifically because a card was *bought* (not gained
    /// any other way): the one place for them, parallel to `may_buy`. Runs after the bought
    /// card's own gain (and its triggers) have resolved.
    fn on_buy_effects<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        if c == id::MINT {
            // 2nd edition: trash only non-Duration Treasures (1st edition trashed all Treasures).
            let treasures = self.players[p as usize].in_play;
            for (tc, n) in treasures.iter() {
                if self.is_treasure(tc) && !cards::is(tc, cards::DURATION) {
                    for _ in 0..n {
                        self.players[p as usize].in_play.remove(tc);
                        self.trash.add(tc, 1);
                        sink.event(Event::Trash { player: p, card: tc });
                    }
                }
            }
        }
    }

    /// Moat / Lighthouse check: Moat is auto-revealed from hand; Lighthouse is a lingering
    /// static while in play ("other players' Attacks don't affect you"), so it's just an
    /// `in_play` check, no reaction event (nothing is revealed).
    pub(crate) fn immune<S: EventSink>(&self, victim: u8, sink: &mut S) -> bool {
        if self.players[victim as usize].hand.has(id::MOAT) {
            sink.event(Event::Reaction { player: victim, card: id::MOAT });
            true
        } else {
            self.players[victim as usize].in_play.has(id::LIGHTHOUSE)
        }
    }

    // ------------------------------------------------------------------
    // Pending bookkeeping. Stored in the state so it survives snapshots.
    // ------------------------------------------------------------------

    pub fn pending(&self) -> Pending {
        self.pending
    }
    fn set_pending(&mut self, p: Pending) {
        self.pending = p;
    }

    /// Who must decide right now: the top frame's chooser, else the current player.
    fn decider(&self) -> u8 {
        self.stack.top().map(|f| f.chooser).unwrap_or(self.turn.player)
    }

    /// Whose zones/effects the pending decision concerns: the top frame's player, else the
    /// current player. Equal to `decider()` except when another player is choosing on this
    /// player's behalf (Swindler).
    fn decision_for_player(&self) -> u8 {
        self.stack.top().map(|f| f.player).unwrap_or(self.turn.player)
    }

    fn decision_upgrade(&self) -> Option<Upgrade> {
        match self.stack.top() {
            Some(Frame { kind: FrameKind::Select, then: Then::GainUpTo { plus, filter, dest, exact, dest_by_type }, .. }) => {
                Some(Upgrade { plus, filter, dest, exact, dest_by_type })
            }
            _ => None,
        }
    }

    fn decision_play_times(&self) -> u8 {
        match self.stack.top() {
            Some(Frame { kind: FrameKind::Select, then: Then::PlayPicked { times }, .. }) => times,
            _ => 1,
        }
    }

    fn decision_source(&self) -> Option<CardId> {
        self.stack.top().map(|f| f.source)
    }
}

/// Helper for "pick cards one at a time" decisions. Picks are forced into non-decreasing
/// card-id order (`min`) so each multiset of picks is reachable by exactly one path — this
/// keeps search trees free of duplicate orderings. `need` = picks still required (0 if optional);
/// a card is offered only if picking it still leaves enough cards (id >= it) to finish.
pub(crate) fn canonical_picks(from: &Counts, min: CardId, need: u32, filter: impl Fn(CardId) -> bool, out: &mut ChoiceBuf) {
    let mut at_or_above = [0u32; NUM_CARDS + 1];
    for c in (0..NUM_CARDS).rev() {
        let n = if filter(c as CardId) { from.0[c] as u32 } else { 0 };
        at_or_above[c] = at_or_above[c + 1] + n;
    }
    for c in min as usize..NUM_CARDS {
        if from.0[c] > 0 && filter(c as CardId) && at_or_above[c] >= need {
            out.push(Choice::Card(c as CardId));
        }
    }
}
