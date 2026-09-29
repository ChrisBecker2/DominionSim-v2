//! How often does the play search choose differently from the strategy's rule order, and what
//! does each cost? Plays games with the strategy as written (full behaviour) and, at every
//! action-play decision with at least two different cards to choose from, also asks the rule
//! order, timing both.
//!
//!   cargo run -p dominion-evolve --release --example play_agreement -- <strategy.toml> <opponent.toml> [games]
use dominion_engine::cards::{self, id};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::{play_game, Agent, GameConfig, GameState, NoEvents, PlayerView};
use dominion_sim::eval::is_play_decision;
use dominion_sim::Strategy;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Default)]
struct Stats {
    decisions: u64,
    multi: u64,
    differ: u64,
    search_secs: f64,
    rule_secs: f64,
    other_secs: f64,
    /// (rule pick, search pick) -> count
    pairs: BTreeMap<(String, String), u64>,
}

struct Probe<'a> {
    s: &'a Strategy,
    st: Stats,
}

impl Agent for Probe<'_> {
    fn name(&self) -> &str {
        &self.s.name
    }
    fn choose(&mut self, view: &PlayerView, d: &Decision, choices: &[Choice]) -> Choice {
        self.st.decisions += 1;
        let distinct = {
            let mut v: Vec<Choice> = choices.iter().copied().filter(|c| matches!(c, Choice::Card(_))).collect();
            v.dedup();
            v.len()
        };
        if is_play_decision(view.turn().player, d) && distinct >= 2 && d.player == view.me() {
            self.st.multi += 1;
            let t = Instant::now();
            let rule = self.s.rule_play(view, d, choices);
            self.st.rule_secs += t.elapsed().as_secs_f64();
            let t = Instant::now();
            // Exactly what the bot plays (its win check, then the play search).
            let search = self.s.decide(view, d, choices);
            self.st.search_secs += t.elapsed().as_secs_f64();
            if search != rule {
                self.st.differ += 1;
                let name = |c: Choice| match c {
                    Choice::Card(c) => cards::name(c).to_string(),
                    _ => "(stop)".to_string(),
                };
                *self.st.pairs.entry((name(rule), name(search))).or_default() += 1;
            }
            return search;
        }
        let t = Instant::now();
        let c = self.s.decide(view, d, choices);
        self.st.other_secs += t.elapsed().as_secs_f64();
        c
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = Strategy::load(args[1].as_ref()).unwrap();
    let b = Strategy::load(args[2].as_ref()).unwrap();
    let games: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(200);
    let kingdom = vec![id::SENTRY, id::MILITIA, id::POACHER, id::MERCHANT, id::WITCH, id::VILLAGE, id::SMITHY, id::MARKET, id::CELLAR, id::MOAT];
    let mut p = Probe { s: &a, st: Stats::default() };
    let mut opp = dominion_sim::StrategyAgent::new(&b);
    let t0 = Instant::now();
    for g in 0..games {
        let mut st = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed: 1000 + g, max_turns: 200 });
        let mut agents: [&mut dyn Agent; 2] = if g % 2 == 0 { [&mut p, &mut opp] } else { [&mut opp, &mut p] };
        play_game(&mut st, &mut agents, &mut NoEvents);
    }
    let s = &p.st;
    println!("{games} games in {:.1}s (single thread)", t0.elapsed().as_secs_f64());
    println!("decisions by the strategy: {}", s.decisions);
    println!("action-play decisions with a real choice: {} ({:.1} per game)", s.multi, s.multi as f64 / games as f64);
    println!("search picked differently from rule order: {} ({:.1}%)", s.differ, 100.0 * s.differ as f64 / s.multi.max(1) as f64);
    println!(
        "time: search {:.2}s ({:.1} ms/decision), rule order {:.4}s, all other decisions {:.2}s",
        s.search_secs,
        1000.0 * s.search_secs / s.multi.max(1) as f64,
        s.rule_secs,
        s.other_secs
    );
    let mut pairs: Vec<_> = s.pairs.iter().collect();
    pairs.sort_by(|x, y| y.1.cmp(x.1));
    for ((r, sr), n) in pairs.iter().take(10) {
        println!("  rules play {r:<10} search plays {sr:<10} x{n}");
    }
}
