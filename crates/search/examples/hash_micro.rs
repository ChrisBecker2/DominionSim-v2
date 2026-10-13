//! Micro-benchmark of `turn_hash` on states from random games (so the zones vary); prints a checksum of the hashes, which a
//! pure speed-up must not change.
use dominion_engine::cards;
use dominion_engine::rng::Rng;
use dominion_engine::{ChoiceBuf, GameConfig, GameState, NoEvents, Step};
use dominion_search::turn_hash;

fn main() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(run).unwrap().join().unwrap();
}

fn run() {
    let mut states: Vec<(GameState, u8)> = vec![];
    let mut rng = Rng::new(7);
    let names = ["Village", "Smithy", "Militia", "Witch", "Market", "Laboratory", "Moat", "Cellar", "Moneylender", "Chapel"];
    let kingdom: Vec<_> = names.iter().map(|n| cards::by_name(n).unwrap()).collect();
    for seed in 0..40 {
        let mut g = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed, ..Default::default() });
        g.chance_mode = false;
        let mut n = 0;
        loop {
            match g.advance(&mut NoEvents) {
                Step::Decision(d) => {
                    let mut buf = ChoiceBuf::default();
                    g.legal_choices(&mut buf);
                    n += 1;
                    if n % 3 == 0 {
                        states.push((g, d.player));
                    }
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut NoEvents).unwrap();
                }
                Step::GameOver => break,
                _ => {}
            }
        }
    }
    let mut ck = 0u64;
    for (g, me) in &states { ck = ck.rotate_left(3) ^ turn_hash(g, *me); }
    println!("{} states, checksum {ck:x}", states.len());
    for (name, f) in [("turn_hash", turn_hash as fn(&GameState, u8) -> u64)] {
        let mut best = f64::MAX;
        for _ in 0..7 {
            let t = std::time::Instant::now();
            let mut acc = 0u64;
            for _ in 0..20 {
                for (g, me) in &states {
                    acc ^= f(std::hint::black_box(g), *me);
                }
            }
            std::hint::black_box(acc);
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!("{name}: {:.1} ns per hash", best * 1e9 / (20 * states.len()) as f64);
    }
}
