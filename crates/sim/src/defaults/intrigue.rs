//! Intrigue bot defaults: the fast rule-order choice for each mode card (Pawn, Steward, Nobles, Minion, Courtier, Lurker, and
//! Torturer's victim), and when to refuse to reveal Moat to a Minion.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ModeOpt, ACTION, CURSE_T, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision};

use super::draws;
use super::trashers;
use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Intrigue;

impl SetDefaults for Intrigue {
    /// Trading Post, Upgrade: forced trashes, played only when the trash pays (see `defaults::trashers`).
    fn useless_play(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::TRADING_POST => !trashers::trading_post_pays(s, view, card),
            id::UPGRADE => !trashers::upgrade_pays(s, view, card, 1, true),
            _ => false,
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        if card == id::COURTIER {
            if let Some(c) = draws::mode_by_value(s, view, card, choices) {
                return Some(c);
            }
        }
        Some(match card {
            id::PAWN => default_pawn(view, choices),
            id::STEWARD => default_steward(s, view, choices),
            id::NOBLES => default_nobles(view, choices),
            id::MINION => default_minion(view, choices),
            id::COURTIER => default_courtier(choices),
            id::LURKER => default_lurker(s, view, choices),
            id::TORTURER => default_torturer(s, view, choices),
            _ => return None,
        })
    }

    /// Revealing a blocking Reaction (Moat) to an Attack that can help the victim: Minion hands a victim with 5+ cards a new
    /// hand of 4, declined when that is worth more than the hand held (the comparison `default_minion` makes).
    fn reveal_to_attack(_s: &Strategy, view: &PlayerView, _decision: &Decision, attack: CardId) -> Option<Choice> {
        if attack == id::MINION && view.hand().total() >= 5 {
            let (hand_money, redraw) = hand_vs_redraw(view);
            if redraw > hand_money as f64 {
                return Some(Choice::No);
            }
        }
        None
    }
}

/// Pawn: +1 Card and +$1, unless the hand holds an action but there are no actions left to
/// play it with, then +1 Action first.
pub(super) fn default_pawn(view: &PlayerView, choices: &[Choice]) -> Choice {
    let want_action = view.hand().any_type(ACTION) && view.turn().actions == 0;
    let prefs: [fn(&ModeOpt) -> bool; 4] =
        if want_action { [is_actions, is_cards, is_coins, is_buys] } else { [is_cards, is_coins, is_actions, is_buys] };
    for pred in prefs {
        if let Some(c) = find_mode(id::PAWN, choices, pred) {
            return c;
        }
    }
    choices[0]
}

/// Steward: trash 2 if the hand has 2+ cards the trash rules want gone, else +2 Cards.
pub(super) fn default_steward(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let junk: u32 = view.hand().iter().filter(|&(c, _)| s.wants_trash(view, c)).map(|(_, n)| n as u32).sum();
    let pred: fn(&ModeOpt) -> bool = if junk >= 2 { is_trash_hand } else { is_cards };
    find_mode(id::STEWARD, choices, pred).unwrap_or(choices[0])
}

/// Nobles: +2 Actions if the hand holds 2+ Action cards, else +3 Cards.
pub(super) fn default_nobles(view: &PlayerView, choices: &[Choice]) -> Choice {
    let pred: fn(&ModeOpt) -> bool = if view.hand().count_type(ACTION) >= 2 { is_actions } else { is_cards };
    find_mode(id::NOBLES, choices, pred).unwrap_or(choices[0])
}

/// Minion: +$2, unless a fresh 4-card hand is worth more: keep the hand when the treasure
/// in hand plus $2 (and any other Minions in hand, each good for at least $2 more) beats 4
/// cards drawn at the deck's average money per card.
pub(super) fn default_minion(view: &PlayerView, choices: &[Choice]) -> Choice {
    let (hand_money, redraw) = hand_vs_redraw(view);
    let keep = hand_money + 2 + 2 * view.hand().get(id::MINION) as u32;
    let pred: fn(&ModeOpt) -> bool = if redraw > keep as f64 { is_discard_hand_draw } else { is_coins };
    find_mode(id::MINION, choices, pred).unwrap_or(choices[0])
}

/// The treasure money in my hand, and what a new hand of 4 cards is worth on average (by the
/// money of the deck, reshuffling the discard in if the deck is short).
pub(super) fn hand_vs_redraw(view: &PlayerView) -> (u32, f64) {
    let money = |c: &Counts| -> u32 {
        c.iter().filter(|&(x, _)| cards::is(x, TREASURE)).map(|(x, n)| cards::def(x).coins as u32 * n as u32).sum()
    };
    let mut pool = view.deck();
    if pool.total() < 4 {
        pool.add_all(view.discard());
    }
    let redraw = if pool.total() == 0 { 0.0 } else { 4.0 * money(&pool) as f64 / pool.total() as f64 };
    (money(view.hand()), redraw)
}

/// Courtier: +$3, then gain a Gold, then +1 Buy, then +1 Action (in that priority order,
/// across however many picks the revealed card unlocked).
pub(super) fn default_courtier(choices: &[Choice]) -> Choice {
    let prefs: [fn(&ModeOpt) -> bool; 4] = [is_coins, is_gain, is_buys, is_actions];
    for pred in prefs {
        if let Some(c) = find_mode(id::COURTIER, choices, pred) {
            return c;
        }
    }
    choices[0]
}

/// Lurker: gain an Action from the trash if it holds one the gain list wants; otherwise
/// trash from the Supply the costliest Action the strategy doesn't want (the actual trash
/// target is then chosen by `choose_trash`'s `Zone::Supply` branch).
pub(super) fn default_lurker(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let trash_has_wanted = view.trash().iter().any(|(c, _)| cards::is(c, ACTION) && s.gain_rank(view, c).is_some());
    let pred: fn(&ModeOpt) -> bool = if trash_has_wanted { is_gain_trash } else { is_trash_supply };
    find_mode(id::LURKER, choices, pred).unwrap_or(choices[0])
}

/// Torturer's victim: gain the Curse only if discarding 2 would throw away more value than a
/// Curse costs later. Heuristic: discard 2 if the hand has at least 2 cards that are
/// Victory/Curse/Copper or otherwise excess (unwanted per the trash rules); otherwise gain
/// the Curse to hand. Always used regardless of `search_play` (a reactive decision during
/// someone else's turn, not this player's own play to search).
pub(super) fn default_torturer(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let expendable: u32 = view
        .hand()
        .iter()
        .filter(|&(c, _)| c == id::COPPER || cards::is(c, VICTORY) || cards::is(c, CURSE_T) || s.wants_trash(view, c))
        .map(|(_, n)| n as u32)
        .sum();
    let pred: fn(&ModeOpt) -> bool = if expendable >= 2 { is_discard_hand } else { is_gain };
    find_mode(id::TORTURER, choices, pred).unwrap_or(choices[0])
}
