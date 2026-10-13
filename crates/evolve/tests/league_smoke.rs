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

#[test]
fn stop_during_the_final_ranking_ends_the_run_promptly() {
    // Grand Scheme with full searching bots in the round robin: slow enough that it must be
    // stoppable mid-ranking (it used to ignore Stop and report nothing until done).
    let k = ["Artisan", "Council Room", "Market", "Militia", "Workshop", "Bridge", "Mill", "Mining Village", "Patrol", "Shanty Town"];
    let cfg = EvolveConfig {
        track: Track::Fixed { kingdom: k.iter().map(|s| s.to_string()).collect() },
        league: Some(LeagueConfig { final_games: 2000, final_full: true, ..LeagueConfig::default() }),
        forbidden: vec![],
        islands: 2,
        island_size: 8,
        generations: 2,
        race_games: vec![100],
        validate_games: 200,
        polish_games: 0,
        ..EvolveConfig::default()
    };
    let control = Control::default();
    let mut saw_ranking = false;
    let mut stopped_at = None;
    let p = run(&cfg, &control, &mut |pr| {
        if pr.status.starts_with("ranking: pair") {
            saw_ranking = true;
            if stopped_at.is_none() {
                control.stop.store(true, std::sync::atomic::Ordering::Relaxed);
                stopped_at = Some(std::time::Instant::now());
            }
        }
    })
    .unwrap();
    assert!(saw_ranking, "the ranking reports its progress pair by pair");
    assert_eq!(p.status, "stopped");
    let waited = stopped_at.unwrap().elapsed();
    assert!(waited.as_secs_f64() < 30.0, "stopped {waited:?} after Stop");
}

fn exploit_cfg(rounds: usize, margin: f64) -> EvolveConfig {
    let mut c = bm_cfg();
    c.generations = 2;
    if let Some(l) = c.league.as_mut() {
        l.exploit_rounds = rounds;
        l.exploit_generations = 2;
        l.exploit_margin = margin;
        l.exploit_games = 200;
    }
    c
}

#[test]
fn exploiter_loop_stops_at_the_first_round_that_does_not_beat_the_margin() {
    // A margin no win rate can exceed: one round is played, nothing is added, the loop settles.
    let p = run(&exploit_cfg(3, 0.5), &Control::default(), &mut |_| {}).unwrap();
    assert_eq!(p.status, "done");
    let l = p.league.unwrap();
    assert_eq!(l.exploit_rounds, 3);
    assert_eq!(l.exploiters.len(), 1, "stopped after the first failure");
    let r = &l.exploiters[0];
    assert!(!r.added && r.name.is_none());
    assert!(r.ci_lo <= r.rate_vs_mix && r.rate_vs_mix <= r.ci_hi);
    assert!(!r.mix.is_empty() && (r.mix.iter().map(|m| m.1).sum::<f64>() - 1.0).abs() < 1e-9);
    assert_eq!(r.vs.len(), r.mix.len());
    assert!(l.kinds.iter().all(|k| k != "exploiter"));
    assert_eq!(l.nash_exploitability, Some(r.rate_vs_mix - 0.5));
    let t = l.crosstable.unwrap();
    assert_eq!(l.exploitability.len(), t.names.len());
    assert!(l.exploitability.iter().all(|&e| (0.0..=0.5).contains(&e)));
    assert!(l.top.iter().all(|row| row.exploitability == l.exploitability[row.index]));
}

#[test]
fn exploiters_that_beat_the_margin_join_and_the_nash_mix_is_recomputed() {
    let off = run(&exploit_cfg(0, 0.03), &Control::default(), &mut |_| {}).unwrap().league.unwrap();
    assert!(off.exploiters.is_empty() && off.nash_exploitability.is_none());
    let before = off.crosstable.unwrap().names.len();
    // A margin of -60 points: every exploiter is added.
    let p = run(&exploit_cfg(2, -0.6), &Control::default(), &mut |_| {}).unwrap();
    assert_eq!(p.status, "done");
    let l = p.league.unwrap();
    assert_eq!(l.exploiters.len(), 2);
    assert!(l.exploiters.iter().all(|r| r.added));
    let t = l.crosstable.unwrap();
    assert_eq!(t.names.len(), before + 2);
    assert_eq!(l.strategies.len(), before + 2);
    assert_eq!(l.kinds.iter().filter(|k| *k == "exploiter").count(), 2);
    assert!(t.names.iter().any(|n| n == "Exploiter 2"));
    assert!((t.nash.iter().sum::<f64>() - 1.0).abs() < 1e-6);
    for i in 0..t.names.len() {
        for j in 0..t.names.len() {
            assert!((t.rate[i][j] + t.rate[j][i] - 1.0).abs() < 1e-9);
        }
    }
    // Round 2 faced the mix recomputed after round 1; the report's last Nash support is the final mix.
    let last = l.exploiters.last().unwrap();
    for (name, w) in &last.nash_after {
        let i = t.names.iter().position(|n| n == name).unwrap();
        assert!(t.nash[i] > 0.0 && *w > 0.0);
    }
    // Each ranked strategy's per-opponent split covers every other strategy.
    for e in &l.strategies {
        assert_eq!(e.vs.len(), t.names.len() - 1);
        dominion_sim::Strategy::parse(&e.toml).unwrap();
    }
}

#[test]
fn stop_during_the_exploiter_rounds_ends_the_run_as_stopped() {
    let control = Control::default();
    let mut saw = false;
    let p = run(&exploit_cfg(3, -0.6), &control, &mut |pr| {
        if pr.status.starts_with("exploiter round") {
            saw = true;
            control.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    })
    .unwrap();
    assert!(saw);
    assert_eq!(p.status, "stopped");
    let l = p.league.unwrap();
    assert!(l.exploiters.is_empty(), "the interrupted round is not reported");
    assert!(l.crosstable.is_some(), "the round robin so far stays in the report");
}
