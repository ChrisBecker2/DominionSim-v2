//! Bot policies for Cornucopia & Guilds: spending Coffers, overpaying (by rule and by search), the
//! Rewards a strategy names, Plaza/Advisor/Jester/Journeyman/Farmhands/Joust defaults, how Coffers are
//! valued by the strategy's own scoring, and whole games of the sample strategies.

use dominion_engine::cards::{self, id};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_search::Evaluator;
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::{GainListEvaluator, Strategy};

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

fn decide(strat: &Strategy, g: &GameState, d: &Decision, choices: &[Choice]) -> Choice {
    strat.decide(&PlayerView::new(g, d.player), d, choices)
}

const KINGDOM: &str = "Smithy, Plaza, Advisor, Jester, Journeyman, Farmhands, Joust, Stonemason, Farrier, Horn of Plenty, Village";

/// Player 1's turn, with the given zones and turn line.
fn state(phase: &str, actions: u8, buys: u8, coins: u16, p1: &str, p2: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: {phase}  actions: {actions}  buys: {buys}  coins: {coins}\n\n[player 1]\n{p1}\n\n[player 2]\n{p2}\n"
    );
    parse_state(&text).unwrap()
}

const P2: &str = "hand: 5 Copper\ndeck: 5 Estate";

fn money(extra: &str) -> Strategy {
    Strategy::parse(&format!(
        "name = \"T\"\n{extra}\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n"
    ))
    .unwrap()
}

// --- Coffers ---------------------------------------------------------------------------------

#[test]
fn coffers_are_spent_when_they_reach_a_better_card_and_saved_otherwise() {
    let strat = money("");
    // $7 and one token: a Province ($8) is within reach: spend.
    let mut g = state("buy", 0, 1, 7, "hand: \ndeck: 5 Copper\ncoffers: 1", P2);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Coffers);
    // $6 and one token: only $7: the best card is a Gold either way, so buy it and keep the token.
    let mut g = state("buy", 0, 1, 6, "hand: \ndeck: 5 Copper\ncoffers: 1", P2);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GOLD));
    // $3 and five tokens: spend until the Province is affordable (one at a time).
    let mut g = state("buy", 0, 1, 3, "hand: \ndeck: 5 Copper\ncoffers: 5", P2);
    for _ in 0..6 {
        let (d, ch) = next_decision(&mut g);
        let c = decide(&strat, &g, &d, &ch);
        if c != Choice::Coffers {
            assert_eq!(c, Choice::Card(id::PROVINCE));
            return;
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    panic!("never bought the Province");
}

#[test]
fn coffers_are_not_spent_for_a_card_that_is_already_affordable() {
    // The list is Province (needs $8) then Silver: $3 + 2 tokens = $5 reaches nothing new: buy Silver.
    let strat = money("");
    let mut g = state("buy", 0, 1, 3, "hand: \ndeck: 5 Copper\ncoffers: 2", P2);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SILVER));
}

#[test]
fn the_strategys_scoring_values_coffers_gained_and_penalizes_spending_for_nothing() {
    let strat = money("");
    let root = state("action", 1, 1, 0, "hand: Smithy\ndeck: 5 Copper\ncoffers: 2", P2);
    let ev = GainListEvaluator::new(&strat);
    let mut base = root;
    base.turn.phase = dominion_engine::Phase::Buy;
    let mut more = base;
    more.players[0].coffers += 1;
    let mut spent = base;
    spent.players[0].coffers -= 1;
    let (v_base, v_more, v_spent) = (ev.leaf_value(&root, &base, 0), ev.leaf_value(&root, &more, 0), ev.leaf_value(&root, &spent, 0));
    assert!(v_more > v_base && v_base > v_spent, "{v_more} {v_base} {v_spent}");
    // A token is worth less than the smallest difference between listed cards.
    let mut silver = base;
    silver.players[0].discard.add(id::SILVER, 1);
    assert!(ev.leaf_value(&root, &silver, 0) > v_more, "a Silver gained beats a token");
    // ...and spending one to gain a Silver (a listed card) is still a gain overall.
    let mut buy = spent;
    buy.players[0].discard.add(id::SILVER, 1);
    assert!(ev.leaf_value(&root, &buy, 0) > v_base);
}

// --- Overpay ---------------------------------------------------------------------------------

/// The Overpay decision after buying `card` with `coins` and `buys` buys.
fn overpay_decision(card: u8, coins: u16, buys: u8, p1: &str) -> (GameState, Decision, Vec<Choice>) {
    let mut g = state("buy", 0, buys, coins, p1, P2);
    let (_, _) = next_decision(&mut g);
    g.apply(Choice::Card(card), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Overpay { .. }), "{d:?}");
    (g, d, ch)
}

#[test]
fn stonemason_is_overpaid_to_gain_two_listed_actions_and_not_for_unlisted_ones() {
    let smithy = Strategy::parse("name = \"S\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Smithy\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let (g, d, ch) = overpay_decision(id::STONEMASON, 6, 1, "hand: \ndeck: 5 Copper");
    let mut by_rule = Strategy::parse("name = \"S\"\nsearch_play = false\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Smithy\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    assert_eq!(decide(&by_rule, &g, &d, &ch), Choice::Overpay(4), "by rule");
    by_rule.search_play = true;
    assert_eq!(decide(&smithy, &g, &d, &ch), Choice::Overpay(4), "two Smithies, $4 each"); 
    // The same by rule alone (no search).
    let mut rules = smithy;
    rules.search_play = false;
    assert_eq!(decide(&rules, &g, &d, &ch), Choice::Overpay(4));
    // No listed Action to gain: nothing is overpaid for it.
    let silver_only = money("");
    assert_eq!(decide(&silver_only, &g, &d, &ch), Choice::Overpay(0));
}

#[test]
fn overpaying_is_wrong_when_the_coins_buy_a_better_card_with_another_buy() {
    // $9 and 2 buys: Stonemason ($2) leaves $7; a Gold ($6) listed above Smithy is worth more than two
    // Smithies, so no more than $1 is overpaid.
    let strat = Strategy::parse("name = \"G\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Smithy\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let (g, d, ch) = overpay_decision(id::STONEMASON, 9, 2, "hand: \ndeck: 5 Copper");
    match decide(&strat, &g, &d, &ch) {
        Choice::Overpay(n) => assert!(n <= 1, "overpaid {n}"),
        c => panic!("{c:?}"),
    }
}

#[test]
fn farrier_is_overpaid_with_coins_that_buy_nothing_else() {
    let strat = money("");
    // $6, one Buy: after the Farrier ($2) there is no Buy left; $4 buys cards.
    let (g, d, ch) = overpay_decision(id::FARRIER, 6, 1, "hand: \ndeck: 12 Copper");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Overpay(4));
    let mut rules = money("");
    rules.search_play = false;
    assert_eq!(decide(&rules, &g, &d, &ch), Choice::Overpay(4));
    // $9 and two Buys: $7 left is a Gold with the second Buy, so none of it is overpaid.
    let (g, d, ch) = overpay_decision(id::FARRIER, 9, 2, "hand: \ndeck: 12 Copper");
    match decide(&strat, &g, &d, &ch) {
        Choice::Overpay(n) => assert!(n <= 1, "overpaid {n}"),
        c => panic!("{c:?}"),
    }
    match decide(&rules, &g, &d, &ch) {
        Choice::Overpay(n) => assert!(n <= 1, "overpaid {n} by rule"),
        c => panic!("{c:?}"),
    }
}

// --- Rewards named in a strategy -------------------------------------------------------------

fn joust_reward(strat: &Strategy) -> Choice {
    let mut g = state("action", 1, 1, 0, "hand: Joust, Province\ndeck: 5 Estate", P2);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    g.apply(Choice::Card(id::JOUST), &mut NoEvents).unwrap();
    let (d, ch2) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { .. }), "{d:?}");
    let _ = ch;
    assert_eq!(decide(strat, &g, &d, &ch2), Choice::Card(id::PROVINCE), "a Province is set aside while Rewards are left");
    g.apply(Choice::Card(id::PROVINCE), &mut NoEvents).unwrap();
    let (d, ch3) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    decide(strat, &g, &d, &ch3)
}

#[test]
fn strategies_naming_different_rewards_take_different_rewards_from_joust() {
    let courser = Strategy::parse("name = \"C\"\n[[gain]]\ncard = \"Courser\"\n[[gain]]\ncard = \"Renown\"\n[[gain]]\ncard = \"Province\"\n").unwrap();
    let renown = Strategy::parse("name = \"R\"\n[[gain]]\ncard = \"Renown\"\n[[gain]]\ncard = \"Courser\"\n[[gain]]\ncard = \"Province\"\n").unwrap();
    assert_eq!(joust_reward(&courser), Choice::Card(id::COURSER));
    assert_eq!(joust_reward(&renown), Choice::Card(id::RENOWN));
    // Naming nothing: the default order puts Renown first.
    assert_eq!(joust_reward(&money("")), Choice::Card(id::RENOWN));
}

#[test]
fn a_strategy_may_name_rewards_without_putting_them_in_the_kingdom() {
    let s = load("joust_courser.toml");
    let refs = s.kingdom_refs();
    assert!(refs.contains(&id::JOUST), "naming a Reward asks for Joust");
    assert!(!refs.iter().any(|&c| cards::is_reward(c)), "a Reward is never a kingdom card: {refs:?}");
    let k = dominion_sim::kingdom::resolve("auto", &[&s], 1).unwrap();
    assert!(k.contains(&id::JOUST) && k.len() == 10);
    // Conditions can read a Reward pile and a count.
    let s = Strategy::parse("name = \"X\"\n[[gain]]\ncard = \"Gold\"\nif = \"count(Courser) < 2 and supply(Courser) > 0\"\n[[gain]]\ncard = \"Silver\"\n");
    assert!(s.is_ok(), "{:?}", s.err());
}

// --- Small defaults --------------------------------------------------------------------------

#[test]
fn plaza_discards_a_copper_for_a_token() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Plaza, Copper, Estate\ndeck: 5 Estate", P2);
    next_decision(&mut g);
    g.apply(Choice::Card(id::PLAZA), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::COPPER));
}

#[test]
fn the_player_to_the_left_discards_the_best_card_advisor_revealed() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Advisor\ndeck: Copper, Gold, Estate, Estate", "hand: 5 Copper\ndeck: 5 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::ADVISOR), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.player, 1);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GOLD));
}

#[test]
fn jester_copies_go_to_the_bot_when_it_wants_the_card_else_the_victim() {
    let mut g = state("action", 1, 1, 0, "hand: Jester\ndeck: 5 Estate", "hand: 5 Copper\ndeck top: Silver, Copper\ndeck: 5 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::JESTER), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }) && d.subject == id::SILVER);
    let mut rules = money("");
    rules.search_play = false;
    // A listed Silver: mine (mode 1 = "You gain a copy").
    assert_eq!(decide(&rules, &g, &d, &ch), Choice::Mode(1));
    // A Copper nobody lists and that costs nothing: theirs.
    let mut g = state("action", 1, 1, 0, "hand: Jester\ndeck: 5 Estate", "hand: 5 Copper\ndeck top: Copper, Copper\ndeck: 5 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::JESTER), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    let no_copper = Strategy::parse("name = \"N\"\nsearch_play = false\n[[gain]]\ncard = \"Gold\"\n").unwrap();
    assert_eq!(decide(&no_copper, &g, &d, &ch), Choice::Mode(0));
}

#[test]
fn journeyman_names_curses_then_victory_cards_then_coppers() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Journeyman\ndeck: Curse, Estate, Estate, Copper, Gold, Silver", P2);
    next_decision(&mut g);
    g.apply(Choice::Card(id::JOURNEYMAN), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Name);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::ESTATE), "two Estates outweigh one Curse (4 vs 6)");
    let mut g = state("action", 1, 1, 0, "hand: Journeyman\ndeck: Curse, Curse, Estate, Copper, Gold, Silver", P2);
    next_decision(&mut g);
    g.apply(Choice::Card(id::JOURNEYMAN), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::CURSE));
}

#[test]
fn farmhands_sets_aside_an_action_it_cannot_play_and_nothing_otherwise() {
    let strat = money("");
    // Bought in the Buy phase: the Village in hand is stuck, so it is set aside.
    let mut g = state("buy", 0, 1, 4, "hand: Village, Gold\ndeck: 5 Copper", P2);
    // (The Gold is played on entering the Buy phase; the Village stays.)
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    let _ = ch;
    g.apply(Choice::Card(id::FARMHANDS), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: dominion_engine::Act::SetAside, .. }), "{d:?}");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::VILLAGE));
}

#[test]
fn horn_of_plenty_is_played_after_the_plain_treasures() {
    let strat = money("");
    let g = state("buy", 0, 1, 0, "hand: Horn of Plenty, Anvil\ndeck: 5 Copper", P2);
    // Choices as the engine would offer them: both in hand, plain Treasures first.
    let choices = [Choice::Card(id::HORN_OF_PLENTY), Choice::Card(id::SILVER), Choice::Pass];
    assert_eq!(strat.rule_play_treasure(&choices), Choice::Card(id::SILVER));
    let choices = [Choice::Card(id::HORN_OF_PLENTY), Choice::Pass];
    assert_eq!(strat.rule_play_treasure(&choices), Choice::Card(id::HORN_OF_PLENTY));
    let _ = g;
}

// --- Whole games -----------------------------------------------------------------------------

#[test]
fn the_sample_strategies_play_whole_games_and_beat_big_money() {
    let bmu = load("big_money_ultimate.toml");
    for (file, floor) in [("young_witch_bm.toml", 0.8), ("baker_guild.toml", 0.55), ("joust_courser.toml", 0.55)] {
        let s = load(file);
        let refs = [&s, &bmu];
        let kingdom = dominion_sim::kingdom::resolve("auto", &refs, 1).unwrap();
        let cfg = MatchConfig { games: 300, kingdom, seed: 5, max_turns: 200 };
        let stats = run_match(&refs, &cfg);
        let rate = stats[0].win_rate();
        assert!(rate > floor, "{file}: {:.1}%", rate * 100.0);
    }
}
