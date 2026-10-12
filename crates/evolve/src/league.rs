//! Kingdom league: find several distinct good strategies for one fixed kingdom without premade
//! opponents.
//!
//! - **Anchors**: automatic baselines ([`crate::baselines`]) for the kingdom.
//! - **League scoring**: a candidate's fitness is its win rate against a weighted mix of the
//!   anchors, the newest hall-of-fame champions and a sample of older ones. Every candidate of a
//!   generation plays the same deals (common random numbers).
//! - **Niches** ([`crate::niche`]): the best strategy per behavioural niche is kept in an
//!   archive; parents are drawn from it as well as from each island's tournament.
//! - **Escaping local optima**: islands seeded from different baselines with migration of the
//!   best and of a niche champion; an island that stops contributing niche champions is shaken
//!   up or restarted from another baseline; new niches earn a small novelty bonus.
//! - **Final ranking** ([`crate::rank`]): a round robin of the niche champions and the anchors
//!   with full bots, a crosstable, each strategy's mean against the field and a Nash mix.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use dominion_engine::cards::{self, CardId};
use dominion_engine::rng::Rng;
use dominion_sim::batch::{run_match, MatchConfig, Profile};
use dominion_sim::stats::{wilson_ci, Z_95};
use dominion_sim::Strategy;
use rayon::prelude::*;

use crate::arena::{Arena, Opponent, Scenario, Score};
use crate::baselines::{baselines, Baseline};
use crate::config::{EvolveConfig, LeagueConfig};
use crate::ga::{Control, Setup};
use crate::genome::{Genome, TomlMeta};
use crate::niche::{Archive, Behaviour, Descriptor, Elite, NicheParams, Offer};
use crate::ops;
use crate::polish::polish;
use crate::progress::{AnchorInfo, Entry, GenStat, LeagueReport, NicheRow, Progress, TopRow};
use crate::rank;

/// Per-opponent weights for a league of `anchors` anchors, `recent` recent and `old` older
/// hall-of-fame members, in that order. Each group's share is `*_weight`, spread evenly inside
/// the group; a group with no members is dropped and the rest renormalized, so the weights sum
/// to one (or are empty for an empty league).
pub fn league_weights(cfg: &LeagueConfig, anchors: usize, recent: usize, old: usize) -> Vec<f64> {
    let groups = [(anchors, cfg.anchor_weight.max(0.0)), (recent, cfg.recent_weight.max(0.0)), (old, cfg.old_weight.max(0.0))];
    let mut total: f64 = groups.iter().filter(|g| g.0 > 0).map(|g| g.1).sum();
    if total <= 0.0 {
        // All present groups have zero weight: fall back to equal shares.
        total = groups.iter().filter(|g| g.0 > 0).count() as f64;
        return groups.iter().flat_map(|&(n, _)| std::iter::repeat(1.0 / total / n.max(1) as f64).take(n)).collect();
    }
    groups.iter().flat_map(|&(n, w)| std::iter::repeat(w / total / n.max(1) as f64).take(n)).collect()
}

/// Tracks, per island, when it last contributed a niche champion, and when it must be shaken up.
#[derive(Clone, Debug)]
pub struct Stagnation {
    last: Vec<usize>,
    restarts: Vec<usize>,
    limit: usize,
}

impl Stagnation {
    pub fn new(islands: usize, limit: usize) -> Stagnation {
        Stagnation { last: vec![0; islands], restarts: vec![0; islands], limit }
    }
    /// Island `i` contributed a new or improved niche champion in generation `gen`.
    pub fn improved(&mut self, i: usize, gen: usize) {
        self.last[i] = gen;
    }
    /// Whether island `i` has gone `limit` generations without contributing.
    pub fn due(&self, i: usize, gen: usize) -> bool {
        self.limit > 0 && gen.saturating_sub(self.last[i]) >= self.limit
    }
    /// Record a restart of island `i`; returns how many it has had now (1 for the first).
    pub fn restarted(&mut self, i: usize, gen: usize) -> usize {
        self.last[i] = gen;
        self.restarts[i] += 1;
        self.restarts[i]
    }
}

struct Anchor {
    base: Baseline,
}

struct Member {
    genome: Genome,
    key: String,
    name: String,
    gen: usize,
    val: Score,
}

struct Cand {
    genome: Genome,
    key: String,
    strategy: Option<Strategy>,
    score: Score,
    profile: Option<Profile>,
    stages: usize,
    protected: bool,
}

fn fitness(score: &Score, g: &Genome, parsimony: f64) -> f64 {
    if score.games == 0.0 {
        return f64::NEG_INFINITY;
    }
    score.win_rate() - parsimony * g.size() as f64
}

fn sample_entry(g: &Genome, score: &Score, parsimony: f64, generation: usize, validated: bool, meta: &TomlMeta) -> Entry {
    let (lo, hi) = score.ci95();
    Entry {
        toml: g.to_toml(meta),
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

fn kingdom_text(cfg: &EvolveConfig) -> String {
    match &cfg.track {
        crate::config::Track::Fixed { kingdom } => kingdom.join(", "),
        _ => String::new(),
    }
}

fn meta_for(cfg: &EvolveConfig, setup: &Setup, name: String, label: &str, how: &str, extra: String) -> TomlMeta {
    TomlMeta {
        name,
        description: format!("Kingdom league strategy ({label})."),
        header: vec![
            format!("Found by the kingdom league search (dominion-lab): {how}."),
            format!("Kingdom: {}.", kingdom_text(cfg)),
            format!("Niche: {label}."),
            extra,
        ],
        never_gain: setup.arena.forbidden.clone(),
        notes: Vec::new(),
    }
}

/// The population of an island around `base`: the base itself, then mutants (`shake` mutations
/// each, so a larger value wanders further).
fn population_around(base: &Genome, extra: &[Genome], size: usize, shake: u32, setup: &Setup, rng: &mut Rng) -> Vec<Genome> {
    let mut seen = HashSet::new();
    let mut out: Vec<Genome> = Vec::new();
    for g in std::iter::once(base).chain(extra.iter()) {
        let mut g = g.clone();
        setup.space.sanitize(&mut g);
        if seen.insert(g.key()) {
            out.push(g);
        }
    }
    let mut tries = 0;
    while out.len() < size && tries < size * 40 {
        tries += 1;
        let mut g = if rng.chance(0.15) { ops::random_pair(&setup.space, rng) } else { base.clone() };
        for _ in 0..shake.max(1) {
            ops::mutate(&mut g, &setup.space, rng);
        }
        if seen.insert(g.key()) {
            out.push(g);
        }
    }
    out
}

fn initial_islands(cfg: &EvolveConfig, setup: &Setup, anchors: &[Anchor], rng: &mut Rng, progress: &mut Progress) -> Vec<Vec<Genome>> {
    let n = cfg.islands.max(1);
    let size = cfg.island_size.max(4);
    let mut primaries: Vec<Genome> = anchors.iter().map(|a| a.base.genome.clone()).collect();
    for (i, src) in cfg.seeds.iter().enumerate() {
        match Genome::from_toml(src) {
            Ok(g) => primaries.push(g),
            Err(e) => progress.note(format!("seed {} skipped: {e}", i + 1)),
        }
    }
    let mut islands = Vec::new();
    for i in 0..n {
        // Island i is centred on primary i, and also holds every n-th other primary unmutated.
        let base = primaries[i % primaries.len()].clone();
        let others: Vec<Genome> = primaries.iter().enumerate().filter(|(k, _)| k % n == i && *k != i % primaries.len()).map(|(_, g)| g.clone()).collect();
        islands.push(population_around(&base, &others, size, 1, setup, rng));
    }
    islands
}

/// Pick the league for a generation: all anchors, the newest `recent_count` hall members, and a
/// random `old_sample` of the rest. Returns `(opponents, display names)`.
fn pick_league(lc: &LeagueConfig, anchors: &[Anchor], hall: &[Member], template: &Arena, rng: &mut Rng) -> (Vec<Opponent>, Vec<String>) {
    let split = hall.len().saturating_sub(lc.recent_count);
    let recent: Vec<&Member> = hall[split..].iter().collect();
    let mut older: Vec<&Member> = hall[..split].iter().collect();
    let mut old: Vec<&Member> = Vec::new();
    while old.len() < lc.old_sample && !older.is_empty() {
        old.push(older.swap_remove(rng.below(older.len() as u32) as usize));
    }
    let w = league_weights(lc, anchors.len(), recent.len(), old.len());
    let mut opps = Vec::new();
    let mut names = Vec::new();
    let all = anchors.iter().map(|a| (a.base.name.clone(), &a.base.genome)).chain(recent.iter().chain(old.iter()).map(|m| (m.name.clone(), &m.genome)));
    for ((name, g), weight) in all.zip(w) {
        if let Ok(strategy) = template.compile(g) {
            names.push(name.clone());
            opps.push(Opponent { name, strategy, weight });
        }
    }
    (opps, names)
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

fn pick_parent<'a>(members: &'a [(Genome, f64)], archive: &'a Archive, cfg: &EvolveConfig, lc: &LeagueConfig, rng: &mut Rng) -> &'a Genome {
    if !archive.is_empty() && rng.chance(lc.archive_parent_rate) {
        if let Some(g) = archive.pick_parent(rng) {
            return g;
        }
    }
    tournament(members, cfg.tournament, rng)
}

fn champion_name(desc: &Descriptor, used: &mut HashSet<String>) -> String {
    let names = desc.card_names();
    let base = if names.is_empty() { "Money".to_string() } else { names.join(" + ") };
    let mut name = base.clone();
    let mut n = 1;
    while !used.insert(name.clone()) {
        n += 1;
        name = format!("{base} ({n})");
    }
    name
}

/// Run the kingdom league search. Called by [`crate::ga::run`] when `cfg.league` is set.
pub fn run(cfg: &EvolveConfig, control: &Control, report: &mut dyn FnMut(&Progress)) -> Result<Progress, String> {
    let lc = cfg.league.clone().ok_or("no league configuration")?;
    let setup = Setup::new(cfg)?;
    let kingdom: Vec<CardId> = setup.fixed_kingdom().ok_or("kingdom league needs a fixed kingdom")?.to_vec();
    let colonies = kingdom.contains(&cards::id::COLONY);
    let start = Instant::now();
    let mut progress = Progress { status: "starting".into(), generations: cfg.generations, ..Default::default() };
    let mut rng = Rng::derive(cfg.seed, 0xE70);

    // Anchors.
    let forbidden = setup.arena.forbidden.clone();
    let anchors: Vec<Anchor> = baselines(&kingdom, colonies, &forbidden).into_iter().map(|base| Anchor { base }).collect();
    let template = Arena { opponents: Vec::new(), forbidden: forbidden.clone(), fast: cfg.fast_eval, max_turns: cfg.max_turns };
    let anchor_meta = |a: &Anchor| TomlMeta {
        name: a.base.name.clone(),
        description: a.base.description.clone(),
        header: vec![format!("Baseline anchor of the kingdom league for: {}.", kingdom_text(cfg))],
        never_gain: forbidden.clone(),
        notes: Vec::new(),
    };
    let mut report_data = LeagueReport {
        anchors: anchors
            .iter()
            .map(|a| AnchorInfo { name: a.base.name.clone(), description: a.base.description.clone(), toml: a.base.genome.to_toml(&anchor_meta(a)) })
            .collect(),
        ..Default::default()
    };
    let anchor_keys: HashMap<String, usize> = anchors.iter().enumerate().map(|(i, a)| (a.base.genome.key(), i)).collect();

    // Reference game length (for the length buckets of the niche descriptor).
    let reference_turns = {
        let bmu = template.compile(&anchors[0].base.genome)?;
        let cfg_m = MatchConfig { games: 600, kingdom: kingdom.clone(), seed: Rng::derive(cfg.seed, 0x5EF).next_u64(), max_turns: cfg.max_turns };
        let s = run_match(&[&bmu, &bmu], &cfg_m);
        progress.games_played += 600;
        s[0].avg_turns().max(1.0)
    };
    report_data.reference_turns = reference_turns;

    let mut islands = initial_islands(cfg, &setup, &anchors, &mut rng, &mut progress);
    progress.note(format!(
        "kingdom league: {} anchors ({}); {} islands x {} candidates; Big Money Ultimate games last {:.1} turns",
        anchors.len(),
        anchors.iter().map(|a| a.base.name.as_str()).collect::<Vec<_>>().join(", "),
        islands.len(),
        cfg.island_size,
        reference_turns
    ));
    progress.league = Some(report_data.clone());
    progress.status = "running".into();
    report(&progress);

    let parsimony = cfg.parsimony;
    let race: Vec<u64> = if cfg.race_games.is_empty() { vec![1000] } else { cfg.race_games.clone() };
    let prm = NicheParams::default();
    let mut hall: Vec<Member> = Vec::new();
    let mut archive = Archive::default();
    let mut stagnation = Stagnation::new(islands.len(), lc.stagnation_generations);
    let mut anchor_cursor = islands.len();
    let mut last_progress_gen = 0;
    let mut last_gen = 0;
    let scenario = vec![Scenario { kingdom: kingdom.clone() }];

    for gen in 1..=cfg.generations {
        if control.stopped() {
            break;
        }
        last_gen = gen;
        let round_seed = Rng::derive(cfg.seed, gen as u64).next_u64();

        // This generation's league.
        let mut lrng = Rng::derive(round_seed, 0x01D);
        let (opps, league_names) = pick_league(&lc, &anchors, &hall, &template, &mut lrng);
        let arena = Arena { opponents: opps, forbidden: forbidden.clone(), fast: cfg.fast_eval, max_turns: cfg.max_turns };

        // Unique candidates across islands, plus the niche champions (protected through the race,
        // so they are re-measured against this generation's league like everyone else).
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut cands: Vec<Cand> = Vec::new();
        let mut membership: Vec<Vec<usize>> = Vec::new();
        let mut add = |g: &Genome, protected: bool, cands: &mut Vec<Cand>| -> usize {
            let key = g.key();
            let id = *index.entry(key.clone()).or_insert_with(|| {
                cands.push(Cand { genome: g.clone(), key, strategy: None, score: Score::default(), profile: None, stages: 0, protected: false });
                cands.len() - 1
            });
            cands[id].protected |= protected;
            id
        };
        for isl in &islands {
            let ids: Vec<usize> = isl.iter().map(|g| add(g, false, &mut cands)).collect();
            membership.push(ids);
        }
        for (_, e) in archive.iter() {
            add(&e.genome, true, &mut cands);
        }
        cands.par_iter_mut().for_each(|c| c.strategy = arena.compile(&c.genome).ok());

        // Racing: everyone plays stage 0; the best fraction (and the protected) go on.
        let mut alive: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].strategy.is_some()).collect();
        for (stage, &games) in race.iter().enumerate() {
            if stage > 0 {
                alive.sort_by(|&a, &b| fitness(&cands[b].score, &cands[b].genome, parsimony).total_cmp(&fitness(&cands[a].score, &cands[a].genome, parsimony)));
                let keep = ((alive.len() as f64 * cfg.race_keep).ceil() as usize).clamp(1, alive.len());
                let mut kept: Vec<usize> = alive[..keep].to_vec();
                kept.extend(alive[keep..].iter().copied().filter(|&i| cands[i].protected));
                alive = kept;
            }
            if control.stopped() && stage > 0 {
                break;
            }
            let stage_seed = Rng::derive(round_seed, stage as u64 + 1).next_u64();
            let profiled = stage + 1 == race.len();
            let results: Vec<(usize, Score, Option<Profile>)> = alive
                .par_iter()
                .map(|&i| {
                    let (s, p) = arena.evaluate_with(cands[i].strategy.as_ref().unwrap(), &scenario, games, stage_seed, profiled);
                    (i, s, p)
                })
                .collect();
            for (i, s, p) in results {
                cands[i].score.add(&s);
                cands[i].stages = stage + 1;
                if p.is_some() {
                    cands[i].profile = p;
                }
                progress.games_played += s.played;
            }
        }

        // Describe every candidate that ran the whole race, and update the niche archive.
        let finished: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].stages == race.len() && cands[i].profile.is_some()).collect();
        let old_archive = archive.clone();
        let old_champs: HashSet<String> = old_archive.iter().map(|(_, e)| e.genome.key()).collect();
        let mut island_of: HashMap<usize, usize> = HashMap::new();
        for (k, ids) in membership.iter().enumerate() {
            for &id in ids {
                island_of.entry(id).or_insert(k);
            }
        }
        let mut described: HashMap<usize, (Descriptor, Behaviour)> = HashMap::new();
        let mut new_archive = Archive::default();
        let mut archive_changed = false;
        let mut order = finished.clone();
        order.sort_by(|&a, &b| fitness(&cands[b].score, &cands[b].genome, parsimony).total_cmp(&fitness(&cands[a].score, &cands[a].genome, parsimony)));
        for &i in &order {
            let c = &cands[i];
            let beh = Behaviour::from_profile(c.profile.as_ref().unwrap(), c.score.turns, c.score.played);
            let d = beh.describe(reference_turns, &prm);
            let f = fitness(&c.score, &c.genome, parsimony);
            let elite = Elite {
                genome: c.genome.clone(),
                fitness: f,
                win_rate: c.score.win_rate(),
                games: c.score.played,
                behaviour: beh.clone(),
                generation: gen,
                island: island_of.get(&i).copied().unwrap_or(0),
            };
            if matches!(new_archive.offer(d, elite), Offer::New | Offer::Replaced) && !old_champs.contains(&c.key) {
                archive_changed = true;
            }
            described.insert(i, (d, beh));
        }
        new_archive.trim(lc.niches.max(1));
        archive = new_archive;
        let new_champs: HashSet<String> = archive.iter().map(|(_, e)| e.genome.key()).filter(|k| !old_champs.contains(k)).collect();
        for (k, ids) in membership.iter().enumerate() {
            if ids.iter().any(|&id| new_champs.contains(&cands[id].key)) {
                stagnation.improved(k, gen);
            }
        }

        // Selection fitness: win rate against the league, with a novelty bonus for new niches.
        let sel = |i: usize| -> f64 {
            let f = fitness(&cands[i].score, &cands[i].genome, parsimony);
            match described.get(&i) {
                Some((d, _)) if !old_archive.contains(d) => f + lc.novelty_bonus,
                _ => f,
            }
        };

        // Generation statistics.
        let Some(&champ) = order.first() else {
            progress.note("no candidate could be evaluated");
            break;
        };
        let mut rates: Vec<f64> = cands.iter().filter(|c| c.stages > 0).map(|c| c.score.win_rate()).collect();
        rates.sort_by(f64::total_cmp);
        let median = rates.get(rates.len() / 2).copied().unwrap_or(0.0);
        let cf = fitness(&cands[champ].score, &cands[champ].genome, parsimony);
        let c = &cands[champ];
        let meta = meta_for(cfg, &setup, "Candidate".into(), &described[&champ].0.label(), &format!("generation {gen}, selection seeds"), String::new());
        progress.champion = Some(sample_entry(&c.genome, &c.score, parsimony, gen, false, &meta));
        let mut stat = GenStat {
            generation: gen,
            best_fitness: cf,
            best_win_rate: c.score.win_rate(),
            median_win_rate: median,
            validated_win_rate: None,
            best_summary: c.genome.summary(),
            niches: archive.len(),
            hall: hall.len(),
        };

        // Hall of fame: the best few new strategies (distinct niches, not anchors, not members yet)
        // are validated on held-out seeds against the same league and join if they hold up.
        progress.status = "validating".into();
        report(&progress);
        let val_fast = !cfg.validate_full;
        let val_template = Arena { opponents: Vec::new(), forbidden: forbidden.clone(), fast: val_fast, max_turns: cfg.max_turns };
        let (vopps, _) = pick_league(&lc, &anchors, &hall, &val_template, &mut Rng::derive(round_seed, 0x01D));
        let val_arena = Arena { opponents: vopps, forbidden: forbidden.clone(), fast: val_fast, max_turns: cfg.max_turns };
        let mut taken_niches: HashSet<Descriptor> = HashSet::new();
        let mut joined = 0;
        let hall_keys: HashSet<String> = hall.iter().map(|m| m.key.clone()).collect();
        let vseed = Rng::derive(cfg.seed ^ 0x7A11_D000, gen as u64).next_u64();
        for &i in &order {
            if joined >= lc.join_per_generation {
                break;
            }
            let (d, _) = &described[&i];
            let c = &cands[i];
            if anchor_keys.contains_key(&c.key) || hall_keys.contains(&c.key) || taken_niches.contains(d) {
                continue;
            }
            taken_niches.insert(*d);
            let vs = match val_arena.compile(&c.genome) {
                Ok(s) => val_arena.evaluate(&s, &scenario, cfg.validate_games, vseed),
                Err(_) => Score::default(),
            };
            progress.games_played += vs.played;
            stat.validated_win_rate = Some(stat.validated_win_rate.map_or(vs.win_rate(), |v: f64| v.max(vs.win_rate())));
            if vs.win_rate() >= lc.hof_min_win {
                let name = format!("{} (gen {gen})", d.card_names().join(" + "));
                progress.note(format!("gen {gen}: joins the hall of fame, validated {:.1}% over {} games: {}", vs.win_rate() * 100.0, vs.played, c.genome.summary()));
                hall.push(Member { genome: c.genome.clone(), key: c.key.clone(), name: name.trim().to_string(), gen, val: vs });
                joined += 1;
                last_progress_gen = gen;
            }
        }
        if joined > 0 && hall.len() > cfg.hall_of_fame.max(1) {
            // Drop the weakest of the older members.
            let protect = lc.recent_count;
            let cut = hall.len().saturating_sub(protect);
            if let Some(w) = (0..cut).min_by(|&a, &b| hall[a].val.win_rate().total_cmp(&hall[b].val.win_rate())) {
                hall.remove(w);
            }
        }
        if archive_changed {
            last_progress_gen = gen;
        }
        stat.hall = hall.len();
        progress.hall_of_fame = hall
            .iter()
            .map(|m| {
                let meta = meta_for(cfg, &setup, m.name.clone(), "hall of fame", &format!("generation {}, held-out seeds", m.gen), String::new());
                sample_entry(&m.genome, &m.val, parsimony, m.gen, true, &meta)
            })
            .collect();

        // The niche table, for the UI.
        report_data.league_now = league_names;
        report_data.niches = niche_rows(cfg, &setup, &archive, &anchor_keys, &anchors, parsimony);
        progress.league = Some(report_data.clone());
        progress.history.push(stat);
        progress.generation = gen;
        progress.status = "running".into();
        progress.elapsed_secs = start.elapsed().as_secs_f64();
        progress.games_per_sec = progress.games_played as f64 / progress.elapsed_secs.max(1e-9);
        report(&progress);

        let stalled = cfg.stall_generations > 0 && gen - last_progress_gen >= cfg.stall_generations;
        if stalled {
            progress.note(format!("stopping: no new niche champion or hall-of-fame member for {} generations", gen - last_progress_gen));
        }
        if gen == cfg.generations || stalled || control.stopped() {
            break;
        }

        // Breed the next generation, island by island.
        let mut next = Vec::with_capacity(islands.len());
        for (k, ids) in membership.iter().enumerate() {
            let size = cfg.island_size.max(4);
            let mut members: Vec<(Genome, f64)> = ids.iter().map(|&i| (cands[i].genome.clone(), sel(i))).collect();
            members.sort_by(|a, b| b.1.total_cmp(&a.1));
            members.dedup_by(|a, b| a.0 == b.0);

            if stagnation.due(k, gen) {
                let n = stagnation.restarted(k, gen);
                let best = members[0].0.clone();
                let pop = if n % 2 == 1 {
                    // Shake: the island's best plus much more strongly mutated relatives.
                    progress.note(format!("gen {gen}: island {} stagnated; shaking it up", k + 1));
                    population_around(&best, &[], size, 4, &setup, &mut rng)
                } else {
                    let a = &anchors[anchor_cursor % anchors.len()];
                    anchor_cursor += 1;
                    progress.note(format!("gen {gen}: island {} stagnated; restarting from {}", k + 1, a.base.name));
                    population_around(&a.base.genome, &[], size, 2, &setup, &mut rng)
                };
                next.push(pop);
                continue;
            }

            let mut out: Vec<Genome> = members.iter().take(cfg.elite.min(size)).map(|m| m.0.clone()).collect();
            let mut keys: HashSet<String> = out.iter().map(Genome::key).collect();
            let mut tries = 0;
            while out.len() < size && tries < size * 20 {
                tries += 1;
                let a = pick_parent(&members, &archive, cfg, &lc, &mut rng);
                let child = if rng.chance(cfg.crossover_rate) {
                    let b = pick_parent(&members, &archive, cfg, &lc, &mut rng);
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
        // Migration: each island's best replaces the next island's worst, and a champion of a
        // random niche replaces the second worst of a random island.
        if cfg.migration_interval > 0 && gen % cfg.migration_interval == 0 && next.len() > 1 {
            let bests: Vec<Genome> = next.iter().map(|isl| isl[0].clone()).collect();
            let n = next.len();
            for (k, b) in bests.into_iter().enumerate() {
                let dst = &mut next[(k + 1) % n];
                if !dst.contains(&b) {
                    *dst.last_mut().unwrap() = b;
                }
            }
            if let Some(g) = archive.pick_parent(&mut rng).cloned() {
                let dst = &mut next[rng.below(n as u32) as usize];
                if dst.len() > 2 && !dst.contains(&g) {
                    let at = dst.len() - 2;
                    dst[at] = g;
                }
            }
        }
        islands = next;
    }

    if last_gen == 0 {
        progress.status = "stopped".into();
        report(&progress);
        return Ok(progress);
    }

    // ---- Final ranking -------------------------------------------------------------------
    progress.status = "polishing".into();
    report(&progress);
    let final_fast = !lc.final_full;
    let final_template = Arena { opponents: Vec::new(), forbidden: forbidden.clone(), fast: final_fast, max_turns: cfg.max_turns };
    let mut champions: Vec<(Descriptor, Genome, Elite)> = archive.champions().into_iter().map(|(d, e)| (*d, e.genome.clone(), e.clone())).collect();
    // Champions that are exactly an anchor are ranked as that anchor.
    champions.retain(|(_, g, _)| !anchor_keys.contains_key(&g.key()));
    if cfg.polish_games > 0 && !champions.is_empty() && !control.stopped() {
        progress.note("polishing the niche champions (dropping rules that don't pay)");
        report(&progress);
        let (popps, _) = pick_league(&lc, &anchors, &hall, &template, &mut Rng::derive(cfg.seed, 0x9011));
        let parena = Arena { opponents: popps, forbidden: forbidden.clone(), fast: cfg.fast_eval, max_turns: cfg.max_turns };
        let pseed = Rng::derive(cfg.seed ^ 0x9011_5400, 1).next_u64();
        for (_, g, _) in champions.iter_mut() {
            let p = polish(g, &parena, &setup.space, &scenario, cfg.polish_games, pseed, parsimony);
            progress.games_played += p.score.played;
            *g = p.genome;
        }
        let mut seen = HashSet::new();
        champions.retain(|(_, g, _)| seen.insert(g.key()) && !anchor_keys.contains_key(&g.key()));
    }

    progress.status = "ranking".into();
    progress.note(format!(
        "round robin: {} anchors + {} niche champions, {} games per pair ({} bots)",
        anchors.len(),
        champions.len(),
        lc.final_games,
        if lc.final_full { "full" } else { "fast" }
    ));
    report(&progress);
    let mut names: Vec<String> = anchors.iter().map(|a| a.base.name.clone()).collect();
    let mut genomes: Vec<Genome> = anchors.iter().map(|a| a.base.genome.clone()).collect();
    let mut used: HashSet<String> = names.iter().cloned().collect();
    for (d, g, _) in &champions {
        names.push(champion_name(d, &mut used));
        genomes.push(g.clone());
    }
    let strategies: Vec<Strategy> = genomes.iter().map(|g| final_template.compile(g)).collect::<Result<_, _>>()?;
    let refs: Vec<&Strategy> = strategies.iter().collect();
    let rr_seed = Rng::derive(cfg.seed ^ 0xF1AA_0000, 1).next_u64();
    let rr = rank::round_robin(&refs, &names, lc.final_games.max(2), &kingdom, rr_seed, cfg.max_turns);
    let n = names.len();
    progress.games_played += (n * (n - 1) / 2) as u64 * rr.table.games;

    // Behaviour of each strategy over its round-robin games, labels, entries.
    let mut descriptors: Vec<Descriptor> = Vec::new();
    let mut behaviours: Vec<Behaviour> = Vec::new();
    for i in 0..n {
        let b = Behaviour::from_profile(&rr.profiles[i], rr.turns_sum[i], rr.games_played[i]);
        descriptors.push(b.describe(reference_turns, &prm));
        behaviours.push(b);
    }
    let field_games = (rr.table.games * (n as u64 - 1).max(1)) as f64;
    let mut entries: Vec<Entry> = Vec::new();
    let mut kinds: Vec<String> = Vec::new();
    for i in 0..n {
        let is_anchor = i < anchors.len();
        let wins = rr.table.mean_vs_field[i] * field_games;
        let (lo, hi) = wilson_ci(wins, field_games, Z_95);
        let label = descriptors[i].label();
        let extra = format!("Round robin on this kingdom: {:.1}% against the field, Nash weight {:.0}%.", rr.table.mean_vs_field[i] * 100.0, rr.table.nash[i] * 100.0);
        let meta = if is_anchor {
            let mut m = anchor_meta(&anchors[i]);
            m.header.push(extra);
            m
        } else {
            meta_for(cfg, &setup, format!("League: {}", names[i]), &label, "niche champion", extra)
        };
        let g = &genomes[i];
        entries.push(Entry {
            toml: g.to_toml(&meta),
            summary: g.summary(),
            win_rate: rr.table.mean_vs_field[i],
            ci_lo: lo,
            ci_hi: hi,
            games: field_games as u64,
            size: g.size(),
            fitness: rr.table.mean_vs_field[i],
            generation: last_gen,
            validated: true,
        });
        kinds.push(if is_anchor { "anchor".into() } else { "champion".into() });
    }

    // Top N distinct strategies by mean against the field.
    let mut by_mean: Vec<usize> = (0..n).collect();
    by_mean.sort_by(|&a, &b| rr.table.mean_vs_field[b].total_cmp(&rr.table.mean_vs_field[a]));
    let mut top: Vec<TopRow> = Vec::new();
    let mut seen_desc: Vec<Descriptor> = Vec::new();
    for &i in &by_mean {
        if top.len() >= lc.top_n.max(1) {
            break;
        }
        if seen_desc.contains(&descriptors[i]) {
            continue;
        }
        seen_desc.push(descriptors[i]);
        top.push(TopRow {
            rank: top.len() + 1,
            index: i,
            name: names[i].clone(),
            description: format!("{} ({}; {})", descriptors[i].label(), behaviours[i].text(), genomes[i].summary()),
            baseline: i < anchors.len(),
            mean_vs_field: rr.table.mean_vs_field[i],
            nash: rr.table.nash[i],
        });
    }

    report_data.crosstable = Some(rr.table.clone());
    report_data.strategies = entries.clone();
    report_data.kinds = kinds;
    report_data.top = top;
    progress.league = Some(report_data);
    // The result: the best evolved strategy (an anchor only if no champion exists).
    let best = by_mean.iter().copied().find(|&i| i >= anchors.len()).or_else(|| by_mean.first().copied());
    progress.result = best.map(|i| entries[i].clone());
    if let Some(i) = best {
        progress.note(format!("result: {} - {:.1}% against the field: {}", names[i], rr.table.mean_vs_field[i] * 100.0, genomes[i].summary()));
    }
    progress.status = if control.stopped() { "stopped".into() } else { "done".into() };
    progress.elapsed_secs = start.elapsed().as_secs_f64();
    progress.games_per_sec = progress.games_played as f64 / progress.elapsed_secs.max(1e-9);
    report(&progress);
    Ok(progress)
}

fn niche_rows(cfg: &EvolveConfig, setup: &Setup, archive: &Archive, anchor_keys: &HashMap<String, usize>, anchors: &[Anchor], parsimony: f64) -> Vec<NicheRow> {
    archive
        .champions()
        .into_iter()
        .map(|(d, e)| {
            let label = d.label();
            let key = e.genome.key();
            let how = match anchor_keys.get(&key) {
                Some(&k) => format!("this is the baseline {}", anchors[k].base.name),
                None => format!("generation {}", e.generation),
            };
            let meta = meta_for(cfg, setup, format!("League: {label}"), &label, &how, String::new());
            let score = Score { wins: e.win_rate * e.games as f64, games: e.games as f64, played: e.games, turns: 0.0 };
            NicheRow {
                label,
                key_cards: d.card_names().into_iter().map(String::from).collect(),
                trash: crate::niche::TRASH_LABELS[d.trash as usize].to_string(),
                length: crate::niche::LENGTH_LABELS[d.length as usize].to_string(),
                behaviour: e.behaviour.text(),
                entry: sample_entry(&e.genome, &score, parsimony, e.generation, false, &meta),
                island: e.island + 1,
                generation: e.generation,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn league_weights_split_by_group_and_renormalize_when_groups_are_empty() {
        let lc = LeagueConfig::default(); // 0.4 / 0.4 / 0.2
        let w = league_weights(&lc, 4, 2, 1);
        assert_eq!(w.len(), 7);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!((w[0] - 0.1).abs() < 1e-9 && (w[4] - 0.2).abs() < 1e-9 && (w[6] - 0.2).abs() < 1e-9, "{w:?}");
        // No hall of fame yet: the anchors are the whole league.
        let w = league_weights(&lc, 4, 0, 0);
        assert!(w.iter().all(|x| (x - 0.25).abs() < 1e-9), "{w:?}");
        // No old members: the other groups renormalize (0.4 : 0.4).
        let w = league_weights(&lc, 2, 2, 0);
        assert!(w.iter().all(|x| (x - 0.25).abs() < 1e-9), "{w:?}");
        assert!(league_weights(&lc, 0, 0, 0).is_empty());
        // Zero weights everywhere: equal shares.
        let z = LeagueConfig { anchor_weight: 0.0, recent_weight: 0.0, old_weight: 0.0, ..lc };
        let w = league_weights(&z, 2, 2, 0);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_island_is_due_after_the_limit_without_a_new_champion_and_restarts_alternate() {
        let mut s = Stagnation::new(2, 5);
        s.improved(0, 3);
        assert!(!s.due(0, 7));
        assert!(s.due(0, 8));
        assert!(s.due(1, 5), "island 1 never contributed");
        assert_eq!(s.restarted(0, 8), 1);
        assert!(!s.due(0, 9), "a restart resets the clock");
        assert_eq!(s.restarted(0, 14), 2);
        assert!(!Stagnation::new(1, 0).due(0, 100), "limit 0 never restarts");
    }
}
