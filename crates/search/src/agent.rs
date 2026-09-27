//! Wiring the search into `dominion_engine::Agent`.

use crate::eval::Evaluator;
use crate::policy::default_policy;
use crate::search::{SearchConfig, Searcher};
use dominion_engine::rng::Rng;
use dominion_engine::{Agent, Choice, Decision, GameState, PlayerView};

/// Determinize `state` from `me`'s point of view, run `analyze`, and return the best choice.
pub fn search_choose<E: Evaluator>(
    state: &GameState,
    me: u8,
    searcher: &mut Searcher,
    cfg: &SearchConfig,
    eval: &E,
    rng: &mut Rng,
) -> Choice {
    let world = PlayerView::new(state, me).determinize(rng);
    searcher.analyze(&world, me, cfg, eval).best().choice
}

/// An honest `Agent` that searches its own decisions. It sees only its `PlayerView`, from which
/// it builds a determinized world (`PlayerView::determinize`) to search.
pub struct SearchAgent<E: Evaluator> {
    name: String,
    searcher: Searcher,
    cfg: SearchConfig,
    eval: E,
    rng: Rng,
}

impl<E: Evaluator> SearchAgent<E> {
    pub fn new(name: impl Into<String>, cfg: SearchConfig, eval: E, seed: u64) -> Self {
        let tt_bits = cfg.tt_bits;
        SearchAgent { name: name.into(), searcher: Searcher::new(tt_bits), cfg, eval, rng: Rng::new(seed) }
    }
}

impl<E: Evaluator + Send> Agent for SearchAgent<E> {
    fn name(&self) -> &str {
        &self.name
    }

    fn choose(&mut self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        if choices.len() == 1 {
            return choices[0];
        }
        let world = view.determinize(&mut self.rng);
        if world.pending_decision() != Some(*decision) {
            return default_policy(&world, decision, choices);
        }
        let choice = self.searcher.analyze(&world, view.me(), &self.cfg, &self.eval).best().choice;
        debug_assert!(choices.contains(&choice));
        choice
    }
}
