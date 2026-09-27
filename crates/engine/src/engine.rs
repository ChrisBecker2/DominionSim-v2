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

use crate::cards::{self, id, CardId, ACTION, NUM_CARDS, TREASURE};
use crate::counts::Counts;
use crate::state::{Act, Dest, Filter, Frame, FrameKind, GameState, Phase, TurnState, Zone};

/// What kind of input is needed. Deliberately generic: a new card should almost always be
/// expressible as a composition of these, not a new variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecisionKind {
    /// Action phase. Choices: `Card(action in hand)` or `Pass` (go to buy phase).
    PlayAction,
    /// Buy phase. Choices: `Card(affordable supply card)` or `Pass` (end turn).
    Buy,
    /// Gain a card from the supply costing up to `max_cost`. Choices: `Card(..)`.
    Gain { max_cost: u8, filter: Filter, dest: Dest },
    /// Pick ONE card from `from` (matching `filter`) to `act` on. The selection repeats;
    /// `min`/`max` are the picks still required/allowed (including this one). `Pass` is legal
    /// when `min == 0` and ends the selection. When `ordered` is false, picks are offered in
    /// non-decreasing card order (each multiset of picks has exactly one path).
    Select { from: Zone, act: Act, filter: Filter, min: u8, max: u8, ordered: bool },
    /// Apply `act` to `Decision::subject`? Choices: `Yes` / `No`.
    YesNo { act: Act },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decision {
    pub player: u8,
    pub kind: DecisionKind,
    /// The card whose effect caused this decision (`None` for PlayAction/Buy).
    pub source: Option<CardId>,
    /// The card a `YesNo` decision is about.
    pub subject: CardId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Choice {
    Pass,
    Card(CardId),
    Yes,
    No,
}

pub const CHOICE_CAP: usize = 48;

#[derive(Clone, Copy)]
pub struct ChoiceBuf {
    items: [Choice; CHOICE_CAP],
    len: u8,
}

impl Default for ChoiceBuf {
    fn default() -> Self {
        ChoiceBuf { items: [Choice::Pass; CHOICE_CAP], len: 0 }
    }
}

impl ChoiceBuf {
    #[inline]
    pub fn push(&mut self, c: Choice) {
        self.items[self.len as usize] = c;
        self.len += 1;
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
    pub fn as_slice(&self) -> &[Choice] {
        &self.items[..self.len as usize]
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
    Buy { player: u8, card: CardId },
    Gain { player: u8, card: CardId, to: Dest },
    Trash { player: u8, card: CardId },
    Discard { player: u8, card: CardId },
    Topdeck { player: u8, card: CardId },
    Reveal { player: u8, card: CardId },
    SetAside { player: u8, card: CardId },
    Reaction { player: u8, card: CardId },
    GameOver,
}

pub trait EventSink {
    fn event(&mut self, e: Event);
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
                Run::Chance(p) => {
                    self.set_pending(Pending::Chance(p));
                    return Step::Chance { player: p };
                }
                Run::Decide(kind, subject) => {
                    let d = Decision { player: self.decider(), kind, source: self.decision_source(), subject };
                    self.set_pending(Pending::Decision(d));
                    if self.auto_single {
                        let mut buf = ChoiceBuf::default();
                        self.legal_choices(&mut buf);
                        debug_assert!(!buf.is_empty(), "decision {:?} with no legal choices", d);
                        if buf.len() == 1 {
                            self.set_pending(Pending::None);
                            self.apply_unchecked(buf.as_slice()[0], sink);
                            continue;
                        }
                    }
                    return Step::Decision(d);
                }
            }
        }
    }

    /// Apply a choice to the pending decision. Rejects illegal choices without changing state.
    pub fn apply<S: EventSink>(&mut self, choice: Choice, sink: &mut S) -> Result<(), &'static str> {
        if !matches!(self.pending(), Pending::Decision(_)) {
            return Err("no decision pending");
        }
        let mut buf = ChoiceBuf::default();
        self.legal_choices(&mut buf);
        if !buf.contains(choice) {
            return Err("illegal choice");
        }
        self.set_pending(Pending::None);
        self.apply_unchecked(choice, sink);
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
        let p = self.turn.player as usize;
        match self.turn.phase {
            Phase::Setup => {
                self.turn = TurnState::start(0, 1);
                sink.event(Event::TurnStart { player: 0, turn: 1 });
                sink.event(Event::PhaseStart { player: 0, phase: Phase::Action });
                Run::Continue
            }
            Phase::Action => {
                if self.turn.actions > 0 && self.players[p].hand.any_type(ACTION) {
                    Run::Decide(DecisionKind::PlayAction, 0)
                } else {
                    self.enter_buy(sink);
                    Run::Continue
                }
            }
            Phase::Buy => {
                // States edited or loaded mid-buy-phase may hold unplayed treasures.
                if self.players[p].hand.any_type(TREASURE) {
                    self.play_treasures(sink);
                }
                if self.turn.buys > 0 {
                    Run::Decide(DecisionKind::Buy, 0)
                } else {
                    self.cleanup(sink);
                    Run::Continue
                }
            }
            Phase::CleanupDraw => {
                self.players[p].turns_taken += 1;
                if self.end_condition_met() {
                    self.turn.phase = Phase::GameOver;
                    sink.event(Event::GameOver);
                } else {
                    let next = ((p + 1) % self.num_players as usize) as u8;
                    self.turn = TurnState::start(next, self.turn.number + 1);
                    sink.event(Event::TurnStart { player: next, turn: self.turn.number });
                    sink.event(Event::PhaseStart { player: next, phase: Phase::Action });
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
                for c in 0..NUM_CARDS as CardId {
                    if self.in_supply(c) && self.supply.get(c) > 0 && cards::cost(c) as u16 <= self.turn.coins {
                        out.push(Choice::Card(c));
                    }
                }
                out.push(Choice::Pass);
            }
            _ => unreachable!(),
        }
    }

    fn apply_unchecked<S: EventSink>(&mut self, choice: Choice, sink: &mut S) {
        match self.stack.top() {
            Some(f) => self.apply_frame(f, choice, sink),
            None => self.apply_phase(choice, sink),
        }
    }

    fn apply_phase<S: EventSink>(&mut self, choice: Choice, sink: &mut S) {
        let p = self.turn.player;
        match (self.turn.phase, choice) {
            (Phase::Action, Choice::Card(c)) => {
                self.turn.actions -= 1;
                let ps = &mut self.players[p as usize];
                ps.hand.remove(c);
                ps.in_play.add(c, 1);
                sink.event(Event::Play { player: p, card: c });
                self.resolve_effects(c, sink);
            }
            (Phase::Action, _) => self.enter_buy(sink),
            (Phase::Buy, Choice::Card(c)) => {
                self.turn.coins -= cards::cost(c) as u16;
                self.turn.buys -= 1;
                sink.event(Event::Buy { player: p, card: c });
                self.gain(p, c, Dest::Discard, sink);
            }
            (Phase::Buy, _) => self.cleanup(sink),
            _ => unreachable!(),
        }
    }

    /// Enter the buy phase: all treasures in hand are played automatically.
    fn enter_buy<S: EventSink>(&mut self, sink: &mut S) {
        self.turn.phase = Phase::Buy;
        sink.event(Event::PhaseStart { player: self.turn.player, phase: Phase::Buy });
        self.play_treasures(sink);
    }

    /// Play every treasure in the current player's hand.
    fn play_treasures<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let hand = self.players[p as usize].hand;
        for (c, n) in hand.iter() {
            if !cards::is(c, TREASURE) {
                continue;
            }
            let ps = &mut self.players[p as usize];
            ps.hand.set(c, 0);
            ps.in_play.add(c, n);
            for _ in 0..n {
                sink.event(Event::Play { player: p, card: c });
            }
            self.turn.coins += cards::def(c).coins as u16 * n as u16;
            if c == id::SILVER {
                if self.turn.silvers_played == 0 {
                    self.turn.coins += self.turn.merchants as u16;
                }
                self.turn.silvers_played += n;
            }
        }
    }

    fn cleanup<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player as usize;
        sink.event(Event::PhaseStart { player: p as u8, phase: Phase::CleanupDraw });
        let ps = &mut self.players[p];
        let hand = ps.hand;
        let play = ps.in_play;
        ps.discard.add_all(&hand);
        ps.discard.add_all(&play);
        ps.hand.clear();
        ps.in_play.clear();
        self.turn.phase = Phase::CleanupDraw;
        self.stack.push(Frame { max: 5, ..Frame::new(FrameKind::Draw, p as u8, 0) });
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

    /// Gain `c` from the supply if available. Returns whether it was gained.
    pub(crate) fn gain<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, sink: &mut S) -> bool {
        if !self.supply.remove(c) {
            return false;
        }
        let ps = &mut self.players[p as usize];
        match to {
            Dest::Hand => ps.hand.add(c, 1),
            Dest::DeckTop => ps.deck_known.push_top(c),
            Dest::Discard => ps.discard.add(c, 1),
        }
        sink.event(Event::Gain { player: p, card: c, to });
        true
    }

    /// Moat check (auto-revealed; revealing is never worse in the base set).
    pub(crate) fn immune<S: EventSink>(&self, victim: u8, sink: &mut S) -> bool {
        if self.players[victim as usize].hand.has(id::MOAT) {
            sink.event(Event::Reaction { player: victim, card: id::MOAT });
            true
        } else {
            false
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

    /// Who must decide right now: the top frame's player, else the current player.
    fn decider(&self) -> u8 {
        self.stack.top().map(|f| f.player).unwrap_or(self.turn.player)
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
