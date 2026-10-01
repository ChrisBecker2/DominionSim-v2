//! The search as a graph, for visualization: every distinct position the search valued, with
//! every route into it. Built *after* a normal analysis by walking the search's own cached values
//! (the transposition table), so the search itself is untouched and pays nothing for this.
//!
//! - **Nodes** are positions with a real choice or a draw pending, or a turn's end. Positions are
//!   merged exactly as the search merges them (`turn_hash`), so a position reached by several
//!   orders of play is one node with several incoming edges: a transposition.
//! - **Forced steps** (a single legal choice, or a choice the evaluator's policy fixes, such as an
//!   opponent's reaction) are folded into the label of the edge that leads through them.
//! - **Breadth-first** up to `max_nodes`, so the earliest plies are complete; nodes left
//!   unexpanded at the cap are marked.

use std::collections::{HashMap, VecDeque};

use crate::eval::Evaluator;
use crate::hash::turn_hash;
use crate::search::{describe, fixed_choice, is_leaf, SearchConfig, Searcher};
use dominion_engine::cards::CardId;
use dominion_engine::{Choice, ChoiceBuf, GameState, NoEvents, Step};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// The analyzed decision.
    Root,
    /// The searching player chooses among several options.
    Decision,
    /// A card is drawn or revealed from an unknown deck.
    Chance,
    /// The turn (or game) ends here; scored by the evaluator.
    Leaf,
}

/// A position's visible essentials, for labels and tooltips.
#[derive(Clone, Debug)]
pub struct NodeState {
    pub hand: Vec<(CardId, u8)>,
    pub in_play: Vec<(CardId, u8)>,
    pub coins: u16,
    pub actions: u8,
    pub buys: u8,
    /// Cards gained this turn so far (supply decreases, by card).
    pub gained: Vec<(CardId, u8)>,
}

#[derive(Clone, Debug)]
pub struct GraphNode {
    pub kind: NodeKind,
    /// Steps from the root along the shortest route (each edge is one real choice or draw).
    pub depth: u16,
    /// The search's value for the searching player.
    pub value: f64,
    pub exact: bool,
    /// Distinct incoming edges: more than one means the position was reached by different routes.
    pub in_edges: u32,
    /// False when the node cap stopped exploration below this node.
    pub expanded: bool,
    /// On the best line: reached by best choices from the root (all draws along it count).
    pub best: bool,
    pub state: NodeState,
}

#[derive(Clone, Debug)]
pub struct GraphEdge {
    pub from: u32,
    pub to: u32,
    /// "Play Smithy", "draw Copper", plus any forced steps folded in after it.
    pub label: String,
    /// Probability, for draws.
    pub prob: Option<f64>,
    /// The best choice at its decision (for draws: part of a best line).
    pub best: bool,
}

#[derive(Clone, Debug)]
pub struct SearchGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// True if `max_nodes` stopped exploration.
    pub truncated: bool,
    /// The analysis it was built from: nodes searched, and how many were answered from the table.
    pub searched_nodes: u64,
    pub tt_hits: u64,
}

fn counts_list(c: &dominion_engine::Counts) -> Vec<(CardId, u8)> {
    c.iter().collect()
}

fn node_state(root: &GameState, s: &GameState) -> NodeState {
    let p = s.turn.player as usize;
    let mut gained = Vec::new();
    for c in s.supply_cards() {
        let before = root.supply.get(c);
        let after = s.supply.get(c);
        if before > after {
            gained.push((c, before - after));
        }
    }
    NodeState {
        hand: counts_list(&s.players[p].hand),
        in_play: counts_list(&s.players[p].in_play),
        coins: s.turn.coins,
        actions: s.turn.actions,
        buys: s.turn.buys,
        gained,
    }
}

impl Searcher {
    /// Analyze `state` for `me` (as `analyze` does), then walk the valued positions into a graph
    /// of at most `max_nodes` nodes.
    pub fn search_graph<E: Evaluator>(&mut self, state: &GameState, me: u8, cfg: &SearchConfig, eval: &E, max_nodes: usize) -> SearchGraph {
        let analysis = self.analyze(state, me, cfg, eval);
        let mut root = *state;
        root.chance_mode = true;
        root.pause_at_turn_start = false;

        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut index: HashMap<u64, u32> = HashMap::new();
        // Positions to expand: (node id, the position with its next step not yet taken).
        let mut queue: VecDeque<(u32, GameState)> = VecDeque::new();
        let mut truncated = false;

        // The root: the pending decision itself.
        let root_value = analysis.options.first().map_or(0.0, |o| o.ev);
        let root_exact = analysis.options.iter().all(|o| o.exact);
        nodes.push(GraphNode {
            kind: NodeKind::Root,
            depth: 0,
            value: root_value,
            exact: root_exact,
            in_edges: 0,
            expanded: true,
            best: true,
            state: node_state(&root, &root),
        });
        let d = root.pending_decision().expect("search_graph: no pending decision");
        let mut buf = ChoiceBuf::default();
        root.legal_choices(&mut buf);
        let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&root, me, &d, c));
        for &choice in buf.as_slice() {
            if any_allowed && !eval.allows(&root, me, &d, choice) {
                continue;
            }
            let mut child = root;
            child.apply(choice, &mut NoEvents).expect("search_graph: root choice rejected");
            let label = describe(&d, choice);
            self.link(&root, me, cfg, eval, 0, child, label, None, 1, max_nodes, &mut nodes, &mut edges, &mut index, &mut queue, &mut truncated);
        }

        while let Some((id, s)) = queue.pop_front() {
            let depth = nodes[id as usize].depth + 1;
            let mut t = s;
            match t.advance(&mut NoEvents) {
                Step::Chance { player } => {
                    let outcomes = t.chance_outcomes(player);
                    let total = outcomes.total() as f64;
                    for (card, n) in outcomes.iter() {
                        let mut child = t;
                        child.resolve_chance(player, card);
                        let label = format!("draw {}", dominion_engine::cards::name(card));
                        let prob = Some(n as f64 / total);
                        self.link(&root, me, cfg, eval, id, child, label, prob, depth, max_nodes, &mut nodes, &mut edges, &mut index, &mut queue, &mut truncated);
                    }
                }
                Step::Decision(d) => {
                    let mut buf = ChoiceBuf::default();
                    t.legal_choices(&mut buf);
                    let any_allowed = buf.as_slice().iter().any(|&c| eval.allows(&t, me, &d, c));
                    for &choice in buf.as_slice() {
                        if any_allowed && !eval.allows(&t, me, &d, choice) {
                            continue;
                        }
                        let mut child = t;
                        child.apply(choice, &mut NoEvents).expect("search_graph: legal choice rejected");
                        let label = describe(&d, choice);
                        self.link(&root, me, cfg, eval, id, child, label, None, depth, max_nodes, &mut nodes, &mut edges, &mut index, &mut queue, &mut truncated);
                    }
                }
                Step::GameOver | Step::TurnStart { .. } => {}
            }
        }

        mark_best(&mut nodes, &mut edges);
        SearchGraph { nodes, edges, truncated, searched_nodes: analysis.nodes, tt_hits: analysis.tt_hits }
    }

    /// Follow `child` through forced steps to the next real choice, draw or end of turn, then add
    /// an edge from `from` to that position's node (creating it if new).
    #[allow(clippy::too_many_arguments)]
    fn link<E: Evaluator>(
        &mut self,
        root: &GameState,
        me: u8,
        cfg: &SearchConfig,
        eval: &E,
        from: u32,
        child: GameState,
        mut label: String,
        prob: Option<f64>,
        depth: u16,
        max_nodes: usize,
        nodes: &mut Vec<GraphNode>,
        edges: &mut Vec<GraphEdge>,
        index: &mut HashMap<u64, u32>,
        queue: &mut VecDeque<(u32, GameState)>,
        truncated: &mut bool,
    ) {
        // Settle: apply forced choices, folding them into the label.
        let mut s = child;
        let kind = loop {
            if is_leaf(&s) {
                break NodeKind::Leaf;
            }
            let mut t = s;
            match t.advance(&mut NoEvents) {
                Step::GameOver | Step::TurnStart { .. } => {
                    s = t;
                    break NodeKind::Leaf;
                }
                Step::Chance { .. } => break NodeKind::Chance,
                Step::Decision(d) => {
                    let mut buf = ChoiceBuf::default();
                    t.legal_choices(&mut buf);
                    let forced: Option<Choice> = if buf.len() == 1 { Some(buf.as_slice()[0]) } else { fixed_choice(eval, &t, me, &d, buf.as_slice()) };
                    match forced {
                        Some(c) => {
                            label.push_str(" \u{2192} ");
                            label.push_str(&describe(&d, c));
                            t.apply(c, &mut NoEvents).expect("search_graph: forced choice rejected");
                            s = t;
                        }
                        None => break NodeKind::Decision,
                    }
                }
            }
        };
        let key = turn_hash(&s, me) ^ (kind as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let to = match index.get(&key) {
            Some(&id) => {
                nodes[id as usize].in_edges += 1;
                id
            }
            None => {
                if nodes.len() >= max_nodes {
                    *truncated = true;
                    nodes[from as usize].expanded = false;
                    return;
                }
                let (value, exact) = if kind == NodeKind::Leaf {
                    (eval.leaf_value(root, &s, me), true)
                } else {
                    let r = self.node_value_pub(root, &s, me, cfg, eval);
                    (r.0, r.1)
                };
                let id = nodes.len() as u32;
                nodes.push(GraphNode { kind, depth, value, exact, in_edges: 1, expanded: kind == NodeKind::Leaf, best: false, state: node_state(root, &s) });
                index.insert(key, id);
                if kind != NodeKind::Leaf {
                    nodes[id as usize].expanded = true;
                    queue.push_back((id, s));
                }
                id
            }
        };
        edges.push(GraphEdge { from, to, label, prob, best: false });
    }
}

/// Mark the best lines: from the root, the best choice at every decision and every draw outcome.
fn mark_best(nodes: &mut [GraphNode], edges: &mut [GraphEdge]) {
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (i, e) in edges.iter().enumerate() {
        out[e.from as usize].push(i);
    }
    let mut stack = vec![0usize];
    let mut seen = vec![false; nodes.len()];
    while let Some(n) = stack.pop() {
        if seen[n] {
            continue;
        }
        seen[n] = true;
        nodes[n].best = true;
        match nodes[n].kind {
            NodeKind::Chance => {
                for &e in &out[n] {
                    edges[e].best = true;
                    stack.push(edges[e].to as usize);
                }
            }
            NodeKind::Root | NodeKind::Decision => {
                // The best child by value (ties: first).
                let best = out[n].iter().copied().max_by(|&a, &b| {
                    nodes[edges[a].to as usize].value.partial_cmp(&nodes[edges[b].to as usize].value).unwrap_or(std::cmp::Ordering::Equal).then(b.cmp(&a))
                });
                if let Some(e) = best {
                    edges[e].best = true;
                    stack.push(edges[e].to as usize);
                }
            }
            NodeKind::Leaf => {}
        }
    }
}
