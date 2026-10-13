//! `GameState` is copied constantly (every search node, every bot rollout) and its size is a direct cost: Adventures grew it from 14 KB to
//! 20 KB and made whole games 18% slower until the zones that are empty in nearly every game were made small (see "Keeping GameState
//! small" in `docs/adding-a-set.md`). These bounds fail when a new field makes it grow again; raise one only deliberately, after
//! looking for a `SmallMultiset` instead of a `Counts` (384 bytes) for the new zone. What a 2-player game copies (`GameState::dup`: the
//! struct without the empty seats) is bounded too.

use dominion_engine::counts::Counts;
use dominion_engine::state::{Frame, GameState, PlayerState, TurnState};
use dominion_engine::{ChoiceBuf, GameConfig, NoEvents, Step};
use std::mem::size_of;

#[test]
fn game_state_stays_small() {
    let sizes = [
        ("GameState", size_of::<GameState>(), 22_500),
        ("PlayerState", size_of::<PlayerState>(), 3_000),
        ("TurnState", size_of::<TurnState>(), 950),
        ("Frame", size_of::<Frame>(), 48),
        ("Counts", size_of::<Counts>(), 416),
    ];
    for (name, size, limit) in sizes {
        assert!(size <= limit, "{name} is {size} bytes, over its {limit} byte bound: see tests/state_size.rs");
    }
}

fn game(players: usize, seed: u64) -> GameState {
    GameState::new(&GameConfig { num_players: players, seed, ..Default::default() })
}

#[test]
fn a_copy_of_a_two_player_game_leaves_out_the_empty_seats() {
    let g = game(2, 1);
    let copied = g.dup_bytes();
    assert!(copied <= 10_600, "a 2-player copy is {copied} bytes (the struct is {}), over its bound: see tests/state_size.rs", size_of::<GameState>());
    assert_eq!(game(6, 1).dup_bytes(), size_of::<GameState>(), "a 6-player copy is the whole struct");
    for n in 2..=6 {
        assert_eq!(game(n, 1).dup_bytes() - g.dup_bytes(), (n - 2) * size_of::<PlayerState>());
    }
}

/// `dup` is `*self` for the part of the state that matters: equal to the original, and playing on from it does exactly what the original does.
#[test]
fn a_dup_plays_on_like_the_original() {
    for n in 2..=6 {
        for seed in 0..4 {
            let mut a = game(n, seed);
            let mut buf = ChoiceBuf::default();
            let mut steps = 0;
            let mut b = a.dup();
            loop {
                assert!(a == b, "{n} players, seed {seed}, step {steps}: the copy differs");
                let (sa, sb) = (a.advance(&mut NoEvents), b.advance(&mut NoEvents));
                assert_eq!(sa, sb);
                match sa {
                    Step::Decision(_) => {
                        a.legal_choices(&mut buf);
                        let c = buf.as_slice()[(steps * 7 + seed as usize) % buf.len()];
                        a.apply(c, &mut NoEvents).unwrap();
                        b.apply(c, &mut NoEvents).unwrap();
                    }
                    Step::GameOver => break,
                    _ => {}
                }
                steps += 1;
                // Copy again now and then: the copy of a copy.
                if steps % 50 == 0 {
                    b = b.dup();
                }
            }
            assert_eq!(a.scores(), b.scores());
        }
    }
}
