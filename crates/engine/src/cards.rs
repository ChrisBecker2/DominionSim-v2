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
    /// Plays another Action card this many times (Throne Room: 2); 0 if it doesn't.
    pub plays: u8,
    /// Whether playing it involves a choice for its player (see [`OnPlay`]). Every Action card
    /// must be marked explicitly with `choice_free(..)` or `has_choice(..)`.
    pub on_play: OnPlay,
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
    CardDef { name, cost, types, coins, vp, cards, actions, buys, plays: 0, on_play: OnPlay::NotAction }
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
    c("Copper", 0, TREASURE, 1, 0, 0, 0, 0),
    c("Silver", 3, TREASURE, 2, 0, 0, 0, 0),
    c("Gold", 6, TREASURE, 3, 0, 0, 0, 0),
    c("Estate", 2, VICTORY, 0, 1, 0, 0, 0),
    c("Duchy", 5, VICTORY, 0, 3, 0, 0, 0),
    c("Province", 8, VICTORY, 0, 6, 0, 0, 0),
    c("Curse", 0, CURSE_T, 0, -1, 0, 0, 0),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Action card must say whether playing it is choice-free (`choice_free(..)` or
    /// `has_choice(..)` in `CARDS`); bots skip the play-order search only for choice-free hands.
    /// When adding cards, mark each one and update the list below deliberately.
    #[test]
    fn every_action_is_marked_choice_free_or_not() {
        for (i, d) in CARDS.iter().enumerate() {
            let is_action = d.types & ACTION != 0;
            assert_eq!(d.on_play != OnPlay::NotAction, is_action, "{} (card {i}) must be marked with choice_free/has_choice iff it is an Action", d.name);
        }
        let free: Vec<&str> = CARDS.iter().filter(|d| d.on_play == OnPlay::ChoiceFree).map(|d| d.name).collect();
        assert_eq!(
            free,
            ["Moat", "Merchant", "Village", "Militia", "Smithy", "Council Room", "Festival", "Laboratory", "Market", "Witch"],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
    }
}
