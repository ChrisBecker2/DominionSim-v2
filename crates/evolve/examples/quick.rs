//! Quick end-to-end run: evolve against Double Witch on the fixed "everything available" kingdom.
use dominion_evolve::{run, Control, EvolveConfig, OpponentSpec};

fn main() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies");
    let read = |f: &str| std::fs::read_to_string(format!("{root}/{f}")).unwrap();
    let mut cfg = EvolveConfig::default();
    cfg.opponents = vec![OpponentSpec { name: "Double Witch".into(), toml: read("double_witch.toml"), weight: 1.0 }];
    cfg.seeds = std::fs::read_dir(root).unwrap().filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok()).collect();
    let args: Vec<String> = std::env::args().collect();
    cfg.generations = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    cfg.validate_every = 5;
    let t = std::time::Instant::now();
    let p = run(&cfg, &Control::default(), &mut |p| {
        if let Some(h) = p.history.last() {
            eprintln!("[{:>5.1}s] {} gen {:>3} best {:.1}% median {:.1}% val {:?} {:.0} g/s | {}", t.elapsed().as_secs_f64(), p.status, h.generation, h.best_win_rate * 100.0, h.median_win_rate * 100.0, h.validated_win_rate.map(|v| (v * 1000.0).round() / 10.0), p.games_per_sec, h.best_summary);
        }
    })
    .unwrap();
    for l in &p.log {
        eprintln!("  {l}");
    }
    if let Some(r) = &p.result {
        println!("{}", r.toml);
    }
}
