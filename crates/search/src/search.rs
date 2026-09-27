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
        let d = root.pending_decision().expect("analyze: root state has no pending decision");
        assert_eq!(d.player, me, "analyze: pending decision belongs to player {}, not {me}", d.player);

        let mut buf = ChoiceBuf::default();
        root.legal_choices(&mut buf);
        let mut options = Vec::with_capacity(buf.len());
        for &choice in buf.as_slice() {
            let mut child = root;
            child.apply(choice, &mut NoEvents).expect("analyze: root choice rejected by the engine");
            let r = self.node_value(&root, &child, me, cfg, eval);
            let mut pv = vec![describe(&d, choice)];
            pv.extend(self.build_pv(&root, &child, me, cfg, eval));
            options.push(RootOption { choice, ev: r.ev, exact: r.exact, pv: pv.join(" \u{2192} ") });
        }
        options.sort_by(|a, b| b.ev.partial_cmp(&a.ev).unwrap_or(std::cmp::Ordering::Equal));

        Analysis { options, nodes: self.nodes, tt_hits: self.tt_hits, tt_stores: self.tt_stores, elapsed: start.map(|t| t.elapsed()).unwrap_or_default() }
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
        TaskResult { ev: r.ev, exact: r.exact, nodes: self.nodes, tt_hits: self.tt_hits, pv }
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
            Step::GameOver => Eval { ev: eval.leaf_value(root, &s, me), exact: true },
            Step::Chance { player } => self.chance_value(root, &s, player, me, cfg, eval),
            Step::Decision(d) => {
                let mut buf = ChoiceBuf::default();
                s.legal_choices(&mut buf);
                debug_assert!(!buf.is_empty());
                if d.player != me {
                    let choice = default_policy(&s, &d, buf.as_slice());
                    let mut child = s;
                    child.apply(choice, &mut NoEvents).expect("default policy chose an illegal choice");
                    self.node_value(root, &child, me, cfg, eval)
                } else {
                    let mut best = f64::NEG_INFINITY;
                    let mut exact = true;
                    for &choice in buf.as_slice() {
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
                Step::GameOver => return Eval { ev: eval.leaf_value(root, &s, me), exact: false },
                Step::Chance { player } => {
                    let outcomes = s.chance_outcomes(player);
                    let card = outcomes.nth(self.rng.below(outcomes.total()));
                    s.resolve_chance(player, card);
                }
                Step::Decision(d) => {
                    s.legal_choices(&mut buf);
                    let choice = if d.player != me {
                        default_policy(&s, &d, buf.as_slice())
                    } else {
                        let mut best = (f64::NEG_INFINITY, buf.as_slice()[0]);
                        for &c in buf.as_slice() {
                            let mut child = s;
                            child.apply(c, &mut NoEvents).expect("legal choice");
                            let v = eval.leaf_value(root, &child, me);
                            if v > best.0 {
                                best = (v, c);
                            }
                        }
                        best.1
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

    // ------------------------------------------------------------------------------------
    // Principal-variation text, built by greedily following argmax choices (backed by the
    // now-populated TT, so this is cheap) and, at chance nodes, the most likely outcome.
    // ------------------------------------------------------------------------------------

    pub(crate) fn build_pv<E: Evaluator>(&mut self, root: &GameState, after: &GameState, me: u8, cfg: &SearchConfig, eval: &E) -> Vec<String> {
        let mut s = *after;
        let mut parts = Vec::new();
        loop {
            if is_leaf(&s) {
                parts.push("end turn".to_string());
                return parts;
            }
            match s.advance(&mut NoEvents) {
                Step::GameOver => {
                    parts.push("game over".to_string());
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
                    let choice = if d.player == me {
                        let mut best_choice = buf.as_slice()[0];
                        let mut best = f64::NEG_INFINITY;
                        for &c in buf.as_slice() {
                            let mut child = s;
                            child.apply(c, &mut NoEvents).expect("legal choice rejected by the engine");
                            let r = self.node_value(root, &child, me, cfg, eval);
                            if r.ev > best {
                                best = r.ev;
                                best_choice = c;
                            }
                        }
                        best_choice
                    } else {
                        default_policy(&s, &d, buf.as_slice())
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
