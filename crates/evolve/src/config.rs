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

/// Whether a fixed kingdom's games use Platinum and Colony.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Colonies {
    /// Colonies are in play iff the fixed kingdom contains a Prosperity card (the official rule).
    Auto,
    Yes,
    No,
}

impl Default for Colonies {
    fn default() -> Self {
        Colonies::Auto
    }
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

/// Kingdom league mode: instead of fixed opponents, a candidate plays a weighted mix of the
/// automatic baselines (anchors) and champions found earlier in the same run (the hall of
/// fame), keeps one champion per niche, and the niche champions are ranked in a round robin at
/// the end. Needs `track = Fixed` with a kingdom; `opponents` is ignored. See the "Kingdom league"
/// section of `docs/strategy-search-plan.md`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LeagueConfig {
    /// Share of a candidate's games played against the anchors (baselines), against the most
    /// recent hall-of-fame members, and against a sample of older ones. Groups that are still
    /// empty (no hall of fame yet) are dropped and the rest renormalized.
    pub anchor_weight: f64,
    pub recent_weight: f64,
    pub old_weight: f64,
    /// How many of the newest hall-of-fame members count as "recent".
    pub recent_count: usize,
    /// How many of the older ones are sampled each generation.
    pub old_sample: usize,
    /// Largest number of niches kept (the archive holds one champion per niche).
    pub niches: usize,
    /// The best this many new strategies per generation are validated and may join the hall of fame.
    pub join_per_generation: usize,
    /// Validated win rate against the league needed to join the hall of fame.
    pub hof_min_win: f64,
    /// Chance that a parent is drawn from the niche archive instead of by tournament.
    pub archive_parent_rate: f64,
    /// Added to the selection fitness of candidates in a niche nobody held before.
    pub novelty_bonus: f64,
    /// An island that contributed no new niche champion for this many generations is shaken up
    /// (stronger mutation) or restarted from another baseline, alternately (0 = never).
    pub stagnation_generations: usize,
    /// Round robin among the niche champions and the anchors: games per pair.
    pub final_games: u64,
    /// Play the final round robin with full bot behaviour (turn search), not fast mode.
    pub final_full: bool,
    /// How many distinct top strategies the final report presents.
    pub top_n: usize,
    /// Exploiter rounds after the final round robin (0 = none): each evolves a best response to
    /// the current Nash mix and joins the population if it beats the mix by more than
    /// `exploit_margin`; the loop stops at the first round that does not.
    pub exploit_rounds: usize,
    /// Generations of the GA run that evolves each exploiter.
    pub exploit_generations: usize,
    /// An exploiter must beat the mix by more than this (0.03: win rate above 53%) to be added.
    pub exploit_margin: f64,
    /// Games measuring an exploiter against the mix (full bots when `final_full`).
    pub exploit_games: u64,
}

impl Default for LeagueConfig {
    fn default() -> Self {
        LeagueConfig {
            anchor_weight: 0.4,
            recent_weight: 0.4,
            old_weight: 0.2,
            recent_count: 4,
            old_sample: 3,
            niches: 10,
            join_per_generation: 2,
            hof_min_win: 0.5,
            archive_parent_rate: 0.4,
            novelty_bonus: 0.01,
            stagnation_generations: 8,
            final_games: 2_000,
            final_full: true,
            top_n: 5,
            exploit_rounds: 3,
            exploit_generations: 30,
            exploit_margin: 0.03,
            exploit_games: 4_000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EvolveConfig {
    pub track: Track,
    /// Which expansions' kingdom cards are in play: any of "Base", "Intrigue", "Seaside",
    /// "Prosperity", "Alchemy", "Promo", "Cornucopia & Guilds", "Dark Ages", "Adventures". Governs the default (empty) fixed kingdom, the random track's draw pool,
    /// and so the search space.
    pub sets: Vec<String>,
    /// Fixed track only: whether its games use Platinum and Colony (default: auto-detect from
    /// the fixed kingdom). The random track always follows the official per-kingdom rule.
    pub colonies: Colonies,
    pub opponents: Vec<OpponentSpec>,
    /// Kingdom league mode (replaces `opponents`; needs a fixed kingdom). `None`: fixed opponents.
    pub league: Option<LeagueConfig>,
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
    /// Most `[[mode]]` rules (a choose-one card's stated option, e.g. Investment's trash-for-VP).
    pub max_mode_rules: usize,
    pub max_conds: usize,
    pub hall_of_fame: usize,
}

impl Default for EvolveConfig {
    fn default() -> Self {
        EvolveConfig {
            track: Track::Fixed { kingdom: Vec::new() },
            sets: vec!["Base".into(), "Intrigue".into(), "Seaside".into(), "Prosperity".into()],
            colonies: Colonies::default(),
            opponents: Vec::new(),
            league: None,
            forbidden: Vec::new(),
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
            max_mode_rules: 3,
            max_conds: 2,
            hall_of_fame: 20,
        }
    }
}
