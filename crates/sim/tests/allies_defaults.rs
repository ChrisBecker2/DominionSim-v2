//! Bot defaults for Allies: the bots play every card and Ally to the end of a game without an illegal choice, and a few decisions
//! (rulings and bot policy in `docs/allies-plan.md`).

use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::sets::allies::ids as a;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, Decision, DecisionKind, NoEvents, PlayerView, Step};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

/// A strategy that gains two of each of the given kingdom cards, then Big Money.
fn gainer(in_kingdom: &[u16]) -> Strategy {
    let mut s = String::from("name = \"Gainer\"\n\n[[gain]]\ncard = \"Province\"\n\n[[gain]]\ncard = \"Duchy\"\nif = \"provinces_left <= 3\"\n\n");
    for &c in in_kingdom {
        if cards::is_kingdom(c) {
            s += &format!("[[gain]]\ncard = \"{}\"\nif = \"count({}) < 2\"\n\n", cards::name(c), cards::name(c));
        }
    }
    s += "[[gain]]\ncard = \"Gold\"\n\n[[gain]]\ncard = \"Silver\"\n";
    Strategy::parse(&s).unwrap_or_else(|e| panic!("{e}\n{s}"))
}

fn names(cs: &[u16]) -> Vec<&'static str> {
    cs.iter().map(|&c| cards::name(c)).collect()
}

#[test]
fn bots_play_every_card_and_ally_to_the_end() {
    let bmu = load("big_money_ultimate.toml");
    let al: Vec<u16> = cards::kingdom_cards_in(CardSet::Allies).collect();
    assert_eq!(al.len(), 31);
    let allies: Vec<u16> = (cards::FIRST_ALLY..cards::FIRST_ALLY + cards::NUM_ALLIES as u16).collect();
    assert_eq!(allies.len(), 23);
    // (The bots search the turn: games with many cards that make choices are slow, so the games are few and the strategies buy only a few of the cards.)
    for round in 0..8 {
        let mut kingdom: Vec<u16> = al.iter().copied().cycle().skip(round * 4).take(6).collect();
        for extra in [id::WITCH, id::CHAPEL, id::MILITIA, id::THRONE_ROOM] {
            if !kingdom.contains(&extra) {
                kingdom.push(extra);
            }
        }
        let mut kd = kingdom.clone();
        kd.push(allies[round * 3 % allies.len()]);
        let s = gainer(&kd[..4]);
        let cfg = MatchConfig { games: 2, kingdom: kd.clone(), seed: round as u64, max_turns: 200 };
        let stats = run_match(&[&s, &bmu], &cfg);
        assert_eq!(stats[0].games, 2, "{:?}", names(&kd));
    }
}

#[test]
fn every_ally_is_played_with_the_liaisons() {
    let bmu = load("big_money_ultimate.toml");
    for (i, ally) in (cards::FIRST_ALLY..cards::FIRST_ALLY + cards::NUM_ALLIES as u16).enumerate() {
        let kd = vec![a::UNDERLING, a::SYCOPHANT, a::IMPORTER, a::STUDENT, a::TOWN_CRIER, a::HERB_GATHERER, a::TENT, a::OLD_MAP, a::BATTLE_PLAN, id::VILLAGE, ally];
        let s = gainer(&[a::UNDERLING, a::SYCOPHANT, a::TOWN_CRIER]);
        let cfg = MatchConfig { games: 4, kingdom: kd.clone(), seed: 100 + i as u64, max_turns: 200 };
        let stats = run_match(&[&s, &bmu], &cfg);
        assert_eq!(stats[0].games, 4, "{:?}", names(&kd));
    }
}

#[test]
fn the_whole_set_in_one_kingdom() {
    // Every Allies card and every Ally at once (more than ten kingdom cards: the engine does not mind; the Allies are one per game in random kingdoms, not here).
    let mut kd: Vec<u16> = cards::kingdom_cards_in(CardSet::Allies).collect();
    kd.push(a::CAVE_DWELLERS);
    let s = gainer(&kd[..3]);
    let bmu = load("big_money_ultimate.toml");
    let cfg = MatchConfig { games: 4, kingdom: kd, seed: 99, max_turns: 200 };
    let stats = run_match(&[&s, &bmu], &cfg);
    assert_eq!(stats[0].games, 4);
}

fn decide_with(text: &str, strat: &Strategy, play: Option<u16>) -> (Decision, Choice) {
    let mut g = parse_state(text).unwrap();
    let mut d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    if let Some(card) = play {
        g.apply(Choice::Card(card), &mut NoEvents).unwrap();
        d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
    }
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
    (d, c)
}

const KINGDOM: &str = "Village, Smithy, Market, Militia, Moat, Cellar, Chapel, Workshop, Bandit, Gardens";

fn text(ally: &str, extra: &str, hand: &str, favors: u8) -> String {
    format!(
        "players: 2\nkingdom: {KINGDOM}, {extra}\nally: {ally}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: {hand}\ndeck: 5 Copper\nfavors: {favors}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\nfavors: 0\n"
    )
}

#[test]
fn the_bot_spends_five_favors_on_mountain_folk_and_a_stated_rule_overrides_it() {
    let strat = load("big_money_ultimate.toml");
    let (d, c) = decide_with(&text("Mountain Folk", "Underling", "Copper, Copper, Silver, Estate, Estate", 5), &strat, None);
    assert_eq!(d.source, Some(a::MOUNTAIN_FOLK), "{d:?}");
    assert_eq!(c, Choice::Yes);
    let never = Strategy::parse("name = \"N\"\n[[favor]]\nuse = false\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let (_, c) = decide_with(&text("Mountain Folk", "Underling", "Copper, Copper, Silver, Estate, Estate", 5), &never, None);
    assert_eq!(c, Choice::No);
    let rule = Strategy::parse(
        "name = \"R\"\n[[favor]]\nally = \"Mountain Folk\"\nuse = false\nif = \"favors < 7\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n",
    )
    .unwrap();
    let (_, c) = decide_with(&text("Mountain Folk", "Underling", "Copper, Copper, Silver, Estate, Estate", 5), &rule, None);
    assert_eq!(c, Choice::No);
    let (_, c) = decide_with(&text("Mountain Folk", "Underling", "Copper, Copper, Silver, Estate, Estate", 8), &rule, None);
    assert_eq!(c, Choice::Yes);
}

#[test]
fn cave_dwellers_is_used_to_get_rid_of_a_dead_card_and_not_otherwise() {
    let strat = load("big_money_ultimate.toml");
    let (d, c) = decide_with(&text("Cave Dwellers", "Underling", "Gold, Gold, Silver, Estate, Silver", 3), &strat, None);
    assert_eq!(d.source, Some(a::CAVE_DWELLERS), "{d:?}");
    assert_eq!(c, Choice::Yes);
    let (_, c) = decide_with(&text("Cave Dwellers", "Underling", "Gold, Gold, Silver, Silver, Silver", 3), &strat, None);
    assert_eq!(c, Choice::No);
}

#[test]
fn a_rotation_is_asked_for_only_when_it_uncovers_a_card_the_strategy_wants() {
    // Town Crier on top of the Townsfolk, then Blacksmith, Miller, Elder.
    let wants_blacksmith = Strategy::parse(
        "name = \"B\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Blacksmith\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n",
    )
    .unwrap();
    let t = text("Plateau Shepherds", "Town Crier, Underling", "Town Crier, Copper, Copper, Estate, Estate", 1);
    let (d, c) = decide_with(&t, &wants_blacksmith, Some(a::TOWN_CRIER));
    // The mode first (+$2), then the question about the rotation.
    assert!(matches!(d.kind, DecisionKind::Mode { .. }) || matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?} {c:?}");
    let mut g = parse_state(&t).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(a::TOWN_CRIER), &mut NoEvents).unwrap();
    let mut buf = ChoiceBuf::default();
    for _ in 0..4 {
        let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
        g.legal_choices(&mut buf);
        let c = wants_blacksmith.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
        if matches!(d.kind, DecisionKind::YesNo { .. }) && d.source == Some(a::TOWN_CRIER) {
            assert_eq!(c, Choice::Yes, "the Blacksmith is under the Town Crier: rotate");
            break;
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    // A strategy that wants the Town Crier itself does not rotate.
    let wants_crier = Strategy::parse("name = \"C\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Town Crier\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let mut g = parse_state(&t).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(a::TOWN_CRIER), &mut NoEvents).unwrap();
    for _ in 0..4 {
        let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
        g.legal_choices(&mut buf);
        let c = wants_crier.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
        if matches!(d.kind, DecisionKind::YesNo { .. }) && d.source == Some(a::TOWN_CRIER) {
            assert_eq!(c, Choice::No);
            return;
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    panic!("the rotation question never came");
}

#[test]
fn the_expression_variables_for_favors_and_the_ally() {
    let s = Strategy::parse(
        "name = \"V\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\nif = \"favors >= 3 and has_ally(Cave Dwellers) and opp_favors == 0\"\n[[gain]]\ncard = \"Silver\"\n",
    )
    .unwrap();
    let t = format!(
        "players: 2\nkingdom: {KINGDOM}, Underling\nally: Cave Dwellers\nturn: 5  player: 1  phase: buy  actions: 1  buys: 1  coins: 6\n\n[player 1]\nhand: Estate\ndeck: 5 Copper\nfavors: 3\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\nfavors: 0\n"
    );
    let (_, c) = decide_with(&t, &s, None);
    assert_eq!(c, Choice::Card(id::GOLD));
    let t = t.replace("favors: 3", "favors: 2");
    let (_, c) = decide_with(&t, &s, None);
    assert_eq!(c, Choice::Card(id::SILVER));
}

#[test]
fn a_strategy_naming_a_split_pile_card_or_an_ally_parses() {
    Strategy::parse("name = \"P\"\n[[gain]]\ncard = \"Blacksmith\"\n[[gain]]\ncard = \"Gold\"\n[[favor]]\nally = \"Island Folk\"\nuse = true\n").unwrap();
    assert!(Strategy::parse("name = \"P\"\n[[favor]]\nally = \"Smithy\"\nuse = true\n").is_err());
}

// ---- audit: policies ----

fn plain_text(extra: &str, hand1: &str, hand2: &str) -> String {
    format!(
        "players: 2\nkingdom: {KINGDOM}, {extra}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: {hand1}\ndeck: 5 Copper\n\n[player 2]\nhand: {hand2}\ndeck: 5 Estate\n"
    )
}

#[test]
fn the_archers_victim_keeps_the_best_card_and_the_attacker_discards_the_best_of_the_rest() {
    let strat = load("big_money_ultimate.toml");
    let t = plain_text("Battle Plan", "Archer, Copper", "Gold, Silver, Copper, Estate, Estate");
    let (d, c) = decide_with(&t, &strat, Some(a::ARCHER));
    assert_eq!(d.player, 1, "{d:?}");
    assert_eq!(c, Choice::Card(id::GOLD), "the victim hides the Gold");
    // The attacker picks among the revealed cards: the Silver is the best of them.
    let mut g = parse_state(&t).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(a::ARCHER), &mut NoEvents).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!((d.player, d.for_player), (0, 1), "{d:?}");
    assert_eq!(strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice()), Choice::Card(id::SILVER));
}

#[test]
fn capital_city_discards_the_dead_cards() {
    let strat = load("big_money_ultimate.toml");
    let t = plain_text("Capital City", "Capital City, Copper, Copper, Estate, Estate", "5 Copper");
    let (d, c) = decide_with(&t, &strat, Some(a::CAPITAL_CITY));
    assert!(matches!(d.kind, DecisionKind::Select { .. }), "{d:?}");
    assert_eq!(c, Choice::Card(id::ESTATE));
}

#[test]
fn bauble_does_not_put_a_gained_victory_card_onto_the_deck() {
    // (The rules of the fast bots; a bot that searches also searches this question, a Bauble being a Treasure with an effect.)
    let strat = Strategy::parse("name = \"B\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n").unwrap();
    let t = format!(
        "players: 2\nkingdom: {KINGDOM}, Bauble, Plateau Shepherds\nally: Plateau Shepherds\nturn: 5  player: 1  phase: buy  actions: 1  buys: 1  coins: 8\nal turn: bauble\n\n[player 1]\nhand: Estate\ndeck: 5 Copper\nfavors: 1\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\nfavors: 1\n"
    );
    let mut g = parse_state(&t).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
    assert_eq!(c, Choice::Card(id::PROVINCE));
    g.apply(c, &mut NoEvents).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.subject == id::PROVINCE, "{d:?}");
    g.legal_choices(&mut buf);
    assert_eq!(strat.decide_by_rules(&PlayerView::new(&g, d.player), &d, buf.as_slice()), Choice::No, "{d:?}");
}
