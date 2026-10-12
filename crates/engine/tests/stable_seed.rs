//! `PlayerView::stable_seed` feeds `DefaultHasher` through a buffer (`agent::BufferedSip`) for speed. SipHash is a function of the byte
//! stream, so the seed (and with it every determinization, hence every searched bot decision) must be what hashing straight into a plain
//! `DefaultHasher` gives. This is that plain version, compared over the positions of some played games.

use dominion_engine::rng::Rng;
use dominion_engine::{ChoiceBuf, GameConfig, GameState, NoEvents, PlayerView, Step};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn plain_seed(s: &GameState, me: u8) -> u64 {
    let mut h = DefaultHasher::new();
    me.hash(&mut h);
    s.players[me as usize].hash(&mut h);
    s.turn.hash(&mut h);
    s.supply.hash(&mut h);
    s.trash.hash(&mut h);
    s.stack.hash(&mut h);
    for p in 0..s.num_players {
        if p != me {
            let ps = &s.players[p as usize];
            ps.all_cards().hash(&mut h);
            ps.hand.total().hash(&mut h);
            ps.discard.hash(&mut h);
            ps.in_play.hash(&mut h);
        }
    }
    h.finish()
}

#[test]
fn the_buffered_seed_is_the_plain_seed() {
    let kingdoms: [&[&str]; 3] = [
        &["Village", "Smithy", "Market", "Militia", "Moat", "Cellar", "Remodel", "Laboratory", "Mine", "Witch"],
        &["Ball", "Page", "Peasant", "Raze", "Archive", "Haunted Woods", "Lost City", "Messenger", "Miser", "Storyteller"],
        &["Cultist", "Knights", "Castles", "Chapel", "Moat", "Ghost Town", "Fool", "Guardian", "Vampire", "Werewolf"],
    ];
    let mut checked = 0;
    for (k, names) in kingdoms.iter().enumerate() {
        let kingdom: Vec<_> = names.iter().map(|n| dominion_engine::cards::by_name(n).unwrap_or_else(|| panic!("no card {n}"))).collect();
        for n in [2usize, 4] {
            let mut g = GameState::new(&GameConfig { num_players: n, kingdom: kingdom.clone(), seed: 7 + k as u64, ..Default::default() });
            let mut rng = Rng::new(k as u64);
            let mut buf = ChoiceBuf::default();
            for step in 0..3000 {
                match g.advance(&mut NoEvents) {
                    Step::Decision(d) => {
                        g.legal_choices(&mut buf);
                        let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                        // Every few decisions: the seed as seen by each player.
                        if step % 9 == 0 {
                            for me in 0..n as u8 {
                                assert_eq!(PlayerView::new(&g, me).stable_seed(), plain_seed(&g, me), "kingdom {k}, {n} players, step {step}, seat {me}, {:?}", d.kind);
                                checked += 1;
                            }
                        }
                        g.apply(c, &mut NoEvents).unwrap();
                    }
                    Step::GameOver => break,
                    _ => {}
                }
            }
        }
    }
    assert!(checked > 200, "only {checked} positions were compared");
}
