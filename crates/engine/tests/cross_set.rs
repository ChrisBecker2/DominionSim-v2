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


// ===========================================================================
// Enlightenment (Rising Sun): the Treasures are Action cards for every purpose, scoring included
// ===========================================================================

fn enlightened(kingdom: &[CardId]) -> GameState {
    let mut k = kingdom.to_vec();
    k.push(id::ENLIGHTENMENT);
    let mut g = new_state(&k, 2);
    g.rs.suns = 0;
    g.rs.active = id::ENLIGHTENMENT;
    g.rs_update_hooks();
    g
}

#[test]
fn peddler_costs_2_less_per_treasure_in_play_under_enlightenment() {
    let mut g = enlightened(&[id::PEDDLER]);
    set_in_play(&mut g, 0, &[id::SILVER, id::COPPER]);
    g.turn.phase = Phase::Buy;
    assert_eq!(g.cost(id::PEDDLER), 4);
    let mut g = new_state(&[id::PEDDLER], 2);
    set_in_play(&mut g, 0, &[id::SILVER, id::COPPER]);
    g.turn.phase = Phase::Buy;
    assert_eq!(g.cost(id::PEDDLER), 8, "without the Prophecy Treasures are no Actions");
}

#[test]
fn collection_pays_a_vp_for_a_treasure_gained_under_enlightenment() {
    for (enl, vp) in [(true, 1), (false, 0)] {
        let mut g = if enl { enlightened(&[id::COLLECTION, id::WORKSHOP]) } else { new_state(&[id::COLLECTION, id::WORKSHOP], 2) };
        set_in_play(&mut g, 0, &[id::COLLECTION]);
        set_hand(&mut g, 0, &[id::WORKSHOP]);
        play(&mut g, id::WORKSHOP);
        choose(&mut g, Choice::Card(id::SILVER));
        assert_eq!(g.players[0].vp_tokens, vp, "enlightenment: {enl}");
    }
}

#[test]
fn vineyard_counts_treasures_as_actions_when_scoring_under_enlightenment() {
    let mut g = enlightened(&[id::VINEYARD]);
    set_hand(&mut g, 0, &[id::VINEYARD, id::SILVER, id::SILVER, id::SILVER]);
    assert_eq!(g.player_vp(0), 1);
    let mut g = new_state(&[id::VINEYARD], 2);
    set_hand(&mut g, 0, &[id::VINEYARD, id::SILVER, id::SILVER, id::SILVER]);
    assert_eq!(g.player_vp(0), 0);
}

#[test]
fn housecarl_counts_the_treasures_in_play_as_actions_under_enlightenment() {
    for (enl, drawn) in [(true, 3), (false, 1)] {
        let mut g = if enl { enlightened(&[id::JOUST]) } else { new_state(&[id::JOUST], 2) };
        set_in_play(&mut g, 0, &[id::SILVER, id::GOLD]);
        set_hand(&mut g, 0, &[id::HOUSECARL]);
        set_deck_known(&mut g, 0, &[id::ESTATE; 5]);
        play(&mut g, id::HOUSECARL);
        assert_eq!(g.players[0].hand.total(), drawn, "enlightenment: {enl}");
    }
}

#[test]
fn transmute_gains_a_duchy_for_a_trashed_treasure_under_enlightenment() {
    for (enl, duchy) in [(true, 1), (false, 0)] {
        let mut g = if enl { enlightened(&[id::TRANSMUTE]) } else { new_state(&[id::TRANSMUTE], 2) };
        set_hand(&mut g, 0, &[id::TRANSMUTE, id::SILVER]);
        play(&mut g, id::TRANSMUTE);
        adv(&mut g);
        assert_eq!(g.players[0].discard.get(id::DUCHY), duchy, "enlightenment: {enl}");
        assert_eq!(g.players[0].discard.get(id::TRANSMUTE), 1);
    }
}

#[test]
fn scrying_pool_keeps_revealing_through_treasures_under_enlightenment() {
    for (enl, in_hand) in [(true, 3), (false, 1)] {
        let mut g = if enl { enlightened(&[id::SCRYING_POOL]) } else { new_state(&[id::SCRYING_POOL], 2) };
        set_hand(&mut g, 0, &[id::SCRYING_POOL]);
        set_deck_known(&mut g, 0, &[id::SILVER, id::COPPER, id::ESTATE, id::DUCHY]);
        play(&mut g, id::SCRYING_POOL);
        choose(&mut g, Choice::Pass); // leave the top card (the opponent has no deck)
        // (A Treasure drawn into the hand may already have been played in the Buy phase.)
        let drawn = g.players[0].hand.total() + g.players[0].in_play.total() - 1;
        assert_eq!(drawn, in_hand, "enlightenment: {enl}");
    }
}

/// Young Witch's Bane gets the set-up its card requires, like a kingdom card: a Jewelled Egg Bane brings the Loot pile, a Sleigh Bane the
/// Horses (found by the Rising Sun fuzz: the text round trip of such a game listed a Loot pile the game never had).
#[test]
fn a_bane_gets_the_set_up_its_card_requires() {
    use dominion_engine::sets::{menagerie::ids as men, plunder::ids as plu};
    let g = GameState::new(&GameConfig { num_players: 2, kingdom: vec![id::YOUNG_WITCH], bane: Some(plu::JEWELLED_EGG), ..Default::default() });
    assert_eq!(g.bane, plu::JEWELLED_EGG);
    assert!(g.plu.flags & dominion_engine::state::PF_LOOT != 0, "the Loot pile is set up");
    assert!(g.supply.get(cards::FIRST_LOOT) > 0);
    let g = GameState::new(&GameConfig { num_players: 2, kingdom: vec![id::YOUNG_WITCH], bane: Some(men::SLEIGH), ..Default::default() });
    assert!(g.has_horse_pile(), "the Horse pile is set up");
}

/// Merchant Guild with Villa: "+1 Coffers per card you gained in it" is counted at the end of each Buy phase, for that Buy phase only (Villa
/// returns the player to the Action phase, which ends the Buy phase).
#[test]
fn merchant_guild_counts_each_buy_phase_once_when_villa_returns_to_the_action_phase() {
    let mut g = new_state(&[id::MERCHANT_GUILD, id::VILLA], 2);
    set_hand(&mut g, 0, &[id::MERCHANT_GUILD]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::MERCHANT_GUILD);
    g.turn.coins = 6;
    while g.turn.phase == Phase::Action {
        pass(&mut g);
    }
    buy(&mut g, id::VILLA);
    assert_eq!(g.turn.phase, Phase::Action, "Villa returns to the Action phase");
    assert_eq!(g.players[0].coffers, 1, "the first Buy phase gained one card");
    while g.turn.phase == Phase::Action {
        pass(&mut g);
    }
    buy(&mut g, id::COPPER);
    while g.turn.player == 0 {
        pass(&mut g);
    }
    assert_eq!(g.players[0].coffers, 2, "the second Buy phase gained one more; the Villa is not counted again");
}

/// Every Event can be bought whenever the rules allow it, also when it would do nothing (Dominate with no Provinces left, Kintsugi with an
/// empty hand, Amass with an Action in play, Mission on an extra turn): buying it then plays out without a crash, and every card stays somewhere.
/// Some still do something: Salt the Earth's +1 VP, Bury's +1 Buy, Continue's return with +1 Action and +1 Buy.
#[test]
fn every_event_can_be_bought_when_it_would_do_nothing() {
    for e in cards::FIRST_EVENT..cards::FIRST_EVENT + cards::NUM_EVENTS as CardId {
        for scenario in 0..3 {
            let mut g = new_state(&[e, id::VILLAGE, id::SMITHY], 2);
            match scenario {
                // Nothing anywhere: no hand, deck or discard pile, and the piles an Event could gain from or trash are empty.
                0 => {
                    set_hand(&mut g, 0, &[]);
                    clear_deck(&mut g, 0);
                    set_discard(&mut g, 0, &[]);
                    for c in [id::PROVINCE, id::DUCHY, id::ESTATE, id::CURSE, id::GOLD, id::SILVER, id::VILLAGE, id::SMITHY] {
                        empty_pile(&mut g, c);
                    }
                }
                // An Action in play, a hand of Coppers.
                1 => {
                    set_in_play(&mut g, 0, &[id::SMITHY]);
                    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER]);
                    set_deck_known(&mut g, 0, &[id::ESTATE; 10]);
                }
                // An extra turn.
                _ => {
                    g.turn.is_extra_turn = true;
                    set_hand(&mut g, 0, &[id::ESTATE]);
                    set_deck_known(&mut g, 0, &[id::ESTATE; 10]);
                }
            }
            g.turn.phase = Phase::Buy;
            g.turn.coins = 20;
            g.turn.buys = 2;
            g.turn.treasures_done = true;
            g.pending = Pending::None;
            let before = total_cards(&g);
            let start = g.turn.number;
            adv(&mut g);
            let name = cards::name(e);
            assert!(choices(&g).contains(&Choice::Card(e)), "{name} (scenario {scenario}) is not offered: {:?}", choices(&g));
            let mut step = choose(&mut g, Choice::Card(e));
            let mut over = false;
            for _ in 0..500 {
                match step {
                    Step::Decision(d) if g.turn.number == start => {
                        let c = match d.kind {
                            DecisionKind::PlayAction | DecisionKind::Buy => Choice::Pass,
                            _ => choices(&g)[0],
                        };
                        step = choose(&mut g, c);
                    }
                    Step::GameOver => {
                        over = true;
                        break;
                    }
                    _ => break,
                }
            }
            assert!(over || g.turn.number != start, "{name} (scenario {scenario}): the turn did not end: {step:?}");
            assert_eq!(total_cards(&g), before, "{name} (scenario {scenario}): cards appeared or vanished
{}", dominion_engine::text::format_state(&g));
        }
    }
}

// ===========================================================================
// Extra turns: Outpost, Mission, Seize the Day, Voyage, Island Folk, Journey, Fleet, and Lich's skipped turns
// ===========================================================================

/// A 2-player game from the text format, player 1 to act on turn 5 with 2 Actions, 2 Buys and $8 (both players have taken 2 turns); `head` holds
/// the game lines (Events, Projects, Ally, supply), `p1` / `p2` the players' lines. Both decks are Estates only, so every draw is known.
fn xt_game(head: &str, p1: &str, p2: &str) -> GameState {
    let text = format!(
        "players: 2\n{head}\nturn: 5  player: 1  phase: action  actions: 2  buys: 2  coins: 8\nmax_turns: 500\n\n\
         [player 1]\ndeck: 40 Estate\nturns: 2\n{p1}\n\n[player 2]\ndeck: 40 Estate\nturns: 2\n{p2}\n"
    );
    let mut g = dominion_engine::text::parse_state(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    // Stop as each turn begins, even one without a decision in it.
    g.pause_at_turn_start = true;
    g
}

/// The kingdom with every extra-turn card of these tests (Voyage is in the Odysseys' pile under Old Map, Lich in the Wizards' under Student).
const XT_HEAD: &str = "kingdom: Outpost, Old Map, Student, Throne Room, King's Court\nevents: Mission, Seize the Day, Journey";

/// Answer the decision the way a passive player does: Pass, else No, else the first choice.
fn xt_default(g: &mut GameState) {
    let cs = choices(g);
    let c = if cs.contains(&Choice::Pass) {
        Choice::Pass
    } else if cs.contains(&Choice::No) {
        Choice::No
    } else {
        cs[0]
    };
    g.apply(c, &mut NoEvents).unwrap();
}

/// Pass to the end of the current turn: whether another turn started (false: the game is over).
fn xt_next_turn(g: &mut GameState) -> bool {
    for _ in 0..500 {
        match adv(g) {
            Step::GameOver => return false,
            Step::TurnStart { .. } => return true,
            Step::Decision(_) => xt_default(g),
            other => panic!("{other:?}"),
        }
    }
    panic!("the turn never ended");
}

/// Like `xt_next_turn`, but spend the Favors for Island Folk's extra turn when it is offered: (it was offered, another turn started).
fn xt_island_folk_yes(g: &mut GameState) -> (bool, bool) {
    let mut asked = false;
    for _ in 0..500 {
        match adv(g) {
            Step::GameOver => return (asked, false),
            Step::TurnStart { .. } => return (asked, true),
            Step::Decision(d) if d.source == Some(id::ISLAND_FOLK) => {
                asked = true;
                g.apply(Choice::Yes, &mut NoEvents).unwrap();
            }
            Step::Decision(_) => xt_default(g),
            other => panic!("{other:?}"),
        }
    }
    panic!("the turn never ended");
}

/// Pass up to the Buy decision of the current turn.
fn xt_to_buy(g: &mut GameState) {
    loop {
        let d = expect_decision(g);
        if d.kind == DecisionKind::Buy {
            return;
        }
        xt_default(g);
    }
}

/// From the Action phase: play `plays` from the hand (a Throne Room or King's Court plays the card listed after it), then buy `buys` (cards, Events
/// or Projects).
fn xt_do(g: &mut GameState, plays: &[CardId], buys: &[CardId]) {
    let mut i = 0;
    while i < plays.len() {
        play(g, plays[i]);
        if plays[i] == id::THRONE_ROOM || plays[i] == id::KINGS_COURT {
            i += 1;
            choose(g, Choice::Card(plays[i]));
        }
        i += 1;
    }
    xt_to_buy(g);
    for &c in buys {
        xt_buy(g, c);
    }
}

/// Buy `c` at the Buy decision, without going on (the turn may end with it: `xt_next_turn` goes on).
fn xt_buy(g: &mut GameState, c: CardId) {
    assert_eq!(expect_decision(g).kind, DecisionKind::Buy);
    g.apply(Choice::Card(c), &mut NoEvents).unwrap();
}

/// The (turn number, player) of the current turn and of each turn after it until the game ends or `max` turns are listed.
fn xt_turns(g: &mut GameState, max: usize) -> Vec<(u16, u8)> {
    let mut v = vec![(g.turn.number, g.turn.player)];
    while v.len() < max && xt_next_turn(g) {
        v.push((g.turn.number, g.turn.player));
    }
    v
}

/// The limits of the current turn: (cards in hand, no cards may be bought, a Voyage turn).
fn xt_limits(g: &GameState) -> (u32, bool, bool) {
    (g.players[g.turn.player as usize].hand.total() as u32, g.turn.adv.no_buy, g.turn.al.limited)
}

#[test]
fn two_extra_turn_sources_on_one_turn_give_one_extra_turn_and_the_least_limited_is_taken() {
    // (plays, buys, the extra turn's limits): of two extra turns set up together only one is taken, the other would be a 3rd turn in a row. Outpost's
    // 3-card hand is drawn whichever is taken; a Journey's or an Outpost's turn has no other limit, so it is taken over a Mission's or a Voyage's; of
    // Mission and Voyage the engine takes the Mission turn.
    let cases: [(&[CardId], &[CardId], (u32, bool, bool)); 6] = [
        (&[id::OUTPOST, id::VOYAGE], &[], (3, false, false)),
        (&[id::OUTPOST], &[id::MISSION], (3, false, false)),
        (&[id::OUTPOST], &[id::JOURNEY], (3, false, false)),
        (&[id::VOYAGE], &[id::MISSION], (5, true, false)),
        (&[id::VOYAGE], &[id::JOURNEY], (5, false, false)),
        (&[], &[id::MISSION, id::JOURNEY], (5, false, false)),
    ];
    for (plays, buys, limits) in cases {
        let mut g = xt_game(XT_HEAD, "hand: Voyage, Outpost", "hand: 5 Estate");
        xt_do(&mut g, plays, buys);
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.number, g.turn.player, g.turn.is_extra_turn), (6, 0, true), "{plays:?} {buys:?}");
        assert_eq!(xt_limits(&g), limits, "{plays:?} {buys:?}");
        // The limits are the extra turn's only; no 3rd turn: the other player's turn and the next one of player 1 are normal.
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.player, g.turn.is_extra_turn, xt_limits(&g)), (1, false, (5, false, false)), "{plays:?} {buys:?}");
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.player, g.turn.is_extra_turn, xt_limits(&g)), (0, false, (5, false, false)), "{plays:?} {buys:?}");
        assert_eq!((g.turn.number, g.players[0].turns_taken, g.players[1].turns_taken), (8, 3, 3), "the extra turn is not counted: {plays:?} {buys:?}");
    }
}

#[test]
fn seize_the_day_with_another_extra_turn_on_the_same_turn_gives_both_turns_seize_the_days_last() {
    // Seize the Day does not check the turns before it: the other extra turn (not a 3rd in a row) is taken first, then Seize the Day's, a 3rd turn.
    let cases: [(&[CardId], &[CardId], (u32, bool, bool)); 4] = [
        (&[id::OUTPOST], &[], (3, false, false)),
        (&[id::VOYAGE], &[], (5, false, true)),
        (&[], &[id::MISSION], (5, true, false)),
        (&[], &[id::JOURNEY], (5, false, false)),
    ];
    for (plays, buys, limits) in cases {
        let mut g = xt_game(XT_HEAD, "hand: Voyage, Outpost", "hand: 5 Estate");
        let mut b = buys.to_vec();
        b.push(id::SEIZE_THE_DAY);
        xt_do(&mut g, plays, &b);
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.player, g.turn.is_extra_turn, xt_limits(&g)), (0, true, limits), "{plays:?} {buys:?}");
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.player, g.turn.is_extra_turn, xt_limits(&g)), (0, true, (5, false, false)), "Seize the Day's turn: {plays:?} {buys:?}");
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.number, g.turn.player), (8, 1), "{plays:?} {buys:?}");
        assert_eq!(g.players[0].turns_taken, 3, "only the normal turn counted: {plays:?} {buys:?}");
    }
}

#[test]
fn island_folk_is_offered_with_seize_the_day_bought_and_both_extra_turns_are_taken() {
    let mut g = xt_game(&format!("{XT_HEAD}\nally: Island Folk"), "hand: Copper\nfavors: 5", "hand: 5 Estate");
    xt_do(&mut g, &[], &[id::SEIZE_THE_DAY]);
    assert_eq!(xt_island_folk_yes(&mut g), (true, true), "Island Folk is offered: Seize the Day's turn comes after its turn");
    assert_eq!(g.players[0].al.favors, 0);
    let turns = xt_turns(&mut g, 3);
    assert_eq!(turns, [(6, 0), (7, 0), (8, 1)]);
}

#[test]
fn the_most_turns_in_a_row_is_four_a_normal_turn_an_extra_turn_seize_the_day_and_fleet() {
    // Player 1 has Fleet and ends the game with an Outpost in play: the Outpost turn is taken (Fleet is in the game), then Seize the Day's turn,
    // then the Fleet turn. Every try for a 5th turn in a row fails: Outpost on the Outpost turn, Voyage on Seize the Day's turn (a 3rd turn in a row
    // for both), Outpost on the Fleet turn (and no extra turn comes after the last Fleet turn).
    let mut g = xt_game(
        &format!("{XT_HEAD}\nprojects: Fleet\nsupply: Province=1"),
        "hand: Outpost, Copper\ndeck top: Outpost, Estate, Estate, Voyage, Estate, Estate, Outpost, Estate, Estate, Estate, Estate\nowned projects: Fleet",
        "hand: 5 Estate",
    );
    xt_do(&mut g, &[id::OUTPOST], &[id::PROVINCE]);
    let mut turns = vec![(g.turn.number, g.turn.player)];
    // The Outpost turn: Outpost again (no 3rd turn from it), and Seize the Day.
    assert!(xt_next_turn(&mut g));
    turns.push((g.turn.number, g.turn.player));
    assert_eq!(xt_limits(&g), (3, false, false));
    g.turn.coins = 4;
    xt_do(&mut g, &[id::OUTPOST], &[id::SEIZE_THE_DAY]);
    // Seize the Day's turn (a 3-card hand: an Outpost was played on the turn before).
    assert!(xt_next_turn(&mut g));
    turns.push((g.turn.number, g.turn.player));
    assert_eq!((g.turn.is_extra_turn, xt_limits(&g)), (true, (3, false, false)));
    xt_do(&mut g, &[id::VOYAGE], &[]);
    // The Fleet turn, the 4th in a row.
    assert!(xt_next_turn(&mut g));
    turns.push((g.turn.number, g.turn.player));
    assert!(g.ren.flags & dominion_engine::state::REN_FLEET_ROUND != 0 && g.turn.is_extra_turn);
    xt_do(&mut g, &[id::OUTPOST], &[]);
    assert!(!xt_next_turn(&mut g), "the game is over after the Fleet turn");
    assert_eq!(turns, [(5, 0), (6, 0), (7, 0), (8, 0)], "four turns in a row, numbered one after another");
    // Only the normal turn counts for the tie-break.
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken), (3, 2));
}

#[test]
fn without_fleet_seize_the_day_on_an_extra_turn_gives_the_third_turn_in_a_row_and_nothing_gives_a_fourth() {
    let mut g = xt_game(XT_HEAD, "hand: Outpost\ndeck top: Voyage, Outpost, Estate, Outpost, Voyage, Estate, Estate, Estate", "hand: 5 Estate");
    xt_do(&mut g, &[id::OUTPOST], &[]);
    assert!(xt_next_turn(&mut g));
    assert_eq!(xt_limits(&g), (3, false, false));
    g.turn.coins = 8;
    g.turn.buys = 2;
    // Mission bought on an extra turn gives nothing (it would be a 3rd turn in a row); Seize the Day gives its turn, with no Mission limit.
    xt_do(&mut g, &[], &[id::MISSION, id::SEIZE_THE_DAY]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.player, g.turn.is_extra_turn, xt_limits(&g)), (0, true, (5, false, false)), "Seize the Day's turn, the 3rd in a row");
    xt_do(&mut g, &[id::VOYAGE, id::OUTPOST], &[]);
    assert_eq!(xt_turns(&mut g, 3), [(7, 0), (8, 1), (9, 0)], "no 4th turn");
}

#[test]
fn kings_court_on_outpost_and_throne_room_on_voyage_give_one_extra_turn_and_stay_in_play_until_its_clean_up() {
    // King's Court plays Outpost three times: one extra turn (3-card hand); both stay in play for it and are discarded at its Clean-up.
    let mut g = xt_game(XT_HEAD, "hand: King's Court, Outpost", "hand: 5 Estate");
    xt_do(&mut g, &[id::KINGS_COURT, id::OUTPOST], &[]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.player, xt_limits(&g)), (0, (3, false, false)));
    assert_eq!((g.players[0].in_play.get(id::KINGS_COURT), g.players[0].in_play.get(id::OUTPOST)), (1, 1));
    assert!(xt_next_turn(&mut g));
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].in_play.is_empty(), "discarded at the extra turn's Clean-up");
    assert_eq!((g.players[0].discard.get(id::KINGS_COURT), g.players[0].discard.get(id::OUTPOST)), (1, 1));
    // Throne Room plays Voyage twice: one Voyage turn; both stay in play for it.
    let mut g = xt_game(XT_HEAD, "hand: Throne Room, Voyage", "hand: 5 Estate");
    xt_do(&mut g, &[id::THRONE_ROOM, id::VOYAGE], &[]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.player, xt_limits(&g)), (0, (5, false, true)));
    assert_eq!((g.players[0].in_play.get(id::THRONE_ROOM), g.players[0].in_play.get(id::VOYAGE)), (1, 1));
    assert!(xt_next_turn(&mut g));
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].in_play.is_empty(), "discarded at the Voyage turn's Clean-up");
}

#[test]
fn a_voyage_that_fails_on_an_extra_turn_is_discarded_with_its_throne_room_at_the_next_clean_up() {
    // Played on an Outpost turn, the Voyage (and the Throne Room that played it) give no turn; they are discarded in the next Clean-up, the other
    // player's (like a failed Outpost).
    let mut g = xt_game(XT_HEAD, "hand: Outpost\ndeck top: Throne Room, Voyage, Estate", "hand: 5 Estate");
    xt_do(&mut g, &[id::OUTPOST], &[]);
    assert!(xt_next_turn(&mut g));
    xt_do(&mut g, &[id::THRONE_ROOM, id::VOYAGE], &[]);
    assert!(xt_next_turn(&mut g));
    assert_eq!(g.turn.player, 1, "no 3rd turn");
    assert_eq!((g.players[0].in_play.get(id::THRONE_ROOM), g.players[0].in_play.get(id::VOYAGE)), (1, 1), "in play until the next Clean-up");
    assert!(xt_next_turn(&mut g));
    assert_eq!(g.turn.player, 0);
    assert_eq!((g.players[0].in_play.get(id::THRONE_ROOM), g.players[0].in_play.get(id::VOYAGE)), (0, 0), "discarded in player 2's Clean-up");
    assert!(xt_next_turn(&mut g));
    assert_eq!(g.turn.player, 1, "and it gives no turn later either");
}

#[test]
fn a_mission_turn_buys_events_and_projects_but_no_cards() {
    let mut g = xt_game(&format!("{XT_HEAD}\nprojects: Fleet"), "hand: Copper", "hand: 5 Estate");
    xt_do(&mut g, &[], &[id::MISSION]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.player, g.turn.adv.no_buy), (0, true));
    g.turn.coins = 8;
    xt_to_buy(&mut g);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SEIZE_THE_DAY)) && cs.contains(&Choice::Card(id::FLEET)), "{cs:?}");
    assert!(!cs.contains(&Choice::Card(id::SILVER)) && !cs.contains(&Choice::Card(id::OUTPOST)), "{cs:?}");
}

// ---- Lich: skipped turns next to extra turns ----

#[test]
fn lich_skips_the_first_of_two_extra_turns_set_up_on_one_turn_and_the_second_is_taken() {
    // "If you play two Outposts and Lich on one turn, Lich can skip the first extra turn. The second Outpost still happens, since you haven't taken
    // 2 turns in a row yet" (the 2023 errata ruling); the same for two Voyages, a Throne-Roomed Outpost, and an extra turn with Seize the Day's.
    let cases: [(&[CardId], &[CardId], (u32, bool, bool)); 5] = [
        (&[id::LICH, id::OUTPOST, id::OUTPOST], &[], (3, false, false)),
        (&[id::LICH, id::THRONE_ROOM, id::OUTPOST], &[], (3, false, false)),
        (&[id::LICH, id::VOYAGE, id::VOYAGE], &[], (5, false, true)),
        (&[id::LICH, id::OUTPOST, id::VOYAGE], &[], (3, false, false)),
        (&[id::LICH, id::OUTPOST], &[id::SEIZE_THE_DAY], (3, false, false)),
    ];
    for (plays, buys, limits) in cases {
        let mut g = xt_game(XT_HEAD, "hand: Lich, Outpost, Outpost, Voyage, Voyage, Throne Room", "hand: 5 Estate");
        xt_do(&mut g, plays, buys);
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.number, g.turn.player, g.turn.is_extra_turn), (7, 0, true), "turn 6 was skipped: {plays:?} {buys:?}");
        assert_eq!((xt_limits(&g), g.players[0].al.skips), (limits, 0), "{plays:?} {buys:?}");
        assert!(xt_next_turn(&mut g));
        assert_eq!((g.turn.number, g.turn.player), (8, 1), "no 3rd turn: {plays:?} {buys:?}");
        assert_eq!(g.players[0].turns_taken, 3, "the skipped extra turn is not counted either: {plays:?} {buys:?}");
    }
}

#[test]
fn lich_with_one_extra_turn_skips_it_the_outpost_is_discarded_at_the_next_clean_up_and_the_three_card_hand_is_kept() {
    // "if you play Lich and Outpost in the same turn, you draw 3 cards during Clean-up, skip the Outpost turn, and then you keep that 3-card hand for
    // your next turn"; the Outpost that failed is discarded in the next Clean-up, whoever's turn it is.
    let mut g = xt_game(XT_HEAD, "hand: Lich, Outpost", "hand: 5 Estate");
    xt_do(&mut g, &[id::LICH, id::OUTPOST], &[]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player, g.turn.is_extra_turn), (7, 1, false));
    assert_eq!(g.players[0].in_play.get(id::OUTPOST), 1);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player, g.turn.is_extra_turn), (8, 0, false));
    assert_eq!((g.players[0].in_play.get(id::OUTPOST), g.players[0].discard.get(id::OUTPOST)), (0, 1), "discarded in player 2's Clean-up");
    assert_eq!(xt_limits(&g), (3, false, false));
    // Lich and Voyage, and Lich and Seize the Day, the same: the extra turn is skipped.
    for (plays, buys) in [(&[id::LICH, id::VOYAGE][..], &[][..]), (&[id::LICH][..], &[id::SEIZE_THE_DAY][..])] {
        let mut g = xt_game(XT_HEAD, "hand: Lich, Voyage", "hand: 5 Estate");
        xt_do(&mut g, plays, buys);
        assert_eq!(xt_turns(&mut g, 3), [(5, 0), (7, 1), (8, 0)], "{plays:?} {buys:?}");
    }
}

#[test]
fn lich_and_outpost_on_an_extra_turn_skip_the_next_normal_turn_instead() {
    // "If you play Outpost and Lich on an Outpost turn, Outpost doesn't generate an extra turn and Lich ends up skipping your next normal turn."
    let mut g = xt_game(XT_HEAD, "hand: Outpost\ndeck top: Lich, Outpost, Estate", "hand: 5 Estate");
    xt_do(&mut g, &[id::OUTPOST], &[]);
    assert!(xt_next_turn(&mut g));
    xt_do(&mut g, &[id::LICH, id::OUTPOST], &[]);
    let turns = xt_turns(&mut g, 4);
    assert_eq!(turns, [(6, 0), (7, 1), (9, 1), (10, 0)], "player 1's normal turn 8 is skipped");
    // The skipped normal turn counts for the tie-break; player 2's two turns in a row are normal turns and count too.
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken), (4, 4));
}

#[test]
fn after_lich_skips_the_other_players_turn_the_previous_turn_was_yours_so_outpost_gives_no_extra_turn() {
    // "cards that check the owner of the previous turn (e.g. Voyage) will check the last turn that was actually played. So if all the other players
    // have skipped their turns with Lich, your Voyage will fail."
    let mut g = xt_game(XT_HEAD, "hand: Copper\ndeck top: Outpost, Voyage, Estate, Estate, Estate", "hand: 5 Estate\nskips: 1");
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player), (7, 0), "player 2's turn 6 is skipped");
    assert!(g.turn.is_extra_turn, "the previous turn taken was player 1's");
    xt_do(&mut g, &[id::VOYAGE, id::OUTPOST], &[]);
    assert_eq!(xt_turns(&mut g, 2), [(7, 0), (8, 1)], "no 3rd turn in a row");
    // Both of player 1's turns were normal turns, and player 2's skipped one counts as taken.
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken), (4, 3));
}

// ---- The end of the game ----

#[test]
fn extra_turns_set_up_on_the_turn_the_game_ends_are_not_taken_without_fleet() {
    let cases: [(&[CardId], &[CardId]); 5] = [
        (&[id::OUTPOST], &[]),
        (&[id::VOYAGE], &[]),
        (&[], &[id::MISSION]),
        (&[], &[id::SEIZE_THE_DAY]),
        (&[], &[id::JOURNEY]),
    ];
    for (plays, buys) in cases {
        let mut g = xt_game(&format!("{XT_HEAD}\nsupply: Province=1"), "hand: Outpost, Voyage", "hand: 5 Estate");
        g.turn.coins = 12;
        let mut b = buys.to_vec();
        b.push(id::PROVINCE);
        xt_do(&mut g, plays, &b);
        assert!(!xt_next_turn(&mut g), "{plays:?} {buys:?}");
        assert_eq!((g.turn.number, g.players[0].turns_taken, g.players[1].turns_taken), (5, 3, 2), "{plays:?} {buys:?}");
    }
    // Island Folk is not even offered.
    let mut g = xt_game(&format!("{XT_HEAD}\nally: Island Folk\nsupply: Province=1"), "hand: Copper\nfavors: 5", "hand: 5 Estate");
    xt_do(&mut g, &[], &[id::PROVINCE]);
    assert_eq!(xt_island_folk_yes(&mut g), (false, false));
    assert_eq!(g.players[0].al.favors, 5);
}

#[test]
fn with_fleet_in_the_game_the_extra_turn_set_up_as_the_game_would_end_is_taken_before_the_fleet_turns() {
    // "If you end the game and take an extra turn in the same turn, and another player has Fleet, you will get your extra turn. During that turn, if
    // you buy Fleet, you will not get a Fleet turn."
    let mut g = xt_game(&format!("{XT_HEAD}\nprojects: Fleet\nsupply: Province=1"), "hand: Outpost, Copper", "hand: 5 Estate\nowned projects: Fleet");
    xt_do(&mut g, &[id::OUTPOST], &[id::PROVINCE]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player, xt_limits(&g)), (6, 0, (3, false, false)), "the Outpost turn");
    g.turn.coins = 5;
    xt_do(&mut g, &[], &[id::FLEET]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player), (7, 1), "player 2's Fleet turn");
    assert!(!xt_next_turn(&mut g), "no Fleet turn for the Fleet bought in the Outpost turn");
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken), (3, 2));
    // Island Folk is offered when the game would end and someone has Fleet: its turn comes first.
    let mut g = xt_game(
        &format!("{XT_HEAD}\nprojects: Fleet\nally: Island Folk\nsupply: Province=1"),
        "hand: Copper\nfavors: 5",
        "hand: 5 Estate\nowned projects: Fleet",
    );
    xt_do(&mut g, &[], &[id::PROVINCE]);
    assert_eq!(xt_island_folk_yes(&mut g), (true, true));
    assert_eq!(g.players[0].al.favors, 0);
    assert_eq!(xt_turns(&mut g, 4), [(6, 0), (7, 1)]);
}

#[test]
fn extra_turns_happen_between_fleet_turns_but_not_after_the_last_one() {
    // "Other extra turns, such as from Outpost can happen in-between those turns; however after the last extra turn due to Fleet, no other extra
    // turns can happen."
    let mut g = xt_game(
        &format!("{XT_HEAD}\nprojects: Fleet\nsupply: Province=1"),
        "hand: Copper\ndeck top: Outpost, Estate, Estate, Estate, Estate\nowned projects: Fleet",
        "hand: Outpost, 4 Estate\nowned projects: Fleet",
    );
    xt_do(&mut g, &[], &[id::PROVINCE]);
    // Player 2's Fleet turn: the previous turn was player 1's, so their Outpost gives an extra turn (player 1's Fleet turn is still to come).
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player, g.turn.is_extra_turn), (6, 1, false));
    xt_do(&mut g, &[id::OUTPOST], &[]);
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player, g.turn.is_extra_turn, xt_limits(&g)), (7, 1, true, (3, false, false)), "player 2's Outpost turn");
    // Player 1's Fleet turn, the last: their Outpost gives nothing.
    assert!(xt_next_turn(&mut g));
    assert_eq!((g.turn.number, g.turn.player), (8, 0));
    xt_do(&mut g, &[id::OUTPOST], &[]);
    assert!(!xt_next_turn(&mut g));
    // None of the turns after the one that ended the game counts for the tie-break.
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken), (3, 2));
}

#[test]
fn lich_skipping_the_last_fleet_turn_ends_the_game_and_gives_no_turn_to_a_player_without_fleet() {
    let mut g = xt_game(&format!("{XT_HEAD}\nprojects: Fleet\nsupply: Province=1"), "hand: Lich\nowned projects: Fleet", "hand: 5 Estate");
    g.turn.coins = 8;
    xt_do(&mut g, &[id::LICH], &[id::PROVINCE]);
    assert!(!xt_next_turn(&mut g), "player 1's Fleet turn is skipped and the game is over");
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken, g.players[0].al.skips), (3, 2, 0));
    // With both players holding Fleet: player 2's Fleet turn, then player 1's is skipped.
    let mut g = xt_game(&format!("{XT_HEAD}\nprojects: Fleet\nsupply: Province=1"), "hand: Lich\nowned projects: Fleet", "hand: 5 Estate\nowned projects: Fleet");
    xt_do(&mut g, &[id::LICH], &[id::PROVINCE]);
    assert_eq!(xt_turns(&mut g, 4), [(5, 0), (6, 1)]);
}
