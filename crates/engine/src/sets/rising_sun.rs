//! Rising Sun: only Change, the card that needs Debt. It is here, a set of its own with one card, and not among the Promos, because it is a
//! card of a real expansion that the rest of Rising Sun (Omens, Prophecies, Shadows, Sun tokens) does not come with; a page or Lab user who wants
//! Debt cards beyond Empires ticks "Rising Sun". See `docs/empires-plan.md`.

use super::prelude::*;
use crate::cards::{rs, ACTION as ACT};

/// Card ids of this set. Change is the last kingdom card id (`cards::LAST_KINGDOM`).
pub mod ids {
    use crate::cards::CardId;
    pub const CHANGE: CardId = 257;
}

const CARDS: &[(CardId, CardDef)] = &[
    // If the player has Debt: +$3. Otherwise trash a card from the hand and gain a card costing more $, taking Debt for the difference.
    (ids::CHANGE, rs(order_sensitive(c("Change", 4, ACT, 0, 0, 0, 0, 0)))), // trashes any card of the hand (may be the +Actions card), or +$3
];

/// The Rising Sun cards (see the module docs of `sets`).
pub struct RisingSun;

impl SetRules for RisingSun {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;

    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        if card == ids::CHANGE {
            g.change_play(p, sink);
        }
    }
}

impl GameState {
    /// Change: with any Debt, +$3; otherwise trash a card from the hand and gain a card costing more $ than it (it costs the difference in
    /// Debt, taken when the card is gained: `Then::ChangeGain`).
    fn change_play<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        if self.players[p as usize].debt > 0 {
            self.turn.coins += 3;
            sink.bonus(p, ids::CHANGE, 0, 0, 3, 0);
        } else {
            self.stack.push(select(p, ids::CHANGE, Zone::Hand, Act::Trash, Filter::Any, 1, 1, Then::ChangeGain));
        }
    }
}
