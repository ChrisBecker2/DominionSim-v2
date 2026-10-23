mod common;
use common::*;
use dominion_engine::cards::{self, id};
use dominion_engine::*;

fn junk() -> Vec<CardId> {
    [id::ESTATE, id::DUCHY, id::PROVINCE].iter().cycle().take(40).copied().collect()
}

#[test]
fn p1_warrior_discards_tunnel_reveal_gold() {
    let mut g = new_state(&[id::PAGE, id::TUNNEL], 2);
    set_hand(&mut g, 0, &[id::WARRIOR]);
    set_deck_known(&mut g, 0, &junk());
    set_deck_known(&mut g, 1, &[id::TUNNEL, id::COPPER]);
    play(&mut g, id::WARRIOR);
    let d = g.pending_decision();
    eprintln!("pending after warrior: {d:?}; trash={:?} discard={:?}", g.trash.total(), g.players[1].discard.total());
    assert!(matches!(d, Some(Decision { kind: DecisionKind::YesNo { .. }, player: 1, .. })), "{d:?}");
    choose(&mut g, Choice::Yes);
    assert_eq!(g.players[1].discard.get(id::GOLD), 1);
    assert_eq!(g.trash.get(id::TUNNEL), 1);
}

#[test]
fn p2_duplicate_not_for_debt_cost() {
    let mut g = new_state(&[id::DUPLICATE, id::ENGINEER], 2);
    g.players[0].tavern.add(id::DUPLICATE, 1);
    set_deck_known(&mut g, 0, &junk());
    g.turn.phase = Phase::Buy;
    g.turn.coins = 0;
    adv(&mut g);
    buy(&mut g, id::ENGINEER);
    let d = g.pending_decision();
    eprintln!("after engineer buy: {d:?}");
    assert!(!matches!(d, Some(Decision { kind: DecisionKind::YesNo { act: Act::Call }, .. })), "{d:?}");
}

#[test]
fn p3_giant_down_still_opens_reaction() {
    let mut g = new_state(&[id::GIANT, id::MOAT], 2);
    set_hand(&mut g, 0, &[id::GIANT]);
    set_hand(&mut g, 1, &[id::MOAT]);
    set_deck_known(&mut g, 0, &junk());
    let s = play(&mut g, id::GIANT);
    eprintln!("{s:?} {:?}", g.pending_decision());
    assert!(matches!(g.pending_decision(), Some(Decision { player: 1, .. })));
}
