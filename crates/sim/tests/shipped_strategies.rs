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

#[test]
fn conquest_bm_beats_big_money_ultimate() {
    // The Conquest Event (Empires): two Silvers and a VP token for each Silver gained this turn (~80%).
    beats_big_money_ultimate("conquest_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Gardens,Conquest", 2000, 0.7);
}

#[test]
fn forum_bm_beats_big_money_ultimate() {
    // Two Forums (Empires), ~74%.
    beats_big_money_ultimate("forum_bm.toml", "Forum,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.65);
}

#[test]
fn wild_hunt_bm_beats_big_money_ultimate() {
    // Two Wild Hunts (Empires, a Gathering card), ~70%.
    beats_big_money_ultimate("wild_hunt_bm.toml", "Wild Hunt,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.62);
}

#[test]
fn castles_bm_beats_big_money_ultimate() {
    // The Castles pile bought behind Provinces by a rich deck (Empires), ~67%.
    beats_big_money_ultimate("castles_bm.toml", "Castles,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.6);
}

#[test]
fn skulk_bm_beats_big_money_ultimate() {
    // Two Skulks (Nocturne, a Doom card): a Gold on gain and a Hex per play, ~80%.
    beats_big_money_ultimate("skulk_bm.toml", "Skulk,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.7);
}

#[test]
fn sacred_grove_bm_beats_big_money_ultimate() {
    // Two Sacred Groves (Nocturne, a Fate card): a terminal Gold with a Boon, ~72%.
    beats_big_money_ultimate("sacred_grove_bm.toml", "Sacred Grove,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.62);
}

#[test]
fn werewolf_bm_beats_big_money_ultimate() {
    // Two Werewolves (Nocturne): a Smithy by day, a Hex at Night, ~69%.
    beats_big_money_ultimate("werewolf_bm.toml", "Werewolf,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.6);
}

#[test]
fn jack_of_all_trades_bm_beats_big_money_ultimate() {
    // Up to two Jacks of All Trades (a Silver per play, draw to five), ~78%.
    beats_big_money_ultimate("jack_of_all_trades_bm.toml", "Jack of All Trades,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.7);
}

#[test]
fn fools_gold_bm_beats_big_money_ultimate() {
    // Fool's Gold bought on every hand with nothing better (worth $4 after the first), ~63%.
    beats_big_money_ultimate("fools_gold_bm.toml", "Fool's Gold,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.57);
}

#[test]
fn border_village_engine_beats_big_money_ultimate() {
    // Border Villages gaining Smithies and Markets, ~65%.
    beats_big_money_ultimate("border_village_engine.toml", "Border Village,Inn,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit", 2000, 0.58);
}

#[test]
fn imperial_envoy_bm_beats_big_money_ultimate() {
    // Rising Sun: terminal +5 Cards +1 Buy for 2 Debt (~73%).
    beats_big_money_ultimate("imperial_envoy_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Imperial Envoy", 2000, 0.6);
}

#[test]
fn samurai_bm_beats_big_money_ultimate() {
    // Rising Sun: a Duration that is +$1 every turn (~69%).
    beats_big_money_ultimate("samurai_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Samurai", 2000, 0.6);
}

#[test]
fn kitsune_bm_beats_big_money_ultimate() {
    // Rising Sun: an Omen that curses (~68%); the Prophecy it brings closer is dealt with the kingdom.
    beats_big_money_ultimate("kitsune_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Kitsune", 2000, 0.58);
}

#[test]
fn credit_bm_beats_big_money_ultimate() {
    // The Credit Event: a Gold for 6 Debt early (~53%).
    beats_big_money_ultimate("credit_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Gardens,Credit", 3000, 0.5);
}

#[test]
fn pageant_bm_beats_big_money_ultimate() {
    // Renaissance: the Pageant Project turns a leftover dollar into a Coffers every turn (~88%).
    beats_big_money_ultimate("pageant_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Gardens,Pageant", 2000, 0.7);
}

#[test]
fn silk_merchant_bm_beats_big_money_ultimate() {
    // Renaissance: two Silk Merchants, Coffers and Villagers when gained or trashed (~77%).
    beats_big_money_ultimate("silk_merchant_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Silk Merchant", 2000, 0.65);
}

#[test]
fn seer_bm_beats_big_money_ultimate() {
    // Renaissance: Seer is a cantrip that finds the cheap cards (~57%; the searching bot is slow, so few games).
    beats_big_money_ultimate("seer_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Seer", 800, 0.5);
}

#[test]
fn canal_bm_beats_big_money_ultimate() {
    // Renaissance: the Canal Project, every card costs $1 less on your turns (~72%).
    beats_big_money_ultimate("canal_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Gardens,Canal", 2000, 0.6);
}

#[test]
fn livery_bm_beats_big_money_ultimate() {
    // The Livery (Menagerie): $3 and a Horse for every card costing $4 or more gained that turn (~79%).
    beats_big_money_ultimate("livery_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Livery", 2000, 0.7);
}

#[test]
fn cardinal_bm_beats_big_money_ultimate() {
    // The Cardinal (Menagerie): a Militia-like Attack that Exiles the better of two cards from the deck (~54%).
    beats_big_money_ultimate("cardinal_bm.toml", "Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Workshop,Bandit,Cardinal", 3000, 0.51);
}
