//! The Lab evolves Renaissance strategies: Projects sit in the gain list, kingdoms hold at most two of them, every generated strategy parses.
use dominion_engine::cards::{self, id};
use dominion_engine::rng::Rng;
use dominion_evolve::{run, Control, EvolveConfig, OpponentSpec, Track};

#[test]
fn random_track_with_renaissance_runs_and_offers_projects() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies");
    let bmu = std::fs::read_to_string(format!("{root}/big_money_ultimate.toml")).unwrap();
    let c = EvolveConfig {
        opponents: vec![OpponentSpec { name: "BMU".into(), toml: bmu.clone(), weight: 1.0 }],
        seeds: vec![bmu],
        sets: vec!["Renaissance".into()],
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
    assert!(setup.space.gainable.contains(&id::PAGEANT) && setup.space.gainable.contains(&id::CANAL));
    assert!(setup.space.gainable.contains(&id::SILK_MERCHANT));
    let mut rng = Rng::new(1);
    for s in setup.scenarios(&mut rng, 6) {
        assert_eq!(s.kingdom.iter().filter(|&&k| cards::is_kingdom(k)).count(), 10);
        assert!(s.kingdom.iter().filter(|&&k| cards::is_project(k)).count() <= 2);
    }
    let p = run(&c, &Control::default(), &mut |_| {}).unwrap();
    let r = p.result.expect("a result");
    assert!(dominion_sim::Strategy::parse(&r.toml).is_ok(), "{}", r.toml);
}
