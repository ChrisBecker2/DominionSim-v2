//! Bot policy for revealing Moat (a choice now): always, except against an Attack that can help the
//! victim (Minion's new hand of four when that beats the hand held), and for the one-card deck-top
//! zone of a gain onto the deck (Watchtower's offer for a Silver; the bot keeps it).

use dominion_engine::cards::id;
use dominion_engine::text::parse_state;
use dominion_engine::{Act, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step, Zone};
use dominion_sim::Strategy;

const KINGDOM: &str = "Witch, Militia, Minion, Moat, Watchtower, Bureaucrat, Village, Smithy, Market, Cellar";

/// Player 1 about to play `attack`; player 2 holds `victim_hand` over the unknown deck `victim_deck`.
fn state(attack: &str, victim_hand: &str, victim_deck: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: {attack}\ndeck top: 4 Copper\n\n[player 2]\nhand: {victim_hand}\ndeck: {victim_deck}\n"
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

fn bot() -> Strategy {
    Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n").unwrap()
}

/// The victim's answer to the Moat question the attack `attack` raises.
fn moat_answer(attack: dominion_engine::CardId, g: &mut GameState) -> Choice {
    next_decision(g);
    g.apply(Choice::Card(attack), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(g);
    assert_eq!((d.player, d.subject, d.source), (1, id::MOAT, Some(attack)));
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Reveal });
    bot().decide(&PlayerView::new(g, d.player), &d, &ch)
}

#[test]
fn the_bot_reveals_moat_against_witch_and_militia() {
    let mut g = state("Witch", "Moat, 4 Estate", "5 Copper");
    assert_eq!(moat_answer(id::WITCH, &mut g), Choice::Yes);
    let mut g = state("Militia", "Moat, 4 Gold", "5 Copper");
    assert_eq!(moat_answer(id::MILITIA, &mut g), Choice::Yes, "even a good hand: Militia only takes cards away");
}

#[test]
fn the_bot_declines_moat_against_a_minion_when_a_new_hand_is_better() {
    // Four Estates are worth nothing; four cards from a deck of Golds are worth $12.
    let mut g = state("Minion", "Moat, 4 Estate", "10 Gold");
    assert_eq!(moat_answer(id::MINION, &mut g), Choice::No);
    // A rich hand against a poor deck: keep it, so block.
    let mut g = state("Minion", "Moat, 4 Gold", "10 Estate");
    assert_eq!(moat_answer(id::MINION, &mut g), Choice::Yes);
    // Fewer than 5 cards: Minion does nothing to the victim either way.
    let mut g = state("Minion", "Moat, 3 Estate", "10 Gold");
    assert_eq!(moat_answer(id::MINION, &mut g), Choice::Yes);
}

#[test]
fn the_bot_keeps_the_silver_watchtower_sees_on_top_of_the_deck() {
    // Bureaucrat's Silver arrives on the deck: Watchtower's offer is over the deck-top zone.
    let strat = Strategy::parse("name = \"T\"\n[[trash]]\ncard = \"Curse\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let mut g = parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Bureaucrat, Watchtower\ndeck top: Copper\n\n[player 2]\nhand: 5 Estate\n"
    ))
    .unwrap();
    next_decision(&mut g);
    g.apply(Choice::Card(id::BUREAUCRAT), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.source, Some(id::WATCHTOWER));
    assert!(matches!(d.kind, DecisionKind::Select { from: Zone::DeckTop, act: Act::Trash, .. }), "{d:?}");
    assert_eq!(ch, vec![Choice::Card(id::SILVER), Choice::Pass]);
    let view = PlayerView::new(&g, 0);
    assert_eq!(view.deck_known_top().next(), Some(id::SILVER), "the view shows the gained card on top");
    assert_eq!(strat.decide(&view, &d, &ch), Choice::Pass, "a Silver is not trash");
}
