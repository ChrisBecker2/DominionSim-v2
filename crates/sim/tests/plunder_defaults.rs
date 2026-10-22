//! Bot defaults for Plunder: the bots play every card, Loot, Event and Trait to the end of a game without an illegal choice, and a few decisions
//! (rulings and bot policy in `docs/plunder-plan.md`).

use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::sets::plunder::ids as p;
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
        } else if cards::is_event(c) {
            s += &format!("[[gain]]\ncard = \"{}\"\nif = \"bought({}) < 2\"\n\n", cards::name(c), cards::name(c));
        }
    }
    s += "[[gain]]\ncard = \"Gold\"\n\n[[gain]]\ncard = \"Silver\"\n";
    Strategy::parse(&s).unwrap_or_else(|e| panic!("{e}\n{s}"))
}

fn names(cs: &[u16]) -> Vec<&'static str> {
    cs.iter().map(|&c| cards::name(c)).collect()
}

#[test]
fn bots_play_every_card_event_and_trait_to_the_end() {
    let bmu = load("big_money_ultimate.toml");
    let kingdom: Vec<u16> = cards::kingdom_cards_in(CardSet::Plunder).collect();
    assert_eq!(kingdom.len(), 40);
    let events: Vec<u16> = (cards::FIRST_EVENT..cards::FIRST_EVENT + cards::NUM_EVENTS as u16).filter(|&e| cards::set_of(e) == CardSet::Plunder).collect();
    assert_eq!(events.len(), 15);
    let traits: Vec<u16> = (cards::FIRST_TRAIT..cards::FIRST_TRAIT + cards::NUM_TRAITS as u16).collect();
    assert_eq!(traits.len(), 15);
    for round in 0..10 {
        let mut kd: Vec<u16> = kingdom.iter().copied().cycle().skip(round * 4).take(7).collect();
        for extra in [id::WITCH, id::CHAPEL, id::MILITIA] {
            if !kd.contains(&extra) {
                kd.push(extra);
            }
        }
        kd.push(events[round % events.len()]);
        kd.push(events[(round * 2 + 5) % events.len()]);
        kd.push(traits[round % traits.len()]);
        kd.push(traits[(round * 3 + 7) % traits.len()]);
        let s = gainer(&kd[..6]);
        for seed in 0..2 {
            let cfg = MatchConfig { games: 2, kingdom: kd.clone(), seed: round as u64 * 10 + seed, max_turns: 200 };
            let stats = run_match(&[&s, &bmu], &cfg);
            assert_eq!(stats[0].games, 2, "{:?}", names(&kd));
        }
    }
}

#[test]
fn every_trait_is_played_on_a_busy_pile() {
    let bmu = load("big_money_ultimate.toml");
    for (i, t) in (cards::FIRST_TRAIT..cards::FIRST_TRAIT + cards::NUM_TRAITS as u16).enumerate() {
        let kd = vec![p::WEALTHY_VILLAGE, p::LONGSHIP, p::CREW, p::SACK_OF_LOOT, id::VILLAGE, id::SMITHY, id::MARKET, id::MILITIA, id::CHAPEL, id::WITCH, t];
        let s = gainer(&kd[..6]);
        let cfg = MatchConfig { games: 4, kingdom: kd.clone(), seed: i as u64 + 1, max_turns: 200 };
        let stats = run_match(&[&s, &bmu], &cfg);
        assert_eq!(stats[0].games, 4, "{}", cards::name(t));
    }
}

#[test]
fn every_event_is_bought_by_a_bot() {
    let bmu = load("big_money_ultimate.toml");
    let events: Vec<u16> = (cards::FIRST_EVENT..cards::FIRST_EVENT + cards::NUM_EVENTS as u16).filter(|&e| cards::set_of(e) == CardSet::Plunder).collect();
    for (i, &e) in events.iter().enumerate() {
        let kd = vec![p::WEALTHY_VILLAGE, p::LONGSHIP, p::CREW, p::SACK_OF_LOOT, p::PILGRIM, id::VILLAGE, id::SMITHY, id::MILITIA, id::CHAPEL, id::WITCH, e];
        let s = gainer(&[e, p::SACK_OF_LOOT, p::WEALTHY_VILLAGE, p::PILGRIM]);
        let cfg = MatchConfig { games: 4, kingdom: kd.clone(), seed: i as u64 + 100, max_turns: 200 };
        let stats = run_match(&[&s, &bmu], &cfg);
        assert_eq!(stats[0].games, 4, "{}", cards::name(e));
    }
}

#[test]
fn loot_comes_to_the_bots_from_every_source() {
    let bmu = load("big_money_ultimate.toml");
    let kd = vec![p::SACK_OF_LOOT, p::PILGRIM, p::PICKAXE, p::FRIGATE, p::CUTTHROAT, id::VILLAGE, id::SMITHY, id::MILITIA, id::CHAPEL, id::WITCH];
    let s = gainer(&kd[..5]);
    let cfg = MatchConfig { games: 20, kingdom: kd, seed: 9, max_turns: 200 };
    let stats = run_match(&[&s, &bmu], &cfg);
    assert_eq!(stats[0].games, 20);
}
