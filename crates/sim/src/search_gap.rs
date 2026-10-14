//! Selective search: which cards' decisions a fast bot still searches.
//!
//! `GAPS` lists the cards for which a full bot (turn search) beats a fast bot (rules only) by at least 3 points playing
//! "Big Money + the card" against itself on the card plus nine Base cards (`examples/search_gap.rs`, 4000 games): the win rate of a
//! bot that searches only the decisions involving that card, minus 50, and the CPU cost of such a game relative to a fast game
//! (measured over 1000 games). The whole table, with the cards that show no gap, is in `docs/search-in-league.md`.
use dominion_engine::cards;

/// (card, gap in win-rate points, game cost relative to a fast game), largest gap first.
pub const GAPS: &[(&str, f32, f32)] = &[
    // Artificer: two copies in the mirror show 1.4 points, but with four the full bot beats the fast one 59-63%.
    ("Artificer", 10.0, 33.0),
    ("Rats", 49.9, 208.1),
    ("Overlord", 49.3, 2109.3),
    ("Band of Misfits", 39.7, 868.0),
    ("Amulet", 39.3, 14.8),
    ("Stonemason", 38.6, 5.2),
    ("Anvil", 37.5, 68.4),
    ("Crown", 34.6, 76.9),
    ("Root Cellar", 33.8, 741.4),
    ("Remake", 30.1, 3.5),
    ("Royal Blacksmith", 28.5, 650.5),
    ("Develop", 27.1, 4.7),
    ("Investment", 26.7, 63.9),
    ("Tanuki", 26.3, 6.0),
    ("Lookout", 26.0, 128.8),
    ("Crystal Ball", 23.0, 624.1),
    ("Trader", 19.9, 6.4),
    ("Forager", 19.3, 9.2),
    ("Trading Post", 17.5, 5.0),
    ("Tormentor", 17.4, 206.7),
    ("Remodel", 15.6, 5.3),
    ("Dismantle", 15.4, 9.0),
    ("Captain", 14.6, 1816.3),
    ("Count", 14.2, 24.3),
    ("Courtier", 13.2, 15.9),
    ("Steward", 13.1, 64.6),
    ("Encampment", 12.9, 156.7),
    ("Apprentice", 12.5, 208.2),
    ("Graverobber", 12.0, 10.7),
    ("Craftsman", 11.8, 5.8),
    ("Tiara", 11.4, 79.7),
    ("Rice Broker", 10.9, 49.8),
    ("Necromancer", 10.7, 141.0),
    ("Counterfeit", 10.6, 45.2),
    ("Forge", 10.3, 6.4),
    ("Salvager", 10.2, 6.7),
    ("Treasure Map", 9.2, 5.3),
    ("Junk Dealer", 8.9, 20.3),
    ("Raze", 8.8, 28.5),
    ("Replace", 8.6, 6.6),
    ("Butcher", 8.5, 12.3),
    ("Altar", 8.1, 5.3),
    ("Fool", 7.2, 27198.0),
    ("Catacombs", 7.1, 6420.1),
    ("Peasant", 7.1, 371.6),
    ("Pawn", 6.9, 168.5),
    ("Gold Mine", 6.6, 22.9),
    ("Upgrade", 6.3, 10.9),
    ("Squire", 5.7, 18.7),
    ("Rebuild", 5.6, 5624.4),
    ("Governor", 5.2, 1322.2),
    ("River Shrine", 4.9, 6.7),
    ("Ferryman", 4.8, 467.3),
    ("Leprechaun", 4.0, 6331.3),
    ("Minion", 3.8, 10408.9),
    ("Spice Merchant", 3.7, 50.0),
    ("Kitsune", 3.7, 33.0),
    ("Expand", 3.6, 3.9),
    ("Druid", 3.6, 6707.2),
    ("Farrier", 3.5, 2.0),
    ("Idol", 3.5, 3613.2),
    ("Infirmary", 3.2, 17.2),
];

/// The cards (by id) whose decisions are searched: gap at least `min_gap` points and cost at most `max_cost` times a fast game.
pub fn mask(min_gap: f64, max_cost: f64) -> Vec<bool> {
    let mut m = vec![false; cards::NUM_CARDS];
    for &(name, gap, cost) in GAPS {
        if gap as f64 >= min_gap && cost as f64 <= max_cost {
            if let Some(c) = cards::by_name(name) {
                m[c as usize] = true;
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_follows_the_thresholds() {
        let id = |n| cards::by_name(n).unwrap() as usize;
        let m = mask(3.0, 100.0);
        assert!(m[id("Remodel")] && m[id("Artificer")]);
        assert!(!m[id("Village")], "no gap, not searched");
        assert!(!m[id("Minion")], "too expensive");
        assert!(mask(3.0, 1e9)[id("Minion")]);
        assert!(!mask(30.0, 100.0)[id("Remodel")]);
    }
}
