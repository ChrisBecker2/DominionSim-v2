//! Run configuration (serializable, so the Lab UI and the CLI share one format).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Track {
    /// One fixed kingdom (e.g. every card available): the strongest answer for that kingdom.
    Fixed { kingdom: Vec<String> },
    /// Fresh random 10-card kingdoms each generation, always containing `required` (e.g. Witch),
    /// drawn from the card pool: a strategy that plays well across kingdoms.
    Random { required: Vec<String>, kingdoms_per_generation: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OpponentSpec {
    pub name: String,
    /// Strategy TOML source.
    pub toml: String,
    #[serde(default = "one")]
    pub weight: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EvolveConfig {
    pub track: Track,
    /// Which expansions' kingdom cards are in play: "Base", "Intrigue", or both. Governs the
    /// default (empty) fixed kingdom, the random track's draw pool, and so the search space.
    pub sets: Vec<String>,
    pub opponents: Vec<OpponentSpec>,
    /// Cards candidates may never gain.
    pub forbidden: Vec<String>,
    /// Strategy TOML sources to seed the population with (ones using forbidden cards are
    /// stripped of those rules; unsupported conditions are skipped).
    pub seeds: Vec<String>,
    pub islands: usize,
    pub island_size: usize,
    pub generations: usize,
    /// Stop early after this many generations without a new best (0 = never).
    pub stall_generations: usize,
    /// Best candidates copied unchanged into the next generation, per island.
    pub elite: usize,
    pub tournament: usize,
    pub crossover_rate: f64,
    /// Every this many generations each island's best replaces the next island's worst.
    pub migration_interval: usize,
    /// Racing: games per stage. Everyone plays stage 0; the best `race_keep` fraction goes on
    /// to each later stage.
    pub race_games: Vec<u64>,
    pub race_keep: f64,
    /// Fitness penalty, in win-rate points (0.002 = 0.2%), per rule and per condition.
    pub parsimony: f64,
    /// Validate the champion on held-out seeds every this many generations (0 = only at the end).
    pub validate_every: usize,
    pub validate_games: u64,
    /// Validate with full bot behaviour (turn search for action plays and `win_this_turn`),
    /// exactly as bots play in the game UI. Slower for engine strategies.
    pub validate_full: bool,
    /// Games per ablation test in the final polish (0 = skip polishing).
    pub polish_games: u64,
    /// Evolve and polish in fast mode (rule-order action plays, no win lookahead). Several times
    /// faster; validation re-checks with full behaviour when `validate_full` is set.
    pub fast_eval: bool,
    pub max_turns: u16,
    pub seed: u64,
    pub max_gain_rules: usize,
    pub max_play_rules: usize,
    pub max_trash_rules: usize,
    pub max_conds: usize,
    pub hall_of_fame: usize,
}

impl Default for EvolveConfig {
    fn default() -> Self {
        EvolveConfig {
            track: Track::Fixed { kingdom: Vec::new() },
            sets: vec!["Base".into(), "Intrigue".into()],
            opponents: Vec::new(),
            forbidden: vec!["Witch".into()],
            seeds: Vec::new(),
            islands: 4,
            island_size: 32,
            generations: 100,
            stall_generations: 30,
            elite: 3,
            tournament: 3,
            crossover_rate: 0.2,
            migration_interval: 10,
            race_games: vec![1_000, 4_000, 16_000],
            race_keep: 0.25,
            parsimony: 0.002,
            validate_every: 10,
            validate_games: 20_000,
            validate_full: false,
            polish_games: 20_000,
            fast_eval: true,
            max_turns: 60,
            seed: 1,
            max_gain_rules: 10,
            max_play_rules: 5,
            max_trash_rules: 4,
            max_conds: 2,
            hall_of_fame: 20,
        }
    }
}
