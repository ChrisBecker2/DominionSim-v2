//! Seaside (2nd edition) card tests, ported from the old C++ simulator's
//! `DominionSimTestCards\SeasideCardsTests.cpp` (1st-edition Seaside) where the card exists in
//! 2nd edition, adapted to this engine's API. Each test cites the C++ `TEST_METHOD` it came from.
//! Tests for cards new in 2nd edition are written from the card text. See
//! `docs/seaside-prosperity-plan.md` §4.
//!
//! Adapting C++ assertions: `VerifyTurnBasics(actions, buys, coins)` there is checked before
//! treasures are played; this engine auto-plays choice-free treasures on entering the Buy phase.
//!
//! Not ported (1st edition only): TestSeahag, TestExplorer, TestNavigator, TestAmbassador,
//! TestGhostShip, TestPirateShip, TestPearlDiver, TestEmbargo. "Simulation" sub-cases test the
//! C++ bot harness and aren't ported.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::state::PendingDuration;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

// ===========================================================================
// Bazaar — C++ TestBazaar
// ===========================================================================

#[test]
fn bazaar_card_two_actions_and_a_coin() {
    let d = cards::def(id::BAZAAR);
    assert_eq!((d.cost, d.vp), (5, 0));
    assert_eq!(cards::set_of(id::BAZAAR), CardSet::Seaside);
    // Regular play: MakeGame<PlayFirstAction>({Bazaar}, {Gold, Gold}): hand {Gold}, deck {Gold},
    // (2, 1, $1) before treasures; the drawn Gold then auto-plays for $1 + $3.
    let mut g = new_state(&[id::BAZAAR], 2);
    set_hand(&mut g, 0, &[id::BAZAAR]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    play(&mut g, id::BAZAAR);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 1));
    assert_eq!(g.turn.coins, 1 + 3);
    assert_eq!(g.players[0].in_play.get(id::GOLD), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));

    // No cards to draw.
    let mut g = new_state(&[id::BAZAAR], 2);
    set_hand(&mut g, 0, &[id::BAZAAR]);
    play(&mut g, id::BAZAAR);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 1));
    assert!(g.players[0].hand.is_empty());
}

// ===========================================================================
// The Duration framework: for "as duration" (start-of-turn) behaviour, the C++ suite directly
// sets up `VirtuallyPlayedDurations()` rather than playing a card and cycling a full turn; the
// same shortcut works here: `new_state` already leaves the state at the very start of player 0's
// turn (phase Action, empty stack, `announced: false`), so injecting `pending_durations` and then
// asking for the next decision (`expect_decision`, which calls `advance`) exercises exactly the
// same `push_turn_start_frames` / `resolve_duration_start` path a real second turn would.
// ===========================================================================

fn inject_pending(g: &mut GameState, p: usize, card: CardId, times: u8, arg: CardId) {
    let i = g.players[p].pending_durations_len as usize;
    g.players[p].pending_durations[i] = PendingDuration { card, times, arg, used: false };
    g.players[p].pending_durations_len += 1;
}

// ===========================================================================
// Caravan — C++ TestCaravan (570). "+1 Card +1 Action. Next turn: +1 Card."
// ===========================================================================

#[test]
fn caravan_now_gives_a_card_and_action_and_schedules_next_turn() {
    let d = cards::def(id::CARAVAN);
    assert_eq!((d.cost, d.vp), (4, 0));
    let mut g = new_state(&[id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::CARAVAN, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::CARAVAN);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert!(g.players[0].in_play.has(id::CARAVAN), "stays in play, not discarded at cleanup");
    assert_eq!(g.players[0].pending_durations_len, 1);
    let e = g.players[0].pending_durations[0];
    assert_eq!((e.card, e.times, e.arg), (id::CARAVAN, 1, 0));

    // No cards to draw: the "now" draw finds nothing, but next turn is still scheduled.
    let mut g = new_state(&[id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::CARAVAN, id::ESTATE]);
    play(&mut g, id::CARAVAN);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn caravan_next_turn_draws_one_card() {
    let mut g = new_state(&[id::CARAVAN], 2);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    inject_pending(&mut g, 0, id::CARAVAN, 1, 0);
    let d = expect_decision(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].pending_durations_len, 0, "resolved and cleared");
    assert_eq!(d.kind, DecisionKind::Buy, "no action in a plain hand of one Estate");

    // No cards to draw: resolves cleanly to nothing.
    let mut g = new_state(&[id::CARAVAN], 2);
    inject_pending(&mut g, 0, id::CARAVAN, 1, 0);
    expect_decision(&mut g);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].pending_durations_len, 0);
}

// ===========================================================================
// Wharf — C++ TestWharf (616). "Now and next turn: +2 Cards +1 Buy."
// ===========================================================================

#[test]
fn wharf_now_draws_two_and_a_buy_and_schedules_next_turn() {
    let d = cards::def(id::WHARF);
    assert_eq!((d.cost, d.vp), (5, 0));
    let mut g = new_state(&[id::WHARF], 2);
    set_hand(&mut g, 0, &[id::WHARF, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::GOLD]);
    play(&mut g, id::WHARF);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 2, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(g.players[0].in_play.has(id::WHARF));
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn wharf_next_turn_draws_two_and_a_buy() {
    let mut g = new_state(&[id::WHARF], 2);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    inject_pending(&mut g, 0, id::WHARF, 1, 0);
    expect_decision(&mut g);
    assert_eq!((g.turn.actions, g.turn.buys), (1, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[0].pending_durations_len, 0);
}

// ===========================================================================
// Merchant Ship — C++ TestMerchantShip (684). "Now and next turn: +$2."
// ===========================================================================

#[test]
fn merchant_ship_now_and_next_turn_give_two_coins() {
    let d = cards::def(id::MERCHANT_SHIP);
    assert_eq!((d.cost, d.vp), (5, 0));
    let mut g = new_state(&[id::MERCHANT_SHIP], 2);
    set_hand(&mut g, 0, &[id::MERCHANT_SHIP, id::ESTATE]);
    play(&mut g, id::MERCHANT_SHIP);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].in_play.has(id::MERCHANT_SHIP));

    let mut g = new_state(&[id::MERCHANT_SHIP], 2);
    inject_pending(&mut g, 0, id::MERCHANT_SHIP, 1, 0);
    expect_decision(&mut g);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 2));
    assert_eq!(g.players[0].pending_durations_len, 0);
}

// ===========================================================================
// Lighthouse — C++ TestLighthouse (1119). "+1 Action. Now and next turn: +$1. While in play,
// other players' Attacks don't affect you."
// ===========================================================================

#[test]
fn lighthouse_now_and_next_turn_give_a_coin() {
    let d = cards::def(id::LIGHTHOUSE);
    assert_eq!((d.cost, d.vp), (2, 0));
    let mut g = new_state(&[id::LIGHTHOUSE], 2);
    set_hand(&mut g, 0, &[id::LIGHTHOUSE, id::ESTATE]);
    play(&mut g, id::LIGHTHOUSE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));

    let mut g = new_state(&[id::LIGHTHOUSE], 2);
    inject_pending(&mut g, 0, id::LIGHTHOUSE, 1, 0);
    expect_decision(&mut g);
    assert_eq!(g.turn.coins, 1);
}

#[test]
fn lighthouse_blocks_attacks_while_in_play() {
    // Militia against a Lighthouse-holding victim: no forced discard.
    let mut g = new_state(&[id::LIGHTHOUSE, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_in_play(&mut g, 1, &[id::LIGHTHOUSE]);
    play(&mut g, id::MILITIA);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[1].hand.total(), 5, "Lighthouse blocked the discard");

    // Two Lighthouses: no difference (still just immune).
    let mut g = new_state(&[id::LIGHTHOUSE, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_in_play(&mut g, 1, &[id::LIGHTHOUSE, id::LIGHTHOUSE]);
    play(&mut g, id::MILITIA);
    assert_eq!(g.players[1].hand.total(), 5);

    // Lighthouse owner is unaffected by their own future turns, but *not* by an attack played
    // while they're the victim and someone else's Lighthouse is irrelevant to them, obviously;
    // the interesting case is the middle player in a 3-player game *without* Lighthouse still
    // getting hit.
    let mut g = new_state(&[id::LIGHTHOUSE, id::MILITIA], 3);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_hand(&mut g, 2, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_in_play(&mut g, 2, &[id::LIGHTHOUSE]);
    play(&mut g, id::MILITIA);
    assert_eq!(g.players[2].hand.total(), 5, "Lighthouse-holding player 2 is immune");
    // Player 1 (no Lighthouse) still discards down to 3 Estates; since every card in hand is the
    // same id, `canonical_picks` offers only one distinct choice at each step, so `auto_single`
    // resolves the whole forced discard without ever surfacing an intermediate decision.
    assert_eq!(g.players[1].hand.total(), 3);
}

#[test]
fn lighthouse_blocks_witch_and_torturer_too() {
    let mut g = new_state(&[id::LIGHTHOUSE, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_in_play(&mut g, 1, &[id::LIGHTHOUSE]);
    play(&mut g, id::WITCH);
    assert!(!g.players[1].discard.has(id::CURSE) && !g.players[1].hand.has(id::CURSE), "no Curse gained");

    let mut g = new_state(&[id::LIGHTHOUSE, id::TORTURER], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    set_in_play(&mut g, 1, &[id::LIGHTHOUSE]);
    play(&mut g, id::TORTURER);
    assert_eq!(g.players[1].hand.total(), 3, "no forced choice at all");
}

// ===========================================================================
// Fishing Village — C++ TestFishingVillage (1325). "+2 Actions +$1. Next turn: +1 Action +$1."
// ===========================================================================

#[test]
fn fishing_village_now_and_next_turn() {
    let d = cards::def(id::FISHING_VILLAGE);
    assert_eq!((d.cost, d.vp), (3, 0));
    let mut g = new_state(&[id::FISHING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::FISHING_VILLAGE, id::ESTATE]);
    play(&mut g, id::FISHING_VILLAGE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));

    let mut g = new_state(&[id::FISHING_VILLAGE], 2);
    inject_pending(&mut g, 0, id::FISHING_VILLAGE, 1, 0);
    expect_decision(&mut g);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 1)); // +1 action from new turn, +1, +$1
}

// ===========================================================================
// Tactician — C++ TestTactitian (1228), TestTactitianThroneRoom (1290). "If you have a card in
// hand, discard your hand, and next turn: +5 Cards +1 Action +1 Buy."
// ===========================================================================

#[test]
fn tactician_with_an_empty_hand_does_nothing_and_is_not_a_duration() {
    let d = cards::def(id::TACTICIAN);
    assert_eq!((d.cost, d.vp), (5, 0));
    let mut g = new_state(&[id::TACTICIAN], 2);
    set_hand(&mut g, 0, &[id::TACTICIAN]);
    play(&mut g, id::TACTICIAN);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 0));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].pending_durations_len, 0, "empty hand: no duration scheduled at all");
}

#[test]
fn tactician_discards_the_hand_and_schedules_next_turn() {
    let mut g = new_state(&[id::TACTICIAN], 2);
    set_hand(&mut g, 0, &[id::TACTICIAN, id::ESTATE]);
    play(&mut g, id::TACTICIAN);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
    assert!(g.players[0].in_play.has(id::TACTICIAN));
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn tactician_next_turn_draws_five_and_a_buy_and_action() {
    let mut g = new_state(&[id::TACTICIAN], 2);
    set_deck_known(&mut g, 0, &[id::DUCHY; 6]);
    inject_pending(&mut g, 0, id::TACTICIAN, 1, 0);
    expect_decision(&mut g);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 2));
    assert_eq!(g.players[0].hand.get(id::DUCHY), 5);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
}

#[test]
fn tactician_through_throne_room_keeps_throne_room_when_one_play_activates() {
    // TestTactitianThroneRoom (1290) is 1st edition; current rule: "Even if only one play of the
    // Duration card is keeping it in play, Throne Room stays in play with it." The 1st play
    // discards the hand and schedules; the 2nd finds an empty hand and does nothing. Tactician
    // and Throne Room both stay, and next turn's bonus happens once.
    let mut g = new_state(&[id::THRONE_ROOM, id::TACTICIAN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TACTICIAN, id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TACTICIAN));
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
    assert!(g.turn.duration_held.has(id::TACTICIAN), "Tactician stays");
    assert!(g.turn.duration_held.has(id::THRONE_ROOM), "Throne Room stays with it");
    assert_eq!(g.players[0].pending_durations_len, 1);
    assert_eq!(g.players[0].pending_durations[0].times, 1, "only the 1st play had a hand to discard");
}

#[test]
fn tactician_through_throne_room_with_an_empty_hand_keeps_neither() {
    let mut g = new_state(&[id::THRONE_ROOM, id::TACTICIAN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TACTICIAN]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TACTICIAN));
    assert!(!g.turn.duration_held.has(id::TACTICIAN) && !g.turn.duration_held.has(id::THRONE_ROOM));
    assert_eq!(g.players[0].pending_durations_len, 0);
}

// ===========================================================================
// Haven — C++ TestHaven (1795), TestHavenThroneRoom (1750). "+1 Card +1 Action. Set aside a card
// from your hand face down; next turn put it into your hand." (1st and 2nd edition agree: the
// set-aside card returns to hand next turn either way.)
// ===========================================================================

#[test]
fn haven_sets_aside_a_card_and_schedules_its_return() {
    let d = cards::def(id::HAVEN);
    assert_eq!((d.cost, d.vp), (2, 0));
    let mut g = new_state(&[id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::HAVEN, id::GOLD]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::HAVEN);
    // +1 Card draws the Estate; the set-aside choice is real (Gold or Estate).
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].set_aside, counts_of(&[id::GOLD]));
    assert_eq!(g.players[0].pending_durations_len, 1);
    let e = g.players[0].pending_durations[0];
    assert_eq!((e.card, e.times, e.arg), (id::HAVEN, 1, id::GOLD));
}

#[test]
fn haven_with_no_card_to_set_aside_is_not_a_duration() {
    // C++ TestHaven: "no card to Haven, Haven gets cleaned up (not marked for duration)".
    let mut g = new_state(&[id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::HAVEN]);
    play(&mut g, id::HAVEN);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].pending_durations_len, 0);
}

#[test]
fn haven_next_turn_returns_the_set_aside_card_to_hand() {
    // Estate, not a Treasure, so it doesn't get auto-played away once we reach the Buy decision.
    let mut g = new_state(&[id::HAVEN], 2);
    g.players[0].set_aside = counts_of(&[id::ESTATE]);
    inject_pending(&mut g, 0, id::HAVEN, 1, id::ESTATE);
    expect_decision(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].set_aside.is_empty());
    assert_eq!(g.players[0].pending_durations_len, 0);
}

#[test]
fn haven_via_throne_room_sets_aside_two_different_cards() {
    // TestHavenThroneRoom (1750), "2 cards" case: both resolutions have a card to set aside, so
    // Haven and Throne Room both stay in play, and two *different* cards end up set aside.
    let mut g = new_state(&[id::THRONE_ROOM, id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAVEN, id::ESTATE, id::GOLD]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HAVEN));
    // 1st Haven resolution: +1 Card (nothing to draw), set aside a card (a real 2-way choice).
    choose(&mut g, Choice::Card(id::ESTATE));
    // 2nd Haven resolution: +1 Card, set aside the only remaining card — auto-single (no choice).
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].set_aside, counts_of(&[id::ESTATE, id::GOLD]));
    assert!(g.turn.duration_held.has(id::HAVEN) && g.turn.duration_held.has(id::THRONE_ROOM));
    assert_eq!(g.players[0].pending_durations_len, 2);
}

#[test]
fn haven_via_throne_room_with_one_card_keeps_haven_and_throne_room() {
    // TestHavenThroneRoom, "1 card" case: only the 1st resolution has a card to set aside. The
    // 1st-edition C++ suite discarded Throne Room here; the current rule keeps it in play with
    // Haven ("even if only one play of the Duration card is keeping it in play").
    let mut g = new_state(&[id::THRONE_ROOM, id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAVEN, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    // Only one card in hand throughout: every SetAside pick is auto-single.
    choose(&mut g, Choice::Card(id::HAVEN));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].set_aside, counts_of(&[id::ESTATE]));
    assert!(g.turn.duration_held.has(id::HAVEN), "Haven stays");
    assert!(g.turn.duration_held.has(id::THRONE_ROOM), "Throne Room stays with it");
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn haven_via_throne_room_with_no_cards_holds_neither() {
    // TestHavenThroneRoom, "0 cards" case.
    let mut g = new_state(&[id::THRONE_ROOM, id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAVEN]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HAVEN));
    assert!(g.players[0].hand.is_empty());
    assert!(!g.turn.duration_held.has(id::HAVEN) && !g.turn.duration_held.has(id::THRONE_ROOM));
    assert_eq!(g.players[0].pending_durations_len, 0);
}


// ===========================================================================
// Outpost — C++ TestOutpost (2821). "Your next hand is 3 cards; take an extra turn after this
// one (but not a 3rd turn in a row — 2nd edition only; see below)."
// ===========================================================================

#[test]
fn outpost_marks_extra_turn_and_draws_three_for_the_same_player() {
    let d = cards::def(id::OUTPOST);
    assert_eq!((d.cost, d.vp), (5, 0));
    let mut g = new_state(&[id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::OUTPOST);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 0));
    pass(&mut g); // ends the turn; cleanup draws 3 (Outpost); the same player goes again
    assert_eq!(g.turn.player, 0, "same player again");
    assert!(g.turn.is_extra_turn);
    assert_eq!(g.players[0].hand.total(), 3, "extra turn draws only 3");
    assert!(g.players[0].in_play.has(id::OUTPOST), "stays in play through the extra turn");
    assert_eq!(g.players[0].pending_durations_len, 0, "its (no-op) start-of-turn resolution already ran");
}

#[test]
fn two_outposts_the_same_turn_grant_only_one_extra_turn() {
    // C++ TestOutpost: "Play multiple Outposts, only gets marked for extra turn once."
    let mut g = new_state(&[id::VILLAGE, id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::OUTPOST, id::OUTPOST]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::OUTPOST);
    play(&mut g, id::OUTPOST);
    assert_eq!(g.players[0].in_play.get(id::OUTPOST), 2);
    pass(&mut g); // buy phase -> cleanup -> same player's (single) extra turn
    assert_eq!(g.turn.player, 0);
    assert!(g.turn.is_extra_turn);
    assert_eq!(g.players[0].hand.total(), 3, "only 3, not 6");
    assert_eq!(g.players[0].in_play.get(id::OUTPOST), 2, "both copies stay in play");
}

#[test]
fn throne_room_outpost_only_grants_one_extra_turn_and_has_one_pending_entry() {
    let mut g = new_state(&[id::THRONE_ROOM, id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::OUTPOST]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::OUTPOST));
    // Both resolutions are unconditional no-ops that each schedule (argless, merges into one
    // entry with times=2); Outpost stays in play, and Throne Room (every resolution "activated")
    // stays too.
    assert_eq!(g.players[0].pending_durations_len, 1);
    assert!(g.turn.duration_held.has(id::OUTPOST) && g.turn.duration_held.has(id::THRONE_ROOM));
    pass(&mut g);
    assert_eq!(g.turn.player, 0);
    assert!(g.turn.is_extra_turn);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn outpost_twice_in_a_row_does_not_grant_a_third_turn() {
    // 2nd edition only: "This can't cause you to take more than two turns in a row" — not
    // present in the 1st-edition C++ suite (new test).
    let mut g = new_state(&[id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_known(&mut g, 0, &[id::OUTPOST, id::ESTATE, id::ESTATE]);
    play(&mut g, id::OUTPOST);
    pass(&mut g); // 1st Outpost's extra turn begins
    assert_eq!(g.turn.player, 0);
    assert!(g.turn.is_extra_turn);
    assert!(g.players[0].hand.has(id::OUTPOST), "the 2nd Outpost was drawn into the extra turn's hand");
    play(&mut g, id::OUTPOST); // played during the extra turn itself
    pass(&mut g);
    assert_eq!(g.turn.player, 1, "normal rotation resumes: no 3rd turn in a row");
    assert!(!g.turn.is_extra_turn);
}

/// Pass through decisions until it's `player`'s turn.
fn pass_until_turn_of(g: &mut GameState, player: u8) {
    while g.turn.player != player {
        pass(g);
    }
}

#[test]
fn a_failed_outpost_still_draws_three_and_is_discarded_at_the_next_turns_cleanup() {
    // Official rules: "You only draw 3 cards, even if you know you won't get the extra turn";
    // "If Outpost fails ... you discard it during Clean-up of the next turn, whoever's turn it is."
    let mut g = new_state(&[id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_known(&mut g, 0, &[id::OUTPOST, id::ESTATE, id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::OUTPOST);
    pass(&mut g); // the Outpost turn
    assert!(g.turn.is_extra_turn);
    play(&mut g, id::OUTPOST); // fails: it would be a 3rd turn in a row
    pass(&mut g);
    assert_eq!(g.turn.player, 1);
    assert_eq!(g.players[0].hand.total(), 3, "a failed Outpost still means a 3-card hand");
    assert_eq!(g.players[0].in_play.get(id::OUTPOST), 1, "the failed Outpost stays in play; the first one was discarded");
    assert_eq!(g.players[0].discard_next_cleanup.get(id::OUTPOST), 1);
    assert_eq!(g.players[0].pending_durations_len, 0, "nothing left to do at the start of my next turn");
    let text = format_state(&g);
    assert!(text.contains("discard at next cleanup: Outpost"), "{text}");
    assert_eq!(parse_state(&text).unwrap().players[0].discard_next_cleanup, g.players[0].discard_next_cleanup);
    pass_until_turn_of(&mut g, 0); // the opponent's turn, with its cleanup
    assert!(g.players[0].in_play.is_empty(), "discarded during the opponent's cleanup");
    assert!(g.players[0].discard.has(id::OUTPOST));
    assert!(g.players[0].discard_next_cleanup.is_empty());
    assert!(!g.turn.is_extra_turn);
}

#[test]
fn throne_room_on_a_failed_outpost_stays_with_it_until_the_next_cleanup() {
    // "If you play Outpost multiple times with a card like Throne Room, Throne Room stays in play
    // with Outpost even though you won't get a second extra turn."
    let mut g = new_state(&[id::THRONE_ROOM, id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_known(&mut g, 0, &[id::THRONE_ROOM, id::OUTPOST, id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::OUTPOST);
    pass(&mut g); // the Outpost turn: hand Throne Room, Outpost, Estate
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::OUTPOST));
    pass(&mut g);
    assert_eq!(g.turn.player, 1);
    assert_eq!(g.players[0].hand.total(), 3);
    assert!(g.players[0].in_play.has(id::THRONE_ROOM) && g.players[0].in_play.has(id::OUTPOST));
    pass_until_turn_of(&mut g, 0);
    assert!(g.players[0].in_play.is_empty());
    assert!(g.players[0].discard.has(id::THRONE_ROOM) && g.players[0].discard.has(id::OUTPOST));
}

#[test]
fn outpost_grants_no_extra_turn_once_the_game_has_ended() {
    let mut g = new_state(&[id::OUTPOST], 2);
    set_supply(&mut g, id::PROVINCE, 1);
    set_hand(&mut g, 0, &[id::OUTPOST, id::GOLD, id::GOLD, id::SILVER]);
    play(&mut g, id::OUTPOST);
    buy(&mut g, id::PROVINCE);
    assert!(matches!(adv(&mut g), Step::GameOver), "the last Province ends the game; no Outpost turn");
}

// ===========================================================================
// Astrolabe — new in 2nd edition, Treasure-Duration: "Now and next turn: +$1, +1 Buy." Choice-
// free: it auto-plays, and its next-turn part is a duration (see the plan's step-3 note).
// ===========================================================================

#[test]
fn astrolabe_now_and_next_turn_give_a_coin_and_a_buy() {
    let d = cards::def(id::ASTROLABE);
    assert_eq!((d.cost, d.vp), (3, 0));
    assert!(cards::is(id::ASTROLABE, cards::TREASURE) && cards::is(id::ASTROLABE, cards::DURATION));
    assert!(cards::is_choice_free(id::ASTROLABE));
    let mut g = new_state(&[id::ASTROLABE], 2);
    set_hand(&mut g, 0, &[id::ASTROLABE]);
    expect_decision(&mut g); // choice-free: auto-plays entering the Buy phase
    assert_eq!((g.turn.buys, g.turn.coins), (2, 1));
    assert!(g.turn.duration_held.has(id::ASTROLABE));
    assert_eq!(g.players[0].pending_durations_len, 1);

    let mut g = new_state(&[id::ASTROLABE], 2);
    inject_pending(&mut g, 0, id::ASTROLABE, 1, 0);
    expect_decision(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 1));
}

#[test]
fn two_astrolabes_each_independently_schedule_their_own_next_turn() {
    let mut g = new_state(&[id::ASTROLABE], 2);
    set_hand(&mut g, 0, &[id::ASTROLABE, id::ASTROLABE]);
    expect_decision(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins), (3, 2));
    assert_eq!(g.turn.duration_held.get(id::ASTROLABE), 2);
    assert_eq!(g.players[0].pending_durations_len, 1, "merged into one argless entry");
    assert_eq!(g.players[0].pending_durations[0].times, 2);
}

// ===========================================================================
// Monkey — new in 2nd edition: "Until your next turn, when the player to your right gains a
// card, +1 Card. Next turn: +1 Card."
// ===========================================================================

#[test]
fn monkey_now_is_a_no_op_and_schedules_next_turns_card() {
    let d = cards::def(id::MONKEY);
    assert_eq!((d.cost, d.vp), (3, 0));
    let mut g = new_state(&[id::MONKEY], 2);
    set_hand(&mut g, 0, &[id::MONKEY]);
    play(&mut g, id::MONKEY);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 0));
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn monkey_next_turn_draws_one_card() {
    let mut g = new_state(&[id::MONKEY], 2);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    inject_pending(&mut g, 0, id::MONKEY, 1, 0);
    expect_decision(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn monkey_draws_when_the_player_to_its_owners_right_gains_a_card() {
    // Turn order goes to the left (0 -> 1 -> 2 -> 0), so "the player to my right" is whoever
    // acted just before me: Monkey owned by player 1 reacts to player 0's gains, not player 2's.
    let mut g = new_state(&[id::MONKEY, id::WORKSHOP], 3);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_in_play(&mut g, 1, &[id::MONKEY]);
    set_deck_known(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::SILVER)); // player 0 gains a Silver
    assert_eq!(g.players[1].hand.total(), 1, "Monkey's owner (player 1) draws");
    assert!(g.players[2].hand.is_empty(), "player 2's Monkey (if it had one) would not react here");
}

#[test]
fn monkey_draws_from_an_attackers_gains_during_the_attackers_own_turn() {
    // "including gains during your own turn from your attacks, e.g. Witch giving them a Curse":
    // player 0's Witch gives player 1 a Curse; the player to whose right player 1 sits is player
    // 2 (0 -> 1 -> 2 -> 0), so player 2's Monkey reacts even though it's still player 0's turn.
    let mut g = new_state(&[id::MONKEY, id::WITCH], 3);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_in_play(&mut g, 2, &[id::MONKEY]);
    set_deck_known(&mut g, 2, &[id::ESTATE]);
    play(&mut g, id::WITCH);
    assert!(g.players[1].discard.has(id::CURSE));
    assert_eq!(g.players[2].hand.total(), 1, "Monkey draws off the Witch-caused Curse gain");
}

// ===========================================================================
// Blockade — new in 2nd edition: "Gain a card up to $4, set aside; next turn put it into your
// hand. While set aside, when another player gains a copy on their turn, they gain a Curse."
// ===========================================================================

#[test]
fn blockade_gains_a_card_sets_it_aside_and_schedules_its_return() {
    let d = cards::def(id::BLOCKADE);
    assert_eq!((d.cost, d.vp), (4, 0));
    let mut g = new_state(&[id::BLOCKADE], 2);
    set_hand(&mut g, 0, &[id::BLOCKADE]);
    play(&mut g, id::BLOCKADE);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 4, filter: Filter::Any, dest: Dest::Discard, exact: false });
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].set_aside, counts_of(&[id::SILVER]));
    assert!(g.players[0].discard.is_empty(), "moved out of discard into set_aside");
    assert_eq!(g.players[0].pending_durations_len, 1);
    let e = g.players[0].pending_durations[0];
    assert_eq!((e.card, e.times, e.arg), (id::BLOCKADE, 1, id::SILVER));
}

#[test]
fn blockade_next_turn_returns_the_gained_card_to_hand() {
    let mut g = new_state(&[id::BLOCKADE], 2);
    g.players[0].set_aside = counts_of(&[id::ESTATE]);
    inject_pending(&mut g, 0, id::BLOCKADE, 1, id::ESTATE);
    expect_decision(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].set_aside.is_empty());
}

#[test]
fn blockade_curses_another_player_who_gains_a_copy_on_their_own_turn() {
    let mut g = new_state(&[id::BLOCKADE], 2);
    set_hand(&mut g, 0, &[id::BLOCKADE]);
    play(&mut g, id::BLOCKADE);
    choose(&mut g, Choice::Card(id::SILVER)); // player 0 gains and sets aside a Silver
    pass(&mut g); // end player 0's buy phase -> cleanup -> player 1's turn
    g.turn.coins = 3;
    // `buy`'s cascade continues past this into player 1's own cleanup, which (their deck and
    // discard both starting empty) reshuffles the just-gained cards into a fresh hand — so the
    // Curse may no longer be sitting in `discard` by the time control returns; check ownership.
    buy(&mut g, id::SILVER); // player 1 buys a copy on their own turn
    assert!(g.players[1].all_cards().has(id::CURSE), "cursed for gaining a copy of the set-aside card");
}

#[test]
fn blockade_does_not_curse_its_own_owner() {
    // Blockade itself grants no +Actions, so a Village supplies the actions to also play
    // Workshop this same turn (Blockade's set-aside is only "active" for the rest of *this*
    // turn and the owner's own next turn start, so both plays must happen in the same turn).
    let mut g = new_state(&[id::BLOCKADE, id::WORKSHOP, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::BLOCKADE, id::WORKSHOP]);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::BLOCKADE);
    choose(&mut g, Choice::Card(id::SILVER));
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::SILVER)); // player 0 gains a 2nd Silver themselves
    assert!(!g.players[0].all_cards().has(id::CURSE));
}

// ===========================================================================
// Sailor — new in 2nd edition: "+1 Action. Once this turn, when you gain a Duration card, you
// may play it. Next turn: +$2, and you may trash a card from your hand."
// ===========================================================================

#[test]
fn sailor_now_gives_an_action_and_schedules_next_turn() {
    let d = cards::def(id::SAILOR);
    assert_eq!((d.cost, d.vp), (4, 0));
    let mut g = new_state(&[id::SAILOR], 2);
    set_hand(&mut g, 0, &[id::SAILOR]);
    play(&mut g, id::SAILOR);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 0));
    assert_eq!(g.players[0].pending_durations_len, 1);
    let e = g.players[0].pending_durations[0];
    assert_eq!((e.card, e.times, e.used), (id::SAILOR, 1, false));
}

#[test]
fn sailor_next_turn_gives_two_coins_and_an_optional_trash() {
    let mut g = new_state(&[id::SAILOR], 2);
    set_hand(&mut g, 0, &[id::COPPER]);
    inject_pending(&mut g, 0, id::SAILOR, 1, 0);
    let d = expect_decision(&mut g);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.trash.has(id::COPPER));
}

#[test]
fn sailor_may_play_a_gained_duration_card_once_per_turn() {
    let mut g = new_state(&[id::SAILOR, id::WORKSHOP, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::SAILOR, id::WORKSHOP]);
    play(&mut g, id::SAILOR); // +1 Action refunds the action just spent
    play(&mut g, id::WORKSHOP);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 4, filter: Filter::Any, dest: Dest::Discard, exact: false });
    choose(&mut g, Choice::Card(id::CARAVAN)); // gain a Duration card
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    choose(&mut g, Choice::Yes); // Sailor lets us play it immediately
    assert!(g.players[0].in_play.has(id::CARAVAN), "played straight from the gain");
    let sailor_entry = g.players[0].pending_durations[..g.players[0].pending_durations_len as usize]
        .iter()
        .find(|e| e.card == id::SAILOR)
        .expect("Sailor's own next-turn entry");
    assert!(sailor_entry.used, "Sailor's reaction is spent for the turn");
}

// ===========================================================================
// Tide Pools — new in 2nd edition: "+3 Cards +1 Action. Next turn: discard 2 cards."
// ===========================================================================

#[test]
fn tide_pools_now_draws_three_and_an_action() {
    let d = cards::def(id::TIDE_POOLS);
    assert_eq!((d.cost, d.vp), (4, 0));
    let mut g = new_state(&[id::TIDE_POOLS], 2);
    set_hand(&mut g, 0, &[id::TIDE_POOLS]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::TIDE_POOLS);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 0));
    assert_eq!(g.players[0].hand.get(id::ESTATE), 3);
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn tide_pools_next_turn_forces_a_discard_of_two() {
    let mut g = new_state(&[id::TIDE_POOLS], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY]);
    inject_pending(&mut g, 0, id::TIDE_POOLS, 1, 0);
    let d = expect_decision(&mut g);
    // The 1st Estate is forced (canonical order leaves only it reachable); the 2nd pick is a
    // real choice between the remaining Estate and the Duchy.
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 1, max: 1, ordered: false });
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
}

// ===========================================================================
// Corsair — new in 2nd edition: "+$2. Next turn: +1 Card. Until then, each other player
// trashes the first Silver or Gold they play each turn."
// ===========================================================================

#[test]
fn corsair_now_gives_two_coins_and_schedules_next_turns_card() {
    let d = cards::def(id::CORSAIR);
    assert_eq!((d.cost, d.vp), (5, 0));
    let mut g = new_state(&[id::CORSAIR], 2);
    set_hand(&mut g, 0, &[id::CORSAIR]);
    play(&mut g, id::CORSAIR);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 2));
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn corsair_next_turn_draws_one() {
    let mut g = new_state(&[id::CORSAIR], 2);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    inject_pending(&mut g, 0, id::CORSAIR, 1, 0);
    expect_decision(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn corsair_trashes_the_first_silver_or_gold_another_player_plays_each_turn() {
    let mut g = new_state(&[id::CORSAIR], 2);
    set_in_play(&mut g, 0, &[id::CORSAIR]);
    set_hand(&mut g, 1, &[id::SILVER, id::GOLD]);
    reset_turn(&mut g, 1);
    expect_decision(&mut g); // both treasures auto-play entering the Buy phase
    assert!(g.trash.has(id::SILVER), "the first (lower id) is trashed");
    assert!(!g.trash.has(id::GOLD), "only the first each turn");
    assert!(g.players[1].in_play.has(id::GOLD));
    assert_eq!(g.turn.coins, 2 + 3, "still get the $ for both; only the card itself is trashed");
}

#[test]
fn corsair_does_not_affect_its_own_owner() {
    let mut g = new_state(&[id::CORSAIR], 2);
    set_in_play(&mut g, 0, &[id::CORSAIR]);
    set_hand(&mut g, 0, &[id::SILVER]);
    expect_decision(&mut g); // Silver auto-plays entering Buy
    assert!(!g.trash.has(id::SILVER));
    assert!(g.players[0].in_play.has(id::SILVER));
}

// ===========================================================================
// Pirate — new in 2nd edition: "Next turn: gain a Treasure up to $6 to your hand. When any
// player gains a Treasure, you may play this from your hand."
// ===========================================================================

#[test]
fn pirate_now_is_a_no_op_and_schedules_next_turn() {
    let d = cards::def(id::PIRATE);
    assert_eq!((d.cost, d.vp), (5, 0));
    let mut g = new_state(&[id::PIRATE], 2);
    set_hand(&mut g, 0, &[id::PIRATE]);
    play(&mut g, id::PIRATE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 0));
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn pirate_next_turn_gains_a_treasure_to_hand() {
    let mut g = new_state(&[id::PIRATE], 2);
    inject_pending(&mut g, 0, id::PIRATE, 1, 0);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 6, filter: Filter::Treasure, dest: Dest::Hand, exact: false });
    // Gained straight to hand; driving on into the Buy phase then auto-plays it (choice-free).
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    assert!(g.players[0].hand.has(id::GOLD), "landed in hand before any phase transition");
}

#[test]
fn pirate_may_be_played_from_hand_when_any_player_gains_a_treasure() {
    let mut g = new_state(&[id::PIRATE], 2);
    set_hand(&mut g, 0, &[id::PIRATE]);
    reset_turn(&mut g, 1);
    g.turn.coins = 3;
    buy(&mut g, id::SILVER);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 0);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    let step = choose(&mut g, Choice::Yes);
    // Player 1 had no buys left, so this cascades straight through their cleanup into player
    // 0's own next turn, where Pirate's just-scheduled next-turn effect (gain a Treasure up to
    // $6) resolves immediately — proving the schedule took effect end to end.
    match step {
        Step::Decision(dec) => {
            assert_eq!(dec.player, 0);
            assert_eq!(dec.kind, DecisionKind::Gain { max_cost: 6, filter: Filter::Treasure, dest: Dest::Hand, exact: false });
        }
        other => panic!("expected Pirate's own next-turn Gain decision, got {other:?}"),
    }
}

// ===========================================================================
// Sea Witch — new in 2nd edition: "+2 Cards. Each other player gains a Curse. Next turn:
// +2 Cards, then discard 2 cards."
// ===========================================================================

#[test]
fn sea_witch_now_draws_two_and_curses_opponents() {
    let d = cards::def(id::SEA_WITCH);
    assert_eq!((d.cost, d.vp), (5, 0));
    let mut g = new_state(&[id::SEA_WITCH], 2);
    set_hand(&mut g, 0, &[id::SEA_WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::SEA_WITCH);
    assert_eq!(g.players[0].hand.get(id::ESTATE), 2);
    assert!(g.players[1].discard.has(id::CURSE));
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn sea_witch_next_turn_draws_two_then_discards_two() {
    // With exactly 2 distinct cards drawn and exactly 2 to discard, the canonical (non-
    // decreasing) pick order makes every step a single legal choice, so `auto_single` resolves
    // the whole draw-then-discard sequence without ever surfacing a decision; check the outcome.
    let mut g = new_state(&[id::SEA_WITCH], 2);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    inject_pending(&mut g, 0, id::SEA_WITCH, 1, 0);
    expect_decision(&mut g);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE, id::DUCHY]));
}

// ===========================================================================
// Clerk's start-of-turn reaction (step 3: the TODO from step 2 is now resolved). Its attack is
// already covered by `prosperity_port.rs`.
// ===========================================================================

#[test]
fn clerk_may_be_played_from_hand_at_the_start_of_the_turn() {
    let mut g = new_state(&[id::CLERK], 2);
    set_hand(&mut g, 0, &[id::CLERK]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].in_play.has(id::CLERK));
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn clerk_start_of_turn_reaction_can_be_declined() {
    let mut g = new_state(&[id::CLERK], 2);
    set_hand(&mut g, 0, &[id::CLERK]);
    expect_decision(&mut g);
    choose(&mut g, Choice::No);
    assert!(g.players[0].hand.has(id::CLERK));
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn clerk_start_of_turn_reaction_offered_per_copy() {
    let mut g = new_state(&[id::CLERK], 2);
    set_hand(&mut g, 0, &[id::CLERK, id::CLERK]);
    expect_decision(&mut g);
    choose(&mut g, Choice::Yes);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].in_play.get(id::CLERK), 2);
    assert_eq!(g.turn.coins, 4);
}

// ===========================================================================
// Cross-cutting: text-format round trips with durations pending, and old files without the new
// Seaside fields still parsing.
// ===========================================================================

#[test]
fn text_format_round_trips_pending_durations_and_lingering_flags() {
    let mut g = new_state(&[id::HAVEN, id::CORSAIR, id::THRONE_ROOM], 2);
    g.players[0].set_aside = counts_of(&[id::GOLD]);
    inject_pending(&mut g, 0, id::HAVEN, 1, id::GOLD);
    inject_pending(&mut g, 0, id::CORSAIR, 1, 0);
    set_in_play(&mut g, 0, &[id::HAVEN, id::CORSAIR, id::THRONE_ROOM]);
    g.turn.duration_held.add(id::HAVEN, 1);
    g.turn.duration_held.add(id::CORSAIR, 1);
    g.turn.duration_held.add(id::THRONE_ROOM, 1);
    g.turn.is_extra_turn = true;
    g.turn.corsair_trashed_first = true;
    let text = format_state(&g);
    assert!(text.contains("durations:"), "{text}");
    assert!(text.contains("extra_turn: true") && text.contains("corsair_trashed: true"), "{text}");
    assert!(text.contains("held:"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].pending_durations_len, 2);
    let entries: Vec<_> = back.players[0].pending_durations[..2].iter().map(|e| (e.card, e.times, e.arg)).collect();
    assert!(entries.contains(&(id::HAVEN, 1, id::GOLD)));
    assert!(entries.contains(&(id::CORSAIR, 1, 0)));
    assert!(back.turn.is_extra_turn && back.turn.corsair_trashed_first);
    assert_eq!(back.turn.duration_held, g.turn.duration_held);
    assert_eq!(back.players[0].set_aside, counts_of(&[id::GOLD]));
}

#[test]
fn old_state_text_without_seaside_fields_still_parses() {
    let text = "players: 2\nkingdom: Village, Smithy\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Village\ndeck top:\ndeck: 5 Copper\ndiscard:\nin play:\n\n[player 2]\nhand:\ndeck top:\ndeck: 5 Copper\ndiscard:\nin play:\n";
    let s = parse_state(text).unwrap();
    assert_eq!(s.players[0].pending_durations_len, 0);
    assert!(!s.turn.is_extra_turn && !s.turn.corsair_trashed_first);
    assert!(s.turn.duration_held.is_empty());
}

// ===========================================================================
// Cross-cutting: `determinize` hides Haven's set-aside card from opponents.
// ===========================================================================

#[test]
fn determinize_hides_havens_set_aside_card_from_opponents() {
    let mut g = new_state(&[id::HAVEN], 2);
    set_hand(&mut g, 1, &[id::COPPER]);
    g.players[1].deck_unknown = counts_of(&[id::COPPER, id::SILVER, id::GOLD]);
    g.players[1].set_aside = counts_of(&[id::ESTATE]);
    inject_pending(&mut g, 1, id::HAVEN, 1, id::ESTATE);
    let view = PlayerView::new(&g, 0);
    let mut rng = Rng::new(42);
    for _ in 0..20 {
        let det = view.determinize(&mut rng);
        // Total composition is preserved (Haven's card is still somewhere in player 1's zones).
        assert_eq!(det.players[1].all_cards().total(), g.players[1].all_cards().total());
        assert_eq!(det.players[1].pending_durations_len, 1);
        let arg = det.players[1].pending_durations[0].arg;
        assert!(det.players[1].set_aside.has(arg), "the pending entry's arg always matches what's set aside");
    }
}

// ===========================================================================
// Step 4: the last 10 Seaside cards. None are Durations.
// ===========================================================================

// ===========================================================================
// Cutpurse — C++ TestCutpurse (286). "+$2. Each other player discards a Copper (or reveals a
// hand with no Copper)."
// ===========================================================================

#[test]
fn cutpurse_gives_two_coins() {
    let d = cards::def(id::CUTPURSE);
    assert_eq!((d.cost, d.vp), (4, 0));
    assert_eq!(cards::set_of(id::CUTPURSE), CardSet::Seaside);
    let mut g = new_state(&[id::CUTPURSE], 2);
    set_hand(&mut g, 0, &[id::CUTPURSE, id::ESTATE]);
    play(&mut g, id::CUTPURSE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn cutpurse_makes_each_other_player_discard_a_copper() {
    let mut g = new_state(&[id::CUTPURSE], 2);
    set_hand(&mut g, 0, &[id::CUTPURSE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::COPPER, id::COPPER, id::GOLD]);
    play(&mut g, id::CUTPURSE);
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::COPPER, id::GOLD]));
    assert_eq!(g.players[1].discard, counts_of(&[id::COPPER]));
}

#[test]
fn cutpurse_does_nothing_if_the_victim_has_no_copper_in_hand() {
    // Copper elsewhere (deck/discard) doesn't count: only the hand matters.
    let mut g = new_state(&[id::CUTPURSE], 2);
    set_hand(&mut g, 0, &[id::CUTPURSE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::SILVER, id::GOLD]);
    set_deck_known(&mut g, 1, &[id::COPPER]);
    set_discard(&mut g, 1, &[id::COPPER]);
    play(&mut g, id::CUTPURSE);
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::SILVER, id::GOLD]));
    assert_eq!(g.players[1].discard, counts_of(&[id::COPPER]), "unchanged");
}

// ===========================================================================
// Lookout — C++ TestLookout (455). "+1 Action. Look at the top 3 cards of your deck. Trash one
// of them. Discard one of them. Put the other one back on top." Mandatory (not "may") up to
// however many are available; reuses Sentry's RevealTop + Select stack.
// ===========================================================================

#[test]
fn lookout_basics_and_no_cards_to_look_at() {
    let d = cards::def(id::LOOKOUT);
    assert_eq!((d.cost, d.vp), (3, 0));
    let mut g = new_state(&[id::LOOKOUT], 2);
    set_hand(&mut g, 0, &[id::LOOKOUT, id::ESTATE]);
    play(&mut g, id::LOOKOUT);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn lookout_with_one_card_only_trashes_it() {
    let mut g = new_state(&[id::LOOKOUT], 2);
    set_hand(&mut g, 0, &[id::LOOKOUT, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::LOOKOUT);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.trash, counts_of(&[id::DUCHY]));
    assert!(g.players[0].discard.is_empty());
    assert!(g.players[0].deck_known.is_empty() && g.players[0].deck_unknown.is_empty());
}

#[test]
fn lookout_with_two_cards_trashes_one_and_discards_the_other() {
    let mut g = new_state(&[id::LOOKOUT], 2);
    set_hand(&mut g, 0, &[id::LOOKOUT, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::LOOKOUT);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.trash, counts_of(&[id::DUCHY]));
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY]));
}

#[test]
fn lookout_with_three_cards_trashes_discards_and_keeps_one_on_top() {
    let mut g = new_state(&[id::LOOKOUT], 2);
    set_hand(&mut g, 0, &[id::LOOKOUT, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::LOOKOUT);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.trash, counts_of(&[id::DUCHY]));
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY]));
    let top_down: Vec<CardId> = g.players[0].deck_known.iter_top_down().collect();
    assert_eq!(top_down, vec![id::DUCHY, id::PROVINCE], "the 3rd Duchy goes back on top, above the untouched Province");
}

#[test]
fn lookout_trash_and_discard_are_real_choices_with_distinct_cards() {
    // Adapted from the C++ "Trash Curse, place Gold on KnownDeck" optimization case: a scripted
    // bot there; here, driven explicitly.
    let mut g = new_state(&[id::LOOKOUT], 2);
    set_hand(&mut g, 0, &[id::LOOKOUT]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER, id::CURSE]);
    play(&mut g, id::LOOKOUT);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Trash, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::CURSE));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Discard, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(g.trash.has(id::CURSE));
    assert!(g.players[0].discard.has(id::COPPER));
}

// ===========================================================================
// Warehouse — C++ TestWarehouse (723). "+3 Cards +1 Action. Discard 3 cards."
// ===========================================================================

#[test]
fn warehouse_basics_and_nothing_to_draw_or_discard() {
    let d = cards::def(id::WAREHOUSE);
    assert_eq!((d.cost, d.vp), (3, 0));
    let mut g = new_state(&[id::WAREHOUSE], 2);
    set_hand(&mut g, 0, &[id::WAREHOUSE]);
    play(&mut g, id::WAREHOUSE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 0));
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn warehouse_discards_everything_drawn_when_fewer_than_three() {
    let mut g = new_state(&[id::WAREHOUSE], 2);
    set_hand(&mut g, 0, &[id::WAREHOUSE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    play(&mut g, id::WAREHOUSE);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD, id::GOLD]));
}

#[test]
fn warehouse_discards_exactly_three_with_more_in_hand() {
    // Duchy (not a Treasure) so the leftover cards stay in hand to inspect, rather than
    // auto-playing away once the Buy phase is entered.
    let mut g = new_state(&[id::WAREHOUSE], 2);
    set_hand(&mut g, 0, &[id::WAREHOUSE, id::DUCHY, id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::SILVER]);
    play(&mut g, id::WAREHOUSE);
    // 3 Duchies already in hand + 3 more drawn = 6; discard exactly 3, leaving 3 (all identical,
    // so which physical ones doesn't matter — only the count).
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::DUCHY, id::DUCHY]));
    assert_eq!(g.players[0].discard.get(id::DUCHY), 3);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
}

// ===========================================================================
// Salvager — C++ TestSalvager (819). "+1 Buy. Trash a card from your hand. +$ equal to its
// cost." Mandatory if hand holds anything (the C++ "Must trash if played" case): min 1, not 0.
// ===========================================================================

#[test]
fn salvager_basics_and_no_cards_to_trash() {
    let d = cards::def(id::SALVAGER);
    assert_eq!((d.cost, d.vp), (4, 0));
    let mut g = new_state(&[id::SALVAGER], 2);
    set_hand(&mut g, 0, &[id::SALVAGER]);
    play(&mut g, id::SALVAGER);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 2, 0));
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn salvager_trashes_a_card_for_its_current_cost() {
    let mut g = new_state(&[id::SALVAGER], 2);
    set_hand(&mut g, 0, &[id::SALVAGER, id::GOLD]);
    play(&mut g, id::SALVAGER);
    assert_eq!(g.turn.coins, 6);
    assert!(g.trash.has(id::GOLD));
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn salvager_trashing_a_copper_gains_no_coins() {
    let mut g = new_state(&[id::SALVAGER], 2);
    set_hand(&mut g, 0, &[id::SALVAGER, id::COPPER]);
    play(&mut g, id::SALVAGER);
    assert_eq!(g.turn.coins, 0);
    assert!(g.trash.has(id::COPPER));
}

#[test]
fn salvager_trashing_an_action_gains_its_cost() {
    let mut g = new_state(&[id::SALVAGER, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SALVAGER, id::VILLAGE]);
    play(&mut g, id::SALVAGER);
    assert_eq!(g.turn.coins, 3);
    assert!(g.trash.has(id::VILLAGE));
}

#[test]
fn salvager_uses_the_reduced_cost_when_trashing() {
    let mut g = new_state(&[id::SALVAGER], 2);
    set_hand(&mut g, 0, &[id::SALVAGER, id::GOLD]);
    g.turn.cost_reduction = 1;
    play(&mut g, id::SALVAGER);
    assert_eq!(g.turn.coins, 5);
}

// ===========================================================================
// Treasure Map — C++ TestTreasureMap (975). "Trash this and a Treasure Map from your hand. If
// you trashed two Treasure Maps, gain 4 Golds onto your deck." 2nd edition: "this" is trashed
// only if it's still in play (Throne Room / King's Court: only the 1st resolution can ever
// trash "this").
// ===========================================================================

#[test]
fn treasure_map_alone_just_trashes_itself() {
    let d = cards::def(id::TREASURE_MAP);
    assert_eq!((d.cost, d.vp), (4, 0));
    // Duchy (not a Treasure) alongside Estate so hand stays inspectable after the Buy phase
    // auto-plays any Treasures.
    let mut g = new_state(&[id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::ESTATE, id::DUCHY]);
    play(&mut g, id::TREASURE_MAP);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.trash, counts_of(&[id::TREASURE_MAP]));
    assert!(g.players[0].deck_known.is_empty());
}

#[test]
fn treasure_map_trashes_two_and_gains_four_golds() {
    let mut g = new_state(&[id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::TREASURE_MAP, id::ESTATE, id::DUCHY]);
    play(&mut g, id::TREASURE_MAP);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Card(id::TREASURE_MAP), min: 0, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.trash.get(id::TREASURE_MAP), 2);
    let top_down: Vec<CardId> = g.players[0].deck_known.iter_top_down().collect();
    assert_eq!(top_down, vec![id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
}

#[test]
fn treasure_map_gains_only_as_many_golds_as_the_pile_has() {
    let mut g = new_state(&[id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::TREASURE_MAP]);
    set_supply(&mut g, id::GOLD, 2);
    play(&mut g, id::TREASURE_MAP);
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    assert_eq!(g.trash.get(id::TREASURE_MAP), 2);
    assert_eq!(g.players[0].deck_known.len as u32, 2);
    assert_eq!(g.supply.get(id::GOLD), 0);
}

#[test]
fn treasure_map_gains_nothing_when_the_gold_pile_is_empty() {
    let mut g = new_state(&[id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::TREASURE_MAP]);
    set_supply(&mut g, id::GOLD, 0);
    play(&mut g, id::TREASURE_MAP);
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    assert_eq!(g.trash.get(id::TREASURE_MAP), 2);
    assert!(g.players[0].deck_known.is_empty());
}

#[test]
fn treasure_map_with_three_in_hand_trashes_only_two() {
    let mut g = new_state(&[id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::TREASURE_MAP, id::TREASURE_MAP, id::ESTATE, id::DUCHY]);
    play(&mut g, id::TREASURE_MAP);
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::TREASURE_MAP]));
    assert_eq!(g.trash.get(id::TREASURE_MAP), 2);
    assert_eq!(g.players[0].deck_known.len as u32, 4);
}

#[test]
fn treasure_map_through_throne_room_with_zero_hand_copies_gains_nothing() {
    let mut g = new_state(&[id::THRONE_ROOM, id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TREASURE_MAP, id::ESTATE, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    // Both resolutions' hand-picks find nothing available (no Treasure Map in hand): auto-finish,
    // no decision surfaces for either.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.trash.get(id::TREASURE_MAP), 1, "only the physical copy");
    assert!(g.players[0].deck_known.is_empty());
}

#[test]
fn treasure_map_through_throne_room_with_one_hand_copy_gains_four_golds() {
    let mut g = new_state(&[id::THRONE_ROOM, id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TREASURE_MAP, id::TREASURE_MAP, id::ESTATE, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    // 1st resolution: "this" + the one hand copy (a real, optional choice) -> 4 Golds.
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    // 2nd resolution: "this" already gone; no hand copy left either -> auto-finishes.
    assert_eq!(g.trash.get(id::TREASURE_MAP), 2);
    let top_down: Vec<CardId> = g.players[0].deck_known.iter_top_down().collect();
    assert_eq!(top_down, vec![id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
}

#[test]
fn treasure_map_through_throne_room_with_two_hand_copies_still_gains_only_four() {
    let mut g = new_state(&[id::THRONE_ROOM, id::TREASURE_MAP], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TREASURE_MAP, id::TREASURE_MAP, id::TREASURE_MAP, id::ESTATE, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    // 1st resolution: "this" + one hand copy -> 4 Golds.
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    // 2nd resolution: "this" already gone, but the 2nd hand copy can still be trashed — it just
    // doesn't grant more gold, since this resolution didn't also trash "this".
    choose(&mut g, Choice::Card(id::TREASURE_MAP));
    assert_eq!(g.trash.get(id::TREASURE_MAP), 3, "this + both hand copies, across both resolutions");
    let top_down: Vec<CardId> = g.players[0].deck_known.iter_top_down().collect();
    assert_eq!(top_down, vec![id::GOLD, id::GOLD, id::GOLD, id::GOLD], "only the 1st resolution grants gold");
}

// ===========================================================================
// Island — C++ TestIsland (1616). "Put this and a card from your hand onto your Island mat."
// Action–Victory (2 VP). The mat is public; its cards count as owned (VP, `all_cards`).
// ===========================================================================

#[test]
fn island_basics_and_mat_vp() {
    let d = cards::def(id::ISLAND);
    assert_eq!((d.cost, d.vp), (4, 2));
    assert!(cards::is(id::ISLAND, cards::VICTORY) && cards::is(id::ISLAND, cards::ACTION));

    let mut g = new_state(&[id::ISLAND], 2);
    g.players[0].island_mat.add(id::ISLAND, 1);
    g.players[0].island_mat.add(id::PROVINCE, 1);
    assert_eq!(g.players[0].vp(), 2 + 6);
    assert!(g.players[0].all_cards().has(id::ISLAND) && g.players[0].all_cards().has(id::PROVINCE));
}

#[test]
fn island_alone_goes_onto_the_mat_by_itself() {
    let mut g = new_state(&[id::ISLAND], 2);
    set_hand(&mut g, 0, &[id::ISLAND]);
    play(&mut g, id::ISLAND);
    assert!(g.players[0].hand.is_empty());
    assert!(!g.players[0].in_play.has(id::ISLAND), "moved to the mat, not left in play");
    assert_eq!(g.players[0].island_mat.counts(), counts_of(&[id::ISLAND]));
}

#[test]
fn island_puts_a_hand_card_on_the_mat_too() {
    // Adapted from the C++ "IslandEstate" bot, which always picks the Estate: here, a real
    // 2-way choice (Estate vs. Duchy, neither a Treasure so hand stays inspectable), driven
    // explicitly.
    let mut g = new_state(&[id::ISLAND], 2);
    set_hand(&mut g, 0, &[id::ISLAND, id::ESTATE, id::DUCHY]);
    play(&mut g, id::ISLAND);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    assert_eq!(g.players[0].island_mat.counts(), counts_of(&[id::ISLAND, id::ESTATE]));
}

#[test]
fn island_through_throne_room_moves_only_hand_cards_the_second_time() {
    let mut g = new_state(&[id::THRONE_ROOM, id::ISLAND], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::ISLAND, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::ISLAND));
    // Both resolutions' hand-picks are forced and identical (Estates only): auto-single resolves
    // the whole thing without either surfacing as a decision.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].island_mat.counts(), counts_of(&[id::ISLAND, id::ESTATE, id::ESTATE]));
}

#[test]
fn island_through_throne_room_with_only_one_card_in_hand() {
    let mut g = new_state(&[id::THRONE_ROOM, id::ISLAND], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::ISLAND, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::ISLAND));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].island_mat.counts(), counts_of(&[id::ISLAND, id::ESTATE]));
}

// ===========================================================================
// Native Village — C++ TestNativeVillage (1951). "+2 Actions. Choose one: put the top card of
// your deck face down on your Native Village mat; or put all the cards from your mat into your
// hand." The mat is private (`determinize` hides it); its cards count as owned (VP, `all_cards`).
// Not ported: "-1 Card token" cases (a different expansion's mechanic, not implemented here).
// ===========================================================================

#[test]
fn native_village_basics_and_mat_vp() {
    let d = cards::def(id::NATIVE_VILLAGE);
    assert_eq!((d.cost, d.vp), (2, 0));
    let mut g = new_state(&[id::NATIVE_VILLAGE], 2);
    g.players[0].native_village_mat.add(id::ISLAND, 1);
    g.players[0].native_village_mat.add(id::PROVINCE, 1);
    assert_eq!(g.players[0].vp(), 2 + 6);
}

#[test]
fn native_village_gives_two_actions() {
    let mut g = new_state(&[id::NATIVE_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::NATIVE_VILLAGE, id::PROVINCE]);
    play(&mut g, id::NATIVE_VILLAGE);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Mode { picks: 1, distinct: false });
    choose(&mut g, Choice::Mode(0)); // "put the top card on the mat" (deck is empty: a no-op)
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
}

#[test]
fn native_village_sets_aside_the_top_card() {
    let mut g = new_state(&[id::NATIVE_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::NATIVE_VILLAGE, id::PROVINCE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER]);
    play(&mut g, id::NATIVE_VILLAGE);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::COPPER));
    assert_eq!(g.players[0].native_village_mat.counts(), counts_of(&[id::ESTATE]));
}

#[test]
fn native_village_set_aside_from_an_empty_deck_is_a_no_op() {
    let mut g = new_state(&[id::NATIVE_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::NATIVE_VILLAGE, id::PROVINCE]);
    play(&mut g, id::NATIVE_VILLAGE);
    choose(&mut g, Choice::Mode(0));
    assert!(g.players[0].native_village_mat.is_empty());
}

#[test]
fn native_village_takes_an_empty_mat() {
    let mut g = new_state(&[id::NATIVE_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::NATIVE_VILLAGE, id::PROVINCE]);
    play(&mut g, id::NATIVE_VILLAGE);
    choose(&mut g, Choice::Mode(1)); // "take the mat"
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
    assert!(g.players[0].native_village_mat.is_empty());
}

#[test]
fn native_village_takes_a_nonempty_mat_into_hand() {
    // Non-Treasure mat cards, so hand stays inspectable after the Buy phase auto-plays Treasures.
    let mut g = new_state(&[id::NATIVE_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::NATIVE_VILLAGE, id::PROVINCE]);
    g.players[0].native_village_mat.add(id::DUCHY, 1);
    g.players[0].native_village_mat.add(id::ESTATE, 1);
    play(&mut g, id::NATIVE_VILLAGE);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE, id::DUCHY, id::ESTATE]));
    assert!(g.players[0].native_village_mat.is_empty());
}

#[test]
fn native_village_round_trip_through_conspirator_draws_the_2nd_gold() {
    // Adapted from the C++ "Round trip Gold through NativeVillage to use Conspirator" scenario
    // (a scripted-bot Simulation case there): put a Gold on the mat with one Native Village, take
    // it back with another, driven explicitly rather than via a bot. The Gold, once back in hand,
    // auto-plays as soon as the Buy phase is entered (no more actions left), landing in `in_play`
    // rather than staying visible in `hand` — checked there instead.
    let mut g = new_state(&[id::NATIVE_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::NATIVE_VILLAGE, id::NATIVE_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    play(&mut g, id::NATIVE_VILLAGE);
    choose(&mut g, Choice::Mode(0)); // set the Gold aside on the mat
    assert_eq!(g.players[0].native_village_mat.counts(), counts_of(&[id::GOLD]));
    play(&mut g, id::NATIVE_VILLAGE);
    choose(&mut g, Choice::Mode(1)); // take it back into hand
    assert!(g.players[0].native_village_mat.is_empty());
    assert!(g.players[0].in_play.has(id::GOLD), "taken into hand, then auto-played");
}

// ===========================================================================
// Smugglers — C++ TestSmugglers (2656). "Gain a copy of a card costing up to $6 that the player
// to your right gained on their last turn. Choose among those cards that are still in the
// supply, at the current cost."
// ===========================================================================

#[test]
fn smugglers_basics() {
    let d = cards::def(id::SMUGGLERS);
    assert_eq!((d.cost, d.vp), (3, 0));
}

#[test]
fn smugglers_with_nothing_gained_last_turn_does_nothing() {
    let mut g = new_state(&[id::SMUGGLERS], 2);
    set_hand(&mut g, 0, &[id::SMUGGLERS, id::ESTATE]);
    play(&mut g, id::SMUGGLERS);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn smugglers_offers_only_the_right_hand_players_last_turn_gains_up_to_six() {
    let mut g = new_state(&[id::SMUGGLERS, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SMUGGLERS, id::ESTATE]);
    for &c in &[id::CURSE, id::COPPER, id::ESTATE, id::VILLAGE, id::GOLD, id::PROVINCE] {
        g.players[1].last_turn_gains.add(c, 1);
    }
    play(&mut g, id::SMUGGLERS);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 6, filter: Filter::Any, dest: Dest::Discard, exact: false });
    let cs = choices(&g);
    for &c in &[id::CURSE, id::COPPER, id::ESTATE, id::VILLAGE, id::GOLD] {
        assert!(cs.contains(&Choice::Card(c)), "{} (gained last turn, $6 or less) should be offered", cards::name(c));
    }
    assert!(!cs.contains(&Choice::Card(id::PROVINCE)), "Province ($8) is over the $6 cap");
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
}

#[test]
fn smugglers_excludes_a_card_that_currently_costs_more_than_six() {
    // Smugglers checks the *current* cost, not whatever it cost when the right-hand player
    // originally gained it.
    let mut g = new_state(&[id::SMUGGLERS, id::KINGS_COURT], 2);
    set_hand(&mut g, 0, &[id::SMUGGLERS]);
    g.players[1].last_turn_gains.add(id::KINGS_COURT, 1); // base cost $7
    play(&mut g, id::SMUGGLERS);
    assert!(g.players[0].hand.is_empty() && g.players[0].discard.is_empty(), "King's Court costs $7 right now, over the cap");
}

#[test]
fn smugglers_uses_the_current_cost_including_an_active_bridge() {
    // Village supplies the 2nd action (Bridge and Smugglers are both terminal). King's Court is
    // the only candidate, so gaining it resolves via `auto_single` without a visible decision —
    // checked via the outcome (discard) instead.
    let mut g = new_state(&[id::SMUGGLERS, id::BRIDGE, id::VILLAGE, id::KINGS_COURT], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::BRIDGE, id::SMUGGLERS]);
    g.players[1].last_turn_gains.add(id::KINGS_COURT, 1);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::BRIDGE); // -$1 to every card's cost this turn
    play(&mut g, id::SMUGGLERS);
    assert!(g.players[0].discard.has(id::KINGS_COURT), "King's Court costs $6 this turn with Bridge active");
}

#[test]
fn smugglers_reads_the_right_hand_players_actual_last_turn() {
    let mut g = new_state(&[id::SMUGGLERS], 2);
    reset_turn(&mut g, 1);
    g.turn.coins = 6;
    set_hand(&mut g, 1, &[]);
    // Buying with the last buy cascades straight through cleanup into player 0's turn (their
    // deck/discard are both empty, so their opening hand is empty too, landing on a fresh Buy
    // decision rather than pausing at a PlayAction one — nothing more to do here).
    buy(&mut g, id::GOLD);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.players[1].last_turn_gains.counts(), counts_of(&[id::GOLD]));
    // Player 0 landed on a Buy decision (their empty hand had no actions); reset to a clean
    // Action-phase turn (preserving zones, including `last_turn_gains`) to give them Smugglers.
    reset_turn(&mut g, 0);
    set_hand(&mut g, 0, &[id::SMUGGLERS]);
    // Gold is the only candidate, so it's gained via `auto_single` with no visible decision.
    play(&mut g, id::SMUGGLERS);
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
}

// ===========================================================================
// Treasury — C++ TestTreasury (2939). "+1 Card +1 Action +$1. At the end of your Buy phase this
// turn, if you didn't gain a Victory card in it, you may put this onto your deck." 2nd edition:
// the trigger moved from "start of Clean-up" to "end of your Buy phase", and the condition is
// "gained" (any gain during the Buy phase), not just "bought" — broader than the 1st edition's
// literal-purchase check, though every C++ case here only ever exercises a literal buy.
// ===========================================================================

#[test]
fn treasury_basics() {
    let d = cards::def(id::TREASURY);
    assert_eq!((d.cost, d.vp), (5, 0));
}

#[test]
fn treasury_gives_a_card_action_and_coin() {
    let mut g = new_state(&[id::TREASURY], 2);
    set_hand(&mut g, 0, &[id::TREASURY]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::TREASURY);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

// Every test below that answers Treasury's offer drives the game all the way through cleanup
// (there's no way to pause mid-cleanup), so a *bare* deck would immediately reshuffle whatever
// just got discarded (or draw back whatever just got topdecked) into the very next hand,
// masking the distinction between "declined" and "accepted". Padding the deck with 5 plain
// Coppers first means cleanup's 5-card draw is satisfied without ever touching the discard
// (declined case) while still drawing a topdecked Treasury immediately, since it's the very
// next card (accepted case) — making the two outcomes observably different.
fn pad_deck_with_five_coppers(g: &mut GameState, p: usize) {
    set_deck_known(g, p, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
}

#[test]
fn treasury_declined_discards_normally() {
    let mut g = new_state(&[id::TREASURY], 2);
    set_hand(&mut g, 0, &[id::TREASURY]);
    play(&mut g, id::TREASURY); // empty deck: nothing to draw yet
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    pad_deck_with_five_coppers(&mut g, 0);
    pass(&mut g); // end the Buy phase with nothing bought (buys still available)
    let d = expect_decision(&mut g); // Treasury's offer
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    choose(&mut g, Choice::No);
    assert!(g.players[0].discard.has(id::TREASURY), "discarded, and the 5 Coppers mean it's never reshuffled back in");
    assert!(!g.players[0].hand.has(id::TREASURY));
    assert!(!g.players[0].in_play.has(id::TREASURY));
}

#[test]
fn treasury_accepted_goes_onto_the_deck() {
    let mut g = new_state(&[id::TREASURY], 2);
    set_hand(&mut g, 0, &[id::TREASURY]);
    play(&mut g, id::TREASURY);
    pad_deck_with_five_coppers(&mut g, 0);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    choose(&mut g, Choice::Yes);
    // Topdecked ahead of the Coppers: drawn as the very next card, as part of this same
    // cleanup's draw (one of the 5 Coppers is left behind instead).
    assert!(g.players[0].hand.has(id::TREASURY), "topdecked, then drawn immediately by cleanup");
    assert_eq!(g.players[0].hand.get(id::COPPER), 4);
    assert_eq!(g.players[0].deck_known.counts().get(id::COPPER), 1, "the 5th Copper stays on the deck");
    assert!(!g.players[0].in_play.has(id::TREASURY));
}

#[test]
fn treasury_cannot_go_back_if_a_victory_card_was_bought() {
    let mut g = new_state(&[id::TREASURY], 2);
    set_hand(&mut g, 0, &[id::TREASURY]);
    g.turn.coins = 10;
    play(&mut g, id::TREASURY);
    pad_deck_with_five_coppers(&mut g, 0);
    buy(&mut g, id::ESTATE);
    // No offer at all: straight through cleanup into the next player's turn.
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].discard.has(id::TREASURY), "no YesNo was ever offered");
}

#[test]
fn treasury_can_go_back_after_a_victory_card_gained_in_the_action_phase() {
    // 2nd edition's broader "gained... in it [the Buy phase]": a Victory card gained during the
    // *Action* phase (Remodel, here) doesn't block the offer, only one gained during the Buy
    // phase itself.
    let mut g = new_state(&[id::TREASURY, id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::TREASURY, id::REMODEL, id::SILVER]);
    play(&mut g, id::TREASURY);
    play(&mut g, id::REMODEL); // auto-trashes the only card in hand (Silver): forced, no decision
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 5, filter: Filter::Any, dest: Dest::Discard, exact: false });
    choose(&mut g, Choice::Card(id::DUCHY));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    pad_deck_with_five_coppers(&mut g, 0);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck }, "an action-phase Victory gain doesn't block the offer");
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].hand.has(id::TREASURY));
}

#[test]
fn treasury_through_throne_room_offers_once_for_the_one_physical_copy() {
    let mut g = new_state(&[id::THRONE_ROOM, id::TREASURY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TREASURY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TREASURY));
    // 0 (Throne Room spent it) + 1 (1st resolution) + 1 (2nd resolution) = 2.
    assert_eq!((g.turn.actions, g.turn.coins), (2, 2));
    pad_deck_with_five_coppers(&mut g, 0);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].hand.has(id::TREASURY));
}

#[test]
fn treasury_offered_once_per_copy_in_play() {
    let mut g = new_state(&[id::TREASURY], 2);
    set_hand(&mut g, 0, &[id::TREASURY, id::TREASURY]);
    play(&mut g, id::TREASURY);
    play(&mut g, id::TREASURY);
    pad_deck_with_five_coppers(&mut g, 0);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    choose(&mut g, Choice::Yes);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].in_play.get(id::TREASURY), 0);
    // Both Treasuries drawn back as part of this same cleanup (topdecked ahead of the Coppers).
    assert_eq!(g.players[0].hand.get(id::TREASURY), 2);
    assert_eq!(g.players[0].hand.get(id::COPPER), 3);
}

// ===========================================================================
// Sea Chart — new in 2nd edition (not in the C++ suite): "+1 Card +1 Action. Reveal the top card
// of your deck. If you have a copy of it in play, put it into your hand; otherwise it stays on
// top, now known."
// ===========================================================================

#[test]
fn sea_chart_basics_and_vanilla_bonus() {
    let d = cards::def(id::SEA_CHART);
    assert_eq!((d.cost, d.vp), (3, 0));
    let mut g = new_state(&[id::SEA_CHART], 2);
    set_hand(&mut g, 0, &[id::SEA_CHART]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER]);
    play(&mut g, id::SEA_CHART);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 0));
    // +1 Card draws the Estate; the revealed top card is then the Copper.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn sea_chart_with_an_empty_deck_does_nothing_extra() {
    let mut g = new_state(&[id::SEA_CHART], 2);
    set_hand(&mut g, 0, &[id::SEA_CHART]);
    play(&mut g, id::SEA_CHART);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn sea_chart_reveals_and_draws_a_duplicate_of_itself() {
    // Two Sea Charts stacked on the deck: the +1 Card draws the 1st into hand and plays nothing
    // new, so "a copy in play" still only means the just-played Sea Chart itself; the revealed
    // 2nd Sea Chart matches it and is drawn too.
    let mut g = new_state(&[id::SEA_CHART], 2);
    set_hand(&mut g, 0, &[id::SEA_CHART]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::SEA_CHART]);
    play(&mut g, id::SEA_CHART);
    // +1 Card draws the Estate; the revealed top card (2nd Sea Chart) matches the one in play.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::SEA_CHART]));
    assert!(g.players[0].deck_known.is_empty());
}

#[test]
fn sea_chart_leaves_a_non_matching_card_on_top_now_known() {
    let mut g = new_state(&[id::SEA_CHART], 2);
    set_hand(&mut g, 0, &[id::SEA_CHART]);
    set_deck_unknown(&mut g, 0, &[id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::GOLD]);
    play(&mut g, id::SEA_CHART);
    // +1 Card draws the Estate; Gold is revealed (not a copy of anything in play) and stays on
    // top, now known; the Duchy remains unknown beneath it.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert_eq!(g.players[0].deck_unknown, counts_of(&[id::DUCHY]));
}

#[test]
fn sea_chart_through_throne_room_finds_itself_in_play_the_second_time() {
    // 1st resolution: +1 Card draws the 2nd Sea Chart; the revealed card (a 3rd Sea Chart) has
    // no match yet (only one physical Sea Chart is in play) — wait: the just-played copy IS
    // already in play, so it always matches. Kept simple: two distinct non-Sea-Chart draws, then
    // a 3rd Sea Chart revealed on the 2nd resolution, which *does* match (the physical copy has
    // been in play since the 1st resolution).
    let mut g = new_state(&[id::THRONE_ROOM, id::SEA_CHART], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SEA_CHART]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::SEA_CHART]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SEA_CHART));
    // 1st resolution: +1 Card draws the Estate; reveals the Duchy (no match) -> stays on top.
    // 2nd resolution: +1 Card draws the Duchy (now known, on top); reveals the Sea Chart, which
    // matches the copy in play -> drawn too.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::SEA_CHART]));
    assert!(g.players[0].deck_known.is_empty());
}

// ===========================================================================
// Cross-cutting: Throne Room / King's Court on other step-4 cards, Island/Native Village VP and
// ownership counting, `determinize` hiding the Native Village mat, and text round trips.
// ===========================================================================

#[test]
fn kings_court_on_salvager_trashes_three_times() {
    let mut g = new_state(&[id::KINGS_COURT, id::SALVAGER], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::SALVAGER, id::GOLD, id::GOLD, id::GOLD]);
    play(&mut g, id::KINGS_COURT);
    choose(&mut g, Choice::Card(id::SALVAGER));
    // All 3 resolutions each mandatorily trash a Gold (only Golds left in hand): auto-single all
    // the way through.
    assert_eq!(g.turn.buys, 4); // 1 (start) + 3 (one per Salvager resolution)
    assert_eq!(g.turn.coins, 18); // 3 Golds trashed at $6 each
    assert_eq!(g.trash.get(id::GOLD), 3);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn island_and_native_village_mats_both_count_for_vp_and_all_cards() {
    let mut g = new_state(&[id::ISLAND, id::NATIVE_VILLAGE], 2);
    g.players[0].island_mat.add(id::ISLAND, 1);
    g.players[0].island_mat.add(id::DUCHY, 1);
    g.players[0].native_village_mat.add(id::PROVINCE, 1);
    g.players[0].native_village_mat.add(id::COPPER, 1);
    assert_eq!(g.players[0].vp(), 2 + 3 + 6); // Island + Duchy + Province
    let all = g.players[0].all_cards();
    assert_eq!(all.total(), 4);
    assert!(all.has(id::ISLAND) && all.has(id::DUCHY) && all.has(id::PROVINCE) && all.has(id::COPPER));
}

#[test]
fn determinize_hides_the_native_village_mat_but_not_its_size_or_the_island_mat() {
    let mut g = new_state(&[id::NATIVE_VILLAGE, id::ISLAND], 2);
    set_hand(&mut g, 1, &[id::COPPER]);
    g.players[1].deck_unknown = counts_of(&[id::COPPER, id::SILVER, id::GOLD]);
    g.players[1].native_village_mat.add(id::ESTATE, 1);
    g.players[1].native_village_mat.add(id::DUCHY, 1);
    g.players[1].island_mat.add(id::ISLAND, 1);
    g.players[1].island_mat.add(id::PROVINCE, 1);
    let view = PlayerView::new(&g, 0);
    let mut rng = Rng::new(7);
    for _ in 0..20 {
        let det = view.determinize(&mut rng);
        assert_eq!(det.players[1].all_cards().total(), g.players[1].all_cards().total());
        assert_eq!(det.players[1].native_village_mat.total(), 2, "the mat's size is public");
        // Island's mat is public: contents are preserved exactly, unlike Native Village's.
        assert_eq!(det.players[1].island_mat.counts(), counts_of(&[id::ISLAND, id::PROVINCE]));
    }
}

#[test]
fn text_format_round_trips_the_mats_and_smugglers_gain_record() {
    let mut g = new_state(&[id::NATIVE_VILLAGE, id::ISLAND, id::SMUGGLERS], 2);
    g.players[0].native_village_mat.add(id::GOLD, 1);
    g.players[0].native_village_mat.add(id::ESTATE, 2);
    g.players[0].island_mat.add(id::ISLAND, 1);
    g.players[0].island_mat.add(id::DUCHY, 1);
    g.players[1].last_turn_gains.add(id::SILVER, 1);
    g.players[1].last_turn_gains.add(id::VILLAGE, 1);
    let text = format_state(&g);
    assert!(text.contains("native village:"), "{text}");
    assert!(text.contains("island:"), "{text}");
    assert!(text.contains("last turn gains:"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].native_village_mat.counts(), counts_of(&[id::GOLD, id::ESTATE, id::ESTATE]));
    assert_eq!(back.players[0].island_mat.counts(), counts_of(&[id::ISLAND, id::DUCHY]));
    assert_eq!(back.players[1].last_turn_gains.counts(), counts_of(&[id::SILVER, id::VILLAGE]));
}

#[test]
fn old_state_text_without_step_4_fields_still_parses() {
    let text = "players: 2\nkingdom: Village, Smithy\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Village\ndeck top:\ndeck: 5 Copper\ndiscard:\nin play:\n\n[player 2]\nhand:\ndeck top:\ndeck: 5 Copper\ndiscard:\nin play:\n";
    let s = parse_state(text).unwrap();
    assert!(s.players[0].native_village_mat.is_empty());
    assert!(s.players[0].island_mat.is_empty());
    assert!(s.players[0].last_turn_gains.is_empty());
}

// ===========================================================================
// The full Seaside set is now implemented.
// ===========================================================================

#[test]
fn all_27_seaside_cards_are_ready() {
    assert_eq!(cards::kingdom_cards_in(CardSet::Seaside).count(), 27);
}






#[test]
fn gaining_more_distinct_cards_than_the_smugglers_record_holds_does_not_crash() {
    // The per-turn gain record is capped; overflowing it must only narrow Smugglers' choices.
    let kingdom = [id::SMUGGLERS, id::VILLAGE, id::SMITHY, id::MARKET, id::CELLAR, id::MOAT, id::WORKSHOP, id::FESTIVAL, id::LIBRARY, id::MINE];
    let mut g = new_state(&kingdom, 2);
    set_hand(&mut g, 0, &[]);
    expect_decision(&mut g);
    g.turn.buys = 12;
    g.turn.coins = 200;
    for c in [id::COPPER, id::SILVER, id::ESTATE, id::VILLAGE, id::SMITHY, id::MARKET, id::CELLAR, id::MOAT, id::WORKSHOP, id::FESTIVAL, id::LIBRARY] {
        buy(&mut g, c);
    }
    assert_eq!(g.players[0].all_cards().get(id::LIBRARY), 1, "every buy still happened");
}

// ===========================================================================
// Throne Room / King's Court with Durations, end to end (current rules):
// - the target is played twice (three times): each play's next-turn effect happens;
// - the multiplier stays in play with the Duration if any of its plays keeps the Duration in
//   play; both are discarded at the cleanup of the turn the Duration finishes;
// - Throne Room on Throne Room plays two different cards twice each.
// ===========================================================================

/// Finish the current player's turn and the opponent's (passing every optional choice), so
/// player 0's next turn starts and its start-of-turn Duration effects resolve. Returns once
/// player 0 has a decision on that turn.
fn to_my_next_turn(g: &mut GameState) {
    let start = g.turn.number;
    loop {
        let d = expect_decision(g);
        if g.turn.player == 0 && g.turn.number > start + 1 {
            let _ = d;
            return;
        }
        let cs = choices(g);
        let c = if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] };
        choose(g, c);
    }
}

#[test]
fn throne_room_wharf_doubles_now_and_next_turn_and_both_stay_until_then() {
    let mut g = new_state(&[id::THRONE_ROOM, id::WHARF], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::WHARF]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 20]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::WHARF));
    assert_eq!(g.players[0].hand.get(id::ESTATE), 4, "+2 Cards twice");
    assert_eq!(g.turn.buys, 3, "+1 Buy twice");
    to_my_next_turn(&mut g);
    // Both stayed in play through cleanup and the opponent's turn.
    assert!(g.players[0].in_play.has(id::THRONE_ROOM) && g.players[0].in_play.has(id::WHARF));
    // Next turn: 5-card hand + 2 Cards twice, +1 Buy twice.
    assert_eq!(g.players[0].hand.total(), 5 + 4);
    assert_eq!(g.turn.buys, 3);
    assert_eq!(g.players[0].pending_durations_len, 0);
    // Both are discarded at this turn's cleanup.
    to_my_next_turn(&mut g);
    assert!(!g.players[0].in_play.has(id::THRONE_ROOM) && !g.players[0].in_play.has(id::WHARF));
}

#[test]
fn throne_room_haven_with_one_card_keeps_throne_room_until_haven_returns_its_card() {
    // Empty deck: the 1st Haven play sets aside the Gold (the only card); the 2nd has nothing to
    // set aside. One play keeps Haven in play, so Throne Room stays too.
    let mut g = new_state(&[id::THRONE_ROOM, id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAVEN, id::GOLD]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HAVEN));
    assert_eq!(g.players[0].set_aside, counts_of(&[id::GOLD]));
    assert_eq!(g.players[0].pending_durations_len, 1);
    to_my_next_turn(&mut g);
    assert!(g.players[0].in_play.has(id::THRONE_ROOM), "Throne Room stays with Haven");
    // Haven returned the Gold to hand at the start of the turn (it then auto-played as a
    // Treasure on reaching the Buy phase).
    assert!(g.players[0].set_aside.is_empty());
    assert!(g.players[0].hand.has(id::GOLD) || g.players[0].in_play.has(id::GOLD), "Haven returned the set-aside Gold");
    to_my_next_turn(&mut g);
    assert!(!g.players[0].in_play.has(id::THRONE_ROOM) && !g.players[0].in_play.has(id::HAVEN));
}

#[test]
fn throne_room_on_a_duration_that_does_nothing_both_times_discards_both() {
    let mut g = new_state(&[id::THRONE_ROOM, id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAVEN]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HAVEN));
    to_my_next_turn(&mut g);
    assert!(!g.players[0].in_play.has(id::THRONE_ROOM) && !g.players[0].in_play.has(id::HAVEN));
}

#[test]
fn kings_court_merchant_ship_gives_six_now_and_six_next_turn() {
    let mut g = new_state(&[id::KINGS_COURT, id::MERCHANT_SHIP], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::MERCHANT_SHIP]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 20]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    play(&mut g, id::KINGS_COURT);
    choose(&mut g, Choice::Card(id::MERCHANT_SHIP));
    assert_eq!(g.turn.coins, 6);
    to_my_next_turn(&mut g);
    assert!(g.players[0].in_play.has(id::KINGS_COURT) && g.players[0].in_play.has(id::MERCHANT_SHIP));
    assert_eq!(g.turn.coins, 6, "+$2 three times at the start of the next turn");
    to_my_next_turn(&mut g);
    assert!(!g.players[0].in_play.has(id::KINGS_COURT));
}

#[test]
fn throne_room_on_throne_room_on_two_durations_keeps_both_throne_rooms() {
    // Throne Room plays Throne Room twice: Caravan twice, then Wharf twice. Both Throne Rooms
    // played Durations that stay, so all four cards stay; next turn: Caravan +1 Card x2, Wharf
    // +2 Cards +1 Buy x2.
    let mut g = new_state(&[id::THRONE_ROOM, id::CARAVAN, id::WHARF], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::THRONE_ROOM, id::CARAVAN, id::WHARF]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 30]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::THRONE_ROOM));
    choose(&mut g, Choice::Card(id::CARAVAN));
    choose(&mut g, Choice::Card(id::WHARF));
    assert_eq!(g.turn.played.get(id::CARAVAN), 2);
    assert_eq!(g.turn.played.get(id::WHARF), 2);
    to_my_next_turn(&mut g);
    let ip = g.players[0].in_play;
    assert_eq!((ip.get(id::THRONE_ROOM), ip.get(id::CARAVAN), ip.get(id::WHARF)), (2, 1, 1));
    assert_eq!(g.players[0].hand.total(), 5 + 2 + 4);
    assert_eq!(g.turn.buys, 3);
}

#[test]
fn throne_room_on_throne_room_on_a_duration_and_a_non_duration_keeps_one_throne_room() {
    // Throne Room plays Throne Room twice: Caravan twice (a Duration: the Throne Room that played
    // it stays), then Village twice (not a Duration). Only one Throne Room stays.
    let mut g = new_state(&[id::THRONE_ROOM, id::CARAVAN, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::THRONE_ROOM, id::CARAVAN, id::VILLAGE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 30]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::THRONE_ROOM));
    choose(&mut g, Choice::Card(id::CARAVAN));
    choose(&mut g, Choice::Card(id::VILLAGE));
    // 1 action spent on the first Throne Room; Caravan twice (+1 each), Village twice (+2 each).
    assert_eq!(g.turn.actions, 0 + 2 + 4);
    to_my_next_turn(&mut g);
    let ip = g.players[0].in_play;
    assert_eq!((ip.get(id::THRONE_ROOM), ip.get(id::CARAVAN), ip.get(id::VILLAGE)), (1, 1, 0));
    assert_eq!(g.players[0].hand.total(), 5 + 2);
}
