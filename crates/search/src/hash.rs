//! A fast, allocation-free hash over only the turn-relevant slice of `GameState`, used to key
//! the transposition table. `GameState` derives `Hash`, but hashing it directly would walk all
//! `MAX_PLAYERS` (6) player slots (~1.8 KB) when only the searching player's own zones matter:
//! opponents' hands never affect *my* future choices or the evaluator's score of a leaf reached
//! on my turn (my cards only ever branch on my own zones; attacks are resolved by a fixed
//! opponent policy, not searched). So we hash: my own `PlayerState` fields, the shared turn
//! counters, the supply (buy legality depends on it), and the effect stack (it determines what
//! happens next). Trash and opponents' zones are deliberately excluded.

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
fn hash_known_stack(ks: &KnownStack, h: &mut FxHasher) {
    // Only the meaningful prefix, not the full 120-byte fixed array.
    ks.len.hash(h);
    ks.cards[..ks.len as usize].hash(h);
}

#[inline]
fn hash_player(ps: &PlayerState, h: &mut FxHasher) {
    ps.hand.hash(h);
    hash_known_stack(&ps.deck_known, h);
    ps.deck_unknown.hash(h);
    ps.discard.hash(h);
    ps.in_play.hash(h);
    ps.set_aside.hash(h);
}

#[inline]
fn hash_stack(stack: &FrameStack, h: &mut FxHasher) {
    stack.len.hash(h);
    for f in stack.iter() {
        f.hash(h);
    }
}

/// Hash of the part of `state` that determines the value of the subtree rooted here, from
/// `me`'s point of view: my own zones, the turn counters, the supply, and the pending effect
/// stack. Two states with the same hash are treated as the same search node (collisions are
/// accepted as negligible at 64 bits for the node counts this search runs).
pub fn turn_hash(state: &GameState, me: u8) -> u64 {
    let mut h = FxHasher::new();
    me.hash(&mut h);
    hash_player(&state.players[me as usize], &mut h);
    state.turn.hash(&mut h);
    state.supply.hash(&mut h);
    hash_stack(&state.stack, &mut h);
    // Phase is already inside `turn`, but a defensive explicit tag costs nothing and guards
    // against ever changing what `turn`'s Hash impl covers.
    (state.turn.phase == Phase::CleanupDraw).hash(&mut h);
    h.finish()
}
