//! Scoring a turn by a strategy's own priorities, for analysis of a strategy seat's decisions.

use dominion_engine::cards::{CardId, NUM_CARDS};
use dominion_engine::{GameState, PlayerView};
use dominion_search::{Evaluator, NextHandEvaluator};

use crate::Strategy;

/// Scores the end of a turn by the strategy's gain list: every card gained this turn is worth
/// its place in the list, each entry outranking everything below it (weights are powers of 10),
/// and every card trashed this turn costs its own list value (so Remodel Gold -> Gold nets zero).
/// Conditions are evaluated at the start of the analyzed decision. A tiny general-purpose
/// economy term breaks ties between lines that gain the same things.
pub struct GainListEvaluator<'a> {
    pub strategy: &'a Strategy,
    pub tie_break: NextHandEvaluator,
}

impl<'a> GainListEvaluator<'a> {
    pub fn new(strategy: &'a Strategy) -> Self {
        GainListEvaluator { strategy, tie_break: NextHandEvaluator::default() }
    }

    fn weight(&self, view: &PlayerView, card: CardId) -> f64 {
        match self.strategy.gain_rank(view, card) {
            Some(rank) => 10f64.powi((self.strategy.gain_list_len() - rank) as i32),
            None => 0.0,
        }
    }
}

impl Evaluator for GainListEvaluator<'_> {
    fn leaf_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let view = PlayerView::new(root, me);
        let before = root.players[me as usize].all_cards();
        let after = leaf.players[me as usize].all_cards();
        let mut v = 0.0;
        for c in 0..NUM_CARDS as CardId {
            let d = after.get(c) as i32 - before.get(c) as i32;
            if d != 0 {
                v += d as f64 * self.weight(&view, c);
            }
        }
        v + 1e-3 * self.tie_break.leaf_value(root, leaf, me)
    }
}
