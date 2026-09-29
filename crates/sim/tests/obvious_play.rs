//! The obvious-play shortcut (no search when every action in hand is choice-free and all of them
//! can be played) must never be worse than a full search of the turn.

use dominion_engine::cards::{self, id, CardId, OnPlay};
use dominion_engine::rng::Rng;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_search::{Evaluator, SearchConfig};
use dominion_sim::strategy::obvious_play;
use dominion_sim::{GainListEvaluator, Strategy};

/// The strategy's scoring with every play decision searched (no shortcut), as the reference.
struct FullSearch<'a>(GainListEvaluator<'a>);

impl Evaluator for FullSearch<'_> {
    fn leaf_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        self.0.leaf_value(root, leaf, me)
    }
    fn allows(&self, state: &GameState, me: u8, decision: &Decision, choice: Choice) -> bool {
        self.0.allows(state, me, decision, choice)
    }
    fn value_when_nothing_allowed(&self) -> Option<f64> {
        self.0.value_when_nothing_allowed()
    }
    fn policy(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        if matches!(decision.kind, DecisionKind::PlayAction) {
            return None;
        }
        self.0.policy(state, me, decision, choices)
    }
    fn playout_choice(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        self.0.playout_choice(state, me, decision, choices)
    }
}

fn free_actions() -> Vec<CardId> {
    cards::kingdom_cards().filter(|&c| cards::def(c).on_play == OnPlay::ChoiceFree).collect()
}

fn names(cs: &[CardId]) -> String {
    cs.iter().map(|&c| cards::name(c)).collect::<Vec<_>>().join(", ")
}

#[test]
fn obvious_play_is_as_good_as_searching() {
    let free = free_actions();
    assert_eq!(free.len(), 10, "the kingdom below holds every choice-free card");
    let fillers = [id::COPPER, id::COPPER, id::SILVER, id::GOLD, id::ESTATE];
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies/");
    let strategies: Vec<Strategy> = ["village_smithy_engine.toml", "double_witch.toml", "laboratory_bm.toml"]
        .iter()
        .map(|f| Strategy::parse(&std::fs::read_to_string(format!("{root}{f}")).unwrap()).unwrap())
        .collect();
    let cfg = SearchConfig::default();
    let mut rng = Rng::new(12345);
    let (mut checked, mut differs) = (0, 0);
    for case in 0..400 {
        let pick = |rng: &mut Rng, xs: &[CardId]| xs[rng.below(xs.len() as u32) as usize];
        let mut hand: Vec<CardId> = (0..1 + rng.below(4)).map(|_| pick(&mut rng, &free)).collect();
        while hand.len() < 5 + rng.below(2) as usize {
            hand.push(pick(&mut rng, &fillers));
        }
        let mut pool = free.clone();
        pool.extend_from_slice(&fillers);
        let deck: Vec<CardId> = (0..6 + rng.below(6)).map(|_| pick(&mut rng, &pool)).collect();
        let actions = 1 + rng.below(2);
        let text = format!(
            "players: 2\nkingdom: {}\nturn: 5  player: 1  phase: action  actions: {actions}  buys: 1  coins: 0\n\n\
             [player 1]\nhand: {}\ndeck: {}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n",
            names(&free),
            names(&hand),
            names(&deck)
        );
        let mut g = parse_state(&text).unwrap();
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) if matches!(d.kind, DecisionKind::PlayAction) => d,
            _ => continue,
        };
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        let view = PlayerView::new(&g, 0);
        let Some(obvious) = obvious_play(&view, buf.as_slice()) else { continue };
        let strat = &strategies[case % strategies.len()];
        // The bot (search_play on) now plays the obvious card without searching.
        assert_eq!(strat.decide(&view, &d, buf.as_slice()), Choice::Card(obvious), "bot uses the shortcut\n{text}");
        let a = dominion_search::analyze(&g, 0, &cfg, &FullSearch(GainListEvaluator::new(strat)));
        let best = a.best();
        let mine = a.options.iter().find(|o| o.choice == Choice::Card(obvious)).expect("obvious play is an option");
        if !(best.exact && mine.exact) {
            continue;
        }
        checked += 1;
        if best.choice != mine.choice {
            differs += 1;
        }
        let tol = 1e-6 * best.ev.abs().max(1.0);
        assert!(
            mine.ev >= best.ev - tol,
            "obvious play {} ({}) is worse than {:?} ({}) for {}\n{text}",
            cards::name(obvious),
            mine.ev,
            best.choice,
            best.ev,
            strat.name
        );
    }
    println!("checked {checked} positions exactly ({differs} where search listed a different, equally good card first)");
    assert!(checked >= 100, "too few exact positions checked: {checked}");
}

#[test]
fn a_card_with_a_choice_or_too_many_terminals_means_search() {
    let text = |hand: &str, actions: u8| {
        format!(
            "players: 2\nkingdom: Village, Smithy, Remodel, Militia, Laboratory, Market, Festival, Witch, Moat, Merchant\n\
             turn: 5  player: 1  phase: action  actions: {actions}  buys: 1  coins: 0\n\n\
             [player 1]\nhand: {hand}\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
        )
    };
    let obvious = |hand: &str, actions: u8| {
        let mut g = parse_state(&text(hand, actions)).unwrap();
        assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        obvious_play(&PlayerView::new(&g, 0), buf.as_slice())
    };
    // Village, Smithy: Village first.
    assert_eq!(obvious("Village Smithy Copper Copper Estate", 1), Some(id::VILLAGE));
    // Two terminals, one Village: both playable, Village first.
    assert_eq!(obvious("Village Smithy Militia Copper Copper", 1), Some(id::VILLAGE));
    // Two terminals and one action: which one is a real choice.
    assert_eq!(obvious("Smithy Militia Copper Copper Copper", 1), None);
    // ...but with two actions both get played, drawing first.
    assert_eq!(obvious("Smithy Militia Copper Copper Copper", 2), Some(id::SMITHY));
    // Remodel involves choices: search.
    assert_eq!(obvious("Remodel Village Copper Copper Estate", 1), None);
    // Labs and Markets: +Actions first.
    assert_eq!(obvious("Market Laboratory Witch Copper Copper", 1), Some(id::LABORATORY));
}
