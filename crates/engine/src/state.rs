//! Game state. Everything is fixed-size and `Copy`: cloning a state is a memcpy,
//! which is what makes search and undo cheap and keeps the hot loop allocation-free.

use crate::cards::{self, id, CardId};
use crate::counts::Counts;
use crate::engine::Pending;
use crate::rng::Rng;

pub const MAX_PLAYERS: usize = 6;
pub const KNOWN_CAP: usize = 120;
pub const STACK_CAP: usize = 48;
/// Max simultaneous pending start-of-next-turn Duration effects for one player. Each play of a
/// Duration card (or a Throne Room/King's Court resolution of one) needs at most one slot; same-
/// card argless plays merge into one entry (see `GameState::push_duration_pending`), so this is
/// generous for realistic turns (a handful of distinct Duration cards, plus Haven/Blockade/Sailor
/// copies, which don't merge since each carries its own argument).
pub const PENDING_DURATIONS_CAP: usize = 12;
/// Max distinct cards `DurationHeld` tracks in one turn (see its doc comment): generous for a
/// realistic turn (a handful of distinct Duration cards, plus any Throne Room/King's Court that
/// multiplied one).
pub const DURATION_HELD_CAP: usize = 8;

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

/// A compact, order-independent multiset of (card, count) pairs capped at `N` distinct card
/// ids: the same trick as `DurationHeld` below (see its doc comment for the full rationale),
/// generalized over the cap so one definition serves every use at its own size instead of
/// paying for a full 128-lane, 128-byte `Counts` per instance. Total *copies* of one card are
/// unbounded (`u8` count, saturating); only the number of distinct card ids is capped. Used for
/// the Native Village and Island mats (`MAT_CAP`) and each player's per-turn gain record for
/// Smugglers (`GAIN_RECORD_CAP`).
#[derive(Clone, Copy, Debug)]
pub struct SmallMultiset<const N: usize> {
    entries: [(CardId, u8); N],
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
    pub const EMPTY: Self = SmallMultiset { entries: [(0, 0); N], len: 0 };

    /// Add `n` more copies of `card` (merging into an existing entry for it, if any).
    pub fn add(&mut self, card: CardId, n: u8) {
        for i in 0..self.len as usize {
            if self.entries[i].0 == card {
                self.entries[i].1 = self.entries[i].1.saturating_add(n);
                return;
            }
        }
        let i = self.len as usize;
        assert!(i < N, "SmallMultiset overflow: more than {N} distinct card ids");
        self.entries[i] = (card, n);
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
    pub fn total(&self) -> u32 {
        self.iter().map(|(_, n)| n as u32).sum()
    }
    pub fn clear(&mut self) {
        self.len = 0;
    }
    /// Sum into a `Counts` (for `all_cards`/VP: the mats' cards count as owned).
    pub fn counts(&self) -> Counts {
        let mut c = Counts::EMPTY;
        for (card, n) in self.iter() {
            c.add(card, n);
        }
        c
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
    /// Push/pop-order: index 0 was pushed first. Used for `deck_known_bottom` (Secret Passage),
    /// where `push_top` appends a newly-bottomed card beneath everything already bottomed, and
    /// `pop_front` removes the one nearest the unknown pile (the next to be drawn).
    pub fn iter_front_to_back(&self) -> impl Iterator<Item = CardId> + '_ {
        self.cards[..self.len as usize].iter().copied()
    }
    /// Remove and return the earliest-pushed card (index 0), shifting the rest down. Used only for
    /// `deck_known_bottom`, which is always short in practice (bounded by `KNOWN_CAP`).
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
        assert!((self.len as usize) < KNOWN_CAP, "known deck stack overflow");
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
    /// Sailor only: whether its "once this turn, when you gain a Duration card, you may play
    /// it" reaction has already been used by this copy. Unused (always false) for every other
    /// card.
    pub used: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PlayerState {
    pub hand: Counts,
    pub deck_known: KnownStack,
    pub deck_unknown: Counts,
    /// Known cards under the unknown multiset, at the very bottom of the deck (Secret Passage).
    /// Drawn only once `deck_known` and `deck_unknown` are both exhausted.
    pub deck_known_bottom: KnownStack,
    pub discard: Counts,
    pub in_play: Counts,
    /// Library's set-aside cards (discarded when Library finishes).
    pub set_aside: Counts,
    /// Masquerade's holding zone: the card this player has committed to pass, held until every
    /// passing player has chosen and all the cards move at once. At most one card at a time.
    pub passed: Counts,
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
    /// Every card the player owns, wherever it is.
    pub fn all_cards(&self) -> Counts {
        let mut c = self.deck_counts();
        c.add_all(&self.hand);
        c.add_all(&self.discard);
        c.add_all(&self.in_play);
        c.add_all(&self.set_aside);
        c.add_all(&self.passed);
        c.add_all(&self.native_village_mat.counts());
        c.add_all(&self.island_mat.counts());
        c
    }
    pub fn vp(&self) -> i32 {
        vp_of_cards(&self.all_cards()) + self.vp_tokens as i32
    }
}

/// Victory points of a whole collection of cards (Gardens counts the collection's size, Duke
/// its Duchies).
pub fn vp_of_cards(all: &Counts) -> i32 {
    let total = all.total() as i32;
    all.iter()
        .map(|(c, n)| {
            let per = match c {
                id::GARDENS => total / 10,
                id::DUKE => all.get(id::DUCHY) as i32,
                _ => cards::def(c).vp as i32,
            };
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
    pub named_for_war_chest: Counts,
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
    /// `FrameKind::MultiplierFinalize`. Ported test `TestHavenThroneRoom`/`TestTactitianThroneRoom`
    /// (1st edition, adapted): the multiplier itself stays in play only if *every* resolution
    /// actually scheduled a next-turn effect — for a conditional Duration (Haven with too few
    /// cards, Tactician with an empty hand), a resolution that finds nothing to do does not count,
    /// so the multiplier discards normally even though the target itself may still stay (its
    /// physical copy is tracked separately by `duration_held`, from the original play/pick, not
    /// per resolution).
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
    /// Whether Treasury's end-of-Buy-phase "put this onto your deck?" offer has already been
    /// pushed this turn (guards `GameState::push_treasury_offers` against re-offering to a
    /// still-in-play, already-declined copy every time the effect stack empties out again before
    /// cleanup runs). Per-turn bookkeeping, not part of the text format.
    pub treasury_offered: bool,
    /// Reentrancy guard for Blockade's "gain a copy of the blockaded card -> gain a Curse"
    /// cross-trigger (`GameState::run_seaside_gain_triggers`): true only while processing a
    /// Curse just granted by that trigger, so a Blockaded Curse can't recursively re-trigger
    /// itself. Transient mid-resolution bookkeeping (never a text-format rest state, like
    /// `multiplier_card`): always false at any point the text format could observe.
    pub blockading_curse: bool,
}

impl TurnState {
    pub fn start(player: u8, number: u16) -> Self {
        TurnState {
            player,
            phase: Phase::Action,
            actions: 1,
            buys: 1,
            coins: 0,
            merchants: 0,
            silvers_played: 0,
            number,
            announced: false,
            turn_start_resolved: false,
            played: Counts::EMPTY,
            cost_reduction: 0,
            treasures_done: false,
            named_for_war_chest: Counts::EMPTY,
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
    /// frame was pushed, before the reaction ran.
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
}

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
    /// Resolve one pending start-of-turn Duration effect (Seaside): `source` = the card, `count`
    /// = how many times, `subject` = its argument (Haven/Blockade). Pushed for every entry in
    /// `PlayerState::pending_durations` at the start of the owner's turn (see
    /// `GameState::push_turn_start_frames`), which clears the list up front since every entry's
    /// data is already captured in the pushed frames. See `GameState::resolve_duration_start`.
    DurationStart,
    /// Sits below a Throne Room/King's Court's `times` `PlayEffects` resolutions of a Duration
    /// target (and everything they push): once every one of them has fully unwound, checks
    /// whether all `times` actually scheduled a next-turn effect (`TurnState::multiplier_*`) and,
    /// if so, keeps the multiplier itself (`source`) in play too. See `Then::PlayPicked`.
    MultiplierFinalize,
    /// Native Village's "put the top card of your deck face down on your mat" mode option:
    /// chance-aware like `Draw` (may need a `Step::Chance` sample), but the card goes to the
    /// player's `native_village_mat` instead of their hand.
    NativeVillageAdd,
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
    pub in_supply: u128,
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
        let mut in_supply = 0u128;
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
            // Platinum and Colony (Prosperity) join the supply when the kingdom list names them.
            assert!(cards::is_kingdom(k) || cards::is_optional_basic(k), "{} is not a kingdom card", cards::name(k));
            assert!(cards::is_ready(k), "{} is not implemented yet", cards::name(k));
            let pile = match k {
                id::PLATINUM => 12,
                id::COLONY => province_pile_size(n),
                _ if cards::is(k, cards::VICTORY) => victory_pile_size(n),
                _ => 10,
            };
            put(k, pile);
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

    /// What `c` costs right now: its printed cost less this turn's reductions, min 0. Bridge is a
    /// flat turn-wide reduction; Quarry reduces Action cards while it's in play (stacking per
    /// copy); Peddler reduces only itself, and only during its owner's Buy phase, by $2 per
    /// Action card they have in play. Everything that compares costs (buying, "gain a card
    /// costing up to", Remodel's +$2...) goes through this, never through `cards::cost` directly.
    #[inline(always)]
    pub fn cost(&self, c: CardId) -> u8 {
        let pi = self.turn.player as usize;
        let mut reduction: u32 = self.turn.cost_reduction as u32;
        // `in_supply` check first: a single cheap bitmask test that skips the (still cheap, but
        // not free) `in_play` lookup entirely for the overwhelming majority of games, which have
        // no Quarry in the kingdom at all.
        if self.in_supply(id::QUARRY) && cards::is(c, cards::ACTION) {
            reduction += 2 * self.players[pi].in_play.get(id::QUARRY) as u32;
        }
        if c == id::PEDDLER && self.turn.phase == Phase::Buy {
            reduction += 2 * self.players[pi].in_play.count_type(cards::ACTION);
        }
        (cards::cost(c) as u32).saturating_sub(reduction) as u8
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
        match c {
            id::GRAND_MARKET => !self.players[self.turn.player as usize].in_play.has(id::COPPER),
            _ => true,
        }
    }

    #[inline]
    pub fn in_supply(&self, c: CardId) -> bool {
        self.in_supply & (1u128 << c) != 0
    }

    /// The card ids of this game's supply piles, in id order (walks the `in_supply` bitmask, so
    /// it costs one step per pile rather than one per card id).
    #[inline]
    pub fn supply_cards(&self) -> impl Iterator<Item = CardId> {
        let mut bits = self.in_supply;
        std::iter::from_fn(move || {
            if bits == 0 {
                return None;
            }
            let c = bits.trailing_zeros() as CardId;
            bits &= bits - 1;
            Some(c)
        })
    }

    pub fn empty_piles(&self) -> u32 {
        self.supply_cards().filter(|&c| self.supply.get(c) == 0).count() as u32
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
            Some(EndReason::ProvincesGone) | Some(EndReason::ColoniesGone) | Some(EndReason::PilesEmpty) => {}
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
