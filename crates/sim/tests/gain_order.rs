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
fn five_throne_rooms_still_remodel_both_golds() {
    // The bot searches its play order; whatever line it picks must turn both Golds into
    // Provinces (its top gain priority).
    let (played, g) = bot_action_plays("5 Throne Room Gold Gold Remodel Village");
    assert!(played.contains(&id::REMODEL), "{played:?}");
    assert_eq!(g.trash.get(id::GOLD), 2);
    assert_eq!(g.players[0].discard.get(id::PROVINCE), 2);
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

/// Play player 1's whole turn with the strategy `src`; returns the state afterwards.
fn play_turn(src: &str, text: &str) -> dominion_engine::GameState {
    let strat = Strategy::parse(src).unwrap();
    let mut g = parse_state(text).unwrap();
    let mut buf = ChoiceBuf::default();
    loop {
        match g.advance(&mut NoEvents) {
            Step::GameOver | Step::TurnStart { .. } => return g,
            Step::Decision(d) => {
                if g.turn.player != 0 {
                    return g;
                }
                g.legal_choices(&mut buf);
                let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
                g.apply(c, &mut NoEvents).unwrap();
            }
            s => panic!("{s:?}"),
        }
    }
}

fn read_strategy(file: &str) -> String {
    std::fs::read_to_string(format!("{}/../../strategies/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// A shipped strategy with the (opt-in) `win_this_turn` lookahead turned on. Top-level keys must
/// precede the first [[table]].
fn with_win_check(file: &str) -> String {
    read_strategy(file).replacen("[[", "win_this_turn = true\n\n[[", 1)
}

const DUCHY_WIN: &str = "players: 2\nkingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop\n\
    supply: Silver=0, Estate=0, Duchy=2, Province=3\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
    [player 1]\nhand: Throne Room Gold Gold Remodel Village Village\ndeck: 2 Copper, 3 Estate\n\n\
    [player 2]\nhand: 3 Copper, 2 Estate\ndeck: 4 Copper, Estate\n";

#[test]
fn win_this_turn_finds_the_two_duchy_win() {
    // Duchy is on Double Witch's list (provinces_left <= 4 holds), and two Duchies empty the
    // third pile while ahead: the bot wins this turn instead of remodeling into Provinces.
    let g = play_turn(&with_win_check("double_witch.toml"), DUCHY_WIN);
    assert!(g.is_game_over(), "game should end this turn");
    assert_eq!(g.winners(), 1, "player 1 wins outright");
    assert_eq!(g.supply.get(id::DUCHY), 0);
}

#[test]
fn win_this_turn_is_off_by_default() {
    let g = play_turn(&read_strategy("double_witch.toml"), DUCHY_WIN);
    assert!(!g.is_game_over(), "without the rule it follows its list (Gold -> Province twice)");
    assert_eq!(g.players[0].all_cards().get(id::PROVINCE), 2);
}

#[test]
fn win_this_turn_never_acquires_unlisted_cards() {
    // Big Money's list has no Duchy: the Duchy win exists but needs unlisted cards, so no
    // certain win is found and it follows its list.
    let g = play_turn(&with_win_check("big_money.toml"), DUCHY_WIN);
    assert!(!g.is_game_over());
    assert_eq!(g.players[0].all_cards().get(id::DUCHY), 0);
}

#[test]
fn analysis_ranks_the_rules_pick_first_when_win_this_turn_applies() {
    // The bot plays Remodel (certain win: Throne Room -> Duchy, buy the last Duchy); the
    // strategy-scored analysis must rank that same move first, not Throne Room -> 2 Provinces.
    let text = "players: 2\nkingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop\n\
                supply: Gold=0, Estate=0, Duchy=2, Province=4\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
                [player 1]\nhand: Gold Gold Throne Room Remodel\ndeck: 2 Copper, 3 Estate\n\n\
                [player 2]\nhand: 3 Copper, 2 Estate\ndeck: 4 Copper, Estate\n";
    let strat = Strategy::parse(&with_win_check("double_witch.toml")).unwrap();
    let mut g = parse_state(text).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let pick = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(pick, Choice::Card(id::REMODEL));
    let a = dominion_search::analyze(&g, 0, &dominion_search::SearchConfig::default(), &dominion_sim::GainListEvaluator::new(&strat));
    assert_eq!(a.best().choice, pick, "analysis best = rules' pick; got {:?}", a.options);
    assert!(a.best().pv.contains("Gain Duchy") && a.best().pv.contains("Buy Duchy"), "{}", a.best().pv);
}

#[test]
fn non_terminal_actions_are_played_before_stated_terminals() {
    // Double Witch states [[play]] Witch, but Village gives the action back: Village first keeps
    // every option open, then Witch (stated), then Chapel with the spare action.
    let (played, g) = bot_action_plays("Chapel Curse Witch Village");
    assert_eq!(played, vec![id::VILLAGE, id::WITCH, id::CHAPEL]);
    assert_eq!(g.trash.get(id::CURSE), 1, "Chapel trashes the Curse (default trash rule)");
}

#[test]
fn second_copy_is_scored_with_its_condition_rechecked() {
    // Double Witch wants Witch only while count(Witch) < 2. Owning one, a turn that gains two
    // Witches must score the second at zero (the rule no longer applies), not 2x.
    use dominion_search::Evaluator;
    let strat = Strategy::parse(&read_strategy("double_witch.toml")).unwrap();
    let text = "players: 2\nkingdom: Witch\nturn: 5  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n\
                [player 1]\nhand: Witch\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Copper\n";
    let root = parse_state(text).unwrap();
    let eval = dominion_sim::GainListEvaluator::new(&strat);
    let gain = |n: u8| {
        let mut leaf = root;
        leaf.players[0].discard.add(id::WITCH, n);
        leaf.supply.set(id::WITCH, 10 - n);
        eval.leaf_value(&root, &leaf, 0)
    };
    let one = gain(1) - gain(0);
    let two = gain(2) - gain(0);
    assert!(one > 1000.0, "first Witch counts: {one}");
    assert!((two - one).abs() < 1.0, "second Witch adds (almost) nothing: one {one}, two {two}");
}

#[test]
fn big_hand_game_plays_to_the_end_without_panicking() {
    // Regression: playouts past the node budget asked the action-play rule for every decision and
    // answered "Pass" at a gain (no Pass there) -> "illegal choice" panic. Play the reported
    // position to the end with several strategy pairings.
    let text = "players: 2\nkingdom: Witch\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\nseed: 17600139498230406601\n\n\
                [player 1]\nhand: Bandit, Council Room, Festival, Laboratory, Library, 2 Market, Mine, Sentry, Witch\ndeck: 4 Copper, Estate\n\n\
                [player 2]\nhand: 4 Copper, Estate\ndeck: 3 Copper, 2 Estate\n";
    for (a, b) in [("double_witch.toml", "big_money_ultimate.toml"), ("big_money_ultimate.toml", "double_witch.toml"), ("chapel_witch.toml", "smithy_bm.toml")] {
        let sa = Strategy::parse(&read_strategy(a)).unwrap();
        let sb = Strategy::parse(&read_strategy(b)).unwrap();
        for seed in 0..3u64 {
            let mut g = parse_state(text).unwrap();
            g.rng = dominion_engine::rng::Rng::new(seed);
            let mut ga = dominion_sim::StrategyAgent::new(&sa);
            let mut gb = dominion_sim::StrategyAgent::new(&sb);
            let r = dominion_engine::play_game(&mut g, &mut [&mut ga, &mut gb], &mut NoEvents);
            assert!(g.is_game_over() && r.winners != 0);
        }
    }
}

fn workshop_gain(src: &str) -> Choice {
    let strat = Strategy::parse(src).unwrap();
    let text = "players: 2\nkingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop\n\
         turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\nhand: Workshop 4 Copper\ndeck: Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::WORKSHOP), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice())
}

#[test]
fn forced_gains_never_take_never_gain_cards() {
    // Nothing listed costs <= 4, so the fallback takes the most expensive legal card...
    let base = "name = \"P\"\n[[gain]]\ncard = \"Province\"\n";
    assert_eq!(workshop_gain(base), Choice::Card(id::MILITIA));
    // ...but never one the strategy forbids.
    let src = "name = \"P\"
never_gain = [\"Militia\", \"Remodel\", \"Smithy\"]
[[gain]]
card = \"Province\"
";
    assert_eq!(workshop_gain(src), Choice::Card(id::SILVER));
    assert!(Strategy::parse("name = \"x\"\nnever_gain = [\"Nope\"]\n").is_err());
}
