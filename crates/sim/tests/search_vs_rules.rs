//! Positions where the turn search used to choose worse than the strategy's own rules (see `docs/search-in-league.md`, "Cards where the full
//! bot is worse than the fast one"): the search valued what lands outside the turn (a Miser's mat, a Tactician's next hand, a thinner deck,
//! VP tokens, a Pillage trashing itself) as nothing, and passed on cards the rules play. Each test is a position taken from a full-vs-fast game.

use dominion_engine::cards::{id, CardId};
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

/// "BM + X" as the league's baseline builds it: Big Money Ultimate with `x` at a count of one (and two once the deck has 14 cards), played always.
fn bm_plus(x: &str) -> Strategy {
    Strategy::parse(&format!(
        "name = \"T\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Duchy\"\nif = \"provinces_left <= 4\"\n[[gain]]\ncard = \"Estate\"\nif = \"provinces_left <= 2\"\n\
         [[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"{x}\"\nif = \"count({x}) < 1\"\n[[gain]]\ncard = \"{x}\"\nif = \"count({x}) < 2 and total_cards >= 14\"\n\
         [[gain]]\ncard = \"Silver\"\n[[play]]\ncard = \"{x}\"\n"
    ))
    .unwrap()
}

fn position(x: &str, p1: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: {x}, Village, Smithy, Market, Cellar, Militia, Moat, Workshop, Mine, Laboratory\nturn: 9  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\n{p1}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    );
    parse_state(&text).unwrap()
}

fn next(g: &mut GameState) -> (dominion_engine::Decision, Vec<Choice>) {
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    (d, buf.as_slice().to_vec())
}

fn plays(x: &str, c: CardId, p1: &str) {
    let s = bm_plus(x);
    let mut g = position(x, p1);
    let (d, ch) = next(&mut g);
    assert!(ch.contains(&Choice::Card(c)) && ch.contains(&Choice::Pass), "{d:?} {ch:?}");
    let full = s.decide(&PlayerView::new(&g, d.player), &d, &ch);
    assert_eq!(full, Choice::Card(c), "the search must play the card the rules play");
}

#[test]
fn pillage_trashing_itself_is_not_a_loss() {
    plays("Pillage", id::PILLAGE, "hand: Copper, Silver, Estate, Estate, Pillage\ndeck: 3 Copper, 2 Silver, Estate\ndiscard: 3 Copper, Silver, Gold, Province");
}

#[test]
fn trashers_are_played_when_the_trash_is_junk() {
    plays("Chapel", id::CHAPEL, "hand: Copper, Estate, Estate, Estate, Chapel\ndeck: 3 Copper, 3 Silver, 2 Gold, 2 Estate");
    plays("Temple", id::TEMPLE, "hand: Copper, Silver, Silver, Estate, Temple\ndeck: 6 Copper, Silver, 2 Estate");
}

#[test]
fn bishop_tokens_are_worth_something() {
    plays("Bishop", id::BISHOP, "hand: Silver, Silver, Gold, Bishop\ndeck: 5 Copper, 2 Silver, Estate");
}

#[test]
fn miser_builds_a_mat_when_the_buy_is_the_same() {
    let s = bm_plus("Miser");
    let mut g = position("Miser", "hand: Copper, Silver, Silver, Miser\ndeck: 6 Copper, Silver, 2 Estate");
    let (d, ch) = next(&mut g);
    g.apply(Choice::Card(id::MISER), &mut NoEvents).unwrap();
    let _ = (d, ch);
    let (d, ch) = next(&mut g);
    assert!(matches!(d.kind, dominion_engine::DecisionKind::Mode { .. }), "{d:?}");
    // Mode 0 puts a Copper on the mat; the Silvers alone still buy a Silver.
    let c = s.decide(&PlayerView::new(&g, d.player), &d, &ch);
    assert_eq!(c, Choice::Mode(0), "{ch:?}");
}

#[test]
fn tactician_is_valued_by_next_turns_hand() {
    // A hand that buys nothing on the list: the rules play it, and so must the search (the next hand is five cards bigger).
    plays("Tactician", id::TACTICIAN, "hand: Copper, Estate, Estate, Estate, Tactician\ndeck: 6 Copper, 3 Silver, Gold, 2 Estate\ndiscard: Copper");
}

#[test]
fn haven_sets_aside_what_the_buy_does_not_need() {
    let s = bm_plus("Haven");
    let mut g = position("Haven", "hand: Copper, Silver, Gold, Estate, Haven\ndeck: 6 Estate");
    let (_, _) = next(&mut g);
    g.apply(Choice::Card(id::HAVEN), &mut NoEvents).unwrap();
    // The card drawn is random; the choice that follows is the set-aside.
    let (d, ch) = next(&mut g);
    let c = s.decide(&PlayerView::new(&g, d.player), &d, &ch);
    // $6 buys a Gold only with all three Treasures: the dead Estate goes aside.
    assert_eq!(c, Choice::Card(id::ESTATE), "{d:?} {ch:?} -> {c:?}");
}

#[test]
fn a_card_that_trashes_itself_is_used_up_not_lost() {
    let mut g = position("Pillage", "hand: Copper, Silver, Estate, Estate, Pillage\ndeck: 5 Copper");
    let _ = next(&mut g);
    g.apply(Choice::Card(id::PILLAGE), &mut NoEvents).unwrap();
    let _ = next(&mut g);
    assert_eq!(g.turn.used_up.get(id::PILLAGE), 1);
}
