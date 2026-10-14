//! Artificer: the bot weighs what its discards cost the Buy phase against the card it gains onto the deck. Both the search
//! (`search_play = true`) and the fast rule (`search_play = false`) are tested on fixed positions.

use dominion_engine::cards::{id, CardId};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, Counts, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

const KINGDOM: &str = "Artificer, Kitsune, Samurai, Page, Duplicate, Distant Lands, Dungeon, Root Cellar, Aristocrat, Artist";

fn strategy(extra: &str, gains: &[&str]) -> Strategy {
    let mut s = format!("name = \"T\"\n{extra}\n");
    for g in gains {
        s.push_str(&format!("[[gain]]\ncard = \"{g}\"\n"));
    }
    Strategy::parse(&s).unwrap()
}

/// Player 1 in the Action phase with Artificer and `hand` (the card Artificer draws is an Estate, from a deck of Estates), nothing else played.
fn start(hand: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Artificer, {hand}\ndeck: 12 Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap()
}

/// What the bot does with the turn: plays Artificer, makes the discards and the gain by its rules (`decide`) and then the rest of the turn.
/// Returns what was discarded (by Artificer), the card gained onto the deck (if any), and what was bought.
fn play_artificer(strat: &Strategy, mut g: GameState) -> (Counts, Option<CardId>, Option<CardId>) {
    let mut gained = None;
    let mut bought = None;
    let mut buf = ChoiceBuf::default();
    let mut picks = Counts::EMPTY;
    g.apply(Choice::Card(id::ARTIFICER), &mut NoEvents).ok();
    for _ in 0..200 {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("unexpected {s:?}"),
        };
        g.legal_choices(&mut buf);
        let ch = buf.as_slice().to_vec();
        let c = strat.decide(&PlayerView::new(&g, d.player), &d, &ch);
        match (d.kind, d.source) {
            (DecisionKind::Select { .. }, Some(id::ARTIFICER)) => {
                if let Choice::Card(x) = c {
                    picks.add(x, 1);
                }
            }
            (DecisionKind::Gain { .. }, Some(id::ARTIFICER)) => {
                if let Choice::Card(x) = c {
                    gained = Some(x);
                }
            }
            (DecisionKind::Buy, _) => {
                if let Choice::Card(x) = c {
                    bought = Some(x);
                }
                break;
            }
            _ => {}
        }
        g.apply(c, &mut NoEvents).unwrap();
    }
    (picks, gained, bought)
}

fn both(extra: &str, gains: &[&str]) -> [Strategy; 2] {
    [strategy(extra, gains), strategy(&format!("search_play = false\n{extra}"), gains)]
}

#[test]
fn surplus_treasure_and_junk_buy_a_cheap_card_while_the_best_buy_is_kept() {
    // Artificer's +$1 and 6 Coppers: $7 buys a Gold ($6). Discarding the 3 Estates (2, and the one Artificer draws) gains a Silver onto the deck.
    for strat in both("", &["Gold", "Silver"]) {
        let (picks, gained, bought) = play_artificer(&strat, start("6 Copper, 2 Estate"));
        assert_eq!(gained, Some(id::SILVER), "search_play = {}", strat.search_play);
        assert_eq!(picks.get(id::ESTATE), 3);
        assert_eq!(picks.total(), 3);
        assert_eq!(bought, Some(id::GOLD));
    }
}

#[test]
fn nothing_is_gained_when_the_discards_would_cost_the_buy() {
    // $6 buys the Gold exactly: any Treasure discarded loses it, and no listed card costs 1 or 2.
    for strat in both("", &["Gold", "Silver"]) {
        let (picks, gained, bought) = play_artificer(&strat, start("5 Copper, 1 Estate"));
        assert_eq!(picks.get(id::COPPER), 0, "search_play = {}", strat.search_play);
        assert_eq!(gained, None);
        assert_eq!(bought, Some(id::GOLD));
    }
}

#[test]
fn a_big_hand_discards_for_a_gold() {
    // A hand carried over by Biding Time: 14 cards, 6 of them Estates (5, and the one Artificer draws). The six are $6 of discards, a Gold onto the deck.
    for strat in both("", &["Gold", "Silver"]) {
        let (picks, gained, bought) = play_artificer(&strat, start("6 Copper, 5 Estate, 2 Silver"));
        assert_eq!(gained, Some(id::GOLD), "search_play = {}", strat.search_play);
        assert_eq!(picks.get(id::ESTATE), 6);
        assert_eq!(bought, Some(id::GOLD));
    }
}

#[test]
fn a_higher_listed_card_beats_the_extra_gain() {
    // Province first: $8 buys it; discarding Coppers for a Gold would lose it. The 3 Estates (one is drawn) are free: a Silver onto the deck.
    for strat in both("", &["Province", "Gold", "Silver"]) {
        let (picks, gained, bought) = play_artificer(&strat, start("7 Copper, 2 Estate"));
        assert_eq!(picks.get(id::COPPER), 0, "search_play = {}", strat.search_play);
        assert_eq!(gained, Some(id::SILVER));
        assert_eq!(bought, Some(id::PROVINCE));
    }
}
