//! Snapshots of a running search, for the UI and logs.

use serde::Serialize;

pub use crate::arena::OppRate;

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
    /// The win rate split by opponent, with 95% intervals (`win_rate` above is their weighted
    /// average). Empty when the strategy played a single unnamed pool or the split is unknown.
    #[serde(default)]
    pub vs: Vec<OppRate>,
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
    /// The fixed kingdom the run plays on, as it is set up (names: the kingdom cards, Events,
    /// Landmarks, the Prophecy, Platinum / Colony and Potion when in play). Empty for random
    /// kingdoms and for a fixed track naming no cards. The game page starts on exactly this.
    pub kingdom: Vec<String>,
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
    /// Largest win rate any challenger (ranked strategy or exploiter) reached against it, minus 50%
    /// (at least 0). Near 0: nobody found a way to beat it.
    pub exploitability: f64,
}

/// One round of exploiter search: a best response evolved against the current Nash mix.
#[derive(Clone, Debug, Serialize)]
pub struct ExploiterRound {
    pub round: usize,
    /// The mix it was evolved against: (strategy name, Nash weight) for the support.
    pub mix: Vec<(String, f64)>,
    /// The exploiter's win rate against that mix (full-bot games), with a 95% interval.
    pub rate_vs_mix: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
    pub games: u64,
    /// Its win rate against each member of the mix.
    pub vs: Vec<OppRate>,
    /// It beat the mix by more than the margin and joined the population.
    pub added: bool,
    /// Its name in the crosstable (when added).
    pub name: Option<String>,
    pub summary: String,
    /// The Nash support after the round (the same as before when it was not added).
    pub nash_after: Vec<(String, f64)>,
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
    /// Exploiter rounds run after the round robin (empty when switched off).
    pub exploiters: Vec<ExploiterRound>,
    /// Rounds asked for (`exploit_rounds`).
    pub exploit_rounds: usize,
    /// Per crosstable row: the strongest challenger's win rate against it minus 50% (at least 0).
    pub exploitability: Vec<f64>,
    /// The last exploiter's win rate against the Nash mix of its round, minus 50%: how much the
    /// mix could still be beaten (negative: the exploiter lost). `None` without exploiter rounds.
    pub nash_exploitability: Option<f64>,
}

impl Progress {
    pub fn note(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }
}
