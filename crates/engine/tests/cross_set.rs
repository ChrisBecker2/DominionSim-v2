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




// ===========================================================================
// Renaissance with the other sets
// ===========================================================================

fn own_project(g: &mut GameState, p: usize, project: CardId) {
    let bit = 1u32 << (project - dominion_engine::cards::FIRST_PROJECT);
    g.players[p].ren.projects |= bit;
    g.ren.projects |= bit;
    g.ren_update_hooks();
}

#[test]
fn throne_room_on_seer_plays_it_twice_and_each_play_reveals_three_cards() {
    let mut g = new_state(&[id::THRONE_ROOM, id::SEER, id::VILLAGE, id::SMITHY], 2);
    g.turn.actions = 1;
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SEER]);
    // First Seer: +1 Card (Gold), reveals Estate, Village, Smithy: the Village and the Smithy (cost $3 and $4) come to the hand.
    // Second Seer: +1 Card (Duchy), reveals three Golds: nothing costs $2 to $4, all three go back.
    set_deck_known(&mut g, 0, &[id::GOLD, id::ESTATE, id::VILLAGE, id::SMITHY, id::DUCHY, id::GOLD, id::GOLD, id::GOLD]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SEER));
    assert_eq!(g.players[0].hand.get(id::VILLAGE) + g.players[0].hand.get(id::SMITHY), 2);
    assert_eq!(g.players[0].hand.get(id::GOLD) + g.players[0].hand.get(id::DUCHY), 2, "one card from each play");
    assert_eq!(g.turn.actions, 2, "two plays, each +1 Action");
}

#[test]
fn citadel_replays_a_throne_room_which_plays_its_action_twice_again() {
    let mut g = new_state(&[id::CITADEL, id::THRONE_ROOM, id::SMITHY], 2);
    own_project(&mut g, 0, id::CITADEL);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SMITHY, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SMITHY));
    // Throne Room's two plays of the first Smithy: +6; the Citadel replays the Throne Room, which asks again (the second Smithy): +6.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { .. }), "{d:?}");
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].hand.total(), 12);
    assert_eq!(g.players[0].in_play.total(), 3);
}

#[test]
fn capitalism_treasures_are_gold_for_counting_and_scepter_replays_them_as_actions() {
    // A Market is a Treasure on the owner's turn under Capitalism but is still an Action: a Scepter can replay it,
    // and Hoard-like Treasure triggers use is_treasure.
    let mut g = new_state(&[id::MARKET, id::SCEPTER, id::MINE], 2);
    own_project(&mut g, 0, id::CAPITALISM);
    assert!(g.is_treasure(id::MARKET));
    assert!(g.is_action(id::MARKET));
    reset_turn(&mut g, 1);
    assert!(!g.is_treasure(id::MARKET), "only on its owner's turns");
}

#[test]
fn replace_into_skulk_gains_a_gold_and_still_puts_the_skulk_on_the_deck() {
    // The stop-moving rule (2019): "Gained cards are expected to be where they were gained to"; "cards in discard piles can be moved even if
    // covered up". Replacing an Estate into a Skulk: the Skulk's gain covers it with a Gold, and Replace still puts the Skulk onto the deck.
    let mut g = new_state(&[id::REPLACE, id::SKULK], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::ESTATE, id::DUCHY]);
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::SKULK));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SKULK), "the Skulk went onto the deck");
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]), "its Gold stays in the discard pile");
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
}

