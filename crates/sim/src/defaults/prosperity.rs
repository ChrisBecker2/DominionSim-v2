//! Prosperity bot defaults: Investment, War Chest's name, Clerk's place in the start-of-turn order, and Bank last.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{id, CardId, ModeOpt};
use dominion_engine::engine::{Choice, Decision};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Prosperity;

impl SetDefaults for Prosperity {
    fn default_mode(_s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        (card == id::INVESTMENT).then(|| default_investment(view, choices))
    }

    /// War Chest, as the namer (the player to the owner's left): deny the most expensive card up to $5.
    fn name(_s: &Strategy, view: &PlayerView, _decision: &Decision, source: CardId, choices: &[Choice]) -> Option<Choice> {
        (source == id::WAR_CHEST).then(|| choose_war_chest_name(view, choices))
    }

    fn start_order_group(card: CardId) -> Option<u8> {
        (card == id::CLERK).then_some(2)
    }

    /// Clerk plays itself from the hand: the one start-of-turn effect of the set with a play in it.
    fn start_order_play(card: CardId) -> bool {
        card == id::CLERK
    }

    /// Bank counts the Treasures in play, so it goes after every other one (and after Horn of Plenty).
    fn plays_last_rank(card: CardId) -> Option<u8> {
        (card == id::BANK).then_some(1)
    }
}

/// Investment: trash it for VP once the hand already holds decent Treasure diversity (3+
/// differently-named Treasures), otherwise the safe +$1.
pub(super) fn default_investment(view: &PlayerView, choices: &[Choice]) -> Choice {
    let distinct = view.hand().iter().filter(|&(c, _)| view.is_treasure(c)).count();
    let pred: fn(&ModeOpt) -> bool = if distinct >= 3 { is_trash_self_investment } else { is_coins };
    find_mode(id::INVESTMENT, choices, pred).unwrap_or(choices[0])
}

/// War Chest, as the namer: deny the most expensive card up to $5 (a card costing more is
/// never reachable by War Chest's own gain anyway, so naming it denies nothing), guessing
/// it's the biggest prize the owner was after. A simple heuristic, not adversarially optimal
/// (we don't model the owner's own strategy).
pub(super) fn choose_war_chest_name(view: &PlayerView, choices: &[Choice]) -> Choice {
    iter_cards(choices)
        .filter(|&c| view.cost(c) <= 5)
        .max_by_key(|&c| view.cost(c))
        .or_else(|| iter_cards(choices).min_by_key(|&c| view.cost(c)))
        .map(Choice::Card)
        .unwrap_or_else(|| choices[0])
}
