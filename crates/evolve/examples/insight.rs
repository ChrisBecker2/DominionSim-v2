//! Print `dominion_evolve::insight`'s report on two strategies: opening splits, gain and trash heat maps, win rate by copies (the points
//! where one more copy starts to lose), final-deck pairs, decision trees at turns 5/9/13/17, turning points, and (with `--explain`) what
//! each rule of the first strategy is worth.
//! `cargo run --release -p dominion-evolve --example insight -- "<kingdom>" a.toml b.toml [games] [seed] [--fast] [--explain]`
use dominion_evolve::insight::{self, HeatRow, Rate, TURNS};
use dominion_sim::{kingdom, Strategy};

fn pct(r: &Rate) -> String {
    if r.games == 0 { "   -  ".into() } else { format!("{:5.1}%", 100.0 * r.win) }
}

fn shade(x: f64) -> char {
    match x {
        x if x < 0.02 => ' ',
        x if x < 0.1 => '.',
        x if x < 0.25 => ':',
        x if x < 0.5 => '+',
        x if x < 0.8 => '*',
        _ => '#',
    }
}

fn heat(title: &str, rows: &[HeatRow]) {
    println!("\n{title} per own turn (avg per game: ' ' <.02 '.' <.1 ':' <.25 '+' <.5 '*' <.8 '#'); per game; win rate with one before turn 10 / not:");
    let header: String = (1..=TURNS).map(|t| if t % 5 == 0 { char::from_digit((t % 10) as u32, 10).unwrap() } else { '.' }).collect();
    println!("  {:<16}{header}", "");
    for r in rows {
        let cells: String = r.per_turn.iter().map(|&x| shade(x)).collect();
        println!("  {:<16}{cells}  {:5.2}   {} ({:>5}) / {}", r.card, r.total, pct(&r.early), r.early.games, pct(&r.late));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let fast = args.iter().any(|a| a == "--fast");
    let explain = args.iter().any(|a| a == "--explain");
    let a: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let a_src = std::fs::read_to_string(a[1]).unwrap();
    let mut strats = [Strategy::parse(&a_src).unwrap(), Strategy::load(a[2].as_ref()).unwrap()];
    if fast {
        strats.iter_mut().for_each(insight::make_fast);
    }
    let games: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(4000);
    let seed: u64 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);
    let kingdom = kingdom::resolve(a[0], &[], seed).unwrap();
    let r = insight::analyze(&kingdom, &strats[0], &strats[1], games, seed);
    println!("{} vs {}: {} games", r.sides[0].name, r.sides[1].name, r.games);
    for s in &r.sides {
        println!("  {:<30} wins {}  avg VP {:.1}  avg turns {:.1}", s.name, pct(&s.rate), s.avg_vp, s.avg_turns);
    }
    for s in &r.sides {
        println!("\n==================== {} ====================", s.name);
        println!("\nWin rate by opening split (mine), and by the opponent's:");
        for row in &s.splits {
            let vs: String = row.vs.iter().map(|(k, x)| format!("   | opp {k}: {} ({})", pct(x), x.games)).collect();
            println!("  {}: {} of {:>5} games{vs}", row.split, pct(&row.rate), row.rate.games);
            for o in row.openings.iter().take(4) {
                println!("        opens {:<36} {:>5} games, wins {}", o.cards, o.rate.games, pct(&o.rate));
            }
        }
        heat("Gains", &s.gains);
        heat("Trashes", &s.trashes);
        println!("\nDifferently named cards at the end: {:.1} in wins, {:.1} in losses", s.names_in_wins, s.names_in_losses);
        for (title, curves) in [("gained by my turn 10", &s.by_turn_10), ("owned at the end", &s.at_end)] {
            println!("\nWin rate by copies {title} (games):");
            for c in curves {
                let cells: String = c.points.iter().map(|(k, x)| format!(" {k}:{}({:>4})", pct(x), x.games)).collect();
                println!("  {:<16}{cells}", c.card);
            }
        }
        println!("\nDecision trees: the position at the start of own turn N, and how often games from there are won:");
        for t in &s.trees {
            println!("  turn {} ({} games, {}):", t.turn, t.rate.games, pct(&t.rate));
            for l in &t.leaves {
                println!("    {} ({:>5})  {}", pct(&l.rate), l.rate.games, l.path.join(" and "));
            }
        }
        println!("\nTurning points: gaining the card in the window moves the win estimate this much more than not gaining it:");
        for w in &s.swings {
            println!("  {:>+6.1} pts  {:<8} {:<18} {:<20} ({} games)", 100.0 * w.effect, w.who, w.card, w.window, w.games);
        }
    }
    if let Some(p) = &r.prophecy {
        println!("\nProphecy happened in {} of {} games", p.happened, r.games);
        for (s, (x, t)) in r.sides.iter().zip(&p.made_by) {
            println!("  made by {:<30} {:>5} games (their turn {:.1} on average), they win {}", s.name, x.games, t, pct(x));
        }
    }
    if explain {
        match insight::explain(&kingdom, &a_src, &strats[1], games, seed, fast) {
            Ok(e) => {
                println!("\nWhat each rule of {} is worth ({:.1}% with all of them, {} games each):", r.sides[0].name, 100.0 * e.with, e.games);
                for x in &e.rules {
                    let bars: String = x.per_turn.iter().map(|&d| shade(d.abs())).collect();
                    println!("  {:>+6.1} pts  {:<6} {:<46} {:<12} {bars}", 100.0 * x.delta, x.list, x.rule, x.when);
                }
            }
            Err(e) => println!("\n{e}"),
        }
    }
}
