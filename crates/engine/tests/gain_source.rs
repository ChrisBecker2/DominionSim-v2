//! `Event::Gain` says where a gain came from: the card whose effect or trigger caused it, or
//! `NO_SOURCE` for a plain buy.

mod common;
use common::*;
use dominion_engine::cards::id;
use dominion_engine::*;

fn gains(ev: &[Event]) -> Vec<(u8, CardId, CardId)> {
    ev.iter().filter_map(|e| if let Event::Gain { player, card, source, .. } = *e { Some((player, card, source)) } else { None }).collect()
}

#[test]
fn a_plain_buy_has_no_source() {
    let mut g = new_state(&[id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::SILVER]);
    expect_decision(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut ev);
    assert_eq!(gains(&ev), vec![(0, id::SILVER, NO_SOURCE)]);
}

#[test]
fn hoards_gold_names_hoard() {
    let mut g = new_state(&[id::HOARD], 2);
    set_hand(&mut g, 0, &[id::HOARD]);
    expect_decision(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::ESTATE), &mut ev);
    assert_eq!(gains(&ev), vec![(0, id::ESTATE, NO_SOURCE), (0, id::GOLD, id::HOARD)]);
}

#[test]
fn witchs_curse_names_the_witch() {
    let mut g = new_state(&[id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    let mut ev: Vec<Event> = Vec::new();
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    choose_ev(&mut g, Choice::Card(id::WITCH), &mut ev);
    assert_eq!(gains(&ev), vec![(1, id::CURSE, id::WITCH)]);
}

#[test]
fn card_effect_gains_name_their_cards() {
    // Familiar's Curse, Transmute's Gold and Workshop's gain.
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::FAMILIAR]);
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(id::FAMILIAR), &mut ev);
    assert_eq!(gains(&ev), vec![(1, id::CURSE, id::FAMILIAR)]);

    let mut g = new_state(&[id::TRANSMUTE], 2);
    set_hand(&mut g, 0, &[id::TRANSMUTE, id::ESTATE]);
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(id::TRANSMUTE), &mut ev);
    assert_eq!(gains(&ev), vec![(0, id::GOLD, id::TRANSMUTE)]);

    let mut g = new_state(&[id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(id::WORKSHOP), &mut ev);
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut ev);
    assert_eq!(gains(&ev), vec![(0, id::SILVER, id::WORKSHOP)]);

    // Bureaucrat's Silver, onto the deck.
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(id::BUREAUCRAT), &mut ev);
    assert_eq!(gains(&ev)[0], (0, id::SILVER, id::BUREAUCRAT));
}

#[test]
fn a_gain_from_the_trash_names_its_source() {
    // Lurker gains an Action from the trash.
    let mut g = new_state(&[id::LURKER], 2);
    g.trash.add(id::VILLAGE, 1);
    set_hand(&mut g, 0, &[id::LURKER]);
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(id::LURKER), &mut ev);
    // Mode: the second option gains from the trash.
    choose_ev(&mut g, Choice::Mode(1), &mut ev);
    assert!(gains(&ev).contains(&(0, id::VILLAGE, id::LURKER)), "{:?}", gains(&ev));
}
