//! Differential check between two builds of the engine: plays random games (random kingdoms, random choices) and prints one line per
//! game with a hash of everything that happened (every event and every choice), so that the output of two builds can be diffed.
//!
//!   rand_diff [GAMES] [FIRST_SEED] [FOCUS]
//!
//! `FOCUS` names cards (comma separated) that are always in the kingdom, to make the interactions under test frequent.

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
        let mut log: Vec<Event> = Vec::new();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut steps = 0u32;
        let mix = |h: &mut u64, s: &str| {
            for b in s.bytes() {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x100_0000_01b3);
            }
        };
        let trace = std::env::var("TRACE").ok().and_then(|t| t.parse::<u64>().ok()) == Some(game);
        loop {
            let step = g.advance(&mut log);
            for e in &log {
                if trace {
                    println!("  {e:?}");
                }
                mix(&mut h, &format!("{e:?}"));
            }
            log.clear();
            match step {
                Step::Decision(_) => {
                    g.legal_choices(&mut buf);
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    if trace {
                        println!("> {:?} {c:?}", g.pending_decision().map(|d| (d.player, d.kind)));
                        let st: Vec<String> = g.stack.iter().map(|f| format!("{:?}/{:?}/{}", f.kind, f.zone, f.subject)).collect();
                        println!("    stack {}", st.join(" "));
                    }
                    mix(&mut h, &format!("{c:?}"));
                    g.apply(c, &mut log).unwrap();
                    for e in &log {
                        if trace {
                            println!("  {e:?}");
                        }
                        mix(&mut h, &format!("{e:?}"));
                    }
                    log.clear();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            steps += 1;
            if steps > 400_000 {
                mix(&mut h, "timeout");
                break;
            }
        }
        let names: Vec<&str> = kingdom.iter().map(|&c| cards::name(c)).collect();
        println!("{game} {h:016x} {:?} {}", g.scores(), names.join(","));
    }
}
