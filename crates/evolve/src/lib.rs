//! Strategy discovery: search the space of *strategy files* for ones that beat a target opponent.
//!
//! Everything the search produces is an ordinary strategy TOML (`[[gain]]` / `[[play]]` /
//! `[[trash]]` rules with simple conditions), so results are readable by construction. The
//! pieces are independent so other search methods (MAP-Elites, threshold tuning, ML-guided
//! proposals) can reuse them:
//!
//! - [`genome`]: the rule-list representation, printing to / parsing from strategy TOML.
//! - [`space`]: which cards and condition templates a search may use (e.g. "no Witch").
//! - [`ops`]: random generation, mutation and crossover of genomes.
//! - [`arena`]: fitness: head-to-head games against an opponent pool on a set of kingdoms,
//!   with common random numbers so every candidate faces the same deals.
//! - [`ga`]: the island-model genetic algorithm with racing, parsimony, validation on
//!   held-out seeds and a hall of fame; reports [`progress::Progress`] snapshots.
//! - [`polish`]: rule ablation: drop rules that don't matter, annotate the rest with their impact.
//! - [`config`]: the serializable run configuration.

pub mod arena;
pub mod config;
pub mod ga;
pub mod genome;
pub mod ops;
pub mod polish;
pub mod progress;
pub mod space;

pub use arena::{Arena, Scenario, Score};
pub use config::{EvolveConfig, OpponentSpec, Track};
pub use ga::{assess, run, Control};
pub use genome::{Atom, Genome, Op, Rule, Var};
pub use progress::{Entry, Progress};
pub use space::SearchSpace;
