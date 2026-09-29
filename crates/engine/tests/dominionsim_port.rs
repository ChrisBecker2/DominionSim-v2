//! Cards tests ported from the old C++ simulator's `DominionSimTestCards\DominionCardsTests.cpp`
//! (`DominionSimTest::DominionCardsTests`), adapted to this engine's API and to Base Set 2nd
//! edition rules. Each test cites the C++ `TEST_METHOD` it was ported from.
//!
//! Scope: only cards that exist in the Base Set 2nd edition. Skipped entirely (not in 2E, or
//! Duration cards which belong to a different expansion):
//!   TestWoodcutter, TestAdventurer, TestFeast, TestChancellor, TestThief, TestSpy,
//!   TestThroneRoomDuration.
//!
//! 1E -> 2E adaptations (called out again at each relevant test):
//!   - Throne Room: "You may play an Action card from your hand twice" (optional in 2E; the
//!     C++ suite's `VerifyThrows<Cheat>` "must ThroneRoom a card" case is inverted below).
//!   - Moneylender: "You may trash a Copper from your hand for +$3" (optional; the C++ suite
//!     treated the trash as automatic/mandatory).
//!   - Mine: "You may trash a Treasure... gain a Treasure to your HAND" (optional, and gains
//!     to hand rather than discard; the C++ suite's cheat case "must trash a treasure" is
//!     likewise inverted).
//!   - Bureaucrat: matches the C++ suite already (gains Silver to the top of the deck).
//!
//! Not ported (test the C++ engine's own scripted-strategy / simulation harness, discount
//! mechanics, Potion-costing cards, or promos not present in this engine, none of which this
//! engine models): the `VerifyOptimizations<BasicBigMoney>` cases, discount (`AddDiscount`)
//! cases, Potion/Transmute/Apothecary/Harem/Platinum cases, and the `-1 Card token` cases.

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
// Treasures: Copper, Silver, Gold — C++ TestCopper / TestSilver / TestGold
// ===========================================================================

#[test]
fn dsim_copper_basics() {
    // C++ TEST_METHOD(TestCopper) — VerifyBasics<MatchTreasure>(Copper, "Copper", 0, 0)
    let d = cards::def(id::COPPER);
    assert_eq!(d.cost, 0);
    assert!(cards::is(id::COPPER, cards::TREASURE));
    assert_eq!(d.vp, 0);
    // Variable "copper worth" (AddCopperWorth) isn't modeled by this engine (no such kingdom
    // card is implemented in Base Set 2E), so only the plain worth is ported below.
}

#[test]
fn dsim_copper_worth_one() {
    // C++ TEST_METHOD(TestCopper) — TestTreasure(Copper, 1)
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::COPPER]);
    let step = adv(&mut g);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.turn.coins, 1);
}

#[test]
fn dsim_silver_basics_and_worth() {
    // C++ TEST_METHOD(TestSilver) — VerifyBasics<MatchTreasure>(Silver, "Silver", 3, 0); TestTreasure(Silver, 2)
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
fn dsim_gold_basics_and_worth() {
    // C++ TEST_METHOD(TestGold) — VerifyBasics<MatchTreasure>(Gold, "Gold", 6, 0); TestTreasure(Gold, 3)
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
// Victory cards: Estate, Duchy, Province — C++ TestEstate / TestDuchy / TestProvince
// ===========================================================================

#[test]
fn dsim_estate_basics_and_vp() {
    // C++ TEST_METHOD(TestEstate)
    let d = cards::def(id::ESTATE);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::ESTATE, cards::VICTORY));
    assert_eq!(d.vp, 1);
    assert_eq!(vp_of(&[(id::ESTATE, 1)]), 1);
    assert_eq!(vp_of(&[(id::ESTATE, 2)]), 2);
    assert_eq!(vp_of(&[(id::ESTATE, 3)]), 3);
    assert_eq!(vp_of(&[(id::ESTATE, 10)]), 10);
}

#[test]
fn dsim_duchy_basics_and_vp() {
    // C++ TEST_METHOD(TestDuchy)
    let d = cards::def(id::DUCHY);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::DUCHY, cards::VICTORY));
    assert_eq!(d.vp, 3);
    assert_eq!(vp_of(&[(id::DUCHY, 1)]), 3);
    assert_eq!(vp_of(&[(id::DUCHY, 2)]), 6);
    assert_eq!(vp_of(&[(id::DUCHY, 3)]), 9);
    assert_eq!(vp_of(&[(id::DUCHY, 10)]), 30);
}

#[test]
fn dsim_province_basics_and_vp() {
    // C++ TEST_METHOD(TestProvince)
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
// Curse — C++ TestCurse
// ===========================================================================

#[test]
fn dsim_curse_basics_and_vp() {
    // C++ TEST_METHOD(TestCurse)
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
// Gardens ("Garden" in the C++ suite) — C++ TestGarden
// ===========================================================================

#[test]
fn dsim_gardens_basics() {
    // C++ TEST_METHOD(TestGarden) — VerifyBasics<MatchVictory, MatchHasExtendedVP>(Garden, "Garden", 4, 0)
    let d = cards::def(id::GARDENS);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::GARDENS, cards::VICTORY));
    assert_eq!(d.vp, 0, "static field is 0; real VP is dynamic (floor(deck size / 10) per Gardens)");
}

#[test]
fn dsim_gardens_vp_rounds_down_when_small() {
    // C++ TEST_METHOD(TestGarden) — "Should all round down to 0"
    assert_eq!(vp_of(&[(id::GARDENS, 0)]), 0);
    assert_eq!(vp_of(&[(id::GARDENS, 1)]), 0);
    assert_eq!(vp_of(&[(id::GARDENS, 9)]), 0);
}

#[test]
fn dsim_gardens_vp_rounds_to_nearest_whole() {
    // C++ TEST_METHOD(TestGarden) — "Should round to nearest whole"
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 9)]), 1, "total 10 -> floor(10/10)=1");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 11)]), 1, "total 12 -> floor(12/10)=1");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 24)]), 2, "total 25 -> floor(25/10)=2");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 36)]), 3, "total 37 -> floor(37/10)=3");
    assert_eq!(vp_of(&[(id::GARDENS, 1), (id::GOLD, 154)]), 15, "total 155 -> floor(155/10)=15");
}

#[test]
fn dsim_gardens_vp_suddenly_worth_a_lot() {
    // C++ TEST_METHOD(TestGarden) — "Suddenly worth a lot of points"
    assert_eq!(vp_of(&[(id::GARDENS, 10)]), 10);
    assert_eq!(vp_of(&[(id::GARDENS, 12)]), 12);
}

#[test]
fn dsim_gardens_finds_cards_everywhere() {
    // C++ TEST_METHOD(TestGarden) — "Gardens should find cards everywhere" (counts every zone,
    // not just the deck; this engine's vp_of_cards already sums over `all_cards()`).
    assert_eq!(vp_of(&[(id::VILLAGE, 1), (id::SMITHY, 8), (id::GARDENS, 1)]), 1, "total 10 -> 1");
    assert_eq!(
        vp_of(&[(id::VILLAGE, 1 + 5), (id::SMITHY, 8), (id::GARDENS, 1), (id::GOLD, 5)]),
        2,
        "total 20 -> 2"
    );
}

#[test]
fn dsim_gardens_stacks_with_other_vp_cards() {
    // C++ TEST_METHOD(TestGarden) — "Stacks with other VP cards"
    assert_eq!(vp_of(&[(id::PROVINCE, 1), (id::ESTATE, 8), (id::DUCHY, 1), (id::GARDENS, 1)]), 18);
}

#[test]
fn dsim_gardens_live_scoring_via_playerstate_vp() {
    // Same contract, but exercised through `PlayerState::vp()` on a real GameState.
    let mut g = new_state(&[id::GARDENS], 2);
    set_hand(&mut g, 0, &[id::GARDENS]);
    set_deck_unknown(&mut g, 0, &[id::COPPER; 22]);
    assert_eq!(g.players[0].all_cards().total(), 23);
    assert_eq!(g.players[0].vp(), 2, "floor(23/10) = 2");
}

// ===========================================================================
// Village — C++ TestVillage
// ===========================================================================

#[test]
fn dsim_village_basics() {
    let d = cards::def(id::VILLAGE);
    assert_eq!(d.cost, 3);
    assert!(cards::is(id::VILLAGE, cards::ACTION));
    assert_eq!(d.vp, 0);
}

#[test]
fn dsim_village_draws_and_gives_two_actions() {
    // C++ TEST_METHOD(TestVillage) — deck of [Estate, Estate]
    let mut g = new_state(&[id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::VILLAGE);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]));
    assert_eq!(g.turn.actions, 2, "1 - 1 + 2");
}

#[test]
fn dsim_village_with_empty_deck() {
    // C++ TEST_METHOD(TestVillage) — "No cards in deck"
    let mut g = new_state(&[id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    let next = play(&mut g, id::VILLAGE);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.turn.actions, 2);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::PlayAction, .. }) | Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

// ===========================================================================
// Smithy — C++ TestSmithy
// ===========================================================================

#[test]
fn dsim_smithy_basics() {
    let d = cards::def(id::SMITHY);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::SMITHY, cards::ACTION));
}

#[test]
fn dsim_smithy_full_deck() {
    // C++ TEST_METHOD(TestSmithy) — deck { Province, Duchy, Estate, Gold }, 3 drawn, 1 left
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::DUCHY, id::ESTATE, id::GOLD]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::GOLD]));
    assert_eq!(g.turn.actions, 0, "1 - 1 + 0");
}

#[test]
fn dsim_smithy_only_two_cards_in_deck() {
    // C++ TEST_METHOD(TestSmithy) — "Only 2 cards in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert!(g.players[0].deck_counts().is_empty());
}

#[test]
fn dsim_smithy_only_one_card_in_deck() {
    // C++ TEST_METHOD(TestSmithy) — "Only 1 card in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].deck_counts().is_empty());
}

#[test]
fn dsim_smithy_empty_deck_and_discard() {
    // C++ TEST_METHOD(TestSmithy) — "No cards in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    play(&mut g, id::SMITHY);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn dsim_smithy_cards_in_discard_but_not_deck() {
    // C++ TEST_METHOD(TestSmithy) — "Cards in discard, but not in deck"
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_discard(&mut g, 0, &[id::ESTATE; 4]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]), "1 of the 4 shuffled Estates remains");
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn dsim_smithy_some_in_deck_but_not_enough() {
    // C++ TEST_METHOD(TestSmithy) — "Some cards in deck, but not enough" (spans a reshuffle)
    let mut g = new_state(&[id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    // (The C++ fixture used a Gold here; substituted with a Duchy since this engine auto-plays
    // any treasure left in the current player's hand the instant the turn rolls into the Buy
    // phase, which would otherwise move it out of `hand` before this assertion runs.)
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    set_discard(&mut g, 0, &[id::ESTATE; 3]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::DUCHY]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]));
    assert!(g.players[0].discard.is_empty());
}

// ===========================================================================
// Moat — C++ TestMoat
// ===========================================================================

#[test]
fn dsim_moat_basics() {
    let d = cards::def(id::MOAT);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::MOAT, cards::ACTION));
    assert!(cards::is(id::MOAT, cards::REACTION));
}

#[test]
fn dsim_moat_played_as_action_draws_two() {
    // C++ TEST_METHOD(TestMoat) — "Play as action"
    let mut g = new_state(&[id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MOAT]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 5]);
    play(&mut g, id::MOAT);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE; 3]));
    assert_eq!(g.turn.actions, 0, "1 - 1 + 0");
}

#[test]
fn dsim_moat_blocks_militia() {
    // C++ TEST_METHOD(TestMoat) — "Test a bunch of attacks" (Militia; Seahag/Saboteur/Torturer
    // aren't in Base Set 2E and are skipped).
    let mut g = new_state(&[id::MOAT, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    let victim_hand = [id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::MOAT];
    set_hand(&mut g, 1, &victim_hand);
    play(&mut g, id::MILITIA);
    assert_eq!(g.players[1].hand, counts_of(&victim_hand), "fully blocked, nothing discarded");
}

#[test]
fn dsim_moat_blocks_witch() {
    // C++ TEST_METHOD(TestMoat) — "Test a bunch of attacks" (Witch)
    let mut g = new_state(&[id::MOAT, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WITCH);
    assert!(!g.players[1].discard.has(id::CURSE), "Moat blocks the Curse");
    assert_eq!(g.players[0].hand.total(), 2, "the attacker still draws +2 Cards");
}

// ===========================================================================
// Market — C++ TestMarket
// ===========================================================================

#[test]
fn dsim_market_basics() {
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
fn dsim_market_normal_draw() {
    // C++ TEST_METHOD(TestMarket)
    let mut g = new_state(&[id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::MARKET);
    verify_market_played(&g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::DUCHY]));
}

#[test]
fn dsim_market_no_cards_left() {
    // C++ TEST_METHOD(TestMarket) — "No cards left"
    let mut g = new_state(&[id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    play(&mut g, id::MARKET);
    verify_market_played(&g);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn dsim_market_deck_empty_reshuffles_from_discard() {
    // C++ TEST_METHOD(TestMarket) — "Deck empty with cards in discard"
    let mut g = new_state(&[id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    set_discard(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::MARKET);
    verify_market_played(&g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_counts(), counts_of(&[id::ESTATE]));
    assert!(g.players[0].discard.is_empty());
}

// ===========================================================================
// Festival — C++ TestFestival
// ===========================================================================

#[test]
fn dsim_festival_gives_actions_buy_and_coin() {
    // C++ TEST_METHOD(TestFestival)
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

// ===========================================================================
// Laboratory — C++ TestLaboratory
// ===========================================================================

#[test]
fn dsim_laboratory_draws_two_keeps_action() {
    // C++ TEST_METHOD(TestLaboratory)
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

// ===========================================================================
// Chapel — C++ TestChapel
// ===========================================================================

#[test]
fn dsim_chapel_basics() {
    let d = cards::def(id::CHAPEL);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::CHAPEL, cards::ACTION));
}

#[test]
fn dsim_chapel_may_trash_nothing() {
    // C++ TEST_METHOD(TestChapel) — "Should trash nothing"
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL]);
    let next = play(&mut g, id::CHAPEL);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "nothing to trash, auto-resolved");
    assert!(g.trash.is_empty());
}

#[test]
fn dsim_chapel_trash_the_only_card_in_hand() {
    // C++ TEST_METHOD(TestChapel) — "Trash the only remaining card in hand"
    let mut g = new_state(&[id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER]);
    play(&mut g, id::CHAPEL);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
}

#[test]
fn dsim_chapel_trash_one_of_several() {
    // C++ TEST_METHOD(TestChapel) — "Trash a card in hand". Coppers are treasures, so the two
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
fn dsim_chapel_trash_two() {
    // C++ TEST_METHOD(TestChapel) — "Trash 2 cards" (see auto-play note above)
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
fn dsim_chapel_trash_three() {
    // C++ TEST_METHOD(TestChapel) — "Trash 3 cards"
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
fn dsim_chapel_trash_four_is_the_cap() {
    // C++ TEST_METHOD(TestChapel) — "Trash 4 cards" plus the "trash 4/5 when cheating" cases:
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

// ===========================================================================
// Cellar — C++ TestCellar
// ===========================================================================

#[test]
fn dsim_cellar_basics() {
    let d = cards::def(id::CELLAR);
    assert_eq!(d.cost, 2);
    assert!(cards::is(id::CELLAR, cards::ACTION));
}

#[test]
fn dsim_cellar_decline_has_no_effect() {
    // C++ TEST_METHOD(TestCellar) — "Chooses to not discard anything"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::ESTATE, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::CELLAR);
    assert!(choices(&g).contains(&Choice::Pass));
    pass(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1 (Cellar's own +1 Action)");
}

#[test]
fn dsim_cellar_empty_hand_offers_nothing() {
    // C++ TEST_METHOD(TestCellar) — "No cards in hand, shouldn't get asked to discard something"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR]);
    let next = play(&mut g, id::CELLAR);
    assert!(g.players[0].hand.is_empty());
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn dsim_cellar_discards_one_and_draws_one() {
    // C++ TEST_METHOD(TestCellar) — "Discard the only card in hand and draw a single card"
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
fn dsim_cellar_discard_and_get_it_straight_back_via_reshuffle() {
    // C++ TEST_METHOD(TestCellar) — "Discard the only card in hand and only one in the deck
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
fn dsim_cellar_discard_only_three_of_four() {
    // C++ TEST_METHOD(TestCellar) — "Discard only 3 cards even though more are in hand"
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
fn dsim_cellar_discard_all_four() {
    // C++ TEST_METHOD(TestCellar) — "Discard all 4 cards"
    let mut g = new_state(&[id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 4]);
    play(&mut g, id::CELLAR);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::DUCHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE; 4]));
    assert!(g.players[0].deck_counts().is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY; 4]));
}

// ===========================================================================
// Workshop — C++ TestWorkshop
// ===========================================================================

#[test]
fn dsim_workshop_basics() {
    let d = cards::def(id::WORKSHOP);
    assert_eq!(d.cost, 3);
    assert!(cards::is(id::WORKSHOP, cards::ACTION));
}

#[test]
fn dsim_workshop_gains_from_kingdom() {
    // C++ TEST_METHOD(TestWorkshop) — "Gain a card (should be a Smithy)"
    let mut g = new_state(&[id::WORKSHOP, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    assert!(choices(&g).contains(&Choice::Card(id::SMITHY)));
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].discard, counts_of(&[id::SMITHY]));
}

#[test]
fn dsim_workshop_falls_back_to_base_cards_when_kingdom_too_pricey() {
    // C++ TEST_METHOD(TestWorkshop) — "Nothing else on the board under 4, should not throw"
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
fn dsim_workshop_gain_is_mandatory() {
    // C++ TEST_METHOD(TestWorkshop) — "Must gain a card" (cheat check). In this engine a Gain
    // frame with any legal option never offers Pass, so a Pass attempt is simply illegal.
    let mut g = new_state(&[id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    assert!(!choices(&g).contains(&Choice::Pass));
    assert!(g.apply(Choice::Pass, &mut NoEvents).is_err());
}

// ===========================================================================
// Library — C++ TestLibrary
// ===========================================================================

#[test]
fn dsim_library_basics() {
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
fn dsim_library_no_actions_drawn_leaves_the_rest_in_deck() {
    // C++ TEST_METHOD(TestLibrary) — "Should draw 7 Estates, leaving the duchies in the deck"
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
fn dsim_library_keeps_actions_when_told_to() {
    // C++ TEST_METHOD(TestLibrary) — "Should draw all the 7 cards in the deck" (KeepXCard<true>).
    // Treasures are avoided in the fixture (Copper/Gold in the original) since they'd be
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
fn dsim_library_skips_actions_which_get_discarded_at_the_end() {
    // C++ TEST_METHOD(TestLibrary) — "Should draw 7 cards, but chooses to skip keeping the
    // actions" (KeepXCard<false>). Woodcutter (not in 2E) is replaced with Merchant.
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
fn dsim_library_not_enough_cards_skips_actions_and_stops_short() {
    // C++ TEST_METHOD(TestLibrary) — "Not enough in the deck, so should draw 5 cards" (skipping
    // actions). Gold is replaced with Curse in the fixture to avoid the Buy-phase auto-play
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
fn dsim_library_only_actions_left_gains_nothing_and_terminates() {
    // C++ TEST_METHOD(TestLibrary) — "Only actions left, should gain nothing (and not loop
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
fn dsim_library_nothing_in_deck_or_discard() {
    // C++ TEST_METHOD(TestLibrary) — "Nothing else in the deck, should gain nothing"
    let mut g = new_state(&[id::LIBRARY], 2);
    set_hand(&mut g, 0, &[id::LIBRARY]);
    let next = play(&mut g, id::LIBRARY);
    assert!(g.players[0].hand.is_empty());
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

// ===========================================================================
// Moneylender — C++ TestMoneyLender
// ===========================================================================

#[test]
fn dsim_moneylender_basics() {
    let d = cards::def(id::MONEYLENDER);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::MONEYLENDER, cards::ACTION));
}

#[test]
fn dsim_moneylender_optional_in_2e() {
    // 2E ADAPTATION: "You may trash a Copper from your hand for +$3" is optional. The C++
    // suite's `PlayFirstAction` strategy always trashed the Copper with no decision at all
    // ("MoneyLender doesn't ask the player to choose Coppers... there's no cheats to test
    // for"); this engine correctly offers Pass instead.
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::COPPER]);
    play(&mut g, id::MONEYLENDER);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass]);
}

#[test]
fn dsim_moneylender_single_copper_in_hand() {
    // C++ TEST_METHOD(TestMoneyLender) — "Single copper in hand"
    let mut g = new_state(&[id::MONEYLENDER], 2);
    set_hand(&mut g, 0, &[id::MONEYLENDER, id::COPPER]);
    play(&mut g, id::MONEYLENDER);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.turn.coins, 3);
}

#[test]
fn dsim_moneylender_copper_with_other_cards() {
    // C++ TEST_METHOD(TestMoneyLender) — "Single copper in hand with other cards". Silver/Gold
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
fn dsim_moneylender_multiple_coppers_trashes_only_one() {
    // C++ TEST_METHOD(TestMoneyLender) — "Multiple coppers, should only trash one". The
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
fn dsim_moneylender_no_coppers_offers_no_decision() {
    // C++ TEST_METHOD(TestMoneyLender) — "No coppers in hand, should not gain coins". Gold and
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

// ===========================================================================
// Remodel — C++ TestRemodel
// ===========================================================================

#[test]
fn dsim_remodel_basics() {
    let d = cards::def(id::REMODEL);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::REMODEL, cards::ACTION));
}

#[test]
fn dsim_remodel_estate_to_smithy_with_multiple_choices() {
    // C++ TEST_METHOD(TestRemodel) — "Remodel an estate into a Smithy from multiple choices".
    // Gold/Silver are treasures and auto-play into the Buy phase once Remodel resolves (Village
    // can't be played either, since 0 Actions remain) — check total ownership, not raw `hand`.
    let mut g = new_state(&[id::REMODEL, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE, id::GOLD, id::SILVER, id::VILLAGE]);
    play(&mut g, id::REMODEL);
    choose(&mut g, Choice::Card(id::ESTATE));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: cards::cost(id::ESTATE) + 2, filter: Filter::Any, dest: Dest::Discard, exact: false });
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].hand.get(id::VILLAGE), 1, "Village stays in hand (no Actions left to spend on it)");
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 1);
    assert!(g.players[0].discard.has(id::SMITHY));
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
}

#[test]
fn dsim_remodel_gold_to_province() {
    // C++ TEST_METHOD(TestRemodel) — "Gold to a Province". Silver/Copper auto-play into the Buy
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
fn dsim_remodel_upgrade_cap_is_trashed_cost_plus_two() {
    // C++ TEST_METHOD(TestRemodel) — "Remodel 5 point card with base cards, should only get
    // back cards 6 or less" (a Laboratory, cost 5, caps the gain at cost 7; Province at 8 is
    // unreachable).
    let mut g = new_state(&[id::REMODEL, id::LABORATORY], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::LABORATORY]);
    // Laboratory is the only card in hand, so its trash is the only legal choice and is
    // auto-applied by `play`; the very next decision is already the Gain.
    let step = play(&mut g, id::REMODEL);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Gain { .. }, .. })));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 7, filter: Filter::Any, dest: Dest::Discard, exact: false });
    assert!(choices(&g).contains(&Choice::Card(id::GOLD)), "Gold costs 6 <= 7");
    assert!(!choices(&g).contains(&Choice::Card(id::PROVINCE)), "Province costs 8 > 7");
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
    assert_eq!(g.trash, counts_of(&[id::LABORATORY]));
}

#[test]
fn dsim_remodel_nothing_in_hand_is_a_no_op() {
    // C++ TEST_METHOD(TestRemodel) — "Nothing else in hand, should not remodel anything"
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL]);
    let next = play(&mut g, id::REMODEL);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert!(g.trash.is_empty());
}

#[test]
fn dsim_remodel_trash_is_mandatory_when_something_is_available() {
    // C++ TEST_METHOD(TestRemodel) — "Must trash a card" (cheat check)
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE]);
    play(&mut g, id::REMODEL);
    assert!(!choices(&g).contains(&Choice::Pass));
}

#[test]
fn dsim_remodel_cannot_trash_a_card_not_in_hand() {
    // C++ TEST_METHOD(TestRemodel) — "Tried to trash a card not in the hand" (cheat check).
    // Two distinct candidates in hand so the trash pick isn't auto-applied as a forced single
    // choice (which would otherwise skip straight past this decision to the Gain that follows).
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::ESTATE, id::DUCHY]);
    play(&mut g, id::REMODEL);
    assert!(g.apply(Choice::Card(id::COPPER), &mut NoEvents).is_err(), "Copper isn't in hand");
}

#[test]
fn dsim_remodel_cannot_gain_a_card_not_offered() {
    // C++ TEST_METHOD(TestRemodel) — "Tried to gain a card not offered" (cheat check): a Copper
    // (cost 0) is trashed, capping the gain at cost 2, so Gold (cost 6) is illegal.
    let mut g = new_state(&[id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::COPPER]);
    play(&mut g, id::REMODEL);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.apply(Choice::Card(id::GOLD), &mut NoEvents).is_err());
}

// ===========================================================================
// Throne Room — C++ TestThroneRoom
// ===========================================================================

#[test]
fn dsim_throne_room_basics() {
    let d = cards::def(id::THRONE_ROOM);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::THRONE_ROOM, cards::ACTION));
}

#[test]
fn dsim_throne_room_is_optional_in_2e() {
    // 2E ADAPTATION: "You may play an Action card from your hand twice." The C++ suite's
    // "Must ThroneRoom a card" cheat check assumed 1E's mandatory wording; in 2E, Pass is a
    // legal choice even with a playable Action in hand.
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
fn dsim_throne_room_smithy() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a Smithy"
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
fn dsim_throne_room_village() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a Village"
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
fn dsim_throne_room_market() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a Market"
    let mut g = new_state(&[id::THRONE_ROOM, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MARKET]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MARKET));
    verify_throne_room_played(&g, 2, 3, 2);
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn dsim_throne_room_remodel() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a Remodel"
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
fn dsim_throne_room_witch() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a Witch"
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
fn dsim_throne_room_militia() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a Militia"
    let mut g = new_state(&[id::THRONE_ROOM, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MILITIA]);
    set_hand(&mut g, 1, &[id::ESTATE; 5]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MILITIA));
    verify_throne_room_played(&g, 0, 1, 4);
    assert_eq!(g.players[1].hand.total(), 3, "discarded down to 3, doesn't matter that Militia hit twice");
}

#[test]
fn dsim_throne_room_moneylender() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a MoneyLender"
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
fn dsim_throne_room_workshop() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a Workshop"
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
fn dsim_throne_room_mine_copper_to_gold() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom to Mine a single Copper into a Gold". Also
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
fn dsim_throne_room_on_throne_room_with_nothing_left_to_play() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a ThroneRoom with nothing to play, should
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
fn dsim_throne_room_on_throne_room_chains_two_more_actions() {
    // C++ TEST_METHOD(TestThroneRoom) — "ThroneRoom a ThroneRoom to play more actions". The
    // original used Woodcutter (not in 2E: +1 Buy +2 Coins, no Actions); replaced here with
    // Festival (+2 Actions +1 Buy +2 Coins), so the expected action count differs from the
    // C++ source (4, not 0) — the buys/coins totals happen to still match.
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

// ===========================================================================
// Mine — C++ TestMine
// ===========================================================================

#[test]
fn dsim_mine_basics() {
    let d = cards::def(id::MINE);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::MINE, cards::ACTION));
}

#[test]
fn dsim_mine_is_optional_and_gains_to_hand_in_2e() {
    // 2E ADAPTATION: "You may trash a Treasure... gain a Treasure to your hand costing up to
    // $3 more." The C++ suite's "Must trash a treasure" cheat check assumed 1E's mandatory
    // wording and a discard destination; 2E offers Pass, and (checked separately below) the
    // gain lands in hand.
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::GOLD]);
    play(&mut g, id::MINE);
    assert_eq!(choices(&g), vec![Choice::Card(id::GOLD), Choice::Pass]);
}

#[test]
fn dsim_mine_copper_to_silver_gains_to_hand() {
    // C++ TEST_METHOD(TestMine) — "Copper to a Silver"
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
fn dsim_mine_silver_to_gold() {
    // C++ TEST_METHOD(TestMine) — "Silver to a Gold"
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE, id::SILVER]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.trash, counts_of(&[id::SILVER]));
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1);
}

#[test]
fn dsim_mine_gold_to_gold() {
    // C++ TEST_METHOD(TestMine) — "Gold to a Gold"
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
fn dsim_mine_gold_to_copper_downgrade_is_legal() {
    // C++ TEST_METHOD(TestMine) — "Gold to a Copper". Real Mine has no lower bound on the
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
fn dsim_mine_empty_hand_is_a_no_op() {
    // C++ TEST_METHOD(TestMine) — "Mine with empty hand"
    let mut g = new_state(&[id::MINE], 2);
    set_hand(&mut g, 0, &[id::MINE]);
    let next = play(&mut g, id::MINE);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
}

#[test]
fn dsim_mine_no_treasures_in_hand_is_a_no_op() {
    // C++ TEST_METHOD(TestMine) — "Mine with cards in hand, but no treasures"
    let mut g = new_state(&[id::MINE, id::VILLAGE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MINE, id::VILLAGE, id::CURSE, id::MOAT]);
    let next = play(&mut g, id::MINE);
    // Mine grants no Actions itself, so even though Village/Moat remain in hand, 0 Actions
    // remain to play them with — the turn goes straight to the Buy phase.
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::CURSE, id::MOAT]));
}

// ===========================================================================
// Council Room — C++ TestCouncilRoom
// ===========================================================================

#[test]
fn dsim_council_room_basics() {
    let d = cards::def(id::COUNCIL_ROOM);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::COUNCIL_ROOM, cards::ACTION));
}

#[test]
fn dsim_council_room_one_other_player() {
    // C++ TEST_METHOD(TestCouncilRoom) — "One other player"
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
fn dsim_council_room_current_player_in_the_middle() {
    // C++ TEST_METHOD(TestCouncilRoom) — "Two other players, current is in the middle"
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
fn dsim_council_room_neither_player_has_enough_cards_to_draw() {
    // C++ TEST_METHOD(TestCouncilRoom) — "Neither player has enough cards to draw"
    let mut g = new_state(&[id::COUNCIL_ROOM], 2);
    set_hand(&mut g, 0, &[id::COUNCIL_ROOM]);
    set_hand(&mut g, 1, &[id::ESTATE; 5]);
    let next = play(&mut g, id::COUNCIL_ROOM);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE; 5]));
}

// ===========================================================================
// Militia — C++ TestMilitia
// ===========================================================================

#[test]
fn dsim_militia_basics() {
    let d = cards::def(id::MILITIA);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::MILITIA, cards::ACTION));
    assert!(cards::is(id::MILITIA, cards::ATTACK));
}

#[test]
fn dsim_militia_multiple_players_varying_hand_sizes() {
    // C++ TEST_METHOD(TestMilitia) — "Militia against other players, some with more and some
    // with less than 3 cards" (a representative subset of the C++ suite's per-player loop).
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
fn dsim_militia_hand_of_three_or_fewer_is_untouched() {
    // C++ TEST_METHOD(TestMilitia) — implicit in the loop above (initialHandSize <= 3)
    let mut g = new_state(&[id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER]);
    let next = play(&mut g, id::MILITIA);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER]));
}

// ===========================================================================
// Witch — C++ TestWitch
// ===========================================================================

#[test]
fn dsim_witch_basics() {
    let d = cards::def(id::WITCH);
    assert_eq!(d.cost, 5);
    assert!(cards::is(id::WITCH, cards::ACTION));
    assert!(cards::is(id::WITCH, cards::ATTACK));
}

#[test]
fn dsim_witch_two_other_players() {
    // C++ TEST_METHOD(TestWitch) — "Witch with 2 other players"
    let mut g = new_state(&[id::WITCH], 3);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::WITCH);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    assert_eq!(g.players[2].discard.get(id::CURSE), 1);
}

#[test]
fn dsim_witch_no_curses_left() {
    // C++ TEST_METHOD(TestWitch) — "No more curses"
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
fn dsim_witch_not_enough_curses_leftmost_players_get_priority() {
    // C++ TEST_METHOD(TestWitch) — "Not enough curses"
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

// ===========================================================================
// Bureaucrat — C++ TestBureaucrat
// ===========================================================================

#[test]
fn dsim_bureaucrat_basics() {
    let d = cards::def(id::BUREAUCRAT);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::BUREAUCRAT, cards::ACTION));
    assert!(cards::is(id::BUREAUCRAT, cards::ATTACK));
}

#[test]
fn dsim_bureaucrat_no_cards_in_other_players_hand() {
    // C++ TEST_METHOD(TestBureaucrat) — "No cards in other player's hand"
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT, id::ESTATE]);
    set_hand(&mut g, 1, &[]);
    let next = play(&mut g, id::BUREAUCRAT);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no Actions left in hand");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn dsim_bureaucrat_multiple_opponents_each_choose_a_victory_card() {
    // C++ TEST_METHOD(TestBureaucrat) — "Other players have estates, last player has Moat"
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
fn dsim_bureaucrat_no_silver_left_still_forces_the_topdeck() {
    // C++ TEST_METHOD(TestBureaucrat) — "No silvers left"
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
fn dsim_bureaucrat_victim_with_no_victory_card_is_untouched() {
    // C++ TEST_METHOD(TestBureaucrat) — matches the reveal-only path also exercised via Moat
    let mut g = new_state(&[id::BUREAUCRAT], 2);
    set_hand(&mut g, 0, &[id::BUREAUCRAT]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::SILVER]);
    let next = play(&mut g, id::BUREAUCRAT);
    assert!(matches!(next, Step::Decision(Decision { kind: DecisionKind::Buy, .. })), "no Actions left in hand");
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::COPPER, id::SILVER]));
}
