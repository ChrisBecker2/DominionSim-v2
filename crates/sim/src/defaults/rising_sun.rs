//! Rising Sun bot defaults (rules in `docs/rising-sun-plan.md`): Kitsune's and Sickness's options, Gold Mine and Snake Witch, how many cards
//! Asceticism trashes, the order of the Prophecies' start-of-turn effects, and the Debt cards that are not worth playing late. Change keeps the
//! Empires policy (it was the first Debt card).

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, VICTORY};
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::state::{Act, Zone};

use super::empires::Empires;
use super::draws;
use super::trashers;
use super::SetDefaults;
use crate::strategy::*;

pub(super) struct RisingSun;

impl SetDefaults for RisingSun {
    /// Tanuki: a forced upgrade, played only when it pays (see `defaults::trashers`); Root Cellar only when its cards pay the Debt (`defaults::draws`).
    fn useless_play(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::TANUKI => !trashers::upgrade_pays(s, view, card, 2, false),
            id::ROOT_CELLAR => !draws::root_cellar_pays(s, view),
            _ => false,
        }
    }

    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            id::CHANGE => Empires::select(s, view, decision, source, act, from, choices),
            _ => None,
        }
    }

    fn default_mode(_s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            // Kitsune (Actions, $2, Curses, Silver): the Curses while there are some, then +2 Actions when a terminal waits in the hand, else $2.
            id::KITSUNE => {
                let curse = find_mode(card, choices, |o| matches!(o, ModeOpt::CurseOthers)).filter(|_| view.supply(id::CURSE) > 0);
                let terminal = view.hand().iter().any(|(c, _)| cards::is(c, ACTION) && cards::def(c).actions == 0);
                let actions = find_mode(card, choices, |o| matches!(o, ModeOpt::Actions(_)));
                let coins = find_mode(card, choices, |o| matches!(o, ModeOpt::Coins(_)));
                let silver = find_mode(card, choices, |o| matches!(o, ModeOpt::Gain(..)));
                let first = if terminal { actions.or(coins) } else { coins.or(actions) };
                curse.or(first).or(coins).or(actions).or(silver)
            }
            // Sickness: three cards discarded when they are junk (the three worst cards of the hand are worth no more than $3 together and are
            // no Action), else the Curse goes onto the deck.
            id::SICKNESS => {
                let mut hand: Vec<i32> = view
                    .hand()
                    .iter()
                    .flat_map(|(c, n)| std::iter::repeat(sickness_value(view, c)).take(n as usize))
                    .collect();
                hand.sort_unstable();
                let worst3: i32 = hand.iter().take(3).sum();
                let discard = find_mode(card, choices, |o| matches!(o, ModeOpt::DiscardFromHand(_)));
                let curse = find_mode(card, choices, |o| matches!(o, ModeOpt::Gain(..)));
                if hand.len() <= 3 || worst3 <= 3 { discard.or(curse) } else { curse.or(discard) }
            }
            _ => None,
        }
    }

    fn yes_no(_s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        match (decision.kind, source) {
            // Snake Witch: return it to give Curses (only asked when the hand has no duplicates).
            (DecisionKind::YesNo { .. }, id::SNAKE_WITCH) => Some(if view.supply(id::CURSE) > 0 { Choice::Yes } else { Choice::No }),
            // Gold Mine: a Gold for 4 Debt, unless the game is nearly over (the Debt would block the last buys).
            (DecisionKind::YesNo { .. }, id::GOLD_MINE) => {
                let late = view.supply(id::PROVINCE) <= 3 || view.empty_piles() >= 2;
                Some(if late { Choice::No } else { Choice::Yes })
            }
            _ => None,
        }
    }

    /// Asceticism: a $ for each card of the hand the trash rules want gone (Curses, Estates, whatever the strategy trashes), as far as the $ go.
    fn overpay(s: &Strategy, view: &PlayerView, card: CardId, max_coins: u8, _potion: bool) -> Option<Choice> {
        if card != id::ASCETICISM {
            return None;
        }
        let junk: u32 = view.hand().iter().filter(|&(c, _)| s.wants_trash(view, c)).map(|(_, n)| n as u32).sum();
        Some(Choice::Overpay(junk.min(max_coins as u32) as u8))
    }

    /// The Prophecies that act at the start of the turn: the set-aside hand first, then the gained Action, the plays, and Sickness last (it acts
    /// on the whole hand).
    fn start_order_group(card: CardId) -> Option<u8> {
        match card {
            id::BIDING_TIME => Some(0),
            id::KIND_EMPEROR => Some(1),
            id::RAPID_EXPANSION | id::RIVERBOAT => Some(3),
            id::SICKNESS => Some(4),
            _ => None,
        }
    }

    fn start_order_play(card: CardId) -> bool {
        matches!(card, id::RAPID_EXPANSION | id::RIVERBOAT)
    }

    /// The Debt cards are not worth playing when the game is nearly over: the Debt would stand between the player and the last buys. Daimyo, which
    /// replays the next Action, needs one in the hand. Treasures played in the Action phase (Enlightenment) are worth it for the Coppers only.
    fn worth_playing(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::ROOT_CELLAR | id::IMPERIAL_ENVOY | id::LITTER | id::CRAFTSMAN | id::GOLD_MINE => {
                !(view.supply(id::PROVINCE) <= 2 || view.empty_piles() >= 2)
            }
            id::DAIMYO => view.hand().iter().any(|(c, _)| c != id::DAIMYO && cards::is(c, ACTION) && !cards::is(c, cards::COMMAND)),
            _ => true,
        }
    }
}

/// How much a card of the hand is worth keeping against Sickness' discard (lower is discarded first): Victory cards and Curses nothing, Coppers
/// $1, other Treasures their $, Actions a good deal.
fn sickness_value(_view: &PlayerView, c: CardId) -> i32 {
    if cards::is(c, ACTION) {
        4
    } else if cards::is(c, VICTORY) || c == id::CURSE {
        0
    } else {
        cards::def(c).coins as i32
    }
}
