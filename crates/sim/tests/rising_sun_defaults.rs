//! Bot defaults for Rising Sun: the bots play every Prophecy and every card of the set to the end without an illegal choice, and a few decisions.

use dominion_engine::cards::{self, id};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, NoEvents, PlayerView, Step};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_search::Evaluator;
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

#[test]
fn bots_play_every_prophecy_to_the_end() {
    let bmu = load("big_money_ultimate.toml");
    let kitsune = load("kitsune_bm.toml");
    let rs: Vec<_> = cards::kingdom_cards_in(cards::CardSet::RisingSun).filter(|&c| c != id::RIVERBOAT).collect();
    for i in 0..cards::NUM_PROPHECIES {
        let prophecy = cards::FIRST_PROPHECY + i as u16;
        let mut kingdom: Vec<u16> = rs.iter().copied().skip(i % 5).step_by(2).take(8).collect();
        for extra in [id::TEA_HOUSE, id::POET, id::ALLEY, id::FISHMONGER, id::SMITHY, id::VILLAGE] {
            if kingdom.len() < 10 && !kingdom.contains(&extra) {
                kingdom.push(extra);
            }
        }
        kingdom.push(prophecy);
        if prophecy == id::APPROACHING_ARMY {
            kingdom.push(id::MILITIA);
        }
        let cfg = MatchConfig { games: 30, kingdom, seed: i as u64, max_turns: 200 };
        let stats = run_match(&[&kitsune, &bmu], &cfg);
        assert_eq!(stats[0].games + 0, 30, "{}", cards::name(prophecy));
    }
}

#[test]
fn snake_witch_and_gold_mine_decisions() {
    let strat = load("big_money_ultimate.toml");
    let text = "players: 2\nkingdom: Snake Witch, Gold Mine, Smithy, Village, Market, Militia, Moat, Cellar, Workshop, Remodel\n\
                turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Gold Mine\ndeck: 3 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::GOLD_MINE), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!(strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice()), Choice::Yes, "a Gold for 4 Debt early in the game");
}

#[test]
fn the_searching_bot_curses_with_kitsune() {
    // Kitsune's "choose two": the turn search used to value only its own deck, so a Silver beat
    // handing out a Curse. A Curse given is worth more than a Silver gained.
    let strat = load("kitsune_bm.toml");
    assert!(strat.search_play, "the full (searching) bot");
    let text = "players: 2\nkingdom: Kitsune, Smithy, Village, Market, Militia, Moat, Cellar, Workshop, Remodel, Mine\n\
                turn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Kitsune, Copper, Copper, Estate, Estate\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    let mut buf = ChoiceBuf::default();
    while g.turn.player == 0 && g.turn.number == 3 {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            _ => break,
        };
        if d.player != 0 || matches!(d.kind, dominion_engine::DecisionKind::Buy) {
            break;
        }
        g.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
        g.apply(c, &mut NoEvents).unwrap();
    }
    assert_eq!(g.players[1].all_cards().get(id::CURSE), 1, "player 2 got a Curse");
}

// --- Sun tokens ------------------------------------------------------------------------------------------

/// A state at the Buy phase with the given Prophecy and 3 Sun tokens left; each player's cards are given as `deck:` lines.
fn sun_state(prophecy: &str, p1_cards: &str, p2_cards: &str) -> dominion_engine::GameState {
    parse_state(&format!(
        "players: 2\nkingdom: Kitsune, Samurai, Militia, Smithy, Village, Market, Moat, Cellar, Workshop, Remodel\nprophecy: {prophecy}\nsuns: 3\n\
         turn: 5  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n[player 1]\nhand: Copper\ndeck: {p1_cards}\n\n[player 2]\nhand: Copper\ndeck: {p2_cards}\n"
    ))
    .unwrap()
}

#[test]
fn removing_a_sun_token_is_worth_what_the_prophecy_does_for_the_player() {
    let strat = load("big_money_ultimate.toml");
    let ev = dominion_sim::GainListEvaluator::new(&strat);
    let value_of_a_token = |g: &dominion_engine::GameState| {
        let mut leaf = *g;
        leaf.rs.suns -= 1;
        ev.leaf_value(g, &leaf, 0) - ev.leaf_value(g, g, 0)
    };
    // Approaching Army (+$1 for each Attack played): good for the player with more Attacks, bad for the other, nothing when level.
    let mine = sun_state("Approaching Army", "4 Militia, 6 Copper", "6 Copper, 4 Estate");
    let theirs = sun_state("Approaching Army", "6 Copper, 4 Estate", "4 Militia, 6 Copper");
    let level = sun_state("Approaching Army", "4 Militia, 6 Copper", "4 Militia, 6 Copper");
    assert!(value_of_a_token(&mine) > 0.1, "{}", value_of_a_token(&mine));
    assert!(value_of_a_token(&theirs) < -0.1, "{}", value_of_a_token(&theirs));
    assert!(value_of_a_token(&level).abs() < 1e-6, "{}", value_of_a_token(&level));
    // Panic hurts the player with more Treasures.
    let treasure_rich = sun_state("Panic", "10 Silver", "4 Silver, 6 Estate");
    assert!(value_of_a_token(&treasure_rich) < -0.1);
    // Biding Time: the Artificer (discarding from a bigger hand) and clean decks favour their owner.
    let artificer = sun_state("Biding Time", "2 Artificer, 8 Copper", "2 Smithy, 8 Copper");
    assert!(value_of_a_token(&artificer) > 0.1);
    // A Prophecy without a model (Progress) and a game without Prophecy: nothing.
    assert!(value_of_a_token(&sun_state("Progress", "4 Militia, 6 Copper", "6 Copper, 4 Estate")).abs() < 1e-6);
    // Always far below one gain-list step (a Silver, 1000 on the gain-list scale) and below a stated play rule (5).
    assert!(value_of_a_token(&mine) < 1.0);
}

#[test]
fn kintsugi_is_bought_only_with_a_card_worth_trashing() {
    // Kintsugi must trash a card from the hand. Late in the game the hand left at the buy is often all Victory cards: a Duchy turned into a
    // Gold loses 3 VP, a Province into a Province wastes the buy. Early, an Estate is worth trashing.
    let strat = Strategy::parse(
        "name = \"K\"
[[gain]]
card = \"Province\"
[[gain]]
card = \"Duchy\"
if = \"provinces_left <= 4\"
[[gain]]
card = \"Gold\"
[[gain]]
card = \"Kintsugi\"
[[gain]]
card = \"Silver\"
",
    )
    .unwrap();
    let buy = |supply: &str, hand: &str| {
        let text = format!(
            "players: 2
kingdom: Samurai, Kitsune, Page, Dungeon, Artist, Aristocrat, Root Cellar, Duplicate, Distant Lands, Artificer
events: Kintsugi
             supply: {supply}
turn: 9  player: 1  phase: buy  actions: 0  buys: 1  coins: 4

[player 1]
hand: {hand}
deck: 3 Copper, Gold
rs gold: true

             [player 2]
hand: 5 Copper
deck: 5 Copper
"
        );
        let mut g = parse_state(&text).unwrap();
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        assert!(buf.as_slice().contains(&Choice::Card(id::KINTSUGI)), "Kintsugi is on offer");
        strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice())
    };
    assert_eq!(buy("Province=8", "Estate, Province"), Choice::Card(id::KINTSUGI), "an Estate is worth trashing early");
    assert_ne!(buy("Province=3", "Duchy, Province"), Choice::Card(id::KINTSUGI), "nothing worth trashing late");
}

#[test]
fn sickness_does_not_make_the_bot_discard_a_small_hand_of_good_cards() {
    use dominion_engine::{Counts, DecisionKind, GameConfig, GameState};
    let strat = load("big_money_ultimate.toml");
    let cfg = GameConfig { num_players: 2, kingdom: vec![id::SMITHY, id::TEA_HOUSE, id::SICKNESS], seed: 3, ..Default::default() };
    let mut g = GameState::new(&cfg);
    g.rs.suns = 0;
    g.rs.active = id::SICKNESS;
    g.rs_update_hooks();
    g.turn = dominion_engine::state::TurnState::start(0, 1);
    g.stack = Default::default();
    g.pending = dominion_engine::Pending::None;
    let mut hand = Counts::EMPTY;
    hand.add(id::GOLD, 3);
    g.players[0].hand = hand;
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert!(matches!(d.kind, DecisionKind::Mode { .. }), "{d:?}");
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    // Mode 0 is the Curse onto the deck, Mode 1 the discard.
    assert_eq!(strat.rule_mode(&PlayerView::new(&g, 0), &d, buf.as_slice()), Choice::Mode(0));
}
