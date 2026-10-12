//! `GameState` is copied constantly (every search node, every bot rollout) and its size is a direct cost: Adventures grew it from 14 KB to
//! 20 KB and made whole games 18% slower until the zones that are empty in nearly every game were made small (see "Keeping GameState
//! small" in `docs/adding-a-set.md`). These bounds fail when a new field makes it grow again; raise one only deliberately, after
//! looking for a `SmallMultiset` instead of a `Counts` (288 bytes) for the new zone.

use dominion_engine::counts::Counts;
use dominion_engine::state::{Frame, GameState, PlayerState, TurnState};
use std::mem::size_of;

#[test]
fn game_state_stays_small() {
    let sizes = [
        ("GameState", size_of::<GameState>(), 19_700),
        ("PlayerState", size_of::<PlayerState>(), 2_600),
        ("TurnState", size_of::<TurnState>(), 800),
        ("Frame", size_of::<Frame>(), 48),
        ("Counts", size_of::<Counts>(), 352),
    ];
    for (name, size, limit) in sizes {
        assert!(size <= limit, "{name} is {size} bytes, over its {limit} byte bound: see tests/state_size.rs");
    }
}
