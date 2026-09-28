//! Time every bot decision in a few games and report the slowest (with the position text).
use dominion_engine::cards::id;
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::{play_game, Agent, GameConfig, GameState, NoEvents, PlayerView};
use dominion_sim::Strategy;
use std::time::Instant;

struct Timed<'a> {
    s: &'a Strategy,
    log: Vec<(f64, String, String)>,
}
impl Agent for Timed<'_> {
    fn name(&self) -> &str {
        &self.s.name
    }
    fn choose(&mut self, view: &PlayerView, d: &Decision, choices: &[Choice]) -> Choice {
        let t = Instant::now();
        let c = self.s.decide(view, d, choices);
        let dt = t.elapsed().as_secs_f64();
        if dt > 0.05 {
            self.log.push((dt, format!("{:?}", d.kind), dominion_engine::text::format_state(&view.determinize(&mut dominion_engine::rng::Rng::new(view.stable_seed())))));
        }
        c
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = Strategy::load(args[1].as_ref()).unwrap();
    let b = Strategy::load(args[2].as_ref()).unwrap();
    let games: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3);
    let kingdom = vec![id::SENTRY, id::MILITIA, id::WITCH, id::MERCHANT, id::POACHER];
    let (mut x, mut y) = (Timed { s: &a, log: vec![] }, Timed { s: &b, log: vec![] });
    for seed in 1..=games {
        let mut st = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed, max_turns: 200 });
        let t = Instant::now();
        let mut agents: [&mut dyn Agent; 2] = [&mut y, &mut x];
        play_game(&mut st, &mut agents, &mut NoEvents);
        println!("game {seed}: {:.2}s", t.elapsed().as_secs_f64());
    }
    for (who, t) in [("candidate", &mut x), ("double witch", &mut y)] {
        t.log.sort_by(|p, q| q.0.total_cmp(&p.0));
        let total: f64 = t.log.iter().map(|l| l.0).sum();
        println!("{who}: {} slow decisions (>50ms), {:.2}s total", t.log.len(), total);
        for (dt, k, s) in t.log.iter().take(3) {
            println!("--- {dt:.2}s {k}\n{s}");
        }
    }
}
