//! Turn structure, supply sizing, end-of-game conditions and generic engine-API guarantees
//! that aren't specific to any one card.

mod common;
use common::*;
use dominion_engine::cards::id;
use dominion_engine::*;

// ---------------------------------------------------------------------------
// Supply sizes
// ---------------------------------------------------------------------------

#[test]
fn supply_sizes_by_player_count() {
    // Province: 8/12/12/15/18 for 2..6 players. Copper: 60-7n. Curse: 10*(n-1).
    // Estate/Duchy: 8 for <=2 players, else 12.
    let expected: [(usize, u8, u8, u8, u8); 5] = [
        (2, 8, 8, 46, 10),
        (3, 12, 12, 39, 20),
        (4, 12, 12, 32, 30),
        (5, 12, 15, 25, 40),
        (6, 12, 18, 18, 50),
    ];
    for (n, estate, province, copper, curse) in expected {
        let g = new_state(&[id::CELLAR], n);
        assert_eq!(g.supply.get(id::ESTATE), estate, "estate pile for {n} players");
        assert_eq!(g.supply.get(id::DUCHY), estate, "duchy pile for {n} players");
        assert_eq!(g.supply.get(id::PROVINCE), province, "province pile for {n} players");
        assert_eq!(g.supply.get(id::COPPER), copper, "copper pile for {n} players");
        assert_eq!(g.supply.get(id::CURSE), curse, "curse pile for {n} players");
        assert_eq!(g.supply.get(id::SILVER), 40);
        assert_eq!(g.supply.get(id::GOLD), 30);
    }
}

#[test]
fn kingdom_action_piles_are_ten_kingdom_victory_piles_match_estate() {
    let g = new_state(&[id::SMITHY, id::GARDENS], 4);
    assert_eq!(g.supply.get(id::SMITHY), 10);
    assert_eq!(g.supply.get(id::GARDENS), 12, "Gardens is a Victory card, sized like Estate");
}

// ---------------------------------------------------------------------------
// Game end conditions
// ---------------------------------------------------------------------------

#[test]
fn game_ends_when_provinces_run_out() {
    let mut g = new_state(&[id::CELLAR], 2);
    assert!(!g.end_condition_met());
    set_supply(&mut g, id::PROVINCE, 0);
    assert!(g.end_condition_met());
}

#[test]
fn game_ends_at_three_empty_piles_for_up_to_four_players() {
    let mut g = new_state(&[id::CELLAR, id::CHAPEL, id::MOAT], 4);
    empty_pile(&mut g, id::CELLAR);
    empty_pile(&mut g, id::CHAPEL);
    assert!(!g.end_condition_met(), "only 2 empty piles");
    empty_pile(&mut g, id::MOAT);
    assert!(g.end_condition_met(), "3 empty piles ends it for <=4 players");
}

#[test]
fn game_needs_four_empty_piles_for_five_or_more_players() {
    let mut g = new_state(&[id::CELLAR, id::CHAPEL, id::MOAT, id::VILLAGE], 5);
    empty_pile(&mut g, id::CELLAR);
    empty_pile(&mut g, id::CHAPEL);
    empty_pile(&mut g, id::MOAT);
    assert!(!g.end_condition_met(), "only 3 empty piles, need 4 with 5 players");
    empty_pile(&mut g, id::VILLAGE);
    assert!(g.end_condition_met());
}

#[test]
fn ties_are_broken_by_fewer_turns_taken() {
    let mut g = new_state(&[id::CELLAR], 2);
    g.players[0].turns_taken = 5;
    g.players[1].turns_taken = 6;
    assert_eq!(g.scores()[0], g.scores()[1], "equal VP by construction (same starting deck)");
    assert_eq!(g.winners(), 0b01, "fewer turns taken wins the tie");
}

#[test]
fn true_ties_share_the_win() {
    let mut g = new_state(&[id::CELLAR], 2);
    g.players[0].turns_taken = 5;
    g.players[1].turns_taken = 5;
    assert_eq!(g.winners(), 0b11);
}

// ---------------------------------------------------------------------------
// Action / Buy / Cleanup mechanics
// ---------------------------------------------------------------------------

#[test]
fn treasures_auto_play_when_entering_the_buy_phase() {
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD]);
    let step = adv(&mut g); // no actions in hand -> straight to Buy
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 6);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].in_play, counts_of(&[id::COPPER, id::SILVER, id::GOLD]));
}

#[test]
fn reshuffle_only_happens_when_drawing_from_an_empty_deck() {
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]), "3 known cards sufficed; no reshuffle");
    assert!(g.players[0].deck_known.is_empty());
}

#[test]
fn cleanup_discards_hand_and_play_area_then_draws_five() {
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::ESTATE]);
    clear_deck(&mut g, 0);
    set_deck_unknown(&mut g, 0, &[id::COPPER; 10]);
    set_discard(&mut g, 0, &[]);

    adv(&mut g); // Action phase, no actions -> Buy (Copper auto-played)
    pass(&mut g); // Buy phase, buy nothing -> cleanup -> draw 5 -> next player's turn

    assert_eq!(g.players[0].hand.total(), 5);
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::ESTATE]), "old hand + play area");
    assert_eq!(g.players[0].deck_unknown.total(), 5);
    assert_eq!(g.players[0].in_play.total(), 0);
}

// ---------------------------------------------------------------------------
// Generic engine-API guarantees
// ---------------------------------------------------------------------------

#[test]
fn apply_rejects_illegal_choices_without_changing_state() {
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY, id::COPPER]);
    adv(&mut g); // reach the PlayAction decision
    let snapshot = g;

    // Gold is not a legal PlayAction choice (not an Action card, not even in hand).
    let err = g.apply(Choice::Card(id::GOLD), &mut NoEvents);
    assert!(err.is_err());
    assert!(g == snapshot, "state must be unchanged after a rejected choice");

    // Copper is in hand but isn't an Action card either.
    let err2 = g.apply(Choice::Card(id::COPPER), &mut NoEvents);
    assert!(err2.is_err());
    assert!(g == snapshot);
}

#[test]
fn auto_single_applies_the_only_legal_choice() {
    let mut g = new_state(&[id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM]);
    // With auto_single (the default), Throne Room with no other Action in hand skips
    // straight past its own "Pass"-only decision.
    let next = play(&mut g, id::THRONE_ROOM);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn auto_single_can_be_turned_off() {
    // Militia victim holding 4 Coppers must discard 1: the only legal choice is Copper.
    let mut g = new_state(&[id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    let auto = play(&mut g.clone(), id::MILITIA);
    assert!(matches!(auto, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "forced discard auto-applied");

    g.auto_single = false;
    let next = play(&mut g, id::MILITIA);
    assert!(matches!(
        next,
        Step::Decision(Decision { player: 1, kind: DecisionKind::Select { from: Zone::Hand, act: Act::Discard, min: 1, max: 1, .. }, .. })
    ));
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER)]);
}

#[test]
fn poacher_canonical_ordering_never_dead_ends() {
    // 3 empty piles, hand of 3 Coppers + Estate (+1 drawn Silver) = must discard 3 of 5.
    let mut g = new_state(&[id::POACHER, id::CELLAR, id::CHAPEL, id::MOAT], 2);
    empty_pile(&mut g, id::CELLAR);
    empty_pile(&mut g, id::CHAPEL);
    empty_pile(&mut g, id::MOAT);
    set_hand(&mut g, 0, &[id::POACHER, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    g.auto_single = false;
    play(&mut g, id::POACHER);
    // Any 3 of {3 Copper, Silver, Estate} includes a Copper, and canonical (non-decreasing)
    // order means the first pick must be Copper. Pick the *largest* offered card each time
    // to probe for dead ends.
    let mut picks = 0;
    while let Some(Decision { kind: DecisionKind::Select { act: Act::Discard, .. }, .. }) = g.pending_decision() {
        let cs = choices(&g);
        if picks == 0 {
            assert_eq!(cs, vec![Choice::Card(id::COPPER)]);
        }
        let pick = *cs.iter().filter(|c| matches!(c, Choice::Card(_))).last().expect("never dead-ends");
        choose(&mut g, pick);
        picks += 1;
    }
    assert_eq!(picks, 3);
    assert_eq!(g.players[0].discard.total(), 3, "3 of 5 discarded");
}
