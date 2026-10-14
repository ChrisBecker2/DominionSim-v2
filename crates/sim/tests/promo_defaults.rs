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

fn play_games(strategy: &str, kingdom: &[dominion_engine::CardId], check: impl Fn(&GameState, usize)) {
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

// --- Governor, Dismantle, Church, Marchland, Summon ----------------------------------------------------------

const NEW_KINGDOM: &str = "Governor, Dismantle, Church, Marchland, Summon, Smithy, Village, Market, Militia, Cellar, Watchtower";

fn state_new(phase: &str, p1: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {NEW_KINGDOM}\nturn: 5  player: 1  phase: {phase}  actions: 1  buys: 1  coins: 0\n\n[player 1]\n{p1}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap()
}

fn fast(extra: &str) -> Strategy {
    let mut s = money(extra);
    s.search_play = false;
    s
}

fn fast_with_play(extra: &str) -> Strategy {
    fast(&format!("{extra}\n[[play]]\ncard = \"Governor\"\n[[play]]\ncard = \"Dismantle\"\n[[play]]\ncard = \"Church\"\n"))
}

/// The state after player 1 plays `card` from `p1`'s zones in the Action phase, and the decision that follows.
fn after_play(card: dominion_engine::CardId, p1: &str) -> (GameState, Decision, Vec<Choice>) {
    let mut g = state_new("action", p1);
    next_decision(&mut g);
    g.apply(Choice::Card(card), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    (g, d, ch)
}

// --- Governor ---------------------------------------------------------------------------------------

#[test]
fn governor_remodels_a_gold_late_and_draws_otherwise() {
    let strat = fast("");
    // Many Provinces left: cards.
    let (g, d, ch) = after_play(id::GOVERNOR, "hand: Governor, Gold, Estate\ndeck: 10 Estate");
    assert!(matches!(d.kind, DecisionKind::Mode { .. }));
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Mode(0), "+3 Cards");
    // The Provinces are running out and a Gold is in hand: Remodel it into a Province.
    let mut late = state_new("action", "hand: Governor, Gold, Estate\ndeck: 10 Estate");
    late.supply.set(id::PROVINCE, 3);
    next_decision(&mut late);
    late.apply(Choice::Card(id::GOVERNOR), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut late);
    assert_eq!(decide(&strat, &late, &d, &ch), Choice::Mode(2), "Trash a card, gain one exactly $2 more");
    // No Gold in hand: nothing to remodel.
    let mut late = state_new("action", "hand: Governor, Silver, Estate\ndeck: 10 Estate");
    late.supply.set(id::PROVINCE, 3);
    next_decision(&mut late);
    late.apply(Choice::Card(id::GOVERNOR), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut late);
    assert_eq!(decide(&strat, &late, &d, &ch), Choice::Mode(0));
}

#[test]
fn a_stated_mode_rule_picks_governors_option_by_its_label() {
    let strat = fast("[[mode]]\ncard = \"Governor\"\nchoose = \"Gain a Gold (others a Silver)\"\n");
    let (g, d, ch) = after_play(id::GOVERNOR, "hand: Governor, Gold, Estate\ndeck: 10 Estate");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Mode(1));
    let labels: Vec<String> = dominion_engine::cards::modes(id::GOVERNOR).iter().map(|o| o.label()).collect();
    assert_eq!(labels[0], "+3 Cards (others +1 Card)");
    assert_eq!(labels[2], "Trash a card, gain one costing exactly $2 more (others $1 more)");
    // An unknown label is an error naming the valid ones.
    let err = Strategy::parse("name = \"T\"\n[[mode]]\ncard = \"Governor\"\nchoose = \"nope\"\n[[gain]]\ncard = \"Silver\"\n").err().unwrap();
    assert!(err.contains("+3 Cards"), "{err}");
}

#[test]
fn governors_remodel_trashes_only_for_a_better_card_of_the_exact_cost() {
    // The Governor's own player gains $2 more: a Gold becomes a Province; a Silver ($5: nothing listed) stays.
    let strat = fast("");
    let mut g = state_new("action", "hand: Governor, Gold, Silver, Estate\ndeck: 10 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::GOVERNOR), &mut NoEvents).unwrap();
    let (d, _) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }));
    g.apply(Choice::Mode(2), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.player, 0);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }));
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GOLD), "a Gold ($6) buys exactly $8: the Province");
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    // (The Province is the only card costing exactly $8: it is gained at once.)
    // The other player holds five Coppers: nothing of exactly $1 is wanted, so they keep them.
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.player, 1, "the other player decides next");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Pass);
    assert!(g.players[0].discard.has(id::PROVINCE) && g.trash.has(id::GOLD));
}

#[test]
fn governors_other_players_trash_an_estate_for_a_silver() {
    let strat = fast("");
    let mut g = parse_state(&format!(
        "players: 2\nkingdom: {NEW_KINGDOM}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Governor\ndeck: 10 Estate\n\n[player 2]\nhand: 4 Copper, Estate\ndeck: 5 Estate\n"
    ))
    .unwrap();
    next_decision(&mut g);
    g.apply(Choice::Card(id::GOVERNOR), &mut NoEvents).unwrap();
    next_decision(&mut g);
    g.apply(Choice::Mode(2), &mut NoEvents).unwrap();
    // (The Governor's own hand is empty: nothing is asked of them.)
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.player, 1);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::ESTATE));
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SILVER));
}

// --- Dismantle --------------------------------------------------------------------------------------

#[test]
fn dismantle_trashes_a_curse_or_estate_before_it_trashes_money() {
    let strat = fast_with_play("");
    let (g, d, ch) = after_play(id::DISMANTLE, "hand: Dismantle, Gold, Copper, Estate\ndeck: 10 Estate");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, min: 1, max: 1, .. }));
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::ESTATE), "an Estate becomes a Gold (and a Copper)");
    let (g, d, ch) = after_play(id::DISMANTLE, "hand: Dismantle, Gold, Curse, Estate\ndeck: 10 Estate");
    // Not the Curse: an Estate brings a Gold and a Copper, which the gain list values far above removing a Curse (a Curse brings nothing).
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::ESTATE), "an Estate becomes a Gold");
    let (g, d, ch) = after_play(id::DISMANTLE, "hand: Dismantle, Gold, Curse, Copper
deck: 10 Estate");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::CURSE), "a Curse is worth removing most when nothing else brings anything");
    // Late in the game a Victory card is worth its points: the Silver goes instead.
    let mut late = state_new("action", "hand: Dismantle, Silver, Duchy\ndeck: 10 Estate");
    late.supply.set(id::PROVINCE, 2);
    next_decision(&mut late);
    late.apply(Choice::Card(id::DISMANTLE), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut late);
    assert_eq!(decide(&strat, &late, &d, &ch), Choice::Card(id::SILVER));
}

#[test]
fn dismantle_is_not_played_by_default_with_nothing_worth_trashing() {
    let strat = money("");
    // With the Golds gone, trashing a Province brings only a Silver (otherwise a Gold is gained: Dismantle turns even a Gold into a Gold and
    // a cheaper card, which the turn search plays too).
    let mut g = state_new("action", "hand: Dismantle, Province\ndeck: 10 Estate");
    g.supply.set(id::GOLD, 0);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Pass, "Dismantle would only trash a Province");
    let mut g = state_new("action", "hand: Dismantle, Copper, Estate\ndeck: 10 Estate");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::DISMANTLE));
}

// --- Church -----------------------------------------------------------------------------------------

#[test]
fn church_sets_aside_surplus_money_and_actions_that_cannot_be_played() {
    let strat = fast_with_play("");
    // $12 in hand against a $8 target: one Gold goes, and then no more surplus is left (Province at $8).
    let (mut g, d, ch) = after_play(id::CHURCH, "hand: Church, 4 Gold, Estate\ndeck: 10 Estate");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::SetAside, min: 0, max: 3, .. }));
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GOLD));
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Pass, "$9 left: no surplus");
    // $6: nothing to spare.
    let (g, d, ch) = after_play(id::CHURCH, "hand: Church, 2 Gold, Estate\ndeck: 10 Estate");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Pass);
    // Church gives its Action back, so the Smithy can still be played: it stays in the hand.
    let (g, d, ch) = after_play(id::CHURCH, "hand: Church, Smithy, Silver\ndeck: 10 Estate");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Pass);
    // With no Action left (a Smithy already played, a second one in hand) the Smithy is dead this turn: set it aside.
    let mut g = state_new("action", "hand: Church, Smithy, Silver\ndeck: 10 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::CHURCH), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    g.turn.actions = 0;
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SMITHY));
}

#[test]
fn church_start_of_turn_is_ordered_after_the_cards_a_summon_plays() {
    // Church's cards come into the hand and it trashes one: after a Smithy has drawn, so the trash sees more cards.
    let strat = fast("");
    let mut g = state_new("action", "hand: Estate\ndeck: 10 Estate\nsummoned: Smithy\nchurch: Gold\ndurations: Church (Church)\nin play: Church");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SMITHY));
}

// --- Marchland ----------------------------------------------------------------------------------------

/// Discard the cards the bot wants to, one at a time, and return them.
fn drive_marchland_discards(strat: &Strategy, g: &mut GameState) -> Vec<dominion_engine::CardId> {
    let mut discarded = Vec::new();
    loop {
        let (d, ch) = next_decision(g);
        if !matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) {
            return discarded;
        }
        let c = decide(strat, g, &d, &ch);
        match c {
            Choice::Card(x) => {
                discarded.push(x);
                g.apply(c, &mut NoEvents).unwrap();
            }
            _ => {
                g.apply(c, &mut NoEvents).unwrap();
                return discarded;
            }
        }
    }
}

#[test]
fn marchland_discards_what_the_rest_of_the_turn_does_not_need() {
    let strat = fast("");
    let mut g = state_new("buy", "hand: Estate, Curse, Smithy, Watchtower\ndeck: 10 Estate");
    g.turn.coins = 5;
    let (d, _) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    g.apply(Choice::Card(id::MARCHLAND), &mut NoEvents).unwrap();
    // Everything but the Watchtower (it reacts to a gain).
    let mut discarded = drive_marchland_discards(&strat, &mut g);
    discarded.sort();
    assert_eq!(discarded, vec![id::ESTATE, id::CURSE, id::SMITHY]);
    assert_eq!(g.turn.coins, 3, "$5 spent on the Marchland, $1 per card discarded");
}

#[test]
fn marchland_in_the_action_phase_keeps_treasures_and_playable_actions() {
    let strat = fast("");
    // Gained by Artisan, which used the only Action: the Smithy cannot be played any more, the Silver will be, and the Estate and the Marchland
    // are dead weight.
    let mut g = parse_state(
        "players: 2\nkingdom: Marchland, Artisan, Smithy, Village, Market, Militia, Cellar, Workshop, Mine, Moat\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Artisan, Estate, Silver, Smithy\ndeck: 10 Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n",
    )
    .unwrap();
    next_decision(&mut g);
    g.apply(Choice::Card(id::ARTISAN), &mut NoEvents).unwrap();
    let (d, _) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }));
    g.apply(Choice::Card(id::MARCHLAND), &mut NoEvents).unwrap();
    let mut discarded = drive_marchland_discards(&strat, &mut g);
    discarded.sort();
    assert_eq!(discarded, vec![id::ESTATE, id::SMITHY, id::MARCHLAND]);
}

#[test]
fn marchland_is_not_discarded_for_on_another_players_turn() {
    // Gained by the other player's Governor: the $ would be lost, so the bot discards nothing.
    let strat = fast("");
    let mut g = parse_state(&format!(
        "players: 2\nkingdom: {NEW_KINGDOM}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Governor\ndeck: 10 Estate\n\n[player 2]\nhand: Smithy, Estate, Curse\ndeck: 5 Estate\n"
    ))
    .unwrap();
    next_decision(&mut g);
    g.apply(Choice::Card(id::GOVERNOR), &mut NoEvents).unwrap();
    next_decision(&mut g);
    g.apply(Choice::Mode(2), &mut NoEvents).unwrap();
    let (d, _) = next_decision(&mut g);
    assert_eq!(d.player, 1);
    g.apply(Choice::Card(id::SMITHY), &mut NoEvents).unwrap();
    let (_, ch) = next_decision(&mut g);
    assert!(ch.contains(&Choice::Card(id::MARCHLAND)));
    g.apply(Choice::Card(id::MARCHLAND), &mut NoEvents).unwrap();
    let discarded = drive_marchland_discards(&strat, &mut g);
    assert!(discarded.is_empty(), "{discarded:?}");
}

// --- Summon -------------------------------------------------------------------------------------------

#[test]
fn summon_gains_what_the_list_names_else_the_most_useful_action() {
    let listed = fast("");
    let mut other = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Village\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    other.search_play = false;
    let mut g = state_new("buy", "hand: Estate\ndeck: 10 Estate");
    g.turn.coins = 5;
    next_decision(&mut g);
    g.apply(Choice::Card(id::SUMMON), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.source, Some(id::SUMMON));
    assert!(ch.contains(&Choice::Card(id::SMITHY)) && ch.contains(&Choice::Card(id::VILLAGE)) && !ch.contains(&Choice::Card(id::MARKET)));
    assert_eq!(decide(&other, &g, &d, &ch), Choice::Card(id::VILLAGE), "the gain list first");
    assert_eq!(decide(&listed, &g, &d, &ch), Choice::Card(id::SMITHY), "else the card that gives the most when played free");
}

// --- Whole games --------------------------------------------------------------------------------------

#[test]
fn the_new_promo_strategies_play_whole_games() {
    let kingdom = |extra: dominion_engine::CardId| {
        [extra, id::SMITHY, id::VILLAGE, id::MARKET, id::MILITIA, id::MOAT, id::REMODEL, id::CELLAR, id::WORKSHOP, id::MINE]
    };
    for (name, card, check) in [
        ("governor_bm", id::GOVERNOR, id::GOVERNOR),
        ("dismantle_bm", id::DISMANTLE, id::DISMANTLE),
        ("marchland_bm", id::MARCHLAND, id::MARCHLAND),
        ("summon_bm", id::SUMMON, id::SMITHY),
    ] {
        let path = format!("{}/../../strategies/{name}.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let seen = std::cell::Cell::new(0u32);
        play_games(&text, &kingdom(card), |g, me| {
            let all = g.players[me].all_cards();
            seen.set(seen.get() + all.get(check) as u32);
            let mut total = g.supply.total() + g.trash.total() + g.side_pile_cards();
            for p in 0..2 {
                total += g.players[p].all_cards().total();
            }
            assert!(total >= 100, "{name}: cards are conserved");
        });
        assert!(seen.get() > 0, "{name}: the strategy should have bought its card");
    }
}
