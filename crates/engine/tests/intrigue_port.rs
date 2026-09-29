//! Intrigue (2nd edition) card tests, ported from the old C++ simulator's
//! `DominionSimTestCards\IntrigueCardsTests.cpp` (1st-edition Intrigue) where the card exists in
//! 2nd edition, adapted to this engine's API. Each test cites the C++ `TEST_METHOD` it came
//! from; tests for cards new in 2nd edition (Lurker, Diplomat, Mill, Secret Passage, Courtier,
//! Patrol, Replace) are written from the card text. See `docs/intrigue-plan.md` §5a.
//!
//! Not ported (1st edition only): TestGreatHall, TestSecretChamber, TestScout, TestSaboteur,
//! TestCoppersmith, TestTribute.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::state;
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
// Harem — C++ TestHarem
// ===========================================================================

#[test]
fn harem_basics_vp_and_worth() {
    // VerifyBasics<MatchVictory, MatchTreasure>(Harem, "Harem", 6, 2)
    let d = cards::def(id::HAREM);
    assert_eq!((d.cost, d.vp, d.coins), (6, 2, 2));
    assert!(cards::is(id::HAREM, cards::TREASURE) && cards::is(id::HAREM, cards::VICTORY));
    assert_eq!(cards::set_of(id::HAREM), CardSet::Intrigue);
    for (n, vp) in [(1, 2), (2, 4), (3, 6), (10, 20)] {
        assert_eq!(vp_of(&[(id::HAREM, n)]), vp);
    }
    // MakeGame<PlayAllTreasures>(Harem, {Estate, Estate}): played as a treasure for $2.
    let mut g = new_state(&[id::HAREM], 2);
    set_hand(&mut g, 0, &[id::HAREM]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].in_play.get(id::HAREM), 1);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn harem_pile_is_a_victory_pile() {
    // Victory kingdom piles have 8 cards with 2 players, 12 with more.
    assert_eq!(new_state(&[id::HAREM], 2).supply.get(id::HAREM), 8);
    assert_eq!(new_state(&[id::HAREM], 4).supply.get(id::HAREM), 12);
}

#[test]
fn mine_a_silver_into_a_harem_and_a_harem_into_a_gold() {
    // Base C++ suite, TestMine / Harem cases: Harem is a Treasure for Mine in both directions.
    let mut g = new_state(&[id::MINE, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::MINE, id::SILVER]);
    play(&mut g, id::MINE);
    // Only one treasure: the trash choice is Silver (or pass).
    choose(&mut g, Choice::Card(id::SILVER));
    let d = g.pending_decision().expect("gain decision");
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 6, .. }));
    choose(&mut g, Choice::Card(id::HAREM));
    assert_eq!(g.players[0].in_play.get(id::HAREM), 1, "gained to hand, then played as a treasure");
    assert_eq!(g.turn.coins, 2);

    let mut g = new_state(&[id::MINE, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::MINE, id::HAREM]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::HAREM));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.trash.get(id::HAREM), 1);
    assert_eq!(g.turn.coins, 3);
}

// ===========================================================================
// Duke — C++ TestDuke
// ===========================================================================

#[test]
fn duke_is_worth_one_per_duchy() {
    let d = cards::def(id::DUKE);
    assert_eq!((d.cost, d.vp), (5, 0));
    assert!(cards::is(id::DUKE, cards::VICTORY));
    assert_eq!(vp_of(&[(id::DUKE, 1)]), 0);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 1)]), 1 + 3);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 5)]), 5 + 3 * 5);
    assert_eq!(vp_of(&[(id::DUKE, 5), (id::DUCHY, 1)]), 5 + 3);
    assert_eq!(vp_of(&[(id::DUKE, 5), (id::DUCHY, 5)]), 5 * 5 + 5 * 3);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 1), (id::VILLAGE, 1)]), 1 + 3);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 1), (id::VILLAGE, 1), (id::ESTATE, 1)]), 1 + 3 + 1);
}

// ===========================================================================
// Bridge — C++ TestBridge
// ===========================================================================

#[test]
fn bridge_gives_a_buy_a_coin_and_a_discount() {
    let d = cards::def(id::BRIDGE);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::BRIDGE, cards::ACTION));
    // MakeGame<PlayFirstAction>({Bridge}): VerifyTurnBasics(0 actions, 2 buys, $1), discount 1.
    let mut g = new_state(&[id::BRIDGE], 2);
    set_hand(&mut g, 0, &[id::BRIDGE]);
    play(&mut g, id::BRIDGE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins, g.turn.cost_reduction), (0, 2, 1, 1));
    assert_eq!(g.players[0].in_play.get(id::BRIDGE), 1);
    // With $1 and a discount of 1, Estates ($2 -> $1) and Silver ($3 -> $2) cost less; Curse/Copper stay 0.
    assert_eq!(g.cost(id::ESTATE), 1);
    assert_eq!(g.cost(id::SILVER), 2);
    assert_eq!(g.cost(id::COPPER), 0);
    let buyable = choices(&g);
    assert!(buyable.contains(&Choice::Card(id::ESTATE)));
    assert!(!buyable.contains(&Choice::Card(id::SILVER)));
    buy(&mut g, id::ESTATE);
    assert_eq!(g.turn.coins, 0, "paid the reduced cost");
}

#[test]
fn two_throne_rooms_and_two_bridges_buy_two_provinces_and_three_silvers() {
    // C++ TestBridge simulation case: {ThroneRoom, ThroneRoom, Bridge, Bridge, Silver, Silver}.
    // Throne Room plays the second Throne Room twice, each time on a Bridge: 4 Bridge plays with
    // one action = +4 buys (5), +$4, discount 4. With $8 (4 + two Silvers) Provinces cost $4:
    // two Provinces, then three free Silvers.
    let mut g = new_state(&[id::THRONE_ROOM, id::BRIDGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::THRONE_ROOM, id::BRIDGE, id::BRIDGE, id::SILVER, id::SILVER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::THRONE_ROOM));
    choose(&mut g, Choice::Card(id::BRIDGE));
    choose(&mut g, Choice::Card(id::BRIDGE));
    assert_eq!(g.turn.played.get(id::BRIDGE), 4);
    assert_eq!((g.turn.buys, g.turn.coins, g.turn.cost_reduction), (5, 8, 4));
    assert_eq!(g.cost(id::PROVINCE), 4);
    buy(&mut g, id::PROVINCE);
    buy(&mut g, id::PROVINCE);
    for _ in 0..3 {
        buy(&mut g, id::SILVER);
    }
    // Cleanup reshuffles the (empty) deck, so count everything the player owns.
    let gained = g.players[0].all_cards();
    assert_eq!((gained.get(id::PROVINCE), gained.get(id::SILVER)), (2, 2 + 3), "two Silvers in hand plus three bought");
    assert_eq!(g.players[0].turns_taken, 1, "all five buys used, so the turn ended");
}

#[test]
fn bridge_reduces_gain_costs_too_and_resets_next_turn() {
    // Bridge then Workshop: "gain a card costing up to $4" reaches a $5 card.
    let mut g = new_state(&[id::BRIDGE, id::WORKSHOP, id::VILLAGE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::BRIDGE, id::WORKSHOP]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    play(&mut g, id::VILLAGE);
    choose(&mut g, Choice::Card(id::BRIDGE));
    choose(&mut g, Choice::Card(id::WORKSHOP));
    assert!(choices(&g).contains(&Choice::Card(id::MARKET)), "Market costs $4 this turn");
    choose(&mut g, Choice::Card(id::MARKET));
    assert_eq!(g.players[0].discard.get(id::MARKET), 1);
    // The next turn starts without the reduction.
    assert_eq!(state::TurnState::start(1, 2).cost_reduction, 0);
}

#[test]
fn cost_reduction_round_trips_through_the_text_format() {
    let mut g = new_state(&[id::BRIDGE], 2);
    set_hand(&mut g, 0, &[id::BRIDGE, id::COPPER]);
    play(&mut g, id::BRIDGE);
    let text = format_state(&g);
    assert!(text.contains("cost_reduction: 1"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.turn.cost_reduction, 1);
    // Absent when zero.
    assert!(!format_state(&new_state(&[id::BRIDGE], 2)).contains("cost_reduction"));
}

#[test]
fn unimplemented_cards_cannot_be_in_a_kingdom() {
    let not_ready: Vec<CardId> = (cards::FIRST_KINGDOM..cards::NUM_CARDS as CardId).filter(|&c| !cards::is_ready(c)).collect();
    for &c in &not_ready {
        assert!(!cards::kingdom_cards().any(|k| k == c));
        let err = text::parse_kingdom(cards::name(c)).unwrap_err();
        assert!(err.contains("not implemented"), "{err}");
    }
}
