//! `dominion-sim` CLI: batch-simulate scripted strategies against each other.
//!
//! ```text
//! dominion-sim match strategies/big_money.toml strategies/double_witch.toml \
//!     --games 100000 --kingdom first-game --seed 1
//! dominion-sim league strategies/ --games 20000
//! ```

use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use dominion_sim::batch::{run_league, run_match, MatchConfig};
use dominion_sim::stats::StratStats;
use dominion_sim::{kingdom, Strategy};

#[derive(Parser)]
#[command(name = "dominion-sim", about = "Batch-simulate Dominion (Base Set, 2nd edition) strategies", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Play N scripted strategies against each other, one per seat, rotating seats.
    Match {
        /// Strategy TOML files, one per seat (2-6).
        #[arg(required = true, num_args = 2..)]
        strategies: Vec<PathBuf>,
        /// Number of games to simulate.
        #[arg(long, default_value_t = 10_000)]
        games: u64,
        /// Number of seats; must equal the number of strategy files given (kept as an explicit,
        /// checked flag since it's easy to typo the strategy list).
        #[arg(long)]
        players: Option<usize>,
        /// "first-game"; "auto" (union of cards the strategies reference, padded to 10 by
        /// ascending card id); "auto:<sets>" (same, padded at random from <sets>, seeded by
        /// --seed); "random" or "random:<sets>" (10 random kingdom cards from <sets>, seeded by
        /// --seed); or a comma-separated list of exactly 10 kingdom card names. <sets> is
        /// "base", "intrigue", or "base+intrigue" (random/random:<sets> without a suffix use both).
        #[arg(long, default_value = "first-game")]
        kingdom: String,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 200)]
        max_turns: u16,
    },
    /// Round-robin: every pair of strategies (from a directory, or a list of files) plays a
    /// 2-player match; prints each strategy's aggregate record.
    League {
        /// A directory of `.toml` strategy files, and/or individual files.
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
        /// Games per pairing.
        #[arg(long, default_value_t = 2_000)]
        games: u64,
        /// See `match --kingdom`'s help for the accepted forms.
        #[arg(long, default_value = "first-game")]
        kingdom: String,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 200)]
        max_turns: u16,
    },
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.cmd {
        Cmd::Match { strategies, games, players, kingdom, seed, max_turns } => cmd_match(strategies, games, players, &kingdom, seed, max_turns),
        Cmd::League { paths, games, kingdom, seed, max_turns } => cmd_league(paths, games, &kingdom, seed, max_turns),
    }
}

fn load_all(paths: &[PathBuf]) -> Result<Vec<Strategy>, String> {
    paths.iter().map(|p| Strategy::load(p)).collect()
}

fn print_kingdom(k: &[dominion_engine::CardId]) {
    let names: Vec<&str> = k.iter().map(|&c| dominion_engine::cards::name(c)).collect();
    println!("Kingdom: {}", names.join(", "));
}

fn cmd_match(paths: Vec<PathBuf>, games: u64, players: Option<usize>, kingdom_spec: &str, seed: u64, max_turns: u16) -> Result<(), String> {
    if let Some(p) = players {
        if p != paths.len() {
            return Err(format!("--players {p} doesn't match the {} strategy files given", paths.len()));
        }
    }
    let strategies = load_all(&paths)?;
    let refs: Vec<&Strategy> = strategies.iter().collect();
    let k = kingdom::resolve(kingdom_spec, &refs, seed)?;
    print_kingdom(&k);
    let cfg = MatchConfig { games, kingdom: k, seed, max_turns };

    let start = Instant::now();
    let stats = run_match(&refs, &cfg);
    let elapsed = start.elapsed();

    print_table(&strategies, &stats);
    println!("\n{games} games in {elapsed:.2?} ({:.0} games/sec)", games as f64 / elapsed.as_secs_f64());
    Ok(())
}

fn cmd_league(paths: Vec<PathBuf>, games: u64, kingdom_spec: &str, seed: u64, max_turns: u16) -> Result<(), String> {
    let files = collect_toml_files(&paths)?;
    let strategies = load_all(&files)?;
    if strategies.len() < 2 {
        return Err("league needs at least 2 strategies".into());
    }
    let refs: Vec<&Strategy> = strategies.iter().collect();
    let k = kingdom::resolve(kingdom_spec, &refs, seed)?;
    print_kingdom(&k);

    let start = Instant::now();
    let cells = run_league(&refs, games, &k, seed, max_turns);
    let elapsed = start.elapsed();

    let n = strategies.len();
    let mut totals = vec![StratStats::default(); n];
    for cell in &cells {
        totals[cell.a].merge(&cell.a_stats);
        totals[cell.b].merge(&cell.b_stats);
    }
    print_table(&strategies, &totals);

    println!("\nHead-to-head win rate (row vs column):");
    print_head_to_head(&strategies, &cells);

    let total_games = cells.len() as u64 * games;
    println!("\n{} pairings, {total_games} games in {elapsed:.2?} ({:.0} games/sec)", cells.len(), total_games as f64 / elapsed.as_secs_f64());
    Ok(())
}

fn collect_toml_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(p)
                .map_err(|e| format!("reading {}: {e}", p.display()))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map_or(false, |ext| ext == "toml"))
                .collect();
            entries.sort();
            out.extend(entries);
        } else {
            out.push(p.clone());
        }
    }
    Ok(out)
}

fn print_table(strategies: &[Strategy], stats: &[StratStats]) {
    println!("{:<28} {:>7} {:>9} {:>16} {:>8} {:>10}", "strategy", "games", "win rate", "95% CI", "avg VP", "avg turns");
    for (s, st) in strategies.iter().zip(stats) {
        let (lo, hi) = st.wilson_ci_95();
        println!(
            "{:<28} {:>7} {:>8.1}% [{:>4.1}%,{:>5.1}%] {:>8.1} {:>10.1}",
            s.name,
            st.games,
            st.win_rate() * 100.0,
            lo * 100.0,
            hi * 100.0,
            st.avg_vp(),
            st.avg_turns(),
        );
    }
}

fn print_head_to_head(strategies: &[Strategy], cells: &[dominion_sim::batch::LeagueCell]) {
    let n = strategies.len();
    let mut grid = vec![vec![f64::NAN; n]; n];
    for cell in cells {
        grid[cell.a][cell.b] = cell.a_stats.win_rate() * 100.0;
        grid[cell.b][cell.a] = cell.b_stats.win_rate() * 100.0;
    }
    let name_w = strategies.iter().map(|s| s.name.len()).max().unwrap_or(8).max(8);
    print!("{:<name_w$}", "");
    for s in strategies {
        print!(" {:>8.8}", s.name);
    }
    println!();
    for i in 0..n {
        print!("{:<name_w$}", strategies[i].name);
        for j in 0..n {
            if i == j {
                print!(" {:>8}", "-");
            } else {
                print!(" {:>7.1}%", grid[i][j]);
            }
        }
        println!();
    }
}
