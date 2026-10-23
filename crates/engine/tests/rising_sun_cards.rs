//! Rising Sun tests (rulings in `docs/rising-sun-plan.md`): the cards, the Omens and Prophecies with their Sun tokens, the Shadow cards, Debt on play and the
//! Events.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn strip_seed(s: &str) -> String {
    s.lines().filter(|l| !l.starts_with("seed:")).collect::<Vec<_>>().join("\n")
}

// ===========================================================================
// Random games: nothing is lost, nothing panics, every game ends
// ===========================================================================

#[test]
fn random_rising_sun_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0x215_1_5E7);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::RisingSun).collect();
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::RisingSun).collect();
    let events: Vec<CardId> = (id::AMASS..=id::SEA_TRADE).collect();
    let mut seen_active = [false; cards::NUM_PROPHECIES];
    let mut shadow_plays = 0;
    for game in 0..3000u64 {
        let n = 2 + (game % 4) as usize;
        let mut kingdom: Vec<CardId> = Vec::new();
        let mut from = pool.clone();
        for _ in 0..8 {
            let i = rng.below(from.len() as u32) as usize;
            kingdom.push(from.swap_remove(i));
        }
        // Always an Omen and a Shadow card at hand for the Prophecy and the deck rules.
        if !kingdom.iter().any(|&c| cards::is(c, cards::OMEN)) {
            kingdom.push(id::POET);
        }
        if !kingdom.iter().any(|&c| cards::is(c, cards::SHADOW)) && !kingdom.contains(&id::ALLEY) {
            kingdom.push(id::ALLEY);
        }
        for _ in 0..2 {
            let c = others[rng.below(others.len() as u32) as usize];
            if !kingdom.contains(&c) {
                kingdom.push(c);
            }
        }
        let mut evs = events.clone();
        for _ in 0..(1 + game % 3) {
            let i = rng.below(evs.len() as u32) as usize;
            kingdom.push(evs.swap_remove(i));
        }
        // The Prophecies that are available (Divine Wind is disabled for now; its own test sets it up directly).
        let ready: Vec<CardId> = (0..cards::NUM_PROPHECIES as CardId).map(|i| cards::FIRST_PROPHECY + i).filter(|&p| cards::is_ready(p)).collect();
        let prophecy = ready[(game % ready.len() as u64) as usize];
        kingdom.push(prophecy);
        if prophecy == id::APPROACHING_ARMY {
            kingdom.push(id::WITCH);
        }
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom, seed: game, max_turns: 300, ..Default::default() });
        // Few Sun tokens, so that the Prophecy happens early in most games.
        g.rs.suns = g.rs.suns.min(1 + (game % 3) as u8);
        let mut initial = total_cards(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        let mut log: Vec<Event> = Vec::new();
        loop {
            log.clear();
            match g.advance(&mut log) {
                Step::Decision(_) => {
                    g.legal_choices(&mut buf);
                    assert!(!buf.is_empty());
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut log).unwrap_or_else(|e| panic!("game {game}: {e} for {c:?}"));
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            if g.rs.active != 0 {
                seen_active[(g.rs.active - cards::FIRST_PROPHECY) as usize] = true;
            }
            shadow_plays += log.iter().filter(|e| matches!(e, Event::Play { card, .. } if cards::is(*card, cards::SHADOW))).count();
            // Divine Wind swaps the Kingdom piles: the cards the players and the trash hold stay, the Supply is another one.
            if log.iter().any(|e| matches!(e, Event::Prophecy { prophecy, .. } if *prophecy == id::DIVINE_WIND)) {
                initial = total_cards(&g);
            }
            if total_cards(&g) != initial {
                panic!("card conservation, game {game} ({}): {log:?}", cards::name(prophecy));
            }
            steps += 1;
            assert!(steps < 200_000, "game {game} does not end");
        }
        // The text format round-trips whatever the game left.
        let text = format_state(&g);
        let back = parse_state(&text).unwrap_or_else(|e| panic!("game {game}: {e}\n{text}"));
        assert_eq!(strip_seed(&format_state(&back)), strip_seed(&text), "game {game}");
    }
    assert!(
        seen_active.iter().enumerate().all(|(i, &b)| b || !cards::is_ready(cards::FIRST_PROPHECY + i as CardId)),
        "every available Prophecy should have happened in some game: {seen_active:?}"
    );
    assert!(shadow_plays > 0);
}

// ===========================================================================
// Helpers
// ===========================================================================

fn count(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].all_cards().get(c)
}

fn pick(g: &mut GameState, c: CardId) -> Step {
    choose(g, Choice::Card(c))
}

fn has_choice(g: &GameState, c: Choice) -> bool {
    choices(g).contains(&c)
}

/// A state of the kingdom with its Prophecy already in effect (the last Sun token gone), as after the Omen that removed it.
fn active(kingdom: &[CardId], prophecy: CardId, players: usize) -> GameState {
    let mut k = kingdom.to_vec();
    k.push(prophecy);
    let mut g = new_state(&k, players);
    g.rs.suns = 0;
    g.rs.active = prophecy;
    g.rs_update_hooks();
    if prophecy == id::FLOURISHING_TRADE {
        g.turn.cost_reduction = 1;
    }
    g
}

/// A Buy phase for player 1 with `coins` and `buys` (the Treasures all played).
fn buy_phase(g: &mut GameState, coins: u16, buys: u8) {
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
    g.turn.buys = buys;
    g.turn.treasures_done = true;
    g.pending = Pending::None;
}

/// Plays `card` and answers the questions that follow with the first choice that is not `Pass`, until the engine asks for another play.
fn events_of(g: &mut GameState, card: CardId) -> Vec<Event> {
    let mut log: Vec<Event> = Vec::new();
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    choose_ev(g, Choice::Card(card), &mut log);
    log
}

// ===========================================================================
// Set-up: ids, the Sun tokens and the Prophecy
// ===========================================================================

#[test]
fn the_ids_and_counts_line_up() {
    assert_eq!(cards::kingdom_cards_in(CardSet::RisingSun).count(), 25);
    assert_eq!(cards::NUM_EVENTS, 79);
    assert_eq!(cards::NUM_PROPHECIES, 15);
    assert_eq!(id::SICKNESS as usize + 1, cards::FIRST_PROJECT as usize);
    assert!(cards::is_prophecy(id::APPROACHING_ARMY) && cards::is_prophecy(id::SICKNESS) && !cards::is_prophecy(id::TWICE_MISERABLE));
}

/// Plays the game on until it is `player`'s turn again (everyone just passes; cleanup and draws happen as usual), stopping at the first decision of that
/// turn.
fn pass_until_turn_of(g: &mut GameState, player: u8) {
    let start = g.turn.number;
    for _ in 0..200 {
        match adv(g) {
            Step::Decision(d) => {
                if g.turn.player == player && g.turn.number > start && d.player == player {
                    return;
                }
                g.apply(Choice::Pass, &mut NoEvents).or_else(|_| {
                    let c = choices(g)[0];
                    g.apply(c, &mut NoEvents)
                }).unwrap();
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("the turn never came back");
}

#[test]
fn sun_tokens_depend_on_the_number_of_players() {
    for (n, tokens) in [(2, 5), (3, 8), (4, 10), (5, 12), (6, 13)] {
        let g = new_state(&[id::POET, id::SMITHY, id::GROWTH], n);
        assert_eq!((g.rs.prophecy, g.rs.suns, g.rs.active), (id::GROWTH, tokens, 0), "{n} players");
    }
}

#[test]
fn an_omen_removes_a_sun_token_and_the_last_one_makes_the_prophecy_happen_once() {
    let mut g = new_state(&[id::TEA_HOUSE, id::SMITHY, id::GROWTH], 2);
    g.rs.suns = 2;
    g.turn.actions = 3;
    set_hand(&mut g, 0, &[id::TEA_HOUSE, id::TEA_HOUSE, id::TEA_HOUSE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    let log = events_of(&mut g, id::TEA_HOUSE);
    assert!(log.iter().any(|e| matches!(e, Event::Sun { left: 1, .. })));
    assert_eq!((g.rs.suns, g.rs.active), (1, 0));
    let log = events_of(&mut g, id::TEA_HOUSE);
    assert!(log.iter().any(|e| matches!(e, Event::Sun { left: 0, .. })) && log.iter().any(|e| matches!(e, Event::Prophecy { prophecy, .. } if *prophecy == id::GROWTH)));
    assert_eq!((g.rs.suns, g.rs.active), (0, id::GROWTH));
    // Further Omens do nothing to the Prophecy.
    let log = events_of(&mut g, id::TEA_HOUSE);
    assert!(!log.iter().any(|e| matches!(e, Event::Sun { .. } | Event::Prophecy { .. })));
    assert_eq!((g.turn.coins, g.rs.suns), (6, 0), "each Tea House is +$2");
}

#[test]
fn throne_room_on_an_omen_removes_two_tokens_and_a_game_without_a_prophecy_ignores_the_sun() {
    let mut g = new_state(&[id::TEA_HOUSE, id::THRONE_ROOM, id::GROWTH], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::TEA_HOUSE]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::TEA_HOUSE);
    assert_eq!(g.rs.suns, 3);
    let mut g = new_state(&[id::TEA_HOUSE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::TEA_HOUSE]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::TEA_HOUSE);
    assert_eq!((g.rs.prophecy, g.rs.suns), (0, 0));
}

#[test]
fn a_randomized_kingdom_gets_a_prophecy_exactly_when_it_has_an_omen() {
    let mut with_prophecy = 0;
    let mut approaching = 0;
    for seed in 0..400u64 {
        let mut rng = Rng::new(seed);
        let k = cards::random_kingdom_with_colonies(&[CardSet::RisingSun, CardSet::Base], &[], &mut rng);
        let omen = k.iter().any(|&c| cards::is(c, cards::OMEN));
        let prophecies = k.iter().filter(|&&c| cards::is_prophecy(c)).count();
        assert_eq!(prophecies, usize::from(omen), "seed {seed}: {k:?}");
        with_prophecy += prophecies;
        if k.contains(&id::APPROACHING_ARMY) {
            approaching += 1;
            // An Attack pile on top of the ten Kingdom cards, even if the kingdom had an Attack already.
            assert_eq!(k.iter().filter(|&&c| cards::is_kingdom(c)).count(), 11, "seed {seed}: {k:?}");
            assert!(k.iter().any(|&c| cards::is_kingdom(c) && cards::is(c, cards::ATTACK)));
        } else {
            assert_eq!(k.iter().filter(|&&c| cards::is_kingdom(c)).count(), 10, "seed {seed}");
        }
    }
    assert!(with_prophecy > 50 && approaching > 0, "{with_prophecy} games with a Prophecy, {approaching} with Approaching Army");
}

// ===========================================================================
// The kingdom cards
// ===========================================================================

#[test]
fn mountain_shrine_trashes_a_card_and_draws_two_if_the_trash_has_an_action() {
    let k = [id::MOUNTAIN_SHRINE, id::SMITHY, id::GROWTH];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::MOUNTAIN_SHRINE, id::SMITHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::CURSE, id::ESTATE]);
    play(&mut g, id::MOUNTAIN_SHRINE);
    assert_eq!((g.turn.coins, g.rs.suns), (2, 4), "+$2 and a Sun token");
    pick(&mut g, id::SMITHY);
    // The Action just trashed is in the trash: +2 Cards.
    assert_eq!(g.trash.get(id::SMITHY), 1);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE, id::CURSE]));
    // No Action in the trash: no cards.
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::MOUNTAIN_SHRINE, id::SMITHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::CURSE, id::ESTATE]);
    play(&mut g, id::MOUNTAIN_SHRINE);
    pick(&mut g, id::DUCHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::SMITHY]));
    // Trashing is optional, and an Action put in the trash by anyone earlier counts.
    let mut g = new_state(&k, 2);
    g.trash.add(id::SMITHY, 1);
    set_hand(&mut g, 0, &[id::MOUNTAIN_SHRINE, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::CURSE, id::ESTATE]);
    play(&mut g, id::MOUNTAIN_SHRINE);
    pass(&mut g);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::PROVINCE, id::CURSE]));
}

#[test]
fn mountain_shrine_checks_the_trash_after_a_fortress_has_left_it() {
    let mut g = new_state(&[id::MOUNTAIN_SHRINE, id::FORTRESS, id::GROWTH], 2);
    set_hand(&mut g, 0, &[id::MOUNTAIN_SHRINE, id::FORTRESS]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD, id::ESTATE]);
    play(&mut g, id::MOUNTAIN_SHRINE);
    pick(&mut g, id::FORTRESS);
    // The Fortress came back to the hand (+1 Card +2 Actions on trashing), so there is no Action in the trash: no +2 Cards.
    assert_eq!(g.trash.get(id::FORTRESS), 0);
    assert!(g.players[0].hand.has(id::FORTRESS) && !g.players[0].hand.has(id::GOLD), "{:?}", g.players[0].hand);
}

#[test]
fn debt_costs_take_the_tokens_when_the_card_is_bought() {
    let mut g = new_state(&[id::MOUNTAIN_SHRINE, id::DAIMYO, id::ARTIST, id::GROWTH], 2);
    for (card, debt) in [(id::MOUNTAIN_SHRINE, 5), (id::DAIMYO, 6), (id::ARTIST, 8)] {
        let mut h = g;
        buy_phase(&mut h, 0, 1);
        buy(&mut h, card);
        assert_eq!((h.players[0].debt, count(&h, 0, card)), (debt, 1), "{}", cards::name(card));
    }
    assert_eq!(cards::cost_string(id::DAIMYO), "6 Debt");
    g.players[0].debt = 1;
    buy_phase(&mut g, 1, 1);
    adv(&mut g);
    assert!(has_choice(&g, Choice::PayDebt) && !has_choice(&g, Choice::Card(id::COPPER)), "no purchases with Debt");
}

#[test]
fn daimyo_replays_the_next_non_command_action_once_for_each_daimyo() {
    let k = [id::DAIMYO, id::SMITHY, id::CRAFTSMAN];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::DAIMYO, id::SMITHY, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 8]);
    play(&mut g, id::DAIMYO);
    play(&mut g, id::SMITHY);
    // +1 Card, then Smithy's +3 Cards twice.
    assert_eq!(g.players[0].hand.total(), 2 + 1 + 3 + 3);
    // Command cards (Daimyo itself) are not replayed, and the replay is for the next Action only.
    let mut g = new_state(&k, 2);
    g.turn.actions = 3;
    set_hand(&mut g, 0, &[id::DAIMYO, id::DAIMYO, id::CRAFTSMAN, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 12]);
    play(&mut g, id::DAIMYO);
    play(&mut g, id::DAIMYO);
    // Two Daimyos and a Craftsman: three plays in all.
    play(&mut g, id::CRAFTSMAN);
    for _ in 0..3 {
        pick(&mut g, id::SILVER);
    }
    assert_eq!((g.players[0].debt, count(&g, 0, id::SILVER)), (6, 3));
    // The next play after that is not replayed.
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 2 + 3, "two Daimyo draws, Smithy's 3 once");
}

#[test]
fn daimyo_waits_for_the_next_action_even_across_non_actions() {
    let mut g = new_state(&[id::DAIMYO, id::SMITHY], 2);
    g.turn.actions = 1;
    set_hand(&mut g, 0, &[id::DAIMYO, id::SMITHY, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 10]);
    play(&mut g, id::DAIMYO);
    assert_eq!(g.turn.rs.daimyo, 1);
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.rs.daimyo, 0);
}

#[test]
fn artist_draws_a_card_per_card_with_exactly_one_copy_in_play() {
    let mut g = new_state(&[id::ARTIST, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ARTIST]);
    set_in_play(&mut g, 0, &[id::SILVER, id::GOLD, id::GOLD, id::COPPER, id::COPPER, id::COPPER, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 8]);
    play(&mut g, id::ARTIST);
    // Artist itself, Silver and Smithy have one copy each; Gold and Copper have more.
    assert_eq!(g.players[0].hand.total(), 3);
    assert_eq!(g.turn.actions, 1, "+1 Action");
}

#[test]
fn artist_counts_the_cards_that_stayed_in_play_from_earlier_turns() {
    let mut g = new_state(&[id::ARTIST, id::SAMURAI], 2);
    set_hand(&mut g, 0, &[id::ARTIST]);
    set_in_play(&mut g, 0, &[id::SAMURAI]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 4]);
    play(&mut g, id::ARTIST);
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn fishmonger_gives_a_buy_and_a_coin() {
    let mut g = new_state(&[id::FISHMONGER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::FISHMONGER]);
    play(&mut g, id::FISHMONGER);
    assert_eq!((g.turn.buys, g.turn.coins, g.turn.actions), (2, 1, 0));
}

fn snake_witch_game() -> GameState {
    let mut g = new_state(&[id::SNAKE_WITCH, id::THRONE_ROOM, id::SMITHY], 3);
    set_deck_known(&mut g, 0, &[id::GOLD, id::ESTATE]);
    g
}

#[test]
fn snake_witch_returns_to_its_pile_to_curse_the_others_when_the_hand_has_no_duplicates() {
    let mut g = snake_witch_game();
    set_hand(&mut g, 0, &[id::SNAKE_WITCH, id::COPPER, id::SILVER]);
    let curses = g.supply.get(id::CURSE);
    let pile = g.supply.get(id::SNAKE_WITCH);
    play(&mut g, id::SNAKE_WITCH);
    // The Gold was drawn first, and then the hand (Copper, Silver, Gold) is revealed.
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::SILVER, id::GOLD]));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.supply.get(id::SNAKE_WITCH), pile + 1, "back on its pile");
    assert!(!g.players[0].in_play.has(id::SNAKE_WITCH));
    assert_eq!(g.supply.get(id::CURSE), curses - 2);
    assert_eq!((count(&g, 1, id::CURSE), count(&g, 2, id::CURSE), count(&g, 0, id::CURSE)), (1, 1, 0));
    // Declining keeps everything.
    let mut g = snake_witch_game();
    set_hand(&mut g, 0, &[id::SNAKE_WITCH, id::COPPER, id::SILVER]);
    play(&mut g, id::SNAKE_WITCH);
    choose(&mut g, Choice::No);
    assert!(g.players[0].in_play.has(id::SNAKE_WITCH) && g.supply.get(id::CURSE) == curses);
}

#[test]
fn snake_witch_does_nothing_with_a_duplicate_in_the_hand_and_may_reveal_an_empty_hand() {
    let mut g = snake_witch_game();
    set_hand(&mut g, 0, &[id::SNAKE_WITCH, id::COPPER, id::GOLD]);
    play(&mut g, id::SNAKE_WITCH);
    // The drawn Gold makes a pair: no question (the +1 Action finds no Action to play: the Buy phase).
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!(g.supply.get(id::CURSE), 20);
    let mut g = new_state(&[id::SNAKE_WITCH, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SNAKE_WITCH]);
    play(&mut g, id::SNAKE_WITCH);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }), "no cards in hand: no duplicates, {d:?}");
}

#[test]
fn a_throne_roomed_snake_witch_curses_only_once() {
    let mut g = snake_witch_game();
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SNAKE_WITCH, id::COPPER, id::SILVER]);
    g.turn.actions = 1;
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::SNAKE_WITCH);
    // The first play: reveal and return.
    choose(&mut g, Choice::Yes);
    // The second play cannot return it (it is not in play): no question, no Curses.
    assert_eq!((count(&g, 1, id::CURSE), count(&g, 2, id::CURSE)), (1, 1));
    assert_ne!(pending_kind_of(&mut g), None);
}

fn pending_kind_of(g: &mut GameState) -> Option<DecisionKind> {
    match adv(g) {
        Step::Decision(d) => Some(d.kind),
        _ => None,
    }
}

#[test]
fn aristocrat_depends_on_the_aristocrats_in_play() {
    let mut g = new_state(&[id::ARISTOCRAT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ARISTOCRAT; 4]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
    play(&mut g, id::ARISTOCRAT);
    assert_eq!(g.turn.actions, 3, "1 in play: +3 Actions");
    play(&mut g, id::ARISTOCRAT);
    assert_eq!(g.players[0].hand.total(), 2 + 3, "2 in play: +3 Cards");
    play(&mut g, id::ARISTOCRAT);
    assert_eq!(g.turn.coins, 3, "3 in play: +$3");
    play(&mut g, id::ARISTOCRAT);
    assert_eq!(g.turn.buys, 1 + 3, "4 in play: +3 Buys");
    // Five to eight repeat the cycle, nine and ten do nothing.
    for (n, actions, cards_drawn, coins, buys) in [(5, 3, 0, 0, 0), (6, 0, 3, 0, 0), (7, 0, 0, 3, 0), (8, 0, 0, 0, 3), (9, 0, 0, 0, 0), (10, 0, 0, 0, 0)] {
        let mut g = new_state(&[id::ARISTOCRAT, id::SMITHY], 2);
        set_in_play(&mut g, 0, &vec![id::ARISTOCRAT; n]);
        set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
        // The card played is one of those in play: the count is of the cards in play.
        set_hand(&mut g, 0, &[id::ARISTOCRAT]);
        g.players[0].in_play.remove(id::ARISTOCRAT);
        g.players[0].hand.remove(id::ARISTOCRAT);
        g.players[0].in_play.add(id::ARISTOCRAT, 1);
        g.turn.actions = 1;
        let before = g.turn;
        g.stack.push(state::Frame::new(state::FrameKind::PlayEffects, 0, id::ARISTOCRAT));
        let mut top = g.stack.top().unwrap();
        top.subject = id::ARISTOCRAT;
        top.count = 1;
        g.stack.set_top(top);
        let _ = adv(&mut g);
        assert_eq!(g.turn.actions - before.actions, actions, "{n} in play");
        assert_eq!(g.players[0].hand.total(), cards_drawn, "{n} in play");
        assert_eq!(g.turn.coins - before.coins, coins, "{n} in play");
        assert_eq!(g.turn.buys - before.buys, buys, "{n} in play");
    }
}

#[test]
fn aristocrat_counts_the_aristocrats_in_play_not_the_plays_this_turn() {
    let mut g = new_state(&[id::ARISTOCRAT, id::DAIMYO], 2);
    set_hand(&mut g, 0, &[id::DAIMYO, id::ARISTOCRAT]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
    play(&mut g, id::DAIMYO);
    play(&mut g, id::ARISTOCRAT);
    // One Aristocrat in play, played twice: +3 Actions both times.
    assert_eq!(g.turn.actions, 6);
}

#[test]
fn craftsman_takes_debt_and_gains_a_card_even_with_debt() {
    let mut g = new_state(&[id::CRAFTSMAN, id::SMITHY, id::MARKET], 2);
    g.players[0].debt = 3;
    set_hand(&mut g, 0, &[id::CRAFTSMAN]);
    play(&mut g, id::CRAFTSMAN);
    assert_eq!(g.players[0].debt, 5);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::MARKET)) && !c.contains(&Choice::Card(id::GOLD)), "up to $5");
    pick(&mut g, id::MARKET);
    assert_eq!(count(&g, 0, id::MARKET), 1);
}

#[test]
fn root_cellar_imperial_envoy_and_litter_take_debt() {
    for (card, cards_drawn, actions, buys, debt) in [(id::ROOT_CELLAR, 3, 1, 1, 3), (id::IMPERIAL_ENVOY, 5, 0, 2, 2), (id::LITTER, 2, 2, 1, 1)] {
        let mut g = new_state(&[card, id::SMITHY], 2);
        g.players[0].debt = 4;
        set_hand(&mut g, 0, &[card]);
        set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
        g.turn.actions = 1;
        // $1 to spend on the Debt: the Buy phase has a decision, so that the turn is still there to look at.
        g.turn.coins = 1;
        play(&mut g, card);
        assert_eq!(g.players[0].hand.total(), cards_drawn, "{}", cards::name(card));
        assert_eq!((g.turn.actions, g.turn.buys, g.players[0].debt), (actions, buys, 4 + debt), "{}", cards::name(card));
    }
}

#[test]
fn alley_discards_a_card_after_drawing_and_ninja_cuts_hands_to_three() {
    let mut g = new_state(&[id::ALLEY, id::NINJA], 2);
    set_hand(&mut g, 0, &[id::ALLEY, id::PROVINCE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::GOLD]);
    play(&mut g, id::ALLEY);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE, id::DUCHY]));
    assert_eq!(g.players[0].discard.get(id::ESTATE), 1);
    assert_eq!(g.turn.actions, 1);
    // Ninja: +1 Card; each other player discards down to 3.
    let mut g = new_state(&[id::ALLEY, id::NINJA], 2);
    set_hand(&mut g, 0, &[id::NINJA]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::ESTATE, id::SILVER, id::GOLD]);
    play(&mut g, id::NINJA);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    pick(&mut g, id::COPPER);
    pick(&mut g, id::COPPER);
    assert_eq!(g.players[1].hand.total(), 3);
}

#[test]
fn poet_takes_the_top_card_into_the_hand_if_it_costs_three_or_less() {
    let k = [id::POET, id::MOUNTAIN_SHRINE, id::GROWTH];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::POET]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::ESTATE, id::GOLD]);
    play(&mut g, id::POET);
    // +1 Card (Duchy), then the Estate ($2) is revealed and taken.
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY, id::ESTATE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    assert_eq!((g.rs.suns, g.turn.actions), (4, 1));
    // A more expensive card goes back on top, a card with Debt in its cost is not "$3 or less".
    for top in [id::PROVINCE, id::MOUNTAIN_SHRINE] {
        let mut g = new_state(&k, 2);
        set_hand(&mut g, 0, &[id::POET]);
        set_deck_known(&mut g, 0, &[id::ESTATE, top, id::COPPER]);
        play(&mut g, id::POET);
        assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
        assert_eq!(g.players[0].deck_known.peek_top(), Some(top));
    }
}

#[test]
fn river_shrine_trashes_up_to_two_and_gains_in_cleanup_unless_a_card_was_gained_in_the_buy_phase() {
    let k = [id::RIVER_SHRINE, id::SMITHY, id::GROWTH];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::RIVER_SHRINE, id::ESTATE, id::ESTATE, id::COPPER]);
    play(&mut g, id::RIVER_SHRINE);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.trash.get(id::ESTATE), 2);
    // No Action phase left: the Buy phase. Buying nothing, the cleanup gain comes up.
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    pick(&mut g, id::SMITHY);
    assert_eq!(count(&g, 0, id::SMITHY), 1);
    // A card gained in the Buy phase turns the gain off.
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::RIVER_SHRINE, id::SILVER, id::SILVER]);
    play(&mut g, id::RIVER_SHRINE);
    pass(&mut g);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    buy(&mut g, id::SMITHY);
    assert_eq!(count(&g, 0, id::SMITHY), 1);
    assert_eq!(g.turn.player, 1, "the turn is over: no Clean-up gain");
}

#[test]
fn every_play_of_river_shrine_gains_in_cleanup() {
    let k = [id::RIVER_SHRINE, id::THRONE_ROOM, id::SMITHY];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::RIVER_SHRINE]);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::RIVER_SHRINE);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    pass(&mut g);
    // Two plays, two gains.
    pick(&mut g, id::SMITHY);
    pick(&mut g, id::SMITHY);
    assert_eq!(count(&g, 0, id::SMITHY), 2);
}

#[test]
fn rustic_village_draws_gives_two_actions_and_discards_two_for_a_card() {
    let k = [id::RUSTIC_VILLAGE, id::SMITHY, id::GROWTH];
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::RUSTIC_VILLAGE, id::PROVINCE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::CURSE]);
    play(&mut g, id::RUSTIC_VILLAGE);
    assert_eq!((g.turn.actions, g.rs.suns), (2, 4));
    // The card drawn first can be one of the two discarded.
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::DUCHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE, id::ESTATE, id::CURSE]));
    // Declining is allowed.
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::RUSTIC_VILLAGE, id::PROVINCE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::CURSE]);
    play(&mut g, id::RUSTIC_VILLAGE);
    pass(&mut g);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn gold_mine_may_gain_a_gold_for_four_debt() {
    let mut g = new_state(&[id::GOLD_MINE, id::SMITHY], 2);
    g.players[0].debt = 2;
    set_hand(&mut g, 0, &[id::GOLD_MINE]);
    set_deck_known(&mut g, 0, &[id::COPPER]);
    play(&mut g, id::GOLD_MINE);
    assert_eq!((g.turn.actions, g.turn.buys), (1, 2));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }));
    choose(&mut g, Choice::Yes);
    assert_eq!((count(&g, 0, id::GOLD), g.players[0].debt), (1, 6), "even in Debt");
    let mut g = new_state(&[id::GOLD_MINE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GOLD_MINE]);
    play(&mut g, id::GOLD_MINE);
    choose(&mut g, Choice::No);
    assert_eq!((count(&g, 0, id::GOLD), g.players[0].debt), (0, 0));
}

#[test]
fn kitsune_does_two_different_options_in_the_order_listed() {
    let k = [id::KITSUNE, id::SMITHY, id::GROWTH];
    // +2 Actions and the Curses.
    let mut g = new_state(&k, 3);
    set_hand(&mut g, 0, &[id::KITSUNE]);
    play(&mut g, id::KITSUNE);
    assert_eq!(g.rs.suns, 7);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 2, distinct: true }), "{d:?}");
    assert_eq!(choices(&g).len(), 4);
    choose(&mut g, Choice::Mode(0));
    assert!(!has_choice(&g, Choice::Mode(0)), "two different options");
    choose(&mut g, Choice::Mode(2));
    assert_eq!(g.turn.actions, 2);
    assert_eq!((count(&g, 1, id::CURSE), count(&g, 2, id::CURSE)), (1, 1));
    // $2 and a Silver.
    let mut g = new_state(&k, 2);
    set_hand(&mut g, 0, &[id::KITSUNE]);
    play(&mut g, id::KITSUNE);
    choose(&mut g, Choice::Mode(1));
    choose(&mut g, Choice::Mode(3));
    assert_eq!((g.turn.coins, count(&g, 0, id::SILVER)), (2, 1));
}

#[test]
fn rice_broker_draws_two_for_a_treasure_and_five_for_an_action() {
    let k = [id::RICE_BROKER, id::SMITHY, id::GROWTH];
    for (trashed, cards_drawn) in [(id::COPPER, 2), (id::SMITHY, 5), (id::ESTATE, 0)] {
        let mut g = new_state(&k, 2);
        set_hand(&mut g, 0, &[id::RICE_BROKER, trashed]);
        set_deck_known(&mut g, 0, &[id::DUCHY; 8]);
        play(&mut g, id::RICE_BROKER);
        assert_eq!(g.turn.actions, 1, "+1 Action");
        // A single card in hand: trashed at once.
        assert_eq!(g.trash.get(trashed), 1);
        assert_eq!(g.players[0].hand.total(), cards_drawn, "{}", cards::name(trashed));
    }
}

#[test]
fn ronin_draws_until_seven_cards_in_hand() {
    let mut g = new_state(&[id::RONIN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RONIN, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 10]);
    play(&mut g, id::RONIN);
    assert_eq!(g.players[0].hand.total(), 7);
    let mut g = new_state(&[id::RONIN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RONIN, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 10]);
    play(&mut g, id::RONIN);
    assert_eq!(g.players[0].hand.total(), 8, "already 7 or more: no cards");
}

#[test]
fn tanuki_trashes_and_gains_up_to_two_more() {
    let mut g = new_state(&[id::TANUKI, id::SMITHY, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::TANUKI, id::SILVER]);
    play(&mut g, id::TANUKI);
    assert_eq!(g.trash.get(id::SILVER), 1);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::MARKET)) && !c.contains(&Choice::Card(id::GOLD)), "up to $5");
    pick(&mut g, id::MARKET);
    assert_eq!(count(&g, 0, id::MARKET), 1);
}

#[test]
fn samurai_attacks_once_and_gives_a_coin_at_the_start_of_every_turn() {
    let mut g = new_state(&[id::SAMURAI, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SAMURAI]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::COPPER, id::ESTATE, id::ESTATE]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    play(&mut g, id::SAMURAI);
    // The second Estate is the only card left to pick: taken at once.
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[1].hand.total(), 3);
    pass_until_turn_of(&mut g, 0);
    assert!(g.players[0].in_play.has(id::SAMURAI));
    assert_eq!(g.turn.coins, 1);
    // Next turn again: stays for the rest of the game, and the opponent is not attacked again.
    let hand_1 = g.players[1].hand.total();
    pass_until_turn_of(&mut g, 0);
    assert!(g.players[0].in_play.has(id::SAMURAI));
    assert_eq!(g.turn.coins, 1);
    assert!(g.players[1].hand.total() >= hand_1.min(5));
}

#[test]
fn rice_counts_the_different_types_among_the_cards_in_play() {
    let mut g = new_state(&[id::RICE, id::DAIMYO, id::LITTER, id::FISHMONGER], 2);
    set_in_play(&mut g, 0, &[id::DAIMYO, id::LITTER, id::FISHMONGER, id::COPPER, id::COPPER, id::COPPER]);
    set_hand(&mut g, 0, &[id::RICE]);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 3;
    g.turn.treasures_done = false;
    expect_decision(&mut g);
    // Action, Command, Shadow and Treasure: 4 types, Rice itself included.
    assert_eq!((g.turn.coins, g.turn.buys), (3 + 4, 2));
}

#[test]
fn tea_house_is_a_cantrip_with_two_coins() {
    let mut g = new_state(&[id::TEA_HOUSE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::TEA_HOUSE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::TEA_HOUSE);
    assert_eq!((g.turn.coins, g.turn.actions, g.players[0].hand.total()), (2, 1, 1));
}

#[test]
fn riverboat_sets_aside_a_five_cost_action_in_set_up_and_plays_it_at_the_start_of_the_next_turn() {
    let g = new_state(&[id::RIVERBOAT, id::SMITHY, id::VILLAGE], 2);
    let card = g.rs.riverboat;
    assert!(card != 0 && cards::cost(card) == 5 && cards::is(card, cards::ACTION) && !cards::is(card, cards::DURATION), "{}", cards::name(card));
    assert!(!g.in_supply(card), "not in the Supply");
    // A named card is used; one that does not fit is not.
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::RIVERBOAT, id::SMITHY], riverboat: Some(id::MARKET), seed: 1, ..Default::default() };
    assert_eq!(GameState::new(&cfg).rs.riverboat, id::MARKET);
    let cfg = GameConfig { riverboat: Some(id::SMITHY), ..cfg };
    assert_ne!(GameState::new(&cfg).rs.riverboat, id::SMITHY, "Smithy is in the kingdom");
    // Played from the Supply at the start of the next turn, where it stays.
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::RIVERBOAT, id::SMITHY], riverboat: Some(id::MARKET), seed: 1, ..Default::default() };
    let mut g = GameState::new(&cfg);
    let _ = g.advance(&mut NoEvents);
    reset_turn(&mut g, 0);
    for p in 0..2 {
        g.players[p] = state::PlayerState::default();
        set_deck_unknown(&mut g, p, &[id::ESTATE; 14]);
    }
    set_hand(&mut g, 0, &[id::RIVERBOAT]);
    play(&mut g, id::RIVERBOAT);
    pass_until_turn_of(&mut g, 0);
    // Market: +1 Card, +1 Action, +1 Buy, +$1.
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 1));
    assert_eq!(g.players[0].hand.total(), 5 + 1);
    assert_eq!(g.rs.riverboat, id::MARKET);
    assert_eq!(count(&g, 0, id::MARKET), 0, "the set-aside card stays out of the decks");
    assert!(g.players[0].in_play.has(id::RIVERBOAT));
}

// ===========================================================================
// Shadow cards
// ===========================================================================

fn draw_cards(g: &mut GameState, p: u8, n: u8) {
    g.pending = Pending::None;
    g.stack.push(state::Frame { max: n, ..state::Frame::new(state::FrameKind::Draw, p, 0) });
    let _ = adv(g);
}


#[test]
fn a_gained_shadow_is_not_put_on_the_bottom_until_the_next_shuffle() {
    let mut g = new_state(&[id::ALLEY, id::SMITHY], 2);
    g.gain_card(0, id::ALLEY, Dest::DeckTop, &mut NoEvents);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ALLEY));
    g.gain_card(0, id::ALLEY, Dest::Discard, &mut NoEvents);
    assert_eq!(g.players[0].discard.get(id::ALLEY), 1);
    assert_eq!(g.players[0].deck_known_bottom.len, 0);
}

#[test]
fn a_shadow_card_in_the_deck_can_be_played_as_if_in_the_hand() {
    // In the unknown middle, on the known top and at the known bottom: wherever it is.
    for place in 0..3 {
        let mut g = new_state(&[id::ALLEY, id::SMITHY], 2);
        set_hand(&mut g, 0, &[id::ESTATE, id::DUCHY]);
        match place {
            0 => set_deck_unknown(&mut g, 0, &[id::ALLEY, id::PROVINCE, id::PROVINCE]),
            1 => set_deck_known(&mut g, 0, &[id::PROVINCE, id::CURSE, id::ALLEY]),
            _ => {
                set_deck_unknown(&mut g, 0, &[id::PROVINCE, id::PROVINCE]);
                g.players[0].deck_known_bottom.push_top(id::ALLEY);
            }
        }
        let d = expect_decision(&mut g);
        assert_eq!(d.kind, DecisionKind::PlayAction, "no Action in the hand, but a Shadow in the deck");
        assert!(has_choice(&g, Choice::Card(id::ALLEY)));
        let size = g.players[0].deck_size();
        play(&mut g, id::ALLEY);
        assert!(g.players[0].in_play.has(id::ALLEY) && !g.players[0].deck_has(id::ALLEY), "place {place}");
        // +1 Card (from the deck), +1 Action, discard a card; the deck lost the Alley and the card drawn.
        assert_eq!(g.players[0].deck_size(), size - 2, "place {place}");
        assert_eq!(g.turn.actions, 1);
        assert_eq!(g.players[0].hand.total(), 3, "the discard is still to be picked");
    }
}

#[test]
fn a_shadow_play_uses_an_action_and_prefers_the_copy_in_the_deck() {
    let mut g = new_state(&[id::FISHMONGER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::FISHMONGER, id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::FISHMONGER, id::PROVINCE]);
    play(&mut g, id::FISHMONGER);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (0, 2, 1));
    assert!(g.players[0].hand.has(id::FISHMONGER), "the copy in the hand stays");
    assert!(!g.players[0].deck_has(id::FISHMONGER));
}

#[test]
fn throne_room_can_play_a_shadow_card_from_the_deck_twice() {
    let mut g = new_state(&[id::FISHMONGER, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::FISHMONGER, id::PROVINCE]);
    play(&mut g, id::THRONE_ROOM);
    assert!(has_choice(&g, Choice::Card(id::FISHMONGER)));
    pick(&mut g, id::FISHMONGER);
    assert_eq!((g.turn.buys, g.turn.coins), (3, 2));
    assert_eq!(g.players[0].in_play.get(id::FISHMONGER), 1);
}


// ===========================================================================
// The Prophecies
// ===========================================================================

#[test]
fn approaching_army_gives_a_coin_after_each_attack() {
    let mut g = active(&[id::MILITIA, id::SMITHY, id::SAMURAI, id::TEA_HOUSE], id::APPROACHING_ARMY, 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::MILITIA, id::SMITHY, id::SAMURAI]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.coins, 0, "not an Attack");
    play(&mut g, id::MILITIA);
    // (The other player has to discard first.)
    while g.pending_decision().is_some_and(|d| d.player == 1) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.turn.coins, 3, "Militia's $2 and the $1");
}

#[test]
fn approaching_army_gives_the_coin_for_a_duration_attack_only_on_the_turn_it_is_played() {
    let mut g = active(&[id::SAMURAI, id::SMITHY], id::APPROACHING_ARMY, 2);
    set_hand(&mut g, 0, &[id::SAMURAI]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 14]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 14]);
    set_hand(&mut g, 1, &[id::ESTATE; 5]);
    play(&mut g, id::SAMURAI);
    while g.pending_decision().is_some_and(|d| d.player == 1) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(g.turn.coins, 1, "the $1 of Approaching Army");
    pass_until_turn_of(&mut g, 0);
    assert_eq!(g.turn.coins, 1, "next turn only the Samurai's own $1");
}

#[test]
fn approaching_army_adds_an_attack_pile_to_a_kingdom_with_an_omen() {
    let mut n_with = 0;
    for seed in 0..200u64 {
        let mut rng = Rng::new(seed);
        let k = cards::random_kingdom_with_colonies(&[CardSet::RisingSun], &[id::TEA_HOUSE], &mut rng);
        if k.contains(&id::APPROACHING_ARMY) {
            n_with += 1;
            assert!(k.iter().any(|&c| c != id::TEA_HOUSE && cards::is_kingdom(c) && cards::is(c, cards::ATTACK)), "{k:?}");
        }
    }
    assert!(n_with > 3);
}



#[test]
fn divine_wind_replaces_the_kingdom_piles_with_ten_new_ones() {
    let old = [id::TEA_HOUSE, id::SMITHY, id::VILLAGE, id::WITCH, id::MARKET, id::MILITIA, id::MOAT, id::CELLAR, id::MINE, id::REMODEL];
    let mut k = old.to_vec();
    k.push(id::DIVINE_WIND);
    let mut g = new_state(&k, 2);
    g.rs.suns = 1;
    g.emp.piles.add_debt(id::SMITHY, 2);
    set_hand(&mut g, 0, &[id::TEA_HOUSE, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    g.supply.set(id::SMITHY, 3);
    let total_before: u32 = g.players[0].all_cards().total();
    play(&mut g, id::TEA_HOUSE);
    assert_eq!(g.rs.active, id::DIVINE_WIND);
    for c in old {
        assert!(!g.in_supply(c) && g.supply.get(c) == 0, "{} left the Supply", cards::name(c));
    }
    assert_eq!(g.emp.piles.debt_on(id::SMITHY), 0, "no tokens on a removed pile");
    // Ten new piles, none of them cards of the old kingdom, from the sets of the game; the basic cards stay.
    let new: Vec<CardId> = g.supply_cards().filter(|&c| cards::is_kingdom(c)).collect();
    assert_eq!(new.len(), 10, "{:?}", new.iter().map(|&c| cards::name(c)).collect::<Vec<_>>());
    assert!(new.iter().all(|c| !old.contains(c)));
    assert!(g.in_supply(id::COPPER) && g.in_supply(id::PROVINCE) && g.in_supply(id::CURSE));
    // The cards the players have stay with them (the Tea House played, the Smithy in hand).
    assert_eq!(g.players[0].all_cards().total(), total_before, "nothing moved but the Estate drawn");
    assert!(g.players[0].hand.has(id::SMITHY) && g.players[0].in_play.has(id::TEA_HOUSE));
}


#[test]
fn enlightenment_treasures_trashed_to_rice_broker_count_as_both() {
    let mut g = active(&[id::RICE_BROKER, id::SMITHY], id::ENLIGHTENMENT, 2);
    set_hand(&mut g, 0, &[id::RICE_BROKER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 8]);
    play(&mut g, id::RICE_BROKER);
    // Trashing the lone Copper: +2 Cards for a Treasure and +5 Cards for an Action.
    assert_eq!(g.players[0].hand.total(), 7);
}



#[test]
fn great_leader_gives_an_action_after_each_action_including_the_omen_that_made_it_happen() {
    let mut g = active(&[id::SMITHY, id::TEA_HOUSE], id::GREAT_LEADER, 2);
    set_hand(&mut g, 0, &[id::SMITHY, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 8]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.actions, 1, "1 - 1 + 1");
    play(&mut g, id::SMITHY);
    assert_eq!(g.turn.actions, 1);
    let mut g = new_state(&[id::TEA_HOUSE, id::SMITHY, id::GREAT_LEADER], 2);
    g.rs.suns = 1;
    set_hand(&mut g, 0, &[id::TEA_HOUSE]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::TEA_HOUSE);
    assert_eq!(g.turn.actions, 2, "the Tea House's own +1 Action and Great Leader's");
}

#[test]
fn growth_gains_a_cheaper_card_when_a_treasure_is_gained_and_chains() {
    let mut g = active(&[id::SMITHY], id::GROWTH, 2);
    buy_phase(&mut g, 6, 1);
    buy(&mut g, id::GOLD);
    // Mandatory: a card costing less than $6.
    assert!(has_choice(&g, Choice::Card(id::SILVER)) && !has_choice(&g, Choice::Card(id::GOLD)) && !has_choice(&g, Choice::Card(id::PROVINCE)));
    pick(&mut g, id::SILVER);
    // The Silver is a Treasure too: a cheaper card than $3.
    assert!(has_choice(&g, Choice::Card(id::ESTATE)) && !has_choice(&g, Choice::Card(id::SILVER)));
    pick(&mut g, id::ESTATE);
    assert_eq!((count(&g, 0, id::GOLD), count(&g, 0, id::SILVER), count(&g, 0, id::ESTATE)), (1, 1, 1));
}







fn pass_until_sickness(g: &mut GameState, player: u8) {
    for _ in 0..50 {
        let d = expect_decision(g);
        if g.turn.player == player && matches!(d.kind, DecisionKind::Mode { .. }) {
            return;
        }
        let c = choices(g)[0];
        let c = if choices(g).contains(&Choice::Pass) { Choice::Pass } else { c };
        choose(g, c);
    }
    panic!("no Sickness decision");
}

#[test]
fn shadow_cards_go_to_the_bottom_of_the_deck_when_the_discard_pile_is_shuffled() {
    let mut g = new_state(&[id::ALLEY, id::FISHMONGER, id::SMITHY], 2);
    set_discard(&mut g, 0, &[id::ALLEY, id::FISHMONGER, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    draw_cards(&mut g, 0, 5);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE; 5]), "the Shadows are not drawn");
    assert_eq!(g.players[0].deck_known_bottom.len, 2);
    assert_eq!(g.players[0].deck_size(), 3);
    draw_cards(&mut g, 0, 3);
    assert_eq!(g.players[0].hand.get(id::ALLEY) + g.players[0].hand.get(id::FISHMONGER), 2, "drawn last, at the end of the deck");
    assert_eq!(g.players[0].deck_size(), 0);
}

#[test]
fn the_secret_passage_bottom_card_is_drawn_once_the_deck_is_gone() {
    // (A card put at the bottom was never drawn before the Shadow cards used the same place.)
    let mut g = new_state(&[id::SECRET_PASSAGE, id::SMITHY], 2);
    set_deck_unknown(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    g.players[0].deck_known_bottom.push_top(id::PROVINCE);
    draw_cards(&mut g, 0, 5);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::ESTATE, id::PROVINCE]));
}

/// Plays the Buy phase of the turn (the Treasures played, then pass).
fn end_turn(g: &mut GameState) {
    adv(g);
    while g.pending_decision().is_some_and(|d| d.player == 0 && g.turn.player == 0) {
        let c = if choices(g).contains(&Choice::Pass) { Choice::Pass } else { choices(g)[0] };
        choose(g, c);
    }
}

#[test]
fn biding_time_sets_the_hand_aside_at_cleanup_and_returns_it_at_the_start_of_the_next_turn() {
    let mut g = active(&[id::SMITHY, id::TEA_HOUSE], id::BIDING_TIME, 2);
    set_hand(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::CURSE, id::ESTATE, id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    end_turn(&mut g);
    // The opponent's turn: the hand is set aside, face down; the new hand of five was drawn.
    assert_eq!(g.turn.player, 1);
    assert_eq!(g.players[0].aside.total(), 5);
    assert_eq!(g.players[0].hand.total(), 5);
    let view = PlayerView::new(&g, 1);
    assert!(view.aside_of(0).is_empty() && view.aside_count_of(0) == 5, "private to its owner");
    pass_until_turn_of(&mut g, 0);
    assert_eq!(g.players[0].hand.total(), 10, "the set-aside hand is back");
    assert!(g.players[0].hand.has(id::DUCHY) && g.players[0].aside.is_empty());
}

#[test]
fn bureaucracy_gains_a_copper_with_every_card_that_does_not_cost_zero() {
    let mut g = active(&[id::SMITHY, id::MOUNTAIN_SHRINE], id::BUREAUCRACY, 2);
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    assert_eq!((g.players[0].discard.get(id::SILVER), g.players[0].discard.get(id::COPPER)), (1, 1));
    g.gain_card(0, id::COPPER, Dest::Discard, &mut NoEvents);
    assert_eq!(g.players[0].discard.get(id::COPPER), 2, "a Copper costs $0: no further Copper");
    g.gain_card(0, id::CURSE, Dest::Discard, &mut NoEvents);
    assert_eq!(g.players[0].discard.get(id::COPPER), 2);
    // A card with Debt in its cost does not cost exactly $0, and any player's gain counts.
    g.gain_card(1, id::MOUNTAIN_SHRINE, Dest::Discard, &mut NoEvents);
    assert_eq!(g.players[1].discard.get(id::COPPER), 1);
}

#[test]
fn enlightenment_makes_treasures_actions_that_draw_and_give_an_action_in_the_action_phase() {
    let mut g = active(&[id::SMITHY, id::RICE_BROKER], id::ENLIGHTENMENT, 2);
    set_hand(&mut g, 0, &[id::GOLD, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    assert!(has_choice(&g, Choice::Card(id::GOLD)) && has_choice(&g, Choice::Card(id::COPPER)) && !has_choice(&g, Choice::Card(id::ESTATE)));
    play(&mut g, id::GOLD);
    // +1 Card and +1 Action instead of the $3.
    assert_eq!((g.turn.coins, g.turn.actions, g.players[0].hand.total()), (0, 1, 3));
    // In the Buy phase a Treasure is a Treasure.
    let mut g = active(&[id::SMITHY], id::ENLIGHTENMENT, 2);
    set_hand(&mut g, 0, &[id::GOLD, id::COPPER]);
    adv(&mut g);
    pass(&mut g);
    assert_eq!(g.turn.coins, 4);
}

#[test]
fn flourishing_trade_lowers_costs_and_lets_actions_be_used_as_buys() {
    let mut g = active(&[id::SMITHY, id::MARKET], id::FLOURISHING_TRADE, 2);
    assert_eq!((g.cost(id::SMITHY), g.cost(id::PROVINCE), g.cost(id::COPPER)), (3, 7, 0));
    // One Buy and an unused Action: two purchases.
    set_hand(&mut g, 0, &[id::ESTATE]);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 6;
    g.turn.treasures_done = true;
    assert_eq!(g.buys_left(), 2);
    buy(&mut g, id::SMITHY);
    assert_eq!((g.turn.buys, g.turn.actions), (0, 1), "the Buy goes first");
    assert!(has_choice(&g, Choice::Card(id::SMITHY)), "the Action play is a Buy");
    buy(&mut g, id::SMITHY);
    assert_eq!(count(&g, 0, id::SMITHY), 2);
    assert_eq!(g.turn.player, 1, "no Buy left: the turn is over");
    // Every turn starts with the reduction.
    assert_eq!(g.cost(id::SMITHY), 3);
}

#[test]
fn good_harvest_gives_a_buy_and_a_coin_for_the_first_play_of_each_treasure_each_turn() {
    let mut g = active(&[id::SMITHY], id::GOOD_HARVEST, 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::SILVER]);
    adv(&mut g);
    // 4 Coppers, a Silver and the bonus for each of the two names: +2 Buys, +$2.
    assert_eq!((g.turn.coins, g.turn.buys), (4 + 2 + 2, 3));
    // A Treasure played before the Prophecy happened this turn does not count again.
    let mut g = new_state(&[id::TEA_HOUSE, id::GOOD_HARVEST], 2);
    g.rs.suns = 1;
    set_hand(&mut g, 0, &[id::TEA_HOUSE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    g.turn.played.add(id::COPPER, 1);
    play(&mut g, id::TEA_HOUSE);
    assert_eq!(g.rs.active, id::GOOD_HARVEST);
    adv(&mut g);
    assert_eq!((g.turn.coins, g.turn.buys), (2 + 1, 1));
}

#[test]
fn harsh_winter_puts_debt_on_a_pile_and_the_next_gain_from_it_takes_it() {
    let mut g = active(&[id::SMITHY], id::HARSH_WINTER, 2);
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    assert_eq!((g.emp.piles.debt_on(id::SILVER), g.players[0].debt), (2, 0));
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    assert_eq!((g.emp.piles.debt_on(id::SILVER), g.players[0].debt), (0, 2), "took the Debt");
    // Other players' gains do nothing.
    g.gain_card(1, id::GOLD, Dest::Discard, &mut NoEvents);
    assert_eq!((g.emp.piles.debt_on(id::GOLD), g.players[1].debt), (0, 0));
}

#[test]
fn kind_emperor_gains_an_action_to_the_hand_when_it_happens_and_at_the_start_of_every_turn() {
    let mut g = new_state(&[id::TEA_HOUSE, id::SMITHY, id::MOUNTAIN_SHRINE, id::KIND_EMPEROR], 2);
    g.rs.suns = 1;
    set_hand(&mut g, 0, &[id::TEA_HOUSE]);
    set_deck_known(&mut g, 0, &[id::ESTATE]);
    play(&mut g, id::TEA_HOUSE);
    // Any Action, whatever its cost (a card with Debt in its cost too), to the hand.
    assert!(has_choice(&g, Choice::Card(id::MOUNTAIN_SHRINE)) && has_choice(&g, Choice::Card(id::SMITHY)) && !has_choice(&g, Choice::Card(id::SILVER)));
    pick(&mut g, id::MOUNTAIN_SHRINE);
    assert!(g.players[0].hand.has(id::MOUNTAIN_SHRINE));
    // At the start of each turn.
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    g.players[0].hand = Counts::EMPTY;
    g.turn.treasures_done = true;
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    pick(&mut g, id::SMITHY);
    assert!(g.players[1].hand.has(id::SMITHY));
}

#[test]
fn panic_gives_buys_for_playing_treasures_and_returns_them_to_their_piles() {
    let mut g = active(&[id::SMITHY], id::PANIC, 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::SILVER]);
    let (copper, silver) = (g.supply.get(id::COPPER), g.supply.get(id::SILVER));
    adv(&mut g);
    assert_eq!((g.turn.coins, g.turn.buys), (4, 1 + 2 * 3));
    pass(&mut g);
    // Cleanup: the Treasures went back to their piles, not to the discard pile.
    assert_eq!((g.supply.get(id::COPPER), g.supply.get(id::SILVER)), (copper + 2, silver + 1));
    assert_eq!(g.players[0].discard.total(), 0);
}

#[test]
fn progress_puts_every_gained_card_onto_the_deck() {
    let mut g = active(&[id::SMITHY, id::WORKSHOP], id::PROGRESS, 2);
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    assert_eq!(g.players[0].discard.total(), 0);
    // On someone else's turn too, and for a card gained to the hand.
    g.gain_card(1, id::GOLD, Dest::Hand, &mut NoEvents);
    assert_eq!((g.players[1].deck_known.peek_top(), g.players[1].hand.total()), (Some(id::GOLD), 0));
}

#[test]
fn rapid_expansion_sets_aside_gained_actions_and_treasures_to_be_played_next_turn() {
    let mut g = active(&[id::SMITHY, id::TEA_HOUSE], id::RAPID_EXPANSION, 2);
    g.turn.turn_start_resolved = true;
    g.gain_card(0, id::SMITHY, Dest::Discard, &mut NoEvents);
    g.gain_card(0, id::GOLD, Dest::Discard, &mut NoEvents);
    g.gain_card(0, id::DUCHY, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.players[0].aside.get(id::SMITHY) + g.players[0].aside.get(id::GOLD), 2);
    assert_eq!(g.players[0].discard.get(id::DUCHY), 1, "Victory cards are not set aside");
    assert_eq!(g.players[0].discard.get(id::SMITHY) + g.players[0].discard.get(id::GOLD), 0);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 15]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 15]);
    set_hand(&mut g, 0, &[]);
    end_turn(&mut g);
    pass_until_turn_of(&mut g, 0);
    // (The two cards set aside are played in an order of the player's choice.)
    while g.pending_decision().is_some_and(|d| d.kind == DecisionKind::StartOrder) {
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    // The Smithy was played at the start of the turn (+3 Cards); the Gold is in play, its $ lost (no Treasure play in the Action phase).
    assert_eq!(g.players[0].hand.total(), 5 + 3);
    assert!(g.players[0].aside.is_empty() && g.players[0].in_play.has(id::SMITHY) && g.players[0].in_play.has(id::GOLD));
}

#[test]
fn sickness_asks_at_the_start_of_every_turn_for_a_curse_on_the_deck_or_three_discards() {
    let mut g = active(&[id::SMITHY], id::SICKNESS, 2);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    set_hand(&mut g, 0, &[id::DUCHY]);
    end_turn(&mut g);
    // Player 2's turn starts: Sickness.
    assert_eq!(g.turn.player, 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[1].deck_known.peek_top(), Some(id::CURSE));
    pass_until_sickness(&mut g, 0);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.players[0].discard.total() >= 3, true);
}

// ===========================================================================
// The Events
// ===========================================================================

fn event_game(events: &[CardId]) -> GameState {
    let mut k = vec![id::SMITHY, id::VILLAGE, id::MARKET, id::MILITIA, id::MOUNTAIN_SHRINE];
    k.extend_from_slice(events);
    let mut g = new_state(&k, 2);
    g.turn.turn_start_resolved = true;
    g
}

#[test]
fn amass_gains_an_action_up_to_five_only_with_no_action_in_play() {
    let mut g = event_game(&[id::AMASS]);
    buy_phase(&mut g, 2, 1);
    adv(&mut g);
    assert!(has_choice(&g, Choice::Card(id::AMASS)));
    pick(&mut g, id::AMASS);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::MARKET)) && !c.contains(&Choice::Card(id::SILVER)), "Actions only");
    pick(&mut g, id::MARKET);
    assert_eq!(count(&g, 0, id::MARKET), 1);
    let mut g = event_game(&[id::AMASS]);
    set_in_play(&mut g, 0, &[id::VILLAGE]);
    buy_phase(&mut g, 2, 1);
    adv(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::AMASS)), "an Action in play: Amass does nothing");
}

#[test]
fn asceticism_pays_any_amount_to_trash_that_many_cards() {
    let mut g = event_game(&[id::ASCETICISM]);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::CURSE]);
    buy_phase(&mut g, 4, 1);
    adv(&mut g);
    pick(&mut g, id::ASCETICISM);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Overpay { max_coins: 2, .. }), "$2 left: {d:?}");
    choose(&mut g, Choice::Overpay(2));
    pick(&mut g, id::CURSE);
    assert_eq!(g.trash.total(), 2);
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn continue_returns_to_the_action_phase_and_plays_the_card_gained() {
    let mut g = event_game(&[id::CONTINUE]);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    buy_phase(&mut g, 1, 1);
    adv(&mut g);
    pick(&mut g, id::CONTINUE);
    assert_eq!(g.players[0].debt, 8);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && !c.contains(&Choice::Card(id::MILITIA)), "a non-Attack Action up to $4");
    pick(&mut g, id::SMITHY);
    // Back in the Action phase: +1 Action and +1 Buy, and the Smithy was played from the discard pile.
    assert!(g.players[0].in_play.has(id::SMITHY));
    assert_eq!(g.players[0].hand.total(), 1 + 2, "Smithy drew what was left");
    assert_eq!(g.turn.buys, 1, "the Buy was used and the Event gave one back");
    // Once per turn.
    g.turn.phase = Phase::Buy;
    g.players[0].debt = 0;
    g.turn.coins = 0;
    g.turn.treasures_done = true;
    g.pending = Pending::None;
    adv(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::CONTINUE)));
}

#[test]
fn credit_gains_an_action_or_treasure_up_to_eight_and_takes_debt_equal_to_its_cost() {
    let mut g = event_game(&[id::CREDIT]);
    buy_phase(&mut g, 2, 1);
    adv(&mut g);
    pick(&mut g, id::CREDIT);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::GOLD)) && c.contains(&Choice::Card(id::SMITHY)) && !c.contains(&Choice::Card(id::PROVINCE)));
    assert!(!c.contains(&Choice::Card(id::MOUNTAIN_SHRINE)), "no Debt in the cost");
    pick(&mut g, id::GOLD);
    assert_eq!((count(&g, 0, id::GOLD), g.players[0].debt), (1, 6));
}

#[test]
fn foresight_sets_the_first_action_aside_and_puts_it_into_the_next_hand() {
    let mut g = event_game(&[id::FORESIGHT]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER, id::SMITHY, id::GOLD]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    buy_phase(&mut g, 2, 2);
    adv(&mut g);
    pick(&mut g, id::FORESIGHT);
    assert_eq!(g.players[0].foresight.get(id::SMITHY), 1);
    assert_eq!(g.players[0].discard.get(id::ESTATE) + g.players[0].discard.get(id::COPPER), 2, "the others revealed are discarded");
    pass_until_turn_of(&mut g, 1);
    assert!(g.players[0].hand.has(id::SMITHY) && g.players[0].foresight.is_empty());
    assert_eq!(g.players[0].hand.total(), 6);
}

#[test]
fn gather_gains_cards_costing_exactly_three_four_and_five() {
    let mut g = event_game(&[id::GATHER]);
    buy_phase(&mut g, 7, 1);
    adv(&mut g);
    pick(&mut g, id::GATHER);
    assert!(has_choice(&g, Choice::Card(id::SILVER)) && !has_choice(&g, Choice::Card(id::SMITHY)), "exactly $3");
    pick(&mut g, id::SILVER);
    pick(&mut g, id::SMITHY);
    pick(&mut g, id::MARKET);
    assert_eq!((count(&g, 0, id::SILVER), count(&g, 0, id::SMITHY), count(&g, 0, id::MARKET)), (1, 1, 1));
}

#[test]
fn kintsugi_trashes_and_remodels_once_a_gold_has_been_gained() {
    let mut g = event_game(&[id::KINTSUGI]);
    set_hand(&mut g, 0, &[id::ESTATE, id::COPPER]);
    buy_phase(&mut g, 3, 1);
    adv(&mut g);
    pick(&mut g, id::KINTSUGI);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.trash.get(id::ESTATE), 1);
    assert!(g.pending_decision().map_or(true, |d| d.player == 1 || !matches!(d.kind, DecisionKind::Gain { .. })), "no Gold gained yet: just a trasher");
    let mut g = event_game(&[id::KINTSUGI]);
    g.gain_card(0, id::GOLD, Dest::Discard, &mut NoEvents);
    g.players[0].discard.remove(id::GOLD);
    set_hand(&mut g, 0, &[id::ESTATE, id::COPPER]);
    buy_phase(&mut g, 3, 1);
    adv(&mut g);
    pick(&mut g, id::KINTSUGI);
    pick(&mut g, id::ESTATE);
    assert!(has_choice(&g, Choice::Card(id::SMITHY)) && !has_choice(&g, Choice::Card(id::MARKET)), "up to $4: $2 more");
    pick(&mut g, id::SMITHY);
    assert_eq!(count(&g, 0, id::SMITHY), 1);
}

#[test]
fn practice_plays_an_action_from_the_hand_twice() {
    let mut g = event_game(&[id::PRACTICE]);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 8]);
    buy_phase(&mut g, 3, 2);
    adv(&mut g);
    pick(&mut g, id::PRACTICE);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 6);
}

#[test]
fn receive_tribute_needs_three_gains_and_gains_up_to_three_different_actions_not_in_play() {
    let mut g = event_game(&[id::RECEIVE_TRIBUTE]);
    set_in_play(&mut g, 0, &[id::VILLAGE]);
    buy_phase(&mut g, 5, 2);
    adv(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::RECEIVE_TRIBUTE)), "fewer than three cards gained");
    for _ in 0..3 {
        g.gain_card(0, id::COPPER, Dest::Discard, &mut NoEvents);
    }
    adv(&mut g);
    pick(&mut g, id::RECEIVE_TRIBUTE);
    let c = choices(&g);
    assert!(!c.contains(&Choice::Card(id::VILLAGE)), "a copy is in play");
    assert!(c.contains(&Choice::Card(id::MOUNTAIN_SHRINE)) && c.contains(&Choice::Pass), "any cost, optional");
    pick(&mut g, id::MOUNTAIN_SHRINE);
    assert_eq!(g.players[0].debt, 0, "no Debt for a card gained this way");
    assert!(!has_choice(&g, Choice::Card(id::MOUNTAIN_SHRINE)), "differently named");
    pick(&mut g, id::SMITHY);
    assert!(!has_choice(&g, Choice::Card(id::SMITHY)) && !has_choice(&g, Choice::Card(id::MOUNTAIN_SHRINE)));
    pick(&mut g, id::MARKET);
    assert_eq!(count(&g, 0, id::MARKET) + count(&g, 0, id::SMITHY) + count(&g, 0, id::MOUNTAIN_SHRINE), 3);
}

#[test]
fn sea_trade_draws_a_card_per_action_in_play_and_trashes_up_to_that_many() {
    let mut g = event_game(&[id::SEA_TRADE]);
    set_in_play(&mut g, 0, &[id::VILLAGE, id::SMITHY]);
    set_hand(&mut g, 0, &[id::ESTATE, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::PROVINCE]);
    buy_phase(&mut g, 4, 1);
    adv(&mut g);
    pick(&mut g, id::SEA_TRADE);
    assert_eq!(g.players[0].hand.total(), 4);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    assert_eq!(g.trash.total(), 2);
}

#[test]
fn divine_wind_is_disabled_for_now() {
    // Not dealt by the set-up rule, and a kingdom may not name it.
    assert!(!cards::is_ready(id::DIVINE_WIND));
    let mut rng = dominion_engine::rng::Rng::new(5);
    for _ in 0..2000 {
        let mut k = vec![id::KITSUNE, id::SMITHY, id::VILLAGE, id::MARKET, id::MOAT, id::CELLAR, id::WORKSHOP, id::REMODEL, id::MINE, id::MILITIA];
        cards::add_random_prophecy(&mut k, &[cards::CardSet::Base, cards::CardSet::RisingSun], &mut rng);
        assert!(!k.contains(&id::DIVINE_WIND));
    }
    let text = "players: 2\nkingdom: Kitsune, Smithy, Village, Market, Moat, Cellar, Workshop, Remodel, Mine, Militia, Divine Wind\n";
    assert!(parse_state(text).is_err());
}

// ===========================================================================
// The stop-moving rule (docs/stop-moving-rule.md)
// ===========================================================================

#[test]
fn a_daimyo_replay_of_mining_village_cannot_trash_another_mining_village() {
    let mut g = new_state(&[id::DAIMYO, id::MINING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::DAIMYO, id::MINING_VILLAGE]);
    set_in_play(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 10]);
    g.turn.actions = 2;
    play(&mut g, id::DAIMYO);
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::Yes); // the 1st play trashes it
    // The Daimyo's replay: its Mining Village is gone; the one played earlier is another card.
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })), "{:?}", g.pending_decision());
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.players[0].in_play.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.turn.led.n, 0);
}

// ===========================================================================
// Audit: rulings of the wiki's FAQ pages that had no test
// ===========================================================================

/// Plays Continue as the only buy of a Buy phase with Debt 0 and `coins` $ in a game with `extra` in the kingdom.
fn continue_game(extra: &[CardId]) -> GameState {
    let mut k = vec![id::RIVER_SHRINE, id::SMITHY, id::CONTINUE];
    k.extend_from_slice(extra);
    let mut g = new_state(&k, 2);
    g.turn.turn_start_resolved = true;
    g
}

#[test]
fn river_shrine_does_not_gain_in_cleanup_when_continue_gained_a_card_in_the_buy_phase() {
    // "Normally, buying Continue will make you gain a card, which means River Shrine can't gain a card."
    let mut g = continue_game(&[]);
    set_hand(&mut g, 0, &[id::RIVER_SHRINE]);
    play(&mut g, id::RIVER_SHRINE);
    buy_phase(&mut g, 0, 1);
    adv(&mut g);
    pick(&mut g, id::CONTINUE);
    pick(&mut g, id::SMITHY);
    // Back in the Action phase with the Smithy played; end the turn with no further buy.
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1, "no Clean-up gain was offered: {:?}", g.pending_decision());
}

#[test]
fn continue_can_be_bought_when_no_non_attack_action_up_to_four_is_left() {
    // "buying Continue when there are no non-Attack Actions costing up to $4 remaining in the Supply": legal, you still return to the Action phase.
    let mut g = continue_game(&[]);
    empty_pile(&mut g, id::SMITHY);
    set_hand(&mut g, 0, &[id::ESTATE]);
    buy_phase(&mut g, 0, 1);
    adv(&mut g);
    assert!(has_choice(&g, Choice::Card(id::CONTINUE)), "Continue is a legal buy even when it can gain nothing");
    pick(&mut g, id::CONTINUE);
    assert_eq!(g.turn.phase, Phase::Action);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 1));
}

#[test]
fn foresight_can_be_bought_with_no_action_in_the_deck_and_reveals_the_whole_deck() {
    // "Reveal cards from your deck until revealing an Action. Set it aside and discard the rest": with none, the whole deck is revealed and discarded.
    let mut g = event_game(&[id::FORESIGHT]);
    set_hand(&mut g, 0, &[id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER]);
    clear_discard(&mut g);
    buy_phase(&mut g, 2, 2);
    adv(&mut g);
    assert!(has_choice(&g, Choice::Card(id::FORESIGHT)));
    pick(&mut g, id::FORESIGHT);
    assert_eq!(g.players[0].discard.total(), 2, "{:?} {:?} {:?}", g.players[0].set_aside, g.players[0].deck_known.peek_top(), g.pending_decision());
}

fn clear_discard(g: &mut GameState) {
    g.players[0].discard = Counts::EMPTY;
}

#[test]
#[ignore = "audit: Events that can do nothing right now (Amass with an Action in play, Kintsugi with an empty hand...) are hidden from the Buy choices, not offered as legal no-op buys"]
fn amass_with_an_action_in_play_may_be_bought_and_gains_nothing() {
    // "If you have no Action cards in play, gain an Action card": buying it with one in play is legal and does nothing.
    let mut g = event_game(&[id::AMASS]);
    set_in_play(&mut g, 0, &[id::SMITHY]);
    buy_phase(&mut g, 2, 1);
    adv(&mut g);
    assert!(has_choice(&g, Choice::Card(id::AMASS)), "legal, if pointless");
}

#[test]
fn sickness_offers_both_options_with_an_empty_curse_pile_and_a_small_hand() {
    // "You can choose to gain a Curse even if the Curse pile is empty, or to discard cards even if you have fewer than 3 cards in hand."
    let mut g = active(&[id::SMITHY], id::SICKNESS, 2);
    empty_pile(&mut g, id::CURSE);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    set_hand(&mut g, 0, &[id::DUCHY]);
    end_turn(&mut g);
    assert_eq!(g.turn.player, 1);
    set_hand(&mut g, 1, &[id::COPPER]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Mode(0)) && c.contains(&Choice::Mode(1)), "{c:?}");
}

#[test]
fn panic_returns_a_loot_treasure_to_the_loot_pile() {
    // "Loot (from Plunder) is returned to the top of the pile."
    let mut g = active(&[id::SMITHY, id::CUTPURSE], id::PANIC, 2);
    g.supply.set(id::DOUBLOONS, 1);
    g.players[0].in_play = counts_of(&[id::DOUBLOONS]);
    let before = g.supply.get(id::DOUBLOONS);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    end_turn(&mut g);
    assert_eq!(g.supply.get(id::DOUBLOONS), before + 1);
    assert_eq!(g.players[0].discard.get(id::DOUBLOONS), 0);
}

#[test]
fn panic_activated_while_a_treasure_is_in_play_returns_it_but_gave_no_buys() {
    // "If you play a Treasure and then activate Panic while the Treasure is in play, you don't get the +2 Buys but do return the Treasure to its pile."
    let mut g = new_state(&[id::TEA_HOUSE, id::SMITHY, id::PANIC], 2);
    g.rs.suns = 1;
    set_hand(&mut g, 0, &[id::TEA_HOUSE]);
    set_in_play(&mut g, 0, &[id::COPPER]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    let copper_before = g.supply.get(id::COPPER);
    play(&mut g, id::TEA_HOUSE);
    assert_eq!(g.turn.buys, 1, "no Buys for a Treasure played before Panic");
    end_turn(&mut g);
    assert_eq!(g.supply.get(id::COPPER), copper_before + 1);
}

#[test]
fn harsh_winter_ignores_gains_on_other_players_turns() {
    // "When it's not your turn, gaining a card neither puts Debt on the pile nor removes Debt from the pile."
    let mut g = active(&[id::SMITHY], id::HARSH_WINTER, 2);
    g.emp.piles.add_debt(id::SILVER, 2);
    g.gain_card(1, id::SILVER, Dest::Discard, &mut NoEvents);
    assert_eq!((g.emp.piles.debt_on(id::SILVER), g.players[1].debt), (2, 0));
}

#[test]
fn rapid_expansion_sets_aside_a_treasure_gained_to_the_hand() {
    // "if you use Mine while Rapid Expansion is in effect, the gained Treasure will be set aside and can't be played on the current turn."
    let mut g = active(&[id::SMITHY, id::TEA_HOUSE], id::RAPID_EXPANSION, 2);
    g.turn.turn_start_resolved = true;
    g.gain_card(0, id::GOLD, Dest::Hand, &mut NoEvents);
    adv(&mut g);
    assert_eq!((g.players[0].aside.get(id::GOLD), g.players[0].hand.get(id::GOLD)), (1, 0));
}

#[test]
fn practice_can_play_a_shadow_card_from_the_deck_twice() {
    // Shadow: "When a card like Throne Room tells you to play a card from your hand, you can use that opportunity to play a Shadow card from your deck."
    let mut g = event_game(&[id::PRACTICE, id::ALLEY]);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::ALLEY, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    buy_phase(&mut g, 3, 1);
    adv(&mut g);
    pick(&mut g, id::PRACTICE);
    assert!(has_choice(&g, Choice::Card(id::ALLEY)), "{:?}", choices(&g));
}

#[test]
fn bureaucracy_gains_a_copper_for_a_loot_but_not_for_a_copper() {
    // "This includes cards gained from outside the Supply (such as Loot)." A Copper costs $0: no Copper in return.
    let mut g = active(&[id::SMITHY, id::CUTPURSE], id::BUREAUCRACY, 2);
    g.supply.set(id::DOUBLOONS, 1);
    let copper = g.supply.get(id::COPPER);
    g.gain_card(0, id::COPPER, Dest::Discard, &mut NoEvents);
    assert_eq!(g.supply.get(id::COPPER), copper - 1);
    g.gain_card(0, id::DOUBLOONS, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(id::COPPER), 2);
}

#[test]
fn amass_is_stopped_by_a_duration_played_on_an_earlier_turn() {
    // "Duration cards in play that were played on previous turns will stop Amass from gaining an Action card": they count as Actions in play.
    let mut g = event_game(&[id::AMASS]);
    set_in_play(&mut g, 0, &[id::SAMURAI]);
    assert_eq!(g.actions_in_play(0), 1);
    // (Amass is not even offered then, see the ignored test below.)
    buy_phase(&mut g, 2, 2);
    adv(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::AMASS)));
}

#[test]
fn gather_still_gains_the_other_cards_when_nothing_costs_exactly_four() {
    // "If you can't gain one - for example if nothing in the Supply costs exactly $4 - you still gain the others."
    let mut g = new_state(&[id::VILLAGE, id::MARKET, id::GATHER], 2);
    g.turn.turn_start_resolved = true;
    buy_phase(&mut g, 7, 2);
    adv(&mut g);
    pick(&mut g, id::GATHER);
    pick(&mut g, id::VILLAGE);
    // No card costs exactly $4: on to the $5 card.
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::MARKET)) && !c.contains(&Choice::Card(id::VILLAGE)), "{c:?}");
    pick(&mut g, id::MARKET);
    assert_eq!(g.players[0].discard.total(), 2);
}

#[test]
fn receive_tribute_gains_actions_with_potion_or_debt_in_the_cost_without_paying() {
    // "Actions with Debt or P in their cost can be gained. You do not take any Debt for this or need to have any Potion in play."
    let mut g = new_state(&[id::ALCHEMIST, id::DAIMYO, id::SMITHY, id::RECEIVE_TRIBUTE], 2);
    g.turn.turn_start_resolved = true;
    for _ in 0..3 {
        g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    }
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    pick(&mut g, id::RECEIVE_TRIBUTE);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::ALCHEMIST)) && c.contains(&Choice::Card(id::DAIMYO)), "{c:?}");
    pick(&mut g, id::DAIMYO);
    assert_eq!(g.players[0].debt, 0);
}

#[test]
fn credit_cannot_gain_a_card_with_debt_or_potion_in_its_cost() {
    // "This can't gain cards with Debt in the cost." "It also can't gain cards with P in the cost."
    let mut g = new_state(&[id::ALCHEMIST, id::DAIMYO, id::SMITHY, id::CREDIT], 2);
    g.turn.turn_start_resolved = true;
    buy_phase(&mut g, 2, 1);
    adv(&mut g);
    pick(&mut g, id::CREDIT);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && !c.contains(&Choice::Card(id::ALCHEMIST)) && !c.contains(&Choice::Card(id::DAIMYO)), "{c:?}");
}

#[test]
fn continue_still_returns_to_the_action_phase_when_the_gained_card_was_set_aside_by_rapid_expansion() {
    // "If the gained card gets moved elsewhere (Rapid Expansion), Continue can't play it, but you still return to your Action phase and get +1 Action and +1 Buy."
    let mut g = active(&[id::SMITHY, id::CONTINUE, id::VILLAGE], id::RAPID_EXPANSION, 2);
    g.turn.turn_start_resolved = true;
    set_hand(&mut g, 0, &[id::ESTATE]);
    buy_phase(&mut g, 3, 1);
    adv(&mut g);
    pick(&mut g, id::CONTINUE);
    pick(&mut g, id::SMITHY);
    adv(&mut g);
    assert_eq!(g.players[0].aside.get(id::SMITHY), 1, "set aside, not played");
    assert!(!g.players[0].in_play.has(id::SMITHY));
    assert_eq!((g.turn.actions, g.turn.buys), (2, 1));
}

#[test]
fn good_harvest_is_not_retroactive() {
    // "If you played a Treasure in the same turn before removing the last Sun token, it doesn't retroactively give you +1 Buy and $1."
    let mut g = new_state(&[id::TEA_HOUSE, id::SMITHY, id::GOOD_HARVEST], 2);
    g.rs.suns = 1;
    set_hand(&mut g, 0, &[id::TEA_HOUSE]);
    set_in_play(&mut g, 0, &[id::COPPER]);
    g.turn.played.add(id::COPPER, 1);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::TEA_HOUSE);
    assert_eq!(g.rs.active, id::GOOD_HARVEST);
    assert_eq!((g.turn.buys, g.turn.coins), (1, 2), "only the Tea House's $2");
}

#[test]
fn bureaucracy_gains_a_copper_for_a_card_with_debt_in_its_cost() {
    // "Cards with Debt costs don't cost exactly $0, and so come with a Copper."
    let mut g = active(&[id::DAIMYO, id::SMITHY], id::BUREAUCRACY, 2);
    g.gain_card(0, id::DAIMYO, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(id::COPPER), 1);
}

#[test]
fn progress_overrules_a_gain_to_the_hand_but_not_a_card_that_moves_itself_after_being_gained() {
    // "Ghost Town is gained to the hand (and must be moved to the deck by Progress), whereas Villa moves itself to the hand after being gained
    // (which can overrule Progress)."
    let mut g = active(&[id::GHOST_TOWN, id::VILLA, id::SMITHY], id::PROGRESS, 2);
    g.gain_card(0, id::GHOST_TOWN, Dest::Hand, &mut NoEvents);
    adv(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GHOST_TOWN), "{:?}", g.players[0].hand);
    let mut g = active(&[id::GHOST_TOWN, id::VILLA, id::SMITHY], id::PROGRESS, 2);
    g.gain_card(0, id::VILLA, Dest::Discard, &mut NoEvents);
    while g.pending_decision().is_some() {
        let c = choices(&g);
        choose(&mut g, c[0]);
    }
    assert!(g.players[0].hand.has(id::VILLA) || g.players[0].deck_known.peek_top() == Some(id::VILLA), "Villa went to the hand or the deck");
}

#[test]
fn enlightenment_makes_a_crown_played_in_the_action_phase_a_cantrip() {
    // "Enlightenment even applies to Treasure cards that are already Actions; if you play Crown in the Action phase, you still get +1 Card and +1 Action."
    let mut g = active(&[id::CROWN, id::SMITHY], id::ENLIGHTENMENT, 2);
    set_hand(&mut g, 0, &[id::CROWN]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::CROWN);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    assert_eq!(g.turn.actions, 1, "the Action used, +1 Action");
}

#[test]
fn kitsune_curse_is_stopped_by_a_moat() {
    let mut g = new_state(&[id::KITSUNE, id::MOAT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::KITSUNE]);
    set_hand(&mut g, 1, &[id::MOAT, id::COPPER]);
    play(&mut g, id::KITSUNE);
    moat(&mut g, true);
    // Options: +2 Actions, +$2, curse, Silver: pick the curse and the Silver.
    choose(&mut g, Choice::Mode(2));
    choose(&mut g, Choice::Mode(3));
    assert_eq!(g.players[1].discard.get(id::CURSE) + g.players[1].hand.get(id::CURSE), 0);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
}

#[test]
fn an_alley_played_from_the_deck_discards_a_card_of_the_hand_not_a_shadow_of_the_deck() {
    // Shadow: "you cannot discard it to an ability like Alley's (unless it is actually in your hand)."
    let mut g = new_state(&[id::ALLEY, id::NINJA, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::COPPER]);
    set_deck_unknown(&mut g, 0, &[id::ALLEY, id::NINJA, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::ALLEY);
    let c = choices(&g);
    assert!(!c.contains(&Choice::Card(id::NINJA)) && !c.contains(&Choice::Card(id::ALLEY)), "{c:?}");
}

#[test]
#[ignore = "audit: with Capitalism a Fishmonger in the deck should be playable like a Treasure in the Buy phase (Fishmonger FAQ); PlayTreasure only offers the hand"]
fn fishmonger_can_be_played_from_the_deck_in_the_buy_phase_with_capitalism() {
    let mut g = new_state(&[id::FISHMONGER, id::CAPITALISM, id::SMITHY], 2);
    let bit = 1u32 << (id::CAPITALISM - cards::FIRST_PROJECT);
    g.players[0].ren.projects |= bit;
    g.ren.projects |= bit;
    g.ren_update_hooks();
    set_hand(&mut g, 0, &[id::COPPER]);
    set_deck_unknown(&mut g, 0, &[id::FISHMONGER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    g.turn.actions = 0;
    expect_decision(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    assert!(has_choice(&g, Choice::Card(id::FISHMONGER)), "{:?}", choices(&g));
}

#[test]
fn approaching_army_gives_its_coin_even_when_the_attack_was_played_as_a_way() {
    // "You get the $1 even if you didn't follow the instructions on the Attack card; for example if you used a Way."
    let mut g = active(&[id::MILITIA, id::WAY_OF_THE_OWL], id::APPROACHING_ARMY, 2);
    set_hand(&mut g, 0, &[id::MILITIA]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::MILITIA);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Way), "{d:?}");
    choose(&mut g, Choice::Card(id::WAY_OF_THE_OWL));
    assert_eq!(g.turn.coins, 1, "no Militia $2, but Approaching Army's $1");
}

#[test]
fn a_second_samurai_play_by_throne_room_does_not_make_the_others_discard_twice() {
    // Samurai: "It doesn't make players discard again." (The 2nd play of a Throne Room finds hands already at 3.)
    let mut g = new_state(&[id::SAMURAI, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SAMURAI]);
    set_hand(&mut g, 1, &[id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY]);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::SAMURAI);
    let mut asked = 0;
    while g.pending_decision().is_some_and(|d| d.player == 1) {
        asked += 1;
        let c = choices(&g)[0];
        choose(&mut g, c);
    }
    assert_eq!(asked, 2, "two cards discarded by the first play, nothing left to discard for the second");
    assert_eq!(g.players[1].hand.total(), 3);
}

#[test]
fn great_leader_gives_its_action_after_a_card_played_as_a_way() {
    let mut g = active(&[id::SMITHY, id::WAY_OF_THE_OWL], id::GREAT_LEADER, 2);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::SMITHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Way), "{d:?}");
    choose(&mut g, Choice::Card(id::WAY_OF_THE_OWL));
    assert_eq!(g.turn.actions, 1, "the Action used and Great Leader's +1");
}

#[test]
fn a_shadow_card_goes_to_the_bottom_when_annex_shuffles_the_discard_pile_into_the_deck() {
    // Shadow: "When shuffling Shadow cards, put them on the bottom." (Every shuffle, not only a draw's reshuffle.)
    let mut g = new_state(&[id::ANNEX, id::ALLEY, id::SMITHY], 2);
    set_discard(&mut g, 0, &[id::ALLEY, id::COPPER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    buy_phase(&mut g, 0, 2);
    adv(&mut g);
    pick(&mut g, id::ANNEX);
    // Keep nothing.
    pass(&mut g);
    let ps = &g.players[0];
    assert_eq!(ps.deck_known_bottom.len, 1, "the Alley is at the bottom of the shuffled deck");
    assert!(!ps.deck_unknown.has(id::ALLEY));
}

#[test]
fn a_shadow_card_goes_to_the_bottom_when_inn_shuffles_actions_into_the_deck() {
    let mut g = new_state(&[id::INN, id::ALLEY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    set_discard(&mut g, 0, &[id::ALLEY, id::VILLAGE, id::ESTATE]);
    set_deck_unknown(&mut g, 0, &[id::CURSE; 12]);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    g.turn.buys = 2;
    buy(&mut g, id::INN);
    pick(&mut g, id::VILLAGE);
    pick(&mut g, id::ALLEY);
    pass(&mut g);
    let ps = &g.players[0];
    assert_eq!(ps.deck_known_bottom.len, 1, "Alley at the bottom");
    assert!(!ps.deck_unknown.has(id::ALLEY));
}
