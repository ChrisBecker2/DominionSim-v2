//! Scoring a turn by a strategy's own priorities, for analysis of a strategy seat's decisions.

use dominion_engine::cards::{CardId, NUM_CARDS};
use dominion_engine::{GameState, PlayerView};
use dominion_engine::{Choice, Decision, DecisionKind};
use dominion_search::{game_end_value, Evaluator, NextHandEvaluator};

use crate::Strategy;
use dominion_engine::rng::Rng;
use dominion_search::{SearchConfig, Searcher};
use std::cell::RefCell;

/// Scores the end of a turn by the strategy's gain list (plus, with `win_this_turn`, a winning
/// turn above everything): every card gained this turn is worth
/// its place in the list, each entry outranking everything below it (weights are powers of 10),
/// and every card trashed this turn costs its own list value (so Remodel Gold -> Gold nets zero).
/// Conditions are evaluated at the start of the analyzed decision. Below every stated priority:
/// default preferences (attacking, trashing unwanted cards, playing actions/treasures), then a
/// tiny general-purpose economy term to break remaining ties.
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

impl GainListEvaluator<'_> {
    /// Default preferences, always below every stated gain priority (the smallest gain weight is
    /// 10): each Attack played +0.5, each card trashed that the trash rules want gone +0.3, each
    /// Action played +0.1, each Treasure played +0.01.
    fn preferences(&self, root: &GameState, leaf: &GameState, me: u8, before: &dominion_engine::Counts, after: &dominion_engine::Counts) -> f64 {
        if leaf.turn.player != me {
            return 0.0;
        }
        let view = PlayerView::new(root, me);
        let mut p = 0.0;
        for (c, n) in leaf.turn.played.iter() {
            let n = n as f64;
            if dominion_engine::cards::is(c, dominion_engine::cards::ATTACK) {
                p += 0.5 * n;
            }
            if dominion_engine::cards::is(c, dominion_engine::cards::ACTION) {
                p += 0.1 * n;
            }
            if dominion_engine::cards::is(c, dominion_engine::cards::TREASURE) {
                p += 0.01 * n;
            }
        }
        for c in 0..NUM_CARDS as CardId {
            let gone = before.get(c) as i32 - after.get(c) as i32;
            if gone > 0 && self.strategy.wants_trash(&view, c) {
                p += 0.3 * gone as f64;
            }
        }
        p
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
        // from the tie-break: winning only matters if the rules say so. With `win_this_turn` (the
        // default) a turn that wins the game outright, acquiring only listed cards, outranks every
        // list priority, matching what the bot itself plays.
        let tie = self.tie_break.leaf_value(root, leaf, me) - game_end_value(leaf, me);
        let win = if self.strategy.win_this_turn && leaf.result_if_turn_ends() == Some(1 << me) {
            let only_listed = (0..NUM_CARDS as CardId).all(|c| after.get(c) <= before.get(c) || self.strategy.lists(c));
            if only_listed { 1e15 } else { 0.0 }
        } else {
            0.0
        };
        win + v + self.preferences(root, leaf, me, &before, &after) + 1e-6 * tie
    }

    /// Below the analyzed decision, play exactly as the strategy's rules do, so the analysis
    /// values each option by what the bot will actually do next.
    fn policy(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        Some(self.strategy.decide(&PlayerView::new(state, me), decision, choices))
    }

    /// The strategy only buys cards in its gain list whose conditions hold, else Done.
    fn allows(&self, state: &GameState, me: u8, decision: &Decision, choice: Choice) -> bool {
        match (decision.kind, choice) {
            (DecisionKind::Buy, Choice::Card(c)) => self.strategy.allows_buy(&PlayerView::new(state, me), c),
            _ => true,
        }
    }
}

/// Scores 1 if the turn ends the game with `me` winning outright, else 0. Only lets `me` buy or
/// gain cards from the strategy's gain list (conditions true) or pass; a forced gain with no
/// listed option prunes the line. Expectimax over this is the probability of a certain win
/// using only cards the strategy wants, so a value of 1 means a guaranteed win this turn.
pub struct WinFinder<'a> {
    pub strategy: &'a Strategy,
}

impl Evaluator for WinFinder<'_> {
    fn leaf_value(&self, _root: &GameState, leaf: &GameState, me: u8) -> f64 {
        if leaf.result_if_turn_ends() == Some(1 << me) { 1.0 } else { 0.0 }
    }

    fn allows(&self, state: &GameState, me: u8, decision: &Decision, choice: Choice) -> bool {
        match (decision.kind, choice) {
            (DecisionKind::Buy | DecisionKind::Gain { .. }, Choice::Card(c)) => {
                self.strategy.gain_rank(&PlayerView::new(state, me), c).is_some()
            }
            _ => true,
        }
    }

    fn value_when_nothing_allowed(&self) -> Option<f64> {
        Some(0.0)
    }
}

thread_local! {
    static WIN_SEARCHER: RefCell<Option<Searcher>> = const { RefCell::new(None) };
}

const WIN_TT_BITS: u32 = 16;

/// `win_this_turn`: the choice that starts a line of play winning the game this turn with
/// certainty (every draw outcome), acquiring only cards the strategy wants. Deterministic: the
/// hidden-information sample and the search are seeded identically for a given position.
/// `None` if no such line exists or it can't be proven within the node budget.
pub fn certain_win_choice(strategy: &Strategy, view: &PlayerView, choices: &[Choice]) -> Option<Choice> {
    let world = view.determinize(&mut Rng::new(0x51_7c));
    if world.pending_decision().map(|d| d.player) != Some(view.me()) {
        return None;
    }
    let cfg = SearchConfig { node_budget: 100_000, tt_bits: WIN_TT_BITS, ..SearchConfig::default() };
    let eval = WinFinder { strategy };
    let choice = WIN_SEARCHER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let searcher = slot.get_or_insert_with(|| Searcher::new(WIN_TT_BITS));
        searcher.first_choice_where(&world, view.me(), &cfg, &eval, |ev, exact| exact && ev >= 1.0 - 1e-9)
    })?;
    choices.contains(&choice).then_some(choice)
}
