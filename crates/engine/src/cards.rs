//! Card identities and static definitions: Base Set and Intrigue, both 2nd edition.

pub type CardId = u8;
pub const NUM_CARDS: usize = 59;

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
}

/// The expansion a card comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardSet {
    /// Base Set 2nd edition, including the basic Treasure/Victory/Curse cards.
    Base,
    /// Intrigue 2nd edition.
    Intrigue,
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

/// An Intrigue card.
const fn intrigue(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Intrigue, ..d }
}

/// An Intrigue card whose effects aren't implemented yet (not allowed in kingdoms).
const fn intrigue_todo(d: CardDef) -> CardDef {
    CardDef { set: CardSet::Intrigue, ready: false, ..d }
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
    // ---- Intrigue (2nd edition). Stats are the vanilla part; the rest is in `effects.rs`. ----
    intrigue_todo(has_choice(c("Courtyard", 2, ACTION, 0, 0, 3, 0, 0))), // what to put on the deck
    intrigue_todo(has_choice(c("Lurker", 2, ACTION, 0, 0, 0, 1, 0))), // trash from Supply or gain from trash
    intrigue_todo(has_choice(c("Pawn", 2, ACTION, 0, 0, 0, 0, 0))), // two of four bonuses
    intrigue_todo(has_choice(c("Masquerade", 3, ACTION, 0, 0, 2, 0, 0))), // what to pass / trash
    intrigue_todo(has_choice(c("Shanty Town", 3, ACTION, 0, 0, 0, 2, 0))), // draws only with no Actions in hand: order matters
    intrigue_todo(has_choice(c("Steward", 3, ACTION, 0, 0, 0, 0, 0))), // one of three
    intrigue_todo(has_choice(c("Swindler", 3, ACTION | ATTACK, 2, 0, 0, 0, 0))), // what the victims gain
    intrigue_todo(has_choice(c("Wishing Well", 3, ACTION, 0, 0, 1, 1, 0))), // name a card
    intrigue_todo(has_choice(c("Baron", 4, ACTION, 0, 0, 0, 0, 1))), // discard an Estate?
    intrigue(choice_free(c("Bridge", 4, ACTION, 1, 0, 0, 0, 1))), // cost reduction only (no gains of its own)
    intrigue_todo(has_choice(c("Conspirator", 4, ACTION, 2, 0, 0, 0, 0))), // depends on actions played: order matters
    intrigue_todo(has_choice(c("Diplomat", 4, ACTION | REACTION, 0, 0, 2, 0, 0))), // depends on hand size: order matters
    intrigue_todo(has_choice(c("Ironworks", 4, ACTION, 0, 0, 0, 0, 0))), // what to gain
    intrigue_todo(has_choice(c("Mill", 4, ACTION | VICTORY, 0, 1, 1, 1, 0))), // discard 2?
    intrigue_todo(has_choice(c("Mining Village", 4, ACTION, 0, 0, 1, 2, 0))), // trash it?
    intrigue_todo(has_choice(c("Secret Passage", 4, ACTION, 0, 0, 2, 1, 0))), // what to put where in the deck
    intrigue_todo(has_choice(c("Courtier", 5, ACTION, 0, 0, 0, 0, 0))), // what to reveal, which bonuses
    intrigue(c("Duke", 5, VICTORY, 0, 0, 0, 0, 0)), // 1 VP per Duchy (`state::vp_of_cards`)
    intrigue_todo(has_choice(c("Minion", 5, ACTION | ATTACK, 0, 0, 0, 1, 0))), // +$2 or new hands
    intrigue_todo(has_choice(c("Patrol", 5, ACTION, 0, 0, 3, 0, 0))), // order of the cards put back
    intrigue_todo(has_choice(c("Replace", 5, ACTION | ATTACK, 0, 0, 0, 0, 0))), // what to trash and gain
    intrigue_todo(choice_free(c("Torturer", 5, ACTION | ATTACK, 0, 0, 3, 0, 0))), // only the victims choose
    intrigue_todo(has_choice(c("Trading Post", 5, ACTION, 0, 0, 0, 0, 0))), // what to trash
    intrigue_todo(has_choice(c("Upgrade", 5, ACTION, 0, 0, 1, 1, 0))), // what to trash and gain
    intrigue(c("Harem", 6, TREASURE | VICTORY, 2, 2, 0, 0, 0)),
    intrigue_todo(has_choice(c("Nobles", 6, ACTION | VICTORY, 0, 2, 0, 0, 0))), // +3 Cards or +2 Actions
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

#[inline(always)]
pub fn set_of(card: CardId) -> CardSet {
    CARDS[card as usize].set
}

/// Whether the card's effects are implemented (only those can be in a kingdom).
#[inline(always)]
pub fn is_ready(card: CardId) -> bool {
    CARDS[card as usize].ready
}

/// Every kingdom card of every set that can be played (implemented).
pub fn kingdom_cards() -> impl Iterator<Item = CardId> {
    (FIRST_KINGDOM..NUM_CARDS as CardId).filter(|&c| is_ready(c))
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
            ["Moat", "Merchant", "Village", "Militia", "Smithy", "Council Room", "Festival", "Laboratory", "Market", "Witch", "Bridge", "Torturer"],
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
        let in_set = |set| (FIRST_KINGDOM..NUM_CARDS as CardId).filter(|&c| set_of(c) == set).count();
        assert_eq!(in_set(CardSet::Base), 26);
        assert_eq!(in_set(CardSet::Intrigue), 26);
        // Every card name is unique.
        for a in 0..NUM_CARDS as CardId {
            assert_eq!(by_name(name(a)), Some(a), "{}", name(a));
        }
        assert!(is(id::HAREM, TREASURE) && is(id::HAREM, VICTORY));
        assert!(is(id::MILL, ACTION) && is(id::MILL, VICTORY));
        assert_eq!(kingdom_cards_in(CardSet::Base).count(), 26);
    }
}
