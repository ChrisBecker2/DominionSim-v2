//! Bot defaults for Renaissance: the bots play every card, Artifact and Project to the end of a game without an illegal choice, and a few decisions.

use dominion_engine::cards::{self, id};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, NoEvents, PlayerView, Step};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

/// A strategy that gains two of each of the given cards (and the Projects, as soon as it can), then Big Money.
fn gainer(cards_in_kingdom: &[u16]) -> Strategy {
    let mut s = String::from("name = \"Gainer\"\n\n[[gain]]\ncard = \"Province\"\n\n");
    for &c in cards_in_kingdom {
        if cards::is_project(c) {
            s += &format!("[[gain]]\ncard = \"{}\"\n\n", cards::name(c));
        }
    }
    s += "[[gain]]\ncard = \"Duchy\"\nif = \"provinces_left <= 3\"\n\n";
    for &c in cards_in_kingdom {
        if !cards::is_project(c) && c <= cards::LAST_KINGDOM {
            s += &format!("[[gain]]\ncard = \"{}\"\nif = \"count({}) < 2\"\n\n", cards::name(c), cards::name(c));
        }
    }
    s += "[[gain]]\ncard = \"Gold\"\n\n[[gain]]\ncard = \"Silver\"\n";
    Strategy::parse(&s).unwrap_or_else(|e| panic!("{e}\n{s}"))
}

#[test]
fn bots_play_every_card_and_every_project_to_the_end() {
    let bmu = load("big_money_ultimate.toml");
    let ren: Vec<u16> = cards::kingdom_cards_in(cards::CardSet::Renaissance).collect();
    assert_eq!(ren.len(), 25);
    let projects: Vec<u16> = (0..cards::NUM_PROJECTS).map(|i| cards::FIRST_PROJECT + i as u16).collect();
    for round in 0..5 {
        // Ten kingdom cards: five of the set in turn, always with the Curse givers and trashers around them.
        let mut kingdom: Vec<u16> = ren.iter().copied().skip(round * 5).take(5).collect();
        for extra in [id::WITCH, id::CHAPEL, id::MILITIA, id::THRONE_ROOM, id::VILLAGE] {
            kingdom.push(extra);
        }
        for k in 0..2 {
            let mut kd = kingdom.clone();
            kd.push(projects[(round * 4 + k * 3) % projects.len()]);
            kd.push(projects[(round * 4 + k + 7) % projects.len()]);
            let s = gainer(&kd);
            let cfg = MatchConfig { games: 10, kingdom: kd.clone(), seed: (round * 10 + k) as u64, max_turns: 200 };
            let stats = run_match(&[&s, &bmu], &cfg);
            assert_eq!(stats[0].games, 10, "{:?}", kd.iter().map(|&c| cards::name(c)).collect::<Vec<_>>());
        }
    }
}

#[test]
fn the_rest_of_the_set_plays_with_the_whole_set_in_one_kingdom() {
    // Every Renaissance card at once (more than ten: the engine does not mind), the Projects the other kingdoms leave out and the Artifacts that come with them.
    let ren: Vec<u16> = cards::kingdom_cards_in(cards::CardSet::Renaissance).collect();
    let mut kd = ren.clone();
    for p in [id::CITY_GATE, id::EXPLORATION, id::ACADEMY, id::PIAZZA, id::INNOVATION] {
        kd.push(p);
    }
    let s = gainer(&kd);
    let bmu = load("big_money_ultimate.toml");
    let cfg = MatchConfig { games: 20, kingdom: kd, seed: 99, max_turns: 200 };
    let stats = run_match(&[&s, &bmu], &cfg);
    assert_eq!(stats[0].games, 20);
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

#[test]
fn a_villager_is_spent_for_an_action_that_is_worth_playing_and_not_for_nothing() {
    let strat = load("big_money_ultimate.toml");
    let head = "players: 2\nkingdom: Village, Smithy, Market, Militia, Moat, Cellar, Chapel, Workshop, Bandit, Silk Merchant\nturn: 3  player: 1  phase: action  actions: 0  buys: 1  coins: 0\n\n[player 1]\nvillagers: 2\n";
    // A Smithy in hand and no Action left: spend a Villager.
    let with_smithy = format!("{head}hand: Smithy, Copper, Copper, Estate, Estate\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n");
    assert_eq!(decide(&with_smithy, &strat), Choice::Villager);
}

#[test]
fn the_bot_buys_a_project_when_its_gain_list_says_so() {
    let strat = gainer(&[id::PAGEANT]);
    let text = "players: 2\nkingdom: Village, Smithy, Market, Militia, Moat, Cellar, Chapel, Workshop, Bandit, Gardens\nprojects: Pageant\n\
                turn: 5  player: 1  phase: buy  actions: 1  buys: 1  coins: 3\n\n[player 1]\nhand: Estate\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    assert_eq!(decide(text, &strat), Choice::Card(id::PAGEANT));
}

#[test]
fn a_lone_seer_is_played_without_a_search() {
    // Seer is order-sensitive (it looks at the deck) but never worse to play alone: the bot plays it straight away, and searching it was
    // thousands of times slower for the same choices.
    use dominion_engine::{Choice, ChoiceBuf, NoEvents, PlayerView, Step};
    assert!(dominion_engine::cards::safe_alone(dominion_engine::cards::id::SEER));
    let strat = dominion_sim::Strategy::parse("name = \"S\"
[[gain]]
card = \"Province\"
[[gain]]
card = \"Seer\"
[[gain]]
card = \"Silver\"
").unwrap();
    assert!(strat.search_play);
    let text = "players: 2
kingdom: Seer, Village, Smithy, Market, Militia, Moat, Cellar, Workshop, Mine, Laboratory
                turn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: Seer, 3 Copper, Estate
deck: 5 Copper, Silver, Estate

                [player 2]
hand: 5 Copper
deck: 5 Copper
";
    let mut g = dominion_engine::text::parse_state(text).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!(strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice()), Choice::Card(dominion_engine::cards::id::SEER));
}
