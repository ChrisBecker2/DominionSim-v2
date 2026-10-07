//! Reactions that act on a card gained straight onto the deck (Watchtower trashes it, Sailor plays
//! it), Replace's gain-then-topdeck order, Sailor's per-play uses under Throne Room / King's Court,
//! and Moat as a choice (revealed or not, per Attack, also when it arrives after the Attack was
//! played). See `docs/seaside-prosperity-plan.md` §6, `docs/intrigue-plan.md` §7 and
//! `docs/alchemy-plan.md` §8 for the rulings.

mod common;
use common::*;
use dominion_engine::cards::{self, id};
use dominion_engine::state::{self, Frame, FrameKind};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn top_down(g: &GameState, p: usize) -> Vec<CardId> {
    g.players[p].deck_known.iter_top_down().collect()
}

/// Answer Diplomat's "discard 3" with the lowest-numbered card each time (never the Moat, which is
/// numbered above Copper and Estate).
fn discard_forced(g: &mut GameState) {
    let mut n = 0;
    while let Some(Decision { kind: DecisionKind::Select { act: Act::Discard, .. }, .. }) = g.pending_decision() {
        let c = choices(g)[0];
        choose(g, c);
        n += 1;
    }
    assert!(n >= 1);
}

fn is_moat_decision(d: &Decision) -> bool {
    matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }) && d.subject == id::MOAT
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

fn sailor(g: &GameState) -> state::PendingDuration {
    let ps = &g.players[0];
    ps.pending_durations[..ps.pending_durations_len as usize].iter().find(|e| e.card == id::SAILOR).copied().expect("a Sailor entry")
}

/// Workshop gains a Caravan (a Duration): the Sailor offer follows. Returns whether one came.
fn workshop_gains_caravan(g: &mut GameState) -> bool {
    play(g, id::WORKSHOP);
    let step = choose(g, Choice::Card(id::CARAVAN));
    matches!(step, Step::Decision(Decision { kind: DecisionKind::YesNo { act: Act::Play }, source: Some(id::SAILOR), .. }))
}

#[test]
fn a_throne_roomed_sailor_plays_two_gained_durations_not_three() {
    let mut g = new_state(&[id::THRONE_ROOM, id::SAILOR, id::WORKSHOP, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SAILOR, id::WORKSHOP, id::WORKSHOP, id::WORKSHOP]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SAILOR));
    let e = sailor(&g);
    assert_eq!((e.times, e.used), (2, 0));
    assert!(workshop_gains_caravan(&mut g));
    choose(&mut g, Choice::Yes);
    assert_eq!(sailor(&g).used, 1);
    assert!(workshop_gains_caravan(&mut g));
    choose(&mut g, Choice::Yes);
    assert_eq!(sailor(&g).used, 2);
    assert_eq!(g.players[0].in_play.get(id::CARAVAN), 2);
    assert!(!workshop_gains_caravan(&mut g), "the third gained Duration is not offered");
    assert_eq!(g.players[0].discard.get(id::CARAVAN), 1);
}

#[test]
fn a_kings_courted_sailor_plays_three_gained_durations_not_four() {
    let mut g = new_state(&[id::KINGS_COURT, id::SAILOR, id::WORKSHOP, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::SAILOR, id::WORKSHOP, id::WORKSHOP, id::WORKSHOP, id::WORKSHOP]);
    play(&mut g, id::KINGS_COURT);
    choose(&mut g, Choice::Card(id::SAILOR));
    assert_eq!(sailor(&g).times, 3);
    for _ in 0..3 {
        assert!(workshop_gains_caravan(&mut g));
        choose(&mut g, Choice::Yes);
    }
    assert!(!workshop_gains_caravan(&mut g));
    assert_eq!(g.players[0].in_play.get(id::CARAVAN), 3);
}

#[test]
fn two_separate_sailors_give_one_use_each() {
    let mut g = new_state(&[id::SAILOR, id::WORKSHOP, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::SAILOR, id::SAILOR, id::WORKSHOP, id::WORKSHOP, id::WORKSHOP]);
    play(&mut g, id::SAILOR);
    play(&mut g, id::SAILOR);
    assert!(workshop_gains_caravan(&mut g));
    choose(&mut g, Choice::Yes);
    assert!(workshop_gains_caravan(&mut g));
    choose(&mut g, Choice::Yes);
    assert!(!workshop_gains_caravan(&mut g));
    assert_eq!(g.players[0].in_play.get(id::CARAVAN), 2);
}

#[test]
fn declining_the_offer_still_spends_that_sailors_use() {
    let mut g = new_state(&[id::SAILOR, id::WORKSHOP, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::SAILOR, id::WORKSHOP, id::WORKSHOP]);
    play(&mut g, id::SAILOR);
    g.turn.actions = 3;
    assert!(workshop_gains_caravan(&mut g));
    choose(&mut g, Choice::No);
    assert!(!workshop_gains_caravan(&mut g));
}

#[test]
fn sailors_partly_spent_uses_survive_the_text_format_and_old_saves_still_load() {
    let mut g = new_state(&[id::THRONE_ROOM, id::SAILOR, id::WORKSHOP, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SAILOR, id::WORKSHOP, id::WORKSHOP]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SAILOR));
    assert!(workshop_gains_caravan(&mut g));
    choose(&mut g, Choice::Yes);
    // Back at the PlayAction decision, one of two uses spent.
    expect_decision(&mut g);
    let text = format_state(&g);
    assert!(text.contains("Sailor x2 (used 1)"), "{text}");
    let back = parse_state(&text).unwrap();
    let find = |g: &GameState| sailor(g);
    assert_eq!(find(&back), find(&g));
    assert_eq!((find(&back).times, find(&back).used), (2, 1));

    // The older marker: "(used)" meant every use was spent.
    let old = parse_state(&text.replace("Sailor x2 (used 1)", "Sailor x2 (used)")).unwrap();
    assert_eq!((find(&old).times, find(&old).used), (2, 2));
    let single = parse_state(&text.replace("Sailor x2 (used 1)", "Sailor (used)")).unwrap();
    assert_eq!((find(&single).times, find(&single).used), (1, 1));
    let fresh = parse_state(&text.replace("Sailor x2 (used 1)", "Sailor x2")).unwrap();
    assert_eq!(find(&fresh).used, 0);
}

// ===========================================================================
// Moat is a choice
// ===========================================================================

#[test]
fn moat_declined_so_the_attack_hits() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::WITCH);
    moat(&mut g, false);
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
    assert_eq!(g.players[1].hand, counts_of(&[id::MOAT]));
    assert_eq!(g.players[0].hand.total(), 2, "Witch's +2 Cards either way");
}

#[test]
fn moat_revealed_so_the_attack_is_blocked_and_the_reveal_is_logged() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT]);
    let Step::Decision(d) = play(&mut g, id::WITCH) else { panic!() };
    assert_eq!((d.player, d.source, d.subject), (1, Some(id::WITCH), id::MOAT), "asked of the attacked player, naming the Attack");
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Reveal });
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Yes, &mut ev);
    assert!(g.players[1].discard.is_empty());
    assert!(g.players[1].hand.has(id::MOAT), "Moat stays in the hand");
    assert!(ev.iter().any(|e| matches!(e, Event::Reaction { player: 1, card: id::MOAT })), "{ev:?}");
}

#[test]
fn a_player_without_a_reaction_is_never_asked() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE]);
    let step = play(&mut g, id::WITCH);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "{step:?}");
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
}

#[test]
fn each_attacked_player_chooses_for_themselves_in_turn_order() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 3);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_hand(&mut g, 2, &[id::MOAT]);
    let Step::Decision(d) = play(&mut g, id::WITCH) else { panic!() };
    assert_eq!(d.player, 1);
    let Step::Decision(d) = choose(&mut g, Choice::Yes) else { panic!() };
    assert!(is_moat_decision(&d));
    assert_eq!(d.player, 2);
    choose(&mut g, Choice::No);
    assert!(g.players[1].discard.is_empty());
    assert_eq!(g.players[2].discard, counts_of(&[id::CURSE]));
}

#[test]
fn moat_is_chosen_again_for_every_attack() {
    // Throne Room plays Militia twice: reveal for the first, not the second.
    let mut g = new_state(&[id::THRONE_ROOM, id::MILITIA, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MILITIA]);
    let victim = [id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::COPPER];
    set_hand(&mut g, 1, &victim);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MILITIA));
    moat(&mut g, true);
    assert_eq!(g.players[1].hand, counts_of(&victim), "the first play is blocked");
    moat(&mut g, false);
    // Discard down to 3: the player to choose is player 1.
    let d = g.pending_decision().expect("discard");
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }));
}

#[test]
fn a_moat_drawn_by_a_diplomat_reaction_can_still_be_revealed() {
    let mut g = new_state(&[id::WITCH, id::MOAT, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::MOAT, id::COPPER]);
    // Diplomat comes first: it is the only Reaction in the hand when the Witch is played.
    let Step::Decision(d) = play(&mut g, id::WITCH) else { panic!() };
    assert_eq!((d.player, d.subject), (1, id::DIPLOMAT));
    choose(&mut g, Choice::Yes); // draw Moat and Copper, then discard 3
    discard_forced(&mut g);
    assert!(g.players[1].hand.has(id::MOAT));
    let d = expect_decision(&mut g);
    assert!(is_moat_decision(&d), "the new Moat is offered: {d:?}");
    choose(&mut g, Choice::Yes);
    assert!(!g.players[1].discard.has(id::CURSE), "blocked");
}

#[test]
fn a_moat_that_never_arrives_is_not_offered_and_the_attack_hits() {
    let mut g = new_state(&[id::WITCH, id::MOAT, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::COPPER]);
    play(&mut g, id::WITCH);
    choose(&mut g, Choice::Yes);
    discard_forced(&mut g);
    let d = expect_decision(&mut g);
    assert!(!is_moat_decision(&d), "{d:?}");
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
}

#[test]
fn lighthouse_still_protects_without_any_decision() {
    let mut g = new_state(&[id::WITCH, id::MOAT, id::LIGHTHOUSE], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_in_play(&mut g, 1, &[id::LIGHTHOUSE]);
    let step = play(&mut g, id::WITCH);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no question asked: {step:?}");
    assert!(g.players[1].discard.is_empty());
}

#[test]
fn moat_declined_against_minion_leaves_the_player_with_a_new_hand_of_four() {
    let mut g = new_state(&[id::MINION, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MINION]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::COPPER; 4]);
    play(&mut g, id::MINION);
    moat(&mut g, false);
    let d = expect_decision(&mut g);
    choose(&mut g, Choice::Mode(minion_redraw_mode(&d)));
    assert_eq!(g.players[1].hand, counts_of(&[id::GOLD; 4]), "the declined Moat went with the old hand");
    assert_eq!(g.players[1].discard.get(id::MOAT), 1);
}

/// The index of Minion's "discard your hand, +4 Cards" mode among the pending Mode decision's options.
fn minion_redraw_mode(d: &Decision) -> u8 {
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    cards::modes(id::MINION)
        .iter()
        .position(|o| matches!(o, cards::ModeOpt::DiscardHandDraw { .. }))
        .expect("Minion's redraw option") as u8
}

#[test]
fn attacked_players_share_one_reveal_per_attack_the_second_moat_copy_is_not_asked() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT, id::MOAT]);
    play(&mut g, id::WITCH);
    let step = moat(&mut g, true);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "{step:?}");
}

