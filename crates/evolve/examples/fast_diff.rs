//! Where does the full bot (turn search) decide differently from the fast bot on "BM + X" (the search_gap mirror)?
//! Plays fast-vs-fast games, and at every non-Buy decision of seat 0 with a real choice asks the full bot too. Prints the
//! disagreements grouped by (kind, deciding card, fast choice, full choice) and a few samples.
//!   cargo run --release -p dominion-evolve --example fast_diff -- <card> [games] [samples]
use dominion_engine::cards::{self, CardId};
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::{play_game, Agent, GameConfig, GameState, NoEvents, PlayerView};
use dominion_evolve::arena::Arena;
use dominion_evolve::baselines::baselines;
use dominion_sim::Strategy;
use std::collections::BTreeMap;

const BASE: [&str; 9] = ["Village", "Smithy", "Market", "Cellar", "Militia", "Moat", "Workshop", "Mine", "Laboratory"];

fn cn(c: CardId) -> String {
    cards::name(c).to_string()
}
fn ch(c: Choice) -> String {
    match c {
        Choice::Card(c) => cn(c),
        Choice::Mode(i) => format!("mode{i}"),
        o => format!("{o:?}"),
    }
}
fn counts(c: &dominion_engine::Counts) -> String {
    c.iter().map(|(c, n)| if n > 1 { format!("{}x{}", n, cn(c)) } else { cn(c) }).collect::<Vec<_>>().join(",")
}

struct Probe<'a> {
    full: &'a Strategy,
    fast: &'a Strategy,
    seat0: bool,
    groups: BTreeMap<String, u64>,
    samples: BTreeMap<String, Vec<String>>,
    n_samples: usize,
    total: u64,
    differ: u64,
}

impl Agent for Probe<'_> {
    fn name(&self) -> &str {
        "probe"
    }
    fn choose(&mut self, view: &PlayerView, d: &Decision, choices: &[Choice]) -> Choice {
        let fast = self.fast.decide(view, d, choices);
        if !self.seat0 || d.player != view.me() || matches!(d.kind, DecisionKind::Buy) || choices.len() < 2 {
            return fast;
        }
        self.total += 1;
        let full = self.full.decide(view, d, choices);
        if full != fast {
            self.differ += 1;
            let kind = match d.kind {
                DecisionKind::PlayAction => "play".to_string(),
                DecisionKind::Select { act, .. } => format!("select-{act:?}"),
                DecisionKind::Mode { .. } => "mode".into(),
                DecisionKind::PlayTreasure => "treasure".into(),
                k => format!("{k:?}").chars().take(20).collect(),
            };
            let key = format!("{kind} src={} fast={} full={}", d.source.map(cn).unwrap_or("-".into()), ch(fast), ch(full));
            *self.groups.entry(key.clone()).or_default() += 1;
            let v = self.samples.entry(key).or_default();
            if v.len() < self.n_samples {
                let opts: Vec<String> = choices.iter().map(|c| ch(*c)).collect();
                // DETAIL=1: the turn search's value of each option, with its best line.
                if std::env::var("DETAIL").is_ok() {
                    let world = view.determinize(&mut dominion_engine::rng::Rng::new(view.stable_seed()));
                    let eval = dominion_sim::GainListEvaluator::new(self.full);
                    let cfg = dominion_search::SearchConfig { node_budget: 200_000, tt_bits: 16, ..Default::default() };
                    let an = dominion_search::analyze(&world, view.me(), &cfg, &eval);
                    for o in an.options.iter().take(4) {
                        v.push(format!("      option {} ev {:.3} pv {}", ch(o.choice), o.ev, o.pv));
                    }
                }
                v.push(format!(
                    "    turn {} hand [{}] inplay [{}] deck {} | options [{}] | owned [{}]",
                    view.turn().number,
                    counts(view.hand()),
                    counts(view.in_play()),
                    view.deck_size(),
                    opts.join(","),
                    counts(&view.my_cards())
                ));
            }
        }
        fast
    }
}

fn main() {
    std::thread::Builder::new().stack_size(1 << 30).spawn(real_main).unwrap().join().unwrap();
}

fn real_main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let name = a.first().expect("card");
    let games: u64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(200);
    let n_samples: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
    let x = cards::by_name(name).expect("card name");
    let mut kingdom: Vec<CardId> = vec![x];
    for n in BASE {
        let c = cards::by_name(n).unwrap();
        if c != x && kingdom.len() < 10 {
            kingdom.push(c);
        }
    }
    let bl = baselines(&kingdom, false, &[]);
    let b = bl.iter().skip(1).find(|b| b.name.contains(name.as_str())).expect("baseline");
    let arena = |fast| Arena { opponents: vec![], forbidden: vec![], fast, search_cards: None, max_turns: 60 };
    let fast = arena(true).compile(&b.genome).unwrap();
    let full = arena(false).compile(&b.genome).unwrap();
    let mut p = Probe { full: &full, fast: &fast, seat0: true, groups: BTreeMap::new(), samples: BTreeMap::new(), n_samples, total: 0, differ: 0 };
    let mut opp = Probe { full: &full, fast: &fast, seat0: false, groups: BTreeMap::new(), samples: BTreeMap::new(), n_samples, total: 0, differ: 0 };
    for g in 0..games {
        let mut st = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed: 5000 + g, max_turns: 60, ..Default::default() });
        let mut agents: [&mut dyn Agent; 2] = [&mut p, &mut opp];
        play_game(&mut st, &mut agents, &mut NoEvents);
    }
    println!("{} decisions with a choice, {} differ", p.total, p.differ);
    let mut g: Vec<_> = p.groups.iter().collect();
    g.sort_by(|x, y| y.1.cmp(x.1));
    for (k, n) in g.iter().take(25) {
        println!("{n:>6}  {k}");
        for s in &p.samples[*k] {
            println!("{s}");
        }
    }
}
