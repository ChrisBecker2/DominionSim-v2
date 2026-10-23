//! Nocturne bot defaults (see `docs/nocturne-plan.md`, section 5): which Night card to play when no rule says, which Boon to take from a pool,
//! whether to receive a Boon now or later, whether to exchange a gained card for a Changeling, and the small choices inside Boons and cards.
//! Stated `[[play]]` rules (Night cards are in the same list as Actions) and `[[exchange]]` rules come first.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::state::{Act, Phase, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Nocturne;

/// A card that does nothing for a deck: Curse, Ruins, Victory cards that are not Actions or Treasures, Shelters that are junk.
fn is_junk(c: CardId) -> bool {
    c == id::CURSE
        || cards::is(c, cards::RUINS)
        || c == id::HOVEL
        || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
}

/// A Boon is received now (rather than at the start of the next turn) when it does something in what is left of this turn and the hand: the ones
/// that need a fresh hand wait (a Treasure to discard, cards to draw).
fn better_now(boon: CardId) -> bool {
    !matches!(boon, id::THE_EARTHS_GIFT | id::THE_SEAS_GIFT | id::THE_WINDS_GIFT)
}

/// The order in which the Fool's Boons are received (the ones that give an Action and cards first, those that act on the hand last), and the
/// value of a Boon to pick from Druid's three.
fn fool_rank(boon: CardId) -> usize {
    const ORDER: [CardId; 12] = [
        id::THE_FIELDS_GIFT, id::THE_SEAS_GIFT, id::THE_SUNS_GIFT, id::THE_MOONS_GIFT, id::THE_RIVERS_GIFT, id::THE_WINDS_GIFT,
        id::THE_SKYS_GIFT, id::THE_EARTHS_GIFT, id::THE_FLAMES_GIFT, id::THE_MOUNTAINS_GIFT, id::THE_SWAMPS_GIFT, id::THE_FORESTS_GIFT,
    ];
    ORDER.iter().position(|&b| b == boon).unwrap_or(usize::MAX)
}

fn druid_rank(boon: CardId) -> usize {
    const ORDER: [CardId; 12] = [
        id::THE_EARTHS_GIFT, id::THE_MOUNTAINS_GIFT, id::THE_SKYS_GIFT, id::THE_SWAMPS_GIFT, id::THE_FLAMES_GIFT, id::THE_FIELDS_GIFT,
        id::THE_FORESTS_GIFT, id::THE_SEAS_GIFT, id::THE_MOONS_GIFT, id::THE_RIVERS_GIFT, id::THE_SUNS_GIFT, id::THE_WINDS_GIFT,
    ];
    ORDER.iter().position(|&b| b == boon).unwrap_or(usize::MAX)
}

impl SetDefaults for Nocturne {
    fn select(s: &Strategy, view: &PlayerView, _decision: &Decision, source: CardId, act: Act, _from: Zone, choices: &[Choice]) -> Option<Choice> {
        let hand_junk = || view.hand().iter().filter(|&(c, _)| is_junk(c)).map(|(_, n)| n as u32).sum::<u32>();
        // The lowest-valued offered card (the cards that matter least to the deck first).
        let worst = || iter_cards(choices).min_by_key(|&c| (Strategy::discard_rank(c), c)).map(Choice::Card);
        match source {
            // Lost in the Woods: a dead card for a Boon, otherwise keep the hand.
            id::LOST_IN_THE_WOODS if act == Act::Discard => {
                Some(iter_cards(choices).filter(|&c| is_junk(c)).min_by_key(|&c| (Strategy::discard_rank(c), c)).map(Choice::Card).unwrap_or(Choice::Pass))
            }
            // Secret Cave: discarding 3 is for a hand of dead cards (all or nothing: once started the rest follow).
            id::SECRET_CAVE if act == Act::Discard => {
                if !choices.contains(&Choice::Pass) || (view.hand().total() >= 3 && hand_junk() >= 3) {
                    worst()
                } else {
                    Some(Choice::Pass)
                }
            }
            // The Sky's Gift: three cards for a Gold. Free once the Buy phase is over (the hand is spent); in the Action phase only dead cards.
            id::THE_SKYS_GIFT if act == Act::Discard => {
                // Spent: the Night phase, or the Buy phase with no Treasure left to play (an Idol's Boon comes before the other Treasures are played).
                let spent = view.phase() == Phase::Night
                    || (view.phase() == Phase::Buy && !view.hand().iter().any(|(c, _)| cards::is(c, TREASURE)));
                if !choices.contains(&Choice::Pass) || (view.hand().total() >= 3 && (spent || hand_junk() >= 3)) {
                    worst()
                } else {
                    Some(Choice::Pass)
                }
            }
            // The Earth's Gift: a Treasure for a card up to $4 when the gain list wants one; the least valuable Treasure.
            id::THE_EARTHS_GIFT if act == Act::Discard => {
                if s.best_gain_rank_up_to(view, 4).is_some() {
                    Some(iter_cards(choices).min_by_key(|&c| (cards::def(c).coins, c)).map(Choice::Card).unwrap_or(Choice::Pass))
                } else {
                    Some(Choice::Pass)
                }
            }
            // Changeling: a copy of the card in play that the gain list likes best, else the costliest.
            id::CHANGELING if act == Act::Reveal => Some(
                iter_cards(choices)
                    .min_by_key(|&c| (s.buy_rank_of(c).unwrap_or(usize::MAX), std::cmp::Reverse(cards::cost(c)), c))
                    .map(Choice::Card)
                    .unwrap_or(Choice::Pass),
            ),
            // Crypt: the Treasures worth $2 or more (Silver, Gold, Platinum...) go under it, the Coppers stay in the deck.
            id::CRYPT if act == Act::Archive => Some(
                iter_cards(choices).filter(|&c| cards::def(c).coins >= 2 || cards::cost(c) >= 3).min().map(Choice::Card).unwrap_or(Choice::Pass),
            ),
            // Haunted Mirror: any Action card is worth a Ghost; discard the cheapest.
            id::HAUNTED_MIRROR if act == Act::Discard => {
                Some(iter_cards(choices).min_by_key(|&c| (cards::cost(c), c)).map(Choice::Card).unwrap_or(Choice::Pass))
            }
            _ => None,
        }
    }

    fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        Some(match source {
            // Sacred Grove's Boon for the other players, Blessed Village's "now or at the start of your next turn".
            id::BLESSED_VILLAGE if matches!(decision.kind, DecisionKind::YesNo { act: Act::Reveal }) => {
                if better_now(decision.subject) { Choice::Yes } else { Choice::No }
            }
            id::SACRED_GROVE => Choice::Yes,
            // Pixie: trash it for the Boon twice when the Boon is one worth two of.
            id::PIXIE => {
                let b = decision.target;
                if b != 0 && !matches!(b, id::THE_FORESTS_GIFT | id::THE_FIELDS_GIFT) { Choice::Yes } else { Choice::No }
            }
            // A Faithful Hound is set aside whenever it can be.
            id::FAITHFUL_HOUND => Choice::Yes,
            // Tracker: a gained card goes onto the deck when it is worth drawing again soon.
            id::TRACKER => {
                let c = decision.subject;
                if is_junk(c) || c == id::COPPER { Choice::No } else { Choice::Yes }
            }
            // Changeling: exchange a card that is low on the gain list (or unlisted), never a Victory card or a top priority.
            id::CHANGELING => {
                let c = decision.subject;
                let n = s.gain_list_len();
                let low = match s.gain_rank(view, c) {
                    None => true,
                    Some(r) => r * 3 >= n * 2,
                };
                if low && !cards::is(c, VICTORY) { Choice::Yes } else { Choice::No }
            }
            _ => return None,
        })
    }

    /// Boons waiting at the start of the turn and Lost in the Woods: they act on the hand, after the cards are drawn and put into it.
    fn start_order_group(card: CardId) -> Option<u8> {
        match card {
            id::LOST_IN_THE_WOODS => Some(4),
            c if cards::is_boon(c) => Some(4),
            id::COBBLER | id::CRYPT => Some(0),
            _ => None,
        }
    }

    /// Whether the Action `card` is worth playing by default.
    fn worth_playing(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            // The Imp and Conclave need an Action to play to be more than weak; Zombie Apprentice an Action to trash.
            id::ZOMBIE_APPRENTICE => view.hand().iter().any(|(c, _)| c != card && cards::is(c, ACTION)),
            _ => true,
        }
    }
}

/// The Night card to play by rule order alone when no `[[play]]` rule names one: the cards with no decision first, then the gainers, then the
/// trashers (a trash after the gains it counts), the deck changers last; the lowest id first within a group.
pub(super) fn rule_night(_s: &Strategy, _view: &PlayerView, choices: &[Choice]) -> Choice {
    let group = |c: CardId| -> u8 {
        match c {
            id::DEVILS_WORKSHOP | id::CHANGELING | id::VAMPIRE => 1,
            id::MONASTERY | id::EXORCIST | id::BAT => 2,
            id::NIGHT_WATCHMAN | id::GHOST => 3,
            _ => 0,
        }
    };
    iter_cards(choices).min_by_key(|&c| (group(c), c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

/// A Boon to pick from a pool: the Fool receives all of them (in a good order), Druid's pick is the best of its three.
pub(super) fn pick_boon(_s: &Strategy, _view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
    let druid = decision.source == Some(id::DRUID);
    iter_cards(choices)
        .min_by_key(|&c| if druid { druid_rank(c) } else { fool_rank(c) })
        .map(Choice::Card)
        .unwrap_or(Choice::Pass)
}
