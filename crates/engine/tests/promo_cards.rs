//! Promo card tests: Prince, Walled Village, Envoy and Captain (rulings in `docs/promo-plan.md`). Prince, from
//! the official rules as provided by the user:
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
    ps.stays.add(id::PRINCE, 1);
    for &c in held {
        ps.prince_cards.add(c, 1);
    }
}

/// Player `p`'s hand plus the Treasures in their play area: the engine plays the Treasures in hand when the
/// Action phase ends, so a test of what a card put into the hand looks at both.
fn held(g: &GameState, p: usize) -> Counts {
    let mut c = g.players[p].hand;
    for (card, n) in g.players[p].in_play.iter() {
        if cards::is(card, cards::TREASURE) {
            c.add(card, n);
        }
    }
    c
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
    // Action-Duration-Command since the 2022 errata (see `prince_is_a_duration_that_schedules_nothing`).
    assert!(cards::is(id::PRINCE, cards::ACTION) && cards::is(id::PRINCE, cards::DURATION) && cards::is(id::PRINCE, cards::COMMAND));
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
        DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::CommandCostUpTo(4), min: 0, max: 1, ordered: false }
    );
    // Market costs $5: only the Village and the Smithy are offered, and "may" allows a Pass.
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Card(id::SMITHY), Choice::Pass]);
    choose(&mut g, Choice::Card(id::SMITHY));

    let ps = &g.players[0];
    assert_eq!(ps.prince_cards.get(id::SMITHY), 1);
    assert!(ps.in_play.has(id::PRINCE), "the Prince stays in play");
    assert_eq!(ps.stays.get(id::PRINCE), 1);
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
    assert!(g.players[0].prince_cards.is_empty() && g.players[0].stays.is_empty());
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
    assert_eq!((ps.stays.get(id::PRINCE), ps.stays.get(id::THRONE_ROOM)), (1, 1));
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
    assert!(g.players[0].stays.is_empty());
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
    assert_eq!(g.turn.led.cur, 0);
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
        dominion_engine::state::PendingDuration { card: id::SEA_WITCH, times: 1, arg: 0, used: 0 };
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
    g.players[0].stays.add(id::THRONE_ROOM, 1);
    let text = format_state(&g);
    assert!(text.contains("prince: Village, Smithy"), "{text}");
    assert!(text.contains("stays: Throne Room, Prince"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].prince_cards, g.players[0].prince_cards);
    assert_eq!(back.players[0].stays, g.players[0].stays);
    assert_eq!(back.players[0].all_cards(), g.players[0].all_cards());
    assert_eq!(count_of(&back, id::PRINCE), 1);
    // No holdings: no lines (Base-only saves are unchanged).
    let plain = format_state(&new_state(&[id::SMITHY], 2));
    assert!(!plain.contains("prince"), "{plain}");
    // A hand-written `prince:` alone puts the Prince in play to stay.
    let one = "players: 2\nkingdom: Prince, Smithy\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
               [player 1]\nin play: Prince\nprince: Smithy\ndeck: 6 Estate\n\n[player 2]\nhand: 5 Copper\n";
    let loaded = parse_state(one).unwrap();
    assert_eq!(loaded.players[0].stays.get(id::PRINCE), 1);
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
    assert_eq!(world.players[0].stays, g.players[0].stays);
}

// ===========================================================================
// Walled Village ($4): +1 Card, +2 Actions; at the start of Clean-up, if you have this and no more
// than one other Action card in play, you may put this onto your deck.
// ===========================================================================

/// Pass every Action and Buy decision until something else is asked (the Clean-up offer, a
/// Treasury or Alchemist question) or the turn is over (`None`).
fn to_cleanup_offer(g: &mut GameState) -> Option<Decision> {
    let start = g.turn.number;
    loop {
        match g.advance(&mut NoEvents) {
            Step::Decision(d) => {
                if g.turn.number != start {
                    return None;
                }
                if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) {
                    g.apply(Choice::Pass, &mut NoEvents).unwrap();
                } else {
                    return Some(d);
                }
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

fn is_topdeck_offer(d: &Decision, card: CardId) -> bool {
    d.kind == DecisionKind::YesNo { act: Act::Topdeck } && d.subject == card
}

#[test]
fn walled_village_is_a_four_cost_village_with_a_cleanup_offer() {
    let d = cards::def(id::WALLED_VILLAGE);
    assert_eq!((d.cost, d.cards, d.actions, d.buys, d.coins, d.vp), (4, 1, 2, 0, 0, 0));
    assert_eq!(cards::set_of(id::WALLED_VILLAGE), CardSet::Promo);
    assert!(cards::is(id::WALLED_VILLAGE, cards::ACTION) && !cards::is(id::WALLED_VILLAGE, cards::DURATION));
    assert!(cards::is_choice_free(id::WALLED_VILLAGE), "the topdeck is not a play decision");
    assert!(cards::is_kingdom(id::WALLED_VILLAGE) && cards::is_ready(id::WALLED_VILLAGE));
    assert_eq!(cards::by_name("walled village"), Some(id::WALLED_VILLAGE));
    let mut g = new_state(&[id::WALLED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WALLED_VILLAGE);
    assert_eq!((g.players[0].hand.total(), g.turn.actions), (1, 2));
}

#[test]
fn walled_village_alone_may_go_onto_the_deck_or_be_discarded() {
    for accept in [false, true] {
        let mut g = new_state(&[id::WALLED_VILLAGE], 2);
        set_hand(&mut g, 0, &[id::WALLED_VILLAGE]);
        set_deck_known(&mut g, 0, &junk());
        let before = total_cards(&g);
        play(&mut g, id::WALLED_VILLAGE);
        let d = to_cleanup_offer(&mut g).expect("offered at the start of Clean-up");
        assert!(is_topdeck_offer(&d, id::WALLED_VILLAGE), "{d:?}");
        assert_eq!(g.players[0].in_play.get(id::WALLED_VILLAGE), 1, "still in play when offered");
        assert_eq!(choices(&g), vec![Choice::Yes, Choice::No]);
        choose(&mut g, if accept { Choice::Yes } else { Choice::No });
        assert!(to_cleanup_offer(&mut g).is_none());
        let ps = &g.players[0];
        assert!(ps.in_play.is_empty());
        if accept {
            // Put on top of the deck, it is drawn into the next hand.
            assert!(ps.hand.has(id::WALLED_VILLAGE) && !ps.discard.has(id::WALLED_VILLAGE));
        } else {
            assert!(ps.discard.has(id::WALLED_VILLAGE) && !ps.hand.has(id::WALLED_VILLAGE));
        }
        assert_eq!(total_cards(&g), before);
    }
}

#[test]
fn walled_village_with_one_other_action_may_go_but_not_with_two() {
    // Village + Walled Village: two Actions in play.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::VILLAGE);
    play(&mut g, id::WALLED_VILLAGE);
    let d = to_cleanup_offer(&mut g).expect("two Actions in play: offered");
    assert!(is_topdeck_offer(&d, id::WALLED_VILLAGE));

    // Two Villages + Walled Village: three, so no offer, and it is discarded.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::VILLAGE, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::VILLAGE);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::WALLED_VILLAGE);
    assert!(to_cleanup_offer(&mut g).is_none(), "no question: too many Actions in play");
    assert!(g.players[0].discard.has(id::WALLED_VILLAGE) && g.players[0].in_play.is_empty());
}

#[test]
fn durations_still_in_play_count_as_actions_in_play() {
    // One Fishing Village from last turn is still in play: Fishing Village + Walled Village = 2.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::FISHING_VILLAGE], 2);
    set_in_play(&mut g, 0, &[id::FISHING_VILLAGE]);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WALLED_VILLAGE);
    let d = to_cleanup_offer(&mut g).expect("one Duration plus Walled Village: offered");
    assert!(is_topdeck_offer(&d, id::WALLED_VILLAGE));

    // Two of them: three Actions in play.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::FISHING_VILLAGE], 2);
    set_in_play(&mut g, 0, &[id::FISHING_VILLAGE, id::FISHING_VILLAGE]);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WALLED_VILLAGE);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert!(g.players[0].discard.has(id::WALLED_VILLAGE));

    // A Prince that stays in play is an Action in play too.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::PRINCE, id::FISHING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    g.players[0].in_play.add(id::PRINCE, 1);
    g.players[0].in_play.add(id::FISHING_VILLAGE, 1);
    g.players[0].stays.add(id::PRINCE, 1);
    play(&mut g, id::WALLED_VILLAGE);
    assert!(to_cleanup_offer(&mut g).is_none(), "Prince + Fishing Village + Walled Village");
}

#[test]
fn two_walled_villages_alone_may_both_go_onto_the_deck() {
    let mut g = new_state(&[id::WALLED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WALLED_VILLAGE);
    play(&mut g, id::WALLED_VILLAGE);
    let d = to_cleanup_offer(&mut g).expect("the first");
    assert!(is_topdeck_offer(&d, id::WALLED_VILLAGE));
    choose(&mut g, Choice::Yes);
    let d = to_cleanup_offer(&mut g).expect("the second: one Walled Village is left in play");
    assert!(is_topdeck_offer(&d, id::WALLED_VILLAGE));
    choose(&mut g, Choice::Yes);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert_eq!(g.players[0].hand.get(id::WALLED_VILLAGE), 2, "both are on top of the deck, so in the next hand");
    assert!(!g.players[0].discard.has(id::WALLED_VILLAGE));

    // One of them may go while the other is discarded.
    let mut g = new_state(&[id::WALLED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WALLED_VILLAGE);
    play(&mut g, id::WALLED_VILLAGE);
    to_cleanup_offer(&mut g).unwrap();
    choose(&mut g, Choice::No);
    to_cleanup_offer(&mut g).expect("still offered for the other copy");
    choose(&mut g, Choice::Yes);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert_eq!((g.players[0].hand.get(id::WALLED_VILLAGE), g.players[0].discard.get(id::WALLED_VILLAGE)), (1, 1));

    // Three of them: each would see two others.
    let mut g = new_state(&[id::WALLED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE, id::WALLED_VILLAGE, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    for _ in 0..3 {
        play(&mut g, id::WALLED_VILLAGE);
    }
    assert!(to_cleanup_offer(&mut g).is_none());
    assert_eq!(g.players[0].discard.get(id::WALLED_VILLAGE), 3);

    // Two of them and a Duration still in play from last turn: three Actions, so neither may go.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::FISHING_VILLAGE], 2);
    set_in_play(&mut g, 0, &[id::FISHING_VILLAGE]);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WALLED_VILLAGE);
    play(&mut g, id::WALLED_VILLAGE);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert_eq!(g.players[0].discard.get(id::WALLED_VILLAGE), 2);
}

#[test]
fn a_throne_roomed_walled_village_is_one_card_in_play() {
    let mut g = new_state(&[id::WALLED_VILLAGE, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::WALLED_VILLAGE));
    assert_eq!((g.players[0].hand.total(), g.turn.actions), (2, 4), "two plays: +2 Cards, +4 Actions");
    // Throne Room + Walled Village: two Actions in play; only the Walled Village is offered.
    let d = to_cleanup_offer(&mut g).expect("offered once");
    assert!(is_topdeck_offer(&d, id::WALLED_VILLAGE));
    choose(&mut g, Choice::Yes);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert!(g.players[0].discard.has(id::THRONE_ROOM));
    assert_eq!(g.players[0].hand.get(id::WALLED_VILLAGE), 1);
}

#[test]
fn walled_village_is_checked_before_treasury_may_leave_play() {
    // Treasury + Village + Walled Village: three Actions in play. Treasury's "when you discard this
    // from play" topdeck comes after Walled Village's start-of-Clean-up check, so it still counts.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::VILLAGE, id::TREASURY], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::TREASURY, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    for c in [id::VILLAGE, id::TREASURY, id::WALLED_VILLAGE] {
        play(&mut g, c);
    }
    let d = to_cleanup_offer(&mut g).expect("Treasury is asked");
    assert!(is_topdeck_offer(&d, id::TREASURY), "Walled Village was not offered: {d:?}");
    choose(&mut g, Choice::Yes);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert!(g.players[0].discard.has(id::WALLED_VILLAGE));
}

#[test]
fn a_topdecked_alchemist_makes_room_for_walled_village() {
    // Alchemist + Village + Walled Village = 3 Actions; Alchemist's start-of-Clean-up topdeck is
    // asked first (a Potion is in play), after which Walled Village has only one other Action.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::VILLAGE, id::ALCHEMIST], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::ALCHEMIST, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    g.players[0].in_play.add(id::POTION, 1);
    for c in [id::VILLAGE, id::ALCHEMIST, id::WALLED_VILLAGE] {
        play(&mut g, c);
    }
    let d = to_cleanup_offer(&mut g).expect("Alchemist first");
    assert!(is_topdeck_offer(&d, id::ALCHEMIST), "{d:?}");
    choose(&mut g, Choice::Yes);
    let d = to_cleanup_offer(&mut g).expect("now Walled Village");
    assert!(is_topdeck_offer(&d, id::WALLED_VILLAGE), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert_eq!((g.players[0].hand.get(id::ALCHEMIST), g.players[0].hand.get(id::WALLED_VILLAGE)), (1, 1));

    // Declining the Alchemist leaves three Actions: no Walled Village offer.
    let mut g = new_state(&[id::WALLED_VILLAGE, id::VILLAGE, id::ALCHEMIST], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::ALCHEMIST, id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    g.players[0].in_play.add(id::POTION, 1);
    for c in [id::VILLAGE, id::ALCHEMIST, id::WALLED_VILLAGE] {
        play(&mut g, c);
    }
    let d = to_cleanup_offer(&mut g).unwrap();
    assert!(is_topdeck_offer(&d, id::ALCHEMIST));
    choose(&mut g, Choice::No);
    assert!(to_cleanup_offer(&mut g).is_none());
    assert!(g.players[0].discard.has(id::WALLED_VILLAGE));
}

#[test]
fn walled_village_logs_the_topdeck() {
    let mut g = new_state(&[id::WALLED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WALLED_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WALLED_VILLAGE);
    to_cleanup_offer(&mut g).unwrap();
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Yes, &mut ev);
    assert!(ev.contains(&Event::Topdeck { player: 0, card: id::WALLED_VILLAGE }), "{ev:?}");
}

// ===========================================================================
// Envoy ($4): reveal the top 5 cards of your deck. The player to your left chooses one. Discard that
// one and put the rest into your hand.
// ===========================================================================

#[test]
fn envoy_reveals_five_and_the_left_player_chooses_the_one_to_discard() {
    let d = cards::def(id::ENVOY);
    assert_eq!((d.cost, d.cards, d.actions, d.buys), (4, 0, 0, 0));
    assert_eq!(cards::set_of(id::ENVOY), CardSet::Promo);
    assert!(cards::is(id::ENVOY, cards::ACTION) && !cards::is(id::ENVOY, cards::ATTACK));
    assert!(cards::is_order_sensitive(id::ENVOY) && cards::is_kingdom(id::ENVOY));
    let mut g = new_state(&[id::ENVOY], 3);
    set_hand(&mut g, 0, &[id::ENVOY, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER, id::DUCHY, id::CURSE, id::PLATINUM]);
    let before = total_cards(&g);
    let mut ev: Vec<Event> = Vec::new();
    let Step::Decision(d) = play_ev(&mut g, id::ENVOY, &mut ev) else { panic!() };
    assert_eq!((d.player, d.for_player), (1, 0), "the player to the left decides, for the Envoy's player");
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Discard, min: 1, max: 1, .. }));
    assert_eq!(
        choices(&g),
        vec![Choice::Card(id::COPPER), Choice::Card(id::SILVER), Choice::Card(id::GOLD), Choice::Card(id::DUCHY), Choice::Card(id::CURSE)],
        "only the five revealed cards, not the Platinum under them"
    );
    assert_eq!(ev.iter().filter(|e| matches!(e, Event::Reveal { player: 0, .. })).count(), 5);
    choose(&mut g, Choice::Card(id::GOLD));
    let ps = &g.players[0];
    assert_eq!(held(&g, 0), counts_of(&[id::ESTATE, id::SILVER, id::COPPER, id::DUCHY, id::CURSE]));
    assert_eq!(ps.discard, counts_of(&[id::GOLD]));
    assert_eq!(ps.deck_known.peek_top(), Some(id::PLATINUM));
    assert!(ps.set_aside.is_empty());
    assert_eq!(g.turn.actions, 0, "a terminal");
    assert_eq!(total_cards(&g), before);
}

#[test]
fn envoy_with_fewer_than_five_cards_reveals_what_there_is() {
    // One card: it is discarded and nothing joins the hand.
    let mut g = new_state(&[id::ENVOY], 2);
    set_hand(&mut g, 0, &[id::ENVOY, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::CURSE]);
    play(&mut g, id::ENVOY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::CURSE]));

    // Two: the opponent picks one of the two.
    let mut g = new_state(&[id::ENVOY], 2);
    set_hand(&mut g, 0, &[id::ENVOY]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD]);
    play(&mut g, id::ENVOY);
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Card(id::GOLD)]);
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!((held(&g, 0), g.players[0].discard), (counts_of(&[id::GOLD]), counts_of(&[id::SILVER])));

    // None at all (deck and discard empty): nothing happens.
    let mut g = new_state(&[id::ENVOY], 2);
    set_hand(&mut g, 0, &[id::ENVOY, id::ESTATE]);
    play(&mut g, id::ENVOY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn envoy_shuffles_the_discard_pile_in_to_reveal_the_rest() {
    // Three cards in the deck, two Coppers in the discard pile: the 5 revealed cards are the three on
    // top and, after the shuffle, the two Coppers. The card the opponent discards lands on the new
    // (empty) discard pile.
    let mut g = new_state(&[id::ENVOY], 2);
    set_hand(&mut g, 0, &[id::ENVOY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::ESTATE]);
    set_discard(&mut g, 0, &[id::COPPER, id::COPPER]);
    play(&mut g, id::ENVOY);
    assert_eq!(
        choices(&g),
        vec![Choice::Card(id::COPPER), Choice::Card(id::SILVER), Choice::Card(id::GOLD), Choice::Card(id::ESTATE)]
    );
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(held(&g, 0), counts_of(&[id::SILVER, id::ESTATE, id::COPPER, id::COPPER]));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]), "the discard pile was shuffled away into the deck");
    assert_eq!(g.players[0].deck_size(), 0);
}

#[test]
fn a_throne_roomed_envoy_reveals_ten_cards_in_two_goes() {
    let mut g = new_state(&[id::ENVOY, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::ENVOY]);
    set_deck_known(
        &mut g, 0,
        &[id::GOLD, id::SILVER, id::COPPER, id::DUCHY, id::ESTATE, id::CURSE, id::PLATINUM, id::PROVINCE, id::SILVER, id::SILVER, id::POTION],
    );
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::ENVOY));
    choose(&mut g, Choice::Card(id::GOLD));
    choose(&mut g, Choice::Card(id::PLATINUM));
    assert_eq!(
        held(&g, 0),
        counts_of(&[id::SILVER, id::COPPER, id::DUCHY, id::ESTATE, id::CURSE, id::PROVINCE, id::SILVER, id::SILVER])
    );
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD, id::PLATINUM]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::POTION));
}

// ===========================================================================
// Captain ($6, Action-Duration-Command): now and at the start of your next turn, play a non-Duration,
// non-Command Action card from the Supply costing up to $4, leaving it there.
// ===========================================================================

fn captain_kingdom() -> Vec<CardId> {
    vec![id::CAPTAIN, id::VILLAGE, id::SMITHY, id::MARKET, id::CARAVAN, id::FAMILIAR, id::BRIDGE, id::PRINCE]
}

fn supply_pick(g: &GameState) -> Decision {
    let d = g.pending_decision().expect("a decision");
    assert_eq!(
        d.kind,
        DecisionKind::Select { from: Zone::Supply, act: Act::Play, filter: Filter::CommandCostUpTo(4), min: 1, max: 1, ordered: false },
        "{d:?}"
    );
    d
}

fn sorted(mut v: Vec<Choice>) -> Vec<Choice> {
    v.sort_by_key(|c| format!("{c:?}"));
    v
}

/// Player 0 starts the next turn with a Captain in play and `times` plays pending.
fn with_pending_captain(g: &mut GameState, times: u8) {
    g.players[0].in_play.add(id::CAPTAIN, 1);
    let i = g.players[0].pending_durations_len as usize;
    g.players[0].pending_durations[i] = dominion_engine::state::PendingDuration { card: id::CAPTAIN, times, arg: 0, used: 0 };
    g.players[0].pending_durations_len += 1;
}

#[test]
fn captain_is_a_six_cost_action_duration_command_promo() {
    let d = cards::def(id::CAPTAIN);
    assert_eq!((d.cost, d.vp, d.cards, d.actions), (6, 0, 0, 0));
    assert_eq!(cards::set_of(id::CAPTAIN), CardSet::Promo);
    assert!(cards::is(id::CAPTAIN, cards::ACTION) && cards::is(id::CAPTAIN, cards::DURATION) && cards::is(id::CAPTAIN, cards::COMMAND));
    assert!(cards::is_order_sensitive(id::CAPTAIN) && cards::is_kingdom(id::CAPTAIN) && cards::is_ready(id::CAPTAIN));
    assert!(cards::start_order_sensitive(id::CAPTAIN), "its start-of-turn play is an order-sensitive effect");
    assert_eq!(cards::by_name("captain"), Some(id::CAPTAIN));
}

#[test]
fn captain_plays_a_card_from_the_supply_and_leaves_it_there() {
    let mut g = new_state(&captain_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CAPTAIN, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    let before = total_cards(&g);
    let village = g.supply.get(id::VILLAGE);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::CAPTAIN, &mut ev);
    let d = supply_pick(&g);
    assert_eq!(d.player, 0);
    // Smithy, Village and Bridge ($4 or less, non-Duration Actions); the Market costs $5, Caravan is a
    // Duration, Familiar costs a Potion, Prince is a Duration-Command.
    assert_eq!(
        sorted(choices(&g)),
        sorted(vec![Choice::Card(id::VILLAGE), Choice::Card(id::SMITHY), Choice::Card(id::BRIDGE)]),
        "no Pass: playing a card is not optional"
    );
    choose_ev(&mut g, Choice::Card(id::VILLAGE), &mut ev);
    assert!(ev.contains(&Event::StartPlay { player: 0, source: id::CAPTAIN, card: id::VILLAGE }), "{ev:?}");
    let ps = &g.players[0];
    assert_eq!(ps.hand.total(), 2, "Village's +1 Card");
    assert_eq!(g.turn.actions, 2, "Captain used the Action; Village gave 2, and the Supply play cost none");
    assert!(!ps.in_play.has(id::VILLAGE), "the Village is not in play");
    assert!(g.turn.played.has(id::VILLAGE));
    assert_eq!(g.supply.get(id::VILLAGE), village, "it stays in the Supply");
    assert!(ps.in_play.has(id::CAPTAIN));
    assert_eq!(total_cards(&g), before, "nothing moved");
    assert_eq!(ps.pending_durations_len, 1);
    assert_eq!(ps.pending_durations[0].card, id::CAPTAIN);
}

#[test]
fn captain_stays_in_play_and_plays_another_card_at_the_start_of_the_next_turn() {
    let mut g = new_state(&captain_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CAPTAIN);
    choose(&mut g, Choice::Card(id::VILLAGE));
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::CAPTAIN) && !g.players[0].discard.has(id::CAPTAIN), "Duration: still in play");
    let mut ev: Vec<Event> = Vec::new();
    finish_turn_ev(&mut g, &mut ev); // player 1's turn, then player 0's start
    supply_pick(&g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.actions, 1);
    // The same card again is fine if there is still a copy; a different one too.
    assert!(choices(&g).contains(&Choice::Card(id::VILLAGE)) && choices(&g).contains(&Choice::Card(id::SMITHY)));
    let hand_before = g.players[0].hand.total();
    choose_ev(&mut g, Choice::Card(id::SMITHY), &mut ev);
    assert!(ev.contains(&Event::StartPlay { player: 0, source: id::CAPTAIN, card: id::SMITHY }), "{ev:?}");
    assert_eq!(g.players[0].hand.total(), hand_before + 3);
    assert_eq!(g.turn.actions, 1, "no Action used");
    // The turn after, Captain is discarded like any Duration that has done its work.
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::CAPTAIN) && !g.players[0].in_play.has(id::CAPTAIN));
}

#[test]
fn captain_only_offers_visible_non_duration_non_command_actions_up_to_four() {
    // An empty pile is not there.
    let mut g = new_state(&captain_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    empty_pile(&mut g, id::SMITHY);
    play(&mut g, id::CAPTAIN);
    assert_eq!(sorted(choices(&g)), sorted(vec![Choice::Card(id::VILLAGE), Choice::Card(id::BRIDGE)]));

    // Cost reduction applies: with a Bridge played, the Market costs $4.
    let mut g = new_state(&captain_kingdom(), 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::BRIDGE, id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::VILLAGE);
    play(&mut g, id::BRIDGE);
    assert_eq!(g.cost(id::MARKET), 4);
    play(&mut g, id::CAPTAIN);
    assert!(choices(&g).contains(&Choice::Card(id::MARKET)));
    assert!(!choices(&g).contains(&Choice::Card(id::FAMILIAR)), "a Potion cost never qualifies");
    assert!(!choices(&g).contains(&Choice::Card(id::CARAVAN)) && !choices(&g).contains(&Choice::Card(id::PRINCE)));
}

#[test]
fn captain_with_nothing_to_play_still_stays_in_play() {
    // Only a $5 Action and a Duration in the kingdom: nothing qualifies.
    let mut g = new_state(&[id::CAPTAIN, id::MARKET, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    let Step::Decision(d) = play(&mut g, id::CAPTAIN) else { panic!() };
    assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "no question: {d:?}");
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::CAPTAIN), "it stays in play and tries again next turn");
    finish_turn(&mut g);
    // The next turn: nothing to play, the turn simply goes on, and the Captain is then discarded.
    assert!(g.players[0].in_play.has(id::CAPTAIN));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "{d:?}");
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::CAPTAIN));
}

#[test]
fn a_throne_roomed_captain_plays_two_cards_now_and_two_next_turn() {
    let kingdom: Vec<CardId> = captain_kingdom().into_iter().chain([id::THRONE_ROOM]).collect();
    let mut g = new_state(&kingdom, 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::CAPTAIN));
    supply_pick(&g);
    choose(&mut g, Choice::Card(id::VILLAGE));
    supply_pick(&g);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].hand.total(), 1 + 3, "Village, then Smithy");
    assert_eq!(g.players[0].pending_durations_len, 1);
    assert_eq!(g.players[0].pending_durations[0].times, 2);
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::THRONE_ROOM) && g.players[0].in_play.has(id::CAPTAIN), "the Throne Room stays with it");
    finish_turn(&mut g);
    supply_pick(&g);
    choose(&mut g, Choice::Card(id::VILLAGE));
    supply_pick(&g);
    choose(&mut g, Choice::Card(id::VILLAGE));
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::THRONE_ROOM) && g.players[0].discard.has(id::CAPTAIN));
}

#[test]
fn a_card_captain_plays_cannot_move_itself() {
    // Mining Village: it can't trash itself (it is in the Supply), and the +$2 that depends on it is lost.
    let mut g = new_state(&[id::CAPTAIN, id::MINING_VILLAGE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CAPTAIN);
    let n = g.supply.get(id::MINING_VILLAGE);
    let Step::Decision(d) = choose(&mut g, Choice::Card(id::MINING_VILLAGE)) else { panic!() };
    assert!(!matches!(d.kind, DecisionKind::YesNo { .. }), "no 'trash it?' question: {d:?}");
    assert_eq!((g.turn.coins, g.turn.actions), (0, 2));
    assert!(g.trash.is_empty());
    assert_eq!(g.supply.get(id::MINING_VILLAGE), n);

    // A card that gains: Workshop can gain itself, and the pile it was played from shrinks.
    let mut g = new_state(&[id::CAPTAIN, id::WORKSHOP, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CAPTAIN);
    choose(&mut g, Choice::Card(id::WORKSHOP));
    assert!(choices(&g).contains(&Choice::Card(id::WORKSHOP)));
    choose(&mut g, Choice::Card(id::WORKSHOP));
    assert_eq!(g.supply.get(id::WORKSHOP), 9);
    assert_eq!(g.players[0].discard.get(id::WORKSHOP), 1);

    // Island stays in the Supply, but the card from the hand still goes on the mat.
    let mut g = new_state(&[id::CAPTAIN, id::ISLAND], 2);
    set_hand(&mut g, 0, &[id::CAPTAIN, id::ESTATE, id::DUCHY]);
    play(&mut g, id::CAPTAIN);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::SetAside, .. }), "{d:?}");
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!((g.players[0].island_mat.get(id::ESTATE), g.players[0].island_mat.get(id::ISLAND)), (1, 0));
}

#[test]
fn captain_playing_throne_room_lets_a_duration_stay_longer() {
    // At the start of the next turn Captain plays a Throne Room (from the Supply); it plays the Caravan
    // in hand twice. The Caravan stays in play with both of its plays; the Captain is done.
    let mut g = new_state(&[id::CAPTAIN, id::THRONE_ROOM, id::CARAVAN, id::VILLAGE], 2);
    with_pending_captain(&mut g, 1);
    set_hand(&mut g, 0, &[id::CARAVAN]);
    set_deck_known(&mut g, 0, &junk());
    let before = total_cards(&g);
    adv(&mut g);
    supply_pick(&g);
    choose(&mut g, Choice::Card(id::THRONE_ROOM));
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Play, .. }), "{d:?}");
    choose(&mut g, Choice::Card(id::CARAVAN));
    assert_eq!(g.players[0].hand.total(), 2, "Caravan twice: +1 Card each");
    assert_eq!(g.turn.actions, 3, "Caravan's +1 Action twice; the Supply plays used none");
    assert!(!g.players[0].in_play.has(id::THRONE_ROOM), "the Throne Room is in the Supply, not in play");
    let entry = (0..g.players[0].pending_durations_len as usize)
        .map(|i| g.players[0].pending_durations[i])
        .find(|e| e.card == id::CARAVAN)
        .unwrap();
    assert_eq!(entry.times, 2);
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::CARAVAN) && !g.players[0].in_play.has(id::THRONE_ROOM));
    assert!(g.players[0].discard.has(id::CAPTAIN));
    assert_eq!(total_cards(&g), before);
}

#[test]
fn a_walled_village_captain_plays_is_not_in_play_to_be_topdecked() {
    let mut g = new_state(&[id::CAPTAIN, id::WALLED_VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CAPTAIN]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CAPTAIN);
    choose(&mut g, Choice::Card(id::WALLED_VILLAGE));
    assert_eq!(g.turn.actions, 2);
    // Captain (a Duration) stays in play; no Walled Village is in play, so there is no offer.
    assert!(to_cleanup_offer(&mut g).is_none());
    assert_eq!(g.supply.get(id::WALLED_VILLAGE), 10);
}

#[test]
fn captains_start_of_turn_play_is_ordered_with_a_princes_card() {
    // Captain's pick and a Prince's Smithy are both plays: the order is the player's.
    let mut g = new_state(&[id::CAPTAIN, id::PRINCE, id::SMITHY, id::VILLAGE], 2);
    with_prince_holding(&mut g, &[id::SMITHY]);
    with_pending_captain(&mut g, 1);
    set_hand(&mut g, 0, &[id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(sorted(choices(&g)), sorted(vec![Choice::Card(id::CAPTAIN), Choice::Card(id::SMITHY)]));
    choose(&mut g, Choice::Card(id::SMITHY));
    supply_pick(&g);
    assert_eq!(g.players[0].hand.total(), 1 + 3, "the Prince's Smithy was played first");
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::PlayAction | DecisionKind::Buy));
}

#[test]
fn captains_pending_play_survives_the_text_format() {
    let mut g = new_state(&[id::CAPTAIN, id::VILLAGE, id::SMITHY], 2);
    with_pending_captain(&mut g, 2);
    g.players[0].deck_unknown = counts_of(&[id::ESTATE; 8]);
    let text = format_state(&g);
    assert!(text.contains("durations: Captain x2"), "{text}");
    let mut back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].pending_durations[0].times, 2);
    // Loaded at the start of the turn, it asks for the two cards.
    let d = expect_decision(&mut back);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Supply, act: Act::Play, .. }), "{d:?}");
    choose(&mut back, Choice::Card(id::VILLAGE));
    assert!(matches!(back.pending_decision().unwrap().kind, DecisionKind::Select { from: Zone::Supply, .. }));
}

// ===========================================================================
// Governor, Dismantle, Church, Marchland and the Event Summon (wiki rulings in `docs/promo-plan.md` section 5)
// ===========================================================================

use dominion_engine::state::PendingDuration;

fn hand_of(g: &GameState, p: usize) -> Counts {
    g.players[p].hand
}

fn discard_of(g: &GameState, p: usize) -> Counts {
    g.players[p].discard
}

fn sorted_choices(g: &GameState) -> Vec<Choice> {
    sorted(choices(g))
}

#[test]
fn the_new_promo_cards_are_in_the_promo_set_with_their_printed_types_and_costs() {
    for (c, cost, ty) in [
        (id::GOVERNOR, 5, cards::ACTION),
        (id::DISMANTLE, 4, cards::ACTION),
        (id::CHURCH, 3, cards::ACTION | cards::DURATION),
        (id::MARCHLAND, 5, cards::VICTORY),
    ] {
        let d = cards::def(c);
        assert_eq!(d.cost, cost, "{}", d.name);
        assert_eq!(cards::set_of(c), CardSet::Promo, "{}", d.name);
        assert!(cards::is(c, ty), "{}", d.name);
        assert!(cards::is_kingdom(c) && cards::is_ready(c) && !cards::is_pseudo(c), "{} is a kingdom card", d.name);
        assert_eq!(cards::by_name(&d.name.to_lowercase()), Some(c));
    }
    assert!(!cards::is(id::MARCHLAND, cards::ACTION) && !cards::is(id::CHURCH, cards::VICTORY));
    assert_eq!(cards::def(id::CHURCH).actions, 1, "+1 Action");
    assert_eq!(cards::def(id::GOVERNOR).actions, 1, "+1 Action");
    // Summon is an Event: a pseudo-card for $5, never in a zone.
    assert!(cards::is_event(id::SUMMON) && cards::is_pseudo(id::SUMMON) && !cards::is_kingdom(id::SUMMON));
    assert_eq!((cards::cost(id::SUMMON), cards::set_of(id::SUMMON)), (5, CardSet::Promo));
    assert!(cards::pseudo_cards_of(CardSet::Promo).any(|c| c == id::SUMMON));
    assert_eq!(cards::by_name("summon"), Some(id::SUMMON));
    assert!((id::SUMMON as usize) >= cards::NUM_ZONE_IDS && (id::MARCHLAND as usize) < cards::NUM_ZONE_IDS);
    // Zone card ids stay below the `Counts` bookkeeping lanes.
    assert!(cards::NUM_ZONE_IDS < dominion_engine::counts::LANES - 4);
    // Governor, Dismantle and Church can take an Action out of the hand: order-sensitive. The start-of-turn parts of Church and Summon are plays /
    // hand changes, so they are ordered against other start-of-turn effects.
    for c in [id::GOVERNOR, id::DISMANTLE, id::CHURCH] {
        assert!(cards::is_order_sensitive(c), "{}", cards::name(c));
    }
    assert!(cards::start_order_sensitive(id::CHURCH) && cards::start_order_sensitive(id::SUMMON));
}

// ---------------------------------------------------------------------------
// Governor
// ---------------------------------------------------------------------------

#[test]
fn governor_draws_three_for_you_and_one_for_each_other_player() {
    let mut g = new_state(&[id::GOVERNOR], 3);
    set_hand(&mut g, 0, &[id::GOVERNOR, id::ESTATE]);
    for p in 0..3 {
        set_deck_known(&mut g, p, &junk());
    }
    play(&mut g, id::GOVERNOR);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1), Choice::Mode(2)]);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(hand_of(&g, 0).total(), 1 + 3, "+3 Cards (the Estate stays)");
    assert_eq!(hand_of(&g, 1).total(), 1);
    assert_eq!(hand_of(&g, 2).total(), 1);
    assert_eq!(g.turn.actions, 1, "+1 Action (one used to play it)");
}

#[test]
fn governor_gains_a_gold_for_you_and_a_silver_for_each_other_player_in_turn_order() {
    let mut g = new_state(&[id::GOVERNOR], 3);
    set_hand(&mut g, 0, &[id::GOVERNOR]);
    let (gold, silver) = (g.supply.get(id::GOLD), g.supply.get(id::SILVER));
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Mode(1), &mut ev);
    assert_eq!(discard_of(&g, 0), counts_of(&[id::GOLD]));
    assert_eq!(discard_of(&g, 1), counts_of(&[id::SILVER]));
    assert_eq!(discard_of(&g, 2), counts_of(&[id::SILVER]));
    assert_eq!((g.supply.get(id::GOLD), g.supply.get(id::SILVER)), (gold - 1, silver - 2));
    let gains: Vec<u8> = ev.iter().filter_map(|e| if let Event::Gain { player, .. } = e { Some(*player) } else { None }).collect();
    assert_eq!(gains, vec![0, 1, 2], "in turn order, starting with the Governor's player");
}

#[test]
fn governor_with_the_last_silver_only_the_first_player_in_turn_order_gets_it() {
    // Player 1 is the first to the Governor's player's left; the Governor's own player gains a Gold, which is another pile.
    let mut g = new_state(&[id::GOVERNOR], 3);
    set_hand(&mut g, 0, &[id::GOVERNOR]);
    set_supply(&mut g, id::SILVER, 1);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(discard_of(&g, 1), counts_of(&[id::SILVER]));
    assert!(discard_of(&g, 2).is_empty(), "the pile is empty: no gain, and no choice to make");
    assert!(discard_of(&g, 0).has(id::GOLD));
}

#[test]
fn governor_remodel_you_gain_exactly_two_more_others_exactly_one_more_and_they_decide_in_turn_order() {
    let mut g = new_state(&[id::GOVERNOR, id::SMITHY, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::GOVERNOR, id::SILVER, id::ESTATE]);
    set_hand(&mut g, 1, &[id::SMITHY, id::COPPER]);
    let before = total_cards(&g);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(2));
    // The Governor's player first: a card of the hand (may be none).
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.kind), (0, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false }));
    choose(&mut g, Choice::Card(id::SILVER));
    // Silver $3 + $2 = exactly $5: the Market and the Duchy, not the $4 Smithy.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 0);
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Card(id::DUCHY), Choice::Card(id::MARKET), Choice::Card(id::GOVERNOR)]));
    assert!(!choices(&g).contains(&Choice::Pass), "if you trash a card you must gain one");
    choose(&mut g, Choice::Card(id::MARKET));
    // Then the other player: Copper $0 + $1 = exactly $1; nothing costs $1 in this kingdom, so only the trash happens.
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.kind), (1, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false }));
    assert!(choices(&g).contains(&Choice::Pass), "they may decline");
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.trash.has(id::COPPER) && g.trash.has(id::SILVER));
    assert!(discard_of(&g, 1).is_empty());
    assert_eq!(total_cards(&g), before);
}

#[test]
fn governor_other_players_gain_exactly_one_more_and_may_decline() {
    let mut g = new_state(&[id::GOVERNOR, id::SMITHY, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::GOVERNOR]);
    set_hand(&mut g, 1, &[id::SMITHY, id::COPPER]);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(2));
    // Nothing in the Governor's hand: no choice is asked of them. The other player trashes the Smithy for an exactly-$5 card.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Card(id::DUCHY), Choice::Card(id::MARKET), Choice::Card(id::GOVERNOR)]));
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(discard_of(&g, 1).has(id::DUCHY));
    // Declining gains nothing.
    let mut g = new_state(&[id::GOVERNOR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GOVERNOR]);
    set_hand(&mut g, 1, &[id::SMITHY]);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(2));
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(hand_of(&g, 1), counts_of(&[id::SMITHY]));
    assert!(g.trash.is_empty());
}

#[test]
fn governor_cannot_trash_itself_but_can_trash_another_copy() {
    let mut g = new_state(&[id::GOVERNOR], 2);
    set_hand(&mut g, 0, &[id::GOVERNOR, id::GOVERNOR]);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(2));
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 0);
    assert_eq!(choices(&g), vec![Choice::Card(id::GOVERNOR), Choice::Pass], "only the other copy is in the hand");
    assert_eq!(g.players[0].in_play.get(id::GOVERNOR), 1);
}

#[test]
fn governor_exactly_one_more_keeps_the_potion_of_the_card_trashed() {
    // Apothecary $2P + $1 = exactly $3P (Familiar, Alchemist); a plain $3 card (Silver) is not "exactly $3P".
    let mut g = new_state(&[id::GOVERNOR, id::APOTHECARY, id::FAMILIAR, id::ALCHEMIST, id::GOLEM], 2);
    set_hand(&mut g, 0, &[id::GOVERNOR]);
    set_hand(&mut g, 1, &[id::APOTHECARY]);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(2));
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::APOTHECARY));
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Card(id::ALCHEMIST), Choice::Card(id::FAMILIAR)]));
}

#[test]
fn governor_exactly_more_ignores_cards_with_debt_in_their_cost() {
    // A Copper trashed with +$1: exactly $1. The Debt-only Engineer ($0 and 4 Debt) is not exactly $1.
    let mut g = new_state(&[id::GOVERNOR, id::ENGINEER, id::POOR_HOUSE], 2);
    set_hand(&mut g, 0, &[id::GOVERNOR]);
    set_hand(&mut g, 1, &[id::COPPER]);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(2));
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::COPPER));
    // The only card exactly $1 (a single choice is gained at once); the Engineer ($0 and 4 Debt) was not an option.
    assert_eq!(discard_of(&g, 1), counts_of(&[id::POOR_HOUSE]));
    assert_eq!(g.supply.get(id::ENGINEER), 10);
}

#[test]
fn a_throne_roomed_governor_gives_two_choices_and_the_choices_may_differ() {
    let mut g = new_state(&[id::GOVERNOR, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::GOVERNOR]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::GOVERNOR));
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(0));
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(hand_of(&g, 0).total(), 3);
    assert_eq!(hand_of(&g, 1).total(), 1);
    assert!(discard_of(&g, 0).has(id::GOLD) && discard_of(&g, 1).has(id::SILVER));
    assert_eq!(g.turn.actions, 2, "+1 Action twice, one used by the Throne Room");
}

// ---------------------------------------------------------------------------
// Dismantle
// ---------------------------------------------------------------------------

#[test]
fn dismantle_trashes_a_card_and_gains_a_cheaper_card_then_a_gold() {
    let mut g = new_state(&[id::DISMANTLE], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE, id::SILVER, id::COPPER]);
    let before = total_cards(&g);
    play(&mut g, id::DISMANTLE);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 1, max: 1, ordered: false });
    assert!(!choices(&g).contains(&Choice::Pass), "trashing is not optional");
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut ev);
    // Cheaper than $3: Copper, Curse, Estate.
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Card(id::COPPER), Choice::Card(id::CURSE), Choice::Card(id::ESTATE)]));
    choose_ev(&mut g, Choice::Card(id::ESTATE), &mut ev);
    assert_eq!(discard_of(&g, 0), counts_of(&[id::ESTATE, id::GOLD]));
    let gains: Vec<CardId> = ev.iter().filter_map(|e| if let Event::Gain { card, .. } = e { Some(*card) } else { None }).collect();
    assert_eq!(gains, vec![id::ESTATE, id::GOLD], "the cheaper card goes to the discard pile first");
    assert!(g.trash.has(id::SILVER));
    assert_eq!(total_cards(&g), before, "gains and the trash only move cards");
}

#[test]
fn dismantle_on_a_copper_or_an_empty_hand_does_nothing_more() {
    let mut g = new_state(&[id::DISMANTLE], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE, id::COPPER]);
    play(&mut g, id::DISMANTLE);
    // A single card in hand is trashed without a question.
    assert!(g.trash.has(id::COPPER));
    assert!(discard_of(&g, 0).is_empty(), "$0: no gain");
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::PlayAction | DecisionKind::Buy));

    let mut g = new_state(&[id::DISMANTLE], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE]);
    play(&mut g, id::DISMANTLE);
    assert!(g.trash.is_empty() && discard_of(&g, 0).is_empty());
}

#[test]
fn dismantle_still_gains_a_gold_with_nothing_cheaper_and_a_cheaper_card_with_no_gold() {
    let mut g = new_state(&[id::DISMANTLE], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE, id::ESTATE]);
    for c in [id::COPPER, id::CURSE] {
        empty_pile(&mut g, c);
    }
    play(&mut g, id::DISMANTLE);
    // (With nothing left to do the turn ends at once: look at everything the player owns.)
    assert_eq!(g.players[0].all_cards(), counts_of(&[id::DISMANTLE, id::GOLD]), "no cheaper card in the Supply: just the Gold");
    assert!(g.trash.has(id::ESTATE));

    let mut g = new_state(&[id::DISMANTLE], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE, id::SILVER]);
    empty_pile(&mut g, id::GOLD);
    play(&mut g, id::DISMANTLE);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(discard_of(&g, 0), counts_of(&[id::ESTATE]));
}

#[test]
fn dismantle_cheaper_uses_the_current_cost() {
    // With a Bridge in play a Silver costs $2: cheaper than that is $1 or less (a Moat is $1 now; the Silver is not cheaper than itself).
    let mut g = new_state(&[id::DISMANTLE, id::BRIDGE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE, id::SILVER]);
    set_in_play(&mut g, 0, &[id::BRIDGE]);
    g.turn.cost_reduction = 1;
    play(&mut g, id::DISMANTLE);
    // The hand is the Silver alone: trashed without a question; the cheaper card comes next.
    assert!(g.trash.has(id::SILVER));
    assert_eq!(
        sorted_choices(&g),
        sorted(vec![Choice::Card(id::COPPER), Choice::Card(id::CURSE), Choice::Card(id::ESTATE), Choice::Card(id::MOAT)]),
        "Estate and Moat cost $1 under the Bridge; Silver, at $2, is not cheaper than itself"
    );
}

#[test]
fn dismantle_potion_and_debt_costs() {
    // A cost of only a Potion (Transmute $0P) or only Debt (Engineer $0 and 4 Debt) is not "$1 or more": no gains.
    for c in [id::TRANSMUTE, id::ENGINEER] {
        let mut g = new_state(&[id::DISMANTLE, id::TRANSMUTE, id::ENGINEER], 2);
        set_hand(&mut g, 0, &[id::DISMANTLE, c]);
        play(&mut g, id::DISMANTLE);
        assert!(g.trash.has(c));
        assert!(discard_of(&g, 0).is_empty(), "{}: no cheaper card and no Gold", cards::name(c));
    }
    // Apothecary costs $2P: that is $1 or more. Cheaper: no part larger and one smaller (an Estate $2 has no Potion; Transmute $0P; Copper).
    let mut g = new_state(&[id::DISMANTLE, id::APOTHECARY, id::TRANSMUTE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE, id::APOTHECARY]);
    play(&mut g, id::DISMANTLE);
    assert_eq!(
        sorted_choices(&g),
        sorted(vec![Choice::Card(id::COPPER), Choice::Card(id::CURSE), Choice::Card(id::ESTATE), Choice::Card(id::TRANSMUTE)]),
        "a Familiar $3P is not cheaper (more $), Apothecary itself is not cheaper than itself"
    );
    choose(&mut g, Choice::Card(id::TRANSMUTE));
    assert!(discard_of(&g, 0).has(id::GOLD) && discard_of(&g, 0).has(id::TRANSMUTE));
}

#[test]
fn dismantle_a_fortune_gains_a_province_or_an_overlord_and_a_gold() {
    // Fortune costs $8 and 8 Debt: Province ($8, no Debt) and Overlord ($0 and 8 Debt) are cheaper (wiki: Dismantle).
    let mut g = new_state(&[id::DISMANTLE, id::GLADIATOR, id::OVERLORD], 2);
    set_hand(&mut g, 0, &[id::DISMANTLE, id::FORTUNE]);
    play(&mut g, id::DISMANTLE);
    let offered = choices(&g);
    assert!(offered.contains(&Choice::Card(id::PROVINCE)), "{offered:?}");
    assert!(offered.contains(&Choice::Card(id::OVERLORD)), "{offered:?}");
    assert!(offered.contains(&Choice::Card(id::GOLD)));
    assert!(!offered.contains(&Choice::Card(id::FORTUNE)), "not cheaper than itself");
    choose(&mut g, Choice::Card(id::PROVINCE));
    assert_eq!(discard_of(&g, 0), counts_of(&[id::PROVINCE, id::GOLD]));
}

// ---------------------------------------------------------------------------
// Church
// ---------------------------------------------------------------------------

fn church_kingdom() -> Vec<CardId> {
    vec![id::CHURCH, id::THRONE_ROOM, id::VILLAGE, id::SMITHY, id::HAVEN, id::PRINCE]
}

/// Play out player 0's turn and player 1's, stopping at the first decision of player 0's next turn.
fn to_next_own_turn(g: &mut GameState) {
    let start = g.turn.number;
    loop {
        finish_turn(g);
        if g.turn.player == 0 && g.turn.number > start {
            return;
        }
    }
}

/// From the first decision of player 0's turn to the Buy decision: pass the Action phase if there is one.
fn to_buy(g: &mut GameState) {
    let d = expect_decision(g);
    if d.kind == DecisionKind::PlayAction {
        pass(g);
    }
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Buy);
}

#[test]
fn church_sets_aside_up_to_three_cards_face_down_and_stays_in_play() {
    let mut g = new_state(&church_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CHURCH, id::GOLD, id::SILVER, id::COPPER, id::ESTATE, id::PROVINCE]);
    set_deck_known(&mut g, 0, &junk());
    let before = total_cards(&g);
    let step = play(&mut g, id::CHURCH);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::Any, min: 0, max: 3, ordered: false });
    assert_eq!(g.turn.actions, 1, "+1 Action (one used to play it)");
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::GOLD));
    choose(&mut g, Choice::Pass); // "up to 3"
    let ps = &g.players[0];
    assert_eq!(ps.church.len(), 1);
    assert_eq!(ps.church.total(), 2);
    assert_eq!(ps.church.batch(0).1, 2);
    assert!(ps.set_aside.is_empty() && !ps.hand.has(id::COPPER) && !ps.hand.has(id::GOLD));
    assert_eq!(ps.all_cards().get(id::GOLD), 1, "still owned");
    assert_eq!(total_cards(&g), before);
    assert_eq!(ps.pending_durations_len, 1);
    assert_eq!(ps.pending_durations[0].card, id::CHURCH);
    assert!(ps.in_play.has(id::CHURCH));
    // Duration: it stays in play through Clean-up.
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::CHURCH) && !g.players[0].discard.has(id::CHURCH));
}

#[test]
fn church_three_cards_end_the_pick_and_zero_is_fine() {
    let mut g = new_state(&church_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CHURCH, id::GOLD, id::SILVER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHURCH);
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].church.total(), 3, "a third pick finishes it");
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::PlayAction | DecisionKind::Buy));

    let mut g = new_state(&church_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CHURCH, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHURCH);
    assert!(choices(&g).contains(&Choice::Pass));
    pass(&mut g);
    assert_eq!((g.players[0].church.len(), g.players[0].church.total()), (1, 0), "an empty batch still has its trash next turn");
    assert!(held(&g, 0).has(id::GOLD), "nothing was set aside");
}

#[test]
fn church_puts_the_cards_into_the_hand_next_turn_and_then_may_trash_one() {
    let mut g = new_state(&church_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CHURCH, id::GOLD, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHURCH);
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::GOLD));
    to_next_own_turn(&mut g);
    // At the start of the turn: the set-aside cards are in the hand, and a card may be trashed.
    let d = g.pending_decision().expect("the trash");
    assert_eq!((d.player, d.kind), (0, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false }));
    assert!(hand_of(&g, 0).has(id::GOLD) && hand_of(&g, 0).has(id::COPPER));
    assert_eq!(hand_of(&g, 0).total(), 5 + 2);
    assert!(g.players[0].church.is_empty() && g.players[0].pending_durations_len == 0);
    assert!(choices(&g).contains(&Choice::Pass) && choices(&g).contains(&Choice::Card(id::ESTATE)), "a card that was already in the hand can be trashed");
    // The card trashed may be one that was set aside.
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.trash.has(id::COPPER));
    assert!(g.players[0].in_play.has(id::CHURCH), "still in play this turn");
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::PlayAction | DecisionKind::Buy));
    // Discarded at the end of that turn.
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::CHURCH) && !g.players[0].in_play.has(id::CHURCH));
}

#[test]
fn church_with_nothing_set_aside_may_still_trash_next_turn() {
    let mut g = new_state(&church_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CHURCH]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHURCH);
    pass(&mut g);
    to_next_own_turn(&mut g);
    let d = g.pending_decision().expect("the trash");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, min: 0, max: 1, .. }), "{d:?}");
    pass(&mut g);
    assert!(g.trash.is_empty());
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::CHURCH));
}

#[test]
fn two_church_plays_are_two_batches_each_followed_by_its_own_trash() {
    // A Throne Room'd Church: two batches. The batches come in one at a time, each with a trash: the first trash is decided before the second
    // batch is in the hand.
    let mut g = new_state(&church_kingdom(), 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::CHURCH, id::GOLD, id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::CHURCH));
    choose(&mut g, Choice::Card(id::GOLD)); // first play: Gold (no card with a higher id is left to pick, so it ends)
    choose(&mut g, Choice::Card(id::COPPER)); // second play: Copper and Silver
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].church.len(), 2, "{:?}", g.pending_decision());
    to_next_own_turn(&mut g);
    assert!(g.players[0].in_play.has(id::THRONE_ROOM) && g.players[0].in_play.has(id::CHURCH), "the Throne Room stays with the Duration");
    // The two batches differ: the player chooses which goes first (here the older one).
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(choices(&g), vec![Choice::Position(0), Choice::Position(1)]);
    choose(&mut g, Choice::Position(0));
    // Batch 1: the Gold; the Silver is not in the hand yet.
    assert!(hand_of(&g, 0).has(id::GOLD) && !hand_of(&g, 0).has(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    // Batch 2 arrives, with its own trash.
    assert!(hand_of(&g, 0).has(id::SILVER));
    let d = g.pending_decision().expect("the second trash");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, min: 0, max: 1, .. }), "{d:?}");
    pass(&mut g);
    assert_eq!(g.trash.total(), 1);
}

#[test]
fn church_is_ordered_against_other_start_of_turn_effects() {
    // A Prince's Smithy and a Church batch: both change the hand, so the player chooses which comes first.
    let mut g = new_state(&church_kingdom(), 2);
    g.players[0].in_play.add(id::PRINCE, 1);
    g.players[0].stays.add(id::PRINCE, 1);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    g.players[0].in_play.add(id::CHURCH, 1);
    assert!(g.players[0].church.begin() && g.players[0].church.add(id::GOLD));
    g.players[0].pending_durations[0] = PendingDuration { card: id::CHURCH, times: 1, arg: id::CHURCH, used: 0 };
    g.players[0].pending_durations_len = 1;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Position(0), Choice::Card(id::SMITHY)]));
    choose(&mut g, Choice::Card(id::SMITHY));
    // Smithy first: 3 cards drawn, then the Church batch and its trash.
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
    assert_eq!(hand_of(&g, 0).total(), 1 + 3 + 1);
    assert!(hand_of(&g, 0).has(id::GOLD));
}

/// A start of turn with Church batches `batches` (oldest first), one Church entry each, and a junk deck.
fn church_batches_state(batches: &[&[CardId]]) -> GameState {
    let mut g = new_state(&church_kingdom(), 2);
    g.players[0].in_play.add(id::CHURCH, 1);
    for (i, b) in batches.iter().enumerate() {
        assert!(g.players[0].church.begin());
        for &c in b.iter() {
            assert!(g.players[0].church.add(c));
        }
        g.players[0].pending_durations[i] = PendingDuration { card: id::CHURCH, times: 1, arg: id::CHURCH, used: 0 };
    }
    g.players[0].pending_durations_len = batches.len() as u8;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    g
}

#[test]
fn the_player_chooses_which_church_batch_goes_into_the_hand_first() {
    // The newer batch (Gold) first: it arrives, its trash is decided (Gold can go), then the older batch (Silver) arrives with its own trash.
    let mut g = church_batches_state(&[&[id::SILVER], &[id::GOLD]]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(choices(&g), vec![Choice::Position(0), Choice::Position(1)]);
    choose(&mut g, Choice::Position(1));
    assert!(hand_of(&g, 0).has(id::GOLD) && !hand_of(&g, 0).has(id::SILVER));
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
    choose(&mut g, Choice::Card(id::GOLD));
    assert!(g.trash.has(id::GOLD));
    // The one batch left needs no question: it comes in, with its trash.
    let d = g.pending_decision().unwrap();
    assert!(hand_of(&g, 0).has(id::SILVER), "{d:?}");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
    pass(&mut g);
    assert!(g.players[0].church.is_empty());
}

#[test]
fn equal_church_batches_are_one_choice_and_a_single_church_is_not_asked() {
    // Two batches of the same cards: nothing to decide.
    let mut g = church_batches_state(&[&[id::GOLD], &[id::GOLD]]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
    assert_eq!(hand_of(&g, 0).get(id::GOLD), 1);
    // Three batches, two alike: two choices, each batch named by its first (oldest) index.
    let mut g = church_batches_state(&[&[id::GOLD], &[id::SILVER], &[id::GOLD]]);
    expect_decision(&mut g);
    assert_eq!(choices(&g), vec![Choice::Position(0), Choice::Position(1)]);
    choose(&mut g, Choice::Position(1));
    assert!(hand_of(&g, 0).has(id::SILVER) && !hand_of(&g, 0).has(id::GOLD));
    // A lone Church batch: no question.
    let mut g = church_batches_state(&[&[id::GOLD]]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
}

#[test]
fn church_batches_and_another_effect_are_all_ordered_by_the_player() {
    // A Prince's Smithy with two different Church batches: Smithy can come between the batches.
    let mut g = church_batches_state(&[&[id::SILVER], &[id::GOLD]]);
    g.players[0].in_play.add(id::PRINCE, 1);
    g.players[0].stays.add(id::PRINCE, 1);
    g.players[0].prince_cards.add(id::SMITHY, 1);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Position(0), Choice::Position(1), Choice::Card(id::SMITHY)]));
    choose(&mut g, Choice::Position(1));
    choose(&mut g, Choice::Card(id::GOLD)); // trash the Gold
    // The older batch and the Smithy are still to come.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Position(0), Choice::Card(id::SMITHY)]));
}

#[test]
fn church_cards_are_hidden_from_the_other_players_view_and_dealt_afresh_in_a_determinization() {
    let mut g = new_state(&church_kingdom(), 2);
    set_hand(&mut g, 0, &[id::CHURCH, id::GOLD, id::PROVINCE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHURCH);
    choose(&mut g, Choice::Card(id::GOLD));
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Pass);
    let mine = PlayerView::new(&g, 0);
    let theirs = PlayerView::new(&g, 1);
    assert_eq!(mine.church(), counts_of(&[id::ESTATE, id::GOLD]), "the owner knows");
    assert!(theirs.church().is_empty(), "nobody else sees them");
    assert_eq!(theirs.church_count_of(0), 2, "only how many");
    // A determinization from the other player's side deals two hidden cards of its own back (same sizes, same conservation).
    let mut rng = dominion_engine::rng::Rng::new(7);
    for _ in 0..20 {
        let s = theirs.determinize(&mut rng);
        assert_eq!((s.players[0].church.len(), s.players[0].church.total()), (1, 2));
        assert_eq!(s.players[0].all_cards(), g.players[0].all_cards(), "same cards overall");
        assert_eq!(s.players[0].hand.total(), g.players[0].hand.total());
    }
    // The owner's own determinization keeps theirs.
    let s = mine.determinize(&mut rng);
    assert_eq!(s.players[0].church, g.players[0].church);
}

#[test]
fn church_batches_round_trip_through_the_text_format() {
    let mut g = new_state(&church_kingdom(), 2);
    g.players[0].in_play.add(id::CHURCH, 2);
    assert!(g.players[0].church.begin() && g.players[0].church.add(id::GOLD) && g.players[0].church.add(id::COPPER) && g.players[0].church.add(id::GOLD));
    assert!(g.players[0].church.begin());
    for i in 0..2 {
        g.players[0].pending_durations[i] = PendingDuration { card: id::CHURCH, times: 1, arg: id::CHURCH, used: 0 };
    }
    g.players[0].pending_durations_len = 2;
    g.players[0].deck_unknown = counts_of(&[id::ESTATE; 8]);
    let text = format_state(&g);
    assert!(text.contains("church: Copper, 2 Gold; -"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].church, g.players[0].church);
    assert_eq!(back.players[0].church.len(), 2);
    assert_eq!(back.players[0].all_cards(), g.players[0].all_cards());
}

// ---------------------------------------------------------------------------
// Marchland
// ---------------------------------------------------------------------------

#[test]
fn marchland_is_worth_a_vp_per_three_victory_cards_counting_itself_rounded_down() {
    let mut g = new_state(&[id::MARCHLAND], 2);
    let vp = |g: &GameState| g.players[0].vp();
    g.players[0].discard = counts_of(&[id::MARCHLAND]);
    assert_eq!(vp(&g), 0, "1 Victory card (itself)");
    g.players[0].discard = counts_of(&[id::MARCHLAND, id::ESTATE, id::ESTATE]);
    assert_eq!(vp(&g), 1 + 2, "3 Victory cards: itself and two Estates");
    // 11 Victory cards: each Marchland is worth 3.
    g.players[0].discard = counts_of(&[id::MARCHLAND; 1].iter().chain([id::ESTATE; 10].iter()).copied().collect::<Vec<_>>());
    assert_eq!(vp(&g), 3 + 10);
    g.players[0].discard = counts_of(&[id::MARCHLAND; 2].iter().chain([id::ESTATE; 9].iter()).copied().collect::<Vec<_>>());
    assert_eq!(vp(&g), 2 * 3 + 9);
    // Only Victory cards count, wherever they are and whatever else they are; Curses and Coppers do not.
    g.players[0].discard = counts_of(&[id::MARCHLAND, id::CURSE, id::COPPER, id::COPPER, id::VILLAGE]);
    assert_eq!(vp(&g), 0 - 1);
    let mut g = new_state(&[id::MARCHLAND, id::NOBLES, id::HAREM], 2);
    g.players[0].hand = counts_of(&[id::MARCHLAND]);
    g.players[0].in_play = counts_of(&[id::NOBLES]);
    g.players[0].deck_unknown = counts_of(&[id::HAREM]);
    assert_eq!(g.players[0].vp(), 1 + 2 + 2, "Action-Victory and Treasure-Victory cards count as Victory cards");
}

#[test]
fn buying_marchland_gives_a_buy_and_a_dollar_per_card_discarded() {
    let mut g = new_state(&[id::MARCHLAND], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY]);
    g.turn.coins = 5;
    to_buy(&mut g);
    buy(&mut g, id::MARCHLAND);
    assert_eq!((g.turn.coins, g.turn.buys), (0, 1), "the Buy it used is given back");
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 0, max: 255, ordered: false });
    assert!(choices(&g).contains(&Choice::Pass), "\"any number\" includes zero");
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Pass);
    assert_eq!((g.turn.coins, g.turn.buys), (2, 1), "+$1 per card discarded");
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
    assert!(g.players[0].discard.has(id::MARCHLAND) && g.players[0].hand.has(id::DUCHY));
    // The $2 buy an Estate.
    buy(&mut g, id::ESTATE);
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn marchland_zero_discards_and_discarding_the_whole_hand() {
    let mut g = new_state(&[id::MARCHLAND], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    g.turn.coins = 5;
    to_buy(&mut g);
    buy(&mut g, id::MARCHLAND);
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!((g.turn.coins, g.turn.buys), (0, 1));
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn marchland_gained_into_the_hand_can_be_one_of_the_cards_discarded() {
    let mut g = new_state(&[id::MARCHLAND, id::ARTISAN], 2);
    set_hand(&mut g, 0, &[id::ARTISAN, id::ESTATE]);
    play(&mut g, id::ARTISAN);
    choose(&mut g, Choice::Card(id::MARCHLAND));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, min: 0, .. }), "{d:?}");
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Card(id::ESTATE), Choice::Card(id::MARCHLAND), Choice::Pass]));
    assert_eq!(g.turn.buys, 2, "+1 Buy now, in the Action phase");
    choose(&mut g, Choice::Card(id::MARCHLAND));
    assert_eq!(g.turn.coins, 1);
    assert!(g.players[0].discard.has(id::MARCHLAND));
}

#[test]
fn marchland_gained_on_another_players_turn_gives_that_turn_nothing() {
    // Governor's third option: the other player trashes a Smithy ($4) for a card costing exactly $5, a Marchland. Its Buy and $ are for the
    // gainer's own turn, so the current player's Buys and $ are untouched; the discard is still allowed.
    let mut g = new_state(&[id::GOVERNOR, id::MARCHLAND, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GOVERNOR]);
    set_hand(&mut g, 1, &[id::SMITHY, id::ESTATE]);
    play(&mut g, id::GOVERNOR);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(2));
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::SMITHY));
    choose(&mut g, Choice::Card(id::MARCHLAND));
    let d = expect_decision(&mut g);
    assert_eq!((d.player, g.turn.player), (1, 0));
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!((g.turn.coins, g.turn.buys), (0, 1));
    assert!(g.players[1].discard.has(id::MARCHLAND) && g.players[1].discard.has(id::ESTATE));
}

// ---------------------------------------------------------------------------
// Summon
// ---------------------------------------------------------------------------

fn summon_kingdom() -> Vec<CardId> {
    vec![id::SUMMON, id::VILLAGE, id::SMITHY, id::MARKET, id::CARAVAN, id::WATCHTOWER, id::CHURCH, id::PRINCE]
}

/// Player 0 is in the Buy phase with `coins` and `buys`.
fn summon_buy_phase(coins: u16, buys: u8) -> GameState {
    let mut g = new_state(&summon_kingdom(), 2);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.coins = coins;
    g.turn.buys = buys;
    to_buy(&mut g);
    g
}

#[test]
fn summon_is_bought_for_five_and_a_buy_to_gain_an_action_up_to_four_and_set_it_aside() {
    let mut g = summon_buy_phase(5, 1);
    assert!(choices(&g).contains(&Choice::Card(id::SUMMON)));
    let mut ev: Vec<Event> = Vec::new();
    let smithy = g.supply.get(id::SMITHY);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    choose_ev(&mut g, Choice::Card(id::SUMMON), &mut ev);
    assert!(ev.contains(&Event::Buy { player: 0, card: id::SUMMON }));
    assert_eq!((g.turn.coins, g.turn.buys), (0, 0));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }) || d.source == Some(id::SUMMON), "{d:?}");
    // Actions costing up to $4: the Village, the Smithy, the Caravan and the Watchtower ($3), the Church ($3), not the Market or the Prince.
    assert_eq!(
        sorted_choices(&g),
        sorted(vec![
            Choice::Card(id::VILLAGE), Choice::Card(id::SMITHY), Choice::Card(id::CARAVAN), Choice::Card(id::WATCHTOWER), Choice::Card(id::CHURCH),
        ])
    );
    assert!(!choices(&g).contains(&Choice::Pass), "the gain is not optional");
    let before = total_cards(&g);
    choose_ev(&mut g, Choice::Card(id::SMITHY), &mut ev);
    assert_eq!(g.supply.get(id::SMITHY), smithy - 1);
    assert_eq!(g.players[0].summon_cards.get(id::SMITHY), 1);
    assert!(!g.players[0].discard.has(id::SMITHY), "set aside, not in the discard pile");
    assert_eq!(g.players[0].all_cards().get(id::SMITHY), 1, "still owned");
    assert_eq!(total_cards(&g), before);
    // Face up: everybody sees it.
    assert_eq!(PlayerView::new(&g, 1).summon_cards_of(0), counts_of(&[id::SMITHY]));
}

#[test]
fn the_summoned_card_is_played_at_the_start_of_the_next_turn_without_using_an_action() {
    let mut g = summon_buy_phase(5, 1);
    buy(&mut g, id::SUMMON);
    choose(&mut g, Choice::Card(id::SMITHY));
    let mut ev: Vec<Event> = Vec::new();
    let start = g.turn.number;
    while !(g.turn.player == 0 && g.turn.number > start) {
        finish_turn_ev(&mut g, &mut ev);
    }
    assert!(ev.contains(&Event::StartPlay { player: 0, source: id::SUMMON, card: id::SMITHY }), "{ev:?}");
    assert_eq!(g.turn.actions, 1, "no Action used");
    assert_eq!(g.players[0].hand.total(), 5 + 3, "the Smithy drew its three cards");
    assert!(g.players[0].in_play.has(id::SMITHY) && g.players[0].summon_cards.is_empty());
    // It is discarded in Clean-up like any played Action.
    finish_turn(&mut g);
    assert!(g.players[0].discard.has(id::SMITHY) && !g.players[0].in_play.has(id::SMITHY));
}

#[test]
fn a_summoned_duration_stays_in_play_and_works_next_turn_too() {
    let mut g = summon_buy_phase(5, 1);
    buy(&mut g, id::SUMMON);
    choose(&mut g, Choice::Card(id::CARAVAN));
    to_next_own_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 5 + 1, "Caravan's +1 Card now");
    assert_eq!(g.players[0].pending_durations_len, 1, "and its +1 Card next turn");
    finish_turn(&mut g);
    assert!(g.players[0].in_play.has(id::CARAVAN), "Duration: still in play");
}

#[test]
fn two_summons_of_the_same_card_play_it_twice() {
    let mut g = summon_buy_phase(10, 2);
    buy(&mut g, id::SUMMON);
    choose(&mut g, Choice::Card(id::VILLAGE));
    buy(&mut g, id::SUMMON);
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.players[0].summon_cards.get(id::VILLAGE), 2);
    to_next_own_turn(&mut g);
    assert_eq!(g.turn.actions, 1 + 2 + 2, "two Villages, no Actions used");
    assert_eq!(g.players[0].hand.total(), 5 + 2);
}

#[test]
fn summon_loses_track_of_a_card_that_moved_after_it_was_gained() {
    // Watchtower puts the gained Village on top of the deck: Summon cannot set it aside, and the Village is not played at the start of the next turn.
    let mut g = summon_buy_phase(5, 1);
    set_hand(&mut g, 0, &[id::WATCHTOWER]);
    let mut ev: Vec<Event> = Vec::new();
    buy(&mut g, id::SUMMON);
    choose(&mut g, Choice::Card(id::VILLAGE));
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.source), (0, Some(id::WATCHTOWER)));
    pass(&mut g); // not trashed
    expect_decision(&mut g);
    g.apply(Choice::Card(id::VILLAGE), &mut ev).unwrap(); // put it on the deck
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::VILLAGE));
    let _ = g.advance(&mut ev);
    assert_eq!(g.players[0].all_cards().get(id::VILLAGE), 1);
    assert!(g.players[0].summon_cards.is_empty(), "lost track: nothing set aside");
}

#[test]
fn summon_loses_track_of_a_card_a_watchtower_trashes() {
    let mut g = summon_buy_phase(5, 1);
    set_hand(&mut g, 0, &[id::WATCHTOWER]);
    buy(&mut g, id::SUMMON);
    choose(&mut g, Choice::Card(id::VILLAGE));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::VILLAGE)); // trash it
    assert!(g.trash.has(id::VILLAGE));
    assert!(g.players[0].summon_cards.is_empty());
}

#[test]
fn the_summoned_card_and_a_church_batch_are_ordered_by_the_player() {
    let mut g = new_state(&summon_kingdom(), 2);
    g.players[0].summon_cards.add(id::SMITHY, 1);
    g.players[0].in_play.add(id::CHURCH, 1);
    assert!(g.players[0].church.begin() && g.players[0].church.add(id::GOLD));
    g.players[0].pending_durations[0] = PendingDuration { card: id::CHURCH, times: 1, arg: id::CHURCH, used: 0 };
    g.players[0].pending_durations_len = 1;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(sorted_choices(&g), sorted(vec![Choice::Position(0), Choice::Card(id::SMITHY)]));
    // Church first: the Gold arrives and may be trashed before the Smithy draws.
    choose(&mut g, Choice::Position(0));
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
    choose(&mut g, Choice::Card(id::GOLD));
    assert!(g.trash.has(id::GOLD));
    assert!(g.players[0].in_play.has(id::SMITHY), "then the Smithy");
    assert_eq!(hand_of(&g, 0).total(), 1 + 3);
}

#[test]
fn summon_is_not_offered_without_an_action_to_gain_and_not_without_the_money() {
    let mut g = new_state(&[id::SUMMON, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::ESTATE]);
    g.turn.coins = 8;
    expect_decision(&mut g);
    pass(&mut g);
    assert!(!choices(&g).contains(&Choice::Card(id::SUMMON)), "no Action costing $4 or less to gain");

    let g = summon_buy_phase(4, 1);
    assert!(!choices(&g).contains(&Choice::Card(id::SUMMON)), "it costs $5");
}

#[test]
fn summoned_cards_round_trip_through_the_text_format_and_count_as_owned() {
    let mut g = new_state(&summon_kingdom(), 2);
    g.players[0].summon_cards.add(id::SMITHY, 1);
    g.players[0].summon_cards.add(id::VILLAGE, 2);
    g.players[0].deck_unknown = counts_of(&[id::ESTATE; 8]);
    let text = format_state(&g);
    assert!(text.contains("summoned: "), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].summon_cards.counts(), counts_of(&[id::SMITHY, id::VILLAGE, id::VILLAGE]));
    assert_eq!(back.players[0].all_cards().get(id::VILLAGE), 2);
}

#[test]
fn summon_is_a_promo_event_chosen_with_the_promo_cards() {
    let mut rng = dominion_engine::rng::Rng::new(3);
    let mut with_summon = 0;
    for _ in 0..200 {
        let k = cards::random_kingdom_with_colonies(&[CardSet::Promo], &[], &mut rng);
        assert!(k.iter().filter(|&&c| cards::is_kingdom(c)).all(|&c| cards::set_of(c) == CardSet::Promo));
        with_summon += k.contains(&id::SUMMON) as u32;
    }
    assert!(with_summon > 0, "Summon is dealt as an Event in Promo games");
}

// ---------------------------------------------------------------------------
// Random games
// ---------------------------------------------------------------------------

/// Random legal play over kingdoms with the new promo cards (and cards from every set): nothing is lost, nothing panics, every game ends, the
/// Church batches and the pending Church entries stay in step, and the new cards are all reached.
#[test]
fn random_games_with_the_new_promo_cards_conserve_cards_and_terminate() {
    let mut rng = dominion_engine::rng::Rng::new(0xC40C);
    let promo = [id::GOVERNOR, id::DISMANTLE, id::CHURCH, id::MARCHLAND];
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| !promo.contains(&c) && cards::cost(c) <= 5).collect();
    let (mut played, mut gained_marchland, mut summoned, mut started) = (std::collections::BTreeSet::new(), 0u32, 0u32, 0u32);
    for game in 0..1500u64 {
        let n = 2 + (game % 3) as usize;
        let mut kingdom: Vec<CardId> = promo.to_vec();
        kingdom.push(id::SUMMON);
        while kingdom.iter().filter(|&&c| cards::is_kingdom(c)).count() < 10 {
            let c = others[rng.below(others.len() as u32) as usize];
            if !kingdom.contains(&c) && cards::is_ready(c) {
                kingdom.push(c);
            }
        }
        cards::add_potion_if_needed(&mut kingdom);
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom, seed: game, max_turns: 300, ..Default::default() });
        let initial = total_cards(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        let mut log: Vec<Event> = Vec::new();
        loop {
            for e in &log {
                match *e {
                    Event::Play { card, .. } | Event::PlayAgain { card, .. } => {
                        played.insert(card);
                    }
                    Event::Gain { card, .. } if card == id::MARCHLAND => gained_marchland += 1,
                    Event::SetAside { card, .. } if cards::is(card, cards::ACTION) => summoned += 1,
                    Event::StartPlay { source, .. } if source == id::SUMMON => started += 1,
                    _ => {}
                }
            }
            log.clear();
            match g.advance(&mut log) {
                Step::Decision(_) => {
                    g.legal_choices(&mut buf);
                    assert!(!buf.is_empty());
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut log).unwrap();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            assert_eq!(total_cards(&g), initial, "game {game}: card conservation after {log:?}");
            // Every Church batch has its Church entry waiting (in the pending Durations, or in the start-of-turn list while they resolve).
            for p in 0..n {
                let ps = &g.players[p];
                let waiting = ps.pending_durations[..ps.pending_durations_len as usize].iter().filter(|e| e.card == id::CHURCH).count()
                    + if g.turn.player as usize == p { g.turn.start_pending.as_slice().iter().filter(|e| e.card == id::CHURCH).count() } else { 0 };
                assert_eq!(ps.church.len(), waiting, "game {game}: Church batches and entries");
            }
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
    }
    for c in [id::GOVERNOR, id::DISMANTLE, id::CHURCH] {
        assert!(played.contains(&c), "{} was never played", cards::name(c));
    }
    assert!(gained_marchland > 20 && summoned > 20 && started > 20, "marchland {gained_marchland}, summoned {summoned}, started {started}");
}

// ===========================================================================
// The stop-moving rule (docs/stop-moving-rule.md)
// ===========================================================================

#[test]
fn a_card_played_by_a_prince_cannot_trash_another_copy_of_itself_that_is_in_play() {
    // The Mining Village the Prince plays is not in play; the one in play (a Duration-less leftover set up directly) is another card.
    let mut g = new_state(&[id::PRINCE, id::MINING_VILLAGE], 2);
    with_prince_holding(&mut g, &[id::MINING_VILLAGE]);
    g.players[0].in_play.add(id::MINING_VILLAGE, 1);
    set_deck_known(&mut g, 0, &junk());
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!() };
    assert!(!matches!(d.kind, DecisionKind::YesNo { .. }), "no offer to trash it: {d:?}");
    assert_eq!(g.turn.coins, 0);
    assert_eq!(g.players[0].in_play.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.players[0].prince_cards.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.turn.actions, 2 + 1, "+2 Actions, and the Prince's play used none");
    assert_eq!(g.turn.led.n, 0);
}

// ===========================================================================
// Audit: Command cards, and Enlightenment's Treasures-are-Actions
// ===========================================================================

#[test]
fn prince_cannot_set_aside_a_command_card_that_cost_reduction_made_cheap() {
    // Band of Misfits ($5, Action-Command) costs $4 after a reduction, but Prince takes a "non-Duration, non-Command Action".
    let mut g = new_state(&[id::PRINCE, id::BAND_OF_MISFITS, id::VILLAGE], 2);
    g.turn.cost_reduction = 1;
    assert_eq!(g.cost(id::BAND_OF_MISFITS), 4);
    set_hand(&mut g, 0, &[id::PRINCE, id::BAND_OF_MISFITS, id::VILLAGE]);
    play(&mut g, id::PRINCE);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::VILLAGE)), "{c:?}");
    assert!(!c.contains(&Choice::Card(id::BAND_OF_MISFITS)), "{c:?}");
}

#[test]
fn captain_cannot_play_a_command_card_that_cost_reduction_made_cheap() {
    let mut g = new_state(&[id::CAPTAIN, id::BAND_OF_MISFITS, id::VILLAGE], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::CAPTAIN]);
    play(&mut g, id::CAPTAIN);
    // The Village is the only legal pick, so it is played without asking (+2 Actions).
    assert_eq!(g.turn.actions, 2);
}

#[test]
fn walled_village_counts_the_treasures_in_play_as_actions_under_enlightenment() {
    for (enlightened, offered) in [(true, false), (false, true)] {
        let mut k = vec![id::WALLED_VILLAGE];
        if enlightened {
            k.push(id::ENLIGHTENMENT);
        }
        let mut g = new_state(&k, 2);
        if enlightened {
            g.rs.suns = 0;
            g.rs.active = id::ENLIGHTENMENT;
            g.rs_update_hooks();
        }
        set_in_play(&mut g, 0, &[id::COPPER, id::SILVER]);
        set_hand(&mut g, 0, &[id::WALLED_VILLAGE]);
        set_deck_known(&mut g, 0, &junk());
        play(&mut g, id::WALLED_VILLAGE);
        let d = to_cleanup_offer(&mut g);
        assert_eq!(d.as_ref().is_some_and(|d| is_topdeck_offer(d, id::WALLED_VILLAGE)), offered, "enlightened: {enlightened}");
    }
}
