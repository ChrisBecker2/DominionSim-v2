//! Bot policies for the Nocturne cards: Night cards in `[[play]]` rules and the Night search, Boon picks, now-or-later, the Changeling exchange,
//! the small choices inside Boons and cards, stated rules, and whole games.

use dominion_engine::cards::{self, id, CardId};
use dominion_engine::text::parse_state;
use dominion_engine::{Act, Agent, Choice, ChoiceBuf, Decision, DecisionKind, Event, GameConfig, GameState, NoEvents, PlayerView, Step};
use dominion_sim::{Strategy, StrategyAgent};

const KINGDOM: &str = "Monastery, Changeling, Bard, Sacred Grove, Blessed Village, Werewolf, Vampire, Druid, Fool, Pixie";

fn load(name: &str) -> Strategy {
    Strategy::load(std::path::Path::new(&format!("../../strategies/{name}.toml"))).unwrap_or_else(|e| panic!("{e}"))
}

/// Player 1's turn in `phase`, the given zones; player 2 holds 5 Coppers over 5 Estates.
fn state(phase: &str, coins: u16, buys: u8, p1: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: {phase}  actions: 1  buys: {buys}  coins: {coins}\n\n[player 1]\n{p1}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap()
}

fn next_decision(g: &mut GameState) -> (Decision, Vec<Choice>) {
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    (d, buf.as_slice().to_vec())
}

fn decide(strat: &Strategy, g: &GameState, d: &Decision, choices: &[Choice]) -> Choice {
    strat.decide(&PlayerView::new(g, d.player), d, choices)
}

fn strategy(extra: &str, gains: &[&str]) -> Strategy {
    let mut s = format!("name = \"T\"\n{extra}\n");
    for g in gains {
        s.push_str(&format!("[[gain]]\ncard = \"{g}\"\n"));
    }
    Strategy::parse(&s).unwrap()
}

fn money(extra: &str) -> Strategy {
    strategy(extra, &["Province", "Gold", "Silver"])
}

/// Play one whole game of two strategies on a Nocturne kingdom, returning the events.
fn play_logged(a: &Strategy, b: &Strategy, kingdom: &[CardId], seed: u64) -> (GameState, Vec<Event>) {
    let mut g = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.to_vec(), seed, max_turns: 200, ..Default::default() });
    let mut agents = [StrategyAgent::new(a), StrategyAgent::new(b)];
    let mut log: Vec<Event> = Vec::new();
    let mut buf = ChoiceBuf::default();
    loop {
        match g.advance(&mut log) {
            Step::Decision(d) => {
                g.legal_choices(&mut buf);
                let view = PlayerView::new(&g, d.player);
                let c = agents[d.player as usize].choose(&view, &d, buf.as_slice());
                g.apply(c, &mut log).expect("illegal choice by the bot");
            }
            Step::GameOver => break,
            Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
        }
    }
    (g, log)
}

// --- the Night phase ---------------------------------------------------------------------------------------------

fn night_state(hand: &str) -> GameState {
    // A Buy phase with nothing to buy with: passing it leads to the Night decision.
    let mut g = state("buy", 0, 1, &format!("hand: {hand}\ndeck: 8 Estate"));
    let _ = next_decision(&mut g);
    g.apply(Choice::Pass, &mut NoEvents).unwrap();
    g
}

#[test]
fn choice_free_night_cards_are_played_without_a_search_in_any_order() {
    let strat = money("");
    let mut g = night_state("Guardian, Ghost Town, Raider");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayNight);
    // The lowest id first (any order is the same): Guardian.
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GUARDIAN));
    assert!(dominion_sim::strategy::night_obvious(&PlayerView::new(&g, 0), &ch).is_some());
    // A hand with a gainer and a trasher is a real choice.
    let mut g = night_state("Monastery, Devil's Workshop");
    let (_, ch) = next_decision(&mut g);
    assert!(dominion_sim::strategy::night_obvious(&PlayerView::new(&g, 0), &ch).is_none());
}

#[test]
fn stated_play_rules_rank_night_cards() {
    assert!(Strategy::parse("name = \"T\"\n[[play]]\ncard = \"Raider\"\n").is_ok(), "a Night card is accepted in a play list");
    assert!(Strategy::parse("name = \"T\"\n[[play]]\ncard = \"Gold\"\n").is_err());
    let strat = money("[[play]]\ncard = \"Vampire\"\n[[play]]\ncard = \"Monastery\"\n");
    let mut g = night_state("Monastery, Vampire");
    let (_, ch) = next_decision(&mut g);
    assert_eq!(strat.rule_night(&PlayerView::new(&g, 0), &ch), Choice::Card(id::VAMPIRE));
    // A rule with a condition on the phase: `night` is 1 in the Night phase.
    let cond = money("[[play]]\ncard = \"Werewolf\"\nif = \"night == 1\"\n");
    let mut g = night_state("Werewolf, Monastery");
    let (_, ch) = next_decision(&mut g);
    assert_eq!(cond.rule_night(&PlayerView::new(&g, 0), &ch), Choice::Card(id::WEREWOLF));
}

// --- Boons ------------------------------------------------------------------------------------------------------

#[test]
fn blessed_village_receives_boons_that_act_now_and_saves_those_that_want_a_fresh_hand() {
    let strat = money("");
    for (boon, now) in [
        (id::THE_MOUNTAINS_GIFT, Choice::Yes),
        (id::THE_SWAMPS_GIFT, Choice::Yes),
        (id::THE_EARTHS_GIFT, Choice::No),
        (id::THE_SEAS_GIFT, Choice::No),
        (id::THE_WINDS_GIFT, Choice::No),
    ] {
        let mut g = state("buy", 4, 1, "hand: Gold\ndeck: 8 Estate");
        g.noc.boon_deck = 1 << (boon - cards::FIRST_BOON);
        next_decision(&mut g);
        g.apply(Choice::Card(id::BLESSED_VILLAGE), &mut NoEvents).unwrap();
        // (Changeling is in this kingdom: its exchange is asked first.)
        let (mut d, mut ch) = next_decision(&mut g);
        if d.source == Some(id::CHANGELING) {
            g.apply(Choice::No, &mut NoEvents).unwrap();
            (d, ch) = next_decision(&mut g);
        }
        assert!(matches!(d.kind, DecisionKind::YesNo { act: Act::Reveal }) && d.subject == boon, "{d:?}");
        assert_eq!(decide(&strat, &g, &d, &ch), now, "{}", cards::name(boon));
    }
}

#[test]
fn druid_picks_the_best_boon_of_its_three() {
    let strat = money("");
    let mut g = state("action", 0, 1, "hand: Druid\ndeck: 8 Estate");
    g.noc.boon_druid = (1 << (id::THE_SEAS_GIFT - cards::FIRST_BOON))
        | (1 << (id::THE_MOUNTAINS_GIFT - cards::FIRST_BOON))
        | (1 << (id::THE_WINDS_GIFT - cards::FIRST_BOON));
    next_decision(&mut g);
    g.apply(Choice::Card(id::DRUID), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PickBoon);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::THE_MOUNTAINS_GIFT));
}

#[test]
fn lost_in_the_woods_discards_only_dead_cards() {
    let strat = money("");
    let mut g = state("action", 0, 1, "hand: Estate, Silver, Silver, Gold, Copper\ndeck: 8 Estate");
    g.noc.lost = 1;
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }), "{d:?}");
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::ESTATE));
    let mut g = state("action", 0, 1, "hand: Silver, Silver, Gold, Copper, Copper\ndeck: 8 Estate");
    g.noc.lost = 1;
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Pass);
}

// --- the Changeling exchange --------------------------------------------------------------------------------------

fn changeling_ask(card: CardId) -> (GameState, Decision, Vec<Choice>) {
    let mut g = state("buy", 6, 1, "hand: Gold, Gold\ndeck: 8 Estate");
    next_decision(&mut g);
    g.apply(Choice::Card(card), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }) && d.source == Some(id::CHANGELING), "{d:?}");
    (g, d, ch)
}

#[test]
fn a_card_low_on_the_gain_list_is_exchanged_for_a_changeling_and_a_top_priority_is_not() {
    let strat = strategy("", &["Province", "Gold", "Vampire", "Silver"]);
    let (g, d, ch) = changeling_ask(id::SILVER);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Yes, "listed last");
    let (g, d, ch) = changeling_ask(id::BARD);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Yes, "unlisted");
    let (g, d, ch) = changeling_ask(id::GOLD);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::No, "listed second");
    let (g, d, ch) = changeling_ask(id::DUCHY);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::No, "a Victory card is kept");
    // A stated [[exchange]] rule overrides the default either way.
    let rule = strategy("[[exchange]]\ncard = \"Gold\"\n[[exchange]]\ncard = \"Silver\"\nif = \"coins < 0\"\n", &["Province", "Gold", "Silver"]);
    let (g, d, ch) = changeling_ask(id::GOLD);
    assert_eq!(decide(&rule, &g, &d, &ch), Choice::Yes);
    let (g, d, ch) = changeling_ask(id::SILVER);
    assert_eq!(decide(&rule, &g, &d, &ch), Choice::No);
    assert!(Strategy::parse("name = \"T\"\n[[exchange]]\ncard = \"Estate\"\n").is_err(), "only cards costing $3 or more, or Travellers");
}

// --- conditions --------------------------------------------------------------------------------------------------

#[test]
fn conditions_see_boons_hexes_and_states() {
    let mut g = state("action", 0, 1, "hand: Estate\ndeck: 8 Estate");
    g.noc.boon_discard = 1 << (id::THE_SEAS_GIFT - cards::FIRST_BOON);
    g.noc.boon_deck = 0xFFF & !g.noc.boon_discard;
    g.noc.hex_discard = 1 << (id::WAR - cards::FIRST_HEX);
    g.players[0].noc = dominion_engine::state::P_DELUDED;
    g.players[1].noc = dominion_engine::state::P_ENVIOUS;
    let view = PlayerView::new(&g, 0);
    let ev = |s: &str| dominion_sim::expr::Expr::parse(s).unwrap().eval(&view);
    assert_eq!(ev("boon_in_discard(The Sea's Gift)"), 1);
    assert_eq!(ev("boon_in_deck(The Sea's Gift)"), 0);
    assert_eq!(ev("boon_in_deck(The Sun's Gift)"), 1);
    assert_eq!(ev("hex_in_discard(War)"), 1);
    assert_eq!(ev("has_state(Deluded)"), 1);
    assert_eq!(ev("has_state(Envious)"), 0);
    assert_eq!(ev("opp_has_state(Envious)"), 1);
    assert_eq!(ev("boons_in_discard"), 1);
    assert_eq!(ev("boons_in_deck"), 11);
    assert_eq!(ev("hexes_in_discard"), 1);
    assert_eq!(ev("night"), 0);
    assert_eq!(ev("count_type(night)"), 0);
    assert!(dominion_sim::expr::Expr::parse("has_state(Gold)").is_err());
    assert!(dominion_sim::expr::Expr::parse("boon_in_discard(War)").is_err());
}

// --- whole games ---------------------------------------------------------------------------------------------------

// (A thread with a big stack: the bots' search recurses deeply, and every level holds a `GameState`.)
#[test]
fn bots_play_whole_games_on_random_nocturne_kingdoms() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(bots_play_whole_games_on_random_nocturne_kingdoms_inner).unwrap().join().unwrap();
}

fn bots_play_whole_games_on_random_nocturne_kingdoms_inner() {
    let strategies = ["skulk_bm", "sacred_grove_bm", "werewolf_bm", "big_money_ultimate"].map(load);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == cards::CardSet::Nocturne).collect();
    let mut rng = dominion_engine::rng::Rng::new(77);
    for game in 0..60u64 {
        let mut from = pool.clone();
        let mut kingdom = Vec::new();
        for _ in 0..10 {
            let i = rng.below(from.len() as u32) as usize;
            kingdom.push(from.swap_remove(i));
        }
        let a = &strategies[(game % 4) as usize];
        let b = &strategies[((game / 4 + 1) % 4) as usize];
        let (g, _) = play_logged(a, b, &kingdom, game);
        assert!(g.is_game_over());
    }
}
