//! Adventures bot defaults (see `docs/adventures-plan.md`, section 3): where tokens go, whether to call a Reserve card, whether to exchange a
//! Traveller, Inheritance's pick, Gear's set-aside, and the modes of Amulet, Miser, Quest and Teacher. Stated `[[token]]`, `[[call]]` and
//! `[[exchange]]` rules come first.

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ModeOpt, ACTION, TREASURE, VICTORY};
use dominion_engine::counts::Counts;
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::state::{Act, Zone, TOK_ACTION, TOK_BUY, TOK_CARD, TOK_COIN, TOK_COST, TOK_TRASH};

use super::SetDefaults;
use crate::strategy::*;

pub(super) struct Adventures;

/// A card that does nothing for a deck: Curse, Victory cards that are not Actions or Treasures, Ruins, Shelters that are junk.
fn is_junk(c: CardId) -> bool {
    c == id::CURSE
        || cards::is(c, cards::RUINS)
        || c == id::HOVEL
        || (cards::is(c, VICTORY) && !cards::is(c, ACTION) && !cards::is(c, TREASURE))
}

impl SetDefaults for Adventures {
    fn select(s: &Strategy, view: &PlayerView, _decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice]) -> Option<Choice> {
        match source {
            // Inheritance: the Action to set under the Estate token (the Prince's rule: the strategy's own play rules, then the most useful).
            id::INHERITANCE if act == Act::SetAside && from == Zone::Supply => Some(super::promo::choose_prince_card(s, view, choices)),
            // Pilgrimage: copies of the cards the strategy gains (up to three, the best by the gain list), then any other good card.
            id::PILGRIMAGE if act == Act::Reveal => Some(pilgrimage_pick(s, view, choices)),
            // Gear: set aside what cannot be used this turn and will be good next turn.
            id::GEAR if act == Act::SetAside => Some(gear_pick(view, choices)),
            // Artificer: the discards, by the fast rule (the turn search chooses them in `decide` when the strategy searches its play).
            id::ARTIFICER if act == Act::Discard && from == Zone::Hand => Some(artificer_pick(s, view, choices)),
            // Miser's Copper and other forced single picks.
            _ if act == Act::Tavern => Some(iter_cards(choices).next().map(Choice::Card).unwrap_or(Choice::Pass)),
            _ => None,
        }
    }

    fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        match card {
            id::AMULET => Some(default_amulet(s, view, choices)),
            id::MISER => Some(default_miser(view, choices)),
            id::QUEST => Some(default_quest(view, choices)),
            id::TEACHER => Some(default_teacher(view, choices)),
            _ => None,
        }
    }

    fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId) -> Option<Choice> {
        Some(match decision.kind {
            DecisionKind::YesNo { act: Act::Call } => default_call(s, view, decision, source),
            // Wine Merchant: discarded from the mat whenever the rules allow it (the card goes back into the deck cycle).
            DecisionKind::YesNo { act: Act::Discard } if source == id::WINE_MERCHANT => Choice::Yes,
            // An exchange is always taken, unless a rule says otherwise (rules are read first, by the caller).
            DecisionKind::YesNo { act: Act::Exchange } => Choice::Yes,
            // Travelling Fair: a gained card goes onto the deck when it is worth drawing again soon, not a card that is dead weight.
            DecisionKind::YesNo { act: Act::Topdeck } if source == id::TRAVELLING_FAIR => {
                let c = decision.subject;
                if is_junk(c) || c == id::COPPER {
                    Choice::No
                } else {
                    Choice::Yes
                }
            }
            // Raze: trash a card from the hand the trash rules want gone; else trash the Raze itself.
            DecisionKind::YesNo { act: Act::Trash } if source == id::RAZE => {
                if view.hand().iter().any(|(c, _)| s.wants_trash(view, c)) { Choice::No } else { Choice::Yes }
            }
            // Messenger: put the deck into the discard pile when what is there is better than what is left of the deck.
            DecisionKind::YesNo { .. } if source == id::MESSENGER => {
                let avg = |c: &dominion_engine::Counts| {
                    let n = c.total().max(1) as f64;
                    c.iter().map(|(card, k)| Strategy::card_value(card) as f64 * k as f64).sum::<f64>() / n
                };
                let deck = view.deck();
                let d = *view.discard();
                if d.total() > 0 && avg(&d) > avg(&deck) { Choice::Yes } else { Choice::No }
            }
            _ => return None,
        })
    }

    /// Soldier and Giant-like attacks are blocked; nothing to decline.
    fn start_order_group(card: CardId) -> Option<u8> {
        // Calls and returns first (they change the hand), then the plain draws.
        match card {
            id::GEAR => Some(0),
            id::GUIDE | id::RATCATCHER | id::TRANSMOGRIFY | id::TEACHER => Some(3),
            id::AMULET | id::DUNGEON => Some(4),
            _ => None,
        }
    }

    /// Whether the Action `card` is worth playing by default.
    fn worth_playing(_s: &Strategy, view: &PlayerView, card: CardId) -> bool {
        match card {
            // A Raze with nothing to trash but itself is still a (weak) cantrip; Disciple/Storyteller need something to work with.
            id::DISCIPLE => view.hand().iter().any(|(c, _)| c != id::DISCIPLE && cards::is(c, ACTION)),
            _ => true,
        }
    }
}

/// Where to put token `tok` (`state::TOK_*`) among the offered piles: the strategy's `[[token]]` rules first, then the defaults of the
/// plan (the Action pile the player owns most of, ...).
pub(super) fn choose_token(s: &Strategy, view: &PlayerView, tok: usize, choices: &[Choice]) -> Choice {
    for (t, pile, cond) in &s.token {
        if *t == tok && has_card(choices, *pile) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *pile)) {
            return Choice::Card(*pile);
        }
    }
    default_token_pile(s, view, tok, choices)
}

/// Copies of the pile `pile` the player owns (the Knights marker: any Knight).
fn owned(view: &PlayerView, pile: CardId) -> u32 {
    let mine = view.my_cards();
    match dominion_engine::sets::dark_ages::MixedPile::of_marker(pile) {
        Some(p) => p.members().map(|m| mine.get(m) as u32).sum(),
        None if pile == id::SAUNA => mine.get(id::SAUNA) as u32 + mine.get(id::AVANTO) as u32,
        None => mine.get(pile) as u32,
    }
}

/// Position of the pile's first card in the gain list (the pile of Knights: its best Knight), `usize::MAX` if it is not listed.
fn list_rank(s: &Strategy, pile: CardId) -> usize {
    match dominion_engine::sets::dark_ages::MixedPile::of_marker(pile) {
        Some(p) => p.members().filter_map(|m| s.buy_rank_of(m)).min().unwrap_or(usize::MAX),
        None => s.buy_rank_of(pile).unwrap_or(usize::MAX),
    }
}

/// How well token `tok` suits the card of `pile` (lower = better): +Action on a non-terminal, +Card on a terminal draw.
fn token_fit(tok: usize, pile: CardId) -> u8 {
    if dominion_engine::sets::dark_ages::MixedPile::of_marker(pile).is_some() {
        return 1;
    }
    let d = cards::def(pile);
    match tok {
        TOK_ACTION => (d.actions == 0) as u8,
        TOK_CARD => {
            if d.actions == 0 && d.cards > 0 {
                0
            } else if d.cards > 0 {
                1
            } else {
                2
            }
        }
        _ => 0,
    }
}

fn default_token_pile(s: &Strategy, view: &PlayerView, tok: usize, choices: &[Choice]) -> Choice {
    // Ferry: the best Action pile the gain list still wants.
    if tok == TOK_COST {
        let wanted = iter_cards(choices).filter(|&c| view.supply(c) > 0).min_by_key(|&c| (list_rank(s, c), c));
        if let Some(c) = wanted.filter(|&c| list_rank(s, c) != usize::MAX) {
            return Choice::Card(c);
        }
    }
    // Plan: a pile the strategy gains (so there is something to trash for), the one it owns most of.
    let key = |c: CardId| {
        let listed = list_rank(s, c) != usize::MAX;
        let fit = token_fit(tok, c);
        (
            tok == TOK_TRASH && !listed,
            fit,
            std::cmp::Reverse(owned(view, c)),
            list_rank(s, c),
            std::cmp::Reverse(cards::cost(c)),
            c,
        )
    };
    iter_cards(choices).min_by_key(|&c| key(c)).map(Choice::Card).unwrap_or(choices[0])
}

/// The default decision for calling a Reserve card (`decision.subject`), when no `[[call]]` rule speaks.
fn default_call(s: &Strategy, view: &PlayerView, decision: &Decision, card: CardId) -> Choice {
    let yes = match card {
        // Coin of the Realm: when the Actions are used up and an Action waits in the hand.
        id::COIN_OF_THE_REALM => view.turn().actions == 0 && view.hand().iter().any(|(c, _)| cards::is(c, ACTION)),
        // Royal Carriage: replay an Action that draws, pays or attacks (not a village or a thing that does nothing twice).
        id::ROYAL_CARRIAGE => {
            let d = cards::def(decision.target);
            decision.target != 0 && d.plays == 0 && (d.cards + d.coins + d.buys > 0 || cards::is(decision.target, cards::ATTACK))
        }
        // Duplicate: a copy of a card the gain list wants, or of any card worth having (a Treasure or an Action, not junk).
        id::DUPLICATE => {
            let c = decision.target;
            c != 0 && (s.lists(c) || (!is_junk(c) && (cards::is(c, TREASURE) || cards::is(c, ACTION)) && c != id::COPPER))
        }
        // Guide: a hand that is mostly dead cards, or that a handsize attack has cut down.
        id::GUIDE => {
            let dead = view.hand().iter().filter(|&(c, _)| is_junk(c)).map(|(_, n)| n as u32).sum::<u32>();
            dead >= 3 || (view.hand().total() <= 3 && view.deck_size() >= 5)
        }
        // Ratcatcher: when the trash rules want a card of the hand gone.
        id::RATCATCHER => view.hand().iter().any(|(c, _)| s.wants_trash(view, c)),
        // Transmogrify: trash something the rules want gone, or a card that turns into a better listed one.
        id::TRANSMOGRIFY => view.hand().iter().any(|(c, _)| {
            s.wants_trash(view, c) || s.best_gain_rank(view, view.cost(c) + 1, dominion_engine::Filter::Any, false, cards::potion_cost(c)).is_some_and(|r| {
                // Something listed that costs more than the card.
                s.buy.get(r).is_some_and(|(g, _)| cards::cost(*g) > cards::cost(c))
            })
        }),
        // Teacher: while one of the four tokens has no pile yet.
        id::TEACHER => {
            let t = view.tokens_of(view.me());
            [TOK_CARD, TOK_ACTION, TOK_BUY, TOK_COIN].iter().any(|&i| t[i] == 0)
        }
        _ => true,
    };
    if yes { Choice::Yes } else { Choice::No }
}

/// The stated `[[call]]` rule for the Reserve card `card`: `Some(true)` if a rule for it holds, `Some(false)` if there are rules and none does,
/// `None` if there is no rule for it.
pub(super) fn stated_call(s: &Strategy, view: &PlayerView, card: CardId) -> Option<bool> {
    let mut any = false;
    for (c, cond) in &s.call {
        if *c == card {
            any = true;
            if cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)) {
                return Some(true);
            }
        }
    }
    any.then_some(false)
}

/// The stated `[[exchange]]` rule for the Traveller `card`.
pub(super) fn stated_exchange(s: &Strategy, view: &PlayerView, card: CardId) -> Option<bool> {
    let mut any = false;
    for (c, cond) in &s.exchange {
        if *c == card {
            any = true;
            if cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)) {
                return Some(true);
            }
        }
    }
    any.then_some(false)
}

fn mode_index(card: CardId, pred: impl Fn(&ModeOpt) -> bool, choices: &[Choice]) -> Option<Choice> {
    find_mode(card, choices, pred)
}

/// Amulet: trash a card the rules want gone; else a Silver when the gain list wants Silver; else +$1.
fn default_amulet(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let trash = mode_index(id::AMULET, is_trash_hand, choices);
    let silver = mode_index(id::AMULET, is_gain, choices);
    let coins = mode_index(id::AMULET, is_coins, choices);
    if view.hand().iter().any(|(c, _)| s.wants_trash(view, c)) && view.hand().total() > 1 {
        if let Some(t) = trash {
            return t;
        }
    }
    if s.lists(id::SILVER) && view.supply(id::SILVER) > 0 {
        if let Some(g) = silver {
            return g;
        }
    }
    coins.or(silver).unwrap_or(choices[0])
}

/// Miser: a Copper onto the mat while there are few there and the hand holds one; else cash the mat in.
fn default_miser(view: &PlayerView, choices: &[Choice]) -> Choice {
    let put = mode_index(id::MISER, |o| matches!(o, ModeOpt::TavernCopper), choices);
    let cash = mode_index(id::MISER, |o| matches!(o, ModeOpt::CoinsPerTavernCopper), choices);
    let on_mat = view.tavern_of(view.me()).get(id::COPPER);
    let in_hand = view.hand().get(id::COPPER);
    if in_hand > 0 && on_mat < in_hand + 2 {
        return put.or(cash).unwrap_or(choices[0]);
    }
    cash.or(put).unwrap_or(choices[0])
}

/// Quest: two Curses; else an Attack in the hand; else six cards when nothing in the hand is worth keeping; else nothing.
fn default_quest(view: &PlayerView, choices: &[Choice]) -> Choice {
    let hand = view.hand();
    let pick = |i: u8| choices.contains(&Choice::Mode(i)).then_some(Choice::Mode(i));
    if hand.get(id::CURSE) >= 2 {
        if let Some(c) = pick(1) {
            return c;
        }
    }
    if hand.count_type(cards::ATTACK) > 0 {
        if let Some(c) = pick(0) {
            return c;
        }
    }
    if hand.total() >= 6 && hand.iter().all(|(c, _)| is_junk(c) || view.turn().actions == 0) {
        if let Some(c) = pick(2) {
            return c;
        }
    }
    pick(3).unwrap_or(choices[0])
}

/// Teacher: the first of the four tokens that has no pile yet (+1 Card, +1 Action, +$1, +1 Buy), else +1 Action.
fn default_teacher(view: &PlayerView, choices: &[Choice]) -> Choice {
    let t = view.tokens_of(view.me());
    for tok in [TOK_CARD, TOK_ACTION, TOK_COIN, TOK_BUY] {
        let ch = Choice::Mode(tok as u8 - TOK_CARD as u8);
        if t[tok] == 0 && choices.contains(&ch) {
            return ch;
        }
    }
    choices.iter().copied().find(|c| matches!(c, Choice::Mode(_))).unwrap_or(choices[0])
}

/// Pilgrimage: a copy of each of up to three cards in play the gain list lists (best first); other cards worth having only when fewer than
/// three are listed. Picks come in card order, so this answers "is this still wanted": the lowest wanted card first, `Pass` when done.
fn pilgrimage_pick(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let score = |c: CardId| -> (u8, usize, std::cmp::Reverse<i32>, CardId) {
        let listed = s.gain_rank(view, c);
        let junk = is_junk(c) || c == id::COPPER;
        (if listed.is_some() { 0 } else if junk { 2 } else { 1 }, listed.unwrap_or(usize::MAX), std::cmp::Reverse(Strategy::card_value(c)), c)
    };
    // The three best of the cards still offered (picks are made in card order, each name once); the lowest card id among them is taken now.
    let wanted = |c: CardId| score(c).0 < 2 && iter_cards(choices).filter(|&o| score(o) < score(c)).count() < 3;
    match iter_cards(choices).filter(|&c| wanted(c)).min() {
        Some(c) => Choice::Card(c),
        None => Choice::Pass,
    }
}

/// The most Artificer discards a candidate list holds (one per number of cards that a listed card costs).
pub(crate) const ARTIFICER_MAX_CANDIDATES: usize = 16;

/// The discard sets worth weighing at an Artificer selection, from the cheapest cards of the hand up (see `Strategy::artificer_candidates`).
pub(crate) struct ArtificerCandidates {
    /// How many cards each candidate discards in addition to those picked already (the first is 0: stop here).
    pub more: [u8; ARTIFICER_MAX_CANDIDATES],
    /// The cards each one discards in addition, by card.
    pub sets: [Counts; ARTIFICER_MAX_CANDIDATES],
    pub len: usize,
}

/// How much a card in the hand is worth keeping for this turn's Buy phase (lower = discard sooner): nothing for dead cards, the coins of a
/// Treasure, and a little more than the same for an Action that can still be played (Artificer gives +1 Action).
fn artificer_keep_value(c: CardId) -> i32 {
    let d = cards::def(c);
    if is_junk(c) {
        -100
    } else if cards::is(c, TREASURE) {
        10 * d.coins as i32
    } else if cards::is(c, ACTION) {
        12 + 10 * d.coins as i32 + 8 * d.cards as i32 + 6 * d.actions as i32
    } else {
        0
    }
}

impl Strategy {
    /// The Artificer discard sets worth weighing: the `m` least valuable cards of the hand (dead cards first, then by what they give this
    /// turn), for each `m` with a listed card costing exactly the number discarded in all (those picked already count), and `m = 0` (stop
    /// here). Each is the continuation of a plan picked in card order: its cards are not below the last pick.
    pub(crate) fn artificer_candidates(&self, view: &PlayerView) -> ArtificerCandidates {
        let (done, last) = view.select_progress().unwrap_or((0, 0));
        let hand = *view.hand();
        // The hand's cards, least valuable first (ties by card): at most 32 are looked at.
        let mut order = [0 as CardId; 32];
        let mut n = 0usize;
        let mut kinds = [(0i32, 0 as CardId); 32];
        let mut k = 0usize;
        for (c, cnt) in hand.iter() {
            if k < kinds.len() {
                kinds[k] = (artificer_keep_value(c), c);
                k += 1;
            }
            let _ = cnt;
        }
        kinds[..k].sort_unstable();
        for &(_, c) in &kinds[..k] {
            for _ in 0..hand.get(c) {
                if n < order.len() {
                    order[n] = c;
                    n += 1;
                }
            }
        }
        let mut out = ArtificerCandidates { more: [0; ARTIFICER_MAX_CANDIDATES], sets: [Counts::EMPTY; ARTIFICER_MAX_CANDIDATES], len: 1 };
        let mut set = Counts::EMPTY;
        let mut lowest = CardId::MAX;
        for m in 1..=n {
            let c = order[m - 1];
            set.add(c, 1);
            lowest = lowest.min(c);
            if lowest < last {
                // This plan would have to pick below the last pick already made: not a continuation.
                break;
            }
            let cost = done as usize + m;
            if cost > u8::MAX as usize || out.len >= ARTIFICER_MAX_CANDIDATES {
                break;
            }
            if self.best_gain_rank(view, cost as u8, dominion_engine::Filter::Any, true, false).is_some() {
                out.more[out.len] = m as u8;
                out.sets[out.len] = set;
                out.len += 1;
            }
        }
        out
    }
}

/// Artificer's discards by rule, without a search: of the candidate sets (`artificer_candidates`), the one for which the card gained onto the
/// deck (the best listed card costing exactly the discards) plus the best listed card the rest of the hand still buys is worth most on the
/// gain list's scale; stopping wins ties (no discard is free). Picks come in card order, so this answers with the lowest card of the plan, or
/// `Pass` when the plan is to stop. Chosen again after each pick, from what is left of the hand (the same plan every time).
fn artificer_pick(s: &Strategy, view: &PlayerView, choices: &[Choice]) -> Choice {
    let cands = s.artificer_candidates(view);
    if cands.len <= 1 {
        return Choice::Pass;
    }
    let done = view.select_progress().map_or(0, |(c, _)| c) as usize;
    let hand = *view.hand();
    let buys = view.buys_left() > 0;
    let mut best = (f64::NEG_INFINITY, 0usize);
    for i in 0..cands.len {
        let cost = done + cands.more[i] as usize;
        let mut score = 0.0;
        if cost <= u8::MAX as usize {
            if let Some(r) = s.best_gain_rank(view, cost as u8, dominion_engine::Filter::Any, true, false) {
                score += s.rank_weight(r);
            }
        }
        if buys {
            let mut money = view.turn().coins as u32 + view.coffers() as u32;
            for (c, n) in hand.iter() {
                if cards::is(c, TREASURE) {
                    money += cards::def(c).coins as u32 * n.saturating_sub(cands.sets[i].get(c)) as u32;
                }
            }
            if let Some(r) = s.reachable_rank_with(view, money, view.turn().potions) {
                score += s.rank_weight(r);
            }
        }
        if score > best.0 {
            best = (score, i);
        }
    }
    match cands.sets[best.1].iter().map(|(c, _)| c).find(|&c| has_card(choices, c)) {
        Some(c) if best.1 > 0 => Choice::Card(c),
        _ => Choice::Pass,
    }
}

/// Gear: set aside Actions that cannot be played this turn (no Actions left), best first; Treasures beyond what the turn can use; else nothing.
fn gear_pick(view: &PlayerView, choices: &[Choice]) -> Choice {
    if view.turn().actions == 0 {
        if let Some(c) = iter_cards(choices).filter(|&c| cards::is(c, ACTION)).max_by_key(|&c| (cards::cost(c), c)) {
            return Choice::Card(c);
        }
    }
    let coins_in_play: u32 = view.in_play().iter().filter(|&(c, _)| view.is_treasure(c)).map(|(c, n)| cards::def(c).coins as u32 * n as u32).sum();
    let hand_coins: u32 = view.hand().iter().filter(|&(c, _)| view.is_treasure(c)).map(|(c, n)| cards::def(c).coins as u32 * n as u32).sum();
    if view.turn().coins as u32 + coins_in_play * 0 + hand_coins >= 11 {
        if let Some(c) = iter_cards(choices).filter(|&c| cards::is(c, TREASURE)).max_by_key(|&c| (cards::def(c).coins, c)) {
            return Choice::Card(c);
        }
    }
    Choice::Pass
}
