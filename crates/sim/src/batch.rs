//! Massively parallel batch simulation over independent games (rayon).
//!
//! Each game is fully independent; its seed is derived from `(base_seed, game_index)` via
//! `dominion_engine::rng::Rng::derive`, so results are reproducible regardless of thread count
//! or chunking. Per-thread work is folded into a local `[StratStats; MAX_PLAYERS]` accumulator
//! (no locks/atomics in the per-game loop) and reduced once at the end. Seats rotate every
//! `players` games so each strategy plays every seat equally (this also gives the strategies
//! common random numbers: `game_index / players` is the same underlying seed across a
//! rotation's `players` games).

use dominion_engine::cards::CardId;
use dominion_engine::rng::Rng;
use dominion_engine::state::MAX_PLAYERS;
use dominion_engine::{play_game, Agent, GameConfig, GameState, NoEvents};
use rayon::prelude::*;

use crate::agent::StrategyAgent;
use crate::stats::StratStats;
use crate::strategy::Strategy;

pub struct MatchConfig {
    pub games: u64,
    pub kingdom: Vec<CardId>,
    pub seed: u64,
    pub max_turns: u16,
}

/// Play `strategies.len()` strategies against each other (one per seat) for `cfg.games` games,
/// rotating seats. Returns one `StratStats` per input strategy, in input order.
pub fn run_match(strategies: &[&Strategy], cfg: &MatchConfig) -> Vec<StratStats> {
    let p = strategies.len();
    assert!((2..=MAX_PLAYERS).contains(&p), "match needs 2..={MAX_PLAYERS} strategies, got {p}");

    let base_seed = cfg.seed;
    let base_cfg = GameConfig { num_players: p, kingdom: cfg.kingdom.clone(), seed: 0, max_turns: cfg.max_turns };

    let totals: [StratStats; MAX_PLAYERS] = (0..cfg.games)
        .into_par_iter()
        .fold(
            || (base_cfg.clone(), [StratStats::default(); MAX_PLAYERS]),
            |(mut gcfg, mut acc), g| {
                let rotation = (g % p as u64) as usize;
                gcfg.seed = Rng::derive(base_seed, g / p as u64).next_u64();
                let mut state = GameState::new(&gcfg);

                // Build `p` agents on the stack, then split them into disjoint `&mut dyn Agent`
                // slots. No heap allocation: `agents` is a fixed-size array, not a Vec.
                let mut agents: [StrategyAgent; MAX_PLAYERS] = std::array::from_fn(|i| {
                    let idx = if i < p { (i + rotation) % p } else { 0 };
                    StrategyAgent::new(strategies[idx])
                });
                let mut refs: [&mut dyn Agent; MAX_PLAYERS] = {
                    let mut it = agents.iter_mut();
                    std::array::from_fn(|_| it.next().unwrap() as &mut dyn Agent)
                };

                let result = play_game(&mut state, &mut refs[..p], &mut NoEvents);
                let win_share = 1.0 / result.winners.count_ones().max(1) as f64;
                for i in 0..p {
                    let strat_idx = (i + rotation) % p;
                    let s = &mut acc[strat_idx];
                    s.games += 1;
                    if result.winners & (1 << i) != 0 {
                        s.wins += win_share;
                    }
                    s.vp_sum += result.scores[i] as f64;
                    s.turns_sum += result.turns[i] as f64;
                    if result.capped {
                        s.capped_games += 1;
                    }
                }
                (gcfg, acc)
            },
        )
        .map(|(_, acc)| acc)
        .reduce(
            || [StratStats::default(); MAX_PLAYERS],
            |mut a, b| {
                for i in 0..MAX_PLAYERS {
                    a[i].merge(&b[i]);
                }
                a
            },
        );

    totals[..p].to_vec()
}

/// One row of a round-robin league table: `a` vs `b`, from `a`'s perspective.
pub struct LeagueCell {
    pub a: usize,
    pub b: usize,
    pub a_stats: StratStats,
    pub b_stats: StratStats,
}

/// Round-robin: every unordered pair of `strategies` plays a 2-player match of `games_per_match`
/// games. Pairs run in parallel too (via rayon), each pair's games also parallelized internally.
pub fn run_league(strategies: &[&Strategy], games_per_match: u64, kingdom: &[CardId], seed: u64, max_turns: u16) -> Vec<LeagueCell> {
    let n = strategies.len();
    let mut pairs = Vec::with_capacity(n * (n - 1) / 2);
    for i in 0..n {
        for j in (i + 1)..n {
            pairs.push((i, j));
        }
    }
    pairs
        .into_par_iter()
        .map(|(i, j)| {
            // Distinct seed per pair so different pairs don't share random streams.
            let pair_seed = Rng::derive(seed, (i as u64) << 32 | j as u64).next_u64();
            let cfg = MatchConfig { games: games_per_match, kingdom: kingdom.to_vec(), seed: pair_seed, max_turns };
            let stats = run_match(&[strategies[i], strategies[j]], &cfg);
            LeagueCell { a: i, b: j, a_stats: stats[0], b_stats: stats[1] }
        })
        .collect()
}
