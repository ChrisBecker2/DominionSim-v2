//! Intrigue (2nd edition) card tests, ported from the old C++ simulator's
//! `DominionSimTestCards\IntrigueCardsTests.cpp` (1st-edition Intrigue) where the card exists in
//! 2nd edition, adapted to this engine's API. Each test cites the C++ `TEST_METHOD` it came
//! from; tests for cards new in 2nd edition (Lurker, Diplomat, Mill, Secret Passage, Courtier,
//! Patrol, Replace) are written from the card text. See `docs/intrigue-plan.md` §5a.
//!
//! Not ported (1st edition only): TestGreatHall, TestSecretChamber, TestScout, TestSaboteur,
//! TestCoppersmith, TestTribute.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet, ModeOpt};
use dominion_engine::state;
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
// Harem — C++ TestHarem
// ===========================================================================

#[test]
fn harem_basics_vp_and_worth() {
    // VerifyBasics<MatchVictory, MatchTreasure>(Harem, "Harem", 6, 2)
    let d = cards::def(id::HAREM);
    assert_eq!((d.cost, d.vp, d.coins), (6, 2, 2));
    assert!(cards::is(id::HAREM, cards::TREASURE) && cards::is(id::HAREM, cards::VICTORY));
    assert_eq!(cards::set_of(id::HAREM), CardSet::Intrigue);
    for (n, vp) in [(1, 2), (2, 4), (3, 6), (10, 20)] {
        assert_eq!(vp_of(&[(id::HAREM, n)]), vp);
    }
    // MakeGame<PlayAllTreasures>(Harem, {Estate, Estate}): played as a treasure for $2.
    let mut g = new_state(&[id::HAREM], 2);
    set_hand(&mut g, 0, &[id::HAREM]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].in_play.get(id::HAREM), 1);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn harem_pile_is_a_victory_pile() {
    // Victory kingdom piles have 8 cards with 2 players, 12 with more.
    assert_eq!(new_state(&[id::HAREM], 2).supply.get(id::HAREM), 8);
    assert_eq!(new_state(&[id::HAREM], 4).supply.get(id::HAREM), 12);
}

#[test]
fn mine_a_silver_into_a_harem_and_a_harem_into_a_gold() {
    // Base C++ suite, TestMine / Harem cases: Harem is a Treasure for Mine in both directions.
    let mut g = new_state(&[id::MINE, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::MINE, id::SILVER]);
    play(&mut g, id::MINE);
    // Only one treasure: the trash choice is Silver (or pass).
    choose(&mut g, Choice::Card(id::SILVER));
    let d = g.pending_decision().expect("gain decision");
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 6, .. }));
    choose(&mut g, Choice::Card(id::HAREM));
    assert_eq!(g.players[0].in_play.get(id::HAREM), 1, "gained to hand, then played as a treasure");
    assert_eq!(g.turn.coins, 2);

    let mut g = new_state(&[id::MINE, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::MINE, id::HAREM]);
    play(&mut g, id::MINE);
    choose(&mut g, Choice::Card(id::HAREM));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.trash.get(id::HAREM), 1);
    assert_eq!(g.turn.coins, 3);
}

// ===========================================================================
// Duke — C++ TestDuke
// ===========================================================================

#[test]
fn duke_is_worth_one_per_duchy() {
    let d = cards::def(id::DUKE);
    assert_eq!((d.cost, d.vp), (5, 0));
    assert!(cards::is(id::DUKE, cards::VICTORY));
    assert_eq!(vp_of(&[(id::DUKE, 1)]), 0);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 1)]), 1 + 3);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 5)]), 5 + 3 * 5);
    assert_eq!(vp_of(&[(id::DUKE, 5), (id::DUCHY, 1)]), 5 + 3);
    assert_eq!(vp_of(&[(id::DUKE, 5), (id::DUCHY, 5)]), 5 * 5 + 5 * 3);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 1), (id::VILLAGE, 1)]), 1 + 3);
    assert_eq!(vp_of(&[(id::DUKE, 1), (id::DUCHY, 1), (id::VILLAGE, 1), (id::ESTATE, 1)]), 1 + 3 + 1);
}

// ===========================================================================
// Bridge — C++ TestBridge
// ===========================================================================

#[test]
fn bridge_gives_a_buy_a_coin_and_a_discount() {
    let d = cards::def(id::BRIDGE);
    assert_eq!(d.cost, 4);
    assert!(cards::is(id::BRIDGE, cards::ACTION));
    // MakeGame<PlayFirstAction>({Bridge}): VerifyTurnBasics(0 actions, 2 buys, $1), discount 1.
    let mut g = new_state(&[id::BRIDGE], 2);
    set_hand(&mut g, 0, &[id::BRIDGE]);
    play(&mut g, id::BRIDGE);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins, g.turn.cost_reduction), (0, 2, 1, 1));
    assert_eq!(g.players[0].in_play.get(id::BRIDGE), 1);
    // With $1 and a discount of 1, Estates ($2 -> $1) and Silver ($3 -> $2) cost less; Curse/Copper stay 0.
    assert_eq!(g.cost(id::ESTATE), 1);
    assert_eq!(g.cost(id::SILVER), 2);
    assert_eq!(g.cost(id::COPPER), 0);
    let buyable = choices(&g);
    assert!(buyable.contains(&Choice::Card(id::ESTATE)));
    assert!(!buyable.contains(&Choice::Card(id::SILVER)));
    buy(&mut g, id::ESTATE);
    assert_eq!(g.turn.coins, 0, "paid the reduced cost");
}

#[test]
fn two_throne_rooms_and_two_bridges_buy_two_provinces_and_three_silvers() {
    // C++ TestBridge simulation case: {ThroneRoom, ThroneRoom, Bridge, Bridge, Silver, Silver}.
    // Throne Room plays the second Throne Room twice, each time on a Bridge: 4 Bridge plays with
    // one action = +4 buys (5), +$4, discount 4. With $8 (4 + two Silvers) Provinces cost $4:
    // two Provinces, then three free Silvers.
    let mut g = new_state(&[id::THRONE_ROOM, id::BRIDGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::THRONE_ROOM, id::BRIDGE, id::BRIDGE, id::SILVER, id::SILVER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::THRONE_ROOM));
    choose(&mut g, Choice::Card(id::BRIDGE));
    choose(&mut g, Choice::Card(id::BRIDGE));
    assert_eq!(g.turn.played.get(id::BRIDGE), 4);
    assert_eq!((g.turn.buys, g.turn.coins, g.turn.cost_reduction), (5, 8, 4));
    assert_eq!(g.cost(id::PROVINCE), 4);
    buy(&mut g, id::PROVINCE);
    buy(&mut g, id::PROVINCE);
    for _ in 0..3 {
        buy(&mut g, id::SILVER);
    }
    // Cleanup reshuffles the (empty) deck, so count everything the player owns.
    let gained = g.players[0].all_cards();
    assert_eq!((gained.get(id::PROVINCE), gained.get(id::SILVER)), (2, 2 + 3), "two Silvers in hand plus three bought");
    assert_eq!(g.players[0].turns_taken, 1, "all five buys used, so the turn ended");
}

#[test]
fn bridge_reduces_gain_costs_too_and_resets_next_turn() {
    // Bridge then Workshop: "gain a card costing up to $4" reaches a $5 card.
    let mut g = new_state(&[id::BRIDGE, id::WORKSHOP, id::VILLAGE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::BRIDGE, id::WORKSHOP]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    play(&mut g, id::VILLAGE);
    choose(&mut g, Choice::Card(id::BRIDGE));
    choose(&mut g, Choice::Card(id::WORKSHOP));
    assert!(choices(&g).contains(&Choice::Card(id::MARKET)), "Market costs $4 this turn");
    choose(&mut g, Choice::Card(id::MARKET));
    assert_eq!(g.players[0].discard.get(id::MARKET), 1);
    // The next turn starts without the reduction.
    assert_eq!(state::TurnState::start(1, 2).cost_reduction, 0);
}

#[test]
fn cost_reduction_round_trips_through_the_text_format() {
    let mut g = new_state(&[id::BRIDGE], 2);
    set_hand(&mut g, 0, &[id::BRIDGE, id::COPPER]);
    play(&mut g, id::BRIDGE);
    let text = format_state(&g);
    assert!(text.contains("cost_reduction: 1"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.turn.cost_reduction, 1);
    // Absent when zero.
    assert!(!format_state(&new_state(&[id::BRIDGE], 2)).contains("cost_reduction"));
}

#[test]
fn unimplemented_cards_cannot_be_in_a_kingdom() {
    let not_ready: Vec<CardId> = (cards::FIRST_KINGDOM..cards::NUM_CARDS as CardId).filter(|&c| !cards::is_ready(c)).collect();
    for &c in &not_ready {
        assert!(!cards::kingdom_cards().any(|k| k == c));
        let err = text::parse_kingdom(cards::name(c)).unwrap_err();
        assert!(err.contains("not implemented"), "{err}");
    }
}

// ===========================================================================
// Courtyard — C++ TestCourtyard (line 15)
// ===========================================================================

#[test]
fn courtyard_basics_and_deck_sizes() {
    let d = cards::def(id::COURTYARD);
    assert_eq!((d.cost, d.vp), (2, 0));
    assert!(cards::is(id::COURTYARD, cards::ACTION));

    // 3 cards in deck: draws all 3, then must put one back (no Pass: min = max = 1). Two
    // distinct card types among the draw so the topdeck is a real decision (a homogeneous draw
    // has only one distinct choice, which `auto_single` applies without stopping — see below).
    let mut g = new_state(&[id::COURTYARD], 2);
    set_hand(&mut g, 0, &[id::COURTYARD]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY, id::SILVER]);
    play(&mut g, id::COURTYARD);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::DUCHY]));
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::Select { from: Zone::Hand, act: Act::Topdeck, min: 1, max: 1, ordered: false, .. }));
    assert!(!choices(&g).contains(&Choice::Pass), "Courtyard's topdeck is mandatory");
    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::DUCHY, id::SILVER]);

    // 2 cards in deck: only 2 are drawn (deck exhausted), then one goes back.
    let mut g = new_state(&[id::COURTYARD], 2);
    set_hand(&mut g, 0, &[id::COURTYARD]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::COURTYARD);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));

    // 1 card in deck: draws the 1 card; with only one distinct card in hand there's only one
    // legal (canonical) choice, so `auto_single` puts it straight back without stopping.
    let mut g = new_state(&[id::COURTYARD], 2);
    set_hand(&mut g, 0, &[id::COURTYARD]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::COURTYARD);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));

    // 0 cards in deck: nothing drawn, nothing to put back (no decision at all).
    let mut g = new_state(&[id::COURTYARD], 2);
    set_hand(&mut g, 0, &[id::COURTYARD]);
    play(&mut g, id::COURTYARD);
    assert!(g.players[0].hand.is_empty());
    assert!(g.stack.is_empty(), "nothing left to select: Courtyard's own effect stack unwound");
}

// ===========================================================================
// Shanty Town — C++ TestShantyTown (line 638)
// ===========================================================================

#[test]
fn shanty_town_draws_only_with_no_action_in_hand() {
    let d = cards::def(id::SHANTY_TOWN);
    assert_eq!((d.cost, d.vp), (3, 0));

    // Action in hand (Village): no draw. (C++'s GreatHall sub-case is 1st-edition-only: skipped.)
    let mut g = new_state(&[id::SHANTY_TOWN, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SHANTY_TOWN, id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    let mut events: Vec<Event> = Vec::new();
    play_ev(&mut g, id::SHANTY_TOWN, &mut events);
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE]));
    assert_eq!(g.turn.actions, 2);
    // The whole hand (just Village at reveal time) is logged as revealed.
    assert!(events.iter().any(|e| matches!(e, Event::Reveal { card, .. } if *card == id::VILLAGE)));

    // Moat (Action + Reaction) also counts as an Action card.
    let mut g = new_state(&[id::SHANTY_TOWN, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::SHANTY_TOWN, id::MOAT]);
    play(&mut g, id::SHANTY_TOWN);
    assert_eq!(g.players[0].hand, counts_of(&[id::MOAT]));
    assert_eq!(g.turn.actions, 2);

    // No Action cards in hand: +2 Cards. Junk only (no Treasure), so the empty-of-Actions hand
    // doesn't also trigger the Buy phase's auto-play of treasures and confuse the assertions.
    let mut g = new_state(&[id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::SHANTY_TOWN, id::ESTATE, id::CURSE, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::SHANTY_TOWN);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE, id::CURSE, id::DUCHY]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::ESTATE]);
    assert_eq!(g.turn.actions, 2);

    // 1 card left to draw.
    let mut g = new_state(&[id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::SHANTY_TOWN]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::SHANTY_TOWN);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));

    // No cards to draw.
    let mut g = new_state(&[id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::SHANTY_TOWN]);
    play(&mut g, id::SHANTY_TOWN);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn shanty_town_with_throne_room_and_two_shanty_towns() {
    // ThroneRoom: actions drawn on the first resolution stop the second from drawing.
    let mut g = new_state(&[id::THRONE_ROOM, id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SHANTY_TOWN]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::VILLAGE, id::GOLD, id::GOLD]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SHANTY_TOWN));
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::VILLAGE]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::GOLD, id::GOLD]);
    assert_eq!(g.turn.actions, 4, "0 (ThroneRoom, no actions) + 2 + 2 (both ShantyTown resolutions)");

    // No actions ever drawn: both resolutions draw, for 4 cards total. Junk only (no Treasure)
    // so the resulting hand (no Actions) doesn't also auto-play into the Buy phase.
    let mut g = new_state(&[id::THRONE_ROOM, id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SHANTY_TOWN]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SHANTY_TOWN));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY]));
    assert!(g.players[0].deck_known.is_empty());
    assert_eq!(g.turn.actions, 4);

    // Two separate Shanty Towns: the first sees the second still in hand (an Action), so only
    // the second (now alone) draws.
    let mut g = new_state(&[id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::SHANTY_TOWN, id::SHANTY_TOWN]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::VILLAGE, id::GOLD, id::GOLD]);
    play(&mut g, id::SHANTY_TOWN);
    assert_eq!(g.turn.actions, 2, "1st Shanty Town sees the 2nd (an Action) in hand: no draw");
    play(&mut g, id::SHANTY_TOWN);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::VILLAGE]));
    assert_eq!(g.turn.actions, 3, "2 - 1 (play) + 2 (2nd Shanty Town's own bonus)");
}

// ===========================================================================
// Conspirator — C++ TestConspirator (line 928)
// ===========================================================================

#[test]
fn conspirator_counts_action_cards_played_this_turn() {
    let d = cards::def(id::CONSPIRATOR);
    assert_eq!((d.cost, d.vp, d.coins), (4, 0, 2));

    // Only Conspirator played: 1 Action played, below 3.
    let mut g = new_state(&[id::CONSPIRATOR], 2);
    set_hand(&mut g, 0, &[id::CONSPIRATOR]);
    play(&mut g, id::CONSPIRATOR);
    assert_eq!((g.turn.actions, g.turn.coins), (0, 2));
    assert!(g.players[0].hand.is_empty());

    // 2 Actions played (Village, Conspirator): still below 3. The resulting hand (just a
    // Copper) has no Action left to play, so the engine rolls straight into the Buy phase and
    // auto-plays it — +$1 on top of Conspirator's own +$2.
    let mut g = new_state(&[id::VILLAGE, id::CONSPIRATOR], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::CONSPIRATOR]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER]);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::CONSPIRATOR);
    assert_eq!((g.turn.actions, g.turn.coins), (1, 3), "Conspirator's +$2, then Copper auto-played for +$1");
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::SILVER]);

    // 3 Actions played (Village, Conspirator, Conspirator): the 3rd unlocks +1 Card +1 Action,
    // drawing the Silver; both drawn treasures (Copper, Silver) then auto-play in the Buy phase.
    let mut g = new_state(&[id::VILLAGE, id::CONSPIRATOR], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::CONSPIRATOR, id::CONSPIRATOR]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER]);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::CONSPIRATOR);
    play(&mut g, id::CONSPIRATOR);
    assert_eq!((g.turn.actions, g.turn.coins), (1, 7), "2x Conspirator (+$4) + Copper (+$1) + Silver (+$2)");
    assert!(g.players[0].hand.is_empty());
    assert!(g.players[0].deck_known.is_empty());
}

#[test]
fn conspirator_with_throne_room_counts_both_plays() {
    // Throne Room plays Conspirator twice: the 1st resolution is the 2nd Action played this
    // turn (Throne Room itself is the 1st), the 2nd resolution is the 3rd and unlocks the bonus.
    let mut g = new_state(&[id::THRONE_ROOM, id::CONSPIRATOR], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::CONSPIRATOR]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::CONSPIRATOR));
    // +$4 from both Conspirator resolutions, then Copper (drawn by the bonus) auto-plays for +$1.
    assert_eq!((g.turn.actions, g.turn.coins), (1, 5));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::SILVER]);
}

// ===========================================================================
// Baron — C++ TestBaron (line 872)
// ===========================================================================

#[test]
fn baron_discards_estate_or_gains_one() {
    let d = cards::def(id::BARON);
    assert_eq!((d.cost, d.vp, d.buys), (4, 0, 1));

    // Discards the Estate for +$4.
    let mut g = new_state(&[id::BARON], 2);
    set_hand(&mut g, 0, &[id::BARON, id::ESTATE]);
    play(&mut g, id::BARON);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Discard });
    assert_eq!(g.pending_decision().unwrap().subject, id::ESTATE);
    choose(&mut g, Choice::Yes);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 4));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));

    // No Estate in hand: skipped straight to gaining one, no decision at all.
    let mut g = new_state(&[id::BARON], 2);
    set_hand(&mut g, 0, &[id::BARON]);
    play(&mut g, id::BARON);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 0));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));

    // Chooses not to discard the Estate: gains one instead, keeping the one in hand.
    let mut g = new_state(&[id::BARON], 2);
    set_hand(&mut g, 0, &[id::BARON, id::ESTATE]);
    play(&mut g, id::BARON);
    choose(&mut g, Choice::No);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));

    // No Estates left in the supply: nothing is gained, but the decision still happens (or is
    // skipped) the same way.
    let mut g = new_state(&[id::BARON], 2);
    set_hand(&mut g, 0, &[id::BARON]);
    empty_pile(&mut g, id::ESTATE);
    play(&mut g, id::BARON);
    assert_eq!(g.turn.buys, 2);
    assert!(g.players[0].discard.is_empty());
}

// ===========================================================================
// Mining Village — C++ TestMiningVillage (line 2686)
// ===========================================================================

#[test]
fn mining_village_may_trash_itself_for_two_coins() {
    let d = cards::def(id::MINING_VILLAGE);
    assert_eq!((d.cost, d.vp, d.cards, d.actions), (4, 0, 1, 2));

    // Played, declines the trash.
    let mut g = new_state(&[id::MINING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    play(&mut g, id::MINING_VILLAGE);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Trash });
    choose(&mut g, Choice::No);
    assert_eq!((g.turn.actions, g.turn.coins), (2, 0));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    assert_eq!(g.players[0].in_play, counts_of(&[id::MINING_VILLAGE]));

    // Played and trashed: +$2, and it truly leaves play.
    let mut g = new_state(&[id::MINING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::Yes);
    assert_eq!((g.turn.actions, g.turn.coins), (2, 2));
    assert!(g.players[0].in_play.is_empty(), "Mining Village left play");
    assert_eq!(g.trash, counts_of(&[id::MINING_VILLAGE]));

    // Two Mining Villages: trashing the first doesn't affect whether the second can be trashed,
    // and declining the first doesn't leave it un-trashable later.
    let mut g = new_state(&[id::MINING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE, id::MINING_VILLAGE]);
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::No); // keep the first
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::Yes); // trash the second
    assert_eq!(g.players[0].in_play, counts_of(&[id::MINING_VILLAGE]), "one copy remains in play");
    assert_eq!(g.trash, counts_of(&[id::MINING_VILLAGE]));
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn mining_village_with_throne_room_trashes_only_once() {
    // The 1st resolution may trash it; the 2nd still gives +1 Card +2 Actions but there's
    // nothing left in play to trash, so no decision is offered the second time.
    let mut g = new_state(&[id::THRONE_ROOM, id::MINING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MINING_VILLAGE));
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Trash });
    choose(&mut g, Choice::Yes);
    // The 2nd resolution's +1 Card / +2 Actions still applied, with no further decision.
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::DUCHY]));
    assert_eq!(g.turn.actions, 4, "0 (Throne Room) + 2 + 2, both Mining Village resolutions");
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash, counts_of(&[id::MINING_VILLAGE]));
    assert!(g.players[0].in_play.get(id::MINING_VILLAGE) == 0);
}

// ===========================================================================
// Ironworks — C++ TestIronworks (line 1017)
// ===========================================================================

#[test]
fn ironworks_bonus_matches_the_gained_card_type() {
    let d = cards::def(id::IRONWORKS);
    assert_eq!((d.cost, d.vp), (4, 0));

    // Gain a Treasure (Silver): +$1.
    let mut g = new_state(&[id::IRONWORKS], 2);
    set_hand(&mut g, 0, &[id::IRONWORKS]);
    play(&mut g, id::IRONWORKS);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Gain { max_cost: 4, filter: Filter::Any, dest: Dest::Discard, exact: false });
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!((g.turn.actions, g.turn.coins), (0, 1));
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER]));

    // Gain a Victory card (Estate): +1 Card. The drawn Copper is then the only card in hand, so
    // it auto-plays into the Buy phase (+$1) rather than sitting in hand.
    let mut g = new_state(&[id::IRONWORKS], 2);
    set_hand(&mut g, 0, &[id::IRONWORKS]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::IRONWORKS);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.turn.coins, 1, "the drawn Copper auto-played");
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));

    // Gain an Action (Village): +1 Action.
    let mut g = new_state(&[id::IRONWORKS, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::IRONWORKS]);
    play(&mut g, id::IRONWORKS);
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!((g.turn.actions, g.turn.coins), (1, 0));

    // Dual type (Mill: Action + Victory): both +1 Action and +1 Card apply. Again, the drawn
    // Copper ends up alone in hand and auto-plays.
    let mut g = new_state(&[id::IRONWORKS, id::MILL], 2);
    set_hand(&mut g, 0, &[id::IRONWORKS]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::IRONWORKS);
    choose(&mut g, Choice::Card(id::MILL));
    assert_eq!((g.turn.actions, g.turn.coins), (1, 1));
    assert!(g.players[0].hand.is_empty());

    // Dual type + Bridge discount (Harem: Treasure + Victory, costs 6 - 1 = 5 > 4 even
    // discounted once, so use two Bridges to reach 4): +$1 and +1 Card both apply.
    let mut g = new_state(&[id::IRONWORKS, id::HAREM, id::BRIDGE], 2);
    g.turn.cost_reduction = 2;
    set_hand(&mut g, 0, &[id::IRONWORKS]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::IRONWORKS);
    assert!(choices(&g).contains(&Choice::Card(id::HAREM)), "Harem costs 6 - 2 = 4");
    choose(&mut g, Choice::Card(id::HAREM));
    assert_eq!(g.turn.coins, 2, "+$1 from the Harem bonus, +$1 from the drawn Copper auto-playing");
    assert!(g.players[0].hand.is_empty());

    // Nothing affordable: Copper (cost 0) is always gainable, so this can't happen with the
    // basic supply present (C++'s "nothing to gain" case excludes the basic cards entirely,
    // which this engine always includes): not ported.
}

// ===========================================================================
// Trading Post — C++ TestTradingPost (line 1566)
// ===========================================================================

#[test]
fn trading_post_gains_a_silver_only_for_exactly_two_trashed() {
    let d = cards::def(id::TRADING_POST);
    assert_eq!((d.cost, d.vp), (5, 0));

    // Trash 2, gain a Silver to hand. Trading Post grants no Actions, so playing it (the only
    // action available) uses the turn's last action and every following step — both trash
    // picks (single legal choice each, since `min = max` leaves no Pass) and the Buy-phase
    // auto-play of the now-lone Silver in hand — happens automatically within `play` itself.
    let mut g = new_state(&[id::TRADING_POST], 2);
    set_hand(&mut g, 0, &[id::TRADING_POST, id::COPPER, id::ESTATE]);
    play(&mut g, id::TRADING_POST);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.turn.coins, 2, "the gained Silver auto-played");
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::ESTATE]));

    // Only 1 card in hand: trashes it (mandatory, clamped to hand size), gains nothing.
    let mut g = new_state(&[id::TRADING_POST], 2);
    set_hand(&mut g, 0, &[id::TRADING_POST, id::COPPER]);
    play(&mut g, id::TRADING_POST);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER]));

    // No cards in hand: nothing to trash, nothing gained.
    let mut g = new_state(&[id::TRADING_POST], 2);
    set_hand(&mut g, 0, &[id::TRADING_POST]);
    play(&mut g, id::TRADING_POST);
    assert!(g.players[0].hand.is_empty());
    assert!(g.trash.is_empty());

    // Must trash: with 3+ cards in hand there's no Pass (min = max = 2).
    let mut g = new_state(&[id::TRADING_POST], 2);
    set_hand(&mut g, 0, &[id::TRADING_POST, id::COPPER, id::SILVER, id::GOLD]);
    play(&mut g, id::TRADING_POST);
    assert!(!choices(&g).contains(&Choice::Pass));
}

// ===========================================================================
// Upgrade — C++ TestUpgrade (line 1670)
// ===========================================================================

#[test]
fn upgrade_trashes_and_gains_exactly_one_more() {
    let d = cards::def(id::UPGRADE);
    assert_eq!((d.cost, d.vp, d.cards, d.actions), (5, 0, 1, 1));

    // Estate ($2) -> Silver ($3). Shanty Town also costs $3, so both the trash (Estate vs.
    // Duchy) and the gain (Silver vs. Shanty Town) stay real decisions instead of being
    // auto-applied by `auto_single` (which only fires when exactly one distinct choice exists).
    let mut g = new_state(&[id::UPGRADE, id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::UPGRADE, id::ESTATE, id::DUCHY]);
    play(&mut g, id::UPGRADE);
    assert!(matches!(
        g.pending_decision().unwrap().upgrade,
        Some(Upgrade { plus: 1, filter: Filter::Any, dest: Dest::Discard, exact: true, dest_by_type: false })
    ));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Gain { max_cost: 3, filter: Filter::Any, dest: Dest::Discard, exact: true });
    assert!(!choices(&g).contains(&Choice::Card(id::COPPER)), "Copper costs 0, not exactly 3");
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));

    // Nothing costs exactly 1 (trashing a Copper, cost 0): gain is skipped, no decision — the
    // trash itself is also the only legal choice, so `play` alone resolves everything.
    let mut g = new_state(&[id::UPGRADE], 2);
    set_hand(&mut g, 0, &[id::UPGRADE, id::COPPER]);
    play(&mut g, id::UPGRADE);
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert!(g.players[0].discard.is_empty());

    // Nothing costs exactly 7 (trashing a Gold, cost 6): same.
    let mut g = new_state(&[id::UPGRADE], 2);
    set_hand(&mut g, 0, &[id::UPGRADE, id::GOLD]);
    play(&mut g, id::UPGRADE);
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
    assert!(g.players[0].discard.is_empty());

    // No cards to trash: mandatory trash is skipped (min clamps to hand size).
    let mut g = new_state(&[id::UPGRADE], 2);
    set_hand(&mut g, 0, &[id::UPGRADE]);
    play(&mut g, id::UPGRADE);
    assert!(g.trash.is_empty());

    // Bridge discount of 1: Copper (0 - 1, clamped to 0) trashes for "exactly 1", and Estate
    // (2 - 1 = 1) is the only card costing exactly 1 under the same discount, so it's gained
    // automatically too (single legal choice at every step).
    let mut g = new_state(&[id::UPGRADE, id::BRIDGE], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::UPGRADE, id::COPPER]);
    play(&mut g, id::UPGRADE);
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
}

#[test]
fn upgrade_with_throne_room_and_dual_type_gain() {
    // A known deck of 2 Duchies covers both resolutions' +1 Card without ever emptying the
    // deck — otherwise the Silver gained (to the discard) by the 1st resolution would reshuffle
    // in and get drawn by the 2nd resolution's +1 Card, which isn't what this test is about.
    let mut g = new_state(&[id::THRONE_ROOM, id::UPGRADE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::UPGRADE, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::UPGRADE));
    // 1st resolution: +1 Card (Duchy), trash Estate (2) -> gain exactly 3 (Silver, the only
    // card at that cost, auto-applied to the discard).
    choose(&mut g, Choice::Card(id::ESTATE));
    // 2nd resolution: +1 Card (2nd Duchy), trash Copper (0) -> gain exactly 1: nothing costs
    // exactly $1, so the gain is skipped.
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.trash, counts_of(&[id::ESTATE, id::COPPER]));
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER]));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::DUCHY]));
    assert_eq!(g.turn.actions, 2, "0 (Throne Room) + 1 + 1 (both Upgrade resolutions)");
}

// ===========================================================================
// Mill (Action-Victory, 1 VP) — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn mill_basics_and_discard_bonus() {
    let d = cards::def(id::MILL);
    assert_eq!((d.cost, d.vp, d.cards, d.actions), (4, 1, 1, 1));
    assert!(cards::is(id::MILL, cards::ACTION) && cards::is(id::MILL, cards::VICTORY));

    // Discards exactly 2 for +$2.
    let mut g = new_state(&[id::MILL], 2);
    set_hand(&mut g, 0, &[id::MILL, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::MILL);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::COPPER, id::ESTATE]), "+1 Card first");
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::ESTATE]));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));

    // All or nothing: with 2+ cards in hand you can't stop after discarding 1. Picks come in
    // card-id order, so the first pick must leave a second one (Estate, the highest id, can't be
    // picked first here).
    let mut g = new_state(&[id::MILL], 2);
    set_hand(&mut g, 0, &[id::MILL, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::MILL);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Card(id::SILVER), Choice::Pass]);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(!choices(&g).contains(&Choice::Pass), "committed: must discard a second card");
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.turn.coins, 2 + 2, "+$2 from Mill, then the Silver left in hand auto-plays");
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::ESTATE]));

    // Discards 0: no bonus. The Copper stays in hand only until the phase turns over: with no
    // Action left to play it then auto-plays into the Buy phase for +$1.
    let mut g = new_state(&[id::MILL], 2);
    set_hand(&mut g, 0, &[id::MILL, id::COPPER]);
    play(&mut g, id::MILL);
    pass(&mut g);
    assert_eq!(g.turn.coins, 1, "no Mill bonus (0 discarded), but the Copper auto-played");
    assert!(g.players[0].hand.is_empty());

    // Exactly 1 card in hand: may discard it, but gets no $ (never reaches 2).
    let mut g = new_state(&[id::MILL], 2);
    set_hand(&mut g, 0, &[id::MILL]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::MILL);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER]));
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.turn.coins, 0);
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER]));
}

#[test]
fn mill_with_throne_room() {
    // A known Duchy on top for each resolution's +1 Card: otherwise the discard pile from the
    // 1st resolution would reshuffle into the deck for the 2nd resolution's draw, drawing an
    // unplanned extra card (legitimate engine behavior, just not what this test is about).
    let mut g = new_state(&[id::THRONE_ROOM, id::MILL], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MILL, id::COPPER, id::COPPER, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MILL));
    // 1st resolution: +1 Card (Duchy), discard 2 Coppers for +$2.
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::COPPER));
    // 2nd resolution: +1 Card (Duchy), discard 2 Estates for +$2.
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.turn.coins, 4);
    assert_eq!(g.turn.actions, 2, "0 (Throne Room) + 1 + 1");
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::COPPER, id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::DUCHY]));
}

// ===========================================================================
// Patrol — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn patrol_moves_victory_and_curse_to_hand_reorders_the_rest() {
    let d = cards::def(id::PATROL);
    assert_eq!((d.cost, d.vp, d.cards), (5, 0, 3));

    // Two distinct non-Victory/Curse cards among the revealed 4 (Gold, Silver) keep the
    // put-back a real decision (a single leftover type would be auto-applied by `auto_single`,
    // as the 2nd case below relies on).
    let mut g = new_state(&[id::PATROL], 2);
    // With only 1 card left to place, the final topdeck pick would otherwise be auto-applied
    // before we get to make it explicitly below (same as `sentry_can_keep_both_and_choose_the_order`).
    g.auto_single = false;
    set_hand(&mut g, 0, &[id::PATROL]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::ESTATE, id::DUCHY, id::CURSE, id::GOLD, id::SILVER]);
    play(&mut g, id::PATROL);
    // +3 Cards drawn first (Copper, Estate, Duchy from the top), then the top 4 remaining are
    // revealed (Curse, Gold, Silver — the deck only had 3 left, so that's all of them): the
    // Curse goes straight to hand, Gold/Silver await reordering.
    let d2 = g.pending_decision().unwrap();
    assert!(matches!(d2.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Topdeck, ordered: true, .. }));
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::ESTATE, id::DUCHY, id::CURSE]), "Curse moved to hand as soon as revealing finished");
    assert!(g.players[0].deck_known.is_empty());
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::GOLD, id::SILVER], "the last one placed ends on top");
    // The stack is now empty and the Buy phase auto-plays the lone Copper left in hand.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::CURSE]));
    assert_eq!(g.turn.coins, 1);

    // Multiple Victory/Curse cards among the 4 revealed, leaving only Silver: a single leftover
    // type has only one legal (canonical) choice, so `auto_single` puts it back without a pause,
    // and the engine runs straight through to the Buy phase, auto-playing every treasure in hand.
    let mut g = new_state(&[id::PATROL], 2);
    set_hand(&mut g, 0, &[id::PATROL]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::ESTATE, id::DUCHY, id::CURSE, id::SILVER]);
    play(&mut g, id::PATROL);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER), "the only non-Victory/Curse card revealed goes back onto the deck alone");
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::CURSE]), "the 3x Copper auto-played as treasures; Silver went back onto the deck, not to hand");
    assert_eq!(g.turn.coins, 3, "3x Copper (+$3); Silver never reached hand");

    // Fewer than 4 cards left in the deck: reveals what's there and stops. All 3 remaining
    // cards are consumed by the +3 Cards draw itself, so nothing is left to reveal at all; the
    // 2 Coppers then auto-play in the Buy phase.
    let mut g = new_state(&[id::PATROL], 2);
    set_hand(&mut g, 0, &[id::PATROL]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::ESTATE]);
    play(&mut g, id::PATROL);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.turn.coins, 2);
    assert!(g.players[0].deck_known.is_empty());
    assert!(g.stack.is_empty());
}

#[test]
fn patrol_with_throne_room() {
    let mut g = new_state(&[id::THRONE_ROOM, id::PATROL], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::PATROL]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::ESTATE, id::COPPER, id::COPPER, id::COPPER, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::PATROL));
    // 1st resolution: +3 Cards (Copper x3), reveal 4 (Estate + 3 Copper): Estate to hand.
    assert!(g.players[0].hand.has(id::ESTATE));
    // Put the 3 Coppers back in some order (no real choice among identical cards).
    pick_while(&mut g, Zone::Revealed, Act::Topdeck, id::COPPER);
    // 2nd resolution: +3 Cards from what's now on top (the 3 re-topdecked Coppers), reveal 4
    // remaining: 3 Copper + Duchy -> Duchy to hand.
    assert!(g.players[0].hand.has(id::DUCHY));
    pick_while(&mut g, Zone::Revealed, Act::Topdeck, id::COPPER);
    assert!(g.players[0].deck_known.is_empty());
}

// ===========================================================================
// Replace (Action-Attack) — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn replace_action_or_treasure_goes_onto_the_deck() {
    let d = cards::def(id::REPLACE);
    assert_eq!((d.cost, d.vp), (5, 0));
    assert!(cards::is(id::REPLACE, cards::ACTION) && cards::is(id::REPLACE, cards::ATTACK));

    // Trash Estate (2, keeping Duchy in hand as a 2nd distinct trash option so the pick stays a
    // real decision), gain up to 4: an Action (Village) goes onto the deck.
    let mut g = new_state(&[id::REPLACE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::ESTATE, id::DUCHY]);
    play(&mut g, id::REPLACE);
    assert_eq!(g.pending_decision().unwrap().upgrade, Some(Upgrade { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: true }));
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::VILLAGE));
    assert!(g.players[0].discard.is_empty(), "went onto the deck, not the discard");
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));

    // A Treasure gain (Gold, trashing a Duchy to reach it) also goes onto the deck.
    let mut g = new_state(&[id::REPLACE], 2);
    set_hand(&mut g, 0, &[id::REPLACE, id::DUCHY]);
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
}

#[test]
fn replace_victory_gain_curses_other_players_unless_moat() {
    // Trash a Gold (cap 6 + 2 = 8) so a Victory card up to Province is reachable. A Victory
    // gain (Duchy) curses each other player instead of going onto the deck.
    let mut g = new_state(&[id::REPLACE], 3);
    set_hand(&mut g, 0, &[id::REPLACE, id::GOLD]);
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY]), "Victory gains go to the normal destination (discard), not the deck");
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
    assert_eq!(g.players[2].discard, counts_of(&[id::CURSE]));

    // Moat blocks the curse for whoever reveals it.
    let mut g = new_state(&[id::REPLACE, id::MOAT], 3);
    set_hand(&mut g, 0, &[id::REPLACE, id::GOLD]);
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(g.players[1].discard.is_empty(), "player 1 revealed Moat");
    assert_eq!(g.players[2].discard, counts_of(&[id::CURSE]), "player 2 has no Moat");

    // No Curses left: nothing is gained, no crash.
    let mut g = new_state(&[id::REPLACE], 2);
    empty_pile(&mut g, id::CURSE);
    set_hand(&mut g, 0, &[id::REPLACE, id::GOLD]);
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(g.players[1].discard.is_empty());
}

#[test]
fn replace_dual_type_harem_goes_on_the_deck_and_curses() {
    // Harem is both a Treasure and a Victory card: it goes onto the deck AND curses. Trash a
    // Gold (cap 6 + 2 = 8) to reach Harem's $6.
    let mut g = new_state(&[id::REPLACE, id::HAREM], 3);
    set_hand(&mut g, 0, &[id::REPLACE, id::GOLD]);
    play(&mut g, id::REPLACE);
    choose(&mut g, Choice::Card(id::HAREM));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::HAREM));
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
    assert_eq!(g.players[2].discard, counts_of(&[id::CURSE]));
}

#[test]
fn replace_with_bridge_discount_and_throne_room() {
    // Bridge discount applies to both the trash-derived cap and the gain itself.
    let mut g = new_state(&[id::REPLACE, id::BRIDGE], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::REPLACE, id::SILVER]);
    play(&mut g, id::REPLACE);
    // Silver ($3 - 1 = $2) is the only card in hand, so trashing it is auto-applied; up to +2
    // more = $4; Duchy costs 5 - 1 = 4, exactly reachable.
    assert!(choices(&g).contains(&Choice::Card(id::DUCHY)));
    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));

    // Throne Room plays Replace twice: trash twice (Estate, $2, auto-applied each time since
    // it's the only card in hand at that point), gain twice (up to $2 + $2 = $4, reaching
    // Village).
    let mut g = new_state(&[id::THRONE_ROOM, id::REPLACE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::REPLACE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::REPLACE));
    choose(&mut g, Choice::Card(id::VILLAGE));
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.trash, counts_of(&[id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::VILLAGE, id::VILLAGE]);
}

// ===========================================================================
// Mode decisions ("choose one/many"): Pawn, Steward, Nobles, Minion, Courtier, Lurker,
// Torturer's victim. See `cards::modes` / `state::FrameKind::Mode`.
// ===========================================================================

/// Index of the table entry matching `pred`, for readable test code.
fn mode_idx(card: CardId, pred: impl Fn(&ModeOpt) -> bool) -> u8 {
    cards::modes(card).iter().position(|o| pred(o)).expect("mode option exists") as u8
}

// ===========================================================================
// Pawn — C++ TestPawn (line 110)
// ===========================================================================

#[test]
fn pawn_two_different_bonuses() {
    let d = cards::def(id::PAWN);
    assert_eq!((d.cost, d.vp), (2, 0));
    assert_eq!(cards::modes(id::PAWN).len(), 4);
    let cards_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Cards(_)));
    let actions_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Actions(_)));
    let buys_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Buys(_)));
    let coins_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Coins(_)));

    // Actions, Buys.
    let mut g = new_state(&[id::PAWN], 2);
    set_hand(&mut g, 0, &[id::PAWN]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::PAWN);
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.kind, DecisionKind::Mode { picks: 2, distinct: true });
    assert_eq!(choices(&g).len(), 4);
    choose(&mut g, Choice::Mode(actions_i));
    assert!(!choices(&g).contains(&Choice::Mode(actions_i)), "can't pick the same option twice");
    choose(&mut g, Choice::Mode(buys_i));
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (1, 2, 0));
    assert!(g.players[0].hand.is_empty());

    // Coins, Buys.
    let mut g = new_state(&[id::PAWN], 2);
    set_hand(&mut g, 0, &[id::PAWN]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::PAWN);
    choose(&mut g, Choice::Mode(coins_i));
    choose(&mut g, Choice::Mode(buys_i));
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 2, 1));
    assert!(g.players[0].hand.is_empty());

    // Cards, Coins: the +1 Card must not resolve before the 2nd pick is made.
    let mut g = new_state(&[id::PAWN], 2);
    set_hand(&mut g, 0, &[id::PAWN]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::PAWN);
    choose(&mut g, Choice::Mode(cards_i));
    assert!(g.players[0].hand.is_empty(), "not drawn until the 2nd pick is chosen too");
    choose(&mut g, Choice::Mode(coins_i));
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 1, 1));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.players[0].deck_known.is_empty());

    // Picking the same option twice (the C++ "Coins, Coins" / bad-choice cases) is illegal.
    let mut g = new_state(&[id::PAWN], 2);
    set_hand(&mut g, 0, &[id::PAWN]);
    play(&mut g, id::PAWN);
    choose(&mut g, Choice::Mode(coins_i));
    assert!(g.apply(Choice::Mode(coins_i), &mut NoEvents).is_err(), "repeated pick rejected");
    assert!(g.apply(Choice::Mode(9), &mut NoEvents).is_err(), "out-of-range index rejected");
}

// ===========================================================================
// Nobles — C++ TestNobles (line 730)
// ===========================================================================

#[test]
fn nobles_choose_cards_or_actions() {
    let d = cards::def(id::NOBLES);
    assert_eq!((d.cost, d.vp), (6, 2));
    assert!(cards::is(id::NOBLES, cards::ACTION) && cards::is(id::NOBLES, cards::VICTORY));
    let cards_i = mode_idx(id::NOBLES, |o| matches!(o, ModeOpt::Cards(_)));
    let actions_i = mode_idx(id::NOBLES, |o| matches!(o, ModeOpt::Actions(_)));

    // +3 Cards. Non-Treasure fillers throughout: with 0 actions left after Nobles, any Treasure
    // drawn into hand would auto-play into the Buy phase, which isn't what these cases test.
    let mut g = new_state(&[id::NOBLES], 2);
    set_hand(&mut g, 0, &[id::NOBLES]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::VILLAGE, id::DUCHY, id::DUCHY]);
    play(&mut g, id::NOBLES);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Mode { picks: 1, distinct: false });
    choose(&mut g, Choice::Mode(cards_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::VILLAGE, id::DUCHY]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::DUCHY]);
    assert_eq!(g.turn.actions, 0);

    // +2 Actions.
    let mut g = new_state(&[id::NOBLES], 2);
    set_hand(&mut g, 0, &[id::NOBLES]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::VILLAGE, id::DUCHY, id::DUCHY]);
    play(&mut g, id::NOBLES);
    choose(&mut g, Choice::Mode(actions_i));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.turn.actions, 2);

    // 2 cards left in deck.
    let mut g = new_state(&[id::NOBLES], 2);
    set_hand(&mut g, 0, &[id::NOBLES]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::VILLAGE]);
    play(&mut g, id::NOBLES);
    choose(&mut g, Choice::Mode(cards_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::VILLAGE]));
    assert!(g.players[0].deck_known.is_empty());

    // No cards left.
    let mut g = new_state(&[id::NOBLES], 2);
    set_hand(&mut g, 0, &[id::NOBLES]);
    play(&mut g, id::NOBLES);
    choose(&mut g, Choice::Mode(cards_i));
    assert!(g.players[0].hand.is_empty());

    // An illegal mode index is rejected.
    let mut g = new_state(&[id::NOBLES], 2);
    set_hand(&mut g, 0, &[id::NOBLES]);
    play(&mut g, id::NOBLES);
    assert!(g.apply(Choice::Mode(2), &mut NoEvents).is_err());
}

// ===========================================================================
// Steward — C++ TestSteward (line 775)
// ===========================================================================

#[test]
fn steward_choose_one_of_three() {
    let d = cards::def(id::STEWARD);
    assert_eq!((d.cost, d.vp), (3, 0));
    let cards_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::Cards(_)));
    let coins_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::Coins(_)));
    let trash_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::TrashFromHand(_)));

    // Coins.
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD, id::DUCHY, id::ESTATE]);
    play(&mut g, id::STEWARD);
    choose(&mut g, Choice::Mode(coins_i));
    assert_eq!((g.turn.actions, g.turn.coins), (0, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::ESTATE]));

    // Cards. Non-Treasure fillers: with 0 actions left after Steward, any Treasure in hand
    // would auto-play into the Buy phase, which isn't what these cases test.
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD, id::VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::DUCHY, id::DUCHY]);
    play(&mut g, id::STEWARD);
    choose(&mut g, Choice::Mode(cards_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::DUCHY, id::DUCHY]);

    // Draw 1 card (only 1 left in deck).
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD, id::VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::STEWARD);
    choose(&mut g, Choice::Mode(cards_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::ESTATE, id::DUCHY]));

    // Draw 0 cards.
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD, id::VILLAGE, id::ESTATE]);
    play(&mut g, id::STEWARD);
    choose(&mut g, Choice::Mode(cards_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE, id::ESTATE]));

    // Trash 2: a real choice among 3 cards (the last one left, DUCHY, isn't a Treasure either,
    // so it's stable in hand once the stack empties).
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD, id::COPPER, id::ESTATE, id::DUCHY]);
    play(&mut g, id::STEWARD);
    choose(&mut g, Choice::Mode(trash_i));
    assert!(!choices(&g).contains(&Choice::Pass), "must trash 2");
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::ESTATE]));

    // Trash 1 card (fewer than 2 in hand).
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD, id::DUCHY]);
    play(&mut g, id::STEWARD);
    choose(&mut g, Choice::Mode(trash_i));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::DUCHY]));

    // Trash no cards (empty hand): nothing to trash.
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD]);
    play(&mut g, id::STEWARD);
    choose(&mut g, Choice::Mode(trash_i));
    assert!(g.players[0].hand.is_empty());
    assert!(g.stack.is_empty());

    // An illegal mode index (e.g. a made-up 4th option) is rejected.
    let mut g = new_state(&[id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::STEWARD]);
    play(&mut g, id::STEWARD);
    assert!(g.apply(Choice::Mode(3), &mut NoEvents).is_err());
}

// ===========================================================================
// Minion — C++ TestMinion (line 2518)
// ===========================================================================

#[test]
fn minion_coins_or_new_hands() {
    let d = cards::def(id::MINION);
    assert_eq!((d.cost, d.vp, d.actions), (5, 0, 1));
    assert!(cards::is(id::MINION, cards::ATTACK));
    let coins_i = mode_idx(id::MINION, |o| matches!(o, ModeOpt::Coins(_)));
    let hand_i = mode_idx(id::MINION, |o| matches!(o, ModeOpt::DiscardHandDraw { .. }));

    // +$2: nothing changes for the other player.
    let mut g = new_state(&[id::MINION], 2);
    set_hand(&mut g, 0, &[id::MINION, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::MINION);
    choose(&mut g, Choice::Mode(coins_i));
    assert_eq!((g.turn.actions, g.turn.coins), (1, 2));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]));

    // Discard hand, +4 Cards; the other player (5+ cards) does the same.
    let mut g = new_state(&[id::MINION], 2);
    set_hand(&mut g, 0, &[id::MINION, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    play(&mut g, id::MINION);
    choose(&mut g, Choice::Mode(hand_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[1].hand, counts_of(&[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]));
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]));
    assert_eq!(g.turn.actions, 1);

    // Other player has fewer than 5 cards: unaffected.
    let mut g = new_state(&[id::MINION], 2);
    set_hand(&mut g, 0, &[id::MINION, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::MINION);
    choose(&mut g, Choice::Mode(hand_i));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]), "fewer than 5 cards: unaffected");
    assert!(g.players[1].discard.is_empty());

    // Moat blocks the attack part (revealed only because this option was actually chosen).
    let mut g = new_state(&[id::MINION, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MINION, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::MINION);
    choose(&mut g, Choice::Mode(hand_i));
    assert_eq!(g.players[1].hand, counts_of(&[id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]), "Moat blocked the attack");
    assert!(g.players[1].discard.is_empty());

    // An illegal mode index is rejected.
    let mut g = new_state(&[id::MINION], 2);
    set_hand(&mut g, 0, &[id::MINION]);
    play(&mut g, id::MINION);
    assert!(g.apply(Choice::Mode(2), &mut NoEvents).is_err());
}

// ===========================================================================
// Torturer — C++ TestTorturer (line 1376)
// ===========================================================================

#[test]
fn torturer_victim_discards_or_gains_a_curse() {
    let d = cards::def(id::TORTURER);
    assert_eq!((d.cost, d.vp, d.cards), (5, 0, 3));
    assert!(cards::is(id::TORTURER, cards::ATTACK));
    assert_eq!(cards::def(id::TORTURER).on_play, cards::OnPlay::ChoiceFree, "only the victims choose");
    let discard_i = mode_idx(id::TORTURER, |o| matches!(o, ModeOpt::DiscardFromHand(_)));
    let curse_i = mode_idx(id::TORTURER, |o| matches!(o, ModeOpt::Gain(c, _) if *c == id::CURSE));

    // Draw 3 cards; other player gains a curse.
    let mut g = new_state(&[id::TORTURER], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::TORTURER);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::SILVER, id::GOLD]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::VILLAGE]);
    let dv = g.pending_decision().unwrap();
    assert_eq!(dv.player, 1);
    assert_eq!(dv.source, Some(id::TORTURER));
    choose(&mut g, Choice::Mode(curse_i));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE, id::CURSE]));

    // Other player tries to gain a curse, but none are left: no change.
    let mut g = new_state(&[id::TORTURER], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    empty_pile(&mut g, id::CURSE);
    play(&mut g, id::TORTURER);
    choose(&mut g, Choice::Mode(curse_i));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::ESTATE, id::ESTATE]));

    // Other player discards 2 cards.
    let mut g = new_state(&[id::TORTURER], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::TORTURER);
    choose(&mut g, Choice::Mode(discard_i));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE, id::ESTATE]));

    // Other player discards 1 card (fewer than 2 in hand).
    let mut g = new_state(&[id::TORTURER], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::TORTURER);
    choose(&mut g, Choice::Mode(discard_i));
    assert!(g.players[1].hand.is_empty());
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE]));

    // Other player discards no cards (empty hand): no change.
    let mut g = new_state(&[id::TORTURER], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE]);
    play(&mut g, id::TORTURER);
    choose(&mut g, Choice::Mode(discard_i));
    assert!(g.players[1].hand.is_empty());

    // Only 2 cards to draw.
    let mut g = new_state(&[id::TORTURER], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER]);
    set_hand(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::TORTURER);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::SILVER]));
    assert!(g.players[0].deck_known.is_empty());
    choose(&mut g, Choice::Mode(curse_i));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::CURSE]));

    // 3 players: each victim decides independently, in turn order.
    let mut g = new_state(&[id::TORTURER], 3);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE]);
    set_hand(&mut g, 2, &[id::DUCHY, id::DUCHY]);
    play(&mut g, id::TORTURER);
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.player, 1);
    choose(&mut g, Choice::Mode(discard_i));
    let d2 = g.pending_decision().unwrap();
    assert_eq!(d2.player, 2);
    choose(&mut g, Choice::Mode(curse_i));
    assert!(g.players[1].hand.is_empty());
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[2].hand, counts_of(&[id::DUCHY, id::DUCHY, id::CURSE]));
}

// ===========================================================================
// Courtier — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn courtier_reveals_and_chooses_per_type() {
    let d = cards::def(id::COURTIER);
    assert_eq!((d.cost, d.vp), (5, 0));
    let actions_i = mode_idx(id::COURTIER, |o| matches!(o, ModeOpt::Actions(_)));
    let buys_i = mode_idx(id::COURTIER, |o| matches!(o, ModeOpt::Buys(_)));
    let coins_i = mode_idx(id::COURTIER, |o| matches!(o, ModeOpt::Coins(_)));
    let gold_i = mode_idx(id::COURTIER, |o| matches!(o, ModeOpt::Gain(c, _) if *c == id::GOLD));

    // A Curse (1 type): 1 pick. Two distinct cards in hand keep the reveal a real decision.
    // Non-Treasure filler (Estate): with 0 actions left after Courtier, a Treasure would
    // auto-play into the Buy phase once the stack empties, which isn't what this tests.
    let mut g = new_state(&[id::COURTIER], 2);
    set_hand(&mut g, 0, &[id::COURTIER, id::CURSE, id::ESTATE]);
    play(&mut g, id::COURTIER);
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Reveal, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.players[0].hand, counts_of(&[id::CURSE, id::ESTATE]), "revealing doesn't remove the card");
    let dm = g.pending_decision().unwrap();
    assert_eq!(dm.kind, DecisionKind::Mode { picks: 1, distinct: false });
    choose(&mut g, Choice::Mode(coins_i));
    assert_eq!(g.turn.coins, 3);

    // Mill (Action-Victory, 2 types): 2 distinct picks.
    let mut g = new_state(&[id::COURTIER, id::MILL], 2);
    set_hand(&mut g, 0, &[id::COURTIER, id::MILL, id::ESTATE]);
    play(&mut g, id::COURTIER);
    choose(&mut g, Choice::Card(id::MILL));
    assert_eq!(g.players[0].hand, counts_of(&[id::MILL, id::ESTATE]));
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Mode { picks: 2, distinct: true });
    choose(&mut g, Choice::Mode(buys_i));
    choose(&mut g, Choice::Mode(gold_i));
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));

    // Witch (Action-Attack, 2 types).
    let mut g = new_state(&[id::COURTIER, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::COURTIER, id::WITCH, id::ESTATE]);
    play(&mut g, id::COURTIER);
    choose(&mut g, Choice::Card(id::WITCH));
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Mode { picks: 2, distinct: true });
    choose(&mut g, Choice::Mode(actions_i));
    choose(&mut g, Choice::Mode(coins_i));
    assert_eq!((g.turn.actions, g.turn.coins), (1, 3));

    // No card in hand: nothing happens.
    let mut g = new_state(&[id::COURTIER], 2);
    set_hand(&mut g, 0, &[id::COURTIER]);
    play(&mut g, id::COURTIER);
    assert!(g.players[0].hand.is_empty());
    assert!(g.stack.is_empty());
}

// ===========================================================================
// Lurker — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn lurker_trashes_from_supply_or_gains_from_trash() {
    let d = cards::def(id::LURKER);
    assert_eq!((d.cost, d.vp, d.actions), (2, 0, 1));
    let trash_i = mode_idx(id::LURKER, |o| matches!(o, ModeOpt::TrashFromSupply(_)));
    let gain_i = mode_idx(id::LURKER, |o| matches!(o, ModeOpt::GainFromTrash(_)));

    // Trash an Action card from the Supply.
    let mut g = new_state(&[id::LURKER, id::VILLAGE, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::LURKER]);
    play(&mut g, id::LURKER);
    assert_eq!(g.turn.actions, 1, "+1 Action, net of the 1 spent to play Lurker itself");
    choose(&mut g, Choice::Mode(trash_i));
    let dv = g.pending_decision().unwrap();
    assert_eq!(dv.kind, DecisionKind::Select { from: Zone::Supply, act: Act::Trash, filter: Filter::Action, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.trash, counts_of(&[id::VILLAGE]));
    assert_eq!(g.supply.get(id::VILLAGE), 9);

    // Gain an Action card from the trash. Two distinct Action cards in the trash (Village,
    // Market) keep it a real decision.
    let mut g = new_state(&[id::LURKER], 2);
    g.trash = counts_of(&[id::VILLAGE, id::MARKET, id::ESTATE]);
    set_hand(&mut g, 0, &[id::LURKER]);
    play(&mut g, id::LURKER);
    choose(&mut g, Choice::Mode(gain_i));
    let dv = g.pending_decision().unwrap();
    assert_eq!(dv.kind, DecisionKind::Select { from: Zone::Trash, act: Act::Gain, filter: Filter::Action, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.players[0].discard, counts_of(&[id::VILLAGE]));
    assert_eq!(g.trash, counts_of(&[id::MARKET, id::ESTATE]));

    // Nothing eligible to trash (no Action cards left in the Supply): skipped silently.
    let mut g = new_state(&[id::LURKER], 2);
    empty_pile(&mut g, id::LURKER);
    set_hand(&mut g, 0, &[id::LURKER]);
    play(&mut g, id::LURKER);
    choose(&mut g, Choice::Mode(trash_i));
    assert!(g.trash.is_empty());

    // Nothing eligible to gain (the trash has no Action card): skipped silently.
    let mut g = new_state(&[id::LURKER], 2);
    g.trash = counts_of(&[id::ESTATE, id::COPPER]);
    set_hand(&mut g, 0, &[id::LURKER]);
    play(&mut g, id::LURKER);
    choose(&mut g, Choice::Mode(gain_i));
    assert!(g.players[0].discard.is_empty());
}

// ===========================================================================
// Throne Room with Pawn / Steward / Nobles / Minion
// ===========================================================================

#[test]
fn throne_room_with_mode_cards() {
    // Pawn: two separate mode decisions, one per resolution.
    let cards_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Cards(_)));
    let coins_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Coins(_)));
    let mut g = new_state(&[id::THRONE_ROOM, id::PAWN], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::PAWN]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::PAWN));
    choose(&mut g, Choice::Mode(cards_i));
    choose(&mut g, Choice::Mode(coins_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    choose(&mut g, Choice::Mode(cards_i));
    choose(&mut g, Choice::Mode(coins_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.turn.coins, 2);

    // Steward twice: +$2 then trash 2.
    let steward_coins_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::Coins(_)));
    let steward_trash_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::TrashFromHand(_)));
    let mut g = new_state(&[id::THRONE_ROOM, id::STEWARD], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::STEWARD, id::COPPER, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::STEWARD));
    choose(&mut g, Choice::Mode(steward_coins_i));
    choose(&mut g, Choice::Mode(steward_trash_i));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::ESTATE]));
    assert_eq!(g.turn.coins, 2);

    // Nobles twice: +3 Cards then +2 Actions.
    let nobles_cards_i = mode_idx(id::NOBLES, |o| matches!(o, ModeOpt::Cards(_)));
    let nobles_actions_i = mode_idx(id::NOBLES, |o| matches!(o, ModeOpt::Actions(_)));
    let mut g = new_state(&[id::THRONE_ROOM, id::NOBLES], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::NOBLES]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::NOBLES));
    choose(&mut g, Choice::Mode(nobles_cards_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::COPPER, id::COPPER]));
    choose(&mut g, Choice::Mode(nobles_actions_i));
    assert_eq!(g.turn.actions, 2, "0 (Throne Room) + 2 (2nd Nobles resolution)");

    // Minion twice: +$2 then discard hand/draw 4.
    let minion_coins_i = mode_idx(id::MINION, |o| matches!(o, ModeOpt::Coins(_)));
    let minion_hand_i = mode_idx(id::MINION, |o| matches!(o, ModeOpt::DiscardHandDraw { .. }));
    let mut g = new_state(&[id::THRONE_ROOM, id::MINION], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MINION, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MINION));
    choose(&mut g, Choice::Mode(minion_coins_i));
    assert_eq!(g.turn.coins, 2);
    choose(&mut g, Choice::Mode(minion_hand_i));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE, id::ESTATE]));
    assert_eq!(g.turn.actions, 2, "0 (Throne Room) + 1 + 1, both Minion resolutions");
}
