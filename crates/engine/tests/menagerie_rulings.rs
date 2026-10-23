//! Menagerie: one test per ruling of the wiki pages (Official FAQ and other rules clarifications) not already covered by `menagerie_cards.rs`.
//! `docs/audit/menagerie.md` has the table that maps each ruling to its test.

mod common;
use common::*;
use dominion_engine::cards::id as I;
use dominion_engine::*;

fn junk() -> Vec<CardId> {
    [I::ESTATE, I::DUCHY, I::PROVINCE].iter().cycle().take(40).copied().collect()
}

fn game_n(kingdom: &[CardId], n: usize) -> GameState {
    let mut g = new_state(kingdom, n);
    for p in 0..n {
        set_deck_known(&mut g, p, &junk());
    }
    g
}

fn game(kingdom: &[CardId]) -> GameState {
    game_n(kingdom, 2)
}

fn at_buy(kingdom: &[CardId], coins: u16, hand: &[CardId]) -> GameState {
    let mut g = game(kingdom);
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
    g.turn.buys = 2;
    set_hand(&mut g, 0, hand);
    g
}

fn ex(g: &GameState, p: usize, c: CardId) -> u8 {
    g.players[p].men.exile.get(c)
}

fn with_way(way: CardId, kingdom: &[CardId], hand: &[CardId]) -> GameState {
    let mut k = kingdom.to_vec();
    k.push(way);
    let mut g = game(&k);
    set_hand(&mut g, 0, hand);
    g
}

fn play_as(g: &mut GameState, card: CardId, way: CardId) {
    play(g, card);
    let d = expect_decision(g);
    assert!(matches!(d.kind, DecisionKind::Way), "{d:?}");
    choose(g, Choice::Card(way));
}

/// Pass decisions until player `p`'s next turn has started (stops at its first decision).
fn to_turn_of(g: &mut GameState, p: u8) {
    let n0 = g.turn.number;
    let mut guard = 0;
    while !(g.turn.player == p && g.turn.number > n0) && guard < 200 {
        guard += 1;
        if let Step::Decision(_) = adv(g) {
            pass(g);
        }
    }
    assert!(g.turn.player == p && g.turn.number > n0, "never reached the turn");
}

fn yes_no_for(g: &mut GameState, subject: CardId) -> Decision {
    let d = expect_decision(g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.subject == subject, "{d:?}");
    d
}

// ===========================================================================
// Black Cat
// ===========================================================================

#[test]
fn black_cat_played_out_of_turn_curses_in_turn_order_starting_with_the_turns_player() {
    let mut g = game_n(&[I::BLACK_CAT], 3);
    reset_turn(&mut g, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 2;
    set_hand(&mut g, 0, &[I::BLACK_CAT]);
    set_hand(&mut g, 1, &[]);
    g.supply.set(I::CURSE, 1);
    buy(&mut g, I::ESTATE);
    yes_no_for(&mut g, I::BLACK_CAT);
    choose(&mut g, Choice::Yes);
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(I::CURSE), 1, "the player whose turn it is gets the last Curse");
    assert_eq!(g.players[2].discard.get(I::CURSE), 0);
    assert_eq!(g.players[0].discard.get(I::CURSE), 0);
}

#[test]
fn black_cat_may_be_played_on_its_owners_turn_when_another_player_gains_a_victory_card_and_curses_nobody() {
    let mut g = game(&[I::BLACK_CAT]);
    set_hand(&mut g, 0, &[I::BLACK_CAT]);
    g.gain_card(1, I::PROVINCE, Dest::Discard, &mut NoEvents);
    yes_no_for(&mut g, I::BLACK_CAT);
    choose(&mut g, Choice::Yes);
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 2);
    assert_eq!(g.players[1].discard.get(I::CURSE), 0, "it is the owner's turn: no Curses");
}

#[test]
fn black_cat_played_out_of_turn_can_be_answered_with_moat_by_the_turns_player() {
    let mut g = game(&[I::BLACK_CAT, I::MOAT]);
    reset_turn(&mut g, 1);
    g.turn.phase = Phase::Buy;
    g.turn.coins = 2;
    set_hand(&mut g, 0, &[I::BLACK_CAT]);
    set_hand(&mut g, 1, &[I::MOAT]);
    buy(&mut g, I::ESTATE);
    yes_no_for(&mut g, I::BLACK_CAT);
    choose(&mut g, Choice::Yes);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }) && d.subject == I::MOAT && d.player == 1, "{d:?}");
    choose(&mut g, Choice::Yes);
    adv(&mut g);
    assert_eq!(g.players[1].discard.get(I::CURSE), 0);
}

// ===========================================================================
// Sleigh
// ===========================================================================

#[test]
fn sleigh_moves_a_card_gained_directly_onto_the_deck() {
    let mut g = game(&[I::SLEIGH]);
    set_hand(&mut g, 0, &[I::SLEIGH, I::SMITHY]);
    g.gain_card(0, I::SILVER, Dest::DeckTop, &mut NoEvents);
    yes_no_for(&mut g, I::SLEIGH);
    choose(&mut g, Choice::Yes);
    choose(&mut g, Choice::Mode(0));
    adv(&mut g);
    assert_eq!(g.players[0].hand.get(I::SILVER), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(I::ESTATE));
}

#[test]
fn sleigh_gained_to_the_hand_discards_itself_but_cannot_move_itself_back() {
    let mut g = game(&[I::SLEIGH]);
    set_hand(&mut g, 0, &[]);
    g.gain_card(0, I::SLEIGH, Dest::Hand, &mut NoEvents);
    yes_no_for(&mut g, I::SLEIGH);
    choose(&mut g, Choice::Yes);
    adv(&mut g);
    assert_eq!(g.players[0].discard.get(I::SLEIGH), 1);
    assert_eq!(g.players[0].hand.get(I::SLEIGH), 0, "it lost track of itself");
}

#[test]
fn sleigh_cannot_move_a_card_something_else_has_moved() {
    // Sleigh and Watchtower: the gained card is trashed by Watchtower first, so Sleigh has nothing to move.
    let mut g = game(&[I::SLEIGH, I::WATCHTOWER]);
    set_hand(&mut g, 0, &[I::SLEIGH, I::WATCHTOWER]);
    g.gain_card(0, I::SILVER, Dest::Discard, &mut NoEvents);
    let mut guard = 0;
    while let Some(d) = g.pending_decision() {
        guard += 1;
        assert!(guard < 10);
        match d.kind {
            DecisionKind::YesNo { .. } if d.subject == I::WATCHTOWER => {
                choose(&mut g, Choice::Yes);
            }
            DecisionKind::YesNo { .. } => {
                choose(&mut g, Choice::Yes);
            }
            DecisionKind::Mode { .. } => {
                choose(&mut g, Choice::Mode(0));
            }
            _ => {
                let c0 = choices(&g)[0];
                choose(&mut g, c0);
            }
        }
    }
    assert_eq!(g.players[0].hand.get(I::SILVER), 0, "Sleigh could not move the card once Watchtower had");
}

// ===========================================================================
// Goatherd, Scrap, Sheepdog
// ===========================================================================

#[test]
fn goatherd_draws_even_when_it_trashes_nothing() {
    let mut g = game(&[I::GOATHERD]);
    g.players[1].men.trashed_last = 1;
    set_hand(&mut g, 0, &[I::GOATHERD, I::ESTATE]);
    play(&mut g, I::GOATHERD);
    choose(&mut g, Choice::Pass);
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 1 + 1);
}

#[test]
fn goatherd_looks_at_the_player_on_its_right_in_a_three_player_game() {
    let mut g = game_n(&[I::GOATHERD], 3);
    g.players[2].men.trashed_last = 2; // to the right of player 0
    g.players[1].men.trashed_last = 5;
    set_hand(&mut g, 0, &[I::GOATHERD]);
    play(&mut g, I::GOATHERD);
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn goatherd_counts_each_trash_even_of_the_same_card_twice() {
    // Two Fortresses-like trashes of one card name: Chapel trashes two Coppers.
    let mut g = game(&[I::GOATHERD, I::CHAPEL]);
    set_hand(&mut g, 0, &[I::CHAPEL, I::COPPER, I::COPPER]);
    play(&mut g, I::CHAPEL);
    choose(&mut g, Choice::Card(I::COPPER));
    choose(&mut g, Choice::Card(I::COPPER));
    to_turn_of(&mut g, 1);
    assert_eq!(g.players[0].men.trashed_last, 2);
}

#[test]
fn scrap_draws_before_gaining_so_a_drawn_sheepdog_reacts_to_the_gain() {
    let mut g = game(&[I::SCRAP, I::SHEEPDOG]);
    set_hand(&mut g, 0, &[I::SCRAP, I::SILVER]);
    set_deck_known(&mut g, 0, &[I::SHEEPDOG, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE]);
    play(&mut g, I::SCRAP);
    choose(&mut g, Choice::Mode(0)); // +1 Card
    choose(&mut g, Choice::Mode(1)); // +1 Action
    choose(&mut g, Choice::Mode(4)); // gain a Silver
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == I::SHEEPDOG, "{d:?}");
}

#[test]
fn sheepdog_is_played_when_a_card_is_gained_on_another_players_turn_and_is_discarded_at_clean_up() {
    let mut g = game(&[I::SHEEPDOG]);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 0, &[I::SHEEPDOG]);
    g.gain_card(0, I::COPPER, Dest::Discard, &mut NoEvents);
    yes_no_for(&mut g, I::SHEEPDOG);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].hand.total(), 2);
    assert_eq!(g.players[0].in_play.get(I::SHEEPDOG), 1);
    to_turn_of(&mut g, 0);
    assert_eq!(g.players[0].in_play.get(I::SHEEPDOG), 0, "discarded at that turn's Clean-up");
}

#[test]
fn sheepdog_gained_to_the_hand_can_be_played_for_that_gain() {
    let mut g = game(&[I::SHEEPDOG]);
    set_hand(&mut g, 0, &[]);
    g.gain_card(0, I::SHEEPDOG, Dest::Hand, &mut NoEvents);
    yes_no_for(&mut g, I::SHEEPDOG);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].hand.total(), 2);
}

// ===========================================================================
// Snowy Village
// ===========================================================================

#[test]
fn snowy_village_does_not_stop_a_throne_room_from_playing_cards() {
    let mut g = game(&[I::SNOWY_VILLAGE, I::THRONE_ROOM]);
    set_hand(&mut g, 0, &[I::THRONE_ROOM, I::SNOWY_VILLAGE]);
    play(&mut g, I::THRONE_ROOM);
    choose(&mut g, Choice::Card(I::SNOWY_VILLAGE));
    adv(&mut g);
    assert_eq!(g.turn.actions, 4, "the second Snowy Village's +4 Actions are ignored");
    assert_eq!(g.turn.buys, 3, "but both gave +1 Buy");
}

#[test]
fn a_champion_played_before_snowy_village_gives_its_action_once() {
    let mut g = game(&[I::SNOWY_VILLAGE, I::SMITHY]);
    g.adv |= dominion_engine::state::G_CHAMPION;
    set_hand(&mut g, 0, &[I::SNOWY_VILLAGE, I::SMITHY]);
    set_in_play(&mut g, 0, &[I::CHAMPION]);
    play(&mut g, I::SNOWY_VILLAGE);
    assert_eq!(g.turn.actions, 5, "1 - 1 + Champion's 1 + 4");
    play(&mut g, I::SMITHY);
    assert_eq!(g.turn.actions, 4, "Champion's +1 Action for the Smithy is ignored");
}

#[test]
fn great_leader_gives_nothing_after_snowy_village() {
    let mut g = game(&[I::SNOWY_VILLAGE, I::SMITHY]);
    g.rs.active = I::GREAT_LEADER;
    set_hand(&mut g, 0, &[I::SNOWY_VILLAGE, I::SMITHY]);
    play(&mut g, I::SNOWY_VILLAGE);
    assert_eq!(g.turn.actions, 4, "Great Leader's +1 Action after it is ignored");
}

#[test]
#[ignore = "audit: Conclave gives its +1 Action before the card it plays, but its text gives it after (the wiki's Snowy Village ruling): fix in nocturne.rs Then::ConclavePlay"]
fn conclave_gives_no_action_after_it_played_snowy_village() {
    let mut g = game(&[I::CONCLAVE, I::SNOWY_VILLAGE]);
    set_hand(&mut g, 0, &[I::CONCLAVE, I::SNOWY_VILLAGE]);
    play(&mut g, I::CONCLAVE);
    choose(&mut g, Choice::Card(I::SNOWY_VILLAGE));
    adv(&mut g);
    assert_eq!(g.turn.actions, 4, "Conclave's +1 Action is ignored");
}

// ===========================================================================
// Stockpile, Bounty Hunter, Cardinal
// ===========================================================================

#[test]
fn stockpile_played_twice_with_crown_gives_six_and_two_buys_and_is_exiled_once() {
    let mut g = game(&[I::STOCKPILE, I::CROWN]);
    g.turn.phase = Phase::Buy;
    set_hand(&mut g, 0, &[I::CROWN, I::STOCKPILE]);
    let mut guard = 0;
    while let Some(d) = g.pending_decision().or_else(|| match adv(&mut g) { Step::Decision(d) => Some(d), _ => None }) {
        guard += 1;
        assert!(guard < 10);
        if d.kind == DecisionKind::Buy {
            break;
        }
        let cs = choices(&g);
        let c = cs.iter().copied().find(|c| *c == Choice::Card(I::STOCKPILE) || *c == Choice::Card(I::CROWN)).unwrap_or(cs[0]);
        choose(&mut g, c);
    }
    assert_eq!(g.turn.coins, 6);
    assert_eq!(g.turn.buys, 3);
    assert_eq!(ex(&g, 0, I::STOCKPILE), 1);
}

#[test]
fn bounty_hunter_with_an_empty_hand_gives_no_coins() {
    let mut g = game(&[I::BOUNTY_HUNTER]);
    set_hand(&mut g, 0, &[I::BOUNTY_HUNTER]);
    play(&mut g, I::BOUNTY_HUNTER);
    assert_eq!(g.turn.coins, 0);
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn bounty_hunter_exiling_the_first_copy_of_a_card_gives_three_coins_even_with_another_in_hand() {
    let mut g = game(&[I::BOUNTY_HUNTER]);
    set_hand(&mut g, 0, &[I::BOUNTY_HUNTER, I::ESTATE, I::ESTATE]);
    play(&mut g, I::BOUNTY_HUNTER); // (both Estates are alike: the pick is made at once)
    assert_eq!(ex(&g, 0, I::ESTATE), 1);
    assert_eq!(g.turn.coins, 3);
}

// ===========================================================================
// Cavalry
// ===========================================================================

#[test]
fn cavalry_gained_onto_the_deck_draws_itself() {
    let mut g = at_buy(&[I::DEMAND, I::CAVALRY], 5, &[]);
    expect_decision(&mut g);
    buy(&mut g, I::DEMAND);
    choose(&mut g, Choice::Card(I::CAVALRY));
    adv(&mut g);
    assert_eq!(g.players[0].hand.get(I::CAVALRY), 1);
}

#[test]
fn cavalry_returning_to_the_action_phase_gives_no_actions() {
    let mut g = at_buy(&[I::CAVALRY, I::SMITHY], 4, &[I::SMITHY]);
    g.turn.actions = 0;
    expect_decision(&mut g);
    let mut ev: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(I::CAVALRY), &mut ev);
    assert!(ev.iter().any(|e| matches!(e, Event::PhaseStart { phase: Phase::Action, .. })), "{ev:?}");
    assert_eq!(g.turn.actions, 0, "no Actions are given (so the turn goes on to the Buy phase again)");
}

#[test]
fn cavalry_gained_in_the_action_phase_draws_and_gives_a_buy_but_stays_in_that_phase() {
    let mut g = game(&[I::CAVALRY, I::WORKSHOP]);
    set_hand(&mut g, 0, &[I::WORKSHOP, I::WORKSHOP]);
    g.turn.actions = 2;
    play(&mut g, I::WORKSHOP);
    choose(&mut g, Choice::Card(I::CAVALRY));
    adv(&mut g);
    assert_eq!(g.players[0].hand.total(), 1 + 2);
    assert_eq!(g.turn.buys, 2);
    assert_eq!(g.turn.phase, Phase::Action);
}

// ===========================================================================
// Groom, Hostelry, Village Green
// ===========================================================================

#[test]
#[ignore = "audit: Groom's bonus is given as the card is gained, before the gain's reactions (Trader exchanging it, Cemetery's trash): it should wait under them and not apply to a card that was exchanged"]
fn groom_gives_no_bonus_when_the_card_was_not_gained() {
    // Trader exchanges the Smithy for a Silver: the Smithy was not gained, so no Horse; the Silver was not gained by Groom either.
    let mut g = game(&[I::GROOM, I::TRADER, I::SMITHY, I::SLEIGH]);
    set_hand(&mut g, 0, &[I::GROOM, I::TRADER]);
    play(&mut g, I::GROOM);
    choose(&mut g, Choice::Card(I::SMITHY));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.source == Some(I::TRADER), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].discard.get(I::HORSE), 0);
    assert_eq!(g.players[0].discard.get(I::SILVER), 1, "only the Silver the Trader gave");
}

#[test]
fn hostelry_discarded_treasures_are_discarded_not_trashed_and_need_not_be_discarded() {
    let mut g = game(&[I::HOSTELRY, I::SLEIGH]);
    set_hand(&mut g, 0, &[I::COPPER, I::SILVER, I::ESTATE]);
    g.gain_card(0, I::HOSTELRY, Dest::Discard, &mut NoEvents);
    expect_decision(&mut g);
    assert!(!choices(&g).contains(&Choice::Card(I::ESTATE)));
    choose(&mut g, Choice::Pass);
    assert_eq!(g.players[0].discard.get(I::HORSE), 0);
    assert_eq!(g.players[0].discard.total(), 1, "only the Hostelry: nothing was discarded");
}

#[test]
fn mastermind_playing_village_green_chooses_now_or_next_turn_each_time_and_stays_if_any_was_for_next_turn() {
    let mut g = game(&[I::MASTERMIND, I::VILLAGE_GREEN]);
    set_hand(&mut g, 0, &[I::MASTERMIND]);
    play(&mut g, I::MASTERMIND);
    let mut cards = vec![I::VILLAGE_GREEN, I::ESTATE, I::ESTATE, I::ESTATE, I::ESTATE];
    cards.extend(junk());
    set_deck_known(&mut g, 0, &cards);
    to_turn_of(&mut g, 0);
    choose(&mut g, Choice::Card(I::VILLAGE_GREEN));
    let mut answers = [Choice::Yes, Choice::Yes, Choice::No].into_iter();
    let mut guard = 0;
    while let Some(d) = g.pending_decision() {
        guard += 1;
        assert!(guard < 10);
        if d.source == Some(I::VILLAGE_GREEN) {
            choose(&mut g, answers.next().unwrap());
        } else {
            break;
        }
    }
    to_turn_of(&mut g, 1);
    assert_eq!(g.players[0].in_play.get(I::VILLAGE_GREEN), 1, "one play was for next turn");
    assert_eq!(g.players[0].in_play.get(I::MASTERMIND), 1, "the Mastermind stays with it");
}

#[test]
fn village_green_discarded_to_militia_together_can_each_be_played() {
    let mut g = game(&[I::VILLAGE_GREEN, I::MILITIA]);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 0, &[I::VILLAGE_GREEN, I::VILLAGE_GREEN, I::COPPER, I::COPPER, I::COPPER]);
    set_hand(&mut g, 1, &[I::MILITIA]);
    play(&mut g, I::MILITIA);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }) && d.player == 0, "{d:?}");
    choose(&mut g, Choice::Card(I::VILLAGE_GREEN)); // (the second is forced)
    let d = yes_no_for(&mut g, I::VILLAGE_GREEN);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }));
    assert_eq!(g.players[0].discard.get(I::VILLAGE_GREEN), 2, "both were discarded at once");
}

#[test]
fn village_green_discarded_from_the_revealed_cards_of_cardinal_may_be_played_out_of_turn() {
    let mut g = game(&[I::VILLAGE_GREEN, I::CARDINAL]);
    set_hand(&mut g, 0, &[I::CARDINAL]);
    set_deck_known(&mut g, 1, &[I::GOLD, I::VILLAGE_GREEN, I::ESTATE]);
    play(&mut g, I::CARDINAL);
    choose(&mut g, Choice::Card(I::GOLD)); // Exile the Gold; the Village Green is discarded
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == I::VILLAGE_GREEN && d.player == 1, "{d:?}");
}

// ===========================================================================
// Coven, Displace, Falconer
// ===========================================================================

#[test]
fn coven_one_player_exiles_the_last_curse_the_next_discards_theirs() {
    let mut g = game_n(&[I::COVEN], 3);
    g.supply.set(I::CURSE, 1);
    g.players[2].men.exile.add(I::CURSE, 2);
    g.men.hook |= dominion_engine::state::H_EXILE;
    set_hand(&mut g, 0, &[I::COVEN]);
    play(&mut g, I::COVEN);
    assert_eq!(ex(&g, 1, I::CURSE), 1);
    assert_eq!(ex(&g, 2, I::CURSE), 0);
    assert_eq!(g.players[2].discard.get(I::CURSE), 2);
}

#[test]
fn coven_is_not_a_gain_so_watchtower_cannot_avoid_it() {
    let mut g = game(&[I::COVEN, I::WATCHTOWER]);
    set_hand(&mut g, 1, &[I::WATCHTOWER]);
    set_hand(&mut g, 0, &[I::COVEN]);
    play(&mut g, I::COVEN);
    assert_eq!(ex(&g, 1, I::CURSE), 1);
    assert!(g.pending_decision().map_or(true, |d| !matches!(d.kind, DecisionKind::YesNo { .. })));
}

#[test]
fn displace_may_gain_a_card_costing_less_than_the_one_exiled() {
    let mut g = game(&[I::DISPLACE]);
    set_hand(&mut g, 0, &[I::DISPLACE, I::PROVINCE]);
    play(&mut g, I::DISPLACE);
    assert!(choices(&g).contains(&Choice::Card(I::GOLD)));
    choose(&mut g, Choice::Card(I::GOLD));
    assert_eq!(ex(&g, 0, I::PROVINCE), 1);
}

#[test]
fn falconer_gained_to_the_hand_can_be_played() {
    let mut g = game(&[I::FALCONER]);
    set_hand(&mut g, 0, &[]);
    g.gain_card(0, I::FALCONER, Dest::Hand, &mut NoEvents);
    yes_no_for(&mut g, I::FALCONER);
}

#[test]
fn falconer_reacts_in_turn_order_starting_with_the_turns_player() {
    let mut g = game_n(&[I::FALCONER, I::MILL], 3);
    reset_turn(&mut g, 1);
    set_hand(&mut g, 0, &[I::FALCONER]);
    set_hand(&mut g, 2, &[I::FALCONER]);
    set_hand(&mut g, 1, &[I::FALCONER]);
    g.gain_card(0, I::MILL, Dest::Discard, &mut NoEvents);
    let d = yes_no_for(&mut g, I::FALCONER);
    assert_eq!(d.player, 1, "the player whose turn it is reacts first");
    choose(&mut g, Choice::No);
    let d = yes_no_for(&mut g, I::FALCONER);
    assert_eq!(d.player, 2);
}

#[test]
fn falconer_counts_a_curse_as_two_types_with_charlatan() {
    let mut g = game(&[I::FALCONER, I::CHARLATAN]);
    set_hand(&mut g, 0, &[I::FALCONER]);
    g.gain_card(1, I::CURSE, Dest::Discard, &mut NoEvents);
    yes_no_for(&mut g, I::FALCONER);
}

#[test]
fn falconer_does_not_react_to_a_single_type_card() {
    let mut g = game(&[I::FALCONER]);
    set_hand(&mut g, 0, &[I::FALCONER]);
    g.gain_card(1, I::CURSE, Dest::Discard, &mut NoEvents);
    assert!(g.pending_decision().map_or(true, |d| !matches!(d.kind, DecisionKind::YesNo { .. })));
}
