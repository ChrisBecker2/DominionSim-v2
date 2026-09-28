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

fn bot_action_plays(hand: &str) -> (Vec<dominion_engine::CardId>, dominion_engine::GameState) {
    use dominion_engine::Phase;
    let src = std::fs::read_to_string(format!("{}/../../strategies/double_witch.toml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let strat = Strategy::parse(&src).unwrap();
    let text = format!(
        "players: 2
kingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop
         turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0

         [player 1]
hand: {hand}
deck: 2 Copper, 3 Estate

[player 2]
hand: 3 Copper, 2 Estate
deck: 4 Copper, Estate
"
    );
    let mut g = parse_state(&text).unwrap();
    let mut played = Vec::new();
    let mut buf = ChoiceBuf::default();
    loop {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        if g.turn.phase != Phase::Action {
            break;
        }
        g.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
        if let Choice::Card(card) = c {
            if d.kind == dominion_engine::DecisionKind::PlayAction
                || matches!(d.kind, dominion_engine::DecisionKind::Select { act: dominion_engine::Act::Play, .. })
            {
                played.push(card);
            }
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    (played, g)
}

#[test]
fn throne_room_does_not_chain_into_throne_rooms_without_targets() {
    // 5 Throne Rooms but only one ordinary action (Remodel) left after Village: play Village,
    // one Throne Room on Remodel, and leave the other Throne Rooms (nothing to double).
    let (played, g) = bot_action_plays("5 Throne Room Gold Gold Remodel Village");
    assert_eq!(played, vec![id::VILLAGE, id::THRONE_ROOM, id::REMODEL]);
    assert_eq!(g.players[0].discard.get(id::PROVINCE), 2);
    assert_eq!(g.players[0].hand.get(id::THRONE_ROOM), 4);
}

#[test]
fn throne_room_is_played_before_remodel_to_double_it() {
    use dominion_engine::Phase;
    let src = std::fs::read_to_string(format!("{}/../../strategies/double_witch.toml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let strat = Strategy::parse(&src).unwrap();
    let text = "players: 2\nkingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop\n\
                turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
                [player 1]\nhand: Village Remodel Gold Gold Throne Room\ndeck: 2 Copper, 3 Estate\n\n\
                [player 2]\nhand: 3 Copper, 2 Estate\ndeck: 4 Copper, Estate\n";
    let mut g = parse_state(text).unwrap();
    let mut played = Vec::new();
    let mut buf = ChoiceBuf::default();
    while g.turn.phase == Phase::Action {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        if g.turn.phase != Phase::Action {
            break;
        }
        g.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
        if let (dominion_engine::DecisionKind::PlayAction, Choice::Card(card)) = (d.kind, c) {
            played.push(card);
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    assert_eq!(played, vec![id::VILLAGE, id::THRONE_ROOM], "Village, then Throne Room (which plays Remodel twice)");
    assert_eq!(g.trash.get(id::GOLD), 2, "both Golds remodeled");
    assert_eq!(g.players[0].discard.get(id::PROVINCE), 2, "into two Provinces");
}

fn buy_decision(strategy_file: &str, state_text: &str) -> (Choice, Vec<String>) {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{strategy_file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let strat = Strategy::parse(&src).unwrap();
    let mut g = parse_state(state_text).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, dominion_engine::DecisionKind::Buy);
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let pick = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    // What the analysis would consider for this seat.
    let eval = dominion_sim::GainListEvaluator::new(&strat);
    let a = dominion_search::analyze(&g, 0, &dominion_search::SearchConfig::default(), &eval);
    let offered = a.options.iter().map(|o| format!("{:?}", o.choice)).collect();
    (pick, offered)
}

fn endgame(provinces: u8, p1_hand: &str, p2_extra_vp: &str) -> String {
    format!(
        "players: 2\nkingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop\n\
         supply: Province={provinces}\nturn: 30  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n\
         [player 1]\nhand: {p1_hand}\ndeck: 5 Copper\nturns: 14\n\n\
         [player 2]\nhand: 5 Copper\ndeck: 5 Copper\ndiscard: {p2_extra_vp}\nturns: 15\n"
    )
}

#[test]
fn bot_takes_the_game_winning_buy_over_its_list() {
    // Behind 0-3 with $8 and one Province left: buying it ends the game 6-3 -> must buy it.
    let (pick, _) = buy_decision("big_money.toml", &endgame(1, "Gold Gold Silver", "Duchy"));
    assert_eq!(pick, Choice::Card(id::PROVINCE));
    // Double Witch too, whatever its list says.
    let (pick, _) = buy_decision("double_witch.toml", &endgame(1, "Gold Gold Silver", "Duchy"));
    assert_eq!(pick, Choice::Card(id::PROVINCE));
}

#[test]
fn bot_does_not_end_the_game_on_a_tie() {
    // 6-6 on equal turns would be a shared win: keep playing instead.
    let (pick, _) = buy_decision("big_money.toml", &endgame(1, "Gold Gold Silver", "Province"));
    assert_ne!(pick, Choice::Card(id::PROVINCE));
}

#[test]
fn bot_avoids_ending_the_game_on_a_loss() {
    // Last Province while behind by 12: buying it ends the game on a loss -> don't.
    let (pick, offered) = buy_decision("big_money.toml", &endgame(1, "Gold Gold Silver", "3 Province"));
    assert_ne!(pick, Choice::Card(id::PROVINCE));
    assert!(!offered.contains(&format!("{:?}", Choice::Card(id::PROVINCE))), "{offered:?}");
}

#[test]
fn analysis_only_offers_listed_buys_or_done() {
    // $8, mid-game: Big Money's list is Province/Gold/Silver; nothing else may be offered.
    let (_, offered) = buy_decision("big_money.toml", &endgame(8, "Gold Gold Silver", ""));
    let allowed = [Choice::Card(id::PROVINCE), Choice::Card(id::GOLD), Choice::Card(id::SILVER), Choice::Pass];
    for o in &offered {
        assert!(allowed.iter().any(|a| &format!("{a:?}") == o), "unlisted buy offered: {o} in {offered:?}");
    }
    assert!(offered.len() >= 2);
}
