//! Reactions that act on a card gained straight onto the deck by another set's card (Watchtower
//! trashes it, Sailor plays it, Tiara puts it back) and Replace's gain-then-topdeck order. See
//! `docs/seaside-prosperity-plan.md` §6, `docs/intrigue-plan.md` §7 and `docs/alchemy-plan.md` §8
//! for the rulings. Moat and Lighthouse as a choice are in `base_cards.rs` / `seaside_cards.rs`,
//! Sailor's per-play uses in `seaside_cards.rs`.

mod common;
use common::*;
use dominion_engine::cards::id;
use dominion_engine::state::{Frame, FrameKind};
use dominion_engine::*;

fn top_down(g: &GameState, p: usize) -> Vec<CardId> {
    g.players[p].deck_known.iter_top_down().collect()
}



/// Gain a card costing up to `max_cost` onto player 0's deck, as a real effect would ("gains it onto
/// the deck" is how Bureaucrat, Treasure Map and Replace's older wording put a card there).
fn gain_onto_deck(g: &mut GameState, max_cost: u8) {
    g.pending = Pending::None;
    g.stack.push(Frame { max: max_cost, dest: Dest::DeckTop, ..Frame::new(FrameKind::Gain, 0, id::WORKSHOP) });
}

// ===========================================================================
// Gains onto the deck: Watchtower and Sailor react to the known top card
// ===========================================================================

#[test]
fn watchtower_trashes_the_silver_a_bureaucrat_gains_onto_the_deck() {
    let mut g = new_state(&[id::BUREAUCRAT, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT, id::WATCHTOWER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::ESTATE]);
    let Step::Decision(d) = play(&mut g, id::BUREAUCRAT) else { panic!() };
    assert_eq!(d.player, 0);
    assert_eq!(d.source, Some(id::WATCHTOWER));
    assert_eq!(
        d.kind,
        DecisionKind::Select { from: Zone::DeckTop, act: Act::Trash, filter: Filter::Card(id::SILVER), min: 0, max: 1, ordered: false }
    );
    assert_eq!(top_down(&g, 0), vec![id::SILVER, id::COPPER, id::ESTATE]);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut ev);
    assert_eq!(g.trash.get(id::SILVER), 1);
    assert_eq!(top_down(&g, 0), vec![id::COPPER, id::ESTATE], "the rest of the deck is untouched");
    assert!(g.players[0].hand.has(id::WATCHTOWER), "revealing keeps it in hand");
    assert!(ev.iter().any(|e| matches!(e, Event::Reaction { player: 0, card: id::WATCHTOWER })), "reveal logged: {ev:?}");
}

#[test]
fn watchtower_may_leave_the_gained_silver_on_the_deck_and_is_asked_only_once() {
    // Putting it onto the deck is pointless (it is there), so there is a single offer: trash it or not.
    let mut g = new_state(&[id::BUREAUCRAT, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT, id::WATCHTOWER]);
    play(&mut g, id::BUREAUCRAT);
    choose(&mut g, Choice::Pass);
    assert_eq!(top_down(&g, 0), vec![id::SILVER]);
    assert!(g.trash.is_empty());
    assert!(matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Buy, .. })), "no second Watchtower offer");
}

#[test]
fn watchtower_reacts_to_each_of_treasure_maps_four_golds() {
    let mut g = new_state(&[id::TREASURE_MAP, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::TREASURE_MAP, id::WATCHTOWER]);
    play(&mut g, id::TREASURE_MAP);
    // One offer per Gold. Trash the first and the third, keep the others.
    for (i, trash) in [true, false, true, false].into_iter().enumerate() {
        let d = g.pending_decision().unwrap_or_else(|| panic!("offer {i}"));
        assert_eq!(
            d.kind,
            DecisionKind::Select { from: Zone::DeckTop, act: Act::Trash, filter: Filter::Card(id::GOLD), min: 0, max: 1, ordered: false },
            "offer {i}"
        );
        choose(&mut g, if trash { Choice::Card(id::GOLD) } else { Choice::Pass });
    }
    assert_eq!(g.trash.get(id::GOLD), 2);
    assert_eq!(top_down(&g, 0), vec![id::GOLD, id::GOLD]);
    assert_eq!(g.supply.get(id::GOLD), 30 - 4, "all four were gained");
}

#[test]
fn watchtower_cannot_trash_a_gained_card_that_is_no_longer_on_top() {
    // Two Watchtower offers for the same Silver (two copies in hand): the first trashes it, the
    // second finds nothing to act on and is skipped.
    let mut g = new_state(&[id::BUREAUCRAT, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT, id::WATCHTOWER, id::WATCHTOWER]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::BUREAUCRAT);
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.trash.get(id::SILVER), 1);
    assert_eq!(top_down(&g, 0), vec![id::COPPER], "the Copper beneath is not mistaken for the gained card");
    assert!(matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn sailor_plays_a_duration_gained_onto_the_deck_from_the_top_of_the_deck() {
    let mut g = new_state(&[id::SAILOR, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::SAILOR]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::ESTATE]);
    play(&mut g, id::SAILOR);
    gain_onto_deck(&mut g, 4);
    expect_decision(&mut g);
    let Step::Decision(d) = choose(&mut g, Choice::Card(id::CARAVAN)) else { panic!() };
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    assert_eq!((d.source, d.subject), (Some(id::SAILOR), id::CARAVAN));
    assert_eq!(top_down(&g, 0), vec![id::CARAVAN, id::DUCHY, id::ESTATE]);
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].in_play.has(id::CARAVAN), "played straight from the top of the deck");
    assert_eq!(g.players[0].hand.get(id::DUCHY), 1, "Caravan's +1 Card drew the card beneath it");
    assert_eq!(top_down(&g, 0), vec![id::ESTATE]);
}

#[test]
fn sailor_may_decline_to_play_the_duration_it_gained_onto_the_deck() {
    let mut g = new_state(&[id::SAILOR, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::SAILOR]);
    play(&mut g, id::SAILOR);
    gain_onto_deck(&mut g, 4);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::CARAVAN));
    choose(&mut g, Choice::No);
    assert_eq!(top_down(&g, 0), vec![id::CARAVAN]);
    assert!(!g.players[0].in_play.has(id::CARAVAN));
}

// ===========================================================================
// Replace gains first, then moves an Action or Treasure onto the deck
// ===========================================================================

#[test]
fn replace_gains_to_the_discard_pile_then_topdecks_an_action_or_treasure() {
    let mut g = new_state(&[id::REPLACE], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::ESTATE]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::REPLACE, &mut ev);
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut ev);
    assert_eq!(top_down(&g, 0), vec![id::SILVER]);
    assert!(g.players[0].discard.is_empty());
    let gain = ev.iter().position(|e| matches!(e, Event::Gain { player: 0, card: id::SILVER, to: Dest::Discard, .. })).expect("gained to the discard pile");
    let top = ev.iter().position(|e| matches!(e, Event::Topdeck { player: 0, card: id::SILVER })).expect("then put onto the deck");
    assert!(gain < top, "{ev:?}");
}

#[test]
fn tiara_puts_back_the_card_replace_gained_and_replace_then_leaves_it_alone() {
    let mut g = new_state(&[id::REPLACE, id::TIARA], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::ESTATE]);
    set_in_play(&mut g, 0, &[id::TIARA]);
    play(&mut g, id::REPLACE);
    let Step::Decision(d) = choose(&mut g, Choice::Card(id::SILVER)) else { panic!() };
    // The Silver is in the discard pile: Tiara's offer is a real one.
    assert_eq!(
        d.kind,
        DecisionKind::Select { from: Zone::Discard, act: Act::Topdeck, filter: Filter::Card(id::SILVER), min: 0, max: 1, ordered: false }
    );
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut ev);
    assert_eq!(top_down(&g, 0), vec![id::SILVER], "on the deck once, not twice");
    assert!(g.players[0].discard.is_empty());
    assert_eq!(ev.iter().filter(|e| matches!(e, Event::Topdeck { .. })).count(), 1, "{ev:?}");
}

#[test]
fn tiara_declined_replace_still_puts_the_gained_treasure_on_the_deck() {
    let mut g = new_state(&[id::REPLACE, id::TIARA], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::ESTATE]);
    set_in_play(&mut g, 0, &[id::TIARA]);
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Pass);
    assert_eq!(top_down(&g, 0), vec![id::SILVER]);
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn watchtower_trashing_replaces_gain_stops_the_topdeck_and_spares_an_older_copy() {
    let mut g = new_state(&[id::REPLACE, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::ESTATE, id::WATCHTOWER]);
    set_discard(&mut g, 0, &[id::SILVER]); // an older Silver must not be mistaken for the gained one
    // Replace's trash: two cards in hand, so it asks.
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::SILVER));
    let d = g.pending_decision().expect("Watchtower's offer");
    assert_eq!(d.source, Some(id::WATCHTOWER));
    choose(&mut g, Choice::Card(id::SILVER)); // trash the gained Silver
    assert_eq!(g.trash.get(id::SILVER), 1);
    // Watchtower's topdeck offer cannot tell the older Silver from the gained one: decline it.
    choose(&mut g, Choice::Pass);
    assert!(g.players[0].deck_known.is_empty(), "Replace put nothing onto the deck");
    assert_eq!(g.players[0].discard.get(id::SILVER), 1, "the older Silver stays in the discard pile");
}

// ===========================================================================
// Sailor: one use per play
// ===========================================================================








// ===========================================================================
// Moat is a choice
// ===========================================================================












