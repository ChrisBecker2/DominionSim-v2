//! Game state. Everything is fixed-size and `Copy`: cloning a state is a memcpy,
//! which is what makes search and undo cheap and keeps the hot loop allocation-free.

use crate::cards::{self, id, CardId, NUM_CARDS};
use crate::counts::Counts;
use crate::engine::Pending;
use crate::rng::Rng;

pub const MAX_PLAYERS: usize = 6;
pub const KNOWN_CAP: usize = 120;
pub const STACK_CAP: usize = 48;

/// Cards on top of the deck whose identity is known, top = last element.
/// Beneath them sits `PlayerState::deck_unknown`, a multiset in unknown order.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct KnownStack {
    pub cards: [CardId; KNOWN_CAP],
    pub len: u8,
}

impl Default for KnownStack {
    fn default() -> Self {
        KnownStack { cards: [0; KNOWN_CAP], len: 0 }
    }
}

impl KnownStack {
    #[inline]
    pub fn push_top(&mut self, c: CardId) {
        assert!((self.len as usize) < KNOWN_CAP, "known deck stack overflow");
        self.cards[self.len as usize] = c;
        self.len += 1;
    }
    #[inline]
    pub fn pop_top(&mut self) -> Option<CardId> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.cards[self.len as usize])
    }
    #[inline]
    pub fn peek_top(&self) -> Option<CardId> {
        if self.len == 0 { None } else { Some(self.cards[self.len as usize - 1]) }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Top-first order.
    pub fn iter_top_down(&self) -> impl Iterator<Item = CardId> + '_ {
        self.cards[..self.len as usize].iter().rev().copied()
    }
    pub fn counts(&self) -> Counts {
        let mut c = Counts::EMPTY;
        for &x in &self.cards[..self.len as usize] {
            c.add(x, 1);
        }
        c
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PlayerState {
    pub hand: Counts,
    pub deck_known: KnownStack,
    pub deck_unknown: Counts,
    pub discard: Counts,
    pub in_play: Counts,
    /// Library's set-aside cards (discarded when Library finishes).
    pub set_aside: Counts,
    pub turns_taken: u16,
}

impl PlayerState {
    pub fn deck_size(&self) -> u32 {
        self.deck_known.len as u32 + self.deck_unknown.total()
    }
    /// Deck contents as a multiset (known + unknown).
    pub fn deck_counts(&self) -> Counts {
        let mut c = self.deck_known.counts();
        c.add_all(&self.deck_unknown);
        c
    }
    /// Every card the player owns, wherever it is.
    pub fn all_cards(&self) -> Counts {
        let mut c = self.deck_counts();
        c.add_all(&self.hand);
        c.add_all(&self.discard);
        c.add_all(&self.in_play);
        c.add_all(&self.set_aside);
        c
    }
    pub fn vp(&self) -> i32 {
        vp_of_cards(&self.all_cards())
    }
}

/// Victory points of a whole collection of cards (Gardens counts the collection's size).
pub fn vp_of_cards(all: &Counts) -> i32 {
    let total = all.total() as i32;
    all.iter()
        .map(|(c, n)| {
            let per = if c == id::GARDENS { total / 10 } else { cards::def(c).vp as i32 };
            per * n as i32
        })
        .sum()
}

/// Winners bitmask from final scores and turns taken: highest VP, ties broken by fewer turns;
/// remaining ties share the win.
pub fn winners_of(scores: &[i32], turns: &[u16]) -> u8 {
    let key = |p: usize| (scores[p], -(turns[p] as i32));
    let best = (0..scores.len()).map(key).max().unwrap();
    (0..scores.len()).filter(|&p| key(p) == best).fold(0u8, |m, p| m | 1 << p)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Initial 5-card draws for every player are on the stack.
    Setup,
    Action,
    Buy,
    /// Cleanup happened; the 5-card draw for next turn is on the stack.
    CleanupDraw,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TurnState {
    pub player: u8,
    pub phase: Phase,
    pub actions: u8,
    pub buys: u8,
    pub coins: u16,
    /// Number of Merchant plays this turn (each gives +$1 on the first Silver).
    pub merchants: u8,
    pub silvers_played: u8,
    /// Global turn counter (1-based, counts every player's turn).
    pub number: u16,
    /// Whether the turn's start has been announced (TurnStart/PhaseStart events). Deferred to
    /// the first step of the turn so a turn-boundary pause logs nothing of the next turn.
    pub announced: bool,
}

impl TurnState {
    pub fn start(player: u8, number: u16) -> Self {
        TurnState { player, phase: Phase::Action, actions: 1, buys: 1, coins: 0, merchants: 0, silvers_played: 0, number, announced: false }
    }
}

/// A zone a selection picks cards from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    Hand,
    Discard,
    /// Cards revealed / looked at / set aside (the player's `set_aside` zone).
    Revealed,
}

/// What happens to a picked card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    Discard,
    Trash,
    /// Put onto the draw pile (it becomes the new top card).
    Topdeck,
    /// Play it (e.g. Throne Room target, Vassal's discarded action).
    Play,
    SetAside,
}

/// Which cards are eligible for a selection or gain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Filter {
    Any,
    Action,
    Treasure,
    Victory,
    Card(CardId),
    NonCopperTreasure,
}

impl Filter {
    #[inline]
    pub fn matches(self, c: CardId) -> bool {
        match self {
            Filter::Any => true,
            Filter::Action => cards::is(c, cards::ACTION),
            Filter::Treasure => cards::is(c, cards::TREASURE),
            Filter::Victory => cards::is(c, cards::VICTORY),
            Filter::Card(x) => c == x,
            Filter::NonCopperTreasure => c != id::COPPER && cards::is(c, cards::TREASURE),
        }
    }
}

/// Where a gained card goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dest {
    Discard,
    Hand,
    DeckTop,
}

/// Continuation run when a `Select` frame finishes. `count` = cards picked, `last` = last pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Then {
    Nothing,
    /// Draw one card per pick (Cellar).
    DrawPerPick,
    /// +$n per pick (Moneylender).
    CoinsPerPick(u8),
    /// If something was picked, gain a card costing up to cost(last) + plus (Remodel, Mine).
    GainUpTo { plus: u8, filter: Filter, dest: Dest },
    /// Resolve the picked card's effects `times` times (Throne Room).
    PlayPicked { times: u8 },
    /// Discard whatever is left in the Revealed zone (Bandit).
    DiscardRevealed,
}

/// A pending piece of work on the effect stack. Card effects that need input or span
/// several steps are built from these few generic frames, so the engine can stop and
/// resume at any decision or chance point, and new cards mostly compose existing frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    pub kind: FrameKind,
    /// The player this frame acts on (the victim for attacks).
    pub player: u8,
    /// The card whose effect created this frame (0 with `FrameKind::Draw` from cleanup).
    pub source: CardId,
    pub zone: Zone,
    pub act: Act,
    pub filter: Filter,
    pub dest: Dest,
    /// Select: minimum/maximum picks. Draw/RevealTop: cards remaining. Gain: max cost.
    pub min: u8,
    pub max: u8,
    /// Select: picks made so far. Library: 1 while `subject` awaits a decision.
    pub count: u8,
    /// Select: last picked card (also the lower bound for canonical ordering).
    pub last: CardId,
    /// Select: picks are order-sensitive (topdecking several cards), so no canonical ordering.
    pub ordered: bool,
    pub then: Then,
    /// The card a YesNo decision is about / the card to PlayEffects.
    pub subject: CardId,
}

impl Frame {
    pub fn new(kind: FrameKind, player: u8, source: CardId) -> Self {
        Frame {
            kind,
            player,
            source,
            zone: Zone::Hand,
            act: Act::Discard,
            filter: Filter::Any,
            dest: Dest::Discard,
            min: 0,
            max: 0,
            count: 0,
            last: 0,
            ordered: false,
            then: Then::Nothing,
            subject: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameKind {
    /// Draw `max` cards for `player`.
    Draw,
    /// Resolve the on-play effects of `subject` (already in play). Used by Throne Room / Vassal.
    PlayEffects,
    /// Gain a card costing up to `max` matching `filter` to `dest`. Mandatory if any is legal.
    Gain,
    /// Pick cards one at a time from `zone` matching `filter` and apply `act`; `min..=max` picks
    /// (clamped to what's available), then run `then`.
    Select,
    /// Reveal the top `max` cards of the deck into the Revealed zone (set_aside).
    RevealTop,
    /// Yes/no: apply `act` to `subject` located in `zone`.
    YesNo,
    /// Vassal: discard the top card of the deck; if it's an Action, ask to play it.
    Vassal,
    /// Library: draw to 7, may set aside Actions (`count` = 1 while `subject` awaits the decision).
    Library,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameStack {
    pub frames: [Frame; STACK_CAP],
    pub len: u8,
}

impl Default for FrameStack {
    fn default() -> Self {
        FrameStack { frames: [Frame::new(FrameKind::Draw, 0, 0); STACK_CAP], len: 0 }
    }
}

impl FrameStack {
    #[inline]
    pub fn push(&mut self, f: Frame) {
        assert!((self.len as usize) < STACK_CAP, "effect stack overflow");
        self.frames[self.len as usize] = f;
        self.len += 1;
    }
    #[inline]
    pub fn pop(&mut self) -> Option<Frame> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.frames[self.len as usize])
    }
    #[inline]
    pub fn top(&self) -> Option<Frame> {
        if self.len == 0 { None } else { Some(self.frames[self.len as usize - 1]) }
    }
    #[inline]
    pub fn set_top(&mut self, f: Frame) {
        let i = self.len as usize - 1;
        self.frames[i] = f;
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn iter(&self) -> impl Iterator<Item = &Frame> {
        self.frames[..self.len as usize].iter()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    ProvincesGone,
    /// 3 supply piles empty (4 with 5+ players); see `GameState::empty_piles`.
    PilesEmpty,
    /// The simulation turn cap (`max_turns`) was reached.
    TurnLimit,
}

#[derive(Clone, Debug)]
pub struct GameConfig {
    pub num_players: usize,
    pub kingdom: Vec<CardId>,
    pub seed: u64,
    /// Game ends (as if piles ran out) once this many total turns have been taken.
    pub max_turns: u16,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig { num_players: 2, kingdom: cards::FIRST_GAME.to_vec(), seed: 0, max_turns: 200 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameState {
    pub num_players: u8,
    pub players: [PlayerState; MAX_PLAYERS],
    pub supply: Counts,
    /// Bitmask of card ids whose piles are part of this game's supply.
    pub in_supply: u64,
    pub trash: Counts,
    pub turn: TurnState,
    pub stack: FrameStack,
    pub rng: Rng,
    /// When true, draws from an unknown deck stop with `Step::Chance` instead of sampling.
    pub chance_mode: bool,
    /// When true, decisions with exactly one legal choice are applied automatically.
    pub auto_single: bool,
    pub max_turns: u16,
    /// When true, `advance` stops with `Step::TurnStart` each time a new turn begins (for
    /// stepping through a game turn by turn). Simulation and search leave it off.
    pub pause_at_turn_start: bool,
    pub pending: Pending,
}

pub fn victory_pile_size(n: usize) -> u8 {
    if n <= 2 { 8 } else { 12 }
}

pub fn province_pile_size(n: usize) -> u8 {
    match n {
        0..=2 => 8,
        3 | 4 => 12,
        5 => 15,
        _ => 18,
    }
}

impl GameState {
    /// Standard setup: supply sized per player count, 7 Copper + 3 Estate each,
    /// initial 5-card draws pending on the stack (resolved by the first `advance`).
    pub fn new(cfg: &GameConfig) -> Self {
        let n = cfg.num_players;
        assert!((2..=MAX_PLAYERS).contains(&n), "player count must be 2..={MAX_PLAYERS}");
        let mut supply = Counts::EMPTY;
        let mut in_supply = 0u64;
        let mut put = |c: CardId, k: u8| {
            supply.set(c, k);
            in_supply |= 1 << c;
        };
        put(id::COPPER, 60 - 7 * n as u8);
        put(id::SILVER, 40);
        put(id::GOLD, 30);
        put(id::ESTATE, victory_pile_size(n));
        put(id::DUCHY, victory_pile_size(n));
        put(id::PROVINCE, province_pile_size(n));
        put(id::CURSE, 10 * (n as u8 - 1));
        for &k in &cfg.kingdom {
            assert!(k >= cards::FIRST_KINGDOM, "{} is not a kingdom card", cards::name(k));
            put(k, if cards::is(k, cards::VICTORY) { victory_pile_size(n) } else { 10 });
        }
        let mut s = GameState {
            num_players: n as u8,
            players: [PlayerState::default(); MAX_PLAYERS],
            supply,
            in_supply,
            trash: Counts::EMPTY,
            turn: TurnState { phase: Phase::Setup, ..TurnState::start(0, 1) },
            stack: FrameStack::default(),
            rng: Rng::new(cfg.seed),
            chance_mode: false,
            auto_single: true,
            max_turns: cfg.max_turns,
            pause_at_turn_start: false,
            pending: Pending::None,
        };
        for p in 0..n {
            s.players[p].deck_unknown.set(id::COPPER, 7);
            s.players[p].deck_unknown.set(id::ESTATE, 3);
        }
        // Push in reverse so player 0 draws first.
        for p in (0..n).rev() {
            s.stack.push(Frame { max: 5, ..Frame::new(FrameKind::Draw, p as u8, 0) });
        }
        s
    }

    #[inline]
    pub fn in_supply(&self, c: CardId) -> bool {
        self.in_supply & (1 << c) != 0
    }

    pub fn empty_piles(&self) -> u32 {
        (0..NUM_CARDS as CardId).filter(|&c| self.in_supply(c) && self.supply.get(c) == 0).count() as u32
    }

    pub fn is_game_over(&self) -> bool {
        self.turn.phase == Phase::GameOver
    }

    /// End condition checked at end of each turn.
    pub fn end_condition_met(&self) -> bool {
        self.end_reason().is_some()
    }

    /// Why the game ends (or would end if checked now), if it does.
    pub fn end_reason(&self) -> Option<EndReason> {
        let pile_limit = if self.num_players >= 5 { 4 } else { 3 };
        if self.in_supply(id::PROVINCE) && self.supply.get(id::PROVINCE) == 0 {
            Some(EndReason::ProvincesGone)
        } else if self.empty_piles() >= pile_limit {
            Some(EndReason::PilesEmpty)
        } else if self.turn.number >= self.max_turns {
            Some(EndReason::TurnLimit)
        } else {
            None
        }
    }

    pub fn current(&self) -> usize {
        self.turn.player as usize
    }

    /// Other players in turn order starting left of `p`.
    pub fn others(&self, p: u8) -> impl Iterator<Item = u8> {
        let n = self.num_players;
        (1..n).map(move |i| (p + i) % n)
    }

    pub fn scores(&self) -> [i32; MAX_PLAYERS] {
        let mut s = [0; MAX_PLAYERS];
        for p in 0..self.num_players as usize {
            s[p] = self.players[p].vp();
        }
        s
    }

    /// Winners bitmask: highest VP, ties broken by fewer turns; remaining ties share the win.
    pub fn winners(&self) -> u8 {
        let n = self.num_players as usize;
        let scores = self.scores();
        let mut turns = [0u16; MAX_PLAYERS];
        for p in 0..n {
            turns[p] = self.players[p].turns_taken;
        }
        winners_of(&scores[..n], &turns[..n])
    }

    /// If the game would end when the current turn ends (Provinces or piles; not the turn cap),
    /// the winners bitmask it would end with, counting the current player's turn as taken.
    pub fn result_if_turn_ends(&self) -> Option<u8> {
        match self.end_reason() {
            Some(EndReason::ProvincesGone) | Some(EndReason::PilesEmpty) => {}
            _ => return None,
        }
        let n = self.num_players as usize;
        let scores = self.scores();
        let mut turns = [0u16; MAX_PLAYERS];
        for p in 0..n {
            turns[p] = self.players[p].turns_taken + u16::from(p == self.turn.player as usize && self.turn.phase != Phase::GameOver);
        }
        Some(winners_of(&scores[..n], &turns[..n]))
    }
}
