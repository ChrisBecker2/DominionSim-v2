//! The Lab evolves `[[mode]]` rules: they are only offered for choose-one cards in the pool,
//! survive the TOML round trip, and the parsed strategy decides as the genome says.
use dominion_engine::cards::{self, id, CardId};
use dominion_engine::rng::Rng;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, DecisionKind, NoEvents, PlayerView, Step};
use dominion_evolve::genome::{Genome, Rule, TomlMeta};
use dominion_evolve::ops;
use dominion_evolve::space::SearchSpace;
use dominion_evolve::{run, Control, EvolveConfig, OpponentSpec, Track};

fn genome_with_mode(opt: u8) -> Genome {
    let mut g = ops::big_money_with(&[(id::INVESTMENT, 2)]);
    g.mode.push(Rule { card: id::INVESTMENT, opt, conds: vec![] });
    g
}

#[test]
fn mode_rule_round_trips_and_decides_the_same() {
    let text = "players: 2\nkingdom: Investment\nturn: 9  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n\
                [player 1]\nhand: Investment, Estate, Copper\ndeck: 5 Copper\ndiscard: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    for opt in [0u8, 1] {
        let g = genome_with_mode(opt);
        let toml = g.to_toml(&TomlMeta::default());
        assert!(toml.contains("[[mode]]"), "{toml}");
        assert_eq!(Genome::from_toml(&toml).unwrap(), g, "{toml}");
        let strat = dominion_sim::Strategy::parse(&toml).unwrap();
        let mut state = parse_state(text).unwrap();
        loop {
            let d = match state.advance(&mut NoEvents) {
                Step::Decision(d) => d,
                s => panic!("{s:?}"),
            };
            let mut buf = ChoiceBuf::default();
            state.legal_choices(&mut buf);
            let c = strat.decide(&PlayerView::new(&state, d.player), &d, buf.as_slice());
            if matches!(d.kind, DecisionKind::Mode { .. }) {
                assert_eq!(c, Choice::Mode(opt), "{toml}");
                break;
            }
            state.apply(c, &mut NoEvents).unwrap();
        }
    }
}

#[test]
fn search_space_offers_mode_rules_only_for_modal_cards_in_the_pool() {
    let pool: Vec<CardId> = vec![id::INVESTMENT, id::SMITHY, id::VILLAGE];
    let space = SearchSpace::new(&pool, &[]);
    assert_eq!(space.modal, vec![id::INVESTMENT]);
    let mut rng = Rng::new(3);
    let mut pop = ops::templates(&space);
    for _ in 0..3000 {
        let mut c = pop[rng.below(pop.len() as u32) as usize].clone();
        ops::mutate(&mut c, &space, &mut rng);
        for r in &c.mode {
            assert_eq!(r.card, id::INVESTMENT);
            assert!((r.opt as usize) < cards::modes(r.card).len());
        }
        assert!(c.mode.len() <= space.max_mode_rules);
        let toml = c.to_toml(&TomlMeta::default());
        dominion_sim::Strategy::parse(&toml).unwrap_or_else(|e| panic!("{e}\n{toml}"));
        assert_eq!(Genome::from_toml(&toml).unwrap(), c, "{toml}");
        pop.push(c);
    }
    assert!(pop.iter().any(|g| !g.mode.is_empty()), "mutation never produced a mode rule");
    // A pool without mode cards offers none.
    let plain = SearchSpace::new(&[id::SMITHY], &[]);
    assert!(plain.modal.is_empty());
}

#[test]
fn ga_smoke_with_investment_produces_a_valid_strategy() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies");
    let bm = std::fs::read_to_string(format!("{root}/big_money_ultimate.toml")).unwrap();
    let cfg = EvolveConfig {
        track: Track::Fixed { kingdom: vec!["Investment".into(), "Steward".into(), "Smithy".into()] },
        sets: vec!["Base".into(), "Prosperity".into()],
        opponents: vec![OpponentSpec { name: "BMU".into(), toml: bm, weight: 1.0 }],
        forbidden: vec![],
        islands: 2,
        island_size: 8,
        generations: 3,
        race_games: vec![100, 200],
        validate_games: 400,
        polish_games: 200,
        ..EvolveConfig::default()
    };
    let p = run(&cfg, &Control::default(), &mut |_| {}).unwrap();
    let r = p.result.expect("a result");
    dominion_sim::Strategy::parse(&r.toml).unwrap();
    let g = Genome::from_toml(&r.toml).unwrap();
    assert!(g.mode.iter().all(|m| [id::INVESTMENT, id::STEWARD].contains(&m.card)), "{}", r.toml);
}
