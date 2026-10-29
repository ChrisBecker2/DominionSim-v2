use dominion_engine::cards::{self, id};
use dominion_engine::{play_game, Agent, Event, GameConfig, GameState};
use dominion_sim::{Strategy, StrategyAgent};

/// `log_game a.toml b.toml [seed] [kingdom] [--fast]`: one game, a line per turn. Runs on a thread with a large stack (the searching bots
/// recurse deeply).
fn main() {
    std::thread::Builder::new().stack_size(256 << 20).spawn(run).unwrap().join().unwrap();
}

fn run() {
    let all: Vec<String> = std::env::args().collect();
    let fast = all.iter().any(|a| a == "--fast");
    let args: Vec<String> = all.into_iter().filter(|a| a != "--fast").collect();
    let mut a = Strategy::load(args[1].as_ref()).unwrap();
    let mut b = Strategy::load(args[2].as_ref()).unwrap();
    if fast {
        for s in [&mut a, &mut b] {
            s.search_play = false;
            s.win_this_turn = false;
        }
    }
    let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let kingdom = match args.get(4) {
        Some(spec) => dominion_sim::kingdom::resolve(spec, &[], seed).unwrap(),
        None => vec![id::SENTRY, id::MILITIA, id::WITCH, id::MERCHANT, id::SMITHY, id::MARKET, id::CELLAR, id::MOAT, id::WORKSHOP, id::REMODEL],
    };
    let mut st = GameState::new(&GameConfig { num_players: 2, kingdom, seed, max_turns: 60, ..Default::default() });
    let (mut x, mut y) = (StrategyAgent::new(&a), StrategyAgent::new(&b));
    let mut agents: [&mut dyn Agent; 2] = [&mut x, &mut y];
    let mut ev: Vec<Event> = Vec::new();
    let r = play_game(&mut st, &mut agents, &mut ev);
    let mut line = String::new();
    for e in ev {
        match e {
            Event::TurnStart { player, turn } => { println!("{line}"); line = format!("T{turn} P{}:", player + 1); }
            Event::Play { card, .. } => line += &format!(" play {}", cards::name(card)),
            Event::Buy { card, .. } => line += &format!(" BUY {}", cards::name(card)),
            Event::Gain { card, .. } => line += &format!(" gain {}", cards::name(card)),
            Event::Trash { card, .. } => line += &format!(" trash {}", cards::name(card)),
            Event::TakeDebt { n, .. } => line += &format!(" +{n} Debt"),
            Event::PayDebt { n, .. } => line += &format!(" pays {n} Debt"),
            Event::VpTokens { n, source, .. } => line += &format!(" +{n} VP ({})", cards::name(source)),
            Event::Sun { left, .. } => line += &format!(" Sun ({left} left)"),
            _ => {}
        }
    }
    println!("{line}\nscores {:?} winners {:b}", r.scores, r.winners);
}
