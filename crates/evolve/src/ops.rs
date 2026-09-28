//! Variation operators: seed templates, random rules, mutation and crossover.

use dominion_engine::cards::{self, id, CardId, ACTION};
use dominion_engine::rng::Rng;

use crate::genome::{Atom, Genome, List, Op, Rule, Var};
use crate::space::SearchSpace;

fn pick<T: Copy>(rng: &mut Rng, xs: &[T]) -> T {
    xs[rng.below(xs.len() as u32) as usize]
}

fn cond(var: Var, op: Op, n: i16) -> Atom {
    Atom { var, op, n }
}

/// Big Money with the usual Duchy/Estate endgame: the baseline every template builds on.
pub fn big_money() -> Genome {
    Genome {
        gain: vec![
            Rule::new(id::PROVINCE),
            Rule { card: id::DUCHY, conds: vec![cond(Var::ProvincesLeft, Op::Lt, 5)] },
            Rule::new(id::GOLD),
            Rule { card: id::ESTATE, conds: vec![cond(Var::ProvincesLeft, Op::Lt, 3)] },
            Rule::new(id::SILVER),
        ],
        keep_treasure: 2,
        ..Genome::default()
    }
}

/// "Big Money + X": up to `n` copies of each kingdom card, bought ahead of Gold (and played in
/// the listed order when they are actions).
pub fn big_money_with(xs: &[(CardId, i16)]) -> Genome {
    let mut g = big_money();
    let gold = g.gain.iter().position(|r| r.card == id::GOLD).unwrap_or(1);
    for (k, &(c, n)) in xs.iter().enumerate() {
        g.gain.insert(gold + k, Rule { card: c, conds: vec![cond(Var::Count(c), Op::Lt, n)] });
        if cards::is(c, ACTION) {
            g.play.push(Rule::new(c));
        }
    }
    g
}

/// Seed templates: Big Money, and Big Money + X for every allowed kingdom card X (1 and 2 copies).
pub fn templates(space: &SearchSpace) -> Vec<Genome> {
    let mut out = vec![big_money()];
    for c in space.gainable.iter().copied().filter(|&c| c >= cards::FIRST_KINGDOM) {
        out.push(big_money_with(&[(c, 1)]));
        out.push(big_money_with(&[(c, 2)]));
    }
    out
}

/// "Big Money + X + Y" for a random pair.
pub fn random_pair(space: &SearchSpace, rng: &mut Rng) -> Genome {
    let kingdom: Vec<CardId> = space.gainable.iter().copied().filter(|&c| c >= cards::FIRST_KINGDOM).collect();
    if kingdom.len() < 2 {
        return big_money();
    }
    let x = pick(rng, &kingdom);
    let mut y = pick(rng, &kingdom);
    while y == x {
        y = pick(rng, &kingdom);
    }
    big_money_with(&[(x, 1 + rng.below(2) as i16), (y, 1 + rng.below(2) as i16)])
}

/// A random condition, biased towards the ones people actually write for `card`.
pub fn random_atom(space: &SearchSpace, rng: &mut Rng, card: CardId) -> Atom {
    let r = rng.below(100);
    let (var, op) = match r {
        0..=39 if card >= cards::FIRST_KINGDOM || card == id::GOLD || card == id::SILVER => (Var::Count(card), Op::Lt),
        0..=39 | 40..=59 => (Var::ProvincesLeft, if rng.chance(0.7) { Op::Lt } else { Op::Ge }),
        60..=67 => (Var::Money, if rng.chance(0.5) { Op::Lt } else { Op::Ge }),
        68..=74 => (Var::MyTurn, if rng.chance(0.5) { Op::Lt } else { Op::Ge }),
        75..=81 => (Var::VpLead, if rng.chance(0.5) { Op::Lt } else { Op::Ge }),
        82..=87 => (Var::CountActions, Op::Lt),
        88..=92 => (Var::TotalCards, if rng.chance(0.5) { Op::Lt } else { Op::Ge }),
        _ => (Var::Count(pick(rng, &space.gainable)), if rng.chance(0.5) { Op::Lt } else { Op::Ge }),
    };
    let (lo, hi) = var.range();
    // Small thresholds are far more common for counts and provinces.
    let n = match var {
        Var::Count(_) => 1 + rng.below(3) as i16,
        Var::ProvincesLeft => 2 + rng.below(5) as i16,
        _ => lo + rng.below((hi - lo + 1) as u32) as i16,
    };
    Atom { var, op, n }
}

fn random_rule(space: &SearchSpace, rng: &mut Rng, list: List) -> Option<Rule> {
    let card = match list {
        List::Gain => pick(rng, &space.gainable),
        List::Play => {
            let actions: Vec<CardId> = space.actions().collect();
            if actions.is_empty() {
                return None;
            }
            pick(rng, &actions)
        }
        List::Trash => pick(rng, &space.trashable),
    };
    let mut r = Rule::new(card);
    let p_cond = if list == List::Gain && card >= cards::FIRST_KINGDOM { 0.8 } else { 0.3 };
    if rng.chance(p_cond) {
        r.conds.push(random_atom(space, rng, card));
    }
    Some(r)
}

/// Apply one random mutation. Returns a short description (for logs).
pub fn mutate_once(g: &mut Genome, space: &SearchSpace, rng: &mut Rng) -> &'static str {
    // The gain list carries most of a strategy, so it gets most of the attention.
    let list = match rng.below(10) {
        0..=6 => List::Gain,
        7..=8 => List::Play,
        _ => List::Trash,
    };
    let max = space.max_rules(list);
    let len = g.list_mut(list).len();
    let op = rng.below(if list == List::Gain { 10 } else { 5 });
    match op {
        // insert
        0 if len < max => {
            if let Some(r) = random_rule(space, rng, list) {
                let at = rng.below(len as u32 + 1) as usize;
                g.list_mut(list).insert(at, r);
                return "insert rule";
            }
        }
        // delete
        1 if len > 0 => {
            let at = rng.below(len as u32) as usize;
            g.list_mut(list).remove(at);
            return "delete rule";
        }
        // move up/down
        2 if len > 1 => {
            let at = rng.below(len as u32 - 1) as usize;
            g.list_mut(list).swap(at, at + 1);
            return "swap rules";
        }
        // change card
        3 if len > 0 => {
            if let Some(r) = random_rule(space, rng, list) {
                let at = rng.below(len as u32) as usize;
                let rule = &mut g.list_mut(list)[at];
                let old = rule.card;
                rule.card = r.card;
                // Conditions on the rule's own count follow the card.
                for a in rule.conds.iter_mut() {
                    if a.var == Var::Count(old) {
                        a.var = Var::Count(r.card);
                    }
                }
                return "change card";
            }
        }
        4 if list != List::Gain => {
            if list == List::Trash || rng.chance(0.5) {
                g.keep_treasure = (g.keep_treasure as i16 + if rng.chance(0.5) { 1 } else { -1 }).clamp(0, 4) as u8;
                return "keep_treasure";
            }
        }
        _ => {}
    }
    // Condition-level edits on a random rule of the chosen list (or the gain list).
    let list = if g.list_mut(list).is_empty() { List::Gain } else { list };
    let rules = g.list_mut(list);
    if rules.is_empty() {
        return "none";
    }
    let at = rng.below(rules.len() as u32) as usize;
    let card = rules[at].card;
    let nconds = rules[at].conds.len();
    match rng.below(4) {
        0 if nconds < space.max_conds => {
            let a = random_atom(space, rng, card);
            g.list_mut(list)[at].conds.push(a);
            "add condition"
        }
        1 if nconds > 0 => {
            let i = rng.below(nconds as u32) as usize;
            g.list_mut(list)[at].conds.remove(i);
            "remove condition"
        }
        2 if nconds > 0 => {
            let i = rng.below(nconds as u32) as usize;
            let a = &mut g.list_mut(list)[at].conds[i];
            a.op = if a.op == Op::Lt { Op::Ge } else { Op::Lt };
            "flip comparison"
        }
        _ if nconds > 0 => {
            let i = rng.below(nconds as u32) as usize;
            let a = &mut g.list_mut(list)[at].conds[i];
            let step = a.var.step() * if rng.below(4) == 0 { 2 } else { 1 };
            a.n += if rng.chance(0.5) { step } else { -step };
            "nudge threshold"
        }
        _ => {
            let a = random_atom(space, rng, card);
            g.list_mut(list)[at].conds.push(a);
            "add condition"
        }
    }
}

/// One to three mutations, then sanitize.
pub fn mutate(g: &mut Genome, space: &SearchSpace, rng: &mut Rng) {
    let n = 1 + (rng.below(100) < 35) as u32 + (rng.below(100) < 10) as u32;
    for _ in 0..n {
        mutate_once(g, space, rng);
    }
    space.sanitize(g);
}

/// Splice the parents' gain lists at random cut points; take play and trash lists and
/// `keep_treasure` each from one parent.
pub fn crossover(a: &Genome, b: &Genome, space: &SearchSpace, rng: &mut Rng) -> Genome {
    let i = rng.below(a.gain.len() as u32 + 1) as usize;
    let j = rng.below(b.gain.len() as u32 + 1) as usize;
    let mut gain: Vec<Rule> = a.gain[..i].to_vec();
    gain.extend_from_slice(&b.gain[j..]);
    let mut child = Genome {
        gain,
        play: if rng.chance(0.5) { a.play.clone() } else { b.play.clone() },
        trash: if rng.chance(0.5) { a.trash.clone() } else { b.trash.clone() },
        keep_treasure: if rng.chance(0.5) { a.keep_treasure } else { b.keep_treasure },
    };
    // Keep play rules for any action the child now gains but neither list mentions.
    for r in a.play.iter().chain(&b.play) {
        if child.gain.iter().any(|g| g.card == r.card) && !child.play.iter().any(|p| p.card == r.card) {
            child.play.push(r.clone());
        }
    }
    space.sanitize(&mut child);
    child
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutants_are_always_valid_strategies_and_respect_the_space() {
        let pool: Vec<CardId> = cards::kingdom_cards().collect();
        let space = SearchSpace::new(&pool, &[id::WITCH]);
        let mut rng = Rng::new(7);
        let mut pop = templates(&space);
        for step in 0..4000 {
            let a = pop[rng.below(pop.len() as u32) as usize].clone();
            let child = if step % 5 == 0 {
                let b = &pop[rng.below(pop.len() as u32) as usize];
                crossover(&a, b, &space, &mut rng)
            } else {
                let mut c = a;
                mutate(&mut c, &space, &mut rng);
                c
            };
            let text = child.to_toml(&Default::default());
            dominion_sim::Strategy::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
            assert!(child.gain.iter().all(|r| r.card != id::WITCH), "{text}");
            assert!(child.gain.len() <= space.max_gain_rules && child.play.len() <= space.max_play_rules);
            assert_eq!(crate::genome::Genome::from_toml(&text).unwrap(), child, "{text}");
            pop.push(child);
        }
    }
}
