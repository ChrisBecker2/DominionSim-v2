//! Insight: how the games between two strategies go, told for people rather than for the search. Plays the games once and reports
//!
//! - the win rate by opening split (5/2 or 4/3, for both players) and what each split opened with;
//! - what each strategy gains and trashes, turn by turn (a heat map), and how that goes with winning;
//! - win rate by how many copies of a card were gained by turn 10, and owned at the end (the points where one more copy starts to lose);
//! - pairs of cards in the final deck, the Prophecy (when, and who made it happen), differently named cards (Museum);
//! - decision trees: at the start of own turns 5, 9, 13 and 17, a shallow tree over the position (VP lead, money, the cards owned, the
//!   opponent's) that predicts the winner; its leaves read as conditions of the strategy language ("count(Gold) >= 1 and vp_lead < -4");
//! - turning points: the gains between two of those turns that move the tree's estimate most, beyond what the position already said;
//! - rule explanations ([`explain`]): each rule of a strategy taken out, the same deals replayed: what the rule is worth and when it acts.
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
    trashes: [Vec<(u16, u16, CardId)>; 2],
    prophecy: Option<(u8, u16)>,
}

impl EventSink for Sink {
    fn event(&mut self, e: Event) {
        match e {
            Event::TurnStart { player, turn } => {
                self.cur = player;
                self.global = turn;
                if (player as usize) < 2 {
                    self.own_turn[player as usize] += 1;
                }
            }
            Event::Draw { player, card } if (player as usize) < 2 && self.opening[player as usize].len() < 10 => self.opening[player as usize].push(card),
            Event::Gain { player, card, .. } if (player as usize) < 2 => self.gains[player as usize].push((self.own_turn[player as usize], self.global, card)),
            Event::Trash { player, card } if (player as usize) < 2 => self.trashes[player as usize].push((self.own_turn[player as usize], self.global, card)),
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
    /// The strategy that made the Prophecy happen, and on which of its own turns.
    prophecy: Option<(usize, u16)>,
    deck: [Vec<(CardId, u16)>; 2],
    vp: [i32; 2],
    turns: [u16; 2],
    snaps: [Vec<Snap>; 2],
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
        trashes: std::array::from_fn(|i| sink.trashes[seat(i)].clone()),
        prophecy: sink.prophecy.map(|(p, t)| (if seat(0) == p as usize { 0 } else { 1 }, t)),
        deck: std::array::from_fn(|i| sparse(&PlayerView::new(&st, seat(i) as u8).my_cards())),
        vp: std::array::from_fn(|i| scores[seat(i)]),
        turns: std::array::from_fn(|i| st.players[seat(i)].turns_taken),
        snaps: if swap { [s1, s0] } else { [s0, s1] },
    }
}

/// `games` games, seats alternating, every pair of games on one deal (common random numbers).
fn play_many(kingdom: &[CardId], s: [&Strategy; 2], games: u64, seed: u64) -> Vec<Game> {
    (0..games).into_par_iter().map(|g| play_one(kingdom, s, Rng::derive(seed, g / 2).next_u64(), g % 2 == 1)).collect()
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

#[derive(Serialize, Debug)]
pub struct Opening {
    /// The cards gained on the first two turns, e.g. "Change + Silver".
    pub cards: String,
    pub rate: Rate,
}

#[derive(Serialize, Debug)]
pub struct SplitRow {
    /// "5/2" or "4/3".
    pub split: String,
    pub rate: Rate,
    /// The same, by the opponent's split.
    pub vs: Vec<(String, Rate)>,
    pub openings: Vec<Opening>,
}

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
pub struct CountCurve {
    pub card: String,
    /// (copies, rate); "8" stands for 8 or more. Only counts seen in at least 2% of games.
    pub points: Vec<(String, Rate)>,
}

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
pub struct TreeAt {
    /// The own turn at whose start the positions were taken.
    pub turn: u16,
    pub rate: Rate,
    /// Best first.
    pub leaves: Vec<Leaf>,
}

/// A turning point: games where `who` gained `card` between two recorded turns moved the win estimate by `effect` more than the others.
#[derive(Serialize, Debug)]
pub struct Swing {
    /// e.g. "turns 5-8" (own turns), or "turn 17 to the end".
    pub window: String,
    /// "you" (this strategy) or "opponent".
    pub who: String,
    pub card: String,
    pub games: usize,
    pub effect: f64,
}

#[derive(Serialize, Debug)]
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
}

#[derive(Serialize, Debug)]
pub struct ProphecyStats {
    pub happened: usize,
    /// Per strategy: the games it made the Prophecy happen in (its win rate there), and on which of its own turns on average.
    pub made_by: Vec<(Rate, f64)>,
}

/// What one rule of a strategy is worth: the same deals played without it.
#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
pub struct Explanation {
    /// The strategy's win rate as the rules read it (the file round-tripped through the template language).
    pub with: f64,
    pub games: u64,
    pub rules: Vec<RuleImpact>,
}

#[derive(Serialize, Debug)]
pub struct Insight {
    pub kingdom: Vec<String>,
    pub games: usize,
    pub turns: usize,
    pub sides: Vec<Side>,
    pub prophecy: Option<ProphecyStats>,
}

/// Play `games` games between `a` and `b` on `kingdom` and report.
pub fn analyze(kingdom: &[CardId], a: &Strategy, b: &Strategy, games: u64, seed: u64) -> Insight {
    let all = play_many(kingdom, [a, b], games.max(2), seed);
    let n = all.len();
    let sides = (0..2).map(|i| side(&all, i, if i == 0 { &a.name } else { &b.name })).collect();
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
    Insight { kingdom: kingdom.iter().map(|&c| cards::name(c).to_string()).collect(), games: n, turns: TURNS, sides, prophecy }
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
    }
}

fn splits(all: &[Game], i: usize) -> Vec<SplitRow> {
    let label = |s: (u8, u8)| format!("{}/{}", s.0, s.1);
    [(5, 2), (4, 3)]
        .into_iter()
        .map(|split| {
            let gs: Vec<&Game> = all.iter().filter(|g| g.split[i] == split).collect();
            let vs = [(5, 2), (4, 3)].into_iter().map(|os| (label(os), rate(gs.iter().filter(|g| g.split[1 - i] == os).map(|g| g.win[i])))).collect();
            let mut open: BTreeMap<String, Vec<f64>> = BTreeMap::new();
            for g in &gs {
                let mut names: Vec<&str> = g.gains[i].iter().filter(|&&(t, _, _)| t <= 2).map(|&(_, _, c)| cards::name(c)).collect();
                names.sort();
                open.entry(if names.is_empty() { "nothing".into() } else { names.join(" + ") }).or_default().push(g.win[i]);
            }
            let mut openings: Vec<Opening> = open.into_iter().map(|(cards, w)| Opening { cards, rate: rate(w.into_iter()) }).collect();
            openings.sort_by(|a, b| b.rate.games.cmp(&a.rate.games));
            openings.truncate(5);
            SplitRow { split: label(split), rate: rate(gs.iter().map(|g| g.win[i])), vs, openings }
        })
        .collect()
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
    let g = Genome::from_toml(a_toml).map_err(|e| format!("only strategies in the search's rule language can be explained rule by rule: {e}"))?;
    let name = Strategy::parse(a_toml)?.name;
    let compile = |g: &Genome| -> Result<Strategy, String> {
        let mut s = Strategy::parse(&g.to_toml(&TomlMeta { name: name.clone(), ..Default::default() }))?;
        if fast {
            make_fast(&mut s);
        }
        Ok(s)
    };
    let games = games.max(2);
    let base_s = compile(&g)?;
    let base = play_many(kingdom, [&base_s, b], games, seed);
    let base_rate = rate(base.iter().map(|x| x.win[0])).win;
    let mut rules = Vec::new();
    for (list, rs) in g.lists() {
        for (k, r) in rs.iter().enumerate() {
            let mut v = g.clone();
            v.list_mut(list).remove(k);
            let s = compile(&v)?;
            let res = play_many(kingdom, [&s, b], games, seed);
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
    Ok(Explanation { with: base_rate, games, rules })
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
}
