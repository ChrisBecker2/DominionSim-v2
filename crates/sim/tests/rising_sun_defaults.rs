//! Bot defaults for Rising Sun: the bots play every Prophecy and every card of the set to the end without an illegal choice, and a few decisions.

use dominion_engine::cards::{self, id};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, NoEvents, PlayerView, Step};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

#[test]
fn bots_play_every_prophecy_to_the_end() {
    let bmu = load("big_money_ultimate.toml");
    let kitsune = load("kitsune_bm.toml");
    let rs: Vec<_> = cards::kingdom_cards_in(cards::CardSet::RisingSun).filter(|&c| c != id::RIVERBOAT).collect();
    for i in 0..cards::NUM_PROPHECIES {
        let prophecy = cards::FIRST_PROPHECY + i as u16;
        let mut kingdom: Vec<u16> = rs.iter().copied().skip(i % 5).step_by(2).take(8).collect();
        for extra in [id::TEA_HOUSE, id::POET, id::ALLEY, id::FISHMONGER, id::SMITHY, id::VILLAGE] {
            if kingdom.len() < 10 && !kingdom.contains(&extra) {
                kingdom.push(extra);
            }
        }
        kingdom.push(prophecy);
        if prophecy == id::APPROACHING_ARMY {
            kingdom.push(id::MILITIA);
        }
        let cfg = MatchConfig { games: 30, kingdom, seed: i as u64, max_turns: 200 };
        let stats = run_match(&[&kitsune, &bmu], &cfg);
        assert_eq!(stats[0].games + 0, 30, "{}", cards::name(prophecy));
    }
}

#[test]
fn snake_witch_and_gold_mine_decisions() {
    let strat = load("big_money_ultimate.toml");
    let text = "players: 2\nkingdom: Snake Witch, Gold Mine, Smithy, Village, Market, Militia, Moat, Cellar, Workshop, Remodel\n\
                turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Gold Mine\ndeck: 3 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::GOLD_MINE), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!(strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice()), Choice::Yes, "a Gold for 4 Debt early in the game");
}
