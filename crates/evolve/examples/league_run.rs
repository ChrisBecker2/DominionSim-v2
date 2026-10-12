//! Run a kingdom league from the command line and print the niche table and crosstable.
//! `cargo run --release -p dominion-evolve --example league_run -- "Card1,Card2,..." [generations] [final_games] [out_dir]`
use dominion_evolve::{run, Control, EvolveConfig, LeagueConfig, Track};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let kingdom: Vec<String> = a[0].split(',').map(|s| s.trim().to_string()).collect();
    let generations = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(40);
    let final_games = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let cfg = EvolveConfig {
        track: Track::Fixed { kingdom },
        league: Some(LeagueConfig { final_games, ..LeagueConfig::default() }),
        forbidden: vec![],
        islands: 6,
        island_size: 24,
        generations,
        polish_games: 4000,
        validate_games: 10_000,
        ..EvolveConfig::default()
    };
    let mut last = 0;
    let t = std::time::Instant::now();
    let p = run(&cfg, &Control::default(), &mut |p| {
        if p.generation != last {
            last = p.generation;
            let h = p.history.last().unwrap();
            println!("[{:>5.0}s] gen {:>3} best {:.1}% niches {} hall {} | {}", t.elapsed().as_secs_f64(), h.generation, h.best_win_rate * 100.0, h.niches, h.hall, h.best_summary);
        }
    })
    .unwrap();
    for l in &p.log {
        println!("  {l}");
    }
    let l = p.league.unwrap();
    println!("\nReference turns {:.1}", l.reference_turns);
    println!("\nNiche champions (vs league):");
    for r in &l.niches {
        println!("  {:.1}%  {} | {} | {}", r.entry.win_rate * 100.0, r.label, r.behaviour, r.entry.summary);
    }
    let t = l.crosstable.unwrap();
    println!("\nCrosstable (row beats column %):");
    for (i, n) in t.names.iter().enumerate() {
        let cells: Vec<String> = t.rate[i].iter().map(|x| format!("{:>3.0}", x * 100.0)).collect();
        println!("{:>2} {:<34} {} | mean {:.1} nash {:.0}%", i, n, cells.join(" "), t.mean_vs_field[i] * 100.0, t.nash[i] * 100.0);
    }
    println!("\nTop:");
    for r in &l.top {
        println!("  {}. {} {} mean {:.1}% nash {:.0}%\n     {}", r.rank, r.name, if r.baseline { "[baseline]" } else { "" }, r.mean_vs_field * 100.0, r.nash * 100.0, r.description);
    }
    if let Some(dir) = a.get(3) {
        std::fs::create_dir_all(dir).unwrap();
        for (i, e) in l.strategies.iter().enumerate() {
            std::fs::write(format!("{dir}/{:02}_{}.toml", i, t.names[i].replace([' ', '+', '(', ')', ':'], "_")), &e.toml).unwrap();
        }
    }
}
