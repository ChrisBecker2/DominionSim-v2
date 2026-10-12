//! Dark Ages: Shelters, Ruins and Looters, Spoils, the Knights, the "when you trash this" cards, and the cards that gain from
//! and play with the trash. See `docs/dark-ages-plan.md` for the rulings and the design of the mixed piles.

use super::prelude::*;
use crate::state::{GameConfig, TypeSet};

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table). The kingdom cards come first (161-195, the Knights pile marker among them), then the
/// cards no kingdom contains (196-201); the Ruins and Knights, the members of the two mixed piles, are the highest ids
/// (280-295: the Empires cards and the Traveller upgrades sit between; the Events and Landmarks come after them).
pub mod ids {
    use crate::cards::CardId;
    pub const POOR_HOUSE: CardId = 161;
    pub const BEGGAR: CardId = 162;
    pub const SQUIRE: CardId = 163;
    pub const VAGRANT: CardId = 164;
    pub const FORAGER: CardId = 165;
    pub const HERMIT: CardId = 166;
    pub const MARKET_SQUARE: CardId = 167;
    pub const SAGE: CardId = 168;
    pub const STOREROOM: CardId = 169;
    pub const URCHIN: CardId = 170;
    pub const ARMORY: CardId = 171;
    pub const DEATH_CART: CardId = 172;
    pub const FEODUM: CardId = 173;
    pub const FORTRESS: CardId = 174;
    pub const IRONMONGER: CardId = 175;
    pub const MARAUDER: CardId = 176;
    pub const PROCESSION: CardId = 177;
    pub const RATS: CardId = 178;
    pub const SCAVENGER: CardId = 179;
    pub const WANDERING_MINSTREL: CardId = 180;
    pub const BAND_OF_MISFITS: CardId = 181;
    pub const BANDIT_CAMP: CardId = 182;
    pub const CATACOMBS: CardId = 183;
    pub const COUNT: CardId = 184;
    pub const COUNTERFEIT: CardId = 185;
    pub const CULTIST: CardId = 186;
    pub const GRAVEROBBER: CardId = 187;
    pub const JUNK_DEALER: CardId = 188;
    /// The Knights pile (a kingdom card: it stands for the pile of ten different Knights; never in a zone).
    pub const KNIGHTS: CardId = 189;
    pub const MYSTIC: CardId = 190;
    pub const PILLAGE: CardId = 191;
    pub const REBUILD: CardId = 192;
    pub const ROGUE: CardId = 193;
    pub const ALTAR: CardId = 194;
    pub const HUNTING_GROUNDS: CardId = 195;
    // Not in any kingdom.
    pub const HOVEL: CardId = 196;
    pub const NECROPOLIS: CardId = 197;
    pub const OVERGROWN_ESTATE: CardId = 198;
    pub const SPOILS: CardId = 199;
    pub const MADMAN: CardId = 200;
    pub const MERCENARY: CardId = 201;
    /// The Ruins pile (in the Supply with any Looter; never in a zone).
    pub const RUINS: CardId = 354;
    // The Ruins, then the Knights.
    pub const ABANDONED_MINE: CardId = 355;
    pub const RUINED_LIBRARY: CardId = 356;
    pub const RUINED_MARKET: CardId = 357;
    pub const RUINED_VILLAGE: CardId = 358;
    pub const SURVIVORS: CardId = 359;
    pub const DAME_ANNA: CardId = 360;
    pub const DAME_JOSEPHINE: CardId = 361;
    pub const DAME_MOLLY: CardId = 362;
    pub const DAME_NATALIE: CardId = 363;
    pub const DAME_SYLVIA: CardId = 364;
    pub const SIR_BAILEY: CardId = 365;
    pub const SIR_DESTRY: CardId = 366;
    pub const SIR_MARTIN: CardId = 367;
    pub const SIR_MICHAEL: CardId = 368;
    pub const SIR_VANDER: CardId = 369;
}

/// The last kingdom card of the set (ids `FIRST_DA..=LAST_KINGDOM_DA` are kingdom cards).
pub const LAST_KINGDOM_DA: CardId = ids::HUNTING_GROUNDS;

const CARDS: &[(CardId, CardDef)] = &[
    (ids::POOR_HOUSE, da(order_sensitive(c("Poor House", 1, ACTION, 4, 0, 0, 0, 0)))), // -$1 per Treasure in hand: cards drawn first add Treasures
    (ids::BEGGAR, da(has_choice(c("Beggar", 2, ACTION | REACTION, 0, 0, 0, 0, 0)))), // gains 3 Coppers to the hand
    (ids::SQUIRE, da(on_trash(has_choice(c("Squire", 2, ACTION, 1, 0, 0, 0, 0))))), // +2 Actions, +2 Buys or a Silver
    (ids::VAGRANT, da(order_sensitive(c("Vagrant", 2, ACTION, 0, 0, 1, 1, 0)))), // reveals the top card and puts it back or in hand
    (ids::FORAGER, da(order_sensitive(c("Forager", 3, ACTION, 0, 0, 0, 1, 1)))), // what to trash (may be the +Actions card in hand)
    (ids::HERMIT, da(order_sensitive(c("Hermit", 3, ACTION, 0, 0, 0, 0, 0)))), // what to trash (may be the +Actions card in hand)
    (ids::MARKET_SQUARE, da(order_sensitive(c("Market Square", 3, ACTION | REACTION, 0, 0, 1, 1, 1)))), // a cantrip, but kept in the hand it is a reaction to a trash (Moneylender, Remodel): playing it first can lose the Gold
    (ids::SAGE, da(order_sensitive(c("Sage", 3, ACTION, 0, 0, 0, 1, 0)))), // reveals from the deck until a $3 card
    (ids::STOREROOM, da(order_sensitive(c("Storeroom", 3, ACTION, 0, 0, 0, 0, 1)))), // what to discard (may be the +Actions card in hand)
    (ids::URCHIN, da(choice_free(c("Urchin", 3, ACTION | ATTACK, 0, 0, 1, 1, 0)))), // only victims act (trashing it for a Mercenary is a choice of another Attack's play)
    (ids::ARMORY, da(order_sensitive(c("Armory", 4, ACTION, 0, 0, 0, 0, 0)))), // gains onto the deck: a later draw takes it
    (ids::DEATH_CART, da(order_sensitive(c("Death Cart", 4, ACTION | LOOTER, 0, 0, 0, 0, 0)))), // may trash an Action from the hand (may be the +Actions card)
    (ids::FEODUM, da(on_trash(c("Feodum", 4, VICTORY, 0, 0, 0, 0, 0)))), // 1 VP per 3 Silvers (`vp`)
    (ids::FORTRESS, da(on_trash(choice_free(c("Fortress", 4, ACTION, 0, 0, 1, 2, 0))))),
    (ids::IRONMONGER, da(order_sensitive(c("Ironmonger", 4, ACTION, 0, 0, 1, 1, 0)))), // reveals the top card, may discard it
    (ids::MARAUDER, da(has_choice(c("Marauder", 4, ACTION | ATTACK | LOOTER, 0, 0, 0, 0, 0)))), // gains a Spoils
    (ids::PROCESSION, da(order_sensitive(plays(c("Procession", 4, ACTION, 0, 0, 0, 0, 0), 2)))), // plays a card twice and trashes it
    (ids::RATS, da(on_trash(order_sensitive(c("Rats", 4, ACTION, 0, 0, 1, 1, 0))))), // what to trash (may be the +Actions card in hand)
    (ids::SCAVENGER, da(order_sensitive(c("Scavenger", 4, ACTION, 2, 0, 0, 0, 0)))), // changes the deck
    (ids::WANDERING_MINSTREL, da(order_sensitive(c("Wandering Minstrel", 4, ACTION, 0, 0, 1, 2, 0)))), // reveals 3 and puts Actions back
    (ids::BAND_OF_MISFITS, da(order_sensitive(c("Band of Misfits", 5, ACTION | COMMAND, 0, 0, 0, 0, 0)))), // plays an Action from the Supply (may need the +Actions card in hand)
    (ids::BANDIT_CAMP, da(has_choice(c("Bandit Camp", 5, ACTION, 0, 0, 1, 2, 0)))), // gains a Spoils
    (ids::CATACOMBS, da(on_trash(order_sensitive(c("Catacombs", 5, ACTION, 0, 0, 0, 0, 0))))), // looks at the top 3
    (ids::COUNT, da(order_sensitive(c("Count", 5, ACTION, 0, 0, 0, 0, 0)))), // two choices over the hand and deck
    (ids::COUNTERFEIT, da(has_choice(c("Counterfeit", 5, TREASURE, 1, 0, 0, 0, 1)))), // which Treasure to play twice
    (ids::CULTIST, da(on_trash(order_sensitive(c("Cultist", 5, ACTION | ATTACK | LOOTER, 0, 0, 2, 0, 0))))), // may play another Cultist from the hand
    (ids::GRAVEROBBER, da(order_sensitive(c("Graverobber", 5, ACTION, 0, 0, 0, 0, 0)))), // gains from the trash, or trashes an Action from the hand
    (ids::JUNK_DEALER, da(order_sensitive(c("Junk Dealer", 5, ACTION, 1, 0, 1, 1, 0)))), // what to trash (may be the +Actions card in hand)
    // The Knights pile: a kingdom card by name, standing for the pile of the ten Knights below. Never in a zone.
    (ids::KNIGHTS, da(c("Knights", 5, 0, 0, 0, 0, 0, 0))),
    (ids::MYSTIC, da(order_sensitive(c("Mystic", 5, ACTION, 2, 0, 0, 1, 0)))), // names a card and reveals the top card
    (ids::PILLAGE, da(has_choice(c("Pillage", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // trashes itself, gains Spoils, hand-size attack
    (ids::REBUILD, da(order_sensitive(c("Rebuild", 5, ACTION, 0, 0, 0, 1, 0)))), // names a card, reveals until a Victory card
    (ids::ROGUE, da(has_choice(c("Rogue", 5, ACTION | ATTACK, 2, 0, 0, 0, 0)))), // gains from the trash or attacks
    (ids::ALTAR, da(order_sensitive(c("Altar", 6, ACTION, 0, 0, 0, 0, 0)))), // what to trash (may be the +Actions card in hand)
    (ids::HUNTING_GROUNDS, da(on_trash(choice_free(c("Hunting Grounds", 6, ACTION, 0, 0, 4, 0, 0))))),
    // Shelters: no pile, only in the starting decks (replacing the Estates).
    (ids::HOVEL, da(c("Hovel", 1, REACTION | SHELTER, 0, 0, 0, 0, 0))),
    (ids::NECROPOLIS, da(choice_free(c("Necropolis", 1, ACTION | SHELTER, 0, 0, 0, 2, 0)))),
    (ids::OVERGROWN_ESTATE, da(on_trash(c("Overgrown Estate", 1, VICTORY | SHELTER, 0, 0, 0, 0, 0)))),
    // Piles outside the Supply, gained by one or two cards each.
    (ids::SPOILS, da(choice_free(c("Spoils", 0, TREASURE, 3, 0, 0, 0, 0)))), // returns to its pile when played
    (ids::MADMAN, da(order_sensitive(c("Madman", 0, ACTION, 0, 0, 0, 2, 0)))), // draws a card per card in hand
    (ids::MERCENARY, da(order_sensitive(c("Mercenary", 0, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // may trash 2 cards from the hand
    // The Ruins pile, then the five Ruins.
    (ids::RUINS, da(c("Ruins", 0, 0, 0, 0, 0, 0, 0))),
    (ids::ABANDONED_MINE, da(choice_free(c("Abandoned Mine", 0, ACTION | RUINS, 1, 0, 0, 0, 0)))),
    (ids::RUINED_LIBRARY, da(choice_free(c("Ruined Library", 0, ACTION | RUINS, 0, 0, 1, 0, 0)))),
    (ids::RUINED_MARKET, da(choice_free(c("Ruined Market", 0, ACTION | RUINS, 0, 0, 0, 0, 1)))),
    (ids::RUINED_VILLAGE, da(choice_free(c("Ruined Village", 0, ACTION | RUINS, 0, 0, 0, 1, 0)))),
    (ids::SURVIVORS, da(order_sensitive(c("Survivors", 0, ACTION | RUINS, 0, 0, 0, 0, 0)))), // looks at the top 2 and puts them back
    // The ten Knights.
    (ids::DAME_ANNA, da(order_sensitive(c("Dame Anna", 5, KNIGHTS_T, 0, 0, 0, 0, 0)))), // may trash up to 2 cards from the hand
    (ids::DAME_JOSEPHINE, da(choice_free(c("Dame Josephine", 5, KNIGHTS_T | VICTORY, 0, 2, 0, 0, 0)))),
    (ids::DAME_MOLLY, da(choice_free(c("Dame Molly", 5, KNIGHTS_T, 0, 0, 0, 2, 0)))),
    (ids::DAME_NATALIE, da(has_choice(c("Dame Natalie", 5, KNIGHTS_T, 0, 0, 0, 0, 0)))), // may gain a card costing up to $3
    (ids::DAME_SYLVIA, da(choice_free(c("Dame Sylvia", 5, KNIGHTS_T, 2, 0, 0, 0, 0)))),
    (ids::SIR_BAILEY, da(choice_free(c("Sir Bailey", 5, KNIGHTS_T, 0, 0, 1, 1, 0)))),
    (ids::SIR_DESTRY, da(choice_free(c("Sir Destry", 5, KNIGHTS_T, 0, 0, 2, 0, 0)))),
    (ids::SIR_MARTIN, da(choice_free(c("Sir Martin", 4, KNIGHTS_T, 0, 0, 0, 0, 2)))),
    (ids::SIR_MICHAEL, da(choice_free(c("Sir Michael", 5, KNIGHTS_T, 0, 0, 0, 0, 0)))), // discard attack, then the Knights attack
    (ids::SIR_VANDER, da(on_trash(choice_free(c("Sir Vander", 5, KNIGHTS_T, 0, 0, 0, 0, 0))))),
];

/// Action-Attack-Knight.
const KNIGHTS_T: u32 = ACTION | ATTACK | KNIGHT;

/// A Supply pile of different cards, shuffled, of which only the top card is known and gainable: the Ruins and the Knights.
/// Its members are consecutive ids; `copies` of each exist (10 of each Ruins, 1 of each Knight). See the plan, 2.2.
#[derive(Clone, Copy, Debug)]
pub struct MixedPile {
    /// The id that stands for the whole pile (`KNIGHTS`, `RUINS`): never in a zone.
    pub marker: CardId,
    pub first: CardId,
    pub kinds: u8,
    pub copies: u8,
    /// Where the pile's dealt order starts in `GameState::pile_order`.
    pub offset: u8,
}

/// The size of `GameState::pile_order`: 50 Ruins (a 6-player game's pile) then the 10 Knights.
pub const PILE_ORDER_LEN: usize = 60;

pub const RUINS_PILE: MixedPile = MixedPile { marker: ids::RUINS, first: ids::ABANDONED_MINE, kinds: 5, copies: 10, offset: 0 };
pub const KNIGHTS_PILE: MixedPile = MixedPile { marker: ids::KNIGHTS, first: ids::DAME_ANNA, kinds: 10, copies: 1, offset: 50 };

impl MixedPile {
    /// The most cards the pile can hold (all its cards): the length of its slice of `GameState::pile_order`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.kinds as usize * self.copies as usize
    }
    /// The member ids.
    #[inline]
    pub fn members(&self) -> std::ops::Range<CardId> {
        self.first..self.first + self.kinds as CardId
    }
    /// The pile a member belongs to (a card at or above `cards::FIRST_PILE_CARD`).
    #[inline]
    pub fn of_member(c: CardId) -> Option<&'static MixedPile> {
        if c < cards::FIRST_PILE_CARD {
            None
        } else if c < KNIGHTS_PILE.first {
            Some(&RUINS_PILE)
        } else {
            Some(&KNIGHTS_PILE)
        }
    }
    /// The pile a marker stands for.
    #[inline]
    pub fn of_marker(c: CardId) -> Option<&'static MixedPile> {
        if c == ids::KNIGHTS {
            Some(&KNIGHTS_PILE)
        } else if c == ids::RUINS {
            Some(&RUINS_PILE)
        } else {
            None
        }
    }
    /// The pile `c` is the marker or a member of.
    #[inline]
    pub fn of(c: CardId) -> Option<&'static MixedPile> {
        Self::of_marker(c).or_else(|| Self::of_member(c))
    }
}

/// Whether the starting decks hold the Shelters, by the rule of the "Shelter" page: when every kingdom card is from Dark Ages, always; when none
/// is, never; otherwise when one kingdom card picked at random (by hashing `seed`, so the same game always decides the same) is.
pub fn uses_shelters(kingdom: &[CardId], seed: u64) -> bool {
    let mut kingdom_cards = 0u32;
    let mut dark = 0u32;
    for &k in kingdom {
        if cards::is_kingdom(k) {
            kingdom_cards += 1;
            dark += (cards::set_of(k) == CardSet::DarkAges) as u32;
        }
    }
    if dark == 0 {
        return false;
    }
    if dark == kingdom_cards {
        return true;
    }
    // A card at random: splitmix64 of the seed (independent of the stream the game draws from).
    let mut z = seed.wrapping_add(0x5E17_E125_9E37_79B9).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 32;
    let pick = (z % kingdom_cards as u64) as u32;
    kingdom
        .iter()
        .filter(|&&k| cards::is_kingdom(k))
        .nth(pick as usize)
        .is_some_and(|&k| cards::set_of(k) == CardSet::DarkAges)
}

/// What a game with Dark Ages cards starts with (`GameState::new`).
pub(crate) struct Setup {
    pub knights: bool,
    /// The size of the Ruins pile (10 per player after the first), 0 when no Looter is in the game.
    pub ruins: u8,
    pub spoils: u8,
    pub madman: u8,
    pub mercenary: u8,
    pub shelters: bool,
}

/// Set-up for `cfg`: the Knights pile, the Ruins with any Looter (also one that is the Bane or the Ferryman pile's card), the Spoils (Bandit
/// Camp, Marauder, Pillage), Madman (Hermit) and Mercenary (Urchin) piles, and whether the decks hold the Shelters. One pass over the
/// kingdom, nothing allocated: this runs once per simulated game.
pub(crate) fn setup(cfg: &GameConfig, bane: CardId, ferryman: CardId, num_players: usize) -> Setup {
    let mut s = Setup { knights: false, ruins: 0, spoils: 0, madman: 0, mercenary: 0, shelters: false };
    for &k in cfg.kingdom.iter().chain([bane, ferryman].iter().filter(|&&k| k != 0)) {
        if cards::set_of(k) != CardSet::DarkAges {
            continue;
        }
        match k {
            ids::KNIGHTS => s.knights = true,
            ids::BANDIT_CAMP | ids::MARAUDER | ids::PILLAGE => s.spoils = 15,
            ids::HERMIT => s.madman = 10,
            ids::URCHIN => s.mercenary = 10,
            _ => {}
        }
        if cards::is(k, LOOTER) {
            s.ruins = 10 * (num_players as u8 - 1);
        }
    }
    s.shelters = cfg.shelters.unwrap_or_else(|| uses_shelters(&cfg.kingdom, cfg.seed));
    s
}

/// The Dark Ages cards (see the module docs of `sets`).
pub struct DarkAges;

impl SetRules for DarkAges {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Death Cart's Ruins on gaining it. (Hovel's trash on gaining a Victory card is live when the game has Shelters: `gain_live_extra`.)
    const GAIN_HI: u128 = 1u128 << (id::DEATH_CART - 128);
    const ATTACK_REACTIONS: &'static [(CardId, Act, Then)] = &[(id::BEGGAR, Act::Discard, Then::BeggarGain)];

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            id::SQUIRE => &[ModeOpt::Actions(2), ModeOpt::Buys(2), ModeOpt::Gain(id::SILVER, Dest::Discard)],
            id::CATACOMBS => &[ModeOpt::TakeRevealed, ModeOpt::DiscardRevealedDraw(3)],
            // Two separate choices from one table (`Then::ModeWindow`): indices 0-2, then 3-5.
            id::COUNT => &[
                ModeOpt::DiscardFromHand(2),
                ModeOpt::TopdeckFromHand(1),
                ModeOpt::Gain(id::COPPER, Dest::Discard),
                ModeOpt::Coins(3),
                ModeOpt::TrashHand,
                ModeOpt::Gain(id::DUCHY, Dest::Discard),
            ],
            id::GRAVEROBBER => &[
                ModeOpt::GainFromTrashTo(Filter::CostRange(3, 6), Dest::DeckTop),
                ModeOpt::TrashGain { filter: Filter::Types(TypeSet::Action), plus: 3, dest: Dest::Discard },
            ],
            id::HUNTING_GROUNDS => &[ModeOpt::Gain(id::DUCHY, Dest::Discard), ModeOpt::GainMany(id::ESTATE, Dest::Discard, 3)],
            _ => &[],
        }
    }

    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        (card == id::RATS).then_some(20)
    }

    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        match card {
            // 1 VP per 3 Silvers.
            id::FEODUM => all.get(id::SILVER) as i32 / 3,
            _ => cards::def(card).vp as i32,
        }
    }

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        match card {
            id::POOR_HOUSE => {
                // Reveal the hand; -$1 per Treasure in it (a Curse too, under Charlatan), never below $0.
                let hand = g.players[pi].hand;
                let mut treasures = 0u16;
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                    if g.is_treasure(c) {
                        treasures += k as u16;
                    }
                }
                g.turn.coins = g.turn.coins.saturating_sub(treasures);
            }
            id::BEGGAR => {
                for _ in 0..3 {
                    g.gain(p, id::COPPER, Dest::Hand, false, card, sink);
                }
            }
            id::SQUIRE | id::GRAVEROBBER => g.stack.push(mode_frame(p, card, 1)),
            id::VAGRANT => {
                // Reveal the top card; a Curse, Ruins, Shelter or Victory card goes to the hand, anything else back on top.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(Frame {
                    max: 1,
                    then: Then::MoveMatchingToHand(Filter::Types(TypeSet::CurseRuinsShelterVictory)),
                    ..Frame::new(K::RevealTop, p, card)
                });
            }
            id::FORAGER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::ForagerCoins)),
            id::HERMIT => {
                g.stack.push(gain_frame(p, card, 3, Filter::Any, Dest::Discard));
                g.stack.push(select(p, card, Zone::Discard, Trash, Filter::NonTreasure, 0, 1, Then::HermitHand));
            }
            id::SAGE => g.stack.push(Frame {
                filter: Filter::CostAtLeast(3), max: 1, then: Then::KeepMatchingDiscardRest,
                ..Frame::new(K::RevealUntil, p, card)
            }),
            id::STOREROOM => {
                // Discard any number to draw that many; then discard any number for +$1 each.
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::CoinsPerPick(1)));
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::DrawPerPick));
            }
            id::URCHIN => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 4, Then::Nothing) });
                }
            }
            id::ARMORY => g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::DeckTop)),
            id::DEATH_CART => {
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Types(TypeSet::Action), 0, 1, Then::DeathCartHand));
            }
            id::IRONMONGER => g.stack.push(Frame { max: 1, then: Then::IronmongerReveal, ..Frame::new(K::RevealTop, p, card) }),
            id::MARAUDER => {
                g.gain_side(p, id::SPOILS, Dest::Discard, card, sink);
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(gain_frame(v, card, u8::MAX, Filter::Types(TypeSet::Ruins), Dest::Discard));
                }
            }
            id::PROCESSION => g.stack.push(select(
                p, card, Zone::Hand, Play, Filter::Types(TypeSet::ActionNotDuration), 0, 1,
                Then::PlayTrash { gain: true, plus: 1 },
            )),
            id::RATS => {
                // Gain a Rats, then trash a card from the hand other than a Rats.
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::NotCard(id::RATS), 1, 1, Then::Nothing));
                g.stack.push(gain_frame(p, card, u8::MAX, Filter::Card(id::RATS), Dest::Discard));
            }
            id::SCAVENGER => {
                g.stack.push(select(p, card, Zone::Discard, Topdeck, Filter::Any, 1, 1, Then::Nothing));
                g.stack.push(Frame { then: Then::DeckToDiscard, ..Frame::new(K::Ask, p, card) });
            }
            id::WANDERING_MINSTREL => {
                // Reveal the top 3, discard the non-Actions, put the Actions back in any order.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Action, ALL, ALL, Then::Nothing) });
                g.stack.push(Frame {
                    filter: Filter::Action, max: 3, then: Then::DiscardRevealedNotMatching,
                    ..Frame::new(K::RevealTop, p, card)
                });
            }
            id::BAND_OF_MISFITS => {
                // A non-Duration Action in the Supply costing less than this card now; it stays there.
                let cost = g.cost(card);
                if cost > 0 {
                    g.stack.push(select(p, card, Zone::Supply, Play, Filter::ActionCostUpTo(cost - 1), 1, 1, Then::PlayPicked { times: 1 }));
                }
            }
            id::BANDIT_CAMP => {
                g.gain_side(p, id::SPOILS, Dest::Discard, card, sink);
            }
            id::CATACOMBS => {
                g.stack.push(mode_frame(p, card, 1));
                g.stack.push(Frame { max: 3, ..Frame::new(K::RevealTop, p, card) });
            }
            id::COUNT => {
                // Two choices, the first made and done before the second.
                g.stack.push(Frame { then: Then::ModeWindow { first: 3, count: 3 }, ..mode_frame(p, card, 1) });
                g.stack.push(Frame { then: Then::ModeWindow { first: 0, count: 3 }, ..mode_frame(p, card, 1) });
            }
            id::COUNTERFEIT => g.stack.push(select(
                p, card, Zone::Hand, Play, Filter::TreasureNonDuration, 0, 1, Then::PlayTrash { gain: false, plus: 0 },
            )),
            id::CULTIST => {
                // After the Ruins, a Cultist from the hand may be played (no Action used); the +2 Cards come first.
                g.stack.push(select(p, card, Zone::Hand, Play, Filter::Card(id::CULTIST), 0, 1, Then::PlayPicked { times: 1 }));
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(gain_frame(v, card, u8::MAX, Filter::Types(TypeSet::Ruins), Dest::Discard));
                }
            }
            id::JUNK_DEALER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Nothing)),
            id::MYSTIC => g.stack.push(Frame::new(K::Name, p, card)),
            id::PILLAGE => {
                // "Trash this. If you did, ...": a Pillage played from the Supply (Band of Misfits) or already trashed (a second play) does nothing more.
                if !g.turn.prince_play {
                    g.stack.push(select(p, card, Zone::InPlay, Trash, Filter::Card(id::PILLAGE), 1, 1, Then::PillageRest));
                }
            }
            id::REBUILD => g.stack.push(Frame { then: Then::RebuildNamed, ..Frame::new(K::Name, p, card) }),
            id::ROGUE => {
                // A card costing $3 to $6 in the trash: gain one. Otherwise the Knights' attack.
                let any = g.trash.iter().any(|(c, _)| g.filter_ok(p, Filter::CostRange(3, 6), c));
                if any {
                    g.stack.push(select(p, card, Zone::Trash, Act::Gain, Filter::CostRange(3, 6), 1, 1, Then::Nothing));
                } else {
                    let (vs, n) = g.victims(sink);
                    for &v in vs[..n].iter().rev() {
                        g.stack.push(select(v, card, Zone::Revealed, Trash, Filter::CostRange(3, 6), 1, 1, Then::DiscardRevealed));
                        g.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, v, card) });
                    }
                }
            }
            id::ALTAR => {
                g.stack.push(gain_frame(p, card, 5, Filter::Any, Dest::Discard));
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Nothing));
            }
            // The Knights: the card's own ability, then the attack.
            id::DAME_ANNA => {
                knights_attack(g, card, sink);
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 2, Then::Nothing));
            }
            id::DAME_NATALIE => {
                knights_attack(g, card, sink);
                g.stack.push(Frame { optional: true, ..gain_frame(p, card, 3, Filter::Any, Dest::Discard) });
            }
            id::SIR_MICHAEL => {
                knights_attack(g, card, sink);
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::Nothing) });
                }
            }
            id::DAME_JOSEPHINE | id::DAME_MOLLY | id::DAME_SYLVIA | id::SIR_BAILEY | id::SIR_DESTRY | id::SIR_MARTIN | id::SIR_VANDER => {
                knights_attack(g, card, sink)
            }
            id::MADMAN => {
                // Return this to the Madman pile (if it is in play to return); if that worked, +1 Card per card in the hand.
                if !g.turn.prince_play && g.players[pi].in_play.remove(card) {
                    g.madman += 1;
                    sink.event(Event::Return { player: p, card });
                    let n = g.players[pi].hand.total();
                    if n > 0 {
                        g.stack.push(draw_frame(p, card, n.min(u8::MAX as u32) as u8));
                    }
                }
            }
            id::MERCENARY => g.stack.push(Frame {
                exact: true,
                ..select(p, card, Zone::Hand, Trash, Filter::Any, 0, 2, Then::MercenaryBonus)
            }),
            id::SPOILS => {
                // Played by an effect (Counterfeit): back to the Spoils pile.
                if g.players[pi].in_play.remove(card) {
                    g.spoils += 1;
                    sink.event(Event::Return { player: p, card });
                }
            }
            id::SURVIVORS => {
                // Look at the top 2: discard both or put both back in any order.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(Frame { then: Then::DiscardRevealedAsk, ..Frame::new(K::Ask, p, card) });
                g.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, p, card) });
            }
            _ => {}
        }
    }

    /// "When you trash this" (the cards marked `on_trash`), for the player told to trash the card.
    fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, sink: &mut S) {
        let pi = p as usize;
        match card {
            // An Attack card from the Supply (not Mercenary or a Ferryman pile's card).
            id::SQUIRE => g.stack.push(gain_frame(p, card, u8::MAX, Filter::Types(TypeSet::Attack), Dest::Discard)),
            id::FEODUM => {
                for _ in 0..3 {
                    g.gain(p, id::SILVER, Dest::Discard, false, card, sink);
                }
            }
            // Out of the trash into the hand (not a gain); it was trashed all the same.
            id::FORTRESS => {
                if g.trash.remove(card) {
                    g.players[pi].hand.add(card, 1);
                    sink.event(Event::TakeFromTrash { player: p, card });
                }
            }
            id::RATS | id::OVERGROWN_ESTATE => g.stack.push(draw_frame(p, card, 1)),
            id::CULTIST => g.stack.push(draw_frame(p, card, 3)),
            id::CATACOMBS => {
                let cost = g.cost(card);
                g.stack.push(Frame { less: true, ..gain_frame(p, card, cost, Filter::Any, Dest::Discard) });
            }
            id::HUNTING_GROUNDS => g.stack.push(mode_frame(p, card, 1)),
            id::SIR_VANDER => {
                g.gain(p, id::GOLD, Dest::Discard, false, card, sink);
            }
            _ => {}
        }
    }

    /// Death Cart's two Ruins, and Hovel's trash from the hand when a Victory card is gained.
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, _to: Dest, _on_buy: bool, _sink: &mut S) {
        if c == id::DEATH_CART {
            for _ in 0..2 {
                g.stack.push(gain_frame(p, id::DEATH_CART, u8::MAX, Filter::Types(TypeSet::Ruins), Dest::Discard));
            }
        }
        if g.shelters && cards::is(c, VICTORY) {
            for _ in 0..g.players[p as usize].hand.get(id::HOVEL) {
                g.stack.push(Frame { zone: Zone::Hand, act: Act::Trash, subject: id::HOVEL, ..Frame::new(K::YesNo, p, id::HOVEL) });
            }
        }
    }

    /// Hovel's trigger is live in every game with Shelters, and Death Cart's in one where it is the Ferryman pile's card.
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.shelters || g.ferryman == id::DEATH_CART
    }

    /// Hermit: at the end of the Buy phase, a Hermit in play goes back to its pile for a Madman if the player gained no card in the Buy phase.
    fn end_of_buy<S: EventSink>(g: &mut GameState, sink: &mut S) {
        if g.madman == 0 {
            return;
        }
        let p = g.turn.player;
        if g.turn.gained_in_buy == 0 {
            let n = g.players[p as usize].in_play.get(id::HERMIT);
            for _ in 0..n {
                if g.madman == 0 {
                    break;
                }
                g.players[p as usize].in_play.remove(id::HERMIT);
                if g.in_supply(id::HERMIT) {
                    g.supply.add(id::HERMIT, 1);
                } else if g.ferryman == id::HERMIT {
                    g.ferryman_left += 1;
                }
                sink.event(Event::Return { player: p, card: id::HERMIT });
                g.gain_side(p, id::MADMAN, Dest::Discard, id::HERMIT, sink);
            }
        }
    }
}

/// The Knights' attack, for each other player in turn order: reveal the top 2 cards, trash one costing $3 to $6 (their choice, when both do),
/// discard the rest; a Knight trashed that way makes the attacker trash the Knight played (`Then::KnightTrashed`).
fn knights_attack<S: EventSink>(g: &mut GameState, card: CardId, sink: &mut S) {
    let (vs, n) = g.victims(sink);
    for &v in vs[..n].iter().rev() {
        g.stack.push(select(v, card, Zone::Revealed, Act::Trash, Filter::CostRange(3, 6), 1, 1, Then::KnightTrashed));
        g.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, v, card) });
    }
}

// -------------------------------------------------------------------------------------------
// Trashing: the "when you trash this" machinery
// -------------------------------------------------------------------------------------------

impl GameState {
    /// Whether Market Square can be in a player's hand in this game (in the Supply or the Ferryman pile's card).
    #[inline(always)]
    fn market_square_live(&self) -> bool {
        self.in_supply(id::MARKET_SQUARE) || self.ferryman == id::MARKET_SQUARE
    }

    /// The one place a card enters the trash (all the others call it): it goes to the trash pile, and if it has a "when you trash this"
    /// effect, or its owner may react with Market Square, it waits in `trash_queue` until the effect that trashed it has picked everything
    /// (`flush_trash_triggers`). `p` is the player told to trash it; `owned` is false for a card that was not theirs (a Supply card trashed
    /// by Lurker).
    pub(crate) fn trash_card<S: EventSink>(&mut self, p: u8, c: CardId, owned: bool, sink: &mut S) {
        self.trash.add(c, 1);
        sink.event(Event::Trash { player: p, card: c });
        // Tomb (Empires): +1 VP token for each card trashed.
        if self.emp.flags & crate::state::E_TOMB != 0 {
            self.gain_vp_tokens(p, id::TOMB, 1, sink);
        }
        if cards::def(c).on_trash || (owned && self.market_square_live()) {
            if self.trash_queue.len as usize == crate::state::TRASH_QUEUE_CAP {
                self.flush_trash_triggers(0, sink);
            }
            let q = &mut self.trash_queue;
            q.items[q.len as usize] = (p, c, if owned { crate::state::Q_TRASHED_OWN } else { crate::state::Q_TRASHED });
            q.len += 1;
        }
    }

    /// Put what the queued trashed cards do onto the stack: the first card trashed resolves first; a card's own "when you trash this" before
    /// Market Square's offer for it. Every Market Square offer is made again while one is in the hand.
    pub(crate) fn flush_trash_triggers<S: EventSink>(&mut self, depth: u8, sink: &mut S) {
        if self.trash_queue.is_empty() {
            return;
        }
        let q = std::mem::take(&mut self.trash_queue);
        let base = self.stack.len as usize;
        for i in (0..q.len as usize).rev() {
            let (p, c, kind) = q.items[i];
            // A discard that a card reacts to: the reaction is asked now that everything discarded together is in the discard pile.
            if kind == crate::state::Q_DISCARDED {
                self.discard_reaction(p, c);
                continue;
            }
            let owned = kind == crate::state::Q_TRASHED_OWN;
            if owned && self.market_square_live() {
                // Offered when the player holds one, or some trigger of theirs could draw one.
                let could_draw = q.items[..q.len as usize].iter().any(|&(q2, c2, k2)| k2 != crate::state::Q_DISCARDED && q2 == p && cards::def(c2).on_trash);
                if self.players[p as usize].hand.has(id::MARKET_SQUARE) || could_draw {
                    self.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Discard, subject: id::MARKET_SQUARE, then: Then::MarketSquareGold,
                        ..Frame::new(K::YesNo, p, id::MARKET_SQUARE)
                    });
                }
            }
            if cards::def(c).on_trash {
                crate::sets::on_trash(self, p, c, sink);
            }
        }
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            f.depth = depth;
        }
    }

    /// Whether `p`, playing the Attack `attack`, may trash an Urchin in play (and a Mercenary is left to gain): another Urchin than the
    /// one being played (the same Urchin played twice does not count).
    pub(crate) fn urchin_offer_possible(&self, attack: CardId, p: u8) -> bool {
        let needed = 1 + (attack == id::URCHIN && !self.turn.prince_play) as u8;
        self.mercenary > 0 && self.players[p as usize].in_play.get(id::URCHIN) >= needed
    }

    /// The revealed cards (`set_aside`) are discarded.
    pub(crate) fn discard_revealed<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        let rest = ps.set_aside;
        ps.set_aside.clear();
        ps.discard.add_all(&rest);
        for (c, n) in rest.iter() {
            for _ in 0..n {
                sink.event(Event::Discard { player: p, card: c });
            }
        }
        if self.adv & crate::state::G_DISCARD_HOOK != 0 {
            self.discard_hook_counts(p, &rest, 0, sink);
        }
    }

    /// Ironmonger, once the top card is revealed: its types give +1 Action, +$1 and +1 Card (the draw comes after the choice to discard it,
    /// so a Victory card put back is the card drawn).
    pub(crate) fn finish_ironmonger<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let Some((c, _)) = self.players[p as usize].set_aside.iter().next() else { return };
        let actions = cards::is(c, ACTION) as u8;
        let coins = self.is_treasure(c) as u16;
        self.turn.actions += actions;
        self.turn.coins += coins;
        sink.bonus(p, f.source, actions, 0, coins, 0);
        if cards::is(c, VICTORY) {
            self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 1) });
        }
        self.stack.push(Frame {
            depth: f.depth, ordered: true,
            ..select(p, f.source, Zone::Revealed, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing)
        });
        self.stack.push(Frame { depth: f.depth, ..select(p, f.source, Zone::Revealed, Act::Discard, Filter::Any, 0, 1, Then::Nothing) });
    }

    /// Pillage, after the trash of itself: if it was trashed, 2 Spoils, and each other player with 5 or more cards in hand reveals it and
    /// discards a card the attacker chooses.
    pub(crate) fn finish_pillage<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        if f.count == 0 {
            return;
        }
        let p = f.player;
        for _ in 0..2 {
            self.gain_side(p, id::SPOILS, Dest::Discard, f.source, sink);
        }
        let (vs, n) = self.victims(sink);
        for &v in vs[..n].iter().rev() {
            let hand = self.players[v as usize].hand;
            if hand.total() >= 5 {
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        sink.event(Event::Reveal { player: v, card: c });
                    }
                }
            }
            self.stack.push(Frame {
                chooser: p, down_to_target: true, exact: true, depth: f.depth,
                ..select(v, f.source, Zone::Hand, Act::Discard, Filter::Any, 0, 5, Then::Nothing)
            });
        }
    }

    /// The cards Rebuild may name: any card of the game in the Supply (the Knights by name, not the Ruins), and Overgrown Estate with the
    /// Shelters. Naming a Victory card protects it, naming anything else is a way to skip nothing.
    pub(crate) fn rebuild_offer(&self, out: &mut ChoiceBuf) {
        for c in self.supply_cards() {
            if c < cards::FIRST_PILE_CARD || c >= KNIGHTS_PILE.first {
                out.push(Choice::Card(c));
            }
        }
        if self.shelters {
            out.push(Choice::Card(id::OVERGROWN_ESTATE));
        }
    }

    /// An optional effect with no card to act on said Yes.
    pub(crate) fn ask_yes<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            // Scavenger: the whole deck (known top, unknown middle and known bottom) into the discard pile.
            Then::DeckToDiscard => {
                let ps = &mut self.players[p as usize];
                let n = ps.deck_size();
                let deck = ps.deck_counts();
                ps.deck_known.clear();
                ps.deck_known_bottom.clear();
                ps.deck_unknown.clear();
                ps.discard.add_all(&deck);
                sink.event(Event::DeckToDiscard { player: p, n: n as u16 });
            }
            Then::DiscardRevealedAsk => self.discard_revealed(p, sink),
            _ => {}
        }
    }
}

// -------------------------------------------------------------------------------------------
// The mixed piles (Knights, Ruins)
// -------------------------------------------------------------------------------------------

impl GameState {
    /// Put `pile` into the game with `size` cards: its members join the Supply and a top card is drawn with the game's RNG.
    pub(crate) fn init_mixed_pile(&mut self, pile: &MixedPile, size: u8) {
        for m in pile.members() {
            self.in_supply.insert(m);
            self.supply.set(m, 0);
        }
        self.deal_pile(pile, size);
    }

    /// Shuffle `pile` and deal it `size` cards: a top card drawn with the game's RNG, then the face-down part below it. The pile's members
    /// must not be held by anyone yet (the cards come from `pile_unseen`).
    fn deal_pile(&mut self, pile: &MixedPile, size: u8) {
        for m in pile.members() {
            self.supply.set(m, 0);
        }
        let off = pile.offset as usize;
        self.pile_order[off..off + pile.capacity()].fill(0);
        let top = self.draw_unseen(pile).unwrap_or(pile.first);
        self.supply.set(top, size);
        self.deal_face_down(pile);
    }

    /// Deal the face-down part of `pile` again, below its top card, from the cards nobody has seen (`pile_unseen`) with this state's RNG:
    /// the order a pile loaded without one gets, and what a determinization (a state the searching player sees the pile of) does so that
    /// the real order is not read from the state.
    pub fn deal_face_down(&mut self, pile: &MixedPile) {
        let Some(top) = self.pile_top(pile) else { return };
        let size = (self.pile_size(pile) as usize).min(pile.capacity());
        let off = pile.offset as usize;
        let mut pool = self.pile_unseen(pile);
        let mut total: u32 = pool.iter().map(|&n| n as u32).sum();
        self.pile_order[off..off + pile.capacity()].fill(0);
        self.pile_order[off + size - 1] = (top - pile.first) as u8;
        for slot in (0..size - 1).rev() {
            let mut idx = (top - pile.first) as usize;
            if total > 0 {
                let mut k = self.rng.below(total);
                for (i, &n) in pool.iter().enumerate() {
                    if k < n as u32 {
                        idx = i;
                        break;
                    }
                    k -= n as u32;
                }
                pool[idx] -= 1;
                total -= 1;
            }
            self.pile_order[off + slot] = idx as u8;
        }
    }

    /// The pile's cards, top first: the top card is the one in the Supply, the others are as dealt (face down). Returns the cards and how many.
    pub fn pile_cards(&self, pile: &MixedPile) -> ([CardId; 50], usize) {
        let mut out = [0 as CardId; 50];
        let Some(top) = self.pile_top(pile) else { return (out, 0) };
        let size = (self.pile_size(pile) as usize).min(pile.capacity());
        let off = pile.offset as usize;
        out[0] = top;
        for d in 1..size {
            out[d] = pile.first + self.pile_order[off + size - 1 - d] as CardId;
        }
        (out, size)
    }

    /// The copies of each member of `pile` nobody has seen yet, i.e. that are neither in a player's zones, nor in the trash, nor the pile's
    /// top card: what the pile's next unrevealed card is drawn from (public information).
    pub fn pile_unseen(&self, pile: &MixedPile) -> [u8; 10] {
        let mut out = [0u8; 10];
        let mut seen = Counts::EMPTY;
        for p in 0..self.num_players as usize {
            seen.add_all(&self.players[p].all_cards());
        }
        for (i, m) in pile.members().enumerate() {
            // The top card (the member with a nonzero count) is known too.
            let n = seen.get(m) as u32 + self.trash.get(m) as u32 + (self.supply.get(m) > 0) as u32;
            out[i] = (pile.copies as u32).saturating_sub(n) as u8;
        }
        out
    }

    /// Draw the next top card of `pile` uniformly from the cards nobody has seen (`pile_unseen`): the same distribution as shuffling the
    /// whole set of Ruins (or Knights) before the game and dealing the pile from it.
    fn draw_unseen(&mut self, pile: &MixedPile) -> Option<CardId> {
        let unseen = self.pile_unseen(pile);
        let total: u32 = unseen.iter().map(|&n| n as u32).sum();
        if total == 0 {
            return None;
        }
        let mut k = self.rng.below(total);
        for (i, &n) in unseen.iter().enumerate() {
            if k < n as u32 {
                return Some(pile.first + i as CardId);
            }
            k -= n as u32;
        }
        None
    }

    /// Draw again the top card of the Knights and Ruins piles of a game that has not started (nothing gained, nothing seen), keeping their
    /// sizes (and Druid's Boons, Nocturne), with this state's RNG. A set of games that start from one new game (the page's "Simulate from a new game") calls it with each
    /// game's own RNG, so every game has its own pile order, as at a real table, and not the one `new` drew.
    pub fn redeal_mixed_piles(&mut self) {
        for pile in [&RUINS_PILE, &KNIGHTS_PILE] {
            if self.has_pile(pile) {
                let size = self.pile_size(pile);
                self.deal_pile(pile, size);
            }
        }
        // Nocturne: Druid's three Boons are set aside at random too (the rest of the Boons and Hexes are drawn at random when needed).
        self.redeal_druid_boons();
    }

    /// The pile's top card was just taken (gained, or trashed from the Supply) and is now in a zone or the trash: if the pile has cards left,
    /// the next one is revealed.
    pub(crate) fn advance_pile(&mut self, old_top: CardId) {
        let Some(pile) = MixedPile::of_member(old_top) else { return };
        let left = self.supply.get(old_top);
        if left == 0 {
            return;
        }
        self.supply.set(old_top, 0);
        let off = pile.offset as usize;
        let slot = (left as usize).min(pile.capacity());
        // The next stored card is revealed. A state whose top card was forced by hand (a test, a hand-edited save) may store a card that has
        // no unseen copy left; then the next one is drawn from the unseen cards instead.
        let stored = self.pile_order[off + slot - 1] as usize;
        let top = if stored < pile.kinds as usize && self.pile_unseen(pile)[stored] > 0 {
            pile.first + stored as CardId
        } else {
            let t = self.draw_unseen(pile).unwrap_or(old_top);
            self.pile_order[off + slot - 1] = (t - pile.first) as u8;
            t
        };
        if slot < pile.capacity() {
            self.pile_order[off + slot] = 0;
        }
        self.supply.set(top, left);
    }

    /// The top card of `pile` (the one card of it that may be bought or gained), `None` when the pile is empty or not in the game.
    pub fn pile_top(&self, pile: &MixedPile) -> Option<CardId> {
        pile.members().find(|&m| self.supply.get(m) > 0)
    }

    /// The cards left in `pile`.
    pub fn pile_size(&self, pile: &MixedPile) -> u8 {
        pile.members().map(|m| self.supply.get(m)).sum()
    }

    /// Whether `pile` is in the game.
    #[inline]
    pub fn has_pile(&self, pile: &MixedPile) -> bool {
        self.in_supply(pile.first)
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
                "Urchin", "Fortress", "Hunting Grounds", "Necropolis", "Spoils", "Abandoned Mine",
                "Ruined Library", "Ruined Market", "Ruined Village", "Dame Josephine", "Dame Molly", "Dame Sylvia",
                "Sir Bailey", "Sir Destry", "Sir Martin", "Sir Michael", "Sir Vander",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Poor House", "Vagrant", "Forager", "Hermit", "Market Square", "Sage", "Storeroom", "Armory", "Death Cart", "Ironmonger",
                "Procession", "Rats", "Scavenger", "Wandering Minstrel", "Band of Misfits", "Catacombs", "Count",
                "Cultist", "Graverobber", "Junk Dealer", "Mystic", "Rebuild", "Altar", "Madman", "Mercenary",
                "Survivors", "Dame Anna",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
