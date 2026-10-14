//! When a card that has to trash something from the hand is worth playing, for the cards of every set (`SetDefaults::useless_play`).
//!
//! The turn search passes on Remodel, Remake, Develop, Stonemason, Trading Post, Trader, Forager, Rats, Tanuki... whenever every card it
//! could trash is worth more to the gain list than what the effect gives: a Silver for a Silver costs this turn's $2, a Gold for a Province
//! is a gain, an Estate for a Silver is a gain, a Copper for nothing is free. The fast bot judges the same sum on the same scale
//! (`Strategy::trash_value`): the best card of the hand to give up is the one that maximizes what the effect gains minus what losing the
//! card costs (its own place in the gain list, and the buy its coins would have paid for). The effect is worth playing when that is not
//! negative. These checks apply to stated `[[play]]` rules too: "play Remodel" does not mean "play it at a loss".

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId};
use dominion_engine::state::Filter;

use crate::strategy::*;

/// What trashing `c` gains for Remodel and its kin: the best card costing up to (exactly, if `exact`) `plus` more.
pub(super) fn upgrade_gain(s: &Strategy, view: &PlayerView, c: CardId, plus: u8, exact: bool) -> f64 {
    s.gain_weight(view, view.cost(c).saturating_add(plus), Filter::Any, exact, cards::potion_cost(c))
}

/// Stonemason: two cards each costing less. Gaining nothing the strategy lists is a loss (it fills the deck with the cheap cards).
pub(super) fn stonemason_gain(s: &Strategy, view: &PlayerView, c: CardId) -> f64 {
    match view.cost(c) {
        // Nothing costs less: a plain trash (a Copper thinned out).
        0 => 0.0,
        k => match s.gain_weight(view, k - 1, Filter::Any, false, false) {
            g if g > 0.0 => 2.0 * g,
            _ => -1.0,
        },
    }
}

/// Develop: one card costing exactly $1 more and one exactly $1 less.
pub(super) fn develop_gain(s: &Strategy, view: &PlayerView, c: CardId) -> f64 {
    let k = view.cost(c);
    let up = s.gain_weight(view, k.saturating_add(1), Filter::Any, true, cards::potion_cost(c));
    let down = if k > 0 { s.gain_weight(view, k - 1, Filter::Any, true, cards::potion_cost(c)) } else { 0.0 };
    up + down
}

/// Trader: a Silver per $1 the card cost.
pub(super) fn trader_gain(s: &Strategy, view: &PlayerView, c: CardId) -> f64 {
    view.cost(c) as f64 * s.weight_of(view, id::SILVER)
}

/// Whether the best card of the hand is worth giving to the effect (not a loss).
fn pays(s: &Strategy, view: &PlayerView, me: CardId, gain: impl Fn(&Strategy, CardId) -> f64) -> bool {
    s.best_hand_trash(view, me, gain).is_some_and(|(_, v)| v >= 0.0)
}

/// The card to trash among `choices` for an effect worth `gain` (the best net value; a forced trash takes the least bad).
pub(super) fn pick(s: &Strategy, view: &PlayerView, choices: &[dominion_engine::engine::Choice], gain: impl Fn(&Strategy, CardId) -> f64) -> Option<dominion_engine::engine::Choice> {
    s.best_trash_among(view, iter_cards(choices), gain).map(|(c, _)| dominion_engine::engine::Choice::Card(c))
}

/// Remodel-like: trash a card, gain one costing up to (exactly, if `exact`) `plus` more.
pub(super) fn upgrade_pays(s: &Strategy, view: &PlayerView, me: CardId, plus: u8, exact: bool) -> bool {
    pays(s, view, me, |s, c| upgrade_gain(s, view, c, plus, exact))
}

pub(super) fn stonemason_pays(s: &Strategy, view: &PlayerView, me: CardId) -> bool {
    pays(s, view, me, |s, c| stonemason_gain(s, view, c))
}

pub(super) fn develop_pays(s: &Strategy, view: &PlayerView, me: CardId) -> bool {
    pays(s, view, me, |s, c| develop_gain(s, view, c))
}

pub(super) fn trader_pays(s: &Strategy, view: &PlayerView, me: CardId) -> bool {
    pays(s, view, me, |s, c| trader_gain(s, view, c))
}

/// A card whose trash gains nothing (Rats, Forager): worth playing when something in the hand is free to lose. With nothing else in the
/// hand the card is a cantrip that trashes from what it draws: played.
pub(super) fn plain_trash_pays(s: &Strategy, view: &PlayerView, me: CardId) -> bool {
    // Rats gains a Rats (worth something while the strategy still lists one) and cannot trash a Rats.
    let bonus = if me == id::RATS { s.weight_of(view, id::RATS) } else { 0.0 };
    s.best_hand_trash(view, me, |_, c| if me == id::RATS && c == id::RATS { f64::NEG_INFINITY } else { bonus }).map_or(true, |(_, v)| v >= 0.0 || !v.is_finite())
}

/// Forager's $: one for each differently named Treasure in the trash once `c` is in it.
fn forager_coins(view: &PlayerView, c: CardId) -> u32 {
    let names = view.trash().iter().filter(|&(t, _)| view.is_treasure(t)).count() as u32;
    names + (view.is_treasure(c) && view.trash().get(c) == 0) as u32
}

/// Forager: worth playing when the $ it brings pay for the trash (a Copper into an empty trash is free).
pub(super) fn forager_pays(s: &Strategy, view: &PlayerView, me: CardId) -> bool {
    s.best_trash_with_coins(view, view.hand().iter().filter(|&(c, n)| c != me || n >= 2).map(|(c, _)| c), |_, _| 0.0, |_, c| forager_coins(view, c))
        .map_or(true, |(_, v)| v >= 0.0)
}

/// The card Forager trashes among `choices`.
pub(super) fn forager_pick(s: &Strategy, view: &PlayerView, choices: &[dominion_engine::engine::Choice]) -> Option<dominion_engine::engine::Choice> {
    s.best_trash_with_coins(view, iter_cards(choices), |_, _| 0.0, |_, c| forager_coins(view, c)).map(|(c, _)| dominion_engine::engine::Choice::Card(c))
}

/// Trading Post: trash two cards, gain a Silver into the hand (it is played at once: +$2).
pub(super) fn trading_post_pays(s: &Strategy, view: &PlayerView, me: CardId) -> bool {
    let money = Strategy::money_in_hand(view);
    // The cards of the hand that can be trashed (one copy of Trading Post itself cannot).
    let mut hand = [0 as CardId; 12];
    let mut n = 0;
    for (c, k) in view.hand().iter() {
        let k = if c == me { k.saturating_sub(1) } else { k };
        for _ in 0..k.min((12 - n) as u8) {
            hand[n] = c;
            n += 1;
        }
    }
    let coins = |c: CardId| if view.is_treasure(c) { cards::def(c).coins as u32 } else { 0 };
    let base = s.buy_weight(view, money);
    let silver = s.weight_of(view, id::SILVER);
    let mut best = f64::NEG_INFINITY;
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (hand[i], hand[j]);
            let left = (money + 2).saturating_sub(coins(a) + coins(b));
            let v = silver + s.buy_weight(view, left) - base - s.weight_of(view, a) - s.weight_of(view, b)
                + if s.wants_trash(view, a) { 0.3 } else { -0.3 }
                + if s.wants_trash(view, b) { 0.3 } else { -0.3 };
            best = best.max(v);
        }
    }
    best >= 0.0
}

/// Dismantle: a Gold (when the card cost $1 or more) and a card costing less.
pub(super) fn dismantle_gain(s: &Strategy, view: &PlayerView, c: CardId) -> f64 {
    match view.cost(c) {
        0 => 0.0,
        k => s.weight_of(view, id::GOLD) + s.gain_weight(view, k - 1, Filter::Any, false, cards::potion_cost(c)),
    }
}

pub(super) fn dismantle_pays(s: &Strategy, view: &PlayerView, me: CardId) -> bool {
    pays(s, view, me, |s, c| dismantle_gain(s, view, c))
}
