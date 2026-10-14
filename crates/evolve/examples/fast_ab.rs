//! A/B for the fast bot's defaults on "BM + X" (the search_gap mirror: X plus nine Base cards): the new fast bot against the old one
//! (`Strategy::legacy_defaults`), and the full bot against each.
//!   cargo run --release -p dominion-evolve --example fast_ab -- <games> <card>[,<card>...]
//! Env: FULL=0 skips the full-bot columns; ONLYFAST=1 same. Columns: new-fast v old-fast, full v old-fast (gap before), full v new-fast
//! (gap after), full(new) v full(old).
use dominion_engine::cards::{self, CardId};
use dominion_evolve::arena::Arena;
use dominion_evolve::baselines::baselines;
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::stats::wilson_ci;
use dominion_sim::Strategy;

const BASE: [&str; 9] = ["Village", "Smithy", "Market", "Cellar", "Militia", "Moat", "Workshop", "Mine", "Laboratory"];

fn rate(s: &Strategy, t: &Strategy, cfg: &MatchConfig) -> String {
    let r = run_match(&[s, t], cfg);
    let (lo, hi) = wilson_ci(r[0].wins, r[0].games as f64, dominion_sim::stats::Z_95);
    format!("{:.1}+-{:.1}", r[0].wins / r[0].games as f64 * 100.0, (hi - lo) / 2.0 * 100.0)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let games: u64 = a.first().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let names = a.get(1).cloned().unwrap_or_default();
    let full_too = std::env::var("FULL").map_or(true, |v| v != "0");
    let arena = |fast| Arena { opponents: vec![], forbidden: vec![], fast, search_cards: None, max_turns: 60 };
    println!("| card | new fast v old fast | full v old fast | full v new fast | full(new) v full(old) |");
    println!("|---|---|---|---|---|");
    for name in names.split(',') {
        let Some(x) = cards::by_name(name.trim()) else {
            eprintln!("no card {name}");
            continue;
        };
        let mut kingdom: Vec<CardId> = vec![x];
        for n in BASE {
            if let Some(c) = cards::by_name(n) {
                if c != x && kingdom.len() < 10 {
                    kingdom.push(c);
                }
            }
        }
        let bl = baselines(&kingdom, false, &[]);
        let Some(b) = bl.iter().skip(1).find(|b| b.name.contains(cards::name(x))) else { continue };
        let new_fast = arena(true).compile(&b.genome).unwrap();
        let mut old_fast = arena(true).compile(&b.genome).unwrap();
        old_fast.legacy_defaults = true;
        let cfg = MatchConfig { games, kingdom: kingdom.clone(), seed: 17, max_turns: 60 };
        let nf = rate(&new_fast, &old_fast, &cfg);
        let (mut f_old, mut f_new, mut f_ff) = ("-".to_string(), "-".to_string(), "-".to_string());
        if full_too {
            let full = arena(false).compile(&b.genome).unwrap();
            let mut full_old = arena(false).compile(&b.genome).unwrap();
            full_old.legacy_defaults = true;
            f_old = rate(&full_old, &old_fast, &cfg);
            f_new = rate(&full, &new_fast, &cfg);
            f_ff = rate(&full, &full_old, &cfg);
        }
        println!("| {} | {} | {} | {} | {} |", cards::name(x), nf, f_old, f_new, f_ff);
    }
}
