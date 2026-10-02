//! Promo card tests: Prince, from the official rules as provided by the user:
//!
//! - The turn you play Prince, you may set aside a non-Duration Action card from your hand costing
//!   $4 or less (with cost reduction, a card that normally costs more; never a Potion cost);
//!   then every turn after that you play that Action at the start of the turn.
//! - Without a card the Prince is discarded in Clean-up; with one it stays in play for the rest
//!   of the game.
//! - The set-aside card stays set aside and is played without entering play: if it tries to move
//!   itself (Mining Village, Treasure Map, Island) it fails to, and a bonus conditional on moving
//!   (Mining Village's +$2) is lost. It is mandatory and uses no Action. It isn't played on the
//!   turn it is set aside.
//! - With Throne Room two cards are set aside, and both the Throne Room and the Prince stay in
//!   play for the rest of the game (King's Court: up to three).
//! - It plays on extra turns (Outpost). At the end of the game the Prince and its card count.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

/// Play out the rest of the turn (passing every Action and Buy decision) until the next turn has
/// started, i.e. cleanup has happened; records the events (including the next turn's start).
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

fn finish_turn(g: &mut GameState) {
    finish_turn_ev(g, &mut NoEvents);
}

/// A deck of cards that are neither Treasures nor Actions: every draw is visible, nothing is
/// auto-played, and there are enough for several turns.
fn junk() -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(30).copied().collect()
}

fn count_of(g: &GameState, c: CardId) -> u8 {
    g.players[0].all_cards().get(c)
}

/// Player 0 owns a Prince in play (staying there) that holds `held`.
fn with_prince_holding(g: &mut GameState, held: &[CardId]) {
    let ps = &mut g.players[0];
    ps.in_play.add(id::PRINCE, 1);
    ps.prince_stays.add(id::PRINCE, 1);
    for &c in held {
        ps.prince_cards.add(c, 1);
    }
}

fn start_play(card: CardId) -> Event {
    Event::StartPlay { player: 0, source: id::PRINCE, card }
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
fn prince_sets_an_action_costing_up_to_four_aside_and_stays_in_play() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::VILLAGE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::SMITHY, id::VILLAGE, id::MARKET, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    let before = total_cards(&g);
    let step = play(&mut g, id::PRINCE);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(
        d.kind,
        DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::ActionCostUpTo(4), min: 0, max: 1, ordered: false }
    );
    // Market costs $5: only the Village and the Smithy are offered, and "may" allows a Pass.
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Card(id::SMITHY), Choice::Pass]);
    choose(&mut g, Choice::Card(id::SMITHY));

    let ps = &g.players[0];
    assert_eq!(ps.prince_cards.get(id::SMITHY), 1);
    assert!(ps.in_play.has(id::PRINCE), "the Prince stays in play");
    assert_eq!(ps.prince_stays.get(id::PRINCE), 1);
    assert!(ps.set_aside.is_empty());
    assert_eq!(ps.hand.get(id::SMITHY), 0);
    assert!(!g.turn.played.has(id::SMITHY), "the card is not played on the turn it is set aside");
    assert_eq!((count_of(&g, id::PRINCE), count_of(&g, id::SMITHY)), (1, 1));
    assert_eq!(total_cards(&g), before);

    // Clean-up keeps the Prince in play; the next turn's cleanup does too.
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::PRINCE) && !g.players[0].discard.has(id::PRINCE));
}

#[test]
fn a_prince_that_sets_nothing_aside_is_discarded_at_cleanup() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::PRINCE);
    choose(&mut g, Choice::Pass);
    assert!(g.players[0].prince_cards.is_empty() && g.players[0].prince_stays.is_empty());
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::PRINCE), "discarded like any other Action");
    assert!(!g.players[0].in_play.has(id::PRINCE));

    // No eligible card at all: nothing to decide, and the same.
    let mut g = new_state(&[id::PRINCE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::MARKET, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    let Step::Decision(d) = play(&mut g, id::PRINCE) else { panic!() };
    assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "{d:?}");
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::PRINCE));
}

#[test]
fn durations_and_potion_costs_can_never_be_set_aside() {
    // Caravan ($4) and Haven ($2) are Durations; Familiar costs $3 and a Potion.
    let mut g = new_state(&[id::PRINCE, id::CARAVAN, id::HAVEN, id::FAMILIAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::CARAVAN, id::HAVEN, id::FAMILIAR]);
    let Step::Decision(d) = play(&mut g, id::PRINCE) else { panic!() };
    assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "nothing eligible: {d:?}");
    assert!(g.players[0].prince_cards.is_empty());

    let mut g = new_state(&[id::PRINCE, id::CARAVAN, id::HAVEN, id::FAMILIAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::CARAVAN, id::HAVEN, id::FAMILIAR, id::SMITHY]);
    play(&mut g, id::PRINCE);
    assert_eq!(choices(&g), vec![Choice::Card(id::SMITHY), Choice::Pass], "only the Smithy qualifies");
}

#[test]
fn cost_reduction_lets_prince_take_a_card_that_normally_costs_more() {
    // Without Bridge the Market ($5) can't be set aside.
    let mut g = new_state(&[id::PRINCE, id::MARKET, id::BRIDGE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::MARKET]);
    let Step::Decision(d) = play(&mut g, id::PRINCE) else { panic!() };
    assert!(!matches!(d.kind, DecisionKind::Select { .. }));

    // With a Bridge in play it costs $4.
    let mut g = new_state(&[id::PRINCE, id::MARKET, id::BRIDGE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::BRIDGE, id::PRINCE, id::MARKET]);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::BRIDGE);
    assert_eq!(g.cost(id::MARKET), 4);
    play(&mut g, id::PRINCE);
    assert_eq!(choices(&g), vec![Choice::Card(id::MARKET), Choice::Pass]);
    choose(&mut g, Choice::Card(id::MARKET));
    assert_eq!(g.players[0].prince_cards.get(id::MARKET), 1);
}

#[test]
fn throne_room_on_prince_sets_two_aside_and_both_stay_in_play() {
    let mut g = new_state(&[id::THRONE_ROOM, id::PRINCE, id::SMITHY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::PRINCE, id::SMITHY, id::VILLAGE, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    let before = total_cards(&g);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::PRINCE));
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Card(id::SMITHY), Choice::Pass]);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Pass], "a second card for the second play");
    choose(&mut g, Choice::Card(id::VILLAGE));
    let ps = &g.players[0];
    assert_eq!(ps.prince_cards, {
        let mut m = dominion_engine::state::SmallMultiset::<8>::EMPTY;
        m.add(id::SMITHY, 1);
        m.add(id::VILLAGE, 1);
        m
    });
    assert_eq!((ps.prince_stays.get(id::PRINCE), ps.prince_stays.get(id::THRONE_ROOM)), (1, 1));
    assert_eq!(total_cards(&g), before);

    // Both stay in play, now and every later turn; both held cards play every turn.
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::PRINCE) && g.players[0].in_play.has(id::THRONE_ROOM));
    let mut ev: Vec<Event> = Vec::new();
    finish_turn_ev(&mut g, &mut ev); // player 1's turn, then player 0's next start
    // Two different cards to play: the order is asked.
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::StartOrder);
    g.apply(Choice::Card(id::SMITHY), &mut ev).unwrap();
    g.advance(&mut ev);
    assert!(ev.contains(&start_play(id::SMITHY)) && ev.contains(&start_play(id::VILLAGE)), "{ev:?}");
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::PRINCE) && g.players[0].in_play.has(id::THRONE_ROOM));
    assert!(g.players[0].discard.get(id::PRINCE) == 0 && g.players[0].discard.get(id::THRONE_ROOM) == 0);
}

#[test]
fn a_prince_that_set_nothing_aside_does_not_keep_the_throne_room() {
    let mut g = new_state(&[id::THRONE_ROOM, id::PRINCE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::PRINCE, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::PRINCE));
    choose(&mut g, Choice::Pass);
    choose(&mut g, Choice::Pass);
    assert!(g.players[0].prince_stays.is_empty());
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::PRINCE) && g.players[0].discard.has(id::THRONE_ROOM));
}

// ===========================================================================
// Start of turn
// ===========================================================================

#[test]
fn the_held_action_is_played_each_turn_without_entering_play_or_using_an_action() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::VILLAGE], 2);
    with_prince_holding(&mut g, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::VILLAGE, id::PROVINCE]);
    let before = total_cards(&g);
    let mut ev: Vec<Event> = Vec::new();
    let Step::Decision(d) = g.advance(&mut ev) else { panic!() };
    assert_eq!(d.kind, DecisionKind::PlayAction, "the drawn Village can be played");
    assert!(ev.contains(&start_play(id::SMITHY)), "{ev:?}");
    assert_eq!(g.turn.actions, 1, "the Prince's play costs no Action");
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::VILLAGE]));
    assert!(!g.players[0].in_play.has(id::SMITHY), "it never enters play");
    assert!(g.turn.played.has(id::SMITHY));
    assert_eq!(g.players[0].prince_cards.get(id::SMITHY), 1, "and never leaves the Prince");
    assert_eq!(total_cards(&g), before);
    assert!(!g.turn.prince_play);
    // It is still played the turn after.
    finish_turn(&mut g);
    assert_eq!(g.players[0].prince_cards.get(id::SMITHY), 1);
    assert!(!g.players[0].discard.has(id::SMITHY));
}

#[test]
fn it_is_played_on_an_outposts_extra_turn_too() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::OUTPOST], 2);
    with_prince_holding(&mut g, &[id::SMITHY]);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_known(&mut g, 0, &junk());
    let mut ev: Vec<Event> = Vec::new();
    g.advance(&mut ev);
    play_ev(&mut g, id::OUTPOST, &mut ev);
    finish_turn_ev(&mut g, &mut ev);
    assert_eq!(g.turn.player, 0, "Outpost: player 0 again");
    assert!(g.turn.is_extra_turn);
    let plays = ev.iter().filter(|e| **e == start_play(id::SMITHY)).count();
    assert_eq!(plays, 2, "the first turn and the extra turn");
    assert!(g.players[0].in_play.has(id::PRINCE));
}

#[test]
fn two_held_copies_of_a_card_are_both_played() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    with_prince_holding(&mut g, &[id::SMITHY, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Buy), "no question: the same effect twice: {d:?}");
    assert_eq!(g.players[0].hand.total(), 6);
    assert_eq!(g.players[0].prince_cards.get(id::SMITHY), 2);
}

#[test]
fn a_princes_card_is_one_of_the_start_of_turn_effects() {
    // Prince's Smithy and a pending Sea Witch: Smithy first draws cards for Sea Witch's discard
    // to choose among, so the order is a real question, and the choice names the Smithy.
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::SEA_WITCH], 2);
    with_prince_holding(&mut g, &[id::SMITHY]);
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
    assert_eq!(g.players[0].hand.total(), 2 + 3 + 2, "Smithy's 3 Cards, then Sea Witch's 2, before the discard");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }));
}

// ===========================================================================
// Cards that move themselves fail to, and never move a copy that is in play
// ===========================================================================

#[test]
fn a_held_mining_village_cannot_trash_itself_and_gives_no_bonus() {
    let mut g = new_state(&[id::PRINCE, id::MINING_VILLAGE], 2);
    with_prince_holding(&mut g, &[id::MINING_VILLAGE]);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    let d = expect_decision(&mut g);
    // No "trash it?" question for the held one: straight to the Action phase. Its +1 Card +2
    // Actions applied; the +$2 that depends on trashing did not.
    assert_eq!(d.kind, DecisionKind::PlayAction);
    assert_eq!((g.turn.actions, g.turn.coins), (3, 0));
    assert_eq!(g.players[0].prince_cards.get(id::MINING_VILLAGE), 1);
    assert!(g.trash.is_empty());

    // The copy in hand is an ordinary Mining Village: it can trash itself, and only it.
    play(&mut g, id::MINING_VILLAGE);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Trash });
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.players[0].prince_cards.get(id::MINING_VILLAGE), 1, "the held one is untouched");
    finish_turn(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    finish_turn_ev(&mut g, &mut ev);
    assert!(ev.contains(&start_play(id::MINING_VILLAGE)), "it keeps playing: {ev:?}");
}

#[test]
fn a_held_treasure_map_trashes_a_map_from_hand_but_gains_no_golds() {
    let mut g = new_state(&[id::PRINCE, id::TREASURE_MAP], 2);
    with_prince_holding(&mut g, &[id::TREASURE_MAP]);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    let golds = g.supply.get(id::GOLD);
    // The second Map (the only one in hand: no choice, so no decision) is trashed as printed;
    // "this" can't be trashed, so no Golds.
    expect_decision(&mut g);
    assert_eq!(g.trash.get(id::TREASURE_MAP), 1);
    assert_eq!(g.supply.get(id::GOLD), golds, "no Golds");
    assert_ne!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert_eq!(g.players[0].prince_cards.get(id::TREASURE_MAP), 1, "the held Map stays");
}

#[test]
fn a_held_island_stays_but_still_puts_a_hand_card_on_the_mat() {
    let mut g = new_state(&[id::PRINCE, id::ISLAND], 2);
    with_prince_holding(&mut g, &[id::ISLAND]);
    set_hand(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    let before = total_cards(&g);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::SetAside, .. }), "{d:?}");
    choose(&mut g, Choice::Card(id::DUCHY));
    let ps = &g.players[0];
    assert_eq!(ps.island_mat.get(id::DUCHY), 1);
    assert_eq!(ps.island_mat.get(id::ISLAND), 0, "Island itself did not move");
    assert_eq!(ps.prince_cards.get(id::ISLAND), 1);
    assert_eq!(total_cards(&g), before);
}

// ===========================================================================
// Counting, text, views
// ===========================================================================

#[test]
fn the_prince_and_its_card_count_for_gardens() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY, id::GARDENS], 2);
    // Gardens + 7 Coppers = 8 cards: worth 0. A Prince in play holding a Smithy makes 10: worth 1.
    set_discard(&mut g, 0, &[id::GARDENS, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    assert_eq!(g.players[0].vp(), 0);
    with_prince_holding(&mut g, &[id::SMITHY]);
    assert_eq!(g.players[0].all_cards().total(), 10);
    assert_eq!(g.players[0].vp(), 1);
}

#[test]
fn text_format_round_trips_the_holdings() {
    let mut g = new_state(&[id::PRINCE, id::THRONE_ROOM, id::SMITHY, id::VILLAGE], 2);
    with_prince_holding(&mut g, &[id::SMITHY, id::VILLAGE]);
    g.players[0].in_play.add(id::THRONE_ROOM, 1);
    g.players[0].prince_stays.add(id::THRONE_ROOM, 1);
    let text = format_state(&g);
    assert!(text.contains("prince: Village, Smithy"), "{text}");
    assert!(text.contains("prince stays: Throne Room, Prince"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].prince_cards, g.players[0].prince_cards);
    assert_eq!(back.players[0].prince_stays, g.players[0].prince_stays);
    assert_eq!(back.players[0].all_cards(), g.players[0].all_cards());
    assert_eq!(count_of(&back, id::PRINCE), 1);
    // No holdings: no lines (Base-only saves are unchanged).
    let plain = format_state(&new_state(&[id::SMITHY], 2));
    assert!(!plain.contains("prince"), "{plain}");
    // A hand-written `prince:` alone puts the Prince in play to stay.
    let one = "players: 2\nkingdom: Prince, Smithy\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
               [player 1]\nin play: Prince\nprince: Smithy\ndeck: 6 Estate\n\n[player 2]\nhand: 5 Copper\n";
    let loaded = parse_state(one).unwrap();
    assert_eq!(loaded.players[0].prince_stays.get(id::PRINCE), 1);
    assert!(parse_state(&one.replace("prince: Smithy", "prince: Bogus")).is_err());
}

#[test]
fn a_loaded_state_plays_its_princes_cards_at_the_start_of_the_turn() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    with_prince_holding(&mut g, &[id::SMITHY]);
    g.players[0].deck_unknown = counts_of(&[id::ESTATE; 6]);
    let mut back = parse_state(&format_state(&g)).unwrap();
    let mut ev: Vec<Event> = Vec::new();
    back.advance(&mut ev);
    assert!(ev.contains(&start_play(id::SMITHY)));
    assert_eq!(back.players[0].hand.total(), 3);
    finish_turn(&mut back);
    assert!(back.players[0].in_play.has(id::PRINCE), "the loaded Prince still stays in play");
}

#[test]
fn princes_cards_are_public_information() {
    let mut g = new_state(&[id::PRINCE, id::SMITHY], 2);
    with_prince_holding(&mut g, &[id::SMITHY]);
    let view = PlayerView::new(&g, 1);
    assert_eq!(view.prince_cards_of(0), counts_of(&[id::SMITHY]));
    assert_eq!(view.cards_of(0).get(id::PRINCE), 1);
    assert_eq!(view.cards_of(0).get(id::SMITHY), 1);
    // Determinizing for another player keeps them.
    let world = view.determinize(&mut rng::Rng::new(1));
    assert_eq!(world.players[0].prince_cards, g.players[0].prince_cards);
    assert_eq!(world.players[0].prince_stays, g.players[0].prince_stays);
}
