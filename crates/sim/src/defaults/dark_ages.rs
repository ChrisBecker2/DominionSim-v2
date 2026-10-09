//! Dark Ages bot defaults (see `docs/dark-ages-plan.md`, section 3): Rebuild's name, Procession and Counterfeit, Count, Graverobber,
//! Catacombs, Squire, Death Cart's trash of itself, Urchin's Mercenary, Pillage's victim, Scavenger and Survivors, and which Actions
//! are worth playing (Poor House).

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ModeOpt, ACTION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Filter, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct DarkAges;

/// A card with no use in a deck: Curse, Ruins, Hovel, Victory cards (Overgrown Estate and the Knights' Dame Josephine too, being worth
/// little more than a stop card).
fn is_junk(c: CardId) -> bool {
    c == id::CURSE
        || cards::is(c, cards::RUINS)
        || c == id::HOVEL
        || c == id::OVERGROWN_ESTATE
        || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
}

impl SetDefaults for DarkAges {
    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            // Procession: the Action whose upgrade (exactly $1 more) the gain list ranks highest; a card that cannot be upgraded to
            // anything listed (a good Action) is kept.
            id::PROCESSION if act == Act::Play => Some(choose_procession_card(s, view, choices)),
            // Counterfeit: a Spoils for $6, else a Copper the trash rules want gone (it is played twice and thinned), else nothing.
            id::COUNTERFEIT if act == Act::Play => Some(
                if has_card(choices, id::SPOILS) {
                    Choice::Card(id::SPOILS)
                } else if has_card(choices, id::COPPER) && s.wants_trash(view, id::COPPER) {
                    Choice::Card(id::COPPER)
                } else {
                    Choice::Pass
                },
            ),
            // Pillage: the attacker picks the best card of the victim's hand to discard.
            id::PILLAGE if act == Act::Discard && decision.player != decision.for_player => {
                Some(iter_cards(choices).max_by_key(|&c| (Strategy::card_value(c), c)).map(Choice::Card).unwrap_or(choices[0]))
            }
            // Scavenger: the card from the discard pile to put on the deck (mandatory): the best by the gain list, else the costliest.
            id::SCAVENGER if act == Act::Topdeck && from == Zone::Discard => Some(
                s.best_by_buy_rank(choices)
                    .or_else(|| iter_cards(choices).max_by_key(|&c| (Strategy::card_value(c), c)))
                    .map(Choice::Card)
                    .unwrap_or(choices[0]),
            ),
            // Death Cart: an Action from the hand only if it is a dead one (Ruins) or the trash rules want it gone.
            id::DEATH_CART if act == Act::Trash && from == Zone::Hand => Some(
                iter_cards(choices)
                    .filter(|&c| cards::is(c, cards::RUINS) || s.wants_trash(view, c))
                    .min_by_key(|&c| (cards::cost(c), c))
                    .map(Choice::Card)
                    .unwrap_or(Choice::Pass),
            ),
            _ => None,
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            id::SQUIRE => Some(default_squire(s, view, choices)),
            id::COUNT => Some(default_count(s, view, choices)),
            id::GRAVEROBBER => Some(default_graverobber(s, view, choices)),
            id::CATACOMBS => Some(default_catacombs(view, choices)),
            // A Duchy is the sure 3 VP, and cannot be a pile that is gone for the other piles' sake: only the Estates when the Duchies
            // are gone.
            id::HUNTING_GROUNDS => {
                let duchy = find_mode(id::HUNTING_GROUNDS, choices, is_gain);
                let estates = find_mode(id::HUNTING_GROUNDS, choices, is_gain_many);
                Some(if view.supply(id::DUCHY) > 0 { duchy } else { estates.or(duchy) }.unwrap_or(choices[0]))
            }
            _ => None,
        }
    }

    /// Rebuild names the Victory card the bot least wants trashed: the costliest one of the game (Province, or Colony), so that the cards
    /// it finds are the cheaper ones to upgrade.
    fn name(_s: &Strategy, _view: &PlayerView, _decision: &Decision, source: CardId, choices: &[Choice]) -> Option<Choice> {
        if source != id::REBUILD {
            return None;
        }
        iter_cards(choices)
            .filter(|&c| cards::is(c, VICTORY) && !cards::is(c, ACTION))
            .max_by_key(|&c| (cards::cost(c), c))
            .map(Choice::Card)
            .or(Some(choices[0]))
    }

    /// The yes/no decisions of this set's cards.
    fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        Some(match source {
            // Trash the Death Cart itself for the +$5 (its Ruins have come already).
            id::DEATH_CART => Choice::Yes,
            // Urchin: a Mercenary is better than the Urchin, whenever one is to be had.
            id::URCHIN => if view.supply(id::MERCENARY) > 0 { Choice::Yes } else { Choice::No },
            // Scavenger: put the deck into the discard pile if what is in the discard pile is better than what is left of the deck.
            id::SCAVENGER => {
                let avg = |c: &dominion_engine::Counts| {
                    let n = c.total().max(1) as f64;
                    c.iter().map(|(card, k)| Strategy::card_value(card) as f64 * k as f64).sum::<f64>() / n
                };
                let deck = view.deck();
                let d = *view.discard();
                if d.total() > 0 && avg(&d) > avg(&deck) { Choice::Yes } else { Choice::No }
            }
            // Survivors: discard both unless the strategy likes either card enough to see it again.
            id::SURVIVORS => {
                let top: Vec<CardId> = view.revealed().iter().flat_map(|(c, n)| std::iter::repeat(c).take(n as usize)).collect();
                if top.iter().all(|&c| is_junk(c) || c == id::COPPER) { Choice::Yes } else { Choice::No }
            }
            // Hovel: trash it when a Victory card is gained (the trash rules list it).
            id::HOVEL => if s.wants_trash(view, id::HOVEL) { Choice::Yes } else { Choice::No },
            // Market Square: always the Gold.
            id::MARKET_SQUARE => Choice::Yes,
            _ => {
                let _ = decision;
                return None;
            }
        })
    }

    /// Poor House gives $4 less $1 per Treasure in the hand: not worth playing with four or more Treasures (it can only cost coins).
    fn worth_playing(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::POOR_HOUSE => view.hand().iter().filter(|&(c, _)| view.is_treasure(c)).map(|(_, n)| n as u32).sum::<u32>() < 4,
            _ => true,
        }
    }
}

/// Procession: of the Actions offered, the one whose upgrade is first in the gain list (a card costing exactly $1 more); dead Actions
/// (Ruins) are the best fodder; a good Action with no listed upgrade is left alone.
fn choose_procession_card(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let mut best: Option<(usize, u8, CardId)> = None;
    for c in iter_cards(choices) {
        let rank = s.best_gain_rank(view, view.cost(c) + 1, Filter::Action, true, cards::potion_cost(c));
        if let Some(r) = rank {
            // Cheaper fodder first among equal gains: the worse card to lose.
            let key = (r, view.cost(c), c);
            if best.map_or(true, |b| key < b) {
                best = Some(key);
            }
        }
    }
    match best {
        Some((_, _, c)) => Choice::Card(c),
        None => Choice::Pass,
    }
}

/// Squire: +2 Actions when Actions in hand are stuck, else a Silver while the deck is thin, else +2 Buys when there is spare coin.
fn default_squire(_s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let stuck = view.turn().actions == 0 && view.hand().any_type(ACTION);
    let order: [fn(&ModeOpt) -> bool; 3] = if stuck { [is_actions, is_gain, is_buys] } else { [is_gain, is_buys, is_actions] };
    for pred in order {
        if let Some(c) = find_mode(id::SQUIRE, choices, pred) {
            return c;
        }
    }
    choices[0]
}

/// Count: the first choice discards two dead cards when the hand has them, else puts nothing back and takes a Copper (coins matter
/// less than a thicker deck of Coppers? no: a Copper is the mildest); the second is +$3, or a Duchy late in the game, or the
/// whole hand when it is all dead cards.
fn default_count(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let hand = view.hand();
    let has = |m: &ModeOpt| cards::modes(id::COUNT).iter().position(|o| o == m).map(|i| Choice::Mode(i as u8)).filter(|c| choices.contains(c));
    let junk: u32 = hand.iter().filter(|&(c, _)| is_junk(c)).map(|(_, n)| n as u32).sum();
    let first = [ModeOpt::DiscardFromHand(2), ModeOpt::TopdeckFromHand(1), ModeOpt::Gain(id::COPPER, dominion_engine::state::Dest::Discard)];
    if first.iter().any(|m| has(m).is_some()) {
        if junk >= 2 {
            return has(&first[0]).unwrap();
        }
        // Nothing dead to shed: a Copper gained is the mild option.
        return has(&first[2]).unwrap();
    }
    let all_dead = hand.total() > 0 && hand.iter().all(|(c, _)| is_junk(c));
    let late = view.supply(id::PROVINCE) <= 4;
    if all_dead && hand.total() >= 3 {
        return has(&ModeOpt::TrashHand).unwrap_or(choices[0]);
    }
    if late && s.gain_rank(view, id::DUCHY).is_some() {
        return has(&ModeOpt::Gain(id::DUCHY, dominion_engine::state::Dest::Discard)).unwrap_or(choices[0]);
    }
    has(&ModeOpt::Coins(3)).unwrap_or(choices[0])
}

/// Graverobber: the card from the trash if one costing $3 to $6 is there that the strategy lists (or costs $5 or more); else upgrade an
/// Action in the hand if one is there; else the trash.
fn default_graverobber(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let from_trash = find_mode(id::GRAVEROBBER, choices, is_gain_trash_to);
    let upgrade = find_mode(id::GRAVEROBBER, choices, is_trash_gain);
    let wanted = view.trash().iter().any(|(c, _)| Filter::CostRange(3, 6).matches(c) && (s.gain_rank(view, c).is_some() || cards::cost(c) >= 5));
    if wanted {
        return from_trash.unwrap_or(choices[0]);
    }
    let junk_action = view.hand().iter().any(|(c, _)| cards::is(c, ACTION) && (cards::is(c, cards::RUINS) || s.wants_trash(view, c)));
    if junk_action {
        return upgrade.unwrap_or(choices[0]);
    }
    from_trash.or(upgrade).unwrap_or(choices[0])
}

/// Catacombs: take the three cards looked at unless at least two of them are dead; then discard them and draw three more.
fn default_catacombs(view: &PlayerView, choices: &[Choice]) -> Choice {
    let dead: u32 = view.revealed().iter().filter(|&(c, _)| is_junk(c) || c == id::COPPER).map(|(_, n)| n as u32).sum();
    let want = if dead >= 2 { is_discard_reveal_draw as fn(&ModeOpt) -> bool } else { is_take_revealed };
    find_mode(id::CATACOMBS, choices, want).unwrap_or(choices[0])
}

fn is_gain_trash_to(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::GainFromTrashTo(..))
}
fn is_trash_gain(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::TrashGain { .. })
}
fn is_take_revealed(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::TakeRevealed)
}
fn is_discard_reveal_draw(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::DiscardRevealedDraw(_))
}
