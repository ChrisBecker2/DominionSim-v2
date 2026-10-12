//! Card identities and the generic shape of a card: type flags, `CardDef`, lookups, the "choose one" mode atoms and kingdom
//! selection. The cards themselves are in `crate::sets`, one module per expansion (Base Set, Intrigue, Seaside, Prosperity,
//! Alchemy, the Prince, Walled Village, Envoy and Captain promos, Cornucopia & Guilds): see `sets/mod.rs`.

use crate::rng::Rng;
use crate::state::{Dest, Filter};

pub type CardId = u16;
/// Number of cards of every set; ids are dense, `0..NUM_CARDS`.
pub use crate::sets::NUM_CARDS;
/// Every card, by id.
pub use crate::sets::CARDS;
/// Reaction cards that block an Attack when revealed (Moat), reactions with an optional effect (Diplomat: `(card,
/// minimum hand size, cards drawn, cards then discarded)`), and cards that make their owner immune to Attacks while in
/// play (Lighthouse): the union of each set's `SetRules` constants.
pub use crate::sets::{ATTACK_REACTIONS, BLOCK_REACTIONS, IMMUNE_IN_PLAY, REACTION_EFFECTS};
// Kingdom set-up that belongs to one expansion.
pub use crate::sets::alchemy::add_potion_if_needed;
pub use crate::sets::cornucopia_guilds::{default_bane, default_ferryman};

// Type flags.
pub const ACTION: u32 = 1;
pub const TREASURE: u32 = 2;
pub const VICTORY: u32 = 4;
pub const CURSE_T: u32 = 8;
pub const ATTACK: u32 = 16;
pub const REACTION: u32 = 32;
/// Stays in play with effects on a later turn (Seaside).
pub const DURATION: u32 = 64;
/// The six non-Supply cards Joust gains (Cornucopia & Guilds).
pub const REWARD: u32 = 128;
/// Prince's other type: it only stops Command cards from playing each other, and no Command card
/// exists here besides Prince itself, so it is a flag with no rules of its own.
pub const COMMAND: u32 = 256;
/// The five Ruins (Dark Ages): Actions in the Ruins pile.
pub const RUINS: u32 = 512;
/// The ten Knights (Dark Ages).
pub const KNIGHT: u32 = 1024;
/// Cultist, Death Cart and Marauder (Dark Ages): any in the kingdom brings the Ruins pile.
pub const LOOTER: u32 = 2048;
/// Hovel, Necropolis and Overgrown Estate (Dark Ages): the starting-deck cards that replace the Estates.
pub const SHELTER: u32 = 4096;
/// Page, Peasant and the Traveller upgrades that can be exchanged for the next card of their line (Adventures).
pub const TRAVELLER: u32 = 8192;
/// Cards that go onto the Tavern mat when played (Adventures). (`types` is a `u32` since Nocturne.)
pub const RESERVE: u32 = 16384;
/// The eight Castles (Empires): Victory cards in the Castles pile (a fixed order, only the top one can be gained).
pub const CASTLE: u32 = 1 << 15;
/// Night cards (Nocturne): played in the Night phase, after the Buy phase.
pub const NIGHT: u32 = 1 << 16;
/// Fate cards (Nocturne): in a game with one, the Boons are shuffled.
pub const FATE: u32 = 1 << 17;
/// Doom cards (Nocturne): in a game with one, the Hexes are shuffled.
pub const DOOM: u32 = 1 << 18;
/// Spirits (Nocturne): Will-o'-Wisp, Imp and Ghost, the cards Exorcist gains.
pub const SPIRIT: u32 = 1 << 19;
/// The three Zombies (Nocturne): they start in the trash when Necromancer is in the game.
pub const ZOMBIE: u32 = 1 << 20;
/// Heirlooms (Nocturne): they replace a starting Copper when their kingdom card is in the game.
pub const HEIRLOOM: u32 = 1 << 21;

/// Card ids by name. The constants are defined next to their cards, in each set's `ids` module.
pub mod id {
    pub use crate::sets::adventures::ids::*;
    pub use crate::sets::alchemy::ids::*;
    pub use crate::sets::base::ids::*;
    pub use crate::sets::cornucopia_guilds::ids::*;
    pub use crate::sets::dark_ages::ids::*;
    pub use crate::sets::empires::ids::*;
    pub use crate::sets::intrigue::ids::*;
    pub use crate::sets::nocturne::ids::*;
    pub use crate::sets::promo::ids::*;
    pub use crate::sets::prosperity::ids::*;
    pub use crate::sets::rising_sun::ids::*;
    pub use crate::sets::seaside::ids::*;
}

/// The first of the Events (pseudo-cards: ids for things that are bought but never gained, see `sets::adventures`). The Events are the 20
/// of Adventures, then the 13 of Empires, and every id from here up (Events, then the Landmarks) is a pseudo-card: never in a zone, a pile
/// or a supply mask, so `Counts` needs no lane for it.
pub const FIRST_EVENT: CardId = id::ALMS;
/// The number of Adventures Events.
pub const NUM_ADV_EVENTS: usize = 20;
/// The number of Events (Adventures, then Empires): `events` masks are indexed by `event - FIRST_EVENT`.
pub const NUM_EVENTS: usize = 33;
/// The first of the 21 Landmarks (Empires), after the Events.
pub const FIRST_LANDMARK: CardId = id::AQUEDUCT;
pub const NUM_LANDMARKS: usize = 21;
/// Ids below this can be in a zone (a hand, a pile, the trash); the pseudo-cards (Events, Landmarks) are above it. `counts::LANES` covers them.
pub const NUM_ZONE_IDS: usize = FIRST_EVENT as usize;
/// The first of the eight Traveller upgrade cards (Treasure Hunter, Warrior, Hero, Champion, Soldier, Fugitive, Disciple, Teacher): non-Supply
/// piles of five, in ids `FIRST_TRAV..FIRST_TRAV + NUM_TRAV`.
pub const FIRST_TRAV: CardId = id::TREASURE_HUNTER;
pub const NUM_TRAV: usize = 8;

/// What a Traveller is exchanged for when it is discarded from play (`None` for the end of a line: Champion and Teacher are not Travellers).
pub fn traveller_upgrade(card: CardId) -> Option<CardId> {
    match card {
        id::PAGE => Some(id::TREASURE_HUNTER),
        id::TREASURE_HUNTER => Some(id::WARRIOR),
        id::WARRIOR => Some(id::HERO),
        id::HERO => Some(id::CHAMPION),
        id::PEASANT => Some(id::SOLDIER),
        id::SOLDIER => Some(id::FUGITIVE),
        id::FUGITIVE => Some(id::DISCIPLE),
        id::DISCIPLE => Some(id::TEACHER),
        _ => None,
    }
}

/// Whether `card` is one of the Adventures Events (not a card at all: never in a zone or a pile; bought for its cost in the Buy phase).
#[inline(always)]
pub fn is_event(card: CardId) -> bool {
    card.wrapping_sub(FIRST_EVENT) < NUM_EVENTS as CardId
}

/// The first of the 12 Boons (pseudo-cards: ids for the cards of the Boons deck, never in a zone), then the 12 Hexes and the 5 States.
pub const FIRST_BOON: CardId = id::THE_EARTHS_GIFT;
pub const NUM_BOONS: usize = 12;
pub const FIRST_HEX: CardId = id::BAD_OMENS;
pub const NUM_HEXES: usize = 12;
pub const FIRST_STATE: CardId = id::LOST_IN_THE_WOODS;
pub const NUM_STATES: usize = 5;

/// Whether `card` is one of the 12 Boons.
#[inline(always)]
pub fn is_boon(card: CardId) -> bool {
    card.wrapping_sub(FIRST_BOON) < NUM_BOONS as CardId
}

/// Whether `card` is one of the 12 Hexes.
#[inline(always)]
pub fn is_hex(card: CardId) -> bool {
    card.wrapping_sub(FIRST_HEX) < NUM_HEXES as CardId
}

/// Whether `card` is one of the States (Lost in the Woods, Deluded, Envious, Miserable, Twice Miserable).
#[inline(always)]
pub fn is_state(card: CardId) -> bool {
    card.wrapping_sub(FIRST_STATE) < NUM_STATES as CardId
}

/// The Heirloom a Nocturne kingdom card comes with (it replaces a starting Copper), 0 for any other card.
pub fn heirloom_of(card: CardId) -> CardId {
    match card {
        id::CEMETERY => id::HAUNTED_MIRROR,
        id::SECRET_CAVE => id::MAGIC_LAMP,
        id::PIXIE => id::GOAT,
        id::SHEPHERD => id::PASTURE,
        id::TRACKER => id::POUCH,
        id::POOKA => id::CURSED_GOLD,
        id::FOOL => id::LUCKY_COIN,
        _ => 0,
    }
}

/// The Gathering cards (Empires): Farmers' Market, Temple and Wild Hunt collect VP tokens on their piles. (Not a type bit: the type flags
/// are all used.)
#[inline(always)]
pub fn is_gathering(card: CardId) -> bool {
    matches!(card, id::FARMERS_MARKET | id::TEMPLE | id::WILD_HUNT)
}

/// Whether `card` is one of the Empires Landmarks (not a card at all: never bought or gained; it changes the scoring).
#[inline(always)]
pub fn is_landmark(card: CardId) -> bool {
    card.wrapping_sub(FIRST_LANDMARK) < NUM_LANDMARKS as CardId
}

/// Whether `card` is a pseudo-card (an Event or a Landmark): it has an id, a name and a cost but never sits in a zone.
#[inline(always)]
pub fn is_pseudo(card: CardId) -> bool {
    card >= FIRST_EVENT
}

/// The bottom halves of the split piles (ids `AVANTO..AVANTO + 6`: Avanto, Plunder, Emporium, Bustling Village, Rocks, Fortune): in the
/// Supply with their own count, but only gainable once the top half is gone.
#[inline(always)]
pub fn is_split_bottom(card: CardId) -> bool {
    card.wrapping_sub(id::AVANTO) < 6
}

/// The top half of the split pile whose bottom half is `bottom`.
#[inline]
pub fn split_top(bottom: CardId) -> Option<CardId> {
    const TOPS: [CardId; 6] = [id::SAUNA, id::ENCAMPMENT, id::PATRICIAN, id::SETTLERS, id::CATAPULT, id::GLADIATOR];
    is_split_bottom(bottom).then(|| TOPS[(bottom - id::AVANTO) as usize])
}

/// The bottom half of the split pile whose top half is `top`.
#[inline]
pub fn split_bottom(top: CardId) -> Option<CardId> {
    match top {
        id::SAUNA => Some(id::AVANTO),
        id::ENCAMPMENT => Some(id::PLUNDER),
        id::PATRICIAN => Some(id::EMPORIUM),
        id::SETTLERS => Some(id::BUSTLING_VILLAGE),
        id::CATAPULT => Some(id::ROCKS),
        id::GLADIATOR => Some(id::FORTUNE),
        _ => None,
    }
}

/// The eight Castles (ids `HUMBLE_CASTLE..HUMBLE_CASTLE + 8`, in cost order).
#[inline(always)]
pub fn is_castle(card: CardId) -> bool {
    card.wrapping_sub(id::HUMBLE_CASTLE) < 8
}

/// The pile a card belongs to for the rules about piles (tokens, Ferry): the card itself, the Knights or Ruins marker for a member of
/// those mixed piles, and Sauna for Avanto (the split pile).
#[inline]
pub fn pile_of(card: CardId) -> CardId {
    if card >= FIRST_PILE_CARD {
        match crate::sets::dark_ages::MixedPile::of_member(card) {
            Some(pile) => pile.marker,
            None => card,
        }
    } else if is_split_bottom(card) {
        split_top(card).unwrap_or(card)
    } else if is_castle(card) {
        id::CASTLES
    } else {
        card
    }
}

/// The first of the six Rewards (card ids are contiguous: `FIRST_REWARD..FIRST_REWARD + 6`).
pub const FIRST_REWARD: CardId = id::CORONET;
/// The number of different Rewards.
pub const NUM_REWARDS: usize = 6;

/// Whether `card` is one of the six Rewards (non-Supply cards only Joust gains).
#[inline(always)]
pub fn is_reward(card: CardId) -> bool {
    card.wrapping_sub(FIRST_REWARD) < NUM_REWARDS as CardId
}

/// Whether buying `card` may be overpaid for (Cornucopia & Guilds: a "+" in the cost): the
/// overpay ability happens when it is gained (see `SetRules::overpay`).
#[inline]
pub fn has_overpay(card: CardId) -> bool {
    crate::sets::has_overpay(card)
}

/// The expansion a card comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardSet {
    /// Base Set 2nd edition, including the basic Treasure/Victory/Curse cards.
    Base,
    /// Intrigue 2nd edition.
    Intrigue,
    /// Seaside 2nd edition.
    Seaside,
    /// Prosperity 2nd edition (including Platinum and Colony).
    Prosperity,
    /// Alchemy (Potion costs). Possession is not implemented.
    Alchemy,
    /// Promo cards (Prince, Walled Village, Envoy, Captain).
    Promo,
    /// Cornucopia & Guilds 2nd edition (Coffers, Overpay, the Rewards).
    CornucopiaGuilds,
    /// Dark Ages (Shelters, Ruins, Knights, Spoils, trash-for-benefit).
    DarkAges,
    /// Adventures (the Tavern mat and Reserve cards, Travellers, Events, tokens).
    Adventures,
    /// Empires (Debt, split piles and the Castles, Gathering cards, Events, Landmarks).
    Empires,
    /// Rising Sun: only Change, the card that needs Debt (see `sets::rising_sun`).
    RisingSun,
    /// Nocturne (the Night phase, Boons, Hexes and States, Heirlooms, the Spirits and the Zombies).
    Nocturne,
}

impl CardSet {
    pub const ALL: [CardSet; 12] = [
        CardSet::Base, CardSet::Intrigue, CardSet::Seaside, CardSet::Prosperity, CardSet::Alchemy, CardSet::Promo,
        CardSet::CornucopiaGuilds, CardSet::DarkAges, CardSet::Adventures, CardSet::Empires, CardSet::RisingSun, CardSet::Nocturne,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CardSet::Base => "Base",
            CardSet::Intrigue => "Intrigue",
            CardSet::Seaside => "Seaside",
            CardSet::Prosperity => "Prosperity",
            CardSet::Alchemy => "Alchemy",
            CardSet::Promo => "Promo",
            CardSet::CornucopiaGuilds => "Cornucopia & Guilds",
            CardSet::DarkAges => "Dark Ages",
            CardSet::Adventures => "Adventures",
            CardSet::Empires => "Empires",
            CardSet::RisingSun => "Rising Sun",
            CardSet::Nocturne => "Nocturne",
        }
    }
}

/// Static card data. "Vanilla" bonuses (+cards/+actions/+buys/+coins) are applied
/// generically when the card is played; anything else is the card's set module's (`sets/`).
#[derive(Clone, Copy, Debug)]
pub struct CardDef {
    pub name: &'static str,
    pub cost: u8,
    /// Whether the cost includes a Potion (printed as "$2P").
    pub potion: bool,
    /// Potions produced when played as a Treasure (Potion: 1).
    pub potions: u8,
    pub types: u32,
    /// Treasure value when played as a treasure, or +$ when played as an action.
    pub coins: u8,
    pub vp: i8,
    pub cards: u8,
    pub actions: u8,
    pub buys: u8,
    /// Plays another Action card this many times (Throne Room: 2); 0 if it doesn't.
    pub plays: u8,
    /// Whether playing it involves a choice for its player (see [`OnPlay`]). Every Action card
    /// must be marked explicitly with `choice_free(..)`, `has_choice(..)` or `order_sensitive(..)`.
    pub on_play: OnPlay,
    pub set: CardSet,
    /// Whether the card's effects are implemented. Cards that aren't can't be put in a kingdom
    /// (they would silently play as plain cards); see `intrigue_todo`.
    pub ready: bool,
    /// Whether the card does something "when you trash this" (`SetRules::on_trash`): the other cards' trash costs nothing extra.
    pub on_trash: bool,
    /// The Debt in the card's cost (Empires): buying it takes this many Debt tokens. 0 for nearly every card.
    pub debt: u8,
}

/// What playing an Action card means for its player's choice of play order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnPlay {
    /// Not an Action card.
    NotAction,
    /// Only vanilla bonuses for its player (+Cards / +Actions / +Buys / +$), plus effects on
    /// other players (Militia, Witch, Council Room) or a bonus that doesn't depend on the order
    /// actions are played in (Merchant's +$1 on the first Silver). Its player decides nothing and
    /// nothing is gained to, revealed from or put on their deck. When every action in hand is
    /// choice-free and all of them can be played, the order is obvious: +Actions first, then
    /// drawing terminals, then the rest (`best_obvious_play`).
    ChoiceFree,
    /// Its player makes a decision (what to trash, gain, discard, set aside or play), or it
    /// changes that player's own deck or discard in a way a later draw can see (Bandit,
    /// Courtyard). Play order is a real choice: bots search it. Playing a choice-free +Actions
    /// card (Village, Laboratory, Market...) before it never makes it worse: more cards in hand
    /// only add options, so bots play those first without searching (`cantrip_first`).
    Choice,
    /// Like `Choice` (or choice-free in itself), but a +Actions card played before it can make it
    /// worse, so cantrips are not played first while it's in hand: it multiplies a card (Throne
    /// Room), depends on hand size or the Actions in hand (Library, Watchtower, Diplomat, Shanty
    /// Town, Minion), puts, reveals or leaves known cards on top of the deck for a later draw
    /// (Harbinger, Bureaucrat, Artisan, Courtyard, Secret Passage, Sentry, Patrol, Lookout,
    /// Treasure Map, Replace, Apothecary, Scrying Pool, Wishing Well, Sea Chart, Native Village),
    /// plays from the deck top (Vassal, Golem), or picks any card from its
    /// player's hand to trash, discard, set aside, pass or reveal, since the +Actions card itself
    /// may be the best pick (Remodel a Festival into a Gold, Warehouse discarding a Sailor;
    /// Chapel, Upgrade, Expand, Forge, Trading Post, Salvager, Bishop, Apprentice, Transmute,
    /// Steward, Masquerade, Haven, Island, Courtier, Cellar, Vault, Mill, Poacher). Cards that
    /// only pick Treasures or a named card (Mine, Moneylender, Mint, Baron), or discard the whole
    /// hand (Tactician), don't. Rule of thumb: anything that looks at or puts cards on its player's own deck, or
    /// could take an Action from their hand, belongs here.
    OrderSensitive,
}

pub(crate) const fn c(name: &'static str, cost: u8, types: u32, coins: u8, vp: i8, cards: u8, actions: u8, buys: u8) -> CardDef {
    CardDef { name, cost, potion: false, potions: 0, types, coins, vp, cards, actions, buys, plays: 0, on_play: OnPlay::NotAction, set: CardSet::Base, ready: true, on_trash: false, debt: 0 }
}

/// A Seaside card.
pub(crate) const fn seaside(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Seaside, ..d }
}

/// A Prosperity card.
pub(crate) const fn prosperity(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Prosperity, ..d }
}

/// An Intrigue card.
pub(crate) const fn intrigue(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Intrigue, ..d }
}

/// An Alchemy card.
pub(crate) const fn alchemy(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Alchemy, ..d }
}

/// A Promo card.
pub(crate) const fn promo(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Promo, ..d }
}

/// A Cornucopia & Guilds card.
pub(crate) const fn cg(d: CardDef) -> CardDef {
    CardDef { set: CardSet::CornucopiaGuilds, ..d }
}

/// A Dark Ages card.
pub(crate) const fn da(d: CardDef) -> CardDef {
    CardDef { set: CardSet::DarkAges, ..d }
}

/// An Adventures card.
pub(crate) const fn adv(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Adventures, ..d }
}

/// The card has a "when you trash this" effect.
pub(crate) const fn on_trash(d: CardDef) -> CardDef {
    CardDef { on_trash: true, ..d }
}

/// An Empires card.
pub(crate) const fn emp(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Empires, ..d }
}

/// A Nocturne card.
pub(crate) const fn noc(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Nocturne, ..d }
}

/// A Rising Sun card.
pub(crate) const fn rs(d: CardDef) -> CardDef {
    CardDef { set: CardSet::RisingSun, ..d }
}

/// The card's cost includes `n` Debt (buying it takes the tokens).
pub(crate) const fn with_debt(d: CardDef, n: u8) -> CardDef {
    CardDef { debt: n, ..d }
}

/// Not implemented yet: can't be put in a kingdom.
#[allow(dead_code)]
pub(crate) const fn todo(d: CardDef) -> CardDef {
    CardDef { ready: false, ..d }
}

/// The card's cost includes a Potion.
pub(crate) const fn with_p(d: CardDef) -> CardDef {
    CardDef { potion: true, ..d }
}

/// Playing it as a Treasure gives one Potion.
pub(crate) const fn potion_card(d: CardDef) -> CardDef {
    CardDef { potions: 1, ..d }
}

pub(crate) const fn plays(d: CardDef, times: u8) -> CardDef {
    CardDef { plays: times, ..d }
}

/// Marks an Action card as [`OnPlay::ChoiceFree`].
pub(crate) const fn choice_free(d: CardDef) -> CardDef {
    CardDef { on_play: OnPlay::ChoiceFree, ..d }
}

/// Marks an Action card as [`OnPlay::Choice`].
pub(crate) const fn has_choice(d: CardDef) -> CardDef {
    CardDef { on_play: OnPlay::Choice, ..d }
}

/// Marks an Action card as [`OnPlay::OrderSensitive`].
pub(crate) const fn order_sensitive(d: CardDef) -> CardDef {
    CardDef { on_play: OnPlay::OrderSensitive, ..d }
}

/// Whether `card` is an [`OnPlay::ChoiceFree`] action.
#[inline(always)]
pub fn is_choice_free(card: CardId) -> bool {
    CARDS[card as usize].on_play == OnPlay::ChoiceFree
}

/// Whether `card`'s bonus depends on how many supply piles are empty (City). It is choice-free,
/// but a card played before it that empties a pile changes it: a Curse-giving Attack (Witch,
/// Sea Witch, Familiar, Charlatan) or a card that gains. Bots check for that before taking a
/// play-order shortcut (`sim::strategy::empty_pile_risk`) instead of always searching City hands.
#[inline]
pub fn reads_empty_piles(card: CardId) -> bool {
    crate::sets::reads_empty_piles(card)
}

/// Whether `card` is an [`OnPlay::OrderSensitive`] action: while it's in hand, choice-free
/// +Actions cards are not simply played first.
#[inline]
pub fn is_order_sensitive(card: CardId) -> bool {
    CARDS[card as usize].on_play == OnPlay::OrderSensitive
}

#[inline(always)]
pub fn def(card: CardId) -> &'static CardDef {
    &CARDS[card as usize]
}
#[inline(always)]
pub fn cost(card: CardId) -> u8 {
    CARDS[card as usize].cost
}
/// Whether the card's cost includes a Potion.
#[inline(always)]
pub fn potion_cost(card: CardId) -> bool {
    CARDS[card as usize].potion
}
/// "$2P" / "$4": the cost as printed on the card.
/// Printed cost: "$3", "$2P", and "$2+" for a card you may overpay for (Stonemason, Herald).
pub fn cost_string(card: CardId) -> String {
    let debt = debt_cost(card);
    if debt > 0 && cost(card) == 0 {
        return format!("{debt} Debt");
    }
    format!(
        "${}{}{}{}",
        cost(card),
        if has_overpay(card) { "+" } else { "" },
        if potion_cost(card) { "P" } else { "" },
        if debt > 0 { format!(" and {debt} Debt") } else { String::new() }
    )
}
#[inline(always)]
pub fn is(card: CardId, flag: u32) -> bool {
    CARDS[card as usize].types & flag != 0
}
/// The Debt in the card's cost (Empires), 0 for a card without one.
#[inline(always)]
pub fn debt_cost(card: CardId) -> u8 {
    CARDS[card as usize].debt
}
#[inline(always)]
pub fn name(card: CardId) -> &'static str {
    CARDS[card as usize].name
}

/// The split piles: (top half, bottom half).
pub const SPLIT_PILES: [(CardId, CardId); 6] = [
    (id::SAUNA, id::AVANTO),
    (id::ENCAMPMENT, id::PLUNDER),
    (id::PATRICIAN, id::EMPORIUM),
    (id::SETTLERS, id::BUSTLING_VILLAGE),
    (id::CATAPULT, id::ROCKS),
    (id::GLADIATOR, id::FORTUNE),
];

/// Case-insensitive lookup; ignores spaces, so "throneroom" and "Throne Room" both work.
pub fn by_name(s: &str) -> Option<CardId> {
    // Ignores spaces, '_', '-' and apostrophes: "kingscourt", "King's Court", "workers_village".
    let norm = |x: &str| -> String {
        x.chars().filter(|c| !c.is_whitespace() && !matches!(c, '_' | '-' | '\'' | '\u{2019}')).flat_map(|c| c.to_lowercase()).collect()
    };
    let want = norm(s);
    if want.is_empty() {
        return None;
    }
    // The split pile by its name: the card that stands for it in a kingdom (the strategy rules expand it to both halves).
    for (top, bottom) in SPLIT_PILES {
        if want == format!("{}/{}", norm(name(top)), norm(name(bottom))) {
            return Some(top);
        }
    }
    (0..NUM_CARDS as CardId).find(|&c| norm(name(c)) == want)
}

// ---------------------------------------------------------------------------------------
// Mode decisions ("choose one" / "choose N different"): a small set of generic effect
// atoms, as data. A card's `modes()` table is the ordered list of options it offers;
// `Choice::Mode(i)` picks table entry `i`. See `state::FrameKind::Mode` for resolution.
// ---------------------------------------------------------------------------------------

/// A generic "choose one/many" effect atom. Every Intrigue mode card (Pawn, Steward, Nobles,
/// Minion, Courtier, Lurker, and Torturer's victim choice) is expressed as a table of these;
/// nothing card-specific appears outside this module and `effects.rs`'s generic `Mode` frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModeOpt {
    Cards(u8),
    Actions(u8),
    Buys(u8),
    Coins(u8),
    /// Trash up to N cards from hand (as many as you can): forced up to what's available.
    TrashFromHand(u8),
    /// Discard N cards from hand (as many as you can): forced up to what's available.
    DiscardFromHand(u8),
    /// Gain a specific card from the Supply, if any is left (Courtier's Gold, Torturer's Curse).
    Gain(CardId, Dest),
    /// Minion's 2nd option: discard your hand, +`draw` Cards, and each other player (Moat
    /// allowing) with at least `attack_min_hand` cards in hand does the same.
    DiscardHandDraw { draw: u8, attack_min_hand: u8 },
    /// Lurker: trash a card matching `Filter` from the Supply, if any.
    TrashFromSupply(Filter),
    /// Lurker: gain a card matching `Filter` from the trash, if any.
    GainFromTrash(Filter),
    /// Investment's 2nd option: trash the card that offered this choice (already in play), then
    /// reveal the hand for +1 VP token per differently-named Treasure in it.
    TrashSelfRevealVpPerTreasureType,
    /// Native Village's 1st option: put the top card of the deck face down on the private mat
    /// (chance-aware: pushes `state::FrameKind::NativeVillageAdd`, unlike every other atom here).
    NativeVillageAdd,
    /// Native Village's 2nd option: put every card from the mat into hand (immediate).
    NativeVillageTake,
    /// Jester: gain a copy of the frame's `subject` for the victim (`to_chooser: false`) or for the
    /// player who chooses (the attacker).
    GainSubject { to_chooser: bool },
    /// Courser: gain `n` copies of a card, one gain at a time (frames, so that the draw an earlier option
    /// pushed resolves first: "do the options in the order listed").
    GainMany(CardId, Dest, u8),
    /// Count: put a card from the hand onto the deck (as many as you can: forced when the hand has one).
    TopdeckFromHand(u8),
    /// Count: trash the whole hand.
    TrashHand,
    /// Graverobber's second option: trash a card matching `filter` from the hand, then gain a card costing up to `plus` more.
    TrashGain { filter: Filter, plus: u8, dest: Dest },
    /// Gain a card matching `Filter` from the trash to `Dest` (Graverobber: onto the deck).
    GainFromTrashTo(Filter, Dest),
    /// Catacombs' first option: put the revealed cards into the hand.
    TakeRevealed,
    /// Catacombs' second option: discard the revealed cards and draw `n` cards.
    DiscardRevealedDraw(u8),
    /// Miser's first option: put a Copper from the hand onto the Tavern mat.
    TavernCopper,
    /// Miser's second option: +$1 per Copper on the Tavern mat.
    CoinsPerTavernCopper,
    /// Teacher: move the player's token (`TOK_*`) to an Action Supply pile they have no token on.
    MoveToken(u8),
    /// Quest: discard `n` cards matching `filter` (as many as there are, up to `n`); if exactly `n` were discarded, gain `gain`.
    DiscardFor { filter: Filter, n: u8, gain: CardId },
    /// Quest: do none of the options.
    Nothing,
    /// Wild Hunt: `+n Cards` and add `v` VP tokens to the card's pile.
    CardsGather(u8, u8),
    /// Wild Hunt: gain a copy of the card; if it was gained, take the VP tokens on the source card's pile.
    GainTakeVp(CardId),
    /// Charm: `+b Buys` and `+$c`.
    BuysCoins(u8, u8),
    /// Charm: the next time a card is gained this turn, a differently named card costing the same may be gained too.
    NextGainDouble,
}

/// The names of the tokens, indexed like `state::TOK_*` (the six pile tokens), then the Estate token, the -1 Card token, the -$1 token and the
/// Journey token.
pub const TOKEN_NAMES: [&str; 10] = ["+1 Card", "+1 Action", "+1 Buy", "+$1", "-$2 cost", "Trashing", "Estate", "-1 Card", "-$1", "Journey"];

/// The index of the token called `s` (case-insensitive; `+1 coin`, `+$1`, `-$2`, the Unicode minus and `-$1 coin` also work).
pub fn token_by_name(s: &str) -> Option<usize> {
    let norm: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == '\u{2212}' || c == '\u{2013}' { '-' } else { c })
        .flat_map(|c| c.to_lowercase())
        .collect();
    let norm = norm.trim_end_matches("token");
    match norm {
        "+1card" => Some(0),
        "+1action" => Some(1),
        "+1buy" => Some(2),
        "+$1" | "+1coin" | "+1$" => Some(3),
        "-$2cost" | "-$2" | "-2cost" | "-$2coin" => Some(4),
        "trashing" => Some(5),
        "estate" => Some(6),
        "-1card" => Some(7),
        "-$1" | "-1coin" | "-$1coin" => Some(8),
        "journey" => Some(9),
        _ => None,
    }
}

/// Article + noun for a filter, e.g. `("an", "Action")`, used by [`ModeOpt::label`].
fn filter_words(f: Filter) -> (&'static str, &'static str) {
    match f {
        Filter::Any => ("a", "card"),
        Filter::Action => ("an", "Action"),
        Filter::Treasure => ("a", "Treasure"),
        Filter::Victory => ("a", "Victory card"),
        Filter::NonCopperTreasure => ("a", "Treasure other than Copper"),
        Filter::Card(c) => (if starts_with_vowel_sound(name(c)) { "an" } else { "a" }, name(c)),
        Filter::VictoryOrCurse => ("a", "Victory card or Curse"),
        Filter::ActionOrTreasure => ("an", "Action or Treasure card"),
        Filter::TreasureOrCurse => ("a", "Treasure"),
        Filter::Either(..) => ("a", "card"),
        Filter::NonAction => ("a", "non-Action card"),
        Filter::ActionNot(_) => ("an", "Action"),
        Filter::ActionCostUpTo(_) | Filter::NotInPlayAction => ("an", "Action"),
        Filter::NotCard(_) | Filter::NotInHand => ("a", "card"),
        Filter::ActionNotReward => ("an", "Action other than a Reward"),
        Filter::TreasureNotReward => ("a", "Treasure other than a Reward"),
        Filter::TreasureToDiscard => ("a", "Treasure"),
        Filter::Types(crate::state::TypeSet::Attack) => ("an", "Attack"),
        Filter::Types(_) | Filter::CostAtLeast(_) | Filter::CostRange(..) => ("a", "card"),
        Filter::NonTreasure => ("a", "non-Treasure card"),
        Filter::TreasureNonDuration => ("a", "Treasure"),
        Filter::VictoryNot(_) => ("a", "Victory card"),
        Filter::GainableCopy => ("a", "card"),
        Filter::InheritCard => ("an", "Action"),
        Filter::NonVictory => ("a", "non-Victory card"),
        Filter::CommandCostUpTo(_) => ("an", "Action"),
        Filter::TreasureNot(_) => ("a", "Treasure"),
        Filter::CopyInAttackersPlay => ("a", "card"),
        Filter::NecroTarget => ("an", "Action"),
        Filter::SharesTypeWith(_) => ("a", "card"),
    }
}

fn starts_with_vowel_sound(s: &str) -> bool {
    matches!(s.chars().next(), Some('A' | 'E' | 'I' | 'O' | 'U' | 'a' | 'e' | 'i' | 'o' | 'u'))
}

impl ModeOpt {
    /// A short human label for the option, used by the UI, logging, and strategy `[[mode]]`
    /// rules' `choose = "..."` (matched case/space-insensitively; see `sim::strategy`).
    pub fn label(self) -> String {
        match self {
            ModeOpt::Cards(1) => "+1 Card".to_string(),
            ModeOpt::Cards(n) => format!("+{n} Cards"),
            ModeOpt::Actions(1) => "+1 Action".to_string(),
            ModeOpt::Actions(n) => format!("+{n} Actions"),
            ModeOpt::Buys(1) => "+1 Buy".to_string(),
            ModeOpt::Buys(n) => format!("+{n} Buys"),
            ModeOpt::Coins(n) => format!("+${n}"),
            ModeOpt::TrashFromHand(1) => "Trash a card".to_string(),
            ModeOpt::TrashFromHand(n) => format!("Trash {n} cards"),
            ModeOpt::DiscardFromHand(1) => "Discard a card".to_string(),
            ModeOpt::DiscardFromHand(n) => format!("Discard {n} cards"),
            ModeOpt::Gain(c, dest) => {
                let (a, noun) = filter_words(Filter::Card(c));
                let suffix = match dest {
                    Dest::Hand => " to your hand",
                    Dest::DeckTop => " onto your deck",
                    Dest::Discard => "",
                };
                format!("Gain {a} {noun}{suffix}")
            }
            ModeOpt::DiscardHandDraw { draw, .. } => format!("Discard your hand, +{draw} Cards"),
            ModeOpt::TrashFromSupply(f) => {
                let (a, noun) = filter_words(f);
                format!("Trash {a} {noun} from the Supply")
            }
            ModeOpt::GainFromTrash(f) => {
                let (a, noun) = filter_words(f);
                format!("Gain {a} {noun} from the trash")
            }
            ModeOpt::TrashSelfRevealVpPerTreasureType => "Trash this for +1 VP per Treasure type in hand".to_string(),
            ModeOpt::NativeVillageAdd => "Put the top card of your deck on your Native Village mat".to_string(),
            ModeOpt::NativeVillageTake => "Put the cards on your Native Village mat into your hand".to_string(),
            ModeOpt::GainMany(c, _, n) => format!("Gain {n} {}s", name(c)),
            ModeOpt::TopdeckFromHand(_) => "Put a card from your hand onto your deck".to_string(),
            ModeOpt::TrashHand => "Trash your hand".to_string(),
            ModeOpt::TrashGain { plus, .. } => format!("Trash a card, gain a card costing up to ${plus} more"),
            ModeOpt::GainFromTrashTo(f, dest) => {
                let (a, noun) = filter_words(f);
                let suffix = match dest {
                    Dest::Hand => " to your hand",
                    Dest::DeckTop => " onto your deck",
                    Dest::Discard => "",
                };
                format!("Gain {a} {noun} from the trash{suffix}")
            }
            ModeOpt::TakeRevealed => "Put them into your hand".to_string(),
            ModeOpt::DiscardRevealedDraw(n) => format!("Discard them and +{n} Cards"),
            ModeOpt::GainSubject { to_chooser: false } => "They gain a copy".to_string(),
            ModeOpt::GainSubject { to_chooser: true } => "You gain a copy".to_string(),
            ModeOpt::TavernCopper => "Put a Copper on your Tavern mat".to_string(),
            ModeOpt::CoinsPerTavernCopper => "+$1 per Copper on your Tavern mat".to_string(),
            ModeOpt::MoveToken(t) => format!("Move your {} token", TOKEN_NAMES[t as usize]),
            ModeOpt::DiscardFor { filter, n, .. } => {
                let (a, noun) = filter_words(filter);
                if n == 1 { format!("Discard {a} {noun}") } else if noun == "card" { format!("Discard {n} cards") } else { format!("Discard {n} {noun}s") }
            }
            ModeOpt::Nothing => "Do nothing".to_string(),
            ModeOpt::CardsGather(n, _) => format!("+{n} Cards"),
            ModeOpt::GainTakeVp(c) => {
                let (a, noun) = filter_words(Filter::Card(c));
                format!("Gain {a} {noun} and take the VP tokens")
            }
            ModeOpt::BuysCoins(b, c) => format!("+{b} Buy and +${c}"),
            ModeOpt::NextGainDouble => "Next gain brings another card".to_string(),
        }
    }
}

/// The ordered "choose one/many" options `card` offers (empty if it isn't a mode card): each set's `SetRules::modes`.
pub fn modes(card: CardId) -> &'static [ModeOpt] {
    crate::sets::modes(card)
}

// ---------------------------------------------------------------------------------------
// Duration cards: the vanilla start-of-next-turn bonus each Duration card grants, applied `times` times
// (Throne Room / King's Court) by `GameState::resolve_duration_start`. Card-specific extras beyond these numbers
// (Haven/Blockade's returned card, Sailor's optional trash, ...) are the set's `SetRules::duration_start`.
// ---------------------------------------------------------------------------------------

/// (cards, actions, buys, coins) granted at the start of the owner's next turn, once per
/// `PendingDuration::times`. All-zero for cards whose next-turn part is card-specific only
/// (Haven, Blockade, Pirate) or nonexistent (Outpost: entirely a cleanup/turn-order effect).
pub fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
    crate::sets::duration_bonus(card)
}

/// Whether `card`'s start-of-turn effect is order-sensitive: it acts on the hand (Sea Witch and
/// Tide Pools discard 2, Sailor may trash a card), adds to the hand (Haven and Blockade return a
/// card, Pirate gains a Treasure to hand), or plays a card (Prince's set-aside Action, Captain's Supply Action, Clerk's
/// "play this from your hand"). The other start-of-turn effects are plain bonuses
/// (`duration_bonus`: +Cards/+Actions/+Buys/+$) that never change what another effect can do.
/// When no pending start-of-turn effect is order-sensitive, or only one is pending, the engine
/// doesn't ask for an order (`GameState::run_start_order`).
pub fn start_order_sensitive(card: CardId) -> bool {
    crate::sets::start_order_sensitive(card)
}

pub const FIRST_KINGDOM: CardId = id::CELLAR;
/// The last kingdom card id: Sauna, which stands for the Sauna/Avanto pile. The kingdom cards are the ids from `FIRST_KINGDOM` to here, except
/// the Dark Ages cards no kingdom contains (Shelters, Spoils, Madman, Mercenary: ids after Hunting Grounds, `LAST_KINGDOM_DA`, before the
/// first Adventures card), the Rewards and the optional basics.
pub const LAST_KINGDOM: CardId = id::RAIDER;
/// The first member of a mixed pile (the five Ruins, then the ten Knights are the highest ids): a card from here is
/// the top card of the Knights or Ruins pile when it is in the Supply.
pub const FIRST_PILE_CARD: CardId = id::ABANDONED_MINE;

#[inline(always)]
pub fn set_of(card: CardId) -> CardSet {
    CARDS[card as usize].set
}

/// Whether the card's effects are implemented (only those can be in a kingdom).
#[inline(always)]
pub fn is_ready(card: CardId) -> bool {
    CARDS[card as usize].ready
}

/// Whether `card` is a kingdom card (not a basic card: Copper..Curse, Platinum, Colony, Potion, and
/// not one of the Rewards).
#[inline(always)]
pub fn is_kingdom(card: CardId) -> bool {
    card >= FIRST_KINGDOM
        && card <= LAST_KINGDOM
        && !is_optional_basic(card)
        && !is_reward(card)
        && !(card > crate::sets::dark_ages::LAST_KINGDOM_DA && card < id::COIN_OF_THE_REALM)
        // The split pile halves, the Castles and the Traveller upgrades sit between Change and the Nocturne cards.
        && !(card > id::CHANGE && card < id::DRUID)
}

/// Basic cards that are only in some games' supply (Prosperity's Platinum and Colony, Alchemy's
/// Potion). A kingdom list may name them to include them.
#[inline(always)]
pub fn is_optional_basic(card: CardId) -> bool {
    card == id::PLATINUM || card == id::COLONY || card == id::POTION
}

/// Every kingdom card of every set that can be played (implemented).
pub fn kingdom_cards() -> impl Iterator<Item = CardId> {
    (FIRST_KINGDOM..=LAST_KINGDOM).filter(|&c| is_kingdom(c) && is_ready(c))
}

/// The playable kingdom cards of one set.
pub fn kingdom_cards_in(set: CardSet) -> impl Iterator<Item = CardId> {
    kingdom_cards().filter(move |&c| set_of(c) == set)
}

/// The recommended "First Game" kingdom from the 2E rulebook.
pub const FIRST_GAME: [CardId; 10] = [
    id::CELLAR, id::MARKET, id::MERCHANT, id::MILITIA, id::MINE,
    id::MOAT, id::REMODEL, id::SMITHY, id::VILLAGE, id::WORKSHOP,
];

/// Case-insensitive lookup of a [`CardSet`] by its display name ("Base", "Intrigue").
pub fn set_by_name(s: &str) -> Option<CardSet> {
    match s.trim().to_lowercase().as_str() {
        "base" => Some(CardSet::Base),
        "intrigue" => Some(CardSet::Intrigue),
        "seaside" => Some(CardSet::Seaside),
        "prosperity" => Some(CardSet::Prosperity),
        "alchemy" => Some(CardSet::Alchemy),
        "promo" => Some(CardSet::Promo),
        "cornucopia & guilds" | "cornucopia and guilds" | "cornucopia" | "guilds" | "c&g" | "cg" | "cornucopia_guilds"
        | "cornucopiaguilds" => Some(CardSet::CornucopiaGuilds),
        "dark ages" | "darkages" | "dark_ages" | "da" => Some(CardSet::DarkAges),
        "adventures" | "adv" => Some(CardSet::Adventures),
        "empires" | "emp" => Some(CardSet::Empires),
        "rising sun" | "risingsun" | "rising_sun" | "rs" => Some(CardSet::RisingSun),
        "nocturne" | "noc" => Some(CardSet::Nocturne),
        _ => None,
    }
}

/// Ten kingdom cards: `required` is always included (deduplicated, in the order given), padded
/// with cards drawn uniformly at random from `sets` (excluding anything already in `required`)
/// using `rng`. If `required` already has 10 or more distinct cards, no padding happens and all
/// of them are kept (the caller may end up with more than 10 piles). The result is sorted by
/// card id, so the same `(sets, required, rng state)` always reproduces the same kingdom.
pub fn random_kingdom(sets: &[CardSet], required: &[CardId], rng: &mut Rng) -> Vec<CardId> {
    let mut out: Vec<CardId> = Vec::new();
    for &c in required {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    let mut pool: Vec<CardId> = kingdom_cards().filter(|c| sets.contains(&set_of(*c)) && !out.contains(c)).collect();
    // Optional basics named in `required` (Platinum, Colony, Potion) don't take a kingdom slot.
    while out.iter().filter(|&&c| is_kingdom(c)).count() < 10 && !pool.is_empty() {
        let i = rng.below(pool.len() as u32) as usize;
        out.push(pool.swap_remove(i));
    }
    out.sort_unstable();
    out
}

/// `random_kingdom`, plus the official Prosperity rule: reveal one of the kingdom's cards at
/// random; if it's from Prosperity, Platinum and Colony join the game (appended to the returned
/// list). The chance of Colonies therefore grows with how many Prosperity cards the kingdom has.
/// Used wherever a kingdom is randomized rather than named explicitly, so the rule stays
/// reproducible by seed alongside the kingdom draw itself. Potion joins whenever any kingdom card
/// has a Potion in its cost (no randomness involved).
pub fn random_kingdom_with_colonies(sets: &[CardSet], required: &[CardId], rng: &mut Rng) -> Vec<CardId> {
    let mut out = random_kingdom(sets, required, rng);
    let kingdom_only: Vec<CardId> = out.iter().copied().filter(|&c| is_kingdom(c)).collect();
    let revealed = kingdom_only[rng.below(kingdom_only.len() as u32) as usize];
    if set_of(revealed) == CardSet::Prosperity {
        if !out.contains(&id::PLATINUM) {
            out.push(id::PLATINUM);
        }
        if !out.contains(&id::COLONY) {
            out.push(id::COLONY);
        }
        out.sort_unstable();
    }
    add_potion_if_needed(&mut out);
    if sets.iter().any(|&s| pseudo_cards_of(s).next().is_some()) {
        add_random_events(&mut out, sets, rng);
    }
    out
}

/// The Events and Landmarks of one set (Adventures: 20 Events; Empires: 13 Events and 21 Landmarks).
pub fn pseudo_cards_of(set: CardSet) -> impl Iterator<Item = CardId> {
    (FIRST_EVENT..FIRST_EVENT + (NUM_EVENTS + NUM_LANDMARKS) as CardId).filter(move |&c| set_of(c) == set)
}

/// Choosing Events for a randomized kingdom, after the rulebook's randomizer: the Events are shuffled in with the kingdom cards of `sets`,
/// cards are dealt until ten kingdom cards are out, the Events dealt on the way are in the game, but at most two are used (0 to 2 per game).
/// The Events are appended to `kingdom` (they are not kingdom cards). Only the order of kingdom cards and Events matters here, so this
/// simulates the deal over counts. With Adventures alone (30 cards + 20 Events) about 6 Events are dealt, so two are used nearly always;
/// with every set (about 300 cards) about 0.7 are dealt, so roughly half the games have none, 1 in 3 one, the rest two.
pub fn add_random_events(kingdom: &mut Vec<CardId>, sets: &[CardSet], rng: &mut Rng) {
    let k = kingdom_cards().filter(|&c| sets.contains(&set_of(c))).count() as u32;
    let mut events: Vec<CardId> = sets.iter().flat_map(|&s| pseudo_cards_of(s)).filter(|e| !kingdom.contains(e)).collect();
    if events.is_empty() {
        return;
    }
    let (mut cards_left, mut events_left) = (k, events.len() as u32);
    let mut dealt_cards = 0;
    let mut used = 0;
    while dealt_cards < 10 && cards_left + events_left > 0 {
        if rng.below(cards_left + events_left) < cards_left {
            cards_left -= 1;
            dealt_cards += 1;
        } else {
            events_left -= 1;
            if used < 2 {
                let i = rng.below(events.len() as u32) as usize;
                kingdom.push(events.swap_remove(i));
                used += 1;
            }
        }
    }
}

/// Whether `c` can be the Bane (cost $2 or $3) or Ferryman's pile (cost $3 or $4): a playable
/// Kingdom card whose coins cost is in `lo..=hi`. The "+" of an overpay card is only a reminder
/// (Infirmary costs $3), but a Potion in the cost is not a plain $2 or $3 or $4, so those never
/// qualify. A card that needs a set-up pile of its own (Young Witch, Ferryman) never qualifies
/// by default, but may be named explicitly.
pub fn pile_eligible(c: CardId, lo: u8, hi: u8) -> bool {
    is_kingdom(c) && is_ready(c) && !potion_cost(c) && (lo..=hi).contains(&cost(c))
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Every Action, Treasure or Night card (plus Curse, which is a Treasure when Charlatan is in the
    /// game) must say whether playing it is choice-free (`choice_free(..)`, `has_choice(..)` or
    /// `order_sensitive(..)`); bots skip the play-order search only for choice-free hands. The exact
    /// lists are asserted by each set's own test (`sets/*.rs`).
    #[test]
    fn every_action_is_marked_choice_free_or_not() {
        for (i, d) in CARDS.iter().enumerate() {
            let marked = d.types & (ACTION | TREASURE | NIGHT) != 0 || i as CardId == id::CURSE;
            assert_eq!(d.on_play != OnPlay::NotAction, marked, "{} (card {i}) must be marked with choice_free/has_choice/order_sensitive iff it is an Action or Treasure", d.name);
        }
    }
}

#[cfg(test)]
mod set_tests {
    use super::*;

    #[test]
    fn ids_names_and_sets_line_up() {
        assert_eq!(by_name("Nobles"), Some(id::NOBLES));
        assert_eq!(by_name("shanty town"), Some(id::SHANTY_TOWN));
        assert_eq!(by_name("WishingWell"), Some(id::WISHING_WELL));
        let in_set = |set| (FIRST_KINGDOM..NUM_CARDS as CardId).filter(|&c| is_kingdom(c) && set_of(c) == set).count();
        assert_eq!(in_set(CardSet::Base), 26);
        assert_eq!(in_set(CardSet::Intrigue), 26);
        assert_eq!(in_set(CardSet::Seaside), 27);
        assert_eq!(in_set(CardSet::Prosperity), 25);
        assert_eq!(in_set(CardSet::Alchemy), 11);
        assert_eq!(in_set(CardSet::Promo), 5); // Prince, Walled Village, Envoy, Captain and the split pile Sauna/Avanto (Sauna stands for it)
        assert!(!is_kingdom(id::POTION) && is_optional_basic(id::POTION) && potion_cost(id::GOLEM) && !potion_cost(id::APPRENTICE));
        assert_eq!(cost_string(id::APOTHECARY), "$2P");
        assert!(!is_kingdom(id::PLATINUM) && !is_kingdom(id::COLONY) && is_optional_basic(id::COLONY));
        assert_eq!(by_name("kings court"), Some(id::KINGS_COURT));
        assert_eq!(by_name("Worker's Village"), Some(id::WORKERS_VILLAGE));
        assert!(is(id::HAVEN, DURATION) && is(id::ASTROLABE, TREASURE) && is(id::ASTROLABE, DURATION));
        // Every card name is unique.
        for a in 0..NUM_CARDS as CardId {
            assert_eq!(by_name(name(a)), Some(a), "{}", name(a));
        }
        assert!(is(id::HAREM, TREASURE) && is(id::HAREM, VICTORY));
        assert!(is(id::MILL, ACTION) && is(id::MILL, VICTORY));
        assert_eq!(kingdom_cards_in(CardSet::Base).count(), 26);
        assert_eq!(kingdom_cards_in(CardSet::Intrigue).count(), 26);
        // All 27 Seaside and 25 Prosperity kingdom cards are implemented (`ready`) as of step 4.
        assert_eq!(kingdom_cards_in(CardSet::Seaside).count(), 27);
        assert_eq!(kingdom_cards_in(CardSet::Prosperity).count(), 25);
        assert_eq!(kingdom_cards_in(CardSet::Alchemy).count(), 11);
        assert_eq!(kingdom_cards_in(CardSet::Promo).count(), 5);
        assert_eq!(kingdom_cards_in(CardSet::Adventures).count(), 30);
        assert_eq!(in_set(CardSet::Adventures), 30);
    }

    #[test]
    fn set_by_name_matches_common_spellings() {
        assert_eq!(set_by_name("Base"), Some(CardSet::Base));
        assert_eq!(set_by_name("base"), Some(CardSet::Base));
        assert_eq!(set_by_name(" Intrigue "), Some(CardSet::Intrigue));
        assert_eq!(set_by_name("intrigue"), Some(CardSet::Intrigue));
        assert_eq!(set_by_name("Seaside"), Some(CardSet::Seaside));
        assert_eq!(set_by_name("prosperity"), Some(CardSet::Prosperity));
        assert_eq!(set_by_name("Alchemy"), Some(CardSet::Alchemy));
        assert_eq!(set_by_name("promo"), Some(CardSet::Promo));
        assert_eq!(set_by_name("Dark Ages"), Some(CardSet::DarkAges));
        assert_eq!(set_by_name("nonsense"), None);
    }

    #[test]
    fn random_kingdom_is_reproducible_respects_sets_and_has_no_duplicates() {
        let mut rng1 = Rng::new(42);
        let k1 = random_kingdom(&[CardSet::Base], &[], &mut rng1);
        let mut rng2 = Rng::new(42);
        let k2 = random_kingdom(&[CardSet::Base], &[], &mut rng2);
        assert_eq!(k1, k2, "same seed must reproduce the same kingdom");
        assert_eq!(k1.len(), 10);
        for &c in &k1 {
            assert_eq!(set_of(c), CardSet::Base, "{} is not from Base", name(c));
        }
        let mut dedup = k1.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), k1.len(), "no duplicates: {k1:?}");

        // A different seed usually gives a different kingdom.
        let mut rng3 = Rng::new(43);
        let k3 = random_kingdom(&[CardSet::Base], &[], &mut rng3);
        assert_ne!(k1, k3);

        // Required cards always stay in, even from another set.
        let mut rng4 = Rng::new(1);
        let k4 = random_kingdom(&[CardSet::Base], &[id::WITCH, id::TORTURER], &mut rng4);
        assert_eq!(k4.len(), 10);
        assert!(k4.contains(&id::WITCH));
        assert!(k4.contains(&id::TORTURER));
        // Everything else padded in is from Base.
        for &c in &k4 {
            if c != id::TORTURER {
                assert!(c == id::WITCH || set_of(c) == CardSet::Base);
            }
        }

        // Both sets: cards from either are allowed.
        let mut rng5 = Rng::new(7);
        let k5 = random_kingdom(&[CardSet::Base, CardSet::Intrigue], &[], &mut rng5);
        assert_eq!(k5.len(), 10);
        assert!(k5.iter().all(|&c| set_of(c) == CardSet::Base || set_of(c) == CardSet::Intrigue));
    }

    #[test]
    fn colonies_join_only_when_prosperity_is_drawn() {
        // Base/Intrigue only: never Prosperity, so never Platinum/Colony.
        for seed in 0..20u64 {
            let mut rng = Rng::new(seed);
            let k = random_kingdom_with_colonies(&[CardSet::Base, CardSet::Intrigue], &[], &mut rng);
            assert!(!k.contains(&id::PLATINUM) && !k.contains(&id::COLONY));
        }
        // Prosperity-only: always drawn, so always Platinum/Colony, appended and sorted in.
        let mut rng = Rng::new(1);
        let k = random_kingdom_with_colonies(&[CardSet::Prosperity], &[], &mut rng);
        assert!(k.contains(&id::PLATINUM) && k.contains(&id::COLONY));
        assert_eq!(k.len(), 12);
        let mut sorted = k.clone();
        sorted.sort_unstable();
        assert_eq!(k, sorted);

        // One Prosperity card among ten: Colonies only when that card is the one revealed, so
        // about 10% of games.
        let with = (0..2000)
            .filter(|&s| {
                let mut r = Rng::new(s);
                random_kingdom_with_colonies(&[CardSet::Base], &[id::CITY], &mut r).contains(&id::COLONY)
            })
            .count();
        assert!((120..=280).contains(&with), "Colonies in {with} of 2000 games; expected ~200");

        // Reproducible by seed.
        let mut rng3a = Rng::new(9);
        let mut rng3b = Rng::new(9);
        let ka = random_kingdom_with_colonies(&[CardSet::Prosperity, CardSet::Seaside], &[], &mut rng3a);
        let kb = random_kingdom_with_colonies(&[CardSet::Prosperity, CardSet::Seaside], &[], &mut rng3b);
        assert_eq!(ka, kb);
    }
}
