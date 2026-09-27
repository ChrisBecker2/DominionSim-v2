//! Test scenario helpers shared by the engine's integration tests.
#![allow(dead_code)]

use dominion_engine::state::{FrameStack, PlayerState, TurnState};
use dominion_engine::*;

/// Build a `Counts` multiset from a list of card ids (repeats allowed).
pub fn counts_of(cards: &[CardId]) -> Counts {
    let mut c = Counts::EMPTY;
    for &x in cards {
        c.add(x, 1);
    }
    c
}

/// A state with the given kingdom cards (any subset, not necessarily 10), reset to a clean
/// Action-phase turn for player 0 (stack empty, no pending decision) with every player's
/// zones wiped to empty. Tests build exact scenarios with `set_hand`/`set_deck_known`/etc.;
/// nothing from the (discarded) initial deal leaks through, so unset zones read as empty.
pub fn new_state(kingdom: &[CardId], num_players: usize) -> GameState {
    let cfg = GameConfig { num_players, kingdom: kingdom.to_vec(), seed: 0x5EED_1234, max_turns: 500 };
    let mut g = GameState::new(&cfg);
    // Drain the initial 5-card deals (lands on some early decision, which we discard) so the
    // engine is in a well-formed post-setup state before we clear it back out below.
    let _ = g.advance(&mut NoEvents);
    reset_turn(&mut g, 0);
    for p in 0..num_players {
        g.players[p] = PlayerState::default();
    }
    g
}

/// Force a clean Action-phase turn for `player`: 1 action, 1 buy, 0 coins, empty effect
/// stack, no pending decision. Does not touch player zones or the supply.
pub fn reset_turn(g: &mut GameState, player: u8) {
    let number = g.turn.number.max(1);
    g.turn = TurnState::start(player, number);
    g.stack = FrameStack::default();
    g.pending = Pending::None;
}

pub fn set_hand(g: &mut GameState, p: usize, cards: &[CardId]) {
    g.players[p].hand = counts_of(cards);
}
pub fn set_discard(g: &mut GameState, p: usize, cards: &[CardId]) {
    g.players[p].discard = counts_of(cards);
}
pub fn set_in_play(g: &mut GameState, p: usize, cards: &[CardId]) {
    g.players[p].in_play = counts_of(cards);
}
pub fn set_set_aside(g: &mut GameState, p: usize, cards: &[CardId]) {
    g.players[p].set_aside = counts_of(cards);
}

/// `top_to_bottom[0]` is the very next card that will be drawn.
pub fn set_deck_known(g: &mut GameState, p: usize, top_to_bottom: &[CardId]) {
    let ps = &mut g.players[p];
    ps.deck_known.clear();
    for &c in top_to_bottom.iter().rev() {
        ps.deck_known.push_top(c);
    }
}
pub fn set_deck_unknown(g: &mut GameState, p: usize, cards: &[CardId]) {
    g.players[p].deck_unknown = counts_of(cards);
}
pub fn clear_deck(g: &mut GameState, p: usize) {
    g.players[p].deck_known.clear();
    g.players[p].deck_unknown = Counts::EMPTY;
}

pub fn set_supply(g: &mut GameState, card: CardId, n: u8) {
    g.supply.set(card, n);
}
pub fn empty_pile(g: &mut GameState, card: CardId) {
    g.supply.set(card, 0);
}

/// Total cards across supply, trash and every player zone (a conservation invariant).
pub fn total_cards(g: &GameState) -> u32 {
    let mut t = g.supply.total() + g.trash.total();
    for p in 0..g.num_players as usize {
        t += g.players[p].all_cards().total();
    }
    t
}

pub fn adv(g: &mut GameState) -> Step {
    g.advance(&mut NoEvents)
}

/// Apply a choice and drive the engine to the next decision/chance/game-over. `apply` only
/// performs the single chosen mutation (it does not loop), so any cascading effect (a
/// continuation frame, an auto-single-applied follow-up decision, a phase transition) needs
/// an explicit `advance` afterwards — this does that for you, which is what every test wants.
pub fn choose(g: &mut GameState, c: Choice) -> Step {
    g.apply(c, &mut NoEvents).expect("expected a legal choice");
    adv(g)
}

/// Same as `choose`, but records events through `sink` instead of discarding them.
pub fn choose_ev<S: EventSink>(g: &mut GameState, c: Choice, sink: &mut S) -> Step {
    g.apply(c, sink).expect("expected a legal choice");
    g.advance(sink)
}

pub fn choose_and_advance(g: &mut GameState, c: Choice) -> Step {
    choose(g, c)
}

/// Current legal choices for the pending decision.
pub fn choices(g: &GameState) -> Vec<Choice> {
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    buf.as_slice().to_vec()
}

/// Expect the pending step to be a decision and return it (advancing first).
pub fn expect_decision(g: &mut GameState) -> Decision {
    match adv(g) {
        Step::Decision(d) => d,
        other => panic!("expected a decision, got {other:?}"),
    }
}

/// From the Action-phase PlayAction decision, play `card`. Returns the next `Step`
/// (the card's own effects resolve automatically up to the next decision/chance/game-over).
pub fn play(g: &mut GameState, card: CardId) -> Step {
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::PlayAction, "expected PlayAction decision");
    choose_and_advance(g, Choice::Card(card))
}

/// From the Buy-phase decision, buy `card`.
pub fn buy(g: &mut GameState, card: CardId) -> Step {
    let d = expect_decision(g);
    assert_eq!(d.kind, DecisionKind::Buy, "expected Buy decision");
    choose_and_advance(g, Choice::Card(card))
}

/// Pass on whatever decision is currently pending.
pub fn pass(g: &mut GameState) -> Step {
    choose_and_advance(g, Choice::Pass)
}

/// Repeatedly pick `card` while the pending decision is a `Select { from, act, .. }` (some of
/// the picks may be auto-applied internally by `auto_single` before we ever see them — this is
/// robust to that, unlike hard-coding an exact number of manual `choose` calls).
pub fn pick_while(g: &mut GameState, from: Zone, act: Act, card: CardId) {
    loop {
        match g.pending_decision() {
            Some(Decision { kind: DecisionKind::Select { from: f, act: a, .. }, .. }) if f == from && a == act => {
                if !choices(g).contains(&Choice::Card(card)) {
                    break;
                }
                choose(g, Choice::Card(card));
            }
            _ => break,
        }
    }
}
