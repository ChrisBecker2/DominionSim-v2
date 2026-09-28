//! Parallel analysis of a single position (root parallelism).
//!
//! `Plan::build` expands the top of the expectimax tree breadth-first until it has enough open
//! subtrees ("tasks") to keep every worker busy. Each task is an independent `GameState` that any
//! `Searcher` can evaluate (`Searcher::evaluate`) — on another thread, or in another WebAssembly
//! instance in a browser worker. `Plan::finish` then folds the task results back up the planned
//! tree exactly as the serial search would: max at my decisions, probability-weighted sums at
//! chance nodes, the fixed policy at other players' decisions.
//!
//! Tasks don't share a transposition table, so a parallel search visits somewhat more nodes than a
//! serial one; the values are identical whenever both are exact.

use crate::eval::Evaluator;
use crate::search::{describe, describe_chance, fixed_choice, is_leaf, Analysis, RootOption, TaskResult};
use dominion_engine::{Choice, ChoiceBuf, GameState, NoEvents, Step};
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Clone, Debug)]
enum Node {
    /// My decision: the best child. Edges are labelled with the choice.
    Max(Vec<(String, usize)>),
    /// A draw/reveal: children weighted by probability. `usize` = most likely child.
    Chance(String, Vec<(f64, usize)>, usize),
    /// Another player's decision, resolved by the fixed policy.
    Forced(String, usize),
    /// Already scored during planning (end of turn / game over).
    Value(f64, &'static str),
    /// Open subtree handed to a worker: index into `Plan::tasks`.
    Task(usize),
    /// Placeholder while planning.
    Open,
}

/// The top of the search tree plus the independent subtrees below it.
pub struct Plan {
    /// The (determinized) position being analyzed; evaluators measure against it.
    pub root: GameState,
    pub me: u8,
    /// Root choices: (choice, label, node index).
    roots: Vec<(Choice, String, usize)>,
    nodes: Vec<Node>,
    /// Independent subtrees to evaluate, each with nothing pending.
    pub tasks: Vec<GameState>,
}

impl Plan {
    /// Expand from `root` (which must have `me`'s decision pending) until at least `target_tasks`
    /// open subtrees exist or the tree is exhausted.
    pub fn build<E: Evaluator>(root: &GameState, me: u8, target_tasks: usize, eval: &E) -> Plan {
        let mut root = *root;
        root.chance_mode = true;
        root.pause_at_turn_start = false;
        let d = root.pending_decision().expect("Plan::build: root has no pending decision");
        assert_eq!(d.player, me, "Plan::build: decision belongs to player {}, not {me}", d.player);

        let mut nodes: Vec<Node> = Vec::new();
        let mut states: Vec<Option<GameState>> = Vec::new();
        let mut open: VecDeque<usize> = VecDeque::new();
        let new_open = |nodes: &mut Vec<Node>, states: &mut Vec<Option<GameState>>, open: &mut VecDeque<usize>, s: GameState| {
            nodes.push(Node::Open);
            states.push(Some(s));
            open.push_back(nodes.len() - 1);
            nodes.len() - 1
        };

        let mut buf = ChoiceBuf::default();
        root.legal_choices(&mut buf);
        let mut roots = Vec::new();
        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&root, me, &d, c));
        for &choice in buf.as_slice() {
            if any_allowed && !eval.allows(&root, me, &d, choice) {
                continue;
            }
            let mut child = root;
            child.apply(choice, &mut NoEvents).expect("legal root choice");
            let idx = new_open(&mut nodes, &mut states, &mut open, child);
            roots.push((choice, describe(&d, choice), idx));
        }

        // Breadth-first expansion; a cap keeps pathological fan-out from ballooning the plan.
        let node_cap = target_tasks.max(1) * 64;
        while open.len() < target_tasks && nodes.len() < node_cap {
            let Some(n) = open.pop_front() else { break };
            let state = states[n].take().unwrap();
            nodes[n] = if is_leaf(&state) {
                Node::Value(eval.leaf_value(&root, &state, me), "end turn")
            } else {
                let mut s = state;
                match s.advance(&mut NoEvents) {
                    Step::GameOver => Node::Value(eval.leaf_value(&root, &s, me), "game over"),
                    Step::TurnStart { .. } => Node::Value(eval.leaf_value(&root, &s, me), "end turn"),
                    Step::Chance { player } => {
                        let outcomes = s.chance_outcomes(player);
                        let total = outcomes.total();
                        let mut kids = Vec::new();
                        let (mut best, mut best_p) = (0, -1.0);
                        for (card, k) in outcomes.iter() {
                            let mut c = s;
                            c.resolve_chance(player, card);
                            let p = k as f64 / total as f64;
                            let idx = new_open(&mut nodes, &mut states, &mut open, c);
                            if p > best_p {
                                best_p = p;
                                best = idx;
                            }
                            kids.push((p, idx));
                        }
                        Node::Chance(describe_chance(&outcomes, total), kids, best)
                    }
                    Step::Decision(d) => {
                        let mut buf = ChoiceBuf::default();
                        s.legal_choices(&mut buf);
                        if let Some(choice) = fixed_choice(eval, &s, me, &d, buf.as_slice()) {
                            let mut c = s;
                            c.apply(choice, &mut NoEvents).expect("policy choice is legal");
                            let idx = new_open(&mut nodes, &mut states, &mut open, c);
                            Node::Forced(describe(&d, choice), idx)
                        } else {
                            let mut kids = Vec::new();
                            let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&s, me, &d, c));
                            for &choice in buf.as_slice() {
                                if any_allowed && !eval.allows(&s, me, &d, choice) {
                                    continue;
                                }
                                let mut c = s;
                                c.apply(choice, &mut NoEvents).expect("legal choice");
                                let idx = new_open(&mut nodes, &mut states, &mut open, c);
                                kids.push((describe(&d, choice), idx));
                            }
                            Node::Max(kids)
                        }
                    }
                }
            };
        }

        let mut tasks = Vec::new();
        for n in open {
            nodes[n] = Node::Task(tasks.len());
            tasks.push(states[n].take().unwrap());
        }
        Plan { root, me, roots, nodes, tasks }
    }

    /// Fold task results (indexed like `tasks`) back into a ranked analysis.
    pub fn finish(&self, results: &[TaskResult], elapsed: Duration) -> Analysis {
        assert_eq!(results.len(), self.tasks.len());
        let mut options: Vec<RootOption> = self
            .roots
            .iter()
            .map(|(choice, label, n)| {
                let (ev, exact) = self.value(*n, results);
                let mut pv = vec![label.clone()];
                self.pv(*n, results, &mut pv);
                RootOption { choice: *choice, ev, exact, pv: pv.join(" \u{2192} ") }
            })
            .collect();
        options.sort_by(|a, b| b.ev.partial_cmp(&a.ev).unwrap_or(std::cmp::Ordering::Equal));
        Analysis {
            options,
            nodes: self.nodes.len() as u64 + results.iter().map(|r| r.nodes).sum::<u64>(),
            tt_hits: results.iter().map(|r| r.tt_hits).sum(),
            tt_stores: 0,
            elapsed,
        }
    }

    fn value(&self, n: usize, r: &[TaskResult]) -> (f64, bool) {
        match &self.nodes[n] {
            Node::Max(kids) => kids.iter().map(|(_, k)| self.value(*k, r)).fold((f64::NEG_INFINITY, true), |(best, ex), (v, e)| (best.max(v), ex && e)),
            Node::Chance(_, kids, _) => kids.iter().fold((0.0, true), |(sum, ex), (p, k)| {
                let (v, e) = self.value(*k, r);
                (sum + p * v, ex && e)
            }),
            Node::Forced(_, k) => self.value(*k, r),
            Node::Value(v, _) => (*v, true),
            Node::Task(i) => (r[*i].ev, r[*i].exact),
            Node::Open => unreachable!(),
        }
    }

    fn pv(&self, n: usize, r: &[TaskResult], out: &mut Vec<String>) {
        match &self.nodes[n] {
            Node::Max(kids) => {
                let (label, k) = kids
                    .iter()
                    .max_by(|a, b| self.value(a.1, r).0.partial_cmp(&self.value(b.1, r).0).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap();
                out.push(label.clone());
                self.pv(*k, r, out);
            }
            Node::Chance(label, _, likely) => {
                out.push(label.clone());
                self.pv(*likely, r, out);
            }
            Node::Forced(label, k) => {
                out.push(label.clone());
                self.pv(*k, r, out);
            }
            Node::Value(_, tail) => out.push(tail.to_string()),
            Node::Task(i) => out.extend(r[*i].pv.iter().cloned()),
            Node::Open => unreachable!(),
        }
    }
}

/// Analyze one position using `threads` OS threads (each with its own `Searcher`).
#[cfg(not(target_arch = "wasm32"))]
pub fn analyze_parallel<E: Evaluator + Sync>(
    state: &GameState,
    me: u8,
    cfg: &crate::search::SearchConfig,
    eval: &E,
    threads: usize,
) -> Analysis {
    use crate::search::Searcher;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let start = std::time::Instant::now();
    let threads = threads.max(1);
    let plan = Plan::build(state, me, threads * 8, eval);
    let next = AtomicUsize::new(0);
    let mut results: Vec<Option<TaskResult>> = vec![None; plan.tasks.len()];
    let done: Vec<Vec<(usize, TaskResult)>> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                sc.spawn(|| {
                    let mut searcher = Searcher::new(cfg.tt_bits);
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= plan.tasks.len() {
                            break;
                        }
                        out.push((i, searcher.evaluate(&plan.root, &plan.tasks[i], me, cfg, eval)));
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (i, r) in done.into_iter().flatten() {
        results[i] = Some(r);
    }
    let results: Vec<TaskResult> = results.into_iter().map(|r| r.unwrap()).collect();
    plan.finish(&results, start.elapsed())
}
