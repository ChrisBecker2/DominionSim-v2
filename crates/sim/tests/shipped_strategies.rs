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

fn beats_big_money_ultimate(file: &str, kingdom_names: &str, games: u64, at_least: f64) {
    let s = load(file);
    let bmu = load("big_money_ultimate.toml");
    let kingdom = dominion_sim::kingdom::resolve(kingdom_names, &[&s, &bmu], 1).unwrap();
    let cfg = MatchConfig { games, kingdom, seed: 5, max_turns: 200 };
    let stats = run_match(&[&s, &bmu], &cfg);
    let rate = stats[0].win_rate();
    assert!(rate > at_least, "{} should beat Big Money Ultimate on {kingdom_names}, got {:.1}%", s.name, rate * 100.0);
    println!("{} vs Big Money Ultimate: {:.1}%", s.name, rate * 100.0);
}

#[test]
fn ball_bm_beats_big_money_ultimate() {
    // The Ball Event (Adventures): two Silvers for $5 and a -$1 token (~58%).
    beats_big_money_ultimate("ball_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Gardens,Ball", 2000, 0.53);
}

#[test]
fn page_champion_beats_big_money_ultimate() {
    // A Page that climbs the Traveller line, with one Smithy (~58%).
    beats_big_money_ultimate("page_champion.toml", "Page,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.53);
}

#[test]
fn reserve_engine_beats_big_money_ultimate() {
    // Reserve cards on the Tavern mat: Royal Carriage, Wine Merchant, Ratcatcher with a Smithy (~55%).
    beats_big_money_ultimate("reserve_engine.toml", "Royal Carriage,Wine Merchant,Ratcatcher,Smithy,Village,Market,Militia,Moat,Cellar,Workshop", 1500, 0.51);
}
