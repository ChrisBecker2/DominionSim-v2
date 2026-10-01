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
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Gain { max_cost: 4, filter: Filter::Any, dest: Dest::Discard, exact: false, potion: false, optional: false });
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
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Gain { max_cost: 3, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
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

// ===========================================================================
// Wishing Well — C++ TestWishingWell (line 2407)
// ===========================================================================

#[test]
fn wishing_well_names_hit_and_miss() {
    // C++ TestWishingWell, "Didn't wish for the right thing" / "Wished for the right thing"
    // (lines 2420, 2430). Non-Treasure fillers throughout (Duchy/Province/Estate) in place of the
    // C++ case's Estate/Village/Copper/Gold: with no Action left to play after Wishing Well, any
    // Treasure drawn into hand would auto-play into the Buy phase once the stack empties, which
    // isn't what these cases test.
    let d = cards::def(id::WISHING_WELL);
    assert_eq!((d.cost, d.cards, d.actions), (3, 1, 1));

    // Hit: the named card matches the real top and goes to hand.
    let mut g = new_state(&[id::WISHING_WELL], 2);
    set_hand(&mut g, 0, &[id::WISHING_WELL]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::ESTATE]);
    play(&mut g, id::WISHING_WELL);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]), "+1 Card first");
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.kind, DecisionKind::Name);
    // Offered in ascending card-id order: Estate (3) before Province (5); Duchy is already drawn.
    assert_eq!(choices(&g), vec![Choice::Card(id::ESTATE), Choice::Card(id::PROVINCE)]);
    choose(&mut g, Choice::Card(id::PROVINCE));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::ESTATE]);
    assert_eq!(g.turn.actions, 1);

    // Miss: the named card stays on top, now known, but doesn't go to hand.
    let mut g = new_state(&[id::WISHING_WELL], 2);
    set_hand(&mut g, 0, &[id::WISHING_WELL]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::ESTATE]);
    play(&mut g, id::WISHING_WELL);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::PROVINCE, id::ESTATE]);
}

#[test]
fn wishing_well_port_wished_for_the_right_thing_out_of_discard() {
    // C++ TestWishingWell, "Wished for the right thing out of discard" (line 2450): the +1 Card
    // draws the deck's only card, then Wishing Well itself names correctly out of the reshuffled
    // discard. The commented-out sibling case ("Didn't wish...out of discard", line 2440) is
    // disabled in the C++ source itself, so it isn't ported. Duchy in place of the C++ case's
    // Gold: with no Action left to play, a Treasure drawn into hand would auto-play into the Buy
    // phase once the stack empties, which isn't what this tests.
    let mut g = new_state(&[id::WISHING_WELL], 2);
    set_hand(&mut g, 0, &[id::WISHING_WELL]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    set_discard(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::WISHING_WELL);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]), "got the wish");
    assert!(g.players[0].deck_known.is_empty() && g.players[0].deck_unknown.is_empty());
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn wishing_well_port_one_card_left_cannot_wish_for_anything() {
    // C++ TestWishingWell, "1 card left, can't wish for anything" / "1 card left in discard,
    // can't wish for anything" (lines 2461, 2470): the +1 Card draw consumes the only available
    // card, leaving nothing to name. ("0 cards left", line 2479, is
    // `wishing_well_empty_deck_and_discard_skips_naming` below.)
    let mut g = new_state(&[id::WISHING_WELL], 2);
    set_hand(&mut g, 0, &[id::WISHING_WELL]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::WISHING_WELL);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g.stack.is_empty(), "nothing left to name");

    let mut g2 = new_state(&[id::WISHING_WELL], 2);
    set_hand(&mut g2, 0, &[id::WISHING_WELL]);
    set_discard(&mut g2, 0, &[id::ESTATE]);
    play(&mut g2, id::WISHING_WELL);
    assert_eq!(g2.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g2.players[0].discard.is_empty());
    assert!(g2.stack.is_empty(), "nothing left to name");
}

#[test]
fn wishing_well_empty_deck_and_discard_skips_naming() {
    // C++ TestWishingWell, "0 cards left, can't wish for anything" (line 2479).
    let mut g = new_state(&[id::WISHING_WELL], 2);
    set_hand(&mut g, 0, &[id::WISHING_WELL]);
    play(&mut g, id::WISHING_WELL);
    assert!(g.players[0].hand.is_empty());
    assert!(g.stack.is_empty(), "nothing to name or reveal");
    assert_eq!(g.turn.actions, 1);
}

// Not ported: the "Simulation" sub-case (line 2489, Scout + Wishing Well) is a bot-harness case
// that also relies on Scout, a 1st-edition-only card not in this project's scope; the commented-out
// TODO case right after it (line 2504) is disabled in the C++ source itself.

#[test]
fn wishing_well_reshuffles_the_discard_to_name_from() {
    // Deck fully empty, discard has 2 cards: the +1 Card draw itself reshuffles and samples one
    // at random; only one distinct card is then left to name, so the result is guaranteed (and
    // deterministic) regardless of which one the reshuffle happened to draw first.
    let mut g = new_state(&[id::WISHING_WELL], 2);
    set_hand(&mut g, 0, &[id::WISHING_WELL]);
    set_discard(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::WISHING_WELL);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE]), "both end up in hand either way");
    assert!(g.players[0].discard.is_empty());
    assert!(g.players[0].deck_known.is_empty() && g.players[0].deck_unknown.is_empty());
}

// ===========================================================================
// Swindler (Action-Attack) — C++ TestSwindler (line 2130)
// ===========================================================================

#[test]
fn swindler_attacker_picks_the_victims_gain_at_the_trashed_cost() {
    // C++ TestSwindler, `VerifyBasics` (line 2132).
    let d = cards::def(id::SWINDLER);
    assert_eq!((d.cost, d.coins), (3, 2));
    assert!(cards::is(id::SWINDLER, cards::ATTACK));

    let mut g = new_state(&[id::SWINDLER, id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::SILVER]);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash, counts_of(&[id::SILVER]));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 0, "the attacker decides");
    assert_eq!(d.for_player, 1, "the victim receives the gain");
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 3, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    assert!(choices(&g).contains(&Choice::Card(id::SILVER)) && choices(&g).contains(&Choice::Card(id::SHANTY_TOWN)));
    choose(&mut g, Choice::Card(id::SHANTY_TOWN));
    assert_eq!(g.players[1].discard, counts_of(&[id::SHANTY_TOWN]));
    assert!(g.players[1].hand.is_empty());
}

#[test]
fn swindler_port_copper_for_curse() {
    // C++ TestSwindler, "Swindle a Copper for a Curse" (line 2135).
    let mut g = new_state(&[id::SWINDLER], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::VILLAGE]);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 0, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    assert!(choices(&g).contains(&Choice::Card(id::CURSE)) && choices(&g).contains(&Choice::Card(id::COPPER)));
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::VILLAGE));
}

#[test]
fn swindler_port_village_for_silver() {
    // C++ TestSwindler, "Swindle a Village for a Silver" (line 2152). The C++ kingdom pairing
    // (Village, Woodcutter -- Woodcutter isn't in this engine) just gives Silver a 2nd cost-3
    // option to choose over; Village's own always-in-supply pile (once it's a kingdom card here)
    // already does that.
    let mut g = new_state(&[id::SWINDLER, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::VILLAGE, id::ESTATE]);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::VILLAGE]));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 3, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[1].discard, counts_of(&[id::SILVER]));
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::ESTATE));
}

#[test]
fn swindler_port_must_swindle_estate_for_estate() {
    // C++ TestSwindler, "Must Swindle an Estate for an Estate" (line 2171): nothing else costs
    // 2, so the only legal gain is another Estate, auto-applied.
    let mut g = new_state(&[id::SWINDLER], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::ESTATE, id::VILLAGE]);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::VILLAGE));
}

#[test]
fn swindler_port_discount_gives_the_same_relative_choice() {
    // C++ TestSwindler, "Swindle a Village for a Silver with a discount (should be exact same
    // result as above)" (line 2189): a uniform discount shifts every cost by the same amount, so
    // the same two options (Village, Silver) remain the choice, just at a lower price.
    let mut g = new_state(&[id::SWINDLER, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::VILLAGE, id::ESTATE]);
    g.turn.cost_reduction = 1;
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::VILLAGE]));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 2, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[1].discard, counts_of(&[id::SILVER]));
}

#[test]
fn swindler_port_discount_allows_estate_to_curse() {
    // C++ TestSwindler, "Swindle with discount allows for Estate to Curse" (line 2209).
    let mut g = new_state(&[id::SWINDLER], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::ESTATE, id::VILLAGE]);
    g.turn.cost_reduction = 2;
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 0, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
}

#[test]
fn swindler_port_extreme_discount_makes_every_card_swindleable() {
    // C++ TestSwindler, "Swindle with discount allows for Province to Curse" (line 2227): a
    // discount of 8 floors every card's cost at 0, so the whole supply -- not just cards that
    // happen to cost 0 -- becomes a legal (and equally worst-case) gain ("All cards can be
    // Swindled too", per the C++ comment).
    let mut g = new_state(&[id::SWINDLER], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::PROVINCE, id::VILLAGE]);
    g.turn.cost_reduction = 8;
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::PROVINCE]));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 0, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    for c in [id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY, id::PROVINCE, id::CURSE, id::SWINDLER] {
        assert!(choices(&g).contains(&Choice::Card(c)), "{} should cost 0 under an 8-cost reduction", cards::name(c));
    }
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.players[1].discard, counts_of(&[id::CURSE]));
}

#[test]
fn swindler_port_no_cards_to_gain() {
    // C++ TestSwindler, "No cards to gain" (line 2245): there the whole supply was built without
    // any basic cards, so nothing cost as much as the trashed Gold; the 7 basics can't be removed
    // in this engine, so emptying Gold's own pile (nothing else in the base/Intrigue supply costs
    // 6) reaches the same "nothing to gain" state.
    let mut g = new_state(&[id::SWINDLER], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::VILLAGE]);
    empty_pile(&mut g, id::GOLD);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
    assert!(g.players[1].discard.is_empty());
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::VILLAGE));
}

// Not ported from TestSwindler: the Potions (lines 2277-2330) and Debt (lines 2332-2386) cases
// exercise currencies this project doesn't implement, and several use cards from other
// expansions (Transmute, Apothecary, ScryingPool, Engineer, CityQuarter, RoyalBlacksmith,
// Fortune) outside Base/Intrigue 2nd edition. "Solo play" (line 2389) needs a 1-player game,
// which this engine doesn't support (minimum 2 players); its externally observable behavior --
// +$2, nothing trashed, no victims -- is already covered by `swindler_moat_blocks_it_entirely`.
// "Simulation - Gets played" (line 2398) is a bot-harness case.

#[test]
fn swindler_moat_blocks_it_entirely() {
    let mut g = new_state(&[id::SWINDLER, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 1, &[id::SILVER]);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.turn.coins, 2, "Swindler's own +$2 always applies");
    assert!(g.trash.is_empty(), "Moat blocks the trash");
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::SILVER));
}

#[test]
fn swindler_with_bridge_uses_the_reduced_cost() {
    let mut g = new_state(&[id::SWINDLER, id::BRIDGE, id::SHANTY_TOWN], 2);
    set_hand(&mut g, 0, &[id::BRIDGE, id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::SILVER]);
    g.turn.actions = 2; // enough to play both Bridge (which grants none) and Swindler
    play(&mut g, id::BRIDGE);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::SILVER]));
    let d = g.pending_decision().unwrap();
    assert_eq!(
        d.kind,
        DecisionKind::Gain { max_cost: 2, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false },
        "Silver's cost, reduced by Bridge"
    );
    assert!(choices(&g).contains(&Choice::Card(id::SHANTY_TOWN)), "Shanty Town also costs 3, i.e. 2 after the reduction");
    choose(&mut g, Choice::Card(id::SHANTY_TOWN));
    assert_eq!(g.players[1].discard, counts_of(&[id::SHANTY_TOWN]));
}

#[test]
fn swindler_on_an_empty_deck_trashes_nothing() {
    // C++ TestSwindler, "No cards in deck" (line 2262).
    let mut g = new_state(&[id::SWINDLER], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    play(&mut g, id::SWINDLER);
    assert!(g.trash.is_empty());
    assert!(g.players[1].discard.is_empty());
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn swindler_with_no_card_at_that_cost_gains_nothing() {
    // Bridge isn't in this kingdom/supply, but the victim's deck can still hold a copy of it (data
    // only): no supply card currently costs exactly $4, so the trash still happens but nothing gains.
    let mut g = new_state(&[id::SWINDLER], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::BRIDGE]);
    play(&mut g, id::SWINDLER);
    assert_eq!(g.trash, counts_of(&[id::BRIDGE]), "still trashed");
    assert!(g.players[1].discard.is_empty(), "nothing costs exactly 4");
}

// ===========================================================================
// Masquerade — C++ TestMasquerade (line 2940). 2nd edition changed the pass: only players
// with cards in hand pass, to the next such player -- an empty-handed player is skipped
// entirely, on *both* ends (it neither gives nor receives). The 1st-edition suite only skipped
// the giving side (an empty-handed player still received a pass); every case below that
// involves an empty hand is rewritten for the 2nd-edition rule, with a comment explaining the
// change and citing the original case(s) it replaces.
// ===========================================================================

#[test]
fn masquerade_port_self_pass_when_no_other_player_has_cards() {
    // C++ TestMasquerade, "One player" / "Draw Cards, pass Curse to self, Trash Curse"
    // (line 2944): the C++ suite tests this with a genuine 1-player game, which this engine
    // doesn't support (minimum 2 players); a 2nd player with an empty hand produces the
    // identical self-pass wraparound, since 2nd edition already skips empty-handed players on
    // both ends of the pass.
    let mut g = new_state(&[id::MASQUERADE], 2);
    set_hand(&mut g, 0, &[id::MASQUERADE, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY]);
    play(&mut g, id::MASQUERADE);
    assert_eq!(g.players[0].hand, counts_of(&[id::CURSE, id::ESTATE, id::ESTATE]), "+2 Cards first");
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Pass, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::CURSE)); // passes to itself: the only player with cards
    assert_eq!(g.players[0].hand, counts_of(&[id::CURSE, id::ESTATE, id::ESTATE]), "self-pass: nothing actually changes hands");
    let d2 = g.pending_decision().unwrap();
    assert_eq!(d2.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
    assert_eq!(g.trash, counts_of(&[id::CURSE]));
}

#[test]
fn masquerade_port_self_pass_with_one_card_then_optional_trash() {
    // C++ TestMasquerade, "One player" / "Only 1 card to draw" (line 2965) and "Trashing is
    // optional" (line 3344), both adapted to 2 players as above.

    // Trashes the self-passed card.
    let mut g = new_state(&[id::MASQUERADE], 2);
    set_hand(&mut g, 0, &[id::MASQUERADE, id::ESTATE]);
    play(&mut g, id::MASQUERADE);
    // Nothing left to draw; the pass (Estate, the only card) is a single legal choice, auto-applied
    // as a self-pass (the only player with cards), landing straight on the optional trash.
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::ESTATE));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));

    // Declines the trash: trashing stays optional even with just one card.
    let mut g2 = new_state(&[id::MASQUERADE], 2);
    set_hand(&mut g2, 0, &[id::MASQUERADE, id::ESTATE]);
    play(&mut g2, id::MASQUERADE);
    pass(&mut g2);
    assert_eq!(g2.players[0].hand, counts_of(&[id::ESTATE]));
    assert!(g2.trash.is_empty());
}

#[test]
fn masquerade_port_base_case_simultaneous_pass() {
    // C++ TestMasquerade, "Two player" / "Base case" (line 2986). The C++ brains also assert
    // that the offered choices and the player's own hand haven't been touched by the *other*
    // player's pick yet (`VerifyPermutation` on `pick.Choices()`/`view.Hand()`); that honesty
    // property is covered directly by `masquerade_honesty_hides_earlier_passes_from_a_later_chooser`
    // below via `determinize`, so it isn't re-asserted here. Gardens in place of player 0's Gold
    // (the C++ case's own hand card): with no Action left to play, a Treasure ending up in the
    // *active* player's hand would auto-play into the Buy phase once the stack empties, which
    // isn't what this tests; player 1's Gold is unaffected (only the active player auto-plays),
    // so it's kept as-is.
    let d = cards::def(id::MASQUERADE);
    assert_eq!((d.cost, d.cards), (3, 2));

    let mut g = new_state(&[id::MASQUERADE], 2);
    set_hand(&mut g, 0, &[id::MASQUERADE, id::GARDENS]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::DUCHY, id::PROVINCE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::GOLD, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::VILLAGE, id::DUCHY]);
    play(&mut g, id::MASQUERADE);
    assert_eq!(g.players[0].hand, counts_of(&[id::GARDENS, id::CURSE, id::DUCHY]), "+2 Cards first");

    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.player, 0);
    assert_eq!(d0.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Pass, filter: Filter::Any, min: 1, max: 1, ordered: false });
    assert!(!choices(&g).contains(&Choice::Pass), "passing is mandatory");
    choose(&mut g, Choice::Card(id::CURSE));

    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.player, 1);
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::GOLD, id::COPPER]), "unaffected by player 0's pick");
    choose(&mut g, Choice::Card(id::ESTATE));

    // Delivered at once: each receives what the other passed.
    assert_eq!(g.players[0].hand, counts_of(&[id::GARDENS, id::DUCHY, id::ESTATE]));
    assert_eq!(g.players[1].hand, counts_of(&[id::GOLD, id::COPPER, id::CURSE]));
    assert!(g.players[0].passed.is_empty() && g.players[1].passed.is_empty());

    let d2 = g.pending_decision().unwrap();
    assert_eq!(d2.player, 0);
    assert_eq!(d2.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].hand, counts_of(&[id::GARDENS, id::DUCHY]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::PROVINCE));
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
}

// Not ported: "Base case, 2nd player has Masquerade" (line 3029) and the following duplicate of
// the base case (line 3063) are redundant with the port above (same scenario; the first only
// relabels which seat currently has the turn, the second is an exact repeat without the
// mid-decision assertions).

#[test]
fn masquerade_port_self_pass_when_the_other_player_has_no_cards() {
    // C++ TestMasquerade, "Playing player has nothing to pass, but can still Trash any card"
    // (line 3092), "Other player has nothing to pass" (line 3121), and "...leaving nothing for
    // playing player to trash" (line 3150) all test a 1st-edition-only mechanic: an empty-handed
    // player still *receives* a pass (only the *giving* side was skipped for them). 2nd edition
    // changed this so an empty-handed player is skipped on both ends, so in all three cases here
    // the lone player with cards simply passes to themselves and the empty-handed player is
    // never touched; this single case (closest to line 3121, the richest of the three) replaces
    // all three 1st-edition variants. Duchy in place of the C++ case's Gold: with no Action left
    // to play, a Treasure sitting in the active player's final hand would auto-play into the Buy
    // phase once the stack empties, which isn't what this tests.
    let mut g = new_state(&[id::MASQUERADE], 2);
    set_hand(&mut g, 0, &[id::MASQUERADE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::CURSE, id::GARDENS]);
    play(&mut g, id::MASQUERADE);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::CURSE]), "+2 Cards first");
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 0);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Pass, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::ESTATE)); // self-pass: player 1 is empty-handed, excluded
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::CURSE]), "unchanged: it came right back");
    assert!(g.players[1].hand.is_empty(), "never touched");
    choose(&mut g, Choice::Card(id::CURSE)); // the optional trash
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GARDENS));
    assert_eq!(g.trash, counts_of(&[id::CURSE]));
    assert!(g.players[1].hand.is_empty());
}

#[test]
fn masquerade_port_neither_player_can_pass_or_trash() {
    // C++ TestMasquerade, the 2-player "nothing to draw, trash or pass, so nothing happens" case
    // (line 3332); the preceding 1-player variant (line 3325) is redundant with this one plus
    // the self-pass ports above.
    let mut g = new_state(&[id::MASQUERADE], 2);
    set_hand(&mut g, 0, &[id::MASQUERADE]);
    play(&mut g, id::MASQUERADE);
    assert!(g.players[0].hand.is_empty());
    assert!(g.players[1].hand.is_empty());
    assert!(g.trash.is_empty());
    assert!(g.stack.is_empty(), "nothing to pass or trash: fully auto-resolved");
}

#[test]
fn masquerade_port_three_player_all_pass_simultaneously() {
    // C++ TestMasquerade, "Three player" / "All usage" (line 3206). Fully portable to 2nd
    // edition as-is: every player has cards throughout, so the empty-hand rule change never
    // applies here.
    let mut g = new_state(&[id::MASQUERADE], 3);
    set_hand(&mut g, 0, &[id::MASQUERADE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::VILLAGE, id::GOLD]);
    set_hand(&mut g, 1, &[id::CURSE, id::PROVINCE]);
    set_deck_known(&mut g, 1, &[id::DUCHY]);
    set_hand(&mut g, 2, &[id::ESTATE, id::PROVINCE]);
    set_deck_known(&mut g, 2, &[id::DUCHY]);
    play(&mut g, id::MASQUERADE);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::VILLAGE]), "+2 Cards first");
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.player, 0);
    choose(&mut g, Choice::Card(id::COPPER));
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.player, 1);
    choose(&mut g, Choice::Card(id::CURSE));
    let d2 = g.pending_decision().unwrap();
    assert_eq!(d2.player, 2);
    choose(&mut g, Choice::Card(id::ESTATE));
    // Passed left: 0 -> 1 -> 2 -> 0.
    assert_eq!(g.players[1].hand, counts_of(&[id::PROVINCE, id::COPPER]), "Copper from player 0");
    assert_eq!(g.players[2].hand, counts_of(&[id::PROVINCE, id::CURSE]), "Curse from player 1");
    let d3 = g.pending_decision().unwrap();
    assert_eq!(d3.player, 0);
    choose(&mut g, Choice::Card(id::ESTATE)); // trash the Estate received from player 2
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
}

#[test]
fn masquerade_port_three_player_second_player_has_no_cards() {
    // C++ TestMasquerade, "Three player" / "2nd player has no cards to pass" (line 3271).
    // Player 0's result happens to be identical either edition (it receives from player 2 either
    // way); players 1 and 2 differ -- 2nd edition skips the empty-handed player 1 entirely on
    // both ends, so player 0 passes across it straight to player 2, and player 1 never receives
    // anything (1st edition: player 0's neighbor-based pass still reached player 1 despite its
    // empty hand, and player 2 then got nothing from its own empty-handed neighbor).
    let mut g = new_state(&[id::MASQUERADE], 3);
    set_hand(&mut g, 0, &[id::MASQUERADE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::VILLAGE, id::GOLD]);
    set_deck_known(&mut g, 1, &[id::DUCHY]);
    set_hand(&mut g, 2, &[id::ESTATE, id::PROVINCE]);
    set_deck_known(&mut g, 2, &[id::DUCHY]);
    play(&mut g, id::MASQUERADE);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::VILLAGE]));
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.player, 0);
    choose(&mut g, Choice::Card(id::COPPER));
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.player, 2, "player 1 is skipped entirely: empty-handed");
    choose(&mut g, Choice::Card(id::ESTATE));
    assert!(g.players[1].hand.is_empty(), "never touched");
    assert_eq!(g.players[2].hand, counts_of(&[id::PROVINCE, id::COPPER]), "from player 0, skipping player 1");
    let d2 = g.pending_decision().unwrap();
    assert_eq!(d2.player, 0);
    choose(&mut g, Choice::Card(id::ESTATE)); // trash the Estate received from player 2
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
}

// Not ported from TestMasquerade: "Optimization" (lines 3358-3380, one case plus a disabled
// TODO) and "Simulation" (lines 3382-3520+: pass/trash decisions driven by BasicBigMoney's/
// DoBasicBrain's own deck-value heuristics across six sub-cases) are bot-harness cases, not
// card-rules tests.

#[test]
fn masquerade_empty_handed_player_neither_passes_nor_receives() {
    // Fresh scenario (no direct C++ analogue at 3 players with exactly one empty hand): the
    // other two pass to each other, skipping the empty-handed player entirely (2nd edition; see
    // the note at the top of this section).
    let mut g = new_state(&[id::MASQUERADE], 3);
    set_hand(&mut g, 0, &[id::MASQUERADE, id::COPPER]);
    set_hand(&mut g, 2, &[id::ESTATE]);
    play(&mut g, id::MASQUERADE);
    // No deck to draw +2 Cards from; player 0's pass (Copper, its only card) and player 2's
    // (Estate) are each a single legal choice, auto-applied; player 1 (empty hand) is never asked
    // anything — the whole thing cascades within this one `play` call, straight to the optional
    // trash that follows the swap.
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 0, "the optional trash, after the swap");
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]), "received from player 2 (left of player 0, skipping player 1)");
    assert!(g.players[1].hand.is_empty());
    assert_eq!(g.players[2].hand, counts_of(&[id::COPPER]), "received from player 0");
    pass(&mut g);
}

#[test]
fn masquerade_honesty_hides_earlier_passes_from_a_later_chooser() {
    let mut g = new_state(&[id::MASQUERADE], 3);
    set_hand(&mut g, 0, &[id::MASQUERADE, id::GOLD]);
    set_hand(&mut g, 1, &[id::SILVER, id::COPPER]);
    set_hand(&mut g, 2, &[id::ESTATE, id::DUCHY]);
    play(&mut g, id::MASQUERADE);
    // Player 0's own pass (Gold, its only card) is a single legal choice, auto-applied; player 1
    // is asked next.
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 1);
    assert_eq!(g.players[0].passed, counts_of(&[id::GOLD]), "committed, held secretly");

    let mut rng = dominion_engine::rng::Rng::new(7);
    for _ in 0..20 {
        let world = PlayerView::new(&g, 1).determinize(&mut rng);
        assert!(world.players[0].passed.is_empty(), "player 0's committed pass must be pooled as hidden, not copied through");
        assert_eq!(world.players[0].all_cards(), g.players[0].all_cards(), "total composition (public) is preserved");
    }
}

// ===========================================================================
// Secret Passage — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn secret_passage_top_and_middle_then_verifies_order_by_drawing() {
    let d = cards::def(id::SECRET_PASSAGE);
    assert_eq!((d.cost, d.cards, d.actions), (4, 2, 1));

    // Top: with 2 known cards left after the +2 draw, put the picked card on top.
    let mut g = new_state(&[id::SECRET_PASSAGE], 2);
    set_hand(&mut g, 0, &[id::SECRET_PASSAGE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::SILVER, id::GOLD]);
    play(&mut g, id::SECRET_PASSAGE);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::ESTATE, id::DUCHY]), "+2 Cards first");
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.kind, DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::COPPER));
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.kind, DecisionKind::DeckPosition { max_known: 2 });
    assert_eq!(d1.subject, id::COPPER);
    assert_eq!(choices(&g), vec![Choice::Position(0), Choice::Position(1), Choice::Position(2)]);
    choose(&mut g, Choice::Position(0));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::COPPER, id::SILVER, id::GOLD]);
    assert_eq!(g.turn.actions, 1);

    // Middle: put a card below the (new) top known card.
    let mut g = new_state(&[id::SECRET_PASSAGE], 2);
    set_hand(&mut g, 0, &[id::SECRET_PASSAGE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::SILVER, id::GOLD]);
    play(&mut g, id::SECRET_PASSAGE);
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Position(1)); // below the 1st known top card (Silver)
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::SILVER, id::COPPER, id::GOLD]);

    // Verify the placed order by actually drawing it: a Smithy played right after draws exactly
    // this sequence. Non-Treasure filler (Estate) for the set-aside pick and the deck: with no
    // Action left to play after Smithy, a Treasure drawn into hand would auto-play into the Buy
    // phase once the stack empties, which isn't what this checks.
    let mut g2 = new_state(&[id::SECRET_PASSAGE, id::SMITHY], 2);
    set_hand(&mut g2, 0, &[id::SECRET_PASSAGE, id::ESTATE, id::SMITHY]);
    set_deck_known(&mut g2, 0, &[id::DUCHY, id::PROVINCE, id::CURSE, id::GARDENS]);
    play(&mut g2, id::SECRET_PASSAGE);
    choose(&mut g2, Choice::Card(id::ESTATE));
    choose(&mut g2, Choice::Position(1)); // below the 1st known top card (Curse)
    assert_eq!(g2.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::CURSE, id::ESTATE, id::GARDENS]);
    play(&mut g2, id::SMITHY);
    assert_eq!(
        g2.players[0].hand,
        counts_of(&[id::DUCHY, id::PROVINCE, id::CURSE, id::ESTATE, id::GARDENS]),
        "Smithy's +3 Cards drew Curse, Estate, Gardens, in the placed order"
    );
}

#[test]
fn secret_passage_to_bottom_and_deck_bottom_text_round_trip() {
    let mut g = new_state(&[id::SECRET_PASSAGE], 2);
    set_hand(&mut g, 0, &[id::SECRET_PASSAGE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    set_deck_unknown(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::SECRET_PASSAGE);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::ESTATE, id::DUCHY]));
    assert!(g.players[0].deck_known.is_empty());
    choose(&mut g, Choice::Card(id::COPPER));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::DeckPosition { max_known: 0 });
    assert_eq!(choices(&g), vec![Choice::Position(0), Choice::Position(255)], "bottom is distinct while the unknown pile is nonempty");
    choose(&mut g, Choice::Position(255));
    assert_eq!(g.players[0].deck_known_bottom.iter_front_to_back().collect::<Vec<_>>(), vec![id::COPPER]);
    assert_eq!(g.players[0].deck_size(), 2, "Copper (bottom) + Silver (unknown)");

    let text = format_state(&g);
    assert!(text.contains("deck bottom: Copper"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[0].deck_known_bottom.iter_front_to_back().collect::<Vec<_>>(), vec![id::COPPER]);
    assert_eq!(back.players[0].deck_unknown.get(id::SILVER), 1);
}

#[test]
fn secret_passage_when_the_deck_is_totally_empty_has_only_one_position() {
    // A 2nd, distinct card in hand keeps the set-aside pick a real decision (`auto_single` would
    // otherwise apply it, and the position pick right along with it, before this test could
    // observe the `DeckPosition` decision at all).
    let mut g = new_state(&[id::SECRET_PASSAGE], 2);
    g.auto_single = false;
    set_hand(&mut g, 0, &[id::SECRET_PASSAGE, id::COPPER, id::ESTATE]);
    play(&mut g, id::SECRET_PASSAGE);
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::ESTATE]), "nothing to draw");
    choose(&mut g, Choice::Card(id::COPPER));
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::DeckPosition { max_known: 0 });
    // Only choice: top and bottom coincide (nothing else in the deck at all), so bottom isn't
    // offered separately.
    assert_eq!(choices(&g), vec![Choice::Position(0)]);
    choose(&mut g, Choice::Position(0));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::COPPER));
}

#[test]
fn old_state_text_without_deck_bottom_line_still_parses() {
    let text = "players: 2\nkingdom: Secret Passage\n\n[player 1]\nhand: Copper\ndeck top: Silver\ndeck:\ndiscard:\nin play:\n\n\
                [player 2]\nhand:\ndeck top:\ndeck:\ndiscard:\nin play:\n";
    let g = parse_state(text).unwrap();
    assert!(g.players[0].deck_known_bottom.is_empty());
    assert!(g.players[1].deck_known_bottom.is_empty());
}

// ===========================================================================
// Diplomat (Action-Reaction) — new in 2nd edition, no C++ test
// ===========================================================================

#[test]
fn diplomat_actions_threshold_exactly_five_vs_six() {
    let d = cards::def(id::DIPLOMAT);
    assert_eq!((d.cost, d.cards), (4, 2));
    assert!(cards::is(id::DIPLOMAT, cards::ACTION) && cards::is(id::DIPLOMAT, cards::REACTION));

    // Non-Treasure fillers (see the note on `throne_room_with_wishing_well`).

    // 3 other cards in hand + 2 drawn = 5: +2 Actions.
    let mut g = new_state(&[id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::DIPLOMAT, id::ESTATE, id::ESTATE, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::DIPLOMAT);
    assert_eq!(g.players[0].hand.total(), 5);
    assert_eq!(g.turn.actions, 2, "1 - 1 (play) + 2 (threshold met)");

    // 4 other cards + 2 drawn = 6: no bonus.
    let mut g2 = new_state(&[id::DIPLOMAT], 2);
    set_hand(&mut g2, 0, &[id::DIPLOMAT, id::ESTATE, id::ESTATE, id::CURSE, id::GARDENS]);
    set_deck_known(&mut g2, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g2, id::DIPLOMAT);
    assert_eq!(g2.players[0].hand.total(), 6);
    assert_eq!(g2.turn.actions, 0, "1 - 1 (play) + 0");
}

#[test]
fn diplomat_reacts_to_militia_before_the_forced_discard() {
    // Exactly 3 Coppers, so the reaction's exact-3 discard consumes all of them and `pick_while`
    // (which just keeps picking Copper while it's offered) can't bleed into Militia's own
    // following discard by picking a 4th.
    let mut g = new_state(&[id::MILITIA, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]); // 5 cards
    set_deck_known(&mut g, 1, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::MILITIA);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 1);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Reveal });
    assert_eq!(d.subject, id::DIPLOMAT);
    choose(&mut g, Choice::Yes);
    assert!(g.players[1].hand.has(id::DIPLOMAT), "revealing doesn't remove it from hand");
    assert_eq!(g.players[1].hand.total(), 7, "5 + 2 drawn, before discarding");
    let dd = g.pending_decision().unwrap();
    assert_eq!(dd.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 3, max: 3, ordered: false });
    pick_while(&mut g, Zone::Hand, Act::Discard, id::COPPER); // discards exactly the 3 Coppers
    assert_eq!(g.players[1].hand, counts_of(&[id::DIPLOMAT, id::ESTATE, id::DUCHY, id::PROVINCE]));
    // Militia's own "discard down to 3" now sees the *current* (post-reaction) hand: 4 cards, so
    // only 1 more must go — not 2, which is what a stale pre-reaction count would have forced.
    let dm = g.pending_decision().unwrap();
    assert_eq!(dm.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[1].hand.total(), 3, "Militia always leaves exactly 3");
}

#[test]
fn diplomat_reacts_to_torturer_before_the_victims_choice() {
    let mut g = new_state(&[id::TORTURER, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::TORTURER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]); // 5 cards
    set_deck_known(&mut g, 1, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::TORTURER);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 1);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Reveal });
    choose(&mut g, Choice::Yes);
    let dd = g.pending_decision().unwrap();
    assert_eq!(dd.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 3, max: 3, ordered: false });
    pick_while(&mut g, Zone::Hand, Act::Discard, id::COPPER); // the 3 Coppers
    assert_eq!(g.players[1].hand, counts_of(&[id::DIPLOMAT, id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[1].discard, counts_of(&[id::COPPER, id::COPPER, id::COPPER]));
    // Only now does Torturer's own victim choice happen.
    let dm = g.pending_decision().unwrap();
    assert_eq!(dm.player, 1);
    assert_eq!(dm.kind, DecisionKind::Mode { picks: 1, distinct: false });
    let curse_i = cards::modes(id::TORTURER).iter().position(|o| matches!(o, ModeOpt::Gain(c, _) if *c == id::CURSE)).unwrap() as u8;
    choose(&mut g, Choice::Mode(curse_i));
    assert!(g.players[1].hand.has(id::CURSE));
}

#[test]
fn diplomat_reaction_can_be_declined_or_skipped_below_five_cards() {
    // Declined.
    let mut g = new_state(&[id::MILITIA, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::DIPLOMAT, id::COPPER, id::COPPER, id::COPPER, id::COPPER]); // 5 cards
    play(&mut g, id::MILITIA);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Reveal });
    choose(&mut g, Choice::No);
    // Militia's own discard-to-3 still applies to the original (undrawn) 5-card hand: 2 discards
    // in total (the canonical first pick among 4 identical Coppers is forced/auto-applied, so
    // only the last of the two picks is necessarily still visible as its own decision here).
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, .. }));
    pick_while(&mut g, Zone::Hand, Act::Discard, id::COPPER);
    assert_eq!(g.players[1].hand.total(), 3, "Militia always leaves exactly 3");
    assert!(g.players[1].hand.has(id::DIPLOMAT), "Diplomat itself wasn't drawn/discarded, still in hand");

    // Below the 5-card threshold: not offered at all.
    let mut g2 = new_state(&[id::MILITIA, id::DIPLOMAT], 2);
    set_hand(&mut g2, 0, &[id::MILITIA]);
    set_hand(&mut g2, 1, &[id::DIPLOMAT, id::COPPER, id::COPPER, id::COPPER]); // 4 cards
    play(&mut g2, id::MILITIA);
    let d2 = g2.pending_decision().unwrap();
    assert_eq!(d2.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 1, max: 1, ordered: false });
}

// ===========================================================================
// Throne Room with each step-4/5 card
// ===========================================================================

#[test]
fn throne_room_with_wishing_well() {
    // Non-Treasure fillers throughout: with no Action left to play after both resolutions, a
    // Treasure drawn into hand would auto-play into the Buy phase, which isn't what this tests.
    let mut g = new_state(&[id::THRONE_ROOM, id::WISHING_WELL], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::WISHING_WELL]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::ESTATE, id::CURSE, id::GARDENS]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::WISHING_WELL));
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]), "1st resolution's +1 Card");
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.kind, DecisionKind::Name);
    choose(&mut g, Choice::Card(id::PROVINCE)); // hits
    // The 2nd resolution's own +1 Card already happened too (Estate), landing on its own Name.
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE, id::ESTATE]));
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.kind, DecisionKind::Name);
    choose(&mut g, Choice::Card(id::CURSE)); // hits
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE, id::ESTATE, id::CURSE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GARDENS));
    assert_eq!(g.turn.actions, 2, "0 (Throne Room) + 1 + 1");
}

#[test]
fn throne_room_with_swindler() {
    let mut g = new_state(&[id::THRONE_ROOM, id::SWINDLER, id::SHANTY_TOWN, id::COURTYARD], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SWINDLER));
    // 1st resolution: +$2, trash Silver (cost 3), gain exactly 3.
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash, counts_of(&[id::SILVER]));
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.kind, DecisionKind::Gain { max_cost: 3, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    choose(&mut g, Choice::Card(id::SHANTY_TOWN));
    // 2nd resolution: +$2 more, trash Estate (cost 2), gain exactly 2.
    assert_eq!(g.turn.coins, 4, "two Swindler resolutions, +$2 each");
    assert_eq!(g.trash, counts_of(&[id::SILVER, id::ESTATE]));
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.kind, DecisionKind::Gain { max_cost: 2, filter: Filter::Any, dest: Dest::Discard, exact: true, potion: false, optional: false });
    choose(&mut g, Choice::Card(id::COURTYARD));
    assert_eq!(g.players[1].discard, counts_of(&[id::SHANTY_TOWN, id::COURTYARD]));
    assert_eq!(g.turn.actions, 0, "Swindler grants no Actions");
}

#[test]
fn throne_room_with_masquerade() {
    let mut g = new_state(&[id::THRONE_ROOM, id::MASQUERADE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MASQUERADE, id::COPPER]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::SILVER, id::SILVER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MASQUERADE));
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::GOLD, id::GOLD]), "1st resolution's +2 Cards");
    choose(&mut g, Choice::Card(id::COPPER)); // p0 passes
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.player, 1);
    choose(&mut g, Choice::Card(id::DUCHY)); // p1 passes
    assert_eq!(g.players[0].hand, counts_of(&[id::GOLD, id::GOLD, id::DUCHY]));
    assert_eq!(g.players[1].hand, counts_of(&[id::ESTATE, id::COPPER]));
    let d2 = g.pending_decision().unwrap();
    assert_eq!(d2.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false });
    pass(&mut g); // decline the 1st resolution's trash

    // 2nd resolution: its own +2 Cards, then a fresh round of passing.
    assert_eq!(g.players[0].hand, counts_of(&[id::GOLD, id::GOLD, id::DUCHY, id::SILVER, id::SILVER]));
    choose(&mut g, Choice::Card(id::DUCHY)); // p0 passes
    let d3 = g.pending_decision().unwrap();
    assert_eq!(d3.player, 1);
    choose(&mut g, Choice::Card(id::ESTATE)); // p1 passes
    assert_eq!(g.players[0].hand, counts_of(&[id::GOLD, id::GOLD, id::SILVER, id::SILVER, id::ESTATE]));
    assert_eq!(g.players[1].hand, counts_of(&[id::COPPER, id::DUCHY]));
    pass(&mut g); // decline the 2nd resolution's trash
    assert_eq!(g.turn.actions, 0, "Masquerade grants no Actions");
}

#[test]
fn throne_room_with_secret_passage() {
    // Non-Treasure fillers throughout (see the note on `throne_room_with_wishing_well`). Placing
    // the 1st resolution's card on top means the 2nd resolution's own +2 Cards immediately draws
    // it back — a real, legitimate interaction this traces through explicitly.
    let mut g = new_state(&[id::THRONE_ROOM, id::SECRET_PASSAGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SECRET_PASSAGE, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::CURSE, id::GARDENS, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SECRET_PASSAGE));
    // 1st resolution: +2 Cards (Province, Curse).
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE, id::CURSE]));
    let d0 = g.pending_decision().unwrap();
    assert_eq!(d0.kind, DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::DUCHY));
    let d1 = g.pending_decision().unwrap();
    assert_eq!(d1.kind, DecisionKind::DeckPosition { max_known: 2 }, "Gardens, Estate still known");
    choose(&mut g, Choice::Position(0)); // top: Duchy, Gardens, Estate (top to bottom)
    // 2nd resolution: its own +2 Cards immediately redraws the just-placed Duchy, then Gardens.
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE, id::CURSE, id::DUCHY, id::GARDENS]));
    let d2 = g.pending_decision().unwrap();
    assert_eq!(d2.kind, DecisionKind::Select { from: Zone::Hand, act: Act::SetAside, filter: Filter::Any, min: 1, max: 1, ordered: false });
    choose(&mut g, Choice::Card(id::DUCHY));
    let d3 = g.pending_decision().unwrap();
    assert_eq!(d3.kind, DecisionKind::DeckPosition { max_known: 1 }, "only Estate left known");
    choose(&mut g, Choice::Position(1)); // below the 1 known top card (Estate)
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE, id::CURSE, id::GARDENS]));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::ESTATE, id::DUCHY]);
    assert_eq!(g.turn.actions, 2, "0 (Throne Room) + 1 + 1");
}

#[test]
fn throne_room_with_diplomat() {
    // Non-Treasure fillers throughout (see the note on `throne_room_with_wishing_well`).
    let mut g = new_state(&[id::THRONE_ROOM, id::DIPLOMAT], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::DIPLOMAT, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::CURSE, id::GARDENS]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::DIPLOMAT));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE, id::CURSE, id::GARDENS]));
    // 1st resolution: hand became 3 (<=5) -> +2 Actions. 2nd: hand became 5 (<=5) -> +2 more.
    assert_eq!(g.turn.actions, 4, "0 (Throne Room) + 2 + 2");
}
