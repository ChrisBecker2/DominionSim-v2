//! Analyze the pending decision of a text game state using every CPU.
//!
//! Usage: dominion-analyze <state.txt> [--threads N] [--budget NODES_PER_TASK] [--serial]
//! The state is advanced to its first decision; the deciding player's view is determinized.

use dominion_engine::rng::Rng;
use dominion_engine::text::parse_state;
use dominion_engine::{NoEvents, PlayerView, Step};
use dominion_search::{analyze_parallel, NextHandEvaluator, SearchConfig, Searcher};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut path = None;
    let mut threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let mut budget = u64::MAX;
    let mut serial = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--threads" => {
                threads = args[i + 1].parse().expect("--threads N");
                i += 1;
            }
            "--budget" => {
                budget = args[i + 1].parse().expect("--budget NODES");
                i += 1;
            }
            "--serial" => serial = true,
            p => path = Some(p.to_string()),
        }
        i += 1;
    }
    let path = path.expect("usage: dominion-analyze <state.txt> [--threads N] [--budget NODES] [--serial]");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut g = parse_state(&text).unwrap_or_else(|e| panic!("{e}"));
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("no decision to analyze: {s:?}"),
    };
    let world = PlayerView::new(&g, d.player).determinize(&mut Rng::new(1));
    let cfg = SearchConfig { node_budget: budget, tt_bits: 20, ..Default::default() };
    let eval = NextHandEvaluator::default();
    let a = if serial {
        Searcher::new(cfg.tt_bits).analyze(&world, d.player, &cfg, &eval)
    } else {
        analyze_parallel(&world, d.player, &cfg, &eval, threads)
    };
    println!(
        "Player {} decides; {} nodes, {} transpositions, {:.1} ms ({}):",
        d.player + 1,
        a.nodes,
        a.tt_hits,
        a.elapsed.as_secs_f64() * 1e3,
        if serial { "serial".to_string() } else { format!("{threads} threads") }
    );
    for o in &a.options {
        println!("{:>9.3}{} {}", o.ev, if o.exact { " " } else { "~" }, o.pv);
    }
}
