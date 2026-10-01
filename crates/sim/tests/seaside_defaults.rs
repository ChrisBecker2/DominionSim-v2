//! Bot default policies for Seaside's Duration decisions (plan step 3). Most of these decisions
//! (Haven's set-aside pick, Sailor's own optional trash, Tide Pools'/Sea Witch's forced discard)
//! turn out to already be covered by `Strategy`'s existing generic, shape-driven defaults
//! (`Act::SetAside`, `Act::Trash`, `Act::Discard`) with no Seaside-specific code needed — these
//! tests pin down that those defaults do the sensible thing for the new decision shapes.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Act, Choice, ChoiceBuf, DecisionKind, Dest, Filter, NoEvents, PlayerView, Step, Zone};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
}

/// Parse `text`, drive to the first pending decision, and return the strategy's pick for it.
fn decide(strat: &Strategy, text: &str) -> Choice {
    let mut g = parse_state(text).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice())
}

const KINGDOM: &str = "Haven, Sailor, Tide Pools, Sea Witch, Pirate, Clerk";

/// Build a text state with the given player-1 fields already at a pending decision (a
/// `[[player 1]] durations: ...` line, or plain hand/discard fields the caller supplies).
fn state(fields: &str) -> String {
    format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\n{fields}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    )
}

// ---------------------------------------------------------------------------------------
// Haven: set aside the best card you can't use this turn.
// ---------------------------------------------------------------------------------------

#[test]
fn haven_sets_aside_the_most_valuable_card() {
    let strat = load("big_money_ultimate.toml");
    // Hand after Haven's own +1 Card +1 Action leaves {Gold, Copper, Estate} to choose from.
    let text = state("hand: Haven, Gold, Copper, Estate\ndeck: Duchy\nin play:");
    let mut g = parse_state(&text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::HAVEN), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::Select { from: dominion_engine::Zone::Hand, act: Act::SetAside, filter: dominion_engine::Filter::Any, min: 1, max: 1, ordered: false });
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Card(id::GOLD), "the Gold is the most valuable card to keep for next turn");
}

// ---------------------------------------------------------------------------------------
// Sailor: trash per trash rules (its own optional next-turn trash reuses `Act::Trash`).
// ---------------------------------------------------------------------------------------

#[test]
fn sailor_next_turn_trash_follows_the_trash_rules() {
    let strat = load("big_money_ultimate.toml"); // default trash rules: Curse, Estate, Copper
    // 3 Coppers: the default `keep_treasure = 2` leaves a budget of 1 Copper to trash optionally.
    let text = state("hand: 3 Copper\ndurations: Sailor\nin play: Sailor");
    let choice = decide(&strat, &text);
    assert_eq!(choice, Choice::Card(id::COPPER));
}

#[test]
fn sailor_next_turn_trash_declines_when_nothing_is_wanted() {
    let strat = load("big_money_ultimate.toml");
    // Only 1 Copper, below `keep_treasure`: the default optional-trash rule leaves it alone.
    let text = state("hand: Copper\ndurations: Sailor\nin play: Sailor");
    let choice = decide(&strat, &text);
    assert_eq!(choice, Choice::Pass);
}

// ---------------------------------------------------------------------------------------
// Tide Pools / Sea Witch: their forced next-turn discard reuses the existing discard defaults
// (dead weight — Curse/Victory — first, then cheapest).
// ---------------------------------------------------------------------------------------

/// Drive a forced 2-card discard to completion and return the one card left in hand. The
/// canonical (non-decreasing) pick order can force a lower-id card to be offered first even when
/// a higher-id one is more "wanted" (e.g. Copper before Estate here) — what matters is the *set*
/// discarded overall, not the exact order.
fn resolve_forced_discard(strat: &Strategy, text: &str) -> dominion_engine::CardId {
    let mut g = parse_state(text).unwrap();
    loop {
        let d = match g.advance(&mut NoEvents) {
            Step::Decision(d) => d,
            s => panic!("{s:?}"),
        };
        let mut buf = ChoiceBuf::default();
        g.legal_choices(&mut buf);
        let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
        g.apply(choice, &mut NoEvents).unwrap();
        if g.players[0].hand.total() == 1 {
            return g.players[0].hand.iter().next().unwrap().0;
        }
    }
}

#[test]
fn tide_pools_next_turn_discard_uses_the_existing_discard_defaults() {
    let strat = load("big_money_ultimate.toml");
    let text = state("hand: Estate, Copper, Silver\ndurations: Tide Pools\nin play: Tide Pools");
    let left = resolve_forced_discard(&strat, &text);
    assert_eq!(left, id::SILVER, "dead weight (Estate, Copper) discarded before the Silver");
}

#[test]
fn sea_witch_next_turn_discard_uses_the_existing_discard_defaults() {
    let strat = load("big_money_ultimate.toml");
    let text = state("hand: Estate, Copper, Silver\ndurations: Sea Witch\nin play: Sea Witch");
    let left = resolve_forced_discard(&strat, &text);
    assert_eq!(left, id::SILVER);
}

// ---------------------------------------------------------------------------------------
// Pirate / Clerk: play the optional reaction when it's good. The existing default for any
// "you may play this" `YesNo` (`Act::Play`) is "almost always worth taking" — Yes.
// ---------------------------------------------------------------------------------------

#[test]
fn pirate_plays_when_offered() {
    use dominion_engine::state::{GameConfig, GameState};
    use dominion_engine::CardId;
    let strat = load("big_money_ultimate.toml");
    let kingdom: Vec<CardId> = vec![id::PIRATE];
    let mut g = GameState::new(&GameConfig { num_players: 2, kingdom, seed: 1, max_turns: 50 });
    let _ = g.advance(&mut NoEvents); // drain the initial deals
    g.players[0].hand = dominion_engine::Counts::EMPTY;
    g.players[0].hand.add(id::PIRATE, 1);
    g.turn = dominion_engine::state::TurnState::start(1, g.turn.number); // player 2's turn
    g.turn.coins = 3;
    g.stack = dominion_engine::state::FrameStack::default();
    g.pending = dominion_engine::Pending::None;
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_))); // player 2's Buy decision
    g.apply(Choice::Card(id::SILVER), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("Pirate's reaction was never offered: {s:?}"),
    };
    assert_eq!(d.player, 0);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Yes);
}

#[test]
fn clerk_reveals_at_the_start_of_the_turn() {
    let strat = load("big_money_ultimate.toml");
    let text = state("hand: Clerk\ndeck: Copper");
    let mut g = parse_state(&text).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Yes);
}

// ---------------------------------------------------------------------------------------
// Step 4: Native Village, Lookout, Smugglers, Treasury.
// ---------------------------------------------------------------------------------------

const STEP4_KINGDOM: &str = "Native Village, Lookout, Smugglers, Treasury, Witch, Militia";

fn step4_state(fields: &str) -> String {
    format!(
        "players: 2\nkingdom: {STEP4_KINGDOM}\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
         [player 1]\n{fields}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    )
}

#[test]
fn native_village_adds_to_an_empty_mat() {
    let strat = load("big_money_ultimate.toml");
    let text = step4_state("hand: Native Village\ndeck: Gold");
    let mut g = parse_state(&text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::NATIVE_VILLAGE), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    // `rule_mode` (not `decide`): bypasses the `search_play` turn-search wrapper so this checks
    // the hand-written default heuristic directly, not a searched outcome (meaningless here
    // anyway, since adding to vs. taking an empty mat are equally good no-ops).
    let choice = strat.rule_mode(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Mode(0), "nothing on the mat yet: keep adding");
}

#[test]
fn native_village_takes_the_mat_once_it_holds_two_cards() {
    let strat = load("big_money_ultimate.toml");
    let mut g = parse_state(&step4_state("hand: Native Village\ndeck: Gold\nnative village: Silver, Copper")).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::NATIVE_VILLAGE), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.rule_mode(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Mode(1), "2+ cards already on the mat: take them");
}

#[test]
fn native_village_takes_a_single_card_when_this_turn_needs_it() {
    let strat = load("big_money_ultimate.toml");
    // 1 card on the mat and no actions left to spend on digging further: worth taking now.
    let text = "players: 2\nkingdom: Native Village\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n\
                [player 1]\nhand: Native Village\nnative village: Silver\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::NATIVE_VILLAGE), &mut NoEvents).unwrap();
    // Native Village's own +2 Actions already applied by the time the mode decision is asked, so
    // `turn.actions` isn't 0 here; the "thin hand" branch is what fires instead — hand is empty
    // (Native Village was the only card) once it's been played.
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.rule_mode(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Mode(1), "hand is down to nothing else to do: take the 1 card now");
}

#[test]
fn lookout_trashes_and_discards_by_value() {
    // Reuses the existing generic trash/discard defaults (Curse/Estate/Copper priority, then
    // cheapest): no Lookout-specific code needed, same as the step-3 doc header's point.
    let strat = load("big_money_ultimate.toml");
    let mut g = parse_state(&step4_state("hand: Lookout\ndeck top: Gold, Copper, Curse")).unwrap();
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::LOOKOUT), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Revealed, act: Act::Trash, filter: Filter::Any, min: 1, max: 1, ordered: false });
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Card(id::CURSE), "Curse is the worst card to keep");
    g.apply(choice, &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    g.legal_choices(&mut buf);
    let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Card(id::COPPER), "then the cheaper of the two treasures, keeping Gold");
}

#[test]
fn smugglers_follows_the_gain_list() {
    // Reuses the existing generic Gain default (the strategy's own gain-priority list): no
    // Smugglers-specific code needed.
    let strat = load("big_money_ultimate.toml"); // gain list: Province, Duchy?, Gold, Estate?, Silver
    let mut g = parse_state(&step4_state("hand: Smugglers")).unwrap();
    for &c in &[id::SILVER, id::GOLD, id::ESTATE] {
        g.players[1].last_turn_gains.add(c, 1);
    }
    assert!(matches!(g.advance(&mut NoEvents), Step::Decision(_)));
    g.apply(Choice::Card(id::SMUGGLERS), &mut NoEvents).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::Gain { max_cost: 6, filter: Filter::Any, dest: Dest::Discard, exact: false, potion: false, optional: false });
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Card(id::GOLD), "Gold ranks above Estate and Silver in the gain list");
}

#[test]
fn treasury_is_put_back_by_default() {
    let strat = load("big_money_ultimate.toml");
    let text = "players: 2\nkingdom: Treasury\nturn: 1  player: 1  phase: buy  actions: 0  buys: 0  coins: 0\n\n\
                [player 1]\nhand:\ndeck: 5 Copper\nin play: Treasury\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n";
    let mut g = parse_state(text).unwrap();
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("{s:?}"),
    };
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Topdeck });
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    let choice = strat.decide(&PlayerView::new(&g, 0), &d, buf.as_slice());
    assert_eq!(choice, Choice::Yes);
}
