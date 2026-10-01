//! Bot default policies for Alchemy's decisions: Transmute and Apprentice trash by the trash
//! rules, Scrying Pool keeps/discards by whose deck it is, Herbalist puts back the best Treasure,
//! Alchemist always goes back, University gains by the gain list. Plus a few whole games.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

fn next_decision(g: &mut GameState) -> (Decision, Vec<Choice>) {
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    (d, buf.as_slice().to_vec())
}

fn decide(strat: &Strategy, g: &GameState, d: &Decision, choices: &[Choice], me: u8) -> Choice {
    strat.decide(&PlayerView::new(g, me), d, choices)
}

const KINGDOM: &str = "Transmute, Apprentice, Scrying Pool, Herbalist, Alchemist, University, Golem, Familiar, Village, Smithy";

/// A state at the very start of player 1's Action phase with the given player-1 lines.
fn state(p1: &str, p2: &str) -> String {
    format!("players: 2\nkingdom: {KINGDOM}\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\n{p1}\n\n[player 2]\n{p2}\n")
}

/// Play `card` from player 1's hand and return the state at the next decision.
fn play(strat_text: &str, card: u8) -> (GameState, Decision, Vec<Choice>) {
    let mut g = parse_state(strat_text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(card), &mut NoEvents).unwrap();
    let (d, c) = next_decision(&mut g);
    (g, d, c)
}

// ---------------------------------------------------------------------------------------
// Transmute and Apprentice: the trash rules (Curse, Estate, Copper by default), then the
// cheapest card when a trash is forced and nothing is wanted.
// ---------------------------------------------------------------------------------------

#[test]
fn transmute_trashes_curse_then_estate_then_copper() {
    let strat = load("big_money_ultimate.toml");
    for (hand, expect) in [
        ("Transmute, Curse, Estate, Copper, Gold", id::CURSE),
        ("Transmute, Estate, Copper, Gold", id::ESTATE),
        ("Transmute, Copper, Gold, Silver", id::COPPER),
    ] {
        let (g, d, c) = play(&state(&format!("hand: {hand}\ndeck: 5 Copper"), "hand: 5 Copper\ndeck: 5 Estate"), id::TRANSMUTE);
        assert_eq!(decide(&strat, &g, &d, &c, 0), Choice::Card(expect), "{hand}");
    }
}

#[test]
fn transmute_forced_with_nothing_junk_trashes_the_cheapest_card() {
    let strat = load("big_money_ultimate.toml");
    let (g, d, c) = play(&state("hand: Transmute, Gold, Silver\ndeck: 5 Copper", "hand: 5 Copper\ndeck: 5 Estate"), id::TRANSMUTE);
    assert_eq!(decide(&strat, &g, &d, &c, 0), Choice::Card(id::SILVER));
}

#[test]
fn apprentice_trashes_by_the_trash_rules_and_never_passes() {
    let strat = load("big_money_ultimate.toml");
    for (hand, expect) in [
        ("Apprentice, Gold, Estate, Copper", id::ESTATE),
        ("Apprentice, Gold, Curse, Estate", id::CURSE),
        ("Apprentice, Gold, Silver", id::SILVER),
    ] {
        let (g, d, c) = play(&state(&format!("hand: {hand}\ndeck: 5 Copper"), "hand: 5 Copper\ndeck: 5 Estate"), id::APPRENTICE);
        assert!(!c.contains(&Choice::Pass));
        assert_eq!(decide(&strat, &g, &d, &c, 0), Choice::Card(expect), "{hand}");
    }
}

// ---------------------------------------------------------------------------------------
// Scrying Pool: on my own deck discard what I don't want to draw (junk, and Treasures worse
// than my deck's average non-Action card), keep Actions and good Treasures; on an opponent's
// deck the reverse: discard their good cards and leave their junk on top.
// ---------------------------------------------------------------------------------------

fn pool_own(top: &str, deck: &str) -> Choice {
    let strat = load("big_money_ultimate.toml");
    let (g, d, c) = play(&state(&format!("hand: Scrying Pool\ndeck top: {top}\ndeck: {deck}"), "hand: 5 Copper\ndeck: 5 Estate"), id::SCRYING_POOL);
    assert_eq!(d.for_player, 0);
    assert_eq!(c.len(), 2, "the card or Pass");
    decide(&strat, &g, &d, &c, 0)
}

#[test]
fn scrying_pool_own_deck_discards_junk_and_keeps_actions_and_gold() {
    assert_eq!(pool_own("Curse", "5 Copper, Gold"), Choice::Card(id::CURSE));
    assert_eq!(pool_own("Estate", "5 Copper, Gold"), Choice::Card(id::ESTATE));
    assert_eq!(pool_own("Province", "5 Copper, Gold"), Choice::Card(id::PROVINCE));
    assert_eq!(pool_own("Village", "5 Copper, Gold"), Choice::Pass, "Actions are kept: the reveal draws them all");
    assert_eq!(pool_own("Gold", "5 Copper, Estate"), Choice::Pass);
    assert_eq!(pool_own("Potion", "5 Copper, Estate"), Choice::Pass, "a Potion is kept even though it is worth no coins");
}

#[test]
fn scrying_pool_own_deck_copper_depends_on_how_rich_the_rest_is() {
    // Early deck: the average non-Action card is worth less than $1: keep the Copper.
    assert_eq!(pool_own("Copper", "6 Copper, 3 Estate"), Choice::Pass);
    // A rich deck: a Copper is below average: discard it and reach for something better.
    assert_eq!(pool_own("Copper", "5 Gold, 2 Silver"), Choice::Card(id::COPPER));
}

fn pool_opponent(top: &str) -> Choice {
    let strat = load("big_money_ultimate.toml");
    // Player 1 has no deck of its own, so the first decision is about player 2's top card.
    let (g, d, c) = play(&state("hand: Scrying Pool", &format!("hand: 5 Copper\ndeck top: {top}\ndeck: 4 Estate")), id::SCRYING_POOL);
    assert_eq!((d.player, d.for_player), (0, 1));
    decide(&strat, &g, &d, &c, 0)
}

#[test]
fn scrying_pool_opponent_deck_discards_their_good_cards_and_keeps_their_junk() {
    assert_eq!(pool_opponent("Gold"), Choice::Card(id::GOLD));
    assert_eq!(pool_opponent("Silver"), Choice::Card(id::SILVER));
    assert_eq!(pool_opponent("Village"), Choice::Card(id::VILLAGE));
    assert_eq!(pool_opponent("Potion"), Choice::Card(id::POTION));
    assert_eq!(pool_opponent("Curse"), Choice::Pass);
    assert_eq!(pool_opponent("Estate"), Choice::Pass);
    assert_eq!(pool_opponent("Copper"), Choice::Pass);
}

// ---------------------------------------------------------------------------------------
// Herbalist, Alchemist, University.
// ---------------------------------------------------------------------------------------

/// Player 1 has just passed the Buy phase with `in_play` in play; returns the strategy's answer
/// to the end-of-turn offer.
fn end_of_turn_offer(strat: &Strategy, in_play: &str) -> Choice {
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 1  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n[player 1]\nhand:\ndeck: 8 Copper\nin play: {in_play}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    let (d, c) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    g.apply(Choice::Pass, &mut NoEvents).unwrap();
    let (d, c2) = next_decision(&mut g);
    let _ = c;
    decide(strat, &g, &d, &c2, 0)
}

#[test]
fn herbalist_puts_back_the_best_treasure() {
    let strat = load("big_money_ultimate.toml");
    assert_eq!(end_of_turn_offer(&strat, "Herbalist, Gold, Silver, Copper"), Choice::Card(id::GOLD));
    assert_eq!(end_of_turn_offer(&strat, "Herbalist, Silver, Copper"), Choice::Card(id::SILVER));
    // A Potion nobody wants is not worth topdecking.
    assert_eq!(end_of_turn_offer(&strat, "Herbalist, Potion"), Choice::Pass);
}

#[test]
fn herbalist_takes_the_potion_when_the_strategy_wants_a_potion_card() {
    let strat = load("familiar_bm.toml");
    assert_eq!(end_of_turn_offer(&strat, "Herbalist, Gold, Potion"), Choice::Card(id::POTION));
}

#[test]
fn alchemist_always_goes_back_on_the_deck() {
    let strat = load("big_money_ultimate.toml");
    assert_eq!(end_of_turn_offer(&strat, "Alchemist, Potion"), Choice::Yes);
}

#[test]
fn university_gains_what_the_gain_list_wants_and_otherwise_declines() {
    let wants = Strategy::parse("name = \"U\"\n[[gain]]\ncard = \"Smithy\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let (g, d, c) = play(&state("hand: University\ndeck: 5 Copper", "hand: 5 Copper\ndeck: 5 Estate"), id::UNIVERSITY);
    assert!(c.contains(&Choice::Pass));
    assert_eq!(decide(&wants, &g, &d, &c, 0), Choice::Card(id::SMITHY));
    let none = Strategy::parse("name = \"N\"\n[[gain]]\ncard = \"Gold\"\n").unwrap();
    assert_eq!(decide(&none, &g, &d, &c, 0), Choice::Pass, "no Action wanted: decline the optional gain");
}

#[test]
fn golem_order_is_decided_without_panicking() {
    let strat = load("big_money_ultimate.toml");
    let text = state("hand: Golem\ndeck top: Copper, Village, Estate, Smithy, Gold\ndeck: 4 Copper", "hand: 5 Copper\ndeck: 5 Estate");
    let (g, d, c) = play(&text, id::GOLEM);
    assert!(c.len() >= 2, "a real order choice between Village and Smithy: {c:?}");
    let pick = decide(&strat, &g, &d, &c, 0);
    assert!(c.contains(&pick));
}

// ---------------------------------------------------------------------------------------
// Whole games.
// ---------------------------------------------------------------------------------------

#[test]
fn familiar_bm_plays_whole_games_and_beats_plain_big_money() {
    let bm = load("big_money.toml");
    let fam = load("familiar_bm.toml");
    let refs = [&bm, &fam];
    let kingdom = dominion_sim::kingdom::resolve("auto", &refs, 3).unwrap();
    assert!(kingdom.contains(&id::FAMILIAR));
    let cfg = MatchConfig { games: 3_000, kingdom, seed: 9, max_turns: 200 };
    let stats = run_match(&refs, &cfg);
    let rate = stats[1].win_rate();
    assert!(rate > 0.5, "Familiar-BM should beat Big Money, got {:.1}%", rate * 100.0);
}

#[test]
fn bots_play_random_alchemy_kingdoms_to_the_end() {
    let bm = load("big_money_ultimate.toml");
    let fam = load("familiar_bm.toml");
    for seed in 0..6u64 {
        let kingdom = dominion_sim::kingdom::resolve("random:alchemy", &[], seed).unwrap();
        assert!(kingdom.contains(&id::POTION));
        let cfg = MatchConfig { games: 150, kingdom, seed, max_turns: 200 };
        let stats = run_match(&[&bm, &fam], &cfg);
        assert_eq!(stats.len(), 2);
    }
}
