//! Wraps a compiled [`Strategy`] as a `dominion_engine::Agent`.

use dominion_engine::agent::{Agent, PlayerView};
use dominion_engine::engine::{Choice, Decision};

use crate::strategy::Strategy;

/// An `Agent` driven entirely by a scripted [`Strategy`]. Cheap to construct (holds only a
/// reference), so a batch runner creates one per thread and reuses it for every game; `choose`
/// never allocates.
pub struct StrategyAgent<'a> {
    strategy: &'a Strategy,
}

impl<'a> StrategyAgent<'a> {
    pub fn new(strategy: &'a Strategy) -> Self {
        StrategyAgent { strategy }
    }
}

impl<'a> Agent for StrategyAgent<'a> {
    fn name(&self) -> &str {
        &self.strategy.name
    }
    fn choose(&mut self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        self.strategy.decide(view, decision, choices)
    }
}
