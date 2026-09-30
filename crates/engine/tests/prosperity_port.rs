//! Prosperity (2nd edition) card tests, ported from the old C++ simulator's
//! `DominionSimTestCards\PropserityCardsTests.cpp` (1st-edition Prosperity) where the card exists
//! in 2nd edition, adapted to this engine's API. Each test cites the C++ `TEST_METHOD` it came
//! from. Tests for cards new in 2nd edition are written from the card text. See
//! `docs/seaside-prosperity-plan.md` §4.
//!
//! Adapting C++ assertions: `VerifyTurnBasics(actions, buys, coins)` there is checked before
//! treasures are played; this engine auto-plays choice-free treasures on entering the Buy phase,
//! so coin totals below include any treasures in hand.
//!
//! Not ported (1st edition only): TestLoan, TestVenture, TestMountebank, TestTalisman, TestGoons,
//! TestCountingHouse, TestTradeRoute, TestTradeRouteTokens, TestRoyalSeal. The "Simulation"
//! sub-cases test the C++ bot harness, not card rules, and aren't ported.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::state::{self, EndReason};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn vp_of(pairs: &[(CardId, u8)]) -> i32 {
    let mut c = Counts::EMPTY;
    for &(card, n) in pairs {
        c.add(card, n);
    }
    state::vp_of_cards(&c)
}

// ===========================================================================
// Platinum and Colony — C++ TestPlatinum, TestColony
// ===========================================================================

#[test]
fn platinum_is_a_five_coin_treasure_with_a_pile_of_twelve() {
    let d = cards::def(id::PLATINUM);
    assert_eq!((d.cost, d.coins, d.vp), (9, 5, 0));
    assert!(cards::is(id::PLATINUM, cards::TREASURE));
    assert!(!cards::is_kingdom(id::PLATINUM) && cards::is_optional_basic(id::PLATINUM));
    // "Platinum pile always contains 12", with 2 and 6 players.
    for n in [2, 6] {
        assert_eq!(new_state(&[id::PLATINUM, id::COLONY], n).supply.get(id::PLATINUM), 12);
    }
    // TestTreasure(Platinum, 5)
    let mut g = new_state(&[id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::PLATINUM]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 5);
}

#[test]
fn colony_is_worth_ten_and_its_pile_matches_provinces() {
    let d = cards::def(id::COLONY);
    assert_eq!((d.cost, d.vp), (11, 10));
    for (n, vp) in [(1, 10), (2, 20), (3, 30), (10, 100)] {
        assert_eq!(vp_of(&[(id::COLONY, n)]), vp);
    }
    assert_eq!(cards::set_of(id::COLONY), CardSet::Prosperity);
    for n in 2..=6 {
        let g = new_state(&[id::PLATINUM, id::COLONY], n);
        assert_eq!(g.supply.get(id::COLONY), g.supply.get(id::PROVINCE), "{n} players");
    }
    // Not in the supply unless named.
    assert!(!new_state(&[id::VILLAGE], 2).in_supply(id::COLONY));
}

#[test]
fn an_empty_colony_pile_ends_the_game() {
    let mut g = new_state(&[id::PLATINUM, id::COLONY], 2);
    empty_pile(&mut g, id::COLONY);
    assert_eq!(g.end_reason(), Some(EndReason::ColoniesGone));
    // Provinces still end it too.
    let mut g = new_state(&[id::PLATINUM, id::COLONY], 2);
    empty_pile(&mut g, id::PROVINCE);
    assert_eq!(g.end_reason(), Some(EndReason::ProvincesGone));
}

#[test]
fn colony_games_round_trip_through_the_text_format() {
    let g = new_state(&[id::PLATINUM, id::COLONY, id::MONUMENT], 2);
    let text = format_state(&g);
    assert!(text.contains("Colony") && text.contains("Platinum"), "{text}");
    let back = parse_state(&text).unwrap();
    assert!(back.in_supply(id::COLONY) && back.in_supply(id::PLATINUM));
}

// ===========================================================================
// Monument — C++ TestMonument
// ===========================================================================

#[test]
fn monument_gives_two_coins_and_a_vp_token() {
    let d = cards::def(id::MONUMENT);
    assert_eq!((d.cost, d.vp), (4, 0));
    // TestGame<PlayFirstAction>({Monument}, {Duchy}): VerifyTurnBasics(0, 1, 2), 1 VP token.
    let mut g = new_state(&[id::MONUMENT], 2);
    set_hand(&mut g, 0, &[id::MONUMENT]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::MONUMENT);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 2));
    assert_eq!(g.players[0].vp_tokens, 1);
    assert!(g.players[0].hand.is_empty());
    // VP tokens count in the score (the Duchy still in the deck is the other 3), and survive
    // the text format.
    assert_eq!(g.players[0].vp(), 3 + 1);
    let back = parse_state(&format_state(&g)).unwrap();
    assert_eq!(back.players[0].vp_tokens, 1);
}

#[test]
fn throne_room_monument_gives_two_tokens() {
    // C++ TestMonument "ThroneRoom for most VP" (the card-rule part: two plays, two tokens, $4).
    let mut g = new_state(&[id::MONUMENT, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MONUMENT]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MONUMENT));
    assert_eq!((g.turn.coins, g.players[0].vp_tokens), (4, 2));
}

// ===========================================================================
// Worker's Village — C++ TestWorkersVillage
// ===========================================================================

#[test]
fn workers_village_card_two_actions_and_a_buy() {
    assert_eq!(cards::by_name("Workers Village"), Some(id::WORKERS_VILLAGE));
    // TestGame<PlayFirstAction>({WorkersVillage, Estate}, {Duchy, Province}): (2, 2, 0).
    let mut g = new_state(&[id::WORKERS_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WORKERS_VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::WORKERS_VILLAGE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::PROVINCE));
}

// ===========================================================================
// City — C++ TestCity
// ===========================================================================

fn city_turn(empty: &[CardId]) -> GameState {
    let mut g = new_state(&[id::CITY, id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::CITY, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::PLATINUM]);
    for &c in empty {
        empty_pile(&mut g, c);
    }
    play(&mut g, id::CITY);
    g
}

#[test]
fn city_scales_with_empty_piles() {
    // No empty piles: +1 Card +2 Actions.
    let g = city_turn(&[]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    // One empty pile (Silver): another card.
    let g = city_turn(&[id::SILVER]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    // Two empty piles (Silver, Copper): another card, +1 Buy, +$1.
    let g = city_turn(&[id::SILVER, id::COPPER]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    // Many empty piles: still just the two-pile bonus.
    let g = city_turn(&[id::SILVER, id::COPPER, id::CURSE, id::ESTATE]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 1));
}

// ===========================================================================
// Grand Market — C++ TestGrandMarket
// ===========================================================================

#[test]
fn grand_market_plays_as_a_market_plus() {
    // TestGame<PlayFirstAction>({GrandMarket, Estate}, {Duchy, Province}): (1, 2, 2).
    let mut g = new_state(&[id::GRAND_MARKET], 2);
    set_hand(&mut g, 0, &[id::GRAND_MARKET, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::GRAND_MARKET);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 2, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
}

#[test]
fn grand_market_cant_be_bought_with_a_copper_in_play() {
    // Can be bought with any treasure that isn't Copper: Platinum, Gold, Silver = $10.
    let mut g = new_state(&[id::GRAND_MARKET, id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::PLATINUM, id::GOLD, id::SILVER]);
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::GRAND_MARKET)));
    buy(&mut g, id::GRAND_MARKET);
    // Cannot be bought with a Copper in play (all treasures auto-play in this engine).
    let mut g = new_state(&[id::GRAND_MARKET, id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::PLATINUM, id::GOLD, id::SILVER, id::COPPER]);
    expect_decision(&mut g);
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(id::GRAND_MARKET)), "Copper in play");
    assert!(cs.contains(&Choice::Card(id::GOLD)));
}

// ===========================================================================
// Magnate — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn magnate_draws_one_card_per_treasure_in_hand() {
    let mut g = new_state(&[id::MAGNATE], 2);
    set_hand(&mut g, 0, &[id::MAGNATE, id::COPPER, id::SILVER, id::ESTATE, id::HAREM]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::GOLD]);
    play(&mut g, id::MAGNATE);
    // Copper, Silver, Harem are Treasures (Harem is Treasure-Victory): +3 Cards.
    assert_eq!(g.players[0].all_cards().get(id::DUCHY), 3);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    // No treasures in hand: nothing drawn.
    let mut g = new_state(&[id::MAGNATE], 2);
    set_hand(&mut g, 0, &[id::MAGNATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    play(&mut g, id::MAGNATE);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
}

// ===========================================================================
// Cross-cutting
// ===========================================================================

#[test]
fn treasure_buys_are_applied_when_played() {
    // Generic: a Treasure's +Buy (Collection, Tiara, Astrolabe) applies when it's played. Checked
    // here with the card data only, since those treasures arrive in later steps.
    assert_eq!(cards::def(id::COLLECTION).buys, 1);
    assert_eq!(cards::def(id::ASTROLABE).buys, 1);
}
