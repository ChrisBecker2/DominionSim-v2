//! Win-rate statistics: Wilson score confidence intervals and per-strategy accumulators.

/// Wilson score interval for a binomial proportion `wins/n` at confidence level implied by `z`
/// (1.959964 for 95%). Returns `(lo, hi)` in `[0, 1]`. `n == 0` returns `(0.0, 1.0)`.
pub fn wilson_ci(wins: f64, n: f64, z: f64) -> (f64, f64) {
    if n <= 0.0 {
        return (0.0, 1.0);
    }
    let p = wins / n;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = p + z2 / (2.0 * n);
    let adj = z * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).max(0.0).sqrt();
    (((center - adj) / denom).clamp(0.0, 1.0), ((center + adj) / denom).clamp(0.0, 1.0))
}

pub const Z_95: f64 = 1.959_964;

/// Accumulated results for one strategy within a match or league cell. Ties are counted as a
/// fractional win share (1/number of winners).
#[derive(Clone, Copy, Default)]
pub struct StratStats {
    pub games: u64,
    pub wins: f64,
    pub vp_sum: f64,
    pub turns_sum: f64,
    pub capped_games: u64,
}

impl StratStats {
    pub fn merge(&mut self, other: &StratStats) {
        self.games += other.games;
        self.wins += other.wins;
        self.vp_sum += other.vp_sum;
        self.turns_sum += other.turns_sum;
        self.capped_games += other.capped_games;
    }

    pub fn win_rate(&self) -> f64 {
        if self.games == 0 { 0.0 } else { self.wins / self.games as f64 }
    }
    pub fn avg_vp(&self) -> f64 {
        if self.games == 0 { 0.0 } else { self.vp_sum / self.games as f64 }
    }
    pub fn avg_turns(&self) -> f64 {
        if self.games == 0 { 0.0 } else { self.turns_sum / self.games as f64 }
    }
    pub fn wilson_ci_95(&self) -> (f64, f64) {
        wilson_ci(self.wins, self.games as f64, Z_95)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_narrows_with_more_games() {
        let (lo1, hi1) = wilson_ci(50.0, 100.0, Z_95);
        let (lo2, hi2) = wilson_ci(5000.0, 10000.0, Z_95);
        assert!(hi1 - lo1 > hi2 - lo2);
        assert!(lo1 < 0.5 && hi1 > 0.5);
    }

    #[test]
    fn wilson_handles_extremes() {
        let (lo, hi) = wilson_ci(100.0, 100.0, Z_95);
        assert!(hi >= 0.999 && hi <= 1.0);
        assert!(lo > 0.9);
        let (lo0, _) = wilson_ci(0.0, 100.0, Z_95);
        assert!(lo0 < 0.01);
    }
}
