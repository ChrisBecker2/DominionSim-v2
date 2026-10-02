//! Investment (and other self-trashing cards) in strategies: using a card is not losing it, and
//! stated `[[mode]]` rules win over the bot's search.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

fn next_decision(g: &mut GameState) -> Decision {
    match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    }
}

fn decide(strat: &Strategy, g: &GameState, d: &Decision) -> Choice {
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide(&PlayerView::new(g, d.player), d, buf.as_slice())
}

/// Player 1 in the Buy phase with `hand` (the deck behind it plain Coppers), `provinces` left.
fn buy_state(kingdom: &str, hand: &str, provinces: u8) -> GameState {
    let text = format!(
        "players: 2\nkingdom: {kingdom}\nsupply: Province={provinces}\nturn: 9  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n\
         [player 1]\nhand: {hand}\ndeck: 5 Copper\ndiscard: 5 Copper, 3 Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    parse_state(&text).unwrap()
}

/// Plays Investment from `g`, answering its decisions with `strat`, and returns whether it was
/// trashed for VP (`Some(vp tokens)`) or took the coin (`None`).
fn investment_outcome(strat: &Strategy, g: &mut GameState) -> Option<u16> {
    loop {
        let d = next_decision(g);
        match d.kind {
            DecisionKind::Buy => return None,
            DecisionKind::Mode { .. } if d.source == Some(id::INVESTMENT) => {
                let c = decide(strat, g, &d);
                g.apply(c, &mut NoEvents).unwrap();
                if matches!(c, Choice::Mode(1)) {
                    let _ = g.advance(&mut NoEvents); // resolve the mode's effect
                    return Some(g.players[0].vp_tokens);
                }
                return None;
            }
            _ => {
                let c = decide(strat, g, &d);
                g.apply(c, &mut NoEvents).unwrap();
            }
        }
    }
}

const GAINS: &str = "[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Investment\"\n[[gain]]\ncard = \"Silver\"\n";

fn strat(search: bool, extra: &str) -> Strategy {
    Strategy::parse(&format!("name = \"T\"\nsearch_play = {search}\n{GAINS}{extra}")).unwrap()
}

// Bank and Tiara stay in hand (has-choice Treasures), so Investment sees several Treasure types.
const RICH_HAND: &str = "Investment, Estate, Bank, Tiara, Gold, Gold, Gold";
const POOR_HAND: &str = "Investment, Estate, Estate, Copper, Copper";

#[test]
fn investment_on_the_gain_list_is_trashed_late_with_diverse_treasures() {
    for search in [true, false] {
        let s = strat(search, "");
        let mut g = buy_state("Investment, Bank, Tiara", RICH_HAND, 2);
        let out = investment_outcome(&s, &mut g);
        assert!(out.is_some_and(|vp| vp >= 3), "search={search}: late, 3+ Treasure types: trash for VP, got {out:?}");
    }
}

#[test]
fn investment_on_the_gain_list_takes_the_coin_early() {
    for search in [true, false] {
        let s = strat(search, "");
        // Early (8 Provinces) with few Treasure types in hand.
        let mut g = buy_state("Investment", POOR_HAND, 8);
        let out = investment_outcome(&s, &mut g);
        assert!(out.is_none(), "search={search}: early with 1 Treasure type: +$1, got {out:?}");
    }
}

/// Plays Investment by hand (trashing an Estate), stopping at its mode decision.
fn at_investment_mode(kingdom: &str, hand: &str, provinces: u8) -> GameState {
    let mut g = buy_state(kingdom, hand, provinces);
    loop {
        let d = next_decision(&mut g);
        match d.kind {
            DecisionKind::PlayTreasure => g.apply(Choice::Card(id::INVESTMENT), &mut NoEvents).unwrap(),
            DecisionKind::Select { .. } => g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap(),
            DecisionKind::Mode { .. } => return g,
            k => panic!("unexpected {k:?}"),
        }
    }
}

#[test]
fn stated_mode_rule_beats_the_search() {
    let trash = "[[mode]]
card = \"Investment\"
choose = \"Trash this for +1 VP per Treasure type in hand\"
";
    let coin = "[[mode]]
card = \"Investment\"
choose = \"+$1\"
";
    for search in [true, false] {
        // Early with one Treasure type, the search alone takes +$1; late with three, it trashes.
        let early = at_investment_mode("Investment", POOR_HAND, 8);
        let late = at_investment_mode("Investment, Bank, Tiara", RICH_HAND, 2);
        for (g, rules, want) in [(&early, "", 0u8), (&early, trash, 1), (&late, "", 1), (&late, coin, 0)] {
            let mut g = *g;
            let d = next_decision(&mut g);
            let c = decide(&strat(search, rules), &g, &d);
            assert_eq!(c, Choice::Mode(want), "search={search} rules={rules:?}");
        }
    }
}

#[test]
fn play_order_search_respects_a_stated_steward_rule() {
    // Steward in hand with two Estates: left to itself the search might pick any mode; the stated
    // rule says +$2, and the bot follows it.
    let text = "players: 2\nkingdom: Steward\nturn: 9  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
                [player 1]\nhand: Steward, Estate, Estate, Copper, Copper\ndeck: 5 Copper\ndiscard: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    for (choose, want) in [("+$2", true), ("Trash 2 cards", false)] {
        let s = Strategy::parse(&format!(
            "name = \"T\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[play]]\ncard = \"Steward\"\n[[mode]]\ncard = \"Steward\"\nchoose = \"{choose}\"\n"
        ))
        .unwrap();
        let mut g = parse_state(text).unwrap();
        let mut coins_mode = None;
        loop {
            let d = next_decision(&mut g);
            if d.kind == DecisionKind::Buy {
                break;
            }
            let c = decide(&s, &g, &d);
            if matches!(d.kind, DecisionKind::Mode { .. }) {
                coins_mode = Some(c);
            }
            g.apply(c, &mut NoEvents).unwrap();
        }
        let table = dominion_engine::cards::modes(id::STEWARD);
        let idx = match coins_mode {
            Some(Choice::Mode(i)) => i as usize,
            other => panic!("no Steward mode decision: {other:?}"),
        };
        assert_eq!(table[idx].label() == "+$2", want, "stated rule {choose:?} followed, got {}", table[idx].label());
    }
}

#[test]
fn mining_village_self_trash_is_taken_when_the_coins_buy_something_better() {
    // Mining Village + $6 in hand: trashing it for +$2 reaches $8 (Province on the list).
    // Gold is the other thing $6 buys; Mining Village itself isn't on the gain list.
    for search in [true, false] {
        let s = strategy_with_mining_village(search);
        let text = "players: 2\nkingdom: Mining Village\nturn: 9  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
                    [player 1]\nhand: Mining Village, Gold, Gold, Copper, Copper\ndeck: 5 Copper\ndiscard: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
        let mut g = parse_state(text).unwrap();
        let bought;
        loop {
            let d = next_decision(&mut g);
            let c = decide(&s, &g, &d);
            if d.kind == DecisionKind::Buy {
                bought = Some(c);
                break;
            }
            g.apply(c, &mut NoEvents).unwrap();
        }
        assert_eq!(bought, Some(Choice::Card(id::PROVINCE)), "search={search}: Mining Village trashed itself for +$2 to afford the Province");
    }
}

fn strategy_with_mining_village(search: bool) -> Strategy {
    Strategy::parse(&format!(
        "name = \"T\"\nsearch_play = {search}\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[play]]\ncard = \"Mining Village\"\n"
    ))
    .unwrap()
}

#[test]
fn investment_bm_games_trash_investment_for_vp_sometimes() {
    use dominion_engine::rng::Rng;
    use dominion_engine::{play_game, Agent, Event, GameConfig};
    use dominion_sim::StrategyAgent;
    let bm = Strategy::parse(
        "name = \"Investment BM\"
[[gain]]
card = \"Province\"
[[gain]]
card = \"Duchy\"
if = \"provinces_left <= 4\"
[[gain]]
card = \"Gold\"
[[gain]]
card = \"Investment\"
if = \"count(Investment) < 2\"
[[gain]]
card = \"Silver\"
",
    )
    .unwrap();
    let mut kingdom = dominion_engine::cards::FIRST_GAME.to_vec();
    kingdom.push(id::INVESTMENT);
    let (mut trashed_for_vp, mut coin_modes) = (0, 0);
    for g in 0..30u64 {
        let cfg = GameConfig { num_players: 2, kingdom: kingdom.clone(), seed: Rng::derive(7, g).next_u64(), max_turns: 200 };
        let mut state = GameState::new(&cfg);
        let (mut a, mut b) = (StrategyAgent::new(&bm), StrategyAgent::new(&bm));
        let mut agents: [&mut dyn Agent; 2] = [&mut a, &mut b];
        let mut events: Vec<Event> = Vec::new();
        play_game(&mut state, &mut agents, &mut events);
        for e in &events {
            match e {
                Event::Bonus { source, vp, .. } if *source == id::INVESTMENT && *vp > 0 => trashed_for_vp += 1,
                Event::Bonus { source, coins, .. } if *source == id::INVESTMENT && *coins == 1 => coin_modes += 1,
                _ => {}
            }
        }
    }
    println!("Investment: trashed for VP {trashed_for_vp} times, +$1 {coin_modes} times");
    assert!(trashed_for_vp > 0, "Investment is sometimes trashed for VP ({coin_modes} coin uses)");
}
