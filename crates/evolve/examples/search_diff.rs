//! Where does the full bot choose differently from its own rules? Plays "BM + X" (full) against the fast bot on the search_gap
//! kingdom and lists the decisions where the search overrides the rules.
//!   cargo run --release -p dominion-evolve --example search_diff -- <card> [games] [details]
use dominion_engine::cards::{self, CardId};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::{play_game, Agent, GameConfig, GameState, NoEvents, PlayerView};
use dominion_evolve::arena::Arena;
use dominion_evolve::baselines::baselines;
use dominion_sim::Strategy;
use std::collections::BTreeMap;

const BASE: [&str; 9] = ["Village", "Smithy", "Market", "Cellar", "Militia", "Moat", "Workshop", "Mine", "Laboratory"];

struct Probe<'a> {
    s: &'a Strategy,
    f: &'a Strategy,
    pairs: BTreeMap<String, u64>,
    shown: u32,
    max_show: u32,
    total: u64,
}

fn cname(c: Choice) -> String {
    match c {
        Choice::Card(c) => cards::name(c).to_string(),
        o => format!("{o:?}"),
    }
}

fn counts(c: &dominion_engine::Counts) -> String {
    c.iter().map(|(c, n)| format!("{}x{}", n, cards::name(c))).collect::<Vec<_>>().join(" ")
}

impl Agent for Probe<'_> {
    fn name(&self) -> &str {
        &self.s.name
    }
    fn choose(&mut self, view: &PlayerView, d: &Decision, choices: &[Choice]) -> Choice {
        let full = self.s.decide(view, d, choices);
        if choices.len() > 1 {
            self.total += 1;
            let rule = self.f.decide(view, d, choices);
            if rule != full {
                let key = format!("{:?} src={:?} rules={} search={}", d.kind, d.source.map(cards::name), cname(rule), cname(full));
                *self.pairs.entry(key).or_default() += 1;
                if self.shown < self.max_show {
                    self.shown += 1;
                    println!(
                        "--- turn {} {:?} src={:?}\n  hand: {}\n  play: {}\n  deck: {}\n  discard: {}\n  tavern: {} vp={}\n  choices: {}\n  rules: {}  search: {}",
                        view.turn().number,
                        d.kind,
                        d.source.map(cards::name),
                        counts(view.hand()),
                        counts(view.in_play()),
                        counts(&view.deck()),
                        counts(view.discard()),
                        counts(&view.tavern_of(view.me())),
                        view.my_vp(),
                        choices.iter().map(|c| cname(*c)).collect::<Vec<_>>().join(", "),
                        cname(rule),
                        cname(full)
                    );
                    if std::env::var("ANALYZE").is_ok() {
                        for (c, ev, pv) in dominion_sim::eval::analyze_play(self.s, view) {
                            println!("    {ev:>14.3}  {} :: {pv}", cname(c));
                        }
                    }
                }
            }
        }
        full
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let x: CardId = cards::by_name(&a[1]).expect("card");
    let games: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(200);
    let show: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
    let mut kingdom: Vec<CardId> = vec![x];
    for n in BASE {
        let c = cards::by_name(n).unwrap();
        if c != x && kingdom.len() < 10 {
            kingdom.push(c);
        }
    }
    let arena = |fast| Arena { opponents: vec![], forbidden: vec![], fast, search_cards: None, max_turns: 60 };
    let bl = baselines(&kingdom, false, &[]);
    let b = bl.iter().skip(1).find(|b| b.name.contains(cards::name(x))).unwrap();
    let fast = arena(true).compile(&b.genome).unwrap();
    let full = arena(false).compile(&b.genome).unwrap();
    println!("{}", b.name);
    if std::env::var("DUMP").is_ok() {
        println!("{}", b.genome.to_toml(&dominion_evolve::genome::TomlMeta { name: "Candidate".into(), ..Default::default() }));
    }
    let mut p = Probe { s: &full, f: &fast, pairs: BTreeMap::new(), shown: 0, max_show: show, total: 0 };
    let mut opp = dominion_sim::StrategyAgent::new(&fast);
    for g in 0..games {
        let mut st = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed: 1000 + g, max_turns: 60, ..Default::default() });
        let mut agents: [&mut dyn Agent; 2] = if g % 2 == 0 { [&mut p, &mut opp] } else { [&mut opp, &mut p] };
        play_game(&mut st, &mut agents, &mut NoEvents);
    }
    println!("{} multi-choice decisions", p.total);
    let mut v: Vec<_> = p.pairs.iter().collect();
    v.sort_by(|x, y| y.1.cmp(x.1));
    for (k, n) in v.iter().take(25) {
        println!("{n:6}  {k}");
    }
}
