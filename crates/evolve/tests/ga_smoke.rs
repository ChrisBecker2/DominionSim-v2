//! End-to-end: a tiny GA run on both tracks produces a valid, validated strategy that respects
//! the forbidden list; `assess` measures files; stopping works.
use dominion_engine::cards::{self, CardSet};
use dominion_evolve::ga::Setup;
use dominion_evolve::{assess, run, Control, EvolveConfig, OpponentSpec, Track};
use std::sync::atomic::Ordering;

fn cfg() -> EvolveConfig {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies");
    let dw = std::fs::read_to_string(format!("{root}/double_witch.toml")).unwrap();
    EvolveConfig {
        opponents: vec![OpponentSpec { name: "Double Witch".into(), toml: dw.clone(), weight: 1.0 }],
        seeds: vec![dw],
        islands: 2,
        island_size: 8,
        generations: 2,
        race_games: vec![100, 200],
        validate_games: 400,
        polish_games: 200,
        ..EvolveConfig::default()
    }
}

#[test]
fn fixed_track_runs_and_respects_forbidden_cards() {
    let mut seen = 0;
    let p = run(&cfg(), &Control::default(), &mut |_| seen += 1).unwrap();
    assert_eq!(p.status, "done");
    assert_eq!(p.history.len(), 2);
    assert!(seen >= 3);
    let r = p.result.expect("a result");
    let s = dominion_sim::Strategy::parse(&r.toml).unwrap();
    assert!(!s.lists(dominion_engine::cards::id::WITCH), "{}", r.toml);
    assert!(r.toml.contains("never_gain = [\"Witch\"]"));
    assert!(!p.hall_of_fame.is_empty());
}

#[test]
fn random_track_runs() {
    let mut c = cfg();
    c.track = Track::Random { required: vec!["Witch".into()], kingdoms_per_generation: 2 };
    let p = run(&c, &Control::default(), &mut |_| {}).unwrap();
    assert!(p.result.is_some());
}

#[test]
fn stop_before_start_still_reports() {
    let control = Control::default();
    control.stop.store(true, Ordering::Relaxed);
    let p = run(&cfg(), &control, &mut |_| {}).unwrap();
    assert_eq!(p.status, "stopped");
    assert!(p.history.is_empty());
}

#[test]
fn fixed_track_with_intrigue_only_offers_intrigue_kingdom_cards() {
    let mut c = cfg();
    c.sets = vec!["Intrigue".into()];
    // Empty kingdom: "every card of the selected sets".
    let setup = Setup::new(&c).unwrap();
    let basics = 5; // Province, Duchy, Estate, Gold, Silver
    assert_eq!(setup.space.gainable.len(), basics + cards::kingdom_cards_in(CardSet::Intrigue).count());
    for &c in &setup.space.gainable {
        assert!(c < cards::FIRST_KINGDOM || cards::set_of(c) == CardSet::Intrigue, "{} leaked into the Intrigue-only search space", cards::name(c));
    }
}

#[test]
fn random_track_draws_only_from_the_selected_sets() {
    let mut c = cfg();
    c.sets = vec!["Intrigue".into()];
    c.track = Track::Random { required: vec!["Witch".into()], kingdoms_per_generation: 4 };
    let setup = Setup::new(&c).unwrap();
    let mut rng = dominion_engine::rng::Rng::new(9);
    let scenarios = setup.scenarios(&mut rng, 3);
    assert!(!scenarios.is_empty());
    for s in &scenarios {
        assert!(s.kingdom.contains(&cards::id::WITCH), "the required card must always be present: {:?}", s.kingdom);
        for &c in &s.kingdom {
            assert!(c == cards::id::WITCH || cards::set_of(c) == CardSet::Intrigue, "{} outside the selected sets: {:?}", cards::name(c), s.kingdom);
        }
    }
}

#[test]
fn unknown_set_name_is_a_config_error() {
    let mut c = cfg();
    c.sets = vec!["Alchemy".into()];
    assert!(Setup::new(&c).is_err());
}

#[test]
fn empty_sets_is_a_config_error() {
    let mut c = cfg();
    c.sets = Vec::new();
    assert!(Setup::new(&c).is_err());
}

#[test]
fn assess_measures_any_file_and_reports_errors() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies");
    let bm = std::fs::read_to_string(format!("{root}/big_money_ultimate.toml")).unwrap();
    let e = assess(&cfg(), &bm, 2000, false).unwrap();
    assert!(e.win_rate > 0.05 && e.win_rate < 0.35, "{}", e.win_rate);
    assert!(assess(&cfg(), "name = 1", 10, false).is_err());
    let mut bad = cfg();
    bad.opponents.clear();
    assert!(run(&bad, &Control::default(), &mut |_| {}).is_err());
}
