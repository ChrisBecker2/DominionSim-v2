//! Card identities and static definitions (Base Set, 2nd edition).

pub type CardId = u8;
pub const NUM_CARDS: usize = 33;

// Type flags.
pub const ACTION: u8 = 1;
pub const TREASURE: u8 = 2;
pub const VICTORY: u8 = 4;
pub const CURSE_T: u8 = 8;
pub const ATTACK: u8 = 16;
pub const REACTION: u8 = 32;

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
}

const fn c(name: &'static str, cost: u8, types: u8, coins: u8, vp: i8, cards: u8, actions: u8, buys: u8) -> CardDef {
    CardDef { name, cost, types, coins, vp, cards, actions, buys }
}

pub static CARDS: [CardDef; NUM_CARDS] = [
    c("Copper", 0, TREASURE, 1, 0, 0, 0, 0),
    c("Silver", 3, TREASURE, 2, 0, 0, 0, 0),
    c("Gold", 6, TREASURE, 3, 0, 0, 0, 0),
    c("Estate", 2, VICTORY, 0, 1, 0, 0, 0),
    c("Duchy", 5, VICTORY, 0, 3, 0, 0, 0),
    c("Province", 8, VICTORY, 0, 6, 0, 0, 0),
    c("Curse", 0, CURSE_T, 0, -1, 0, 0, 0),
    c("Cellar", 2, ACTION, 0, 0, 0, 1, 0),
    c("Chapel", 2, ACTION, 0, 0, 0, 0, 0),
    c("Moat", 2, ACTION | REACTION, 0, 0, 2, 0, 0),
    c("Harbinger", 3, ACTION, 0, 0, 1, 1, 0),
    c("Merchant", 3, ACTION, 0, 0, 1, 1, 0),
    c("Vassal", 3, ACTION, 2, 0, 0, 0, 0),
    c("Village", 3, ACTION, 0, 0, 1, 2, 0),
    c("Workshop", 3, ACTION, 0, 0, 0, 0, 0),
    c("Bureaucrat", 4, ACTION | ATTACK, 0, 0, 0, 0, 0),
    c("Gardens", 4, VICTORY, 0, 0, 0, 0, 0),
    c("Militia", 4, ACTION | ATTACK, 2, 0, 0, 0, 0),
    c("Moneylender", 4, ACTION, 0, 0, 0, 0, 0),
    c("Poacher", 4, ACTION, 1, 0, 1, 1, 0),
    c("Remodel", 4, ACTION, 0, 0, 0, 0, 0),
    c("Smithy", 4, ACTION, 0, 0, 3, 0, 0),
    c("Throne Room", 4, ACTION, 0, 0, 0, 0, 0),
    c("Bandit", 5, ACTION | ATTACK, 0, 0, 0, 0, 0),
    c("Council Room", 5, ACTION, 0, 0, 4, 0, 1),
    c("Festival", 5, ACTION, 2, 0, 0, 2, 1),
    c("Laboratory", 5, ACTION, 0, 0, 2, 1, 0),
    c("Library", 5, ACTION, 0, 0, 0, 0, 0),
    c("Market", 5, ACTION, 1, 0, 1, 1, 1),
    c("Mine", 5, ACTION, 0, 0, 0, 0, 0),
    c("Sentry", 5, ACTION, 0, 0, 1, 1, 0),
    c("Witch", 5, ACTION | ATTACK, 0, 0, 2, 0, 0),
    c("Artisan", 6, ACTION, 0, 0, 0, 0, 0),
];

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
    let norm = |x: &str| -> String { x.chars().filter(|c| !c.is_whitespace() && *c != '_' && *c != '-').flat_map(|c| c.to_lowercase()).collect() };
    let want = norm(s);
    if want.is_empty() {
        return None;
    }
    (0..NUM_CARDS as CardId).find(|&c| norm(name(c)) == want)
}

pub const FIRST_KINGDOM: CardId = id::CELLAR;

/// All 26 kingdom cards.
pub fn kingdom_cards() -> impl Iterator<Item = CardId> {
    FIRST_KINGDOM..NUM_CARDS as CardId
}

/// The recommended "First Game" kingdom from the 2E rulebook.
pub const FIRST_GAME: [CardId; 10] = [
    id::CELLAR, id::MARKET, id::MERCHANT, id::MILITIA, id::MINE,
    id::MOAT, id::REMODEL, id::SMITHY, id::VILLAGE, id::WORKSHOP,
];
