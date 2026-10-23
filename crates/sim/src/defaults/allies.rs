//! Allies bot defaults (rules in `docs/allies-plan.md`): whether to spend Favors on the Ally's ability (`yes_no`, after a stated `[[favor]]` rule), rotating the split piles, the
//! cards the Allies choose to keep, set aside or put on top, and the choice of the "choose one" cards. Which cards to buy is the gain list's job.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, ATTACK, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::state::{Act, Zone};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Allies;

/// Whether `c` does nothing but sit in the deck: a Curse, a Ruins, a Victory card with no other use.
fn dead(c: CardId) -> bool {
    c == id::CURSE || cards::is(c, cards::RUINS) || c == id::HOVEL || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
}

/// How much the card is worth having in the hand or on top of the deck (higher is better): dead cards last, a Copper next, then by what it does.
fn value(s: &Strategy, view: &PlayerView, c: CardId) -> i32 {
    if dead(c) {
        -10
    } else if c == id::COPPER {
        0
    } else if cards::is(c, ACTION) {
        2 * cards::cost(c) as i32 + 4 + if s.states_play(c) || s.lists(c) { 6 } else { 0 }
    } else if view.is_treasure(c) {
        3 * cards::def(c).coins as i32 + 1
    } else {
        cards::cost(c) as i32
    }
}

fn cards_of(choices: &[Choice]) -> ([CardId; 40], usize) {
    let mut out = [0 as CardId; 40];
    let mut n = 0;
    for c in iter_cards(choices) {
        if n < out.len() {
            out[n] = c;
            n += 1;
        }
    }
    (out, n)
}

fn yn(yes: bool) -> Option<Choice> {
    Some(if yes { Choice::Yes } else { Choice::No })
}

/// What the cards of the pile the card `first` names are worth to the strategy: whether rotating the pile (top card and the copies under it to the bottom) uncovers a card the gain
/// list or the play rules want, and the top card is not one of them.
fn rotate_wanted(s: &Strategy, view: &PlayerView, first: CardId) -> bool {
    let pile = view.split_cards(first);
    let Some(&top) = pile.first() else { return false };
    let wanted = |c: CardId| s.lists(c) || s.states_play(c);
    if wanted(top) {
        return false;
    }
    // The next different card on top, once the top run is at the bottom.
    pile.iter().copied().find(|&c| c != top).is_some_and(wanted)
}

/// Whether the hand holds a card worth losing to a discard: a dead card or a Copper.
fn hand_junk(view: &PlayerView) -> u32 {
    view.hand().iter().filter(|&(c, _)| dead(c) || c == id::COPPER).map(|(_, n)| n as u32).sum()
}

/// The default answer to an offer to spend Favors on the Ally `ally`'s ability (a stated `[[favor]]` rule has been looked at).
fn favor_use(s: &Strategy, view: &PlayerView, decision: &Decision, ally: CardId) -> bool {
    let favors = view.favors();
    let hand = view.hand();
    match ally {
        // A cheaper card for 2 Favors: always, the Favors have no better use than free cards.
        id::ARCHITECTS_GUILD | id::CRAFTERS_GUILD | id::MOUNTAIN_FOLK | id::ISLAND_FOLK | id::BAND_OF_NOMADS | id::CIRCLE_OF_WITCHES => true,
        // Cave Dwellers sifts a dead card out of the hand; Desert Guides replaces a poor hand.
        id::CAVE_DWELLERS => hand_junk(view) > 0,
        id::DESERT_GUIDES => {
            let money = Strategy::money_in_hand(view) as i32;
            let actions = hand.iter().filter(|&(c, _)| cards::is(c, ACTION) && (s.states_play(c) || s.lists(c))).map(|(_, n)| n as i32).sum::<i32>();
            money + 3 * actions + 2 * (hand.total() as i32 - hand_junk(view) as i32 - actions) <= 6 && hand.total() >= 3
        }
        // City-state plays the Action gained; Trappers' Lodge puts the card gained on the deck: not a dead card.
        id::CITY_STATE => decision.subject != 0 && (s.states_play(decision.subject) || s.lists(decision.subject) || cards::def(decision.subject).cards > 0 || cards::def(decision.subject).actions > 0),
        id::TRAPPERS_LODGE => decision.subject != 0 && !dead(decision.subject) && decision.subject != id::COPPER,
        id::FELLOWSHIP_OF_SCRIBES => true,
        id::FOREST_DWELLERS => favors >= 2 || view.deck_size() > 0,
        // Pay a Favor to keep the whole hand only when it holds nothing to lose; discard a dead card for free.
        id::GANG_OF_PICKPOCKETS => hand_junk(view) == 0 && favors >= 2,
        id::MARKET_TOWNS => hand.iter().any(|(c, _)| cards::is(c, ACTION) && (s.states_play(c) || s.lists(c) || cards::is_choice_free(c) && cards::def(c).actions > 0 || !cards::is(c, ATTACK))),
        // Put a token on a pile the strategy buys from (it lowers the price for everyone: worth it only for a card it wants).
        id::FAMILY_OF_INVENTORS => view.supply_cards().any(|c| view.pile_open(c) && !cards::is(c, VICTORY) && c != id::COPPER && c != id::CURSE && s.lists(c) && view.supply(c) > 0),
        id::WOODWORKERS_GUILD => {
            hand.iter().any(|(c, _)| cards::is(c, ACTION) && !s.states_play(c) && !s.lists(c))
                && view.supply_cards().any(|c| cards::is(c, ACTION) && view.pile_open(c) && view.supply(c) > 0 && s.lists(c))
        }
        _ => false,
    }
}

/// The picks of a "choose one" card, by what the hand can use.
fn mode_pick(s: &Strategy, view: &PlayerView, decision: &Decision, card: CardId, choices: &[Choice]) -> Option<Choice> {
    let hand = view.hand();
    let actions_in_hand = hand.iter().any(|(c, _)| cards::is(c, ACTION));
    let table = cards::modes(card);
    let pick = |pred: &dyn Fn(&cards::ModeOpt) -> bool| -> Option<Choice> {
        table.iter().enumerate().find_map(|(i, o)| {
            let ch = Choice::Mode(i as u8);
            (pred(o) && choices.contains(&ch)).then_some(ch)
        })
    };
    use cards::ModeOpt as M;
    match card {
        id::BAUBLE => {
            // +$1 first, then a Favor when the Ally uses them, else a Buy; the top-decking last.
            let money = view.turn().coins as u32 + Strategy::money_in_hand(view);
            let order: [fn(&M) -> bool; 4] = if money >= 10 {
                [|o| matches!(o, M::Buys(_)), |o| matches!(o, M::Coins(_)), |o| matches!(o, M::Favors(_)), |o| matches!(o, M::TopdeckGains)]
            } else if view.ally() != 0 {
                [|o| matches!(o, M::Coins(_)), |o| matches!(o, M::Favors(_)), |o| matches!(o, M::Buys(_)), |o| matches!(o, M::TopdeckGains)]
            } else {
                [|o| matches!(o, M::Coins(_)), |o| matches!(o, M::Buys(_)), |o| matches!(o, M::TopdeckGains), |o| matches!(o, M::Favors(_))]
            };
            order.iter().find_map(|f| pick(f))
        }
        id::TOWN => {
            if actions_in_hand && view.turn().actions == 0 { pick(&|o| matches!(o, M::CardsActions(..))) } else { pick(&|o| matches!(o, M::BuysCoins(..))).or_else(|| pick(&|_| true)) }
        }
        id::TOWN_CRIER => {
            // +$2, unless the deck is still thin on money and a Silver is worth more.
            if view.turn().number <= 4 && s.lists(id::SILVER) { pick(&|o| matches!(o, M::GainMany(..))) } else { pick(&|o| matches!(o, M::Coins(_))) }
        }
        id::BLACKSMITH => {
            let h = hand.total();
            if actions_in_hand && view.turn().actions == 0 {
                pick(&|o| matches!(o, M::CardsActions(..)))
            } else if h <= 3 {
                pick(&|o| matches!(o, M::DrawUpTo(_)))
            } else {
                pick(&|o| matches!(o, M::Cards(_)))
            }
        }
        id::INNKEEPER => {
            let junk = hand_junk(view);
            if junk >= 5 {
                pick(&|o| matches!(o, M::DrawThenDiscard(5, _)))
            } else if junk >= 3 {
                pick(&|o| matches!(o, M::DrawThenDiscard(3, _)))
            } else {
                pick(&|o| matches!(o, M::Cards(_)))
            }
        }
        id::BROKER => {
            let cost = cards::cost(decision.subject);
            if cost == 0 {
                return pick(&|_| true);
            }
            let favors_matter = view.ally() != 0 && view.favors() < 6 && matches!(
                view.ally(),
                id::CAVE_DWELLERS | id::ARCHITECTS_GUILD | id::MOUNTAIN_FOLK | id::LEAGUE_OF_BANKERS | id::ISLAND_FOLK | id::BAND_OF_NOMADS
            );
            if favors_matter && cost >= 2 {
                pick(&|o| matches!(o, M::CostFavors))
            } else if cost >= 4 {
                pick(&|o| matches!(o, M::CostCoins))
            } else {
                pick(&|o| matches!(o, M::CostCards))
            }
        }
        id::MODIFY => {
            let cost = cards::cost(decision.subject);
            if cost + 2 >= 3 && !(actions_in_hand && cost <= 1) { pick(&|o| matches!(o, M::GainPlus(_))) } else { pick(&|o| matches!(o, M::CardsActions(..))) }
        }
        id::SPECIALIST => {
            if view.is_treasure(decision.subject) && !cards::is(decision.subject, ACTION) {
                pick(&|o| matches!(o, M::GainCopy))
            } else {
                pick(&|o| matches!(o, M::PlaySubjectAgain))
            }
        }
        id::HILL_FORT => {
            let c = decision.subject;
            let useful = c != 0 && !dead(c) && (view.is_treasure(c) && cards::def(c).coins >= 2 || cards::is(c, ACTION) && (s.states_play(c) || s.lists(c)));
            if useful { pick(&|o| matches!(o, M::MoveGained(_))) } else { pick(&|o| matches!(o, M::CardsActions(..))) }
        }
        // +$3 now, unless the next turn's 3 Cards are worth more (early in the game, when the deck is small).
        id::STRONGHOLD => {
            if view.turn().number <= 3 { pick(&|o| matches!(o, M::DurationNext)) } else { pick(&|o| matches!(o, M::Coins(_))) }
        }
        id::BAND_OF_NOMADS => {
            if actions_in_hand && view.turn().actions == 0 {
                pick(&|o| matches!(o, M::Actions(_)))
            } else if view.turn().coins as u32 + Strategy::money_in_hand(view) >= 10 {
                pick(&|o| matches!(o, M::Buys(_)))
            } else {
                pick(&|o| matches!(o, M::Cards(_)))
            }
        }
        _ => None,
    }
}

impl SetDefaults for Allies {
    fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        // An Ally's ability: a stated rule, else the default.
        if cards::is_ally(source) {
            return yn(s.stated_favor(view, decision).unwrap_or_else(|| favor_use(s, view, decision, source)));
        }
        let DecisionKind::YesNo { act } = decision.kind else { return None };
        match (source, act) {
            // The questions "may rotate the pile?" (the Ask frames, `act` Discard, `subject` the first card of the pile).
            (id::HERB_GATHERER | id::OLD_MAP | id::STUDENT | id::TENT | id::TOWN_CRIER, Act::Discard) => yn(rotate_wanted(s, view, cards::pile_of(decision.subject))),
            // Capital City: pay $2 for 2 Cards when there is an Action left to use them and the $ are not needed.
            (id::CAPITAL_CITY, Act::Discard) => {
                let coins = view.turn().coins as i32;
                yn(coins >= 2 && view.turn().actions > 0 && coins - 2 + Strategy::money_in_hand(view) as i32 >= 5)
            }
            // Bauble: a gained card is put onto the deck unless it is dead there (a Curse, a Victory card).
            (id::BAUBLE, Act::Topdeck) => yn(!dead(decision.subject)),
            // Acolyte: trash itself for an Augur the strategy wants.
            (id::ACOLYTE, Act::Trash) => {
                let top = view.split_top(id::HERB_GATHERER);
                yn(top.is_some_and(|t| s.lists(t) && t != id::ACOLYTE))
            }
            _ => None,
        }
    }

    fn select(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        let (cs, n) = cards_of(choices);
        let cs = &cs[..n];
        let best = |f: &dyn Fn(CardId) -> i32| cs.iter().copied().max_by_key(|&c| (f(c), std::cmp::Reverse(c))).map(Choice::Card);
        let worst = |f: &dyn Fn(CardId) -> i32| cs.iter().copied().min_by_key(|&c| (f(c), c)).map(Choice::Card);
        match (source, act, from) {
            // Battle Plan rotates a pile when that uncovers a card the strategy wants (and hides nothing it wants).
            (id::BATTLE_PLAN, Act::Choose, Zone::Supply) => {
                let pick = cs.iter().copied().find(|&c| rotate_wanted(s, view, cards::pile_of(c)));
                Some(pick.map(Choice::Card).unwrap_or(Choice::Pass))
            }
            // An Attack revealed for a card: any.
            (id::BATTLE_PLAN, Act::Reveal, _) => cs.first().map(|&c| Choice::Card(c)),
            // Sibyl: the best card on top, the next best on the bottom (a dead card stays in the hand to be discarded).
            (id::SIBYL, Act::Topdeck, _) | (id::SIBYL, Act::Bottom, _) => best(&|c| value(s, view, c)),
            // The Archer's victim keeps the best card hidden; the attacker has another discarded: the best of what they reveal.
            (id::ARCHER, Act::Discard, _) => best(&|c| value(s, view, c)),
            // Coastal Haven keeps the best cards, each for a Favor: Treasures and Actions worth a slot of the next hand.
            (id::COASTAL_HAVEN, Act::SetAside, _) => {
                let pick = cs.iter().copied().filter(|&c| value(s, view, c) >= 6).max_by_key(|&c| (value(s, view, c), std::cmp::Reverse(c)));
                Some(pick.map(Choice::Card).unwrap_or(Choice::Pass))
            }
            // Order of Astrologers: the best cards of the discard pile on top, the worst of those picked first (the last picked is on top).
            (id::ORDER_OF_ASTROLOGERS, Act::Topdeck, _) => {
                let DecisionKind::Select { max, .. } = decision.kind else { return None };
                let mut good: Vec<CardId> = cs.iter().copied().filter(|&c| value(s, view, c) >= 6).collect();
                good.sort_by_key(|&c| (std::cmp::Reverse(value(s, view, c)), c));
                good.truncate(max as usize);
                let pick = good.last().copied();
                Some(pick.map(Choice::Card).unwrap_or(Choice::Pass))
            }
            // Order of Masons: dead cards are left out of the shuffle.
            (id::ORDER_OF_MASONS, Act::SetAside, _) => {
                let pick = cs.iter().copied().filter(|&c| dead(c)).min_by_key(|&c| (value(s, view, c), c));
                Some(pick.map(Choice::Card).unwrap_or(Choice::Pass))
            }
            // Family of Inventors: a token on the pile of the best non-Victory card the strategy lists.
            (id::FAMILY_OF_INVENTORS, Act::Choose, _) => {
                let ranked = cs.iter().copied().filter(|&c| s.lists(c) && c != id::COPPER).filter_map(|c| s.gain_rank(view, c).map(|r| (r, c))).min();
                ranked.map(|(_, c)| Choice::Card(c)).or_else(|| worst(&|c| cards::cost(c) as i32))
            }
            // Woodworkers' Guild trashes the Action it likes least.
            (id::WOODWORKERS_GUILD, Act::Trash, _) => worst(&|c| value(s, view, c)),
            // Miller and Hunter: the card that is worth most.
            (id::MILLER | id::HUNTER, Act::ToHand, _) => best(&|c| value(s, view, c)),
            // Courier and Herb Gatherer play the best Treasure or Action of the discard pile.
            (id::COURIER, Act::Play, Zone::Discard) | (id::HERB_GATHERER, Act::Play, Zone::Discard) => {
                let pick = cs.iter().copied().filter(|&c| !dead(c) && (view.is_treasure(c) || s.states_play(c) || s.lists(c) || cards::is_choice_free(c))).max_by_key(|&c| (value(s, view, c), std::cmp::Reverse(c)));
                Some(pick.map(Choice::Card).unwrap_or(Choice::Pass))
            }
            _ => None,
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        // The decision's subject is not passed here: the cards that need it (Broker, Modify, Specialist, Hill Fort) read the pending decision from the view.
        let subject = view.pending_subject().unwrap_or(0);
        let d = Decision {
            player: view.me(),
            for_player: view.me(),
            kind: DecisionKind::Mode { picks: 1, distinct: false },
            source: Some(card),
            subject,
            upgrade: None,
            play_times: 1,
            target: 0,
        };
        mode_pick(s, view, &d, card, choices)
    }

    /// Gang of Pickpockets and the abilities that discard come before the hand grows; the rest follow the Durations.
    fn start_order_group(card: CardId) -> Option<u8> {
        match card {
            id::GANG_OF_PICKPOCKETS => Some(0),
            id::CONJURER => Some(1),
            id::ROYAL_GALLEY | id::CONTRACT => Some(2),
            id::CAVE_DWELLERS | id::DESERT_GUIDES | id::FOREST_DWELLERS | id::CRAFTERS_GUILD | id::MOUNTAIN_FOLK => Some(3),
            _ => None,
        }
    }

    fn start_order_play(card: CardId) -> bool {
        matches!(card, id::ROYAL_GALLEY | id::CONTRACT)
    }

    /// An Action Hunter and the other pile cards are worth playing as any other; Royal Galley, Specialist and Elder need an Action in hand to be worth a play.
    fn worth_playing(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            id::ROYAL_GALLEY | id::ELDER | id::SPECIALIST => view.hand().iter().any(|(c, _)| c != card && cards::is(c, ACTION)) || card == id::SPECIALIST,
            _ => true,
        }
    }
}
