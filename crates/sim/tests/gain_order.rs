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

fn buy_decision_src(src: &str, state_text: &str) -> (Choice, Vec<String>) {
    let strat = Strategy::parse(src).unwrap();
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

fn buy_decision(strategy_file: &str, state_text: &str) -> (Choice, Vec<String>) {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{strategy_file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    buy_decision_src(&src, state_text)
}

fn endgame(provinces: u8, p1_hand: &str, p2_extra_vp: &str) -> String {
    format!(
        "players: 2
kingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop
         supply: Province={provinces}
turn: 30  player: 1  phase: buy  actions: 0  buys: 1  coins: 0

         [player 1]
hand: {p1_hand}
deck: 5 Copper
turns: 14

         [player 2]
hand: 5 Copper
deck: 5 Copper
discard: {p2_extra_vp}
turns: 15
"
    )
}

#[test]
fn bots_follow_their_list_even_when_it_ends_the_game_on_a_loss() {
    // Behind 0-18 with one Province left: Big Money's list says Province at $8, so it buys it
    // (and loses). No hidden "don't lose" override.
    let (pick, offered) = buy_decision("big_money.toml", &endgame(1, "Gold Gold Silver", "3 Province"));
    assert_eq!(pick, Choice::Card(id::PROVINCE));
    assert!(offered.contains(&format!("{:?}", Choice::Card(id::PROVINCE))));
}

#[test]
fn rules_can_ask_to_avoid_ending_the_game_badly() {
    let src = r#"
        name = "Careful Big Money"
        [[gain]]
        card = "Province"
        if = "not loses_game"
        [[gain]]
        card = "Gold"
        [[gain]]
        card = "Silver"
    "#;
    // Losing ending: skips the Province, takes Gold (next entry).
    let (pick, offered) = buy_decision_src(src, &endgame(1, "Gold Gold Silver", "3 Province"));
    assert_eq!(pick, Choice::Card(id::GOLD));
    assert!(!offered.contains(&format!("{:?}", Choice::Card(id::PROVINCE))), "{offered:?}");
    // Tied ending (6-6, equal turns = shared win) also counts as not winning.
    let (pick, _) = buy_decision_src(src, &endgame(1, "Gold Gold Silver", "Province"));
    assert_eq!(pick, Choice::Card(id::GOLD));
    // Winning ending (6-3): buys it.
    let (pick, _) = buy_decision_src(src, &endgame(1, "Gold Gold Silver", "Duchy"));
    assert_eq!(pick, Choice::Card(id::PROVINCE));
}

#[test]
fn rules_can_ask_to_take_a_game_winning_card() {
    // Estate normally never bought; `wins_game` makes it the top priority when it ends the game
    // with a win. One Province left is not affordable ($5), but the Estate pile's last card
    // ends the game on piles.
    let src = r#"
        name = "Closer"
        [[gain]]
        card = "Estate"
        if = "wins_game"
        [[gain]]
        card = "Silver"
    "#;
    let text = "players: 2
kingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop
                supply: Estate=1, Cellar=0, Moat=0
turn: 30  player: 1  phase: buy  actions: 0  buys: 1  coins: 0

                [player 1]
hand: Gold Silver
deck: 5 Copper
discard: Duchy
turns: 14

                [player 2]
hand: 5 Copper
deck: 5 Copper
discard: Duchy
turns: 15
";
    let (pick, _) = buy_decision_src(src, text);
    assert_eq!(pick, Choice::Card(id::ESTATE), "4 VP vs 3 VP after the Estate: wins");
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
