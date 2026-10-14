//! What each search mode costs on a set of strategies: every pair from a directory of strategy files plays `games` games on a kingdom,
//! once per mode (fast, selective, selective with a small budget, full), and the time per game is printed.
//! `cargo run --release -p dominion-evolve --example mode_cost -- DIR "Card1,Card2,..." [games]`
use dominion_engine::cards;
use dominion_evolve::arena::{set_play_mode, Selective};
use dominion_evolve::EvolveConfig;
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut files: Vec<_> = std::fs::read_dir(&a[0]).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
    files.sort();
    let kingdom: Vec<cards::CardId> = a[1].split(',').map(|n| cards::by_name(n.trim()).unwrap_or_else(|| panic!("{n}"))).collect();
    let games: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(200);
    let cfg = EvolveConfig::default();
    let sel = Selective { cards: dominion_sim::search_gap::mask(cfg.search_gap, cfg.search_max_cost), budget: cfg.search_budget };
    let selb = Selective { cards: dominion_sim::search_gap::mask(cfg.search_gap, 300.0), budget: 100 };
    println!("{} strategies, {} pairs, {games} games each", files.len(), files.len() * (files.len() - 1) / 2);
    for (name, mode) in [("fast", 0), ("selective", 1), ("selective, budget 100", 2), ("full", 3)] {
        let strats: Vec<Strategy> = files
            .iter()
            .map(|f| {
                let mut s = Strategy::load(f).unwrap();
                match mode {
                    0 => set_play_mode(&mut s, true, None),
                    1 => set_play_mode(&mut s, true, Some(&sel)),
                    2 => set_play_mode(&mut s, true, Some(&selb)),
                    _ => {}
                }
                s
            })
            .collect();
        let t = Instant::now();
        let mut played = 0;
        for i in 0..strats.len() {
            for j in i + 1..strats.len() {
                let c = MatchConfig { games, kingdom: kingdom.clone(), seed: (i * 100 + j) as u64, max_turns: 60 };
                played += run_match(&[&strats[i], &strats[j]], &c)[0].games;
            }
        }
        let dt = t.elapsed().as_secs_f64();
        println!("{name:<24} {dt:8.2} s for {played} games: {:8.1} us/game (wall, all cores)", dt / played as f64 * 1e6);
    }
}
