//! Fitness: a candidate plays head-to-head against each opponent in a pool, on each kingdom of
//! a scenario set. Seeds depend only on (round, scenario, opponent), never on the candidate, so
//! all candidates in a round face exactly the same deals (common random numbers).

use dominion_engine::cards::CardId;
use dominion_engine::rng::Rng;
use dominion_sim::batch::{run_match, run_match_profiled, MatchConfig, Profile};
use dominion_sim::stats::wilson_ci;
use dominion_sim::Strategy;
use serde::Serialize;

use crate::genome::{Genome, TomlMeta};

/// One kingdom to play on.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub kingdom: Vec<CardId>,
}

/// Most opponents whose results are tallied separately in a [`Score`]; later ones only count in
/// the totals.
pub const MAX_TALLY: usize = 32;

/// One opponent's share of a [`Score`]: unweighted wins (ties count half) and games played.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tally {
    pub wins: f64,
    pub games: u64,
}

/// A win rate against one opponent, for reports.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct OppRate {
    pub name: String,
    /// The opponent's share of the arena's weights (they sum to 1).
    pub weight: f64,
    pub win_rate: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
    pub games: u64,
}

/// "78.9% vs A, 53.8% vs B" (the 95% intervals are in the structured data, not the text).
pub fn vs_text(vs: &[OppRate]) -> String {
    vs.iter().map(|o| format!("{:.1}% vs {}", o.win_rate * 100.0, o.name)).collect::<Vec<_>>().join(", ")
}

/// Accumulated results. `wins`/`games` are opponent-weighted (ties count as a fractional win).
#[derive(Clone, Copy, Debug, Default)]
pub struct Score {
    pub wins: f64,
    pub games: f64,
    /// Games actually played.
    pub played: u64,
    /// The candidate's own turns, summed over games played (unweighted).
    pub turns: f64,
    /// Per opponent (arena order), unweighted: how the weighted total above came about.
    pub per_opp: [Tally; MAX_TALLY],
}

impl Score {
    pub fn win_rate(&self) -> f64 {
        if self.games > 0.0 { self.wins / self.games } else { 0.0 }
    }
    /// 95% Wilson interval (treating the weighted counts as plain counts).
    pub fn ci95(&self) -> (f64, f64) {
        wilson_ci(self.wins, self.games, dominion_sim::stats::Z_95)
    }
    pub fn add(&mut self, o: &Score) {
        self.wins += o.wins;
        self.games += o.games;
        self.played += o.played;
        self.turns += o.turns;
        for (a, b) in self.per_opp.iter_mut().zip(o.per_opp.iter()) {
            a.wins += b.wins;
            a.games += b.games;
        }
    }
    /// Average number of the candidate's own turns per game.
    pub fn avg_turns(&self) -> f64 {
        if self.played > 0 { self.turns / self.played as f64 } else { 0.0 }
    }
}

/// Apply an arena's play mode to a strategy (fast mode turns off both turn searches).
pub fn set_fast(s: &mut Strategy, fast: bool) {
    set_play_mode(s, fast, None);
}

/// [`set_fast`], optionally leaving the turn search on for decisions that involve the cards of `search_cards` (selective search:
/// see `docs/search-in-league.md`). Without a mask a fast strategy searches nothing.
pub fn set_play_mode(s: &mut Strategy, fast: bool, search: Option<&Selective>) {
    if fast {
        s.win_this_turn = false;
        match search {
            Some(sel) if s.search_play => {
                s.set_search_cards(sel.cards.clone());
                s.search_budget = sel.budget;
            }
            _ => s.search_play = false,
        }
    }
}

/// Selective search: fast bots still search the decisions that involve `cards` (indexed by card id), with a second-pass node budget of
/// `budget` (`Strategy::search_budget`).
#[derive(Clone, Debug)]
pub struct Selective {
    pub cards: Vec<bool>,
    pub budget: u64,
}

pub struct Opponent {
    pub name: String,
    pub strategy: Strategy,
    pub weight: f64,
}

pub struct Arena {
    pub opponents: Vec<Opponent>,
    /// Written into every candidate as `never_gain`.
    pub forbidden: Vec<CardId>,
    /// Fast mode: actions are played in rule order (no turn search) and there is no
    /// `win_this_turn` lookahead. Several times faster, slightly weaker endgames. Full mode plays
    /// exactly like the bots in the game UI. Applies to candidates and opponents alike.
    pub fast: bool,
    /// With `fast`: the cards whose decisions are still searched (selective search, `None`: none are).
    pub search_cards: Option<Selective>,
    pub max_turns: u16,
}

impl Arena {
    /// Compile a genome into a runnable strategy with this arena's settings.
    pub fn compile(&self, g: &Genome) -> Result<Strategy, String> {
        let meta = TomlMeta { name: "Candidate".into(), never_gain: self.forbidden.clone(), ..Default::default() };
        let mut s = Strategy::parse(&g.to_toml(&meta))?;
        set_play_mode(&mut s, self.fast, self.search_cards.as_ref());
        Ok(s)
    }

    /// A score's results per opponent, with 95% intervals (opponents with no games are left out).
    pub fn breakdown(&self, score: &Score) -> Vec<OppRate> {
        let total: f64 = self.opponents.iter().map(|o| o.weight).sum();
        self.opponents
            .iter()
            .zip(score.per_opp.iter())
            .filter(|(_, t)| t.games > 0)
            .map(|(o, t)| {
                let (ci_lo, ci_hi) = wilson_ci(t.wins, t.games as f64, dominion_sim::stats::Z_95);
                OppRate { name: o.name.clone(), weight: if total > 0.0 { o.weight / total } else { 0.0 }, win_rate: t.wins / t.games as f64, ci_lo, ci_hi, games: t.games }
            })
            .collect()
    }

    /// Play `games` games (split evenly over scenarios and opponents, rounded up to an even number
    /// per pairing so seats balance) with seeds derived from `round_seed`.
    pub fn evaluate(&self, candidate: &Strategy, scenarios: &[Scenario], games: u64, round_seed: u64) -> Score {
        self.evaluate_with(candidate, scenarios, games, round_seed, false).0
    }

    /// [`Arena::evaluate`], optionally also profiling the candidate's gains and trashes (pooled
    /// over all pairings) for niche descriptors.
    pub fn evaluate_with(&self, candidate: &Strategy, scenarios: &[Scenario], games: u64, round_seed: u64, profiled: bool) -> (Score, Option<Profile>) {
        let pairings = (scenarios.len() * self.opponents.len()).max(1) as u64;
        let per = (games.div_ceil(pairings) + 1) & !1;
        let mut total = Score::default();
        let mut profile: Option<Profile> = profiled.then(Profile::default);
        for (k, sc) in scenarios.iter().enumerate() {
            for (o, opp) in self.opponents.iter().enumerate() {
                let cfg = MatchConfig {
                    games: per,
                    kingdom: sc.kingdom.clone(),
                    seed: Rng::derive(round_seed, (k as u64) << 16 | o as u64).next_u64(),
                    max_turns: self.max_turns,
                };
                let stats = match profile.as_mut() {
                    Some(p) => {
                        let (stats, mut profs) = run_match_profiled(&[candidate, &opp.strategy], &cfg);
                        p.merge(&profs.swap_remove(0));
                        stats
                    }
                    None => run_match(&[candidate, &opp.strategy], &cfg),
                };
                total.wins += stats[0].wins * opp.weight;
                total.games += stats[0].games as f64 * opp.weight;
                total.played += stats[0].games;
                total.turns += stats[0].turns_sum;
                if let Some(t) = total.per_opp.get_mut(o) {
                    t.wins += stats[0].wins;
                    t.games += stats[0].games;
                }
            }
        }
        (total, profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dominion_engine::cards;

    fn bm(name: &str, extra: &str) -> Strategy {
        Strategy::parse(&format!("name = \"{name}\"

[[gain]]
card = \"Province\"

{extra}[[gain]]
card = \"Gold\"

[[gain]]
card = \"Silver\"
")).unwrap()
    }

    #[test]
    fn per_opponent_tallies_sum_to_the_overall_score() {
        let arena = Arena {
            opponents: vec![
                Opponent { name: "A".into(), strategy: bm("A", ""), weight: 3.0 },
                Opponent { name: "B".into(), strategy: bm("B", ""), weight: 1.0 },
            ],
            forbidden: vec![],
            fast: true,
            search_cards: None,
            max_turns: 60,
        };
        let k: Vec<CardId> = vec![cards::by_name("Smithy").unwrap()];
        let sc = [Scenario { kingdom: k.clone() }, Scenario { kingdom: k }];
        let cand = bm("C", "[[gain]]
card = \"Smithy\"
if = \"count(Smithy) < 1\"

");
        let s = arena.evaluate(&cand, &sc, 400, 7);
        let games: u64 = s.per_opp.iter().map(|t| t.games).sum();
        assert_eq!(games, s.played);
        let vs = arena.breakdown(&s);
        assert_eq!(vs.len(), 2);
        assert_eq!(vs[0].name, "A");
        assert!((vs[0].weight - 0.75).abs() < 1e-9);
        // The weighted mean of the per-opponent rates is the overall rate (equal games per opponent).
        let weighted: f64 = vs.iter().map(|o| o.win_rate * o.weight).sum();
        assert!((weighted - s.win_rate()).abs() < 1e-9, "{weighted} vs {}", s.win_rate());
        let wins: f64 = s.per_opp.iter().map(|t| t.wins).sum();
        assert!(wins > 0.0 && wins < games as f64);
        for o in &vs {
            assert!(o.ci_lo <= o.win_rate && o.win_rate <= o.ci_hi);
        }
        // Adding two scores adds the tallies.
        let mut t = s;
        t.add(&s);
        assert_eq!(t.per_opp[0].games, 2 * s.per_opp[0].games);
        assert!(vs_text(&vs).contains("vs A, ") && vs_text(&vs).contains("% vs B"));
    }
}
