//! Rules that cross expansions: a card of one set changing how a card of another behaves
//! (Charlatan's Curse-as-Treasure for Ironworks, Replace and Courtier; Corsair trashing the Silver
//! Crystal Ball plays). Found by checking the cards against the official rulebooks' FAQs and the
//! DominionStrategy wiki's rules pages (the plan docs, `docs/intrigue-plan.md`,
//! `docs/seaside-prosperity-plan.md` and `docs/alchemy-plan.md`, have a "Rules audit" section each).
//! The rule tests about one card live in that card's set file (`base_cards.rs`, ...).

mod common;
use common::*;
use dominion_engine::cards::id;
use dominion_engine::*;




// ===========================================================================
// Setup and end of game
// ===========================================================================



// ===========================================================================
// Mint, Quarry, Hoard, Collection, Tiara: gains and "this turn" effects
// ===========================================================================







// ===========================================================================
// Investment
// ===========================================================================


// ===========================================================================
// Charlatan: Curse is a Treasure for every purpose
// ===========================================================================

#[test]
fn ironworks_gaining_a_curse_gives_a_coin_when_charlatan_makes_it_a_treasure() {
    for (kingdom, bonus) in [([id::IRONWORKS, id::CHARLATAN], 1), ([id::IRONWORKS, id::MOAT], 0)] {
        let mut g = new_state(&kingdom, 2);
        set_hand(&mut g, 0, &[id::IRONWORKS]);
        play(&mut g, id::IRONWORKS);
        choose(&mut g, Choice::Card(id::CURSE));
        assert_eq!(g.turn.coins, bonus, "{kingdom:?}");
        assert_eq!(g.turn.actions, 0);
    }
}

#[test]
fn replace_puts_a_gained_curse_on_the_deck_when_charlatan_makes_it_a_treasure() {
    let mut g = new_state(&[id::REPLACE, id::CHARLATAN], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::COPPER]);
    play(&mut g, id::REPLACE); // the Copper is the only card to trash: trashed without asking
    g.apply(Choice::Card(id::CURSE), &mut NoEvents).unwrap(); // gain a Curse ($0 <= $2)
    adv(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::CURSE));
    assert!(!g.players[0].discard.has(id::CURSE));

    let mut g = new_state(&[id::REPLACE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::COPPER]);
    play(&mut g, id::REPLACE);
    g.apply(Choice::Card(id::CURSE), &mut NoEvents).unwrap();
    adv(&mut g);
    assert!(g.players[0].discard.has(id::CURSE), "without Charlatan a Curse is not a Treasure");
}

#[test]
fn courtier_counts_a_curse_as_two_types_when_charlatan_makes_it_a_treasure() {
    for (kingdom, picks) in [([id::COURTIER, id::CHARLATAN], 2), ([id::COURTIER, id::MOAT], 1)] {
        let mut g = new_state(&kingdom, 2);
        set_hand(&mut g, 0, &[id::COURTIER, id::CURSE]);
        // The Curse is the only card left in hand, so it is revealed without asking.
        let Step::Decision(d) = play(&mut g, id::COURTIER) else { panic!() };
        assert_eq!(d.kind, DecisionKind::Mode { picks, distinct: picks > 1 }, "{kingdom:?}");
    }
}

// ===========================================================================
// Attacks that linger (Corsair, Blockade): immunity counts when they are played
// ===========================================================================




#[test]
fn corsair_trashes_a_silver_that_crystal_ball_plays() {
    let mut g = new_state(&[id::CORSAIR, id::CRYSTAL_BALL], 2);
    set_in_play(&mut g, 0, &[id::CORSAIR]);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::CRYSTAL_BALL]);
    set_deck_known(&mut g, 1, &[id::SILVER]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::CRYSTAL_BALL);
    choose(&mut g, Choice::Card(id::SILVER)); // play it
    assert!(g.trash.has(id::SILVER), "the first Silver or Gold played, by any means");
    assert_eq!(g.turn.coins, 1 + 2, "the coins were made first");
}


// ===========================================================================
// Monkey
// ===========================================================================



// ===========================================================================
// Reactions and attacks: Diplomat, Clerk, Pirate
// ===========================================================================







// ===========================================================================
// Herbalist and Astrolabe
// ===========================================================================


