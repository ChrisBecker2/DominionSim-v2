//! `Event::Bonus`: the vanilla bonuses (+Actions, +Buys, +$, +VP tokens) a card gives are
//! reported once per application, never as +Cards (those are Draw events), and a Duration
//! firing at the start of its owner's turn is always reported.

mod common;
use common::*;
use dominion_engine::cards::id;
use dominion_engine::state::PendingDuration;
use dominion_engine::*;

fn bonuses(events: &[Event]) -> Vec<(u8, u8, u8, u16, u16)> {
    events
        .iter()
        .filter_map(|e| match *e {
            Event::Bonus { source, actions, buys, coins, vp, .. } => Some((source, actions, buys, coins, vp)),
            _ => None,
        })
        .collect()
}

#[test]
fn village_and_market_report_their_bonuses_once() {
    let mut g = new_state(&[id::VILLAGE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::MARKET]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::VILLAGE, &mut ev);
    assert_eq!(bonuses(&ev), vec![(id::VILLAGE, 2, 0, 0, 0)]);
    ev.clear();
    play_ev(&mut g, id::MARKET, &mut ev);
    assert_eq!(bonuses(&ev), vec![(id::MARKET, 1, 1, 1, 0)]);
}

#[test]
fn merchant_bonus_is_reported_on_the_first_silver_only() {
    let mut g = new_state(&[id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::MERCHANT, id::SILVER, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::MERCHANT, &mut ev);
    // The Treasures are auto-played on entering the Buy phase, right after the Merchant.
    // Merchant's own +1 Action, then +$1 once; a Silver's printed $2 is not a bonus.
    assert_eq!(bonuses(&ev), vec![(id::MERCHANT, 1, 0, 0, 0), (id::MERCHANT, 0, 0, 1, 0)]);
    assert_eq!(g.turn.coins, 6); // Silver, Silver, the drawn Copper, Merchant's +$1
}

#[test]
fn monument_reports_coins_and_a_vp_token() {
    let mut g = new_state(&[id::MONUMENT], 2);
    set_hand(&mut g, 0, &[id::MONUMENT]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::MONUMENT, &mut ev);
    assert_eq!(bonuses(&ev), vec![(id::MONUMENT, 0, 0, 2, 0), (id::MONUMENT, 0, 0, 0, 1)]);
}

#[test]
fn astrolabe_reports_now_and_at_turn_start() {
    let mut g = new_state(&[id::ASTROLABE], 2);
    set_hand(&mut g, 0, &[id::ASTROLABE, id::ESTATE]);
    let mut ev: Vec<Event> = Vec::new();
    g.advance(&mut ev);
    assert_eq!(bonuses(&ev), vec![(id::ASTROLABE, 0, 1, 1, 0)]);
    // Next turn: a pending Astrolabe fires with the same bonus, reported even at depth 0.
    let mut g = new_state(&[id::ASTROLABE], 2);
    let i = g.players[0].pending_durations_len as usize;
    g.players[0].pending_durations[i] = PendingDuration { card: id::ASTROLABE, times: 1, arg: 0, used: false };
    g.players[0].pending_durations_len += 1;
    let mut ev: Vec<Event> = Vec::new();
    g.advance(&mut ev);
    assert_eq!(bonuses(&ev), vec![(id::ASTROLABE, 0, 1, 1, 0)]);
}

#[test]
fn a_duration_with_no_vanilla_bonus_still_reports_firing_before_its_draw() {
    let mut g = new_state(&[id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER]);
    let i = g.players[0].pending_durations_len as usize;
    g.players[0].pending_durations[i] = PendingDuration { card: id::CARAVAN, times: 1, arg: 0, used: false };
    g.players[0].pending_durations_len += 1;
    let mut ev: Vec<Event> = Vec::new();
    g.advance(&mut ev);
    let fire = ev.iter().position(|e| matches!(e, Event::Bonus { source, .. } if *source == id::CARAVAN)).expect("firing reported");
    let draw = ev.iter().position(|e| matches!(e, Event::Draw { card, .. } if *card == id::GOLD)).expect("draw reported");
    assert!(fire < draw);
}
