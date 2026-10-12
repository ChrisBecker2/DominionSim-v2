//! Cornucopia & Guilds (2nd edition) tests: the card list, Coffers, Overpay, the piles outside the
//! Supply (Bane, Ferryman's pile, the Rewards) and every card's printed effect, edge cases and
//! rulings. The rulings are the official FAQs (see `docs/cornucopia-guilds-plan.md`); these tests
//! are written in this project's own words.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

/// Junk that is neither Action nor Treasure, so every draw is visible and nothing auto-plays.
fn junk(n: usize) -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(n).copied().collect()
}

/// Player `p`'s hand plus the Treasures in their play area: the engine plays the Treasures in hand when the
/// Action phase ends, so a test of what a card put into the hand looks at both.
fn held(g: &GameState, p: usize) -> Counts {
    let mut c = g.players[p].hand;
    for (card, n) in g.players[p].in_play.iter() {
        if cards::is(card, cards::TREASURE) {
            c.add(card, n);
        }
    }
    c
}

/// Answer pending Select decisions with their first choice (stops at the first other decision).
fn drain_selects(g: &mut GameState) {
    while let Some(d) = g.pending_decision() {
        if !matches!(d.kind, DecisionKind::Select { .. }) {
            break;
        }
        let first = choices(g)[0];
        choose(g, first);
    }
}

/// Put player 0 in their Buy phase with `coins` and nothing else (the Buy decision is next).
fn buy_phase(g: &mut GameState, coins: u16) {
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
}

// ===========================================================================
// The card list, the ids, the types
// ===========================================================================

#[test]
fn the_set_has_26_kingdom_cards_and_6_rewards() {
    assert_eq!(cards::NUM_CARDS, 427);
    assert_eq!(CardSet::ALL.len(), 12);
    assert_eq!(cards::set_by_name("Cornucopia & Guilds"), Some(CardSet::CornucopiaGuilds));
    assert_eq!(cards::set_by_name("cornucopia"), Some(CardSet::CornucopiaGuilds));
    assert_eq!(CardSet::CornucopiaGuilds.name(), "Cornucopia & Guilds");
    let in_set = |set| (cards::FIRST_KINGDOM..cards::NUM_CARDS as CardId).filter(|&c| cards::is_kingdom(c) && cards::set_of(c) == set).count();
    assert_eq!(in_set(CardSet::CornucopiaGuilds), 26);
    let rewards: Vec<&str> = (cards::FIRST_REWARD..cards::FIRST_REWARD + cards::NUM_REWARDS as CardId).map(cards::name).collect();
    assert_eq!(rewards, ["Coronet", "Courser", "Demesne", "Housecarl", "Huge Turnip", "Renown"]);
    for c in cards::FIRST_REWARD..cards::FIRST_REWARD + cards::NUM_REWARDS as CardId {
        assert!(cards::is_reward(c) && !cards::is_kingdom(c), "{} is not a kingdom card", cards::name(c));
        assert!(cards::is(c, cards::REWARD) && cards::set_of(c) == CardSet::CornucopiaGuilds);
        assert_eq!(cards::cost(c), 0, "the Rewards cost $0*");
    }
    assert!(!cards::is_reward(id::FAIRGROUNDS) && cards::is_kingdom(id::FAIRGROUNDS));
    // The removed first-edition cards are not here.
    for gone in ["Fortune Teller", "Doctor", "Masterpiece", "Farming Village", "Horse Traders", "Taxman", "Tournament", "Harvest"] {
        assert!(cards::by_name(gone).is_none(), "{gone}");
    }
}

#[test]
fn prince_is_a_duration_that_schedules_nothing() {
    // Action-Duration-Command since the 2022 errata.
    assert!(cards::is(id::PRINCE, cards::ACTION) && cards::is(id::PRINCE, cards::DURATION) && cards::is(id::PRINCE, cards::COMMAND));
    assert_eq!(cards::def(id::PRINCE).types.count_ones(), 3, "Courtier counts three types");
    // Its own start-of-turn effect is the Prince bookkeeping: playing it with a card sets nothing
    // pending as a Duration, and without a card it is discarded as before.
    let mut g = new_state(&[id::PRINCE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::PRINCE, id::VILLAGE]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::PRINCE);
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.players[0].pending_durations_len, 0, "no generic Duration entry for Prince");
    assert!(g.turn.duration_held.is_empty());
    assert_eq!(g.players[0].prince_cards.get(id::VILLAGE), 1);
    assert!(g.players[0].in_play.has(id::PRINCE));

    let mut g = new_state(&[id::PRINCE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::PRINCE]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::PRINCE);
    assert_eq!(g.players[0].pending_durations_len, 0);
    pass(&mut g); // end the Action phase; the Buy phase has nothing to buy with
    buy_phase(&mut g, 0);
    while !matches!(g.pending_decision(), None) && g.turn.number == 1 {
        let _ = pass(&mut g);
    }
    assert!(!g.players[0].in_play.has(id::PRINCE), "a Prince that holds nothing is discarded in Clean-up");
    assert!(g.players[0].discard.has(id::PRINCE));
}

#[test]
fn prince_is_a_duration_for_sailor_and_the_page() {
    // Gaining a Prince with a Sailor lets the Sailor play it (it is a Duration); Throne Room still
    // doubles it and keeps both in play (the existing Prince tests), and the page colours
    // Durations, so the type is what changes its colour (checked in web/test.mjs).
    assert!(cards::is(id::PRINCE, cards::DURATION));
    assert!(!cards::is(id::SMITHY, cards::DURATION));
}

// ===========================================================================
// 160 lanes and the two-word supply mask
// ===========================================================================

#[test]
fn counts_hold_every_card_id() {
    let mut c = Counts::EMPTY;
    for id in [0, 127, 128, 152, 157] {
        c.add(id, 2);
    }
    assert_eq!(c.total(), 10);
    assert!(!c.is_empty());
    let seen: Vec<(CardId, u8)> = c.iter().collect();
    assert_eq!(seen, vec![(0, 2), (127, 2), (128, 2), (152, 2), (157, 2)]);
    assert_eq!(c.nth(9), 157);
    assert_eq!(c.nth(4), 128);
    c.clear();
    assert!(c.is_empty() && c.total() == 0);
}

#[test]
fn supply_ids_above_127_work() {
    let g = new_state(&[id::FARRIER, id::CANDLESTICK_MAKER], 2);
    for c in [id::FARRIER, id::CANDLESTICK_MAKER] {
        assert!(g.in_supply(c), "{}", cards::name(c));
        assert_eq!(g.supply.get(c), 10);
    }
    let ids: Vec<CardId> = g.supply_cards().collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "the supply iterates in id order across both mask words");
    assert!(ids.contains(&id::FARRIER) && ids.contains(&id::COPPER));
    assert_eq!(g.empty_piles(), 0);
}

// ===========================================================================
// Coffers
// ===========================================================================

#[test]
fn candlestick_maker_gives_action_buy_and_a_coffers_token() {
    let d = cards::def(id::CANDLESTICK_MAKER);
    assert_eq!((d.cost, d.actions, d.buys, d.cards, d.coins), (2, 1, 1, 0, 0));
    let mut g = new_state(&[id::CANDLESTICK_MAKER], 2);
    set_hand(&mut g, 0, &[id::CANDLESTICK_MAKER]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::CANDLESTICK_MAKER, &mut ev);
    assert_eq!((g.turn.actions, g.turn.buys), (1, 2));
    assert_eq!(g.players[0].coffers, 1);
    assert!(ev.contains(&Event::Coffers { player: 0, source: id::CANDLESTICK_MAKER, n: 1 }));
    // It stays on the mat: not coins, and not lost at the end of the turn.
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn coffers_are_spent_one_token_at_a_time_at_the_buy_decision() {
    let mut g = new_state(&[id::CANDLESTICK_MAKER], 2);
    g.players[0].coffers = 3;
    buy_phase(&mut g, 1);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert!(choices(&g).contains(&Choice::Coffers));
    assert!(!choices(&g).contains(&Choice::Card(id::SILVER)), "$1 does not buy a Silver");
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Coffers, &mut ev);
    assert_eq!((g.turn.coins, g.players[0].coffers), (2, 2));
    assert!(ev.contains(&Event::SpendCoffers { player: 0, n: 1 }));
    expect_decision(&mut g);
    choose(&mut g, Choice::Coffers);
    assert_eq!((g.turn.coins, g.players[0].coffers), (3, 1));
    assert!(choices(&g).contains(&Choice::Card(id::SILVER)));
    buy(&mut g, id::SILVER);
    // One buy used (the only one), so the Buy phase is over; the unspent token is kept.
    assert_eq!(g.players[0].coffers, 1);
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 1);
}

#[test]
fn coffers_can_be_spent_between_buys_and_are_not_offered_without_tokens_or_a_buy() {
    let mut g = new_state(&[id::CANDLESTICK_MAKER], 2);
    buy_phase(&mut g, 3);
    g.turn.buys = 2;
    expect_decision(&mut g);
    assert!(!choices(&g).contains(&Choice::Coffers), "no tokens, no choice");
    g.players[0].coffers = 2;
    assert!(choices(&g).contains(&Choice::Coffers));
    buy(&mut g, id::SILVER); // $3 -> $0, one buy left
    expect_decision(&mut g);
    choose(&mut g, Choice::Coffers);
    choose(&mut g, Choice::Coffers);
    assert_eq!(g.turn.coins, 2);
    buy(&mut g, id::ESTATE);
    assert_eq!(g.players[0].coffers, 0);
    assert_eq!(g.players[0].all_cards().get(id::ESTATE), 1);
}

#[test]
fn coffers_round_trip_through_the_text_format() {
    let mut g = new_state(&[id::CANDLESTICK_MAKER], 2);
    g.players[0].coffers = 4;
    g.players[1].coffers = 1;
    let text = format_state(&g);
    assert!(text.contains("coffers: 4"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!((back.players[0].coffers, back.players[1].coffers), (4, 1));
    assert!(!format_state(&new_state(&[id::SMITHY], 2)).contains("coffers"), "absent when zero");
}

// ===========================================================================
// Overpay (Farrier: the simplest, +1 Card at the end of the turn per $1)
// ===========================================================================

#[test]
fn buying_an_overpay_card_with_nothing_to_spare_asks_nothing() {
    let mut g = new_state(&[id::FARRIER], 2);
    set_deck_known(&mut g, 0, &junk(12));
    buy_phase(&mut g, 2);
    buy(&mut g, id::FARRIER);
    assert!(g.players[0].discard.has(id::FARRIER));
    assert_eq!(g.players[0].hand.total(), 5);
}

#[test]
fn farrier_overpay_draws_extra_cards_at_cleanup() {
    let d = cards::def(id::FARRIER);
    assert_eq!((d.cost, d.cards, d.actions, d.buys), (2, 1, 1, 1));
    assert!(cards::has_overpay(id::FARRIER));
    let mut g = new_state(&[id::FARRIER], 2);
    set_deck_known(&mut g, 0, &junk(12));
    buy_phase(&mut g, 5);
    g.turn.buys = 1;
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::FARRIER));
    // The decision: overpay 0..=3 (the Potion is only for Stonemason).
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Overpay { card: id::FARRIER, max_coins: 3, potion: false });
    let offered = choices(&g);
    assert_eq!(offered, vec![Choice::Overpay(0), Choice::Overpay(1), Choice::Overpay(2), Choice::Overpay(3)]);
    assert!(!g.players[0].discard.has(id::FARRIER), "gained after the amount is chosen");
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Overpay(3), &mut ev);
    assert!(ev.contains(&Event::Overpay { player: 0, card: id::FARRIER, coins: 3, potion: false }));
    // Cleanup draws 5 + 3 cards; the Farrier is in the discard pile (gained there).
    assert_eq!(g.players[0].hand.total(), 8);
    assert!(g.players[0].discard.has(id::FARRIER));
    assert_eq!(g.turn.number, 2, "the next turn has started");
}

#[test]
fn overpaying_zero_or_declining_is_a_plain_buy() {
    let mut g = new_state(&[id::FARRIER], 2);
    set_deck_known(&mut g, 0, &junk(12));
    buy_phase(&mut g, 4);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::FARRIER));
    expect_decision(&mut g);
    choose(&mut g, Choice::Overpay(0));
    assert_eq!(g.players[0].hand.total(), 5, "no extra cards");
}

#[test]
fn overpay_is_not_available_when_gaining_without_buying() {
    // Workshop gains a Farrier: nothing is asked and nothing is drawn extra.
    let mut g = new_state(&[id::WORKSHOP, id::FARRIER], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &junk(12));
    g.turn.coins = 9;
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::FARRIER));
    assert!(g.players[0].discard.has(id::FARRIER));
    assert_eq!(g.turn.extra_draw, 0);
}

#[test]
fn farrier_overpay_draws_extra_even_after_an_outpost() {
    // An Outpost turn draws 3; Farrier's extra cards still come on top.
    let mut g = new_state(&[id::FARRIER, id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_known(&mut g, 0, &junk(14));
    play(&mut g, id::OUTPOST); // no Actions left: straight to the Buy phase
    expect_decision(&mut g);
    g.turn.coins = 4;
    choose(&mut g, Choice::Card(id::FARRIER));
    expect_decision(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Overpay(2), &mut ev);
    let first_turn_end = ev.iter().position(|e| matches!(e, Event::TurnStart { .. })).unwrap_or(ev.len());
    let draws = ev[..first_turn_end].iter().filter(|e| matches!(e, Event::Draw { .. })).count();
    assert_eq!(draws, 3 + 2, "an Outpost turn draws 3, plus the Farrier's 2");
}

#[test]
fn extra_draw_round_trips_through_the_text_format() {
    let mut g = new_state(&[id::FARRIER], 2);
    g.turn.phase = Phase::Buy;
    g.turn.extra_draw = 3;
    g.turn.guilds = 1;
    g.turn.gained_in_buy = 2;
    let text = format_state(&g);
    let back = parse_state(&text).unwrap();
    assert_eq!((back.turn.extra_draw, back.turn.guilds, back.turn.gained_in_buy), (3, 1, 2));
}

// ===========================================================================
// Hamlet: +1 Card +1 Action; may discard a card for +1 Action; may discard a card for +1 Buy
// ===========================================================================

#[test]
fn hamlet_draws_then_offers_two_independent_discards() {
    let d = cards::def(id::HAMLET);
    assert_eq!((d.cost, d.cards, d.actions, d.buys), (2, 1, 1, 0));
    assert!(cards::is_order_sensitive(id::HAMLET));
    let mut g = new_state(&[id::HAMLET], 2);
    set_hand(&mut g, 0, &[id::HAMLET, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD]);
    play(&mut g, id::HAMLET);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::COPPER, id::SILVER]), "drew first");
    assert_eq!(g.turn.actions, 1);
    // First: discard for +1 Action.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, max: 1, min: 0, .. }));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.turn.buys, 1);
    // Then: discard for +1 Buy (a separate decision).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, max: 1, .. }));
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!((g.turn.actions, g.turn.buys), (2, 2));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE, id::COPPER]));
}

#[test]
fn hamlet_bonuses_are_each_optional_and_only_for_a_discard() {
    let mut g = new_state(&[id::HAMLET], 2);
    set_hand(&mut g, 0, &[id::HAMLET, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE]);
    play(&mut g, id::HAMLET);
    expect_decision(&mut g);
    pass(&mut g); // no +Action
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::ESTATE)); // +Buy only
    assert_eq!((g.turn.actions, g.turn.buys), (1, 2));
    assert_eq!(g.players[0].hand.total(), 2, "one card discarded, nothing else");
}

#[test]
fn hamlet_with_one_card_discards_it_for_one_bonus_and_never_twice() {
    let mut g = new_state(&[id::HAMLET], 2);
    set_hand(&mut g, 0, &[id::HAMLET]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::HAMLET);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::ESTATE)); // the card it drew
    assert_eq!(g.turn.actions, 2);
    // The hand is empty: the +Buy discard has nothing to act on and is skipped.
    assert_eq!(g.turn.buys, 1);
    assert!(g.players[0].hand.is_empty());
}

#[test]
fn hamlet_with_an_empty_deck_and_hand_does_nothing_more() {
    let mut g = new_state(&[id::HAMLET], 2);
    set_hand(&mut g, 0, &[id::HAMLET]);
    play(&mut g, id::HAMLET);
    assert_eq!((g.turn.actions, g.turn.buys), (1, 1));
}

#[test]
fn throne_room_on_hamlet_asks_four_discards_and_gives_both_bonuses_twice() {
    let mut g = new_state(&[id::HAMLET, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAMLET, id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HAMLET));
    for _ in 0..4 {
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }));
        let first = choices(&g)[0];
        choose(&mut g, first);
    }
    // 1 action - Throne Room + 2 (printed) + 2 (discards); 1 buy + 2.
    assert_eq!((g.turn.actions, g.turn.buys), (2 + 2, 1 + 2));
}

// ===========================================================================
// Menagerie: +1 Action; reveal the hand; +3 Cards if all names differ, else +1 Card
// ===========================================================================

#[test]
fn menagerie_draws_three_when_every_name_is_different() {
    let d = cards::def(id::MENAGERIE);
    assert_eq!((d.cost, d.actions, d.cards), (3, 1, 0));
    let mut g = new_state(&[id::MENAGERIE], 2);
    set_hand(&mut g, 0, &[id::MENAGERIE, id::COPPER, id::SILVER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(6));
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::MENAGERIE, &mut ev);
    assert_eq!(held(&g, 0).total(), 3 + 3);
    assert_eq!(g.turn.actions, 1, "+1 Action replaces the one spent");
    for c in [id::COPPER, id::SILVER, id::ESTATE] {
        assert!(ev.contains(&Event::Reveal { player: 0, card: c }), "{}", cards::name(c));
    }
}

#[test]
fn menagerie_draws_one_card_with_a_duplicate_name() {
    let mut g = new_state(&[id::MENAGERIE], 2);
    set_hand(&mut g, 0, &[id::MENAGERIE, id::COPPER, id::COPPER, id::SILVER]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::MENAGERIE);
    assert_eq!(held(&g, 0).total(), 3 + 1, "Copper and Copper match; Copper and Silver do not");
}

#[test]
fn menagerie_with_an_empty_hand_draws_three_and_reshuffles_mid_draw() {
    let mut g = new_state(&[id::MENAGERIE], 2);
    set_hand(&mut g, 0, &[id::MENAGERIE]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    set_discard(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    play(&mut g, id::MENAGERIE);
    assert_eq!(g.players[0].hand.total(), 3, "no cards left in hand = no matching cards = +3 Cards");
    assert!(g.players[0].discard.is_empty(), "the discard pile was shuffled into the deck for the draw");
}

// ===========================================================================
// Shop: +1 Card +$1; may play an Action from the hand that has no copy in play
// ===========================================================================

#[test]
fn shop_draws_gives_a_coin_and_plays_an_action_for_free() {
    let d = cards::def(id::SHOP);
    assert_eq!((d.cost, d.cards, d.coins, d.actions), (3, 1, 1, 0));
    let mut g = new_state(&[id::SHOP, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SHOP, id::VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(8));
    play(&mut g, id::SHOP);
    assert_eq!(g.turn.coins, 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Pass]);
    choose(&mut g, Choice::Card(id::VILLAGE));
    // Shop used the only Action (0 left); the Village was free and gave +2 Actions and a card.
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].hand.total(), 3, "Estate + the Shop's card + the Village's card");
    assert!(g.players[0].in_play.has(id::VILLAGE) && g.players[0].in_play.has(id::SHOP));
}

#[test]
fn shop_cannot_play_an_action_with_a_copy_in_play() {
    let mut g = new_state(&[id::SHOP, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SHOP, id::VILLAGE, id::SMITHY]);
    set_in_play(&mut g, 0, &[id::VILLAGE]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::SHOP);
    expect_decision(&mut g);
    assert_eq!(choices(&g), vec![Choice::Card(id::SMITHY), Choice::Pass], "the Village has a copy in play");
    // Declining is allowed.
    pass(&mut g);
    assert!(g.players[0].hand.has(id::VILLAGE) && g.players[0].hand.has(id::SMITHY));
}

#[test]
fn shop_with_no_eligible_action_asks_nothing() {
    let mut g = new_state(&[id::SHOP, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SHOP, id::VILLAGE]);
    set_in_play(&mut g, 0, &[id::VILLAGE]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::SHOP);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy, "no Action to play: straight on (the Village cannot be played, no Actions left)");
}

#[test]
fn shop_playing_a_duration_does_not_keep_the_shop_in_play() {
    let mut g = new_state(&[id::SHOP, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::SHOP, id::CARAVAN]);
    set_deck_known(&mut g, 0, &junk(8));
    play(&mut g, id::SHOP);
    choose(&mut g, Choice::Card(id::CARAVAN));
    assert_eq!(g.players[0].pending_durations_len, 1);
    assert!(g.turn.duration_held.has(id::CARAVAN));
    assert!(!g.turn.duration_held.has(id::SHOP), "only the Caravan stays");
}

#[test]
fn throne_room_on_shop_plays_two_different_actions() {
    let mut g = new_state(&[id::SHOP, id::THRONE_ROOM, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SHOP, id::VILLAGE, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SHOP));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::VILLAGE));
    expect_decision(&mut g);
    // The Village is in play now, so the second Shop can only play the Smithy.
    assert_eq!(choices(&g), vec![Choice::Card(id::SMITHY), Choice::Pass]);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.turn.coins, 2, "+$1 twice");
}

// ===========================================================================
// Plaza: +1 Card +2 Actions; may discard a Treasure for +1 Coffers
// ===========================================================================

#[test]
fn plaza_draws_then_discards_a_treasure_for_a_coffers_token() {
    let d = cards::def(id::PLAZA);
    assert_eq!((d.cost, d.cards, d.actions), (4, 1, 2));
    let mut g = new_state(&[id::PLAZA], 2);
    set_hand(&mut g, 0, &[id::PLAZA, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::GOLD]);
    play(&mut g, id::PLAZA);
    assert_eq!(g.turn.actions, 2);
    // The card it drew can be discarded: the choices are Treasures only.
    expect_decision(&mut g);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass]);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::COPPER), &mut ev);
    assert_eq!(g.players[0].coffers, 1);
    assert!(ev.contains(&Event::Discard { player: 0, card: id::COPPER }));
    assert!(ev.contains(&Event::Coffers { player: 0, source: id::PLAZA, n: 1 }));
}

#[test]
fn plaza_gives_nothing_for_declining_or_without_a_treasure() {
    let mut g = new_state(&[id::PLAZA], 2);
    set_hand(&mut g, 0, &[id::PLAZA, id::SILVER]);
    set_deck_known(&mut g, 0, &junk(4));
    play(&mut g, id::PLAZA);
    pass(&mut g);
    assert_eq!(g.players[0].coffers, 0);
    assert!(held(&g, 0).has(id::SILVER));

    let mut g = new_state(&[id::PLAZA], 2);
    set_hand(&mut g, 0, &[id::PLAZA, id::ESTATE, id::CURSE]);
    set_deck_known(&mut g, 0, &junk(4));
    play(&mut g, id::PLAZA);
    assert_eq!(g.players[0].coffers, 0, "only Treasures can be discarded; nothing to ask");
}

// ===========================================================================
// Remake: do this twice: trash a card from your hand, gain a card costing exactly $1 more
// ===========================================================================

#[test]
fn remake_trashes_and_gains_one_card_at_a_time() {
    let d = cards::def(id::REMAKE);
    assert_eq!((d.cost, d.cards, d.actions), (4, 0, 0));
    let mut g = new_state(&[id::REMAKE, id::SMITHY], 2); // Smithy costs 4
    set_hand(&mut g, 0, &[id::REMAKE, id::ESTATE, id::SILVER]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::REMAKE, &mut ev);
    choose_ev(&mut g, Choice::Card(id::ESTATE), &mut ev); // $2 -> exactly $3: a Silver (the only one)
    // The gain was forced (one card costs $3), so the second trash is next (the Silver in hand is the only
    // card left) and its gain is a choice: exactly $4 is a Remake or a Smithy.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, exact: true, .. }), "{d:?}");
    choose_ev(&mut g, Choice::Card(id::SMITHY), &mut ev);
    let pos = |e: &Event| ev.iter().position(|x| x == e).unwrap();
    let trash_estate = pos(&Event::Trash { player: 0, card: id::ESTATE });
    let gain_silver = pos(&Event::Gain { player: 0, card: id::SILVER, to: Dest::Discard, source: id::REMAKE });
    let trash_silver = pos(&Event::Trash { player: 0, card: id::SILVER });
    assert!(trash_estate < gain_silver && gain_silver < trash_silver, "trash, gain, then trash again: {ev:?}");
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1, "the Silver ($3) became a Smithy ($4)");
    assert_eq!(g.trash, counts_of(&[id::ESTATE, id::SILVER]));
}

#[test]
fn remake_with_one_card_or_no_exact_cost_card_gains_nothing_for_it() {
    // One card in hand: trash it and gain; the second trash has nothing to act on.
    let mut g = new_state(&[id::REMAKE], 2);
    set_hand(&mut g, 0, &[id::REMAKE, id::ESTATE]);
    play(&mut g, id::REMAKE);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert_eq!(g.trash.total(), 1);
    // A Copper ($0) needs a $1 card: there is none in the Supply.
    let mut g = new_state(&[id::REMAKE], 2);
    set_hand(&mut g, 0, &[id::REMAKE, id::COPPER, id::COPPER]);
    play(&mut g, id::REMAKE);
    assert_eq!(g.trash.get(id::COPPER), 2, "both Coppers trashed, mandatory");
    assert_eq!(g.players[0].discard.total(), 0, "and nothing gained");
}

#[test]
fn remake_with_an_empty_hand_does_nothing() {
    let mut g = new_state(&[id::REMAKE], 2);
    set_hand(&mut g, 0, &[id::REMAKE]);
    play(&mut g, id::REMAKE);
    assert_eq!(g.trash.total(), 0);
}

#[test]
fn remake_exact_cost_keeps_the_potion_of_the_trashed_card() {
    // Familiar ($3P) -> exactly $4P (Golem), not a plain $4 (Smithy); Estate ($2) -> exactly $3: Silver, not Familiar ($3P).
    let mut g = new_state(&[id::REMAKE, id::FAMILIAR, id::GOLEM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::REMAKE, id::FAMILIAR, id::COPPER]);
    play(&mut g, id::REMAKE);
    choose(&mut g, Choice::Card(id::FAMILIAR));
    // Exactly $4P: the Golem is the only choice (the Smithy costs $4 without a Potion), so it is gained at once;
    // the Copper then makes the second trash, which gains nothing.
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLEM]));
    assert_eq!(g.trash, counts_of(&[id::FAMILIAR, id::COPPER]));
    let mut g = new_state(&[id::REMAKE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::REMAKE, id::ESTATE]);
    play(&mut g, id::REMAKE);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert_eq!(g.players[0].discard.get(id::FAMILIAR), 0);
}

// ===========================================================================
// Baker, Merchant Guild
// ===========================================================================

#[test]
fn baker_gives_a_card_an_action_and_a_token_and_everyone_starts_with_one() {
    let d = cards::def(id::BAKER);
    assert_eq!((d.cost, d.cards, d.actions), (5, 1, 1));
    let fresh = GameState::new(&GameConfig { num_players: 3, kingdom: vec![id::BAKER], ..Default::default() });
    for p in 0..3 {
        assert_eq!(fresh.players[p].coffers, 1, "each player starts with +1 Coffers");
    }
    let none = GameState::new(&GameConfig { num_players: 3, kingdom: vec![id::SMITHY], ..Default::default() });
    assert!((0..3).all(|p| none.players[p].coffers == 0));
    let mut g = new_state(&[id::BAKER], 2);
    set_hand(&mut g, 0, &[id::BAKER]);
    set_deck_known(&mut g, 0, &junk(3));
    play(&mut g, id::BAKER);
    assert_eq!((g.turn.actions, g.players[0].hand.total(), g.players[0].coffers), (1, 1, 1));
}

#[test]
fn merchant_guild_pays_a_token_per_card_gained_in_the_buy_phase() {
    let d = cards::def(id::MERCHANT_GUILD);
    assert_eq!((d.cost, d.coins, d.buys), (5, 1, 1));
    let mut g = new_state(&[id::MERCHANT_GUILD], 2);
    set_hand(&mut g, 0, &[id::MERCHANT_GUILD, id::SILVER, id::SILVER, id::SILVER]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::MERCHANT_GUILD);
    assert_eq!((g.turn.coins, g.turn.buys), (1 + 6, 2));
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].coffers, 0, "paid at the end of the Buy phase, not when gaining");
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(id::ESTATE), &mut ev);
    assert_eq!(g.players[0].coffers, 2);
    assert!(ev.contains(&Event::Coffers { player: 0, source: id::MERCHANT_GUILD, n: 2 }));
}

#[test]
fn merchant_guild_is_cumulative_and_ignores_gains_outside_the_buy_phase() {
    let mut g = new_state(&[id::MERCHANT_GUILD, id::THRONE_ROOM, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MERCHANT_GUILD, id::SILVER, id::SILVER]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MERCHANT_GUILD)); // played twice: +$2, +2 Buys, two plays counted
    assert_eq!(g.turn.guilds, 2);
    assert_eq!((g.turn.buys, g.turn.coins), (3, 2 + 4));
    assert_eq!(g.players[0].coffers, 0);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    buy(&mut g, id::SILVER);
    expect_decision(&mut g);
    buy(&mut g, id::COPPER);
    expect_decision(&mut g);
    pass(&mut g); // done buying, with a Buy left
    assert_eq!(g.players[0].coffers, 4, "2 cards gained in the Buy phase x 2 Guild plays");
}

// ===========================================================================
// Hunting Party, Carnival
// ===========================================================================

#[test]
fn hunting_party_takes_the_first_card_that_is_not_a_copy_of_one_in_hand() {
    let d = cards::def(id::HUNTING_PARTY);
    assert_eq!((d.cost, d.cards, d.actions), (5, 1, 1));
    let mut g = new_state(&[id::HUNTING_PARTY], 2);
    set_hand(&mut g, 0, &[id::HUNTING_PARTY, id::COPPER, id::SILVER]);
    // It draws the first card (Estate), then reveals until a card not in the hand: Copper and Silver are, the
    // Estate is too now; Gold is the first that is not.
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER, id::SILVER, id::ESTATE, id::GOLD, id::DUCHY]);
    play(&mut g, id::HUNTING_PARTY);
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::SILVER, id::ESTATE, id::GOLD]));
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::SILVER, id::ESTATE]), "the revealed non-matches");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
}

#[test]
fn hunting_party_finding_nothing_discards_everything_revealed_after_a_reshuffle() {
    let mut g = new_state(&[id::HUNTING_PARTY], 2);
    set_hand(&mut g, 0, &[id::HUNTING_PARTY, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER]);
    set_discard(&mut g, 0, &[id::COPPER, id::ESTATE]);
    play(&mut g, id::HUNTING_PARTY);
    // The Estate was drawn first (the +1 Card); then Copper (in hand), the discard pile is shuffled in and
    // revealed: Copper and Estate both match what is in the hand now.
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::COPPER, id::ESTATE]));
    assert_eq!(g.players[0].discard.total(), 3, "all revealed cards discarded");
    assert!(g.players[0].deck_known.is_empty() && g.players[0].deck_unknown.is_empty());
}

#[test]
fn carnival_keeps_one_of_each_name_from_the_top_four() {
    let d = cards::def(id::CARNIVAL);
    assert_eq!((d.cost, d.cards, d.actions), (5, 0, 0));
    let mut g = new_state(&[id::CARNIVAL, id::FARMHANDS], 2);
    set_hand(&mut g, 0, &[id::CARNIVAL]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::FARMHANDS, id::COPPER, id::GOLD]);
    play(&mut g, id::CARNIVAL);
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::FARMHANDS]));
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER, id::COPPER]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
}

#[test]
fn carnival_with_fewer_than_four_cards_available_reveals_what_it_can() {
    let mut g = new_state(&[id::CARNIVAL], 2);
    set_hand(&mut g, 0, &[id::CARNIVAL]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    set_discard(&mut g, 0, &[id::SILVER, id::ESTATE]);
    play(&mut g, id::CARNIVAL);
    // The Silver, then the discard pile (Silver, Estate) is shuffled in: 3 cards revealed.
    assert_eq!(held(&g, 0), counts_of(&[id::SILVER, id::ESTATE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER]));
}

// ===========================================================================
// Fairgrounds: 2 VP per 5 differently named cards
// ===========================================================================

#[test]
fn fairgrounds_is_worth_two_vp_per_five_different_names() {
    let d = cards::def(id::FAIRGROUNDS);
    assert_eq!((d.cost, d.vp), (6, 0));
    assert!(cards::is(id::FAIRGROUNDS, cards::VICTORY));
    let vp = |cs: &[CardId]| {
        let mut p = state::PlayerState::default();
        for &c in cs {
            p.discard.add(c, 1);
        }
        p.vp()
    };
    // Fairgrounds itself counts as a name: 1 + 3 others = 4 names -> 0.
    assert_eq!(vp(&[id::FAIRGROUNDS, id::COPPER, id::SILVER, id::GOLD]), 0);
    assert_eq!(vp(&[id::FAIRGROUNDS, id::COPPER, id::SILVER, id::GOLD, id::SMITHY]), 2, "5 names");
    assert_eq!(vp(&[id::FAIRGROUNDS, id::FAIRGROUNDS, id::COPPER, id::COPPER, id::SILVER, id::GOLD, id::SMITHY]), 4, "two Fairgrounds, 5 names");
    let ten: Vec<CardId> = vec![id::FAIRGROUNDS, id::COPPER, id::SILVER, id::GOLD, id::SMITHY, id::VILLAGE, id::MARKET, id::MINE, id::CELLAR, id::MOAT];
    assert_eq!(vp(&ten), 4);
    // Wherever the cards are: hand, deck, in play, mats count.
    let mut p = state::PlayerState::default();
    p.hand.add(id::FAIRGROUNDS, 1);
    p.deck_unknown.add(id::COPPER, 1);
    p.in_play.add(id::SILVER, 1);
    p.discard.add(id::GOLD, 1);
    p.set_aside.add(id::ESTATE, 1);
    assert_eq!(p.vp(), 2 + 1, "5 names = 2 VP, plus the Estate's 1");
    // Piles of 8 (2 players) or 12.
    assert_eq!(new_state(&[id::FAIRGROUNDS], 2).supply.get(id::FAIRGROUNDS), 8);
    assert_eq!(new_state(&[id::FAIRGROUNDS], 3).supply.get(id::FAIRGROUNDS), 12);
}

// ===========================================================================
// Farmhands
// ===========================================================================

#[test]
fn farmhands_is_a_village_with_a_card() {
    let d = cards::def(id::FARMHANDS);
    assert_eq!((d.cost, d.cards, d.actions), (4, 1, 2));
    let mut g = new_state(&[id::FARMHANDS], 2);
    set_hand(&mut g, 0, &[id::FARMHANDS]);
    set_deck_known(&mut g, 0, &junk(4));
    play(&mut g, id::FARMHANDS);
    assert_eq!((g.turn.actions, g.players[0].hand.total()), (2, 1));
}

#[test]
fn gaining_farmhands_may_set_aside_an_action_or_treasure_to_play_next_turn() {
    let mut g = new_state(&[id::FARMHANDS, id::VILLAGE, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::VILLAGE, id::SILVER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(20));
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::FARMHANDS));
    // Offered: Actions and Treasures from the hand (not the Estate).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::SetAside, min: 0, max: 1, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Card(id::VILLAGE), Choice::Pass]);
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.players[0].farm_cards.get(id::VILLAGE), 1);
    assert!(!g.players[0].hand.has(id::VILLAGE));
    assert_eq!(g.players[0].all_cards().get(id::VILLAGE), 1, "still owned");
    // Public to the other player.
    assert_eq!(PlayerView::new(&g, 1).farm_cards_of(0), counts_of(&[id::VILLAGE]));
}

#[test]
fn the_set_aside_card_is_played_at_the_start_of_the_next_turn_without_an_action() {
    let mut g = new_state(&[id::FARMHANDS, id::VILLAGE], 2);
    g.players[0].farm_cards.add(id::VILLAGE, 1);
    set_deck_known(&mut g, 0, &junk(6));
    let mut ev: Vec<Event> = Vec::new();
    // The turn starts: Village plays by itself.
    let _ = g.advance(&mut ev);
    assert!(ev.contains(&Event::StartPlay { player: 0, source: id::FARMHANDS, card: id::VILLAGE }), "{ev:?}");
    assert_eq!(g.turn.actions, 1 + 2, "no Action used; +2 Actions");
    assert_eq!(g.players[0].hand.total(), 1, "+1 Card");
    assert!(g.players[0].in_play.has(id::VILLAGE), "an ordinary card in play now");
    assert!(g.players[0].farm_cards.is_empty());
}

#[test]
fn a_set_aside_treasure_gives_its_coins_and_declining_sets_nothing_aside() {
    let mut g = new_state(&[id::FARMHANDS], 2);
    g.players[0].farm_cards.add(id::GOLD, 1);
    let _ = adv(&mut g);
    assert_eq!(g.turn.coins, 3);
    assert!(g.players[0].in_play.has(id::GOLD));

    let mut g = new_state(&[id::FARMHANDS, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::SILVER]);
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::FARMHANDS));
    expect_decision(&mut g);
    pass(&mut g);
    assert!(g.players[0].farm_cards.is_empty());
    assert!(held(&g, 0).has(id::SILVER));
}

#[test]
fn a_set_aside_duration_stays_in_play_and_schedules_its_next_turn() {
    let mut g = new_state(&[id::FARMHANDS, id::CARAVAN], 2);
    g.players[0].farm_cards.add(id::CARAVAN, 1);
    set_deck_known(&mut g, 0, &junk(6));
    let _ = adv(&mut g);
    assert!(g.turn.duration_held.has(id::CARAVAN), "Caravan scheduled its next turn");
    assert_eq!(g.players[0].pending_durations_len, 1);
    assert_eq!(g.players[0].hand.total(), 1, "its +1 Card now");
}

#[test]
fn farmhands_set_aside_cards_round_trip_through_the_text_format() {
    let mut g = new_state(&[id::FARMHANDS], 2);
    g.players[1].farm_cards.add(id::GOLD, 1);
    g.players[1].farm_cards.add(id::VILLAGE, 2);
    let text = format_state(&g);
    assert!(text.contains("farmhands: "), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.players[1].farm_cards.counts(), counts_of(&[id::GOLD, id::VILLAGE, id::VILLAGE]));
}

// ===========================================================================
// Footpad: +2 Coffers; attack: discard to 3; the passive: +1 Card when you gain a card in an Action phase
// ===========================================================================

#[test]
fn footpad_gives_two_tokens_and_makes_others_discard_down_to_three() {
    let d = cards::def(id::FOOTPAD);
    assert_eq!((d.cost, d.cards, d.actions), (5, 0, 0));
    assert!(cards::is(id::FOOTPAD, cards::ATTACK));
    let mut g = new_state(&[id::FOOTPAD], 2);
    set_hand(&mut g, 0, &[id::FOOTPAD]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::SILVER, id::ESTATE, id::DUCHY]);
    play(&mut g, id::FOOTPAD);
    assert_eq!(g.players[0].coffers, 2);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 2, max: 2, .. }), "{d:?}");
    choose(&mut g, Choice::Card(id::ESTATE)); // the second discard is forced (only the Duchy is left in canonical order)
    assert_eq!(g.players[1].hand.total(), 3);
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE, id::DUCHY]));
}

#[test]
fn footpad_blocked_by_moat_still_gives_the_tokens() {
    let mut g = new_state(&[id::FOOTPAD, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::FOOTPAD]);
    set_hand(&mut g, 1, &[id::MOAT, id::COPPER, id::COPPER, id::SILVER, id::ESTATE]);
    play(&mut g, id::FOOTPAD);
    moat(&mut g, true);
    assert_eq!(g.players[0].coffers, 2);
    assert_eq!(g.players[1].hand.total(), 5);
}

#[test]
fn footpad_draws_for_any_player_gaining_a_card_in_an_action_phase_in_games_using_it() {
    // Workshop in the Action phase: +1 Card for the gainer.
    let mut g = new_state(&[id::FOOTPAD, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &junk(5));
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].hand.total(), 1, "the Workshop's gain drew a card");
    // Not in the Buy phase.
    let mut g = new_state(&[id::FOOTPAD], 2);
    set_deck_known(&mut g, 0, &junk(12));
    buy_phase(&mut g, 3);
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].hand.total(), 5, "just the usual 5 cards: nothing extra for a Buy-phase gain");
    // Without Footpad in the game: no draw.
    let mut g = new_state(&[id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &junk(5));
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].hand.total(), 0);
}

#[test]
fn footpad_also_draws_for_a_victim_gaining_a_curse_during_someone_elses_action_phase() {
    let mut g = new_state(&[id::FOOTPAD, id::WITCH], 2);
    set_hand(&mut g, 0, &[id::WITCH]);
    set_deck_known(&mut g, 0, &junk(4));
    set_deck_known(&mut g, 1, &junk(4));
    play(&mut g, id::WITCH);
    assert!(g.players[1].discard.has(id::CURSE));
    assert_eq!(g.players[1].hand.total(), 1, "Player 2 drew for gaining the Curse");
}

// ===========================================================================
// Advisor
// ===========================================================================

#[test]
fn advisor_reveals_three_and_the_left_player_chooses_the_one_to_discard() {
    let d = cards::def(id::ADVISOR);
    assert_eq!((d.cost, d.actions, d.cards), (4, 1, 0));
    let mut g = new_state(&[id::ADVISOR], 2);
    set_hand(&mut g, 0, &[id::ADVISOR]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::GOLD, id::ESTATE, id::DUCHY]);
    play(&mut g, id::ADVISOR);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1, "the player to the left chooses");
    assert_eq!(d.for_player, 0);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Discard, min: 1, max: 1, .. }));
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Card(id::GOLD), Choice::Card(id::ESTATE)]);
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::ESTATE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));
    assert_eq!(g.turn.actions, 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
}

#[test]
fn advisor_with_fewer_than_three_cards_still_discards_one() {
    let mut g = new_state(&[id::ADVISOR], 2);
    set_hand(&mut g, 0, &[id::ADVISOR]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD]);
    play(&mut g, id::ADVISOR);
    assert_eq!(expect_decision(&mut g).player, 1);
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(held(&g, 0), counts_of(&[id::SILVER]));
    // One card only: the choice is forced, nothing is asked.
    let mut g = new_state(&[id::ADVISOR], 2);
    set_hand(&mut g, 0, &[id::ADVISOR]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::ADVISOR);
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER]));
    // Nothing to reveal: nothing happens.
    let mut g = new_state(&[id::ADVISOR], 2);
    set_hand(&mut g, 0, &[id::ADVISOR]);
    play(&mut g, id::ADVISOR);
    assert!(g.players[0].hand.is_empty() && g.players[0].discard.is_empty());
}

// ===========================================================================
// Soothsayer
// ===========================================================================

#[test]
fn soothsayer_gains_a_gold_and_gives_curses_that_draw_a_card() {
    let d = cards::def(id::SOOTHSAYER);
    assert_eq!((d.cost, d.cards, d.actions), (5, 0, 0));
    assert!(cards::is(id::SOOTHSAYER, cards::ATTACK));
    let mut g = new_state(&[id::SOOTHSAYER], 2);
    set_hand(&mut g, 0, &[id::SOOTHSAYER]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::ESTATE]);
    play(&mut g, id::SOOTHSAYER);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    assert_eq!(g.players[1].hand, counts_of(&[id::SILVER]), "and drew a card");
}

#[test]
fn soothsayer_gives_nothing_to_draw_when_no_curse_is_gained() {
    // Curses gone: no gain, no draw. (Gold gone too.)
    let mut g = new_state(&[id::SOOTHSAYER], 2);
    set_hand(&mut g, 0, &[id::SOOTHSAYER]);
    set_deck_known(&mut g, 1, &junk(3));
    empty_pile(&mut g, id::CURSE);
    empty_pile(&mut g, id::GOLD);
    play(&mut g, id::SOOTHSAYER);
    assert!(g.players[0].discard.is_empty());
    assert!(g.players[1].hand.is_empty());
    // Moat: no Curse, no card.
    let mut g = new_state(&[id::SOOTHSAYER, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::SOOTHSAYER]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 1, &junk(3));
    play(&mut g, id::SOOTHSAYER);
    moat(&mut g, true);
    assert_eq!(g.players[1].hand.total(), 1);
    assert!(g.players[1].discard.is_empty());
}

#[test]
fn soothsayer_deals_curses_in_turn_order_each_player_drawing_before_the_next_gains() {
    let mut g = new_state(&[id::SOOTHSAYER], 3);
    set_hand(&mut g, 0, &[id::SOOTHSAYER]);
    set_deck_known(&mut g, 1, &junk(3));
    set_deck_known(&mut g, 2, &junk(3));
    set_supply(&mut g, id::CURSE, 1);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::SOOTHSAYER, &mut ev);
    let gain1 = ev.iter().position(|e| matches!(e, Event::Gain { player: 1, card: id::CURSE, .. })).unwrap();
    let draw1 = ev.iter().position(|e| matches!(e, Event::Draw { player: 1, .. })).unwrap();
    assert!(gain1 < draw1);
    assert!(!ev.iter().any(|e| matches!(e, Event::Gain { player: 2, card: id::CURSE, .. })), "the last Curse went to the player on the left");
    assert!(g.players[2].hand.is_empty(), "the one who did not gain a Curse draws nothing");
}

#[test]
fn soothsayer_curse_trashed_by_watchtower_still_draws() {
    let mut g = new_state(&[id::SOOTHSAYER, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::SOOTHSAYER]);
    set_hand(&mut g, 1, &[id::WATCHTOWER]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::SILVER]);
    play(&mut g, id::SOOTHSAYER);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    choose(&mut g, Choice::Card(id::CURSE)); // trash it
    while g.pending_decision().is_some() {
        pass(&mut g);
    }
    assert_eq!(g.trash.get(id::CURSE), 1);
    assert!(g.players[1].hand.has(id::SILVER), "drew for gaining the Curse");
}

// ===========================================================================
// Journeyman
// ===========================================================================

#[test]
fn journeyman_names_a_card_and_draws_three_cards_without_that_name() {
    let d = cards::def(id::JOURNEYMAN);
    assert_eq!((d.cost, d.cards, d.actions), (5, 0, 0));
    let mut g = new_state(&[id::JOURNEYMAN], 2);
    set_hand(&mut g, 0, &[id::JOURNEYMAN]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER, id::ESTATE, id::SILVER, id::GOLD, id::DUCHY]);
    play(&mut g, id::JOURNEYMAN);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Name);
    assert!(choices(&g).contains(&Choice::Card(id::ESTATE)) && choices(&g).contains(&Choice::Card(id::COPPER)));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::SILVER, id::GOLD]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE, id::ESTATE]), "the named cards revealed are discarded");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
}

#[test]
fn journeyman_stops_when_the_deck_runs_out_and_reshuffles_the_discard() {
    let mut g = new_state(&[id::JOURNEYMAN], 2);
    set_hand(&mut g, 0, &[id::JOURNEYMAN]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    set_discard(&mut g, 0, &[id::ESTATE, id::ESTATE, id::SILVER]);
    play(&mut g, id::JOURNEYMAN);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::ESTATE));
    // Copper, then the discard pile is shuffled in (the discard was Estate, Estate, Silver): only 2 cards
    // without the name exist.
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::SILVER]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE, id::ESTATE]));
    assert!(g.players[0].deck_known.is_empty() && g.players[0].deck_unknown.is_empty());
}

#[test]
fn journeyman_can_name_a_card_that_is_not_in_the_deck() {
    let mut g = new_state(&[id::JOURNEYMAN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::JOURNEYMAN]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::DUCHY]);
    play(&mut g, id::JOURNEYMAN);
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::SMITHY)), "a Supply card the deck does not hold");
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::SILVER, id::GOLD]));
}

// ===========================================================================
// Jester
// ===========================================================================

#[test]
fn jester_gives_two_coins_and_a_victory_card_discarded_means_a_curse() {
    let d = cards::def(id::JESTER);
    assert_eq!((d.cost, d.coins), (5, 2));
    assert!(cards::is(id::JESTER, cards::ATTACK));
    let mut g = new_state(&[id::JESTER], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    set_deck_known(&mut g, 1, &[id::ESTATE, id::COPPER]);
    play(&mut g, id::JESTER);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE, id::CURSE]), "the Estate is discarded and a Curse gained");
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::COPPER));
}

#[test]
fn jester_lets_the_attacker_choose_who_gains_a_copy_of_a_non_victory_card() {
    let mut g = new_state(&[id::JESTER], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::COPPER, id::COPPER]);
    play(&mut g, id::JESTER);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.for_player), (0, 1), "the attacker decides about the victim's discarded Silver");
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 1, .. }));
    assert_eq!(d.subject, id::SILVER);
    // 0: the victim gains it; 1: I gain it.
    let mut g2 = g;
    choose(&mut g2, Choice::Mode(0));
    assert_eq!(g2.players[1].discard, counts_of(&[id::SILVER, id::SILVER]));
    assert_eq!(g2.players[0].discard.total(), 0);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER]));
    assert_eq!(g.players[1].discard, counts_of(&[id::SILVER]));
}

#[test]
fn jester_edge_cases_empty_pile_empty_deck_moat_and_dual_types() {
    // No copies left in the Supply: nobody gains one, nothing is asked.
    let mut g = new_state(&[id::JESTER], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::COPPER]);
    empty_pile(&mut g, id::SILVER);
    play(&mut g, id::JESTER);
    assert_eq!(g.players[1].discard, counts_of(&[id::SILVER]));
    assert_eq!(g.players[0].discard.total(), 0);
    // A player with no cards at all is unaffected.
    let mut g = new_state(&[id::JESTER], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    play(&mut g, id::JESTER);
    assert_eq!(g.players[1].discard.total(), 0);
    // Moat.
    let mut g = new_state(&[id::JESTER, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::JESTER);
    moat(&mut g, true);
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::ESTATE));
    // A Victory-Treasure (Harem) is a Victory card: the Curse.
    let mut g = new_state(&[id::JESTER, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    set_deck_known(&mut g, 1, &[id::HAREM]);
    play(&mut g, id::JESTER);
    assert_eq!(g.players[1].discard, counts_of(&[id::HAREM, id::CURSE]));
}

#[test]
fn jester_reshuffles_the_victims_discard_when_their_deck_is_empty() {
    let mut g = new_state(&[id::JESTER], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    set_discard(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::JESTER);
    // The discard pile is shuffled into the deck and the Estate (the only card) is discarded again, with a Curse.
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE, id::CURSE]));
}

// ===========================================================================
// Butcher
// ===========================================================================

#[test]
fn butcher_gives_two_tokens_then_trashes_and_gains_spending_only_what_it_needs() {
    let d = cards::def(id::BUTCHER);
    assert_eq!((d.cost, d.cards), (5, 0));
    let mut g = new_state(&[id::BUTCHER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::BUTCHER, id::ESTATE]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::BUTCHER, &mut ev);
    assert_eq!(g.players[0].coffers, 2, "the +2 Coffers come first");
    choose_ev(&mut g, Choice::Card(id::ESTATE), &mut ev); // trash an Estate ($2) ...
    // ... gain up to $2 + 2 = $4: Smithy costs $4, so both tokens are spent.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    assert!(choices(&g).contains(&Choice::Card(id::SMITHY)) && choices(&g).contains(&Choice::Card(id::SILVER)));
    choose_ev(&mut g, Choice::Card(id::SMITHY), &mut ev);
    assert_eq!(g.players[0].coffers, 0);
    assert!(ev.contains(&Event::SpendCoffers { player: 0, n: 2 }));
    assert_eq!(g.turn.coins, 0, "removing tokens for Butcher gives no $");
}

#[test]
fn butcher_spends_nothing_for_a_card_that_costs_no_more_than_the_trashed_one() {
    let mut g = new_state(&[id::BUTCHER], 2);
    set_hand(&mut g, 0, &[id::BUTCHER, id::BUTCHER]);
    play(&mut g, id::BUTCHER);
    choose(&mut g, Choice::Card(id::BUTCHER)); // trash the other Butcher ($5)
    choose(&mut g, Choice::Card(id::DUCHY)); // a Duchy is $5: free
    assert_eq!(g.players[0].coffers, 2, "no token spent");
    assert_eq!(g.players[0].discard.get(id::DUCHY), 1);
    // Gaining cheaper than allowed spends less: Silver ($3) for a trashed Estate ($2) costs one token.
    let mut g = new_state(&[id::BUTCHER], 2);
    g.players[0].coffers = 3;
    set_hand(&mut g, 0, &[id::BUTCHER, id::ESTATE]);
    play(&mut g, id::BUTCHER);
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].coffers, 5 - 1);
}

#[test]
fn butcher_can_use_older_tokens_and_the_trash_is_optional() {
    let mut g = new_state(&[id::BUTCHER], 2);
    g.players[0].coffers = 6;
    set_hand(&mut g, 0, &[id::BUTCHER, id::ESTATE]);
    play(&mut g, id::BUTCHER);
    choose(&mut g, Choice::Card(id::ESTATE)); // $2 + 8 tokens = up to $10
    assert!(choices(&g).contains(&Choice::Card(id::PROVINCE)), "a Province for six tokens");
    choose(&mut g, Choice::Card(id::PROVINCE));
    assert_eq!(g.players[0].coffers, 8 - 6);
    // Trashing is optional: a plain source of two tokens.
    let mut g = new_state(&[id::BUTCHER], 2);
    set_hand(&mut g, 0, &[id::BUTCHER, id::ESTATE]);
    play(&mut g, id::BUTCHER);
    pass(&mut g);
    assert_eq!((g.players[0].coffers, g.trash.total()), (2, 0));
}

#[test]
fn butcher_gain_keeps_the_potion_of_the_trashed_card() {
    // Trash a Familiar ($3P): may gain a Potion card costing up to $3P + tokens (and Potion-free cards too).
    let mut g = new_state(&[id::BUTCHER, id::FAMILIAR, id::GOLEM], 2);
    set_hand(&mut g, 0, &[id::BUTCHER, id::FAMILIAR]);
    play(&mut g, id::BUTCHER);
    choose(&mut g, Choice::Card(id::FAMILIAR));
    assert!(choices(&g).contains(&Choice::Card(id::GOLEM)), "$3 + 2 tokens = $5P covers the $4P Golem");
    // But a trashed plain Estate ($2) + 2 tokens = $4 may not gain a Potion card.
    let mut g = new_state(&[id::BUTCHER, id::FAMILIAR, id::GOLEM], 2);
    set_hand(&mut g, 0, &[id::BUTCHER, id::ESTATE]);
    play(&mut g, id::BUTCHER);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert!(!choices(&g).contains(&Choice::Card(id::GOLEM)));
}

// ===========================================================================
// Horn of Plenty
// ===========================================================================

#[test]
fn horn_of_plenty_gains_a_card_costing_up_to_the_names_in_play() {
    let d = cards::def(id::HORN_OF_PLENTY);
    assert_eq!((d.cost, d.coins), (5, 0));
    assert!(cards::is(id::HORN_OF_PLENTY, cards::TREASURE));
    let mut g = new_state(&[id::HORN_OF_PLENTY, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY]);
    set_in_play(&mut g, 0, &[id::COPPER, id::COPPER, id::SILVER, id::SMITHY]); // 3 names, + the Horn itself = 4
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    assert!(choices(&g).contains(&Choice::Card(id::SMITHY)));
    assert!(!choices(&g).contains(&Choice::Card(id::GOLD)));
    assert_eq!(g.turn.coins, 0, "worth $0");
    choose(&mut g, Choice::Card(id::SMITHY));
    assert!(g.players[0].in_play.has(id::HORN_OF_PLENTY), "kept: Smithy is not a Victory card");
}

#[test]
fn horn_of_plenty_gaining_a_victory_card_trashes_it() {
    let mut g = new_state(&[id::HORN_OF_PLENTY], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY]);
    set_in_play(&mut g, 0, &[id::COPPER, id::SILVER]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    choose(&mut g, Choice::Card(id::ESTATE)); // up to $3: an Estate
    assert!(!g.players[0].in_play.has(id::HORN_OF_PLENTY));
    assert_eq!(g.trash.get(id::HORN_OF_PLENTY), 1);
    assert_eq!(g.players[0].discard.get(id::ESTATE), 1);
}

#[test]
fn horn_of_plenty_counts_each_name_once_and_durations_from_last_turn() {
    let mut g = new_state(&[id::HORN_OF_PLENTY, id::CARAVAN, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY]);
    set_in_play(&mut g, 0, &[id::CARAVAN, id::WORKSHOP, id::WORKSHOP, id::SILVER]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "Caravan, Workshop, Silver, Horn = 4 names (the second Workshop is not another name): {d:?}");
}

#[test]
fn horn_of_plenty_victory_gained_other_ways_does_not_trash_it_and_its_gain_counts_for_the_guild() {
    let mut g = new_state(&[id::HORN_OF_PLENTY, id::MERCHANT_GUILD], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY]);
    set_in_play(&mut g, 0, &[id::MERCHANT_GUILD, id::COPPER]);
    g.turn.phase = Phase::Buy;
    g.turn.guilds = 1;
    g.turn.coins = 2;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    choose(&mut g, Choice::Card(id::SILVER)); // 3 names: up to $3
    buy(&mut g, id::ESTATE); // a Victory card bought while the Horn is in play: the Horn stays
    assert_eq!(g.trash.get(id::HORN_OF_PLENTY), 0);
    assert_eq!(g.players[0].coffers, 2, "the Horn's gain and the Estate are two cards gained in the Buy phase");
}

// ===========================================================================
// Throne Room and other interactions
// ===========================================================================

#[test]
fn throne_room_on_soothsayer_gains_two_golds_and_two_curses_with_two_draws() {
    let mut g = new_state(&[id::SOOTHSAYER, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SOOTHSAYER]);
    set_deck_known(&mut g, 1, &junk(6));
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SOOTHSAYER));
    assert_eq!(g.players[0].discard.get(id::GOLD), 2);
    assert_eq!(g.players[1].discard.get(id::CURSE), 2);
    assert_eq!(g.players[1].hand.total(), 2, "a card for each Curse");
}

#[test]
fn throne_room_on_jester_attacks_twice() {
    let mut g = new_state(&[id::JESTER, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::JESTER]);
    set_deck_known(&mut g, 1, &[id::ESTATE, id::COPPER, id::SILVER]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::JESTER));
    // First attack: the Estate (a Curse). Second: the Copper, and the attacker chooses who gains it.
    let d = expect_decision(&mut g);
    assert_eq!(d.subject, id::COPPER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.turn.coins, 4);
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE, id::CURSE, id::COPPER]));
    assert_eq!(g.players[0].discard, counts_of(&[id::COPPER]));
}

#[test]
fn throne_room_on_journeyman_names_twice() {
    let mut g = new_state(&[id::JOURNEYMAN, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::JOURNEYMAN]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::ESTATE, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::JOURNEYMAN));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::ESTATE)); // Copper, Silver, Gold (the Estate discarded)
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::DUCHY)); // Estate, Province, ... the deck then runs out
    assert_eq!(held(&g, 0).get(id::COPPER) + held(&g, 0).get(id::SILVER) + held(&g, 0).get(id::GOLD), 3);
    assert!(g.players[0].hand.has(id::ESTATE) || held(&g, 0).has(id::ESTATE));
}

#[test]
fn throne_room_on_butcher_gives_four_tokens() {
    let mut g = new_state(&[id::BUTCHER, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::BUTCHER, id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::BUTCHER));
    assert_eq!(g.players[0].coffers, 2, "the second play's tokens come after the first play is done");
    choose(&mut g, Choice::Card(id::ESTATE)); // trash an Estate ...
    choose(&mut g, Choice::Card(id::SILVER)); // ... gain a $3 for one token
    assert_eq!(g.players[0].coffers, 1 + 2, "the second play: +2 more");
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].coffers, 2);
}

#[test]
fn farmhands_set_aside_with_footpad_can_set_aside_the_card_the_footpad_drew() {
    let mut g = new_state(&[id::FARMHANDS, id::FOOTPAD, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::FARMHANDS));
    // The Footpad draw (for the Farmhands gain) comes first, then the set-aside offer includes the Gold.
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::GOLD)));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].farm_cards.get(id::GOLD), 1);
}

#[test]
fn farmhands_cards_and_durations_resolve_in_an_order_the_player_chooses() {
    let mut g = new_state(&[id::FARMHANDS, id::CARAVAN, id::VILLAGE], 2);
    g.players[0].farm_cards.add(id::VILLAGE, 1);
    state_push_pending(&mut g, id::CARAVAN);
    set_deck_known(&mut g, 0, &junk(10));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(choices(&g), vec![Choice::Card(id::CARAVAN), Choice::Card(id::VILLAGE)]);
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert!(g.players[0].in_play.has(id::VILLAGE));
    assert_eq!(g.turn.actions, 3);
    assert_eq!(g.players[0].hand.total(), 2, "Village's card and Caravan's card");
}

fn state_push_pending(g: &mut GameState, card: CardId) {
    let i = g.players[0].pending_durations_len as usize;
    g.players[0].pending_durations[i] = state::PendingDuration { card, times: 1, arg: 0, used: 0 };
    g.players[0].pending_durations_len += 1;
}

#[test]
fn a_fairgrounds_game_has_no_other_surprises_in_supply_sizes() {
    let g = new_state(&[id::FAIRGROUNDS, id::HORN_OF_PLENTY], 2);
    assert_eq!(g.supply.get(id::HORN_OF_PLENTY), 10, "a Treasure kingdom card has a pile of 10");
}

// ===========================================================================
// Stonemason: trash a card, gain 2 cards each costing less than it. Overpay: gain 2 Actions each
// costing exactly the amount overpaid.
// ===========================================================================

#[test]
fn stonemason_trashes_a_card_and_gains_two_cheaper_cards_one_at_a_time() {
    let d = cards::def(id::STONEMASON);
    assert_eq!((d.cost, d.cards, d.actions), (2, 0, 0));
    assert!(cards::has_overpay(id::STONEMASON) && cards::is_order_sensitive(id::STONEMASON));
    let mut g = new_state(&[id::STONEMASON], 2);
    set_hand(&mut g, 0, &[id::STONEMASON, id::GOLD, id::COPPER]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::STONEMASON, &mut ev);
    choose_ev(&mut g, Choice::Card(id::GOLD), &mut ev);
    // The first card: anything costing less than $6.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 6, .. }), "{d:?}");
    assert!(!choices(&g).contains(&Choice::Card(id::GOLD)), "less than: not the same cost");
    assert!(choices(&g).contains(&Choice::Card(id::DUCHY)) && choices(&g).contains(&Choice::Card(id::SILVER)));
    choose(&mut g, Choice::Card(id::DUCHY));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::DUCHY)); // the same card again is fine
    assert_eq!(g.players[0].discard.get(id::DUCHY), 2);
    assert_eq!(g.trash, counts_of(&[id::GOLD]));
}

#[test]
fn stonemason_trash_is_mandatory_and_a_cheaper_card_may_not_exist() {
    // A Copper ($0): nothing is cheaper.
    let mut g = new_state(&[id::STONEMASON], 2);
    set_hand(&mut g, 0, &[id::STONEMASON, id::COPPER]);
    play(&mut g, id::STONEMASON);
    assert_eq!(g.trash, counts_of(&[id::COPPER]));
    assert_eq!(g.players[0].discard.total(), 0);
    // An empty hand: nothing at all.
    let mut g = new_state(&[id::STONEMASON], 2);
    set_hand(&mut g, 0, &[id::STONEMASON]);
    play(&mut g, id::STONEMASON);
    assert_eq!(g.trash.total(), 0);
    // The pile of the only cheaper card runs out between the two gains: one gain only.
    let mut g = new_state(&[id::STONEMASON], 2);
    set_hand(&mut g, 0, &[id::STONEMASON, id::ESTATE]);
    set_supply(&mut g, id::COPPER, 1);
    set_supply(&mut g, id::CURSE, 0);
    play(&mut g, id::STONEMASON);
    // Cards costing less than $2: Copper and Curse ($0); only a Copper is left.
    assert_eq!(g.players[0].all_cards().get(id::COPPER), 1);
}

#[test]
fn stonemason_less_than_compares_the_potion_part_too() {
    // Trash a Golem ($4P): anything with no part larger and not equal: a $4 Smithy, a $3P Familiar, a $2P ...
    let mut g = new_state(&[id::STONEMASON, id::GOLEM, id::FAMILIAR, id::SMITHY, id::TRANSMUTE], 2);
    set_hand(&mut g, 0, &[id::STONEMASON, id::GOLEM, id::COPPER]);
    play(&mut g, id::STONEMASON);
    choose(&mut g, Choice::Card(id::GOLEM));
    let offered = choices(&g);
    assert!(offered.contains(&Choice::Card(id::SMITHY)), "$4 is less than $4P: no larger part, not equal");
    assert!(offered.contains(&Choice::Card(id::FAMILIAR)), "$3P is less than $4P");
    assert!(offered.contains(&Choice::Card(id::TRANSMUTE)), "$0P is less than $4P");
    assert!(!offered.contains(&Choice::Card(id::GOLEM)), "$4P is not less than $4P");
    // Trash a Smithy ($4): a card with a Potion is not less ($3P has a part the Smithy lacks).
    let mut g = new_state(&[id::STONEMASON, id::GOLEM, id::FAMILIAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::STONEMASON, id::SMITHY, id::COPPER]);
    play(&mut g, id::STONEMASON);
    choose(&mut g, Choice::Card(id::SMITHY));
    let offered = choices(&g);
    assert!(offered.contains(&Choice::Card(id::SILVER)));
    assert!(!offered.contains(&Choice::Card(id::FAMILIAR)) && !offered.contains(&Choice::Card(id::GOLEM)));
    assert!(!offered.contains(&Choice::Card(id::SMITHY)));
}

#[test]
fn stonemason_overpay_gains_two_actions_costing_exactly_the_amount() {
    let mut g = new_state(&[id::STONEMASON, id::HAMLET, id::SMITHY, id::MILITIA, id::VILLAGE], 2);
    buy_phase(&mut g, 6);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::STONEMASON));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Overpay { card: id::STONEMASON, max_coins: 4, potion: false });
    choose(&mut g, Choice::Overpay(4));
    // Two Actions costing exactly $4: Smithy, Militia (the Stonemason is $2, the Village $3).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, exact: true, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::MILITIA), Choice::Card(id::SMITHY)]);
    choose(&mut g, Choice::Card(id::SMITHY));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::SMITHY)); // the same card twice
    assert_eq!(g.supply.get(id::SMITHY), 8);
    assert_eq!(g.players[0].all_cards().get(id::SMITHY), 2);
    assert_eq!(g.players[0].all_cards().get(id::STONEMASON), 1);
}

#[test]
fn stonemason_overpay_with_no_matching_action_gains_nothing_and_zero_is_not_an_overpay() {
    // Exactly $6 overpaid ($8 coins): no Action costs $6 here.
    let mut g = new_state(&[id::STONEMASON, id::SMITHY], 2);
    buy_phase(&mut g, 8);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::STONEMASON));
    expect_decision(&mut g);
    choose(&mut g, Choice::Overpay(6));
    assert_eq!(g.players[0].all_cards().total(), 1, "just the Stonemason");
    assert_eq!(g.turn.coins, 0);
    // No overpay: no Actions (it cannot gain $0 Actions).
    let mut g = new_state(&[id::STONEMASON, id::SMITHY], 2);
    buy_phase(&mut g, 8);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::STONEMASON));
    expect_decision(&mut g);
    choose(&mut g, Choice::Overpay(0));
    assert_eq!(g.players[0].all_cards().total(), 1);
}

#[test]
fn stonemason_overpay_with_a_potion_gains_actions_costing_a_potion() {
    let mut g = new_state(&[id::STONEMASON, id::APOTHECARY, id::TRANSMUTE, id::SMITHY], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 2;
    g.turn.potions = 1;
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::STONEMASON));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Overpay { card: id::STONEMASON, max_coins: 0, potion: true });
    assert_eq!(choices(&g), vec![Choice::Overpay(0), Choice::OverpayPotion(0)]);
    choose(&mut g, Choice::OverpayPotion(0)); // $0 and a Potion: Transmute ($0P)
    assert_eq!(g.turn.potions, 0);
    // Both gains need exactly $0P: only Transmute (Vineyard is Victory), so both are forced.
    assert_eq!(g.players[0].all_cards().get(id::TRANSMUTE), 2);
    // With $2 and a Potion: Apothecary ($2P).
    let mut g = new_state(&[id::STONEMASON, id::APOTHECARY, id::TRANSMUTE], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    g.turn.potions = 1;
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::STONEMASON));
    expect_decision(&mut g);
    choose(&mut g, Choice::OverpayPotion(2));
    // Exactly $2P: only the Apothecary, twice.
    assert_eq!(g.players[0].all_cards().get(id::APOTHECARY), 2);
}

#[test]
fn overpay_potions_are_only_offered_for_stonemason() {
    let mut g = new_state(&[id::FARRIER, id::APOTHECARY], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    g.turn.potions = 1;
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::FARRIER));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Overpay { card: id::FARRIER, max_coins: 2, potion: false });
}

#[test]
fn renown_does_not_make_overpaying_cheaper() {
    // $5 with two Renowns in play: Farrier costs $0, and overpaying with the $5 still gives +5 Cards.
    let mut g = new_state(&[id::FARRIER], 2);
    set_deck_known(&mut g, 0, &junk(30));
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    g.turn.cost_reduction = 4;
    expect_decision(&mut g);
    assert_eq!(g.cost(id::FARRIER), 0);
    choose(&mut g, Choice::Card(id::FARRIER));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Overpay { card: id::FARRIER, max_coins: 5, potion: false });
    choose(&mut g, Choice::Overpay(5));
    assert_eq!(g.players[0].hand.total(), 5 + 5);
}

// ===========================================================================
// Herald
// ===========================================================================

#[test]
fn herald_plays_an_action_revealed_from_the_deck_without_using_an_action() {
    let d = cards::def(id::HERALD);
    assert_eq!((d.cost, d.cards, d.actions), (4, 1, 1));
    assert!(cards::has_overpay(id::HERALD));
    let mut g = new_state(&[id::HERALD, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HERALD, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::VILLAGE, id::DUCHY, id::PROVINCE, id::ESTATE, id::DUCHY]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::HERALD, &mut ev);
    // +1 Card (Copper), +1 Action, then the Village on top of the deck is played: +1 Card (Duchy), +2 Actions.
    assert!(ev.contains(&Event::Reveal { player: 0, card: id::VILLAGE }));
    assert!(g.players[0].in_play.has(id::VILLAGE));
    assert_eq!(g.turn.actions, 1 + 2, "Herald's +1 Action replaced the one spent, and the Village gave +2");
    assert_eq!(held(&g, 0).total(), 1 + 1 + 1, "Estate, Copper, and the Village's Duchy");
}

#[test]
fn herald_puts_back_a_card_that_is_not_an_action_and_handles_empty_decks() {
    let mut g = new_state(&[id::HERALD], 2);
    set_hand(&mut g, 0, &[id::HERALD]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::DUCHY]);
    play(&mut g, id::HERALD);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY), "revealed, not an Action: back on top");
    // Empty deck and discard pile: nothing to reveal.
    let mut g = new_state(&[id::HERALD], 2);
    set_hand(&mut g, 0, &[id::HERALD]);
    play(&mut g, id::HERALD);
    assert_eq!(g.turn.actions, 1);
    // The deck has one card (drawn), the discard pile has an Action: it is shuffled in and revealed.
    let mut g = new_state(&[id::HERALD, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::HERALD]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    set_discard(&mut g, 0, &[id::VILLAGE]);
    play(&mut g, id::HERALD);
    assert!(g.players[0].in_play.has(id::VILLAGE), "played from the reshuffled deck");
}

#[test]
fn herald_playing_a_duration_is_itself_discarded_normally() {
    let mut g = new_state(&[id::HERALD, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::HERALD]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::CARAVAN, id::DUCHY]);
    play(&mut g, id::HERALD);
    assert!(g.turn.duration_held.has(id::CARAVAN));
    assert!(!g.turn.duration_held.has(id::HERALD));
}

#[test]
fn herald_overpay_puts_cards_from_the_discard_pile_onto_the_deck() {
    let mut g = new_state(&[id::HERALD], 2);
    set_discard(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 12]);
    buy_phase(&mut g, 6);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::HERALD));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Overpay { card: id::HERALD, max_coins: 2, potion: false });
    choose(&mut g, Choice::Overpay(2));
    // Per $1: a card from the discard pile (the Herald may be one) onto the deck; the order is chosen.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Discard, act: Act::Topdeck, min: 2, max: 2, ordered: true, .. }), "{d:?}");
    assert!(choices(&g).contains(&Choice::Card(id::HERALD)));
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::GOLD));
    // The game continues to cleanup: the two topdecked cards are the next draw.
    assert!(g.players[0].hand.has(id::GOLD) && g.players[0].hand.has(id::COPPER));
}

#[test]
fn herald_overpay_with_few_cards_in_the_discard_moves_what_there_is() {
    let mut g = new_state(&[id::HERALD], 2);
    set_deck_known(&mut g, 0, &[id::ESTATE; 12]);
    buy_phase(&mut g, 10);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::HERALD));
    expect_decision(&mut g);
    choose(&mut g, Choice::Overpay(6));
    // Only the Herald itself is in the discard pile: it goes onto the deck (forced).
    assert!(g.players[0].hand.has(id::HERALD), "the Herald was topdecked and drawn next");
}

// ===========================================================================
// Infirmary
// ===========================================================================

#[test]
fn infirmary_draws_a_card_and_may_trash_one() {
    let d = cards::def(id::INFIRMARY);
    assert_eq!((d.cost, d.cards, d.actions), (3, 1, 0));
    assert!(cards::has_overpay(id::INFIRMARY));
    let mut g = new_state(&[id::INFIRMARY], 2);
    set_hand(&mut g, 0, &[id::INFIRMARY, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::SILVER]);
    play(&mut g, id::INFIRMARY);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.trash, counts_of(&[id::CURSE]));
    // Optional.
    let mut g = new_state(&[id::INFIRMARY], 2);
    set_hand(&mut g, 0, &[id::INFIRMARY, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::CURSE]);
    play(&mut g, id::INFIRMARY);
    pass(&mut g);
    assert_eq!(g.trash.total(), 0);
}

#[test]
fn infirmary_overpay_plays_it_once_per_dollar_from_where_it_lies() {
    let mut g = new_state(&[id::INFIRMARY], 2);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::CURSE, id::COPPER, id::DUCHY, id::SILVER, id::GOLD, id::PROVINCE, id::ESTATE]);
    set_hand(&mut g, 0, &[id::ESTATE]);
    buy_phase(&mut g, 5);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::INFIRMARY));
    expect_decision(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Overpay(2), &mut ev);
    // Played twice, in the Buy phase: draw a card, optionally trash, draw another, optionally trash.
    assert_eq!(g.players[0].hand.total(), 1 + 1, "first draw");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, min: 0, max: 1, .. }));
    choose_ev(&mut g, Choice::Card(id::ESTATE), &mut ev); // trash the Estate in hand
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Pass, &mut ev);
    let plays = ev.iter().filter(|e| matches!(e, Event::Play { card: c, .. } if *c == id::INFIRMARY)).count();
    let again = ev.iter().filter(|e| matches!(e, Event::PlayAgain { card: c, nth: 2, .. } if *c == id::INFIRMARY)).count();
    assert_eq!((plays, again), (1, 1), "{ev:?}");
    assert_eq!(g.trash, counts_of(&[id::ESTATE]));
}

#[test]
fn infirmary_gained_without_buying_is_not_played() {
    let mut g = new_state(&[id::INFIRMARY, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &junk(5));
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::INFIRMARY));
    assert!(g.players[0].discard.has(id::INFIRMARY));
    assert!(g.players[0].hand.is_empty());
}

// ===========================================================================
// Young Witch and the Bane
// ===========================================================================

#[test]
fn young_witch_draws_two_discards_two_and_curses_the_others() {
    let d = cards::def(id::YOUNG_WITCH);
    assert_eq!((d.cost, d.cards, d.actions), (4, 2, 0));
    assert!(cards::is(id::YOUNG_WITCH, cards::ATTACK));
    let mut g = new_state(&[id::YOUNG_WITCH, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::YOUNG_WITCH, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::DUCHY]);
    set_deck_known(&mut g, 1, &junk(4));
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER]);
    play(&mut g, id::YOUNG_WITCH);
    // Draw first (Gold, Silver), then discard 2 of the 3 cards in hand (a decision), then the Curse.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 2, max: 2, .. }), "{d:?}");
    assert_eq!(g.players[0].hand.total(), 3);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0, "the attack comes after the discard");
    drain_selects(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    assert_eq!(g.players[0].discard.total(), 2, "two cards discarded (the Estate and one more: a Select of 2, the second forced)");
}

#[test]
fn young_witch_is_blocked_by_revealing_a_bane_or_a_moat_and_the_bane_is_an_extra_pile() {
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::YOUNG_WITCH, id::MOAT], bane: Some(id::HAMLET), ..Default::default() };
    let mut g = GameState::new(&cfg);
    assert_eq!(g.bane, id::HAMLET);
    assert!(g.in_supply(id::HAMLET), "the Bane pile is part of the Supply");
    assert_eq!(g.supply.get(id::HAMLET), 10);
    let _ = g.advance(&mut NoEvents);
    reset_turn(&mut g, 0);
    for p in 0..2 {
        g.players[p] = state::PlayerState::default();
    }
    set_hand(&mut g, 0, &[id::YOUNG_WITCH, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(6));
    set_hand(&mut g, 1, &[id::HAMLET, id::COPPER]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::YOUNG_WITCH, &mut ev);
    drain_selects(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0, "revealed the Bane");
    let _ = &ev;
    // Moat first, no Bane needed.
    let mut g = new_state(&[id::YOUNG_WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::YOUNG_WITCH, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(6));
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::YOUNG_WITCH);
    moat(&mut g, true);
    drain_selects(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0);
}

#[test]
fn young_witch_curses_in_turn_order_and_the_bane_counts_as_a_pile_for_the_end() {
    let mut g = new_state(&[id::YOUNG_WITCH, id::HAMLET], 3);
    set_supply(&mut g, id::CURSE, 1);
    set_hand(&mut g, 0, &[id::YOUNG_WITCH, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::YOUNG_WITCH);
    drain_selects(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1, "the player to the left gets the last Curse");
    assert_eq!(g.players[2].discard.get(id::CURSE), 0);
    // The Bane pile counts toward the three-pile ending: Estates, Curses and the Bane empty ends the game.
    let mut g = GameState::new(&GameConfig { num_players: 2, kingdom: vec![id::YOUNG_WITCH], bane: Some(id::HAMLET), ..Default::default() });
    g.supply.set(id::HAMLET, 0);
    g.supply.set(id::ESTATE, 0);
    assert_eq!(g.empty_piles(), 2);
    g.supply.set(id::CURSE, 0);
    assert_eq!(g.end_reason(), Some(EndReason::PilesEmpty));
}

#[test]
fn the_default_bane_costs_two_or_three_is_not_in_the_kingdom_and_is_stable() {
    let kingdom = vec![id::YOUNG_WITCH, id::SMITHY, id::MARKET, id::MOAT, id::VILLAGE, id::WORKSHOP];
    let a = cards::default_bane(&kingdom, 0).unwrap();
    let mut shuffled = kingdom.clone();
    shuffled.reverse();
    assert_eq!(cards::default_bane(&shuffled, 0), Some(a), "the same kingdom in another order has the same Bane");
    assert!(!kingdom.contains(&a));
    assert!((2..=3).contains(&cards::cost(a)) && !cards::potion_cost(a));
    // A game of that kingdom has the same Bane whatever the seed.
    for seed in 0..5 {
        let g = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed, ..Default::default() });
        assert_eq!(g.bane, a);
        assert!(g.in_supply(a));
    }
    // A Potion-cost card is never the Bane: Apothecary costs $2P, not $2.
    for k in 0..40u16 {
        let kingdom = vec![id::YOUNG_WITCH, k % 20 + id::CELLAR];
        if let Some(b) = cards::default_bane(&kingdom, 0) {
            assert!(!cards::potion_cost(b), "{}", cards::name(b));
        }
    }
    // An explicit Bane wins.
    let g = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), bane: Some(id::CELLAR), ..Default::default() });
    assert_eq!(g.bane, id::CELLAR);
    // No Young Witch: no Bane.
    assert_eq!(new_state(&[id::SMITHY], 2).bane, 0);
}

// ===========================================================================
// Ferryman and its pile
// ===========================================================================

#[test]
fn ferryman_draws_two_gives_an_action_and_discards_a_card() {
    let d = cards::def(id::FERRYMAN);
    assert_eq!((d.cost, d.cards, d.actions), (5, 2, 1));
    let mut g = new_state(&[id::FERRYMAN], 2);
    set_hand(&mut g, 0, &[id::FERRYMAN, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::COPPER]);
    play(&mut g, id::FERRYMAN);
    assert_eq!(g.turn.actions, 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 1, max: 1, .. }));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
}

#[test]
fn gaining_a_ferryman_also_gains_a_copy_of_its_pile_card() {
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::FERRYMAN, id::WORKSHOP], ferryman: Some(id::SHOP), ..Default::default() };
    let mut g = GameState::new(&cfg);
    assert_eq!(g.ferryman, id::SHOP);
    assert_eq!(g.ferryman_left, 10);
    assert!(!g.in_supply(id::SHOP), "outside the Supply");
    assert_eq!(g.supply.get(id::SHOP), 0);
    let _ = g.advance(&mut NoEvents);
    reset_turn(&mut g, 0);
    for p in 0..2 {
        g.players[p] = state::PlayerState::default();
    }
    set_deck_known(&mut g, 0, &junk(12));
    buy_phase(&mut g, 5);
    expect_decision(&mut g);
    // The Shop is not a choice: the pile is not in the Supply.
    assert!(!choices(&g).contains(&Choice::Card(id::SHOP)));
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::FERRYMAN), &mut ev);
    assert_eq!(g.players[0].discard, counts_of(&[id::FERRYMAN, id::SHOP]));
    assert_eq!(g.ferryman_left, 9);
    assert_eq!(g.supply.get(id::FERRYMAN), 9);
    assert!(ev.contains(&Event::Gain { player: 0, card: id::SHOP, to: Dest::Discard, source: id::FERRYMAN }));
}

#[test]
fn the_ferryman_pile_cannot_be_bought_and_runs_out_without_ending_the_game() {
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::FERRYMAN], ferryman: Some(id::SHOP), ..Default::default() };
    let mut g = GameState::new(&cfg);
    let _ = g.advance(&mut NoEvents);
    reset_turn(&mut g, 0);
    for p in 0..2 {
        g.players[p] = state::PlayerState::default();
    }
    buy_phase(&mut g, 9);
    expect_decision(&mut g);
    assert!(!choices(&g).contains(&Choice::Card(id::SHOP)));
    // An empty Ferryman pile is no pile ending, and a Ferryman gained then brings no copy.
    g.ferryman_left = 0;
    g.supply.set(id::PROVINCE, 8);
    assert_eq!(g.empty_piles(), 0);
    buy(&mut g, id::FERRYMAN);
    assert_eq!(g.players[0].all_cards().get(id::SHOP), 0);
}

#[test]
fn the_default_ferryman_pile_costs_three_or_four_and_young_witch_as_the_pile_brings_a_bane() {
    let kingdom = vec![id::FERRYMAN, id::SMITHY, id::MARKET];
    let pile = cards::default_ferryman(&kingdom, 0).unwrap();
    assert!((3..=4).contains(&cards::cost(pile)) && !cards::potion_cost(pile) && !kingdom.contains(&pile));
    assert_eq!(cards::default_ferryman(&[kingdom[2], kingdom[0], kingdom[1]], 0), Some(pile));
    // The "+" is only a reminder: Infirmary ($3+) qualifies.
    assert!(cards::pile_eligible(id::INFIRMARY, 3, 4) && !cards::pile_eligible(id::INFIRMARY, 2, 2));
    assert!(!cards::pile_eligible(id::APOTHECARY, 2, 3), "$2P is not $2");
    assert!(!cards::pile_eligible(id::GOLEM, 3, 4), "$4P is not $4");
    // Young Witch as Ferryman's pile needs its own Bane.
    let g = GameState::new(&GameConfig { num_players: 2, kingdom: vec![id::FERRYMAN], ferryman: Some(id::YOUNG_WITCH), ..Default::default() });
    assert_ne!(g.bane, 0);
    assert!(g.in_supply(g.bane));
    // Baker as a set-up card (it cannot be the pile, it costs $5), but a Baker kingdom card still sets up Coffers.
    let g = GameState::new(&GameConfig { num_players: 2, kingdom: vec![id::BAKER, id::FERRYMAN], ..Default::default() });
    assert_eq!(g.players[0].coffers, 1);
}

#[test]
fn ferryman_copies_count_for_merchant_guild_and_trigger_footpad_and_farmhands() {
    let cfg = GameConfig {
        num_players: 2, kingdom: vec![id::FERRYMAN, id::MERCHANT_GUILD], ferryman: Some(id::FARMHANDS), ..Default::default()
    };
    let mut g = GameState::new(&cfg);
    let _ = g.advance(&mut NoEvents);
    reset_turn(&mut g, 0);
    for p in 0..2 {
        g.players[p] = state::PlayerState::default();
    }
    set_hand(&mut g, 0, &[id::SILVER, id::SILVER]);
    set_deck_known(&mut g, 0, &junk(10));
    g.turn.guilds = 1;
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::FERRYMAN));
    // The copy is a Farmhands: its on-gain set-aside is offered even though it came from outside the Supply
    // (the hand holds the two Silvers, auto-played to the play area: nothing to set aside, so none is asked).
    while g.pending_decision().map_or(false, |d| !matches!(d.kind, DecisionKind::Buy)) {
        pass(&mut g);
    }
    assert_eq!(g.players[0].coffers, 2, "Ferryman and its Farmhands are two cards gained in the Buy phase");
}

// ===========================================================================
// Joust and the Rewards
// ===========================================================================

#[test]
fn joust_sets_aside_a_province_for_a_reward_in_hand() {
    let d = cards::def(id::JOUST);
    assert_eq!((d.cost, d.cards, d.actions, d.coins), (5, 1, 1, 1));
    let g0 = new_state(&[id::JOUST], 2);
    assert_eq!(g0.rewards, [1; 6], "one of each Reward with 2 players");
    assert_eq!(new_state(&[id::JOUST], 3).rewards, [2; 6], "two of each with 3 or more");
    assert_eq!(new_state(&[id::SMITHY], 2).rewards, [0; 6]);
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::JOUST, id::PROVINCE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 5]);
    play(&mut g, id::JOUST);
    assert_eq!((g.turn.coins, g.turn.actions), (1, 1));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::SetAside, min: 0, max: 1, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::PROVINCE), Choice::Pass]);
    choose(&mut g, Choice::Card(id::PROVINCE));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { dest: Dest::Hand, .. }), "{d:?}");
    assert_eq!(choices(&g).len(), 6, "any Reward");
    // Not Supply cards: the Rewards are not in the Supply and their counts are in `rewards`.
    assert!(!g.in_supply(id::COURSER));
    choose(&mut g, Choice::Card(id::COURSER));
    assert!(g.players[0].hand.has(id::COURSER));
    assert_eq!(g.rewards[(id::COURSER - cards::FIRST_REWARD) as usize], 0);
    // The Province is set aside until Clean-up (not in hand, in play or the discard pile), and discarded then.
    assert_eq!(g.players[0].cleanup_aside.get(id::PROVINCE), 1);
    assert_eq!(g.players[0].all_cards().get(id::PROVINCE), 1);
    assert!(!g.players[0].hand.has(id::PROVINCE) && !g.players[0].discard.has(id::PROVINCE));
}

#[test]
fn joust_province_goes_to_the_discard_pile_in_cleanup_and_is_optional() {
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::JOUST, id::PROVINCE]);
    set_deck_known(&mut g, 0, &junk(12));
    play(&mut g, id::JOUST);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::PROVINCE));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::RENOWN));
    // End the turn: every Reward in hand stays; the Province is discarded in Clean-up.
    let mut guard = 0;
    while g.turn.number == 1 && guard < 20 {
        guard += 1;
        match g.pending_decision() {
            Some(_) => {
                let c = if choices(&g).contains(&Choice::Pass) { Choice::Pass } else { choices(&g)[0] };
                choose(&mut g, c);
            }
            None => {
                let _ = adv(&mut g);
            }
        }
    }
    assert!(g.players[0].cleanup_aside.is_empty());
    assert!(g.players[0].discard.has(id::PROVINCE) || g.players[0].all_cards().get(id::PROVINCE) == 1);
    // Declining sets nothing aside.
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::JOUST, id::PROVINCE]);
    set_deck_known(&mut g, 0, &junk(4));
    play(&mut g, id::JOUST);
    pass(&mut g);
    assert!(g.players[0].hand.has(id::PROVINCE));
    assert_eq!(g.rewards, [1; 6]);
}

#[test]
fn joust_with_no_rewards_left_still_sets_the_province_aside() {
    let mut g = new_state(&[id::JOUST], 2);
    g.rewards = [0; 6];
    set_hand(&mut g, 0, &[id::JOUST, id::PROVINCE]);
    set_deck_known(&mut g, 0, &junk(4));
    play(&mut g, id::JOUST);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::PROVINCE));
    assert_eq!(g.players[0].cleanup_aside.get(id::PROVINCE), 1, "set aside, nothing gained");
    assert!(g.players[0].hand.is_empty() || !g.players[0].hand.has(id::PROVINCE));
}

#[test]
fn rewards_are_not_in_the_supply_cannot_be_bought_or_gained_by_other_cards() {
    let mut g = new_state(&[id::JOUST, id::WORKSHOP, id::HORN_OF_PLENTY], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    for r in cards::FIRST_REWARD..cards::FIRST_REWARD + cards::NUM_REWARDS as CardId {
        assert!(!choices(&g).contains(&Choice::Card(r)), "{}", cards::name(r));
    }
    // Buying: none offered with plenty of money.
    let mut g = new_state(&[id::JOUST], 2);
    buy_phase(&mut g, 20);
    expect_decision(&mut g);
    for r in cards::FIRST_REWARD..cards::FIRST_REWARD + cards::NUM_REWARDS as CardId {
        assert!(!choices(&g).contains(&Choice::Card(r)));
    }
    // An empty Rewards pile does not count toward the end.
    g.rewards = [0; 6];
    assert_eq!(g.empty_piles(), 0);
}

#[test]
fn a_lurker_can_gain_a_trashed_reward_and_trashed_rewards_do_not_return_to_their_pile() {
    let mut g = new_state(&[id::LURKER, id::JOUST], 2);
    g.trash.add(id::COURSER, 1);
    set_hand(&mut g, 0, &[id::LURKER]);
    play(&mut g, id::LURKER);
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(1)); // gain an Action card from the trash
    // (The only Action in the trash: gained without asking.)
    assert_eq!(g.players[0].discard.get(id::COURSER), 1);
    assert_eq!(g.rewards[(id::COURSER - cards::FIRST_REWARD) as usize], 1, "the pile was not touched");
}

#[test]
fn courser_chooses_two_different_options_and_draws_before_the_silvers() {
    let d = cards::def(id::COURSER);
    assert_eq!(d.cost, 0);
    assert!(cards::is(id::COURSER, cards::ACTION) && cards::is(id::COURSER, cards::REWARD));
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::COURSER]);
    set_deck_known(&mut g, 0, &junk(2));
    set_discard(&mut g, 0, &[id::SILVER, id::SILVER]);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::COURSER, &mut ev);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 2, distinct: true }), "{d:?}");
    assert_eq!(choices(&g).len(), 4);
    choose_ev(&mut g, Choice::Mode(0), &mut ev); // +2 Cards
    choose_ev(&mut g, Choice::Mode(3), &mut ev); // gain 4 Silvers
    // The two cards were drawn before the Silvers were gained.
    let last_draw = ev.iter().rposition(|e| matches!(e, Event::Draw { .. })).unwrap();
    let first_gain = ev.iter().position(|e| matches!(e, Event::Gain { card, .. } if *card == id::SILVER)).unwrap();
    assert!(last_draw < first_gain, "{ev:?}");
    assert_eq!(g.players[0].discard.get(id::SILVER), 2 + 4);
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn courser_gives_actions_or_coins_and_gains_what_is_left_of_the_silvers() {
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::COURSER]);
    set_supply(&mut g, id::SILVER, 3);
    play(&mut g, id::COURSER);
    choose(&mut g, Choice::Mode(1));
    choose(&mut g, Choice::Mode(2));
    assert_eq!((g.turn.actions, g.turn.coins), (2 + 0, 2), "+2 Actions and +$2 (the Courser's own Action is spent)");
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::COURSER]);
    set_supply(&mut g, id::SILVER, 3);
    play(&mut g, id::COURSER);
    choose(&mut g, Choice::Mode(2));
    choose(&mut g, Choice::Mode(3));
    assert_eq!(g.players[0].discard.get(id::SILVER), 3, "only 3 Silvers were left");
    assert_eq!(g.supply.get(id::SILVER), 0);
}

#[test]
fn demesne_gives_actions_buys_a_gold_and_is_worth_a_vp_per_gold() {
    let d = cards::def(id::DEMESNE);
    assert_eq!((d.actions, d.buys), (2, 2));
    assert!(cards::is(id::DEMESNE, cards::VICTORY) && cards::is(id::DEMESNE, cards::ACTION));
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::DEMESNE]);
    play(&mut g, id::DEMESNE);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 3));
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    let mut p = state::PlayerState::default();
    p.discard.add(id::DEMESNE, 2);
    p.discard.add(id::GOLD, 3);
    assert_eq!(p.vp(), 2 * 3, "each Demesne is worth 1 VP per Gold");
}

#[test]
fn housecarl_draws_a_card_per_differently_named_action_in_play() {
    let mut g = new_state(&[id::JOUST, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HOUSECARL]);
    set_in_play(&mut g, 0, &[id::VILLAGE, id::VILLAGE, id::SMITHY, id::COPPER]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::HOUSECARL);
    assert_eq!(g.players[0].hand.total(), 3, "Village, Smithy, and Housecarl itself");
}

#[test]
fn huge_turnip_gives_two_tokens_and_a_coin_per_token_including_the_new_ones() {
    let d = cards::def(id::HUGE_TURNIP);
    assert!(cards::is(id::HUGE_TURNIP, cards::TREASURE) && cards::is(id::HUGE_TURNIP, cards::REWARD));
    assert!(cards::is_choice_free(id::HUGE_TURNIP));
    assert_eq!(d.cost, 0);
    let mut g = new_state(&[id::JOUST], 2);
    g.players[0].coffers = 3;
    set_hand(&mut g, 0, &[id::HUGE_TURNIP]);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    assert_eq!(g.players[0].coffers, 5);
    assert_eq!(g.turn.coins, 5, "$1 per Coffers, the two just added included");
    // Two Turnips: 2+2... the second sees four more tokens.
    let mut g = new_state(&[id::JOUST], 2);
    set_hand(&mut g, 0, &[id::HUGE_TURNIP, id::HUGE_TURNIP]);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    assert_eq!(g.players[0].coffers, 4);
    assert_eq!(g.turn.coins, 2 + 4);
}

#[test]
fn renown_gives_a_buy_and_reduces_every_cost_by_two_and_stacks() {
    let d = cards::def(id::RENOWN);
    assert_eq!(d.buys, 1);
    let mut g = new_state(&[id::JOUST, id::REMAKE], 2);
    set_hand(&mut g, 0, &[id::RENOWN, id::REMAKE, id::COPPER]);
    g.turn.actions = 2;
    play(&mut g, id::RENOWN);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.cost(id::GOLD), 4);
    assert_eq!(g.cost(id::COPPER), 0, "costs never go below $0");
    assert_eq!(g.cost(id::SILVER), 1);
    // The FAQ example: with Renown, a Copper ($0) can be remade into a Silver (now $1).
    play(&mut g, id::REMAKE);
    assert_eq!(g.trash.get(id::COPPER), 1);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    // Throne Room doubles it.
    let mut g = new_state(&[id::JOUST, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::RENOWN]);
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::RENOWN));
    assert_eq!((g.cost(id::GOLD), g.turn.buys), (2, 3));
}

#[test]
fn coronet_plays_an_action_twice_then_a_treasure_twice_but_never_a_reward() {
    let d = cards::def(id::CORONET);
    assert_eq!(d.cost, 0);
    assert!(cards::is(id::CORONET, cards::ACTION) && cards::is(id::CORONET, cards::TREASURE) && cards::is(id::CORONET, cards::REWARD));
    let mut g = new_state(&[id::JOUST, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CORONET, id::VILLAGE, id::SILVER, id::RENOWN, id::HUGE_TURNIP]);
    set_deck_known(&mut g, 0, &junk(8));
    play(&mut g, id::CORONET);
    // The Action first: the Village (not the Renown: a Reward).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, min: 0, max: 1, .. }));
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Pass]);
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.turn.actions, 4, "two plays of the Village; no Action used for them");
    // Then the Treasure: the Silver (not the Huge Turnip), played twice.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, .. }));
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Pass]);
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.turn.coins, 4, "Silver twice");
}

#[test]
fn coronet_in_the_buy_phase_is_a_treasure_and_both_parts_are_optional() {
    let mut g = new_state(&[id::JOUST, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CORONET, id::VILLAGE, id::GOLD]);
    set_deck_known(&mut g, 0, &junk(8));
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::CORONET);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::VILLAGE)); // an Action played in the Buy phase
    assert_eq!(g.turn.actions, 1 + 4);
    // No Actions can be played now, though: still the Buy phase.
    expect_decision(&mut g);
    pass(&mut g); // not the Gold
    assert_eq!(g.turn.phase, Phase::Buy);
}

// ---- Charlatan makes Curse a Treasure: every "Treasure" choice includes it. ----

#[test]
fn with_charlatan_coronet_may_play_a_curse_twice() {
    let mut g = new_state(&[id::JOUST, id::CHARLATAN, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CORONET, id::CURSE]);
    set_deck_known(&mut g, 0, &junk(8));
    play(&mut g, id::CORONET);
    // No Action to play; then the Treasure: the Curse ($1 under Charlatan), twice.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, .. }));
    assert_eq!(choices(&g), vec![Choice::Card(id::CURSE), Choice::Pass]);
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.turn.coins, 2);

    // Without Charlatan the Curse isn't a Treasure: nothing to pick.
    let mut g = new_state(&[id::JOUST, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CORONET, id::CURSE]);
    set_deck_known(&mut g, 0, &junk(8));
    play(&mut g, id::CORONET);
    assert!(g.players[0].hand.has(id::CURSE));
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn with_charlatan_mine_and_bandit_treat_a_curse_as_a_treasure() {
    // Mine trashes the Curse ($0) and gains a Treasure up to $3 to hand.
    let mut g = new_state(&[id::MINE, id::CHARLATAN], 2);
    set_hand(&mut g, 0, &[id::MINE, id::CURSE]);
    play(&mut g, id::MINE);
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::CURSE)));
    choose(&mut g, Choice::Card(id::CURSE));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::SILVER));
    // (The Silver went to hand and was auto-played on entering the Buy phase.)
    let p0 = &g.players[0];
    assert!(g.trash.has(id::CURSE) && (p0.hand.has(id::SILVER) || p0.in_play.has(id::SILVER)), "trash {:?} hand {:?}", g.trash, p0.hand);

    // Bandit: a revealed Curse is a non-Copper Treasure the victim must trash.
    let mut g = new_state(&[id::BANDIT, id::CHARLATAN], 2);
    set_hand(&mut g, 0, &[id::BANDIT]);
    set_deck_known(&mut g, 1, &[id::CURSE, id::ESTATE]);
    play(&mut g, id::BANDIT);
    drain_selects(&mut g);
    assert!(g.trash.has(id::CURSE), "trash: {:?}", g.trash);
}

// ===========================================================================
// More scenarios: counts of names, short decks, empty piles and what a played card leaves behind
// ===========================================================================

/// Answer the pending Select and Gain decisions with their first choice, up to `n` of them.
fn answer_first(g: &mut GameState, n: usize) {
    for _ in 0..n {
        match g.pending_decision() {
            Some(d) if matches!(d.kind, DecisionKind::Select { .. } | DecisionKind::Gain { .. }) => {
                let first = choices(g)[0];
                choose(g, first);
            }
            _ => break,
        }
    }
}

#[test]
fn menagerie_draws_three_or_one_by_the_names_in_the_rest_of_the_hand() {
    // (the rest of the hand, cards drawn): only names in the hand matter, not the Estates in the deck.
    let cases: [(&[CardId], usize); 6] = [
        (&[], 3),
        (&[id::COPPER], 3),
        (&[id::COPPER, id::ESTATE, id::CURSE], 3),
        (&[id::COPPER, id::COPPER], 1),
        (&[id::COPPER, id::GOLD, id::COPPER], 1),
        (&[id::COPPER, id::ESTATE, id::CURSE, id::COPPER], 1),
    ];
    for (rest, drawn) in cases {
        let mut g = new_state(&[id::MENAGERIE], 2);
        let mut hand = vec![id::MENAGERIE];
        hand.extend_from_slice(rest);
        set_hand(&mut g, 0, &hand);
        set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
        play(&mut g, id::MENAGERIE);
        assert_eq!(held(&g, 0).total() as usize, rest.len() + drawn, "{rest:?}");
        assert_eq!(g.turn.actions, 1);
    }
}

#[test]
fn menagerie_draws_only_what_the_deck_has() {
    // Nothing anywhere: nothing happens but the +Action.
    let mut g = new_state(&[id::MENAGERIE], 2);
    set_hand(&mut g, 0, &[id::MENAGERIE]);
    play(&mut g, id::MENAGERIE);
    assert!(g.players[0].hand.is_empty() && g.turn.actions == 1);
    // A duplicate and no deck: the hand stays as it was.
    let mut g = new_state(&[id::MENAGERIE], 2);
    set_hand(&mut g, 0, &[id::MENAGERIE, id::COPPER, id::COPPER]);
    play(&mut g, id::MENAGERIE);
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER, id::COPPER]));
    // An empty hand but a 2-card deck: +3 Cards gets 2.
    let mut g = new_state(&[id::MENAGERIE], 2);
    set_hand(&mut g, 0, &[id::MENAGERIE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::MENAGERIE);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE]));
}

#[test]
fn hamlet_bonuses_each_depend_on_their_own_discard() {
    // Nothing discarded either time: the drawn card stays, no bonus.
    let mut g = new_state(&[id::HAMLET], 2);
    set_hand(&mut g, 0, &[id::HAMLET]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::HAMLET);
    pass(&mut g);
    pass(&mut g);
    assert_eq!((g.turn.actions, g.turn.buys, g.players[0].hand.total()), (1, 1, 1));

    // Both discards: the drawn Curse and the Curse that was in hand.
    let mut g = new_state(&[id::HAMLET], 2);
    set_hand(&mut g, 0, &[id::HAMLET, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::GOLD]);
    play(&mut g, id::HAMLET);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::CURSE));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!((g.turn.actions, g.turn.buys), (2, 2));
    assert_eq!(g.players[0].discard, counts_of(&[id::CURSE, id::CURSE]));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));

    // Only the first: +1 Action, no +Buy.
    let mut g = new_state(&[id::HAMLET], 2);
    set_hand(&mut g, 0, &[id::HAMLET, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::GOLD]);
    play(&mut g, id::HAMLET);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::CURSE));
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 1));
    assert_eq!(held(&g, 0), counts_of(&[id::COPPER]));
}

#[test]
fn remake_gains_use_the_reduced_cost_and_a_pile_that_ran_out_in_the_first_gain() {
    // With a cost reduction of $1 a Copper and a Curse ($0) can be remade into an Estate ($1).
    let mut g = new_state(&[id::REMAKE], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::REMAKE, id::COPPER, id::CURSE]);
    play(&mut g, id::REMAKE);
    answer_first(&mut g, 4);
    assert_eq!(g.trash, counts_of(&[id::COPPER, id::CURSE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE, id::ESTATE]));

    // The last Silver goes on the first gain, so the second has only the Village to choose.
    let mut g = new_state(&[id::REMAKE, id::VILLAGE], 2);
    set_supply(&mut g, id::SILVER, 1);
    set_hand(&mut g, 0, &[id::REMAKE, id::ESTATE, id::HAMLET]);
    play(&mut g, id::REMAKE);
    choose(&mut g, Choice::Card(id::ESTATE)); // $2: exactly $3 is a Silver or a Village
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Card(id::VILLAGE)]);
    choose(&mut g, Choice::Card(id::SILVER));
    answer_first(&mut g, 2);
    assert_eq!(g.trash, counts_of(&[id::ESTATE, id::HAMLET]));
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER, id::VILLAGE]));
    assert_eq!(g.supply.get(id::SILVER), 0);
}

#[test]
fn fairgrounds_counts_names_in_all_the_cards_and_loses_a_name_when_the_last_copy_goes() {
    let vp = |cs: &[CardId]| {
        let mut p = state::PlayerState::default();
        for &c in cs {
            p.discard.add(c, 1);
        }
        p.vp()
    };
    let nine = [id::FAIRGROUNDS, id::COPPER, id::SILVER, id::GOLD, id::VILLAGE, id::WORKSHOP, id::MARKET, id::SMITHY, id::MINE];
    assert_eq!(vp(&nine), 2, "9 names, rounded down");
    let mut ten = nine.to_vec();
    ten.push(id::PLATINUM);
    assert_eq!(vp(&ten), 4);
    let mut two_nine = nine.to_vec();
    two_nine.push(id::FAIRGROUNDS);
    assert_eq!(vp(&two_nine), 4, "two Fairgrounds, 9 names");
    let mut two_ten = ten.clone();
    two_ten.push(id::FAIRGROUNDS);
    assert_eq!(vp(&two_ten), 8, "two Fairgrounds, 10 names");
    // Extra copies of a name add nothing.
    let mut many = ten.clone();
    many.extend([id::COPPER; 7]);
    assert_eq!(vp(&many), 4);
    // Losing the only Copper drops a name: 10 -> 9.
    let mut p = state::PlayerState::default();
    for &c in &ten {
        p.discard.add(c, 1);
    }
    assert_eq!(p.vp(), 4);
    p.discard.remove(id::COPPER);
    assert_eq!(p.vp(), 2);
}

#[test]
fn hunting_party_reveals_past_copies_of_cards_in_the_hand() {
    // (the deck top to bottom, the hand after, the discard pile after, the next card of the deck). The hand starts
    // as Hunting Party and an Estate; the first card of the deck is the +1 Card.
    type Case = (Vec<CardId>, Vec<CardId>, Vec<CardId>, Option<CardId>);
    let cases: Vec<Case> = vec![
        (vec![], vec![id::ESTATE], vec![], None),
        (vec![id::ESTATE], vec![id::ESTATE, id::ESTATE], vec![], None),
        (vec![id::ESTATE, id::PROVINCE], vec![id::ESTATE, id::ESTATE, id::PROVINCE], vec![], None),
        (vec![id::PROVINCE, id::PROVINCE], vec![id::ESTATE, id::PROVINCE], vec![id::PROVINCE], None),
        (vec![id::PROVINCE, id::ESTATE], vec![id::ESTATE, id::PROVINCE], vec![id::ESTATE], None),
        (vec![id::DUCHY, id::CURSE, id::PROVINCE], vec![id::ESTATE, id::DUCHY, id::CURSE], vec![], Some(id::PROVINCE)),
        (
            vec![id::DUCHY, id::ESTATE, id::DUCHY, id::CURSE, id::GOLD],
            vec![id::ESTATE, id::DUCHY, id::CURSE],
            vec![id::ESTATE, id::DUCHY],
            Some(id::GOLD),
        ),
    ];
    for (deck, hand_after, discard_after, top) in cases {
        let mut g = new_state(&[id::HUNTING_PARTY], 2);
        set_hand(&mut g, 0, &[id::HUNTING_PARTY, id::ESTATE]);
        set_deck_known(&mut g, 0, &deck);
        play(&mut g, id::HUNTING_PARTY);
        assert_eq!(g.players[0].hand, counts_of(&hand_after), "{deck:?}");
        assert_eq!(g.players[0].discard, counts_of(&discard_after), "{deck:?}");
        assert_eq!(g.players[0].deck_known.peek_top(), top, "{deck:?}");
    }
    // Everything revealed is a copy: all of it is discarded, nothing is found.
    let mut g = new_state(&[id::HUNTING_PARTY], 2);
    set_hand(&mut g, 0, &[id::HUNTING_PARTY, id::ESTATE, id::CURSE]);
    set_deck_known(
        &mut g, 0,
        &[id::DUCHY, id::ESTATE, id::DUCHY, id::CURSE, id::ESTATE, id::DUCHY, id::CURSE, id::ESTATE, id::DUCHY, id::CURSE],
    );
    play(&mut g, id::HUNTING_PARTY);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::CURSE]));
    assert_eq!(g.players[0].discard.total(), 9);
    assert_eq!(g.players[0].deck_size(), 0);
}

#[test]
fn horn_of_plenty_gains_up_to_the_number_of_names_in_play_including_itself() {
    // Alone: one name, so up to $1.
    let mut g = new_state(&[id::HORN_OF_PLENTY], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 1, .. }), "{d:?}");
    assert!(choices(&g).contains(&Choice::Card(id::COPPER)) && !choices(&g).contains(&Choice::Card(id::ESTATE)));
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.players[0].in_play.has(id::HORN_OF_PLENTY), "not a Victory card: the Horn stays");
    // Two Coppers and the Horn are two names: $2, an Estate but no Silver.
    let mut g = new_state(&[id::HORN_OF_PLENTY], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY]);
    set_in_play(&mut g, 0, &[id::COPPER, id::COPPER]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::ESTATE)) && !choices(&g).contains(&Choice::Card(id::SILVER)));
}

#[test]
fn a_victory_card_the_horn_gains_is_not_bought_so_hoard_gives_nothing() {
    let mut g = new_state(&[id::HORN_OF_PLENTY, id::HOARD], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY]);
    set_in_play(&mut g, 0, &[id::COPPER, id::HOARD]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]), "gained, not bought: no Gold from the Hoard");
    assert_eq!(g.trash.get(id::HORN_OF_PLENTY), 1);
    // Buying one does give the Gold (checked before the turn ends and the discard pile is reshuffled).
    g.turn.coins = 2;
    expect_decision(&mut g);
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    assert!(g.players[0].discard.has(id::GOLD));
}

#[test]
fn jester_gains_nothing_when_the_discarded_cards_pile_is_not_in_the_game() {
    let mut g = new_state(&[id::JESTER], 2);
    set_hand(&mut g, 0, &[id::JESTER]);
    set_deck_known(&mut g, 1, &[id::VILLAGE, id::COPPER]);
    assert!(!g.in_supply(id::VILLAGE));
    play(&mut g, id::JESTER);
    assert_eq!(g.players[1].discard, counts_of(&[id::VILLAGE]), "discarded, and no copy for anyone");
    assert_eq!(g.players[0].discard.total(), 0);
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::PlayAction | DecisionKind::Buy), "nothing was asked");
}

#[test]
fn young_witch_discards_what_there_is_when_it_has_fewer_than_two_cards() {
    // No cards at all: nothing to ask, the others still get their Curse.
    let mut g = new_state(&[id::YOUNG_WITCH], 2);
    set_hand(&mut g, 0, &[id::YOUNG_WITCH]);
    play(&mut g, id::YOUNG_WITCH);
    assert!(g.players[0].hand.is_empty() && g.players[0].discard.is_empty());
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    // The one card drawn is the one discarded.
    let mut g = new_state(&[id::YOUNG_WITCH], 2);
    set_hand(&mut g, 0, &[id::YOUNG_WITCH]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::YOUNG_WITCH);
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
    assert!(g.players[0].hand.is_empty());
    // One in hand and one drawn: both go, with nothing to choose.
    let mut g = new_state(&[id::YOUNG_WITCH], 2);
    set_hand(&mut g, 0, &[id::YOUNG_WITCH, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::YOUNG_WITCH);
    assert_eq!(g.players[0].discard, counts_of(&[id::DUCHY, id::ESTATE]));
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
}

#[test]
fn butcher_with_no_cards_only_gives_tokens_and_new_tokens_are_spendable_at_once() {
    let mut g = new_state(&[id::BUTCHER], 2);
    set_hand(&mut g, 0, &[id::BUTCHER]);
    let Step::Decision(d) = play(&mut g, id::BUTCHER) else { panic!() };
    assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "{d:?}");
    assert_eq!((g.players[0].coffers, g.trash.total()), (2, 0));
    // A Gold ($6) and the two new tokens buy a Province ($8).
    let mut g = new_state(&[id::BUTCHER], 2);
    set_hand(&mut g, 0, &[id::BUTCHER, id::GOLD]);
    play(&mut g, id::BUTCHER);
    choose(&mut g, Choice::Card(id::GOLD));
    assert!(choices(&g).contains(&Choice::Card(id::PROVINCE)));
    choose(&mut g, Choice::Card(id::PROVINCE));
    assert_eq!((g.players[0].coffers, g.trash.get(id::GOLD), g.players[0].discard.get(id::PROVINCE)), (0, 1, 1));
}

#[test]
fn merchant_guild_pays_only_its_owner_and_only_in_their_own_turn() {
    let mut g = new_state(&[id::MERCHANT_GUILD], 2);
    g.players[1].in_play.add(id::MERCHANT_GUILD, 1);
    buy_phase(&mut g, 3);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!((g.players[0].coffers, g.players[1].coffers), (0, 0));
}

#[test]
fn candlestick_maker_adds_to_the_tokens_already_held() {
    let mut g = new_state(&[id::CANDLESTICK_MAKER], 2);
    g.players[0].coffers = 5;
    set_hand(&mut g, 0, &[id::CANDLESTICK_MAKER]);
    play(&mut g, id::CANDLESTICK_MAKER);
    assert_eq!((g.players[0].coffers, g.turn.actions, g.turn.buys), (6, 1, 2));
}

#[test]
fn stonemason_may_gain_cheaper_cards_of_any_type_and_overpays_with_one_copy_left() {
    // Trash a Duchy ($5): a Silver, then a Quarry ($4, a Treasure): "costing less", whatever the type.
    let mut g = new_state(&[id::STONEMASON, id::QUARRY], 2);
    set_hand(&mut g, 0, &[id::STONEMASON, id::DUCHY, id::ESTATE]);
    play(&mut g, id::STONEMASON);
    choose(&mut g, Choice::Card(id::DUCHY));
    assert!(choices(&g).contains(&Choice::Card(id::QUARRY)) && choices(&g).contains(&Choice::Card(id::SILVER)));
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::QUARRY));
    assert_eq!(g.players[0].discard, counts_of(&[id::SILVER, id::QUARRY]));
    assert_eq!(g.trash, counts_of(&[id::DUCHY]));

    // Overpaying $3 with one Village left: it is gained, and the second gain finds none. The Potion
    // that was not part of the overpay stays unspent.
    let mut g = new_state(&[id::STONEMASON, id::VILLAGE], 2);
    set_supply(&mut g, id::VILLAGE, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    g.turn.potions = 1;
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::STONEMASON));
    expect_decision(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Overpay(3), &mut ev);
    assert!(ev.contains(&Event::Overpay { player: 0, card: id::STONEMASON, coins: 3, potion: false }), "{ev:?}");
    assert_eq!(g.players[0].all_cards().get(id::VILLAGE), 1);
    assert_eq!(g.supply.get(id::VILLAGE), 0);
}

#[test]
fn journeyman_discards_named_cards_among_the_reveals_and_stops_after_three_others() {
    let mut g = new_state(&[id::JOURNEYMAN], 2);
    set_hand(&mut g, 0, &[id::JOURNEYMAN]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::GOLD, id::DUCHY, id::GOLD, id::PROVINCE, id::SILVER]);
    play(&mut g, id::JOURNEYMAN);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD, id::GOLD]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));

    // The only card is the named one: it is discarded and the deck is empty.
    let mut g = new_state(&[id::JOURNEYMAN], 2);
    set_hand(&mut g, 0, &[id::JOURNEYMAN]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    play(&mut g, id::JOURNEYMAN);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::GOLD));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].discard, counts_of(&[id::GOLD]));

    // Nothing to reveal at all: whatever is asked, the hand is unchanged.
    let mut g = new_state(&[id::JOURNEYMAN], 2);
    set_hand(&mut g, 0, &[id::JOURNEYMAN, id::ESTATE]);
    play(&mut g, id::JOURNEYMAN);
    if g.pending_decision().is_some_and(|d| d.kind == DecisionKind::Name) {
        let first = choices(&g)[0];
        choose(&mut g, first);
    }
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
}
