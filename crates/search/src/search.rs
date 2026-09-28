//! The exact within-turn expectimax search (PLAN.md §4).
//!
//! `Searcher` owns a fixed-size, preallocated transposition table (see `hash::turn_hash`) that
//! is reused across `analyze` calls — the only heap allocation is the one `Vec` backing the
//! table (and the `Vec`/`String` used to assemble the final `Analysis`, which is output
//! packaging, not hot-loop state). The recursive `node_value`/`chance_value` walk itself never
//! allocates: it clones `GameState` (a `Copy` memcpy) and reuses `ChoiceBuf`.

use crate::eval::Evaluator;
use crate::hash::turn_hash;
use crate::policy::default_policy;
use dominion_engine::cards;
use dominion_engine::rng::Rng;
use dominion_engine::state::Act;
use dominion_engine::{Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, Pending, Phase, Step};
use std::time::{Duration, Instant};

/// A state is a leaf once the turn has reached the start of cleanup, *before* the next hand is
/// drawn: `Phase::Buy` with no buys left (about to call `cleanup()`, which the search never lets
/// `advance()` do), or `Phase::CleanupDraw` already set with its `Draw` frame not yet run (the
/// `Choice::Pass` buy path folds hand+in_play into discard and flips the phase synchronously
/// inside `apply()`, before any card of the next hand is touched). Either way `pending()` is
/// `Pending::None` at this point; see `apply_phase`/`cleanup` in `dominion_engine::engine`.
/// `Instant::now` panics on wasm32-unknown-unknown (no clock), so timing is native-only.
#[cfg(not(target_arch = "wasm32"))]
fn now() -> Option<Instant> {
    Some(Instant::now())
}
#[cfg(target_arch = "wasm32")]
fn now() -> Option<Instant> {
    None
}

/// A decision that isn't searched: another player's (fixed default policy) or one of `me`'s
/// when the evaluator supplies a policy.
pub(crate) fn fixed_choice<E: Evaluator>(eval: &E, s: &GameState, me: u8, d: &Decision, choices: &[Choice]) -> Option<Choice> {
    if d.player != me {
        Some(default_policy(s, d, choices))
    } else {
        eval.policy(s, me, d, choices)
    }
}

pub(crate) fn is_leaf(state: &GameState) -> bool {
    state.pending() == Pending::None
        && (state.turn.phase == Phase::CleanupDraw || (state.turn.phase == Phase::Buy && state.turn.buys == 0))
}

#[derive(Clone, Copy)]
struct Eval {
    ev: f64,
    exact: bool,
}

#[derive(Clone, Copy)]
struct TTEntry {
    key: u64,
    value: f64,
    exact: bool,
    /// Entries from earlier `analyze` calls (different root, possibly a different evaluator)
    /// are ignored by bumping the searcher's generation instead of clearing the table.
    generation: u32,
}

impl Default for TTEntry {
    fn default() -> Self {
        TTEntry { key: 0, value: 0.0, exact: true, generation: 0 }
    }
}

/// Node budget / sampling-fallback / transposition-table knobs for one `analyze` call.
#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Once `nodes` reaches this, chance nodes stop enumerating every distinct outcome exactly
    /// and switch to weighted sampling (see `chance_sample_k`); the result is then marked
    /// `exact: false` for every ancestor of that node.
    pub node_budget: u64,
    /// How many outcomes to sample (weighted by probability) at a chance node once the node
    /// budget is exhausted.
    pub chance_sample_k: u32,
    /// log2 of the transposition table's entry capacity.
    pub tt_bits: u32,
    /// Seeds the sampling-fallback RNG, for reproducible approximate results.
    pub seed: u64,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig { node_budget: 200_000, chance_sample_k: 8, tt_bits: 18, seed: 0xC0FFEE }
    }
}

/// One legal choice at the root decision, with its expected value and best continuation.
#[derive(Clone, Debug)]
pub struct RootOption {
    pub choice: Choice,
    /// Expected value for `me` (higher is better) if this choice is taken.
    pub ev: f64,
    /// False if any node budget in this subtree fell back to sampling.
    pub exact: bool,
    /// The best continuation line as readable text, e.g.
    /// `"Play Village → Play Smithy → draw {Gold 30%, Copper 50%, Estate 20%} → end turn"`.
    pub pv: String,
    /// Probability of each distinct result of the turn under the best play after this choice:
    /// the cards gained ("Gold + Silver"), plus "Win Game" if the turn ends the game with a win.
    /// Most likely first.
    pub outcomes: Vec<(String, f64)>,
}

/// The result of `analyze`: every legal choice at the root decision, ranked best-first.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub options: Vec<RootOption>,
    pub nodes: u64,
    pub tt_hits: u64,
    pub tt_stores: u64,
    pub elapsed: Duration,
}

impl Analysis {
    /// The highest-EV option (the root's own legal-choice list is never empty per the engine's
    /// invariant, so this panics only if `analyze` was called on a state with no decision).
    pub fn best(&self) -> &RootOption {
        &self.options[0]
    }
}

/// Result of `Searcher::evaluate` for one subtree.
#[derive(Clone, Debug)]
pub struct TaskResult {
    pub ev: f64,
    pub exact: bool,
    pub nodes: u64,
    pub tt_hits: u64,
    /// Best continuation from the subtree's root, as readable steps.
    pub pv: Vec<String>,
    /// Outcome distribution of the subtree (see `RootOption::outcomes`).
    pub outcomes: Vec<(String, f64)>,
}

/// Owns the transposition table and RNG so repeated `analyze` calls (e.g. one per turn across a
/// whole game, or one per candidate move in a UI) don't reallocate.
pub struct Searcher {
    tt: Vec<TTEntry>,
    mask: usize,
    nodes: u64,
    tt_hits: u64,
    tt_stores: u64,
    rng: Rng,
    generation: u32,
}

impl Searcher {
    pub fn new(tt_bits: u32) -> Self {
        let tt_bits = tt_bits.clamp(1, 30);
        let cap = 1usize << tt_bits;
        Searcher { tt: vec![TTEntry::default(); cap], mask: cap - 1, nodes: 0, tt_hits: 0, tt_stores: 0, rng: Rng::new(0), generation: 0 }
    }

    pub fn tt_capacity(&self) -> usize {
        self.tt.len()
    }

    /// Analyze the decision pending in `state` for `me`. `state` should already be honest from
    /// `me`'s point of view (see `determinize`); `chance_mode` is forced on regardless, so
    /// callers don't need to remember to set it.
    pub fn analyze<E: Evaluator>(&mut self, state: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> Analysis {
        self.nodes = 0;
        self.tt_hits = 0;
        self.tt_stores = 0;
        self.rng = Rng::new(cfg.seed);
        self.generation = self.generation.wrapping_add(1).max(1);
        let start = now();

        let mut root = *state;
        root.chance_mode = true;
        root.pause_at_turn_start = false;
        let d = root.pending_decision().expect("analyze: root state has no pending decision");
        assert_eq!(d.player, me, "analyze: pending decision belongs to player {}, not {me}", d.player);

        let mut buf = ChoiceBuf::default();
        root.legal_choices(&mut buf);
        let mut options = Vec::with_capacity(buf.len());
        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&root, me, &d, c));
        for &choice in buf.as_slice() {
            if any_allowed && !eval.allows(&root, me, &d, choice) {
                continue;
            }
            let mut child = root;
            child.apply(choice, &mut NoEvents).expect("analyze: root choice rejected by the engine");
            let r = self.node_value(&root, &child, me, cfg, eval);
            let mut pv = vec![describe(&d, choice)];
            pv.extend(self.build_pv(&root, &child, me, cfg, eval));
            let outcomes = self.outcomes(&root, &child, me, cfg, eval);
            options.push(RootOption { choice, ev: r.ev, exact: r.exact, pv: pv.join(" \u{2192} "), outcomes });
        }
        options.sort_by(|a, b| b.ev.partial_cmp(&a.ev).unwrap_or(std::cmp::Ordering::Equal));

        Analysis { options, nodes: self.nodes, tt_hits: self.tt_hits, tt_stores: self.tt_stores, elapsed: start.map(|t| t.elapsed()).unwrap_or_default() }
    }

    /// The first root choice (in legal-choice order) whose value satisfies `accept(ev, exact)`,
    /// stopping as soon as one is found. No readable lines are built and nothing is allocated
    /// (beyond the searcher's own table), so bots can call it inside batch simulations.
    pub fn first_choice_where<E: Evaluator>(
        &mut self,
        state: &GameState,
        me: u8,
        cfg: &SearchConfig,
        eval: &E,
        accept: impl Fn(f64, bool) -> bool,
    ) -> Option<Choice> {
        self.nodes = 0;
        self.tt_hits = 0;
        self.tt_stores = 0;
        self.rng = Rng::new(cfg.seed);
        self.generation = self.generation.wrapping_add(1).max(1);
        let mut root = *state;
        root.chance_mode = true;
        let d = root.pending_decision()?;
        if d.player != me {
            return None;
        }
        let mut buf = ChoiceBuf::default();
        root.legal_choices(&mut buf);
        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&root, me, &d, c));
        for &choice in buf.as_slice() {
            if any_allowed && !eval.allows(&root, me, &d, choice) {
                continue;
            }
            let mut child = root;
            child.apply(choice, &mut NoEvents).ok()?;
            let r = self.node_value(&root, &child, me, cfg, eval);
            if accept(r.ev, r.exact) {
                return Some(choice);
            }
        }
        None
    }

    /// The best root choice (first in legal-choice order among equal values), its value, and
    /// whether every root option was valued exactly (so the choice is provably best).
    /// Allocation-free, for bots deciding inside batch simulations.
    pub fn best_choice<E: Evaluator>(&mut self, state: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> Option<(Choice, f64, bool)> {
        self.nodes = 0;
        self.tt_hits = 0;
        self.tt_stores = 0;
        self.rng = Rng::new(cfg.seed);
        self.generation = self.generation.wrapping_add(1).max(1);
        let mut root = *state;
        root.chance_mode = true;
        root.pause_at_turn_start = false;
        let d = root.pending_decision()?;
        if d.player != me {
            return None;
        }
        let mut buf = ChoiceBuf::default();
        root.legal_choices(&mut buf);
        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&root, me, &d, c));
        let mut best: Option<(Choice, f64)> = None;
        let mut all_exact = true;
        for &choice in buf.as_slice() {
            if any_allowed && !eval.allows(&root, me, &d, choice) {
                continue;
            }
            let mut child = root;
            child.apply(choice, &mut NoEvents).ok()?;
            let r = self.node_value(&root, &child, me, cfg, eval);
            all_exact &= r.exact;
            if best.map_or(true, |(_, v)| r.ev > v) {
                best = Some((choice, r.ev));
            }
        }
        best.map(|(c, v)| (c, v, all_exact))
    }

    /// Value of one subtree (a state with nothing pending) plus its best continuation, as a
    /// self-contained unit of work for parallel analysis (`plan::Plan`). Uses this searcher's
    /// own transposition table, scoped to this call.
    pub fn evaluate<E: Evaluator>(&mut self, root: &GameState, state: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> TaskResult {
        self.nodes = 0;
        self.tt_hits = 0;
        self.tt_stores = 0;
        self.rng = Rng::new(cfg.seed);
        self.generation = self.generation.wrapping_add(1).max(1);
        let r = self.node_value(root, state, me, cfg, eval);
        let pv = self.build_pv(root, state, me, cfg, eval);
        let outcomes = self.outcomes(root, state, me, cfg, eval);
        TaskResult { ev: r.ev, exact: r.exact, nodes: self.nodes, tt_hits: self.tt_hits, pv, outcomes }
    }

    // ------------------------------------------------------------------------------------
    // Core recursive value function.
    // ------------------------------------------------------------------------------------

    fn node_value<E: Evaluator>(&mut self, root: &GameState, state: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> Eval {
        self.nodes += 1;
        if is_leaf(state) {
            return Eval { ev: eval.leaf_value(root, state, me), exact: true };
        }

        let key = turn_hash(state, me);
        let idx = (key as usize) & self.mask;
        let slot = self.tt[idx];
        if slot.generation == self.generation && slot.key == key {
            self.tt_hits += 1;
            return Eval { ev: slot.value, exact: slot.exact };
        }

        // Hard cap: past 4x the budget, finish this subtree with a single cheap playout so the
        // total work stays bounded however long the turn's action chains get.
        if self.nodes > cfg.node_budget.saturating_mul(4) {
            return self.rollout(root, state, me, eval);
        }

        let mut s = *state;
        let result = match s.advance(&mut NoEvents) {
            Step::GameOver | Step::TurnStart { .. } => Eval { ev: eval.leaf_value(root, &s, me), exact: true },
            Step::Chance { player } => self.chance_value(root, &s, player, me, cfg, eval),
            Step::Decision(d) => {
                let mut buf = ChoiceBuf::default();
                s.legal_choices(&mut buf);
                debug_assert!(!buf.is_empty());
                if let Some(choice) = fixed_choice(eval, &s, me, &d, buf.as_slice()) {
                    let mut child = s;
                    child.apply(choice, &mut NoEvents).expect("fixed policy chose an illegal choice");
                    self.node_value(root, &child, me, cfg, eval)
                } else {
                    let mut best = f64::NEG_INFINITY;
                    let mut exact = true;
                    let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&s, me, &d, c));
                    if !any_allowed {
                        if let Some(v) = eval.value_when_nothing_allowed() {
                            best = v;
                            buf.clear();
                        }
                    }
                    for &choice in buf.as_slice() {
                        if any_allowed && !eval.allows(&s, me, &d, choice) {
                            continue;
                        }
                        let mut child = s;
                        child.apply(choice, &mut NoEvents).expect("legal choice rejected by the engine");
                        let r = self.node_value(root, &child, me, cfg, eval);
                        exact &= r.exact;
                        if r.ev > best {
                            best = r.ev;
                        }
                    }
                    Eval { ev: best, exact }
                }
            }
        };

        self.tt[idx] = TTEntry { key, value: result.ev, exact: result.exact, generation: self.generation };
        self.tt_stores += 1;
        result
    }

    /// One playout to the end of the turn: draws sampled, my decisions greedy on the evaluator
    /// one step ahead, others by the fixed policy. Linear cost; approximate.
    fn rollout<E: Evaluator>(&mut self, root: &GameState, state: &GameState, me: u8, eval: &E) -> Eval {
        let mut s = *state;
        let mut buf = ChoiceBuf::default();
        loop {
            self.nodes += 1;
            if is_leaf(&s) {
                return Eval { ev: eval.leaf_value(root, &s, me), exact: false };
            }
            match s.advance(&mut NoEvents) {
                Step::GameOver | Step::TurnStart { .. } => return Eval { ev: eval.leaf_value(root, &s, me), exact: false },
                Step::Chance { player } => {
                    let outcomes = s.chance_outcomes(player);
                    let card = outcomes.nth(self.rng.below(outcomes.total()));
                    s.resolve_chance(player, card);
                }
                Step::Decision(d) => {
                    s.legal_choices(&mut buf);
                    let choice = if let Some(c) = fixed_choice(eval, &s, me, &d, buf.as_slice())
                        .or_else(|| eval.playout_choice(&s, me, &d, buf.as_slice()))
                    {
                        c
                    } else {
                        let mut best = (f64::NEG_INFINITY, buf.as_slice()[0]);
                        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&s, me, &d, c));
                        for &c in buf.as_slice() {
                            if any_allowed && !eval.allows(&s, me, &d, c) {
                                continue;
                            }
                            let mut child = s;
                            child.apply(c, &mut NoEvents).expect("legal choice");
                            let v = eval.leaf_value(root, &child, me);
                            if v > best.0 {
                                best = (v, c);
                            }
                        }
                        best.1
                    };
                    let choice = if buf.contains(choice) {
                        choice
                    } else {
                        debug_assert!(false, "playout suggested an illegal choice {choice:?} for {d:?}");
                        buf.as_slice()[0]
                    };
                    s.apply(choice, &mut NoEvents).expect("legal choice");
                }
            }
        }
    }

    /// `state` has a chance event pending for `player` (i.e. `state.advance()` would return
    /// `Step::Chance { player }`). Sums over every distinct outcome, weighted by its exact
    /// probability, unless the node budget has already been spent — then falls back to
    /// weighted sampling and marks the result approximate.
    fn chance_value<E: Evaluator>(&mut self, root: &GameState, state: &GameState, player: u8, me: u8, cfg: &SearchConfig, eval: &E) -> Eval {
        let outcomes = state.chance_outcomes(player);
        let total = outcomes.total();
        debug_assert!(total > 0, "chance event with an empty unknown deck");

        if self.nodes < cfg.node_budget {
            let mut ev = 0.0;
            let mut exact = true;
            for (card, n) in outcomes.iter() {
                let mut child = *state;
                child.resolve_chance(player, card);
                let p = n as f64 / total as f64;
                let r = self.node_value(root, &child, me, cfg, eval);
                ev += p * r.ev;
                exact &= r.exact;
            }
            Eval { ev, exact }
        } else {
            let k = cfg.chance_sample_k.max(1);
            let mut sum = 0.0;
            for _ in 0..k {
                let pick = self.rng.below(total);
                let card = outcomes.nth(pick);
                let mut child = *state;
                child.resolve_chance(player, card);
                sum += self.node_value(root, &child, me, cfg, eval).ev;
            }
            Eval { ev: sum / k as f64, exact: false }
        }
    }

    /// Outcome distribution after `after`: follow the best play (the same choices the value and
    /// line use) and weight every draw by its probability. Past `OUTCOME_CHANCE_BUDGET` expanded
    /// draws, only the most likely card is followed (the result is then approximate).
    pub(crate) fn outcomes<E: Evaluator>(&mut self, root: &GameState, after: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> Vec<(String, f64)> {
        const OUTCOME_CHANCE_BUDGET: u32 = 4_000;
        let mut acc: Vec<(String, f64)> = Vec::new();
        let mut budget = OUTCOME_CHANCE_BUDGET;
        self.outcomes_rec(root, *after, me, cfg, eval, 1.0, &mut budget, &mut acc);
        acc.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        acc
    }

    #[allow(clippy::too_many_arguments)]
    fn outcomes_rec<E: Evaluator>(
        &mut self,
        root: &GameState,
        mut s: GameState,
        me: u8,
        cfg: &SearchConfig,
        eval: &E,
        p: f64,
        budget: &mut u32,
        acc: &mut Vec<(String, f64)>,
    ) {
        let add = |label: String, p: f64, acc: &mut Vec<(String, f64)>| match acc.iter_mut().find(|(l, _)| *l == label) {
            Some(e) => e.1 += p,
            None => acc.push((label, p)),
        };
        loop {
            if is_leaf(&s) {
                add(outcome_label(root, &s, me), p, acc);
                return;
            }
            match s.advance(&mut NoEvents) {
                Step::GameOver | Step::TurnStart { .. } => {
                    add(outcome_label(root, &s, me), p, acc);
                    return;
                }
                Step::Chance { player } => {
                    let outcomes = s.chance_outcomes(player);
                    let total = outcomes.total() as f64;
                    if *budget > 0 {
                        *budget -= 1;
                        for (card, n) in outcomes.iter() {
                            let mut c = s;
                            c.resolve_chance(player, card);
                            self.outcomes_rec(root, c, me, cfg, eval, p * n as f64 / total, budget, acc);
                        }
                        return;
                    }
                    let (card, _) = outcomes.iter().max_by_key(|&(_, n)| n).expect("nonempty outcomes");
                    s.resolve_chance(player, card);
                }
                Step::Decision(d) => {
                    let mut buf = ChoiceBuf::default();
                    s.legal_choices(&mut buf);
                    let choice = if let Some(c) = fixed_choice(eval, &s, me, &d, buf.as_slice()) {
                        c
                    } else {
                        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&s, me, &d, c));
                        let mut best = (f64::NEG_INFINITY, buf.as_slice()[0]);
                        for &c in buf.as_slice() {
                            if any_allowed && !eval.allows(&s, me, &d, c) {
                                continue;
                            }
                            let mut child = s;
                            child.apply(c, &mut NoEvents).expect("legal choice");
                            let r = self.node_value(root, &child, me, cfg, eval);
                            if r.ev > best.0 {
                                best = (r.ev, c);
                            }
                        }
                        best.1
                    };
                    s.apply(choice, &mut NoEvents).expect("legal choice");
                }
            }
        }
    }

    // ------------------------------------------------------------------------------------
    // Principal-variation text, built by greedily following argmax choices (backed by the
    // now-populated TT, so this is cheap) and, at chance nodes, the most likely outcome.
    // ------------------------------------------------------------------------------------

    pub(crate) fn build_pv<E: Evaluator>(&mut self, root: &GameState, after: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> Vec<String> {
        let mut s = *after;
        let mut parts = Vec::new();
        loop {
            if is_leaf(&s) {
                parts.push(end_of_turn_label(&s, me));
                return parts;
            }
            match s.advance(&mut NoEvents) {
                Step::GameOver => {
                    parts.push("game over".to_string());
                    return parts;
                }
                Step::TurnStart { .. } => {
                    parts.push(end_of_turn_label(&s, me));
                    return parts;
                }
                Step::Chance { player } => {
                    let outcomes = s.chance_outcomes(player);
                    let total = outcomes.total();
                    parts.push(describe_chance(&outcomes, total));
                    let (card, _) = outcomes.iter().max_by_key(|&(_, n)| n).expect("nonempty outcomes");
                    s.resolve_chance(player, card);
                }
                Step::Decision(d) => {
                    let mut buf = ChoiceBuf::default();
                    s.legal_choices(&mut buf);
                    let choice = if let Some(c) = fixed_choice(eval, &s, me, &d, buf.as_slice()) {
                        c
                    } else {
                        let mut best_choice = buf.as_slice()[0];
                        let mut best = f64::NEG_INFINITY;
                        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&s, me, &d, c));
                        for &c in buf.as_slice() {
                            if any_allowed && !eval.allows(&s, me, &d, c) {
                                continue;
                            }
                            let mut child = s;
                            child.apply(c, &mut NoEvents).expect("legal choice rejected by the engine");
                            let r = self.node_value(root, &child, me, cfg, eval);
                            if r.ev > best {
                                best = r.ev;
                                best_choice = c;
                            }
                        }
                        best_choice
                    };
                    parts.push(describe(&d, choice));
                    s.apply(choice, &mut NoEvents).expect("PV replay: choice became illegal");
                }
            }
        }
    }
}

fn verb(act: Act) -> &'static str {
    match act {
        Act::Discard => "Discard",
        Act::Trash => "Trash",
        Act::Topdeck => "Topdeck",
        Act::Play => "Play",
        Act::SetAside => "Set aside",
    }
}

fn with_source(base: String, source: Option<dominion_engine::CardId>, choice_card: Option<dominion_engine::CardId>) -> String {
    match source {
        Some(src) if Some(src) != choice_card => format!("{base} ({})", cards::name(src)),
        _ => base,
    }
}

/// "end turn", or "end turn → Win Game" / "end turn → game ends (not a win)" when the turn ends it.
pub(crate) fn end_of_turn_label(leaf: &GameState, me: u8) -> String {
    match leaf.result_if_turn_ends() {
        Some(w) if w == 1 << me => "end turn \u{2192} Win Game".to_string(),
        Some(_) => "end turn \u{2192} game ends (not a win)".to_string(),
        None => "end turn".to_string(),
    }
}

/// A turn's result for `me`: actions played (treasures are always played, so left out), cards
/// trashed, cards gained, and the game result if the turn ends it. E.g.
/// "Play Witch · Trash Estate · Gain Gold + Win Game". "Nothing" if none of those happened.
pub(crate) fn outcome_label(root: &GameState, leaf: &GameState, me: u8) -> String {
    use dominion_engine::cards::{CardId, NUM_CARDS, TREASURE};
    let list = |items: &mut Vec<(u8, CardId, i32)>| -> String {
        items.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        items
            .iter()
            .map(|&(_, c, n)| if n > 1 { format!("{n} {}", cards::name(c)) } else { cards::name(c).to_string() })
            .collect::<Vec<_>>()
            .join(" + ")
    };
    let (mut played, mut trashed, mut gained) = (Vec::new(), Vec::new(), Vec::new());
    if leaf.turn.player == me && leaf.turn.number == root.turn.number {
        for c in 0..NUM_CARDS as CardId {
            let n = leaf.turn.played.get(c) as i32 - root.turn.played.get(c) as i32;
            if n > 0 && !cards::is(c, TREASURE) {
                played.push((cards::cost(c), c, n));
            }
        }
    }
    let before = root.players[me as usize].all_cards();
    let after = leaf.players[me as usize].all_cards();
    for c in 0..NUM_CARDS as CardId {
        let d = after.get(c) as i32 - before.get(c) as i32;
        if d > 0 {
            gained.push((cards::cost(c), c, d));
        } else if d < 0 {
            trashed.push((cards::cost(c), c, -d));
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if !played.is_empty() {
        parts.push(format!("Play {}", list(&mut played)));
    }
    if !trashed.is_empty() {
        parts.push(format!("Trash {}", list(&mut trashed)));
    }
    let mut end = String::new();
    if !gained.is_empty() {
        end = format!("Gain {}", list(&mut gained));
    }
    let result = match leaf.result_if_turn_ends() {
        Some(w) if w == 1 << me => Some("Win Game"),
        Some(_) => Some("game ends (not a win)"),
        None => None,
    };
    if let Some(r) = result {
        end = if end.is_empty() { r.to_string() } else { format!("{end} + {r}") };
    }
    if !end.is_empty() {
        parts.push(end);
    }
    if parts.is_empty() {
        "Nothing".to_string()
    } else {
        parts.join(" \u{b7} ")
    }
}

pub(crate) fn describe(d: &Decision, choice: Choice) -> String {
    let base = match (d.kind, choice) {
        (DecisionKind::PlayAction, Choice::Card(c)) => format!("Play {}", cards::name(c)),
        (DecisionKind::PlayAction, _) => "End actions".to_string(),
        (DecisionKind::Buy, Choice::Card(c)) => format!("Buy {}", cards::name(c)),
        (DecisionKind::Buy, _) => "End buys".to_string(),
        (DecisionKind::Gain { .. }, Choice::Card(c)) => format!("Gain {}", cards::name(c)),
        (DecisionKind::Gain { .. }, _) => "Gain nothing".to_string(),
        (DecisionKind::Select { act, .. }, Choice::Card(c)) => format!("{} {}", verb(act), cards::name(c)),
        (DecisionKind::Select { .. }, _) => "stop".to_string(),
        (DecisionKind::YesNo { act }, Choice::Yes) => format!("{} {}", verb(act), cards::name(d.subject)),
        (DecisionKind::YesNo { act }, _) => format!("don't {} {}", verb(act).to_lowercase(), cards::name(d.subject)),
    };
    let choice_card = match choice {
        Choice::Card(c) => Some(c),
        _ => None,
    };
    with_source(base, d.source, choice_card)
}

pub(crate) fn describe_chance(outcomes: &dominion_engine::Counts, total: u32) -> String {
    let mut items: Vec<(dominion_engine::CardId, u8)> = outcomes.iter().collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let inner: Vec<String> =
        items.iter().map(|&(c, n)| format!("{} {:.0}%", cards::name(c), 100.0 * n as f64 / total as f64)).collect();
    format!("draw {{{}}}", inner.join(", "))
}

/// Convenience wrapper matching the exact signature from the design doc: allocates a fresh
/// `Searcher` (and so a fresh transposition table) for this one call. Prefer keeping a
/// `Searcher` around and calling `Searcher::analyze` directly when analyzing many positions
/// (e.g. many turns across a game, or many candidate root states from a UI), so the table is
/// actually reused as intended.
pub fn analyze<E: Evaluator>(state: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> Analysis {
    let mut searcher = Searcher::new(cfg.tt_bits);
    searcher.analyze(state, me, cfg, eval)
}
