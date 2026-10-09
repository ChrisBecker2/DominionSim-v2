//! `GameState::empty_piles` (a 32-lane walk of the supply) against the plain one-pile-at-a-time count.

mod common;
use common::*;
use dominion_engine::cards::{self, id};
use dominion_engine::rng::Rng;
use dominion_engine::*;

#[test]
fn fast_empty_pile_count_matches_the_plain_scan_in_random_games() {
    let mut rng = Rng::new(0xE177);
    let all: Vec<CardId> = cards::kingdom_cards().collect();
    for game in 0..400u64 {
        let n = 2 + (game % 5) as usize;
        let mut kingdom: Vec<CardId> = Vec::new();
        // Some games hold Knights, Ruins-givers and the Sauna/Avanto split pile on purpose.
        let specials = [id::KNIGHTS, id::CULTIST, id::SAUNA, id::YOUNG_WITCH];
        if game % 3 == 0 {
            kingdom.push(specials[(game / 3 % 4) as usize]);
        }
        while kingdom.len() < 10 {
            let c = all[rng.below(all.len() as u32) as usize];
            if !kingdom.contains(&c) {
                kingdom.push(c);
            }
        }
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom, seed: game, max_turns: 120, ..Default::default() });
        let mut buf = ChoiceBuf::default();
        loop {
            assert_eq!(g.empty_piles(), g.empty_piles_scan(), "game {game}");
            match g.advance(&mut NoEvents) {
                Step::Decision(_) => {
                    g.legal_choices(&mut buf);
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    g.apply(c, &mut NoEvents).unwrap();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
        }
    }
}

#[test]
fn fast_empty_pile_count_matches_for_every_single_empty_pile() {
    for kingdom in [vec![id::SAUNA, id::KNIGHTS, id::CULTIST, id::SMITHY], vec![id::SMITHY, id::VILLAGE, id::PLATINUM, id::COLONY]] {
        let mut g = new_state(&kingdom, 2);
        let piles: Vec<CardId> = g.supply_cards().collect();
        for &c in &piles {
            empty_pile(&mut g, c);
            assert_eq!(g.empty_piles(), g.empty_piles_scan(), "{} emptied", cards::name(c));
        }
        for &c in &piles {
            set_supply(&mut g, c, 3);
            assert_eq!(g.empty_piles(), g.empty_piles_scan(), "{} refilled", cards::name(c));
        }
    }
}
