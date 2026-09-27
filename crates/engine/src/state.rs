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
        let all = self.all_cards();
        let total = all.total() as i32;
        all.iter()
            .map(|(c, n)| {
                let per = if c == id::GARDENS { total / 10 } else { cards::def(c).vp as i32 };
                per * n as i32
            })
            .sum()
    }
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
}

impl TurnState {
    pub fn start(player: u8, number: u16) -> Self {
        TurnState { player, phase: Phase::Action, actions: 1, buys: 1, coins: 0, merchants: 0, silvers_played: 0, number }
    }
}

/// A pending piece of work on the effect stack. Card effects that need input or
/// that span several steps are expressed as frames so the engine can stop and
/// resume at any decision or chance point. Field meaning depends on `kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    pub kind: FrameKind,
    /// The player this frame acts on (the victim for attacks).
    pub player: u8,
    pub card: CardId,
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
}

impl Frame {
    pub fn new(kind: FrameKind, player: u8) -> Self {
        Frame { kind, player, card: 0, a: 0, b: 0, c: 0, d: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameKind {
    /// Draw `a` cards for `player`.
    Draw,
    /// Resolve the on-play effects of `card` (card is already in play). Used by Throne Room / Vassal.
    PlayEffects,
    /// Gain a card costing up to `a`; `b` = GainFilter, `c` = GainDest. Mandatory if any legal.
    Gain,
    /// Cellar: discard any number (a = discarded so far, b = min card id allowed next), then draw a.
    Cellar,
    /// Chapel: trash up to 4 (a = trashed so far, b = min id).
    Chapel,
    /// Harbinger: may put a card from discard onto deck.
    Harbinger,
    /// Vassal: discard top card of deck; if action, may play it.
    Vassal,
    /// Vassal follow-up: `card` is the discarded action; YesNo play it.
    VassalPlay,
    /// Bureaucrat victim: topdeck a Victory card from hand.
    BureaucratVictim,
    /// Militia victim: discard down to 3 (b = min id).
    MilitiaVictim,
    /// Moneylender: may trash a Copper for +$3.
    Moneylender,
    /// Poacher: discard `a` more cards (b = min id).
    Poacher,
    /// Remodel: trash a card from hand, then Gain up to cost+2.
    Remodel,
    /// Throne Room: may choose an action in hand to play twice.
    ThroneRoom,
    /// Bandit victim: reveal top 2 (stored in c/d, a = revealed count), trash a non-Copper treasure.
    BanditVictim,
    /// Library: draw to 7, may set aside actions. `a`=1 while `card` awaits the set-aside decision.
    Library,
    /// Mine: may trash a treasure from hand, gain a treasure costing up to +3 to hand.
    Mine,
    /// Sentry: look at top 2 (stored in c/d, a = count, b = stage).
    Sentry,
    /// Artisan follow-up: put a card from hand onto the deck.
    ArtisanTopdeck,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameStack {
    pub frames: [Frame; STACK_CAP],
    pub len: u8,
}

impl Default for FrameStack {
    fn default() -> Self {
        FrameStack { frames: [Frame::new(FrameKind::Draw, 0); STACK_CAP], len: 0 }
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
            pending: Pending::None,
        };
        for p in 0..n {
            s.players[p].deck_unknown.set(id::COPPER, 7);
            s.players[p].deck_unknown.set(id::ESTATE, 3);
        }
        // Push in reverse so player 0 draws first.
        for p in (0..n).rev() {
            s.stack.push(Frame { a: 5, ..Frame::new(FrameKind::Draw, p as u8) });
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
        let pile_limit = if self.num_players >= 5 { 4 } else { 3 };
        (self.in_supply(id::PROVINCE) && self.supply.get(id::PROVINCE) == 0)
            || self.empty_piles() >= pile_limit
            || self.turn.number >= self.max_turns
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
        let scores = self.scores();
        let n = self.num_players as usize;
        let best = (0..n).map(|p| (scores[p], -(self.players[p].turns_taken as i32))).max().unwrap();
        let mut mask = 0u8;
        for p in 0..n {
            if (scores[p], -(self.players[p].turns_taken as i32)) == best {
                mask |= 1 << p;
            }
        }
        mask
    }
}
