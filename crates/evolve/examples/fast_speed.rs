//! Fast-bot throughput, new defaults against `legacy_defaults`, on the league kingdom of bench/perf.sh and on a mirror of trasher cards.
//!   cargo run --release -p dominion-evolve --example fast_speed -- [games]
use dominion_engine::cards::{self, CardId};
use dominion_evolve::arena::Arena;
use dominion_evolve::baselines::baselines;
use dominion_sim::batch::{run_match, MatchConfig};
use std::time::Instant;

fn kingdom(names: &[&str]) -> Vec<CardId> {
    names.iter().map(|n| cards::by_name(n).unwrap()).collect()
}

fn main() {
    let games: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20000);
    let arena = Arena { opponents: vec![], forbidden: vec![], fast: true, search_cards: None, max_turns: 60 };
    let kingdoms: [&[&str]; 2] = [
        &["Village", "Smithy", "Militia", "Witch", "Market", "Laboratory", "Moat", "Cellar", "Moneylender", "Chapel"],
        &["Remodel", "Trading Post", "Forager", "Lookout", "Amulet", "Anvil", "Courtier", "Stonemason", "Develop", "Crown"],
    ];
    for names in kingdoms {
        let k = kingdom(names);
        let bl = baselines(&k, false, &[]);
        for b in bl.iter().take(6) {
            let new = arena.compile(&b.genome).unwrap();
            let mut old = arena.compile(&b.genome).unwrap();
            old.legacy_defaults = true;
            let cfg = MatchConfig { games, kingdom: k.clone(), seed: 5, max_turns: 60 };
            let time = |s: &dominion_sim::Strategy| {
                let _ = run_match(&[s, s], &MatchConfig { games: games / 10, kingdom: k.clone(), seed: 5, max_turns: 60 });
                let t = Instant::now();
                let _ = run_match(&[s, s], &cfg);
                t.elapsed().as_secs_f64()
            };
            let (tn, to) = (time(&new), time(&old));
            println!("{:<28} new {:.2}s old {:.2}s ({:+.1}%)", b.name, tn, to, (tn / to - 1.0) * 100.0);
        }
    }
}
