//! Bot policies for the Promo cards other than Prince (whose defaults are in `start_order_defaults.rs`):
//! the player to the left discards the best card an Envoy revealed, Walled Village goes back onto the deck
//! when it may, Captain plays what the strategy's `[[play]]` rules name and else the most useful card, and
//! whole games with these cards run to the end.

use dominion_engine::cards::id;
use dominion_engine::text::parse_state;
use dominion_engine::{
    Act, Choice, ChoiceBuf, Decision, DecisionKind, GameConfig, GameState, NoEvents, PlayerView, Step, Zone,
};
use dominion_sim::{Strategy, StrategyAgent};

const KINGDOM: &str = "Captain, Envoy, Walled Village, Smithy, Village, Prince, Militia, Market, Cellar, Chapel";

/// Player 1's turn, with the given zones and turn line; player 2 holds 5 Coppers over 5 Estates.
fn state(phase: &str, p1: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: {phase}  actions: 1  buys: 1  coins: 0\n\n[player 1]\n{p1}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap()
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

fn money(extra: &str) -> Strategy {
    Strategy::parse(&format!(
        "name = \"T\"\n{extra}\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n"
    ))
    .unwrap()
}

// --- Envoy -----------------------------------------------------------------------------------------

#[test]
fn the_player_to_the_left_discards_the_best_card_envoy_revealed() {
    let strat = money("");
    let mut g = state("action", "hand: Envoy\ndeck top: Copper, Estate, Gold, Estate, Silver, Platinum");
    next_decision(&mut g);
    g.apply(Choice::Card(id::ENVOY), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!((d.player, d.for_player), (1, 0), "the opponent decides");
    // Five cards were revealed (not the Platinum below them): the Gold is the one to take away.
    assert_eq!(ch.len(), 4);
    assert!(!ch.contains(&Choice::Card(id::PLATINUM)));
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GOLD));
    // Without a Gold, the best of what is there.
    let mut g = state("action", "hand: Envoy\ndeck top: Copper, Estate, Duchy, Estate, Silver, Gold");
    next_decision(&mut g);
    g.apply(Choice::Card(id::ENVOY), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SILVER));
}

// --- Walled Village --------------------------------------------------------------------------------

#[test]
fn walled_village_goes_back_onto_the_deck_when_the_bot_may() {
    let mut strat = money("");
    strat.search_play = false;
    let mut g = state("action", "hand: Walled Village\ndeck: 6 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::WALLED_VILLAGE), &mut NoEvents).unwrap();
    for _ in 0..6 {
        let (d, ch) = next_decision(&mut g);
        let c = decide(&strat, &g, &d, &ch);
        if d.kind == (DecisionKind::YesNo { act: Act::Topdeck }) && d.subject == id::WALLED_VILLAGE {
            assert_eq!(c, Choice::Yes);
            g.apply(c, &mut NoEvents).unwrap();
            assert!(g.players[0].deck_known.peek_top() == Some(id::WALLED_VILLAGE));
            return;
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    panic!("never offered the topdeck");
}

#[test]
fn a_walled_village_with_too_many_actions_is_not_offered() {
    let mut strat = money("");
    strat.search_play = false;
    let mut g = state("action", "hand: Walled Village\nin play: Village, Smithy\ndeck: 6 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::WALLED_VILLAGE), &mut NoEvents).unwrap();
    for _ in 0..8 {
        let (d, ch) = next_decision(&mut g);
        assert!(!matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?}");
        if g.turn.number != 5 {
            return;
        }
        let c = decide(&strat, &g, &d, &ch);
        g.apply(c, &mut NoEvents).unwrap();
        if g.players[0].discard.has(id::WALLED_VILLAGE) {
            return;
        }
    }
}

// --- Captain ---------------------------------------------------------------------------------------

/// Play the Captain in hand and return the decision that picks the Supply Action.
fn captain_pick(strat_for_hand: &str) -> (GameState, Decision, Vec<Choice>) {
    let mut g = state("action", &format!("hand: Captain{strat_for_hand}\ndeck: 6 Estate"));
    next_decision(&mut g);
    g.apply(Choice::Card(id::CAPTAIN), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Supply, act: Act::Play, .. }), "{d:?}");
    (g, d, ch)
}

#[test]
fn captain_plays_what_the_play_rules_name_else_the_most_useful_card() {
    let (g, d, ch) = captain_pick("");
    // Defaults (no search): Smithy's +3 Cards beats a Village's +1 Card +2 Actions, and the Cellar, Militia...
    let mut plain = money("");
    plain.search_play = false;
    assert_eq!(decide(&plain, &g, &d, &ch), Choice::Card(id::SMITHY));
    // A stated [[play]] rule wins, in its order.
    let mut village_first = money("[[play]]\ncard = \"Village\"\n[[play]]\ncard = \"Smithy\"\n");
    village_first.search_play = false;
    assert_eq!(decide(&village_first, &g, &d, &ch), Choice::Card(id::VILLAGE));
    // A rule naming a card that cannot be played is skipped, and one whose condition fails too.
    let mut rules = money("[[play]]\ncard = \"Market\"\n[[play]]\ncard = \"Cellar\"\nif = \"count(Gold) >= 3\"\n[[play]]\ncard = \"Militia\"\n");
    rules.search_play = false;
    assert_eq!(decide(&rules, &g, &d, &ch), Choice::Card(id::MILITIA));
    // Searching the turn also gives a legal pick.
    let searching = money("");
    assert!(ch.contains(&decide(&searching, &g, &d, &ch)));
}

#[test]
fn captains_start_of_turn_pick_follows_the_same_rules() {
    let mut g = state("action", "hand: 5 Copper\nin play: Captain\ndurations: Captain\ndeck: 6 Estate");
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Supply, act: Act::Play, .. }), "{d:?}");
    let mut s = money("[[play]]\ncard = \"Village\"\n");
    s.search_play = false;
    assert_eq!(decide(&s, &g, &d, &ch), Choice::Card(id::VILLAGE));
}

// --- Whole games -----------------------------------------------------------------------------------

fn play_games(strategy: &str, kingdom: &[u8], check: impl Fn(&GameState, usize)) {
    let strat = Strategy::parse(strategy).unwrap();
    let bm = Strategy::parse(&std::fs::read_to_string(format!("{}/../../strategies/big_money_ultimate.toml", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap();
    for seed in 0..30u64 {
        for swap in [false, true] {
            let cfg = GameConfig { num_players: 2, kingdom: kingdom.to_vec(), seed, max_turns: 200, ..Default::default() };
            let mut g = GameState::new(&cfg);
            let mut a = StrategyAgent::new(&strat);
            let mut b = StrategyAgent::new(&bm);
            if swap {
                dominion_engine::play_game(&mut g, &mut [&mut b, &mut a], &mut NoEvents);
            } else {
                dominion_engine::play_game(&mut g, &mut [&mut a, &mut b], &mut NoEvents);
            }
            assert!(g.is_game_over());
            check(&g, if swap { 1 } else { 0 });
        }
    }
}

#[test]
fn captain_strategies_play_whole_games() {
    let kingdom = [id::CAPTAIN, id::VILLAGE, id::SMITHY, id::MILITIA, id::MARKET, id::CELLAR, id::CHAPEL, id::WORKSHOP, id::REMODEL, id::MOAT];
    let captain_turns = std::cell::Cell::new(0u32);
    play_games(
        "name = \"C\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Captain\"\nif = \"count(Captain) < 2\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n[[play]]\ncard = \"Captain\"\n",
        &kingdom,
        |g, me| {
            let ps = &g.players[me];
            captain_turns.set(captain_turns.get() + ps.all_cards().get(id::CAPTAIN) as u32);
            // Nothing is ever lost to the Supply plays: the Supply piles stay within their starting sizes.
            assert!(g.supply.get(id::SMITHY) <= 10 && g.supply.get(id::VILLAGE) <= 10);
        },
    );
    assert!(captain_turns.get() > 0, "the strategy should have bought a Captain");
}

#[test]
fn envoy_and_walled_village_strategies_play_whole_games() {
    let kingdom = [id::ENVOY, id::WALLED_VILLAGE, id::SMITHY, id::MILITIA, id::MARKET, id::CELLAR, id::CHAPEL, id::WORKSHOP, id::REMODEL, id::MOAT];
    let seen = std::cell::Cell::new(0u32);
    play_games(
        "name = \"E\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Walled Village\"\nif = \"count(Walled Village) < 2\"\n[[gain]]\ncard = \"Envoy\"\nif = \"count(Envoy) < 2\"\n[[gain]]\ncard = \"Silver\"\n",
        &kingdom,
        |g, me| {
            let all = g.players[me].all_cards();
            seen.set(seen.get() + (all.get(id::ENVOY) + all.get(id::WALLED_VILLAGE)) as u32);
            assert!(all.total() >= 10);
        },
    );
    assert!(seen.get() > 0);
}
