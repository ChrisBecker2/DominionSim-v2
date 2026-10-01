//! The search graph export: positions merged like the search merges them, with every route in.

use dominion_engine::text::parse_state;
use dominion_engine::{GameState, NoEvents, Step};
use dominion_search::*;

fn scenario(hand: &str, deck_top: &str, deck: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: Village, Smithy, Market, Laboratory, Cellar, Moat, Militia, Festival, Library, Mine\n\
         turn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\nseed: 1\n\n\
         [player 1]\nhand: {hand}\ndeck top: {deck_top}\ndeck: {deck}\ndiscard:\nin play:\n\n\
         [player 2]\nhand: 5 Copper\ndeck top:\ndeck: 2 Copper, 3 Estate\ndiscard:\nin play:\n"
    );
    let mut g = parse_state(&text).unwrap();
    g.chance_mode = true;
    match g.advance(&mut NoEvents) {
        Step::Decision(_) => g,
        s => panic!("expected a decision, got {s:?}"),
    }
}

fn graph(g: &GameState, max_nodes: usize) -> SearchGraph {
    Searcher::new(16).search_graph(g, 0, &SearchConfig::default(), &NextHandEvaluator::default(), max_nodes)
}

#[test]
fn two_play_orders_that_reach_the_same_position_share_a_node() {
    // Village then Market, or Market then Village, with known draws: the same final position.
    let g = scenario("Village, Market, Copper", "Copper, Copper", "");
    let sg = graph(&g, 1000);
    assert_eq!(sg.nodes[0].kind, NodeKind::Root);
    assert!(!sg.truncated);
    let merged = sg.nodes.iter().filter(|n| n.in_edges >= 2).count();
    assert!(merged >= 1, "a position reached by both orders: {:#?}", sg.nodes);
    // Every edge points at real nodes; the root has one edge per legal option.
    assert!(sg.edges.iter().all(|e| (e.from as usize) < sg.nodes.len() && (e.to as usize) < sg.nodes.len()));
    assert_eq!(sg.edges.iter().filter(|e| e.from == 0).count(), 3, "Play Village, Play Market, Done");
    // Exactly one best choice at the root, and the best line reaches a leaf.
    assert_eq!(sg.edges.iter().filter(|e| e.from == 0 && e.best).count(), 1);
    assert!(sg.nodes.iter().any(|n| n.best && n.kind == NodeKind::Leaf));
}

#[test]
fn draws_become_chance_nodes_with_probabilities_summing_to_one() {
    let g = scenario("Smithy, Copper", "", "3 Copper, 2 Estate, Gold");
    let sg = graph(&g, 2000);
    let chance = sg.nodes.iter().position(|n| n.kind == NodeKind::Chance).expect("a chance node");
    let total: f64 = sg.edges.iter().filter(|e| e.from as usize == chance).filter_map(|e| e.prob).sum();
    assert!((total - 1.0).abs() < 1e-9, "{total}");
}

#[test]
fn the_node_cap_truncates_breadth_first() {
    let g = scenario("Smithy, Village, Market, Laboratory, Festival", "", "10 Copper, 5 Estate, 3 Gold, 2 Silver");
    let sg = graph(&g, 40);
    assert!(sg.truncated);
    assert_eq!(sg.nodes.len(), 40);
    assert!(sg.nodes.iter().any(|n| !n.expanded));
    // Breadth-first: depths never decrease in creation order.
    assert!(sg.nodes.windows(2).all(|w| w[0].depth <= w[1].depth));
}
