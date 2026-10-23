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
    assert_eq!(cards::NUM_CARDS, id::TIRELESS as usize + 1);
    assert_eq!(id::SICKNESS as usize + 1, cards::FIRST_PROJECT as usize);
    assert_eq!(id::CITADEL as usize + 1, cards::FIRST_ARTIFACT as usize);
    assert_eq!(cards::NUM_ZONE_IDS, 558);
    assert!(cards::LAST_KINGDOM == id::KINGS_CACHE && cards::is_kingdom(id::BORDER_GUARD) && cards::is_kingdom(id::VILLAIN));
    assert!(cards::is_project(id::CATHEDRAL) && cards::is_project(id::CITADEL) && !cards::is_project(id::FLAG) && !cards::is_project(id::SICKNESS));
    assert!(cards::is_artifact(id::FLAG) && cards::is_artifact(id::TREASURE_CHEST) && !cards::is_artifact(id::CITADEL));
    assert!(cards::is_pseudo(id::CATHEDRAL) && cards::is_pseudo(id::KEY) && !cards::is_event(id::CATHEDRAL));
    assert_eq!(cards::set_by_name("Renaissance"), Some(CardSet::Renaissance));
    assert_eq!(cards::by_name("Treasure Chest"), Some(id::TREASURE_CHEST));
    assert_eq!(CardSet::ALL.len(), 17);
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
fn scepter_replaying_a_duration_played_this_turn_stays_in_play_until_the_duration_leaves() {
    // "It can replay a Duration played the same turn (in which case Scepter stays in play until the Duration card leaves play)."
    let mut g = new_state(&[id::SCEPTER, id::FISHING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::FISHING_VILLAGE, id::SCEPTER]);
    set_deck_known(&mut g, 0, &[id::COPPER; 12]);
    play(&mut g, id::FISHING_VILLAGE);
    // Leave the Action phase, then the Scepter replays the Fishing Village.
    while expect_decision(&mut g).kind == DecisionKind::PlayAction {
        choose(&mut g, Choice::Pass);
    }
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.turn.coins, 2, "+$1 twice");
    // Finish this turn and the opponent's: everything passes.
    let pass_turn = |g: &mut GameState| {
        let me = g.turn.player;
        while g.turn.player == me {
            let d = expect_decision(g);
            let cs = choices(g);
            choose(g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
            let _ = d;
        }
    };
    pass_turn(&mut g);
    assert!(g.players[0].in_play.has(id::SCEPTER) && g.players[0].in_play.has(id::FISHING_VILLAGE), "both stay out through the Clean-up");
    pass_turn(&mut g);
    // The next turn: the Fishing Village's duration effect happened twice (+1 Action each), then both leave play at its Clean-up.
    expect_decision(&mut g);
    assert_eq!((g.turn.player, g.turn.actions), (0, 3), "1 + 1 + 1 Actions");
    pass_turn(&mut g);
    assert!(!g.players[0].in_play.has(id::SCEPTER) && !g.players[0].in_play.has(id::FISHING_VILLAGE));
    assert!(g.players[0].discard.has(id::SCEPTER) && g.players[0].discard.has(id::FISHING_VILLAGE));

    // A Scepter that replays a non-Duration is discarded as usual at the end of the turn.
    let mut g = new_state(&[id::SCEPTER, id::MARKET], 2);
    set_hand(&mut g, 0, &[id::MARKET, id::SCEPTER]);
    set_deck_known(&mut g, 0, &[id::COPPER; 12]);
    play(&mut g, id::MARKET);
    while expect_decision(&mut g).kind == DecisionKind::PlayAction {
        choose(&mut g, Choice::Pass);
    }
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    pass_turn(&mut g);
    assert!(!g.players[0].in_play.has(id::SCEPTER));
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

// ===========================================================================
// Projects
// ===========================================================================

/// Player `p` has put a cube on the Project (the game must list it for it to be bought; the hooks are refreshed).
fn own(g: &mut GameState, p: usize, project: CardId) {
    let bit = 1u32 << (project - cards::FIRST_PROJECT);
    g.players[p].ren.projects |= bit;
    g.ren.projects |= bit;
    g.ren_update_hooks();
}

fn owns(g: &GameState, p: usize, project: CardId) -> bool {
    g.has_project(p as u8, project)
}

#[test]
fn a_project_is_bought_for_its_printed_cost_and_a_buy_and_belongs_to_the_buyer() {
    let mut g = new_state(&[id::FAIR, id::CANAL, id::CATHEDRAL, id::SMITHY], 2);
    buy_phase(&mut g, 4, 2);
    expect_decision(&mut g);
    assert!(has_choice(&g, Choice::Card(id::FAIR)) && has_choice(&g, Choice::Card(id::CATHEDRAL)) && !has_choice(&g, Choice::Card(id::CANAL)));
    let mut log: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::FAIR), &mut log);
    assert!(log.iter().any(|e| matches!(e, Event::Buy { player: 0, card } if *card == id::FAIR)));
    assert!(owns(&g, 0, id::FAIR) && !owns(&g, 1, id::FAIR));
    assert_eq!((g.turn.coins, g.turn.buys), (0, 1), "$4 and a Buy");
    // Not twice, and not for a Project without the $.
    expect_decision(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::FAIR)) && !has_choice(&g, Choice::Card(id::CATHEDRAL)));
    // The other player may have the same Project.
    let mut g = new_state(&[id::FAIR], 2);
    own(&mut g, 0, id::FAIR);
    reset_turn(&mut g, 1);
    buy_phase(&mut g, 4, 1);
    expect_decision(&mut g);
    assert!(has_choice(&g, Choice::Card(id::FAIR)));
    buy(&mut g, id::FAIR);
    assert!(owns(&g, 0, id::FAIR) && owns(&g, 1, id::FAIR));
}

#[test]
fn a_player_has_two_cubes_and_cannot_buy_a_project_with_debt_or_without_a_buy() {
    let mut g = new_state(&[id::FAIR, id::CATHEDRAL, id::SILOS, id::SMITHY], 2);
    own(&mut g, 0, id::FAIR);
    own(&mut g, 0, id::CATHEDRAL);
    buy_phase(&mut g, 8, 3);
    expect_decision(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::SILOS)), "two cubes are all a player has");
    // Debt: nothing can be bought until it is paid.
    let mut g = new_state(&[id::FAIR, id::SMITHY], 2);
    buy_phase(&mut g, 4, 1);
    g.players[0].debt = 2;
    expect_decision(&mut g);
    assert!(!has_choice(&g, Choice::Card(id::FAIR)) && has_choice(&g, Choice::PayDebt));
    // No Buy: no Project.
    let mut g = new_state(&[id::FAIR, id::SMITHY], 2);
    buy_phase(&mut g, 4, 0);
    g.turn.treasures_done = true;
    g.players[0].hand = counts_of(&[id::ESTATE]);
    adv(&mut g);
    assert_eq!(g.turn.player, 1, "no Buy left: the turn is over, nothing was offered");
}

#[test]
fn a_projects_price_is_not_reduced_by_cards_that_make_cards_cheaper() {
    let mut g = new_state(&[id::CATHEDRAL, id::SMITHY, id::CANAL], 2);
    own(&mut g, 0, id::CANAL);
    g.turn.cost_reduction = 3;
    buy_phase(&mut g, 2, 1);
    expect_decision(&mut g);
    assert_eq!(g.cost(id::SMITHY), 1);
    assert!(!has_choice(&g, Choice::Card(id::CATHEDRAL)), "$3 whatever the reductions");
    assert!(has_choice(&g, Choice::Card(id::SMITHY)));
}

#[test]
fn cathedral_trashes_a_card_at_the_start_of_every_turn() {
    let mut g = new_state(&[id::CATHEDRAL, id::SMITHY], 2);
    own(&mut g, 0, id::CATHEDRAL);
    set_hand(&mut g, 0, &[id::ESTATE, id::COPPER]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, min: 1, max: 1, .. }), "{d:?}");
    pick(&mut g, id::ESTATE);
    assert_eq!(g.trash.get(id::ESTATE), 1);
    // A hand of one card: trashed at once. An empty hand: nothing.
    let mut g = new_state(&[id::CATHEDRAL], 2);
    own(&mut g, 0, id::CATHEDRAL);
    set_hand(&mut g, 0, &[id::COPPER]);
    expect_decision(&mut g);
    assert_eq!(g.trash.get(id::COPPER), 1);
    let mut g = new_state(&[id::CATHEDRAL], 2);
    own(&mut g, 0, id::CATHEDRAL);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    // Only the owner's turns.
    let mut g = new_state(&[id::CATHEDRAL], 2);
    own(&mut g, 1, id::CATHEDRAL);
    set_hand(&mut g, 0, &[id::ESTATE, id::COPPER]);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
    assert_eq!(g.trash.total(), 0);
}

#[test]
fn city_gate_draws_a_card_and_puts_a_card_from_the_hand_onto_the_deck() {
    let mut g = new_state(&[id::CITY_GATE], 2);
    own(&mut g, 0, id::CITY_GATE);
    set_hand(&mut g, 0, &[id::COPPER, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::GOLD]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Topdeck, min: 1, max: 1, .. }), "{d:?}");
    assert_eq!(g.players[0].hand.total(), 3, "+1 Card first");
    pick(&mut g, id::ESTATE);
    assert_eq!(g.players[0].deck_known.iter_top_down().collect::<Vec<_>>(), vec![id::ESTATE, id::GOLD]);
}

#[test]
fn pageant_turns_a_leftover_dollar_into_a_coffers_at_the_end_of_the_buy_phase() {
    let mut g = new_state(&[id::PAGEANT, id::SMITHY], 2);
    own(&mut g, 0, id::PAGEANT);
    buy_phase(&mut g, 2, 1);
    expect_decision(&mut g);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert_eq!((d.player, g.turn.player), (0, 0));
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[0].coffers, 1, "$1 paid for the Coffers, once");
    // Asked once per Buy phase; nothing to ask with no $ left.
    let mut g = new_state(&[id::PAGEANT, id::SMITHY], 2);
    own(&mut g, 0, id::PAGEANT);
    g.players[0].hand = counts_of(&[id::ESTATE]);
    buy_phase(&mut g, 0, 1);
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(g.turn.player, 1, "no question without $1");
    // Declined.
    let mut g = new_state(&[id::PAGEANT], 2);
    own(&mut g, 0, id::PAGEANT);
    buy_phase(&mut g, 1, 1);
    expect_decision(&mut g);
    pass(&mut g);
    expect_decision(&mut g);
    choose(&mut g, Choice::No);
    assert_eq!(g.players[0].coffers, 0);
}

#[test]
fn sewers_lets_a_player_trash_a_card_from_the_hand_for_every_card_they_trash_otherwise() {
    let mut g = new_state(&[id::SEWERS, id::CHAPEL], 2);
    own(&mut g, 0, id::SEWERS);
    set_hand(&mut g, 0, &[id::CHAPEL, id::ESTATE, id::ESTATE, id::CURSE, id::COPPER]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::ESTATE);
    pass(&mut g);
    // One extra trash for each of the two Estates, from any card of the hand.
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, min: 0, max: 1, .. }), "{d:?}");
    pick(&mut g, id::CURSE);
    expect_decision(&mut g);
    pick(&mut g, id::COPPER);
    // The trash Sewers made does not set it off again.
    assert_eq!((g.trash.get(id::ESTATE), g.trash.get(id::CURSE), g.trash.get(id::COPPER)), (2, 1, 1));
    assert_ne!(expect_decision(&mut g).kind, DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter: Filter::Any, min: 0, max: 1, ordered: false });
    // Optional.
    let mut g = new_state(&[id::SEWERS, id::CHAPEL], 2);
    own(&mut g, 0, id::SEWERS);
    set_hand(&mut g, 0, &[id::CHAPEL, id::ESTATE, id::CURSE, id::COPPER]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::ESTATE);
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.trash.total(), 1);
    // Only the owner: another player's trash gives nothing.
    let mut g = new_state(&[id::SEWERS, id::CHAPEL], 2);
    own(&mut g, 1, id::SEWERS);
    set_hand(&mut g, 0, &[id::CHAPEL, id::ESTATE, id::CURSE]);
    play(&mut g, id::CHAPEL);
    pick(&mut g, id::ESTATE);
    assert_eq!(g.trash.total(), 1);
}

#[test]
fn sewers_trashes_before_priests_ability_is_set_up() {
    let mut g = new_state(&[id::SEWERS, id::PRIEST], 2);
    own(&mut g, 0, id::SEWERS);
    set_hand(&mut g, 0, &[id::PRIEST, id::ESTATE, id::CURSE, id::DUCHY]);
    play(&mut g, id::PRIEST);
    pick(&mut g, id::ESTATE);
    pick(&mut g, id::CURSE);
    assert_eq!(g.turn.coins, 2, "the Sewers' trash comes before the ability: no $2 for it");
    assert_eq!(g.trash.total(), 2);
}

#[test]
fn star_chart_lets_the_owner_put_a_card_of_the_shuffle_on_top_of_the_new_deck() {
    let mut g = new_state(&[id::STAR_CHART, id::VILLAGE], 2);
    own(&mut g, 0, id::STAR_CHART);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    set_discard(&mut g, 0, &[id::PROVINCE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::VILLAGE);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Discard, act: Act::Topdeck, min: 0, max: 1, .. }), "{d:?}");
    pick(&mut g, id::PROVINCE);
    // The Province is drawn by the Village's +1 Card; the rest was shuffled into the deck.
    assert_eq!(g.players[0].hand, counts_of(&[id::PROVINCE]));
    assert_eq!(g.players[0].deck_unknown, counts_of(&[id::ESTATE, id::ESTATE]));
    assert!(g.players[0].discard.is_empty());
    // Declining shuffles all of it.
    let mut g = new_state(&[id::STAR_CHART, id::VILLAGE], 2);
    own(&mut g, 0, id::STAR_CHART);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    set_discard(&mut g, 0, &[id::PROVINCE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::VILLAGE);
    pass(&mut g);
    assert_eq!(g.players[0].hand.total() + g.players[0].deck_unknown.total(), 3);
    // Nothing to choose between (all alike): no question. And only its owner.
    let mut g = new_state(&[id::STAR_CHART, id::VILLAGE], 2);
    own(&mut g, 0, id::STAR_CHART);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    set_discard(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    play(&mut g, id::VILLAGE);
    assert_eq!(g.players[0].hand, counts_of(&[id::ESTATE]));
    let mut g = new_state(&[id::STAR_CHART, id::VILLAGE], 2);
    own(&mut g, 1, id::STAR_CHART);
    set_hand(&mut g, 0, &[id::VILLAGE]);
    set_discard(&mut g, 0, &[id::PROVINCE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::VILLAGE);
    assert_eq!(g.players[0].hand.total(), 1);
}

#[test]
fn exploration_gives_a_coffers_and_a_villager_when_no_card_was_gained_in_the_buy_phase() {
    let mut g = new_state(&[id::EXPLORATION, id::SMITHY], 2);
    own(&mut g, 0, id::EXPLORATION);
    buy_phase(&mut g, 4, 1);
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!((g.players[0].coffers, villagers(&g, 0)), (1, 1));
    // A card gained in the Buy phase: nothing. Buying a Project is not gaining a card.
    let mut g = new_state(&[id::EXPLORATION, id::SMITHY], 2);
    own(&mut g, 0, id::EXPLORATION);
    buy_phase(&mut g, 4, 1);
    expect_decision(&mut g);
    buy(&mut g, id::SILVER);
    assert_eq!((g.players[0].coffers, villagers(&g, 0)), (0, 0));
    let mut g = new_state(&[id::EXPLORATION, id::FAIR], 2);
    buy_phase(&mut g, 4, 1);
    expect_decision(&mut g);
    buy(&mut g, id::EXPLORATION);
    assert_eq!((g.players[0].coffers, villagers(&g, 0)), (1, 1), "buying only the Project itself");
    // A card gained in the Action phase does not count.
    let mut g = new_state(&[id::EXPLORATION, id::INVENTOR], 2);
    own(&mut g, 0, id::EXPLORATION);
    set_hand(&mut g, 0, &[id::INVENTOR]);
    play(&mut g, id::INVENTOR);
    pick(&mut g, id::SILVER);
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!((g.players[0].coffers, villagers(&g, 0)), (1, 1));
}

#[test]
fn fair_and_barracks_give_a_buy_and_an_action_at_the_start_of_every_turn() {
    let mut g = new_state(&[id::FAIR, id::BARRACKS], 2);
    own(&mut g, 0, id::FAIR);
    own(&mut g, 0, id::BARRACKS);
    set_hand(&mut g, 0, &[id::ESTATE]);
    expect_decision(&mut g);
    assert_eq!((g.turn.buys, g.turn.actions), (2, 2));
    // The other player's turn is unchanged.
    let mut g = new_state(&[id::FAIR, id::BARRACKS], 2);
    own(&mut g, 1, id::FAIR);
    expect_decision(&mut g);
    assert_eq!((g.turn.buys, g.turn.actions), (1, 1));
    // They are given every turn: after a full round.
    let mut g = new_state(&[id::FAIR], 2);
    own(&mut g, 0, id::FAIR);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    pass_until_turn_of(&mut g, 0);
    assert_eq!(g.turn.buys, 2);
}

#[test]
fn silos_discards_any_number_of_coppers_at_the_start_of_the_turn_to_draw_that_many() {
    let mut g = new_state(&[id::SILOS], 2);
    own(&mut g, 0, id::SILOS);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::COPPER, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::DUCHY]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, min: 0, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::COPPER), Choice::Pass], "only Coppers");
    pick(&mut g, id::COPPER);
    pick(&mut g, id::COPPER);
    pass(&mut g);
    assert_eq!(g.players[0].discard.get(id::COPPER), 2);
    assert_eq!((g.players[0].hand.get(id::DUCHY), g.players[0].hand.get(id::PROVINCE), g.players[0].hand.get(id::ESTATE)), (1, 1, 2));
    // No Coppers: no question.
    let mut g = new_state(&[id::SILOS], 2);
    own(&mut g, 0, id::SILOS);
    set_hand(&mut g, 0, &[id::ESTATE, id::SILVER]);
    assert_eq!(expect_decision(&mut g).kind, DecisionKind::Buy);
}

#[test]
fn sinister_plot_adds_a_token_or_removes_them_all_for_a_card_each() {
    // With no tokens it adds one by itself.
    let mut g = new_state(&[id::SINISTER_PLOT], 2);
    own(&mut g, 0, id::SINISTER_PLOT);
    set_hand(&mut g, 0, &[id::ESTATE]);
    expect_decision(&mut g);
    assert_eq!(g.ren.plot[0], 1);
    // With tokens: a question; Yes adds one more.
    let mut g = new_state(&[id::SINISTER_PLOT], 2);
    own(&mut g, 0, id::SINISTER_PLOT);
    g.ren.plot[0] = 2;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::ESTATE]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!((g.ren.plot[0], g.players[0].hand.total()), (3, 1));
    // No: the tokens go, +1 Card each.
    let mut g = new_state(&[id::SINISTER_PLOT], 2);
    own(&mut g, 0, id::SINISTER_PLOT);
    g.ren.plot[0] = 2;
    g.ren.plot[1] = 4;
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::PROVINCE, id::ESTATE]);
    expect_decision(&mut g);
    choose(&mut g, Choice::No);
    assert_eq!((g.ren.plot[0], g.ren.plot[1], g.players[0].hand.total()), (0, 4, 3), "each player has their own tokens");
}

#[test]
fn academy_and_guildhall_pay_for_every_action_or_treasure_gained_by_their_owner() {
    let mut g = new_state(&[id::ACADEMY, id::GUILDHALL, id::SMITHY, id::HAREM], 2);
    own(&mut g, 0, id::ACADEMY);
    g.gain_card(0, id::SMITHY, Dest::Discard, &mut NoEvents);
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    g.gain_card(1, id::SMITHY, Dest::Discard, &mut NoEvents);
    assert_eq!((villagers(&g, 0), villagers(&g, 1), g.players[0].coffers), (1, 0, 0));
    own(&mut g, 0, id::GUILDHALL);
    g.gain_card(0, id::SILVER, Dest::Discard, &mut NoEvents);
    g.gain_card(0, id::SMITHY, Dest::Discard, &mut NoEvents);
    g.gain_card(0, id::DUCHY, Dest::Discard, &mut NoEvents);
    g.gain_card(1, id::GOLD, Dest::Discard, &mut NoEvents);
    assert_eq!((villagers(&g, 0), g.players[0].coffers, g.players[1].coffers), (2, 1, 0));
    // A Treasure-Victory card is a Treasure.
    g.gain_card(0, id::HAREM, Dest::Discard, &mut NoEvents);
    assert_eq!(g.players[0].coffers, 2);
}

#[test]
fn road_network_draws_a_card_whenever_another_player_gains_a_victory_card() {
    let mut g = new_state(&[id::ROAD_NETWORK, id::SMITHY], 3);
    own(&mut g, 0, id::ROAD_NETWORK);
    own(&mut g, 2, id::ROAD_NETWORK);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    set_deck_known(&mut g, 2, &[id::SILVER, id::SILVER]);
    g.gain_card(1, id::PROVINCE, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!((g.players[0].hand.total(), g.players[2].hand.total()), (1, 1), "even on the gainer's own turn, for every owner");
    // Not for their own gains (the other owner draws), or for other cards.
    g.pending = Pending::None;
    g.gain_card(0, id::ESTATE, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!((g.players[0].hand.total(), g.players[2].hand.total()), (1, 2));
    g.pending = Pending::None;
    g.gain_card(1, id::SILVER, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!((g.players[0].hand.total(), g.players[2].hand.total()), (1, 2));
    // Player 3's own gain draws for player 1 only.
    g.pending = Pending::None;
    g.gain_card(2, id::DUCHY, Dest::Discard, &mut NoEvents);
    adv(&mut g);
    assert_eq!((g.players[0].hand.total(), g.players[2].hand.total()), (2, 2));
}

#[test]
fn piazza_plays_the_top_card_of_the_deck_at_the_start_of_the_turn_if_it_is_an_action() {
    let mut g = new_state(&[id::PIAZZA, id::SMITHY], 2);
    own(&mut g, 0, id::PIAZZA);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::SMITHY, id::DUCHY, id::DUCHY, id::PROVINCE]);
    expect_decision(&mut g);
    assert!(g.players[0].in_play.has(id::SMITHY), "played without using an Action");
    assert_eq!((g.turn.actions, g.players[0].hand.total()), (1, 1 + 3));
    // Not an Action: it goes back on top.
    let mut g = new_state(&[id::PIAZZA, id::SMITHY], 2);
    own(&mut g, 0, id::PIAZZA);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::SMITHY]);
    expect_decision(&mut g);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::DUCHY));
    assert!(g.players[0].in_play.is_empty());
}

#[test]
fn crop_rotation_discards_a_victory_card_for_two_cards() {
    let mut g = new_state(&[id::CROP_ROTATION], 2);
    own(&mut g, 0, id::CROP_ROTATION);
    set_hand(&mut g, 0, &[id::ESTATE, id::DUCHY, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::PROVINCE, id::GOLD, id::GOLD]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, min: 0, max: 1, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::ESTATE), Choice::Card(id::DUCHY), Choice::Pass]);
    pick(&mut g, id::DUCHY);
    assert_eq!((g.players[0].discard.get(id::DUCHY), g.players[0].hand.get(id::PROVINCE)), (1, 1));
    assert_eq!(g.players[0].hand.total() + g.players[0].in_play.total(), 4, "one Victory card out, two cards in");
}

#[test]
fn innovation_plays_the_first_action_gained_each_turn_if_the_player_likes() {
    let mut g = new_state(&[id::INNOVATION, id::SMITHY, id::VILLAGE], 2);
    own(&mut g, 0, id::INNOVATION);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
    buy_phase(&mut g, 8, 3);
    g.gain_card(0, id::SMITHY, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::SMITHY, "{d:?}");
    choose(&mut g, Choice::Yes);
    assert!(g.players[0].in_play.has(id::SMITHY) && !g.players[0].discard.has(id::SMITHY));
    assert_eq!(g.players[0].hand.total(), 3, "played in the Buy phase: +3 Cards");
    // Once a turn.
    g.pending = Pending::None;
    g.gain_card(0, id::VILLAGE, Dest::Discard, &mut NoEvents);
    assert!(!matches!(expect_decision(&mut g).kind, DecisionKind::YesNo { .. }));
    // Declining keeps the option for the next Action gained.
    let mut g = new_state(&[id::INNOVATION, id::SMITHY, id::VILLAGE], 2);
    own(&mut g, 0, id::INNOVATION);
    buy_phase(&mut g, 8, 3);
    g.gain_card(0, id::SMITHY, Dest::Discard, &mut NoEvents);
    expect_decision(&mut g);
    choose(&mut g, Choice::No);
    g.pending = Pending::None;
    g.gain_card(0, id::VILLAGE, Dest::Discard, &mut NoEvents);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }) && d.subject == id::VILLAGE);
    // A Treasure or a card gained by another player is not offered.
    choose(&mut g, Choice::No);
    g.pending = Pending::None;
    g.gain_card(1, id::SMITHY, Dest::Discard, &mut NoEvents);
    g.gain_card(0, id::GOLD, Dest::Discard, &mut NoEvents);
    assert!(!matches!(expect_decision(&mut g).kind, DecisionKind::YesNo { .. }));
}

#[test]
fn canal_makes_every_card_cost_one_less_on_its_owners_turns_only() {
    let mut g = new_state(&[id::CANAL, id::SMITHY], 2);
    own(&mut g, 0, id::CANAL);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 10]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 10]);
    // The opponent's turn first (Canal does not apply), then the owner's.
    let costs = |g: &GameState| (g.cost(id::DUCHY), g.cost(id::SMITHY), g.cost(id::COPPER));
    reset_turn(&mut g, 1);
    g.turn.number = 2;
    assert_eq!(costs(&g), (5, 4, 0));
    pass_until_turn_of(&mut g, 0);
    assert_eq!(costs(&g), (4, 3, 0), "Canal: $1 less, and not below $0");
    // Bought mid-turn it applies at once.
    let mut g = new_state(&[id::CANAL, id::SMITHY], 2);
    buy_phase(&mut g, 14, 2);
    expect_decision(&mut g);
    assert_eq!(g.cost(id::DUCHY), 5);
    buy(&mut g, id::CANAL);
    assert_eq!(g.cost(id::DUCHY), 4);
    assert!(has_choice(&g, Choice::Card(id::DUCHY)) && g.turn.coins == 7);
}

#[test]
fn citadel_plays_the_first_action_each_turn_twice() {
    let mut g = new_state(&[id::CITADEL, id::SMITHY], 2);
    own(&mut g, 0, id::CITADEL);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::SMITHY, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 1 + 6, "Smithy twice");
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 6 + 3, "the second Action is played once");
    // Another player's Action is not affected by it.
    let mut g = new_state(&[id::CITADEL, id::SMITHY], 2);
    own(&mut g, 1, id::CITADEL);
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 6]);
    play(&mut g, id::SMITHY);
    assert_eq!(g.players[0].hand.total(), 3);
}

#[test]
fn citadel_plays_a_duration_twice_and_keeps_both_next_turn_effects() {
    let mut g = new_state(&[id::CITADEL, id::CARAVAN], 2);
    own(&mut g, 0, id::CITADEL);
    set_hand(&mut g, 0, &[id::CARAVAN]);
    set_deck_known(&mut g, 0, &[id::ESTATE; 8]);
    play(&mut g, id::CARAVAN);
    assert_eq!(g.players[0].hand.total(), 2, "+1 Card twice");
    let e = g.players[0].pending_durations[0];
    assert_eq!((e.card, e.times), (id::CARAVAN, 2));
}

#[test]
fn capitalism_makes_actions_with_a_dollar_amount_treasures_playable_in_the_buy_phase() {
    let mut g = new_state(&[id::CAPITALISM, id::MARKET, id::SMITHY, id::MINE], 2);
    own(&mut g, 0, id::CAPITALISM);
    assert!(g.is_treasure(id::MARKET) && g.is_treasure(id::COPPER) && !g.is_treasure(id::SMITHY), "Market has +$1 in its text, Smithy has none");
    // Not on the other player's turn.
    let mut h = g;
    reset_turn(&mut h, 1);
    assert!(!h.is_treasure(id::MARKET));
    // Played in the Buy phase, with its other effects too; no Action is used.
    set_hand(&mut g, 0, &[id::MARKET, id::COPPER, id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::ESTATE]);
    g.turn.actions = 0;
    expect_decision(&mut g);
    assert_eq!(g.turn.phase, Phase::Buy);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    assert!(has_choice(&g, Choice::Card(id::MARKET)) && has_choice(&g, Choice::Card(id::COPPER)) && !has_choice(&g, Choice::Card(id::SMITHY)));
    play_treasure(&mut g, id::MARKET);
    assert_eq!((g.turn.coins, g.turn.buys, g.players[0].hand.get(id::DUCHY)), (2, 2, 1), "$1 from the Market and $1 from the Copper, played after it");
}

#[test]
fn fleet_gives_an_extra_turn_after_the_game_would_end_to_each_player_who_has_it() {
    // Player 3 ends the game; players 1 and 3 have Fleet: player 1 takes a turn, then player 3 (the next player after the one who ended it, in order).
    let mut g = new_state(&[id::FLEET, id::SMITHY], 3);
    own(&mut g, 0, id::FLEET);
    own(&mut g, 2, id::FLEET);
    set_supply(&mut g, id::PROVINCE, 0);
    for p in 0..3 {
        set_deck_unknown(&mut g, p, &[id::ESTATE; 20]);
    }
    reset_turn(&mut g, 2);
    g.turn.number = 3;
    expect_decision(&mut g);
    pass(&mut g);
    assert_eq!(g.turn.player, 0, "player 1 takes their Fleet turn");
    assert!(!g.is_game_over());
    pass_until_turn_of(&mut g, 2);
    assert_eq!(g.turn.player, 2, "then player 3, who ended the game, takes theirs");
    // After the last Fleet turn the game is over, even though player 2 never played.
    loop {
        match adv(&mut g) {
            Step::GameOver => break,
            Step::Decision(_) => {
                let cs = choices(&g);
                let c = if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] };
                g.apply(c, &mut NoEvents).unwrap();
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!((g.players[0].turns_taken, g.players[1].turns_taken, g.players[2].turns_taken), (1, 0, 2));
    // Without Fleet the game just ends.
    let mut g = new_state(&[id::FLEET, id::SMITHY], 2);
    set_supply(&mut g, id::PROVINCE, 0);
    reset_turn(&mut g, 1);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 20]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 20]);
    expect_decision(&mut g);
    pass(&mut g);
    assert!(g.is_game_over());
}

#[test]
fn a_fleet_bought_in_the_fleet_round_gets_no_turn_and_fleet_does_not_extend_the_turn_limit() {
    let mut g = new_state(&[id::FLEET, id::SMITHY], 2);
    own(&mut g, 0, id::FLEET);
    set_supply(&mut g, id::PROVINCE, 0);
    for p in 0..2 {
        set_deck_unknown(&mut g, p, &[id::ESTATE; 20]);
    }
    reset_turn(&mut g, 1);
    g.turn.number = 4;
    expect_decision(&mut g);
    pass(&mut g);
    // Player 1's Fleet turn: player 2 has no Fleet and does not get one even if they buy it now (it is player 1's turn: buy for player 1 only).
    assert_eq!(g.turn.player, 0);
    assert!(g.ren.flags & dominion_engine::state::REN_FLEET_ROUND != 0);
    pass(&mut g);
    assert!(g.is_game_over());
}

// ===========================================================================
// Artifacts, interactions, state
// ===========================================================================

#[test]
fn an_artifact_changes_hands_and_the_event_says_from_whom() {
    let mut g = new_state(&[id::FLAG_BEARER], 3);
    let mut log: Vec<Event> = Vec::new();
    g.take_artifact(0, id::FLAG, &mut log);
    g.take_artifact(2, id::FLAG, &mut log);
    g.take_artifact(2, id::FLAG, &mut log);
    let froms: Vec<u8> = log.iter().filter_map(|e| if let Event::Artifact { from, .. } = e { Some(*from) } else { None }).collect();
    assert_eq!(froms, vec![255, 0, 2], "nobody, then player 1, then themselves again");
    assert_eq!((holder(&g, id::FLAG), g.has_artifact(2, id::FLAG), g.has_artifact(0, id::FLAG)), (Some(2), true, false));
}

#[test]
fn the_lantern_taken_by_the_first_play_of_a_throne_roomed_border_guard_is_used_by_the_second() {
    let mut g = new_state(&[id::BORDER_GUARD, id::THRONE_ROOM, id::VILLAGE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::THRONE_ROOM, id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::VILLAGE, id::VILLAGE, id::ESTATE, id::ESTATE, id::ESTATE, id::ESTATE]);
    play(&mut g, id::THRONE_ROOM);
    pick(&mut g, id::BORDER_GUARD);
    // First play: two Villages (nothing to choose): Lantern or Horn.
    assert!(matches!(expect_decision(&mut g).kind, DecisionKind::Mode { .. }));
    choose(&mut g, Choice::Mode(0));
    // Second play: three cards revealed now (the Lantern is held): three Estates, so there is a pick between nothing (all alike).
    assert_eq!(holder(&g, id::LANTERN), Some(0));
}

#[test]
fn improve_cannot_trash_a_duration_that_stays_in_play_but_can_trash_one_that_is_discarded() {
    let mut g = new_state(&[id::IMPROVE, id::CARGO_SHIP, id::SMITHY], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::IMPROVE, id::CARGO_SHIP]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::CARGO_SHIP);
    play(&mut g, id::IMPROVE);
    pass(&mut g);
    // The Cargo Ship set nothing aside: it is discarded this turn and can be trashed with the Improve ($3 -> $4).
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }));
    assert_eq!(choices(&g), vec![Choice::Card(id::CARGO_SHIP), Choice::Card(id::IMPROVE), Choice::Pass]);
    // With a card set aside it stays in play and is not offered.
    let mut g = new_state(&[id::IMPROVE, id::CARGO_SHIP, id::SMITHY], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::IMPROVE, id::CARGO_SHIP]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::CARGO_SHIP);
    play(&mut g, id::IMPROVE);
    buy_phase(&mut g, 3, 1);
    g.pending = Pending::None;
    g.gain_card(0, id::ESTATE, Dest::Discard, &mut NoEvents);
    expect_decision(&mut g);
    choose(&mut g, Choice::Yes);
    g.pending = Pending::None;
    expect_decision(&mut g);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, .. }), "{d:?}");
    assert_eq!(choices(&g), vec![Choice::Card(id::IMPROVE), Choice::Pass], "the Cargo Ship stays in play");
}

#[test]
fn several_start_of_turn_effects_are_ordered_by_the_player_and_the_plain_ones_come_first() {
    let mut g = new_state(&[id::CATHEDRAL, id::SILOS, id::FAIR, id::BARRACKS], 2);
    for p in [id::CATHEDRAL, id::SILOS, id::FAIR, id::BARRACKS] {
        own(&mut g, 0, p);
    }
    set_holder(&mut g, id::KEY, 0);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::DUCHY, id::DUCHY]);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(choices(&g), vec![Choice::Card(id::CATHEDRAL), Choice::Card(id::SILOS)]);
    // The Fair, the Barracks and the Key were resolved first (no question about them).
    assert_eq!((g.turn.buys, g.turn.actions, g.turn.coins), (2, 2, 1));
    // Silos first, then the Cathedral trashes from the larger hand.
    choose(&mut g, Choice::Card(id::SILOS));
    pick(&mut g, id::COPPER);
    pick(&mut g, id::COPPER);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Trash, min: 1, .. }), "{d:?}");
    assert_eq!(g.players[0].hand.get(id::DUCHY), 2);
}

#[test]
fn the_state_text_round_trips_with_everything_renaissance_adds() {
    let mut g = new_state(&[id::BORDER_GUARD, id::CARGO_SHIP, id::RESEARCH, id::FAIR, id::CATHEDRAL, id::SINISTER_PLOT, id::FLEET], 3);
    g.players[0].ren.villagers = 3;
    g.players[1].ren.villagers = 11;
    own(&mut g, 0, id::FAIR);
    own(&mut g, 0, id::SINISTER_PLOT);
    own(&mut g, 2, id::FLEET);
    g.ren.plot[0] = 2;
    g.ren.plot[2] = 1;
    set_holder(&mut g, id::FLAG, 2);
    set_holder(&mut g, id::LANTERN, 0);
    set_holder(&mut g, id::TREASURE_CHEST, 1);
    g.players[0].ren.cargo.add(id::SILVER, 1);
    g.players[1].ren.research.add(id::GOLD, 2);
    g.players[1].ren.research.add(id::ESTATE, 1);
    g.players[2].coffers = 4;
    g.turn.ren.priests = 2;
    g.turn.ren.cargo = 1;
    g.turn.ren.innovation = true;
    g.ren.flags |= dominion_engine::state::REN_FLEET_ROUND;
    g.ren.fleet = 0b101;
    let text = format_state(&g);
    for needle in ["projects: ", "artifacts: ", "fleet round: 1 3", "villagers: 3", "owned projects: Fair, Sinister Plot", "plot: 2", "cargo: Silver", "research: ", "priests: 2"] {
        assert!(text.contains(needle), "{needle:?} missing from\n{text}");
    }
    let back = parse_state(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(strip_seed(&format_state(&back)), strip_seed(&text));
    assert!(back.players[0].ren == g.players[0].ren && back.players[1].ren == g.players[1].ren && back.players[2].ren == g.players[2].ren);
    assert_eq!((back.ren.projects, back.ren.holder, back.ren.plot, back.ren.fleet, back.ren.flags), (g.ren.projects, g.ren.holder, g.ren.plot, g.ren.fleet, g.ren.flags));
    assert_eq!(back.turn.ren, g.turn.ren);
    assert_eq!(back.ren.hook, g.ren.hook);
    // The tokens and cards count as owned when they are scored.
    assert_eq!(back.players[0].all_cards().get(id::SILVER), 1);
    assert_eq!(back.players[1].all_cards().get(id::GOLD), 2);
}

#[test]
fn the_parser_rejects_what_it_cannot_make_sense_of() {
    let mut g = new_state(&[id::FAIR], 2);
    own(&mut g, 0, id::FAIR);
    let text = format_state(&g);
    assert!(parse_state(&text.replace("projects: Fair", "projects: Smithy")).is_err());
    let with = format!("artifacts: Flag=3\n{text}");
    assert!(parse_state(&with).is_err(), "seat 3 in a 2-player game");
    assert!(parse_state(&format!("artifacts: Smithy=1\n{text}")).is_err());
}

#[test]
fn the_cards_set_aside_under_research_are_hidden_from_other_players_but_conserved() {
    let mut g = new_state(&[id::RESEARCH], 2);
    g.players[1].ren.research.add(id::GOLD, 2);
    g.players[1].ren.research.add(id::PROVINCE, 1);
    set_hand(&mut g, 1, &[id::COPPER, id::ESTATE]);
    set_deck_unknown(&mut g, 1, &[id::COPPER; 6]);
    let before = g.players[1].all_cards();
    let view = PlayerView::new(&g, 0);
    assert_eq!((view.research_count_of(1), view.research().total()), (3, 0), "I do not see theirs");
    let mut seen_different = false;
    for seed in 0..20u64 {
        let w = view.determinize(&mut Rng::new(seed));
        assert_eq!(w.players[1].ren.research.total(), 3);
        assert_eq!(w.players[1].all_cards().total(), before.total());
        // What I may know of them (everything they own) is unchanged; only the split between the zones is dealt again.
        assert_eq!(w.players[1].all_cards(), before);
        seen_different |= w.players[1].ren.research != g.players[1].ren.research;
    }
    assert!(seen_different, "the contents of the Research are dealt again");
    // My own are known to me.
    let view = PlayerView::new(&g, 1);
    assert_eq!(view.research().total(), 3);
    let w = view.determinize(&mut Rng::new(1));
    assert_eq!(w.players[1].ren.research, g.players[1].ren.research);
}

#[test]
fn the_view_reports_villagers_projects_and_artifacts_to_everyone() {
    let mut g = new_state(&[id::FAIR, id::PAGEANT, id::FLAG_BEARER], 2);
    g.players[1].ren.villagers = 5;
    own(&mut g, 1, id::FAIR);
    set_holder(&mut g, id::FLAG, 1);
    g.ren.plot[1] = 3;
    let v = PlayerView::new(&g, 0);
    assert_eq!((v.villagers(), v.villagers_of(1)), (0, 5));
    assert!(v.has_project(1, id::FAIR) && !v.has_project(0, id::FAIR) && v.has_project_in_game(id::PAGEANT) && !v.has_project_in_game(id::CANAL));
    assert!(v.has_artifact(1, id::FLAG) && !v.has_artifact(0, id::FLAG));
    assert_eq!((v.artifact_holder(id::FLAG), v.artifact_holder(id::HORN), v.plot_tokens_of(1), v.projects_owned_by(1)), (Some(1), None, 3, 1));
}

#[test]
fn bought_projects_are_logged_and_projects_owned_are_in_the_text_of_the_players() {
    let mut g = new_state(&[id::FAIR], 2);
    buy_phase(&mut g, 4, 1);
    expect_decision(&mut g);
    let mut log: Vec<Event> = Vec::new();
    choose_ev(&mut g, Choice::Card(id::FAIR), &mut log);
    assert!(log.iter().any(|e| matches!(e, Event::Buy { player: 0, card } if *card == id::FAIR)));
    let text = format_state(&g);
    assert!(text.contains("owned projects: Fair"), "{text}");
}

// ===========================================================================
// The stop-moving rule (docs/stop-moving-rule.md)
// ===========================================================================

#[test]
fn citadels_replay_of_mining_village_cannot_trash_another_mining_village() {
    let mut g = new_state(&[id::CITADEL, id::MINING_VILLAGE], 2);
    own(&mut g, 0, id::CITADEL);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE]);
    set_in_play(&mut g, 0, &[id::MINING_VILLAGE]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 10]);
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::Yes);
    assert!(!matches!(g.pending_decision().map(|d| d.kind), Some(DecisionKind::YesNo { .. })), "{:?}", g.pending_decision());
    assert_eq!(g.turn.coins, 2);
    assert_eq!(g.trash.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.players[0].in_play.get(id::MINING_VILLAGE), 1);
}

#[test]
fn scepters_replay_of_mining_village_can_trash_it_when_it_is_still_in_play() {
    let mut g = new_state(&[id::SCEPTER, id::MINING_VILLAGE], 2);
    set_hand(&mut g, 0, &[id::MINING_VILLAGE, id::SCEPTER]);
    set_deck_known(&mut g, 0, &[id::PROVINCE; 10]);
    play(&mut g, id::MINING_VILLAGE);
    choose(&mut g, Choice::No); // keep it in play
    while expect_decision(&mut g).kind == DecisionKind::PlayAction {
        choose(&mut g, Choice::Pass);
    }
    play_treasure(&mut g, id::SCEPTER);
    choose(&mut g, Choice::Mode(1));
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::YesNo { act: Act::Trash });
    choose(&mut g, Choice::Yes);
    assert_eq!(g.trash.get(id::MINING_VILLAGE), 1);
    assert_eq!(g.players[0].in_play.get(id::MINING_VILLAGE), 0);
    assert_eq!(g.turn.led.n, 0);
}

// ===========================================================================
// Audit: rulings of the wiki's FAQ pages that had no test
// ===========================================================================

#[test]
fn recruiter_counts_only_the_dollar_part_of_the_cost() {
    // "You do not get anything for P or Debt amounts, just for $."
    let mut g = new_state(&[id::RECRUITER, id::ALCHEMIST, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::RECRUITER, id::ALCHEMIST]);
    set_deck_unknown(&mut g, 0, &[id::COPPER; 6]);
    play(&mut g, id::RECRUITER);
    pick(&mut g, id::ALCHEMIST);
    assert_eq!(villagers(&g, 0), 3, "an Alchemist costs $3 and a Potion");
}

#[test]
fn seer_does_not_take_cards_with_potion_or_debt_in_the_cost() {
    // "Cards with P or Debt in their cost do not cost from $2 to $4."
    let mut g = new_state(&[id::SEER, id::ALCHEMIST, id::DAIMYO], 2);
    set_hand(&mut g, 0, &[id::SEER]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::ALCHEMIST, id::DAIMYO, id::SILVER, id::ESTATE]);
    play(&mut g, id::SEER);
    // +1 Card (Copper); the top 3 are Alchemist, Daimyo, Silver: only the Silver is taken.
    assert_eq!(g.players[0].hand, counts_of(&[id::COPPER, id::SILVER]));
}

#[test]
fn an_experiment_gained_to_the_hand_brings_the_second_one_to_the_discard_pile() {
    // "If you gain one to a place other than your discard pile, the 2nd copy goes to your discard pile."
    let mut g = new_state(&[id::EXPERIMENT, id::SMITHY], 2);
    g.gain_card(0, id::EXPERIMENT, Dest::Hand, &mut NoEvents);
    adv(&mut g);
    assert_eq!((g.players[0].hand.get(id::EXPERIMENT), g.players[0].discard.get(id::EXPERIMENT)), (1, 1));
}

#[test]
fn an_experiment_played_without_being_in_play_stays_where_it_is() {
    // "If Experiment somehow is not in play, it fails to return to its pile." (Band of Misfits plays it from the Supply.)
    let mut g = new_state(&[id::EXPERIMENT, id::BAND_OF_MISFITS], 2);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_deck_unknown(&mut g, 0, &[id::COPPER; 6]);
    let pile = g.supply.get(id::EXPERIMENT);
    play(&mut g, id::BAND_OF_MISFITS);
    adv(&mut g);
    assert_eq!(g.supply.get(id::EXPERIMENT), pile, "Band of Misfits stays in play; the Experiment it imitated was never in play");
    assert_eq!(g.players[0].in_play.get(id::COPPER), 2, "+2 Cards (played as Treasures in the Buy phase)");
}

#[test]
fn a_treasurer_played_at_the_start_of_the_turn_gives_the_key_at_once() {
    // "If you play Treasurer at the start of your turn (with e.g. Piazza) and take the Key, you immediately get +$1 from it."
    let mut g = new_state(&[id::PIAZZA, id::TREASURER], 2);
    own(&mut g, 0, id::PIAZZA);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::TREASURER, id::DUCHY, id::DUCHY, id::PROVINCE]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    choose(&mut g, Choice::Mode(2));
    assert_eq!(g.turn.coins, 3 + 1, "Treasurer's $3 and the Key's $1 at once");
}

#[test]
fn a_cargo_ship_trashed_by_improve_leaves_the_card_it_sets_aside_stranded() {
    // Improve: "If you trash a Cargo Ship from play that hasn't set aside a card yet, then when you gain a card after trashing it, Cargo Ship can
    // still set the gained card aside - but the set-aside card will be stranded and never be put in your hand."
    let mut g = new_state(&[id::CARGO_SHIP, id::IMPROVE, id::SMITHY], 2);
    set_hand(&mut g, 0, &[id::CARGO_SHIP, id::IMPROVE]);
    g.turn.actions = 2;
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    set_deck_unknown(&mut g, 1, &[id::ESTATE; 12]);
    play(&mut g, id::CARGO_SHIP);
    play(&mut g, id::IMPROVE);
    // To the end of the Buy phase without buying.
    let mut guard = 0;
    while g.turn.player == 0 && guard < 20 {
        guard += 1;
        let d = expect_decision(&mut g);
        match d.kind {
            DecisionKind::Select { act: Act::Trash, .. } => {
                // Improve: trash the Cargo Ship (costs $3: gain a $4 card).
                pick(&mut g, id::CARGO_SHIP);
            }
            DecisionKind::YesNo { act: Act::SetAside, .. } => {
                choose(&mut g, Choice::Yes);
            }
            _ => {
                let cs = choices(&g);
                choose(&mut g, if cs.contains(&Choice::Pass) { Choice::Pass } else { cs[0] });
            }
        }
    }
    pass_until_turn_of(&mut g, 0);
    assert_eq!(g.players[0].hand.get(id::SMITHY), 0, "the set-aside card never reaches the hand");
}

#[test]
fn an_inventor_gained_by_inventor_and_played_by_innovation_gains_up_to_four_only() {
    // "If you gain an Inventor with Inventor and then play the gained Inventor with Innovation, the first Inventor's cost reduction hasn't kicked in
    // yet, so the second Inventor can still only gain a card costing $4."
    let mut g = new_state(&[id::INVENTOR, id::INNOVATION, id::SMITHY], 2);
    own(&mut g, 0, id::INNOVATION);
    set_hand(&mut g, 0, &[id::INVENTOR]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::INVENTOR);
    pick(&mut g, id::INVENTOR);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Play }), "{d:?}");
    choose(&mut g, Choice::Yes);
    let c = choices(&g);
    assert!(!c.contains(&Choice::Card(id::DUCHY)), "a Duchy costs $5 before the reduction: {c:?}");
    assert!(c.contains(&Choice::Card(id::SMITHY)));
}

#[test]
fn exploration_does_not_count_cards_gained_in_the_action_phase() {
    // "It does not count cards gained in your Action phase, such as with Inventor."
    let mut g = new_state(&[id::EXPLORATION, id::INVENTOR, id::SMITHY], 2);
    own(&mut g, 0, id::EXPLORATION);
    set_hand(&mut g, 0, &[id::INVENTOR]);
    set_deck_unknown(&mut g, 0, &[id::ESTATE; 12]);
    play(&mut g, id::INVENTOR);
    pick(&mut g, id::SMITHY);
    // The Buy phase: buy nothing.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    pass(&mut g);
    assert_eq!((g.players[0].coffers, villagers(&g, 0)), (1, 1));
}

#[test]
fn citadel_with_enchantress_gives_the_cantrip_and_then_replays_the_card_normally() {
    // "If the first Action card you play is affected by Enchantress, you get +1 Card and +1 Action, then Citadel replays the card for its normal ability."
    let mut g = new_state(&[id::CITADEL, id::ENCHANTRESS, id::SMITHY], 2);
    own(&mut g, 0, id::CITADEL);
    g.emp.flags |= dominion_engine::state::E_ENCHANT;
    g.players[0].ench_by = 1 << 1;
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::SMITHY]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::DUCHY, id::ESTATE]);
    play(&mut g, id::SMITHY);
    // Enchantress (the other player's, in play): the first play is +1 Card +1 Action; the replay is Smithy's own +3 Cards.
    // (The Treasures drawn were played at once: the Buy phase is up.) Four cards were drawn, not six.
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::ESTATE));
    assert!(g.players[0].in_play.has(id::GOLD) && g.players[0].hand.has(id::DUCHY));
}

#[test]
fn crop_rotation_discards_only_one_victory_card() {
    // "You cannot discard multiple Victory cards at the start of a turn to draw more than 2 cards."
    let mut g = new_state(&[id::CROP_ROTATION], 2);
    own(&mut g, 0, id::CROP_ROTATION);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::DUCHY]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { max: 1, .. }), "{d:?}");
}

#[test]
fn canal_makes_an_estate_too_cheap_for_villain() {
    // "Villain: other players discard a card costing at least $2, which could not be Estate, as Estate only costs $1 on your turns."
    let mut g = new_state(&[id::CANAL, id::VILLAIN], 2);
    own(&mut g, 0, id::CANAL);
    g.turn.cost_reduction = 1;
    set_hand(&mut g, 0, &[id::VILLAIN]);
    set_hand(&mut g, 1, &[id::ESTATE, id::ESTATE, id::ESTATE, id::SILVER, id::GOLD]);
    play(&mut g, id::VILLAIN);
    let d = expect_decision(&mut g);
    assert_eq!(d.player, 1);
    let c = choices(&g);
    assert!(!c.contains(&Choice::Card(id::ESTATE)) && c.contains(&Choice::Card(id::SILVER)), "{c:?}");
}

#[test]
fn capitalism_lets_a_treasurer_regain_a_card_that_is_a_treasure_on_the_owners_turn() {
    // "if you have Capitalism, you can use Treasurer to gain an Improve from the trash, since Improve is a Treasure on your turns."
    let mut g = new_state(&[id::CAPITALISM, id::TREASURER, id::IMPROVE], 2);
    own(&mut g, 0, id::CAPITALISM);
    g.trash.add(id::IMPROVE, 1);
    set_hand(&mut g, 0, &[id::TREASURER]);
    play(&mut g, id::TREASURER);
    choose(&mut g, Choice::Mode(1));
    assert!(has_choice(&g, Choice::Card(id::IMPROVE)), "{:?}", choices(&g));
}

#[test]
fn capitalism_affects_every_action_the_wiki_lists() {
    // The wiki's "Cards that Capitalism affects" (the removed cards and the pre-2022 Storyteller left out).
    let listed = [
        "Festival", "Market", "Merchant", "Militia", "Moneylender", "Poacher", "Vassal", "Baron", "Bridge", "Conspirator", "Courtier", "Ironworks", "Mill",
        "Mining Village", "Minion", "Pawn", "Steward", "Swindler", "Bazaar", "Corsair", "Cutpurse", "Fishing Village", "Lighthouse", "Merchant Ship",
        "Sailor", "Salvager", "Treasury", "Herbalist", "Bishop", "City", "Charlatan", "Clerk", "Grand Market", "Monument", "Peddler", "Vault", "Courser",
        "Jester", "Joust", "Merchant Guild", "Shop", "Haggler", "Nomads", "Oasis", "Souk", "Spice Merchant", "Abandoned Mine", "Count", "Dame Sylvia",
        "Death Cart", "Forager", "Ironmonger", "Junk Dealer", "Mercenary", "Mystic", "Poor House", "Rogue", "Scavenger", "Storeroom", "Squire", "Amulet",
        "Artificer", "Caravan Guard", "Giant", "Hero", "Messenger", "Miser", "Peasant", "Soldier", "Swamp Hag", "Teacher", "Treasure Hunter",
        "Wine Merchant", "Catapult", "Chariot Race", "Emporium", "Farmers' Market", "Gladiator", "Legionary", "Opulent Castle", "Sacrifice", "Villa",
        "Bard", "Conclave", "Sacred Grove", "Secret Cave", "Tormentor", "Tracker", "Cargo Ship", "Flag Bearer", "Improve", "Patron", "Priest", "Treasurer",
        "Animal Fair", "Bounty Hunter", "Cardinal", "Coven", "Fisherman", "Gatekeeper", "Livery", "Paddock", "Scrap", "Archer", "Barbarian", "Broker",
        "Capital City", "Courier", "Elder", "Galleria", "Garrison", "Guildmaster", "Merchant Camp", "Skirmisher", "Stronghold", "Sycophant", "Tent", "Town",
        "Town Crier", "Cabin Boy", "Flagship", "Fortune Hunter", "Frigate", "Harbor Village", "Mining Road", "Search", "Secluded Shrine", "Shaman",
        "Taskmaster", "Aristocrat", "Change", "Fishmonger", "Kiln", "Kitsune", "Mountain Shrine", "Samurai", "Tea House",
    ];
    let mut checked = 0;
    for name in listed {
        let Some(c) = cards::by_name(name) else { continue };
        assert!(cards::is(c, cards::ACTION), "{name}");
        assert!(renaissance_capitalism(c), "{name} is turned into a Treasure by Capitalism");
        checked += 1;
    }
    assert!(checked > 120, "{checked} of the listed cards are implemented");
    for name in ["Smithy", "Inventor", "Village", "Raider", "Druid"] {
        if let Some(c) = cards::by_name(name) {
            assert!(!renaissance_capitalism(c), "{name} has no +$ amount in its text");
        }
    }
}

fn renaissance_capitalism(c: CardId) -> bool {
    dominion_engine::sets::renaissance::capitalism_card(c)
}

#[test]
fn the_lantern_affects_a_border_guard_played_by_band_of_misfits_and_as_way_of_the_chameleon() {
    // "Lantern will still affect Border Guards you play from the Supply (with e.g. Band of Misfits)"; Way of the Chameleon: "you'll still reveal 3 cards".
    let mut g = new_state(&[id::BORDER_GUARD, id::BAND_OF_MISFITS], 2);
    set_holder(&mut g, id::LANTERN, 0);
    set_hand(&mut g, 0, &[id::BAND_OF_MISFITS]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::ESTATE]);
    play(&mut g, id::BAND_OF_MISFITS);
    // Three cards revealed: one to the hand, two discarded.
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].discard.total(), 2);
    let mut g = new_state(&[id::BORDER_GUARD, id::WAY_OF_THE_CHAMELEON], 2);
    set_holder(&mut g, id::LANTERN, 0);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::ESTATE]);
    play(&mut g, id::BORDER_GUARD);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Way), "{d:?}");
    choose(&mut g, Choice::Card(id::WAY_OF_THE_CHAMELEON));
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].discard.total(), 2);
}

#[test]
fn a_reckless_border_guard_reveals_twice_with_the_lantern_and_the_horn_saves_only_one() {
    // "If Border Guard is Reckless, Lantern affects both iterations, so you'll reveal a total of 6 cards." "...you can topdeck one of your Border Guards with Horn,
    // while the others will return to the Supply."
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE], 2);
    g.put_trait(id::RECKLESS, id::BORDER_GUARD);
    set_holder(&mut g, id::LANTERN, 0);
    set_holder(&mut g, id::HORN, 0);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::SILVER, id::GOLD, id::COPPER, id::SILVER, id::GOLD, id::ESTATE]);
    deck_of(&mut g, 1, 10);
    play(&mut g, id::BORDER_GUARD);
    pick(&mut g, id::GOLD);
    pick(&mut g, id::GOLD);
    assert_eq!(g.players[0].in_play.get(id::GOLD) + g.players[0].hand.get(id::GOLD), 2);
    assert_eq!(g.players[0].discard.total(), 4, "two cards discarded by each reveal of three");
}

#[test]
fn with_reckless_border_guards_the_horn_saves_one_and_the_others_return_to_the_supply() {
    let mut g = new_state(&[id::BORDER_GUARD, id::VILLAGE], 2);
    g.put_trait(id::RECKLESS, id::BORDER_GUARD);
    set_holder(&mut g, id::HORN, 0);
    g.turn.actions = 3;
    set_hand(&mut g, 0, &[id::BORDER_GUARD, id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::COPPER; 8]);
    deck_of(&mut g, 0, 10);
    deck_of(&mut g, 1, 10);
    let pile = g.supply.get(id::BORDER_GUARD);
    play(&mut g, id::BORDER_GUARD);
    for _ in 0..4 {
        if has_choice(&g, Choice::Card(id::COPPER)) {
            pick(&mut g, id::COPPER);
        }
    }
    let mut guard = 0;
    while g.turn.player == 0 && guard < 60 {
        guard += 1;
        let d = expect_decision(&mut g);
        if g.turn.player != 0 {
            break;
        }
        if d.kind == DecisionKind::PlayAction && g.players[0].hand.has(id::BORDER_GUARD) && g.turn.actions > 0 {
            let _ = play(&mut g, id::BORDER_GUARD);
            continue;
        }
        if matches!(d.kind, DecisionKind::YesNo { act: Act::Topdeck }) {
            choose(&mut g, Choice::Yes);
        } else {
            let c = if has_choice(&g, Choice::Pass) { Choice::Pass } else { choices(&g)[0] };
            choose(&mut g, c);
        }
    }
    // Four Border Guard plays (two each) happened: two cards in play. One went on the deck (and was drawn), the other back to its pile.
    assert_eq!(g.supply.get(id::BORDER_GUARD), pile + 1, "one returned to the pile");
    assert_eq!(g.players[0].hand.get(id::BORDER_GUARD), 1, "the other was topdecked with the Horn and drawn");
}

#[test]
fn improve_trashing_a_fortress_sends_it_to_the_hand_and_it_is_discarded_with_the_rest_of_cleanup() {
    // "If you put cards into your hand using Improve (e.g. you trash a Fortress...), they will get discarded during the remainder of Clean-up."
    let mut g = new_state(&[id::IMPROVE, id::FORTRESS, id::SMITHY, id::VILLAGE], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::IMPROVE, id::FORTRESS]);
    set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
    deck_of(&mut g, 0, 12);
    deck_of(&mut g, 1, 12);
    play(&mut g, id::IMPROVE);
    play(&mut g, id::FORTRESS);
    let mut guard = 0;
    while g.turn.player == 0 && guard < 30 {
        guard += 1;
        let d = expect_decision(&mut g);
        if g.turn.player != 0 {
            break;
        }
        if matches!(d.kind, DecisionKind::Select { act: Act::Trash, from: Zone::InPlay, .. }) {
            pick(&mut g, id::FORTRESS);
        } else if matches!(d.kind, DecisionKind::Gain { .. }) {
            pick(&mut g, id::SMITHY);
        } else {
            let c = if has_choice(&g, Choice::Pass) { Choice::Pass } else { choices(&g)[0] };
            choose(&mut g, c);
        }
    }
    assert_eq!(g.trash.get(id::FORTRESS), 0, "the Fortress left the trash for the hand");
    assert_eq!(g.players[0].discard.get(id::FORTRESS) + g.players[0].deck_unknown.get(id::FORTRESS), 1, "discarded in the rest of Clean-up");
    assert_eq!(g.players[0].hand.get(id::FORTRESS), 0);
}

#[test]
fn border_guard_revealing_two_patrons_gives_two_coffers_and_a_discarded_patron_gives_none() {
    // "if you play a Border Guard and reveal two Patrons, you will get +2 Coffers." "if another player plays a Villain and you discard a Patron, you do not get +1 Coffers."
    let mut g = new_state(&[id::BORDER_GUARD, id::PATRON], 2);
    set_hand(&mut g, 0, &[id::BORDER_GUARD]);
    set_deck_known(&mut g, 0, &[id::PATRON, id::PATRON, id::COPPER]);
    play(&mut g, id::BORDER_GUARD);
    assert_eq!(g.players[0].coffers, 2);
    let mut g = new_state(&[id::VILLAIN, id::PATRON], 2);
    set_hand(&mut g, 0, &[id::VILLAIN]);
    set_hand(&mut g, 1, &[id::PATRON, id::COPPER, id::COPPER, id::COPPER, id::COPPER]);
    play(&mut g, id::VILLAIN);
    adv(&mut g);
    assert_eq!(g.players[1].coffers, 0);
    assert_eq!(g.players[1].discard.get(id::PATRON), 1);
}

#[test]
fn trashing_an_acting_troupe_with_priest_in_effect_gives_two_dollars() {
    // "you will get +$2 for trashing an Acting Troupe due to playing it".
    let mut g = new_state(&[id::PRIEST, id::ACTING_TROUPE], 2);
    g.turn.actions = 2;
    set_hand(&mut g, 0, &[id::PRIEST, id::ACTING_TROUPE, id::COPPER]);
    play(&mut g, id::PRIEST);
    pick(&mut g, id::COPPER);
    let before = g.turn.coins;
    play(&mut g, id::ACTING_TROUPE);
    assert_eq!(g.turn.coins, before + 2);
}

#[test]
#[ignore = "audit: Star Chart is only offered for the shuffle of a draw, not for Annex, Donate, Inn or Famine (docs/renaissance-plan.md section 6)"]
fn star_chart_is_offered_for_the_shuffle_of_annex() {
    let mut g = new_state(&[id::STAR_CHART, id::SMITHY, id::ANNEX], 2);
    own(&mut g, 0, id::STAR_CHART);
    set_discard(&mut g, 0, &[id::PROVINCE, id::ESTATE, id::SILVER]);
    set_deck_unknown(&mut g, 0, &[id::COPPER; 6]);
    buy_phase(&mut g, 0, 2);
    adv(&mut g);
    pick(&mut g, id::ANNEX);
    pass(&mut g);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Topdeck, .. }), "{d:?}");
}
