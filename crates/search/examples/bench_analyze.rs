//! Benchmark of `Searcher::analyze` on a few fixed positions: prints nodes, transposition hits and
//! every root option's value at full precision (these must not change with a pure speed-up), then
//! the best-of-N time and nodes per second.
//!
//!   cargo run --release -p dominion-search --example bench_analyze [-- REPEATS]

use dominion_engine::text::parse_state;
use dominion_engine::{GameState, NoEvents, Step};
use dominion_search::{NextHandEvaluator, SearchConfig, Searcher};

const KINGDOM: &str = "Village, Smithy, Moneylender, Chapel, Cellar, Laboratory, Market, Militia, Throne Room, Library";

fn scenario(kingdom: &str, hand: &str, deck: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: {kingdom}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\nseed: 1\n\n\
         [player 1]\nhand: {hand}\ndeck top:\ndeck: {deck}\ndiscard:\nin play:\n\n\
         [player 2]\nhand: 5 Copper\ndeck top:\ndeck: 2 Copper, 3 Estate\ndiscard:\nin play:\n"
    );
    let mut g = parse_state(&text).unwrap();
    g.chance_mode = true;
    match g.advance(&mut NoEvents) {
        Step::Decision(_) => g,
        s => panic!("expected a decision, got {s:?}"),
    }
}

fn main() {
    // GameState is large and the search recursive: run on a thread with a roomy stack.
    std::thread::Builder::new().stack_size(256 << 20).spawn(run).unwrap().join().unwrap();
}

fn run() {
    let repeats: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(5);
    let cases = [
        ("Village+Smithy+Lab", "Village, Smithy, Laboratory, 2 Copper", "6 Copper, 3 Estate, 2 Silver, Gold, Village, Smithy"),
        ("Library", "Library, Village, 2 Copper, Estate", "6 Copper, 3 Estate, 2 Silver, Smithy, Village"),
        ("Throne+Smithy", "Throne Room, Smithy, Village, 2 Copper", "6 Copper, 3 Estate, 2 Silver, Gold"),
        ("Cellar", "Cellar, Market, Estate, Estate, Copper", "6 Copper, 3 Estate, 2 Silver, Gold, Laboratory"),
    ];
    let mut total_nodes = 0u64;
    let mut total_secs = 0.0;
    for (name, hand, deck) in cases {
        let g = scenario(KINGDOM, hand, deck);
        let cfg = SearchConfig { node_budget: u64::MAX, tt_bits: 22, ..Default::default() };
        let mut best = f64::MAX;
        let mut nodes = 0;
        for r in 0..repeats {
            let a = Searcher::new(cfg.tt_bits).analyze(&g, 0, &cfg, &NextHandEvaluator::default());
            best = best.min(a.elapsed.as_secs_f64());
            nodes = a.nodes;
            if r == 0 {
                println!("RESULT {name}: nodes {} tt_hits {}", a.nodes, a.tt_hits);
                for o in &a.options {
                    println!("RESULT   {:?} ev {:?} exact {} {}", o.choice, o.ev, o.exact, o.pv);
                }
            }
        }
        total_nodes += nodes;
        println!("TIME {name:<20} {nodes:>9} nodes  {:>8.1} ms  {:>6.2} M nodes/s", best * 1e3, nodes as f64 / best / 1e6);
        total_secs += best;
    }
    println!("TIME total {:.1} ms  {:.2} M nodes/s", total_secs * 1e3, total_nodes as f64 / total_secs / 1e6);
}
