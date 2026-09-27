//! `chance_mode`: exact-search support for drawing from an unknown deck.

mod common;
use common::*;
use dominion_engine::cards::id;
use dominion_engine::*;

#[test]
fn draws_from_deck_known_never_produce_chance() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.chance_mode = true;
    set_hand(&mut g, 0, &[id::SMITHY]);
    // Non-treasure fillers so they aren't auto-played once the turn rolls into Buy.
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::CURSE]);
    let step = play(&mut g, id::SMITHY);
    assert!(matches!(step, Step::Decision(_)), "fully known draws never need Chance");
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::CURSE]));
}

#[test]
fn drawing_from_the_unknown_multiset_yields_a_chance_step() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.chance_mode = true;
    set_hand(&mut g, 0, &[id::SMITHY]);
    clear_deck(&mut g, 0);
    set_deck_unknown(&mut g, 0, &[id::COPPER, id::COPPER, id::SILVER]);
    set_discard(&mut g, 0, &[]);

    let step = play(&mut g, id::SMITHY);
    match step {
        Step::Chance { player } => {
            assert_eq!(player, 0);
            assert_eq!(g.chance_outcomes(player), counts_of(&[id::COPPER, id::COPPER, id::SILVER]));
            g.resolve_chance(player, id::SILVER);
        }
        other => panic!("expected a Chance step, got {other:?}"),
    }
    let next = adv(&mut g);
    assert!(g.players[0].hand.has(id::SILVER), "resolve_chance placed the card and play continued");
    assert!(matches!(next, Step::Decision(_) | Step::Chance { .. }));
}

#[test]
fn single_unknown_card_still_yields_a_chance_step_with_one_outcome() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.chance_mode = true;
    set_hand(&mut g, 0, &[id::SMITHY]);
    clear_deck(&mut g, 0);
    set_deck_unknown(&mut g, 0, &[id::DUCHY]);
    set_discard(&mut g, 0, &[]);

    let step = play(&mut g, id::SMITHY);
    match step {
        Step::Chance { player } => {
            let outcomes = g.chance_outcomes(player);
            assert_eq!(outcomes, counts_of(&[id::DUCHY]), "the only possible outcome");
            g.resolve_chance(player, id::DUCHY);
        }
        other => panic!("expected a Chance step, got {other:?}"),
    }
    adv(&mut g);
    assert!(g.players[0].hand.has(id::DUCHY));
}

#[test]
fn chance_mode_off_by_default_samples_directly() {
    let mut g = new_state(&[id::SMITHY], 2);
    assert!(!g.chance_mode);
    set_hand(&mut g, 0, &[id::SMITHY]);
    clear_deck(&mut g, 0);
    set_deck_unknown(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    let step = play(&mut g, id::SMITHY);
    assert!(matches!(step, Step::Decision(_)), "no Chance ever surfaces without chance_mode");
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn chance_mode_resolves_a_full_reshuffle_mid_draw() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.chance_mode = true;
    set_hand(&mut g, 0, &[id::SMITHY]);
    clear_deck(&mut g, 0);
    set_deck_unknown(&mut g, 0, &[id::ESTATE]);
    set_discard(&mut g, 0, &[id::DUCHY, id::CURSE]);

    // 1st draw: the lone known-unknown Copper (still needs a Chance answer).
    let mut step = play(&mut g, id::SMITHY);
    let mut drawn = 0;
    loop {
        match step {
            Step::Chance { player } => {
                let outcomes = g.chance_outcomes(player);
                let card = outcomes.iter().next().expect("nonempty outcomes").0;
                g.resolve_chance(player, card);
                step = adv(&mut g);
            }
            Step::Decision(_) | Step::GameOver => break,
        }
        drawn += 1;
        assert!(drawn <= 3, "smithy only draws 3 cards");
    }
    assert_eq!(g.players[0].hand.total(), 3, "1 known-unknown + 2 from the reshuffled discard");
    assert!(g.players[0].discard.is_empty());
}
