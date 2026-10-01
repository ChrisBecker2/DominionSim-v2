//! Shipped strategies keep doing what they're shipped for.

use dominion_engine::id;
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

#[test]
fn sentry_merchant_beats_double_witch() {
    // The evolved counter (strategies/sentry_merchant.toml), with full bot behaviour, on the
    // kingdom it was validated on (~97%; full bots searching Sentry turns run ~30 games/s).
    let sm = load("sentry_merchant.toml");
    let dw = load("double_witch.toml");
    assert_eq!(sm.name, "Sentry Merchant");
    let kingdom = vec![id::SENTRY, id::MILITIA, id::WITCH, id::MERCHANT, id::SMITHY, id::MARKET, id::CELLAR, id::MOAT, id::WORKSHOP, id::REMODEL];
    let cfg = MatchConfig { games: 500, kingdom, seed: 3, max_turns: 200 };
    let stats = run_match(&[&sm, &dw], &cfg);
    let rate = stats[0].win_rate();
    assert!(rate > 0.9, "Sentry Merchant should beat Double Witch, got {:.1}%", rate * 100.0);
    println!("Sentry Merchant vs Double Witch: {:.1}%", rate * 100.0);
}
