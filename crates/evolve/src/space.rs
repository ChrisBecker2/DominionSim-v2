//! The search space: which cards a candidate may gain, play and trash, and how big it may get.

use dominion_engine::cards::{self, id, CardId, ACTION};

use crate::genome::{Genome, List, Rule};

#[derive(Clone, Debug)]
pub struct SearchSpace {
    /// Cards a candidate may put in `[[gain]]` rules (basic cards plus allowed kingdom cards).
    pub gainable: Vec<CardId>,
    /// Cards a candidate may put in `[[trash]]` rules.
    pub trashable: Vec<CardId>,
    /// Cards the candidate must never gain (written as `never_gain` so forced gains respect it too).
    pub forbidden: Vec<CardId>,
    pub max_gain_rules: usize,
    pub max_play_rules: usize,
    pub max_trash_rules: usize,
    pub max_conds: usize,
}

impl SearchSpace {
    /// `kingdom_pool`: kingdom cards that can appear in games; `forbidden`: cards we may not use.
    pub fn new(kingdom_pool: &[CardId], forbidden: &[CardId]) -> SearchSpace {
        let basics = [id::PROVINCE, id::DUCHY, id::ESTATE, id::GOLD, id::SILVER];
        let gainable = basics.iter().chain(kingdom_pool).copied().filter(|c| !forbidden.contains(c)).collect();
        SearchSpace {
            gainable,
            trashable: vec![id::CURSE, id::ESTATE, id::COPPER, id::SILVER],
            forbidden: forbidden.to_vec(),
            max_gain_rules: 10,
            max_play_rules: 5,
            max_trash_rules: 4,
            max_conds: 2,
        }
    }

    /// Action cards a candidate can gain (the only cards worth a `[[play]]` rule).
    pub fn actions(&self) -> impl Iterator<Item = CardId> + '_ {
        self.gainable.iter().copied().filter(|&c| cards::is(c, ACTION))
    }

    pub fn max_rules(&self, list: List) -> usize {
        match list {
            List::Gain => self.max_gain_rules,
            List::Play => self.max_play_rules,
            List::Trash => self.max_trash_rules,
        }
    }

    /// Make a genome legal and tidy: drop rules for cards outside the space, play rules for
    /// cards the strategy never gains, rules shadowed by an identical earlier rule, and
    /// conditions beyond the cap; clamp thresholds and list lengths. Never leaves the gain list empty.
    pub fn sanitize(&self, g: &mut Genome) {
        g.gain.retain(|r| self.gainable.contains(&r.card));
        let gained: Vec<CardId> = g.gain.iter().map(|r| r.card).collect();
        g.play.retain(|r| cards::is(r.card, ACTION) && gained.contains(&r.card));
        g.trash.retain(|r| self.trashable.contains(&r.card));
        for list in [List::Gain, List::Play, List::Trash] {
            let max = self.max_rules(list);
            let rules = g.list_mut(list);
            for r in rules.iter_mut() {
                tidy_conds(r, self.max_conds);
            }
            // An unconditional rule makes later rules for the same card unreachable.
            let mut out: Vec<Rule> = Vec::with_capacity(rules.len());
            for r in rules.drain(..) {
                let shadowed = out.iter().any(|p| p.card == r.card && (p.conds.is_empty() || p.conds == r.conds));
                if !shadowed {
                    out.push(r);
                }
            }
            out.truncate(max);
            *rules = out;
        }
        if g.gain.is_empty() {
            g.gain.push(Rule::new(id::PROVINCE));
        }
        g.keep_treasure = g.keep_treasure.min(4);
    }
}

fn tidy_conds(r: &mut Rule, max: usize) {
    for a in r.conds.iter_mut() {
        let (lo, hi) = a.var.range();
        a.n = a.n.clamp(lo, hi);
    }
    // One atom per (variable, op): keep the first.
    let mut seen = Vec::new();
    r.conds.retain(|a| {
        let k = (a.var, a.op);
        let dup = seen.contains(&k);
        seen.push(k);
        !dup
    });
    r.conds.truncate(max);
}
