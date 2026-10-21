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
    let mut g = new_state(kingdom, 2);
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
