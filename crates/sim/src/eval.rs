//! Scoring a turn by a strategy's own priorities, for analysis of a strategy seat's decisions.

use dominion_engine::cards::{CardId, NUM_CARDS};
use dominion_engine::{GameState, PlayerView};
use dominion_engine::{Choice, Decision, DecisionKind};
use dominion_search::{game_end_value, Evaluator, NextHandEvaluator};

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
        // Strictly the strategy's own priorities. The general evaluator's game-end term is removed
        // from the tie-break: winning/losing only matters if the rules say so (`wins_game` etc.).
        let tie = self.tie_break.leaf_value(root, leaf, me) - game_end_value(leaf, me);
        v + 1e-3 * tie
    }

    /// The strategy only buys cards in its gain list whose conditions hold, else Done.
    fn allows(&self, state: &GameState, me: u8, decision: &Decision, choice: Choice) -> bool {
        match (decision.kind, choice) {
            (DecisionKind::Buy, Choice::Card(c)) => self.strategy.allows_buy(&PlayerView::new(state, me), c),
            _ => true,
        }
    }
}
