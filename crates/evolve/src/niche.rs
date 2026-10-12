//! Niches (MAP-Elites): a small descriptor of how a strategy plays, and an archive that keeps
//! the best strategy found in each niche.
//!
//! The descriptor comes from cheap statistics gathered while a candidate plays its evaluation
//! games: which kingdom cards it gains most (its top one or two), how many cards it trashes per
//! game (bucketed), and how long its games last (bucketed against a reference length).

use std::collections::BTreeMap;

use dominion_engine::cards::{self, CardId};
use dominion_engine::rng::Rng;
use dominion_sim::batch::Profile;

use crate::genome::Genome;

/// "No second key card" in a [`Descriptor`].
pub const NO_CARD: CardId = CardId::MAX;

/// Tunable cut points of the descriptor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NicheParams {
    /// A kingdom card is a key card only if the strategy gains at least this many per game.
    pub min_gains: f64,
    /// The second key card must be gained at least this fraction as often as the first.
    pub second_ratio: f64,
    /// Trashes per game below the first value: no trashing; below the second: light; else heavy.
    pub trash_cuts: [f64; 2],
    /// Game length relative to the reference: below the first value: fast; above the second: slow.
    pub length_cuts: [f64; 2],
}

impl Default for NicheParams {
    fn default() -> Self {
        NicheParams { min_gains: 0.75, second_ratio: 0.5, trash_cuts: [1.0, 6.0], length_cuts: [0.88, 1.12] }
    }
}

/// The niche of a strategy: key cards (sorted, so Lab+Mine and Mine+Lab are one niche), trashing
/// bucket and length bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Descriptor {
    pub cards: [CardId; 2],
    /// 0 none, 1 light, 2 heavy.
    pub trash: u8,
    /// 0 fast, 1 average, 2 slow.
    pub length: u8,
}

pub const TRASH_LABELS: [&str; 3] = ["no trashing", "light trashing", "heavy trashing"];
pub const LENGTH_LABELS: [&str; 3] = ["fast games", "average games", "long games"];

impl Descriptor {
    /// The key cards by name (empty for a pure money deck).
    pub fn card_names(&self) -> Vec<&'static str> {
        self.cards.iter().filter(|&&c| c != NO_CARD).map(|&c| cards::name(c)).collect()
    }

    /// e.g. "Laboratory + Mine, light trashing, long games"; "Money only, ..." with no key card.
    pub fn label(&self) -> String {
        let names = self.card_names();
        let head = if names.is_empty() { "Money only".to_string() } else { names.join(" + ") };
        format!("{head}, {}, {}", TRASH_LABELS[self.trash as usize], LENGTH_LABELS[self.length as usize])
    }
}

/// What a strategy was seen doing over its evaluation games.
#[derive(Clone, Debug, Default)]
pub struct Behaviour {
    /// Kingdom cards gained per game, most first.
    pub gains: Vec<(CardId, f64)>,
    pub trashes: f64,
    /// Own turns per game.
    pub turns: f64,
}

impl Behaviour {
    /// Summarize a profile; `turns_sum` / `games` is the average number of the strategy's own turns.
    pub fn from_profile(p: &Profile, turns_sum: f64, games: u64) -> Behaviour {
        let mut gains: Vec<(CardId, f64)> = (0..p.gains.len() as CardId)
            .filter(|&c| cards::is_kingdom(c))
            .map(|c| (c, p.gains_per_game(c)))
            .filter(|&(_, g)| g > 0.0)
            .collect();
        gains.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        Behaviour { gains, trashes: p.trashes_per_game(), turns: if games > 0 { turns_sum / games as f64 } else { 0.0 } }
    }

    pub fn describe(&self, reference_turns: f64, prm: &NicheParams) -> Descriptor {
        let mut key = [NO_CARD; 2];
        if let Some(&(c0, g0)) = self.gains.first() {
            if g0 >= prm.min_gains {
                key[0] = c0;
                if let Some(&(c1, g1)) = self.gains.get(1) {
                    if g1 >= prm.min_gains && g1 >= prm.second_ratio * g0 {
                        key[1] = c1;
                    }
                }
            }
        }
        key.sort_unstable();
        let trash = if self.trashes < prm.trash_cuts[0] { 0 } else if self.trashes < prm.trash_cuts[1] { 1 } else { 2 };
        let ratio = if reference_turns > 0.0 { self.turns / reference_turns } else { 1.0 };
        let length = if ratio < prm.length_cuts[0] { 0 } else if ratio > prm.length_cuts[1] { 2 } else { 1 };
        Descriptor { cards: key, trash, length }
    }

    /// "Laboratory 3.2, Mine 1.1 gains/game; 4.0 trashes; 17.3 turns".
    pub fn text(&self) -> String {
        let gains: Vec<String> = self.gains.iter().take(3).map(|&(c, g)| format!("{} {:.1}", cards::name(c), g)).collect();
        format!("{}; {:.1} trashes; {:.1} turns", if gains.is_empty() { "no kingdom cards".to_string() } else { gains.join(", ") }, self.trashes, self.turns)
    }
}

/// The best strategy seen in one niche.
#[derive(Clone, Debug)]
pub struct Elite {
    pub genome: Genome,
    /// Selection fitness (win rate against the league minus parsimony), measured this generation.
    pub fitness: f64,
    pub win_rate: f64,
    pub games: u64,
    pub behaviour: Behaviour,
    pub generation: usize,
    pub island: usize,
}

/// Best-per-niche archive. Iteration order is the descriptor order, so runs are reproducible.
#[derive(Clone, Debug, Default)]
pub struct Archive {
    cells: BTreeMap<Descriptor, Elite>,
}

/// What [`Archive::offer`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// The niche was empty.
    New,
    /// The offered strategy beat the niche's champion.
    Replaced,
    Rejected,
}

impl Archive {
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub fn contains(&self, d: &Descriptor) -> bool {
        self.cells.contains_key(d)
    }
    pub fn get(&self, d: &Descriptor) -> Option<&Elite> {
        self.cells.get(d)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Descriptor, &Elite)> {
        self.cells.iter()
    }

    /// Offer a strategy for niche `d`: kept if the niche is empty or it is fitter than the champion.
    /// A strategy already holding the niche is refreshed (its fitness replaced).
    pub fn offer(&mut self, d: Descriptor, e: Elite) -> Offer {
        match self.cells.get_mut(&d) {
            None => {
                self.cells.insert(d, e);
                Offer::New
            }
            Some(cur) if cur.genome == e.genome => {
                *cur = e;
                Offer::Rejected
            }
            Some(cur) if e.fitness > cur.fitness => {
                *cur = e;
                Offer::Replaced
            }
            Some(_) => Offer::Rejected,
        }
    }

    /// Keep only the `max` fittest niches.
    pub fn trim(&mut self, max: usize) {
        if self.cells.len() <= max {
            return;
        }
        let mut order: Vec<(Descriptor, f64)> = self.cells.iter().map(|(d, e)| (*d, e.fitness)).collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (d, _) in order.into_iter().skip(max) {
            self.cells.remove(&d);
        }
    }

    /// A champion of a uniformly chosen niche.
    pub fn pick_parent(&self, rng: &mut Rng) -> Option<&Genome> {
        if self.cells.is_empty() {
            return None;
        }
        let k = rng.below(self.cells.len() as u32) as usize;
        self.cells.values().nth(k).map(|e| &e.genome)
    }

    /// Champions, fittest first.
    pub fn champions(&self) -> Vec<(&Descriptor, &Elite)> {
        let mut v: Vec<_> = self.cells.iter().collect();
        v.sort_by(|a, b| b.1.fitness.total_cmp(&a.1.fitness));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cards::id;

    fn profile(gains: &[(CardId, u64)], trashes: u64, games: u64) -> Profile {
        let mut g = vec![0; 512];
        for &(c, n) in gains {
            g[c as usize] = n;
        }
        Profile { games, gains: g, trashes }
    }

    fn elite(g: Genome, fitness: f64) -> Elite {
        Elite { genome: g, fitness, win_rate: fitness, games: 1, behaviour: Behaviour::default(), generation: 1, island: 0 }
    }

    #[test]
    fn descriptor_names_the_top_kingdom_cards_and_buckets_trashing_and_length() {
        let p = profile(&[(id::LABORATORY, 330), (id::MINE, 170), (id::GOLD, 900), (id::SILVER, 800), (id::MOAT, 20)], 400, 100);
        let b = Behaviour::from_profile(&p, 1700.0, 100);
        // Gold and Silver are basic cards, never key cards; Moat is too rare to count.
        assert_eq!(b.gains[0].0, id::LABORATORY);
        let d = b.describe(15.0, &NicheParams::default());
        let mut want = [id::LABORATORY, id::MINE];
        want.sort_unstable();
        assert_eq!(d.cards, want);
        assert_eq!(d.trash, 1, "4 trashes per game is light");
        assert_eq!(d.length, 2, "17 turns against 15 is long");
        assert_eq!(d.label().matches(" + ").count(), 1);
        assert!(d.label().ends_with("light trashing, long games"), "{}", d.label());
    }

    #[test]
    fn a_second_card_needs_to_be_gained_about_half_as_often_and_a_money_deck_has_no_key_card() {
        let p = profile(&[(id::SMITHY, 200), (id::MINE, 40)], 0, 100);
        let d = Behaviour::from_profile(&p, 1500.0, 100).describe(15.0, &NicheParams::default());
        assert_eq!(d.cards, [id::SMITHY, NO_CARD]);
        assert_eq!((d.trash, d.length), (0, 1));
        let none = Behaviour::from_profile(&profile(&[(id::GOLD, 700)], 0, 100), 1500.0, 100).describe(15.0, &NicheParams::default());
        assert_eq!(none.cards, [NO_CARD, NO_CARD]);
        assert!(none.label().starts_with("Money only"));
        let heavy = Behaviour { trashes: 9.0, turns: 10.0, ..Default::default() }.describe(15.0, &NicheParams::default());
        assert_eq!((heavy.trash, heavy.length), (2, 0));
    }

    #[test]
    fn the_archive_keeps_the_best_per_niche_and_parents_come_from_it() {
        let d1 = Descriptor { cards: [id::SMITHY, NO_CARD], trash: 0, length: 1 };
        let d2 = Descriptor { cards: [id::LABORATORY, NO_CARD], trash: 0, length: 1 };
        let a = crate::ops::big_money();
        let b = crate::ops::big_money_with(&[(id::SMITHY, 1)]);
        let c = crate::ops::big_money_with(&[(id::LABORATORY, 2)]);
        let mut arch = Archive::default();
        assert_eq!(arch.offer(d1, elite(a.clone(), 0.5)), Offer::New);
        assert_eq!(arch.offer(d1, elite(b.clone(), 0.4)), Offer::Rejected);
        assert_eq!(arch.get(&d1).unwrap().genome, a);
        assert_eq!(arch.offer(d1, elite(b.clone(), 0.6)), Offer::Replaced);
        assert_eq!(arch.get(&d1).unwrap().genome, b);
        assert_eq!(arch.offer(d2, elite(c.clone(), 0.3)), Offer::New);
        assert_eq!(arch.len(), 2);
        // Parents are the champions, and every niche gets picked sometime.
        let mut rng = Rng::new(3);
        let mut seen = [false; 2];
        for _ in 0..50 {
            let p = arch.pick_parent(&mut rng).unwrap();
            assert!(*p == b || *p == c);
            seen[(*p == c) as usize] = true;
        }
        assert!(seen[0] && seen[1]);
        assert_eq!(arch.champions()[0].1.genome, b);
        arch.trim(1);
        assert_eq!(arch.len(), 1);
        assert!(arch.contains(&d1), "the fittest niche survives trimming");
        assert!(Archive::default().pick_parent(&mut rng).is_none());
    }
}
