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

use crate::cards::{self, id, CardId, ACTION};
use crate::counts::Counts;
use crate::state::{Act, Dest, DurationHeld, Filter, Frame, FrameKind, GameState, Phase, Then, TurnState, Zone};
pub use crate::effects::OVERPAY_CAP;

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
    /// The start of a turn, with two or more start-of-turn effects pending whose order can matter
    /// (see `cards::start_order_sensitive`): choose which resolves next; it resolves fully, then
    /// the rest are asked again. Choices: `Choice::Card(card)`, one per distinct pending effect: the
    /// Duration (or Clerk) card whose effect it is, or for a Prince the card it plays.
    StartOrder,
    /// Overpay for `card`, just bought (Cornucopia & Guilds): pay any amount beyond its cost, up to
    /// `max_coins` of the coins left; `potion` when a Potion may be paid as well (Stonemason: "Gain 2
    /// Action cards each costing the amount overpaid", Potion included). Choices:
    /// `Choice::Overpay(coins)` / `Choice::OverpayPotion(coins)`, `Overpay(0)` meaning no overpay.
    Overpay { card: CardId, max_coins: u8, potion: bool },
    /// Adventures: choose the Supply pile to move token `token` (`state::TOK_*`) to. Choices: `Choice::Card(pile)` (the Supply id of the pile; the
    /// Knights pile by its marker).
    Token { token: u8 },
    /// Empires, Mountain Pass: bid Debt (`Choice::Bid(n)`, 0 = pass). Only bids above `min - 1` are legal: `min` is one more than the highest bid so far.
    Bid { min: u8 },
    /// Nocturne, the Night phase (after the Buy phase, while the player holds a Night card): play one of them or `Pass` (done; no more can be played
    /// this turn). Choices: `Card(night card in hand)` or `Pass`.
    PlayNight,
    /// Nocturne: pick a Boon to receive from a pool (Druid's three, the Fool's). Choices: `Card(boon)` (a pseudo-card id).
    PickBoon,
    /// Menagerie: an Action card (`Decision::subject`) has just been played in a game with a Way. Choices: `Pass` (do what the card says) or `Card(way)`, a Way of the game
    /// (do what the Way says instead).
    Way,
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
    /// For a call (`YesNo` with `Act::Call`): the card it acts on, 0 for none. Royal Carriage: the Action just played; Duplicate: the
    /// card just gained.
    pub target: CardId,
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

#[derive(Clone, Copy, Debug, Eq, Hash)]
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
    /// Buy phase: remove one Coffers token for +$1 (offered at the `Buy` decision while the player has
    /// a token and a Buy; "at any time during your turn" in the rules, and the Buy decision comes
    /// before every buy and every overpay).
    Coffers,
    /// An overpay of this many coins (see `DecisionKind::Overpay`); 0 = no overpay.
    Overpay(u8),
    /// An overpay of this many coins and a Potion.
    OverpayPotion(u8),
    /// Pay off Debt with $: as much as the $ allow (Empires). Offered at the Buy decision while the player has Debt and $, and in the Action phase
    /// when a Change is in the hand (the only card that cares whether the Debt is paid before it is played).
    PayDebt,
    /// Pay off Debt but leave this many tokens (at least 1) on the player, as much as the $ allow: offered in a game with Change, where one
    /// Debt kept makes Change give +$3 next turn instead of remodeling.
    PayDebtKeep(u8),
    /// A Mountain Pass bid of this many Debt (`DecisionKind::Bid`); 0 = no bid.
    Bid(u8),
    /// Action phase, with no Action left: remove one Villager token for +1 Action (Renaissance).
    Villager,
    /// Buy phase: buy this card by its other way of paying (Animal Fair: trash an Action card from the hand instead of paying $7). Uses a Buy, no $.
    Alt(CardId),
}

/// `==` written out and inlined: the derived one is an out-of-line call, and the bots look for a choice in the list (`contains`) several
/// times in every decision.
impl PartialEq for Choice {
    #[inline(always)]
    fn eq(&self, other: &Choice) -> bool {
        use Choice::*;
        // The same variant first (a tag compare), then the payload: the bots look for a choice in a list of them all the time.
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return false;
        }
        match (self, other) {
            (Card(a), Card(b)) | (Alt(a), Alt(b)) => a == b,
            (Mode(a), Mode(b))
            | (Position(a), Position(b))
            | (Overpay(a), Overpay(b))
            | (OverpayPotion(a), OverpayPotion(b))
            | (PayDebtKeep(a), PayDebtKeep(b))
            | (Bid(a), Bid(b)) => a == b,
            _ => true,
        }
    }
}

impl Choice {
    /// The overpay choice for `coins` and a Potion if `potion`. (Two variants rather than one with a
    /// flag: a `Choice` stays two bytes, which the `ChoiceBuf` copies in every decision rely on.)
    #[inline]
    pub fn overpay(coins: u8, potion: bool) -> Choice {
        if potion { Choice::OverpayPotion(coins) } else { Choice::Overpay(coins) }
    }
    /// `(coins, potion)` of an overpay choice.
    #[inline]
    pub fn as_overpay(self) -> Option<(u8, bool)> {
        match self {
            Choice::Overpay(c) => Some((c, false)),
            Choice::OverpayPotion(c) => Some((c, true)),
            _ => None,
        }
    }
}

/// `Event::Gain::source` for a gain with no source card (a plain buy).
pub const NO_SOURCE: CardId = CardId::MAX;

/// Must cover the largest possible choice list: one entry per supply pile (up to `NUM_CARDS`,
/// currently 158) plus headroom, since decisions like Buy or War Chest can offer every
/// affordable/eligible pile at once, and an overpay offers every amount (up to `OVERPAY_CAP`, with and
/// without a Potion).
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
    /// `source` (a Prince) plays the card it holds at the start of `player`'s turn.
    StartPlay { player: u8, source: CardId, card: CardId },
    Reaction { player: u8, card: CardId },
    /// `source` gives `player` `n` Coffers tokens.
    Coffers { player: u8, source: CardId, n: u16 },
    /// `player` removes `n` Coffers tokens for +$n.
    SpendCoffers { player: u8, n: u16 },
    /// `source` gives `player` `n` Villagers.
    Villagers { player: u8, source: CardId, n: u16 },
    /// `player` removes a Villager for +1 Action.
    SpendVillager { player: u8 },
    /// `player` puts `card` on their Exile mat (from the Supply, the hand, a revealed card or a card in play).
    Exile { player: u8, card: CardId },
    /// `player` plays `card` as the Way `way` (instead of doing what the card says).
    Way { player: u8, way: CardId, card: CardId },
    /// `source` (an Ally, a Liaison) gives `player` `n` Favors (`gained`), or `player` spends `n` Favors on it.
    Favor { player: u8, source: CardId, n: u8, gained: bool },
    /// `player` rotates the pile whose top card was `top` (the card now on top is `now`); `source` is the card or Ally that let them.
    Rotate { player: u8, source: CardId, top: CardId, now: CardId },
    /// `player` skips a turn (Lich): the turn that would have been theirs is not taken.
    Skip { player: u8 },
    /// `player` puts `card` on the bottom of their deck (Sibyl).
    Bottom { player: u8, card: CardId },
    /// `player` takes the Artifact `artifact`, from the seat `from` (255: nobody had it).
    Artifact { player: u8, artifact: CardId, from: u8 },
    /// `player` has `tokens` tokens on Sinister Plot now.
    Plot { player: u8, tokens: u8 },
    /// `player` overpays `coins` (and a Potion) for `card`, which they have just bought.
    Overpay { player: u8, card: CardId, coins: u8, potion: bool },
    /// `player` takes `n` Debt tokens (`source` is the card or Event that caused it).
    TakeDebt { player: u8, source: CardId, n: u16 },
    /// `player` pays off `n` Debt with $.
    PayDebt { player: u8, n: u16 },
    /// `player` receives the Boon `boon`, caused by `source` (the Doom, Fate or other card, or a Boon's own id for the Boons it hands on).
    Boon { player: u8, boon: CardId, source: CardId },
    /// `player` receives the Hex `hex`.
    Hex { player: u8, hex: CardId, source: CardId },
    /// `player` takes (`taken`) or returns the State `state` (Lost in the Woods, Deluded, Envious, Miserable, Twice Miserable).
    State { player: u8, state: CardId, taken: bool },
    /// `player` takes `n` VP tokens from a pile, a Landmark or the supply of tokens (`source`: the card, Event or Landmark that gave them).
    VpTokens { player: u8, source: CardId, n: u16 },
    /// `n` VP tokens are put on the pile or Landmark `pile` (a Gathering card, Defiled Shrine, Aqueduct).
    PileVp { pile: CardId, n: u8 },
    /// `n` Debt tokens are put on the Supply pile `pile` (Tax).
    PileDebt { pile: CardId, n: u8 },
    /// `player` removes a Sun token from the Prophecy (`source`: the Omen); `left` are left on it.
    Sun { player: u8, source: CardId, left: u8 },
    /// The last Sun token left the Prophecy (removed by `player`): its rule is in effect from now on.
    Prophecy { prophecy: CardId, player: u8 },

    /// `card` goes back to its pile (a played Spoils or Madman, a Hermit exchanged for a Madman).
    Return { player: u8, card: CardId },
    /// `player` takes `card` out of the trash into their hand (Fortress, when it is trashed).
    TakeFromTrash { player: u8, card: CardId },
    /// `player` puts their whole deck (`n` cards) into their discard pile (Scavenger).
    DeckToDiscard { player: u8, n: u16 },
    /// `player` puts `card` onto their Tavern mat (a Reserve card played, Miser's Copper).
    ToTavern { player: u8, card: CardId },
    /// `player` calls the Reserve card `card` from their Tavern mat.
    Call { player: u8, card: CardId },
    /// `player` exchanges the Traveller `from` (discarded from play) for `to`.
    Exchange { player: u8, from: CardId, to: CardId },
    /// `player` moves their token `token` (`state::TOK_*`) onto the Supply pile `pile`.
    Token { player: u8, token: u8, pile: CardId },
    /// `player` turns their Journey token over; it is face up now if `up`.
    Flip { player: u8, up: bool },
    /// `player` takes their -1 Card token (7: onto their deck) or -$1 token (8), in `cards::TOKEN_NAMES` numbering.
    MinusToken { player: u8, token: u8 },
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
            // The -$1 token takes $1 from the first $ the turn's player gets (Adventures).
            if self.turn.adv.tax_live {
                self.settle_coin_tax();
            }
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
                        target: self.decision_target(),
                    };
                    self.set_pending(Pending::Decision(d));
                    if self.auto_single {
                        let mut buf = ChoiceBuf::default();
                        self.legal_choices(&mut buf);
                        debug_assert!(!buf.is_empty(), "decision {:?} with no legal choices", d);
                        if buf.len() == 1 {
                            self.set_pending(Pending::None);
                            self.apply_unchecked(kind, buf.as_slice()[0], sink);
                            if self.turn.adv.tax_live {
                                self.settle_coin_tax();
                            }
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

    /// Like `apply`, for a caller that has just filled `legal` from `legal_choices` on this very state: the choice is checked against
    /// that list instead of computing the legal choices a second time.
    pub fn apply_listed<S: EventSink>(&mut self, choice: Choice, legal: &ChoiceBuf, sink: &mut S) -> Result<(), &'static str> {
        let Pending::Decision(d) = self.pending() else {
            return Err("no decision pending");
        };
        if !legal.contains(choice) {
            return Err("illegal choice");
        }
        self.set_pending(Pending::None);
        self.apply_unchecked(d.kind, choice, sink);
        if self.turn.adv.tax_live {
            self.settle_coin_tax();
        }
        Ok(())
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
        if self.turn.adv.tax_live {
            self.settle_coin_tax();
        }
        Ok(())
    }

    /// Chance-mode answer: the top card of `player`'s deck is `card` (must be in `deck_unknown`).
    pub fn resolve_chance(&mut self, player: u8, card: CardId) {
        assert_eq!(self.pending(), Pending::Chance(player), "no chance event pending for player");
        if player >= crate::state::BOON_CHANCE {
            // A Boon or Hex draw: the outcome (lane `card` = the Boon or Hex's index) is the next one drawn.
            if player == crate::state::BOON_CHANCE {
                self.noc.boon_top = card as u8 + 1;
            } else {
                self.noc.hex_top = card as u8 + 1;
            }
            self.set_pending(Pending::None);
            return;
        }
        let ps = &mut self.players[player as usize];
        assert!(ps.deck_unknown.remove(card), "chance outcome not in deck");
        ps.deck_known.push_top(card);
        self.set_pending(Pending::None);
    }

    /// Distribution for a pending chance event: card counts in the unknown part of the deck.
    pub fn chance_outcomes(&self, player: u8) -> Counts {
        if player >= crate::state::BOON_CHANCE {
            // The Boons (or Hexes) of the deck: lane `i` is the card `i` of the 12, each equally likely.
            let mask = if player == crate::state::BOON_CHANCE { self.noc.boon_deck } else { self.noc.hex_deck };
            let mut out = Counts::EMPTY;
            for i in 0..12 {
                if mask & (1 << i) != 0 {
                    out.set(i, 1);
                }
            }
            return out;
        }
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
                // Snowy Village: further +Actions are ignored (the Actions are capped, and a Villager gives nothing).
                if self.turn.men.flags & crate::state::MT_SNOWY != 0 {
                    self.turn.actions = self.turn.actions.min(self.turn.men.cap);
                }
                if (self.turn.actions > 0 || (self.players[p].ren.villagers > 0 && self.turn.men.flags & crate::state::MT_SNOWY == 0))
                    && (self.players[p].hand.any_type(ACTION)
                        || (self.players[p].inherit != 0 && self.players[p].hand.has(id::ESTATE))
                        || self.rs_can_play_extra(p as u8))
                {
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
                // A Treasure played here left frames to resolve first (Menagerie: Kiln's gain, stockpile's Exile).
                if !self.stack.is_empty() {
                    return Run::Continue;
                }
                if has_choice_treasure {
                    Run::Decide(DecisionKind::PlayTreasure, 0)
                } else if (self.can_buy() || self.debt_payment_open()) && !self.turn.treasury_offered {
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
            Phase::Night => {
                if self.turn.noc.night_over {
                    self.cleanup(sink);
                    return Run::Continue;
                }
                if self.players[p].hand.any_type(cards::NIGHT) {
                    Run::Decide(DecisionKind::PlayNight, 0)
                } else {
                    self.end_night();
                    Run::Continue
                }
            }
            Phase::CleanupDraw => {
                // Allies: Island Folk, at the end of the turn (the next hand is drawn): 5 Favors for an extra turn.
                if self.al.flags & crate::state::AF_ISLAND != 0 && !self.turn.al.island_done {
                    self.turn.al.island_done = true;
                    if self.al_island_offer() {
                        return Run::Continue;
                    }
                }
                // Extra turns (Outpost) don't count towards the tie-break "fewer turns wins".
                if !self.turn.is_extra_turn {
                    self.players[p].turns_taken += 1;
                }
                // Renaissance: with Projects in the game, Fleet's extra round decides whether the game is over (and who plays next in it).
                let mut fleet_next = None;
                let over = if self.ren.projects == 0 { self.end_condition_met() } else { self.ren_game_over(p as u8, &mut fleet_next) };
                if over {
                    self.turn.phase = Phase::GameOver;
                    sink.event(Event::GameOver);
                } else {
                    // Outpost: an extra turn keeps the same player instead of advancing; the new
                    // turn's own `is_extra_turn` is set so a 2nd Outpost played during it can't
                    // grant a 3rd turn in a row (see `cleanup`, which computed this).
                    let mut extra = self.turn.outpost_grants_extra && fleet_next.is_none();
                    let mut no_buy = self.turn.adv.next_no_buy;
                    let mut voyage_next = self.turn.al.voyage_next;
                    let mut next = match fleet_next {
                        Some(q) => q,
                        None if extra => p as u8,
                        None => ((p + 1) % self.num_players as usize) as u8,
                    };
                    let mut number = self.turn.number;
                    // Allies: a player whose Lich says so skips the turn that would be theirs; the next player's turn follows.
                    if self.al.hook & crate::state::AH_SKIP != 0 {
                        while self.players[next as usize].al.skips > 0 {
                            self.players[next as usize].al.skips -= 1;
                            // A skipped turn still counts for the tie-break (an extra turn never does).
                            if !extra {
                                self.players[next as usize].turns_taken += 1;
                            }
                            sink.event(Event::Skip { player: next });
                            number += 1;
                            extra = false;
                            no_buy = false;
                            voyage_next = false;
                            next = (next + 1) % self.num_players;
                        }
                        self.al_update_hook();
                    }
                    self.turn = TurnState::start(next, number + 1);
                    self.turn.is_extra_turn = extra;
                    // Allies: a Voyage turn plays 3 cards from the hand at most.
                    if voyage_next && extra {
                        self.turn.al.limited = true;
                        self.al.hook |= crate::state::AH_LIMIT;
                    }
                    // What is in play now was played on an earlier turn (Durations held over): `ledger.carried`, for Scepter (the only card that reads it, so only games with a Scepter pay).
                    if self.in_supply(crate::sets::renaissance::ids::SCEPTER) {
                        self.note_carried(next);
                    }
                    // Flourishing Trade: cards cost $1 less (a reduction that every turn starts with).
                    if self.rs.active == id::FLOURISHING_TRADE {
                        self.turn.cost_reduction = 1;
                    }
                    // Renaissance: Canal makes every card cost $1 less on its owner's turns.
                    if self.ren.projects != 0 && self.has_project(next, id::CANAL) {
                        self.turn.cost_reduction = self.turn.cost_reduction.saturating_add(1);
                    }
                    // Adventures: a Mission turn buys no cards; the -$1 token takes $1 from the first $ of the turn.
                    self.turn.adv.no_buy = no_buy;
                    if self.players[next as usize].adv & crate::state::ADV_MINUS_COIN != 0 {
                        self.turn.adv.tax_live = true;
                    }
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
            DecisionKind::PlayAction if self.turn.actions == 0 => {
                // Out of Actions: a Villager buys one (spending one earlier would be no better).
                if self.players[p].ren.villagers > 0 && self.turn.men.flags & crate::state::MT_SNOWY == 0 {
                    out.push(Choice::Villager);
                }
                out.push(Choice::Pass);
            }
            DecisionKind::PlayAction => {
                let estates = self.estates_are_actions(p as u8);
                // Allies: a Warlord's Attack and a Voyage turn limit what can be played from the hand.
                let limited = self.al.hook != 0;
                for (c, _) in self.players[p].hand.iter() {
                    if (cards::is(c, ACTION) || (estates && c == id::ESTATE)) && !(limited && self.al_play_blocked(p as u8, c)) {
                        out.push(Choice::Card(c));
                    }
                }
                // Rising Sun: the Shadow cards of the deck, and Treasures in the Action phase under Enlightenment.
                if self.rs.flags & crate::state::R_SHADOW != 0 {
                    self.push_shadow_choices(p as u8, out);
                }
                if self.rs.active == id::ENLIGHTENMENT {
                    for (c, _) in self.players[p].hand.iter() {
                        if !cards::is(c, ACTION) && self.is_treasure(c) {
                            out.push(Choice::Card(c));
                        }
                    }
                }
                // Change plays differently with Debt: paying it off first (with $ from earlier plays) lets Change remodel.
                if self.players[p].debt > 0 && self.turn.coins > 0 && self.players[p].hand.has(id::CHANGE) {
                    out.push(Choice::PayDebt);
                }
                out.push(Choice::Pass);
            }
            DecisionKind::Buy => {
                if self.players[p].debt > 0 {
                    self.debt_buy_choices(p, out);
                    return;
                }
                // A Mission turn buys no cards (Events still).
                if !self.turn.adv.no_buy && self.can_buy() {
                    // No cost modifier in play (the usual case): the printed cost, without `cost`'s per-card checks.
                    let plain = self.turn.cost_reduction == 0
                        && !self.in_supply(id::QUARRY)
                        && !self.in_supply(id::PEDDLER)
                        && self.adv & crate::state::G_COST_TOKEN == 0
                        && self.men.flags & crate::state::M_COST == 0;
                    // Nocturne: Deluded, no Action can be bought this turn.
                    let no_actions = self.turn.noc.deluded;
                    if plain {
                        // The piles whose printed cost is within the $ (and has no Potion unless there is one), by mask; those with cards left.
                        let masks = cards::buy_masks();
                        let afford = &masks.le[(self.turn.coins as usize).min(masks.le.len() - 1)];
                        let mut m = [self.in_supply.lo & afford[0], self.in_supply.hi & afford[1], self.in_supply.top & afford[2], self.in_supply.ext & afford[3]];
                        if self.turn.potions == 0 {
                            for (w, np) in m.iter_mut().zip(masks.no_potion) {
                                *w &= np;
                            }
                        }
                        for c in crate::state::SupplyCards::from_words(m[0], m[1], m[2], m[3]) {
                            if self.supply.get(c) > 0 && self.may_buy(c) && !(no_actions && cards::is(c, ACTION)) {
                                out.push(Choice::Card(c));
                            }
                        }
                    } else {
                        for c in self.supply_cards() {
                            if self.supply.get(c) > 0
                                && self.cost(c) as u16 <= self.turn.coins
                                && (self.turn.potions > 0 || !cards::potion_cost(c))
                                && self.may_buy(c)
                                && !(no_actions && cards::is(c, ACTION))
                            {
                                out.push(Choice::Card(c));
                            }
                        }
                    }
                }
                // Events (Adventures, Empires): bought like cards, for their cost and a Buy.
                if self.events != 0 && self.can_buy() {
                    for e in cards::FIRST_EVENT..cards::FIRST_EVENT + cards::NUM_EVENTS as CardId {
                        if crate::sets::adventures::event_buyable(self, e) {
                            out.push(Choice::Card(e));
                        }
                    }
                }
                // Animal Fair (Menagerie): bought by trashing an Action card from the hand instead of paying.
                if self.in_supply(crate::sets::menagerie::ids::ANIMAL_FAIR) && self.alt_buy_possible(crate::sets::menagerie::ids::ANIMAL_FAIR) {
                    out.push(Choice::Alt(crate::sets::menagerie::ids::ANIMAL_FAIR));
                }
                // Projects (Renaissance): bought like Events, for their cost and a Buy.
                if self.ren.projects != 0 && self.can_buy() {
                    let mut m = self.ren.projects;
                    while m != 0 {
                        let pr = cards::FIRST_PROJECT + m.trailing_zeros() as CardId;
                        m &= m - 1;
                        if self.project_buyable(pr) {
                            out.push(Choice::Card(pr));
                        }
                    }
                }
                if self.players[p].coffers > 0 {
                    out.push(Choice::Coffers);
                }
                out.push(Choice::Pass);
            }
            DecisionKind::PlayNight => {
                for (c, _) in self.players[p].hand.iter() {
                    if cards::is(c, cards::NIGHT) {
                        out.push(Choice::Card(c));
                    }
                }
                out.push(Choice::Pass);
            }
            DecisionKind::PlayTreasure => {
                // Only Alms to decide (the Treasures are all plain): Alms, or play the Treasures.
                if self.treasure_order_matters() {
                    for (c, _) in self.players[p].hand.iter() {
                        if self.is_treasure(c) {
                            out.push(Choice::Card(c));
                        }
                    }
                }
                if self.alms_window() {
                    out.push(Choice::Card(id::ALMS));
                }
                // Fortune doubles the $ in hand: Coffers tokens are spent before it is played (the Debt is paid after).
                if self.players[p].coffers > 0 && self.players[p].hand.has(id::FORTUNE) {
                    out.push(Choice::Coffers);
                }
                out.push(Choice::Pass);
            }
            _ => unreachable!(),
        }
    }

    /// The Buy decision while the player has Debt: no card or Event can be bought until it is paid off ($ for tokens, at any time of the turn;
    /// the Coffers token is spent here too).
    #[cold]
    #[inline(never)]
    fn debt_buy_choices(&self, p: usize, out: &mut ChoiceBuf) {
        if self.turn.coins > 0 {
            out.push(Choice::PayDebt);
            // With Change in the game, one token of Debt can be worth keeping: leave it unpaid.
            if self.emp.flags & crate::state::E_KEEP_DEBT != 0 && self.players[p].debt > 1 {
                out.push(Choice::PayDebtKeep(1));
            }
        }
        if self.players[p].coffers > 0 {
            out.push(Choice::Coffers);
        }
        out.push(Choice::Pass);
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
            (_, Choice::PayDebt) => self.pay_debt(p, sink),
            (_, Choice::PayDebtKeep(n)) => self.pay_debt_keeping(p, n as u16, sink),
            (Phase::Action, Choice::Card(c)) => {
                self.turn.actions -= 1;
                // Allies: a Voyage turn counts the cards played from the hand.
                if self.al.hook & crate::state::AH_LIMIT != 0 {
                    self.turn.al.hand_plays = self.turn.al.hand_plays.saturating_add(1);
                }
                if self.turn.men.flags & crate::state::MT_SNOWY != 0 {
                    self.turn.men.cap = self.turn.men.cap.saturating_sub(1);
                }
                // A Shadow card with a copy in the deck is played from there.
                let from_deck = self.plays_from_deck(p, c);
                let ps = &mut self.players[p as usize];
                if from_deck {
                    ps.take_from_deck(c);
                } else {
                    ps.hand.remove(c);
                }
                ps.in_play.add(c, 1);
                // Not marked held-for-cleanup here: see `push_duration_pending`.
                sink.depth(0);
                sink.event(Event::Play { player: p, card: c });
                self.resolve_effects(c, p, 0, sink);
            }
            (Phase::Action, Choice::Villager) => self.spend_villager(p, sink),
            (Phase::Action, _) => self.enter_buy(sink),
            // Nocturne: a Night card played in the Night phase costs no Action.
            (Phase::Night, Choice::Card(c)) => {
                let ps = &mut self.players[p as usize];
                ps.hand.remove(c);
                ps.in_play.add(c, 1);
                sink.depth(0);
                sink.event(Event::Play { player: p, card: c });
                self.resolve_effects(c, p, 0, sink);
            }
            (Phase::Night, _) => self.end_night(),
            (Phase::Buy, Choice::Coffers) => {
                self.players[p as usize].coffers -= 1;
                self.turn.coins += 1;
                sink.event(Event::SpendCoffers { player: p, n: 1 });
            }
            // An Event offered with the Treasures (Alms, before they are played).
            (Phase::Buy, Choice::Card(e)) if cards::is_event(e) => self.buy_event(e, p, sink),
            (Phase::Buy, Choice::Card(pr)) if cards::is_project(pr) => self.buy_project(pr, p, sink),
            (Phase::Buy, Choice::Card(c)) if kind == DecisionKind::PlayTreasure && self.plain_treasure(c) => {
                if self.al.hook & crate::state::AH_LIMIT != 0 {
                    self.turn.al.hand_plays = self.turn.al.hand_plays.saturating_add(1);
                }
                self.play_free_treasure(c, 1, sink);
            }
            (Phase::Buy, Choice::Card(c)) if kind == DecisionKind::PlayTreasure => {
                if self.al.hook & crate::state::AH_LIMIT != 0 {
                    self.turn.al.hand_plays = self.turn.al.hand_plays.saturating_add(1);
                }
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
            (Phase::Buy, Choice::Alt(c)) => self.buy_alt(c, p, sink),
            (Phase::Buy, Choice::Card(c)) => {
                self.turn.coins -= self.cost(c) as u16;
                if cards::potion_cost(c) {
                    self.turn.potions -= 1;
                }
                self.spend_buy();
                self.turn.treasures_done = true;
                sink.depth(0);
                sink.event(Event::Buy { player: p, card: c });
                sink.depth(1);
                // A card with Debt in its cost: buying it takes the tokens (before it is gained, which runs its triggers).
                if cards::debt_cost(c) > 0 {
                    self.take_debt(p, c, cards::debt_cost(c) as u16, sink);
                }
                // Overpay (Cornucopia & Guilds): with something to spare, ask how much first.
                if crate::sets::overpay_possible(self, c) {
                    self.stack.push(Frame { depth: 1, ..Frame::new(FrameKind::Overpay, p, c) });
                } else {
                    self.gain(p, c, Dest::Discard, true, NO_SOURCE, sink);
                    self.on_buy_effects(p, c, sink);
                }
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
        // Nocturne: Deluded and Envious are returned now and apply for the rest of the turn.
        if self.players[self.turn.player as usize].noc & (crate::state::P_DELUDED | crate::state::P_ENVIOUS) != 0 {
            self.states_at_buy(sink);
        }
        // Arena (Empires): at the start of each Buy phase an Action may be discarded for VP tokens.
        if self.emp.landmarks != 0 {
            self.start_of_buy_landmarks();
        }
        // Renaissance: the Treasure Chest gains a Gold at the start of each Buy phase.
        if self.ren.holder[(id::TREASURE_CHEST - cards::FIRST_ARTIFACT) as usize] == self.turn.player + 1 {
            self.gain(self.turn.player, id::GOLD, Dest::Discard, false, id::TREASURE_CHEST, sink);
        }
        // Allies: the Ally's start-of-Buy-phase ability (Family of Inventors, League of Bankers, Market Towns, Peaceful Cult, Woodworkers' Guild).
        if self.al.flags & crate::state::AF_BUY != 0 {
            self.al_buy_start(sink);
        }
    }

    /// Start of (or return to) the Buy phase. Choice-free Treasures (Copper, Silver, Gold, ...;
    /// see `cards::is_choice_free`) are auto-played, all at once, unless the hand also holds a
    /// Treasure with a choice (Investment, Anvil, Tiara, Crystal Ball, War Chest, Bank): then
    /// order matters (Investment counts and may trash Treasures still in hand, Anvil discards
    /// one, Tiara replays one, Bank counts those already played), so nothing is auto-played and
    /// every Treasure in hand is offered through the `PlayTreasure` decision. Returns whether
    /// that decision is needed.
    fn play_choice_free_treasures<S: EventSink>(&mut self, sink: &mut S) -> bool {
        if self.treasure_order_matters() || self.alms_window() {
            return true;
        }
        // Allies: a Voyage turn whose plays are used up plays no more Treasures.
        if self.al.hook & crate::state::AH_LIMIT != 0 {
            return false;
        }
        self.play_all_choice_free_treasures(sink);
        false
    }

    /// Whether the order the hand's Treasures are played in matters (so they are offered one by one instead of played at once): a
    /// Treasure with a choice is in the hand, or a Sauna makes the Silvers' order matter.
    fn treasure_order_matters(&self) -> bool {
        let hand = &self.players[self.turn.player as usize].hand;
        // Allies: a Voyage turn plays 3 cards from the hand in all, so the Treasures are chosen one by one until they are used up; under a Highwayman's Attack the
        // first Treasure played does nothing, so which one it is matters.
        if self.al.hook & (crate::state::AH_LIMIT | crate::state::AH_HW) != 0 {
            if let Some(r) = self.al_treasures_by_choice() {
                return r;
            }
        }
        if hand.iter().any(|(c, _)| self.is_treasure(c) && (self.ask_treasures || !self.plain_treasure(c))) {
            return true;
        }
        // Sauna: each Silver played may trash a card from the hand, so the order of the plays matters (a Copper trashed before it is played).
        self.turn.adv.sauna > 0 && hand.has(id::SILVER)
    }

    /// Adventures: Alms ("if you have no Treasures in play, gain a card costing up to $4") can only be bought before the Treasures are
    /// played, so while it could still do something the Buy phase starts with a `PlayTreasure` decision (buy Alms, or play the
    /// Treasures) instead of playing them. Games without Alms pay one bit test.
    #[inline(always)]
    fn alms_window(&self) -> bool {
        self.events & (1u64 << (id::ALMS - cards::FIRST_EVENT)) != 0 && crate::sets::adventures::alms_window(self)
    }

    /// Play every choice-free Treasure in the current player's hand.
    fn play_all_choice_free_treasures<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let hand = self.players[p as usize].hand;
        // Copper, Silver and Gold are played by a short path unless a card of this game acts on their plays: Envious (Silver and Gold make $1),
        // Sauna (a trash for each Silver) or Corsair (the first Silver or Gold of a turn is trashed).
        let plain = !self.turn.noc.envy
            && self.turn.adv.sauna == 0
            && !self.in_supply(id::CORSAIR)
            && self.turn.men.kiln == 0;
        for (c, n) in hand.iter() {
            if c <= id::GOLD && plain {
                self.play_plain_treasure(c, n, sink);
            } else if self.is_treasure(c) && self.plain_treasure(c) {
                if matches!(c, id::COPPER | id::SILVER | id::GOLD | id::PLATINUM | id::POTION) {
                    self.play_free_treasure(c, n, sink);
                } else {
                    // One copy at a time: each play reports its own $ (the first Fool's Gold $1, the next $4).
                    for _ in 0..n {
                        self.play_free_treasure(c, 1, sink);
                    }
                }
            }
        }
    }

    /// `play_free_treasure` of `n` Coppers, Silvers or Golds from the hand when nothing acts on them (see `play_all_choice_free_treasures`):
    /// the same plays and events, without the checks for the other Treasures' extras.
    #[inline(always)]
    fn play_plain_treasure<S: EventSink>(&mut self, c: CardId, n: u8, sink: &mut S) {
        debug_assert!(c <= id::GOLD);
        let p = self.turn.player;
        // Rising Sun: Panic's Buys, Good Harvest's Buy and $.
        if self.rs.active != 0 {
            self.rs_treasure_plays(c, n, sink);
        }
        let ps = &mut self.players[p as usize];
        ps.hand.set(c, ps.hand.get(c) - n);
        ps.in_play.add(c, n);
        self.turn.played.add(c, n);
        for _ in 0..n {
            sink.event(Event::Play { player: p, card: c });
        }
        self.turn.coins += cards::def(c).coins as u16 * n as u16;
        let merchant = if c == id::SILVER && self.turn.silvers_played == 0 { self.turn.merchants as u16 } else { 0 };
        sink.depth(1);
        sink.bonus(p, c, 0, 0, 0, 0);
        sink.bonus(p, id::MERCHANT, 0, 0, merchant, 0);
        sink.depth(0);
        if c == id::SILVER {
            self.turn.coins += merchant;
            self.turn.silvers_played += n;
        }
    }

    /// Buy the Event `e` (Adventures): its cost and a Buy, then its effect.
    #[inline(never)]
    fn buy_event<S: EventSink>(&mut self, e: CardId, p: u8, sink: &mut S) {
        self.turn.coins -= cards::cost(e) as u16;
        self.spend_buy();
        // Alms bought before the Treasures leaves them to be played; every other purchase ends the Treasure plays.
        if !(e == id::ALMS && self.treasures_in_play(p) == 0) {
            self.turn.treasures_done = true;
        }
        if cards::debt_cost(e) > 0 {
            self.take_debt(p, e, cards::debt_cost(e) as u16, sink);
        }
        let bit = 1u64 << (e - cards::FIRST_EVENT);
        self.turn.adv.bought |= bit;
        self.players[p as usize].events_bought |= bit;
        sink.depth(0);
        sink.event(Event::Buy { player: p, card: e });
        sink.depth(1);
        let base = self.stack.len as usize;
        crate::sets::adventures::event_effect(self, e, p, sink);
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            f.depth = 1;
        }
    }

    /// Play `n` copies of the choice-free Treasure `c` from the current player's hand.
    pub(crate) fn play_free_treasure<S: EventSink>(&mut self, c: CardId, n: u8, sink: &mut S) {
        self.play_free_treasure_from(c, n, true, sink);
    }

    /// `play_free_treasure`; with `fresh` false the card is already in play and is played again (Crown plays a Treasure twice).
    pub(crate) fn play_free_treasure_from<S: EventSink>(&mut self, c: CardId, n: u8, fresh: bool, sink: &mut S) {
        let p = self.turn.player;
        // Allies: Highwayman stops the first Treasure the player plays each turn from doing anything (it is played all the same).
        if self.al.flags & crate::state::AF_HW != 0 {
            if n > 1 && !self.turn.al.t1 {
                self.play_free_treasure_from(c, 1, fresh, sink);
                return self.play_free_treasure_from(c, n - 1, fresh, sink);
            }
            if self.al_first_treasure_blocked(p) {
                let ps = &mut self.players[p as usize];
                if fresh {
                    ps.hand.set(c, ps.hand.get(c) - n);
                    ps.in_play.add(c, n);
                }
                self.turn.played.add(c, n);
                for _ in 0..n {
                    sink.event(Event::Play { player: p, card: c });
                }
                return;
            }
        }
        // Rising Sun: Panic's Buys, Good Harvest's Buy and $.
        if self.rs.active != 0 {
            self.rs_treasure_plays(c, n, sink);
        }
        // Kiln: the card played now may come with a copy first (asked before anything else of the play).
        if self.turn.men.kiln > 0 {
            self.kiln_hook(c, p);
        }
        let coins_before = self.turn.coins;
        let ps = &mut self.players[p as usize];
        if fresh {
            ps.hand.set(c, ps.hand.get(c) - n);
            ps.in_play.add(c, n);
        }
        self.turn.played.add(c, n);
        for _ in 0..n {
            sink.event(if fresh { Event::Play { player: p, card: c } } else { Event::PlayAgain { player: p, card: c, source: id::CROWN, nth: 2 } });
        }
        // Plunder: a VP token each time it is played.
        if c == id::PLUNDER {
            self.gain_vp_tokens(p, c, n as u16, sink);
        }
        self.turn.coins += cards::def(c).coins as u16 * n as u16;
        // Fool's Gold: $1 the first time one is played this turn, $4 after (the plays of this turn so far, this one included, are counted).
        if c == id::FOOLS_GOLD {
            let before = self.turn.played.get(c).saturating_sub(n);
            self.turn.coins += if before == 0 { 3 * (n as u16 - 1) } else { 3 * n as u16 };
        }
        // Nocturne: Envious, Silver and Gold make $1.
        if self.turn.noc.envy && (c == id::SILVER || c == id::GOLD) {
            self.turn.coins -= (cards::def(c).coins as u16 - 1) * n as u16;
        }
        self.turn.buys += cards::def(c).buys * n;
        self.turn.potions += cards::def(c).potions * n;
        // Philosopher's Stone's value is dynamic, so it is shown like Bank's; so is Huge Turnip's ($1 per
        // Coffers, the two it gives each time included).
        let mut stone = if c == id::PHILOSOPHERS_STONE {
            self.philosophers_stone_coins(p) * n as u16
        } else if c == id::RICE {
            self.rice_coins(p) * n as u16
        } else {
            0
        };
        if c == id::HUGE_TURNIP {
            for _ in 0..n {
                self.give_coffers(p, c, 2, sink);
                stone += self.players[p as usize].coffers;
            }
        }
        // Ducat (Renaissance): +1 Coffers each time.
        if c == id::DUCAT {
            self.give_coffers(p, c, n as u16, sink);
        }
        // Menagerie: Supplies gains a Horse onto the deck, Stockpile Exiles itself.
        if c == crate::sets::menagerie::ids::SUPPLIES || c == crate::sets::menagerie::ids::STOCKPILE {
            self.men_treasure(c, n, sink);
        }
        self.turn.coins += stone;
        // The $ the play made, indented under it ("+$1", "+$4" for a second Fool's Gold), and its extras (+Buy, Merchant's +$1); not for the
        // basic Treasures, whose value everyone knows (Copper, Silver, Gold, Platinum, Potion).
        let basic = matches!(c, id::COPPER | id::SILVER | id::GOLD | id::PLATINUM | id::POTION);
        let own_coins = if basic { 0 } else { self.turn.coins - coins_before };
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
        // Card ids are fixed in ascending order (Copper < Silver < Gold), so this loop's
        // iteration order already gives Silver priority over Gold when both are in hand,
        // matching "the first ... they play".
        if c == id::SILVER || c == id::GOLD {
            self.corsair_trashes_first_silver_or_gold(p, c, sink);
        }
        // Sauna: "this turn, when you play a Silver, you may trash a card from your hand", once per Sauna played, for each Silver.
        if c == id::SILVER && self.turn.adv.sauna > 0 {
            for _ in 0..n as u16 * self.turn.adv.sauna as u16 {
                self.stack.push(Frame { depth: 1, ..crate::effects::select(p, id::SAUNA, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing) });
            }
        }
        // Spoils (+$3) goes back to the Spoils pile when it is played.
        if c == id::SPOILS {
            let ps = &mut self.players[p as usize];
            let back = n.min(ps.in_play.get(c));
            ps.in_play.set(c, ps.in_play.get(c) - back);
            self.spoils += back;
            for _ in 0..back {
                sink.event(Event::Return { player: p, card: c });
            }
        }
    }

    /// The offers made at the end of the Buy phase, before cleanup discards everything in play (Alchemist, Walled Village,
    /// Treasury, Herbalist): each set pushes its own as the generic `YesNo` / `Select` frames (`sets::end_of_turn_offers`
    /// fixes the order). Cheaply gated: a game without these cards pays a few bitmask tests.
    fn push_end_of_turn_offers(&mut self) {
        crate::sets::end_of_turn_offers(self);
    }

    /// End the Buy phase: the end-of-turn offers (once, guarded by `treasury_offered`), then cleanup
    /// once nothing more is pending from it. Reached both when buys run out naturally
    /// (`run_phase`'s `Phase::Buy` arm) and when the player passes with buys still available
    /// (`apply_phase`'s Buy-decision catch-all), so the offer fires exactly once at the true end
    /// of the Buy phase regardless of which path got there.
    fn end_buy_phase<S: EventSink>(&mut self, sink: &mut S) {
        if !self.turn.treasury_offered {
            self.turn.treasury_offered = true;
            crate::sets::end_of_buy(self, sink);
            // Nocturne: the Night phase comes before Clean-up, so the offers wait for it.
            if !self.night_follows() {
                self.turn.noc.night_over = true;
                self.push_end_of_turn_offers();
            }
            if !self.stack.is_empty() {
                return;
            }
        }
        if !self.turn.noc.night_over {
            if self.night_follows() {
                self.turn.phase = Phase::Night;
                sink.event(Event::PhaseStart { player: self.turn.player, phase: Phase::Night });
                return;
            }
            self.end_night();
            if !self.stack.is_empty() {
                return;
            }
        }
        self.cleanup(sink);
    }

    /// Whether a Night phase follows this Buy phase: the game has Night cards and the player holds one. One byte test in games without.
    #[inline(always)]
    fn night_follows(&self) -> bool {
        self.noc.flags & crate::state::N_NIGHT != 0 && self.players[self.turn.player as usize].hand.any_type(cards::NIGHT)
    }

    /// The end of the Night phase (or of a Buy phase that no Night phase follows): the end-of-turn offers, once.
    #[inline(never)]
    fn end_night(&mut self) {
        self.turn.noc.night_over = true;
        self.push_end_of_turn_offers();
    }

    /// Cleanup, Prince: a Prince holding a card (and a Throne Room / King's Court that played it)
    /// stays in play for the rest of the game; `kept` is what stays.
    #[cold]
    #[inline(never)]
    fn prince_keep(&self, kept: &mut Counts, play: &Counts) {
        for (c, n) in self.players[self.turn.player as usize].stays.iter() {
            kept.set(c, kept.get(c).saturating_add(n).min(play.get(c)));
        }
    }

    fn cleanup<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player as usize;
        sink.event(Event::PhaseStart { player: p as u8, phase: Phase::CleanupDraw });
        // Empires: Capital's Debt when it is discarded from play, Baths, and the $ left over paying off Debt.
        if self.in_supply(id::CAPITAL) || self.emp.landmarks != 0 || self.players[p].debt != 0 {
            self.empires_cleanup(sink);
        }
        // Smugglers' gain record: snapshot this turn's gains as "gained on their last turn" for
        // the player whose turn just ended (see `gain`; empty when Smugglers isn't in play, so
        // this is a cheap no-op copy for Base-only games).
        if !self.turn.gained_this_turn.is_empty() || !self.players[p].last_turn_gains.is_empty() {
            self.players[p].last_turn_gains = self.turn.gained_this_turn;
        }
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
        // Mission (Adventures): an extra turn too, unless this is one already or an Outpost's is taken instead; the extra turn buys no cards.
        let mission = self.turn.adv.mission && !self.turn.is_extra_turn && !grant_extra;
        self.turn.adv.next_no_buy = mission;
        // Seize the Day (Menagerie): an extra turn after this one, with no check on the previous turn.
        self.turn.outpost_grants_extra = grant_extra || mission || self.turn.men.flags & crate::state::MT_SEIZE != 0;
        // Allies: Voyage's extra turn (not a 3rd in a row), in which 3 cards may be played from the hand; another extra turn pending is taken instead.
        if self.turn.al.voyage && !self.turn.is_extra_turn {
            self.turn.al.voyage_next = !self.turn.outpost_grants_extra;
            self.turn.outpost_grants_extra = true;
        }
        if self.al.hook & crate::state::AH_LIMIT != 0 {
            self.al.hook &= !crate::state::AH_LIMIT;
        }
        // Goatherd (Menagerie): the cards trashed this turn, for the player to the left.
        if self.men.flags & crate::state::M_TRASHES != 0 {
            self.players[p].men.trashed_last = self.turn.men.trashed;
        }
        // Farrier's overpay adds to the hand drawn now.
        let draw_n = (if outpost_played { 3u8 } else { 5 }).saturating_add(self.turn.extra_draw);
        // The Flag: +1 Card when drawing the hand.
        let draw_n = draw_n + (self.ren.holder[(id::FLAG - cards::FIRST_ARTIFACT) as usize] == p as u8 + 1) as u8;
        // Way of the Squirrel (Menagerie): +2 Cards at the end of the turn for each play, whoever's turn it is.
        let draw_n = if self.turn.men.squirrel.iter().any(|&x| x != 0) { self.squirrel_draws(p as u8, draw_n) } else { draw_n };
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

        // Rising Sun: Panic returns the Treasures discarded from play to their piles, Biding Time sets the hand aside.
        if self.rs.active != 0 {
            self.rs_cleanup(sink);
        }
        if self.turn.duration_held.is_empty() && self.players[p].stays.is_empty() {
            // The common turn: nothing stays in play, so the hand and play area both discard.
            let ps = &mut self.players[p];
            ps.discard.add_all(&ps.hand);
            ps.discard.add_all(&ps.in_play);
            ps.hand.clear();
            ps.in_play.clear();
        } else {
            let hand = self.players[p].hand;
            let play = self.players[p].in_play;
            // Duration cards (and any Throne Room/King's Court that multiplied one) held for next
            // turn stay in play; everything else in play, plus the whole hand, discards as usual.
            let mut kept = Counts::EMPTY;
            for (c, n) in self.turn.duration_held.iter() {
                kept.set(c, n.min(play.get(c)));
            }
            // Prince: a Prince holding a card (and the Throne Room / King's Court that played it)
            // stays in play for the rest of the game.
            if !self.players[p].stays.is_empty() {
                self.prince_keep(&mut kept, &play);
            }
            let mut discard_part = play;
            for (c, _) in kept.iter() {
                discard_part.set(c, play.get(c) - kept.get(c));
            }
            let ps = &mut self.players[p];
            ps.discard.add_all(&hand);
            ps.discard.add_all(&discard_part);
            ps.hand.clear();
            ps.in_play = kept;
        }
        // Joust's Province: set aside until now.
        let ps = &mut self.players[p];
        if !ps.cleanup_aside.is_empty() {
            let aside = ps.cleanup_aside.take_all();
            aside.add_into(&mut ps.discard);
        }
        self.turn.phase = Phase::CleanupDraw;
        // Nocturne: Boons kept until now are discarded; River's Gift cards and set-aside Faithful Hounds come after the new hand is drawn.
        if self.noc.flags != 0 {
            self.nocturne_cleanup();
        }
        // Foresight (Rising Sun): the Actions set aside go to the hand once the new hand is drawn.
        if !self.players[p].foresight.is_empty() {
            self.stack.push(crate::effects::select(p as u8, id::FORESIGHT, Zone::Hand, Act::Discard, Filter::Any, 0, 0, Then::ForesightToHand));
        }
        // Save (Adventures): the card set aside goes to the hand once the new hand is drawn.
        if self.players[p].saved != 0 {
            self.stack.push(crate::effects::select(p as u8, id::SAVE, Zone::Hand, Act::Discard, Filter::Any, 0, 0, Then::SavedToHand));
        }
        // Coastal Haven (Allies): the cards kept in hand are added to it once the new hand is drawn.
        if self.al.flags & crate::state::AF_KEEP != 0 && !self.players[p].al.keep.is_empty() {
            self.stack.push(crate::effects::select(
                p as u8, crate::sets::allies::ids::COASTAL_HAVEN, Zone::Hand, Act::Discard, Filter::Any, 0, 0, Then::KeepToHand,
            ));
        }
        self.stack.push(Frame { max: draw_n, ..Frame::new(FrameKind::Draw, p as u8, 0) });
    }

    // ------------------------------------------------------------------
    // Primitives used by card effects
    // ------------------------------------------------------------------

    /// The Durations held over into the turn `p` is starting are not "played this turn" (`Ledger::carried`, read by Scepter).
    #[cold]
    #[inline(never)]
    fn note_carried(&mut self, p: u8) {
        let held = &self.players[p as usize];
        if held.pending_durations_len != 0 || !held.stays.is_empty() || !held.discard_next_cleanup.is_empty() {
            for (c, n) in self.players[p as usize].in_play.iter() {
                self.turn.led.carried.add_or_drop(c, n);
            }
        }
    }

    /// Remove the top card of `p`'s deck, shuffling the discard in if the deck is empty.
    /// `Ok(None)` when deck and discard are both empty.
    pub(crate) fn take_top<S: EventSink>(&mut self, p: u8, sink: &mut S) -> Result<Option<CardId>, NeedChance> {
        if let Some(c) = self.players[p as usize].deck_known.pop_top() {
            // The stop-moving rule: a tracked card that was on top is gone.
            self.cover_deck(p);
            return Ok(Some(c));
        }
        // Star Chart: the owner picks a card to go on top of the shuffled deck (a frame does the shuffle once they have).
        if self.ren.hook & crate::state::REN_STAR != 0 {
            let ps = &self.players[p as usize];
            if ps.deck_unknown.is_empty() && ps.deck_known_bottom.len == 0 && !ps.discard.is_empty() && self.star_chart_shuffle(p) {
                return Err(NeedChance(crate::state::STAR_STOP));
            }
        }
        // Allies: Order of Astrologers and Order of Masons, a shuffle is about to happen for `p` (a frame asks which cards, and shuffles once they are picked).
        if self.al.flags & crate::state::AF_SHUFFLE != 0 {
            let ps = &self.players[p as usize];
            if ps.deck_unknown.is_empty() && ps.deck_known_bottom.len == 0 && !ps.discard.is_empty() && self.al_shuffle_offer(p) {
                return Err(NeedChance(crate::state::STAR_STOP));
            }
        }
        let ps = &mut self.players[p as usize];
        // Nothing known on top: the unknown multiset is the deck; once it is empty the discard
        // pile is shuffled in (an idempotent change, so a chance step can redo this).
        let mut total = ps.deck_unknown.total();
        if total == 0 {
            // The known cards at the very bottom (Secret Passage's, the Shadow cards put there by a shuffle) are drawn once the middle is gone.
            if ps.deck_known_bottom.len > 0 {
                return Ok(ps.deck_known_bottom.pop_front());
            }
            if ps.discard.is_empty() {
                return Ok(None);
            }
            let d = ps.discard;
            ps.deck_unknown.add_all(&d);
            ps.discard.clear();
            if self.turn.led.n != 0 {
                self.turn.led.kill_zone(p, crate::track::TZone::Discard);
            }
            // Rising Sun: the Shadow cards go to the bottom of the shuffled deck.
            if self.rs.flags & crate::state::R_SHADOW != 0 {
                ps.sink_shadows();
            }
            sink.event(Event::Shuffle { player: p });
            // Allies: Emissary counts the shuffles of the turn.
            if self.al.flags & crate::state::AF_STAMP != 0 {
                self.turn.al.shuffles = self.turn.al.shuffles.saturating_add(1);
            }
            total = ps.deck_unknown.total();
            if total == 0 {
                return Ok(ps.deck_known_bottom.pop_front());
            }
        }
        if self.chance_mode {
            return Err(NeedChance(p));
        }
        let c = ps.deck_unknown.nth(self.rng.below(total));
        ps.deck_unknown.remove(c);
        Ok(Some(c))
    }

    /// Gain `c` from the supply if available, to `to`. `on_buy` is true only for the literal
    /// purchase in the Buy phase (never for Workshop/Remodel/War Chest/... gains, even though
    /// some of those happen during a buy): it gates Hoard's "if you bought it" (`on_buy_effects`
    /// holds any other buy-only effect). Runs every "when you gain" trigger (Watchtower, Hoard,
    /// Collection, Tiara, Mint's trash). Also maintains two cheap, narrowly-gated per-turn records: `p`'s
    /// gain record for Smugglers (only while Smugglers is in this game's supply) and the
    /// "gained a Victory card this Buy phase" flag for Treasury (only while in the Buy phase).
    /// Returns whether it was gained.
    pub(crate) fn gain<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, on_buy: bool, source: CardId, sink: &mut S) -> bool {
        if !self.supply.remove(c) {
            return false;
        }
        self.finish_gain(p, c, to, on_buy, source, true, sink);
        true
    }

    /// Gain `c` for `p` from the Supply to `to`, as a gain effect (Workshop's) does: runs the gain triggers. For tools and tests that set
    /// positions up; the game itself gains through frames. False when the pile has no `c` (a Knight or Ruins that is not on top).
    pub fn gain_card<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, sink: &mut S) -> bool {
        self.gain(p, c, to, false, NO_SOURCE, sink)
    }

    /// Gain `c` from a pile outside the Supply (Ferryman's pile, a Reward): the same as `gain` for
    /// what happens next, but only effects that name the pile (Ferryman's gain, Joust) may call it.
    pub(crate) fn gain_side<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, source: CardId, sink: &mut S) -> bool {
        if !self.take_side(c) {
            return false;
        }
        self.finish_gain(p, c, to, false, source, false, sink);
        true
    }

    /// Take one card of `c` from the Ferryman pile, the Rewards or the Spoils, Madman and Mercenary piles, if any is left.
    fn take_side(&mut self, c: CardId) -> bool {
        if c == id::SPOILS || c == id::MADMAN || c == id::MERCENARY {
            let n = match c {
                id::SPOILS => &mut self.spoils,
                id::MADMAN => &mut self.madman,
                _ => &mut self.mercenary,
            };
            if *n == 0 {
                return false;
            }
            *n -= 1;
            true
        } else if cards::is_reward(c) {
            let n = &mut self.rewards[(c - cards::FIRST_REWARD) as usize];
            if *n == 0 {
                return false;
            }
            *n -= 1;
            true
        } else if c == self.ferryman && c != 0 && self.ferryman_left > 0 {
            self.ferryman_left -= 1;
            true
        } else if (id::WILL_O_WISP..=id::GHOST).contains(&c) {
            // Nocturne: Will-o'-Wisp, Wish, Bat, Imp, Ghost (ids and `NocState::piles` indices line up).
            let n = &mut self.noc.piles[(c - id::WILL_O_WISP) as usize];
            if *n == 0 {
                return false;
            }
            *n -= 1;
            true
        } else {
            false
        }
    }

    /// What follows a card leaving its pile: it arrives, is logged, and its gain triggers run.
    #[inline(always)]
    pub(crate) fn finish_gain<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, on_buy: bool, source: CardId, from_supply: bool, sink: &mut S) {
        // Nocturne: the cards gained this turn are counted (Monastery, Devil's Workshop); some are gained to the hand.
        let to = if self.noc.flags & crate::state::N_GAIN_HOOK != 0 { self.gain_hook(p, c, to) } else { to };
        // Rising Sun: Progress puts every gained card onto the deck.
        let to = if self.rs.hook != 0 { self.rs_gain_dest(to) } else { to };
        // The stop-moving rule: a gain with reactions waiting on it (Watchtower, Tiara, Travelling Fair, ...), or whose gainer wants to move the
        // card afterwards (Replace, Summon, ...: `led.want`), names the copy by a token, so that whatever moves it first takes it from the
        // others. A Release frame under the reactions lets the token go once they are done.
        let possible = crate::sets::gain_triggers_possible(self);
        let tracked = self.turn.led.want || self.rs.hook != 0 || possible;
        match to {
            Dest::Hand => self.players[p as usize].hand.add(c, 1),
            Dest::DeckTop => {
                self.cover_deck(p);
                self.players[p as usize].deck_known.push_top(c)
            }
            Dest::Discard => self.players[p as usize].discard.add(c, 1),
        }
        let mut track = GainTrack::default();
        if tracked {
            track = self.gain_track_begin(p, c, to);
        }
        sink.event(Event::Gain { player: p, card: c, to, source });
        // The top card of the Knights or Ruins pile was taken: the next one is revealed (it is drawn from the cards nobody has seen).
        if from_supply {
            self.pile_taken(c);
        }
        // Smugglers' gain record: "a card the player to your right gained on their last turn"
        // means gained during *their own* turn specifically, so this excludes a victim's forced
        // gain during someone else's turn (Witch's Curse, Bandit's Gold...), which is `p !=
        // self.turn.player` here. One bitmask test (plus the player check) skips this for the
        // overwhelming majority of games (no Smugglers in the kingdom at all).
        if (self.in_supply(id::SMUGGLERS) || self.in_supply(id::PAGE)) && p == self.turn.player {
            self.turn.gained_this_turn.add_or_drop(c, 1);
        }
        if self.turn.phase == Phase::Buy {
            // Treasury looks at the Victory cards its own player gains in their Buy phase.
            if cards::is(c, cards::VICTORY) && p == self.turn.player {
                self.turn.gained_victory_in_buy = true;
            }
            // Merchant Guild counts the turn player's own gains.
            if p == self.turn.player {
                self.turn.gained_in_buy = self.turn.gained_in_buy.saturating_add(1);
            }
        }
        // Menagerie: Wayfarer's cost follows the last card gained; Destrier and Commerce count this turn's gains.
        if self.men.flags & crate::state::M_GAINS != 0 {
            self.men_track_gain(p, c);
        }
        // Rising Sun: Kintsugi's and Receive Tribute's records, Growth's cheaper card, Rapid Expansion's set-aside (answered after the triggers below).
        if self.rs.hook != 0 {
            self.rs_gain_pre(p, c, to);
        }
        // Every "when you gain" trigger (`sets::gain_triggers`, in the order it fixes; a bonus gain a trigger causes runs its own triggers first,
        // so this gain's own reactions are asked first); `possible` is the cheap test for a game with no gain-watcher card at all.
        if possible {
            crate::sets::gain_triggers(self, p, c, to, on_buy, sink);
        }
        if tracked {
            self.gain_track_end(p, c, to, track);
        }
        // Bureaucracy's Copper, Harsh Winter's Debt.
        if self.rs.hook != 0 {
            self.rs_gain_post(p, c, from_supply, sink);
        }
    }

    /// A gain with reactions waiting (or a gainer that wants to move the card afterwards) names the gained copy by a token; the `Release`
    /// frame under the reactions lets it go (`track.rs`). Cold: ordinary games never get here.
    #[cold]
    #[inline(never)]
    fn gain_track_begin(&mut self, p: u8, c: CardId, to: Dest) -> GainTrack {
        let want = std::mem::take(&mut self.turn.led.want);
        let outer = self.turn.led.ingain;
        let gt = self.track(p, Self::gain_dest_zone(to), c);
        if want {
            self.turn.led.hold(gt);
        }
        self.turn.led.ingain = gt;
        let base = self.stack.len as usize;
        self.stack.push(Frame { tok: gt, ..Frame::new(FrameKind::Release, p, c) });
        GainTrack { tok: gt, base, outer }
    }

    /// The reactions that move or play the gained card carry its token; with none waiting the token is not needed.
    #[cold]
    #[inline(never)]
    fn gain_track_end(&mut self, p: u8, c: CardId, to: Dest, t: GainTrack) {
        let gz = Self::gain_dest_zone(to);
        for fr in &mut self.stack.frames[t.base + 1..self.stack.len as usize] {
            if fr.tok == 0 && fr.player == p && fr.zone == gz && (fr.subject == c && fr.kind == FrameKind::YesNo || fr.kind == FrameKind::Select && fr.filter == Filter::Card(c)) {
                fr.tok = t.tok;
            }
        }
        if self.stack.len as usize == t.base + 1 {
            self.stack.pop();
            self.release_tok(t.tok);
        }
        self.turn.led.ingain = t.outer;
        self.turn.led.gain = t.tok;
    }

    /// The zone a gain destination corresponds to, for triggers that need to find the gained
    /// card again afterward (Watchtower, Sailor). A card gained onto the deck is gained there
    /// directly (Gain rules page), so it is the known top card of the deck: `Zone::DeckTop`, the
    /// one-card zone a reaction can trash it from or play it from.
    pub(crate) fn gain_dest_zone(to: Dest) -> Zone {
        match to {
            Dest::Discard => Zone::Discard,
            Dest::Hand => Zone::Hand,
            Dest::DeckTop => Zone::DeckTop,
        }
    }

    /// Card-specific effects that happen specifically because a card was *bought* (not gained
    /// any other way): the one place for them, parallel to `may_buy`. Runs after the bought
    /// card's own gain (and its triggers) have resolved. None at present: Mint's trash is a
    /// "when you gain this" effect (`mint_trashes_treasures_in_play`), and Hoard's "if you bought
    /// it" is checked on the gain itself (`on_buy`).
    pub(crate) fn on_buy_effects<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        let _ = (p, c, sink);
    }

    /// Moat / Lighthouse check: a Moat revealed to the Attack being resolved (`TurnState::moated`,
    /// set by the victim's choice in the reaction window); Lighthouse is a lingering static while
    /// in play ("other players' Attacks don't affect you"), so it's just an `in_play` check, no
    /// reaction event (nothing is revealed).
    pub(crate) fn immune(&self, victim: u8) -> bool {
        self.turn.moated & (1 << victim) != 0 || self.protected_while_in_play(victim) || self.players[victim as usize].noc & crate::state::P_GUARDED != 0
    }

    /// Whether `victim` has a card in play that protects its owner from other players' Attacks (`cards::IMMUNE_IN_PLAY`).
    #[inline]
    pub(crate) fn protected_while_in_play(&self, victim: u8) -> bool {
        let in_play = &self.players[victim as usize].in_play;
        cards::IMMUNE_IN_PLAY.iter().any(|&c| in_play.has(c))
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

    #[inline]
    fn decision_target(&self) -> CardId {
        match self.stack.frames[..self.stack.len as usize].last() {
            Some(Frame { kind: FrameKind::YesNo, act: Act::Call, last, .. }) => *last,
            // Pixie: the Boon the Pixie may be trashed to receive twice.
            Some(Frame { kind: FrameKind::YesNo, then: Then::PixieTrash, last, .. }) => *last,
            // Sleigh: the card gained that it may move.
            Some(Frame { kind: FrameKind::YesNo, then: Then::SleighMove, last, .. }) => *last,
            _ => 0,
        }
    }

    fn decision_play_times(&self) -> u8 {
        match self.stack.top() {
            Some(Frame { kind: FrameKind::Select, then: Then::PlayPicked { times }, .. }) => times,
            _ => 1,
        }
    }

    fn decision_source(&self) -> Option<CardId> {
        self.stack.top().filter(|f| f.kind != FrameKind::StartOrder).map(|f| f.source)
    }
}

/// What `finish_gain` keeps between `gain_track_begin` and `gain_track_end`.
#[derive(Default, Clone, Copy)]
struct GainTrack {
    tok: u8,
    base: usize,
    outer: u8,
}

/// Helper for "pick cards one at a time" decisions. Picks are forced into non-decreasing
/// card-id order (`min`) so each multiset of picks is reachable by exactly one path — this
/// keeps search trees free of duplicate orderings. `need` = picks still required (0 if optional);
/// a card is offered only if picking it still leaves enough cards (id >= it) to finish.
pub(crate) fn canonical_picks(from: &Counts, min: CardId, need: u32, filter: impl Fn(CardId) -> bool, out: &mut ChoiceBuf) {
    // One more pick (or none) needed: any card that is there and passes the filter leaves enough.
    if need <= 1 {
        for (c, _) in from.iter() {
            if c >= min && filter(c) {
                out.push(Choice::Card(c));
            }
        }
        return;
    }
    // Cards at or above `c` that pass the filter: the total less those below `c`.
    let mut total = 0u32;
    for (c, n) in from.iter() {
        if filter(c) {
            total += n as u32;
        }
    }
    let mut below = 0u32;
    for (c, n) in from.iter() {
        if filter(c) {
            if c >= min && total - below >= need {
                out.push(Choice::Card(c));
            }
            below += n as u32;
        }
    }
}
