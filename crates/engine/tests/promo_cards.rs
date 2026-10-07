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
    ps.prince_stays.add(id::PRINCE, 1);
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
    g.players[0].prince_stays.add(id::PRINCE, 1);
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
        DecisionKind::Select { from: Zone::Supply, act: Act::Play, filter: Filter::ActionCostUpTo(4), min: 1, max: 1, ordered: false },
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
    g.players[0].pending_durations[i] = dominion_engine::state::PendingDuration { card: id::CAPTAIN, times, arg: 0, used: false };
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
