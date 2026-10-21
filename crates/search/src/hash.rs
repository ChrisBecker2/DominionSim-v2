//! A fast, allocation-free hash over only the turn-relevant slice of `GameState`, used to key
//! the transposition table. `GameState` derives `Hash`, but hashing it directly would walk all
//! `MAX_PLAYERS` (6) player slots (~1.8 KB) when only the searching player's own zones matter:
//! opponents' hands never affect *my* future choices or the evaluator's score of a leaf reached
//! on my turn (my cards only ever branch on my own zones; attacks are resolved by a fixed
//! opponent policy, not searched). So we hash: my own `PlayerState` fields, the shared turn
//! counters, the supply (buy legality depends on it), and the effect stack (it determines what
//! happens next). Trash and opponents' zones are deliberately excluded.

use dominion_engine::counts::Counts;
use dominion_engine::state::{FrameStack, KnownStack, PlayerState};
use dominion_engine::{GameState, Phase};
use std::hash::{Hash, Hasher};

/// The FxHash algorithm (as used in `rustc`/`firefox`): simple, fast, no dependency needed.
pub struct FxHasher(u64);

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    #[inline]
    pub fn new() -> Self {
        FxHasher(0)
    }
}

impl Default for FxHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let w = u64::from_le_bytes(chunk.try_into().unwrap());
            h = (h.rotate_left(5) ^ w).wrapping_mul(SEED);
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rem.len()].copy_from_slice(rem);
            let w = u64::from_le_bytes(buf);
            h = (h.rotate_left(5) ^ w).wrapping_mul(SEED);
        }
        self.0 = h;
    }
}

#[inline]
fn hash_known_stack<const N: usize>(ks: &KnownStack<N>, h: &mut FxHasher) {
    // Only the meaningful prefix, not the full 120-byte fixed array.
    ks.len.hash(h);
    ks.cards[..ks.len as usize].hash(h);
}

#[inline]
fn hash_stack(stack: &FrameStack, h: &mut FxHasher) {
    stack.len.hash(h);
    for f in stack.iter() {
        f.hash(h);
    }
}

/// The chain of `turn_hash` up to and including the searching player's undrawn deck (`me`, hand, known deck cards, unknown deck), for
/// the last state hashed. The search hashes a node's children one after another, and most of them (every Buy that gains a card, every
/// choice that changes only the coins) agree with the previous state on those three zones, so the hash resumes from the stored chain
/// instead of walking the zones again. The chain is a pure function of the zones, so the hash is the one `turn_hash` computes.
pub struct HashCache {
    valid: bool,
    me: u8,
    hand: Counts,
    deck_known: KnownStack,
    deck_unknown: Counts,
    /// The hasher's state after `me`, `hand`, `deck_known` and `deck_unknown`.
    chain: u64,
}

impl Default for HashCache {
    fn default() -> Self {
        HashCache { valid: false, me: 0, hand: Counts::EMPTY, deck_known: KnownStack::default(), deck_unknown: Counts::EMPTY, chain: 0 }
    }
}

impl HashCache {
    /// `turn_hash(state, me)`.
    pub fn turn_hash(&mut self, state: &GameState, me: u8) -> u64 {
        let ps = &state.players[me as usize];
        let known_len = ps.deck_known.len as usize;
        let same = self.valid
            && self.me == me
            && self.deck_known.len == ps.deck_known.len
            && self.deck_known.cards[..known_len] == ps.deck_known.cards[..known_len]
            && self.hand == ps.hand
            && self.deck_unknown == ps.deck_unknown;
        if !same {
            *self = HashCache { valid: true, me, hand: ps.hand, deck_known: ps.deck_known, deck_unknown: ps.deck_unknown, chain: chain_start(ps, me).0 };
        }
        finish_turn_hash(state, ps, FxHasher(self.chain))
    }
}

/// The first part of `turn_hash`: `me`, hand, known deck cards and unknown deck.
#[inline]
fn chain_start(ps: &PlayerState, me: u8) -> FxHasher {
    let mut h = FxHasher::new();
    me.hash(&mut h);
    ps.hand.hash(&mut h);
    hash_known_stack(&ps.deck_known, &mut h);
    ps.deck_unknown.hash(&mut h);
    h
}

/// The rest of `turn_hash`, after the chain of `HashCache`.
#[inline]
fn finish_turn_hash(state: &GameState, ps: &PlayerState, mut h: FxHasher) -> u64 {
    ps.discard.hash(&mut h);
    ps.in_play.hash(&mut h);
    ps.set_aside.hash(&mut h);
    // Adventures: the Tavern mat (calls), tokens, the Journey and -1 Card / -$1 flags, Inheritance, Save.
    ps.tavern.hash(&mut h);
    (ps.tok, ps.adv, ps.inherit, ps.saved).hash(&mut h);
    // Renaissance: Villagers, Projects, what is set aside, the Artifacts (nothing is hashed while all of it is zero).
    ps.ren.hash(&mut h);
    state.ren.hash(&mut h);
    // Menagerie: the Exile mat and what a player's cards remember, the Ways and Way of the Mouse's card (nothing is hashed while all of it is zero).
    ps.men.hash(&mut h);
    state.men.hash(&mut h);
    state.turn.hash(&mut h);
    state.supply.hash(&mut h);
    (state.spoils, state.madman, state.mercenary, state.trav, state.adv).hash(&mut h);
    state.noc.hash(&mut h);
    hash_stack(&state.stack, &mut h);
    // Phase is already inside `turn`, but a defensive explicit tag costs nothing and guards
    // against ever changing what `turn`'s Hash impl covers.
    (state.turn.phase == Phase::CleanupDraw).hash(&mut h);
    h.finish()
}

/// Hash of the part of `state` that determines the value of the subtree rooted here, from
/// `me`'s point of view: my own zones, the turn counters, the supply, and the pending effect
/// stack. Two states with the same hash are treated as the same search node (collisions are
/// accepted as negligible at 64 bits for the node counts this search runs).
pub fn turn_hash(state: &GameState, me: u8) -> u64 {
    let ps = &state.players[me as usize];
    finish_turn_hash(state, ps, chain_start(ps, me))
}
