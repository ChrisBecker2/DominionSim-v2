use dominion_engine::cards::{self, id};
use dominion_engine::{play_game, Agent, Event, GameConfig, GameState};
use dominion_sim::{Strategy, StrategyAgent};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = Strategy::load(args[1].as_ref()).unwrap();
    let b = Strategy::load(args[2].as_ref()).unwrap();
    let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let kingdom = vec![id::SENTRY, id::MILITIA, id::WITCH, id::MERCHANT, id::SMITHY, id::MARKET, id::CELLAR, id::MOAT, id::WORKSHOP, id::REMODEL];
    let mut st = GameState::new(&GameConfig { num_players: 2, kingdom, seed, max_turns: 60 });
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
            _ => {}
        }
    }
    println!("{line}\nscores {:?} winners {:b}", r.scores, r.winners);
}
