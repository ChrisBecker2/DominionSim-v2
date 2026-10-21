//! Menagerie tests (rulings in `docs/menagerie-plan.md`): Exile, the Horse pile, the Ways, the Events and the 30 kingdom cards.

mod common;
use common::*;
use dominion_engine::cards::{self, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

// ===========================================================================
// Random games: nothing is lost, nothing panics, every game ends
// ===========================================================================

#[test]
fn random_menagerie_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0x3E4A);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Menagerie).collect();
    assert_eq!(pool.len(), 30);
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::Menagerie).collect();
    let landscapes: Vec<CardId> = cards::pseudo_cards_of(CardSet::Menagerie).collect();
    assert_eq!(landscapes.len(), 40);
    let mut played = std::collections::BTreeSet::new();
    let mut bought = std::collections::BTreeSet::new();
    let mut ways = std::collections::BTreeSet::new();
    let games: u64 = std::env::var("MENAGERIE_GAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    for game in 0..games {
        let n = 2 + (game % 4) as usize;
        let mut kingdom: Vec<CardId> = Vec::new();
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
        // Up to two landscapes of the set (at most one Way).
        let mut ls = landscapes.clone();
        let mut ways_in = 0;
        for _ in 0..(game % 3) {
            let i = rng.below(ls.len() as u32) as usize;
            let l = ls.swap_remove(i);
            if cards::is_way(l) {
                if ways_in == 1 {
                    continue;
                }
                ways_in += 1;
            }
            kingdom.push(l);
        }
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom: kingdom.clone(), seed: game, max_turns: 300, ..Default::default() });
        let initial = total_cards(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        let mut log: Vec<Event> = Vec::new();
        let mut recent: Vec<String> = Vec::new();
        loop {
            for e in &log {
                match *e {
                    Event::Play { card, .. } | Event::PlayAgain { card, .. } | Event::StartPlay { card, .. } => {
                        played.insert(card);
                    }
                    Event::Buy { card, .. } => {
                        bought.insert(card);
                    }
                    Event::Way { way, .. } => {
                        ways.insert(way);
                    }
                    _ => {}
                }
                recent.push(format!("{e:?}"));
            }
            if recent.len() > 60 {
                recent.drain(..recent.len() - 60);
            }
            log.clear();
            let chosen;
            match g.advance(&mut log) {
                Step::Decision(d) => {
                    // Some games are rich, so that the $10 Events are bought too.
                    if d.kind == DecisionKind::Buy && game % 4 == 0 && g.turn.coins < 10 {
                        g.turn.coins = 10;
                    }
                    g.legal_choices(&mut buf);
                    if buf.is_empty() {
                        panic!("no legal choice in game {game} for {d:?}\nkingdom {:?}\nrecent {recent:#?}", kingdom.iter().map(|&c| cards::name(c)).collect::<Vec<_>>());
                    }
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    chosen = Some((d, c));
                    recent.push(format!("{d:?} -> {c:?}"));
                    g.apply(c, &mut log).unwrap();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            if total_cards(&g) != initial {
                let mut sums = g.supply;
                sums.add_all(&g.trash);
                for p in 0..g.num_players as usize {
                    sums.add_all(&g.players[p].all_cards());
                }
                let diff: Vec<String> = sums.iter().map(|(c, k)| format!("{}:{k}", cards::name(c))).collect();
                panic!(
                    "card conservation, game {game}, chose {chosen:?}: {log:?}\nzones: {diff:?}\nside {} (started with {initial}, now {})\nkingdom {:?}\nrecent {recent:#?}",
                    g.side_pile_cards(),
                    total_cards(&g),
                    kingdom.iter().map(|&c| cards::name(c)).collect::<Vec<_>>()
                );
            }
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
    }
    if games >= 3000 {
        let missing: Vec<&str> = (cards::id::BLACK_CAT..=cards::id::HORSE).filter(|c| !played.contains(c)).map(cards::name).collect();
        assert!(missing.is_empty(), "never played: {missing:?}");
        let missing: Vec<&str> = (cards::id::DELAY..=cards::id::POPULATE).filter(|c| !bought.contains(c)).map(cards::name).collect();
        assert!(missing.is_empty(), "never bought: {missing:?}");
        let missing: Vec<&str> = (cards::FIRST_WAY..cards::FIRST_WAY + cards::NUM_WAYS as CardId).filter(|c| !ways.contains(c)).map(cards::name).collect();
        assert!(missing.is_empty(), "never used: {missing:?}");
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

use dominion_engine::cards::id as I;

/// A deck of cards that are neither Treasures nor Actions: every draw is visible, nothing is auto-played.
fn junk() -> Vec<CardId> {
    [I::ESTATE, I::DUCHY, I::PROVINCE].iter().cycle().take(40).copied().collect()
}

/// Player 0's turn in a game with `kingdom` (2 players), the decks of junk.
fn game(kingdom: &[CardId]) -> GameState {
    let mut g = new_state(kingdom, 2);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    g
}

fn ex(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].men.exile.get(c)
}

// ===========================================================================
// Set-up and the Horse pile
// ===========================================================================

#[test]
fn the_ids_and_piles_line_up() {
    assert_eq!(cards::set_by_name("menagerie"), Some(CardSet::Menagerie));
    assert_eq!(cards::kingdom_cards_in(CardSet::Menagerie).count(), 30);
    assert!(!cards::is_kingdom(I::HORSE) && cards::is(I::HORSE, cards::ACTION));
    assert_eq!(cards::LAST_KINGDOM, I::LICH);
    assert!(cards::is_event(I::DELAY) && cards::is_event(I::POPULATE) && !cards::is_event(I::AQUEDUCT));
    assert!(cards::is_way(I::WAY_OF_THE_BUTTERFLY) && cards::is_way(I::WAY_OF_THE_WORM) && !cards::is_way(I::EXILE) && !cards::is_way(I::FLAG));
    assert_eq!((cards::NUM_EVENTS, cards::NUM_WAYS), (64, 20));
    assert_eq!(cards::NUM_ZONE_IDS, 503);
    assert_eq!(cards::cost(I::ANIMAL_FAIR), 7);
    assert_eq!(cards::cost(I::POPULATE), 10);
    assert_eq!(cards::cost(I::HORSE), 3);
}

#[test]
fn the_horse_pile_is_outside_the_supply() {
    let mut g = new_state(&[I::SLEIGH, I::SMITHY], 2);
    assert_eq!(g.supply.get(I::HORSE), 30);
    assert!(!g.in_supply(I::HORSE));
    assert_eq!(g.empty_piles(), 0);
    // Emptying it is not an empty Supply pile.
    g.supply.set(I::HORSE, 0);
    assert_eq!(g.empty_piles(), 0);
    // A game without a Horse gainer has none.
    let g = new_state(&[I::SMITHY, I::KILN], 2);
    assert_eq!(g.supply.get(I::HORSE), 0);
    assert!(!g.has_horse_pile());
    // An Event that gains one brings the pile.
    let g = new_state(&[I::SMITHY, I::RIDE], 2);
    assert_eq!(g.supply.get(I::HORSE), 30);
}

#[test]
fn a_horse_cannot_be_bought() {
    let mut g = game(&[I::SLEIGH, I::WORKSHOP]);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 8;
    set_hand(&mut g, 0, &[I::ESTATE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert!(!choices(&g).contains(&Choice::Card(I::HORSE)));
}

#[test]
fn a_gain_from_the_supply_cannot_take_a_horse() {
    let mut g = game(&[I::SLEIGH, I::WORKSHOP]);
    set_hand(&mut g, 0, &[I::WORKSHOP]);
    play(&mut g, I::WORKSHOP);
    assert!(!choices(&g).contains(&Choice::Card(I::HORSE)));
}

#[test]
fn the_horse_is_two_cards_and_an_action_and_returns_to_its_pile() {
    let mut g = game(&[I::SLEIGH]);
    set_hand(&mut g, 0, &[I::HORSE]);
    g.supply.set(I::HORSE, 29);
    play(&mut g, I::HORSE);
    assert_eq!(g.players[0].hand.total(), 2);
    assert_eq!(g.supply.get(I::HORSE), 30, "back in its pile");
    assert_eq!(g.players[0].in_play.get(I::HORSE), 0);
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn a_horse_played_twice_gives_the_cards_each_time_but_returns_once() {
    let mut g = game(&[I::SLEIGH, I::THRONE_ROOM]);
    set_hand(&mut g, 0, &[I::THRONE_ROOM, I::HORSE]);
    g.supply.set(I::HORSE, 29);
    let before = total_cards(&g);
    play(&mut g, I::THRONE_ROOM);
    choose(&mut g, Choice::Card(I::HORSE));
    assert_eq!(g.players[0].hand.total(), 4, "2 + 2 cards");
    assert_eq!(g.supply.get(I::HORSE), 30);
    assert_eq!(g.players[0].in_play.get(I::HORSE), 0);
    assert_eq!(total_cards(&g), before);
}

// ===========================================================================
// Kingdom cards
// ===========================================================================

#[test]
fn black_cat_draws_two_and_curses_nobody_on_its_owners_turn() {
    let mut g = game(&[I::BLACK_CAT]);
    set_hand(&mut g, 0, &[I::BLACK_CAT]);
    play(&mut g, I::BLACK_CAT);
    assert_eq!(g.players[0].hand.total(), 2);
    assert_eq!(g.players[1].discard.get(I::CURSE), 0);
}

#[test]
fn black_cat_in_hand_is_played_when_another_player_gains_a_victory_card() {
    let mut g = game(&[I::BLACK_CAT]);
    // Player 1's turn: they buy an Estate, player 0 plays the Cat from their hand.
    reset_turn(&mut g, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 2;
    g.turn.buys = 2;
    set_hand(&mut g, 0, &[I::BLACK_CAT]);
    set_hand(&mut g, 1, &[]);
    buy(&mut g, I::ESTATE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == I::BLACK_CAT && d.player == 0, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].hand.total(), 2, "+2 Cards");
    assert_eq!(g.players[1].discard.get(I::CURSE), 1, "the turn's player gains the Curse");
    assert_eq!(g.players[0].discard.get(I::CURSE), 0);
    assert_eq!(g.players[0].in_play.get(I::BLACK_CAT), 1);
}

#[test]
fn black_cat_does_not_react_to_a_card_that_is_not_victory() {
    let mut g = game(&[I::BLACK_CAT]);
    reset_turn(&mut g, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    set_hand(&mut g, 0, &[I::BLACK_CAT]);
    set_hand(&mut g, 1, &[]);
    buy(&mut g, I::SILVER);
    assert!(g.pending_decision().map_or(true, |d| !matches!(d.kind, DecisionKind::YesNo { .. })));
}

#[test]
fn sleigh_gains_two_horses() {
    let mut g = game(&[I::SLEIGH]);
    set_hand(&mut g, 0, &[I::SLEIGH]);
    play(&mut g, I::SLEIGH);
    assert_eq!(g.players[0].discard.get(I::HORSE), 2);
    assert_eq!(g.supply.get(I::HORSE), 28);
}

#[test]
fn sleigh_in_hand_puts_a_gained_card_onto_the_deck_or_into_the_hand() {
    for (mode, on_deck) in [(0u8, false), (1u8, true)] {
        let mut g = game(&[I::SLEIGH]);
        g.turn.phase = Phase::Buy;
        g.turn.coins = 6;
        g.turn.buys = 2;
        set_hand(&mut g, 0, &[I::SLEIGH]);
        buy(&mut g, I::GOLD);
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Discard }) && d.subject == I::SLEIGH, "{d:?}");
        assert_eq!(d.target, I::GOLD);
        choose(&mut g, Choice::Yes);
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Mode { .. }) && d.source == Some(I::SLEIGH));
        choose(&mut g, Choice::Mode(mode));
        adv(&mut g);
        assert_eq!(g.players[0].discard.get(I::SLEIGH), 1);
        if on_deck {
            assert_eq!(g.players[0].deck_known.peek_top(), Some(I::GOLD));
        } else {
            assert_eq!(g.players[0].hand.get(I::GOLD), 1);
        }
        assert_eq!(g.players[0].discard.get(I::GOLD), 0);
    }
}

#[test]
fn supplies_is_a_dollar_and_gains_a_horse_onto_the_deck() {
    let mut g = game(&[I::SLEIGH, I::SUPPLIES]);
    g.turn.phase = Phase::Buy;
    set_hand(&mut g, 0, &[I::SUPPLIES, I::COPPER]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::HORSE));
}

#[test]
fn camel_train_exiles_a_non_victory_card_and_a_gold_when_gained() {
    let mut g = game(&[I::CAMEL_TRAIN, I::SMITHY]);
    set_hand(&mut g, 0, &[I::CAMEL_TRAIN, I::SMITHY]);
    play(&mut g, I::CAMEL_TRAIN);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }));
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(I::SMITHY)) && cs.contains(&Choice::Card(I::CURSE)) && !cs.contains(&Choice::Card(I::ESTATE)), "{cs:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(ex(&g, 0, I::SMITHY), 1);
    assert_eq!(g.supply.get(I::SMITHY), 9);
    assert_eq!(g.players[0].all_cards().get(I::SMITHY), 2, "the Exiled card is yours (and the one in hand)");
    // Gaining a Camel Train Exiles a Gold.
    let mut g = game(&[I::CAMEL_TRAIN, I::SMITHY]);
    g.gain_card(0, I::CAMEL_TRAIN, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(ex(&g, 0, I::GOLD), 1);
    assert_eq!(g.supply.get(I::GOLD), 29);
    assert_eq!(g.players[0].discard.get(I::GOLD), 0, "Exiling is not gaining");
}

#[test]
fn goatherd_trashes_and_draws_for_the_trashes_of_the_player_to_the_right() {
    let mut g = game(&[I::GOATHERD]);
    g.players[1].men.trashed_last = 2; // player 1 is to the right of player 0 in a 2-player game
    set_hand(&mut g, 0, &[I::GOATHERD, I::COPPER, I::ESTATE, I::SMITHY]);
    play(&mut g, I::GOATHERD);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    choose(&mut g, Choice::Card(I::ESTATE));
    assert_eq!(g.trash.get(I::ESTATE), 1);
    assert_eq!(g.players[0].hand.total(), 2 + 2, "Copper and Smithy + 2 Cards drawn");
}

#[test]
fn goatherd_counts_the_trashes_of_a_turn_for_the_next_player() {
    let mut g = game(&[I::GOATHERD, I::CHAPEL]);
    assert_ne!(g.men.flags & dominion_engine::state::M_TRASHES, 0);
    set_hand(&mut g, 0, &[I::CHAPEL, I::ESTATE, I::ESTATE, I::COPPER, I::COPPER]);
    play(&mut g, I::CHAPEL);
    choose(&mut g, Choice::Card(I::COPPER));
    choose(&mut g, Choice::Card(I::ESTATE));
    choose(&mut g, Choice::Card(I::ESTATE));
    let mut guard = 0;
    while g.turn.player == 0 && guard < 30 {
        guard += 1;
        match adv(&mut g) {
            Step::Decision(_) => {
                pass(&mut g);
            }
            _ => break,
        }
    }
    assert_eq!(g.players[0].men.trashed_last, 3);
}

#[test]
fn scrap_gives_one_different_thing_per_dollar_of_the_card_trashed() {
    let mut g = game(&[I::SCRAP, I::SLEIGH]);
    g.turn.buys = 1;
    set_hand(&mut g, 0, &[I::SCRAP, I::SILVER]);
    // (The only card to trash is picked at once.) It costs $3: three things.
    play(&mut g, I::SCRAP);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 3, distinct: true }), "{d:?}");
    assert_eq!(choices(&g).len(), 6);
    // +1 Card, +1 Buy, gain a Horse.
    choose(&mut g, Choice::Mode(0));
    choose(&mut g, Choice::Mode(2));
    choose(&mut g, Choice::Mode(5));
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 1);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.players[0].discard.get(I::HORSE), 1);
}

#[test]
fn scrap_with_a_six_cost_card_gives_all_six() {
    let mut g = game(&[I::SCRAP, I::SLEIGH]);
    set_hand(&mut g, 0, &[I::SCRAP, I::PROVINCE]);
    play(&mut g, I::SCRAP); // the Province is trashed at once: $8, all six
    let mut guard = 0;
    while let Some(Decision { kind: DecisionKind::Mode { .. }, .. }) = g.pending_decision() {
        let c = choices(&g)[0];
        choose(&mut g, c);
        guard += 1;
        assert!(guard < 8);
    }
    adv(&mut g);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    assert_eq!(g.turn.coins, 1);
    assert_eq!(g.players[0].discard.get(I::SILVER), 1);
    assert_eq!(g.players[0].discard.get(I::HORSE), 1);
    assert_eq!(g.players[0].hand.total(), 1);
}

#[test]
fn sheepdog_in_hand_is_played_when_its_owner_gains_a_card() {
    let mut g = game(&[I::SHEEPDOG]);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 6;
    g.turn.buys = 2;
    set_hand(&mut g, 0, &[I::SHEEPDOG]);
    buy(&mut g, I::GOLD);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == I::SHEEPDOG, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].hand.total(), 2, "+2 Cards");
    assert_eq!(g.players[0].in_play.get(I::SHEEPDOG), 1);
    assert_eq!(g.turn.actions, 1, "playing it used no Action");
}

#[test]
fn snowy_village_ignores_the_actions_that_come_after_it() {
    // A Village after Snowy Village gives nothing.
    let mut g = game(&[I::SNOWY_VILLAGE, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::SNOWY_VILLAGE, I::VILLAGE, I::VILLAGE, I::ESTATE]);
    play(&mut g, I::SNOWY_VILLAGE);
    assert_eq!(g.turn.actions, 4);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.players[0].hand.total(), 4, "3 left + 1 Card");
    play(&mut g, I::VILLAGE);
    assert_eq!(g.turn.actions, 3, "the Village's +2 Actions are ignored");
    play(&mut g, I::VILLAGE);
    assert_eq!(g.turn.actions, 2);
    // Played before a Village the other way round it is the Village that counts.
    let mut g = game(&[I::SNOWY_VILLAGE, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::SNOWY_VILLAGE, I::VILLAGE, I::ESTATE]);
    play(&mut g, I::VILLAGE);
    assert_eq!(g.turn.actions, 2);
    play(&mut g, I::SNOWY_VILLAGE);
    assert_eq!(g.turn.actions, 5, "the 2 Actions already there stay");
}

#[test]
fn a_villager_gives_nothing_after_snowy_village() {
    let mut g = game(&[I::SNOWY_VILLAGE, I::VILLAGE]);
    g.players[0].ren.villagers = 2;
    set_hand(&mut g, 0, &[I::SNOWY_VILLAGE, I::ESTATE]);
    play(&mut g, I::SNOWY_VILLAGE);
    // No Action card is left in the hand anyway: on to the Buy phase.
    assert_eq!(g.turn.phase, Phase::Buy);
    let mut g = game(&[I::SNOWY_VILLAGE, I::VILLAGE]);
    g.players[0].ren.villagers = 2;
    set_hand(&mut g, 0, &[I::SNOWY_VILLAGE, I::VILLAGE]);
    g.turn.actions = 1;
    play(&mut g, I::SNOWY_VILLAGE);
    g.turn.actions = 0;
    play_choices_have_no_villager(&mut g);
}

fn play_choices_have_no_villager(g: &mut GameState) {
    g.turn.actions = 0;
    let _ = adv(g);
    assert!(!choices(g).contains(&Choice::Villager));
}

#[test]
fn stockpile_exiles_itself_and_buying_another_brings_them_all_back() {
    let mut g = game(&[I::STOCKPILE]);
    g.turn.phase = Phase::Buy;
    g.turn.buys = 2;
    set_hand(&mut g, 0, &[I::STOCKPILE, I::STOCKPILE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 6);
    assert_eq!(g.turn.buys, 4);
    assert_eq!(ex(&g, 0, I::STOCKPILE), 2, "both Exiled");
    assert_eq!(g.players[0].in_play.get(I::STOCKPILE), 0);
    assert_eq!(g.players[0].all_cards().get(I::STOCKPILE), 2);
    buy(&mut g, I::STOCKPILE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Discard }) && d.subject == I::STOCKPILE && d.source == Some(I::EXILE), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(ex(&g, 0, I::STOCKPILE), 0);
    assert_eq!(g.players[0].discard.get(I::STOCKPILE), 3);
}

#[test]
fn the_copies_on_the_mat_may_be_left_there_when_a_copy_is_gained() {
    let mut g = game(&[I::STOCKPILE]);
    g.players[0].men.exile.add(I::SILVER, 2);
    g.men.hook |= dominion_engine::state::H_EXILE;
    g.gain_card(0, I::SILVER, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Discard }) && d.subject == I::SILVER, "{d:?}");
    choose(&mut g, Choice::No);
    assert_eq!(ex(&g, 0, I::SILVER), 2, "all or none: a No leaves them");
    assert_eq!(g.players[0].discard.get(I::SILVER), 1);
}

#[test]
fn exiled_cards_count_for_scoring_and_belong_to_their_player() {
    let mut g = game(&[I::STOCKPILE]);
    let before = g.player_vp(0);
    let before_provinces = g.players[0].all_cards().get(I::PROVINCE);
    g.players[0].men.exile.add(I::DUCHY, 1);
    assert_eq!(g.player_vp(0), before + 3);
    g.players[0].men.exile.add(I::PROVINCE, 2);
    assert_eq!(g.player_vp(0), before + 15);
    assert_eq!(g.players[0].all_cards().get(I::PROVINCE), before_provinces + 2);
}

#[test]
fn bounty_hunter_gives_three_dollars_only_for_a_card_with_no_copy_in_exile() {
    let mut g = game(&[I::BOUNTY_HUNTER]);
    set_hand(&mut g, 0, &[I::BOUNTY_HUNTER, I::ESTATE, I::BOUNTY_HUNTER, I::COPPER]);
    set_hand(&mut g, 0, &[I::BOUNTY_HUNTER, I::BOUNTY_HUNTER, I::ESTATE]);
    play(&mut g, I::BOUNTY_HUNTER);
    // The only card to Exile is picked at once; the other Bounty Hunter is still a choice.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Exile, min: 1, max: 1, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::ESTATE));
    assert_eq!(ex(&g, 0, I::ESTATE), 1);
    assert_eq!(g.turn.coins, 3);
    assert_eq!(g.turn.actions, 1);
    // A second Estate: a copy is on the mat already.
    g.players[0].hand.add(I::ESTATE, 1);
    play(&mut g, I::BOUNTY_HUNTER);
    assert_eq!(ex(&g, 0, I::ESTATE), 2);
    assert_eq!(g.turn.coins, 3, "no copy was missing from the mat");
}

#[test]
fn cardinal_exiles_a_card_costing_three_to_six_from_the_top_two_and_discards_the_rest() {
    let mut g = game(&[I::CARDINAL]);
    set_deck_known(&mut g, 1, &[I::GOLD, I::SILVER, I::COPPER]);
    set_hand(&mut g, 0, &[I::CARDINAL]);
    play(&mut g, I::CARDINAL);
    assert_eq!(g.turn.coins, 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Exile, from: Zone::Revealed, .. }) && d.player == 1, "{d:?}");
    assert_eq!(choices(&g).len(), 2, "the attacked player chooses");
    choose(&mut g, Choice::Card(I::GOLD));
    assert_eq!(ex(&g, 1, I::GOLD), 1);
    assert_eq!(g.players[1].discard.get(I::SILVER), 1);
    // Nothing costing $3 to $6 among the two: both are discarded.
    let mut g = game(&[I::CARDINAL]);
    set_deck_known(&mut g, 1, &[I::COPPER, I::PROVINCE, I::GOLD]);
    set_hand(&mut g, 0, &[I::CARDINAL]);
    play(&mut g, I::CARDINAL);
    assert_eq!(g.players[1].men.exile.total(), 0);
    assert_eq!(g.players[1].discard.total(), 2);
}

#[test]
fn cavalry_gains_two_horses_and_returns_to_the_action_phase_when_bought() {
    let mut g = game(&[I::CAVALRY]);
    set_hand(&mut g, 0, &[I::CAVALRY]);
    play(&mut g, I::CAVALRY);
    assert_eq!(g.players[0].discard.get(I::HORSE), 2);
    // Bought in the Buy phase: +2 Cards, +1 Buy, back to the Action phase.
    let mut g = game(&[I::CAVALRY]);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    set_hand(&mut g, 0, &[I::HORSE]);
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(I::CAVALRY), &mut ev);
    assert!(ev.iter().any(|e| matches!(e, Event::PhaseStart { phase: Phase::Action, .. })), "{ev:?}");
    assert_eq!(g.turn.buys, 1, "the Buy was used and +1 Buy given");
    assert_eq!(g.players[0].hand.total(), 3, "the Horse and 2 Cards");
}

#[test]
fn cavalry_gained_on_another_players_turn_draws_two_but_gives_no_buy() {
    let mut g = game(&[I::CAVALRY, I::WITCH]);
    g.gain_card(1, I::CAVALRY, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.players[1].hand.total(), 2);
    assert_eq!(g.turn.buys, 1);
}

#[test]
fn groom_gains_a_horse_for_an_action_a_silver_for_a_treasure_and_a_card_for_a_victory_card() {
    // A Silver: another Silver.
    let mut g = game(&[I::GROOM, I::SLEIGH, I::MILL]);
    set_hand(&mut g, 0, &[I::GROOM]);
    play(&mut g, I::GROOM);
    choose(&mut g, Choice::Card(I::SILVER));
    assert_eq!(g.players[0].discard.get(I::SILVER), 2);
    // A Mill (Action-Victory): a Horse, then +1 Card and +1 Action.
    let mut g = game(&[I::GROOM, I::SLEIGH, I::MILL]);
    set_hand(&mut g, 0, &[I::GROOM, I::SMITHY]);
    play(&mut g, I::GROOM);
    choose(&mut g, Choice::Card(I::MILL));
    assert_eq!(g.players[0].discard.get(I::MILL), 1);
    assert_eq!(g.players[0].discard.get(I::HORSE), 1);
    assert_eq!(g.players[0].hand.total(), 2, "Smithy + 1 Card");
    assert_eq!(g.turn.actions, 1, "+1 Action");
}

#[test]
fn hostelry_turns_treasures_into_horses_when_gained() {
    let mut g = game(&[I::HOSTELRY, I::SLEIGH]);
    set_hand(&mut g, 0, &[I::COPPER, I::SILVER, I::ESTATE]);
    g.gain_card(0, I::HOSTELRY, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, filter: Filter::Treasure, min: 0, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::COPPER));
    choose(&mut g, Choice::Card(I::SILVER));
    assert_eq!(g.players[0].discard.get(I::HORSE), 2);
    assert_eq!(g.players[0].hand.total(), 1);
}

#[test]
fn village_green_now_gives_a_card_and_two_actions() {
    let mut g = game(&[I::VILLAGE_GREEN]);
    set_hand(&mut g, 0, &[I::VILLAGE_GREEN, I::SMITHY]);
    play(&mut g, I::VILLAGE_GREEN);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Discard });
    assert_eq!(d.source, Some(I::VILLAGE_GREEN));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].hand.total(), 2);
    assert_eq!(g.players[0].pending_durations_len, 0);
}

#[test]
fn village_green_for_next_turn_stays_in_play_and_gives_its_bonus_then() {
    let mut g = game(&[I::VILLAGE_GREEN]);
    set_hand(&mut g, 0, &[I::VILLAGE_GREEN, I::SMITHY]);
    play(&mut g, I::VILLAGE_GREEN);
    choose(&mut g, Choice::No);
    assert_eq!(g.turn.actions, 0);
    assert_eq!(g.players[0].pending_durations_len, 1);
    // To the next turn of player 0.
    let mut guard = 0;
    while !(g.turn.player == 0 && g.turn.number > 2) && guard < 60 {
        guard += 1;
        settle_pass(&mut g);
    }
    assert_eq!(g.players[0].in_play.get(I::VILLAGE_GREEN), 1, "kept in play for its second part");
}

fn settle_pass(g: &mut GameState) {
    if let Step::Decision(_) = adv(g) {
        pass(g);
    }
}

#[test]
fn village_green_is_played_when_discarded_outside_clean_up() {
    let mut g = game(&[I::VILLAGE_GREEN, I::CELLAR]);
    set_hand(&mut g, 0, &[I::CELLAR, I::VILLAGE_GREEN, I::ESTATE]);
    play(&mut g, I::CELLAR);
    choose(&mut g, Choice::Card(I::VILLAGE_GREEN));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == I::VILLAGE_GREEN, "{d:?}");
    choose(&mut g, Choice::Yes);
    // Now or next turn?
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::VILLAGE_GREEN));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].in_play.get(I::VILLAGE_GREEN), 1);
    assert_eq!(g.players[0].discard.get(I::VILLAGE_GREEN), 0);
}

#[test]
fn village_green_discarded_in_clean_up_is_not_offered() {
    let mut g = game(&[I::VILLAGE_GREEN]);
    set_hand(&mut g, 0, &[I::VILLAGE_GREEN, I::ESTATE]);
    g.turn.phase = Phase::Buy;
    let mut guard = 0;
    while g.turn.player == 0 && guard < 10 {
        guard += 1;
        match adv(&mut g) {
            Step::Decision(d) => {
                assert!(!matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?}");
                pass(&mut g);
            }
            _ => {}
        }
    }
    assert_eq!(g.players[0].discard.get(I::VILLAGE_GREEN), 1);
}

#[test]
fn barge_now_draws_three_and_gives_a_buy() {
    let mut g = game(&[I::BARGE]);
    set_hand(&mut g, 0, &[I::BARGE]);
    play(&mut g, I::BARGE);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].hand.total(), 3);
    assert_eq!(g.turn.buys, 2);
}

#[test]
fn barge_for_next_turn_draws_at_the_start_of_it() {
    let mut g = game(&[I::BARGE]);
    set_hand(&mut g, 0, &[I::BARGE]);
    play(&mut g, I::BARGE);
    choose(&mut g, Choice::No);
    assert_eq!(g.players[0].hand.total(), 0);
    assert_eq!(g.turn.buys, 1);
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn coven_exiles_a_curse_for_each_other_player_and_when_there_is_none_they_discard_theirs() {
    let mut g = game(&[I::COVEN]);
    set_hand(&mut g, 0, &[I::COVEN]);
    play(&mut g, I::COVEN);
    assert_eq!(ex(&g, 1, I::CURSE), 1);
    assert_eq!(g.supply.get(I::CURSE), 9);
    assert_eq!(g.turn.coins, 2);
    // No Curse left: the Exiled Curses are discarded.
    let mut g = game(&[I::COVEN]);
    g.supply.set(I::CURSE, 0);
    g.players[1].men.exile.add(I::CURSE, 2);
    set_hand(&mut g, 0, &[I::COVEN]);
    play(&mut g, I::COVEN);
    assert_eq!(ex(&g, 1, I::CURSE), 0);
    assert_eq!(g.players[1].discard.get(I::CURSE), 2);
}

#[test]
fn displace_exiles_a_card_for_a_differently_named_one_costing_up_to_two_more() {
    let mut g = game(&[I::DISPLACE, I::SMITHY]);
    set_hand(&mut g, 0, &[I::DISPLACE, I::SILVER]);
    play(&mut g, I::DISPLACE);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(I::SMITHY)) && !cs.contains(&Choice::Card(I::SILVER)), "{cs:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(ex(&g, 0, I::SILVER), 1);
    assert_eq!(g.players[0].discard.get(I::SMITHY), 1);
}

#[test]
fn falconer_gains_a_cheaper_card_into_the_hand() {
    let mut g = game(&[I::FALCONER, I::SMITHY]);
    set_hand(&mut g, 0, &[I::FALCONER]);
    play(&mut g, I::FALCONER);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(I::SMITHY)) && !cs.contains(&Choice::Card(I::GOLD)), "{cs:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(g.players[0].hand.get(I::SMITHY), 1);
}

#[test]
fn falconer_is_played_when_a_card_with_two_types_is_gained_by_anyone() {
    let mut g = game(&[I::FALCONER, I::MILL, I::SMITHY]);
    set_hand(&mut g, 0, &[I::FALCONER]);
    g.gain_card(1, I::MILL, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == I::FALCONER && d.player == 0, "{d:?}");
    choose(&mut g, Choice::Yes);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }));
    // A card with one type does not bring it.
    let mut g = game(&[I::FALCONER, I::MILL, I::SMITHY]);
    set_hand(&mut g, 0, &[I::FALCONER]);
    g.gain_card(1, I::SMITHY, Dest::Discard, &mut NoEvents);
    assert!(g.pending_decision().is_none() || !matches!(expect_decision(&mut g).kind, DecisionKind::YesNo { .. }));
}

#[test]
fn gatekeeper_gives_three_dollars_next_turn_and_exiles_what_others_gain() {
    let mut g = game(&[I::GATEKEEPER, I::SMITHY]);
    set_hand(&mut g, 0, &[I::GATEKEEPER]);
    play(&mut g, I::GATEKEEPER);
    assert_eq!(g.players[0].men.gk, 1);
    // Another player buys an Action: it is Exiled instead.
    reset_turn(&mut g, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 20;
    g.turn.buys = 5;
    set_hand(&mut g, 1, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::SMITHY);
    assert_eq!(ex(&g, 1, I::SMITHY), 1);
    assert_eq!(g.players[1].discard.get(I::SMITHY), 0);
    // With a copy on the mat already the next one is gained (and the copy on the mat may be discarded: declined here).
    choose(&mut g, Choice::Card(I::SMITHY));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Discard }), "{d:?}");
    choose(&mut g, Choice::No);
    assert_eq!(ex(&g, 1, I::SMITHY), 1);
    assert_eq!(g.players[1].discard.get(I::SMITHY), 1);
    // A Victory card is gained as usual.
    buy(&mut g, I::ESTATE);
    assert_eq!(ex(&g, 1, I::ESTATE), 0);
}

#[test]
fn hunting_lodge_may_discard_the_hand_for_five_cards() {
    let mut g = game(&[I::HUNTING_LODGE]);
    set_hand(&mut g, 0, &[I::HUNTING_LODGE, I::COPPER, I::COPPER]);
    play(&mut g, I::HUNTING_LODGE);
    assert_eq!(g.turn.actions, 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Discard }));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].hand.total(), 5);
    assert_eq!(g.players[0].discard.get(I::COPPER), 2);
}

#[test]
fn kiln_gains_a_copy_of_the_next_card_played() {
    let mut g = game(&[I::KILN, I::SMITHY]);
    set_hand(&mut g, 0, &[I::KILN, I::SMITHY, I::SMITHY]);
    g.turn.actions = 3;
    play(&mut g, I::KILN);
    assert_eq!(g.turn.coins, 2);
    play(&mut g, I::SMITHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { optional: true, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(g.players[0].discard.get(I::SMITHY), 1, "the copy");
    // Only the next card.
    play(&mut g, I::SMITHY);
    assert_eq!(g.players[0].discard.get(I::SMITHY), 1);
}

#[test]
fn livery_gains_a_horse_for_each_card_costing_four_or_more_gained_this_turn() {
    let mut g = game(&[I::LIVERY, I::SLEIGH]);
    set_hand(&mut g, 0, &[I::LIVERY]);
    play(&mut g, I::LIVERY);
    assert_eq!(g.turn.coins, 3);
    g.gain_card(0, I::SILVER, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::HORSE), 0);
    g.gain_card(0, I::GOLD, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::HORSE), 1);
}

#[test]
fn mastermind_plays_an_action_three_times_next_turn() {
    let mut g = game(&[I::MASTERMIND, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::MASTERMIND]);
    play(&mut g, I::MASTERMIND);
    assert_eq!(g.players[0].pending_durations_len, 1);
    let mut cards = vec![I::VILLAGE, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE];
    cards.extend(junk());
    set_deck_known(&mut g, 0, &cards);
    let mut guard = 0;
    while !(g.turn.player == 0 && g.turn.number > 2) && guard < 60 {
        guard += 1;
        settle_pass(&mut g);
    }
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, .. }) && d.source == Some(I::MASTERMIND), "{d:?}");
    choose(&mut g, Choice::Card(I::VILLAGE));
    assert_eq!(g.turn.actions, 1 + 6, "Village three times: +2 Actions each, one of which was ... and 3 Cards");
    assert_eq!(g.players[0].hand.total(), 5 - 1 + 3);
}

#[test]
fn paddock_gains_two_horses_and_gives_an_action_per_empty_pile() {
    let mut g = game(&[I::PADDOCK, I::SMITHY]);
    empty_pile(&mut g, I::SMITHY);
    set_hand(&mut g, 0, &[I::PADDOCK]);
    play(&mut g, I::PADDOCK);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].discard.get(I::HORSE), 2);
    assert_eq!(g.turn.actions, 1, "1 -1 for playing it +1 pile");
}

#[test]
fn sanctuary_may_exile_a_card_from_the_hand() {
    let mut g = game(&[I::SANCTUARY]);
    set_hand(&mut g, 0, &[I::SANCTUARY, I::COPPER]);
    play(&mut g, I::SANCTUARY);
    assert_eq!((g.turn.actions, g.turn.buys), (1, 2));
    assert_eq!(g.players[0].hand.total(), 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Exile, min: 0, max: 1, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::COPPER));
    assert_eq!(ex(&g, 0, I::COPPER), 1);
}

#[test]
fn fisherman_and_destrier_cost_less_on_your_turn() {
    let mut g = game(&[I::FISHERMAN, I::DESTRIER]);
    g.players[0].discard.clear();
    assert_eq!(g.cost(I::FISHERMAN), 2, "empty discard pile: $3 less");
    g.players[0].discard.add(I::COPPER, 1);
    assert_eq!(g.cost(I::FISHERMAN), 5);
    assert_eq!(g.cost(I::DESTRIER), 6);
    g.gain_card(0, I::SILVER, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    g.gain_card(0, I::SILVER, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.cost(I::DESTRIER), 4);
}

#[test]
fn animal_fair_can_be_bought_by_trashing_an_action() {
    let mut g = game(&[I::ANIMAL_FAIR, I::SMITHY]);
    g.turn.phase = Phase::Buy;
    set_hand(&mut g, 0, &[I::SMITHY]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    let c = choices(&g);
    assert!(c.iter().any(|x| matches!(x, Choice::Alt(_))), "{c:?}");
    let alt = *c.iter().find(|x| matches!(x, Choice::Alt(_))).unwrap();
    choose(&mut g, alt);
    adv(&mut g);
    assert_eq!(g.trash.get(I::SMITHY), 1);
    assert_eq!(g.players[0].discard.get(I::ANIMAL_FAIR), 1);
    assert_eq!(g.turn.buys, 1, "the Buy was used");
}

#[test]
fn animal_fair_gives_four_dollars_and_a_buy_per_empty_pile() {
    let mut g = game(&[I::ANIMAL_FAIR, I::SMITHY]);
    empty_pile(&mut g, I::SMITHY);
    empty_pile(&mut g, I::VILLAGE);
    set_hand(&mut g, 0, &[I::ANIMAL_FAIR]);
    play(&mut g, I::ANIMAL_FAIR);
    assert_eq!(g.turn.coins, 4);
    assert!(g.turn.buys >= 2);
}

#[test]
fn wayfarer_draws_three_and_may_gain_a_silver() {
    let mut g = game(&[I::WAYFARER]);
    set_hand(&mut g, 0, &[I::WAYFARER]);
    play(&mut g, I::WAYFARER);
    assert_eq!(g.players[0].hand.total(), 3);
    choose(&mut g, Choice::Card(I::SILVER));
    assert_eq!(g.players[0].discard.get(I::SILVER), 1);
}

// ===========================================================================
// Events
// ===========================================================================

fn at_buy(kingdom: &[CardId], coins: u16, hand: &[CardId]) -> GameState {
    let mut g = game(kingdom);
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
    g.turn.buys = 2;
    set_hand(&mut g, 0, hand);
    g
}

#[test]
fn ride_gains_a_horse() {
    let mut g = at_buy(&[I::RIDE], 2, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::RIDE);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::HORSE), 1);
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn desperation_gains_a_curse_once_per_turn_for_a_buy_and_two_dollars() {
    let mut g = at_buy(&[I::DESPERATION], 0, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::DESPERATION);
    choose(&mut g, Choice::Card(I::CURSE));
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::CURSE), 1);
    assert_eq!((g.turn.coins, g.turn.buys), (2, 2), "the Buy used, +1 Buy");
    assert!(!choices(&g).contains(&Choice::Card(I::DESPERATION)), "once per turn");
}

#[test]
fn delay_sets_aside_an_action_and_plays_it_next_turn() {
    let mut g = game(&[I::DELAY, I::SMITHY]);
    g.turn.phase = Phase::Buy;
    set_hand(&mut g, 0, &[I::SMITHY, I::ESTATE]);
    expect_decision(&mut g);
    buy(&mut g, I::DELAY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { filter: Filter::Action, min: 0, max: 1, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(g.players[0].hand.get(I::SMITHY), 0);
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn stampede_puts_five_horses_on_the_deck_with_five_or_fewer_cards_in_play() {
    let mut g = at_buy(&[I::STAMPEDE], 5, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::STAMPEDE);
    adv(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::HORSE));
    assert_eq!(g.supply.get(I::HORSE), 25);
    // With six in play: nothing.
    let mut g = at_buy(&[I::STAMPEDE], 5, &[]);
    set_in_play(&mut g, 0, &[I::COPPER; 6]);
    expect_decision(&mut g);
    buy(&mut g, I::STAMPEDE);
    adv(&mut g);
    assert_eq!(g.supply.get(I::HORSE), 30);
}

#[test]
fn bargain_gains_a_non_victory_card_and_gives_the_others_a_horse() {
    let mut g = at_buy(&[I::BARGAIN, I::SMITHY, I::SLEIGH], 4, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::BARGAIN);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(I::SMITHY)) && !cs.contains(&Choice::Card(I::ESTATE)), "{cs:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(I::HORSE), 1);
    assert_eq!(g.players[0].discard.get(I::SMITHY), 1);
}

#[test]
fn enclave_gains_a_gold_and_exiles_a_duchy() {
    let mut g = at_buy(&[I::ENCLAVE], 8, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::ENCLAVE);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::GOLD), 1);
    assert_eq!(ex(&g, 0, I::DUCHY), 1);
}

#[test]
fn alliance_gains_six_cards() {
    let mut g = at_buy(&[I::ALLIANCE], 10, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::ALLIANCE);
    adv(&mut g);
    let d = &g.players[0].discard;
    assert_eq!([d.get(I::PROVINCE), d.get(I::DUCHY), d.get(I::ESTATE), d.get(I::GOLD), d.get(I::SILVER), d.get(I::COPPER)], [1; 6]);
}

#[test]
fn seize_the_day_is_once_per_game_and_gives_an_extra_turn() {
    let mut g = at_buy(&[I::SEIZE_THE_DAY], 4, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::SEIZE_THE_DAY);
    let mut guard = 0;
    while g.turn.number < 2 && guard < 40 {
        guard += 1;
        settle_pass(&mut g);
    }
    assert_eq!(g.turn.player, 0, "the extra turn is player 0's");
    expect_decision(&mut g);
    assert!(!choices(&g).contains(&Choice::Card(I::SEIZE_THE_DAY)));
}

#[test]
fn banish_exiles_all_copies_of_a_card_from_the_hand() {
    let mut g = at_buy(&[I::BANISH], 4, &[I::DUCHY, I::DUCHY, I::ESTATE]);
    expect_decision(&mut g);
    buy(&mut g, I::BANISH);
    choose(&mut g, Choice::Card(I::DUCHY)); // the name
    choose(&mut g, Choice::Card(I::DUCHY));
    adv(&mut g);
    assert_eq!(ex(&g, 0, I::DUCHY), 2);
    assert_eq!(g.players[0].hand.total(), 1);
}

#[test]
fn commerce_gains_a_gold_per_differently_named_card_gained() {
    let mut g = at_buy(&[I::COMMERCE, I::SMITHY], 5, &[]);
    expect_decision(&mut g);
    g.turn.buys = 3;
    g.gain_card(0, I::SMITHY, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    g.gain_card(0, I::SMITHY, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    g.gain_card(0, I::SILVER, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    buy(&mut g, I::COMMERCE);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::GOLD), 2, "Smithy and Silver: two differently named cards");
}

#[test]
fn reap_gains_a_gold_and_plays_it_next_turn() {
    let mut g = at_buy(&[I::REAP], 7, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::REAP);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::GOLD), 0);
    assert_eq!(g.players[0].pending_durations_len, 1);
}

#[test]
fn demand_gains_a_horse_and_a_card_onto_the_deck() {
    let mut g = at_buy(&[I::DEMAND, I::SMITHY, I::SLEIGH], 5, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::DEMAND);
    choose(&mut g, Choice::Card(I::SMITHY));
    adv(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::SMITHY));
    assert_eq!(g.supply.get(I::HORSE), 29);
}

#[test]
fn enhance_trashes_a_non_victory_card_for_one_costing_up_to_two_more() {
    let mut g = at_buy(&[I::ENHANCE, I::SMITHY], 3, &[I::SMITHY, I::ESTATE]);
    expect_decision(&mut g);
    buy(&mut g, I::ENHANCE);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(I::SMITHY)) && !cs.contains(&Choice::Card(I::ESTATE)), "{cs:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(I::GOLD)) && !cs.contains(&Choice::Card(I::PROVINCE)), "{cs:?}");
    choose(&mut g, Choice::Card(I::GOLD));
    adv(&mut g);
    assert_eq!(g.trash.get(I::SMITHY), 1);
    assert_eq!(g.players[0].discard.get(I::GOLD), 1);
}

#[test]
fn toil_plays_an_action_from_the_hand_and_march_from_the_discard_pile() {
    let mut g = at_buy(&[I::TOIL, I::SMITHY], 2, &[I::SMITHY]);
    expect_decision(&mut g);
    buy(&mut g, I::TOIL);
    choose(&mut g, Choice::Card(I::SMITHY));
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 3);
    let mut g = at_buy(&[I::MARCH, I::SMITHY], 3, &[]);
    set_discard(&mut g, 0, &[I::SMITHY]);
    expect_decision(&mut g);
    buy(&mut g, I::MARCH);
    choose(&mut g, Choice::Card(I::SMITHY));
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn transport_exiles_an_action_from_the_supply_or_returns_an_exiled_one_to_the_deck() {
    let mut g = at_buy(&[I::TRANSPORT, I::SMITHY], 3, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::TRANSPORT);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    choose(&mut g, Choice::Mode(0));
    adv(&mut g);
    assert_eq!(ex(&g, 0, I::SMITHY), 1);
}

#[test]
fn gamble_may_play_the_top_card_if_it_is_an_action_or_treasure() {
    let mut g = at_buy(&[I::GAMBLE], 2, &[]);
    set_deck_known(&mut g, 0, &[I::GOLD]);
    expect_decision(&mut g);
    buy(&mut g, I::GAMBLE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.coins, 3);
}

#[test]
fn pursue_names_a_card_and_puts_matches_back() {
    let mut g = at_buy(&[I::PURSUE], 2, &[]);
    set_deck_known(&mut g, 0, &[I::ESTATE, I::ESTATE, I::DUCHY, I::ESTATE]);
    expect_decision(&mut g);
    buy(&mut g, I::PURSUE);
    let mut guard = 0;
    while let Some(d) = g.pending_decision() {
        if matches!(d.kind, DecisionKind::Buy) || guard > 12 {
            break;
        }
        let cs = choices(&g);
        let c = cs.iter().copied().find(|c| *c == Choice::Card(I::ESTATE)).unwrap_or(cs[0]);
        choose(&mut g, c);
        guard += 1;
    }
    assert_eq!(g.players[0].discard.get(I::DUCHY), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::ESTATE));
}

#[test]
fn populate_gains_one_of_each_action_supply_pile() {
    let mut g = at_buy(&[I::POPULATE, I::SMITHY, I::VILLAGE], 10, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::POPULATE);
    let mut guard = 0;
    while let Some(d) = g.pending_decision() {
        if matches!(d.kind, DecisionKind::Buy) || guard > 12 {
            break;
        }
        let cs = choices(&g);
        let c = cs.iter().copied().find(|c| matches!(c, Choice::Card(_))).unwrap_or(cs[0]);
        choose(&mut g, c);
        guard += 1;
    }
    assert_eq!(g.players[0].discard.get(I::SMITHY), 1);
    assert_eq!(g.players[0].discard.get(I::VILLAGE), 1);
}

#[test]
fn invest_draws_two_cards_when_another_player_gains_the_exiled_card() {
    let mut g = at_buy(&[I::INVEST, I::SMITHY], 4, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::INVEST); // the only Action pile is Exiled at once
    adv(&mut g);
    assert_eq!(ex(&g, 0, I::SMITHY), 1);
    let before = g.players[0].hand.total();
    reset_turn(&mut g, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 4;
    g.turn.buys = 1;
    set_hand(&mut g, 1, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::SMITHY);
    assert_eq!(g.players[0].hand.total(), before + 2);
}

// ===========================================================================
// Ways
// ===========================================================================

fn with_way(way: CardId, kingdom: &[CardId], hand: &[CardId]) -> GameState {
    let mut k = kingdom.to_vec();
    k.push(way);
    let mut g = game(&k);
    set_hand(&mut g, 0, hand);
    g
}

/// Play `card` as `way` (the Way decision's choice is the Way's id).
fn play_as(g: &mut GameState, card: CardId, way: CardId) {
    play(g, card);
    let d = expect_decision(g);
    assert!(matches!(d.kind, DecisionKind::Way), "{d:?}");
    choose(g, Choice::Card(way));
}

#[test]
fn a_way_is_offered_when_a_card_is_played_and_none_leaves_the_card_alone() {
    let mut g = with_way(I::WAY_OF_THE_SHEEP, &[I::SMITHY], &[I::SMITHY]);
    play(&mut g, I::SMITHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Way));
    assert!(choices(&g).contains(&Choice::Card(I::WAY_OF_THE_SHEEP)));
    choose(&mut g, Choice::Pass);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn no_way_decision_in_a_game_without_ways() {
    let mut g = game(&[I::SMITHY]);
    set_hand(&mut g, 0, &[I::SMITHY]);
    play(&mut g, I::SMITHY);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn simple_ways_give_their_bonuses() {
    let table: &[(CardId, u8, u8, u8, u16)] = &[
        // way, hand size after, actions, buys, coins
        (I::WAY_OF_THE_SHEEP, 0, 0, 1, 2),
        (I::WAY_OF_THE_OTTER, 2, 0, 1, 0),
        (I::WAY_OF_THE_OX, 0, 2, 1, 0),
        (I::WAY_OF_THE_PIG, 1, 1, 1, 0),
        (I::WAY_OF_THE_MULE, 0, 1, 1, 1),
        (I::WAY_OF_THE_MONKEY, 0, 0, 2, 1),
        (I::WAY_OF_THE_OWL, 6, 0, 1, 0),
        (I::WAY_OF_THE_HORSE, 2, 1, 1, 0),
    ];
    for &(way, hand, actions, buys, coins) in table {
        let mut g = with_way(way, &[I::SMITHY], &[I::SMITHY]);
        play_as(&mut g, I::SMITHY, way);
        adv(&mut g);
        assert_eq!(g.players[0].hand.total() as u8, hand, "{}", cards::name(way));
        assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (actions, buys, coins), "{}", cards::name(way));
    }
}

#[test]
fn way_of_the_horse_returns_the_card_to_its_pile() {
    let mut g = with_way(I::WAY_OF_THE_HORSE, &[I::SMITHY], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_HORSE);
    adv(&mut g);
    assert_eq!(g.supply.get(I::SMITHY), 11 - 1 + 1 - 1 + 1 - 0 + 0 - 0, "back in the Smithy pile");
}

#[test]
fn way_of_the_camel_and_worm_exile_from_the_supply() {
    let mut g = with_way(I::WAY_OF_THE_CAMEL, &[I::SMITHY], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_CAMEL);
    adv(&mut g);
    assert_eq!(ex(&g, 0, I::GOLD), 1);
    let mut g = with_way(I::WAY_OF_THE_WORM, &[I::SMITHY], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_WORM);
    adv(&mut g);
    assert_eq!(ex(&g, 0, I::ESTATE), 1);
}

#[test]
fn way_of_the_mole_discards_the_hand_and_draws_three() {
    let mut g = with_way(I::WAY_OF_THE_MOLE, &[I::SMITHY], &[I::SMITHY, I::COPPER, I::COPPER]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_MOLE);
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 3);
    assert_eq!(g.players[0].discard.get(I::COPPER), 2);
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn way_of_the_goat_trashes_a_card_from_the_hand() {
    let mut g = with_way(I::WAY_OF_THE_GOAT, &[I::SMITHY], &[I::SMITHY, I::COPPER]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_GOAT);
    choose(&mut g, Choice::Card(I::COPPER));
    assert_eq!(g.trash.get(I::COPPER), 1);
}

#[test]
fn way_of_the_squirrel_draws_at_the_end_of_the_turn() {
    let mut g = with_way(I::WAY_OF_THE_SQUIRREL, &[I::SMITHY], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_SQUIRREL);
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 0);
    let mut guard = 0;
    while g.turn.player == 0 && guard < 20 {
        guard += 1;
        settle_pass(&mut g);
    }
    assert_eq!(g.players[0].hand.total(), 5 + 2, "the next hand and Squirrel's two");
}

#[test]
fn way_of_the_turtle_sets_the_card_aside_for_next_turn() {
    let mut g = with_way(I::WAY_OF_THE_TURTLE, &[I::SMITHY], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_TURTLE);
    adv(&mut g);
    assert_eq!(g.players[0].in_play.get(I::SMITHY), 0);
    assert_eq!(g.players[0].pending_durations_len, 1);
    assert_eq!(g.players[0].all_cards().get(I::SMITHY), 1);
}

#[test]
fn way_of_the_frog_puts_the_card_on_the_deck_when_discarded_from_play() {
    let mut g = with_way(I::WAY_OF_THE_FROG, &[I::SMITHY], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_FROG);
    adv(&mut g);
    assert_eq!(g.turn.actions, 1);
    let mut guard = 0;
    let mut saw = false;
    while g.turn.player == 0 && guard < 20 {
        guard += 1;
        if let Step::Decision(d) = adv(&mut g) {
            if matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, .. }) {
                saw = true;
            }
            let c = choices(&g);
            choose(&mut g, *c.iter().find(|c| matches!(c, Choice::Card(_))).unwrap_or(&Choice::Pass));
        }
    }
    let _ = saw;
    assert_eq!(g.players[0].hand.get(I::SMITHY), 1, "Smithy went back to the deck and was drawn again");
}

#[test]
fn way_of_the_rat_may_discard_a_treasure_for_a_copy_of_the_card() {
    let mut g = with_way(I::WAY_OF_THE_RAT, &[I::SMITHY], &[I::SMITHY, I::COPPER]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_RAT);
    choose(&mut g, Choice::Card(I::COPPER));
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::SMITHY), 1);
}

#[test]
fn way_of_the_butterfly_returns_the_card_for_one_costing_exactly_one_more() {
    let mut g = with_way(I::WAY_OF_THE_BUTTERFLY, &[I::SMITHY, I::MARKET], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_BUTTERFLY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?}");
    choose(&mut g, Choice::Yes);
    let cs = choices(&g);
    assert!(cs.iter().all(|c| match c { Choice::Card(x) => g.cost(*x) == 5, _ => true }), "{cs:?}");
}

#[test]
fn way_of_the_seal_lets_gains_go_onto_the_deck() {
    let mut g = with_way(I::WAY_OF_THE_SEAL, &[I::SMITHY], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_SEAL);
    adv(&mut g);
    assert_eq!(g.turn.coins, 1);
    g.turn.phase = Phase::Buy;
    g.turn.buys = 2;
    expect_decision(&mut g);
    buy(&mut g, I::COPPER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::COPPER));
}

#[test]
fn way_of_the_mouse_plays_the_set_aside_card() {
    let mut g = with_way(I::WAY_OF_THE_MOUSE, &[I::SMITHY], &[I::SMITHY]);
    let aside = g.men.mouse;
    assert_ne!(aside, 0, "set-up chose a card to set aside");
    assert!(cards::cost(aside) == 2 || cards::cost(aside) == 3);
    let mut ev: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(I::SMITHY), &mut ev);
    choose_ev(&mut g, Choice::Card(I::WAY_OF_THE_MOUSE), &mut ev);
    let mut guard = 0;
    while g.turn.phase == Phase::Action && guard < 8 {
        guard += 1;
        if g.pending_decision().map_or(true, |d| matches!(d.kind, DecisionKind::PlayAction)) {
            break;
        }
        let c = choices(&g)[0];
        choose_ev(&mut g, c, &mut ev);
    }
    assert!(ev.iter().any(|e| matches!(e, Event::StartPlay { card, .. } if *card == aside)), "the set-aside card was played: {ev:?}");
    assert_eq!(g.players[0].hand.get(I::SMITHY), 0);
}

#[test]
fn way_of_the_chameleon_swaps_cards_and_coins() {
    let mut g = with_way(I::WAY_OF_THE_CHAMELEON, &[I::SMITHY, I::MARKET], &[I::SMITHY]);
    play_as(&mut g, I::SMITHY, I::WAY_OF_THE_CHAMELEON);
    adv(&mut g);
    assert_eq!(g.turn.coins, 3, "+3 Cards became +$3");
    assert_eq!(g.players[0].hand.total(), 0);
}

#[test]
fn a_way_played_card_is_not_the_card() {
    // Way of the Sheep on Throne Room: Throne Room is not played as itself, so no card is doubled.
    let mut g = with_way(I::WAY_OF_THE_SHEEP, &[I::THRONE_ROOM, I::SMITHY], &[I::THRONE_ROOM, I::SMITHY]);
    play_as(&mut g, I::THRONE_ROOM, I::WAY_OF_THE_SHEEP);
    adv(&mut g);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].hand.get(I::SMITHY), 1);
}

#[test]
fn throne_room_plays_a_card_twice_each_with_its_own_way() {
    let mut g = with_way(I::WAY_OF_THE_SHEEP, &[I::THRONE_ROOM, I::SMITHY], &[I::THRONE_ROOM, I::SMITHY]);
    play(&mut g, I::THRONE_ROOM);
    // Throne Room itself may be played as a Way first.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Way));
    choose(&mut g, Choice::Pass);
    choose(&mut g, Choice::Card(I::SMITHY));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Way));
    choose(&mut g, Choice::Card(I::WAY_OF_THE_SHEEP));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Way));
    choose(&mut g, Choice::Pass);
    adv(&mut g);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].hand.total(), 3, "the second play drew 3");
}

#[test]
fn text_round_trip_keeps_exile_horses_and_ways() {
    let mut g = with_way(I::WAY_OF_THE_OWL, &[I::SMITHY, I::SLEIGH, I::STOCKPILE], &[I::SMITHY]);
    g.players[0].men.exile.add(I::SMITHY, 2);
    g.players[1].men.exile.add(I::CURSE, 1);
    let text = format_state(&g);
    let h = parse_state(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(strip_seed(&format_state(&h)), strip_seed(&text));
    assert_eq!(h.players[0].men.exile.get(I::SMITHY), 2);
    assert_eq!(h.supply.get(I::HORSE), g.supply.get(I::HORSE));
}

fn strip_seed(s: &str) -> String {
    s.lines().filter(|l| !l.starts_with("seed:")).collect::<Vec<_>>().join("
")
}

// ===========================================================================
// Stop-moving rule: a card that moves itself leaves a same-named copy elsewhere alone
// ===========================================================================

#[test]
fn a_horse_played_by_throne_room_returns_but_a_same_named_horse_elsewhere_stays() {
    let mut g = game(&[I::SLEIGH, I::THRONE_ROOM]);
    set_hand(&mut g, 0, &[I::THRONE_ROOM, I::HORSE]);
    set_in_play(&mut g, 0, &[I::HORSE]); // another Horse already in play (say, played by a Way earlier)
    g.supply.set(I::HORSE, 28);
    let before = total_cards(&g);
    play(&mut g, I::THRONE_ROOM);
    choose(&mut g, Choice::Card(I::HORSE));
    assert_eq!(g.players[0].in_play.get(I::HORSE), 1, "the other Horse is still in play");
    assert_eq!(g.supply.get(I::HORSE), 29, "only the one played returned");
    assert_eq!(total_cards(&g), before);
}

#[test]
fn way_of_the_horse_on_a_throned_card_returns_the_copy_played_only() {
    let mut g = with_way(I::WAY_OF_THE_HORSE, &[I::SMITHY, I::THRONE_ROOM], &[I::THRONE_ROOM, I::SMITHY]);
    set_in_play(&mut g, 0, &[I::SMITHY]);
    let before = total_cards(&g);
    play(&mut g, I::THRONE_ROOM);
    choose(&mut g, Choice::Pass); // Throne Room itself as itself
    choose(&mut g, Choice::Card(I::SMITHY));
    let mut guard = 0;
    while let Some(d) = g.pending_decision() {
        if !matches!(d.kind, DecisionKind::Way) || guard > 4 {
            break;
        }
        choose(&mut g, Choice::Card(I::WAY_OF_THE_HORSE));
        guard += 1;
    }
    adv(&mut g);
    assert_eq!(g.players[0].in_play.get(I::SMITHY), 1, "the Smithy that was already in play is untouched");
    assert_eq!(total_cards(&g), before);
}

#[test]
fn exile_discard_on_gain_conserves_cards() {
    let mut g = game(&[I::STOCKPILE]);
    g.players[0].men.exile.add(I::SILVER, 3);
    g.men.hook |= dominion_engine::state::H_EXILE;
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    set_hand(&mut g, 0, &[]);
    let before = total_cards(&g);
    expect_decision(&mut g);
    buy(&mut g, I::SILVER);
    choose(&mut g, Choice::Yes);
    assert_eq!(ex(&g, 0, I::SILVER), 0);
    assert_eq!(g.players[0].discard.get(I::SILVER), 4);
    assert_eq!(total_cards(&g), before, "bought from the Supply: nothing is made");
}
