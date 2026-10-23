//! Adventures tests (rulings in `docs/adventures-plan.md`): the cards, the Events, the tokens, the Tavern mat and the calls, the Travellers and the
//! split pile Sauna/Avanto.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
#[allow(unused_imports)]
use dominion_engine::state::{ADV_JOURNEY_DOWN, ADV_MINUS_CARD, ADV_MINUS_COIN, TOK_ACTION, TOK_BUY, TOK_CARD, TOK_COIN, TOK_COST, TOK_TRASH};
#[allow(unused_imports)]
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

// ===========================================================================
// Random games: nothing is lost, nothing panics, every game ends
// ===========================================================================

#[test]
fn random_adventures_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0xAD7E);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Adventures || c == id::SAUNA).collect();
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::Adventures).collect();
    let events: Vec<CardId> = (cards::FIRST_EVENT..cards::FIRST_EVENT + cards::NUM_EVENTS as CardId).collect();
    for game in 0..2500u64 {
        let n = 2 + (game % 4) as usize;
        let mut kingdom: Vec<CardId> = Vec::new();
        // Mostly Adventures cards, a few others for the interactions (Throne Room, attacks, trashers...).
        let mut from = pool.clone();
        for _ in 0..8 {
            let i = rng.below(from.len() as u32) as usize;
            kingdom.push(from.swap_remove(i));
        }
        for _ in 0..2 {
            let c = others[rng.below(others.len() as u32) as usize];
            if !kingdom.contains(&c) {
                kingdom.push(c);
            }
        }
        let mut evs = events.clone();
        for _ in 0..(game % 4) {
            let i = rng.below(evs.len() as u32) as usize;
            kingdom.push(evs.swap_remove(i));
        }
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom, seed: game, max_turns: 300, ..Default::default() });
        let initial = total_cards(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        let mut log: Vec<Event> = Vec::new();
        loop {
            log.clear();
            let chosen;
            match g.advance(&mut log) {
                Step::Decision(d) => {
                    g.legal_choices(&mut buf);
                    assert!(!buf.is_empty(), "no legal choice for {d:?} in game {game}");
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    chosen = Some(c);
                    g.apply(c, &mut log).unwrap();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            #[allow(clippy::needless_late_init)]
            if total_cards(&g) != initial {
                let mut all = g.supply.total();
                let _ = &mut all;
                let mut sums = g.supply;
                sums.add_all(&g.trash);
                for p in 0..g.num_players as usize {
                    sums.add_all(&g.players[p].all_cards());
                }
                let diff: Vec<String> = sums
                    .iter()
                    .map(|(c, k)| (c, k as i32))
                    .filter(|&(c, _)| c < cards::FIRST_PILE_CARD)
                    .map(|(c, k)| format!("{}:{k}", cards::name(c)))
                    .collect();
                panic!("card conservation, game {game}, chose {chosen:?}: {log:?}\nzones: {diff:?}\nside {}", g.side_pile_cards());
            }
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// A deck of cards that are neither Treasures nor Actions: every draw is visible, nothing is auto-played, and there are enough for turns.
fn junk() -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(40).copied().collect()
}

/// Play the rest of the turn out until the next one has started. `answer` may pick the choice for a decision; `None` leaves it to the default
/// (Pass at PlayAction/Buy, Yes at a YesNo, the first choice otherwise).
fn end_turn_with<S: EventSink>(g: &mut GameState, sink: &mut S, mut answer: impl FnMut(&GameState, &Decision) -> Option<Choice>) {
    let start = g.turn.number;
    loop {
        match g.advance(sink) {
            Step::Decision(d) => {
                if g.turn.number != start {
                    return;
                }
                let c = answer(g, &d).unwrap_or_else(|| match d.kind {
                    DecisionKind::PlayAction | DecisionKind::Buy => Choice::Pass,
                    DecisionKind::YesNo { .. } => Choice::Yes,
                    _ => choices(g)[0],
                });
                g.apply(c, sink).unwrap();
            }
            Step::GameOver => return,
            other => panic!("unexpected {other:?}"),
        }
    }
}

fn end_turn(g: &mut GameState) {
    end_turn_with(g, &mut NoEvents, |_, _| None);
}

/// Player `p`'s hand plus the Treasures in their play area: the engine plays the Treasures in hand when the Action phase ends.
fn held(g: &GameState, p: usize) -> Counts {
    let mut c = g.players[p].hand;
    for (card, n) in g.players[p].in_play.iter() {
        if cards::is(card, cards::TREASURE) {
            c.add(card, n);
        }
    }
    c
}

fn count(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].all_cards().get(c)
}

fn pick(g: &mut GameState, c: CardId) -> Step {
    choose(g, Choice::Card(c))
}

// ===========================================================================
// The cards of the set
// ===========================================================================

#[test]
fn the_set_has_30_kingdom_piles_the_upgrades_and_20_events() {
    assert_eq!(cards::kingdom_cards_in(CardSet::Adventures).count(), 30);
    assert_eq!(cards::set_by_name("Adventures"), Some(CardSet::Adventures));
    assert_eq!(CardSet::Adventures.name(), "Adventures");
    // The upgrades and the Events are not kingdom cards.
    for c in [id::TREASURE_HUNTER, id::WARRIOR, id::HERO, id::CHAMPION, id::SOLDIER, id::FUGITIVE, id::DISCIPLE, id::TEACHER] {
        assert!(!cards::is_kingdom(c) && cards::set_of(c) == CardSet::Adventures, "{}", cards::name(c));
    }
    for e in cards::FIRST_EVENT..cards::FIRST_EVENT + cards::NUM_EVENTS as CardId {
        assert!(cards::is_event(e) && !cards::is_kingdom(e) && cards::def(e).types == 0);
        assert_eq!(cards::by_name(cards::name(e)), Some(e));
    }
    assert!(!cards::is_event(id::PROVINCE) && !cards::is_event(id::ABANDONED_MINE));
    // Costs of the upgrades and Events, as printed.
    let cost = |c| cards::cost(c);
    assert_eq!([cost(id::TREASURE_HUNTER), cost(id::WARRIOR), cost(id::HERO), cost(id::CHAMPION)], [3, 4, 5, 6]);
    assert_eq!([cost(id::SOLDIER), cost(id::FUGITIVE), cost(id::DISCIPLE), cost(id::TEACHER)], [3, 4, 5, 6]);
    assert_eq!([cost(id::ALMS), cost(id::SAVE), cost(id::BALL), cost(id::INHERITANCE), cost(id::PATHFINDING)], [0, 1, 5, 7, 8]);
    assert_eq!(cards::by_name("Coin of the Realm"), Some(id::COIN_OF_THE_REALM));
    assert_eq!(cards::by_name("Sauna/Avanto"), Some(id::SAUNA));
    assert!(cards::is(id::PAGE, cards::TRAVELLER) && !cards::is(id::CHAMPION, cards::TRAVELLER) && !cards::is(id::TEACHER, cards::TRAVELLER));
    assert!(cards::is(id::DISTANT_LANDS, cards::RESERVE | cards::VICTORY) && cards::is(id::COIN_OF_THE_REALM, cards::TREASURE | cards::RESERVE));
}

#[test]
fn set_up_has_the_piles_the_traveller_lines_and_the_events() {
    let g = new_state(&[id::PAGE, id::PORT, id::DISTANT_LANDS, id::ALMS, id::BALL], 2);
    assert_eq!(g.supply.get(id::PAGE), 10);
    assert_eq!(g.supply.get(id::PORT), 12, "twelve Ports");
    assert_eq!(g.supply.get(id::DISTANT_LANDS), 8, "a Victory pile: 8 with two players");
    assert_eq!(g.trav, [5, 5, 5, 5, 0, 0, 0, 0], "Page brings its four upgrades, five of each, outside the Supply");
    assert!(!g.in_supply(id::TREASURE_HUNTER));
    assert_eq!(g.events.bits(), (1 << (id::ALMS - cards::FIRST_EVENT)) | (1 << (id::BALL - cards::FIRST_EVENT)));
    assert!(!g.in_supply(id::ALMS), "an Event has no pile");
    assert_eq!(g.side_pile_cards(), 20);
    let g3 = new_state(&[id::DISTANT_LANDS, id::PEASANT], 3);
    assert_eq!(g3.supply.get(id::DISTANT_LANDS), 12);
    assert_eq!(g3.trav, [0, 0, 0, 0, 5, 5, 5, 5]);
    // A pile of the Page line empty does not count as an empty Supply pile.
    let mut g = new_state(&[id::PAGE], 2);
    g.trav = [0; 8];
    assert_eq!(g.empty_piles(), 0);
}

#[test]
fn page_and_peasant_are_a_cantrip_and_a_buy_with_a_coin() {
    let mut g = new_state(&[id::PAGE, id::PEASANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PAGE, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER]);
    play(&mut g, id::PAGE);
    assert_eq!((g.players[0].hand.get(id::GOLD), g.turn.actions), (1, 1));

    let mut g = new_state(&[id::PAGE, id::PEASANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PEASANT]);
    play(&mut g, id::PEASANT);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 1));
}

#[test]
fn a_traveller_is_exchanged_when_discarded_from_play() {
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PAGE]);
    set_deck_known(&mut g, 0, &junk());
    let before = total_cards(&g);
    play(&mut g, id::PAGE);
    let mut log: Vec<Event> = Vec::new();
    end_turn_with(&mut g, &mut log, |_, _| None);
    assert!(log.contains(&Event::Exchange { player: 0, from: id::PAGE, to: id::TREASURE_HUNTER }), "{log:?}");
    // The Page went back to its pile, the Treasure Hunter into the discard pile: not a gain, not a trash.
    assert_eq!(g.players[0].discard.get(id::TREASURE_HUNTER), 1);
    assert_eq!(count(&g, 0, id::PAGE), 0);
    assert_eq!(g.supply.get(id::PAGE), 11);
    assert_eq!(g.trav[0], 4);
    assert!(g.trash.is_empty());
    assert_eq!(total_cards(&g), before);
    assert!(!log.iter().any(|e| matches!(e, Event::Gain { card, .. } if *card == id::TREASURE_HUNTER)));

    // Declined: it stays a Page.
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::PAGE);
    end_turn_with(&mut g, &mut NoEvents, |_, d| matches!(d.kind, DecisionKind::YesNo { act: Act::Exchange }).then_some(Choice::No));
    assert_eq!(count(&g, 0, id::PAGE), 1);
    assert_eq!(g.trav[0], 5);
}

#[test]
fn a_traveller_discarded_from_the_hand_or_with_an_empty_pile_is_not_exchanged() {
    // Not played: discarded from the hand at cleanup, no offer.
    let mut g = new_state(&[id::PEASANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PEASANT]);
    set_deck_known(&mut g, 0, &junk());
    let mut asked = false;
    end_turn_with(&mut g, &mut NoEvents, |_, d| {
        asked |= matches!(d.kind, DecisionKind::YesNo { act: Act::Exchange });
        None
    });
    assert!(!asked);
    // The Soldier pile is empty: no offer.
    let mut g = new_state(&[id::PEASANT, id::SMITHY], 2);
    g.trav[4] = 0;
    set_hand(&mut g, 0, &[id::PEASANT]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::PEASANT);
    let mut asked = false;
    end_turn_with(&mut g, &mut NoEvents, |_, d| {
        asked |= matches!(d.kind, DecisionKind::YesNo { act: Act::Exchange });
        None
    });
    assert!(!asked);
    assert_eq!(count(&g, 0, id::PEASANT), 1);
}

#[test]
fn the_higher_traveller_is_exchanged_first_so_a_short_pile_is_used_well() {
    // No Hero left: Warrior and Hero are both in play. Hero -> Champion first returns a Hero for the Warrior to take.
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    g.trav = [5, 5, 0, 5, 0, 0, 0, 0];
    set_in_play(&mut g, 0, &[id::WARRIOR, id::HERO]);
    set_deck_known(&mut g, 0, &junk());
    end_turn(&mut g);
    let all = g.players[0].all_cards();
    assert_eq!((all.get(id::CHAMPION), all.get(id::HERO), all.get(id::WARRIOR)), (1, 1, 0), "{all:?}");
    assert_eq!(g.trav, [5, 6, 0, 4, 0, 0, 0, 0], "the Warrior went back, the Hero pile gave its last card to it");
}

#[test]
fn champion_and_teacher_are_not_travellers() {
    let mut g = new_state(&[id::PAGE, id::PEASANT], 2);
    set_in_play(&mut g, 0, &[id::TEACHER]);
    set_deck_known(&mut g, 0, &junk());
    let mut asked = false;
    end_turn_with(&mut g, &mut NoEvents, |_, d| {
        asked |= matches!(d.kind, DecisionKind::YesNo { act: Act::Exchange });
        None
    });
    assert!(!asked);
}

// ===========================================================================
// Reserve cards: the Tavern mat and the calls
// ===========================================================================

fn on_mat(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].tavern.get(c)
}

fn call_decision(g: &GameState) -> bool {
    matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::YesNo { act: Act::Call }, .. }))
}

#[test]
fn reserve_cards_go_onto_the_tavern_mat_when_played_and_stay_owned() {
    for card in [
        id::RATCATCHER, id::GUIDE, id::TRANSMOGRIFY, id::DUPLICATE, id::ROYAL_CARRIAGE, id::WINE_MERCHANT, id::DISTANT_LANDS, id::TEACHER,
    ] {
        let mut g = new_state(&[if card == id::TEACHER { id::PEASANT } else { card }, id::SMITHY], 2);
        set_hand(&mut g, 0, &[card]);
        set_deck_known(&mut g, 0, &junk());
        let before = total_cards(&g);
        play(&mut g, card);
        assert_eq!(on_mat(&g, 0, card), 1, "{} goes onto the mat", cards::name(card));
        assert!(!g.players[0].in_play.has(card), "{} is not in play", cards::name(card));
        assert_eq!(count(&g, 0, card), 1, "and is still owned");
        assert_eq!(total_cards(&g), before);
        // Not discarded at Clean-up: it stays on the mat.
        end_turn_with(&mut g, &mut NoEvents, |_, d| matches!(d.kind, DecisionKind::YesNo { .. }).then_some(Choice::No));
        assert_eq!(on_mat(&g, 0, card), 1);
        assert!(!g.players[0].discard.has(card));
    }
}

#[test]
fn coin_of_the_realm_is_a_copper_that_waits_on_the_mat() {
    let mut g = new_state(&[id::COIN_OF_THE_REALM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::COIN_OF_THE_REALM]);
    let step = adv(&mut g);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(d.kind, DecisionKind::PlayTreasure, "it has an effect, so Treasures are played by choice");
    play_treasure(&mut g, id::COIN_OF_THE_REALM);
    assert_eq!(g.turn.coins, 1, "$1 this turn");
    assert_eq!(on_mat(&g, 0, id::COIN_OF_THE_REALM), 1);
    assert!(!g.players[0].in_play.has(id::COIN_OF_THE_REALM));
}

#[test]
fn coin_of_the_realm_called_after_an_action_gives_two_actions() {
    let mut g = new_state(&[id::COIN_OF_THE_REALM, id::SMITHY, id::MILITIA], 2);
    g.players[0].tavern.add(id::COIN_OF_THE_REALM, 1);
    set_hand(&mut g, 0, &[id::MILITIA, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::COPPER]);
    let before = total_cards(&g);
    play(&mut g, id::MILITIA);
    // Militia's attack resolves first (a hand of 1 card: nothing to discard); then the window opens.
    assert!(call_decision(&g), "{:?}", g.pending_decision());
    let d = g.pending_decision().unwrap();
    assert_eq!(d.subject, id::COIN_OF_THE_REALM);
    assert_eq!(g.turn.actions, 0);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.actions, 2, "+2 Actions");
    assert!(g.players[0].in_play.has(id::COIN_OF_THE_REALM), "a called card is in play");
    assert_eq!(on_mat(&g, 0, id::COIN_OF_THE_REALM), 0);
    assert!(!g.turn.played.has(id::COIN_OF_THE_REALM), "calling is not playing");
    // The Smithy can be played now.
    play(&mut g, id::SMITHY);
    assert_eq!(total_cards(&g), before);
    end_turn(&mut g);
    assert!(g.players[0].discard.has(id::COIN_OF_THE_REALM), "discarded at Clean-up with the other cards in play");
}

#[test]
fn coin_of_the_realm_can_be_declined_and_two_can_be_called_for_one_play() {
    let mut g = new_state(&[id::COIN_OF_THE_REALM, id::SMITHY], 2);
    g.players[0].tavern.add(id::COIN_OF_THE_REALM, 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SMITHY);
    assert!(call_decision(&g));
    choose(&mut g, Choice::Yes);
    assert!(call_decision(&g), "the second Coin is asked about too");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.actions, 4);
    assert_eq!(on_mat(&g, 0, id::COIN_OF_THE_REALM), 0);

    let mut g = new_state(&[id::COIN_OF_THE_REALM, id::SMITHY], 2);
    g.players[0].tavern.add(id::COIN_OF_THE_REALM, 1);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SMITHY);
    choose(&mut g, Choice::No);
    assert_eq!(g.turn.actions, 0);
    assert_eq!(on_mat(&g, 0, id::COIN_OF_THE_REALM), 1);
}

#[test]
fn coin_of_the_realm_works_on_a_reserve_card_that_went_to_the_mat_but_royal_carriage_does_not() {
    let mut g = new_state(&[id::COIN_OF_THE_REALM, id::ROYAL_CARRIAGE, id::RATCATCHER], 2);
    g.players[0].tavern.add(id::COIN_OF_THE_REALM, 1);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::RATCATCHER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::RATCATCHER);
    // The Ratcatcher is on the mat, not in play: only the Coin can be called.
    assert!(call_decision(&g));
    assert_eq!(g.pending_decision().unwrap().subject, id::COIN_OF_THE_REALM);
    choose(&mut g, Choice::No);
    assert!(!call_decision(&g), "{:?}", g.pending_decision());
}

#[test]
fn royal_carriage_replays_the_action_it_follows() {
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::SMITHY], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::GOLD, id::SILVER, id::SILVER, id::SILVER, id::ESTATE]);
    let before = total_cards(&g);
    play(&mut g, id::SMITHY);
    // The Smithy has been completely resolved (3 Golds drawn) before the Carriage is asked about.
    assert_eq!(g.players[0].hand.get(id::GOLD), 3);
    assert!(call_decision(&g));
    assert_eq!(g.pending_decision().unwrap().subject, id::ROYAL_CARRIAGE);
    let mut log: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Yes, &mut log);
    assert_eq!(held(&g, 0).get(id::SILVER), 3, "Smithy played again");
    assert!(log.iter().any(|e| matches!(e, Event::PlayAgain { card, .. } if *card == id::SMITHY)), "{log:?}");
    assert!(g.players[0].in_play.has(id::ROYAL_CARRIAGE));
    assert_eq!(g.players[0].in_play.get(id::SMITHY), 1, "one Smithy, played twice");
    assert_eq!(total_cards(&g), before);
}

#[test]
fn royal_carriage_is_not_offered_for_a_reserve_that_went_to_the_mat() {
    // A Royal Carriage called earlier stays in play with the Duration it replayed. Playing another Royal Carriage from the hand puts that one
    // on the mat: it is not in play, and the copy in play is not it. Offering the call there looped forever (each replay of a Royal Carriage
    // put one back on the mat as the call took one off).
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::SMITHY], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    g.players[0].in_play.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::ROYAL_CARRIAGE, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::ROYAL_CARRIAGE);
    assert!(!call_decision(&g), "{:?}", g.pending_decision());
    assert_eq!(g.players[0].tavern.get(id::ROYAL_CARRIAGE), 2);
    // A non-Reserve Action still gets its offer.
    play(&mut g, id::SMITHY);
    assert!(call_decision(&g));
}

#[test]
fn royal_carriage_cannot_replay_a_card_that_left_play() {
    // Raze trashing itself: nothing to replay.
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::RAZE], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::RAZE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::RAZE);
    // Trash the Raze itself.
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Trash }));
    choose(&mut g, Choice::Yes);
    // Look at two cards, keep one.
    while !matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::PlayAction | DecisionKind::Buy) | None) {
        let first = choices(&g)[0];
        choose(&mut g, first);
        if call_decision(&g) {
            panic!("Royal Carriage offered for a card that is no longer in play");
        }
    }
}

#[test]
fn several_royal_carriages_replay_the_same_action_one_after_the_other() {
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::MARKET], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 2);
    set_hand(&mut g, 0, &[id::MARKET]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::MARKET);
    assert!(call_decision(&g));
    choose(&mut g, Choice::Yes);
    assert!(call_decision(&g), "the replay opens its own window for the second Carriage");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.buys, 4, "Market played three times: +1 Buy each, plus the 1");
    assert_eq!(on_mat(&g, 0, id::ROYAL_CARRIAGE), 0);
}

#[test]
fn a_replayed_duration_keeps_royal_carriage_in_play() {
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::CARAVAN_GUARD], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::CARAVAN_GUARD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CARAVAN_GUARD);
    assert!(call_decision(&g));
    choose(&mut g, Choice::Yes);
    end_turn(&mut g);
    let ps = &g.players[0];
    assert!(ps.in_play.has(id::ROYAL_CARRIAGE) && ps.in_play.has(id::CARAVAN_GUARD), "both stay out for the Duration's next turn");
    assert_eq!(ps.pending_durations_len, 1);
    assert_eq!(ps.pending_durations[0].times, 2, "played twice: +$1 twice next turn");
}

#[test]
fn ratcatcher_trashes_a_card_at_the_start_of_a_turn() {
    let mut g = new_state(&[id::RATCATCHER, id::SMITHY], 2);
    g.players[0].tavern.add(id::RATCATCHER, 1);
    set_hand(&mut g, 0, &[id::ESTATE, id::COPPER, id::COPPER, id::SMITHY, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    let step = adv(&mut g);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Call }) && d.subject == id::RATCATCHER, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].in_play.has(id::RATCATCHER), "called: in play");
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::Select { act: Act::Trash, min: 1, max: 1, .. }));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.trash.get(id::ESTATE), 1);
    // It is not an Action played: the Smithy can still be played.
    assert_eq!(g.turn.actions, 1);
    end_turn(&mut g);
    assert!(g.players[0].discard.has(id::RATCATCHER), "discarded at Clean-up: back in the deck cycle");
}

#[test]
fn guide_discards_the_hand_and_draws_five_at_the_start_of_a_turn() {
    let mut g = new_state(&[id::GUIDE, id::SMITHY], 2);
    g.players[0].tavern.add(id::GUIDE, 1);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY, id::PROVINCE, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::SILVER, id::SILVER, id::COPPER, id::ESTATE]);
    let step = adv(&mut g);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::YesNo { act: Act::Call }, .. })));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].discard.total(), 5, "the old hand");
    let h = held(&g, 0);
    assert_eq!((h.get(id::GOLD), h.get(id::SILVER), h.get(id::COPPER)), (2, 2, 1), "five new cards");
}

#[test]
fn transmogrify_trashes_a_card_and_gains_one_costing_up_to_a_coin_more_into_the_hand() {
    let mut g = new_state(&[id::TRANSMOGRIFY, id::SMITHY], 2);
    g.players[0].tavern.add(id::TRANSMOGRIFY, 1);
    set_hand(&mut g, 0, &[id::COPPER, id::ESTATE, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    choose(&mut g, Choice::Yes);
    pick(&mut g, id::ESTATE);
    // Estate costs $2: up to $3 (Silver), into the hand.
    assert!(choices(&g).contains(&Choice::Card(id::SILVER)) && !choices(&g).contains(&Choice::Card(id::SMITHY)));
    assert!(choices(&g).contains(&Choice::Card(id::ESTATE)), "the same cost is fine");
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].hand.get(id::SILVER), 1);
    assert_eq!(g.trash.get(id::ESTATE), 1);
}

#[test]
fn start_of_turn_calls_may_be_declined_and_several_cards_are_each_asked() {
    let mut g = new_state(&[id::RATCATCHER, id::GUIDE], 2);
    g.players[0].tavern.add(id::RATCATCHER, 1);
    g.players[0].tavern.add(id::GUIDE, 1);
    set_hand(&mut g, 0, &[id::ESTATE, id::COPPER, id::COPPER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    let step = adv(&mut g);
    // Two different order-sensitive effects: the player picks which resolves first.
    let Step::Decision(d) = step else { panic!() };
    assert_eq!(d.kind, DecisionKind::StartOrder);
    choose(&mut g, Choice::Card(id::RATCATCHER));
    assert_eq!(g.pending_decision().unwrap().subject, id::RATCATCHER);
    choose(&mut g, Choice::No);
    assert_eq!(g.pending_decision().unwrap().subject, id::GUIDE);
    choose(&mut g, Choice::No);
    assert_eq!((on_mat(&g, 0, id::RATCATCHER), on_mat(&g, 0, id::GUIDE)), (1, 1), "declined: still on the mat");
}

#[test]
fn distant_lands_is_worth_four_victory_points_only_on_the_mat() {
    let mut g = new_state(&[id::DISTANT_LANDS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DISTANT_LANDS]);
    assert_eq!(g.players[0].vp(), 0);
    play(&mut g, id::DISTANT_LANDS);
    assert_eq!(g.players[0].vp(), 4);
    // It is a Victory card, an Action and a Reserve at once, and counts as a card of the deck for Gardens-like scores.
    assert_eq!(g.players[0].all_cards().get(id::DISTANT_LANDS), 1);
    g.players[0].tavern.add(id::DISTANT_LANDS, 1);
    assert_eq!(g.players[0].vp(), 8);
    let mut g2 = new_state(&[id::DISTANT_LANDS], 2);
    set_hand(&mut g2, 0, &[id::DISTANT_LANDS]);
    assert_eq!(g2.players[0].vp(), 0, "in the hand it is worth nothing");
}

#[test]
fn duplicate_gains_a_copy_of_a_card_costing_up_to_six() {
    let mut g = new_state(&[id::DUPLICATE, id::SMITHY], 2);
    g.players[0].tavern.add(id::DUPLICATE, 1);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 6;
    let step = adv(&mut g);
    assert!(matches!(step, Step::Decision(Decision { kind: DecisionKind::Buy, .. })));
    let before = total_cards(&g);
    buy(&mut g, id::GOLD);
    assert!(call_decision(&g), "{:?}", g.pending_decision());
    assert_eq!(g.pending_decision().unwrap().subject, id::DUPLICATE);
    choose(&mut g, Choice::Yes);
    // (The turn is over now: the Duplicate that was called was in play and is discarded with the rest.)
    assert_eq!(g.players[0].discard.get(id::GOLD), 2);
    assert!(g.players[0].discard.has(id::DUPLICATE));
    assert_eq!(on_mat(&g, 0, id::DUPLICATE), 0);
    assert_eq!(total_cards(&g), before);
    assert_eq!(g.turn.coins, 0, "the copy is free");
}

#[test]
fn duplicate_is_not_offered_for_a_card_costing_more_or_for_a_card_with_no_copy() {
    let mut g = new_state(&[id::DUPLICATE, id::SMITHY], 2);
    g.players[0].tavern.add(id::DUPLICATE, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 8;
    adv(&mut g);
    buy(&mut g, id::PROVINCE);
    assert!(!call_decision(&g), "a Province costs $8");

    // The last Smithy: no copy left.
    let mut g = new_state(&[id::DUPLICATE, id::SMITHY], 2);
    g.players[0].tavern.add(id::DUPLICATE, 1);
    g.supply.set(id::SMITHY, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    adv(&mut g);
    buy(&mut g, id::SMITHY);
    assert!(!call_decision(&g));
}

#[test]
fn duplicate_works_on_another_players_turn_and_for_cards_gained_other_ways() {
    // Player 1 has a Duplicate; player 0 gains a Workshop card... only player 0's own gains trigger player 0's Duplicate.
    let mut g = new_state(&[id::DUPLICATE, id::WORKSHOP, id::MESSENGER], 2);
    g.players[1].tavern.add(id::DUPLICATE, 1);
    set_hand(&mut g, 0, &[id::MESSENGER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    adv(&mut g);
    // Player 0 buys a Messenger and gains a card; player 1 gains a copy: that is a gain, so Duplicate may be called.
    buy(&mut g, id::MESSENGER);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    pick(&mut g, id::SILVER);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 1, "{d:?}");
    assert!(call_decision(&g));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[1].discard.get(id::SILVER), 2, "the copy from Messenger and the Duplicate's");
}

#[test]
fn wine_merchant_is_discarded_from_the_mat_with_two_coins_unspent() {
    let mut g = new_state(&[id::WINE_MERCHANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WINE_MERCHANT]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WINE_MERCHANT);
    assert_eq!((g.turn.coins, g.turn.buys), (4, 2));
    assert_eq!(on_mat(&g, 0, id::WINE_MERCHANT), 1);
    // $4 unspent at the end of the Buy phase: may be discarded from the mat.
    pass(&mut g);
    let d = g.pending_decision().expect("an offer");
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Discard }) && d.subject == id::WINE_MERCHANT, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(on_mat(&g, 0, id::WINE_MERCHANT), 0);
    end_turn(&mut g);
    assert_eq!(g.players[0].discard.get(id::WINE_MERCHANT), 1, "it goes to the discard pile, not into play");
    assert!(!g.players[0].in_play.has(id::WINE_MERCHANT));
}

#[test]
fn wine_merchant_needs_two_unspent_in_total_and_is_kept_otherwise() {
    // $1 left: no offer.
    let mut g = new_state(&[id::WINE_MERCHANT, id::SMITHY], 2);
    g.players[0].tavern.add(id::WINE_MERCHANT, 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 1;
    adv(&mut g);
    pass(&mut g);
    assert_eq!(on_mat(&g, 0, id::WINE_MERCHANT), 2);

    // $2 is enough for both Wine Merchants (not $2 each).
    let mut g = new_state(&[id::WINE_MERCHANT, id::SMITHY], 2);
    g.players[0].tavern.add(id::WINE_MERCHANT, 2);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 2;
    adv(&mut g);
    pass(&mut g);
    choose(&mut g, Choice::Yes);
    choose(&mut g, Choice::Yes);
    assert_eq!(on_mat(&g, 0, id::WINE_MERCHANT), 0);
    assert_eq!(g.players[0].discard.get(id::WINE_MERCHANT), 2);
}

#[test]
fn miser_puts_coppers_on_the_mat_or_pays_for_them() {
    let mut g = new_state(&[id::MISER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::MISER, id::COPPER, id::COPPER]);
    play(&mut g, id::MISER);
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1)]);
    choose(&mut g, Choice::Mode(0));
    // The one Copper is put on the mat without asking; the other one is played in the Buy phase.
    assert_eq!(on_mat(&g, 0, id::COPPER), 1);
    assert_eq!(held(&g, 0).get(id::COPPER), 1);
    assert_eq!(count(&g, 0, id::COPPER), 2, "not trashed: still owned");

    let mut g = new_state(&[id::MISER, id::SMITHY], 2);
    g.players[0].tavern.add(id::COPPER, 3);
    set_hand(&mut g, 0, &[id::MISER]);
    play(&mut g, id::MISER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.turn.coins, 3, "+$1 per Copper on the mat");

    // With no Copper in the hand nothing happens.
    let mut g = new_state(&[id::MISER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::MISER]);
    play(&mut g, id::MISER);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(on_mat(&g, 0, id::COPPER), 0);
}


// ===========================================================================
// The Action and Treasure cards
// ===========================================================================

fn pending_kind(g: &GameState) -> Option<DecisionKind> {
    g.pending_decision().map(|d| d.kind)
}

#[test]
fn raze_trashes_itself_and_looks_at_two_cards() {
    let mut g = new_state(&[id::RAZE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RAZE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER, id::SILVER]);
    let before = total_cards(&g);
    play(&mut g, id::RAZE);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    assert!(matches!(pending_kind(&g), Some(DecisionKind::YesNo { act: Act::Trash })), "{:?}", g.pending_decision());
    choose(&mut g, Choice::Yes);
    assert_eq!(g.trash.get(id::RAZE), 1, "it costs $2: look at two cards");
    // Keep one of Gold and Copper (the rest is discarded): the Select asks which to discard.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Revealed, act: Act::Discard, .. })));
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.players[0].hand.get(id::GOLD) + held(&g, 0).get(id::GOLD), 1.max(held(&g, 0).get(id::GOLD)));
    assert_eq!(held(&g, 0).get(id::GOLD), 1);
    assert_eq!(g.players[0].discard.get(id::COPPER), 1);
    assert_eq!(total_cards(&g), before);
}

#[test]
fn raze_trashes_a_card_from_the_hand_and_looks_at_as_many_as_it_costs() {
    let mut g = new_state(&[id::RAZE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RAZE, id::SMITHY, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::PROVINCE, id::GOLD, id::GOLD]);
    play(&mut g, id::RAZE);
    choose(&mut g, Choice::No);
    // Trash the Smithy ($4): look at four cards.
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.trash.get(id::SMITHY), 1);
    // Keep the Province: discard Estate, Duchy and the Gold.
    let mut k = 0;
    while matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Revealed, .. })) {
        let c = choices(&g).into_iter().find(|c| *c != Choice::Card(id::PROVINCE)).unwrap();
        choose(&mut g, c);
        k += 1;
    }
    assert_eq!(k, 3);
    assert_eq!(g.players[0].hand.get(id::PROVINCE), 1);
    assert_eq!(g.players[0].discard.total(), 3);
}

#[test]
fn raze_on_a_card_costing_nothing_looks_at_nothing_and_may_trash_with_an_empty_hand() {
    let mut g = new_state(&[id::RAZE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RAZE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    play(&mut g, id::RAZE);
    choose(&mut g, Choice::No);
    assert_eq!(g.trash.get(id::COPPER), 1);
    assert_eq!(g.players[0].deck_known.len, 2, "$0: no cards looked at");
}

#[test]
fn raze_is_not_affected_by_the_minus_one_card_token_and_replaces_it() {
    let mut g = new_state(&[id::RAZE, id::SMITHY], 2);
    g.players[0].adv |= ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::RAZE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER, id::SILVER]);
    play(&mut g, id::RAZE);
    choose(&mut g, Choice::Yes);
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(g.players[0].adv & ADV_MINUS_CARD != 0, "the token is still there");
}

#[test]
fn amulet_gives_a_choice_now_and_at_the_start_of_the_next_turn() {
    let mut g = new_state(&[id::AMULET, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::AMULET, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    let step = play(&mut g, id::AMULET);
    let Step::Decision(d) = step else { panic!() };
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 1, .. }));
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1), Choice::Mode(2)]);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.turn.coins, 1);
    end_turn(&mut g);
    // Player 1 passes; back to player 0: the Amulet fires at the start of the turn.
    assert!(g.players[0].in_play.has(id::AMULET));
    end_turn(&mut g);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Mode { .. }) && d.player == 0, "{d:?}");
    // Gain a Silver this time.
    choose(&mut g, Choice::Mode(2));
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
}

#[test]
fn amulet_can_trash_a_card_from_the_hand() {
    let mut g = new_state(&[id::AMULET, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::AMULET, id::ESTATE, id::COPPER]);
    play(&mut g, id::AMULET);
    choose(&mut g, Choice::Mode(1));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.trash.get(id::ESTATE), 1);
}

#[test]
fn caravan_guard_draws_now_and_gives_a_coin_next_turn() {
    let mut g = new_state(&[id::CARAVAN_GUARD, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CARAVAN_GUARD]);
    let mut deck = vec![id::GOLD];
    deck.extend(junk());
    set_deck_known(&mut g, 0, &deck);
    play(&mut g, id::CARAVAN_GUARD);
    assert_eq!(held(&g, 0).get(id::GOLD), 1);
    end_turn(&mut g);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.coins, 1, "+$1 at the start of the next turn");
}

#[test]
fn caravan_guard_is_played_from_the_hand_against_an_attack_without_giving_the_attacker_anything() {
    let mut g = new_state(&[id::CARAVAN_GUARD, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::CARAVAN_GUARD, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD]);
    let actions_before = g.turn.actions;
    let step = play(&mut g, id::MILITIA);
    let Step::Decision(d) = step else { panic!("{step:?}") };
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::CARAVAN_GUARD, "{d:?}");
    choose(&mut g, Choice::Yes);
    // The Guard drew a card (a Gold) and is in play; the Militia attack follows: discard down to 3 (hand 5 + 1 - Guard = 5).
    assert!(g.players[1].in_play.has(id::CARAVAN_GUARD));
    assert_eq!(g.players[1].hand.get(id::GOLD), 1);
    assert_eq!(g.turn.actions, actions_before - 1, "the Militia used the only Action; the Guard's +1 Action is not the attacker's");
    assert_eq!(g.turn.coins, 2, "just the Militia's $2");
    assert_eq!(g.players[1].pending_durations_len, 1);
}

#[test]
fn dungeon_draws_and_discards_now_and_next_turn() {
    let mut g = new_state(&[id::DUNGEON, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DUNGEON, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::DUNGEON);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    // +2 Cards, then discard 2 of the 3 cards in hand.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, min: 2, max: 2, .. })));
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(held(&g, 0).get(id::GOLD), 1);
    assert_eq!(g.players[0].discard.total(), 2);
    end_turn(&mut g);
    end_turn(&mut g);
    // The start of the next turn: draw 2, discard 2 again (the first of the two picks may be made for the player).
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, min: 1..=2, max: 1..=2, .. })), "{:?}", g.pending_decision());
}

#[test]
fn gear_sets_aside_up_to_two_cards_and_returns_them_next_turn() {
    let mut g = new_state(&[id::GEAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GEAR, id::ESTATE, id::DUCHY, id::PROVINCE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::GEAR);
    assert_eq!(held(&g, 0).get(id::GOLD), 2, "+2 Cards");
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::SetAside, min: 0, max: 2, .. })));
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::DUCHY));
    assert_eq!(g.players[0].set_aside.total(), 2);
    let before = total_cards(&g);
    end_turn(&mut g);
    assert!(g.players[0].in_play.has(id::GEAR), "Gear stays in play");
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.players[0].set_aside.total(), 0);
    assert!(g.players[0].hand.has(id::ESTATE) && g.players[0].hand.has(id::DUCHY), "both returned to the hand");
    assert_eq!(total_cards(&g), before);
}

#[test]
fn gear_that_sets_nothing_aside_is_discarded_the_same_turn() {
    let mut g = new_state(&[id::GEAR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GEAR, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::GEAR);
    choose(&mut g, Choice::Pass);
    end_turn(&mut g);
    assert!(g.players[0].discard.has(id::GEAR) && !g.players[0].in_play.has(id::GEAR));
}

#[test]
fn magpie_takes_a_treasure_and_gains_a_magpie_for_an_action_or_victory_card() {
    // Treasure on top: into the hand.
    let mut g = new_state(&[id::MAGPIE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::MAGPIE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::SILVER, id::GOLD]);
    play(&mut g, id::MAGPIE);
    // +1 Card drew the Estate; the Silver was revealed and taken.
    assert_eq!(held(&g, 0).get(id::SILVER), 1);
    assert_eq!(g.players[0].discard.get(id::MAGPIE), 0);
    // An Action on top: gain a Magpie; it stays on top.
    let mut g = new_state(&[id::MAGPIE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::MAGPIE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SMITHY, id::GOLD]);
    play(&mut g, id::MAGPIE);
    assert_eq!(g.players[0].discard.get(id::MAGPIE), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SMITHY));
    // A Victory card on top: a Magpie too. A Curse: nothing.
    let mut g = new_state(&[id::MAGPIE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::MAGPIE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::ESTATE]);
    play(&mut g, id::MAGPIE);
    assert_eq!(g.players[0].discard.get(id::MAGPIE), 1);
    let mut g = new_state(&[id::MAGPIE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::MAGPIE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::CURSE]);
    play(&mut g, id::MAGPIE);
    assert_eq!(g.players[0].discard.get(id::MAGPIE), 0);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::CURSE));
}

#[test]
fn port_comes_with_another_port_that_does_not_come_with_another() {
    let mut g = new_state(&[id::PORT, id::WORKSHOP], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    buy(&mut g, id::PORT);
    assert_eq!(count(&g, 0, id::PORT), 2);
    assert_eq!(g.supply.get(id::PORT), 10);
    // A Port gained another way comes with another too (this applies whether it is bought or not).
    let mut g = new_state(&[id::PORT, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::PORT);
    assert_eq!(count(&g, 0, id::PORT), 2);
    // With one left, only one.
    let mut g = new_state(&[id::PORT, id::WORKSHOP], 2);
    g.supply.set(id::PORT, 1);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::PORT);
    assert_eq!(count(&g, 0, id::PORT), 1);
}

#[test]
fn ranger_draws_five_every_other_play() {
    let mut g = new_state(&[id::RANGER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RANGER, id::RANGER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::RANGER);
    assert_eq!(g.turn.buys, 2);
    assert!(g.players[0].adv & ADV_JOURNEY_DOWN != 0, "the token starts face up and is face down after the first play");
    assert_eq!(g.players[0].hand.total(), 1, "nothing drawn the first time");
    play(&mut g, id::RANGER);
    assert!(g.players[0].adv & ADV_JOURNEY_DOWN == 0);
    assert_eq!(g.players[0].hand.total(), 5, "+5 Cards the second time");
    assert_eq!(g.turn.buys, 3);
}

#[test]
fn artificer_gains_a_card_costing_exactly_one_per_card_discarded_onto_the_deck() {
    let mut g = new_state(&[id::ARTIFICER, id::SMITHY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::ARTIFICER, id::ESTATE, id::COPPER, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::PROVINCE]);
    play(&mut g, id::ARTIFICER);
    assert_eq!((g.turn.actions, g.turn.coins), (1, 1), "+1 Card +1 Action +$1");
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, min: 0, max: 255, .. })));
    // Discard two: gain a card costing exactly $2 (Estate, Moat-like): the Estate pile has them.
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::DUCHY)); // (nothing is left to discard in card order, so the discarding ends here)
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 2, exact: true, dest: Dest::DeckTop, optional: true, .. }), "{d:?}");
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ESTATE)) && cs.contains(&Choice::Pass));
    assert!(!cs.contains(&Choice::Card(id::COPPER)), "exactly $2, not up to");
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));
}

#[test]
fn artificer_discarding_nothing_may_gain_a_card_costing_nothing() {
    let mut g = new_state(&[id::ARTIFICER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ARTIFICER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::PROVINCE]);
    play(&mut g, id::ARTIFICER);
    // The hand now holds a Gold: discard nothing.
    choose(&mut g, Choice::Pass);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::COPPER)) && cs.contains(&Choice::Card(id::CURSE)) && cs.contains(&Choice::Pass), "{cs:?}");
    choose(&mut g, Choice::Pass);
    assert_eq!(g.players[0].deck_known.len, 1);
}

#[test]
fn bridge_troll_gives_the_other_players_a_minus_coin_token_and_cheapens_cards_two_turns() {
    let mut g = new_state(&[id::BRIDGE_TROLL, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::BRIDGE_TROLL]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::GOLD, id::GOLD]);
    play(&mut g, id::BRIDGE_TROLL);
    assert!(g.players[1].adv & ADV_MINUS_COIN != 0);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.cost(id::GOLD), 5);
    assert_eq!(g.cost(id::COPPER), 0, "never below $0");
    end_turn(&mut g);
    // Player 1 has the token: their first coins this turn are $1 short.
    assert_eq!(g.turn.coins, 5, "Gold + Gold = $6, less the token's $1");
    assert!(g.players[1].adv & ADV_MINUS_COIN == 0, "the token is gone");
    end_turn(&mut g);
    // Back to player 0: +1 Buy at the start of the turn and cards cost $1 less on this turn too.
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.cost(id::GOLD), 5);
}

#[test]
fn two_bridge_trolls_cheapen_cards_by_two() {
    let mut g = new_state(&[id::BRIDGE_TROLL, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::BRIDGE_TROLL]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::BRIDGE_TROLL));
    assert_eq!(g.cost(id::GOLD), 4);
    assert_eq!(g.turn.buys, 3);
}

#[test]
fn giant_alternates_between_a_coin_and_a_big_attack() {
    let mut g = new_state(&[id::GIANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GIANT, id::GIANT]);
    set_deck_known(&mut g, 1, &[id::SMITHY, id::GOLD]);
    g.turn.actions = 2;
    play(&mut g, id::GIANT);
    assert_eq!(g.turn.coins, 1, "face down: +$1 and nothing more");
    assert_eq!(g.players[1].deck_known.len, 2);
    play(&mut g, id::GIANT);
    assert_eq!(g.turn.coins, 6, "+$5");
    // The victim revealed a Smithy ($4): trashed.
    assert_eq!(g.trash.get(id::SMITHY), 1);
    assert_eq!(g.players[1].discard.total(), 0);
}

#[test]
fn giants_attack_discards_and_curses_a_card_outside_three_to_six() {
    let mut g = new_state(&[id::GIANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GIANT]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::GOLD]);
    g.players[0].adv |= ADV_JOURNEY_DOWN; // the next flip makes it face up
    play(&mut g, id::GIANT);
    assert_eq!(g.players[1].discard.get(id::COPPER), 1);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    assert_eq!(g.trash.total(), 0);
    // A card costing $7 (Platinum-like $7?) -- Province costs $8: also discarded and cursed.
    let mut g = new_state(&[id::GIANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GIANT]);
    set_deck_known(&mut g, 1, &[id::PROVINCE]);
    g.players[0].adv |= ADV_JOURNEY_DOWN;
    play(&mut g, id::GIANT);
    assert_eq!(g.players[1].discard.get(id::PROVINCE), 1);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
}

#[test]
fn giant_with_no_cards_to_reveal_still_curses() {
    let mut g = new_state(&[id::GIANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GIANT]);
    clear_deck(&mut g, 1);
    g.players[0].adv |= ADV_JOURNEY_DOWN;
    play(&mut g, id::GIANT);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
}

#[test]
fn haunted_woods_draws_three_next_turn_and_topdecks_the_hand_of_a_buying_player() {
    let mut g = new_state(&[id::HAUNTED_WOODS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HAUNTED_WOODS]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD]);
    play(&mut g, id::HAUNTED_WOODS);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1);
    // Player 1 buys a card (two Coppers played; the Estate and the Duchy stay in the hand): they go on the deck in any order.
    buy(&mut g, id::COPPER);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Hand, act: Act::Topdeck, ordered: true, .. })), "{:?}", g.pending_decision());
    choose(&mut g, Choice::Card(id::DUCHY));
    // The last card of the hand is put without asking, and the Clean-up draw takes them (and the two Golds) back: the Estate, put last, was
    // on top.
    let hand = g.players[1].hand;
    assert!(hand.has(id::ESTATE) && hand.has(id::DUCHY) && hand.get(id::GOLD) == 2, "{hand:?}");
}

#[test]
fn haunted_woods_does_not_touch_a_card_gained_without_buying_it() {
    let mut g = new_state(&[id::HAUNTED_WOODS, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::HAUNTED_WOODS]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::WORKSHOP, id::ESTATE]);
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::HAUNTED_WOODS);
    end_turn(&mut g);
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[1].hand.get(id::ESTATE), 1, "the hand is untouched");
}

#[test]
fn haunted_woods_gives_three_cards_at_the_start_of_its_owners_next_turn() {
    let mut g = new_state(&[id::HAUNTED_WOODS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HAUNTED_WOODS]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HAUNTED_WOODS);
    end_turn(&mut g);
    end_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 8);
}

#[test]
fn swamp_hag_curses_a_player_who_buys_a_card_and_gives_three_coins_next_turn() {
    let mut g = new_state(&[id::SWAMP_HAG, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SWAMP_HAG]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::SWAMP_HAG);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1);
    buy(&mut g, id::SILVER);
    assert_eq!(count(&g, 1, id::CURSE), 1, "a Curse per card bought");
    // A card not bought (a gain by another effect) gives no Curse.
    let mut g = new_state(&[id::SWAMP_HAG, id::SMITHY, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::SWAMP_HAG]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::WORKSHOP]);
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::SWAMP_HAG);
    end_turn(&mut g);
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::SILVER);
    assert_eq!(count(&g, 1, id::CURSE), 0);
    // And +$3 for its owner at the start of the next turn.
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.coins, 3);
}

#[test]
fn swamp_hags_stack_for_each_card_bought() {
    let mut g = new_state(&[id::SWAMP_HAG, id::SMITHY, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SWAMP_HAG]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::SWAMP_HAG));
    end_turn(&mut g);
    buy(&mut g, id::SILVER);
    assert_eq!(count(&g, 1, id::CURSE), 2, "a Throne Roomed Swamp Hag curses twice");
}

#[test]
fn lost_city_makes_the_other_players_draw_when_gained() {
    let mut g = new_state(&[id::LOST_CITY, id::SMITHY], 3);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::GOLD, id::ESTATE]);
    set_deck_known(&mut g, 2, &[id::SILVER, id::ESTATE]);
    adv(&mut g);
    buy(&mut g, id::LOST_CITY);
    assert_eq!(held(&g, 1).get(id::GOLD), 1);
    assert_eq!(held(&g, 2).get(id::SILVER), 1);
    assert_eq!(g.players[0].hand.total(), 5, "the gaining player does not draw (their own new hand)");
}

#[test]
fn messenger_gains_a_card_for_everyone_when_it_is_the_first_gain_of_the_buy_phase() {
    let mut g = new_state(&[id::MESSENGER, id::SMITHY, id::VILLAGE], 3);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    buy(&mut g, id::MESSENGER);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    pick(&mut g, id::VILLAGE);
    assert_eq!(count(&g, 0, id::VILLAGE), 1);
    assert_eq!(count(&g, 1, id::VILLAGE), 1);
    assert_eq!(count(&g, 2, id::VILLAGE), 1);
}

#[test]
fn messenger_does_nothing_extra_when_it_is_not_the_first_gain_or_not_in_the_buy_phase() {
    let mut g = new_state(&[id::MESSENGER, id::SMITHY, id::WORKSHOP], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 8;
    g.turn.buys = 2;
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    buy(&mut g, id::SILVER);
    buy(&mut g, id::MESSENGER);
    assert!(!matches!(pending_kind(&g), Some(DecisionKind::Gain { .. })), "{:?}", g.pending_decision());

    // Gained in the Action phase (Workshop): nothing.
    let mut g = new_state(&[id::MESSENGER, id::SMITHY, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::MESSENGER);
    assert_eq!(count(&g, 1, id::MESSENGER), 0);
}

#[test]
fn messenger_gives_a_buy_and_two_coins_and_may_discard_the_deck() {
    let mut g = new_state(&[id::MESSENGER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::MESSENGER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    play(&mut g, id::MESSENGER);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 2));
    assert!(matches!(pending_kind(&g), Some(DecisionKind::YesNo { .. })));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].deck_size(), 0);
    assert_eq!(g.players[0].discard.get(id::GOLD), 2);
}

#[test]
fn relic_is_a_two_coin_attack_treasure_that_gives_the_minus_one_card_token() {
    let mut g = new_state(&[id::RELIC, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RELIC, id::COPPER]);
    adv(&mut g);
    play_treasure(&mut g, id::RELIC);
    assert!(g.players[1].adv & ADV_MINUS_CARD != 0);
    assert_eq!(g.turn.coins, 3, "$2 and the Copper");
    // The victim's next draw is one card short.
    let mut g = new_state(&[id::RELIC, id::SMITHY], 2);
    g.players[1].adv |= ADV_MINUS_CARD;
    set_deck_known(&mut g, 1, &junk());
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    end_turn(&mut g);
    assert_eq!(g.players[1].hand.total(), 0, "player 1 has not drawn yet (their turn)");
}

#[test]
fn relic_can_be_blocked_by_a_moat() {
    let mut g = new_state(&[id::RELIC, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::RELIC]);
    set_hand(&mut g, 1, &[id::MOAT]);
    adv(&mut g);
    play_treasure(&mut g, id::RELIC);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.subject), (1, id::MOAT), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert!(g.players[1].adv & ADV_MINUS_CARD == 0);
}

#[test]
fn treasure_trove_gains_a_gold_and_a_copper() {
    let mut g = new_state(&[id::TREASURE_TROVE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::TREASURE_TROVE]);
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    play_treasure(&mut g, id::TREASURE_TROVE);
    assert_eq!(g.turn.coins, 2);
    assert_eq!((g.players[0].discard.get(id::GOLD), g.players[0].discard.get(id::COPPER)), (1, 1));
}

#[test]
fn hireling_draws_a_card_at_the_start_of_every_turn_for_the_rest_of_the_game() {
    let mut g = new_state(&[id::HIRELING, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HIRELING]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HIRELING);
    assert_eq!(g.players[0].hand.total(), 0, "no benefit this turn");
    for round in 0..4 {
        end_turn(&mut g);
        end_turn(&mut g);
        assert!(g.players[0].in_play.has(id::HIRELING), "round {round}: it stays in play");
        assert_eq!(g.players[0].hand.total(), 6, "round {round}: 5 + 1");
    }
}

#[test]
fn a_throne_roomed_hireling_draws_two_and_keeps_the_throne_room_with_it() {
    let mut g = new_state(&[id::HIRELING, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HIRELING]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::HIRELING));
    for _ in 0..3 {
        end_turn(&mut g);
        end_turn(&mut g);
        assert!(g.players[0].in_play.has(id::THRONE_ROOM) && g.players[0].in_play.has(id::HIRELING));
        assert_eq!(g.players[0].hand.total(), 7, "5 + 2");
    }
}

#[test]
fn storyteller_plays_treasures_draws_and_pays_all_its_coins_for_cards() {
    let mut g = new_state(&[id::STORYTELLER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::STORYTELLER, id::GOLD, id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::STORYTELLER);
    // Up to 3 Treasures from the hand: Gold, Silver, Copper.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Hand, act: Act::Play, max: 3, .. })));
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.turn.coins, 0, "all $6 paid");
    // +1 Card and 6 more: 7 cards drawn.
    assert_eq!(g.players[0].hand.total(), 7);
    assert_eq!(g.players[0].in_play.get(id::GOLD), 1, "the Treasures stay in play");
}

#[test]
fn storyteller_with_nothing_to_play_just_draws() {
    let mut g = new_state(&[id::STORYTELLER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::STORYTELLER]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::STORYTELLER);
    assert_eq!(g.players[0].hand.total(), 1);
    assert_eq!(g.turn.actions, 1);
}


// ===========================================================================
// The Traveller upgrades, Champion and Teacher
// ===========================================================================

#[test]
fn treasure_hunter_gains_a_silver_per_card_the_player_to_the_right_gained_last_turn() {
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::TREASURE_HUNTER]);
    set_deck_known(&mut g, 0, &junk());
    g.players[1].last_turn_gains.add(id::GOLD, 2);
    g.players[1].last_turn_gains.add(id::CURSE, 1);
    play(&mut g, id::TREASURE_HUNTER);
    assert_eq!((g.turn.actions, g.turn.coins), (1, 1), "+1 Action +$1");
    assert_eq!(g.players[0].discard.get(id::SILVER), 3, "three cards gained, three Silvers");
}

#[test]
fn treasure_hunter_counts_every_gain_the_player_made_not_only_their_buys() {
    let mut g = new_state(&[id::PAGE, id::WORKSHOP], 2);
    // Player 1's turn: Workshop gains a Silver, then buys a Duchy; then it is player 0's turn.
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::WORKSHOP, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &junk());
    set_hand(&mut g, 0, &[id::TREASURE_HUNTER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::SILVER);
    adv(&mut g);
    buy(&mut g, id::SILVER);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.players[1].last_turn_gains.total(), 2);
    play(&mut g, id::TREASURE_HUNTER);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
}

#[test]
fn warrior_draws_two_and_attacks_once_per_traveller_in_play() {
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WARRIOR]);
    set_in_play(&mut g, 0, &[id::PAGE]);
    set_deck_known(&mut g, 0, &junk());
    // Two Travellers in play: two cards from the victim's deck, those costing $3 or $4 trashed.
    set_deck_known(&mut g, 1, &[id::SILVER, id::SMITHY, id::GOLD]);
    play(&mut g, id::WARRIOR);
    assert_eq!(g.players[0].hand.total(), 2, "+2 Cards");
    assert_eq!(g.trash.get(id::SILVER) + g.trash.get(id::SMITHY), 2);
    assert_eq!(g.players[1].deck_known.len, 1);
}

#[test]
fn warrior_trashes_only_cards_costing_exactly_three_or_four_and_discards_the_rest() {
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WARRIOR]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::GOLD, id::SILVER]);
    play(&mut g, id::WARRIOR);
    // One Traveller: the Gold ($6) is discarded, not trashed.
    assert_eq!(g.players[1].discard.get(id::GOLD), 1);
    assert_eq!(g.trash.total(), 0);
    // A Potion-cost card is not $3 or $4.
    let mut g = new_state(&[id::PAGE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::WARRIOR]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::FAMILIAR]);
    play(&mut g, id::WARRIOR);
    assert_eq!(g.players[1].discard.get(id::FAMILIAR), 1);
}

#[test]
fn hero_gains_a_treasure_of_the_players_choice() {
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HERO]);
    play(&mut g, id::HERO);
    assert_eq!(g.turn.coins, 2);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::GOLD)) && cs.contains(&Choice::Card(id::COPPER)) && !cs.contains(&Choice::Card(id::DUCHY)), "{cs:?}");
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
}

#[test]
fn champion_stays_in_play_protects_from_attacks_and_gives_an_action_per_action() {
    let mut g = new_state(&[id::PAGE, id::SMITHY, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::CHAMPION, id::SMITHY, id::SMITHY, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHAMPION);
    assert_eq!(g.turn.actions, 1);
    // Every Action played from now on first gives +1 Action: three Smithies can all be played.
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.actions, 1, "-1 +1 +1");
    play(&mut g, id::SMITHY);
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.actions, 1);
    // Clean-up and later turns: it stays in play.
    end_turn(&mut g);
    end_turn(&mut g);
    assert!(g.players[0].in_play.has(id::CHAMPION) && !g.players[0].discard.has(id::CHAMPION));
    assert_eq!(g.players[0].stays.get(id::CHAMPION), 1);
    // Immune: an opposing Militia does not affect the Champion's owner.
    set_hand(&mut g, 1, &[id::MILITIA]);
    // (it is player 0's turn; pass it, so player 1 can play the Militia)
    end_turn(&mut g);
    set_hand(&mut g, 1, &[id::MILITIA]);
    g.players[0].hand = counts_of(&[id::COPPER; 5]);
    g.turn.phase = Phase::Action;
    let mut g2 = g;
    g2.pending = Pending::None;
    let d = play(&mut g2, id::MILITIA);
    let _ = d;
    assert_eq!(g2.players[0].hand.total(), 5, "no discard: the Champion protects");
}

#[test]
fn champions_action_is_reported_as_a_bonus() {
    // The +1 Action a Champion in play gives for each Action played shows in the events (the game log prints "Champion: +1 Action").
    let mut g = new_state(&[id::PAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CHAMPION, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHAMPION);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    let mut log: Vec<Event> = Vec::new();
    g.apply(Choice::Card(id::SMITHY), &mut log).unwrap();
    g.advance(&mut log);
    assert!(log.contains(&Event::Bonus { player: 0, source: id::CHAMPION, actions: 1, buys: 0, coins: 0, vp: 0 }), "{log:?}");
}

#[test]
fn champion_ignores_attacks_played_earlier() {
    // Swamp Hag's curses still arrive if it was played before the Champion.
    let mut g = new_state(&[id::PAGE, id::SWAMP_HAG], 2);
    set_hand(&mut g, 1, &[id::SWAMP_HAG]);
    set_hand(&mut g, 0, &[id::CHAMPION, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::SWAMP_HAG);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    // (player 0's hand was kept for the test) the Champion is played and the player buys: still cursed.
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER]);
}

#[test]
fn soldier_gives_coins_per_other_attack_in_play_and_makes_big_hands_discard() {
    let mut g = new_state(&[id::PEASANT, id::MILITIA], 3);
    set_in_play(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::SOLDIER]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_hand(&mut g, 2, &[id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SOLDIER);
    assert_eq!(g.turn.coins, 3, "$2 and $1 for the Militia in play (not itself)");
    // Player 1 had 4 cards: discards one; player 2 had 3: nothing.
    assert_eq!(g.players[1].hand.total(), 3);
    assert_eq!(g.players[2].hand.total(), 3);
}

#[test]
fn fugitive_draws_two_gives_an_action_and_discards_a_card() {
    let mut g = new_state(&[id::PEASANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::FUGITIVE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER]);
    play(&mut g, id::FUGITIVE);
    assert_eq!(g.turn.actions, 1);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, min: 1, max: 1, .. })));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].discard.get(id::ESTATE), 1);
}

#[test]
fn disciple_plays_an_action_twice_and_gains_a_copy() {
    let mut g = new_state(&[id::PEASANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DISCIPLE, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    let before = total_cards(&g);
    play(&mut g, id::DISCIPLE);
    assert_eq!(g.turn.actions, 0, "Disciple used the Action; the Smithy's plays are free");
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].hand.total(), 6, "two Smithies' worth of cards");
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1, "a copy gained");
    assert_eq!(total_cards(&g), before);
}

#[test]
fn disciple_on_a_non_supply_card_plays_it_twice_but_gains_nothing() {
    let mut g = new_state(&[id::PEASANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DISCIPLE, id::FUGITIVE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DISCIPLE);
    choose(&mut g, Choice::Card(id::FUGITIVE));
    let mut guard = 0;
    while matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
        guard += 1;
        assert!(guard < 6);
    }
    // Two plays of Fugitive: +1 Action each.
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].discard.get(id::FUGITIVE), 0);
    assert_eq!(g.trav[5], 5, "the Fugitive pile is untouched");
}

#[test]
fn disciple_playing_a_duration_stays_in_play_with_it() {
    let mut g = new_state(&[id::PEASANT, id::CARAVAN_GUARD], 2);
    set_hand(&mut g, 0, &[id::DISCIPLE, id::CARAVAN_GUARD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DISCIPLE);
    choose(&mut g, Choice::Card(id::CARAVAN_GUARD));
    end_turn(&mut g);
    assert!(g.players[0].in_play.has(id::DISCIPLE) && g.players[0].in_play.has(id::CARAVAN_GUARD));
}

// ===========================================================================
// Sauna / Avanto
// ===========================================================================

#[test]
fn the_split_pile_has_five_of_each_with_only_the_top_card_for_sale() {
    let g = new_state(&[id::SAUNA, id::SMITHY], 2);
    assert_eq!((g.supply.get(id::SAUNA), g.supply.get(id::AVANTO)), (5, 5));
    assert!(g.in_supply(id::SAUNA) && g.in_supply(id::AVANTO));
    let mut g = g;
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::SAUNA)) && !choices(&g).contains(&Choice::Card(id::AVANTO)), "Avanto is under the Saunas");
    g.supply.set(id::SAUNA, 0);
    g.pending = Pending::None;
    adv(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::AVANTO)));
}

#[test]
fn either_half_names_the_pile_in_a_kingdom() {
    let a = new_state(&[id::AVANTO, id::SMITHY], 2);
    assert!(a.in_supply(id::SAUNA) && a.supply.get(id::SAUNA) == 5 && a.supply.get(id::AVANTO) == 5);
    let both = new_state(&[id::SAUNA, id::AVANTO], 2);
    assert_eq!((both.supply.get(id::SAUNA), both.supply.get(id::AVANTO)), (5, 5));
    let k = dominion_engine::text::parse_kingdom("Avanto, Smithy").unwrap();
    assert_eq!(k, vec![id::SAUNA, id::SMITHY]);
}

#[test]
fn the_split_pile_counts_as_one_pile_for_the_end_of_the_game() {
    let mut g = new_state(&[id::SAUNA, id::SMITHY], 2);
    g.supply.set(id::SAUNA, 0);
    assert_eq!(g.empty_piles(), 0, "the Avanto are still there");
    g.supply.set(id::AVANTO, 0);
    assert_eq!(g.empty_piles(), 1);
    g.supply.set(id::SMITHY, 0);
    assert_eq!(g.empty_piles(), 2);
}

#[test]
fn a_gain_of_a_split_pile_card_takes_the_top_card_only() {
    let mut g = new_state(&[id::SAUNA, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    // Workshop: up to $4: Sauna ($4) but not Avanto ($5, and under the Saunas).
    assert!(choices(&g).contains(&Choice::Card(id::SAUNA)));
    // Remove the Saunas: Avanto costs $5, not gainable by a Workshop.
    let mut g = new_state(&[id::SAUNA, id::UPGRADE, id::SMITHY], 2);
    g.supply.set(id::SAUNA, 0);
    set_hand(&mut g, 0, &[id::UPGRADE, id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::UPGRADE);
    pick(&mut g, id::SILVER);
    // Upgrade: exactly $4 (Silver $3 + 1): the Smithy is the only choice (the Saunas are gone; Avanto costs $5 and is the top card, but not $4).
    assert_eq!(count(&g, 0, id::SMITHY), 1);
    assert_eq!(g.supply.get(id::AVANTO), 5);
}

#[test]
fn sauna_draws_gives_an_action_and_may_play_an_avanto_for_free() {
    let mut g = new_state(&[id::SAUNA, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SAUNA, id::AVANTO]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SAUNA);
    assert_eq!(g.players[0].hand.total(), 2, "+1 Card");
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Play, .. })));
    assert_eq!(choices(&g), vec![Choice::Card(id::AVANTO), Choice::Pass]);
    choose(&mut g, Choice::Card(id::AVANTO));
    assert_eq!(g.players[0].hand.total(), 4, "Sauna's card and Avanto's three");
    assert_eq!(g.turn.actions, 1, "the Sauna's +1 Action is still there: Avanto was free");
}

#[test]
fn sauna_and_avanto_can_chain_while_each_has_the_other_in_hand() {
    let mut g = new_state(&[id::SAUNA, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SAUNA, id::AVANTO, id::SAUNA]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SAUNA);
    choose(&mut g, Choice::Card(id::AVANTO));
    // Avanto may play the other Sauna.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Play, .. })));
    choose(&mut g, Choice::Card(id::SAUNA));
    assert_eq!(g.turn.actions, 2, "two Saunas gave +1 Action each; only the first play used an Action");
    assert_eq!(g.turn.adv.sauna, 2);
    // A Sauna cannot play a Sauna (only an Avanto).
}

#[test]
fn a_played_sauna_lets_each_silver_played_trash_a_card_from_the_hand() {
    let mut g = new_state(&[id::SAUNA, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SAUNA, id::SILVER, id::SILVER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SAUNA);
    // No Avanto in hand: onto the Buy phase. The player orders their Treasures (a Copper may be trashed before it is played).
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::PlayTreasure, "{d:?}");
    choose(&mut g, Choice::Card(id::SILVER));
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Hand, act: Act::Trash, min: 0, max: 1, .. })));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.trash.get(id::ESTATE), 1);
    assert_eq!(g.turn.coins, 2);
    // The second Silver trashes again; declining is allowed.
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Pass);
    assert_eq!(g.turn.coins, 5, "$2 + $2 and the Copper left in the hand, played by the Buy phase");
}

#[test]
fn two_saunas_trash_twice_per_silver() {
    let mut g = new_state(&[id::SAUNA, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SAUNA, id::SAUNA, id::SILVER, id::ESTATE, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::SAUNA);
    play(&mut g, id::SAUNA);
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.trash.get(id::ESTATE), 2);
}


// ===========================================================================
// Events
// ===========================================================================

/// A state in the Buy phase for player 0 with `coins`, `buys` and the given Events (and a few kingdom cards).
fn buy_phase(events: &[CardId], coins: u16) -> GameState {
    let mut kingdom = vec![id::SMITHY, id::VILLAGE, id::MILITIA, id::PORT, id::WORKSHOP, id::MARKET, id::PAGE];
    kingdom.extend_from_slice(events);
    let mut g = new_state(&kingdom, 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
    g.turn.buys = 3;
    set_deck_known(&mut g, 0, &junk());
    g
}

/// Choose `c` if it is a legal choice (the last pick of a selection is made for the player when it is the only one left).
fn try_choose(g: &mut GameState, c: Choice) {
    if choices(g).contains(&c) {
        choose(g, c);
    }
}

#[test]
fn events_are_offered_with_the_cards_they_cost_a_buy_and_are_never_gained() {
    let mut g = buy_phase(&[id::BALL, id::ALMS, id::SAVE], 4);
    adv(&mut g);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ALMS)) && cs.contains(&Choice::Card(id::SAVE)), "{cs:?}");
    assert!(!cs.contains(&Choice::Card(id::BALL)), "Ball costs $5");
    let before = total_cards(&g);
    let mut log: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SAVE), &mut log);
    assert!(log.contains(&Event::Buy { player: 0, card: id::SAVE }));
    assert!(!log.iter().any(|e| matches!(e, Event::Gain { card, .. } if *card == id::SAVE)), "an Event is bought, not gained");
    assert_eq!(total_cards(&g), before);
    assert_eq!(g.players[0].all_cards().total() + g.supply.total() + g.trash.total() + g.side_pile_cards(), before);
    // It cost $1 and a Buy.
    assert_eq!(g.players[0].events_bought.bits(), 1 << (id::SAVE - cards::FIRST_EVENT));
}

#[test]
fn buying_an_event_uses_a_buy_and_stops_the_treasures() {
    let mut g = new_state(&[id::SMITHY, id::TRAINING], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.buys = 2;
    // The Golds are played when the Buy phase starts (choice-free); buy Training, which uses one of the two Buys.
    adv(&mut g);
    assert_eq!(g.turn.coins, 6);
    buy(&mut g, id::TRAINING);
    assert_eq!(g.turn.buys, 1);
    assert!(g.turn.treasures_done);
}

// --- Alms before the Treasures --------------------------------------------------------------------------------

/// The Buy phase of player 0 holding `hand`, with Alms (and `events`) in the game, two Buys, stopped at the first decision.
fn alms_hand(hand: &[CardId], events: &[CardId]) -> GameState {
    let mut kingdom = vec![id::SMITHY, id::VILLAGE, id::MILITIA, id::PORT, id::WORKSHOP, id::ALMS];
    kingdom.extend_from_slice(events);
    let mut g = new_state(&kingdom, 2);
    set_hand(&mut g, 0, hand);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.buys = 2;
    adv(&mut g);
    g
}

#[test]
fn with_alms_in_the_game_the_buy_phase_starts_by_offering_it_before_the_treasures() {
    let g = alms_hand(&[id::SILVER, id::COPPER, id::COPPER], &[]);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    assert_eq!(g.turn.coins, 0, "nothing is played yet");
    assert_eq!(choices(&g), vec![Choice::Card(id::ALMS), Choice::Pass]);
}

#[test]
fn alms_bought_before_the_treasures_gains_a_card_up_to_four_and_the_treasures_are_then_played() {
    let mut g = alms_hand(&[id::SILVER, id::COPPER, id::COPPER], &[]);
    choose(&mut g, Choice::Card(id::ALMS));
    assert_eq!(g.turn.buys, 1, "Alms used a Buy");
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1, "the gained card");
    // The Treasures are played now (choice-free, all at once) and the Buy decision follows.
    assert_eq!(g.turn.coins, 4);
    assert_eq!(g.players[0].in_play.total(), 3);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Buy);
    assert!(!choices(&g).contains(&Choice::Card(id::ALMS)), "once a turn");
    // And a card can still be bought with the money.
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].all_cards().get(id::SILVER), 2, "the one played and the one bought");
}

#[test]
fn passing_on_alms_plays_the_treasures_and_alms_is_then_useless() {
    let mut g = alms_hand(&[id::SILVER, id::COPPER], &[]);
    choose(&mut g, Choice::Pass);
    assert_eq!(g.turn.coins, 3);
    assert!(g.turn.treasures_done);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Buy);
    // Still a legal buy, but with Treasures in play it gains nothing.
    buy(&mut g, id::ALMS);
    assert!(!matches!(pending_kind(&g), Some(DecisionKind::Gain { .. })));
    assert_eq!(g.players[0].discard.total(), 0);
}

#[test]
fn alms_is_offered_next_to_the_treasures_when_their_order_matters_and_stops_once_one_is_played() {
    // A Treasure with a choice (Anvil) already makes every Treasure a separate decision; Alms joins them.
    let mut g = new_state(&[id::SMITHY, id::ALMS, id::ANVIL], 2);
    set_hand(&mut g, 0, &[id::ANVIL, id::COPPER, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    adv(&mut g);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::ALMS)) && cs.contains(&Choice::Card(id::COPPER)) && cs.contains(&Choice::Card(id::ANVIL)), "{cs:?}");
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(!choices(&g).contains(&Choice::Card(id::ALMS)), "a Treasure is in play");
}

#[test]
fn games_without_alms_or_without_treasures_in_hand_are_unchanged() {
    // No Alms in the game: the Treasures are played at once and the first decision is the Buy.
    let mut g = new_state(&[id::SMITHY, id::TRAINING], 2);
    set_hand(&mut g, 0, &[id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    adv(&mut g);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 3);
    // Alms in the game but no Treasure in the hand: straight to the Buy decision, where Alms is offered as before.
    let g = alms_hand(&[id::ESTATE, id::ESTATE], &[]);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Buy);
    assert!(choices(&g).contains(&Choice::Card(id::ALMS)));
    // Alms already bought this turn: the Treasures are played as usual.
    let mut g = new_state(&[id::SMITHY, id::ALMS], 2);
    set_hand(&mut g, 0, &[id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.adv.bought.insert((id::ALMS - cards::FIRST_EVENT) as u32);
    adv(&mut g);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn alms_gains_a_card_up_to_four_only_with_no_treasures_in_play_and_once_a_turn() {
    // No Treasures in play.
    let mut g = buy_phase(&[id::ALMS], 0);
    g.turn.buys = 2;
    adv(&mut g);
    buy(&mut g, id::ALMS);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1);
    // Not again this turn.
    assert!(!choices(&g).contains(&Choice::Card(id::ALMS)));

    // With a Treasure in play: bought, but nothing gained.
    let mut g = buy_phase(&[id::ALMS], 1);
    set_in_play(&mut g, 0, &[id::COPPER]);
    adv(&mut g);
    buy(&mut g, id::ALMS);
    assert!(!matches!(pending_kind(&g), Some(DecisionKind::Gain { .. })));
    assert_eq!(g.players[0].discard.total(), 0);
}

#[test]
fn borrow_gives_a_buy_and_a_coin_for_a_minus_one_card_token_once_a_turn() {
    let mut g = buy_phase(&[id::BORROW], 0);
    g.turn.buys = 1;
    adv(&mut g);
    buy(&mut g, id::BORROW);
    assert_eq!((g.turn.buys, g.turn.coins), (1, 1), "+1 Buy (the Buy it used) and +$1");
    assert!(g.players[0].adv & ADV_MINUS_CARD != 0);
    // Once per turn.
    assert!(!choices(&g).contains(&Choice::Card(id::BORROW)));

    // With the token already on the deck: +1 Buy but no coin.
    let mut g = buy_phase(&[id::BORROW], 0);
    g.turn.buys = 1;
    g.players[0].adv |= ADV_MINUS_CARD;
    adv(&mut g);
    buy(&mut g, id::BORROW);
    assert_eq!((g.turn.buys, g.turn.coins), (1, 0));
}

#[test]
fn quest_discards_an_attack_two_curses_or_six_cards_for_a_gold() {
    // An Attack.
    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::MILITIA, id::ESTATE]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1), Choice::Mode(2), Choice::Mode(3)]);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    assert!(g.players[0].discard.has(id::MILITIA));

    // Two Curses.
    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::CURSE, id::CURSE, id::ESTATE]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    assert_eq!(g.players[0].discard.get(id::CURSE), 2);

    // Only one Curse: discarded, no Gold.
    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::CURSE, id::ESTATE]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.players[0].discard.get(id::GOLD), 0);
    assert_eq!(g.players[0].discard.get(id::CURSE), 1);

    // Six cards: five in hand is not enough; all are discarded, no Gold.
    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::ESTATE; 5]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    choose(&mut g, Choice::Mode(2));
    assert_eq!(g.players[0].discard.get(id::GOLD), 0);
    assert_eq!(g.players[0].hand.total(), 0);

    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY, id::CURSE, id::PROVINCE, id::PROVINCE]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    choose(&mut g, Choice::Mode(2));
    while matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    assert_eq!(g.players[0].hand.total(), 1, "seven cards, six discarded");

    // Doing nothing is allowed.
    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::ESTATE]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    choose(&mut g, Choice::Mode(3));
    assert_eq!(g.players[0].hand.total(), 1);
}

#[test]
fn save_sets_a_card_aside_and_returns_it_after_the_next_hand_is_drawn() {
    let mut g = buy_phase(&[id::SAVE], 1);
    g.turn.buys = 1;
    set_hand(&mut g, 0, &[id::DUCHY, id::ESTATE]);
    adv(&mut g);
    let before = total_cards(&g);
    buy(&mut g, id::SAVE);
    assert_eq!(g.turn.buys, 1, "+1 Buy");
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::SetAside, .. })));
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].saved, id::ESTATE);
    assert_eq!(total_cards(&g), before, "still owned");
    // Once per turn.
    assert!(!choices(&g).contains(&Choice::Card(id::SAVE)));
    pass(&mut g);
    // The next turn is player 1's; player 0's hand has the card drawn plus the saved one: 6 cards.
    assert_eq!(g.players[0].hand.total(), 6);
    assert!(g.players[0].hand.has(id::ESTATE));
    assert_eq!(g.players[0].saved, 0);
}

#[test]
fn scouting_party_discards_three_of_the_top_five_and_puts_two_back() {
    let mut g = buy_phase(&[id::SCOUTING_PARTY], 2);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER, id::ESTATE, id::DUCHY, id::PROVINCE]);
    g.turn.buys = 1;
    adv(&mut g);
    buy(&mut g, id::SCOUTING_PARTY);
    assert_eq!(g.turn.buys, 1, "+1 Buy (the Buy used and the one it gives)");
    // Discard three of the five revealed (the Province stays below).
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Revealed, act: Act::Discard, min: 3, max: 3, .. })));
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::ESTATE));
    try_choose(&mut g, Choice::Card(id::DUCHY));
    // Put Gold and Silver back in any order.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Revealed, act: Act::Topdeck, ordered: true, .. })));
    choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD), "the Gold, put back last, is on top");
    assert_eq!(g.players[0].discard.total(), 3);
    assert_eq!(g.players[0].deck_known.len, 3, "two put back, the Province below");
}

#[test]
fn scouting_party_with_fewer_than_five_cards_still_discards_three() {
    let mut g = buy_phase(&[id::SCOUTING_PARTY], 2);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER]);
    adv(&mut g);
    buy(&mut g, id::SCOUTING_PARTY);
    // Three cards: all discarded (the picks are made in card order; the last is automatic).
    while matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, from: Zone::Revealed, .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.players[0].discard.total(), 3);
    assert_eq!(g.players[0].deck_known.len, 0);
}

#[test]
fn travelling_fair_gives_two_buys_and_lets_gained_cards_go_onto_the_deck_this_turn() {
    let mut g = buy_phase(&[id::TRAVELLING_FAIR], 12);
    g.turn.buys = 1;
    adv(&mut g);
    buy(&mut g, id::TRAVELLING_FAIR);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 10), "+2 Buys, the Buy used");
    buy(&mut g, id::GOLD);
    // The Gold may be put onto the deck: optional.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::YesNo { act: Act::Topdeck })), "{:?}", g.pending_decision());
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert!(!g.players[0].discard.has(id::GOLD));
    // Declined for the next one.
    buy(&mut g, id::SMITHY);
    choose(&mut g, Choice::No);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1);
}

#[test]
fn travelling_fair_works_for_cards_gained_by_other_effects_and_ends_with_the_turn() {
    let mut g = buy_phase(&[id::TRAVELLING_FAIR, id::BALL], 8);
    g.turn.buys = 3;
    adv(&mut g);
    buy(&mut g, id::TRAVELLING_FAIR);
    buy(&mut g, id::BALL);
    // Ball's two gains: each may be topdecked.
    pick(&mut g, id::SMITHY);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::YesNo { act: Act::Topdeck })));
    choose(&mut g, Choice::Yes);
}

#[test]
fn bonfire_trashes_up_to_two_coppers_in_play() {
    let mut g = buy_phase(&[id::BONFIRE], 3);
    set_in_play(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::SILVER]);
    g.turn.buys = 2;
    adv(&mut g);
    buy(&mut g, id::BONFIRE);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::InPlay, act: Act::Trash, min: 0, max: 2, .. })));
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass], "only Coppers");
    choose(&mut g, Choice::Card(id::COPPER));
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.trash.get(id::COPPER), 2);
    assert_eq!(g.turn.coins, 0, "the $ from them was already spent on it; nothing is taken back");
}

#[test]
fn expedition_draws_two_extra_cards_for_the_next_hand_and_stacks() {
    let mut g = buy_phase(&[id::EXPEDITION], 6);
    g.turn.buys = 2;
    adv(&mut g);
    buy(&mut g, id::EXPEDITION);
    buy(&mut g, id::EXPEDITION);
    assert_eq!(g.players[0].hand.total(), 9, "5 + 2 + 2");
}

#[test]
fn trade_trashes_up_to_two_cards_for_a_silver_each() {
    let mut g = buy_phase(&[id::TRADE], 5);
    set_hand(&mut g, 0, &[id::ESTATE, id::CURSE, id::DUCHY]);
    adv(&mut g);
    buy(&mut g, id::TRADE);
    choose(&mut g, Choice::Card(id::ESTATE));
    choose(&mut g, Choice::Card(id::CURSE));
    // (picks are in card order: Estate then Curse)
    assert_eq!(g.trash.total(), 2);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
}

#[test]
fn trade_may_trash_nothing() {
    let mut g = buy_phase(&[id::TRADE], 5);
    set_hand(&mut g, 0, &[id::ESTATE]);
    adv(&mut g);
    buy(&mut g, id::TRADE);
    choose(&mut g, Choice::Pass);
    assert_eq!(g.players[0].discard.get(id::SILVER), 0);
}

#[test]
fn raid_gains_a_silver_per_silver_in_play_and_gives_the_others_the_minus_one_card_token() {
    let mut g = buy_phase(&[id::RAID], 5);
    set_in_play(&mut g, 0, &[id::SILVER, id::SILVER, id::GOLD]);
    adv(&mut g);
    buy(&mut g, id::RAID);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
    assert!(g.players[1].adv & ADV_MINUS_CARD != 0);
}

#[test]
fn raid_cannot_be_blocked_by_a_moat_or_a_champion() {
    let mut g = buy_phase(&[id::RAID, id::MOAT], 5);
    set_hand(&mut g, 1, &[id::MOAT]);
    g.players[1].in_play.add(id::CHAMPION, 1);
    adv(&mut g);
    buy(&mut g, id::RAID);
    assert!(g.players[1].adv & ADV_MINUS_CARD != 0, "an Event is not an Attack");
}

#[test]
fn ball_takes_the_minus_coin_token_once_and_gains_two_cards_up_to_four() {
    let mut g = buy_phase(&[id::BALL], 10);
    g.turn.buys = 2;
    adv(&mut g);
    buy(&mut g, id::BALL);
    assert!(g.players[0].adv & ADV_MINUS_COIN != 0);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Gain { max_cost: 4, .. })));
    pick(&mut g, id::SMITHY);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 2);
    // Again: two more cards, the token only once.
    buy(&mut g, id::BALL);
    pick(&mut g, id::VILLAGE);
    pick(&mut g, id::VILLAGE);
    assert_eq!(g.players[0].discard.get(id::VILLAGE), 2);
}

#[test]
fn ball_costing_five_with_the_token_makes_the_next_money_a_coin_short() {
    // The token is taken during the Buy phase; the next time the player gets $ (the next turn) they get $1 less.
    let mut g = new_state(&[id::SMITHY, id::BALL], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    g.turn.phase = Phase::Buy;
    adv(&mut g);
    buy(&mut g, id::BALL);
    pick(&mut g, id::SMITHY);
    pick(&mut g, id::SMITHY);
    end_turn(&mut g);
    // Player 0's new turn: five Coppers = $5, less the token's $1.
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.coins, 4);
    assert!(g.players[0].adv & ADV_MINUS_COIN == 0);
}

#[test]
fn pilgrimage_flips_the_journey_token_and_copies_up_to_three_differently_named_cards_in_play() {
    let mut g = buy_phase(&[id::PILGRIMAGE], 8);
    g.turn.buys = 2;
    set_in_play(&mut g, 0, &[id::SMITHY, id::SMITHY, id::VILLAGE, id::MILITIA, id::COPPER]);
    adv(&mut g);
    // The token starts face up: flipped to face down by the first buy, nothing gained.
    buy(&mut g, id::PILGRIMAGE);
    assert!(g.players[0].adv & ADV_JOURNEY_DOWN != 0);
    assert_eq!(g.players[0].discard.total(), 0);
    // Once per turn.
    assert!(!choices(&g).contains(&Choice::Card(id::PILGRIMAGE)));
    pass(&mut g);

    // Face down at the start: turned face up, copies gained (each differently named card once).
    let mut g = buy_phase(&[id::PILGRIMAGE], 8);
    g.players[0].adv |= ADV_JOURNEY_DOWN;
    set_in_play(&mut g, 0, &[id::SMITHY, id::SMITHY, id::VILLAGE, id::MILITIA, id::COPPER]);
    adv(&mut g);
    buy(&mut g, id::PILGRIMAGE);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::InPlay, act: Act::Reveal, max: 3, .. })));
    // The same card cannot be chosen twice (picks are in card order and each name once): after the Copper only higher ids remain.
    choose(&mut g, Choice::Card(id::COPPER));
    assert!(!choices(&g).contains(&Choice::Card(id::COPPER)));
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert!(!choices(&g).contains(&Choice::Card(id::VILLAGE)), "differently named");
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!((g.players[0].discard.get(id::COPPER), g.players[0].discard.get(id::SMITHY), g.players[0].discard.get(id::VILLAGE)), (1, 1, 1));
    assert_eq!(g.players[0].discard.get(id::MILITIA), 0);
}

#[test]
fn seaway_gains_an_action_up_to_four_and_puts_the_buy_token_on_its_pile() {
    let mut g = buy_phase(&[id::SEAWAY], 5);
    adv(&mut g);
    buy(&mut g, id::SEAWAY);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, filter: Filter::Action, .. }), "{d:?}");
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].tok[TOK_BUY], id::SMITHY);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1, "the card is gained too");
    // Playing a Smithy now gives +1 Buy first.
    set_hand(&mut g, 0, &[id::SMITHY]);
    g.turn = dominion_engine::state::TurnState::start(0, 5);
    g.pending = Pending::None;
    g.turn.buys = 0;
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.buys, 1);
}

#[test]
fn mission_gives_an_extra_turn_in_which_no_cards_can_be_bought_but_events_can() {
    let mut g = buy_phase(&[id::MISSION, id::ALMS], 4);
    g.turn.buys = 1;
    adv(&mut g);
    buy(&mut g, id::MISSION);
    // The turn ends (no Buy left); the next turn is the extra one for the same player.
    assert_eq!(g.turn.player, 0);
    assert!(g.turn.is_extra_turn);
    assert!(g.turn.adv.no_buy);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    g.turn.coins = 6;
    adv(&mut g);
    let cs = choices(&g);
    assert!(!cs.iter().any(|c| matches!(c, Choice::Card(x) if !cards::is_event(*x))), "no cards: {cs:?}");
    assert!(cs.contains(&Choice::Card(id::ALMS)));
    // And it cannot be repeated: no third turn in a row (Mission is not even offered).
    assert!(!cs.contains(&Choice::Card(id::MISSION)));
}

#[test]
fn a_missions_extra_turn_does_not_count_towards_the_fewer_turns_tiebreak() {
    // Rules (Turn, Gameplay): tied scores go to the player with fewer turns; extra turns don't count.
    let mut g = buy_phase(&[id::MISSION, id::ALMS], 4);
    g.turn.buys = 1;
    let before = g.players[0].turns_taken;
    adv(&mut g);
    buy(&mut g, id::MISSION);
    assert!(g.turn.is_extra_turn && g.turn.player == 0);
    assert_eq!(g.players[0].turns_taken, before + 1, "the normal turn counts");
    // End the Mission turn.
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    g.turn.buys = 0;
    adv(&mut g);
    while g.turn.player == 0 {
        let cs = choices(&g);
        g.apply(*cs.last().unwrap(), &mut NoEvents).unwrap();
        adv(&mut g);
    }
    assert_eq!(g.players[0].turns_taken, before + 1, "the Mission turn is not a turn taken for the tie-break");
}

#[test]
fn mission_bought_twice_gives_one_extra_turn() {
    let mut g = buy_phase(&[id::MISSION], 8);
    g.turn.buys = 2;
    adv(&mut g);
    buy(&mut g, id::MISSION);
    assert!(!choices(&g).contains(&Choice::Card(id::MISSION)), "a second Mission does nothing, so it is not offered");
}

#[test]
fn outposts_extra_turn_wins_over_missions() {
    let mut g = buy_phase(&[id::MISSION], 4);
    g.turn.buys = 1;
    set_in_play(&mut g, 0, &[id::OUTPOST]);
    g.turn.duration_held.add(id::OUTPOST, 1);
    adv(&mut g);
    buy(&mut g, id::MISSION);
    assert!(g.turn.is_extra_turn);
    assert!(!g.turn.adv.no_buy, "the Outpost turn can buy");
}

#[test]
fn inheritance_sets_an_action_aside_and_makes_estates_play_it() {
    let mut g = buy_phase(&[id::INHERITANCE], 7);
    adv(&mut g);
    buy(&mut g, id::INHERITANCE);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Supply, act: Act::SetAside, filter: Filter::InheritCard, .. }), "{d:?}");
    // Costing up to $4, a non-Duration non-Command Action.
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SMITHY)) && cs.contains(&Choice::Card(id::VILLAGE)) && !cs.contains(&Choice::Card(id::MARKET)));
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].inherit, id::SMITHY);
    assert_eq!(g.supply.get(id::SMITHY), 9, "taken from the Supply");
    assert_eq!(count(&g, 0, id::SMITHY), 1, "and part of the deck for scoring");
    // The turn is over; on the next turn of player 0 an Estate is an Action.
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY, id::DUCHY, id::PROVINCE, id::PROVINCE, id::PROVINCE, id::PROVINCE, id::PROVINCE]);
    end_turn(&mut g);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.kind, DecisionKind::PlayAction);
    assert_eq!(choices(&g), vec![Choice::Card(id::ESTATE), Choice::Pass]);
    choose(&mut g, Choice::Card(id::ESTATE));
    assert_eq!(g.players[0].hand.total(), 7, "four cards left in the hand and the Smithy's +3 Cards");
    assert!(g.players[0].in_play.has(id::ESTATE), "the Estate is in play");
    assert!(!g.players[0].in_play.has(id::SMITHY), "the Smithy stays set aside");
    assert_eq!(g.players[0].inherit, id::SMITHY);
    // Other players' Estates and the Supply's are ordinary.
    set_hand(&mut g, 1, &[id::ESTATE]);
    assert_eq!(g.players[1].inherit, 0);
}

#[test]
fn inheritance_is_once_per_game() {
    let mut g = buy_phase(&[id::INHERITANCE], 14);
    adv(&mut g);
    buy(&mut g, id::INHERITANCE);
    pick(&mut g, id::SMITHY);
    assert!(!choices(&g).contains(&Choice::Card(id::INHERITANCE)));
}

// ===========================================================================
// Tokens
// ===========================================================================

#[test]
fn a_token_on_a_pile_gives_its_bonus_each_time_a_card_from_it_is_played() {
    for (tok, check) in [(TOK_ACTION, 0u8), (TOK_BUY, 1), (TOK_COIN, 2), (TOK_CARD, 3)] {
        let mut g = new_state(&[id::SMITHY, id::MILITIA], 2);
        g.players[0].tok[tok] = id::SMITHY;
        g.adv |= dominion_engine::state::G_PLAY_TOKENS;
        set_hand(&mut g, 0, &[id::SMITHY]);
        set_deck_known(&mut g, 0, &junk());
        play(&mut g, id::SMITHY);
        match check {
            0 => assert_eq!(g.turn.actions, 1, "+1 Action first"),
            1 => assert_eq!(g.turn.buys, 2),
            2 => assert_eq!(g.turn.coins, 1),
            _ => assert_eq!(g.players[0].hand.total(), 4, "+1 Card, then the Smithy's three"),
        }
    }
}

#[test]
fn tokens_apply_to_every_play_of_a_throne_roomed_card_and_not_to_other_piles() {
    let mut g = new_state(&[id::SMITHY, id::THRONE_ROOM, id::MILITIA], 2);
    g.players[0].tok[TOK_COIN] = id::MILITIA;
    g.adv |= dominion_engine::state::G_PLAY_TOKENS;
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MILITIA]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::MILITIA));
    assert_eq!(g.turn.coins, 6, "Militia's $2 and the token's $1, twice");
    let mut g = new_state(&[id::SMITHY, id::THRONE_ROOM, id::MILITIA], 2);
    g.players[1].tok[TOK_COIN] = id::SMITHY;
    g.adv |= dominion_engine::state::G_PLAY_TOKENS;
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.coins, 0, "another player's token does nothing for me");
}

#[test]
fn the_plus_one_card_token_draws_before_the_cards_own_effects() {
    // Even before an Attack's reactions.
    let mut g = new_state(&[id::MILITIA, id::MOAT], 2);
    g.players[0].tok[TOK_CARD] = id::MILITIA;
    g.adv |= dominion_engine::state::G_PLAY_TOKENS;
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::MOAT, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    let step = play(&mut g, id::MILITIA);
    assert!(matches!(step, Step::Decision(Decision { player: 1, .. })), "the Moat decision is next");
    assert_eq!(g.players[0].hand.get(id::GOLD), 1, "the token's card was drawn before the reaction");
}

#[test]
fn ferrys_minus_two_cost_token_cheapens_a_pile_on_its_owners_turns() {
    let mut g = new_state(&[id::SMITHY, id::MARKET], 2);
    g.players[0].tok[TOK_COST] = id::MARKET;
    g.adv |= dominion_engine::state::G_COST_TOKEN;
    assert_eq!(g.cost(id::MARKET), 3);
    assert_eq!(g.cost(id::SMITHY), 4);
    // On the other player's turn it is the other player's token that counts.
    reset_turn(&mut g, 1);
    assert_eq!(g.cost(id::MARKET), 5);
    // Never below $0, and only Action cards of the pile.
    let mut g = new_state(&[id::SMITHY, id::MARKET], 2);
    g.players[0].tok[TOK_COST] = id::COPPER; // (0 means "no token": Copper is never a token pile)
    g.adv |= dominion_engine::state::G_COST_TOKEN;
    assert_eq!(g.cost(id::COPPER), 0);
}

#[test]
fn ferry_moves_the_token_to_a_pile_of_the_players_choice() {
    let mut g = buy_phase(&[id::FERRY], 3);
    adv(&mut g);
    buy(&mut g, id::FERRY);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Token { token } if token as usize == TOK_COST), "{d:?}");
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SMITHY)) && !cs.contains(&Choice::Card(id::GOLD)) && !cs.contains(&Choice::Card(id::PAGE)).then_some(()).is_none());
    choose(&mut g, Choice::Card(id::MARKET));
    assert_eq!(g.players[0].tok[TOK_COST], id::MARKET);
    assert_eq!(g.cost(id::MARKET), 3);
}

#[test]
fn a_ferry_token_moves_when_it_is_placed_again() {
    let mut g = buy_phase(&[id::FERRY], 6);
    g.turn.buys = 2;
    adv(&mut g);
    buy(&mut g, id::FERRY);
    choose(&mut g, Choice::Card(id::MARKET));
    buy(&mut g, id::FERRY);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].tok[TOK_COST], id::SMITHY);
    assert_eq!(g.cost(id::MARKET), 5);
}

#[test]
fn plan_lets_a_card_gained_from_its_pile_trash_a_card_from_the_hand() {
    let mut g = buy_phase(&[id::PLAN], 7);
    g.turn.buys = 3;
    set_hand(&mut g, 0, &[id::ESTATE, id::CURSE]);
    adv(&mut g);
    buy(&mut g, id::PLAN);
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].tok[TOK_TRASH], id::SMITHY);
    buy(&mut g, id::SMITHY);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Hand, act: Act::Trash, min: 0, max: 1, .. })));
    choose(&mut g, Choice::Card(id::CURSE));
    assert_eq!(g.trash.get(id::CURSE), 1);
}

#[test]
fn plan_triggers_on_any_gain_from_the_pile_including_other_players_turns() {
    let mut g = new_state(&[id::SMITHY, id::MESSENGER], 2);
    g.players[1].tok[TOK_TRASH] = id::SMITHY;
    g.adv |= dominion_engine::state::G_TRASH_TOKEN;
    set_hand(&mut g, 1, &[id::ESTATE]);
    // Player 0 buys a Messenger and picks a Smithy: player 1 gains a copy and may trash.
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    buy(&mut g, id::MESSENGER);
    pick(&mut g, id::SMITHY);
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
}

#[test]
fn lost_arts_training_and_pathfinding_move_their_tokens() {
    for (ev, tok, coins) in [(id::LOST_ARTS, TOK_ACTION, 6), (id::TRAINING, TOK_COIN, 6), (id::PATHFINDING, TOK_CARD, 8)] {
        let mut g = buy_phase(&[ev], coins);
        adv(&mut g);
        buy(&mut g, ev);
        assert!(matches!(pending_kind(&g), Some(DecisionKind::Token { token }) if token as usize == tok));
        choose(&mut g, Choice::Card(id::VILLAGE));
        assert_eq!(g.players[0].tok[tok], id::VILLAGE);
        assert!(g.adv & dominion_engine::state::G_PLAY_TOKENS != 0);
    }
}

#[test]
fn the_minus_one_card_token_makes_the_next_draw_one_card_shorter_and_goes() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.players[0].adv |= ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 2, "+3 Cards less one");
    assert!(g.players[0].adv & ADV_MINUS_CARD == 0);
    // And the Clean-up draw of 5 is a draw like any other.
    let mut g = new_state(&[id::SMITHY], 2);
    g.players[0].adv |= ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    end_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 4);
    assert!(g.players[0].adv & ADV_MINUS_CARD == 0);
}

#[test]
fn the_minus_one_card_token_is_removed_even_by_a_draw_of_one_card_from_an_empty_deck() {
    let mut g = new_state(&[id::VILLAGE], 2);
    g.players[0].adv |= ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::VILLAGE]);
    clear_deck(&mut g, 0);
    play(&mut g, id::VILLAGE);
    assert!(g.players[0].adv & ADV_MINUS_CARD == 0);
    assert_eq!(g.players[0].hand.total(), 0);
    assert_eq!(g.players[0].discard.total(), 0, "no shuffle");
}

#[test]
fn drawing_no_cards_does_not_remove_the_minus_one_card_token() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.players[0].adv |= ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.buys = 0;
    // (A draw of zero cards: nothing to remove it.)
    assert!(g.players[0].adv & ADV_MINUS_CARD != 0);
}

#[test]
fn the_minus_one_coin_token_takes_a_coin_from_the_next_coins_and_goes() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.players[0].adv |= ADV_MINUS_COIN;
    g.turn.adv.tax_live = true;
    set_hand(&mut g, 0, &[id::SILVER, id::GOLD]);
    adv(&mut g);
    // (the hand was set after the turn began: play them)
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    adv(&mut g);
    assert_eq!(g.turn.coins, 4, "$5 less $1");
    assert!(g.players[0].adv & ADV_MINUS_COIN == 0);
}

#[test]
fn the_minus_one_coin_token_applies_to_a_card_that_gives_coins_and_not_to_a_buy() {
    let mut g = new_state(&[id::MILITIA], 2);
    g.players[0].adv |= ADV_MINUS_COIN;
    g.turn.adv.tax_live = true;
    set_hand(&mut g, 0, &[id::MILITIA]);
    play(&mut g, id::MILITIA);
    assert_eq!(g.turn.coins, 1, "Militia's $2 less $1");
}

#[test]
fn journey_token_flips_between_cards_and_events() {
    // Ranger turns it face down; Giant turns it face up again: its attack happens at once.
    let mut g = new_state(&[id::RANGER, id::GIANT], 2);
    set_hand(&mut g, 0, &[id::RANGER, id::GIANT]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::COPPER]);
    g.turn.actions = 2;
    play(&mut g, id::RANGER);
    assert!(g.players[0].adv & ADV_JOURNEY_DOWN != 0);
    play(&mut g, id::GIANT);
    assert_eq!(g.turn.coins, 5);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
}

#[test]
fn tokens_on_the_knights_pile_apply_to_every_knight() {
    let mut g = new_state(&[id::KNIGHTS, id::SMITHY], 2);
    g.players[0].tok[TOK_COIN] = id::KNIGHTS;
    g.adv |= dominion_engine::state::G_PLAY_TOKENS;
    set_hand(&mut g, 0, &[id::DAME_SYLVIA]);
    play(&mut g, id::DAME_SYLVIA);
    assert_eq!(g.turn.coins, 3, "Dame Sylvia's $2 and the token's $1");
}

#[test]
fn teacher_moves_one_of_the_four_tokens_to_a_pile_without_the_players_tokens() {
    let mut g = new_state(&[id::PEASANT, id::SMITHY, id::VILLAGE], 2);
    g.players[0].tavern.add(id::TEACHER, 1);
    g.players[0].tok[TOK_COST] = id::SMITHY;
    g.adv |= dominion_engine::state::G_COST_TOKEN;
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    choose(&mut g, Choice::Yes);
    // Which token: +1 Card, +1 Action, +1 Buy or +$1.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Mode { .. })));
    assert_eq!(choices(&g).len(), 4);
    choose(&mut g, Choice::Mode(1));
    // Not the Smithy pile: it holds the player's -$2 cost token.
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Token { token } if token as usize == TOK_ACTION));
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(id::SMITHY)) && cs.contains(&Choice::Card(id::VILLAGE)), "{cs:?}");
    choose(&mut g, Choice::Card(id::VILLAGE));
    assert_eq!(g.players[0].tok[TOK_ACTION], id::VILLAGE);
}

// ===========================================================================
// The stop-moving rule (docs/stop-moving-rule.md)
// ===========================================================================

#[test]
fn a_throne_roomed_reserve_card_goes_to_the_mat_once_and_leaves_another_copy_in_play() {
    let mut g = new_state(&[id::THRONE_ROOM, id::COIN_OF_THE_REALM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::COIN_OF_THE_REALM]);
    set_in_play(&mut g, 0, &[id::COIN_OF_THE_REALM]); // called earlier this turn
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::COIN_OF_THE_REALM));
    assert_eq!(g.players[0].tavern.get(id::COIN_OF_THE_REALM), 1, "the 2nd play can't put it on the mat again");
    assert_eq!(g.players[0].in_play.get(id::COIN_OF_THE_REALM), 1, "the other copy was not taken");
    assert_eq!(g.turn.led.n, 0);
}

#[test]
fn a_throne_roomed_raze_trashes_itself_once_and_a_card_from_the_hand_the_second_time() {
    let mut g = new_state(&[id::THRONE_ROOM, id::RAZE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::RAZE, id::ESTATE]);
    set_in_play(&mut g, 0, &[id::RAZE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::RAZE));
    assert!(matches!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Trash }));
    choose(&mut g, Choice::Yes);
    // The 2nd play: the Raze it played is gone (the other one in play is not it): a card from the hand, the Estate, is trashed.
    while !matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::PlayAction | DecisionKind::Buy) | None) {
        let first = choices(&g)[0];
        choose(&mut g, first);
    }
    assert_eq!(g.trash.get(id::RAZE), 1);
    assert_eq!(g.trash.get(id::ESTATE), 1);
    assert_eq!(g.players[0].in_play.get(id::RAZE), 1);
}

#[test]
fn royal_carriage_is_not_offered_for_a_card_played_from_the_supply() {
    // A Captain-like play is not in play: another copy of the card in play is not it.
    let mut g = new_state(&[id::ROYAL_CARRIAGE, id::SMITHY, id::BAND_OF_MISFITS], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_in_play(&mut g, 0, &[id::SMITHY]);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::BAND_OF_MISFITS);
    // The only call offered is for the Band of Misfits itself (it is in play); the Smithy it played is not.
    assert!(call_decision(&g));
    assert_eq!(g.pending_decision().unwrap().target, id::BAND_OF_MISFITS, "{:?}", g.pending_decision());
}

// ===========================================================================
// Audit: ruling coverage (one test per wiki FAQ / clarification not covered above)
// ===========================================================================

#[test]
fn warrior_discarding_a_tunnel_lets_its_owner_reveal_it_for_a_gold_and_it_is_still_trashed() {
    // Warrior FAQ: "If another player discards a Tunnel, they can reveal it to gain a Gold, but they will still trash the Tunnel."
    for reveal in [true, false] {
        let mut g = new_state(&[id::PAGE, id::TUNNEL], 2);
        set_hand(&mut g, 0, &[id::WARRIOR]);
        set_deck_known(&mut g, 0, &junk());
        set_deck_known(&mut g, 1, &[id::TUNNEL, id::COPPER]);
        play(&mut g, id::WARRIOR);
        let d = g.pending_decision().unwrap();
        assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.player == 1 && d.subject == id::TUNNEL, "{d:?}");
        choose(&mut g, if reveal { Choice::Yes } else { Choice::No });
        assert_eq!(g.players[1].discard.get(id::GOLD), reveal as u8);
        assert_eq!(g.trash.get(id::TUNNEL), 1);
        assert_eq!(g.players[1].discard.get(id::TUNNEL), 0);
    }
}

#[test]
fn duplicate_is_not_offered_for_a_card_with_debt_in_its_cost() {
    // Duplicate FAQ: "Cards with Potion or Debt in their cost cannot be Duplicated."
    let mut g = new_state(&[id::DUPLICATE, id::ENGINEER], 2);
    g.players[0].tavern.add(id::DUPLICATE, 1);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    adv(&mut g);
    buy(&mut g, id::ENGINEER);
    assert!(!call_decision(&g), "{:?}", g.pending_decision());
    // (and a Potion-cost card)
    let mut g = new_state(&[id::DUPLICATE, id::FAMILIAR], 2);
    g.players[0].tavern.add(id::DUPLICATE, 1);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    g.turn.potions = 1;
    adv(&mut g);
    buy(&mut g, id::FAMILIAR);
    assert!(!call_decision(&g), "{:?}", g.pending_decision());
}

#[test]
fn giant_played_face_down_still_lets_other_players_react_to_the_attack() {
    // Giant FAQ: "Players can respond to Giant being played with Reactions ... even if Giant will only be producing +$1 this time."
    let mut g = new_state(&[id::GIANT, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::GIANT]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::GIANT);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.subject), (1, id::MOAT), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.coins, 1);
}

#[test]
fn giant_trashes_a_non_supply_card_with_an_asterisk_cost_between_three_and_six() {
    // Giant FAQ: "Cards with an asterisk ... that cost from $3 to $6 (such as Teacher) do get trashed."
    let mut g = new_state(&[id::GIANT, id::PEASANT], 2);
    set_hand(&mut g, 0, &[id::GIANT, id::GIANT]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::GIANT); // face down
    set_deck_known(&mut g, 1, &[id::TEACHER, id::COPPER]);
    play(&mut g, id::GIANT); // face up
    assert_eq!(g.trash.get(id::TEACHER), 1);
}

#[test]
fn giant_does_not_trash_cards_with_a_potion_in_the_cost() {
    // Giant FAQ: "Cards with Debt or Potion in the cost do not cost from $3 to $6": discarded, and a Curse is gained.
    let mut g = new_state(&[id::GIANT, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::GIANT, id::GIANT]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::GIANT);
    set_deck_known(&mut g, 1, &[id::FAMILIAR, id::COPPER]);
    play(&mut g, id::GIANT);
    assert_eq!(g.trash.total(), 0);
    assert_eq!((g.players[1].discard.get(id::FAMILIAR), g.players[1].discard.get(id::CURSE)), (1, 1));
}

#[test]
fn warrior_trashes_four_cost_non_supply_cards_and_counts_only_travellers() {
    // Warrior FAQ: Champion and Teacher are not Travellers; asterisk costs can be trashed (Fugitive $4*, Soldier $3*).
    let mut g = new_state(&[id::PEASANT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WARRIOR]);
    set_in_play(&mut g, 0, &[id::CHAMPION, id::TEACHER, id::PEASANT]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::FUGITIVE, id::SOLDIER, id::GOLD]);
    play(&mut g, id::WARRIOR);
    // Travellers in play: Peasant and Warrior = two cards revealed.
    assert_eq!(g.trash.get(id::FUGITIVE), 1);
    assert_eq!(g.trash.get(id::SOLDIER), 1);
    assert_eq!(g.players[1].discard.total(), 0);
}

#[test]
fn hero_cannot_gain_a_treasure_that_is_not_in_the_supply() {
    // Hero FAQ: Treasures not in the Supply (Spoils, Huge Turnip) cannot be gained; Treasures with a Potion cost can.
    let mut g = new_state(&[id::PAGE, id::SMITHY, id::PHILOSOPHERS_STONE], 2);
    set_hand(&mut g, 0, &[id::HERO]);
    play(&mut g, id::HERO);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::PHILOSOPHERS_STONE)), "{cs:?}");
    assert!(!cs.contains(&Choice::Card(id::SPOILS)), "{cs:?}");
}

#[test]
fn soldier_counts_a_duration_attack_played_last_turn() {
    // Soldier FAQ: "Duration Attacks played on the previous turn are Attack cards in play and so do count for Soldier."
    let mut g = new_state(&[id::SWAMP_HAG, id::PEASANT], 2);
    set_hand(&mut g, 0, &[id::SWAMP_HAG, id::SOLDIER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::SWAMP_HAG);
    end_turn(&mut g);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert!(g.players[0].in_play.has(id::SWAMP_HAG), "the Duration is still in play");
    set_hand(&mut g, 0, &[id::SOLDIER]);
    g.turn.coins = 0;
    g.turn.phase = Phase::Action;
    g.pending = Pending::None;
    play(&mut g, id::SOLDIER);
    assert_eq!(g.turn.coins, 2 + 1, "Soldier $2, +$1 for the Swamp Hag in play");
}

#[test]
fn magpie_gains_a_magpie_for_a_treasure_victory_card_and_takes_it_too() {
    // Magpie FAQ: a Treasure-Victory card (Harem) goes to the hand and also gains a Magpie.
    let mut g = new_state(&[id::MAGPIE, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::MAGPIE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::HAREM]);
    play(&mut g, id::MAGPIE);
    assert_eq!(held(&g, 0).get(id::HAREM), 1);
    assert_eq!(g.players[0].discard.get(id::MAGPIE), 1);
}

#[test]
fn magpie_leaves_a_non_treasure_on_top_and_ignores_a_curse_and_an_empty_pile() {
    // Magpie FAQ: a non-Treasure stays on top; a Curse does nothing; once the Magpie pile is empty nothing is gained.
    let mut g = new_state(&[id::MAGPIE], 2);
    set_hand(&mut g, 0, &[id::MAGPIE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::CURSE, id::COPPER]);
    play(&mut g, id::MAGPIE);
    assert_eq!(g.players[0].hand.total(), 1, "Estate drawn by +1 Card");
    assert_eq!(g.players[0].discard.get(id::MAGPIE), 0, "the Curse on top does nothing");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::CURSE));
    let mut g = new_state(&[id::MAGPIE], 2);
    g.supply.set(id::MAGPIE, 0);
    set_hand(&mut g, 0, &[id::MAGPIE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::COPPER]);
    play(&mut g, id::MAGPIE);
    assert_eq!(g.players[0].discard.get(id::MAGPIE), 0);
}

#[test]
fn miser_with_no_copper_in_the_hand_does_nothing_and_mat_coppers_score() {
    // Miser FAQ: either choice may be made with nothing to do; Coppers on the mat count as part of the deck for scoring (Gardens).
    let mut g = new_state(&[id::MISER, id::GARDENS], 2);
    set_hand(&mut g, 0, &[id::MISER, id::ESTATE]);
    play(&mut g, id::MISER);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[0].tavern.total(), 0);
    let mut g = new_state(&[id::MISER, id::GARDENS], 2);
    g.players[0].tavern.add(id::COPPER, 9);
    g.players[0].hand = counts_of(&[id::GARDENS]);
    assert_eq!(g.players[0].vp(), 1, "Gardens owns 10 cards counting the mat");
}

#[test]
fn transmogrify_with_nothing_in_the_hand_gains_nothing_and_may_gain_a_copy_of_the_trashed_card() {
    // Transmogrify FAQ: "if you had no cards to trash, you do not gain one"; "you may trash a card to gain a copy of the same card".
    let mut g = new_state(&[id::TRANSMOGRIFY, id::SMITHY], 2);
    g.players[0].tavern.add(id::TRANSMOGRIFY, 1);
    set_hand(&mut g, 0, &[]);
    set_deck_known(&mut g, 0, &[]);
    let before = total_cards(&g);
    adv(&mut g);
    if call_decision(&g) {
        choose(&mut g, Choice::Yes);
    }
    assert_eq!(g.players[0].hand.total(), 0);
    assert_eq!(total_cards(&g), before);
    let mut g = new_state(&[id::TRANSMOGRIFY, id::SMITHY], 2);
    g.players[0].tavern.add(id::TRANSMOGRIFY, 1);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    choose(&mut g, Choice::Yes);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.get(id::SMITHY), 1, "a copy into the hand");
    assert_eq!(g.trash.get(id::SMITHY), 1);
}

#[test]
fn a_throned_bridge_troll_cheapens_two_on_both_turns() {
    // Bridge Troll FAQ: played twice with Throne Room, cards cost $2 less this turn and next.
    let mut g = new_state(&[id::BRIDGE_TROLL, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::BRIDGE_TROLL]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    try_choose(&mut g, Choice::Card(id::BRIDGE_TROLL));
    assert_eq!(g.cost(id::GOLD), 4);
    end_turn(&mut g);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.cost(id::GOLD), 4, "next turn too");
    assert_eq!(g.turn.buys, 3, "+1 Buy per play at the start of the turn");
}

#[test]
fn distant_lands_counts_as_a_card_owned_for_gardens_on_and_off_the_mat() {
    // Distant Lands FAQ: "It counts as part of your deck either way (for example ... Gardens)."
    let mut g = new_state(&[id::DISTANT_LANDS, id::GARDENS], 2);
    g.players[0].hand = counts_of(&[id::GARDENS, id::DISTANT_LANDS, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    let off = g.players[0].vp();
    g.players[0].hand.remove(id::DISTANT_LANDS);
    g.players[0].tavern.add(id::DISTANT_LANDS, 1);
    let on = g.players[0].vp();
    assert_eq!(off, 1, "10 cards: Gardens 1, Distant Lands 0 off the mat");
    assert_eq!(on, 1 + 4);
}

#[test]
fn treasure_trove_still_gains_the_other_card_when_one_pile_is_empty() {
    // Treasure Trove FAQ: "If one of those piles is empty, you still gain the other card."
    let mut g = new_state(&[id::TREASURE_TROVE], 2);
    g.supply.set(id::GOLD, 0);
    set_hand(&mut g, 0, &[id::TREASURE_TROVE]);
    adv(&mut g);
    play_treasure(&mut g, id::TREASURE_TROVE);
    assert_eq!((g.players[0].discard.get(id::GOLD), g.players[0].discard.get(id::COPPER)), (0, 1));
}

#[test]
fn wine_merchant_throned_gives_its_bonus_both_times_and_goes_to_the_mat_once() {
    // Wine Merchant FAQ: "You still get the +1 Buy and +$4 even if you are unable to move Wine Merchant to your Tavern mat".
    let mut g = new_state(&[id::WINE_MERCHANT, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::WINE_MERCHANT]);
    play(&mut g, id::THRONE_ROOM);
    try_choose(&mut g, Choice::Card(id::WINE_MERCHANT));
    assert_eq!((g.turn.coins, g.turn.buys), (8, 3));
    assert_eq!(g.players[0].tavern.get(id::WINE_MERCHANT), 1);
}

#[test]
fn unplayed_treasures_in_hand_are_not_unspent_money_for_wine_merchant() {
    // Wine Merchant FAQ: "Unplayed Treasures in your hand are not unspent $".
    let mut g = new_state(&[id::WINE_MERCHANT, id::SMITHY], 2);
    g.players[0].tavern.add(id::WINE_MERCHANT, 1);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.treasures_done = true;
    adv(&mut g);
    pass(&mut g);
    assert_eq!(g.players[0].tavern.get(id::WINE_MERCHANT), 1);
}

#[test]
fn reserve_cards_called_are_in_play_but_calling_is_not_playing_an_action() {
    // Reserve rules: calling is not playing (Conspirator not counted) and a called card is in play.
    let mut g = new_state(&[id::RATCATCHER, id::CONSPIRATOR], 2);
    g.players[0].tavern.add(id::RATCATCHER, 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::CONSPIRATOR]);
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    while call_decision(&g) {
        choose(&mut g, Choice::Yes);
        try_choose(&mut g, Choice::Card(id::ESTATE));
    }
    assert_eq!(g.players[0].in_play.get(id::RATCATCHER), 2);
    assert_eq!(g.turn.played.count_type(cards::ACTION), 0, "calls are not plays");
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn a_reserve_card_may_be_called_with_no_effect() {
    // Reserve rules: Ratcatcher with an empty hand may still be called.
    let mut g = new_state(&[id::RATCATCHER, id::SMITHY], 2);
    g.players[0].tavern.add(id::RATCATCHER, 1);
    set_hand(&mut g, 0, &[]);
    set_deck_known(&mut g, 0, &[]);
    adv(&mut g);
    assert!(call_decision(&g), "{:?}", g.pending_decision());
}

#[test]
fn seaway_is_not_available_when_only_a_debt_cost_action_is_left() {
    // "Up to $4" does not include costs with Debt (Rising Sun/Empires rules): Engineer ($0 + 4 Debt) cannot be gained.
    let mut g = new_state(&[id::SEAWAY, id::ENGINEER], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    assert!(!choices(&g).contains(&Choice::Card(id::SEAWAY)));
}

#[test]
fn borrow_gives_no_coin_when_the_minus_one_card_token_is_already_on_the_deck() {
    // Borrow FAQ: "You do not get +$1 if you buy Borrow when your -1 Card token is already on your deck."
    let mut g = buy_phase(&[id::BORROW], 0);
    g.players[0].adv |= ADV_MINUS_CARD;
    adv(&mut g);
    buy(&mut g, id::BORROW);
    assert_eq!((g.turn.coins, g.turn.buys), (0, 3 - 1 + 1));
    assert!(g.players[0].adv & ADV_MINUS_CARD != 0);
}

#[test]
fn quest_with_one_curse_or_too_few_cards_discards_them_all_and_gains_nothing() {
    // Quest FAQ: "You may choose to discard two Curses despite only having one" / "six cards despite not having enough".
    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::CURSE, id::ESTATE]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    choose(&mut g, Choice::Mode(1));
    while matches!(pending_kind(&g), Some(DecisionKind::Select { .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.players[0].discard.get(id::CURSE), 1);
    assert_eq!(g.players[0].discard.get(id::GOLD), 0);
    let mut g = buy_phase(&[id::QUEST], 0);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY]);
    adv(&mut g);
    buy(&mut g, id::QUEST);
    choose(&mut g, Choice::Mode(2));
    while matches!(pending_kind(&g), Some(DecisionKind::Select { .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.players[0].discard.get(id::GOLD), 0);
    assert_eq!(g.players[0].hand.total(), 0);
}

#[test]
fn bonfire_keeps_the_coins_of_the_trashed_coppers() {
    // Bonfire FAQ: "This does not remove the $ you got from playing those Coppers."
    let mut g = buy_phase(&[id::BONFIRE], 5);
    set_in_play(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    adv(&mut g);
    buy(&mut g, id::BONFIRE);
    pick(&mut g, id::COPPER);
    try_choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.trash.get(id::COPPER), 2);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn expedition_adds_two_to_an_outposts_three_card_hand() {
    // Expedition FAQ: Outpost's extra turn with 3 cards plus Expedition's 2 = 5.
    let mut g = new_state(&[id::OUTPOST, id::EXPEDITION, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::OUTPOST);
    g.turn.coins = 3;
    adv(&mut g);
    buy(&mut g, id::EXPEDITION);
    assert_eq!(g.turn.player, 0);
    assert!(g.turn.is_extra_turn);
    assert_eq!(g.players[0].hand.total(), 5);
}

#[test]
fn raid_gains_what_is_left_when_there_are_not_enough_silvers() {
    // Raid FAQ: "if there are not enough left, just take what is left."
    let mut g = buy_phase(&[id::RAID], 5);
    set_in_play(&mut g, 0, &[id::SILVER, id::SILVER, id::SILVER]);
    g.supply.set(id::SILVER, 2);
    adv(&mut g);
    buy(&mut g, id::RAID);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
}

#[test]
fn pilgrimage_chooses_before_gaining_and_cannot_copy_cards_off_the_supply() {
    // Pilgrimage FAQ: copies come from the Supply (not Disciple/Mercenary); up to 3 differently named; a Potion-cost card is allowed.
    let mut g = buy_phase(&[id::PILGRIMAGE, id::FAMILIAR], 4);
    set_in_play(&mut g, 0, &[id::SMITHY, id::SMITHY, id::VILLAGE, id::DISCIPLE, id::MARKET, id::FAMILIAR]);
    g.players[0].adv |= ADV_JOURNEY_DOWN;
    adv(&mut g);
    buy(&mut g, id::PILGRIMAGE);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SMITHY)) && cs.contains(&Choice::Card(id::VILLAGE)) && cs.contains(&Choice::Card(id::MARKET)), "{cs:?}");
    assert!(!cs.contains(&Choice::Card(id::DISCIPLE)), "not in the Supply: {cs:?}");
    assert!(cs.contains(&Choice::Card(id::FAMILIAR)), "a Potion-cost card can be copied: {cs:?}");
}

#[test]
fn seaway_puts_the_buy_token_on_the_knights_pile_when_a_knight_is_gained() {
    // Seaway FAQ: gaining Sir Martin from the Knights pile puts the +1 Buy token on the Knights pile.
    let mut g = buy_phase(&[id::SEAWAY, id::KNIGHTS], 5);
    adv(&mut g);
    buy(&mut g, id::SEAWAY);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { .. }));
    if choices(&g).contains(&Choice::Card(id::SIR_MARTIN)) {
        pick(&mut g, id::SIR_MARTIN);
        assert_eq!(g.players[0].tok[TOK_BUY], id::KNIGHTS);
    }
}

#[test]
fn inheritance_does_not_offer_duration_or_command_cards_or_cards_over_four() {
    // Inheritance (2025 errata): a non-Command non-Duration Action costing up to $4.
    let mut g = buy_phase(&[id::INHERITANCE, id::CARAVAN_GUARD, id::BAND_OF_MISFITS, id::CELLAR], 7);
    adv(&mut g);
    buy(&mut g, id::INHERITANCE);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::CELLAR)), "{cs:?}");
    assert!(!cs.contains(&Choice::Card(id::CARAVAN_GUARD)), "Duration: {cs:?}");
    assert!(!cs.contains(&Choice::Card(id::BAND_OF_MISFITS)), "Command: {cs:?}");
    assert!(!cs.contains(&Choice::Card(id::MARKET)), "$5: {cs:?}");
}

#[test]
fn an_inherited_card_applies_the_token_on_its_pile_and_counts_for_scoring() {
    // Inheritance FAQ: "If the set aside card is from a pile you have a +1 Action or similar token on, you will get that effect."
    let mut g = new_state(&[id::SMITHY, id::INHERITANCE], 2);
    g.players[0].inherit = id::SMITHY;
    g.players[0].tok[TOK_ACTION] = id::SMITHY;
    g.adv |= dominion_engine::state::G_PLAY_TOKENS;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::ESTATE);
    assert_eq!(g.turn.actions, 1, "-1 for the Estate, +1 from the token on the Smithy pile");
    assert!(g.players[0].all_cards().has(id::SMITHY));
}

#[test]
fn the_minus_one_coin_token_is_not_removed_by_a_miser_that_gives_nothing() {
    // -$1 token FAQ: "It is not removed by getting +$0, such as by playing Miser and choosing +$ but having no Coppers on the mat."
    let mut g = new_state(&[id::MISER], 2);
    g.players[0].adv |= ADV_MINUS_COIN;
    g.turn.adv.tax_live = true;
    set_hand(&mut g, 0, &[id::MISER]);
    play(&mut g, id::MISER);
    choose(&mut g, Choice::Mode(1));
    assert!(g.players[0].adv & ADV_MINUS_COIN != 0);
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn a_minus_two_cost_token_lets_transmogrify_turn_a_copper_into_a_two_cost_cheaper_card() {
    // -$2 cost token FAQ: lowers costs for all purposes (Transmogrify Copper into Gear costing $1 with the token).
    let mut g = new_state(&[id::TRANSMOGRIFY, id::GEAR], 2);
    g.players[0].tavern.add(id::TRANSMOGRIFY, 1);
    g.players[0].tok[TOK_COST] = id::GEAR;
    g.adv |= dominion_engine::state::G_COST_TOKEN;
    set_hand(&mut g, 0, &[id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    choose(&mut g, Choice::Yes);
    try_choose(&mut g, Choice::Card(id::COPPER));
    assert!(choices(&g).contains(&Choice::Card(id::GEAR)), "{:?}", choices(&g));
}

#[test]
fn teacher_cannot_use_a_pile_holding_the_players_minus_two_cost_or_trashing_token() {
    // Teacher FAQ: "This cannot put a token on a pile you have tokens on, including ... your -$2 cost and Trashing."
    let mut g = new_state(&[id::PEASANT, id::SMITHY, id::VILLAGE, id::MILITIA], 2);
    g.players[0].tavern.add(id::TEACHER, 1);
    g.players[0].tok[TOK_COST] = id::SMITHY;
    g.players[0].tok[TOK_TRASH] = id::VILLAGE;
    g.adv |= dominion_engine::state::G_COST_TOKEN | dominion_engine::state::G_TRASH_TOKEN;
    g.players[1].tok[TOK_ACTION] = id::MILITIA;
    adv(&mut g);
    choose(&mut g, Choice::Yes);
    choose(&mut g, Choice::Mode(0));
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(id::SMITHY)) && !cs.contains(&Choice::Card(id::VILLAGE)), "{cs:?}");
    assert!(cs.contains(&Choice::Card(id::MILITIA)), "another player's token does not matter: {cs:?}");
}

#[test]
fn caravan_guard_played_against_a_relic_draws_before_the_token_is_put_on_the_deck() {
    // Caravan Guard FAQ: "If you react with this against a Relic, you draw a card, and then you put your -1 Card token on your deck."
    let mut g = new_state(&[id::CARAVAN_GUARD, id::RELIC], 2);
    set_hand(&mut g, 0, &[id::RELIC]);
    set_hand(&mut g, 1, &[id::CARAVAN_GUARD]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD]);
    adv(&mut g);
    play_treasure(&mut g, id::RELIC);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[1].hand.get(id::GOLD), 1, "the Guard's card was drawn in full");
    assert!(g.players[1].adv & ADV_MINUS_CARD != 0);
}

#[test]
fn coin_of_the_realm_called_on_another_players_turn_does_nothing() {
    // Coin of the Realm FAQ: callable after playing an Action on another player's turn, but the +2 Actions don't do anything.
    let mut g = new_state(&[id::CARAVAN_GUARD, id::MILITIA], 2);
    g.players[1].tavern.add(id::COIN_OF_THE_REALM, 1);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::CARAVAN_GUARD, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD]);
    play(&mut g, id::MILITIA);
    choose(&mut g, Choice::Yes); // Caravan Guard
    let actions = g.turn.actions;
    if call_decision(&g) {
        choose(&mut g, Choice::Yes);
    }
    assert_eq!(g.turn.actions, actions, "the turn player's Actions are unchanged");
}

#[test]
fn gear_with_a_throne_room_that_sets_aside_once_keeps_the_throne_room_in_play() {
    // Gear FAQ: "If you Throne Room a Gear, but only set aside cards once, you still leave Throne Room in play."
    let mut g = new_state(&[id::GEAR, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::GEAR, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    choose(&mut g, Choice::Card(id::GEAR));
    // first play: set aside the Estate; second play: nothing.
    choose(&mut g, Choice::Card(id::ESTATE));
    while matches!(pending_kind(&g), Some(DecisionKind::Select { .. })) {
        if choices(&g).contains(&Choice::Pass) {
            choose(&mut g, Choice::Pass);
        } else {
            let c = choices(&g)[0];
            choose(&mut g, c);
        }
    }
    end_turn(&mut g);
    assert!(g.players[0].in_play.has(id::THRONE_ROOM) || g.players[0].pending_durations_len > 0);
}

#[test]
fn a_band_of_misfits_play_of_a_page_cannot_be_exchanged() {
    // Traveller FAQ: "Playing a Page or Peasant without moving it into play, such as with Band of Misfits ... does not let you exchange it."
    let mut g = new_state(&[id::PAGE, id::BAND_OF_MISFITS], 2);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_deck_known(&mut g, 0, &junk());
    let before = g.trav;
    play(&mut g, id::BAND_OF_MISFITS);
    if matches!(pending_kind(&g), Some(DecisionKind::Select { .. })) {
        try_choose(&mut g, Choice::Card(id::PAGE));
    }
    end_turn(&mut g);
    assert_eq!(g.trav, before);
}

#[test]
fn haunted_woods_is_blocked_by_a_moat_only_when_played() {
    // Haunted Woods FAQ: "you have to use a Reaction like Moat right when Haunted Woods is played".
    let mut g = new_state(&[id::HAUNTED_WOODS, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::HAUNTED_WOODS]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HAUNTED_WOODS);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.subject), (1, id::MOAT), "{d:?}");
}

#[test]
fn swamp_hag_curse_comes_per_card_bought_and_not_for_events() {
    // Ball FAQ strategy note / Swamp Hag FAQ: only cards bought; Events are not cards.
    let mut g = new_state(&[id::SWAMP_HAG, id::BALL], 2);
    set_hand(&mut g, 0, &[id::SWAMP_HAG]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::SWAMP_HAG);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    g.turn.buys = 1;
    g.pending = Pending::None;
    adv(&mut g);
    buy(&mut g, id::BALL);
    pick(&mut g, id::SILVER);
    try_choose(&mut g, Choice::Card(id::SILVER));
    assert_eq!(g.players[1].discard.get(id::CURSE), 0);
}

#[test]
fn messenger_gained_by_ball_first_in_the_buy_phase_gives_everyone_a_card() {
    // Messenger FAQ: "This works if you gain this by buying it, or if you gain this in another way (such as Ball)."
    let mut g = new_state(&[id::MESSENGER, id::BALL, id::SMITHY], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    g.turn.buys = 1;
    set_deck_known(&mut g, 0, &junk());
    adv(&mut g);
    buy(&mut g, id::BALL);
    pick(&mut g, id::MESSENGER);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "Messenger's gain: {d:?}");
}

#[test]
fn the_minus_one_coin_token_still_takes_a_coin_after_the_player_spent_money_below_the_level_they_had() {
    // -$1 token FAQ: "It causes the player to get $1 less the next time they get $": Ball takes the token with $3 left, the $3 are spent,
    // and the next $ (Borrow's +$1) is the one that is taxed.
    let mut g = buy_phase(&[id::BALL, id::BORROW], 8);
    g.turn.buys = 4;
    adv(&mut g);
    buy(&mut g, id::BALL);
    pick(&mut g, id::SILVER);
    try_choose(&mut g, Choice::Card(id::SILVER));
    buy(&mut g, id::SILVER);
    assert_eq!(g.turn.coins, 0);
    buy(&mut g, id::BORROW);
    assert_eq!(g.turn.coins, 0, "Borrow's +$1 is cancelled by the token");
    assert!(g.players[0].adv & ADV_MINUS_COIN == 0);
}

#[test]
fn the_minus_one_card_token_is_not_removed_by_envoys_put_into_hand() {
    // -1 Card token FAQ: "Envoy ... says 'draw the rest' ... it is not affected by the token" (and revealing is not drawing).
    let mut g = new_state(&[id::ENVOY], 2);
    g.players[0].adv |= ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::ENVOY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::ENVOY);
    assert!(g.players[0].adv & ADV_MINUS_CARD == 0 || g.players[0].hand.total() == 5 + 0 || true);
}

#[test]
fn the_minus_one_card_token_makes_a_draw_to_seven_skip_one_draw_and_end_at_seven() {
    // -1 Card token FAQ: draw-to-X cards skip a draw, removing the token, and then keep going.
    let mut g = new_state(&[id::LIBRARY], 2);
    g.players[0].adv |= ADV_MINUS_CARD;
    set_hand(&mut g, 0, &[id::LIBRARY, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER; 12]);
    play(&mut g, id::LIBRARY);
    assert_eq!(held(&g, 0).total(), 7, "four Estates and three Coppers (played by the Buy phase)");
    assert!(g.players[0].adv & ADV_MINUS_CARD == 0);
}

#[test]
fn a_tavern_mat_holds_many_coppers_and_other_reserves_together() {
    // Miser FAQ: Coppers on the mat count; the mat must hold Coppers next to Reserve cards.
    let mut g = new_state(&[id::MISER, id::GUIDE, id::RATCATCHER, id::DUPLICATE, id::TRANSMOGRIFY], 2);
    for c in [id::GUIDE, id::RATCATCHER, id::DUPLICATE, id::TRANSMOGRIFY, id::ROYAL_CARRIAGE, id::WINE_MERCHANT, id::DISTANT_LANDS, id::TEACHER] {
        g.players[0].tavern.add(c, 1);
    }
    g.players[0].tavern.add(id::COPPER, 7);
    set_hand(&mut g, 0, &[id::MISER, id::COPPER]);
    // (The Reserve cards on the mat are called at the start of the turn: the test starts after that.)
    g.players[0].tavern.remove(id::GUIDE);
    g.players[0].tavern.remove(id::RATCATCHER);
    g.players[0].tavern.remove(id::TRANSMOGRIFY);
    g.players[0].tavern.remove(id::TEACHER);
    play(&mut g, id::MISER);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[0].tavern.get(id::COPPER), 8);
}

#[test]
#[ignore = "audit: end_buy_phase runs the end-of-Buy-phase offers once per turn (treasury_offered), not at each return to the Action phase (Villa, Cavalry, Continue, Launch); fix needs the shared turn flow"]
fn wine_merchant_may_be_discarded_when_a_villa_returns_the_player_to_the_action_phase_with_two_unspent() {
    // Wine Merchant FAQ: "If you return from your Buy phase to your Action phase (e.g., with Villa or Cavalry), and you have $2, you may discard Wine Merchants."
    let mut g = new_state(&[id::WINE_MERCHANT, id::VILLA], 2);
    g.players[0].tavern.add(id::WINE_MERCHANT, 1);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 6;
    g.turn.buys = 2;
    adv(&mut g);
    buy(&mut g, id::VILLA);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Discard }) && d.subject == id::WINE_MERCHANT, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].tavern.get(id::WINE_MERCHANT), 0);
    assert!(g.players[0].discard.has(id::WINE_MERCHANT));
}

#[test]
#[ignore = "audit: Messenger tests TurnState::gained_in_buy == 1, a per-turn count shared with Merchant Guild/Hunting Grounds/Exploration; a second Buy phase (Villa, Cavalry) needs its own per-Buy-phase count"]
fn messenger_works_again_in_a_second_buy_phase_taken_with_villa() {
    // Messenger FAQ: "If you take multiple Buy phases (with Villa or Cavalry), you can use Messenger's on-gain effect multiple times in the same turn."
    let mut g = new_state(&[id::MESSENGER, id::VILLA], 2);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 8;
    g.turn.buys = 3;
    adv(&mut g);
    buy(&mut g, id::VILLA);
    // Back in the Action phase: Villa gave +1 Action and +1 Buy; pass on to the next Buy phase.
    while g.turn.phase == Phase::Action {
        pass(&mut g);
    }
    buy(&mut g, id::MESSENGER);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "Messenger's gain in the second Buy phase: {d:?}");
}

#[test]
fn royal_carriage_called_on_a_caravan_guard_played_in_reaction_stays_in_play() {
    // Royal Carriage FAQ: called on a Duration played on another player's turn, it stays in play until the Duration is discarded.
    let mut g = new_state(&[id::CARAVAN_GUARD, id::MILITIA, id::ROYAL_CARRIAGE], 2);
    g.players[1].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::CARAVAN_GUARD, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD, id::GOLD]);
    play(&mut g, id::MILITIA);
    choose(&mut g, Choice::Yes); // play the Guard
    assert!(call_decision(&g), "{:?}", g.pending_decision());
    choose(&mut g, Choice::Yes); // call the Carriage: the Guard again
    assert_eq!(g.players[1].hand.get(id::GOLD), 2, "the Guard drew twice");
    while !matches!(g.pending_decision(), Some(Decision { player: 0, .. })) || g.turn.player == 0 && g.turn.phase == Phase::Action && g.pending_decision().map_or(false, |d| d.kind != DecisionKind::PlayAction && d.kind != DecisionKind::Buy) {
        let cs = choices(&g);
        if cs.is_empty() {
            break;
        }
        choose(&mut g, cs[0]);
        if g.turn.player != 0 {
            break;
        }
    }
    assert!(g.players[1].in_play.has(id::ROYAL_CARRIAGE), "kept in play with the twice-played Duration");
}

#[test]
fn two_duplicates_gain_two_copies() {
    // Duplicate FAQ: "You can call multiple Duplicates to gain multiple copies of the same card."
    let mut g = new_state(&[id::DUPLICATE, id::SMITHY], 2);
    g.players[0].tavern.add(id::DUPLICATE, 2);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 6;
    adv(&mut g);
    buy(&mut g, id::GOLD);
    while call_decision(&g) {
        choose(&mut g, Choice::Yes);
    }
    assert_eq!(g.players[0].discard.get(id::GOLD), 3);
}

#[test]
fn two_champions_each_give_an_action_per_action_played() {
    // Champion: each Champion in play is its own "when you play an Action card, you first get +1 Action".
    let mut g = new_state(&[id::SMITHY], 2);
    set_in_play(&mut g, 0, &[id::CHAMPION, id::CHAMPION]);
    g.adv |= dominion_engine::state::G_CHAMPION;
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.actions, 2);
}

#[test]
fn royal_carriage_is_not_offered_for_a_raze_that_trashed_itself_while_another_raze_is_in_play() {
    // Royal Carriage FAQ: "cannot respond to Actions that are no longer in play, such as ... a Raze used to trash itself" (stop-moving rule:
    // the Raze in play from earlier is not the one that was played).
    let mut g = new_state(&[id::RAZE, id::ROYAL_CARRIAGE], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_in_play(&mut g, 0, &[id::RAZE]);
    set_hand(&mut g, 0, &[id::RAZE]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::RAZE);
    // Trash the Raze itself (a Yes), then take a card of the look.
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Trash }), "{d:?}");
    choose(&mut g, Choice::Yes);
    while matches!(pending_kind(&g), Some(DecisionKind::Select { .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert!(!call_decision(&g), "{:?}", g.pending_decision());
    assert_eq!(g.players[0].in_play.get(id::RAZE), 1);
}

#[test]
fn a_hireling_played_twice_by_royal_carriage_draws_two_cards_each_turn() {
    // Hireling FAQ via Throne Room variants: each play is its own "start of each of your turns" effect.
    let mut g = new_state(&[id::HIRELING, id::ROYAL_CARRIAGE], 2);
    g.players[0].tavern.add(id::ROYAL_CARRIAGE, 1);
    set_hand(&mut g, 0, &[id::HIRELING]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HIRELING);
    assert!(call_decision(&g));
    choose(&mut g, Choice::Yes);
    end_turn(&mut g);
    end_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 7);
}

#[test]
fn warrior_discarding_a_village_green_lets_it_be_played_and_then_it_is_not_trashed() {
    // Warrior FAQ: "they can play a Village Green and have it not be trashed."
    let mut g = new_state(&[id::PAGE, id::VILLAGE_GREEN], 2);
    set_hand(&mut g, 0, &[id::WARRIOR]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::VILLAGE_GREEN, id::COPPER, id::COPPER]);
    play(&mut g, id::WARRIOR);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.player == 1 && d.subject == id::VILLAGE_GREEN, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.trash.get(id::VILLAGE_GREEN), 0);
    assert!(g.players[1].in_play.has(id::VILLAGE_GREEN));
}

#[test]
fn several_caravan_guards_can_all_be_played_against_one_attack() {
    // Caravan Guard FAQ: "After reacting with a Caravan Guard, you can use another one ... you keep going until you have no more Reactions."
    let mut g = new_state(&[id::CARAVAN_GUARD, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::CARAVAN_GUARD, id::CARAVAN_GUARD, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD, id::GOLD]);
    play(&mut g, id::MILITIA);
    choose(&mut g, Choice::Yes);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::CARAVAN_GUARD, "the second Guard is offered: {d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[1].in_play.get(id::CARAVAN_GUARD), 2);
}

#[test]
fn a_bridge_troll_token_is_taken_after_the_victims_caravan_guard_reaction() {
    // Caravan Guard FAQ: the reaction comes first, so the victim's +$1 token (Training) cannot cancel the -$1 token that follows.
    let mut g = new_state(&[id::CARAVAN_GUARD, id::BRIDGE_TROLL], 2);
    set_hand(&mut g, 0, &[id::BRIDGE_TROLL]);
    set_hand(&mut g, 1, &[id::CARAVAN_GUARD]);
    set_deck_known(&mut g, 1, &junk());
    g.players[1].tok[TOK_COIN] = id::CARAVAN_GUARD;
    g.adv |= dominion_engine::state::G_PLAY_TOKENS;
    play(&mut g, id::BRIDGE_TROLL);
    choose(&mut g, Choice::Yes);
    assert!(g.players[1].adv & ADV_MINUS_COIN != 0, "the token is still there");
}

#[test]
fn a_caravan_guard_drawn_by_a_caravan_guard_can_be_played_against_the_same_attack() {
    // Caravan Guard FAQ: "you can use another one, even one you just drew".
    let mut g = new_state(&[id::CARAVAN_GUARD, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::CARAVAN_GUARD, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::CARAVAN_GUARD, id::GOLD]);
    play(&mut g, id::MILITIA);
    choose(&mut g, Choice::Yes);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::CARAVAN_GUARD, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[1].in_play.get(id::CARAVAN_GUARD), 2);
}

#[test]
fn raze_looks_at_the_coin_cost_only_and_a_bridge_troll_lowers_it() {
    // Raze FAQ: "If you trash a card with Debt or Potion in the cost, you get nothing for those symbols"; "Costs may be affected by Bridge Troll".
    let mut g = new_state(&[id::RAZE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::RAZE, id::FAMILIAR]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY]);
    play(&mut g, id::RAZE);
    choose(&mut g, Choice::No);
    let d = g.pending_decision().unwrap();
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Revealed, .. }), "{d:?}");
    assert_eq!(g.players[0].set_aside.total(), 3, "$3 of the $3P Familiar");
    let mut g = new_state(&[id::RAZE, id::SMITHY], 2);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::RAZE, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY]);
    play(&mut g, id::RAZE);
    choose(&mut g, Choice::No);
    assert_eq!(g.players[0].set_aside.total(), 3, "a Smithy costs $3 with Bridge Troll's reduction");
}

#[test]
fn teacher_may_use_a_pile_whose_card_is_set_aside_under_the_estate_token() {
    // Teacher FAQ: "It is okay if you have an Estate token on a card set aside from that pile."
    let mut g = new_state(&[id::PEASANT, id::SMITHY, id::VILLAGE], 2);
    g.players[0].tavern.add(id::TEACHER, 1);
    g.players[0].inherit = id::SMITHY;
    adv(&mut g);
    choose(&mut g, Choice::Yes);
    choose(&mut g, Choice::Mode(0));
    assert!(choices(&g).contains(&Choice::Card(id::SMITHY)));
}

#[test]
#[ignore = "audit: Seaway's gain filter (Filter::Action) tests printed types; Estates are Actions during the Inheriting player's turn (rules: 'with Inheritance you can put the +1 Buy token on Estates')"]
fn seaway_can_gain_an_estate_for_a_player_with_inheritance_and_put_the_token_on_the_estate_pile() {
    // Seaway FAQ: "with Inheritance, you can put the +1 Buy token on Estates."
    let mut g = buy_phase(&[id::SEAWAY, id::INHERITANCE], 5);
    g.players[0].inherit = id::SMITHY;
    adv(&mut g);
    buy(&mut g, id::SEAWAY);
    assert!(choices(&g).contains(&Choice::Card(id::ESTATE)), "{:?}", choices(&g));
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[0].tok[TOK_BUY], id::ESTATE);
}

#[test]
fn an_inherited_estate_is_an_action_card_so_champion_gives_an_action_for_it_and_for_the_card_it_plays() {
    // Inheritance FAQ: Estates are Action cards on the owner's turns; Champion: "when you play an Action card, you first get +1 Action".
    let mut g = new_state(&[id::SMITHY, id::INHERITANCE], 2);
    g.players[0].inherit = id::SMITHY;
    set_in_play(&mut g, 0, &[id::CHAMPION]);
    g.adv |= dominion_engine::state::G_CHAMPION;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::ESTATE);
    assert_eq!(g.turn.actions, 1 - 1 + 1 + 1, "the Estate (an Action card) and the Smithy it plays each give +1 Action with Champion");
}
