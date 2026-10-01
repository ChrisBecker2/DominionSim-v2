//! The play-order shortcuts must never be worse than a full search of the turn: the obvious play
//! (every action in hand choice-free and all of them playable) and cantrips first (choice-free
//! +Actions cards before anything else unless a card in hand is order-sensitive).

use dominion_engine::cards::{self, id, CardId, OnPlay};
use dominion_engine::rng::Rng;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_search::{Evaluator, SearchConfig};
use dominion_sim::strategy::{cantrip_first, obvious_play};
use dominion_sim::{GainListEvaluator, Strategy};

/// The strategy's scoring with every play decision searched (no shortcut), as the reference.
struct FullSearch<'a>(GainListEvaluator<'a>);

/// The strategy's scoring with every decision of the turn player's own searched: what the
/// best order is worth when the choices after it are made well too (cantrips first only promises
/// that: a card picked by a rule, like Mine's trash, can come out worse with more cards in hand).
struct SearchEverything<'a>(GainListEvaluator<'a>);

impl Evaluator for SearchEverything<'_> {
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
        if decision.player == me && state.turn.player == me {
            return None;
        }
        self.0.policy(state, me, decision, choices)
    }
    fn playout_choice(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        self.0.playout_choice(state, me, decision, choices)
    }
}

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
    assert!(free.contains(&id::VILLAGE) && free.contains(&id::BRIDGE), "every playable choice-free card, from every set: {free:?}");
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

// ---- Cantrips first: with a real choice in hand, the choice-free +Actions cards are played
// first without searching, unless a card in hand is order-sensitive. ----

fn cantrips() -> Vec<CardId> {
    free_actions().into_iter().filter(|&c| cards::def(c).actions > 0).collect()
}

/// Kingdom Action cards with a choice that a cantrip played first can't hurt.
fn plain_choice_actions() -> Vec<CardId> {
    cards::kingdom_cards()
        .filter(|&c| cards::is(c, cards::ACTION) && cards::def(c).on_play == OnPlay::Choice)
        .collect()
}

#[test]
fn cantrip_first_is_as_good_as_searching() {
    let cantrip = cantrips();
    let choice = plain_choice_actions();
    assert!(cantrip.contains(&id::VILLAGE) && choice.contains(&id::WORKSHOP) && !choice.contains(&id::THRONE_ROOM) && !choice.contains(&id::REMODEL));
    let fillers = [id::COPPER, id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::CURSE];
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let cfg = SearchConfig::default();
    let mut rng = Rng::new(777);
    let (mut checked, mut cases) = (0, 0);
    for _ in 0..1500 {
        let pick = |rng: &mut Rng, xs: &[CardId]| xs[rng.below(xs.len() as u32) as usize];
        let mut hand: Vec<CardId> = (0..1 + rng.below(2)).map(|_| pick(&mut rng, &cantrip)).collect();
        let c = pick(&mut rng, &choice);
        hand.push(c);
        while hand.len() < 5 {
            hand.push(pick(&mut rng, &fillers));
        }
        let mut pool = fillers.to_vec();
        pool.extend_from_slice(&cantrip);
        let deck: Vec<CardId> = (0..6 + rng.below(10)).map(|_| pick(&mut rng, &pool)).collect();
        let mut kingdom: Vec<CardId> = hand.iter().chain(deck.iter()).copied().filter(|&c| cards::is_kingdom(c)).collect();
        kingdom.sort();
        kingdom.dedup();
        let text = format!(
            "players: 2\nkingdom: {}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
             [player 1]\nhand: {}\ndeck: {}\ndiscard: 3 Copper, Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n",
            names(&kingdom),
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
        if obvious_play(&view, buf.as_slice()).is_some() {
            continue;
        }
        let Some(first) = cantrip_first(&view, buf.as_slice()) else { continue };
        cases += 1;
        assert_eq!(strat.decide(&view, &d, buf.as_slice()), Choice::Card(first), "bot uses the shortcut\n{text}");
        let a = dominion_search::analyze(&g, 0, &cfg, &SearchEverything(GainListEvaluator::new(&strat)));
        let best = a.best();
        let mine = a.options.iter().find(|o| o.choice == Choice::Card(first)).expect("cantrip is an option");
        if !(best.exact && mine.exact) {
            continue;
        }
        checked += 1;
        let tol = 1e-6 * best.ev.abs().max(1.0);
        assert!(
            mine.ev >= best.ev - tol,
            "{} first ({}) is worse than {:?} ({})\n{text}",
            cards::name(first),
            mine.ev,
            best.choice,
            best.ev
        );
    }
    println!("cantrip first: {cases} positions, {checked} checked exactly");
    assert!(checked >= 100, "too few exact positions checked: {checked} of {cases}");
}

#[test]
fn cantrips_wait_for_order_sensitive_cards_reshuffles_and_stated_play_rules() {
    let state = |hand: &str, deck: &str| {
        let text = format!(
            "players: 2\nkingdom: Village, Laboratory, Remodel, Throne Room, Library, Harbinger, City, Witch, Workshop, Market\n\
             turn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
             [player 1]\nhand: {hand}\ndeck: {deck}\ndiscard: Silver\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
        );
        let mut g = parse_state(&text).unwrap();
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        (g, d)
    };
    let first = |hand: &str, deck: &str| {
        let (g, _) = state(hand, deck);
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        cantrip_first(&PlayerView::new(&g, 0), buf.as_slice())
    };
    assert_eq!(first("Workshop, Village, Laboratory, Copper, Estate", "9 Copper"), Some(id::VILLAGE), "most +Actions first");
    assert_eq!(first("Workshop, Laboratory, Copper, Estate, Estate", "9 Copper"), Some(id::LABORATORY));
    assert_eq!(first("Throne Room, Village, Workshop, Copper, Estate", "5 Copper"), None, "Throne Room may want the Village");
    assert_eq!(first("Remodel, Market, Copper, Estate, Estate", "5 Copper"), None, "Remodel may want the Market (into a Gold)");
    assert_eq!(first("Library, Laboratory, Copper, Estate, Estate", "5 Copper"), None, "Library draws to 7: draw after it");
    assert_eq!(first("Harbinger, Laboratory, Copper, Estate, Estate", "5 Copper"), None, "Harbinger topdecks for the Lab");
    assert_eq!(first("City, Witch, Workshop, Copper, Estate", "9 Copper"), None, "Witch may empty the Curses for City");
    assert_eq!(first("City, Workshop, Copper, Estate, Estate", "9 Copper"), Some(id::CITY), "no pile is low: City is a plain cantrip");
    assert_eq!(first("Workshop, Laboratory, Copper, Estate, Estate", "4 Copper"), None, "Lab + Workshop could draw 5: a reshuffle");
    assert_eq!(first("Workshop, Laboratory, Copper, Estate, Estate", "4 Copper, Laboratory"), None, "the deck's Lab draws too");
    assert_eq!(first("Workshop, Laboratory, Copper, Estate, Estate", "6 Copper"), Some(id::LABORATORY));
    assert_eq!(first("Workshop, Laboratory, Caravan, Copper, Estate", "9 Copper"), Some(id::CARAVAN), "draw 1 before 2: see more before choosing");
    // Stated [[play]] rules for the hand's choice card: the rules decide, not the shortcut.
    let ruled = Strategy::parse("name = \"T\"\nsearch_play = true\n[[gain]]\ncard = \"Province\"\n[[play]]\ncard = \"Workshop\"\n").unwrap();
    let (g, d) = state("Workshop, Laboratory, Copper, Estate, Estate", "5 Copper");
    let view = PlayerView::new(&g, 0);
    assert!(ruled.states_play_for_hand(&view));
    let plain = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Province\"\n").unwrap();
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!(plain.decide(&view, &d, buf.as_slice()), Choice::Card(id::LABORATORY));
}
