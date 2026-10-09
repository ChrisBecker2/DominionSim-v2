//! How a bot chooses what Band of Misfits plays (an Action from the Supply costing less than it,
//! left there). With `search_play` (the default) the choice is searched like any play decision:
//! each option's whole turn is played out and scored by the strategy (what the turn gains, by
//! gain-list priority; stated [[play]] rules as a small bonus). Without search: the stated
//! [[play]] rules in order, then the generic play ranking.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

const KINGDOM: &str = "Band of Misfits, Remodel, Smithy, Village, Workshop, Market Square, Cellar, Moat, Festival, Laboratory";

/// Player 1 plays Band of Misfits from `hand`; returns the state at its "play which Action from
/// the Supply" decision.
fn at_band_choice(hand: &str, deck: &str) -> (GameState, Decision) {
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\nhand: {hand}\ndeck: {deck}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    let mut g = parse_state(&text).unwrap();
    match g.advance(&mut NoEvents) {
        Step::Decision(d) if d.kind == DecisionKind::PlayAction => {}
        s => panic!("expected the action phase, got {s:?}"),
    }
    g.apply(Choice::Card(id::BAND_OF_MISFITS), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected Band of Misfits' choice, got {s:?}"),
    };
    assert!(
        matches!(d.kind, DecisionKind::Select { act: dominion_engine::Act::Play, .. }),
        "Band of Misfits asks which Action to play: {:?}",
        d.kind
    );
    (g, d)
}

fn pick(strat: &Strategy, g: &GameState, d: &Decision) -> Choice {
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let c = strat.decide(&PlayerView::new(g, 0), d, buf.as_slice());
    assert!(buf.as_slice().contains(&c), "{c:?} is a legal choice");
    c
}

fn offered(g: &GameState) -> Vec<Choice> {
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    buf.as_slice().to_vec()
}

const MONEY: &str = "name = \"Money\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n";

#[test]
fn only_cheaper_non_duration_actions_in_the_supply_are_offered() {
    let (g, _) = at_band_choice("Band of Misfits, Gold, Copper, Copper, Copper", "5 Copper");
    let o = offered(&g);
    for c in [id::REMODEL, id::SMITHY, id::VILLAGE, id::WORKSHOP, id::CELLAR, id::MOAT, id::MARKET_SQUARE] {
        assert!(o.contains(&Choice::Card(c)), "{c} costs less than $5");
    }
    for c in [id::FESTIVAL, id::LABORATORY, id::BAND_OF_MISFITS] {
        assert!(!o.contains(&Choice::Card(c)), "{c} costs $5: not less than Band of Misfits");
    }
}

#[test]
fn the_search_plays_remodel_to_turn_a_gold_into_a_province() {
    // Remodel: trash the Gold, gain a Province ($6 + 2), then $3 buys a Silver. Smithy would draw
    // three Estates: $6, only a Gold. The gain list ranks the Province far above.
    let (g, d) = at_band_choice("Band of Misfits, Gold, Copper, Copper, Copper", "5 Estate");
    let strat = Strategy::parse(MONEY).unwrap();
    assert_eq!(pick(&strat, &g, &d), Choice::Card(id::REMODEL));
}

#[test]
fn the_search_draws_nothing_when_a_gain_is_worth_more() {
    // $4 in hand, Estates below: Workshop's Silver plus the $4 Silver beats Remodel (a Copper into
    // nothing the list wants) and Smithy (three Estates).
    let (g, d) = at_band_choice("Band of Misfits, Copper, Copper, Copper, Copper", "5 Estate");
    let strat = Strategy::parse(MONEY).unwrap();
    assert_eq!(pick(&strat, &g, &d), Choice::Card(id::WORKSHOP));
}

#[test]
fn the_search_remodels_an_estate_into_a_silver_over_workshop() {
    // No money: Workshop and Remodel both end with a Silver, but Remodel also trashes an Estate
    // (the default trash rules want it gone).
    let (g, d) = at_band_choice("Band of Misfits, Estate, Estate, Estate, Estate", "5 Estate");
    let strat = Strategy::parse(MONEY).unwrap();
    assert_eq!(pick(&strat, &g, &d), Choice::Card(id::REMODEL));
}

#[test]
fn the_search_draws_when_drawing_reaches_a_better_buy() {
    // $5 in hand and Golds on top: Smithy draws $9 for a Province; Workshop's Silver is worse.
    let (g, d) = at_band_choice("Band of Misfits, Copper, Copper, Copper, Silver", "Gold, Gold, Gold");
    let strat = Strategy::parse(MONEY).unwrap();
    assert_eq!(pick(&strat, &g, &d), Choice::Card(id::SMITHY));
}

#[test]
fn without_search_the_stated_play_rules_decide() {
    let src = MONEY.replace("name = \"Money\"\n", "name = \"Rules\"\nsearch_play = false\n") + "[[play]]\ncard = \"Workshop\"\n";
    let strat = Strategy::parse(&src).unwrap();
    // Even where Remodel would be better: rules-only bots follow their stated order.
    let (g, d) = at_band_choice("Band of Misfits, Gold, Copper, Copper, Copper", "5 Copper");
    assert_eq!(pick(&strat, &g, &d), Choice::Card(id::WORKSHOP));
}

#[test]
fn a_stated_play_rule_tips_a_choice_the_turn_scores_equally() {
    // Nothing to gain either way (an all-Estate hand and deck, and no Workshop target the list
    // wants): the [[play]] rule's bonus picks the named card.
    let src = "name = \"Provinces only\"\n[[gain]]\ncard = \"Province\"\n[[play]]\ncard = \"Smithy\"\n";
    let strat = Strategy::parse(src).unwrap();
    let (g, d) = at_band_choice("Band of Misfits, Estate, Estate, Estate, Estate", "5 Estate");
    assert_eq!(pick(&strat, &g, &d), Choice::Card(id::SMITHY));
}

#[test]
fn without_search_or_rules_it_still_plays_a_legal_card() {
    let src = MONEY.replace("name = \"Money\"\n", "name = \"Rules\"\nsearch_play = false\n");
    let strat = Strategy::parse(&src).unwrap();
    let (g, d) = at_band_choice("Band of Misfits, Gold, Copper, Copper, Copper", "5 Copper");
    let c = pick(&strat, &g, &d);
    assert_ne!(c, Choice::Pass, "Band of Misfits must play something when a card qualifies");
}
