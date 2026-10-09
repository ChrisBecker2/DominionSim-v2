//! The Lab can evolve rules naming the Knights pile as one card ("Knights"): the search space offers it as one gainable, playable
//! entry (never the ten Knights), and a genome with a Knights rule survives the TOML round trip and buys the top Knight.
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, NoEvents, PlayerView, Step};
use dominion_evolve::genome::{Genome, TomlMeta};
use dominion_evolve::ops;
use dominion_evolve::space::SearchSpace;

#[test]
fn the_space_offers_the_knights_pile_as_one_entry() {
    let pool: Vec<_> = cards::kingdom_cards_in(CardSet::DarkAges).collect();
    assert!(pool.contains(&id::KNIGHTS) && !pool.iter().any(|&c| cards::is(c, cards::KNIGHT)));
    let space = SearchSpace::new(&pool, &[]);
    assert!(space.gainable.contains(&id::KNIGHTS));
    assert!(!space.gainable.iter().any(|&c| cards::is(c, cards::KNIGHT) || cards::is(c, cards::RUINS)));
    assert!(space.actions().any(|c| c == id::KNIGHTS), "a play rule may name the Knights");
    // Templates: Big Money + Knights, with the play rule.
    let t = ops::templates(&space);
    assert!(t.iter().any(|g| g.gain.iter().any(|r| r.card == id::KNIGHTS) && g.play.iter().any(|r| r.card == id::KNIGHTS)));
}

#[test]
fn a_knights_rule_round_trips_and_buys_whichever_knight_is_on_top() {
    let mut g = ops::big_money_with(&[(id::KNIGHTS, 3)]);
    let pool = [id::KNIGHTS, id::CULTIST];
    let space = SearchSpace::new(&pool, &[]);
    space.sanitize(&mut g);
    assert!(g.gain.iter().any(|r| r.card == id::KNIGHTS) && g.play.iter().any(|r| r.card == id::KNIGHTS), "sanitize keeps the rules");
    let toml = g.to_toml(&TomlMeta::default());
    assert!(toml.contains("\"Knights\""), "{toml}");
    assert_eq!(Genome::from_toml(&toml).unwrap(), g);
    let strat = dominion_sim::Strategy::parse(&toml).unwrap();
    for top in ["Dame Natalie", "Sir Vander"] {
        let text = format!(
            "players: 2\nkingdom: Knights, Cultist\nsupply: {top}=10\nturn: 5  player: 1  phase: buy  actions: 0  buys: 1  coins: 5\n\n[player 1]\nhand:\ndeck: 5 Copper\n\n[player 2]\nhand:\ndeck: 5 Estate\n"
        );
        let mut s = parse_state(&text).unwrap();
        let d = match s.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            x => panic!("{x:?}"),
        };
        let mut buf = ChoiceBuf::default();
        s.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&s, d.player), &d, buf.as_slice());
        assert_eq!(c, Choice::Card(cards::by_name(top).unwrap()));
    }
}
