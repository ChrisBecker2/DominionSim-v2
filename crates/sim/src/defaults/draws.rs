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
