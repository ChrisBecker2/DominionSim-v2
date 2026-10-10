//! Empires bot defaults (see `docs/empires-plan.md`, section 4): the cards' own decisions, Tax's pile, Mountain Pass bids. The Debt payment itself
//! (pay as soon as the Buy phase allows; the exceptions are searched) is in `strategy.rs` (`buy_decision`) and `eval.rs`. Stated rules come first.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ModeOpt, ACTION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::state::{Act, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Empires;

/// The most Debt a bid for Mountain Pass's 8 VP is worth to the bots: Debt is $ lost from a later hand, and 8 VP is about a Province.
const BID_WORTH: u8 = 5;

impl SetDefaults for Empires {
    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            // Archive: the card to take into the hand now or at the start of a turn.
            id::ARCHIVE if from == Zone::Archive => Some(archive_pick(view, choices)),
            // Crown in the Buy phase: the Treasure worth the most played twice.
            id::CROWN if act == Act::Play && view.phase() == dominion_engine::state::Phase::Buy => Some(crown_treasure_pick(view, choices)),
            // Change: trash the card that unlocks the best gain-list card costing more (junk first, then the cheapest).
            id::CHANGE if act == Act::Trash => Some(change_pick(s, view, choices)),
            // Gladiator: reveal the card the player to the left is least likely to have a copy of.
            id::GLADIATOR if act == Act::Reveal => Some(gladiator_reveal(view, choices)),
            // Encampment and Legionary reveal the card they ask for (it costs nothing).
            id::ENCAMPMENT | id::LEGIONARY if act == Act::Reveal => Some(iter_cards(choices).next().map(Choice::Card).unwrap_or(Choice::Pass)),
            // Annex: the (up to 5) cards that stay out of the shuffle are the dead ones.
            id::ANNEX => Some(annex_keep(view, choices)),
            // Advance: trash the Action in hand the gain list improves on most.
            id::ADVANCE => Some(advance_pick(s, view, choices)),
            // Ritual: the card whose cost in VP tokens is worth more than the card.
            id::RITUAL => Some(ritual_pick(s, view, choices)),
            // Salt the Earth: the Victory card worth the most points, out of the Supply.
            id::SALT_THE_EARTH => Some(iter_cards(choices).max_by_key(|&c| (cards::def(c).vp, cards::cost(c))).map(Choice::Card).unwrap_or(Choice::Pass)),
            // Small Castle: trash a cheap Castle (Humble, Crumbling) from the hand before the Small Castle itself.
            id::SMALL_CASTLE if act == Act::Trash => Some(iter_cards(choices).min_by_key(|&c| (cards::cost(c), c)).map(Choice::Card).unwrap_or(Choice::Pass)),
            // Opulent Castle: every Victory card in the hand that does nothing else (an Action-Victory card is kept to be played).
            id::OPULENT_CASTLE if act == Act::Discard => Some(
                iter_cards(choices)
                    .find(|&c| !cards::is(c, ACTION) && !cards::is(c, TREASURE))
                    .map(Choice::Card)
                    .unwrap_or(Choice::Pass),
            ),
            // Forced single picks: Catapult, Sacrifice and Temple trash by the trash rules (the generic policy), with Rocks first.
            id::CATAPULT | id::SACRIFICE | id::TEMPLE if act == Act::Trash && iter_cards(choices).any(|c| c == id::ROCKS) => Some(Choice::Card(id::ROCKS)),
            _ => {
                let _ = decision;
                None
            }
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            // Wild Hunt: take the VP tokens once there are enough of them (and Estates are left), else draw.
            id::WILD_HUNT => {
                let take = find_mode(card, choices, |o| matches!(o, ModeOpt::GainTakeVp(_)));
                let draw = find_mode(card, choices, |o| matches!(o, ModeOpt::CardsGather(..)));
                match (take, draw) {
                    (Some(t), Some(d)) => Some(if view.pile_vp(id::WILD_HUNT) >= 4 && view.supply(id::ESTATE) > 0 { t } else { d }),
                    (t, d) => t.or(d),
                }
            }
            // Charm: +1 Buy and +$2, unless two cards of one cost are both wanted.
            id::CHARM => {
                let money = find_mode(card, choices, |o| matches!(o, ModeOpt::BuysCoins(..)));
                let double = find_mode(card, choices, |o| matches!(o, ModeOpt::NextGainDouble));
                match (money, double) {
                    (Some(m), Some(d)) => Some(if twin_cost_wanted(s, view) { d } else { m }),
                    (m, d) => m.or(d),
                }
            }
            // Sprawling Castle: 3 Estates are 3 VP of junk in the deck; the Duchy is 3 VP in one card.
            id::SPRAWLING_CASTLE => find_mode(card, choices, |o| matches!(o, ModeOpt::Gain(..))),
            _ => None,
        }
    }

    fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        Some(match (decision.kind, source) {
            // Settlers' Copper and Bustling Village's Settlers from the discard pile: always worth taking.
            (DecisionKind::YesNo { act: Act::ToHand }, _) => Choice::Yes,
            // Engineer: trash it for a second gain when the strategy does not build with it (or the game is nearly over) and a listed card is there.
            (DecisionKind::YesNo { act: Act::Trash }, id::ENGINEER) => {
                let wanted = s.best_gain_rank_up_to(view, 4).is_some();
                let late = view.supply(id::PROVINCE) <= 3 || view.empty_piles() >= 2;
                if wanted && (late || s.wants_trash(view, id::ENGINEER) || !s.lists(id::ENGINEER)) { Choice::Yes } else { Choice::No }
            }
            // Small Castle: trash it for the top Castle, unless a cheap Castle in the hand can go instead.
            (DecisionKind::YesNo { act: Act::Trash }, id::SMALL_CASTLE) => {
                let cheap_in_hand = view.hand().iter().any(|(c, _)| cards::is_castle(c) && cards::cost(c) < cards::cost(id::SMALL_CASTLE));
                if view.castle_top().is_some() && !cheap_in_hand { Choice::Yes } else { Choice::No }
            }
            // Gladiator: the player to the left reveals a copy whenever they can.
            (DecisionKind::YesNo { act: Act::Reveal }, id::GLADIATOR) => Choice::Yes,
            _ => return None,
        })
    }

    /// Archive's draw comes after the plain bonuses and before the plays.
    fn start_order_group(card: CardId) -> Option<u8> {
        (card == id::ARCHIVE).then_some(1)
    }

    /// Crown needs something to play twice; Overlord and Small Castle are judged by their cost and decisions.
    fn worth_playing(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::CROWN => view.hand().iter().any(|(c, _)| c != id::CROWN && cards::is(c, ACTION)),
            // Small Castle is a Victory card first; playing it trashes it (or a Castle) for the top Castle only when that is a gain.
            id::SMALL_CASTLE => view.castle_top().is_some(),
            // Opulent Castle pays $2 per Victory card discarded.
            id::OPULENT_CASTLE => view.hand().iter().any(|(c, _)| cards::is(c, VICTORY) && !cards::is(c, ACTION)),
            _ => true,
        }
    }

    /// Fortune doubles the $ in hand: it goes last among the Treasures (after the Coffers, which the Treasure decision offers first).
    fn plays_last_rank(card: CardId) -> Option<u8> {
        (card == id::FORTUNE).then_some(2)
    }
}

/// Whether two differently named cards of one cost are both on the gain list at an affordable price: Charm's second option then buys an extra.
fn twin_cost_wanted(s: &Strategy, view: &PlayerView) -> bool {
    let money = (view.turn().coins as u32 + view.coffers() as u32).min(14) as u8;
    (1..=money).any(|cost| {
        let mut n = 0;
        for c in view.supply_cards() {
            if view.supply(c) > 0 && view.cost(c) == cost && cards::debt_cost(c) == 0 && !cards::potion_cost(c) && s.allows_buy(view, c) {
                n += 1;
            }
        }
        n >= 2 && view.turn().buys > 0
    })
}

/// Change: for each card of the hand, the best listed gain costing more than it; the card with the best such gain, ties to the one the trash
/// rules want gone and then the least valuable. A hand with no card that unlocks a listed gain trashes its worst card.
fn change_pick(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    iter_cards(choices)
        .min_by_key(|&c| {
            let rank = s.best_gain_rank_more(view, view.cost(c)).unwrap_or(usize::MAX);
            (rank, !s.wants_trash(view, c), Strategy::card_value(c), c)
        })
        .map(Choice::Card)
        .unwrap_or(Choice::Pass)
}

/// Archive: now, an Action when an Action is left to play, else the best Treasure; the cards come out one a turn, so the rest is taken later.
fn archive_pick(view: &PlayerView, choices: &[Choice]) -> Choice {
    let actions_left = view.is_my_turn() && view.phase() == dominion_engine::state::Phase::Action && view.turn().actions > 0;
    let value = |c: CardId| -> i32 {
        let base = if cards::is(c, VICTORY) && !cards::is(c, ACTION) { -5 } else { Strategy::card_value(c) };
        if cards::is(c, ACTION) && actions_left { base + 10 } else { base }
    };
    iter_cards(choices).max_by_key(|&c| (value(c), c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

/// Crown in the Buy phase: the Treasure with the most $ (Capital's $6 and its Debt twice, Gold's $3, ...).
fn crown_treasure_pick(view: &PlayerView, choices: &[Choice]) -> Choice {
    let _ = view;
    iter_cards(choices).max_by_key(|&c| (cards::def(c).coins, c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

/// Gladiator: of the cards in the hand, the one the left player has the fewest of in their whole deck (they may reveal a copy only from their hand).
fn gladiator_reveal(view: &PlayerView, choices: &[Choice]) -> Choice {
    let left = (view.me() + 1) % view.num_players();
    let theirs = view.cards_of(left);
    // A Copper revealed is no loss; prefer cards the left player has none of, then the cheapest.
    iter_cards(choices)
        .min_by_key(|&c| (theirs.get(c), cards::cost(c), c))
        .map(Choice::Card)
        .unwrap_or(Choice::Pass)
}

/// Annex: up to 5 cards of the discard pile stay out of the shuffle: Curses, Ruins, Victory cards and junk Shelters first, then Coppers.
fn annex_keep(view: &PlayerView, choices: &[Choice]) -> Choice {
    let _ = view;
    let junk = |c: CardId| {
        c == id::CURSE || cards::is(c, cards::RUINS) || c == id::HOVEL || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
    };
    iter_cards(choices).find(|&c| junk(c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

/// Advance: trash the cheapest Action in the hand when the gain list names an Action costing up to $6 that is worth more, else nothing.
fn advance_pick(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let Some(best) = s.best_listed_action_up_to(view, 6) else { return Choice::Pass };
    iter_cards(choices)
        .filter(|&c| cards::cost(c) < cards::cost(best) && !s.lists(c))
        .min_by_key(|&c| (cards::cost(c), c))
        .map(Choice::Card)
        .unwrap_or(Choice::Pass)
}

/// Ritual: trash the card whose cost in VP tokens beats what the card is worth to the deck (a Province is 6 VP for an 8: +2, a Gold is a
/// Gold); never a card the strategy wants.
fn ritual_pick(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let worth = |c: CardId| -> i32 {
        let keep = if cards::is(c, VICTORY) && !cards::is(c, ACTION) {
            cards::def(c).vp as i32
        } else if c == id::CURSE {
            -1
        } else if s.lists(c) {
            Strategy::card_value(c).max(8)
        } else {
            Strategy::card_value(c) / 2
        };
        cards::cost(c) as i32 - keep
    };
    let _ = view;
    iter_cards(choices).max_by_key(|&c| (worth(c), cards::cost(c), c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

/// Tax: 2 Debt on the pile the others are likeliest to buy from and this player does not need: the pile (not in the gain list) that the
/// opponents own the most of, else the Province pile.
pub(super) fn choose_tax_pile(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let me = view.me();
    let score = |c: CardId| -> i32 {
        let theirs: u32 = (0..view.num_players()).filter(|&q| q != me).map(|q| view.cards_of(q).get(c) as u32).sum();
        let listed = s.lists(c) as i32;
        (theirs as i32) * 2 - listed * 20 + if c == id::PROVINCE { 6 } else { 0 }
    };
    iter_cards(choices).max_by_key(|&c| (score(c), c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

/// A Mountain Pass bid: one more than the best so far while that is worth it for 8 VP, or nothing. The first bidder opens modestly.
pub(super) fn choose_bid(view: &PlayerView, min: u8, choices: &[Choice]) -> Choice {
    let _ = view;
    let bid = if min == 1 { 2 } else { min };
    if bid <= BID_WORTH && choices.contains(&Choice::Bid(bid)) {
        Choice::Bid(bid)
    } else if min <= BID_WORTH && choices.contains(&Choice::Bid(min)) {
        Choice::Bid(min)
    } else {
        Choice::Bid(0)
    }
}
