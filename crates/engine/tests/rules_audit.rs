//! Rules found by checking the Base, Intrigue, Seaside, Prosperity and Alchemy cards against the
//! official rulebooks' FAQs and the DominionStrategy wiki's rules pages. One test per rule, each
//! named after what the rule says. The plan docs (`docs/intrigue-plan.md`,
//! `docs/seaside-prosperity-plan.md`, `docs/alchemy-plan.md`) have a "Rules audit" section each
//! with what was checked and what is still open.

mod common;
use common::*;
use dominion_engine::cards::{self, id};
use dominion_engine::state::PendingDuration;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn inject_pending(g: &mut GameState, p: usize, card: CardId, times: u8, arg: CardId) {
    let i = g.players[p].pending_durations_len as usize;
    g.players[p].pending_durations[i] = PendingDuration { card, times, arg, used: false };
    g.players[p].pending_durations_len += 1;
}

fn is_yes_no(d: &Decision) -> bool {
    matches!(d.kind, DecisionKind::YesNo { .. })
}

/// Answer a Diplomat's "discard 3 cards" by discarding `card` for as long as it is Diplomat's
/// own discard that is asked (some picks are applied without asking when only one card is legal).
fn discard_for_diplomat(g: &mut GameState, card: CardId) {
    while let Some(Decision { kind: DecisionKind::Select { act: Act::Discard, .. }, source: Some(id::DIPLOMAT), .. }) =
        g.pending_decision()
    {
        choose(g, Choice::Card(card));
    }
}

// ===========================================================================
// Setup and end of game
// ===========================================================================

#[test]
fn an_outposts_extra_turn_does_not_count_towards_the_fewer_turns_tiebreak() {
    let mut g = new_state(&[id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    play(&mut g, id::OUTPOST);
    pass(&mut g); // the Outpost turn ends: player 0's first turn
    assert!(g.turn.is_extra_turn);
    assert_eq!(g.players[0].turns_taken, 1);
    pass(&mut g); // the extra turn ends
    assert_eq!(g.turn.player, 1);
    assert_eq!(g.players[0].turns_taken, 1, "the extra turn is not a turn taken for the tie-break");
    pass(&mut g); // player 1's turn ends: one turn each, equal scores
    assert_eq!(g.players[1].turns_taken, 1);
    assert_eq!(g.winners(), 0b11, "same number of turns, same score: a shared win, not a win for player 1");
}

#[test]
fn colonies_are_8_with_two_players_and_12_with_three_to_six() {
    for (n, want) in [(2, 8), (3, 12), (4, 12), (5, 12), (6, 12)] {
        assert_eq!(new_state(&[id::PLATINUM, id::COLONY], n).supply.get(id::COLONY), want, "{n} players");
    }
}

// ===========================================================================
// Mint, Quarry, Hoard, Collection, Tiara: gains and "this turn" effects
// ===========================================================================

#[test]
fn a_quarry_played_twice_with_tiara_takes_four_off_action_costs() {
    let mut g = new_state(&[id::TIARA, id::QUARRY, id::KINGS_COURT], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::QUARRY]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    choose(&mut g, Choice::Card(id::QUARRY)); // played twice
    assert_eq!(g.cost(id::KINGS_COURT), 3, "$7 less $2 per play of Quarry");
    assert_eq!(g.cost(id::GOLD), 6, "only Action cards get cheaper");
}

#[test]
fn quarrys_discount_lasts_the_turn_even_if_a_mint_trashes_the_quarry() {
    let mut g = new_state(&[id::QUARRY, id::MINT, id::KINGS_COURT], 2);
    set_hand(&mut g, 0, &[id::QUARRY, id::SILVER]);
    expect_decision(&mut g); // both Treasures auto-play: $3, and Mint costs $5 - $2
    g.turn.buys = 2;
    assert_eq!(g.cost(id::MINT), 3);
    g.apply(Choice::Card(id::MINT), &mut NoEvents).unwrap();
    assert!(g.trash.has(id::QUARRY), "Mint trashes the Quarry it was bought with");
    assert_eq!(g.cost(id::KINGS_COURT), 5, "Actions still cost $2 less");
}

#[test]
fn a_hoard_played_twice_with_tiara_gains_two_golds_for_a_bought_victory_card() {
    let mut g = new_state(&[id::TIARA, id::HOARD], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::HOARD]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    choose(&mut g, Choice::Card(id::HOARD)); // played twice: $4
    assert_eq!(g.turn.coins, 4);
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].discard.get(id::GOLD), 2);
}

#[test]
fn a_hoard_still_pays_gold_for_a_bought_victory_card_after_a_mint_trashed_it() {
    let mut g = new_state(&[id::HOARD, id::MINT], 2);
    set_hand(&mut g, 0, &[id::HOARD, id::GOLD, id::GOLD]);
    expect_decision(&mut g); // $8
    g.turn.buys = 2;
    g.apply(Choice::Card(id::MINT), &mut NoEvents).unwrap();
    assert!(g.trash.has(id::HOARD) && g.players[0].in_play.is_empty());
    expect_decision(&mut g);
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].discard.get(id::GOLD), 1, "this turn's Hoard still applies");
}

#[test]
fn a_collection_played_twice_with_tiara_gives_two_vp_per_gained_action_card() {
    let mut g = new_state(&[id::TIARA, id::COLLECTION, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::COLLECTION]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    choose(&mut g, Choice::Card(id::COLLECTION));
    g.apply(Choice::Card(id::VILLAGE), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].vp_tokens, 2);
}

#[test]
fn tiara_still_offers_to_topdeck_a_mint_that_trashes_the_tiara() {
    let mut g = new_state(&[id::TIARA, id::MINT], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::GOLD, id::GOLD]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    pass(&mut g); // nothing to play twice yet: the Golds are auto-played next
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    g.apply(Choice::Card(id::MINT), &mut NoEvents).unwrap();
    assert!(g.trash.has(id::TIARA) && g.trash.get(id::GOLD) == 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, .. }), "{d:?}");
    g.apply(Choice::Card(id::MINT), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::MINT));
}

// ===========================================================================
// Investment
// ===========================================================================

#[test]
fn an_investment_played_twice_cannot_be_trashed_for_vp_by_its_second_play() {
    let mut g = new_state(&[id::TIARA, id::INVESTMENT], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::INVESTMENT, id::COPPER, id::COPPER, id::SILVER, id::ESTATE]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    choose(&mut g, Choice::Card(id::INVESTMENT)); // played twice
    choose(&mut g, Choice::Card(id::ESTATE)); // 1st play: trash a card from hand
    let trash_self = cards::modes(id::INVESTMENT)
        .iter()
        .position(|m| matches!(m, cards::ModeOpt::TrashSelfRevealVpPerTreasureType))
        .unwrap() as u8;
    choose(&mut g, Choice::Mode(trash_self)); // trash this: Copper and Silver in hand
    assert_eq!(g.players[0].vp_tokens, 2);
    assert!(g.trash.has(id::INVESTMENT));
    choose(&mut g, Choice::Card(id::COPPER)); // 2nd play: trash a card from hand
    choose(&mut g, Choice::Mode(trash_self)); // nothing left to trash
    assert_eq!(g.players[0].vp_tokens, 2, "no second payout");
    assert_eq!(g.trash.get(id::INVESTMENT), 1);
}

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
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::CURSE));
    assert!(!g.players[0].discard.has(id::CURSE));

    let mut g = new_state(&[id::REPLACE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::COPPER]);
    play(&mut g, id::REPLACE);
    g.apply(Choice::Card(id::CURSE), &mut NoEvents).unwrap();
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
fn a_moat_revealed_when_corsair_is_played_protects_from_its_silver_trashing() {
    let mut g = new_state(&[id::CORSAIR, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::CORSAIR]);
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::CORSAIR);
    assert_eq!(g.players[0].shielded, 0b10);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::SILVER, id::GOLD]);
    expect_decision(&mut g);
    assert!(g.trash.is_empty(), "the Moat player's Silver and Gold are left alone");
}

#[test]
fn a_lighthouse_in_play_when_corsair_is_played_protects_but_one_played_later_does_not() {
    // In play at the time: protected.
    let mut g = new_state(&[id::CORSAIR, id::LIGHTHOUSE], 2);
    set_hand(&mut g, 0, &[id::CORSAIR]);
    set_in_play(&mut g, 1, &[id::LIGHTHOUSE]);
    play(&mut g, id::CORSAIR);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::SILVER]);
    expect_decision(&mut g);
    assert!(g.trash.is_empty());

    // Played afterwards, on the victim's own turn: too late.
    let mut g = new_state(&[id::CORSAIR, id::LIGHTHOUSE], 2);
    set_hand(&mut g, 0, &[id::CORSAIR]);
    play(&mut g, id::CORSAIR);
    assert_eq!(g.players[0].shielded, 0);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::LIGHTHOUSE, id::SILVER]);
    play(&mut g, id::LIGHTHOUSE);
    expect_decision(&mut g);
    assert!(g.trash.has(id::SILVER), "Lighthouse protects only from Attacks played while it is in play");
}

#[test]
fn a_moat_revealed_when_blockade_is_played_protects_from_its_curse() {
    let mut g = new_state(&[id::BLOCKADE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::BLOCKADE]);
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::BLOCKADE);
    choose(&mut g, Choice::Card(id::SILVER)); // set aside
    pass(&mut g); // end player 0's turn
    pass(&mut g); // player 1 plays no Action
    g.turn.coins = 3;
    buy(&mut g, id::SILVER);
    assert!(!g.players[1].all_cards().has(id::CURSE), "the Moat player gains no Curse");
}

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

#[test]
fn the_shield_against_lingering_attacks_survives_the_text_format() {
    let mut g = new_state(&[id::CORSAIR, id::MOAT], 3);
    g.players[0].shielded = 0b110;
    let text = format_state(&g);
    assert!(text.contains("shielded: 2 3"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].shielded, 0b110);
    assert_eq!(back.players[1].shielded, 0);
}

// ===========================================================================
// Monkey
// ===========================================================================

#[test]
fn monkey_stops_drawing_when_its_owners_next_turn_starts() {
    // The Monkey is still in play during its owner's next turn, but the effect has ended: a
    // Witch curse to the player on the owner's right draws nothing.
    let mut g = new_state(&[id::MONKEY, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_in_play(&mut g, 0, &[id::MONKEY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WITCH);
    assert_eq!(g.players[0].hand.total(), 2, "just the Witch's own +2 Cards");

    // Played this turn, the effect is on from the moment it is played.
    let mut g = new_state(&[id::MONKEY, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_in_play(&mut g, 0, &[id::MONKEY]);
    inject_pending(&mut g, 0, id::MONKEY, 1, 0);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WITCH);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn a_monkey_played_twice_draws_two_cards_per_card_the_player_on_its_right_gains() {
    let mut g = new_state(&[id::MONKEY, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_in_play(&mut g, 1, &[id::MONKEY]);
    inject_pending(&mut g, 1, id::MONKEY, 2, 0); // Throne Room or King's Court: two plays
    set_deck_known(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[1].hand.total(), 2);
}

// ===========================================================================
// Reactions and attacks: Diplomat, Clerk, Pirate
// ===========================================================================

#[test]
fn a_diplomat_can_be_revealed_again_while_the_hand_still_has_five_cards() {
    let mut g = new_state(&[id::MILITIA, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    let Step::Decision(d) = play(&mut g, id::MILITIA) else { panic!() };
    assert!(is_yes_no(&d) && d.player == 1);
    choose(&mut g, Choice::Yes); // draw 2 (8 cards), discard 3
    discard_for_diplomat(&mut g, id::COPPER);
    // Five cards left and the Diplomat is still in hand: again.
    assert_eq!(g.players[1].hand.total(), 5);
    let d = expect_decision(&mut g);
    assert!(is_yes_no(&d) && d.player == 1, "{d:?}");
    choose(&mut g, Choice::Yes); // 7 cards, discard 3: 4 left
    discard_for_diplomat(&mut g, id::ESTATE);
    assert_eq!(g.players[1].hand.total(), 4);
    let d = expect_decision(&mut g);
    assert!(!is_yes_no(&d), "a hand of four cards can't reveal it again: {d:?}");
}

#[test]
fn a_victim_left_with_four_cards_by_a_diplomat_does_not_topdeck_for_clerk() {
    let mut g = new_state(&[id::CLERK, id::DIPLOMAT], 2);
    g.turn.turn_start_resolved = true; // no Clerk start-of-turn offer
    set_hand(&mut g, 0, &[id::CLERK]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    let Step::Decision(d) = play(&mut g, id::CLERK) else { panic!() };
    assert!(is_yes_no(&d) && d.player == 1);
    choose(&mut g, Choice::Yes); // 7 cards, discard 3: 4 left
    discard_for_diplomat(&mut g, id::COPPER);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.kind), (0, DecisionKind::Buy), "Clerk found only four cards: nothing to put back");
    assert_eq!(g.players[1].hand.total(), 4);
}

#[test]
fn a_clerk_still_hits_a_victim_with_five_cards_who_did_not_react() {
    let mut g = new_state(&[id::CLERK], 2);
    g.turn.turn_start_resolved = true; // no Clerk start-of-turn offer
    set_hand(&mut g, 0, &[id::CLERK]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    let Step::Decision(d) = play(&mut g, id::CLERK) else { panic!() };
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, min: 1, max: 1, .. }), "{d:?}");
}

#[test]
fn each_pirate_in_hand_may_be_played_when_a_treasure_is_gained() {
    let mut g = new_state(&[id::PIRATE, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_hand(&mut g, 1, &[id::PIRATE, id::PIRATE]);
    play(&mut g, id::WORKSHOP);
    let Step::Decision(d) = choose(&mut g, Choice::Card(id::SILVER)) else { panic!() };
    assert!(is_yes_no(&d) && d.player == 1);
    let Step::Decision(d) = choose(&mut g, Choice::Yes) else { panic!() };
    assert!(is_yes_no(&d) && d.player == 1, "the second Pirate: {d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[1].in_play.get(id::PIRATE), 2);
    assert_eq!(g.players[1].pending_durations[0].times, 2);
}

#[test]
fn players_react_with_pirates_in_turn_order_starting_with_the_turn_player() {
    let mut g = new_state(&[id::PIRATE, id::WORKSHOP], 3);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::PIRATE]);
    set_hand(&mut g, 1, &[id::PIRATE]);
    set_hand(&mut g, 2, &[id::PIRATE]);
    play(&mut g, id::WORKSHOP);
    let mut order = Vec::new();
    let mut step = choose(&mut g, Choice::Card(id::SILVER));
    while let Step::Decision(d) = step {
        if !is_yes_no(&d) {
            break;
        }
        order.push(d.player);
        step = choose(&mut g, Choice::No);
    }
    assert_eq!(order, vec![0, 1, 2]);
}

#[test]
fn a_pirate_another_player_plays_does_not_count_as_an_action_you_played() {
    // Conspirator needs 3 Actions played this turn, counting itself: Workshop and Conspirator are
    // two, however many Pirates other players play in between.
    let mut g = new_state(&[id::CONSPIRATOR, id::WORKSHOP, id::PIRATE], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::CONSPIRATOR]);
    set_hand(&mut g, 1, &[id::PIRATE]);
    g.turn.actions = 3;
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Yes); // player 1 plays the Pirate
    play(&mut g, id::CONSPIRATOR);
    assert_eq!(g.turn.actions, 1, "3 - Workshop - Conspirator: no +1 Action");
    assert_eq!(g.turn.coins, 2);
}

// ===========================================================================
// Herbalist and Astrolabe
// ===========================================================================

#[test]
fn herbalist_cannot_put_an_astrolabe_that_stays_in_play_onto_the_deck() {
    let mut g = new_state(&[id::HERBALIST, id::ASTROLABE], 2);
    set_hand(&mut g, 0, &[id::HERBALIST, id::ASTROLABE, id::COPPER]);
    play(&mut g, id::HERBALIST);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    pass(&mut g); // end of the Buy phase: Herbalist's offer
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::COPPER)) && c.contains(&Choice::Pass));
    assert!(!c.contains(&Choice::Card(id::ASTROLABE)), "it is kept for next turn, not discarded: {c:?}");
}

#[test]
fn herbalist_may_put_an_astrolabe_from_last_turn_onto_the_deck() {
    // Played on the previous turn, it is discarded at this cleanup like any other card.
    let mut g = new_state(&[id::HERBALIST, id::ASTROLABE], 2);
    set_hand(&mut g, 0, &[id::HERBALIST]);
    set_in_play(&mut g, 0, &[id::ASTROLABE]);
    play(&mut g, id::HERBALIST);
    expect_decision(&mut g);
    pass(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::ASTROLABE)));
}
