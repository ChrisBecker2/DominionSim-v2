//! Insight: how the games between two strategies go, told for people rather than for the search. Plays the games once and reports
//!
//! - the win rate by opening split (every split seen: 5/2 and 4/3, others with Shelters, Heirlooms or a changed deck) and every
//!   opening of each split;
//! - what each strategy gains and trashes, turn by turn (a heat map), and how that goes with winning;
//! - win rate by how many copies of a card were gained by turn 10, and owned at the end (the points where one more copy starts to lose);
//! - races: for each card, how often the first player to gain it wins;
//! - pairs of cards in the final deck, the Prophecy (when, and who made it happen), differently named cards (Museum);
//! - decision trees: at the start of own turns 5, 9, 13 and 17, a shallow tree over the position (VP lead, money, the cards owned, the
//!   opponent's) that predicts the winner; its leaves read as conditions of the strategy language ("count(Gold) >= 1 and vp_lead < -4");
//! - turning points: the gains between two of those turns that move the tree's estimate most, beyond what the position already said;
//! - rule explanations ([`explain`]): each rule of a strategy taken out, the same deals replayed: what the rule is worth and when it acts;
//! - kingdom findings ([`findings`]): the above for a league's champion against the whole field of its ranking, boiled down to
//!   ranked sentences: what decides games on the kingdom and what the champion does about it.
//!
//! Counts at the end are partly a symptom (a game being lost runs longer and gains more of everything); the "by turn 10" counts, the trees
//! and the turning points are the better guide.

use std::collections::BTreeMap;

use rayon::prelude::*;
use serde::Serialize;

use dominion_engine::cards::{self, id, CardId};
use dominion_engine::rng::Rng;
use dominion_engine::{Agent, ChoiceBuf, DecisionKind, Event, EventSink, GameConfig, GameState, PlayerView, Step};
use dominion_sim::{Strategy, StrategyAgent};

use crate::genome::{Genome, List, TomlMeta};

/// Columns of the turn heat maps (own turns 1..=TURNS).
pub const TURNS: usize = 24;
/// Own turns at whose start the position is recorded for the decision trees.
pub const SNAP_TURNS: [u16; 4] = [5, 9, 13, 17];

/// Rules only, no turn search (as the Lab's fast evaluation plays).
pub fn make_fast(s: &mut Strategy) {
    s.search_play = false;
    s.win_this_turn = false;
}

// ---------------------------------------------------------------------------------------------
// Playing and recording
// ---------------------------------------------------------------------------------------------

/// Base features of a position, in this order (then `count(..)` of the cards owned, and the opponent's).
const BASE: [&str; 7] = ["vp_lead", "money", "total_cards", "count_type(action)", "debt", "provinces_left", "opp money"];

#[derive(Clone)]
struct Snap {
    /// Index into `SNAP_TURNS`.
    at: usize,
    /// The game's turn counter when it was taken (for the opponent's gains in between).
    global: u16,
    base: [i32; BASE.len()],
    mine: Vec<(CardId, u16)>,
    theirs: Vec<(CardId, u16)>,
}

#[derive(Default)]
struct Sink {
    own_turn: [u16; 2],
    global: u16,
    cur: u8,
    /// The first ten cards each player draws: the opening hands.
    opening: [Vec<CardId>; 2],
    /// (own turn, game turn, card).
    gains: [Vec<(u16, u16, CardId)>; 2],
    /// The cards each player gained on its own first two turns: its opening (not a Curse an opponent's Witch gave it meanwhile).
    opened: [Vec<CardId>; 2],
    /// The card being played (or replayed), and a card its player trashed while it resolved.
    playing: CardId,
    trashed: Option<(u8, CardId)>,
    /// Trash for gain: (own turn, the card played, the card it trashed, the card it gained), e.g. Remodel: Gold -> Province.
    exchanges: [Vec<(u16, CardId, CardId, CardId)>; 2],
    trashes: [Vec<(u16, u16, CardId)>; 2],
    prophecy: Option<(u8, u16)>,
    /// Each card gained in the game, in the order of the first gain of it, and who gained it first.
    first: Vec<(CardId, u8)>,
}

impl EventSink for Sink {
    fn event(&mut self, e: Event) {
        match e {
            Event::TurnStart { player, turn } => {
                self.cur = player;
                self.global = turn;
                self.playing = 0;
                self.trashed = None;
                if (player as usize) < 2 {
                    self.own_turn[player as usize] += 1;
                }
            }
            Event::Draw { player, card } if (player as usize) < 2 && self.opening[player as usize].len() < 10 => self.opening[player as usize].push(card),
            Event::Play { card, .. } | Event::PlayAgain { card, .. } => {
                self.playing = card;
                self.trashed = None;
            }
            Event::Buy { .. } => {
                self.playing = 0;
                self.trashed = None;
            }
            Event::Gain { player, card, source, .. } if (player as usize) < 2 => {
                // A gain by the card being played after it trashed one of its player's cards: a trash-for-gain (Remodel, Expand, Mine...).
                if let Some((tp, from)) = self.trashed {
                    if tp == player && source != 0 && source == self.playing {
                        self.exchanges[player as usize].push((self.own_turn[player as usize], source, from, card));
                    }
                }
                self.gains[player as usize].push((self.own_turn[player as usize], self.global, card));
                if self.cur == player && self.own_turn[player as usize] <= 2 {
                    self.opened[player as usize].push(card);
                }
                if !self.first.iter().any(|&(c, _)| c == card) {
                    self.first.push((card, player));
                }
            }
            Event::Trash { player, card } if (player as usize) < 2 => {
                self.trashes[player as usize].push((self.own_turn[player as usize], self.global, card));
                if self.playing != 0 && player == self.cur {
                    self.trashed = Some((player, card));
                }
            }
            Event::Prophecy { player, .. } if self.prophecy.is_none() => self.prophecy = Some((player, self.own_turn[(self.cur as usize).min(1)])),
            _ => {}
        }
    }
}

/// One game, from the point of view of the strategies (index 0 = the first strategy), not the seats.
struct Game {
    win: [f64; 2],
    split: [(u8, u8); 2],
    gains: [Vec<(u16, u16, CardId)>; 2],
    trashes: [Vec<(u16, u16, CardId)>; 2],
    /// The cards gained on its own first two turns (see `Sink::opened`).
    opened: [Vec<CardId>; 2],
    /// The opening's coins in the order played: (first hand, second hand).
    split_ordered: [(u8, u8); 2],
    /// The opening hands' cards other than Copper: (the hand with more Coppers, the other), each sorted. Estates in most games;
    /// Shelters (Hovel, Necropolis, Overgrown Estate) or Heirlooms in some.
    extras: [(Vec<CardId>, Vec<CardId>); 2],
    /// Trash for gain (see `Sink::exchanges`).
    exchanges: [Vec<(u16, CardId, CardId, CardId)>; 2],
    /// Each card gained, and the strategy that gained it first.
    first: Vec<(CardId, usize)>,
    /// The strategy that made the Prophecy happen, and on which of its own turns.
    prophecy: Option<(usize, u16)>,
    deck: [Vec<(CardId, u16)>; 2],
    vp: [i32; 2],
    turns: [u16; 2],
    snaps: [Vec<Snap>; 2],
    /// Against a field ([`play_field`]): which of its strategies played side 1.
    opp: usize,
}

fn sparse(c: &dominion_engine::Counts) -> Vec<(CardId, u16)> {
    c.iter().filter(|&(_, n)| n > 0).map(|(c, n)| (c, n as u16)).collect()
}

fn money_of(c: &dominion_engine::Counts) -> i32 {
    c.iter().filter(|&(c, _)| cards::is(c, cards::TREASURE)).map(|(c, n)| cards::def(c).coins as i32 * n as i32).sum()
}

fn snapshot(st: &GameState, p: u8, at: usize, global: u16) -> Snap {
    let view = PlayerView::new(st, p);
    let o = 1 - p;
    let mine = view.my_cards();
    let theirs = view.cards_of(o);
    Snap {
        at,
        global,
        base: [
            view.vp_of(p) - view.vp_of(o),
            money_of(&mine),
            mine.total() as i32,
            mine.count_type(cards::ACTION) as i32,
            view.debt_of(p) as i32,
            view.supply(id::PROVINCE) as i32,
            money_of(&theirs),
        ],
        mine: sparse(&mine),
        theirs: sparse(&theirs),
    }
}

fn play_one(kingdom: &[CardId], s: [&Strategy; 2], seed: u64, swap: bool) -> Game {
    let mut st = GameState::new(&GameConfig { num_players: 2, kingdom: kingdom.to_vec(), seed, max_turns: 120, ..Default::default() });
    st.pause_at_turn_start = true;
    let (mut a, mut b) = (StrategyAgent::new(s[0]), StrategyAgent::new(s[1]));
    let agents: [&mut dyn Agent; 2] = if swap { [&mut b, &mut a] } else { [&mut a, &mut b] };
    let mut sink = Sink::default();
    let mut snaps: [Vec<Snap>; 2] = Default::default();
    let mut buf = ChoiceBuf::default();
    loop {
        match st.advance(&mut sink) {
            Step::Decision(d) => {
                if d.kind == DecisionKind::Buy {
                    let c = match st.buy_avail() {
                        Some(avail) => agents[d.player as usize].choose_buy(&PlayerView::new(&st, d.player), &avail),
                        None => None,
                    };
                    if let Some(c) = c {
                        st.apply_trusted(c, &mut sink);
                        continue;
                    }
                }
                st.legal_choices(&mut buf);
                let c = agents[d.player as usize].choose(&PlayerView::new(&st, d.player), &d, buf.as_slice());
                st.apply_listed(c, &buf, &mut sink).expect("agent chose an illegal option");
            }
            Step::TurnStart { player } => {
                let p = player as usize;
                if p < 2 {
                    if let Some(at) = SNAP_TURNS.iter().position(|&t| t == sink.own_turn[p]) {
                        if snaps[p].last().map_or(true, |s| s.at != at) {
                            snaps[p].push(snapshot(&st, player, at, sink.global));
                        }
                    }
                }
            }
            Step::Chance { .. } => unreachable!("play_one samples draws"),
            Step::GameOver => break,
        }
    }
    let scores = st.scores();
    let winners = st.winners();
    let seat = |i: usize| if swap { 1 - i } else { i };
    let share = 1.0 / winners.count_ones().max(1) as f64;
    let coppers = |v: &[CardId]| v.iter().filter(|&&c| c == id::COPPER).count() as u8;
    let [s0, s1] = snaps;
    Game {
        win: std::array::from_fn(|i| if winners & (1 << seat(i)) != 0 { share } else { 0.0 }),
        split: std::array::from_fn(|i| {
            let o = &sink.opening[seat(i)];
            let k = o.len().min(5);
            let (x, y) = (coppers(&o[..k]), coppers(&o[k..]));
            (x.max(y), x.min(y))
        }),
        gains: std::array::from_fn(|i| sink.gains[seat(i)].clone()),
        opened: std::array::from_fn(|i| sink.opened[seat(i)].clone()),
        split_ordered: std::array::from_fn(|i| {
            let o = &sink.opening[seat(i)];
            let k = o.len().min(5);
            (coppers(&o[..k]), coppers(&o[k..]))
        }),
        exchanges: std::array::from_fn(|i| sink.exchanges[seat(i)].clone()),
        extras: std::array::from_fn(|i| {
            let o = &sink.opening[seat(i)];
            let k = o.len().min(5);
            let (a, b) = if coppers(&o[..k]) >= coppers(&o[k..]) { (&o[..k], &o[k..]) } else { (&o[k..], &o[..k]) };
            let rest = |h: &[CardId]| {
                let mut x: Vec<CardId> = h.iter().copied().filter(|&c| c != id::COPPER).collect();
                x.sort_unstable();
                x
            };
            (rest(a), rest(b))
        }),
        trashes: std::array::from_fn(|i| sink.trashes[seat(i)].clone()),
        first: sink.first.iter().map(|&(c, p)| (c, if seat(0) == p as usize { 0 } else { 1 })).collect(),
        prophecy: sink.prophecy.map(|(p, t)| (if seat(0) == p as usize { 0 } else { 1 }, t)),
        deck: std::array::from_fn(|i| sparse(&PlayerView::new(&st, seat(i) as u8).my_cards())),
        vp: std::array::from_fn(|i| scores[seat(i)]),
        turns: std::array::from_fn(|i| st.players[seat(i)].turns_taken),
        snaps: if swap { [s1, s0] } else { [s0, s1] },
        opp: 0,
    }
}

/// `games` games, seats alternating, every pair of games on one deal (common random numbers).
fn play_many(kingdom: &[CardId], s: [&Strategy; 2], games: u64, seed: u64) -> Vec<Game> {
    (0..games).into_par_iter().map(|g| play_one(kingdom, s, Rng::derive(seed, g / 2).next_u64(), g % 2 == 1)).collect()
}

/// About `games` games of `a` against a field, the same number against each of its strategies (an even number, at least 2), each
/// opponent on its own deals; the same `seed` replays the same deals.
fn play_field(kingdom: &[CardId], a: &Strategy, field: &[&Strategy], games: u64, seed: u64) -> Vec<Game> {
    let each = (games / field.len().max(1) as u64).max(2) & !1;
    let mut all = Vec::new();
    for (k, b) in field.iter().enumerate() {
        all.extend(play_many(kingdom, [a, b], each, Rng::derive(seed, k as u64).next_u64()).into_iter().map(|mut g| {
            g.opp = k;
            g
        }));
    }
    all
}

// ---------------------------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------------------------

/// A win rate over some games (ties count half).
#[derive(Serialize, Clone, Copy, Debug, Default)]
pub struct Rate {
    pub games: usize,
    pub win: f64,
}

fn rate<'a>(it: impl Iterator<Item = f64>) -> Rate {
    let (mut n, mut w) = (0, 0.0);
    for x in it {
        n += 1;
        w += x;
    }
    Rate { games: n, win: if n > 0 { w / n as f64 } else { 0.0 } }
}

#[derive(Serialize, Debug, Clone)]
pub struct Opening {
    /// The cards gained on the first two turns, e.g. "Change + Silver".
    pub cards: String,
    pub rate: Rate,
}

#[derive(Serialize, Debug, Clone)]
pub struct SplitRow {
    /// "5/2" or "4/3".
    pub split: String,
    pub rate: Rate,
    /// The same, by the opponent's split.
    pub vs: Vec<(String, Rate)>,
    pub openings: Vec<Opening>,
}

#[derive(Serialize, Debug, Clone)]
pub struct HeatRow {
    pub card: String,
    /// Average copies gained (trashed) per game on each own turn 1..=TURNS.
    pub per_turn: Vec<f64>,
    /// Per game, over the whole game.
    pub total: f64,
    /// Games with at least one before own turn 10, and the rest.
    pub early: Rate,
    pub late: Rate,
}

#[derive(Serialize, Debug, Clone)]
pub struct CountCurve {
    pub card: String,
    /// (copies, rate); "8" stands for 8 or more. Only counts seen in at least 2% of games.
    pub points: Vec<(String, Rate)>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Pairs {
    pub cards: Vec<String>,
    /// `cells[r][c]`: games where both are owned at the end (`None` under 2% of games). The diagonal is the card alone.
    pub cells: Vec<Vec<Option<Rate>>>,
}

/// One leaf of a decision tree: the conditions that lead to it and the games there.
#[derive(Serialize, Debug, Clone)]
pub struct Leaf {
    pub path: Vec<String>,
    pub rate: Rate,
}

#[derive(Serialize, Debug, Clone)]
pub struct TreeAt {
    /// The own turn at whose start the positions were taken.
    pub turn: u16,
    pub rate: Rate,
    /// Best first.
    pub leaves: Vec<Leaf>,
}

/// A turning point: games where `who` gained `card` between two recorded turns moved the win estimate by `effect` more than the others.
#[derive(Serialize, Debug, Clone)]
pub struct Swing {
    /// e.g. "turns 5-8" (own turns), or "turn 17 to the end".
    pub window: String,
    /// "you" (this strategy) or "opponent".
    pub who: String,
    pub card: String,
    pub games: usize,
    pub effect: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct Side {
    pub name: String,
    pub rate: Rate,
    pub avg_vp: f64,
    pub avg_turns: f64,
    pub splits: Vec<SplitRow>,
    pub gains: Vec<HeatRow>,
    pub trashes: Vec<HeatRow>,
    pub by_turn_10: Vec<CountCurve>,
    pub at_end: Vec<CountCurve>,
    pub pairs: Pairs,
    /// Average differently named cards owned at the end, in wins and in losses (Museum).
    pub names_in_wins: f64,
    pub names_in_losses: f64,
    pub trees: Vec<TreeAt>,
    pub swings: Vec<Swing>,
    /// Trash for gain by own turn, like `gains`: one row per kind ("Remodel: Gold -> Province", with an arrow), the most frequent first.
    pub exchanges: Vec<HeatRow>,
}

/// A race for a card: who gained it first, and how often that player won.
#[derive(Serialize, Debug, Clone)]
pub struct Race {
    pub card: String,
    /// Games where the first to gain it (either strategy) won.
    pub first_wins: Rate,
    /// The same, over the games where both gained it.
    pub contested: Rate,
    /// Per strategy: the games it gained the card first in, and its win rate there.
    pub first_by: Vec<Rate>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ProphecyStats {
    pub happened: usize,
    /// Per strategy: the games it made the Prophecy happen in (its win rate there), and on which of its own turns on average.
    pub made_by: Vec<(Rate, f64)>,
}

/// What one rule of a strategy is worth: the same deals played without it.
#[derive(Serialize, Debug, Clone)]
pub struct RuleImpact {
    /// "gain", "play", "trash", "mode", "token", "call" or "debt".
    pub list: String,
    /// The rule as written, e.g. "Castles if my_turn >= 3".
    pub rule: String,
    /// Win rate without the rule.
    pub without: f64,
    /// Win rate with it minus without it.
    pub delta: f64,
    /// For gain and trash rules: copies of the rule's card gained (trashed) per game with the rule minus without, on each own turn.
    pub per_turn: Vec<f64>,
    /// The own turns where the rule's card is gained (trashed) differently, e.g. "turns 3-9" (empty for other rules).
    pub when: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Explanation {
    /// The strategy's win rate as the rules read it (the file round-tripped through the template language).
    pub with: f64,
    pub games: u64,
    pub rules: Vec<RuleImpact>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Insight {
    pub kingdom: Vec<String>,
    pub games: usize,
    pub turns: usize,
    pub sides: Vec<Side>,
    /// Cards someone gained in at least 5% of games, the first-gainer's best races first.
    pub races: Vec<Race>,
    pub prophecy: Option<ProphecyStats>,
}

/// Play `games` games between `a` and `b` on `kingdom` and report.
pub fn analyze(kingdom: &[CardId], a: &Strategy, b: &Strategy, games: u64, seed: u64) -> Insight {
    report(kingdom, &play_many(kingdom, [a, b], games.max(2), seed), [&a.name, &b.name])
}

/// The report over games already played, side 0 against side 1.
fn report(kingdom: &[CardId], all: &[Game], names: [&str; 2]) -> Insight {
    let n = all.len();
    let sides = (0..2).map(|i| side(all, i, names[i])).collect();
    let made: Vec<&Game> = all.iter().filter(|g| g.prophecy.is_some()).collect();
    let prophecy = (!made.is_empty()).then(|| ProphecyStats {
        happened: made.len(),
        made_by: (0..2)
            .map(|i| {
                let mine: Vec<&&Game> = made.iter().filter(|g| g.prophecy.unwrap().0 == i).collect();
                let t = mine.iter().map(|g| g.prophecy.unwrap().1 as f64).sum::<f64>() / mine.len().max(1) as f64;
                (rate(mine.iter().map(|g| g.win[i])), t)
            })
            .collect(),
    });
    Insight { kingdom: kingdom.iter().map(|&c| cards::name(c).to_string()).collect(), games: n, turns: TURNS, sides, races: races(kingdom, all), prophecy }
}

/// The game's top Victory card: Colony with Colonies, else Province. Races and findings leave it out: buying one mostly says
/// who is ahead, not why.
fn top_victory(kingdom: &[CardId]) -> CardId {
    if kingdom.contains(&id::COLONY) { id::COLONY } else { id::PROVINCE }
}

/// The races, without the game's top Victory card.
fn races(kingdom: &[CardId], all: &[Game]) -> Vec<Race> {
    let top = top_victory(kingdom);
    let mut cards: Vec<CardId> = all.iter().flat_map(|g| g.first.iter().map(|&(c, _)| c)).filter(|&c| c != top).collect();
    cards.sort_unstable();
    cards.dedup();
    let mut out: Vec<Race> = cards
        .into_iter()
        .filter_map(|c| {
            // (first gainer, did the other gain it too) in the games where someone gained it.
            let won: Vec<(usize, bool, &Game)> = all
                .iter()
                .filter_map(|g| g.first.iter().find(|&&(x, _)| x == c).map(|&(_, p)| (p, g.gains[1 - p].iter().any(|&(_, _, x)| x == c), g)))
                .collect();
            (won.len() * 20 >= all.len()).then(|| Race {
                card: cards::name(c).to_string(),
                first_wins: rate(won.iter().map(|&(p, _, g)| g.win[p])),
                contested: rate(won.iter().filter(|w| w.1).map(|&(p, _, g)| g.win[p])),
                first_by: (0..2).map(|i| rate(won.iter().filter(|w| w.0 == i).map(|&(p, _, g)| g.win[p]))).collect(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.first_wins.win.total_cmp(&a.first_wins.win));
    out
}

fn side(all: &[Game], i: usize, name: &str) -> Side {
    let n = all.len();
    let (trees, swings) = trees_and_swings(all, i);
    Side {
        name: name.to_string(),
        rate: rate(all.iter().map(|g| g.win[i])),
        avg_vp: all.iter().map(|g| g.vp[i] as f64).sum::<f64>() / n as f64,
        avg_turns: all.iter().map(|g| g.turns[i] as f64).sum::<f64>() / n as f64,
        splits: splits(all, i),
        gains: heat(all, i, |g| &g.gains[i]),
        trashes: heat(all, i, |g| &g.trashes[i]),
        by_turn_10: curves(all, i, true),
        at_end: curves(all, i, false),
        pairs: pairs(all, i),
        names_in_wins: avg_names(all, i, true),
        names_in_losses: avg_names(all, i, false),
        trees,
        swings,
        exchanges: exchanges(all, i),
    }
}

fn exchanges(all: &[Game], i: usize) -> Vec<HeatRow> {
    let mut kinds: Vec<(CardId, CardId, CardId)> = all.iter().flat_map(|g| g.exchanges[i].iter().map(|&(_, a, b, c)| (a, b, c))).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let n = all.len().max(1) as f64;
    let mut rows: Vec<HeatRow> = kinds
        .into_iter()
        .filter_map(|k| {
            let mut per = vec![0.0; TURNS];
            let mut total = 0usize;
            let (mut early, mut late) = (Vec::new(), Vec::new());
            for g in all {
                let mut e = false;
                for &(t, a, b, c) in &g.exchanges[i] {
                    if (a, b, c) == k {
                        total += 1;
                        if (1..=TURNS as u16).contains(&t) {
                            per[t as usize - 1] += 1.0;
                        }
                        e |= t < 10;
                    }
                }
                if e { early.push(g.win[i]) } else { late.push(g.win[i]) }
            }
            let total = total as f64 / n;
            (total >= 0.02).then(|| HeatRow {
                card: format!("{}: {} \u{2192} {}", cards::name(k.0), cards::name(k.1), cards::name(k.2)),
                per_turn: per.into_iter().map(|x| x / n).collect(),
                total,
                early: rate(early.into_iter()),
                late: rate(late.into_iter()),
            })
        })
        .collect();
    rows.sort_by(|a, b| b.total.total_cmp(&a.total));
    rows
}

fn splits(all: &[Game], i: usize) -> Vec<SplitRow> {
    let label = |s: (u8, u8)| format!("{}/{}", s.0, s.1);
    // Every split that happened (5/2 and 4/3 with the usual seven Coppers; others with Heirlooms or a changed deck), the most common first.
    let mut seen: Vec<((u8, u8), usize)> = Vec::new();
    for g in all {
        match seen.iter_mut().find(|(s, _)| *s == g.split[i]) {
            Some((_, n)) => *n += 1,
            None => seen.push((g.split[i], 1)),
        }
    }
    seen.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    let order: Vec<(u8, u8)> = seen.iter().map(|&(s, _)| s).collect();
    // One row over some games of a split: its rate, by the opponent's split, and every opening.
    let row = |gs: &[&Game], name: String| {
        let vs = order.iter().copied().map(|os| (label(os), rate(gs.iter().filter(|g| g.split[1 - i] == os).map(|g| g.win[i])))).collect();
        let mut open: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for g in gs {
            let mut names: Vec<&str> = g.opened[i].iter().map(|&c| cards::name(c)).collect();
            names.sort();
            open.entry(if names.is_empty() { "nothing".into() } else { names.join(" + ") }).or_default().push(g.win[i]);
        }
        let mut openings: Vec<Opening> = open.into_iter().map(|(cards, w)| Opening { cards, rate: rate(w.into_iter()) }).collect();
        openings.sort_by(|a, b| b.rate.games.cmp(&a.rate.games));
        SplitRow { split: name, rate: rate(gs.iter().map(|g| g.win[i])), vs, openings }
    };
    let mut out = Vec::new();
    for &split in &order {
        let gs: Vec<&Game> = all.iter().filter(|g| g.split[i] == split).collect();
        // With Shelters (or Heirlooms) the hands also differ in their other cards (Estates, or which Shelter came with which hand):
        // a row each, but only when that changes the win rate (the best and the worst apart as `order_matters` judges).
        let mut kinds: Vec<&(Vec<CardId>, Vec<CardId>)> = gs.iter().map(|g| &g.extras[i]).collect();
        kinds.sort();
        kinds.dedup();
        if kinds.len() >= 2 {
            let by: Vec<(&(Vec<CardId>, Vec<CardId>), Vec<&Game>)> = kinds.iter().map(|&k| (k, gs.iter().copied().filter(|g| &g.extras[i] == k).collect())).collect();
            let rates: Vec<Rate> = by.iter().map(|(_, g)| rate(g.iter().map(|g| g.win[i]))).collect();
            let big: Vec<&Rate> = rates.iter().filter(|r| r.games >= 100).collect();
            let best = big.iter().max_by(|a, b| a.win.total_cmp(&b.win));
            let worst = big.iter().min_by(|a, b| a.win.total_cmp(&b.win));
            if let (Some(b), Some(w)) = (best, worst) {
                if order_matters(b, w) {
                    // "4/3: Necropolis | Hovel + Overgrown Estate": the other cards of the $4 hand, then of the $3 hand.
                    let names = |h: &[CardId]| {
                        let mut parts: Vec<String> = Vec::new();
                        let mut k = 0;
                        while k < h.len() {
                            let n = h[k..].iter().take_while(|&&c| c == h[k]).count();
                            parts.push(if n > 1 { format!("{n} {}", cards::name(h[k])) } else { cards::name(h[k]).to_string() });
                            k += n;
                        }
                        if parts.is_empty() { "-".to_string() } else { parts.join(" + ") }
                    };
                    for (k, g) in &by {
                        out.push(row(g, format!("{}: {} | {}", label(split), names(&k.0), names(&k.1))));
                    }
                    continue;
                }
            }
        }
        // 4/3 and 3/4 (5/2 and 2/5) are one split unless the order changes the win rate: then a row each ("4/3, $4 first",
        // "3/4, $3 first").
        if split.0 != split.1 {
            let side_of = |first: (u8, u8)| gs.iter().copied().filter(|g| g.split_ordered[i] == first).collect::<Vec<&Game>>();
            let (hi, lo) = (side_of(split), side_of((split.1, split.0)));
            if order_matters(&rate(hi.iter().map(|g| g.win[i])), &rate(lo.iter().map(|g| g.win[i]))) {
                out.push(row(&hi, format!("{}/{}, ${} first", split.0, split.1, split.0)));
                out.push(row(&lo, format!("{}/{}, ${} first", split.1, split.0, split.1)));
                continue;
            }
        }
        out.push(row(&gs, label(split)));
    }
    out
}

/// Whether the order of an opening's hands changes the win rate: both orders seen 100 times or more, and their rates three
/// standard errors and 3 points apart.
fn order_matters(a: &Rate, b: &Rate) -> bool {
    if a.games < 100 || b.games < 100 {
        return false;
    }
    let p = (a.win * a.games as f64 + b.win * b.games as f64) / (a.games + b.games) as f64;
    let se = (p * (1.0 - p) * (1.0 / a.games as f64 + 1.0 / b.games as f64)).sqrt();
    let d = (a.win - b.win).abs();
    d >= 0.03 && d > 3.0 * se
}

fn heat<'a>(all: &'a [Game], i: usize, list: impl Fn(&'a Game) -> &'a Vec<(u16, u16, CardId)>) -> Vec<HeatRow> {
    let n = all.len() as f64;
    let mut ids: Vec<CardId> = all.iter().flat_map(|g| list(g).iter().map(|&(_, _, c)| c)).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut rows: Vec<HeatRow> = ids
        .into_iter()
        .filter_map(|c| {
            let mut per = vec![0.0; TURNS];
            let mut total = 0usize;
            let (mut early, mut late) = (Vec::new(), Vec::new());
            for g in all {
                let mut e = false;
                for &(t, _, x) in list(g) {
                    if x == c {
                        total += 1;
                        if (1..=TURNS as u16).contains(&t) {
                            per[t as usize - 1] += 1.0;
                        }
                        e |= t < 10;
                    }
                }
                if e { early.push(g.win[i]) } else { late.push(g.win[i]) }
            }
            let total = total as f64 / n;
            (total >= 0.05).then(|| HeatRow {
                card: cards::name(c).to_string(),
                per_turn: per.into_iter().map(|x| x / n).collect(),
                total,
                early: rate(early.into_iter()),
                late: rate(late.into_iter()),
            })
        })
        .collect();
    rows.sort_by(|a, b| b.total.total_cmp(&a.total));
    rows
}

fn curves(all: &[Game], i: usize, by10: bool) -> Vec<CountCurve> {
    let n = all.len();
    let mut ids: Vec<CardId> = all.iter().flat_map(|g| g.gains[i].iter().map(|&(_, _, c)| c)).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .filter_map(|c| {
            let count = |g: &Game| -> usize {
                if by10 {
                    g.gains[i].iter().filter(|&&(t, _, x)| x == c && t <= 10).count()
                } else {
                    g.deck[i].iter().find(|&&(x, _)| x == c).map_or(0, |&(_, k)| k as usize)
                }
            };
            let points: Vec<(String, Rate)> = (0..=8)
                .filter_map(|k| {
                    let r = rate(all.iter().filter(|g| if k == 8 { count(g) >= 8 } else { count(g) == k }).map(|g| g.win[i]));
                    (r.games * 50 >= n).then(|| (if k == 8 { "8+".to_string() } else { k.to_string() }, r))
                })
                .collect();
            (points.len() >= 2).then(|| CountCurve { card: cards::name(c).to_string(), points })
        })
        .collect()
}

fn owns(g: &Game, i: usize, c: CardId) -> bool {
    g.deck[i].iter().any(|&(x, _)| x == c)
}

fn pairs(all: &[Game], i: usize) -> Pairs {
    let n = all.len();
    let mut ids: Vec<CardId> = all.iter().flat_map(|g| g.deck[i].iter().map(|&(c, _)| c)).filter(|&c| c != id::COPPER && c != id::ESTATE && c != id::CURSE).collect();
    ids.sort_unstable();
    ids.dedup();
    // Cards owned in 5% to 95% of games: one owned in (nearly) every game tells nothing.
    ids.retain(|&c| {
        let k = all.iter().filter(|g| owns(g, i, c)).count();
        k * 20 >= n && k * 20 <= n * 19
    });
    let cells = ids
        .iter()
        .map(|&r| {
            ids.iter()
                .map(|&c| {
                    let x = rate(all.iter().filter(|g| owns(g, i, r) && owns(g, i, c)).map(|g| g.win[i]));
                    (x.games * 50 >= n).then_some(x)
                })
                .collect()
        })
        .collect();
    Pairs { cards: ids.iter().map(|&c| cards::name(c).to_string()).collect(), cells }
}

fn avg_names(all: &[Game], i: usize, wins: bool) -> f64 {
    let gs: Vec<&Game> = all.iter().filter(|g| if wins { g.win[i] > 0.5 } else { g.win[i] < 0.5 }).collect();
    gs.iter().map(|g| g.deck[i].len() as f64).sum::<f64>() / gs.len().max(1) as f64
}

// ---------------------------------------------------------------------------------------------
// Decision trees and turning points
// ---------------------------------------------------------------------------------------------

enum Node {
    Leaf(f64),
    Split { f: usize, t: i32, lo: Box<Node>, hi: Box<Node> },
}

impl Node {
    fn predict(&self, x: &[i32]) -> f64 {
        match self {
            Node::Leaf(p) => *p,
            Node::Split { f, t, lo, hi } => if x[*f] < *t { lo.predict(x) } else { hi.predict(x) },
        }
    }

    fn leaves(&self, names: &[String], path: &mut Vec<String>, xs: &[(Vec<i32>, f64)], out: &mut Vec<Leaf>) {
        match self {
            Node::Leaf(_) => out.push(Leaf { path: tightest(path), rate: rate(xs.iter().map(|s| s.1)) }),
            Node::Split { f, t, lo, hi } => {
                let (a, b): (Vec<_>, Vec<_>) = xs.iter().cloned().partition(|s| s.0[*f] < *t);
                path.push(format!("{} < {t}", names[*f]));
                lo.leaves(names, path, &a, out);
                path.pop();
                path.push(format!("{} >= {t}", names[*f]));
                hi.leaves(names, path, &b, out);
                path.pop();
            }
        }
    }
}

/// A path's conditions with one bound per side for each variable (`vp_lead >= -1 and vp_lead >= 5` is `vp_lead >= 5`), in first-use order.
fn tightest(path: &[String]) -> Vec<String> {
    // (name, lower bound from `>=`, upper bound from `<`)
    let mut vars: Vec<(String, Option<i32>, Option<i32>)> = Vec::new();
    for p in path {
        let (name, ge, n) = match p.rsplit_once(" >= ") {
            Some((a, b)) => (a, true, b),
            None => {
                let (a, b) = p.rsplit_once(" < ").expect("a tree condition");
                (a, false, b)
            }
        };
        let n: i32 = n.parse().expect("a tree threshold");
        let k = match vars.iter().position(|v| v.0 == name) {
            Some(k) => k,
            None => {
                vars.push((name.to_string(), None, None));
                vars.len() - 1
            }
        };
        if ge {
            vars[k].1 = Some(vars[k].1.map_or(n, |m| m.max(n)));
        } else {
            vars[k].2 = Some(vars[k].2.map_or(n, |m| m.min(n)));
        }
    }
    let mut out = Vec::new();
    for (name, lo, hi) in vars {
        if let Some(lo) = lo {
            out.push(format!("{name} >= {lo}"));
        }
        if let Some(hi) = hi {
            out.push(format!("{name} < {hi}"));
        }
    }
    out
}

/// A regression tree on the win (0, ½, 1): splits `feature >= t` that reduce the squared error most, `depth` levels, at least `min_leaf`
/// samples a side.
fn fit(xs: &[(Vec<i32>, f64)], depth: usize, min_leaf: usize) -> Node {
    let n = xs.len();
    let sum: f64 = xs.iter().map(|s| s.1).sum();
    let mean = if n > 0 { sum / n as f64 } else { 0.0 };
    if depth == 0 || n < 2 * min_leaf {
        return Node::Leaf(mean);
    }
    let base = sum * sum / n as f64;
    let nf = xs[0].0.len();
    let mut best: Option<(f64, usize, i32)> = None;
    let mut order: Vec<usize> = (0..n).collect();
    for f in 0..nf {
        order.sort_unstable_by_key(|&k| xs[k].0[f]);
        let (mut cnt, mut s) = (0usize, 0.0);
        for w in 0..n - 1 {
            let k = order[w];
            cnt += 1;
            s += xs[k].1;
            let (v, next) = (xs[k].0[f], xs[order[w + 1]].0[f]);
            if v == next || cnt < min_leaf || n - cnt < min_leaf {
                continue;
            }
            let gain = s * s / cnt as f64 + (sum - s) * (sum - s) / (n - cnt) as f64 - base;
            if best.map_or(true, |b| gain > b.0) {
                best = Some((gain, f, next));
            }
        }
    }
    match best {
        // A split must explain something: at least 0.1% of the variance budget of these samples.
        Some((gain, f, t)) if gain > 0.001 * n as f64 => {
            let (a, b): (Vec<_>, Vec<_>) = xs.iter().cloned().partition(|s| s.0[f] < t);
            Node::Split { f, t, lo: Box::new(fit(&a, depth - 1, min_leaf)), hi: Box::new(fit(&b, depth - 1, min_leaf)) }
        }
        _ => Node::Leaf(mean),
    }
}

/// The feature names (in the strategy language where there is a word for it) and the cards behind the `count(..)` features.
fn features(all: &[Game], i: usize) -> (Vec<String>, Vec<CardId>) {
    let mut seen: BTreeMap<CardId, usize> = BTreeMap::new();
    let mut total = 0;
    for g in all {
        for s in &g.snaps[i] {
            total += 1;
            for &(c, _) in s.mine.iter().chain(&s.theirs) {
                *seen.entry(c).or_default() += 1;
            }
        }
    }
    let ids: Vec<CardId> = seen.into_iter().filter(|&(_, k)| k * 20 >= total).map(|(c, _)| c).collect();
    let mut names: Vec<String> = BASE.iter().map(|s| s.to_string()).collect();
    names.push("count(Castles)".into());
    names.push("opp count(Castles)".into());
    for &c in &ids {
        names.push(format!("count({})", cards::name(c)));
    }
    for &c in &ids {
        names.push(format!("opp count({})", cards::name(c)));
    }
    (names, ids)
}

fn vector(s: &Snap, ids: &[CardId]) -> Vec<i32> {
    let get = |v: &[(CardId, u16)], c: CardId| v.iter().find(|&&(x, _)| x == c).map_or(0, |&(_, n)| n as i32);
    let castles = |v: &[(CardId, u16)]| v.iter().filter(|&&(c, _)| cards::is(c, cards::CASTLE)).map(|&(_, n)| n as i32).sum::<i32>();
    let mut x: Vec<i32> = s.base.to_vec();
    x.push(castles(&s.mine));
    x.push(castles(&s.theirs));
    x.extend(ids.iter().map(|&c| get(&s.mine, c)));
    x.extend(ids.iter().map(|&c| get(&s.theirs, c)));
    x
}

fn trees_and_swings(all: &[Game], i: usize) -> (Vec<TreeAt>, Vec<Swing>) {
    let (names, ids) = features(all, i);
    let mut trees = Vec::new();
    let mut models: Vec<Option<Node>> = Vec::new();
    for (at, &turn) in SNAP_TURNS.iter().enumerate() {
        let xs: Vec<(Vec<i32>, f64)> = all.iter().filter_map(|g| g.snaps[i].iter().find(|s| s.at == at).map(|s| (vector(s, &ids), g.win[i]))).collect();
        // Too few games still going (or too few games) for a tree worth reading.
        if xs.len() < 200 || xs.len() * 5 < all.len() {
            models.push(None);
            continue;
        }
        let min_leaf = (xs.len() / 40).max(50);
        let tree = fit(&xs, 3, min_leaf);
        let mut leaves = Vec::new();
        tree.leaves(&names, &mut Vec::new(), &xs, &mut leaves);
        leaves.sort_by(|a, b| b.rate.win.total_cmp(&a.rate.win));
        trees.push(TreeAt { turn, rate: rate(xs.iter().map(|s| s.1)), leaves });
        models.push(Some(tree));
    }

    // Turning points: per window between recorded turns (the last one to the end of the game), the change of the estimate, with and
    // without a card gained in the window, by this strategy and by the opponent.
    let mut swings = Vec::new();
    for (k, model) in models.iter().enumerate() {
        let Some(model) = model else { continue };
        let next = models.get(k + 1).and_then(|m| m.as_ref());
        let window = if next.is_some() { format!("turns {}-{}", SNAP_TURNS[k], SNAP_TURNS[k + 1] - 1) } else { format!("turn {} to the end", SNAP_TURNS[k]) };
        // (delta, my gains in the window, the opponent's)
        let mut rows: Vec<(f64, Vec<CardId>, Vec<CardId>)> = Vec::new();
        for g in all {
            let Some(s) = g.snaps[i].iter().find(|s| s.at == k) else { continue };
            let p0 = model.predict(&vector(s, &ids));
            let (p1, end) = match (next, g.snaps[i].iter().find(|t| t.at == k + 1)) {
                (Some(m), Some(t)) => (m.predict(&vector(t, &ids)), t.global),
                _ => (g.win[i], u16::MAX),
            };
            let pick = |v: &[(u16, u16, CardId)]| v.iter().filter(|&&(_, gl, _)| gl >= s.global && gl < end).map(|&(_, _, c)| c).collect::<Vec<_>>();
            rows.push((p1 - p0, pick(&g.gains[i]), pick(&g.gains[1 - i])));
        }
        if rows.len() < 200 {
            continue;
        }
        let total: f64 = rows.iter().map(|r| r.0).sum();
        for (who, mine) in [("you", true), ("opponent", false)] {
            let mut ids_w: Vec<CardId> = rows.iter().flat_map(|r| if mine { r.1.iter() } else { r.2.iter() }).copied().collect();
            ids_w.sort_unstable();
            ids_w.dedup();
            for c in ids_w {
                let (mut k_with, mut s_with) = (0usize, 0.0);
                for r in &rows {
                    if (if mine { &r.1 } else { &r.2 }).contains(&c) {
                        k_with += 1;
                        s_with += r.0;
                    }
                }
                let k_without = rows.len() - k_with;
                if k_with * 50 < rows.len() || k_without * 50 < rows.len() {
                    continue;
                }
                let effect = s_with / k_with as f64 - (total - s_with) / k_without as f64;
                swings.push(Swing { window: window.clone(), who: who.to_string(), card: cards::name(c).to_string(), games: k_with, effect });
            }
        }
    }
    swings.sort_by(|a, b| b.effect.abs().total_cmp(&a.effect.abs()));
    swings.truncate(20);
    (trees, swings)
}

// ---------------------------------------------------------------------------------------------
// Rule explanations
// ---------------------------------------------------------------------------------------------

fn list_name(l: List) -> &'static str {
    match l {
        List::Gain => "gain",
        List::Play => "play",
        List::Trash => "trash",
        List::Mode => "mode",
        List::Token => "token",
        List::Call => "call",
        List::Debt => "debt",
    }
}

/// Whether a rule for `rule` covers the card `c`: the card itself, or a card of the mixed pile it names (`Castles`, `Knights`, `Ruins`).
fn covers(rule: CardId, c: CardId) -> bool {
    c == rule
        || (rule == id::CASTLES && cards::is(c, cards::CASTLE))
        || dominion_engine::sets::dark_ages::MixedPile::of_marker(rule).is_some_and(|p| p.members().any(|m| m == c))
}

fn per_turn_of(games: &[Game], card: CardId, trash: bool) -> Vec<f64> {
    let mut per = vec![0.0; TURNS];
    for g in games {
        for &(t, _, c) in if trash { &g.trashes[0] } else { &g.gains[0] } {
            if covers(card, c) && (1..=TURNS as u16).contains(&t) {
                per[t as usize - 1] += 1.0;
            }
        }
    }
    per.into_iter().map(|x| x / games.len().max(1) as f64).collect()
}

/// Take each rule of `a_toml` out in turn and replay the same deals against `b`: what each rule is worth, and on which turns it changes
/// what is gained (trashed). Only for files in the template language the search writes (`Genome::from_toml`).
pub fn explain(kingdom: &[CardId], a_toml: &str, b: &Strategy, games: u64, seed: u64, fast: bool) -> Result<Explanation, String> {
    explain_field(kingdom, a_toml, &[b], games, seed, fast)
}

/// [`explain`] against a field: about `games` games for each version of the strategy, spread evenly over the field's strategies.
pub fn explain_field(kingdom: &[CardId], a_toml: &str, field: &[&Strategy], games: u64, seed: u64, fast: bool) -> Result<Explanation, String> {
    let g = Genome::from_toml(a_toml).map_err(|e| format!("only strategies in the search's rule language can be explained rule by rule: {e}"))?;
    let name = Strategy::parse(a_toml)?.name;
    let compile = |g: &Genome| -> Result<Strategy, String> {
        let mut s = Strategy::parse(&g.to_toml(&TomlMeta { name: name.clone(), ..Default::default() }))?;
        if fast {
            make_fast(&mut s);
        }
        Ok(s)
    };
    let base_s = compile(&g)?;
    let base = play_field(kingdom, &base_s, field, games, seed);
    let base_rate = rate(base.iter().map(|x| x.win[0])).win;
    let mut rules = Vec::new();
    for (list, rs) in g.lists() {
        for (k, r) in rs.iter().enumerate() {
            let mut v = g.clone();
            v.list_mut(list).remove(k);
            let s = compile(&v)?;
            let res = play_field(kingdom, &s, field, games, seed);
            let without = rate(res.iter().map(|x| x.win[0])).win;
            let (per_turn, when) = if matches!(list, List::Gain | List::Trash) {
                let trash = list == List::Trash;
                let a = per_turn_of(&base, r.card, trash);
                let w = per_turn_of(&res, r.card, trash);
                let d: Vec<f64> = a.iter().zip(&w).map(|(x, y)| x - y).collect();
                let top = d.iter().fold(0.0f64, |m, x| m.max(x.abs()));
                let on: Vec<usize> = (0..TURNS).filter(|&t| top > 0.01 && d[t].abs() >= 0.25 * top).collect();
                let when = match (on.first(), on.last()) {
                    (Some(&x), Some(&y)) if x == y => format!("turn {}", x + 1),
                    (Some(&x), Some(&y)) => format!("turns {}-{}", x + 1, y + 1),
                    _ => String::new(),
                };
                (d, when)
            } else {
                (Vec::new(), String::new())
            };
            let what = match list {
                List::Debt => format!("pay {}", crate::genome::DEBT_PAY[r.opt as usize]),
                _ => cards::name(r.card).to_string(),
            };
            let rule = match r.cond_text() {
                Some(c) => format!("{what} if {c}"),
                None => what,
            };
            rules.push(RuleImpact { list: list_name(list).to_string(), rule, without, delta: base_rate - without, per_turn, when });
        }
    }
    Ok(Explanation { with: base_rate, games: base.len() as u64, rules })
}

/// The strategy file `toml` with what each rule is worth written as a comment above it ("# +12.5 points against the field:
/// 71.2% without it, acts on turns 3-9"). `e` must explain this very file (its rules in file order); other lines are kept.
pub fn annotate(toml: &str, e: &Explanation) -> String {
    let mut seen: Vec<(&str, usize)> = Vec::new();
    let mut out = String::with_capacity(toml.len() + 64 * e.rules.len());
    for line in toml.lines() {
        let table = line.trim().strip_prefix("[[").and_then(|x| x.strip_suffix("]]"));
        if let Some(t) = table {
            let k = match seen.iter_mut().find(|(n, _)| *n == t) {
                Some((_, k)) => {
                    *k += 1;
                    *k - 1
                }
                None => {
                    seen.push((t, 1));
                    0
                }
            };
            if let Some(r) = e.rules.iter().filter(|r| r.list == t).nth(k) {
                let when = if r.when.is_empty() { String::new() } else { format!(", acts on {}", r.when) };
                out.push_str(&format!("# {:+.1} points against the field: {} without it{when}\n", r.delta * 100.0, pc(r.without)));
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strat(src: &str) -> Strategy {
        let mut s = Strategy::parse(src).unwrap();
        make_fast(&mut s);
        s
    }

    #[test]
    fn an_opening_is_what_a_player_gained_on_its_own_first_two_turns_not_a_curse_given_meanwhile() {
        let kingdom = dominion_sim::kingdom::resolve("Witch,Village,Smithy,Market,Militia,Moat,Cellar,Chapel,Festival,Laboratory", &[], 1).unwrap();
        let witch = strat("name = \"Witch\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Witch\"\nif = \"count(Witch) < 2\"\n[[gain]]\ncard = \"Silver\"\n[[play]]\ncard = \"Witch\"\n");
        let money = strat("name = \"Money\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n");
        let r = analyze(&kingdom, &money, &witch, 4000, 7);
        // A 5/2 Witch is played on turn 3, before the second player's turn 3: that Curse is no part of its opening.
        let curses_by_turn_2: f64 = r.sides[0].gains.iter().find(|h| h.card == "Curse").map_or(0.0, |h| h.per_turn[..2].iter().sum());
        assert!(curses_by_turn_2 > 0.0, "the test needs an early Curse");
        for s in &r.sides {
            assert!(s.splits.iter().flat_map(|x| &x.openings).all(|o| !o.cards.contains("Curse")), "{:?}", s.splits);
        }
    }

    #[test]
    fn trash_for_gain_records_what_a_remodel_trashed_and_gained() {
        let kingdom = dominion_sim::kingdom::resolve("first-game", &[], 1).unwrap();
        let remodel = strat("name = \"Remodel\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Remodel\"\nif = \"count(Remodel) < 1\"\n[[gain]]\ncard = \"Silver\"\n[[play]]\ncard = \"Remodel\"\n");
        let money = strat("name = \"Money\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n");
        let r = analyze(&kingdom, &remodel, &money, 1000, 5);
        let x = &r.sides[0].exchanges;
        assert!(!x.is_empty() && x.iter().all(|e| e.card.starts_with("Remodel: ")), "{x:?}");
        assert!(x.iter().any(|e| e.card == "Remodel: Gold \u{2192} Province" || e.card.starts_with("Remodel: Estate")), "{x:?}");
        assert!(x.windows(2).all(|w| w[0].total >= w[1].total));
        // By own turn, like the gains: the per-turn figures add up to the total (games past the last turn column aside).
        assert!(x.iter().all(|e| e.per_turn.len() == TURNS && e.per_turn.iter().sum::<f64>() <= e.total + 1e-9));
        // Money trashes nothing, and buying is no trash-for-gain.
        assert!(r.sides[1].exchanges.is_empty(), "{:?}", r.sides[1].exchanges);
    }

    #[test]
    fn the_order_of_the_opening_hands_matters_only_when_the_win_rates_differ_clearly() {
        let r = |games, win| Rate { games, win };
        assert!(!order_matters(&r(4000, 0.60), &r(4000, 0.59)), "1 point: the same split");
        assert!(order_matters(&r(4000, 0.65), &r(4000, 0.55)), "10 points over 4000 games each");
        assert!(!order_matters(&r(60, 0.80), &r(4000, 0.55)), "too few games one way round");
    }

    #[test]
    fn a_report_covers_both_sides_and_its_trees_split_on_the_position() {
        let kingdom = dominion_sim::kingdom::resolve("first-game", &[], 1).unwrap();
        let smithy = strat("name = \"Smithy\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Smithy\"\nif = \"count(Smithy) < 1\"\n[[gain]]\ncard = \"Silver\"\n");
        let money = strat("name = \"Money\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n");
        let r = analyze(&kingdom, &smithy, &money, 2000, 3);
        assert_eq!((r.games, r.sides.len()), (2000, 2));
        let s = &r.sides[0];
        // Every game has one split; the two rows count them all.
        assert_eq!(s.splits.iter().map(|x| x.rate.games).sum::<usize>(), 2000);
        // Smithy-BM beats money and gains Provinces late, Silver early.
        assert!(s.rate.win > 0.55, "{:?}", s.rate);
        let prov = s.gains.iter().find(|h| h.card == "Province").unwrap();
        assert!(prov.per_turn[..4].iter().sum::<f64>() < 0.05 && prov.total > 2.0, "{prov:?}");
        // Trees from turn 9 on, with conditions in the strategy language, and win rates that differ between leaves.
        let t = s.trees.iter().find(|t| t.turn == 13).expect("a tree at turn 13");
        assert!(t.leaves.len() >= 2 && t.leaves[0].rate.win > t.leaves.last().unwrap().rate.win + 0.1, "{t:?}");
        assert!(t.leaves.iter().all(|l| l.path.iter().all(|p| p.contains(" < ") || p.contains(" >= "))));
        assert_eq!(tightest(&["vp_lead >= -1".into(), "money < 9".into(), "vp_lead >= 5".into(), "money < 7".into()]), vec!["vp_lead >= 5", "money < 7"]);
        assert_eq!(t.leaves.iter().map(|l| l.rate.games).sum::<usize>(), t.rate.games);
        // Races leave out the Province (the first to buy one is just ahead); Gold and Smithy are there.
        assert!(r.races.iter().all(|x| x.card != "Province") && r.races.iter().any(|x| x.card == "Gold"), "{:?}", r.races);
        // Gaining a Province is a turning point upward.
        assert!(s.swings.iter().any(|w| w.card == "Province" && w.who == "you" && w.effect > 0.0), "{:?}", s.swings);
    }

    #[test]
    fn explaining_rules_measures_each_one_on_the_same_deals() {
        let kingdom = dominion_sim::kingdom::resolve("first-game", &[], 1).unwrap();
        let money = strat("name = \"Money\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n");
        let a = "name = \"Smithy\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Smithy\"\nif = \"count(Smithy) < 1\"\n[[gain]]\ncard = \"Silver\"\n";
        let e = explain(&kingdom, a, &money, 2000, 3, true).unwrap();
        assert_eq!(e.rules.len(), 4);
        let smithy = e.rules.iter().find(|r| r.rule.starts_with("Smithy")).unwrap();
        assert_eq!(smithy.rule, "Smithy if count(Smithy) < 1");
        assert!(smithy.delta > 0.05, "the Smithy is what beats money: {smithy:?}");
        assert!(smithy.when.starts_with("turn"), "{smithy:?}");
        // Without the Province rule the strategy cannot win at all.
        let prov = e.rules.iter().find(|r| r.rule == "Province").unwrap();
        assert!(prov.without < 0.1, "{prov:?}");
        assert!(explain(&kingdom, "name = \"X\"\n[[gain]]\ncard = \"Gold\"\nif = \"coins >= 6 or buys > 1\"\n", &money, 10, 1, true).is_err());
    }

    #[test]
    fn findings_play_the_champion_against_the_whole_field() {
        let kingdom = dominion_sim::kingdom::resolve("first-game", &[], 1).unwrap();
        let money = "name = \"Money\"
[[gain]]
card = \"Province\"
[[gain]]
card = \"Gold\"
[[gain]]
card = \"Silver\"
";
        let silver = "name = \"Silver\"
[[gain]]
card = \"Province\"
[[gain]]
card = \"Silver\"
";
        let smithy = "name = \"Smithy\"
[[gain]]
card = \"Province\"
[[gain]]
card = \"Gold\"
[[gain]]
card = \"Smithy\"
if = \"count(Smithy) < 1\"
[[gain]]
card = \"Silver\"
";
        let field = [("BM".to_string(), money.to_string()), ("Silvers".to_string(), silver.to_string())];
        let f = findings(&kingdom, smithy, &field, 2000, 3).unwrap();
        assert_eq!(f.champion, "Smithy");
        assert!(f.explanation.is_some(), "{:?}", f.explain_error);
        // The same number of games against each, the hardest matchup first.
        assert_eq!(f.insight.games, 2000);
        assert_eq!(f.field.iter().map(|r| (r.name.as_str(), r.rate.games)).collect::<Vec<_>>(), [("BM", 1000), ("Silvers", 1000)]);
        assert!(f.field[0].rate.win < f.field[1].rate.win, "{:?}", f.field);
        // What each rule is worth goes into the file as a comment above it (the Smithy rule among the valuable ones), not into the findings.
        assert!(f.findings.iter().all(|x| x.kind != "rule"), "{:#?}", f.findings);
        let noted = annotate(smithy, f.explanation.as_ref().unwrap());
        let smithy_note = noted.split("card = \"Smithy\"").next().unwrap().lines().filter(|l| l.starts_with("# ")).last().unwrap().to_string();
        assert!(smithy_note.starts_with("# +") && smithy_note.contains(" points against the field: "), "{noted}");
        assert_eq!(noted.lines().filter(|l| l.starts_with("[[")).count(), noted.lines().filter(|l| l.contains(" points against the field: ")).count(), "{noted}");
        Strategy::parse(&noted).unwrap();
        // The power map: the owner's win rate by copies at the end, both sides of every game, Province included.
        let row = |c: &str| f.power.iter().find(|r| r.card == c).unwrap_or_else(|| panic!("{c}: {:#?}", f.power));
        let smithy_row = row("Smithy");
        assert_eq!(smithy_row.by_count.len(), 10);
        assert_eq!(smithy_row.by_count.iter().map(|r| r.games).sum::<usize>(), 2 * f.insight.games);
        assert!(smithy_row.by_count[1].win > smithy_row.by_count[0].win, "{smithy_row:?}");
        assert!(row("Province").swing > 0.3);
        assert!(f.power.windows(2).all(|w| w[0].swing >= w[1].swing));
        // Province (the top Victory card here) is in no finding: buying one mostly says who is ahead.
        assert!(f.findings.iter().all(|x| !x.text.contains("Province")), "{:#?}", f.findings);
        // With no field the champion plays itself.
        let alone = findings(&kingdom, smithy, &[], 1000, 3).unwrap();
        assert!(alone.field.len() == 1 && alone.findings.iter().all(|x| x.kind != "difference" && x.kind != "opening"));
    }
}

// ---------------------------------------------------------------------------------------------
// Kingdom findings
// ---------------------------------------------------------------------------------------------

/// One sentence about the kingdom, with how much it moves the win rate (for ordering) and the games behind it.
#[derive(Serialize, Debug, Clone)]
pub struct Finding {
    /// "race", "position" or "turning point" (what each rule is worth goes into the file: [`annotate`]; what each card is
    /// worth is [`Findings::power`]; the openings are the champion's side of [`Findings::insight`]).
    pub kind: String,
    pub text: String,
    /// Win-rate points (0.1 = 10 points) of the champion: a rule's worth, how much a race or an opening changes its win rate, how
    /// far a position is from its average, the move of a turning point. For "difference": copies gained per game, the champion's
    /// minus the field's.
    pub points: f64,
    pub games: usize,
}

/// The champion against one strategy of the field.
#[derive(Serialize, Debug, Clone)]
pub struct FieldRow {
    pub name: String,
    /// The champion's win rate against it.
    pub rate: Rate,
    /// The opponent's average VP and own turns.
    pub avg_vp: f64,
    pub avg_turns: f64,
}

/// What a league found on its kingdom: its champion against the whole field of the final ranking (every other strategy, the
/// baselines included, the same number of games against each): what decides its games, what it does that the field does not, and
/// what each of its rules is worth.
#[derive(Serialize, Debug, Clone)]
pub struct Findings {
    pub champion: String,
    /// What having a card is worth on this kingdom: for every card someone owned, the owner's win rate by how many copies they
    /// had at the end (both sides of every game), the biggest swing first. The top Victory card included.
    pub power: Vec<PowerRow>,
    /// Against each strategy of the field, the champion's worst matchup first.
    pub field: Vec<FieldRow>,
    /// By kind (in the order of [`Finding::kind`]'s list), strongest first within a kind.
    pub findings: Vec<Finding>,
    /// The champion (side 0) against the field (side 1, all its strategies together).
    pub insight: Insight,
    pub explanation: Option<Explanation>,
    pub explain_error: Option<String>,
}

/// A card's row of the power map: the win rate of a player who ended with 0, 1, ..., 8 and 9 or more copies.
#[derive(Serialize, Debug, Clone)]
pub struct PowerRow {
    pub card: String,
    pub by_count: Vec<Rate>,
    /// Highest minus lowest win rate over the counts with enough games (2% of the samples, at least 30).
    pub swing: f64,
}

/// The power map over games already played (see [`Findings::power`]).
fn power(all: &[Game]) -> Vec<PowerRow> {
    const COLS: usize = 10;
    let samples = 2 * all.len();
    let mut ids: Vec<CardId> = all.iter().flat_map(|g| g.deck.iter().flat_map(|d| d.iter().map(|&(c, _)| c))).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut rows: Vec<PowerRow> = ids
        .into_iter()
        .map(|c| {
            let mut cols: Vec<Vec<f64>> = vec![Vec::new(); COLS];
            for g in all {
                for i in 0..2 {
                    let n = g.deck[i].iter().find(|&&(x, _)| x == c).map_or(0, |&(_, n)| n as usize);
                    cols[n.min(COLS - 1)].push(g.win[i]);
                }
            }
            let by_count: Vec<Rate> = cols.into_iter().map(|w| rate(w.into_iter())).collect();
            let enough: Vec<f64> = by_count.iter().filter(|r| r.games >= 30 && r.games * 50 >= samples).map(|r| r.win).collect();
            let swing = if enough.len() >= 2 { enough.iter().cloned().fold(f64::MIN, f64::max) - enough.iter().cloned().fold(f64::MAX, f64::min) } else { 0.0 };
            PowerRow { card: cards::name(c).to_string(), by_count, swing }
        })
        .filter(|r| r.swing > 0.0)
        .collect();
    rows.sort_by(|a, b| b.swing.total_cmp(&a.swing));
    rows
}

fn pc(x: f64) -> String {
    format!("{:.0}%", 100.0 * x)
}

/// Analyze a league's champion (`champion`, a strategy file) on `kingdom` with fast bots against `field` (name, strategy file; an
/// empty field means the champion itself): about `games` games, and each of its rules taken out, `games / 2` games each.
pub fn findings(kingdom: &[CardId], champion: &str, field: &[(String, String)], games: u64, seed: u64) -> Result<Findings, String> {
    let fast = |src: &str| -> Result<Strategy, String> {
        let mut s = Strategy::parse(src)?;
        make_fast(&mut s);
        Ok(s)
    };
    let a = fast(champion)?;
    let mut names: Vec<String> = Vec::new();
    let mut opps: Vec<Strategy> = Vec::new();
    for (name, src) in field {
        names.push(name.clone());
        opps.push(fast(src)?);
    }
    if opps.is_empty() {
        names.push(a.name.clone());
        opps.push(a.clone());
    }
    let refs: Vec<&Strategy> = opps.iter().collect();
    // The top Victory card is left out of every finding (see `top_victory`).
    let top = cards::name(top_victory(kingdom));
    let count_top = format!("count({top})");
    let all = play_field(kingdom, &a, &refs, games, seed);
    let insight = report(kingdom, &all, [&a.name, "the field"]);
    let mut rows: Vec<FieldRow> = (0..opps.len())
        .map(|k| {
            let mine: Vec<&Game> = all.iter().filter(|g| g.opp == k).collect();
            let n = mine.len().max(1) as f64;
            FieldRow {
                name: names[k].clone(),
                rate: rate(mine.iter().map(|g| g.win[0])),
                avg_vp: mine.iter().map(|g| g.vp[1] as f64).sum::<f64>() / n,
                avg_turns: mine.iter().map(|g| g.turns[1] as f64).sum::<f64>() / n,
            }
        })
        .collect();
    rows.sort_by(|x, y| x.rate.win.total_cmp(&y.rate.win));
    let (explanation, explain_error) = match explain_field(kingdom, champion, &refs, (games / 2).max(500), seed, true) {
        Ok(e) => (Some(e), None),
        Err(e) => (None, Some(e)),
    };
    let mut out: Vec<Finding> = Vec::new();
    let mut push = |kind: &str, text: String, points: f64, games: usize| out.push(Finding { kind: kind.into(), text, points, games });

    // (What each rule is worth is not a finding: `annotate` writes it into the champion's file, above each rule.)

    // (No "difference" sentences: the card power map says what having each card is worth.)
    let me = &insight.sides[0];

    // Races: the champion's win rate when it gains a card first, and when its opponent does.
    let n = insight.games;
    let mut races: Vec<(&Race, f64, f64)> = insight
        .races
        .iter()
        .filter(|r| r.first_by[0].games * 30 >= n && r.first_by[1].games * 30 >= n)
        .map(|r| (r, r.first_by[0].win, 1.0 - r.first_by[1].win))
        .filter(|&(_, mine, theirs)| (mine - theirs).abs() >= 0.1)
        .collect();
    races.sort_by(|x, y| (y.1 - y.2).abs().total_cmp(&(x.1 - x.2).abs()));
    for (r, mine, theirs) in races.into_iter().take(8) {
        let (gets, both) = if r.contested.games == 0 { ("gets it", String::new()) } else { ("gains the first one", format!(" ({} of games both gain one)", pc(r.contested.games as f64 / r.first_wins.games.max(1) as f64))) };
        push("race", format!("{}: the champion wins {} when it {gets}, {} when the opponent does{both}.", r.card, pc(mine), pc(theirs)), mine - theirs, r.first_wins.games);
    }

    // (No opening sentences: the Lab lists every opening of the champion, by split, from `insight`.)

    // Positions: the decision trees' strongest leaves, against the champion's average at that turn.
    for t in &me.trees {
        let mut leaves: Vec<&Leaf> = t.leaves.iter().filter(|l| l.rate.games * 20 >= t.rate.games && !l.path.is_empty() && !l.path.iter().any(|p| p.contains(&count_top)) && (l.rate.win - t.rate.win).abs() >= 0.1).collect();
        leaves.sort_by(|x, y| (y.rate.win - t.rate.win).abs().total_cmp(&(x.rate.win - t.rate.win).abs()));
        for l in leaves.into_iter().take(2) {
            push("position", format!("Start of the champion's turn {}, {}: wins {} (on average {}).", t.turn, l.path.join(" and "), pc(l.rate.win), pc(t.rate.win)), l.rate.win - t.rate.win, l.rate.games);
        }
    }

    // Turning points.
    for w in me.swings.iter().filter(|w| w.card != top && w.effect.abs() >= 0.03).take(6) {
        let who = if w.who == "you" { "The champion gaining" } else { "The opponent gaining" };
        push("turning point", format!("{who} {} ({}).", w.card, w.window), w.effect, w.games);
    }

    Ok(Findings { champion: a.name.clone(), power: power(&all), field: rows, findings: out, insight, explanation, explain_error })
}
