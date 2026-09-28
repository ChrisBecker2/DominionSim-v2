//! `dominion-lab`: serve the Strategy Lab web UI, or run an evolution headlessly.
//!
//! `dominion-lab [serve] [--port 8723] [--strategies <dir>] [--no-open]`
//! `dominion-lab run --config <lab-request.json> [--strategies <dir>]`

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use dominion_lab::{build_config, save_strategy, AppState, LabRequest};

fn get_opt(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}
fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run_headless(&args[1..]),
        Some("serve") => serve(&args[1..]),
        _ => serve(&args),
    }
}

fn serve(args: &[String]) {
    let port: u16 = get_opt(args, "--port").and_then(|s| s.parse().ok()).unwrap_or(8723);
    let dir = get_opt(args, "--strategies").unwrap_or_else(|| "./strategies".to_string());
    let no_open = has_flag(args, "--no-open");

    let state = AppState::new(PathBuf::from(dir));
    let server = match tiny_http::Server::http(("127.0.0.1", port)) {
        Ok(s) => std::sync::Arc::new(s),
        Err(e) => {
            eprintln!("failed to bind 127.0.0.1:{port}: {e}");
            std::process::exit(1);
        }
    };
    let url = format!("http://127.0.0.1:{port}/");
    println!("Strategy Lab listening on {url}");
    if !no_open {
        #[cfg(windows)]
        let _ = Command::new("cmd").args(["/C", "start", "", &url]).spawn();
        #[cfg(not(windows))]
        let _ = Command::new("xdg-open").arg(&url).spawn();
    }
    dominion_lab::accept_loop(state, server);
}

fn run_headless(args: &[String]) {
    let Some(config_path) = get_opt(args, "--config") else {
        eprintln!("usage: dominion-lab run --config <lab-request.json> [--strategies <dir>]");
        std::process::exit(2);
    };
    let dir = PathBuf::from(get_opt(args, "--strategies").unwrap_or_else(|| "./strategies".to_string()));

    let body = match std::fs::read_to_string(&config_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("reading {config_path}: {e}");
            std::process::exit(1);
        }
    };
    let req: LabRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("parsing {config_path}: {e}");
            std::process::exit(1);
        }
    };
    let cfg = match build_config(&dir, &req) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let control = dominion_evolve::Control::default();
    let t = Instant::now();
    let mut last_gen = 0usize;
    let result = dominion_evolve::run(&cfg, &control, &mut |p| {
        if let Some(h) = p.history.last() {
            if h.generation != last_gen {
                last_gen = h.generation;
                let val = h.validated_win_rate.map(|v| format!("{:.1}%", v * 100.0)).unwrap_or_else(|| "-".into());
                println!(
                    "[{:>6.1}s] gen {:>3}/{} best {:>5.1}% median {:>5.1}% val {:>6} {:>6.0} g/s | {}",
                    t.elapsed().as_secs_f64(),
                    h.generation,
                    p.generations,
                    h.best_win_rate * 100.0,
                    h.median_win_rate * 100.0,
                    val,
                    p.games_per_sec,
                    h.best_summary
                );
            }
        }
    });

    match result {
        Ok(p) => {
            for l in &p.log {
                println!("  {l}");
            }
            match &p.result {
                Some(r) => match save_strategy(&dir, &r.toml, None) {
                    Ok(path) => println!("saved: {path}"),
                    Err(e) => {
                        eprintln!("save failed: {e}");
                        std::process::exit(1);
                    }
                },
                None => {
                    eprintln!("no result produced");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
