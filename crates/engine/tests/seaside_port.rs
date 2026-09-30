//! Seaside (2nd edition) card tests, ported from the old C++ simulator's
//! `DominionSimTestCards\SeasideCardsTests.cpp` (1st-edition Seaside) where the card exists in
//! 2nd edition, adapted to this engine's API. Each test cites the C++ `TEST_METHOD` it came from.
//! Tests for cards new in 2nd edition are written from the card text. See
//! `docs/seaside-prosperity-plan.md` §4.
//!
//! Adapting C++ assertions: `VerifyTurnBasics(actions, buys, coins)` there is checked before
//! treasures are played; this engine auto-plays choice-free treasures on entering the Buy phase.
//!
//! Not ported (1st edition only): TestSeahag, TestExplorer, TestNavigator, TestAmbassador,
//! TestGhostShip, TestPirateShip, TestPearlDiver, TestEmbargo. "Simulation" sub-cases test the
//! C++ bot harness and aren't ported.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::*;

// ===========================================================================
// Bazaar — C++ TestBazaar
// ===========================================================================

#[test]
fn bazaar_card_two_actions_and_a_coin() {
    let d = cards::def(id::BAZAAR);
    assert_eq!((d.cost, d.vp), (5, 0));
    assert_eq!(cards::set_of(id::BAZAAR), CardSet::Seaside);
    // Regular play: MakeGame<PlayFirstAction>({Bazaar}, {Gold, Gold}): hand {Gold}, deck {Gold},
    // (2, 1, $1) before treasures; the drawn Gold then auto-plays for $1 + $3.
    let mut g = new_state(&[id::BAZAAR], 2);
    set_hand(&mut g, 0, &[id::BAZAAR]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    play(&mut g, id::BAZAAR);
    assert_eq!((g.turn.actions, g.turn.buys), (2, 1));
    assert_eq!(g.turn.coins, 1 + 3);
    assert_eq!(g.players[0].in_play.get(id::GOLD), 1);
    assert_eq!(g.players[0].deck_known.peek_top(), Some(id::GOLD));

    // No cards to draw.
    let mut g = new_state(&[id::BAZAAR], 2);
    set_hand(&mut g, 0, &[id::BAZAAR]);
    play(&mut g, id::BAZAAR);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 1, 1));
    assert!(g.players[0].hand.is_empty());
}
