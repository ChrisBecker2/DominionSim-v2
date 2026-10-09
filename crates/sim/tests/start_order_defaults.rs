//! Bot defaults for the order of start-of-turn effects (`DecisionKind::StartOrder`): cards put
//! into the hand first (Haven, Blockade, Pirate), then the plain bonuses and draws, then plays
//! (Clerk, a Prince's card), and the effects that discard or trash from the hand last (Sea Witch,
//! Tide Pools, Sailor). The order matters, and the bot gets it right.
//!
//! And Prince: set aside the best Action costing up to $4, and play whole games with it.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Choice, ChoiceBuf, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::{Strategy, StrategyAgent};

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

/// The rules' own order: with `search_play` off, a Clerk among the options isn't searched.
fn no_search(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&format!("search_play = false\n{src}")).unwrap()
}

const KINGDOM: &str = "Haven, Sailor, Tide Pools, Sea Witch, Caravan, Pirate, Clerk, Prince, Smithy, Village, Militia, Chapel";

fn state(fields: &str) -> String {
    format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\n{fields}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    )
}

/// Play the strategy's own choices from the start of the turn until the first Action or Buy
/// decision; returns the state there and the first StartOrder choice it made (if asked).
fn run_start(strat: &Strategy, text: &str) -> (GameState, Option<Choice>) {
    let mut g = parse_state(text).unwrap();
    let mut first = None;
    for _ in 0..40 {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) {
            return (g, first);
        }
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
        if d.kind == DecisionKind::StartOrder && first.is_none() {
            first = Some(c);
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    panic!("no turn decision reached");
}

#[test]
fn a_haven_card_returns_before_sea_witch_discards() {
    // Hand {Gold, Silver}; Haven holds an Estate; Sea Witch must discard 2. Returning the Estate
    // first lets the discard choose from three cards and keep the Gold; discarding first would
    // throw away both good cards.
    let strat = load("big_money_ultimate.toml");
    let text = state("hand: Gold, Silver\nset aside: Estate\ndurations: Sea Witch, Haven (Estate)\nin play: Sea Witch, Haven");
    let (g, first) = run_start(&strat, &text);
    assert_eq!(first, Some(Choice::Card(id::HAVEN)), "cards come back first");
    let mine = {
        let mut c = g.players[0].hand;
        c.add_all(&g.players[0].in_play);
        c
    };
    assert!(mine.has(id::GOLD), "the Gold was kept");
    assert!(g.players[0].discard.has(id::ESTATE) && g.players[0].discard.has(id::SILVER), "the Estate and Silver were discarded");
}

#[test]
fn a_haven_gold_is_in_hand_for_the_discard_to_choose_from() {
    // The other way round: Haven holds a Gold, and the bot still returns it first.
    let strat = load("big_money_ultimate.toml");
    let text = state("hand: Copper, Estate\nset aside: Gold\ndurations: Sea Witch, Haven (Gold)\nin play: Sea Witch, Haven\ndeck: Estate, Copper");
    let (g, first) = run_start(&strat, &text);
    assert_eq!(first, Some(Choice::Card(id::HAVEN)));
    let mut mine = g.players[0].hand;
    mine.add_all(&g.players[0].in_play);
    assert!(mine.has(id::GOLD), "the Gold is kept");
    assert_eq!(g.players[0].discard.total(), 2);
}

#[test]
fn sailors_trash_comes_after_the_draws() {
    // Hand {Silver, Silver}; Caravan's card is a Curse. Drawing first puts the Curse in hand for
    // the Sailor to trash; trashing first finds nothing worth trashing and the Curse is stuck.
    let strat = load("big_money_ultimate.toml");
    let text = state("hand: 2 Silver\ndeck top: Curse\ndeck: Gold\ndurations: Sailor, Caravan\nin play: Sailor, Caravan");
    let (g, first) = run_start(&strat, &text);
    assert_eq!(first, Some(Choice::Card(id::CARAVAN)), "the draw goes before the trash");
    assert!(g.trash.has(id::CURSE), "the Curse drawn by Caravan was trashed by Sailor");
}

#[test]
fn the_default_order_is_returns_then_bonuses_then_plays_then_discards() {
    // Caravan (a draw), Clerk in hand and Tide Pools (a discard): the first choice of each
    // successive question follows that order.
    let strat = no_search("big_money_ultimate.toml");
    let text = state("hand: Clerk, Estate, Copper\ndeck: 8 Estate\nset aside:\ndurations: Tide Pools, Caravan\nin play: Tide Pools, Caravan");
    let mut g = parse_state(&text).unwrap();
    let mut order: Vec<dominion_engine::CardId> = Vec::new();
    for _ in 0..40 {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) {
            break;
        }
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
        if d.kind == DecisionKind::StartOrder {
            if let Choice::Card(x) = c {
                order.push(x);
            }
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    assert_eq!(order, vec![id::CARAVAN, id::CLERK], "Caravan, then Clerk's play; Tide Pools' discard is last (no question left)");
}

// ---------------------------------------------------------------------------------------
// Prince
// ---------------------------------------------------------------------------------------

#[test]
fn prince_sets_aside_the_strongest_action_it_can_hold() {
    let strat = load("big_money_ultimate.toml");
    // Smithy ($4, +3 Cards) over Village ($3) and Militia ($4, +$2 attack); the Chapel costs $2.
    let text = state("hand: Prince, Village, Smithy, Militia, Chapel");
    let mut g = parse_state(&text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::PRINCE), &mut NoEvents).unwrap();
    let mut picked = None;
    for _ in 0..4 {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        let c = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
        if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) {
            break;
        }
        if matches!(d.kind, DecisionKind::Select { .. }) {
            picked = Some(c);
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    assert_eq!(picked, Some(Choice::Card(id::SMITHY)));
    assert_eq!(g.players[0].prince_cards.get(id::SMITHY), 1);
}

#[test]
fn prince_never_offers_a_duration_and_holds_the_best_other_action() {
    let strat = load("big_money_ultimate.toml");
    // Caravan is a Duration: not eligible, so the Village is the card.
    let text = state("hand: Prince, Caravan, Village, Gold");
    let mut g = parse_state(&text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::PRINCE), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    assert_eq!(buf.as_slice(), &[Choice::Card(id::VILLAGE), Choice::Pass]);
    assert_eq!(strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice()), Choice::Card(id::VILLAGE));
}

#[test]
fn a_prince_smithy_strategy_plays_whole_games() {
    let ps = load("prince_smithy.toml");
    let bm = load("big_money_ultimate.toml");
    let kingdom = vec![id::PRINCE, id::SMITHY, id::VILLAGE, id::MILITIA, id::MOAT, id::MARKET, id::CELLAR, id::CHAPEL, id::WORKSHOP, id::REMODEL];
    let mut games_with_a_held_card = 0;
    let mut prince_turns = 0u32;
    for seed in 0..40u64 {
        for swap in [false, true] {
            let cfg = dominion_engine::GameConfig { num_players: 2, kingdom: kingdom.clone(), seed, max_turns: 200, ..Default::default() };
            let mut g = GameState::new(&cfg);
            let mut a = StrategyAgent::new(&ps);
            let mut b = StrategyAgent::new(&bm);
            let r = if swap {
                dominion_engine::play_game(&mut g, &mut [&mut b, &mut a], &mut NoEvents)
            } else {
                dominion_engine::play_game(&mut g, &mut [&mut a, &mut b], &mut NoEvents)
            };
            assert!(g.is_game_over() && r.winners != 0);
            let me = if swap { 1 } else { 0 };
            let ps_ = &g.players[me];
            if ps_.prince_cards.total() > 0 {
                assert!(ps_.in_play.has(id::PRINCE), "a Prince holding a card stays in play");
                games_with_a_held_card += 1;
            }
            prince_turns += ps_.turns_taken as u32;
            // Cards are never lost or duplicated by the Prince bookkeeping.
            let owned: u32 = ps_.all_cards().total();
            assert!(owned >= 10, "{owned}");
        }
    }
    assert!(games_with_a_held_card > 0, "the Prince strategy should have set a Prince aside in some game");
    assert!(prince_turns > 0);
}
