//! Bot default policies for Seaside's Duration decisions (plan step 3). Most of these decisions
//! (Haven's set-aside pick, Sailor's own optional trash, Tide Pools'/Sea Witch's forced discard)
//! turn out to already be covered by `Strategy`'s existing generic, shape-driven defaults
//! (`Act::SetAside`, `Act::Trash`, `Act::Discard`) with no Seaside-specific code needed — these
//! tests pin down that those defaults do the sensible thing for the new decision shapes.

use dominion_engine::text::parse_state;
use dominion_engine::{id, Act, Choice, ChoiceBuf, DecisionKind, NoEvents, PlayerView, Step};
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
