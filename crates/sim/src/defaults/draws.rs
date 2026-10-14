//! When a card that draws or reshapes the hand at a price is worth playing (`SetDefaults::useless_play`), judged by the $ the hand is
//! expected to have afterwards against the buy it has now (the turn search sees this as the best buy of each line).
//!
//! Root Cellar draws 3 cards for 3 Debt, Royal Blacksmith draws 5 and discards the Coppers in the hand: both can leave a hand worth less
//! than it was, when the deck is poor in $. Lookout trashes one of three cards it looks at: it can only lose when all three are cards the
//! strategy wants.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id};

use crate::strategy::*;

/// The cards a draw takes from: the deck, or when it is nearly empty everything the player owns outside the hand and play (the discard
/// pile is shuffled into the next draw).
fn draw_pool(view: &PlayerView) -> dominion_engine::Counts {
    let deck = view.deck();
    if deck.total() >= 5 {
        deck
    } else {
        let mut all = view.my_cards();
        for (c, n) in view.hand().iter().chain(view.in_play().iter()) {
            all.set(c, all.get(c).saturating_sub(n));
        }
        all
    }
}

/// The average $ of a card of the pool (Coppers count for nothing when `no_copper`).
fn money_per_card(view: &PlayerView, no_copper: bool) -> f64 {
    let pool = draw_pool(view);
    let n = pool.total().max(1) as f64;
    let coins: u32 = pool
        .iter()
        .filter(|&(c, _)| view.is_treasure(c) && !(no_copper && c == id::COPPER))
        .map(|(c, k)| cards::def(c).coins as u32 * k as u32)
        .sum();
    coins as f64 / n
}

/// Root Cellar: +3 Cards for 3 Debt that comes out of the $ of the turn. Worth it when the hand's $ plus the expected $ of three cards, less the Debt,
/// still buys as much as the hand does now.
pub(super) fn root_cellar_pays(s: &Strategy, view: &PlayerView) -> bool {
    let money = Strategy::money_in_hand(view);
    let after = (money as f64 + 3.0 * money_per_card(view, false) - 3.0).round().max(0.0) as u32;
    s.buy_weight(view, after) >= s.buy_weight(view, money)
}

/// Royal Blacksmith: +5 Cards, then the Coppers of the hand are discarded.
pub(super) fn royal_blacksmith_pays(s: &Strategy, view: &PlayerView) -> bool {
    let money = Strategy::money_in_hand(view);
    let coppers = view.hand().get(id::COPPER) as f64;
    let after = (money as f64 - coppers + 5.0 * money_per_card(view, true)).round().max(0.0) as u32;
    s.buy_weight(view, after) >= s.buy_weight(view, money)
}

/// Lookout: the chance that the three cards it looks at are all cards the strategy lists (it must trash one of them).
pub(super) fn lookout_risk(s: &Strategy, view: &PlayerView) -> f64 {
    let pool = draw_pool(view);
    let n = pool.total() as f64;
    if n < 3.0 {
        return 1.0;
    }
    let wanted: f64 = pool.iter().filter(|&(c, _)| s.weight_of(view, c) > 0.0).map(|(_, k)| k as f64).sum();
    (wanted / n) * ((wanted - 1.0).max(0.0) / (n - 1.0)) * ((wanted - 2.0).max(0.0) / (n - 2.0))
}

/// The Action from the Supply that a card playing "an Action from the Supply" (Overlord, Band of Misfits, Captain) is best used as: the
/// strategy's own `[[play]]` rules first, in their order; else the card worth most in this hand, in $: its $, its cards at the average $ of
/// the deck, a Buy a little, +Actions for the Actions the hand has left to play, an Attack a little. Cards that only help with a choice
/// (a gain, a trash) are worth a little less than their plain bonuses.
pub(super) fn mimic_pick(s: &Strategy, view: &PlayerView, choices: &[dominion_engine::engine::Choice]) -> dominion_engine::engine::Choice {
    use dominion_engine::engine::Choice;
    if let Some(c) = s.play.iter().find_map(|(card, cond)| {
        (has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card))).then_some(*card)
    }) {
        return Choice::Card(c);
    }
    let avg = money_per_card(view, false);
    // More $ buy nothing better when the hand already reaches the best card the gain list could ask for.
    let money = Strategy::money_in_hand(view);
    let need = if s.buy_weight(view, money) >= s.buy_weight(view, money + 8) { 0.0 } else { 1.0 };
    let others: f64 = view.hand().iter().filter(|&(c, _)| cards::is(c, cards::ACTION)).map(|(_, n)| n as f64).sum();
    let score = |c: CardId| -> f64 {
        let d = cards::def(c);
        let mut v = need * (d.coins as f64 + d.cards as f64 * avg) + 0.4 * d.buys as f64;
        if d.actions >= 2 {
            v += (d.actions as f64 - 1.0).min(others) * 2.0;
        } else if d.actions == 1 {
            v += 0.3;
        }
        if cards::is(c, cards::ATTACK) {
            v += 1.2;
        }
        v
    };
    iter_cards(choices)
        .max_by(|&a, &b| score(a).total_cmp(&score(b)).then(b.cmp(&a)))
        .map(Choice::Card)
        .unwrap_or(choices[0])
}

/// The option of a mode card (Courtier, Squire, Count's second choice) that the buy values most, as the turn search sees it: +$ by the buy
/// it reaches, a gain by its place in the gain list, cards by the $ they are expected to bring, a Buy or an Action a little. `None` when an
/// option is not one of these (the card's own default decides). Ties go to the first option.
pub(super) fn mode_by_value(s: &Strategy, view: &PlayerView, card: CardId, choices: &[dominion_engine::engine::Choice]) -> Option<dominion_engine::engine::Choice> {
    use dominion_engine::cards::ModeOpt;
    use dominion_engine::engine::Choice;
    if s.legacy_defaults {
        return None;
    }
    let money = Strategy::money_in_hand(view);
    let base = s.buy_weight(view, money);
    let avg = money_per_card(view, false);
    let actions_left = view.hand().iter().any(|(c, _)| cards::is(c, cards::ACTION));
    let table = cards::modes(card);
    let mut best: Option<(f64, Choice)> = None;
    for &ch in choices {
        let Choice::Mode(i) = ch else { return None };
        let v = match table.get(i as usize)? {
            ModeOpt::Coins(n) => s.buy_weight(view, money + *n as u32),
            ModeOpt::Cards(n) => s.buy_weight(view, money + (*n as f64 * avg).round() as u32),
            ModeOpt::Gain(c, _) => if view.supply(*c) > 0 { s.weight_of(view, *c) + base } else { f64::NEG_INFINITY },
            ModeOpt::Buys(_) => base + 0.1,
            ModeOpt::Actions(_) => base + if actions_left { 2.0 } else { 0.0 },
            ModeOpt::TrashHand => f64::NEG_INFINITY,
            _ => return None,
        };
        if best.map_or(true, |(b, _)| v > b) {
            best = Some((v, ch));
        }
    }
    best.map(|(_, c)| c)
}
