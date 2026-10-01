//! Actions that can cost more than they give are weighed, not just played: a lone playable
//! Action that isn't choice-free is searched against not playing it, and the rule order (no
//! search) has defaults for Tactician and Treasure Map.

use dominion_engine::cards::id;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, GameState, NoEvents, PlayerView, Step};
use dominion_search::expected_next_hand_money;
use dominion_sim::Strategy;

const BM: &str = "name = \"T\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n";

fn strategies() -> [Strategy; 2] {
    [Strategy::parse(BM).unwrap(), Strategy::parse(&BM.replace("name = \"T\"\n", "name = \"T\"\nsearch_play = false\n")).unwrap()]
}

fn state(hand: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: Tactician, Treasure Map, Village, Smithy, Market, Militia, Cellar, Moat, Festival, Laboratory\n\
         turn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\nhand: {hand}\ndeck: 4 Copper, 2 Silver, Gold, 3 Estate\ndiscard: 3 Copper\n\n\
         [player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    parse_state(&text).unwrap()
}

/// The first action-phase choice of each strategy (search on, rules only).
fn picks(hand: &str) -> Vec<Choice> {
    strategies()
        .iter()
        .map(|s| {
            let mut g = state(hand);
            let d = match g.advance(&mut NoEvents) {
                Step::Decision(d) => d,
                s => panic!("{s:?}"),
            };
            let mut buf = ChoiceBuf::default();
            g.legal_choices(&mut buf);
            s.decide(&PlayerView::new(&g, 0), &d, buf.as_slice())
        })
        .collect()
}

#[test]
fn tactician_is_not_played_over_a_hand_that_buys_something() {
    assert_eq!(picks("Tactician, Gold, Gold, Silver, Copper"), [Choice::Pass, Choice::Pass], "keep the Province");
    assert_eq!(picks("Tactician, Silver, Copper, Estate, Estate"), [Choice::Pass, Choice::Pass], "keep the Silver");
    let t = Choice::Card(id::TACTICIAN);
    assert_eq!(picks("Tactician, Copper, Estate, Estate, Estate"), [t, t], "$1 buys nothing listed: discard for +5 Cards");
}

#[test]
fn a_lone_treasure_map_is_not_played_but_a_pair_is() {
    assert_eq!(picks("Treasure Map, Copper, Copper, Estate, Estate"), [Choice::Pass, Choice::Pass]);
    let m = Choice::Card(id::TREASURE_MAP);
    assert_eq!(picks("Treasure Map, Treasure Map, Copper, Estate, Estate"), [m, m]);
}

#[test]
fn the_next_hand_counts_durations_played_this_turn() {
    let mut g = state("Tactician, Copper, Estate, Estate, Estate");
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let before = expected_next_hand_money(&g.players[0]);
    g.apply(Choice::Card(id::TACTICIAN), &mut NoEvents).unwrap();
    let after = expected_next_hand_money(&g.players[0]);
    // 10 cards in the deck worth $11: a 5-card hand expects $5.50, a 10-card hand (Tactician) $11.
    assert!((before - 5.5).abs() < 1e-9 && (after - 11.0).abs() < 1e-9, "{before} -> {after}");
}
