//! The Buy decision answered from a test on a card (`Agent::choose_buy`, `GameState::buy_avail`) plays exactly the games the list does: same
//! final position, scores and turns, for every shipped strategy on random kingdoms (Events, Landmarks and Projects among them), with search on and off, 2 to 4 players.

use dominion_engine::agent::{Agent, PlayerView};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::MAX_PLAYERS;
use dominion_engine::{format_state, play_game, GameConfig, GameState, NoEvents};
use dominion_sim::agent::StrategyAgent;
use dominion_sim::Strategy;

/// A strategy that answers every decision from the list (no `choose_buy`).
struct ListOnly<'a>(StrategyAgent<'a>);

impl Agent for ListOnly<'_> {
    fn name(&self) -> &str {
        "list"
    }
    fn choose(&mut self, view: &PlayerView, d: &Decision, choices: &[Choice]) -> Choice {
        self.0.choose(view, d, choices)
    }
}

fn strategies() -> Vec<Strategy> {
    let dir = format!("{}/../../strategies", env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "toml") {
            out.push(Strategy::load(&p).unwrap());
        }
    }
    out
}

fn play(strats: &[&Strategy], kingdom: &[u16], seed: u64, fast_buy: bool) -> (String, Vec<i32>, Vec<u16>, u8) {
    let n = strats.len();
    let mut state = GameState::new(&GameConfig { num_players: n, kingdom: kingdom.to_vec(), seed, max_turns: 100, ..Default::default() });
    let mut a: Vec<StrategyAgent> = strats.iter().map(|s| StrategyAgent::new(s)).collect();
    let mut l: Vec<ListOnly> = strats.iter().map(|s| ListOnly(StrategyAgent::new(s))).collect();
    let mut refs: Vec<&mut dyn Agent> = if fast_buy { a.iter_mut().map(|x| x as &mut dyn Agent).collect() } else { l.iter_mut().map(|x| x as &mut dyn Agent).collect() };
    let r = play_game(&mut state, &mut refs, &mut NoEvents);
    (format_state(&state), r.scores[..n].to_vec(), r.turns[..n].to_vec(), r.winners)
}

#[test]
fn buy_from_a_test_on_a_card_plays_the_same_games_as_the_list() {
    // The searching bots recurse over copies of the state: a worker thread with a big stack, as the batch runner has.
    std::thread::Builder::new().stack_size(64 << 20).spawn(run).unwrap().join().unwrap();
}

fn run() {
    let all = strategies();
    assert!(all.len() > 20);
    let sets = ["base", "adventures", "empires", "rising sun", "base+intrigue+seaside+prosperity+alchemy+promo+cornucopia+dark ages+adventures+empires+rising sun+nocturne"];
    let mut games = 0;
    for (i, s) in all.iter().enumerate() {
        // Opponents: the next strategies in the list (2 to 4 players).
        let players = 2 + i % 3;
        let strats: Vec<&Strategy> = (0..players).map(|k| &all[(i + k * 7) % all.len()]).collect();
        assert!(players <= MAX_PLAYERS);
        for (j, set) in sets.iter().enumerate() {
            let spec = format!("auto:{set}");
            let kingdom = dominion_sim::kingdom::resolve(&spec, &strats, (i * 31 + j) as u64).unwrap();
            let seed = 1000 + (i * 13 + j) as u64;
            let list = play(&strats, &kingdom, seed, false);
            let fast = play(&strats, &kingdom, seed, true);
            assert_eq!(list, fast, "{} (+{} others) on {kingdom:?} seed {seed}", s.name, players - 1);
            games += 1;
        }
    }
    println!("{games} games identical");
}
