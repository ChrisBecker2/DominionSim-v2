//! Renaissance bot defaults (rules in `docs/renaissance-plan.md`): when to spend Villagers (`Strategy::villager_choice`, in `strategy.rs`), the choices of
//! the set's cards that the generic shapes do not decide (which revealed card to keep, what to take from the discard pile, which Artifact, Scepter's
//! replay, Sinister Plot's tokens, Cargo Ship's set-aside), the order of the Projects' start-of-turn effects, and the forced trashes that should not
//! give a Curse. Which Project to buy is the gain list's job.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, ATTACK, DURATION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Renaissance;

/// How good a card is to have in the hand (higher is better) for the picks that choose one card to keep: a Treasure by its coins, an Action the
/// strategy plays by a good deal more, Victory cards and Curses nothing.
fn keep_value(s: &Strategy, view: &PlayerView, c: CardId) -> i32 {
    if c == id::CURSE || cards::is(c, cards::RUINS) {
        -5
    } else if cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE) {
        -1
    } else if cards::is(c, ACTION) {
        // An Action the strategy plays (or one that gives cards, Actions or $) beats a Treasure of the same price.
        let d = cards::def(c);
        let listed = s.states_play(c) || s.lists(c);
        2 * cards::cost(c) as i32 + 4 + if listed { 6 } else { 0 } + 2 * d.cards as i32 + 3 * d.actions as i32 + d.coins as i32 + if view.turn().actions == 0 && d.actions == 0 { -6 } else { 0 }
    } else if view.is_treasure(c) {
        3 * cards::def(c).coins as i32 + 1
    } else {
        cards::cost(c) as i32
    }
}

/// The picks of `choices` that are cards, copied into a small array (no allocation): up to 32 of them, in order.
fn cards_of(choices: &[Choice]) -> ([CardId; 32], usize) {
    let mut out = [0 as CardId; 32];
    let mut n = 0;
    for c in iter_cards(choices) {
        if n < out.len() {
            out[n] = c;
            n += 1;
        }
    }
    (out, n)
}

impl SetDefaults for Renaissance {
    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        let _ = (decision, from);
        let best = |f: &dyn Fn(CardId) -> i32| {
            let (cs, n) = cards_of(choices);
            cs[..n].iter().copied().max_by_key(|&c| (f(c), std::cmp::Reverse(c))).map(Choice::Card)
        };
        match (source, act) {
            // Border Guard keeps the better of the revealed cards; Mountain Village takes the best card of the discard pile.
            (id::BORDER_GUARD, Act::ToHand) | (id::MOUNTAIN_VILLAGE, Act::ToHand) => best(&|c| keep_value(s, view, c)),
            // Hideout trashes a card of the hand; a Victory card is a Curse in exchange, so it is only trashed when nothing else is left.
            (id::HIDEOUT, Act::Trash) => {
                let mut other = [Choice::Pass; 32];
                let mut k = 0;
                for c in iter_cards(choices).filter(|&c| !cards::is(c, VICTORY)).take(32) {
                    other[k] = Choice::Card(c);
                    k += 1;
                }
                if k == 0 { None } else { Some(s.choose_trash(view, decision, &other[..k], from, Filter::Any, 1, 1, false)) }
            }
            // Silos discards every Copper (each is replaced by an average card).
            (id::SILOS, Act::Discard) => iter_cards(choices).next().map(Choice::Card),
            // Crop Rotation: a Victory card that is nothing but points.
            (id::CROP_ROTATION, Act::Discard) => {
                let dead = |c: CardId| cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE);
                iter_cards(choices).filter(|&c| dead(c)).min_by_key(|&c| (cards::cost(c), c)).map(Choice::Card).or(Some(Choice::Pass))
            }
            // City Gate puts the best card of the hand back on the deck: next turn's hand holds it.
            (id::CITY_GATE, Act::Topdeck) => best(&|c| keep_value(s, view, c)),
            // Star Chart: the best card to start the next pass through the deck with (a Victory card never).
            (id::STAR_CHART, Act::Topdeck) => {
                let c = best(&|c| keep_value(s, view, c))?;
                match c {
                    Choice::Card(x) if keep_value(s, view, x) > 0 => Some(c),
                    _ => Some(Choice::Pass),
                }
            }
            // Scepter replays the Action in play that is worth the most.
            (id::SCEPTER, Act::Choose) => best(&|c| replay_value(c)),
            _ => None,
        }
    }

    fn default_mode(_s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            // The Lantern (a third card to choose from) over the Horn.
            id::BORDER_GUARD => find_mode(card, choices, |o| matches!(o, ModeOpt::TakeArtifact(id::LANTERN))).or_else(|| choices.first().copied()),
            // Scepter: replay an Action that gives a good deal (cards, $, Buys, an attack), else +$2.
            id::SCEPTER => {
                let target = view
                    .in_play()
                    .iter()
                    .filter(|&(c, _)| cards::is(c, ACTION) && !cards::is(c, cards::COMMAND) && view.turn().played.has(c))
                    .map(|(c, _)| replay_value(c))
                    .max()
                    .unwrap_or(0);
                let replay = find_mode(card, choices, |o| matches!(o, ModeOpt::ReplayAction));
                let coins = find_mode(card, choices, |o| matches!(o, ModeOpt::Coins(_)));
                if target >= 5 { replay.or(coins) } else { coins.or(replay) }
            }
            // Treasurer: trash a Copper (or whatever the strategy trashes) while the hand has one, take the Key when nobody has it yet, regain a
            // Treasure from the trash when it holds a Gold or better; otherwise the Key again (it is free).
            id::TREASURER => {
                let trash = find_mode(card, choices, |o| matches!(o, ModeOpt::TrashFiltered(_)));
                let regain = find_mode(card, choices, |o| matches!(o, ModeOpt::GainFromTrashTo(..)));
                let key = find_mode(card, choices, |o| matches!(o, ModeOpt::TakeArtifact(_)));
                let me = view.me();
                let has_key = view.has_artifact(me, id::KEY);
                let junk = view.hand().iter().any(|(c, _)| view.is_treasure(c) && c == id::COPPER);
                let good_in_trash = view.trash().iter().any(|(c, _)| view.is_treasure(c) && cards::def(c).coins >= 3);
                if good_in_trash {
                    regain.or(key)
                } else if !has_key {
                    key.or(trash)
                } else if junk {
                    trash.or(key)
                } else {
                    key.or(trash)
                }
            }
            _ => None,
        }
    }

    fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        match source {
            // Cargo Ship: set aside a card worth having in next turn's hand (not a Victory card or a Curse, nor a Copper).
            id::CARGO_SHIP => {
                let c = decision.subject;
                let good = c != id::COPPER && keep_value(s, view, c) > 3;
                Some(if good { Choice::Yes } else { Choice::No })
            }
            // Innovation plays the Action gained when it is worth playing at all.
            id::INNOVATION => Some(if s.default_worth_playing(view, decision.subject) && !s.useless_play(view, decision.subject) { Choice::Yes } else { Choice::No }),
            // Sinister Plot (Yes: add a token; No: remove them all for cards): cash in with 3 or more tokens, or any near the end of the game.
            id::SINISTER_PLOT => {
                let tokens = view.plot_tokens_of(view.me());
                let late = view.supply(id::PROVINCE) <= 3 || view.empty_piles() >= 2;
                Some(if tokens >= 4 || (tokens >= 3 && view.deck_size() >= 3) || (late && tokens >= 1) { Choice::No } else { Choice::Yes })
            }
            // The Horn puts a Border Guard onto the deck, Pageant pays $1 for a Coffers: always.
            id::HORN | id::PAGEANT => Some(Choice::Yes),
            _ => None,
        }
    }

    /// Forced trashes that give nothing back: Hideout with only Victory cards in the hand turns the best of them into a Curse.
    fn useless_play(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::HIDEOUT => {
                // Nothing in the hand besides Victory cards (and the Hideout itself): the trash gives a Curse.
                let (mut any, mut all_victory) = (false, true);
                for (c, n) in view.hand().iter() {
                    if n - (c == id::HIDEOUT) as u8 > 0 {
                        any = true;
                        all_victory &= cards::is(c, VICTORY) && !cards::is(c, ACTION);
                    }
                }
                any && all_victory
            }
            _ => false,
        }
    }

    /// The Projects' start-of-turn effects: the cards that come into the hand first (Cargo Ship, Research), then the draws, the plays and the trash
    /// (Cathedral) last, from the fullest hand.
    fn start_order_group(card: CardId) -> Option<u8> {
        match card {
            id::CARGO_SHIP | id::RESEARCH => Some(0),
            id::SINISTER_PLOT | id::CITY_GATE => Some(1),
            id::SILOS | id::CROP_ROTATION => Some(2),
            id::PIAZZA => Some(3),
            id::CATHEDRAL => Some(4),
            _ => None,
        }
    }

    fn start_order_play(_card: CardId) -> bool {
        false
    }
}

/// What replaying the Action `c` is worth with Scepter: the cards, $, Actions and Buys it gives, an Attack a little, a Duration (it comes back next turn
/// too) a little more; 5 or more is worth the replay over +$2.
fn replay_value(c: CardId) -> i32 {
    let d = cards::def(c);
    let mut v = 2 * d.cards as i32 + d.coins as i32 + d.buys as i32 + d.actions as i32;
    if cards::is(c, ATTACK) {
        v += 2;
    }
    if cards::is(c, DURATION) {
        v += 1;
    }
    // Cards that gain, trash or draw by their text, not their numbers.
    if matches!(c, id::OLD_WITCH | id::VILLAIN | id::SCHOLAR | id::SWASHBUCKLER | id::SEER | id::BORDER_GUARD | id::RECRUITER | id::SCULPTOR | id::ACTING_TROUPE | id::INVENTOR) {
        v += 5;
    }
    v
}
