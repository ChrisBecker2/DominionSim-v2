//! Default bot behavior for step-2 Intrigue cards' `YesNo` decisions when the strategy states no
//! rule for them (see docs/intrigue-plan.md, "Bots follow stated rules first; defaults rank
//! below them"):
//!   - Baron: discard an Estate for +$4 by default.
//!   - Mining Village: trash itself only if the strategy's `[[trash]]` rules name it.

use dominion_engine::cards::{self, ModeOpt};
use dominion_engine::text::parse_state;
use dominion_engine::{id, Act, CardId, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

/// Index of `card`'s mode-table entry matching `pred`.
fn mode_idx(card: CardId, pred: impl Fn(&ModeOpt) -> bool) -> u8 {
    cards::modes(card).iter().position(|o| pred(o)).expect("mode option exists") as u8
}

/// Apply `c` and drive to the next decision.
fn apply_and_next(g: &mut GameState, c: Choice) -> Decision {
    g.apply(c, &mut NoEvents).unwrap();
    match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    }
}

fn decide_for(strat: &Strategy, g: &GameState, player: u8, d: &Decision) -> Choice {
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide_by_rules(&PlayerView::new(g, player), d, buf.as_slice())
}

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

// ===========================================================================
// `[[mode]]` rule parsing: priority order, conditions, and the unknown-label error.
// ===========================================================================

#[test]
fn mode_rule_parses_and_applies_by_priority_and_condition() {
    let strat = Strategy::parse(
        r#"
        name = "T"
        search_play = false
        [[gain]]
        card = "Silver"
        [[mode]]
        card = "Steward"
        choose = "+2 Cards"
        if = "provinces_left <= 4"
        [[mode]]
        card = "steward"
        choose = "  trash 2 CARDS  "
        "#,
    )
    .unwrap();
    let cards_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::Cards(_)));
    let trash_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::TrashFromHand(_)));

    // Provinces are still plentiful (default 2-player pile is 8): the 1st rule's condition is
    // false, so the 2nd stated rule (trash, matched case/space-insensitively) applies.
    let (g, d) = decide_after_play("Steward", "Steward, Copper, Estate", "5 Copper", id::STEWARD);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(trash_i));

    // With few Provinces left, the 1st rule's condition holds and takes priority.
    let mut g2 = g;
    g2.supply.set(id::PROVINCE, 4);
    assert_eq!(decide(&strat, &g2, &d), Choice::Mode(cards_i));
}

#[test]
fn unknown_mode_choice_is_a_parse_error_listing_valid_labels() {
    let err = Strategy::parse(
        r#"
        name = "T"
        [[mode]]
        card = "Steward"
        choose = "Do a barrel roll"
        "#,
    )
    .unwrap_err();
    assert!(err.contains("Do a barrel roll"), "{err}");
    assert!(err.contains("+2 Cards"), "{err}");
    assert!(err.contains("+$2"), "{err}");
    assert!(err.contains("Trash 2 cards"), "{err}");
}

// ===========================================================================
// Fast rule-order defaults (`search_play = false`), per docs/intrigue-plan.md.
// ===========================================================================

#[test]
fn pawn_default_prefers_cards_then_coins_unless_an_action_is_needed() {
    let strat = Strategy::parse("name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let cards_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Cards(_)));
    let coins_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Coins(_)));
    let actions_i = mode_idx(id::PAWN, |o| matches!(o, ModeOpt::Actions(_)));

    // No other Action in hand: +1 Card, then +$1.
    let (mut g, d) = decide_after_play("Pawn", "Pawn, Estate", "5 Copper", id::PAWN);
    let c1 = decide(&strat, &g, &d);
    assert_eq!(c1, Choice::Mode(cards_i));
    let d2 = apply_and_next(&mut g, c1);
    assert_eq!(decide(&strat, &g, &d2), Choice::Mode(coins_i));

    // An unplayed Action in hand and no actions left: +1 Action first.
    let (g, d) = decide_after_play("Pawn, Village", "Pawn, Village, Estate", "5 Copper", id::PAWN);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(actions_i));
}

#[test]
fn steward_default_trashes_when_two_or_more_cards_are_wanted_gone() {
    let strat = Strategy::parse("name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let trash_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::TrashFromHand(_)));
    let cards_i = mode_idx(id::STEWARD, |o| matches!(o, ModeOpt::Cards(_)));

    // 2+ cards the (default) trash rules want gone: Curse, Estate.
    let (g, d) = decide_after_play("Steward", "Steward, Curse, Estate", "5 Copper", id::STEWARD);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(trash_i));

    // Nothing the default trash rules want: +2 Cards instead.
    let (g, d) = decide_after_play("Steward", "Steward, Silver, Duchy", "5 Copper", id::STEWARD);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(cards_i));
}

#[test]
fn nobles_default_actions_with_two_or_more_actions_in_hand() {
    let strat = Strategy::parse("name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let actions_i = mode_idx(id::NOBLES, |o| matches!(o, ModeOpt::Actions(_)));
    let cards_i = mode_idx(id::NOBLES, |o| matches!(o, ModeOpt::Cards(_)));

    let (g, d) = decide_after_play("Nobles, Village", "Nobles, Village, Village", "5 Copper", id::NOBLES);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(actions_i));

    let (g, d) = decide_after_play("Nobles", "Nobles, Estate, Duchy", "5 Copper", id::NOBLES);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(cards_i));
}

#[test]
fn minion_default_redraws_only_when_four_new_cards_beat_the_hand_plus_two() {
    let strat = Strategy::parse("name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Province\"\n").unwrap();
    let coins_i = mode_idx(id::MINION, |o| matches!(o, ModeOpt::Coins(_)));
    let hand_i = mode_idx(id::MINION, |o| matches!(o, ModeOpt::DiscardHandDraw { .. }));

    // A hand of Estates ($0 + $2) vs 4 Coppers from the deck ($4): redraw.
    let (g, d) = decide_after_play("Minion", "Minion, Estate, Estate", "5 Copper", id::MINION);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(hand_i));

    // Treasure in hand counts even though it isn't played until the buy phase: Gold + Silver
    // ($5 + $2 = $7) beats 4 Coppers ($4).
    let (g, d) = decide_after_play("Minion", "Minion, Gold, Silver, Estate", "5 Copper", id::MINION);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(coins_i));

    // Nothing worth drawing: keep the +$2.
    let (g, d) = decide_after_play("Minion", "Minion, Estate, Estate, Estate, Estate", "5 Estate", id::MINION);
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(coins_i));
}

#[test]
fn courtier_default_priority_is_coins_then_gain_then_buys_then_actions() {
    let strat = Strategy::parse("name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let coins_i = mode_idx(id::COURTIER, |o| matches!(o, ModeOpt::Coins(_)));
    let gain_i = mode_idx(id::COURTIER, |o| matches!(o, ModeOpt::Gain(c, _) if *c == id::GOLD));

    // Witch (Action-Attack, 2 types) is the only card in hand, so it's auto-revealed and the
    // decision lands straight on a 2-pick Mode decision.
    let (mut g, d) = decide_after_play("Courtier, Witch", "Courtier, Witch", "5 Copper", id::COURTIER);
    assert_eq!(d.kind, DecisionKind::Mode { picks: 2, distinct: true });
    let c1 = decide(&strat, &g, &d);
    assert_eq!(c1, Choice::Mode(coins_i));
    let d2 = apply_and_next(&mut g, c1);
    assert_eq!(decide(&strat, &g, &d2), Choice::Mode(gain_i));
}

#[test]
fn lurker_default_gains_a_wanted_action_from_trash_else_trashes_from_supply() {
    let strat = Strategy::parse("name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Village\"\n").unwrap();
    let gain_i = mode_idx(id::LURKER, |o| matches!(o, ModeOpt::GainFromTrash(_)));
    let trash_i = mode_idx(id::LURKER, |o| matches!(o, ModeOpt::TrashFromSupply(_)));

    // The trash holds a Village, which the gain list wants: gain it.
    let text = "players: 2\nkingdom: Lurker, Village, Market\ntrash: Village\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Lurker\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::LURKER));
    assert_eq!(decide(&strat, &g, &d), Choice::Mode(gain_i));

    // Nothing wanted in the trash: trash from the Supply instead.
    let text2 = "players: 2\nkingdom: Lurker, Village, Market\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Lurker\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g2 = parse_state(text2).unwrap();
    assert!(matches!(g2.advance(&mut NoEvents), Step::Decision(_)));
    let d2 = apply_and_next(&mut g2, Choice::Card(id::LURKER));
    assert_eq!(decide(&strat, &g2, &d2), Choice::Mode(trash_i));
}

#[test]
fn torturer_victim_default_discards_junk_else_gains_the_curse() {
    let strat = Strategy::parse("name = \"T\"\nsearch_play = false\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let discard_i = mode_idx(id::TORTURER, |o| matches!(o, ModeOpt::DiscardFromHand(_)));
    let curse_i = mode_idx(id::TORTURER, |o| matches!(o, ModeOpt::Gain(c, _) if *c == id::CURSE));

    // The victim's hand has 2 expendable cards (a Copper and a Victory card): discard.
    let text = "players: 2\nkingdom: Torturer\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Torturer\ndeck: 5 Copper\n\n[player 2]\nhand: Copper, Estate, Gold\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::TORTURER));
    assert_eq!(d.player, 1);
    assert_eq!(decide_for(&strat, &g, 1, &d), Choice::Mode(discard_i));

    // Fewer than 2 expendable cards (only the Duchy counts): gain the Curse instead.
    let text2 = "players: 2\nkingdom: Torturer\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Torturer\ndeck: 5 Copper\n\n[player 2]\nhand: Gold, Silver, Duchy\ndeck: 5 Estate\n";
    let mut g2 = parse_state(text2).unwrap();
    assert!(matches!(g2.advance(&mut NoEvents), Step::Decision(_)));
    let d2 = apply_and_next(&mut g2, Choice::Card(id::TORTURER));
    assert_eq!(decide_for(&strat, &g2, 1, &d2), Choice::Mode(curse_i));
}

#[test]
fn stated_mode_rule_overrides_the_torturer_default() {
    // Stated `[[mode]]` rules for Torturer take priority over the default heuristic, even when
    // the default would have picked the opposite option.
    let strat = Strategy::parse(
        r#"
        name = "T"
        search_play = false
        [[gain]]
        card = "Silver"
        [[mode]]
        card = "Torturer"
        choose = "Gain a Curse to your hand"
        "#,
    )
    .unwrap();
    let curse_i = mode_idx(id::TORTURER, |o| matches!(o, ModeOpt::Gain(c, _) if *c == id::CURSE));
    let text = "players: 2\nkingdom: Torturer\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Torturer\ndeck: 5 Copper\n\n[player 2]\nhand: Copper, Estate, Gold\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::TORTURER));
    // The default heuristic would discard here (2 expendable cards); the stated rule overrides it.
    assert_eq!(decide_for(&strat, &g, 1, &d), Choice::Mode(curse_i));
}

// ===========================================================================
// Step 4/5 defaults (hidden information / reactions), per docs/intrigue-plan.md.
// ===========================================================================

#[test]
fn wishing_well_default_names_the_most_likely_card() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    // The unknown deck is 3 Silver + 1 Gold; whichever one the +1 Card draw happens to take,
    // Silver is still the majority of what's left, so naming it is correct either way.
    let text = "players: 2\nkingdom: Wishing Well\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Wishing Well\ndeck: 3 Silver, Gold\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::WISHING_WELL));
    assert_eq!(d.kind, DecisionKind::Name);
    assert_eq!(decide(&strat, &g, &d), Choice::Card(id::SILVER), "3 of the 4 remaining cards are Silver");
}

#[test]
fn secret_passage_default_keeps_a_good_card_on_top() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let text = "players: 2\nkingdom: Secret Passage\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Secret Passage, Estate, Gold\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::SECRET_PASSAGE));
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::SetAside, .. }));
    let c = decide(&strat, &g, &d);
    assert_eq!(c, Choice::Card(id::GOLD), "the single best card in hand");
    let d2 = apply_and_next(&mut g, c);
    assert!(matches!(d2.kind, DecisionKind::DeckPosition { .. }));
    assert_eq!(decide(&strat, &g, &d2), Choice::Position(0), "kept on top for next turn");
}

#[test]
fn secret_passage_default_buries_pure_junk_at_the_bottom() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let text = "players: 2\nkingdom: Secret Passage\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Secret Passage, Estate, Curse\ndeck: 5 Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::SECRET_PASSAGE));
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::SetAside, .. }));
    let c = decide(&strat, &g, &d);
    let d2 = apply_and_next(&mut g, c);
    assert!(matches!(d2.kind, DecisionKind::DeckPosition { .. }));
    assert_eq!(decide(&strat, &g, &d2), Choice::Position(255), "nothing but junk to place: bury it");
}

#[test]
fn swindler_default_prefers_a_curse_over_a_copper_at_cost_zero() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let text = "players: 2\nkingdom: Swindler\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Swindler\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck top: Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::SWINDLER));
    assert_eq!(d.player, 0, "the attacker decides");
    assert_eq!(d.for_player, 1, "for the victim");
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 0, exact: true, .. }));
    assert_eq!(decide_for(&strat, &g, 0, &d), Choice::Card(id::CURSE), "the worst option at cost 0 is a Curse");
}

#[test]
fn swindler_default_prefers_a_victory_card_over_an_action_at_the_same_cost() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let text = "players: 2\nkingdom: Swindler, Courtyard\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Swindler\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck top: Estate\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::SWINDLER));
    assert!(matches!(d.kind, DecisionKind::Gain { max_cost: 2, exact: true, .. }));
    assert_eq!(
        decide_for(&strat, &g, 0, &d),
        Choice::Card(id::ESTATE),
        "the worst option at cost 2 is the Victory card, not the Action"
    );
}

#[test]
fn masquerade_default_passes_the_least_valuable_card() {
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let text = "players: 2\nkingdom: Masquerade\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Masquerade, Curse, Silver\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::MASQUERADE));
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Pass, .. }));
    assert_eq!(decide(&strat, &g, &d), Choice::Card(id::CURSE), "the trash-priority order picks Curse first");
}

#[test]
fn diplomat_default_reveals_when_offered() {
    // The reaction is only ever offered once the hand already has 5+ cards, so the default is
    // simply to always take it.
    let strat = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let text = "players: 2\nkingdom: Militia, Diplomat\n\
        turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
        [player 1]\nhand: Militia\ndeck: 5 Copper\n\n\
        [player 2]\nhand: Diplomat, Copper, Copper, Copper, Estate\ndeck: 5 Copper\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    let d = apply_and_next(&mut g, Choice::Card(id::MILITIA));
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Reveal });
    assert_eq!(decide_for(&strat, &g, 1, &d), Choice::Yes);
}
