//! Final ranking: a round robin between strategies on one kingdom (common random numbers: every
//! pair plays the same deals), a crosstable with confidence intervals, each strategy's mean
//! against the field, and an approximate Nash mix of the payoff matrix.

use dominion_engine::cards::CardId;
use dominion_sim::batch::{run_match_profiled, MatchConfig, Profile};
use dominion_sim::stats::{wilson_ci, Z_95};
use dominion_sim::Strategy;
use dominion_engine::rng::Rng;
use std::sync::atomic::{AtomicBool, Ordering};
use serde::Serialize;

/// Results of a round robin. `rate[i][j]` is how often `i` beats `j` (ties count half; the
/// diagonal is 0.5), `lo`/`hi` its 95% interval.
#[derive(Clone, Debug, Serialize)]
pub struct Crosstable {
    pub names: Vec<String>,
    pub rate: Vec<Vec<f64>>,
    pub lo: Vec<Vec<f64>>,
    pub hi: Vec<Vec<f64>>,
    /// Games per pair.
    pub games: u64,
    /// Mean of `rate[i][j]` over the other strategies `j`.
    pub mean_vs_field: Vec<f64>,
    /// Approximate Nash mix over the strategies (sums to 1).
    pub nash: Vec<f64>,
}

/// Everything a round robin measured, besides the crosstable itself.
pub struct RoundRobin {
    pub table: Crosstable,
    /// Each strategy's profile and own turns, pooled over its games against all the others.
    pub profiles: Vec<Profile>,
    pub turns_sum: Vec<f64>,
    pub games_played: Vec<u64>,
}

/// Games per batch in `round_robin`: between batches it checks `stop` and reports progress, so a
/// slow round robin (full searching bots on a choice-heavy kingdom) can be watched and stopped.
const RR_BATCH: u64 = 250;

/// Every pair plays `games` games (seats balanced) on `kingdom` with the same seeds, so all pairs
/// see the same deals. Strategies are used as given (callers decide fast or full behaviour). Pairs
/// are played one after another, each in batches of `RR_BATCH` games run in parallel; after each
/// batch `progress(pairs_done, pairs_total)` is called and `stop` checked. `None` when stopped.
pub fn round_robin(
    strategies: &[&Strategy],
    names: &[String],
    games: u64,
    kingdom: &[CardId],
    seed: u64,
    max_turns: u16,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(usize, usize),
) -> Option<RoundRobin> {
    let n = strategies.len();
    let games = (games + 1) & !1;
    let mut pairs = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            pairs.push((i, j));
        }
    }
    let total = pairs.len();
    let mut results: Vec<(usize, usize, dominion_sim::stats::StratStats, dominion_sim::stats::StratStats, Profile, Profile)> = Vec::new();
    for (k, &(i, j)) in pairs.iter().enumerate() {
        let (mut si, mut sj) = Default::default();
        let (mut pi, mut pj) = (Profile::default(), Profile::default());
        let mut played = 0u64;
        let mut batch = 0u64;
        while played < games {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            let n_games = RR_BATCH.min(games - played);
            // Each batch has its own seed, the same for every pair: all pairs still see the same deals.
            let cfg = MatchConfig { games: n_games, kingdom: kingdom.to_vec(), seed: Rng::derive(seed, batch).next_u64(), max_turns };
            let (s, mut p) = run_match_profiled(&[strategies[i], strategies[j]], &cfg);
            let bj = p.pop().unwrap_or_default();
            let bi = p.pop().unwrap_or_default();
            dominion_sim::stats::StratStats::merge(&mut si, &s[0]);
            dominion_sim::stats::StratStats::merge(&mut sj, &s[1]);
            pi.merge(&bi);
            pj.merge(&bj);
            played += n_games;
            batch += 1;
        }
        results.push((i, j, si, sj, pi, pj));
        progress(k + 1, total);
    }
    let mut rate = vec![vec![0.5; n]; n];
    let mut lo = vec![vec![0.5; n]; n];
    let mut hi = vec![vec![0.5; n]; n];
    let mut profiles = vec![Profile::default(); n];
    let mut turns_sum = vec![0.0; n];
    let mut games_played = vec![0u64; n];
    for (i, j, si, sj, pi, pj) in results {
        let g = si.games as f64;
        let (l, h) = wilson_ci(si.wins, g, Z_95);
        rate[i][j] = si.win_rate();
        rate[j][i] = sj.win_rate();
        lo[i][j] = l;
        hi[i][j] = h;
        lo[j][i] = 1.0 - h;
        hi[j][i] = 1.0 - l;
        profiles[i].merge(&pi);
        profiles[j].merge(&pj);
        turns_sum[i] += si.turns_sum;
        turns_sum[j] += sj.turns_sum;
        games_played[i] += si.games;
        games_played[j] += sj.games;
    }
    let mean_vs_field = mean_vs_field(&rate);
    let payoff: Vec<Vec<f64>> = rate.iter().map(|r| r.iter().map(|x| x - 0.5).collect()).collect();
    let nash = nash_mix(&payoff, 20_000);
    Some(RoundRobin { table: Crosstable { names: names.to_vec(), rate, lo, hi, games, mean_vs_field, nash }, profiles, turns_sum, games_played })
}

/// Each row's mean over the other columns (1.0 / 0 for a single strategy: 0.5).
pub fn mean_vs_field(rate: &[Vec<f64>]) -> Vec<f64> {
    let n = rate.len();
    (0..n)
        .map(|i| if n < 2 { 0.5 } else { (0..n).filter(|&j| j != i).map(|j| rate[i][j]).sum::<f64>() / (n - 1) as f64 })
        .collect()
}

/// An approximate Nash equilibrium mix of the symmetric zero-sum game with payoff matrix
/// `a[i][j]` (what `i` gets against `j`, antisymmetric: win rate minus one half). Multiplicative
/// weights, averaged over the iterates: for rock-paper-scissors it returns one third each.
pub fn nash_mix(a: &[Vec<f64>], iters: usize) -> Vec<f64> {
    let n = a.len();
    if n == 0 {
        return Vec::new();
    }
    let mut x = vec![1.0 / n as f64; n];
    let mut avg = vec![0.0; n];
    let eta = 0.5 / (1.0f64).max(a.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs())) * 2.0);
    for t in 0..iters {
        let u: Vec<f64> = (0..n).map(|i| (0..n).map(|j| a[i][j] * x[j]).sum()).collect();
        // Decaying step: large moves first, fine adjustments later.
        let step = eta * 8.0 / (1.0 + t as f64 / 200.0).sqrt();
        let mut z = 0.0;
        for i in 0..n {
            x[i] *= (step * u[i]).exp();
            z += x[i];
        }
        for i in 0..n {
            x[i] = (x[i] / z).max(1e-12);
            avg[i] += x[i];
        }
    }
    let s: f64 = avg.iter().sum();
    avg.iter().map(|v| v / s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rock_paper_scissors_mixes_one_third_each() {
        // rock beats scissors, scissors beats paper, paper beats rock.
        let a = vec![vec![0.0, -0.5, 0.5], vec![0.5, 0.0, -0.5], vec![-0.5, 0.5, 0.0]];
        let m = nash_mix(&a, 20_000);
        for v in &m {
            assert!((v - 1.0 / 3.0).abs() < 0.02, "{m:?}");
        }
        assert!((m.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_dominated_strategy_gets_no_weight_and_a_dominant_one_all_of_it() {
        // 0 beats 1 beats 2, and 0 beats 2.
        let a = vec![vec![0.0, 0.2, 0.3], vec![-0.2, 0.0, 0.1], vec![-0.3, -0.1, 0.0]];
        let m = nash_mix(&a, 20_000);
        assert!(m[0] > 0.95, "{m:?}");
        // Rock-paper-scissors plus a strictly worse fourth strategy.
        let b = vec![
            vec![0.0, -0.5, 0.5, 0.4],
            vec![0.5, 0.0, -0.5, 0.4],
            vec![-0.5, 0.5, 0.0, 0.4],
            vec![-0.4, -0.4, -0.4, 0.0],
        ];
        let m = nash_mix(&b, 20_000);
        assert!(m[3] < 0.02, "{m:?}");
        assert!((m[0] - 1.0 / 3.0).abs() < 0.03, "{m:?}");
    }

    #[test]
    fn mean_vs_field_ignores_the_diagonal() {
        let rate = vec![vec![0.5, 0.7, 0.9], vec![0.3, 0.5, 0.6], vec![0.1, 0.4, 0.5]];
        let m = mean_vs_field(&rate);
        assert!((m[0] - 0.8).abs() < 1e-9 && (m[1] - 0.45).abs() < 1e-9 && (m[2] - 0.25).abs() < 1e-9);
    }
}
