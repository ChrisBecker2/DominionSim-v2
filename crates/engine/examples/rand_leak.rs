//! Plays random games (random kingdoms, random choices) and checks that the stop-moving ledger drains: whenever the effect stack is empty no
//! token may be held (every `Release` frame has run), and no game panics.
//!
//!   rand_leak [GAMES] [FIRST_SEED] [FOCUS]

use dominion_engine::cards::{self, CardId};
use dominion_engine::rng::Rng;
use dominion_engine::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let first: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let focus: Vec<CardId> = args
        .get(3)
        .map(|s| s.split(',').filter(|n| !n.is_empty()).map(|n| cards::by_name(n.trim()).unwrap_or_else(|| panic!("no card {n}"))).collect())
        .unwrap_or_default();
    let pool: Vec<CardId> = cards::kingdom_cards().collect();
    let mut leaks = 0;
    let mut max_marks = 0;
    for game in first..first + games {
        let mut rng = Rng::new(0x5EED ^ game.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let n = 2 + (game % 3) as usize;
        let mut kingdom: Vec<CardId> = focus.clone();
        let mut from = pool.clone();
        while kingdom.len() < 10 {
            let i = rng.below(from.len() as u32) as usize;
            let c = from.swap_remove(i);
            if !kingdom.contains(&c) {
                kingdom.push(c);
            }
        }
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom: kingdom.clone(), seed: game, max_turns: 200, ..Default::default() });
        let mut buf = ChoiceBuf::default();
        let mut steps = 0u32;
        loop {
            let step = g.advance(&mut NoEvents);
            max_marks = max_marks.max(g.turn.led.n);
            match step {
                Step::Decision(d) => {
                    if g.stack.is_empty() && g.turn.led.n != 0 {
                        leaks += 1;
                        let names: Vec<&str> = kingdom.iter().map(|&c| cards::name(c)).collect();
                        println!("game {game}: {} marks held with an empty stack at {:?}: {}", g.turn.led.n, d.kind, names.join(","));
                    }
                    g.legal_choices(&mut buf);
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut NoEvents).unwrap();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            steps += 1;
            if steps > 400_000 {
                break;
            }
        }
    }
    println!("{games} games, {leaks} leaks, at most {max_marks} marks at once");
}
