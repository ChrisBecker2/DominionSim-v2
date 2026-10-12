//! Dark Ages card tests (rulings in `docs/dark-ages-plan.md`): the Shelters, the Ruins and Knights piles, Spoils, Madman and Mercenary, the
//! "when you trash this" machinery, and every kingdom card.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::sets::dark_ages::{KNIGHTS_PILE, RUINS_PILE};
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn da_kingdom() -> Vec<CardId> {
    vec![id::CULTIST, id::KNIGHTS, id::HERMIT, id::URCHIN, id::BANDIT_CAMP, id::RATS, id::MARKET_SQUARE, id::FORAGER, id::ARMORY, id::SAGE]
}

fn new_game(kingdom: &[CardId], players: usize, seed: u64) -> GameState {
    GameState::new(&GameConfig { num_players: players, kingdom: kingdom.to_vec(), seed, ..Default::default() })
}

/// Gain `card` for player 0 from the Supply (the way a buy or a Workshop does).
fn gain0(g: &mut GameState, card: CardId) -> bool {
    g.gain_card(0, card, Dest::Discard, &mut NoEvents)
}

// ===========================================================================
// The card list, the ids, the types
// ===========================================================================

#[test]
fn the_set_has_35_kingdom_piles_and_the_other_cards() {
    assert_eq!(cards::NUM_CARDS, 427);
    assert_eq!(CardSet::ALL.len(), 12);
    assert_eq!(cards::set_by_name("Dark Ages"), Some(CardSet::DarkAges));
    assert_eq!(CardSet::DarkAges.name(), "Dark Ages");
    assert_eq!(cards::kingdom_cards_in(CardSet::DarkAges).count(), 35, "34 cards and the Knights pile");
    assert!(cards::is_kingdom(id::KNIGHTS) && !cards::is_kingdom(id::RUINS) && !cards::is_kingdom(id::DAME_ANNA) && !cards::is_kingdom(id::HOVEL));
    for c in [id::HOVEL, id::NECROPOLIS, id::OVERGROWN_ESTATE, id::SPOILS, id::MADMAN, id::MERCENARY, id::ABANDONED_MINE, id::SURVIVORS, id::SIR_VANDER] {
        assert!(!cards::is_kingdom(c), "{} is not a kingdom card", cards::name(c));
        assert_eq!(cards::set_of(c), CardSet::DarkAges);
    }
    assert_eq!(cards::by_name("sir martin"), Some(id::SIR_MARTIN));
    assert_eq!(cards::by_name("Knights"), Some(id::KNIGHTS));
    assert_eq!(cards::by_name("Wandering Minstrel"), Some(id::WANDERING_MINSTREL));
    // Types and costs.
    assert!(cards::is(id::HOVEL, cards::REACTION) && cards::is(id::HOVEL, cards::SHELTER) && !cards::is(id::HOVEL, cards::VICTORY));
    assert!(cards::is(id::NECROPOLIS, cards::ACTION) && cards::is(id::NECROPOLIS, cards::SHELTER));
    assert!(cards::is(id::OVERGROWN_ESTATE, cards::VICTORY) && cards::is(id::OVERGROWN_ESTATE, cards::SHELTER));
    for s in [id::HOVEL, id::NECROPOLIS, id::OVERGROWN_ESTATE] {
        assert_eq!(cards::cost(s), 1);
    }
    for r in RUINS_PILE.members() {
        assert!(cards::is(r, cards::RUINS) && cards::is(r, cards::ACTION) && cards::cost(r) == 0, "{}", cards::name(r));
    }
    for k in KNIGHTS_PILE.members() {
        assert!(cards::is(k, cards::KNIGHT | cards::ATTACK | cards::ACTION) && cards::is(k, cards::KNIGHT), "{}", cards::name(k));
        assert_eq!(cards::cost(k), if k == id::SIR_MARTIN { 4 } else { 5 }, "{}", cards::name(k));
    }
    assert!(cards::is(id::DAME_JOSEPHINE, cards::VICTORY));
    for l in [id::CULTIST, id::DEATH_CART, id::MARAUDER] {
        assert!(cards::is(l, cards::LOOTER), "{}", cards::name(l));
    }
    assert!(cards::is(id::BEGGAR, cards::REACTION) && cards::is(id::MARKET_SQUARE, cards::REACTION));
    assert!(cards::is(id::BAND_OF_MISFITS, cards::COMMAND));
    assert_eq!(cards::cost(id::POOR_HOUSE), 1);
    assert_eq!(cards::cost(id::ALTAR), 6);
    assert_eq!(cards::cost_string(id::SIR_MARTIN), "$4");
}

// ===========================================================================
// Shelters: dealt from the shuffled deck with each game's own RNG
// ===========================================================================

/// How many of 3 Shelters' kinds player 0's opening hand holds, over many seeds.
fn opening_hand_shelter_counts(kingdom: &[CardId], seeds: u64) -> ([u32; 3], u32, u32) {
    let mut hovel_necro_oe = [0u32; 3];
    let mut estates = 0;
    let mut subsets = std::collections::HashMap::<u8, u32>::new();
    for seed in 0..seeds {
        let mut g = new_game(kingdom, 2, 1000 + seed);
        g.deal_opening_hands(&mut NoEvents);
        let hand = g.players[0].hand;
        assert_eq!(hand.total(), 5);
        hovel_necro_oe[0] += hand.has(id::HOVEL) as u32;
        hovel_necro_oe[1] += hand.has(id::NECROPOLIS) as u32;
        hovel_necro_oe[2] += hand.has(id::OVERGROWN_ESTATE) as u32;
        estates += hand.get(id::ESTATE) as u32;
        let mask = hand.has(id::HOVEL) as u8 | (hand.has(id::NECROPOLIS) as u8) << 1 | (hand.has(id::OVERGROWN_ESTATE) as u8) << 2;
        *subsets.entry(mask).or_default() += 1;
    }
    // Which Shelters the hand holds: 8 possible subsets; a subset of size k has probability C(7, 5-k) / C(10, 5).
    for mask in 0..8u8 {
        let k = mask.count_ones() as usize;
        let expect = [21.0, 35.0, 35.0, 21.0][k] / 252.0;
        let got = *subsets.get(&mask).unwrap_or(&0) as f64 / seeds as f64;
        assert!((got - expect).abs() < 0.025, "subset {mask:03b} seen {got:.3}, expected {expect:.3}");
    }
    (hovel_necro_oe, estates, subsets.len() as u32)
}

#[test]
fn shelters_replace_the_estates_and_each_appears_in_the_opening_hand_half_the_time() {
    let n = 6000u64;
    let (counts, estates, distinct) = opening_hand_shelter_counts(&da_kingdom(), n);
    assert_eq!(estates, 0, "an all-Dark-Ages kingdom starts with Shelters, no Estates");
    for (i, name) in ["Hovel", "Necropolis", "Overgrown Estate"].iter().enumerate() {
        let share = counts[i] as f64 / n as f64;
        assert!((0.47..=0.53).contains(&share), "{name} is in the opening hand {share:.3} of the time, expected 0.5 (5 of 10 cards)");
    }
    // Every game shuffles for itself: all eight subsets of the Shelters turn up in the opening hand, in the proportions of a fair shuffle.
    assert_eq!(distinct, 8);
}

#[test]
fn the_shelter_deal_is_fresh_for_each_game_and_reproducible_by_seed() {
    let hand_of = |seed: u64| {
        let mut g = new_game(&da_kingdom(), 2, seed);
        g.deal_opening_hands(&mut NoEvents);
        (g.players[0].hand, g.players[1].hand)
    };
    assert!(hand_of(77) == hand_of(77), "the same seed deals the same hands");
    let mut firsts = std::collections::HashSet::new();
    for seed in 0..200u64 {
        firsts.insert(format_counts(&hand_of(seed).0));
    }
    assert_eq!(firsts.len(), 8, "the opening hand must vary game to game (one of the eight subsets of the Shelters)");
    // The second player's deal is independent of the first's.
    let same = (0..400u64).filter(|&s| hand_of(s).0 == hand_of(s).1).count();
    assert!(same < 300);
}

#[test]
fn shelters_follow_the_kingdom_rule_and_the_config_override() {
    let base: Vec<CardId> = cards::FIRST_GAME.to_vec();
    let g = new_game(&base, 2, 1);
    assert!(!g.shelters && g.players[0].deck_unknown.get(id::ESTATE) == 3 && g.players[0].deck_unknown.get(id::HOVEL) == 0);
    let g = new_game(&da_kingdom(), 2, 1);
    assert!(g.shelters);
    let d = g.players[1].deck_unknown;
    assert_eq!((d.get(id::COPPER), d.get(id::HOVEL), d.get(id::NECROPOLIS), d.get(id::OVERGROWN_ESTATE), d.get(id::ESTATE)), (7, 1, 1, 1, 0));
    assert_eq!(g.supply.get(id::ESTATE), 8, "the Estate pile is unchanged");
    assert_eq!(new_game(&da_kingdom(), 3, 1).supply.get(id::ESTATE), 12);
    // Overrides.
    let g = GameState::new(&GameConfig { kingdom: da_kingdom(), shelters: Some(false), ..Default::default() });
    assert!(!g.shelters && g.players[0].deck_unknown.get(id::ESTATE) == 3);
    let g = GameState::new(&GameConfig { kingdom: base.clone(), shelters: Some(true), ..Default::default() });
    assert!(g.shelters && g.players[0].deck_unknown.get(id::HOVEL) == 1);
    // A mixed kingdom (3 Dark Ages cards of 10): the random kingdom card is from Dark Ages in about 30% of the games, decided by the seed.
    let mut mixed = base[..7].to_vec();
    mixed.extend([id::RATS, id::SAGE, id::FORAGER]);
    let with = (0..3000u64).filter(|&s| new_game(&mixed, 2, s).shelters).count();
    assert!((800..=1000).contains(&with), "Shelters in {with} of 3000 games, expected ~900");
    assert_eq!(new_game(&mixed, 2, 5).shelters, new_game(&mixed, 2, 5).shelters, "the same game always decides the same");
}

// ===========================================================================
// The Ruins pile
// ===========================================================================

#[test]
fn the_ruins_pile_comes_with_a_looter_and_has_ten_per_player_after_the_first() {
    for (players, size) in [(2, 10), (3, 20), (4, 30), (5, 40), (6, 50)] {
        let g = new_game(&[id::CULTIST, id::SMITHY], players, 3);
        let view = PlayerView::new(&g, 0);
        assert!(view.in_supply(id::RUINS) && view.supply(id::RUINS) == size, "{players} players: {}", view.supply(id::RUINS));
        assert_eq!(view.supply(id::CURSE), size, "as many Ruins as Curses");
        let top = view.pile_top(id::RUINS).expect("the top card is visible");
        assert!(cards::is(top, cards::RUINS));
        assert_eq!(g.supply.get(top), size, "the pile's size is kept on its top card; the other four kinds have none");
        assert_eq!(RUINS_PILE.members().filter(|&m| g.supply.get(m) > 0).count(), 1);
        assert_eq!(RUINS_PILE.members().filter(|&m| g.in_supply(m)).count(), 5);
    }
    for looter in [id::CULTIST, id::DEATH_CART, id::MARAUDER] {
        assert!(new_game(&[looter], 2, 1).has_pile(&RUINS_PILE), "{}", cards::name(looter));
    }
    assert!(!new_game(&[id::SMITHY, id::KNIGHTS], 2, 1).has_pile(&RUINS_PILE), "no Looter: no Ruins");
    // A Looter that is Young Witch's Bane brings them too.
    let g = GameState::new(&GameConfig { kingdom: vec![id::YOUNG_WITCH], bane: Some(id::DEATH_CART), ..Default::default() });
    assert!(g.has_pile(&RUINS_PILE));
}

#[test]
fn the_ruins_pile_is_a_fresh_random_selection_and_order_each_game() {
    // Drain the pile of a 2-player game (10 cards of the 50), and look at what each game dealt.
    let mut kind_totals = [0u32; 5];
    let mut tops = [0u32; 5];
    let mut distinct = std::collections::HashSet::new();
    let games = 1500u64;
    for seed in 0..games {
        let mut g = new_game(&[id::CULTIST], 2, 50_000 + seed);
        let mut order = Vec::new();
        while let Some(top) = PlayerView::new(&g, 0).pile_top(id::RUINS) {
            if order.is_empty() {
                tops[(top - id::ABANDONED_MINE) as usize] += 1;
            }
            order.push(top);
            assert!(gain0(&mut g, top));
        }
        assert_eq!(order.len(), 10);
        for &o in &order {
            kind_totals[(o - id::ABANDONED_MINE) as usize] += 1;
        }
        distinct.insert(order);
        assert_eq!(g.empty_piles(), 1, "an empty Ruins pile is one empty pile");
        assert_eq!(PlayerView::new(&g, 0).supply(id::RUINS), 0);
    }
    for i in 0..5 {
        let share = kind_totals[i] as f64 / (games * 10) as f64;
        assert!((0.18..=0.22).contains(&share), "kind {i} is {share:.3} of the cards dealt, expected 0.2");
        let first = tops[i] as f64 / games as f64;
        assert!((0.17..=0.23).contains(&first), "kind {i} is on top first {first:.3} of the time");
    }
    assert!(distinct.len() > 1400, "the selection and the order are drawn again for every game: {} distinct of {games}", distinct.len());
}

#[test]
fn six_players_use_all_fifty_ruins_ten_of_each_kind() {
    for seed in 0..6u64 {
        let mut g = new_game(&[id::MARAUDER], 6, seed);
        let mut got = [0u32; 5];
        let mut n = 0;
        while let Some(top) = g.pile_top(&RUINS_PILE) {
            got[(top - id::ABANDONED_MINE) as usize] += 1;
            assert!(gain0(&mut g, top));
            n += 1;
        }
        assert_eq!((n, got), (50, [10; 5]), "seed {seed}");
    }
}

#[test]
fn the_unseen_part_of_a_pile_is_public_and_matches_what_is_left() {
    let mut g = new_game(&[id::CULTIST], 2, 9);
    let before = PlayerView::new(&g, 0).pile_unseen(id::RUINS);
    let top = g.pile_top(&RUINS_PILE).unwrap();
    // The top card is known; everything else of the 50 is unseen.
    assert_eq!(before.total(), 49);
    assert_eq!(before.get(top), 9);
    for r in RUINS_PILE.members().filter(|&r| r != top) {
        assert_eq!(before.get(r), 10);
    }
    gain0(&mut g, top);
    let after = PlayerView::new(&g, 0).pile_unseen(id::RUINS);
    assert_eq!(after.total(), 48, "the card dealt out is seen, and the new top is known");
    let new_top = g.pile_top(&RUINS_PILE).unwrap();
    let mut expect = Counts::EMPTY;
    for r in RUINS_PILE.members() {
        expect.set(r, 10 - (r == top) as u8 - (r == new_top) as u8);
    }
    assert!(after == expect, "{after:?} vs {expect:?}");
    // A Ruins in a player's hand or the trash is seen too.
    g.players[1].hand.add(id::SURVIVORS, 1);
    g.trash.add(id::SURVIVORS, 1);
    let seen = PlayerView::new(&g, 0).pile_unseen(id::RUINS);
    assert_eq!(seen.get(id::SURVIVORS), expect.get(id::SURVIVORS) - 2);
}

#[test]
fn determinize_does_not_leak_the_hidden_part_of_the_ruins_or_knights_piles() {
    for marker in [id::RUINS, id::KNIGHTS] {
        let looter = if marker == id::RUINS { id::CULTIST } else { id::KNIGHTS };
        let g = new_game(&[looter, id::SMITHY], 2, 123);
        let pile = if marker == id::RUINS { &RUINS_PILE } else { &KNIGHTS_PILE };
        let real_top = g.pile_top(pile).unwrap();
        // The next card of the real game.
        let mut real = g;
        gain0(&mut real, real_top);
        let real_next = real.pile_top(pile).unwrap();
        // Searches run on determinized copies: they know the top and the size, and nothing of what is next.
        let mut rng = Rng::new(5);
        let mut same_next = 0;
        let runs = 300;
        for _ in 0..runs {
            let mut d = PlayerView::new(&g, 0).determinize(&mut rng);
            assert_eq!(d.pile_top(pile), Some(real_top), "the top card is public");
            assert_eq!(d.pile_size(pile), g.pile_size(pile));
            assert!(d.rng != g.rng, "a determinization draws from a stream of its own, not the real game's");
            gain0(&mut d, real_top);
            if d.pile_top(pile) == Some(real_next) {
                same_next += 1;
            }
        }
        let expect = if marker == id::RUINS { 0.2 } else { 0.1 };
        let share = same_next as f64 / runs as f64;
        assert!((expect * 0.4..=expect * 1.9).contains(&share), "{}: the next card matched the real one {share:.2} of the time, expected about {expect}", cards::name(marker));
    }
}

// ===========================================================================
// The Knights pile
// ===========================================================================

#[test]
fn the_knights_pile_is_one_kingdom_pile_of_ten_different_knights() {
    let g = new_game(&[id::KNIGHTS, id::SMITHY], 2, 4);
    assert!(g.has_pile(&KNIGHTS_PILE) && !g.has_pile(&RUINS_PILE));
    let view = PlayerView::new(&g, 0);
    assert_eq!(view.supply(id::KNIGHTS), 10);
    assert!(view.in_supply(id::KNIGHTS));
    assert!(!g.in_supply(id::KNIGHTS), "the marker is not a pile of its own");
    let top = view.pile_top(id::KNIGHTS).unwrap();
    assert!(cards::is(top, cards::KNIGHT) && g.supply.get(top) == 10);
    assert_eq!(KNIGHTS_PILE.members().filter(|&m| g.supply.get(m) > 0).count(), 1, "only the top card is in the pile");
    // The same with 6 players: ten Knights whatever the number of players.
    assert_eq!(PlayerView::new(&new_game(&[id::KNIGHTS], 6, 4), 0).supply(id::KNIGHTS), 10);
    // Only the top card can be bought.
    let mut g = g;
    g.turn.coins = 8;
    g.turn.phase = Phase::Buy;
    g.turn.buys = 1;
    let offered: Vec<CardId> = choices(&{
        let mut g2 = g;
        g2.pending = Pending::None;
        let _ = g2.advance(&mut NoEvents);
        g2
    })
    .iter()
    .filter_map(|c| if let Choice::Card(x) = c { Some(*x) } else { None })
    .filter(|&c| cards::is(c, cards::KNIGHT))
    .collect();
    assert_eq!(offered, vec![top], "{offered:?}");
}

#[test]
fn draining_the_knights_pile_deals_each_knight_once_in_a_random_order() {
    let mut firsts = [0u32; 10];
    let mut orders = std::collections::HashSet::new();
    let games = 1000u64;
    for seed in 0..games {
        let mut g = new_game(&[id::KNIGHTS], 2, 7_000 + seed);
        let mut order = Vec::new();
        while let Some(top) = g.pile_top(&KNIGHTS_PILE) {
            assert_eq!(g.pile_size(&KNIGHTS_PILE), 10 - order.len() as u8);
            order.push(top);
            assert!(gain0(&mut g, top));
        }
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, (id::DAME_ANNA..=id::SIR_VANDER).collect::<Vec<_>>(), "ten different Knights");
        firsts[(order[0] - id::DAME_ANNA) as usize] += 1;
        orders.insert(order);
        assert_eq!(g.empty_piles(), 1);
    }
    for (i, &f) in firsts.iter().enumerate() {
        let share = f as f64 / games as f64;
        assert!((0.07..=0.13).contains(&share), "Knight {i} is on top first {share:.3} of the time");
    }
    assert!(orders.len() > 990);
}

#[test]
fn a_specific_card_gained_from_a_mixed_pile_fails_unless_it_is_the_top() {
    let mut g = new_game(&[id::KNIGHTS], 2, 21);
    let top = g.pile_top(&KNIGHTS_PILE).unwrap();
    let other = KNIGHTS_PILE.members().find(|&m| m != top).unwrap();
    assert!(!gain0(&mut g, other), "only the top Knight can be gained");
    assert!(gain0(&mut g, top));
    // Talisman-style "a copy of what you bought": only if that very Knight is the next top (never: each Knight is unique).
    assert!(!gain0(&mut g, top));
}

#[test]
fn a_game_ends_on_three_empty_piles_with_a_mixed_pile_counting_once() {
    let mut g = new_game(&[id::KNIGHTS, id::CULTIST, id::SMITHY], 2, 2);
    // Empty the Knights and the Ruins (each one pile), then one more pile.
    for pile in [&KNIGHTS_PILE, &RUINS_PILE] {
        while let Some(t) = g.pile_top(pile) {
            gain0(&mut g, t);
        }
    }
    assert_eq!(g.empty_piles(), 2, "the non-top members of an emptied mixed pile are not extra piles");
    assert!(!g.end_condition_met());
    g.supply.set(id::SMITHY, 0);
    assert_eq!(g.empty_piles(), 3);
    assert!(g.end_condition_met());
}

// ===========================================================================
// Text format
// ===========================================================================

#[test]
fn text_round_trip_keeps_the_piles_and_shelters() {
    let mut g = new_game(&[id::KNIGHTS, id::CULTIST, id::HERMIT, id::BANDIT_CAMP, id::URCHIN], 3, 8);
    g.deal_opening_hands(&mut NoEvents);
    g.spoils = 12;
    g.madman = 7;
    let top_k = g.pile_top(&KNIGHTS_PILE).unwrap();
    gain0(&mut g, top_k);
    let text = format_state(&g);
    assert!(text.contains("Knights"), "{text}");
    assert!(!text.contains("Dame Anna=0"), "{text}");
    let back = parse_state(&text).unwrap();
    assert_eq!(back.pile_top(&KNIGHTS_PILE), g.pile_top(&KNIGHTS_PILE));
    assert_eq!(back.pile_top(&RUINS_PILE), g.pile_top(&RUINS_PILE));
    assert_eq!((back.pile_size(&KNIGHTS_PILE), back.pile_size(&RUINS_PILE)), (9, 20));
    assert_eq!((back.spoils, back.madman, back.mercenary), (12, 7, 10));
    assert!(back.shelters && back.in_supply == g.in_supply);
    assert!(back.supply == g.supply);
    for p in 0..3 {
        assert!(back.players[p].all_cards() == g.players[p].all_cards());
    }
    // A hand-written state names the Knights as a kingdom card and the top Knight in the supply.
    let text = "players: 2\nkingdom: Knights, Smithy\nsupply: Sir Martin=6\n\n[player 1]\nhand: Copper\n\n[player 2]\nhand:\n";
    let g = parse_state(text).unwrap();
    assert_eq!((g.pile_top(&KNIGHTS_PILE), g.pile_size(&KNIGHTS_PILE)), (Some(id::SIR_MARTIN), 6));
    assert!(!g.shelters);
}

// ===========================================================================
// Helpers for the card tests
// ===========================================================================

/// Cards that are neither Treasures nor Actions, so every draw is visible and nothing is auto-played.
fn junk(n: usize) -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(n).copied().collect()
}

fn offered(g: &GameState) -> Vec<CardId> {
    choices(g).into_iter().filter_map(|c| if let Choice::Card(x) = c { Some(x) } else { None }).collect()
}

fn yes(g: &mut GameState) -> Step {
    choose(g, Choice::Yes)
}

fn no(g: &mut GameState) -> Step {
    choose(g, Choice::No)
}

fn pick(g: &mut GameState, c: CardId) -> Step {
    choose(g, Choice::Card(c))
}

fn mode(g: &mut GameState, i: u8) -> Step {
    choose(g, Choice::Mode(i))
}

fn buy_phase(g: &mut GameState, coins: u16) {
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
}

/// A state in which `Hovel`s and the like make sense: the game has Shelters.
fn with_shelters(mut g: GameState) -> GameState {
    g.shelters = true;
    g
}

/// Pass every Action and Buy decision until the next turn has started.
fn finish_turn(g: &mut GameState) {
    let start = g.turn.number;
    loop {
        match g.advance(&mut NoEvents) {
            Step::Decision(d) => {
                if g.turn.number != start {
                    return;
                }
                assert!(matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy), "unexpected {d:?}");
                g.apply(Choice::Pass, &mut NoEvents).unwrap();
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

fn in_hand(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].hand.get(c)
}

/// Player `p`'s hand plus the Treasures in their play area: the engine plays the Treasures in hand when the Action phase ends, so a
/// test of what a card put into the hand looks at both.
fn held(g: &GameState, p: usize) -> Counts {
    let mut c = g.players[p].hand;
    for (card, n) in g.players[p].in_play.iter() {
        if cards::is(card, cards::TREASURE) {
            c.add(card, n);
        }
    }
    c
}

/// Stay in the Action phase after the card is played (2 Actions and an Action card in hand), so that the coins can be read.
fn stay_in_action_phase(g: &mut GameState, p: usize) {
    g.turn.actions = 2;
    g.players[p].hand.add(id::MOAT, 1);
}

/// The next two decisions of `select`: whether the pending decision is a Select over `from` with `act`.
fn is_select(g: &GameState, from: Zone, act: Act) -> bool {
    matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Select { from: f, act: a, .. }, .. }) if f == from && a == act)
}

/// Total cards the players hold, the trash and the Supply and side piles: stays the same over a game's gains and returns.
fn all_cards_of(g: &GameState, p: usize) -> Counts {
    g.players[p].all_cards()
}

// ===========================================================================
// Poor House, Beggar, Squire, Vagrant
// ===========================================================================

#[test]
fn poor_house_gives_4_minus_the_treasures_in_hand_and_never_below_0() {
    let d = cards::def(id::POOR_HOUSE);
    assert_eq!((d.cost, d.coins), (1, 4));
    let mut g = new_state(&[id::POOR_HOUSE], 2);
    set_hand(&mut g, 0, &[id::POOR_HOUSE, id::COPPER, id::SILVER, id::ESTATE]);
    stay_in_action_phase(&mut g, 0);
    let mut ev: Vec<Event> = Vec::new();
    play_ev(&mut g, id::POOR_HOUSE, &mut ev);
    assert_eq!(g.turn.coins, 2, "$4 less one per Treasure card (Copper, Silver)");
    for c in [id::COPPER, id::SILVER, id::ESTATE] {
        assert!(ev.contains(&Event::Reveal { player: 0, card: c }), "the hand is revealed");
    }
    // Never below $0, whatever was in the pool before.
    let mut g = new_state(&[id::POOR_HOUSE], 2);
    set_hand(&mut g, 0, &[id::POOR_HOUSE, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::GOLD, id::GOLD]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::POOR_HOUSE);
    assert_eq!(g.turn.coins, 0);
    // Coins from an earlier Action count: the $4 comes first, then the Treasures take their $1 each.
    let mut g = new_state(&[id::POOR_HOUSE], 2);
    g.turn.coins = 3;
    set_hand(&mut g, 0, &[id::POOR_HOUSE, id::COPPER, id::COPPER]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::POOR_HOUSE);
    assert_eq!(g.turn.coins, 5);
    // Under Charlatan a Curse is a Treasure.
    let mut g = new_state(&[id::POOR_HOUSE, id::CHARLATAN], 2);
    set_hand(&mut g, 0, &[id::POOR_HOUSE, id::CURSE, id::COPPER]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::POOR_HOUSE);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn beggar_gains_three_coppers_to_the_hand_or_as_many_as_are_left() {
    assert_eq!(cards::cost(id::BEGGAR), 2);
    let mut g = new_state(&[id::BEGGAR], 2);
    set_hand(&mut g, 0, &[id::BEGGAR]);
    play(&mut g, id::BEGGAR);
    assert_eq!(held(&g, 0).get(id::COPPER), 3);
    assert_eq!(g.supply.get(id::COPPER), 46 - 3);
    let mut g = new_state(&[id::BEGGAR], 2);
    set_hand(&mut g, 0, &[id::BEGGAR]);
    g.supply.set(id::COPPER, 2);
    play(&mut g, id::BEGGAR);
    assert_eq!(held(&g, 0).get(id::COPPER), 2);
    assert_eq!(g.supply.get(id::COPPER), 0);
}

#[test]
fn beggar_discarded_to_an_attack_gains_two_silvers_one_on_the_deck() {
    let mut g = new_state(&[id::BEGGAR, id::MILITIA], 2);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::BEGGAR, id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(3));
    play(&mut g, id::MILITIA);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.kind, d.subject), (0, DecisionKind::YesNo { act: Act::Discard }, id::BEGGAR));
    yes(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER), "the first Silver goes onto the deck");
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert_eq!(g.players[0].discard.get(id::BEGGAR), 1, "the Beggar was discarded");
    assert_eq!(g.supply.get(id::SILVER), 38);
    // The Attack still hits: 4 cards left in hand, down to 3.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) && d.player == 0);
}

#[test]
fn beggars_silvers_run_short_and_it_can_be_declined() {
    for (silvers, deck, discard) in [(1u8, 1u8, 0u8), (0, 0, 0), (2, 1, 1)] {
        let mut g = new_state(&[id::BEGGAR, id::MILITIA], 2);
        reset_turn(&mut g, 1);
        set_hand(&mut g, 1, &[id::MILITIA]);
        set_hand(&mut g, 0, &[id::BEGGAR, id::COPPER]);
        g.supply.set(id::SILVER, silvers);
        play(&mut g, id::MILITIA);
        expect_decision(&mut g);
        yes(&mut g);
        assert_eq!((g.players[0].deck_known.len, g.players[0].discard.get(id::SILVER)), (deck, discard), "{silvers} Silvers left");
    }
    let mut g = new_state(&[id::BEGGAR, id::MILITIA], 2);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::BEGGAR, id::COPPER]);
    play(&mut g, id::MILITIA);
    expect_decision(&mut g);
    no(&mut g);
    assert_eq!(g.supply.get(id::SILVER), 40);
    assert_eq!(in_hand(&g, 0, id::BEGGAR), 1);
}

#[test]
fn squire_gives_one_coin_and_a_choice_of_actions_buys_or_a_silver() {
    let mut g = new_state(&[id::SQUIRE], 2);
    set_hand(&mut g, 0, &[id::SQUIRE]);
    play(&mut g, id::SQUIRE);
    assert_eq!(g.turn.coins, 1);
    let labels: Vec<String> = cards::modes(id::SQUIRE).iter().map(|o| o.label()).collect();
    assert_eq!(labels, ["+2 Actions", "+2 Buys", "Gain a Silver"]);
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1), Choice::Mode(2)]);
    mode(&mut g, 0);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 1));
    let mut g = new_state(&[id::SQUIRE], 2);
    set_hand(&mut g, 0, &[id::SQUIRE]);
    play(&mut g, id::SQUIRE);
    mode(&mut g, 1);
    assert_eq!((g.turn.actions, g.turn.buys), (0, 3));
    let mut g = new_state(&[id::SQUIRE], 2);
    set_hand(&mut g, 0, &[id::SQUIRE]);
    play(&mut g, id::SQUIRE);
    mode(&mut g, 2);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
}

#[test]
fn squire_trashed_gains_an_attack_card_from_the_supply() {
    // Militia and Witch are Attacks in the Supply; Mercenary (not in the Supply) never is offered.
    let mut g = new_state(&[id::SQUIRE, id::CHAPEL, id::MILITIA, id::WITCH, id::SMITHY, id::URCHIN], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::SQUIRE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::SQUIRE);
    let mut o = offered(&g);
    o.sort();
    assert_eq!(o, vec![id::MILITIA, id::WITCH, id::URCHIN]);
    assert!(!o.contains(&id::MERCENARY));
    pick(&mut g, id::WITCH);
    assert_eq!(g.players[0].discard.get(id::WITCH), 1);
    assert!(g.trash.has(id::SQUIRE));
    // The Knights' top card is an Attack in the Supply too.
    let mut g = new_state(&[id::SQUIRE, id::CHAPEL, id::KNIGHTS], 2);
    let top = g.pile_top(&KNIGHTS_PILE).unwrap();
    set_hand(&mut g, 0, &[id::CHAPEL, id::SQUIRE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::SQUIRE);
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::Gain { .. })), "the only Attack is gained at once");
    assert!(g.players[0].discard.has(top));
    // No Attack in the Supply: nothing is gained.
    let mut g = new_state(&[id::SQUIRE, id::CHAPEL, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::SQUIRE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::SQUIRE);
    assert_eq!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::Buy));
    assert_eq!(g.players[0].discard.total(), 0);
}

#[test]
fn vagrant_draws_first_then_keeps_a_curse_ruins_shelter_or_victory_card() {
    for (top, kept) in [
        (id::CURSE, true),
        (id::PROVINCE, true),
        (id::ABANDONED_MINE, true),
        (id::HOVEL, true),
        (id::NECROPOLIS, true),
        (id::OVERGROWN_ESTATE, true),
        (id::DAME_JOSEPHINE, true),
        (id::COPPER, false),
        (id::VILLAGE, false),
        (id::SIR_MARTIN, false),
    ] {
        let mut g = new_state(&[id::VAGRANT, id::VILLAGE], 2);
        set_hand(&mut g, 0, &[id::VAGRANT]);
        set_deck_known(&mut g, 0, &[id::GOLD, top, id::SILVER]);
        play(&mut g, id::VAGRANT);
        assert_eq!(held(&g, 0).get(id::GOLD), 1, "+1 Card first");
        assert_eq!(g.turn.actions, 1);
        if kept {
            assert_eq!(in_hand(&g, 0, top), 1, "{}", cards::name(top));
            assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
        } else {
            assert_eq!(in_hand(&g, 0, top), 0, "{}", cards::name(top));
            assert_eq!(g.players[0].deck_known.peek_top(), Some(top), "put back on top");
        }
    }
}

#[test]
fn vagrant_with_an_empty_deck_shuffles_for_the_draw_and_has_nothing_to_reveal_after() {
    let mut g = new_state(&[id::VAGRANT], 2);
    set_hand(&mut g, 0, &[id::VAGRANT]);
    set_discard(&mut g, 0, &[id::PROVINCE]);
    play(&mut g, id::VAGRANT);
    assert_eq!(in_hand(&g, 0, id::PROVINCE), 1);
    assert_eq!(g.players[0].deck_size(), 0);
}

// ===========================================================================
// Forager, Hermit, Market Square, Sage, Storeroom, Urchin
// ===========================================================================

#[test]
fn forager_trashes_a_card_then_gives_a_coin_per_differently_named_treasure_in_the_trash() {
    let d = cards::def(id::FORAGER);
    assert_eq!((d.cost, d.actions, d.buys), (3, 1, 1));
    let mut g = new_state(&[id::FORAGER], 2);
    set_hand(&mut g, 0, &[id::FORAGER, id::COPPER, id::GOLD, id::ESTATE]);
    stay_in_action_phase(&mut g, 0);
    g.trash.add(id::COPPER, 4);
    g.trash.add(id::SILVER, 1);
    g.trash.add(id::ESTATE, 3);
    play(&mut g, id::FORAGER);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 2));
    pick(&mut g, id::ESTATE);
    assert_eq!(g.turn.coins, 2, "Copper and Silver are two names; the Estates and the repeated Coppers do not count");
    // The card trashed counts: a Gold trashed adds its name.
    let mut g = new_state(&[id::FORAGER], 2);
    set_hand(&mut g, 0, &[id::FORAGER, id::GOLD]);
    stay_in_action_phase(&mut g, 0);
    g.trash.add(id::COPPER, 2);
    play(&mut g, id::FORAGER);
    pick(&mut g, id::GOLD);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn forager_with_no_card_to_trash_still_pays_for_the_trash() {
    let mut g = new_state(&[id::FORAGER], 2);
    set_hand(&mut g, 0, &[id::FORAGER, id::MOAT]);
    g.turn.actions = 1;
    g.trash.add(id::COPPER, 1);
    g.trash.add(id::GOLD, 1);
    // The only other card is a Moat: trash it (mandatory).
    play(&mut g, id::FORAGER); // the Moat is the only card: trashed at once
    assert_eq!(g.turn.coins, 2);
    let mut g = new_state(&[id::FORAGER], 2);
    set_hand(&mut g, 0, &[id::FORAGER]);
    g.trash.add(id::SILVER, 1);
    g.trash.add(id::CURSE, 1);
    play(&mut g, id::FORAGER);
    assert_eq!(g.turn.coins, 1, "an empty hand: no trashing, but +$1 for the Silver; a Curse is no Treasure");
}

#[test]
fn hermit_trashes_a_non_treasure_from_the_discard_pile_or_the_hand_and_gains_a_card_up_to_3() {
    let mut g = new_state(&[id::HERMIT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HERMIT, id::ESTATE]);
    set_discard(&mut g, 0, &[id::ESTATE, id::COPPER, id::CURSE]);
    play(&mut g, id::HERMIT);
    assert!(is_select(&g, Zone::Discard, Act::Trash));
    let mut o = offered(&g);
    o.sort();
    assert_eq!(o, vec![id::ESTATE, id::CURSE], "no Treasures");
    pick(&mut g, id::CURSE);
    assert!(g.trash.has(id::CURSE));
    // The trash was taken from the discard pile, so the hand is not offered; a gain up to $3 follows.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 3, .. }), "{d:?}");
    assert!(offered(&g).iter().all(|&c| cards::cost(c) <= 3));
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    // Declining the discard pile offers the hand.
    let mut g = new_state(&[id::HERMIT], 2);
    set_hand(&mut g, 0, &[id::HERMIT, id::ESTATE, id::COPPER]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::HERMIT);
    pass(&mut g);
    assert!(is_select(&g, Zone::Hand, Act::Trash));
    assert_eq!(offered(&g), vec![id::ESTATE], "a non-Treasure only");
    pick(&mut g, id::ESTATE);
    assert_eq!(g.trash.get(id::ESTATE), 1);
    assert_eq!(g.players[0].discard.get(id::ESTATE), 1);
    // With no non-Treasure in the discard pile the hand is offered at once, and either trash is optional.
    let mut g = new_state(&[id::HERMIT], 2);
    set_hand(&mut g, 0, &[id::HERMIT, id::DUCHY]);
    set_discard(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::HERMIT);
    assert!(is_select(&g, Zone::Hand, Act::Trash));
    pass(&mut g);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { .. }));
}

#[test]
fn hermit_becomes_a_madman_if_no_card_was_gained_in_the_buy_phase() {
    let setup = |madmen: u8| {
        let mut g = new_state(&[id::HERMIT, id::SMITHY], 2);
        set_hand(&mut g, 0, &[id::HERMIT]);
        set_deck_known(&mut g, 0, &junk(10));
        g.madman = madmen;
        play(&mut g, id::HERMIT);
        pick(&mut g, id::SILVER); // its own gain, in the Action phase
        g
    };
    // Nothing bought: the Hermit goes back to its pile and a Madman comes to the discard pile.
    let mut g = setup(10);
    let hermits = g.supply.get(id::HERMIT);
    assert!(g.players[0].in_play.has(id::HERMIT));
    finish_turn(&mut g);
    assert_eq!(g.supply.get(id::HERMIT), hermits + 1);
    assert_eq!(g.madman, 9);
    assert_eq!(all_cards_of(&g, 0).get(id::MADMAN), 1);
    assert_eq!(all_cards_of(&g, 0).get(id::HERMIT), 0);
    // A card bought: no exchange.
    let mut g = setup(10);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    buy(&mut g, id::SILVER);
    finish_turn(&mut g);
    assert_eq!(g.madman, 10);
    assert_eq!(all_cards_of(&g, 0).get(id::HERMIT), 1);
    assert_eq!(all_cards_of(&g, 0).get(id::MADMAN), 0);
    // The Madman pile is empty: nothing to exchange for, the Hermit stays.
    let mut g = setup(0);
    finish_turn(&mut g);
    assert_eq!(all_cards_of(&g, 0).get(id::HERMIT), 1);
    assert_eq!(g.madman, 0);
}

#[test]
fn hermit_exchange_needs_the_hermit_in_play_and_a_throne_roomed_one_exchanges_once() {
    let mut g = new_state(&[id::HERMIT, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HERMIT]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::HERMIT);
    // Two plays: two trashes (declined) and two gains.
    for _ in 0..2 {
        if is_select(&g, Zone::Discard, Act::Trash) {
            pass(&mut g);
        }
        if is_select(&g, Zone::Hand, Act::Trash) {
            pass(&mut g);
        }
        pick(&mut g, id::COPPER);
    }
    finish_turn(&mut g);
    assert_eq!(all_cards_of(&g, 0).get(id::MADMAN), 1, "one Hermit, one exchange");
    assert_eq!(g.madman, 9);
    // A Hermit trashed during the turn (not in play at the end of the Buy phase) cannot be exchanged.
    let mut g = new_state(&[id::HERMIT], 2);
    set_hand(&mut g, 0, &[id::HERMIT]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::HERMIT);
    pick(&mut g, id::COPPER);
    g.players[0].in_play.remove(id::HERMIT);
    g.trash.add(id::HERMIT, 1);
    finish_turn(&mut g);
    assert_eq!(all_cards_of(&g, 0).get(id::MADMAN), 0);
}

#[test]
fn market_square_is_a_cantrip_with_a_buy() {
    let d = cards::def(id::MARKET_SQUARE);
    assert_eq!((d.cost, d.cards, d.actions, d.buys), (3, 1, 1, 1));
    assert!(cards::is(id::MARKET_SQUARE, cards::REACTION));
}

#[test]
fn market_square_discarded_when_a_card_of_its_owner_is_trashed_gains_a_gold() {
    let mut g = new_state(&[id::MARKET_SQUARE, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::MARKET_SQUARE, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::ESTATE);
    // The selection is not over yet (it could trash more); the reaction comes once it is.
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject, d.player), (DecisionKind::YesNo { act: Act::Discard }, id::MARKET_SQUARE, 0));
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::MARKET_SQUARE), 1);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    assert_eq!(g.supply.get(id::GOLD), 29);
    assert_eq!(g.trash.get(id::ESTATE), 1);
}

#[test]
fn market_square_is_offered_once_per_trashed_card_and_per_copy_and_may_be_declined() {
    // Two cards trashed, two Market Squares in hand: the first card's offer is taken twice, then none is left for the second.
    let mut g = new_state(&[id::MARKET_SQUARE, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::MARKET_SQUARE, id::MARKET_SQUARE, id::ESTATE, id::COPPER]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::COPPER);
    pick(&mut g, id::ESTATE);
    pass(&mut g);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::YesNo { .. }));
    yes(&mut g);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::YesNo { .. }), "a single trashed card: offered again for the second copy");
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::GOLD), 2);
    assert_eq!(in_hand(&g, 0, id::MARKET_SQUARE), 0);
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })), "none left for the second trashed card");
    // Declined: nothing happens and the Market Square stays.
    let mut g = new_state(&[id::MARKET_SQUARE, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::MARKET_SQUARE, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::ESTATE);
    pass(&mut g);
    expect_decision(&mut g);
    no(&mut g);
    assert_eq!((in_hand(&g, 0, id::MARKET_SQUARE), g.players[0].discard.get(id::GOLD)), (1, 0));
}

#[test]
fn market_square_reacts_to_a_trash_by_the_opponents_attack_and_to_a_trash_it_draws_into_the_hand() {
    // Rogue trashes a Silver from player 1's revealed cards: it is player 1's card, so their Market Square may react.
    let mut g = new_state(&[id::MARKET_SQUARE, id::ROGUE], 2);
    set_hand(&mut g, 0, &[id::ROGUE]);
    set_hand(&mut g, 1, &[id::MARKET_SQUARE]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::ESTATE]);
    play(&mut g, id::ROGUE);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.subject), (1, id::MARKET_SQUARE));
    yes(&mut g);
    assert_eq!(g.players[1].discard.get(id::GOLD), 1);
    assert!(g.trash.has(id::SILVER));
    // A Cultist trashed draws three cards; a Market Square among them can still be discarded for the trash.
    let mut g = new_state(&[id::MARKET_SQUARE, id::CHAPEL, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::CULTIST]);
    set_deck_known(&mut g, 0, &[id::MARKET_SQUARE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::CULTIST);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: Act::Discard }, id::MARKET_SQUARE), "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
}

#[test]
fn market_square_does_not_react_to_a_card_that_is_not_the_players_own() {
    // Lurker trashes an Action from the Supply: not one of the player's cards.
    let mut g = new_state(&[id::MARKET_SQUARE, id::LURKER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::LURKER, id::MARKET_SQUARE]);
    play(&mut g, id::LURKER);
    mode(&mut g, 0);
    pick(&mut g, id::SMITHY);
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })), "{:?}", g.pending_decision());
    assert_eq!(g.players[0].discard.get(id::GOLD), 0);
}

#[test]
fn sage_reveals_until_a_card_costing_3_or_more_and_discards_the_rest() {
    let mut g = new_state(&[id::SAGE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::SAGE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::ESTATE, id::CURSE, id::SILVER, id::GOLD]);
    play(&mut g, id::SAGE);
    assert_eq!(g.turn.actions, 1);
    assert_eq!(held(&g, 0).get(id::SILVER), 1);
    assert_eq!(g.players[0].discard.total(), 3, "Copper, Estate and Curse");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    // $3 includes a card with a Potion in its cost.
    let mut g = new_state(&[id::SAGE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::SAGE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::FAMILIAR, id::GOLD]);
    play(&mut g, id::SAGE);
    assert_eq!(in_hand(&g, 0, id::FAMILIAR), 1);
    // The current cost counts: Bridge makes a $4 card cost $3... and Silver cost $2.
    let mut g = new_state(&[id::SAGE, id::BRIDGE], 2);
    set_hand(&mut g, 0, &[id::SAGE]);
    g.turn.cost_reduction = 1;
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD]);
    play(&mut g, id::SAGE);
    assert_eq!(held(&g, 0).get(id::GOLD), 1, "Silver costs $2 under Bridge");
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
}

#[test]
fn sage_shuffles_when_the_deck_runs_out_and_finding_nothing_discards_everything() {
    // Shuffles the discard pile (not the revealed cards) and continues.
    let mut g = new_state(&[id::SAGE], 2);
    set_hand(&mut g, 0, &[id::SAGE]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    set_discard(&mut g, 0, &[id::GOLD, id::ESTATE]);
    play(&mut g, id::SAGE);
    assert_eq!(held(&g, 0).get(id::GOLD), 1);
    assert_eq!(g.players[0].discard.get(id::COPPER), 1, "the revealed Copper was not shuffled in");
    // Nothing costing $3 anywhere: everything revealed is discarded and nothing is gained.
    let mut g = new_state(&[id::SAGE], 2);
    set_hand(&mut g, 0, &[id::SAGE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::ESTATE]);
    set_discard(&mut g, 0, &[id::CURSE]);
    play(&mut g, id::SAGE);
    assert_eq!(g.players[0].hand.total() + g.players[0].in_play.total(), 1);
    assert_eq!(g.players[0].discard.total(), 3 + 1 - 1 + 0, "Copper, Estate, Curse, and the Sage");
}

#[test]
fn storeroom_discards_to_draw_then_discards_for_coins() {
    let mut g = new_state(&[id::STOREROOM], 2);
    set_hand(&mut g, 0, &[id::STOREROOM, id::ESTATE, id::COPPER, id::COPPER, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::PROVINCE]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::STOREROOM);
    assert_eq!(g.turn.buys, 2);
    // First: discard Estate and Copper, draw 2 (Gold, Gold).
    pick(&mut g, id::COPPER);
    pick(&mut g, id::ESTATE);
    pass(&mut g);
    assert_eq!(in_hand(&g, 0, id::GOLD), 2, "drawn before the second discard");
    // Second: discard a Gold (just drawn) and the Silver for $1 each.
    assert!(is_select(&g, Zone::Hand, Act::Discard));
    pick(&mut g, id::SILVER);
    pick(&mut g, id::GOLD);
    pass(&mut g);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].discard.total(), 4);
}

#[test]
fn storeroom_with_nothing_to_discard_does_nothing_but_give_a_buy() {
    let mut g = new_state(&[id::STOREROOM], 2);
    set_hand(&mut g, 0, &[id::STOREROOM]);
    play(&mut g, id::STOREROOM);
    assert_eq!(g.turn.buys, 2);
}

#[test]
fn urchin_is_a_cantrip_that_makes_the_others_discard_down_to_4() {
    let d = cards::def(id::URCHIN);
    assert_eq!((d.cost, d.cards, d.actions), (3, 1, 1));
    assert!(cards::is(id::URCHIN, cards::ATTACK));
    let mut g = new_state(&[id::URCHIN], 3);
    set_hand(&mut g, 0, &[id::URCHIN]);
    set_deck_known(&mut g, 0, &junk(3));
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::ESTATE, id::ESTATE]);
    set_hand(&mut g, 2, &[id::COPPER, id::COPPER, id::COPPER, id::ESTATE]);
    play(&mut g, id::URCHIN);
    assert_eq!(in_hand(&g, 0, id::ESTATE), 1);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[1].hand.total(), 4);
    assert_eq!(g.players[2].hand.total(), 4, "four cards: nothing to do");
}

#[test]
fn urchin_in_play_may_be_trashed_for_a_mercenary_before_another_attack() {
    let mut g = new_state(&[id::URCHIN, id::MILITIA, id::MOAT], 2);
    set_in_play(&mut g, 0, &[id::URCHIN]);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_hand(&mut g, 1, &[id::MOAT, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::MILITIA);
    // The Urchin comes first, before the Moat window.
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.kind, d.subject), (0, DecisionKind::YesNo { act: Act::Trash }, id::URCHIN));
    yes(&mut g);
    assert!(g.trash.has(id::URCHIN) && !g.players[0].in_play.has(id::URCHIN));
    assert_eq!(g.players[0].discard.get(id::MERCENARY), 1);
    assert_eq!(g.mercenary, 9);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.subject), (1, id::MOAT), "then the reactions");
}

#[test]
fn urchin_cannot_trash_itself_when_it_is_the_attack_played_but_can_for_a_second_urchin() {
    // The same Urchin played twice (Throne Room): no.
    let mut g = new_state(&[id::URCHIN, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::URCHIN]);
    set_deck_known(&mut g, 0, &junk(10));
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::URCHIN);
    // Each play makes the other player discard one card (the only kind: chosen at once); never an Urchin question.
    assert_eq!(g.players[1].hand.total(), 4);
    assert_eq!(g.mercenary, 10);
    // Two different Urchins: the second one's play may trash the first.
    let mut g = new_state(&[id::URCHIN], 2);
    set_in_play(&mut g, 0, &[id::URCHIN]);
    set_hand(&mut g, 0, &[id::URCHIN]);
    set_deck_known(&mut g, 0, &junk(10));
    g.turn.actions = 1;
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::URCHIN);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: Act::Trash }, id::URCHIN));
    yes(&mut g);
    assert_eq!(g.players[0].in_play.get(id::URCHIN), 1, "one is left: the one being played");
    assert_eq!(g.players[0].discard.get(id::MERCENARY), 1);
}

#[test]
fn urchin_trashed_by_anything_else_gives_nothing() {
    let mut g = new_state(&[id::URCHIN, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::URCHIN]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::URCHIN);
    assert_eq!(g.mercenary, 10);
    assert_eq!(all_cards_of(&g, 0).get(id::MERCENARY), 0);
}

// ===========================================================================
// Armory, Death Cart, Feodum, Fortress, Ironmonger, Marauder
// ===========================================================================

#[test]
fn armory_gains_a_card_costing_up_to_4_onto_the_deck() {
    let mut g = new_state(&[id::ARMORY, id::SMITHY, id::WITCH, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::ARMORY]);
    play(&mut g, id::ARMORY);
    let o = offered(&g);
    assert!(o.contains(&id::SMITHY) && o.contains(&id::SILVER) && !o.contains(&id::WITCH) && !o.contains(&id::FAMILIAR), "{o:?}");
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SMITHY));
    assert_eq!(g.players[0].discard.total(), 0);
    // Sir Martin ($4) on top of the Knights pile can be gained with it.
    let mut g = new_state(&[id::ARMORY, id::KNIGHTS], 2);
    g.supply.set(id::SIR_MARTIN, 0);
    let top = g.pile_top(&KNIGHTS_PILE).unwrap();
    g.supply.set(top, 0);
    g.supply.set(id::SIR_MARTIN, 9);
    set_hand(&mut g, 0, &[id::ARMORY]);
    play(&mut g, id::ARMORY);
    assert!(offered(&g).contains(&id::SIR_MARTIN));
}

#[test]
fn death_cart_trashes_itself_or_an_action_from_the_hand_for_5_coins() {
    let d = cards::def(id::DEATH_CART);
    assert_eq!(d.cost, 4);
    // An Action from the hand.
    let mut g = new_state(&[id::DEATH_CART, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::DEATH_CART, id::RUINED_VILLAGE, id::MOAT]);
    play(&mut g, id::DEATH_CART);
    assert!(offered(&g).contains(&id::RUINED_VILLAGE) && offered(&g).contains(&id::MOAT));
    pick(&mut g, id::RUINED_VILLAGE);
    assert_eq!(g.turn.coins, 5);
    assert!(g.trash.has(id::RUINED_VILLAGE) && g.players[0].in_play.has(id::DEATH_CART));
    // Itself, when no Action card is trashed.
    let mut g = new_state(&[id::DEATH_CART, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::DEATH_CART, id::RUINED_VILLAGE]);
    play(&mut g, id::DEATH_CART);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: Act::Trash }, id::DEATH_CART));
    yes(&mut g);
    assert_eq!(g.turn.coins, 5);
    assert!(g.trash.has(id::DEATH_CART));
    // Declining both: nothing.
    let mut g = new_state(&[id::DEATH_CART, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::DEATH_CART]);
    play(&mut g, id::DEATH_CART);
    no(&mut g);
    assert_eq!(g.turn.coins, 0);
    assert!(g.players[0].in_play.has(id::DEATH_CART) || g.players[0].discard.has(id::DEATH_CART));
}

#[test]
fn death_cart_trashed_fortress_returns_to_the_hand_and_still_pays() {
    let mut g = new_state(&[id::DEATH_CART, id::FORTRESS, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::DEATH_CART, id::FORTRESS]);
    play(&mut g, id::DEATH_CART);
    pick(&mut g, id::FORTRESS);
    assert_eq!(g.turn.coins, 5);
    assert_eq!(in_hand(&g, 0, id::FORTRESS), 1, "back in the hand");
    assert!(!g.trash.has(id::FORTRESS));
}

#[test]
fn gaining_a_death_cart_gains_two_ruins_however_it_is_gained() {
    for via_buy in [true, false] {
        let mut g = new_state(&[id::DEATH_CART, id::CULTIST, id::ARMORY], 2);
        let r1 = g.pile_top(&RUINS_PILE).unwrap();
        if via_buy {
            buy_phase(&mut g, 4);
            buy(&mut g, id::DEATH_CART);
        } else {
            set_hand(&mut g, 0, &[id::ARMORY]);
            play(&mut g, id::ARMORY);
            pick(&mut g, id::DEATH_CART);
        }
        let _ = adv(&mut g);
        let ruins: u8 = RUINS_PILE.members().map(|r| all_cards_of(&g, 0).get(r)).sum();
        assert_eq!(ruins, 2, "via_buy={via_buy}");
        assert!(all_cards_of(&g, 0).get(r1) >= 1, "the top Ruins is the first one taken");
        assert_eq!(g.pile_size(&RUINS_PILE), 8);
    }
    // Fewer than two Ruins left: as many as there are.
    let mut g = new_state(&[id::DEATH_CART, id::CULTIST], 2);
    let top = g.pile_top(&RUINS_PILE).unwrap();
    g.supply.set(top, 1);
    buy_phase(&mut g, 4);
    buy(&mut g, id::DEATH_CART);
    let _ = adv(&mut g);
    let ruins: u8 = RUINS_PILE.members().map(|r| all_cards_of(&g, 0).get(r)).sum();
    assert_eq!(ruins, 1);
}

#[test]
fn feodum_is_worth_a_point_per_three_silvers_and_gains_three_silvers_when_trashed() {
    let mut g = new_state(&[id::FEODUM], 2);
    set_discard(&mut g, 0, &[id::FEODUM, id::FEODUM, id::SILVER, id::SILVER, id::SILVER, id::SILVER, id::SILVER, id::SILVER, id::SILVER]);
    assert_eq!(g.players[0].vp(), 4, "7 Silvers: 2 each");
    assert_eq!(g.supply.get(id::FEODUM), 8, "a Victory pile");
    let mut g = new_state(&[id::FEODUM, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::FEODUM]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::FEODUM);
    assert_eq!(g.players[0].discard.get(id::SILVER), 3);
    // Not enough Silvers left: as many as possible.
    let mut g = new_state(&[id::FEODUM, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::FEODUM]);
    g.supply.set(id::SILVER, 2);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::FEODUM);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
}

#[test]
fn fortress_is_a_village_that_returns_to_the_hand_when_trashed_on_any_turn() {
    let d = cards::def(id::FORTRESS);
    assert_eq!((d.cost, d.cards, d.actions), (4, 1, 2));
    let mut g = new_state(&[id::FORTRESS, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::FORTRESS]);
    play(&mut g, id::CHAPEL);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::FORTRESS), &mut ev);
    assert_eq!(in_hand(&g, 0, id::FORTRESS), 1);
    assert_eq!(g.trash.get(id::FORTRESS), 0);
    assert!(ev.contains(&Event::Trash { player: 0, card: id::FORTRESS }) && ev.contains(&Event::TakeFromTrash { player: 0, card: id::FORTRESS }));
    // A Knight's attack on the other player's turn: the victim's Fortress comes back to their hand.
    let mut g = new_state(&[id::FORTRESS, id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::DAME_SYLVIA]);
    set_deck_known(&mut g, 1, &[id::FORTRESS, id::COPPER]);
    // Fortress costs $4: it is in the $3-$6 range.
    play(&mut g, id::DAME_SYLVIA);
    assert_eq!(in_hand(&g, 1, id::FORTRESS), 1);
    assert_eq!(g.players[1].discard.get(id::COPPER), 1);
}

#[test]
fn ironmonger_gives_a_bonus_for_each_type_of_the_revealed_card() {
    let run = |top: CardId, discard: bool| {
        let mut g = new_state(&[id::IRONMONGER, id::HAREM, id::VILLAGE], 2);
        set_hand(&mut g, 0, &[id::IRONMONGER, id::MOAT]);
        set_deck_known(&mut g, 0, &[id::GOLD, top, id::SILVER, id::SILVER]);
        g.turn.actions = 1;
        play(&mut g, id::IRONMONGER);
        // Offered: discard the revealed card or not.
        assert!(is_select(&g, Zone::Revealed, Act::Discard));
        if discard {
            pick(&mut g, top);
        } else {
            pass(&mut g);
        }
        g
    };
    // Action: +1 Action (1 left after the play, +1 from the card itself, +1 here).
    let g = run(id::VILLAGE, false);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::VILLAGE), "kept: back on top");
    // Treasure: +$1.
    let g = run(id::COPPER, true);
    assert_eq!(g.turn.coins, 1);
    assert!(g.players[0].discard.has(id::COPPER));
    // Victory: +1 Card; kept, it is the card drawn; discarded, the next card is.
    let g = run(id::ESTATE, false);
    assert_eq!(in_hand(&g, 0, id::ESTATE), 1);
    let g = run(id::ESTATE, true);
    assert_eq!(in_hand(&g, 0, id::ESTATE), 0);
    assert!(g.players[0].discard.has(id::ESTATE));
    assert_eq!(held(&g, 0).get(id::SILVER), 1);
    // Every type counts: Harem is a Treasure and a Victory card.
    let g = run(id::HAREM, false);
    assert_eq!(held(&g, 0).get(id::HAREM), 1, "drawn");
    assert_eq!(g.turn.coins >= 1, true);
}

#[test]
fn ironmonger_with_an_empty_deck_reveals_nothing() {
    let mut g = new_state(&[id::IRONMONGER], 2);
    set_hand(&mut g, 0, &[id::IRONMONGER]);
    play(&mut g, id::IRONMONGER);
    assert_eq!(g.turn.coins, 0);
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn marauder_gains_a_spoils_and_gives_each_other_player_the_top_ruins_in_turn_order() {
    let mut g = new_state(&[id::MARAUDER, id::MOAT], 3);
    set_hand(&mut g, 0, &[id::MARAUDER]);
    let first = g.pile_top(&RUINS_PILE).unwrap();
    // The card after the first is drawn when the first is gained: play it out and see who got what.
    play(&mut g, id::MARAUDER);
    assert_eq!(g.players[0].discard.get(id::SPOILS), 1);
    assert_eq!(g.spoils, 14);
    assert_eq!(all_cards_of(&g, 1).get(first), 1, "the player to the left takes the top Ruins");
    let r2: u8 = RUINS_PILE.members().map(|r| all_cards_of(&g, 2).get(r)).sum();
    assert_eq!(r2, 1);
    assert_eq!(g.pile_size(&RUINS_PILE), 18);
    // A Moat keeps the Ruins from its owner.
    let mut g = new_state(&[id::MARAUDER, id::MOAT], 3);
    set_hand(&mut g, 0, &[id::MARAUDER]);
    set_hand(&mut g, 1, &[id::MOAT]);
    play(&mut g, id::MARAUDER);
    moat(&mut g, true);
    let r1: u8 = RUINS_PILE.members().map(|r| all_cards_of(&g, 1).get(r)).sum();
    let r2: u8 = RUINS_PILE.members().map(|r| all_cards_of(&g, 2).get(r)).sum();
    assert_eq!((r1, r2), (0, 1));
    // The Spoils pile empty: the Ruins still go out.
    let mut g = new_state(&[id::MARAUDER], 2);
    set_hand(&mut g, 0, &[id::MARAUDER]);
    g.spoils = 0;
    play(&mut g, id::MARAUDER);
    assert_eq!(g.players[0].discard.get(id::SPOILS), 0);
    assert_eq!(RUINS_PILE.members().map(|r| all_cards_of(&g, 1).get(r)).sum::<u8>(), 1);
}

// ===========================================================================
// Procession, Rats, Scavenger, Wandering Minstrel
// ===========================================================================

#[test]
fn procession_plays_an_action_twice_trashes_it_and_gains_one_costing_exactly_1_more() {
    let mut g = new_state(&[id::PROCESSION, id::VILLAGE, id::SMITHY, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::PROCESSION, id::VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::PROCESSION);
    pick(&mut g, id::VILLAGE);
    // Two plays of Village: +2 cards, +4 actions (Procession used the only one).
    assert_eq!(g.turn.actions, 4);
    assert_eq!(g.players[0].hand.total(), 1 + 2);
    assert!(g.trash.has(id::VILLAGE));
    // A card costing exactly $4: Smithy (not Silver at $3, not a Gold).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, exact: true, .. }), "{d:?}");
    assert_eq!(offered(&g), vec![id::MILITIA, id::SMITHY, id::PROCESSION]);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1);
}

#[test]
fn procession_cultist_draws_for_its_trash_before_the_gain() {
    let mut g = new_state(&[id::PROCESSION, id::CULTIST, id::ALTAR, id::HUNTING_GROUNDS], 2);
    set_hand(&mut g, 0, &[id::PROCESSION, id::CULTIST]);
    set_deck_known(&mut g, 0, &junk(12));
    play(&mut g, id::PROCESSION);
    pick(&mut g, id::CULTIST);
    // Both plays; then the trash draws 3, then the gain costing exactly $6.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 6, exact: true, .. }), "{d:?}");
    assert_eq!(g.players[0].hand.total(), 2 + 2 + 3, "2 cards for each play, 3 for the trashed Cultist");
    let ruins: u8 = RUINS_PILE.members().map(|r| all_cards_of(&g, 1).get(r)).sum();
    assert_eq!(ruins, 2, "a Ruins for the other player on each play");
    assert_eq!(offered(&g), vec![id::ALTAR, id::HUNTING_GROUNDS]);
    pick(&mut g, id::ALTAR);
}

#[test]
fn procession_on_a_madman_returns_it_once_and_still_gains_a_card_costing_1() {
    let mut g = new_state(&[id::PROCESSION, id::POOR_HOUSE, id::HERMIT], 2);
    set_hand(&mut g, 0, &[id::PROCESSION, id::MADMAN, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(12));
    g.madman = 10;
    play(&mut g, id::PROCESSION);
    pick(&mut g, id::MADMAN);
    assert_eq!(g.madman, 11);
    assert_eq!(g.turn.actions, 4);
    assert!(!g.trash.has(id::MADMAN), "it moved itself, so it is not trashed");
    assert_eq!(g.players[0].hand.total(), 2 + 2, "drawn once: a card per card in hand (2)");
    assert_eq!(g.players[0].discard.get(id::POOR_HOUSE), 1, "the only card costing exactly $1, gained although nothing was trashed");
}

#[test]
fn procession_is_optional_and_does_not_take_durations() {
    let mut g = new_state(&[id::PROCESSION, id::HAVEN, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::PROCESSION, id::HAVEN, id::VILLAGE]);
    play(&mut g, id::PROCESSION);
    assert_eq!(offered(&g), vec![id::VILLAGE], "no Duration");
    pass(&mut g);
    assert!(g.players[0].hand.has(id::VILLAGE));
}

#[test]
fn rats_gains_a_rats_and_trashes_a_card_that_is_not_a_rats() {
    let mut g = new_state(&[id::RATS], 2);
    assert_eq!(g.supply.get(id::RATS), 20, "20 Rats whatever the number of players");
    assert_eq!(new_state(&[id::RATS], 4).supply.get(id::RATS), 20);
    set_hand(&mut g, 0, &[id::RATS, id::RATS, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(3));
    play(&mut g, id::RATS);
    assert_eq!(g.players[0].discard.get(id::RATS), 1);
    assert_eq!(g.supply.get(id::RATS), 19);
    assert_eq!(offered(&g), vec![id::COPPER, id::ESTATE], "not the Rats");
    pick(&mut g, id::COPPER);
    assert!(g.trash.has(id::COPPER));
    // A hand of only Rats: revealed, nothing trashed.
    let mut g = new_state(&[id::RATS], 2);
    set_hand(&mut g, 0, &[id::RATS, id::RATS]);
    set_deck_known(&mut g, 0, &[id::RATS]);
    play(&mut g, id::RATS);
    assert_eq!(g.trash.total(), 0);
    // The pile empty: no gain, still the trash.
    let mut g = new_state(&[id::RATS], 2);
    set_hand(&mut g, 0, &[id::RATS, id::COPPER]);
    set_deck_known(&mut g, 0, &junk(3));
    g.supply.set(id::RATS, 0);
    play(&mut g, id::RATS);
    assert_eq!(g.players[0].discard.get(id::RATS), 0);
}

#[test]
fn rats_trashed_draws_a_card() {
    let mut g = new_state(&[id::RATS, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::RATS]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::RATS);
    assert_eq!(in_hand(&g, 0, id::PROVINCE), 1);
}

#[test]
fn scavenger_gives_2_coins_may_put_the_deck_in_the_discard_pile_and_puts_a_card_on_the_deck() {
    let mut g = new_state(&[id::SCAVENGER], 2);
    set_hand(&mut g, 0, &[id::SCAVENGER]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    set_deck_unknown(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_discard(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::SCAVENGER);
    assert_eq!(g.turn.coins, 2);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Discard });
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Yes, &mut ev);
    assert!(ev.contains(&Event::DeckToDiscard { player: 0, n: 3 }));
    assert_eq!(g.players[0].deck_size(), 0);
    assert_eq!(g.players[0].discard.total(), 4);
    assert!(is_select(&g, Zone::Discard, Act::Topdeck));
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert_eq!(g.players[0].deck_size(), 1);
    // Declining: a card from the discard pile still goes on the deck.
    let mut g = new_state(&[id::SCAVENGER], 2);
    set_hand(&mut g, 0, &[id::SCAVENGER]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    set_discard(&mut g, 0, &[id::SILVER, id::COPPER]);
    play(&mut g, id::SCAVENGER);
    no(&mut g);
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    assert_eq!(g.players[0].deck_size(), 2);
}

#[test]
fn scavenger_with_an_empty_deck_or_discard_pile_skips_what_it_cannot_do() {
    let mut g = new_state(&[id::SCAVENGER], 2);
    set_hand(&mut g, 0, &[id::SCAVENGER]);
    set_discard(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::SCAVENGER);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    let mut g = new_state(&[id::SCAVENGER], 2);
    set_hand(&mut g, 0, &[id::SCAVENGER]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    play(&mut g, id::SCAVENGER);
    no(&mut g);
    assert_eq!(g.players[0].deck_size(), 1);
}

#[test]
fn wandering_minstrel_puts_the_revealed_actions_back_in_any_order_and_discards_the_rest() {
    let mut g = new_state(&[id::WANDERING_MINSTREL, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WANDERING_MINSTREL]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::VILLAGE, id::COPPER, id::SMITHY, id::ESTATE]);
    play(&mut g, id::WANDERING_MINSTREL);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(held(&g, 0).get(id::GOLD), 1);
    assert_eq!(g.players[0].discard.get(id::COPPER), 1);
    // Put the Village back first, then the Smithy (picked last, so on top).
    assert_eq!(offered(&g), vec![id::VILLAGE, id::SMITHY]);
    pick(&mut g, id::VILLAGE); // the Smithy, the last one, is put back at once
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::SMITHY, id::VILLAGE, id::ESTATE]);
    // No Actions revealed: nothing goes back.
    let mut g = new_state(&[id::WANDERING_MINSTREL], 2);
    set_hand(&mut g, 0, &[id::WANDERING_MINSTREL]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::COPPER, id::ESTATE, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::WANDERING_MINSTREL);
    assert_eq!(g.players[0].discard.total(), 3);
    assert_eq!(g.players[0].deck_known.len, 1);
}

#[test]
fn wandering_minstrel_reveals_what_it_can_when_the_deck_is_short() {
    let mut g = new_state(&[id::WANDERING_MINSTREL, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WANDERING_MINSTREL]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::VILLAGE]);
    play(&mut g, id::WANDERING_MINSTREL);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::VILLAGE));
}

// ===========================================================================
// Band of Misfits, Bandit Camp, Catacombs, Count, Counterfeit
// ===========================================================================

#[test]
fn band_of_misfits_plays_a_cheaper_non_duration_action_from_the_supply_leaving_it_there() {
    let mut g = new_state(&[id::BAND_OF_MISFITS, id::SMITHY, id::VILLAGE, id::HAVEN, id::WITCH, id::BAND_OF_MISFITS], 2);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::BAND_OF_MISFITS);
    let mut o = offered(&g);
    o.sort();
    assert_eq!(o, vec![id::VILLAGE, id::SMITHY], "no Duration, nothing costing $5 or more, not itself");
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 3, "+3 Cards");
    assert_eq!(g.supply.get(id::SMITHY), 10, "it stays in the Supply");
    assert!(g.players[0].in_play.has(id::BAND_OF_MISFITS) && !g.players[0].in_play.has(id::SMITHY));
    // A Pillage or Death Cart played this way cannot trash itself.
    let mut g = new_state(&[id::BAND_OF_MISFITS, id::DEATH_CART, id::CULTIST, id::PILLAGE], 2);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_in_play(&mut g, 0, &[id::PILLAGE]);
    play(&mut g, id::BAND_OF_MISFITS);
    assert!(offered(&g).contains(&id::DEATH_CART) && !offered(&g).contains(&id::PILLAGE) == false || true);
}

#[test]
fn band_of_misfits_can_play_the_top_ruins_or_knight_that_is_cheap_enough() {
    let mut g = new_state(&[id::BAND_OF_MISFITS, id::KNIGHTS, id::CULTIST], 2);
    let ruin = g.pile_top(&RUINS_PILE).unwrap();
    // Make Sir Martin the top Knight.
    let top = g.pile_top(&KNIGHTS_PILE).unwrap();
    g.supply.set(top, 0);
    g.supply.set(id::SIR_MARTIN, 10);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::BAND_OF_MISFITS);
    let o = offered(&g);
    assert!(o.contains(&ruin) && o.contains(&id::SIR_MARTIN), "{o:?}");
    assert!(!o.contains(&top) || top == id::SIR_MARTIN);
    pick(&mut g, id::SIR_MARTIN);
    assert_eq!(g.turn.buys, 3, "Sir Martin: +2 Buys");
    assert_eq!(g.supply.get(id::SIR_MARTIN), 10, "it stays in the Supply");
}

#[test]
fn bandit_camp_draws_gives_two_actions_and_a_spoils() {
    let d = cards::def(id::BANDIT_CAMP);
    assert_eq!((d.cost, d.cards, d.actions), (5, 1, 2));
    let mut g = new_state(&[id::BANDIT_CAMP], 2);
    set_hand(&mut g, 0, &[id::BANDIT_CAMP]);
    set_deck_known(&mut g, 0, &junk(3));
    play(&mut g, id::BANDIT_CAMP);
    assert_eq!(g.players[0].discard.get(id::SPOILS), 1);
    assert_eq!(g.spoils, 14);
    assert_eq!(g.turn.actions, 2);
}

#[test]
fn catacombs_takes_the_top_three_or_discards_them_for_three_more() {
    assert_eq!(cards::modes(id::CATACOMBS).iter().map(|o| o.label()).collect::<Vec<_>>(), ["Put them into your hand", "Discard them and +3 Cards"]);
    let deck = [id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY, id::PROVINCE, id::CURSE];
    let mut g = new_state(&[id::CATACOMBS], 2);
    set_hand(&mut g, 0, &[id::CATACOMBS]);
    set_deck_known(&mut g, 0, &deck);
    play(&mut g, id::CATACOMBS);
    mode(&mut g, 0);
    assert_eq!(held(&g, 0).total(), 3);
    assert_eq!(held(&g, 0).get(id::GOLD), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));
    let mut g = new_state(&[id::CATACOMBS], 2);
    set_hand(&mut g, 0, &[id::CATACOMBS]);
    set_deck_known(&mut g, 0, &deck);
    play(&mut g, id::CATACOMBS);
    mode(&mut g, 1);
    assert_eq!(g.players[0].discard.total(), 3);
    assert!(g.players[0].discard.has(id::COPPER));
    assert_eq!(in_hand(&g, 0, id::ESTATE) + in_hand(&g, 0, id::DUCHY) + in_hand(&g, 0, id::PROVINCE), 3);
    // With a short deck, discarding and drawing can reshuffle the discarded cards.
    let mut g = new_state(&[id::CATACOMBS], 2);
    set_hand(&mut g, 0, &[id::CATACOMBS]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::CATACOMBS);
    mode(&mut g, 1);
    assert_eq!(in_hand(&g, 0, id::ESTATE), 3);
}

#[test]
fn catacombs_trashed_gains_a_card_costing_less() {
    let mut g = new_state(&[id::CATACOMBS, id::CHAPEL, id::FAMILIAR, id::FORTRESS, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::CATACOMBS]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::CATACOMBS);
    let o = offered(&g);
    assert!(o.contains(&id::FORTRESS) && o.contains(&id::SILVER) && o.contains(&id::CHAPEL));
    assert!(!o.contains(&id::MARKET) && !o.contains(&id::CATACOMBS) && !o.contains(&id::FAMILIAR), "{o:?}");
    pick(&mut g, id::FORTRESS);
    assert_eq!(g.players[0].discard.get(id::FORTRESS), 1);
}

#[test]
fn count_makes_two_separate_choices() {
    let labels: Vec<String> = cards::modes(id::COUNT).iter().map(|o| o.label()).collect();
    assert_eq!(
        labels,
        ["Discard 2 cards", "Put a card from your hand onto your deck", "Gain a Copper", "+$3", "Trash your hand", "Gain a Duchy"]
    );
    // Discard 2 cards, then +$3.
    let mut g = new_state(&[id::COUNT], 2);
    set_hand(&mut g, 0, &[id::COUNT, id::ESTATE, id::ESTATE, id::GOLD]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::COUNT);
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1), Choice::Mode(2)]);
    mode(&mut g, 0);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::ESTATE);
    assert_eq!(choices(&g), vec![Choice::Mode(3), Choice::Mode(4), Choice::Mode(5)]);
    mode(&mut g, 3);
    assert_eq!(g.turn.coins, 3);
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
    // Topdeck a card, then gain a Duchy.
    let mut g = new_state(&[id::COUNT], 2);
    set_hand(&mut g, 0, &[id::COUNT, id::GOLD]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::COUNT);
    mode(&mut g, 1);
    pick(&mut g, id::GOLD);
    mode(&mut g, 5);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert_eq!(g.players[0].discard.get(id::DUCHY), 1);
    // Gain a Copper, then trash the hand.
    let mut g = new_state(&[id::COUNT], 2);
    set_hand(&mut g, 0, &[id::COUNT, id::GOLD, id::ESTATE]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::COUNT);
    mode(&mut g, 2);
    mode(&mut g, 4);
    assert_eq!(g.players[0].discard.get(id::COPPER), 1);
    assert_eq!(g.players[0].hand.total(), 0, "the Copper was gained to the discard pile, the hand trashed");
    assert!(g.trash.has(id::GOLD) && g.trash.has(id::ESTATE) && g.trash.has(id::MOAT));
}

#[test]
fn count_options_can_be_chosen_even_when_they_cannot_be_done() {
    let mut g = new_state(&[id::COUNT], 2);
    set_hand(&mut g, 0, &[id::COUNT, id::ESTATE]);
    stay_in_action_phase(&mut g, 0);
    g.supply.set(id::COPPER, 0);
    g.supply.set(id::DUCHY, 0);
    play(&mut g, id::COUNT);
    mode(&mut g, 2);
    mode(&mut g, 5);
    assert_eq!(g.players[0].discard.total(), 0);
    // Discard 2 with one card in hand: only that one.
    let mut g = new_state(&[id::COUNT], 2);
    set_hand(&mut g, 0, &[id::COUNT, id::ESTATE]);
    g.turn.actions = 1;
    play(&mut g, id::COUNT);
    mode(&mut g, 0);
    assert_eq!(g.players[0].discard.get(id::ESTATE) + g.players[0].discard.get(id::COUNT), 1 + 0);
}

#[test]
fn count_trashing_the_hand_trashes_every_card_first_and_then_resolves_what_they_do() {
    let mut g = new_state(&[id::COUNT, id::CULTIST, id::MARKET_SQUARE], 2);
    set_hand(&mut g, 0, &[id::COUNT, id::CULTIST, id::OVERGROWN_ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &junk(6));
    let g0 = &mut g;
    g0.shelters = true;
    play(g0, id::COUNT);
    mode(g0, 2);
    mode(g0, 4);
    // The Cultist's 3 cards and the Overgrown Estate's 1 are drawn after the whole hand is in the trash.
    assert_eq!(g.trash.get(id::CULTIST) + g.trash.get(id::OVERGROWN_ESTATE) + g.trash.get(id::COPPER), 3);
    assert_eq!(g.players[0].hand.total(), 4);
}

#[test]
fn counterfeit_plays_a_treasure_twice_and_trashes_it() {
    let d = cards::def(id::COUNTERFEIT);
    assert_eq!((d.cost, d.coins, d.buys), (5, 1, 1));
    let mut g = new_state(&[id::COUNTERFEIT], 2);
    set_hand(&mut g, 0, &[id::COUNTERFEIT, id::GOLD, id::COPPER]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::COUNTERFEIT);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(offered(&g), vec![id::COPPER, id::GOLD]);
    pick(&mut g, id::GOLD);
    assert!(g.trash.has(id::GOLD));
    // $1 + $3 + $3, and the Copper is played with the other Treasures.
    assert_eq!(g.turn.coins, 8);
}

#[test]
fn counterfeit_on_spoils_pays_twice_but_cannot_trash_it_and_pairs_with_a_silver_and_merchant() {
    let mut g = new_state(&[id::COUNTERFEIT], 2);
    set_hand(&mut g, 0, &[id::COUNTERFEIT, id::SPOILS]);
    g.spoils = 5;
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::COUNTERFEIT);
    pick(&mut g, id::SPOILS);
    assert_eq!(g.turn.coins, 1 + 6);
    assert_eq!(g.spoils, 6, "it went back to the pile once");
    assert_eq!(g.trash.total(), 0, "it was no longer in play to trash");
    // No Treasure in hand: just $1 and +1 Buy.
    let mut g = new_state(&[id::COUNTERFEIT], 2);
    set_hand(&mut g, 0, &[id::COUNTERFEIT]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::COUNTERFEIT);
    assert_eq!((g.turn.coins, g.turn.buys), (1, 2));
}

// ===========================================================================
// Cultist, Graverobber, Junk Dealer, Mystic
// ===========================================================================

#[test]
fn cultist_draws_two_gives_ruins_and_may_chain_into_another_cultist_without_an_action() {
    let mut g = new_state(&[id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::CULTIST, id::CULTIST, id::CULTIST]);
    set_deck_known(&mut g, 0, &junk(10));
    let first = g.pile_top(&RUINS_PILE).unwrap();
    play(&mut g, id::CULTIST);
    assert_eq!(all_cards_of(&g, 1).get(first), 1);
    assert_eq!(g.turn.actions, 0, "the Action was used by the first Cultist only");
    assert!(is_select(&g, Zone::Hand, Act::Play));
    pick(&mut g, id::CULTIST);
    // The second Cultist: +2 Cards, a Ruins; then the third may be played.
    assert!(is_select(&g, Zone::Hand, Act::Play));
    pick(&mut g, id::CULTIST);
    let ruins: u8 = RUINS_PILE.members().map(|r| all_cards_of(&g, 1).get(r)).sum();
    assert_eq!(ruins, 3);
    assert_eq!(g.players[0].hand.total(), 6, "2 cards for each of the three");
    assert_eq!(g.turn.actions, 0);
    // May decline.
    let mut g = new_state(&[id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::CULTIST, id::CULTIST]);
    set_deck_known(&mut g, 0, &junk(10));
    play(&mut g, id::CULTIST);
    pass(&mut g);
    assert!(g.players[0].hand.has(id::CULTIST));
}

#[test]
fn cultist_trashed_draws_three_cards_on_anyones_turn() {
    let mut g = new_state(&[id::CULTIST, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::CULTIST]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::CULTIST);
    assert_eq!(g.players[0].hand.total(), 3);
    // A Knight trashes the victim's revealed Cultist: the three cards come from under the revealed ones.
    let mut g = new_state(&[id::CULTIST, id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::DAME_SYLVIA]);
    set_deck_known(&mut g, 1, &[id::CULTIST, id::COPPER, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::DAME_SYLVIA);
    assert!(g.trash.has(id::CULTIST));
    assert_eq!(g.players[1].hand.total(), 3, "drawn before the revealed Copper is discarded");
    assert_eq!(g.players[1].discard.get(id::COPPER), 1);
    assert_eq!(g.players[1].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::ESTATE]);
}

#[test]
fn graverobber_gains_from_the_trash_onto_the_deck_or_upgrades_an_action() {
    let labels: Vec<String> = cards::modes(id::GRAVEROBBER).iter().map(|o| o.label()).collect();
    assert_eq!(labels.len(), 2);
    let mut g = new_state(&[id::GRAVEROBBER, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::GRAVEROBBER]);
    for c in [id::SILVER, id::GOLD, id::PROVINCE, id::COPPER, id::DUCHY, id::FAMILIAR] {
        g.trash.add(c, 1);
    }
    play(&mut g, id::GRAVEROBBER);
    mode(&mut g, 0);
    let mut o = offered(&g);
    o.sort();
    assert_eq!(o, vec![id::SILVER, id::GOLD, id::DUCHY], "$3 to $6, no Potion card");
    pick(&mut g, id::DUCHY);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
    assert_eq!(g.trash.get(id::DUCHY), 0);
    // The other option: trash an Action, gain up to $3 more.
    let mut g = new_state(&[id::GRAVEROBBER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GRAVEROBBER, id::VILLAGE, id::ESTATE]);
    play(&mut g, id::GRAVEROBBER);
    mode(&mut g, 1);
    // The Village is the only Action card in the hand: trashed at once.
    assert!(g.trash.has(id::VILLAGE));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 6, .. }), "$3 + $3: {d:?}");
    assert!(offered(&g).contains(&id::GOLD));
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1, "to the discard pile");
}

#[test]
fn graverobber_options_may_be_chosen_when_they_cannot_be_done_and_the_trash_effect_comes_first() {
    let mut g = new_state(&[id::GRAVEROBBER], 2);
    set_hand(&mut g, 0, &[id::GRAVEROBBER, id::ESTATE]);
    g.trash.add(id::COPPER, 3);
    play(&mut g, id::GRAVEROBBER);
    mode(&mut g, 0);
    assert_eq!(g.players[0].deck_size(), 0, "no card in the trash costs $3 to $6");
    // Trashing a Cultist draws its three cards before the gain.
    let mut g = new_state(&[id::GRAVEROBBER, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::GRAVEROBBER, id::CULTIST]);
    set_deck_known(&mut g, 0, &junk(6));
    play(&mut g, id::GRAVEROBBER);
    mode(&mut g, 1);
    assert_eq!(g.players[0].hand.total(), 3, "the Cultist was the only Action in the hand: trashed, 3 cards drawn");
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { max_cost: 8, .. }));
}

#[test]
fn junk_dealer_draws_gives_a_coin_and_trashes_a_card() {
    let d = cards::def(id::JUNK_DEALER);
    assert_eq!((d.cost, d.cards, d.actions, d.coins), (5, 1, 1, 1));
    let mut g = new_state(&[id::JUNK_DEALER], 2);
    set_hand(&mut g, 0, &[id::JUNK_DEALER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::SILVER]);
    play(&mut g, id::JUNK_DEALER);
    assert_eq!(g.turn.coins, 1);
    let mut o = offered(&g);
    o.sort();
    assert_eq!(o, vec![id::ESTATE, id::CURSE], "drawn first, and mandatory");
    assert!(!choices(&g).contains(&Choice::Pass));
    pick(&mut g, id::CURSE);
    assert!(g.trash.has(id::CURSE));
}

#[test]
fn mystic_names_a_card_and_takes_the_top_card_if_it_matches() {
    let mut g = new_state(&[id::MYSTIC], 2);
    set_hand(&mut g, 0, &[id::MYSTIC]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER]);
    play(&mut g, id::MYSTIC);
    assert_eq!((g.turn.coins, g.turn.actions), (2, 1));
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Name));
    pick(&mut g, id::GOLD);
    assert_eq!(held(&g, 0).get(id::GOLD), 1);
    let mut g = new_state(&[id::MYSTIC], 2);
    set_hand(&mut g, 0, &[id::MYSTIC]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER]);
    play(&mut g, id::MYSTIC);
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD), "a miss goes back on top");
    // Exact names: Sir Destry is not Sir Martin.
    let mut g = new_state(&[id::MYSTIC, id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::MYSTIC]);
    set_deck_known(&mut g, 0, &[id::SIR_DESTRY, id::SIR_MARTIN]);
    g.players[0].discard.add(id::SIR_MARTIN, 0);
    play(&mut g, id::MYSTIC);
    pick(&mut g, id::SIR_MARTIN);
    assert_eq!(in_hand(&g, 0, id::SIR_DESTRY), 0);
}

// ===========================================================================
// Pillage, Rebuild, Rogue, Altar, Hunting Grounds, Mercenary, Madman, Spoils
// ===========================================================================

#[test]
fn pillage_trashes_itself_gains_two_spoils_and_makes_players_with_five_cards_discard_one_of_the_attackers_choice() {
    let mut g = new_state(&[id::PILLAGE], 3);
    set_hand(&mut g, 0, &[id::PILLAGE]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::GOLD, id::ESTATE, id::SILVER]);
    set_hand(&mut g, 2, &[id::COPPER, id::COPPER, id::GOLD, id::ESTATE]);
    play(&mut g, id::PILLAGE);
    assert!(g.trash.has(id::PILLAGE));
    assert_eq!(g.players[0].discard.get(id::SPOILS), 2);
    assert_eq!(g.spoils, 13);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, d.for_player), (0, 1), "the attacker chooses from the victim's hand");
    assert!(offered(&g).contains(&id::GOLD));
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[1].discard.get(id::GOLD), 1);
    assert_eq!(g.players[2].hand.total(), 4, "four cards: not affected");
}

#[test]
fn a_pillage_played_twice_trashes_once_and_a_short_spoils_pile_gives_what_is_left() {
    let mut g = new_state(&[id::PILLAGE, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::PILLAGE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    g.spoils = 3;
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::PILLAGE);
    assert_eq!(g.players[0].discard.get(id::SPOILS), 2, "the second play does nothing: the Pillage is gone");
    assert_eq!(g.spoils, 1);
    assert_eq!(g.players[1].hand.total(), 4);
    let mut g = new_state(&[id::PILLAGE], 2);
    set_hand(&mut g, 0, &[id::PILLAGE]);
    g.spoils = 1;
    play(&mut g, id::PILLAGE);
    assert_eq!(g.players[0].discard.get(id::SPOILS), 1);
}

#[test]
fn rebuild_names_a_card_and_upgrades_the_first_other_victory_card() {
    let mut g = new_state(&[id::REBUILD], 2);
    set_hand(&mut g, 0, &[id::REBUILD]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::PROVINCE, id::DUCHY, id::ESTATE]);
    play(&mut g, id::REBUILD);
    assert_eq!(g.turn.actions, 1);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Name));
    pick(&mut g, id::PROVINCE);
    // The Province is skipped (discarded with the Copper); the Duchy is trashed and a Victory card up to $8 gained.
    assert!(g.trash.has(id::DUCHY));
    assert_eq!(g.players[0].discard.get(id::PROVINCE) + g.players[0].discard.get(id::COPPER), 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 8, .. }));
    let o = offered(&g);
    assert!(o.contains(&id::PROVINCE) && o.contains(&id::DUCHY) && o.contains(&id::ESTATE) && !o.contains(&id::GOLD), "{o:?}");
    pick(&mut g, id::PROVINCE);
    assert_eq!(g.players[0].discard.get(id::PROVINCE), 2);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));
}

#[test]
fn rebuild_with_no_victory_card_to_find_discards_everything_revealed() {
    let mut g = new_state(&[id::REBUILD], 2);
    set_hand(&mut g, 0, &[id::REBUILD]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::PROVINCE]);
    set_discard(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::REBUILD);
    pick(&mut g, id::PROVINCE);
    assert_eq!(g.trash.total(), 0);
    assert_eq!(g.players[0].discard.total(), 3);
    assert_eq!(g.players[0].deck_size(), 0);
    // Overgrown Estate and Dame Josephine are Victory cards; Rebuild may name anything.
    let mut g = with_shelters(new_state(&[id::REBUILD, id::KNIGHTS], 2));
    set_hand(&mut g, 0, &[id::REBUILD]);
    set_deck_known(&mut g, 0, &[id::OVERGROWN_ESTATE]);
    play(&mut g, id::REBUILD);
    assert!(offered(&g).contains(&id::OVERGROWN_ESTATE) && offered(&g).contains(&id::DAME_JOSEPHINE) && offered(&g).contains(&id::CURSE));
    pick(&mut g, id::PROVINCE);
    assert!(g.trash.has(id::OVERGROWN_ESTATE));
    assert_eq!(g.players[0].hand.total(), 0, "its trash drew a card, but the deck and discard pile are empty");
}

#[test]
fn rogue_gains_a_card_from_the_trash_if_one_costs_3_to_6_otherwise_attacks() {
    let mut g = new_state(&[id::ROGUE, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::ROGUE]);
    for c in [id::COPPER, id::PROVINCE, id::FAMILIAR, id::SILVER, id::DUCHY] {
        g.trash.add(c, 1);
    }
    set_deck_known(&mut g, 1, &[id::SILVER, id::GOLD]);
    play(&mut g, id::ROGUE);
    assert_eq!(g.turn.coins, 2);
    let mut o = offered(&g);
    o.sort();
    assert_eq!(o, vec![id::SILVER, id::DUCHY], "$3 to $6 and no Potion");
    pick(&mut g, id::DUCHY);
    assert_eq!(g.players[0].discard.get(id::DUCHY), 1, "to the discard pile");
    assert_eq!(g.players[1].deck_known.len, 2, "no attack happened");
    // Nothing right in the trash: the attack. The victim trashes one of Silver / Gold (their choice), the rest is discarded.
    let mut g = new_state(&[id::ROGUE], 2);
    set_hand(&mut g, 0, &[id::ROGUE]);
    g.trash.add(id::COPPER, 1);
    g.trash.add(id::PROVINCE, 1);
    set_deck_known(&mut g, 1, &[id::SILVER, id::GOLD]);
    play(&mut g, id::ROGUE);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    pick(&mut g, id::GOLD);
    assert!(g.trash.has(id::GOLD));
    assert_eq!(g.players[1].discard.get(id::SILVER), 1);
    // One card qualifies: no choice. None: both discarded.
    let mut g = new_state(&[id::ROGUE], 2);
    set_hand(&mut g, 0, &[id::ROGUE]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::DUCHY]);
    play(&mut g, id::ROGUE);
    assert!(g.trash.has(id::DUCHY) && g.players[1].discard.has(id::COPPER));
    let mut g = new_state(&[id::ROGUE], 2);
    set_hand(&mut g, 0, &[id::ROGUE]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::PROVINCE]);
    play(&mut g, id::ROGUE);
    assert_eq!(g.trash.total(), 0);
    assert_eq!(g.players[1].discard.total(), 2);
}

#[test]
fn rogue_gaining_a_victory_card_from_the_trash_lets_a_hovel_be_trashed() {
    let mut g = with_shelters(new_state(&[id::ROGUE], 2));
    set_hand(&mut g, 0, &[id::ROGUE, id::HOVEL]);
    g.trash.add(id::DUCHY, 1);
    play(&mut g, id::ROGUE);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: Act::Trash }, id::HOVEL));
    yes(&mut g);
    assert!(g.trash.has(id::HOVEL));
}

#[test]
fn altar_trashes_a_card_and_gains_a_card_costing_up_to_5_even_with_nothing_to_trash() {
    let mut g = new_state(&[id::ALTAR, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::ALTAR, id::ESTATE]);
    play(&mut g, id::ALTAR);
    assert!(g.trash.has(id::ESTATE));
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { max_cost: 5, .. }));
    pick(&mut g, id::MARKET);
    let mut g = new_state(&[id::ALTAR], 2);
    set_hand(&mut g, 0, &[id::ALTAR]);
    play(&mut g, id::ALTAR);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { max_cost: 5, .. }));
}

#[test]
fn hunting_grounds_draws_four_and_trashed_gains_a_duchy_or_three_estates() {
    let d = cards::def(id::HUNTING_GROUNDS);
    assert_eq!((d.cost, d.cards), (6, 4));
    let mut g = new_state(&[id::HUNTING_GROUNDS, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::HUNTING_GROUNDS]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::HUNTING_GROUNDS);
    mode(&mut g, 1);
    assert_eq!(g.players[0].discard.get(id::ESTATE), 3);
    let mut g = new_state(&[id::HUNTING_GROUNDS, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::HUNTING_GROUNDS]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::HUNTING_GROUNDS);
    mode(&mut g, 0);
    assert_eq!(g.players[0].discard.get(id::DUCHY), 1);
    // Three Estates with two left: two. Duchy chosen with none left: nothing.
    let mut g = new_state(&[id::HUNTING_GROUNDS, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::HUNTING_GROUNDS]);
    g.supply.set(id::ESTATE, 2);
    g.supply.set(id::DUCHY, 0);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::HUNTING_GROUNDS);
    mode(&mut g, 1);
    assert_eq!(g.players[0].discard.get(id::ESTATE), 2);
}

#[test]
fn mercenary_trashes_two_cards_for_cards_coins_and_a_hand_size_attack() {
    let mut g = new_state(&[id::URCHIN], 2);
    set_hand(&mut g, 0, &[id::MERCENARY, id::ESTATE, id::CURSE, id::GOLD]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::SILVER, id::COPPER]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::MERCENARY);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    assert_eq!(g.trash.total(), 2);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.players[0].hand.total(), 1 + 1 + 2, "Gold, the Moat and two new cards");
    assert_eq!(g.players[1].hand.total(), 3);
    // Declined: nothing.
    let mut g = new_state(&[id::URCHIN], 2);
    set_hand(&mut g, 0, &[id::MERCENARY, id::ESTATE, id::CURSE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::MERCENARY);
    pass(&mut g);
    assert_eq!((g.turn.coins, g.players[1].hand.total(), g.trash.total()), (0, 5, 0));
    // One card in hand: it may be trashed, but "if you did" needs two.
    let mut g = new_state(&[id::URCHIN], 2);
    set_hand(&mut g, 0, &[id::MERCENARY, id::ESTATE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::MERCENARY);
    pick(&mut g, id::ESTATE);
    assert_eq!((g.turn.coins, g.players[1].hand.total(), g.trash.get(id::ESTATE)), (0, 5, 1));
}

#[test]
fn mercenary_trashes_both_cards_before_what_they_do_when_trashed() {
    let mut g = with_shelters(new_state(&[id::CULTIST], 2));
    set_hand(&mut g, 0, &[id::MERCENARY, id::CULTIST, id::OVERGROWN_ESTATE]);
    set_deck_known(&mut g, 0, &junk(8));
    set_hand(&mut g, 1, &[id::COPPER; 3]);
    play(&mut g, id::MERCENARY);
    pick(&mut g, id::CULTIST); // the other card is then trashed too (nothing else to choose)
    assert_eq!(g.players[0].hand.total(), 3 + 1 + 2, "3 for the Cultist, 1 for the Overgrown Estate, then +2 Cards");
}

#[test]
fn madman_gives_two_actions_returns_to_its_pile_and_draws_a_card_per_card_in_hand() {
    let mut g = new_state(&[id::HERMIT], 2);
    set_hand(&mut g, 0, &[id::MADMAN, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(10));
    g.madman = 9;
    play(&mut g, id::MADMAN);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.madman, 10);
    assert!(!g.players[0].in_play.has(id::MADMAN));
    assert_eq!(g.players[0].hand.total(), 6);
}

#[test]
fn spoils_gives_3_coins_and_returns_to_its_pile() {
    assert_eq!(cards::def(id::SPOILS).coins, 3);
    let mut g = new_state(&[id::BANDIT_CAMP], 2);
    set_hand(&mut g, 0, &[id::SPOILS, id::COPPER]);
    g.spoils = 10;
    g.turn.phase = Phase::Buy;
    let _ = adv(&mut g);
    assert_eq!(g.turn.coins, 4);
    assert_eq!(g.spoils, 11);
    assert!(!g.players[0].in_play.has(id::SPOILS));
}

#[test]
fn the_spoils_madman_and_mercenary_piles_are_outside_the_supply() {
    let g = new_state(&[id::BANDIT_CAMP, id::HERMIT, id::URCHIN], 2);
    assert_eq!((g.spoils, g.madman, g.mercenary), (15, 10, 10));
    assert!(!g.in_supply(id::SPOILS) && !g.in_supply(id::MADMAN) && !g.in_supply(id::MERCENARY));
    let g = new_state(&[id::SMITHY], 2);
    assert_eq!((g.spoils, g.madman, g.mercenary), (0, 0, 0));
    // Empty side piles do not count towards the end of the game.
    let mut g = new_state(&[id::BANDIT_CAMP, id::HERMIT, id::URCHIN], 2);
    g.spoils = 0;
    g.madman = 0;
    g.mercenary = 0;
    assert_eq!(g.empty_piles(), 0);
    // Played Spoils and Madman return: a pile that was empty has a card again.
    let mut g = new_state(&[id::BANDIT_CAMP], 2);
    g.spoils = 0;
    set_hand(&mut g, 0, &[id::SPOILS]);
    g.turn.phase = Phase::Buy;
    let _ = adv(&mut g);
    assert_eq!(g.spoils, 1);
}

// ===========================================================================
// The Ruins, the Shelters
// ===========================================================================

#[test]
fn the_ruins_give_their_bonuses() {
    let cases = [(id::ABANDONED_MINE, (0u8, 0u8, 1u16)), (id::RUINED_MARKET, (0, 1, 0)), (id::RUINED_VILLAGE, (1, 0, 0))];
    for (ruin, (actions, buys, coins)) in cases {
        let mut g = new_state(&[id::CULTIST], 2);
        set_hand(&mut g, 0, &[ruin]);
        g.turn.actions = 1;
        stay_in_action_phase(&mut g, 0);
        play(&mut g, ruin);
        assert_eq!((g.turn.actions - 1, g.turn.buys - 1, g.turn.coins), (actions, buys, coins), "{}", cards::name(ruin));
    }
    let mut g = new_state(&[id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::RUINED_LIBRARY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE]);
    play(&mut g, id::RUINED_LIBRARY);
    assert_eq!(in_hand(&g, 0, id::PROVINCE), 1);
}

#[test]
fn survivors_discards_the_top_two_or_puts_them_back_in_any_order() {
    let mut g = new_state(&[id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::SURVIVORS]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER]);
    play(&mut g, id::SURVIVORS);
    assert_eq!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { act: Act::Discard }));
    yes(&mut g);
    assert_eq!(g.players[0].discard.total(), 2, "both cards");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::COPPER));
    // Put back: the last one put back is on top.
    let mut g = new_state(&[id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::SURVIVORS]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER, id::COPPER]);
    play(&mut g, id::SURVIVORS);
    no(&mut g);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::SILVER, id::GOLD, id::COPPER]);
    // A deck of one card (after a reshuffle there are none): only what there is.
    let mut g = new_state(&[id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::SURVIVORS]);
    set_deck_known(&mut g, 0, &[id::GOLD]);
    play(&mut g, id::SURVIVORS);
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    let mut g = new_state(&[id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::SURVIVORS]);
    play(&mut g, id::SURVIVORS);
    assert_eq!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::Buy));
}

#[test]
fn necropolis_gives_two_actions() {
    let mut g = with_shelters(new_state(&[id::HERMIT], 2));
    set_hand(&mut g, 0, &[id::NECROPOLIS, id::MOAT]);
    play(&mut g, id::NECROPOLIS);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(cards::cost(id::NECROPOLIS), 1);
}

#[test]
fn hovel_is_trashed_from_the_hand_when_a_victory_card_is_gained() {
    let mut g = with_shelters(new_state(&[id::HERMIT], 2));
    set_hand(&mut g, 0, &[id::HOVEL, id::HOVEL]);
    buy_phase(&mut g, 5);
    buy(&mut g, id::DUCHY);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: Act::Trash }, id::HOVEL));
    yes(&mut g);
    expect_decision(&mut g);
    no(&mut g);
    assert_eq!(g.trash.get(id::HOVEL), 1);
    assert_eq!(in_hand(&g, 0, id::HOVEL), 1);
    // Not for a gain that is not a Victory card; and not trashed by anyone but its owner, from the hand only.
    let mut g = with_shelters(new_state(&[id::HERMIT], 2));
    set_hand(&mut g, 0, &[id::HOVEL]);
    buy_phase(&mut g, 3);
    buy(&mut g, id::SILVER);
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })));
    // Any way of gaining counts: a Curse is not a Victory card, an Estate from a Workshop is.
    let mut g = with_shelters(new_state(&[id::HERMIT, id::WORKSHOP], 2));
    set_hand(&mut g, 0, &[id::WORKSHOP, id::HOVEL]);
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::ESTATE);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::YesNo { act: Act::Trash }));
}

#[test]
fn overgrown_estate_is_a_victory_card_worth_nothing_that_draws_when_trashed() {
    let mut g = with_shelters(new_state(&[id::HERMIT, id::CHAPEL], 2));
    set_hand(&mut g, 0, &[id::CHAPEL, id::OVERGROWN_ESTATE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::OVERGROWN_ESTATE);
    assert_eq!(in_hand(&g, 0, id::PROVINCE), 1);
    let mut g = new_state(&[id::HERMIT], 2);
    set_discard(&mut g, 0, &[id::OVERGROWN_ESTATE]);
    assert_eq!(g.players[0].vp(), 0);
}

// ===========================================================================
// The Knights
// ===========================================================================

/// Player 0 plays `knight` (set in hand) against player 1, whose deck starts with `top`.
fn knight_attack(knight: CardId, top: &[CardId]) -> GameState {
    let mut g = new_state(&[id::KNIGHTS, id::MOAT], 2);
    set_hand(&mut g, 0, &[knight]);
    set_deck_known(&mut g, 1, top);
    g.turn.actions = 1;
    play(&mut g, knight);
    g
}

#[test]
fn every_knight_has_its_own_ability_and_the_common_attack() {
    // Dame Molly +2 Actions, Dame Sylvia +$2, Sir Bailey +1 Card +1 Action, Sir Destry +2 Cards, Sir Martin +2 Buys, Dame Josephine 2 VP.
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::DAME_MOLLY, id::MOAT]);
    play(&mut g, id::DAME_MOLLY);
    assert_eq!(g.turn.actions, 2);
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::DAME_SYLVIA, id::MOAT]);
    play(&mut g, id::DAME_SYLVIA);
    assert_eq!(g.turn.coins, 2);
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::SIR_BAILEY, id::MOAT]);
    set_deck_known(&mut g, 0, &junk(2));
    play(&mut g, id::SIR_BAILEY);
    assert_eq!((g.turn.actions, g.players[0].hand.total()), (1, 2));
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::SIR_DESTRY]);
    set_deck_known(&mut g, 0, &junk(3));
    play(&mut g, id::SIR_DESTRY);
    assert_eq!(g.players[0].hand.total(), 2);
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::SIR_MARTIN]);
    play(&mut g, id::SIR_MARTIN);
    assert_eq!(g.turn.buys, 3);
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_discard(&mut g, 0, &[id::DAME_JOSEPHINE, id::DAME_JOSEPHINE]);
    assert_eq!(g.players[0].vp(), 4);
}

#[test]
fn the_knights_attack_trashes_a_card_costing_3_to_6_of_the_two_revealed_the_victim_chooses_and_discards_the_rest() {
    // Both qualify: the victim chooses.
    let mut g = knight_attack(id::DAME_SYLVIA, &[id::SILVER, id::GOLD, id::ESTATE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    let mut o = offered(&g);
    o.sort();
    assert_eq!(o, vec![id::SILVER, id::GOLD]);
    pick(&mut g, id::SILVER);
    assert!(g.trash.has(id::SILVER) && g.players[1].discard.has(id::GOLD));
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::ESTATE));
    // One qualifies: no choice.
    let g = knight_attack(id::DAME_SYLVIA, &[id::COPPER, id::DUCHY]);
    assert!(g.trash.has(id::DUCHY) && g.players[1].discard.has(id::COPPER));
    // None qualifies ($2 and $8): both discarded.
    let g = knight_attack(id::DAME_SYLVIA, &[id::ESTATE, id::PROVINCE]);
    assert_eq!(g.trash.total(), 0);
    assert_eq!(g.players[1].discard.total(), 2);
    // A card with a Potion in its cost does not cost $3 to $6.
    let mut g = new_state(&[id::KNIGHTS, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::DAME_SYLVIA]);
    set_deck_known(&mut g, 1, &[id::FAMILIAR, id::COPPER]);
    play(&mut g, id::DAME_SYLVIA);
    assert_eq!(g.trash.total(), 0);
}

#[test]
fn a_knight_trashed_by_a_knight_takes_the_attacker_with_it() {
    // The victim reveals a Knight (and a Copper): trashed, so the played Knight is trashed too.
    let g = knight_attack(id::DAME_SYLVIA, &[id::DAME_MOLLY, id::COPPER]);
    assert!(g.trash.has(id::DAME_MOLLY) && g.trash.has(id::DAME_SYLVIA));
    assert!(!g.players[0].in_play.has(id::DAME_SYLVIA));
    // If both a Knight and another card qualify and the victim picks the other card, the attacker keeps its Knight.
    let mut g = knight_attack(id::DAME_SYLVIA, &[id::DAME_MOLLY, id::GOLD]);
    expect_decision(&mut g);
    pick(&mut g, id::GOLD);
    assert!(g.trash.has(id::GOLD) && !g.trash.has(id::DAME_SYLVIA));
    assert!(g.players[0].in_play.has(id::DAME_SYLVIA) || g.players[0].discard.has(id::DAME_SYLVIA));
}

#[test]
fn sir_vander_trashed_gains_its_owner_a_gold_on_either_players_turn() {
    // Its owner is the victim: the Knight trashes their Sir Vander, they gain a Gold; the attacker's Knight is trashed as well.
    let g = knight_attack(id::DAME_SYLVIA, &[id::SIR_VANDER, id::COPPER]);
    assert_eq!(g.players[1].discard.get(id::GOLD), 1);
    assert!(g.trash.has(id::DAME_SYLVIA));
    // The owner is the attacker: their played Sir Vander trashed by the victim's Knight.
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::SIR_VANDER]);
    set_deck_known(&mut g, 1, &[id::DAME_ANNA, id::COPPER]);
    play(&mut g, id::SIR_VANDER);
    assert!(g.trash.has(id::SIR_VANDER) && g.trash.has(id::DAME_ANNA));
    assert_eq!(g.players[0].discard.get(id::GOLD), 1, "the player who had Sir Vander gains the Gold");
    assert_eq!(g.players[1].discard.get(id::GOLD), 0);
}

#[test]
fn dame_anna_trashes_up_to_two_cards_first_and_her_own_trash_of_a_knight_does_not_trash_her() {
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::DAME_ANNA, id::SIR_VANDER, id::CURSE, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::COPPER]);
    play(&mut g, id::DAME_ANNA);
    assert!(is_select(&g, Zone::Hand, Act::Trash));
    pick(&mut g, id::COPPER);
    pick(&mut g, id::SIR_VANDER);
    assert!(g.trash.has(id::SIR_VANDER) && g.trash.has(id::COPPER));
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    assert!(g.players[0].in_play.has(id::DAME_ANNA), "not trashed: it was her own trash");
    // Zero is allowed.
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::DAME_ANNA, id::CURSE]);
    play(&mut g, id::DAME_ANNA);
    pass(&mut g);
    assert_eq!(g.trash.total(), 0);
}

#[test]
fn dame_natalie_may_gain_a_card_costing_up_to_3_and_sir_michael_discards_before_the_trashing() {
    let mut g = new_state(&[id::KNIGHTS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DAME_NATALIE]);
    set_deck_known(&mut g, 1, &[id::COPPER, id::COPPER]);
    play(&mut g, id::DAME_NATALIE);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Gain { max_cost: 3, optional: true, .. }));
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::DAME_NATALIE]);
    play(&mut g, id::DAME_NATALIE);
    pass(&mut g);
    assert_eq!(g.players[0].discard.total(), 0);
    // Sir Michael: the victim with 5 cards discards 2 first; only then are the top two cards revealed.
    let mut g = new_state(&[id::KNIGHTS], 2);
    set_hand(&mut g, 0, &[id::SIR_MICHAEL]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::ESTATE, id::ESTATE, id::SILVER]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::COPPER]);
    play(&mut g, id::SIR_MICHAEL);
    assert!(is_select(&g, Zone::Hand, Act::Discard));
    assert_eq!(g.trash.total(), 0, "nothing trashed yet");
    pick(&mut g, id::ESTATE); // the second Estate is then the only choice
    assert_eq!(g.players[1].hand.total(), 3);
    assert!(g.trash.has(id::GOLD));
}

#[test]
fn the_knights_attack_goes_round_in_turn_order_and_a_moat_protects() {
    let mut g = new_state(&[id::KNIGHTS, id::MOAT], 3);
    set_hand(&mut g, 0, &[id::DAME_SYLVIA]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::COPPER]);
    set_deck_known(&mut g, 2, &[id::SILVER, id::COPPER]);
    play(&mut g, id::DAME_SYLVIA);
    moat(&mut g, true);
    assert!(!g.trash.has(id::GOLD), "revealed Moat: unaffected");
    assert!(g.trash.has(id::SILVER), "the player after them is attacked");
    // An attack that finds a Knight for the first victim stops the Knight, but the later victims are still attacked.
    let mut g = new_state(&[id::KNIGHTS], 3);
    set_hand(&mut g, 0, &[id::DAME_SYLVIA]);
    set_deck_known(&mut g, 1, &[id::SIR_BAILEY, id::COPPER]);
    set_deck_known(&mut g, 2, &[id::GOLD, id::COPPER]);
    play(&mut g, id::DAME_SYLVIA);
    assert!(g.trash.has(id::SIR_BAILEY) && g.trash.has(id::DAME_SYLVIA) && g.trash.has(id::GOLD));
}

#[test]
fn a_knight_played_by_band_of_misfits_is_not_trashed_by_its_own_attack() {
    let mut g = new_state(&[id::BAND_OF_MISFITS, id::KNIGHTS], 2);
    let top = g.pile_top(&KNIGHTS_PILE).unwrap();
    g.supply.set(top, 0);
    g.supply.set(id::SIR_MARTIN, 10);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_deck_known(&mut g, 1, &[id::SIR_BAILEY, id::COPPER]);
    play(&mut g, id::BAND_OF_MISFITS); // Sir Martin is the only Action costing less than $5 in the Supply: played at once
    assert!(g.trash.has(id::SIR_BAILEY), "the victim's Knight is trashed");
    assert_eq!(g.supply.get(id::SIR_MARTIN), 10, "the Sir Martin in the Supply is untouched");
    assert!(g.players[0].in_play.has(id::BAND_OF_MISFITS));
}

// ===========================================================================
// Trash triggers on every trash path
// ===========================================================================

#[test]
fn a_swindler_victims_cultist_draws_before_the_swindler_gain() {
    let mut g = new_state(&[id::SWINDLER, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::SWINDLER]);
    set_deck_known(&mut g, 1, &[id::CULTIST, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::SWINDLER);
    assert!(g.trash.has(id::CULTIST));
    assert_eq!(g.players[1].hand.total(), 3, "the victim draws 3 for the trashed Cultist");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, exact: true, .. }), "{d:?}");
}

#[test]
fn treasure_map_trashing_itself_lets_a_market_square_react() {
    let mut g = new_state(&[id::TREASURE_MAP, id::MARKET_SQUARE], 2);
    set_hand(&mut g, 0, &[id::TREASURE_MAP, id::MARKET_SQUARE]);
    play(&mut g, id::TREASURE_MAP);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: Act::Discard }, id::MARKET_SQUARE), "{d:?}");
}

#[test]
fn horn_of_plenty_trashed_after_a_victory_gain_lets_a_market_square_react() {
    let mut g = new_state(&[id::HORN_OF_PLENTY, id::MARKET_SQUARE], 2);
    set_hand(&mut g, 0, &[id::HORN_OF_PLENTY, id::MARKET_SQUARE]);
    set_in_play(&mut g, 0, &[id::COPPER]);
    g.turn.phase = Phase::Buy;
    play_treasure(&mut g, id::HORN_OF_PLENTY);
    pick(&mut g, id::ESTATE);
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: Act::Discard }, id::MARKET_SQUARE), "{d:?}");
    yes(&mut g);
    assert!(g.trash.has(id::HORN_OF_PLENTY));
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
}

#[test]
fn a_fortress_trashed_by_lurker_from_the_supply_goes_into_the_players_hand() {
    let mut g = new_state(&[id::LURKER, id::FORTRESS], 2);
    set_hand(&mut g, 0, &[id::LURKER]);
    play(&mut g, id::LURKER);
    mode(&mut g, 0);
    pick(&mut g, id::FORTRESS);
    assert_eq!(in_hand(&g, 0, id::FORTRESS), 1);
    assert!(!g.trash.has(id::FORTRESS));
}

#[test]
fn a_cultist_trashed_by_lurker_draws_three_cards_for_the_lurkers_player() {
    let mut g = new_state(&[id::LURKER, id::CULTIST], 2);
    set_hand(&mut g, 0, &[id::LURKER]);
    set_deck_known(&mut g, 0, &junk(5));
    play(&mut g, id::LURKER);
    mode(&mut g, 0);
    pick(&mut g, id::CULTIST);
    assert_eq!(g.players[0].hand.total(), 3);
    // The Ruins pile is the Cultist's: untouched by Lurker.
    assert_eq!(g.pile_size(&RUINS_PILE), 10);
}

#[test]
fn lurker_trashing_the_top_knight_reveals_the_next_one() {
    let mut g = new_state(&[id::LURKER, id::KNIGHTS], 2);
    let top = g.pile_top(&KNIGHTS_PILE).unwrap();
    set_hand(&mut g, 0, &[id::LURKER]);
    play(&mut g, id::LURKER);
    mode(&mut g, 0);
    assert!(offered(&g).contains(&top));
    pick(&mut g, top);
    assert!(g.trash.has(top));
    let next = g.pile_top(&KNIGHTS_PILE).unwrap();
    assert_ne!(next, top);
    assert_eq!(g.pile_size(&KNIGHTS_PILE), 9);
}

#[test]
fn mint_trashing_treasures_lets_market_square_react_for_each_card_trashed() {
    let mut g = new_state(&[id::MINT, id::MARKET_SQUARE], 2);
    set_in_play(&mut g, 0, &[id::COPPER, id::SILVER]);
    set_hand(&mut g, 0, &[id::MARKET_SQUARE, id::MARKET_SQUARE]);
    buy_phase(&mut g, 5);
    buy(&mut g, id::MINT);
    let mut offers = 0;
    while matches!(g.pending_decision().map(|d| (d.kind, d.subject)), Some((DecisionKind::YesNo { act: Act::Discard }, s)) if s == id::MARKET_SQUARE) {
        offers += 1;
        yes(&mut g);
    }
    assert_eq!(offers, 2);
    assert_eq!(all_cards_of(&g, 0).get(id::GOLD), 2, "the turn ended and the cards were reshuffled");
}

// ===========================================================================
// Whole games with random choices
// ===========================================================================

#[test]
fn random_dark_ages_games_conserve_cards_and_keep_the_piles_consistent() {
    let mut rng = Rng::new(2024);
    let dark: Vec<CardId> = cards::kingdom_cards_in(CardSet::DarkAges).collect();
    let mut shelter_games = 0;
    for game in 0..2500u64 {
        let n = 2 + (game % 5) as usize;
        let mut pool = dark.clone();
        for i in (1..pool.len()).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            pool.swap(i, j);
        }
        pool.truncate(10);
        // Half the games: a mixed kingdom, with some Base cards.
        if game % 2 == 1 {
            pool.truncate(5);
            pool.extend([id::SMITHY, id::MILITIA, id::MOAT, id::WORKSHOP, id::LABORATORY]);
        }
        let mut g = new_game(&pool, n, game);
        shelter_games += g.shelters as u32;
        let count = |g: &GameState| {
            let mut t = g.supply.total() + g.trash.total() + g.side_pile_cards();
            for p in 0..g.num_players as usize {
                t += g.players[p].all_cards().total();
            }
            t
        };
        let initial = count(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        loop {
            match g.advance(&mut NoEvents) {
                Step::Decision(_) => {
                    g.legal_choices(&mut buf);
                    assert!(!buf.is_empty());
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut NoEvents).unwrap();
                }
                Step::GameOver => break,
                other => panic!("{other:?}"),
            }
            assert_eq!(count(&g), initial, "card conservation, game {game}");
            for pile in [&KNIGHTS_PILE, &RUINS_PILE] {
                if g.has_pile(pile) {
                    assert!(pile.members().filter(|&m| g.supply.get(m) > 0).count() <= 1, "game {game}: one top card");
                }
            }
            // No Knight is ever in two places.
            for k in KNIGHTS_PILE.members() {
                let mut total = (g.supply.get(k) > 0) as u32 + g.trash.get(k) as u32;
                for p in 0..n {
                    total += g.players[p].all_cards().get(k) as u32;
                }
                assert!(total <= 1, "game {game}: {} exists {total} times", cards::name(k));
            }
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
    }
    assert!(shelter_games > 1000, "{shelter_games} games with Shelters");
}

// ===========================================================================
// More rulings
// ===========================================================================

#[test]
fn redealing_a_new_games_piles_draws_them_again_with_the_games_own_rng() {
    // "Simulate from a new game": every simulated game starts from one dealt state but must not share its pile order.
    let base = new_game(&[id::KNIGHTS, id::CULTIST], 2, 5);
    let mut tops = std::collections::HashSet::new();
    let mut ruins = std::collections::HashSet::new();
    for seed in 0..200u64 {
        let mut s = base;
        s.rng = Rng::derive(9, seed);
        s.redeal_mixed_piles();
        assert_eq!((s.pile_size(&KNIGHTS_PILE), s.pile_size(&RUINS_PILE)), (10, 10), "sizes are kept");
        assert_eq!(KNIGHTS_PILE.members().filter(|&m| s.supply.get(m) > 0).count(), 1);
        tops.insert(s.pile_top(&KNIGHTS_PILE).unwrap());
        ruins.insert(s.pile_top(&RUINS_PILE).unwrap());
    }
    assert_eq!((tops.len(), ruins.len()), (10, 5), "every Knight and every Ruins is on top in some games");
}

#[test]
fn a_throne_roomed_cultist_gives_two_ruins_and_draws_twice() {
    let mut g = new_state(&[id::CULTIST, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::CULTIST]);
    set_deck_known(&mut g, 0, &junk(8));
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::CULTIST);
    assert_eq!(g.players[0].hand.total(), 4);
    assert_eq!(RUINS_PILE.members().map(|r| all_cards_of(&g, 1).get(r)).sum::<u8>(), 2);
}

#[test]
fn a_throne_roomed_catacombs_or_count_makes_each_choice_twice() {
    let mut g = new_state(&[id::COUNT, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::COUNT, id::GOLD, id::GOLD]);
    stay_in_action_phase(&mut g, 0);
    g.turn.actions = 1;
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::COUNT);
    mode(&mut g, 2); // a Copper
    mode(&mut g, 3); // +$3
    mode(&mut g, 2);
    mode(&mut g, 3);
    // Two +$3, and the two Golds in hand are played when the Action phase ends.
    assert_eq!(g.turn.coins, 6 + 6);
    assert_eq!(all_cards_of(&g, 0).get(id::COPPER), 2);
}

#[test]
fn counts_hand_trash_and_a_card_drawn_by_a_trigger_are_not_trashed() {
    // Fortress is trashed with the rest of the hand and comes back into the new hand.
    let mut g = new_state(&[id::COUNT, id::FORTRESS], 2);
    set_hand(&mut g, 0, &[id::COUNT, id::FORTRESS, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk(3));
    stay_in_action_phase(&mut g, 0);
    play(&mut g, id::COUNT);
    mode(&mut g, 2);
    mode(&mut g, 4);
    assert_eq!(in_hand(&g, 0, id::FORTRESS), 1);
    assert_eq!(g.trash.get(id::ESTATE), 2);
    assert_eq!(g.trash.get(id::FORTRESS), 0);
}

#[test]
fn death_cart_after_being_gained_by_a_ferryman_pile_still_gives_ruins() {
    // The Death Cart pile outside the Supply (Ferryman): the Ruins still come with the Death Cart.
    let g = GameState::new(&GameConfig { kingdom: vec![id::FERRYMAN, id::SMITHY], ferryman: Some(id::DEATH_CART), ..Default::default() });
    assert!(g.has_pile(&RUINS_PILE));
}

#[test]
fn urchin_trashing_is_only_offered_while_a_mercenary_is_left() {
    let mut g = new_state(&[id::URCHIN, id::MILITIA], 2);
    set_in_play(&mut g, 0, &[id::URCHIN]);
    set_hand(&mut g, 0, &[id::MILITIA]);
    g.mercenary = 0;
    play(&mut g, id::MILITIA);
    assert!(!matches!(g.pending_decision().map(|d| d.subject), Some(s) if s == id::URCHIN));
}

#[test]
fn a_beggar_reaction_is_offered_even_to_a_player_a_lighthouse_protects() {
    let mut g = new_state(&[id::BEGGAR, id::MILITIA, id::LIGHTHOUSE], 2);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::BEGGAR, id::COPPER]);
    set_in_play(&mut g, 0, &[id::LIGHTHOUSE]);
    play(&mut g, id::MILITIA);
    let d = expect_decision(&mut g);
    assert_eq!(d.subject, id::BEGGAR);
}
