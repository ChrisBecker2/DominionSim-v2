//! Readability pass: greedily drop rules and conditions that don't earn their place, then
//! measure what each remaining rule is worth (win rate lost without it).

use rayon::prelude::*;

use crate::arena::{Arena, Scenario, Score};
use crate::genome::{Genome, List};
use crate::space::SearchSpace;

/// A single-step simplification of `g`: without one rule, or one condition.
fn simplifications(g: &Genome, space: &SearchSpace) -> Vec<(String, Genome)> {
    let mut out = Vec::new();
    for list in [List::Gain, List::Play, List::Trash] {
        let n = g.clone().list_mut(list).len();
        for i in 0..n {
            let mut v = g.clone();
            let r = v.list_mut(list).remove(i);
            space.sanitize(&mut v);
            out.push((format!("dropped {:?} rule for {}", list, dominion_engine::cards::name(r.card)), v));
            for j in 0..r.conds.len() {
                let mut v = g.clone();
                let a = v.list_mut(list)[i].conds.remove(j);
                space.sanitize(&mut v);
                out.push((format!("dropped condition `{}` on {}", a.text(), dominion_engine::cards::name(r.card)), v));
            }
        }
    }
    out.retain(|(_, v)| v != g);
    out
}

fn score_all(arena: &Arena, gs: &[&Genome], scenarios: &[Scenario], games: u64, seed: u64) -> Vec<Score> {
    gs.par_iter()
        .map(|g| match arena.compile(g) {
            Ok(s) => arena.evaluate(&s, scenarios, games, seed),
            Err(_) => Score::default(),
        })
        .collect()
}

pub struct Polished {
    pub genome: Genome,
    pub score: Score,
    /// `(list, index, note)` for each remaining rule: its measured impact.
    pub notes: Vec<(List, usize, String)>,
    /// What was removed, in order.
    pub removed: Vec<String>,
}

/// Simplify while fitness (win rate − parsimony·size) doesn't drop, all on the same seeds.
pub fn polish(g: &Genome, arena: &Arena, space: &SearchSpace, scenarios: &[Scenario], games: u64, seed: u64, parsimony: f64) -> Polished {
    let fit = |s: &Score, g: &Genome| s.win_rate() - parsimony * g.size() as f64;
    let mut cur = g.clone();
    let mut cur_score = score_all(arena, &[&cur], scenarios, games, seed)[0];
    let mut removed = Vec::new();
    loop {
        let cands = simplifications(&cur, space);
        if cands.is_empty() {
            break;
        }
        let refs: Vec<&Genome> = cands.iter().map(|(_, g)| g).collect();
        let scores = score_all(arena, &refs, scenarios, games, seed);
        let best = (0..cands.len()).max_by(|&a, &b| fit(&scores[a], &cands[a].1).total_cmp(&fit(&scores[b], &cands[b].1)));
        match best {
            Some(i) if fit(&scores[i], &cands[i].1) >= fit(&cur_score, &cur) => {
                removed.push(cands[i].0.clone());
                cur = cands[i].1.clone();
                cur_score = scores[i];
            }
            _ => break,
        }
    }

    // Impact of each remaining rule.
    let mut drops = Vec::new();
    for list in [List::Gain, List::Play, List::Trash] {
        for i in 0..cur.clone().list_mut(list).len() {
            let mut v = cur.clone();
            v.list_mut(list).remove(i);
            space.sanitize(&mut v);
            drops.push((list, i, v));
        }
    }
    let refs: Vec<&Genome> = drops.iter().map(|(_, _, g)| g).collect();
    let scores = score_all(arena, &refs, scenarios, games, seed);
    let notes = drops
        .iter()
        .zip(&scores)
        .map(|((list, i, _), s)| {
            let d = (cur_score.win_rate() - s.win_rate()) * 100.0;
            (*list, *i, format!("worth {d:+.1}% win rate (vs. dropping this rule)"))
        })
        .collect();
    Polished { genome: cur, score: cur_score, notes, removed }
}
