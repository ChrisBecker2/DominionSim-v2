//! Nocturne: the Night phase, Boons and Hexes (decks of pseudo-cards drawn at random), States, Heirlooms that replace a starting Copper,
//! the non-Supply Spirits, Wish, Bat and Zombies, and the 33 kingdom cards. See `docs/nocturne-plan.md` for the rulings and the design.

use super::prelude::*;
use crate::cards::{noc, DOOM, FATE, HEIRLOOM, NIGHT, SPIRIT, ZOMBIE};

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but the ids index the card table).
/// 280-312 are the kingdom cards, 313-319 the Heirlooms, 320-327 the other cards outside the Supply (the Ruins and Knights come after them
/// and are the highest zone ids). 398-409 are the Boons, 410-421 the Hexes and 422-426 the States: pseudo-cards above every id that can sit
/// in a zone, like the Events and Landmarks.
pub mod ids {
    use crate::cards::CardId;
    pub const DRUID: CardId = 280;
    pub const FAITHFUL_HOUND: CardId = 281;
    pub const GUARDIAN: CardId = 282;
    pub const MONASTERY: CardId = 283;
    pub const PIXIE: CardId = 284;
    pub const TRACKER: CardId = 285;
    pub const CHANGELING: CardId = 286;
    pub const FOOL: CardId = 287;
    pub const GHOST_TOWN: CardId = 288;
    pub const LEPRECHAUN: CardId = 289;
    pub const NIGHT_WATCHMAN: CardId = 290;
    pub const SECRET_CAVE: CardId = 291;
    pub const BARD: CardId = 292;
    pub const BLESSED_VILLAGE: CardId = 293;
    pub const CEMETERY: CardId = 294;
    pub const CONCLAVE: CardId = 295;
    pub const DEVILS_WORKSHOP: CardId = 296;
    pub const EXORCIST: CardId = 297;
    pub const NECROMANCER: CardId = 298;
    pub const SHEPHERD: CardId = 299;
    pub const SKULK: CardId = 300;
    pub const COBBLER: CardId = 301;
    pub const CRYPT: CardId = 302;
    pub const CURSED_VILLAGE: CardId = 303;
    pub const DEN_OF_SIN: CardId = 304;
    pub const IDOL: CardId = 305;
    pub const POOKA: CardId = 306;
    pub const SACRED_GROVE: CardId = 307;
    pub const TORMENTOR: CardId = 308;
    pub const TRAGIC_HERO: CardId = 309;
    pub const VAMPIRE: CardId = 310;
    pub const WEREWOLF: CardId = 311;
    pub const RAIDER: CardId = 312;
    // The Heirlooms: no pile, only in the starting decks.
    pub const HAUNTED_MIRROR: CardId = 313;
    pub const MAGIC_LAMP: CardId = 314;
    pub const GOAT: CardId = 315;
    pub const PASTURE: CardId = 316;
    pub const POUCH: CardId = 317;
    pub const CURSED_GOLD: CardId = 318;
    pub const LUCKY_COIN: CardId = 319;
    // Piles outside the Supply.
    pub const WILL_O_WISP: CardId = 320;
    pub const WISH: CardId = 321;
    pub const BAT: CardId = 322;
    pub const IMP: CardId = 323;
    pub const GHOST: CardId = 324;
    pub const ZOMBIE_APPRENTICE: CardId = 325;
    pub const ZOMBIE_MASON: CardId = 326;
    pub const ZOMBIE_SPY: CardId = 327;
    // The Boons (pseudo-cards).
    pub const THE_EARTHS_GIFT: CardId = 658;
    pub const THE_FIELDS_GIFT: CardId = 659;
    pub const THE_FLAMES_GIFT: CardId = 660;
    pub const THE_FORESTS_GIFT: CardId = 661;
    pub const THE_MOONS_GIFT: CardId = 662;
    pub const THE_MOUNTAINS_GIFT: CardId = 663;
    pub const THE_RIVERS_GIFT: CardId = 664;
    pub const THE_SEAS_GIFT: CardId = 665;
    pub const THE_SKYS_GIFT: CardId = 666;
    pub const THE_SUNS_GIFT: CardId = 667;
    pub const THE_SWAMPS_GIFT: CardId = 668;
    pub const THE_WINDS_GIFT: CardId = 669;
    // The Hexes.
    pub const BAD_OMENS: CardId = 670;
    pub const DELUSION: CardId = 671;
    pub const ENVY: CardId = 672;
    pub const FAMINE: CardId = 673;
    pub const FEAR: CardId = 674;
    pub const GREED: CardId = 675;
    pub const HAUNTING: CardId = 676;
    pub const LOCUSTS: CardId = 677;
    pub const MISERY: CardId = 678;
    pub const PLAGUE: CardId = 679;
    pub const POVERTY: CardId = 680;
    pub const WAR: CardId = 681;
    // The States.
    pub const LOST_IN_THE_WOODS: CardId = 682;
    pub const DELUDED: CardId = 683;
    pub const ENVIOUS: CardId = 684;
    pub const MISERABLE: CardId = 685;
    pub const TWICE_MISERABLE: CardId = 686;
}

/// A Boon, Hex or State: no types, no bonuses (what it does is `boon_effect` / `hex_effect` / the state flags); a pseudo-card that is never in a zone.
const fn pseudo(name: &'static str) -> CardDef {
    noc(c(name, 0, 0, 0, 0, 0, 0, 0))
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::DRUID, noc(order_sensitive(c("Druid", 2, ACTION | FATE, 0, 0, 0, 0, 1)))), // receives one of the three set-aside Boons
    (ids::FAITHFUL_HOUND, noc(choice_free(c("Faithful Hound", 2, ACTION | REACTION, 0, 0, 2, 0, 0)))), // its reaction is to a discard
    (ids::GUARDIAN, noc(choice_free(c("Guardian", 2, NIGHT | DURATION, 0, 0, 0, 0, 0)))), // gained to the hand
    (ids::MONASTERY, noc(has_choice(c("Monastery", 2, NIGHT, 0, 0, 0, 0, 0)))), // trashes a card per card gained this turn
    (ids::PIXIE, noc(order_sensitive(c("Pixie", 2, ACTION | FATE, 0, 0, 1, 1, 0)))), // may trash itself for a Boon twice
    (ids::TRACKER, noc(order_sensitive(c("Tracker", 2, ACTION | FATE, 1, 0, 0, 0, 0)))), // a Boon, and the cards gained this turn may go onto the deck
    (ids::CHANGELING, noc(has_choice(c("Changeling", 3, NIGHT, 0, 0, 0, 0, 0)))), // trashes itself, gains a copy of a card in play
    (ids::FOOL, noc(order_sensitive(c("Fool", 3, ACTION | FATE, 0, 0, 0, 0, 0)))), // three Boons in any order
    (ids::GHOST_TOWN, noc(choice_free(c("Ghost Town", 3, NIGHT | DURATION, 0, 0, 0, 0, 0)))), // gained to the hand
    (ids::LEPRECHAUN, noc(order_sensitive(c("Leprechaun", 3, ACTION | DOOM, 0, 0, 0, 0, 0)))), // counts the cards in play
    (ids::NIGHT_WATCHMAN, noc(has_choice(c("Night Watchman", 3, NIGHT, 0, 0, 0, 0, 0)))), // looks at the top of the deck; gained to the hand
    (ids::SECRET_CAVE, noc(order_sensitive(c("Secret Cave", 3, ACTION | DURATION, 0, 0, 1, 1, 0)))), // may discard 3 cards
    (ids::BARD, noc(order_sensitive(c("Bard", 4, ACTION | FATE, 2, 0, 0, 0, 0)))),
    (ids::BLESSED_VILLAGE, noc(choice_free(c("Blessed Village", 4, ACTION | FATE, 0, 0, 1, 2, 0)))), // a Boon when it is gained
    (ids::CEMETERY, noc(c("Cemetery", 4, VICTORY, 0, 2, 0, 0, 0))), // trashes up to 4 cards when gained
    (ids::CONCLAVE, noc(order_sensitive(c("Conclave", 4, ACTION, 2, 0, 0, 0, 0)))), // may play an Action from the hand
    (ids::DEVILS_WORKSHOP, noc(has_choice(c("Devil's Workshop", 4, NIGHT, 0, 0, 0, 0, 0)))),
    (ids::EXORCIST, noc(has_choice(c("Exorcist", 4, NIGHT, 0, 0, 0, 0, 0)))),
    (ids::NECROMANCER, noc(order_sensitive(c("Necromancer", 4, ACTION, 0, 0, 0, 0, 0)))), // plays an Action from the trash
    (ids::SHEPHERD, noc(order_sensitive(c("Shepherd", 4, ACTION, 0, 0, 0, 1, 0)))), // discards Victory cards of the hand
    (ids::SKULK, noc(choice_free(c("Skulk", 4, ACTION | ATTACK | DOOM, 0, 0, 0, 0, 1)))), // a Gold when gained
    (ids::COBBLER, noc(choice_free(c("Cobbler", 5, NIGHT | DURATION, 0, 0, 0, 0, 0)))),
    (ids::CRYPT, noc(has_choice(c("Crypt", 5, NIGHT | DURATION, 0, 0, 0, 0, 0)))), // which Treasures to set aside
    (ids::CURSED_VILLAGE, noc(order_sensitive(c("Cursed Village", 5, ACTION | DOOM, 0, 0, 0, 2, 0)))), // draws to 6 cards; a Hex when gained
    (ids::DEN_OF_SIN, noc(choice_free(c("Den of Sin", 5, NIGHT | DURATION, 0, 0, 0, 0, 0)))), // gained to the hand
    (ids::IDOL, noc(has_choice(c("Idol", 5, TREASURE | ATTACK | FATE, 2, 0, 0, 0, 0)))), // a Boon or Curses, by the Idols in play
    (ids::POOKA, noc(order_sensitive(c("Pooka", 5, ACTION, 0, 0, 0, 0, 0)))), // may trash a Treasure from the hand
    (ids::SACRED_GROVE, noc(order_sensitive(c("Sacred Grove", 5, ACTION | FATE, 3, 0, 0, 0, 1)))),
    (ids::TORMENTOR, noc(order_sensitive(c("Tormentor", 5, ACTION | ATTACK | DOOM, 2, 0, 0, 0, 0)))), // counts the other cards in play
    (ids::TRAGIC_HERO, noc(order_sensitive(c("Tragic Hero", 5, ACTION, 0, 0, 3, 0, 1)))), // counts the cards in hand after drawing
    (ids::VAMPIRE, noc(has_choice(c("Vampire", 5, NIGHT | ATTACK | DOOM, 0, 0, 0, 0, 0)))),
    (ids::WEREWOLF, noc(choice_free(c("Werewolf", 5, ACTION | NIGHT | ATTACK | DOOM, 0, 0, 0, 0, 0)))), // +3 Cards by day, a Hex at Night
    (ids::RAIDER, noc(choice_free(c("Raider", 6, NIGHT | DURATION | ATTACK, 0, 0, 0, 0, 0)))),
    // The Heirlooms.
    (ids::HAUNTED_MIRROR, noc(on_trash(choice_free(c("Haunted Mirror", 0, TREASURE | HEIRLOOM, 1, 0, 0, 0, 0))))),
    (ids::MAGIC_LAMP, noc(has_choice(c("Magic Lamp", 0, TREASURE | HEIRLOOM, 1, 0, 0, 0, 0)))),
    (ids::GOAT, noc(has_choice(c("Goat", 2, TREASURE | HEIRLOOM, 1, 0, 0, 0, 0)))),
    (ids::PASTURE, noc(choice_free(c("Pasture", 2, TREASURE | VICTORY | HEIRLOOM, 1, 0, 0, 0, 0)))), // 1 VP per Estate (`vp`)
    (ids::POUCH, noc(choice_free(c("Pouch", 2, TREASURE | HEIRLOOM, 1, 0, 0, 0, 1)))),
    (ids::CURSED_GOLD, noc(has_choice(c("Cursed Gold", 4, TREASURE | HEIRLOOM, 3, 0, 0, 0, 0)))),
    (ids::LUCKY_COIN, noc(has_choice(c("Lucky Coin", 4, TREASURE | HEIRLOOM, 1, 0, 0, 0, 0)))),
    // Piles outside the Supply.
    (ids::WILL_O_WISP, noc(order_sensitive(c("Will-o'-Wisp", 0, ACTION | SPIRIT, 0, 0, 1, 1, 0)))), // reveals the top card
    (ids::WISH, noc(has_choice(c("Wish", 0, ACTION, 0, 0, 0, 1, 0)))),
    (ids::BAT, noc(has_choice(c("Bat", 2, NIGHT, 0, 0, 0, 0, 0)))),
    (ids::IMP, noc(order_sensitive(c("Imp", 2, ACTION | SPIRIT, 0, 0, 2, 0, 0)))), // may play an Action from the hand
    (ids::GHOST, noc(has_choice(c("Ghost", 4, NIGHT | DURATION | SPIRIT, 0, 0, 0, 0, 0)))), // reveals cards from the deck until an Action
    (ids::ZOMBIE_APPRENTICE, noc(order_sensitive(c("Zombie Apprentice", 3, ACTION | ZOMBIE, 0, 0, 0, 0, 0)))),
    (ids::ZOMBIE_MASON, noc(order_sensitive(c("Zombie Mason", 3, ACTION | ZOMBIE, 0, 0, 0, 0, 0)))),
    (ids::ZOMBIE_SPY, noc(order_sensitive(c("Zombie Spy", 3, ACTION | ZOMBIE, 0, 0, 1, 1, 0)))),
    // The Boons.
    (ids::THE_EARTHS_GIFT, pseudo("The Earth's Gift")),
    (ids::THE_FIELDS_GIFT, pseudo("The Field's Gift")),
    (ids::THE_FLAMES_GIFT, pseudo("The Flame's Gift")),
    (ids::THE_FORESTS_GIFT, pseudo("The Forest's Gift")),
    (ids::THE_MOONS_GIFT, pseudo("The Moon's Gift")),
    (ids::THE_MOUNTAINS_GIFT, pseudo("The Mountain's Gift")),
    (ids::THE_RIVERS_GIFT, pseudo("The River's Gift")),
    (ids::THE_SEAS_GIFT, pseudo("The Sea's Gift")),
    (ids::THE_SKYS_GIFT, pseudo("The Sky's Gift")),
    (ids::THE_SUNS_GIFT, pseudo("The Sun's Gift")),
    (ids::THE_SWAMPS_GIFT, pseudo("The Swamp's Gift")),
    (ids::THE_WINDS_GIFT, pseudo("The Wind's Gift")),
    // The Hexes.
    (ids::BAD_OMENS, pseudo("Bad Omens")),
    (ids::DELUSION, pseudo("Delusion")),
    (ids::ENVY, pseudo("Envy")),
    (ids::FAMINE, pseudo("Famine")),
    (ids::FEAR, pseudo("Fear")),
    (ids::GREED, pseudo("Greed")),
    (ids::HAUNTING, pseudo("Haunting")),
    (ids::LOCUSTS, pseudo("Locusts")),
    (ids::MISERY, pseudo("Misery")),
    (ids::PLAGUE, pseudo("Plague")),
    (ids::POVERTY, pseudo("Poverty")),
    (ids::WAR, pseudo("War")),
    // The States.
    (ids::LOST_IN_THE_WOODS, pseudo("Lost in the Woods")),
    (ids::DELUDED, pseudo("Deluded")),
    (ids::ENVIOUS, pseudo("Envious")),
    (ids::MISERABLE, pseudo("Miserable")),
    (ids::TWICE_MISERABLE, pseudo("Twice Miserable")),
];

/// Boons that stay in front of the player until Clean-up ("keep this until Clean-up").
const KEPT_BOONS: u16 = (1 << 1) | (1 << 3) | (1 << 6);

/// The index (0-11) of the Boon or Hex `card` among its twelve.
#[inline]
pub fn boon_index(boon: CardId) -> u16 {
    boon - cards::FIRST_BOON
}

/// The `k`-th (counting from 0) set bit of `mask`.
fn nth_bit(mut mask: u16, mut k: u32) -> u16 {
    while k > 0 {
        mask &= mask - 1;
        k -= 1;
    }
    mask.trailing_zeros() as u16
}

/// Set-up for `cfg`: the Boons (with Druid's three set aside) and Hexes, the piles outside the Supply of the cards in the kingdom, the Zombies in
/// the trash, the Heirlooms replacing a starting Copper in every player's deck. One pass over the kingdom, nothing allocated.
pub(crate) fn setup(g: &mut GameState, cfg: &crate::state::GameConfig, bane: CardId, num_players: usize) {
    let (mut fate, mut doom, mut druid, mut necro) = (false, false, false, false);
    let mut flags = 0u8;
    let mut piles = [0u8; 5];
    let mut heirlooms = [0 as CardId; 7];
    let mut nh = 0;
    for &k in cfg.kingdom.iter().chain(std::iter::once(&bane).filter(|&&b| b != 0)) {
        if !cards::is_kingdom(k) || cards::set_of(k) != CardSet::Nocturne {
            continue;
        }
        let d = cards::def(k);
        if d.types & NIGHT != 0 {
            flags |= crate::state::N_NIGHT;
        }
        fate |= d.types & FATE != 0;
        doom |= d.types & DOOM != 0;
        match k {
            ids::DRUID => druid = true,
            ids::NECROMANCER => necro = true,
            ids::FAITHFUL_HOUND => flags |= crate::state::N_HOUND,
            ids::CEMETERY => {
                flags |= crate::state::N_NIGHT;
                piles[crate::state::NOC_GHOST] = 6;
            }
            ids::EXORCIST => piles[crate::state::NOC_GHOST] = 6,
            ids::DEVILS_WORKSHOP | ids::TORMENTOR => piles[crate::state::NOC_IMP] = 13,
            ids::VAMPIRE => piles[crate::state::NOC_BAT] = 10,
            ids::LEPRECHAUN | ids::SECRET_CAVE => piles[crate::state::NOC_WISH] = 12,
            _ => {}
        }
        if matches!(k, ids::MONASTERY | ids::DEVILS_WORKSHOP | ids::GUARDIAN | ids::GHOST_TOWN | ids::NIGHT_WATCHMAN | ids::DEN_OF_SIN) {
            flags |= crate::state::N_GAIN_HOOK;
        }
        let h = cards::heirloom_of(k);
        if h != 0 && !heirlooms[..nh].contains(&h) {
            heirlooms[nh] = h;
            nh += 1;
        }
    }
    // Exorcist brings all three Spirit piles; Fate cards (the Boons) bring Will-o'-Wisp.
    if cfg.kingdom.contains(&ids::EXORCIST) || bane == ids::EXORCIST {
        piles[crate::state::NOC_WISP] = 12;
        piles[crate::state::NOC_IMP] = 13;
    }
    if fate {
        piles[crate::state::NOC_WISP] = 12;
        flags |= crate::state::N_BOONS;
        let all = (1u16 << cards::NUM_BOONS) - 1;
        g.noc.boon_deck = all;
        if druid {
            // Setup: set aside the top 3 Boons face up.
            for _ in 0..3 {
                let i = nth_bit(g.noc.boon_deck, g.rng.below(g.noc.boon_deck.count_ones()));
                g.noc.boon_deck &= !(1 << i);
                g.noc.boon_druid |= 1 << i;
            }
        }
    }
    if doom {
        flags |= crate::state::N_HEXES;
        g.noc.hex_deck = (1u16 << cards::NUM_HEXES) - 1;
    }
    g.noc.flags = flags;
    g.noc.piles = piles;
    if necro {
        g.trash.add(ids::ZOMBIE_APPRENTICE, 1);
        g.trash.add(ids::ZOMBIE_MASON, 1);
        g.trash.add(ids::ZOMBIE_SPY, 1);
    }
    // Heirlooms replace a starting Copper (the Coppers they replace go back to the Copper pile).
    if nh > 0 {
        for p in 0..num_players {
            let ps = &mut g.players[p];
            ps.deck_unknown.set(id::COPPER, 7 - nh as u8);
            for &h in &heirlooms[..nh] {
                ps.deck_unknown.set(h, 1);
            }
        }
        g.supply.add(id::COPPER, (num_players * nh) as u8);
    }
}

/// Draw again the Druid's Boons of a game that has not started, keeping the others in the deck, with this state's RNG: a set of games that start
/// from one new game (the page's "Simulate from a new game") calls it with each game's own RNG, so every game has its own three Boons, as at a
/// real table, and not the ones `new` drew.
impl GameState {
    pub fn redeal_druid_boons(&mut self) {
        if self.noc.boon_druid == 0 {
            return;
        }
        let all = (1u16 << cards::NUM_BOONS) - 1;
        self.noc.boon_deck = all;
        self.noc.boon_druid = 0;
        for _ in 0..3 {
            let i = nth_bit(self.noc.boon_deck, self.rng.below(self.noc.boon_deck.count_ones()));
            self.noc.boon_deck &= !(1 << i);
            self.noc.boon_druid |= 1 << i;
        }
    }
}

/// The Nocturne cards (see the module docs of `sets`).
pub struct Nocturne;

impl SetRules for Nocturne {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Cards with a "when you gain" effect: Changeling (the exchange), Blessed Village, Cemetery, Skulk and Cursed Village. (Tracker's
    /// topdecking is live for a turn: `gain_live_extra`.)
    const GAIN_TOP: u128 = (1u128 << (ids::CHANGELING - 256))
        | (1u128 << (ids::BLESSED_VILLAGE - 256))
        | (1u128 << (ids::CEMETERY - 256))
        | (1u128 << (ids::SKULK - 256))
        | (1u128 << (ids::CURSED_VILLAGE - 256));

    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::GUARDIAN => (0, 0, 0, 1),
            ids::GHOST_TOWN => (1, 1, 0, 0),
            ids::DEN_OF_SIN => (2, 0, 0, 0),
            ids::RAIDER => (0, 0, 0, 3),
            ids::SECRET_CAVE => (0, 0, 0, 3),
            _ => (0, 0, 0, 0),
        }
    }

    /// Cobbler, Crypt and Ghost add to the hand or play a card; Lost in the Woods discards; a Boon kept by Blessed Village is received.
    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, ids::COBBLER | ids::CRYPT | ids::GHOST | ids::LOST_IN_THE_WOODS | ids::BLESSED_VILLAGE)
    }

    /// A Boon waiting from a Blessed Village is named by the Boon.
    fn start_entry_named_by_arg(card: CardId) -> bool {
        card == ids::BLESSED_VILLAGE
    }

    /// Pasture is worth 1 VP per Estate its owner has.
    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        match card {
            ids::PASTURE => all.get(id::ESTATE) as i32,
            _ => cards::def(card).vp as i32,
        }
    }

    /// Secret Cave and Crypt keep themselves in play only when they have something to do next turn; Ghost when it set an Action aside.
    fn schedules_own_duration(card: CardId) -> bool {
        matches!(card, ids::SECRET_CAVE | ids::CRYPT | ids::GHOST)
    }

    #[inline(never)]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        match card {
            ids::DRUID => g.stack.push(Frame { cost_sum: g.noc.boon_druid, exact: true, ..Frame::new(K::PickBoon, p, card) }),
            ids::GUARDIAN => g.players[pi].noc |= crate::state::P_GUARDED,
            ids::MONASTERY => {
                let n = g.turn.noc.gains;
                if n > 0 {
                    // For each card gained this turn: a card from the hand or a Copper in play (the hand first, then the Coppers).
                    g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, n, Then::MonasteryCopper));
                }
            }
            ids::PIXIE => g.stack.push(Frame { then: Then::PixieDraw, ..Frame::new(K::ReceiveBoon, p, card) }),
            ids::TRACKER => {
                g.turn.noc.tracker = true;
                g.stack.push(Frame::new(K::ReceiveBoon, p, card));
            }
            ids::CHANGELING => {
                // Trash this, then gain a copy of a card in play (this one is no longer there).
                if g.leave_play_self_now(p, card) {
                    g.trash_card(p, card, true, sink);
                }
                g.stack.push(Frame { distinct: true, ..select(p, card, Zone::InPlay, Reveal, Filter::GainableCopy, 1, 1, Then::GainCopyEachPick) });
            }
            ids::FOOL => {
                if g.noc.lost != p + 1 {
                    g.take_lost_in_the_woods(p, sink);
                    g.stack.push(Frame { max: 3, ..Frame::new(K::PickBoon, p, card) });
                }
            }
            ids::LEPRECHAUN => {
                // The cards in play are counted after the Gold is gained, reactions to it included (a Sheepdog played then counts): the check is
                // pushed first so that it runs last.
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::LeprechaunCheck));
                g.gain(p, id::GOLD, Dest::Discard, false, card, sink);
            }
            ids::NIGHT_WATCHMAN => g.look_at_top(p, card, 5),
            ids::SECRET_CAVE => g.stack.push(Frame { exact: true, ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::SecretCave) }),
            ids::BARD => g.stack.push(Frame::new(K::ReceiveBoon, p, card)),
            ids::CONCLAVE => g.stack.push(select(p, card, Zone::Hand, Play, Filter::NotInPlayAction, 0, 1, Then::ConclavePlay)),
            ids::DEVILS_WORKSHOP => match g.turn.noc.gains {
                0 => {
                    g.gain(p, id::GOLD, Dest::Discard, false, card, sink);
                }
                1 => g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard)),
                _ => {
                    g.gain_side(p, ids::IMP, Dest::Discard, card, sink);
                }
            },
            ids::EXORCIST => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::ExorcistGain)),
            ids::NECROMANCER => g.stack.push(select(p, card, Zone::Trash, Play, Filter::NecroTarget, 1, 1, Then::PlayPicked { times: 1 })),
            ids::SHEPHERD => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Victory, 0, ALL, Then::DrawNPerPick(2))),
            ids::SKULK => g.hex_all_others(p, card, sink),
            ids::CRYPT => g.stack.push(select(p, card, Zone::InPlay, Archive, Filter::TreasureNonDuration, 0, ALL, Then::CryptSet)),
            ids::CURSED_VILLAGE => {
                let n = 6u32.saturating_sub(g.players[pi].hand.total());
                if n > 0 {
                    g.stack.push(draw_frame(p, card, n as u8));
                }
            }
            ids::IDOL => {
                if g.players[pi].in_play.get(card) % 2 == 1 {
                    g.stack.push(Frame::new(K::ReceiveBoon, p, card));
                } else {
                    let (vs, n) = g.victims(sink);
                    g.push_gain_to_each(&vs[..n], id::CURSE, card);
                }
            }
            ids::POOKA => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::TreasureNot(ids::CURSED_GOLD), 0, 1, Then::DrawIfCount { count: 1, draw: 4 })),
            ids::SACRED_GROVE => g.stack.push(Frame { then: Then::GroveBoon, ..Frame::new(K::ReceiveBoon, p, card) }),
            ids::TORMENTOR => {
                let own = u32::from(g.self_in_play_now(p, card));
                let others = g.players[pi].in_play.total().saturating_sub(own);
                if others == 0 {
                    g.gain_side(p, ids::IMP, Dest::Discard, card, sink);
                } else {
                    g.hex_all_others(p, card, sink);
                }
            }
            ids::TRAGIC_HERO => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::TragicHero)),
            ids::VAMPIRE => {
                // In order: the Hex, the gain, the exchange (pushed in the reverse order).
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::VampireBat));
                g.stack.push(gain_frame(p, card, 5, Filter::NotCard(ids::VAMPIRE), Dest::Discard));
                g.hex_all_others(p, card, sink);
            }
            ids::WEREWOLF => {
                if g.turn.phase == Phase::Night && p == g.turn.player {
                    g.hex_all_others(p, card, sink);
                } else {
                    g.stack.push(draw_frame(p, card, 3));
                }
            }
            ids::RAIDER => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    if g.players[v as usize].hand.total() >= 5 {
                        g.stack.push(select(v, card, Zone::Hand, Discard, Filter::CopyInAttackersPlay, 1, 1, Then::Nothing));
                    }
                }
            }
            // The Heirlooms.
            ids::MAGIC_LAMP => {
                // 6 or more cards of which exactly one copy is in play (this one counts): trash it, gain 3 Wishes.
                let singles = g.players[pi].in_play.iter().filter(|&(_, n)| n == 1).count();
                if singles >= 6 && g.leave_play_self_now(p, card) {
                    g.trash_card(p, card, true, sink);
                    for _ in 0..3 {
                        g.gain_side(p, ids::WISH, Dest::Discard, card, sink);
                    }
                }
            }
            ids::GOAT => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing)),
            ids::LUCKY_COIN => {
                g.gain(p, id::SILVER, Dest::Discard, false, card, sink);
            }
            ids::CURSED_GOLD => {
                g.gain(p, id::CURSE, Dest::Discard, false, card, sink);
            }
            // The cards outside the Supply.
            ids::WILL_O_WISP => g.stack.push(Frame { max: 1, then: Then::WispCheck, ..Frame::new(K::RevealTop, p, card) }),
            ids::WISH => {
                if g.leave_play_self_now(p, card) {
                    g.return_to_pile(p, card, sink);
                    g.stack.push(gain_frame(p, card, 6, Filter::Any, Dest::Hand));
                }
            }
            ids::BAT => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 2, Then::BatExchange)),
            ids::IMP => g.stack.push(select(p, card, Zone::Hand, Play, Filter::NotInPlayAction, 0, 1, Then::PlayPicked { times: 1 })),
            ids::GHOST => g.stack.push(Frame { filter: Filter::Action, max: 1, then: Then::GhostSet, ..Frame::new(K::RevealUntil, p, card) }),
            ids::ZOMBIE_APPRENTICE => {
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Action, 0, 1, Then::DrawActionsIfPicked { draw: 3, actions: 1 }))
            }
            ids::ZOMBIE_MASON => g.stack.push(Frame { act: Trash, then: Then::MasonTrash, ..Frame::new(K::TopCard, p, card) }),
            ids::ZOMBIE_SPY => g.look_at_top_discard_one(p, card),
            _ => {}
        }
    }

    #[inline(never)]
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
        match e.card {
            ids::COBBLER => {
                for _ in 0..e.times {
                    g.stack.push(Frame { depth: 1, ..gain_frame(p, e.card, 4, Filter::Any, Dest::Hand) });
                }
            }
            ids::CRYPT => {
                // One of the Treasures under it into the hand; while any remain it stays for the next turn too.
                g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Archive, Act::ToHand, Filter::Any, 1, 1, Then::Nothing) });
                if e.arg > 1 {
                    g.push_duration_pending(p, e.card, 1, e.arg - 1);
                }
            }
            ids::GHOST => {
                // Play the Action it set aside twice (it stays in play with a Duration it plays).
                let c = e.arg;
                if c != 0 && g.players[p as usize].set_aside.remove(c) {
                    g.put(p, c, Act::Play, Dest::Discard, e.card, sink);
                    let fr = Frame { last: c, depth: 1, ..Frame::new(K::Select, p, e.card) };
                    g.push_plays(&fr, 2);
                }
            }
            _ => {}
        }
    }

    /// Lost in the Woods' discard-for-a-Boon and the Boons Blessed Villages took, at the start of the turn.
    fn turn_start_entries(g: &mut GameState) {
        if g.noc.flags & (crate::state::N_BOONS | crate::state::N_HEXES) == 0 {
            return;
        }
        let p = g.turn.player;
        if g.noc.lost == p + 1 {
            g.turn.start_pending.push(PendingDuration { card: ids::LOST_IN_THE_WOODS, times: 1, arg: 0, used: 0 });
        }
        let later = g.players[p as usize].boons_later;
        if later != 0 {
            for i in 0..cards::NUM_BOONS as u16 {
                if later & (1 << i) != 0 {
                    g.turn.start_pending.push(PendingDuration { card: ids::BLESSED_VILLAGE, times: 1, arg: cards::FIRST_BOON + i, used: 0 });
                }
            }
        }
    }

    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) -> bool {
        match e.card {
            ids::LOST_IN_THE_WOODS => {
                g.stack.push(select(p, ids::LOST_IN_THE_WOODS, Zone::Hand, Act::Discard, Filter::Any, 0, 1, Then::LostBoon));
                true
            }
            ids::BLESSED_VILLAGE => {
                g.players[p as usize].boons_later &= !(1 << boon_index(e.arg));
                g.stack.push(Frame { subject: e.arg, ..Frame::new(K::ReceiveBoon, p, ids::BLESSED_VILLAGE) });
                true
            }
            _ => false,
        }
    }

    /// Skulk's Gold, Blessed Village's Boon, Cursed Village's Hex, Cemetery's trashing, the Changeling exchange, Tracker's topdecking.
    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, _on_buy: bool, sink: &mut S) {
        match c {
            ids::SKULK => {
                g.gain(p, id::GOLD, Dest::Discard, false, ids::SKULK, sink);
            }
            ids::BLESSED_VILLAGE => g.stack.push(Frame { optional: true, then: Then::VillageDraw, ..Frame::new(K::ReceiveBoon, p, c) }),
            ids::CURSED_VILLAGE => g.stack.push(Frame { min: 1 << p, ..Frame::new(K::ReceiveHex, p, c) }),
            ids::CEMETERY => g.stack.push(select(p, c, Zone::Hand, Act::Trash, Filter::Any, 0, 4, Then::Nothing)),
            _ => {}
        }
        // Tracker: this turn, a card the player gains may be put onto their deck.
        if g.turn.noc.tracker && p == g.turn.player && to != Dest::DeckTop {
            g.stack.push(Frame {
                zone: GameState::gain_dest_zone(to), act: Act::Topdeck, subject: c,
                ..Frame::new(K::YesNo, p, ids::TRACKER)
            });
        }
        // Changeling: a card costing $3 or more gained while Changeling is in the Supply may be exchanged for one (asked first: the other
        // effects of the gain still happen). The cost is the one the card had as it was gained (a Fisherman gained to an empty discard pile cost $2).
        if g.in_supply(ids::CHANGELING) && g.supply.get(ids::CHANGELING) > 0 && c != ids::CHANGELING && g.cost_when_gained(p, c, to) >= 3 {
            g.stack.push(Frame {
                zone: GameState::gain_dest_zone(to), act: Act::Return, subject: c, then: Then::ChangelingSwap,
                ..Frame::new(K::YesNo, p, ids::CHANGELING)
            });
        }
    }

    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.turn.noc.tracker
    }

    /// Haunted Mirror: when trashed, its owner may discard an Action card to gain a Ghost.
    fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, _sink: &mut S) {
        if card == ids::HAUNTED_MIRROR {
            g.stack.push(select(p, card, Zone::Hand, Act::Discard, Filter::Action, 0, 1, Then::MirrorGhost));
        }
    }
}

// -------------------------------------------------------------------------------------------
// Boons, Hexes and States
// -------------------------------------------------------------------------------------------

impl GameState {
    /// Draw the next Boon from the deck: its index. The deck's order is not stored: the next Boon is one of those in the deck, equally likely, or
    /// (in chance mode) a chance outcome the search enumerates. `Ok(None)` when no Boon is left to draw (all are set aside or in play).
    pub(crate) fn draw_boon(&mut self) -> Result<Option<u16>, NeedChance> {
        // Reshuffle: when the deck is empty the discarded Boons form the deck again (idempotent, so a chance step can redo this).
        if self.noc.boon_deck == 0 {
            self.noc.boon_deck = std::mem::take(&mut self.noc.boon_discard);
        }
        let deck = self.noc.boon_deck;
        if deck == 0 {
            return Ok(None);
        }
        let i = if self.noc.boon_top != 0 {
            let i = self.noc.boon_top as u16 - 1;
            self.noc.boon_top = 0;
            i
        } else if self.chance_mode {
            return Err(NeedChance(crate::state::BOON_CHANCE));
        } else {
            nth_bit(deck, self.rng.below(deck.count_ones()))
        };
        self.noc.boon_deck &= !(1 << i);
        Ok(Some(i))
    }

    /// The same for the Hexes.
    pub(crate) fn draw_hex(&mut self) -> Result<Option<u16>, NeedChance> {
        if self.noc.hex_deck == 0 {
            self.noc.hex_deck = std::mem::take(&mut self.noc.hex_discard);
        }
        let deck = self.noc.hex_deck;
        if deck == 0 {
            return Ok(None);
        }
        let i = if self.noc.hex_top != 0 {
            let i = self.noc.hex_top as u16 - 1;
            self.noc.hex_top = 0;
            i
        } else if self.chance_mode {
            return Err(NeedChance(crate::state::HEX_CHANCE));
        } else {
            nth_bit(deck, self.rng.below(deck.count_ones()))
        };
        self.noc.hex_deck &= !(1 << i);
        Ok(Some(i))
    }

    /// A Boon that has been received goes to the Boons discard pile, or stays in front of the player until Clean-up (the Field's, Forest's and
    /// River's Gifts).
    fn place_boon(&mut self, boon: CardId) {
        let bit = 1u16 << boon_index(boon);
        self.noc.boon_discard &= !bit;
        self.noc.boon_held &= !bit;
        if bit & KEPT_BOONS != 0 {
            self.noc.boon_held |= bit;
        } else {
            self.noc.boon_discard |= bit;
        }
    }

    /// `ReceiveBoon`: draw the Boon if it is not known yet, ask (Sacred Grove's other players, Blessed Village), receive it.
    #[inline(never)]
    pub(crate) fn run_receive_boon<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        if f.subject == 0 {
            match self.draw_boon() {
                Err(NeedChance(q)) => return Run::Chance(q),
                Ok(None) => {
                    self.stack.pop();
                    return Run::Continue;
                }
                Ok(Some(i)) => f.subject = cards::FIRST_BOON + i,
            }
            // Pixie: the top Boon is discarded; the Pixie may be trashed to receive it twice.
            if f.then == Then::PixieDraw {
                self.stack.pop();
                self.place_boon(f.subject);
                if self.self_in_play(p, ids::PIXIE, f.tok) {
                    self.stack.push(Frame {
                        zone: Zone::InPlay, act: Act::Trash, subject: ids::PIXIE, then: Then::PixieTrash, last: f.subject, depth: f.depth, tok: f.tok,
                        ..Frame::new(K::YesNo, p, ids::PIXIE)
                    });
                }
                return Run::Continue;
            }
            // Sacred Grove: the Boon is received by its player, then (unless it gives +$1) each other player may receive it.
            if f.then == Then::GroveBoon {
                let b = f.subject;
                self.stack.pop();
                if b != ids::THE_FIELDS_GIFT && b != ids::THE_FORESTS_GIFT {
                    // Pushed under the receiver's own frame, so they decide and receive after the Grove's player.
                    let n = self.num_players;
                    for i in (1..n).rev() {
                        let v = (p + i) % n;
                        self.stack.push(Frame { optional: true, depth: f.depth, subject: b, ..Frame::new(K::ReceiveBoon, v, f.source) });
                    }
                }
                f.then = Then::Nothing;
                self.stack.push(f);
                return Run::Continue;
            }
            self.stack.set_top(f);
        }
        // The Boon is known: ask whether it is wanted first.
        if f.optional && f.min == 0 {
            return Run::Decide(DecisionKind::YesNo { act: Act::Reveal }, f.subject);
        }
        self.stack.pop();
        let times = f.count.max(1);
        // `exact`: the Boon stays where it is (Druid's three set-aside Boons).
        for _ in 0..times {
            self.receive_boon(p, f.subject, f.source, f.depth, !f.exact, sink);
        }
        Run::Continue
    }

    /// `p` receives the Boon `boon` now: it is logged, placed (unless it stays where it is), and its effect begins.
    #[inline(never)]
    pub(crate) fn receive_boon<S: EventSink>(&mut self, p: u8, boon: CardId, source: CardId, depth: u8, place: bool, sink: &mut S) {
        sink.event(Event::Boon { player: p, boon, source });
        if place {
            self.place_boon(boon);
        }
        let base = self.stack.len as usize;
        let pi = p as usize;
        match boon {
            ids::THE_EARTHS_GIFT => self.stack.push(select(
                p, boon, Zone::Hand, Act::Discard, Filter::Treasure, 0, 1,
                Then::GainFixedUpTo { max_cost: 4, filter: Filter::Any, dest: Dest::Discard },
            )),
            ids::THE_FIELDS_GIFT => {
                self.turn.actions += 1;
                self.turn.coins += 1;
                sink.bonus(p, boon, 1, 0, 1, 0);
            }
            ids::THE_FLAMES_GIFT => self.stack.push(select(p, boon, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing)),
            ids::THE_FORESTS_GIFT => {
                self.turn.buys += 1;
                self.turn.coins += 1;
                sink.bonus(p, boon, 0, 1, 1, 0);
            }
            ids::THE_MOONS_GIFT => self.stack.push(select(p, boon, Zone::Discard, Act::Topdeck, Filter::Any, 0, 1, Then::Nothing)),
            ids::THE_MOUNTAINS_GIFT => {
                self.gain(p, id::SILVER, Dest::Discard, false, boon, sink);
            }
            ids::THE_RIVERS_GIFT => self.turn.noc.river[pi] = self.turn.noc.river[pi].saturating_add(1),
            ids::THE_SEAS_GIFT => self.stack.push(draw_frame(p, boon, 1)),
            ids::THE_SKYS_GIFT => {
                self.stack.push(Frame {
                    exact: true,
                    ..select(p, boon, Zone::Hand, Act::Discard, Filter::Any, 0, 3, Then::GainCardIfCount { count: 3, card: id::GOLD, dest: Dest::Discard })
                })
            }
            ids::THE_SUNS_GIFT => self.look_at_top(p, boon, 4),
            ids::THE_SWAMPS_GIFT => {
                self.gain_side(p, ids::WILL_O_WISP, Dest::Discard, boon, sink);
            }
            ids::THE_WINDS_GIFT => {
                self.stack.push(select(p, boon, Zone::Hand, Act::Discard, Filter::Any, 2, 2, Then::Nothing));
                self.stack.push(draw_frame(p, boon, 2));
            }
            _ => {}
        }
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            f.depth = depth;
        }
    }

    /// `PickBoon`: draw the Boons the Fool takes into the pool, then pick one at a time.
    #[inline(never)]
    pub(crate) fn run_pick_boon(&mut self, mut f: Frame, _sink: &mut impl EventSink) -> Run {
        if f.max > 0 {
            match self.draw_boon() {
                Err(NeedChance(q)) => return Run::Chance(q),
                Ok(None) => f.max = 0,
                Ok(Some(i)) => {
                    f.cost_sum |= 1 << i;
                    f.max -= 1;
                }
            }
            self.stack.set_top(f);
            return Run::Continue;
        }
        if f.cost_sum == 0 {
            self.stack.pop();
            return Run::Continue;
        }
        Run::Decide(DecisionKind::PickBoon, 0)
    }

    #[inline(never)]
    pub(crate) fn pick_boon_choices(&self, f: Frame, out: &mut ChoiceBuf) {
        for i in 0..cards::NUM_BOONS as u16 {
            if f.cost_sum & (1 << i) != 0 {
                out.push(Choice::Card(cards::FIRST_BOON + i));
            }
        }
    }

    #[inline(never)]
    pub(crate) fn apply_pick_boon(&mut self, mut f: Frame, boon: CardId) {
        let bit = 1u16 << boon_index(boon);
        // Druid's Boons stay set aside, and the Druid's one pick is done; the Fool's pool shrinks until it is empty.
        if f.exact {
            self.stack.pop();
        } else {
            f.cost_sum &= !bit;
            self.stack.set_top(f);
        }
        self.stack.push(Frame { subject: boon, exact: f.exact, depth: f.depth, ..Frame::new(K::ReceiveBoon, f.player, f.source) });
    }

    /// `ReceiveBoon` answered: yes receives it; no leaves it (a Blessed Village's Boon waits for the next turn).
    #[inline(never)]
    pub(crate) fn answer_receive_boon(&mut self, mut f: Frame, yes: bool) {
        if yes {
            f.min = 1;
            self.stack.set_top(f);
            return;
        }
        self.stack.pop();
        if f.then == Then::VillageDraw {
            self.noc.boon_discard &= !(1 << boon_index(f.subject));
            self.players[f.player as usize].boons_later |= 1 << boon_index(f.subject);
        }
    }

    /// `ReceiveHex`: draw the Hex (once), then every player in the mask `min` receives it in turn order from the player's left.
    #[inline(never)]
    pub(crate) fn run_receive_hex<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        if f.count == 1 {
            // One victim, now.
            self.stack.pop();
            self.hex_effect(p, f.subject, f.source, f.depth, sink);
            return Run::Continue;
        }
        if f.subject == 0 {
            match self.draw_hex() {
                Err(NeedChance(q)) => return Run::Chance(q),
                Ok(None) => {
                    self.stack.pop();
                    return Run::Continue;
                }
                Ok(Some(i)) => {
                    f.subject = cards::FIRST_HEX + i;
                    self.noc.hex_discard |= 1 << i;
                }
            }
        }
        self.stack.pop();
        let n = self.num_players;
        // The victims are the players of the mask: the Doom card's player first if they are in it ("receive a Hex"), then the others in turn order.
        for i in (0..n).rev() {
            let v = (p + i) % n;
            if f.min & (1 << v) != 0 {
                self.stack.push(Frame { count: 1, min: 0, subject: f.subject, depth: f.depth, ..Frame::new(K::ReceiveHex, v, f.source) });
            }
        }
        Run::Continue
    }

    /// "Each other player receives the next Hex": the players not immune to the Attack.
    #[inline(never)]
    pub(crate) fn hex_all_others<S: EventSink>(&mut self, p: u8, source: CardId, sink: &mut S) {
        let (vs, n) = self.victims(sink);
        let mut mask = 0u8;
        for &v in &vs[..n] {
            mask |= 1 << v;
        }
        if mask != 0 {
            self.stack.push(Frame { min: mask, ..Frame::new(K::ReceiveHex, p, source) });
        }
    }

    /// What the Hex `hex` does to `v`.
    #[inline(never)]
    pub(crate) fn hex_effect<S: EventSink>(&mut self, v: u8, hex: CardId, source: CardId, depth: u8, sink: &mut S) {
        sink.event(Event::Hex { player: v, hex, source });
        let base = self.stack.len as usize;
        let vi = v as usize;
        match hex {
            ids::BAD_OMENS => {
                // The deck goes into the discard pile; two Coppers from it back onto the deck (or as many as there are).
                let ps = &mut self.players[vi];
                let n = ps.deck_size();
                let deck = ps.deck_counts();
                ps.deck_known.clear();
                ps.deck_known_bottom.clear();
                ps.deck_unknown.clear();
                ps.discard.add_all(&deck);
                sink.event(Event::DeckToDiscard { player: v, n: n as u16 });
                for _ in 0..2 {
                    self.cover_deck(v);
                    let ps = &mut self.players[vi];
                    if ps.discard.remove(id::COPPER) {
                        ps.deck_known.push_top(id::COPPER);
                        sink.event(Event::Topdeck { player: v, card: id::COPPER });
                    }
                }
            }
            ids::DELUSION => self.take_state(v, ids::DELUDED, sink),
            ids::ENVY => self.take_state(v, ids::ENVIOUS, sink),
            ids::FAMINE => self.stack.push(Frame { max: 3, then: Then::FamineSort, ..Frame::new(K::RevealTop, v, hex) }),
            ids::FEAR => {
                if self.players[vi].hand.total() >= 5 {
                    self.stack.push(select(v, hex, Zone::Hand, Act::Discard, Filter::ActionOrTreasure, 1, 1, Then::Nothing));
                }
            }
            ids::GREED => {
                self.gain(v, id::COPPER, Dest::DeckTop, false, hex, sink);
            }
            ids::HAUNTING => {
                if self.players[vi].hand.total() >= 4 {
                    self.stack.push(select(v, hex, Zone::Hand, Act::Topdeck, Filter::Any, 1, 1, Then::Nothing));
                }
            }
            ids::LOCUSTS => self.stack.push(Frame { act: Act::Trash, then: Then::LocustsHit, ..Frame::new(K::TopCard, v, hex) }),
            ids::MISERY => self.take_state(v, ids::MISERABLE, sink),
            ids::PLAGUE => {
                self.gain(v, id::CURSE, Dest::Hand, false, hex, sink);
            }
            ids::POVERTY => {
                self.stack.push(Frame { down_to_target: true, ..select(v, hex, Zone::Hand, Act::Discard, Filter::Any, 0, 3, Then::Nothing) })
            }
            ids::WAR => {
                // Reveal until a card costing $3 or $4: trash it, discard the rest.
                self.stack.push(select(v, hex, Zone::Revealed, Act::Trash, Filter::Any, 1, 1, Then::Nothing));
                self.stack.push(Frame {
                    filter: Filter::CostRange(3, 4), max: 1, then: Then::DiscardRevealedNotMatching, ..Frame::new(K::RevealUntil, v, hex)
                });
            }
            _ => {}
        }
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            f.depth = depth;
        }
    }

    /// `p` takes the State `state` (Delusion/Envy give Deluded/Envious unless the player has either; Misery gives Miserable, and Twice Miserable if
    /// they have it already; the Fool gives Lost in the Woods, taking it from whoever has it).
    #[inline(never)]
    pub fn take_state<S: EventSink>(&mut self, p: u8, state: CardId, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        match state {
            ids::DELUDED | ids::ENVIOUS => {
                if ps.noc & (crate::state::P_DELUDED | crate::state::P_ENVIOUS) == 0 {
                    ps.noc |= if state == ids::DELUDED { crate::state::P_DELUDED } else { crate::state::P_ENVIOUS };
                    sink.event(Event::State { player: p, state, taken: true });
                }
            }
            ids::MISERABLE => {
                if ps.noc & crate::state::P_MISERABLE == 0 {
                    ps.noc |= crate::state::P_MISERABLE;
                    sink.event(Event::State { player: p, state: ids::MISERABLE, taken: true });
                } else if ps.noc & crate::state::P_TWICE_MISERABLE == 0 {
                    ps.noc |= crate::state::P_TWICE_MISERABLE;
                    sink.event(Event::State { player: p, state: ids::TWICE_MISERABLE, taken: true });
                }
            }
            _ => {}
        }
    }

    /// The Fool: `p` takes Lost in the Woods (from the player who has it, if any).
    #[inline(never)]
    pub(crate) fn take_lost_in_the_woods<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        if self.noc.lost != 0 {
            sink.event(Event::State { player: self.noc.lost - 1, state: ids::LOST_IN_THE_WOODS, taken: false });
        }
        self.noc.lost = p + 1;
        sink.event(Event::State { player: p, state: ids::LOST_IN_THE_WOODS, taken: true });
    }

    /// The start of the Buy phase: Deluded and Envious are returned and apply for the rest of the turn.
    #[cold]
    #[inline(never)]
    pub(crate) fn states_at_buy<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let ps = &mut self.players[p as usize];
        if ps.noc & crate::state::P_DELUDED != 0 {
            ps.noc &= !crate::state::P_DELUDED;
            self.turn.noc.deluded = true;
            sink.event(Event::State { player: p, state: ids::DELUDED, taken: false });
        }
        if ps.noc & crate::state::P_ENVIOUS != 0 {
            ps.noc &= !crate::state::P_ENVIOUS;
            self.turn.noc.envy = true;
            sink.event(Event::State { player: p, state: ids::ENVIOUS, taken: false });
        }
    }

    /// Look at the top `n` cards of the deck, discard any number and put the rest back in any order (Night Watchman, The Sun's Gift).
    #[inline(never)]
    pub(crate) fn look_at_top(&mut self, p: u8, source: CardId, n: u8) {
        self.stack.push(Frame { ordered: true, ..select(p, source, Zone::Revealed, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
        self.stack.push(select(p, source, Zone::Revealed, Act::Discard, Filter::Any, 0, ALL, Then::Nothing));
        self.stack.push(Frame { max: n, ..Frame::new(K::RevealTop, p, source) });
    }

    /// Zombie Spy: look at the top card of the deck; discard it or put it back.
    #[inline(never)]
    pub(crate) fn look_at_top_discard_one(&mut self, p: u8, source: CardId) {
        self.stack.push(Frame { ordered: true, ..select(p, source, Zone::Revealed, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
        self.stack.push(select(p, source, Zone::Revealed, Act::Discard, Filter::Any, 0, 1, Then::Nothing));
        self.stack.push(Frame { max: 1, then: Then::Nothing, exact: true, ..Frame::new(K::RevealTop, p, source) });
    }

    // ---- gains and discards ----

    /// A card gained while the gain hook is live: count it (Monastery, Devil's Workshop) and send some to the hand (Guardian, Ghost Town, Night
    /// Watchman, Den of Sin: "this is gained to your hand instead of your discard pile").
    #[inline(never)]
    pub(crate) fn gain_hook(&mut self, p: u8, c: CardId, to: Dest) -> Dest {
        if p == self.turn.player {
            self.turn.noc.gains = self.turn.noc.gains.saturating_add(1);
        }
        if to == Dest::Discard && matches!(c, ids::GUARDIAN | ids::GHOST_TOWN | ids::NIGHT_WATCHMAN | ids::DEN_OF_SIN) {
            Dest::Hand
        } else {
            to
        }
    }

    /// `n` Faithful Hounds were just discarded (not at Clean-up) by `p`: each may be set aside until the end of the turn.
    #[inline(never)]
    pub(crate) fn hounds_discarded(&mut self, p: u8, n: u8) {
        if self.turn.phase == Phase::CleanupDraw {
            return;
        }
        for _ in 0..n {
            self.stack.push(Frame {
                zone: Zone::Discard, act: Act::SetAside, subject: ids::FAITHFUL_HOUND, then: Then::HoundAside,
                ..Frame::new(K::YesNo, p, ids::FAITHFUL_HOUND)
            });
        }
    }

    /// Clean-up: Boons kept in front of players are discarded; the cards River's Gift draws and the Faithful Hounds set aside come after the new
    /// hand is drawn (these frames are pushed under the draw).
    #[inline(never)]
    pub(crate) fn nocturne_cleanup(&mut self) {
        self.noc.boon_discard |= std::mem::take(&mut self.noc.boon_held);
        for q in 0..self.num_players {
            let n = std::mem::take(&mut self.turn.noc.river[q as usize]);
            if n > 0 {
                self.stack.push(draw_frame(q, ids::THE_RIVERS_GIFT, n));
            }
            if self.players[q as usize].hounds > 0 {
                self.stack.push(select(q, ids::FAITHFUL_HOUND, Zone::Hand, Act::Discard, Filter::Any, 0, 0, Then::HoundsToHand));
            }
        }
    }

    /// `c` goes back to its pile: a pile outside the Supply, the Knights or Ruins pile (the card becomes its top), or a Supply pile.
    #[inline(never)]
    pub(crate) fn return_to_pile<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        sink.event(Event::Return { player: p, card: c });
        match c {
            ids::WILL_O_WISP => self.noc.piles[crate::state::NOC_WISP] += 1,
            ids::WISH => self.noc.piles[crate::state::NOC_WISH] += 1,
            ids::BAT => self.noc.piles[crate::state::NOC_BAT] += 1,
            ids::IMP => self.noc.piles[crate::state::NOC_IMP] += 1,
            ids::GHOST => self.noc.piles[crate::state::NOC_GHOST] += 1,
            // Allies: a card of a split pile goes on top of it, even when other cards of the pile are above or below.
            _ if cards::is_allies_split(c) => self.al_return(c),
            _ if c >= cards::FIRST_PILE_CARD => {
                // The returned member of a Knights or Ruins pile becomes the top card; the old top card is not visible any more.
                if let Some(pile) = crate::sets::dark_ages::MixedPile::of_member(c) {
                    let left = self.pile_size(pile);
                    for m in pile.members() {
                        self.supply.set(m, 0);
                    }
                    self.supply.set(c, left + 1);
                    if (left as usize) < pile.capacity() {
                        self.pile_order[pile.offset as usize + left as usize] = (c - pile.first) as u8;
                    }
                }
            }
            _ => self.supply.add(c, 1),
        }
    }

    // ---- frames that need the engine's state ----

    /// The Locusts Hex and Zombie Mason: the top card of the deck is trashed.
    #[inline(never)]
    pub(crate) fn run_noc_top_card<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        match take_top!(self, p, sink) {
            None => {
                self.stack.pop();
            }
            Some(c) => {
                self.stack.pop();
                let cost = self.cost(c);
                let potion = cards::potion_cost(c);
                if f.then == Then::LocustsHit {
                    if c == id::COPPER || c == id::ESTATE {
                        self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
                    } else {
                        // A cheaper card that shares a type with it (every part of the cost no larger, and not equal).
                        self.stack.push(Frame {
                            less: true, potion, min: cards::debt_cost(c), depth: f.depth, ..gain_frame(p, f.source, cost, Filter::SharesTypeWith(c), Dest::Discard)
                        });
                    }
                } else {
                    // Zombie Mason: a card costing up to $1 more, if the player likes.
                    self.stack.push(Frame {
                        optional: true, potion, depth: f.depth, ..gain_frame(p, f.source, cost.saturating_add(1), Filter::Any, Dest::Discard)
                    });
                }
                self.trash_card(p, c, true, sink);
                self.flush_trash_triggers(f.depth, sink);
            }
        }
        Run::Continue
    }

    /// Will-o'-Wisp's revealed card, the Famine Hex's three, Ghost's Action: a `RevealTop` / `RevealUntil` that has finished.
    #[inline(never)]
    pub(crate) fn finish_noc_reveal<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            Then::WispCheck => {
                // The revealed card (in `set_aside`) goes to the hand if it costs $2 or less (a Potion or Debt in the cost is more), else stays on top.
                let Some(c) = self.players[pi].set_aside.iter().next().map(|(c, _)| c) else { return };
                self.players[pi].set_aside.remove(c);
                if self.cost(c) <= 2 && !cards::potion_cost(c) && cards::debt_cost(c) == 0 {
                    self.players[pi].hand.add(c, 1);
                    sink.event(Event::Draw { player: p, card: c });
                } else {
                    self.players[pi].deck_known.push_top(c);
                }
            }
            Then::FamineSort => {
                // The Actions are discarded; the rest is shuffled into the deck (what was known of the deck's order is lost).
                let revealed = self.players[pi].set_aside;
                self.players[pi].set_aside.clear();
                let mut discarded = Counts::EMPTY;
                for (c, n) in revealed.iter() {
                    if cards::is(c, cards::ACTION) {
                        self.players[pi].discard.add(c, n);
                        for _ in 0..n {
                            sink.event(Event::Discard { player: p, card: c });
                        }
                        discarded.add(c, n);
                    } else {
                        self.players[pi].deck_unknown.add(c, n);
                    }
                }
                let ps = &mut self.players[pi];
                let known = ps.deck_known.counts();
                ps.deck_unknown.add_all(&known);
                ps.deck_known.clear();
                let bottom = ps.deck_known_bottom.counts();
                ps.deck_unknown.add_all(&bottom);
                ps.deck_known_bottom.clear();
                self.rs_shuffled(p);
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.discard_hook_counts(p, &discarded, f.depth, sink);
                }
            }
            Then::GhostSet => {
                // Everything revealed but the Action is discarded; the Action stays set aside and is played twice at the start of the next turn.
                let revealed = self.players[pi].set_aside;
                let mut discarded = Counts::EMPTY;
                for (c, n) in revealed.iter() {
                    let keep = if f.count > 0 && c == f.last { 1 } else { 0 };
                    let discard = n.saturating_sub(keep);
                    if discard > 0 {
                        self.players[pi].set_aside.set(c, keep);
                        self.players[pi].discard.add(c, discard);
                        for _ in 0..discard {
                            sink.event(Event::Discard { player: p, card: c });
                        }
                        discarded.add(c, discard);
                    }
                }
                if f.count > 0 {
                    sink.event(Event::SetAside { player: p, card: f.last });
                    self.push_duration_pending(p, f.source, 1, f.last);
                }
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.discard_hook_counts(p, &discarded, f.depth, sink);
                }
            }
            _ => {}
        }
    }

    /// The `then` of a finished `Select` or `YesNo` that belongs to Nocturne.
    #[inline(never)]
    pub(crate) fn finish_noc_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            Then::DrawNPerPick(n) => {
                if f.count > 0 {
                    self.stack.push(draw_frame(p, f.source, n.saturating_mul(f.count)));
                }
            }
            Then::DrawActionsIfPicked { draw, actions } => {
                if f.count > 0 {
                    self.turn.actions += actions;
                    sink.bonus(p, f.source, actions, 0, 0, 0);
                    self.stack.push(draw_frame(p, f.source, draw));
                }
            }
            Then::ConclavePlay => {
                if f.count > 0 {
                    self.turn.actions += 1;
                    sink.bonus(p, f.source, 1, 0, 0, 0);
                    self.push_plays(f, 1);
                }
            }
            Then::MirrorGhost => {
                if f.count > 0 {
                    self.gain_side(p, ids::GHOST, Dest::Discard, f.source, sink);
                }
            }
            Then::ExorcistGain => {
                if f.count > 0 {
                    let c = f.last;
                    self.stack.push(Frame {
                        gf: crate::state::GF_SPIRITS, less: true, potion: cards::potion_cost(c), min: cards::debt_cost(c), depth: f.depth,
                        ..gain_frame(p, f.source, self.cost(c), Filter::Any, Dest::Discard)
                    });
                }
            }
            Then::MonasteryCopper => {
                let left = f.max.saturating_sub(f.count);
                if left > 0 && self.players[pi].in_play.has(id::COPPER) {
                    self.stack.push(Frame {
                        depth: f.depth,
                        ..select(p, f.source, Zone::InPlay, Act::Trash, Filter::Card(id::COPPER), 0, left, Then::Nothing)
                    });
                }
            }
            Then::TragicHero => {
                if self.players[pi].hand.total() >= 8 {
                    // Pushed so that the trash comes first, then the gain.
                    self.stack.push(Frame {
                        potion: true, depth: f.depth, ..gain_frame(p, f.source, u8::MAX, self.treasure_filter(), Dest::Discard)
                    });
                    if self.leave_play_self(p, f.source, f.tok) {
                        self.trash_card(p, f.source, true, sink);
                    }
                }
            }
            Then::LeprechaunCheck => {
                if self.players[pi].in_play.total() == 7 {
                    self.gain_side(p, ids::WISH, Dest::Discard, f.source, sink);
                } else {
                    self.stack.push(Frame { min: 1 << p, depth: f.depth, ..Frame::new(K::ReceiveHex, p, f.source) });
                }
            }
            Then::SecretCave => {
                if f.count == 3 {
                    self.push_duration_pending(p, f.source, 1, 0);
                }
            }
            Then::BatExchange => {
                if f.count > 0 && self.supply.get(ids::VAMPIRE) > 0 && self.in_supply(ids::VAMPIRE) && self.self_in_play(p, ids::BAT, f.tok) {
                    self.exchange_in_play(p, ids::BAT, ids::VAMPIRE, f.tok, sink);
                }
            }
            Then::VampireBat => {
                if self.noc.piles[crate::state::NOC_BAT] > 0 && self.self_in_play(p, ids::VAMPIRE, f.tok) {
                    self.exchange_in_play(p, ids::VAMPIRE, ids::BAT, f.tok, sink);
                }
            }
            Then::LostBoon => {
                if f.count > 0 {
                    self.stack.push(Frame { depth: f.depth, ..Frame::new(K::ReceiveBoon, p, ids::LOST_IN_THE_WOODS) });
                }
            }
            Then::CryptSet => {
                if f.count > 0 {
                    self.push_duration_pending(p, f.source, 1, f.count as CardId);
                }
            }
            Then::HoundsToHand => {
                let ps = &mut self.players[pi];
                let n = std::mem::take(&mut ps.hounds);
                ps.hand.add(ids::FAITHFUL_HOUND, n);
                for _ in 0..n {
                    sink.event(Event::Draw { player: p, card: ids::FAITHFUL_HOUND });
                }
            }
            _ => {}
        }
    }

    /// The exchange of the card `from` in play for `to` from its pile: `from` returns to its pile, `to` goes to the discard pile. Not a gain.
    #[inline(never)]
    fn exchange_in_play<S: EventSink>(&mut self, p: u8, from: CardId, to: CardId, tok: u8, sink: &mut S) {
        if !self.leave_play_self(p, from, tok) {
            return;
        }
        let taken = match to {
            ids::BAT => {
                let n = &mut self.noc.piles[crate::state::NOC_BAT];
                *n -= 1;
                true
            }
            _ => self.supply.remove(to),
        };
        if !taken {
            self.players[p as usize].in_play.add(from, 1);
            return;
        }
        match from {
            ids::BAT => self.noc.piles[crate::state::NOC_BAT] += 1,
            _ => self.supply.add(from, 1),
        }
        self.players[p as usize].discard.add(to, 1);
        sink.event(Event::Exchange { player: p, from, to });
    }

    /// The yes of a `YesNo` that belongs to Nocturne (after the card has moved).
    #[inline(never)]
    pub(crate) fn noc_yes<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::HoundAside => {
                let ps = &mut self.players[p as usize];
                if ps.set_aside.remove(ids::FAITHFUL_HOUND) {
                    ps.hounds += 1;
                }
            }
            Then::PixieTrash => {
                // The Pixie is trashed; the Boon (kept in `last`) is received twice.
                self.stack.push(Frame { subject: f.last, count: 2, depth: f.depth, ..Frame::new(K::ReceiveBoon, p, ids::PIXIE) });
            }
            Then::ChangelingSwap => {
                // The gained card went back to its pile; a Changeling comes to the discard pile instead.
                if self.supply.remove(ids::CHANGELING) {
                    self.players[p as usize].discard.add(ids::CHANGELING, 1);
                    sink.event(Event::Exchange { player: p, from: f.subject, to: ids::CHANGELING });
                }
            }
            _ => {}
        }
    }
}
