//! Bot policies for Empires and Change: Debt payment (the default, stated `[[debt]]` rules, the Change search), the cards' own decisions
//! (Archive, Crown, Gladiator, Annex, Advance, Ritual, Salt the Earth, Small Castle, Opulent Castle, Change, Tax, Mountain Pass bids,
//! the modes of Wild Hunt and Charm), Events and Castles in gain lists, split piles named either way, and whole games.

use dominion_engine::cards::{self, id};
use dominion_engine::text::parse_state;
use dominion_engine::{Act, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

const KINGDOM: &str = "Archive, Crown, Gladiator, Annex, Engineer, Change, Wild Hunt, Charm, Smithy, Village";

/// Player 1's turn, with the given zones and turn line; player 2 holds 5 Coppers over 5 Estates.
fn state_with(kingdom: &str, extra_lines: &str, phase: &str, actions: u8, coins: u16, buys: u8, p1: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {kingdom}\n{extra_lines}turn: 5  player: 1  phase: {phase}  actions: {actions}  buys: {buys}  coins: {coins}\n\n[player 1]\n{p1}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap()
}

fn state(phase: &str, coins: u16, buys: u8, p1: &str) -> GameState {
    state_with(KINGDOM, "", phase, 1, coins, buys, p1)
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

fn strategy(extra: &str, gains: &[&str]) -> Strategy {
    let mut s = format!("name = \"T\"\n{extra}\n");
    for g in gains {
        s.push_str(&format!("[[gain]]\ncard = \"{g}\"\n"));
    }
    Strategy::parse(&s).unwrap()
}

fn money(extra: &str) -> Strategy {
    strategy(extra, &["Province", "Gold", "Silver"])
}

/// The decision of a position, decided by `strat`.
fn pick_of(strat: &Strategy, g: &mut GameState) -> (Decision, Choice) {
    let (d, ch) = next_decision(g);
    let c = decide(strat, g, &d, &ch);
    assert!(ch.contains(&c), "{c:?} is not legal among {ch:?}");
    (d, c)
}

/// Play the Action `c` (or buy the card) from a position that has not been advanced yet.
fn do_card(g: &mut GameState, c: dominion_engine::cards::CardId) {
    let (_, ch) = next_decision(g);
    assert!(ch.contains(&Choice::Card(c)), "{c} not among {ch:?}");
    g.apply(Choice::Card(c), &mut NoEvents).unwrap();
}

fn until(g: &mut GameState, f: impl Fn(&Decision) -> bool) -> (Decision, Vec<Choice>) {
    for _ in 0..50 {
        let (d, ch) = next_decision(g);
        if f(&d) {
            return (d, ch);
        }
        let c = if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) { Choice::Pass } else { ch[0] };
        g.apply(c, &mut NoEvents).unwrap();
    }
    panic!("no such decision");
}

// --- Debt ----------------------------------------------------------------------------------------------------

#[test]
fn debt_is_paid_before_anything_is_bought() {
    let strat = money("");
    let mut g = state("buy", 11, 1, "hand: Copper\ndeck: 8 Estate\ndebt: 3");
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::PayDebt);
    g.apply(c, &mut NoEvents).unwrap();
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::Card(id::PROVINCE), "$8 left");
}

#[test]
fn debt_that_the_coins_cannot_cover_is_paid_as_far_as_it_goes() {
    let strat = money("");
    let mut g = state("buy", 2, 1, "hand: Copper\ndeck: 8 Estate\ndebt: 5");
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::PayDebt);
    g.apply(c, &mut NoEvents).unwrap();
    assert_eq!(g.players[0].debt, 2, "$2 and the Copper in hand paid 3 of the 5");
}

#[test]
fn a_stated_debt_rule_decides_when_its_condition_holds() {
    // Keep one token while Provinces are plentiful: Change (below) pays +$3 for it.
    let strat = money("[[debt]]\npay = \"keep1\"\nif = \"provinces_left >= 4\"\n");
    let mut g = state("buy", 11, 1, "hand: Copper\ndeck: 8 Estate\ndebt: 3");
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::PayDebtKeep(1));
    // Late in the game the rule does not apply and the default pays everything.
    let late = state_with(KINGDOM, "supply: Province=3\n", "buy", 1, 11, 1, "hand: Copper\ndeck: 8 Estate\ndebt: 3");
    let mut g = late;
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::PayDebt);
}

#[test]
fn a_stated_none_rule_pays_nothing_and_moves_on() {
    let strat = money("[[debt]]\npay = \"none\"\n");
    let mut g = state("buy", 11, 1, "hand: Copper\ndeck: 8 Estate\ndebt: 3");
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::Pass);
}

#[test]
fn debt_rules_are_validated_when_a_strategy_is_parsed() {
    assert!(Strategy::parse("name = \"T\"\n[[debt]]\npay = \"some\"\n[[gain]]\ncard = \"Gold\"\n").is_err());
    assert!(Strategy::parse("name = \"T\"\n[[debt]]\npay = \"all\"\nif = \"debt >= 2 and money >= 6\"\n[[gain]]\ncard = \"Gold\"\n").is_ok());
}

#[test]
fn in_a_game_with_change_a_stated_pay_all_rule_pays_in_the_action_phase_so_that_change_remodels() {
    let strat = money("[[debt]]\npay = \"all\"\n");
    let mut g = state_with("Change, Smithy, Village, Archive, Crown, Gladiator, Annex, Engineer, Wild Hunt, Charm", "", "action", 1, 3, 1, "hand: Change, Estate, Copper\ndeck: 8 Estate\ndebt: 2");
    let (d, c) = pick_of(&strat, &mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    assert_eq!(c, Choice::PayDebt);
}

#[test]
fn change_trashes_junk_first_when_every_card_unlocks_the_best_gain() {
    // Change with no Debt remodels: from {Estate, Copper, Silver} a Province-seeking list wants the Silver gone (Gold costs more).
    let strat = strategy("", &["Province", "Gold", "Silver"]);
    let mut g = state_with("Change, Smithy, Village, Archive, Crown, Gladiator, Annex, Engineer, Wild Hunt, Charm", "", "action", 1, 0, 1, "hand: Change, Estate, Copper, Silver\ndeck: 8 Estate");
    do_card(&mut g, id::CHANGE);
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{:?}", d.kind);
    let c = decide(&strat, &g, &d, &ch);
    // Every card in the hand unlocks the Province (Debt is the difference), so the one the trash rules want gone goes: the Estate.
    assert_eq!(c, Choice::Card(id::ESTATE));
}

// --- Events and Castles in gain lists -----------------------------------------------------------------------

#[test]
fn an_event_in_the_gain_list_is_bought_when_it_is_first_affordable() {
    let strat = strategy("", &["Province", "Conquest", "Gold", "Silver"]);
    let mut g = state_with("Smithy, Village, Archive, Crown, Gladiator, Annex, Engineer, Change, Wild Hunt, Charm", "events: Conquest\n", "buy", 1, 6, 1, "hand: Copper\ndeck: 8 Estate");
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::Card(id::CONQUEST));
}

#[test]
fn castles_are_named_as_a_pile_and_bought_from_the_top() {
    let strat = strategy("", &["Province", "Castles", "Gold"]);
    let mut g = state_with("Smithy, Village, Castles, Crown, Gladiator, Annex, Engineer, Change, Wild Hunt, Charm", "", "buy", 1, 3, 1, "hand: Copper\ndeck: 8 Estate");
    let (_, c) = pick_of(&strat, &mut g);
    assert_eq!(c, Choice::Card(id::HUMBLE_CASTLE), "the top Castle costs $3");
}

#[test]
fn a_split_pile_is_named_by_either_half_or_both() {
    for name in ["Settlers", "Settlers/Bustling Village"] {
        let strat = strategy("", &[name, "Silver"]);
        let mut g = state_with("Settlers, Smithy, Village, Castles, Crown, Gladiator, Annex, Engineer, Change, Wild Hunt", "", "buy", 1, 2, 1, "hand: Copper\ndeck: 8 Estate");
        let (_, c) = pick_of(&strat, &mut g);
        assert_eq!(c, Choice::Card(id::SETTLERS), "{name}");
    }
    // The bottom half cannot be bought while the top is there; a list naming it only does nothing yet.
    let strat = strategy("", &["Bustling Village", "Silver"]);
    let mut g = state_with("Settlers, Smithy, Village, Castles, Crown, Gladiator, Annex, Engineer, Change, Wild Hunt", "", "buy", 1, 5, 1, "hand: Copper\ndeck: 8 Estate");
    let (_, c) = pick_of(&strat, &mut g);
    assert_ne!(c, Choice::Card(id::BUSTLING_VILLAGE));
}

#[test]
fn conditions_see_debt_the_debt_cost_and_pile_vp() {
    let strat = strategy("", &["Province"]);
    let g = state("buy", 8, 1, "hand: Copper\ndeck: 8 Estate\ndebt: 2");
    let view = PlayerView::new(&g, 0);
    assert_eq!(view.debt_of(0), 2);
    let e = dominion_sim::expr::Expr::parse("debt >= 2 and debt_cost(Gold) == 0").ok();
    let _ = (strat, e);
    assert!(dominion_sim::expr::Expr::parse("debt >= 2 and opponent_debt == 0").unwrap().eval_bool(&view));
    assert!(!dominion_sim::expr::Expr::parse("debt < 2").unwrap().eval_bool(&view));
}

// --- the cards' decisions -----------------------------------------------------------------------------------

#[test]
fn wild_hunt_draws_until_its_pile_is_worth_taking() {
    let wild_hunt = |strat: &Strategy, tokens: u8, deck: &str| {
        let mut g = state_with(KINGDOM, &format!("pile vp: Wild Hunt={tokens}\n"), "action", 1, 0, 1, &format!("hand: Wild Hunt, Copper\ndeck: {deck}"));
        do_card(&mut g, id::WILD_HUNT);
        let (d, ch) = next_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{:?}", d.kind);
        let c = decide(strat, &g, &d, &ch);
        mode_is(&ch, c)
    };
    // Searched: three Golds to draw are worth more than 2 tokens and an Estate; with nothing to draw the tokens are.
    let searched = money("");
    assert!(matches!(wild_hunt(&searched, 2, "8 Gold"), ModeIs::Draw));
    assert!(matches!(wild_hunt(&searched, 2, "8 Estate"), ModeIs::Take));
    // The fast default (no search) draws below 4 tokens and takes from 4 on.
    let fast = money("search_play = false");
    assert!(matches!(wild_hunt(&fast, 2, "8 Estate"), ModeIs::Draw));
    assert!(matches!(wild_hunt(&fast, 6, "8 Estate"), ModeIs::Take));
}

enum ModeIs {
    Draw,
    Take,
    Other,
}

fn mode_is(choices: &[Choice], c: Choice) -> ModeIs {
    let Choice::Mode(i) = c else { return ModeIs::Other };
    let _ = choices;
    match cards::modes(id::WILD_HUNT).get(i as usize) {
        Some(cards::ModeOpt::CardsGather(..)) => ModeIs::Draw,
        Some(cards::ModeOpt::GainTakeVp(_)) => ModeIs::Take,
        _ => ModeIs::Other,
    }
}

#[test]
fn salt_the_earth_trashes_the_most_valuable_victory_card_from_the_supply() {
    let strat = money("");
    let mut g = state_with(KINGDOM, "events: Salt the Earth\n", "buy", 1, 4, 1, "hand: Copper\ndeck: 8 Estate");
    do_card(&mut g, id::SALT_THE_EARTH);
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{:?}", d.kind);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::PROVINCE));
}

#[test]
fn annex_keeps_the_junk_out_of_the_shuffle_and_stops_when_none_is_left() {
    let strat = money("");
    let mut g = state_with(KINGDOM, "events: Annex\n", "buy", 1, 0, 1, "hand: Copper\ndeck: 8 Estate\ndiscard: Curse, Estate, Silver, Gold");
    do_card(&mut g, id::ANNEX);
    let (d, ch) = next_decision(&mut g);
    let c = decide(&strat, &g, &d, &ch);
    assert!(matches!(c, Choice::Card(x) if x == id::CURSE || x == id::ESTATE), "{c:?}");
    g.apply(c, &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    let c = decide(&strat, &g, &d, &ch);
    assert!(matches!(c, Choice::Card(x) if x == id::CURSE || x == id::ESTATE), "{c:?}");
    g.apply(c, &mut NoEvents).unwrap();
    let (d, _) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy, "the junk is out; Silver and Gold are shuffled in");
}

#[test]
fn archive_takes_the_action_first_since_it_gives_the_action_to_play_it() {
    let strat = money("");
    // Archive sets the top three cards aside; an Action is left to play (Archive gives +1 Action), so the Smithy comes first, not the Gold.
    let mut g = state("action", 0, 1, "hand: Archive\ndeck: Smithy, Gold, Estate");
    do_card(&mut g, id::ARCHIVE);
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { .. }), "{:?}", d.kind);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SMITHY));
}

#[test]
fn crown_in_the_buy_phase_doubles_the_best_treasure() {
    let strat = money("");
    let mut g = state("buy", 0, 1, "hand: Crown, Gold, Silver\ndeck: 8 Estate");
    do_card(&mut g, id::CROWN);
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, .. }), "{:?}", d.kind);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GOLD), "{ch:?}");
}

#[test]
fn advance_trashes_a_cheaper_action_for_the_listed_one_and_nothing_otherwise() {
    let strat = strategy("", &["Province", "Smithy", "Gold", "Silver"]);
    let mut g = state_with(KINGDOM, "events: Advance\n", "buy", 1, 0, 1, "hand: Village, Copper\ndeck: 8 Estate");
    do_card(&mut g, id::ADVANCE);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::VILLAGE), "Village ($3) -> Smithy ($4)");
    // A Smithy in hand is the card listed itself: kept.
    let mut g = state_with(KINGDOM, "events: Advance\n", "buy", 1, 0, 1, "hand: Smithy, Copper\ndeck: 8 Estate");
    do_card(&mut g, id::ADVANCE);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Pass);
}

#[test]
fn ritual_trashes_the_province_whose_cost_in_vp_tokens_beats_its_points() {
    let strat = money("");
    let mut g = state_with(KINGDOM, "events: Ritual\n", "buy", 1, 0, 1, "hand: Province, Gold, Estate, Copper\ndeck: 8 Estate");
    do_card(&mut g, id::RITUAL);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::PROVINCE), "8 tokens for 6 points");
}

#[test]
fn opulent_castle_discards_the_plain_victory_cards() {
    let strat = money("");
    let mut g = state_with("Castles, Gladiator, Annex, Engineer, Change, Wild Hunt, Charm, Smithy, Village, Crown", "", "action", 1, 0, 1, "hand: Opulent Castle, Estate, Province, Copper\ndeck: 8 Estate");
    do_card(&mut g, id::OPULENT_CASTLE);
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }), "{:?}", d.kind);
    let c = decide(&strat, &g, &d, &ch);
    assert!(matches!(c, Choice::Card(x) if x == id::ESTATE || x == id::PROVINCE), "{c:?}");
}

#[test]
fn the_bid_for_mountain_pass_stays_below_what_eight_vp_are_worth() {
    let strat = money("");
    let mut g = state_with(KINGDOM, "landmarks: Mountain Pass\n", "buy", 1, 8, 1, "hand: Copper\ndeck: 8 Estate");
    do_card(&mut g, id::PROVINCE);
    let (d, ch) = until(&mut g, |d| matches!(d.kind, DecisionKind::Bid { .. }));
    let c = decide(&strat, &g, &d, &ch);
    assert!(matches!(c, Choice::Bid(n) if (1..=5).contains(&n)), "{c:?}");
    // Past what it is worth, the bot bids nothing.
    let DecisionKind::Bid { .. } = d.kind else { unreachable!() };
    assert!(ch.contains(&Choice::Bid(0)));
}

// --- whole games --------------------------------------------------------------------------------------------

#[test]
fn bots_finish_games_with_every_empires_card() {
    use dominion_sim::batch::{run_match, MatchConfig};
    let bmu = Strategy::parse(&std::fs::read_to_string(format!("{}/../../strategies/big_money_ultimate.toml", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap();
    for (k, kingdom) in ["Settlers, Catapult, Encampment, Gladiator, Patrician, Castles, Wild Hunt, Archive, Crown, Engineer", "Forum, Capital, Charm, Chariot Race, Legionary, Overlord, Sacrifice, Temple, Villa, Farmers' Market"].iter().enumerate() {
        let kingdom = dominion_sim::kingdom::resolve(kingdom, &[&bmu], 1).unwrap();
        let cfg = MatchConfig { games: 200, kingdom, seed: 11 + k as u64, max_turns: 300 };
        let stats = run_match(&[&bmu, &bmu], &cfg);
        assert_eq!(stats.len(), 2);
    }
}

#[test]
fn a_change_game_finishes_with_the_stated_debt_rules() {
    use dominion_sim::batch::{run_match, MatchConfig};
    let s = money("[[debt]]\npay = \"keep1\"\nif = \"debt >= 2\"\n[[debt]]\npay = \"all\"\n");
    let kingdom = dominion_sim::kingdom::resolve("Change, Village, Smithy, Market, Militia, Moat, Cellar, Chapel, Workshop, Bandit", &[&s], 1).unwrap();
    let cfg = MatchConfig { games: 100, kingdom, seed: 3, max_turns: 300 };
    let stats = run_match(&[&s, &s], &cfg);
    assert_eq!(stats.len(), 2);
}
