//! Menagerie tests (rulings in `docs/menagerie-plan.md`): Exile, the Horse pile, the Ways, the Events and the 30 kingdom cards.

mod common;
use common::*;
use dominion_engine::cards::{self, id, CardSet};
use dominion_engine::rng::Rng;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

// ===========================================================================
// Random games: nothing is lost, nothing panics, every game ends
// ===========================================================================

#[test]
fn random_menagerie_games_conserve_cards_and_terminate() {
    let mut rng = Rng::new(0x3E4A);
    let pool: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) == CardSet::Menagerie).collect();
    assert_eq!(pool.len(), 30);
    let others: Vec<CardId> = cards::kingdom_cards().filter(|&c| cards::set_of(c) != CardSet::Menagerie).collect();
    let landscapes: Vec<CardId> = cards::pseudo_cards_of(CardSet::Menagerie).collect();
    assert_eq!(landscapes.len(), 40);
    let mut played = std::collections::BTreeSet::new();
    let mut bought = std::collections::BTreeSet::new();
    let mut ways = std::collections::BTreeSet::new();
    let games: u64 = std::env::var("MENAGERIE_GAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    for game in 0..games {
        let n = 2 + (game % 4) as usize;
        let mut kingdom: Vec<CardId> = Vec::new();
        let mut from = pool.clone();
        for _ in 0..8 {
            let i = rng.below(from.len() as u32) as usize;
            kingdom.push(from.swap_remove(i));
        }
        for _ in 0..2 {
            let c = others[rng.below(others.len() as u32) as usize];
            if !kingdom.contains(&c) {
                kingdom.push(c);
            }
        }
        // Up to two landscapes of the set (at most one Way).
        let mut ls = landscapes.clone();
        let mut ways_in = 0;
        for _ in 0..(game % 3) {
            let i = rng.below(ls.len() as u32) as usize;
            let l = ls.swap_remove(i);
            if cards::is_way(l) {
                if ways_in == 1 {
                    continue;
                }
                ways_in += 1;
            }
            kingdom.push(l);
        }
        let mut g = GameState::new(&GameConfig { num_players: n, kingdom: kingdom.clone(), seed: game, max_turns: 300, ..Default::default() });
        let initial = total_cards(&g);
        let mut buf = ChoiceBuf::default();
        let mut steps = 0;
        let mut log: Vec<Event> = Vec::new();
        let mut recent: Vec<String> = Vec::new();
        loop {
            for e in &log {
                match *e {
                    Event::Play { card, .. } | Event::PlayAgain { card, .. } | Event::StartPlay { card, .. } => {
                        played.insert(card);
                    }
                    Event::Buy { card, .. } => {
                        bought.insert(card);
                    }
                    Event::Way { way, .. } => {
                        ways.insert(way);
                    }
                    _ => {}
                }
                recent.push(format!("{e:?}"));
            }
            if recent.len() > 60 {
                recent.drain(..recent.len() - 60);
            }
            log.clear();
            let chosen;
            match g.advance(&mut log) {
                Step::Decision(d) => {
                    // Some games are rich, so that the $10 Events are bought too.
                    if d.kind == DecisionKind::Buy && game % 4 == 0 && g.turn.coins < 10 {
                        g.turn.coins = 10;
                    }
                    g.legal_choices(&mut buf);
                    if buf.is_empty() {
                        panic!("no legal choice in game {game} for {d:?}\nkingdom {:?}\nrecent {recent:#?}", kingdom.iter().map(|&c| cards::name(c)).collect::<Vec<_>>());
                    }
                    let c = buf.as_slice()[rng.below(buf.len() as u32) as usize];
                    chosen = Some((d, c));
                    recent.push(format!("{d:?} -> {c:?}"));
                    g.apply(c, &mut log).unwrap();
                }
                Step::Chance { .. } | Step::TurnStart { .. } => unreachable!(),
                Step::GameOver => break,
            }
            if total_cards(&g) != initial {
                let mut sums = g.supply;
                sums.add_all(&g.trash);
                for p in 0..g.num_players as usize {
                    sums.add_all(&g.players[p].all_cards());
                }
                let diff: Vec<String> = sums.iter().map(|(c, k)| format!("{}:{k}", cards::name(c))).collect();
                panic!(
                    "card conservation, game {game}, chose {chosen:?}: {log:?}\nzones: {diff:?}\nside {} (started with {initial}, now {})\nkingdom {:?}\nrecent {recent:#?}",
                    g.side_pile_cards(),
                    total_cards(&g),
                    kingdom.iter().map(|&c| cards::name(c)).collect::<Vec<_>>()
                );
            }
            steps += 1;
            assert!(steps < 300_000, "game {game} did not terminate");
        }
    }
    if games >= 3000 {
        let missing: Vec<&str> = (cards::id::BLACK_CAT..=cards::id::HORSE).filter(|c| !played.contains(c)).map(cards::name).collect();
        assert!(missing.is_empty(), "never played: {missing:?}");
        let missing: Vec<&str> = (cards::id::DELAY..=cards::id::POPULATE).filter(|c| !bought.contains(c)).map(cards::name).collect();
        assert!(missing.is_empty(), "never bought: {missing:?}");
        let missing: Vec<&str> = (cards::FIRST_WAY..cards::FIRST_WAY + cards::NUM_WAYS as CardId).filter(|c| !ways.contains(c)).map(cards::name).collect();
        assert!(missing.is_empty(), "never used: {missing:?}");
    }
}
