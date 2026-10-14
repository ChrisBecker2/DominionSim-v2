//! The search's transposition-table key must not change: `HashCache::turn_hash` (which resumes from the previous state's zones) is
//! `turn_hash`, and the table key is a pure function of the state, whatever was hashed before.

use dominion_engine::cards;
use dominion_engine::rng::Rng;
use dominion_engine::{ChoiceBuf, GameConfig, GameState, NoEvents, Step};
use dominion_search::{turn_hash, HashCache};

fn kingdom(names: &[&str]) -> Vec<cards::CardId> {
    names.iter().map(|n| cards::by_name(n).unwrap_or_else(|| panic!("no card {n}"))).collect()
}

/// The children of a decision (what the search hashes one after another) hash like `turn_hash` with one cache, whatever the order: a gain changes
/// one lane of the discard pile, the blocks before it are resumed.
#[test]
fn cached_hash_of_every_child_of_every_decision_equals_plain_hash() {
    let kingdom = kingdom(&["Village", "Smithy", "Militia", "Witch", "Market", "Laboratory", "Moat", "Cellar", "Moneylender", "Chapel"]);
    let mut rng = Rng::new(5);
    let mut cache = HashCache::default();
    let mut children = 0;
    for seed in 0..10 {
        let mut g = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.clone(), seed, ..Default::default() });
        loop {
            match g.advance(&mut NoEvents) {
                Step::Decision(d) => {
                    let me = d.player;
                    let mut buf = ChoiceBuf::default();
                    g.legal_choices(&mut buf);
                    assert_eq!(cache.turn_hash(&g, me), turn_hash(&g, me));
                    // Children in the order given, then in reverse, then the parent again.
                    let mut order: Vec<_> = buf.as_slice().to_vec();
                    order.extend(buf.as_slice().iter().rev());
                    for c in order {
                        let mut child = g;
                        child.apply(c, &mut NoEvents).unwrap();
                        assert_eq!(cache.turn_hash(&child, me), turn_hash(&child, me), "seed {seed}, choice {c:?}");
                        children += 1;
                    }
                    assert_eq!(cache.turn_hash(&g, me), turn_hash(&g, me));
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut NoEvents).unwrap();
                }
                Step::GameOver => break,
                _ => {}
            }
        }
    }
    assert!(children > 5000, "{children} children");
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
