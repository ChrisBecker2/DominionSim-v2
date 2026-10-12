//! Kingdom league end to end on Biggest Money: a short run yields several niche champions and a
//! valid crosstable, and every strategy loads.
use dominion_evolve::{run, Control, EvolveConfig, LeagueConfig, Track};

fn bm_cfg() -> EvolveConfig {
    let k = ["Artisan", "Harbinger", "Laboratory", "Mine", "Moneylender", "Bank", "Crystal Ball", "Grand Market", "Mint", "Tiara"];
    EvolveConfig {
        track: Track::Fixed { kingdom: k.iter().map(|s| s.to_string()).collect() },
        league: Some(LeagueConfig { final_games: 200, final_full: false, ..LeagueConfig::default() }),
        forbidden: vec![],
        islands: 3,
        island_size: 10,
        generations: 3,
        race_games: vec![100, 300],
        validate_games: 400,
        polish_games: 0,
        ..EvolveConfig::default()
    }
}

#[test]
fn short_league_run_on_biggest_money() {
    let p = run(&bm_cfg(), &Control::default(), &mut |_| {}).unwrap();
    assert_eq!(p.status, "done");
    let l = p.league.expect("league report");
    assert!(l.niches.len() >= 2, "niches: {}", l.niches.len());
    let t = l.crosstable.expect("crosstable");
    let n = t.names.len();
    assert!(n >= l.anchors.len());
    assert_eq!(l.strategies.len(), n);
    for i in 0..n {
        assert!((t.rate[i][i] - 0.5).abs() < 1e-9);
        for j in 0..n {
            assert!((t.rate[i][j] + t.rate[j][i] - 1.0).abs() < 1e-9);
        }
    }
    assert!((t.nash.iter().sum::<f64>() - 1.0).abs() < 1e-6);
    for e in &l.strategies {
        dominion_sim::Strategy::parse(&e.toml).unwrap();
    }
    for r in &l.niches {
        dominion_sim::Strategy::parse(&r.entry.toml).unwrap();
    }
    assert!(!l.top.is_empty());
    assert!(p.result.is_some());
}

#[test]
fn league_needs_a_fixed_kingdom() {
    let mut c = bm_cfg();
    c.track = Track::Random { required: vec![], kingdoms_per_generation: 2 };
    assert!(run(&c, &Control::default(), &mut |_| {}).is_err());
}
