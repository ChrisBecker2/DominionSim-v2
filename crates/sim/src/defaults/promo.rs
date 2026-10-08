//! Promo bot defaults: the Action Prince holds, the Supply Action Captain plays, and which card Envoy's left player makes
//! the revealer lose.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{id, CardId};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Promo;

impl SetDefaults for Promo {
    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            id::PRINCE if act == Act::SetAside => Some(choose_prince_card(s, view, choices)),
            // Captain: the Action from the Supply to play (now, or at the start of the next turn).
            id::CAPTAIN if act == Act::Play && from == Zone::Supply => Some(choose_captain_card(s, view, decision, choices)),
            // Envoy: the player to the left picks which of the revealed cards the revealer loses.
            id::ENVOY if act == Act::Discard && decision.player != decision.for_player => {
                Some(iter_cards(choices).max_by_key(|&c| (Strategy::card_value(c), c)).map(Choice::Card).unwrap_or(choices[0]))
            }
            _ => None,
        }
    }

    /// The play choice by rule order alone (inside search playouts): Captain's pick without the search.
    fn rule_play(s: &Strategy, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        (decision.source == Some(id::CAPTAIN)).then(|| captain_rule_pick(s, view, choices))
    }

    fn start_order_group(card: CardId) -> Option<u8> {
        (card == id::CAPTAIN).then_some(2)
    }

    fn start_order_play(card: CardId) -> bool {
        card == id::CAPTAIN
    }
}

/// The Action costing up to $4 in hand for a Prince to hold: the strategy's own [[play]]
/// rules first, in their order; else the most useful card (`prince_card_score`). It is played
/// at the start of every turn without using an Action, so being a terminal costs nothing.
pub(super) fn choose_prince_card(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    for (card, cond) in &s.play {
        if has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)) {
            return Choice::Card(*card);
        }
    }
    iter_cards(choices)
        .max_by_key(|&c| (prince_card_score(c), std::cmp::Reverse(c)))
        .map(Choice::Card)
        .unwrap_or(choices[0])
}

/// Captain's Action from the Supply: the strategy's own [[play]] rules first, in their order; else
/// the best line of the turn, searched when there is a real choice; else the most useful card
/// (`prince_card_score`). The card is played now or at the start of the turn, using no Action.
pub(super) fn choose_captain_card(s: &Strategy, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
    if let Some(c) = captain_stated_pick(s, view, choices) {
        return c;
    }
    if s.search_play && iter_cards(choices).count() >= 2 && decision.player == view.me() {
        if let Some(c) = crate::eval::search_play_choice(s, view) {
            if choices.contains(&c) {
                return c;
            }
        }
    }
    captain_default_pick(choices)
}

/// `choose_captain_card` without the search (used inside the search's own playouts).
pub(super) fn captain_rule_pick(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    captain_stated_pick(s, view, choices).unwrap_or_else(|| captain_default_pick(choices))
}

pub(super) fn captain_stated_pick(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Option<Choice> {
    s.play.iter().find_map(|(card, cond)| {
        (has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card))).then_some(Choice::Card(*card))
    })
}

pub(super) fn captain_default_pick(choices: &[Choice]) -> Choice {
    iter_cards(choices)
        .max_by_key(|&c| (prince_card_score(c), std::cmp::Reverse(c)))
        .map(Choice::Card)
        .unwrap_or(choices[0])
}
