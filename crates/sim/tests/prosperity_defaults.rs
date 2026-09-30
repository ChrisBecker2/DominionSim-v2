//! Default bot behavior for step-2 Prosperity cards' decisions when the strategy states no rule
//! for them (see docs/seaside-prosperity-plan.md; "Bots follow stated rules first; defaults rank
//! below them"):
//!   - `PlayTreasure`: play a has-choice Treasure that isn't Bank before Pass; Bank last.
//!   - Investment's mode: +$1 unless the hand already holds 3+ differently-named Treasures.
//!   - War Chest's `Name` (as the player to the left): deny the costliest legal card.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

fn decide(strat: &Strategy, g: &GameState, player: u8, d: &Decision) -> Choice {
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide_by_rules(&PlayerView::new(g, player), d, buf.as_slice())
}

fn next_decision(g: &mut GameState) -> Decision {
    match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    }
}

/// A 2-player state with `kingdom`, player 1's `hand`, at the start of their Buy phase.
fn buy_phase_state(kingdom: &str, hand: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: {kingdom}\nturn: 1  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n\
         [player 1]\nhand: {hand}\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    parse_state(&text).unwrap()
}

// `search_play = false`: these tests target the rule-order defaults specifically, not the
// (much stronger, but slower and less predictable) turn search that `search_play = true` uses.
const BASIC: &str = "name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Province\"\n";

#[test]
fn play_treasure_default_prefers_a_has_choice_treasure_over_passing() {
    let strat = Strategy::parse(BASIC).unwrap();
    let mut g = buy_phase_state("Anvil", "Anvil, Copper");
    let d = next_decision(&mut g); // Copper auto-plays; Anvil offered
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    assert_eq!(decide(&strat, &g, 0, &d), Choice::Card(id::ANVIL), "should play Anvil rather than Pass");
}

#[test]
fn play_treasure_default_plays_bank_last() {
    let strat = Strategy::parse(BASIC).unwrap();
    let mut g = buy_phase_state("Bank, Anvil", "Bank, Anvil");
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    assert_eq!(decide(&strat, &g, 0, &d), Choice::Card(id::ANVIL), "Anvil before Bank, so Bank counts more treasures");
    g.apply(Choice::Card(id::ANVIL), &mut NoEvents).unwrap();
    let d = next_decision(&mut g); // Anvil's own "discard a Treasure?" (only Bank left): decline
    assert_eq!(decide(&strat, &g, 0, &d), Choice::Pass);
    g.apply(Choice::Pass, &mut NoEvents).unwrap();
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    assert_eq!(decide(&strat, &g, 0, &d), Choice::Card(id::BANK), "Bank is the only Treasure left");
}

#[test]
fn investment_default_takes_the_coin_with_few_treasures() {
    let strat = Strategy::parse(BASIC).unwrap();
    let mut g = buy_phase_state("Investment", "Investment, Estate");
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    g.apply(Choice::Card(id::INVESTMENT), &mut NoEvents).unwrap();
    // Investment's mandatory trash has only Estate to offer: auto-resolved (single legal
    // choice), landing straight on the +$1 / trash-for-VP Mode decision.
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Mode { picks: 1, distinct: false });
    assert_eq!(decide(&strat, &g, 0, &d), Choice::Mode(0), "no Treasure diversity to cash in: take +$1");
}

#[test]
fn investment_default_trashes_itself_with_three_or_more_treasure_types() {
    let strat = Strategy::parse(BASIC).unwrap();
    // Anvil, Bank and Tiara are has-choice Treasures, so they're still in hand (unlike
    // Copper/Silver/Gold, which would already have auto-played).
    let mut g = buy_phase_state("Investment, Anvil, Bank, Tiara", "Investment, Estate, Anvil, Bank, Tiara");
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    g.apply(Choice::Card(id::INVESTMENT), &mut NoEvents).unwrap();
    let d = next_decision(&mut g); // Investment's mandatory trash
    assert_eq!(decide(&strat, &g, 0, &d), Choice::Card(id::ESTATE), "trash the non-Treasure junk, not a wanted Treasure");
    g.apply(Choice::Card(id::ESTATE), &mut NoEvents).unwrap();
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Mode { picks: 1, distinct: false });
    assert_eq!(decide(&strat, &g, 0, &d), Choice::Mode(1), "3 differently-named Treasures: trash for VP");
}

#[test]
fn war_chest_naming_default_denies_the_costliest_legal_card() {
    let strat = Strategy::parse(BASIC).unwrap();
    // Player 2 (index 1) is to player 1's left, so they answer the Name decision.
    let text = "players: 2\nkingdom: War Chest\nturn: 1  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n\
                [player 1]\nhand: War Chest\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure);
    g.apply(Choice::Card(id::WAR_CHEST), &mut NoEvents).unwrap();
    let d = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Name);
    assert_eq!(d.player, 1); // 0-indexed seat for text-format player 2
    // Duchy and War Chest both cost $5, the most affordable in this plain kingdom+basics
    // supply; ties break toward the higher card id (War Chest).
    assert_eq!(decide(&strat, &g, 1, &d), Choice::Card(id::WAR_CHEST));
}
