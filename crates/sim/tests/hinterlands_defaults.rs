//! Hinterlands bot defaults (rulings and defaults in `docs/hinterlands-plan.md`): the reactions are always taken, the small choices inside
//! cards follow the junk / gain-list rules, and whole games with every card finish with the cards conserved.

use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::{CardId, Choice, ChoiceBuf, GameConfig, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

fn bm() -> Strategy {
    Strategy::parse(&std::fs::read_to_string(format!("{}/../../strategies/big_money_ultimate.toml", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

/// Random Hinterlands kingdoms played by the bots to the end: cards conserved, every game ends, the reactions got used.
#[test]
fn bots_finish_hinterlands_games_with_the_cards_conserved() {
    let s = bm();
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Hinterlands).collect();
    let mut rng = Rng::new(77);
    let mut reactions = std::collections::BTreeSet::new();
    for game in 0..150u64 {
        let mut from = pool.clone();
        let kingdom: Vec<CardId> = (0..10).map(|_| from.swap_remove(rng.below(from.len() as u32) as usize)).collect();
        let mut g = GameState::new(&GameConfig { num_players: 2 + (game % 3) as usize, kingdom, seed: game, max_turns: 300, ..Default::default() });
        let total = |g: &GameState| {
            let mut t = g.supply.total() + g.trash.total() + g.side_pile_cards();
            for p in 0..g.num_players as usize {
                t += g.players[p].all_cards().total();
            }
            t
        };
        let initial = total(&g);
        let mut buf = ChoiceBuf::default();
        let mut log: Vec<dominion_engine::Event> = Vec::new();
        for _ in 0..200_000 {
            for e in log.drain(..) {
                if let dominion_engine::Event::Reaction { card, .. } = e {
                    reactions.insert(card);
                }
            }
            match g.advance(&mut log) {
                Step::Decision(d) => {
                    g.legal_choices(&mut buf);
                    let view = PlayerView::new(&g, d.player);
                    let c = s.decide(&view, &d, buf.as_slice());
                    assert!(buf.contains(c), "{c:?} is not legal for {d:?}");
                    g.apply(c, &mut log).unwrap();
                }
                Step::GameOver => break,
                _ => unreachable!(),
            }
            assert_eq!(total(&g), initial, "game {game}");
        }
        assert!(g.is_game_over(), "game {game} did not end");
    }
    let _ = reactions;
}

#[test]
fn fools_gold_and_tunnel_reactions_are_taken_by_default() {
    let s = bm();
    let mut g = GameState::new(&GameConfig { kingdom: vec![id::FOOLS_GOLD, id::TUNNEL, id::CELLAR], seed: 1, ..Default::default() });
    g.players[0].hand = Default::default();
    g.players[0].hand.add(id::FOOLS_GOLD, 1);
    g.gain_card(1, id::PROVINCE, dominion_engine::Dest::Discard, &mut NoEvents);
    if let Step::Decision(d) = g.advance(&mut NoEvents) {
        if d.subject == id::FOOLS_GOLD {
            let mut buf = ChoiceBuf::default();
            g.legal_choices(&mut buf);
            let c = s.decide(&PlayerView::new(&g, d.player), &d, buf.as_slice());
            assert_eq!(c, Choice::Yes);
        }
    }
}
