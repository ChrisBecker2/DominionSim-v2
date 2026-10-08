//! Cornucopia & Guilds bot defaults: Overpay, Joust's Reward, Jester and Courser, Farmhands' set-aside, Plaza, Advisor, and Journeyman's
//! name.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ModeOpt, ACTION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Filter, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct CornucopiaGuilds;

impl SetDefaults for CornucopiaGuilds {
    fn select(_s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, _from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            // Advisor: the player to the left picks which of the revealed cards the revealer loses.
            id::ADVISOR if act == Act::Discard && decision.player != decision.for_player => {
                Some(iter_cards(choices).max_by_key(|&c| (Strategy::card_value(c), c)).map(Choice::Card).unwrap_or(choices[0]))
            }
            // Plaza: a Copper for a Coffers token (a token is at least the $1 the Copper would be).
            id::PLAZA if act == Act::Discard => {
                Some(if has_card(choices, id::COPPER) { Choice::Card(id::COPPER) } else { Choice::Pass })
            }
            id::FARMHANDS if act == Act::SetAside => Some(choose_farmhands_card(view, choices)),
            id::JOUST if act == Act::SetAside => Some(
                if has_card(choices, id::PROVINCE) && view.rewards_left().iter().any(|&n| n > 0) { Choice::Card(id::PROVINCE) } else { Choice::Pass },
            ),
            _ => None,
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            id::JESTER => Some(default_jester(s, view, choices)),
            id::COURSER => Some(default_courser(view, choices)),
            _ => None,
        }
    }

    /// Journeyman: name the card that is worst to draw, by how many of it there are to be skipped: Curses, then Victory cards,
    /// then Coppers; with none of those, a card not in the deck at all.
    fn name(_s: &Strategy, view: &PlayerView, _decision: &Decision, source: CardId, choices: &[Choice]) -> Option<Choice> {
        if source != id::JOURNEYMAN {
            return None;
        }
        let mut seen = view.deck();
        seen.add_all(view.discard());
        let junk = |c: CardId| -> u32 {
            if c == id::CURSE {
                4
            } else if cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE) {
                3
            } else if c == id::COPPER {
                2
            } else {
                0
            }
        };
        let best = iter_cards(choices).max_by_key(|&c| (junk(c) * seen.get(c) as u32, std::cmp::Reverse(c)));
        if let Some(c) = best.filter(|&c| junk(c) * seen.get(c) as u32 > 0) {
            return Some(Choice::Card(c));
        }
        Some(iter_cards(choices).find(|&c| seen.get(c) == 0).or(best).map(Choice::Card).unwrap_or(choices[0]))
    }

    /// How much to overpay (see `Strategy::choose_overpay`). Stonemason: the amount at which the gain list ranks an Action
    /// costing exactly that highest (with the Potion when that is a listed Potion card), none if there is no listed one.
    fn overpay(s: &Strategy, view: &PlayerView, card: CardId, max_coins: u8, potion: bool) -> Option<Choice> {
        if card != id::STONEMASON {
            return None;
        }
        let mut best: Option<(usize, u8, bool)> = None;
        for pot in [false, true] {
            if pot && !potion {
                continue;
            }
            for x in 0..=max_coins {
                if x == 0 && !pot {
                    continue;
                }
                if let Some(rank) = s.best_gain_rank(view, x, Filter::Action, true, pot) {
                    if best.map_or(true, |(r, _, _)| rank < r) {
                        best = Some((rank, x, pot));
                    }
                }
            }
        }
        Some(best.map_or(Choice::Overpay(0), |(_, x, pot)| Choice::overpay(x, pot)))
    }

    /// The most worth overpaying: Herald up to the cards in the discard pile, Infirmary up to 3.
    fn overpay_cap(view: &PlayerView, card: CardId) -> u32 {
        match card {
            id::HERALD => view.discard().total() + 1,
            id::INFIRMARY => 3,
            _ => u32::MAX,
        }
    }

    /// Joust's Reward when the gain list names none of them: Renown, Courser, Demesne, Coronet, Housecarl, Huge Turnip (the
    /// community's order of the Rewards).
    fn default_side_gain(choices: &[Choice]) -> Option<CardId> {
        default_reward(choices)
    }

    /// Horn of Plenty counts the differently named cards in play, so it goes after the other Treasures (but before Bank, which counts
    /// the Horn too).
    fn plays_last_rank(card: CardId) -> Option<u8> {
        (card == id::HORN_OF_PLENTY).then_some(0)
    }
}

/// Jester: the copy is mine if the gain list names the card or it costs $3 or more, else the victim's
/// (a Copper or an Estate-like card is no help to me, and the Jester's other effect, the Curse, is
/// not this choice).
pub(super) fn default_jester(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let card = view.pending_subject();
    let mine = card.is_some_and(|c| s.gain_rank(view, c).is_some() || cards::cost(c) >= 3);
    let want = ModeOpt::GainSubject { to_chooser: mine };
    let pick = cards::modes(id::JESTER).iter().position(|o| *o == want).map(|i| Choice::Mode(i as u8));
    pick.filter(|c| choices.contains(c)).unwrap_or(choices[0])
}

/// Courser (two different options): +2 Actions first when the hand holds Actions and none are left,
/// then +$2 and the Silvers; +2 Cards last.
pub(super) fn default_courser(view: &PlayerView, choices: &[Choice]) -> Choice {
    let want_actions = view.hand().any_type(ACTION) && view.turn().actions == 0;
    let prefs: [fn(&ModeOpt) -> bool; 4] =
        if want_actions { [is_actions, is_coins, is_gain_many, is_cards] } else { [is_coins, is_gain_many, is_cards, is_actions] };
    for pred in prefs {
        if let Some(c) = find_mode(id::COURSER, choices, pred) {
            return c;
        }
    }
    choices[0]
}

/// Farmhands' set-aside: an Action that cannot be played this turn (no Actions left, or already in
/// the Buy phase), the best one by what it gives; otherwise nothing (a Treasure set aside is as
/// much as it would give now).
pub(super) fn choose_farmhands_card(view: &PlayerView, choices: &[Choice]) -> Choice {
    let stuck = view.phase() == dominion_engine::Phase::Buy || view.turn().actions == 0;
    if !stuck {
        return Choice::Pass;
    }
    iter_cards(choices)
        .filter(|&c| cards::is(c, ACTION))
        .max_by_key(|&c| (prince_card_score(c), std::cmp::Reverse(c)))
        .map(Choice::Card)
        .unwrap_or(Choice::Pass)
}

/// Joust's Reward when the gain list names none of them: Renown, Courser, Demesne, Coronet,
/// Housecarl, Huge Turnip (the community's order of the Rewards).
pub(super) fn default_reward(choices: &[Choice]) -> Option<CardId> {
    const ORDER: [CardId; 6] = [id::RENOWN, id::COURSER, id::DEMESNE, id::CORONET, id::HOUSECARL, id::HUGE_TURNIP];
    ORDER.iter().copied().find(|&c| has_card(choices, c))
}
