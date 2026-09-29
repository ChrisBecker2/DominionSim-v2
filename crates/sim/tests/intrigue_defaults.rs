//! Default bot behavior for step-2 Intrigue cards' `YesNo` decisions when the strategy states no
//! rule for them (see docs/intrigue-plan.md, "Bots follow stated rules first; defaults rank
//! below them"):
//!   - Baron: discard an Estate for +$4 by default.
//!   - Mining Village: trash itself only if the strategy's `[[trash]]` rules name it.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Act, Choice, ChoiceBuf, Decision, DecisionKind, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

/// Parse a fresh 2-player state with `kingdom`, player 1's `hand`/`deck`, play `play_card`, and
/// return the state at the next decision.
fn decide_after_play(kingdom: &str, hand: &str, deck: &str, play_card: dominion_engine::CardId) -> (dominion_engine::GameState, Decision) {
    let text = format!(
        "players: 2\nkingdom: {kingdom}\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\nhand: {hand}\ndeck: {deck}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(play_card), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    };
    (g, d)
}

fn decide(strat: &Strategy, g: &dominion_engine::GameState, d: &Decision) -> Choice {
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide_by_rules(&PlayerView::new(g, 0), d, buf.as_slice())
}

#[test]
fn baron_discards_estate_for_four_by_default() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let (g, d) = decide_after_play("Baron", "Baron, Estate", "5 Copper", id::BARON);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Discard });
    assert_eq!(d.subject, id::ESTATE);
    assert_eq!(decide(&strat, &g, &d), Choice::Yes, "Baron should discard the Estate for +$4 by default");
}

#[test]
fn mining_village_not_trashed_by_default() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let (g, d) = decide_after_play("Mining Village", "Mining Village", "5 Copper", id::MINING_VILLAGE);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Trash });
    assert_eq!(d.subject, id::MINING_VILLAGE);
    assert_eq!(decide(&strat, &g, &d), Choice::No, "the default trash list is Curse/Estate/Copper, not Mining Village");
}

#[test]
fn mining_village_trashed_when_the_trash_rules_name_it() {
    let strat = Strategy::parse("name = \"T\"\n[[trash]]\ncard = \"Mining Village\"\n").unwrap();
    let (g, d) = decide_after_play("Mining Village", "Mining Village", "5 Copper", id::MINING_VILLAGE);
    assert_eq!(decide(&strat, &g, &d), Choice::Yes, "a stated [[trash]] rule for Mining Village should trash it");
}
