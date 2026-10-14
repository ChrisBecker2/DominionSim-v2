//! Game state. Everything is fixed-size and `Copy`: cloning a state is a memcpy,
//! which is what makes search and undo cheap and keeps the hot loop allocation-free.

use crate::cards::{self, id, CardId};
use crate::counts::Counts;
use crate::engine::Pending;
use crate::rng::Rng;

pub const MAX_PLAYERS: usize = 6;
pub const KNOWN_CAP: usize = 120;
/// Capacity of the known cards at the very bottom of a deck (`PlayerState::deck_known_bottom`, Secret Passage). They are drawn once the
/// rest of the deck is gone, i.e. within one pass through it, so a handful is the most there ever are; a stack of `KNOWN_CAP` per player
/// was 240 bytes on each of the six seats for a zone that is empty in nearly every game.
pub const BOTTOM_CAP: usize = 32;
pub const STACK_CAP: usize = 48;
/// Max simultaneous pending start-of-next-turn Duration effects for one player. Each play of a
/// Duration card (or a Throne Room/King's Court resolution of one) needs at most one slot; same-
/// card argless plays merge into one entry (see `GameState::push_duration_pending`), so this is
/// generous for realistic turns (a handful of distinct Duration cards, plus Haven/Blockade/Sailor
/// copies, which don't merge since each carries its own argument).
pub const PENDING_DURATIONS_CAP: usize = 12;
/// Max distinct Action cards the player's Princes hold at once (`PlayerState::prince_cards`).
pub const PRINCE_CAP: usize = 8;
/// Max distinct cards a player's Farmhands hold set aside at once (`PlayerState::farm_cards`).
pub const FARM_CAP: usize = 8;
/// Max distinct cards a player's Summons hold set aside at once (`PlayerState::summon_cards`).
pub const SUMMON_CAP: usize = 3;
/// Max batches of cards Churches hold at once (`PlayerState::church`): one per Church play still waiting for the next turn.
pub const CHURCH_CAP: usize = 4;
/// Max start-of-turn effects pending at once (`TurnState::start_pending`): every pending
/// Duration entry, one per Prince'd card, one per Farmhands-held card, and Clerk (one merged entry).
pub const START_PENDING_CAP: usize = PENDING_DURATIONS_CAP + PRINCE_CAP + FARM_CAP + 1 + ASIDE_CAP;
/// Max distinct cards `DurationHeld` tracks in one turn (see its doc comment): generous for a
/// realistic turn (a handful of distinct Duration cards, plus any Throne Room/King's Court that
/// multiplied one).
pub const DURATION_HELD_CAP: usize = 8;
/// Max distinct cards on one player's Tavern mat (Adventures): the nine Reserve cards and Coppers (Miser).
pub const TAVERN_CAP: usize = 10;
/// Max distinct cards that stay in play for the rest of the game (`PlayerState::stays`).
pub const STAYS_CAP: usize = 8;
/// Max distinct cards a player's Archives hold set aside at once (`PlayerState::archive`): three per Archive played.
pub const ARCHIVE_CAP: usize = 12;
/// Max Supply piles that carry VP or Debt tokens at once (`EmpState::piles`): every pile of a game with Tax, plus the Gathering piles.
pub const PILE_TOKEN_CAP: usize = 32;
/// `EmpState::flags`: which Empires effects are live in this game, so that other games skip each check with one byte test.
/// Some Landmark or token watches gains (Aqueduct, Basilica, Battlefield, Colonnade, Defiled Shrine, Labyrinth, Mountain Pass, a pile with VP or Debt).
pub const E_GAIN_WATCH: u8 = 1;
/// An Enchantress has been played: the first Action each affected player plays is a cantrip.
pub const E_ENCHANT: u8 = 2;
/// Change is in the game: Debt can be worth keeping (the end-of-turn payment is not automatic, the Debt payment is searched).
pub const E_KEEP_DEBT: u8 = 4;
/// Tax is in the game: gains in the Buy phase take the Debt on their pile.
pub const E_TAX: u8 = 8;
/// A Castle pile is in the game.
pub const E_CASTLES: u8 = 32;
/// Tomb is in the game: +1 VP token for every card trashed.
pub const E_TOMB: u8 = 64;
/// `PlayerState::emp` flags.
/// Donate: at the start of this player's next turn they put their deck and discard pile into the hand, trash, shuffle and draw 5.
pub const P_DONATE: u8 = 1;

/// `NocState::flags`: which Nocturne effects are live in this game, so that other games skip each check with one byte test.
/// A Night card can be in a hand: the Buy phase is followed by a Night phase when the player holds one.
pub const N_NIGHT: u8 = 1;
/// Boons are in the game (a Fate card is in the kingdom).
pub const N_BOONS: u8 = 2;
/// Hexes are in the game (a Doom card is in the kingdom).
pub const N_HEXES: u8 = 4;
/// Faithful Hound is in the game: a discarded Hound may be set aside.
pub const N_HOUND: u8 = 8;
/// Cards gained are counted per turn (Monastery, Devil's Workshop) and some are gained to the hand (Guardian, Ghost Town, Night Watchman, Den of Sin).
pub const N_GAIN_HOOK: u8 = 16;
/// `PlayerState::noc` flags: the States in front of a player. Deluded and Envious are the two sides of one card (a player has at most one).
pub const P_DELUDED: u8 = 1;
pub const P_ENVIOUS: u8 = 2;
/// Miserable (-2 VP); flipped over it is Twice Miserable (-4 VP): both bits.
pub const P_MISERABLE: u8 = 4;
pub const P_TWICE_MISERABLE: u8 = 8;
/// Guardian: until the start of the player's next turn, other players' Attacks do not affect them.
pub const P_GUARDED: u8 = 16;
/// The piles outside the Supply of Nocturne, in `NocState::piles` order: Will-o'-Wisp, Wish, Bat, Imp, Ghost.
pub const NOC_WISP: usize = 0;
pub const NOC_WISH: usize = 1;
pub const NOC_BAT: usize = 2;
pub const NOC_IMP: usize = 3;
pub const NOC_GHOST: usize = 4;
/// Chance "players" for the Boon and Hex draws: `Step::Chance { player: BOON_CHANCE }` asks which Boon is drawn (the outcomes are lanes
/// `0..12` of `chance_outcomes`, one count each for every Boon in the deck, so the search treats them like any other draw).
pub const BOON_CHANCE: u8 = 254;
pub const HEX_CHANCE: u8 = 255;

/// `RsState::flags`: which Rising Sun effects are live in this game, so that other games skip each check with one byte test.
/// A Shadow card is in the kingdom: the Shadows in a deck can be played from it, and are put at the bottom when shuffled.
pub const R_SHADOW: u8 = 1;
/// Kintsugi is in the game: gaining a Gold is recorded (`P_RS_GOLD`).
pub const R_KINTSUGI: u8 = 2;
/// Receive Tribute is in the game: the cards gained are counted per turn (`RsTurn::gains`).
pub const R_TRIBUTE: u8 = 4;
/// `PlayerState::rs` flags. The player has gained a Gold this game (Kintsugi).
pub const P_RS_GOLD: u8 = 1;
/// Max distinct cards a player's Prophecy holds set aside (`PlayerState::aside`): the hand Biding Time sets aside (any card beyond this stays in the
/// hand and is discarded as usual), or the cards Rapid Expansion holds for the next turn (a card gained beyond this is not set aside).
pub const ASIDE_CAP: usize = 16;
/// Max distinct cards Foresight sets aside in one turn (`PlayerState::foresight`).
pub const FORESIGHT_CAP: usize = 4;

/// `PlayerState::tok` indices: the pile each of a player's pile tokens is on (Adventures).
pub const TOK_CARD: usize = 0;
pub const TOK_ACTION: usize = 1;
pub const TOK_BUY: usize = 2;
pub const TOK_COIN: usize = 3;
pub const TOK_COST: usize = 4;
pub const TOK_TRASH: usize = 5;
/// The number of tokens that are placed on a Supply pile.
pub const NUM_PILE_TOKENS: usize = 6;
/// `PlayerState::adv` flags.
/// The -1 Card token is on the player's deck.
pub const ADV_MINUS_CARD: u8 = 1;
/// The -$1 token is in front of the player.
pub const ADV_MINUS_COIN: u8 = 2;
/// The Journey token is face down (it starts face up).
pub const ADV_JOURNEY_DOWN: u8 = 4;
/// `GameState::adv` flags: which token effects are live in this game, so that games without them skip every check with one byte test.
/// Some player's +1 Card / +1 Action / +1 Buy / +$1 token is on a pile.
pub const G_PLAY_TOKENS: u8 = 1;
/// Some player's -$2 cost token is on a pile.
pub const G_COST_TOKEN: u8 = 2;
/// Some player's Trashing token is on a pile.
pub const G_TRASH_TOKEN: u8 = 4;
/// A Champion has been played: Action plays get +1 Action per Champion their player has in play.
pub const G_CHAMPION: u8 = 8;
/// Some card in the game does something when it is discarded other than in Clean-up (Faithful Hound, Tunnel, Trail, Weaver): discards are watched.
pub const G_DISCARD_HOOK: u8 = 16;

/// The card ids that have a pile in the Supply: a `u128` for ids below 128, another for ids 128-255, a third for ids 256-383 and a fourth for ids
/// 384 and up (Cornucopia & Guilds took the id count past 128, Dark Ages past 192, Adventures past 256, Hinterlands past 352, Rising Sun past 384).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct SupplyMask {
    pub lo: u128,
    pub hi: u128,
    /// Ids 256-383 (Wild Hunt, the split pile bottoms, the Castles and most of the Nocturne, Hinterlands and Rising Sun cards are in the Supply there).
    pub top: u128,
    /// Ids 384 and up (the end of the Rising Sun cards' range, the members of the mixed piles).
    pub ext: u128,
}

/// Iterator over the card ids of a `SupplyMask`, in id order.
pub struct SupplyCards {
    lo: u128,
    hi: u128,
    top: u128,
    ext: u128,
}

impl SupplyCards {
    /// The cards of a mask given as its four words (see `SupplyMask`).
    #[inline(always)]
    pub fn from_words(lo: u128, hi: u128, top: u128, ext: u128) -> SupplyCards {
        SupplyCards { lo, hi, top, ext }
    }
}

impl Iterator for SupplyCards {
    type Item = CardId;
    #[inline]
    fn next(&mut self) -> Option<CardId> {
        if self.lo != 0 {
            let c = self.lo.trailing_zeros() as CardId;
            self.lo &= self.lo - 1;
            Some(c)
        } else if self.hi != 0 {
            let c = 128 + self.hi.trailing_zeros() as CardId;
            self.hi &= self.hi - 1;
            Some(c)
        } else if self.top != 0 {
            let c = 256 + self.top.trailing_zeros() as CardId;
            self.top &= self.top - 1;
            Some(c)
        } else if self.ext != 0 {
            let c = 384 + self.ext.trailing_zeros() as CardId;
            self.ext &= self.ext - 1;
            Some(c)
        } else {
            None
        }
    }
}

impl SupplyMask {
    #[inline(always)]
    pub fn has(&self, c: CardId) -> bool {
        if c < 128 {
            self.lo & (1u128 << c) != 0
        } else if c < 256 {
            self.hi & (1u128 << (c - 128)) != 0
        } else if c < 384 {
            self.top & (1u128 << (c - 256)) != 0
        } else {
            self.ext & (1u128 << (c - 384)) != 0
        }
    }
    #[inline]
    pub fn insert(&mut self, c: CardId) {
        if c < 128 {
            self.lo |= 1u128 << c;
        } else if c < 256 {
            self.hi |= 1u128 << (c - 128);
        } else if c < 384 {
            self.top |= 1u128 << (c - 256);
        } else {
            self.ext |= 1u128 << (c - 384);
        }
    }
    /// Takes the pile of `c` out of the mask.
    #[inline]
    pub fn remove(&mut self, c: CardId) {
        if c < 128 {
            self.lo &= !(1u128 << c);
        } else if c < 256 {
            self.hi &= !(1u128 << (c - 128));
        } else if c < 384 {
            self.top &= !(1u128 << (c - 256));
        } else {
            self.ext &= !(1u128 << (c - 384));
        }
    }
    /// Whether any card of the mask words is in the Supply (the gain-trigger gates). No card with a gain trigger has an id from 384.
    #[inline(always)]
    pub fn any(&self, lo: u128, hi: u128, top: u128) -> bool {
        (self.lo & lo) != 0 || (self.hi & hi) != 0 || (self.top & top) != 0
    }
}

/// A compact multiset of (card, count) pairs for `TurnState::duration_held`: a plain `Counts`
/// would work (and is what the very first version of this used) but costs 128 bytes on `TurnState`
/// — cheap in isolation (`TurnState` is one copy, not one per player), but `GameState` is copied
/// constantly in the hot loop and search, so every byte on it is felt. At most a handful of
/// distinct cards are ever held in one turn, so a small fixed list is both correct and far
/// cheaper (16 bytes of data). `PartialEq`/`Eq`/`Hash` are hand-written (not derived) to treat
/// this as an order-independent multiset, like `Counts`: entries can land in a different array
/// order depending on how they were built (e.g. insertion order during play vs. card-id order
/// after a text-format round trip), even when the held cards are the same.
#[derive(Clone, Copy, Debug)]
pub struct DurationHeld {
    entries: [(CardId, u8); DURATION_HELD_CAP],
    len: u8,
}

impl PartialEq for DurationHeld {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().all(|(c, n)| other.get(c) == n)
    }
}
impl Eq for DurationHeld {}
impl std::hash::Hash for DurationHeld {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Order-independent: XOR each entry's own hash together, then hash the (order-free) sum.
        let mut acc: u64 = 0;
        for (c, n) in self.iter() {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (c, n).hash(&mut h);
            acc ^= std::hash::Hasher::finish(&h);
        }
        acc.hash(state);
    }
}

impl Default for DurationHeld {
    fn default() -> Self {
        DurationHeld { entries: [(0, 0); DURATION_HELD_CAP], len: 0 }
    }
}

impl DurationHeld {
    pub const EMPTY: DurationHeld = DurationHeld { entries: [(0, 0); DURATION_HELD_CAP], len: 0 };

    /// Add `n` more held copies of `card` (merging into an existing entry for it, if any).
    pub fn add(&mut self, card: CardId, n: u8) {
        for i in 0..self.len as usize {
            if self.entries[i].0 == card {
                self.entries[i].1 = self.entries[i].1.saturating_add(n);
                return;
            }
        }
        let i = self.len as usize;
        assert!(i < DURATION_HELD_CAP, "DurationHeld overflow: more than {DURATION_HELD_CAP} distinct cards held in one turn");
        self.entries[i] = (card, n);
        self.len += 1;
    }
    pub fn get(&self, card: CardId) -> u8 {
        self.entries[..self.len as usize].iter().find(|e| e.0 == card).map_or(0, |e| e.1)
    }
    #[inline]
    pub fn has(&self, card: CardId) -> bool {
        self.get(card) > 0
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn iter(&self) -> impl Iterator<Item = (CardId, u8)> + '_ {
        self.entries[..self.len as usize].iter().copied()
    }
}

/// Max distinct card ids `SmallMultiset<MAT_CAP>` (the Native Village and Island mats) ever
/// needs at once: bounded by the number of distinct card types this game's supply can ever
/// contain (at most 7 basics + a kingdom + Platinum/Colony, comfortably under this for any
/// realistic game — see `SmallMultiset`'s doc comment), with headroom.
pub const MAT_CAP: usize = 20;
/// Max distinct card ids `SmallMultiset<GAIN_RECORD_CAP>` (the per-player Smugglers gain
/// record) ever needs at once: a handful of distinct cards gained in one turn, the same
/// "realistic turn" reasoning as `DURATION_HELD_CAP`.
pub const GAIN_RECORD_CAP: usize = 8;
/// Max distinct cards `TurnState::named_for_war_chest` ever holds: one per War Chest play in a turn, so far more than any turn has.
pub const WAR_CHEST_CAP: usize = 24;

/// A compact, order-independent multiset of (card, count) pairs capped at `N` distinct card
/// ids: the same trick as `DurationHeld` below (see its doc comment for the full rationale),
/// generalized over the cap so one definition serves every use at its own size instead of
/// paying for a full 128-lane, 128-byte `Counts` per instance. Total *copies* of one card are
/// unbounded (`u8` count, saturating); only the number of distinct card ids is capped. Used for
/// the Native Village and Island mats (`MAT_CAP`) and each player's per-turn gain record for
/// Smugglers (`GAIN_RECORD_CAP`).
#[derive(Clone, Copy, Debug)]
pub struct SmallMultiset<const N: usize> {
    // Two arrays rather than one of pairs: a (u16, u8) pair is four bytes with its padding, this is three.
    ids: [CardId; N],
    counts: [u8; N],
    len: u8,
}

impl<const N: usize> Default for SmallMultiset<N> {
    fn default() -> Self {
        SmallMultiset::EMPTY
    }
}

impl<const N: usize> PartialEq for SmallMultiset<N> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().all(|(c, n)| other.get(c) == n)
    }
}
impl<const N: usize> Eq for SmallMultiset<N> {}
impl<const N: usize> std::hash::Hash for SmallMultiset<N> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Order-independent: XOR each entry's own hash together, then hash the (order-free) sum.
        let mut acc: u64 = 0;
        for (c, n) in self.iter() {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (c, n).hash(&mut h);
            acc ^= std::hash::Hasher::finish(&h);
        }
        acc.hash(state);
    }
}

impl<const N: usize> SmallMultiset<N> {
    pub const EMPTY: Self = SmallMultiset { ids: [0; N], counts: [0; N], len: 0 };

    /// Add `n` more copies of `card` (merging into an existing entry for it, if any).
    pub fn add(&mut self, card: CardId, n: u8) {
        for i in 0..self.len as usize {
            if self.ids[i] == card {
                self.counts[i] = self.counts[i].saturating_add(n);
                return;
            }
        }
        let i = self.len as usize;
        assert!(i < N, "SmallMultiset overflow: more than {N} distinct card ids");
        self.ids[i] = card;
        self.counts[i] = n;
        self.len += 1;
    }
    /// Like `add`, but a full set drops the new card instead of aborting (release builds abort on
    /// panic). For records where losing an entry is harmless (the Smugglers gain record: a turn
    /// that gains more distinct cards than the cap only narrows Smugglers' later choices).
    pub fn add_or_drop(&mut self, card: CardId, n: u8) {
        if (self.len as usize) < N || self.has(card) {
            self.add(card, n);
        }
    }
    pub fn get(&self, card: CardId) -> u8 {
        match self.ids[..self.len as usize].iter().position(|&c| c == card) {
            Some(i) => self.counts[i],
            None => 0,
        }
    }
    #[inline]
    pub fn has(&self, card: CardId) -> bool {
        self.get(card) > 0
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn iter(&self) -> impl Iterator<Item = (CardId, u8)> + '_ {
        let n = self.len as usize;
        self.ids[..n].iter().copied().zip(self.counts[..n].iter().copied())
    }
    pub fn total(&self) -> u32 {
        self.iter().map(|(_, n)| n as u32).sum()
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
    /// Remove one copy of `card`; false if there was none.
    pub fn remove(&mut self, card: CardId) -> bool {
        let Some(i) = self.ids[..self.len as usize].iter().position(|&c| c == card) else { return false };
        if self.counts[i] > 1 {
            self.counts[i] -= 1;
        } else {
            self.len -= 1;
            self.ids[i] = self.ids[self.len as usize];
            self.counts[i] = self.counts[self.len as usize];
        }
        true
    }
    /// Sum into a `Counts` (for `all_cards`/VP: the mats' cards count as owned).
    pub fn counts(&self) -> Counts {
        let mut c = Counts::EMPTY;
        for (card, n) in self.iter() {
            c.add(card, n);
        }
        c
    }
    /// Add every entry into `c`.
    #[inline]
    pub fn add_into(&self, c: &mut Counts) {
        for (card, n) in self.iter() {
            c.add(card, n);
        }
    }
    /// Take (and clear) every entry at once (Native Village: "put all the cards from your mat
    /// into your hand").
    pub fn take_all(&mut self) -> Self {
        let out = *self;
        self.clear();
        out
    }
}

/// Cards on top of the deck whose identity is known, top = last element.
/// Beneath them sits `PlayerState::deck_unknown`, a multiset in unknown order.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct KnownStack<const N: usize = KNOWN_CAP> {
    pub cards: [CardId; N],
    pub len: u8,
}

impl<const N: usize> Default for KnownStack<N> {
    fn default() -> Self {
        KnownStack { cards: [0; N], len: 0 }
    }
}

impl<const N: usize> KnownStack<N> {
    #[inline]
    pub fn push_top(&mut self, c: CardId) {
        assert!((self.len as usize) < N, "known deck stack overflow");
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
    /// Push/pop-order: index 0 was pushed first. Used for `deck_known_bottom` (Secret Passage),
    /// where `push_top` appends a newly-bottomed card beneath everything already bottomed, and
    /// `pop_front` removes the one nearest the unknown pile (the next to be drawn).
    pub fn iter_front_to_back(&self) -> impl Iterator<Item = CardId> + '_ {
        self.cards[..self.len as usize].iter().copied()
    }
    /// Remove and return the earliest-pushed card (index 0), shifting the rest down. Used only for
    /// `deck_known_bottom`, which is always short in practice (bounded by `BOTTOM_CAP`).
    pub fn pop_front(&mut self) -> Option<CardId> {
        if self.len == 0 {
            return None;
        }
        let c = self.cards[0];
        for i in 1..self.len as usize {
            self.cards[i - 1] = self.cards[i];
        }
        self.len -= 1;
        Some(c)
    }
    /// Insert `c` at depth `k` from the top (0 = the new top; `k` = the current length puts it
    /// just above whatever lies beneath the known section), shifting deeper cards down to make
    /// room. Used for Secret Passage's "below the Nth known card" placement.
    pub fn insert_from_top(&mut self, k: u8, c: CardId) {
        assert!((self.len as usize) < N, "known deck stack overflow");
        assert!(k <= self.len, "insert depth beyond the known section");
        let idx = self.len - k;
        let mut i = self.len;
        while i > idx {
            self.cards[i as usize] = self.cards[i as usize - 1];
            i -= 1;
        }
        self.cards[idx as usize] = c;
        self.len += 1;
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

/// One pending start-of-owner's-next-turn Duration effect (Seaside): see
/// `crates/engine/src/effects.rs`'s "Duration framework" section header comment for the full
/// scheduling/resolution design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PendingDuration {
    /// The Duration card whose effect this is (or, for Haven/Blockade, the card that pushed
    /// this entry). 0 = unused slot.
    pub card: CardId,
    /// How many times to resolve the effect at the start of the owner's next turn (Throne Room
    /// x2 / King's Court x3; same-card argless entries merge by summing this rather than adding
    /// new slots). 0 for a card with no next-turn effect of its own that's only in this list to
    /// stay in play (Outpost; a Throne Room/King's Court that multiplied a Duration play).
    pub times: u8,
    /// Haven: the card set aside from hand (in `set_aside`) to return to hand. Blockade: the
    /// card gained and set aside, whose copies curse other players who gain one on their turn.
    /// 0 = no argument.
    pub arg: CardId,
    /// Sailor only: how many of this entry's `times` plays have already used their "once this
    /// turn, when you gain a Duration card, you may play it" reaction (a Throne Room'd Sailor has
    /// two, a King's Court'd one three). Always 0 for every other card.
    pub used: u8,
}

/// The cards Church plays have set aside (`PlayerState::church`): one batch of up to three cards per play, oldest first. A batch is three
/// 10-bit fields (card id + 1, 0 = empty) in a `u32`, kept sorted so that equal batches are equal values. Each live batch has a Church entry in
/// the owner's pending Durations (or in the turn's start-of-turn list); the entries are resolved oldest first, taking the oldest batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ChurchBatches {
    batches: [u32; CHURCH_CAP],
    len: u8,
}

impl ChurchBatches {
    pub const EMPTY: ChurchBatches = ChurchBatches { batches: [0; CHURCH_CAP], len: 0 };

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// The number of batches (Church plays waiting).
    pub fn len(&self) -> usize {
        self.len as usize
    }
    fn decode(b: u32) -> ([CardId; 3], usize) {
        let mut out = [0; 3];
        let mut n = 0;
        for i in 0..3 {
            let v = (b >> (10 * i)) & 1023;
            if v != 0 {
                out[n] = (v - 1) as CardId;
                n += 1;
            }
        }
        (out, n)
    }
    fn encode(cards: &[CardId]) -> u32 {
        let mut v = [0u32; 3];
        for (i, &c) in cards.iter().enumerate() {
            v[i] = c as u32 + 1;
        }
        v.sort_unstable_by(|a, b| b.cmp(a));
        v[0] | (v[1] << 10) | (v[2] << 20)
    }
    /// Start a new, empty batch; false (nothing done) when all the slots are in use.
    pub fn begin(&mut self) -> bool {
        if (self.len as usize) < CHURCH_CAP {
            self.batches[self.len as usize] = 0;
            self.len += 1;
            true
        } else {
            false
        }
    }
    /// Add `card` to the newest batch (at most three cards fit; a fourth is refused).
    pub fn add(&mut self, card: CardId) -> bool {
        if self.len == 0 {
            return false;
        }
        let i = self.len as usize - 1;
        let (mut cards, n) = Self::decode(self.batches[i]);
        if n >= 3 {
            return false;
        }
        cards[n] = card;
        self.batches[i] = Self::encode(&cards[..n + 1]);
        true
    }
    /// Remove the oldest batch and return its cards.
    pub fn take_oldest(&mut self) -> ([CardId; 3], usize) {
        if self.len == 0 {
            return ([0; 3], 0);
        }
        let out = Self::decode(self.batches[0]);
        for i in 1..self.len as usize {
            self.batches[i - 1] = self.batches[i];
        }
        self.len -= 1;
        self.batches[self.len as usize] = 0;
        out
    }
    /// The cards of batch `i` (0 = oldest).
    pub fn batch(&self, i: usize) -> ([CardId; 3], usize) {
        Self::decode(self.batches[i])
    }
    /// Every card in every batch.
    pub fn add_into(&self, c: &mut Counts) {
        for i in 0..self.len as usize {
            let (cards, n) = Self::decode(self.batches[i]);
            for &card in &cards[..n] {
                c.add(card, 1);
            }
        }
    }
    /// The number of cards in all the batches.
    pub fn total(&self) -> usize {
        (0..self.len as usize).map(|i| Self::decode(self.batches[i]).1).sum()
    }
    /// The number of cards in each batch (oldest first, the unused slots 0): `determinize` deals fresh hidden cards of these sizes.
    pub fn sizes(&self) -> [u8; CHURCH_CAP] {
        let mut out = [0; CHURCH_CAP];
        for i in 0..self.len as usize {
            out[i] = Self::decode(self.batches[i]).1 as u8;
        }
        out
    }
}

/// The start-of-turn effects not yet resolved (`TurnState::start_pending`): a short fixed list.
/// `PartialEq`/`Hash` look only at the live prefix, so stale slots never matter and the search's
/// state hash (which walks `TurnState`) stays cheap.
#[derive(Clone, Copy, Debug)]
pub struct StartList {
    entries: [PendingDuration; START_PENDING_CAP],
    len: u8,
}

impl StartList {
    pub const EMPTY: StartList =
        StartList { entries: [PendingDuration { card: 0, times: 0, arg: 0, used: 0 }; START_PENDING_CAP], len: 0 };

    #[inline]
    pub fn as_slice(&self) -> &[PendingDuration] {
        &self.entries[..self.len as usize]
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [PendingDuration] {
        &mut self.entries[..self.len as usize]
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn push(&mut self, e: PendingDuration) {
        assert!((self.len as usize) < START_PENDING_CAP, "start-of-turn effect list overflow");
        self.entries[self.len as usize] = e;
        self.len += 1;
    }
    /// Remove entry `i`, keeping the order of the rest.
    pub fn remove(&mut self, i: usize) {
        let n = self.len as usize;
        for j in i..n - 1 {
            self.entries[j] = self.entries[j + 1];
        }
        self.entries[n - 1] = PendingDuration::default();
        self.len -= 1;
    }
}

impl Default for StartList {
    fn default() -> Self {
        StartList::EMPTY
    }
}
impl PartialEq for StartList {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for StartList {}
impl std::hash::Hash for StartList {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PlayerState {
    pub hand: Counts,
    pub deck_known: KnownStack,
    pub deck_unknown: Counts,
    /// Known cards under the unknown multiset, at the very bottom of the deck (Secret Passage).
    /// Drawn only once `deck_known` and `deck_unknown` are both exhausted.
    pub deck_known_bottom: KnownStack<BOTTOM_CAP>,
    pub discard: Counts,
    pub in_play: Counts,
    /// Library's set-aside cards (discarded when Library finishes).
    pub set_aside: Counts,
    /// Masquerade's holding zone: the card this player has committed to pass, held until every
    /// passing player has chosen and all the cards move at once. At most one card at a time.
    pub passed: SmallMultiset<2>,
    /// Cards held aside while an effect resolves, to enter play (or move on) later in it: Golem's
    /// second Action while the first resolves. Always empty between effects (not in the text
    /// format), counted as owned by `all_cards` so cards are conserved at every step.
    pub held: SmallMultiset<4>,
    pub turns_taken: u16,
    /// Victory point tokens (Monument, Bishop, ...), counted in the score.
    pub vp_tokens: u16,
    /// Pending start-of-my-next-turn Duration effects (Seaside): see `PendingDuration`.
    pub pending_durations: [PendingDuration; PENDING_DURATIONS_CAP],
    pub pending_durations_len: u8,
    /// Native Village's private mat (Seaside step 4): "put the top card of your deck face down
    /// on your Native Village mat" / "put all the cards from your mat into your hand". Private
    /// to its owner (`determinize` pools it for opponents, who only know its size); its cards
    /// count as owned (`all_cards`/VP).
    pub native_village_mat: SmallMultiset<MAT_CAP>,
    /// Island's public mat (Seaside step 4): "put this and a card from your hand onto your
    /// Island mat". Public (every player can see its exact contents); its cards count as owned
    /// (`all_cards`/VP). Island itself moves here straight from `in_play` and never returns.
    pub island_mat: SmallMultiset<MAT_CAP>,
    /// Cards this player gained on their own last completed turn (Seaside step 4: Smugglers,
    /// "a card the player to your right gained on their last turn"). Snapshotted from
    /// `TurnState::gained_this_turn` at cleanup; only maintained (see `GameState::gain`) while
    /// Smugglers is in this game's supply. Public information (all gains are public), so
    /// `determinize` doesn't touch it.
    pub last_turn_gains: SmallMultiset<GAIN_RECORD_CAP>,
    /// Cards that stayed in this player's play area past their own cleanup but are discarded at
    /// the *next* turn's cleanup, whoever's turn it is: a failed Outpost (played during an
    /// Outpost turn, so it can't grant a 3rd turn in a row) and any Throne Room / King's Court
    /// that played it ("If Outpost fails, you discard it during Clean-up of the next turn").
    pub discard_next_cleanup: DurationHeld,
    /// Players (bit `1 << player`) that this player's lingering Duration Attacks (Corsair,
    /// Blockade) don't affect, because they were immune (Moat revealed, Lighthouse in play) when
    /// the Attack was played: "attack immunity only works if it is in force as the Attack is
    /// played", however much later the effect would hit. Set when the Attack is played, cleared
    /// when it ends at the start of this player's next turn (`push_turn_start_frames`).
    pub shielded: u8,
    /// Prince (Promo): the Action cards set aside by this player's Princes (a Prince played with
    /// Throne Room sets aside two). Each is played, without entering play, at the start of every
    /// one of their turns after the one it was set aside on (`GameState::run_start_order`) and
    /// never leaves. Public; they count as owned (`all_cards`/VP).
    pub prince_cards: SmallMultiset<PRINCE_CAP>,
    /// Cards that stay in play for the rest of the game: the Princes (and Throne Rooms / King's Courts that played one) that hold
    /// cards, and Hireling and Champion with whatever multiplied them. Cleanup keeps this many of each in play. A Prince that set
    /// nothing aside is discarded as usual.
    pub stays: SmallMultiset<STAYS_CAP>,
    /// Coffers tokens (Cornucopia & Guilds): each can be removed during the player's turn for +$1
    /// (`Choice::Coffers` at the Buy decision; Butcher spends them inside its own gain).
    pub coffers: u16,
    /// Farmhands (Cornucopia & Guilds): the Actions and Treasures set aside when this player gained
    /// a Farmhands, each played at the start of their next turn (`GameState::run_start_order`). Face
    /// up (public); they count as owned.
    pub farm_cards: SmallMultiset<FARM_CAP>,
    /// Summon (Promo): the Action cards gained and set aside by this player's Summons, each played at the start of their next turn
    /// (`GameState::run_start_order`). Face up (public); they count as owned.
    pub summon_cards: SmallMultiset<SUMMON_CAP>,
    /// Church (Promo): the cards each Church play set aside face down, one batch per play still waiting for the start of the next turn
    /// (private to the owner; `determinize` pools them for the others). They count as owned.
    pub church: ChurchBatches,
    /// Cards set aside until Clean-up, when they are discarded (Joust's Province: not in the hand,
    /// not in play, and not in `set_aside`, which other effects empty). Counts as owned.
    pub cleanup_aside: SmallMultiset<2>,
    /// The Tavern mat (Adventures): Reserve cards played onto it and Coppers put there by Miser. Public; counts as owned.
    pub tavern: SmallMultiset<TAVERN_CAP>,
    /// The pile each of the player's pile tokens is on (`TOK_*`), 0 = not on a pile (Adventures). A pile is named by its Supply id
    /// (`cards::pile_of`).
    pub tok: [CardId; NUM_PILE_TOKENS],
    /// The card the Estate token is on (Inheritance), 0 = none: the player's Estates play it.
    pub inherit: CardId,
    /// Adventures flags (`ADV_*`): the -1 Card token on the deck, the -$1 token, the Journey token face down.
    pub adv: u8,
    /// The card Save set aside, returned to the hand after the Clean-up draw (0 = none). Counts as owned.
    pub saved: CardId,
    /// Bitmask of the Events this player has bought so far (bit `event - FIRST_EVENT`).
    pub events_bought: u64,
    /// Debt tokens (Empires): while there are any the player cannot buy cards or Events. Paid off with $ (`Choice::PayDebt`).
    pub debt: u16,
    /// The cards set aside under this player's Archives, face down (private to them; their owner draws one at the start of each of the next
    /// turns, `FrameKind`-less: a Duration entry per Archive says how many picks are left).
    pub archive: SmallMultiset<ARCHIVE_CAP>,
    /// Enchantresses (bit per seat) whose Attack affects this player: until that player's next turn, the first Action this player plays on each
    /// turn is +1 Card +1 Action instead.
    pub ench_by: u8,
    /// `P_*` flags.
    pub emp: u8,
    /// Nocturne: the States in front of the player and Guardian's protection (`P_DELUDED` ...).
    pub noc: u8,
    /// Nocturne: Faithful Hounds set aside until the end of the turn, when they go into the hand. Counted as owned.
    pub hounds: u8,
    /// Nocturne: the Boons (bit per Boon) Blessed Villages took, to be received at the start of this player's next turn. Not in the Boons deck meanwhile.
    pub boons_later: u16,
    /// Rising Sun, what the Prophecy in effect has set aside for this player (only one Prophecy is in a game, so one field serves): with Rapid Expansion
    /// the Actions and Treasures gained, each played at the start of the next turn (face up, public); with Biding Time the hand set aside at the start of
    /// Clean-up, face down (private to the owner; `determinize` pools it for the others), put into the hand at the start of the next turn. Counts as owned.
    pub aside: SmallMultiset<ASIDE_CAP>,
    /// Rising Sun, Foresight: the Actions set aside this turn, put into the hand after the Clean-up draw. Face up; counts as owned.
    pub foresight: SmallMultiset<FORESIGHT_CAP>,
    /// Rising Sun: `P_RS_*` flags.
    pub rs: u8,
}

impl PlayerState {
    pub fn deck_size(&self) -> u32 {
        self.deck_known.len as u32 + self.deck_unknown.total() + self.deck_known_bottom.len as u32
    }
    /// Deck contents as a multiset (known top + unknown + known bottom).
    pub fn deck_counts(&self) -> Counts {
        let mut c = self.deck_known.counts();
        c.add_all(&self.deck_unknown);
        c.add_all(&self.deck_known_bottom.counts());
        c
    }
    /// How many copies of `card` the player owns, wherever they are: `all_cards().get(card)` without building the multiset
    /// (the zones listed here must be the ones `all_cards` sums; a test checks the two agree).
    pub fn count_of(&self, card: CardId) -> u32 {
        let mut n = self.deck_known.iter_top_down().filter(|&c| c == card).count() as u32
            + self.deck_known_bottom.iter_front_to_back().filter(|&c| c == card).count() as u32
            + self.deck_unknown.get(card) as u32
            + self.hand.get(card) as u32
            + self.discard.get(card) as u32
            + self.in_play.get(card) as u32
            + self.set_aside.get(card) as u32
            + self.passed.get(card) as u32
            + self.held.get(card) as u32
            + self.native_village_mat.get(card) as u32
            + self.island_mat.get(card) as u32
            + self.prince_cards.get(card) as u32
            + self.farm_cards.get(card) as u32
            + self.summon_cards.get(card) as u32
            + self.cleanup_aside.get(card) as u32
            + self.tavern.get(card) as u32
            + self.archive.get(card) as u32
            + self.aside.get(card) as u32
            + self.foresight.get(card) as u32
            + (self.saved == card && card != 0) as u32
            + (self.inherit == card && card != 0) as u32;
        if card == id::FAITHFUL_HOUND {
            n += self.hounds as u32;
        }
        if !self.church.is_empty() {
            let mut c = Counts::EMPTY;
            self.church.add_into(&mut c);
            n += c.get(card) as u32;
        }
        n
    }
    /// Every card the player owns, wherever it is.
    pub fn all_cards(&self) -> Counts {
        let mut c = self.deck_counts();
        c.add_all(&self.hand);
        c.add_all(&self.discard);
        c.add_all(&self.in_play);
        c.add_all(&self.set_aside);
        self.passed.add_into(&mut c);
        // The small zones add straight into the total (no temporary `Counts` per zone).
        self.held.add_into(&mut c);
        self.native_village_mat.add_into(&mut c);
        self.island_mat.add_into(&mut c);
        self.prince_cards.add_into(&mut c);
        self.farm_cards.add_into(&mut c);
        self.summon_cards.add_into(&mut c);
        self.church.add_into(&mut c);
        self.cleanup_aside.add_into(&mut c);
        self.tavern.add_into(&mut c);
        if self.saved != 0 {
            c.add(self.saved, 1);
        }
        self.archive.add_into(&mut c);
        self.aside.add_into(&mut c);
        self.foresight.add_into(&mut c);
        // The card under the Estate token is set aside, and in the player's deck when scoring (Inheritance).
        if self.inherit != 0 {
            c.add(self.inherit, 1);
        }
        if self.hounds != 0 {
            c.add(id::FAITHFUL_HOUND, self.hounds);
        }
        c
    }
    pub fn vp(&self) -> i32 {
        // Distant Lands is worth 4 VP on the Tavern mat at the end of the game and 0 anywhere else.
        let mut vp = vp_of_cards(&self.all_cards()) + self.vp_tokens as i32 + 4 * self.tavern.get(id::DISTANT_LANDS) as i32;
        // Nocturne: Miserable is -2 VP, Twice Miserable -4 VP.
        if self.noc & P_MISERABLE != 0 {
            vp -= if self.noc & P_TWICE_MISERABLE != 0 { 4 } else { 2 };
        }
        vp
    }
}

/// Victory points of a whole collection of cards (a card whose score varies, like Gardens, says so in its
/// set: `SetRules::vp`).
pub fn vp_of_cards(all: &Counts) -> i32 {
    let total = all.total() as i32;
    all.iter()
        .map(|(c, n)| crate::sets::vp(c, all, total) * n as i32)
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
    /// Nocturne: after the Buy phase, a player holding a Night card may play any number of them.
    Night,
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
    /// Potions available to spend this turn (Potion cards played; Alchemy). Buying a card with a
    /// Potion in its cost spends one; unspent Potions are lost at the end of the turn.
    pub potions: u8,
    /// Number of Merchant plays this turn (each gives +$1 on the first Silver).
    pub merchants: u8,
    pub silvers_played: u8,
    /// Global turn counter (1-based, counts every player's turn).
    pub number: u16,
    /// Whether the turn's start has been announced (TurnStart/PhaseStart events). Deferred to
    /// the first step of the turn so a turn-boundary pause logs nothing of the next turn.
    pub announced: bool,
    /// Whether `GameState::push_turn_start_frames` (Clerk's reaction, pending Duration effects)
    /// has been attempted this turn. Deliberately separate from `announced`: a state loaded from
    /// text always starts with `announced: true` (no re-announcing turn-start events for a
    /// position the reader is resuming mid-turn), but still needs its start-of-turn resolution to
    /// run if it hasn't yet (the common case: a freshly saved turn-start position). Idempotent-safe
    /// to attempt again on load even when already resolved (an empty `pending_durations` and no
    /// Clerk in hand make it a no-op); the one gap is a state saved after explicitly declining
    /// Clerk's reaction offering it again on reload, a narrow edge case accepted for now.
    pub turn_start_resolved: bool,
    /// Every card played this turn, counting each resolution (Throne Room's target twice).
    pub played: Counts,
    /// Cards cost this much less this turn, to a minimum of 0 (Bridge).
    pub cost_reduction: u8,
    /// Once a card has been bought this turn (2nd-edition rule), or once the player has passed
    /// on the `PlayTreasure` decision (declining durably, not just skipping one offer), no more
    /// Treasures may be played: gates both the choice-free auto-play and `PlayTreasure`.
    pub treasures_done: bool,
    /// Cards named for War Chest this turn (by any War Chest play): a War Chest's own gain may
    /// not be any of these.
    pub named_for_war_chest: SmallMultiset<WAR_CHEST_CAP>,
    /// Duration cards (and any Throne Room/King's Court that multiplied one) currently held in
    /// play for this turn's player: exempted from cleanup's discard. A running counter
    /// incremented wherever a card enters play by being played (see `effects::put`,
    /// `GameState::play_choice_free_treasures`, the two direct plays in `apply_phase`, and
    /// `Then::PlayPicked`'s multiplier hold), so it also covers a mid-turn save/reload. `TurnState`
    /// is per-turn, not per-player, but only the turn's own player ever plays cards into their own
    /// `in_play`, so this is unambiguous.
    pub duration_held: DurationHeld,
    /// True if this turn itself was granted as an extra turn by an Outpost played on the
    /// previous turn (so a second Outpost played now must not grant a further extra turn: no 3rd
    /// turn in a row). Also true when the game is between such turns, used by `pass`.
    pub is_extra_turn: bool,
    /// Set during this turn's cleanup when an Outpost in play grants an extra turn; read when
    /// `Phase::CleanupDraw` picks the next player and builds their `TurnState`. Purely transient
    /// (cleanup is never a text-format rest state), reset fresh every turn.
    pub outpost_grants_extra: bool,
    /// Corsair (owned by anyone else): whether the current player has already had a Silver or
    /// Gold trashed by it this turn (only the first each turn).
    pub corsair_trashed_first: bool,
    /// Throne Room / King's Court multiplier bookkeeping, live only while its target's `times`
    /// `PlayEffects` resolutions (and everything they push, e.g. Haven's set-aside pick) are
    /// still unwinding: 0 when no multiplier group is in progress. See `Then::PlayPicked` and
    /// `FrameKind::MultiplierFinalize`. Current rule: the multiplier stays in play with the
    /// Duration if *any* of its resolutions scheduled a next-turn effect ("even if only one play
    /// of the Duration card is keeping it in play, Throne Room stays in play with it"). Only when
    /// no resolution did anything for next turn (Haven with an empty hand, say) do both discard.
    pub multiplier_card: CardId,
    pub multiplier_expected: u8,
    pub multiplier_successes: u8,
    /// Distinct cards gained so far this turn (Seaside step 4: Smugglers), only maintained
    /// while Smugglers is in this game's supply (see `GameState::gain`). Snapshotted into the
    /// gaining player's `PlayerState::last_turn_gains` at cleanup, then reset for the next turn.
    /// Per-turn, within-turn bookkeeping like `treasures_done`/`named_for_war_chest`: not part
    /// of the text format, reset fresh on load (see `text.rs`'s module docs).
    pub gained_this_turn: SmallMultiset<GAIN_RECORD_CAP>,
    /// Whether this player has gained a Victory card during this Buy phase (Seaside step 4:
    /// Treasury's "if you didn't gain a Victory card in it"), set by `GameState::gain`. Per-turn
    /// bookkeeping, not part of the text format (same gap as `treasures_done`).
    pub gained_victory_in_buy: bool,
    /// Whether the end-of-Buy-phase offers (Treasury's "put this onto your deck?", and the
    /// discard-from-play offers of Alchemist and Herbalist) have already been pushed this turn
    /// (guards `GameState::push_end_of_turn_offers` against re-offering to a still-in-play,
    /// already-declined copy every time the effect stack empties out again before cleanup runs).
    /// Per-turn bookkeeping, not part of the text format.
    pub treasury_offered: bool,
    /// Reentrancy guard for Blockade's "gain a copy of the blockaded card -> gain a Curse"
    /// cross-trigger (`GameState::run_seaside_gain_triggers`): true only while processing a
    /// Curse just granted by that trigger, so a Blockaded Curse can't recursively re-trigger
    /// itself. Transient mid-resolution bookkeeping (never a text-format rest state, like
    /// `multiplier_card`): always false at any point the text format could observe.
    pub blockading_curse: bool,
    /// The start-of-turn effects not yet resolved, `GameState::run_start_order`: this turn's
    /// player's `pending_durations` (moved here at turn start so effects scheduled during the
    /// turn, for the next one, don't mix with them), their Clerks in hand, and Prince's set-aside
    /// cards. Transient: only non-empty while `FrameKind::StartOrder` is on the stack, and shown in
    /// the text format's `durations:` line (see `text.rs`).
    pub start_pending: StartList,
    /// True only while a card held by a Prince is being played at the start of the turn: it is not
    /// in play, so its effects that move "this" (Mining Village trashing itself, Treasure Map,
    /// Island) fail instead of moving a copy that happens to be in play.
    pub prince_play: bool,
    /// Cards that trashed themselves as part of their own effect this turn (Investment's
    /// trash-for-VP, Mining Village's +$2, Treasure Map): used, not lost. Lets a scoring
    /// function (the bots' `GainListEvaluator`) tell "used up" from "had a card taken". Tiny
    /// fixed record, per-turn bookkeeping, not part of the text format (reset on load).
    pub used_up: SmallMultiset<4>,
    /// Players (bit per seat) who revealed a blocking Reaction (Moat) to the Attack being
    /// resolved: unaffected by it. Reset when an Attack is played that opens a reaction window;
    /// transient, not part of the text format.
    pub moated: u8,
    /// Cards to draw at Clean-up on top of the usual 5 (3 after an Outpost): Farrier's overpay.
    pub extra_draw: u8,
    /// Merchant Guild plays this turn: each pays +1 Coffers per card gained in the Buy phase.
    pub guilds: u8,
    /// Cards the turn's player has gained during this Buy phase (Merchant Guild).
    pub gained_in_buy: u8,
    /// Adventures per-turn bookkeeping.
    pub adv: AdvTurn,
    /// Empires per-turn bookkeeping.
    pub emp: EmpTurn,
    /// Nocturne per-turn bookkeeping.
    pub noc: NocTurn,
    /// Hinterlands per-turn bookkeeping.
    pub hint: HintTurn,
    /// Rising Sun per-turn bookkeeping.
    pub rs: RsTurn,
}

/// The per-turn bookkeeping of Rising Sun.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RsTurn {
    /// Daimyos played this turn whose replay is still to come: the next non-Command Action played is replayed this many times.
    pub daimyo: u8,
    /// Cards the turn's player has gained this turn (only counted while `R_TRIBUTE`: Receive Tribute).
    pub gains: u8,
}

/// The per-game state of Rising Sun: the Prophecy, the Sun tokens on it, and Riverboat's card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RsState {
    /// `R_*` flags.
    pub flags: u8,
    /// The Sun tokens left on the Prophecy (0 once it has happened, or when there is none).
    pub suns: u8,
    /// The Prophecy of the game (a pseudo-card id), 0 = none.
    pub prophecy: CardId,
    /// The Prophecy once the last Sun token has left it, 0 before (or when there is none): the rule that applies to every player for the rest of the
    /// game. Equal to `prophecy` once active.
    pub active: CardId,
    /// The card set aside in set-up for Riverboat (an Action costing $5 that is not in the kingdom), 0 = no Riverboat in the game.
    pub riverboat: CardId,
    /// Nonzero while a gain has something to do: a Prophecy that changes or answers gains is in effect, or Kintsugi / Receive Tribute watch them
    /// (`GameState::rs_update_hooks`). One byte test in every gain.
    pub hook: u8,
}

/// Sun tokens put on the Prophecy for a game of `n` players.
pub fn sun_tokens_for(n: usize) -> u8 {
    match n {
        0..=2 => 5,
        3 => 8,
        4 => 10,
        5 => 12,
        _ => 13,
    }
}

/// The per-turn bookkeeping of Hinterlands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct HintTurn {
    /// Actions the turn's player has gained this turn (counted only while Cauldron is in the game; stops at 3): the third makes the other
    /// players gain a Curse for each Cauldron played before it.
    pub acts_gained: u8,
    /// Cauldrons played this turn.
    pub cauldrons: u8,
    /// Players (bit per seat) that were unaffected by a Cauldron played this turn (a Moat revealed, a Lighthouse in play): its Curses skip them.
    pub caul_shield: u8,
}

/// The per-turn bookkeeping of Nocturne.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct NocTurn {
    /// Cards the turn's player has gained this turn (Monastery, Devil's Workshop; only counted while `N_GAIN_HOOK`). An exchange is not a gain.
    pub gains: u8,
    /// Envious was returned at the start of this Buy phase: Silver and Gold make $1 this turn.
    pub envy: bool,
    /// Deluded was returned at the start of this Buy phase: no Action can be bought this turn.
    pub deluded: bool,
    /// Tracker was played this turn: the cards the player gains may be put onto their deck.
    pub tracker: bool,
    /// The Night phase: 0 = playing Night cards (or not reached), 1 = over (the Clean-up offers were made).
    pub night_over: bool,
    /// River's Gift: the cards each player draws at the end of this turn, by seat.
    pub river: [u8; MAX_PLAYERS],
    /// The Action cards Necromancer has turned face down this turn (each can be used once per turn).
    pub necro: SmallMultiset<4>,
}

/// The per-game state of Nocturne: the Boons and Hexes decks (which cards are where; the order is not stored, the next one is drawn at
/// random from the cards of the deck: nobody knows the order, and the cards in the deck are known to all), Lost in the Woods, and the piles
/// outside the Supply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct NocState {
    /// `N_*` flags.
    pub flags: u8,
    /// Bit `i` = Boon `FIRST_BOON + i` is in the (face-down) Boons deck, in the discard pile, set aside face up for Druid, or in front of a player
    /// (taken, or kept until Clean-up). A Boon being resolved is in none of them until it is placed.
    pub boon_deck: u16,
    pub boon_discard: u16,
    pub boon_druid: u16,
    pub boon_held: u16,
    /// The same for the Hexes (they are never held).
    pub hex_deck: u16,
    pub hex_discard: u16,
    /// Chance mode: the Boon / Hex a chance outcome has fixed for the next draw (index + 1), 0 = none yet.
    pub boon_top: u8,
    pub hex_top: u8,
    /// The seat (+ 1) of the player with Lost in the Woods, 0 = nobody.
    pub lost: u8,
    /// Will-o'-Wisp, Wish, Bat, Imp and Ghost left in their piles (`NOC_*`).
    pub piles: [u8; 5],
}

/// The per-turn bookkeeping of Empires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct EmpTurn {
    /// Cards the turn's player has gained this turn (Triumph, Labyrinth, Baths).
    pub gains: u8,
    /// Silvers the turn's player has gained this turn (Conquest).
    pub silvers: u8,
    /// Groundskeepers played this turn: +1 VP token per Victory card gained, each.
    pub groundskeepers: u8,
    /// Charms played this turn whose "next gain" has not happened yet.
    pub charms: u8,
    /// The turn's player has already played an Action this turn (Enchantress affects only the first).
    pub acted: bool,
    /// A Fortune has doubled the $ this turn.
    pub doubled: bool,
}

/// The per-turn bookkeeping of Adventures (Events bought, Travelling Fair, Sauna, Mission, the -$1 token).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct AdvTurn {
    /// Bitmask of the Events bought this turn (bit `event - FIRST_EVENT`): the once-per-turn Events are not offered again.
    pub bought: u64,
    /// Travelling Fair was bought this turn: cards gained may be put onto the deck.
    pub fair: bool,
    /// Saunas played this turn: each Silver played may trash that many cards from the hand.
    pub sauna: u8,
    /// Mission was bought this turn: an extra turn follows (if this one is not itself one).
    pub mission: bool,
    /// This is a Mission turn: no cards may be bought (Events still may).
    pub no_buy: bool,
    /// The turn's player has the -$1 token: the next time the turn's $ rises above `tax_base` they get $1 less.
    pub tax_live: bool,
    pub tax_base: u16,
    /// Port's second Port is being gained: it doesn't come with another.
    pub no_chain: bool,
    /// The next turn is a Mission turn (set at Clean-up): it will have `no_buy`.
    pub next_no_buy: bool,
}

impl TurnState {
    pub fn start(player: u8, number: u16) -> Self {
        TurnState {
            player,
            phase: Phase::Action,
            actions: 1,
            buys: 1,
            coins: 0,
            potions: 0,
            merchants: 0,
            silvers_played: 0,
            number,
            announced: false,
            turn_start_resolved: false,
            played: Counts::EMPTY,
            cost_reduction: 0,
            treasures_done: false,
            named_for_war_chest: SmallMultiset::EMPTY,
            duration_held: DurationHeld::EMPTY,
            is_extra_turn: false,
            outpost_grants_extra: false,
            corsair_trashed_first: false,
            multiplier_card: 0,
            multiplier_expected: 0,
            multiplier_successes: 0,
            gained_this_turn: SmallMultiset::EMPTY,
            gained_victory_in_buy: false,
            treasury_offered: false,
            blockading_curse: false,
            start_pending: StartList::EMPTY,
            prince_play: false,
            used_up: SmallMultiset::EMPTY,
            moated: 0,
            extra_draw: 0,
            guilds: 0,
            gained_in_buy: 0,
            adv: AdvTurn::default(),
            emp: EmpTurn::default(),
            noc: NocTurn::default(),
            hint: HintTurn::default(),
            rs: RsTurn::default(),
        }
    }
}

/// A zone a selection picks cards from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    Hand,
    Discard,
    /// Cards revealed / looked at / set aside (the player's `set_aside` zone).
    Revealed,
    /// Cards currently in play (Mining Village trashing itself).
    InPlay,
    /// The shared Supply (Lurker: trash an Action card from it).
    Supply,
    /// The shared trash pile (Lurker: gain a card from it).
    Trash,
    /// The known top card of the player's own deck, if there is one (a card just gained onto the
    /// deck: Watchtower trashes it, Sailor plays it). Empty while the top is unknown.
    DeckTop,
    /// The player's Tavern mat (Adventures).
    Tavern,
    /// The cards set aside under the player's Archives (Empires).
    Archive,
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
    /// Gain it (to the frame's `dest`): a gain event, not a buy. Used for Select frames whose
    /// zone is the source of the gain (Lurker: gain from the trash).
    Gain,
    /// Reveal it and leave it where it is (Courtier: reveal a card from hand without removing
    /// it). Generic: any future "reveal a card from a zone" effect reuses this.
    Reveal,
    /// Move it to the player's `passed` holding zone (Masquerade's simultaneous pass), to be
    /// delivered to the next passing player once everyone has chosen.
    Pass,
    /// Shuffle it into the deck (Inn): it joins the unknown part of the deck, and the whole deck is shuffled when the selection finishes (`Then::InnShuffle`).
    ShuffleIn,
    /// Put it onto the player's Tavern mat (Miser's Copper, a Reserve card played).
    Tavern,
    /// Call it: from the Tavern mat into play (a `YesNo` on a Reserve card; what the call does is the frame's `then`, `Then::Called`).
    Call,
    /// Exchange it (a Traveller in play, being discarded) for the next card of its line: it returns to its pile and the upgrade goes to the
    /// discard pile.
    Exchange,
    /// Take it into the hand from where it is (Settlers' Copper from the discard pile): not a draw, not a gain.
    ToHand,
    /// Return it to its pile (Wish, Bat, Vampire, a card exchanged for a Changeling): neither a trash nor a discard.
    Return,
    /// Set it aside face down under the player's Archives / Crypts (`PlayerState::archive`).
    Archive,
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
    /// Victory cards and Curses (Patrol).
    VictoryOrCurse,
    /// Action or Treasure cards (Crystal Ball's "play it" option, Rabble's discard).
    ActionOrTreasure,
    /// Treasure cards, plus Curse (statically, regardless of Charlatan). Chosen dynamically at
    /// frame-construction time by `GameState::treasure_filter` when Charlatan is in the game, so
    /// Anvil/Tiara/Mint's Treasure selections include Curse-as-Treasure; see `is_treasure`.
    TreasureOrCurse,
    /// Either of two specific cards (Apothecary: Copper or Potion).
    Either(CardId, CardId),
    /// A non-Duration Action card costing up to `n` coins and no Potion (Prince). The Select code checks
    /// the *current* cost (`GameState::filter_ok`); `matches` can only use the printed cost.
    ActionCostUpTo(u8),
    /// Any card that isn't an Action (Scrying Pool's "until revealing a non-Action").
    NonAction,
    /// An Action card other than the named card (Golem: "other than Golem cards").
    ActionNot(CardId),
    /// Any card except the named one (Journeyman: "cards without that name").
    NotCard(CardId),
    /// A card the player has no copy of in their hand (Hunting Party). Decided against the live hand
    /// (`GameState::filter_ok`); `matches` can only say "any".
    NotInHand,
    /// An Action card the player has no copy of in play (Shop). Decided against the live play area
    /// (`GameState::filter_ok`); `matches` can only say "an Action".
    NotInPlayAction,
    /// An Action that is not a Reward, and a Treasure that is not a Reward (Coronet: "a non-Reward Action").
    ActionNotReward,
    TreasureNotReward,
    /// A Treasure in play that this cleanup discards (Herbalist): not a Duration Treasure that is
    /// staying in play for next turn (Astrolabe). Decided against the live game
    /// (`GameState::filter_ok`); `matches` can only say "a Treasure".
    TreasureToDiscard,
    /// A card of a named group of types (see [`TypeSet`]): an Attack (Squire), a non-Duration Action (Procession), a Ruins (Cultist), ...
    Types(TypeSet),
    /// Any card that isn't a Treasure (Hermit), Curse counting as one under Charlatan (`GameState::filter_ok`).
    NonTreasure,
    /// A non-Duration Treasure (Counterfeit), Curse counting under Charlatan (`GameState::filter_ok`).
    TreasureNonDuration,
    /// A card costing at least `n` coins now, Potion or not (Sage: "costing $3 or more"; a Potion card with $3 counts).
    CostAtLeast(u8),
    /// A card with no Potion in its cost, costing from `lo` to `hi` coins now (Rogue, Graverobber, the Knights: "from $3 to $6").
    CostRange(u8, u8),
    /// A Victory card other than the named one (Rebuild).
    VictoryNot(CardId),
    /// A card with a copy left in its Supply pile (Pilgrimage: a copy of each can be gained). Decided against the live Supply
    /// (`GameState::filter_ok`); `matches` can only say "any".
    GainableCopy,
    /// A non-Duration, non-Command Action costing up to $4 and no Potion (Inheritance's pick). Decided against the live costs (`filter_ok`).
    InheritCard,
    /// Any card that isn't a Victory card (Banquet).
    NonVictory,
    /// A non-Duration, non-Command Action costing up to `n` coins, no Potion and no Debt (Overlord: the top card of a Supply pile).
    CommandCostUpTo(u8),
    /// A Treasure other than the named card (Pooka: "other than Cursed Gold").
    TreasureNot(CardId),
    /// A card of which the Attack's player (the turn's player) has a copy in play (Raider). Decided against the live play area (`filter_ok`).
    CopyInAttackersPlay,
    /// A non-Duration Action in the trash that is not turned face down: Necromancer's pick. Decided against the trash and the turn (`filter_ok`).
    NecroTarget,
    /// A card that shares a type with the named card (Locusts: "a cheaper card that shares a type with it").
    SharesTypeWith(CardId),
    /// An Action in play that this Clean-up discards (Scheme): not one that stays in play (a Duration held for the next turn, a Prince's).
    /// Decided against the live game (`filter_ok`); `matches` can only say "an Action".
    ActionToDiscard,
}

/// The groups of card types a `Filter::Types` selects (kept to one byte so that frames stay small).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeSet {
    /// Attack cards.
    Attack,
    /// Ruins.
    Ruins,
    /// Action cards.
    Action,
    /// Action cards that are not Durations.
    ActionNotDuration,
    /// A Curse, Ruins, Shelter or Victory card (Vagrant).
    CurseRuinsShelterVictory,
    /// A Castle (Small Castle's trash and gain).
    Castle,
    /// Action cards that are not Attacks (Continue).
    ActionNotAttack,
}

impl TypeSet {
    /// `(types of which a card needs at least one, types it may have none of)`.
    pub const fn masks(self) -> (u32, u32) {
        match self {
            TypeSet::Attack => (cards::ATTACK, 0),
            TypeSet::Ruins => (cards::RUINS, 0),
            TypeSet::Action => (cards::ACTION, 0),
            TypeSet::ActionNotDuration => (cards::ACTION, cards::DURATION),
            TypeSet::CurseRuinsShelterVictory => (cards::CURSE_T | cards::RUINS | cards::SHELTER | cards::VICTORY, 0),
            TypeSet::Castle => (cards::CASTLE, 0),
            TypeSet::ActionNotAttack => (cards::ACTION, cards::ATTACK),
        }
    }
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
            Filter::VictoryOrCurse => cards::is(c, cards::VICTORY) || cards::is(c, cards::CURSE_T),
            Filter::ActionOrTreasure => cards::is(c, cards::ACTION) || cards::is(c, cards::TREASURE),
            Filter::TreasureOrCurse => cards::is(c, cards::TREASURE) || c == id::CURSE,
            Filter::Either(a, b) => c == a || c == b,
            Filter::NonAction => !cards::is(c, cards::ACTION),
            Filter::ActionNot(x) => c != x && cards::is(c, cards::ACTION),
            Filter::NotCard(x) => c != x,
            Filter::NotInHand => true,
            Filter::NotInPlayAction => cards::is(c, cards::ACTION),
            Filter::ActionNotReward => cards::is(c, cards::ACTION) && !cards::is(c, cards::REWARD),
            Filter::TreasureNotReward => cards::is(c, cards::TREASURE) && !cards::is(c, cards::REWARD),
            Filter::TreasureToDiscard => cards::is(c, cards::TREASURE),
            Filter::ActionCostUpTo(n) => {
                cards::is(c, cards::ACTION) && !cards::is(c, cards::DURATION) && cards::cost(c) <= n && !cards::potion_cost(c) && cards::debt_cost(c) == 0
            }
            Filter::Types(set) => {
                let (any, none) = set.masks();
                let t = cards::def(c).types;
                t & any != 0 && t & none == 0
            }
            Filter::NonTreasure => !cards::is(c, cards::TREASURE),
            Filter::TreasureNonDuration => cards::is(c, cards::TREASURE) && !cards::is(c, cards::DURATION),
            Filter::CostAtLeast(n) => cards::cost(c) >= n,
            Filter::CostRange(lo, hi) => !cards::potion_cost(c) && cards::debt_cost(c) == 0 && (lo..=hi).contains(&cards::cost(c)),
            Filter::VictoryNot(x) => c != x && cards::is(c, cards::VICTORY),
            Filter::GainableCopy => true,
            Filter::InheritCard => {
                cards::is(c, cards::ACTION) && !cards::is(c, cards::DURATION) && !cards::is(c, cards::COMMAND) && cards::cost(c) <= 4 && !cards::potion_cost(c) && cards::debt_cost(c) == 0
            }
            Filter::NonVictory => !cards::is(c, cards::VICTORY),
            Filter::CommandCostUpTo(n) => {
                cards::is(c, cards::ACTION)
                    && !cards::is(c, cards::DURATION)
                    && !cards::is(c, cards::COMMAND)
                    && cards::cost(c) <= n
                    && !cards::potion_cost(c)
                    && cards::debt_cost(c) == 0
            }
            Filter::TreasureNot(x) => c != x && cards::is(c, cards::TREASURE),
            Filter::CopyInAttackersPlay => true,
            Filter::NecroTarget => cards::is(c, cards::ACTION) && !cards::is(c, cards::DURATION),
            Filter::SharesTypeWith(x) => cards::def(c).types & cards::def(x).types != 0,
            Filter::ActionToDiscard => cards::is(c, cards::ACTION),
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

/// Continuation run when a `Select`, `YesNo` or `RevealTop` frame finishes. `count` = cards
/// picked, `last` = last pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Then {
    Nothing,
    /// Draw one card per pick (Cellar).
    DrawPerPick,
    /// +$n per pick (Moneylender); on a `YesNo`, +$n if Yes (Mining Village's self-trash).
    CoinsPerPick(u8),
    /// If something was picked, gain a card costing up to cost(last) + plus (Remodel, Mine,
    /// Replace), or exactly that cost when `exact` (Upgrade). Normally to `dest`; when
    /// `dest_by_type` is set, the spawned Gain frame ignores `dest` and instead resolves its
    /// destination (and any attack) from the gained card's type (`GainedTypeDestAttack`, for
    /// Replace: `dest` is unused).
    GainUpTo { plus: u8, filter: Filter, dest: Dest, exact: bool, dest_by_type: bool },
    /// Resolve the picked card's effects `times` times (Throne Room).
    PlayPicked { times: u8 },
    /// Discard whatever is left in the Revealed zone (Bandit).
    DiscardRevealed,
    /// Select: reward `coins` iff exactly `count` cards were picked, neither more nor fewer
    /// (Mill: discard exactly 2 for +$2; discarding 0 or 1 gets nothing).
    CoinsIfCount { count: u8, coins: u8 },
    /// Select: gain a fixed card iff exactly `count` cards were picked (Trading Post: trash
    /// exactly 2, gain a Silver to hand).
    GainCardIfCount { count: u8, card: CardId, dest: Dest },
    /// Gain: bonus based on the gained card's type, every bonus that applies for a dual type
    /// (Ironworks: Action -> +1 Action, Treasure -> +$1, Victory -> +1 Card).
    GainedTypeStatBonus,
    /// Gain: an Action or Treasure gained card goes onto the deck instead of the frame's normal
    /// destination; a Victory card instead makes each other player (Moat allowing) gain a Curse.
    /// Both apply for a dual type (Replace + Harem: onto the deck AND curses).
    GainedTypeDestAttack,
    /// YesNo: +`coins` if Yes; if No (declined, or the subject wasn't there to act on), gain a
    /// copy of the YesNo's subject to the discard instead (Baron: discard an Estate for +$4,
    /// otherwise gain one).
    YesCoinsElseGainSubject(u8),
    /// RevealTop: once revealing finishes, move every revealed card matching `Filter` straight
    /// to hand, before anything left in the Revealed zone is dealt with by a later frame
    /// (Patrol: Victory cards and Curses go to hand, the rest get reordered back by a `Select`).
    MoveMatchingToHand(Filter),
    /// Select (a reveal-from-hand pick): if a card was revealed, push a `Mode` frame over its
    /// source's `cards::modes` table with picks = the revealed card's number of types, capped at
    /// the table's length (Courtier: "for each type it has, choose a different one"). Generic
    /// over any future "modes per type of the picked card" card.
    ModePerType,
    /// Select (a set-aside pick from hand): if a card was picked, push a `DeckPosition` frame to
    /// place it back into the deck (Secret Passage).
    TakeToDeckPosition,
    /// Select (a 0-pick frame used purely to run code once whatever was pushed above it has
    /// resolved): grant `actions` Actions if the player's hand then holds at most `max_hand`
    /// cards (Diplomat's conditional +2 Actions, evaluated after its own +2 Cards draw).
    ActionsIfHandAtMost { max_hand: u8, actions: u8 },
    /// YesNo (`act: Reveal`): if revealed, draw `draw` cards then discard exactly `discard`
    /// (Diplomat's reaction to an Attack being played).
    ReactDrawDiscard { draw: u8, discard: u8 },
    /// YesNo (`act: Reveal`): if revealed, the player is unaffected by the Attack being played
    /// (`TurnState::moated`; Moat). The frame's `source` is the Attack.
    BlockAttack,
    /// Select (a 0-pick frame sitting under the reactions a gain triggers): once they have run,
    /// move one copy of `subject` out of `zone` by `act` if the zone still holds at least `min`
    /// copies, i.e. nothing moved the gained card meanwhile (Replace: an Action or Treasure
    /// gained to the discard pile goes onto the deck, unless Watchtower or Tiara moved it first).
    MoveSubjectIfPresent,
    /// Select: if something was picked, gain a card costing up to a *fixed* amount, regardless
    /// of what was picked (Anvil: discard a Treasure to gain a card up to $4).
    GainFixedUpTo { max_cost: u8, filter: Filter, dest: Dest },
    /// Select (a reveal-from-hand pick, kept in the zone): if a Treasure was revealed, gain a
    /// copy of it (Mint).
    GainCopyOfRevealed { dest: Dest },
    /// Select: +1 VP token per `per` $ of the trashed card's cost, rounded down (Bishop: +1 VP
    /// per $2).
    VpPerCost { per: u8 },
    /// Select (trash any number from hand): gain a card costing *exactly* the summed current
    /// cost of everything trashed (each valued at its cost at the moment it was trashed, so
    /// Bridge/Quarry-style reductions already active apply on both sides) — Forge. Always runs,
    /// even if nothing was trashed (total $0: a Copper or Curse).
    GainExactCostSum { dest: Dest },
    /// Select: draw `draw` cards iff exactly `count` were picked; picking fewer (e.g. a hand
    /// with only 1 card, Vault's "discard 2 to draw 1") draws nothing.
    DrawIfCount { count: u8, draw: u8 },
    /// RevealTop: move every revealed card matching `Filter` straight to discard (Rabble),
    /// mirroring `MoveMatchingToHand` for Patrol.
    MoveMatchingToDiscard(Filter),
    /// Select (`Act::SetAside`, Haven) or Gain (Blockade): once the pick/gain resolves, schedule
    /// a pending start-of-next-turn Duration effect for the frame's own `source` card, resolved
    /// `times` times (always 1 in practice: each Throne Room/King's Court resolution independently
    /// asks its own set-aside/gain question, so multiplying shows up as separate entries with
    /// distinct arguments, not one entry with `times > 1`), with the picked/gained card as `arg`.
    /// No-op if nothing was picked/gained. See `GameState::push_duration_pending`.
    ScheduleDuration { times: u8 },
    /// Select (`Act::SetAside`, Island): once the pick resolves, move the picked card from
    /// `set_aside` onto the player's Island mat, alongside the Island card itself (already
    /// moved there directly when Island was played). No-op if nothing was picked (empty hand).
    MoveToIslandMat,
    /// Select: +$ equal to the summed current cost of everything picked (Salvager: the pick is
    /// always 0 or 1 card, so this is just that card's cost, computed at the moment of picking,
    /// same as `GainExactCostSum`/Forge).
    CoinsEqualToCostSum,
    /// Select (`Act::Trash`, Treasure Map's optional 2nd-copy trash): if a card was picked *and*
    /// the Treasure Map physically in play ("this") was also trashed by this same resolution
    /// (`Frame::self_trashed`), gain 4 Golds onto the deck.
    TreasureMapGold,
    /// RevealTop (Sea Chart, max 1): if a card was revealed, put it into hand when its owner
    /// already has a copy of it in play, else leave it on top of the deck (now known).
    SeaChartCheck,
    /// Select (trash from hand): gain a card for each type the trashed card has: `action` if it's
    /// an Action, `treasure` if a Treasure, `victory` if a Victory card (0 = nothing for that
    /// type). Transmute: Duchy / Transmute / Gold.
    GainPerType { action: CardId, treasure: CardId, victory: CardId },
    /// Select (trash from hand): draw one card per $1 the trashed card costs, plus
    /// `potion_extra` more if its cost includes a Potion (Apprentice: +2).
    DrawPerCost { potion_extra: u8 },
    /// RevealTop/RevealUntil: discard every revealed card that does NOT match the frame's own
    /// `filter` (Golem keeps the two Actions and discards the rest).
    DiscardRevealedNotMatching,
    /// Select (Zone::Revealed, `Act::Play`, one pick): play the picked card, then the other cards
    /// left in the Revealed zone (which are Actions), one after the other (Golem: "play the Action
    /// cards in either order"). The others are held in their `PlayEffects` frames meanwhile, so
    /// nothing the first card does to the Revealed zone can disturb them.
    PlayPickedThenRest,
    /// Select (Prince, `Act::SetAside`, optional): the picked card, already in `set_aside`, is held
    /// by the Prince (`PlayerState::prince_cards`), which then stays in play for the rest of the
    /// game (`PlayerState::stays`, with the Throne Room / King's Court playing it).
    PrinceHold,
    /// Select: +`actions` Actions and +`buys` Buys if a card was picked (Hamlet's two discards).
    BonusIfPicked { actions: u8, buys: u8 },
    /// Select: +`n` Coffers per card picked (Plaza's Treasure).
    CoffersPerPick(u8),
    /// Select (Butcher, trash from hand): if a card was picked, gain a card costing up to its cost
    /// plus the Coffers the player has, spending as many as the gain needs beyond its cost.
    GainWithCoffers,
    /// Select (Advisor): once the chosen card is discarded, every other revealed card goes to the hand.
    RevealedToHand,
    /// RevealTop (Carnival): one of each differently named revealed card goes to the hand, the rest
    /// are discarded.
    OneOfEachToHand,
    /// RevealUntil (Hunting Party, Journeyman): the revealed cards matching the frame's `filter` go to
    /// the hand, the rest are discarded.
    KeepMatchingDiscardRest,
    /// Select (`Act::SetAside`, Farmhands' gain trigger): the picked card, in `set_aside`, is held
    /// until the start of the player's next turn (`PlayerState::farm_cards`).
    HoldUntilNextTurn,
    /// Gain (Soothsayer's Curse): the player draws a card if the gain happens.
    DrawIfGained,
    /// Gain (Horn of Plenty): if the gained card is a Victory card, trash the Horn (`source`) from play.
    HornTrash,
    /// TopCard (Jester): a Victory card gives the victim a Curse; otherwise the attacker (`chooser`)
    /// picks who gains a copy.
    JesterGain,
    /// Name (Journeyman): once a card is named, reveal until 3 cards without that name.
    JourneymanDraw,
    /// Select (Joust, `Act::SetAside`, a Province from the hand): the Province is set aside until Clean-up
    /// (`PlayerState::cleanup_aside`) and a Reward is gained to the hand.
    JoustReward,
    /// Select (Stonemason, trash from hand): gain `count` cards each costing less than the trashed card.
    GainLess { count: u8 },
    /// YesNo (Market Square, discarded from the hand when one of its owner's cards is trashed): gain a Gold; offered again while another Market Square is in the hand.
    MarketSquareGold,
    /// YesNo (Beggar, discarded to another player's Attack): gain 2 Silvers, the first onto the deck.
    BeggarGain,
    /// YesNo (Urchin, trashed from play when another Attack is played): gain a Mercenary.
    UrchinMercenary,
    /// Select (Hermit, a non-Treasure from the discard pile): if none was picked, offer one from the hand.
    HermitHand,
    /// Select (Forager, trash): +$1 per differently named Treasure in the trash, counted once the trash's own effects are done.
    ForagerCoins,
    /// Select (Procession and Counterfeit, `Act::Play`): the picked card is played twice, then trashed if it can still be, and with
    /// `gain` a card costing exactly `plus` more is gained (an Action).
    PlayTrash { gain: bool, plus: u8 },
    /// Select (the Knights' attack, trash from the victim's revealed cards): discard the rest; if a Knight was trashed, the attacker trashes
    /// the Knight played (`source`).
    KnightTrashed,
    /// Select (Mercenary, trash from hand, up to 2): if two cards were trashed, +2 Cards, +$2 and the other players discard down to 3.
    MercenaryBonus,
    /// Select (Pillage, trash this from play): if it was trashed, gain 2 Spoils and the other players with 5 or more cards discard one of the attacker's choice.
    PillageRest,
    /// RevealTop (Ironmonger, the top card): +1 Action for an Action, +$1 for a Treasure and +1 Card for a Victory card, after the choice to discard it.
    IronmongerReveal,
    /// Name (Rebuild): reveal until a Victory card that was not named, trash it and gain a Victory card costing up to $3 more.
    RebuildNamed,
    /// Ask (Scavenger): put the deck into the discard pile.
    DeckToDiscard,
    /// Ask (Survivors): discard the revealed cards.
    DiscardRevealedAsk,
    /// Select (Death Cart, an Action from the hand): +$5 if one was trashed; otherwise the Death Cart itself may be trashed for it.
    DeathCartHand,
    /// Mode (Count): only the options `first..first + count` of the card's table are on offer in this frame, so that one table
    /// serves a card that makes two separate choices.
    ModeWindow { first: u8, count: u8 },
    /// YesNo (`Act::Call`, Adventures): the Reserve card `subject` was called (it is in play now); do what its call does. For a call after a
    /// played Action `last` is that Action (Royal Carriage replays it); for Duplicate it is the card gained.
    Called,
    /// Select (Artificer, discard any number from the hand): after the discards, optionally gain a card onto the deck costing exactly $1 per
    /// card discarded.
    ArtificerGain,
    /// TopCard (Giant's attack): the top card is revealed; trash it if it costs $3 to $6, else discard it and gain a Curse.
    GiantReveal,
    /// TopCard (Warrior's attack): the top card is discarded; trashed if it costs exactly $3 or $4.
    WarriorHit,
    /// RevealTop (Magpie, the top card): a Treasure goes to the hand, an Action or Victory card gains a Magpie, anything else stays on top.
    MagpieReveal,
    /// Gain (Messenger's first gain of the Buy phase): each other player gains a copy of the card gained.
    MessengerCopies,
    /// YesNo (Raze, trash this from play): after the trash (or the refusal: then a card from the hand is trashed instead), look at cards.
    RazeSelf,
    /// Select (Raze, trash a card from the hand): look at as many cards from the top of the deck as the trashed card costs, take one.
    RazeLook,
    /// Select (Storyteller, up to 3 Treasures from the hand): each Treasure is played as it is picked.
    StorytellerPlay,
    /// Select (a 0-pick frame under Storyteller's draw): pay all the $ for +1 Card each.
    StorytellerPay,
    /// Select (Save, a card from the hand set aside): keep it until the Clean-up draw is done.
    SaveAside,
    /// Select (a 0-pick frame under the Clean-up draw): the card Save set aside goes to the hand.
    SavedToHand,
    /// Select (Trade): gain `card` once per card picked.
    GainPerPick { card: CardId },
    /// Select (Pilgrimage, `Act::Reveal` on cards in play): gain a copy of each card as it is picked.
    GainCopyEachPick,
    /// Gain (Seaway): move the player's token `tok` to the pile of the card gained.
    TokenOnGained { tok: u8 },
    /// Select (Gear, `Act::SetAside`): each card set aside is scheduled to return at the start of the next turn.
    GearAside,
    /// Select (Disciple): the picked Action is played twice, then a copy of it is gained.
    PlayTwiceCopy,
    /// Select (Inheritance, `Act::SetAside` from the Supply): the card is under the player's Estate token.
    Inherit,
    /// Select (Change, trash from the hand): gain a card costing more $ than the one trashed; its Debt is the difference.
    ChangeGain,
    /// Gain (Change): take Debt equal to the $ the card gained costs beyond `Frame::max`, the cost of the card trashed.
    ChangeDebt,
    /// YesNo (Engineer, trash this from play): gain a card costing up to $4.
    EngineerGain,
    /// Select (Encampment, reveal a Gold or Plunder): with none revealed it is set aside and returns to its pile.
    EncampmentReveal,
    /// RevealTop (Patrician): a revealed card costing $5 or more goes to the hand, else back on the deck.
    PatricianCheck,
    /// Select (Catapult, trash from the hand): a card costing $3 or more gives each other player a Curse, a Treasure makes them discard down to 3.
    CatapultHit,
    /// Select (Gladiator, reveal a card from the hand): the player to the left may reveal a copy; if not, +$1 and a Gladiator is trashed from the pile.
    GladiatorReveal,
    /// YesNo (Gladiator, the left player reveals a copy): on No (or with none to reveal) +$1 for the Gladiator's player and a Gladiator is trashed.
    GladiatorMatch,
    /// Select (Legionary, reveal a Gold): the other players discard down to 2 cards, then draw a card.
    LegionaryHit,
    /// Select (Sacrifice, trash from the hand): +2 Cards +2 Actions for an Action, +$2 for a Treasure, +2 VP tokens for a Victory card.
    SacrificeBonus,
    /// Select (Temple): add 1 VP token to the source card's pile.
    AddPileVp,
    /// Gain (Wild Hunt's Estate): if the gain happened, take the VP tokens on the source card's pile.
    TakePileVp,
    /// Select (Annex): the cards picked stay in the discard pile; the rest is shuffled into the deck.
    AnnexShuffle,
    /// Select (a 0-pick frame under Conquest's gains): +1 VP token per Silver gained this turn.
    ConquestVp,
    /// Gain (Dominate's Province): if it was gained, +9 VP tokens.
    DominateVp,
    /// Gain (Triumph's Estate): if it was gained, +1 VP token per card gained this turn.
    TriumphVp,
    /// Gain (Ritual's Curse): if it was gained, trash a card from the hand for VP tokens equal to its cost in $.
    RitualTrash,
    /// Select (Arena, discard an Action): take 2 VP tokens from the Arena if it was done.
    ArenaVp,
    /// Select (Donate, trash from the hand): the rest of the hand is shuffled into the deck.
    DonateShuffle,
    /// RevealTop (Archive): the cards revealed are set aside under it (`PlayerState::archive`); one goes into the hand now.
    ArchiveSet,
    /// YesNo (Small Castle, trash this): gain a Castle; declined, a Castle may be trashed from the hand instead.
    SmallCastle,
    /// Select (Crown in the Buy phase): the Treasure picked is played twice.
    CrownTreasure,
    /// TopCard (Chariot Race): the card drawn is compared with the top card of the left player's deck.
    ChariotRace,
    /// Bid (Mountain Pass, after the last bid): the high bidder takes 8 VP tokens and the Debt they bid.
    MountainPass,
    /// Select (Shepherd, Victory cards discarded): `n` cards drawn per pick.
    DrawNPerPick(u8),
    /// Select (Zombie Apprentice, an Action trashed from the hand): +`draw` Cards and +`actions` Actions if one was.
    DrawActionsIfPicked { draw: u8, actions: u8 },
    /// ReceiveBoon (Pixie): once the top Boon is discarded, the player may trash the Pixie to receive it twice (`Then::PixieTrash` on the YesNo).
    PixieDraw,
    PixieTrash,
    /// ReceiveBoon (Blessed Village, gained): once the Boon is taken, the player may receive it now; otherwise it waits for the start of their next turn.
    VillageDraw,
    /// Select (Haunted Mirror, trashed; an Action discarded from the hand): gain a Ghost.
    MirrorGhost,
    /// Select (Exorcist, trash from the hand): gain a Spirit costing less than the card trashed.
    ExorcistGain,
    /// Select (Monastery, cards trashed from the hand): the allowance not used (the cards gained this turn, less the picks) may trash Coppers in play.
    MonasteryCopper,
    /// Select (a zero-pick frame under Tragic Hero's draw): with 8 or more cards in hand, trash the Tragic Hero and gain a Treasure.
    TragicHero,
    /// Select (Secret Cave, 3 cards discarded): the Duration is kept for +$3 next turn.
    SecretCave,
    /// RevealTop (Will-o'-Wisp): a revealed card costing $2 or less goes to the hand, any other stays on the deck.
    WispCheck,
    /// Select (Bat, up to 2 cards trashed from the hand): if any were, exchange the Bat for a Vampire.
    BatExchange,
    /// Select (a zero-pick frame under Vampire's gain): exchange the Vampire for a Bat.
    VampireBat,
    /// YesNo (Changeling, a card gained that costs $3 or more): return it to its pile and take a Changeling into the discard pile.
    ChangelingSwap,
    /// YesNo (Faithful Hound, discarded): set it aside until the end of the turn.
    HoundAside,
    /// Select (Lost in the Woods, a card discarded at the start of the turn): receive a Boon.
    LostBoon,
    /// TopCard (the Locusts Hex): trash the top card; a Copper or Estate gives a Curse, any other card a cheaper card that shares a type.
    LocustsHit,
    /// TopCard (Zombie Mason): trash the top card, then gain a card costing up to $1 more.
    MasonTrash,
    /// RevealTop (the Famine Hex): discard the Actions among the revealed cards and shuffle the rest into the deck.
    FamineSort,
    /// Select (Crypt, Treasures set aside): if any, the Crypt stays in play and puts one of them into the hand at the start of each turn.
    CryptSet,
    /// ReceiveBoon (Sacred Grove): the Boon is received, and unless it gives +$1 each other player may receive it.
    GroveBoon,
    /// Select (Conclave, an Action played from the hand): +1 Action if one was played.
    ConclavePlay,
    /// RevealUntil (Ghost): the Action revealed is set aside to be played twice next turn, the rest discarded.
    GhostSet,
    /// Select (a zero-pick frame under the Clean-up draw): the Faithful Hounds set aside go into the hand.
    HoundsToHand,
    /// YesNo (Tunnel, revealed from the discard pile when discarded): gain a Gold.
    TunnelGold,
    /// YesNo (Fool's Gold, trashed from the hand when another player gains a Province): gain a Gold onto the deck.
    FoolsGoldGain,
    /// YesNo (Trader, revealed when a card is gained): the card gained (`subject`) goes back to its pile and a Silver comes to the discard pile instead.
    TraderSwap,
    /// YesNo (Berserker, gained): when there is an Action in play it is played, with no question asked.
    BerserkerPlay,
    /// Select (Inn, Actions from the discard pile shuffled into the deck): the whole deck is shuffled, whatever was picked, even nothing.
    InnShuffle,
    /// Select (a zero-pick frame): draw until there are `n` cards in the hand.
    DrawUpTo(u8),
    /// Select (a zero-pick frame): draw `draw` cards if the hand then holds at most `max_hand` cards (Guard Dog).
    DrawIfHandAtMost { max_hand: u8, draw: u8 },
    /// Select: if a card was picked, choose one option of the card's table (Spice Merchant).
    ModeIfPicked,
    /// Select (Witch's Hut, discard 2): if both were Actions each other player gains a Curse (`subject` counts the Actions picked).
    WitchsHut,
    /// Select (Develop, trash from the hand): gain a card costing exactly $1 more and one costing exactly $1 less, onto the deck, in either order.
    DevelopGain,
    /// Select (Trader, trash from the hand): gain a `card` per $1 the card trashed cost.
    GainPerCost { card: CardId },
    /// Select (Church, `Act::SetAside`, up to 3 cards from the hand): each card as it is picked goes into the newest Church batch (`PlayerState::church`).
    ChurchAside,
    /// Select (Dismantle, trash a card from the hand): if it cost $1 or more, gain a cheaper card, then a Gold.
    DismantleGain,
    /// Select (a 0-pick frame under Summon's gain and its reactions): the card gained (`subject`) is set aside for the next turn if it is still where it was gained.
    SummonAside,
    /// Select (a zero-pick frame under Mountain Shrine's trash): +2 Cards if there is an Action in the trash.
    MountainShrine,
    /// Ask (Snake Witch, after its +1 Card): reveal the hand (no duplicates) and return the Snake Witch to its pile for a Curse for each other player.
    SnakeWitch,
    /// Ask (Gold Mine): gain a Gold and take 4 Debt.
    GoldMine,
    /// Select (Rice Broker, the card trashed from the hand): +2 Cards for a Treasure, +5 Cards for an Action.
    RiceBroker,
    /// RevealTop (Poet, the top card): a card costing $3 or less goes to the hand, any other stays on top.
    PoetCheck,
    /// Select (Kintsugi, trash from the hand): with a Gold gained this game, gain a card costing up to $2 more.
    KintsugiGain,
    /// Gain (Credit): take Debt equal to the $ the card gained costs.
    CreditDebt,
    /// Gain (Continue): the card gained is played, from the discard pile, in the Action phase the player returns to.
    ContinueGain,
    /// Select (a zero-pick frame under Continue's gain): return to the Action phase (+1 Action, +1 Buy) and play the card gained if it is still in the discard pile.
    ContinueReturn,
    /// RevealUntil (Foresight): the Action revealed is set aside until the end of the turn, the rest discarded.
    ForesightSet,
    /// Select (a zero-pick frame under the Clean-up draw): the cards Foresight set aside go into the hand.
    ForesightToHand,
    /// Gain (Receive Tribute): up to `count - 1` more Actions follow, each with a name no gain before it had (`Frame::subject` and `last`).
    TributeGain,
    /// Select (a zero-pick frame under a gain's reactions, Rapid Expansion): the card gained is set aside if it is still where it was gained.
    RapidAside,
}

/// A pending piece of work on the effect stack. Card effects that need input or span
/// several steps are built from these few generic frames, so the engine can stop and
/// resume at any decision or chance point, and new cards mostly compose existing frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    pub kind: FrameKind,
    /// The player this frame acts on (the victim for attacks): whose zones are read/written.
    pub player: u8,
    /// Who makes the decision, if any (defaults to `player`). Differs from `player` only when
    /// another player decides on this player's behalf (Swindler: the attacker chooses what the
    /// victim gains).
    pub chooser: u8,
    /// The card whose effect created this frame (0 with `FrameKind::Draw` from cleanup).
    pub source: CardId,
    pub zone: Zone,
    pub act: Act,
    pub filter: Filter,
    pub dest: Dest,
    /// Select: minimum/maximum picks. Draw/RevealTop: cards remaining. Gain: max cost (or the
    /// exact cost when `exact`). Mode: `min` is a bitmask of already-chosen indices into
    /// `cards::modes(source)`; `max` is the total number of picks required.
    pub min: u8,
    pub max: u8,
    /// Gain: match `max` exactly rather than up to it (Upgrade). Select: all or nothing: pick
    /// none, or once a pick is made, `max` of them (clamped to what's available) (Mill).
    pub exact: bool,
    /// Select: picks made so far. Library: 1 while `subject` awaits a decision. Mode: picks made
    /// so far (`count == max` once the frame is done).
    pub count: u8,
    /// Select: last picked card (also the lower bound for canonical ordering).
    pub last: CardId,
    /// Select: picks are order-sensitive (topdecking several cards), so no canonical ordering.
    pub ordered: bool,
    /// Select: before the first pick, `max` names a target zone size to reduce to rather than an
    /// absolute pick count; the actual (mandatory) count is computed live, from the zone's size
    /// the moment this frame is first run, and then baked in (`min = max = that count`, this flag
    /// cleared). This makes a "discard down to N" attack (Militia) pick up a hand-size change
    /// from a reaction that resolves first (Diplomat), instead of using a count computed when the
    /// frame was pushed, before the reaction ran. With `exact` it is a threshold instead (Clerk's
    /// "with 5 or more cards in hand, put one onto your deck"): `max` is the size from which one
    /// card is picked, none below it.
    pub down_to_target: bool,
    pub then: Then,
    /// The card a YesNo decision is about / the card to PlayEffects.
    pub subject: CardId,
    /// Nesting depth of the card effect this frame belongs to (0 = top level), for logging
    /// effects indented under the card that caused them.
    pub depth: u8,
    /// Gain: also exclude cards named for War Chest this turn (`GameState::turn.named_for_war_chest`).
    pub excl_named: bool,
    /// Select: running sum of the current cost of every card picked so far, used by
    /// `Then::GainExactCostSum` (Forge) and `Then::CoinsEqualToCostSum` (Salvager).
    pub cost_sum: u16,
    /// Gain (Smugglers): restrict legal choices to cards in the frame's player's right-hand
    /// neighbor's `last_turn_gains` (in addition to the ordinary cost/filter check), mirroring
    /// `excl_named`'s shape for a different per-card restriction.
    pub gain_from_record: bool,
    /// Select (Treasure Map, `Act::Trash`, Zone::Hand): whether the Treasure Map physically in
    /// play ("this") was already trashed before this frame was pushed, carried through so
    /// `Then::TreasureMapGold` can tell whether both copies ended up trashed by this resolution.
    pub self_trashed: bool,
    /// Select: using this pick reveals `source` from the player's hand (a Reaction such as
    /// Watchtower), so the first pick logs a Reaction event.
    pub reveal_source: bool,
    /// Gain: the reference cost includes a Potion. Up to: Potion-cost cards may be gained;
    /// `exact`: the gained card's Potion must match. False for "costing up to $5" style gains
    /// (no Potion cards).
    pub potion: bool,
    /// Gain: may be declined (`Choice::Pass`), e.g. University's "you may gain".
    pub optional: bool,
    /// Gain: strictly cheaper than `max` (with `potion`: the Potion part counts too): every part of
    /// the cost no larger, and not equal (Stonemason's trash gains).
    pub less: bool,
    /// Gain (Butcher): `min` is the base cost; a card costing more spends that many Coffers, and
    /// `max` is the base plus the Coffers the player has.
    pub pay_coffers: bool,
    /// Gain: `GF_*` flags (kept in one byte so that a `Frame` stays 48 bytes).
    pub gf: u8,
    /// Select: every pick is a different card (Pilgrimage: differently named cards).
    pub distinct: bool,
    /// Gain: strictly more $ than `max` (Change: "a card costing more $ than it"), Debt and Potion in the cost allowed.
    pub more: bool,
}

/// `Frame::gf`: Young Witch's Curse is skipped when the player reveals a Bane from their hand.
pub const GF_BANE_BLOCKS: u8 = 1;
/// `Frame::gf`: the choices are the Rewards (non-Supply, Joust) instead of the Supply piles.
pub const GF_FROM_SIDE: u8 = 2;
/// `Frame::gf`: the choices are the Spirit piles (Will-o'-Wisp, Imp, Ghost) costing less than `max` (Exorcist).
pub const GF_SPIRITS: u8 = 4;
/// `Frame::gf`: an exact gain that may cost `max` or `max + 2` (Develop: the card $1 less or $1 more than the card trashed, in either order); once one
/// is gained, a gain of exactly the other cost follows.
pub const GF_BOTH: u8 = 8;
/// `Frame::gf`: any cost will do (no comparison with `max`; Debt and Potion cards allowed): Receive Tribute's Actions.
pub const GF_ANY_COST: u8 = 16;
/// `Frame::gf`: the cards `Frame::subject` and `Frame::last` (when not 0) are not on offer: Receive Tribute's differently named Actions.
pub const GF_EXCL: u8 = 32;

impl Frame {
    pub fn new(kind: FrameKind, player: u8, source: CardId) -> Self {
        Frame {
            kind,
            player,
            chooser: player,
            source,
            zone: Zone::Hand,
            act: Act::Discard,
            filter: Filter::Any,
            dest: Dest::Discard,
            min: 0,
            max: 0,
            exact: false,
            count: 0,
            last: 0,
            ordered: false,
            down_to_target: false,
            then: Then::Nothing,
            subject: 0,
            depth: 0,
            excl_named: false,
            cost_sum: 0,
            gain_from_record: false,
            self_trashed: false,
            reveal_source: false,
            potion: false,
            optional: false,
            less: false,
            pay_coffers: false,
            gf: 0,
            distinct: false,
            more: false,
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
    /// Choose `max` options (picking `min` as the bitmask of chosen indices) from
    /// `cards::modes(source)`, offered in increasing index order; `player` may be another
    /// player (Torturer's victim). Resolved in index order once every pick is made.
    Mode,
    /// Name a card: a `DecisionKind::Name`. Two variants, told apart by `zone`:
    /// - Wishing Well (any other `zone`): offers every card that could be on top of `player`'s
    ///   deck (or discard, if the deck is empty); `chooser` is `player`. `subject` holds the
    ///   named card once chosen; the next step reveals the real top card and compares.
    /// - War Chest (`zone: Zone::Supply`): offers every card in this game's supply; `chooser` is
    ///   the player to `player`'s left. Once named, it's recorded in
    ///   `TurnState::named_for_war_chest` and a `Gain` frame (up to $5, excluding every name so
    ///   far this turn) follows — no reveal-and-compare step.
    Name,
    /// Place `subject` (already removed from hand into `set_aside`) into `player`'s deck: a
    /// `DecisionKind::DeckPosition` (Secret Passage).
    DeckPosition,
    /// Trash the top card of `player`'s deck, then push a `Gain` frame (for `player`, decided by
    /// `chooser`) for a card costing exactly its cost. Swindler: `chooser` is the attacker.
    TrashTopThenGain,
    /// "Each player with cards in hand passes one to the next such player to their left, at
    /// once" (Masquerade): once `player`'s own +2 Cards has resolved (this frame sits below it,
    /// so it only runs afterward), push a `Select` (`Act::Pass`) frame for every player who
    /// currently has cards in hand, then a `PassLeftDeliver`, then `player`'s optional trash. No
    /// decision; deferred so who-has-cards reflects the post-draw hand, not the hand when
    /// Masquerade was played.
    PassLeftBegin,
    /// Once every passing player has chosen (their pick sits in their own `passed` zone),
    /// deliver each held card to the next passing player to the left, all at once. No decision.
    /// Masquerade is the only card that uses `PassLeftBegin`/`PassLeftDeliver` today, but the
    /// mechanism ("each player with X passes one to the next such player") isn't specific to it.
    PassLeftDeliver,
    /// Sits below a Throne Room/King's Court's `times` `PlayEffects` resolutions of a Duration
    /// target (and everything they push): once every one of them has fully unwound, checks
    /// whether all `times` actually scheduled a next-turn effect (`TurnState::multiplier_*`) and,
    /// if so, keeps the multiplier itself (`source`) in play too. See `Then::PlayPicked`.
    MultiplierFinalize,
    /// An Attack's own effects, held back (`subject`) until the reaction window opened above it
    /// has resolved, so a victim's Moat is known before the Attack does anything. No decision.
    AttackBody,
    /// Native Village's "put the top card of your deck face down on your mat" mode option:
    /// chance-aware like `Draw` (may need a `Step::Chance` sample), but the card goes to the
    /// player's `native_village_mat` instead of their hand.
    NativeVillageAdd,
    /// Reveal cards from the top of `player`'s deck into the Revealed zone until `max` of them
    /// match `filter` (`count` = matches so far) or the deck and discard run out, then run `then`
    /// (Scrying Pool: until a non-Action; Golem: 2 Actions other than Golem).
    RevealUntil,
    /// Sits at the bottom of the stack for the start of a turn, over `TurnState::start_pending`
    /// (the start-of-turn effects not yet resolved). Each run it takes the next effect: the only
    /// one left, or the first when order can't matter, otherwise it asks the player which
    /// resolves next (`DecisionKind::StartOrder`). Pops once none are left. See
    /// `GameState::run_start_order`.
    StartOrder,
    /// Overpaying for the card just bought (`source`): asks how much extra to pay
    /// (`DecisionKind::Overpay`), then gains the card and resolves its overpay ability.
    Overpay,
    /// Take the top card of `player`'s deck (chance-aware), move it by `act` (Jester: discard it) and
    /// run `then` on it.
    TopCard,
    /// An optional effect with no card to act on (Scavenger's "put your deck into your discard pile", Survivors' "discard them"): Yes
    /// runs `then`, No does nothing. Decision: `DecisionKind::YesNo`, subject 0.
    Ask,
    /// Adventures: the card `subject` has just been played and fully resolved: offer the calls that follow a played Action (Coin of the Realm,
    /// Royal Carriage). No decision of its own: it pushes the `YesNo` frames.
    AfterPlay,
    /// Adventures: move token `subject` (`TOK_*`) to a pile of the player's choice (`DecisionKind::Token`); `max` = 1 restricts it to piles
    /// the player has no token on (Teacher).
    Token,
    /// Empires: `player` bids Debt (Mountain Pass, `DecisionKind::Bid`). With `then: Then::MountainPass` it is the resolution after the last bid.
    Bid,
    /// Nocturne: `player` receives a Boon. `subject` is the Boon once it is known (0 = the next Boon of the deck is drawn when the frame runs, which
    /// can need a chance outcome); `count` how many times it is received (Pixie: 2). `optional` first asks `player` whether to receive it (Sacred
    /// Grove's other players, Blessed Village's "now or at the start of your next turn"); `then` says what else happens (`Then::PixieDraw`,
    /// `Then::VillageDraw`).
    ReceiveBoon,
    /// Nocturne: Hexes. With `subject` 0 this draws the next Hex (chance-aware) and then receives it for every player in the bit mask `min`, in turn
    /// order from the left of `player` (the Doom card's player; `player` themselves when the mask holds them: "receive a Hex"); with `subject`
    /// set, `player` receives that Hex now.
    ReceiveHex,
    /// Nocturne: pick a Boon from a pool (`cost_sum` is the bit mask of the Boons) to receive: Druid's three set-aside Boons (`exact`: the pool is
    /// unchanged), or the Boons a Fool took (`max` of them are drawn into the pool first; each is received and leaves the pool, until it is empty).
    /// `DecisionKind::PickBoon`.
    PickBoon,
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
    /// The Colony pile is empty (Prosperity games with Colonies).
    ColoniesGone,
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
    /// Young Witch's Bane pile: a Kingdom card costing $2 or $3 that is not in `kingdom`. `None` =
    /// chosen from the kingdom when a Young Witch needs one (`cards::default_bane`).
    pub bane: Option<CardId>,
    /// Ferryman's pile: an unused Kingdom card costing $3 or $4, outside the Supply. `None` = chosen
    /// from the kingdom when a Ferryman needs one (`cards::default_ferryman`).
    pub ferryman: Option<CardId>,
    /// Whether the starting decks hold the three Shelters (Hovel, Necropolis, Overgrown Estate) instead of the three Estates.
    /// `None` = the official rule, decided by the game's seed (`cards::uses_shelters`): always when every kingdom card is from
    /// Dark Ages, never when none is, otherwise when a randomly chosen kingdom card is.
    pub shelters: Option<bool>,
    /// The Obelisk's pile (an Action Supply pile, named by its top card or marker). `None` = chosen from the game's seed.
    pub obelisk: Option<CardId>,
    /// Riverboat's card (an Action costing $5 that is not in the kingdom and not a Duration). `None` = chosen from the game's seed when a Riverboat needs one.
    pub riverboat: Option<CardId>,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig { num_players: 2, kingdom: cards::FIRST_GAME.to_vec(), seed: 0, max_turns: 200, bane: None, ferryman: None, shelters: None, obelisk: None, riverboat: None }
    }
}

/// The VP and Debt tokens on Supply piles (Gathering cards, Aqueduct and Defiled Shrine's setup, Tax), as a short list: `(pile, VP, Debt)`.
/// A pile is named by `cards::pile_of` (a split pile by its top half, the Castles by their marker, Knights and Ruins by theirs).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PileTokens {
    pub piles: [CardId; PILE_TOKEN_CAP],
    pub vp: [u8; PILE_TOKEN_CAP],
    pub debt: [u8; PILE_TOKEN_CAP],
    pub len: u8,
}

impl Default for PileTokens {
    fn default() -> Self {
        PileTokens { piles: [0; PILE_TOKEN_CAP], vp: [0; PILE_TOKEN_CAP], debt: [0; PILE_TOKEN_CAP], len: 0 }
    }
}

impl PileTokens {
    fn slot(&self, pile: CardId) -> Option<usize> {
        self.piles[..self.len as usize].iter().position(|&c| c == pile)
    }
    fn slot_or_new(&mut self, pile: CardId) -> usize {
        match self.slot(pile) {
            Some(i) => i,
            None => {
                let i = self.len as usize;
                assert!(i < PILE_TOKEN_CAP, "more piles with tokens than PILE_TOKEN_CAP");
                self.piles[i] = pile;
                self.len += 1;
                i
            }
        }
    }
    /// VP tokens on `pile`.
    pub fn vp_on(&self, pile: CardId) -> u8 {
        self.slot(pile).map_or(0, |i| self.vp[i])
    }
    /// Debt tokens on `pile` (Tax).
    pub fn debt_on(&self, pile: CardId) -> u8 {
        self.slot(pile).map_or(0, |i| self.debt[i])
    }
    pub fn add_vp(&mut self, pile: CardId, n: u8) {
        let i = self.slot_or_new(pile);
        self.vp[i] = self.vp[i].saturating_add(n);
    }
    pub fn add_debt(&mut self, pile: CardId, n: u8) {
        let i = self.slot_or_new(pile);
        self.debt[i] = self.debt[i].saturating_add(n);
    }
    /// Take every VP token off `pile`; how many there were.
    pub fn take_vp(&mut self, pile: CardId) -> u8 {
        match self.slot(pile) {
            Some(i) => std::mem::take(&mut self.vp[i]),
            None => 0,
        }
    }
    /// Take 1 VP token off `pile`; whether there was one.
    pub fn take_one_vp(&mut self, pile: CardId) -> bool {
        match self.slot(pile) {
            Some(i) if self.vp[i] > 0 => {
                self.vp[i] -= 1;
                true
            }
            _ => false,
        }
    }
    pub fn take_debt(&mut self, pile: CardId) -> u8 {
        match self.slot(pile) {
            Some(i) => std::mem::take(&mut self.debt[i]),
            None => 0,
        }
    }
    pub fn clear_vp(&mut self) {
        self.vp = [0; PILE_TOKEN_CAP];
    }
    /// Takes every token off `pile` (the pile left the game).
    pub fn clear_pile(&mut self, pile: CardId) {
        if let Some(i) = self.slot(pile) {
            self.vp[i] = 0;
            self.debt[i] = 0;
        }
    }
    pub fn clear_debt(&mut self) {
        self.debt = [0; PILE_TOKEN_CAP];
    }
    /// The piles that carry any token, with their VP and Debt.
    pub fn iter(&self) -> impl Iterator<Item = (CardId, u8, u8)> + '_ {
        (0..self.len as usize).map(|i| (self.piles[i], self.vp[i], self.debt[i])).filter(|&(_, v, d)| v > 0 || d > 0)
    }
}

/// The per-game state of Empires: which Landmarks are in the game, the VP tokens on them and on Supply piles, and Mountain Pass's bidding.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EmpState {
    /// Bit `landmark - FIRST_LANDMARK`: the Landmarks in this game (public).
    pub landmarks: u32,
    /// `E_*` flags.
    pub flags: u8,
    /// VP tokens on each Landmark (index `landmark - FIRST_LANDMARK`): Arena, Basilica, Baths, Battlefield, Colonnade and Labyrinth start with 6 per
    /// player, Aqueduct and Defiled Shrine collect.
    pub lm_vp: [u8; cards::NUM_LANDMARKS],
    /// Obelisk's pile (a Supply pile named by `cards::pile_of`), 0 = none.
    pub obelisk: CardId,
    /// Mountain Pass: someone has gained a Province already (so no more bidding).
    pub pass_done: bool,
    /// Mountain Pass: the highest bid so far and the seat that made it.
    pub bid_high: u8,
    pub bid_who: u8,
    /// VP and Debt tokens on Supply piles.
    pub piles: PileTokens,
}

/// How many trashed cards wait for their "when you trash this" effects (`GameState::trash_queue`).
pub const TRASH_QUEUE_CAP: usize = 12;

/// `TrashQueue` entry kinds: a card trashed that was not the player's own (taken from the Supply), one that was, and a card discarded whose
/// owner may react to it (Tunnel, Trail, Weaver: `SetRules` of Hinterlands).
pub const Q_TRASHED: u8 = 0;
pub const Q_TRASHED_OWN: u8 = 1;
pub const Q_DISCARDED: u8 = 2;

/// Cards trashed (or discarded) whose effects have not been put on the stack yet: `(player told to trash or discard, card, kind `Q_*`)`.
/// A selection that trashes several cards trashes them all first and then resolves what they do (`Dark Ages`, "Count"); the same goes for
/// the discards that cards react to, which happen all at once (Hinterlands). The queue is flushed into frames when the selection
/// finishes (`GameState::flush_trash_triggers`). Empty between effects.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrashQueue {
    pub items: [(u8, CardId, u8); TRASH_QUEUE_CAP],
    pub len: u8,
}

impl Default for TrashQueue {
    fn default() -> Self {
        TrashQueue { items: [(0, 0, 0); TRASH_QUEUE_CAP], len: 0 }
    }
}

impl TrashQueue {
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The players' seats: `MAX_PLAYERS` slots of which only the first `n` (the game's player count) hold a player. The others are uninitialized
/// (`MaybeUninit`) and never read, so `GameState::dup` can leave them out of a copy: a 2-player game then copies 5.4 KB of seats, not 16.3 KB.
/// Indexing a seat that is not in the game is a bug: debug builds assert, a release build would read uninitialized memory.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Seats {
    n: u8,
    seats: [std::mem::MaybeUninit<PlayerState>; MAX_PLAYERS],
}

impl Seats {
    /// `n` seats, not yet initialized: `init` must run before any is used.
    fn uninit(n: usize) -> Seats {
        debug_assert!(n <= MAX_PLAYERS);
        Seats { n: n as u8, seats: [std::mem::MaybeUninit::uninit(); MAX_PLAYERS] }
    }
    /// Gives each of the `n` seats an empty player, in place.
    fn init(&mut self) {
        for s in &mut self.seats[..self.n as usize] {
            s.write(PlayerState::default());
        }
    }
    /// The players in the game, in seat order.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &PlayerState> {
        // SAFETY: the first `n` seats were initialized by `new` and are the only ones `Seats` hands out.
        self.seats[..self.n as usize].iter().map(|s| unsafe { s.assume_init_ref() })
    }
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut PlayerState> {
        // SAFETY: as in `iter`.
        self.seats[..self.n as usize].iter_mut().map(|s| unsafe { s.assume_init_mut() })
    }
    pub fn len(&self) -> usize {
        self.n as usize
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
}

impl std::ops::Index<usize> for Seats {
    type Output = PlayerState;
    #[inline(always)]
    fn index(&self, i: usize) -> &PlayerState {
        debug_assert!(i < self.n as usize, "seat {i} is not in this {}-player game", self.n);
        // SAFETY: seats below `n` are initialized (see `iter`).
        unsafe { self.seats[i].assume_init_ref() }
    }
}

impl std::ops::IndexMut<usize> for Seats {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut PlayerState {
        debug_assert!(i < self.n as usize, "seat {i} is not in this {}-player game", self.n);
        // SAFETY: seats below `n` are initialized (see `iter`).
        unsafe { self.seats[i].assume_init_mut() }
    }
}

impl PartialEq for Seats {
    fn eq(&self, other: &Seats) -> bool {
        self.n == other.n && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl Eq for Seats {}

impl std::hash::Hash for Seats {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.n.hash(state);
        for p in self.iter() {
            p.hash(state);
        }
    }
}

/// `repr(C)` with `players` last: `GameState::dup` copies a prefix of the struct, leaving out the seats beyond the game's players.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct GameState {
    pub supply: Counts,
    /// Bitmask of card ids whose piles are part of this game's supply.
    pub in_supply: SupplyMask,
    /// Young Witch's Bane card (its pile is an ordinary Supply pile), 0 when there is none.
    pub bane: CardId,
    /// Ferryman's pile: its card (0 = none) and how many are left. Not in the Supply: gained only by
    /// gaining a Ferryman (`GameState::gain_side`).
    pub ferryman: CardId,
    pub ferryman_left: u8,
    /// The Rewards left (`cards::FIRST_REWARD + i`), outside the Supply: gained only by Joust.
    pub rewards: [u8; cards::NUM_REWARDS],
    /// The Dark Ages piles outside the Supply: Spoils (15), Madman (10) and Mercenary (10), each 0 in a game without a card that gains it.
    pub spoils: u8,
    pub madman: u8,
    pub mercenary: u8,
    /// The dealt order of the Ruins and Knights piles, bottom first: entry `offset + i` is the member index (0-based into the pile's cards) of
    /// the card with `i` cards beneath it, so the top card is entry `offset + size - 1` (the Supply count of the top card is the pile's size).
    /// Entries at or above the size are 0. Only the top card is public; the rest is face down (`PlayerView::determinize` deals it again).
    pub pile_order: [u8; crate::sets::dark_ages::PILE_ORDER_LEN],
    /// The Events in this game (bit `event - cards::FIRST_EVENT`; Adventures, then Empires). Events are bought, never gained, and are in no zone or pile.
    pub events: u64,
    /// Empires: Landmarks, tokens on piles, Mountain Pass's bids, `E_*` flags.
    pub emp: EmpState,
    /// Nocturne: the Boons and Hexes decks, Lost in the Woods, the piles outside the Supply, `N_*` flags.
    pub noc: NocState,
    /// Rising Sun: the Prophecy and its Sun tokens, Riverboat's card, `R_*` flags.
    pub rs: RsState,
    /// The Traveller upgrade piles outside the Supply (`cards::FIRST_TRAV + i`: Treasure Hunter, Warrior, Hero, Champion, Soldier, Fugitive,
    /// Disciple, Teacher), five each when the line's first card (Page, Peasant) is in the kingdom.
    pub trav: [u8; cards::NUM_TRAV],
    /// `G_*` flags: which token effects are live in this game.
    pub adv: u8,
    /// Whether the players started with Shelters (Hovel's reaction to a gained Victory card is live).
    pub shelters: bool,
    /// Trashed cards whose "when you trash this" effects are still to be put on the stack (see `TrashQueue`).
    pub trash_queue: TrashQueue,
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
    pub num_players: u8,
    pub players: Seats,
}

/// The bits of `in_supply.top` of the bottom halves of the split piles (ids 258-263).
const SPLIT_BOTTOMS_TOP: u128 = 0b111111 << (258 - 256);

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
    /// A copy of this state, as `*self` but without the seats beyond the game's players (they are not read, see `Seats`): the copy the search
    /// and the bots make at every node, in a 2-player game about half the bytes of a plain copy (`dup_bytes`).
    #[inline]
    pub fn dup(&self) -> GameState {
        let live = self.dup_bytes();
        let mut out = std::mem::MaybeUninit::<GameState>::uninit();
        // SAFETY: `players` is the last field of this `repr(C)` struct and `seats` the last of `Seats`, so the first `live` bytes hold every field
        // but the seats `n..`; the rest of `out` stays uninitialized, which is valid for the `MaybeUninit` seats it belongs to. (Padding bytes
        // inside the copied prefix are copied as they are.)
        unsafe {
            std::ptr::copy_nonoverlapping(self as *const GameState as *const u8, out.as_mut_ptr() as *mut u8, live);
            out.assume_init()
        }
    }

    /// How many bytes of this state `dup` copies: all of it but the seats beyond the game's players (the first `dup_bytes` bytes of the struct).
    #[inline]
    pub fn dup_bytes(&self) -> usize {
        let live = std::mem::offset_of!(GameState, players) + std::mem::offset_of!(Seats, seats) + self.players.n as usize * std::mem::size_of::<PlayerState>();
        debug_assert!(live <= std::mem::size_of::<GameState>());
        live
    }

    /// Standard setup: supply sized per player count, 7 Copper + 3 Estate each,
    /// initial 5-card draws pending on the stack (resolved by the first `advance`).
    pub fn new(cfg: &GameConfig) -> Self {
        let n = cfg.num_players;
        assert!((2..=MAX_PLAYERS).contains(&n), "player count must be 2..={MAX_PLAYERS}");
        let mut supply = Counts::EMPTY;
        let mut in_supply = SupplyMask::default();
        let mut put = |c: CardId, k: u8| {
            supply.set(c, k);
            in_supply.insert(c);
        };
        put(id::COPPER, 60 - 7 * n as u8);
        put(id::SILVER, 40);
        put(id::GOLD, 30);
        put(id::ESTATE, victory_pile_size(n));
        put(id::DUCHY, victory_pile_size(n));
        put(id::PROVINCE, province_pile_size(n));
        put(id::CURSE, 10 * (n as u8 - 1));
        // Cornucopia & Guilds set-up piles: Ferryman's (outside the Supply), Young Witch's Bane (an extra Supply pile),
        // the Rewards and Baker's Coffers.
        let has = |c: CardId| cfg.kingdom.contains(&c);
        let cg = crate::sets::cornucopia_guilds::setup(cfg, n);
        let (ferryman, bane) = (cg.ferryman, cg.bane);
        // The Bane pile is a Supply pile like the kingdom's (no allocation: this runs per game).
        let extra_bane = if bane != 0 && !has(bane) { Some(bane) } else { None };
        let kingdom = || cfg.kingdom.iter().copied().chain(extra_bane);
        // Dark Ages: the Knights and Ruins piles, Spoils / Madman / Mercenary and the Shelters (one pass over the kingdom).
        let da = crate::sets::dark_ages::setup(cfg, bane, ferryman, n);
        // Adventures: the Events are listed with the kingdom, take no Supply pile and are only a mask here.
        let events = kingdom().filter(|&k| cards::is_event(k)).fold(0u64, |m, k| m | 1 << (k - cards::FIRST_EVENT));
        let needs_potion = kingdom().any(cards::potion_cost);
        if needs_potion {
            put(id::POTION, crate::sets::pile_size(id::POTION, n).unwrap());
        }
        let mut split_pile = false;
        for k in kingdom() {
            // Events and Landmarks are listed with the kingdom and take no Supply pile.
            if cards::is_pseudo(k) {
                continue;
            }
            // A split pile is named by either half.
            let k = if cards::is_split_bottom(k) { cards::split_top(k).unwrap_or(k) } else { k };
            if k == id::SAUNA {
                if split_pile {
                    continue;
                }
                split_pile = true;
            }
            if k == id::CASTLES {
                continue; // the eight Castles are set up with the other Empires piles below
            }
            // Platinum and Colony (Prosperity) join the supply when the kingdom list names them.
            assert!(cards::is_kingdom(k) || cards::is_optional_basic(k), "{} is not a kingdom card", cards::name(k));
            assert!(cards::is_ready(k), "{} is not implemented yet", cards::name(k));
            if k == id::KNIGHTS {
                continue; // the pile of ten Knights is set up below
            }
            // A set may size its own piles (Platinum 12, Potion 16); otherwise a Victory pile is 8 with 2 players and 12
            // with 3 or more (Colony too: unlike the Provinces, 5 and 6 players add none), any other pile 10.
            let pile = crate::sets::pile_size(k, n).unwrap_or(if cards::is(k, cards::VICTORY) { victory_pile_size(n) } else { 10 });
            put(k, pile);
        }
        // Sauna/Avanto: five Avanto under the five Sauna.
        if split_pile {
            put(id::AVANTO, 5);
        }
        let pile_size = |k: CardId| if cards::is(k, cards::VICTORY) { victory_pile_size(n) } else { 10 };
        let mut s = GameState {
            num_players: n as u8,
            players: Seats::uninit(n),
            supply,
            in_supply,
            bane,
            ferryman,
            ferryman_left: if ferryman != 0 { pile_size(ferryman) } else { 0 },
            rewards: cg.rewards,
            spoils: da.spoils,
            madman: da.madman,
            mercenary: da.mercenary,
            pile_order: [0; crate::sets::dark_ages::PILE_ORDER_LEN],
            events,
            emp: EmpState::default(),
            noc: NocState::default(),
            rs: RsState::default(),
            trav: crate::sets::adventures::traveller_piles(&in_supply),
            adv: 0,
            shelters: da.shelters,
            trash_queue: TrashQueue::default(),
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
        s.players.init();
        for p in 0..n {
            s.players[p].deck_unknown.set(id::COPPER, 7);
            // The Shelters replace the Estates in the unknown deck: the opening hand is dealt from it with this game's RNG.
            if da.shelters {
                s.players[p].deck_unknown.set(id::HOVEL, 1);
                s.players[p].deck_unknown.set(id::NECROPOLIS, 1);
                s.players[p].deck_unknown.set(id::OVERGROWN_ESTATE, 1);
            } else {
                s.players[p].deck_unknown.set(id::ESTATE, 3);
            }
            s.players[p].coffers = cg.starting_coffers;
        }
        if da.knights {
            s.init_mixed_pile(&crate::sets::dark_ages::KNIGHTS_PILE, 10);
        }
        if da.ruins > 0 {
            s.init_mixed_pile(&crate::sets::dark_ages::RUINS_PILE, da.ruins);
        }
        // Empires: the Landmarks, the split piles' bottom halves, the Castles, Tax's Debt and the Obelisk's pile.
        crate::sets::empires::setup(&mut s, cfg, n);
        // Nocturne: the Boons and Hexes, the Heirlooms in the starting decks, the piles outside the Supply, the Zombies in the trash.
        crate::sets::nocturne::setup(&mut s, cfg, bane, n);
        // Hinterlands: discards are watched when a card reacts to them.
        crate::sets::hinterlands::setup(&mut s);
        // Rising Sun: the Prophecy and its Sun tokens, Riverboat's card.
        crate::sets::rising_sun::setup(&mut s, cfg, n);
        s.push_opening_deal();
        // Returned without the unused seats: a plain `s` here is a copy of all 20 KB.
        s.dup()
    }

    /// The `Phase::Setup` stack: every player's opening 5-card draw (player 0 first). Advancing
    /// deals them (randomly from each shuffled deck), then turn 1 starts.
    pub fn push_opening_deal(&mut self) {
        // Push in reverse so player 0 draws first.
        for p in (0..self.num_players as usize).rev() {
            self.stack.push(Frame { max: 5, ..Frame::new(FrameKind::Draw, p as u8, 0) });
        }
    }

    /// What `c` costs right now: its printed cost less this turn's reductions, min 0. Bridge is a
    /// flat turn-wide reduction; Quarry reduces Action cards for the rest of the turn once played
    /// (stacking per play: twice with Tiara is $4); Peddler reduces only itself, and only during
    /// its owner's Buy phase, by $2 per Action card they have in play. Everything that compares
    /// costs (buying, "gain a card costing up to", Remodel's +$2...) goes through this, never
    /// through `cards::cost` directly.
    #[inline(always)]
    pub fn cost(&self, c: CardId) -> u8 {
        let pi = self.turn.player as usize;
        let mut reduction: u32 = self.turn.cost_reduction as u32;
        // `in_supply` check first: a single cheap bitmask test that skips the (still cheap, but
        // not free) lookup entirely for the overwhelming majority of games, which have no
        // Quarry in the kingdom at all.
        if self.in_supply(id::QUARRY) && cards::is(c, cards::ACTION) {
            reduction += 2 * self.plays_this_turn(self.turn.player, id::QUARRY) as u32;
        }
        if c == id::PEDDLER && self.turn.phase == Phase::Buy {
            reduction += 2 * self.players[pi].in_play.count_type(cards::ACTION);
        }
        // Ferry's -$2 cost token: cards of its pile cost $2 less on its owner's turns.
        if self.adv & G_COST_TOKEN != 0 && cards::pile_of(c) == self.players[pi].tok[TOK_COST] && cards::is(c, cards::ACTION) {
            reduction += 2;
        }
        (cards::cost(c) as u32).saturating_sub(reduction) as u8
    }

    /// How many times `p` has played `card` this turn, for "this turn" effects of Treasures that
    /// outlive their card (Quarry, Hoard, Collection, Tiara): each play counts (Tiara plays a
    /// Treasure twice), and the effect stays when the card leaves play (Mint trashes it). A
    /// card in play that no play was recorded for (a position built by hand) counts once.
    #[inline]
    pub fn plays_this_turn(&self, p: u8, card: CardId) -> u8 {
        let in_play = self.players[p as usize].in_play.get(card);
        if p == self.turn.player { in_play.max(self.turn.played.get(card)) } else { in_play }
    }

    /// Whether `c` is a Treasure right now: statically Treasure-typed, or Curse when Charlatan is
    /// in this game's supply ("Curse is also a Treasure worth $1"). Use this (never
    /// `cards::is(c, TREASURE)`) wherever the engine asks "is this a Treasure" at game time.
    #[inline(always)]
    pub fn is_treasure(&self, c: CardId) -> bool {
        cards::is(c, cards::TREASURE) || (c == id::CURSE && self.in_supply(id::CHARLATAN))
    }

    /// The `Filter` to use for a Select/Gain that should match Treasures, extended to Curse when
    /// Charlatan makes it one this game (Anvil, Tiara, Mint). Chosen once, at frame-construction
    /// time (`Filter::matches` itself stays a pure, stateless function).
    #[inline]
    pub fn treasure_filter(&self) -> Filter {
        if self.in_supply(id::CHARLATAN) { Filter::TreasureOrCurse } else { Filter::Treasure }
    }

    /// Treasures `p` has in play right now, counting Curse when Charlatan makes it one (Bank).
    pub fn treasures_in_play(&self, p: u8) -> u32 {
        self.players[p as usize].in_play.iter().filter(|&(c, _)| self.is_treasure(c)).map(|(_, n)| n as u32).sum()
    }

    /// Card-specific rules on whether `c` may be bought right now, beyond cost and pile (the one
    /// place for them): Grand Market can't be bought with a Copper in play.
    #[inline]
    pub fn may_buy(&self, c: CardId) -> bool {
        crate::sets::may_buy(self, c)
    }

    #[inline]
    pub fn in_supply(&self, c: CardId) -> bool {
        self.in_supply.has(c)
    }

    /// The card ids of this game's supply piles, in id order (walks the `in_supply` bitmask, so
    /// it costs one step per pile rather than one per card id).
    #[inline]
    pub fn supply_cards(&self) -> SupplyCards {
        SupplyCards { lo: self.in_supply.lo, hi: self.in_supply.hi, top: self.in_supply.top, ext: self.in_supply.ext }
    }

    /// The cards left in the piles outside the Supply (Ferryman's, the Rewards, Spoils, Madman and Mercenary).
    pub fn side_pile_cards(&self) -> u32 {
        self.ferryman_left as u32
            + self.rewards.iter().map(|&n| n as u32).sum::<u32>()
            + self.spoils as u32
            + self.madman as u32
            + self.mercenary as u32
            + self.trav.iter().map(|&n| n as u32).sum::<u32>()
            + self.noc.piles.iter().map(|&n| n as u32).sum::<u32>()
    }

    pub fn empty_piles(&self) -> u32 {
        // Walks the supply 32 lanes at a time: a pile is empty when its bit is set in `in_supply` and its lane is zero.
        // Not piles of their own: the members of the Knights and Ruins piles (ids from `FIRST_PILE_CARD`, counted below as
        // one pile each) and Avanto (the split pile Sauna/Avanto counts once, as Sauna, when Avanto is gone too).
        let mut n = 0u32;
        let words = [self.in_supply.lo, self.in_supply.hi, self.in_supply.top, self.in_supply.ext];
        for (w, bits) in words.into_iter().enumerate() {
            for k in 0..4 {
                let block = w * 4 + k;
                if block >= crate::counts::LANES / 32 {
                    break;
                }
                let first = block * 32;
                let mut in_block = (bits >> (32 * k)) as u32;
                if first + 32 > cards::FIRST_PILE_CARD as usize {
                    in_block &= if first >= cards::FIRST_PILE_CARD as usize { 0 } else { (1u32 << (cards::FIRST_PILE_CARD as usize - first)) - 1 };
                }
                // The bottom halves of the split piles and the Castles (ids AVANTO..AVANTO + 14) are not piles of their own.
                if in_block != 0 && first <= id::AVANTO as usize + 13 && first + 32 > id::AVANTO as usize {
                    let lo = (id::AVANTO as usize).saturating_sub(first);
                    let hi = ((id::AVANTO as usize + 14).min(first + 32)) - first;
                    let span = if hi - lo >= 32 { u32::MAX } else { ((1u32 << (hi - lo)) - 1) << lo };
                    in_block &= !span;
                }
                if in_block != 0 {
                    n += (in_block & !self.supply.nonzero_mask(block)).count_ones();
                }
            }
        }
        // A split pile is empty when both halves are: the top half was counted above once it is gone.
        if self.in_supply.top & SPLIT_BOTTOMS_TOP != 0 {
            for (top, bottom) in cards::SPLIT_PILES {
                if self.in_supply(bottom) && self.supply.get(top) == 0 && self.supply.get(bottom) > 0 {
                    n -= 1;
                }
            }
        }
        // The Castles are one pile.
        if self.in_supply(id::HUMBLE_CASTLE) {
            n += (self.castles_left() == 0) as u32;
        }
        for pile in [&crate::sets::dark_ages::RUINS_PILE, &crate::sets::dark_ages::KNIGHTS_PILE] {
            n += (self.has_pile(pile) && self.pile_size(pile) == 0) as u32;
        }
        n
    }

    /// `empty_piles` the plain way, one pile at a time (the reference the fast version is tested against).
    #[doc(hidden)]
    pub fn empty_piles_scan(&self) -> u32 {
        // The members of the Knights and Ruins piles are not piles of their own: each mixed pile is one pile, empty when it has no cards.
        // A split pile is one pile: empty when both halves are (counted once, as its top half). The Castles are one pile.
        let mut n = self
            .supply_cards()
            .filter(|&c| {
                c < cards::FIRST_PILE_CARD
                    && !cards::is_split_bottom(c)
                    && !cards::is_castle(c)
                    && self.supply.get(c) == 0
                    && cards::split_bottom(c).map_or(true, |b| !self.in_supply(b) || self.supply.get(b) == 0)
            })
            .count() as u32;
        if self.in_supply(id::HUMBLE_CASTLE) {
            n += (self.castles_left() == 0) as u32;
        }
        for pile in [&crate::sets::dark_ages::RUINS_PILE, &crate::sets::dark_ages::KNIGHTS_PILE] {
            n += (self.has_pile(pile) && self.pile_size(pile) == 0) as u32;
        }
        n
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
        } else if self.in_supply(id::COLONY) && self.supply.get(id::COLONY) == 0 {
            Some(EndReason::ColoniesGone)
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

    /// "The player to `p`'s right": turns pass to the left (0 -> 1 -> ... -> 0), so this is
    /// whoever acted just before `p` (Smugglers, Monkey's owner). In a 2-player game, `p`'s only
    /// opponent.
    #[inline]
    pub(crate) fn right_of(&self, p: u8) -> u8 {
        let n = self.num_players;
        (p + n - 1) % n
    }

    /// `p`'s score: their cards and VP tokens, and what the Landmarks add (Empires).
    pub fn player_vp(&self, p: u8) -> i32 {
        self.players[p as usize].vp() + if self.emp.landmarks != 0 { self.landmark_score(p) } else { 0 }
    }

    pub fn scores(&self) -> [i32; MAX_PLAYERS] {
        let mut s = [0; MAX_PLAYERS];
        for p in 0..self.num_players as usize {
            s[p] = self.player_vp(p as u8);
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
            Some(EndReason::ProvincesGone) | Some(EndReason::ColoniesGone) | Some(EndReason::PilesEmpty) => {}
            _ => return None,
        }
        let n = self.num_players as usize;
        let scores = self.scores();
        let mut turns = [0u16; MAX_PLAYERS];
        for p in 0..n {
            let counts = p == self.turn.player as usize && self.turn.phase != Phase::GameOver && !self.turn.is_extra_turn;
            turns[p] = self.players[p].turns_taken + u16::from(counts);
        }
        Some(winners_of(&scores[..n], &turns[..n]))
    }
}
