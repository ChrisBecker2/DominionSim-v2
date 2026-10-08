//! Alchemy bot defaults: Scrying Pool's keep-or-discard and Herbalist's Treasure.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Alchemy;

impl SetDefaults for Alchemy {
    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        // Scrying Pool: the owner decides, for each player's revealed top card, whether it is discarded (`Card`) or put back (`Pass`).
        if act == Act::Discard && from == Zone::Revealed && source == id::SCRYING_POOL {
            return Some(choose_scry(view, decision, choices));
        }
        // Herbalist: the Treasure to put on the deck from play.
        if act == Act::Topdeck && from == Zone::InPlay && source == id::HERBALIST {
            return Some(choose_herbalist_topdeck(s, view, choices));
        }
        None
    }
}

/// Scrying Pool, for the top card `choices` offers (`Card(c)` = discard it, `Pass` = put it
/// back). On my own deck: discard what I don't want to draw: Curses and Victory-only cards,
/// and Treasures worth less than the average card the discard would let me reach instead
/// (the average money of my deck's non-Action cards); keep every Action (the reveal-until
/// draws them all), Potions and good Treasures. On an opponent's deck, the reverse: discard the cards
/// they'd like to draw (Actions, Silver or better, Potions) and leave their junk on top.
pub(super) fn choose_scry(view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
    let Some(c) = iter_cards(choices).next() else { return Choice::Pass };
    let discard = if decision.for_player == view.me() {
        if cards::is(c, ACTION) || c == id::POTION {
            false
        } else if c == id::CURSE || (cards::is(c, VICTORY) && !cards::is(c, TREASURE)) {
            true
        } else {
            let deck = view.deck();
            let (mut money, mut n) = (0u32, 0u32);
            for (x, k) in deck.iter().filter(|&(x, _)| !cards::is(x, ACTION)) {
                money += cards::def(x).coins as u32 * k as u32;
                n += k as u32;
            }
            // value(c) < average, compared without division.
            n > 0 && (cards::def(c).coins as u32) * n < money
        }
    } else {
        cards::is(c, ACTION) || c == id::POTION || (cards::is(c, TREASURE) && cards::def(c).coins >= 2)
    };
    if discard { Choice::Card(c) } else { Choice::Pass }
}

/// Herbalist: put the best Treasure in play on the deck: a Potion if the gain list wants a
/// card with a Potion in its cost that is still in the supply (so next turn can buy it),
/// else the Treasure worth the most; never a Curse (Charlatan's Treasure Curse) or a worthless
/// Treasure (an unwanted Potion): then nothing.
pub(super) fn choose_herbalist_topdeck(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let wants_potion_card = s.buy.iter().any(|(c, _)| cards::potion_cost(*c) && view.in_supply(*c) && view.supply(*c) > 0);
    let key = |c: CardId| (wants_potion_card && c == id::POTION, cards::def(c).coins, c);
    iter_cards(choices)
        .filter(|&c| c != id::CURSE && (cards::def(c).coins > 0 || (wants_potion_card && c == id::POTION)))
        .max_by_key(|&c| key(c)).map(Choice::Card).unwrap_or(Choice::Pass)
}
