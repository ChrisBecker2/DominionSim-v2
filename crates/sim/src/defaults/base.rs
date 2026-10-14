//! Base Set bot defaults. The Base cards are mostly handled by the shape-based defaults in `strategy.rs` (a discard is
//! a discard, a trash a trash). Put a card's own policy here when it needs one (a `SetDefaults` method, called for the
//! card's set).

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{id, CardId};

use super::trashers;
use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Base;

impl SetDefaults for Base {
    /// Remodel must trash a card: not worth it when every card of the hand is worth as much as the best card it could become.
    fn useless_play(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        card == id::REMODEL && !trashers::upgrade_pays(s, view, card, 2, false)
    }
}
