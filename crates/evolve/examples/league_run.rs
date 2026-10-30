//! Run a kingdom league from the command line and print the niche table, crosstable and kingdom findings.
//! `cargo run --release -p dominion-evolve --example league_run -- "Card1,Card2,..." [generations] [final_games] [out_dir]`
//! (env: ISLANDS, ISLAND_SIZE, RACES=200,400, EXPLOIT_ROUNDS, EXPLOIT_GENS, POLISH, VALIDATE, SEED, FINAL=fast|selective|full (the bots of the final ranking, default fast),
//! SEARCH=off|selective|full with SEARCH_GAP (points) and SEARCH_COST (times a fast game): how much turn search the evaluation bots keep)
use dominion_evolve::{run, Control, EvolveConfig, FinalBots, LeagueConfig, SearchMode, Track};

/// An environment override of a count (`ISLANDS=3`), for quicker runs.
fn env(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let kingdom: Vec<String> = a[0].split(',').map(|s| s.trim().to_string()).collect();
    let generations = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(40);
    let final_games = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let cfg = EvolveConfig {
        track: Track::Fixed { kingdom },
        league: Some(LeagueConfig {
            final_games,
            exploit_rounds: env("EXPLOIT_ROUNDS", 3),
            exploit_generations: env("EXPLOIT_GENS", 30),
            final_bots: match std::env::var("FINAL").as_deref() {
                Ok("full") => FinalBots::Full,
                Ok("selective") => FinalBots::Selective,
                _ => FinalBots::Fast,
            },
            ..LeagueConfig::default()
        }),
        forbidden: vec![],
        islands: env("ISLANDS", 6),
        island_size: env("ISLAND_SIZE", 24),
        generations,
        race_games: if std::env::var("RACES").is_ok() { std::env::var("RACES").unwrap().split(',').filter_map(|x| x.parse().ok()).collect() } else { EvolveConfig::default().race_games },
        polish_games: env("POLISH", 4000) as u64,
        validate_games: env("VALIDATE", 10_000) as u64,
        seed: env("SEED", EvolveConfig::default().seed as usize) as u64,
        fast_eval: std::env::var("SEARCH").map_or(true, |v| v != "full"),
        search_mode: if std::env::var("SEARCH").is_ok_and(|v| v == "selective") { SearchMode::Selective } else { SearchMode::Off },
        search_gap: std::env::var("SEARCH_GAP").ok().and_then(|v| v.parse().ok()).unwrap_or(EvolveConfig::default().search_gap),
        search_budget: env("SEARCH_BUDGET", EvolveConfig::default().search_budget as usize) as u64,
        search_max_cost: std::env::var("SEARCH_COST").ok().and_then(|v| v.parse().ok()).unwrap_or(EvolveConfig::default().search_max_cost),
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
    println!("\nExploiter rounds ({} planned):", l.exploit_rounds);
    for r in &l.exploiters {
        println!(
            "  round {}: {:.1}% (CI {:.1}-{:.1}) vs mix [{}] -> {}",
            r.round,
            r.rate_vs_mix * 100.0,
            r.ci_lo * 100.0,
            r.ci_hi * 100.0,
            r.mix.iter().map(|(n, w)| format!("{:.0}% {n}", w * 100.0)).collect::<Vec<_>>().join(", "),
            if r.added { format!("ADDED as {}", r.name.clone().unwrap_or_default()) } else { "not added".to_string() }
        );
        println!("     per opponent: {}", r.vs.iter().map(|o| format!("{:.1}% vs {}", o.win_rate * 100.0, o.name)).collect::<Vec<_>>().join(", "));
        println!("     {}", r.summary);
        println!("     Nash after: {}", r.nash_after.iter().map(|(n, w)| format!("{:.0}% {n}", w * 100.0)).collect::<Vec<_>>().join(", "));
    }
    println!("  nash exploitability (last exploiter vs its mix, minus 50 points): {:?}", l.nash_exploitability);
    let expl = l.exploitability.clone();
    println!("\nCrosstable (row beats column %):");
    for (i, n) in t.names.iter().enumerate() {
        let cells: Vec<String> = t.rate[i].iter().map(|x| format!("{:>3.0}", x * 100.0)).collect();
        println!("{:>2} {:<34} {} | mean {:.1} nash {:.0}% expl {:.1}", i, n, cells.join(" "), t.mean_vs_field[i] * 100.0, t.nash[i] * 100.0, expl[i] * 100.0);
    }
    println!("\nTop:");
    for r in &l.top {
        println!("  {}. {} {} mean {:.1}% nash {:.0}%\n     {}", r.rank, r.name, if r.baseline { "[baseline]" } else { "" }, r.mean_vs_field * 100.0, r.nash * 100.0, r.description);
    }
    match (&l.findings, &l.findings_error) {
        (Some(f), _) => {
            println!("\nFindings ({} against the field, {} games):", f.champion, f.insight.games);
            for r in &f.field {
                println!("  {:>5.1}% vs {}", r.rate.win * 100.0, r.name);
            }
            println!("  card power (owner's win rate with 0 1 2 3 4 5+ copies at the end):");
            for r in f.power.iter().take(10) {
                let cells: Vec<String> = r.by_count.iter().map(|x| if x.games >= 30 { format!("{:>3.0}", x.win * 100.0) } else { "  -".into() }).collect();
                println!("    {:<20} {}  swing {:.0}", r.card, cells.join(" "), r.swing * 100.0);
            }
            let mut kind = "";
            for x in &f.findings {
                if x.kind != kind {
                    kind = &x.kind;
                    println!("  {kind}:");
                }
                println!("    {:>+6.1}  {}", 100.0 * x.points, x.text);
            }
        }
        (None, Some(e)) => println!("\nNo findings: {e}"),
        _ => {}
    }
    if let Some(dir) = a.get(3) {
        std::fs::create_dir_all(dir).unwrap();
        for (i, e) in l.strategies.iter().enumerate() {
            std::fs::write(format!("{dir}/{:02}_{}.toml", i, t.names[i].replace([' ', '+', '(', ')', ':'], "_")), &e.toml).unwrap();
        }
    }
}
