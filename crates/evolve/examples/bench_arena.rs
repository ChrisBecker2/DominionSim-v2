use dominion_engine::cards;
use dominion_evolve::{arena::Opponent, genome::Genome, Arena, Scenario};
use dominion_sim::Strategy;
use std::time::Instant;

fn main() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies");
    let dw = std::fs::read_to_string(format!("{root}/double_witch.toml")).unwrap();
    let bm = std::fs::read_to_string(format!("{root}/big_money_ultimate.toml")).unwrap();
    let full: Vec<u8> = cards::kingdom_cards().collect();
    let ten: Vec<u8> = cards::FIRST_GAME.iter().copied().chain([cards::id::WITCH]).filter(|&c| c != cards::id::MINE).collect();
    for win in [true, false] {
        let mut opp = Strategy::parse(&dw).unwrap();
        opp.search_play = false;
        opp.win_this_turn = win;
        let arena = Arena { opponents: vec![Opponent { name: "DW".into(), strategy: opp, weight: 1.0 }], forbidden: vec![cards::id::WITCH], fast: false, max_turns: 60 };
        let mut cand = arena.compile(&Genome::from_toml(&bm).unwrap()).unwrap();
        cand.win_this_turn = win;
        for (name, k) in [("full", &full), ("ten", &ten)] {
            let sc = [Scenario { kingdom: k.clone() }];
            let t = Instant::now();
            let s = arena.evaluate(&cand, &sc, 100_000, 1);
            let dt = t.elapsed().as_secs_f64();
            println!("win_check={win} {name}: {:.0} games/s, win {:.1}%", s.played as f64 / dt, s.win_rate() * 100.0);
        }
    }
}
