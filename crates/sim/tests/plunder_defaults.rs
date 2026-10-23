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

/// Orb's "play an Action or Treasure from your discard pile" can't be declined: a fast bot with no Action worth playing plays a Treasure
/// (it used to answer Pass, an illegal choice, on this kingdom).
#[test]
fn a_fast_bot_plays_a_treasure_from_the_discard_for_orb() {
    let kd: Vec<u16> = ["Jewelled Egg", "Search", "Siren", "Abundance", "Fortune Hunter", "Rope", "Cutthroat", "Figurine", "Silver Mine", "Sack of Loot", "Journey", "Fawning"]
        .iter()
        .map(|n| cards::by_name(n).unwrap())
        .collect();
    let mut a = load("courtyard_bm.toml");
    let mut b = load("governor_bm.toml");
    for s in [&mut a, &mut b] {
        s.search_play = false;
        s.win_this_turn = false;
    }
    let cfg = MatchConfig { games: 2000, kingdom: kd, seed: 549999, max_turns: 200 };
    let stats = run_match(&[&a, &b], &cfg);
    assert_eq!(stats[0].games, 2000);
}

// ---- decisions (bot policy; notes in docs/audit/plunder.md) ----

use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, DecisionKind, GameConfig, GameState, NoEvents, PlayerView, Step};

const KD: &str = "Village, Smithy, Market, Militia, Moat, Cellar, Chapel, Workshop, Bandit, Gardens";

/// The bot's answers to the decisions that follow `first` (a card played, or nothing) in the state `text`, up to the next Action or Buy prompt.
fn answers(text: &str, first: Option<Choice>, strat: &Strategy) -> Vec<(DecisionKind, Choice)> {
    let mut g = parse_state(text).unwrap();
    let Step::Decision(_) = g.advance(&mut NoEvents) else { panic!() };
    if let Some(c) = first {
        g.apply(c, &mut NoEvents).unwrap();
    }
    let mut out = Vec::new();
    for _ in 0..12 {
        let Step::Decision(d) = g.advance(&mut NoEvents) else { break };
        if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) {
            break;
        }
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
        assert!(buf.as_slice().contains(&c), "illegal {c:?} in {:?}", buf.as_slice());
        out.push((d.kind, c));
        g.apply(c, &mut NoEvents).unwrap();
    }
    out
}

fn state(extra: &str, hand: &str, phase: &str) -> String {
    format!("players: 2\nkingdom: {KD}, {extra}\nturn: 5  player: 1  phase: {phase}  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: {hand}\ndeck: 8 Copper, 2 Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n")
}

#[test]
fn inherited_replaces_a_starting_estate() {
    let bmu = load("big_money_ultimate.toml");
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::VILLAGE, id::SMITHY, p::INHERITED], seed: 3, trait_piles: vec![(p::INHERITED, id::SMITHY)], ..Default::default() };
    let mut g = GameState::new(&cfg);
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!(bmu.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice()), Choice::Card(id::ESTATE));
}

#[test]
fn grotto_sets_the_dead_cards_aside_and_keeps_the_good_treasures() {
    let bmu = load("big_money_ultimate.toml");
    let a = answers(&state("Grotto", "Grotto, Copper, Copper, Estate, Silver, Gold", "action"), Some(Choice::Card(p::GROTTO)), &bmu);
    let picked: Vec<Choice> = a.iter().filter(|(k, _)| matches!(k, DecisionKind::Select { .. })).map(|&(_, c)| c).collect();
    assert_eq!(picked.first(), Some(&Choice::Card(id::ESTATE)), "the Estate first");
    assert!(!picked.contains(&Choice::Card(id::GOLD)) && !picked.contains(&Choice::Card(id::SILVER)));
}

#[test]
fn shaman_and_maroon_trash_the_curse() {
    let bmu = load("big_money_ultimate.toml");
    for (card, name) in [(p::SHAMAN, "Shaman"), (p::MAROON, "Maroon")] {
        let a = answers(&state(name, &format!("{name}, Copper, Curse, Estate, Silver, Gold"), "action"), Some(Choice::Card(card)), &bmu);
        assert_eq!(a[0].1, Choice::Card(id::CURSE), "{name}");
    }
}

#[test]
#[ignore = "audit: policy. Cage should be played before the Coppers so that it can set them aside (wiki: 'initially a lot like a Chapel that can only be used once'); the bot plays every Treasure first and Cage last, so only Victory cards and Curses are left to set aside"]
fn cage_is_played_before_the_coppers_so_that_it_can_set_them_aside() {
    let bmu = load("big_money_ultimate.toml");
    let text = state("Cage", "Cage, Copper, Copper, Estate, Silver", "buy");
    let mut g = parse_state(&text).unwrap();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!(bmu.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice()), Choice::Card(p::CAGE));
}
