//! Massively parallel batch simulation over independent games (rayon).
//!
//! Each game is fully independent; its seed is derived from `(base_seed, game_index)` via
//! `dominion_engine::rng::Rng::derive`, so results are reproducible regardless of thread count
//! or chunking. Per-thread work is folded into a local `[StratStats; MAX_PLAYERS]` accumulator
//! (no locks/atomics in the per-game loop) and reduced once at the end. Seats rotate every
//! `players` games so each strategy plays every seat equally (this also gives the strategies
//! common random numbers: `game_index / players` is the same underlying seed across a
//! rotation's `players` games).

use dominion_engine::cards::{self, CardId};
use dominion_engine::rng::Rng;
use dominion_engine::state::MAX_PLAYERS;
use dominion_engine::{play_game, Agent, Event, EventSink, GameConfig, GameState, NoEvents};
use rayon::prelude::*;

use crate::agent::StrategyAgent;
use crate::stats::StratStats;
use crate::strategy::Strategy;

/// Make rayon's worker threads big enough for the bots' searches (they recurse over copies of the `GameState`, which is a few KB each):
/// the default stack of a worker is too small once a game state is this large. Call before the first parallel batch; later calls do nothing.
pub fn init_thread_pool() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = rayon::ThreadPoolBuilder::new().stack_size(64 << 20).build_global();
    });
}

pub struct MatchConfig {
    pub games: u64,
    pub kingdom: Vec<CardId>,
    pub seed: u64,
    pub max_turns: u16,
}

/// Extra per-game tallies a match can collect besides the win counts. The default `()` collects
/// nothing and costs nothing (`NoEvents` compiles away); [`Profiles`] counts gains and trashes.
pub trait Tally: Default + Send {
    type Sink: EventSink;
    fn sink() -> Self::Sink;
    /// Fold in one finished game. `strat_of_seat[i]` is the index (in the `strategies` slice) of
    /// the strategy that sat in seat `i`.
    fn absorb(&mut self, sink: &Self::Sink, strat_of_seat: &[usize]);
    fn merge(&mut self, other: &Self);
}

impl Tally for () {
    type Sink = NoEvents;
    #[inline(always)]
    fn sink() -> NoEvents {
        NoEvents
    }
    #[inline(always)]
    fn absorb(&mut self, _: &NoEvents, _: &[usize]) {}
    #[inline(always)]
    fn merge(&mut self, _: &()) {}
}

/// Ids at or above this (Events, Landmarks, Boons, ...) are not counted by [`Profile`]: the ids that can sit in a zone.
pub const PROFILE_IDS: usize = cards::NUM_ZONE_IDS;

/// What one strategy did over a batch of games: cards gained (bought or otherwise) and cards
/// trashed. Cheap statistics for describing a strategy's behaviour.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub games: u64,
    /// Gains per card id, summed over games.
    pub gains: Vec<u64>,
    pub trashes: u64,
    /// Purchases per Event (index `event - cards::FIRST_EVENT`), summed over games; empty when no Event was ever bought.
    pub events: Vec<u64>,
}

impl Profile {
    pub fn gains_per_game(&self, card: CardId) -> f64 {
        if self.games == 0 { 0.0 } else { self.gains.get(card as usize).copied().unwrap_or(0) as f64 / self.games as f64 }
    }
    /// Times the strategy bought `event` per game.
    pub fn events_per_game(&self, event: CardId) -> f64 {
        let i = event.wrapping_sub(cards::FIRST_EVENT) as usize;
        if self.games == 0 { 0.0 } else { self.events.get(i).copied().unwrap_or(0) as f64 / self.games as f64 }
    }
    pub fn trashes_per_game(&self) -> f64 {
        if self.games == 0 { 0.0 } else { self.trashes as f64 / self.games as f64 }
    }
    pub fn merge(&mut self, o: &Profile) {
        self.games += o.games;
        self.trashes += o.trashes;
        if self.gains.len() < o.gains.len() {
            self.gains.resize(o.gains.len(), 0);
        }
        for (a, b) in self.gains.iter_mut().zip(&o.gains) {
            *a += b;
        }
        if self.events.len() < o.events.len() {
            self.events.resize(o.events.len(), 0);
        }
        for (a, b) in self.events.iter_mut().zip(&o.events) {
            *a += b;
        }
    }
}

pub struct ProfileSink {
    gains: [[u16; PROFILE_IDS]; MAX_PLAYERS],
    trashes: [u32; MAX_PLAYERS],
    /// Event purchases per seat (Events are bought, never gained, so they have no `Gain` event).
    events: [[u8; cards::NUM_EVENTS]; MAX_PLAYERS],
}

impl EventSink for ProfileSink {
    fn event(&mut self, e: Event) {
        match e {
            Event::Gain { player, card, .. } if (card as usize) < PROFILE_IDS => {
                self.gains[player as usize][card as usize] += 1;
            }
            Event::Trash { player, .. } => self.trashes[player as usize] += 1,
            Event::Buy { player, card } if cards::is_event(card) => {
                let n = &mut self.events[player as usize][(card - cards::FIRST_EVENT) as usize];
                *n = n.saturating_add(1);
            }
            _ => {}
        }
    }
}

/// One [`Profile`] per strategy.
#[derive(Clone, Debug, Default)]
pub struct Profiles(pub Vec<Profile>);

impl Tally for Profiles {
    type Sink = ProfileSink;
    fn sink() -> ProfileSink {
        ProfileSink { gains: [[0; PROFILE_IDS]; MAX_PLAYERS], trashes: [0; MAX_PLAYERS], events: [[0; cards::NUM_EVENTS]; MAX_PLAYERS] }
    }
    fn absorb(&mut self, sink: &ProfileSink, strat_of_seat: &[usize]) {
        if self.0.len() < MAX_PLAYERS {
            self.0.resize(MAX_PLAYERS, Profile::default());
        }
        for (seat, &s) in strat_of_seat.iter().enumerate() {
            let p = &mut self.0[s];
            if p.gains.is_empty() {
                p.gains = vec![0; PROFILE_IDS];
            }
            p.games += 1;
            p.trashes += sink.trashes[seat] as u64;
            if sink.events[seat].iter().any(|&n| n > 0) {
                if p.events.is_empty() {
                    p.events = vec![0; cards::NUM_EVENTS];
                }
                for (a, b) in p.events.iter_mut().zip(sink.events[seat].iter()) {
                    *a += *b as u64;
                }
            }
            for (a, b) in p.gains.iter_mut().zip(sink.gains[seat].iter()) {
                *a += *b as u64;
            }
        }
    }
    fn merge(&mut self, o: &Profiles) {
        if self.0.len() < o.0.len() {
            self.0.resize(o.0.len(), Profile::default());
        }
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            a.merge(b);
        }
    }
}

/// Play `strategies.len()` strategies against each other (one per seat) for `cfg.games` games,
/// rotating seats: each group of `p` consecutive games shares one deal and rotates the seating,
/// so every strategy sits in every seat once (2 players: both go first once). With 3+ players
/// each group also starts from a random seating, so every relative order comes up. Returns one
/// `StratStats` per input strategy, in input order.
pub fn run_match(strategies: &[&Strategy], cfg: &MatchConfig) -> Vec<StratStats> {
    run_match_tally::<()>(strategies, cfg).0
}

/// [`run_match`] that also profiles each strategy's gains and trashes.
pub fn run_match_profiled(strategies: &[&Strategy], cfg: &MatchConfig) -> (Vec<StratStats>, Vec<Profile>) {
    let (stats, mut t) = run_match_tally::<Profiles>(strategies, cfg);
    t.0.resize(strategies.len(), Profile::default());
    (stats, t.0)
}

fn run_match_tally<T: Tally>(strategies: &[&Strategy], cfg: &MatchConfig) -> (Vec<StratStats>, T) {
    init_thread_pool();
    let p = strategies.len();
    assert!((2..=MAX_PLAYERS).contains(&p), "match needs 2..={MAX_PLAYERS} strategies, got {p}");

    let base_seed = cfg.seed;
    let base_cfg = GameConfig { num_players: p, kingdom: cfg.kingdom.clone(), seed: 0, max_turns: cfg.max_turns, ..Default::default() };

    let (totals, tally): ([StratStats; MAX_PLAYERS], T) = (0..cfg.games)
        .into_par_iter()
        .fold(
            || (base_cfg.clone(), [StratStats::default(); MAX_PLAYERS], T::default()),
            |(mut gcfg, mut acc, mut tally), g| {
                let rotation = (g % p as u64) as usize;
                gcfg.seed = Rng::derive(base_seed, g / p as u64).next_u64();
                // Base seating for this group: identity for 2 players (rotation already covers
                // both orders), a random order for 3+.
                let mut base: [usize; MAX_PLAYERS] = std::array::from_fn(|i| i);
                if p >= 3 {
                    let mut r = Rng::derive(base_seed ^ 0x5EA7_0D3E, g / p as u64);
                    for i in (1..p).rev() {
                        base.swap(i, r.below(i as u32 + 1) as usize);
                    }
                }
                let mut state = GameState::new(&gcfg);

                // Build `p` agents on the stack, then split them into disjoint `&mut dyn Agent`
                // slots. No heap allocation: `agents` is a fixed-size array, not a Vec.
                let mut agents: [StrategyAgent; MAX_PLAYERS] = std::array::from_fn(|i| {
                    let idx = if i < p { base[(i + rotation) % p] } else { 0 };
                    StrategyAgent::new(strategies[idx])
                });
                let mut refs: [&mut dyn Agent; MAX_PLAYERS] = {
                    let mut it = agents.iter_mut();
                    std::array::from_fn(|_| it.next().unwrap() as &mut dyn Agent)
                };

                let mut sink = T::sink();
                let result = play_game(&mut state, &mut refs[..p], &mut sink);
                let strat_of_seat: [usize; MAX_PLAYERS] = std::array::from_fn(|i| base[(i + rotation) % p]);
                tally.absorb(&sink, &strat_of_seat[..p]);
                let win_share = 1.0 / result.winners.count_ones().max(1) as f64;
                for i in 0..p {
                    let strat_idx = strat_of_seat[i];
                    let s = &mut acc[strat_idx];
                    s.games += 1;
                    if result.winners & (1 << i) != 0 {
                        s.wins += win_share;
                    }
                    s.vp_sum += result.scores[i] as f64;
                    s.turns_sum += result.turns[i] as f64;
                    if result.capped {
                        s.capped_games += 1;
                    }
                }
                (gcfg, acc, tally)
            },
        )
        .map(|(_, acc, tally)| (acc, tally))
        .reduce(
            || ([StratStats::default(); MAX_PLAYERS], T::default()),
            |(mut a, mut ta), (b, tb)| {
                for i in 0..MAX_PLAYERS {
                    a[i].merge(&b[i]);
                }
                ta.merge(&tb);
                (a, ta)
            },
        );

    (totals[..p].to_vec(), tally)
}

/// One row of a round-robin league table: `a` vs `b`, from `a`'s perspective.
pub struct LeagueCell {
    pub a: usize,
    pub b: usize,
    pub a_stats: StratStats,
    pub b_stats: StratStats,
}

/// Round-robin: every unordered pair of `strategies` plays a 2-player match of `games_per_match`
/// games. Pairs run in parallel too (via rayon), each pair's games also parallelized internally.
pub fn run_league(strategies: &[&Strategy], games_per_match: u64, kingdom: &[CardId], seed: u64, max_turns: u16) -> Vec<LeagueCell> {
    init_thread_pool();
    let n = strategies.len();
    let mut pairs = Vec::with_capacity(n * (n - 1) / 2);
    for i in 0..n {
        for j in (i + 1)..n {
            pairs.push((i, j));
        }
    }
    pairs
        .into_par_iter()
        .map(|(i, j)| {
            // Distinct seed per pair so different pairs don't share random streams.
            let pair_seed = Rng::derive(seed, (i as u64) << 32 | j as u64).next_u64();
            let cfg = MatchConfig { games: games_per_match, kingdom: kingdom.to_vec(), seed: pair_seed, max_turns };
            let stats = run_match(&[strategies[i], strategies[j]], &cfg);
            LeagueCell { a: i, b: j, a_stats: stats[0], b_stats: stats[1] }
        })
        .collect()
}
