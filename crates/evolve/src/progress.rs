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
    /// League mode: niches held and hall-of-fame size after this generation.
    pub niches: usize,
    pub hall: usize,
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
    /// Kingdom league mode: anchors, niche champions, final crosstable.
    pub league: Option<LeagueReport>,
    /// Recent messages (newest last).
    pub log: Vec<String>,
}

/// A baseline strategy that anchors the league.
#[derive(Clone, Debug, Serialize)]
pub struct AnchorInfo {
    pub name: String,
    pub description: String,
    pub toml: String,
}

/// The champion of one niche.
#[derive(Clone, Debug, Serialize)]
pub struct NicheRow {
    pub label: String,
    pub key_cards: Vec<String>,
    pub trash: String,
    pub length: String,
    /// "Laboratory 3.2, Mine 1.1; 4.0 trashes; 17.3 turns".
    pub behaviour: String,
    /// Win rate is against the league of the generation it was measured in.
    pub entry: Entry,
    pub island: usize,
    pub generation: usize,
}

/// One line of the final presentation.
#[derive(Clone, Debug, Serialize)]
pub struct TopRow {
    pub rank: usize,
    /// Index into the crosstable.
    pub index: usize,
    pub name: String,
    pub description: String,
    pub baseline: bool,
    pub mean_vs_field: f64,
    pub nash: f64,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct LeagueReport {
    pub anchors: Vec<AnchorInfo>,
    /// Niche champions, fittest first (updated every generation).
    pub niches: Vec<NicheRow>,
    /// The opponents of the latest generation's league.
    pub league_now: Vec<String>,
    /// Average game length of Big Money Ultimate here (the reference for the length buckets).
    pub reference_turns: f64,
    /// Final round robin (anchors then niche champions), when finished.
    pub crosstable: Option<crate::rank::Crosstable>,
    /// Per crosstable row: the strategy file and its mean win rate against the field.
    pub strategies: Vec<Entry>,
    /// Per crosstable row: "anchor" or "champion".
    pub kinds: Vec<String>,
    /// The best distinct strategies by mean against the field, with their Nash weight.
    pub top: Vec<TopRow>,
}

impl Progress {
    pub fn note(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }
}
