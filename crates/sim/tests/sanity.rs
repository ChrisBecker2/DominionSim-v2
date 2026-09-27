//! Sanity checks against known community results (dominionstrategy.com): Smithy-BM should beat
//! plain Big Money by a clear but not overwhelming margin in 2p, and Double Witch should beat
//! Big Money heavily. Run with `cargo test --release -p dominion-sim -- --ignored` for the full
//! (slower, tighter-CI) versions; the non-ignored versions below run a smaller game count on
//! every `cargo test` so a regression is caught quickly.

use std::path::{Path, PathBuf};

use dominion_engine::cards;
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn strategies_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("strategies")
}

fn load(name: &str) -> Strategy {
    Strategy::load(&strategies_dir().join(name)).unwrap_or_else(|e| panic!("loading {name}: {e}"))
}

#[test]
fn smithy_bm_beats_big_money() {
    let bm = load("big_money.toml");
    let smithy = load("smithy_bm.toml");
    let cfg = MatchConfig { games: 4_000, kingdom: cards::FIRST_GAME.to_vec(), seed: 42, max_turns: 200 };
    let stats = run_match(&[&bm, &smithy], &cfg);
    let win_rate = stats[1].win_rate();
    assert!((0.60..0.80).contains(&win_rate), "Smithy-BM win rate {:.1}% out of the expected ~65-75% band", win_rate * 100.0);
}

#[test]
fn double_witch_beats_big_money_heavily() {
    let bm = load("big_money.toml");
    let dw = load("double_witch.toml");
    let refs = [&bm, &dw];
    let kingdom = dominion_sim::kingdom::resolve("auto", &refs).unwrap();
    let cfg = MatchConfig { games: 4_000, kingdom, seed: 42, max_turns: 200 };
    let stats = run_match(&refs, &cfg);
    let win_rate = stats[1].win_rate();
    assert!(win_rate > 0.85, "Double Witch win rate {:.1}% should heavily beat Big Money (>85%)", win_rate * 100.0);
}

#[test]
#[ignore = "slow: larger sample for a tighter confidence interval"]
fn smithy_bm_beats_big_money_large_sample() {
    let bm = load("big_money.toml");
    let smithy = load("smithy_bm.toml");
    let cfg = MatchConfig { games: 200_000, kingdom: cards::FIRST_GAME.to_vec(), seed: 42, max_turns: 200 };
    let stats = run_match(&[&bm, &smithy], &cfg);
    let win_rate = stats[1].win_rate();
    let (lo, hi) = stats[1].wilson_ci_95();
    println!("Smithy-BM vs Big Money: {:.2}% [{:.2}%, {:.2}%]", win_rate * 100.0, lo * 100.0, hi * 100.0);
    assert!((0.65..0.75).contains(&win_rate));
}
