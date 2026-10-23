//! Rising Sun: 25 kingdom cards (Change, which needs Debt, came first and sits among the Empires ids; the other 24 follow the Promo cards), the
//! Omens and the Prophecies (pseudo-cards with Sun tokens), the Shadow cards (played from the deck), on-play Debt and 10 Events.
//! See `docs/rising-sun-plan.md` for the design and the rulings.

use super::prelude::*;
use crate::cards::{rs, with_debt, OMEN, SHADOW};
use crate::state::{
    GameConfig, PlayerState, R_KINTSUGI, R_SHADOW, R_TRIBUTE, GF_ANY_COST, GF_EXCL, P_RS_GOLD, TypeSet,
};

/// Card ids of this set. Change is the first kingdom card (it came with Empires' Debt and sits after its ids); the others are the highest
/// kingdom ids, before the Ruins and Knights. The Events follow Summon, the Prophecies are the highest ids of all.
pub mod ids {
    use crate::cards::CardId;
    pub const CHANGE: CardId = 257;
    pub const MOUNTAIN_SHRINE: CardId = 358;
    pub const DAIMYO: CardId = 359;
    pub const ARTIST: CardId = 360;
    pub const FISHMONGER: CardId = 361;
    pub const SNAKE_WITCH: CardId = 362;
    pub const ARISTOCRAT: CardId = 363;
    pub const CRAFTSMAN: CardId = 364;
    pub const RIVERBOAT: CardId = 365;
    pub const ROOT_CELLAR: CardId = 366;
    pub const ALLEY: CardId = 367;
    pub const NINJA: CardId = 368;
    pub const POET: CardId = 369;
    pub const RIVER_SHRINE: CardId = 370;
    pub const RUSTIC_VILLAGE: CardId = 371;
    pub const GOLD_MINE: CardId = 372;
    pub const IMPERIAL_ENVOY: CardId = 373;
    pub const KITSUNE: CardId = 374;
    pub const LITTER: CardId = 375;
    pub const RICE_BROKER: CardId = 376;
    pub const RONIN: CardId = 377;
    pub const TANUKI: CardId = 378;
    pub const TEA_HOUSE: CardId = 379;
    pub const SAMURAI: CardId = 380;
    pub const RICE: CardId = 381;
    // The Events, after Summon.
    pub const AMASS: CardId = 592;
    pub const ASCETICISM: CardId = 593;
    pub const CONTINUE: CardId = 594;
    pub const CREDIT: CardId = 595;
    pub const FORESIGHT: CardId = 596;
    pub const GATHER: CardId = 597;
    pub const KINTSUGI: CardId = 598;
    pub const PRACTICE: CardId = 599;
    pub const RECEIVE_TRIBUTE: CardId = 600;
    pub const SEA_TRADE: CardId = 601;
    // The Prophecies, the highest ids.
    pub const APPROACHING_ARMY: CardId = 687;
    pub const BIDING_TIME: CardId = 688;
    pub const BUREAUCRACY: CardId = 689;
    pub const DIVINE_WIND: CardId = 690;
    pub const ENLIGHTENMENT: CardId = 691;
    pub const FLOURISHING_TRADE: CardId = 692;
    pub const GOOD_HARVEST: CardId = 693;
    pub const GREAT_LEADER: CardId = 694;
    pub const GROWTH: CardId = 695;
    pub const HARSH_WINTER: CardId = 696;
    pub const KIND_EMPEROR: CardId = 697;
    pub const PANIC: CardId = 698;
    pub const PROGRESS: CardId = 699;
    pub const RAPID_EXPANSION: CardId = 700;
    pub const SICKNESS: CardId = 701;
}

/// The Shadow cards (playable from the deck).
pub const SHADOWS: [CardId; 5] = [ids::ALLEY, ids::FISHMONGER, ids::NINJA, ids::RONIN, ids::TANUKI];

/// An Event: no types, no bonuses (what it does is `event_effect`); its cost is the cost to buy it (`with_debt` adds the Debt part).
const fn event(name: &'static str, cost: u8) -> CardDef {
    rs(c(name, cost, 0, 0, 0, 0, 0, 0))
}

/// A Prophecy: no cost, nothing to buy.
const fn prophecy(name: &'static str) -> CardDef {
    rs(c(name, 0, 0, 0, 0, 0, 0, 0))
}

const CARDS: &[(CardId, CardDef)] = &[
    // If the player has Debt: +$3. Otherwise trash a card from the hand and gain a card costing more $, taking Debt for the difference.
    (ids::CHANGE, rs(order_sensitive(c("Change", 4, ACTION, 0, 0, 0, 0, 0)))), // trashes any card of the hand (may be the +Actions card), or +$3
    (ids::MOUNTAIN_SHRINE, rs(with_debt(order_sensitive(c("Mountain Shrine", 0, ACTION | OMEN, 2, 0, 0, 0, 0)), 5))), // may trash a card of the hand
    (ids::DAIMYO, rs(with_debt(order_sensitive(plays(c("Daimyo", 0, ACTION | COMMAND, 0, 0, 1, 1, 0), 2)), 6))), // replays the next non-Command Action: which one is a choice
    (ids::ARTIST, rs(with_debt(order_sensitive(c("Artist", 0, ACTION, 0, 0, 0, 1, 0)), 8))), // draws per card with one copy in play
    (ids::FISHMONGER, rs(choice_free(c("Fishmonger", 2, ACTION | SHADOW, 1, 0, 0, 0, 1)))),
    (ids::SNAKE_WITCH, rs(order_sensitive(c("Snake Witch", 2, ACTION | ATTACK, 0, 0, 1, 1, 0)))), // reveals a hand with no duplicates: drawing first matters
    (ids::ARISTOCRAT, rs(order_sensitive(c("Aristocrat", 3, ACTION, 0, 0, 0, 0, 0)))), // +3 of something that depends on the Aristocrats in play
    (ids::CRAFTSMAN, rs(has_choice(c("Craftsman", 3, ACTION, 0, 0, 0, 0, 0)))), // gains a card up to $5, takes 2 Debt
    (ids::RIVERBOAT, rs(choice_free(c("Riverboat", 3, ACTION | DURATION, 0, 0, 0, 0, 0)))), // plays the set-aside card at the start of the next turn
    (ids::ROOT_CELLAR, rs(has_choice(c("Root Cellar", 3, ACTION, 0, 0, 3, 1, 0)))), // takes 3 Debt: can cost more than it gives
    (ids::ALLEY, rs(order_sensitive(c("Alley", 4, ACTION | SHADOW, 0, 0, 1, 1, 0)))), // discards any card of the hand
    (ids::NINJA, rs(choice_free(c("Ninja", 4, ACTION | ATTACK | SHADOW, 0, 0, 1, 0, 0)))),
    (ids::POET, rs(order_sensitive(c("Poet", 4, ACTION | OMEN, 0, 0, 1, 1, 0)))), // reveals the top card: a miss stays known for a later draw
    (ids::RIVER_SHRINE, rs(order_sensitive(c("River Shrine", 4, ACTION | OMEN, 0, 0, 0, 0, 0)))), // trashes up to 2 cards of the hand
    (ids::RUSTIC_VILLAGE, rs(order_sensitive(c("Rustic Village", 4, ACTION | OMEN, 0, 0, 1, 2, 0)))), // may discard 2 cards of the hand
    (ids::GOLD_MINE, rs(has_choice(c("Gold Mine", 5, ACTION, 0, 0, 1, 1, 1)))), // may gain a Gold and take 4 Debt
    (ids::IMPERIAL_ENVOY, rs(has_choice(c("Imperial Envoy", 5, ACTION, 0, 0, 5, 0, 1)))), // takes 2 Debt: can cost more than it gives
    (ids::KITSUNE, rs(has_choice(c("Kitsune", 5, ACTION | ATTACK | OMEN, 0, 0, 0, 0, 0)))), // chooses two different options
    (ids::LITTER, rs(has_choice(c("Litter", 5, ACTION, 0, 0, 2, 2, 0)))), // takes 1 Debt: can cost more than it gives
    (ids::RICE_BROKER, rs(order_sensitive(c("Rice Broker", 5, ACTION, 0, 0, 0, 1, 0)))), // trashes a card of the hand
    (ids::RONIN, rs(order_sensitive(c("Ronin", 5, ACTION | SHADOW, 0, 0, 0, 0, 0)))), // draws up to 7 cards in hand
    (ids::TANUKI, rs(order_sensitive(c("Tanuki", 5, ACTION | SHADOW, 0, 0, 0, 0, 0)))), // trashes a card of the hand
    (ids::TEA_HOUSE, rs(choice_free(c("Tea House", 5, ACTION | OMEN, 2, 0, 1, 1, 0)))),
    (ids::SAMURAI, rs(choice_free(c("Samurai", 6, ACTION | DURATION | ATTACK, 0, 0, 0, 0, 0)))), // +$1 at the start of every turn, stays in play
    (ids::RICE, rs(choice_free(c("Rice", 7, TREASURE, 0, 0, 0, 0, 1)))), // +$1 per type among the cards in play (`Rice` is played after the other Treasures)
    // The Events.
    (ids::AMASS, event("Amass", 2)),
    (ids::ASCETICISM, event("Asceticism", 2)),
    (ids::CONTINUE, rs(with_debt(c("Continue", 0, 0, 0, 0, 0, 0, 0), 8))),
    (ids::CREDIT, event("Credit", 2)),
    (ids::FORESIGHT, event("Foresight", 2)),
    (ids::GATHER, event("Gather", 7)),
    (ids::KINTSUGI, event("Kintsugi", 3)),
    (ids::PRACTICE, event("Practice", 3)),
    (ids::RECEIVE_TRIBUTE, event("Receive Tribute", 5)),
    (ids::SEA_TRADE, event("Sea Trade", 4)),
    // The Prophecies.
    (ids::APPROACHING_ARMY, prophecy("Approaching Army")),
    (ids::BIDING_TIME, prophecy("Biding Time")),
    (ids::BUREAUCRACY, prophecy("Bureaucracy")),
    // Disabled for now (it replaces every Kingdom pile mid-game): not dealt, and a kingdom may not name it. The code stays below.
    (ids::DIVINE_WIND, crate::cards::todo(prophecy("Divine Wind"))),
    (ids::ENLIGHTENMENT, prophecy("Enlightenment")),
    (ids::FLOURISHING_TRADE, prophecy("Flourishing Trade")),
    (ids::GOOD_HARVEST, prophecy("Good Harvest")),
    (ids::GREAT_LEADER, prophecy("Great Leader")),
    (ids::GROWTH, prophecy("Growth")),
    (ids::HARSH_WINTER, prophecy("Harsh Winter")),
    (ids::KIND_EMPEROR, prophecy("Kind Emperor")),
    (ids::PANIC, prophecy("Panic")),
    (ids::PROGRESS, prophecy("Progress")),
    (ids::RAPID_EXPANSION, prophecy("Rapid Expansion")),
    (ids::SICKNESS, prophecy("Sickness")),
];

/// The Rising Sun cards (see the module docs of `sets`).
pub struct RisingSun;

impl SetRules for RisingSun {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            // Two different options, done in the order listed.
            ids::KITSUNE => &[ModeOpt::Actions(2), ModeOpt::Coins(2), ModeOpt::CurseOthers, ModeOpt::Gain(id::SILVER, Dest::Discard)],
            // Sickness at the start of the turn: a Curse onto the deck, or discard 3 cards.
            ids::SICKNESS => &[ModeOpt::Gain(id::CURSE, Dest::DeckTop), ModeOpt::DiscardFromHand(3)],
            _ => &[],
        }
    }

    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::SAMURAI => (0, 0, 0, 1),
            _ => (0, 0, 0, 0),
        }
    }

    /// Riverboat plays a card; the Prophecies that act at the start of the turn put cards into the hand, gain or play: order can matter.
    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, ids::RIVERBOAT | ids::BIDING_TIME | ids::KIND_EMPEROR | ids::SICKNESS | ids::RAPID_EXPANSION)
    }

    /// Rapid Expansion's entries are named by the card they play, and stand for every copy of it.
    fn start_entry_named_by_arg(card: CardId) -> bool {
        card == ids::RAPID_EXPANSION
    }

    fn start_entry_repeats(card: CardId) -> bool {
        card == ids::RAPID_EXPANSION
    }

    /// The Prophecies that happen at the start of every turn.
    fn turn_start_entries(g: &mut GameState) {
        let pi = g.turn.player as usize;
        match g.rs.active {
            ids::BIDING_TIME => {
                if !g.players[pi].aside.is_empty() {
                    g.turn.start_pending.push(PendingDuration { card: ids::BIDING_TIME, times: 1, arg: 0, used: 0 });
                }
            }
            ids::KIND_EMPEROR => g.turn.start_pending.push(PendingDuration { card: ids::KIND_EMPEROR, times: 1, arg: 0, used: 0 }),
            ids::SICKNESS => g.turn.start_pending.push(PendingDuration { card: ids::SICKNESS, times: 1, arg: 0, used: 0 }),
            ids::RAPID_EXPANSION => {
                for (c, k) in g.players[pi].aside.iter() {
                    g.turn.start_pending.push(PendingDuration { card: ids::RAPID_EXPANSION, times: k, arg: c, used: 0 });
                }
            }
            _ => {}
        }
    }

    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
        match e.card {
            // The hand set aside at the start of the last Clean-up comes back.
            ids::BIDING_TIME => {
                let ps = &mut g.players[p as usize];
                let aside = ps.aside.take_all();
                sink.depth(0);
                sink.event(Event::Bonus { player: p, source: e.card, actions: 0, buys: 0, coins: 0, vp: 0 });
                for (c, n) in aside.iter() {
                    ps.hand.add(c, n);
                    for _ in 0..n {
                        sink.event(Event::Draw { player: p, card: c });
                    }
                }
                true
            }
            ids::KIND_EMPEROR => {
                sink.depth(0);
                sink.event(Event::Bonus { player: p, source: e.card, actions: 0, buys: 0, coins: 0, vp: 0 });
                g.stack.push(Frame { depth: 1, gf: GF_ANY_COST, ..gain_frame(p, e.card, u8::MAX, Filter::Action, Dest::Hand) });
                true
            }
            ids::SICKNESS => {
                sink.depth(0);
                sink.event(Event::Bonus { player: p, source: e.card, actions: 0, buys: 0, coins: 0, vp: 0 });
                g.stack.push(Frame { depth: 1, ..mode_frame(p, e.card, 1) });
                true
            }
            // The card set aside is played now, from set aside into play (no Action used).
            ids::RAPID_EXPANSION => {
                if g.players[p as usize].aside.remove(e.arg) {
                    g.players[p as usize].in_play.add(e.arg, 1);
                    sink.depth(0);
                    sink.event(Event::StartPlay { player: p, source: e.card, card: e.arg });
                    g.resolve_effects(e.arg, p, 0, sink);
                }
                true
            }
            _ => false,
        }
    }

    /// Riverboat plays its set-aside card, leaving it there (once for each play of the Riverboat); Samurai comes back every turn.
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) {
        match e.card {
            ids::RIVERBOAT => {
                let card = g.rs.riverboat;
                if card != 0 {
                    for _ in 0..e.times {
                        g.stack.push(Frame { depth: 1, subject: card, count: 1, zone: Zone::Supply, ..Frame::new(K::PlayEffects, p, e.card) });
                    }
                }
            }
            ids::SAMURAI => g.push_duration_pending(p, e.card, e.times, 0),
            _ => {}
        }
    }

    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        match card {
            ids::CHANGE => g.change_play(p, sink),
            ids::MOUNTAIN_SHRINE => {
                // +Sun; may trash a card from the hand; then if there is an Action in the trash, +2 Cards. The check waits under the trash and
                // whatever it sets off (a Fortress that comes back to the hand is not in the trash any more).
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::MountainShrine));
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing));
                g.remove_sun(p, card, sink);
            }
            ids::DAIMYO => {
                // The next non-Command Action played this turn is replayed afterwards.
                if p == g.turn.player {
                    g.turn.rs.daimyo = g.turn.rs.daimyo.saturating_add(1);
                }
            }
            ids::ARTIST => {
                // +1 Card per card with exactly one copy in play (this one included while it is in play).
                let n = g.players[pi].in_play.iter().filter(|&(_, k)| k == 1).count().min(u8::MAX as usize) as u8;
                if n > 0 {
                    g.stack.push(draw_frame(p, card, n));
                }
            }
            ids::SNAKE_WITCH => {
                // After the +1 Card: with no duplicates in the hand, it may be revealed and the Snake Witch returned to its pile for Curses.
                g.stack.push(Frame { then: Then::SnakeWitch, ..Frame::new(K::Ask, p, card) });
            }
            ids::ARISTOCRAT => {
                // By the Aristocrats in play: 1 or 5 +3 Actions, 2 or 6 +3 Cards, 3 or 7 +$3, 4 or 8 +3 Buys, otherwise nothing.
                let n = g.players[pi].in_play.get(card);
                if (1..=8).contains(&n) {
                    match (n - 1) % 4 {
                        0 => {
                            g.turn.actions += 3;
                            sink.bonus(p, card, 3, 0, 0, 0);
                        }
                        1 => g.stack.push(draw_frame(p, card, 3)),
                        2 => {
                            g.turn.coins += 3;
                            sink.bonus(p, card, 0, 0, 3, 0);
                        }
                        _ => {
                            g.turn.buys += 3;
                            sink.bonus(p, card, 0, 3, 0, 0);
                        }
                    }
                }
            }
            ids::CRAFTSMAN => {
                g.stack.push(gain_frame(p, card, 5, Filter::Any, Dest::Discard));
                g.take_debt(p, card, 2, sink);
            }
            ids::ROOT_CELLAR => g.take_debt(p, card, 3, sink),
            ids::IMPERIAL_ENVOY => g.take_debt(p, card, 2, sink),
            ids::LITTER => g.take_debt(p, card, 1, sink),
            ids::ALLEY => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 1, 1, Then::Nothing)),
            ids::NINJA => g.discard_attack(card, sink),
            ids::SAMURAI => {
                g.discard_attack(card, sink);
                // Stays in play for the rest of the game (with the Throne Room / King's Court that played it twice).
                let ps = &mut g.players[pi];
                if ps.stays.get(card) < ps.in_play.get(card) {
                    ps.stays.add(card, 1);
                }
            }
            ids::POET => {
                g.stack.push(Frame { max: 1, then: Then::PoetCheck, ..Frame::new(K::RevealTop, p, card) });
                g.remove_sun(p, card, sink);
            }
            ids::RIVER_SHRINE => {
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 2, Then::Nothing));
                g.remove_sun(p, card, sink);
            }
            ids::RUSTIC_VILLAGE => {
                // +1 Card (drawn first), then you may discard 2 cards for +1 Card.
                g.stack.push(Frame { exact: true, ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 2, Then::DrawIfCount { count: 2, draw: 1 }) });
                g.remove_sun(p, card, sink);
            }
            ids::GOLD_MINE => g.stack.push(Frame { then: Then::GoldMine, ..Frame::new(K::Ask, p, card) }),
            ids::KITSUNE => {
                g.stack.push(mode_frame(p, card, 2));
                g.remove_sun(p, card, sink);
            }
            ids::RICE_BROKER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::RiceBroker)),
            ids::RONIN => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::DrawUpTo(7))),
            ids::TANUKI => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
            )),
            ids::TEA_HOUSE => g.remove_sun(p, card, sink),
            _ => {}
        }
    }

    /// River Shrine: "at the start of Clean-up, if you didn't gain any cards in your Buy phase this turn, gain a card costing up to $4", once for
    /// each play of it this turn (a Throne Room'd one gains twice). The condition is looked at once, so the gains of the shrines do not stop each other.
    fn offers_at_cleanup(g: &mut GameState) {
        if g.in_supply(ids::RIVER_SHRINE) {
            let p = g.turn.player;
            let n = g.turn.played.get(ids::RIVER_SHRINE);
            if n > 0 && g.turn.gained_in_buy == 0 {
                for _ in 0..n {
                    g.stack.push(gain_frame(p, ids::RIVER_SHRINE, 4, Filter::Any, Dest::Discard));
                }
            }
        }
    }

    /// Asceticism's overpay: trash that many cards from the hand.
    fn overpay<S: EventSink>(g: &mut GameState, p: u8, card: CardId, coins: u8, _potion: bool, depth: u8, _sink: &mut S) {
        if card == ids::ASCETICISM && coins > 0 {
            g.stack.push(Frame { depth, ..select(p, card, Zone::Hand, Act::Trash, Filter::Any, coins, coins, Then::Nothing) });
        }
    }
}

// -------------------------------------------------------------------------------------------
// Set-up
// -------------------------------------------------------------------------------------------

/// Whether `c` could be Riverboat's card in a game with `kingdom`: a non-Duration Action costing exactly $5 (no Potion, no Debt) that is not
/// in the Supply and needs no set-up of its own.
fn riverboat_ok(in_game: &dyn Fn(CardId) -> bool, c: CardId) -> bool {
    let ok_type = cards::is(c, ACTION) && !cards::is(c, DURATION) && !cards::is(c, cards::FATE) && !cards::is(c, cards::DOOM) && !cards::is(c, LOOTER);
    ok_type && cards::cost(c) == 5 && !cards::potion_cost(c) && cards::debt_cost(c) == 0 && cards::is_ready(c) && !in_game(c)
        && match c {
            // A member of the Knights pile: only when the Knights are not in the game; a bottom half of a split pile: only when its top half is not.
            _ if c >= cards::FIRST_PILE_CARD => crate::sets::dark_ages::KNIGHTS_PILE.members().contains(&c) && !in_game(id::KNIGHTS),
            _ if cards::is_split_bottom(c) => cards::split_top(c).is_some_and(|t| !in_game(t)),
            _ => cards::is_kingdom(c),
        }
}

/// The card Riverboat sets aside when none was named: a candidate picked by the seed (the same one for the same kingdom and seed).
pub fn default_riverboat(in_game: &dyn Fn(CardId) -> bool, seed: u64) -> CardId {
    let pool = || (cards::FIRST_KINGDOM..cards::NUM_ZONE_IDS as CardId).filter(|&c| riverboat_ok(in_game, c));
    let n = pool().count();
    if n == 0 {
        return 0;
    }
    let mut z = seed.wrapping_add(0x5249_5645_5242_4F41).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 32;
    pool().nth((z % n as u64) as usize).unwrap_or(0)
}

/// Rising Sun set-up: the Prophecy and its Sun tokens, the flags for the effects this kingdom has, Riverboat's card.
pub(crate) fn setup(g: &mut GameState, cfg: &GameConfig, n: usize) {
    // What is in the game: the Supply piles (the Bane too), the Events and the Prophecy named by the kingdom.
    let mut shadow = false;
    for &k in &cfg.kingdom {
        if cards::is_prophecy(k) && g.rs.prophecy == 0 {
            g.rs.prophecy = k;
            g.rs.suns = crate::state::sun_tokens_for(n);
        }
        shadow |= cards::is(k, SHADOW);
    }
    // (The Bane pile is in the Supply but not in the kingdom list.)
    shadow |= g.bane != 0 && cards::is(g.bane, SHADOW);
    if shadow {
        g.rs.flags |= R_SHADOW;
    }
    let has = |c: CardId| cfg.kingdom.contains(&c);
    if has(ids::KINTSUGI) {
        g.rs.flags |= R_KINTSUGI;
    }
    if has(ids::RECEIVE_TRIBUTE) {
        g.rs.flags |= R_TRIBUTE;
    }
    g.rs_update_hooks();
    if g.in_supply(ids::RIVERBOAT) {
        let in_game = |c: CardId| g.in_supply(c) || has(c);
        let card = cfg.riverboat.filter(|&c| riverboat_ok(&in_game, c)).unwrap_or_else(|| default_riverboat(&in_game, cfg.seed));
        g.rs.riverboat = card;
    }
}

// -------------------------------------------------------------------------------------------
// Events
// -------------------------------------------------------------------------------------------

/// Whether `c` can be gained now as a plain pile card of Rising Sun's Events (a pile with a card left, on top).
fn gainable(g: &GameState, c: CardId) -> bool {
    g.supply.get(c) > 0 && g.pile_open(c)
}

/// Whether the Rising Sun Event `e` is in the game and does something now (the common checks, `adventures::event_available`, are made first).
pub(crate) fn event_available(g: &GameState, e: CardId) -> bool {
    let p = g.turn.player as usize;
    match e {
        ids::AMASS => {
            g.actions_in_play(p as u8) == 0 && g.supply_cards().any(|c| gainable(g, c) && g.is_action(c) && g.cost(c) <= 5 && !cards::potion_cost(c) && cards::debt_cost(c) == 0)
        }
        ids::ASCETICISM => !g.players[p].hand.is_empty(),
        ids::CONTINUE => g.supply_cards().any(|c| {
            gainable(g, c) && cards::is(c, ACTION) && !cards::is(c, ATTACK) && g.cost(c) <= 4 && !cards::potion_cost(c) && cards::debt_cost(c) == 0
        }),
        // (With no Action anywhere the whole deck is revealed and discarded: a shuffle that is legal to ask for.)
        ids::FORESIGHT => true,
        ids::KINTSUGI => !g.players[p].hand.is_empty(),
        ids::PRACTICE => g.players[p].hand.iter().any(|(c, _)| g.is_action(c)) || g.shadow_playable(p as u8),
        ids::RECEIVE_TRIBUTE => g.turn.rs.gains >= 3 && g.supply_cards().any(|c| gainable(g, c) && cards::is(c, ACTION) && !g.players[p].in_play.has(c)),
        ids::SEA_TRADE => g.actions_in_play(p as u8) > 0,
        _ => true,
    }
}

/// What buying the Rising Sun Event `e` does, after its cost and Buy are paid.
pub(crate) fn event_effect<S: EventSink>(g: &mut GameState, e: CardId, p: u8, _sink: &mut S) {
    use Act::*;
    match e {
        // With no Action in play: an Action costing up to $5.
        ids::AMASS => {
            if g.actions_in_play(p) == 0 {
                g.stack.push(gain_frame(p, e, 5, Filter::Action, Dest::Discard));
            }
        }
        // Pay any amount of $ (up to the cards in hand) to trash that many cards: an overpay decision.
        ids::ASCETICISM => g.stack.push(Frame { depth: 1, ..Frame::new(K::Overpay, p, e) }),
        // A non-Attack Action costing up to $4; back to the Action phase to play it (the return waits under the gain's reactions).
        ids::CONTINUE => g.stack.push(Frame { then: Then::ContinueGain, ..gain_frame(p, e, 4, Filter::Types(TypeSet::ActionNotAttack), Dest::Discard) }),
        // An Action or Treasure costing up to $8; Debt equal to its cost.
        ids::CREDIT => g.stack.push(Frame { then: Then::CreditDebt, ..gain_frame(p, e, 8, Filter::ActionOrTreasure, Dest::Discard) }),
        ids::FORESIGHT => g.stack.push(Frame { filter: Filter::Action, max: 1, then: Then::ForesightSet, ..Frame::new(K::RevealUntil, p, e) }),
        // $3, then $4, then $5.
        ids::GATHER => {
            for cost in [5, 4, 3] {
                g.stack.push(Frame { exact: true, ..gain_frame(p, e, cost, Filter::Any, Dest::Discard) });
            }
        }
        ids::KINTSUGI => g.stack.push(select(p, e, Zone::Hand, Trash, Filter::Any, 1, 1, Then::KintsugiGain)),
        ids::PRACTICE => g.stack.push(select(p, e, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 2 })),
        // Up to 3 differently named Actions that are not in play, of any cost.
        ids::RECEIVE_TRIBUTE => {
            if g.turn.rs.gains >= 3 {
                g.stack.push(tribute_frame(p, e, 3, 0, 0));
            }
        }
        // +1 Card per Action in play (drawn first), then trash up to that many cards.
        ids::SEA_TRADE => {
            let n = g.actions_in_play(p);
            g.stack.push(select(p, e, Zone::Hand, Trash, Filter::Any, 0, n, Then::Nothing));
            g.stack.push(draw_frame(p, e, n));
        }
        _ => {}
    }
}

/// Receive Tribute's gain: an optional Action of any cost that is not in play and not one of the (up to two) already gained, with `left` gains still allowed.
fn tribute_frame(p: u8, source: CardId, left: u8, a: CardId, b: CardId) -> Frame {
    Frame {
        filter: Filter::NotInPlayAction, optional: true, count: left, subject: a, last: b, then: Then::TributeGain,
        gf: GF_ANY_COST | GF_EXCL, ..gain_frame(p, source, u8::MAX, Filter::NotInPlayAction, Dest::Discard)
    }
}

// -------------------------------------------------------------------------------------------
// The game: Sun tokens and Prophecies, Shadows, plays, gains
// -------------------------------------------------------------------------------------------

impl PlayerState {
    /// Whether a copy of `c` is somewhere in the deck (the known top, the unknown middle or the known bottom).
    pub fn deck_has(&self, c: CardId) -> bool {
        self.deck_unknown.has(c)
            || self.deck_known.cards[..self.deck_known.len as usize].contains(&c)
            || self.deck_known_bottom.cards[..self.deck_known_bottom.len as usize].contains(&c)
    }

    /// Takes one copy of `c` out of the deck, wherever it is; whether there was one.
    pub fn take_from_deck(&mut self, c: CardId) -> bool {
        let k = &mut self.deck_known;
        if let Some(i) = (0..k.len as usize).rev().find(|&i| k.cards[i] == c) {
            for j in i..k.len as usize - 1 {
                k.cards[j] = k.cards[j + 1];
            }
            k.len -= 1;
            return true;
        }
        if self.deck_unknown.remove(c) {
            return true;
        }
        let b = &mut self.deck_known_bottom;
        if let Some(i) = (0..b.len as usize).find(|&i| b.cards[i] == c) {
            for j in i..b.len as usize - 1 {
                b.cards[j] = b.cards[j + 1];
            }
            b.len -= 1;
            return true;
        }
        false
    }

    /// Shadow cards put at the bottom when the deck is shuffled: every Shadow in the unknown middle of the deck moves to the known bottom (a
    /// full bottom stack leaves the rest in the middle).
    pub fn sink_shadows(&mut self) {
        for &s in &SHADOWS {
            while self.deck_unknown.has(s) && (self.deck_known_bottom.len as usize) < crate::state::BOTTOM_CAP {
                self.deck_unknown.remove(s);
                self.deck_known_bottom.push_top(s);
            }
        }
    }
}

impl GameState {
    /// Change: with any Debt, +$3; otherwise trash a card from the hand and gain a card costing more $ than it (it costs the difference in
    /// Debt, taken when the card is gained: `Then::ChangeGain`).
    fn change_play<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        if self.players[p as usize].debt > 0 {
            self.turn.coins += 3;
            sink.bonus(p, ids::CHANGE, 0, 0, 3, 0);
        } else {
            self.stack.push(select(p, ids::CHANGE, Zone::Hand, Act::Trash, Filter::Any, 1, 1, Then::ChangeGain));
        }
    }

    /// Whether `c` is an Action right now: an Action card, or any Treasure while Enlightenment is in effect.
    #[inline]
    pub fn is_action(&self, c: CardId) -> bool {
        cards::is(c, ACTION) || (self.rs.active == ids::ENLIGHTENMENT && self.is_treasure(c))
    }

    /// The Action cards `p` has in play (Treasures too under Enlightenment).
    pub fn actions_in_play(&self, p: u8) -> u8 {
        self.players[p as usize].in_play.iter().filter(|&(c, _)| self.is_action(c)).map(|(_, n)| n as u32).sum::<u32>().min(u8::MAX as u32) as u8
    }

    /// `p`'s deck was just shuffled by an effect other than a draw's reshuffle (Annex, Donate, Inn, Famine): the Shadow cards go to the bottom.
    #[inline]
    pub(crate) fn rs_shuffled(&mut self, p: u8) {
        if self.rs.flags & R_SHADOW != 0 {
            self.players[p as usize].sink_shadows();
        }
    }

    /// Whether some Shadow card is in `p`'s deck, to be played from there (only in a game with a Shadow card).
    #[inline]
    pub fn shadow_playable(&self, p: u8) -> bool {
        self.rs.flags & R_SHADOW != 0 && self.shadow_in_deck(p)
    }

    #[inline(never)]
    fn shadow_in_deck(&self, p: u8) -> bool {
        let ps = &self.players[p as usize];
        SHADOWS.iter().any(|&s| ps.deck_has(s))
    }

    /// The Shadow cards of the deck of `p` (distinct), for the Action phase's choices.
    #[inline(never)]
    pub(crate) fn push_shadow_choices(&self, p: u8, out: &mut ChoiceBuf) {
        let ps = &self.players[p as usize];
        for &s in &SHADOWS {
            if ps.deck_has(s) && !ps.hand.has(s) {
                out.push(Choice::Card(s));
            }
        }
    }

    /// Whether the card `c` played from `p`'s hand is taken from their deck instead (a Shadow card with a copy in the deck).
    #[inline]
    pub(crate) fn plays_from_deck(&self, p: u8, c: CardId) -> bool {
        self.rs.flags & R_SHADOW != 0 && cards::is(c, SHADOW) && self.players[p as usize].deck_has(c)
    }

    /// Daimyo: the card `card` just played (if it is a non-Command Action of the turn's player) is replayed once for each Daimyo played this turn, after its
    /// effects: the replays are pushed under them.
    #[cold]
    #[inline(never)]
    pub(crate) fn push_daimyo_replays(&mut self, card: CardId, player: u8, tok: u8) {
        if player == self.turn.player && self.is_action(card) && !cards::is(card, COMMAND) {
            let d = std::mem::take(&mut self.turn.rs.daimyo);
            let zone = if tok == crate::track::NIP { Zone::Supply } else { Zone::Hand };
            for nth in (2..=d.saturating_add(1)).rev() {
                self.stack.push(Frame { subject: card, count: nth, zone, tok, ..Frame::new(K::PlayEffects, player, ids::DAIMYO) });
            }
        }
    }

    /// Whether `p` has something to play in the Action phase that is not an Action card in their hand: a Shadow card in the deck, or (Enlightenment)
    /// a Treasure in the hand.
    #[inline(always)]
    pub(crate) fn rs_can_play_extra(&self, p: u8) -> bool {
        (self.rs.flags & R_SHADOW != 0 || self.rs.active == ids::ENLIGHTENMENT) && self.rs_can_play_extra_slow(p)
    }

    #[inline(never)]
    fn rs_can_play_extra_slow(&self, p: u8) -> bool {
        self.shadow_playable(p) || (self.rs.active == ids::ENLIGHTENMENT && self.players[p as usize].hand.iter().any(|(c, _)| self.is_treasure(c)))
    }

    /// The Buys the turn's player can still use: the Buys, and under Flourishing Trade the Actions too ("you may use Action plays as Buys").
    pub fn buys_left(&self) -> u16 {
        self.turn.buys as u16 + self.buys_from_actions()
    }

    /// Whether a Buy can be used (`buys_left() > 0`): the Actions are only looked at when no Buy is left.
    #[inline(always)]
    pub fn can_buy(&self) -> bool {
        self.turn.buys > 0 || self.buys_from_actions() > 0
    }

    /// The Actions that count as Buys when there are no Buys left (Flourishing Trade).
    #[inline(never)]
    fn buys_from_actions(&self) -> u16 {
        if self.rs.active == ids::FLOURISHING_TRADE { self.turn.actions as u16 } else { 0 }
    }

    /// Uses one Buy (an Action play when no Buy is left, under Flourishing Trade).
    #[inline(always)]
    pub(crate) fn spend_buy(&mut self) {
        if self.turn.buys > 0 {
            self.turn.buys -= 1;
        } else {
            self.turn.actions = self.turn.actions.saturating_sub(1);
        }
    }

    /// Cleanup with a Prophecy in effect: Panic returns the Treasures discarded from play to their piles, Biding Time sets the hand aside.
    #[inline(never)]
    pub(crate) fn rs_cleanup<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        match self.rs.active {
            ids::PANIC => self.rs_panic_cleanup(p, sink),
            ids::BIDING_TIME => self.rs_biding_cleanup(p, sink),
            _ => {}
        }
    }

    /// Each other player discards down to 3 cards in hand (Ninja, Samurai).
    pub(crate) fn discard_attack<S: EventSink>(&mut self, card: CardId, sink: &mut S) {
        let (vs, n) = self.victims(sink);
        for &v in vs[..n].iter().rev() {
            self.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Act::Discard, Filter::Any, 0, 3, Then::Nothing) });
        }
    }

    /// Recomputes `RsState::hook` (whether a gain has anything to do): a live Prophecy that changes or answers gains, or an Event that records them.
    pub fn rs_update_hooks(&mut self) {
        let a = self.rs.active;
        let hook = self.rs.flags & (R_KINTSUGI | R_TRIBUTE) != 0
            || matches!(a, ids::BUREAUCRACY | ids::GROWTH | ids::HARSH_WINTER | ids::PROGRESS | ids::RAPID_EXPANSION);
        self.rs.hook = hook as u8;
    }

    /// `p` removes a Sun token from the Prophecy (a Omen was played): the last one makes the Prophecy happen, from now on, for everyone.
    pub(crate) fn remove_sun<S: EventSink>(&mut self, p: u8, source: CardId, sink: &mut S) {
        if self.rs.suns == 0 {
            return;
        }
        self.rs.suns -= 1;
        sink.event(Event::Sun { player: p, source, left: self.rs.suns });
        if self.rs.suns == 0 {
            self.rs.active = self.rs.prophecy;
            sink.event(Event::Prophecy { prophecy: self.rs.active, player: p });
            self.rs_update_hooks();
            self.prophecy_happens(p, sink);
        }
    }

    /// The moment the Prophecy happens: the effects that are one-off (the rest are read while they apply).
    #[inline(never)]
    fn prophecy_happens<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        match self.rs.active {
            // Gain an Action to the hand now (the player that removed the last token).
            ids::KIND_EMPEROR => self.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, ids::KIND_EMPEROR, u8::MAX, Filter::Action, Dest::Hand) }),
            // Cards cost $1 less, this turn and the following ones.
            ids::FLOURISHING_TRADE => self.turn.cost_reduction = self.turn.cost_reduction.saturating_add(1),
            ids::DIVINE_WIND => self.divine_wind(sink),
            _ => {}
        }
    }

    /// After a card was played (its effects done, bonuses and Sun tokens included): Great Leader's +1 Action, Approaching Army's +$1.
    #[inline(never)]
    pub(crate) fn rs_after_play<S: EventSink>(&mut self, card: CardId, p: u8, sink: &mut S) {
        match self.rs.active {
            ids::GREAT_LEADER if self.is_action(card) => {
                self.turn.actions += 1;
                sink.bonus(p, ids::GREAT_LEADER, 1, 0, 0, 0);
            }
            ids::APPROACHING_ARMY if cards::is(card, ATTACK) => {
                self.turn.coins += 1;
                sink.bonus(p, ids::APPROACHING_ARMY, 0, 0, 1, 0);
            }
            _ => {}
        }
    }

    /// Rice: $1 per different type among the cards `p` has in play (Rice itself is in play when it is played; Enlightenment makes Treasures Actions too).
    pub(crate) fn rice_coins(&self, p: u8) -> u16 {
        let mut types = 0u32;
        let mut gathering = false;
        for (c, _) in self.players[p as usize].in_play.iter() {
            types |= cards::def(c).types;
            gathering |= cards::is_gathering(c);
            // A Curse is a Treasure too, in a game with Charlatan.
            if c == id::CURSE && self.is_treasure(c) {
                types |= TREASURE;
            }
        }
        if self.rs.active == ids::ENLIGHTENMENT && types & TREASURE != 0 {
            types |= ACTION;
        }
        types.count_ones() as u16 + gathering as u16
    }

    /// `n` copies of the Treasure `c` are about to be played by the turn's player (`turn.played` does not count them yet): Panic's +2 Buys for
    /// each, Good Harvest's +1 Buy and +$1 for the first play of each differently named Treasure each turn.
    #[inline(never)]
    pub(crate) fn rs_treasure_plays<S: EventSink>(&mut self, c: CardId, n: u8, sink: &mut S) {
        let p = self.turn.player;
        match self.rs.active {
            ids::PANIC => {
                self.turn.buys = self.turn.buys.saturating_add(2 * n);
                sink.bonus(p, ids::PANIC, 0, 2 * n, 0, 0);
            }
            ids::GOOD_HARVEST if self.turn.played.get(c) == 0 => {
                self.turn.buys += 1;
                self.turn.coins += 1;
                sink.bonus(p, ids::GOOD_HARVEST, 0, 1, 1, 0);
            }
            _ => {}
        }
    }

    /// Enlightenment: a Treasure played in the Action phase gives +1 Card and +1 Action instead of its instructions. Whether it did.
    #[inline(never)]
    pub(crate) fn rs_enlightened_play<S: EventSink>(&mut self, card: CardId, p: u8, sink: &mut S) {
        self.turn.played.add(card, 1);
        self.turn.actions += 1;
        sink.bonus(p, ids::ENLIGHTENMENT, 1, 0, 0, 0);
        self.stack.push(draw_frame(p, card, 1));
    }

    /// The destination of a card `p` gains: Progress puts every gained card onto the deck.
    #[inline(never)]
    pub(crate) fn rs_gain_dest(&self, to: Dest) -> Dest {
        if self.rs.active == ids::PROGRESS { Dest::DeckTop } else { to }
    }

    /// What a gain of `c` by `p` (just moved to `to`) sets off before the cards' own gain triggers (so that it is answered after them): the records of
    /// Kintsugi and Receive Tribute, Growth's cheaper card, Rapid Expansion's set-aside.
    #[inline(never)]
    pub(crate) fn rs_gain_pre(&mut self, p: u8, c: CardId, to: Dest) {
        if self.rs.flags & R_KINTSUGI != 0 && c == id::GOLD {
            self.players[p as usize].rs |= P_RS_GOLD;
        }
        if self.rs.flags & R_TRIBUTE != 0 && p == self.turn.player {
            self.turn.rs.gains = self.turn.rs.gains.saturating_add(1);
        }
        match self.rs.active {
            // Gain a cheaper card (mandatory if there is one): every part of the cost no larger and not equal.
            ids::GROWTH if self.is_treasure(c) => {
                self.stack.push(Frame {
                    less: true, potion: cards::potion_cost(c), min: cards::debt_cost(c),
                    ..gain_frame(p, ids::GROWTH, self.cost(c), Filter::Any, Dest::Discard)
                });
            }
            // An Action or Treasure is set aside (once the gain's reactions have run, if it is still where it was gained).
            ids::RAPID_EXPANSION if cards::is(c, ACTION) || self.is_treasure(c) => {
                let zone = Self::gain_dest_zone(to);
                // The gained copy's token (made by the gain, which tracks every gain while a Prophecy hooks them): the set-aside finds it gone
                // if a reaction moved it, or covered it on the deck.
                let tok = self.turn.led.ingain;
                self.turn.led.hold(tok);
                self.stack.push(Frame {
                    zone, subject: c, tok, ..select(p, ids::RAPID_EXPANSION, zone, Act::SetAside, Filter::Any, 0, 0, Then::RapidAside)
                });
            }
            _ => {}
        }
    }

    /// What a gain of `c` by `p` sets off at once, after the card's own gain triggers: Bureaucracy's Copper, Harsh Winter's Debt.
    #[inline(never)]
    pub(crate) fn rs_gain_post<S: EventSink>(&mut self, p: u8, c: CardId, from_supply: bool, sink: &mut S) {
        match self.rs.active {
            ids::BUREAUCRACY => {
                if self.cost(c) != 0 || cards::debt_cost(c) > 0 || cards::potion_cost(c) {
                    self.gain(p, id::COPPER, Dest::Discard, false, ids::BUREAUCRACY, sink);
                }
            }
            // On the player's own turn: the Debt on the card's pile is taken, or $2 of it is put there.
            ids::HARSH_WINTER if p == self.turn.player && self.turn.phase != Phase::GameOver => {
                let pile = cards::pile_of(c);
                if from_supply || self.has_pile_of(c) {
                    let debt = self.emp.piles.take_debt(pile);
                    if debt > 0 {
                        self.take_debt(p, ids::HARSH_WINTER, debt as u16, sink);
                    } else {
                        self.emp.piles.add_debt(pile, 2);
                        sink.event(Event::PileDebt { pile, n: 2 });
                    }
                }
            }
            _ => {}
        }
    }

    /// Whether the card `c` has a pile in this game (a Supply pile, or one outside it), for the gains of the trash and the side piles.
    pub(crate) fn has_pile_of(&self, c: CardId) -> bool {
        let pile = cards::pile_of(c);
        self.in_supply(c)
            || self.in_supply(pile)
            || matches!(c, id::SPOILS | id::MADMAN | id::MERCENARY)
            || cards::is_reward(c)
            || (self.ferryman == c && c != 0)
            || (cards::FIRST_TRAV..cards::FIRST_TRAV + cards::NUM_TRAV as CardId).contains(&c)
            || (id::WILL_O_WISP..=id::GHOST).contains(&c)
            || crate::sets::dark_ages::MixedPile::of_member(c).is_some_and(|mp| self.has_pile(mp))
    }

    /// Cleanup, Panic: each Treasure discarded from play goes back to its pile instead (a Treasure with no pile, an Heirloom, is discarded as usual).
    #[inline(never)]
    pub(crate) fn rs_panic_cleanup<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let play = self.players[p as usize].in_play;
        for (c, n) in play.iter() {
            if !self.is_treasure(c) {
                continue;
            }
            let ps = &self.players[p as usize];
            let staying = self.turn.duration_held.get(c).saturating_add(ps.stays.get(c));
            let leaving = n.saturating_sub(staying);
            if leaving == 0 || !(self.has_pile_of(c) || cards::is_loot(c)) {
                continue;
            }
            self.players[p as usize].in_play.set(c, n - leaving);
            for _ in 0..leaving {
                self.return_pile_card(p, c, sink);
            }
        }
    }

    /// `c` goes back to its pile: any pile of the game (see `return_to_pile`, which the Nocturne cards use, and the Dark Ages' piles outside the Supply).
    fn return_pile_card<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        match c {
            id::SPOILS => {
                self.spoils += 1;
                sink.event(Event::Return { player: p, card: c });
            }
            id::MADMAN => {
                self.madman += 1;
                sink.event(Event::Return { player: p, card: c });
            }
            id::MERCENARY => {
                self.mercenary += 1;
                sink.event(Event::Return { player: p, card: c });
            }
            _ if cards::is_reward(c) => {
                self.rewards[(c - cards::FIRST_REWARD) as usize] += 1;
                sink.event(Event::Return { player: p, card: c });
            }
            _ if self.ferryman == c && c != 0 => {
                self.ferryman_left += 1;
                sink.event(Event::Return { player: p, card: c });
            }
            _ if (cards::FIRST_TRAV..cards::FIRST_TRAV + cards::NUM_TRAV as CardId).contains(&c) => {
                self.trav[(c - cards::FIRST_TRAV) as usize] += 1;
                sink.event(Event::Return { player: p, card: c });
            }
            _ => self.return_to_pile(p, c, sink),
        }
    }

    /// Cleanup, Biding Time: the hand is set aside (face down) instead of being discarded, to return at the start of the next turn.
    #[inline(never)]
    pub(crate) fn rs_biding_cleanup<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        let hand = ps.hand;
        for (c, n) in hand.iter() {
            if ps.aside.has(c) || ps.aside.iter().count() < crate::state::ASIDE_CAP {
                ps.aside.add(c, n);
                ps.hand.set(c, 0);
                for _ in 0..n {
                    sink.event(Event::SetAside { player: p, card: c });
                }
            }
        }
    }

    /// Divine Wind: the Kingdom piles leave the Supply (those of the kingdom and the Bane's, the Knights, the Castles and the split piles; not the
    /// basic cards, the Potion, Platinum and Colony, or the Ruins) and ten new ones are dealt, drawn with the game's RNG from the Kingdom cards of the sets
    /// the game uses that were not in the game yet. The removed piles are gone: no tokens, Debt or VP on them, not empty piles; the cards players
    /// have from them stay in their decks. Cards that need a pile of their own in set-up (the Bane, Ferryman's, the Rewards, the Castles, the Boons and
    /// Hexes and the other piles of Nocturne) are not dealt: see `divine_wind_ok`.
    #[inline(never)]
    fn divine_wind<S: EventSink>(&mut self, _sink: &mut S) {
        use crate::sets::dark_ages::RUINS_PILE;
        let n = self.num_players as usize;
        // The piles to remove, and the cards they stood for (a split pile by its top half, the Knights and the Castles by their markers).
        let mut old = [0 as CardId; 64];
        let mut used = [0 as CardId; 64];
        let (mut n_old, mut n_used) = (0, 0);
        let mut sets = [false; CardSet::ALL.len()];
        for c in self.supply_cards() {
            if c <= id::CURSE || cards::is_optional_basic(c) || RUINS_PILE.members().contains(&c) || n_old >= old.len() {
                continue;
            }
            old[n_old] = c;
            n_old += 1;
            let pile = cards::pile_of(c);
            if n_used < used.len() && !used[..n_used].contains(&pile) {
                used[n_used] = pile;
                n_used += 1;
            }
            sets[cards::set_of(c) as usize] = true;
        }
        for &c in &old[..n_old] {
            self.supply.set(c, 0);
            self.in_supply.remove(c);
        }
        for &pile in &used[..n_used] {
            self.emp.piles.clear_pile(pile);
            for ps in self.players.iter_mut() {
                for t in ps.tok.iter_mut() {
                    if *t == pile {
                        *t = 0;
                    }
                }
            }
        }
        self.emp.flags &= !crate::state::E_CASTLES;
        // Ten new piles.
        sets[CardSet::RisingSun as usize] = true;
        let ruins_in_game = self.has_pile(&RUINS_PILE);
        let mut pool = [0 as CardId; 400];
        let mut n_pool = 0;
        for c in cards::kingdom_cards() {
            let taken = used[..n_used].contains(&c) || c == self.bane || c == self.ferryman || c == self.rs.riverboat || self.emp.obelisk == c;
            if sets[cards::set_of(c) as usize] && !taken && Self::divine_wind_ok(c, ruins_in_game) && n_pool < pool.len() {
                pool[n_pool] = c;
                n_pool += 1;
            }
        }
        for _ in 0..10 {
            if n_pool == 0 {
                break;
            }
            let i = self.rng.below(n_pool as u32) as usize;
            let c = pool[i];
            pool[i] = pool[n_pool - 1];
            n_pool -= 1;
            self.add_kingdom_pile(c, n);
        }
        // The Traveller upgrades of a line that is now in the Supply.
        if self.in_supply(id::PAGE) && self.trav[..4].iter().all(|&k| k == 0) {
            self.trav[..4].fill(5);
        }
        if self.in_supply(id::PEASANT) && self.trav[4..].iter().all(|&k| k == 0) {
            self.trav[4..].fill(5);
        }
    }

    /// Whether Divine Wind may deal `c`: a Kingdom card whose set-up is its own pile (and the piles outside the Supply it brings in plain counts).
    fn divine_wind_ok(c: CardId, ruins_in_game: bool) -> bool {
        use cards::{DOOM, FATE, LOOTER};
        let setup_cards = [
            id::YOUNG_WITCH, id::FERRYMAN, id::JOUST, id::BAKER, id::CASTLES, id::EXORCIST, id::NECROMANCER, id::LEPRECHAUN, id::SECRET_CAVE,
            id::VAMPIRE, id::DEVILS_WORKSHOP, id::TORMENTOR, id::CEMETERY,
        ];
        !setup_cards.contains(&c) && !cards::is(c, FATE) && !cards::is(c, DOOM) && (ruins_in_game || !cards::is(c, LOOTER)) && c != id::RIVERBOAT
    }

    /// Adds the Supply pile of the Kingdom card `c` for `n` players, with the piles outside the Supply and the tokens it brings.
    fn add_kingdom_pile(&mut self, c: CardId, n: usize) {
        use crate::sets::dark_ages::KNIGHTS_PILE;
        let put = |g: &mut GameState, k: CardId, size: u8| {
            g.supply.set(k, size);
            g.in_supply.insert(k);
        };
        if c == id::KNIGHTS {
            self.init_mixed_pile(&KNIGHTS_PILE, 10);
        } else {
            let size = crate::sets::pile_size(c, n).unwrap_or(if cards::is(c, VICTORY) { crate::state::victory_pile_size(n) } else { 10 });
            put(self, c, size);
            if let Some(bottom) = cards::split_bottom(c) {
                put(self, bottom, 5);
            }
        }
        if cards::potion_cost(c) && !self.in_supply(id::POTION) {
            let size = crate::sets::pile_size(id::POTION, n).unwrap_or(16);
            put(self, id::POTION, size);
        }
        if cards::is(c, LOOTER) && !self.has_pile(&crate::sets::dark_ages::RUINS_PILE) {
            self.init_mixed_pile(&crate::sets::dark_ages::RUINS_PILE, 10 * (n as u8 - 1));
        }
        match c {
            id::BANDIT_CAMP | id::MARAUDER | id::PILLAGE if self.spoils == 0 => self.spoils = 15,
            id::HERMIT if self.madman == 0 => self.madman = 10,
            id::URCHIN if self.mercenary == 0 => self.mercenary = 10,
            _ => {}
        }
        if self.emp.flags & crate::state::E_TAX != 0 {
            self.emp.piles.add_debt(cards::pile_of(c), 1);
        }
    }

    // ---- frames ----

    /// The `then` of a finished `Select` that belongs to Rising Sun.
    #[inline(never)]
    pub(crate) fn finish_rs_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            // Mountain Shrine: +2 Cards if any Action is in the trash (the card just trashed too).
            Then::MountainShrine => {
                if self.trash.iter().any(|(c, _)| self.is_action(c)) {
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 2) });
                }
            }
            // Rice Broker: +2 Cards for a Treasure, +5 Cards for an Action (both for a card that is both).
            Then::RiceBroker => {
                if f.count > 0 {
                    let c = f.last;
                    if self.is_action(c) {
                        self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 5) });
                    }
                    if self.is_treasure(c) {
                        self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 2) });
                    }
                }
            }
            // Kintsugi: with a Gold gained this game (the trash's own effects included), a card costing up to $2 more.
            Then::KintsugiGain => {
                if f.count > 0 && self.players[pi].rs & P_RS_GOLD != 0 {
                    self.stack.push(Frame {
                        potion: cards::potion_cost(f.last), depth: f.depth,
                        ..gain_frame(p, f.source, self.cost(f.last).saturating_add(2), Filter::Any, Dest::Discard)
                    });
                }
            }
            // Continue: back to the Action phase with +1 Action and +1 Buy, and the card gained is played from where it was gained.
            Then::ContinueReturn => {
                if self.turn.phase == Phase::Buy && p == self.turn.player {
                    self.turn.phase = Phase::Action;
                    self.turn.treasures_done = false;
                    self.turn.actions += 1;
                    self.turn.buys += 1;
                    sink.event(Event::PhaseStart { player: p, phase: Phase::Action });
                    sink.bonus(p, f.source, 1, 1, 0, 0);
                }
                let c = f.subject;
                let here = c != 0 && f.tok != 0 && self.tok_valid(f.tok);
                self.tok_moved(f.tok, p, Zone::Discard, c);
                let ps = &mut self.players[pi];
                if here && ps.discard.remove(c) {
                    ps.in_play.add(c, 1);
                    sink.event(Event::Play { player: p, card: c });
                    self.resolve_effects(c, p, f.depth, sink);
                }
                self.release_tok(f.tok);
            }
            // Foresight: the cards set aside join the hand once the new hand is drawn.
            Then::ForesightToHand => {
                let ps = &mut self.players[pi];
                let aside = ps.foresight.take_all();
                for (c, n) in aside.iter() {
                    ps.hand.add(c, n);
                    for _ in 0..n {
                        sink.event(Event::Draw { player: p, card: c });
                    }
                }
            }
            // Rapid Expansion: the card gained is set aside if it is still where it was gained once the reactions have run.
            Then::RapidAside => {
                let c = f.subject;
                let ps = &mut self.players[pi];
                let room = ps.aside.has(c) || ps.aside.iter().count() < crate::state::ASIDE_CAP;
                let here = room && f.tok != 0 && self.tok_valid(f.tok);
                self.tok_moved(f.tok, p, f.zone, c);
                let ps = &mut self.players[pi];
                let taken = here
                    && match f.zone {
                        Zone::Discard => ps.discard.remove(c),
                        Zone::Hand => ps.hand.remove(c),
                        _ => ps.deck_known.pop_top().is_some(),
                    };
                if taken {
                    ps.aside.add(c, 1);
                    sink.event(Event::SetAside { player: p, card: c });
                }
                self.release_tok(f.tok);
            }
            _ => {}
        }
    }

    /// A `RevealTop` / `RevealUntil` of Rising Sun has finished.
    #[inline(never)]
    pub(crate) fn finish_rs_reveal<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            // Poet: the revealed card goes to the hand if it costs $3 or less (a Potion or Debt in the cost is more), else stays on top.
            Then::PoetCheck => {
                let Some(c) = self.players[pi].set_aside.iter().next().map(|(c, _)| c) else { return };
                self.players[pi].set_aside.remove(c);
                if self.cost(c) <= 3 && !cards::potion_cost(c) && cards::debt_cost(c) == 0 {
                    self.players[pi].hand.add(c, 1);
                    sink.event(Event::Draw { player: p, card: c });
                } else {
                    self.players[pi].deck_known.push_top(c);
                }
            }
            // Foresight: the Action revealed is set aside until the end of the turn, everything else revealed is discarded.
            Then::ForesightSet => {
                let revealed = self.players[pi].set_aside;
                let mut discarded = Counts::EMPTY;
                for (c, n) in revealed.iter() {
                    let keep = u8::from(f.count > 0 && c == f.last);
                    let ps = &mut self.players[pi];
                    if keep == 1 && (ps.foresight.has(c) || ps.foresight.iter().count() < crate::state::FORESIGHT_CAP) {
                        ps.foresight.add(c, 1);
                        sink.event(Event::SetAside { player: p, card: c });
                    } else if keep == 1 {
                        ps.discard.add(c, 1);
                        discarded.add(c, 1);
                    }
                    let rest = n - keep;
                    if rest > 0 {
                        ps.discard.add(c, rest);
                        for _ in 0..rest {
                            sink.event(Event::Discard { player: p, card: c });
                        }
                        discarded.add(c, rest);
                    }
                }
                self.players[pi].set_aside.clear();
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.discard_hook_counts(p, &discarded, f.depth, sink);
                }
            }
            _ => {}
        }
    }

    /// An `Ask` of Rising Sun is pointless (nothing to decide): Snake Witch needs a hand with no duplicates and a Snake Witch to return, Gold Mine a Gold.
    pub(crate) fn rs_ask_pointless(&self, f: &Frame) -> bool {
        match f.then {
            Then::SnakeWitch => {
                let ps = &self.players[f.player as usize];
                !self.self_in_play(f.player, f.source, f.tok) || ps.hand.iter().any(|(_, n)| n > 1)
            }
            Then::GoldMine => self.supply.get(id::GOLD) == 0,
            _ => false,
        }
    }

    /// An `Ask` of Rising Sun said Yes.
    pub(crate) fn rs_ask_yes<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::GoldMine => {
                self.gain(p, id::GOLD, Dest::Discard, false, f.source, sink);
                self.take_debt(p, f.source, 4, sink);
            }
            Then::SnakeWitch => {
                let hand = self.players[p as usize].hand;
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        self.reveal_card(p, c, sink);
                    }
                }
                if self.leave_play_self(p, f.source, f.tok) {
                    self.return_to_pile(p, f.source, sink);
                    self.curse_others(f.source, f.depth, sink);
                }
            }
            _ => {}
        }
    }

    /// Each other player (not protected from the Attack being resolved) gains a Curse, leftmost first.
    pub(crate) fn curse_others<S: EventSink>(&mut self, source: CardId, _depth: u8, sink: &mut S) {
        let (vs, n) = self.victims(sink);
        for &v in &vs[..n] {
            self.gain(v, id::CURSE, Dest::Discard, false, source, sink);
        }
    }

    /// A gain of Rising Sun finished with the card `c` gained: Credit's Debt, the next of Receive Tribute's Actions.
    pub(crate) fn rs_gained<S: EventSink>(&mut self, f: &Frame, c: CardId, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::CreditDebt => {
                let n = self.cost(c) as u16;
                if n > 0 {
                    self.take_debt(p, f.source, n, sink);
                }
            }
            Then::TributeGain => {
                if f.count > 1 {
                    self.stack.push(Frame { depth: f.depth, ..tribute_frame(p, f.source, f.count - 1, c, f.subject) });
                }
            }
            _ => {}
        }
    }

    /// Continue's gain is about to happen: a zero-pick selection under the gain's reactions returns to the Action phase and plays the card.
    pub(crate) fn push_continue_guard(&mut self, f: &Frame, c: CardId) {
        let p = f.player;
        self.stack.push(Frame {
            depth: f.depth, subject: c,
            ..select(p, f.source, Zone::Discard, Act::Play, Filter::Any, 0, 0, Then::ContinueReturn)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Action or Treasure of the set says whether playing it is choice-free (`choice_free(..)`), has a
    /// choice (`has_choice(..)`) or is order-sensitive (`order_sensitive(..)`); bots skip the play-order search
    /// only for choice-free hands. When adding cards, mark each one and update the lists below deliberately.
    #[test]
    fn choice_free_and_order_sensitive_cards() {
        let names = |kind| CARDS.iter().map(|(_, d)| d).filter(|d| d.on_play == kind).map(|d| d.name).collect::<Vec<_>>();
        assert_eq!(
            names(cards::OnPlay::ChoiceFree),
            [
                "Fishmonger", "Riverboat", "Ninja", "Tea House", "Samurai", "Rice",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Change", "Mountain Shrine", "Daimyo", "Artist", "Snake Witch", "Aristocrat", "Alley", "Poet", "River Shrine", "Rustic Village",
                "Rice Broker", "Ronin", "Tanuki",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
