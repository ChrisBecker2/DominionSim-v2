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


// ===========================================================================
// The stop-moving rule (docs/stop-moving-rule.md): effects that move a card they put somewhere, across expansions
// ===========================================================================

#[test]
fn vassal_cannot_play_a_discarded_hound_that_its_own_reaction_set_aside() {
    // Vassal discards a Faithful Hound; the Hound sets itself aside. Another Hound lies in the discard pile, but it is not the card Vassal
    // discarded: Vassal can't play it.
    let mut g = new_state(&[id::VASSAL, id::FAITHFUL_HOUND], 2);
    set_hand(&mut g, 0, &[id::VASSAL]);
    set_discard(&mut g, 0, &[id::FAITHFUL_HOUND]);
    set_deck_known(&mut g, 0, &[id::FAITHFUL_HOUND, id::COPPER]);
    play(&mut g, id::VASSAL);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::SetAside });
    choose(&mut g, Choice::Yes);
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { act: Act::Play })), "{:?}", g.pending_decision());
    assert_eq!(g.players[0].hounds, 1);
    assert_eq!(g.players[0].discard.get(id::FAITHFUL_HOUND), 1, "the older Hound stays");
    assert_eq!(g.players[0].in_play.get(id::FAITHFUL_HOUND), 0);
    assert_eq!(g.turn.led.n, 0);
}

#[test]
fn vassal_plays_the_hound_it_discarded_when_the_hound_stays() {
    let mut g = new_state(&[id::VASSAL, id::FAITHFUL_HOUND], 2);
    set_hand(&mut g, 0, &[id::VASSAL]);
    set_discard(&mut g, 0, &[id::FAITHFUL_HOUND]);
    set_deck_known(&mut g, 0, &[id::FAITHFUL_HOUND, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::VASSAL);
    choose(&mut g, Choice::No); // leave the Hound
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Play });
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].in_play.get(id::FAITHFUL_HOUND), 1);
    assert_eq!(g.players[0].discard.get(id::FAITHFUL_HOUND), 1, "the older Hound stays");
}

#[test]
fn royal_carriage_is_not_offered_for_a_mining_village_that_trashed_itself_with_another_in_play() {
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::MINING_VILLAGE], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE]);
    set_in_play(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::Yes); // trash it
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { act: Act::Call })), "{:?}", g.pending_decision());
    assert_eq!(g.players[0].in_play.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.turn.led.n, 0);
}

#[test]
fn royal_carriage_is_offered_for_a_mining_village_that_stayed() {
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::MINING_VILLAGE], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::No);
    assert!(matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { act: Act::Call })));
    choose(&mut g, Choice::Yes);
    // Played again: the same copy, still in play, may be trashed now.
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Trash });
}

#[test]
fn a_kings_courted_mining_village_trashes_itself_once() {
    let mut g = new_state(&[id::KINGS_COURT, id::MINING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::MINING_VILLAGE]);
    set_in_play(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY]);
    play(&mut g, id::KINGS_COURT);
    choose(&mut g, Choice::Card(id::MINING_VILLAGE));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.players[0].in_play.get(id::MINING_VILLAGE), 1);
}

#[test]
fn scepter_does_not_replay_a_duration_held_over_from_an_earlier_turn_when_this_turns_copy_has_left_play() {
    // A Caravan played on an earlier turn is in play. This turn another Caravan was played and has left play again (state set up directly):
    // the Caravan in play is not one that was played this turn, so Scepter has nothing to replay.
    let mut g = new_state(&[id::SCEPTER, id::CARAVAN], 2);
    set_in_play(&mut g, 0, &[id::CARAVAN]);
    g.turn.led.carried.add(id::CARAVAN, 1);
    g.turn.played.add(id::CARAVAN, 1);
    set_hand(&mut g, 0, &[id::SCEPTER]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy, "nothing to replay");
}

#[test]
fn scepter_replays_this_turns_duration_even_with_one_held_over_from_an_earlier_turn() {
    let mut g = new_state(&[id::SCEPTER, id::CARAVAN], 2);
    set_in_play(&mut g, 0, &[id::CARAVAN, id::CARAVAN]);
    g.turn.led.carried.add(id::CARAVAN, 1);
    g.turn.played.add(id::CARAVAN, 1);
    set_hand(&mut g, 0, &[id::SCEPTER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    // The Caravan played this turn was replayed (+1 Card): one Copper is drawn.
    assert_eq!(g.players[0].deck_known.len, 1, "{:?}", g.pending_decision());
}

#[test]
fn scheme_putting_a_traveller_on_the_deck_stops_its_exchange() {
    let mut g = new_state(&[id::SCHEME, id::PAGE], 2);
    set_in_play(&mut g, 0, &[id::PAGE, id::SCHEME]);
    g.turn.played.add(id::SCHEME, 1);
    g.turn.played.add(id::PAGE, 1);
    let hunters = g.trav[0];
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    let mut step = pass(&mut g);
    let mut topdecked = false;
    for _ in 0..6 {
        let Step::Decision(d) = step else { break };
        match d.kind {
            DecisionKind::Select { act: Act::Topdeck, .. } => {
                step = choose(&mut g, Choice::Card(id::PAGE));
                topdecked = true;
            }
            DecisionKind::YesNo { act: Act::Exchange } => panic!("Page offered for an exchange after Scheme put it on the deck"),
            _ => break,
        }
    }
    assert!(topdecked);
    assert_eq!(g.trav[0], hunters, "no Treasure Hunter was taken");
}


// ===========================================================================
// Allies with the other sets
// ===========================================================================

#[test]
fn workshop_gains_only_the_top_card_of_a_split_pile_and_a_rotation_changes_it() {
    let offered = |rotate: bool| {
        let mut g = new_state(&[id::WORKSHOP, id::TOWN_CRIER], 2);
        if rotate {
            g.al_rotate_for(0, id::TOWN_CRIER, id::TOWN_CRIER, &mut NoEvents);
        }
        set_hand(&mut g, 0, &[id::WORKSHOP]);
        play(&mut g, id::WORKSHOP);
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
        choices(&g).into_iter().filter_map(|c| if let Choice::Card(c) = c { Some(c) } else { None }).collect::<Vec<_>>()
    };
    let before = offered(false);
    assert!(before.contains(&id::TOWN_CRIER) && !before.contains(&id::BLACKSMITH) && !before.contains(&id::MILLER), "{before:?}");
    let after = offered(true);
    assert!(!after.contains(&id::TOWN_CRIER) && after.contains(&id::BLACKSMITH) && !after.contains(&id::MILLER), "{after:?}");
}

#[test]
fn a_villager_buys_the_action_for_a_second_allies_card() {
    let mut g = new_state(&[id::TOWN, id::PLATEAU_SHEPHERDS], 2);
    g.players[0].ren.villagers = 1;
    set_hand(&mut g, 0, &[id::TOWN, id::TOWN]);
    play(&mut g, id::TOWN);
    choose(&mut g, Choice::Mode(1)); // +1 Buy, +$2
    // Out of Actions with a Town in hand: a Villager or end the phase.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    assert_eq!(choices(&g), vec![Choice::Villager, Choice::Pass]);
    choose(&mut g, Choice::Villager);
    play(&mut g, id::TOWN);
    choose(&mut g, Choice::Mode(1));
    adv(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins), (3, 4));
}

#[test]
fn coffers_pay_for_a_card_made_cheaper_by_family_of_inventors() {
    let mut g = new_state(&[id::SMITHY, id::FAMILY_OF_INVENTORS], 2);
    g.al.fam.add(id::SMITHY, 1);
    assert_eq!(g.cost(id::SMITHY), 3);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 2;
    g.players[0].coffers = 1;
    expect_decision(&mut g);
    assert!(!choices(&g).contains(&Choice::Card(id::SMITHY)));
    choose(&mut g, Choice::Coffers);
    assert!(choices(&g).contains(&Choice::Card(id::SMITHY)));
    choose(&mut g, Choice::Card(id::SMITHY));
    // (The turn ended with the only Buy.)
    assert_eq!(g.supply.get(id::SMITHY), 9);
}
