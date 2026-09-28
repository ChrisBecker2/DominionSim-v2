//! Snapshots of a running search, for the UI and logs.

use serde::Serialize;

/// A strategy found by the search, with its measured strength.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    /// The strategy file (with a header describing how it was found).
    pub toml: String,
    /// One-line rule summary.
    pub summary: String,
    pub win_rate: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
    pub games: u64,
    /// Rules + conditions.
    pub size: usize,
    pub fitness: f64,
    pub generation: usize,
    /// Measured on held-out seeds (rather than the seeds it was selected on).
    pub validated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GenStat {
    pub generation: usize,
    pub best_fitness: f64,
    pub best_win_rate: f64,
    pub median_win_rate: f64,
    /// The champion's validated win rate, when validated this generation.
    pub validated_win_rate: Option<f64>,
    pub best_summary: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Progress {
    /// "starting", "running", "validating", "polishing", "stopped", "done" or "error".
    pub status: String,
    pub generation: usize,
    pub generations: usize,
    pub games_played: u64,
    pub games_per_sec: f64,
    pub elapsed_secs: f64,
    pub history: Vec<GenStat>,
    /// Best candidate of the latest generation.
    pub champion: Option<Entry>,
    /// Best validated strategies so far (best first, distinct).
    pub hall_of_fame: Vec<Entry>,
    /// The polished final strategy, when the run finishes.
    pub result: Option<Entry>,
    /// Recent messages (newest last).
    pub log: Vec<String>,
}

impl Progress {
    pub fn note(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }
}
