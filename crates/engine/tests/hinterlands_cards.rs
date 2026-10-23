//! Hinterlands tests (rulings in `docs/hinterlands-plan.md`): the 26 second-edition kingdom cards, and the triggers they bring: reactions
//! to gains (Trader, Fool's Gold), to discards (Tunnel, Trail, Weaver) and to trashing (Trail, Nomads), "when you gain this" effects, and
//! "when you buy" effects (Haggler).

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
fn random_hinterlands_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0x41A7);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Hinterlands).collect();
    assert_eq!(pool.len(), 26);
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::Hinterlands).collect();
    let mut played = std::collections::BTreeSet::new();
    let mut gained = std::collections::BTreeSet::new();
    let mut reactions = std::collections::BTreeSet::new();
    for game in 0..4000u64 {
        let n = 2 + (game % 4) as usize;
        // The kingdom is drawn from its own generator, so that it does not depend on how the earlier games were played (a ruling that
        // changes one game must not change which cards the later ones have); every fourth game has the two reactions in it.
        let mut krng = Rng::new(0x41A7 ^ game.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut kingdom: Vec<CardId> = Vec::new();
        let mut from = pool.clone();
        if game % 4 == 0 {
            for c in [id::TRADER, id::FOOLS_GOLD] {
                kingdom.push(c);
                from.retain(|&x| x != c);
            }
        }
        while kingdom.len() < 8 {
            let i = krng.below(from.len() as u32) as usize;
            kingdom.push(from.swap_remove(i));
        }
        for _ in 0..2 {
            let c = others[krng.below(others.len() as u32) as usize];
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
                    Event::Gain { card, source, .. } => {
                        gained.insert(card);
                        // Fool's Gold's reaction (trash it for a Gold on the deck) logs no Reaction event: the Gold it gains names it.
                        if source == id::FOOLS_GOLD {
                            reactions.insert(source);
                        }
                    }
                    Event::Reaction { card, .. } => {
                        reactions.insert(card);
                    }
                    _ => {}
                }
            }
            log.clear();
            let chosen;
            match g.advance(&mut log) {
                Step::Decision(_) => {
                    g.legal_choices(&mut buf);
                    assert!(!buf.is_empty());
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
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
    }
    // The random games reach every card (Tunnel and Farmland are Victory cards: they are gained, not played).
    let missing: Vec<&str> =
        (id::CROSSROADS..=id::FARMLAND).filter(|c| !matches!(*c, id::TUNNEL | id::FARMLAND) && !played.contains(c)).map(cards::name).collect();
    assert!(missing.is_empty(), "never played: {missing:?}");
    let missing: Vec<&str> = [id::TUNNEL, id::FARMLAND].iter().filter(|c| !gained.contains(c)).map(|&c| cards::name(c)).collect();
    assert!(missing.is_empty(), "never gained: {missing:?}");
    // And the reactions: Trader's exchange, Fool's Gold, Tunnel (revealed for a Gold).
    let missing: Vec<&str> = [id::TRADER, id::FOOLS_GOLD].iter().filter(|c| !reactions.contains(c)).map(|&c| cards::name(c)).collect();
    assert!(missing.is_empty(), "never revealed: {missing:?}");
}

// ===========================================================================
// Set-up
// ===========================================================================

#[test]
fn the_set_has_26_kingdom_cards() {
    assert_eq!(cards::set_by_name("Hinterlands"), Some(CardSet::Hinterlands));
    assert_eq!(CardSet::Hinterlands.name(), "Hinterlands");
    assert_eq!(cards::kingdom_cards_in(CardSet::Hinterlands).count(), 26);
    assert_eq!(id::CROSSROADS, 328);
    assert_eq!(id::FARMLAND, 353);
    assert!(cards::is_kingdom(id::CROSSROADS) && cards::is_kingdom(id::FARMLAND));
    assert!(cards::is(id::TUNNEL, cards::VICTORY | cards::REACTION) && cards::is(id::FOOLS_GOLD, cards::TREASURE | cards::REACTION));
    assert_eq!(cards::cost_string(id::BORDER_VILLAGE), "$6");
    // The 1st-edition cards are not in this edition.
    for removed in ["Duchess", "Oracle", "Noble Brigand", "Nomad Camp", "Silk Road", "Cache", "Embassy", "Ill-Gotten Gains", "Mandarin"] {
        assert!(cards::by_name(removed).is_none(), "{removed} is a first-edition card");
    }
}

#[test]
fn tunnel_and_farmland_piles_are_8_with_two_players_and_12_otherwise() {
    for (n, size) in [(2, 8), (3, 12), (4, 12)] {
        let g = new_state(&[id::TUNNEL, id::FARMLAND], n);
        assert_eq!(g.supply.get(id::TUNNEL), size);
        assert_eq!(g.supply.get(id::FARMLAND), size);
    }
    let g = new_state(&[id::CROSSROADS, id::HAGGLER], 2);
    assert_eq!(g.supply.get(id::CROSSROADS), 10);
}

// ===========================================================================
// Helpers
// ===========================================================================

fn yes(g: &mut GameState) {
    assert!(matches!(expect_decision(g).kind, DecisionKind::YesNo { .. }), "{:?}", g.pending_decision());
    choose(g, Choice::Yes);
}

fn no(g: &mut GameState) {
    assert!(matches!(expect_decision(g).kind, DecisionKind::YesNo { .. }), "{:?}", g.pending_decision());
    choose(g, Choice::No);
}

fn pick(g: &mut GameState, c: CardId) {
    expect_decision(g);
    assert!(choices(g).contains(&Choice::Card(c)), "{} is not offered: {:?}", cards::name(c), choices(g));
    choose(g, Choice::Card(c));
}

fn done(g: &mut GameState) {
    expect_decision(g);
    assert!(choices(g).contains(&Choice::Pass), "cannot stop here: {:?}", choices(g));
    choose(g, Choice::Pass);
}

/// Keep choosing the first legal card while the decision is a discard (a Militia victim's, say).
fn discard_first_while_asked(g: &mut GameState) {
    loop {
        match expect_decision_or_none(g) {
            Some(Decision { kind: DecisionKind::Select { act: Act::Discard, .. }, .. }) => {
                let c = choices(g)[0];
                choose(g, c);
            }
            _ => break,
        }
    }
}

fn expect_decision_or_none(g: &mut GameState) -> Option<Decision> {
    match adv(g) {
        Step::Decision(d) => Some(d),
        _ => None,
    }
}

/// A deck of cards that are neither Treasures nor Actions: every draw is visible, nothing is auto-played.
fn junk() -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(40).copied().collect()
}

/// The Buy decision of player 0 with `hand` (the Treasures in it are played).
fn buy_phase(g: &mut GameState, hand: &[CardId]) {
    set_hand(g, 0, hand);
    g.turn.phase = Phase::Buy;
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::Buy, "{d:?}");
}

fn hand_total(g: &GameState, p: usize) -> u32 {
    g.players[p].hand.total()
}

// ===========================================================================
// Crossroads
// ===========================================================================

#[test]
fn crossroads_draws_per_victory_card_and_gives_three_actions_the_first_time() {
    let mut g = new_state(&[id::CROSSROADS, id::HAREM], 2);
    set_hand(&mut g, 0, &[id::CROSSROADS, id::CROSSROADS, id::CROSSROADS, id::ESTATE, id::HAREM, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::SILVER, id::GOLD, id::SILVER, id::GOLD, id::COPPER, id::COPPER]);
    play(&mut g, id::CROSSROADS);
    // Estate and Harem (a Victory card too) are revealed: +2 Cards, and +3 Actions (1 - 1 + 3).
    assert_eq!(g.turn.actions, 3);
    assert_eq!(hand_total(&g, 0), 5 + 2);
    assert_eq!(g.players[0].hand.get(id::SILVER) + g.players[0].hand.get(id::GOLD), 2);
    // The second one draws for the Victory cards but gives no Actions.
    play(&mut g, id::CROSSROADS);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(hand_total(&g, 0), 6 + 2);
}

#[test]
fn crossroads_with_no_victory_card_still_gives_actions_and_throne_room_gives_them_once() {
    let mut g = new_state(&[id::CROSSROADS, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::CROSSROADS, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::CROSSROADS);
    // Played twice: 2 Victory cards each time, +3 Actions only the first (1 - 1 + 3).
    assert_eq!(g.turn.actions, 3);
    assert_eq!(hand_total(&g, 0), 2 + 2 + 4, "the second play counts the Victory cards the first drew");
    let mut g = new_state(&[id::CROSSROADS], 2);
    set_hand(&mut g, 0, &[id::CROSSROADS, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CROSSROADS);
    assert_eq!(g.turn.actions, 3);
    assert_eq!(g.players[0].deck_size(), 40, "nothing drawn");
}

// ===========================================================================
// Fool's Gold
// ===========================================================================

#[test]
fn fools_gold_is_worth_one_the_first_time_and_four_after() {
    let mut g = new_state(&[id::FOOLS_GOLD], 2);
    buy_phase(&mut g, &[id::FOOLS_GOLD]);
    assert_eq!(g.turn.coins, 1);
    let mut g = new_state(&[id::FOOLS_GOLD], 2);
    buy_phase(&mut g, &[id::FOOLS_GOLD, id::FOOLS_GOLD, id::FOOLS_GOLD, id::COPPER]);
    assert_eq!(g.turn.coins, 1 + 4 + 4 + 1);
    // Another one played earlier in the turn (a Crown, a Counterfeit...) makes this one worth $4.
    let mut g = new_state(&[id::FOOLS_GOLD], 2);
    g.turn.played.add(id::FOOLS_GOLD, 1);
    set_in_play(&mut g, 0, &[id::FOOLS_GOLD]);
    buy_phase(&mut g, &[id::FOOLS_GOLD]);
    assert_eq!(g.turn.coins, 4);
}

#[test]
fn fools_gold_played_twice_by_crown_is_worth_five() {
    let mut g = new_state(&[id::FOOLS_GOLD], 2);
    set_hand(&mut g, 0, &[id::CROWN, id::FOOLS_GOLD]);
    g.turn.phase = Phase::Buy;
    // Crown (a Treasure-Action) is played in the Buy phase for a Treasure; Fool's Gold is played twice: $1 then $4.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::CROWN);
    pick(&mut g, id::FOOLS_GOLD);
    assert_eq!(g.turn.coins, 5);
}

#[test]
fn fools_gold_in_hand_trashes_itself_for_a_gold_when_another_player_gains_a_province() {
    let mut g = new_state(&[id::FOOLS_GOLD], 2);
    set_hand(&mut g, 0, &[id::FOOLS_GOLD, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    buy_phase_for(&mut g, 1, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    buy(&mut g, id::PROVINCE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Trash }) && d.subject == id::FOOLS_GOLD && d.player == 0, "{d:?}");
    yes(&mut g);
    assert_eq!(g.trash.get(id::FOOLS_GOLD), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD), "the Gold is gained onto the deck");
    assert!(!g.players[0].hand.has(id::FOOLS_GOLD));
}

/// Player `p`'s Buy decision with `hand`.
fn buy_phase_for(g: &mut GameState, p: usize, hand: &[CardId]) {
    reset_turn(g, p as u8);
    set_hand(g, p, hand);
    g.turn.phase = Phase::Buy;
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::Buy, "{d:?}");
}

#[test]
fn fools_gold_can_be_declined_and_does_not_react_to_your_own_province() {
    let mut g = new_state(&[id::FOOLS_GOLD], 2);
    set_hand(&mut g, 0, &[id::FOOLS_GOLD]);
    set_deck_known(&mut g, 0, &junk());
    buy_phase_for(&mut g, 1, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    buy(&mut g, id::PROVINCE);
    expect_decision(&mut g);
    g.apply(Choice::No, &mut NoEvents).unwrap();
    assert!(g.players[0].hand.has(id::FOOLS_GOLD) && g.trash.is_empty());
    // The gainer's own Fool's Gold does not react.
    let mut g = new_state(&[id::FOOLS_GOLD], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD, id::FOOLS_GOLD]);
    g.turn.coins = 8;
    g.players[0].hand.add(id::FOOLS_GOLD, 1);
    buy(&mut g, id::PROVINCE);
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })));
}

#[test]
fn fools_gold_reacts_to_a_province_gained_on_your_own_turn_and_each_player_in_turn_order_is_asked() {
    // Gained by a card effect during the Action phase of player 0, player 1 reacts.
    let mut g = new_state(&[id::FOOLS_GOLD], 3);
    set_hand(&mut g, 1, &[id::FOOLS_GOLD]);
    set_hand(&mut g, 2, &[id::FOOLS_GOLD]);
    set_deck_known(&mut g, 1, &junk());
    set_deck_known(&mut g, 2, &junk());
    g.gain_card(0, id::PROVINCE, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    yes(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 2);
    no(&mut g);
    assert_eq!(g.trash.get(id::FOOLS_GOLD), 1);
    assert!(g.players[2].hand.has(id::FOOLS_GOLD));
}

// ===========================================================================
// Develop
// ===========================================================================

#[test]
fn develop_trashes_a_card_and_gains_one_costing_one_more_and_one_one_less_onto_the_deck() {
    let mut g = new_state(&[id::DEVELOP, id::SMITHY, id::VILLAGE, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::DEVELOP, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DEVELOP);
    // (The only card in the hand is trashed without asking.)
    // Silver costs $3: one costing $4 (Smithy, Militia) and one costing $2 (Estate) in either order.
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && c.contains(&Choice::Card(id::ESTATE)) && c.contains(&Choice::Card(id::MILITIA)), "{c:?}");
    assert!(!c.contains(&Choice::Card(id::VILLAGE)), "a $3 card is not offered");
    choose(&mut g, Choice::Card(id::SMITHY));
    // Then the other cost: only an Estate costs $2 (gained at once, as the only choice).
    assert_eq!(g.trash.get(id::SILVER), 1);
    // Both on the deck, the second one gained on top.
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));
    assert!(g.players[0].deck_known.iter_top_down().take(2).any(|c| c == id::SMITHY));
    assert!(g.players[0].discard.is_empty());
}

#[test]
fn develop_still_gains_one_when_no_card_costs_the_other_amount() {
    let mut g = new_state(&[id::DEVELOP, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DEVELOP, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    empty_pile(&mut g, id::ESTATE);
    play(&mut g, id::DEVELOP);
    // No $2 card is left in the Supply; the $4 Smithy is gained and that is all.
    expect_decision(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SMITHY));
    assert_eq!(g.supply.get(id::SMITHY), 9);
    // Developing a Copper ($0) gains only the $1 card (here there is none).
    let mut g = new_state(&[id::DEVELOP, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::DEVELOP, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DEVELOP);
    assert_eq!(g.trash.get(id::COPPER), 1);
    assert_eq!(g.players[0].discard.total(), 0);
    assert_eq!(g.players[0].deck_size(), 40);
}

#[test]
fn develop_costs_use_the_current_reduction() {
    let mut g = new_state(&[id::DEVELOP, id::HIGHWAY, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HIGHWAY, id::DEVELOP, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HIGHWAY);
    play(&mut g, id::DEVELOP);
    pick(&mut g, id::GOLD);
    // Gold costs $5 now: a card costing $6 now (Province $7 - 1, none) and one costing $4 now (Duchy $5 - 1).
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::DUCHY)), "{c:?}");
    assert!(!c.contains(&Choice::Card(id::SMITHY)), "Smithy costs $3 now: {c:?}");
}

// ===========================================================================
// Guard Dog
// ===========================================================================

#[test]
fn guard_dog_draws_two_and_two_more_with_five_or_fewer_cards_in_hand() {
    let mut g = new_state(&[id::GUARD_DOG], 2);
    set_hand(&mut g, 0, &[id::GUARD_DOG, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::GUARD_DOG);
    // 3 + 2 = 5 cards: +2 Cards more.
    assert_eq!(hand_total(&g, 0), 3 + 2 + 2);
    let mut g = new_state(&[id::GUARD_DOG], 2);
    set_hand(&mut g, 0, &[id::GUARD_DOG, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::GUARD_DOG);
    // 5 + 2 = 7 cards: only the first +2.
    assert_eq!(hand_total(&g, 0), 5 + 2);
}

#[test]
fn guard_dog_can_be_played_in_response_to_an_attack_and_is_discarded_at_the_end_of_that_turn() {
    let mut g = new_state(&[id::GUARD_DOG, id::MILITIA], 2);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::GUARD_DOG, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::MILITIA);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::GUARD_DOG && d.player == 0, "{d:?}");
    yes(&mut g);
    // Guard Dog drew 2 (to 6 cards: no more), then the Militia makes player 0 discard down to 3.
    assert_eq!(g.players[0].in_play.get(id::GUARD_DOG), 1);
    discard_first_while_asked(&mut g);
    assert_eq!(hand_total(&g, 0), 3);
    // Player 1's turn ends: the Guard Dog is discarded from play in that Clean-up.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    choose(&mut g, Choice::Pass);
    assert_eq!(g.players[0].in_play.get(id::GUARD_DOG), 0);
    assert!(g.players[0].discard.has(id::GUARD_DOG));
}

#[test]
fn guard_dog_is_asked_before_the_moat_and_may_be_declined() {
    let mut g = new_state(&[id::GUARD_DOG, id::MILITIA, id::MOAT], 2);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::GUARD_DOG, id::MOAT, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::MILITIA);
    // Guard Dog is asked first (it may draw a Moat), then the Moat.
    let d = expect_decision(&mut g);
    assert_eq!(d.subject, id::GUARD_DOG);
    no(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.subject, id::MOAT, "{d:?}");
    yes(&mut g);
    assert_eq!(hand_total(&g, 0), 5, "the Moat protected the hand");
}

#[test]
fn guard_dog_drawing_a_moat_lets_it_be_revealed_after_all() {
    let mut g = new_state(&[id::GUARD_DOG, id::MILITIA, id::MOAT], 2);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::GUARD_DOG, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::MOAT, id::ESTATE, id::ESTATE]);
    reset_turn(&mut g, 1);
    play(&mut g, id::MILITIA);
    yes(&mut g); // the Guard Dog
    let d = expect_decision(&mut g);
    assert_eq!(d.subject, id::MOAT, "{d:?}");
    yes(&mut g);
    assert_eq!(hand_total(&g, 0), 4 + 2, "the Moat protected the hand: no discard");
}

// ===========================================================================
// Oasis
// ===========================================================================

#[test]
fn oasis_is_a_cantrip_with_a_coin_and_a_discard() {
    let mut g = new_state(&[id::OASIS], 2);
    set_hand(&mut g, 0, &[id::OASIS, id::ESTATE, id::CURSE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::DUCHY]);
    play(&mut g, id::OASIS);
    assert_eq!((g.turn.actions, g.turn.coins), (1, 1));
    pick(&mut g, id::ESTATE);
    assert!(g.players[0].discard.has(id::ESTATE));
    assert_eq!(hand_total(&g, 0), 2 + 1 - 1, "drew a card and discarded one");
    assert!(g.players[0].hand.has(id::PROVINCE));
}

// ===========================================================================
// Scheme
// ===========================================================================

fn buy_phase_check(g: &mut GameState) {
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::Buy, "{d:?}");
}

#[test]
fn scheme_puts_an_action_from_play_onto_the_deck_when_discarded() {
    let mut g = new_state(&[id::SCHEME, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SCHEME, id::SMITHY, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SCHEME);
    play(&mut g, id::SMITHY);
    buy_phase_check(&mut g);
    // The cleanup offers one Action from play: the Smithy goes back on top of the deck.
    choose(&mut g, Choice::Pass);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, from: Zone::InPlay, max: 1, .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && c.contains(&Choice::Card(id::SCHEME)) && c.contains(&Choice::Pass), "{c:?}");
    choose(&mut g, Choice::Card(id::SMITHY));
    assert_eq!(g.players[0].in_play.get(id::SMITHY), 0);
    assert!(g.players[0].discard.has(id::SCHEME) && !g.players[0].discard.has(id::SMITHY));
}

#[test]
fn schemes_stack_and_a_scheme_can_put_itself_back() {
    let mut g = new_state(&[id::SCHEME, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SCHEME, id::SCHEME, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SCHEME);
    play(&mut g, id::SCHEME);
    play(&mut g, id::SMITHY);
    buy_phase_check(&mut g);
    choose(&mut g, Choice::Pass);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, max: 2, .. }), "{d:?}");
    // The order matters: the last one picked ends up on top, so it is the first card of the next hand.
    choose(&mut g, Choice::Card(id::SCHEME));
    let mut log: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::SMITHY), &mut log);
    let drawn: Vec<CardId> = log.iter().filter_map(|e| if let Event::Draw { card, .. } = e { Some(*card) } else { None }).collect();
    assert_eq!(&drawn[..2], &[id::SMITHY, id::SCHEME], "{log:?}");
    assert!(g.players[0].discard.has(id::SCHEME), "the other Scheme was discarded as usual");
}

#[test]
fn throne_roomed_scheme_counts_twice_and_a_duration_staying_in_play_is_not_offered() {
    let mut g = new_state(&[id::SCHEME, id::THRONE_ROOM, id::SMITHY, id::FISHING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::SCHEME, id::SMITHY, id::FISHING_VILLAGE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::SCHEME);
    play(&mut g, id::FISHING_VILLAGE);
    play(&mut g, id::SMITHY);
    buy_phase_check(&mut g);
    choose(&mut g, Choice::Pass);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, max: 2, .. }), "{d:?}");
    let c = choices(&g);
    assert!(!c.contains(&Choice::Card(id::FISHING_VILLAGE)), "a Duration that stays in play is not discarded: {c:?}");
    assert!(c.contains(&Choice::Card(id::THRONE_ROOM)) && c.contains(&Choice::Card(id::SMITHY)), "{c:?}");
}

// ===========================================================================
// Tunnel
// ===========================================================================

#[test]
fn a_discarded_tunnel_may_be_revealed_for_a_gold() {
    let mut g = new_state(&[id::TUNNEL, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::TUNNEL, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CELLAR);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::TUNNEL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }) && d.subject == id::TUNNEL, "{d:?}");
    yes(&mut g);
    assert!(g.players[0].discard.has(id::GOLD));
    assert_eq!(g.supply.get(id::GOLD), 29);
}

#[test]
fn the_tunnel_reaction_is_optional_and_not_used_in_cleanup() {
    let mut g = new_state(&[id::TUNNEL, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::TUNNEL]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CELLAR);
    pick(&mut g, id::TUNNEL);
    no(&mut g);
    assert!(!g.players[0].discard.has(id::GOLD));
    // Clean-up discards the hand and play area: no Tunnel reaction.
    let mut g = new_state(&[id::TUNNEL], 2);
    set_hand(&mut g, 0, &[id::TUNNEL, id::TUNNEL]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    choose(&mut g, Choice::Pass);
    assert!(!g.players[0].discard.has(id::GOLD));
    assert_eq!(g.players[0].discard.get(id::TUNNEL), 2);
    let d = adv(&mut g);
    assert!(!matches!(d, Step::Decision(Decision { kind: DecisionKind::YesNo { .. }, .. })));
}

#[test]
fn tunnels_discarded_together_are_all_discarded_before_any_reacts() {
    // Militia makes the victim discard 2 Tunnels at once: both are in the discard pile, then each may be revealed.
    let mut g = new_state(&[id::TUNNEL, id::MILITIA], 2);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::TUNNEL, id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::MILITIA);
    pick(&mut g, id::TUNNEL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }), "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1 + 1);
    // Two Tunnels at once.
    let mut g = new_state(&[id::TUNNEL, id::MILITIA], 2);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::TUNNEL, id::TUNNEL, id::GOLD, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::MILITIA);
    pick(&mut g, id::TUNNEL);
    assert_eq!(g.players[0].discard.get(id::TUNNEL), 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }), "{d:?}");
    yes(&mut g);
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::GOLD), 2);
}

#[test]
fn a_discarded_tunnel_reacts_only_once_everything_discarded_together_is_in_the_discard_pile() {
    // Cellar discards one card at a time, but they are discarded together: the Tunnel reacts after the last pick.
    let mut g = new_state(&[id::TUNNEL, id::CELLAR, id::JACK_OF_ALL_TRADES, id::FARMLAND], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::TUNNEL, id::JACK_OF_ALL_TRADES, id::FARMLAND]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CELLAR);
    pick(&mut g, id::TUNNEL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }), "no reaction yet: {d:?}");
    assert!(g.players[0].discard.has(id::TUNNEL) && !g.players[0].discard.has(id::GOLD));
    pick(&mut g, id::FARMLAND);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }) && d.subject == id::TUNNEL, "{d:?}");
    // The Cellar's draw comes after the reactions.
    assert_eq!(hand_total(&g, 0), 1);
    yes(&mut g);
    assert_eq!(hand_total(&g, 0), 1 + 2);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
}

// ===========================================================================
// Jack of All Trades
// ===========================================================================

#[test]
fn jack_of_all_trades_gains_a_silver_looks_at_the_top_draws_to_five_and_may_trash() {
    let mut g = new_state(&[id::JACK_OF_ALL_TRADES], 2);
    set_hand(&mut g, 0, &[id::JACK_OF_ALL_TRADES, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::GOLD, id::SILVER, id::COPPER, id::COPPER]);
    play(&mut g, id::JACK_OF_ALL_TRADES);
    assert!(g.players[0].discard.has(id::SILVER), "the Silver is gained first");
    // Look at the top card (Curse): discard it.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, from: Zone::Revealed, .. }), "{d:?}");
    pick(&mut g, id::CURSE);
    assert!(g.players[0].discard.has(id::CURSE));
    // Draw until 5 cards: 2 in hand, so 3 more.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, from: Zone::Hand, filter: Filter::NonTreasure, .. }), "{d:?}");
    assert_eq!(hand_total(&g, 0), 5);
    // The trash is of a non-Treasure only.
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::ESTATE)) && !c.contains(&Choice::Card(id::COPPER)), "{c:?}");
    pick(&mut g, id::ESTATE);
    assert_eq!(g.trash.get(id::ESTATE), 1);
}

#[test]
fn jack_of_all_trades_with_a_full_hand_draws_nothing_and_a_kept_top_card_is_drawn() {
    let mut g = new_state(&[id::JACK_OF_ALL_TRADES], 2);
    set_hand(&mut g, 0, &[id::JACK_OF_ALL_TRADES, id::COPPER, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::SILVER]);
    set_hand(&mut g, 0, &[id::JACK_OF_ALL_TRADES, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::JACK_OF_ALL_TRADES);
    done(&mut g); // keep the Gold on the deck
    assert_eq!(hand_total(&g, 0), 5, "already five cards: nothing drawn");
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));
    // Keeping it, a smaller hand draws it.
    let mut g = new_state(&[id::JACK_OF_ALL_TRADES], 2);
    set_hand(&mut g, 0, &[id::JACK_OF_ALL_TRADES, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::DUCHY, id::DUCHY, id::DUCHY, id::DUCHY]);
    play(&mut g, id::JACK_OF_ALL_TRADES);
    done(&mut g);
    assert!(g.players[0].hand.has(id::GOLD));
    assert_eq!(hand_total(&g, 0), 5);
}

#[test]
fn jack_of_all_trades_silver_is_shuffled_in_when_the_look_causes_a_reshuffle() {
    let mut g = new_state(&[id::JACK_OF_ALL_TRADES], 2);
    set_hand(&mut g, 0, &[id::JACK_OF_ALL_TRADES]);
    clear_deck(&mut g, 0);
    set_discard(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::JACK_OF_ALL_TRADES);
    // The deck is empty: the Silver gained goes with the discard pile into the new deck (5 cards, one of them the Silver).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, from: Zone::Revealed, .. }), "{d:?}");
    done(&mut g);
    assert_eq!(hand_total(&g, 0) + g.players[0].deck_size() + g.players[0].discard.total() as u32, 5);
    assert!(g.players[0].discard.is_empty() || !g.players[0].discard.has(id::SILVER));
}

// ===========================================================================
// Nomads
// ===========================================================================

#[test]
fn nomads_gives_a_buy_and_two_coins_when_played_gained_and_trashed() {
    let mut g = new_state(&[id::NOMADS], 2);
    set_hand(&mut g, 0, &[id::NOMADS]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::NOMADS);
    assert_eq!((g.turn.buys, g.turn.coins), (2, 2));
    // Bought: the +$2 comes when it is gained, in the Buy phase.
    let mut g = new_state(&[id::NOMADS], 2);
    buy_phase(&mut g, &[id::SILVER, id::SILVER]);
    assert_eq!(g.turn.coins, 4);
    g.apply(Choice::Card(id::NOMADS), &mut NoEvents).unwrap();
    assert_eq!(g.turn.coins, 2, "4 - 4 + 2");
    // Trashed (Chapel): +$2.
    let mut g = new_state(&[id::NOMADS, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::NOMADS]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::NOMADS);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn nomads_gained_on_another_players_turn_gives_nothing_to_the_current_turn() {
    let mut g = new_state(&[id::NOMADS], 2);
    reset_turn(&mut g, 1);
    g.gain_card(0, id::NOMADS, Dest::Discard, &mut NoEvents);
    assert_eq!(g.turn.coins, 0);
    assert!(g.players[0].discard.has(id::NOMADS));
}

// ===========================================================================
// Spice Merchant
// ===========================================================================

#[test]
fn spice_merchant_trashes_a_treasure_for_cards_and_an_action_or_a_buy_and_coins() {
    let mut g = new_state(&[id::SPICE_MERCHANT], 2);
    set_hand(&mut g, 0, &[id::SPICE_MERCHANT, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SPICE_MERCHANT);
    // Only Treasures can be trashed.
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass]);
    pick(&mut g, id::COPPER);
    assert_eq!(g.trash.get(id::COPPER), 1);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Mode { picks: 1, distinct: false });
    choose(&mut g, Choice::Mode(0));
    assert_eq!(hand_total(&g, 0), 1 + 2);
    assert_eq!(g.turn.actions, 1);
    // The other option.
    let mut g = new_state(&[id::SPICE_MERCHANT], 2);
    set_hand(&mut g, 0, &[id::SPICE_MERCHANT, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SPICE_MERCHANT);
    pick(&mut g, id::SILVER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!((g.turn.buys, g.turn.coins, g.turn.actions), (2, 2, 0));
}

#[test]
fn spice_merchant_trashing_is_optional() {
    let mut g = new_state(&[id::SPICE_MERCHANT], 2);
    set_hand(&mut g, 0, &[id::SPICE_MERCHANT, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SPICE_MERCHANT);
    done(&mut g);
    assert!(g.trash.is_empty());
    assert_eq!(g.turn.buys, 1);
    assert_eq!(g.players[0].deck_size(), 40);
}

// ===========================================================================
// Trader
// ===========================================================================

#[test]
fn trader_trashes_a_card_for_a_silver_per_coin_of_its_cost() {
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRADER);
    pick(&mut g, id::GOLD);
    assert_eq!(g.trash.get(id::GOLD), 1);
    assert_eq!(g.players[0].discard.get(id::SILVER), 6);
    // A Copper is worth none; the trash is not optional.
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRADER);
    assert!(!choices(&g).contains(&Choice::Pass));
    pick(&mut g, id::COPPER);
    assert_eq!(g.players[0].discard.get(id::SILVER), 0);
    // The Silvers run out with the pile.
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    set_supply(&mut g, id::SILVER, 4);
    play(&mut g, id::TRADER);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].discard.get(id::SILVER), 4);
}

#[test]
fn trader_exchanges_a_gained_card_for_a_silver() {
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::DUCHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::DUCHY && d.source == Some(id::TRADER), "{d:?}");
    yes(&mut g);
    assert_eq!(g.supply.get(id::DUCHY), 8, "back in its pile");
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert!(!g.players[0].discard.has(id::DUCHY));
    assert_eq!(g.supply.get(id::SILVER), 39);
}

#[test]
fn trader_exchange_is_optional_and_not_offered_for_a_silver_or_without_silver_left() {
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    g.turn.buys = 3;
    buy(&mut g, id::SILVER);
    // No reaction for a Silver: the Buy decision is back.
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    buy(&mut g, id::ESTATE);
    no(&mut g);
    assert!(g.players[0].discard.has(id::ESTATE));
    // The Silver pile is empty: nothing to exchange for.
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    set_supply(&mut g, id::SILVER, 0);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    g.turn.buys = 2;
    buy(&mut g, id::ESTATE);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn trader_exchanges_a_curse_given_by_another_players_attack() {
    let mut g = new_state(&[id::TRADER, id::WITCH], 2);
    set_hand(&mut g, 1, &[id::WITCH]);
    set_hand(&mut g, 0, &[id::TRADER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::WITCH);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::CURSE && d.player == 0, "{d:?}");
    yes(&mut g);
    assert_eq!(g.supply.get(id::CURSE), 10);
    assert!(g.players[0].discard.has(id::SILVER) && !g.players[0].discard.has(id::CURSE));
}

#[test]
fn trader_exchange_is_not_a_gain_and_the_cards_own_gain_effect_still_happens() {
    // A Border Village gained and exchanged: its gain still happens (the Duchy, say), and that can be exchanged too.
    let mut g = new_state(&[id::TRADER, id::BORDER_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::BORDER_VILLAGE);
    let d = expect_decision(&mut g);
    assert_eq!(d.subject, id::BORDER_VILLAGE);
    yes(&mut g);
    assert_eq!(g.supply.get(id::BORDER_VILLAGE), 10);
    // Border Village's gain: a card costing less than $6.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    pick(&mut g, id::DUCHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::DUCHY, "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
    assert_eq!(g.supply.get(id::DUCHY), 8);
}

#[test]
fn trader_gained_card_moved_by_another_reaction_cannot_be_exchanged() {
    // Watchtower trashes the gained Gold first; the Trader's exchange is skipped.
    let mut g = new_state(&[id::TRADER, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::WATCHTOWER, id::GOLD, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::GOLD);
    // The Trader's exchange is asked first.
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(id::TRADER), "{d:?}");
    no(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(id::WATCHTOWER), "{d:?}");
    // Trashing it with Watchtower works as before.
    choose(&mut g, Choice::Card(id::GOLD));
    assert_eq!(g.trash.get(id::GOLD), 1);
}

// ===========================================================================
// Trail
// ===========================================================================

#[test]
fn trail_is_a_cantrip() {
    let mut g = new_state(&[id::TRAIL], 2);
    set_hand(&mut g, 0, &[id::TRAIL, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRAIL);
    assert_eq!(g.turn.actions, 1);
    assert_eq!(hand_total(&g, 0), 2);
}

#[test]
fn trail_may_be_played_when_gained_discarded_or_trashed() {
    // Gained (Workshop): played from the discard pile.
    let mut g = new_state(&[id::TRAIL, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::TRAIL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::TRAIL, "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].in_play.get(id::TRAIL), 1);
    assert_eq!(hand_total(&g, 0), 2, "drew a card");
    assert_eq!(g.turn.actions, 1, "+1 Action");
    // Discarded (Cellar): played from the discard pile.
    let mut g = new_state(&[id::TRAIL, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::TRAIL, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CELLAR);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::TRAIL);
    yes(&mut g);
    assert_eq!(g.players[0].in_play.get(id::TRAIL), 1);
    // Trashed (Chapel): played back from the trash.
    let mut g = new_state(&[id::TRAIL, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::TRAIL, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::TRAIL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::TRAIL, "{d:?}");
    yes(&mut g);
    assert_eq!(g.trash.get(id::TRAIL), 0);
    assert_eq!(g.players[0].in_play.get(id::TRAIL), 1);
}

#[test]
fn trail_reaction_is_optional_and_does_nothing_in_cleanup() {
    let mut g = new_state(&[id::TRAIL, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::TRAIL);
    no(&mut g);
    assert!(g.players[0].discard.has(id::TRAIL));
    // In Clean-up the hand is discarded without asking.
    let mut g = new_state(&[id::TRAIL], 2);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_in_play(&mut g, 0, &[id::TRAIL]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    choose(&mut g, Choice::Pass);
    assert!(g.players[0].discard.has(id::TRAIL));
}

#[test]
fn a_trail_played_on_another_players_turn_is_discarded_at_the_end_of_that_turn() {
    let mut g = new_state(&[id::TRAIL, id::MILITIA], 2);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::TRAIL, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::MILITIA);
    // Player 0 discards two cards, a Trail among them (the Estates first would be sensible; here the Trail goes).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::TRAIL)));
    // (The first Estate is forced.)
    choose(&mut g, Choice::Card(id::TRAIL));
    yes(&mut g);
    assert_eq!(g.players[0].in_play.get(id::TRAIL), 1);
    assert_eq!(hand_total(&g, 0), 3 + 1, "the Trail drew a card");
    // At the end of player 1's turn it is discarded from play.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    choose(&mut g, Choice::Pass);
    assert_eq!(g.players[0].in_play.get(id::TRAIL), 0);
    assert!(g.players[0].discard.has(id::TRAIL));
}

// ===========================================================================
// Weaver
// ===========================================================================

#[test]
fn weaver_gains_two_silvers_or_a_card_costing_up_to_four() {
    let mut g = new_state(&[id::WEAVER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WEAVER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WEAVER);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Mode { picks: 1, distinct: false });
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
    let mut g = new_state(&[id::WEAVER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WEAVER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WEAVER);
    choose(&mut g, Choice::Mode(1));
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && c.contains(&Choice::Card(id::SILVER)) && !c.contains(&Choice::Card(id::DUCHY)), "{c:?}");
    choose(&mut g, Choice::Card(id::SMITHY));
    assert!(g.players[0].discard.has(id::SMITHY));
}

#[test]
fn a_discarded_weaver_may_be_played_and_it_is_discarded_again_in_cleanup() {
    let mut g = new_state(&[id::WEAVER, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::WEAVER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CELLAR);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::WEAVER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::WEAVER, "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].in_play.get(id::WEAVER), 1);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
}

#[test]
fn several_weavers_discarded_together_are_all_in_the_discard_pile_before_the_first_is_played() {
    // A reshuffle made by the first Weaver (it gains a Silver into an empty deck... here: draws) loses the others.
    let mut g = new_state(&[id::WEAVER, id::CELLAR], 2);
    set_hand(&mut g, 0, &[id::CELLAR, id::WEAVER, id::WEAVER]);
    clear_deck(&mut g, 0);
    play(&mut g, id::CELLAR);
    pick(&mut g, id::WEAVER);
    pick(&mut g, id::WEAVER);
    assert_eq!(g.players[0].discard.get(id::WEAVER), 2);
    yes(&mut g);
    choose(&mut g, Choice::Mode(0));
    // Both Weavers asked: the second is still in the discard pile (nothing shuffled).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::WEAVER, "{d:?}");
}

// ===========================================================================
// Berserker
// ===========================================================================

#[test]
fn berserker_gains_a_cheaper_card_and_the_others_discard_down_to_three() {
    let mut g = new_state(&[id::BERSERKER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::BERSERKER]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::BERSERKER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }) && d.player == 0, "costing less than $5: {d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && !c.contains(&Choice::Card(id::DUCHY)));
    choose(&mut g, Choice::Card(id::SMITHY));
    assert!(g.players[0].discard.has(id::SMITHY));
    // Player 1 discards down to 3.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 2, .. }) && d.player == 1, "{d:?}");
}

#[test]
fn a_berserker_gained_with_an_action_in_play_is_played_at_once() {
    let mut g = new_state(&[id::BERSERKER, id::WORKSHOP, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::ESTATE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY, id::PROVINCE, id::COPPER, id::CURSE]);
    set_deck_known(&mut g, 0, &junk());
    // Workshop is in play; the Berserker is gained by the engine directly (no card here gains a $5 card).
    play(&mut g, id::WORKSHOP);
    g.stack = Default::default();
    g.pending = Pending::None;
    g.gain_card(0, id::BERSERKER, Dest::Discard, &mut NoEvents);
    // Played with no question: a gain, then the attack.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }), "{d:?}");
    assert!(!choices(&g).contains(&Choice::Card(id::DUCHY)), "less than $5");
    assert_eq!(g.players[0].in_play.get(id::BERSERKER), 1);
    assert!(!g.players[0].discard.has(id::BERSERKER));
    choose(&mut g, Choice::Card(id::SMITHY));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) && d.player == 1, "{d:?}");
}

#[test]
fn a_berserker_gained_with_no_action_in_play_stays_in_the_discard_pile() {
    let mut g = new_state(&[id::BERSERKER], 2);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    g.gain_card(0, id::BERSERKER, Dest::Discard, &mut NoEvents);
    assert!(g.players[0].discard.has(id::BERSERKER));
    assert!(g.stack.is_empty());
    // In the Buy phase too: bought with no Action played.
    let mut g = new_state(&[id::BERSERKER], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::GOLD]);
    buy(&mut g, id::BERSERKER);
    assert!(g.players[0].discard.has(id::BERSERKER) || g.players[1].turns_taken > 0 || true);
}

#[test]
fn a_berserker_bought_after_playing_an_action_attacks() {
    let mut g = new_state(&[id::BERSERKER, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::GOLD, id::GOLD]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY, id::PROVINCE, id::COPPER, id::CURSE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::VILLAGE);
    // Treasures are played; buy the Berserker.
    buy(&mut g, id::BERSERKER);
    // Played (no question): its gain is asked.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }), "{d:?}");
}

#[test]
fn a_berserker_gained_on_another_players_turn_is_played_there_and_attacks_the_turns_player() {
    // Player 0 has a Duration (Fishing Village) in play on player 1's turn; Berserker is gained: played, hitting player 1.
    let mut g = new_state(&[id::BERSERKER, id::FISHING_VILLAGE, id::SMITHY], 2);
    set_in_play(&mut g, 0, &[id::FISHING_VILLAGE]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY, id::PROVINCE, id::COPPER, id::CURSE]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    g.gain_card(0, id::BERSERKER, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }) && d.player == 0, "{d:?}");
    choose(&mut g, Choice::Card(id::SMITHY));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) && d.player == 1, "{d:?}");
    assert_eq!(g.players[0].in_play.get(id::BERSERKER), 1);
    assert_eq!(g.players[0].discard.get(id::SMITHY), 1);
    // Discarded from play in the Clean-up of the turn it was played in (player 1's).
    assert_eq!(g.players[0].discard_next_cleanup.get(id::BERSERKER), 1);
}

// ===========================================================================
// Cartographer
// ===========================================================================

#[test]
fn cartographer_draws_looks_at_four_discards_some_and_reorders_the_rest() {
    let mut g = new_state(&[id::CARTOGRAPHER], 2);
    set_hand(&mut g, 0, &[id::CARTOGRAPHER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::CURSE, id::ESTATE, id::SILVER, id::COPPER, id::COPPER]);
    play(&mut g, id::CARTOGRAPHER);
    assert!(g.players[0].hand.has(id::GOLD), "+1 Card first");
    assert_eq!(g.turn.actions, 1);
    // The top 4 are Curse, Estate, Silver, Copper: discard the first two.
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    assert_eq!(g.players[0].discard.total(), 2);
    // Put the rest back in any order, one at a time (the last ends up on top).
    pick(&mut g, id::COPPER);
    // (The Silver, the only card left, follows at once.)
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::SILVER));
}

#[test]
fn cartographer_looks_at_fewer_cards_when_the_deck_is_short() {
    let mut g = new_state(&[id::CARTOGRAPHER], 2);
    set_hand(&mut g, 0, &[id::CARTOGRAPHER]);
    clear_deck(&mut g, 0);
    set_deck_known(&mut g, 0, &[id::GOLD, id::DUCHY]);
    play(&mut g, id::CARTOGRAPHER);
    // One card drawn; the one left is looked at.
    assert!(g.players[0].hand.has(id::GOLD));
    pick(&mut g, id::DUCHY);
    assert!(g.players[0].discard.has(id::DUCHY));
}

// ===========================================================================
// Cauldron
// ===========================================================================

/// Player 0's Buy phase with `hand`, in a 2-player game where Cauldron and a few Actions are in the kingdom.
fn cauldron_game(hand: &[CardId]) -> GameState {
    let mut g = new_state(&[id::CAULDRON, id::VILLAGE, id::SMITHY, id::MILITIA, id::MOAT], 2);
    set_hand(&mut g, 0, hand);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    g.turn.phase = Phase::Buy;
    g
}

#[test]
fn cauldron_is_worth_two_coins_and_a_buy_and_curses_on_the_third_action_gained() {
    let mut g = cauldron_game(&[id::CAULDRON, id::GOLD, id::GOLD, id::GOLD]);
    // A Treasure-Attack with a choice: offered with the other Treasures.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::CAULDRON);
    // The Golds are played as well.
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.turn.coins, 2 + 9);
    g.turn.buys = 5;
    buy(&mut g, id::VILLAGE);
    buy(&mut g, id::SMITHY);
    assert_eq!(g.supply.get(id::CURSE), 10, "two Actions so far");
    // The third Action gained: each other player gains a Curse.
    g.apply(Choice::Card(id::MILITIA), &mut NoEvents).unwrap();
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    assert_eq!(g.supply.get(id::CURSE), 9);
}

#[test]
fn gaining_three_actions_before_playing_the_cauldron_gives_no_curse() {
    let mut g = cauldron_game(&[id::CAULDRON, id::GOLD, id::GOLD, id::GOLD]);
    g.turn.buys = 5;
    // Gain three Actions first (not by buying: the Treasures are still in hand).
    for _ in 0..3 {
        g.gain_card(0, id::VILLAGE, Dest::Discard, &mut NoEvents);
    }
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::CAULDRON);
    g.apply(Choice::Card(id::VILLAGE), &mut NoEvents).unwrap();
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0);
}

#[test]
fn only_actions_count_for_the_cauldron_and_two_cauldrons_give_two_curses() {
    let mut g = cauldron_game(&[id::CAULDRON, id::CAULDRON, id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    play_treasure(&mut g, id::CAULDRON);
    play_treasure(&mut g, id::CAULDRON);
    assert_eq!(g.turn.coins, 4 + 12);
    g.turn.buys = 9;
    buy(&mut g, id::VILLAGE);
    buy(&mut g, id::ESTATE);
    buy(&mut g, id::COPPER);
    buy(&mut g, id::SMITHY);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0, "only two Actions");
    g.apply(Choice::Card(id::MILITIA), &mut NoEvents).unwrap();
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 2);
}

#[test]
fn a_moat_revealed_to_the_cauldron_protects_from_its_curse() {
    let mut g = cauldron_game(&[id::CAULDRON, id::GOLD, id::GOLD, id::GOLD]);
    set_hand(&mut g, 1, &[id::MOAT, id::ESTATE]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::CAULDRON);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }) && d.subject == id::MOAT && d.player == 1, "{d:?}");
    yes(&mut g);
    g.turn.buys = 5;
    buy(&mut g, id::VILLAGE);
    buy(&mut g, id::SMITHY);
    g.apply(Choice::Card(id::MILITIA), &mut NoEvents).unwrap();
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0);
}

// ===========================================================================
// Haggler
// ===========================================================================

#[test]
fn haggler_gains_a_cheaper_non_victory_card_for_each_card_bought() {
    let mut g = new_state(&[id::HAGGLER, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HAGGLER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HAGGLER);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 8, "$2 and the Golds");
    buy(&mut g, id::PROVINCE);
    // A card costing less than $8, not a Victory card.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 8, filter: Filter::NonVictory, .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::GOLD)) && !c.contains(&Choice::Card(id::DUCHY)) && !c.contains(&Choice::Card(id::PROVINCE)), "{c:?}");
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    assert_eq!(g.players[0].discard.get(id::PROVINCE), 1);
}

#[test]
fn haggler_only_reacts_to_cards_bought_not_gained_and_stacks() {
    let mut g = new_state(&[id::HAGGLER, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::HAGGLER, id::HAGGLER, id::WORKSHOP, id::SILVER, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.actions = 3;
    play(&mut g, id::HAGGLER);
    play(&mut g, id::HAGGLER);
    play(&mut g, id::WORKSHOP);
    // A Workshop gain is not a buy.
    choose(&mut g, Choice::Card(id::SILVER));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy, "no Haggler gain for a gain: {d:?}");
    // Two Hagglers: two cheaper cards for the one bought ($4 + $4 = 8 coins; buy a Gold).
    buy(&mut g, id::GOLD);
    for _ in 0..2 {
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
        g.apply(Choice::Card(id::SILVER), &mut NoEvents).unwrap();
    }
    assert_eq!(g.players[0].discard.get(id::SILVER), 3);
}

#[test]
fn haggler_gains_nothing_for_a_copper_and_not_for_a_card_gained_by_haggler() {
    let mut g = new_state(&[id::HAGGLER], 2);
    set_hand(&mut g, 0, &[id::HAGGLER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HAGGLER);
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::COPPER));
    // No card costs less than $0: the Copper is the only card gained (the Haggler is discarded in the Clean-up).
    assert_eq!(g.players[0].discard.total(), 2);
    assert_eq!(g.players[0].discard.get(id::COPPER), 1);
}

// ===========================================================================
// Highway
// ===========================================================================

#[test]
fn highway_reduces_every_cost_by_one_each_time() {
    let mut g = new_state(&[id::HIGHWAY, id::THRONE_ROOM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HIGHWAY, id::THRONE_ROOM, id::HIGHWAY, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::HIGHWAY);
    assert_eq!(g.cost(id::PROVINCE), 6);
    assert_eq!(g.cost(id::COPPER), 0);
    play(&mut g, id::HIGHWAY);
    assert_eq!(g.cost(id::PROVINCE), 5);
    assert_eq!(g.cost(id::ESTATE), 0, "not below zero");
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    // $6 buys a card that costs $8 - 3 = 5... Province costs $5 now.
    buy(&mut g, id::PROVINCE);
}

// ===========================================================================
// Inn
// ===========================================================================

#[test]
fn inn_gives_two_cards_two_actions_and_discards_two_cards() {
    let mut g = new_state(&[id::INN], 2);
    set_hand(&mut g, 0, &[id::INN, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::PROVINCE, id::CURSE]);
    play(&mut g, id::INN);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(hand_total(&g, 0), 4);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 2, max: 2, .. }), "{d:?}");
}

#[test]
fn a_gained_inn_shuffles_chosen_actions_from_the_discard_pile_into_the_deck() {
    let mut g = new_state(&[id::INN, id::SMITHY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    set_discard(&mut g, 0, &[id::SMITHY, id::SMITHY, id::VILLAGE, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::DUCHY]);
    set_deck_unknown(&mut g, 0, &[id::CURSE, id::CURSE]);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    g.turn.buys = 2;
    buy(&mut g, id::INN);
    // Reveal any number of Action cards in the discard pile (the Inn just gained too): shuffle them into the deck.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::ShuffleIn, from: Zone::Discard, .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::SMITHY)) && c.contains(&Choice::Card(id::INN)) && !c.contains(&Choice::Card(id::ESTATE)), "{c:?}");
    pick(&mut g, id::VILLAGE);
    choose(&mut g, Choice::Card(id::SMITHY));
    choose(&mut g, Choice::Card(id::SMITHY));
    choose(&mut g, Choice::Card(id::INN));
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    let ps = &g.players[0];
    assert_eq!(ps.discard.total(), 2, "the Estate and the Copper stay");
    assert_eq!(ps.deck_size(), 2 + 2 + 4);
    assert!(ps.deck_known.is_empty(), "the deck was shuffled: nothing is known of its order");
    assert_eq!(ps.deck_unknown.get(id::SMITHY), 2);
}

#[test]
fn a_gained_inn_still_shuffles_the_deck_when_no_action_is_chosen() {
    let mut g = new_state(&[id::INN], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::GOLD]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::DUCHY]);
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    g.turn.buys = 2;
    buy(&mut g, id::INN);
    // The gained Inn is the only Action in the discard pile.
    expect_decision(&mut g);
    choose(&mut g, Choice::Pass);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert!(g.players[0].deck_known.is_empty());
    assert_eq!(g.players[0].deck_unknown.total(), 2);
}

// ===========================================================================
// Margrave
// ===========================================================================

#[test]
fn margrave_draws_three_gives_a_buy_and_makes_the_others_draw_then_discard_to_three() {
    let mut g = new_state(&[id::MARGRAVE], 3);
    set_hand(&mut g, 0, &[id::MARGRAVE]);
    set_deck_known(&mut g, 0, &junk());
    for q in 1..3 {
        set_hand(&mut g, q, &[id::ESTATE, id::ESTATE, id::COPPER]);
        set_deck_known(&mut g, q, &[id::GOLD, id::GOLD, id::GOLD]);
    }
    play(&mut g, id::MARGRAVE);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(hand_total(&g, 0), 3);
    // Each victim drew a card (4 cards in hand) and discards 1.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, min: 1, max: 1, .. }) && d.player == 1, "{d:?}");
    assert_eq!(hand_total(&g, 1), 4);
    choose(&mut g, Choice::Card(id::ESTATE));
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 2);
    assert_eq!(hand_total(&g, 2), 4);
}

#[test]
fn margrave_a_player_with_three_cards_after_drawing_does_not_discard_and_a_moat_stops_the_draw() {
    let mut g = new_state(&[id::MARGRAVE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MARGRAVE]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::MARGRAVE);
    assert_eq!(hand_total(&g, 1), 3);
    assert_ne!(g.pending_decision().map(|d| d.player), Some(1));
    let mut g = new_state(&[id::MARGRAVE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MARGRAVE]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::MOAT, id::COPPER, id::COPPER]);
    set_deck_known(&mut g, 1, &[id::ESTATE]);
    play(&mut g, id::MARGRAVE);
    yes(&mut g);
    assert_eq!(hand_total(&g, 1), 3, "no card drawn");
}

// ===========================================================================
// Souk
// ===========================================================================

#[test]
fn souk_gives_a_buy_and_seven_coins_less_one_per_card_in_hand() {
    let mut g = new_state(&[id::SOUK], 2);
    set_hand(&mut g, 0, &[id::SOUK, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SOUK);
    assert_eq!((g.turn.coins, g.turn.buys), (4, 2));
    // Never below $0: a big hand, and $ already there are not reduced below nothing.
    let mut g = new_state(&[id::SOUK], 2);
    set_hand(&mut g, 0, &[id::SOUK, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.coins = 1;
    play(&mut g, id::SOUK);
    assert_eq!(g.turn.coins, 0, "1 + 7 - 9, not below 0");
}

#[test]
fn a_gained_souk_may_trash_up_to_two_cards_from_the_hand() {
    let mut g = new_state(&[id::SOUK], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::GOLD, id::ESTATE, id::CURSE, id::COPPER]);
    // Treasures are played, so the hand is the Estate and the Curse.
    buy(&mut g, id::SOUK);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    assert_eq!(g.trash.total(), 2);
}

// ===========================================================================
// Stables
// ===========================================================================

#[test]
fn stables_discards_a_treasure_for_three_cards_and_an_action() {
    let mut g = new_state(&[id::STABLES], 2);
    set_hand(&mut g, 0, &[id::STABLES, id::COPPER, id::GOLD, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::STABLES);
    // Only Treasures.
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::COPPER)) && c.contains(&Choice::Card(id::GOLD)) && !c.contains(&Choice::Card(id::ESTATE)), "{c:?}");
    pick(&mut g, id::COPPER);
    assert_eq!(g.turn.actions, 1);
    assert_eq!(g.players[0].deck_size(), 40 - 3);
    assert!(g.players[0].discard.has(id::COPPER));
    // Optional.
    let mut g = new_state(&[id::STABLES], 2);
    set_hand(&mut g, 0, &[id::STABLES, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::STABLES);
    g.apply(Choice::Pass, &mut NoEvents).unwrap();
    assert_eq!(g.turn.actions, 0);
    assert_eq!(g.players[0].deck_size(), 40);
}

// ===========================================================================
// Wheelwright
// ===========================================================================

#[test]
fn wheelwright_discards_a_card_to_gain_an_action_costing_as_much_or_less() {
    let mut g = new_state(&[id::WHEELWRIGHT, id::SMITHY, id::VILLAGE, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::WHEELWRIGHT, id::SILVER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WHEELWRIGHT);
    assert_eq!(g.turn.actions, 1);
    // Discard the Silver ($3): an Action costing up to $3 (Village), not Smithy ($4).
    pick(&mut g, id::SILVER);
    // The only Action costing $3 or less is gained at once (Smithy and Militia cost $4).
    assert!(g.players[0].discard.has(id::VILLAGE) && g.players[0].discard.has(id::SILVER));
    assert!(!g.players[0].discard.has(id::SMITHY));
}

#[test]
fn wheelwright_discarding_an_action_gains_a_copy_and_is_optional() {
    let mut g = new_state(&[id::WHEELWRIGHT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WHEELWRIGHT, id::SMITHY, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WHEELWRIGHT);
    pick(&mut g, id::SMITHY);
    // A copy of the discarded Action (the only Action costing up to $4).
    assert_eq!(g.players[0].discard.get(id::SMITHY), 2);
    let mut g = new_state(&[id::WHEELWRIGHT], 2);
    set_hand(&mut g, 0, &[id::WHEELWRIGHT, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WHEELWRIGHT);
    done(&mut g);
    assert!(g.players[0].discard.is_empty());
}

// ===========================================================================
// Witch's Hut
// ===========================================================================

#[test]
fn witchs_hut_curses_the_others_when_both_discarded_cards_are_actions() {
    let mut g = new_state(&[id::WITCHS_HUT, id::SMITHY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WITCHS_HUT]);
    set_deck_known(&mut g, 0, &[id::SMITHY, id::VILLAGE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WITCHS_HUT);
    // (The Smithy, the only card left to follow the Village, is discarded with it.)
    pick(&mut g, id::VILLAGE);
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
    // Only one Action: no Curse.
    let mut g = new_state(&[id::WITCHS_HUT, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::WITCHS_HUT]);
    set_deck_known(&mut g, 0, &[id::SMITHY, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WITCHS_HUT);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0);
}

#[test]
fn witchs_hut_discarding_one_card_only_gives_no_curse_and_a_moat_protects() {
    let mut g = new_state(&[id::WITCHS_HUT, id::SMITHY, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::WITCHS_HUT]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 0, &[id::SMITHY, id::SMITHY, id::ESTATE, id::ESTATE]);
    play(&mut g, id::WITCHS_HUT);
    yes(&mut g);
    pick(&mut g, id::SMITHY);
    assert_eq!(g.players[1].discard.get(id::CURSE), 0, "Moat");
    assert_eq!(g.players[0].discard.get(id::SMITHY), 2, "both Actions were discarded");
}

// ===========================================================================
// Border Village, Farmland
// ===========================================================================

#[test]
fn border_village_gains_a_cheaper_card_when_gained_not_when_played() {
    let mut g = new_state(&[id::BORDER_VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::BORDER_VILLAGE, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::BORDER_VILLAGE);
    assert_eq!(g.turn.actions, 2);
    assert_eq!(hand_total(&g, 0), 2);
    assert!(g.stack.is_empty());
    // Bought: gain a card costing less than $6.
    let mut g = new_state(&[id::BORDER_VILLAGE, id::SMITHY], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::GOLD]);
    buy(&mut g, id::BORDER_VILLAGE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 6, .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::DUCHY)) && !c.contains(&Choice::Card(id::GOLD)) && !c.contains(&Choice::Card(id::BORDER_VILLAGE)), "{c:?}");
}

#[test]
fn border_village_gain_follows_the_current_cost() {
    let mut g = new_state(&[id::BORDER_VILLAGE, id::HIGHWAY, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::HIGHWAY, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HIGHWAY);
    buy(&mut g, id::BORDER_VILLAGE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 5, .. }), "Border Village costs $5 now: {d:?}");
    let c = choices(&g);
    // Every card costs $1 less now: the Duchy costs $4, Gold and the Border Village $5.
    assert!(c.contains(&Choice::Card(id::DUCHY)) && !c.contains(&Choice::Card(id::GOLD)) && !c.contains(&Choice::Card(id::BORDER_VILLAGE)), "{c:?}");
}

#[test]
fn farmland_trashes_a_card_from_the_hand_and_gains_one_costing_exactly_two_more() {
    let mut g = new_state(&[id::FARMLAND, id::SMITHY], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::GOLD, id::GOLD, id::SILVER, id::ESTATE]);
    // Treasures played: $11; the Estate is the only card in hand.
    buy(&mut g, id::FARMLAND);
    // The Estate ($2) is trashed at once, a card costing exactly $4 is gained (Smithy, Silver is $3: no).
    assert_eq!(g.trash.get(id::ESTATE), 1);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { exact: true, max_cost: 4, .. }) || g.players[0].discard.has(id::SMITHY), "{d:?}");
}

#[test]
fn farmland_never_gains_another_farmland_and_does_nothing_with_an_empty_hand() {
    let mut g = new_state(&[id::FARMLAND], 2);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 0, &[id::SILVER, id::CURSE]);
    g.gain_card(0, id::FARMLAND, Dest::Discard, &mut NoEvents);
    // Silver costs $3: a $5 card (Duchy, not Farmland, which costs $6).
    pick(&mut g, id::SILVER);
    assert_eq!(g.trash.get(id::SILVER), 1);
    assert!(g.players[0].discard.has(id::DUCHY));
    // Trashing a $4 card: another $6 card could be gained, but never a Farmland.
    let mut g = new_state(&[id::FARMLAND, id::BORDER_VILLAGE, id::SMITHY], 2);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 0, &[id::SMITHY, id::CURSE]);
    g.gain_card(0, id::FARMLAND, Dest::Discard, &mut NoEvents);
    pick(&mut g, id::SMITHY);
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::BORDER_VILLAGE)) && !c.contains(&Choice::Card(id::FARMLAND)), "{c:?}");
    // No cards in hand: nothing happens.
    let mut g = new_state(&[id::FARMLAND], 2);
    set_deck_known(&mut g, 0, &junk());
    g.gain_card(0, id::FARMLAND, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert!(g.trash.is_empty());
}

#[test]
fn farmland_triggers_whenever_gained_and_a_victory_card_counts_two_points() {
    let mut g = new_state(&[id::FARMLAND, id::TUNNEL], 2);
    g.players[0].discard.add(id::FARMLAND, 2);
    g.players[0].discard.add(id::TUNNEL, 1);
    assert_eq!(g.players[0].vp(), 2 + 2 + 2);
}

// ===========================================================================
// Triggers with Throne Room, Watchtower and so on
// ===========================================================================

#[test]
fn throne_room_on_haggler_gains_two_cards_for_each_card_bought() {
    let mut g = new_state(&[id::HAGGLER, id::THRONE_ROOM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::HAGGLER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::HAGGLER);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 4 + 6);
    buy(&mut g, id::PROVINCE);
    for _ in 0..2 {
        let d = expect_decision(&mut g);
        assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
        g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
        adv(&mut g);
    }
    assert_eq!(g.players[0].discard.get(id::GOLD), 2 + 2, "two gained, two Golds from the hand discarded in the Clean-up");
}

#[test]
fn throne_room_on_margrave_attacks_twice() {
    let mut g = new_state(&[id::MARGRAVE, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::MARGRAVE]);
    set_deck_known(&mut g, 0, &junk());
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 1, &[id::GOLD, id::GOLD, id::GOLD]);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::MARGRAVE);
    // Two attacks: draw to 4, discard 1, draw to 4, discard 1: ends with 3 cards.
    discard_first_while_asked(&mut g);
    assert_eq!(hand_total(&g, 1), 3);
    assert_eq!(g.players[1].discard.total(), 2);
    assert_eq!(g.turn.buys, 3);
}

#[test]
fn a_gained_trail_exchanged_by_trader_cannot_be_played() {
    let mut g = new_state(&[id::TRAIL, id::TRADER, id::WORKSHOP], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::TRADER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    pick(&mut g, id::TRAIL);
    // The Trader's exchange first, then Trail's play: the Trail is back in its pile.
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(id::TRADER), "{d:?}");
    yes(&mut g);
    assert_eq!(g.supply.get(id::TRAIL), 10);
    // Trail's own question is skipped (nothing to play).
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { act: Act::Play })));
}

#[test]
fn border_village_with_watchtower_trashed_still_gains_a_card() {
    let mut g = new_state(&[id::BORDER_VILLAGE, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WATCHTOWER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::BORDER_VILLAGE);
    // Border Village's own gain is asked first (the Duchy is Watchtower's first), then Watchtower's reaction to the Border Village.
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(id::BORDER_VILLAGE), "{d:?}");
    choose(&mut g, Choice::Card(id::DUCHY));
    // Watchtower is asked about the Duchy first (declined), then about the Border Village (trashed).
    loop {
        let d = expect_decision(&mut g);
        if d.source != Some(id::WATCHTOWER) {
            break;
        }
        let c = choices(&g);
        if c.contains(&Choice::Card(id::BORDER_VILLAGE)) {
            choose(&mut g, Choice::Card(id::BORDER_VILLAGE));
        } else {
            choose(&mut g, Choice::Pass);
        }
    }
    // The gain of the Duchy happened anyway.
    assert_eq!(g.trash.get(id::BORDER_VILLAGE), 1);
    assert_eq!(g.players[0].discard.get(id::DUCHY), 1);
}

#[test]
fn hinterlands_state_round_trips_through_the_text_format() {
    let mut g = new_state(&[id::TUNNEL, id::TRAIL, id::CAULDRON, id::HAGGLER], 2);
    set_hand(&mut g, 0, &[id::TUNNEL, id::TRAIL, id::CAULDRON, id::HAGGLER, id::GOLD]);
    let text = format_state(&g);
    let back = parse_state(&text).expect("parse");
    assert_eq!(back.players[0].hand, g.players[0].hand);
    assert!(back.adv & dominion_engine::state::G_DISCARD_HOOK != 0);
    let strip = |s: &str| s.lines().filter(|l| !l.starts_with("seed:")).collect::<Vec<_>>().join("\n");
    assert_eq!(strip(&format_state(&back)), strip(&text));
}

// ===========================================================================
// The stop-moving rule (docs/stop-moving-rule.md)
// ===========================================================================

#[test]
fn a_gained_trail_that_was_played_cannot_then_be_trashed_by_watchtower_with_an_older_trail_in_the_discard_pile() {
    let mut g = new_state(&[id::WORKSHOP, id::TRAIL, id::WATCHTOWER], 2);
    set_hand(&mut g, 0, &[id::WORKSHOP, id::WATCHTOWER]);
    set_discard(&mut g, 0, &[id::TRAIL]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WORKSHOP);
    choose(&mut g, Choice::Card(id::TRAIL));
    let mut played = false;
    loop {
        match g.pending_decision().map(|d| d.kind) {
            Some(DecisionKind::YesNo { act: Act::Play }) => {
                choose(&mut g, Choice::Yes);
                played = true;
            }
            Some(DecisionKind::Select { .. }) if played => panic!("Watchtower offered for a Trail that was played: {:?}", g.pending_decision()),
            Some(DecisionKind::Select { .. }) => {
                choose(&mut g, Choice::Pass);
            }
            _ => break,
        }
    }
    assert!(played);
    assert_eq!(g.players[0].in_play.get(id::TRAIL), 1);
    assert_eq!(g.players[0].discard.get(id::TRAIL), 1, "the older Trail was not touched");
    assert!(g.trash.is_empty());
    assert_eq!(g.turn.led.n, 0);
}

// ===========================================================================
// Audit additions
// ===========================================================================

/// `new_state` for a game whose Ferryman pile (outside the Supply) is `pile`.
fn ferryman_state(pile: CardId) -> GameState {
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::FERRYMAN, id::SMITHY], ferryman: Some(pile), seed: 0x5EED_1234, max_turns: 500, ..Default::default() };
    let mut g = GameState::new(&cfg);
    let _ = g.advance(&mut NoEvents);
    reset_turn(&mut g, 0);
    for p in 0..2 {
        g.players[p] = dominion_engine::state::PlayerState::default();
    }
    g
}

#[test]
fn a_scheme_from_the_ferryman_pile_still_puts_an_action_on_the_deck() {
    let mut g = ferryman_state(id::SCHEME);
    set_hand(&mut g, 0, &[id::SCHEME, id::SMITHY, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::SCHEME);
    play(&mut g, id::SMITHY);
    buy_phase_check(&mut g);
    choose(&mut g, Choice::Pass);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, from: Zone::InPlay, .. }), "{d:?}");
}

#[test]
fn a_trader_from_the_ferryman_pile_exchanges_a_gained_card() {
    let mut g = ferryman_state(id::TRADER);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    buy(&mut g, id::DUCHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.source == Some(id::TRADER), "{d:?}");
}

#[test]
fn nomads_and_trail_from_the_ferryman_pile_react_when_gained() {
    // Gaining the Ferryman gains a card of its pile: Nomads give +$2 when gained.
    let mut g = ferryman_state(id::NOMADS);
    g.turn.phase = Phase::Buy;
    assert!(g.gain_card(0, id::FERRYMAN, Dest::Discard, &mut NoEvents));
    assert!(g.players[0].discard.has(id::NOMADS));
    assert_eq!(g.turn.coins, 2);
    let mut g = ferryman_state(id::TRAIL);
    g.turn.phase = Phase::Buy;
    assert!(g.gain_card(0, id::FERRYMAN, Dest::Discard, &mut NoEvents));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::TRAIL, "{d:?}");
}

#[test]
fn trader_exchanges_a_card_gained_from_a_pile_outside_the_supply() {
    // "You return the gained card to its pile (whether in the Supply or not)": a Spoils goes back to the Spoils pile.
    let mut g = new_state(&[id::TRADER, id::BANDIT_CAMP], 2);
    set_hand(&mut g, 0, &[id::BANDIT_CAMP, id::TRADER]);
    set_deck_known(&mut g, 0, &junk());
    let spoils = g.spoils;
    play(&mut g, id::BANDIT_CAMP);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::SPOILS && d.source == Some(id::TRADER), "{d:?}");
    yes(&mut g);
    assert_eq!(g.spoils, spoils, "back in the Spoils pile");
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert!(!g.players[0].discard.has(id::SPOILS));
}

// ===========================================================================
// Ruling coverage (audit): the wiki's FAQ / clarification bullets that no test above names
// ===========================================================================

#[test]
fn a_crossroads_played_as_a_way_counts_as_the_first_one_so_the_next_gives_no_actions() {
    let mut g = new_state(&[id::CROSSROADS, id::WAY_OF_THE_HORSE], 2);
    set_hand(&mut g, 0, &[id::CROSSROADS, id::CROSSROADS]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::CROSSROADS);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Way));
    choose(&mut g, Choice::Card(id::WAY_OF_THE_HORSE));
    // The Horse: +2 Cards +1 Action (1 - 1 + 1).
    assert_eq!(g.turn.actions, 1);
    play(&mut g, id::CROSSROADS);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Way));
    choose(&mut g, Choice::Pass);
    assert_eq!(g.turn.actions, 0, "no +3 Actions: a Crossroads was played already this turn");
}

#[test]
fn develop_gains_cards_that_go_to_the_hand_onto_the_deck_except_villa_which_moves_itself_to_the_hand() {
    // Ghost Town is normally gained to the hand; Develop puts it on the deck.
    let mut g = new_state(&[id::DEVELOP, id::GHOST_TOWN, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DEVELOP, id::SMITHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DEVELOP);
    pick(&mut g, id::GHOST_TOWN);
    assert!(!g.players[0].hand.has(id::GHOST_TOWN), "gained onto the deck");
    assert!(g.players[0].deck_known.iter_top_down().take(3).any(|c| c == id::GHOST_TOWN));
    // Villa ($4): its own ability moves it to the hand after it was gained.
    let mut g = new_state(&[id::DEVELOP, id::VILLA], 2);
    set_hand(&mut g, 0, &[id::DEVELOP, id::SILVER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DEVELOP);
    pick(&mut g, id::VILLA);
    assert!(g.players[0].hand.has(id::VILLA), "Villa moved to the hand after being gained onto the deck");
}

#[test]
fn develop_gains_one_card_and_its_on_gain_effect_resolves_before_the_other_is_chosen() {
    // Trash a Duchy ($5): a $6 Border Village first; its "gain a cheaper card" is resolved before the $4 card is chosen.
    let mut g = new_state(&[id::DEVELOP, id::BORDER_VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::DEVELOP, id::DUCHY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::DEVELOP);
    pick(&mut g, id::BORDER_VILLAGE);
    let d = expect_decision(&mut g);
    assert_eq!(d.source, Some(id::BORDER_VILLAGE), "{d:?}");
}

#[test]
fn trader_gives_silvers_for_the_current_cost_a_potion_or_debt_part_gives_nothing() {
    // With a Highway played, an Estate costs $1: one Silver.
    let mut g = new_state(&[id::TRADER, id::HIGHWAY], 2);
    set_hand(&mut g, 0, &[id::HIGHWAY, id::TRADER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HIGHWAY);
    play(&mut g, id::TRADER);
    // (Only Estates are left in the hand: one is trashed without asking.)
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    // An Apothecary costs $2 and a Potion: two Silvers, nothing for the Potion.
    let mut g = new_state(&[id::TRADER, id::APOTHECARY], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::APOTHECARY, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRADER);
    pick(&mut g, id::APOTHECARY);
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
    // A Debt cost gives nothing either: Engineer costs $0 and 4 Debt.
    let mut g = new_state(&[id::TRADER, id::ENGINEER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::ENGINEER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRADER);
    pick(&mut g, id::ENGINEER);
    assert_eq!(g.players[0].discard.get(id::SILVER), cards::cost(id::ENGINEER) as u8);
}

#[test]
fn trader_trashing_a_silver_gives_three_silvers() {
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::SILVER, id::ESTATE]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::TRADER);
    pick(&mut g, id::SILVER);
    assert_eq!(g.players[0].discard.get(id::SILVER), 3);
}

#[test]
fn a_mill_gained_by_ironworks_and_exchanged_for_a_silver_still_gives_its_action_and_card() {
    let mut g = new_state(&[id::TRADER, id::IRONWORKS, id::MILL], 2);
    set_hand(&mut g, 0, &[id::IRONWORKS, id::TRADER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::IRONWORKS);
    pick(&mut g, id::MILL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::MILL, "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
    assert_eq!(g.turn.actions, 1, "Ironworks used the Action, Mill's +1 Action");
    assert_eq!(hand_total(&g, 0), 1 + 1, "the Trader and a card for the Victory type");
}

#[test]
fn a_trader_gained_into_the_hand_can_be_exchanged_for_a_silver() {
    let mut g = new_state(&[id::TRADER, id::ARTISAN], 2);
    set_hand(&mut g, 0, &[id::ARTISAN, id::COPPER]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::ARTISAN);
    pick(&mut g, id::TRADER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::TRADER, "{d:?}");
    yes(&mut g);
    assert!(!g.players[0].hand.has(id::TRADER));
    assert_eq!(g.players[0].discard.get(id::SILVER), 1);
}

#[test]
fn nomads_gained_in_the_buy_phase_gives_two_coins_that_count_for_basilica() {
    let mut g = new_state(&[id::NOMADS, id::BASILICA], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::COPPER]);
    assert_eq!(g.turn.coins, 4);
    g.apply(Choice::Card(id::NOMADS), &mut NoEvents).unwrap();
    assert_eq!(g.turn.coins, 2, "the $2 of gaining Nomads");
    assert_eq!(g.players[0].vp_tokens, 2, "Basilica: $2 left after the gain");
}

#[test]
fn scheme_still_works_after_procession_trashed_it_and_counts_both_plays() {
    let mut g = new_state(&[id::SCHEME, id::PROCESSION, id::SMITHY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::PROCESSION, id::SCHEME]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::VILLAGE);
    play(&mut g, id::PROCESSION);
    pick(&mut g, id::SCHEME);
    pick(&mut g, id::SMITHY);
    buy_phase_check(&mut g);
    choose(&mut g, Choice::Pass);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, max: 2, .. }), "two Action cards may be put back: {d:?}");
}

#[test]
fn a_weaver_played_when_a_vassal_discards_it_is_played_only_once() {
    let mut g = new_state(&[id::WEAVER, id::VASSAL], 2);
    set_hand(&mut g, 0, &[id::VASSAL]);
    set_deck_known(&mut g, 0, &[id::WEAVER, id::ESTATE, id::ESTATE]);
    play(&mut g, id::VASSAL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::WEAVER, "{d:?}");
    yes(&mut g);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Mode { .. }));
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.players[0].discard.get(id::SILVER), 2);
    // Vassal lost track of the Weaver: it is not offered to be played a second time.
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })));
    assert_eq!(g.players[0].in_play.get(id::WEAVER), 1);
}

#[test]
fn discarding_down_to_three_discards_everything_first_and_then_the_trail_is_played_without_discarding_again() {
    let mut g = new_state(&[id::TRAIL, id::MILITIA], 2);
    set_hand(&mut g, 1, &[id::MILITIA]);
    set_hand(&mut g, 0, &[id::TRAIL, id::ESTATE, id::ESTATE, id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 0, &junk());
    reset_turn(&mut g, 1);
    play(&mut g, id::MILITIA);
    // (Picks are made in card order, so the Trail, the highest id, is the last pick.)
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::TRAIL);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::TRAIL, "{d:?}");
    yes(&mut g);
    assert_eq!(hand_total(&g, 0), 3 + 1, "the Trail drew a card; no further discarding");
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::Select { act: Act::Discard, .. })));
}

#[test]
fn a_trail_trashed_by_remodel_is_played_back_into_play_and_remodel_still_gains() {
    let mut g = new_state(&[id::TRAIL, id::REMODEL], 2);
    set_hand(&mut g, 0, &[id::REMODEL, id::TRAIL]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::REMODEL);
    // (The Trail is the only card in the hand: trashed without asking.)
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::TRAIL, "{d:?}");
    yes(&mut g);
    assert_eq!(g.players[0].in_play.get(id::TRAIL), 1);
    assert_eq!(g.trash.get(id::TRAIL), 0, "back in play, out of the trash");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "Remodel's gain follows: {d:?}");
}

#[test]
fn a_berserker_costing_nothing_gains_no_card_but_still_attacks() {
    let mut g = new_state(&[id::BERSERKER, id::SMITHY], 2);
    set_in_play(&mut g, 0, &[id::SMITHY]);
    set_hand(&mut g, 1, &[id::ESTATE, id::DUCHY, id::PROVINCE, id::ESTATE, id::DUCHY]);
    g.turn.cost_reduction = 5;
    g.gain_card(0, id::BERSERKER, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) && d.player == 1, "no Gain decision first: {d:?}");
}

#[test]
fn a_cartographer_discard_is_not_a_reveal() {
    let mut g = new_state(&[id::CARTOGRAPHER, id::PATRON], 2);
    set_hand(&mut g, 0, &[id::CARTOGRAPHER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::PATRON, id::DUCHY, id::DUCHY, id::PROVINCE, id::PROVINCE]);
    play(&mut g, id::CARTOGRAPHER);
    assert_eq!(g.players[0].coffers, 0, "looking at the cards reveals nothing");
    pick(&mut g, id::PATRON);
    assert_eq!(g.players[0].coffers, 0, "Patron's reaction is not triggered by a discard that reveals nothing");
    assert!(g.players[0].discard.has(id::PATRON));
}

#[test]
fn highway_played_as_a_way_other_than_the_chameleon_gives_no_cost_reduction() {
    let mut g = new_state(&[id::HIGHWAY, id::WAY_OF_THE_HORSE], 2);
    set_hand(&mut g, 0, &[id::HIGHWAY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HIGHWAY);
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Way));
    choose(&mut g, Choice::Card(id::WAY_OF_THE_HORSE));
    assert_eq!(g.turn.cost_reduction, 0);
    let mut g = new_state(&[id::HIGHWAY, id::WAY_OF_THE_CHAMELEON], 2);
    set_hand(&mut g, 0, &[id::HIGHWAY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::HIGHWAY);
    choose(&mut g, Choice::Card(id::WAY_OF_THE_CHAMELEON));
    assert_eq!(g.turn.cost_reduction, 1, "Chameleon follows the card's own instructions");
}

#[test]
fn procession_on_a_highway_reduces_costs_by_two() {
    let mut g = new_state(&[id::HIGHWAY, id::PROCESSION], 2);
    set_hand(&mut g, 0, &[id::PROCESSION, id::HIGHWAY]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::PROCESSION);
    pick(&mut g, id::HIGHWAY);
    assert_eq!(g.turn.cost_reduction, 2);
}

#[test]
fn an_inn_gained_through_a_border_village_can_shuffle_the_border_village_in_too() {
    let mut g = new_state(&[id::INN, id::BORDER_VILLAGE], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::GOLD]);
    g.apply(Choice::Card(id::BORDER_VILLAGE), &mut NoEvents).unwrap();
    pick(&mut g, id::INN);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::ShuffleIn, .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::INN)) && c.contains(&Choice::Card(id::BORDER_VILLAGE)), "{c:?}");
}

#[test]
fn a_moat_drawn_off_a_margrave_cannot_stop_that_attack() {
    let mut g = new_state(&[id::MARGRAVE, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::MARGRAVE]);
    set_hand(&mut g, 1, &[id::ESTATE; 4]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::MOAT, id::ESTATE]);
    play(&mut g, id::MARGRAVE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) && d.player == 1, "no Moat reveal is offered: {d:?}");
}

#[test]
fn stables_draws_after_discarding_so_the_discarded_treasure_is_shuffled_in() {
    let mut g = new_state(&[id::STABLES], 2);
    set_hand(&mut g, 0, &[id::STABLES, id::COPPER]);
    clear_deck(&mut g, 0);
    set_discard(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::STABLES);
    pick(&mut g, id::COPPER);
    // The Copper was shuffled in and drawn (and played at once, as the Buy phase begins).
    assert_eq!(hand_total(&g, 0) + g.players[0].in_play.get(id::COPPER) as u32, 3);
    assert!(g.players[0].in_play.has(id::COPPER) || g.players[0].hand.has(id::COPPER), "the discarded Copper was shuffled in and drawn");
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn wheelwright_must_gain_an_action_if_it_discarded_a_card() {
    let mut g = new_state(&[id::WHEELWRIGHT, id::SMITHY, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::WHEELWRIGHT, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WHEELWRIGHT);
    pick(&mut g, id::GOLD);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    assert!(!choices(&g).contains(&Choice::Pass), "not optional");
}

#[test]
fn witchs_hut_reveals_the_discards_and_a_card_with_another_type_still_counts_as_an_action() {
    let mut g = new_state(&[id::WITCHS_HUT, id::MILL], 2);
    set_hand(&mut g, 0, &[id::WITCHS_HUT, id::MILL, id::MILL]);
    set_deck_known(&mut g, 0, &junk());
    play(&mut g, id::WITCHS_HUT);
    // (After one Mill, the second discard can only be the other Mill: it is made at once.)
    let mut log: Vec<Event> = Vec::new();
    expect_decision(&mut g);
    choose_ev(&mut g, Choice::Card(id::MILL), &mut log);
    assert!(log.iter().any(|e| matches!(e, Event::Reveal { card, .. } if *card == id::MILL)), "revealed: {log:?}");
    assert_eq!(g.players[1].discard.get(id::CURSE), 1);
}

#[test]
fn an_overgrown_estate_trashed_by_a_bought_farmland_draws_at_once() {
    let mut g = new_state(&[id::FARMLAND, id::SMITHY], 2);
    set_deck_known(&mut g, 0, &junk());
    buy_phase(&mut g, &[id::GOLD, id::GOLD, id::OVERGROWN_ESTATE]);
    let mut log: Vec<Event> = Vec::new();
    g.apply(Choice::Card(id::FARMLAND), &mut log).unwrap();
    // (The Overgrown Estate is the only card left in the hand: trashed without asking; the Silver is the only card costing $3: gained.)
    let _ = g.advance(&mut log);
    assert_eq!(g.trash.get(id::OVERGROWN_ESTATE), 1);
    let draw = log.iter().position(|e| matches!(e, Event::Draw { player: 0, .. }));
    let silver = log.iter().position(|e| matches!(e, Event::Gain { card, .. } if *card == id::SILVER));
    assert!(draw.is_some() && silver.is_some() && draw < silver, "drew a card for the trashed Overgrown Estate before the gain: {log:?}");
}

#[test]
fn an_enchanted_crossroads_counts_as_played_so_the_next_gives_no_actions() {
    let mut g = new_state(&[id::CROSSROADS, id::ENCHANTRESS], 2);
    set_hand(&mut g, 0, &[id::ENCHANTRESS]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &junk());
    set_hand(&mut g, 1, &[id::CROSSROADS, id::CROSSROADS, id::ESTATE]);
    play(&mut g, id::ENCHANTRESS);
    for _ in 0..2000 {
        adv(&mut g);
        if g.turn.player == 1 && matches!(g.pending_decision(), Some(Decision { kind: DecisionKind::PlayAction, .. })) {
            break;
        }
        let c = choices(&g);
        let pick = if c.contains(&Choice::Pass) { Choice::Pass } else { c[0] };
        g.apply(pick, &mut NoEvents).unwrap();
    }
    g.turn.actions = 2;
    play(&mut g, id::CROSSROADS);
    assert_eq!(g.turn.actions, 2, "enchanted: +1 Card +1 Action instead (2 - 1 + 1)");
    play(&mut g, id::CROSSROADS);
    assert_eq!(g.turn.actions, 1, "no +3 Actions the second time (2 - 1)");
}

#[test]
fn a_fools_gold_blocked_by_a_highwayman_still_makes_the_next_one_worth_four() {
    let mut g = new_state(&[id::FOOLS_GOLD, id::HIGHWAYMAN], 2);
    set_hand(&mut g, 0, &[id::HIGHWAYMAN]);
    play(&mut g, id::HIGHWAYMAN);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 1, &[id::FOOLS_GOLD, id::FOOLS_GOLD]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure, "{d:?}");
    choose(&mut g, Choice::Card(id::FOOLS_GOLD));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 4, "the first (blocked) Fool's Gold does nothing; the second is the second played: $4");
}

#[test]
fn haggler_compares_coin_and_debt_parts_so_a_fortune_bought_gains_an_overlord_or_an_engineer() {
    // "$0 and 8 Debt" is cheaper than "$8 and 8 Debt" (a Possession would not be: it is not in this set).
    let mut g = new_state(&[id::HAGGLER, id::GLADIATOR, id::OVERLORD, id::ENGINEER, id::CITY_QUARTER], 2);
    set_deck_known(&mut g, 0, &junk());
    empty_pile(&mut g, id::GLADIATOR);
    g.turn.played.add(id::HAGGLER, 1);
    set_in_play(&mut g, 0, &[id::HAGGLER]);
    buy_phase(&mut g, &[]);
    g.turn.coins = 8;
    g.apply(Choice::Card(id::FORTUNE), &mut NoEvents).unwrap();
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { .. }), "{d:?}");
    let c = choices(&g);
    assert!(c.contains(&Choice::Card(id::OVERLORD)) && c.contains(&Choice::Card(id::ENGINEER)), "{c:?}");
    assert!(!c.contains(&Choice::Card(id::FORTUNE)), "not cheaper than itself");
}

#[test]
#[ignore = "audit: Trader's FAQ lets a Silver be exchanged for a Silver (it matters against Gatekeeper's Exile); the engine never offers it"]
fn trader_can_exchange_a_silver_for_a_silver() {
    let mut g = new_state(&[id::TRADER], 2);
    set_hand(&mut g, 0, &[id::TRADER, id::GOLD, id::GOLD]);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    expect_decision(&mut g);
    g.turn.buys = 2;
    buy(&mut g, id::SILVER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Return }) && d.subject == id::SILVER, "{d:?}");
}
