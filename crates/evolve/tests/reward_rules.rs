//! The Lab can evolve rules naming the Rewards Joust gains (they are not Supply cards): the search
//! space offers them only when Joust is in the pool, and a genome with a Reward rule survives the TOML
//! round trip and decides as written.
use dominion_engine::cards::{self, id};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, DecisionKind, NoEvents, PlayerView, Step};
use dominion_evolve::genome::{Genome, Rule, TomlMeta};
use dominion_evolve::ops;
use dominion_evolve::space::SearchSpace;

#[test]
fn the_space_offers_rewards_only_with_joust() {
    let with = SearchSpace::new(&[id::JOUST, id::SMITHY], &[]);
    for r in cards::FIRST_REWARD..cards::FIRST_REWARD + cards::NUM_REWARDS as u8 {
        assert!(with.gainable.contains(&r), "{}", cards::name(r));
    }
    let without = SearchSpace::new(&[id::SMITHY, id::VILLAGE], &[]);
    assert!(!without.gainable.iter().any(|&c| cards::is_reward(c)));
    // Forbidden cards stay out.
    let forbid = SearchSpace::new(&[id::JOUST], &[id::COURSER]);
    assert!(!forbid.gainable.contains(&id::COURSER) && forbid.gainable.contains(&id::RENOWN));
    // The seed templates never try to buy a Reward as a kingdom card.
    for g in ops::templates(&with) {
        assert!(!g.gain.iter().any(|r| cards::is_reward(r.card) && r.conds.is_empty() && g.gain.len() <= 3));
    }
}

#[test]
fn a_reward_rule_round_trips_and_joust_follows_it() {
    let mut g = ops::big_money_with(&[(id::JOUST, 1)]);
    g.gain.insert(0, Rule::new(id::COURSER));
    let space = SearchSpace::new(&[id::JOUST], &[]);
    space.sanitize(&mut g);
    assert!(g.gain.iter().any(|r| r.card == id::COURSER), "sanitize keeps a Reward rule when Joust is in the pool");
    let toml = g.to_toml(&TomlMeta::default());
    assert!(toml.contains("Courser"), "{toml}");
    assert_eq!(Genome::from_toml(&toml).unwrap(), g);
    let strat = dominion_sim::Strategy::parse(&toml).unwrap();
    let text = "players: 2\nkingdom: Joust\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
                [player 1]\nhand: Joust, Province\ndeck: 5 Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut s = parse_state(text).unwrap();
    loop {
        let d = match s.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            x => panic!("{x:?}"),
        };
        let mut buf = ChoiceBuf::default();
        s.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&s, d.player), &d, buf.as_slice());
        if matches!(d.kind, DecisionKind::Gain { .. }) {
            assert_eq!(c, Choice::Card(id::COURSER));
            return;
        }
        s.apply(c, &mut NoEvents).unwrap();
    }
}
