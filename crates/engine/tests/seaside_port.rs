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
fn tactician_through_throne_room_only_holds_if_activated_every_time() {
    // Adapted from TestTactitianThroneRoom (1290, 1st edition): in this engine's simpler,
    // documented rule (see `state::TurnState::multiplier_card`), the *multiplier* only stays in
    // play if every one of its resolutions actually scheduled the target's next-turn effect;
    // Tactician itself always stays once at least one resolution had a card to discard (tracked
    // separately, by the physical play, not per resolution).
    let mut g = new_state(&[id::THRONE_ROOM, id::TACTICIAN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TACTICIAN, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TACTICIAN));
    // 1st resolution: hand has the Estate, discards it, schedules. 2nd resolution: hand is
    // already empty, does nothing, does not schedule.
    assert!(g.players[0].discard.has(id::ESTATE));
    assert!(g.turn.duration_held.has(id::TACTICIAN), "Tactician itself stays (activated once)");
    assert!(!g.turn.duration_held.has(id::THRONE_ROOM), "Throne Room discards (not activated every time)");
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn tactician_through_throne_room_can_never_activate_both_times() {
    // Unlike Haven/Blockade (each resolution acts on a fresh, independent pick), Tactician's
    // condition is "if you have a card in hand" and its effect discards the *whole* hand: the
    // 1st resolution (if it activates at all) always empties the hand, so the 2nd can never also
    // activate. Throne Room therefore never stays in play when multiplying Tactician, even with
    // plenty of cards.
    let mut g = new_state(&[id::THRONE_ROOM, id::TACTICIAN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TACTICIAN, id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::TACTICIAN));
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
    assert!(g.turn.duration_held.has(id::TACTICIAN) && !g.turn.duration_held.has(id::THRONE_ROOM));
    assert_eq!(g.players[0].pending_durations_len, 1);
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
fn haven_via_throne_room_with_one_card_keeps_haven_but_not_throne_room() {
    // TestHavenThroneRoom, "1 card" case: only the 1st resolution has a card to set aside.
    let mut g = new_state(&[id::THRONE_ROOM, id::HAVEN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAVEN, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    // Only one card in hand throughout: every SetAside pick is auto-single.
    choose(&mut g, Choice::Card(id::HAVEN));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].set_aside, counts_of(&[id::ESTATE]));
    assert!(g.turn.duration_held.has(id::HAVEN), "Haven stays (activated once)");
    assert!(!g.turn.duration_held.has(id::THRONE_ROOM), "Throne Room discards (not activated every time)");
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





