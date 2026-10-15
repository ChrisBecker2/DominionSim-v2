//! Renaissance tests (rulings in `docs/renaissance-plan.md`): the 25 kingdom cards, Villagers, the Artifacts and the Projects.

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
fn random_renaissance_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0x2E4_A155);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Renaissance).collect();
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::Renaissance).collect();
    let projects: Vec<CardId> = (0..cards::NUM_PROJECTS as CardId).map(|i| cards::FIRST_PROJECT + i).collect();
    let mut played = std::collections::HashSet::new();
    let mut bought = std::collections::HashSet::new();
    let mut artifacts_taken = std::collections::HashSet::new();
    for game in 0..2500u64 {
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
        let mut ps = projects.clone();
        for _ in 0..(1 + game % 4) {
            let i = rng.below(ps.len() as u32) as usize;
            kingdom.push(ps.swap_remove(i));
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
                    g.apply(c, &mut log).unwrap_or_else(|e| panic!("game {game}: {e} for {c:?}"));
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            for e in &log {
                match *e {
                    Event::Play { card, .. } => {
                        played.insert(card);
                    }
                    Event::Buy { card, .. } if cards::is_project(card) => {
                        bought.insert(card);
                    }
                    Event::Artifact { artifact, .. } => {
                        artifacts_taken.insert(artifact);
                    }
                    _ => {}
                }
            }
            if total_cards(&g) != initial {
                panic!("card conservation, game {game}: {log:?}");
            }
            steps += 1;
            assert!(steps < 200_000, "game {game} does not end");
        }
        // The text format round-trips whatever the game left.
        let text = format_state(&g);
        let back = parse_state(&text).unwrap_or_else(|e| panic!("game {game}: {e}\n{text}"));
        assert_eq!(strip_seed(&format_state(&back)), strip_seed(&text), "game {game}");
    }
    let missing: Vec<&str> = pool.iter().filter(|c| !played.contains(c)).map(|&c| cards::name(c)).collect();
    assert!(missing.is_empty(), "never played: {missing:?}");
    let missing: Vec<&str> = projects.iter().filter(|c| !bought.contains(c)).map(|&c| cards::name(c)).collect();
    assert!(missing.is_empty(), "never bought: {missing:?}");
    assert!(artifacts_taken.len() >= 4, "{artifacts_taken:?}");
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

/// A Buy phase for player 1 with `coins` and `buys` (the Treasures all played).
fn buy_phase(g: &mut GameState, coins: u16, buys: u8) {
    g.turn.phase = Phase::Buy;
    g.turn.coins = coins;
    g.turn.buys = buys;
    g.turn.treasures_done = true;
    g.pending = Pending::None;
}

/// Plays `card` and returns the events it produced.
fn events_of(g: &mut GameState, card: CardId) -> Vec<Event> {
    let mut log: Vec<Event> = Vec::new();
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    choose_ev(g, Choice::Card(card), &mut log);
    log
}

/// Plays the game on until it is `player`'s turn again (everyone just passes; cleanup and draws happen as usual), stopping at the first decision of that
/// turn.
fn pass_until_turn_of(g: &mut GameState, player: u8) {
    let start = g.turn.number;
    for _ in 0..400 {
        match adv(g) {
            Step::Decision(d) => {
                if g.turn.player == player && g.turn.number > start && d.player == player {
                    return;
                }
                let cs = choices(g);
                let c = if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] };
                g.apply(c, &mut NoEvents).unwrap();
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("the turn never came back");
}

fn deck_of(g: &mut GameState, p: usize, n: usize) {
    set_deck_unknown(g, p, &vec![id::COPPER; n]);
}

fn villagers(g: &GameState, p: usize) -> u16 {
    g.players[p].ren.villagers
}

fn holder(g: &GameState, a: CardId) -> Option<u8> {
    g.artifact_holder(a)
}

// ===========================================================================
// Set-up
// ===========================================================================

#[test]
fn the_ids_and_counts_line_up() {
    assert_eq!(cards::kingdom_cards_in(CardSet::Renaissance).count(), 25);
    assert_eq!((cards::NUM_PROJECTS, cards::NUM_ARTIFACTS), (20, 5));
    assert_eq!(cards::NUM_CARDS, id::TREASURE_CHEST as usize + 1);
    assert_eq!(id::SICKNESS as usize + 1, cards::FIRST_PROJECT as usize);
    assert_eq!(id::CITADEL as usize + 1, cards::FIRST_ARTIFACT as usize);
    assert_eq!(cards::NUM_ZONE_IDS, 423);
    assert!(cards::LAST_KINGDOM == id::VILLAIN && cards::is_kingdom(id::BORDER_GUARD) && cards::is_kingdom(id::VILLAIN));
    assert!(cards::is_project(id::CATHEDRAL) && cards::is_project(id::CITADEL) && !cards::is_project(id::FLAG) && !cards::is_project(id::SICKNESS));
    assert!(cards::is_artifact(id::FLAG) && cards::is_artifact(id::TREASURE_CHEST) && !cards::is_artifact(id::CITADEL));
    assert!(cards::is_pseudo(id::CATHEDRAL) && cards::is_pseudo(id::KEY) && !cards::is_event(id::CATHEDRAL));
    assert_eq!(cards::set_by_name("Renaissance"), Some(CardSet::Renaissance));
    assert_eq!(cards::by_name("Treasure Chest"), Some(id::TREASURE_CHEST));
    assert_eq!(CardSet::ALL.len(), 14);
    let costs = [
        (id::BORDER_GUARD, 2), (id::DUCAT, 2), (id::LACKEYS, 2), (id::ACTING_TROUPE, 3), (id::CARGO_SHIP, 3), (id::EXPERIMENT, 3), (id::IMPROVE, 3),
        (id::FLAG_BEARER, 4), (id::HIDEOUT, 4), (id::INVENTOR, 4), (id::MOUNTAIN_VILLAGE, 4), (id::PATRON, 4), (id::PRIEST, 4), (id::RESEARCH, 4),
        (id::SILK_MERCHANT, 4), (id::OLD_WITCH, 5), (id::RECRUITER, 5), (id::SCEPTER, 5), (id::SCHOLAR, 5), (id::SCULPTOR, 5), (id::SEER, 5),
        (id::SPICES, 5), (id::SWASHBUCKLER, 5), (id::TREASURER, 5), (id::VILLAIN, 5),
        (id::CATHEDRAL, 3), (id::CITY_GATE, 3), (id::PAGEANT, 3), (id::SEWERS, 3), (id::STAR_CHART, 3), (id::EXPLORATION, 4), (id::FAIR, 4), (id::SILOS, 4),
        (id::SINISTER_PLOT, 4), (id::ACADEMY, 5), (id::CAPITALISM, 5), (id::FLEET, 5), (id::GUILDHALL, 5), (id::PIAZZA, 5), (id::ROAD_NETWORK, 5),
        (id::BARRACKS, 6), (id::CROP_ROTATION, 6), (id::INNOVATION, 6), (id::CANAL, 7), (id::CITADEL, 8),
    ];
    for (c, cost) in costs {
        assert_eq!(cards::cost(c), cost, "{}", cards::name(c));
    }
    assert!(cards::is(id::DUCAT, cards::TREASURE) && cards::is(id::SPICES, cards::TREASURE) && cards::is(id::SCEPTER, cards::TREASURE | cards::COMMAND));
    assert!(cards::is(id::CARGO_SHIP, cards::DURATION) && cards::is(id::RESEARCH, cards::DURATION) && cards::is(id::PATRON, cards::REACTION));
    assert!(cards::is(id::OLD_WITCH, cards::ATTACK) && cards::is(id::VILLAIN, cards::ATTACK));
    // The Event dealer deals the Projects of the set.
    assert_eq!(cards::pseudo_cards_of(CardSet::Renaissance).count(), 20);
}

#[test]
fn a_kingdom_lists_projects_and_every_player_starts_with_nothing() {
    let g = new_state(&[id::FAIR, id::CATHEDRAL, id::BORDER_GUARD, id::FLAG_BEARER], 3);
    assert_eq!(g.ren.projects, (1 << (id::FAIR - cards::FIRST_PROJECT)) | (1 << (id::CATHEDRAL - cards::FIRST_PROJECT)));
    assert!(!g.in_supply(id::FAIR) && g.in_supply(id::BORDER_GUARD) && !g.in_supply(id::FLAG));
    for p in 0..3 {
        assert_eq!((g.players[p].ren.villagers, g.players[p].ren.projects), (0, 0));
    }
    assert_eq!(g.supply.get(id::BORDER_GUARD), 10);
    assert_eq!(holder(&g, id::FLAG), None);
}

#[test]
fn random_kingdoms_of_the_set_deal_projects_with_the_events_and_landmarks() {
    let mut with = 0;
    for seed in 0..300u64 {
        let mut rng = Rng::new(seed);
        let k = cards::random_kingdom_with_colonies(&[CardSet::Renaissance], &[], &mut rng);
        let kingdom = k.iter().filter(|&&c| cards::is_kingdom(c)).count();
        let projects = k.iter().filter(|&&c| cards::is_project(c)).count();
        assert_eq!(kingdom, 10, "seed {seed}: {k:?}");
        assert!(projects <= 2, "at most two Projects: seed {seed}: {k:?}");
        with += projects.min(1);
    }
    // 25 cards and 20 Projects: about 4 Projects are dealt before the tenth card, two of them used.
    assert!(with > 150, "{with} of 300 kingdoms have a Project");
    // Projects are shuffled in with the Events of the other sets too: at most two of the three kinds in all.
    for seed in 0..300u64 {
        let mut rng = Rng::new(seed);
        let k = cards::random_kingdom_with_colonies(&[CardSet::Renaissance, CardSet::Adventures, CardSet::Empires], &[], &mut rng);
        let extras = k.iter().filter(|&&c| cards::is_project(c) || cards::is_event(c) || cards::is_landmark(c)).count();
        assert!(extras <= 2, "seed {seed}: {k:?}");
    }
}

// ===========================================================================
// Villagers
// ===========================================================================

#[test]
fn acting_troupe_gives_four_villagers_and_trashes_itself_even_under_a_throne_room() {
    let mut g = new_state(&[id::ACTING_TROUPE, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::ACTING_TROUPE, id::COPPER]);
    play(&mut g, id::ACTING_TROUPE);
    assert_eq!(villagers(&g, 0), 4);
    assert_eq!((g.trash.get(id::ACTING_TROUPE), g.players[0].in_play.get(id::ACTING_TROUPE)), (1, 0));
    // Throne Room: both plays give 4 Villagers, but there is only one card to trash.
    let mut g = new_state(&[id::ACTING_TROUPE, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::ACTING_TROUPE]);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::ACTING_TROUPE);
    assert_eq!(villagers(&g, 0), 8, "the second play gives its 4 Villagers although it cannot trash the card");
    assert_eq!((g.trash.get(id::ACTING_TROUPE), g.players[0].in_play.get(id::THRONE_ROOM)), (1, 1));
    // With two Acting Troupes in the hand only the one played is trashed.
    let mut g = new_state(&[id::ACTING_TROUPE], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::ACTING_TROUPE, id::ACTING_TROUPE]);
    play(&mut g, id::ACTING_TROUPE);
    assert_eq!((g.trash.get(id::ACTING_TROUPE), g.players[0].hand.get(id::ACTING_TROUPE)), (1, 1));
}

#[test]
fn a_villager_is_spent_for_an_action_only_when_no_action_is_left() {
    let mut g = new_state(&[id::SMITHY, id::VILLAGE], 2);
    g.players[0].ren.villagers = 2;
    set_hand(&mut g, 0, &[id::SMITHY, id::SMITHY, id::SMITHY]);
    deck_of(&mut g, 0, 12);
    // With an Action left, spending a Villager is not offered.
    let d = expect_decision(&mut g);
    assert!(!choices(&g).contains(&Choice::Villager), "{d:?}");
    play(&mut g, id::SMITHY);
    // Out of Actions with a Smithy in hand: a Villager or end the phase.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    assert_eq!(choices(&g), vec![Choice::Villager, Choice::Pass]);
    choose(&mut g, Choice::Villager);
    assert_eq!((g.turn.actions, villagers(&g, 0)), (1, 1));
    assert!(has_choice(&g, Choice::Card(id::SMITHY)) && !has_choice(&g, Choice::Villager));
    play(&mut g, id::SMITHY);
    choose(&mut g, Choice::Villager);
    play(&mut g, id::SMITHY);
    // No Villager left and no Action: the Buy phase.
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!(villagers(&g, 0), 0);
}

#[test]
fn villagers_are_not_asked_about_without_an_action_card_in_hand_and_may_be_kept() {
    let mut g = new_state(&[id::SMITHY], 2);
    g.players[0].ren.villagers = 3;
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::ESTATE]);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy, "no Action in hand: straight to the Buy phase");
    // Declining keeps them (they last the whole game).
    let mut g = new_state(&[id::SMITHY], 2);
    g.players[0].ren.villagers = 3;
    g.turn.actions = 0;
    set_hand(&mut g, 0, &[id::SMITHY, id::COPPER]);
    expect_decision(&mut g);
    assert_eq!(choices(&g), vec![Choice::Villager, Choice::Pass]);
    pass(&mut g);
    assert_eq!((villagers(&g, 0), g.turn.phase), (3, Phase::Buy));
}

// ===========================================================================
// The kingdom cards
// ===========================================================================

fn set_holder(g: &mut GameState, a: CardId, seat: u8) {
    g.ren.holder[(a - cards::FIRST_ARTIFACT) as usize] = seat + 1;
}

#[test]
fn border_guard_keeps_one_of_two_revealed_cards_and_discards_the_other() {
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::BORDER_GUARD);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::ToHand, .. }), "{d:?}");
    pick(&mut g, id::DUCHY);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
    assert_eq!(g.players[0].discard, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::PROVINCE));
    assert_eq!(holder(&g, id::LANTERN), None, "no Artifact without two Actions");
    // +1 Action: the Border Guard is a cantrip.
    assert_eq!(g.turn.actions, 1);
}

#[test]
fn border_guard_with_two_actions_takes_the_lantern_or_the_horn() {
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::SMITHY, id::COPPER]);
    play(&mut g, id::BORDER_GUARD);
    pick(&mut g, id::SMITHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { picks: 1, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1)]);
    choose(&mut g, Choice::Mode(1));
    assert_eq!((holder(&g, id::HORN), holder(&g, id::LANTERN)), (Some(0), None));
    assert_eq!(g.players[0].discard, counts_of(&[id::VILLAGE]));
    // The Lantern instead.
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::VILLAGE, id::COPPER]);
    play(&mut g, id::BORDER_GUARD);
    // Two Villages: nothing to choose between, straight to the Artifact.
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Mode { .. }));
    choose(&mut g, Choice::Mode(0));
    assert_eq!((holder(&g, id::LANTERN), holder(&g, id::HORN)), (Some(0), None));
}

#[test]
fn the_lantern_makes_border_guard_reveal_three_and_all_three_must_be_actions_for_the_horn() {
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE, id::SMITHY], 2);
    set_holder(&mut g, id::LANTERN, 0);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::SMITHY, id::VILLAGE, id::COPPER]);
    play(&mut g, id::BORDER_GUARD);
    pick(&mut g, id::SMITHY);
    // The Horn is taken without a question (the Lantern is already held), the other two cards are discarded.
    assert_eq!(holder(&g, id::HORN), Some(0));
    assert_eq!((g.players[0].hand.total(), g.players[0].discard.total()), (1, 2));
    // Two Actions and a Copper: no Horn.
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE, id::SMITHY], 2);
    set_holder(&mut g, id::LANTERN, 0);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::SMITHY, id::COPPER]);
    play(&mut g, id::BORDER_GUARD);
    pick(&mut g, id::SMITHY);
    assert_eq!(holder(&g, id::HORN), None);
    assert_eq!(g.players[0].discard.total(), 2);
}

#[test]
fn border_guard_reveals_fewer_cards_and_takes_no_artifact_when_the_deck_is_short() {
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::VILLAGE]);
    play(&mut g, id::BORDER_GUARD);
    // Only one card could be revealed: it is taken, and no Artifact (even though it is an Action).
    assert_eq!(g.players[0].hand, counts_of(&[id::VILLAGE]));
    assert_eq!((holder(&g, id::LANTERN), holder(&g, id::HORN)), (None, None));
}

#[test]
fn the_horn_puts_a_border_guard_onto_the_deck_once_a_turn() {
    let mut g = new_state(&[id::BORDER_GUARD, id::THRONE_ROOM], 2);
    set_holder(&mut g, id::HORN, 0);
    g.turn.actions = 3;
    set_hand(&mut g, 0, &[id::BORDER_GUARD, id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::BORDER_GUARD);
    play(&mut g, id::BORDER_GUARD);
    // Two Border Guards in play: only one is offered at Clean-up.
    pass(&mut g);
    let mut offers = 0;
    loop {
        let d = expect_decision(&mut g);
        if g.turn.player != 0 {
            break;
        }
        if matches!(d.kind, DecisionKind::YesNo { act: Act::Topdeck }) {
            assert_eq!(d.subject, id::BORDER_GUARD);
            offers += 1;
            choose(&mut g, Choice::Yes);
        } else {
            let c = if has_choice(&g, Choice::Pass) { Choice::Pass } else { choices(&g)[0] };
            choose(&mut g, c);
        }
    }
    assert_eq!(offers, 1);
    // The Border Guard on the deck was drawn into the next hand.
    assert_eq!(g.players[0].hand.get(id::BORDER_GUARD), 1);
}

#[test]
fn ducat_gives_a_coffers_and_a_buy_and_may_trash_a_copper_when_gained() {
    let mut g = new_state(&[id::DUCAT], 2);
    set_hand(&mut g, 0, &[id::DUCAT, id::COPPER]);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!((g.players[0].coffers, g.turn.buys, g.turn.coins), (1, 2, 1), "no $ of its own");
    // Gained: trash a Copper from the hand, optional.
    let mut g = new_state(&[id::DUCAT], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::ESTATE]);
    g.gain_card(0, id::DUCAT, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Trash }) && d.subject == id::COPPER, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!((count(&g, 0, id::COPPER), g.trash.get(id::COPPER)), (1, 1));
    let mut g = new_state(&[id::DUCAT], 2);
    set_hand(&mut g, 0, &[id::COPPER]);
    g.gain_card(0, id::DUCAT, Dest::Discard, &mut NoEvents);
    expect_decision(&mut g);
    choose(&mut g, Choice::No);
    assert_eq!((count(&g, 0, id::COPPER), g.trash.get(id::COPPER)), (1, 0));
    // No Copper in the hand: no question.
    let mut g = new_state(&[id::DUCAT], 2);
    set_hand(&mut g, 0, &[id::SILVER]);
    g.gain_card(0, id::DUCAT, Dest::Discard, &mut NoEvents);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn lackeys_draws_two_and_gives_two_villagers_when_gained() {
    let mut g = new_state(&[id::LACKEYS], 2);
    set_hand(&mut g, 0, &[id::LACKEYS]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::PROVINCE]);
    play(&mut g, id::LACKEYS);
    assert_eq!((g.players[0].hand.total(), villagers(&g, 0)), (2, 0));
    g.gain_card(0, id::LACKEYS, Dest::Discard, &mut NoEvents);
    assert_eq!(villagers(&g, 0), 2);
    // Any gain gives them, to whoever gains it (the other player here).
    g.gain_card(1, id::LACKEYS, Dest::Discard, &mut NoEvents);
    assert_eq!((villagers(&g, 0), villagers(&g, 1)), (2, 2));
}

#[test]
fn cargo_ship_sets_aside_a_card_gained_this_turn_for_the_next_hand() {
    let mut g = new_state(&[id::CARGO_SHIP, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CARGO_SHIP, id::COPPER, id::COPPER, id::COPPER]);
    set_deck_unknown(&mut g, 0, &[id::DUCHY; 10]);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::CARGO_SHIP);
    // The Coppers are played automatically ($2 from the Cargo Ship and $3); buy an Estate and set it aside.
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, g.turn.coins), (DecisionKind::Buy, 5));
    buy(&mut g, id::ESTATE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::SetAside }) && d.subject == id::ESTATE, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!((g.players[0].ren.cargo.get(id::ESTATE), g.players[0].discard.get(id::ESTATE)), (1, 0));
    assert_eq!(count(&g, 0, id::ESTATE), 1, "still owned");
    pass_until_turn_of(&mut g, 0);
    // The Estate is in the hand with the new hand of five, and the Cargo Ship is still in play for this turn.
    assert_eq!(g.players[0].hand.get(id::ESTATE), 1);
    assert!(g.players[0].ren.cargo.is_empty() && g.players[0].in_play.has(id::CARGO_SHIP));
    assert_eq!(g.players[0].hand.total(), 6);
}

#[test]
fn a_cargo_ship_that_set_nothing_aside_is_discarded_and_each_one_serves_one_card_once() {
    let mut g = new_state(&[id::CARGO_SHIP], 2);
    set_hand(&mut g, 0, &[id::CARGO_SHIP]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::CARGO_SHIP);
    pass_until_turn_of(&mut g, 0);
    assert!(!g.players[0].in_play.has(id::CARGO_SHIP) && g.players[0].discard.has(id::CARGO_SHIP));
    // Declining the first card leaves the option for the next gain; once used it is gone.
    let mut g = new_state(&[id::CARGO_SHIP], 2);
    g.turn.actions = 1;
    set_hand(&mut g, 0, &[id::CARGO_SHIP]);
    play(&mut g, id::CARGO_SHIP);
    buy_phase(&mut g, 8, 3);
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::SetAside }));
    choose(&mut g, Choice::No);
    g.gain_card(0, id::GOLD, Dest::Discard, &mut NoEvents);
    expect_decision(&mut g);
    choose(&mut g, Choice::Yes);
    assert_eq!((g.players[0].ren.cargo.get(id::GOLD), g.players[0].ren.cargo.get(id::SILVER), g.turn.ren.cargo), (1, 0, 0));
    g.gain_card(0, id::GOLD, Dest::Discard, &mut NoEvents);
    assert!(!matches!(expect_decision(&mut g).kind, DecisionKind::YesNo { .. }), "the Cargo Ship has been used");
}

#[test]
fn experiment_returns_to_its_pile_and_comes_with_another_one() {
    let mut g = new_state(&[id::EXPERIMENT], 2);
    set_hand(&mut g, 0, &[id::EXPERIMENT]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY]);
    set_supply(&mut g, id::EXPERIMENT, 9);
    play(&mut g, id::EXPERIMENT);
    assert_eq!((g.players[0].hand.total(), g.turn.actions, g.supply.get(id::EXPERIMENT), g.players[0].in_play.total()), (2, 1, 10, 0));
    // Gaining one gives a second, that does not chain.
    g.gain_card(0, id::EXPERIMENT, Dest::Discard, &mut NoEvents);
    assert_eq!((g.players[0].discard.get(id::EXPERIMENT), g.supply.get(id::EXPERIMENT)), (2, 8));
    // With one left only one is gained.
    set_supply(&mut g, id::EXPERIMENT, 1);
    g.gain_card(1, id::EXPERIMENT, Dest::Discard, &mut NoEvents);
    assert_eq!((g.players[1].discard.get(id::EXPERIMENT), g.supply.get(id::EXPERIMENT)), (1, 0));
    // Gained to the hand (Sculptor): the second goes to the discard pile.
    let mut g = new_state(&[id::EXPERIMENT, id::SCULPTOR], 2);
    set_hand(&mut g, 0, &[id::SCULPTOR]);
    play(&mut g, id::SCULPTOR);
    pick(&mut g, id::EXPERIMENT);
    assert_eq!((g.players[0].hand.get(id::EXPERIMENT), g.players[0].discard.get(id::EXPERIMENT)), (1, 1));
    // Played by a Throne Room, it returns once.
    let mut g = new_state(&[id::EXPERIMENT, id::THRONE_ROOM], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::EXPERIMENT]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 4]);
    set_supply(&mut g, id::EXPERIMENT, 9);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::EXPERIMENT);
    assert_eq!((g.supply.get(id::EXPERIMENT), g.players[0].hand.total()), (10, 4));
}

#[test]
fn improve_trashes_an_action_at_cleanup_for_a_card_costing_exactly_one_more() {
    let mut g = new_state(&[id::IMPROVE, id::VILLAGE, id::SMITHY, id::MILITIA], 2);
    set_hand(&mut g, 0, &[id::IMPROVE, id::VILLAGE]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::VILLAGE);
    play(&mut g, id::IMPROVE);
    assert_eq!(g.turn.coins, 3, "$2 from the Improve and $1 from the Copper the Village drew");
    pass(&mut g);
    // The start of Clean-up: an Action card that would be discarded from play (the Silver cannot be chosen).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::InPlay, act: Act::Trash, min: 0, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Card(id::IMPROVE), Choice::Pass]);
    pick(&mut g, id::VILLAGE);
    // Exactly $4: Smithy or Militia (not the Silver, not a $5 card).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { exact: true, max_cost: 4, .. }), "{d:?}");
    assert!(has_choice(&g, Choice::Card(id::SMITHY)) && !has_choice(&g, Choice::Card(id::SILVER)) && !has_choice(&g, Choice::Card(id::DUCHY)));
    pick(&mut g, id::SMITHY);
    assert_eq!((g.trash.get(id::VILLAGE), count(&g, 0, id::SMITHY)), (1, 1));
}

#[test]
fn improve_may_trash_itself_or_nothing_and_each_play_gives_its_own_offer() {
    let mut g = new_state(&[id::IMPROVE, id::THRONE_ROOM, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::IMPROVE]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::IMPROVE);
    assert_eq!(g.turn.coins, 4, "$2 twice");
    pass(&mut g);
    // Two offers (one per play of the Improve).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::InPlay, act: Act::Trash, .. }));
    pick(&mut g, id::IMPROVE);
    // Improve ($3) -> a card costing $4.
    pick(&mut g, id::SMITHY);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::InPlay, act: Act::Trash, .. }), "the second offer: {d:?}");
    pass(&mut g);
    assert_eq!((g.trash.get(id::IMPROVE), count(&g, 0, id::SMITHY)), (1, 1));
}

#[test]
fn flag_bearer_takes_the_flag_when_gained_or_trashed_from_whoever_has_it() {
    let mut g = new_state(&[id::FLAG_BEARER, id::CHAPEL], 2);
    set_holder(&mut g, id::FLAG, 1);
    g.gain_card(0, id::FLAG_BEARER, Dest::Discard, &mut NoEvents);
    assert_eq!(holder(&g, id::FLAG), Some(0), "taken from the other player");
    // Trashed: the trasher takes it.
    let mut g = new_state(&[id::FLAG_BEARER, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::CHAPEL, id::FLAG_BEARER]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::FLAG_BEARER);
    assert_eq!((holder(&g, id::FLAG), g.trash.get(id::FLAG_BEARER)), (Some(0), 1));
    // Playing it is $2.
    let mut g = new_state(&[id::FLAG_BEARER], 2);
    set_hand(&mut g, 0, &[id::FLAG_BEARER]);
    play(&mut g, id::FLAG_BEARER);
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn the_flag_adds_a_card_to_the_cleanup_draw_whatever_the_size_of_the_hand() {
    let mut g = new_state(&[id::FLAG_BEARER, id::OUTPOST], 2);
    set_holder(&mut g, id::FLAG, 0);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 20]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 20]);
    set_hand(&mut g, 0, &[id::COPPER]);
    pass_until_turn_of(&mut g, 0);
    assert_eq!(g.players[0].hand.total(), 6);
    // The other player's hand is the usual 5.
    assert_eq!(g.players[1].hand.total(), 5);
    // Outpost: 3 cards, plus the Flag's.
    let mut g = new_state(&[id::FLAG_BEARER, id::OUTPOST], 2);
    set_holder(&mut g, id::FLAG, 0);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 20]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 20]);
    set_hand(&mut g, 0, &[id::OUTPOST]);
    play(&mut g, id::OUTPOST);
    pass(&mut g);
    assert_eq!((g.turn.player, g.players[0].hand.total()), (0, 4), "an Outpost turn: 3 cards and the Flag's");
}

#[test]
fn hideout_trashes_a_card_and_a_victory_card_gives_a_curse() {
    let mut g = new_state(&[id::HIDEOUT], 2);
    set_hand(&mut g, 0, &[id::HIDEOUT, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::HIDEOUT);
    assert_eq!((g.turn.actions, g.players[0].hand.get(id::SILVER)), (2, 1), "+1 Card +2 Actions");
    pick(&mut g, id::ESTATE);
    assert_eq!((g.trash.get(id::ESTATE), count(&g, 0, id::CURSE)), (1, 1));
    let mut g = new_state(&[id::HIDEOUT], 2);
    set_hand(&mut g, 0, &[id::HIDEOUT, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::HIDEOUT);
    pick(&mut g, id::COPPER);
    assert_eq!((g.trash.get(id::COPPER), count(&g, 0, id::CURSE)), (1, 0));
    // A Curse is not a Victory card; trashing is not optional (a hand of one card: it is trashed).
    let mut g = new_state(&[id::HIDEOUT], 2);
    set_hand(&mut g, 0, &[id::HIDEOUT, id::CURSE]);
    play(&mut g, id::HIDEOUT);
    assert_eq!((g.trash.get(id::CURSE), count(&g, 0, id::CURSE)), (1, 0));
}

#[test]
fn inventor_gains_up_to_four_and_then_cards_cost_one_less_cumulatively() {
    let mut g = new_state(&[id::INVENTOR, id::SMITHY], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::INVENTOR, id::INVENTOR]);
    play(&mut g, id::INVENTOR);
    // First the gain (up to $4), only then the cost reduction.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, .. }), "{d:?}");
    assert!(!has_choice(&g, Choice::Card(id::DUCHY)));
    pick(&mut g, id::SILVER);
    assert_eq!((g.cost(id::DUCHY), g.cost(id::SMITHY), g.cost(id::COPPER)), (4, 3, 0));
    // The second Inventor can gain a Duchy now.
    play(&mut g, id::INVENTOR);
    assert!(has_choice(&g, Choice::Card(id::DUCHY)) && !has_choice(&g, Choice::Card(id::GOLD)));
    pick(&mut g, id::DUCHY);
    assert_eq!((g.cost(id::DUCHY), g.cost(id::GOLD)), (3, 4));
}

#[test]
fn mountain_village_takes_a_card_from_the_discard_pile_or_draws_when_it_is_empty() {
    let mut g = new_state(&[id::MOUNTAIN_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::MOUNTAIN_VILLAGE]);
    set_discard(&mut g, 0, &[id::PROVINCE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::SILVER]);
    play(&mut g, id::MOUNTAIN_VILLAGE);
    assert_eq!(g.turn.actions, 2);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Discard, act: Act::ToHand, min: 1, max: 1, .. }), "{d:?}");
    pick(&mut g, id::PROVINCE);
    assert_eq!((g.players[0].hand, g.players[0].discard.total()), (counts_of(&[id::PROVINCE]), 2));
    // With an empty discard pile: +1 Card instead.
    let mut g = new_state(&[id::MOUNTAIN_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::MOUNTAIN_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::GOLD]);
    play(&mut g, id::MOUNTAIN_VILLAGE);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
}

#[test]
fn patron_gives_a_villager_and_two_dollars_and_a_coffers_when_revealed_in_an_action_phase() {
    let mut g = new_state(&[id::PATRON], 2);
    set_hand(&mut g, 0, &[id::PATRON]);
    play(&mut g, id::PATRON);
    assert_eq!((villagers(&g, 0), g.turn.coins), (1, 2));
    // Revealed by a Border Guard (during the Action phase).
    let mut g = new_state(&[id::PATRON, id::BORDER_GUARD], 2);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::PATRON, id::PATRON, id::COPPER]);
    play(&mut g, id::BORDER_GUARD);
    assert_eq!(g.players[0].coffers, 2, "both Patrons were revealed");
    // Any player's reveal in any Action phase counts; the Buy phase does not.
    let mut log = Vec::new();
    let mut g = new_state(&[id::PATRON], 2);
    g.reveal_card(1, id::PATRON, &mut log);
    assert_eq!((g.players[1].coffers, g.players[0].coffers), (1, 0));
    g.turn.phase = Phase::Buy;
    g.reveal_card(1, id::PATRON, &mut log);
    assert_eq!(g.players[1].coffers, 1);
}

#[test]
fn priest_trashes_a_card_and_every_later_trash_this_turn_is_two_dollars() {
    let mut g = new_state(&[id::PRIEST, id::CHAPEL], 2);
    set_hand(&mut g, 0, &[id::PRIEST, id::CURSE, id::CURSE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::PRIEST);
    assert_eq!(g.turn.coins, 2);
    pick(&mut g, id::CURSE);
    assert_eq!(g.turn.coins, 2, "the trash of the Priest itself is before its ability");
    assert_eq!(g.turn.ren.priests, 1);
    // Two Priests and two Coppers: $2 + $2 for the plays, and $2 for the second trash.
    let mut g = new_state(&[id::PRIEST], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::PRIEST, id::PRIEST, id::CURSE, id::ESTATE, id::DUCHY]);
    play(&mut g, id::PRIEST);
    pick(&mut g, id::CURSE);
    play(&mut g, id::PRIEST);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.turn.coins, 6);
    // The bonus is for the Priest's turn: a Chapel afterwards is worth $2 per card.
    let mut g = new_state(&[id::PRIEST, id::CHAPEL], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::PRIEST, id::CHAPEL, id::CURSE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::PRIEST);
    pick(&mut g, id::CURSE);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::ESTATE);
    pass(&mut g);
    assert_eq!(g.turn.coins, 2 + 4);
}

#[test]
fn research_trashes_a_card_and_sets_aside_a_card_per_dollar_for_next_turn() {
    let mut g = new_state(&[id::RESEARCH], 2);
    set_hand(&mut g, 0, &[id::RESEARCH, id::SILVER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::DUCHY, id::PROVINCE, id::SILVER]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::RESEARCH);
    assert_eq!(g.turn.actions, 1, "+1 Action");
    // The Silver (the only card) is trashed: the top three cards are set aside.
    assert_eq!(g.trash.get(id::SILVER), 1);
    assert_eq!(g.players[0].ren.research.total(), 3);
    assert_eq!(count(&g, 0, id::PROVINCE), 1);
    pass_until_turn_of(&mut g, 0);
    assert!(g.players[0].ren.research.is_empty());
    // The three cards are in the hand (the Treasures of the new hand were played at once).
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE, id::DUCHY, id::PROVINCE]));
    assert!(g.players[0].in_play.has(id::RESEARCH), "stays in play for the turn the cards come back");
}

#[test]
fn research_sets_aside_what_there_is_and_is_discarded_when_it_sets_nothing_aside() {
    let mut g = new_state(&[id::RESEARCH], 2);
    set_hand(&mut g, 0, &[id::RESEARCH, id::GOLD]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::COPPER]);
    play(&mut g, id::RESEARCH);
    assert_eq!(g.players[0].ren.research.total(), 2, "$6 asks for six cards, there are two");
    // Trashing a Copper... no cards at all in the deck: nothing set aside, the Research is discarded at Clean-up.
    let mut g = new_state(&[id::RESEARCH], 2);
    set_hand(&mut g, 0, &[id::RESEARCH, id::COPPER]);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::RESEARCH);
    assert!(g.players[0].ren.research.is_empty());
    pass_until_turn_of(&mut g, 0);
    assert!(!g.players[0].in_play.has(id::RESEARCH));
}

#[test]
fn silk_merchant_gives_coffers_and_a_villager_when_gained_or_trashed() {
    let mut g = new_state(&[id::SILK_MERCHANT, id::CHAPEL], 2);
    g.gain_card(0, id::SILK_MERCHANT, Dest::Discard, &mut NoEvents);
    assert_eq!((g.players[0].coffers, villagers(&g, 0)), (1, 1));
    set_hand(&mut g, 0, &[id::CHAPEL, id::SILK_MERCHANT]);
    deck_of(&mut g, 0, 5);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::SILK_MERCHANT);
    assert_eq!((g.players[0].coffers, villagers(&g, 0)), (2, 2));
    // Played: +2 Cards +1 Buy.
    let mut g = new_state(&[id::SILK_MERCHANT], 2);
    set_hand(&mut g, 0, &[id::SILK_MERCHANT]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::SILK_MERCHANT);
    assert_eq!((g.players[0].hand.total(), g.turn.buys), (2, 2));
}

#[test]
fn old_witch_draws_three_and_gives_a_curse_that_may_be_trashed_with_a_curse_from_the_hand() {
    let mut g = new_state(&[id::OLD_WITCH], 3);
    set_hand(&mut g, 0, &[id::OLD_WITCH]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_hand(&mut g, 1, &[id::CURSE, id::COPPER]);
    set_hand(&mut g, 2, &[id::COPPER]);
    let curses = g.supply.get(id::CURSE);
    play(&mut g, id::OLD_WITCH);
    assert_eq!(g.players[0].hand.total(), 3);
    // Player 2 gained a Curse and may trash the one in hand (the new one is in the discard pile).
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, min: 0, max: 1, .. }), "{d:?}");
    assert_eq!(count(&g, 1, id::CURSE), 2);
    pick(&mut g, id::CURSE);
    assert_eq!((count(&g, 1, id::CURSE), count(&g, 2, id::CURSE), g.supply.get(id::CURSE), g.trash.get(id::CURSE)), (1, 1, curses - 2, 1));
    // Player 3 has no Curse in hand: no question.
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn old_witch_trashing_works_with_the_curse_pile_empty_and_a_moat_stops_both_halves() {
    let mut g = new_state(&[id::OLD_WITCH], 2);
    set_hand(&mut g, 0, &[id::OLD_WITCH]);
    set_hand(&mut g, 1, &[id::CURSE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::COPPER; 3]);
    empty_pile(&mut g, id::CURSE);
    play(&mut g, id::OLD_WITCH);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    pick(&mut g, id::CURSE);
    assert_eq!((g.trash.get(id::CURSE), count(&g, 1, id::CURSE)), (1, 0));
    let mut g = new_state(&[id::OLD_WITCH, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::OLD_WITCH]);
    set_hand(&mut g, 1, &[id::CURSE, id::MOAT]);
    set_deck_known(&mut g, 0, &[id::COPPER; 3]);
    play(&mut g, id::OLD_WITCH);
    moat(&mut g, true);
    assert_eq!((count(&g, 1, id::CURSE), g.trash.get(id::CURSE)), (1, 0), "no Curse gained and none trashed");
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn recruiter_draws_two_trashes_a_card_and_gives_a_villager_per_dollar_it_cost() {
    let mut g = new_state(&[id::RECRUITER], 2);
    set_hand(&mut g, 0, &[id::RECRUITER, id::SILVER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::GOLD]);
    play(&mut g, id::RECRUITER);
    assert_eq!(g.players[0].hand.total(), 4);
    pick(&mut g, id::SILVER);
    assert_eq!(villagers(&g, 0), 3);
    let mut g = new_state(&[id::RECRUITER], 2);
    set_hand(&mut g, 0, &[id::RECRUITER, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::ESTATE, id::GOLD]);
    play(&mut g, id::RECRUITER);
    pick(&mut g, id::GOLD);
    assert_eq!(villagers(&g, 0), 6);
    // A card costing 0 gives none; a Potion in the cost is not counted.
    let mut g = new_state(&[id::RECRUITER, id::FAMILIAR], 2);
    set_hand(&mut g, 0, &[id::RECRUITER, id::FAMILIAR]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    play(&mut g, id::RECRUITER);
    pick(&mut g, id::FAMILIAR);
    assert_eq!(villagers(&g, 0), 3, "Familiar costs $3 and a Potion");
}

#[test]
fn scepter_is_two_dollars_or_replays_an_action_played_this_turn() {
    let mut g = new_state(&[id::SCEPTER, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET, id::SCEPTER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD]);
    play(&mut g, id::MARKET);
    assert_eq!((g.turn.coins, g.turn.buys, g.players[0].hand.get(id::COPPER)), (1, 2, 1));
    // Buy phase: the Scepter is a Treasure with a choice.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    play_treasure(&mut g, id::SCEPTER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }));
    choose(&mut g, Choice::Mode(1));
    // The Market is the only Action to replay: +1 Card, +1 Action, +1 Buy, +$1.
    // ($1 and $1, and the Copper and the Silver drawn are played in the Buy phase that follows.)
    assert_eq!((g.turn.coins, g.turn.buys, g.players[0].in_play.get(id::SILVER)), (1 + 1 + 1 + 2, 3, 1));
    // With two Actions to choose from, the Scepter asks.
    let mut g = new_state(&[id::SCEPTER, id::MARKET, id::VILLAGE], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::MARKET, id::VILLAGE, id::SCEPTER]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
    play(&mut g, id::MARKET);
    play(&mut g, id::VILLAGE);
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::InPlay, act: Act::Choose, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::VILLAGE), Choice::Card(id::MARKET)]);
    pick(&mut g, id::VILLAGE);
    assert_eq!((g.turn.actions, g.turn.coins, g.players[0].hand.total()), (5, 1, 3));
    // +$2 instead.
    let mut g = new_state(&[id::SCEPTER, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::SCEPTER]);
    expect_decision(&mut g);
    assert_eq!(g.turn.phase, Phase::Buy);
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(0));
    assert_eq!(g.turn.coins, 2);
}

#[test]
fn scepter_cannot_replay_a_card_that_is_not_in_play_or_a_duration_of_an_earlier_turn() {
    let mut g = new_state(&[id::SCEPTER, id::ACTING_TROUPE, id::CARGO_SHIP], 2);
    set_hand(&mut g, 0, &[id::ACTING_TROUPE, id::SCEPTER]);
    play(&mut g, id::ACTING_TROUPE);
    // Acting Troupe trashed itself: it is not in play to be replayed.
    expect_decision(&mut g);
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy, "nothing to replay");
    assert_eq!(villagers(&g, 0), 4);
    // A Cargo Ship played on an earlier turn is in play but was not played this turn.
    let mut g = new_state(&[id::SCEPTER, id::CARGO_SHIP], 2);
    set_in_play(&mut g, 0, &[id::CARGO_SHIP]);
    set_hand(&mut g, 0, &[id::SCEPTER]);
    expect_decision(&mut g);
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!(g.turn.coins, 0);
}

#[test]
fn scholar_discards_the_hand_and_draws_seven() {
    let mut g = new_state(&[id::SCHOLAR], 2);
    set_hand(&mut g, 0, &[id::SCHOLAR, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 7]);
    deck_of(&mut g, 0, 5);
    play(&mut g, id::SCHOLAR);
    assert_eq!((g.players[0].hand, g.players[0].discard), (counts_of(&[id::PROVINCE; 7]), counts_of(&[id::COPPER, id::ESTATE])));
    // A short deck: the discarded cards are shuffled in.
    let mut g = new_state(&[id::SCHOLAR], 2);
    set_hand(&mut g, 0, &[id::SCHOLAR, id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::PROVINCE]);
    play(&mut g, id::SCHOLAR);
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE, id::PROVINCE, id::ESTATE]), "the Copper was played; the Estate came back from the shuffle");
    assert!(g.players[0].in_play.has(id::COPPER));
}

#[test]
fn sculptor_gains_to_the_hand_and_a_treasure_gives_a_villager() {
    let mut g = new_state(&[id::SCULPTOR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SCULPTOR]);
    play(&mut g, id::SCULPTOR);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 4, dest: Dest::Hand, .. }), "{d:?}");
    pick(&mut g, id::SILVER);
    assert_eq!((g.players[0].in_play.get(id::SILVER), villagers(&g, 0)), (1, 1), "the Silver was gained to the hand and played in the Buy phase");
    let mut g = new_state(&[id::SCULPTOR, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::SCULPTOR]);
    play(&mut g, id::SCULPTOR);
    pick(&mut g, id::SMITHY);
    assert_eq!((g.players[0].hand, villagers(&g, 0)), (counts_of(&[id::SMITHY]), 0));
}

#[test]
fn seer_puts_the_cards_costing_two_to_four_into_the_hand_and_the_rest_back_in_any_order() {
    let mut g = new_state(&[id::SEER, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::SEER]);
    set_deck_known(&mut g, 0, &[id::CURSE, id::VILLAGE, id::ESTATE, id::GOLD, id::PROVINCE]);
    play(&mut g, id::SEER);
    // +1 Card (Curse), then Village ($3) and Estate ($2) are taken; the Gold goes back on top.
    assert_eq!(g.players[0].hand, counts_of(&[id::CURSE, id::VILLAGE, id::ESTATE]));
    assert_eq!((g.players[0].deck_known.peek_top(), g.turn.actions), (Some(id::GOLD), 1));
    // Nothing in range: the three go back in the order chosen.
    let mut g = new_state(&[id::SEER], 2);
    set_hand(&mut g, 0, &[id::SEER]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::GOLD, id::PROVINCE, id::CURSE]);
    play(&mut g, id::SEER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Topdeck, ordered: true, .. }), "{d:?}");
    pick(&mut g, id::GOLD);
    pick(&mut g, id::PROVINCE);
    // The last card has nothing to be ordered with: it goes back by itself, on top.
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::CURSE));
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::CURSE, id::PROVINCE, id::GOLD]);
    // A Potion in the cost is not $2 to $4 (Alchemist costs $3 and a Potion).
    let mut g = new_state(&[id::SEER, id::ALCHEMIST], 2);
    set_hand(&mut g, 0, &[id::SEER]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::ALCHEMIST, id::ALCHEMIST, id::ALCHEMIST]);
    play(&mut g, id::SEER);
    assert_eq!(g.players[0].hand, counts_of(&[id::DUCHY]));
}

#[test]
fn spices_is_two_dollars_and_a_buy_and_gives_two_coffers_when_gained() {
    let mut g = new_state(&[id::SPICES], 2);
    set_hand(&mut g, 0, &[id::SPICES, id::SPICES]);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!((g.turn.coins, g.turn.buys), (4, 3));
    g.gain_card(0, id::SPICES, Dest::Discard, &mut NoEvents);
    assert_eq!(g.players[0].coffers, 2);
}

#[test]
fn swashbuckler_gives_a_coffers_if_the_discard_pile_is_not_empty_after_the_draw() {
    let mut g = new_state(&[id::SWASHBUCKLER], 2);
    set_hand(&mut g, 0, &[id::SWASHBUCKLER]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 3]);
    play(&mut g, id::SWASHBUCKLER);
    assert_eq!((g.players[0].hand.total(), g.players[0].coffers), (3, 1));
    // The draw shuffles the discard pile into the deck: nothing.
    let mut g = new_state(&[id::SWASHBUCKLER], 2);
    set_hand(&mut g, 0, &[id::SWASHBUCKLER]);
    set_discard(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY]);
    play(&mut g, id::SWASHBUCKLER);
    assert_eq!((g.players[0].hand.total(), g.players[0].coffers), (3, 0));
    // No discard pile at all.
    let mut g = new_state(&[id::SWASHBUCKLER], 2);
    set_hand(&mut g, 0, &[id::SWASHBUCKLER]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 3]);
    play(&mut g, id::SWASHBUCKLER);
    assert_eq!(g.players[0].coffers, 0);
}

#[test]
fn four_coffers_make_swashbuckler_take_the_treasure_chest_which_gains_a_gold_every_buy_phase() {
    let mut g = new_state(&[id::SWASHBUCKLER, id::BORDER_GUARD], 2);
    set_hand(&mut g, 0, &[id::SWASHBUCKLER]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY; 3]);
    g.players[0].coffers = 3;
    play(&mut g, id::SWASHBUCKLER);
    assert_eq!((g.players[0].coffers, holder(&g, id::TREASURE_CHEST)), (4, Some(0)));
    // The Chest works the turn it is taken: a Gold at the start of the Buy phase.
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!(g.players[0].discard.get(id::GOLD), 1);
    // Fewer than 4: no Chest. Taken from the other player when it was theirs.
    let mut g = new_state(&[id::SWASHBUCKLER], 2);
    set_holder(&mut g, id::TREASURE_CHEST, 1);
    set_hand(&mut g, 0, &[id::SWASHBUCKLER]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER; 3]);
    g.players[0].coffers = 2;
    play(&mut g, id::SWASHBUCKLER);
    assert_eq!((g.players[0].coffers, holder(&g, id::TREASURE_CHEST)), (3, Some(1)));
    // Already holding 4 or more: the Chest, even if it had to be taken from the other player.
    let mut g = new_state(&[id::SWASHBUCKLER], 2);
    set_holder(&mut g, id::TREASURE_CHEST, 1);
    set_hand(&mut g, 0, &[id::SWASHBUCKLER]);
    set_discard(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER; 3]);
    g.players[0].coffers = 5;
    play(&mut g, id::SWASHBUCKLER);
    assert_eq!(holder(&g, id::TREASURE_CHEST), Some(0));
    // The opponent's Buy phase does not give it a Gold.
    let mut g = new_state(&[id::SWASHBUCKLER], 2);
    set_holder(&mut g, id::TREASURE_CHEST, 0);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    pass_until_turn_of(&mut g, 0);
    assert_eq!((g.players[0].discard.get(id::GOLD), g.players[1].discard.get(id::GOLD)), (2, 0), "a Gold in each of the holder's Buy phases, none in the other player's");
}

#[test]
fn treasurer_is_three_dollars_and_trashes_a_treasure_or_regains_one_or_takes_the_key() {
    let mut g = new_state(&[id::TREASURER], 2);
    set_hand(&mut g, 0, &[id::TREASURER, id::COPPER, id::SILVER]);
    play(&mut g, id::TREASURER);
    assert_eq!(g.turn.coins, 3);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }));
    assert_eq!(choices(&g), vec![Choice::Mode(0), Choice::Mode(1), Choice::Mode(2)]);
    choose(&mut g, Choice::Mode(0));
    pick(&mut g, id::COPPER);
    assert_eq!((g.trash.get(id::COPPER), g.players[0].hand.get(id::COPPER)), (1, 0));
    // Gain a Treasure from the trash to the hand.
    let mut g = new_state(&[id::TREASURER], 2);
    g.trash.add(id::GOLD, 1);
    g.trash.add(id::SILVER, 1);
    g.trash.add(id::ESTATE, 1);
    set_hand(&mut g, 0, &[id::TREASURER]);
    play(&mut g, id::TREASURER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Card(id::GOLD)], "only Treasures");
    pick(&mut g, id::GOLD);
    assert_eq!((g.players[0].in_play.get(id::GOLD), g.trash.get(id::GOLD)), (1, 0), "gained to the hand, played in the Buy phase");
    // The Key: no help this turn, +$1 at the start of every later turn; it can be taken again by the holder.
    let mut g = new_state(&[id::TREASURER], 2);
    set_hand(&mut g, 0, &[id::TREASURER]);
    set_holder(&mut g, id::KEY, 1);
    play(&mut g, id::TREASURER);
    choose(&mut g, Choice::Mode(2));
    assert_eq!((holder(&g, id::KEY), g.turn.coins), (Some(0), 3));
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    pass_until_turn_of(&mut g, 0);
    assert_eq!(g.turn.coins, 1, "the Key's $1 at the start of the turn");
}

#[test]
fn villain_gives_two_coffers_and_makes_players_with_five_cards_discard_a_card_costing_two_or_more() {
    let mut g = new_state(&[id::VILLAIN], 3);
    set_hand(&mut g, 0, &[id::VILLAIN]);
    set_hand(&mut g, 1, &[id::ESTATE, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    set_hand(&mut g, 2, &[id::ESTATE, id::ESTATE, id::SILVER, id::COPPER]);
    play(&mut g, id::VILLAIN);
    assert_eq!(g.players[0].coffers, 2);
    // Player 2 must discard the Estate (the only card costing $2): nothing to choose. Player 3 has only four cards.
    assert_eq!(g.players[1].discard, counts_of(&[id::ESTATE]));
    assert_eq!(g.players[2].discard.total(), 0);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    // Two cards to choose between.
    let mut g = new_state(&[id::VILLAIN], 2);
    set_hand(&mut g, 0, &[id::VILLAIN]);
    set_hand(&mut g, 1, &[id::ESTATE, id::SILVER, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::VILLAIN);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    assert_eq!(choices(&g), vec![Choice::Card(id::SILVER), Choice::Card(id::ESTATE)], "Copper costs less than $2");
    pick(&mut g, id::SILVER);
    // Five Coppers: nothing costs $2, the hand is revealed.
    let mut g = new_state(&[id::VILLAIN], 2);
    set_hand(&mut g, 0, &[id::VILLAIN]);
    set_hand(&mut g, 1, &[id::COPPER; 5]);
    let log = events_of(&mut g, id::VILLAIN);
    assert_eq!(log.iter().filter(|e| matches!(e, Event::Reveal { player: 1, .. })).count(), 5);
    assert_eq!(g.players[1].hand.total(), 5);
}
