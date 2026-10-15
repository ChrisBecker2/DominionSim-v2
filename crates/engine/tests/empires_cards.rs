//! Empires tests (rulings in `docs/empires-plan.md`): the cards, the split piles and the Castles, Debt, the Events and the Landmarks.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

// ===========================================================================
// Random games: nothing is lost, nothing panics, every game ends
// ===========================================================================

#[test]
fn random_empires_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0xE3_91_4E5);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| matches!(cards::set_of(c), CardSet::Empires | CardSet::RisingSun)).collect();
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| !matches!(cards::set_of(c), CardSet::Empires | CardSet::RisingSun)).collect();
    let events: Vec<CardId> = (id::ADVANCE..=id::WINDFALL).collect();
    let landmarks: Vec<CardId> = (id::AQUEDUCT..=id::WOLF_DEN).collect();
    let mut debt_seen = false;
    let mut castles_seen = false;
    let mut tokens_seen = false;
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
        let mut evs = events.clone();
        for _ in 0..(game % 4) {
            let i = rng.below(evs.len() as u32) as usize;
            kingdom.push(evs.swap_remove(i));
        }
        let mut lms = landmarks.clone();
        for _ in 0..(game % 3) {
            let i = rng.below(lms.len() as u32) as usize;
            kingdom.push(lms.swap_remove(i));
        }
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom, seed: game, max_turns: 300, ..Default::default() });
        let initial = total_cards(&g);
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
                    g.apply(c, &mut log).unwrap();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            debt_seen |= g.players.iter().any(|p| p.debt > 0);
            castles_seen |= g.castles_left() > 0 && g.supply.get(id::HUMBLE_CASTLE) == 0;
            tokens_seen |= g.players.iter().any(|p| p.vp_tokens > 0);
            if total_cards(&g) != initial {
                panic!("card conservation, game {game}: {log:?}");
            }
            steps += 1;
            assert!(steps < 200_000, "game {game} does not end");
        }
        // The text format round-trips whatever the game left.
        let text = format_state(&g);
        let back = parse_state(&text).unwrap_or_else(|e| panic!("game {game}: {e}\n{text}"));
        // (The seed line is a number derived from the live RNG: it cannot round-trip.)
        let no_seed = |s: &str| s.lines().filter(|l| !l.starts_with("seed:")).collect::<Vec<_>>().join("
");
        assert_eq!(no_seed(&format_state(&back)), no_seed(&text), "game {game}");
    }
    assert!(debt_seen && castles_seen && tokens_seen, "the random games should reach Debt ({debt_seen}), Castles ({castles_seen}) and VP tokens ({tokens_seen})");
}

// ===========================================================================
// Helpers
// ===========================================================================

fn junk() -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(40).copied().collect()
}

fn count(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].all_cards().get(c)
}

fn pick(g: &mut GameState, c: CardId) -> Step {
    choose(g, Choice::Card(c))
}

fn pending_kind(g: &GameState) -> Option<DecisionKind> {
    g.pending_decision().map(|d| d.kind)
}

/// A Buy phase for player 1 with `coins`, the Treasures all played.
fn buy_phase(g: &mut GameState, coins: u16, buys: u8) {
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
    g.turn.buys = buys;
    g.turn.treasures_done = true;
    g.pending = Pending::None;
}

fn has_choice(g: &GameState, c: Choice) -> bool {
    choices(g).contains(&c)
}

fn strip_seed(s: &str) -> String {
    s.lines().filter(|l| !l.starts_with("seed:")).collect::<Vec<_>>().join("\n")
}

// ===========================================================================
// Set-up: the split piles, the Castles, Events and Landmarks
// ===========================================================================

#[test]
fn the_ids_line_up() {
    assert!(cards::is_event(id::ADVANCE) && cards::is_event(id::WINDFALL) && cards::is_event(id::ALMS) && !cards::is_event(id::AQUEDUCT));
    assert!(cards::is_landmark(id::AQUEDUCT) && cards::is_landmark(id::WOLF_DEN) && !cards::is_landmark(id::WINDFALL));
    assert_eq!(id::TREASURE_CHEST as usize + 1, cards::NUM_CARDS);
    assert_eq!(cards::NUM_EVENTS, 44, "20 Adventures, 13 Empires, Summon and 10 Rising Sun");
    assert_eq!(cards::kingdom_cards_in(CardSet::Empires).count(), 24);
    assert_eq!(cards::kingdom_cards_in(CardSet::RisingSun).count(), 25);
    // Events and Landmarks are above every id that can sit in a zone.
    assert_eq!(cards::NUM_ZONE_IDS, cards::FIRST_EVENT as usize);
    assert!(cards::NUM_ZONE_IDS <= dominion_engine::counts::LANES);
    // The costs.
    assert_eq!((cards::cost(id::ENGINEER), cards::debt_cost(id::ENGINEER)), (0, 4));
    assert_eq!((cards::cost(id::FORTUNE), cards::debt_cost(id::FORTUNE)), (8, 8));
    assert_eq!((cards::cost(id::WEDDING), cards::debt_cost(id::WEDDING)), (4, 3));
    assert_eq!((cards::cost(id::TRIUMPH), cards::debt_cost(id::TRIUMPH)), (0, 5));
    assert_eq!(cards::cost_string(id::CITY_QUARTER), "8 Debt");
    assert_eq!(cards::cost_string(id::FORTUNE), "$8 and 8 Debt");
    assert_eq!(cards::by_name("Settlers/Bustling Village"), Some(id::SETTLERS));
    assert_eq!(cards::by_name("bustling village"), Some(id::BUSTLING_VILLAGE));
    assert_eq!(cards::pile_of(id::ROCKS), id::CATAPULT);
    assert_eq!(cards::pile_of(id::KINGS_CASTLE), id::CASTLES);
}

#[test]
fn split_piles_have_five_of_each_half_and_the_bottom_is_not_for_sale_until_the_top_is_gone() {
    for (top, bottom) in cards::SPLIT_PILES {
        // Either half names the pile.
        for named in [top, bottom] {
            let g = new_state(&[named, id::SMITHY], 2);
            assert_eq!((g.supply.get(top), g.supply.get(bottom)), (5, 5), "{}", cards::name(top));
            assert!(g.in_supply(top) && g.in_supply(bottom));
        }
    }
    let mut g = new_state(&[id::GLADIATOR, id::SMITHY], 2);
    buy_phase(&mut g, 8, 1);
    adv(&mut g);
    assert!(has_choice(&g, Choice::Card(id::GLADIATOR)) && !has_choice(&g, Choice::Card(id::FORTUNE)));
    g.supply.set(id::GLADIATOR, 0);
    g.pending = Pending::None;
    adv(&mut g);
    assert!(has_choice(&g, Choice::Card(id::FORTUNE)));
    // A gain too: Catapult's pile, the Rocks ($4) under the Catapults.
    let mut g = new_state(&[id::CATAPULT, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    assert!(has_choice(&g, Choice::Card(id::CATAPULT)) && !has_choice(&g, Choice::Card(id::ROCKS)));
}

#[test]
fn a_split_pile_is_one_pile_for_the_end_of_the_game() {
    let mut g = new_state(&[id::PATRICIAN, id::SMITHY], 2);
    g.supply.set(id::PATRICIAN, 0);
    assert_eq!(g.empty_piles(), 0, "the Emporiums are still there");
    assert_eq!(g.empty_piles_scan(), 0);
    g.supply.set(id::EMPORIUM, 0);
    assert_eq!((g.empty_piles(), g.empty_piles_scan()), (1, 1));
    g.supply.set(id::SMITHY, 0);
    assert_eq!((g.empty_piles(), g.empty_piles_scan()), (2, 2));
}

#[test]
fn the_castles_are_eight_cards_in_cost_order_with_the_top_one_for_sale() {
    let g = new_state(&[id::CASTLES, id::SMITHY], 2);
    for i in 0..8 {
        assert_eq!(g.supply.get(id::HUMBLE_CASTLE + i), 1, "two players: one of each");
    }
    assert_eq!(g.castles_left(), 8);
    assert_eq!(g.castle_top(), Some(id::HUMBLE_CASTLE));
    let g3 = new_state(&[id::CASTLES, id::SMITHY], 3);
    assert_eq!(g3.castles_left(), 12);
    for (i, want) in [2, 1, 2, 1, 2, 1, 1, 2].into_iter().enumerate() {
        assert_eq!(g3.supply.get(id::HUMBLE_CASTLE + i as CardId), want, "{}", cards::name(id::HUMBLE_CASTLE + i as CardId));
    }
    let mut g = g;
    buy_phase(&mut g, 10, 1);
    adv(&mut g);
    let offered: Vec<CardId> =
        choices(&g).iter().filter_map(|c| if let Choice::Card(x) = c { Some(*x) } else { None }).filter(|&c| cards::is_castle(c)).collect();
    assert_eq!(offered, vec![id::HUMBLE_CASTLE], "only the top Castle");
    // The pile is one pile for the end of the game, empty when every Castle is gone.
    for i in 0..7 {
        g.supply.set(id::HUMBLE_CASTLE + i, 0);
    }
    assert_eq!(g.empty_piles(), 0);
    g.supply.set(id::KINGS_CASTLE, 0);
    assert_eq!((g.empty_piles(), g.empty_piles_scan()), (1, 1));
    assert_eq!(g.castle_top(), None);
}

#[test]
fn the_kingdom_names_the_castles_and_text_round_trips_the_empires_state() {
    let k = dominion_engine::text::parse_kingdom("Castles, Plunder, Temple").unwrap();
    assert_eq!(k, vec![id::CASTLES, id::ENCAMPMENT, id::TEMPLE]);
    let mut g = new_state(&[id::CASTLES, id::ENCAMPMENT, id::TEMPLE, id::TAX, id::ARENA, id::AQUEDUCT, id::DEFILED_SHRINE, id::SMITHY], 3);
    g.players[1].debt = 7;
    g.players[2].vp_tokens = 3;
    g.emp.piles.add_vp(id::TEMPLE, 2);
    g.emp.lm_vp[0] += 1;
    let text = format_state(&g);
    let back = parse_state(&text).unwrap();
    assert_eq!(strip_seed(&format_state(&back)), strip_seed(&text));
    assert_eq!(back.players[1].debt, 7);
    assert_eq!(back.emp.piles.vp_on(id::TEMPLE), 2);
    assert_eq!(back.emp.landmarks, g.emp.landmarks);
    assert_eq!(back.castles_left(), 12);
}

// ===========================================================================
// Debt
// ===========================================================================

#[test]
fn buying_a_card_with_debt_in_its_cost_takes_the_tokens_and_blocks_buying_until_paid() {
    let mut g = new_state(&[id::CITY_QUARTER, id::SMITHY], 2);
    buy_phase(&mut g, 4, 2);
    // City Quarter costs only Debt: affordable with $0, and 8 Debt are taken.
    adv(&mut g);
    buy(&mut g, id::CITY_QUARTER);
    assert_eq!(g.players[0].debt, 8);
    assert_eq!(count(&g, 0, id::CITY_QUARTER), 1);
    // A Buy is left, but Debt blocks every purchase: only paying (or passing) is offered.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    let c = choices(&g);
    assert!(c.contains(&Choice::PayDebt) && c.contains(&Choice::Pass) && !c.iter().any(|x| matches!(x, Choice::Card(_))), "{c:?}");
    choose(&mut g, Choice::PayDebt);
    assert_eq!((g.players[0].debt, g.turn.coins), (4, 0));
}

#[test]
fn debt_is_paid_with_coins_at_any_buy_decision_and_leftover_coins_pay_it_at_the_end_of_the_turn() {
    let mut g = new_state(&[id::ENGINEER, id::SMITHY], 2);
    g.players[0].debt = 3;
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    choose(&mut g, Choice::PayDebt);
    assert_eq!((g.players[0].debt, g.turn.coins), (0, 2));
    // With no Debt the cards are on offer again.
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SILVER)) || c.contains(&Choice::Card(id::SMITHY)) || c.contains(&Choice::Card(id::ESTATE)));
    // Passing with Debt left: the $ left over pay it in Clean-up.
    let mut g = new_state(&[id::ENGINEER, id::SMITHY], 2);
    g.players[0].debt = 3;
    buy_phase(&mut g, 2, 1);
    adv(&mut g);
    pass(&mut g);
    assert_eq!(g.players[0].debt, 1, "the 2 coins left over were paid against the Debt at the end of the turn");
}

#[test]
fn debt_does_not_stop_playing_cards_but_blocks_events_too() {
    let mut g = new_state(&[id::ENGINEER, id::DELVE, id::SMITHY], 2);
    g.players[0].debt = 2;
    g.turn.coins = 5;
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 3, "playing cards is allowed");
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert!(!choices(&g).iter().any(|c| matches!(c, Choice::Card(_))), "no purchase, no Event: {:?}", choices(&g));
}

#[test]
fn coffers_pay_debt_one_token_at_a_time_through_the_coins() {
    let mut g = new_state(&[id::CITY_QUARTER, id::SMITHY, id::BAKER], 2);
    g.players[0].debt = 2;
    g.players[0].coffers = 3;
    buy_phase(&mut g, 0, 1);
    adv(&mut g);
    // No $: only Coffers (and passing) can be offered.
    let c = choices(&g);
    assert!(c.contains(&Choice::Coffers) && !c.contains(&Choice::PayDebt));
    choose(&mut g, Choice::Coffers);
    choose(&mut g, Choice::PayDebt);
    assert_eq!((g.players[0].debt, g.players[0].coffers, g.turn.coins), (1, 2, 0));
}

#[test]
fn capital_gives_six_debt_when_discarded_from_play_once_even_when_played_twice_and_the_coins_left_pay_it() {
    let mut g = new_state(&[id::CAPITAL, id::CROWN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CAPITAL, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    adv(&mut g);
    assert_eq!(g.turn.coins, 7, "Capital $6 and a Copper");
    assert_eq!(g.turn.buys, 2);
    pass(&mut g);
    // Clean-up: 6 Debt for the Capital, then the $7 left over pay it off.
    assert_eq!(g.players[0].debt, 0);
    assert_eq!(g.turn.player, 1);
    // A Crown'd Capital still gives 6 once; with nothing left to pay it with, the Debt stays.
    let mut g = new_state(&[id::CAPITAL, id::CROWN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CAPITAL, id::CROWN]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    expect_decision(&mut g);
    play_treasure(&mut g, id::CROWN);
    pick(&mut g, id::CAPITAL);
    assert_eq!(g.turn.coins, 12, "Capital twice");
    // Spend it all on a Province ($8) and a Smithy ($4): nothing is left for the Debt.
    buy(&mut g, id::PROVINCE);
    buy(&mut g, id::SMITHY);
    pass(&mut g);
    assert_eq!(g.turn.player, 1);
    assert_eq!(g.players[0].debt, 6, "6 Debt once");
}

#[test]
fn a_game_with_change_does_not_pay_the_debt_for_the_player_at_the_end_of_the_turn() {
    let mut g = new_state(&[id::CHANGE, id::SMITHY], 2);
    g.players[0].debt = 3;
    buy_phase(&mut g, 2, 1);
    adv(&mut g);
    // Both ways of paying and passing are offered; passing leaves the Debt.
    let c = choices(&g);
    assert!(c.contains(&Choice::PayDebt) && c.contains(&Choice::PayDebtKeep(1)) && c.contains(&Choice::Pass));
    pass(&mut g);
    assert_eq!(g.players[0].debt, 3, "Change makes Debt worth keeping: nothing is paid unless the player pays");
    // Paying all but one.
    let mut g = new_state(&[id::CHANGE, id::SMITHY], 2);
    g.players[0].debt = 3;
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    choose(&mut g, Choice::PayDebtKeep(1));
    assert_eq!((g.players[0].debt, g.turn.coins), (1, 3));
}

#[test]
fn change_gives_three_coins_with_debt_and_otherwise_remodels_and_takes_the_difference_as_debt() {
    let mut g = new_state(&[id::CHANGE, id::SMITHY], 2);
    g.players[0].debt = 1;
    set_hand(&mut g, 0, &[id::CHANGE, id::COPPER]);
    play(&mut g, id::CHANGE);
    assert_eq!(g.turn.coins, 4, "Change's $3, and the Copper once the Buy phase starts");
    // No Debt: trash a Copper ($0), gain a Gold ($6): 6 Debt.
    let mut g = new_state(&[id::CHANGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CHANGE, id::COPPER, id::ESTATE]);
    play(&mut g, id::CHANGE);
    pick(&mut g, id::COPPER);
    assert!(has_choice(&g, Choice::Card(id::GOLD)) && !has_choice(&g, Choice::Card(id::COPPER)), "only cards costing more $");
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].debt, 6);
    assert_eq!(count(&g, 0, id::GOLD), 1);
    assert_eq!(g.trash.get(id::COPPER), 1);
}

#[test]
fn paying_debt_in_the_action_phase_lets_change_remodel() {
    let mut g = new_state(&[id::CHANGE, id::SMITHY, id::MERCHANT], 2);
    g.players[0].debt = 2;
    g.turn.coins = 3;
    set_hand(&mut g, 0, &[id::CHANGE, id::ESTATE, id::COPPER]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    assert!(has_choice(&g, Choice::PayDebt));
    choose(&mut g, Choice::PayDebt);
    assert_eq!((g.players[0].debt, g.turn.coins), (0, 1));
    play(&mut g, id::CHANGE);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].debt, 2, "Estate $2 -> Smithy $4: 2 Debt");
}

#[test]
fn engineer_gains_cards_up_to_four_but_never_one_with_debt_and_may_trash_itself_for_another() {
    let mut g = new_state(&[id::ENGINEER, id::CITY_QUARTER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ENGINEER]);
    play(&mut g, id::ENGINEER);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && c.contains(&Choice::Card(id::SILVER)));
    assert!(!c.contains(&Choice::Card(id::ENGINEER)) && !c.contains(&Choice::Card(id::CITY_QUARTER)), "a card with Debt in its cost is not 'up to $4'");
    pick(&mut g, id::SMITHY);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::YesNo { act: Act::Trash })));
    choose(&mut g, Choice::Yes);
    pick(&mut g, id::SILVER);
    assert_eq!(g.trash.get(id::ENGINEER), 1);
    assert_eq!((count(&g, 0, id::SMITHY), count(&g, 0, id::SILVER)), (1, 1));
}

#[test]
fn fortune_doubles_the_coins_once_and_is_played_after_the_other_treasures() {
    let mut g = new_state(&[id::GLADIATOR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::FORTUNE, id::FORTUNE, id::GOLD, id::SILVER, id::COPPER]);
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    // The plain Treasures first: $6, then a Fortune doubles it, the second only gives its Buy.
    choose(&mut g, Choice::Card(id::GOLD));
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::COPPER));
    assert_eq!(g.turn.coins, 6);
    play_treasure(&mut g, id::FORTUNE);
    assert_eq!(g.turn.coins, 12);
    let buys = g.turn.buys;
    play_treasure(&mut g, id::FORTUNE);
    assert_eq!((g.turn.coins, g.turn.buys), (12, buys + 1));
}

#[test]
fn fortune_gains_a_gold_per_gladiator_in_play_when_it_is_gained() {
    let mut g = new_state(&[id::GLADIATOR, id::SMITHY], 2);
    g.supply.set(id::GLADIATOR, 0);
    set_in_play(&mut g, 0, &[id::GLADIATOR, id::GLADIATOR, id::GLADIATOR]);
    buy_phase(&mut g, 8, 1);
    adv(&mut g);
    buy(&mut g, id::FORTUNE);
    assert_eq!(count(&g, 0, id::GOLD), 3, "one Gold per Gladiator in play");
    assert_eq!(g.players[0].debt, 8);
}

// ===========================================================================
// The cards
// ===========================================================================

/// Pass (or take the first choice) on every decision until `done` holds, with the next decision pending.
fn pass_until(g: &mut GameState, mut done: impl FnMut(&GameState) -> bool) {
    for _ in 0..2000 {
        if let Step::GameOver = adv(g) {
            panic!("the game ended");
        }
        if done(g) {
            return;
        }
        let c = choices(g);
        let pick = if c.contains(&Choice::Pass) { Choice::Pass } else { c[0] };
        g.apply(pick, &mut NoEvents).unwrap();
    }
    panic!("never got there");
}

#[test]
fn city_quarter_gives_two_actions_and_a_card_per_action_in_the_hand() {
    let mut g = new_state(&[id::CITY_QUARTER, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CITY_QUARTER, id::VILLAGE, id::SMITHY, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CITY_QUARTER);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(g.players[0].hand.total(), 4 + 2, "Village and Smithy are Actions: +2 Cards");
}

#[test]
fn royal_blacksmith_draws_five_then_discards_every_copper_in_the_hand() {
    let mut g = new_state(&[id::ROYAL_BLACKSMITH, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ROYAL_BLACKSMITH, id::COPPER, id::COPPER, id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::COPPER, id::GOLD, id::COPPER, id::ESTATE]);
    g.turn.actions = 2;
    play(&mut g, id::ROYAL_BLACKSMITH);
    assert_eq!(g.players[0].discard.get(id::COPPER), 5, "the 2 in the hand and the 3 drawn");
    assert_eq!(g.players[0].hand.total(), 3);
    assert!(g.players[0].hand.has(id::SILVER) && g.players[0].hand.has(id::GOLD));
}

#[test]
fn overlord_plays_an_action_costing_up_to_five_from_the_supply_and_leaves_it_there() {
    let mut g = new_state(&[id::OVERLORD, id::VILLAGE, id::SMITHY, id::ENGINEER, id::FORUM, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::OVERLORD, id::COPPER, id::VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::OVERLORD);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && c.contains(&Choice::Card(id::VILLAGE)) && c.contains(&Choice::Card(id::FORUM)));
    assert!(!c.contains(&Choice::Card(id::ENGINEER)), "Debt is not 'up to $5'");
    assert!(!c.contains(&Choice::Card(id::OVERLORD)), "not a Command card");
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 2 + 3, "Smithy's +3 Cards");
    assert_eq!(g.supply.get(id::SMITHY), 10, "it stays in the Supply");
    assert_eq!(g.players[0].in_play.get(id::SMITHY), 0);
}

#[test]
fn encampment_stays_with_a_gold_or_plunder_and_goes_back_to_its_pile_without() {
    let mut g = new_state(&[id::ENCAMPMENT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ENCAMPMENT, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::ENCAMPMENT);
    assert_eq!(g.turn.actions, 2);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Reveal, .. })));
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].in_play.get(id::ENCAMPMENT), 1);
    // Without one it is set aside and returns to its pile (it does not count as a card of the player).
    let mut g = new_state(&[id::ENCAMPMENT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ENCAMPMENT, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    let before = g.supply.get(id::ENCAMPMENT);
    play(&mut g, id::ENCAMPMENT);
    assert_eq!(g.supply.get(id::ENCAMPMENT), before + 1);
    assert_eq!(count(&g, 0, id::ENCAMPMENT), 0);
    assert_eq!(g.turn.actions, 2);
}

#[test]
fn patrician_takes_a_revealed_card_costing_five_or_more_into_the_hand() {
    let mut g = new_state(&[id::PATRICIAN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PATRICIAN]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::PROVINCE, id::SILVER]);
    play(&mut g, id::PATRICIAN);
    assert!(g.players[0].hand.has(id::ESTATE) && g.players[0].hand.has(id::PROVINCE), "Province costs $8");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
    // $4 stays on the deck; a card with Debt in its cost is not $5 or more (City Quarter), Fortune is.
    let mut g = new_state(&[id::PATRICIAN, id::CITY_QUARTER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PATRICIAN]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::CITY_QUARTER, id::SILVER]);
    play(&mut g, id::PATRICIAN);
    assert_eq!(g.players[0].hand.total(), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::CITY_QUARTER));
    let mut g = new_state(&[id::PATRICIAN, id::GLADIATOR], 2);
    set_hand(&mut g, 0, &[id::PATRICIAN]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::FORTUNE, id::SILVER]);
    play(&mut g, id::PATRICIAN);
    assert!(g.players[0].hand.has(id::FORTUNE), "Fortune costs $8 and 8 Debt: $5 or more");
}

#[test]
fn settlers_and_bustling_village_take_a_copper_or_a_settlers_from_the_discard_pile() {
    let mut g = new_state(&[id::SETTLERS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SETTLERS, id::VILLAGE]);
    set_discard(&mut g, 0, &[id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD]);
    g.turn.actions = 2;
    play(&mut g, id::SETTLERS);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::YesNo { act: Act::ToHand })));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].hand.get(id::COPPER), 1);
    assert_eq!(g.players[0].discard.get(id::COPPER), 0);
    // No Copper in the discard pile: nothing to ask.
    let mut g = new_state(&[id::SETTLERS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SETTLERS]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD]);
    play(&mut g, id::SETTLERS);
    assert!(!matches!(pending_kind(&g), Some(DecisionKind::YesNo { .. })));
    // Bustling Village: +1 Card +3 Actions, and a Settlers from the discard pile.
    let mut g = new_state(&[id::SETTLERS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::BUSTLING_VILLAGE]);
    set_discard(&mut g, 0, &[id::SETTLERS, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD]);
    play(&mut g, id::BUSTLING_VILLAGE);
    assert_eq!(g.turn.actions, 3);
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].hand.has(id::SETTLERS), "the Settlers is an Action: the Action phase goes on");
    assert!(g.players[0].hand.has(id::SILVER));
}

#[test]
fn catapult_trashes_a_card_and_curses_for_three_or_more_and_cuts_hands_for_a_treasure() {
    let mut g = new_state(&[id::CATAPULT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CATAPULT, id::SILVER, id::ESTATE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::CATAPULT);
    assert_eq!(g.turn.coins, 1);
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1, "Silver costs $3");
    assert_eq!(g.players[1].hand.total(), 3, "a Treasure: discard down to 3");
    // A Province ($8, not a Treasure): the Curse only. An Estate ($2): nothing.
    let mut g = new_state(&[id::CATAPULT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CATAPULT, id::PROVINCE, id::ESTATE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::CATAPULT);
    pick(&mut g, id::PROVINCE);
    assert_eq!((g.players[1].discard.get(id::CURSE), g.players[1].hand.total()), (1, 5));
    let mut g = new_state(&[id::CATAPULT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CATAPULT, id::PROVINCE, id::ESTATE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::CATAPULT);
    pick(&mut g, id::ESTATE);
    assert_eq!((g.players[1].discard.get(id::CURSE), g.players[1].hand.total()), (0, 5));
}

#[test]
fn rocks_give_a_silver_when_gained_or_trashed_onto_the_deck_in_the_buy_phase_else_into_the_hand() {
    let mut g = new_state(&[id::CATAPULT, id::SMITHY], 2);
    g.supply.set(id::CATAPULT, 0);
    buy_phase(&mut g, 4, 2);
    adv(&mut g);
    buy(&mut g, id::ROCKS);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER), "bought: the Silver goes onto the deck");
    assert_eq!(count(&g, 0, id::ROCKS), 1);
    // Trashed by a Catapult in the Action phase: the Silver goes to the hand.
    let mut g = new_state(&[id::CATAPULT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CATAPULT, id::ROCKS, id::COPPER]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::CATAPULT);
    pick(&mut g, id::ROCKS);
    assert_eq!(g.turn.coins, 1 + 2 + 1, "Catapult's $1, then the Copper and the Silver (from the trashed Rocks) played in the Buy phase");
    assert_eq!(g.trash.get(id::ROCKS), 1);
}

#[test]
fn chariot_race_draws_and_compares_the_cost_with_the_left_players_top_card() {
    for (mine, theirs, wins) in
        [(id::PROVINCE, id::SILVER, true), (id::ESTATE, id::GOLD, false), (id::ESTATE, id::ESTATE, false), (id::FORTUNE, id::OVERLORD, true), (id::OVERLORD, id::SILVER, false)]
    {
        let mut g = new_state(&[id::CHARIOT_RACE, id::GLADIATOR, id::OVERLORD], 2);
        set_hand(&mut g, 0, &[id::CHARIOT_RACE]);
        set_deck_known(&mut g, 0, &[mine, id::COPPER]);
        set_deck_known(&mut g, 1, &[theirs, id::COPPER]);
        play(&mut g, id::CHARIOT_RACE);
        assert_eq!(count(&g, 0, mine), 1);
        assert_eq!(g.players[1].deck_known.peek_top(), Some(theirs), "it goes back");
        assert_eq!((g.turn.coins, g.players[0].vp_tokens), if wins { (1, 1) } else { (0, 0) }, "{} against {}", cards::name(mine), cards::name(theirs));
    }
    // No card to reveal for the left player: no bonus.
    let mut g = new_state(&[id::CHARIOT_RACE, id::GLADIATOR], 2);
    set_hand(&mut g, 0, &[id::CHARIOT_RACE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE]);
    clear_deck(&mut g, 1);
    play(&mut g, id::CHARIOT_RACE);
    assert_eq!((g.turn.coins, g.players[0].vp_tokens), (0, 0));
}

#[test]
fn enchantress_makes_the_first_action_of_each_other_players_turn_a_cantrip_until_its_owners_next_turn() {
    let mut g = new_state(&[id::ENCHANTRESS, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ENCHANTRESS]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 1, &junk());
    set_hand(&mut g, 1, &[id::SMITHY, id::SMITHY, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::ENCHANTRESS);
    // Through the rest of the turn: player 2 starts with the mark on them.
    pass_until(&mut g, |g| g.turn.player == 1 && matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::PlayAction, .. })));
    assert_eq!(g.players[1].ench_by, 1);
    set_hand(&mut g, 1, &[id::SMITHY, id::SMITHY, id::VILLAGE, id::COPPER, id::COPPER]);
    g.turn.actions = 2;
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[1].hand.total(), 4 + 1, "the first Smithy: +1 Card +1 Action instead of +3 Cards");
    assert_eq!(g.turn.actions, 2);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[1].hand.total(), 4 + 1 - 1 + 3, "the second is a Smithy");
    // The mark ends when its owner's next turn starts.
    pass_until(&mut g, |g| g.turn.player == 0 && g.turn.number > 2);
    assert_eq!(g.players[1].ench_by, 0);
    // Enchantress itself: +2 Cards at the start of that turn.
    assert_eq!(g.turn.coins, 7 * 3, "5 Golds drawn at Clean-up and 2 more from the Enchantress, all played");
}

#[test]
fn farmers_market_gathers_vp_for_four_plays_then_gives_them_to_the_fifth_and_trashes_itself() {
    let mut g = new_state(&[id::FARMERS_MARKET, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::FARMERS_MARKET; 5]);
    g.turn.actions = 5;
    let mut coins = 0;
    for n in 1..=4u16 {
        play(&mut g, id::FARMERS_MARKET);
        coins += n;
        assert_eq!(g.turn.coins, coins, "play {n}: +$ per VP token on the pile");
        assert_eq!(g.emp.piles.vp_on(id::FARMERS_MARKET), n as u8);
    }
    play(&mut g, id::FARMERS_MARKET);
    assert_eq!((g.turn.coins, g.players[0].vp_tokens, g.emp.piles.vp_on(id::FARMERS_MARKET)), (coins, 4, 0));
    assert_eq!(g.trash.get(id::FARMERS_MARKET), 1);
    assert_eq!(g.turn.buys, 1 + 5);
}

#[test]
fn gladiator_gives_a_coin_and_trashes_a_gladiator_unless_the_left_player_reveals_a_copy() {
    let mut g = new_state(&[id::GLADIATOR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GLADIATOR, id::COPPER, id::SILVER, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    g.turn.actions = 2;
    play(&mut g, id::GLADIATOR);
    assert_eq!(g.turn.coins, 2);
    pick(&mut g, id::SILVER);
    assert_eq!(g.turn.coins, 3, "+$2 and +$1: the left player has no Silver");
    assert_eq!((g.supply.get(id::GLADIATOR), g.trash.get(id::GLADIATOR)), (4, 1));
    // They have a copy: they are asked, and a reveal ends it.
    let mut g = new_state(&[id::GLADIATOR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GLADIATOR, id::COPPER, id::SILVER, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::COPPER, id::SILVER, id::ESTATE]);
    g.turn.actions = 2;
    play(&mut g, id::GLADIATOR);
    pick(&mut g, id::SILVER);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (1, DecisionKind::YesNo { act: Act::Reveal }));
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.supply.get(id::GLADIATOR), 5);
    // Declined, it works.
    let mut g = new_state(&[id::GLADIATOR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GLADIATOR, id::COPPER, id::SILVER, id::VILLAGE]);
    set_hand(&mut g, 1, &[id::COPPER, id::SILVER, id::ESTATE]);
    g.turn.actions = 2;
    play(&mut g, id::GLADIATOR);
    pick(&mut g, id::SILVER);
    choose(&mut g, Choice::No);
    assert_eq!(g.turn.coins, 3);
}

#[test]
fn sacrifice_gives_a_bonus_for_each_type_of_the_trashed_card() {
    // Action: +2 Cards +2 Actions.
    let mut g = new_state(&[id::SACRIFICE, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SACRIFICE, id::VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SACRIFICE);
    pick(&mut g, id::VILLAGE);
    assert_eq!((g.players[0].hand.total(), g.turn.actions), (1 + 2, 2));
    // Treasure: +$2; Victory: +2 VP tokens; Humble Castle is both.
    let mut g = new_state(&[id::SACRIFICE, id::CASTLES], 2);
    set_hand(&mut g, 0, &[id::SACRIFICE, id::GOLD, id::ESTATE, id::HUMBLE_CASTLE]);
    play(&mut g, id::SACRIFICE);
    pick(&mut g, id::GOLD);
    assert_eq!(g.turn.coins, 2 + 1, "+$2 (the Estate and the Humble Castle's $1 are played in the Buy phase)");
    let mut g = new_state(&[id::SACRIFICE, id::CASTLES], 2);
    set_hand(&mut g, 0, &[id::SACRIFICE, id::GOLD, id::ESTATE, id::HUMBLE_CASTLE]);
    play(&mut g, id::SACRIFICE);
    pick(&mut g, id::HUMBLE_CASTLE);
    assert_eq!((g.players[0].vp_tokens, g.turn.coins), (2, 2 + 3), "both bonuses (+$2 and +2 VP), then the Gold");
    let mut g = new_state(&[id::SACRIFICE, id::CASTLES], 2);
    set_hand(&mut g, 0, &[id::SACRIFICE, id::CURSE, id::ESTATE]);
    play(&mut g, id::SACRIFICE);
    pick(&mut g, id::CURSE);
    assert_eq!((g.players[0].vp_tokens, g.turn.coins), (0, 0), "a Curse has no relevant type");
}

#[test]
fn temple_gathers_a_vp_token_on_its_pile_and_gaining_a_temple_takes_them() {
    let mut g = new_state(&[id::TEMPLE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::TEMPLE, id::COPPER, id::COPPER, id::ESTATE]);
    play(&mut g, id::TEMPLE);
    assert_eq!(g.players[0].vp_tokens, 1);
    // 1 to 3 differently named cards: a second Copper is not offered.
    pick(&mut g, id::COPPER);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::ESTATE)) && !c.contains(&Choice::Card(id::COPPER)), "{c:?}");
    pick(&mut g, id::ESTATE);
    assert_eq!(g.trash.total(), 2);
    assert_eq!(g.emp.piles.vp_on(id::TEMPLE), 1);
    // Buying a Temple takes the tokens.
    buy_phase(&mut g, 4, 1);
    g.players[0].hand = Counts::EMPTY;
    adv(&mut g);
    buy(&mut g, id::TEMPLE);
    assert_eq!((g.players[0].vp_tokens, g.emp.piles.vp_on(id::TEMPLE)), (2, 0));
}

#[test]
fn villa_goes_to_the_hand_with_an_action_and_returns_a_buyer_to_the_action_phase() {
    let mut g = new_state(&[id::VILLA, id::SMITHY], 2);
    g.turn.actions = 0;
    buy_phase(&mut g, 4, 1);
    adv(&mut g);
    buy(&mut g, id::VILLA);
    assert!(g.players[0].hand.has(id::VILLA) && g.players[0].discard.get(id::VILLA) == 0);
    assert_eq!((g.turn.phase, g.turn.actions), (Phase::Action, 1));
    // Play it: +2 Actions +1 Buy +$1, and the Buy phase starts over with a Buy.
    play(&mut g, id::VILLA);
    assert_eq!(g.turn.phase, Phase::Buy);
    assert_eq!((g.turn.buys, g.turn.coins), (1, 1));
    // Gained in the Action phase (Workshop): into the hand with +1 Action.
    let mut g = new_state(&[id::VILLA, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP]);
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::VILLA);
    assert!(g.players[0].hand.has(id::VILLA));
    assert_eq!(g.turn.actions, 1, "Workshop used the Action, the Villa gave it back");
    // Gained on another player's turn: into the hand, no action to use.
    let mut g = new_state(&[id::VILLA, id::WORKSHOP], 2);
    g.gain_card(1, id::VILLA, Dest::Discard, &mut NoEvents);
    assert!(g.players[1].hand.has(id::VILLA));
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn archive_sets_aside_three_cards_and_gives_one_now_and_one_at_the_start_of_each_of_the_next_two_turns() {
    let mut g = new_state(&[id::ARCHIVE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::ARCHIVE]);
    let mut deck = vec![id::GOLD, id::SILVER, id::COPPER];
    deck.extend(junk());
    set_deck_known(&mut g, 0, &deck);
    set_deck_known(&mut g, 1, &junk());
    play(&mut g, id::ARCHIVE);
    assert_eq!(g.turn.actions, 1);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { from: Zone::Archive, act: Act::ToHand, .. })));
    pick(&mut g, id::GOLD);
    assert_eq!(g.turn.coins, 3, "the Gold taken into the hand was played in the Buy phase");
    assert_eq!(g.players[0].archive.total(), 2);
    assert_eq!(count(&g, 0, id::GOLD) + count(&g, 0, id::SILVER) + count(&g, 0, id::COPPER), 3, "set-aside cards are still owned");
    // The next turn: a choice among the two left.
    pass_until(&mut g, |g| g.turn.player == 0 && g.turn.number > 1 && matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Select { from: Zone::Archive, .. }, .. })));
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].archive.total(), 1);
    assert!(g.players[0].in_play.has(id::ARCHIVE), "Archive stays in play while it has cards for another turn");
    // The last card is taken without asking (it is the only one), and that turn is the Archive's last: it is discarded in its Clean-up.
    pass_until(&mut g, |g| g.turn.player == 0 && g.turn.number > 3);
    assert!(g.players[0].archive.is_empty() && g.players[0].in_play.has(id::ARCHIVE));
    assert_eq!(g.turn.coins, 1, "the Copper, the last of the three, was drawn and played this turn");
    pass_until(&mut g, |g| g.turn.player == 1 && g.turn.number > 4);
    assert!(!g.players[0].in_play.has(id::ARCHIVE));
    assert_eq!(count(&g, 0, id::ARCHIVE), 1);
}

#[test]
fn charm_gives_buy_and_coins_or_a_second_card_of_the_same_cost_with_the_next_gain() {
    let mut g = new_state(&[id::CHARM, id::FORUM, id::MARKET, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CHARM, id::COPPER]);
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    expect_decision(&mut g);
    play_treasure(&mut g, id::CHARM);
    choose(&mut g, Choice::Mode(0));
    assert_eq!((g.turn.coins, g.turn.buys), (2 + 1, 1 + 1));
    // The other option: the next gain also gains a differently named card with the same cost.
    let mut g = new_state(&[id::CHARM, id::FORUM, id::MARKET, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CHARM, id::GOLD, id::SILVER]);
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    expect_decision(&mut g);
    play_treasure(&mut g, id::CHARM);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.turn.coins, 5, "the Gold and the Silver were played");
    buy(&mut g, id::FORUM);
    // Forum ($5): Market ($5) may come too; not another Forum.
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::MARKET)) && !c.contains(&Choice::Card(id::FORUM)) && c.contains(&Choice::Pass), "{c:?}");
    pick(&mut g, id::MARKET);
    assert_eq!((count(&g, 0, id::FORUM), count(&g, 0, id::MARKET)), (1, 1));
    assert_eq!(g.turn.buys, 1, "the Buy used on Forum, and the Buy Forum gives when gained");
}

#[test]
fn crown_plays_an_action_twice_in_the_action_phase_and_a_treasure_twice_in_the_buy_phase() {
    let mut g = new_state(&[id::CROWN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CROWN, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 1;
    play(&mut g, id::CROWN);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 6, "two Smithies, one Action used");
    let mut g = new_state(&[id::CROWN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CROWN, id::GOLD]);
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    expect_decision(&mut g);
    play_treasure(&mut g, id::CROWN);
    pick(&mut g, id::GOLD);
    assert_eq!(g.turn.coins, 6);
    assert_eq!(g.players[0].in_play.get(id::GOLD), 1);
}

#[test]
fn forum_draws_three_discards_two_and_gives_a_buy_when_gained() {
    let mut g = new_state(&[id::FORUM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::FORUM, id::COPPER, id::ESTATE, id::VILLAGE]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::SILVER, id::SILVER, id::GOLD]);
    play(&mut g, id::FORUM);
    assert_eq!(g.turn.actions, 1);
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, min: 2, max: 2, .. })));
    pick(&mut g, id::COPPER);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[0].hand.total(), 4, "3 drawn, 2 discarded, the Village");
    let mut g = new_state(&[id::FORUM, id::SMITHY], 2);
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    buy(&mut g, id::FORUM);
    assert_eq!(g.turn.buys, 1, "the Buy used and the Buy gained");
}

#[test]
fn groundskeeper_gives_a_vp_token_for_each_victory_card_gained_this_turn_per_groundskeeper() {
    let mut g = new_state(&[id::GROUNDSKEEPER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::GROUNDSKEEPER, id::GROUNDSKEEPER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    g.turn.coins = 12;
    g.turn.buys = 2;
    play(&mut g, id::GROUNDSKEEPER);
    play(&mut g, id::GROUNDSKEEPER);
    buy(&mut g, id::DUCHY);
    assert_eq!(g.players[0].vp_tokens, 2);
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].vp_tokens, 2, "not a Victory card");
}

#[test]
fn legionary_reveals_a_gold_to_make_the_others_discard_to_two_and_draw_one() {
    let mut g = new_state(&[id::LEGIONARY, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::LEGIONARY, id::GOLD]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    set_deck_known(&mut g, 1, &[id::SILVER, id::SILVER]);
    play(&mut g, id::LEGIONARY);
    assert_eq!(g.turn.coins, 3);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[1].hand.total(), 2 + 1);
    assert!(g.players[1].hand.has(id::SILVER));
    // Without revealing, nothing happens to them.
    let mut g = new_state(&[id::LEGIONARY, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::LEGIONARY, id::GOLD]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::LEGIONARY);
    pass(&mut g);
    assert_eq!(g.players[1].hand.total(), 5);
}

#[test]
fn wild_hunt_draws_and_adds_a_vp_token_or_gains_an_estate_and_takes_the_tokens() {
    let mut g = new_state(&[id::WILD_HUNT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WILD_HUNT, id::WILD_HUNT]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 2;
    play(&mut g, id::WILD_HUNT);
    choose(&mut g, Choice::Mode(0));
    assert_eq!((g.players[0].hand.total(), g.emp.piles.vp_on(id::WILD_HUNT)), (1 + 3, 1));
    let estates = g.supply.get(id::ESTATE);
    play(&mut g, id::WILD_HUNT);
    choose(&mut g, Choice::Mode(1));
    assert_eq!((g.players[0].vp_tokens, g.emp.piles.vp_on(id::WILD_HUNT)), (1, 0));
    assert_eq!(g.supply.get(id::ESTATE), estates - 1);
    // With no Estates left the option still exists but gives nothing.
    let mut g = new_state(&[id::WILD_HUNT, id::SMITHY], 2);
    g.supply.set(id::ESTATE, 0);
    g.emp.piles.add_vp(id::WILD_HUNT, 3);
    set_hand(&mut g, 0, &[id::WILD_HUNT]);
    play(&mut g, id::WILD_HUNT);
    choose(&mut g, Choice::Mode(1));
    assert_eq!((g.players[0].vp_tokens, g.emp.piles.vp_on(id::WILD_HUNT)), (0, 3));
}

#[test]
fn emporium_gives_two_vp_tokens_when_gained_with_five_actions_in_play() {
    let mut g = new_state(&[id::PATRICIAN, id::SMITHY], 2);
    set_in_play(&mut g, 0, &[id::VILLAGE; 5]);
    g.supply.set(id::PATRICIAN, 0);
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    buy(&mut g, id::EMPORIUM);
    assert_eq!(g.players[0].vp_tokens, 2);
    let mut g = new_state(&[id::PATRICIAN, id::SMITHY], 2);
    set_in_play(&mut g, 0, &[id::VILLAGE; 4]);
    g.supply.set(id::PATRICIAN, 0);
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    buy(&mut g, id::EMPORIUM);
    assert_eq!(g.players[0].vp_tokens, 0);
}

#[test]
fn plunder_gives_a_vp_token_each_time_it_is_played() {
    let mut g = new_state(&[id::ENCAMPMENT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::PLUNDER, id::PLUNDER, id::SILVER]);
    g.turn.phase = Phase::Buy;
    g.pending = Pending::None;
    adv(&mut g);
    assert_eq!((g.turn.coins, g.players[0].vp_tokens), (2 + 2 + 2, 2));
}

// ===========================================================================
// The Castles
// ===========================================================================

#[test]
fn castles_score_by_their_own_rules() {
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HUMBLE_CASTLE, id::CRUMBLING_CASTLE, id::KINGS_CASTLE]);
    // Humble Castle: 1 VP per Castle (3); Crumbling: 1; King's Castle: 2 per Castle (6).
    assert_eq!(g.player_vp(0), 3 + 1 + 6);
    set_hand(&mut g, 0, &[id::HUMBLE_CASTLE, id::HUMBLE_CASTLE, id::SMALL_CASTLE, id::GRAND_CASTLE, id::SPRAWLING_CASTLE, id::HAUNTED_CASTLE, id::OPULENT_CASTLE]);
    assert_eq!(g.player_vp(0), 2 * 7 + 2 + 5 + 4 + 2 + 3);
}

#[test]
fn crumbling_castle_gives_a_vp_token_and_a_silver_when_gained_or_trashed() {
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    g.supply.set(id::HUMBLE_CASTLE, 0);
    buy_phase(&mut g, 4, 2);
    adv(&mut g);
    buy(&mut g, id::CRUMBLING_CASTLE);
    assert_eq!((g.players[0].vp_tokens, count(&g, 0, id::SILVER)), (1, 1));
    // Trashed by a Catapult: the same.
    let mut g = new_state(&[id::CASTLES, id::CATAPULT], 2);
    set_hand(&mut g, 0, &[id::CATAPULT, id::CRUMBLING_CASTLE, id::ESTATE]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    play(&mut g, id::CATAPULT);
    pick(&mut g, id::CRUMBLING_CASTLE);
    assert_eq!((g.players[0].vp_tokens, count(&g, 0, id::SILVER)), (1, 1));
}

#[test]
fn small_castle_trashes_itself_or_a_castle_from_the_hand_to_gain_the_top_castle() {
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMALL_CASTLE]);
    play(&mut g, id::SMALL_CASTLE);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.trash.get(id::SMALL_CASTLE), 1);
    assert_eq!(count(&g, 0, id::HUMBLE_CASTLE), 1, "the top Castle");
    // Declining the Small Castle: a Castle from the hand instead.
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SMALL_CASTLE, id::OPULENT_CASTLE, id::ESTATE]);
    play(&mut g, id::SMALL_CASTLE);
    choose(&mut g, Choice::No);
    pick(&mut g, id::OPULENT_CASTLE);
    assert_eq!(g.trash.get(id::OPULENT_CASTLE), 1);
    assert_eq!(count(&g, 0, id::HUMBLE_CASTLE), 1);
    assert_eq!(count(&g, 0, id::SMALL_CASTLE), 1);
}

#[test]
fn haunted_castle_gains_a_gold_and_makes_the_others_with_five_cards_topdeck_two_but_only_on_its_owners_turn() {
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    for i in 0..3 {
        g.supply.set(id::HUMBLE_CASTLE + i, 0);
    }
    set_hand(&mut g, 1, &[id::COPPER, id::COPPER, id::SILVER, id::ESTATE, id::GOLD]);
    buy_phase(&mut g, 6, 2);
    adv(&mut g);
    buy(&mut g, id::HAUNTED_CASTLE);
    assert_eq!(count(&g, 0, id::GOLD), 1);
    // The other player puts 2 cards from their hand onto their deck.
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 1);
    pick(&mut g, id::COPPER);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[1].hand.total(), 3);
    assert_eq!(g.players[1].deck_known.len, 2);
    // Gained on another player's turn: nothing.
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    for i in 0..3 {
        g.supply.set(id::HUMBLE_CASTLE + i, 0);
    }
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    g.gain_card(1, id::HAUNTED_CASTLE, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!((count(&g, 1, id::GOLD), count(&g, 0, id::GOLD)), (0, 0));
}

#[test]
fn opulent_castle_gives_two_coins_per_victory_card_discarded() {
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::OPULENT_CASTLE, id::ESTATE, id::DUCHY, id::COPPER, id::VILLAGE]);
    play(&mut g, id::OPULENT_CASTLE);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::DUCHY);
    assert_eq!(g.turn.coins, 4 + 1, "$2 per Victory card, then the Copper in the Buy phase");
    assert_eq!(g.players[0].discard.get(id::ESTATE), 1);
}

#[test]
fn sprawling_castle_gains_a_duchy_or_three_estates_and_grand_castle_counts_victory_cards() {
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    for i in 0..5 {
        g.supply.set(id::HUMBLE_CASTLE + i, 0);
    }
    buy_phase(&mut g, 8, 1);
    adv(&mut g);
    buy(&mut g, id::SPRAWLING_CASTLE);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(count(&g, 0, id::ESTATE), 3);
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    for i in 0..5 {
        g.supply.set(id::HUMBLE_CASTLE + i, 0);
    }
    buy_phase(&mut g, 8, 1);
    adv(&mut g);
    buy(&mut g, id::SPRAWLING_CASTLE);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(count(&g, 0, id::DUCHY), 1);
    // Grand Castle: +1 VP token per Victory card in the hand and in play.
    let mut g = new_state(&[id::CASTLES, id::SMITHY], 2);
    for i in 0..6 {
        g.supply.set(id::HUMBLE_CASTLE + i, 0);
    }
    set_hand(&mut g, 0, &[id::ESTATE, id::DUCHY, id::COPPER]);
    set_in_play(&mut g, 0, &[id::OPULENT_CASTLE]);
    buy_phase(&mut g, 9, 1);
    g.pending = Pending::None;
    adv(&mut g);
    buy(&mut g, id::GRAND_CASTLE);
    assert_eq!(g.players[0].vp_tokens, 3);
}

// ===========================================================================
// The Events
// ===========================================================================

fn event_game(kingdom: &[CardId]) -> GameState {
    let mut g = new_state(kingdom, 2);
    set_deck_known(&mut g, 0, &junk());
    g
}

#[test]
fn advance_trashes_an_action_for_an_action_costing_up_to_six() {
    let mut g = event_game(&[id::ADVANCE, id::SMITHY, id::VILLAGE, id::FORUM]);
    set_hand(&mut g, 0, &[id::VILLAGE, id::COPPER]);
    g.turn.actions = 0;
    buy_phase(&mut g, 0, 1);
    adv(&mut g);
    buy(&mut g, id::ADVANCE);
    pick(&mut g, id::VILLAGE);
    assert!(has_choice(&g, Choice::Card(id::FORUM)) && !has_choice(&g, Choice::Card(id::SILVER)), "an Action costing up to $6");
    pick(&mut g, id::FORUM);
    assert_eq!((g.trash.get(id::VILLAGE), count(&g, 0, id::FORUM)), (1, 1));
}

#[test]
fn annex_shuffles_the_discard_pile_into_the_deck_but_up_to_five_chosen_cards_and_gains_a_duchy() {
    let mut g = event_game(&[id::ANNEX, id::SMITHY]);
    set_discard(&mut g, 0, &[id::CURSE, id::CURSE, id::ESTATE, id::COPPER, id::COPPER, id::GOLD, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::PROVINCE]);
    buy_phase(&mut g, 0, 2);
    adv(&mut g);
    buy(&mut g, id::ANNEX);
    assert_eq!(g.players[0].debt, 8);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    pick(&mut g, id::CURSE);
    pass(&mut g);
    // The Duchy is in the discard pile with the three kept cards; the rest (and the known top cards) are the deck.
    // (The Debt ended the turn, and the discard pile is as the Annex left it: the three kept cards and the Duchy.)
    assert_eq!(g.players[0].discard.total(), 3 + 1);
    assert_eq!((g.players[0].discard.get(id::DUCHY), g.players[0].discard.get(id::CURSE), g.players[0].discard.get(id::ESTATE)), (1, 2, 1));
}

#[test]
fn banquet_gains_two_coppers_and_a_card_costing_up_to_five_that_is_not_a_victory_card() {
    let mut g = event_game(&[id::BANQUET, id::SMITHY, id::FORUM]);
    buy_phase(&mut g, 3, 1);
    adv(&mut g);
    buy(&mut g, id::BANQUET);
    assert!(!has_choice(&g, Choice::Card(id::DUCHY)) && has_choice(&g, Choice::Card(id::FORUM)) && has_choice(&g, Choice::Card(id::SILVER)));
    pick(&mut g, id::FORUM);
    assert_eq!((count(&g, 0, id::COPPER), count(&g, 0, id::FORUM)), (2, 1));
}

#[test]
fn conquest_gains_two_silvers_and_a_vp_token_per_silver_gained_this_turn() {
    let mut g = event_game(&[id::CONQUEST, id::SMITHY]);
    buy_phase(&mut g, 9, 2);
    adv(&mut g);
    buy(&mut g, id::SILVER);
    buy(&mut g, id::CONQUEST);
    assert_eq!(count(&g, 0, id::SILVER), 3);
    assert_eq!(g.players[0].vp_tokens, 3);
}

#[test]
fn delve_gives_a_buy_back_and_a_silver() {
    let mut g = event_game(&[id::DELVE, id::SMITHY]);
    buy_phase(&mut g, 4, 1);
    adv(&mut g);
    buy(&mut g, id::DELVE);
    buy(&mut g, id::DELVE);
    assert_eq!((g.turn.buys, g.turn.coins, count(&g, 0, id::SILVER)), (1, 0, 2));
}

#[test]
fn dominate_gains_a_province_and_nine_vp_tokens_only_if_the_province_is_gained() {
    let mut g = event_game(&[id::DOMINATE, id::SMITHY]);
    buy_phase(&mut g, 14, 1);
    adv(&mut g);
    let provinces = g.supply.get(id::PROVINCE);
    buy(&mut g, id::DOMINATE);
    assert_eq!((g.supply.get(id::PROVINCE), g.players[0].vp_tokens), (provinces - 1, 9));
    let mut g = event_game(&[id::DOMINATE, id::SMITHY]);
    g.supply.set(id::PROVINCE, 0);
    buy_phase(&mut g, 14, 1);
    adv(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::DOMINATE)), "it could do nothing");
}

#[test]
fn donate_trashes_from_the_whole_deck_at_the_start_of_the_next_turn_and_draws_five() {
    let mut g = event_game(&[id::DONATE, id::SMITHY]);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER]);
    set_discard(&mut g, 0, &[id::CURSE, id::ESTATE, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::GOLD, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::SILVER]);
    buy_phase(&mut g, 0, 1);
    adv(&mut g);
    buy(&mut g, id::DONATE);
    assert_eq!(g.players[0].debt, 8);
    assert!(!has_choice(&g, Choice::Card(id::DONATE)) || g.turn.player != 0);
    // The next turn of player 1 begins and ends; then player 0's turn starts with Donate.
    pass_until(&mut g, |g| g.turn.player == 0 && g.turn.number > 1 && matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::Select { act: Act::Trash, .. }, .. })));
    // Everything is in the hand now (the cards drawn at Clean-up were put away with the rest).
    let all_before = g.players[0].all_cards().total();
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, 0);
    assert!(g.players[0].deck_size() == 0 && g.players[0].discard.is_empty(), "deck and discard pile are in the hand");
    pick(&mut g, id::CURSE);
    pick(&mut g, id::CURSE);
    pass(&mut g);
    assert_eq!(g.players[0].hand.total(), 5, "5 cards drawn");
    assert_eq!(g.players[0].all_cards().total(), all_before - 2);
    assert_eq!(g.trash.get(id::CURSE), 2);
}

#[test]
fn ritual_gains_a_curse_and_trashes_a_card_for_a_vp_token_per_coin_it_costs() {
    let mut g = event_game(&[id::RITUAL, id::SMITHY]);
    set_hand(&mut g, 0, &[id::GOLD, id::ESTATE]);
    buy_phase(&mut g, 4, 1);
    g.turn.treasures_done = true;
    adv(&mut g);
    buy(&mut g, id::RITUAL);
    assert_eq!(count(&g, 0, id::CURSE), 1);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].vp_tokens, 6);
    // No Curses left: nothing at all.
    let mut g = event_game(&[id::RITUAL, id::SMITHY]);
    g.supply.set(id::CURSE, 0);
    set_hand(&mut g, 0, &[id::GOLD]);
    buy_phase(&mut g, 4, 1);
    adv(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::RITUAL)));
}

#[test]
fn salt_the_earth_gives_a_vp_token_and_trashes_a_victory_card_from_the_supply() {
    let mut g = event_game(&[id::SALT_THE_EARTH, id::SMITHY]);
    buy_phase(&mut g, 4, 1);
    adv(&mut g);
    buy(&mut g, id::SALT_THE_EARTH);
    assert_eq!(g.players[0].vp_tokens, 1);
    pick(&mut g, id::PROVINCE);
    assert_eq!((g.trash.get(id::PROVINCE), g.supply.get(id::PROVINCE)), (1, 7));
}

#[test]
fn tax_starts_with_one_debt_on_every_pile_adds_two_and_is_taken_by_a_gain_in_the_buy_phase() {
    let mut g = event_game(&[id::TAX, id::SMITHY, id::VILLAGE]);
    for c in [id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY, id::PROVINCE, id::CURSE, id::SMITHY, id::VILLAGE] {
        assert_eq!(g.emp.piles.debt_on(c), 1, "{}", cards::name(c));
    }
    buy_phase(&mut g, 7, 3);
    adv(&mut g);
    buy(&mut g, id::TAX);
    pick(&mut g, id::SILVER);
    assert_eq!(g.emp.piles.debt_on(id::SILVER), 3);
    // Buying a Silver takes its 3 Debt (and the Buy phase goes on: the player pays them with the $ left).
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].debt, 3);
    assert_eq!(g.emp.piles.debt_on(id::SILVER), 0);
}

#[test]
fn triumph_gains_an_estate_and_a_vp_token_per_card_gained_this_turn() {
    let mut g = event_game(&[id::TRIUMPH, id::SMITHY]);
    buy_phase(&mut g, 3, 3);
    adv(&mut g);
    buy(&mut g, id::SILVER);
    buy(&mut g, id::TRIUMPH);
    assert_eq!(g.players[0].debt, 5);
    assert_eq!(g.players[0].vp_tokens, 2, "the Silver and the Estate");
}

#[test]
fn wedding_gives_a_vp_token_and_a_gold_for_four_coins_and_three_debt() {
    let mut g = event_game(&[id::WEDDING, id::SMITHY]);
    buy_phase(&mut g, 4, 1);
    adv(&mut g);
    buy(&mut g, id::WEDDING);
    assert_eq!((g.players[0].vp_tokens, count(&g, 0, id::GOLD), g.players[0].debt), (1, 1, 3));
}

#[test]
fn windfall_gains_three_golds_only_with_an_empty_deck_and_discard_pile() {
    let mut g = event_game(&[id::WINDFALL, id::SMITHY]);
    clear_deck(&mut g, 0);
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    buy(&mut g, id::WINDFALL);
    assert_eq!(count(&g, 0, id::GOLD), 3);
    let mut g = event_game(&[id::WINDFALL, id::SMITHY]);
    buy_phase(&mut g, 5, 1);
    adv(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::WINDFALL)), "the deck is not empty");
}

// ===========================================================================
// The Landmarks
// ===========================================================================

#[test]
fn landmarks_with_tokens_start_with_six_per_player() {
    let g = new_state(&[id::ARENA, id::BASILICA, id::BATHS, id::BATTLEFIELD, id::COLONNADE, id::LABYRINTH, id::AQUEDUCT, id::DEFILED_SHRINE, id::SMITHY], 3);
    for l in [id::ARENA, id::BASILICA, id::BATHS, id::BATTLEFIELD, id::COLONNADE, id::LABYRINTH] {
        assert_eq!(g.emp.lm_vp[dominion_engine::sets::empires::lm_index(l)], 18, "{}", cards::name(l));
    }
    assert_eq!(g.emp.piles.vp_on(id::SILVER), 8);
    assert_eq!(g.emp.piles.vp_on(id::GOLD), 8);
    assert_eq!(g.emp.piles.vp_on(id::SMITHY), 2, "an Action pile");
    assert_eq!(g.emp.piles.vp_on(id::COPPER), 0);
}

fn lm(g: &GameState, l: CardId) -> u8 {
    g.emp.lm_vp[dominion_engine::sets::empires::lm_index(l)]
}

#[test]
fn aqueduct_moves_a_vp_token_from_a_treasure_pile_and_a_victory_card_takes_them() {
    let mut g = event_game(&[id::AQUEDUCT, id::SMITHY]);
    buy_phase(&mut g, 13, 3);
    adv(&mut g);
    buy(&mut g, id::SILVER);
    buy(&mut g, id::GOLD);
    assert_eq!((lm(&g, id::AQUEDUCT), g.emp.piles.vp_on(id::SILVER), g.emp.piles.vp_on(id::GOLD)), (2, 7, 7));
    buy(&mut g, id::ESTATE);
    assert_eq!((lm(&g, id::AQUEDUCT), g.players[0].vp_tokens), (0, 2));
}

#[test]
fn arena_lets_an_action_be_discarded_at_the_start_of_the_buy_phase_for_two_vp_tokens() {
    let mut g = event_game(&[id::ARENA, id::SMITHY]);
    set_hand(&mut g, 0, &[id::VILLAGE, id::COPPER]);
    g.turn.actions = 0;
    g.turn.phase = Phase::Action;
    adv(&mut g);
    // The Action phase ends (no Actions left); the Buy phase starts with the offer.
    assert!(matches!(pending_kind(&g), Some(DecisionKind::Select { act: Act::Discard, .. })));
    pick(&mut g, id::VILLAGE);
    assert_eq!((g.players[0].vp_tokens, lm(&g, id::ARENA)), (2, 12 - 2));
}

#[test]
fn basilica_gives_two_vp_tokens_for_a_gain_in_the_buy_phase_with_two_coins_left() {
    let mut g = event_game(&[id::BASILICA, id::SMITHY]);
    buy_phase(&mut g, 5, 3);
    adv(&mut g);
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].vp_tokens, 2, "$2 left");
    buy(&mut g, id::ESTATE);
    assert_eq!(g.players[0].vp_tokens, 2, "no $ left");
}

#[test]
fn baths_gives_two_vp_tokens_to_a_player_who_ends_a_turn_without_gaining_a_card() {
    let mut g = event_game(&[id::BATHS, id::SMITHY]);
    buy_phase(&mut g, 0, 1);
    adv(&mut g);
    pass(&mut g);
    assert_eq!(g.players[0].vp_tokens, 2);
    let mut g = event_game(&[id::BATHS, id::SMITHY]);
    buy_phase(&mut g, 3, 2);
    adv(&mut g);
    buy(&mut g, id::SILVER);
    pass(&mut g);
    assert_eq!(g.players[0].vp_tokens, 0);
}

#[test]
fn battlefield_gives_two_vp_tokens_for_each_victory_card_gained() {
    let mut g = event_game(&[id::BATTLEFIELD, id::SMITHY]);
    buy_phase(&mut g, 7, 3);
    adv(&mut g);
    buy(&mut g, id::ESTATE);
    buy(&mut g, id::ESTATE);
    buy(&mut g, id::SILVER);
    assert_eq!((g.players[0].vp_tokens, lm(&g, id::BATTLEFIELD)), (4, 12 - 4));
}

#[test]
fn colonnade_rewards_gaining_an_action_in_the_buy_phase_while_a_copy_is_in_play() {
    let mut g = event_game(&[id::COLONNADE, id::SMITHY, id::VILLAGE]);
    set_in_play(&mut g, 0, &[id::SMITHY]);
    buy_phase(&mut g, 8, 3);
    adv(&mut g);
    buy(&mut g, id::VILLAGE);
    assert_eq!(g.players[0].vp_tokens, 0);
    buy(&mut g, id::SMITHY);
    assert_eq!(g.players[0].vp_tokens, 2);
}

#[test]
fn defiled_shrine_collects_vp_from_action_piles_and_a_curse_bought_takes_them() {
    let mut g = event_game(&[id::DEFILED_SHRINE, id::SMITHY, id::TEMPLE, id::VILLAGE]);
    assert_eq!((g.emp.piles.vp_on(id::SMITHY), g.emp.piles.vp_on(id::TEMPLE)), (2, 0), "not on the Gathering piles");
    buy_phase(&mut g, 7, 3);
    adv(&mut g);
    buy(&mut g, id::SMITHY);
    buy(&mut g, id::VILLAGE);
    assert_eq!((lm(&g, id::DEFILED_SHRINE), g.emp.piles.vp_on(id::SMITHY)), (2, 1));
    buy(&mut g, id::CURSE);
    assert_eq!((g.players[0].vp_tokens, lm(&g, id::DEFILED_SHRINE)), (2, 0));
}

#[test]
fn labyrinth_gives_two_vp_tokens_for_the_second_card_gained_in_a_turn_only() {
    let mut g = event_game(&[id::LABYRINTH, id::SMITHY]);
    buy_phase(&mut g, 9, 4);
    adv(&mut g);
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].vp_tokens, 0);
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].vp_tokens, 2);
    buy(&mut g, id::SILVER);
    assert_eq!(g.players[0].vp_tokens, 2);
}

#[test]
fn tomb_gives_a_vp_token_for_each_card_trashed() {
    let mut g = event_game(&[id::TOMB, id::CHAPEL]);
    set_hand(&mut g, 0, &[id::CHAPEL, id::COPPER, id::COPPER, id::ESTATE, id::ESTATE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::COPPER);
    pick(&mut g, id::COPPER);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[0].vp_tokens, 3);
}

#[test]
fn mountain_pass_has_each_player_bid_debt_for_eight_vp_when_the_first_province_is_gained() {
    let mut g = new_state(&[id::MOUNTAIN_PASS, id::SMITHY], 3);
    buy_phase(&mut g, 8, 1);
    adv(&mut g);
    buy(&mut g, id::PROVINCE);
    // The player to the left of the buyer bids first, then the next, and the buyer last.
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (1, DecisionKind::Bid { min: 1 }));
    choose(&mut g, Choice::Bid(3));
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (2, DecisionKind::Bid { min: 4 }));
    assert!(!has_choice(&g, Choice::Bid(3)) && has_choice(&g, Choice::Bid(4)) && has_choice(&g, Choice::Bid(40)) && has_choice(&g, Choice::Bid(0)));
    choose(&mut g, Choice::Bid(0));
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (0, DecisionKind::Bid { min: 4 }));
    choose(&mut g, Choice::Bid(0));
    // The high bidder (player 2) takes 8 VP tokens and 3 Debt.
    assert_eq!((g.players[1].vp_tokens, g.players[1].debt), (8, 3));
    assert_eq!((g.players[0].debt, g.players[2].debt), (0, 0));
    // Only the first Province starts the bidding.
    let mut g2 = g;
    g2.turn.player = 0;
    g2.turn.phase = Phase::Buy;
    g2.turn.coins = 8;
    g2.turn.buys = 1;
    g2.turn.treasures_done = true;
    g2.pending = Pending::None;
    g2.stack = Default::default();
    adv(&mut g2);
    buy(&mut g2, id::PROVINCE);
    assert!(!matches!(pending_kind(&g2), Some(DecisionKind::Bid { .. })));
}

// --- the scoring Landmarks ---

fn owns(g: &mut GameState, cards_: &[CardId]) {
    let mut all = Counts::EMPTY;
    for &c in cards_ {
        all.add(c, 1);
    }
    g.players[0].discard = all;
}

#[test]
fn bandit_fort_fountain_and_palace_score_from_the_treasures() {
    let mut g = new_state(&[id::BANDIT_FORT, id::FOUNTAIN, id::PALACE, id::SMITHY], 2);
    owns(&mut g, &[id::SILVER, id::SILVER, id::SILVER, id::GOLD]);
    assert_eq!(g.player_vp(0), -8);
    let mut g = new_state(&[id::FOUNTAIN, id::SMITHY], 2);
    owns(&mut g, &[id::COPPER; 9]);
    assert_eq!(g.player_vp(0), 0);
    owns(&mut g, &[id::COPPER; 10]);
    assert_eq!(g.player_vp(0), 15);
    owns(&mut g, &[id::COPPER; 20]);
    assert_eq!(g.player_vp(0), 15, "no bonus for 20");
    let mut g = new_state(&[id::PALACE, id::SMITHY], 2);
    let mut cs = vec![id::COPPER; 7];
    cs.extend([id::SILVER; 5]);
    cs.extend([id::GOLD; 2]);
    owns(&mut g, &cs);
    assert_eq!(g.player_vp(0), 6, "two sets of Copper-Silver-Gold");
}

#[test]
fn keep_gives_five_vp_per_treasure_you_have_the_most_or_tied_for_the_most_of() {
    let mut g = new_state(&[id::KEEP, id::SMITHY], 2);
    owns(&mut g, &[id::COPPER, id::COPPER, id::SILVER, id::GOLD]);
    g.players[1].discard = counts_of(&[id::COPPER, id::SILVER, id::SILVER]);
    // Player 1: Copper (2 vs 1) and Gold (1 vs 0); Silver is behind (1 vs 2).
    assert_eq!(g.player_vp(0), 10);
    // Player 2: Silver only.
    assert_eq!(g.player_vp(1), 5);
    // Ties count for everyone.
    g.players[1].discard = counts_of(&[id::COPPER, id::COPPER, id::SILVER, id::GOLD]);
    assert_eq!((g.player_vp(0), g.player_vp(1)), (15, 15));
}

#[test]
fn museum_obelisk_orchard_triumphal_arch_wall_and_wolf_den_score_from_the_deck() {
    let mut g = new_state(&[id::MUSEUM, id::SMITHY], 2);
    owns(&mut g, &[id::COPPER, id::COPPER, id::SILVER, id::ESTATE]);
    assert_eq!(g.player_vp(0), 1 + 2 * 3, "the Estate and 2 per differently named card");
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::OBELISK, id::SMITHY, id::VILLAGE], seed: 1, obelisk: Some(id::SMITHY), ..Default::default() };
    let mut g = GameState::new(&cfg);
    g.players[0] = Default::default();
    owns(&mut g, &[id::SMITHY, id::SMITHY, id::VILLAGE]);
    assert_eq!(g.player_vp(0), 4);
    let mut g = new_state(&[id::ORCHARD, id::SMITHY], 2);
    owns(&mut g, &[id::SMITHY, id::SMITHY, id::SMITHY, id::VILLAGE, id::VILLAGE, id::MARKET, id::MARKET, id::MARKET, id::MARKET, id::MARKET, id::MARKET]);
    assert_eq!(g.player_vp(0), 8, "Smithy and Market: 4 each; 6 copies is no more than 3");
    let mut g = new_state(&[id::TRIUMPHAL_ARCH, id::SMITHY], 2);
    let mut cs = vec![id::VILLAGE; 7];
    cs.extend([id::WILD_HUNT; 4]);
    owns(&mut g, &cs);
    assert_eq!(g.player_vp(0), 12);
    let mut cs = vec![id::VILLAGE; 7];
    cs.extend([id::SMITHY; 4]);
    cs.extend([id::WILD_HUNT; 4]);
    owns(&mut g, &cs);
    assert_eq!(g.player_vp(0), 12, "a tie for second counts either");
    let mut g = new_state(&[id::WALL, id::SMITHY], 2);
    owns(&mut g, &[id::COPPER; 27]);
    assert_eq!(g.player_vp(0), -12);
    let mut g = new_state(&[id::WOLF_DEN, id::SMITHY], 2);
    owns(&mut g, &[id::COPPER, id::COPPER, id::SILVER, id::GOLD, id::ESTATE]);
    assert_eq!(g.player_vp(0), -9 + 1, "Silver, Gold and Estate: one copy each, and the Estate is worth 1");
}

#[test]
fn tower_scores_a_point_for_each_non_victory_card_from_an_empty_supply_pile() {
    let mut g = new_state(&[id::TOWER, id::SMITHY, id::PATRICIAN], 2);
    owns(&mut g, &[id::SMITHY, id::SMITHY, id::CURSE, id::ESTATE, id::PATRICIAN]);
    assert_eq!(g.player_vp(0), 1 - 1 + 0, "nothing is empty: the Estate and the Curse");
    g.supply.set(id::SMITHY, 0);
    g.supply.set(id::CURSE, 0);
    g.supply.set(id::ESTATE, 0);
    // The Smithies (2) and the Curse (1) count; the Estate is a Victory card.
    assert_eq!(g.player_vp(0), 1 - 1 + 3);
    // A split pile with only the top half gone is not empty.
    g.supply.set(id::PATRICIAN, 0);
    assert_eq!(g.player_vp(0), 1 - 1 + 3);
    g.supply.set(id::EMPORIUM, 0);
    assert_eq!(g.player_vp(0), 1 - 1 + 3 + 1);
}
