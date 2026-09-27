//! The gain list is followed strictly for upgrades (Remodel/Mine), with no hidden card rules.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Choice, ChoiceBuf, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

fn remodel_pick(strategy_file: &str, hand: &str) -> Choice {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{strategy_file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let strat = Strategy::parse(&src).unwrap();
    let text = format!(
        "players: 2\nkingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop\n\
         turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\nhand: {hand}\ndeck: Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::REMODEL), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert!(d.upgrade.is_some(), "Remodel's trash decision reports its upgrade");
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice())
}

#[test]
fn remodel_follows_gain_order_strictly() {
    // Gold (+2 -> $8) unlocks Province, the top of Double Witch's gain list.
    assert_eq!(remodel_pick("double_witch.toml", "Remodel Gold 3 Estate Copper"), Choice::Card(id::GOLD));
    // Without a Gold, Estate (+2 -> $4) unlocks Silver; Copper would only reach $2 (nothing listed).
    assert_eq!(remodel_pick("double_witch.toml", "Remodel 3 Estate Copper"), Choice::Card(id::ESTATE));
}
