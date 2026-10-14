//! Fast-vs-full gap per kingdom card: for every card X, "BM + X" (the league's baseline) plays itself, once as a full bot
//! (turn search) and once as a fast bot (rules only), on X plus nine Base cards. Prints a markdown table.
//! `cargo run --release -p dominion-evolve --example search_gap -- [games] [filter-substring]`
//! Columns: full-vs-fast win rate, selective(X)-vs-fast win rate (search only decisions involving X), the cost of a full / selective
//! game relative to a fast one.
use dominion_engine::cards::{self, CardId};
use dominion_evolve::arena::{Arena, set_fast};
use dominion_evolve::baselines::baselines;
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::stats::wilson_ci;
use dominion_sim::Strategy;
use std::time::Instant;

const BASE: [&str; 9] = ["Village", "Smithy", "Market", "Cellar", "Militia", "Moat", "Workshop", "Mine", "Laboratory"];

fn rate(wins: f64, games: u64) -> (f64, f64) {
    let (lo, hi) = wilson_ci(wins, games as f64, dominion_sim::stats::Z_95);
    (wins / games as f64, (hi - lo) / 2.0)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let games: u64 = a.first().and_then(|s| s.parse().ok()).unwrap_or(4000);
    let filter = a.get(1).cloned().unwrap_or_default();
    let arena = |fast| Arena { opponents: vec![], forbidden: vec![], fast, search_cards: None, max_turns: 60 };
    println!("| card | class | full v fast | sel v fast | full cost | sel cost | baseline |");
    println!("|---|---|---|---|---|---|---|");
    let mut cards_to_do: Vec<CardId> = cards::kingdom_cards().filter(|&c| dominion_evolve::baselines::is_money_partner(c)).collect();
    cards_to_do.retain(|&c| filter.is_empty() || cards::name(c).contains(&filter));
    // SKIP="A,B": leave cards out; AFTER="Name": resume after that card (a long run can be continued).
    if let Ok(skip) = std::env::var("SKIP") {
        cards_to_do.retain(|&c| !skip.split(',').any(|n| n.trim() == cards::name(c)));
    }
    if let Ok(after) = std::env::var("AFTER") {
        if let Some(i) = cards_to_do.iter().position(|&c| cards::name(c) == after) {
            cards_to_do.drain(..=i);
        }
    }
    for x in cards_to_do {
        let mut kingdom: Vec<CardId> = vec![x];
        for n in BASE {
            if let Some(c) = cards::by_name(n) {
                if c != x && kingdom.len() < 10 {
                    kingdom.push(c);
                }
            }
        }
        eprintln!("{}", cards::name(x));
        let bl = baselines(&kingdom, false, &[]);
        let Some(b) = bl.iter().skip(1).find(|b| b.name.contains(cards::name(x))) else { continue };
        let res = std::panic::catch_unwind(|| {
            let fast = arena(true).compile(&b.genome).unwrap();
            let full = arena(false).compile(&b.genome).unwrap();
            let mut full = full;
            let mut sel = arena(false).compile(&b.genome).unwrap();
            // BUDGET=n: the second search pass gets n nodes (<= 1000: only the first pass).
            if let Some(n) = std::env::var("BUDGET").ok().and_then(|v| v.parse().ok()) {
                full.search_budget = n;
                sel.search_budget = n;
            }
            set_fast(&mut sel, true);
            sel.search_play = true;
            let mut mask = vec![false; cards::NUM_CARDS];
            mask[x as usize] = true;
            sel.set_search_cards(mask);
            let cfg = MatchConfig { games, kingdom: kingdom.clone(), seed: 17, max_turns: 60 };
            let timed = |s: &Strategy, t: &Strategy| {
                let t0 = Instant::now();
                let r = run_match(&[s, t], &cfg);
                (r, t0.elapsed().as_secs_f64())
            };
            // The fast bots are so cheap that their time is measured over 20 times the games.
            let cfg_fast = MatchConfig { games: games * 20, kingdom: kingdom.clone(), seed: 17, max_turns: 60 };
            let _ = run_match(&[&fast, &fast], &cfg_fast);
            let t0 = Instant::now();
            let _ = run_match(&[&fast, &fast], &cfg_fast);
            let t_fast = t0.elapsed().as_secs_f64() / 20.0;
            let (_, t_full) = timed(&full, &full);
            let (_, t_sel) = timed(&sel, &sel);
            // TIMEONLY=1: skip the win rates (the gaps are known).
            let (rf, rs) = if std::env::var("TIMEONLY").is_ok() {
                ((f64::NAN, f64::NAN), (f64::NAN, f64::NAN))
            } else {
                let (r_full, _) = timed(&full, &fast);
                let (r_sel, _) = timed(&sel, &fast);
                (rate(r_full[0].wins, r_full[0].games), rate(r_sel[0].wins, r_sel[0].games))
            };
            (rf, rs, t_full / t_fast, t_sel / t_fast, t_fast)
        });
        let class = if cards::is_choice_free(x) { "free" } else if cards::is_order_sensitive(x) { "order" } else { "choice" };
        match res {
            Ok((f, s, cf, cs, tf)) => println!("| {} | {} | {:.1} +-{:.1} | {:.1} +-{:.1} | {:.1}x | {:.1}x | {} | fast {:.0} us/game", cards::name(x), class, f.0 * 100.0, f.1 * 100.0, s.0 * 100.0, s.1 * 100.0, cf, cs, b.name, tf / games as f64 * 1e6),
            Err(_) => println!("| {} | {} | error | | | | |", cards::name(x), class),
        }
    }
}
