//! Card identities and static definitions: Base Set, Intrigue, Seaside and Prosperity, all 2nd
//! edition.

use crate::rng::Rng;
use crate::state::{Dest, Filter};

pub type CardId = u8;
pub const NUM_CARDS: usize = 113;

// Type flags.
pub const ACTION: u8 = 1;
pub const TREASURE: u8 = 2;
pub const VICTORY: u8 = 4;
pub const CURSE_T: u8 = 8;
pub const ATTACK: u8 = 16;
pub const REACTION: u8 = 32;
/// Stays in play with effects on a later turn (Seaside).
pub const DURATION: u8 = 64;

pub mod id {
    use super::CardId;
    pub const COPPER: CardId = 0;
    pub const SILVER: CardId = 1;
    pub const GOLD: CardId = 2;
    pub const ESTATE: CardId = 3;
    pub const DUCHY: CardId = 4;
    pub const PROVINCE: CardId = 5;
    pub const CURSE: CardId = 6;
    pub const CELLAR: CardId = 7;
    pub const CHAPEL: CardId = 8;
    pub const MOAT: CardId = 9;
    pub const HARBINGER: CardId = 10;
    pub const MERCHANT: CardId = 11;
    pub const VASSAL: CardId = 12;
    pub const VILLAGE: CardId = 13;
    pub const WORKSHOP: CardId = 14;
    pub const BUREAUCRAT: CardId = 15;
    pub const GARDENS: CardId = 16;
    pub const MILITIA: CardId = 17;
    pub const MONEYLENDER: CardId = 18;
    pub const POACHER: CardId = 19;
    pub const REMODEL: CardId = 20;
    pub const SMITHY: CardId = 21;
    pub const THRONE_ROOM: CardId = 22;
    pub const BANDIT: CardId = 23;
    pub const COUNCIL_ROOM: CardId = 24;
    pub const FESTIVAL: CardId = 25;
    pub const LABORATORY: CardId = 26;
    pub const LIBRARY: CardId = 27;
    pub const MARKET: CardId = 28;
    pub const MINE: CardId = 29;
    pub const SENTRY: CardId = 30;
    pub const WITCH: CardId = 31;
    pub const ARTISAN: CardId = 32;
    // Intrigue (2nd edition)
    pub const COURTYARD: CardId = 33;
    pub const LURKER: CardId = 34;
    pub const PAWN: CardId = 35;
    pub const MASQUERADE: CardId = 36;
    pub const SHANTY_TOWN: CardId = 37;
    pub const STEWARD: CardId = 38;
    pub const SWINDLER: CardId = 39;
    pub const WISHING_WELL: CardId = 40;
    pub const BARON: CardId = 41;
    pub const BRIDGE: CardId = 42;
    pub const CONSPIRATOR: CardId = 43;
    pub const DIPLOMAT: CardId = 44;
    pub const IRONWORKS: CardId = 45;
    pub const MILL: CardId = 46;
    pub const MINING_VILLAGE: CardId = 47;
    pub const SECRET_PASSAGE: CardId = 48;
    pub const COURTIER: CardId = 49;
    pub const DUKE: CardId = 50;
    pub const MINION: CardId = 51;
    pub const PATROL: CardId = 52;
    pub const REPLACE: CardId = 53;
    pub const TORTURER: CardId = 54;
    pub const TRADING_POST: CardId = 55;
    pub const UPGRADE: CardId = 56;
    pub const HAREM: CardId = 57;
    pub const NOBLES: CardId = 58;
    // Seaside (2nd edition)
    pub const HAVEN: CardId = 59;
    pub const LIGHTHOUSE: CardId = 60;
    pub const NATIVE_VILLAGE: CardId = 61;
    pub const ASTROLABE: CardId = 62;
    pub const FISHING_VILLAGE: CardId = 63;
    pub const LOOKOUT: CardId = 64;
    pub const MONKEY: CardId = 65;
    pub const SEA_CHART: CardId = 66;
    pub const SMUGGLERS: CardId = 67;
    pub const WAREHOUSE: CardId = 68;
    pub const BLOCKADE: CardId = 69;
    pub const CARAVAN: CardId = 70;
    pub const CUTPURSE: CardId = 71;
    pub const ISLAND: CardId = 72;
    pub const SALVAGER: CardId = 73;
    pub const SAILOR: CardId = 74;
    pub const TIDE_POOLS: CardId = 75;
    pub const TREASURE_MAP: CardId = 76;
    pub const BAZAAR: CardId = 77;
    pub const CORSAIR: CardId = 78;
    pub const MERCHANT_SHIP: CardId = 79;
    pub const OUTPOST: CardId = 80;
    pub const PIRATE: CardId = 81;
    pub const SEA_WITCH: CardId = 82;
    pub const TACTICIAN: CardId = 83;
    pub const TREASURY: CardId = 84;
    pub const WHARF: CardId = 85;
    // Prosperity (2nd edition); Platinum and Colony are basic cards, not kingdom cards
    pub const ANVIL: CardId = 86;
    pub const WATCHTOWER: CardId = 87;
    pub const BISHOP: CardId = 88;
    pub const CLERK: CardId = 89;
    pub const INVESTMENT: CardId = 90;
    pub const MONUMENT: CardId = 91;
    pub const QUARRY: CardId = 92;
    pub const TIARA: CardId = 93;
    pub const WORKERS_VILLAGE: CardId = 94;
    pub const CHARLATAN: CardId = 95;
    pub const CITY: CardId = 96;
    pub const COLLECTION: CardId = 97;
    pub const CRYSTAL_BALL: CardId = 98;
    pub const MAGNATE: CardId = 99;
    pub const MINT: CardId = 100;
    pub const RABBLE: CardId = 101;
    pub const VAULT: CardId = 102;
    pub const WAR_CHEST: CardId = 103;
    pub const GRAND_MARKET: CardId = 104;
    pub const HOARD: CardId = 105;
    pub const BANK: CardId = 106;
    pub const EXPAND: CardId = 107;
    pub const FORGE: CardId = 108;
    pub const KINGS_COURT: CardId = 109;
    pub const PEDDLER: CardId = 110;
    pub const PLATINUM: CardId = 111;
    pub const COLONY: CardId = 112;
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
}

impl CardSet {
    pub const ALL: [CardSet; 4] = [CardSet::Base, CardSet::Intrigue, CardSet::Seaside, CardSet::Prosperity];

    pub fn name(self) -> &'static str {
        match self {
            CardSet::Base => "Base",
            CardSet::Intrigue => "Intrigue",
            CardSet::Seaside => "Seaside",
            CardSet::Prosperity => "Prosperity",
        }
    }
}

/// Static card data. "Vanilla" bonuses (+cards/+actions/+buys/+coins) are applied
/// generically when the card is played; anything else is handled in `effects.rs`.
#[derive(Clone, Copy, Debug)]
pub struct CardDef {
    pub name: &'static str,
    pub cost: u8,
    pub types: u8,
    /// Treasure value when played as a treasure, or +$ when played as an action.
    pub coins: u8,
    pub vp: i8,
    pub cards: u8,
    pub actions: u8,
    pub buys: u8,
    /// Plays another Action card this many times (Throne Room: 2); 0 if it doesn't.
    pub plays: u8,
    /// Whether playing it involves a choice for its player (see [`OnPlay`]). Every Action card
    /// must be marked explicitly with `choice_free(..)` or `has_choice(..)`.
    pub on_play: OnPlay,
    pub set: CardSet,
    /// Whether the card's effects are implemented. Cards that aren't can't be put in a kingdom
    /// (they would silently play as plain cards); see `intrigue_todo`.
    pub ready: bool,
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
    /// changes that player's own deck or discard in a way a later draw can see (Bureaucrat,
    /// Bandit, Vassal, Harbinger, Sentry). Play order is a real choice: bots search it.
    Choice,
}

const fn c(name: &'static str, cost: u8, types: u8, coins: u8, vp: i8, cards: u8, actions: u8, buys: u8) -> CardDef {
    CardDef { name, cost, types, coins, vp, cards, actions, buys, plays: 0, on_play: OnPlay::NotAction, set: CardSet::Base, ready: true }
}

/// A Seaside card.
const fn seaside(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Seaside, ..d }
}

/// A Prosperity card.
const fn prosperity(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Prosperity, ..d }
}

/// A card of `set` whose effects aren't implemented yet (not allowed in kingdoms).
const fn todo(set: CardSet, d: CardDef) -> CardDef {
    CardDef { set, ready: false, ..d }
}

/// An Intrigue card.
const fn intrigue(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Intrigue, ..d }
}

const fn plays(d: CardDef, times: u8) -> CardDef {
    CardDef { plays: times, ..d }
}

/// Marks an Action card as [`OnPlay::ChoiceFree`].
const fn choice_free(d: CardDef) -> CardDef {
    CardDef { on_play: OnPlay::ChoiceFree, ..d }
}

/// Marks an Action card as [`OnPlay::Choice`].
const fn has_choice(d: CardDef) -> CardDef {
    CardDef { on_play: OnPlay::Choice, ..d }
}

pub static CARDS: [CardDef; NUM_CARDS] = [
    choice_free(c("Copper", 0, TREASURE, 1, 0, 0, 0, 0)),
    choice_free(c("Silver", 3, TREASURE, 2, 0, 0, 0, 0)),
    choice_free(c("Gold", 6, TREASURE, 3, 0, 0, 0, 0)),
    c("Estate", 2, VICTORY, 0, 1, 0, 0, 0),
    c("Duchy", 5, VICTORY, 0, 3, 0, 0, 0),
    c("Province", 8, VICTORY, 0, 6, 0, 0, 0),
    // Coins=1: only relevant when Curse is also a Treasure this game (Charlatan; see
    // `GameState::is_treasure`). Choice-free: playing it (when it's a Treasure) is just +$1.
    choice_free(c("Curse", 0, CURSE_T, 1, -1, 0, 0, 0)),
    has_choice(c("Cellar", 2, ACTION, 0, 0, 0, 1, 0)), // what to discard
    has_choice(c("Chapel", 2, ACTION, 0, 0, 0, 0, 0)), // what to trash
    choice_free(c("Moat", 2, ACTION | REACTION, 0, 0, 2, 0, 0)),
    has_choice(c("Harbinger", 3, ACTION, 0, 0, 1, 1, 0)), // what to topdeck from the discard
    choice_free(c("Merchant", 3, ACTION, 0, 0, 1, 1, 0)), // +$1 on the first Silver: order-free
    has_choice(c("Vassal", 3, ACTION, 2, 0, 0, 0, 0)), // discards the deck top; may play it
    choice_free(c("Village", 3, ACTION, 0, 0, 1, 2, 0)),
    has_choice(c("Workshop", 3, ACTION, 0, 0, 0, 0, 0)), // what to gain
    has_choice(c("Bureaucrat", 4, ACTION | ATTACK, 0, 0, 0, 0, 0)), // gains Silver onto the deck
    c("Gardens", 4, VICTORY, 0, 0, 0, 0, 0),
    choice_free(c("Militia", 4, ACTION | ATTACK, 2, 0, 0, 0, 0)),
    has_choice(c("Moneylender", 4, ACTION, 0, 0, 0, 0, 0)), // may trash a Copper
    has_choice(c("Poacher", 4, ACTION, 1, 0, 1, 1, 0)), // discards per empty pile
    has_choice(c("Remodel", 4, ACTION, 0, 0, 0, 0, 0)), // what to trash and gain
    choice_free(c("Smithy", 4, ACTION, 0, 0, 3, 0, 0)),
    has_choice(plays(c("Throne Room", 4, ACTION, 0, 0, 0, 0, 0), 2)), // what to play twice
    has_choice(c("Bandit", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)), // gains a Gold (a reshuffle can draw it)
    choice_free(c("Council Room", 5, ACTION, 0, 0, 4, 0, 1)),
    choice_free(c("Festival", 5, ACTION, 2, 0, 0, 2, 1)),
    choice_free(c("Laboratory", 5, ACTION, 0, 0, 2, 1, 0)),
    has_choice(c("Library", 5, ACTION, 0, 0, 0, 0, 0)), // which actions to set aside
    choice_free(c("Market", 5, ACTION, 1, 0, 1, 1, 1)),
    has_choice(c("Mine", 5, ACTION, 0, 0, 0, 0, 0)), // which treasure to upgrade
    has_choice(c("Sentry", 5, ACTION, 0, 0, 1, 1, 0)), // trash / discard / reorder the top 2
    choice_free(c("Witch", 5, ACTION | ATTACK, 0, 0, 2, 0, 0)),
    has_choice(c("Artisan", 6, ACTION, 0, 0, 0, 0, 0)), // what to gain and topdeck
    // ---- Intrigue (2nd edition). Stats are the vanilla part; the rest is in `effects.rs`. ----
    intrigue(has_choice(c("Courtyard", 2, ACTION, 0, 0, 3, 0, 0))), // what to put on the deck
    intrigue(has_choice(c("Lurker", 2, ACTION, 0, 0, 0, 1, 0))), // trash from Supply or gain from trash
    intrigue(has_choice(c("Pawn", 2, ACTION, 0, 0, 0, 0, 0))), // two of four bonuses
    intrigue(has_choice(c("Masquerade", 3, ACTION, 0, 0, 2, 0, 0))), // what to pass / trash
    intrigue(has_choice(c("Shanty Town", 3, ACTION, 0, 0, 0, 2, 0))), // draws only with no Actions in hand: order matters
    intrigue(has_choice(c("Steward", 3, ACTION, 0, 0, 0, 0, 0))), // one of three
    intrigue(has_choice(c("Swindler", 3, ACTION | ATTACK, 2, 0, 0, 0, 0))), // what the victims gain
    intrigue(has_choice(c("Wishing Well", 3, ACTION, 0, 0, 1, 1, 0))), // name a card
    intrigue(has_choice(c("Baron", 4, ACTION, 0, 0, 0, 0, 1))), // discard an Estate?
    intrigue(choice_free(c("Bridge", 4, ACTION, 1, 0, 0, 0, 1))), // cost reduction only (no gains of its own)
    intrigue(has_choice(c("Conspirator", 4, ACTION, 2, 0, 0, 0, 0))), // depends on actions played: order matters
    intrigue(has_choice(c("Diplomat", 4, ACTION | REACTION, 0, 0, 2, 0, 0))), // depends on hand size: order matters
    intrigue(has_choice(c("Ironworks", 4, ACTION, 0, 0, 0, 0, 0))), // what to gain
    intrigue(has_choice(c("Mill", 4, ACTION | VICTORY, 0, 1, 1, 1, 0))), // discard 2?
    intrigue(has_choice(c("Mining Village", 4, ACTION, 0, 0, 1, 2, 0))), // trash it?
    intrigue(has_choice(c("Secret Passage", 4, ACTION, 0, 0, 2, 1, 0))), // what to put where in the deck
    intrigue(has_choice(c("Courtier", 5, ACTION, 0, 0, 0, 0, 0))), // what to reveal, which bonuses
    intrigue(c("Duke", 5, VICTORY, 0, 0, 0, 0, 0)), // 1 VP per Duchy (`state::vp_of_cards`)
    intrigue(has_choice(c("Minion", 5, ACTION | ATTACK, 0, 0, 0, 1, 0))), // +$2 or new hands
    intrigue(has_choice(c("Patrol", 5, ACTION, 0, 0, 3, 0, 0))), // order of the cards put back
    intrigue(has_choice(c("Replace", 5, ACTION | ATTACK, 0, 0, 0, 0, 0))), // what to trash and gain
    intrigue(choice_free(c("Torturer", 5, ACTION | ATTACK, 0, 0, 3, 0, 0))), // only the victims choose
    intrigue(has_choice(c("Trading Post", 5, ACTION, 0, 0, 0, 0, 0))), // what to trash
    intrigue(has_choice(c("Upgrade", 5, ACTION, 0, 0, 1, 1, 0))), // what to trash and gain
    intrigue(choice_free(c("Harem", 6, TREASURE | VICTORY, 2, 2, 0, 0, 0))),
    intrigue(has_choice(c("Nobles", 6, ACTION | VICTORY, 0, 2, 0, 0, 0))), // +3 Cards or +2 Actions
    // ---- Seaside (2nd edition). D = Duration; the next-turn parts are in `effects.rs`. ----
    seaside(has_choice(c("Haven", 2, ACTION | DURATION, 0, 0, 1, 1, 0))), // what to set aside
    seaside(choice_free(c("Lighthouse", 2, ACTION | DURATION, 1, 0, 0, 1, 0))),
    todo(CardSet::Seaside, has_choice(c("Native Village", 2, ACTION, 0, 0, 0, 2, 0))), // mat: add or take
    seaside(choice_free(c("Astrolabe", 3, TREASURE | DURATION, 1, 0, 0, 0, 1))),
    seaside(choice_free(c("Fishing Village", 3, ACTION | DURATION, 1, 0, 0, 2, 0))),
    todo(CardSet::Seaside, has_choice(c("Lookout", 3, ACTION, 0, 0, 0, 1, 0))), // trash / discard / keep
    seaside(choice_free(c("Monkey", 3, ACTION | DURATION, 0, 0, 0, 0, 0))),
    todo(CardSet::Seaside, has_choice(c("Sea Chart", 3, ACTION, 0, 0, 1, 1, 0))), // depends on what's in play: order matters
    todo(CardSet::Seaside, has_choice(c("Smugglers", 3, ACTION, 0, 0, 0, 0, 0))), // what to gain
    todo(CardSet::Seaside, has_choice(c("Warehouse", 3, ACTION, 0, 0, 3, 1, 0))), // what to discard
    seaside(has_choice(c("Blockade", 4, ACTION | DURATION | ATTACK, 0, 0, 0, 0, 0))), // what to gain
    seaside(choice_free(c("Caravan", 4, ACTION | DURATION, 0, 0, 1, 1, 0))),
    todo(CardSet::Seaside, choice_free(c("Cutpurse", 4, ACTION | ATTACK, 2, 0, 0, 0, 0))), // only victims act
    todo(CardSet::Seaside, has_choice(c("Island", 4, ACTION | VICTORY, 0, 2, 0, 0, 0))), // what to put on the mat
    todo(CardSet::Seaside, has_choice(c("Salvager", 4, ACTION, 0, 0, 0, 0, 1))), // what to trash
    seaside(choice_free(c("Sailor", 4, ACTION | DURATION, 0, 0, 0, 1, 0))), // choices only on gains / next turn
    seaside(choice_free(c("Tide Pools", 4, ACTION | DURATION, 0, 0, 3, 1, 0))), // the discard is next turn
    todo(CardSet::Seaside, has_choice(c("Treasure Map", 4, ACTION, 0, 0, 0, 0, 0))), // trashes itself and another
    seaside(choice_free(c("Bazaar", 5, ACTION, 1, 0, 1, 2, 0))),
    seaside(choice_free(c("Corsair", 5, ACTION | DURATION | ATTACK, 2, 0, 0, 0, 0))),
    seaside(choice_free(c("Merchant Ship", 5, ACTION | DURATION, 2, 0, 0, 0, 0))),
    seaside(choice_free(c("Outpost", 5, ACTION | DURATION, 0, 0, 0, 0, 0))),
    seaside(choice_free(c("Pirate", 5, ACTION | DURATION | REACTION, 0, 0, 0, 0, 0))),
    seaside(choice_free(c("Sea Witch", 5, ACTION | DURATION | ATTACK, 0, 0, 2, 0, 0))), // the discard is next turn
    seaside(has_choice(c("Tactician", 5, ACTION | DURATION, 0, 0, 0, 0, 0))), // discards the hand: order matters
    todo(CardSet::Seaside, choice_free(c("Treasury", 5, ACTION, 1, 0, 1, 1, 0))),
    seaside(choice_free(c("Wharf", 5, ACTION | DURATION, 0, 0, 2, 0, 1))),
    // ---- Prosperity (2nd edition). ----
    prosperity(has_choice(c("Anvil", 3, TREASURE, 1, 0, 0, 0, 0))), // discard a Treasure to gain up to $4?
    prosperity(has_choice(c("Watchtower", 3, ACTION | REACTION, 0, 0, 0, 0, 0))), // draws to 6: order matters
    prosperity(has_choice(c("Bishop", 4, ACTION, 1, 0, 0, 0, 0))), // what to trash
    prosperity(choice_free(c("Clerk", 4, ACTION | REACTION | ATTACK, 2, 0, 0, 0, 0))), // only victims act
    prosperity(has_choice(c("Investment", 4, TREASURE, 0, 0, 0, 0, 0))), // trash a card, then +$1 or trash this for VP
    prosperity(choice_free(c("Monument", 4, ACTION, 2, 0, 0, 0, 0))), // +1 VP token
    prosperity(choice_free(c("Quarry", 4, TREASURE, 1, 0, 0, 0, 0))), // flat $1; the cost reduction is passive
    prosperity(has_choice(c("Tiara", 4, TREASURE, 0, 0, 0, 0, 1))), // which Treasure (if any) to play twice
    prosperity(choice_free(c("Worker's Village", 4, ACTION, 0, 0, 1, 2, 1))),
    prosperity(choice_free(c("Charlatan", 5, ACTION | ATTACK, 3, 0, 0, 0, 0))),
    prosperity(choice_free(c("City", 5, ACTION, 0, 0, 1, 2, 0))), // bonus from empty piles, which choice-free plays can't change
    prosperity(choice_free(c("Collection", 5, TREASURE, 2, 0, 0, 0, 1))), // passive +1 VP on gain while in play
    prosperity(has_choice(c("Crystal Ball", 5, TREASURE, 1, 0, 0, 0, 0))), // trash / discard / play the top card
    prosperity(has_choice(c("Magnate", 5, ACTION, 0, 0, 0, 0, 0))), // draws per treasure in hand: order matters
    prosperity(has_choice(c("Mint", 5, ACTION, 0, 0, 0, 0, 0))), // which treasure to copy
    prosperity(choice_free(c("Rabble", 5, ACTION | ATTACK, 0, 0, 3, 0, 0))), // only victims act
    prosperity(has_choice(c("Vault", 5, ACTION, 0, 0, 2, 0, 0))), // what to discard
    prosperity(has_choice(c("War Chest", 5, TREASURE, 0, 0, 0, 0, 0))), // gain up to $5, not named
    prosperity(choice_free(c("Grand Market", 6, ACTION, 2, 0, 1, 1, 1))), // can't be bought with a Copper in play
    prosperity(choice_free(c("Hoard", 6, TREASURE, 2, 0, 0, 0, 0))), // passive bonus Gold on bought Victory gains
    prosperity(has_choice(c("Bank", 7, TREASURE, 0, 0, 0, 0, 0))), // value depends on play order: play it last
    prosperity(has_choice(c("Expand", 7, ACTION, 0, 0, 0, 0, 0))), // what to trash and gain
    prosperity(has_choice(c("Forge", 7, ACTION, 0, 0, 0, 0, 0))), // what to trash and gain
    prosperity(has_choice(plays(c("King's Court", 7, ACTION, 0, 0, 0, 0, 0), 3))), // what to play three times
    prosperity(choice_free(c("Peddler", 8, ACTION, 1, 0, 1, 1, 0))),
    prosperity(choice_free(c("Platinum", 9, TREASURE, 5, 0, 0, 0, 0))),
    prosperity(c("Colony", 11, VICTORY, 0, 10, 0, 0, 0)),
];

/// Whether `card` is an [`OnPlay::ChoiceFree`] action.
#[inline(always)]
pub fn is_choice_free(card: CardId) -> bool {
    CARDS[card as usize].on_play == OnPlay::ChoiceFree
}

#[inline(always)]
pub fn def(card: CardId) -> &'static CardDef {
    &CARDS[card as usize]
}
#[inline(always)]
pub fn cost(card: CardId) -> u8 {
    CARDS[card as usize].cost
}
#[inline(always)]
pub fn is(card: CardId, flag: u8) -> bool {
    CARDS[card as usize].types & flag != 0
}
#[inline(always)]
pub fn name(card: CardId) -> &'static str {
    CARDS[card as usize].name
}

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
        }
    }
}

/// The ordered "choose one/many" options `card` offers (empty if it isn't a mode card).
/// A `match` on constant slices: what varies per card is only this data.
pub fn modes(card: CardId) -> &'static [ModeOpt] {
    match card {
        id::PAWN => &[ModeOpt::Cards(1), ModeOpt::Actions(1), ModeOpt::Buys(1), ModeOpt::Coins(1)],
        id::STEWARD => &[ModeOpt::Cards(2), ModeOpt::Coins(2), ModeOpt::TrashFromHand(2)],
        id::NOBLES => &[ModeOpt::Cards(3), ModeOpt::Actions(2)],
        id::MINION => &[ModeOpt::Coins(2), ModeOpt::DiscardHandDraw { draw: 4, attack_min_hand: 5 }],
        id::COURTIER => &[ModeOpt::Actions(1), ModeOpt::Buys(1), ModeOpt::Coins(3), ModeOpt::Gain(id::GOLD, Dest::Discard)],
        id::LURKER => &[ModeOpt::TrashFromSupply(Filter::Action), ModeOpt::GainFromTrash(Filter::Action)],
        id::TORTURER => &[ModeOpt::DiscardFromHand(2), ModeOpt::Gain(id::CURSE, Dest::Hand)],
        id::INVESTMENT => &[ModeOpt::Coins(1), ModeOpt::TrashSelfRevealVpPerTreasureType],
        _ => &[],
    }
}

// ---------------------------------------------------------------------------------------
// "When you gain a card" triggers (Prosperity step 2; see `GameState::gain`). A small
// per-card table, like `REACTION_EFFECTS` and `modes`: nothing card-specific appears outside
// this data and the single generic resolution point in `state.rs`/`engine.rs`.
// ---------------------------------------------------------------------------------------

/// Where a "when you gain" watcher card must be for its trigger to be live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GainTriggerZone {
    /// A reaction in the gainer's hand (Watchtower).
    Hand,
    /// A static card in the gainer's play area (Hoard, Collection, Tiara).
    InPlay,
}

/// A "when you gain a card" behaviour a watcher card contributes for its owner, once it's
/// found in the right zone (see `GainTriggerZone`). Resolution lives in `GameState::gain`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GainTrigger {
    /// May reveal to trash the gained card or put it onto the deck instead (Watchtower).
    WatchtowerReact,
    /// Gaining a Victory card you bought also gains a Gold (Hoard).
    HoardBoughtVictory,
    /// Gaining an Action card gives +1 VP token (Collection).
    CollectionAction,
    /// May put the gained card onto the deck instead of its normal destination (Tiara).
    TiaraTopdeck,
}

/// (watcher card, zone it must be found in, trigger). See `GameState::gain`.
pub static GAIN_TRIGGERS: &[(CardId, GainTriggerZone, GainTrigger)] = &[
    (id::WATCHTOWER, GainTriggerZone::Hand, GainTrigger::WatchtowerReact),
    (id::HOARD, GainTriggerZone::InPlay, GainTrigger::HoardBoughtVictory),
    (id::COLLECTION, GainTriggerZone::InPlay, GainTrigger::CollectionAction),
    (id::TIARA, GainTriggerZone::InPlay, GainTrigger::TiaraTopdeck),
];

/// Bitmask (over card ids, same encoding as `GameState::in_supply`) of every card named in
/// `GAIN_TRIGGERS`, so `GameState::gain`'s hot path can skip the whole table with one cheap
/// `u128` AND when this game's kingdom has none of them, instead of probing hand/in-play on
/// every single gain.
pub const GAIN_TRIGGER_CARDS_MASK: u128 =
    (1u128 << id::WATCHTOWER) | (1u128 << id::HOARD) | (1u128 << id::COLLECTION) | (1u128 << id::TIARA);

/// Reaction cards that offer an optional effect (not a block) when another player plays an
/// Attack: `(card, minimum hand size to reveal, cards drawn, cards then discarded)`. Moat's
/// blocking reaction is handled separately (`GameState::immune`); this table is for reactions
/// like Diplomat's that let the revealer act without stopping the attack.
pub static REACTION_EFFECTS: &[(CardId, u8, u8, u8)] = &[(id::DIPLOMAT, 5, 2, 3)];

// ---------------------------------------------------------------------------------------
// Duration cards (Seaside step 3): the vanilla start-of-next-turn bonus each Duration card
// grants, applied `times` times (Throne Room / King's Court) by `GameState::resolve_duration_start`.
// Card-specific extras beyond these vanilla numbers (Haven/Blockade's returned card, Sailor's
// optional trash, Tide Pools'/Sea Witch's forced discard, Pirate's Treasure gain) live in that
// function, parallel to how `resolve_effects_inner`'s match arms hold the "now" specifics beyond
// `CardDef`'s vanilla `cards`/`actions`/`buys`/`coins`.
// ---------------------------------------------------------------------------------------

/// (cards, actions, buys, coins) granted at the start of the owner's next turn, once per
/// `PendingDuration::times`. All-zero for cards whose next-turn part is card-specific only
/// (Haven, Blockade, Pirate) or nonexistent (Outpost: entirely a cleanup/turn-order effect).
pub fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
    match card {
        id::LIGHTHOUSE => (0, 0, 0, 1),
        id::ASTROLABE => (0, 0, 1, 1),
        id::FISHING_VILLAGE => (0, 1, 0, 1),
        id::MONKEY => (1, 0, 0, 0),
        id::CARAVAN => (1, 0, 0, 0),
        id::SAILOR => (0, 0, 0, 2),
        id::CORSAIR => (1, 0, 0, 0),
        id::MERCHANT_SHIP => (0, 0, 0, 2),
        id::SEA_WITCH => (2, 0, 0, 0),
        id::TACTICIAN => (5, 1, 1, 0),
        id::WHARF => (2, 0, 1, 0),
        _ => (0, 0, 0, 0),
    }
}

/// Duration cards (and Seaside cards generally) that need `GameState::run_gain_triggers`'s
/// cross-player checks (Monkey, Blockade, Pirate, Sailor): the analogue of
/// `GAIN_TRIGGER_CARDS_MASK`, gating `GameState::run_seaside_gain_triggers`'s whole body behind
/// one cheap bitmask test for the overwhelming majority of games with none of these.
pub const SEASIDE_GAIN_TRIGGER_MASK: u128 =
    (1u128 << id::MONKEY) | (1u128 << id::BLOCKADE) | (1u128 << id::PIRATE) | (1u128 << id::SAILOR);

pub const FIRST_KINGDOM: CardId = id::CELLAR;

#[inline(always)]
pub fn set_of(card: CardId) -> CardSet {
    CARDS[card as usize].set
}

/// Whether the card's effects are implemented (only those can be in a kingdom).
#[inline(always)]
pub fn is_ready(card: CardId) -> bool {
    CARDS[card as usize].ready
}

/// Whether `card` is a kingdom card (not a basic card: Copper..Curse, Platinum, Colony).
#[inline(always)]
pub fn is_kingdom(card: CardId) -> bool {
    card >= FIRST_KINGDOM && card != id::PLATINUM && card != id::COLONY
}

/// Basic cards that are only in some games' supply (Prosperity's Platinum and Colony). A kingdom
/// list may name them to include them.
#[inline(always)]
pub fn is_optional_basic(card: CardId) -> bool {
    card == id::PLATINUM || card == id::COLONY
}

/// Every kingdom card of every set that can be played (implemented).
pub fn kingdom_cards() -> impl Iterator<Item = CardId> {
    (FIRST_KINGDOM..NUM_CARDS as CardId).filter(|&c| is_kingdom(c) && is_ready(c))
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
    while out.len() < 10 && !pool.is_empty() {
        let i = rng.below(pool.len() as u32) as usize;
        out.push(pool.swap_remove(i));
    }
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Action or Treasure card (plus Curse, which is a Treasure when Charlatan is in the
    /// game) must say whether playing it is choice-free (`choice_free(..)` or `has_choice(..)`
    /// in `CARDS`); bots skip the play-order search only for choice-free hands. When adding
    /// cards, mark each one and update the list below deliberately.
    #[test]
    fn every_action_is_marked_choice_free_or_not() {
        for (i, d) in CARDS.iter().enumerate() {
            let marked = d.types & (ACTION | TREASURE) != 0 || i as CardId == id::CURSE;
            assert_eq!(d.on_play != OnPlay::NotAction, marked, "{} (card {i}) must be marked with choice_free/has_choice iff it is an Action or Treasure", d.name);
        }
        let free: Vec<&str> = CARDS.iter().filter(|d| d.on_play == OnPlay::ChoiceFree).map(|d| d.name).collect();
        assert_eq!(
            free,
            [
                "Copper", "Silver", "Gold", "Curse",
                "Moat", "Merchant", "Village", "Militia", "Smithy", "Council Room", "Festival", "Laboratory", "Market", "Witch",
                "Bridge", "Torturer", "Harem",
                "Lighthouse", "Astrolabe", "Fishing Village", "Monkey", "Caravan", "Cutpurse", "Sailor", "Tide Pools", "Bazaar", "Corsair",
                "Merchant Ship", "Outpost", "Pirate", "Sea Witch", "Treasury", "Wharf",
                "Clerk", "Monument", "Quarry", "Worker's Village", "Charlatan", "City", "Collection", "Rabble", "Grand Market",
                "Hoard", "Peddler", "Platinum",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
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
    }

    #[test]
    fn set_by_name_matches_common_spellings() {
        assert_eq!(set_by_name("Base"), Some(CardSet::Base));
        assert_eq!(set_by_name("base"), Some(CardSet::Base));
        assert_eq!(set_by_name(" Intrigue "), Some(CardSet::Intrigue));
        assert_eq!(set_by_name("intrigue"), Some(CardSet::Intrigue));
        assert_eq!(set_by_name("Seaside"), Some(CardSet::Seaside));
        assert_eq!(set_by_name("prosperity"), Some(CardSet::Prosperity));
        assert_eq!(set_by_name("Alchemy"), None);
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
}
