//! Leaf evaluators. A leaf is the state at the start of cleanup, *before* the next hand is
//! drawn (see `search::is_leaf`). `Evaluator::leaf_value` scores it for `me`; higher is better.

use dominion_engine::cards::{self, ACTION, TREASURE};
use dominion_engine::state::PlayerState;
use dominion_engine::{Choice, Counts, Decision, GameState};

pub trait Evaluator {
    /// Score `leaf` (end of the turn being searched) for `me`. `root` is the searched position,
    /// so evaluators can measure change over the turn or fix a horizon that the turn's own
    /// buys don't distort.
    fn leaf_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64;

    /// Whether `me` would consider `choice` at `decision` in `state` (e.g. a strategy only buys
    /// from its gain list). The search skips disallowed choices unless none are allowed.
    fn allows(&self, _state: &GameState, _me: u8, _decision: &Decision, _choice: Choice) -> bool {
        true
    }
}

/// Dominates every other term: winning the game outright if this turn ends it, sharing it,
/// or losing it. Zero if the game doesn't end with this turn.
pub fn game_end_value(leaf: &GameState, me: u8) -> f64 {
    const WIN: f64 = 1e15;
    match leaf.result_if_turn_ends() {
        None => 0.0,
        Some(w) if w & (1 << me) == 0 => -WIN,
        Some(w) if w.count_ones() == 1 => WIN,
        Some(_) => WIN / 2.0,
    }
}

#[inline]
fn treasure_value(c: &Counts) -> f64 {
    c.iter().filter(|&(id, _)| cards::is(id, TREASURE)).map(|(id, n)| cards::def(id).coins as f64 * n as f64).sum()
}

/// Everything that will be in `discard` once cleanup finishes. Depending on which path reached
/// the leaf, hand/in_play may not have been folded into discard yet, so sum all of them.
fn post_cleanup_pool(ps: &PlayerState) -> Counts {
    let mut c = ps.discard;
    c.add_all(&ps.hand);
    c.add_all(&ps.in_play);
    c.add_all(&ps.set_aside);
    c
}

/// Exact expected treasure value of the next 5-card hand. By linearity of expectation, drawing
/// `k` of `N` cards without replacement gives each card marginal probability `k/N`, so
/// `E[value] = k * V / N` exactly. Applied to the known top cards (probability 1), the unknown
/// remainder, and — if the deck runs short — the reshuffled discard (including this turn's
/// hand, in-play cards and gains).
pub fn expected_next_hand_money(ps: &PlayerState) -> f64 {
    const HAND: u32 = 5;
    let known_len = ps.deck_known.len as u32;
    let unknown_total = ps.deck_unknown.total();
    let deck_total = known_len + unknown_total;

    let known_value = |take: u32| -> f64 {
        ps.deck_known
            .iter_top_down()
            .take(take as usize)
            .map(|c| if cards::is(c, TREASURE) { cards::def(c).coins as f64 } else { 0.0 })
            .sum()
    };

    if deck_total >= HAND {
        if known_len >= HAND {
            return known_value(HAND);
        }
        let need = HAND - known_len;
        let unk_val = treasure_value(&ps.deck_unknown);
        return known_value(known_len) + need as f64 * (unk_val / unknown_total as f64);
    }

    // The whole deck is drawn for certain, then a reshuffle supplies the rest.
    let deck_val = known_value(known_len) + treasure_value(&ps.deck_unknown);
    let need = HAND - deck_total;
    let pool = post_cleanup_pool(ps);
    let pool_total = pool.total();
    if pool_total == 0 {
        return deck_val;
    }
    let need = need.min(pool_total);
    deck_val + need as f64 * (treasure_value(&pool) / pool_total as f64)
}

/// Weights for `NextHandEvaluator` (PLAN.md section 5, "Layer 1").
///
/// The evaluator converts money into VP-equivalents over the turns the game is expected to
/// last, so one formula prefers Gold early and Duchy late:
///
/// `value = VP + vp_per_coin * (E[next hand $] + (turns_left - 1) * avg_hand_$)`
#[derive(Clone, Copy, Debug)]
pub struct EvalWeights {
    /// VP-equivalent of +$1 in one future hand.
    pub vp_per_coin: f64,
    /// My expected remaining turns = 1 + turns_per_province * provinces_left / players.
    pub turns_per_province: f64,
}

impl Default for EvalWeights {
    fn default() -> Self {
        // Calibrated against Big Money Ultimate's rule of thumb (2 players): Gold beats Duchy
        // until about 4 Provinces remain (~7 turns left), and Silver about ties Duchy at the start.
        // Deliberately simple; tune or replace.
        EvalWeights { vp_per_coin: 0.55, turns_per_province: 3.0 }
    }
}

/// Rough number of turns `me` has left, from the Province pile and pile-out pressure.
pub fn estimated_turns_left(state: &GameState, w: &EvalWeights) -> f64 {
    let n = state.num_players as f64;
    let provinces = state.supply.get(cards::id::PROVINCE) as f64;
    let mut t = 1.0 + w.turns_per_province * provinces / n;
    let pile_limit = if state.num_players >= 5 { 4 } else { 3 };
    if state.empty_piles() + 1 >= pile_limit {
        t = t.min(2.0);
    }
    t
}

/// Average money an ordinary future hand produces with this deck: 5/N of each card's money
/// value. Treasures give their coins; Actions give their +$ plus, for +Cards, the cards drawn
/// times the deck's treasure density. Terminal Actions are discounted by the chance they
/// collide with another terminal. Ignores villages enabling terminals, attacks, +Buy and
/// trashing: a first-order heuristic.
pub fn average_hand_money(all: &Counts) -> f64 {
    let n = all.total() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let density = treasure_value(all) / n;
    let terminals: f64 = all
        .iter()
        .filter(|&(c, _)| cards::is(c, ACTION) && cards::def(c).actions == 0)
        .map(|(_, k)| k as f64)
        .sum();
    let p_alone = (1.0 - (5.0 / n).min(1.0)).powf((terminals - 1.0).max(0.0));
    let mut total = 0.0;
    for (c, k) in all.iter() {
        let d = cards::def(c);
        let v = if cards::is(c, TREASURE) {
            d.coins as f64
        } else if cards::is(c, ACTION) {
            let raw = d.coins as f64 + d.cards as f64 * density;
            if d.actions == 0 { raw * p_alone } else { raw }
        } else {
            0.0
        };
        total += v * k as f64;
    }
    5.0 * total / n
}

/// Layer 1: exact expected money of the next hand plus a stage-aware value of the deck.
#[derive(Default)]
pub struct NextHandEvaluator {
    pub weights: EvalWeights,
}

impl NextHandEvaluator {
    pub fn new(weights: EvalWeights) -> Self {
        NextHandEvaluator { weights }
    }
}

impl Evaluator for NextHandEvaluator {
    fn leaf_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let w = &self.weights;
        let ps = &leaf.players[me as usize];
        // Horizon from the start of the turn: buying a Province shouldn't make every future
        // coin look less valuable.
        let turns_left = estimated_turns_left(root, w);
        let future = expected_next_hand_money(ps) + (turns_left - 1.0).max(0.0) * average_hand_money(&ps.all_cards());
        game_end_value(leaf, me) + ps.vp() as f64 + w.vp_per_coin * future
    }
}

/// A simple baseline: VP plus raw treasure value owned, minus a small per-card thinning bonus.
/// Ignores next-hand timing entirely.
pub struct MoneyEvaluator;

impl Evaluator for MoneyEvaluator {
    fn leaf_value(&self, _root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let ps = &leaf.players[me as usize];
        let all = ps.all_cards();
        ps.vp() as f64 * 1000.0 + treasure_value(&all) - 0.1 * all.total() as f64
    }
}
