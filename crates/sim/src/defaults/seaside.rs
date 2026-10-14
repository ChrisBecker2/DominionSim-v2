//! Seaside bot defaults: Native Village's mat, when Tactician and Treasure Map are worth playing, and the order the
//! start-of-turn effects of Durations resolve in.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ModeOpt};
use dominion_engine::engine::{Choice};
use dominion_engine::state::{Filter};

use super::draws;
use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Seaside;

/// The most chance of having to trash a listed card that still makes a Lookout worth playing.
const LOOKOUT_RISK: f64 = 0.02;

impl SetDefaults for Seaside {
    /// Lookout trashes one of the top three cards: not worth it once those are likely all cards the strategy lists.
    fn useless_play(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        card == id::LOOKOUT && draws::lookout_risk(s, view) > LOOKOUT_RISK
    }

    fn default_mode(_s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        (card == id::NATIVE_VILLAGE).then(|| default_native_village(view, choices))
    }

    /// Cards back into the hand first (Haven and Blockade returning their card, Pirate's Treasure), so the discards and trashes pick from
    /// the fullest hand; Sea Witch and Tide Pools discarding and Sailor trashing last.
    fn start_order_group(card: CardId) -> Option<u8> {
        match card {
            id::HAVEN | id::BLOCKADE | id::PIRATE => Some(0),
            id::TIDE_POOLS | id::SEA_WITCH | id::SAILOR => Some(3),
            _ => None,
        }
    }

    /// Default (unstated) check for Actions that can cost more than they give: Tactician only when the hand couldn't buy
    /// anything on the gain list anyway (it discards the hand for next turn's +5 Cards), Treasure Map only with a second Map
    /// in hand (alone it just trashes itself).
    fn worth_playing(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::TACTICIAN => {
                let hand = view.hand();
                let money: u32 = view.turn().coins as u32
                    + hand.iter().filter(|&(c, _)| view.is_treasure(c)).map(|(c, n)| cards::def(c).coins as u32 * n as u32).sum::<u32>();
                let potion = hand.has(id::POTION) || view.turn().potions > 0;
                s.best_gain_rank(view, money.min(u8::MAX as u32) as u8, Filter::Any, false, potion).is_none()
            }
            id::TREASURE_MAP => view.hand().get(id::TREASURE_MAP) >= 2,
            _ => true,
        }
    }
}

/// Native Village: take the mat once it holds 2+ cards, or once it holds any and this turn
/// could use them now (no actions left to keep chaining, or a thin hand); otherwise keep
/// adding to it.
pub(super) fn default_native_village(view: &PlayerView, choices: &[Choice]) -> Choice {
    let mat_size = view.native_village_mat().total();
    let needs_them_now = mat_size >= 1 && (view.turn().actions == 0 || view.hand().total() <= 1);
    let take = mat_size >= 2 || needs_them_now;
    let pred: fn(&ModeOpt) -> bool = if take { is_native_village_take } else { is_native_village_add };
    find_mode(id::NATIVE_VILLAGE, choices, pred).unwrap_or(choices[0])
}
