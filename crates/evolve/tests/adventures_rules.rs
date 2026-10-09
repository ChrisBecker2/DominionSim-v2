//! The Lab evolves Adventures strategies: Events sit in the gain list (bought a bounded number of times), `[[token]]` and `[[call]]` rules are
//! offered only where the pool has a card that places that token or a Reserve card, and every generated strategy parses and round-trips.
use dominion_engine::cards::{self, id, CardId};
use dominion_engine::rng::Rng;
use dominion_evolve::genome::{Genome, List, TomlMeta};
use dominion_evolve::ops;
use dominion_evolve::space::SearchSpace;
use dominion_evolve::{run, Control, EvolveConfig, OpponentSpec, Track};

#[test]
fn token_and_call_rules_are_offered_only_for_what_the_pool_has() {
    let plain = SearchSpace::new(&[id::SMITHY, id::VILLAGE], &[]);
    assert!(plain.token_kinds.is_empty() && plain.token_piles.is_empty() && plain.reserves.is_empty());
    let pool: Vec<CardId> = vec![id::SMITHY, id::VILLAGE, id::TRAINING, id::LOST_ARTS, id::WINE_MERCHANT, id::DUPLICATE, id::BALL];
    let space = SearchSpace::new(&pool, &[]);
    assert_eq!(space.token_kinds, vec![1, 3], "Lost Arts: +1 Action, Training: +$1");
    assert!(space.token_piles.contains(&id::SMITHY) && !space.token_piles.contains(&id::TRAINING));
    assert_eq!(space.reserves, vec![id::DUPLICATE, id::WINE_MERCHANT]);
    assert!(space.gainable.contains(&id::BALL), "Events are gainable by the evolving side");
}

#[test]
fn mutation_produces_valid_event_token_and_call_rules() {
    let pool: Vec<CardId> = vec![id::SMITHY, id::VILLAGE, id::TRAINING, id::LOST_ARTS, id::WINE_MERCHANT, id::DUPLICATE, id::BALL];
    let space = SearchSpace::new(&pool, &[]);
    let mut rng = Rng::new(7);
    let mut pop = ops::templates(&space);
    for _ in 0..4000 {
        let mut c = pop[rng.below(pop.len() as u32) as usize].clone();
        ops::mutate(&mut c, &space, &mut rng);
        for r in &c.token {
            assert!(space.token_piles.contains(&r.card) && space.token_kinds.contains(&r.opt));
        }
        for r in &c.call {
            assert!(space.reserves.contains(&r.card));
        }
        for l in [List::Token, List::Call] {
            assert!(c.clone().list_mut(l).len() <= space.max_rules(l));
        }
        let toml = c.to_toml(&TomlMeta::default());
        dominion_sim::Strategy::parse(&toml).unwrap_or_else(|e| panic!("{e}\n{toml}"));
        assert_eq!(Genome::from_toml(&toml).unwrap(), c, "{toml}");
        pop.push(c);
    }
    assert!(pop.iter().any(|g| !g.token.is_empty()), "mutation never produced a token rule");
    assert!(pop.iter().any(|g| !g.call.is_empty()), "mutation never produced a call rule");
    assert!(pop.iter().any(|g| g.gain.iter().any(|r| cards::is_event(r.card))), "no Event in a gain list");
    // Event rules are bounded by purchases, not by owning the Event.
    let g = ops::big_money_with(&[(id::BALL, 1)]);
    assert!(g.to_toml(&TomlMeta::default()).contains("bought(Ball) < 1"));
}

#[test]
fn random_track_with_adventures_runs_and_offers_events() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies");
    let bmu = std::fs::read_to_string(format!("{root}/big_money_ultimate.toml")).unwrap();
    let c = EvolveConfig {
        opponents: vec![OpponentSpec { name: "BMU".into(), toml: bmu.clone(), weight: 1.0 }],
        seeds: vec![bmu],
        sets: vec!["Adventures".into()],
        forbidden: vec![],
        track: Track::Random { required: vec![], kingdoms_per_generation: 2 },
        islands: 2,
        island_size: 8,
        generations: 2,
        race_games: vec![60, 100],
        validate_games: 200,
        polish_games: 100,
        ..EvolveConfig::default()
    };
    let setup = dominion_evolve::ga::Setup::new(&c).unwrap();
    assert!(setup.space.gainable.contains(&id::ALMS) && setup.space.gainable.contains(&id::TRAVELLING_FAIR));
    assert!(!setup.space.gainable.contains(&id::AVANTO), "only the top half of the split pile is named");
    let mut rng = Rng::new(1);
    for s in setup.scenarios(&mut rng, 4) {
        assert_eq!(s.kingdom.iter().filter(|&&k| cards::is_kingdom(k)).count(), 10);
        assert!(s.kingdom.iter().filter(|&&k| cards::is_event(k)).count() <= 2);
    }
    let p = run(&c, &Control::default(), &mut |_| {}).unwrap();
    let r = p.result.expect("a result");
    assert!(dominion_sim::Strategy::parse(&r.toml).is_ok(), "{}", r.toml);
}
