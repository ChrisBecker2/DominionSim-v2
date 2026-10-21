//! Nocturne tests (rulings in `docs/nocturne-plan.md`): the Night phase, Boons and Hexes, States, Heirlooms, the cards outside the Supply and the
//! 33 kingdom cards.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::state::{
    BOON_CHANCE, HEX_CHANCE, N_BOONS, N_HEXES, P_DELUDED, P_ENVIOUS, P_GUARDED, P_MISERABLE, P_TWICE_MISERABLE,
};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

// ===========================================================================
// Random games: nothing is lost, nothing panics, every game ends
// ===========================================================================

#[test]
fn random_nocturne_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0x0C70);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Nocturne).collect();
    assert_eq!(pool.len(), 33);
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::Nocturne).collect();
    let mut played = std::collections::BTreeSet::new();
    let mut boons = std::collections::BTreeSet::new();
    let mut hexes = std::collections::BTreeSet::new();
    let mut night_plays = 0u32;
    for game in 0..3000u64 {
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
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom, seed: game, max_turns: 300, ..Default::default() });
        let initial = total_cards(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        let mut log: Vec<Event> = Vec::new();
        loop {
            for e in &log {
                match *e {
                    Event::Play { card, .. } | Event::PlayAgain { card, .. } => {
                        played.insert(card);
                    }
                    Event::Boon { boon, .. } => {
                        boons.insert(boon);
                    }
                    Event::Hex { hex, .. } => {
                        hexes.insert(hex);
                    }
                    Event::PhaseStart { phase: Phase::Night, .. } => night_plays += 1,
                    _ => {}
                }
            }
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
            if total_cards(&g) != initial {
                let mut sums = g.supply;
                sums.add_all(&g.trash);
                for p in 0..g.num_players as usize {
                    sums.add_all(&g.players[p].all_cards());
                }
                let diff: Vec<String> = sums.iter().map(|(c, k)| format!("{}:{k}", cards::name(c))).collect();
                panic!(
                    "card conservation, game {game}, chose {chosen:?}: {log:?}\nzones: {diff:?}\nside {} (started with {initial}, now {})",
                    g.side_pile_cards(),
                    total_cards(&g)
                );
            }
            // The Boons and Hexes are conserved too: each is in the deck, the discard pile, set aside for Druid, held in front of a player,
            // or in a frame's hands (being resolved).
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
    }
    // The random games reach every card, Boon and Hex.
    // (Cemetery is a Victory card: it is gained, never played.)
    let missing: Vec<&str> =
        (cards::id::DRUID..=cards::id::ZOMBIE_SPY).filter(|c| *c != cards::id::CEMETERY && !played.contains(c)).map(cards::name).collect();
    assert!(missing.is_empty(), "never played: {missing:?}");
    let missing: Vec<&str> = (cards::FIRST_BOON..cards::FIRST_BOON + 12).filter(|c| !boons.contains(c)).map(cards::name).collect();
    assert!(missing.is_empty(), "never received: {missing:?}");
    let missing: Vec<&str> = (cards::FIRST_HEX..cards::FIRST_HEX + 12).filter(|c| !hexes.contains(c)).map(cards::name).collect();
    assert!(missing.is_empty(), "never received: {missing:?}");
    assert!(night_plays > 100, "night phases: {night_plays}");
}

// ===========================================================================
// Helpers
// ===========================================================================

fn bit(card: CardId, first: CardId) -> u16 {
    1 << (card - first)
}

/// Make `b` the only Boon in the deck, so that the next Boon drawn is it.
fn next_boon(g: &mut GameState, b: CardId) {
    g.noc.boon_deck = bit(b, cards::FIRST_BOON);
    g.noc.boon_discard = 0;
}

/// Make `h` the only Hex in the deck, so that the next Hex drawn is it.
fn next_hex(g: &mut GameState, h: CardId) {
    g.noc.hex_deck = bit(h, cards::FIRST_HEX);
    g.noc.hex_discard = 0;
}

/// A deck of cards that are neither Treasures nor Actions: every draw is visible, nothing is auto-played.
fn junk() -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(40).copied().collect()
}

/// Player 0's Buy phase, passing straight on to the Night decision.
fn at_night(g: &mut GameState) -> Decision {
    g.turn.phase = Phase::Buy;
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::Buy, "expected the Buy decision, got {d:?}");
    pass(g);
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::PlayNight, "expected the Night decision, got {d:?}");
    d
}

fn night(g: &mut GameState, c: CardId) -> Step {
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::PlayNight, "expected the Night decision, got {d:?}");
    choose(g, Choice::Card(c))
}

fn pick(g: &mut GameState, c: CardId) -> Step {
    choose(g, Choice::Card(c))
}

fn yes(g: &mut GameState) -> Step {
    choose(g, Choice::Yes)
}

fn no(g: &mut GameState) -> Step {
    choose(g, Choice::No)
}

fn count(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].all_cards().get(c)
}

/// Run the rest of the turn out until the next one has started.
fn end_turn(g: &mut GameState) {
    let start = g.turn.number;
    loop {
        match g.advance(&mut NoEvents) {
            Step::Decision(d) => {
                if g.turn.number != start {
                    return;
                }
                let c = match d.kind {
                    DecisionKind::PlayAction | DecisionKind::Buy | DecisionKind::PlayNight => Choice::Pass,
                    DecisionKind::YesNo { .. } => Choice::Yes,
                    _ => choices(g)[0],
                };
                g.apply(c, &mut NoEvents).unwrap();
            }
            Step::GameOver => return,
            other => panic!("unexpected {other:?}"),
        }
    }
}

fn kingdom_with(cards_: &[CardId]) -> Vec<CardId> {
    let mut k = cards_.to_vec();
    for c in [id::VILLAGE, id::SMITHY, id::MARKET, id::MILITIA, id::MOAT, id::WORKSHOP, id::CELLAR, id::MINE, id::REMODEL, id::MERCHANT] {
        if k.len() >= 10 {
            break;
        }
        if !k.contains(&c) {
            k.push(c);
        }
    }
    k
}

fn new_game(cards_: &[CardId], n: usize, seed: u64) -> GameState {
    GameState::new(&GameConfig { num_players: n, kingdom: kingdom_with(cards_), seed, max_turns: 300, shelters: Some(false), ..Default::default() })
}

// ===========================================================================
// Set-up
// ===========================================================================

#[test]
fn the_set_has_all_its_cards() {
    assert_eq!(CardSet::ALL.len(), 15);
    assert_eq!(cards::set_by_name("Nocturne"), Some(CardSet::Nocturne));
    assert_eq!(cards::kingdom_cards_in(CardSet::Nocturne).count(), 33);
    assert_eq!(cards::NUM_CARDS, 629);
    assert_eq!(cards::NUM_ZONE_IDS, 454);
    assert!(cards::is_boon(id::THE_EARTHS_GIFT) && cards::is_boon(id::THE_WINDS_GIFT) && !cards::is_boon(id::BAD_OMENS));
    assert!(cards::is_hex(id::BAD_OMENS) && cards::is_hex(id::WAR) && !cards::is_hex(id::LOST_IN_THE_WOODS));
    assert!(cards::is_state(id::LOST_IN_THE_WOODS) && cards::is_state(id::TWICE_MISERABLE));
    for c in cards::FIRST_BOON..cards::NUM_CARDS as CardId {
        assert!(cards::is_pseudo(c) && !cards::is_kingdom(c));
    }
    for (c, t) in [
        (id::WEREWOLF, cards::ACTION | cards::NIGHT | cards::ATTACK | cards::DOOM),
        (id::GHOST, cards::NIGHT | cards::DURATION | cards::SPIRIT),
        (id::IDOL, cards::TREASURE | cards::ATTACK | cards::FATE),
        (id::PASTURE, cards::TREASURE | cards::VICTORY | cards::HEIRLOOM),
        (id::ZOMBIE_SPY, cards::ACTION | cards::ZOMBIE),
        (id::CEMETERY, cards::VICTORY),
        (id::RAIDER, cards::NIGHT | cards::DURATION | cards::ATTACK),
    ] {
        assert_eq!(cards::def(c).types, t, "{}", cards::name(c));
    }
    for (c, cost) in [
        (id::DRUID, 2), (id::FAITHFUL_HOUND, 2), (id::GUARDIAN, 2), (id::MONASTERY, 2), (id::PIXIE, 2), (id::TRACKER, 2),
        (id::CHANGELING, 3), (id::FOOL, 3), (id::GHOST_TOWN, 3), (id::LEPRECHAUN, 3), (id::NIGHT_WATCHMAN, 3), (id::SECRET_CAVE, 3),
        (id::BARD, 4), (id::BLESSED_VILLAGE, 4), (id::CEMETERY, 4), (id::CONCLAVE, 4), (id::DEVILS_WORKSHOP, 4), (id::EXORCIST, 4),
        (id::NECROMANCER, 4), (id::SHEPHERD, 4), (id::SKULK, 4), (id::COBBLER, 5), (id::CRYPT, 5), (id::CURSED_VILLAGE, 5),
        (id::DEN_OF_SIN, 5), (id::IDOL, 5), (id::POOKA, 5), (id::SACRED_GROVE, 5), (id::TORMENTOR, 5), (id::TRAGIC_HERO, 5),
        (id::VAMPIRE, 5), (id::WEREWOLF, 5), (id::RAIDER, 6),
        (id::HAUNTED_MIRROR, 0), (id::MAGIC_LAMP, 0), (id::GOAT, 2), (id::PASTURE, 2), (id::POUCH, 2), (id::CURSED_GOLD, 4), (id::LUCKY_COIN, 4),
        (id::WILL_O_WISP, 0), (id::WISH, 0), (id::BAT, 2), (id::IMP, 2), (id::GHOST, 4),
        (id::ZOMBIE_APPRENTICE, 3), (id::ZOMBIE_MASON, 3), (id::ZOMBIE_SPY, 3),
    ] {
        assert_eq!(cards::cost(c), cost, "{}", cards::name(c));
    }
    assert_eq!(cards::by_name("the earths gift"), Some(id::THE_EARTHS_GIFT));
    assert_eq!(cards::by_name("Devil's Workshop"), Some(id::DEVILS_WORKSHOP));
    assert_eq!(cards::by_name("Will-o'-Wisp"), Some(id::WILL_O_WISP));
}

#[test]
fn heirlooms_replace_a_starting_copper_and_the_coppers_go_to_the_pile() {
    let g = new_game(&[id::PIXIE, id::TRACKER], 2, 1);
    for p in 0..2 {
        let d = g.players[p].deck_unknown;
        assert_eq!((d.get(id::COPPER), d.get(id::ESTATE), d.get(id::GOAT), d.get(id::POUCH)), (5, 3, 1, 1));
        assert_eq!(d.total(), 10);
    }
    assert_eq!(g.supply.get(id::COPPER), 60 - 2 * 5, "the unused Coppers go in the Copper pile");
    for (card, heirloom) in [
        (id::CEMETERY, id::HAUNTED_MIRROR), (id::SECRET_CAVE, id::MAGIC_LAMP), (id::PIXIE, id::GOAT), (id::SHEPHERD, id::PASTURE),
        (id::TRACKER, id::POUCH), (id::POOKA, id::CURSED_GOLD), (id::FOOL, id::LUCKY_COIN),
    ] {
        let g = new_game(&[card], 3, 2);
        assert_eq!(g.players[2].deck_unknown.get(heirloom), 1, "{} comes with {}", cards::name(card), cards::name(heirloom));
        assert_eq!(g.players[2].deck_unknown.get(id::COPPER), 6);
    }
    let g = new_game(&[id::BARD], 2, 3);
    assert_eq!(g.players[0].deck_unknown.get(id::COPPER), 7);
    let g = new_game(&[id::CEMETERY, id::SECRET_CAVE, id::PIXIE, id::SHEPHERD, id::TRACKER, id::POOKA, id::FOOL], 2, 4);
    assert_eq!(g.players[0].deck_unknown.get(id::COPPER), 0);
    assert_eq!(g.players[0].deck_unknown.total(), 10);
}

#[test]
fn heirlooms_are_dealt_into_the_shuffled_starting_deck() {
    let mut hits = 0;
    let games = 4000;
    for seed in 0..games {
        let mut g = new_game(&[id::PIXIE], 2, seed);
        g.deal_opening_hands(&mut NoEvents);
        hits += g.players[0].hand.has(id::GOAT) as u32;
    }
    let rate = hits as f64 / games as f64;
    assert!((0.46..0.54).contains(&rate), "Goat in the opening hand in {rate:.3} of the games, expected 0.5");
}

#[test]
fn a_game_with_shelters_and_an_heirloom_has_both() {
    let g = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom_with(&[id::PIXIE]), seed: 1, shelters: Some(true), ..Default::default() });
    let d = g.players[0].deck_unknown;
    assert_eq!((d.get(id::COPPER), d.get(id::GOAT), d.get(id::HOVEL), d.get(id::NECROPOLIS), d.get(id::OVERGROWN_ESTATE), d.get(id::ESTATE)), (6, 1, 1, 1, 1, 0));
}

#[test]
fn the_extra_piles_come_with_the_cards_that_use_them() {
    let piles = |g: &GameState| g.noc.piles;
    assert_eq!(piles(&new_game(&[id::BARD], 2, 1)), [12, 0, 0, 0, 0]);
    assert_eq!(piles(&new_game(&[id::LEPRECHAUN], 2, 1)), [0, 12, 0, 0, 0]);
    assert_eq!(piles(&new_game(&[id::SECRET_CAVE], 2, 1)), [0, 12, 0, 0, 0]);
    assert_eq!(piles(&new_game(&[id::VAMPIRE], 2, 1)), [0, 0, 10, 0, 0]);
    assert_eq!(piles(&new_game(&[id::DEVILS_WORKSHOP], 2, 1)), [0, 0, 0, 13, 0]);
    assert_eq!(piles(&new_game(&[id::TORMENTOR], 2, 1)), [0, 0, 0, 13, 0]);
    assert_eq!(piles(&new_game(&[id::CEMETERY], 2, 1)), [0, 0, 0, 0, 6]);
    assert_eq!(piles(&new_game(&[id::EXORCIST], 2, 1)), [12, 0, 0, 13, 6], "Exorcist brings all three Spirit piles");
    assert_eq!(piles(&new_game(&[id::MONASTERY], 2, 1)), [0; 5]);
    assert_eq!(piles(&new_game(&[id::EXORCIST], 5, 1)), [12, 0, 0, 13, 6]);
    let g = new_game(&[id::NECROMANCER], 2, 1);
    assert_eq!((g.trash.get(id::ZOMBIE_APPRENTICE), g.trash.get(id::ZOMBIE_MASON), g.trash.get(id::ZOMBIE_SPY)), (1, 1, 1));
    assert!(new_game(&[id::BARD], 2, 1).trash.is_empty());
    let g = new_game(&[id::EXORCIST], 2, 1);
    assert!(!g.in_supply(id::IMP) && !g.in_supply(id::WILL_O_WISP) && !g.in_supply(id::GHOST));
    assert_eq!(g.side_pile_cards(), 12 + 13 + 6);
}

#[test]
fn boons_and_hexes_are_present_only_with_a_fate_or_doom_card() {
    let g = new_game(&[id::BARD], 2, 1);
    assert_eq!((g.noc.boon_deck, g.noc.hex_deck), (0xFFF, 0));
    assert!(g.noc.flags & N_BOONS != 0 && g.noc.flags & N_HEXES == 0);
    let g = new_game(&[id::SKULK], 2, 1);
    assert_eq!((g.noc.boon_deck, g.noc.hex_deck), (0, 0xFFF));
    let g = new_game(&[id::BARD, id::WEREWOLF], 2, 1);
    assert_eq!((g.noc.boon_deck, g.noc.hex_deck), (0xFFF, 0xFFF));
    let g = new_game(&[id::MONASTERY, id::FAITHFUL_HOUND], 2, 1);
    assert_eq!((g.noc.boon_deck, g.noc.hex_deck), (0, 0));
    for c in cards::kingdom_cards_in(CardSet::Nocturne) {
        let g = new_game(&[c], 2, 1);
        assert_eq!(g.noc.boon_deck != 0, cards::is(c, cards::FATE), "{}", cards::name(c));
        assert_eq!(g.noc.hex_deck != 0, cards::is(c, cards::DOOM), "{}", cards::name(c));
    }
}

#[test]
fn druid_sets_aside_three_boons_face_up() {
    for seed in 0..50 {
        let g = new_game(&[id::DRUID], 2, seed);
        assert_eq!(g.noc.boon_druid.count_ones(), 3);
        assert_eq!(g.noc.boon_deck.count_ones(), 9);
        assert_eq!(g.noc.boon_deck | g.noc.boon_druid, 0xFFF);
    }
    let sets: std::collections::BTreeSet<u16> = (0..50).map(|s| new_game(&[id::DRUID], 2, s).noc.boon_druid).collect();
    assert!(sets.len() > 20, "the three Boons vary from game to game");
    let mut g = new_game(&[id::DRUID], 2, 1);
    let first = g.noc.boon_druid;
    let mut changed = false;
    for _ in 0..20 {
        g.redeal_druid_boons();
        assert_eq!((g.noc.boon_druid.count_ones(), g.noc.boon_deck.count_ones()), (3, 9));
        changed |= g.noc.boon_druid != first;
    }
    assert!(changed);
}

// ===========================================================================
// The Night phase
// ===========================================================================

#[test]
fn night_follows_the_buy_phase_only_for_a_player_holding_a_night_card() {
    // With a Night card in hand: Action, Buy, Night, then Clean-up.
    let mut g = new_state(&[id::GHOST_TOWN, id::MONASTERY], 2);
    set_hand(&mut g, 0, &[id::MONASTERY, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    let mut log: Vec<Event> = Vec::new();
    g.turn.phase = Phase::Buy;
    let d = match g.advance(&mut log) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::Buy);
    g.apply(Choice::Pass, &mut log).unwrap();
    let d = match g.advance(&mut log) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::PlayNight);
    assert_eq!(g.turn.phase, Phase::Night);
    assert!(log.iter().any(|e| matches!(e, Event::PhaseStart { phase: Phase::Night, .. })));
    assert_eq!(choices(&g), vec![Choice::Card(id::MONASTERY), Choice::Pass]);
    // Passing ends the Night phase; the Night card is discarded at Clean-up.
    g.apply(Choice::Pass, &mut log).unwrap();
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].discard.has(id::MONASTERY) || g.players[0].hand.has(id::MONASTERY));

    // Without a Night card in hand there is no Night phase at all, even in a game with Night cards.
    let mut g = new_state(&[id::GHOST_TOWN, id::MONASTERY], 2);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    let mut log: Vec<Event> = Vec::new();
    g.turn.phase = Phase::Buy;
    let _ = g.advance(&mut log);
    g.apply(Choice::Pass, &mut log).unwrap();
    loop {
        match g.advance(&mut log) {
            Step::Decision(_) => break,
            s => panic!("{s:?}"),
        }
    }
    assert!(!log.iter().any(|e| matches!(e, Event::PhaseStart { phase: Phase::Night, .. })));
    assert_eq!(g.turn.player, 1);
}

#[test]
fn a_game_without_night_cards_has_no_night_phase() {
    let mut g = new_state(&[id::BARD, id::VILLAGE], 2);
    assert_eq!(g.noc.flags & dominion_engine::state::N_NIGHT, 0);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    let _ = expect_decision(&mut g);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1, "the next turn starts right away");
}

#[test]
fn night_cards_are_gained_to_the_hand_and_can_be_played_the_turn_they_are_bought() {
    for card in [id::GUARDIAN, id::GHOST_TOWN, id::NIGHT_WATCHMAN, id::DEN_OF_SIN] {
        let mut g = new_state(&[card], 2);
        set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
        set_deck_known(&mut g, 0, &junk());
        g.turn.phase = Phase::Buy;
        let d = expect_decision(&mut g);
        assert_eq!(d.kind, DecisionKind::Buy);
        buy(&mut g, card);
        assert!(g.players[0].hand.has(card), "{} is gained to the hand", cards::name(card));
        assert!(!g.players[0].discard.has(card));
        // Pass the rest of the Buy phase: the Night phase offers the card just bought.
        let d = expect_decision(&mut g);
        if d.kind == DecisionKind::Buy {
            pass(&mut g);
        }
        let d = expect_decision(&mut g);
        assert_eq!(d.kind, DecisionKind::PlayNight, "{}", cards::name(card));
    }
    // Gained onto the deck (Armory), it stays there.
    let mut g = new_state(&[id::GUARDIAN], 2);
    g.gain_card(0, id::GUARDIAN, Dest::DeckTop, &mut NoEvents);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GUARDIAN));
    // Other cards still go to the discard pile.
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    assert!(g.players[0].discard.has(id::SILVER));
}

#[test]
fn werewolf_draws_by_day_and_hexes_at_night() {
    let mut g = new_state(&[id::WEREWOLF], 2);
    set_hand(&mut g, 0, &[id::WEREWOLF]);
    set_deck_known(&mut g, 0, &junk());
    next_hex(&mut g, id::GREED);
    play(&mut g, id::WEREWOLF);
    assert_eq!(g.players[0].hand.total(), 3, "+3 Cards in the Action phase");
    assert_eq!(g.players[1].deck_known.peek_top(), None, "no Hex by day");
    assert_eq!(g.noc.hex_deck, bit(id::GREED, cards::FIRST_HEX));

    let mut g = new_state(&[id::WEREWOLF], 2);
    set_hand(&mut g, 0, &[id::WEREWOLF]);
    set_deck_known(&mut g, 0, &junk());
    next_hex(&mut g, id::GREED);
    // Played at Night: no cards, no Action used; the other player receives the next Hex.
    g.turn.actions = 0;
    at_night(&mut g);
    night(&mut g, id::WEREWOLF);
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::COPPER), "Greed: a Copper onto the deck");
    assert!(g.noc.hex_discard & bit(id::GREED, cards::FIRST_HEX) != 0);
}

#[test]
fn durations_played_at_night_stay_in_play_and_fire_next_turn() {
    let mut g = new_state(&[id::GHOST_TOWN], 2);
    set_hand(&mut g, 0, &[id::GHOST_TOWN]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    at_night(&mut g);
    night(&mut g, id::GHOST_TOWN);
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].in_play.has(id::GHOST_TOWN), "stays in play");
    assert_eq!(g.players[0].pending_durations_len, 1);
    set_hand(&mut g, 1, &[]);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.actions, 2, "+1 Action at the start of the next turn");
    assert_eq!(g.players[0].hand.total(), 6, "+1 Card");
}

// ===========================================================================
// Boons
// ===========================================================================

/// Player 0 plays Bard with `boon` next, the hand otherwise `hand` and the deck known `deck`.
fn bard_with(boon: CardId, hand: &[CardId], deck: &[CardId]) -> (GameState, Step) {
    let mut g = new_state(&[id::BARD, id::DRUID], 2);
    let mut h = vec![id::BARD];
    h.extend_from_slice(hand);
    set_hand(&mut g, 0, &h);
    set_deck_known(&mut g, 0, deck);
    next_boon(&mut g, boon);
    let st = play(&mut g, id::BARD);
    (g, st)
}

#[test]
fn bard_gives_two_coins_and_a_boon() {
    let (g, _) = bard_with(id::THE_MOUNTAINS_GIFT, &[], &junk());
    assert_eq!(g.turn.coins, 2);
    assert!(g.players[0].discard.has(id::SILVER), "The Mountain's Gift: a Silver");
    assert!(g.noc.boon_discard & bit(id::THE_MOUNTAINS_GIFT, cards::FIRST_BOON) != 0, "received Boons go to the Boons discard pile");
    assert_eq!(g.noc.boon_deck, 0);
}

#[test]
fn the_earths_gift_discards_a_treasure_to_gain_a_card_up_to_4() {
    let (mut g, st) = bard_with(id::THE_EARTHS_GIFT, &[id::COPPER, id::ESTATE], &junk());
    assert!(matches!(st, Step::Decision(_)));
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass], "only Treasures can be discarded");
    pick(&mut g, id::COPPER);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { max_cost: 4, .. }));
    pick(&mut g, id::SILVER);
    assert!(g.players[0].discard.has(id::SILVER) && g.players[0].discard.has(id::COPPER));
    // It is optional.
    let (mut g, _) = bard_with(id::THE_EARTHS_GIFT, &[id::COPPER], &junk());
    pass(&mut g);
    assert!(g.players[0].hand.has(id::COPPER) || g.players[0].in_play.has(id::COPPER));
}

#[test]
fn the_fields_gift_gives_an_action_and_a_coin_and_is_kept_until_cleanup() {
    let (mut g, _) = bard_with(id::THE_FIELDS_GIFT, &[], &junk());
    assert_eq!((g.turn.actions, g.turn.coins), (1, 3), "+1 Action (the Bard used one) and +$1 on top of the $2");
    assert!(g.noc.boon_held & bit(id::THE_FIELDS_GIFT, cards::FIRST_BOON) != 0);
    assert_eq!(g.noc.boon_discard, 0);
    end_turn(&mut g);
    assert_eq!(g.noc.boon_held, 0);
    assert!(g.noc.boon_discard & bit(id::THE_FIELDS_GIFT, cards::FIRST_BOON) != 0, "back in the discard pile at Clean-up");
}

#[test]
fn the_flames_gift_may_trash_a_card_from_the_hand() {
    let (mut g, _) = bard_with(id::THE_FLAMES_GIFT, &[id::CURSE, id::ESTATE], &junk());
    pick(&mut g, id::CURSE);
    assert_eq!(g.trash.get(id::CURSE), 1);
    let (mut g, _) = bard_with(id::THE_FLAMES_GIFT, &[id::CURSE], &junk());
    pass(&mut g);
    assert!(g.trash.is_empty());
}

#[test]
fn the_forests_gift_gives_a_buy_and_a_coin() {
    let (g, _) = bard_with(id::THE_FORESTS_GIFT, &[], &junk());
    assert_eq!((g.turn.buys, g.turn.coins), (2, 3));
    assert!(g.noc.boon_held & bit(id::THE_FORESTS_GIFT, cards::FIRST_BOON) != 0);
}

#[test]
fn the_moons_gift_puts_a_card_from_the_discard_pile_onto_the_deck() {
    let mut g = new_state(&[id::BARD], 2);
    set_hand(&mut g, 0, &[id::BARD]);
    set_discard(&mut g, 0, &[id::GOLD, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOONS_GIFT);
    play(&mut g, id::BARD);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    // An empty discard pile: nothing happens.
    let (g, st) = bard_with(id::THE_MOONS_GIFT, &[], &junk());
    assert!(matches!(st, Step::Decision(_)));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));
}

#[test]
fn the_rivers_gift_draws_a_card_after_the_next_hand_is_drawn() {
    let (mut g, _) = bard_with(id::THE_RIVERS_GIFT, &[], &junk());
    assert!(g.noc.boon_held & bit(id::THE_RIVERS_GIFT, cards::FIRST_BOON) != 0);
    set_deck_known(&mut g, 0, &junk());
    end_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 6, "5 cards and the River's Gift's card");
}

#[test]
fn the_seas_gift_draws_a_card() {
    let (g, _) = bard_with(id::THE_SEAS_GIFT, &[], &[id::PROVINCE, id::DUCHY]);
    assert_eq!(g.players[0].hand.get(id::PROVINCE), 1);
}

#[test]
fn the_skys_gift_discards_three_cards_for_a_gold() {
    let (mut g, _) = bard_with(id::THE_SKYS_GIFT, &[id::ESTATE, id::ESTATE, id::ESTATE, id::DUCHY], &junk());
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    assert!(g.players[0].discard.has(id::GOLD));
    assert_eq!(g.players[0].hand.get(id::DUCHY), 1, "{:?} {:?}", g.players[0].hand.iter().collect::<Vec<_>>(), g.stack.len);
    // With fewer than three cards in hand the rest is discarded and no Gold comes.
    let (mut g, _) = bard_with(id::THE_SKYS_GIFT, &[id::ESTATE, id::DUCHY], &junk());
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    assert_eq!(g.players[0].hand.total(), 0);
    assert!(!g.players[0].discard.has(id::GOLD));
    // Optional.
    let (mut g, _) = bard_with(id::THE_SKYS_GIFT, &[id::ESTATE, id::ESTATE, id::ESTATE], &junk());
    pass(&mut g);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn the_suns_gift_looks_at_four_cards_discards_some_and_reorders_the_rest() {
    let (mut g, _) = bard_with(id::THE_SUNS_GIFT, &[], &[id::GOLD, id::ESTATE, id::SILVER, id::CURSE, id::DUCHY]);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    // Put back one at a time; the last put back is on top.
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD), "the last put back is on top");
    assert_eq!(g.players[0].discard.get(id::ESTATE) + g.players[0].discard.get(id::CURSE), 2);
}

#[test]
fn the_swamps_gift_gains_a_will_o_wisp_from_its_pile() {
    let (g, _) = bard_with(id::THE_SWAMPS_GIFT, &[], &junk());
    assert!(g.players[0].discard.has(id::WILL_O_WISP));
    assert_eq!(g.noc.piles[0], 11);
}

#[test]
fn the_winds_gift_draws_two_and_discards_two() {
    let (mut g, _) = bard_with(id::THE_WINDS_GIFT, &[id::ESTATE], &[id::GOLD, id::SILVER, id::DUCHY]);
    assert_eq!(g.players[0].hand.total(), 3);
    pick(&mut g, id::SILVER);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[0].in_play.get(id::GOLD), 1, "kept (and played in the Buy phase)");
    assert_eq!(g.players[0].discard.total(), 2);
}

#[test]
fn the_boons_deck_is_reshuffled_from_the_discard_pile_when_it_runs_out() {
    let mut g = new_state(&[id::BARD], 2);
    g.noc.boon_deck = 0;
    g.noc.boon_discard = bit(id::THE_SEAS_GIFT, cards::FIRST_BOON) | bit(id::THE_MOUNTAINS_GIFT, cards::FIRST_BOON);
    set_hand(&mut g, 0, &[id::BARD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::BARD);
    assert_eq!(g.noc.boon_deck.count_ones() + g.noc.boon_discard.count_ones(), 2);
    assert_eq!(g.noc.boon_discard.count_ones(), 1, "one drawn, one left in the reformed deck");
    // No Boon anywhere: nothing is received.
    let mut g = new_state(&[id::BARD], 2);
    g.noc.boon_deck = 0;
    g.noc.boon_discard = 0;
    set_hand(&mut g, 0, &[id::BARD]);
    play(&mut g, id::BARD);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn boons_are_drawn_at_random_from_the_deck_and_every_one_comes_up() {
    let mut seen = [0u32; 12];
    for seed in 0..2400u64 {
        let mut g = new_state(&[id::BARD], 2);
        g.rng = Rng::new(seed);
        g.noc.boon_discard = 0;
        set_hand(&mut g, 0, &[id::BARD]);
        set_deck_known(&mut g, 0, &junk());
        play(&mut g, id::BARD);
        let drawn = (g.noc.boon_discard | g.noc.boon_held).trailing_zeros() as usize;
        seen[drawn] += 1;
    }
    for (i, n) in seen.iter().enumerate() {
        assert!((120..=280).contains(n), "Boon {i} came up {n} times in 2400 draws");
    }
}

#[test]
fn a_boon_draw_in_chance_mode_is_a_chance_step_over_the_deck() {
    let mut g = new_state(&[id::BARD], 2);
    g.chance_mode = true;
    g.noc.boon_deck = bit(id::THE_SEAS_GIFT, cards::FIRST_BOON) | bit(id::THE_MOUNTAINS_GIFT, cards::FIRST_BOON);
    set_hand(&mut g, 0, &[id::BARD]);
    set_deck_known(&mut g, 0, &junk());
    let st = play(&mut g, id::BARD);
    let Step::Chance { player } = st else { panic!("{st:?}") };
    assert_eq!(player, BOON_CHANCE);
    let outcomes = g.chance_outcomes(player);
    assert_eq!(outcomes.total(), 2);
    assert!(outcomes.has((id::THE_SEAS_GIFT - cards::FIRST_BOON) as CardId) && outcomes.has((id::THE_MOUNTAINS_GIFT - cards::FIRST_BOON) as CardId));
    g.resolve_chance(player, (id::THE_MOUNTAINS_GIFT - cards::FIRST_BOON) as CardId);
    adv(&mut g);
    assert!(g.players[0].discard.has(id::SILVER));
    assert_eq!(g.noc.boon_deck, bit(id::THE_SEAS_GIFT, cards::FIRST_BOON));
}

#[test]
fn determinizing_does_not_reveal_the_next_boon_or_hex() {
    let mut g = new_state(&[id::BARD, id::WEREWOLF], 2);
    set_hand(&mut g, 0, &[id::BARD]);
    let view = PlayerView::new(&g, 0);
    let mut firsts = std::collections::BTreeSet::new();
    for seed in 0..40 {
        let mut w = view.determinize(&mut Rng::new(seed));
        assert_eq!((w.noc.boon_deck, w.noc.hex_deck), (g.noc.boon_deck, g.noc.hex_deck), "only the public sets are stored");
        assert_eq!((w.noc.boon_top, w.noc.hex_top), (0, 0));
        w.chance_mode = false;
        w.players[0].hand = counts_of(&[id::BARD]);
        w.players[0].deck_known.clear();
        w.turn = dominion_engine::state::TurnState::start(0, 1);
        w.stack = dominion_engine::state::FrameStack::default();
        w.pending = Pending::None;
        play(&mut w, id::BARD);
        firsts.insert((w.noc.boon_discard | w.noc.boon_held).trailing_zeros());
    }
    assert!(firsts.len() > 5, "different worlds draw different Boons");
    assert!(view.boon_in_deck(id::THE_SEAS_GIFT) && !view.boon_in_discard(id::THE_SEAS_GIFT) && view.hex_in_deck(id::WAR));
}

#[test]
fn druid_receives_one_of_the_set_aside_boons_and_leaves_it_there() {
    let mut g = new_state(&[id::DRUID], 2);
    g.noc.boon_druid = bit(id::THE_SEAS_GIFT, cards::FIRST_BOON) | bit(id::THE_MOUNTAINS_GIFT, cards::FIRST_BOON) | bit(id::THE_FIELDS_GIFT, cards::FIRST_BOON);
    g.noc.boon_deck = 0xFFF & !g.noc.boon_druid;
    set_hand(&mut g, 0, &[id::DRUID]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DRUID);
    assert_eq!(g.turn.buys, 2, "+1 Buy");
    assert_eq!(choices(&g).len(), 3);
    pick(&mut g, id::THE_FIELDS_GIFT);
    assert_eq!(g.turn.coins, 1);
    assert_eq!(g.noc.boon_druid.count_ones(), 3, "it stays where it is, even a Boon kept until Clean-up");
    assert_eq!(g.noc.boon_held, 0);
    assert_eq!(g.noc.boon_discard, 0);
}

#[test]
fn pixie_discards_the_top_boon_and_may_be_trashed_to_receive_it_twice() {
    let mut g = new_state(&[id::PIXIE], 2);
    set_hand(&mut g, 0, &[id::PIXIE]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    play(&mut g, id::PIXIE);
    assert_eq!(g.players[0].hand.total(), 1, "+1 Card");
    assert_eq!(g.turn.actions, 1, "+1 Action");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.subject == id::PIXIE);
    yes(&mut g);
    assert_eq!(g.trash.get(id::PIXIE), 1);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2, "the Boon twice");
    // Declined: the Boon was just discarded.
    let mut g = new_state(&[id::PIXIE], 2);
    set_hand(&mut g, 0, &[id::PIXIE]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    play(&mut g, id::PIXIE);
    no(&mut g);
    assert!(g.players[0].discard.is_empty() && g.players[0].in_play.has(id::PIXIE));
    assert!(g.noc.boon_discard != 0);
}

#[test]
fn pixie_and_a_kept_boon_twice() {
    let mut g = new_state(&[id::PIXIE], 2);
    set_hand(&mut g, 0, &[id::PIXIE]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_RIVERS_GIFT);
    play(&mut g, id::PIXIE);
    yes(&mut g);
    assert_eq!(g.turn.noc.river[0], 2);
    set_deck_known(&mut g, 0, &junk());
    end_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 7);
}

#[test]
fn tracker_gives_a_coin_a_boon_and_topdecks_what_is_gained() {
    let mut g = new_state(&[id::TRACKER], 2);
    set_hand(&mut g, 0, &[id::TRACKER]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    play(&mut g, id::TRACKER);
    assert_eq!(g.turn.coins, 1);
    // The Silver from the Boon may be put onto the deck: the topdecking applies before the Boon.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Topdeck }) && d.subject == id::SILVER);
    yes(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    // Later gains this turn too, including buys.
    set_hand(&mut g, 0, &[]);
    g.turn.coins = 3;
    g.turn.phase = Phase::Buy;
    buy(&mut g, id::SILVER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Topdeck }));
    no(&mut g);
}

#[test]
fn sacred_grove_shares_the_boon_unless_it_gives_a_coin() {
    let mut g = new_state(&[id::SACRED_GROVE], 3);
    set_hand(&mut g, 0, &[id::SACRED_GROVE]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    play(&mut g, id::SACRED_GROVE);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 3));
    assert!(g.players[0].discard.has(id::SILVER));
    // Each other player may receive it, in turn order.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    yes(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 2);
    no(&mut g);
    assert!(g.players[1].discard.has(id::SILVER) && g.players[2].discard.is_empty());
    // A Boon that gives +$1 is not shared.
    let mut g = new_state(&[id::SACRED_GROVE], 3);
    set_hand(&mut g, 0, &[id::SACRED_GROVE]);
    next_boon(&mut g, id::THE_FIELDS_GIFT);
    play(&mut g, id::SACRED_GROVE);
    assert_eq!(g.turn.coins, 4);
    assert!(g.players[1].hand.is_empty());
    assert_eq!(expect_decision(&mut g).player, 0, "nobody else is asked");
}

#[test]
fn idol_alternates_between_a_boon_and_curses() {
    let mut g = new_state(&[id::IDOL], 2);
    set_hand(&mut g, 0, &[id::IDOL, id::IDOL]);
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    g.turn.phase = Phase::Buy;
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::IDOL);
    assert_eq!(g.turn.coins, 2);
    assert!(g.players[0].discard.has(id::SILVER), "the first Idol: a Boon");
    play_treasure(&mut g, id::IDOL);
    assert_eq!(g.turn.coins, 4);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1, "the second: Curses for the others");
}

#[test]
fn blessed_village_takes_a_boon_when_gained_to_receive_now_or_next_turn() {
    let mut g = new_state(&[id::BLESSED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    g.turn.phase = Phase::Buy;
    buy(&mut g, id::BLESSED_VILLAGE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.subject == id::THE_MOUNTAINS_GIFT, "the Boon is seen first: {d:?}");
    yes(&mut g);
    assert!(g.players[0].discard.has(id::SILVER));
    // Later: it waits in front of the player and is received at the start of their next turn.
    let mut g = new_state(&[id::BLESSED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    g.turn.phase = Phase::Buy;
    buy(&mut g, id::BLESSED_VILLAGE);
    no(&mut g);
    assert_eq!(g.players[0].boons_later, bit(id::THE_MOUNTAINS_GIFT, cards::FIRST_BOON));
    assert_eq!(g.noc.boon_deck | g.noc.boon_discard, 0, "out of the deck while it waits");
    set_hand(&mut g, 1, &[]);
    while g.turn.number < 3 {
        end_turn(&mut g);
    }
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.players[0].boons_later, 0);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
}

#[test]
fn fool_takes_lost_in_the_woods_and_receives_three_boons_in_any_order() {
    let mut g = new_state(&[id::FOOL], 2);
    set_hand(&mut g, 0, &[id::FOOL]);
    set_deck_known(&mut g, 0, &junk());
    g.noc.boon_deck = bit(id::THE_MOUNTAINS_GIFT, cards::FIRST_BOON) | bit(id::THE_FIELDS_GIFT, cards::FIRST_BOON) | bit(id::THE_SEAS_GIFT, cards::FIRST_BOON);
    play(&mut g, id::FOOL);
    assert_eq!(g.noc.lost, 1);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::PickBoon);
    assert_eq!(choices(&g).len(), 3);
    pick(&mut g, id::THE_SEAS_GIFT);
    assert_eq!(g.players[0].hand.total(), 1);
    pick(&mut g, id::THE_FIELDS_GIFT);
    // The last Boon is forced (a single choice is applied automatically).
    assert!(g.players[0].discard.has(id::SILVER));
    assert_eq!(g.turn.coins, 1);
    // A player with Lost in the Woods does nothing with the Fool.
    let mut g = new_state(&[id::FOOL], 2);
    g.noc.lost = 1;
    set_hand(&mut g, 0, &[id::FOOL]);
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    // (Its own Lost in the Woods asks for a discard at the start of the turn: declined.)
    expect_decision(&mut g);
    pass(&mut g);
    play(&mut g, id::FOOL);
    assert!(g.noc.boon_deck != 0 && g.players[0].discard.is_empty());
    // Taking it from another player.
    let mut g = new_state(&[id::FOOL], 2);
    g.noc.lost = 2;
    set_hand(&mut g, 0, &[id::FOOL]);
    play(&mut g, id::FOOL);
    assert_eq!(g.noc.lost, 1);
}

#[test]
fn lost_in_the_woods_discards_a_card_at_the_start_of_each_turn_for_a_boon() {
    let mut g = new_state(&[id::FOOL], 2);
    g.noc.lost = 2;
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    set_hand(&mut g, 0, &[]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY]);
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, max: 1, .. }), "{d:?}");
    pick(&mut g, id::ESTATE);
    assert!(g.players[1].discard.has(id::SILVER) && g.players[1].discard.has(id::ESTATE));
    assert_eq!(g.noc.lost, 2, "it stays until someone else plays a Fool");
}

// ===========================================================================
// Hexes and States
// ===========================================================================

/// Player 0 plays Skulk (+1 Buy, each other player receives the next Hex) with `hex` next; player 1 holds `hand` over `deck` and `discard`.
fn skulk_hex(hex: CardId, hand: &[CardId], deck: &[CardId], discard: &[CardId]) -> (GameState, Step) {
    let mut g = new_state(&[id::SKULK, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::SKULK]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, hand);
    set_deck_known(&mut g, 1, deck);
    set_discard(&mut g, 1, discard);
    next_hex(&mut g, hex);
    let st = play(&mut g, id::SKULK);
    (g, st)
}

#[test]
fn skulk_hexes_the_others_with_one_hex_and_buys() {
    let mut g = new_state(&[id::SKULK], 3);
    set_hand(&mut g, 0, &[id::SKULK]);
    next_hex(&mut g, id::GREED);
    play(&mut g, id::SKULK);
    assert_eq!(g.turn.buys, 2);
    // One Hex was turned over; both other players follow it.
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::COPPER));
    assert_eq!(g.players[2].deck_known.peek_top(), Some(id::COPPER));
    assert_eq!(g.noc.hex_discard, bit(id::GREED, cards::FIRST_HEX));
    assert_eq!(g.noc.hex_deck, 0);
}

#[test]
fn a_hex_is_blocked_by_moat() {
    let mut g = new_state(&[id::SKULK, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::SKULK]);
    set_hand(&mut g, 1, &[id::MOAT]);
    next_hex(&mut g, id::GREED);
    play(&mut g, id::SKULK);
    moat(&mut g, true);
    assert!(g.players[1].deck_known.is_empty());
    assert_eq!(g.noc.hex_deck, bit(id::GREED, cards::FIRST_HEX), "no Hex is turned over when nobody is affected");
}

#[test]
fn bad_omens_puts_the_deck_in_the_discard_pile_and_two_coppers_back_on_top() {
    let (g, _) = skulk_hex(id::BAD_OMENS, &[id::ESTATE], &[id::GOLD, id::SILVER], &[id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    let v = &g.players[1];
    assert_eq!(v.deck_known.len, 2);
    assert_eq!(v.deck_known.peek_top(), Some(id::COPPER));
    assert_eq!(v.deck_counts().get(id::COPPER), 2);
    assert_eq!(v.discard.get(id::COPPER), 1);
    assert_eq!((v.discard.get(id::GOLD), v.discard.get(id::SILVER), v.discard.get(id::ESTATE)), (1, 1, 1));
    // With one Copper only that one goes back on the deck.
    let (g, _) = skulk_hex(id::BAD_OMENS, &[], &[id::GOLD], &[id::COPPER]);
    assert_eq!(g.players[1].deck_counts().total(), 1);
}

#[test]
fn delusion_stops_buying_actions_for_one_turn() {
    let (mut g, _) = skulk_hex(id::DELUSION, &[], &junk(), &[]);
    assert_eq!(g.players[1].noc & P_DELUDED, P_DELUDED);
    // Player 1's next turn: the Buy phase returns Deluded; Actions cannot be bought, other cards can.
    g.turn.number += 1;
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::GOLD, id::GOLD]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.players[1].noc & P_DELUDED, 0, "returned at the start of the Buy phase");
    let cs = choices(&g);
    assert!(!cs.contains(&Choice::Card(id::VILLAGE)) && !cs.contains(&Choice::Card(id::SKULK)), "no Action");
    assert!(cs.contains(&Choice::Card(id::DUCHY)) && cs.contains(&Choice::Card(id::SILVER)));
    // The next turn is normal.
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::GOLD, id::GOLD]);
    expect_decision(&mut g);
    assert!(choices(&g).contains(&Choice::Card(id::SKULK)));
}

#[test]
fn envy_makes_silver_and_gold_worth_one_coin_for_a_turn() {
    let (mut g, _) = skulk_hex(id::ENVY, &[], &junk(), &[]);
    assert_eq!(g.players[1].noc & P_ENVIOUS, P_ENVIOUS);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::SILVER, id::GOLD, id::COPPER, id::SILVER]);
    expect_decision(&mut g);
    assert_eq!(g.turn.coins, 4, "Silver, Gold and Silver make $1 each, the Copper $1");
    assert_eq!(g.players[1].noc & P_ENVIOUS, 0);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::SILVER, id::GOLD]);
    expect_decision(&mut g);
    assert_eq!(g.turn.coins, 5);
}

#[test]
fn a_player_has_only_one_of_deluded_and_envious() {
    let mut g = new_state(&[id::SKULK], 2);
    g.take_state(1, id::DELUDED, &mut NoEvents);
    g.take_state(1, id::ENVIOUS, &mut NoEvents);
    assert_eq!(g.players[1].noc, P_DELUDED);
    g.take_state(1, id::DELUDED, &mut NoEvents);
    assert_eq!(g.players[1].noc, P_DELUDED);
}

#[test]
fn famine_discards_the_actions_of_the_top_three_and_shuffles_the_rest_into_the_deck() {
    let (g, _) = skulk_hex(id::FAMINE, &[], &[id::VILLAGE, id::GOLD, id::SMITHY, id::SILVER], &[]);
    let v = &g.players[1];
    assert_eq!((v.discard.get(id::VILLAGE), v.discard.get(id::SMITHY)), (1, 1));
    assert!(v.deck_known.is_empty(), "the deck is shuffled: its order is no longer known");
    assert_eq!((v.deck_unknown.get(id::GOLD), v.deck_unknown.get(id::SILVER)), (1, 1));
}

#[test]
fn fear_discards_an_action_or_treasure_from_a_hand_of_five_or_more() {
    let (mut g, _) = skulk_hex(id::FEAR, &[id::ESTATE, id::ESTATE, id::VILLAGE, id::SILVER, id::DUCHY], &junk(), &[]);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Card(id::VILLAGE)]);
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[1].hand.total(), 4);
    // Fewer than 5 cards: nothing. No Action or Treasure: nothing to discard.
    let (g, _) = skulk_hex(id::FEAR, &[id::VILLAGE, id::SILVER], &junk(), &[]);
    assert_eq!(g.players[1].hand.total(), 2);
    let (g, _) = skulk_hex(id::FEAR, &[id::ESTATE; 5], &junk(), &[]);
    assert_eq!(g.players[1].hand.total(), 5);
}

#[test]
fn greed_haunting_plague_and_poverty() {
    let (g, _) = skulk_hex(id::GREED, &[], &[id::GOLD], &[]);
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::COPPER));
    // Haunting: with 4 or more cards in hand, one of them onto the deck.
    let (mut g, _) = skulk_hex(id::HAUNTING, &[id::ESTATE, id::ESTATE, id::SILVER, id::GOLD], &junk(), &[]);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::GOLD));
    let (g, _) = skulk_hex(id::HAUNTING, &[id::ESTATE, id::SILVER, id::GOLD], &junk(), &[]);
    assert_eq!(g.players[1].hand.total(), 3);
    // Plague: a Curse to the hand.
    let (g, _) = skulk_hex(id::PLAGUE, &[id::ESTATE], &junk(), &[]);
    assert_eq!(g.players[1].hand.get(id::CURSE), 1);
    // Poverty: discard down to 3.
    let (mut g, _) = skulk_hex(id::POVERTY, &[id::ESTATE, id::ESTATE, id::SILVER, id::GOLD, id::DUCHY], &junk(), &[]);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    assert_eq!(g.players[1].hand.total(), 3);
}

#[test]
fn locusts_trashes_the_top_card_and_gives_a_curse_or_a_cheaper_card_of_a_shared_type() {
    // Copper and Estate: a Curse.
    for c in [id::COPPER, id::ESTATE] {
        let (g, _) = skulk_hex(id::LOCUSTS, &[], &[c, id::GOLD], &[]);
        assert_eq!(g.trash.get(c), 1);
        assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    }
    // Gold (a $6 Treasure): a cheaper Treasure, the victim's choice.
    let (mut g, _) = skulk_hex(id::LOCUSTS, &[], &[id::GOLD, id::GOLD], &[]);
    assert_eq!(g.trash.get(id::GOLD), 1);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    let cs = choices(&g);
    assert!(cs.contains(&Choice::Card(id::SILVER)) && cs.contains(&Choice::Card(id::COPPER)));
    assert!(!cs.contains(&Choice::Card(id::GOLD)) && !cs.contains(&Choice::Card(id::DUCHY)), "cheaper and sharing a type");
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[1].discard.get(id::SILVER), 1);
    // A Curse: nothing cheaper shares a type.
    let (g, _) = skulk_hex(id::LOCUSTS, &[], &[id::CURSE, id::GOLD], &[]);
    assert_eq!(g.trash.get(id::CURSE), 1);
    assert!(g.players[1].discard.is_empty());
}

#[test]
fn misery_takes_two_vp_then_four() {
    let mut g = new_state(&[id::SKULK], 2);
    set_hand(&mut g, 1, &[id::ESTATE]);
    let base = g.player_vp(1);
    g.take_state(1, id::MISERABLE, &mut NoEvents);
    assert_eq!(g.player_vp(1), base - 2);
    g.take_state(1, id::MISERABLE, &mut NoEvents);
    assert_eq!(g.player_vp(1), base - 4);
    g.take_state(1, id::MISERABLE, &mut NoEvents);
    assert_eq!(g.player_vp(1), base - 4, "a third Misery changes nothing");
    assert_eq!(g.players[1].noc & (P_MISERABLE | P_TWICE_MISERABLE), P_MISERABLE | P_TWICE_MISERABLE);
    let view = PlayerView::new(&g, 0);
    assert!(view.has_state(1, id::TWICE_MISERABLE) && !view.has_state(1, id::MISERABLE) && !view.has_state(0, id::MISERABLE));
}

#[test]
fn war_reveals_until_a_card_costing_three_or_four_and_trashes_it() {
    let (g, _) = skulk_hex(id::WAR, &[], &[id::GOLD, id::ESTATE, id::SILVER, id::COPPER], &[]);
    assert_eq!(g.trash.get(id::SILVER), 1);
    assert_eq!(g.players[1].discard.get(id::GOLD) + g.players[1].discard.get(id::ESTATE), 2);
    assert_eq!(g.players[1].deck_known.len, 1, "the rest of the deck is untouched");
    // None found: the deck is discarded, nothing trashed.
    let (g, _) = skulk_hex(id::WAR, &[], &[id::GOLD, id::COPPER], &[]);
    assert!(g.trash.is_empty());
    assert_eq!(g.players[1].discard.total(), 2);
}

#[test]
fn the_hexes_deck_is_reformed_from_its_discard_pile() {
    let mut g = new_state(&[id::SKULK], 2);
    g.noc.hex_deck = 0;
    g.noc.hex_discard = 0xFFF;
    set_hand(&mut g, 0, &[id::SKULK]);
    play(&mut g, id::SKULK);
    assert_eq!(g.noc.hex_deck.count_ones() + g.noc.hex_discard.count_ones(), 12);
    assert_eq!(g.noc.hex_discard.count_ones(), 1);
}

#[test]
fn cursed_village_draws_to_six_and_hexes_its_owner_when_gained() {
    let mut g = new_state(&[id::CURSED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CURSED_VILLAGE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CURSED_VILLAGE);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].hand.total(), 6);
    // With 6 or more cards in hand it draws nothing.
    let mut g = new_state(&[id::CURSED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::CURSED_VILLAGE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CURSED_VILLAGE);
    assert_eq!(g.players[0].hand.total(), 6);
    // Gained: its owner receives a Hex.
    let mut g = new_state(&[id::CURSED_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    next_hex(&mut g, id::PLAGUE);
    g.turn.phase = Phase::Buy;
    buy(&mut g, id::CURSED_VILLAGE);
    assert_eq!(count(&g, 0, id::CURSE), 1, "a Curse (to the hand, then discarded at Clean-up)");
    assert!(g.players[0].discard.has(id::CURSED_VILLAGE));
}

#[test]
fn leprechaun_gains_a_gold_and_a_wish_with_exactly_seven_cards_in_play_else_a_hex() {
    let mut g = new_state(&[id::LEPRECHAUN], 2);
    set_hand(&mut g, 0, &[id::LEPRECHAUN]);
    set_in_play(&mut g, 0, &[id::COPPER; 5]);
    next_hex(&mut g, id::PLAGUE);
    play(&mut g, id::LEPRECHAUN);
    assert!(g.players[0].discard.has(id::GOLD));
    assert_eq!(g.players[0].discard.get(id::WISH), 0);
    assert_eq!(g.players[0].hand.get(id::CURSE), 1, "6 cards in play: a Hex");
    let mut g = new_state(&[id::LEPRECHAUN], 2);
    set_hand(&mut g, 0, &[id::LEPRECHAUN]);
    set_in_play(&mut g, 0, &[id::COPPER; 6]);
    next_hex(&mut g, id::PLAGUE);
    play(&mut g, id::LEPRECHAUN);
    assert_eq!(g.players[0].discard.get(id::WISH), 1);
    assert_eq!(g.noc.piles[1], 11);
    assert_eq!(g.players[0].hand.get(id::CURSE), 0);
}

#[test]
fn tormentor_gains_an_imp_with_no_other_cards_in_play_else_hexes() {
    let mut g = new_state(&[id::TORMENTOR], 2);
    set_hand(&mut g, 0, &[id::TORMENTOR]);
    next_hex(&mut g, id::GREED);
    play(&mut g, id::TORMENTOR);
    assert_eq!(g.turn.coins, 2);
    assert!(g.players[0].discard.has(id::IMP));
    assert!(g.players[1].deck_known.is_empty());
    let mut g = new_state(&[id::TORMENTOR], 2);
    set_hand(&mut g, 0, &[id::TORMENTOR]);
    set_in_play(&mut g, 0, &[id::COPPER]);
    next_hex(&mut g, id::GREED);
    play(&mut g, id::TORMENTOR);
    assert!(!g.players[0].discard.has(id::IMP));
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::COPPER));
}

#[test]
fn guardian_protects_its_owner_until_their_next_turn() {
    let mut g = new_state(&[id::GUARDIAN, id::SKULK], 2);
    set_hand(&mut g, 0, &[id::GUARDIAN]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::SKULK]);
    set_deck_known(&mut g, 1, &junk());
    next_hex(&mut g, id::GREED);
    at_night(&mut g);
    night(&mut g, id::GUARDIAN);
    assert_eq!(g.turn.player, 1);
    assert!(g.players[0].noc & P_GUARDED != 0);
    // Player 1 attacks with Skulk: no effect on player 0, and no Hex is turned over.
    play(&mut g, id::SKULK);
    assert!(g.players[0].deck_known.peek_top() != Some(id::COPPER));
    assert_eq!(g.noc.hex_deck, bit(id::GREED, cards::FIRST_HEX));
    // Back to player 0: the protection and the +$1 begin and end with the turn.
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.players[0].noc & P_GUARDED, 0);
    assert_eq!(g.turn.coins, 1);
}

// ===========================================================================
// The kingdom cards
// ===========================================================================

#[test]
fn faithful_hound_draws_two_and_can_be_set_aside_when_discarded_until_the_end_of_the_turn() {
    let mut g = new_state(&[id::FAITHFUL_HOUND], 2);
    set_hand(&mut g, 0, &[id::FAITHFUL_HOUND]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::FAITHFUL_HOUND);
    assert_eq!(g.players[0].hand.total(), 2);
    // Discarded by Cellar: set aside, back in the hand after the next hand is drawn.
    let mut g = new_state(&[id::FAITHFUL_HOUND, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::FAITHFUL_HOUND, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CELLAR);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::FAITHFUL_HOUND);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::SetAside }) && d.subject == id::FAITHFUL_HOUND, "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].hounds, 1);
    assert_eq!(count(&g, 0, id::FAITHFUL_HOUND), 1, "still owned");
    end_turn(&mut g);
    assert_eq!(g.players[0].hand.get(id::FAITHFUL_HOUND), 1);
    assert_eq!(g.players[0].hand.total(), 5 + 1, "5 cards and the Hound");
    // Declined: it stays in the discard pile. Not at Clean-up: the Hound in hand is discarded without an offer.
    let mut g = new_state(&[id::FAITHFUL_HOUND], 2);
    set_hand(&mut g, 0, &[id::FAITHFUL_HOUND]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(g.players[0].hounds, 0);
    assert!(g.players[0].discard.has(id::FAITHFUL_HOUND));
}

#[test]
fn a_hound_discarded_to_an_attack_returns_at_the_end_of_that_turn() {
    let mut g = new_state(&[id::FAITHFUL_HOUND, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::FAITHFUL_HOUND, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::MILITIA);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    pick(&mut g, id::FAITHFUL_HOUND);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::SetAside }), "{d:?}");
    yes(&mut g);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    assert_eq!(g.players[1].hand.total(), 3);
    end_turn(&mut g);
    assert_eq!(g.players[1].hand.get(id::FAITHFUL_HOUND), 1);
}

#[test]
fn monastery_trashes_one_card_per_card_gained_this_turn() {
    let mut g = new_state(&[id::MONASTERY], 2);
    set_hand(&mut g, 0, &[id::MONASTERY, id::GOLD, id::GOLD, id::CURSE, id::ESTATE, id::COPPER]);
    set_in_play(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    // $8 from the Golds and Coppers; two buys would be needed: gain two cards in the turn.
    g.turn.buys = 3;
    buy(&mut g, id::SILVER);
    buy(&mut g, id::SILVER);
    pass(&mut g);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::PlayNight);
    assert_eq!(g.turn.noc.gains, 2);
    night(&mut g, id::MONASTERY);
    // Two trashes in all: from the hand first, then Coppers in play with what is left.
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    assert_eq!(g.trash.get(id::CURSE) + g.trash.get(id::ESTATE), 2);
    // A Copper in play instead of a card from the hand.
    let mut g = new_state(&[id::MONASTERY], 2);
    set_hand(&mut g, 0, &[id::MONASTERY, id::GOLD]);
    set_in_play(&mut g, 0, &[id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::COPPER);
    night(&mut g, id::MONASTERY);
    // (The hand is empty: no card to trash from it.)
    pick(&mut g, id::COPPER);
    assert_eq!(g.trash.get(id::COPPER), 1);
    // Nothing gained: nothing to trash.
    let mut g = new_state(&[id::MONASTERY], 2);
    set_hand(&mut g, 0, &[id::MONASTERY, id::CURSE]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::MONASTERY);
    assert!(g.trash.is_empty());
}

#[test]
fn changeling_trashes_itself_and_gains_a_copy_of_a_card_in_play() {
    let mut g = new_state(&[id::CHANGELING, id::FOOL], 2);
    set_hand(&mut g, 0, &[id::CHANGELING]);
    set_in_play(&mut g, 0, &[id::GOLD, id::FOOL]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::CHANGELING);
    assert_eq!(g.trash.get(id::CHANGELING), 1);
    assert_eq!(choices(&g), vec![Choice::Card(id::GOLD), Choice::Card(id::FOOL)]);
    pick(&mut g, id::FOOL);
    assert!(g.players[0].discard.has(id::FOOL));
}

#[test]
fn a_card_costing_three_or_more_may_be_exchanged_for_a_changeling_when_gained() {
    let mut g = new_state(&[id::CHANGELING, id::SKULK], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::SKULK);
    // The exchange is asked first; the rest of the gain still happens (Skulk's Gold).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::SKULK && d.source == Some(id::CHANGELING), "{d:?}");
    yes(&mut g);
    assert_eq!(g.supply.get(id::SKULK), 10, "back in its pile");
    assert!(g.players[0].discard.has(id::CHANGELING) && !g.players[0].discard.has(id::SKULK));
    assert_eq!(g.supply.get(id::CHANGELING), 9);
    assert!(g.players[0].discard.has(id::GOLD), "Skulk's gain still happened");
    // Cards costing less than $3 are not offered, and it is optional.
    let mut g = new_state(&[id::CHANGELING], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    g.turn.buys = 3;
    buy(&mut g, id::ESTATE);
    buy(&mut g, id::SILVER);
    no(&mut g);
    assert!(g.players[0].discard.has(id::SILVER));
    // The exchange is not a gain.
    let mut g = new_state(&[id::CHANGELING, id::MONASTERY], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::SILVER);
    yes(&mut g);
    assert!(g.players[0].discard.has(id::CHANGELING));
}

#[test]
fn night_watchman_looks_at_the_top_five() {
    let mut g = new_state(&[id::NIGHT_WATCHMAN], 2);
    set_hand(&mut g, 0, &[id::NIGHT_WATCHMAN]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::ESTATE, id::GOLD, id::CURSE, id::SILVER, id::DUCHY]);
    at_night(&mut g);
    night(&mut g, id::NIGHT_WATCHMAN);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    pick(&mut g, id::SILVER); // put back first: ends up below
    assert_eq!(g.players[0].discard.get(id::ESTATE) + g.players[0].discard.get(id::CURSE), 2);
}

#[test]
fn secret_cave_discards_three_for_three_coins_next_turn() {
    let mut g = new_state(&[id::SECRET_CAVE], 2);
    set_hand(&mut g, 0, &[id::SECRET_CAVE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SECRET_CAVE);
    assert_eq!(g.players[0].hand.total(), 4, "+1 Card, +1 Action");
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    assert_eq!(g.players[0].pending_durations_len, 1);
    end_turn(&mut g);
    assert!(g.players[0].in_play.has(id::SECRET_CAVE));
    set_deck_known(&mut g, 1, &junk());
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.coins, 3);
    // With fewer than three discarded: no bonus and it leaves play.
    let mut g = new_state(&[id::SECRET_CAVE], 2);
    set_hand(&mut g, 0, &[id::SECRET_CAVE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::SECRET_CAVE);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::DUCHY);
    assert_eq!(g.players[0].pending_durations_len, 0);
}

#[test]
fn conclave_plays_an_action_you_do_not_have_in_play_for_an_extra_action() {
    let mut g = new_state(&[id::CONCLAVE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CONCLAVE, id::SMITHY, id::CONCLAVE]);
    set_in_play(&mut g, 0, &[]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CONCLAVE);
    assert_eq!(g.turn.coins, 2);
    // The second Conclave is a copy of one in play: only Smithy can be played.
    assert_eq!(choices(&g), vec![Choice::Card(id::SMITHY), Choice::Pass]);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.turn.actions, 1, "the Action for Conclave and none spent on Smithy");
    assert_eq!(g.players[0].hand.total(), 4);
}

#[test]
fn devils_workshop_depends_on_the_cards_gained_this_turn() {
    for (gains, expect) in [(0, id::GOLD), (1, id::SILVER), (2, id::IMP), (3, id::IMP)] {
        let mut g = new_state(&[id::DEVILS_WORKSHOP], 2);
        set_hand(&mut g, 0, &[id::DEVILS_WORKSHOP, id::GOLD, id::GOLD]);
        set_deck_known(&mut g, 0, &junk());
        g.turn.phase = Phase::Buy;
        expect_decision(&mut g);
        g.turn.buys = 5;
        for _ in 0..gains {
            buy(&mut g, id::COPPER);
        }
        pass(&mut g);
        night(&mut g, id::DEVILS_WORKSHOP);
        if gains == 1 {
            pick(&mut g, id::SILVER);
        }
        assert!(g.players[0].discard.has(expect), "{gains} gains: expected a {}", cards::name(expect));
    }
    // An Imp is a Spirit from its own pile.
    let mut g = new_state(&[id::DEVILS_WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::DEVILS_WORKSHOP]);
    at_night(&mut g);
    g.turn.noc.gains = 2;
    night(&mut g, id::DEVILS_WORKSHOP);
    assert_eq!(g.noc.piles[3], 12);
}

#[test]
fn exorcist_trashes_a_card_and_gains_a_cheaper_spirit() {
    let mut g = new_state(&[id::EXORCIST], 2);
    set_hand(&mut g, 0, &[id::EXORCIST, id::DUCHY, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::EXORCIST);
    // (The Duchy is the only card in hand: trashed without asking.)
    assert_eq!(g.trash.get(id::DUCHY), 1);
    assert_eq!(choices(&g), vec![Choice::Card(id::WILL_O_WISP), Choice::Card(id::IMP), Choice::Card(id::GHOST)]);
    pick(&mut g, id::GHOST);
    assert!(g.players[0].discard.has(id::GHOST));
    assert_eq!(g.noc.piles[4], 5);
    // A Silver ($3): Will-o'-Wisp or Imp. A Copper: nothing cheaper.
    let mut g = new_state(&[id::EXORCIST], 2);
    set_hand(&mut g, 0, &[id::EXORCIST, id::VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::EXORCIST);
    assert_eq!(choices(&g), vec![Choice::Card(id::WILL_O_WISP), Choice::Card(id::IMP)]);
    let mut g = new_state(&[id::EXORCIST], 2);
    set_hand(&mut g, 0, &[id::EXORCIST, id::CURSE]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::EXORCIST);
    assert_eq!(g.trash.get(id::CURSE), 1);
    assert!(!g.players[0].all_cards().has(id::WILL_O_WISP) && !g.players[0].all_cards().has(id::IMP), "nothing costs less than $0");
}

#[test]
fn necromancer_plays_an_action_from_the_trash_once_per_turn_each() {
    let mut g = new_state(&[id::NECROMANCER], 2);
    set_hand(&mut g, 0, &[id::NECROMANCER, id::NECROMANCER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::NECROMANCER);
    assert_eq!(choices(&g).len(), 3, "the three Zombies");
    pick(&mut g, id::ZOMBIE_SPY);
    assert_eq!(g.trash.get(id::ZOMBIE_SPY), 1, "the card stays in the trash");
    assert_eq!(g.players[0].hand.total(), 2, "Zombie Spy: +1 Card");
    // Zombie Spy looks at the top card: kept.
    pass(&mut g);
    // The second Necromancer cannot use the same Zombie again this turn.
    play(&mut g, id::NECROMANCER);
    assert!(!choices(&g).contains(&Choice::Card(id::ZOMBIE_SPY)));
    // A Duration in the trash cannot be played; an Action that is not a Duration can.
    let mut g = new_state(&[id::NECROMANCER, id::HAVEN], 2);
    g.trash = counts_of(&[id::HAVEN, id::SMITHY]);
    set_hand(&mut g, 0, &[id::NECROMANCER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::NECROMANCER);
    // Smithy was the only one to play (a single choice is applied at once).
    assert_eq!(g.turn.played.get(id::SMITHY), 1);
    assert_eq!(g.turn.played.get(id::HAVEN), 0);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn shepherd_discards_victory_cards_for_two_cards_each() {
    let mut g = new_state(&[id::SHEPHERD], 2);
    set_hand(&mut g, 0, &[id::SHEPHERD, id::ESTATE, id::DUCHY, id::GOLD]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::COPPER, id::SILVER, id::CURSE]);
    play(&mut g, id::SHEPHERD);
    assert_eq!(g.turn.actions, 1);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::DUCHY);
    assert_eq!(g.players[0].discard.get(id::ESTATE) + g.players[0].discard.get(id::DUCHY), 2);
    assert_eq!(count(&g, 0, id::COPPER) + count(&g, 0, id::SILVER) + count(&g, 0, id::CURSE), 5, "all owned: four cards drawn, one left in the deck");
}

#[test]
fn cobbler_gains_a_card_costing_up_to_four_to_the_hand_next_turn() {
    let mut g = new_state(&[id::COBBLER], 2);
    set_hand(&mut g, 0, &[id::COBBLER]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    at_night(&mut g);
    night(&mut g, id::COBBLER);
    set_hand(&mut g, 1, &[]);
    end_turn(&mut g);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, dest: Dest::Hand, .. }), "{d:?}");
    pick(&mut g, id::SILVER);
    assert!(g.players[0].hand.has(id::SILVER) || g.players[0].in_play.has(id::SILVER), "gained to the hand (then played as a Treasure)");
}

#[test]
fn crypt_sets_treasures_aside_and_returns_one_each_turn() {
    let mut g = new_state(&[id::CRYPT], 2);
    set_hand(&mut g, 0, &[id::CRYPT, id::GOLD, id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    pass(&mut g);
    night(&mut g, id::CRYPT);
    // Any number of the Treasures in play: the Gold and the Silver.
    pick(&mut g, id::SILVER);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].archive.total(), 2);
    assert_eq!(count(&g, 0, id::GOLD), 1, "set-aside cards are still owned");
    set_hand(&mut g, 1, &[]);
    end_turn(&mut g);
    assert!(g.players[0].in_play.has(id::CRYPT));
    assert_eq!(g.turn.player, 0);
    // One of them (the player's choice) comes to the hand at the start of the turn.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::ToHand, .. }), "{d:?}");
    pick(&mut g, id::GOLD);
    assert!(g.players[0].hand.has(id::GOLD) || g.players[0].in_play.has(id::GOLD));
    assert_eq!(g.players[0].archive.total(), 1);
    assert_eq!(g.players[0].pending_durations_len, 1, "it stays for the other");
}

#[test]
fn den_of_sin_draws_two_cards_next_turn() {
    let mut g = new_state(&[id::DEN_OF_SIN], 2);
    set_hand(&mut g, 0, &[id::DEN_OF_SIN]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    at_night(&mut g);
    night(&mut g, id::DEN_OF_SIN);
    set_hand(&mut g, 1, &[]);
    end_turn(&mut g);
    assert_eq!(g.players[0].hand.total(), 7);
}

#[test]
fn pooka_trashes_a_treasure_other_than_cursed_gold_for_four_cards() {
    let mut g = new_state(&[id::POOKA], 2);
    set_hand(&mut g, 0, &[id::POOKA, id::COPPER, id::CURSED_GOLD, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::POOKA);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass], "not the Cursed Gold");
    pick(&mut g, id::COPPER);
    assert_eq!(g.trash.get(id::COPPER), 1);
    assert_eq!(g.players[0].hand.total(), 2 + 4);
}

#[test]
fn tragic_hero_draws_three_and_with_eight_cards_trashes_itself_for_a_treasure() {
    let mut g = new_state(&[id::TRAGIC_HERO], 2);
    set_hand(&mut g, 0, &[id::TRAGIC_HERO, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRAGIC_HERO);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.trash.get(id::TRAGIC_HERO), 1);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { .. }));
    assert!(choices(&g).contains(&Choice::Card(id::GOLD)) && !choices(&g).contains(&Choice::Card(id::DUCHY)));
    pick(&mut g, id::GOLD);
    assert!(g.players[0].discard.has(id::GOLD));
    // Fewer than 8 cards in hand after drawing: nothing.
    let mut g = new_state(&[id::TRAGIC_HERO], 2);
    set_hand(&mut g, 0, &[id::TRAGIC_HERO, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRAGIC_HERO);
    assert!(g.trash.is_empty());
}

#[test]
fn vampire_hexes_gains_a_card_up_to_five_and_turns_into_a_bat() {
    let mut g = new_state(&[id::VAMPIRE, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::VAMPIRE]);
    set_deck_known(&mut g, 0, &junk());
    next_hex(&mut g, id::GREED);
    at_night(&mut g);
    night(&mut g, id::VAMPIRE);
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::COPPER), "the Hex first");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }));
    assert!(!choices(&g).contains(&Choice::Card(id::VAMPIRE)), "not a Vampire");
    pick(&mut g, id::MILITIA);
    assert!(g.players[0].discard.has(id::MILITIA));
    assert!(g.players[0].discard.has(id::BAT) && !g.players[0].in_play.has(id::VAMPIRE));
    assert_eq!(g.noc.piles[2], 9);
    assert_eq!(g.supply.get(id::VAMPIRE), 11, "the Vampire (not taken from the pile in this test) went back to it");
}

#[test]
fn bat_trashes_up_to_two_cards_and_if_it_did_turns_into_a_vampire() {
    let mut g = new_state(&[id::VAMPIRE], 2);
    set_hand(&mut g, 0, &[id::BAT, id::CURSE, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::BAT);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    assert_eq!(g.trash.total(), 2);
    assert!(g.players[0].discard.has(id::VAMPIRE) || g.players[0].hand.has(id::VAMPIRE) || count(&g, 0, id::VAMPIRE) == 1);
    assert_eq!(count(&g, 0, id::BAT), 0, "the Bat went back to its pile");
    assert_eq!(g.noc.piles[2], 11);
    // Trashing nothing: the Bat stays a Bat.
    let mut g = new_state(&[id::VAMPIRE], 2);
    set_hand(&mut g, 0, &[id::BAT, id::DUCHY]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::BAT);
    pass(&mut g);
    assert_eq!(count(&g, 0, id::BAT), 1);
    assert_eq!(count(&g, 0, id::VAMPIRE), 0);
}

#[test]
fn raider_attacks_players_with_five_cards_and_pays_three_coins_next_turn() {
    let mut g = new_state(&[id::RAIDER], 2);
    set_hand(&mut g, 0, &[id::RAIDER]);
    set_in_play(&mut g, 0, &[id::COPPER, id::SILVER]);
    set_hand(&mut g, 1, &[id::COPPER, id::GOLD, id::ESTATE, id::ESTATE, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    at_night(&mut g);
    night(&mut g, id::RAIDER);
    // The victim discards a copy of a card in play: Copper, Silver or Raider.
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Card(id::SILVER)]);
    pick(&mut g, id::SILVER);
    assert!(g.players[1].discard.has(id::SILVER));
    // The next turn: +$3.
    set_hand(&mut g, 1, &[]);
    end_turn(&mut g);
    assert_eq!(g.turn.coins, 3);
    // A victim with fewer than 5 cards is not affected.
    let mut g = new_state(&[id::RAIDER], 2);
    set_hand(&mut g, 0, &[id::RAIDER]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    at_night(&mut g);
    night(&mut g, id::RAIDER);
    assert!(g.players[1].discard.is_empty(), "no one discarded anything");
}

#[test]
fn cemetery_trashes_up_to_four_cards_when_gained_and_comes_with_haunted_mirror() {
    let mut g = new_state(&[id::CEMETERY], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::COPPER, id::CURSE, id::ESTATE, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::CEMETERY);
    // Hand left: Curse, Estate, Estate, Duchy (the Treasures were played).
    pick_while(&mut g, Zone::Hand, Act::Trash, id::ESTATE);
    pick_while(&mut g, Zone::Hand, Act::Trash, id::DUCHY);
    pick_while(&mut g, Zone::Hand, Act::Trash, id::CURSE);
    assert!(g.trash.total() >= 1 && g.trash.total() <= 4);
    assert_eq!(cards::def(id::CEMETERY).vp, 2);
}

#[test]
fn haunted_mirror_gains_a_ghost_when_trashed_for_an_action_discard() {
    let mut g = new_state(&[id::CEMETERY, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::HAUNTED_MIRROR, id::VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::HAUNTED_MIRROR);
    assert_eq!(g.trash.get(id::HAUNTED_MIRROR), 1);
    // The Mirror's trigger: may discard an Action card to gain a Ghost.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, max: 1, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Pass]);
    pick(&mut g, id::VILLAGE);
    assert!(g.players[0].discard.has(id::GHOST));
    assert_eq!(g.noc.piles[4], 5);
}

// ===========================================================================
// Heirlooms and the cards outside the Supply
// ===========================================================================

#[test]
fn the_heirlooms() {
    // Pouch: $1 and +1 Buy, choice-free (played with the other Treasures).
    let mut g = new_state(&[id::TRACKER], 2);
    set_hand(&mut g, 0, &[id::POUCH, id::COPPER]);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    assert_eq!((g.turn.coins, g.turn.buys), (2, 2));
    // Goat: $1 and may trash a card from the hand.
    let mut g = new_state(&[id::PIXIE], 2);
    set_hand(&mut g, 0, &[id::GOAT, id::CURSE, id::ESTATE]);
    g.turn.phase = Phase::Buy;
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::GOAT);
    pick(&mut g, id::CURSE);
    assert_eq!((g.turn.coins, g.trash.get(id::CURSE)), (1, 1));
    // Lucky Coin: $1 and a Silver.
    let mut g = new_state(&[id::FOOL], 2);
    set_hand(&mut g, 0, &[id::LUCKY_COIN]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::LUCKY_COIN);
    assert_eq!(g.turn.coins, 1);
    assert!(g.players[0].discard.has(id::SILVER));
    // Cursed Gold: $3 and a Curse; it can be left in the hand.
    let mut g = new_state(&[id::POOKA], 2);
    set_hand(&mut g, 0, &[id::CURSED_GOLD]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::CURSED_GOLD);
    assert_eq!(g.turn.coins, 3);
    assert!(g.players[0].discard.has(id::CURSE));
    let mut g = new_state(&[id::POOKA], 2);
    set_hand(&mut g, 0, &[id::CURSED_GOLD]);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(g.turn.coins, 0, "passing on the Treasures does not play it");
    // Pasture: $1 and 1 VP per Estate.
    let mut g = new_state(&[id::SHEPHERD], 2);
    set_hand(&mut g, 0, &[id::PASTURE, id::ESTATE, id::ESTATE, id::ESTATE]);
    assert_eq!(g.player_vp(0), 3 + 3);
    assert_eq!(g.players[0].vp(), 6);
}

#[test]
fn magic_lamp_trashes_itself_for_three_wishes_with_six_single_cards_in_play() {
    let mut g = new_state(&[id::SECRET_CAVE], 2);
    set_hand(&mut g, 0, &[id::MAGIC_LAMP]);
    set_in_play(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::VILLAGE, id::MARKET]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::MAGIC_LAMP);
    assert_eq!(g.trash.get(id::MAGIC_LAMP), 1);
    assert_eq!(g.players[0].discard.get(id::WISH), 3);
    assert_eq!(g.turn.coins, 1, "it still gave $1");
    // Two Coppers count as none (not exactly one copy).
    let mut g = new_state(&[id::SECRET_CAVE], 2);
    set_hand(&mut g, 0, &[id::MAGIC_LAMP]);
    set_in_play(&mut g, 0, &[id::COPPER, id::COPPER, id::SILVER, id::GOLD, id::VILLAGE, id::MARKET]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::MAGIC_LAMP);
    assert_eq!(g.trash.get(id::MAGIC_LAMP), 0);
}

#[test]
fn will_o_wisp_takes_the_revealed_card_only_if_it_costs_two_or_less() {
    let mut g = new_state(&[id::BARD], 2);
    set_hand(&mut g, 0, &[id::WILL_O_WISP]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER, id::CURSE, id::SILVER]);
    play(&mut g, id::WILL_O_WISP);
    assert_eq!(g.turn.actions, 1);
    // +1 Card (Estate, $2) and the Copper revealed ($0): both in the hand.
    assert_eq!(g.players[0].hand.get(id::ESTATE) + g.players[0].hand.get(id::COPPER) + g.players[0].in_play.get(id::COPPER), 2);
    let mut g = new_state(&[id::BARD], 2);
    set_hand(&mut g, 0, &[id::WILL_O_WISP]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::SILVER, id::CURSE]);
    play(&mut g, id::WILL_O_WISP);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER), "left on the deck");
}

#[test]
fn wish_returns_to_its_pile_and_gains_a_card_costing_up_to_six_to_the_hand() {
    let mut g = new_state(&[id::LEPRECHAUN], 2);
    set_hand(&mut g, 0, &[id::WISH]);
    set_in_play(&mut g, 0, &[]);
    g.noc.piles[1] = 11;
    play(&mut g, id::WISH);
    assert_eq!(g.noc.piles[1], 12);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { max_cost: 6, dest: Dest::Hand, .. }));
    pick(&mut g, id::GOLD);
    assert!(g.players[0].hand.has(id::GOLD) || g.players[0].in_play.has(id::GOLD));
}

#[test]
fn imp_draws_two_and_may_play_an_action_that_is_not_in_play() {
    let mut g = new_state(&[id::DEVILS_WORKSHOP, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::IMP, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::IMP);
    assert_eq!(g.players[0].hand.total(), 3);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 5, "Smithy played free");
    assert_eq!(g.turn.actions, 0, "the Imp used the Action");
}

#[test]
fn ghost_sets_an_action_aside_and_plays_it_twice_next_turn() {
    let mut g = new_state(&[id::EXORCIST, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GHOST]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER, id::SMITHY, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 1, &junk());
    at_night(&mut g);
    night(&mut g, id::GHOST);
    assert_eq!(g.players[0].set_aside.get(id::SMITHY), 1);
    assert_eq!(g.players[0].pending_durations_len, 1);
    // Next turn: Smithy is played twice (+6 cards) at the start.
    set_hand(&mut g, 1, &[]);
    set_deck_known(&mut g, 0, &junk());
    end_turn(&mut g);
    assert_eq!(g.turn.player, 0);
    assert_eq!(g.turn.played.get(id::SMITHY), 2, "played twice at the start of the turn");
    assert!(g.players[0].in_play.has(id::SMITHY));
    // No Action in the deck: everything is discarded and the Ghost leaves play.
    let mut g = new_state(&[id::EXORCIST], 2);
    set_hand(&mut g, 0, &[id::GHOST]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER]);
    at_night(&mut g);
    night(&mut g, id::GHOST);
    assert_eq!(g.players[0].pending_durations_len, 0);
}

#[test]
fn the_zombies() {
    // Apprentice: may trash an Action from the hand for +3 Cards and +1 Action.
    let mut g = new_state(&[id::NECROMANCER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ZOMBIE_APPRENTICE, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::ZOMBIE_APPRENTICE);
    pick(&mut g, id::SMITHY);
    assert_eq!((g.trash.get(id::SMITHY), g.turn.actions, g.players[0].hand.total()), (1, 1, 3));
    // Mason: trash the top card of the deck, gain a card costing up to $1 more.
    let mut g = new_state(&[id::NECROMANCER], 2);
    set_hand(&mut g, 0, &[id::ZOMBIE_MASON]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::ESTATE]);
    play(&mut g, id::ZOMBIE_MASON);
    assert_eq!(g.trash.get(id::SILVER), 1);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { max_cost: 4, .. }));
    assert!(choices(&g).contains(&Choice::Pass), "optional");
    assert!(!choices(&g).contains(&Choice::Card(id::DUCHY)), "$5 is two more than the Silver");
    pick(&mut g, id::SILVER);
    assert!(g.players[0].discard.has(id::SILVER));
    // Spy: +1 Card +1 Action, then look at the top card.
    let mut g = new_state(&[id::NECROMANCER], 2);
    set_hand(&mut g, 0, &[id::ZOMBIE_SPY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::CURSE, id::SILVER]);
    play(&mut g, id::ZOMBIE_SPY);
    assert_eq!(g.turn.actions, 1);
    pick(&mut g, id::CURSE);
    assert_eq!(g.players[0].discard.get(id::CURSE), 1);
}

// ===========================================================================
// Text format and views
// ===========================================================================

#[test]
fn the_text_format_round_trips_the_nocturne_state() {
    let mut g = new_game(&[id::DRUID, id::SKULK, id::VAMPIRE, id::FAITHFUL_HOUND, id::PIXIE], 2, 7);
    g.deal_opening_hands(&mut NoEvents);
    g.noc.boon_discard = bit(id::THE_SEAS_GIFT, cards::FIRST_BOON) | bit(id::THE_SKYS_GIFT, cards::FIRST_BOON);
    g.noc.boon_deck &= !g.noc.boon_discard;
    g.noc.boon_held = 0;
    g.noc.hex_discard = bit(id::WAR, cards::FIRST_HEX);
    g.noc.hex_deck &= !g.noc.hex_discard;
    g.noc.lost = 2;
    g.noc.piles = [11, 0, 9, 0, 0];
    g.players[0].noc = P_DELUDED | P_MISERABLE | P_TWICE_MISERABLE;
    g.players[1].noc = P_ENVIOUS | P_GUARDED;
    g.players[1].hounds = 1;
    g.players[0].boons_later = bit(id::THE_MOONS_GIFT, cards::FIRST_BOON);
    g.turn.phase = Phase::Night;
    let text = format_state(&g);
    let h = parse_state(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(h.turn.phase, Phase::Night);
    assert_eq!((h.noc.boon_deck, h.noc.boon_discard, h.noc.boon_druid), (g.noc.boon_deck, g.noc.boon_discard, g.noc.boon_druid));
    assert_eq!((h.noc.hex_deck, h.noc.hex_discard, h.noc.lost, h.noc.piles), (g.noc.hex_deck, g.noc.hex_discard, 2, [11, 0, 9, 0, 0]));
    for p in 0..2 {
        assert_eq!((h.players[p].noc, h.players[p].hounds, h.players[p].boons_later), (g.players[p].noc, g.players[p].hounds, g.players[p].boons_later));
    }
    let text2 = format_state(&h);
    assert_eq!(parse_state(&text2).unwrap().noc, h.noc);
}

#[test]
fn player_views_show_the_decks_states_and_piles() {
    let mut g = new_game(&[id::DRUID, id::SKULK, id::EXORCIST], 2, 3);
    g.players[1].noc = P_DELUDED;
    g.noc.lost = 1;
    let v = PlayerView::new(&g, 0);
    assert!(v.has_boons() && v.has_hexes() && v.night_possible());
    assert!(v.has_state(1, id::DELUDED) && !v.has_state(0, id::DELUDED));
    assert_eq!(v.lost_in_the_woods(), Some(0));
    assert_eq!(v.supply(id::WILL_O_WISP), 12);
    assert_eq!(v.supply(id::GHOST), 6);
    assert_eq!(v.druid_boon_mask().count_ones(), 3);
    assert_eq!(v.boon_deck_mask().count_ones(), 9);
    assert_eq!(v.hex_deck_mask(), 0xFFF);
}

#[test]
fn search_worlds_reach_the_night_phase_with_chance_outcomes() {
    // A determinized world has chance mode on: a Boon draw is a chance step there, a Night card is a decision.
    let mut g = new_state(&[id::BARD, id::MONASTERY], 2);
    set_hand(&mut g, 0, &[id::BARD, id::MONASTERY]);
    set_deck_known(&mut g, 0, &junk());
    let view = PlayerView::new(&g, 0);
    let mut w = view.determinize(&mut Rng::new(5));
    assert!(w.chance_mode);
    let st = play(&mut w, id::BARD);
    assert!(matches!(st, Step::Chance { player } if player == BOON_CHANCE));
    let outcomes = w.chance_outcomes(BOON_CHANCE);
    assert_eq!(outcomes.total(), 12);
    let _ = HEX_CHANCE;
}

// ===========================================================================
// The stop-moving rule (docs/stop-moving-rule.md)
// ===========================================================================

#[test]
fn a_throne_roomed_pixie_is_trashed_once_even_with_another_pixie_in_play() {
    let mut g = new_state(&[id::THRONE_ROOM, id::PIXIE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::PIXIE]);
    set_in_play(&mut g, 0, &[id::PIXIE]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::PIXIE);
    yes(&mut g); // the 1st play: trash the Pixie to receive the Boon twice
    // The 2nd play: its Pixie is gone; the other one in play is not it, so no offer to trash it.
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })), "{:?}", g.pending_decision());
    assert_eq!(g.trash.get(id::PIXIE), 1);
    assert_eq!(g.players[0].in_play.get(id::PIXIE), 1);
    assert_eq!(g.turn.led.n, 0);
}

#[test]
fn a_pixie_played_by_overlord_is_not_in_play_to_be_trashed() {
    let mut g = new_state(&[id::OVERLORD, id::PIXIE], 2);
    set_hand(&mut g, 0, &[id::OVERLORD]);
    set_in_play(&mut g, 0, &[id::PIXIE]);
    set_deck_known(&mut g, 0, &junk());
    next_boon(&mut g, id::THE_MOUNTAINS_GIFT);
    play(&mut g, id::OVERLORD);
    if matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::Select { act: Act::Play, .. })) {
        pick(&mut g, id::PIXIE);
    }
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })), "{:?}", g.pending_decision());
    assert_eq!(g.players[0].in_play.get(id::PIXIE), 1, "the Pixie in play stays");
    assert_eq!(g.trash.get(id::PIXIE), 0);
}
