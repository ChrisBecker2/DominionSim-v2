use dominion_engine::cards::id;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, DecisionKind, GameState, NoEvents, Step};
use dominion_search::*;

/// Parse a scenario for player 1 (seat 0) at the start of their action phase and advance to
/// the first decision.
fn scenario(kingdom: &str, hand: &str, deck_top: &str, deck: &str, discard: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: {kingdom}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\nseed: 1\n\n\
         [player 1]\nhand: {hand}\ndeck top: {deck_top}\ndeck: {deck}\ndiscard: {discard}\nin play:\n\n\
         [player 2]\nhand: 5 Copper\ndeck top:\ndeck: 2 Copper, 3 Estate\ndiscard:\nin play:\n"
    );
    let mut g = parse_state(&text).unwrap();
    g.chance_mode = true;
    match g.advance(&mut NoEvents) {
        Step::Decision(_) => g,
        s => panic!("expected a decision, got {s:?}"),
    }
}

const KINGDOM: &str = "Village, Smithy, Moneylender, Chapel, Cellar, Laboratory, Market, Militia, Throne Room, Library";

fn analyze_default(g: &GameState) -> Analysis {
    analyze(g, 0, &SearchConfig::default(), &NextHandEvaluator::default())
}

fn option<'a>(a: &'a Analysis, c: Choice) -> &'a RootOption {
    a.options.iter().find(|o| o.choice == c).unwrap_or_else(|| panic!("no option {c:?}"))
}

#[test]
fn village_before_smithy() {
    let g = scenario(KINGDOM, "Village, Smithy, 3 Copper", "", "4 Copper, 2 Silver, 3 Estate, Gold", "");
    let a = analyze_default(&g);
    for o in &a.options {
        eprintln!("{:>8.3} {:?}  {}", o.ev, o.choice, o.pv);
    }
    let village = option(&a, Choice::Card(id::VILLAGE));
    let smithy = option(&a, Choice::Card(id::SMITHY));
    assert_eq!(a.best().choice, Choice::Card(id::VILLAGE));
    assert!(village.ev > smithy.ev);
    assert!(village.exact && smithy.exact);
    // Smithy first consumes the only action: Village can never be played afterwards.
    assert!(!smithy.pv.contains("Play Village"), "{}", smithy.pv);
    assert!(village.pv.contains("Play Smithy"), "{}", village.pv);
}

#[test]
fn smithy_first_strands_village() {
    // After Smithy (0 actions left) the engine offers no action decision at all.
    let mut g = scenario(KINGDOM, "Village, Smithy, 3 Copper", "Estate, Estate, Estate", "5 Copper", "");
    g.apply(Choice::Card(id::SMITHY), &mut NoEvents).unwrap();
    match g.advance(&mut NoEvents) {
        Step::Decision(d) => assert_eq!(d.kind, DecisionKind::Buy),
        s => panic!("{s:?}"),
    }
    assert!(g.players[0].hand.has(id::VILLAGE));
}

#[test]
fn moneylender_trash_branch_unlocks_province() {
    // Mid-game (4 Provinces left): trashing the Copper makes $9 and a Province; keeping it
    // makes $7 and a Gold. The tree must contain both branches and prefer the Province.
    let mut g = scenario(KINGDOM, "Moneylender, Copper, 3 Silver", "", "5 Copper, 3 Estate", "");
    g.supply.set(id::PROVINCE, 4);
    g.apply(Choice::Card(id::MONEYLENDER), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert!(matches!(d.kind, DecisionKind::Select { .. }), "{d:?}");
    let a = analyze_default(&g);
    for o in &a.options {
        eprintln!("{:>8.3} {:?}  {}", o.ev, o.choice, o.pv);
    }
    let trash = option(&a, Choice::Card(id::COPPER));
    let keep = option(&a, Choice::Pass);
    assert!(trash.pv.contains("Buy Province"), "{}", trash.pv);
    assert!(keep.pv.contains("Buy Gold"), "{}", keep.pv);
    assert!(trash.ev > keep.ev);
}

#[test]
fn one_card_deck_is_certain() {
    // One unknown card left: the draw is still a chance node, but with a single outcome.
    let g = scenario(KINGDOM, "Laboratory, 4 Copper", "", "Gold", "3 Estate, 2 Copper");
    let a = analyze_default(&g);
    let lab = option(&a, Choice::Card(id::LABORATORY));
    assert!(lab.pv.contains("draw {Gold 100%}"), "{}", lab.pv);
}

/// Scores total money produced this turn: coins left plus the cost of everything bought
/// (supply decrease since the root), so a line's value doesn't depend on which buys are made.
struct TurnMoney;
impl Evaluator for TurnMoney {
    fn leaf_value(&self, root: &GameState, s: &GameState, _me: u8) -> f64 {
        let bought: u32 = (0..dominion_engine::cards::NUM_CARDS as u8)
            .map(|c| (root.supply.get(c) - s.supply.get(c)) as u32 * dominion_engine::cards::cost(c) as u32)
            .sum();
        s.turn.coins as f64 + bought as f64
    }
}

#[test]
fn smithy_ev_matches_hypergeometric_expectation() {
    // Hand: Smithy + 4 Copper. Deck: 3 Gold, 3 Copper, 4 Estate (unknown order).
    // E[money] = 4 + 3 * (3*3 + 3*1) / 10 = 7.6 exactly.
    let g = scenario("Smithy, Village, Cellar, Chapel, Moat, Market, Mine, Remodel, Witch, Library", "Smithy, 4 Copper", "", "3 Gold, 3 Copper, 4 Estate", "");
    let a = analyze(&g, 0, &SearchConfig::default(), &TurnMoney);
    let smithy = option(&a, Choice::Card(id::SMITHY));
    assert!(smithy.exact);
    assert!((smithy.ev - 7.6).abs() < 1e-9, "ev = {}", smithy.ev);

    // Cross-check with sampled playouts (engine RNG draws, no search).
    let mut total = 0.0;
    let n = 100_000;
    for seed in 0..n {
        let mut s = g;
        s.chance_mode = false;
        s.rng = dominion_engine::rng::Rng::new(seed);
        s.apply(Choice::Card(id::SMITHY), &mut NoEvents).unwrap();
        let _ = s.advance(&mut NoEvents); // runs draws, enters buy phase
        total += s.turn.coins as f64;
    }
    let mc = total / n as f64;
    assert!((mc - 7.6).abs() < 0.03, "monte carlo {mc}");
}

#[test]
fn chapel_selection_stays_small_and_trashes_junk() {
    let g = scenario(KINGDOM, "Chapel, Curse, Estate, Estate, Copper", "", "5 Copper, Silver", "");
    let a = analyze_default(&g);
    let chapel = option(&a, Choice::Card(id::CHAPEL));
    eprintln!("nodes {} tt_hits {} :: {}", a.nodes, a.tt_hits, chapel.pv);
    assert!(a.nodes < 5_000, "canonical picks keep this tiny, got {}", a.nodes);
    assert!(chapel.pv.contains("Trash Curse"), "{}", chapel.pv);
}

#[test]
fn search_agent_plays_full_games() {
    use dominion_engine::{play_game, Agent, GameConfig};
    struct BigMoney;
    impl Agent for BigMoney {
        fn name(&self) -> &str {
            "bm"
        }
        fn choose(&mut self, v: &dominion_engine::PlayerView, d: &dominion_engine::Decision, cs: &[Choice]) -> Choice {
            if d.kind == DecisionKind::Buy {
                let coins = v.turn().coins;
                for (c, need) in [(id::PROVINCE, 8), (id::GOLD, 6), (id::SILVER, 3)] {
                    if coins >= need && cs.contains(&Choice::Card(c)) {
                        return Choice::Card(c);
                    }
                }
                return Choice::Pass;
            }
            *cs.iter().find(|c| **c != Choice::Pass).unwrap_or(&cs[0])
        }
    }
    let cfg = SearchConfig { node_budget: 20_000, tt_bits: 16, ..Default::default() };
    let mut wins = 0.0;
    let games = 20;
    for seed in 0..games {
        let mut s = GameState::new(&GameConfig { seed, kingdom: vec![id::SMITHY, id::VILLAGE, id::MARKET, id::LABORATORY, id::CELLAR, id::MOAT, id::WORKSHOP, id::FESTIVAL, id::LIBRARY, id::MINE], ..Default::default() });
        let mut search = SearchAgent::new("search", cfg.clone(), NextHandEvaluator::default(), seed);
        let mut bm = BigMoney;
        let seat = (seed % 2) as usize;
        let r = if seat == 0 {
            play_game(&mut s, &mut [&mut search, &mut bm], &mut NoEvents)
        } else {
            play_game(&mut s, &mut [&mut bm, &mut search], &mut NoEvents)
        };
        if r.winners & (1 << seat) != 0 {
            wins += 1.0 / (r.winners.count_ones() as f64);
        }
        assert!(!r.capped);
    }
    eprintln!("search agent vs big money: {wins}/{games}");
}

#[test]
#[ignore]
fn bench_nodes_per_second() {
    let cases = [
        ("Village+Smithy+Lab", "Village, Smithy, Laboratory, 2 Copper", "6 Copper, 3 Estate, 2 Silver, Gold, Village, Smithy"),
        ("Library", "Library, Village, 2 Copper, Estate", "6 Copper, 3 Estate, 2 Silver, Smithy, Village"),
        ("Throne+Smithy", "Throne Room, Smithy, Village, 2 Copper", "6 Copper, 3 Estate, 2 Silver, Gold"),
        ("Cellar", "Cellar, Market, Estate, Estate, Copper", "6 Copper, 3 Estate, 2 Silver, Gold, Laboratory"),
    ];
    for (name, hand, deck) in cases {
        let g = scenario(KINGDOM, hand, "", deck, "");
        let cfg = SearchConfig { node_budget: u64::MAX, tt_bits: 22, ..Default::default() };
        let mut s = Searcher::new(cfg.tt_bits);
        let a = s.analyze(&g, 0, &cfg, &NextHandEvaluator::default());
        let secs = a.elapsed.as_secs_f64();
        eprintln!(
            "{name:<20} nodes {:>9}  tt_hits {:>8}  {:>7.1} ms  {:>6.2} M nodes/s  best: {}",
            a.nodes, a.tt_hits, secs * 1e3, a.nodes as f64 / secs / 1e6, a.best().pv
        );
    }
}

#[test]
fn parallel_matches_serial_exactly() {
    let cases = [
        "Village, Smithy, Laboratory, 2 Copper",
        "Throne Room, Smithy, Village, 2 Copper",
        "Cellar, Market, Estate, Estate, Copper",
        "Moneylender, Copper, 3 Silver",
    ];
    let cfg = SearchConfig { node_budget: u64::MAX, tt_bits: 20, ..Default::default() };
    for hand in cases {
        let g = scenario(KINGDOM, hand, "", "6 Copper, 3 Estate, 2 Silver, Gold, Laboratory", "");
        let serial = Searcher::new(cfg.tt_bits).analyze(&g, 0, &cfg, &NextHandEvaluator::default());
        for threads in [1, 3, 8] {
            let par = analyze_parallel(&g, 0, &cfg, &NextHandEvaluator::default(), threads);
            assert_eq!(par.options.len(), serial.options.len());
            for o in &serial.options {
                let p = par.options.iter().find(|x| x.choice == o.choice).unwrap();
                assert!((p.ev - o.ev).abs() < 1e-9, "{hand} / {:?}: serial {} vs parallel({threads}) {}", o.choice, o.ev, p.ev);
                assert!(p.exact);
            }
            assert_eq!(par.best().choice, serial.best().choice);
        }
    }
}
