//! The default don't-lose guard (`Strategy::gain_loses`): a bot does not take the gain that ends the game with it losing (the last Province or
//! Colony, the card that empties the third pile), nor the second-to-last Province when that hands the opponent the game; with a second buy it
//! first takes a card that turns the last Province into a win. Off when a gain rule uses `wins_game` / `loses_game` itself.

use dominion_engine::text::parse_state;
use dominion_engine::{id, CardId, Choice, ChoiceBuf, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

const KINGDOM: &str = "Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop";

fn strategy(gains: &[(&str, &str)]) -> Strategy {
    let mut s = String::from("name = \"T\"\n");
    for (card, cond) in gains {
        s.push_str(&format!("[[gain]]\ncard = \"{card}\"\n"));
        if !cond.is_empty() {
            s.push_str(&format!("if = \"{cond}\"\n"));
        }
    }
    Strategy::parse(&s).unwrap()
}

/// Player 1 (seat 0) at the Buy phase with `hand` and `buys`; `p1` / `p2` are the extra cards each owns (discard pile). Turns taken: 14 and 15,
/// so a tied score is a shared win.
fn state(supply: &str, buys: u8, hand: &str, p1: &str, p2: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nsupply: {supply}\nturn: 30  player: 1  phase: buy  actions: 0  buys: {buys}  coins: 0\n\n[player 1]\nhand: {hand}\ndeck: 5 Copper\ndiscard: {p1}\nturns: 14\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Copper\ndiscard: {p2}\nturns: 15\n"
    ))
    .unwrap()
}

/// The bot's choice at the Buy decision of `g`, and what it is offered.
fn buy(strat: &Strategy, g: &mut GameState) -> Choice {
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::Buy);
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide(&PlayerView::new(g, 0), &d, buf.as_slice())
}

fn pick(strat: &Strategy, mut g: GameState) -> Choice {
    buy(strat, &mut g)
}

fn card(c: CardId) -> Choice {
    Choice::Card(c)
}

#[test]
fn the_card_that_empties_the_third_pile_is_not_taken_when_it_loses() {
    // Cellar and Moat are gone, one Estate is left: taking it ends the game. Behind (0 to 6): skip it, buy the Silver.
    let strat = strategy(&[("Estate", ""), ("Silver", "")]);
    let g = state("Estate=1, Cellar=0, Moat=0", 1, "Silver Silver", "", "Province");
    assert_eq!(pick(&strat, g), card(id::SILVER));
    // Ahead by enough that the Estate wins (1 to 0): taken.
    let g = state("Estate=1, Cellar=0, Moat=0", 1, "Silver Silver", "", "");
    assert_eq!(pick(&strat, g), card(id::ESTATE));
    // A tie (5 to 5) when the game ends now: 15 turns each, so a shared win (equal VP and equal turns), not a loss: taken.
    let g = state("Estate=1, Cellar=0, Moat=0", 1, "Silver Silver", "Duchy Estate Estate", "Duchy Duchy");
    assert_eq!(pick(&strat, g), card(id::ESTATE));
}

#[test]
fn the_last_card_of_a_pile_is_free_when_it_is_not_the_ending_pile() {
    // Only one pile is empty: the last Estate ends nothing.
    let strat = strategy(&[("Estate", ""), ("Silver", "")]);
    let g = state("Estate=1, Cellar=0", 1, "Silver Silver", "", "Province");
    assert_eq!(pick(&strat, g), card(id::ESTATE));
}

#[test]
fn four_player_games_need_a_fourth_pile() {
    let strat = strategy(&[("Estate", ""), ("Silver", "")]);
    // Two empty piles: the Estate would be the third, not the fourth (5 players), so a 2-player game ends but a 5-player one does not.
    let text = "players: 5\nkingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop\nsupply: Estate=1, Cellar=0, Moat=0\nturn: 30  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n[player 1]\nhand: Silver Silver\ndeck: 5 Copper\nturns: 14\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Copper\ndiscard: Province\nturns: 15\n\n[player 3]\nhand: 5 Copper\ndeck: 5 Copper\nturns: 15\n\n[player 4]\nhand: 5 Copper\ndeck: 5 Copper\nturns: 15\n\n[player 5]\nhand: 5 Copper\ndeck: 5 Copper\nturns: 15\n";
    let g = parse_state(text).unwrap();
    assert_eq!(pick(&strat, g), card(id::ESTATE));
}

#[test]
fn a_second_buy_sets_up_a_last_province_that_would_lose_alone() {
    // One Province left, the opponent has 8 VP. The Province alone gives 6: a loss. A Duchy first makes it 9: a win. $14 and two Buys pay for
    // Province + Duchy, and the Gold in between would not help (the list asks for it before the Duchy).
    let strat = strategy(&[("Province", ""), ("Gold", ""), ("Duchy", "")]);
    let mut g = state("Province=1", 2, "Gold Gold Gold Gold Silver", "", "2 Duchy 2 Estate");
    assert_eq!(buy(&strat, &mut g), card(id::DUCHY));
    g.apply(card(id::DUCHY), &mut NoEvents).unwrap();
    // Now the Province wins: bought.
    assert_eq!(buy(&strat, &mut g), card(id::PROVINCE));
}

#[test]
fn no_set_up_without_the_money_or_the_second_buy() {
    let strat = strategy(&[("Province", ""), ("Gold", ""), ("Duchy", "")]);
    // One Buy: Gold (next entry), as before.
    assert_eq!(pick(&strat, state("Province=1", 1, "Gold Gold Gold Gold Silver", "", "2 Duchy 2 Estate")), card(id::GOLD));
    // Two Buys but $12 only: no Duchy + Province (13): Gold.
    assert_eq!(pick(&strat, state("Province=1", 2, "Gold Gold Gold Gold", "", "2 Duchy 2 Estate")), card(id::GOLD));
}

#[test]
fn the_second_to_last_province_is_not_taken_when_it_hands_over_the_game() {
    // Two Provinces left; the opponent has 3 of them (18 VP) and I have none. Taking one makes it 6 to 18; they take the last: 24, a loss.
    // The Duchy next in the list is bought instead.
    let strat = strategy(&[("Province", ""), ("Duchy", ""), ("Gold", "")]);
    assert_eq!(pick(&strat, state("Province=2", 1, "Gold Gold Silver", "", "3 Province")), card(id::DUCHY));
    // Far ahead (24 to 0): after mine, 30 to 0; the last one gives them 6. Taken.
    assert_eq!(pick(&strat, state("Province=2", 1, "Gold Gold Silver", "4 Province", "")), card(id::PROVINCE));
    // Level (0 to 0): 6 to 0 after my gain, the opponent's last one makes it 6 to 6 on their 16th turn against my 15: fewer turns wins the
    // tie, so I win; it is fine.
    assert_eq!(pick(&strat, state("Province=2", 1, "Gold Gold Silver", "", "")), card(id::PROVINCE));
    // Behind by 1 (0 to 1): 6 to 1 after mine, the last gives them 7: a loss.
    assert_eq!(pick(&strat, state("Province=2", 1, "Gold Gold Silver", "", "Estate")), card(id::DUCHY));
}

#[test]
fn the_province_is_kept_when_the_alternative_is_not_worth_points() {
    // Declining for a Gold only stalls (and the opponent in front stays in front): the Province is taken.
    let strat = strategy(&[("Province", ""), ("Gold", "")]);
    assert_eq!(pick(&strat, state("Province=2", 1, "Gold Gold Silver", "", "3 Province")), card(id::PROVINCE));
    // Nothing else listed: the same.
    let strat = strategy(&[("Province", "")]);
    assert_eq!(pick(&strat, state("Province=2", 1, "Gold Gold Silver", "", "3 Province")), card(id::PROVINCE));
}

#[test]
fn two_buys_that_pay_for_both_provinces_are_not_blocked() {
    // $16 and two Buys at the second-to-last Province: the bot may take both (the last one is checked when it gets there).
    let strat = strategy(&[("Province", ""), ("Gold", "")]);
    assert_eq!(pick(&strat, state("Province=2", 2, "Gold Gold Gold Gold Gold Silver Silver Silver", "", "3 Province")), card(id::PROVINCE));
}

#[test]
fn rules_that_name_the_game_end_turn_the_default_guard_off() {
    // A strategy that asks `loses_game` itself decides; a plain `Province` rule next to it is not guarded either.
    let strat = strategy(&[("Province", "loses_game < 1"), ("Gold", "")]);
    assert_eq!(pick(&strat, state("Province=1", 1, "Gold Gold Silver", "", "3 Province")), card(id::GOLD));
    // The second-to-last Province is not guarded by a strategy that handles the end itself.
    assert_eq!(pick(&strat, state("Province=2", 1, "Gold Gold Silver", "", "3 Province")), card(id::PROVINCE));
}

#[test]
fn an_event_in_the_gain_list_is_not_a_pile() {
    // The guard asks about the pile of the card matched; an Event has none.
    let strat = Strategy::parse("name = \"T\"
[[gain]]
card = \"Expedition\"
[[gain]]
card = \"Silver\"
").unwrap();
    let text = format!("players: 2
kingdom: {KINGDOM}
events: Expedition
turn: 30  player: 1  phase: buy  actions: 0  buys: 1  coins: 6

[player 1]
hand: Estate
deck: 5 Copper
turns: 14

[player 2]
hand: 5 Copper
deck: 5 Copper
turns: 15
");
    let g = parse_state(&text).unwrap();
    assert_eq!(pick(&strat, g), card(id::EXPEDITION));
}
