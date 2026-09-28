//! Fitness: a candidate plays head-to-head against each opponent in a pool, on each kingdom of
//! a scenario set. Seeds depend only on (round, scenario, opponent), never on the candidate, so
//! all candidates in a round face exactly the same deals (common random numbers).

use dominion_engine::cards::CardId;
use dominion_engine::rng::Rng;
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::stats::wilson_ci;
use dominion_sim::Strategy;

use crate::genome::{Genome, TomlMeta};

/// One kingdom to play on.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub kingdom: Vec<CardId>,
}

/// Accumulated results. `wins`/`games` are opponent-weighted (ties count as a fractional win).
#[derive(Clone, Copy, Debug, Default)]
pub struct Score {
    pub wins: f64,
    pub games: f64,
    /// Games actually played.
    pub played: u64,
}

impl Score {
    pub fn win_rate(&self) -> f64 {
        if self.games > 0.0 { self.wins / self.games } else { 0.0 }
    }
    /// 95% Wilson interval (treating the weighted counts as plain counts).
    pub fn ci95(&self) -> (f64, f64) {
        wilson_ci(self.wins, self.games, dominion_sim::stats::Z_95)
    }
    pub fn add(&mut self, o: &Score) {
        self.wins += o.wins;
        self.games += o.games;
        self.played += o.played;
    }
}

/// Apply an arena's play mode to a strategy (fast mode turns off both turn searches).
pub fn set_fast(s: &mut Strategy, fast: bool) {
    if fast {
        s.search_play = false;
        s.win_this_turn = false;
    }
}

pub struct Opponent {
    pub name: String,
    pub strategy: Strategy,
    pub weight: f64,
}

pub struct Arena {
    pub opponents: Vec<Opponent>,
    /// Written into every candidate as `never_gain`.
    pub forbidden: Vec<CardId>,
    /// Fast mode: actions are played in rule order (no turn search) and there is no
    /// `win_this_turn` lookahead. Several times faster, slightly weaker endgames. Full mode plays
    /// exactly like the bots in the game UI. Applies to candidates and opponents alike.
    pub fast: bool,
    pub max_turns: u16,
}

impl Arena {
    /// Compile a genome into a runnable strategy with this arena's settings.
    pub fn compile(&self, g: &Genome) -> Result<Strategy, String> {
        let meta = TomlMeta { name: "Candidate".into(), never_gain: self.forbidden.clone(), ..Default::default() };
        let mut s = Strategy::parse(&g.to_toml(&meta))?;
        set_fast(&mut s, self.fast);
        Ok(s)
    }

    /// Play `games` games (split evenly over scenarios and opponents, rounded up to an even number
    /// per pairing so seats balance) with seeds derived from `round_seed`.
    pub fn evaluate(&self, candidate: &Strategy, scenarios: &[Scenario], games: u64, round_seed: u64) -> Score {
        let pairings = (scenarios.len() * self.opponents.len()).max(1) as u64;
        let per = (games.div_ceil(pairings) + 1) & !1;
        let mut total = Score::default();
        for (k, sc) in scenarios.iter().enumerate() {
            for (o, opp) in self.opponents.iter().enumerate() {
                let cfg = MatchConfig {
                    games: per,
                    kingdom: sc.kingdom.clone(),
                    seed: Rng::derive(round_seed, (k as u64) << 16 | o as u64).next_u64(),
                    max_turns: self.max_turns,
                };
                let stats = run_match(&[candidate, &opp.strategy], &cfg);
                total.wins += stats[0].wins * opp.weight;
                total.games += stats[0].games as f64 * opp.weight;
                total.played += stats[0].games;
            }
        }
        total
    }
}
