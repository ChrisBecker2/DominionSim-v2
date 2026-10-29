//! Base Set 2nd edition card tests: each card's printed effect, edge cases (empty deck,
//! reshuffles) and 2nd-edition rulings.
//!
//! Scope: only cards that exist in the Base Set 2nd edition (Woodcutter, Adventurer, Feast,
//! Chancellor, Thief and Spy are 1st edition only).
//!
//! 1E -> 2E differences (called out again at each relevant test):
//!   - Throne Room: "You may play an Action card from your hand twice" (optional in 2E; 1E made
//!     it mandatory).
//!   - Moneylender: "You may trash a Copper from your hand for +$3" (optional; 1E trashed
//!     it automatically).
//!   - Mine: "You may trash a Treasure... gain a Treasure to your HAND" (optional, and gains
//!     to hand rather than discard; 1E made the trash mandatory).
//!   - Bureaucrat: gains Silver to the top of the deck (same in both editions).
//!
//! Not covered here: Potion-costing cards (see alchemy_cards.rs) and bot behaviour (sim crate).

mod common;
use common::*;
use dominion_engine::cards::{self, id};
use dominion_engine::state;
use dominion_engine::*;

/// Build a `Counts` from `(card, count)` pairs — handy for the VP tests below, where the exact
/// zone a card sits in doesn't matter, only the multiset `vp_of_cards` scores.
fn counts_pairs(pairs: &[(CardId, u8)]) -> Counts {
    let mut c = Counts::EMPTY;
    for &(card, n) in pairs {
        c.add(card, n);
    }
    c
}
fn vp_of(pairs: &[(CardId, u8)]) -> i32 {
    state::vp_of_cards(&counts_pairs(pairs))
}

// ===========================================================================
// Copper
// ===========================================================================

#[test]
fn copper_basics() {
    let d = cards::def(id::COPPER);
    assert_eq!(d.cost, 0);
    assert!(cards::is(id::COPPER, cards::TREASURE));
    assert_eq!(d.vp, 0);
    // Variable "copper worth" (AddCopperWorth) isn't modeled by this engine (no such kingdom
    // card is implemented in Base Set 2E), so only the plain worth is tested below.
}

#[test]
fn copper_worth_one() {
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::COPPER]);
    let step = adv(&mut g);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 1);
}

// ===========================================================================
// Silver
// ===========================================================================

#[test]
fn silver_basics_and_worth() {
    let d = cards::def(id::SILVER);
    assert_eq!(d.cost, 3);
    assert!(cards::is(id::SILVER, cards::TREASURE));
    assert_eq!(d.vp, 0);

    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::SILVER]);
    adv(&mut g);
    assert_eq!(g.turn.coins, 2);
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

// ===========================================================================
// Gold
// ===========================================================================

#[test]
fn gold_basics_and_worth() {
    let d = cards::def(id::GOLD);
    assert_eq!(d.cost, 6);
    assert!(cards::is(id::GOLD, cards::TREASURE));
    assert_eq!(d.vp, 0);

    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::GOLD]);
    adv(&mut g);
    assert_eq!(g.turn.coins, 3);
}

// ===========================================================================
// Estate
// ===========================================================================

#[test]
fn estate_basics_and_vp() {
    let d = cards::def(id::ESTATE);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::ESTATE, cards::VICTORY));
    assert_eq!(d.vp, 1);
    assert_eq!(vp_of(&[(id::ESTATE, 1)]), 1);
    assert_eq!(vp_of(&[(id::ESTATE, 2)]), 2);
    assert_eq!(vp_of(&[(id::ESTATE, 3)]), 3);
    assert_eq!(vp_of(&[(id::ESTATE, 10)]), 10);
}

// ===========================================================================
// Duchy
// ===========================================================================

#[test]
fn duchy_basics_and_vp() {
    let d = cards::def(id::DUCHY);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::DUCHY, cards::VICTORY));
    assert_eq!(d.vp, 3);
    assert_eq!(vp_of(&[(id::DUCHY, 1)]), 3);
    assert_eq!(vp_of(&[(id::DUCHY, 2)]), 6);
    assert_eq!(vp_of(&[(id::DUCHY, 3)]), 9);
    assert_eq!(vp_of(&[(id::DUCHY, 10)]), 30);
}

// ===========================================================================
// Province
// ===========================================================================

#[test]
fn province_basics_and_vp() {
    let d = cards::def(id::PROVINCE);
    assert_eq!(d.cost, 8);
    assert!(cards::is(id::PROVINCE, cards::VICTORY));
    assert_eq!(d.vp, 6);
    assert_eq!(vp_of(&[(id::PROVINCE, 1)]), 6);
    assert_eq!(vp_of(&[(id::PROVINCE, 2)]), 12);
    assert_eq!(vp_of(&[(id::PROVINCE, 3)]), 18);
    assert_eq!(vp_of(&[(id::PROVINCE, 10)]), 60);
}

// ===========================================================================
// Curse
// ===========================================================================

#[test]
fn curse_basics_and_vp() {
    let d = cards::def(id::CURSE);
    assert_eq!(d.cost, 0);
    assert!(cards::is(id::CURSE, cards::CURSE_T));
    assert!(!cards::is(id::CURSE, cards::VICTORY));
    assert_eq!(d.vp, -1);
    assert_eq!(vp_of(&[(id::CURSE, 1)]), -1);
    assert_eq!(vp_of(&[(id::CURSE, 2)]), -2);
    assert_eq!(vp_of(&[(id::CURSE, 3)]), -3);
    assert_eq!(vp_of(&[(id::CURSE, 10)]), -10);
}

// ===========================================================================
// Cellar
// ===========================================================================

#[test]
fn cellar_basics() {
    let d = cards::def(id::CELLAR);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::CELLAR, cards::ACTION));
}

#[test]
fn cellar_decline_has_no_effect() {
    // "Chooses to not discard anything"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::ESTATE, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::CELLAR);
    assert!(choices(&g).contains(&Choice::Pass));
    pass(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1 (Cellar's own +1 Action)");
}

#[test]
fn cellar_empty_hand_offers_nothing() {
    // "No cards in hand, shouldn't get asked to discard something"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR]);
    let next = play(&mut g, id::CELLAR);
    assert!(g.players[0].hand.is_empty());
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn cellar_discards_one_and_draws_one() {
    // "Discard the only card in hand and draw a single card"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::ESTATE]);
    play(&mut g, id::CELLAR);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::DUCHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY]));
}

#[test]
fn cellar_discard_and_get_it_straight_back_via_reshuffle() {
    // "Discard the only card in hand and only one in the deck
    // (should get it back)"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::DUCHY]);
    play(&mut g, id::CELLAR);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::DUCHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    assert!(g.players[0].deck_counts().is_empty());
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn cellar_discard_only_three_of_four() {
    // "Discard only 3 cards even though more are in hand"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::PROVINCE, id::PROVINCE, id::GOLD]);
    play(&mut g, id::CELLAR);
    choose(&mut g, Choice::Card(id::DUCHY));
    choose(&mut g, Choice::Card(id::DUCHY));
    choose(&mut g, Choice::Card(id::DUCHY));
    pass(&mut g); // decline discarding the 4th Duchy
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE, id::PROVINCE, id::PROVINCE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::GOLD]));
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY, id::DUCHY, id::DUCHY]));
}

#[test]
fn cellar_discard_all_four() {
    // "Discard all 4 cards"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 4]);
    play(&mut g, id::CELLAR);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::DUCHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE; 4]));
    assert!(g.players[0].deck_counts().is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY; 4]));
}

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

#[test]
fn ruling_cellar_and_chapel_cannot_pick_themselves() {
    // "You cannot discard / trash the Cellar / Chapel itself, since it is not in your hand after you play it."
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::ESTATE]);
    play(&mut g, id::CELLAR);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ESTATE)) && !cs.contains(&Choice::Card(id::CELLAR)), "{cs:?}");
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ESTATE)) && !cs.contains(&Choice::Card(id::CHAPEL)), "{cs:?}");
}

// ===========================================================================
// Chapel
// ===========================================================================

#[test]
fn chapel_basics() {
    let d = cards::def(id::CHAPEL);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::CHAPEL, cards::ACTION));
}

#[test]
fn chapel_may_trash_nothing() {
    // "Should trash nothing"
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL]);
    let next = play(&mut g, id::CHAPEL);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "nothing to trash, auto-resolved");
    assert!(g.trash.is_empty());
}

#[test]
fn chapel_trash_the_only_card_in_hand() {
    // "Trash the only remaining card in hand"
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER]);
    play(&mut g, id::CHAPEL);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
}

#[test]
fn chapel_trash_one_of_several() {
    // "Trash a card in hand". Coppers are treasures, so the two
    // left untrashed get auto-played the instant Chapel finishes and the turn rolls into the
    // Buy phase (no more Actions to play) — check total ownership and coins, not raw `hand`.
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::CHAPEL);
    choose(&mut g, Choice::Card(id::COPPER));
    pass(&mut g);
    assert_eq!(g.players[0].all_cards().get(id::COPPER), 2, "the 2 untrashed Coppers are still owned");
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.turn.coins, 2, "the 2 untrashed Coppers auto-played into the Buy phase");
}

#[test]
fn chapel_trash_two() {
    // "Trash 2 cards" (see auto-play note above)
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::CHAPEL);
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::COPPER));
    pass(&mut g);
    assert_eq!(g.players[0].all_cards().get(id::COPPER), 1);
    assert_eq!(g.trash.get(id::COPPER), 2);
    assert_eq!(g.turn.coins, 1);
}

#[test]
fn chapel_trash_three() {
    // "Trash 3 cards"
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER, id::SILVER, id::GOLD]);
    play(&mut g, id::CHAPEL);
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::SILVER, id::GOLD]));
}

#[test]
fn chapel_trash_four_is_the_cap() {
    // "Trash 4 cards" and the attempt to trash a 5th:
    // this engine simply never offers a 5th trash, so the cap is exercised structurally.
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    for _ in 0..4 {
        choose(&mut g, Choice::Card(id::COPPER));
    }
    let next = adv(&mut g);
    assert_eq!(g.trash.get(id::COPPER), 4);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "the cap was hit; no 5th decision was offered");
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

// ---------------------------------------------------------------------------
// Second scenarios for cards already covered above (formerly tests/cards.rs, which had the
// same test names; renamed `_alt` on the merge).
// ---------------------------------------------------------------------------

#[test]
fn chapel_may_trash_nothing_alt() {
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    assert!(choices(&g).contains(&Choice::Pass));
    pass(&mut g);
    assert!(g.trash.is_empty());
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn ruling_chapel_trashes_all_chosen_cards_then_resolves_their_trash_effects() {
    // "You trash all the cards first, then pick an order to resolve things that happen due to trashing them."
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::OVERGROWN_ESTATE, id::OVERGROWN_ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::CURSE]);
    play(&mut g, id::CHAPEL);
    choose(&mut g, Choice::Card(id::OVERGROWN_ESTATE));
    choose(&mut g, Choice::Card(id::OVERGROWN_ESTATE));
    assert_eq!(g.trash, counts_of(&[id::OVERGROWN_ESTATE, id::OVERGROWN_ESTATE]));
    assert_eq!(g.players[0].hand.total(), 2, "each Overgrown Estate drew a card");
}

// ===========================================================================
// Moat
// ===========================================================================

#[test]
fn moat_basics() {
    let d = cards::def(id::MOAT);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::MOAT, cards::ACTION));
    assert!(cards::is(id::MOAT, cards::REACTION));
}

#[test]
fn moat_played_as_action_draws_two() {
    // "Play as action"
    let mut g = new_state(&[id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MOAT]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 5]);
    play(&mut g, id::MOAT);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE; 3]));
    assert_eq!(g.turn.actions, 0, "1 - 1 + 0");
}

#[test]
fn moat_blocks_militia() {
    // Militia blocked by Moat.
    let mut g = new_state(&[id::MOAT, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    let victim_hand = [id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::MOAT];
    set_hand(&mut g, 1, &victim_hand);
    play(&mut g, id::MILITIA);
    moat(&mut g, true);
    assert_eq!(g.players[1].hand, counts_of(&victim_hand), "fully blocked, nothing discarded");
}

#[test]
fn moat_blocks_witch() {
    // Witch blocked by Moat
    let mut g = new_state(&[id::MOAT, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WITCH);
    moat(&mut g, true);
    assert!(!g.players[1].discard.has(id::CURSE), "Moat blocks the Curse");
    assert_eq!(g.players[0].hand.total(), 2, "the attacker still draws +2 Cards");
}

/// Answer Diplomat's "discard 3" with the lowest-numbered card each time (never the Moat, which is
/// numbered above Copper and Estate).
fn discard_forced(g: &mut GameState) {
    let mut n = 0;
    while let Some(Decision { kind: DecisionKind::Select { act: Act::Discard, .. }, .. }) = g.pending_decision() {
        let c = choices(g)[0];
        choose(g, c);
        n += 1;
    }
    assert!(n >= 1);
}

fn is_moat_decision(d: &Decision) -> bool {
    matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }) && d.subject == id::MOAT
}

#[test]
fn moat_declined_so_the_attack_hits() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::WITCH);
    moat(&mut g, false);
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
    assert_eq!(g.players[1].hand, counts_of(&[id::MOAT]));
    assert_eq!(g.players[0].hand.total(), 2, "Witch's +2 Cards either way");
}

#[test]
fn moat_revealed_so_the_attack_is_blocked_and_the_reveal_is_logged() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT]);
    let Step::Decision(d) = play(&mut g, id::WITCH) else { panic!() };
    assert_eq!((d.player, d.source, d.subject), (1, Some(id::WITCH), id::MOAT), "asked of the attacked player, naming the Attack");
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Reveal });
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Yes, &mut ev);
    assert!(g.players[1].discard.is_empty());
    assert!(g.players[1].hand.has(id::MOAT), "Moat stays in the hand");
    assert!(ev.iter().any(|e| matches!(e, Event::Reaction { player: 1, card: id::MOAT })), "{ev:?}");
}

#[test]
fn moat_is_chosen_again_for_every_attack() {
    // Throne Room plays Militia twice: reveal for the first, not the second.
    let mut g = new_state(&[id::THRONE_ROOM, id::MILITIA, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MILITIA]);
    let victim = [id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::COPPER];
    set_hand(&mut g, 1, &victim);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MILITIA));
    moat(&mut g, true);
    assert_eq!(g.players[1].hand, counts_of(&victim), "the first play is blocked");
    moat(&mut g, false);
    // Discard down to 3: the player to choose is player 1.
    let d = g.pending_decision().expect("discard");
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }));
}

#[test]
fn a_moat_that_never_arrives_is_not_offered_and_the_attack_hits() {
    let mut g = new_state(&[id::WITCH, id::MOAT, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::COPPER]);
    play(&mut g, id::WITCH);
    choose(&mut g, Choice::Yes);
    discard_forced(&mut g);
    let d = expect_decision(&mut g);
    assert!(!is_moat_decision(&d), "{d:?}");
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
}

#[test]
fn attacked_players_share_one_reveal_per_attack_the_second_moat_copy_is_not_asked() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT, id::MOAT]);
    play(&mut g, id::WITCH);
    let step = moat(&mut g, true);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "{step:?}");
}

// ---------------------------------------------------------------------------
// Moat (reaction) and the attacks it blocks
// ---------------------------------------------------------------------------






#[test]
fn moat_blocks_bureaucrat() {
    let mut g = new_state(&[id::MOAT, id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE]);
    play(&mut g, id::BUREAUCRAT);
    moat(&mut g, true);
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
    moat(&mut g, true);
    assert_eq!(g.players[1].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::GOLD, id::SILVER], "deck untouched");
    assert_eq!(g.players[0].discard.get(id::GOLD), 1, "attacker still gains Gold");
}

#[test]
fn moat_blocks_militia_alt() {
    let mut g = new_state(&[id::MOAT, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    let victim_hand = [id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::MOAT];
    set_hand(&mut g, 1, &victim_hand);

    play(&mut g, id::MILITIA);
    let next = moat(&mut g, true);
    assert_eq!(g.players[1].hand, counts_of(&victim_hand), "Moat fully blocks Militia's discard");
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn moat_blocks_witch_alt() {
    let mut g = new_state(&[id::MOAT, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::WITCH);
    moat(&mut g, true);
    assert!(!g.players[1].discard.has(id::CURSE), "Moat blocks the Curse");
    assert_eq!(g.players[0].hand.total(), 2, "attacker still draws +2 Cards");
}

#[test]
fn ruling_moat_revealed_to_bureaucrat_still_lets_the_attacker_gain_the_silver() {
    let mut g = new_state(&[id::BUREAUCRAT, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE]);
    play(&mut g, id::BUREAUCRAT);
    moat(&mut g, true);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    assert!(g.players[1].deck_known.is_empty());
}

// ===========================================================================
// Harbinger
// ===========================================================================

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

// ===========================================================================
// Merchant
// ===========================================================================

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
fn merchant_no_silver_no_bonus() {
    let mut g = new_state(&[id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::MERCHANT, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    let step = play(&mut g, id::MERCHANT);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 1, "just the Copper, no Silver was ever played");
}

#[test]
fn ruling_merchant_played_after_the_first_silver_gives_nothing() {
    // "If you play Merchant after you play your first Silver, you do not get +$1 for the Silver you already played."
    let mut g = new_state(&[id::MERCHANT], 2);
    g.turn.silvers_played = 1;
    set_hand(&mut g, 0, &[id::MERCHANT, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::MERCHANT);
    assert_eq!(g.turn.coins, 2, "only the Silver's own $2");
}

// ===========================================================================
// Vassal
// ===========================================================================

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

#[test]
fn ruling_vassal_discarding_a_faithful_hound_uses_the_hound_or_the_play_not_both() {
    // "If you discard a Faithful Hound with Vassal, you can use Faithful Hound's Reaction ability to set it aside, or use Vassal's
    // ability to play it, but not both."
    let mut g = new_state(&[id::VASSAL, id::FAITHFUL_HOUND], 2);
    set_hand(&mut g, 0, &[id::VASSAL]);
    set_deck_known(&mut g, 0, &[id::FAITHFUL_HOUND, id::ESTATE]);
    play(&mut g, id::VASSAL);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.subject, id::FAITHFUL_HOUND, "{d:?}");
    // Set it aside (Yes to the first question, whichever it is); then no offer to play it remains.
    let mut guard = 0;
    while let Some(d) = g.pending_decision() {
        if d.kind == (DecisionKind::YesNo { act: Act::SetAside }) {
            choose(&mut g, Choice::Yes);
        } else if d.kind == (DecisionKind::YesNo { act: Act::Play }) {
            panic!("the Hound was set aside and can no longer be played: {d:?}");
        } else {
            break;
        }
        guard += 1;
        assert!(guard < 5);
    }
    assert_eq!(g.players[0].hounds, 1, "set aside, not played");
    assert!(!g.players[0].in_play.has(id::FAITHFUL_HOUND));
}

// ===========================================================================
// Village
// ===========================================================================

#[test]
fn village_basics() {
    let d = cards::def(id::VILLAGE);
    assert_eq!(d.cost, 3);
    assert!(cards::is(id::VILLAGE, cards::ACTION));
    assert_eq!(d.vp, 0);
}

#[test]
fn village_draws_and_gives_two_actions() {
    // deck of [Estate, Estate]
    let mut g = new_state(&[id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::VILLAGE);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]));
    assert_eq!(g.turn.actions, 2, "1 - 1 + 2");
}

#[test]
fn village_with_empty_deck() {
    // "No cards in deck"
    let mut g = new_state(&[id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    let next = play(&mut g, id::VILLAGE);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.turn.actions, 2);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::PlayAction, .. }) | Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
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

// ===========================================================================
// Workshop
// ===========================================================================

#[test]
fn workshop_basics() {
    let d = cards::def(id::WORKSHOP);
    assert_eq!(d.cost, 3);
    assert!(cards::is(id::WORKSHOP, cards::ACTION));
}

#[test]
fn workshop_gains_from_kingdom() {
    // "Gain a card (should be a Smithy)"
    let mut g = new_state(&[id::WORKSHOP, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    assert!(choices(&g).contains(&Choice::Card(id::SMITHY)));
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].discard, counts_of(&[id::SMITHY]));
}

#[test]
fn workshop_falls_back_to_base_cards_when_kingdom_too_pricey() {
    // "Nothing else on the board under 4, should not throw"
    let mut g = new_state(&[id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::COPPER)));
    assert!(cs.contains(&Choice::Card(id::ESTATE)));
    assert!(cs.contains(&Choice::Card(id::SILVER)));
    assert!(!cs.contains(&Choice::Card(id::DUCHY)), "cost 5 > 4");
    assert!(!cs.contains(&Choice::Card(id::GOLD)), "cost 6 > 4");
}

#[test]
fn workshop_gain_is_mandatory() {
    // "Must gain a card". In this engine a Gain
    // frame with any legal option never offers Pass, so a Pass attempt is simply illegal.
    let mut g = new_state(&[id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    assert!(!choices(&g).contains(&Choice::Pass));
    assert!(g.apply(Choice::Pass, &mut NoEvents).is_err());
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

#[test]
fn ruling_workshop_and_artisan_use_the_costs_cards_have_now() {
    // "Workshop cares about the costs cards have when you play it": with a Bridge played, a Duchy costs $4.
    let mut g = new_state(&[id::WORKSHOP, id::ARTISAN], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    assert!(choices(&g).contains(&Choice::Card(id::DUCHY)));
    // Artisan: up to $5, so a Gold (costs $5 with one Bridge) and not a Province ($7).
    let mut g = new_state(&[id::ARTISAN], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::ARTISAN, id::ESTATE]);
    play(&mut g, id::ARTISAN);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::GOLD)) && !cs.contains(&Choice::Card(id::PROVINCE)), "{cs:?}");
}

// ===========================================================================
// Bureaucrat
// ===========================================================================

#[test]
fn bureaucrat_basics() {
    let d = cards::def(id::BUREAUCRAT);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::BUREAUCRAT, cards::ACTION));
    assert!(cards::is(id::BUREAUCRAT, cards::ATTACK));
}

#[test]
fn bureaucrat_no_cards_in_other_players_hand() {
    // "No cards in other player's hand"
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT, id::ESTATE]);
    set_hand(&mut g, 1, &[]);
    let next = play(&mut g, id::BUREAUCRAT);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no Actions left in hand");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn bureaucrat_multiple_opponents_each_choose_a_victory_card() {
    // "Other players have estates, last player has Moat"
    // (adapted to have each victim independently choose between two Victory cards).
    let mut g = new_state(&[id::BUREAUCRAT], 3);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY, id::COPPER]);
    set_hand(&mut g, 2, &[id::DUCHY, id::COPPER]);
    play(&mut g, id::BUREAUCRAT);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));

    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ESTATE)) && cs.contains(&Choice::Card(id::DUCHY)));
    assert!(!cs.contains(&Choice::Card(id::COPPER)));
    choose(&mut g, Choice::Card(id::DUCHY));

    // Player 2 has only one Victory card, so its topdeck is auto-applied.
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::DUCHY));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::COPPER]));
    assert_eq!(g.players[2].deck_known.peek_top(), Some(id::DUCHY));
    assert_eq!(g.players[2].hand, counts_of(&[id::COPPER]));
}

#[test]
fn bureaucrat_no_silver_left_still_forces_the_topdeck() {
    // "No silvers left"
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::ESTATE]);
    empty_pile(&mut g, id::SILVER);
    play(&mut g, id::BUREAUCRAT);
    assert!(g.players[0].deck_counts().is_empty(), "the gain silently failed");
    assert!(g.players[1].hand.is_empty());
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::ESTATE));
}

#[test]
fn bureaucrat_victim_with_no_victory_card_is_untouched() {
    // matches the reveal-only path also exercised via Moat
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::SILVER]);
    let next = play(&mut g, id::BUREAUCRAT);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no Actions left in hand");
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER, id::SILVER]));
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
fn bureaucrat_victim_with_no_victory_card_is_untouched_alt() {
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::SILVER]);
    let next = play(&mut g, id::BUREAUCRAT);
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER, id::SILVER]));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn ruling_bureaucrat_victim_with_an_empty_deck_gets_the_victory_card_as_the_only_deck_card() {
    // "A player with no cards in their deck will have the card they put on top become the only card in their deck."
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::ESTATE, id::COPPER]);
    clear_deck(&mut g, 1);
    set_discard(&mut g, 1, &[]);
    play(&mut g, id::BUREAUCRAT);
    assert_eq!(g.players[1].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::ESTATE]);
    assert_eq!(g.players[1].deck_size(), 1);
}

// ===========================================================================
// Gardens
// ===========================================================================

#[test]
fn gardens_basics() {
    let d = cards::def(id::GARDENS);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::GARDENS, cards::VICTORY));
    assert_eq!(d.vp, 0, "static field is 0; real VP is dynamic (floor(deck size / 10) per Gardens)");
}

#[test]
fn gardens_vp_rounds_down_when_small() {
    // "Should all round down to 0"
    assert_eq!(vp_of(&[(id::GARDENS, 0)]), 0);
    assert_eq!(vp_of(&[(id::GARDENS, 1)]), 0);
    assert_eq!(vp_of(&[(id::GARDENS, 9)]), 0);
}

#[test]
fn gardens_vp_rounds_to_nearest_whole() {
    // "Should round to nearest whole"
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 9)]), 1, "total 10 -> floor(10/10)=1");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 11)]), 1, "total 12 -> floor(12/10)=1");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 24)]), 2, "total 25 -> floor(25/10)=2");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 36)]), 3, "total 37 -> floor(37/10)=3");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 154)]), 15, "total 155 -> floor(155/10)=15");
}

#[test]
fn gardens_vp_suddenly_worth_a_lot() {
    // "Suddenly worth a lot of points"
    assert_eq!(vp_of(&[(id::GARDENS, 10)]), 10);
    assert_eq!(vp_of(&[(id::GARDENS, 12)]), 12);
}

#[test]
fn gardens_finds_cards_everywhere() {
    // "Gardens should find cards everywhere" (counts every zone,
    // not just the deck; this engine's vp_of_cards already sums over `all_cards()`).
    assert_eq!(vp_of(&[(id::VILLAGE, 1), (id::SMITHY, 8), (id::GARDENS, 1)]), 1, "total 10 -> 1");
    assert_eq!(
        vp_of(&[(id::VILLAGE, 1 + 5), (id::SMITHY, 8), (id::GARDENS, 1), (id::GOLD, 5)]),
        2,
        "total 20 -> 2"
    );
}

#[test]
fn gardens_stacks_with_other_vp_cards() {
    // "Stacks with other VP cards"
    assert_eq!(vp_of(&[(id::PROVINCE, 1), (id::ESTATE, 8), (id::DUCHY, 1), (id::GARDENS, 1)]), 18);
}

#[test]
fn gardens_live_scoring_via_playerstate_vp() {
    // Same contract, but exercised through `PlayerState::vp()` on a real GameState.
    let mut g = new_state(&[id::GARDENS], 2);
    set_hand(&mut g, 0, &[id::GARDENS]);
    set_deck_unknown(&mut g, 0, &[id::COPPER; 22]);
    assert_eq!(g.players[0].all_cards().total(), 23);
    assert_eq!(g.players[0].vp(), 2, "floor(23/10) = 2");
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

#[test]
fn ruling_victory_kingdom_piles_hold_8_cards_with_two_players_and_12_with_more() {
    // Gardens: "Use 8 copies of Gardens in a 2-player game, 12 for 3 or more players."
    assert_eq!(new_state(&[id::GARDENS], 2).supply.get(id::GARDENS), 8);
    assert_eq!(new_state(&[id::GARDENS], 3).supply.get(id::GARDENS), 12);
    assert_eq!(new_state(&[id::GARDENS], 4).supply.get(id::GARDENS), 12);
}

// ===========================================================================
// Militia
// ===========================================================================

#[test]
fn militia_basics() {
    let d = cards::def(id::MILITIA);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::MILITIA, cards::ACTION));
    assert!(cards::is(id::MILITIA, cards::ATTACK));
}

#[test]
fn militia_multiple_players_varying_hand_sizes() {
    // Militia against other players, some with more and some
    // with less than 3 cards (a representative subset of hand sizes).
    let mut g = new_state(&[id::MILITIA], 4);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[]);
    set_hand(&mut g, 2, &[id::COPPER, id::COPPER]);
    set_hand(&mut g, 3, &[id::COPPER; 5]);
    play(&mut g, id::MILITIA);
    assert_eq!(g.turn.coins, 2);
    assert!(g.players[1].hand.is_empty(), "0 <= 3, untouched");
    assert_eq!(g.players[2].hand.total(), 2, "2 <= 3, untouched");
    assert_eq!(g.players[3].hand.total(), 3, "discarded down to 3");
    assert_eq!(g.players[3].discard.get(id::COPPER), 2);
}

#[test]
fn militia_hand_of_three_or_fewer_is_untouched() {
    // a hand of 3 or fewer cards discards nothing
    let mut g = new_state(&[id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER]);
    let next = play(&mut g, id::MILITIA);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER]));
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
fn militia_hand_of_three_or_fewer_is_untouched_alt() {
    let mut g = new_state(&[id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER]);
    let next = play(&mut g, id::MILITIA);
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER]));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn ruling_militia_and_moat_the_attacker_keeps_the_coin_when_the_victim_reveals_moat() {
    // "Moat does not stop anything an Attack does ... for the player who played it": Militia's +$2 still happens.
    let mut g = new_state(&[id::MILITIA, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::MILITIA);
    moat(&mut g, true);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[1].hand.total(), 5);
}

// ===========================================================================
// Moneylender
// ===========================================================================

#[test]
fn moneylender_basics() {
    let d = cards::def(id::MONEYLENDER);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::MONEYLENDER, cards::ACTION));
}

#[test]
fn moneylender_optional_in_2e() {
    // 2E: "You may trash a Copper from your hand for +$3" is optional. 1E trashed the Copper
    // with no decision at all; this engine offers Pass instead.
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::COPPER]);
    play(&mut g, id::MONEYLENDER);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass]);
}

#[test]
fn moneylender_single_copper_in_hand() {
    // "Single copper in hand"
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::COPPER]);
    play(&mut g, id::MONEYLENDER);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.turn.coins, 3);
}

#[test]
fn moneylender_copper_with_other_cards() {
    // "Single copper in hand with other cards". Silver/Gold
    // are treasures, so they auto-play into the Buy phase once Moneylender resolves (no more
    // Actions left) — check total ownership and coins, not raw `hand`.
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::SILVER, id::GOLD, id::COPPER, id::ESTATE, id::CURSE]);
    play(&mut g, id::MONEYLENDER);
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.players[0].hand.get(id::ESTATE), 1);
    assert_eq!(g.players[0].hand.get(id::CURSE), 1);
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 1);
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.turn.coins, 3 + 2 + 3, "Moneylender's +$3, plus Silver(2)+Gold(3) auto-played");
}

#[test]
fn moneylender_multiple_coppers_trashes_only_one() {
    // "Multiple coppers, should only trash one". The
    // untrashed Copper is also a treasure and auto-plays into the Buy phase.
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::COPPER, id::CURSE, id::COPPER, id::CURSE]);
    play(&mut g, id::MONEYLENDER);
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.players[0].hand.get(id::CURSE), 2);
    assert_eq!(g.players[0].all_cards().get(id::COPPER), 1, "the untrashed Copper is still owned");
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.turn.coins, 3 + 1, "Moneylender's +$3, plus the untrashed Copper auto-played");
}

#[test]
fn moneylender_no_coppers_offers_no_decision() {
    // "No coppers in hand, should not gain coins". Gold and
    // Silver still auto-play as ordinary treasures once the turn rolls into the Buy phase; only
    // Moneylender's own conditional +$3 bonus is withheld.
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::GOLD, id::SILVER, id::ESTATE, id::CURSE]);
    let next = play(&mut g, id::MONEYLENDER);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.players[0].hand.get(id::ESTATE), 1);
    assert_eq!(g.players[0].hand.get(id::CURSE), 1);
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 1);
    assert_eq!(g.turn.coins, 3 + 2, "Gold(3) + Silver(2) auto-played; no Moneylender bonus");
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

// ===========================================================================
// Poacher
// ===========================================================================

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

#[test]
fn ruling_poacher_counts_every_empty_supply_pile_including_curses_and_itself_but_not_side_piles() {
    // "This includes all Supply piles, including Curses, Coppers, Poacher itself, etc."; "Non-Supply piles, such as Spoils, do not matter."
    let mut g = new_state(&[id::POACHER], 2);
    set_hand(&mut g, 0, &[id::POACHER, id::ESTATE, id::DUCHY, id::PROVINCE]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    empty_pile(&mut g, id::CURSE);
    empty_pile(&mut g, id::POACHER);
    g.spoils = 0;
    play(&mut g, id::POACHER);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, min: 2, max: 2, .. }), "{d:?}");
    let mut g = new_state(&[id::POACHER], 2);
    set_hand(&mut g, 0, &[id::POACHER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    g.spoils = 0;
    play(&mut g, id::POACHER);
    assert_eq!(g.players[0].hand.total(), 2, "an empty Spoils pile (not in the Supply) makes no discard");
}

// ===========================================================================
// Remodel
// ===========================================================================

#[test]
fn remodel_basics() {
    let d = cards::def(id::REMODEL);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::REMODEL, cards::ACTION));
}

#[test]
fn remodel_estate_to_smithy_with_multiple_choices() {
    // "Remodel an estate into a Smithy from multiple choices".
    // Gold/Silver are treasures and auto-play into the Buy phase once Remodel resolves (Village
    // can't be played either, since 0 Actions remain) — check total ownership, not raw `hand`.
    let mut g = new_state(&[id::REMODEL, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE, id::GOLD, id::SILVER, id::VILLAGE]);
    play(&mut g, id::REMODEL);
    choose(&mut g, Choice::Card(id::ESTATE));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: cards::cost(id::ESTATE) + 2, filter: Filter::Any, dest: Dest::Discard, exact: false, potion: false, optional: false });
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].hand.get(id::VILLAGE), 1, "Village stays in hand (no Actions left to spend on it)");
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 1);
    assert!(g.players[0].discard.has(id::SMITHY));
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
}

#[test]
fn remodel_gold_to_province() {
    // "Gold to a Province". Silver/Copper auto-play into the Buy
    // phase once Remodel resolves — check total ownership and coins, not raw `hand`.
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::GOLD, id::SILVER, id::COPPER]);
    play(&mut g, id::REMODEL);
    choose(&mut g, Choice::Card(id::GOLD));
    choose(&mut g, Choice::Card(id::PROVINCE));
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 1);
    assert_eq!(g.players[0].all_cards().get(id::COPPER), 1);
    assert!(g.players[0].discard.has(id::PROVINCE));
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
    assert_eq!(g.turn.coins, 2 + 1, "Silver(2) + Copper(1) auto-played");
}

#[test]
fn remodel_upgrade_cap_is_trashed_cost_plus_two() {
    // "Remodel 5 point card with base cards, should only get
    // back cards 6 or less" (a Laboratory, cost 5, caps the gain at cost 7; Province at 8 is
    // unreachable).
    let mut g = new_state(&[id::REMODEL, id::LABORATORY], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::LABORATORY]);
    // Laboratory is the only card in hand, so its trash is the only legal choice and is
    // auto-applied by `play`; the very next decision is already the Gain.
    let step = play(&mut g, id::REMODEL);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Gain { .. }, .. })));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 7, filter: Filter::Any, dest: Dest::Discard, exact: false, potion: false, optional: false });
    assert!(choices(&g).contains(&Choice::Card(id::GOLD)), "Gold costs 6 <= 7");
    assert!(!choices(&g).contains(&Choice::Card(id::PROVINCE)), "Province costs 8 > 7");
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
    assert_eq!(g.trash, counts_of(&[id::LABORATORY]));
}

#[test]
fn remodel_nothing_in_hand_is_a_no_op() {
    // "Nothing else in hand, should not remodel anything"
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL]);
    let next = play(&mut g, id::REMODEL);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert!(g.trash.is_empty());
}

#[test]
fn remodel_trash_is_mandatory_when_something_is_available() {
    // "Must trash a card"
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE]);
    play(&mut g, id::REMODEL);
    assert!(!choices(&g).contains(&Choice::Pass));
}

#[test]
fn remodel_cannot_trash_a_card_not_in_hand() {
    // "Tried to trash a card not in the hand".
    // Two distinct candidates in hand so the trash pick isn't auto-applied as a forced single
    // choice (which would otherwise skip straight past this decision to the Gain that follows).
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE, id::DUCHY]);
    play(&mut g, id::REMODEL);
    assert!(g.apply(Choice::Card(id::COPPER), &mut NoEvents).is_err(), "Copper isn't in hand");
}

#[test]
fn remodel_cannot_gain_a_card_not_offered() {
    // "Tried to gain a card not offered": a Copper
    // (cost 0) is trashed, capping the gain at cost 2, so Gold (cost 6) is illegal.
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::COPPER]);
    play(&mut g, id::REMODEL);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.apply(Choice::Card(id::GOLD), &mut NoEvents).is_err());
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
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: cards::cost(id::GOLD) + 2, filter: Filter::Any, dest: Dest::Discard, exact: false, potion: false, optional: false });
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

#[test]
fn ruling_remodel_may_gain_a_copy_of_the_trashed_card_but_not_use_coins_to_gain_more() {
    // "The gained card ... can even be another copy of the trashed card"; "You cannot use $ to increase how expensive a card you gain."
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE]);
    g.turn.coins = 8;
    play(&mut g, id::REMODEL);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ESTATE)), "a copy of the trashed card");
    assert!(cs.contains(&Choice::Card(id::SILVER)), "$2 + 2 = $4 and less");
    assert!(!cs.contains(&Choice::Card(id::GOLD)), "{cs:?}");
    assert!(!cs.contains(&Choice::Card(id::DUCHY)), "{cs:?}");
}

// ===========================================================================
// Smithy
// ===========================================================================

#[test]
fn smithy_basics() {
    let d = cards::def(id::SMITHY);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::SMITHY, cards::ACTION));
}

#[test]
fn smithy_full_deck() {
    // deck { Province, Duchy, Estate, Gold }, 3 drawn, 1 left
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::DUCHY, id::ESTATE, id::GOLD]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::GOLD]));
    assert_eq!(g.turn.actions, 0, "1 - 1 + 0");
}

#[test]
fn smithy_only_two_cards_in_deck() {
    // "Only 2 cards in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert!(g.players[0].deck_counts().is_empty());
}

#[test]
fn smithy_only_one_card_in_deck() {
    // "Only 1 card in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].deck_counts().is_empty());
}

#[test]
fn smithy_empty_deck_and_discard() {
    // "No cards in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    play(&mut g, id::SMITHY);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn smithy_cards_in_discard_but_not_deck() {
    // "Cards in discard, but not in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_discard(&mut g, 0, &[id::ESTATE; 4]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]), "1 of the 4 shuffled Estates remains");
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn smithy_some_in_deck_but_not_enough() {
    // "Some cards in deck, but not enough" (spans a reshuffle)
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    // (A Duchy rather than a Gold here, since this engine auto-plays
    // any treasure left in the current player's hand the instant the turn rolls into the Buy
    // phase, which would otherwise move it out of `hand` before this assertion runs.)
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    set_discard(&mut g, 0, &[id::ESTATE; 3]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::DUCHY]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]));
    assert!(g.players[0].discard.is_empty());
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

// ===========================================================================
// Throne Room
// ===========================================================================

#[test]
fn throne_room_basics() {
    let d = cards::def(id::THRONE_ROOM);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::THRONE_ROOM, cards::ACTION));
}

#[test]
fn throne_room_is_optional_in_2e() {
    // 2E: "You may play an Action card from your hand twice." 1E's wording was mandatory;
    // in 2E, Pass is a legal choice even with a playable Action in hand.
    let mut g = new_state(&[id::THRONE_ROOM, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::WORKSHOP]);
    play(&mut g, id::THRONE_ROOM);
    assert!(choices(&g).contains(&Choice::Pass));
}

fn verify_throne_room_played(g: &GameState, actions: u16, buys: u16, coins: u16) {
    assert_eq!(g.turn.actions as u16, actions);
    assert_eq!(g.turn.buys as u16, buys);
    assert_eq!(g.turn.coins, coins);
}

#[test]
fn throne_room_smithy() {
    // "ThroneRoom a Smithy"
    let mut g = new_state(&[id::THRONE_ROOM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SMITHY));
    verify_throne_room_played(&g, 0, 1, 0);
    assert_eq!(g.players[0].hand.total(), 6);
    assert_eq!(g.players[0].deck_counts().total(), 4);
}

#[test]
fn throne_room_village() {
    // "ThroneRoom a Village"
    let mut g = new_state(&[id::THRONE_ROOM, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::VILLAGE));
    verify_throne_room_played(&g, 4, 1, 0);
    assert_eq!(g.players[0].hand.total(), 2);
    assert_eq!(g.players[0].deck_counts().total(), 3);
}

#[test]
fn throne_room_market() {
    // "ThroneRoom a Market"
    let mut g = new_state(&[id::THRONE_ROOM, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MARKET]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MARKET));
    verify_throne_room_played(&g, 2, 3, 2);
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn throne_room_remodel() {
    // "ThroneRoom a Remodel"
    let mut g = new_state(&[id::THRONE_ROOM, id::REMODEL, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::REMODEL, id::SILVER, id::SILVER]);
    play(&mut g, id::THRONE_ROOM);
    // Each Remodel resolution's trash pick has only one distinct legal card (Silver, even
    // though 2 copies are present) and is auto-applied; only the Gain needs an explicit choice.
    choose(&mut g, Choice::Card(id::REMODEL));
    choose(&mut g, Choice::Card(id::SMITHY)); // 1st Remodel resolution's gain
    choose(&mut g, Choice::Card(id::SMITHY)); // 2nd Remodel resolution's gain
    verify_throne_room_played(&g, 0, 1, 0);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::SMITHY, id::SMITHY]));
    assert_eq!(g.trash, counts_of(&[id::SILVER, id::SILVER]));
}

#[test]
fn throne_room_witch() {
    // "ThroneRoom a Witch"
    let mut g = new_state(&[id::THRONE_ROOM, id::WITCH], 3);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::WITCH]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::WITCH));
    verify_throne_room_played(&g, 0, 1, 0);
    assert_eq!(g.players[0].hand.total(), 4, "2x Witch's +2 Cards");
    // Witch is resolved twice, so each opponent is cursed twice.
    assert_eq!(g.players[1].discard.get(id::CURSE), 2);
    assert_eq!(g.players[2].discard.get(id::CURSE), 2);
}

#[test]
fn throne_room_militia() {
    // "ThroneRoom a Militia"
    let mut g = new_state(&[id::THRONE_ROOM, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MILITIA]);
    set_hand(&mut g, 1, &[id::ESTATE; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MILITIA));
    verify_throne_room_played(&g, 0, 1, 4);
    assert_eq!(g.players[1].hand.total(), 3, "discarded down to 3, doesn't matter that Militia hit twice");
}

#[test]
fn throne_room_moneylender() {
    // "ThroneRoom a MoneyLender"
    let mut g = new_state(&[id::THRONE_ROOM, id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MONEYLENDER, id::COPPER, id::COPPER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MONEYLENDER));
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::COPPER));
    verify_throne_room_played(&g, 0, 1, 6);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::COPPER]));
}

#[test]
fn throne_room_workshop() {
    // "ThroneRoom a Workshop"
    let mut g = new_state(&[id::THRONE_ROOM, id::WORKSHOP, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::WORKSHOP]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::WORKSHOP));
    choose(&mut g, Choice::Card(id::SMITHY));
    choose(&mut g, Choice::Card(id::SMITHY));
    verify_throne_room_played(&g, 0, 1, 0);
    assert_eq!(g.players[0].discard, counts_of(&[id::SMITHY, id::SMITHY]));
}

#[test]
fn throne_room_mine_copper_to_gold() {
    // "ThroneRoom to Mine a single Copper into a Gold". Also
    // exercises the 2E Mine adaptation (gains to hand).
    let mut g = new_state(&[id::THRONE_ROOM, id::MINE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MINE, id::COPPER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MINE));
    // First Mine resolution: trash the Copper, gain a Silver to hand.
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::SILVER));
    // Second Mine resolution: trash the just-gained Silver, gain a Gold to hand.
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.turn.actions, 0);
    assert_eq!(g.turn.buys, 1);
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::SILVER]));
    // The final Gold is the only card left, so it's auto-played into the Buy phase; check via
    // total ownership rather than hand/in_play location.
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 0);
}

#[test]
fn throne_room_on_throne_room_with_nothing_left_to_play() {
    // "ThroneRoom a ThroneRoom with nothing to play, should
    // not throw"
    let mut g = new_state(&[id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::THRONE_ROOM]);
    play(&mut g, id::THRONE_ROOM);
    let next = choose_and_advance(&mut g, Choice::Card(id::THRONE_ROOM));
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert!(g.players[0].hand.is_empty());
    verify_throne_room_played(&g, 0, 1, 0);
}

#[test]
fn throne_room_on_throne_room_chains_two_more_actions() {
    // "ThroneRoom a ThroneRoom to play more actions". Festival stands in for a +2 Actions card
    // (Woodcutter, +1 Buy +$2 and no Actions, is not in 2E), so the expected action count is 4;
    // and the buys/coins totals are unchanged.
    let mut g = new_state(&[id::THRONE_ROOM, id::FESTIVAL, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::THRONE_ROOM, id::FESTIVAL, id::MILITIA]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::THRONE_ROOM)); // outer TR targets the inner TR
    choose(&mut g, Choice::Card(id::FESTIVAL)); // inner TR's 1st resolution targets Festival
    let next = choose_and_advance(&mut g, Choice::Card(id::MILITIA)); // inner TR's 2nd resolution targets Militia
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    verify_throne_room_played(&g, 4, 3, 8);
    assert_eq!(g.players[1].hand.total(), 3, "hit by Militia (twice, idempotent after the first)");
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
fn ruling_throne_room_plays_a_card_that_leaves_play_a_second_time() {
    // "If you use Throne Room to play a card that removes itself from the play area, such as Horse, Throne Room will still play the
    // card a second time."
    let mut g = new_state(&[id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HORSE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HORSE));
    assert_eq!(g.players[0].hand.total(), 4, "Horse's +2 Cards twice");
    assert_eq!(g.turn.actions, 2, "1 - 1 for Throne Room, +1 Action twice");
    assert!(!g.players[0].in_play.has(id::HORSE), "returned to the Horse pile");
}

// ===========================================================================
// Bandit
// ===========================================================================

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

#[test]
fn ruling_bandit_gains_the_gold_before_the_attack() {
    // "First you gain a Gold from the Supply, putting it into your discard pile."
    let mut g = new_state(&[id::BANDIT], 2);
    set_hand(&mut g, 0, &[id::BANDIT]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::GOLD]);
    play(&mut g, id::BANDIT);
    assert!(g.players[0].discard.has(id::GOLD));
}

// ===========================================================================
// Council Room
// ===========================================================================

#[test]
fn council_room_basics() {
    let d = cards::def(id::COUNCIL_ROOM);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::COUNCIL_ROOM, cards::ACTION));
}

#[test]
fn council_room_one_other_player() {
    // "One other player"
    let mut g = new_state(&[id::COUNCIL_ROOM], 2);
    set_hand(&mut g, 0, &[id::COUNCIL_ROOM]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 5]);
    set_deck_known(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::COUNCIL_ROOM);
    assert_eq!(g.players[0].hand.total(), 4);
    assert_eq!(g.players[0].deck_counts().total(), 1);
    assert!(g.players[1].hand.has(id::ESTATE), "the other player also drew a card");
    assert_eq!(g.turn.buys, 2, "1 + 1");
}

#[test]
fn council_room_current_player_in_the_middle() {
    // "Two other players, current is in the middle"
    let mut g = new_state(&[id::COUNCIL_ROOM], 3);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::COUNCIL_ROOM, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::ESTATE; 4]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 2, &[id::ESTATE]);
    play(&mut g, id::COUNCIL_ROOM);
    assert_eq!(g.players[1].hand.total(), 5, "the pre-existing Estate plus 4 drawn");
    assert!(g.players[0].hand.has(id::ESTATE));
    assert!(g.players[2].hand.has(id::ESTATE));
    assert_eq!(g.turn.player, 1, "still the same player's turn");
}

#[test]
fn council_room_neither_player_has_enough_cards_to_draw() {
    // "Neither player has enough cards to draw"
    let mut g = new_state(&[id::COUNCIL_ROOM], 2);
    set_hand(&mut g, 0, &[id::COUNCIL_ROOM]);
    set_hand(&mut g, 1, &[id::ESTATE; 5]);
    let next = play(&mut g, id::COUNCIL_ROOM);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE; 5]));
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

#[test]
fn ruling_council_room_other_players_draw_whether_they_want_to_or_not() {
    let mut g = new_state(&[id::COUNCIL_ROOM], 3);
    set_hand(&mut g, 0, &[id::COUNCIL_ROOM]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 4]);
    set_deck_known(&mut g, 1, &[id::COPPER]);
    set_deck_known(&mut g, 2, &[id::COPPER]);
    let step = play(&mut g, id::COUNCIL_ROOM);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no decision for the others: {step:?}");
    assert_eq!(g.players[1].hand.total(), 1);
    assert_eq!(g.players[2].hand.total(), 1);
}

// ===========================================================================
// Festival
// ===========================================================================

#[test]
fn festival_gives_actions_buy_and_coin() {
    let d = cards::def(id::FESTIVAL);
    assert_eq!(d.cost, 5);
    let mut g = new_state(&[id::FESTIVAL], 2);
    set_hand(&mut g, 0, &[id::FESTIVAL]);
    play(&mut g, id::FESTIVAL);
    assert_eq!(g.turn.actions, 2, "1 - 1 + 2");
    assert_eq!(g.turn.buys, 2, "1 + 1");
    assert_eq!(g.turn.coins, 2);
    assert!(g.players[0].hand.is_empty(), "Festival draws no cards");
}

#[test]
fn festival_gives_actions_buy_and_coin_alt() {
    let mut g = new_state(&[id::FESTIVAL], 2);
    set_hand(&mut g, 0, &[id::FESTIVAL]);
    play(&mut g, id::FESTIVAL);
    assert_eq!(g.turn.actions, 2, "1 - 1 + 2");
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.turn.coins, 2);
}

// ===========================================================================
// Laboratory
// ===========================================================================

#[test]
fn laboratory_draws_two_keeps_action() {
    let d = cards::def(id::LABORATORY);
    assert_eq!(d.cost, 5);
    let mut g = new_state(&[id::LABORATORY], 2);
    set_hand(&mut g, 0, &[id::LABORATORY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::LABORATORY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE]));
    assert!(g.players[0].deck_counts().is_empty());
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1");
}

#[test]
fn laboratory_draws_two_keeps_action_alt() {
    let mut g = new_state(&[id::LABORATORY], 2);
    set_hand(&mut g, 0, &[id::LABORATORY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::LABORATORY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1");
}

// ===========================================================================
// Library
// ===========================================================================

#[test]
fn library_basics() {
    let d = cards::def(id::LIBRARY);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::LIBRARY, cards::ACTION));
}

fn answer_all_library_decisions(g: &mut GameState, keep_actions: bool) {
    loop {
        match g.pending_decision() {
            Some(Decision { kind: DecisionKind::YesNo { act: Act::SetAside }, .. }) => {
                choose(g, if keep_actions { Choice::No } else { Choice::Yes });
            }
            _ => break,
        }
    }
}

#[test]
fn library_no_actions_drawn_leaves_the_rest_in_deck() {
    // "Should draw 7 Estates, leaving the duchies in the deck"
    let mut g = new_state(&[id::LIBRARY], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    set_deck_known(
        &mut g,
        0,
        &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY, id::DUCHY],
    );
    let next = play(&mut g, id::LIBRARY);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE; 7]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::DUCHY; 3]));
}

#[test]
fn library_keeps_actions_when_told_to() {
    // "Should draw all the 7 cards in the deck" (keeping Actions).
    // Treasures are avoided here (no Copper/Gold in the deck) since they'd be
    // auto-played the instant the turn rolls into the Buy phase, before we could inspect them.
    let mut g = new_state(&[id::LIBRARY, id::VILLAGE, id::MOAT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::CURSE, id::ESTATE, id::MOAT]);
    set_discard(&mut g, 0, &[id::DUCHY, id::SMITHY, id::CURSE]);
    play(&mut g, id::LIBRARY);
    answer_all_library_decisions(&mut g, true /*keep*/);
    let ps = &g.players[0];
    assert_eq!(ps.hand.total(), 7);
    assert!(ps.hand.has(id::VILLAGE) && ps.hand.has(id::MOAT) && ps.hand.has(id::SMITHY), "actions were kept");
    assert!(ps.deck_counts().is_empty());
    assert!(ps.discard.is_empty());
}

#[test]
fn library_skips_actions_which_get_discarded_at_the_end() {
    // "Should draw 7 cards, but chooses to skip keeping the
    // actions". Merchant stands in for Woodcutter (not in 2E).
    let mut g = new_state(&[id::LIBRARY, id::VILLAGE, id::MOAT, id::MERCHANT], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::VILLAGE, id::ESTATE, id::MOAT]);
    set_discard(&mut g, 0, &[id::DUCHY, id::MERCHANT, id::PROVINCE]);
    play(&mut g, id::LIBRARY);
    answer_all_library_decisions(&mut g, false /*skip*/);
    let ps = &g.players[0];
    assert_eq!(ps.hand, counts_of(&[id::CURSE, id::ESTATE, id::DUCHY, id::PROVINCE]), "the 4 non-action cards");
    assert_eq!(ps.discard, counts_of(&[id::VILLAGE, id::MOAT, id::MERCHANT]), "the 3 skipped actions");
    assert!(ps.deck_counts().is_empty());
    assert!(ps.set_aside.is_empty());
}

#[test]
fn library_not_enough_cards_skips_actions_and_stops_short() {
    // "Not enough in the deck, so should draw 5 cards" (skipping
    // actions). Curse rather than Gold, to avoid the Buy-phase auto-play
    // pitfall (see comment above); the shape of the test (2 skipped actions, 5 kept) matches.
    let mut g = new_state(&[id::LIBRARY, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::SMITHY, id::CURSE, id::DUCHY, id::SMITHY, id::CURSE, id::PROVINCE]);
    play(&mut g, id::LIBRARY);
    answer_all_library_decisions(&mut g, false /*skip*/);
    let ps = &g.players[0];
    assert_eq!(ps.hand, counts_of(&[id::ESTATE, id::CURSE, id::DUCHY, id::CURSE, id::PROVINCE]));
    assert_eq!(ps.discard, counts_of(&[id::SMITHY, id::SMITHY]));
    assert!(ps.deck_counts().is_empty());
}

#[test]
fn library_only_actions_left_gains_nothing_and_terminates() {
    // "Only actions left, should gain nothing (and not loop
    // forever)". Woodcutter (not in 2E) is replaced with Merchant.
    let mut g = new_state(&[id::LIBRARY, id::VILLAGE, id::SMITHY, id::MERCHANT, id::CHAPEL, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::SMITHY, id::MERCHANT]);
    set_discard(&mut g, 0, &[id::CHAPEL, id::CELLAR]);
    play(&mut g, id::LIBRARY);
    answer_all_library_decisions(&mut g, false /*skip*/);
    let ps = &g.players[0];
    assert!(ps.hand.is_empty());
    assert_eq!(ps.discard, counts_of(&[id::VILLAGE, id::SMITHY, id::MERCHANT, id::CHAPEL, id::CELLAR]));
    assert!(ps.deck_counts().is_empty());
}

#[test]
fn library_nothing_in_deck_or_discard() {
    // "Nothing else in the deck, should gain nothing"
    let mut g = new_state(&[id::LIBRARY], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    let next = play(&mut g, id::LIBRARY);
    assert!(g.players[0].hand.is_empty());
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
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

#[test]
fn ruling_library_with_the_minus_one_card_token_removes_it_and_draws_to_seven() {
    // "If you have 6 or fewer cards in hand and your -1 Card token is on your deck, you'll remove the token and will draw until you
    // have 7 cards."
    let mut g = new_state(&[id::LIBRARY], 2);
    g.players[0].adv |= state::ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::LIBRARY, id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 8]);
    play(&mut g, id::LIBRARY);
    assert_eq!(g.players[0].hand.total(), 7);
    assert_eq!(g.players[0].adv & state::ADV_MINUS_CARD, 0, "the token is gone");
    // With 7 or more cards in hand to start, nothing is drawn and the token stays.
    let mut g = new_state(&[id::LIBRARY], 2);
    g.players[0].adv |= state::ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::LIBRARY, id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY, id::PROVINCE, id::PROVINCE, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::COPPER; 8]);
    play(&mut g, id::LIBRARY);
    assert_eq!(g.players[0].hand.total(), 7);
    assert_eq!(g.players[0].adv & state::ADV_MINUS_CARD, state::ADV_MINUS_CARD);
}

#[test]
fn ruling_library_does_not_shuffle_in_the_set_aside_cards_and_discards_them_afterwards() {
    // "If you shuffle in the middle of doing this, you do not shuffle in the set aside cards."; "You are not forced to set aside
    // Action cards" (covered by library_keeps_actions_when_told_to).
    let mut g = new_state(&[id::LIBRARY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    set_deck_known(&mut g, 0, &[id::VILLAGE]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::LIBRARY);
    // Village is drawn first: set it aside; then the deck is empty, the discard (Estate only) is shuffled in.
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].hand.has(id::ESTATE));
    assert!(!g.players[0].hand.has(id::VILLAGE));
    assert!(g.players[0].discard.has(id::VILLAGE), "discarded at the end, not shuffled into the deck");
}

// ===========================================================================
// Market
// ===========================================================================

#[test]
fn market_basics() {
    let d = cards::def(id::MARKET);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::MARKET, cards::ACTION));
}

fn verify_market_played(g: &GameState) {
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1");
    assert_eq!(g.turn.buys, 2, "1 + 1");
    assert_eq!(g.turn.coins, 1);
}

#[test]
fn market_normal_draw() {
    let mut g = new_state(&[id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::MARKET);
    verify_market_played(&g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::DUCHY]));
}

#[test]
fn market_no_cards_left() {
    // "No cards left"
    let mut g = new_state(&[id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    play(&mut g, id::MARKET);
    verify_market_played(&g);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn market_deck_empty_reshuffles_from_discard() {
    // "Deck empty with cards in discard"
    let mut g = new_state(&[id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    set_discard(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::MARKET);
    verify_market_played(&g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]));
    assert!(g.players[0].discard.is_empty());
}

// ---------------------------------------------------------------------------
// Festival / Laboratory / Market
// ---------------------------------------------------------------------------






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

// ===========================================================================
// Mine
// ===========================================================================

#[test]
fn mine_basics() {
    let d = cards::def(id::MINE);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::MINE, cards::ACTION));
}

#[test]
fn mine_is_optional_and_gains_to_hand_in_2e() {
    // 2E: "You may trash a Treasure... gain a Treasure to your hand costing up to
    // $3 more." 1E made the trash mandatory and gained to the discard pile; 2E offers Pass, and
    // (checked separately below) the
    // gain lands in hand.
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::GOLD]);
    play(&mut g, id::MINE);
    assert_eq!(choices(&g), vec![Choice::Card(id::GOLD), Choice::Pass]);
}

#[test]
fn mine_copper_to_silver_gains_to_hand() {
    // "Copper to a Silver"
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::COPPER]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::COPPER));
    let mut events: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SILVER), &mut events);
    assert!(events.iter().any(|e| matches!(e, Event::Gain { card, to: Dest::Hand, .. } if *card == id::SILVER)));
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
}

#[test]
fn mine_silver_to_gold() {
    // "Silver to a Gold"
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::SILVER]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.trash, counts_of(&[id::SILVER]));
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
}

#[test]
fn mine_gold_to_gold() {
    // "Gold to a Gold"
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::GOLD]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::GOLD));
    assert!(choices(&g).contains(&Choice::Card(id::GOLD)));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
}

#[test]
fn mine_gold_to_copper_downgrade_is_legal() {
    // "Gold to a Copper". Real Mine has no lower bound on the
    // gained Treasure's cost, so this "downgrade" is legal in both 1E and 2E; not a bug.
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::GOLD]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::GOLD));
    assert!(choices(&g).contains(&Choice::Card(id::COPPER)));
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
}

#[test]
fn mine_empty_hand_is_a_no_op() {
    // "Mine with empty hand"
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE]);
    let next = play(&mut g, id::MINE);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn mine_no_treasures_in_hand_is_a_no_op() {
    // "Mine with cards in hand, but no treasures"
    let mut g = new_state(&[id::MINE, id::VILLAGE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MINE, id::VILLAGE, id::CURSE, id::MOAT]);
    let next = play(&mut g, id::MINE);
    // Mine grants no Actions itself, so even though Village/Moat remain in hand, 0 Actions
    // remain to play them with — the turn goes straight to the Buy phase.
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::CURSE, id::MOAT]));
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

// ===========================================================================
// Sentry
// ===========================================================================

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

#[test]
fn ruling_sentry_resolves_trash_then_discard_then_the_put_back() {
    // "You finish trashing any cards you choose to before you decide whether to discard any of the remaining cards."
    let mut g = new_state(&[id::SENTRY], 2);
    set_hand(&mut g, 0, &[id::SENTRY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER, id::SILVER]);
    play(&mut g, id::SENTRY);
    let mut kinds = vec![];
    for _ in 0..4 {
        match g.pending_decision().map(|d| d.kind) {
            Some(DecisionKind::Select { act, .. }) => kinds.push(act),
            _ => break,
        }
        if kinds.len() == 3 {
            break;
        }
        pass(&mut g);
    }
    assert_eq!(kinds, vec![Act::Trash, Act::Discard, Act::Topdeck], "trash, then discard, then the put-back");
}

// ===========================================================================
// Witch
// ===========================================================================

#[test]
fn witch_basics() {
    let d = cards::def(id::WITCH);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::WITCH, cards::ACTION));
    assert!(cards::is(id::WITCH, cards::ATTACK));
}

#[test]
fn witch_two_other_players() {
    // "Witch with 2 other players"
    let mut g = new_state(&[id::WITCH], 3);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::WITCH);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    assert_eq!(g.players[2].discard.get(id::CURSE), 1);
}

#[test]
fn witch_no_curses_left() {
    // "No more curses"
    let mut g = new_state(&[id::WITCH], 3);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    empty_pile(&mut g, id::CURSE);
    play(&mut g, id::WITCH);
    // Still draws 2 cards, but nothing else should change.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert!(g.players[1].discard.is_empty());
    assert!(g.players[2].discard.is_empty());
}

#[test]
fn witch_not_enough_curses_leftmost_players_get_priority() {
    // "Not enough curses"
    let mut g = new_state(&[id::WITCH], 4);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::WITCH]);
    set_supply(&mut g, id::CURSE, 2);
    play(&mut g, id::WITCH);
    assert_eq!(g.players[2].discard.get(id::CURSE), 1, "leftmost gets one");
    assert_eq!(g.players[3].discard.get(id::CURSE), 1, "second leftmost gets the last one");
    assert!(g.players[0].discard.is_empty(), "the current player's left-most-but-last opponent misses out");
    assert_eq!(g.supply.get(id::CURSE), 0);
}

#[test]
fn a_player_without_a_reaction_is_never_asked() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE]);
    let step = play(&mut g, id::WITCH);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "{step:?}");
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
}

#[test]
fn each_attacked_player_chooses_for_themselves_in_turn_order() {
    let mut g = new_state(&[id::WITCH, id::MOAT], 3);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_hand(&mut g, 2, &[id::MOAT]);
    let Step::Decision(d) = play(&mut g, id::WITCH) else { panic!() };
    assert_eq!(d.player, 1);
    let Step::Decision(d) = choose(&mut g, Choice::Yes) else { panic!() };
    assert!(is_moat_decision(&d));
    assert_eq!(d.player, 2);
    choose(&mut g, Choice::No);
    assert!(g.players[1].discard.is_empty());
    assert_eq!(g.players[2].discard, counts_of(&[id::CURSE]));
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

// ===========================================================================
// Artisan
// ===========================================================================

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
fn ruling_artisan_may_put_the_card_it_just_gained_onto_the_deck() {
    // "After gaining the card, you put a card from your hand onto your deck; that can be the card you just gained."
    let mut g = new_state(&[id::ARTISAN], 2);
    set_hand(&mut g, 0, &[id::ARTISAN, id::ESTATE]);
    play(&mut g, id::ARTISAN);
    choose(&mut g, Choice::Card(id::SILVER));
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SILVER)), "{cs:?}");
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    assert!(g.players[0].hand.has(id::ESTATE));
}
