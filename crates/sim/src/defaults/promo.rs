//! Promo bot defaults: the Action Prince holds, the Supply Action Captain plays, which card Envoy's left player makes
//! the revealer lose, and Governor's option and trash, Dismantle's trash, Church's set-aside and Marchland's discard
//! (rules in `docs/promo-plan.md` section 5).

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, VICTORY};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Filter, Phase, Zone};

use super::trashers;
use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Promo;

impl SetDefaults for Promo {
    /// Dismantle trashes a card it must, whatever the play rules say: not played when nothing in the rest of the hand pays for it.
    fn useless_play(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        card == id::DISMANTLE && !trashers::dismantle_pays(s, view, card)
    }

    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            id::PRINCE if act == Act::SetAside => Some(choose_prince_card(s, view, choices)),
            // Captain: the Action from the Supply to play (now, or at the start of the next turn).
            id::CAPTAIN if act == Act::Play && from == Zone::Supply => Some(choose_captain_card(s, view, decision, choices)),
            // Dismantle: the card whose trash pays best (a Gold, a cheaper card of the gain list and the Gold it brings).
            id::DISMANTLE if act == Act::Trash && !s.legacy_defaults => trashers::pick(s, view, choices, |s, c| trashers::dismantle_gain(s, view, c)),
            id::DISMANTLE if act == Act::Trash => Some(choose_dismantle_trash(s, view, choices)),
            // Governor's third option, for every player: trash a card only for a better card of exactly $1 ($2) more.
            id::GOVERNOR if act == Act::Trash => Some(choose_governor_trash(s, view, decision, choices)),
            // Church: surplus money and dead Actions go into the next hand; the trash next turn is the generic one (the [[trash]] rules).
            id::CHURCH if act == Act::SetAside => Some(choose_church_cards(view, choices)),
            // Marchland: every card the rest of the turn does not need, for +$1 each.
            id::MARCHLAND if act == Act::Discard => Some(choose_marchland_discards(view, choices)),
            // Envoy: the player to the left picks which of the revealed cards the revealer loses.
            id::ENVOY if act == Act::Discard && decision.player != decision.for_player => {
                Some(iter_cards(choices).max_by_key(|&c| (Strategy::card_value(c), c)).map(Choice::Card).unwrap_or(choices[0]))
            }
            _ => None,
        }
    }

    /// Governor: a Gold in hand is turned into a Province (the gain list would take one); otherwise a Gold, and cards instead once the game
    /// could end soon (a Gold would not be drawn again).
    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        (card == id::GOVERNOR).then(|| default_governor(s, view, choices))
    }

    /// Dismantle trashes a card it must: not worth playing when nothing in the rest of the hand pays for it.
    fn worth_playing(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        if card == id::GOVERNOR {
            return governor_worth_playing(s, view);
        }
        if card != id::DISMANTLE {
            return true;
        }
        let mut hand = *view.hand();
        hand.remove(id::DISMANTLE);
        let endgame = Strategy::game_could_end_soon(view);
        hand.iter().any(|(c, _)| dismantle_score(s, view, c, endgame) >= 0)
    }

    /// The play choice by rule order alone (inside search playouts): Captain's pick without the search.
    fn rule_play(s: &Strategy, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        (decision.source == Some(id::CAPTAIN)).then(|| captain_rule_pick(s, view, choices))
    }

    fn start_order_group(card: CardId) -> Option<u8> {
        match card {
            id::CAPTAIN => Some(2),
            // Church brings its cards into the hand and then trashes one: after the draws, so the trash sees the fullest hand.
            id::CHURCH => Some(3),
            _ => None,
        }
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
    if s.searches(decision, choices) && iter_cards(choices).count() >= 2 && decision.player == view.me() {
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

/// What a card is worth in the deck for the trash and gain choices below: the engine-wide rough value, but a Victory card is worth its
/// points once the game could end soon.
fn worth(c: CardId, endgame: bool) -> i32 {
    if endgame && cards::is(c, VICTORY) {
        5 * cards::def(c).vp as i32
    } else {
        Strategy::card_value(c)
    }
}

/// Dismantle trashing `c`: a card costing $1 or more brings the Gold (little in the endgame) and the best card of the gain list that is
/// cheaper; what is lost is the card. A $0 card brings nothing, so only a Curse (or other dead weight) is worth it.
fn dismantle_score(s: &Strategy, view: &PlayerView, c: CardId, endgame: bool) -> i32 {
    let cost = view.cost(c);
    let mut score = -worth(c, endgame);
    if cost >= 1 {
        if view.supply(id::GOLD) > 0 {
            score += if endgame { 2 } else { 12 };
        }
        if let Some(rank) = s.best_gain_rank(view, cost - 1, Filter::Any, false, cards::potion_cost(c)) {
            score += worth(s.buy[rank].0, endgame);
        }
    }
    score
}

fn choose_dismantle_trash(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let endgame = Strategy::game_could_end_soon(view);
    iter_cards(choices)
        .max_by_key(|&c| (dismantle_score(s, view, c, endgame), std::cmp::Reverse(c)))
        .map(Choice::Card)
        .unwrap_or(choices[0])
}

/// Governor's "may trash a card and gain a card costing exactly $1 ($2) more": the Governor's own player gains $2 more, the others $1 more.
/// Trash the card that gains the most (a card of the gain list that costs exactly that much, against the worth of what is lost), only if it
/// gains something clearly better.
fn choose_governor_trash(s: &Strategy, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
    let plus: u8 = if decision.player == view.turn().player { 2 } else { 1 };
    let endgame = Strategy::game_could_end_soon(view);
    let mut best: Option<(i32, CardId)> = None;
    for c in iter_cards(choices) {
        let Some(rank) = s.best_gain_rank(view, view.cost(c) + plus, Filter::Any, true, cards::potion_cost(c)) else { continue };
        let gained = s.buy[rank].0;
        // A Victory card the gain list wants is worth its points (the list says when it is time to green).
        let gain = if cards::is(gained, VICTORY) { 6 * cards::def(gained).vp as i32 } else { worth(gained, endgame) };
        let score = gain - worth(c, endgame) + if s.wants_trash(view, c) { 4 } else { 0 };
        if score >= 4 && best.map_or(true, |(b, bc)| (score, std::cmp::Reverse(c)) > (b, std::cmp::Reverse(bc))) {
            best = Some((score, c));
        }
    }
    best.map(|(_, c)| Choice::Card(c)).unwrap_or(Choice::Pass)
}

/// Governor's option: the modes are cards, a Gold, and Remodel.
fn default_governor(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let pick = |want: u8| choices.contains(&Choice::Mode(want)).then_some(Choice::Mode(want));
    if remodel_gold(s, view) {
        if let Some(c) = pick(2) {
            return c;
        }
    }
    pick(0).unwrap_or(choices[0])
}

/// Church: the cards to set aside for the next hand, one pick at a time. Actions that cannot be played this turn (no Action left); and
/// Treasures that this turn's money does not need (more than $8, or $11 with Colonies in the game, in hand and made so far).
fn choose_church_cards(view: &PlayerView, choices: &[Choice]) -> Choice {
    let t = view.turn();
    if t.actions == 0 {
        if let Some(c) = iter_cards(choices).filter(|&c| cards::is(c, ACTION)).max_by_key(|&c| (cards::cost(c), c)) {
            return Choice::Card(c);
        }
    }
    let target: i32 = if view.in_supply(id::COLONY) { 11 } else { 8 };
    let mut have = t.coins as i32;
    for (c, n) in view.hand().iter() {
        if view.is_treasure(c) {
            have += n as i32 * cards::def(c).coins as i32;
        }
    }
    let surplus = have - target;
    iter_cards(choices)
        .filter(|&c| view.is_treasure(c) && (cards::def(c).coins as i32) > 0 && (cards::def(c).coins as i32) <= surplus)
        .max_by_key(|&c| (cards::def(c).coins, c))
        .map(Choice::Card)
        .unwrap_or(Choice::Pass)
}

/// Marchland's "discard any number of cards for +$1 each": the hand is discarded at Clean-up anyway, so every card the rest of this turn does
/// not need goes now. In the Buy phase that is all but Treasures (still to be played) and Reactions that answer a gain (Watchtower, Trader);
/// before it, the cards that are dead weight now (Victory cards, Curses, Ruins and Actions with no Action left to play them). Never on another
/// player's turn (the $ would be lost).
fn choose_marchland_discards(view: &PlayerView, choices: &[Choice]) -> Choice {
    if !view.is_my_turn() {
        return Choice::Pass;
    }
    let buy_phase = view.phase() == Phase::Buy;
    let actions_left = view.turn().actions > 0;
    let dead = |c: CardId| -> bool {
        if view.is_treasure(c) || matches!(c, id::WATCHTOWER | id::TRADER) {
            return false;
        }
        if buy_phase {
            return true;
        }
        let junk = (cards::is(c, VICTORY) && !cards::is(c, ACTION)) || c == id::CURSE || cards::is(c, cards::RUINS);
        junk || (cards::is(c, ACTION) && !actions_left)
    };
    iter_cards(choices).find(|&c| dead(c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

/// Governor gives every other player a card (or a Silver, or a trash): not worth it when the hand already pays for the most the bot buys
/// ($8, $11 with Colonies) and there is no Gold to remodel into a Province.
fn governor_worth_playing(s: &Strategy, view: &PlayerView) -> bool {
    if remodel_gold(s, view) {
        return true;
    }
    let target: i32 = if view.in_supply(id::COLONY) { 11 } else { 8 };
    let mut have = view.turn().coins as i32;
    for (c, n) in view.hand().iter() {
        if view.is_treasure(c) {
            have += n as i32 * cards::def(c).coins as i32;
        }
    }
    have < target
}

/// A Gold in hand that Governor can turn into a Province the gain list wants, late enough to green.
fn remodel_gold(s: &Strategy, view: &PlayerView) -> bool {
    view.hand().has(id::GOLD)
        && view.supply(id::PROVINCE) as u32 <= 4
        && s.best_gain_rank(view, view.cost(id::GOLD) + 2, Filter::Any, true, false).is_some()
}
