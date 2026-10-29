//! Empires: Debt (as a cost and as tokens), split piles, the Castles, Gathering cards (VP tokens on piles), 13 Events and 21 Landmarks.
//! See `docs/empires-plan.md` for the rulings, the Debt analysis and the design.

use super::prelude::*;
use crate::cards::{emp, with_debt, ACTION as ACT, ATTACK as ATK, CASTLE as CAS, COMMAND as COM, DURATION as DUR, TREASURE as TRE, VICTORY as VIC};
use crate::state::{EmpState, GameConfig, TypeSet, E_CASTLES, E_ENCHANT, E_GAIN_WATCH, E_KEEP_DEBT, E_TAX, E_TOMB, P_DONATE};

/// Card ids of this set. 233-256 are the 24 kingdom piles (a split pile is named by its top half; the Castles by their marker), 259-263 the
/// bottom halves of the five split piles (`cards::is_split_bottom`; Avanto, the first bottom, is the Promo set's) and 264-271 the Castles.
/// The Events (316-328) and the Landmarks (329-349) are pseudo-cards above every id that can sit in a zone.
pub mod ids {
    use crate::cards::CardId;
    pub const ENGINEER: CardId = 233;
    pub const CITY_QUARTER: CardId = 234;
    pub const OVERLORD: CardId = 235;
    pub const ROYAL_BLACKSMITH: CardId = 236;
    pub const ENCAMPMENT: CardId = 237;
    pub const PATRICIAN: CardId = 238;
    pub const SETTLERS: CardId = 239;
    /// The Castles pile (a kingdom card by name, standing for the eight different Castles below; never in a zone).
    pub const CASTLES: CardId = 240;
    pub const CATAPULT: CardId = 241;
    pub const CHARIOT_RACE: CardId = 242;
    pub const ENCHANTRESS: CardId = 243;
    pub const FARMERS_MARKET: CardId = 244;
    pub const GLADIATOR: CardId = 245;
    pub const SACRIFICE: CardId = 246;
    pub const TEMPLE: CardId = 247;
    pub const VILLA: CardId = 248;
    pub const ARCHIVE: CardId = 249;
    pub const CAPITAL: CardId = 250;
    pub const CHARM: CardId = 251;
    pub const CROWN: CardId = 252;
    pub const FORUM: CardId = 253;
    pub const GROUNDSKEEPER: CardId = 254;
    pub const LEGIONARY: CardId = 255;
    pub const WILD_HUNT: CardId = 256;
    // (257 Change: rising_sun.rs, 258 Avanto: promo.rs)
    pub const PLUNDER: CardId = 259;
    pub const EMPORIUM: CardId = 260;
    pub const BUSTLING_VILLAGE: CardId = 261;
    pub const ROCKS: CardId = 262;
    pub const FORTUNE: CardId = 263;
    pub const HUMBLE_CASTLE: CardId = 264;
    pub const CRUMBLING_CASTLE: CardId = 265;
    pub const SMALL_CASTLE: CardId = 266;
    pub const HAUNTED_CASTLE: CardId = 267;
    pub const OPULENT_CASTLE: CardId = 268;
    pub const SPRAWLING_CASTLE: CardId = 269;
    pub const GRAND_CASTLE: CardId = 270;
    pub const KINGS_CASTLE: CardId = 271;
    // The Events.
    pub const ADVANCE: CardId = 578;
    pub const ANNEX: CardId = 579;
    pub const BANQUET: CardId = 580;
    pub const CONQUEST: CardId = 581;
    pub const DELVE: CardId = 582;
    pub const DOMINATE: CardId = 583;
    pub const DONATE: CardId = 584;
    pub const RITUAL: CardId = 585;
    pub const SALT_THE_EARTH: CardId = 586;
    pub const TAX: CardId = 587;
    pub const TRIUMPH: CardId = 588;
    pub const WEDDING: CardId = 589;
    pub const WINDFALL: CardId = 590;
    // The Landmarks.
    pub const AQUEDUCT: CardId = 637;
    pub const ARENA: CardId = 638;
    pub const BANDIT_FORT: CardId = 639;
    pub const BASILICA: CardId = 640;
    pub const BATHS: CardId = 641;
    pub const BATTLEFIELD: CardId = 642;
    pub const COLONNADE: CardId = 643;
    pub const DEFILED_SHRINE: CardId = 644;
    pub const FOUNTAIN: CardId = 645;
    pub const KEEP: CardId = 646;
    pub const LABYRINTH: CardId = 647;
    pub const MOUNTAIN_PASS: CardId = 648;
    pub const MUSEUM: CardId = 649;
    pub const OBELISK: CardId = 650;
    pub const ORCHARD: CardId = 651;
    pub const PALACE: CardId = 652;
    pub const TOMB: CardId = 653;
    pub const TOWER: CardId = 654;
    pub const TRIUMPHAL_ARCH: CardId = 655;
    pub const WALL: CardId = 656;
    pub const WOLF_DEN: CardId = 657;
}

/// An Event: no types, no bonuses; its cost is the cost to buy it (`with_debt` adds the Debt part).
const fn event(name: &'static str, cost: u8) -> CardDef {
    emp(c(name, cost, 0, 0, 0, 0, 0, 0))
}

/// A Landmark: no cost, nothing to buy.
const fn landmark(name: &'static str) -> CardDef {
    emp(c(name, 0, 0, 0, 0, 0, 0, 0))
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::ENGINEER, emp(with_debt(has_choice(c("Engineer", 0, ACT, 0, 0, 0, 0, 0)), 4))), // gains, may trash itself to gain again
    (ids::CITY_QUARTER, emp(with_debt(order_sensitive(c("City Quarter", 0, ACT, 0, 0, 0, 2, 0)), 8))), // +1 Card per Action in the hand
    (ids::OVERLORD, emp(with_debt(order_sensitive(c("Overlord", 0, ACT | COM, 0, 0, 0, 0, 0)), 8))), // plays an Action from the Supply
    (ids::ROYAL_BLACKSMITH, emp(with_debt(order_sensitive(c("Royal Blacksmith", 0, ACT, 0, 0, 5, 0, 0)), 8))), // discards the Coppers drawn
    (ids::ENCAMPMENT, emp(has_choice(c("Encampment", 2, ACT, 0, 0, 2, 2, 0)))), // may reveal a Gold or Plunder, else returns to its pile
    (ids::PATRICIAN, emp(order_sensitive(c("Patrician", 2, ACT, 0, 0, 1, 1, 0)))), // reveals the top card: a miss stays known
    (ids::SETTLERS, emp(has_choice(c("Settlers", 2, ACT, 0, 0, 1, 1, 0)))), // may take a Copper from the discard pile
    (ids::CASTLES, emp(c("Castles", 3, 0, 0, 0, 0, 0, 0))),
    (ids::CATAPULT, emp(order_sensitive(c("Catapult", 3, ACT | ATK, 1, 0, 0, 0, 0)))), // trashes a card of the hand
    (ids::CHARIOT_RACE, emp(choice_free(c("Chariot Race", 3, ACT, 0, 0, 0, 1, 0)))), // draws its card itself: +$1 and a VP token when it costs more than the left player's top card
    (ids::ENCHANTRESS, emp(choice_free(c("Enchantress", 3, ACT | DUR | ATK, 0, 0, 0, 0, 0)))),
    (ids::FARMERS_MARKET, emp(choice_free(c("Farmers' Market", 3, ACT, 0, 0, 0, 0, 1)))), // +$ and the pile's VP tokens
    (ids::GLADIATOR, emp(has_choice(c("Gladiator", 3, ACT, 2, 0, 0, 0, 0)))), // reveals a card of the hand
    (ids::SACRIFICE, emp(order_sensitive(c("Sacrifice", 4, ACT, 0, 0, 0, 0, 0)))), // trashes a card of the hand
    (ids::TEMPLE, emp(order_sensitive(c("Temple", 4, ACT, 0, 0, 0, 0, 0)))), // trashes 1-3 cards of the hand
    (ids::VILLA, emp(choice_free(c("Villa", 4, ACT, 1, 0, 0, 2, 1)))),
    (ids::ARCHIVE, emp(order_sensitive(c("Archive", 5, ACT | DUR, 0, 0, 0, 1, 0)))), // sets aside the top 3 cards
    (ids::CAPITAL, emp(choice_free(c("Capital", 5, TRE, 6, 0, 0, 0, 1)))), // 6 Debt when discarded from play
    (ids::CHARM, emp(has_choice(c("Charm", 5, TRE, 0, 0, 0, 0, 0)))),
    (ids::CROWN, emp(order_sensitive(plays(c("Crown", 5, ACT | TRE, 0, 0, 0, 0, 0), 2)))), // plays an Action (or a Treasure) twice
    (ids::FORUM, emp(order_sensitive(c("Forum", 5, ACT, 0, 0, 3, 1, 0)))), // discards 2 cards of the hand
    (ids::GROUNDSKEEPER, emp(choice_free(c("Groundskeeper", 5, ACT, 0, 0, 1, 1, 0)))),
    (ids::LEGIONARY, emp(has_choice(c("Legionary", 5, ACT | ATK, 3, 0, 0, 0, 0)))), // may reveal a Gold
    (ids::WILD_HUNT, emp(has_choice(c("Wild Hunt", 5, ACT, 0, 0, 0, 0, 0)))),
    (ids::PLUNDER, emp(choice_free(c("Plunder", 5, TRE, 2, 0, 0, 0, 0)))), // +1 VP token
    (ids::EMPORIUM, emp(choice_free(c("Emporium", 5, ACT, 1, 0, 1, 1, 0)))),
    (ids::BUSTLING_VILLAGE, emp(has_choice(c("Bustling Village", 5, ACT, 0, 0, 1, 3, 0)))), // may take a Settlers from the discard pile
    (ids::ROCKS, emp(on_trash(choice_free(c("Rocks", 4, TRE, 1, 0, 0, 0, 0))))),
    (ids::FORTUNE, emp(with_debt(has_choice(c("Fortune", 8, TRE, 0, 0, 0, 0, 1)), 8))), // doubles the $
    (ids::HUMBLE_CASTLE, emp(choice_free(c("Humble Castle", 3, TRE | VIC | CAS, 1, 0, 0, 0, 0)))),
    (ids::CRUMBLING_CASTLE, emp(on_trash(c("Crumbling Castle", 4, VIC | CAS, 0, 1, 0, 0, 0)))),
    (ids::SMALL_CASTLE, emp(on_trash(has_choice(c("Small Castle", 5, ACT | VIC | CAS, 0, 2, 0, 0, 0))))), // trashes itself or a Castle to gain a Castle
    (ids::HAUNTED_CASTLE, emp(c("Haunted Castle", 6, VIC | CAS, 0, 2, 0, 0, 0))),
    (ids::OPULENT_CASTLE, emp(has_choice(c("Opulent Castle", 7, ACT | VIC | CAS, 0, 3, 0, 0, 0)))), // discards Victory cards for $
    (ids::SPRAWLING_CASTLE, emp(c("Sprawling Castle", 8, VIC | CAS, 0, 4, 0, 0, 0))),
    (ids::GRAND_CASTLE, emp(c("Grand Castle", 9, VIC | CAS, 0, 5, 0, 0, 0))),
    (ids::KINGS_CASTLE, emp(c("King's Castle", 10, VIC | CAS, 0, 0, 0, 0, 0))), // 2 VP per Castle (`vp`)
    // The Events.
    (ids::ADVANCE, event("Advance", 0)),
    (ids::ANNEX, with_debt(event("Annex", 0), 8)),
    (ids::BANQUET, event("Banquet", 3)),
    (ids::CONQUEST, event("Conquest", 6)),
    (ids::DELVE, event("Delve", 2)),
    (ids::DOMINATE, event("Dominate", 14)),
    (ids::DONATE, with_debt(event("Donate", 0), 8)),
    (ids::RITUAL, event("Ritual", 4)),
    (ids::SALT_THE_EARTH, event("Salt the Earth", 4)),
    (ids::TAX, event("Tax", 2)),
    (ids::TRIUMPH, with_debt(event("Triumph", 0), 5)),
    (ids::WEDDING, with_debt(event("Wedding", 4), 3)),
    (ids::WINDFALL, event("Windfall", 5)),
    // The Landmarks.
    (ids::AQUEDUCT, landmark("Aqueduct")),
    (ids::ARENA, landmark("Arena")),
    (ids::BANDIT_FORT, landmark("Bandit Fort")),
    (ids::BASILICA, landmark("Basilica")),
    (ids::BATHS, landmark("Baths")),
    (ids::BATTLEFIELD, landmark("Battlefield")),
    (ids::COLONNADE, landmark("Colonnade")),
    (ids::DEFILED_SHRINE, landmark("Defiled Shrine")),
    (ids::FOUNTAIN, landmark("Fountain")),
    (ids::KEEP, landmark("Keep")),
    (ids::LABYRINTH, landmark("Labyrinth")),
    (ids::MOUNTAIN_PASS, landmark("Mountain Pass")),
    (ids::MUSEUM, landmark("Museum")),
    (ids::OBELISK, landmark("Obelisk")),
    (ids::ORCHARD, landmark("Orchard")),
    (ids::PALACE, landmark("Palace")),
    (ids::TOMB, landmark("Tomb")),
    (ids::TOWER, landmark("Tower")),
    (ids::TRIUMPHAL_ARCH, landmark("Triumphal Arch")),
    (ids::WALL, landmark("Wall")),
    (ids::WOLF_DEN, landmark("Wolf Den")),
];

/// The Landmark bit of `landmark` in `EmpState::landmarks`.
#[inline(always)]
pub const fn lm_bit(landmark: CardId) -> u32 {
    1u32 << (landmark - cards::FIRST_LANDMARK)
}

/// The index of a Landmark in `EmpState::lm_vp`.
#[inline(always)]
pub const fn lm_index(landmark: CardId) -> usize {
    (landmark - cards::FIRST_LANDMARK) as usize
}

/// The highest bid at Mountain Pass.
pub const MAX_BID: u8 = 40;

/// The Landmarks that start with 6 VP tokens per player.
const LM_TOKEN_PILES: [CardId; 6] = [ids::ARENA, ids::BASILICA, ids::BATHS, ids::BATTLEFIELD, ids::COLONNADE, ids::LABYRINTH];

/// The Landmarks that act when a card is gained: with any of them in the game (or Tax, Conquest, Triumph, which count gains) gains go
/// through `GameState::watch_gain` (`E_GAIN_WATCH`).
const GAIN_WATCHERS: u32 = lm_bit(ids::AQUEDUCT)
    | lm_bit(ids::BASILICA)
    | lm_bit(ids::BATHS)
    | lm_bit(ids::BATTLEFIELD)
    | lm_bit(ids::COLONNADE)
    | lm_bit(ids::DEFILED_SHRINE)
    | lm_bit(ids::LABYRINTH)
    | lm_bit(ids::MOUNTAIN_PASS);

/// What Empires adds to a new game: the Landmarks and their tokens, Tax's Debt, the split piles' bottoms, the Castles and the Obelisk pile.
/// Called by `GameState::new` once the Supply piles of the kingdom are in.
pub(crate) fn setup(g: &mut GameState, cfg: &GameConfig, num_players: usize) {
    // Young Witch's Bane and Ferryman's card are set up like kingdom cards ("do any set-up the card requires").
    let extra = [g.bane, g.ferryman];
    let has = |c: CardId| cfg.kingdom.contains(&c) || (c != 0 && extra.contains(&c));
    let mut emp = EmpState::default();
    for &k in cfg.kingdom.iter().chain(extra.iter().filter(|&&c| c != 0)) {
        if cards::is_landmark(k) {
            emp.landmarks |= lm_bit(k);
        }
    }
    for lm in LM_TOKEN_PILES {
        if emp.landmarks & lm_bit(lm) != 0 {
            emp.lm_vp[lm_index(lm)] = 6 * num_players as u8;
        }
    }
    if emp.landmarks & GAIN_WATCHERS != 0 || has(ids::TAX) || has(ids::CONQUEST) || has(ids::TRIUMPH) {
        emp.flags |= E_GAIN_WATCH;
    }
    if has(ids::TAX) {
        emp.flags |= E_TAX;
    }
    if has(id::CHANGE) {
        emp.flags |= E_KEEP_DEBT;
    }
    if emp.landmarks & lm_bit(ids::TOMB) != 0 {
        emp.flags |= E_TOMB;
    }
    g.emp = emp;
    if has(ids::CASTLES) {
        g.emp.flags |= E_CASTLES;
        for (i, m) in (ids::HUMBLE_CASTLE..ids::HUMBLE_CASTLE + 8).enumerate() {
            // Two players use one of each Castle; more add a second Humble, Small, Opulent and King's Castle.
            let double = num_players > 2 && matches!(i, 0 | 2 | 4 | 7);
            g.supply.set(m, 1 + double as u8);
            g.in_supply.insert(m);
        }
    }
    for (top, bottom) in cards::SPLIT_PILES {
        if top != id::SAUNA && g.in_supply(top) {
            g.supply.set(bottom, 5);
            g.in_supply.insert(bottom);
        }
    }
    if emp.landmarks & lm_bit(ids::AQUEDUCT) != 0 {
        g.emp.piles.add_vp(id::SILVER, 8);
        g.emp.piles.add_vp(id::GOLD, 8);
    }
    // The Action Supply piles (named as `cards::pile_of` names them): Defiled Shrine's 2 VP each (not on the Gathering piles) and the Obelisk's pile.
    if emp.landmarks & (lm_bit(ids::DEFILED_SHRINE) | lm_bit(ids::OBELISK)) != 0 {
        let mut piles = [0 as CardId; 40];
        let mut n = 0;
        for c in g.supply_cards() {
            if cards::pile_of(c) == c && cards::is(c, ACT) && c < cards::FIRST_PILE_CARD && n < piles.len() {
                piles[n] = c;
                n += 1;
            }
        }
        for pk in [id::KNIGHTS, id::RUINS] {
            if let Some(mp) = crate::sets::dark_ages::MixedPile::of_marker(pk) {
                if g.has_pile(mp) && n < piles.len() {
                    piles[n] = pk;
                    n += 1;
                }
            }
        }
        if emp.landmarks & lm_bit(ids::DEFILED_SHRINE) != 0 {
            for &pile in &piles[..n] {
                if !cards::is_gathering(pile) {
                    g.emp.piles.add_vp(pile, 2);
                }
            }
        }
        if emp.landmarks & lm_bit(ids::OBELISK) != 0 && n > 0 {
            g.emp.obelisk = cfg.obelisk.filter(|o| piles[..n].contains(o)).unwrap_or_else(|| {
                let mut z = cfg.seed.wrapping_add(0x0BE1_15C0_9E37_79B9).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^= z >> 32;
                piles[(z % n as u64) as usize]
            });
        }
    }
    // Tax: every Supply pile starts with 1 Debt on it (a split pile, the Castles, the Knights and the Ruins are one pile each).
    if has(ids::TAX) {
        let supply: Vec<CardId> = g.supply_cards().collect();
        for c in supply {
            if cards::pile_of(c) == c {
                g.emp.piles.add_debt(c, 1);
            }
        }
        for pk in [id::KNIGHTS, id::RUINS] {
            if crate::sets::dark_ages::MixedPile::of_marker(pk).is_some_and(|mp| g.has_pile(mp)) {
                g.emp.piles.add_debt(pk, 1);
            }
        }
        if has(ids::CASTLES) {
            g.emp.piles.add_debt(ids::CASTLES, 1);
        }
    }
}

/// The Empires cards (see the module docs of `sets`).
pub struct Empires;

impl SetRules for Empires {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Patrician alone: +1 Card +1 Action, and a revealed card costing $5 or more goes to the hand. Forum alone: +3 Cards +1 Action,
    /// then 2 of the larger hand discarded (the cards it drew can be the ones). Never worse than not playing them.
    fn safe_alone(card: CardId) -> bool {
        matches!(card, ids::PATRICIAN | ids::FORUM)
    }
    /// "When you gain" effects by card: Temple (the VP tokens), Villa and Forum; above id 255, Emporium, Rocks, Fortune and the Castles that
    /// do something (Crumbling, Haunted, Sprawling, Grand).
    const GAIN_HI: u128 = (1u128 << (ids::TEMPLE - 128)) | (1u128 << (ids::VILLA - 128)) | (1u128 << (ids::FORUM - 128));
    const GAIN_TOP: u128 = (1u128 << (ids::EMPORIUM - 256))
        | (1u128 << (ids::ROCKS - 256))
        | (1u128 << (ids::FORTUNE - 256))
        | (1u128 << (ids::CRUMBLING_CASTLE - 256))
        | (1u128 << (ids::HAUNTED_CASTLE - 256))
        | (1u128 << (ids::SPRAWLING_CASTLE - 256))
        | (1u128 << (ids::GRAND_CASTLE - 256));

    /// The bottom halves of the split piles and the Castles can be gained only from the top of their pile.
    const MAY_BUY_CARDS: &'static [CardId] = &[
        ids::PLUNDER, ids::EMPORIUM, ids::BUSTLING_VILLAGE, ids::ROCKS, ids::FORTUNE, ids::HUMBLE_CASTLE, ids::CRUMBLING_CASTLE,
        ids::SMALL_CASTLE, ids::HAUNTED_CASTLE, ids::OPULENT_CASTLE, ids::SPRAWLING_CASTLE, ids::GRAND_CASTLE, ids::KINGS_CASTLE,
    ];
    fn may_buy(g: &GameState, card: CardId) -> bool {
        g.pile_open(card)
    }

    /// Five of each half of a split pile (the bottom halves are set up with them).
    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        matches!(card, ids::ENCAMPMENT | ids::PATRICIAN | ids::SETTLERS | ids::CATAPULT | ids::GLADIATOR).then_some(5)
    }

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            ids::WILD_HUNT => &[ModeOpt::CardsGather(3, 1), ModeOpt::GainTakeVp(id::ESTATE)],
            ids::CHARM => &[ModeOpt::BuysCoins(1, 2), ModeOpt::NextGainDouble],
            ids::SPRAWLING_CASTLE => &[ModeOpt::Gain(id::DUCHY, Dest::Discard), ModeOpt::GainMany(id::ESTATE, Dest::Discard, 3)],
            _ => &[],
        }
    }

    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::ENCHANTRESS => (2, 0, 0, 0),
            _ => (0, 0, 0, 0),
        }
    }

    /// Archive puts a card into the hand (a choice among those it holds), so its order among the start-of-turn effects can matter.
    fn start_order_sensitive(card: CardId) -> bool {
        card == ids::ARCHIVE
    }

    /// Archive schedules the turns it still has a card for once it knows how many it set aside.
    fn schedules_own_duration(card: CardId) -> bool {
        card == ids::ARCHIVE
    }

    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        match card {
            ids::HUMBLE_CASTLE => all.count_type(CAS) as i32 + cards::def(card).vp as i32,
            ids::KINGS_CASTLE => 2 * all.count_type(CAS) as i32,
            _ => cards::def(card).vp as i32,
        }
    }

    #[inline(never)]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        match card {
            ids::ENGINEER => {
                // Gain a card costing up to $4; then this may be trashed for another.
                if g.self_in_play_now(p, card) {
                    g.stack.push(Frame { zone: Zone::InPlay, act: Trash, subject: card, then: Then::EngineerGain, ..Frame::new(K::YesNo, p, card) });
                }
                g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard));
            }
            ids::CITY_QUARTER => {
                // Reveal the hand: +1 Card per Action in it (the hand as it is at the time of the play).
                let hand = g.players[pi].hand;
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        g.reveal_card(p, c, sink);
                    }
                }
                let n = hand.count_type(ACT).min(u8::MAX as u32) as u8;
                if n > 0 {
                    g.stack.push(draw_frame(p, card, n));
                }
            }
            ids::OVERLORD => {
                g.stack.push(select(p, card, Zone::Supply, Play, Filter::CommandCostUpTo(5), 1, 1, Then::PlayPicked { times: 1 }));
            }
            ids::ROYAL_BLACKSMITH => {
                // After the +5 Cards (pushed last by the generic code): reveal the hand and discard the Coppers in it.
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Card(id::COPPER), ALL, ALL, Then::Nothing));
            }
            ids::ENCAMPMENT => {
                g.stack.push(select(p, card, Zone::Hand, Reveal, Filter::Either(id::GOLD, ids::PLUNDER), 0, 1, Then::EncampmentReveal));
            }
            ids::PATRICIAN => g.stack.push(Frame { max: 1, then: Then::PatricianCheck, ..Frame::new(K::RevealTop, p, card) }),
            ids::SETTLERS => {
                g.stack.push(Frame { zone: Zone::Discard, act: ToHand, subject: id::COPPER, ..Frame::new(K::YesNo, p, card) });
            }
            ids::BUSTLING_VILLAGE => {
                g.stack.push(Frame { zone: Zone::Discard, act: ToHand, subject: ids::SETTLERS, ..Frame::new(K::YesNo, p, card) });
            }
            ids::CATAPULT => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::CatapultHit)),
            ids::CHARIOT_RACE => g.stack.push(Frame { then: Then::ChariotRace, ..Frame::new(K::TopCard, p, card) }),
            ids::ENCHANTRESS => {
                let (vs, n) = g.victims(sink);
                for &v in &vs[..n] {
                    g.players[v as usize].ench_by |= 1 << p;
                }
                if n > 0 {
                    g.emp.flags |= E_ENCHANT;
                }
            }
            ids::FARMERS_MARKET => g.farmers_market(p, card, sink),
            ids::GLADIATOR => g.stack.push(select(p, card, Zone::Hand, Reveal, Filter::Any, 1, 1, Then::GladiatorReveal)),
            ids::SACRIFICE => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::SacrificeBonus)),
            ids::TEMPLE => {
                g.gain_vp_tokens(p, card, 1, sink);
                g.stack.push(Frame { distinct: true, ..select(p, card, Zone::Hand, Trash, Filter::Any, 1, 3, Then::AddPileVp) });
            }
            ids::ARCHIVE => {
                // Set aside the top 3 cards face down; one goes into the hand now (chosen), then one at the start of each of the next two turns.
                g.stack.push(Frame { max: 3, then: Then::ArchiveSet, ..Frame::new(K::RevealTop, p, card) });
            }
            ids::CHARM => g.stack.push(mode_frame(p, card, 1)),
            ids::CROWN => {
                // An Action played twice in the Action phase, a Treasure in the Buy phase.
                if g.turn.phase == Phase::Buy {
                    g.stack.push(select(p, card, Zone::Hand, Play, g.treasure_filter(), 0, 1, Then::CrownTreasure));
                } else {
                    g.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 2 }));
                }
            }
            ids::FORUM => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 2, 2, Then::Nothing)),
            ids::GROUNDSKEEPER => g.turn.emp.groundskeepers = g.turn.emp.groundskeepers.saturating_add(1),
            ids::LEGIONARY => g.stack.push(select(p, card, Zone::Hand, Reveal, Filter::Card(id::GOLD), 0, 1, Then::LegionaryHit)),
            ids::WILD_HUNT => g.stack.push(mode_frame(p, card, 1)),
            ids::FORTUNE => {
                // Double the $ the first time a Fortune is played in a turn.
                if !g.turn.emp.doubled {
                    g.turn.emp.doubled = true;
                    let now = g.turn.coins;
                    g.turn.coins = now.saturating_mul(2);
                    sink.bonus(p, card, 0, 0, now, 0);
                }
            }
            ids::SMALL_CASTLE => {
                // Trash this or a Castle from the hand; gain a Castle. This first (a YesNo); declined, a Castle from the hand.
                if g.self_in_play_now(p, card) {
                    g.stack.push(Frame { zone: Zone::InPlay, act: Trash, subject: card, then: Then::SmallCastle, ..Frame::new(K::YesNo, p, card) });
                } else {
                    g.small_castle_hand(p, card);
                }
            }
            ids::OPULENT_CASTLE => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Victory, 0, ALL, Then::CoinsPerPick(2))),
            _ => {}
        }
    }

    /// Archive: the cards still set aside are drawn one a turn (the pool of the cards set aside by all of a player's Archives; each Archive
    /// gives one pick a turn while any card is left).
    #[inline(never)]
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) {
        if e.card != ids::ARCHIVE {
            return;
        }
        let pool = g.players[p as usize].archive.total().min(u8::MAX as u32) as u8;
        let picks = e.times.min(pool);
        for _ in 0..picks {
            g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Archive, Act::ToHand, Filter::Any, 1, 1, Then::Nothing) });
        }
        // The Archives with cards left come back next turn.
        let left = pool - picks;
        if left > 0 {
            g.push_duration_pending(p, e.card, picks.min(left), 0);
        }
    }

    /// "When you gain" effects of the cards, then the Landmarks and tokens that watch gains.
    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, on_buy: bool, sink: &mut S) {
        let pi = p as usize;
        let own_turn = p == g.turn.player;
        // Charm(s) are spent by the first gain, taken before the card's own trigger gains anything else (Rocks' Silver is a later gain).
        let charms = if own_turn { std::mem::take(&mut g.turn.emp.charms) } else { 0 };
        match c {
            ids::VILLA => {
                // Into the hand with +1 Action; gained in the Buy phase, back to the Action phase. It moves the gained copy at once: the
                // reactions still waiting on it (Watchtower, ...) find it gone (the stop-moving rule).
                if to != Dest::Hand {
                    let gt = g.turn.led.ingain;
                    g.tok_moved(gt, p, GameState::gain_dest_zone(to), c);
                }
                let ps = &mut g.players[pi];
                let moved = match to {
                    Dest::Discard => ps.discard.remove(c),
                    Dest::Hand => false,
                    Dest::DeckTop => ps.deck_known.peek_top() == Some(c) && ps.deck_known.pop_top().is_some(),
                };
                if moved {
                    ps.hand.add(c, 1);
                    sink.event(Event::TakeFromTrash { player: p, card: c });
                }
                if own_turn {
                    g.turn.actions += 1;
                    sink.bonus(p, c, 1, 0, 0, 0);
                    if g.turn.phase == Phase::Buy {
                        g.back_to_action_phase(p, sink);
                    }
                }
            }
            ids::EMPORIUM => {
                if g.players[pi].in_play.count_type(ACT) >= 5 {
                    g.gain_vp_tokens(p, c, 2, sink);
                }
            }
            ids::FORUM => {
                if own_turn {
                    g.turn.buys += 1;
                    sink.bonus(p, c, 0, 1, 0, 0);
                }
            }
            ids::FORTUNE => {
                for _ in 0..g.players[pi].in_play.get(ids::GLADIATOR) {
                    g.gain(p, id::GOLD, Dest::Discard, false, c, sink);
                }
            }
            ids::ROCKS => g.rocks_silver(p, c, sink),
            ids::TEMPLE => {
                let n = g.emp.piles.take_vp(ids::TEMPLE);
                if n > 0 {
                    g.gain_vp_tokens(p, c, n as u16, sink);
                }
            }
            ids::CRUMBLING_CASTLE => g.crumbling_castle(p, c, sink),
            ids::HAUNTED_CASTLE => {
                if own_turn {
                    g.gain(p, id::GOLD, Dest::Discard, false, c, sink);
                    // Each other player with 5 or more cards in hand puts 2 of them onto their deck (not an Attack: no reaction, no immunity).
                    let n = g.num_players;
                    for i in (1..n).rev() {
                        let v = (p + i) % n;
                        if g.players[v as usize].hand.total() >= 5 {
                            g.stack.push(select(v, c, Zone::Hand, Act::Topdeck, Filter::Any, 2, 2, Then::Nothing));
                        }
                    }
                }
            }
            ids::SPRAWLING_CASTLE => g.stack.push(mode_frame(p, c, 1)),
            ids::GRAND_CASTLE => {
                // Reveal the hand: +1 VP token per Victory card in the hand and in play.
                let ps = &g.players[pi];
                let n = (ps.hand.count_type(VIC) + ps.in_play.count_type(VIC)) as u16;
                let hand = ps.hand;
                for (card, k) in hand.iter() {
                    for _ in 0..k {
                        g.reveal_card(p, card, sink);
                    }
                }
                if n > 0 {
                    g.gain_vp_tokens(p, c, n, sink);
                }
            }
            _ => {}
        }
        // Groundskeeper(s) played this turn: +1 VP token for each Victory card gained.
        if g.turn.emp.groundskeepers > 0 && own_turn && cards::is(c, VIC) {
            g.gain_vp_tokens(p, ids::GROUNDSKEEPER, g.turn.emp.groundskeepers as u16, sink);
        }
        // Charm(s): the next gain of the turn may bring a differently named card of the same cost, one per Charm.
        if charms > 0 {
            for _ in 0..charms {
                g.stack.push(Frame {
                    exact: true, optional: true, potion: cards::potion_cost(c), min: cards::debt_cost(c),
                    ..gain_frame(p, ids::CHARM, g.cost(c), Filter::NotCard(c), Dest::Discard)
                });
            }
        }
        if g.emp.flags & (E_GAIN_WATCH | E_TAX) != 0 {
            g.watch_gain(p, c, on_buy, sink);
        }
    }

    /// Landmarks and tokens watch gains while one of them is in the game (`E_GAIN_WATCH`), and a Groundskeeper or Charm played this turn does.
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.emp.flags & (E_GAIN_WATCH | E_TAX) != 0 || g.turn.emp.groundskeepers != 0 || g.turn.emp.charms != 0
    }

    /// Rocks gives a Silver, Crumbling Castle a Silver and a VP token, when trashed.
    fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, sink: &mut S) {
        match card {
            ids::ROCKS => g.rocks_silver(p, card, sink),
            ids::CRUMBLING_CASTLE => g.crumbling_castle(p, card, sink),
            _ => {}
        }
    }
}

// -------------------------------------------------------------------------------------------
// Debt, tokens, the Landmarks
// -------------------------------------------------------------------------------------------

impl GameState {
    /// `p` takes `n` Debt tokens.
    pub(crate) fn take_debt<S: EventSink>(&mut self, p: u8, source: CardId, n: u16, sink: &mut S) {
        self.players[p as usize].debt += n;
        sink.event(Event::TakeDebt { player: p, source, n });
    }

    /// `p` pays off as much Debt as the $ allow: $1 per token.
    pub(crate) fn pay_debt<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let n = self.players[p as usize].debt.min(self.turn.coins);
        if n > 0 {
            self.turn.coins -= n;
            self.players[p as usize].debt -= n;
            sink.event(Event::PayDebt { player: p, n });
        }
    }

    /// `p` pays off Debt down to `keep` tokens, as far as the $ allow.
    pub(crate) fn pay_debt_keeping<S: EventSink>(&mut self, p: u8, keep: u16, sink: &mut S) {
        let n = self.players[p as usize].debt.saturating_sub(keep).min(self.turn.coins);
        if n > 0 {
            self.turn.coins -= n;
            self.players[p as usize].debt -= n;
            sink.event(Event::PayDebt { player: p, n });
        }
    }

    /// Whether the Buy phase still has a Debt decision for the turn's player although no Buy is left: a game with Change lets Debt stay
    /// unpaid (it makes Change give +$3), so paying is a choice there and is not done for the player at the end of the turn.
    #[inline(always)]
    pub(crate) fn debt_payment_open(&self) -> bool {
        self.emp.flags & E_KEEP_DEBT != 0 && self.players[self.turn.player as usize].debt > 0 && self.turn.coins > 0
    }

    /// `p` takes `n` VP tokens (from the supply of tokens).
    pub(crate) fn gain_vp_tokens<S: EventSink>(&mut self, p: u8, source: CardId, n: u16, sink: &mut S) {
        if n > 0 {
            self.players[p as usize].vp_tokens += n;
            sink.bonus(p, source, 0, 0, 0, n);
        }
    }

    /// Take up to `n` VP tokens from the Landmark `lm` for `p`; how many were there.
    pub(crate) fn take_landmark_vp<S: EventSink>(&mut self, p: u8, lm: CardId, n: u8, sink: &mut S) -> u8 {
        let i = lm_index(lm);
        let take = n.min(self.emp.lm_vp[i]);
        if take > 0 {
            self.emp.lm_vp[i] -= take;
            self.gain_vp_tokens(p, lm, take as u16, sink);
        }
        take
    }

    /// Whether `c` is on top of its pile, so that it can be gained or bought: the bottom half of a split pile only once the top half is
    /// gone, a Castle only when no cheaper Castle is left.
    #[inline]
    pub fn pile_open(&self, c: CardId) -> bool {
        // The split piles' bottom halves and the Castles are the ids from Avanto (14 in a row); the Allies split piles' cards are another run.
        if c.wrapping_sub(id::AVANTO) < 14 {
            if cards::is_split_bottom(c) {
                cards::split_top(c).map_or(true, |t| self.supply.get(t) == 0)
            } else {
                (ids::HUMBLE_CASTLE..c).all(|m| self.supply.get(m) == 0)
            }
        } else if cards::is_allies_split(c) {
            self.al_pile_open(c)
        } else {
            true
        }
    }

    /// The Castles left in the pile.
    pub fn castles_left(&self) -> u32 {
        (ids::HUMBLE_CASTLE..ids::HUMBLE_CASTLE + 8).map(|m| self.supply.get(m) as u32).sum()
    }

    /// The top Castle of the pile (the one that can be bought or gained), `None` when the pile is empty or not in the game.
    pub fn castle_top(&self) -> Option<CardId> {
        (ids::HUMBLE_CASTLE..ids::HUMBLE_CASTLE + 8).find(|&m| self.supply.get(m) > 0)
    }

    /// Cleanup, Empires: Capital's 6 Debt each time it is discarded from play, Baths' VP tokens for a turn with no gain, and (in a game
    /// without Change, where nobody wants to keep Debt) the $ left over paying off Debt.
    #[cold]
    #[inline(never)]
    pub(crate) fn empires_cleanup<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let capitals = self.players[p as usize].in_play.get(ids::CAPITAL) as u16;
        if capitals > 0 {
            self.take_debt(p, ids::CAPITAL, 6 * capitals, sink);
        }
        if self.emp.landmarks & lm_bit(ids::BATHS) != 0 && self.turn.emp.gains == 0 {
            self.take_landmark_vp(p, ids::BATHS, 2, sink);
        }
        if self.emp.flags & E_KEEP_DEBT == 0 {
            self.pay_debt(p, sink);
        }
    }

    /// The start of a Buy phase: Arena lets an Action in the hand be discarded for 2 VP tokens from it.
    #[cold]
    #[inline(never)]
    pub(crate) fn start_of_buy_landmarks(&mut self) {
        let p = self.turn.player;
        if self.emp.landmarks & lm_bit(ids::ARENA) != 0
            && self.emp.lm_vp[lm_index(ids::ARENA)] > 0
            && self.players[p as usize].hand.any_type(ACT)
        {
            self.stack.push(select(p, ids::ARENA, Zone::Hand, Act::Discard, Filter::Action, 0, 1, Then::ArenaVp));
        }
    }

    /// Enchantress: the first Action the turn's player plays this turn, if an Enchantress affects them, is +1 Card +1 Action instead of its
    /// instructions. Returns whether it was (then the card's own effects are skipped).
    #[inline(never)]
    pub(crate) fn enchanted_play<S: EventSink>(&mut self, card: CardId, p: u8, sink: &mut S) -> bool {
        let first = !self.turn.emp.acted;
        self.turn.emp.acted = true;
        if !first || self.players[p as usize].ench_by == 0 {
            return false;
        }
        self.turn.played.add(card, 1);
        self.turn.actions += 1;
        sink.bonus(p, card, 1, 0, 0, 0);
        sink.event(Event::Reaction { player: p, card: ids::ENCHANTRESS });
        self.stack.push(draw_frame(p, card, 1));
        true
    }

    /// The start of `p`'s turn: the Enchantresses they played stop affecting the others ("until your next turn").
    pub(crate) fn clear_enchantments(&mut self, p: u8) {
        for q in 0..self.num_players as usize {
            self.players[q].ench_by &= !(1 << p);
        }
    }

    /// Rocks gained or trashed: a Silver onto the deck in the Buy phase of its owner's turn, else into the hand.
    fn rocks_silver<S: EventSink>(&mut self, p: u8, source: CardId, sink: &mut S) {
        let to = if p == self.turn.player && self.turn.phase == Phase::Buy { Dest::DeckTop } else { Dest::Hand };
        self.gain(p, id::SILVER, to, false, source, sink);
    }

    /// Crumbling Castle gained or trashed: +1 VP token and a Silver.
    fn crumbling_castle<S: EventSink>(&mut self, p: u8, source: CardId, sink: &mut S) {
        self.gain_vp_tokens(p, source, 1, sink);
        self.gain(p, id::SILVER, Dest::Discard, false, source, sink);
    }

    /// Farmers' Market: with 4 or more VP tokens on its pile, take them and trash this; otherwise add one and get +$1 per token on the pile.
    fn farmers_market<S: EventSink>(&mut self, p: u8, card: CardId, sink: &mut S) {
        if !self.in_supply(card) {
            return;
        }
        let on = self.emp.piles.vp_on(card);
        if on >= 4 {
            let n = self.emp.piles.take_vp(card);
            self.gain_vp_tokens(p, card, n as u16, sink);
            if self.leave_play_self_now(p, card) {
                self.trash_card(p, card, true, sink);
            }
        } else {
            self.emp.piles.add_vp(card, 1);
            sink.event(Event::PileVp { pile: card, n: 1 });
            let coins = on as u16 + 1;
            self.turn.coins += coins;
            sink.bonus(p, card, 0, 0, coins, 0);
        }
    }

    /// Small Castle with no Small Castle in play to trash: a Castle from the hand to trash (for a Castle gained).
    fn small_castle_hand(&mut self, p: u8, card: CardId) {
        self.stack.push(select(
            p, card, Zone::Hand, Act::Trash, Filter::Types(TypeSet::Castle), 0, 1,
            Then::GainFixedUpTo { max_cost: u8::MAX, filter: Filter::Types(TypeSet::Castle), dest: Dest::Discard },
        ));
    }

    /// A card `c` was gained by `p`: the Landmarks and tokens that watch gains (only while one is in the game, `E_GAIN_WATCH` / `E_TAX`).
    #[inline(never)]
    fn watch_gain<S: EventSink>(&mut self, p: u8, c: CardId, on_buy: bool, sink: &mut S) {
        let _ = on_buy;
        let own_turn = p == self.turn.player;
        let pile = cards::pile_of(c);
        let lm = self.emp.landmarks;
        if own_turn {
            self.turn.emp.gains = self.turn.emp.gains.saturating_add(1);
            if c == id::SILVER {
                self.turn.emp.silvers = self.turn.emp.silvers.saturating_add(1);
            }
        }
        let buy_phase = own_turn && self.turn.phase == Phase::Buy;
        // Tax: a gain in the Buy phase takes the Debt on its pile.
        if self.emp.flags & E_TAX != 0 && buy_phase {
            let d = self.emp.piles.take_debt(pile);
            if d > 0 {
                self.take_debt(p, ids::TAX, d as u16, sink);
            }
        }
        if lm == 0 {
            return;
        }
        let treasure = self.is_treasure(c);
        let action = cards::is(c, ACT);
        let victory = cards::is(c, VIC);
        // Aqueduct: a Treasure gained moves 1 VP from its pile to the Aqueduct; a Victory card gained takes them.
        if lm & lm_bit(ids::AQUEDUCT) != 0 {
            if treasure && self.emp.piles.take_one_vp(pile) {
                self.emp.lm_vp[lm_index(ids::AQUEDUCT)] += 1;
                sink.event(Event::PileVp { pile: ids::AQUEDUCT, n: 1 });
            }
            if victory {
                let n = std::mem::take(&mut self.emp.lm_vp[lm_index(ids::AQUEDUCT)]);
                self.gain_vp_tokens(p, ids::AQUEDUCT, n as u16, sink);
            }
        }
        // Defiled Shrine: an Action gained moves 1 VP from its pile to the Shrine; a Curse bought takes them.
        if lm & lm_bit(ids::DEFILED_SHRINE) != 0 {
            if action && self.emp.piles.take_one_vp(pile) {
                self.emp.lm_vp[lm_index(ids::DEFILED_SHRINE)] += 1;
                sink.event(Event::PileVp { pile: ids::DEFILED_SHRINE, n: 1 });
            }
            if c == id::CURSE && buy_phase {
                let n = std::mem::take(&mut self.emp.lm_vp[lm_index(ids::DEFILED_SHRINE)]);
                self.gain_vp_tokens(p, ids::DEFILED_SHRINE, n as u16, sink);
            }
        }
        // A Nomads gained in the Buy phase gives its +$2 first: the player orders the two triggers, and this order is never worse for them.
        let nomads_coins = (c == crate::sets::hinterlands::ids::NOMADS) as u16 * 2;
        if lm & lm_bit(ids::BASILICA) != 0 && buy_phase && self.turn.coins + nomads_coins >= 2 {
            self.take_landmark_vp(p, ids::BASILICA, 2, sink);
        }
        if lm & lm_bit(ids::BATTLEFIELD) != 0 && victory {
            self.take_landmark_vp(p, ids::BATTLEFIELD, 2, sink);
        }
        if lm & lm_bit(ids::COLONNADE) != 0 && buy_phase && action && self.players[p as usize].in_play.has(c) {
            self.take_landmark_vp(p, ids::COLONNADE, 2, sink);
        }
        if lm & lm_bit(ids::LABYRINTH) != 0 && own_turn && self.turn.emp.gains == 2 {
            self.take_landmark_vp(p, ids::LABYRINTH, 2, sink);
        }
        // Mountain Pass: the first Province gained starts the bidding, from the player to the left of the gainer, ending with the gainer.
        if lm & lm_bit(ids::MOUNTAIN_PASS) != 0 && c == id::PROVINCE && !self.emp.pass_done {
            self.emp.pass_done = true;
            self.emp.bid_high = 0;
            self.emp.bid_who = u8::MAX;
            self.stack.push(Frame { then: Then::MountainPass, ..Frame::new(K::Bid, p, ids::MOUNTAIN_PASS) });
            let n = self.num_players;
            for i in (0..n).rev() {
                let bidder = (p + 1 + i) % n;
                self.stack.push(Frame::new(K::Bid, bidder, ids::MOUNTAIN_PASS));
            }
        }
    }

    /// The legal bids of Mountain Pass: none, or any amount above the highest so far.
    pub(crate) fn bid_choices(&self, out: &mut ChoiceBuf) {
        out.push(Choice::Bid(0));
        for n in self.emp.bid_high + 1..=MAX_BID {
            out.push(Choice::Bid(n));
        }
    }

    /// Mountain Pass's bidding frame: a bid, or after the last bid the resolution.
    pub(crate) fn run_bid<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        if f.then == Then::MountainPass {
            self.stack.pop();
            let (who, high) = (self.emp.bid_who, self.emp.bid_high);
            if who != u8::MAX && high > 0 {
                // The high bidder takes 8 VP tokens and the Debt they bid.
                self.gain_vp_tokens(who, ids::MOUNTAIN_PASS, 8, sink);
                self.take_debt(who, ids::MOUNTAIN_PASS, high as u16, sink);
            }
            return Run::Continue;
        }
        if self.emp.bid_high >= MAX_BID {
            // Nothing can beat a bid of 40.
            self.stack.pop();
            return Run::Continue;
        }
        Run::Decide(DecisionKind::Bid { min: self.emp.bid_high + 1 }, 0)
    }

    /// A bid is made (0 = pass).
    pub(crate) fn apply_bid(&mut self, f: Frame, n: u8) {
        self.stack.pop();
        if n > 0 {
            self.emp.bid_high = n;
            self.emp.bid_who = f.player;
        }
    }
}

// -------------------------------------------------------------------------------------------
// Events
// -------------------------------------------------------------------------------------------

/// The Tax token: the "token" a Tax frame (`K::Token`) moves, in `cards::TOKEN_NAMES` numbering (it puts 2 Debt on a pile).
pub const TOK_TAX: usize = 10;

/// Whether the Empires Event `e` is in the game and does something now (the common checks, `adventures::event_available`, are made first).
pub(crate) fn event_available(g: &GameState, e: CardId) -> bool {
    let p = g.turn.player as usize;
    let supply = |c: CardId| g.supply.get(c) > 0;
    match e {
        ids::ADVANCE => g.players[p].hand.any_type(ACT),
        ids::DOMINATE => supply(id::PROVINCE),
        ids::DONATE => g.players[p].emp & P_DONATE == 0,
        ids::RITUAL => supply(id::CURSE) && !g.players[p].hand.is_empty(),
        // (Salt the Earth's +1 VP comes even with no Victory card to trash.)
        ids::TRIUMPH => supply(id::ESTATE),
        ids::WINDFALL => g.players[p].deck_size() == 0 && g.players[p].discard.is_empty() && supply(id::GOLD),
        _ => true,
    }
}

/// What buying the Empires Event `e` does, after its cost and Buy are paid.
pub(crate) fn event_effect<S: EventSink>(g: &mut GameState, e: CardId, p: u8, sink: &mut S) {
    use Act::*;
    let pi = p as usize;
    match e {
        ids::ADVANCE => g.stack.push(select(
            p, e, Zone::Hand, Trash, Filter::Action, 0, 1,
            Then::GainFixedUpTo { max_cost: 6, filter: Filter::Action, dest: Dest::Discard },
        )),
        ids::ANNEX => {
            // Shuffle all but up to 5 chosen cards of the discard pile into the deck, then gain a Duchy (it stays in the discard pile).
            g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::DUCHY), Dest::Discard));
            g.stack.push(select(p, e, Zone::Discard, SetAside, Filter::Any, 0, 5, Then::AnnexShuffle));
        }
        ids::BANQUET => {
            g.stack.push(gain_frame(p, e, 5, Filter::NonVictory, Dest::Discard));
            g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::COPPER), Dest::Discard));
            g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::COPPER), Dest::Discard));
        }
        ids::CONQUEST => {
            g.stack.push(select(p, e, Zone::Hand, Discard, Filter::Any, 0, 0, Then::ConquestVp));
            g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::SILVER), Dest::Discard));
            g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::SILVER), Dest::Discard));
        }
        ids::DELVE => {
            g.turn.buys += 1;
            sink.bonus(p, e, 0, 1, 0, 0);
            g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::SILVER), Dest::Discard));
        }
        ids::DOMINATE => g.stack.push(Frame { then: Then::DominateVp, ..gain_frame(p, e, u8::MAX, Filter::Card(id::PROVINCE), Dest::Discard) }),
        ids::DONATE => g.players[pi].emp |= P_DONATE,
        ids::RITUAL => g.stack.push(Frame { then: Then::RitualTrash, ..gain_frame(p, e, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) }),
        ids::SALT_THE_EARTH => {
            g.gain_vp_tokens(p, e, 1, sink);
            g.stack.push(select(p, e, Zone::Supply, Trash, Filter::Victory, 1, 1, Then::Nothing));
        }
        ids::TAX => g.push_token_frame(p, e, TOK_TAX, false),
        ids::TRIUMPH => g.stack.push(Frame { then: Then::TriumphVp, ..gain_frame(p, e, u8::MAX, Filter::Card(id::ESTATE), Dest::Discard) }),
        ids::WEDDING => {
            g.gain_vp_tokens(p, e, 1, sink);
            g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::GOLD), Dest::Discard));
        }
        ids::WINDFALL => {
            if g.players[pi].deck_size() == 0 && g.players[pi].discard.is_empty() {
                for _ in 0..3 {
                    g.stack.push(gain_frame(p, e, u8::MAX, Filter::Card(id::GOLD), Dest::Discard));
                }
            }
        }
        _ => {}
    }
}

// -------------------------------------------------------------------------------------------
// Frames
// -------------------------------------------------------------------------------------------

impl GameState {
    /// A `Select` of Empires finished picking (`f.count` picks, the last `f.last`).
    #[inline(never)]
    pub(crate) fn empires_finish_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            Then::ChangeGain => {
                // Trashed a card: gain one costing more $, taking Debt for the difference.
                if f.count > 0 {
                    self.stack.push(Frame {
                        gf: crate::state::GF_MORE, then: Then::ChangeDebt, depth: f.depth,
                        ..gain_frame(p, f.source, self.cost(f.last), Filter::Any, Dest::Discard)
                    });
                }
            }
            Then::EncampmentReveal => {
                if f.count == 0 && self.leave_play_self(p, f.source, f.tok) {
                    // Set aside, and back on top of its pile at the start of Clean-up (here at once: nothing sees it in between).
                    self.supply.add(f.source, 1);
                    sink.event(Event::Return { player: p, card: f.source });
                }
            }
            Then::CatapultHit => {
                if f.count > 0 {
                    let c = f.last;
                    let (vs, n) = self.victims(sink);
                    // The Curses are gained first, in turn order, then the hands are cut down (the stack runs the last frame pushed first).
                    if self.is_treasure(c) {
                        for &v in vs[..n].iter().rev() {
                            self.stack.push(Frame {
                                depth: f.depth, down_to_target: true,
                                ..select(v, f.source, Zone::Hand, Act::Discard, Filter::Any, 0, 3, Then::Nothing)
                            });
                        }
                    }
                    if self.cost(c) >= 3 {
                        for &v in vs[..n].iter().rev() {
                            self.stack.push(Frame { depth: f.depth, ..gain_frame(v, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
                        }
                    }
                }
            }
            Then::GladiatorReveal => {
                let left = (p + 1) % self.num_players;
                if f.count > 0 {
                    // The player to the left may reveal a copy; if they do not (or cannot), the Gladiator's player gets +$1 and trashes a Gladiator.
                    self.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Reveal, subject: f.last, then: Then::GladiatorMatch, depth: f.depth,
                        chooser: left, ..Frame::new(K::YesNo, left, f.source)
                    });
                } else {
                    self.gladiator_wins(p, f.source, sink);
                }
            }
            Then::LegionaryHit => {
                if f.count > 0 {
                    let (vs, n) = self.victims(sink);
                    for &v in vs[..n].iter().rev() {
                        self.stack.push(Frame { depth: f.depth, ..draw_frame(v, f.source, 1) });
                        self.stack.push(Frame {
                            depth: f.depth, down_to_target: true,
                            ..select(v, f.source, Zone::Hand, Act::Discard, Filter::Any, 0, 2, Then::Nothing)
                        });
                    }
                }
            }
            Then::SacrificeBonus => {
                if f.count > 0 {
                    let c = f.last;
                    if cards::is(c, ACT) {
                        self.turn.actions += 2;
                        sink.bonus(p, f.source, 2, 0, 0, 0);
                        self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 2) });
                    }
                    if self.is_treasure(c) {
                        self.turn.coins += 2;
                        sink.bonus(p, f.source, 0, 0, 2, 0);
                    }
                    if cards::is(c, VIC) {
                        self.gain_vp_tokens(p, f.source, 2, sink);
                    }
                }
            }
            Then::AddPileVp => {
                // Added even when no card could be trashed (empty hand).
                if self.in_supply(f.source) {
                    self.emp.piles.add_vp(f.source, 1);
                    sink.event(Event::PileVp { pile: f.source, n: 1 });
                }
            }
            Then::AnnexShuffle => {
                // The cards picked (set aside) stay in the discard pile; the rest of it is shuffled into the deck, whose known top cards are
                // shuffled too.
                let ps = &mut self.players[pi];
                let kept = ps.set_aside;
                ps.set_aside.clear();
                let discard = ps.discard;
                ps.deck_unknown.add_all(&discard);
                for i in 0..ps.deck_known.len as usize {
                    let c = ps.deck_known.cards[i];
                    ps.deck_unknown.add(c, 1);
                }
                for i in 0..ps.deck_known_bottom.len as usize {
                    let c = ps.deck_known_bottom.cards[i];
                    ps.deck_unknown.add(c, 1);
                }
                ps.deck_known.clear();
                ps.deck_known_bottom.clear();
                ps.discard = kept;
                self.rs_shuffled(p);
                sink.event(Event::Shuffle { player: p });
            }
            Then::ConquestVp => {
                let n = self.turn.emp.silvers as u16;
                self.gain_vp_tokens(p, f.source, n, sink);
            }
            Then::ArenaVp => {
                if f.count > 0 {
                    self.take_landmark_vp(p, ids::ARENA, 2, sink);
                }
            }
            Then::DonateShuffle => self.donate_shuffle(p, sink),
            _ => {}
        }
    }

    /// Gladiator: nobody matched the revealed card: +$1 and a Gladiator is trashed from its pile (if the top of the pile is one).
    pub(crate) fn gladiator_wins<S: EventSink>(&mut self, p: u8, source: CardId, sink: &mut S) {
        self.turn.coins += 1;
        sink.bonus(p, source, 0, 0, 1, 0);
        if self.supply.get(ids::GLADIATOR) > 0 {
            self.supply.remove(ids::GLADIATOR);
            self.trash_card(p, ids::GLADIATOR, false, sink);
            self.flush_trash_triggers(0, sink);
        }
    }

    /// A Gain of Empires finished with the card `c` gained.
    #[inline(never)]
    pub(crate) fn empires_gained<S: EventSink>(&mut self, f: &Frame, c: CardId, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::ChangeDebt => {
                let d = self.cost(c).saturating_sub(f.max) as u16;
                if d > 0 {
                    self.take_debt(p, f.source, d, sink);
                }
            }
            Then::DominateVp => self.gain_vp_tokens(p, f.source, 9, sink),
            Then::TriumphVp => {
                let n = self.turn.emp.gains.max(1) as u16;
                self.gain_vp_tokens(p, f.source, n, sink);
            }
            Then::RitualTrash => {
                self.stack.push(Frame {
                    depth: f.depth,
                    ..select(p, f.source, Zone::Hand, Act::Trash, Filter::Any, 1, 1, Then::VpPerCost { per: 1 })
                });
            }
            Then::TakePileVp => {
                let n = self.emp.piles.take_vp(cards::pile_of(f.source));
                self.gain_vp_tokens(p, f.source, n as u16, sink);
            }
            _ => {}
        }
    }

    /// A YesNo of Empires was answered Yes.
    pub(crate) fn empires_yes<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::EngineerGain => {
                self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, 4, Filter::Any, Dest::Discard) });
            }
            Then::SmallCastle => {
                self.stack.push(Frame {
                    depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Types(TypeSet::Castle), Dest::Discard)
                });
            }
            _ => {}
        }
        let _ = sink;
    }

    /// A YesNo of Empires was answered No (or had nothing to act on).
    pub(crate) fn empires_decline<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        match f.then {
            // Gladiator: the left player did not (could not) reveal a copy.
            Then::GladiatorMatch => {
                let attacker = self.turn.player;
                self.gladiator_wins(attacker, ids::GLADIATOR, sink);
            }
            // Small Castle not trashed: a Castle from the hand instead.
            Then::SmallCastle => self.small_castle_hand_frame(f),
            _ => {}
        }
    }

    fn small_castle_hand_frame(&mut self, f: &Frame) {
        self.stack.push(Frame {
            depth: f.depth,
            ..select(
                f.player, f.source, Zone::Hand, Act::Trash, Filter::Types(TypeSet::Castle), 0, 1,
                Then::GainFixedUpTo { max_cost: u8::MAX, filter: Filter::Types(TypeSet::Castle), dest: Dest::Discard },
            )
        });
    }

    /// RevealTop frames of Empires finishing: Patrician (a card costing $5 or more goes to the hand, else back on the deck) and Archive (the
    /// cards revealed are set aside under it).
    #[inline(never)]
    pub(crate) fn empires_reveal_done<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            Then::PatricianCheck => {
                let Some((c, _)) = self.players[pi].set_aside.iter().next() else { return };
                self.players[pi].set_aside.remove(c);
                if self.cost(c) >= 5 {
                    self.players[pi].hand.add(c, 1);
                    sink.event(Event::Draw { player: p, card: c });
                } else {
                    self.players[pi].deck_known.push_top(c);
                    sink.event(Event::Topdeck { player: p, card: c });
                }
            }
            Then::ArchiveSet => {
                let ps = &mut self.players[pi];
                let set = ps.set_aside;
                ps.set_aside.clear();
                let n = set.total();
                for (c, k) in set.iter() {
                    ps.archive.add(c, k);
                    for _ in 0..k {
                        sink.event(Event::SetAside { player: p, card: c });
                    }
                }
                if n > 0 {
                    // One card into the hand now (a choice), the others on the next turns.
                    self.stack.push(Frame { depth: f.depth, ..select(p, f.source, Zone::Archive, Act::ToHand, Filter::Any, 1, 1, Then::Nothing) });
                    if n > 1 {
                        self.push_duration_pending(p, f.source, 1, 0);
                    }
                }
            }
            _ => {}
        }
    }

    /// Chariot Race: draw the top card (revealing it), then reveal the top card of the left player's deck and put it back; if the card drawn
    /// costs more, +$1 and a VP token.
    #[inline(never)]
    pub(crate) fn run_chariot<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        let pi = p as usize;
        if f.count == 0 {
            if self.players[pi].adv & crate::state::ADV_MINUS_CARD != 0 {
                // The -1 Card token: no card is drawn, so there is nothing to compare.
                self.players[pi].adv &= !crate::state::ADV_MINUS_CARD;
                self.stack.pop();
                return Run::Continue;
            }
            match take_top!(self, p, sink) {
                None => {
                    self.stack.pop();
                }
                Some(c) => {
                    self.players[pi].hand.add(c, 1);
                    sink.event(Event::Draw { player: p, card: c });
                    self.reveal_card(p, c, sink);
                    f.count = 1;
                    f.subject = c;
                    self.stack.set_top(f);
                }
            }
            return Run::Continue;
        }
        let left = (p + 1) % self.num_players;
        match take_top!(self, left, sink) {
            None => {
                self.stack.pop();
            }
            Some(c2) => {
                self.stack.pop();
                self.cover_deck(left);
                self.players[left as usize].deck_known.push_top(c2);
                self.reveal_card(left, c2, sink);
                if self.costs_more(f.subject, c2) {
                    self.turn.coins += 1;
                    self.gain_vp_tokens(p, f.source, 1, sink);
                    sink.bonus(p, f.source, 0, 0, 1, 0);
                }
            }
        }
        Run::Continue
    }

    /// Whether card `a` costs more than `b`: no part of its cost (the $, the Potion, the Debt) is less, and one is more.
    pub fn costs_more(&self, a: CardId, b: CardId) -> bool {
        let part = |c: CardId| (self.cost(c), cards::potion_cost(c) as u8, cards::debt_cost(c));
        let (x, y) = (part(a), part(b));
        x.0 >= y.0 && x.1 >= y.1 && x.2 >= y.2 && x != y
    }

    /// Donate, at the start of its player's turn: the deck and discard pile go into the hand, any number of cards of it are trashed, the rest
    /// is shuffled into the deck and 5 cards are drawn. Pushed over the other start-of-turn effects, so it comes first.
    #[inline(never)]
    pub(crate) fn start_donate<S: EventSink>(&mut self, p: u8, _sink: &mut S) {
        let ps = &mut self.players[p as usize];
        ps.emp &= !P_DONATE;
        // Everything into the hand (the known top cards of the deck too).
        let deck = ps.deck_counts();
        ps.deck_known.clear();
        ps.deck_known_bottom.clear();
        ps.deck_unknown.clear();
        let discard = ps.discard;
        ps.discard.clear();
        ps.hand.add_all(&deck);
        ps.hand.add_all(&discard);
        // Pushed last-first: trash any number, then the hand is shuffled into the deck and 5 cards are drawn.
        self.stack.push(Frame { max: 5, ..Frame::new(K::Draw, p, ids::DONATE) });
        self.stack.push(select(p, ids::DONATE, Zone::Hand, Act::Discard, Filter::Any, 0, 0, Then::DonateShuffle));
        self.stack.push(select(p, ids::DONATE, Zone::Hand, Act::Trash, Filter::Any, 0, ALL, Then::Nothing));
    }

    /// Donate, after the trashing: the rest of the hand is shuffled into the deck (the first 0-pick frame under the trash).
    pub(crate) fn donate_shuffle<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        let hand = ps.hand;
        ps.hand.clear();
        ps.deck_unknown.add_all(&hand);
        self.rs_shuffled(p);
        sink.event(Event::Shuffle { player: p });
    }
}

// -------------------------------------------------------------------------------------------
// Scoring
// -------------------------------------------------------------------------------------------

impl GameState {
    /// What the Landmarks add to (or take from) `p`'s score at the end of the game, from what everyone owns and the Supply.
    pub fn landmark_score(&self, p: u8) -> i32 {
        let lm = self.emp.landmarks;
        if lm == 0 {
            return 0;
        }
        let mine = self.players[p as usize].all_cards();
        let has = |l: CardId| lm & lm_bit(l) != 0;
        let mut s = 0i32;
        if has(ids::BANDIT_FORT) {
            s -= 2 * (mine.get(id::SILVER) as i32 + mine.get(id::GOLD) as i32);
        }
        if has(ids::FOUNTAIN) && mine.get(id::COPPER) >= 10 {
            s += 15;
        }
        if has(ids::KEEP) {
            // 5 VP per differently named Treasure the player has at least as many copies of as every other player.
            for (c, n) in mine.iter() {
                if cards::is(c, TRE) || (c == id::CURSE && self.in_supply(id::CHARLATAN)) {
                    let most = (0..self.num_players).filter(|&q| q != p).all(|q| self.players[q as usize].all_cards().get(c) <= n);
                    if most {
                        s += 5;
                    }
                }
            }
        }
        if has(ids::MUSEUM) {
            s += 2 * mine.iter().count() as i32;
        }
        if has(ids::OBELISK) && self.emp.obelisk != 0 {
            let pile = self.emp.obelisk;
            let cards_of_pile: i32 = mine.iter().filter(|&(c, _)| cards::pile_of(c) == pile).map(|(_, n)| n as i32).sum();
            s += 2 * cards_of_pile;
        }
        if has(ids::ORCHARD) {
            s += 4 * mine.iter().filter(|&(c, n)| cards::is(c, ACT) && n >= 3).count() as i32;
        }
        if has(ids::PALACE) {
            s += 3 * mine.get(id::COPPER).min(mine.get(id::SILVER)).min(mine.get(id::GOLD)) as i32;
        }
        if has(ids::TOWER) {
            // 1 VP per non-Victory card from an empty Supply pile (Curses count; a split pile with a half left is not empty).
            let empty: Vec<CardId> = self.supply_cards().filter(|&c| cards::pile_of(c) == c && self.pile_is_empty(c)).collect();
            s += mine.iter().filter(|&(c, _)| !cards::is(c, VIC) && empty.contains(&cards::pile_of(c))).map(|(_, n)| n as i32).sum::<i32>();
        }
        if has(ids::TRIUMPHAL_ARCH) {
            // 3 VP per copy of the second most common Action (a tie counts either).
            let (mut a, mut b) = (0i32, 0i32);
            for (c, n) in mine.iter() {
                if cards::is(c, ACT) {
                    let n = n as i32;
                    if n > a {
                        b = a;
                        a = n;
                    } else if n > b {
                        b = n;
                    }
                }
            }
            s += 3 * b;
        }
        if has(ids::WALL) {
            s -= (mine.total() as i32 - 15).max(0);
        }
        if has(ids::WOLF_DEN) {
            s -= 3 * mine.iter().filter(|&(_, n)| n == 1).count() as i32;
        }
        s
    }

    /// Whether the Supply pile of `c` (named as `cards::pile_of` names it) has no cards left: for a split pile, both halves are gone.
    pub fn pile_is_empty(&self, c: CardId) -> bool {
        if self.supply.get(c) > 0 {
            return false;
        }
        match cards::split_bottom(c) {
            Some(b) if self.in_supply(b) => self.supply.get(b) == 0,
            _ => true,
        }
    }
}
