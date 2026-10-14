//! Prosperity bot defaults: Investment, War Chest's name, Clerk's place in the start-of-turn order, and Bank last.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ModeOpt};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Filter, Zone};

use super::trashers;
use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Prosperity;

impl SetDefaults for Prosperity {
    /// Expand: a forced upgrade, played only when it pays (see `defaults::trashers`).
    fn useless_play(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        card == id::EXPAND && !trashers::upgrade_pays(s, view, card, 3, false)
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        (card == id::INVESTMENT).then(|| default_investment(s, view, choices))
    }

    fn select(s: &Strategy, view: &PlayerView, _decision: &Decision, source: CardId, act: Act, _from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            // Anvil: discard the Treasure whose $ the gain list's best card costing up to $4 pays for, the cheapest first; none if no gain is worth
            // the buy it costs this turn.
            id::ANVIL if act == Act::Discard && !s.legacy_defaults => {
                let money = Strategy::money_in_hand(view);
                let gain = s.gain_weight(view, 4, Filter::Any, false, false);
                let mut best: Option<(f64, CardId)> = None;
                for c in iter_cards(choices) {
                    let lost = s.buy_weight(view, money) - s.buy_weight(view, money.saturating_sub(cards::def(c).coins as u32));
                    let v = gain - lost;
                    if v > 0.0 && best.map_or(true, |(b, _)| v > b) {
                        best = Some((v, c));
                    }
                }
                Some(best.map(|(_, c)| Choice::Card(c)).unwrap_or(Choice::Pass))
            }
            // Crystal Ball: the top card is played when it is a Treasure (its $ now) or an Action worth playing.
            id::CRYSTAL_BALL if act == Act::Play && !s.legacy_defaults => Some(
                iter_cards(choices)
                    .find(|&c| view.is_treasure(c) || (cards::is(c, cards::ACTION) && s.default_worth_playing(view, c)))
                    .map(Choice::Card)
                    .unwrap_or(Choice::Pass),
            ),
            _ => None,
        }
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

/// Investment: the +$1 when it reaches a better buy; otherwise trash it for the VP once the hand holds two differently named Treasures
/// (one VP token each, as the turn search values it against the $1 a later hand would have had); before that the safe +$1.
pub(super) fn default_investment(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let distinct = view.hand().iter().filter(|&(c, _)| view.is_treasure(c)).count();
    let money = Strategy::money_in_hand(view);
    let pays = s.buy_weight(view, money + 1) > s.buy_weight(view, money);
    let vp = if s.legacy_defaults { distinct >= 3 } else { !pays && distinct >= 2 };
    let pred: fn(&ModeOpt) -> bool = if vp { is_trash_self_investment } else { is_coins };
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
