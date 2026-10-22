//! Plunder bot defaults (rules in `docs/plunder-plan.md`): which starting card Inherited replaces, which cards Cage / Grotto / Avoid set aside, what Fated
//! puts on top, and Pendant's place in the Treasure order. The rest (trashes, discards, gains, modes) follow the generic policy.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, TREASURE, VICTORY};
use dominion_engine::sets::plunder::ids as p;
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Plunder;

/// A card that does nothing but sit in the deck.
fn junk(c: CardId) -> bool {
    c == id::CURSE || cards::is(c, cards::RUINS) || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
}

impl SetDefaults for Plunder {
    fn select(_s: &Strategy, _view: &PlayerView, _decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match (source, act, from) {
            // Inherited replaces a starting Estate (an Action in its place beats a victory point nobody wants early).
            (p::INHERITED, Act::Choose, Zone::StartDeck) => {
                Some(if choices.contains(&Choice::Card(id::ESTATE)) { Choice::Card(id::ESTATE) } else { iter_cards(choices).next().map(Choice::Card).unwrap_or(Choice::Pass) })
            }
            // Cards kept out of the shuffle or traded for new ones: the dead cards first.
            (p::CAGE | p::GROTTO | p::AVOID, Act::SetAside | Act::Discard, _) => {
                Some(iter_cards(choices).filter(|&c| junk(c) || c == id::COPPER).min_by_key(|&c| (c == id::COPPER, c)).map(Choice::Card).unwrap_or(Choice::Pass))
            }
            // Fated: leave the pile's cards where the shuffle puts them rather than burying them.
            (p::FATED, Act::Bottom, _) => Some(Choice::Pass),
            _ => None,
        }
    }

    /// Pendant counts the differently named Treasures in play, so it goes after every other Treasure.
    fn plays_last_rank(card: CardId) -> Option<u8> {
        (card == p::PENDANT).then_some(2)
    }
}
