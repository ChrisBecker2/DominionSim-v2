//! Alchemy card tests, ported from the old C++ simulator's
//! `DominionSimTestCards\AlchemyCardsTests.cpp` (Alchemy has a single edition, so every card
//! there exists here except Possession, which is not implemented, by decision), adapted to this
//! engine's API. Each test cites the C++ `TEST_METHOD` (and line) it came from. The Potion-cost
//! rules (Remodel, Upgrade, Forge, Bridge, ...) have no C++ counterpart and are written from the
//! rules of the game. See `docs/alchemy-plan.md`.
//!
//! Adapting C++ assertions:
//! - `VerifyTurnBasics(actions, buys, coins)` there is checked before treasures are played; this
//!   engine auto-plays choice-free treasures on entering the Buy phase, so coins include them.
//! - Cards of other sets the C++ tests borrow as decoys (Adventurer, Necropolis, Hovel,
//!   Woodcutter, Poor House, Hamlet, the Ruins ...) are replaced by 2nd-edition cards with the
//!   same relevant properties (Village for a +Actions card, Festival for a "+$" terminal, Estate
//!   for Hovel); the Ruins and shelter cases, the -1 Card token and the Debt cases (Engineer,
//!   City Quarter, Fortune) are not ported (those sets are not in this simulator).
//! - "Simulation" sub-cases test the C++ bot harness, not the rules; the bot defaults are tested
//!   in `crates/sim/tests/alchemy_defaults.rs`.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::state::vp_of_cards;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn top_down(g: &GameState, p: usize) -> Vec<CardId> {
    g.players[p].deck_known.iter_top_down().collect()
}

fn offered(g: &GameState) -> Vec<CardId> {
    choices(g).into_iter().filter_map(|c| if let Choice::Card(x) = c { Some(x) } else { None }).collect()
}

// ===========================================================================
// Potion — C++ TestPotion (15)
// ===========================================================================

#[test]
fn potion_basics() {
    let d = cards::def(id::POTION);
    assert_eq!((d.cost, d.potion, d.vp), (4, false, 0));
    assert!(!cards::is_kingdom(id::POTION) && cards::is_optional_basic(id::POTION));
    assert_eq!(cards::set_of(id::POTION), CardSet::Alchemy);
    // The pile (16) joins the supply when a Potion-cost card is in the kingdom, and only then.
    let g = new_state(&[id::FAMILIAR], 2);
    assert!(g.in_supply(id::POTION));
    assert_eq!(g.supply.get(id::POTION), 16);
    let g = new_state(&[id::HERBALIST], 2);
    assert!(!g.in_supply(id::POTION), "Herbalist has no Potion in its cost");
}

#[test]
fn potion_gives_a_potion_when_played() {
    // One Potion; two Potions; mixed with other treasures (C++ cases, in order).
    for (hand, coins, potions) in [
        (vec![id::POTION], 0u16, 1u8),
        (vec![id::POTION, id::POTION], 0, 2),
        (vec![id::POTION, id::COPPER, id::SILVER], 3, 1),
        (vec![id::POTION, id::COPPER, id::SILVER, id::POTION], 3, 2),
    ] {
        let mut g = new_state(&[id::FAMILIAR], 2);
        set_hand(&mut g, 0, &hand);
        let d = expect_decision(&mut g);
        assert_eq!(d.kind, DecisionKind::Buy);
        assert_eq!((g.turn.coins, g.turn.potions), (coins, potions), "{hand:?}");
        assert!(g.players[0].hand.is_empty());
        assert_eq!(g.players[0].in_play.total() as usize, hand.len());
    }
}

#[test]
fn potions_do_not_carry_over_to_the_next_turn() {
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::POTION]);
    expect_decision(&mut g);
    assert_eq!(g.turn.potions, 1);
    pass(&mut g);
    expect_decision(&mut g);
    assert_eq!((g.turn.player, g.turn.potions), (1, 0));
}

#[test]
fn potion_plays_alongside_a_treasure_with_a_choice() {
    // Bank has a choice, so nothing auto-plays: Potion is offered like every other Treasure.
    let mut g = new_state(&[id::FAMILIAR, id::BANK], 2);
    set_hand(&mut g, 0, &[id::POTION, id::BANK]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    assert!(offered(&g).contains(&id::POTION));
    play_treasure(&mut g, id::POTION);
    assert_eq!(g.turn.potions, 1);
}

// ===========================================================================
// Buying with Potions: a card with a Potion in its cost needs the coins AND a Potion.
// ===========================================================================

#[test]
fn a_potion_cost_card_can_only_be_bought_with_a_potion() {
    // Without a Potion: $3 buys Silver, not Familiar ($3P).
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert!(offered(&g).contains(&id::SILVER));
    assert!(!offered(&g).contains(&id::FAMILIAR));

    // With a Potion: both, and the buy spends the Potion as well as the coins.
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::POTION]);
    expect_decision(&mut g);
    assert_eq!((g.turn.coins, g.turn.potions), (3, 1));
    assert!(offered(&g).contains(&id::FAMILIAR));
    buy(&mut g, id::FAMILIAR);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0);
    assert_eq!(g.supply.get(id::FAMILIAR), 9);
}

#[test]
fn a_potion_is_spent_per_potion_card_bought() {
    let mut g = new_state(&[id::FAMILIAR, id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::POTION]);
    g.turn.buys = 2;
    expect_decision(&mut g);
    assert_eq!((g.turn.coins, g.turn.potions), (5, 1));
    let mut sink: Vec<Event> = Vec::new();
    g.apply(Choice::Card(id::FAMILIAR), &mut sink).unwrap();
    adv(&mut g);
    assert_eq!((g.turn.coins, g.turn.potions), (2, 0));
    // Apothecary ($2P) is no longer affordable: the one Potion is spent.
    assert!(!offered(&g).contains(&id::APOTHECARY));
    assert!(offered(&g).contains(&id::ESTATE));
}

#[test]
fn potion_itself_costs_four_coins_and_no_potion() {
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    expect_decision(&mut g);
    assert!(offered(&g).contains(&id::POTION));
    buy(&mut g, id::POTION);
    assert_eq!(g.supply.get(id::POTION), 15);
}

#[test]
fn potions_survive_a_text_round_trip() {
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::POTION, id::COPPER]);
    expect_decision(&mut g);
    let text = format_state(&g);
    assert!(text.contains("potions: 1"), "{text}");
    assert!(text.contains("Potion=16"), "{text}");
    let g2 = parse_state(&text).unwrap();
    assert_eq!(g2.turn.potions, 1);
    assert!(g2.in_supply(id::POTION));
    // No Potion: nothing printed, so Base-only saves are unchanged.
    let g3 = new_state(&[id::VILLAGE], 2);
    assert!(!format_state(&g3).contains("potions"));
}

// ===========================================================================
// Potion costs in "costing up to / exactly" gains and in cost reductions.
// ===========================================================================

#[test]
fn remodel_on_a_potion_card_may_gain_potion_cost_cards() {
    // Golem ($4P) -> up to $6P: potion-cost cards allowed.
    let mut g = new_state(&[id::REMODEL, id::GOLEM, id::FAMILIAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::GOLEM]);
    play(&mut g, id::REMODEL);
    let d = expect_decision(&mut g);
    assert_eq!(
        d.kind,
        DecisionKind::Gain { max_cost: 6, filter: Filter::Any, dest: Dest::Discard, exact: false, potion: true, optional: false }
    );
    let o = offered(&g);
    for c in [id::GOLEM, id::FAMILIAR, id::SMITHY, id::GOLD] {
        assert!(o.contains(&c), "{}", cards::name(c));
    }
    assert!(!o.contains(&id::PROVINCE));

    // Estate ($2) -> up to $4, no Potion cards (Golem, Familiar), Potion itself ($4) is fine.
    let mut g = new_state(&[id::REMODEL, id::GOLEM, id::FAMILIAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE]);
    play(&mut g, id::REMODEL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, potion: false, .. }));
    let o = offered(&g);
    assert!(o.contains(&id::SMITHY) && o.contains(&id::POTION) && o.contains(&id::SILVER));
    assert!(!o.contains(&id::GOLEM) && !o.contains(&id::FAMILIAR));

    // Apothecary ($2P) -> up to $4P: Golem and Familiar too.
    let mut g = new_state(&[id::REMODEL, id::APOTHECARY, id::GOLEM, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::APOTHECARY]);
    play(&mut g, id::REMODEL);
    expect_decision(&mut g);
    let o = offered(&g);
    assert!(o.contains(&id::GOLEM) && o.contains(&id::FAMILIAR) && o.contains(&id::SILVER));
    assert!(!o.contains(&id::GOLD));
}

#[test]
fn upgrade_needs_the_potion_to_match_exactly() {
    // Apothecary ($2P) -> exactly $3P: Familiar, Alchemist and Philosopher's Stone, not Silver.
    let k = [id::UPGRADE, id::APOTHECARY, id::FAMILIAR, id::ALCHEMIST, id::PHILOSOPHERS_STONE, id::VILLAGE];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::UPGRADE, id::APOTHECARY]);
    play(&mut g, id::UPGRADE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 3, exact: true, potion: true, .. }));
    let mut o = offered(&g);
    o.sort_unstable();
    assert_eq!(o, vec![id::ALCHEMIST, id::FAMILIAR, id::PHILOSOPHERS_STONE]);

    // Estate ($2) -> exactly $3 with no Potion: Silver and Village, not the $3P cards.
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::UPGRADE, id::ESTATE]);
    play(&mut g, id::UPGRADE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 3, exact: true, potion: false, .. }));
    let mut o = offered(&g);
    o.sort_unstable();
    assert_eq!(o, vec![id::SILVER, id::VILLAGE]);
}

#[test]
fn forge_counts_only_coins_and_cannot_gain_a_potion_cost_card() {
    let k = [id::FORGE, id::GOLEM, id::APOTHECARY, id::SMITHY];
    // Golem ($4P) trashed: gain exactly $4, but not a card with a Potion in its cost.
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::FORGE, id::GOLEM]);
    play(&mut g, id::FORGE);
    choose(&mut g, Choice::Card(id::GOLEM));
    let d = pending_or_next(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, exact: true, potion: false, .. }), "{:?}", d.kind);
    let o = offered(&g);
    assert!(o.contains(&id::SMITHY) && o.contains(&id::POTION));
    assert!(!o.contains(&id::GOLEM));

    // Apothecary ($2P) + Estate ($2): the Potion adds nothing, the total is $4.
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::FORGE, id::APOTHECARY, id::ESTATE]);
    play(&mut g, id::FORGE);
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::APOTHECARY));
    let d = pending_or_next(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, exact: true, potion: false, .. }), "{:?}", d.kind);
    assert!(offered(&g).contains(&id::SMITHY));
}

fn pending_or_next(g: &mut GameState) -> Decision {
    match g.pending_decision() {
        Some(d) => d,
        None => expect_decision(g),
    }
}

#[test]
fn bridge_and_quarry_reduce_only_the_coin_part_of_a_cost() {
    // Bridge: Familiar $3P -> $2P; still needs a Potion.
    let mut g = new_state(&[id::BRIDGE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::BRIDGE, id::COPPER]);
    play(&mut g, id::BRIDGE);
    assert_eq!(g.cost(id::FAMILIAR), 2);
    assert!(cards::potion_cost(id::FAMILIAR));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 2);
    assert!(!offered(&g).contains(&id::FAMILIAR), "$2 is enough, but there is no Potion");

    let mut g = new_state(&[id::BRIDGE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::BRIDGE, id::COPPER, id::POTION]);
    play(&mut g, id::BRIDGE);
    expect_decision(&mut g);
    assert!(offered(&g).contains(&id::FAMILIAR));

    // Quarry: Actions cost $2 less: Apothecary $2P -> $0P (and is free of coins, not of Potions).
    let mut g = new_state(&[id::QUARRY, id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::QUARRY]);
    expect_decision(&mut g);
    assert_eq!(g.cost(id::APOTHECARY), 0);
    assert!(!offered(&g).contains(&id::APOTHECARY), "no Potion");
    let mut g = new_state(&[id::QUARRY, id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::QUARRY, id::POTION]);
    expect_decision(&mut g);
    assert!(offered(&g).contains(&id::APOTHECARY));
}

#[test]
fn bishop_counts_only_coins() {
    let mut g = new_state(&[id::BISHOP, id::GOLEM], 2);
    set_hand(&mut g, 0, &[id::BISHOP, id::GOLEM]);
    play(&mut g, id::BISHOP);
    // 1 token for playing it + $4 / 2 for the trashed Golem; the Potion is ignored.
    assert_eq!(g.players[0].vp_tokens, 1 + 2);
    assert!(g.trash.has(id::GOLEM));
}

#[test]
fn workshop_and_smugglers_never_gain_potion_cost_cards() {
    let mut g = new_state(&[id::WORKSHOP, id::GOLEM, id::FAMILIAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    expect_decision(&mut g);
    let o = offered(&g);
    assert!(o.contains(&id::SMITHY) && !o.contains(&id::GOLEM) && !o.contains(&id::FAMILIAR));

    // Smugglers copies a card the right-hand player gained, up to $6: Golem is excluded.
    let mut g = new_state(&[id::SMUGGLERS, id::GOLEM], 2);
    g.players[1].last_turn_gains.add(id::GOLEM, 1);
    g.players[1].last_turn_gains.add(id::SILVER, 1);
    set_hand(&mut g, 0, &[id::SMUGGLERS]);
    play(&mut g, id::SMUGGLERS);
    assert!(g.players[0].discard.has(id::SILVER), "the only legal copy: Silver (auto-applied)");
    assert!(!g.players[0].discard.has(id::GOLEM));
}

#[test]
fn swindler_gives_a_card_with_the_same_cost_including_the_potion() {
    // Victim's top card Golem ($4P): the replacement must cost exactly $4P (only Golem here).
    let mut g = new_state(&[id::SWINDLER, id::GOLEM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::GOLEM]);
    play(&mut g, id::SWINDLER);
    assert!(g.trash.has(id::GOLEM));
    assert!(g.players[1].discard.has(id::GOLEM), "the only card at $4P is another Golem (auto-applied)");

    // Top card Smithy ($4): Smithy or Potion ($4), never the $4P Golem.
    let mut g = new_state(&[id::SWINDLER, id::GOLEM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::SMITHY]);
    play(&mut g, id::SWINDLER);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.for_player), (0, 1));
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, exact: true, potion: false, .. }));
    let mut o = offered(&g);
    o.sort_unstable();
    assert_eq!(o, vec![id::SMITHY, id::POTION]);
}

// ===========================================================================
// Vineyard — C++ TestVineyard (66)
// ===========================================================================

fn vp(cs: &[(CardId, u8)]) -> i32 {
    let mut c = Counts::EMPTY;
    for &(x, n) in cs {
        c.add(x, n);
    }
    vp_of_cards(&c)
}

#[test]
fn vineyard_is_worth_one_vp_per_three_actions() {
    let d = cards::def(id::VINEYARD);
    assert_eq!((d.cost, d.potion, d.vp), (0, true, 0));
    assert!(cards::is(id::VINEYARD, cards::VICTORY) && !cards::is(id::VINEYARD, cards::ACTION));
    // Worth 0 on its own, and everything rounds down.
    assert_eq!(vp(&[]), 0);
    assert_eq!(vp(&[(id::VINEYARD, 1)]), 0);
    assert_eq!(vp(&[(id::VINEYARD, 9)]), 0);
    assert_eq!(vp(&[(id::VINEYARD, 1), (id::VILLAGE, 1)]), 0);
    assert_eq!(vp(&[(id::VINEYARD, 1), (id::VILLAGE, 2)]), 0);
    // Now worth 1.
    assert_eq!(vp(&[(id::VINEYARD, 1), (id::VILLAGE, 3)]), 1);
    assert_eq!(vp(&[(id::VINEYARD, 3), (id::VILLAGE, 3)]), 3);
    // 3 Nobles are 3x2 VP, and are also Actions: 6 / 3 = 2 for the Vineyard.
    assert_eq!(vp(&[(id::VINEYARD, 1), (id::VILLAGE, 3), (id::NOBLES, 3)]), 8);
    // Other card types don't count.
    assert_eq!(vp(&[(id::VINEYARD, 1), (id::VILLAGE, 3), (id::COPPER, 3), (id::ESTATE, 1)]), 1 + 1);
    // Cards on every player zone count (the player's score uses `all_cards`).
    let mut g = new_state(&[id::VINEYARD, id::VILLAGE], 2);
    g.players[0].hand = counts_of(&[id::VINEYARD, id::VILLAGE]);
    g.players[0].discard = counts_of(&[id::VILLAGE, id::VILLAGE]);
    assert_eq!(g.players[0].vp(), 1);
}

#[test]
fn vineyard_pile_is_a_victory_pile() {
    assert_eq!(new_state(&[id::VINEYARD], 2).supply.get(id::VINEYARD), 8);
    assert_eq!(new_state(&[id::VINEYARD], 3).supply.get(id::VINEYARD), 12);
}

// ===========================================================================
// Transmute — C++ TestTransmute (95)
// ===========================================================================

fn transmute(kingdom: &[CardId], hand_extra: &[CardId]) -> GameState {
    let mut g = new_state(kingdom, 2);
    let mut hand = vec![id::TRANSMUTE];
    hand.extend_from_slice(hand_extra);
    set_hand(&mut g, 0, &hand);
    play(&mut g, id::TRANSMUTE);
    g
}

#[test]
fn transmute_basics() {
    let d = cards::def(id::TRANSMUTE);
    assert_eq!((d.cost, d.potion, d.vp), (0, true, 0));
    assert!(cards::is(id::TRANSMUTE, cards::ACTION));
}

#[test]
fn transmute_an_action_gains_a_duchy() {
    let g = transmute(&[id::VILLAGE], &[id::VILLAGE]);
    assert_eq!((g.turn.actions, g.turn.buys), (0, 1));
    assert!(g.trash.has(id::VILLAGE) && g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY]));
    assert_eq!(g.supply.get(id::DUCHY), 7);
}

#[test]
fn transmute_a_victory_card_gains_a_gold() {
    let g = transmute(&[id::VILLAGE], &[id::ESTATE]);
    assert!(g.trash.has(id::ESTATE));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
}

#[test]
fn transmute_a_treasure_gains_a_transmute() {
    let g = transmute(&[id::TRANSMUTE], &[id::COPPER]);
    assert!(g.trash.has(id::COPPER));
    assert_eq!(g.players[0].discard, counts_of(&[id::TRANSMUTE]));
    assert_eq!(g.supply.get(id::TRANSMUTE), 9);
}

#[test]
fn transmute_a_multi_type_card_gains_each() {
    // Nobles (Action-Victory): Gold and Duchy (C++ lists {Gold, Duchy}).
    let g = transmute(&[id::VILLAGE], &[id::NOBLES]);
    assert!(g.trash.has(id::NOBLES));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD, id::DUCHY]));
    // Harem (Treasure-Victory): Gold and Transmute.
    let g = transmute(&[id::TRANSMUTE], &[id::HAREM]);
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD, id::TRANSMUTE]));
}

#[test]
fn transmute_a_curse_gains_nothing() {
    let g = transmute(&[id::VILLAGE], &[id::CURSE]);
    assert!(g.trash.has(id::CURSE));
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn transmute_with_no_other_card_in_hand_does_nothing() {
    let mut g = transmute(&[id::VILLAGE], &[]);
    assert!(g.trash.is_empty() && g.players[0].discard.is_empty());
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn transmute_must_trash_a_card_if_it_can() {
    let g = transmute(&[id::VILLAGE], &[id::PROVINCE, id::ESTATE]);
    // A real decision with no Pass.
    let mut g = g;
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, min: 1, max: 1, .. }));
    assert!(!choices(&g).contains(&Choice::Pass));
}

#[test]
fn transmute_gains_are_skipped_when_the_pile_is_empty_or_absent() {
    // No Gold left: trashing an Estate gains nothing.
    let mut g = new_state(&[id::VILLAGE], 2);
    empty_pile(&mut g, id::GOLD);
    set_hand(&mut g, 0, &[id::TRANSMUTE, id::ESTATE]);
    play(&mut g, id::TRANSMUTE);
    assert!(g.trash.has(id::ESTATE) && g.players[0].discard.is_empty());
    // No Duchy left.
    let mut g = new_state(&[id::VILLAGE], 2);
    empty_pile(&mut g, id::DUCHY);
    set_hand(&mut g, 0, &[id::TRANSMUTE, id::VILLAGE]);
    play(&mut g, id::TRANSMUTE);
    assert!(g.trash.has(id::VILLAGE) && g.players[0].discard.is_empty());
    // Transmute is not in the kingdom.
    let g = transmute(&[id::VILLAGE], &[id::COPPER]);
    assert!(g.trash.has(id::COPPER) && g.players[0].discard.is_empty());
    // The Transmute pile is empty.
    let mut g = new_state(&[id::TRANSMUTE], 2);
    empty_pile(&mut g, id::TRANSMUTE);
    set_hand(&mut g, 0, &[id::TRANSMUTE, id::COPPER]);
    play(&mut g, id::TRANSMUTE);
    assert!(g.trash.has(id::COPPER) && g.players[0].discard.is_empty());
    // Harem with no Transmute left: still the Gold.
    let mut g = new_state(&[id::TRANSMUTE], 2);
    empty_pile(&mut g, id::TRANSMUTE);
    set_hand(&mut g, 0, &[id::TRANSMUTE, id::HAREM]);
    play(&mut g, id::TRANSMUTE);
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
}

#[test]
fn transmute_treats_curse_as_a_treasure_under_charlatan() {
    let g = transmute(&[id::TRANSMUTE, id::CHARLATAN], &[id::CURSE]);
    assert_eq!(g.players[0].discard, counts_of(&[id::TRANSMUTE]));
}

// ===========================================================================
// Apothecary — C++ TestApothecary (305)
// ===========================================================================

#[test]
fn apothecary_basics() {
    let d = cards::def(id::APOTHECARY);
    assert_eq!((d.cost, d.potion, d.vp), (2, true, 0));
}

#[test]
fn apothecary_takes_coppers_and_potions_and_orders_the_rest() {
    // Draw Province; reveal Copper, Gold, Potion, Throne Room: Copper and Potion to hand; Gold and
    // Throne Room go back in the order chosen. The Copper and Potion then auto-play in the Buy
    // phase, so they show up in play (and as $1 and 1 Potion).
    let mut g = new_state(&[id::APOTHECARY, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::COPPER, id::GOLD, id::POTION, id::THRONE_ROOM]);
    play(&mut g, id::APOTHECARY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, ordered: true, from: Zone::Revealed, .. }));
    assert_eq!(g.turn.actions, 1);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE, id::COPPER, id::POTION]));
    // The last card put back is the new top; with one card left the engine puts it back itself.
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(top_down(&g, 0), vec![id::THRONE_ROOM, id::GOLD]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!((g.turn.coins, g.turn.potions), (1, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
}

#[test]
fn apothecary_all_coppers_and_potions_means_nothing_to_order() {
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::COPPER, id::POTION, id::POTION, id::COPPER]);
    set_deck_unknown(&mut g, 0, &[id::VILLAGE]);
    play(&mut g, id::APOTHECARY);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy, "no decision about putting cards back");
    assert_eq!((g.turn.coins, g.turn.potions), (2, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
    assert_eq!(g.players[0].deck_known.len, 0);
    assert_eq!(g.players[0].deck_unknown, counts_of(&[id::VILLAGE]));
}

#[test]
fn apothecary_with_nothing_to_take_puts_all_four_back() {
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::GOLD, id::MARKET, id::PROVINCE, id::CURSE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::APOTHECARY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, ordered: true, .. }));
    // Put back in the order Curse, Market, Gold, Province (the last is on top).
    for c in [id::CURSE, id::MARKET, id::GOLD] {
        choose(&mut g, Choice::Card(c)); // the last card (Province) is put back automatically
    }
    assert_eq!(top_down(&g, 0), vec![id::PROVINCE, id::GOLD, id::MARKET, id::CURSE]);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
    assert_eq!(g.players[0].deck_unknown, counts_of(&[id::ESTATE]));
}

#[test]
fn apothecary_with_a_short_deck() {
    // Only 3 cards left after the draw: Village, Market, Copper -> Copper to hand, 2 back.
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::VILLAGE, id::MARKET, id::COPPER]);
    play(&mut g, id::APOTHECARY);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::MARKET));
    assert_eq!(top_down(&g, 0), vec![id::VILLAGE, id::MARKET]);
    assert_eq!(g.players[0].hand.get(id::PROVINCE), 1);

    // 2 cards after the draw, both Copper: all to hand, no decision.
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::COPPER, id::COPPER]);
    play(&mut g, id::APOTHECARY);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 2);

    // 1 card after the draw, an Action: it goes back, no decision.
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::PLATINUM, id::VILLAGE]);
    play(&mut g, id::APOTHECARY);
    expect_decision(&mut g);
    assert_eq!(top_down(&g, 0), vec![id::VILLAGE]);

    // No cards after the draw; and no cards at all.
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::PLATINUM]);
    play(&mut g, id::APOTHECARY);
    expect_decision(&mut g);
    assert_eq!(g.turn.coins, 5, "the Platinum was drawn and played");
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    play(&mut g, id::APOTHECARY);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn apothecary_reshuffles_the_discard_pile_when_the_deck_runs_out() {
    let mut g = new_state(&[id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::APOTHECARY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::SILVER]);
    set_discard(&mut g, 0, &[id::COPPER, id::COPPER]);
    play(&mut g, id::APOTHECARY);
    // Drew Estate; revealed Silver, then (after the reshuffle) both Coppers.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Buy), "Silver goes back on top and is the only card: auto");
    assert_eq!(g.turn.coins, 2, "the 2 Coppers");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
}

// ===========================================================================
// Scrying Pool — C++ TestScryingPool (431)
// ===========================================================================

#[test]
fn scrying_pool_basics() {
    let d = cards::def(id::SCRYING_POOL);
    assert_eq!((d.cost, d.potion, d.vp), (2, true, 0));
    assert!(cards::is(id::SCRYING_POOL, cards::ATTACK));
}

/// Play Scrying Pool in a 2-player game with `deck` (top first) as the owner's deck; `keep`:
/// the owner's answer to "discard the top card?" (None = no such decision is expected).
fn scry(deck: &[CardId], discard_it: Option<bool>) -> GameState {
    let mut g = new_state(&[id::SCRYING_POOL, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SCRYING_POOL]);
    set_deck_known(&mut g, 0, deck);
    play(&mut g, id::SCRYING_POOL);
    match discard_it {
        None => {}
        Some(discard) => {
            let d = g.pending_decision().expect("a keep-or-discard decision");
            assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, from: Zone::Revealed, min: 0, max: 1, .. }));
            assert_eq!((d.player, d.for_player), (0, 0));
            assert_eq!(choices(&g).len(), 2, "the card, or Pass (put it back)");
            let c = if discard { Choice::Card(deck[0]) } else { Choice::Pass };
            choose(&mut g, c);
        }
    }
    g
}

#[test]
fn scrying_pool_with_no_cards_does_nothing() {
    let mut g = scry(&[], None);
    assert_eq!(g.turn.actions, 1);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn scrying_pool_keep_a_single_card_then_draw_it() {
    let g = scry(&[id::GOLD], Some(false));
    assert!(g.players[0].hand.has(id::GOLD) || g.players[0].in_play.has(id::GOLD));
    assert!(g.players[0].deck_known.is_empty() && g.players[0].discard.is_empty());
}

#[test]
fn scrying_pool_discarding_the_only_card_reshuffles_it_back_in_and_draws_it() {
    let g = scry(&[id::GOLD], Some(true));
    assert!(g.players[0].hand.has(id::GOLD) || g.players[0].in_play.has(id::GOLD));
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn scrying_pool_discard_then_draw_until_a_non_action() {
    // Discard Gold; reveal Village, Smithy (Actions), Transmute, Nobles (Actions), Silver (stops).
    let mut g = scry(&[id::GOLD, id::VILLAGE, id::SMITHY, id::TRANSMUTE, id::NOBLES, id::SILVER, id::ESTATE], Some(true));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
    // The Silver is in hand and then played in the Buy phase; Estate stays on the deck.
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::SMITHY, id::TRANSMUTE, id::NOBLES, id::SILVER]));
    assert_eq!(top_down(&g, 0), vec![id::ESTATE]);
    assert_eq!(g.turn.actions, 1);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction, "the revealed Actions can be played");
}

#[test]
fn scrying_pool_harem_is_not_an_action_so_it_stops_the_reveal() {
    let g = scry(&[id::GOLD, id::VILLAGE, id::SMITHY, id::TRANSMUTE, id::HAREM, id::ESTATE], Some(true));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::SMITHY, id::TRANSMUTE, id::HAREM]));
    assert_eq!(top_down(&g, 0), vec![id::ESTATE]);
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
}

#[test]
fn scrying_pool_keep_the_card_then_draw() {
    let g = scry(&[id::VILLAGE, id::SMITHY, id::TRANSMUTE, id::HAREM, id::ESTATE], Some(false));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::SMITHY, id::TRANSMUTE, id::HAREM]));
    assert_eq!(top_down(&g, 0), vec![id::ESTATE]);
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn scrying_pool_may_discard_an_action_too() {
    let g = scry(&[id::VILLAGE, id::SMITHY, id::TRANSMUTE, id::HAREM, id::ESTATE], Some(true));
    assert_eq!(g.players[0].hand, counts_of(&[id::SMITHY, id::TRANSMUTE, id::HAREM]));
    assert_eq!(g.players[0].discard, counts_of(&[id::VILLAGE]));
    assert_eq!(top_down(&g, 0), vec![id::ESTATE]);
}

#[test]
fn scrying_pool_all_actions_draws_the_whole_deck() {
    let g = scry(&[id::VILLAGE, id::SMITHY, id::TRANSMUTE], Some(false));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::SMITHY, id::TRANSMUTE]));
    assert_eq!(g.players[0].deck_size(), 0);
}

#[test]
fn scrying_pool_as_an_attack_the_owner_decides_for_each_player() {
    // 4 players: the owner (P1) has no deck. P2's top card is Platinum (discarded by the
    // owner's choice), P3's is Duchy (kept), P4 has no deck.
    let mut g = new_state(&[id::SCRYING_POOL, id::VILLAGE], 4);
    set_hand(&mut g, 0, &[id::SCRYING_POOL]);
    for p in 1..4 {
        set_hand(&mut g, p, &[id::GOLD]);
    }
    set_deck_known(&mut g, 1, &[id::PLATINUM, id::PROVINCE]);
    set_deck_known(&mut g, 2, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::SCRYING_POOL);
    let d = g.pending_decision().expect("decision for P2's top card");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, from: Zone::Revealed, .. }));
    assert_eq!((d.player, d.for_player), (0, 1), "the owner decides, about P2's deck");
    assert_eq!(choices(&g), vec![Choice::Card(id::PLATINUM), Choice::Pass]);
    choose(&mut g, Choice::Card(id::PLATINUM));
    let d = g.pending_decision().expect("decision for P3's top card");
    assert_eq!((d.player, d.for_player), (0, 2));
    choose(&mut g, Choice::Pass);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.players[1].discard, counts_of(&[id::PLATINUM]));
    assert_eq!(top_down(&g, 1), vec![id::PROVINCE]);
    assert_eq!(top_down(&g, 2), vec![id::DUCHY, id::PROVINCE], "put back on top");
    assert!(g.players[3].discard.is_empty() && g.players[3].hand.has(id::GOLD));
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn scrying_pool_is_blocked_by_moat_but_the_owner_still_reveals() {
    let mut g = new_state(&[id::SCRYING_POOL, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::SCRYING_POOL]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 1, &[id::PLATINUM, id::PROVINCE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER]);
    play(&mut g, id::SCRYING_POOL);
    // Only the owner's own top card is asked about.
    let d = g.pending_decision().expect("own top card");
    assert_eq!(d.for_player, 0);
    choose(&mut g, Choice::Pass);
    assert_eq!(top_down(&g, 1), vec![id::PLATINUM, id::PROVINCE]);
    assert!(g.players[1].discard.is_empty());
}

// ===========================================================================
// University — C++ TestUniversity (543)
// ===========================================================================

#[test]
fn university_basics() {
    let d = cards::def(id::UNIVERSITY);
    assert_eq!((d.cost, d.potion, d.vp), (2, true, 0));
}

#[test]
fn university_with_no_action_to_gain_just_gives_two_actions() {
    let mut g = new_state(&[id::UNIVERSITY], 2);
    set_hand(&mut g, 0, &[id::UNIVERSITY, id::ESTATE]);
    play(&mut g, id::UNIVERSITY);
    // University itself costs $2P and is the only Action: nothing to gain ($5 actions without a
    // Potion in the cost): the pile of University is an Action costing $2P, so excluded.
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn university_may_gain_an_action_up_to_five_without_a_potion() {
    let k = [id::UNIVERSITY, id::VILLAGE, id::SMITHY, id::WITCH, id::FAMILIAR, id::GOLEM, id::GRAND_MARKET, id::LABORATORY];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::UNIVERSITY, id::ESTATE]);
    play(&mut g, id::UNIVERSITY);
    let d = expect_decision(&mut g);
    assert_eq!(
        d.kind,
        DecisionKind::Gain { max_cost: 5, filter: Filter::Action, dest: Dest::Discard, exact: false, potion: false, optional: true }
    );
    let mut o = offered(&g);
    o.sort_unstable();
    // Village, Smithy, Witch, Laboratory; not Familiar/Golem/University (Potion), not Grand Market ($6).
    assert_eq!(o, vec![id::VILLAGE, id::SMITHY, id::LABORATORY, id::WITCH]);
    assert!(choices(&g).contains(&Choice::Pass), "it is a may");
    assert_eq!(g.turn.actions, 2);
    choose(&mut g, Choice::Card(id::WITCH));
    assert_eq!(g.players[0].discard, counts_of(&[id::WITCH]));
    assert_eq!(g.supply.get(id::WITCH), 9);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn university_gain_can_be_declined() {
    let mut g = new_state(&[id::UNIVERSITY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::UNIVERSITY, id::ESTATE]);
    play(&mut g, id::UNIVERSITY);
    expect_decision(&mut g);
    choose(&mut g, Choice::Pass);
    assert!(g.players[0].discard.is_empty());
    assert_eq!(g.supply.get(id::VILLAGE), 10);
    assert_eq!(g.turn.actions, 2);
}

// ===========================================================================
// Familiar — C++ TestFamiliar (601)
// ===========================================================================

#[test]
fn familiar_basics_and_draw() {
    let d = cards::def(id::FAMILIAR);
    assert_eq!((d.cost, d.potion, d.vp), (3, true, 0));
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::FAMILIAR, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::FAMILIAR);
    assert_eq!((g.turn.actions, g.turn.buys), (1, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(top_down(&g, 0), vec![id::PROVINCE]);
}

#[test]
fn familiar_gives_every_other_player_a_curse() {
    let mut g = new_state(&[id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::FAMILIAR, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    set_hand(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::FAMILIAR);
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
    assert_eq!(g.supply.get(id::CURSE), 9);

    let mut g = new_state(&[id::FAMILIAR], 4);
    set_hand(&mut g, 0, &[id::FAMILIAR]);
    play(&mut g, id::FAMILIAR);
    for p in 1..4 {
        assert_eq!(g.players[p].discard, counts_of(&[id::CURSE]));
    }
    assert_eq!(g.supply.get(id::CURSE), 30 - 3);
}

#[test]
fn familiar_with_too_few_curses_goes_leftmost_first() {
    let mut g = new_state(&[id::FAMILIAR], 4);
    set_supply(&mut g, id::CURSE, 2);
    set_hand(&mut g, 0, &[id::FAMILIAR]);
    play(&mut g, id::FAMILIAR);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    assert_eq!(g.players[2].discard.get(id::CURSE), 1);
    assert_eq!(g.players[3].discard.get(id::CURSE), 0);
    assert_eq!(g.supply.get(id::CURSE), 0);
    // And with none left at all, nothing happens.
    let mut g = new_state(&[id::FAMILIAR], 4);
    empty_pile(&mut g, id::CURSE);
    set_hand(&mut g, 0, &[id::FAMILIAR]);
    play(&mut g, id::FAMILIAR);
    assert!((1..4).all(|p| g.players[p].discard.is_empty()));
}

#[test]
fn familiar_is_blocked_by_moat() {
    let mut g = new_state(&[id::FAMILIAR, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::FAMILIAR]);
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::FAMILIAR);
    assert!(g.players[1].discard.is_empty());
}

// ===========================================================================
// Apprentice — C++ TestApprentice (688)
// ===========================================================================

#[test]
fn apprentice_basics_and_nothing_to_trash() {
    let d = cards::def(id::APPRENTICE);
    assert_eq!((d.cost, d.potion, d.vp), (5, false, 0));
    let mut g = new_state(&[id::APPRENTICE], 2);
    set_hand(&mut g, 0, &[id::APPRENTICE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::APPRENTICE);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    assert!(g.players[0].hand.is_empty());
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

/// C++ `TestApprentice(trashed, discount, drawn)`: the hand is Apprentice, `trashed` and a
/// decoy (Gardens); this turn's costs are reduced by `discount`; Apprentice trashes `trashed`
/// and draws `drawn` Duchies.
fn apprentice(trashed: CardId, discount: u8, drawn: u32) {
    let mut g = new_state(&[id::APPRENTICE, id::VINEYARD], 2);
    set_hand(&mut g, 0, &[id::APPRENTICE, trashed, id::GARDENS]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 10]);
    g.turn.cost_reduction = discount;
    play(&mut g, id::APPRENTICE);
    let d = g.pending_decision().expect("what to trash");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, min: 1, max: 1, .. }));
    choose(&mut g, Choice::Card(trashed));
    let mut expect = vec![id::GARDENS];
    expect.extend(std::iter::repeat(id::DUCHY).take(drawn as usize));
    assert_eq!(g.players[0].hand, counts_of(&expect), "{} with discount {discount}", cards::name(trashed));
    assert_eq!(g.players[0].deck_known.len as u32, 10 - drawn);
    assert!(g.trash.has(trashed));
}

#[test]
fn apprentice_draws_per_coin_of_cost_and_two_more_for_a_potion() {
    apprentice(id::ESTATE, 0, 2);
    apprentice(id::ESTATE, 1, 1);
    apprentice(id::ESTATE, 2, 0);
    apprentice(id::ESTATE, 3, 0);
    apprentice(id::ESTATE, 6, 0);

    apprentice(id::CURSE, 0, 0);
    apprentice(id::COPPER, 0, 0);
    apprentice(id::PROVINCE, 0, 8);
    apprentice(id::POTION, 0, 4);

    // Vineyard ($0P): 0 coins + 2 for the Potion, whatever the discount.
    apprentice(id::VINEYARD, 0, 2);
    apprentice(id::VINEYARD, 1, 2);
    apprentice(id::VINEYARD, 2, 2);

    // Apothecary ($2P): the discount lowers only the coin part.
    apprentice(id::APOTHECARY, 0, 4);
    apprentice(id::APOTHECARY, 1, 3);
    apprentice(id::APOTHECARY, 2, 2);
    apprentice(id::APOTHECARY, 3, 2);
}

#[test]
fn apprentice_draws_what_it_can() {
    // Province: 8 cards, but only 3 in the deck and 1 in the discard.
    let mut g = new_state(&[id::APPRENTICE], 2);
    set_hand(&mut g, 0, &[id::APPRENTICE, id::PROVINCE]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 3]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::APPRENTICE);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::DUCHY, id::DUCHY, id::ESTATE]));
    assert!(g.trash.has(id::PROVINCE));
}

#[test]
fn apprentice_must_trash() {
    let mut g = new_state(&[id::APPRENTICE], 2);
    set_hand(&mut g, 0, &[id::APPRENTICE, id::ESTATE, id::COPPER]);
    play(&mut g, id::APPRENTICE);
    assert!(!choices(&g).contains(&Choice::Pass));
}

// ===========================================================================
// Philosopher's Stone — C++ TestPhilosphersStone (830)
// ===========================================================================

/// C++ `VerifyStone`: hand = 10 Estates + the Stone, 10 Golds and 10 Villages already in play;
/// the Stone is worth $1 per 5 cards in deck + discard (known top, unknown and known bottom).
fn stone(coins: u16, deck: &[CardId], discard: &[CardId], known: &[CardId], bottom: &[CardId]) {
    let mut g = new_state(&[id::PHILOSOPHERS_STONE], 2);
    let mut hand = vec![id::ESTATE; 10];
    hand.push(id::PHILOSOPHERS_STONE);
    set_hand(&mut g, 0, &hand);
    set_deck_unknown(&mut g, 0, deck);
    set_discard(&mut g, 0, discard);
    set_deck_known(&mut g, 0, known);
    for &c in bottom {
        g.players[0].deck_known_bottom.push_top(c);
    }
    let mut in_play = vec![id::GOLD; 10];
    in_play.extend([id::VILLAGE; 10]);
    set_in_play(&mut g, 0, &in_play);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, coins, "deck {} + discard {} + known {} + bottom {}", deck.len(), discard.len(), known.len(), bottom.len());
    assert!(g.players[0].in_play.has(id::PHILOSOPHERS_STONE));
    assert_eq!(g.players[0].hand.get(id::ESTATE), 10);
}

#[test]
fn philosophers_stone_counts_deck_and_discard_in_fives() {
    let d = cards::def(id::PHILOSOPHERS_STONE);
    assert_eq!((d.cost, d.potion, d.vp), (3, true, 0));
    assert!(cards::is_choice_free(id::PHILOSOPHERS_STONE));
    let e = id::ESTATE;
    let v = id::VILLAGE;
    stone(0, &[], &[], &[], &[]); // no cards
    stone(0, &[e], &[e, e, e], &[], &[]); // 4 cards
    stone(0, &[], &[e, e, e, e], &[], &[]);
    stone(0, &[e], &[e], &[e], &[e]);
    stone(1, &[e; 5], &[], &[], &[]); // 5 cards
    stone(1, &[e; 4], &[e], &[], &[]);
    stone(1, &[e; 4], &[e; 2], &[], &[]); // 6
    stone(2, &[e; 6], &[e; 6], &[], &[]); // 12
    stone(3, &[e; 10], &[e; 3], &[v, v], &[]); // 15
    stone(3, &[e; 10], &[e; 3], &[v, v, v, v], &[]); // 17
    stone(4, &[e; 19], &[e], &[], &[]); // 20
}

#[test]
fn philosophers_stone_counts_each_copy_and_every_play() {
    // Two Stones, 10 cards in deck + discard: $2 each.
    let mut g = new_state(&[id::PHILOSOPHERS_STONE], 2);
    set_hand(&mut g, 0, &[id::PHILOSOPHERS_STONE, id::PHILOSOPHERS_STONE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 6]);
    set_discard(&mut g, 0, &[id::ESTATE; 4]);
    expect_decision(&mut g);
    assert_eq!(g.turn.coins, 4);

    // Tiara plays a Treasure from hand twice, resolving the Stone twice.
    let mut g = new_state(&[id::PHILOSOPHERS_STONE, id::TIARA], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::PHILOSOPHERS_STONE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::TIARA);
    choose(&mut g, Choice::Card(id::PHILOSOPHERS_STONE));
    assert_eq!(g.turn.coins, 4, "$2 twice");
}

// ===========================================================================
// Golem — C++ TestGolem (870)
// ===========================================================================

fn golem_state(kingdom: &[CardId], hand: &[CardId], deck_top_first: &[CardId], discard: &[CardId]) -> GameState {
    let mut g = new_state(kingdom, 2);
    set_hand(&mut g, 0, hand);
    set_deck_known(&mut g, 0, deck_top_first);
    set_discard(&mut g, 0, discard);
    g
}

#[test]
fn golem_basics() {
    let d = cards::def(id::GOLEM);
    assert_eq!((d.cost, d.potion, d.vp), (4, true, 0));
}

#[test]
fn golem_with_no_other_actions_discards_everything_it_reveals() {
    // C++ "No other actions": deck {Duchy, Copper, Hovel} (Estate here) and a Province in the
    // discard: all four are revealed (the discard is reshuffled in) and discarded.
    let mut g = golem_state(&[id::GOLEM], &[id::GOLEM, id::ESTATE], &[id::DUCHY, id::COPPER, id::ESTATE], &[id::PROVINCE]);
    play(&mut g, id::GOLEM);
    assert_eq!(g.turn.actions, 0);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::PROVINCE, id::DUCHY, id::COPPER, id::ESTATE]));
    assert_eq!(g.players[0].deck_size(), 0);
}

#[test]
fn golem_with_one_action_plays_it() {
    // C++ "1 action, play the Necropolis" (+2 Actions): Festival here.
    let mut g = golem_state(
        &[id::GOLEM, id::FESTIVAL],
        &[id::GOLEM, id::ESTATE],
        &[id::DUCHY, id::COPPER, id::FESTIVAL, id::ESTATE],
        &[id::PROVINCE],
    );
    play(&mut g, id::GOLEM);
    // 1 (start) - 1 (Golem) + 2 (Festival); the Festival's +1 Buy and +$2 too.
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 2));
    assert!(g.players[0].in_play.has(id::FESTIVAL) && g.players[0].in_play.has(id::GOLEM));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::PROVINCE, id::DUCHY, id::COPPER, id::ESTATE]));
    assert_eq!(g.players[0].deck_size(), 0);
}

#[test]
fn golem_plays_the_two_actions_in_the_order_chosen() {
    // C++ "2 actions, play the Village (draws Gold) and then Woodcutter" (Festival here).
    let mut g = golem_state(
        &[id::GOLEM, id::VILLAGE, id::FESTIVAL],
        &[id::GOLEM, id::ESTATE],
        &[id::DUCHY, id::COPPER, id::VILLAGE, id::ESTATE, id::FESTIVAL, id::GOLD, id::PLATINUM],
        &[id::PROVINCE],
    );
    play(&mut g, id::GOLEM);
    let d = g.pending_decision().expect("which Action to play first");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, from: Zone::Revealed, min: 1, max: 1, .. }));
    let mut o = offered(&g);
    o.sort_unstable();
    assert_eq!(o, vec![id::VILLAGE, id::FESTIVAL]);
    // While the Village resolves, the Festival is held, not in play.
    choose(&mut g, Choice::Card(id::VILLAGE));
    // The Village drew the Gold; with no Action left in hand it is played in the Buy phase.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    // 0 + 2 (Village) + 2 (Festival) Actions, 1 + 1 Buys, $2 (Festival) + $3 (Gold).
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (4, 2, 5));
    assert!(g.players[0].in_play.has(id::GOLD));
    assert!(g.players[0].in_play.has(id::VILLAGE) && g.players[0].in_play.has(id::FESTIVAL));
    assert!(g.players[0].held.is_empty());
    assert_eq!(top_down(&g, 0), vec![id::PLATINUM]);
    assert_eq!(g.players[0].discard, counts_of(&[id::PROVINCE, id::DUCHY, id::COPPER, id::ESTATE]));
}

#[test]
fn golem_second_action_is_held_while_the_first_resolves() {
    // C++ "2 actions, play ThroneRoom, choose Necropolis in hand, then play Woodcutter", and the
    // cheat "cannot choose an action that is waiting to be played by Golem for ThroneRoom".
    // Throne Room first: it may only pick the Festival from the HAND, not Smithy (held by Golem).
    let mut g = golem_state(
        &[id::GOLEM, id::THRONE_ROOM, id::SMITHY, id::FESTIVAL],
        &[id::GOLEM, id::FESTIVAL, id::ESTATE],
        &[id::DUCHY, id::COPPER, id::SMITHY, id::ESTATE, id::THRONE_ROOM, id::GOLD, id::PLATINUM, id::SILVER, id::ESTATE],
        &[id::PROVINCE],
    );
    let before = total_cards(&g);
    play(&mut g, id::GOLEM);
    choose(&mut g, Choice::Card(id::THRONE_ROOM));
    assert_eq!(total_cards(&g), before, "the held Smithy is still counted");
    assert_eq!(g.players[0].held.total(), 1);
    let d = g.pending_decision().expect("Throne Room's target");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, from: Zone::Hand, .. }));
    assert_eq!(offered(&g), vec![id::FESTIVAL], "Smithy is not in the hand");
    choose(&mut g, Choice::Card(id::FESTIVAL));
    // Festival twice: +4 Actions, +2 Buys, +$4; then the Smithy draws Gold, Platinum, Silver,
    // which (no Action left in hand) are played in the Buy phase: +$10.
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (4, 3, 4 + 10));
    assert!(g.players[0].in_play.has(id::SMITHY) && g.players[0].held.is_empty());
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].in_play.has(id::PLATINUM));
}

#[test]
fn golem_played_durations_stay_in_play() {
    // C++ "Play durations" / "Durations are moved after a full turn".
    let mut g = golem_state(
        &[id::GOLEM, id::MERCHANT_SHIP, id::FISHING_VILLAGE],
        &[id::GOLEM, id::ESTATE],
        &[id::MERCHANT_SHIP, id::FISHING_VILLAGE],
        &[],
    );
    play(&mut g, id::GOLEM);
    let d = g.pending_decision().expect("order");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, .. }));
    choose(&mut g, Choice::Card(id::MERCHANT_SHIP));
    // Golem used the action; Fishing Village gives +2 Actions; Merchant Ship +$2 and Fishing Village +$1.
    assert_eq!((g.turn.actions, g.turn.coins), (2, 3));
    assert_eq!(g.players[0].pending_durations_len, 2);
    assert!(g.players[0].in_play.has(id::MERCHANT_SHIP) && g.players[0].in_play.has(id::FISHING_VILLAGE));
    // A full turn: the Durations stay in play for next turn; Golem is discarded.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy, "only an Estate in hand");
    pass(&mut g);
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].in_play.has(id::MERCHANT_SHIP) && g.players[0].in_play.has(id::FISHING_VILLAGE));
    assert!(!g.players[0].in_play.has(id::GOLEM));
    assert!(g.players[0].all_cards().has(id::GOLEM), "discarded (and reshuffled into the next hand)");
}

#[test]
fn golem_never_plays_another_golem() {
    // Golem skips Golem cards: they're discarded with the other non-matching cards.
    let mut g = golem_state(
        &[id::GOLEM, id::VILLAGE, id::FESTIVAL],
        &[id::GOLEM],
        &[id::GOLEM, id::VILLAGE, id::GOLEM, id::FESTIVAL, id::COPPER],
        &[],
    );
    play(&mut g, id::GOLEM);
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLEM, id::GOLEM]));
    // The Village drew the Copper.
    assert_eq!(g.players[0].deck_size(), 0);
    assert!(g.players[0].in_play.has(id::VILLAGE) && g.players[0].in_play.has(id::FESTIVAL));
}

#[test]
fn golem_with_fewer_than_two_actions_in_the_whole_deck_plays_what_it_found() {
    let mut g = golem_state(&[id::GOLEM, id::FESTIVAL], &[id::GOLEM], &[id::COPPER, id::FESTIVAL, id::ESTATE], &[]);
    play(&mut g, id::GOLEM);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::ESTATE]));
    assert!(g.players[0].in_play.has(id::FESTIVAL));
}

#[test]
fn golem_two_identical_actions_need_no_choice() {
    let mut g = golem_state(&[id::GOLEM, id::FESTIVAL], &[id::GOLEM], &[id::FESTIVAL, id::COPPER, id::FESTIVAL], &[]);
    play(&mut g, id::GOLEM);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (4, 3, 4));
    assert_eq!(g.players[0].in_play.get(id::FESTIVAL), 2);
}

#[test]
fn throne_room_on_golem_resolves_it_twice() {
    let mut g = golem_state(
        &[id::GOLEM, id::THRONE_ROOM, id::FESTIVAL],
        &[id::THRONE_ROOM, id::GOLEM],
        &[id::FESTIVAL, id::COPPER, id::FESTIVAL, id::ESTATE, id::FESTIVAL, id::FESTIVAL, id::SILVER],
        &[],
    );
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::GOLEM));
    // Each Golem finds two Festivals (identical: no order choice).
    assert_eq!(g.players[0].in_play.get(id::FESTIVAL), 4);
    assert_eq!(g.turn.coins, 8);
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::ESTATE]));
}

// ===========================================================================
// Alchemist and Herbalist — C++ TestAlchemist (1007), TestHerbalist (1039). Their "when you
// discard this from play" offers are made at the end of the Buy phase, like Treasury's.
// ===========================================================================

#[test]
fn alchemist_basics_and_play() {
    let d = cards::def(id::ALCHEMIST);
    assert_eq!((d.cost, d.potion, d.vp), (3, true, 0));
    let mut g = new_state(&[id::ALCHEMIST], 2);
    set_hand(&mut g, 0, &[id::ALCHEMIST]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::ESTATE]);
    play(&mut g, id::ALCHEMIST);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE]), "+2 Cards");
}

#[test]
fn alchemist_goes_back_on_the_deck_with_a_potion_in_play() {
    // C++ "Place on deck": {Alchemist, Potion}, deck of 5 Duchies.
    let mut g = new_state(&[id::ALCHEMIST], 2);
    set_hand(&mut g, 0, &[id::ALCHEMIST, id::POTION]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::ALCHEMIST);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.potions, 1);
    pass(&mut g);
    let d = g.pending_decision().expect("Alchemist's offer");
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    assert_eq!(d.subject, id::ALCHEMIST);
    g.apply(Choice::Yes, &mut NoEvents).unwrap();
    assert_eq!(top_down(&g, 0).last().copied(), Some(id::DUCHY));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ALCHEMIST), "on top of the deck");
    assert!(!g.players[0].in_play.has(id::ALCHEMIST));
    assert!(g.players[0].discard.has(id::POTION) || g.players[0].in_play.has(id::POTION));
}

#[test]
fn alchemist_offer_can_be_declined() {
    let mut g = new_state(&[id::ALCHEMIST], 2);
    set_hand(&mut g, 0, &[id::ALCHEMIST, id::POTION]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 10]);
    play(&mut g, id::ALCHEMIST);
    expect_decision(&mut g);
    pass(&mut g);
    choose(&mut g, Choice::No);
    assert!(g.players[0].discard.has(id::ALCHEMIST));
}

#[test]
fn alchemist_cannot_go_back_without_a_potion_in_play() {
    // C++ "Cannot place on deck without Potion in play".
    let mut g = new_state(&[id::ALCHEMIST], 2);
    set_hand(&mut g, 0, &[id::ALCHEMIST]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 10]);
    play(&mut g, id::ALCHEMIST);
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(g.turn.player, 1, "no offer at all");
    assert!(g.players[0].discard.has(id::ALCHEMIST));
}

#[test]
fn herbalist_basics_and_nothing_to_put_back() {
    let d = cards::def(id::HERBALIST);
    assert_eq!((d.cost, d.potion, d.vp), (2, false, 0));
    // +1 Buy +$1; no Treasure in play: no offer.
    let mut g = new_state(&[id::HERBALIST], 2);
    set_hand(&mut g, 0, &[id::HERBALIST]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::HERBALIST);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 2, 1));
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].discard.has(id::HERBALIST));
}

#[test]
fn herbalist_puts_a_treasure_from_play_on_the_deck() {
    let mut g = new_state(&[id::HERBALIST], 2);
    set_hand(&mut g, 0, &[id::HERBALIST, id::GOLD, id::SILVER, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::HERBALIST);
    expect_decision(&mut g);
    pass(&mut g);
    let d = g.pending_decision().expect("Herbalist's offer");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, from: Zone::InPlay, min: 0, max: 1, .. }));
    assert_eq!(d.source, Some(id::HERBALIST));
    let mut o = offered(&g);
    o.sort_unstable();
    assert_eq!(o, vec![id::SILVER, id::GOLD], "each distinct Treasure once");
    assert!(choices(&g).contains(&Choice::Pass));
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(!g.players[0].in_play.has(id::GOLD));
}

#[test]
fn two_herbalists_but_only_one_treasure() {
    let mut g = new_state(&[id::HERBALIST, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::HERBALIST, id::HERBALIST, id::GOLD]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::HERBALIST);
    play(&mut g, id::HERBALIST);
    expect_decision(&mut g);
    pass(&mut g);
    // The first offer takes the Gold; the second finds no Treasure and is skipped.
    let d = g.pending_decision().expect("first offer");
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, .. }));
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(g.pending_decision().is_none());
    adv(&mut g);
    assert_eq!(g.turn.player, 1, "no second offer");
}

#[test]
fn throne_room_on_herbalist_still_puts_back_one_treasure() {
    // One physical Herbalist is discarded once: one offer, even though it was played twice.
    let mut g = new_state(&[id::THRONE_ROOM, id::HERBALIST], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HERBALIST, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HERBALIST));
    assert_eq!(g.turn.buys, 3);
    expect_decision(&mut g);
    assert_eq!(g.turn.coins, 2 + 6, "Herbalist twice, then both Golds in the Buy phase");
    pass(&mut g);
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(g.pending_decision().is_none());
    adv(&mut g);
    assert_eq!(g.turn.player, 1);
}

#[test]
fn alchemist_resolves_before_herbalist_so_both_it_and_the_potion_go_back() {
    // C++ "Resolve in order to have both Alchemist and Potion on deck": the Alchemist's offer is
    // made first (the Potion is still in play), then Herbalist puts the Potion on top of it.
    let mut g = new_state(&[id::ALCHEMIST, id::HERBALIST], 2);
    set_hand(&mut g, 0, &[id::ALCHEMIST, id::HERBALIST, id::POTION]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::ALCHEMIST);
    play(&mut g, id::HERBALIST);
    expect_decision(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins, g.turn.potions), (2, 1, 1));
    pass(&mut g);
    let d = g.pending_decision().expect("first the Alchemist's offer");
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    choose(&mut g, Choice::Yes);
    let d = g.pending_decision().expect("then Herbalist's");
    assert_eq!(d.source, Some(id::HERBALIST));
    assert_eq!(offered(&g), vec![id::POTION]);
    g.apply(Choice::Card(id::POTION), &mut NoEvents).unwrap();
    let top: Vec<CardId> = top_down(&g, 0).into_iter().take(2).collect();
    assert_eq!(top, vec![id::POTION, id::ALCHEMIST]);
}

#[test]
fn herbalist_offers_the_treasure_under_charlatan_curse_too() {
    let mut g = new_state(&[id::HERBALIST, id::CHARLATAN], 2);
    set_hand(&mut g, 0, &[id::HERBALIST, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::HERBALIST);
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(offered(&g), vec![id::CURSE], "Curse is a Treasure while Charlatan is in the game");
}

// ===========================================================================
// Possession is not implemented (by decision); the set still reaches every API.
// ===========================================================================

#[test]
fn alchemy_set_plumbing() {
    assert_eq!(CardSet::ALL.len(), 5);
    assert_eq!(cards::set_by_name("alchemy"), Some(CardSet::Alchemy));
    assert!(cards::by_name("Possession").is_none());
    assert_eq!(cards::kingdom_cards_in(CardSet::Alchemy).count(), 11);
    // Random kingdoms from Alchemy get the Potion pile; others never do.
    let mut rng = dominion_engine::rng::Rng::new(5);
    let k = cards::random_kingdom_with_colonies(&[CardSet::Alchemy], &[], &mut rng);
    assert!(k.contains(&id::POTION) && k.iter().filter(|&&c| cards::is_kingdom(c)).count() == 10);
    let mut rng = dominion_engine::rng::Rng::new(5);
    let k = cards::random_kingdom_with_colonies(&[CardSet::Base], &[], &mut rng);
    assert!(!k.contains(&id::POTION));
    // A mixed kingdom with a single Potion-cost card gets it too.
    let mut rng = dominion_engine::rng::Rng::new(5);
    let k = cards::random_kingdom_with_colonies(&[CardSet::Base], &[id::FAMILIAR], &mut rng);
    assert!(k.contains(&id::POTION));
    assert_eq!(k.iter().filter(|&&c| cards::is_kingdom(c)).count(), 10);
}
