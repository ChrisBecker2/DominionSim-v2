//! Hinterlands bot defaults (see `docs/hinterlands-plan.md`, section 6). What the rules say nothing about:
//! * the reactions: Fool's Gold is always trashed for its Gold, a Tunnel always revealed, a Trail or Weaver always played, a Berserker is played
//!   by the engine; the Trader's exchange follows `[[exchange]]` rules for the gained card, else swaps junk and unlisted cards for a Silver;
//! * the small choices inside cards: Jack of All Trades' look at the top card, Cartographer's discards, Stables' Copper, Wheelwright's discard,
//!   Witch's Hut's two Actions, Inn's Actions to shuffle in, Scheme's Actions to put back, Develop's trash, Weaver's and Spice Merchant's modes;
//! * which Actions are worth playing (Souk with a big hand is not).

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Zone};

use super::trashers;
use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Hinterlands;

/// A card that does nothing for a deck: Curse, Ruins, Shelters that are junk, Victory cards that are not Actions or Treasures.
fn is_junk(c: CardId) -> bool {
    c == id::CURSE
        || cards::is(c, cards::RUINS)
        || c == id::HOVEL
        || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
}

/// A card worth less in a hand than the average card of a deck that has a Silver or two in it: junk and Coppers (not the Tunnel, which pays
/// a Gold when it is discarded, but that is the discarder's business, not this test's).
fn is_poor(c: CardId) -> bool {
    is_junk(c) || c == id::COPPER
}

impl SetDefaults for Hinterlands {
    /// Develop and Trader: forced trashes, played only when the trash pays (see `defaults::trashers`).
    fn useless_play(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::DEVELOP => !trashers::develop_pays(s, view, card),
            id::TRADER => !trashers::trader_pays(s, view, card),
            _ => false,
        }
    }

    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            // Jack of All Trades looks at the top card: a poor one is discarded (Tunnel, a Victory card, goes too: it may pay a Gold).
            id::JACK_OF_ALL_TRADES if act == Act::Discard && from == Zone::Revealed => Some(
                iter_cards(choices).find(|&c| is_poor(c)).map(Choice::Card).unwrap_or(Choice::Pass),
            ),
            // Cartographer: poor cards of the top four are discarded; the rest goes back best card on top.
            id::CARTOGRAPHER if act == Act::Discard && from == Zone::Revealed => Some(
                iter_cards(choices).find(|&c| is_poor(c)).map(Choice::Card).unwrap_or(Choice::Pass),
            ),
            // Stables: a Copper for three cards (nothing better to discard).
            id::STABLES if act == Act::Discard => {
                Some(if has_card(choices, id::COPPER) { Choice::Card(id::COPPER) } else { Choice::Pass })
            }
            // Wheelwright: a card that does not matter (dead or poor) for the best listed Action it pays for; nothing if none is listed.
            id::WHEELWRIGHT if act == Act::Discard => Some(choose_wheelwright_discard(s, view, decision, choices)),
            // Witch's Hut: two dead Actions give a Curse. With no Action left to play the Actions in hand are dead (and the discard is free);
            // otherwise dead cards first, a cheap card last.
            id::WITCHS_HUT if act == Act::Discard => Some(choose_hut_discard(view, choices)),
            // Inn: every Action of the discard pile comes back to be drawn sooner (the Ruins are Actions too, and junk).
            id::INN if act == Act::ShuffleIn => Some(
                iter_cards(choices).find(|&c| !cards::is(c, cards::RUINS)).map(Choice::Card).unwrap_or(Choice::Pass),
            ),
            // Scheme: the Actions the strategy wants to see again, at most one terminal.
            id::SCHEME if act == Act::Topdeck => Some(choose_scheme_topdeck(s, view, choices)),
            // Develop: the card that pays best (the gain list's choice at $1 more or $1 less), a card the trash rules want gone first.
            id::DEVELOP if act == Act::Trash && s.legacy_defaults => Some(choose_develop_trash(s, view, choices)),
            id::DEVELOP if act == Act::Trash => trashers::pick(s, view, choices, |s, c| trashers::develop_gain(s, view, c)),
            // Trader: the card whose Silvers (one per $1 it cost) are worth most against losing it.
            id::TRADER if act == Act::Trash => trashers::pick(s, view, choices, |s, c| trashers::trader_gain(s, view, c)),
            _ => None,
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            id::WEAVER => Some(default_weaver(s, view, choices)),
            id::SPICE_MERCHANT => Some(default_spice_merchant(s, view, choices)),
            _ => None,
        }
    }

    fn yes_no(s: &Strategy, _view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        Some(match source {
            // Fool's Gold for a Gold onto the deck: always.
            id::FOOLS_GOLD => Choice::Yes,
            // A Tunnel is always revealed for its Gold.
            id::TUNNEL => Choice::Yes,
            // A Trail or Weaver discarded or gained or trashed: play it.
            id::TRAIL | id::WEAVER => Choice::Yes,
            // Trader: exchange junk, Coppers and cards the gain list does not want for a Silver; never a card the list asks for.
            id::TRADER => {
                let c = decision.subject;
                if is_junk(c) && !(cards::is(c, VICTORY) && s.lists(c)) || c == id::COPPER {
                    Choice::Yes
                } else if s.lists(c) || cards::is(c, VICTORY) {
                    Choice::No
                } else {
                    Choice::Yes
                }
            }
            _ => return None,
        })
    }

    /// Souk gives $7 less $1 per card in the hand: worth playing with a hand of 6 or fewer cards left.
    fn worth_playing(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::SOUK => view.hand().total() <= 6,
            _ => true,
        }
    }
}

/// Wheelwright: discard the card that matters least and pays for the best listed Action costing as much (or less) as it.
fn choose_wheelwright_discard(s: &Strategy, view: &PlayerView, _decision: &Decision, choices: &[Choice]) -> Choice {
    let dead_actions = view.turn().actions == 0;
    let mut best: Option<(usize, i32, CardId)> = None;
    for c in iter_cards(choices) {
        let cheap = Strategy::discard_rank(c) <= 3 || (dead_actions && cards::is(c, ACTION));
        if !cheap {
            continue;
        }
        if let Some(rank) = s.best_listed_action_up_to(view, view.cost(c)).and_then(|a| s.buy_rank_of(a)) {
            let key = (rank, Strategy::discard_rank(c), c);
            if best.map_or(true, |b| key < b) {
                best = Some(key);
            }
        }
    }
    best.map(|(_, _, c)| Choice::Card(c)).unwrap_or(Choice::Pass)
}

/// Witch's Hut: choose a card to discard (two are asked for, one at a time).
fn choose_hut_discard(view: &PlayerView, choices: &[Choice]) -> Choice {
    let dead_actions = view.turn().actions == 0;
    let key = |c: CardId| -> (u8, i32, CardId) {
        let group = if dead_actions && cards::is(c, ACTION) {
            0 // a dead Action: a Curse for the others
        } else if is_junk(c) {
            1
        } else if c == id::COPPER {
            2
        } else {
            3
        };
        (group, Strategy::discard_rank(c), c)
    };
    iter_cards(choices).min_by_key(|&c| key(c)).map(Choice::Card).unwrap_or(choices[0])
}

/// Scheme: put back the Actions the strategy likes (the gain list names them, or they are cantrips), at most one terminal.
fn choose_scheme_topdeck(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    // `choices` are the cards still allowed (the picks so far are gone from the play area and are the top of the deck).
    let want = |c: CardId| s.lists(c) && cards::is(c, ACTION) && !cards::is(c, cards::DURATION);
    let terminal = |c: CardId| cards::def(c).actions == 0;
    let put_terminal = view.deck_known_top().take(3).any(|c| cards::is(c, ACTION) && terminal(c));
    let best = iter_cards(choices)
        .filter(|&c| want(c) && (!terminal(c) || !put_terminal))
        .min_by_key(|&c| (terminal(c), s.buy_rank_of(c).unwrap_or(usize::MAX), c));
    best.map(Choice::Card).unwrap_or(Choice::Pass)
}

/// Develop: the card to trash. A card the trash rules want gone is the fodder (the one whose neighbours in cost the gain list ranks best);
/// failing that, the cheapest card whose neighbours the list wants.
fn choose_develop_trash(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let rank_at = |cost: u8, potion: bool| s.best_gain_rank(view, cost, dominion_engine::state::Filter::Any, true, potion);
    let mut best: Option<(bool, usize, i32, CardId)> = None;
    for c in iter_cards(choices) {
        let cost = view.cost(c);
        let potion = cards::potion_cost(c);
        let up = rank_at(cost + 1, potion);
        let down = if cost > 0 { rank_at(cost - 1, potion) } else { None };
        let Some(r) = [up, down].into_iter().flatten().min() else { continue };
        // Not wanted gone: only fodder that costs less than the gain (the gain list ranks the gain above the card itself).
        let wanted = s.wants_trash(view, c);
        let own = s.gain_rank(view, c).unwrap_or(usize::MAX);
        if !wanted && r >= own {
            continue;
        }
        let key = (!wanted, r, cards::cost(c) as i32, c);
        if best.map_or(true, |b| key < b) {
            best = Some(key);
        }
    }
    match best {
        Some((_, _, _, c)) => Choice::Card(c),
        // Nothing pays: trash the least valuable card (forced).
        None => iter_cards(choices).min_by_key(|&c| (Strategy::card_value(c), c)).map(Choice::Card).unwrap_or(choices[0]),
    }
}

/// Weaver: a card costing up to $4 only when the list ranks one (other than Silver) above Silver; otherwise two Silvers.
fn default_weaver(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let silvers = find_mode(id::WEAVER, choices, is_gain_many);
    let any = find_mode(id::WEAVER, choices, |o| matches!(o, cards::ModeOpt::GainUpToCost(_)));
    let better = match (s.best_gain_rank_up_to(view, 4), s.gain_rank(view, id::SILVER)) {
        (Some(r), Some(silver)) => r < silver,
        (Some(_), None) => true,
        _ => false,
    };
    if better { any.or(silvers) } else { silvers.or(any) }.unwrap_or(choices[0])
}

/// Spice Merchant: the Buy and $2 when they reach a better card on the gain list now; otherwise the cards and the Action.
fn default_spice_merchant(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let cards_mode = find_mode(id::SPICE_MERCHANT, choices, |o| matches!(o, cards::ModeOpt::CardsActions(..)));
    let coins_mode = find_mode(id::SPICE_MERCHANT, choices, |o| matches!(o, cards::ModeOpt::BuysCoins(..)));
    let mut money = view.turn().coins as u32 + view.coffers() as u32;
    let mut potions = view.turn().potions;
    for (c, n) in view.hand().iter().filter(|&(c, _)| view.is_treasure(c)) {
        money += cards::def(c).coins as u32 * n as u32;
        potions += cards::def(c).potions * n;
    }
    let debt = view.debt_of(view.me()) as u32;
    let reach = |m: u32| m.checked_sub(debt).and_then(|m| s.first_reachable_with(view, m, potions));
    let (now, with) = (reach(money), reach(money + 2));
    let rank = |c: Option<CardId>| c.and_then(|c| s.buy_rank_of(c)).unwrap_or(usize::MAX);
    if rank(with) < rank(now) { coins_mode.or(cards_mode) } else { cards_mode.or(coins_mode) }.unwrap_or(choices[0])
}
