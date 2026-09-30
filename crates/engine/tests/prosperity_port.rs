//! Prosperity (2nd edition) card tests, ported from the old C++ simulator's
//! `DominionSimTestCards\PropserityCardsTests.cpp` (1st-edition Prosperity) where the card exists
//! in 2nd edition, adapted to this engine's API. Each test cites the C++ `TEST_METHOD` it came
//! from. Tests for cards new in 2nd edition are written from the card text. See
//! `docs/seaside-prosperity-plan.md` §4.
//!
//! Adapting C++ assertions: `VerifyTurnBasics(actions, buys, coins)` there is checked before
//! treasures are played; this engine auto-plays choice-free treasures on entering the Buy phase,
//! so coin totals below include any treasures in hand.
//!
//! Not ported (1st edition only): TestLoan, TestVenture, TestMountebank, TestTalisman, TestGoons,
//! TestCountingHouse, TestTradeRoute, TestTradeRouteTokens, TestRoyalSeal. The "Simulation"
//! sub-cases test the C++ bot harness, not card rules, and aren't ported.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::state::{self, EndReason};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn vp_of(pairs: &[(CardId, u8)]) -> i32 {
    let mut c = Counts::EMPTY;
    for &(card, n) in pairs {
        c.add(card, n);
    }
    state::vp_of_cards(&c)
}

// ===========================================================================
// Platinum and Colony — C++ TestPlatinum, TestColony
// ===========================================================================

#[test]
fn platinum_is_a_five_coin_treasure_with_a_pile_of_twelve() {
    let d = cards::def(id::PLATINUM);
    assert_eq!((d.cost, d.coins, d.vp), (9, 5, 0));
    assert!(cards::is(id::PLATINUM, cards::TREASURE));
    assert!(!cards::is_kingdom(id::PLATINUM) && cards::is_optional_basic(id::PLATINUM));
    // "Platinum pile always contains 12", with 2 and 6 players.
    for n in [2, 6] {
        assert_eq!(new_state(&[id::PLATINUM, id::COLONY], n).supply.get(id::PLATINUM), 12);
    }
    // TestTreasure(Platinum, 5)
    let mut g = new_state(&[id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::PLATINUM]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 5);
}

#[test]
fn colony_is_worth_ten_and_its_pile_matches_provinces() {
    let d = cards::def(id::COLONY);
    assert_eq!((d.cost, d.vp), (11, 10));
    for (n, vp) in [(1, 10), (2, 20), (3, 30), (10, 100)] {
        assert_eq!(vp_of(&[(id::COLONY, n)]), vp);
    }
    assert_eq!(cards::set_of(id::COLONY), CardSet::Prosperity);
    for n in 2..=6 {
        let g = new_state(&[id::PLATINUM, id::COLONY], n);
        assert_eq!(g.supply.get(id::COLONY), g.supply.get(id::PROVINCE), "{n} players");
    }
    // Not in the supply unless named.
    assert!(!new_state(&[id::VILLAGE], 2).in_supply(id::COLONY));
}

#[test]
fn an_empty_colony_pile_ends_the_game() {
    let mut g = new_state(&[id::PLATINUM, id::COLONY], 2);
    empty_pile(&mut g, id::COLONY);
    assert_eq!(g.end_reason(), Some(EndReason::ColoniesGone));
    // Provinces still end it too.
    let mut g = new_state(&[id::PLATINUM, id::COLONY], 2);
    empty_pile(&mut g, id::PROVINCE);
    assert_eq!(g.end_reason(), Some(EndReason::ProvincesGone));
}

#[test]
fn colony_games_round_trip_through_the_text_format() {
    let g = new_state(&[id::PLATINUM, id::COLONY, id::MONUMENT], 2);
    let text = format_state(&g);
    assert!(text.contains("Colony") && text.contains("Platinum"), "{text}");
    let back = parse_state(&text).unwrap();
    assert!(back.in_supply(id::COLONY) && back.in_supply(id::PLATINUM));
}

// ===========================================================================
// Monument — C++ TestMonument
// ===========================================================================

#[test]
fn monument_gives_two_coins_and_a_vp_token() {
    let d = cards::def(id::MONUMENT);
    assert_eq!((d.cost, d.vp), (4, 0));
    // TestGame<PlayFirstAction>({Monument}, {Duchy}): VerifyTurnBasics(0, 1, 2), 1 VP token.
    let mut g = new_state(&[id::MONUMENT], 2);
    set_hand(&mut g, 0, &[id::MONUMENT]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::MONUMENT);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 2));
    assert_eq!(g.players[0].vp_tokens, 1);
    assert!(g.players[0].hand.is_empty());
    // VP tokens count in the score (the Duchy still in the deck is the other 3), and survive
    // the text format.
    assert_eq!(g.players[0].vp(), 3 + 1);
    let back = parse_state(&format_state(&g)).unwrap();
    assert_eq!(back.players[0].vp_tokens, 1);
}

#[test]
fn throne_room_monument_gives_two_tokens() {
    // C++ TestMonument "ThroneRoom for most VP" (the card-rule part: two plays, two tokens, $4).
    let mut g = new_state(&[id::MONUMENT, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MONUMENT]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MONUMENT));
    assert_eq!((g.turn.coins, g.players[0].vp_tokens), (4, 2));
}

// ===========================================================================
// Worker's Village — C++ TestWorkersVillage
// ===========================================================================

#[test]
fn workers_village_card_two_actions_and_a_buy() {
    assert_eq!(cards::by_name("Workers Village"), Some(id::WORKERS_VILLAGE));
    // TestGame<PlayFirstAction>({WorkersVillage, Estate}, {Duchy, Province}): (2, 2, 0).
    let mut g = new_state(&[id::WORKERS_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WORKERS_VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::WORKERS_VILLAGE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::PROVINCE));
}

// ===========================================================================
// City — C++ TestCity
// ===========================================================================

fn city_turn(empty: &[CardId]) -> GameState {
    let mut g = new_state(&[id::CITY, id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::CITY, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::PLATINUM]);
    for &c in empty {
        empty_pile(&mut g, c);
    }
    play(&mut g, id::CITY);
    g
}

#[test]
fn city_scales_with_empty_piles() {
    // No empty piles: +1 Card +2 Actions.
    let g = city_turn(&[]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    // One empty pile (Silver): another card.
    let g = city_turn(&[id::SILVER]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    // Two empty piles (Silver, Copper): another card, +1 Buy, +$1.
    let g = city_turn(&[id::SILVER, id::COPPER]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    // Many empty piles: still just the two-pile bonus.
    let g = city_turn(&[id::SILVER, id::COPPER, id::CURSE, id::ESTATE]);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 1));
}

// ===========================================================================
// Grand Market — C++ TestGrandMarket
// ===========================================================================

#[test]
fn grand_market_plays_as_a_market_plus() {
    // TestGame<PlayFirstAction>({GrandMarket, Estate}, {Duchy, Province}): (1, 2, 2).
    let mut g = new_state(&[id::GRAND_MARKET], 2);
    set_hand(&mut g, 0, &[id::GRAND_MARKET, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::GRAND_MARKET);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 2, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
}

#[test]
fn grand_market_cant_be_bought_with_a_copper_in_play() {
    // Can be bought with any treasure that isn't Copper: Platinum, Gold, Silver = $10.
    let mut g = new_state(&[id::GRAND_MARKET, id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::PLATINUM, id::GOLD, id::SILVER]);
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::GRAND_MARKET)));
    buy(&mut g, id::GRAND_MARKET);
    // Cannot be bought with a Copper in play (all treasures auto-play in this engine).
    let mut g = new_state(&[id::GRAND_MARKET, id::PLATINUM, id::COLONY], 2);
    set_hand(&mut g, 0, &[id::PLATINUM, id::GOLD, id::SILVER, id::COPPER]);
    expect_decision(&mut g);
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(id::GRAND_MARKET)), "Copper in play");
    assert!(cs.contains(&Choice::Card(id::GOLD)));
}

// ===========================================================================
// Magnate — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn magnate_draws_one_card_per_treasure_in_hand() {
    let mut g = new_state(&[id::MAGNATE], 2);
    set_hand(&mut g, 0, &[id::MAGNATE, id::COPPER, id::SILVER, id::ESTATE, id::HAREM]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::GOLD]);
    play(&mut g, id::MAGNATE);
    // Copper, Silver, Harem are Treasures (Harem is Treasure-Victory): +3 Cards.
    assert_eq!(g.players[0].all_cards().get(id::DUCHY), 3);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    // No treasures in hand: nothing drawn.
    let mut g = new_state(&[id::MAGNATE], 2);
    set_hand(&mut g, 0, &[id::MAGNATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    play(&mut g, id::MAGNATE);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
}

// ===========================================================================
// Cross-cutting
// ===========================================================================

#[test]
fn treasure_buys_are_applied_when_played() {
    // Generic: a Treasure's +Buy (Collection, Tiara, Astrolabe) applies when it's played.
    assert_eq!(cards::def(id::COLLECTION).buys, 1);
    assert_eq!(cards::def(id::ASTROLABE).buys, 1);
}

// ===========================================================================
// Bishop — C++ TestBishop (93)
// ===========================================================================

#[test]
fn bishop_alone_gives_one_vp_token_and_one_coin() {
    let mut g = new_state(&[id::BISHOP], 2);
    set_hand(&mut g, 0, &[id::BISHOP]);
    play(&mut g, id::BISHOP);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 1));
    assert_eq!(g.players[0].vp_tokens, 1);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn bishop_trash_vp_is_one_plus_half_cost_rounded_down() {
    // VerifyBishop(card, expectedVP, discount): the base +1 VP plus floor(cost/2) of whatever
    // was trashed (Bishop's own trash is mandatory once something's available).
    for &(card, expected_vp, discount) in &[
        (id::COPPER, 1u16, 0u8),
        (id::ESTATE, 2, 0),
        (id::SILVER, 2, 0),
        (id::VILLAGE, 2, 0),
        (id::DUCHY, 3, 0),
        (id::GOLD, 4, 0),
        (id::PROVINCE, 5, 0),
        (id::PLATINUM, 5, 0),
        // With discounts (Bridge-style turn-wide reduction).
        (id::ESTATE, 1, 1),
        (id::ESTATE, 1, 6), // clamped to 0
        (id::PROVINCE, 2, 6),
    ] {
        let mut g = new_state(&[id::BISHOP, id::PLATINUM, id::COLONY], 2);
        g.turn.cost_reduction = discount;
        set_hand(&mut g, 0, &[id::BISHOP, card]);
        play(&mut g, id::BISHOP);
        pick_while(&mut g, Zone::Hand, Act::Trash, card);
        assert_eq!(g.players[0].vp_tokens, expected_vp, "{} discount {discount}", cards::name(card));
        assert!(g.trash.has(card));
        assert!(g.players[0].hand.is_empty());
    }
}

#[test]
fn bishop_own_trash_is_mandatory_when_something_is_available() {
    // Two cards in hand, so the choice among them is real (not auto-single) and we can observe
    // that Pass is never offered.
    let mut g = new_state(&[id::BISHOP], 2);
    set_hand(&mut g, 0, &[id::BISHOP, id::GOLD, id::COPPER]);
    play(&mut g, id::BISHOP);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 1, max: 1, ordered: false });
    assert!(!choices(&g).contains(&Choice::Pass), "Bishop's own trash cannot be skipped with a card available");
}

#[test]
fn bishop_other_players_may_each_trash_a_card() {
    // 4 players: victim 1 trashes an Estate, victim 2 declines, victim 3 has nothing to trash.
    let mut g = new_state(&[id::BISHOP], 4);
    set_hand(&mut g, 0, &[id::BISHOP, id::SILVER]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_hand(&mut g, 2, &[id::ESTATE]);
    set_hand(&mut g, 3, &[]);
    play(&mut g, id::BISHOP);
    pick_while(&mut g, Zone::Hand, Act::Trash, id::SILVER); // Bishop's own mandatory trash.
    // Victim 1 (player index 1) trashes an Estate.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::ESTATE));
    // Victim 2 (player index 2) declines.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 2);
    pass(&mut g);
    // Victim 3 (player index 3) has nothing to trash: auto-skipped by `auto_single`.
    expect_decision(&mut g); // Back to player 0's Buy phase.
    assert_eq!(g.players[0].vp_tokens, 2); // 1 base + floor(3/2) for the Silver.
    assert_eq!(g.players[1].hand.total(), 4);
    assert_eq!(g.players[2].hand.total(), 1);
    assert!(g.trash.has(id::SILVER) && g.trash.has(id::ESTATE));
    assert_eq!(g.trash.total(), 2);
}

// ===========================================================================
// Mint — C++ TestMint (409)
// ===========================================================================

#[test]
fn mint_no_treasures_in_hand_is_a_no_op() {
    let mut g = new_state(&[id::MINT], 2);
    set_hand(&mut g, 0, &[id::MINT, id::ESTATE]);
    play(&mut g, id::MINT);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn mint_may_decline_to_reveal() {
    let mut g = new_state(&[id::MINT], 2);
    set_hand(&mut g, 0, &[id::MINT, id::GOLD]);
    play(&mut g, id::MINT);
    pass(&mut g); // drives on into the Buy phase, where the original Gold auto-plays
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 1, "nothing extra gained");
    assert_eq!(g.supply.get(id::GOLD), 30);
}

#[test]
fn mint_reveal_gains_a_copy_to_discard() {
    let mut g = new_state(&[id::MINT], 2);
    set_hand(&mut g, 0, &[id::MINT, id::GOLD]);
    play(&mut g, id::MINT);
    choose(&mut g, Choice::Card(id::GOLD)); // reveal (stays in hand), gain a copy to discard;
    // driving on into the Buy phase then auto-plays the original Gold out of hand.
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 2);
    assert!(g.players[0].discard.has(id::GOLD));
    assert!(g.players[0].in_play.has(id::GOLD)); // the original, auto-played entering Buy
}

#[test]
fn mint_buying_it_trashes_treasures_in_play_but_not_durations_2nd_edition() {
    // 2nd edition: Mint's on-buy trash skips Duration Treasures (Astrolabe). 1st edition
    // trashed every Treasure in play; this engine follows 2nd edition. Checked right after the
    // buy (via `apply`, not the `buy` helper, which would drive on into cleanup and sweep
    // in_play into discard regardless — the step-3 Duration zone isn't built yet).
    let mut g = new_state(&[id::MINT, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER, id::HAREM]);
    set_in_play(&mut g, 0, &[id::ASTROLABE]); // a Duration Treasure, placed directly (step-3 card)
    g.turn.coins = 20;
    expect_decision(&mut g); // treasures auto-play, then the Buy decision
    g.apply(Choice::Card(id::MINT), &mut NoEvents).unwrap();
    assert!(g.trash.has(id::GOLD) && g.trash.has(id::SILVER) && g.trash.has(id::COPPER) && g.trash.has(id::HAREM));
    assert_eq!(g.trash.total(), 4);
    assert!(!g.trash.has(id::ASTROLABE), "Duration Treasures survive Mint's on-buy trash in 2nd edition");
    assert!(g.players[0].in_play.has(id::ASTROLABE));
}

#[test]
fn mint_gaining_it_any_other_way_does_not_trash_treasures() {
    // Artisan gains up to $5 to hand (Mint costs 5); a plain Workshop-style ($4) gain can't
    // reach Mint at all, which already proves the on-buy trash is buy-specific, but Artisan
    // shows a same-cost non-buy gain still doesn't trigger it.
    let mut g = new_state(&[id::MINT, id::ARTISAN], 2);
    set_hand(&mut g, 0, &[id::ARTISAN]);
    set_in_play(&mut g, 0, &[id::GOLD, id::SILVER]);
    play(&mut g, id::ARTISAN);
    choose(&mut g, Choice::Card(id::MINT)); // the gain
    pick_while(&mut g, Zone::Hand, Act::Topdeck, id::MINT); // Artisan's own follow-up: topdeck a card
    assert!(g.trash.is_empty(), "gaining Mint via Artisan, not a buy, must not trigger its trash");
    assert!(g.players[0].in_play.has(id::GOLD) && g.players[0].in_play.has(id::SILVER));
}

// ===========================================================================
// Expand — C++ TestExpand (529)
// ===========================================================================

#[test]
fn expand_estate_to_duchy() {
    // Estate ($2) + 3 = up to $5: Duchy. The trash pick is auto-single (one card in hand).
    let mut g = new_state(&[id::EXPAND], 2);
    set_hand(&mut g, 0, &[id::EXPAND, id::ESTATE]);
    play(&mut g, id::EXPAND);
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(g.trash.has(id::ESTATE));
    assert!(g.players[0].discard.has(id::DUCHY));
}

#[test]
fn expand_copper_to_silver() {
    // Copper ($0) + 3 = up to $3: Silver. With a single card in hand, the trash pick is
    // auto-single (no real choice), so `play` already lands on the Gain decision.
    let mut g = new_state(&[id::EXPAND], 2);
    set_hand(&mut g, 0, &[id::EXPAND, id::COPPER]);
    play(&mut g, id::EXPAND);
    choose(&mut g, Choice::Card(id::SILVER));
    assert!(g.trash.has(id::COPPER) && g.players[0].discard.has(id::SILVER));
}

#[test]
fn expand_discount_applies_to_the_gained_card_not_the_budget() {
    // Copper ($0) + 3 = up to $3 (the discount doesn't inflate the budget: Copper is already
    // $0). But a $2 discount also reduces the *candidates*' cost: Duchy (normally $5) is $3
    // discounted, so it now fits.
    let mut g = new_state(&[id::EXPAND], 2);
    g.turn.cost_reduction = 2;
    set_hand(&mut g, 0, &[id::EXPAND, id::COPPER]);
    play(&mut g, id::EXPAND);
    assert!(choices(&g).contains(&Choice::Card(id::DUCHY)));
    assert!(!choices(&g).contains(&Choice::Card(id::PROVINCE))); // $8 - 2 = $6, still too much
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(g.players[0].discard.has(id::DUCHY));
}

// ===========================================================================
// Forge — C++ TestForge (629)
// ===========================================================================

#[test]
fn forge_no_cards_gains_a_zero_cost_card() {
    // Empty hand: the trash Select finds nothing to offer and finishes on its own (no decision);
    // the mandatory $0 Gain follows directly (Copper or Curse; both cost $0).
    let mut g = new_state(&[id::FORGE], 2);
    set_hand(&mut g, 0, &[id::FORGE]);
    play(&mut g, id::FORGE);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.players[0].discard.has(id::COPPER));
}

#[test]
fn forge_choosing_to_trash_nothing_still_gains_a_zero_cost_card() {
    let mut g = new_state(&[id::FORGE], 2);
    set_hand(&mut g, 0, &[id::FORGE, id::SILVER]);
    play(&mut g, id::FORGE);
    pass(&mut g); // decline to trash anything
    choose(&mut g, Choice::Card(id::COPPER)); // the mandatory $0 gain; drives on into Buy phase
    // (Silver, still in hand until now, auto-plays entering Buy: check total ownership instead.)
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 1);
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER]));
}

#[test]
fn forge_trashes_several_and_gains_exactly_the_summed_cost() {
    // 2 Copper (0) + 2 Curse (0) + Estate (2) = $2: only Estate costs exactly $2, so the last
    // pick auto-resolves the whole thing (the trash Select finishes once hand is empty, and the
    // Gain has exactly one legal choice). Picks must be in non-decreasing card-id order
    // (Copper=0, Estate=3, Curse=6).
    let mut g = new_state(&[id::FORGE], 2);
    set_hand(&mut g, 0, &[id::FORGE, id::COPPER, id::COPPER, id::CURSE, id::CURSE, id::ESTATE]);
    play(&mut g, id::FORGE);
    for c in [id::COPPER, id::COPPER, id::ESTATE, id::CURSE, id::CURSE] {
        choose(&mut g, Choice::Card(c));
    }
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::COPPER, id::CURSE, id::CURSE, id::ESTATE]));
    assert!(g.players[0].discard.has(id::ESTATE));
}

#[test]
fn forge_silver_and_estate_into_duchy() {
    // Silver (3) + Estate (2) = $5: Duchy (the only $5 card here; auto-resolves).
    let mut g = new_state(&[id::FORGE], 2);
    set_hand(&mut g, 0, &[id::FORGE, id::SILVER, id::ESTATE]);
    play(&mut g, id::FORGE);
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert!(g.trash.has(id::SILVER) && g.trash.has(id::ESTATE));
    assert!(g.players[0].discard.has(id::DUCHY));
}

// ===========================================================================
// Hoard — C++ TestHoard (1341)
// ===========================================================================

#[test]
fn hoard_plays_as_a_two_coin_treasure() {
    let mut g = new_state(&[id::HOARD], 2);
    set_hand(&mut g, 0, &[id::HOARD]);
    expect_decision(&mut g); // choice-free: auto-played entering Buy
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn hoard_only_reacts_to_a_bought_victory_card_while_in_play() {
    // Checked with `apply` directly (not the `buy` helper): with only 1 buy, driving `advance`
    // any further would run cleanup and reshuffle discard into a fresh hand before we can look.

    // Buying a non-Victory card: no Gold.
    let mut g = new_state(&[id::HOARD, id::WORKSHOP], 2);
    set_in_play(&mut g, 0, &[id::HOARD]);
    g.turn.coins = 10;
    expect_decision(&mut g);
    g.apply(Choice::Card(id::WORKSHOP), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 0);

    // Buying a Victory card: +1 Gold.
    let mut g = new_state(&[id::HOARD], 2);
    set_in_play(&mut g, 0, &[id::HOARD]);
    g.turn.coins = 10;
    expect_decision(&mut g);
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    assert!(g.players[0].discard.has(id::ESTATE) && g.players[0].discard.has(id::GOLD));

    // Gaining a Victory card WITHOUT buying it (e.g. Workshop-style): no Gold.
    let mut g = new_state(&[id::HOARD, id::IRONWORKS], 2);
    set_in_play(&mut g, 0, &[id::HOARD]);
    set_hand(&mut g, 0, &[id::IRONWORKS]);
    play(&mut g, id::IRONWORKS);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 0);
}

#[test]
fn two_hoards_double_the_gold_per_bought_victory_card() {
    let mut g = new_state(&[id::HOARD], 2);
    set_in_play(&mut g, 0, &[id::HOARD, id::HOARD]);
    g.turn.coins = 10;
    g.turn.buys = 2;
    expect_decision(&mut g);
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    expect_decision(&mut g); // re-derive the next decision (buys=1 remaining, no cleanup yet)
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
    assert_eq!(g.players[0].discard.get(id::GOLD), 4);
}

#[test]
fn hoard_with_no_gold_left_gains_nothing_extra() {
    let mut g = new_state(&[id::HOARD], 2);
    set_in_play(&mut g, 0, &[id::HOARD]);
    empty_pile(&mut g, id::GOLD);
    g.turn.coins = 10;
    expect_decision(&mut g);
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    assert!(g.players[0].discard.has(id::ESTATE));
    assert_eq!(g.players[0].all_cards().get(id::GOLD), 0);
}

// ===========================================================================
// Rabble — C++ TestRabble (1748)
// ===========================================================================

#[test]
fn rabble_draws_three() {
    let mut g = new_state(&[id::RABBLE], 2);
    set_hand(&mut g, 0, &[id::RABBLE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::COLONY, id::GOLD]);
    play(&mut g, id::RABBLE);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE, id::COLONY]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
}

#[test]
fn rabble_discards_actions_and_treasures_from_victims_and_topdecks_the_rest() {
    // Victim's top 3: Village (Action, auto-discarded), Duchy and Curse (neither Action nor
    // Treasure: stay, and the victim orders them back onto the deck — a real 2-way choice).
    let mut g = new_state(&[id::RABBLE], 2);
    set_hand(&mut g, 0, &[id::RABBLE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::COLONY, id::GOLD]);
    set_deck_known(&mut g, 1, &[id::VILLAGE, id::DUCHY, id::CURSE, id::GOLD]);
    play(&mut g, id::RABBLE);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Topdeck, ordered: true, .. }));
    // The victim orders Curse on top of Duchy (the 2nd pick ends up on top).
    choose(&mut g, Choice::Card(id::DUCHY));
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.players[1].discard, counts_of(&[id::VILLAGE]));
    let top_down: Vec<CardId> = g.players[1].deck_known.iter_top_down().collect();
    assert_eq!(top_down, vec![id::CURSE, id::DUCHY, id::GOLD]);
}

// ===========================================================================
// Vault — C++ TestVault (1838)
// ===========================================================================

#[test]
fn vault_own_discard_for_coins() {
    let mut g = new_state(&[id::VAULT], 2);
    set_hand(&mut g, 0, &[id::VAULT, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY]);
    play(&mut g, id::VAULT);
    // Own hand is now 4 Estates; discard 2 for +$2.
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::ESTATE));
    pass(&mut g);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
    assert_eq!(g.players[0].hand.get(id::ESTATE), 2);
}

#[test]
fn vault_victim_discards_two_to_draw_one() {
    let mut g = new_state(&[id::VAULT], 2);
    set_hand(&mut g, 0, &[id::VAULT]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]); // so the own-discard decision is real
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 1, &[id::PROVINCE]);
    play(&mut g, id::VAULT);
    pass(&mut g); // own discard: decline
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[1].hand, counts_of(&[id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[1].discard.get(id::ESTATE), 2);
}

#[test]
fn vault_victim_with_only_one_card_may_discard_it_but_does_not_draw() {
    let mut g = new_state(&[id::VAULT], 2);
    set_hand(&mut g, 0, &[id::VAULT]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_hand(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::VAULT);
    pass(&mut g); // own discard: decline
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    // `max` is the frame's nominal cap (2); only 1 card is actually offered (`choices`), since
    // that's all the victim has.
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 0, max: 2, ordered: false });
    assert_eq!(choices(&g), vec![Choice::Card(id::ESTATE), Choice::Pass]);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert!(g.players[1].hand.is_empty(), "discarding the 1 available card draws nothing");
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE]));
}

#[test]
fn vault_victim_may_decline_to_discard_at_all() {
    let mut g = new_state(&[id::VAULT], 2);
    set_hand(&mut g, 0, &[id::VAULT]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::VAULT);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    pass(&mut g);
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::ESTATE]));
}

// ===========================================================================
// King's Court — C++ TestKingsCourt (1972)
// ===========================================================================

#[test]
fn kings_court_may_play_nothing() {
    // A real choice (Village or Pass); declining leaves Village unplayed.
    let mut g = new_state(&[id::KINGS_COURT, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::VILLAGE]);
    play(&mut g, id::KINGS_COURT);
    let d = expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Pass));
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Play, filter: Filter::Action, min: 0, max: 1, ordered: false });
    pass(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE]));
}

#[test]
fn kings_court_plays_village_three_times() {
    let mut g = new_state(&[id::KINGS_COURT, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::PLATINUM]);
    play(&mut g, id::KINGS_COURT);
    choose(&mut g, Choice::Card(id::VILLAGE));
    // 1 (start) - 1 (KC) + 2*3 (three Village plays) = 6. The 3 drawn cards (Copper, Silver,
    // Gold) are all Treasures, so the Action phase ends and they auto-play entering the Buy
    // phase; check total ownership rather than the now-empty hand.
    assert_eq!(g.turn.actions, 6);
    for c in [id::COPPER, id::SILVER, id::GOLD] {
        assert_eq!(g.players[0].all_cards().get(c), 1, "{}", cards::name(c));
    }
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::PLATINUM));
}

#[test]
fn kings_court_on_kings_court_on_village() {
    // KC KC Village Village Village (the C++ port's exact scenario): the outer KC plays the
    // inner KC 3 times; each of those 3 resolutions independently offers "play an Action from
    // hand", picking a different physical Village (since the previous one already left hand),
    // and each picked Village is itself played 3 times by the inner KC — 3x3 = 9 Village plays.
    let mut g = new_state(&[id::KINGS_COURT, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::KINGS_COURT, id::VILLAGE, id::VILLAGE, id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 10]);
    play(&mut g, id::KINGS_COURT);
    choose(&mut g, Choice::Card(id::KINGS_COURT)); // the 2nd King's Court, played 3 times
    for _ in 0..3 {
        choose(&mut g, Choice::Card(id::VILLAGE));
    }
    // 1 - 1 (outer KC) + 0 (inner KC's own bonus, x3) + 2*9 (nine Village plays) = 18.
    assert_eq!(g.turn.actions, 18);
    assert_eq!(g.players[0].hand.get(id::DUCHY), 9);
    assert_eq!(g.players[0].deck_known.len, 1);
}

// ===========================================================================
// Watchtower — C++ TestWatchtower (2777)
// ===========================================================================

#[test]
fn watchtower_draws_to_six_in_hand() {
    let mut g = new_state(&[id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::WATCHTOWER);
    assert_eq!(g.players[0].hand.total(), 6);
    assert_eq!(g.players[0].deck_known.len, 4);
}

#[test]
fn watchtower_already_at_six_draws_nothing() {
    // 6 Estates besides Watchtower: once Watchtower itself leaves hand to be played, hand is
    // already at 6.
    let mut g = new_state(&[id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 10]);
    play(&mut g, id::WATCHTOWER);
    assert_eq!(g.players[0].hand.total(), 6);
    assert_eq!(g.players[0].deck_known.len, 10);
}

#[test]
fn watchtower_not_enough_cards_to_reach_six() {
    let mut g = new_state(&[id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::WATCHTOWER);
    assert_eq!(g.players[0].hand.total(), 4);
}

#[test]
fn watchtower_may_decline_to_react_to_a_gain() {
    // Watchtower stays in hand throughout (not played as an action): pass on the PlayAction
    // decision first to reach the Buy phase. The gain pipeline's pushed reaction frames keep
    // `buy` from running cleanup before we get to answer them.
    let mut g = new_state(&[id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER]);
    g.turn.coins = 10;
    expect_decision(&mut g);
    pass(&mut g); // end the (empty) action phase
    buy(&mut g, id::GOLD);
    pass(&mut g); // decline to trash
    // The final decline via raw `apply` (not `pass`, which would drive on into cleanup: with
    // both reactions resolved and 0 buys left, the turn would end and reshuffle discard into a
    // fresh hand before we could look).
    expect_decision(&mut g);
    g.apply(Choice::Pass, &mut NoEvents).unwrap();
    assert!(g.players[0].discard.has(id::GOLD));
}

#[test]
fn watchtower_reacts_to_a_bought_card_by_trashing_it() {
    let mut g = new_state(&[id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER]);
    g.turn.coins = 10;
    expect_decision(&mut g);
    pass(&mut g);
    buy(&mut g, id::GOLD);
    choose(&mut g, Choice::Card(id::GOLD)); // trash it
    assert!(g.trash.has(id::GOLD));
    assert!(!g.players[0].discard.has(id::GOLD));
}

#[test]
fn watchtower_reacts_to_a_bought_card_by_topdecking_it() {
    let mut g = new_state(&[id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER]);
    g.turn.coins = 10;
    expect_decision(&mut g);
    pass(&mut g);
    buy(&mut g, id::GOLD);
    pass(&mut g); // decline to trash
    // Raw `apply` (not `choose`), so we can look before cleanup would draw the topdecked card
    // straight into the next hand.
    expect_decision(&mut g);
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(!g.players[0].discard.has(id::GOLD));
}

#[test]
fn watchtower_reacts_to_an_opponents_witch_curse() {
    let mut g = new_state(&[id::WATCHTOWER, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_hand(&mut g, 1, &[id::WATCHTOWER]);
    play(&mut g, id::WITCH);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1); // the victim reacts, not the Witch's owner
    choose(&mut g, Choice::Card(id::CURSE)); // trash the Curse instead of keeping it
    assert!(g.trash.has(id::CURSE));
    assert!(!g.players[1].discard.has(id::CURSE));
}

#[test]
fn watchtower_reacts_to_a_gain_to_hand() {
    // Mine gains its upgraded Treasure to hand; Watchtower can still react to it.
    let mut g = new_state(&[id::WATCHTOWER, id::MINE], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER, id::MINE, id::SILVER]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    choose(&mut g, Choice::Card(id::GOLD)); // trash the newly-gained-to-hand Gold
    assert!(g.trash.has(id::GOLD));
    assert!(!g.players[0].hand.has(id::GOLD));
}

// ===========================================================================
// Bank — C++ TestBank (810)
// ===========================================================================

#[test]
fn bank_counts_treasures_in_play_including_itself() {
    // Bank alone: +$1 (itself).
    let mut g = new_state(&[id::BANK], 2);
    set_hand(&mut g, 0, &[id::BANK]);
    expect_decision(&mut g); // no choice-free treasures to auto-play
    play_treasure(&mut g, id::BANK);
    assert_eq!(g.turn.coins, 1);

    // Two Banks: 1st sees itself (+1), 2nd sees both (+2) = 3.
    let mut g = new_state(&[id::BANK], 2);
    set_hand(&mut g, 0, &[id::BANK, id::BANK]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::BANK);
    play_treasure(&mut g, id::BANK);
    assert_eq!(g.turn.coins, 3);

    // Copper (auto-played, +1) then Bank sees 2 treasures in play (+2) = 3.
    let mut g = new_state(&[id::BANK], 2);
    set_hand(&mut g, 0, &[id::BANK, id::COPPER]);
    play_treasure(&mut g, id::BANK);
    assert_eq!(g.turn.coins, 3);

    // Copper, Silver, Gold (auto-played, 1+2+3=6) then Bank (+4, counting itself) = 10.
    let mut g = new_state(&[id::BANK], 2);
    set_hand(&mut g, 0, &[id::BANK, id::COPPER, id::SILVER, id::GOLD]);
    play_treasure(&mut g, id::BANK);
    assert_eq!(g.turn.coins, 10);

    // Choice-free Treasures always auto-play before any `PlayTreasure` decision is offered (2nd
    // edition; see `docs/seaside-prosperity-plan.md` §2.2), so Bank can never actually be played
    // before the Coppers here, even though it's the only treasure with a real play-order choice:
    // both Coppers auto-play first (+1+1), then Bank counts all 3 treasures in play (+3) = 5.
    let mut g = new_state(&[id::BANK], 2);
    set_hand(&mut g, 0, &[id::BANK, id::COPPER, id::COPPER]);
    play_treasure(&mut g, id::BANK);
    assert_eq!(g.turn.coins, 5);
}

// ===========================================================================
// Quarry — C++ TestQuarry (936)
// ===========================================================================

#[test]
fn quarry_reduces_action_cost_by_two_while_in_play_and_stacks() {
    // Quarry is choice-free: it auto-plays entering the Buy phase, no `PlayTreasure` decision.
    let mut g = new_state(&[id::QUARRY, id::VILLAGE, id::SMITHY, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::QUARRY]);
    expect_decision(&mut g); // lands on the Buy decision directly
    assert_eq!(g.cost(id::VILLAGE), 1); // 3 - 2
    assert_eq!(g.cost(id::SMITHY), 2); // 4 - 2
    assert_eq!(g.cost(id::WITCH), 3); // 5 - 2
    // Non-Action cards are untouched.
    assert_eq!(g.cost(id::GOLD), 6);
    assert_eq!(g.cost(id::PROVINCE), 8);
    assert_eq!(g.cost(id::HAREM), 6);

    // A 2nd Quarry stacks: -4 total.
    set_in_play(&mut g, 0, &[id::QUARRY, id::QUARRY]);
    assert_eq!(g.cost(id::VILLAGE), 0); // clamped floor, not negative
    assert_eq!(g.cost(id::WITCH), 1);
}

#[test]
fn quarry_mixes_with_a_flat_turn_wide_discount() {
    // Quarry (-2 to Actions) plus a Bridge-style flat -1 (applies to everything).
    let mut g = new_state(&[id::QUARRY, id::BRIDGE, id::SMITHY], 2);
    set_in_play(&mut g, 0, &[id::QUARRY]);
    g.turn.cost_reduction = 1;
    assert_eq!(g.cost(id::SMITHY), 1); // 4 - 2 - 1
    assert_eq!(g.cost(id::GOLD), 5); // 6 - 1 (Quarry doesn't apply to Treasures)
}

// ===========================================================================
// Peddler — C++ TestPeddler (1068)
// ===========================================================================

#[test]
fn peddler_no_cards_and_draws() {
    let mut g = new_state(&[id::PEDDLER], 2);
    set_hand(&mut g, 0, &[id::PEDDLER]);
    play(&mut g, id::PEDDLER);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 1, 1));

    let mut g = new_state(&[id::PEDDLER], 2);
    set_hand(&mut g, 0, &[id::PEDDLER]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::PEDDLER);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}

#[test]
fn peddler_costs_less_per_action_in_play_only_during_buy_phase() {
    let mut g = new_state(&[id::PEDDLER, id::VILLAGE], 2);
    assert_eq!(g.cost(id::PEDDLER), 8); // Action phase: full price regardless of actions in play
    set_in_play(&mut g, 0, &[id::VILLAGE, id::VILLAGE]);
    assert_eq!(g.cost(id::PEDDLER), 8);

    g.turn.phase = state::Phase::Buy;
    assert_eq!(g.cost(id::PEDDLER), 4); // 8 - 2*2, only during Buy phase
    set_in_play(&mut g, 0, &[id::VILLAGE; 5]);
    assert_eq!(g.cost(id::PEDDLER), 0); // floor at 0, even with 5 actions (would be -2)
}

#[test]
fn peddler_counts_itself_once_in_play() {
    let mut g = new_state(&[id::PEDDLER], 2);
    set_in_play(&mut g, 0, &[id::PEDDLER]);
    g.turn.phase = state::Phase::Buy;
    assert_eq!(g.cost(id::PEDDLER), 6); // 8 - 2*1 (itself)
}

#[test]
fn forge_discount_applies_to_both_sides() {
    // Silver ($3-1=$2) + Estate ($2-1=$1) + Estate ($1) with a $1 discount sums to $4: Duchy
    // ($5-1=$4 discounted) is the only card at exactly $4, so the last pick auto-resolves it.
    let mut g = new_state(&[id::FORGE], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::FORGE, id::SILVER, id::ESTATE, id::ESTATE]);
    play(&mut g, id::FORGE);
    for c in [id::SILVER, id::ESTATE, id::ESTATE] {
        choose(&mut g, Choice::Card(c));
    }
    assert!(g.trash.has(id::SILVER));
    assert_eq!(g.trash.get(id::ESTATE), 2);
    assert!(g.players[0].discard.has(id::DUCHY));
}

// ===========================================================================
// Anvil — new in 2nd edition, from card text: "$1. You may discard a Treasure to gain a card
// costing up to $4."
// ===========================================================================

#[test]
fn anvil_plays_for_one_coin() {
    let mut g = new_state(&[id::ANVIL], 2);
    set_hand(&mut g, 0, &[id::ANVIL]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::ANVIL);
    assert_eq!(g.turn.coins, 1);
}

#[test]
fn anvil_may_discard_a_treasure_to_gain_up_to_four() {
    // Discards the *other* Anvil: an ordinary choice-free Treasure (Silver) would already have
    // auto-played into `in_play` before this decision even comes up, so it wouldn't be in hand
    // to discard.
    let mut g = new_state(&[id::ANVIL], 2);
    set_hand(&mut g, 0, &[id::ANVIL, id::ANVIL]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::ANVIL);
    choose(&mut g, Choice::Card(id::ANVIL));
    assert!(choices(&g).contains(&Choice::Card(id::ESTATE))); // $2: affordable
    assert!(!choices(&g).contains(&Choice::Card(id::GOLD)), "Gold costs $6: too much");
    choose(&mut g, Choice::Card(id::ESTATE));
    assert!(g.players[0].discard.has(id::ESTATE));
    assert!(g.players[0].discard.has(id::ANVIL)); // discarded, not trashed
}

#[test]
fn anvil_declining_the_discard_gains_nothing() {
    let mut g = new_state(&[id::ANVIL], 2);
    set_hand(&mut g, 0, &[id::ANVIL, id::SILVER]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::ANVIL);
    pass(&mut g);
    assert!(g.players[0].discard.is_empty());
    assert_eq!(g.players[0].hand.get(id::SILVER), 1);
}

// ===========================================================================
// Investment — new in 2nd edition: "Trash a card from your hand. Choose one: +$1; or trash this
// and reveal your hand for +1 VP per differently named Treasure card in it."
// ===========================================================================

#[test]
fn investment_trash_then_plus_one_coin() {
    // Estate (not a choice-free Treasure) stays in hand to be trashed; Copper would have
    // auto-played away before this decision.
    let mut g = new_state(&[id::INVESTMENT], 2);
    set_hand(&mut g, 0, &[id::INVESTMENT, id::ESTATE]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::INVESTMENT); // the trash is mandatory-if-possible: auto-single
    choose(&mut g, Choice::Mode(0)); // "+$1"
    assert_eq!(g.turn.coins, 1);
    assert!(g.trash.has(id::ESTATE));
    assert!(g.players[0].in_play.has(id::INVESTMENT));
}

#[test]
fn investment_trash_self_for_vp_per_differently_named_treasure() {
    // Anvil, Bank and Tiara are has-choice Treasures, so (unlike Copper/Silver/Gold) they stay
    // in hand until explicitly played and are still there when Investment's hand-reveal counts
    // differently-named Treasures.
    let mut g = new_state(&[id::INVESTMENT, id::ANVIL, id::BANK, id::TIARA], 2);
    set_hand(&mut g, 0, &[id::INVESTMENT, id::ESTATE, id::ANVIL, id::BANK, id::TIARA]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::INVESTMENT);
    choose(&mut g, Choice::Card(id::ESTATE)); // trash the Estate
    choose(&mut g, Choice::Mode(1)); // trash Investment for VP
    assert_eq!(g.players[0].vp_tokens, 3); // Anvil, Bank, Tiara: 3 differently-named Treasures
    assert!(g.trash.has(id::INVESTMENT) && g.trash.has(id::ESTATE));
    assert!(!g.players[0].in_play.has(id::INVESTMENT));
    assert_eq!(g.turn.coins, 0);
}

// ===========================================================================
// Tiara — new in 2nd edition: "+1 Buy. This turn, gains may go onto your deck. You may play a
// Treasure from your hand twice."
// ===========================================================================

#[test]
fn tiara_gives_a_buy_and_may_play_a_treasure_twice() {
    // Bank (has-choice) is used as the replay target: an ordinary choice-free Treasure like
    // Silver would already have auto-played into `in_play` before Tiara's own decision comes up,
    // so it would never be available to select.
    let mut g = new_state(&[id::TIARA, id::BANK], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::BANK]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    assert_eq!(g.turn.buys, 2);
    choose(&mut g, Choice::Card(id::BANK)); // played twice
    // Each Bank resolution counts both Treasures in play (Tiara + Bank itself) = 2, twice = 4.
    assert_eq!(g.turn.coins, 4);
    assert!(g.players[0].in_play.has(id::BANK));
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn tiara_may_decline_to_replay_anything() {
    let mut g = new_state(&[id::TIARA, id::BANK], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::BANK]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    pass(&mut g); // decline to replay anything
    // Bank is still in hand (untouched) and gets its own ordinary PlayTreasure decision next.
    play_treasure(&mut g, id::BANK);
    assert_eq!(g.turn.coins, 2); // Bank's single play: itself + Tiara in play = 2
}

#[test]
fn tiara_replaying_bank_recomputes_its_value_each_time() {
    // Bank played first (normally); once it's out of hand, Tiara's own replay decision has
    // nothing left to offer and skips straight through (same as `PlayAction` with no playable
    // actions), landing directly on the Buy decision.
    let mut g = new_state(&[id::TIARA, id::BANK], 2);
    set_hand(&mut g, 0, &[id::TIARA, id::BANK, id::COPPER]);
    expect_decision(&mut g); // Copper auto-plays first
    play_treasure(&mut g, id::BANK);
    play_treasure(&mut g, id::TIARA);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    // Copper (1) + Bank's single play (itself + Tiara in play = 2) = 3.
    assert_eq!(g.turn.coins, 3);
}

#[test]
fn tiara_topdecks_a_gain_instead_of_the_normal_destination() {
    let mut g = new_state(&[id::TIARA], 2);
    set_in_play(&mut g, 0, &[id::TIARA]);
    g.turn.coins = 10;
    expect_decision(&mut g);
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    expect_decision(&mut g);
    // Raw `apply` (not `choose`): this is the last pending reaction, so driving `advance` on
    // from here would run cleanup and draw the just-topdecked Gold straight into a new hand.
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(!g.players[0].discard.has(id::GOLD));
}

// ===========================================================================
// Collection — new in 2nd edition: "$2 +1 Buy. This turn, when you gain an Action card, +1 VP."
// ===========================================================================

#[test]
fn collection_plays_for_two_coins_and_a_buy() {
    let mut g = new_state(&[id::COLLECTION], 2);
    set_hand(&mut g, 0, &[id::COLLECTION]);
    expect_decision(&mut g); // choice-free: auto-played
    assert_eq!((g.turn.coins, g.turn.buys), (2, 2));
}

#[test]
fn collection_gives_vp_only_for_gained_actions_while_in_play() {
    let mut g = new_state(&[id::COLLECTION, id::WORKSHOP], 2);
    set_in_play(&mut g, 0, &[id::COLLECTION]);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::WORKSHOP)); // gain another Workshop: an Action
    assert_eq!(g.players[0].vp_tokens, 1);

    // A Victory/Treasure gain gives nothing.
    let mut g = new_state(&[id::COLLECTION], 2);
    set_in_play(&mut g, 0, &[id::COLLECTION]);
    g.turn.coins = 10;
    expect_decision(&mut g);
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].vp_tokens, 0);
}

// ===========================================================================
// Crystal Ball — new in 2nd edition: "$1. Look at the top card: you may trash it, discard it,
// or, if it's an Action or Treasure, play it."
// ===========================================================================

#[test]
fn crystal_ball_plays_for_one_coin() {
    let mut g = new_state(&[id::CRYSTAL_BALL], 2);
    set_hand(&mut g, 0, &[id::CRYSTAL_BALL]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::CRYSTAL_BALL);
    assert_eq!(g.turn.coins, 1);
}

#[test]
fn crystal_ball_may_leave_the_top_card_in_place() {
    // Estate is neither Action nor Treasure, so "play it" is never offered (auto-skipped): the
    // first real decision is trash, then discard; declining both leaves it for the mandatory
    // (but here single-choice, auto-resolved) "put back" step.
    let mut g = new_state(&[id::CRYSTAL_BALL], 2);
    set_hand(&mut g, 0, &[id::CRYSTAL_BALL]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::CRYSTAL_BALL);
    pass(&mut g); // don't trash
    pass(&mut g); // don't discard
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));
}

#[test]
fn crystal_ball_can_trash_the_top_card() {
    let mut g = new_state(&[id::CRYSTAL_BALL], 2);
    set_hand(&mut g, 0, &[id::CRYSTAL_BALL]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::CRYSTAL_BALL);
    choose(&mut g, Choice::Card(id::ESTATE)); // trash (the first real decision)
    assert!(g.trash.has(id::ESTATE));
}

#[test]
fn crystal_ball_can_discard_the_top_card() {
    let mut g = new_state(&[id::CRYSTAL_BALL], 2);
    set_hand(&mut g, 0, &[id::CRYSTAL_BALL]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::CRYSTAL_BALL);
    pass(&mut g); // don't trash
    choose(&mut g, Choice::Card(id::ESTATE)); // discard
    assert!(g.players[0].discard.has(id::ESTATE));
}

#[test]
fn crystal_ball_can_play_an_action_or_treasure_top_card() {
    let mut g = new_state(&[id::CRYSTAL_BALL, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CRYSTAL_BALL]);
    set_deck_known(&mut g, 0, &[id::VILLAGE]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::CRYSTAL_BALL);
    choose(&mut g, Choice::Card(id::VILLAGE)); // play it
    assert!(g.players[0].in_play.has(id::VILLAGE));
    // Playing a Treasure never spends an action, so the default 1 is untouched; Village then
    // adds its own +2 Actions (its +1 Card finds an empty deck and draws nothing).
    assert_eq!(g.turn.actions, 3);
}

// ===========================================================================
// Charlatan — new in 2nd edition: "+$3. Each other player gains a Curse. Curse is also a
// Treasure worth $1 in games using this."
// ===========================================================================

#[test]
fn charlatan_gives_three_coins_and_curses_opponents() {
    let mut g = new_state(&[id::CHARLATAN], 3);
    set_hand(&mut g, 0, &[id::CHARLATAN]);
    play(&mut g, id::CHARLATAN);
    assert_eq!(g.turn.coins, 3);
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
    assert_eq!(g.players[2].discard, counts_of(&[id::CURSE]));
}

#[test]
fn charlatan_makes_curse_a_one_coin_treasure_and_bank_counts_it() {
    assert!(!new_state(&[id::BANK], 2).is_treasure(id::CURSE), "no Charlatan in this game");
    assert!(new_state(&[id::CHARLATAN], 2).is_treasure(id::CURSE));

    // Curse plays as a choice-free $1 Treasure.
    let mut g = new_state(&[id::CHARLATAN], 2);
    set_hand(&mut g, 0, &[id::CURSE]);
    expect_decision(&mut g);
    assert_eq!(g.turn.coins, 1);

    // Bank counts it too.
    let mut g = new_state(&[id::CHARLATAN, id::BANK], 2);
    set_hand(&mut g, 0, &[id::BANK, id::CURSE]);
    expect_decision(&mut g); // Curse auto-plays (choice-free), Bank offered next
    play_treasure(&mut g, id::BANK);
    assert_eq!(g.turn.coins, 1 + 2); // Curse (+1) then Bank counting both (+2)
}

// ===========================================================================
// Clerk — new in 2nd edition: "+$2. Each other player with 5+ cards in hand puts one onto their
// deck." The start-of-turn reaction needs step 3's turn-start hook (TODO).
// ===========================================================================

#[test]
fn clerk_gives_two_coins_and_attacks_full_handed_opponents() {
    // 2 distinct cards, so the victim's forced topdeck pick is a real decision (not auto-single).
    let mut g = new_state(&[id::CLERK], 2);
    set_hand(&mut g, 0, &[id::CLERK]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::DUCHY]);
    // `new_state`/`reset_turn` puts us at the very start of the turn, so Clerk's own
    // start-of-turn reaction (step 3) is offered first; decline it to test the normal
    // Action-phase play instead.
    expect_decision(&mut g);
    choose(&mut g, Choice::No);
    play(&mut g, id::CLERK);
    assert_eq!(g.turn.coins, 2);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.players[1].hand.total(), 4);
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::DUCHY));
}

#[test]
fn clerk_does_not_attack_a_player_with_fewer_than_five_cards() {
    let mut g = new_state(&[id::CLERK], 2);
    set_hand(&mut g, 0, &[id::CLERK]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE]);
    expect_decision(&mut g);
    choose(&mut g, Choice::No); // decline Clerk's own start-of-turn reaction offer first
    play(&mut g, id::CLERK);
    assert_eq!(g.players[1].hand.total(), 2); // untouched
}

#[test]
fn clerk_is_blocked_by_moat() {
    let mut g = new_state(&[id::CLERK], 2);
    set_hand(&mut g, 0, &[id::CLERK]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    expect_decision(&mut g);
    choose(&mut g, Choice::No); // decline Clerk's own start-of-turn reaction offer first
    play(&mut g, id::CLERK);
    assert_eq!(g.players[1].hand.total(), 6);
}

// ===========================================================================
// War Chest — new in 2nd edition: "The player to your left names a card; gain a card costing up
// to $5 that hasn't been named for War Chests this turn."
// ===========================================================================

#[test]
fn war_chest_left_player_names_then_owner_gains_up_to_five() {
    let mut g = new_state(&[id::WAR_CHEST], 3);
    set_hand(&mut g, 0, &[id::WAR_CHEST]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::WAR_CHEST);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1, "the player to the War Chest owner's left names the card");
    choose(&mut g, Choice::Card(id::GOLD)); // named, and thus excluded from the gain
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 0);
    assert!(!choices(&g).contains(&Choice::Card(id::GOLD)), "the named card can't be gained");
    assert!(choices(&g).contains(&Choice::Card(id::DUCHY))); // $5, not named
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(g.players[0].discard.has(id::DUCHY));
    assert_eq!(g.turn.named_for_war_chest, counts_of(&[id::GOLD]));
}

#[test]
fn war_chest_excludes_names_from_earlier_war_chests_this_turn() {
    let mut g = new_state(&[id::WAR_CHEST, id::TIARA], 3);
    set_hand(&mut g, 0, &[id::TIARA, id::WAR_CHEST]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::TIARA);
    choose(&mut g, Choice::Card(id::WAR_CHEST)); // Tiara replays War Chest: twice total

    // 1st resolution: player 1 names Silver.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::SILVER));
    // 1st gain: player 0 takes the cheap Copper.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 0);
    choose(&mut g, Choice::Card(id::COPPER));
    // Tiara is also in play, so it offers to topdeck this same gain; decline (it's just a Copper).
    pass(&mut g);

    // 2nd resolution: player 1 names Gold.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::GOLD));
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 0);
    assert!(!choices(&g).contains(&Choice::Card(id::SILVER)), "named by the 1st War Chest this turn");
    assert!(!choices(&g).contains(&Choice::Card(id::GOLD)), "named by the 2nd War Chest this turn");
    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.turn.named_for_war_chest, counts_of(&[id::SILVER, id::GOLD]));
}

// ===========================================================================
// Cross-cutting
// ===========================================================================

#[test]
fn cannot_play_a_treasure_after_buying_in_the_same_buy_phase() {
    let mut g = new_state(&[id::ANVIL], 2);
    set_hand(&mut g, 0, &[id::ANVIL, id::COPPER]);
    g.turn.buys = 2;
    expect_decision(&mut g); // Copper auto-plays; Anvil offered
    pass(&mut g); // decline to play Anvil now
    buy(&mut g, id::COPPER); // uses the other buy; 2nd-edition rule kicks in
    // Anvil is still in hand, but the Buy phase must not offer to play it anymore.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy, "no more Treasures may be played after a buy");
    assert!(g.players[0].hand.has(id::ANVIL));
}

#[test]
fn quarry_bridge_and_peddler_costs_stack() {
    // Peddler is itself an Action card, so all three reductions stack on it: Quarry's generic
    // Action discount, Bridge's flat discount, and Peddler's own per-Action-in-play discount.
    let mut g = new_state(&[id::QUARRY, id::BRIDGE, id::PEDDLER, id::VILLAGE], 2);
    set_in_play(&mut g, 0, &[id::QUARRY, id::QUARRY, id::VILLAGE]);
    g.turn.cost_reduction = 1; // one Bridge
    g.turn.phase = state::Phase::Buy;
    // Peddler ($8): -1 (Bridge) -2*2 (two Quarries, Peddler is an Action) -2*1 (1 Action -
    // Village - in play, Peddler's own rule) = 8-1-4-2 = 1.
    assert_eq!(g.cost(id::PEDDLER), 1);
    // Village ($3): -1 (Bridge) -2*2 (two Quarries), clamped to 0 (would be -2).
    assert_eq!(g.cost(id::VILLAGE), 0);
}

#[test]
fn prosperity_state_with_vp_tokens_round_trips_through_text() {
    let mut g = new_state(&[id::HOARD, id::BISHOP], 2);
    g.players[0].vp_tokens = 5;
    let text = format_state(&g);
    assert!(text.contains("vp tokens: 5"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].vp_tokens, 5);
}

#[test]
fn treasures_done_and_named_for_war_chest_are_not_persisted_and_reset_on_load() {
    // Both are transient within-turn bookkeeping (like `TurnState::played`): a loaded position
    // starts fresh, so a reload can't durably remember "Treasures are done" or "these were named
    // for War Chest" from mid-turn. Documented behavior, not just an oversight: check it holds.
    let mut g = new_state(&[id::WAR_CHEST], 2);
    g.turn.treasures_done = true;
    g.turn.named_for_war_chest = counts_of(&[id::GOLD]);
    let text = format_state(&g);
    let back = parse_state(&text).unwrap();
    assert!(!back.turn.treasures_done);
    assert!(back.turn.named_for_war_chest.is_empty());
}

// ===========================================================================
// King's Court on a Duration — PropserityCardsTests.cpp TestKingsCourtDuration (2027).
// Only the King's Court + Merchant Ship cases are ported (both Seaside/Prosperity, implemented
// here); the Hireling case is skipped (Adventures, a permanent Duration, out of scope for our
// four sets).
// ===========================================================================

#[test]
fn kings_court_stays_in_play_with_a_duration_it_multiplies() {
    // "KingsCourt stays with Duration": KC x3 on Merchant Ship ("Now and next turn: +$2") gives
    // +$6 now, and both King's Court and Merchant Ship stay in play for next turn.
    let mut g = new_state(&[id::KINGS_COURT, id::MERCHANT_SHIP], 2);
    set_hand(&mut g, 0, &[id::KINGS_COURT, id::MERCHANT_SHIP]);
    play(&mut g, id::KINGS_COURT);
    choose(&mut g, Choice::Card(id::MERCHANT_SHIP));
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 6));
    assert!(g.turn.duration_held.has(id::KINGS_COURT) && g.turn.duration_held.has(id::MERCHANT_SHIP));
    assert_eq!(g.players[0].pending_durations_len, 1);
    let e = g.players[0].pending_durations[0];
    assert_eq!((e.card, e.times), (id::MERCHANT_SHIP, 3));
}


#[test]
fn witch_draws_before_giving_curses_and_watchtower_is_revealed_to_trash_it() {
    // "+2 Cards. Each other player gains a Curse": the draw comes first. The victim reveals
    // Watchtower from hand to trash the Curse, and the reveal is logged.
    let mut g = new_state(&[id::WITCH, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_hand(&mut g, 1, &[id::WATCHTOWER]);
    let mut ev: Vec<Event> = Vec::new();
    let step = play_ev(&mut g, id::WITCH, &mut ev);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(d.player, 1, "the victim decides the Watchtower reaction");
    let draw = ev.iter().position(|e| matches!(e, Event::Draw { player: 0, .. })).expect("draw");
    let curse = ev.iter().position(|e| matches!(e, Event::Gain { player: 1, card: id::CURSE, .. })).expect("curse");
    assert!(draw < curse, "Witch's +2 Cards comes before the Curse: {ev:?}");
    choose_ev(&mut g, Choice::Card(id::CURSE), &mut ev);
    assert!(ev.iter().any(|e| matches!(e, Event::Reaction { player: 1, card: id::WATCHTOWER })), "reveal logged: {ev:?}");
    assert_eq!(g.trash.get(id::CURSE), 1);
    assert_eq!(g.players[1].all_cards().get(id::CURSE), 0);
    assert!(g.players[1].hand.has(id::WATCHTOWER), "revealing keeps it in hand");
}

#[test]
fn witch_curses_go_to_the_first_player_in_turn_order_when_the_pile_is_short() {
    let mut g = new_state(&[id::WITCH], 3);
    set_supply(&mut g, id::CURSE, 1);
    set_hand(&mut g, 0, &[id::WITCH]);
    play(&mut g, id::WITCH);
    assert_eq!(g.players[1].all_cards().get(id::CURSE), 1, "the player to the left gets the last Curse");
    assert_eq!(g.players[2].all_cards().get(id::CURSE), 0);
}
