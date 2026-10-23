//! Menagerie bot defaults (rules in `docs/menagerie-plan.md`): which Way to play an Action as (`way_choice`, after a stated `[[way]]` rule), what to Exile, which
//! cards to bring back from the Exile mat, the choices of the reactions (Sleigh, Sheepdog, Falconer, Black Cat), "now or next turn" for Village Green and
//! Barge, and the Exile / Horse choices of the Events and cards that gain from the Supply. Which Event to buy is the gain list's job.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, ATTACK, DURATION, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Filter, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Menagerie;

/// Whether `c` does nothing but sit in the deck: a Curse, a Ruins, a Victory card with no other use.
fn dead(c: CardId) -> bool {
    c == id::CURSE || cards::is(c, cards::RUINS) || c == id::HOVEL || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
}

/// How much the card is worth keeping in a deck (higher is better) for the choices of what to Exile or bring back: dead cards first to go, a Copper next, then by
/// price and kind. Cards on the Exile mat still score, so a Victory card loses nothing there.
fn keep_value(s: &Strategy, view: &PlayerView, c: CardId) -> i32 {
    if c == id::CURSE || cards::is(c, cards::RUINS) {
        -10
    } else if dead(c) {
        -6
    } else if c == id::COPPER {
        // Thinning Coppers helps a deck that has other money; a bare deck needs them.
        if view.my_cards().count_type(TREASURE) > 10 { -2 } else { 1 }
    } else if cards::is(c, ACTION) {
        let listed = s.states_play(c) || s.lists(c);
        2 * cards::cost(c) as i32 + 4 + if listed { 6 } else { 0 }
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

/// What the Action `c` is worth when played for its own effect, on the scale of `way_value` (about a Silver = 2): the printed bonuses, an Attack and any text beyond
/// them a little more, a Duration's next-turn part too.
fn own_value(s: &Strategy, view: &PlayerView, c: CardId) -> f64 {
    let d = cards::def(c);
    let (cards_n, actions, buys, coins) = if d.cards == 0 && d.actions == 0 && d.coins == 0 && d.buys == 0 && cards::is(c, DURATION) {
        let (a, b, e, f) = cards::duration_bonus(c);
        (a, b, e, f)
    } else {
        (d.cards, d.actions, d.buys, d.coins)
    };
    let mut v = 1.2 * cards_n as f64 + 0.9 * actions as f64 + 0.3 * buys as f64 + coins as f64;
    if cards::is(c, DURATION) {
        let (a, b, e, f) = cards::duration_bonus(c);
        v += 1.2 * a as f64 + 0.9 * b as f64 + 0.3 * e as f64 + f as f64;
    }
    if cards::is(c, ATTACK) {
        v += 1.5;
    }
    // Text beyond the vanilla bonuses: a trash, a gain, a play. Listed in the strategy's own rules, worth more still.
    if !cards::is_choice_free(c) {
        v += 3.0;
    }
    if s.states_play(c) || s.lists(c) {
        v += 1.0;
    }
    // Actions left: +Actions are worth little when there is nothing to play.
    if actions > 0 && view.turn().actions == 0 && !view.hand().iter().any(|(x, _)| cards::is(x, ACTION)) {
        v -= 0.9 * actions as f64;
    }
    v
}

/// What playing the Action `card` as the Way `way` is worth, on the same scale as `own_value` (a Silver is 2, a Laboratory about 2.1).
fn way_value(s: &Strategy, view: &PlayerView, card: CardId, way: CardId) -> f64 {
    let hand = view.hand();
    let hand_size = hand.total() as f64;
    let dead_in_hand = hand.iter().filter(|&(c, _)| dead(c)).map(|(_, n)| n as f64).sum::<f64>();
    let more_actions = hand.iter().any(|(x, _)| cards::is(x, ACTION));
    let action_v = if view.turn().actions > 0 || more_actions { 0.9 } else { 0.0 };
    let d = cards::def(card);
    match way {
        id::WAY_OF_THE_BUTTERFLY => {
            // Returns this to its pile for a card costing $1 more: an upgrade of a card the strategy does not play.
            let junk = card == id::HORSE || !(s.states_play(card) || s.lists(card));
            if junk { 1.0 + cards::cost(card) as f64 * 0.6 } else { 0.3 }
        }
        id::WAY_OF_THE_CAMEL => 0.4,
        id::WAY_OF_THE_CHAMELEON => own_value(s, view, card) + 1.2 * (d.coins as f64 - d.cards as f64).max(0.0) * 0.0 + 0.1,
        id::WAY_OF_THE_FROG => action_v + 0.4,
        id::WAY_OF_THE_GOAT => 0.4 + 1.6 * (dead_in_hand.min(2.0)),
        id::WAY_OF_THE_HORSE => 2.4 * 1.0 + action_v,
        id::WAY_OF_THE_MOLE => action_v + 3.0 * 1.2 - (hand_size - dead_in_hand) * 1.0,
        id::WAY_OF_THE_MONKEY => 1.0 + if view.turn().coins as u32 + Strategy::money_in_hand(view) >= 8 { 0.6 } else { 0.1 },
        id::WAY_OF_THE_MOUSE => {
            let m = view.mouse_card();
            if m == 0 { 0.0 } else { own_value(s, view, m) }
        }
        id::WAY_OF_THE_MULE => action_v + 1.0,
        id::WAY_OF_THE_OTTER => 2.4,
        id::WAY_OF_THE_OWL => ((6.0 - hand_size).max(0.0)) * 1.2,
        id::WAY_OF_THE_OX => 2.0 * action_v * 0.9,
        id::WAY_OF_THE_PIG => 1.2 + action_v,
        id::WAY_OF_THE_RAT => {
            if hand.iter().any(|(c, _)| c == id::COPPER) && !dead(card) { 1.4 } else { 0.1 }
        }
        id::WAY_OF_THE_SEAL => 1.8,
        id::WAY_OF_THE_SHEEP => 2.0,
        id::WAY_OF_THE_SQUIRREL => 2.2,
        id::WAY_OF_THE_TURTLE => 0.6 * own_value(s, view, card),
        id::WAY_OF_THE_WORM => 1.2,
        _ => 0.0,
    }
}

/// The Way decision's fast default: the option worth the most, the card's own effect unless a Way is worth clearly more. `Some(close)`-style helpers below say
/// when the values are near enough that a searching bot looks.
pub(crate) fn way_values(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> (f64, Option<(CardId, f64)>) {
    let own = own_value(s, view, card);
    let best = iter_cards(choices).map(|w| (w, way_value(s, view, card, w))).max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
    (own, best)
}

/// The fast choice for a Way decision (no stated rule applies): a Way when it is worth at least 1.5 more than the card's own effect.
pub(crate) fn way_choice(s: &Strategy, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
    let (own, best) = way_values(s, view, decision.subject, choices);
    match best {
        Some((w, v)) if v >= own + 1.5 => Choice::Card(w),
        _ => Choice::Pass,
    }
}

/// Whether the options are close enough that a bot which searches should search the Way decision rather than trust the numbers.
pub(crate) fn way_is_close(s: &Strategy, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> bool {
    let (own, best) = way_values(s, view, decision.subject, choices);
    match best {
        Some((_, v)) => (v - own).abs() < 1.5 && v > 0.0,
        None => false,
    }
}

impl SetDefaults for Menagerie {
    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        let _ = (decision, from);
        let (cs, n) = cards_of(choices);
        let cs = &cs[..n];
        let worst = |f: &dyn Fn(CardId) -> i32| cs.iter().copied().min_by_key(|&c| (f(c), c)).map(Choice::Card);
        let best = |f: &dyn Fn(CardId) -> i32| cs.iter().copied().max_by_key(|&c| (f(c), std::cmp::Reverse(c))).map(Choice::Card);
        match (source, act) {
            // Exiling from the hand: the cards that cost nothing to lose, the ones with no copy on the mat first (Bounty Hunter's $3).
            (id::BOUNTY_HUNTER, Act::Exile) => {
                let mat = view.exile();
                worst(&|c| keep_value(s, view, c) * 2 + if mat.has(c) { 3 } else { 0 })
            }
            (id::SANCTUARY, Act::Exile) => {
                let c = worst(&|c| keep_value(s, view, c))?;
                match c {
                    Choice::Card(x) if keep_value(s, view, x) <= 0 => Some(c),
                    _ => Some(Choice::Pass),
                }
            }
            // Displace remodels a card: the one that unlocks the best gain of the gain list.
            (id::DISPLACE, Act::Exile) => {
                let gain = |s: &Strategy, c: CardId| s.gain_weight(view, view.cost(c).saturating_add(2), Filter::Any, false, cards::potion_cost(c));
                match s.best_trash_among(view, cs.iter().copied(), gain) {
                    Some((c, _)) => Some(Choice::Card(c)),
                    None => worst(&|c| keep_value(s, view, c)),
                }
            }
            // Banish: the name with the most dead cards in the hand (several copies at once); nothing if there are none.
            (id::BANISH, Act::Choose) => {
                let hand = view.hand();
                let pick = cs.iter().copied().filter(|&c| keep_value(s, view, c) <= -2).max_by_key(|&c| (hand.get(c), std::cmp::Reverse(c)));
                Some(pick.map(Choice::Card).unwrap_or(Choice::Pass))
            }
            // The copies of the name chosen: all of them.
            (id::BANISH, Act::Exile) => cs.first().map(|&c| Choice::Card(c)).or(Some(Choice::Pass)),
            // A victim of Cardinal Exiles the card it loses least by.
            (id::CARDINAL, Act::Exile) => worst(&|c| keep_value(s, view, c)),
            // Delay sets aside the best terminal Action when another Action is in hand to play now.
            (id::DELAY, Act::Choose) => {
                if cs.len() < 2 && view.hand().count_type(ACTION) < 2 {
                    return Some(Choice::Pass);
                }
                best(&|c| 100 - s.play_rank_of(c).min(100) + cards::cost(c) as i32)
            }
            // Transport puts the best Action of the mat onto the deck.
            (id::TRANSPORT, Act::Topdeck) => best(&|c| keep_value(s, view, c)),
            // Way of the Rat discards a Copper (or a Treasure that is not needed) for a copy of the card played.
            (id::WAY_OF_THE_RAT, Act::Discard) => {
                let c = worst(&|c| cards::def(c).coins as i32 * 10 + cards::cost(c) as i32)?;
                match c {
                    Choice::Card(x) if x == id::COPPER => Some(c),
                    _ => Some(Choice::Pass),
                }
            }
            // Way of the Frog: the cards go back in any order; the best on top.
            (id::WAY_OF_THE_FROG, Act::Topdeck) => worst(&|c| keep_value(s, view, c)),
            // Pursue's matches back on the deck.
            (id::PURSUE, Act::Topdeck) => worst(&|c| keep_value(s, view, c)),
            // Scrap: trash what the strategy wants gone, else the card that pays the most bonuses for the least loss (cost 4 to 6 non-Victory first).
            (id::SCRAP, Act::Trash) => {
                let wanted = cs.iter().copied().filter(|&c| s.wants_trash(view, c)).min_by_key(|&c| (keep_value(s, view, c), c));
                if let Some(c) = wanted {
                    return Some(Choice::Card(c));
                }
                best(&|c| -(keep_value(s, view, c)) * 0 + (cards::cost(c) as i32).min(6) * 3 - keep_value(s, view, c).max(0))
            }
            // Goatherd, Way of the Goat: the usual trash rules (junk first); Goatherd may pass.
            _ => None,
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            // Sleigh: into the hand when it can be played or used this turn, else onto the deck.
            id::SLEIGH => {
                let hand = find_mode(card, choices, |o| matches!(o, ModeOpt::MoveGained(dominion_engine::state::Dest::Hand)));
                let deck = find_mode(card, choices, |o| matches!(o, ModeOpt::MoveGained(dominion_engine::state::Dest::DeckTop)));
                let now = view.is_my_turn() && view.phase() == dominion_engine::Phase::Action && view.turn().actions > 0;
                if now { hand.or(deck) } else { deck.or(hand) }
            }
            // Scrap: the options in this order of worth (a Silver and a Horse are cards gained, +Card first when the deck is thin of draw).
            id::SCRAP => {
                let want = |o: &ModeOpt| match o {
                    ModeOpt::Cards(_) => 6,
                    ModeOpt::GainMany(c, ..) if *c == id::SILVER => 5,
                    ModeOpt::Coins(_) => 4,
                    ModeOpt::GainMany(c, ..) if *c == id::HORSE => 3,
                    ModeOpt::Buys(_) => 2,
                    ModeOpt::Actions(_) => 1,
                    _ => 0,
                };
                let table = cards::modes(card);
                choices
                    .iter()
                    .filter_map(|c| match c {
                        Choice::Mode(i) => Some((want(&table[*i as usize]), *i)),
                        _ => None,
                    })
                    .max()
                    .map(|(_, i)| Choice::Mode(i))
            }
            // Transport: put an Action from the mat onto the deck when it holds one the strategy wants, else Exile the best Action of the Supply.
            id::TRANSPORT => {
                let mat_has = view.exile().iter().any(|(c, _)| cards::is(c, ACTION) && (s.states_play(c) || s.lists(c)));
                let to_deck = find_mode(card, choices, |o| matches!(o, ModeOpt::ExileToDeck(_)));
                let exile = find_mode(card, choices, |o| matches!(o, ModeOpt::ExileFromSupply(_)));
                if mat_has { to_deck.or(exile) } else { exile.or(to_deck) }
            }
            _ => None,
        }
    }

    fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        let yes = |b: bool| Some(if b { Choice::Yes } else { Choice::No });
        match source {
            // Reactions that play a card: always (Sheepdog and Falconer gain cards; Black Cat draws and curses).
            id::SHEEPDOG | id::FALCONER | id::BLACK_CAT => Some(Choice::Yes),
            // Sleigh is discarded to move a good card gained to the hand or the deck.
            id::SLEIGH => {
                let c = decision.target;
                yes(!dead(c) && c != id::COPPER && cards::cost(c) >= 3 && c != id::HORSE)
            }
            // The Exile mat: all the copies come back when the card gained is one the strategy wants in the deck, not a dead card.
            id::EXILE => {
                let c = decision.subject;
                yes(!dead(c) && c != id::COPPER)
            }
            // Way of the Seal: topdeck what is good to draw next.
            id::WAY_OF_THE_SEAL => {
                let c = decision.subject;
                yes(!dead(c) && c != id::COPPER && (cards::is(c, ACTION) || view.is_treasure(c)) && c != id::HORSE)
            }
            // Way of the Butterfly: return a card that is not wanted if a card costing $1 more is in the gain list.
            id::WAY_OF_THE_BUTTERFLY => {
                let c = decision.subject;
                let cost = view.cost(c);
                let junk = c == id::HORSE || !(s.states_play(c) || s.lists(c));
                yes(junk && s.gain_weight(view, cost.saturating_add(1), Filter::Any, true, cards::potion_cost(c)) > 0.0)
            }
            // Village Green: now when there is an Action to use the Actions on, else for the start of the next turn; Barge: now ("Most of the time,
            // it's best to play your Barges to draw this turn"), unless the draw would reshuffle (fewer than 3 cards in the deck) or it is not my turn.
            id::VILLAGE_GREEN => {
                let own = view.is_my_turn();
                yes(own && view.hand().iter().any(|(c, _)| cards::is(c, ACTION)))
            }
            id::BARGE => {
                yes(view.is_my_turn() && view.deck_size() >= 3)
            }
            // Hunting Lodge: discard the hand for 5 cards when most of it is dead.
            id::HUNTING_LODGE => {
                let hand = view.hand();
                let dead_n: u32 = hand.iter().filter(|&(c, _)| dead(c) || c == id::COPPER).map(|(_, n)| n as u32).sum();
                yes(dead_n * 2 >= hand.total() || hand.total() <= 3)
            }
            // Stockpile, Gatekeeper's Exile: forced (the engine does not ask).
            _ => None,
        }
    }

    fn name(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, choices: &[Choice]) -> Option<Choice> {
        let _ = decision;
        if source != id::PURSUE {
            return None;
        }
        // Pursue keeps the top cards that match: name the card that is most worth having come up (a Gold, a Silver, an Action the strategy plays).
        let deck = view.my_cards();
        iter_cards(choices)
            .filter(|&c| !dead(c) && c != id::COPPER)
            .max_by_key(|&c| {
                let v = if view.is_treasure(c) { 3 * cards::def(c).coins as i32 } else if cards::is(c, ACTION) { 4 + if s.states_play(c) { 4 } else { 0 } } else { 0 };
                (v * deck.get(c) as i32, std::cmp::Reverse(c))
            })
            .map(Choice::Card)
    }

    /// The Exile gains of the set (a card taken from the Supply onto a mat) and the Horse gains that choose.
    fn gain(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, choices: &[Choice]) -> Option<Choice> {
        let _ = decision;
        match source {
            // Camel Train: the dearest non-Victory card the strategy does not list (it is denied to everyone; a Gold comes back with the next Gold).
            id::CAMEL_TRAIN => {
                iter_cards(choices).filter(|&c| !s.lists(c)).max_by_key(|&c| (cards::cost(c), c)).or_else(|| iter_cards(choices).find(|&c| c == id::GOLD)).map(Choice::Card)
            }
            // Invest and Transport Exile an Action the strategy wants to gain too (it draws when the others gain it, and Transport brings it back).
            id::INVEST | id::TRANSPORT => {
                let ranked = iter_cards(choices).filter_map(|c| s.gain_rank(view, c).map(|r| (r, c))).min();
                ranked.map(|(_, c)| Choice::Card(c)).or_else(|| iter_cards(choices).max_by_key(|&c| (cards::cost(c), c)).map(Choice::Card))
            }
            _ => None,
        }
    }

    /// Delay, Reap and Way of the Turtle play a card at the start of the next turn: with the Actions (their order is a play).
    fn start_order_group(card: CardId) -> Option<u8> {
        match card {
            id::DELAY | id::REAP | id::WAY_OF_THE_TURTLE | id::MASTERMIND => Some(2),
            _ => None,
        }
    }

    fn start_order_play(card: CardId) -> bool {
        matches!(card, id::DELAY | id::REAP | id::WAY_OF_THE_TURTLE | id::MASTERMIND)
    }

    /// Delay is pointless without an Action in the hand to set aside.
    fn useless_buy(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::DELAY => !view.hand().iter().any(|(c, _)| cards::is(c, ACTION)),
            id::SEIZE_THE_DAY => false,
            _ => false,
        }
    }
}
