use dominion_engine::cards::{self, NUM_CARDS};
use dominion_engine::rng::Rng;
use dominion_engine::*;

/// Total cards across supply, trash and every player zone.
fn total_cards(g: &GameState) -> u32 {
    let mut t = g.supply.total() + g.trash.total();
    for p in 0..g.num_players as usize {
        t += g.players[p].all_cards().total();
    }
    t
}

#[test]
fn random_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(42);
    for game in 0..3000u64 {
        let n = 2 + (game % 5) as usize;
        let mut kingdom: Vec<CardId> = cards::kingdom_cards().collect();
        for i in (1..kingdom.len()).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            kingdom.swap(i, j);
        }
        kingdom.truncate(10);
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom, seed: game, max_turns: 400 });
        let start = total_cards(&g) + 10 * n as u32; // starting decks come from outside supply... except Copper
        let _ = start;
        let initial = total_cards(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        loop {
            match g.advance(&mut NoEvents) {
                Step::Decision(_) => {
                    g.legal_choices(&mut buf);
                    assert!(!buf.is_empty());
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut NoEvents).unwrap();
                }
                Step::Chance { .. } => unreachable!(),
                Step::GameOver => break,
            }
            assert_eq!(total_cards(&g), initial, "card conservation, game {game}");
            steps += 1;
            assert!(steps < 200_000, "game {game} did not terminate");
        }
        let _ = NUM_CARDS;
    }
}
