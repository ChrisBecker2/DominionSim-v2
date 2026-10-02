//! Promo card tests: Prince. "You may set this aside. If you do, set aside an Action card from
//! your hand costing up to $4. At the start of each of your turns, play that Action, setting it
//! aside again when you discard it from play. (Stop playing it if you fail to set it aside on a
//! turn you play it.)"
//!
//! Rulings built here (see `docs/promo-plan.md`):
//! - Setting the Prince aside is optional; with no Action costing up to $4 in hand (current
//!   cost: Bridge counts; no Potion cost), the Prince is still set aside, holding nothing.
//! - Each Prince holds one card. The Princes and their cards stay the player's (VP, Gardens).
//! - The held card is played at the start of each of the owner's turns, as one of the
//!   start-of-turn effects (ordered with the others), without using an Action.
//! - When that card is discarded from play it is set aside with its Prince again. If it leaves
//!   play some other way (trashed) or stays in play (a Duration), the Prince holds nothing from
//!   then on.
//! - Throne Room on a Prince: the second play finds it no longer in play and does nothing.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

/// Play out the rest of the turn (passing every Action and Buy decision) until the next turn has
/// started, i.e. cleanup has happened. Stops at the first decision of the next turn.
fn finish_turn(g: &mut GameState) {
    finish_turn_ev(g, &mut NoEvents);
}

/// `finish_turn`, recording the events (including the next turn's start-of-turn effects).
fn finish_turn_ev<S: EventSink>(g: &mut GameState, sink: &mut S) {
    let start = g.turn.number;
    loop {
        match g.advance(sink) {
            Step::Decision(d) => {
                if g.turn.number != start {
                    return;
                }
                assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "unexpected {d:?}");
                g.apply(Choice::Pass, sink).unwrap();
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

/// A distinct, non-Action, non-Treasure deck: every draw is visible and nothing is auto-played.
const JUNK: [CardId; 6] = [id::ESTATE, id::DUCHY, id::PROVINCE, id::ESTATE, id::DUCHY, id::PROVINCE];

fn count_of(g: &GameState, c: CardId) -> u8 {
    g.players[0].all_cards().get(c)
}

// ===========================================================================
// The card
// ===========================================================================

#[test]
fn prince_is_a_promo_action_costing_eight() {
    let d = cards::def(id::PRINCE);
    assert_eq!((d.cost, d.vp), (8, 0));
    assert_eq!(cards::set_of(id::PRINCE), CardSet::Promo);
    assert!(cards::is(id::PRINCE, cards::ACTION) && !cards::is(id::PRINCE, cards::DURATION));
    assert!(cards::is_order_sensitive(id::PRINCE), "it takes an Action out of the hand");
    assert!(cards::is_kingdom(id::PRINCE) && cards::is_ready(id::PRINCE));
    assert_eq!(cards::set_by_name("Promo"), Some(CardSet::Promo));
    assert_eq!(cards::by_name("prince"), Some(id::PRINCE));
}

// ===========================================================================
// Setting aside
// ===========================================================================

#[test]
fn prince_sets_itself_and_an_action_costing_up_to_four_aside() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::VILLAGE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::SMITHY, id::VILLAGE, id::MARKET, id::SILVER]);
    let before = total_cards(&g);
    let step = play(&mut g, id::PRINCE);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::SetAside });
    assert_eq!(d.subject, id::PRINCE);
    assert_eq!(choices(&g), vec![Choice::Yes, Choice::No]);
    let step = choose(&mut g, Choice::Yes);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(
        d.kind,
        DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::ActionCostUpTo(4), min: 1, max: 1, ordered: false }
    );
    // Market costs $5: only the Village and the Smithy are offered.
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Card(id::SMITHY)]);
    choose(&mut g, Choice::Card(id::SMITHY));

    let ps = &g.players[0];
    assert_eq!(ps.prince_cards.get(id::SMITHY), 1);
    assert_eq!(ps.princes_empty, 0);
    assert!(!ps.in_play.has(id::PRINCE), "the Prince is set aside, not in play");
    assert!(ps.set_aside.is_empty());
    assert_eq!(ps.hand, counts_of(&[id::VILLAGE, id::MARKET]));
    assert!(ps.in_play.has(id::SILVER), "the Prince took the last Action, so the Buy phase started");
    // Both cards are still the player's own.
    assert_eq!((count_of(&g, id::PRINCE), count_of(&g, id::SMITHY)), (1, 1));
    assert_eq!(total_cards(&g), before);
}

#[test]
fn prince_may_stay_in_play_instead() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::SMITHY]);
    set_deck_known(&mut g, 0, &JUNK);
    play(&mut g, id::PRINCE);
    choose(&mut g, Choice::No);
    let ps = &g.players[0];
    assert!(ps.in_play.has(id::PRINCE));
    assert!(ps.prince_cards.is_empty() && ps.princes_empty == 0);
    assert!(ps.hand.has(id::SMITHY));
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::PRINCE), "discarded like any other Action");
    assert!(g.players[0].prince_cards.is_empty());
}

#[test]
fn prince_with_no_eligible_action_is_set_aside_holding_nothing() {
    let mut g = new_state(&[id::PRINCE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::MARKET, id::GOLD]);
    let before = total_cards(&g);
    play(&mut g, id::PRINCE);
    let step = choose(&mut g, Choice::Yes);
    // Nothing to pick: no second decision (the Market costs $5).
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "{d:?}");
    let ps = &g.players[0];
    assert_eq!((ps.prince_cards.total(), ps.princes_empty), (0, 1));
    assert!(!ps.in_play.has(id::PRINCE));
    assert_eq!(count_of(&g, id::PRINCE), 1, "still the player's card");
    assert_eq!(total_cards(&g), before);
}

#[test]
fn bridge_lowers_a_five_to_four_for_prince() {
    // Without Bridge the Market ($5) can't be held.
    let mut g = new_state(&[id::PRINCE, id::MARKET, id::BRIDGE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::MARKET]);
    play(&mut g, id::PRINCE);
    choose(&mut g, Choice::Yes);
    assert_eq!((g.players[0].prince_cards.total(), g.players[0].princes_empty), (0, 1));

    // With a Bridge in play it costs $4.
    let mut g = new_state(&[id::PRINCE, id::MARKET, id::BRIDGE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::BRIDGE, id::PRINCE, id::MARKET]);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::BRIDGE);
    assert_eq!(g.cost(id::MARKET), 4);
    play(&mut g, id::PRINCE);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].prince_cards.get(id::MARKET), 1, "the only eligible card is taken at once");
    assert_eq!(g.players[0].princes_empty, 0);
}

// ===========================================================================
// Start of turn
// ===========================================================================

#[test]
fn the_held_action_is_played_at_the_start_of_each_turn_without_using_an_action() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::VILLAGE], 2);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    // Smithy draws the three Estates/Duchy; the Village then waits in hand for a Play decision.
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::VILLAGE, id::PROVINCE]);
    let before = total_cards(&g);
    let mut ev: Vec<Event> = Vec::new();
    let Step::Decision(d) = g.advance(&mut ev) else { panic!() };
    assert_eq!(d.kind, DecisionKind::PlayAction, "the drawn Village can be played");
    assert!(ev.contains(&Event::StartPlay { player: 0, source: id::PRINCE, card: id::SMITHY }), "{ev:?}");
    assert_eq!(g.turn.actions, 1, "the Prince's play costs no Action");
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::VILLAGE]));
    assert!(g.players[0].in_play.has(id::SMITHY));
    assert!(g.turn.played.has(id::SMITHY));
    assert_eq!(g.turn.prince_played.get(id::SMITHY), 1);
    // The Prince keeps holding (an empty hold) while the card is in play.
    assert_eq!((g.players[0].prince_cards.total(), g.players[0].princes_empty), (0, 1));
    assert_eq!((count_of(&g, id::PRINCE), count_of(&g, id::SMITHY)), (1, 1));
    assert_eq!(total_cards(&g), before);
}

#[test]
fn the_card_is_set_aside_again_when_discarded_from_play() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    set_discard(&mut g, 0, &[id::COPPER; 6]);
    let before = total_cards(&g);
    let mut ev: Vec<Event> = Vec::new();
    g.advance(&mut ev);
    finish_turn(&mut g);
    let ps = &g.players[0];
    assert_eq!(ps.prince_cards.get(id::SMITHY), 1, "back with its Prince");
    assert_eq!(ps.princes_empty, 0);
    assert!(!ps.in_play.has(id::SMITHY) && !ps.discard.has(id::SMITHY), "not in the discard pile");
    assert_eq!(total_cards(&g), before);
    // ... and it plays again next turn.
    reset_turn(&mut g, 0);
    set_hand(&mut g, 0, &[]);
    set_deck_known(&mut g, 0, &JUNK);
    g.turn.prince_played = Default::default();
    let mut ev: Vec<Event> = Vec::new();
    g.advance(&mut ev);
    assert!(ev.contains(&Event::StartPlay { player: 0, source: id::PRINCE, card: id::SMITHY }));
}

#[test]
fn a_trashed_card_is_gone_and_the_prince_stops() {
    // Mining Village trashes itself when played ("you may trash this for +$2").
    let mut g = new_state(&[id::PRINCE, id::MINING_VILLAGE], 2);
    g.players[0].prince_cards.add(id::MINING_VILLAGE, 1);
    set_deck_known(&mut g, 0, &JUNK);
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Trash });
    choose(&mut g, Choice::Yes);
    assert!(g.trash.has(id::MINING_VILLAGE));
    assert_eq!(g.turn.coins, 2);
    finish_turn(&mut g);
    let ps = &g.players[0];
    assert_eq!((ps.prince_cards.total(), ps.princes_empty), (0, 1), "the Prince holds nothing now");
    assert_eq!(count_of(&g, id::PRINCE), 1, "but is still the player's card");
    assert!(!ps.discard.has(id::MINING_VILLAGE));
}

#[test]
fn a_prince_played_duration_stays_in_play_and_the_prince_stops() {
    // Ruling (unconfirmed against the wiki): a Duration played by a Prince stays in play through
    // its next turn, so it is not discarded from play at the first cleanup and isn't set aside
    // again; the Prince stops. At the next cleanup it is discarded normally.
    let mut g = new_state(&[id::PRINCE, id::CARAVAN], 2);
    g.players[0].prince_cards.add(id::CARAVAN, 1);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
    set_discard(&mut g, 0, &[id::DUCHY; 12]);
    g.advance(&mut NoEvents);
    assert!(g.players[0].in_play.has(id::CARAVAN));
    assert_eq!(g.players[0].pending_durations_len, 1, "Caravan's own next-turn draw is scheduled");
    finish_turn(&mut g);
    let ps = &g.players[0];
    assert!(ps.in_play.has(id::CARAVAN), "a Duration stays in play");
    assert_eq!((ps.prince_cards.total(), ps.princes_empty), (0, 1));

    // Player 1's turn passes; at player 0's next turn Caravan draws once and is not played again.
    let mut ev: Vec<Event> = Vec::new();
    finish_turn_ev(&mut g, &mut ev);
    assert_eq!(g.turn.player, 0);
    assert!(!ev.iter().any(|e| matches!(e, Event::StartPlay { .. })), "the Prince doesn't play it again");
    assert!(ev.iter().any(|e| matches!(e, Event::Bonus { source, .. } if *source == id::CARAVAN)));
    finish_turn(&mut g);
    let ps = &g.players[0];
    assert!(!ps.in_play.has(id::CARAVAN) && ps.discard.has(id::CARAVAN), "discarded at its second cleanup");
    assert_eq!((ps.prince_cards.total(), ps.princes_empty), (0, 1));
}

#[test]
fn throne_room_on_prince_sets_it_aside_only_once() {
    let mut g = new_state(&[id::THRONE_ROOM, id::PRINCE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::PRINCE, id::SMITHY, id::SILVER]);
    let before = total_cards(&g);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::PRINCE));
    let d = g.pending_decision().expect("first play of Prince");
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::SetAside });
    // The only Action in hand costing up to $4 is taken at once; the second play finds the
    // Prince gone, so it asks nothing and does nothing.
    let step = choose(&mut g, Choice::Yes);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(d.kind, DecisionKind::Buy, "no second offer");
    let ps = &g.players[0];
    assert_eq!(ps.prince_cards.get(id::SMITHY), 1);
    assert_eq!(ps.princes_empty, 0);
    assert_eq!(count_of(&g, id::PRINCE), 1);
    assert_eq!(total_cards(&g), before);
}

// ===========================================================================
// The order of the start-of-turn plays
// ===========================================================================

#[test]
fn a_princes_card_is_one_of_the_start_of_turn_effects() {
    // Prince's Smithy and a pending Sea Witch: Smithy first draws cards for Sea Witch's discard
    // to choose among, so the order is a real question, and the choice names the Smithy.
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::SEA_WITCH], 2);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    set_hand(&mut g, 0, &[id::COPPER, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    let i = g.players[0].pending_durations_len as usize;
    g.players[0].pending_durations[i] =
        dominion_engine::state::PendingDuration { card: id::SEA_WITCH, times: 1, arg: 0, used: false };
    g.players[0].pending_durations_len += 1;
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(choices(&g), vec![Choice::Card(id::SEA_WITCH), Choice::Card(id::SMITHY)]);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert!(g.players[0].in_play.has(id::SMITHY));
    assert_eq!(g.players[0].hand.total(), 2 + 3 + 2, "Smithy's 3 Cards, then Sea Witch's 2, before the discard");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }));
}

#[test]
fn two_princes_holding_the_same_card_both_play_it() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    g.players[0].prince_cards.add(id::SMITHY, 2);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::PROVINCE, id::ESTATE, id::DUCHY, id::PROVINCE]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Buy), "no question: the same effect twice: {d:?}");
    assert_eq!(g.players[0].hand.total(), 6);
    assert_eq!(g.players[0].in_play.get(id::SMITHY), 2);
    assert_eq!(g.players[0].princes_empty, 2);
    finish_turn(&mut g);
    assert_eq!(g.players[0].prince_cards.get(id::SMITHY), 2, "both go back to their Princes");
    assert_eq!(g.players[0].princes_empty, 0);
}

// ===========================================================================
// Counting, text, views
// ===========================================================================

#[test]
fn set_aside_princes_and_their_cards_count_for_gardens() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::GARDENS], 2);
    // Gardens + 7 Coppers = 8 cards: worth 0. A Prince holding a Smithy makes 10: worth 1.
    set_discard(&mut g, 0, &[id::GARDENS, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    assert_eq!(g.players[0].vp(), 0);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    assert_eq!(g.players[0].all_cards().total(), 10);
    assert_eq!(g.players[0].vp(), 1);
    // A Prince holding nothing is one card too.
    g.players[0].prince_cards.clear();
    g.players[0].princes_empty = 1;
    assert_eq!(g.players[0].all_cards().total(), 9);
    assert_eq!(g.players[0].vp(), 0);
}

#[test]
fn text_format_round_trips_princes() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::VILLAGE], 2);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    g.players[0].prince_cards.add(id::VILLAGE, 1);
    g.players[0].princes_empty = 2;
    g.turn.prince_played.add(id::SMITHY, 1);
    let text = format_state(&g);
    assert!(text.contains("prince: Village, Smithy, 2 nothing"), "{text}");
    assert!(text.contains("prince played: Smithy"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].prince_cards, g.players[0].prince_cards);
    assert_eq!(back.players[0].princes_empty, 2);
    assert_eq!(back.turn.prince_played, g.turn.prince_played);
    assert_eq!(back.players[0].all_cards(), g.players[0].all_cards());
    assert_eq!(count_of(&back, id::PRINCE), 4);
    // No Princes: no lines (Base-only saves are unchanged).
    let plain = format_state(&new_state(&[id::SMITHY], 2));
    assert!(!plain.contains("prince"), "{plain}");
    // Hand-written forms.
    let one = text.replace("prince: Village, Smithy, 2 nothing", "prince: Smithy, nothing");
    let back = parse_state(&one).unwrap();
    assert_eq!((back.players[0].prince_cards.total(), back.players[0].princes_empty), (1, 1));
    assert!(parse_state(&text.replace("prince: Village, Smithy, 2 nothing", "prince: Smithy, Bogus")).is_err());
}

#[test]
fn a_loaded_state_plays_its_princes_cards_at_the_start_of_the_turn() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    g.players[0].deck_unknown = counts_of(&[id::ESTATE; 6]);
    let mut back = parse_state(&format_state(&g)).unwrap();
    let mut ev: Vec<Event> = Vec::new();
    back.advance(&mut ev);
    assert!(ev.contains(&Event::StartPlay { player: 0, source: id::PRINCE, card: id::SMITHY }));
    assert_eq!(back.players[0].hand.total(), 3);
}

#[test]
fn princes_are_public_information() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    g.players[0].princes_empty = 1;
    let view = PlayerView::new(&g, 1);
    assert_eq!(view.prince_cards_of(0), counts_of(&[id::SMITHY]));
    assert_eq!(view.princes_empty_of(0), 1);
    assert_eq!(view.cards_of(0).get(id::PRINCE), 2);
    // Determinizing for another player keeps them.
    let world = view.determinize(&mut rng::Rng::new(1));
    assert_eq!(world.players[0].prince_cards, g.players[0].prince_cards);
    assert_eq!(world.players[0].princes_empty, 1);
}
