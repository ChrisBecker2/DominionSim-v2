//! Island-model genetic algorithm over strategy files, with racing (successive halving),
//! parsimony pressure, validation on held-out seeds and a hall of fame.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use dominion_engine::cards::{self, CardId};
use dominion_engine::rng::Rng;
use dominion_sim::Strategy;
use rayon::prelude::*;

use crate::arena::{set_fast, Arena, Opponent, Scenario, Score};
use crate::config::{EvolveConfig, Track};
use crate::genome::{Genome, TomlMeta};
use crate::ops;
use crate::polish::polish;
use crate::progress::{Entry, GenStat, Progress};
use crate::space::SearchSpace;

/// Shared with the caller to stop a run early (it finishes the current stage, then polishes).
#[derive(Default)]
pub struct Control {
    pub stop: AtomicBool,
}

impl Control {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

fn card_list(names: &[String], what: &str) -> Result<Vec<CardId>, String> {
    names.iter().map(|n| cards::by_name(n).ok_or_else(|| format!("unknown card {n:?} in {what}"))).collect()
}

/// Everything derived from a config: the arenas, the search space and the kingdom source.
pub struct Setup {
    pub arena: Arena,
    /// Same opponents, with the validation play mode.
    pub val_arena: Arena,
    pub space: SearchSpace,
    kingdoms: Kingdoms,
    pub opponent_names: Vec<String>,
}

enum Kingdoms {
    Fixed(Vec<CardId>),
    Random { required: Vec<CardId>, pool: Vec<CardId>, per_gen: usize },
}

impl Setup {
    pub fn new(cfg: &EvolveConfig) -> Result<Setup, String> {
        let forbidden = card_list(&cfg.forbidden, "forbidden cards")?;
        let (kingdoms, pool) = match &cfg.track {
            Track::Fixed { kingdom } => {
                let k = if kingdom.is_empty() { cards::kingdom_cards().collect() } else { card_list(kingdom, "kingdom")? };
                if let Some(&c) = k.iter().find(|&&c| c < cards::FIRST_KINGDOM) {
                    return Err(format!("{} is not a kingdom card", cards::name(c)));
                }
                (Kingdoms::Fixed(k.clone()), k)
            }
            Track::Random { required, kingdoms_per_generation } => {
                let required = card_list(required, "required cards")?;
                if required.len() > 10 {
                    return Err("at most 10 required cards".into());
                }
                let pool: Vec<CardId> = cards::kingdom_cards().filter(|c| !required.contains(c)).collect();
                (Kingdoms::Random { required, pool: pool.clone(), per_gen: (*kingdoms_per_generation).max(1) }, cards::kingdom_cards().collect())
            }
        };
        if cfg.opponents.is_empty() {
            return Err("choose at least one opponent".into());
        }
        let opponents = |fast: bool| -> Result<Vec<Opponent>, String> {
            cfg.opponents
                .iter()
                .map(|o| {
                    let mut strategy = Strategy::parse(&o.toml).map_err(|e| format!("opponent {}: {e}", o.name))?;
                    set_fast(&mut strategy, fast);
                    Ok(Opponent { name: o.name.clone(), strategy, weight: o.weight.max(0.0) })
                })
                .collect()
        };
        let arena = Arena { opponents: opponents(cfg.fast_eval)?, forbidden: forbidden.clone(), fast: cfg.fast_eval, max_turns: cfg.max_turns };
        let val_fast = !cfg.validate_full;
        let val_arena = Arena { opponents: opponents(val_fast)?, forbidden: forbidden.clone(), fast: val_fast, max_turns: cfg.max_turns };
        let mut space = SearchSpace::new(&pool, &forbidden);
        space.max_gain_rules = cfg.max_gain_rules.max(1);
        space.max_play_rules = cfg.max_play_rules;
        space.max_trash_rules = cfg.max_trash_rules;
        space.max_conds = cfg.max_conds;
        Ok(Setup { arena, val_arena, space, kingdoms, opponent_names: cfg.opponents.iter().map(|o| o.name.clone()).collect() })
    }

    /// Kingdoms for one round; random kingdoms are drawn from `rng`.
    pub fn scenarios(&self, rng: &mut Rng, scale: usize) -> Vec<Scenario> {
        match &self.kingdoms {
            Kingdoms::Fixed(k) => vec![Scenario { kingdom: k.clone() }],
            Kingdoms::Random { required, pool, per_gen } => (0..per_gen * scale)
                .map(|_| {
                    let mut k = required.clone();
                    let mut rest = pool.clone();
                    while k.len() < 10 && !rest.is_empty() {
                        k.push(rest.swap_remove(rng.below(rest.len() as u32) as usize));
                    }
                    k.sort_unstable();
                    Scenario { kingdom: k }
                })
                .collect(),
        }
    }
}

struct Candidate {
    genome: Genome,
    strategy: Option<Strategy>,
    score: Score,
    stages: usize,
}

fn fitness(score: &Score, g: &Genome, parsimony: f64) -> f64 {
    if score.games == 0.0 {
        return f64::NEG_INFINITY;
    }
    score.win_rate() - parsimony * g.size() as f64
}

/// The initial population: provided seeds, then templates (Big Money + X), then random pairs
/// and mutants, shuffled across islands.
fn initial_population(cfg: &EvolveConfig, setup: &Setup, rng: &mut Rng, progress: &mut Progress) -> Vec<Vec<Genome>> {
    let space = &setup.space;
    let mut pool: Vec<Genome> = Vec::new();
    for (i, src) in cfg.seeds.iter().enumerate() {
        match Genome::from_toml(src) {
            Ok(mut g) => {
                space.sanitize(&mut g);
                pool.push(g);
            }
            Err(e) => progress.note(format!("seed {} skipped: {e}", i + 1)),
        }
    }
    let mut templates = ops::templates(space);
    // Shuffle templates so small populations still see a variety of kingdom cards.
    for i in (1..templates.len()).rev() {
        templates.swap(i, rng.below(i as u32 + 1) as usize);
    }
    pool.extend(templates);
    let total = cfg.islands.max(1) * cfg.island_size.max(4);
    let mut islands: Vec<Vec<Genome>> = vec![Vec::new(); cfg.islands.max(1)];
    let mut seen = std::collections::HashSet::new();
    let mut i = 0;
    let mut attempts = 0;
    while islands.iter().map(Vec::len).sum::<usize>() < total && attempts < total * 50 {
        attempts += 1;
        let g = if i < pool.len() {
            pool[i].clone()
        } else if rng.chance(0.3) {
            ops::random_pair(space, rng)
        } else {
            let mut g = pool[rng.below(pool.len() as u32) as usize].clone();
            ops::mutate(&mut g, space, rng);
            g
        };
        i += 1;
        if seen.insert(g.key()) {
            let isl = islands.iter().enumerate().min_by_key(|(_, v)| v.len()).map(|(k, _)| k).unwrap();
            islands[isl].push(g);
        }
    }
    islands
}

fn tournament<'a>(members: &'a [(Genome, f64)], k: usize, rng: &mut Rng) -> &'a Genome {
    let mut best = &members[rng.below(members.len() as u32) as usize];
    for _ in 1..k.max(1) {
        let c = &members[rng.below(members.len() as u32) as usize];
        if c.1 > best.1 {
            best = c;
        }
    }
    &best.0
}

fn entry(g: &Genome, score: &Score, parsimony: f64, generation: usize, validated: bool, meta: TomlMeta) -> Entry {
    let (lo, hi) = score.ci95();
    Entry {
        toml: g.to_toml(&meta),
        summary: g.summary(),
        win_rate: score.win_rate(),
        ci_lo: lo,
        ci_hi: hi,
        games: score.played,
        size: g.size(),
        fitness: fitness(score, g, parsimony),
        generation,
        validated,
    }
}

fn meta_for(cfg: &EvolveConfig, setup: &Setup, generation: usize, score: &Score, how: &str) -> TomlMeta {
    let (lo, hi) = score.ci95();
    let track = match &cfg.track {
        Track::Fixed { kingdom } if kingdom.is_empty() => "every kingdom card available".to_string(),
        Track::Fixed { kingdom } => format!("kingdom {}", kingdom.join(", ")),
        Track::Random { required, .. } => format!("random kingdoms containing {}", required.join(", ")),
    };
    TomlMeta {
        name: format!("Evolved vs {} (gen {generation})", setup.opponent_names.join(" + ")),
        description: "Found by strategy search (dominion-lab).".into(),
        header: vec![
            format!("Evolved by dominion-lab, generation {generation}, {how}."),
            format!("Opponents: {}. Track: {track}.", setup.opponent_names.join(", ")),
            format!(
                "Win rate {:.1}% (95% CI {:.1}-{:.1}%) over {} games; never gains {}.",
                score.win_rate() * 100.0,
                lo * 100.0,
                hi * 100.0,
                score.played,
                if cfg.forbidden.is_empty() { "nothing extra".into() } else { cfg.forbidden.join(", ") }
            ),
        ],
        never_gain: setup.arena.forbidden.clone(),
        notes: Vec::new(),
    }
}

/// Run the search. `report` is called with a fresh snapshot after every generation and status
/// change; the final snapshot is also returned.
pub fn run(cfg: &EvolveConfig, control: &Control, report: &mut dyn FnMut(&Progress)) -> Result<Progress, String> {
    let setup = Setup::new(cfg)?;
    let start = Instant::now();
    let mut progress = Progress { status: "starting".into(), generations: cfg.generations, ..Default::default() };
    let mut rng = Rng::derive(cfg.seed, 0xE70);
    let mut islands = initial_population(cfg, &setup, &mut rng, &mut progress);
    progress.note(format!(
        "{} islands x {} candidates; {} gainable cards; opponents: {}",
        islands.len(),
        cfg.island_size,
        setup.space.gainable.len(),
        setup.opponent_names.join(", ")
    ));
    progress.status = "running".into();
    report(&progress);

    let parsimony = cfg.parsimony;
    let race: Vec<u64> = if cfg.race_games.is_empty() { vec![1000] } else { cfg.race_games.clone() };
    let mut best_ever = f64::NEG_INFINITY;
    let mut stall = 0;
    let mut hall: Vec<(String, Genome, Score, Entry)> = Vec::new();
    let mut last_gen = 0;

    for gen in 1..=cfg.generations {
        if control.stopped() {
            break;
        }
        last_gen = gen;
        let round_seed = Rng::derive(cfg.seed, gen as u64).next_u64();
        let mut krng = Rng::derive(round_seed, 0x6B);
        let scenarios = setup.scenarios(&mut krng, 1);

        // Unique candidates across islands.
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut cands: Vec<Candidate> = Vec::new();
        let mut membership: Vec<Vec<usize>> = Vec::new();
        for isl in &islands {
            let mut ids = Vec::with_capacity(isl.len());
            for g in isl {
                let id = *index.entry(g.key()).or_insert_with(|| {
                    cands.push(Candidate { genome: g.clone(), strategy: None, score: Score::default(), stages: 0 });
                    cands.len() - 1
                });
                ids.push(id);
            }
            membership.push(ids);
        }
        cands.par_iter_mut().for_each(|c| c.strategy = setup.arena.compile(&c.genome).ok());

        // Racing: everyone plays stage 0; the best fraction plays each later stage.
        let mut alive: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].strategy.is_some()).collect();
        for (stage, &games) in race.iter().enumerate() {
            if stage > 0 {
                alive.sort_by(|&a, &b| fitness(&cands[b].score, &cands[b].genome, parsimony).total_cmp(&fitness(&cands[a].score, &cands[a].genome, parsimony)));
                let keep = ((alive.len() as f64 * cfg.race_keep).ceil() as usize).clamp(1, alive.len());
                alive.truncate(keep);
            }
            if control.stopped() && stage > 0 {
                break;
            }
            let stage_seed = Rng::derive(round_seed, stage as u64 + 1).next_u64();
            let results: Vec<(usize, Score)> = alive
                .par_iter()
                .map(|&i| (i, setup.arena.evaluate(cands[i].strategy.as_ref().unwrap(), &scenarios, games, stage_seed)))
                .collect();
            for (i, s) in results {
                cands[i].score.add(&s);
                cands[i].stages = stage + 1;
                progress.games_played += s.played;
            }
        }

        // Generation statistics. The champion is the fittest among those that ran the full race.
        let max_stage = cands.iter().map(|c| c.stages).max().unwrap_or(0);
        let champ = (0..cands.len())
            .filter(|&i| cands[i].stages == max_stage && max_stage > 0)
            .max_by(|&a, &b| fitness(&cands[a].score, &cands[a].genome, parsimony).total_cmp(&fitness(&cands[b].score, &cands[b].genome, parsimony)));
        let mut rates: Vec<f64> = cands.iter().filter(|c| c.stages > 0).map(|c| c.score.win_rate()).collect();
        rates.sort_by(f64::total_cmp);
        let median = rates.get(rates.len() / 2).copied().unwrap_or(0.0);
        let Some(champ) = champ else {
            progress.note("no candidate could be evaluated");
            break;
        };
        let cf = fitness(&cands[champ].score, &cands[champ].genome, parsimony);
        if cf > best_ever + 0.002 {
            best_ever = cf;
            stall = 0;
        } else {
            stall += 1;
        }
        let c = &cands[champ];
        progress.champion = Some(entry(&c.genome, &c.score, parsimony, gen, false, meta_for(cfg, &setup, gen, &c.score, "selection seeds")));
        let mut stat = GenStat {
            generation: gen,
            best_fitness: cf,
            best_win_rate: c.score.win_rate(),
            median_win_rate: median,
            validated_win_rate: None,
            best_summary: c.genome.summary(),
        };

        // Validation on held-out seeds.
        let last = gen == cfg.generations || (cfg.stall_generations > 0 && stall >= cfg.stall_generations) || control.stopped();
        if (cfg.validate_every > 0 && gen % cfg.validate_every == 0) || last {
            progress.status = "validating".into();
            report(&progress);
            let (g, s) = validate(cfg, &setup, &c.genome, gen);
            progress.games_played += s.played;
            stat.validated_win_rate = Some(s.win_rate());
            progress.note(format!("gen {gen}: validated {:.1}% over {} games: {}", s.win_rate() * 100.0, s.played, g.summary()));
            add_to_hall(&mut hall, cfg, &setup, g, s, gen);
            progress.hall_of_fame = hall.iter().map(|h| h.3.clone()).collect();
            progress.status = "running".into();
        }
        progress.history.push(stat);
        progress.generation = gen;
        progress.elapsed_secs = start.elapsed().as_secs_f64();
        progress.games_per_sec = progress.games_played as f64 / progress.elapsed_secs.max(1e-9);
        report(&progress);

        if last {
            if cfg.stall_generations > 0 && stall >= cfg.stall_generations {
                progress.note(format!("stopping: no improvement for {stall} generations"));
            }
            break;
        }

        // Breed the next generation, island by island.
        let mut next = Vec::with_capacity(islands.len());
        for ids in &membership {
            let mut members: Vec<(Genome, f64)> = ids.iter().map(|&i| (cands[i].genome.clone(), fitness(&cands[i].score, &cands[i].genome, parsimony))).collect();
            members.sort_by(|a, b| b.1.total_cmp(&a.1));
            members.dedup_by(|a, b| a.0 == b.0);
            let size = cfg.island_size.max(4);
            let mut out: Vec<Genome> = members.iter().take(cfg.elite.min(size)).map(|m| m.0.clone()).collect();
            let mut keys: std::collections::HashSet<String> = out.iter().map(Genome::key).collect();
            let mut tries = 0;
            while out.len() < size && tries < size * 20 {
                tries += 1;
                let a = tournament(&members, cfg.tournament, &mut rng);
                let child = if rng.chance(cfg.crossover_rate) {
                    let b = tournament(&members, cfg.tournament, &mut rng);
                    let mut c = ops::crossover(a, b, &setup.space, &mut rng);
                    if rng.chance(0.5) {
                        ops::mutate(&mut c, &setup.space, &mut rng);
                    }
                    c
                } else {
                    let mut c = a.clone();
                    ops::mutate(&mut c, &setup.space, &mut rng);
                    c
                };
                if keys.insert(child.key()) {
                    out.push(child);
                }
            }
            next.push(out);
        }
        // Migration: each island's best replaces the next island's worst.
        if cfg.migration_interval > 0 && gen % cfg.migration_interval == 0 && next.len() > 1 {
            let bests: Vec<Genome> = next.iter().map(|isl| isl[0].clone()).collect();
            let n = next.len();
            for (k, b) in bests.into_iter().enumerate() {
                let dst = &mut next[(k + 1) % n];
                if !dst.contains(&b) {
                    *dst.last_mut().unwrap() = b;
                }
            }
        }
        islands = next;
    }

    // Polish the best validated strategy.
    if let Some((_, g, _, _)) = hall.first().cloned() {
        if cfg.polish_games > 0 {
            progress.status = "polishing".into();
            progress.note("polishing the best strategy (dropping rules that don't pay)");
            report(&progress);
            let seed = Rng::derive(cfg.seed ^ 0x9011_5400, 1).next_u64();
            let mut krng = Rng::derive(seed, 0x6B);
            let scen = setup.scenarios(&mut krng, 2);
            let p = polish(&g, &setup.arena, &setup.space, &scen, cfg.polish_games, seed, parsimony);
            for r in &p.removed {
                progress.note(format!("polish: {r}"));
            }
            progress.status = "validating".into();
            report(&progress);
            let (_, score) = validate(cfg, &setup, &p.genome, last_gen + 1);
            progress.games_played += p.score.played + score.played;
            let mut meta = meta_for(cfg, &setup, last_gen, &score, "polished, held-out seeds");
            meta.notes = p.notes.clone();
            progress.note(format!("result: {:.1}% over {} held-out games: {}", score.win_rate() * 100.0, score.played, p.genome.summary()));
            progress.result = Some(entry(&p.genome, &score, parsimony, last_gen, true, meta));
        } else {
            progress.result = Some(hall[0].3.clone());
        }
    }
    progress.status = if control.stopped() { "stopped".into() } else { "done".into() };
    progress.elapsed_secs = start.elapsed().as_secs_f64();
    progress.games_per_sec = progress.games_played as f64 / progress.elapsed_secs.max(1e-9);
    report(&progress);
    Ok(progress)
}

/// Measure a genome on held-out seeds (a separate seed domain from selection; twice as many
/// random kingdoms on the random track).
fn validate(cfg: &EvolveConfig, setup: &Setup, g: &Genome, gen: usize) -> (Genome, Score) {
    let seed = Rng::derive(cfg.seed ^ 0x7A11_D000, gen as u64).next_u64();
    let mut krng = Rng::derive(seed, 0x6B);
    let scen = setup.scenarios(&mut krng, 2);
    let s = match setup.val_arena.compile(g) {
        Ok(strat) => setup.val_arena.evaluate(&strat, &scen, cfg.validate_games, seed),
        Err(_) => Score::default(),
    };
    (g.clone(), s)
}

fn add_to_hall(hall: &mut Vec<(String, Genome, Score, Entry)>, cfg: &EvolveConfig, setup: &Setup, g: Genome, s: Score, gen: usize) {
    let key = g.key();
    let e = entry(&g, &s, cfg.parsimony, gen, true, meta_for(cfg, setup, gen, &s, "held-out seeds"));
    match hall.iter_mut().find(|h| h.0 == key) {
        // Seen before: pool the evidence.
        Some(h) => {
            h.2.add(&s);
            h.3 = entry(&h.1, &h.2, cfg.parsimony, gen, true, meta_for(cfg, setup, gen, &h.2, "held-out seeds"));
        }
        None => hall.push((key, g, s, e)),
    }
    hall.sort_by(|a, b| b.3.fitness.total_cmp(&a.3.fitness));
    hall.truncate(cfg.hall_of_fame.max(1));
}

/// Measure any strategy file against the configured opponents and kingdoms on held-out seeds
/// (`full`: exact bot behaviour instead of fast mode). Also works for files outside the
/// template language, since it runs the strategy as written.
pub fn assess(cfg: &EvolveConfig, toml_src: &str, games: u64, full: bool) -> Result<Entry, String> {
    let mut c = cfg.clone();
    c.fast_eval = !full;
    let setup = Setup::new(&c)?;
    let mut strategy = Strategy::parse(toml_src)?;
    set_fast(&mut strategy, setup.arena.fast);
    let seed = Rng::derive(cfg.seed ^ 0xA55E_5500, games).next_u64();
    let mut krng = Rng::derive(seed, 0x6B);
    let scen = setup.scenarios(&mut krng, 2);
    let score = setup.arena.evaluate(&strategy, &scen, games, seed);
    let (lo, hi) = score.ci95();
    let genome = Genome::from_toml(toml_src).ok();
    Ok(Entry {
        toml: toml_src.to_string(),
        summary: genome.as_ref().map(Genome::summary).unwrap_or_else(|| strategy.name.clone()),
        win_rate: score.win_rate(),
        ci_lo: lo,
        ci_hi: hi,
        games: score.played,
        size: genome.as_ref().map_or(0, Genome::size),
        fitness: score.win_rate() - cfg.parsimony * genome.as_ref().map_or(0, Genome::size) as f64,
        generation: 0,
        validated: true,
    })
}
