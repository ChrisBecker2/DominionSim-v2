//! One or more rules-correctness tests per kingdom card, plus the specific edge cases
//! called out in the project brief (Moat interactions, canonical discard ordering,
//! Throne Room composition, reshuffles mid-effect, etc).

mod common;
use common::*;
use dominion_engine::cards::{self, id};
use dominion_engine::*;

// ---------------------------------------------------------------------------
// Cellar
// ---------------------------------------------------------------------------

#[test]
fn cellar_discards_chosen_cards_and_draws_that_many() {
    let mut g = new_state(&[id::CELLAR], 2);
    // Non-treasure cards throughout: the discarded cards become the reshuffle pool for the
    // 3rd draw below, so if any of them were a treasure it could get sampled back and then
    // auto-played away the instant the turn rolls into Buy, muddying the assertions.
    set_hand(&mut g, 0, &[id::CELLAR, id::CURSE, id::CURSE, id::ESTATE]);
    // 3 known (non-treasure) cards so this test doesn't also need to reason about a reshuffle.
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY]);

    let step = play(&mut g, id::CELLAR);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Select { from: Zone::Hand, act: Act::Discard, .. }, .. })));
    assert!(choices(&g).contains(&Choice::Pass), "cellar discard is optional");

    // Canonical (non-decreasing card-id) ordering: Estate (id 3) must be picked before
    // Curse (id 6), or Estate would no longer be a legal pick afterwards.
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::CURSE);

    assert_eq!(g.players[0].discard.total(), 3);
    assert_eq!(g.players[0].hand.total(), 3, "drew as many as were discarded");
    assert_eq!(g.players[0].hand.get(id::DUCHY), 3);
}

#[test]
fn cellar_draws_after_reshuffle() {
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::CURSE, id::CURSE, id::ESTATE]);
    // Only 2 known (non-treasure) cards; the 3rd draw must reshuffle the discard.
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    set_discard(&mut g, 0, &[]);

    let before = total_cards(&g);
    play(&mut g, id::CELLAR);
    // Canonical (non-decreasing card-id) ordering: Estate (id 3) before Curse (id 6).
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::CURSE);

    let ps = &g.players[0];
    assert_eq!(ps.hand.total(), 3, "drew 3 despite only 2 known cards on top");
    assert_eq!(ps.hand.get(id::DUCHY), 2);
    assert!(ps.discard.is_empty(), "the reshuffled discard should be empty afterwards");
    assert_eq!(ps.deck_unknown.total(), 2, "2 of the 3 discarded cards remain in the shuffled deck");
    assert_eq!(total_cards(&g), before, "card conservation");
}

// ---------------------------------------------------------------------------
// Chapel
// ---------------------------------------------------------------------------

#[test]
fn chapel_trashes_up_to_four_never_more() {
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);

    play(&mut g, id::CHAPEL);
    for _ in 0..4 {
        choose(&mut g, Choice::Card(id::COPPER));
    }
    // The 4-trash cap is hit; the frame must finish on its own (no 5th decision offered
    // for the Chapel itself — whatever comes next is the ordinary PlayAction/Buy decision).
    let next = adv(&mut g);
    assert_eq!(g.trash.get(id::COPPER), 4);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn chapel_may_trash_nothing() {
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    assert!(choices(&g).contains(&Choice::Pass));
    pass(&mut g);
    assert!(g.trash.is_empty());
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

// ---------------------------------------------------------------------------
// Moat (reaction) and the attacks it blocks
// ---------------------------------------------------------------------------

#[test]
fn moat_blocks_militia() {
    let mut g = new_state(&[id::MOAT, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    let victim_hand = [id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::MOAT];
    set_hand(&mut g, 1, &victim_hand);

    let next = play(&mut g, id::MILITIA);
    assert_eq!(g.players[1].hand, counts_of(&victim_hand), "Moat fully blocks Militia's discard");
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn moat_blocks_witch() {
    let mut g = new_state(&[id::MOAT, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::WITCH);
    assert!(!g.players[1].discard.has(id::CURSE), "Moat blocks the Curse");
    assert_eq!(g.players[0].hand.total(), 2, "attacker still draws +2 Cards");
}

#[test]
fn moat_blocks_bureaucrat() {
    let mut g = new_state(&[id::MOAT, id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE]);
    play(&mut g, id::BUREAUCRAT);
    assert_eq!(g.players[1].hand, counts_of(&[id::MOAT, id::ESTATE]), "victim untouched");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER), "attacker still gains Silver");
}

#[test]
fn moat_blocks_bandit() {
    let mut g = new_state(&[id::MOAT, id::BANDIT], 2);
    set_hand(&mut g, 0, &[id::BANDIT]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::SILVER]);
    play(&mut g, id::BANDIT);
    assert_eq!(g.players[1].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::GOLD, id::SILVER], "deck untouched");
    assert_eq!(g.players[0].discard.get(id::GOLD), 1, "attacker still gains Gold");
}

// ---------------------------------------------------------------------------
// Harbinger
// ---------------------------------------------------------------------------

#[test]
fn harbinger_topdecks_from_discard() {
    let mut g = new_state(&[id::HARBINGER], 2);
    set_hand(&mut g, 0, &[id::HARBINGER]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    set_discard(&mut g, 0, &[id::SILVER, id::ESTATE]);

    play(&mut g, id::HARBINGER);
    assert!(g.players[0].hand.has(id::COPPER), "+1 Card happened first");
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SILVER)) && cs.contains(&Choice::Card(id::ESTATE)) && cs.contains(&Choice::Pass));
    choose(&mut g, Choice::Card(id::SILVER));

    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
}

#[test]
fn harbinger_with_empty_discard_offers_no_decision() {
    let mut g = new_state(&[id::HARBINGER], 2);
    set_hand(&mut g, 0, &[id::HARBINGER]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    set_discard(&mut g, 0, &[]);
    let next = play(&mut g, id::HARBINGER);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

// ---------------------------------------------------------------------------
// Merchant
// ---------------------------------------------------------------------------

#[test]
fn merchant_bonus_only_on_first_silver() {
    let mut g = new_state(&[id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::MERCHANT, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    // No actions remain in hand after Merchant resolves, so `play` already auto-advances
    // through the Buy-phase treasure auto-play; there's no separate Pass to apply.
    let step = play(&mut g, id::MERCHANT);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 3, "Silver(2) + Merchant(1)");
}

#[test]
fn two_merchants_one_silver_gives_plus_two() {
    let mut g = new_state(&[id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::MERCHANT, id::MERCHANT, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::MERCHANT);
    let step = play(&mut g, id::MERCHANT);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 4, "Silver(2) + 2x Merchant(1)");
}

#[test]
fn merchant_no_silver_no_bonus() {
    let mut g = new_state(&[id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::MERCHANT, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    let step = play(&mut g, id::MERCHANT);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 1, "just the Copper, no Silver was ever played");
}

#[test]
fn throne_room_merchant_gives_plus_two() {
    let mut g = new_state(&[id::THRONE_ROOM, id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MERCHANT, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MERCHANT));
    let step = adv(&mut g);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 4, "Silver(2) + 2x Merchant bonus from the doubled play");
}

// ---------------------------------------------------------------------------
// Vassal
// ---------------------------------------------------------------------------

#[test]
fn vassal_plays_the_discarded_action_without_spending_an_action() {
    let mut g = new_state(&[id::VASSAL, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VASSAL]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::ESTATE]);

    play(&mut g, id::VASSAL);
    assert_eq!(g.turn.coins, 2, "Vassal's +$2");
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    assert_eq!(d.subject, id::VILLAGE);

    choose(&mut g, Choice::Yes);
    assert!(g.players[0].in_play.has(id::VILLAGE), "Village was played");
    assert!(!g.players[0].discard.has(id::VILLAGE));
    // 1 (start) - 1 (Vassal) + 2 (Village, free of the action cost since played via Vassal) = 2.
    assert_eq!(g.turn.actions, 2);
    assert!(g.players[0].hand.has(id::ESTATE), "Village's +1 Card");
}

#[test]
fn vassal_reveals_a_non_action_and_just_discards_it() {
    let mut g = new_state(&[id::VASSAL], 2);
    set_hand(&mut g, 0, &[id::VASSAL]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    let next = play(&mut g, id::VASSAL);
    assert!(g.players[0].discard.has(id::COPPER));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no play-it? decision for a non-action");
}

// ---------------------------------------------------------------------------
// Village / Workshop
// ---------------------------------------------------------------------------

#[test]
fn village_gives_a_card_and_two_actions() {
    let mut g = new_state(&[id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::VILLAGE);
    assert!(g.players[0].hand.has(id::ESTATE));
    assert_eq!(g.turn.actions, 2, "1 - 1 + 2");
}

#[test]
fn workshop_gains_a_card_costing_up_to_four() {
    let mut g = new_state(&[id::WORKSHOP, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SILVER)), "Silver costs 3 <= 4");
    assert!(cs.contains(&Choice::Card(id::MOAT)), "Moat costs 2 <= 4");
    assert!(!cs.contains(&Choice::Card(id::GOLD)), "Gold costs 6 > 4");
    assert!(!cs.contains(&Choice::Card(id::PROVINCE)), "Province costs 8 > 4");
    choose(&mut g, Choice::Card(id::MOAT));
    assert_eq!(g.players[0].discard, counts_of(&[id::MOAT]));
}

#[test]
fn workshop_when_desired_pile_empty_must_still_gain_something_else() {
    let mut g = new_state(&[id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    empty_pile(&mut g, id::ESTATE);
    play(&mut g, id::WORKSHOP);
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(id::ESTATE)));
    assert!(cs.contains(&Choice::Card(id::SILVER)), "3-cost Silver is still legal (<=4)");
    choose(&mut g, Choice::Card(id::SILVER));
    assert!(g.players[0].discard.has(id::SILVER));
}

// ---------------------------------------------------------------------------
// Bureaucrat
// ---------------------------------------------------------------------------

#[test]
fn bureaucrat_gains_silver_and_victim_topdecks_chosen_victory_card() {
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY, id::COPPER]);

    play(&mut g, id::BUREAUCRAT);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));

    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ESTATE)) && cs.contains(&Choice::Card(id::DUCHY)));
    assert!(!cs.contains(&Choice::Card(id::COPPER)), "Copper isn't a Victory card");

    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::DUCHY));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::COPPER]));
}

#[test]
fn bureaucrat_victim_with_no_victory_card_is_untouched() {
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::SILVER]);
    let next = play(&mut g, id::BUREAUCRAT);
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER, id::SILVER]));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

// ---------------------------------------------------------------------------
// Gardens
// ---------------------------------------------------------------------------

#[test]
fn gardens_scores_floor_of_total_cards_over_ten() {
    let mut g = new_state(&[id::GARDENS], 2);
    // 23 total cards including one Gardens.
    set_hand(&mut g, 0, &[id::GARDENS]);
    set_deck_unknown(&mut g, 0, &[id::COPPER; 22]);
    assert_eq!(g.players[0].all_cards().total(), 23);
    assert_eq!(g.players[0].vp(), 2, "floor(23/10) = 2");
}

// ---------------------------------------------------------------------------
// Militia
// ---------------------------------------------------------------------------

#[test]
fn militia_discards_down_to_three() {
    let mut g = new_state(&[id::MILITIA], 2);
    // Disable auto_single so the forced first pick is actually observable below instead of
    // being silently applied before we get a chance to inspect it.
    g.auto_single = false;
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    play(&mut g, id::MILITIA);
    assert_eq!(g.turn.coins, 2, "Militia's +$2");

    // Canonical ordering must force the low-id Coppers first so the 2-discard quota is
    // always reachable (picking Estate first would strand only 4 Coppers for 1 more pick).
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER)]);
    choose(&mut g, Choice::Card(id::COPPER));
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::COPPER)) && cs.contains(&Choice::Card(id::ESTATE)));
    choose(&mut g, Choice::Card(id::COPPER));

    assert_eq!(g.players[1].hand.total(), 3);
    assert_eq!(g.players[1].discard, counts_of(&[id::COPPER, id::COPPER]));
}

#[test]
fn militia_hand_of_three_or_fewer_is_untouched() {
    let mut g = new_state(&[id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER]);
    let next = play(&mut g, id::MILITIA);
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER]));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

// ---------------------------------------------------------------------------
// Moneylender
// ---------------------------------------------------------------------------

#[test]
fn moneylender_trashes_copper_for_three_coins() {
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::COPPER]);
    play(&mut g, id::MONEYLENDER);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass]);
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.trash.get(id::COPPER), 1);
    assert_eq!(g.turn.coins, 3);
}

#[test]
fn moneylender_decline_leaves_copper_and_no_bonus() {
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::COPPER]);
    play(&mut g, id::MONEYLENDER);
    pass(&mut g);
    assert!(g.trash.is_empty());
    // No +$3 bonus; the 1 coin present is just the untrashed Copper auto-played into the
    // Buy phase (it's the only card left in hand), not a Moneylender bonus.
    assert_eq!(g.turn.coins, 1);
    assert_eq!(g.players[0].all_cards().get(id::COPPER), 1);
}

#[test]
fn moneylender_no_copper_offers_no_decision() {
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::SILVER]);
    let next = play(&mut g, id::MONEYLENDER);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

// ---------------------------------------------------------------------------
// Poacher
// ---------------------------------------------------------------------------

#[test]
fn poacher_with_zero_empty_piles_has_no_discard() {
    let mut g = new_state(&[id::POACHER], 2);
    set_hand(&mut g, 0, &[id::POACHER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    let next = play(&mut g, id::POACHER);
    assert!(g.players[0].hand.has(id::DUCHY), "+1 Card");
    assert_eq!(g.turn.coins, 1);
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1");
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no discard needed");
}

#[test]
fn poacher_with_one_empty_pile_discards_one() {
    let mut g = new_state(&[id::POACHER, id::CELLAR], 2);
    empty_pile(&mut g, id::CELLAR);
    set_hand(&mut g, 0, &[id::POACHER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::POACHER);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER]));
}

#[test]
fn poacher_with_two_empty_piles_discards_two() {
    let mut g = new_state(&[id::POACHER, id::CELLAR, id::CHAPEL], 2);
    empty_pile(&mut g, id::CELLAR);
    empty_pile(&mut g, id::CHAPEL);
    set_hand(&mut g, 0, &[id::POACHER, id::COPPER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::POACHER);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::COPPER);
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::COPPER]));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::ESTATE]));
}

#[test]
fn poacher_discards_whole_hand_when_short_of_empty_piles() {
    let mut g = new_state(&[id::POACHER, id::CELLAR, id::CHAPEL, id::MOAT], 2);
    empty_pile(&mut g, id::CELLAR);
    empty_pile(&mut g, id::CHAPEL);
    empty_pile(&mut g, id::MOAT);
    set_hand(&mut g, 0, &[id::POACHER]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    // Hand after the +1 Card is just [Silver]; 3 empty piles but only 1 card to give, so the
    // single legal choice (discard the whole hand) is forced and auto-applied by `play`.
    play(&mut g, id::POACHER);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER]));
}

// ---------------------------------------------------------------------------
// Remodel
// ---------------------------------------------------------------------------

#[test]
fn remodel_gold_into_province() {
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::GOLD]);
    play(&mut g, id::REMODEL);
    // Gold is the only card in hand; the trash is forced (single legal choice, auto-applied).
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: cards::cost(id::GOLD) + 2, filter: Filter::Any, dest: Dest::Discard, exact: false });
    assert!(choices(&g).contains(&Choice::Card(id::PROVINCE)));
    choose(&mut g, Choice::Card(id::PROVINCE));
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
    assert!(g.players[0].discard.has(id::PROVINCE));
}

#[test]
fn remodel_when_target_pile_empty_other_options_remain() {
    let mut g = new_state(&[id::REMODEL, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::COPPER]);
    empty_pile(&mut g, id::ESTATE); // costs 2 = cost(Copper)+2, but is unavailable
    play(&mut g, id::REMODEL);
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(id::ESTATE)));
    assert!(cs.contains(&Choice::Card(id::CHAPEL)), "another cost-2 card is still legal");
}

// ---------------------------------------------------------------------------
// Smithy / Throne Room
// ---------------------------------------------------------------------------

#[test]
fn smithy_draws_three() {
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::CURSE]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::CURSE]));
}

#[test]
fn throne_room_doubles_smithy() {
    let mut g = new_state(&[id::THRONE_ROOM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SMITHY]);
    // Non-treasure fillers, so nothing gets auto-played away once the turn rolls into Buy.
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::CURSE, id::ESTATE, id::DUCHY, id::CURSE]);
    play(&mut g, id::THRONE_ROOM);
    let cs = choices(&g);
    assert_eq!(cs, vec![Choice::Card(id::SMITHY), Choice::Pass]);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].hand.total(), 6, "3 + 3 cards");
}

#[test]
fn throne_room_with_no_actions_in_hand_auto_resolves() {
    let mut g = new_state(&[id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM]);
    let next = play(&mut g, id::THRONE_ROOM);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "Pass was the only choice, auto-applied");
}

#[test]
fn throne_room_on_throne_room_with_a_single_target_only_doubles_once() {
    // Outer TR plays the inner TR twice. The first inner-TR resolution plays Smithy
    // twice (6 cards); by the second inner-TR resolution Smithy is already in play,
    // so there's nothing left in hand to target and it whiffs. Total: 6 cards, not 12.
    let mut g = new_state(&[id::THRONE_ROOM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::THRONE_ROOM, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);

    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::THRONE_ROOM));
    // First inner Throne Room: only Smithy is available.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Play, filter: Filter::Action, min: 0, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::SMITHY));

    let next = adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 6, "Smithy played exactly twice, not four times");
    // The second inner Throne Room had nothing to target, so it auto-passed straight
    // through to the next real decision.
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn throne_room_smithy_reshuffles_mid_draw() {
    let mut g = new_state(&[id::THRONE_ROOM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    set_discard(&mut g, 0, &[id::CURSE; 8]);

    let before = total_cards(&g);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SMITHY));
    adv(&mut g);

    let ps = &g.players[0];
    assert_eq!(ps.hand.total(), 6, "3 + 3 cards, spanning the reshuffle");
    assert!(ps.deck_known.is_empty());
    assert!(ps.discard.is_empty(), "the whole discard was shuffled in");
    assert_eq!(ps.deck_unknown.total(), 4, "8 shuffled in, 4 drawn, 4 remain");
    assert_eq!(total_cards(&g), before);
}

// ---------------------------------------------------------------------------
// Bandit
// ---------------------------------------------------------------------------

#[test]
fn bandit_silver_and_gold_is_a_choice() {
    let mut g = new_state(&[id::BANDIT], 2);
    set_hand(&mut g, 0, &[id::BANDIT]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::GOLD]);
    play(&mut g, id::BANDIT);
    assert!(g.players[0].discard.has(id::GOLD), "attacker gains a Gold");

    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Card(id::GOLD)]);
    choose(&mut g, Choice::Card(id::GOLD));

    assert_eq!(g.trash, counts_of(&[id::GOLD]));
    assert_eq!(g.players[1].discard, counts_of(&[id::SILVER]));
}

#[test]
fn bandit_copper_and_gold_only_gold_is_trashable() {
    let mut g = new_state(&[id::BANDIT], 2);
    set_hand(&mut g, 0, &[id::BANDIT]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::GOLD]);
    play(&mut g, id::BANDIT);
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
    assert_eq!(g.players[1].discard, counts_of(&[id::COPPER]));
}

#[test]
fn bandit_two_coppers_neither_trashed() {
    let mut g = new_state(&[id::BANDIT], 2);
    set_hand(&mut g, 0, &[id::BANDIT]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::COPPER]);
    let next = play(&mut g, id::BANDIT);
    assert!(g.trash.is_empty());
    assert_eq!(g.players[1].discard, counts_of(&[id::COPPER, id::COPPER]));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn bandit_one_card_left_in_victim_deck() {
    let mut g = new_state(&[id::BANDIT], 2);
    set_hand(&mut g, 0, &[id::BANDIT]);
    set_deck_known(&mut g, 1, &[id::SILVER]);
    set_discard(&mut g, 1, &[]);
    let before = total_cards(&g);
    play(&mut g, id::BANDIT);
    assert_eq!(g.trash, counts_of(&[id::SILVER]));
    assert!(g.players[1].discard.is_empty());
    assert_eq!(total_cards(&g), before);
}

// ---------------------------------------------------------------------------
// Council Room
// ---------------------------------------------------------------------------

#[test]
fn council_room_draws_four_and_each_other_player_draws_one() {
    let mut g = new_state(&[id::COUNCIL_ROOM], 3);
    set_hand(&mut g, 0, &[id::COUNCIL_ROOM]);
    // Non-treasure fillers for player 0: nothing to auto-play once the turn rolls into Buy.
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::CURSE, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::SILVER]);
    set_deck_known(&mut g, 2, &[id::GOLD]);
    play(&mut g, id::COUNCIL_ROOM);
    assert_eq!(g.players[0].hand.total(), 4);
    assert!(g.players[1].hand.has(id::SILVER));
    assert!(g.players[2].hand.has(id::GOLD));
    assert_eq!(g.turn.buys, 2, "1 + 1");
}

// ---------------------------------------------------------------------------
// Festival / Laboratory / Market
// ---------------------------------------------------------------------------

#[test]
fn festival_gives_actions_buy_and_coin() {
    let mut g = new_state(&[id::FESTIVAL], 2);
    set_hand(&mut g, 0, &[id::FESTIVAL]);
    play(&mut g, id::FESTIVAL);
    assert_eq!(g.turn.actions, 2, "1 - 1 + 2");
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn laboratory_draws_two_keeps_action() {
    let mut g = new_state(&[id::LABORATORY], 2);
    set_hand(&mut g, 0, &[id::LABORATORY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::LABORATORY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1");
}

#[test]
fn market_gives_everything() {
    let mut g = new_state(&[id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::MARKET);
    assert!(g.players[0].hand.has(id::ESTATE));
    assert_eq!(g.turn.actions, 1);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.turn.coins, 1);
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

#[test]
fn library_skips_actions_and_discards_them_at_the_end() {
    let mut g = new_state(&[id::LIBRARY, id::SMITHY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    // Non-treasure, non-action fillers so the eventual 6-Curse hand isn't auto-played
    // away once there are no more actions to play and the turn rolls into Buy.
    set_deck_known(
        &mut g,
        0,
        &[id::SMITHY, id::CURSE, id::VILLAGE, id::CURSE, id::CURSE, id::CURSE, id::CURSE, id::CURSE],
    );

    play(&mut g, id::LIBRARY);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::SetAside });
    assert_eq!(d.subject, id::SMITHY);
    choose(&mut g, Choice::Yes); // skip Smithy

    let d2 = expect_decision(&mut g);
    assert_eq!(d2.subject, id::VILLAGE);
    choose(&mut g, Choice::No); // keep Village

    let ps = &g.players[0];
    assert_eq!(ps.hand.total(), 7, "skipped Actions don't count toward the 7");
    assert!(ps.hand.has(id::VILLAGE));
    assert_eq!(ps.hand.get(id::CURSE), 6);
    assert_eq!(ps.discard, counts_of(&[id::SMITHY]), "the skipped Smithy is discarded at the end");
    assert!(ps.set_aside.is_empty());
}

#[test]
fn library_reshuffle_mid_draw_does_not_shuffle_in_the_set_aside_card() {
    let mut g = new_state(&[id::LIBRARY, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    set_deck_known(&mut g, 0, &[id::SMITHY]);
    set_discard(&mut g, 0, &[id::CURSE; 6]);

    play(&mut g, id::LIBRARY);
    let d = expect_decision(&mut g);
    assert_eq!(d.subject, id::SMITHY);
    choose(&mut g, Choice::Yes); // skip it (it must not reappear from the reshuffle)

    let ps = &g.players[0];
    assert_eq!(ps.hand.total(), 6, "only 6 Curses were ever available; Library stops early");
    assert!(ps.hand.get(id::SMITHY) == 0, "the set-aside Smithy never re-entered the draw pool");
    assert_eq!(ps.discard, counts_of(&[id::SMITHY]));
}

// ---------------------------------------------------------------------------
// Mine
// ---------------------------------------------------------------------------

#[test]
fn mine_trashes_treasure_and_gains_to_hand() {
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::COPPER]);
    play(&mut g, id::MINE);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass]);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(choices(&g).contains(&Choice::Card(id::SILVER)));
    assert!(!choices(&g).contains(&Choice::Card(id::GOLD)), "Gold costs 6 > 0+3");

    // The gained Silver is the only card left in hand, so it gets auto-played the instant
    // the turn rolls into Buy — check the Gain event's destination, not the post-Buy hand.
    let mut events: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut events);
    assert!(
        events.iter().any(|e| matches!(e, Event::Gain { card, to: Dest::Hand, .. } if *card == id::SILVER)),
        "Silver was gained to hand, not discard: {events:?}"
    );
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
}

#[test]
fn mine_may_decline() {
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::COPPER]);
    play(&mut g, id::MINE);
    pass(&mut g);
    assert!(g.trash.is_empty());
    // The Copper is never touched, though by now it may have been auto-played into
    // `in_play` as part of the automatic Buy-phase entry.
    assert_eq!(g.players[0].all_cards().get(id::COPPER), 1);
}

#[test]
fn mine_whiffs_when_no_affordable_treasure_remains() {
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::COPPER]);
    empty_pile(&mut g, id::COPPER);
    empty_pile(&mut g, id::SILVER);
    play(&mut g, id::MINE);
    let next = choose_and_advance(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.trash, counts_of(&[id::COPPER]), "the trash still happens");
    assert!(!g.players[0].hand.has(id::SILVER) && !g.players[0].hand.has(id::GOLD));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "gain whiffed silently");
}

// ---------------------------------------------------------------------------
// Sentry
// ---------------------------------------------------------------------------

#[test]
fn sentry_can_trash_one_and_discard_the_other() {
    let mut g = new_state(&[id::SENTRY], 2);
    set_hand(&mut g, 0, &[id::SENTRY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER, id::ESTATE]);

    play(&mut g, id::SENTRY);
    assert!(g.players[0].hand.has(id::GOLD), "+1 Card happened before the look-at-top-2");
    let d = expect_decision(&mut g);
    match d.kind {
        DecisionKind::Select { from: Zone::Revealed, act: Act::Trash, filter: Filter::Any, min: 0, .. } => {}
        other => panic!("expected a Sentry trash decision, got {other:?}"),
    }
    choose(&mut g, Choice::Card(id::COPPER));
    // Trashing is "any number", so Estate (still revealed) is offered again; decline it to
    // move on to the discard phase.
    pass(&mut g);

    let d2 = expect_decision(&mut g);
    match d2.kind {
        DecisionKind::Select { from: Zone::Revealed, act: Act::Discard, filter: Filter::Any, min: 0, .. } => {}
        other => panic!("expected a Sentry discard decision, got {other:?}"),
    }
    choose(&mut g, Choice::Card(id::ESTATE));

    let next = adv(&mut g);
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
    assert!(g.players[0].set_aside.is_empty());
    assert!(matches!(next, Step::Decision(_)), "nothing left to reorder onto the deck");
}

#[test]
fn sentry_can_keep_both_and_choose_the_order() {
    let mut g = new_state(&[id::SENTRY], 2);
    // Disable auto_single: with only 1 card left to place, the final topdeck pick would
    // otherwise be silently auto-applied before we get to make it explicitly below.
    g.auto_single = false;
    set_hand(&mut g, 0, &[id::SENTRY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER]);

    play(&mut g, id::SENTRY);
    pass(&mut g); // trash nothing
    pass(&mut g); // discard nothing

    let d = expect_decision(&mut g);
    match d.kind {
        DecisionKind::Select { from: Zone::Revealed, act: Act::Topdeck, filter: Filter::Any, min: 2, ordered: true, .. } => {}
        other => panic!("expected a Sentry topdeck-order decision, got {other:?}"),
    }
    // Pick Copper first (goes down), then Silver (ends on top).
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::SILVER));

    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::SILVER, id::COPPER]);
}

#[test]
fn sentry_keeping_two_identical_cards_needs_no_order_decision() {
    let mut g = new_state(&[id::SENTRY], 2);
    set_hand(&mut g, 0, &[id::SENTRY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER, id::COPPER]);

    play(&mut g, id::SENTRY);
    pass(&mut g); // trash nothing
    let next = pass(&mut g); // discard nothing -> both identical Coppers auto-topdeck
    assert!(!matches!(next, Step::Decision(Decision { kind: DecisionKind::Select { from: Zone::Revealed, act: Act::Topdeck, .. }, .. })));
    assert_eq!(g.players[0].deck_known.counts().get(id::COPPER), 2);
}

// ---------------------------------------------------------------------------
// Witch
// ---------------------------------------------------------------------------

#[test]
fn witch_draws_two_and_curses_opponents_in_turn_order() {
    let mut g = new_state(&[id::WITCH], 3);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::WITCH);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert!(g.players[1].discard.has(id::CURSE));
    assert!(g.players[2].discard.has(id::CURSE));
}

#[test]
fn witch_curses_run_out_leftmost_players_get_priority() {
    let mut g = new_state(&[id::WITCH], 4);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_supply(&mut g, id::CURSE, 2);
    play(&mut g, id::WITCH);
    assert!(g.players[1].discard.has(id::CURSE), "leftmost opponent gets one");
    assert!(g.players[2].discard.has(id::CURSE), "second opponent gets the last one");
    assert!(!g.players[3].discard.has(id::CURSE), "curses ran out");
    assert_eq!(g.supply.get(id::CURSE), 0);
}

// ---------------------------------------------------------------------------
// Artisan
// ---------------------------------------------------------------------------

#[test]
fn artisan_gains_to_hand_then_topdecks_a_card() {
    let mut g = new_state(&[id::ARTISAN], 2);
    // Non-treasure fillers so they aren't auto-played once the turn rolls into Buy.
    set_hand(&mut g, 0, &[id::ARTISAN, id::ESTATE, id::CURSE]);
    play(&mut g, id::ARTISAN);
    assert!(choices(&g).contains(&Choice::Card(id::DUCHY)));
    assert!(!choices(&g).contains(&Choice::Card(id::GOLD)), "Gold costs 6 > 5");
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(g.players[0].hand.has(id::DUCHY), "gained to hand");

    let d = expect_decision(&mut g);
    match d.kind {
        DecisionKind::Select { from: Zone::Hand, act: Act::Topdeck, filter: Filter::Any, min: 1, max: 1, ordered: false } => {}
        other => panic!("expected an Artisan topdeck decision, got {other:?}"),
    }
    choose(&mut g, Choice::Card(id::DUCHY));

    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::CURSE]));
}

#[test]
fn throne_room_merchant_bonus_counts_both_plays_but_only_the_first_silver() {
    // One physical Merchant played twice sets up two "+$1 on the first Silver" bonuses; both
    // trigger on the first Silver, none on the second: 2 Silvers ($4) + $2 = $6.
    let mut g = new_state(&[id::THRONE_ROOM, id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MERCHANT, id::SILVER, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MERCHANT));
    assert!(matches!(adv(&mut g), Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.players[0].in_play.get(id::MERCHANT), 1, "only one physical Merchant in play");
    assert_eq!(g.turn.merchants, 2, "two Merchant plays");
    assert_eq!(g.turn.coins, 6);
}

#[test]
fn workshop_with_nothing_affordable_in_the_supply_gains_nothing() {
    // Every pile costing $4 or less is empty: Workshop's gain has no legal target, so it is
    // skipped (no decision) and the turn continues to the buy phase with nothing gained.
    let mut g = new_state(&[id::WORKSHOP, id::MARKET], 2);
    for c in [id::COPPER, id::SILVER, id::ESTATE, id::CURSE, id::WORKSHOP] {
        empty_pile(&mut g, c);
    }
    set_hand(&mut g, 0, &[id::WORKSHOP, id::ESTATE]);
    // With 5 piles empty the game ends this turn; stop at the (pass-only) buy to inspect it.
    g.auto_single = false;
    let before = g.players[0].all_cards();
    let next = play(&mut g, id::WORKSHOP);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no Gain decision: {next:?}");
    assert_eq!(g.players[0].all_cards().total(), before.total(), "nothing gained");
    assert!(g.players[0].in_play.has(id::WORKSHOP));
}
