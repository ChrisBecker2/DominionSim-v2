//! Exact within-turn expectimax search (PLAN.md §4–§5): for the current player's own turn,
//! enumerate every legal line of play and every draw outcome with exact probabilities, and rank
//! the options at the current decision.
//!
//! Entry points:
//! - [`determinize`] resamples opponents' hidden hand/deck split honestly (own knowledge only).
//! - [`Searcher::analyze`] (or the free function [`analyze`]) runs the search and returns an
//!   [`Analysis`]: every legal choice at the root, its expected value, whether it's exact, and a
//!   readable principal variation.
//! - [`Evaluator`] scores a leaf (the state at the start of cleanup, before the next hand is
//!   drawn); [`NextHandEvaluator`] is PLAN.md's "Layer 1", [`MoneyEvaluator`] a simpler baseline.
//! - [`search_choose`] / [`SearchAgent`] wire the search into `dominion_engine::Agent` (see
//!   `agent.rs` for why `SearchAgent` can't implement `Agent` cleanly with today's engine API).

mod agent;
mod determinize;
mod eval;
mod hash;
mod plan;
mod policy;
mod search;

pub use agent::{search_choose, SearchAgent};
pub use determinize::determinize;
pub use eval::{average_hand_money, estimated_turns_left, game_end_value, expected_next_hand_money, EvalWeights, Evaluator, MoneyEvaluator, NextHandEvaluator};
pub use hash::turn_hash;
pub use policy::default_policy;
#[cfg(not(target_arch = "wasm32"))]
pub use plan::analyze_parallel;
pub use plan::Plan;
pub use search::{analyze, Analysis, RootOption, SearchConfig, Searcher, TaskResult};
