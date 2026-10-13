//! The search's transposition-table key must not change: `HashCache::turn_hash` (which resumes from the previous state's zones) is
//! `turn_hash`, and the table key is a pure function of the state, whatever was hashed before.

use dominion_engine::cards;
use dominion_engine::rng::Rng;
use dominion_engine::{ChoiceBuf, GameConfig, GameState, NoEvents, Step};
use dominion_search::{turn_hash, HashCache};

fn kingdom(names: &[&str]) -> Vec<cards::CardId> {
    names.iter().map(|n| cards::by_name(n).unwrap_or_else(|| panic!("no card {n}"))).collect()
}

#[test]
fn cached_hash_equals_plain_hash_through_random_games() {
    let kingdoms = [
        kingdom(&["Village", "Smithy", "Militia", "Witch", "Market", "Laboratory", "Moat", "Cellar", "Moneylender", "Chapel"]),
        kingdom(&["Kitsune", "Samurai", "Root Cellar", "Page", "Duplicate", "Distant Lands", "Artificer", "Dungeon", "Aristocrat", "Artist"]),
    ];
    let mut rng = Rng::new(11);
    // One cache for every state of every game, as a Searcher keeps one across nodes and searches.
    let mut cache = HashCache::default();
    let mut states = 0;
    for (k, kingdom) in kingdoms.iter().enumerate() {
        for seed in 0..12 {
            let mut g = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed: seed + 100 * k as u64, ..Default::default() });
            loop {
                match g.advance(&mut NoEvents) {
                    Step::Decision(_) => {
                        let mut buf = ChoiceBuf::default();
                        g.legal_choices(&mut buf);
                        for me in 0..2u8 {
                            let plain = turn_hash(&g, me);
                            assert_eq!(cache.turn_hash(&g, me), plain);
                            // Asking again, and for the other seat and back, still agrees.
                            assert_eq!(cache.turn_hash(&g, me), plain);
                        }
                        states += 1;
                        let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                        g.apply(c, &mut NoEvents).unwrap();
                    }
                    Step::GameOver => break,
                    _ => {}
                }
            }
        }
    }
    assert!(states > 1000, "{states} states");
}
