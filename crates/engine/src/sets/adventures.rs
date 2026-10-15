//! Adventures: the Tavern mat and Reserve cards (called at the moments their text names), Travellers (exchanged when discarded from play),
//! the tokens that sit on piles or in front of players, the Events (bought, never gained), and the Duration cards that come with it.
//! See `docs/adventures-plan.md` for the rulings and the design.

use super::prelude::*;
use crate::cards::{
    adv, ACTION as ACT, FIRST_EVENT, FIRST_TRAV, NUM_TRAV, RESERVE as RES, TRAVELLER as TRAV,
};
use crate::state::{
    SupplyMask, TypeSet, ADV_JOURNEY_DOWN, ADV_MINUS_CARD, ADV_MINUS_COIN, G_CHAMPION, G_COST_TOKEN, G_PLAY_TOKENS, G_TRASH_TOKEN,
    TOK_ACTION, TOK_BUY, TOK_CARD, TOK_COIN, TOK_COST, TOK_TRASH,
};

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but the ids index the card table).
/// 202-231 are the kingdom cards, 272-279 the Traveller upgrades (232 is Sauna, in the Promo set), 296-315 the Events (pseudo-cards, above
/// every id that can sit in a zone).
pub mod ids {
    use crate::cards::CardId;
    pub const COIN_OF_THE_REALM: CardId = 202;
    pub const PAGE: CardId = 203;
    pub const PEASANT: CardId = 204;
    pub const RATCATCHER: CardId = 205;
    pub const RAZE: CardId = 206;
    pub const AMULET: CardId = 207;
    pub const CARAVAN_GUARD: CardId = 208;
    pub const DUNGEON: CardId = 209;
    pub const GEAR: CardId = 210;
    pub const GUIDE: CardId = 211;
    pub const DUPLICATE: CardId = 212;
    pub const MAGPIE: CardId = 213;
    pub const MESSENGER: CardId = 214;
    pub const MISER: CardId = 215;
    pub const PORT: CardId = 216;
    pub const RANGER: CardId = 217;
    pub const TRANSMOGRIFY: CardId = 218;
    pub const ARTIFICER: CardId = 219;
    pub const BRIDGE_TROLL: CardId = 220;
    pub const DISTANT_LANDS: CardId = 221;
    pub const GIANT: CardId = 222;
    pub const HAUNTED_WOODS: CardId = 223;
    pub const LOST_CITY: CardId = 224;
    pub const RELIC: CardId = 225;
    pub const ROYAL_CARRIAGE: CardId = 226;
    pub const STORYTELLER: CardId = 227;
    pub const SWAMP_HAG: CardId = 228;
    pub const TREASURE_TROVE: CardId = 229;
    pub const WINE_MERCHANT: CardId = 230;
    pub const HIRELING: CardId = 231;
    // The Traveller upgrades: non-Supply piles of five.
    pub const TREASURE_HUNTER: CardId = 272;
    pub const WARRIOR: CardId = 273;
    pub const HERO: CardId = 274;
    pub const CHAMPION: CardId = 275;
    pub const SOLDIER: CardId = 276;
    pub const FUGITIVE: CardId = 277;
    pub const DISCIPLE: CardId = 278;
    pub const TEACHER: CardId = 279;
    // The Events (not cards: pseudo-cards for what is bought, see `cards::is_event`).
    pub const ALMS: CardId = 423;
    pub const BALL: CardId = 424;
    pub const BONFIRE: CardId = 425;
    pub const BORROW: CardId = 426;
    pub const EXPEDITION: CardId = 427;
    pub const FERRY: CardId = 428;
    pub const INHERITANCE: CardId = 429;
    pub const LOST_ARTS: CardId = 430;
    pub const MISSION: CardId = 431;
    pub const PATHFINDING: CardId = 432;
    pub const PILGRIMAGE: CardId = 433;
    pub const PLAN: CardId = 434;
    pub const QUEST: CardId = 435;
    pub const RAID: CardId = 436;
    pub const SAVE: CardId = 437;
    pub const SCOUTING_PARTY: CardId = 438;
    pub const SEAWAY: CardId = 439;
    pub const TRADE: CardId = 440;
    pub const TRAINING: CardId = 441;
    pub const TRAVELLING_FAIR: CardId = 442;
}

/// An Event: no types, no bonuses (what it does is `event_effect`); its cost is the cost to buy it.
const fn event(name: &'static str, cost: u8) -> CardDef {
    adv(c(name, cost, 0, 0, 0, 0, 0, 0))
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::COIN_OF_THE_REALM, adv(has_choice(c("Coin of the Realm", 2, TREASURE | RES, 1, 0, 0, 0, 0)))), // goes to the Tavern mat
    (ids::PAGE, adv(choice_free(c("Page", 2, ACT | TRAV, 0, 0, 1, 1, 0)))),
    (ids::PEASANT, adv(choice_free(c("Peasant", 2, ACT | TRAV, 1, 0, 0, 0, 1)))),
    (ids::RATCATCHER, adv(choice_free(c("Ratcatcher", 2, ACT | RES, 0, 0, 1, 1, 0)))), // the trash is at the start of a turn
    (ids::RAZE, adv(order_sensitive(c("Raze", 2, ACT, 0, 0, 0, 1, 0)))), // trashes itself or any card in the hand (may be the +Actions card)
    (ids::AMULET, adv(order_sensitive(c("Amulet", 3, ACT | DURATION, 0, 0, 0, 0, 0)))), // may trash any card in the hand
    (ids::CARAVAN_GUARD, adv(choice_free(c("Caravan Guard", 3, ACT | DURATION | REACTION, 0, 0, 1, 1, 0)))),
    (ids::DUNGEON, adv(order_sensitive(c("Dungeon", 3, ACT | DURATION, 0, 0, 0, 1, 0)))), // discards 2 of the hand
    (ids::GEAR, adv(order_sensitive(c("Gear", 3, ACT | DURATION, 0, 0, 2, 0, 0)))), // sets aside up to 2 of the hand
    (ids::GUIDE, adv(choice_free(c("Guide", 3, ACT | RES, 0, 0, 1, 1, 0)))),
    (ids::DUPLICATE, adv(choice_free(c("Duplicate", 4, ACT | RES, 0, 0, 0, 0, 0)))),
    (ids::MAGPIE, adv(order_sensitive(c("Magpie", 4, ACT, 0, 0, 1, 1, 0)))), // reveals the top card: a miss stays known for a later draw
    (ids::MESSENGER, adv(order_sensitive(c("Messenger", 4, ACT, 2, 0, 0, 0, 1)))), // may put the deck into the discard pile
    (ids::MISER, adv(has_choice(c("Miser", 4, ACT, 0, 0, 0, 0, 0)))),
    (ids::PORT, adv(choice_free(c("Port", 4, ACT, 0, 0, 1, 2, 0)))),
    (ids::RANGER, adv(choice_free(c("Ranger", 4, ACT, 0, 0, 0, 0, 1)))),
    (ids::TRANSMOGRIFY, adv(choice_free(c("Transmogrify", 4, ACT | RES, 0, 0, 0, 1, 0)))),
    (ids::ARTIFICER, adv(order_sensitive(c("Artificer", 5, ACT, 1, 0, 1, 1, 0)))), // discards any cards of the hand, gains onto the deck
    (ids::BRIDGE_TROLL, adv(choice_free(c("Bridge Troll", 5, ACT | DURATION | ATTACK, 0, 0, 0, 0, 1)))),
    (ids::DISTANT_LANDS, adv(choice_free(c("Distant Lands", 5, ACT | RES | VICTORY, 0, 0, 0, 0, 0)))),
    (ids::GIANT, adv(choice_free(c("Giant", 5, ACT | ATTACK, 0, 0, 0, 0, 0)))), // only victims act
    (ids::HAUNTED_WOODS, adv(choice_free(c("Haunted Woods", 5, ACT | DURATION | ATTACK, 0, 0, 0, 0, 0)))),
    (ids::LOST_CITY, adv(choice_free(c("Lost City", 5, ACT, 0, 0, 2, 2, 0)))),
    (ids::RELIC, adv(has_choice(c("Relic", 5, TREASURE | ATTACK, 2, 0, 0, 0, 0)))), // an Attack Treasure: played through the effect path
    (ids::ROYAL_CARRIAGE, adv(choice_free(c("Royal Carriage", 5, ACT | RES, 0, 0, 0, 1, 0)))),
    (ids::STORYTELLER, adv(order_sensitive(c("Storyteller", 5, ACT, 0, 0, 0, 1, 0)))), // plays Treasures from the hand, converts $ to cards
    (ids::SWAMP_HAG, adv(choice_free(c("Swamp Hag", 5, ACT | DURATION | ATTACK, 0, 0, 0, 0, 0)))),
    (ids::TREASURE_TROVE, adv(has_choice(c("Treasure Trove", 5, TREASURE, 2, 0, 0, 0, 0)))), // gains a Gold and a Copper
    (ids::WINE_MERCHANT, adv(choice_free(c("Wine Merchant", 5, ACT | RES, 4, 0, 0, 0, 1)))),
    (ids::HIRELING, adv(choice_free(c("Hireling", 6, ACT | DURATION, 0, 0, 0, 0, 0)))),
    (ids::TREASURE_HUNTER, adv(has_choice(c("Treasure Hunter", 3, ACT | TRAV, 1, 0, 0, 1, 0)))), // gains Silvers
    (ids::WARRIOR, adv(choice_free(c("Warrior", 4, ACT | ATTACK | TRAV, 0, 0, 2, 0, 0)))),
    (ids::HERO, adv(has_choice(c("Hero", 5, ACT | TRAV, 2, 0, 0, 0, 0)))), // gains a Treasure of its choice
    (ids::CHAMPION, adv(choice_free(c("Champion", 6, ACT | DURATION, 0, 0, 0, 1, 0)))),
    (ids::SOLDIER, adv(choice_free(c("Soldier", 3, ACT | ATTACK | TRAV, 2, 0, 0, 0, 0)))), // only victims act
    (ids::FUGITIVE, adv(order_sensitive(c("Fugitive", 4, ACT | TRAV, 0, 0, 2, 1, 0)))), // discards any card of the hand
    (ids::DISCIPLE, adv(order_sensitive(plays(c("Disciple", 5, ACT | TRAV, 0, 0, 0, 0, 0), 2)))), // plays an Action of the hand twice
    (ids::TEACHER, adv(choice_free(c("Teacher", 6, ACT | RES, 0, 0, 0, 0, 0)))),
    // The Events.
    (ids::ALMS, event("Alms", 0)),
    (ids::BALL, event("Ball", 5)),
    (ids::BONFIRE, event("Bonfire", 3)),
    (ids::BORROW, event("Borrow", 0)),
    (ids::EXPEDITION, event("Expedition", 3)),
    (ids::FERRY, event("Ferry", 3)),
    (ids::INHERITANCE, event("Inheritance", 7)),
    (ids::LOST_ARTS, event("Lost Arts", 6)),
    (ids::MISSION, event("Mission", 4)),
    (ids::PATHFINDING, event("Pathfinding", 8)),
    (ids::PILGRIMAGE, event("Pilgrimage", 4)),
    (ids::PLAN, event("Plan", 3)),
    (ids::QUEST, event("Quest", 0)),
    (ids::RAID, event("Raid", 5)),
    (ids::SAVE, event("Save", 1)),
    (ids::SCOUTING_PARTY, event("Scouting Party", 2)),
    (ids::SEAWAY, event("Seaway", 5)),
    (ids::TRADE, event("Trade", 5)),
    (ids::TRAINING, event("Training", 6)),
    (ids::TRAVELLING_FAIR, event("Travelling Fair", 2)),
];

/// The Reserve cards called at the start of the turn (Guide, Ratcatcher, Transmogrify, Teacher): entries of the start-of-turn list.
const START_CALLS: [CardId; 4] = [ids::GUIDE, ids::RATCATCHER, ids::TRANSMOGRIFY, ids::TEACHER];

/// The Traveller upgrade piles of a game whose Supply is `in_supply`: five of each upgrade of a line whose first card is in it.
pub(crate) fn traveller_piles(in_supply: &SupplyMask) -> [u8; NUM_TRAV] {
    let mut out = [0u8; NUM_TRAV];
    if in_supply.has(ids::PAGE) {
        out[..4].fill(5);
    }
    if in_supply.has(ids::PEASANT) {
        out[4..].fill(5);
    }
    out
}

/// Whether `e` is an Event that can be bought only once per turn (Alms, Borrow, Save, Pilgrimage).
pub(crate) fn once_per_turn(e: CardId) -> bool {
    matches!(e, ids::ALMS | ids::BORROW | ids::SAVE | ids::PILGRIMAGE | crate::sets::rising_sun::ids::CONTINUE)
}

/// The Adventures cards (see the module docs of `sets`).
pub struct Adventures;

impl SetRules for Adventures {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Port, Lost City, Messenger, Haunted Woods, Swamp Hag and Duplicate have a "when you gain" effect or one that watches gains.
    const GAIN_HI: u128 = (1u128 << (ids::PORT - 128))
        | (1u128 << (ids::LOST_CITY - 128))
        | (1u128 << (ids::MESSENGER - 128))
        | (1u128 << (ids::HAUNTED_WOODS - 128))
        | (1u128 << (ids::SWAMP_HAG - 128))
        | (1u128 << (ids::DUPLICATE - 128));
    /// Caravan Guard: when another player plays an Attack, play it first from the hand.
    const ATTACK_REACTIONS: &'static [(CardId, Act, Then)] = &[(ids::CARAVAN_GUARD, Act::Play, Then::Nothing)];
    /// Champion: other players' Attacks don't affect its owner (it stays in play for the rest of the game).
    const IMMUNE_IN_PLAY: &'static [CardId] = &[ids::CHAMPION];

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            ids::AMULET => &[ModeOpt::Coins(1), ModeOpt::TrashFromHand(1), ModeOpt::Gain(id::SILVER, Dest::Discard)],
            ids::MISER => &[ModeOpt::TavernCopper, ModeOpt::CoinsPerTavernCopper],
            ids::TEACHER => &[
                ModeOpt::MoveToken(TOK_CARD as u8),
                ModeOpt::MoveToken(TOK_ACTION as u8),
                ModeOpt::MoveToken(TOK_BUY as u8),
                ModeOpt::MoveToken(TOK_COIN as u8),
            ],
            ids::QUEST => &[
                ModeOpt::DiscardFor { filter: Filter::Types(TypeSet::Attack), n: 1, gain: id::GOLD },
                ModeOpt::DiscardFor { filter: Filter::Card(id::CURSE), n: 2, gain: id::GOLD },
                ModeOpt::DiscardFor { filter: Filter::Any, n: 6, gain: id::GOLD },
                ModeOpt::Nothing,
            ],
            _ => &[],
        }
    }

    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::CARAVAN_GUARD => (0, 0, 0, 1),
            ids::BRIDGE_TROLL => (0, 0, 1, 0),
            ids::HAUNTED_WOODS => (3, 0, 0, 0),
            ids::SWAMP_HAG => (0, 0, 0, 3),
            ids::HIRELING => (1, 0, 0, 0),
            _ => (0, 0, 0, 0),
        }
    }

    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, ids::AMULET | ids::DUNGEON | ids::GEAR | ids::GUIDE | ids::RATCATCHER | ids::TRANSMOGRIFY | ids::TEACHER)
    }

    fn start_entry_repeats(card: CardId) -> bool {
        START_CALLS.contains(&card)
    }

    /// Port's pile has 12 cards.
    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        (card == ids::PORT).then_some(12)
    }

    #[inline(never)]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        // A Reserve card goes onto the Tavern mat when it is played: this one, if it is still in play (a Throne Room's second play finds
        // it gone) and was played from the hand.
        if cards::is(card, RES) && !g.turn.prince_play && g.players[pi].in_play.remove(card) {
            g.players[pi].tavern.add(card, 1);
            sink.event(Event::ToTavern { player: p, card });
        }
        match card {
            ids::RAZE => {
                // "Trash this or a card from your hand": this one first (a YesNo), else a card from the hand.
                if g.turn.prince_play || !g.players[pi].in_play.has(card) {
                    g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::RazeLook));
                } else {
                    g.stack.push(Frame { zone: Zone::InPlay, act: Trash, subject: card, then: Then::RazeSelf, ..Frame::new(K::YesNo, p, card) });
                }
            }
            ids::AMULET => g.stack.push(mode_frame(p, card, 1)),
            ids::DUNGEON => {
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 2, 2, Then::Nothing));
                g.stack.push(draw_frame(p, card, 2));
            }
            ids::GEAR => g.stack.push(select(p, card, Zone::Hand, SetAside, Filter::Any, 0, 2, Then::GearAside)),
            ids::MAGPIE => g.stack.push(Frame { max: 1, then: Then::MagpieReveal, ..Frame::new(K::RevealTop, p, card) }),
            ids::MESSENGER => g.stack.push(Frame { then: Then::DeckToDiscard, ..Frame::new(K::Ask, p, card) }),
            ids::MISER => g.stack.push(mode_frame(p, card, 1)),
            ids::RANGER => {
                if g.flip_journey(p, sink) {
                    g.stack.push(draw_frame(p, card, 5));
                }
            }
            ids::ARTIFICER => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::ArtificerGain)),
            ids::BRIDGE_TROLL => {
                // Each other player takes the -$1 token; cards cost $1 less this turn (and next: `duration_start`).
                let (vs, n) = g.victims(sink);
                for &v in &vs[..n] {
                    g.give_minus_coin(v, sink);
                }
                g.turn.cost_reduction = g.turn.cost_reduction.saturating_add(1);
            }
            ids::GIANT => {
                let up = g.flip_journey(p, sink);
                let coins = if up { 5 } else { 1 };
                g.turn.coins += coins;
                sink.bonus(p, card, 0, 0, coins, 0);
                if up {
                    let (vs, n) = g.victims(sink);
                    for &v in vs[..n].iter().rev() {
                        g.stack.push(Frame { then: Then::GiantReveal, ..Frame::new(K::TopCard, v, card) });
                    }
                }
            }
            ids::HAUNTED_WOODS | ids::SWAMP_HAG => g.shield_lingering_attack(p, sink),
            ids::RELIC => {
                let (vs, n) = g.victims(sink);
                for &v in &vs[..n] {
                    g.give_minus_card(v, sink);
                }
            }
            ids::STORYTELLER => {
                // Play up to 3 Treasures, then +1 Card, then pay all the $ for cards: pushed in the reverse order.
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::StorytellerPay));
                g.stack.push(draw_frame(p, card, 1));
                g.stack.push(select(p, card, Zone::Hand, Play, g.treasure_filter(), 0, 3, Then::StorytellerPlay));
            }
            ids::TREASURE_TROVE => {
                g.gain(p, id::GOLD, Dest::Discard, false, card, sink);
                g.gain(p, id::COPPER, Dest::Discard, false, card, sink);
            }
            ids::HIRELING => {
                // Stays in play for the rest of the game (with the Throne Room / King's Court that played it twice).
                let ps = &mut g.players[pi];
                if ps.stays.get(card) < ps.in_play.get(card) {
                    ps.stays.add(card, 1);
                }
            }
            ids::TREASURE_HUNTER => {
                let right = g.right_of(p);
                let n = g.players[right as usize].last_turn_gains.total();
                for _ in 0..n {
                    g.gain(p, id::SILVER, Dest::Discard, false, card, sink);
                }
            }
            ids::WARRIOR => {
                // For each Traveller in play (this one included), each other player discards the top card and trashes it if it costs $3 or $4.
                let n = g.players[pi].in_play.count_type(TRAV).max(1);
                let (vs, nv) = g.victims(sink);
                for &v in vs[..nv].iter().rev() {
                    for _ in 0..n {
                        g.stack.push(Frame { then: Then::WarriorHit, act: Discard, ..Frame::new(K::TopCard, v, card) });
                    }
                }
            }
            ids::HERO => g.stack.push(Frame { potion: true, ..gain_frame(p, card, u8::MAX, g.treasure_filter(), Dest::Discard) }),
            ids::CHAMPION => {
                let ps = &mut g.players[pi];
                if ps.stays.get(card) < ps.in_play.get(card) {
                    ps.stays.add(card, 1);
                    g.adv |= G_CHAMPION;
                    if g.turn.multiplier_card != 0 {
                        g.turn.multiplier_successes += 1;
                    }
                }
            }
            ids::SOLDIER => {
                // +$1 per other Attack in play, then every other player with 4 or more cards discards one.
                let in_play = &g.players[pi].in_play;
                let others = in_play.count_type(ATTACK).saturating_sub(u32::from(in_play.has(card) && !g.turn.prince_play)) as u16;
                if others > 0 {
                    g.turn.coins += others;
                    sink.bonus(p, card, 0, 0, others, 0);
                }
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame {
                        down_to_target: true, exact: true,
                        ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 4, Then::Nothing)
                    });
                }
            }
            ids::FUGITIVE => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 1, 1, Then::Nothing)),
            ids::DISCIPLE => g.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayTwiceCopy)),
            _ => {}
        }
    }

    /// Gear sets cards aside one pending entry each; Champion stays in play for good and has no next-turn entry.
    fn schedules_own_duration(card: CardId) -> bool {
        matches!(card, ids::GEAR | ids::CHAMPION)
    }

    #[inline(never)]
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
        match e.card {
            ids::AMULET => {
                for _ in 0..e.times {
                    g.stack.push(Frame { depth: 1, ..mode_frame(p, e.card, 1) });
                }
            }
            ids::DUNGEON => {
                for _ in 0..e.times {
                    g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Hand, Act::Discard, Filter::Any, 2, 2, Then::Nothing) });
                    g.stack.push(Frame { depth: 1, ..draw_frame(p, e.card, 2) });
                }
            }
            ids::GEAR => {
                if e.arg != 0 {
                    let ps = &mut g.players[p as usize];
                    if ps.set_aside.remove(e.arg) {
                        ps.hand.add(e.arg, 1);
                        sink.event(Event::Draw { player: p, card: e.arg });
                    }
                }
            }
            ids::BRIDGE_TROLL => g.turn.cost_reduction = g.turn.cost_reduction.saturating_add(e.times),
            // At the start of every turn for the rest of the game.
            ids::HIRELING => g.push_duration_pending(p, e.card, e.times, 0),
            _ => {}
        }
    }

    /// Guide, Ratcatcher, Transmogrify and Teacher on the Tavern mat may be called at the start of the turn: one entry each, `times` = copies.
    fn turn_start_entries(g: &mut GameState) {
        let pi = g.turn.player as usize;
        if g.players[pi].tavern.is_empty() {
            return;
        }
        for card in START_CALLS {
            let n = g.players[pi].tavern.get(card);
            if n > 0 {
                g.turn.start_pending.push(PendingDuration { card, times: n, arg: 0, used: 0 });
            }
        }
    }

    #[inline(never)]
    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) -> bool {
        if !START_CALLS.contains(&e.card) {
            return false;
        }
        // "You may call this": a question, unless it is no longer on the mat.
        g.stack.push(Frame {
            zone: Zone::Tavern, act: Act::Call, subject: e.card, then: Then::Called,
            ..Frame::new(K::YesNo, p, e.card)
        });
        true
    }

    /// Port's second Port, Lost City's cards for the others, Messenger's gains, the Curses and topdecking of Swamp Hag and Haunted Woods (a
    /// card bought), Duplicate, Travelling Fair and the Trashing token.
    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, on_buy: bool, sink: &mut S) {
        let pi = p as usize;
        // Swamp Hag / Haunted Woods: another player gained a card they bought (not Events, not Black Market-less gains).
        if on_buy && p == g.turn.player && (g.in_supply(ids::SWAMP_HAG) || g.in_supply(ids::HAUNTED_WOODS)) {
            let mut curses = 0u8;
            let mut topdeck = false;
            for owner in g.others(p) {
                let ops = &g.players[owner as usize];
                if ops.shielded & (1 << p) != 0 {
                    continue;
                }
                for i in 0..ops.pending_durations_len as usize {
                    let e = ops.pending_durations[i];
                    match e.card {
                        ids::SWAMP_HAG => curses = curses.saturating_add(e.times),
                        ids::HAUNTED_WOODS => topdeck = true,
                        _ => {}
                    }
                }
            }
            for _ in 0..curses {
                g.gain(p, id::CURSE, Dest::Discard, false, ids::SWAMP_HAG, sink);
            }
            if topdeck && !g.players[pi].hand.is_empty() {
                g.stack.push(Frame {
                    ordered: true,
                    ..select(p, ids::HAUNTED_WOODS, Zone::Hand, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing)
                });
            }
        }
        match c {
            // "Gain another Port (that doesn't come with another)".
            ids::PORT if !g.turn.adv.no_chain => {
                g.turn.adv.no_chain = true;
                g.gain(p, ids::PORT, Dest::Discard, false, ids::PORT, sink);
                g.turn.adv.no_chain = false;
            }
            ids::LOST_CITY => {
                // Each other player draws a card; the player to the left first (pushed last).
                let mut buf = [0u8; MAX_PLAYERS];
                let mut n = 0;
                for q in g.others(p) {
                    buf[n] = q;
                    n += 1;
                }
                for &q in buf[..n].iter().rev() {
                    g.stack.push(draw_frame(q, ids::LOST_CITY, 1));
                }
            }
            // The first card gained in the Buy phase: gain a card up to $4, and each other player gains a copy.
            ids::MESSENGER if g.turn.phase == Phase::Buy && p == g.turn.player && g.turn.gained_in_buy == 1 => {
                g.stack.push(Frame { then: Then::MessengerCopies, ..gain_frame(p, ids::MESSENGER, 4, Filter::Any, Dest::Discard) });
            }
            _ => {}
        }
        // Plan: a card gained from the pile with the Trashing token may trash a card from the hand.
        if g.adv & G_TRASH_TOKEN != 0 {
            let tok = g.players[pi].tok[TOK_TRASH];
            if tok != 0 && tok == cards::pile_of(c) {
                g.stack.push(select(p, ids::PLAN, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing));
            }
        }
        // Travelling Fair: the card gained this turn may be put onto the deck.
        if g.turn.adv.fair && p == g.turn.player && to != Dest::DeckTop {
            g.stack.push(Frame {
                zone: GameState::gain_dest_zone(to), act: Act::Topdeck, subject: c,
                ..Frame::new(K::YesNo, p, ids::TRAVELLING_FAIR)
            });
        }
        // Duplicate: call it on a card costing up to $6 that has a copy left in its Supply pile.
        if g.players[pi].tavern.has(ids::DUPLICATE)
            && g.in_supply(c)
            && g.supply.get(c) > 0
            && g.cost(c) <= 6
            && !cards::potion_cost(c)
        {
            g.stack.push(Frame {
                zone: Zone::Tavern, act: Act::Call, subject: ids::DUPLICATE, then: Then::Called, last: c,
                ..Frame::new(K::YesNo, p, ids::DUPLICATE)
            });
        }
    }

    /// The Trashing token and Travelling Fair are live in a game where a Trashing token is on a pile or Fair was bought this turn.
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.adv & G_TRASH_TOKEN != 0 || g.turn.adv.fair
    }

    /// Wine Merchant: with at least $2 unspent at the end of the Buy phase, each on the mat may be discarded from it.
    #[inline(never)]
    fn end_of_buy<S: EventSink>(g: &mut GameState, _sink: &mut S) {
        let p = g.turn.player;
        if g.turn.coins >= 2 && g.players[p as usize].tavern.has(ids::WINE_MERCHANT) {
            for _ in 0..g.players[p as usize].tavern.get(ids::WINE_MERCHANT) {
                g.stack.push(Frame {
                    zone: Zone::Tavern, act: Act::Discard, subject: ids::WINE_MERCHANT,
                    ..Frame::new(K::YesNo, p, ids::WINE_MERCHANT)
                });
            }
        }
    }

    /// "When you discard this from play, you may exchange it": one offer per Traveller in play that this Clean-up discards. The highest
    /// upgrade of each line is asked first (pushed last), so that a short pile is used where it matters.
    #[inline(never)]
    fn offers_on_discard(g: &mut GameState) {
        if g.trav == [0; NUM_TRAV] {
            return;
        }
        let p = g.turn.player;
        for card in [ids::PAGE, ids::TREASURE_HUNTER, ids::WARRIOR, ids::HERO, ids::PEASANT, ids::SOLDIER, ids::FUGITIVE, ids::DISCIPLE] {
            let leaving = g.players[p as usize].in_play.get(card).saturating_sub(g.turn.duration_held.get(card));
            for _ in 0..leaving {
                g.stack.push(Frame { zone: Zone::InPlay, act: Act::Exchange, subject: card, ..Frame::new(K::YesNo, p, card) });
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// Events
// -------------------------------------------------------------------------------------------

/// Whether the Event `e` is in the game and may be bought now with `coins` (it costs `cards::cost(e)`, never reduced; a Buy is the caller's
/// check): once-per-turn and once-per-game Events, and Events that could do nothing at all, are not offered.
pub fn event_buyable(g: &GameState, e: CardId) -> bool {
    g.turn.coins >= cards::cost(e) as u16 && event_available(g, e)
}

/// `event_buyable` without the cost: the Event is in the game and nothing but its price keeps it from being bought (a Coffers token spent
/// may still pay it).
pub fn event_available(g: &GameState, e: CardId) -> bool {
    let bit = 1u64 << (e - FIRST_EVENT);
    if g.events & bit == 0 {
        return false;
    }
    let p = g.turn.player as usize;
    if once_per_turn(e) && g.turn.adv.bought & bit != 0 {
        return false;
    }
    if e == crate::sets::promo::ids::SUMMON {
        return crate::sets::promo::event_available(g, e);
    }
    if cards::set_of(e) == CardSet::RisingSun {
        return crate::sets::rising_sun::event_available(g, e);
    }
    if e >= crate::sets::empires::ids::ADVANCE {
        return crate::sets::empires::event_available(g, e);
    }
    match e {
        // Once per game; needs a card to set aside.
        ids::INHERITANCE => g.players[p].inherit == 0 && g.supply_cards().any(|c| g.supply.get(c) > 0 && g.filter_ok(p as u8, Filter::InheritCard, c)),
        // Not a third turn in a row, and not twice.
        ids::MISSION => !g.turn.is_extra_turn && !g.turn.adv.mission,
        // Needs an Action card costing up to $4 to gain.
        ids::SEAWAY => g.supply_cards().any(|c| g.supply.get(c) > 0 && cards::is(c, ACTION) && g.cost(c) <= 4 && !cards::potion_cost(c)),
        ids::FERRY | ids::PLAN | ids::LOST_ARTS | ids::TRAINING | ids::PATHFINDING => g.has_token_pile(),
        _ => true,
    }
}

/// Whether Alms is still worth buying before the Treasures are played: it is in the game and not yet bought this turn, a Buy is left, no Treasure
/// is in play and the hand holds one that would be played (`GameState::alms_window`; the caller has tested that Alms is in the game).
pub(crate) fn alms_window(g: &GameState) -> bool {
    let bit = 1u64 << (ids::ALMS - FIRST_EVENT);
    let p = g.turn.player;
    g.events & bit != 0
        && g.turn.adv.bought & bit == 0
        && g.can_buy()
        && !g.turn.treasures_done
        && g.treasures_in_play(p) == 0
        && g.players[p as usize].hand.iter().any(|(c, _)| g.is_treasure(c))
}

/// What buying the Event `e` does, after its cost and Buy are paid.
pub(crate) fn event_effect<S: EventSink>(g: &mut GameState, e: CardId, p: u8, sink: &mut S) {
    use Act::*;
    if e == crate::sets::promo::ids::SUMMON {
        return crate::sets::promo::event_effect(g, e, p, sink);
    }
    if cards::set_of(e) == CardSet::RisingSun {
        return crate::sets::rising_sun::event_effect(g, e, p, sink);
    }
    if e >= crate::sets::empires::ids::ADVANCE {
        return crate::sets::empires::event_effect(g, e, p, sink);
    }
    let pi = p as usize;
    match e {
        ids::ALMS => {
            if g.treasures_in_play(p) == 0 {
                g.stack.push(gain_frame(p, e, 4, Filter::Any, Dest::Discard));
            }
        }
        ids::BORROW => {
            g.turn.buys += 1;
            sink.bonus(p, e, 0, 1, 0, 0);
            if g.players[pi].adv & ADV_MINUS_CARD == 0 {
                g.give_minus_card(p, sink);
                g.turn.coins += 1;
                sink.bonus(p, e, 0, 0, 1, 0);
            }
        }
        ids::QUEST => g.stack.push(mode_frame(p, e, 1)),
        ids::SAVE => {
            g.turn.buys += 1;
            sink.bonus(p, e, 0, 1, 0, 0);
            g.stack.push(select(p, e, Zone::Hand, SetAside, Filter::Any, 1, 1, Then::SaveAside));
        }
        ids::SCOUTING_PARTY => {
            g.turn.buys += 1;
            sink.bonus(p, e, 0, 1, 0, 0);
            // Look at the top 5, discard 3, put the rest back in any order.
            g.stack.push(Frame { ordered: true, ..select(p, e, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
            g.stack.push(select(p, e, Zone::Revealed, Discard, Filter::Any, 3, 3, Then::Nothing));
            g.stack.push(Frame { max: 5, ..Frame::new(K::RevealTop, p, e) });
        }
        ids::TRAVELLING_FAIR => {
            g.turn.buys += 2;
            sink.bonus(p, e, 0, 2, 0, 0);
            g.turn.adv.fair = true;
        }
        ids::BONFIRE => g.stack.push(select(p, e, Zone::InPlay, Trash, Filter::Card(id::COPPER), 0, 2, Then::Nothing)),
        ids::EXPEDITION => g.turn.extra_draw = g.turn.extra_draw.saturating_add(2),
        ids::FERRY => g.push_token_frame(p, e, TOK_COST, false),
        ids::PLAN => g.push_token_frame(p, e, TOK_TRASH, false),
        ids::LOST_ARTS => g.push_token_frame(p, e, TOK_ACTION, false),
        ids::TRAINING => g.push_token_frame(p, e, TOK_COIN, false),
        ids::PATHFINDING => g.push_token_frame(p, e, TOK_CARD, false),
        ids::SEAWAY => g.stack.push(Frame {
            then: Then::TokenOnGained { tok: TOK_BUY as u8 },
            ..gain_frame(p, e, 4, Filter::Action, Dest::Discard)
        }),
        ids::INHERITANCE => g.stack.push(select(p, e, Zone::Supply, SetAside, Filter::InheritCard, 1, 1, Then::Inherit)),
        ids::MISSION => g.turn.adv.mission = true,
        ids::PILGRIMAGE => {
            if g.flip_journey(p, sink) {
                g.stack.push(Frame { distinct: true, ..select(p, e, Zone::InPlay, Reveal, Filter::GainableCopy, 0, 3, Then::GainCopyEachPick) });
            }
        }
        ids::BALL => {
            g.give_minus_coin(p, sink);
            for _ in 0..2 {
                g.stack.push(gain_frame(p, e, 4, Filter::Any, Dest::Discard));
            }
        }
        ids::RAID => {
            let n = g.players[pi].in_play.get(id::SILVER);
            for _ in 0..n {
                g.gain(p, id::SILVER, Dest::Discard, false, e, sink);
            }
            // Not an Attack: nobody is immune.
            for v in g.others(p) {
                g.give_minus_card(v, sink);
            }
        }
        ids::TRADE => g.stack.push(select(p, e, Zone::Hand, Trash, Filter::Any, 0, 2, Then::GainPerPick { card: id::SILVER })),
        _ => {}
    }
}

// -------------------------------------------------------------------------------------------
// The generic frames' Adventures-specific parts
// -------------------------------------------------------------------------------------------

impl GameState {
    /// Turn `p`'s Journey token over; whether it is face up now.
    #[inline(never)]
    pub(crate) fn flip_journey<S: EventSink>(&mut self, p: u8, sink: &mut S) -> bool {
        let ps = &mut self.players[p as usize];
        ps.adv ^= ADV_JOURNEY_DOWN;
        let up = ps.adv & ADV_JOURNEY_DOWN == 0;
        sink.event(Event::Flip { player: p, up });
        up
    }

    /// `p` puts their -1 Card token on their deck (nothing if it is already there).
    #[inline(never)]
    pub(crate) fn give_minus_card<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        if ps.adv & ADV_MINUS_CARD == 0 {
            ps.adv |= ADV_MINUS_CARD;
            sink.event(Event::MinusToken { player: p, token: 7 });
        }
    }

    /// `p` takes their -$1 token (nothing if they have it). The turn's player pays it from the next $ they get this turn.
    #[inline(never)]
    pub(crate) fn give_minus_coin<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        if ps.adv & ADV_MINUS_COIN == 0 {
            ps.adv |= ADV_MINUS_COIN;
            sink.event(Event::MinusToken { player: p, token: 8 });
            if p == self.turn.player {
                self.turn.adv.tax_live = true;
                self.turn.adv.tax_base = self.turn.coins;
            }
        }
    }

    /// The -$1 token takes $1 from the first $ the turn's player gets while they have it: after the step that raised their coins.
    #[cold]
    pub(crate) fn settle_coin_tax(&mut self) {
        if self.turn.coins > self.turn.adv.tax_base {
            self.turn.coins -= 1;
            self.turn.adv.tax_live = false;
            self.players[self.turn.player as usize].adv &= !ADV_MINUS_COIN;
        }
    }

    /// `p`'s token `tok` is on `pile` now.
    #[inline(never)]
    pub(crate) fn place_token<S: EventSink>(&mut self, p: u8, tok: usize, pile: CardId, sink: &mut S) {
        // Tax (Empires): 2 Debt on a Supply pile.
        if tok == crate::sets::empires::TOK_TAX {
            self.emp.piles.add_debt(pile, 2);
            sink.event(Event::PileDebt { pile, n: 2 });
            return;
        }
        self.players[p as usize].tok[tok] = pile;
        self.adv |= match tok {
            TOK_COST => G_COST_TOKEN,
            TOK_TRASH => G_TRASH_TOKEN,
            _ => G_PLAY_TOKENS,
        };
        sink.event(Event::Token { player: p, token: tok as u8, pile });
    }

    /// Whether the game has an Action Supply pile a token may be placed on.
    pub(crate) fn has_token_pile(&self) -> bool {
        self.supply_cards().any(|c| self.is_token_pile(c)) || self.has_pile(&crate::sets::dark_ages::KNIGHTS_PILE)
    }

    /// Whether the Supply pile named `c` (a Supply id) is an Action Supply pile: the Knights count, the Ruins and the Supply's non-Actions do
    /// not (the Ruins pile is an Action pile by the rules, but a token there would be pointless).
    pub(crate) fn is_token_pile(&self, c: CardId) -> bool {
        c < cards::FIRST_PILE_CARD && cards::pile_of(c) == c && cards::is(c, ACTION)
    }

    /// Push the frame that moves `p`'s token `tok` to a pile of their choice; `fresh` = only a pile they have no token on (Teacher).
    #[inline(never)]
    pub(crate) fn push_token_frame(&mut self, p: u8, source: CardId, tok: usize, fresh: bool) {
        self.stack.push(Frame { subject: tok as CardId, max: fresh as u8, ..Frame::new(K::Token, p, source) });
    }

    /// The piles `f` (a `K::Token` frame) may move its token to.
    #[inline(never)]
    pub(crate) fn token_choices(&self, f: Frame, out: &mut ChoiceBuf) {
        // Tax: any Supply pile (a split pile by its top half, the Castles, the Knights and the Ruins by their markers).
        if f.subject as usize == crate::sets::empires::TOK_TAX {
            for c in self.supply_cards() {
                if cards::pile_of(c) == c && c < cards::FIRST_PILE_CARD {
                    out.push(Choice::Card(c));
                }
            }
            for pk in [id::KNIGHTS, id::RUINS] {
                if crate::sets::dark_ages::MixedPile::of_marker(pk).is_some_and(|mp| self.has_pile(mp)) {
                    out.push(Choice::Card(pk));
                }
            }
            if self.in_supply(id::HUMBLE_CASTLE) && self.castles_left() > 0 {
                out.push(Choice::Card(id::CASTLES));
            }
            return;
        }
        let ps = &self.players[f.player as usize];
        let ok = |pile: CardId| {
            // Teacher: not a pile that holds any of the player's tokens.
            f.max == 0 || !ps.tok.iter().any(|&t| t == pile)
        };
        for c in self.supply_cards() {
            if self.is_token_pile(c) && ok(c) {
                out.push(Choice::Card(c));
            }
        }
        if self.has_pile(&crate::sets::dark_ages::KNIGHTS_PILE) && ok(id::KNIGHTS) {
            out.push(Choice::Card(id::KNIGHTS));
        }
    }

    #[inline(never)]
    pub(crate) fn run_token(&mut self, f: Frame) -> Run {
        let mut buf = ChoiceBuf::default();
        self.token_choices(f, &mut buf);
        if buf.is_empty() {
            self.stack.pop();
            return Run::Continue;
        }
        Run::Decide(DecisionKind::Token { token: f.subject as u8 }, 0)
    }

    /// `K::AfterPlay`: the card `f.subject` was played and has fully resolved. Offer the calls that follow a played Action: Coin of the Realm
    /// (+2 Actions) and, if the Action is still in play, Royal Carriage (replay it). A replay opens its own window, so another Carriage can
    /// follow; Coins are offered again while one is left.
    #[inline(never)]
    pub(crate) fn run_after_play(&mut self, f: Frame) -> Run {
        self.stack.pop();
        let p = f.player;
        let ps = &self.players[p as usize];
        // A Reserve card goes to the Tavern mat when played, so it is no longer in play: another copy in play (a Royal Carriage called
        // earlier, kept in play by the Duration it replayed) is not it (FAQ: a called Reserve card cannot be replayed either). Offering the
        // call there would loop forever: each replay of a Royal Carriage puts one back on the mat as the call takes one off.
        if ps.tavern.has(ids::ROYAL_CARRIAGE) && ps.in_play.has(f.subject) && !cards::is(f.subject, RES) {
            self.stack.push(Frame {
                zone: Zone::Tavern, act: Act::Call, subject: ids::ROYAL_CARRIAGE, then: Then::Called, last: f.subject, depth: f.depth,
                ..Frame::new(K::YesNo, p, ids::ROYAL_CARRIAGE)
            });
        }
        if ps.tavern.has(ids::COIN_OF_THE_REALM) {
            self.stack.push(Frame {
                zone: Zone::Tavern, act: Act::Call, subject: ids::COIN_OF_THE_REALM, then: Then::Called, last: f.subject, depth: f.depth,
                ..Frame::new(K::YesNo, p, ids::COIN_OF_THE_REALM)
            });
        }
        Run::Continue
    }

    /// Whether `p` has a Reserve card on the mat that is called after an Action is played.
    #[inline(always)]
    pub(crate) fn after_play_possible(&self, p: u8) -> bool {
        let t = &self.players[p as usize].tavern;
        !t.is_empty() && (t.has(ids::COIN_OF_THE_REALM) || t.has(ids::ROYAL_CARRIAGE))
    }

    /// A call was made (`YesNo` with `Then::Called`, the card already moved from the mat into play): what it does.
    #[inline(never)]
    pub(crate) fn call_effect<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        let depth = f.depth;
        match f.subject {
            ids::COIN_OF_THE_REALM => {
                if p == self.turn.player {
                    self.turn.actions += 2;
                    sink.bonus(p, f.subject, 2, 0, 0, 0);
                }
                // Another Coin on the mat may be called as well.
                if self.players[pi].tavern.has(f.subject) {
                    self.stack.push(Frame { depth, ..*f });
                }
            }
            ids::ROYAL_CARRIAGE => {
                // Replay the Action. A Duration replayed keeps the Carriage in play with it (like a Throne Room): not tracked while another
                // multiplier group is unwinding.
                let card = f.last;
                if cards::is(card, DURATION) && self.turn.multiplier_card == 0 && self.players[pi].in_play.has(f.subject) {
                    self.turn.multiplier_card = f.subject;
                    self.turn.multiplier_expected = 1;
                    self.turn.multiplier_successes = 0;
                    self.stack.push(Frame { depth, subject: card, ..Frame::new(K::MultiplierFinalize, p, f.subject) });
                }
                self.stack.push(Frame { subject: card, count: 2, depth, ..Frame::new(K::PlayEffects, p, f.subject) });
            }
            ids::DUPLICATE => {
                self.gain(p, f.last, Dest::Discard, false, f.subject, sink);
            }
            ids::GUIDE => {
                self.discard_whole_hand(p, sink);
                self.stack.push(Frame { depth, ..draw_frame(p, f.subject, 5) });
            }
            ids::RATCATCHER => {
                self.stack.push(Frame { depth, ..select(p, f.subject, Zone::Hand, Act::Trash, Filter::Any, 1, 1, Then::Nothing) });
            }
            ids::TRANSMOGRIFY => {
                self.stack.push(Frame {
                    depth,
                    ..select(
                        p, f.subject, Zone::Hand, Act::Trash, Filter::Any, 1, 1,
                        Then::GainUpTo { plus: 1, filter: Filter::Any, dest: Dest::Hand, exact: false, dest_by_type: false },
                    )
                });
            }
            ids::TEACHER => self.stack.push(Frame { depth, ..mode_frame(p, f.subject, 1) }),
            _ => {}
        }
    }

    /// Exchange the Traveller `c` (already out of play) for the next card of its line: it returns to its pile, the upgrade goes to the discard
    /// pile. (Not a gain or a trash.)
    #[inline(never)]
    pub(crate) fn exchange_traveller<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        let Some(next) = cards::traveller_upgrade(c) else { return };
        if c >= FIRST_TRAV {
            self.trav[(c - FIRST_TRAV) as usize] += 1;
        } else {
            self.supply.add(c, 1);
        }
        let slot = &mut self.trav[(next - FIRST_TRAV) as usize];
        *slot = slot.saturating_sub(1);
        self.players[p as usize].discard.add(next, 1);
        sink.event(Event::Exchange { player: p, from: c, to: next });
    }

    /// Whether the Traveller `c` can be exchanged now: its upgrade pile has a card.
    pub(crate) fn exchange_possible(&self, c: CardId) -> bool {
        cards::traveller_upgrade(c).is_some_and(|next| self.trav[(next - FIRST_TRAV) as usize] > 0)
    }

    /// The `K::YesNo` outcomes that are Adventures': a Raze that was not trashed, and Raze's look.
    #[inline(never)]
    pub(crate) fn decline_adv(&mut self, f: &Frame) {
        if f.then == Then::RazeSelf {
            self.stack.push(Frame { depth: f.depth, ..select(f.player, f.source, Zone::Hand, Act::Trash, Filter::Any, 1, 1, Then::RazeLook) });
        }
    }

    /// Raze: look at `n` cards from the top of the deck, put one into the hand and discard the rest.
    #[inline(never)]
    pub(crate) fn raze_look(&mut self, p: u8, source: CardId, n: u8, depth: u8) {
        if n == 0 {
            return;
        }
        // Pushed last-first: reveal, then keep one (the others are the picks to discard), the rest to the hand.
        self.stack.push(Frame {
            depth, down_to_target: true,
            ..select(p, source, Zone::Revealed, Act::Discard, Filter::Any, 0, 1, Then::RevealedToHand)
        });
        self.stack.push(Frame { depth, max: n, ..Frame::new(K::RevealTop, p, source) });
    }

    /// Magpie, once the top card is revealed (in `set_aside`): a Treasure to the hand, a card that is not a Treasure back on top; an Action or
    /// Victory card gains a Magpie.
    #[inline(never)]
    pub(crate) fn finish_magpie<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let Some((c, _)) = self.players[p as usize].set_aside.iter().next() else { return };
        self.players[p as usize].set_aside.remove(c);
        if self.is_treasure(c) {
            self.players[p as usize].hand.add(c, 1);
            sink.event(Event::Draw { player: p, card: c });
        } else {
            self.players[p as usize].deck_known.push_top(c);
            sink.event(Event::Topdeck { player: p, card: c });
        }
        if cards::is(c, ACTION) || cards::is(c, VICTORY) {
            self.gain(p, ids::MAGPIE, Dest::Discard, false, f.source, sink);
        }
    }

    /// The top card of the deck of the victim `f.player` for Giant and Warrior: taken, and dealt with by the attack.
    #[inline(never)]
    pub(crate) fn run_adv_top_card<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        match take_top!(self, p, sink) {
            None => {
                self.stack.pop();
                // Giant: with no card to reveal, the Curse is gained all the same ("If they have no cards they gain a Curse").
                if f.then == Then::GiantReveal {
                    self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
                }
            }
            Some(c) => {
                self.stack.pop();
                let trash_range = !cards::potion_cost(c) && {
                    let k = self.cost(c);
                    if f.then == Then::GiantReveal { (3..=6).contains(&k) } else { k == 3 || k == 4 }
                };
                if f.then == Then::GiantReveal {
                    self.reveal_card(p, c, sink);
                    if trash_range {
                        self.trash_card(p, c, true, sink);
                    } else {
                        self.put(p, c, Act::Discard, Dest::Discard, f.source, sink);
                        self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
                    }
                } else if trash_range {
                    // Warrior: discarded (not revealed), then trashed from the discard pile if it costs $3 or $4.
                    sink.event(Event::Discard { player: p, card: c });
                    self.trash_card(p, c, true, sink);
                } else {
                    self.put(p, c, Act::Discard, Dest::Discard, f.source, sink);
                }
                self.flush_trash_triggers(f.depth, sink);
            }
        }
        Run::Continue
    }

    /// Storyteller: one Treasure of the hand, picked, is played now (a choice-free one by the usual path, so Merchant and Sauna see it).
    #[inline(never)]
    pub(crate) fn storyteller_play<S: EventSink>(&mut self, f: &Frame, c: CardId, sink: &mut S) {
        let p = f.player;
        if self.plain_treasure(c) {
            // Still in the hand: the usual path moves it.
            self.play_free_treasure(c, 1, sink);
        } else {
            self.players[p as usize].hand.remove(c);
            self.players[p as usize].in_play.add(c, 1);
            sink.event(Event::Play { player: p, card: c });
            self.resolve_effects(c, p, f.depth, sink);
        }
    }

    /// Storyteller, paying: all the $ are spent for +1 Card each.
    #[inline(never)]
    pub(crate) fn storyteller_pay(&mut self, f: &Frame) {
        let n = self.turn.coins.min(u8::MAX as u16) as u8;
        self.turn.coins -= n as u16;
        if n > 0 {
            self.stack.push(Frame { depth: f.depth, ..draw_frame(f.player, f.source, n) });
        }
    }

    /// Disciple: the picked Action is played twice, then a copy of it is gained.
    #[inline(never)]
    pub(crate) fn push_play_twice_copy(&mut self, f: &Frame) {
        if f.count == 0 {
            return;
        }
        let p = f.player;
        self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(f.last), Dest::Discard) });
        // The two plays are Throne Room's (a Duration keeps the Disciple in play with it).
        self.push_plays(f, 2);
    }

    /// Inheritance: the card picked (set aside from the Supply) is under the Estate token.
    #[inline(never)]
    pub(crate) fn finish_inherit(&mut self, f: &Frame) {
        if f.count == 0 {
            return;
        }
        let ps = &mut self.players[f.player as usize];
        if ps.set_aside.remove(f.last) {
            ps.inherit = f.last;
        }
        if f.last >= cards::FIRST_PILE_CARD {
            self.advance_pile(f.last);
        }
    }

    /// The tokens on the pile `card` is from give their owner `p` their bonus before the card does anything: +1 Action, +1 Buy and +$1 now,
    /// +1 Card as a draw on top of the stack (so it comes before the card's own effects, even an Attack's reactions).
    #[inline(never)]
    pub(crate) fn play_tokens<S: EventSink>(&mut self, card: CardId, p: u8, sink: &mut S) {
        let ps = &self.players[p as usize];
        if ps.tok[..TOK_COST].iter().all(|&t| t == 0) {
            return;
        }
        let pile = cards::pile_of(card);
        let on = |tok: usize| ps.tok[tok] != 0 && ps.tok[tok] == pile;
        let (a, b, k, draw) = (on(TOK_ACTION) as u8, on(TOK_BUY) as u8, on(TOK_COIN) as u16, on(TOK_CARD));
        if a + b != 0 || k != 0 {
            if p == self.turn.player {
                self.turn.actions += a;
                self.turn.buys += b;
                self.turn.coins += k;
            }
            sink.bonus(p, card, a, b, k, 0);
        }
        if draw {
            self.stack.push(draw_frame(p, card, 1));
        }
    }

    /// Whether `p`'s Estates are Action (Command) cards now: they bought Inheritance and it is their turn.
    #[inline]
    pub(crate) fn estates_are_actions(&self, p: u8) -> bool {
        self.players[p as usize].inherit != 0 && p == self.turn.player
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Action or Treasure of the set says whether playing it is choice-free (`choice_free(..)`), has a choice (`has_choice(..)`) or is
    /// order-sensitive (`order_sensitive(..)`); bots skip the play-order search only for choice-free hands. When adding cards, mark each one and
    /// update the lists below deliberately.
    #[test]
    fn choice_free_and_order_sensitive_cards() {
        let names = |kind| CARDS.iter().map(|(_, d)| d).filter(|d| d.on_play == kind).map(|d| d.name).collect::<Vec<_>>();
        assert_eq!(
            names(cards::OnPlay::ChoiceFree),
            [
                "Page", "Peasant", "Ratcatcher", "Caravan Guard", "Guide", "Duplicate", "Port", "Ranger", "Transmogrify", "Bridge Troll",
                "Distant Lands", "Giant", "Haunted Woods", "Lost City", "Royal Carriage", "Swamp Hag", "Wine Merchant", "Hireling", "Warrior",
                "Champion", "Soldier", "Teacher",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Raze", "Amulet", "Dungeon", "Gear", "Magpie", "Messenger", "Artificer", "Storyteller", "Fugitive", "Disciple",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
        assert_eq!(
            names(cards::OnPlay::Choice),
            ["Coin of the Realm", "Miser", "Relic", "Treasure Trove", "Treasure Hunter", "Hero"],
        );
    }

    #[test]
    fn token_names_and_events() {
        assert_eq!(cards::token_by_name("+1 Card"), Some(0));
        assert_eq!(cards::token_by_name("-$2"), Some(4));
        assert_eq!(cards::token_by_name("\u{2212}$2 cost"), Some(4));
        assert_eq!(cards::token_by_name("+1 coin"), Some(3));
        assert_eq!(cards::token_by_name("Trashing token"), Some(5));
        assert_eq!(crate::state::NUM_PILE_TOKENS, 6);
        for e in FIRST_EVENT..FIRST_EVENT + cards::NUM_ADV_EVENTS as CardId {
            assert!(cards::is_event(e) && cards::set_of(e) == CardSet::Adventures);
        }
    }
}
