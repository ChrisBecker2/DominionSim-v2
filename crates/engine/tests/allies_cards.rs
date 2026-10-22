//! Allies tests (rulings in `docs/allies-plan.md`): Favors, the 23 Allies, the split piles and their rotation, and the 25 + 24 kingdom cards.

mod common;
use common::*;
use dominion_engine::cards::{self, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

use dominion_engine::cards::id as I;

// ===========================================================================
// Random games: nothing is lost, nothing panics, every game ends
// ===========================================================================

/// The split piles' bookkeeping is consistent: each card's count is what the pile's order holds, and the order is as long as the cards left.
fn check_piles(g: &GameState, context: &str) {
    for i in 0..6 {
        let first = cards::ALLIES_SPLIT_FIRST + 4 * i as CardId;
        if !g.in_supply(first) {
            continue;
        }
        let (order, len) = g.al_pile_cards(i);
        let left: u32 = (0..4).map(|k| g.supply.get(first + k) as u32).sum();
        assert_eq!(len as u32, left, "pile {i}: the order has {len} cards, the counts {left}: {context}");
        for k in 0..4u16 {
            let n = order[..len].iter().filter(|&&c| c == first + k).count() as u8;
            assert_eq!(n, g.supply.get(first + k), "pile {i} card {k}: {context}");
        }
        // Only the top card is open.
        for k in 0..4u16 {
            let open = g.pile_open(first + k);
            assert_eq!(open, len > 0 && order[0] == first + k, "pile {i} card {k} open: {context}");
        }
    }
}

#[test]
fn random_allies_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0xA11E5);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Allies).collect();
    assert_eq!(pool.len(), 31);
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::Allies).collect();
    let allies: Vec<CardId> = (cards::FIRST_ALLY..cards::FIRST_ALLY + cards::NUM_ALLIES as CardId).collect();
    let mut played = std::collections::BTreeSet::new();
    let mut spent_on = std::collections::BTreeSet::new();
    let mut ally_games = std::collections::BTreeSet::new();
    let games: u64 = std::env::var("ALLIES_GAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
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
        // Every game has an Ally (the Allies are dealt in turn, so each is used often).
        let ally = allies[(game % 23) as usize];
        kingdom.push(ally);
        ally_games.insert(ally);
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom: kingdom.clone(), seed: game, max_turns: 300, ..Default::default() });
        assert_eq!(g.al.ally, ally);
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
                    Event::Favor { source, gained: false, .. } => {
                        spent_on.insert(source);
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
                    // Some games are rich in Favors, so that the Allies are used.
                    if game % 2 == 0 && d.kind == DecisionKind::PlayAction && g.players[g.turn.player as usize].al.favors < 12 {
                        g.players[g.turn.player as usize].al.favors = 12;
                    }
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
            if g.stack.is_empty() {
                check_piles(&g, &format!("game {game}, chose {chosen:?}, recent {recent:#?}"));
            }
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
        // The end of the game: the empty-pile count agrees with the plain scan.
        assert_eq!(g.empty_piles(), g.empty_piles_scan());
    }
    if games >= 3000 {
        let missing: Vec<&str> = (I::BAUBLE..=I::LICH).filter(|c| !played.contains(c) && !cards::is(*c, cards::VICTORY)).map(cards::name).collect();
        assert!(missing.is_empty(), "never played: {missing:?}");
        let used: Vec<&str> = (cards::FIRST_ALLY..cards::FIRST_ALLY + cards::NUM_ALLIES as CardId).filter(|a| !spent_on.contains(a)).map(cards::name).collect();
        // Plateau Shepherds and League of Bankers/Shopkeepers spend no Favors.
        let allowed = [I::PLATEAU_SHEPHERDS, I::LEAGUE_OF_BANKERS, I::LEAGUE_OF_SHOPKEEPERS];
        let used: Vec<&str> = used.into_iter().filter(|n| !allowed.iter().any(|&a| cards::name(a) == *n)).collect();
        assert!(used.is_empty(), "Favors never spent on: {used:?}");
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// A deck of cards that are neither Treasures nor Actions: every draw is visible, nothing is auto-played.
fn junk() -> Vec<CardId> {
    [I::ESTATE, I::DUCHY, I::PROVINCE].iter().cycle().take(40).copied().collect()
}

/// Player 0's turn in a game with `kingdom` (2 players), the decks of junk.
fn game(kingdom: &[CardId]) -> GameState {
    // A kingdom with a Liaison and no Ally gets one from the seed: pin a harmless one (it only scores) so that a test sees only its own card.
    let mut k = kingdom.to_vec();
    if k.iter().any(|&c| cards::is(c, cards::LIAISON)) && !k.iter().any(|&c| cards::is_ally(c)) {
        k.push(I::PLATEAU_SHEPHERDS);
    }
    let mut g = new_state(&k, 2);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    g
}

fn favors(g: &GameState, p: usize) -> u8 {
    g.players[p].al.favors
}

// ===========================================================================
// Set-up
// ===========================================================================

#[test]
fn the_ids_and_piles_line_up() {
    assert_eq!(cards::set_by_name("allies"), Some(CardSet::Allies));
    assert_eq!(cards::kingdom_cards_in(CardSet::Allies).count(), 31);
    assert_eq!(cards::LAST_KINGDOM, I::LICH);
    assert!(cards::is_ally(I::ARCHITECTS_GUILD) && cards::is_ally(I::WOODWORKERS_GUILD) && !cards::is_ally(I::EXILE));
    assert_eq!(I::WOODWORKERS_GUILD as usize + 1, cards::NUM_CARDS);
    assert_eq!(I::EXILE + 1, I::ARCHITECTS_GUILD);
    // A split pile is named by its first card; the other cards of the pile are not kingdom cards.
    assert!(cards::is_kingdom(I::TOWN_CRIER) && !cards::is_kingdom(I::BLACKSMITH) && !cards::is_kingdom(I::LICH) && cards::is_kingdom(I::STUDENT));
    assert_eq!(cards::pile_of(I::LICH), I::STUDENT);
    assert_eq!(cards::by_name("Wizards"), Some(I::STUDENT));
    assert_eq!(cards::by_name("townsfolk"), Some(I::TOWN_CRIER));
    assert_eq!(cards::sub_type(I::LICH), 6);
    assert_eq!(cards::sub_type(I::TOWN), 0);
    // The Ruins and Knights stay the highest zone ids; the zone ids fit the lanes with the bookkeeping lanes to spare.
    assert!(cards::NUM_ZONE_IDS < dominion_engine::counts::LANES - 4);
    assert_eq!(cards::NUM_ZONE_IDS, I::SIR_VANDER as usize + 1);
    for c in I::BAUBLE..=I::LICH {
        assert_eq!(cards::set_of(c), CardSet::Allies);
    }
}

#[test]
fn a_game_with_a_split_pile_has_its_four_cards_in_order() {
    let g = new_state(&[I::TOWN_CRIER, I::STUDENT, I::VILLAGE], 2);
    for c in [I::TOWN_CRIER, I::BLACKSMITH, I::MILLER, I::ELDER, I::STUDENT, I::CONJURER, I::SORCERER, I::LICH] {
        assert!(g.in_supply(c), "{}", cards::name(c));
        assert_eq!(g.supply.get(c), 4, "{}", cards::name(c));
    }
    assert!(!g.in_supply(I::HERB_GATHERER), "the Augurs are not in this game");
    assert_eq!(g.al_top(4), Some(I::TOWN_CRIER));
    assert_eq!(g.al_top(5), Some(I::STUDENT));
    assert!(g.pile_open(I::TOWN_CRIER) && !g.pile_open(I::BLACKSMITH) && !g.pile_open(I::LICH));
    // Another card of the pile names it in a kingdom too.
    let g2 = new_state(&[I::LICH], 2);
    assert!(g2.in_supply(I::STUDENT));
    // A pile counts once when it is empty.
    check_piles(&g, "new");
}

#[test]
fn every_player_starts_with_a_favor_and_the_importer_gives_five() {
    let g = GameState::new(&GameConfig { kingdom: vec![I::UNDERLING, I::VILLAGE, I::CAVE_DWELLERS], ..Default::default() });
    assert_eq!(g.al.ally, I::CAVE_DWELLERS);
    assert_eq!((favors(&g, 0), favors(&g, 1)), (1, 1));
    let g = GameState::new(&GameConfig { kingdom: vec![I::IMPORTER, I::VILLAGE, I::MARKET_TOWNS], ..Default::default() });
    assert_eq!((favors(&g, 0), favors(&g, 1)), (5, 5));
    // No Liaison, no Ally: no Favors.
    let g = GameState::new(&GameConfig { kingdom: vec![I::TOWN, I::VILLAGE], ..Default::default() });
    assert_eq!(g.al.ally, 0);
    assert_eq!(favors(&g, 0), 0);
}

#[test]
fn a_random_kingdom_with_a_liaison_gets_one_ally() {
    for seed in 0..40 {
        let mut rng = Rng::new(seed);
        let mut k = vec![I::UNDERLING, I::VILLAGE];
        cards::add_random_ally(&mut k, &mut rng);
        assert_eq!(k.iter().filter(|&&c| cards::is_ally(c)).count(), 1);
        let mut rng = Rng::new(seed);
        let mut k = vec![I::TOWN, I::VILLAGE];
        cards::add_random_ally(&mut k, &mut rng);
        assert_eq!(k.len(), 2, "no Liaison, no Ally");
        // A kingdom that names an Ally keeps it.
        let mut k = vec![I::STUDENT, I::MARKET_TOWNS];
        cards::add_random_ally(&mut k, &mut rng);
        assert_eq!(k.len(), 2);
    }
}

#[test]
fn the_state_round_trips_through_text() {
    let mut g = new_state(&[I::TOWN_CRIER, I::STUDENT, I::VILLAGE, I::FAMILY_OF_INVENTORS, I::GALLERIA], 2);
    g.players[0].al.favors = 7;
    g.players[1].al.favors = 2;
    g.al.fam.add(I::VILLAGE, 2);
    g.turn.al.galleria = 1;
    g.turn.al.live = 1;
    g.players[0].al.keep.add(I::GOLD, 1);
    g.players[0].al.skips = 1;
    g.al_update_hook();
    // A rotated split pile.
    g.al_rotate_for(0, I::TOWN_CRIER, I::TOWN_CRIER, &mut NoEvents);
    let text = format_state(&g);
    assert!(text.contains("ally: Family of Inventors"), "{text}");
    assert!(text.contains("pile order: 4 Blacksmith, 4 Miller, 4 Elder, 4 Town Crier"), "{text}");
    let back = parse_state(&text).unwrap();
    assert!(back.al == g.al, "{text}");
    assert!(back.players[0].al == g.players[0].al);
    assert!(back.turn.al == g.turn.al);
    // The seed is the generator's state, which the text keeps as it was at the start; every other line must match.
    let strip = |s: &str| s.lines().filter(|l| !l.starts_with("seed:")).collect::<Vec<_>>().join("
");
    assert_eq!(strip(&format_state(&back)), strip(&text));
}

fn hand(g: &GameState, c: CardId) -> u8 {
    g.players[0].hand.get(c)
}

/// The next decision has this source; answer it.
fn answer(g: &mut GameState, source: CardId, c: Choice) {
    let d = expect_decision(g);
    assert_eq!(d.source, Some(source), "decision {d:?}");
    choose(g, c);
}

// ===========================================================================
// Favors and the Liaisons
// ===========================================================================

#[test]
fn underling_draws_and_gives_a_favor() {
    let mut g = game(&[I::UNDERLING]);
    set_hand(&mut g, 0, &[I::UNDERLING]);
    play(&mut g, I::UNDERLING);
    assert_eq!(g.players[0].hand.total(), 1);
    assert_eq!(g.turn.actions, 1);
    assert_eq!(favors(&g, 0), 1);
}

#[test]
fn sycophant_discards_three_for_three_coins_and_gives_favors_when_gained() {
    let mut g = game(&[I::SYCOPHANT]);
    set_hand(&mut g, 0, &[I::SYCOPHANT, I::ESTATE, I::DUCHY, I::DUCHY, I::PROVINCE]);
    play(&mut g, I::SYCOPHANT);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 3, max: 3, .. }), "{d:?}");
    // (The picks are in canonical order, so the rest of the hand can be forced once the first is chosen.)
    while matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Select { act: Act::Discard, .. }, .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.players[0].hand.total(), 1);
    assert_eq!(g.turn.coins, 3);
    assert_eq!(g.players[0].hand.total(), 1);
    let mut g = game(&[I::SYCOPHANT]);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 2;
    buy(&mut g, I::SYCOPHANT);
    assert_eq!(favors(&g, 0), 2);
}

#[test]
fn sycophant_with_fewer_than_three_cards_discards_them_all() {
    let mut g = game(&[I::SYCOPHANT]);
    set_hand(&mut g, 0, &[I::SYCOPHANT, I::ESTATE]);
    play(&mut g, I::SYCOPHANT);
    adv(&mut g);
    assert_eq!(g.turn.coins, 3);
    assert_eq!(g.players[0].hand.total(), 0);
    // Nothing discarded: no coins.
    let mut g = game(&[I::SYCOPHANT]);
    set_hand(&mut g, 0, &[I::SYCOPHANT]);
    play(&mut g, I::SYCOPHANT);
    adv(&mut g);
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn merchant_camp_gives_actions_and_coins_and_may_go_onto_the_deck_when_discarded() {
    let mut g = game(&[I::MERCHANT_CAMP]);
    set_hand(&mut g, 0, &[I::MERCHANT_CAMP]);
    play(&mut g, I::MERCHANT_CAMP);
    assert_eq!((g.turn.actions, g.turn.coins), (2, 1));
    g.turn.phase = Phase::Buy;
    // Clean-up: the offer; it is drawn again with the next hand, so look at the deck right after saying yes.
    loop {
        let d = expect_decision(&mut g);
        if d.source == Some(I::MERCHANT_CAMP) {
            choose(&mut g, Choice::Yes);
            break;
        }
        choose(&mut g, Choice::Pass);
    }
    let ps = &g.players[0];
    assert!(ps.hand.get(I::MERCHANT_CAMP) + ps.deck_known.iter_top_down().filter(|&c| c == I::MERCHANT_CAMP).count() as u8 >= 1);
}

#[test]
fn town_chooses_between_two_modes() {
    let mut g = game(&[I::TOWN]);
    set_hand(&mut g, 0, &[I::TOWN]);
    play(&mut g, I::TOWN);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 1, .. }), "{d:?}");
    choose(&mut g, Choice::Mode(0));
    adv(&mut g);
    assert_eq!((g.players[0].hand.total(), g.turn.actions), (1, 2));
    let mut g = game(&[I::TOWN]);
    set_hand(&mut g, 0, &[I::TOWN]);
    play(&mut g, I::TOWN);
    choose(&mut g, Choice::Mode(1));
    adv(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins, g.turn.actions), (2, 2, 0));
}

#[test]
fn galleria_gives_a_buy_for_gains_costing_three_or_four() {
    let mut g = game(&[I::GALLERIA, I::TOWN]);
    set_hand(&mut g, 0, &[I::GALLERIA]);
    play(&mut g, I::GALLERIA);
    assert_eq!(g.turn.coins, 3);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 13;
    g.turn.buys = 1;
    buy(&mut g, I::SILVER); // $3: +1 Buy
    assert_eq!(g.turn.buys, 1);
    buy(&mut g, I::TOWN); // $4: +1 Buy
    assert_eq!(g.turn.buys, 1);
    buy(&mut g, I::GOLD); // $6: none, so the last Buy ends the turn
    assert_eq!(g.turn.player, 1);
}

#[test]
fn guildmaster_gives_a_favor_for_every_gain_this_turn() {
    let mut g = game(&[I::GUILDMASTER]);
    set_hand(&mut g, 0, &[I::GUILDMASTER]);
    play(&mut g, I::GUILDMASTER);
    g.turn.phase = Phase::Buy;
    g.turn.buys = 2;
    g.turn.coins = 6;
    buy(&mut g, I::SILVER);
    buy(&mut g, I::SILVER);
    assert_eq!(favors(&g, 0), 2);
}

#[test]
fn emissary_gives_actions_and_favors_only_when_it_shuffled() {
    let mut g = game(&[I::EMISSARY]);
    set_hand(&mut g, 0, &[I::EMISSARY]);
    play(&mut g, I::EMISSARY);
    assert_eq!((g.players[0].hand.total(), g.turn.actions, favors(&g, 0)), (3, 0, 0));
    let mut g = game(&[I::EMISSARY]);
    set_hand(&mut g, 0, &[I::EMISSARY]);
    clear_deck(&mut g, 0);
    set_deck_known(&mut g, 0, &[I::ESTATE]);
    set_discard(&mut g, 0, &[I::DUCHY, I::DUCHY, I::DUCHY]);
    play(&mut g, I::EMISSARY);
    assert_eq!(g.players[0].hand.total(), 3);
    assert_eq!((g.turn.actions, favors(&g, 0)), (1, 2));
}

#[test]
fn skirmisher_makes_the_others_discard_down_to_three_when_an_attack_is_gained() {
    let mut g = game(&[I::SKIRMISHER]);
    set_hand(&mut g, 0, &[I::SKIRMISHER]);
    set_hand(&mut g, 1, &[I::COPPER, I::COPPER, I::SILVER, I::ESTATE, I::ESTATE]);
    play(&mut g, I::SKIRMISHER);
    assert_eq!((g.turn.coins, g.turn.actions), (1, 1));
    g.turn.phase = Phase::Buy;
    g.turn.coins = 8;
    g.turn.buys = 2;
    buy(&mut g, I::SILVER); // not an Attack
    assert_eq!(g.players[1].hand.total(), 5);
    buy(&mut g, I::SKIRMISHER); // an Attack
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 2, max: 2, .. }) && d.player == 1, "{d:?}");
}

// ===========================================================================
// Split piles and rotation
// ===========================================================================

fn open_buys(g: &GameState) -> Vec<CardId> {
    choices(g).into_iter().filter_map(|c| if let Choice::Card(c) = c { Some(c) } else { None }).collect()
}

fn take(g: &mut GameState, c: CardId, n: usize) {
    for _ in 0..n {
        assert!(g.gain_card(0, c, Dest::Discard, &mut NoEvents), "{}", cards::name(c));
    }
}

#[test]
fn rotating_puts_the_top_card_and_its_copies_at_the_bottom() {
    let mut g = game(&[I::TOWN_CRIER]);
    set_hand(&mut g, 0, &[I::TOWN_CRIER]);
    play(&mut g, I::TOWN_CRIER);
    choose(&mut g, Choice::Mode(0)); // +$2
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.source == Some(I::TOWN_CRIER), "{d:?}");
    choose(&mut g, Choice::Yes);
    adv(&mut g);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.al_top(4), Some(I::BLACKSMITH));
    assert!(g.pile_open(I::BLACKSMITH) && !g.pile_open(I::TOWN_CRIER));
    // Four Town Criers are still in the pile, but only the Blacksmith can be bought.
    g.turn.phase = Phase::Buy;
    g.turn.coins = 8;
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    let buys = open_buys(&g);
    assert!(buys.contains(&I::BLACKSMITH) && !buys.contains(&I::TOWN_CRIER) && !buys.contains(&I::MILLER), "{buys:?}");
    check_piles(&g, "rotated");
}

#[test]
fn declining_the_rotation_leaves_the_pile_as_it_was() {
    let mut g = game(&[I::TOWN_CRIER]);
    set_hand(&mut g, 0, &[I::TOWN_CRIER]);
    play(&mut g, I::TOWN_CRIER);
    choose(&mut g, Choice::Mode(0));
    answer(&mut g, I::TOWN_CRIER, Choice::No);
    assert_eq!(g.al_top(4), Some(I::TOWN_CRIER));
}

#[test]
fn a_pile_with_one_kind_left_is_not_offered_for_rotation() {
    let mut g = game(&[I::TOWN_CRIER]);
    take(&mut g, I::TOWN_CRIER, 4);
    take(&mut g, I::BLACKSMITH, 4);
    take(&mut g, I::MILLER, 4);
    // Only the Elders are left (the Town Crier itself is in the discard pile; one more is played from the hand).
    assert_eq!(g.al_top(4), Some(I::ELDER));
    set_hand(&mut g, 0, &[I::TOWN_CRIER]);
    play(&mut g, I::TOWN_CRIER);
    choose(&mut g, Choice::Mode(0));
    // Nothing to rotate to: no question.
    let s = adv(&mut g);
    assert!(!matches!(s, Step::Decision(Decision { source: Some(I::TOWN_CRIER), .. })), "{s:?}");
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.al_top(4), Some(I::ELDER));
}

#[test]
fn taking_the_last_copy_of_the_top_card_opens_the_next_and_the_pile_is_empty_when_all_four_cards_are_gone() {
    let mut g = game(&[I::TOWN_CRIER, I::VILLAGE]);
    let before = g.empty_piles();
    take(&mut g, I::TOWN_CRIER, 4);
    assert_eq!(g.al_top(4), Some(I::BLACKSMITH));
    assert!(g.pile_open(I::BLACKSMITH) && !g.pile_open(I::MILLER));
    take(&mut g, I::BLACKSMITH, 4);
    take(&mut g, I::MILLER, 4);
    assert_eq!(g.empty_piles(), before, "a pile with cards left is not empty");
    take(&mut g, I::ELDER, 4);
    assert_eq!(g.empty_piles(), before + 1, "the pile counts once, when all four cards are gone");
    assert_eq!(g.empty_piles(), g.empty_piles_scan());
    assert_eq!(g.al_top(4), None);
    check_piles(&g, "emptied");
}

#[test]
fn a_gain_costing_up_to_a_price_can_only_take_the_top_card_of_a_split_pile() {
    let mut g = game(&[I::TOWN_CRIER]);
    // The Blacksmith costs 3 as well, but is under the Town Crier: a Workshop-like gain for up to $4 shows the Town Crier only.
    g.stack.push(state::Frame::new(state::FrameKind::Gain, 0, I::TOWN_CRIER));
    let f = g.stack.top().unwrap();
    let mut f = f;
    f.max = 4;
    g.stack.set_top(f);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    let offered = open_buys(&g);
    assert!(offered.contains(&I::TOWN_CRIER) && !offered.contains(&I::BLACKSMITH) && !offered.contains(&I::MILLER) && !offered.contains(&I::ELDER), "{offered:?}");
}

#[test]
fn a_pile_that_is_not_in_the_game_is_not_offered() {
    let g = new_state(&[I::TOWN_CRIER], 2);
    assert!(!g.in_supply(I::STUDENT) && !g.in_supply(I::LICH));
}

#[test]
fn rotation_puts_a_pile_out_of_order_when_a_card_is_returned_on_top() {
    // Swap returns an Action to its pile: a returned Town Crier goes on top of the Blacksmiths after a rotation.
    let mut g = game(&[I::TOWN_CRIER, I::SWAP, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::SWAP, I::TOWN_CRIER]);
    take(&mut g, I::TOWN_CRIER, 4);
    // The Blacksmiths are on top now; return a Town Crier on top of them.
    play(&mut g, I::SWAP);
    // +1 Card +1 Action, then "may return an Action from your hand": the Town Crier.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Return, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::TOWN_CRIER));
    // The gain of a different Action costing up to $5 into the hand: the Town Crier is on top of the pile again but is the same card;
    // the Blacksmith is under it.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { dest: Dest::Hand, .. }), "{d:?}");
    let offered = open_buys(&g);
    assert!(!offered.contains(&I::TOWN_CRIER) && !offered.contains(&I::BLACKSMITH), "{offered:?}");
    assert_eq!(g.supply.get(I::TOWN_CRIER), 1);
    assert_eq!(g.al_top(4), Some(I::TOWN_CRIER));
    assert!(g.pile_open(I::TOWN_CRIER) && !g.pile_open(I::BLACKSMITH));
    check_piles(&g, "returned");
}

// ===========================================================================
// The kingdom cards
// ===========================================================================

/// Pass every decision (taking the first choice where passing is not allowed) until it is player 0's turn again; returns the number of decisions.
fn to_my_next_turn(g: &mut GameState) {
    let start = g.turn.number;
    for _ in 0..400 {
        expect_decision(g);
        if g.turn.player == 0 && g.turn.number > start {
            return;
        }
        let cs = choices(g);
        let c = if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] };
        choose(g, c);
    }
    panic!("no next turn");
}

#[test]
fn bauble_chooses_two_different_things() {
    let mut g = game(&[I::BAUBLE]);
    g.turn.phase = Phase::Buy;
    set_hand(&mut g, 0, &[I::BAUBLE]);
    play_treasure(&mut g, I::BAUBLE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 2, distinct: true }), "{d:?}");
    choose(&mut g, Choice::Mode(0)); // +1 Buy
    choose(&mut g, Choice::Mode(2)); // +1 Favor
    adv(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins, favors(&g, 0)), (2, 0, 1));
}

#[test]
fn bauble_can_put_the_cards_gained_this_turn_onto_the_deck() {
    let mut g = game(&[I::BAUBLE]);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    set_hand(&mut g, 0, &[I::BAUBLE]);
    play_treasure(&mut g, I::BAUBLE);
    choose(&mut g, Choice::Mode(1)); // +$1
    choose(&mut g, Choice::Mode(3)); // gains onto the deck
    adv(&mut g);
    assert_eq!(g.turn.coins, 4);
    buy(&mut g, I::SILVER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Topdeck }) && d.subject == I::SILVER, "{d:?}");
    choose(&mut g, Choice::Yes);
    // The turn ends with the last Buy; the Silver was put on the deck before the next hand was drawn: it is in the new hand or on the deck.
    let ps = &g.players[0];
    assert!(ps.hand.get(I::SILVER) == 1 || ps.deck_known.peek_top() == Some(I::SILVER));
}

#[test]
fn sentinel_trashes_up_to_two_of_the_top_five_and_puts_the_rest_back() {
    let mut g = game(&[I::SENTINEL]);
    set_hand(&mut g, 0, &[I::SENTINEL]);
    set_deck_known(&mut g, 0, &[I::CURSE, I::ESTATE, I::ESTATE, I::DUCHY, I::DUCHY, I::PROVINCE]);
    play(&mut g, I::SENTINEL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, from: Zone::Revealed, min: 0, max: 2, .. }), "{d:?}");
    // (The Curse has the highest id of the cards looked at: no pick can follow it, so the selection ends.)
    choose(&mut g, Choice::Card(I::CURSE));
    while matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Select { act: Act::Topdeck, .. }, .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.trash.get(I::CURSE), 1);
    // 4 of the 5 looked at went back, with the Province under them.
    assert_eq!(g.players[0].deck_known.len as u32 + g.players[0].deck_unknown.total(), 5);
}

#[test]
fn broker_trashes_a_card_and_gives_one_thing_per_dollar() {
    for (mode, check) in [(0u8, "cards"), (1, "actions"), (2, "coins"), (3, "favors")] {
        let mut g = game(&[I::BROKER]);
        set_hand(&mut g, 0, &[I::BROKER, I::SILVER, I::ESTATE]);
        play(&mut g, I::BROKER);
        choose(&mut g, Choice::Card(I::SILVER)); // costs $3
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Mode { picks: 1, .. }), "{d:?}");
        choose(&mut g, Choice::Mode(mode));
        adv(&mut g);
        assert_eq!(g.trash.get(I::SILVER), 1);
        match check {
            "cards" => assert_eq!(g.players[0].hand.total(), 1 + 3),
            "actions" => assert_eq!(g.turn.actions, 3),
            "coins" => assert_eq!(g.turn.coins, 3),
            _ => assert_eq!(favors(&g, 0), 3),
        }
    }
}

#[test]
fn carpenter_gains_a_card_and_gives_an_action_while_no_pile_is_empty_else_trashes_and_upgrades() {
    let mut g = game(&[I::CARPENTER, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::CARPENTER]);
    play(&mut g, I::CARPENTER);
    assert_eq!(g.turn.actions, 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::VILLAGE));
    assert_eq!(g.players[0].discard.get(I::VILLAGE), 1);
    // With an empty pile: trash a card, gain one costing up to $2 more.
    let mut g = game(&[I::CARPENTER, I::VILLAGE]);
    set_supply(&mut g, I::VILLAGE, 0);
    set_hand(&mut g, 0, &[I::CARPENTER, I::SILVER, I::ESTATE]);
    play(&mut g, I::CARPENTER);
    assert_eq!(g.turn.actions, 0);
    choose(&mut g, Choice::Card(I::SILVER));
    assert_eq!(g.trash.get(I::SILVER), 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }), "{d:?}");
}

#[test]
fn courier_discards_the_top_card_and_plays_a_card_from_the_discard_pile() {
    let mut g = game(&[I::COURIER]);
    set_hand(&mut g, 0, &[I::COURIER]);
    set_deck_known(&mut g, 0, &[I::ESTATE, I::DUCHY]);
    set_discard(&mut g, 0, &[I::GOLD, I::PROVINCE]);
    play(&mut g, I::COURIER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, from: Zone::Discard, .. }), "{d:?}");
    // The top card of the deck was discarded first.
    assert_eq!(g.players[0].discard.get(I::ESTATE), 1);
    choose(&mut g, Choice::Card(I::GOLD));
    adv(&mut g);
    assert_eq!(g.turn.coins, 1 + 3);
    assert_eq!(g.players[0].in_play.get(I::GOLD), 1);
}

#[test]
fn innkeeper_draws_and_discards_by_mode() {
    let mut g = game(&[I::INNKEEPER]);
    set_hand(&mut g, 0, &[I::INNKEEPER, I::ESTATE]);
    play(&mut g, I::INNKEEPER);
    choose(&mut g, Choice::Mode(0));
    adv(&mut g);
    assert_eq!((g.players[0].hand.total(), g.turn.actions), (2, 1));
    let mut g = game(&[I::INNKEEPER]);
    set_hand(&mut g, 0, &[I::INNKEEPER, I::ESTATE]);
    play(&mut g, I::INNKEEPER);
    choose(&mut g, Choice::Mode(1)); // +3 Cards then discard 3
    while matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Select { act: Act::Discard, .. }, .. })) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.players[0].hand.total(), 1);
    let mut g = game(&[I::INNKEEPER]);
    set_hand(&mut g, 0, &[I::INNKEEPER, I::ESTATE]);
    play(&mut g, I::INNKEEPER);
    choose(&mut g, Choice::Mode(2)); // +5 Cards then discard 6: all 6 cards in the hand
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 0);
    assert_eq!(g.players[0].discard.total(), 6);
}

#[test]
fn royal_galley_plays_an_action_now_and_again_next_turn_and_keeps_both_in_play() {
    let mut g = game(&[I::ROYAL_GALLEY, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::ROYAL_GALLEY, I::VILLAGE]);
    play(&mut g, I::ROYAL_GALLEY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::VILLAGE));
    assert_eq!(g.players[0].in_play.get(I::VILLAGE), 1);
    assert_eq!(g.turn.actions, 2);
    to_my_next_turn(&mut g);
    // Both are in play still; the Village was played again at the start of the turn (+1 Card +2 Actions).
    assert_eq!(g.players[0].in_play.get(I::ROYAL_GALLEY), 1);
    assert_eq!(g.players[0].in_play.get(I::VILLAGE), 1);
    assert_eq!(g.turn.actions, 3);
    to_my_next_turn(&mut g);
    assert_eq!(g.players[0].in_play.get(I::ROYAL_GALLEY) + g.players[0].in_play.get(I::VILLAGE), 0);
}

#[test]
fn capital_city_discards_two_for_coins_or_pays_two_for_cards() {
    let mut g = game(&[I::CAPITAL_CITY]);
    set_hand(&mut g, 0, &[I::CAPITAL_CITY, I::ESTATE, I::ESTATE]);
    play(&mut g, I::CAPITAL_CITY);
    assert_eq!(g.turn.actions, 2);
    choose(&mut g, Choice::Card(I::ESTATE)); // discard two (the second is forced)
    adv(&mut g);
    assert_eq!(g.turn.coins, 2);
    // Pay $2 for +2 Cards.
    let mut g = game(&[I::CAPITAL_CITY]);
    set_hand(&mut g, 0, &[I::CAPITAL_CITY]);
    g.turn.coins = 2;
    play(&mut g, I::CAPITAL_CITY);
    choose(&mut g, Choice::Pass); // the drawn card is not discarded
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.source == Some(I::CAPITAL_CITY), "{d:?}");
    choose(&mut g, Choice::Yes);
    adv(&mut g);
    assert_eq!(g.turn.coins, 0);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn contract_sets_an_action_aside_and_plays_it_at_the_start_of_next_turn() {
    let mut g = game(&[I::CONTRACT, I::VILLAGE]);
    g.turn.phase = Phase::Buy;
    set_hand(&mut g, 0, &[I::CONTRACT, I::VILLAGE]);
    play_treasure(&mut g, I::CONTRACT);
    assert_eq!((g.turn.coins, favors(&g, 0)), (2, 1));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Choose, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::VILLAGE));
    assert_eq!(g.players[0].men.aside.get(I::VILLAGE), 1);
    to_my_next_turn(&mut g);
    assert_eq!(g.players[0].men.aside.get(I::VILLAGE), 0);
    assert_eq!(g.players[0].in_play.get(I::VILLAGE), 1);
    assert_eq!(g.turn.actions, 3); // +2 Actions from the Village, no Action used
    assert_eq!(g.players[0].in_play.get(I::CONTRACT), 1);
}

#[test]
fn hunter_takes_an_action_a_treasure_and_a_victory_card_of_the_top_three() {
    let mut g = game(&[I::HUNTER, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::HUNTER]);
    set_deck_known(&mut g, 0, &[I::VILLAGE, I::GOLD, I::PROVINCE, I::ESTATE]);
    play(&mut g, I::HUNTER);
    adv(&mut g);
    for c in [I::VILLAGE, I::GOLD, I::PROVINCE] {
        assert_eq!(hand(&g, c), 1, "{}", cards::name(c));
    }
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn modify_trashes_a_card_for_a_card_and_an_action_or_a_better_card() {
    let mut g = game(&[I::MODIFY]);
    set_hand(&mut g, 0, &[I::MODIFY, I::SILVER, I::ESTATE]);
    play(&mut g, I::MODIFY);
    choose(&mut g, Choice::Card(I::SILVER));
    choose(&mut g, Choice::Mode(0));
    adv(&mut g);
    assert_eq!((g.players[0].hand.total(), g.turn.actions, g.trash.get(I::SILVER)), (2, 1, 1));
    let mut g = game(&[I::MODIFY]);
    set_hand(&mut g, 0, &[I::MODIFY, I::SILVER, I::ESTATE]);
    play(&mut g, I::MODIFY);
    choose(&mut g, Choice::Card(I::SILVER));
    choose(&mut g, Choice::Mode(1));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }), "{d:?}");
}

#[test]
fn marquis_draws_a_card_per_card_in_hand_and_discards_down_to_ten() {
    let mut g = game(&[I::MARQUIS]);
    set_hand(&mut g, 0, &[I::MARQUIS, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE]);
    play(&mut g, I::MARQUIS);
    // 6 cards in hand: +6 Cards makes 12, discard 2.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 2, max: 2, .. }), "{d:?}");
    assert_eq!(g.turn.buys, 2);
}

#[test]
fn sibyl_draws_four_and_puts_one_card_on_the_deck_and_one_on_the_bottom() {
    let mut g = game(&[I::SIBYL]);
    set_hand(&mut g, 0, &[I::SIBYL]);
    set_deck_known(&mut g, 0, &[I::ESTATE, I::DUCHY, I::PROVINCE, I::CURSE, I::ESTATE]);
    play(&mut g, I::SIBYL);
    // Hand: Estate, Duchy, Province, Curse. First the top, then the bottom.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::PROVINCE));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Bottom, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::CURSE));
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 2);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::PROVINCE));
}

#[test]
fn herb_gatherer_puts_the_deck_into_the_discard_pile_and_may_play_a_treasure_from_it() {
    let mut g = game(&[I::HERB_GATHERER]);
    set_hand(&mut g, 0, &[I::HERB_GATHERER]);
    set_deck_known(&mut g, 0, &[I::GOLD, I::ESTATE, I::DUCHY]);
    play(&mut g, I::HERB_GATHERER);
    assert_eq!(g.turn.buys, 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Play, from: Zone::Discard, .. }), "{d:?}");
    assert_eq!(g.players[0].discard.total(), 3);
    choose(&mut g, Choice::Card(I::GOLD));
    // The rotation of the Augurs: Herb Gatherer is on top; Acolyte under it: worth asking.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.source == Some(I::HERB_GATHERER), "{d:?}");
    choose(&mut g, Choice::Yes);
    adv(&mut g);
    assert_eq!(g.turn.coins, 3);
    assert_eq!(g.al_top(0), Some(I::ACOLYTE));
}

#[test]
fn acolyte_trashes_an_action_or_victory_for_a_gold_and_itself_for_an_augur() {
    let mut g = game(&[I::HERB_GATHERER]);
    set_hand(&mut g, 0, &[I::ACOLYTE, I::ESTATE, I::COPPER]);
    // An Acolyte is not on top of the Augurs: it can still be played from the hand.
    play(&mut g, I::ACOLYTE);
    choose(&mut g, Choice::Card(I::ESTATE));
    assert_eq!(g.players[0].discard.get(I::GOLD), 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Trash }) && d.source == Some(I::ACOLYTE), "{d:?}");
    choose(&mut g, Choice::Yes);
    // It is gone from play; an Augur is gained: the Herb Gatherer, the only one open (so there is nothing to choose).
    assert_eq!(g.trash.get(I::ACOLYTE), 1);
    assert_eq!(g.players[0].discard.get(I::HERB_GATHERER), 1);
}

#[test]
fn sorceress_names_a_card_and_the_others_get_curses_when_it_is_the_one_drawn() {
    let mut g = game(&[I::SORCERESS]);
    set_hand(&mut g, 0, &[I::SORCERESS]);
    set_deck_known(&mut g, 0, &[I::PROVINCE, I::ESTATE]);
    play(&mut g, I::SORCERESS);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Name, "{d:?}");
    choose(&mut g, Choice::Card(I::PROVINCE));
    adv(&mut g);
    assert_eq!(hand(&g, I::PROVINCE), 1);
    assert_eq!(g.players[1].discard.get(I::CURSE), 1);
    // A wrong guess: no Curse.
    let mut g = game(&[I::SORCERESS]);
    set_hand(&mut g, 0, &[I::SORCERESS]);
    set_deck_known(&mut g, 0, &[I::PROVINCE, I::ESTATE]);
    play(&mut g, I::SORCERESS);
    choose(&mut g, Choice::Card(I::ESTATE));
    adv(&mut g);
    assert_eq!(hand(&g, I::PROVINCE), 1);
    assert_eq!(g.players[1].discard.get(I::CURSE), 0);
}

#[test]
fn battle_plan_reveals_an_attack_for_a_card_and_rotates_any_pile() {
    let mut g = game(&[I::BATTLE_PLAN, I::TOWN_CRIER, I::SKIRMISHER]);
    set_hand(&mut g, 0, &[I::BATTLE_PLAN, I::SKIRMISHER]);
    play(&mut g, I::BATTLE_PLAN);
    assert_eq!(hand(&g, I::SKIRMISHER), 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Reveal, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::SKIRMISHER));
    // 1 Card from the Battle Plan, 1 for the Attack.
    assert_eq!(g.players[0].hand.total(), 1 + 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Choose, from: Zone::Supply, .. }), "{d:?}");
    // Only piles that change by rotating are offered: the Clashes and the Townsfolk, not the Estates.
    let offered = open_buys(&g);
    assert!(offered.contains(&I::BATTLE_PLAN) && offered.contains(&I::TOWN_CRIER) && !offered.contains(&I::ESTATE) && !offered.contains(&I::SKIRMISHER), "{offered:?}");
    choose(&mut g, Choice::Card(I::TOWN_CRIER));
    assert_eq!(g.al_top(4), Some(I::BLACKSMITH));
}

#[test]
fn archer_makes_a_player_with_five_cards_reveal_all_but_one_and_discards_the_one_the_attacker_picks() {
    let mut g = game(&[I::ARCHER]);
    set_hand(&mut g, 0, &[I::ARCHER]);
    set_hand(&mut g, 1, &[I::COPPER, I::SILVER, I::GOLD, I::ESTATE, I::DUCHY]);
    play(&mut g, I::ARCHER);
    assert_eq!(g.turn.coins, 2);
    // The victim keeps one card (here the Duchy); the attacker chooses among the other four.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1, "{d:?}");
    choose(&mut g, Choice::Card(I::DUCHY));
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.for_player), (0, 1), "{d:?}");
    choose(&mut g, Choice::Card(I::GOLD));
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(I::GOLD), 1);
    assert_eq!(g.players[1].hand.total(), 4);
    assert_eq!(g.players[1].hand.get(I::DUCHY), 1);
}

#[test]
fn archer_does_not_attack_a_player_with_fewer_than_five_cards() {
    let mut g = game(&[I::ARCHER]);
    set_hand(&mut g, 0, &[I::ARCHER]);
    set_hand(&mut g, 1, &[I::COPPER, I::SILVER, I::GOLD, I::ESTATE]);
    play(&mut g, I::ARCHER);
    adv(&mut g);
    assert_eq!(g.players[1].hand.total(), 4);
}

#[test]
fn tent_gives_two_coins_and_rotates_the_forts() {
    let mut g = game(&[I::TENT]);
    set_hand(&mut g, 0, &[I::TENT]);
    play(&mut g, I::TENT);
    assert_eq!(g.turn.coins, 2);
    answer(&mut g, I::TENT, Choice::Yes);
    assert_eq!(g.al_top(2), Some(I::GARRISON));
}

#[test]
fn old_map_draws_discards_and_draws() {
    let mut g = game(&[I::OLD_MAP]);
    set_hand(&mut g, 0, &[I::OLD_MAP, I::ESTATE, I::DUCHY]);
    set_deck_known(&mut g, 0, &[I::PROVINCE, I::CURSE, I::ESTATE]);
    play(&mut g, I::OLD_MAP);
    assert_eq!(g.players[0].hand.total(), 3);
    choose(&mut g, Choice::Card(I::PROVINCE));
    assert_eq!(g.players[0].hand.total(), 3);
    assert_eq!(g.players[0].discard.get(I::PROVINCE), 1);
    assert_eq!(g.turn.actions, 1);
    // The Odysseys can be rotated (Old Map is on top, Voyage under it).
    answer(&mut g, I::OLD_MAP, Choice::Yes);
    assert_eq!(g.al_top(3), Some(I::VOYAGE));
}

#[test]
fn importer_gains_a_card_up_to_five_next_turn_and_gives_each_player_five_favors_at_set_up() {
    let g = GameState::new(&GameConfig { kingdom: vec![I::IMPORTER, I::VILLAGE, I::TOWN], ..Default::default() });
    assert_eq!((favors(&g, 0), favors(&g, 1)), (5, 5));
    let mut g = game(&[I::IMPORTER, I::TOWN]);
    set_hand(&mut g, 0, &[I::IMPORTER]);
    play(&mut g, I::IMPORTER);
    to_my_next_turn(&mut g);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::TOWN));
    assert_eq!(g.players[0].discard.get(I::TOWN), 1);
}

#[test]
fn highwayman_stops_the_first_treasure_each_other_player_plays_until_its_next_turn() {
    let coins_of_player_1 = |highwayman: bool| {
        let mut g = game(&[I::HIGHWAYMAN]);
        if highwayman {
            set_hand(&mut g, 0, &[I::HIGHWAYMAN]);
            play(&mut g, I::HIGHWAYMAN);
        }
        // Player 1's turn: the Treasures are played at the start of the Buy phase.
        reset_turn(&mut g, 1);
        set_hand(&mut g, 1, &[I::SILVER, I::SILVER, I::GOLD]);
        // Under the Attack the first Treasure played does nothing, so the Treasures are played by choice: the Silver first.
        let mut d = expect_decision(&mut g);
        if highwayman {
            assert_eq!(d.kind, DecisionKind::PlayTreasure, "{d:?}");
            choose(&mut g, Choice::Card(I::SILVER));
            d = expect_decision(&mut g);
        }
        assert_eq!(d.kind, DecisionKind::Buy);
        g.turn.coins
    };
    assert_eq!(coins_of_player_1(false), 7);
    let c = coins_of_player_1(true);
    assert_eq!(c, 5, "the first Treasure played (a Silver) does nothing: the other Silver and the Gold give $2 and $3");
}

#[test]
fn warlord_stops_others_from_playing_an_action_they_have_two_copies_of_in_play() {
    for (copies, can_play) in [(2, false), (1, true)] {
        let mut g = game(&[I::WARLORD, I::VILLAGE]);
        set_hand(&mut g, 0, &[I::WARLORD]);
        play(&mut g, I::WARLORD);
        // Player 1's turn.
        reset_turn(&mut g, 1);
        set_in_play(&mut g, 1, &vec![I::VILLAGE; copies]);
        set_hand(&mut g, 1, &[I::VILLAGE, I::ESTATE]);
        let d = expect_decision(&mut g);
        // (Nothing to play: the turn goes to the Buy phase.)
        assert_eq!(d.kind == DecisionKind::PlayAction, can_play, "{copies} copies: {d:?}");
    }
}

#[test]
fn garrison_counts_the_gains_of_the_turn_and_draws_that_many_cards_next_turn() {
    let mut g = game(&[I::GARRISON]);
    set_hand(&mut g, 0, &[I::GARRISON]);
    play(&mut g, I::GARRISON);
    assert_eq!(g.turn.coins, 2);
    g.turn.phase = Phase::Buy;
    g.turn.buys = 2;
    g.turn.coins = 6;
    buy(&mut g, I::SILVER);
    buy(&mut g, I::SILVER);
    assert_eq!(g.players[0].al.tokens, 2);
    to_my_next_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 5 + 2);
    assert_eq!(g.players[0].al.tokens, 0);
}

#[test]
fn hill_fort_gains_a_card_to_the_hand_or_gives_a_card_and_an_action() {
    let mut g = game(&[I::HILL_FORT, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::HILL_FORT]);
    play(&mut g, I::HILL_FORT);
    choose(&mut g, Choice::Card(I::VILLAGE));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    choose(&mut g, Choice::Mode(0)); // the gained card into the hand
    adv(&mut g);
    assert_eq!(hand(&g, I::VILLAGE), 1);
    let mut g = game(&[I::HILL_FORT, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::HILL_FORT]);
    play(&mut g, I::HILL_FORT);
    choose(&mut g, Choice::Card(I::VILLAGE));
    choose(&mut g, Choice::Mode(1));
    adv(&mut g);
    assert_eq!((g.players[0].discard.get(I::VILLAGE), g.players[0].hand.total(), g.turn.actions), (1, 1, 1));
}

#[test]
fn stronghold_gives_three_coins_or_three_cards_next_turn_and_two_victory_points() {
    let mut g = game(&[I::STRONGHOLD]);
    set_hand(&mut g, 0, &[I::STRONGHOLD]);
    play(&mut g, I::STRONGHOLD);
    choose(&mut g, Choice::Mode(0));
    adv(&mut g);
    assert_eq!(g.turn.coins, 3);
    let mut g = game(&[I::STRONGHOLD]);
    set_hand(&mut g, 0, &[I::STRONGHOLD]);
    play(&mut g, I::STRONGHOLD);
    choose(&mut g, Choice::Mode(1));
    adv(&mut g);
    assert_eq!(g.turn.coins, 0);
    to_my_next_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 5 + 3);
    let g = game(&[I::STRONGHOLD]);
    let mut g2 = g;
    set_hand(&mut g2, 0, &[I::STRONGHOLD]);
    assert_eq!(g2.scores()[0] - g.scores()[0], 2);
}

#[test]
fn voyage_takes_an_extra_turn_in_which_three_cards_can_be_played_from_the_hand() {
    let mut g = game(&[I::VOYAGE, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::VOYAGE]);
    // The next hand is five Villages.
    set_deck_known(&mut g, 0, &[I::VILLAGE; 5]);
    play(&mut g, I::VOYAGE);
    // The turn ends; player 0 takes the next turn too.
    let n = g.turn.number;
    for _ in 0..40 {
        expect_decision(&mut g);
        if g.turn.number > n {
            break;
        }
        let cs = choices(&g);
        choose(&mut g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
    }
    assert_eq!(g.turn.player, 0, "an extra turn");
    assert!(g.turn.is_extra_turn);
    // Three Villages can be played from the hand, the others cannot.
    for _ in 0..3 {
        let cs = choices(&g);
        assert!(cs.contains(&Choice::Card(I::VILLAGE)), "{cs:?}");
        choose(&mut g, Choice::Card(I::VILLAGE));
    }
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(I::VILLAGE)), "{cs:?}");
}

#[test]
fn conjurer_gains_a_card_and_returns_to_the_hand_next_turn() {
    let mut g = game(&[I::STUDENT, I::VILLAGE]);
    // Conjurer is under the Student: put it in the hand by hand.
    set_hand(&mut g, 0, &[I::CONJURER]);
    play(&mut g, I::CONJURER);
    choose(&mut g, Choice::Card(I::VILLAGE));
    assert_eq!(g.players[0].discard.get(I::VILLAGE), 1);
    to_my_next_turn(&mut g);
    assert_eq!(hand(&g, I::CONJURER), 1);
    assert_eq!(g.players[0].in_play.get(I::CONJURER), 0);
}

#[test]
fn student_rotates_the_wizards_and_trashes_a_card_and_a_treasure_gives_a_favor_and_returns_it_to_the_deck() {
    let mut g = game(&[I::STUDENT]);
    set_hand(&mut g, 0, &[I::STUDENT, I::COPPER]);
    play(&mut g, I::STUDENT);
    answer(&mut g, I::STUDENT, Choice::No);
    // (The only card in the hand is trashed without a question.)
    assert_eq!(g.trash.get(I::COPPER), 1);
    assert_eq!(favors(&g, 0), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::STUDENT));
    assert_eq!(g.players[0].in_play.get(I::STUDENT), 0);
    // Not a Treasure: no Favor, and the Student is discarded as usual.
    let mut g = game(&[I::STUDENT]);
    set_hand(&mut g, 0, &[I::STUDENT, I::ESTATE]);
    play(&mut g, I::STUDENT);
    answer(&mut g, I::STUDENT, Choice::No);
    assert_eq!((favors(&g, 0), g.players[0].in_play.get(I::STUDENT)), (0, 1));
    // The rotation: Student is on top of the Wizards, Conjurer under it.
    let mut g = game(&[I::STUDENT]);
    set_hand(&mut g, 0, &[I::STUDENT, I::ESTATE]);
    play(&mut g, I::STUDENT);
    answer(&mut g, I::STUDENT, Choice::Yes);
    assert_eq!(g.al_top(5), Some(I::CONJURER));
}

#[test]
fn a_student_played_twice_returns_only_once() {
    let mut g = game(&[I::STUDENT, I::THRONE_ROOM]);
    set_hand(&mut g, 0, &[I::THRONE_ROOM, I::STUDENT, I::COPPER, I::COPPER]);
    play(&mut g, I::THRONE_ROOM);
    choose(&mut g, Choice::Card(I::STUDENT));
    // First play: no rotation, trash a Copper (Treasure): the Student goes onto the deck; the second play finds no Student in play.
    for _ in 0..8 {
        match g.pending_decision() {
            Some(Decision { kind: DecisionKind::YesNo { .. }, .. }) => {
                choose(&mut g, Choice::No);
            }
            Some(Decision { kind: DecisionKind::Select { act: Act::Trash, .. }, .. }) => {
                choose(&mut g, Choice::Card(I::COPPER));
            }
            _ => {
                if matches!(adv(&mut g), Step::Decision(_)) && g.pending_decision().is_none() {
                    break;
                }
                if g.pending_decision().is_none() {
                    break;
                }
            }
        }
    }
    let total = g.players[0].deck_known.iter_top_down().filter(|&c| c == I::STUDENT).count() as u8
        + g.players[0].hand.get(I::STUDENT)
        + g.players[0].in_play.get(I::STUDENT)
        + g.players[0].discard.get(I::STUDENT);
    assert_eq!(total, 1);
}

#[test]
fn miller_looks_at_four_and_takes_one() {
    let mut g = game(&[I::TOWN_CRIER]);
    set_hand(&mut g, 0, &[I::MILLER]);
    set_deck_known(&mut g, 0, &[I::ESTATE, I::DUCHY, I::CURSE, I::PROVINCE, I::CURSE]);
    play(&mut g, I::MILLER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Revealed, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::PROVINCE));
    adv(&mut g);
    assert_eq!(hand(&g, I::PROVINCE), 1);
    assert_eq!(g.players[0].discard.total(), 3);
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn blacksmith_draws_up_to_six_or_two_cards_or_a_card_and_an_action() {
    let mut g = game(&[I::TOWN_CRIER]);
    set_hand(&mut g, 0, &[I::BLACKSMITH, I::ESTATE]);
    play(&mut g, I::BLACKSMITH);
    choose(&mut g, Choice::Mode(0));
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 6);
    let mut g = game(&[I::TOWN_CRIER]);
    set_hand(&mut g, 0, &[I::BLACKSMITH]);
    play(&mut g, I::BLACKSMITH);
    choose(&mut g, Choice::Mode(1));
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn elder_gives_two_coins_and_plays_an_action_with_an_extra_choice() {
    let mut g = game(&[I::ELDER, I::TOWN]);
    set_hand(&mut g, 0, &[I::ELDER, I::TOWN]);
    play(&mut g, I::ELDER);
    assert_eq!(g.turn.coins, 2);
    choose(&mut g, Choice::Card(I::TOWN));
    // Town's choose-one gives two picks now; the first is not optional, the extra one is.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 2, distinct: true }), "{d:?}");
    assert!(!choices(&g).contains(&Choice::Pass));
    choose(&mut g, Choice::Mode(0));
    expect_decision(&mut g);
    assert_eq!(choices(&g), vec![Choice::Mode(1), Choice::Pass]);
    choose(&mut g, Choice::Mode(1));
    adv(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins, g.turn.actions, g.players[0].hand.total()), (2, 4, 2, 1));
    // The extra option may be left out.
    let mut g = game(&[I::ELDER, I::TOWN]);
    set_hand(&mut g, 0, &[I::ELDER, I::TOWN]);
    play(&mut g, I::ELDER);
    choose(&mut g, Choice::Card(I::TOWN));
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(1));
    expect_decision(&mut g);
    choose(&mut g, Choice::Pass);
    adv(&mut g);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 4));
    assert_eq!(g.turn.actions, 0);
    // A card without a choice is simply played.
    let mut g = game(&[I::ELDER, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::ELDER, I::VILLAGE]);
    play(&mut g, I::ELDER);
    choose(&mut g, Choice::Card(I::VILLAGE));
    adv(&mut g);
    assert_eq!(g.turn.actions, 2);
}

#[test]
fn elder_on_count_gives_each_choose_one_an_optional_extra_pick() {
    let mut g = game(&[I::ELDER, I::COUNT]);
    set_hand(&mut g, 0, &[I::ELDER, I::COUNT, I::ESTATE, I::ESTATE, I::ESTATE]);
    play(&mut g, I::ELDER);
    choose(&mut g, Choice::Card(I::COUNT));
    // First choose-one (discard 2 / topdeck 1 / gain a Copper): only one option, no extra.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 2, .. }), "{d:?}");
    choose(&mut g, Choice::Mode(2));
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Pass));
    choose(&mut g, Choice::Pass);
    // Second choose-one (+$3 / trash the hand / gain a Duchy): both +$3 and a Duchy.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 2, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Mode(3), Choice::Mode(4), Choice::Mode(5)]);
    choose(&mut g, Choice::Mode(3));
    expect_decision(&mut g);
    choose(&mut g, Choice::Mode(5));
    adv(&mut g);
    assert_eq!(g.turn.coins, 5);
    assert_eq!(g.players[0].discard.get(I::COPPER), 1);
    assert_eq!(g.players[0].discard.get(I::DUCHY), 1);
    assert_eq!(g.players[0].hand.get(I::ESTATE), 3);
}

#[test]
fn barbarian_trashes_the_top_card_and_gives_a_cheaper_card_sharing_a_type_or_a_curse() {
    let mut g = game(&[I::BARBARIAN, I::VILLAGE, I::SMITHY]);
    set_hand(&mut g, 0, &[I::BARBARIAN]);
    set_deck_known(&mut g, 1, &[I::GOLD, I::ESTATE]);
    play(&mut g, I::BARBARIAN);
    assert_eq!(g.turn.coins, 2);
    // A Gold ($6) is trashed; the victim gains a cheaper Treasure: Silver (or Copper).
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1, "{d:?}");
    assert_eq!(g.trash.get(I::GOLD), 1);
    choose(&mut g, Choice::Card(I::SILVER));
    assert_eq!(g.players[1].discard.get(I::SILVER), 1);
    // A cheap card ($2 or less): a Curse.
    let mut g = game(&[I::BARBARIAN]);
    set_hand(&mut g, 0, &[I::BARBARIAN]);
    set_deck_known(&mut g, 1, &[I::ESTATE]);
    play(&mut g, I::BARBARIAN);
    adv(&mut g);
    assert_eq!((g.trash.get(I::ESTATE), g.players[1].discard.get(I::CURSE)), (1, 1));
}

#[test]
fn sorcerer_makes_the_others_name_a_card_and_curses_a_wrong_guess() {
    let mut g = game(&[I::STUDENT]);
    set_hand(&mut g, 0, &[I::SORCERER]);
    set_deck_known(&mut g, 1, &[I::PROVINCE, I::DUCHY]);
    play(&mut g, I::SORCERER);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.player), (DecisionKind::Name, 1), "{d:?}");
    choose(&mut g, Choice::Card(I::DUCHY)); // wrong
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(I::CURSE), 1);
    // The revealed card goes back on the deck.
    assert_eq!(g.players[1].deck_known.peek_top(), Some(I::PROVINCE));
    let mut g = game(&[I::STUDENT]);
    set_hand(&mut g, 0, &[I::SORCERER]);
    set_deck_known(&mut g, 1, &[I::PROVINCE, I::DUCHY]);
    play(&mut g, I::SORCERER);
    choose(&mut g, Choice::Card(I::PROVINCE)); // right
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(I::CURSE), 0);
}

#[test]
fn specialist_plays_a_card_and_then_plays_it_again_or_gains_a_copy() {
    let mut g = game(&[I::SPECIALIST, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::SPECIALIST, I::VILLAGE]);
    play(&mut g, I::SPECIALIST);
    choose(&mut g, Choice::Card(I::VILLAGE));
    choose(&mut g, Choice::Mode(0)); // again
    adv(&mut g);
    assert_eq!(g.turn.actions, 1 - 1 + 2 + 2);
    let mut g = game(&[I::SPECIALIST, I::VILLAGE]);
    set_hand(&mut g, 0, &[I::SPECIALIST, I::VILLAGE]);
    play(&mut g, I::SPECIALIST);
    choose(&mut g, Choice::Card(I::VILLAGE));
    choose(&mut g, Choice::Mode(1)); // a copy
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::VILLAGE), 1);
    assert_eq!(g.turn.actions, 2);
}

#[test]
fn distant_shore_draws_two_gains_an_estate_and_is_worth_two_points() {
    let mut g = game(&[I::OLD_MAP]);
    set_hand(&mut g, 0, &[I::DISTANT_SHORE]);
    let before = g.scores()[0];
    play(&mut g, I::DISTANT_SHORE);
    assert_eq!((g.players[0].hand.total(), g.turn.actions, g.players[0].discard.get(I::ESTATE)), (2, 1, 1));
    // The Estate is worth 1 (the Distant Shore's own 2 VP were counted already).
    assert_eq!(g.scores()[0] - before, 1);
    let g2 = new_state(&[I::OLD_MAP], 2);
    let mut g2 = g2;
    set_hand(&mut g2, 0, &[I::DISTANT_SHORE]);
    assert_eq!(g2.scores()[0], 2);
}

#[test]
fn sunken_treasure_gains_an_action_with_no_copy_in_play() {
    let mut g = game(&[I::OLD_MAP, I::VILLAGE, I::SMITHY]);
    g.turn.phase = Phase::Buy;
    set_hand(&mut g, 0, &[I::SUNKEN_TREASURE]);
    set_in_play(&mut g, 0, &[I::VILLAGE]);
    play_treasure(&mut g, I::SUNKEN_TREASURE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    let offered = open_buys(&g);
    assert!(offered.contains(&I::SMITHY) && !offered.contains(&I::VILLAGE), "{offered:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(g.players[0].discard.get(I::SMITHY), 1);
}

#[test]
fn territory_is_worth_a_point_per_differently_named_victory_card_and_gains_golds_for_empty_piles() {
    let mut g = new_state(&[I::OLD_MAP, I::VILLAGE], 2);
    set_hand(&mut g, 0, &[I::TERRITORY, I::ESTATE, I::DUCHY, I::ESTATE]);
    // Estate x2 = 2, Duchy = 3, Territory = Estate, Duchy, Territory = 3.
    assert_eq!(g.scores()[0], 2 + 3 + 3);
    // Gaining it with two piles empty: two Golds.
    let mut g = game(&[I::BATTLE_PLAN, I::VILLAGE]);
    // (The Battle Plan pile's first 12 cards go: Territory is on top then.)
    take(&mut g, I::BATTLE_PLAN, 4);
    take(&mut g, I::ARCHER, 4);
    take(&mut g, I::WARLORD, 4);
    set_supply(&mut g, I::VILLAGE, 0);
    set_supply(&mut g, I::CURSE, 0);
    assert_eq!(g.empty_piles(), 2);
    assert_eq!(g.al_top(1), Some(I::TERRITORY));
    take(&mut g, I::TERRITORY, 1);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::GOLD), 2);
}

#[test]
fn lich_draws_six_skips_the_next_turn_and_gains_a_cheaper_card_from_the_trash_when_trashed() {
    let mut g = game(&[I::STUDENT]);
    set_hand(&mut g, 0, &[I::LICH]);
    play(&mut g, I::LICH);
    assert_eq!((g.players[0].hand.total(), g.turn.actions), (6, 2));
    assert_eq!(g.players[0].al.skips, 1);
    // The next turn of player 0 is skipped: after player 1's turn it is player 1 again.
    let n = g.turn.number;
    let mut turns = Vec::new();
    for _ in 0..300 {
        let d = expect_decision(&mut g);
        if turns.last() != Some(&(g.turn.number, g.turn.player)) {
            turns.push((g.turn.number, g.turn.player));
        }
        if g.turn.number >= n + 3 {
            let _ = d;
            break;
        }
        let cs = choices(&g);
        choose(&mut g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
    }
    let players: Vec<u8> = turns.iter().map(|t| t.1).collect();
    assert_eq!(&players[..3], &[0, 1, 1], "{turns:?}");
}

// ===========================================================================
// The Allies
// ===========================================================================

/// A game with this Ally and these kingdom cards (junk decks), player 0 with `fav` Favors.
fn agame(ally: CardId, kingdom: &[CardId], fav: u8) -> GameState {
    let mut k = kingdom.to_vec();
    k.push(ally);
    let mut g = game(&k);
    assert_eq!(g.al.ally, ally);
    g.players[0].al.favors = fav;
    g
}

#[test]
fn architects_guild_spends_two_favors_to_gain_a_cheaper_non_victory_card() {
    let mut g = agame(I::ARCHITECTS_GUILD, &[I::VILLAGE], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 6;
    g.turn.buys = 2;
    buy(&mut g, I::GOLD);
    answer(&mut g, I::ARCHITECTS_GUILD, Choice::Yes);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    let offered = open_buys(&g);
    assert!(offered.contains(&I::SILVER) && !offered.contains(&I::GOLD) && !offered.contains(&I::DUCHY) && !offered.contains(&I::ESTATE), "{offered:?}");
    choose(&mut g, Choice::Card(I::SILVER));
    assert_eq!(favors(&g, 0), 0);
    assert_eq!(g.players[0].discard.get(I::SILVER), 1);
}

#[test]
fn architects_guild_needs_two_favors() {
    let mut g = agame(I::ARCHITECTS_GUILD, &[I::VILLAGE], 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 6;
    g.turn.buys = 2;
    buy(&mut g, I::GOLD);
    let d = expect_decision(&mut g);
    assert!(d.source != Some(I::ARCHITECTS_GUILD), "{d:?}");
}

#[test]
fn band_of_nomads_gives_a_card_an_action_or_a_buy_for_a_favor_on_a_gain_of_three_or_more() {
    let mut g = agame(I::BAND_OF_NOMADS, &[I::VILLAGE], 3);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    g.turn.buys = 1;
    buy(&mut g, I::SILVER);
    answer(&mut g, I::BAND_OF_NOMADS, Choice::Yes);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }) && d.source == Some(I::BAND_OF_NOMADS), "{d:?}");
    choose(&mut g, Choice::Mode(2)); // +1 Buy
    assert_eq!(g.turn.buys, 1);
    assert_eq!(favors(&g, 0), 2);
    // A card costing less than $3: no offer.
    let mut g = agame(I::BAND_OF_NOMADS, &[I::VILLAGE], 3);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 5;
    g.turn.buys = 2;
    buy(&mut g, I::ESTATE);
    let d = expect_decision(&mut g);
    assert!(d.source != Some(I::BAND_OF_NOMADS), "{d:?}");
}

#[test]
fn city_state_plays_a_gained_action_for_two_favors() {
    let mut g = agame(I::CITY_STATE, &[I::VILLAGE], 2);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    g.turn.buys = 2;
    // (Played in the Buy phase: the Village still gives its +1 Card and +2 Actions.)
    buy(&mut g, I::VILLAGE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.source == Some(I::CITY_STATE), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(favors(&g, 0), 0);
    assert_eq!(g.players[0].in_play.get(I::VILLAGE), 1);
}

#[test]
fn trappers_lodge_puts_a_gained_card_onto_the_deck_for_a_favor() {
    let mut g = agame(I::TRAPPERS_LODGE, &[I::VILLAGE], 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    g.turn.buys = 2;
    buy(&mut g, I::SILVER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Topdeck }) && d.source == Some(I::TRAPPERS_LODGE), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::SILVER));
    assert_eq!(favors(&g, 0), 0);
}

#[test]
fn cave_dwellers_discards_a_card_and_draws_one_as_often_as_desired() {
    // (A start-of-turn ability is offered at once in a state made by `new_state`: its first turn starts with the first decision.)
    let mut g = agame(I::CAVE_DWELLERS, &[I::VILLAGE], 2);
    set_deck_known(&mut g, 0, &[I::GOLD; 12]);
    set_hand(&mut g, 0, &[I::ESTATE, I::ESTATE, I::DUCHY, I::DUCHY, I::PROVINCE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::CAVE_DWELLERS));
    choose(&mut g, Choice::Yes);
    // Discard a card, draw a card; asked again with the Favor left.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }), "{d:?}");
    let c = choices(&g)[0];
    choose(&mut g, c);
    assert_eq!(favors(&g, 0), 1);
    assert_eq!(g.players[0].hand.get(I::GOLD), 1);
    answer(&mut g, I::CAVE_DWELLERS, Choice::No);
}

#[test]
fn crafters_guild_gains_a_card_onto_the_deck_for_two_favors() {
    let mut g = agame(I::CRAFTERS_GUILD, &[I::VILLAGE], 2);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::CRAFTERS_GUILD));
    choose(&mut g, Choice::Yes);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::SILVER));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::SILVER));
    assert_eq!(favors(&g, 0), 0);
}

#[test]
fn desert_guides_discards_the_hand_and_draws_five() {
    let mut g = agame(I::DESERT_GUIDES, &[I::VILLAGE], 1);
    set_deck_known(&mut g, 0, &[I::DUCHY; 20]);
    set_hand(&mut g, 0, &[I::ESTATE; 3]);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::DESERT_GUIDES));
    choose(&mut g, Choice::Yes);
    assert_eq!((g.players[0].hand.get(I::DUCHY), g.players[0].hand.total(), favors(&g, 0)), (5, 5, 0));
    assert_eq!(g.players[0].discard.get(I::ESTATE), 3);
}

#[test]
fn forest_dwellers_looks_at_three_cards_and_discards_any_number() {
    let mut g = agame(I::FOREST_DWELLERS, &[I::VILLAGE], 1);
    set_deck_known(&mut g, 0, &[I::GOLD, I::SILVER, I::COPPER, I::DUCHY]);
    set_hand(&mut g, 0, &[I::ESTATE; 5]);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::FOREST_DWELLERS));
    choose(&mut g, Choice::Yes);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, from: Zone::Revealed, min: 0, .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::COPPER));
    while matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Select { from: Zone::Revealed, .. }, .. })) {
        let cs = choices(&g);
        choose(&mut g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
    }
    assert_eq!(g.players[0].discard.get(I::COPPER), 1);
    assert_eq!(g.players[0].deck_known.len, 3);
}

#[test]
fn gang_of_pickpockets_discards_down_to_four_unless_a_favor_is_spent() {
    let mut g = agame(I::GANG_OF_PICKPOCKETS, &[I::VILLAGE], 1);
    set_hand(&mut g, 0, &[I::ESTATE; 5]);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::GANG_OF_PICKPOCKETS));
    choose(&mut g, Choice::Yes);
    assert_eq!((g.players[0].hand.total(), favors(&g, 0)), (5, 0));
    // Without a Favor: discard down to 4.
    let mut g = agame(I::GANG_OF_PICKPOCKETS, &[I::VILLAGE], 0);
    set_hand(&mut g, 0, &[I::ESTATE, I::DUCHY, I::PROVINCE, I::CURSE, I::GOLD]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) && d.source == Some(I::GANG_OF_PICKPOCKETS), "{d:?}");
    let c = choices(&g)[0];
    choose(&mut g, c);
    assert_eq!(g.players[0].hand.total(), 4);
}

#[test]
fn mountain_folk_draws_three_cards_for_five_favors() {
    let mut g = agame(I::MOUNTAIN_FOLK, &[I::VILLAGE], 5);
    set_hand(&mut g, 0, &[I::ESTATE; 5]);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::MOUNTAIN_FOLK));
    choose(&mut g, Choice::Yes);
    assert_eq!((g.players[0].hand.total(), favors(&g, 0)), (8, 0));
    let mut g = agame(I::MOUNTAIN_FOLK, &[I::VILLAGE], 4);
    set_hand(&mut g, 0, &[I::ESTATE; 5]);
    let d = expect_decision(&mut g);
    assert!(d.source != Some(I::MOUNTAIN_FOLK), "{d:?}");
}

#[test]
fn family_of_inventors_puts_a_favor_on_a_pile_to_make_it_cheaper() {
    let mut g = agame(I::FAMILY_OF_INVENTORS, &[I::VILLAGE, I::SMITHY], 2);
    set_hand(&mut g, 0, &[I::ESTATE]);
    // The Buy phase starts at once with an Estate in hand.
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::FAMILY_OF_INVENTORS), "{d:?}");
    choose(&mut g, Choice::Yes);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Token { .. }) || matches!(d.kind, DecisionKind::Select { .. }), "{d:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(g.cost(I::SMITHY), 3);
    assert_eq!(g.cost(I::VILLAGE), 3);
    assert_eq!(favors(&g, 0), 1);
    // The cost is lower for everyone, and a Victory pile cannot carry a token.
    assert_eq!(g.al.fam.get(I::SMITHY), 1);
}

#[test]
fn league_of_bankers_gives_a_coin_per_four_favors_at_the_start_of_the_buy_phase() {
    let mut g = agame(I::LEAGUE_OF_BANKERS, &[I::VILLAGE], 9);
    set_hand(&mut g, 0, &[I::ESTATE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn market_towns_plays_an_action_from_the_hand_for_a_favor_at_the_start_of_the_buy_phase() {
    let mut g = agame(I::MARKET_TOWNS, &[I::VILLAGE], 2);
    // No Action to play from the hand: no offer; with one in hand during the Buy phase the Ally asks.
    set_hand(&mut g, 0, &[I::VILLAGE, I::ESTATE]);
    g.turn.actions = 0;
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::MARKET_TOWNS), "{d:?}");
    choose(&mut g, Choice::Yes);
    // (The only Action in the hand is played without a question, and then there is nothing left to play: no second offer.)
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!((favors(&g, 0), g.players[0].in_play.get(I::VILLAGE)), (1, 1));
}

#[test]
fn peaceful_cult_trashes_cards_for_favors_spent() {
    let mut g = agame(I::PEACEFUL_CULT, &[I::VILLAGE], 3);
    set_hand(&mut g, 0, &[I::ESTATE, I::ESTATE, I::CURSE]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }) && d.source == Some(I::PEACEFUL_CULT), "{d:?}");
    choose(&mut g, Choice::Card(I::CURSE));
    let cs = choices(&g);
    choose(&mut g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
    assert_eq!(g.trash.get(I::CURSE), 1);
    assert_eq!(favors(&g, 0), 2);
}

#[test]
fn woodworkers_guild_trashes_an_action_and_gains_an_action() {
    let mut g = agame(I::WOODWORKERS_GUILD, &[I::VILLAGE, I::SMITHY], 1);
    set_hand(&mut g, 0, &[I::VILLAGE]);
    g.turn.actions = 0;
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::WOODWORKERS_GUILD), "{d:?}");
    choose(&mut g, Choice::Yes);
    // The only Action in the hand is trashed (forced), then an Action is gained.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    assert_eq!(g.trash.get(I::VILLAGE), 1);
    let offered = open_buys(&g);
    assert!(offered.contains(&I::SMITHY) && !offered.contains(&I::SILVER), "{offered:?}");
    choose(&mut g, Choice::Card(I::SMITHY));
    assert_eq!(favors(&g, 0), 0);
}

#[test]
fn circle_of_witches_gives_curses_after_a_liaison_for_three_favors() {
    let mut g = agame(I::CIRCLE_OF_WITCHES, &[I::UNDERLING], 3);
    set_hand(&mut g, 0, &[I::UNDERLING]);
    play(&mut g, I::UNDERLING);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::CIRCLE_OF_WITCHES), "{d:?}");
    choose(&mut g, Choice::Yes);
    // (The Underling gave a Favor too.)
    assert_eq!(favors(&g, 0), 1);
    assert_eq!(g.players[1].discard.get(I::CURSE), 1);
}

#[test]
fn fellowship_of_scribes_draws_a_card_after_an_action_with_four_or_fewer_cards_in_hand() {
    let mut g = agame(I::FELLOWSHIP_OF_SCRIBES, &[I::VILLAGE], 1);
    set_hand(&mut g, 0, &[I::VILLAGE, I::ESTATE]);
    play(&mut g, I::VILLAGE);
    answer(&mut g, I::FELLOWSHIP_OF_SCRIBES, Choice::Yes);
    assert_eq!(g.players[0].hand.total(), 3);
    assert_eq!(favors(&g, 0), 0);
    // With five cards in hand: no offer.
    let mut g = agame(I::FELLOWSHIP_OF_SCRIBES, &[I::VILLAGE], 1);
    set_hand(&mut g, 0, &[I::VILLAGE, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE]);
    play(&mut g, I::VILLAGE);
    let d = expect_decision(&mut g);
    assert!(d.source != Some(I::FELLOWSHIP_OF_SCRIBES), "{d:?}");
}

#[test]
fn league_of_shopkeepers_gives_a_coin_with_five_favors_and_an_action_and_a_buy_with_ten() {
    let mut g = agame(I::LEAGUE_OF_SHOPKEEPERS, &[I::UNDERLING], 5);
    set_hand(&mut g, 0, &[I::UNDERLING]);
    play(&mut g, I::UNDERLING);
    assert_eq!((g.turn.coins, g.turn.actions, g.turn.buys), (1, 1, 1));
    let mut g = agame(I::LEAGUE_OF_SHOPKEEPERS, &[I::UNDERLING], 10);
    set_hand(&mut g, 0, &[I::UNDERLING]);
    play(&mut g, I::UNDERLING);
    assert_eq!((g.turn.coins, g.turn.actions, g.turn.buys), (1, 2, 2));
    let mut g = agame(I::LEAGUE_OF_SHOPKEEPERS, &[I::UNDERLING], 3);
    set_hand(&mut g, 0, &[I::UNDERLING]);
    play(&mut g, I::UNDERLING);
    assert_eq!((g.turn.coins, g.turn.actions, g.turn.buys), (0, 1, 1));
}

#[test]
fn coastal_haven_keeps_cards_in_hand_for_favors() {
    let mut g = agame(I::COASTAL_HAVEN, &[I::VILLAGE], 2);
    set_hand(&mut g, 0, &[I::GOLD, I::SILVER, I::ESTATE]);
    set_deck_known(&mut g, 0, &[I::COPPER; 12]);
    // The Treasures are played in the Buy phase; end the turn.
    g.turn.phase = Phase::Buy;
    g.turn.coins = 0;
    loop {
        let d = expect_decision(&mut g);
        if d.source == Some(I::COASTAL_HAVEN) {
            break;
        }
        let cs = choices(&g);
        choose(&mut g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
    }
    choose(&mut g, Choice::Card(I::ESTATE));
    // Next turn's hand: the kept card and five new ones.
    assert_eq!(favors(&g, 0), 1);
    assert!(g.players[0].hand.get(I::ESTATE) == 1 && g.players[0].hand.total() == 6);
}

#[test]
fn island_folk_takes_an_extra_turn_for_five_favors() {
    let mut g = agame(I::ISLAND_FOLK, &[I::VILLAGE], 5);
    g.turn.phase = Phase::Buy;
    let n = g.turn.number;
    let mut asked = false;
    for _ in 0..40 {
        let d = expect_decision(&mut g);
        if d.source == Some(I::ISLAND_FOLK) {
            asked = true;
            choose(&mut g, Choice::Yes);
            continue;
        }
        if g.turn.number > n {
            break;
        }
        let cs = choices(&g);
        choose(&mut g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
    }
    assert!(asked);
    assert_eq!((g.turn.player, g.turn.is_extra_turn, favors(&g, 0)), (0, true, 0));
}

#[test]
fn plateau_shepherds_pairs_favors_with_cards_costing_two() {
    let score = |fav: u8, estates: usize| {
        let mut g = new_state(&[I::VILLAGE, I::PLATEAU_SHEPHERDS], 2);
        g.players[0].al.favors = fav;
        set_hand(&mut g, 0, &vec![I::ESTATE; estates]);
        g.scores()[0]
    };
    // Estates cost $2: each pair of a Favor and an Estate is worth 2 VP (on top of the Estate's own).
    assert_eq!(score(3, 2) - score(0, 2), 4);
    assert_eq!(score(1, 2) - score(0, 2), 2);
    assert_eq!(score(5, 0) - score(0, 0), 0);
}

#[test]
fn order_of_astrologers_puts_cards_on_top_of_the_deck_when_shuffling_for_favors() {
    let mut g = agame(I::ORDER_OF_ASTROLOGERS, &[I::VILLAGE], 2);
    set_hand(&mut g, 0, &[]);
    clear_deck(&mut g, 0);
    set_discard(&mut g, 0, &[I::GOLD, I::SILVER, I::COPPER, I::ESTATE, I::DUCHY, I::CURSE]);
    // Drawing from the empty deck shuffles: the ability is offered.
    g.stack.push(state::Frame::new(state::FrameKind::Draw, 0, I::VILLAGE));
    let mut f = g.stack.top().unwrap();
    f.max = 1;
    g.stack.set_top(f);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::ORDER_OF_ASTROLOGERS), "{d:?}");
}

#[test]
fn order_of_masons_keeps_cards_out_of_a_shuffle_for_favors() {
    let mut g = agame(I::ORDER_OF_MASONS, &[I::VILLAGE], 1);
    set_hand(&mut g, 0, &[]);
    clear_deck(&mut g, 0);
    set_discard(&mut g, 0, &[I::GOLD, I::SILVER, I::COPPER, I::ESTATE, I::DUCHY, I::CURSE]);
    g.stack.push(state::Frame::new(state::FrameKind::Draw, 0, I::VILLAGE));
    let mut f = g.stack.top().unwrap();
    f.max = 1;
    g.stack.set_top(f);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(I::ORDER_OF_MASONS), "{d:?}");
}

// ===========================================================================
// Same-named copies, Throne Room and the stop-moving rule
// ===========================================================================

#[test]
fn a_conjurer_played_twice_returns_to_the_hand_once_and_a_second_copy_stays_where_it_is() {
    let mut g = game(&[I::STUDENT, I::THRONE_ROOM, I::VILLAGE]);
    // A second Conjurer sits in the discard pile.
    set_hand(&mut g, 0, &[I::THRONE_ROOM, I::CONJURER]);
    set_discard(&mut g, 0, &[I::CONJURER]);
    play(&mut g, I::THRONE_ROOM);
    choose(&mut g, Choice::Card(I::CONJURER));
    // Two gains of a card costing up to $4.
    for _ in 0..2 {
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
        choose(&mut g, Choice::Card(I::VILLAGE));
    }
    to_my_next_turn(&mut g);
    // One Conjurer came back to the hand (the other copy is in the discard pile or already drawn again: three cards in all).
    let ps = &g.players[0];
    let conjurers = ps.hand.get(I::CONJURER) + ps.in_play.get(I::CONJURER) + ps.discard.get(I::CONJURER) + ps.deck_known.iter_top_down().filter(|&c| c == I::CONJURER).count() as u8;
    assert_eq!(conjurers, 2);
    assert_eq!(g.players[0].in_play.get(I::CONJURER), 0);
}

#[test]
fn a_highwayman_played_twice_is_discarded_once() {
    let mut g = game(&[I::HIGHWAYMAN, I::THRONE_ROOM]);
    set_hand(&mut g, 0, &[I::THRONE_ROOM, I::HIGHWAYMAN]);
    let total = total_cards(&g);
    play(&mut g, I::THRONE_ROOM);
    choose(&mut g, Choice::Card(I::HIGHWAYMAN));
    assert_eq!(g.players[0].al.hw, 2);
    to_my_next_turn(&mut g);
    assert_eq!(g.players[0].al.hw, 0);
    assert_eq!(total_cards(&g), total);
    assert_eq!(g.players[0].in_play.get(I::HIGHWAYMAN), 0);
}

#[test]
fn a_trashed_lich_is_discarded_and_gains_a_cheaper_card_from_the_trash() {
    let mut g = game(&[I::STUDENT]);
    g.trash.add(I::SILVER, 1);
    g.trash.add(I::GOLD, 1);
    // A Student trashes the only other card in the hand, the Lich.
    set_hand(&mut g, 0, &[I::STUDENT, I::LICH]);
    play(&mut g, I::STUDENT);
    answer(&mut g, I::STUDENT, Choice::No);
    // The Lich is in the discard pile; a card costing less than $6 is gained from the trash: the Gold ($6) is not allowed, the Silver is (the only one: no question).
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::LICH), 1);
    assert_eq!(g.players[0].discard.get(I::SILVER), 1);
    assert_eq!(g.trash.get(I::GOLD), 1);
}
