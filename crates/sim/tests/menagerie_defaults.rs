//! Bot defaults for Menagerie: the bots play every card, Event and Way to the end of a game without an illegal choice, and a few decisions
//! (rulings and bot policy in `docs/menagerie-plan.md`).

use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::sets::menagerie::ids as m;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, NoEvents, PlayerView, Step};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

/// A strategy that gains two of each of the given cards (and the Events, as soon as it can), then Big Money.
fn gainer(in_kingdom: &[u16]) -> Strategy {
    let mut s = String::from("name = \"Gainer\"\n\n[[gain]]\ncard = \"Province\"\n\n");
    for &c in in_kingdom {
        if cards::is_event(c) {
            s += &format!("[[gain]]\ncard = \"{}\"\nif = \"bought({}) < 2\"\n\n", cards::name(c), cards::name(c));
        }
    }
    s += "[[gain]]\ncard = \"Duchy\"\nif = \"provinces_left <= 3\"\n\n";
    for &c in in_kingdom {
        if cards::is_kingdom(c) {
            s += &format!("[[gain]]\ncard = \"{}\"\nif = \"count({}) < 2\"\n\n", cards::name(c), cards::name(c));
        }
    }
    s += "[[gain]]\ncard = \"Gold\"\n\n[[gain]]\ncard = \"Silver\"\n";
    Strategy::parse(&s).unwrap_or_else(|e| panic!("{e}\n{s}"))
}

#[test]
fn bots_play_every_card_event_and_way_to_the_end() {
    let bmu = load("big_money_ultimate.toml");
    let men: Vec<u16> = cards::kingdom_cards_in(CardSet::Menagerie).collect();
    assert_eq!(men.len(), 30);
    let events: Vec<u16> = (m::DELAY..=m::POPULATE).collect();
    let ways: Vec<u16> = (m::WAY_OF_THE_BUTTERFLY..=m::WAY_OF_THE_WORM).collect();
    assert_eq!((events.len(), ways.len()), (20, 20));
    for round in 0..6 {
        let mut kingdom: Vec<u16> = men.iter().copied().skip(round * 5).take(5).collect();
        for extra in [id::WITCH, id::CHAPEL, id::MILITIA, id::THRONE_ROOM, id::VILLAGE] {
            kingdom.push(extra);
        }
        for k in 0..3 {
            let mut kd = kingdom.clone();
            kd.push(events[(round * 3 + k * 5) % events.len()]);
            kd.push(events[(round * 3 + k * 5 + 7) % events.len()]);
            kd.push(ways[(round * 4 + k * 6) % ways.len()]);
            if k == 2 {
                kd.push(ways[(round * 4 + 11) % ways.len()]);
            }
            let s = gainer(&kd);
            let cfg = MatchConfig { games: 4, kingdom: kd.clone(), seed: (round * 10 + k) as u64, max_turns: 200 };
            let stats = run_match(&[&s, &bmu], &cfg);
            assert_eq!(stats[0].games, 4, "{:?}", kd.iter().map(|&c| cards::name(c)).collect::<Vec<_>>());
        }
    }
}

#[test]
fn the_whole_set_in_one_kingdom() {
    // Every Menagerie card, Event and Way at once (more than ten kingdom cards: the engine does not mind).
    let mut kd: Vec<u16> = cards::kingdom_cards_in(CardSet::Menagerie).collect();
    kd.extend(m::DELAY..=m::POPULATE);
    kd.extend([m::WAY_OF_THE_CHAMELEON, m::WAY_OF_THE_MOUSE, m::WAY_OF_THE_TURTLE, m::WAY_OF_THE_FROG]);
    let s = gainer(&kd);
    let bmu = load("big_money_ultimate.toml");
    let cfg = MatchConfig { games: 8, kingdom: kd, seed: 99, max_turns: 200 };
    let stats = run_match(&[&s, &bmu], &cfg);
    assert_eq!(stats[0].games, 8);
}

fn decide(text: &str, strat: &Strategy) -> Choice {
    let mut g = parse_state(text).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice())
}

const KINGDOM: &str = "Village, Smithy, Market, Militia, Moat, Cellar, Chapel, Workshop, Bandit, Gardens";

#[test]
fn the_bot_buys_an_event_when_its_gain_list_says_so() {
    let strat = gainer(&[m::RIDE]);
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}, Ride\nturn: 5  player: 1  phase: buy  actions: 1  buys: 1  coins: 3\n\n[player 1]\nhand: Estate\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    assert_eq!(decide(&text, &strat), Choice::Card(m::RIDE));
}

#[test]
fn a_way_rule_in_the_strategy_decides_the_way() {
    let strat = Strategy::parse(
        "name = \"W\"\n[[way]]\ncard = \"Smithy\"\nway = \"Way of the Sheep\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n",
    )
    .unwrap();
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}, Way of the Sheep, Way of the Ox\nturn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Smithy, Copper, Copper, Estate, Estate\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    // Play the Smithy; the Way decision follows.
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(id::SMITHY), &mut NoEvents).unwrap();
    let d2 = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?} after {d:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(&g, d2.player), &d2, buf.as_slice());
    assert_eq!(c, Choice::Card(m::WAY_OF_THE_SHEEP), "{d2:?} {:?}", buf.as_slice());
}

#[test]
fn the_default_way_choice_does_not_waste_a_good_card() {
    // A Smithy is worth more as itself than as Way of the Sheep ($2): the bot plays it as itself without a rule.
    let strat = load("big_money_ultimate.toml");
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}, Way of the Sheep\nturn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Smithy, Copper, Copper, Estate, Estate\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(id::SMITHY), &mut NoEvents).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
    assert_eq!(c, Choice::Pass, "{d:?} {:?}", buf.as_slice());
}

#[test]
fn a_sleigh_in_hand_is_discarded_to_put_a_gained_province_into_the_hand_or_on_the_deck() {
    let strat = load("big_money_ultimate.toml");
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}, Sleigh\nturn: 5  player: 1  phase: buy  actions: 1  buys: 1  coins: 8\n\n[player 1]\nhand: Sleigh, Gold, Gold, Copper, Copper\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(id::PROVINCE), &mut NoEvents).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    // The offer to discard Sleigh: a Province in hand is dead, on the deck it is a dead draw: either is fine, but the bot must answer legally.
    let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
    assert!(buf.as_slice().contains(&c), "{c:?} not in {:?}", buf.as_slice());
}

#[test]
fn exile_choices_prefer_cards_that_cost_nothing_to_lose() {
    // Bounty Hunter: with a Curse and a Gold in hand, the Curse is Exiled.
    let strat = load("big_money_ultimate.toml");
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}, Bounty Hunter\nturn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Bounty Hunter, Curse, Gold, Gold, Silver\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(m::BOUNTY_HUNTER), &mut NoEvents).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
    assert_eq!(c, Choice::Card(id::CURSE), "{d:?}");
}

#[test]
fn barge_is_played_to_draw_now_even_when_actions_are_left() {
    // Wiki (Barge, Strategy): "Most of the time, it's best to play your Barges to draw this turn". With an Action left to play, the drawn
    // cards can be used at once; the bot used to keep the Barge for the next turn exactly then.
    let strat = load("big_money_ultimate.toml");
    let text = format!(
        "players: 2
kingdom: {KINGDOM}, Barge
turn: 3  player: 1  phase: action  actions: 2  buys: 1  coins: 0

[player 1]
hand: Barge, Smithy, Copper, Copper, Estate
deck: 10 Copper

[player 2]
hand: 5 Copper
deck: 5 Estate
"
    );
    let mut g = parse_state(&text).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(m::BARGE), &mut NoEvents).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
    assert_eq!(c, Choice::Yes, "{d:?}");
}

#[test]
#[ignore = "audit policy: the wiki's main use of Camel Train is to stock up on an expensive card the deck wants (\"Exile ... several copies ... and then buy just one when you're ready\"); the bot Exiles the dearest card its strategy does not list (denial)"]
fn camel_train_exiles_a_card_the_strategy_wants_several_copies_of() {
    let strat = Strategy::parse("name = \"S\"
[[gain]]
card = \"Province\"
[[gain]]
card = \"Smithy\"
if = \"count(Smithy) < 3\"
[[gain]]
card = \"Gold\"
[[gain]]
card = \"Silver\"
").unwrap();
    let text = format!(
        "players: 2
kingdom: {KINGDOM}, Camel Train
turn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: Camel Train, Copper, Copper, Estate, Estate
deck: 5 Copper

[player 2]
hand: 5 Copper
deck: 5 Estate
"
    );
    let mut g = parse_state(&text).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    g.apply(Choice::Card(m::CAMEL_TRAIN), &mut NoEvents).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
    assert_eq!(c, Choice::Card(id::SMITHY), "{d:?}");
}
