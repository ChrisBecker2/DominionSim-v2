//! Plunder (40 kingdom cards, 15 Loot cards, 15 Events and 15 Traits). See `docs/plunder-plan.md` for the design and the rulings.

use super::prelude::*;
use crate::cards::{pl, LOOT};
use crate::state::{
    GameConfig, PendingDuration, WaitEntry, GF_ANY_COST, G_DISCARD_HOOK, LOOT_CHANCE, LV_CLEAN, LV_GAIN, LV_PLAY, LV_SHUFFLE, LV_WAIT, PF_ANY, PF_CLEAN, PF_COST, PF_LOOT, PF_SHUFFLE,
    PF_START, PF_TRAIT_GAIN, PF_TRAIT_PLAY, Z_CAGE, Z_EOT_DECK, Z_EOT_DOWN, Z_EOT_UP, Z_HOLDERS, Z_HOLDER0, Z_PREP,
};

/// Card ids of this set. The kingdom cards are the highest kingdom ids (after Lich, before the Loot and the Ruins and Knights); the Loot cards follow them; the Events follow the
/// Menagerie Events; the Traits are the highest ids of all.
pub mod ids {
    use crate::cards::CardId;
    pub const CAGE: CardId = 487;
    pub const GROTTO: CardId = 488;
    pub const JEWELLED_EGG: CardId = 489;
    pub const SEARCH: CardId = 490;
    pub const SHAMAN: CardId = 491;
    pub const SECLUDED_SHRINE: CardId = 492;
    pub const SIREN: CardId = 493;
    pub const STOWAWAY: CardId = 494;
    pub const TASKMASTER: CardId = 495;
    pub const ABUNDANCE: CardId = 496;
    pub const CABIN_BOY: CardId = 497;
    pub const CRUCIBLE: CardId = 498;
    pub const FLAGSHIP: CardId = 499;
    pub const FORTUNE_HUNTER: CardId = 500;
    pub const GONDOLA: CardId = 501;
    pub const HARBOR_VILLAGE: CardId = 502;
    pub const LANDING_PARTY: CardId = 503;
    pub const MAPMAKER: CardId = 504;
    pub const MAROON: CardId = 505;
    pub const ROPE: CardId = 506;
    pub const SWAMP_SHACKS: CardId = 507;
    pub const TOOLS: CardId = 508;
    pub const BURIED_TREASURE: CardId = 509;
    pub const CREW: CardId = 510;
    pub const CUTTHROAT: CardId = 511;
    pub const ENLARGE: CardId = 512;
    pub const FIGURINE: CardId = 513;
    pub const FIRST_MATE: CardId = 514;
    pub const FRIGATE: CardId = 515;
    pub const LONGSHIP: CardId = 516;
    pub const MINING_ROAD: CardId = 517;
    pub const PENDANT: CardId = 518;
    pub const PICKAXE: CardId = 519;
    pub const PILGRIM: CardId = 520;
    pub const QUARTERMASTER: CardId = 521;
    pub const SILVER_MINE: CardId = 522;
    pub const TRICKSTER: CardId = 523;
    pub const WEALTHY_VILLAGE: CardId = 524;
    pub const SACK_OF_LOOT: CardId = 525;
    pub const KINGS_CACHE: CardId = 526;
    // The Loot.
    pub const AMPHORA: CardId = 527;
    pub const DOUBLOONS: CardId = 528;
    pub const ENDLESS_CHALICE: CardId = 529;
    pub const FIGUREHEAD: CardId = 530;
    pub const HAMMER: CardId = 531;
    pub const INSIGNIA: CardId = 532;
    pub const JEWELS: CardId = 533;
    pub const ORB: CardId = 534;
    pub const PRIZE_GOAT: CardId = 535;
    pub const PUZZLE_BOX: CardId = 536;
    pub const SEXTANT: CardId = 537;
    pub const SHIELD: CardId = 538;
    pub const SPELL_SCROLL: CardId = 539;
    pub const STAFF: CardId = 540;
    pub const SWORD: CardId = 541;
    // The Events.
    pub const BURY: CardId = 622;
    pub const AVOID: CardId = 623;
    pub const DELIVER: CardId = 624;
    pub const PERIL: CardId = 625;
    pub const RUSH: CardId = 626;
    pub const FORAY: CardId = 627;
    pub const LAUNCH: CardId = 628;
    pub const MIRROR: CardId = 629;
    pub const PREPARE: CardId = 630;
    pub const SCROUNGE: CardId = 631;
    pub const JOURNEY: CardId = 632;
    pub const MAELSTROM: CardId = 633;
    pub const LOOTING: CardId = 634;
    pub const INVASION: CardId = 635;
    pub const PROSPER: CardId = 636;
    // The Traits.
    pub const CHEAP: CardId = 771;
    pub const CURSED: CardId = 772;
    pub const FATED: CardId = 773;
    pub const FAWNING: CardId = 774;
    pub const FRIENDLY: CardId = 775;
    pub const HASTY: CardId = 776;
    pub const INHERITED: CardId = 777;
    pub const INSPIRING: CardId = 778;
    pub const NEARBY: CardId = 779;
    pub const PATIENT: CardId = 780;
    pub const PIOUS: CardId = 781;
    pub const RECKLESS: CardId = 782;
    pub const RICH: CardId = 783;
    pub const SHY: CardId = 784;
    pub const TIRELESS: CardId = 785;
}

/// An Event: no types, no bonuses (what it does is `event_effect`); its cost is the cost to buy it.
const fn event(name: &'static str, cost: u8) -> CardDef {
    pl(c(name, cost, 0, 0, 0, 0, 0, 0))
}

/// A Trait: nothing to buy (what it does is the code that watches its pile).
const fn tr(name: &'static str) -> CardDef {
    pl(c(name, 0, 0, 0, 0, 0, 0, 0))
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::CAGE, pl(has_choice(c("Cage", 2, TREASURE | DURATION, 0, 0, 0, 0, 0)))), // sets aside up to 4 cards; waits for a Victory gain
    (ids::GROTTO, pl(order_sensitive(c("Grotto", 2, ACTION | DURATION, 0, 0, 0, 1, 0)))), // sets aside up to 4 cards, discards them and draws as many next turn
    (ids::JEWELLED_EGG, pl(on_trash(choice_free(c("Jewelled Egg", 2, TREASURE, 1, 0, 0, 0, 1))))), // a Loot when trashed
    (ids::SEARCH, pl(choice_free(c("Search", 2, ACTION | DURATION, 2, 0, 0, 0, 0)))), // a Loot when a Supply pile empties
    (ids::SHAMAN, pl(order_sensitive(c("Shaman", 2, ACTION, 1, 0, 0, 1, 0)))), // may trash a card; every start of turn gains from the trash
    (ids::SECLUDED_SHRINE, pl(choice_free(c("Secluded Shrine", 3, ACTION | DURATION, 1, 0, 0, 0, 0)))), // trashes up to 2 when a Treasure is gained
    (ids::SIREN, pl(choice_free(c("Siren", 3, ACTION | DURATION | ATTACK, 0, 0, 0, 0, 0)))), // Curses; draws to 8 next turn; trashed when gained unless an Action is trashed
    (ids::STOWAWAY, pl(choice_free(c("Stowaway", 3, ACTION | DURATION | REACTION, 0, 0, 0, 0, 0)))), // +2 Cards next turn; played when a Duration is gained
    (ids::TASKMASTER, pl(has_choice(c("Taskmaster", 3, ACTION | DURATION, 1, 0, 0, 1, 0)))), // repeats next turn if a $5 card was gained
    (ids::ABUNDANCE, pl(has_choice(c("Abundance", 4, TREASURE | DURATION, 0, 0, 0, 0, 0)))), // +1 Buy +$3 when an Action is gained
    (ids::CABIN_BOY, pl(has_choice(c("Cabin Boy", 4, ACTION | DURATION, 0, 0, 1, 1, 0)))), // next turn +$2 or trash it for a Duration
    (ids::CRUCIBLE, pl(has_choice(c("Crucible", 4, TREASURE, 0, 0, 0, 0, 0)))), // trashes a card for $ equal to its cost
    (ids::FLAGSHIP, pl(has_choice(c("Flagship", 4, ACTION | DURATION | COMMAND, 2, 0, 0, 0, 0)))), // replays the next non-Command Action
    (ids::FORTUNE_HUNTER, pl(order_sensitive(c("Fortune Hunter", 4, ACTION, 2, 0, 0, 0, 0)))), // may play a Treasure from the top 3
    (ids::GONDOLA, pl(has_choice(c("Gondola", 4, TREASURE | DURATION, 0, 0, 0, 0, 0)))), // $2 now or next turn; plays an Action when gained
    (ids::HARBOR_VILLAGE, pl(has_choice(c("Harbor Village", 4, ACTION, 0, 0, 1, 2, 0)))), // +$1 after the next Action if it gave +$
    (ids::LANDING_PARTY, pl(has_choice(c("Landing Party", 4, ACTION | DURATION, 0, 0, 2, 2, 0)))), // goes onto the deck when the first card of a turn is a Treasure
    (ids::MAPMAKER, pl(order_sensitive(c("Mapmaker", 4, ACTION | REACTION, 0, 0, 0, 0, 0)))), // takes 2 of the top 4; played when a Victory card is gained
    (ids::MAROON, pl(order_sensitive(c("Maroon", 4, ACTION, 0, 0, 0, 0, 0)))), // trashes a card for 2 Cards per type
    (ids::ROPE, pl(choice_free(c("Rope", 4, TREASURE | DURATION, 1, 0, 0, 0, 1)))), // next turn +1 Card and a trash
    (ids::SWAMP_SHACKS, pl(has_choice(c("Swamp Shacks", 4, ACTION, 0, 0, 0, 2, 0)))), // +1 Card per 3 cards in play
    (ids::TOOLS, pl(has_choice(c("Tools", 4, TREASURE, 0, 0, 0, 0, 0)))), // gains a copy of a card anyone has in play
    (ids::BURIED_TREASURE, pl(choice_free(c("Buried Treasure", 5, TREASURE | DURATION, 0, 0, 0, 0, 0)))), // next turn +1 Buy +$3; played when gained
    (ids::CREW, pl(choice_free(c("Crew", 5, ACTION | DURATION, 0, 0, 3, 0, 0)))), // goes onto the deck next turn
    (ids::CUTTHROAT, pl(choice_free(c("Cutthroat", 5, ACTION | DURATION | ATTACK, 0, 0, 0, 0, 0)))), // discard attack; a Loot when a $5 Treasure is gained
    (ids::ENLARGE, pl(order_sensitive(c("Enlarge", 5, ACTION | DURATION, 0, 0, 0, 0, 0)))), // remodels now and next turn
    (ids::FIGURINE, pl(has_choice(c("Figurine", 5, TREASURE, 0, 0, 2, 0, 0)))), // may discard an Action for +1 Buy +$1
    (ids::FIRST_MATE, pl(order_sensitive(c("First Mate", 5, ACTION, 0, 0, 0, 0, 0)))), // plays copies of an Action, draws to 6
    (ids::FRIGATE, pl(choice_free(c("Frigate", 5, ACTION | DURATION | ATTACK, 3, 0, 0, 0, 0)))), // others discard to 4 after each Action they play
    (ids::LONGSHIP, pl(choice_free(c("Longship", 5, ACTION | DURATION, 0, 0, 0, 2, 0)))), // +2 Cards next turn
    (ids::MINING_ROAD, pl(has_choice(c("Mining Road", 5, ACTION, 2, 0, 0, 1, 1)))), // may play a gained Treasure
    (ids::PENDANT, pl(has_choice(c("Pendant", 5, TREASURE, 0, 0, 0, 0, 0)))), // $1 per differently named Treasure in play
    (ids::PICKAXE, pl(has_choice(c("Pickaxe", 5, TREASURE, 1, 0, 0, 0, 0)))), // trashes a card; a Loot to the hand for $3 or more
    (ids::PILGRIM, pl(order_sensitive(c("Pilgrim", 5, ACTION, 0, 0, 4, 0, 0)))), // puts a card onto the deck
    (ids::QUARTERMASTER, pl(choice_free(c("Quartermaster", 5, ACTION | DURATION, 0, 0, 0, 0, 0)))), // each turn gains a card aside or takes one into the hand
    (ids::SILVER_MINE, pl(has_choice(c("Silver Mine", 5, TREASURE, 0, 0, 0, 0, 0)))), // gains a cheaper Treasure to the hand
    (ids::TRICKSTER, pl(choice_free(c("Trickster", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // Curses; may keep a Treasure for the next hand
    (ids::WEALTHY_VILLAGE, pl(choice_free(c("Wealthy Village", 5, ACTION, 0, 0, 1, 2, 0)))), // a Loot when gained with 3 differently named Treasures in play
    (ids::SACK_OF_LOOT, pl(has_choice(c("Sack of Loot", 6, TREASURE, 1, 0, 0, 0, 1)))), // gains a Loot
    (ids::KINGS_CACHE, pl(has_choice(c("King's Cache", 7, TREASURE, 0, 0, 0, 0, 0)))), // plays a Treasure three times
    // The Loot (non-Supply Treasures, $7 each).
    (ids::AMPHORA, pl(has_choice(c("Amphora", 7, TREASURE | DURATION | LOOT, 0, 0, 0, 0, 0)))), // +1 Buy +$3 now or next turn
    (ids::DOUBLOONS, pl(choice_free(c("Doubloons", 7, TREASURE | LOOT, 3, 0, 0, 0, 0)))), // a Gold when gained
    (ids::ENDLESS_CHALICE, pl(choice_free(c("Endless Chalice", 7, TREASURE | DURATION | LOOT, 1, 0, 0, 0, 1)))), // $1 +1 Buy every turn for the rest of the game
    (ids::FIGUREHEAD, pl(choice_free(c("Figurehead", 7, TREASURE | DURATION | LOOT, 3, 0, 0, 0, 0)))), // +2 Cards next turn
    (ids::HAMMER, pl(has_choice(c("Hammer", 7, TREASURE | LOOT, 3, 0, 0, 0, 0)))), // gains a card costing up to $4
    (ids::INSIGNIA, pl(has_choice(c("Insignia", 7, TREASURE | LOOT, 3, 0, 0, 0, 0)))), // gains may go onto the deck
    (ids::JEWELS, pl(choice_free(c("Jewels", 7, TREASURE | DURATION | LOOT, 3, 0, 0, 0, 1)))), // goes to the bottom of the deck next turn
    (ids::ORB, pl(has_choice(c("Orb", 7, TREASURE | LOOT, 0, 0, 0, 0, 0)))), // plays a card from the discard pile, or +1 Buy +$3
    (ids::PRIZE_GOAT, pl(has_choice(c("Prize Goat", 7, TREASURE | LOOT, 3, 0, 0, 0, 1)))), // may trash a card
    (ids::PUZZLE_BOX, pl(has_choice(c("Puzzle Box", 7, TREASURE | LOOT, 3, 0, 0, 0, 1)))), // keeps a card for the next hand
    (ids::SEXTANT, pl(has_choice(c("Sextant", 7, TREASURE | LOOT, 3, 0, 0, 0, 1)))), // sifts the top 5
    (ids::SHIELD, pl(choice_free(c("Shield", 7, TREASURE | REACTION | LOOT, 3, 0, 0, 0, 1)))), // blocks an Attack
    (ids::SPELL_SCROLL, pl(has_choice(c("Spell Scroll", 7, ACTION | TREASURE | LOOT, 0, 0, 0, 0, 0)))), // trashes itself to gain (and play) a cheaper card
    (ids::STAFF, pl(has_choice(c("Staff", 7, TREASURE | LOOT, 3, 0, 0, 0, 1)))), // may play an Action
    (ids::SWORD, pl(has_choice(c("Sword", 7, TREASURE | ATTACK | LOOT, 3, 0, 0, 0, 1)))), // others discard down to 4 (an Attack: played with its effects, not by the short path)
    // The Events.
    (ids::BURY, event("Bury", 1)),
    (ids::AVOID, event("Avoid", 2)),
    (ids::DELIVER, event("Deliver", 2)),
    (ids::PERIL, event("Peril", 2)),
    (ids::RUSH, event("Rush", 2)),
    (ids::FORAY, event("Foray", 3)),
    (ids::LAUNCH, event("Launch", 3)),
    (ids::MIRROR, event("Mirror", 3)),
    (ids::PREPARE, event("Prepare", 3)),
    (ids::SCROUNGE, event("Scrounge", 3)),
    (ids::JOURNEY, event("Journey", 4)),
    (ids::MAELSTROM, event("Maelstrom", 4)),
    (ids::LOOTING, event("Looting", 6)),
    (ids::INVASION, event("Invasion", 10)),
    (ids::PROSPER, event("Prosper", 10)),
    // The Traits.
    (ids::CHEAP, tr("Cheap")),
    (ids::CURSED, tr("Cursed")),
    (ids::FATED, tr("Fated")),
    (ids::FAWNING, tr("Fawning")),
    (ids::FRIENDLY, tr("Friendly")),
    (ids::HASTY, tr("Hasty")),
    (ids::INHERITED, tr("Inherited")),
    (ids::INSPIRING, tr("Inspiring")),
    (ids::NEARBY, tr("Nearby")),
    (ids::PATIENT, tr("Patient")),
    (ids::PIOUS, tr("Pious")),
    (ids::RECKLESS, tr("Reckless")),
    (ids::RICH, tr("Rich")),
    (ids::SHY, tr("Shy")),
    (ids::TIRELESS, tr("Tireless")),
];

/// The Plunder cards (see the module docs of `sets`).
pub struct Plunder;

// ---- codes: the continuation of a Plunder frame (`Then::Plu`), a Mode option (`ModeOpt::Plu`), a Filter (`Filter::Plu`) and a `PluAfter` frame's flags ----

/// `Then::Plu`: Cage's set-aside cards (Select) / Grotto's / Siren's choice / Maroon's draw / Figurine's bonus / Pickaxe's Loot / First Mate's name and loop / King's Cache.
pub const T_CAGE: u8 = 1;
pub const T_GROTTO: u8 = 2;
pub const T_SIREN: u8 = 3;
pub const T_MAROON: u8 = 4;
pub const T_FIGURINE: u8 = 5;
pub const T_PICKAXE: u8 = 6;
pub const T_FIRST_MATE_NAME: u8 = 7;
pub const T_FIRST_MATE_LOOP: u8 = 8;
pub const T_KINGS_CACHE: u8 = 9;
/// Quartermaster: the card gained is set aside on it (Gain); the card taken from it goes into the hand (Select).
pub const T_QM_ASIDE: u8 = 10;
/// Spell Scroll: the card gained may be played (Gain).
pub const T_SPELL_SCROLL: u8 = 11;
pub const T_FORAY: u8 = 12;
pub const T_PERIL: u8 = 13;
pub const T_PREPARE: u8 = 14;
/// Scrounge: an Estate gained from the trash (Select) is followed by a card up to $5.
pub const T_SCROUNGE: u8 = 15;
pub const T_INHERITED: u8 = 16;
pub const T_FRIENDLY: u8 = 17;
pub const T_SHY: u8 = 18;
pub const T_PUZZLE: u8 = 19;
/// A YesNo whose Yes is forced (a gained Buried Treasure or Rush's Action is played).
pub const T_FORCED_PLAY: u8 = 20;
/// A Loot gained (Invasion): it is played.
pub const T_LOOT_PLAY: u8 = 21;
/// Deliver: the card gained is set aside until the end of the turn. Hasty: it is set aside until the next turn.
pub const T_DELIVER: u8 = 22;
pub const T_HASTY: u8 = 23;
/// Prosper: a Treasure gained; the next one is offered.
pub const T_PROSPER: u8 = 24;
/// The end of the turn: the cards set aside go into the hand or onto the deck (under the Clean-up draw).
pub const T_EOT: u8 = 25;
/// The shuffle (Avoid, Fated) is done: the cards kept out go back to the discard pile.
pub const T_SHUFFLE: u8 = 26;
/// Trickster: the Treasures kept for the next hand.
pub const T_TRICKSTER: u8 = 27;
/// Quartermaster's pick of a card to take from a holder.
pub const T_QM_TAKE: u8 = 28;
/// Patient: each card picked is set aside for the next turn.
pub const T_PATIENT: u8 = 29;
/// Quartermaster: the card gained moves onto the holder `Frame::last` once the gain's own triggers have resolved (a Siren still trashes itself).
pub const T_QM_MOVE: u8 = 30;

/// `ModeOpt::Plu`.
pub const M_CABIN_TRASH: u8 = 1;
pub const M_ORB_PLAY: u8 = 2;
pub const M_SCROUNGE_GAIN: u8 = 3;
pub const M_QM_GAIN: u8 = 4;
pub const M_QM_TAKE: u8 = 5;

/// `Filter::Plu`: 0..15 are the cards of the pile that has the Trait `FIRST_TRAIT + k`.
pub const F_IN_PLAY: u8 = 20;

/// `FrameKind::PluAfter` flags (`Frame::min`).
pub const A_HARBOR: u8 = 1;
pub const A_INSPIRE: u8 = 2;
pub const A_FRIGATE: u8 = 4;
pub const A_LANDING: u8 = 8;
/// Cutthroat: the waiting entry is added once the victims have discarded.
pub const A_WAIT: u8 = 16;

/// The text of a Mode option of this set.
pub fn mode_label(k: u8) -> &'static str {
    match k {
        M_CABIN_TRASH => "Trash this to gain a Duration card",
        M_ORB_PLAY => "Play an Action or Treasure from your discard pile",
        M_SCROUNGE_GAIN => "Gain an Estate from the trash, then a card costing up to $5",
        M_QM_GAIN => "Gain a card costing up to $4, setting it aside",
        M_QM_TAKE => "Put a card set aside on this into your hand",
        _ => "?",
    }
}

#[inline(always)]
const fn trait_bit(t: CardId) -> u16 {
    1 << (t - cards::FIRST_TRAIT)
}

/// The cards, Events and Traits that gain Loot: a game with one of them has the Loot pile.
const LOOT_USERS: [CardId; 12] = [
    ids::JEWELLED_EGG, ids::SEARCH, ids::PICKAXE, ids::WEALTHY_VILLAGE, ids::CUTTHROAT, ids::SACK_OF_LOOT, ids::PERIL, ids::FORAY, ids::LOOTING, ids::INVASION, ids::PROSPER,
    ids::CURSED,
];

impl SetRules for Plunder {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Cards with a "when you gain" effect or a reaction to gains: Siren, Gondola, Buried Treasure, Stowaway, Mapmaker (ids below 512) and Wealthy Village.
    const GAIN_EXT: u128 = (1u128 << (ids::SIREN - 384))
        | (1u128 << (ids::GONDOLA - 384))
        | (1u128 << (ids::BURIED_TREASURE - 384))
        | (1u128 << (ids::STOWAWAY - 384))
        | (1u128 << (ids::MAPMAKER - 384));
    const GAIN_EXT2: u128 = 1u128 << (ids::WEALTHY_VILLAGE - 512);
    const BLOCK_REACTIONS: &'static [CardId] = &[ids::SHIELD];

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            ids::CABIN_BOY => &[ModeOpt::Coins(2), ModeOpt::Plu(M_CABIN_TRASH)],
            ids::ORB => &[ModeOpt::Plu(M_ORB_PLAY), ModeOpt::BuysCoins(1, 3)],
            ids::SCROUNGE => &[ModeOpt::TrashFromHand(1), ModeOpt::Plu(M_SCROUNGE_GAIN)],
            ids::QUARTERMASTER => &[ModeOpt::Plu(M_QM_GAIN), ModeOpt::Plu(M_QM_TAKE)],
            _ => &[],
        }
    }

    /// The start-of-turn parts of the Durations that are plain bonuses (cards, actions, buys, coins).
    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::STOWAWAY | ids::LONGSHIP | ids::FIGUREHEAD => (2, 0, 0, 0),
            ids::ROPE => (1, 0, 0, 0),
            ids::TASKMASTER => (0, 1, 0, 1),
            ids::BURIED_TREASURE | ids::AMPHORA => (0, 0, 1, 3),
            ids::GONDOLA => (0, 0, 0, 2),
            ids::ENDLESS_CHALICE => (0, 0, 1, 1),
            _ => (0, 0, 0, 0),
        }
    }

    /// The start-of-turn effects that change the hand or the deck, or draw (their order can matter).
    fn start_order_sensitive(card: CardId) -> bool {
        matches!(
            card,
            ids::GROTTO | ids::SIREN | ids::ROPE | ids::CREW | ids::ENLARGE | ids::JEWELS | ids::QUARTERMASTER | ids::SHAMAN | ids::PREPARE | ids::SHY | ids::STOWAWAY
                | ids::LONGSHIP | ids::FIGUREHEAD
        )
    }

    /// Two Grottos or Quartermasters with different holders are different effects.
    fn start_entry_named_by_arg(card: CardId) -> bool {
        matches!(card, ids::GROTTO | ids::QUARTERMASTER)
    }

    /// The Durations that schedule themselves: Grotto and Quartermaster once their holder is known, Gondola and Amphora when the player says "next turn", the Frigate when it hit
    /// somebody, Taskmaster when a $5 card is gained, and the "next time" Durations not at all (they wait).
    fn schedules_own_duration(card: CardId) -> bool {
        matches!(
            card,
            ids::GROTTO | ids::QUARTERMASTER | ids::GONDOLA | ids::AMPHORA | ids::FRIGATE | ids::TASKMASTER | ids::CAGE | ids::SEARCH | ids::SECLUDED_SHRINE | ids::ABUNDANCE
                | ids::CUTTHROAT | ids::FLAGSHIP | ids::LANDING_PARTY
        )
    }

    #[inline(never)]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        let own = p == g.turn.player;
        match card {
            ids::CAGE => g.stack.push(select(p, card, Zone::Hand, SetAside, Filter::Any, 0, 4, Then::Plu(T_CAGE))),
            ids::GROTTO => g.stack.push(select(p, card, Zone::Hand, SetAside, Filter::Any, 0, 4, Then::Plu(T_GROTTO))),
            ids::SEARCH | ids::SECLUDED_SHRINE | ids::ABUNDANCE | ids::FLAGSHIP | ids::LANDING_PARTY => g.plu_wait_add(p, card),
            ids::CUTTHROAT => {
                // The "next time" is set up after the others have discarded (a Tunnel's Gold gained by that discard does not count).
                g.stack.push(Frame { min: A_WAIT, ..Frame::new(K::PluAfter, p, card) });
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::Nothing) });
                }
            }
            ids::SHAMAN => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing)),
            ids::SIREN | ids::TRICKSTER => {
                let (vs, n) = g.victims(sink);
                g.push_gain_to_each(&vs[..n], id::CURSE, card);
                if card == ids::TRICKSTER && own {
                    g.turn.plu.trickster = g.turn.plu.trickster.saturating_add(1);
                    g.plu_refresh();
                }
            }
            ids::TASKMASTER => {
                if own {
                    g.turn.plu.tm_watch += 1;
                    g.plu_refresh();
                }
            }
            ids::CRUCIBLE => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::CoinsEqualToCostSum)),
            ids::FORTUNE_HUNTER => {
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Play, Filter::Treasure, 0, 1, Then::PlayPicked { times: 1 }));
                g.stack.push(Frame { max: 3, ..Frame::new(K::RevealTop, p, card) });
            }
            ids::GONDOLA | ids::AMPHORA => g.stack.push(Frame { then: Then::NowOrNext, ..Frame::new(K::Ask, p, card) }),
            ids::HARBOR_VILLAGE => {
                if own {
                    g.turn.plu.harbor += 1;
                    g.plu_refresh();
                }
            }
            ids::MAPMAKER => {
                g.stack.push(select(p, card, Zone::Revealed, ToHand, Filter::Any, 2, 2, Then::DiscardRevealed));
                g.stack.push(Frame { max: 4, ..Frame::new(K::RevealTop, p, card) });
            }
            ids::MAROON => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Plu(T_MAROON))),
            ids::SWAMP_SHACKS => {
                let n = (g.players[pi].in_play.total() / 3).min(u8::MAX as u32) as u8;
                if n > 0 {
                    g.stack.push(draw_frame(p, card, n));
                }
            }
            ids::TOOLS => g.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, card, u8::MAX, Filter::Plu(F_IN_PLAY), Dest::Discard) }),
            ids::ENLARGE => g.plu_remodel(p, card),
            ids::FIGURINE => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Action, 0, 1, Then::Plu(T_FIGURINE))),
            ids::FIRST_MATE => {
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::DrawUpTo(6)));
                g.stack.push(select(p, card, Zone::Hand, Choose, Filter::Action, 0, 1, Then::Plu(T_FIRST_MATE_NAME)));
            }
            ids::FRIGATE => {
                let (_, n) = g.victims(sink);
                if own && n > 0 {
                    g.players[pi].plu.frigate = g.players[pi].plu.frigate.saturating_add(1);
                    g.shield_lingering_attack(p, sink);
                    g.push_duration_pending(p, card, 1, 0);
                    g.plu_refresh();
                }
            }
            ids::MINING_ROAD => {
                if own {
                    g.turn.plu.road += 1;
                    g.plu_refresh();
                }
            }
            ids::PENDANT => {
                let ps = &g.players[pi];
                let n = ps.in_play.iter().filter(|&(c, _)| g.is_treasure(c)).count() as u16;
                if own && n > 0 {
                    g.turn.coins += n;
                    sink.bonus(p, card, 0, 0, n, 0);
                }
            }
            ids::PICKAXE => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Plu(T_PICKAXE))),
            ids::PILGRIM => g.stack.push(select(p, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing)),
            ids::QUARTERMASTER => g.plu_qm_play(p),
            ids::SILVER_MINE => {
                let cost = g.cost(card);
                g.stack.push(Frame { less: true, ..gain_frame(p, card, cost, Filter::Treasure, Dest::Hand) });
            }
            ids::SACK_OF_LOOT => g.push_loot(p, card, Dest::Discard, Then::Nothing),
            ids::KINGS_CACHE => g.stack.push(select(p, card, Zone::Hand, Play, g.treasure_filter(), 0, 1, Then::Plu(T_KINGS_CACHE))),
            // The Loot.
            ids::HAMMER => g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard)),
            ids::INSIGNIA => {
                if own {
                    g.turn.plu.insignia = true;
                    g.plu_refresh();
                }
            }
            ids::ORB => g.stack.push(mode_frame(p, card, 1)),
            ids::PRIZE_GOAT => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing)),
            ids::PUZZLE_BOX => g.stack.push(select(p, card, Zone::Hand, SetAside, Filter::Any, 0, 1, Then::Plu(T_PUZZLE))),
            ids::SEXTANT => {
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 0, ALL, Then::Nothing));
                g.stack.push(Frame { max: 5, ..Frame::new(K::RevealTop, p, card) });
            }
            ids::SPELL_SCROLL => {
                if g.leave_play_self_now(p, card) {
                    g.trash_card(p, card, true, sink);
                    g.flush_trash_triggers(0, sink);
                    g.stack.push(Frame { less: true, then: Then::Plu(T_SPELL_SCROLL), ..gain_frame(p, card, g.cost(card), Filter::Any, Dest::Discard) });
                }
            }
            ids::STAFF => g.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 1 })),
            ids::SWORD => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 4, Then::Nothing) });
                }
            }
            _ => {}
        }
    }

    /// The next-turn parts of the Durations that need more than a bonus.
    #[inline(never)]
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
        let pi = p as usize;
        match e.card {
            ids::GROTTO => {
                let h = e.arg;
                let cards_aside = g.players[pi].plu.ztake(h);
                g.players[pi].plu.holders &= !(1 << (h - Z_HOLDER0));
                g.players[pi].plu.qm &= !(1 << (h - Z_HOLDER0));
                let n = cards_aside.total().min(u8::MAX as u32) as u8;
                if n > 0 {
                    g.stack.push(Frame { depth: 1, ..draw_frame(p, e.card, n) });
                    let ps = &mut g.players[pi];
                    ps.discard.add_all(&cards_aside);
                    for (c, k) in cards_aside.iter() {
                        for _ in 0..k {
                            sink.event(Event::Discard { player: p, card: c });
                        }
                    }
                    if g.adv & G_DISCARD_HOOK != 0 {
                        g.discard_hook_counts(p, &cards_aside, 1, sink);
                    }
                }
            }
            ids::SIREN => g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Hand, Act::Discard, Filter::Any, 0, 0, Then::DrawUpTo(8)) }),
            ids::ROPE => {
                for _ in 0..e.times {
                    g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing) });
                }
            }
            ids::ENLARGE => {
                for _ in 0..e.times {
                    g.plu_remodel(p, e.card);
                }
            }
            ids::CABIN_BOY => {
                for _ in 0..e.times {
                    g.stack.push(Frame { depth: 1, ..mode_frame(p, e.card, 1) });
                }
            }
            ids::CREW | ids::JEWELS => {
                for _ in 0..e.times {
                    if g.remove_from_play(p, e.card) {
                        if e.card == ids::CREW {
                            g.cover_deck(p);
                            g.players[pi].deck_known.push_top(e.card);
                            sink.event(Event::Topdeck { player: p, card: e.card });
                        } else {
                            g.players[pi].deck_known_bottom.push_top(e.card);
                            sink.event(Event::Bottom { player: p, card: e.card });
                        }
                    }
                }
            }
            ids::ENDLESS_CHALICE => g.push_duration_pending(p, e.card, e.times, 0),
            ids::TASKMASTER => {
                g.turn.plu.tm_watch = g.turn.plu.tm_watch.saturating_add(e.times);
                g.plu_refresh();
            }
            ids::QUARTERMASTER => {
                // It stays in play for good: schedule the next turn's, then choose (once for each play).
                g.push_duration_pending(p, e.card, e.times, e.arg);
                for _ in 0..e.times {
                    g.stack.push(Frame { depth: 1, subject: e.arg, ..mode_frame(p, e.card, 1) });
                }
            }
            _ => {}
        }
    }

    /// Shaman gains from the trash at the start of every turn; Shy may discard a card for 2 Cards; Prepare plays the set-aside hand.
    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) -> bool {
        match e.card {
            ids::SHAMAN => {
                g.stack.push(Frame { depth: 0, ..select(p, e.card, Zone::Trash, Act::Gain, Filter::CostRange(0, 6), 1, 1, Then::Nothing) });
                true
            }
            ids::SHY => {
                g.stack.push(select(p, e.card, Zone::Hand, Act::Discard, Filter::Plu((ids::SHY - cards::FIRST_TRAIT) as u8), 0, 1, Then::Plu(T_SHY)));
                true
            }
            ids::PREPARE => {
                g.prepare_start(p);
                true
            }
            _ => false,
        }
    }

    /// The start-of-turn entries that are not pending Durations: Shaman and Shy for every player in a game with them, Prepare's set-aside hand.
    fn turn_start_entries(g: &mut GameState) {
        let p = g.turn.player;
        if g.plu.flags & PF_ANY == 0 {
            return;
        }
        if g.plu.flags & PF_START != 0 {
            if g.in_supply(ids::SHAMAN) {
                g.turn.start_pending.push(PendingDuration { card: ids::SHAMAN, times: 1, arg: 0, used: 0 });
            }
            if g.plu.traits & trait_bit(ids::SHY) != 0 && g.players[p as usize].hand.iter().any(|(c, _)| g.has_trait(ids::SHY, c)) {
                g.turn.start_pending.push(PendingDuration { card: ids::SHY, times: 1, arg: 0, used: 0 });
            }
        }
        if g.players[p as usize].plu.ztotal(Z_PREP) > 0 {
            g.turn.start_pending.push(PendingDuration { card: ids::PREPARE, times: 1, arg: 0, used: 0 });
        }
    }

    /// Jewelled Egg: a Loot when it is trashed.
    fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, _sink: &mut S) {
        if card == ids::JEWELLED_EGG {
            g.push_loot(p, card, Dest::Discard, Then::Nothing);
        }
    }

    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, _on_buy: bool, sink: &mut S) {
        g.plu_gain_triggers(p, c, to, sink);
    }

    /// Something of Plunder watches gains: a waiting Duration, a Trait, a this-turn effect, the Loot.
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.plu.live & LV_GAIN != 0
    }

    /// "When you discard this from play" offers: Trickster keeps Treasures for the next hand.
    fn offers_on_discard(g: &mut GameState) {
        let p = g.turn.player;
        if g.turn.plu.trickster > 0 {
            g.stack.push(select(p, ids::TRICKSTER, Zone::InPlay, Act::SetAside, Filter::TreasureToDiscard, 0, g.turn.plu.trickster, Then::Plu(T_TRICKSTER)));
        }
    }

    /// "At the start of Clean-up" offers: Friendly gains a copy for a discard, Patient sets cards aside to play next turn.
    fn offers_at_cleanup(g: &mut GameState) {
        let p = g.turn.player;
        if g.plu.traits & trait_bit(ids::FRIENDLY) != 0 && g.players[p as usize].hand.iter().any(|(c, _)| g.has_trait(ids::FRIENDLY, c)) {
            g.stack.push(select(p, ids::FRIENDLY, Zone::Hand, Act::Discard, Filter::Plu((ids::FRIENDLY - cards::FIRST_TRAIT) as u8), 0, 1, Then::Plu(T_FRIENDLY)));
        }
        if g.plu.traits & trait_bit(ids::PATIENT) != 0 && g.players[p as usize].hand.iter().any(|(c, _)| g.has_trait(ids::PATIENT, c)) {
            g.stack.push(select(p, ids::PATIENT, Zone::Hand, Act::SetAside, Filter::Plu((ids::PATIENT - cards::FIRST_TRAIT) as u8), 0, ALL, Then::Plu(T_PATIENT)));
        }
    }
}

// -------------------------------------------------------------------------------------------
// Set-up
// -------------------------------------------------------------------------------------------

/// Plunder's set-up: which effects are in the game, the Loot pile, the Traits and the piles they sit on.
pub(crate) fn setup(g: &mut GameState, cfg: &GameConfig, _n: usize) {
    // Young Witch's Bane and Ferryman's card are set up like kingdom cards ("do any set-up the card requires").
    let extra = [g.bane, g.ferryman];
    let mut flags = 0u8;
    let mut traits = 0u16;
    for &k in cfg.kingdom.iter().chain(extra.iter().filter(|&&c| c != 0)) {
        if cards::set_of(k) != CardSet::Plunder {
            continue;
        }
        flags |= PF_ANY;
        if LOOT_USERS.contains(&k) {
            flags |= PF_LOOT;
        }
        match k {
            ids::SHAMAN => flags |= PF_START,
            ids::CHEAP => flags |= PF_COST,
            ids::CURSED | ids::FAWNING | ids::HASTY | ids::NEARBY | ids::PIOUS | ids::RICH => flags |= PF_TRAIT_GAIN,
            ids::INSPIRING => flags |= PF_TRAIT_PLAY,
            ids::RECKLESS => flags |= PF_TRAIT_PLAY | PF_CLEAN,
            ids::FATED => flags |= PF_SHUFFLE,
            ids::SHY => flags |= PF_START,
            ids::FRIENDLY | ids::PATIENT | ids::TIRELESS => flags |= PF_CLEAN,
            _ => {}
        }
        if cards::is_trait(k) {
            traits |= trait_bit(k);
        }
    }
    // A Cursed pile gains Loot too; the Loot cards that can be gained by a Spell Scroll, an Orb... come from this pile only.
    if flags & PF_LOOT != 0 {
        for i in 0..cards::NUM_LOOT as CardId {
            g.supply.set(cards::FIRST_LOOT + i, 2);
        }
    }
    g.plu.flags = flags;
    g.plu.traits = traits;
    // Each Trait sits on a random Action or Treasure kingdom pile (one Trait per pile), unless the config names it.
    let mut used: Vec<CardId> = Vec::new();
    for t in cards::FIRST_TRAIT..cards::FIRST_TRAIT + cards::NUM_TRAITS as CardId {
        if traits & trait_bit(t) == 0 {
            continue;
        }
        let named = cfg.trait_piles.iter().find(|&&(tt, _)| tt == t).map(|&(_, pile)| cards::pile_of(pile));
        let pile = named.or_else(|| {
            let mut pool: Vec<CardId> = cfg
                .kingdom
                .iter()
                .copied()
                .filter(|&k| !cards::is_pseudo(k) && cards::is_kingdom(k) && (cards::is(k, ACTION) || cards::is(k, TREASURE)))
                .map(cards::pile_of)
                .filter(|pl| !used.contains(pl))
                .collect();
            pool.sort_unstable();
            pool.dedup();
            if pool.is_empty() {
                return None;
            }
            let mut r = crate::rng::Rng::new(cfg.seed ^ (0x9e37_79b9_7f4a_7c15u64.wrapping_mul(t as u64 + 1)));
            Some(pool[r.below(pool.len() as u32) as usize])
        });
        match pile {
            Some(pl) => {
                g.plu.trait_pile[(t - cards::FIRST_TRAIT) as usize] = pl;
                used.push(pl);
            }
            // No pile to put it on: the Trait is not in the game.
            None => g.plu.traits &= !trait_bit(t),
        }
    }
    // Traits that do nothing without a pile are off; the flags follow the Traits that are on.
    if g.plu.traits != traits {
        let mut f = flags & (PF_ANY | PF_LOOT | PF_START);
        for t in cards::FIRST_TRAIT..cards::FIRST_TRAIT + cards::NUM_TRAITS as CardId {
            if g.plu.traits & trait_bit(t) != 0 {
                f |= match t {
                    ids::CHEAP => PF_COST,
                    ids::CURSED | ids::FAWNING | ids::HASTY | ids::NEARBY | ids::PIOUS | ids::RICH => PF_TRAIT_GAIN,
                    ids::INSPIRING => PF_TRAIT_PLAY,
                    ids::RECKLESS => PF_TRAIT_PLAY | PF_CLEAN,
                    ids::FATED => PF_SHUFFLE,
                    ids::SHY => PF_START,
                    _ => PF_CLEAN,
                };
            }
        }
        g.plu.flags = f;
    }
    g.plu_refresh();
}

/// Inherited: after the opening deal is on the stack, each player (in turn order) chooses the starting card the Inherited card replaces.
pub(crate) fn setup_inherited(g: &mut GameState) {
    if g.plu.traits & trait_bit(ids::INHERITED) == 0 {
        return;
    }
    for p in (0..g.num_players).rev() {
        g.stack.push(select(p, ids::INHERITED, Zone::StartDeck, Act::Choose, Filter::Any, 1, 1, Then::Plu(T_INHERITED)));
    }
}

/// The Events of the set that can be bought now apart from their cost and the once-per-turn rule.
pub(crate) fn event_available(g: &GameState, e: CardId) -> bool {
    let p = g.turn.player as usize;
    let ps = &g.players[p];
    match e {
        // (Bury's +1 Buy comes even with an empty discard pile.)
        ids::PERIL => ps.hand.iter().any(|(c, _)| g.is_action(c)),
        ids::PREPARE => !ps.hand.is_empty(),
        ids::JOURNEY => !g.turn.plu.journey,
        // The others with 5 or more cards in hand trash one even when the buyer has nothing to trash.
        ids::MAELSTROM => !ps.hand.is_empty() || (0..g.num_players as usize).any(|q| q != p && g.players[q].hand.total() >= 5),
        ids::FORAY => !ps.hand.is_empty(),
        ids::LOOTING | ids::INVASION | ids::PROSPER => true,
        _ => true,
    }
}

/// What buying the Event `e` does, after its cost and Buy are paid.
#[inline(never)]
pub(crate) fn event_effect<S: EventSink>(g: &mut GameState, e: CardId, p: u8, sink: &mut S) {
    use Act::*;
    let plus_buy = |g: &mut GameState, sink: &mut S| {
        g.turn.buys += 1;
        sink.bonus(p, e, 0, 1, 0, 0);
    };
    match e {
        ids::BURY => {
            plus_buy(g, sink);
            g.stack.push(select(p, e, Zone::Discard, Bottom, Filter::Any, 1, 1, Then::Nothing));
        }
        ids::AVOID => {
            plus_buy(g, sink);
            g.turn.plu.avoid = g.turn.plu.avoid.saturating_add(1);
            g.plu_refresh();
        }
        ids::DELIVER => {
            plus_buy(g, sink);
            g.turn.plu.deliver = true;
            g.plu_refresh();
        }
        ids::PERIL => g.stack.push(select(p, e, Zone::Hand, Trash, Filter::Action, 0, 1, Then::Plu(T_PERIL))),
        ids::RUSH => {
            plus_buy(g, sink);
            g.turn.plu.rush = g.turn.plu.rush.saturating_add(1);
            g.plu_refresh();
        }
        ids::FORAY => g.stack.push(select(p, e, Zone::Hand, Discard, Filter::Any, 3, 3, Then::Plu(T_FORAY))),
        ids::LAUNCH => {
            // Back to the Action phase: +1 Card, +1 Action and +1 Buy.
            g.back_to_action_phase(p, sink);
            g.turn.actions += 1;
            g.turn.buys += 1;
            sink.bonus(p, e, 1, 1, 0, 0);
            g.stack.push(draw_frame(p, e, 1));
        }
        ids::MIRROR => {
            plus_buy(g, sink);
            g.turn.plu.mirror = g.turn.plu.mirror.saturating_add(1);
            g.plu_refresh();
        }
        ids::PREPARE => {
            let hand = g.players[p as usize].hand;
            g.players[p as usize].hand.clear();
            for (c, n) in hand.iter() {
                for _ in 0..n {
                    if g.players[p as usize].plu.zadd(Z_PREP, c, 1) {
                        sink.event(Event::SetAside { player: p, card: c });
                    } else {
                        // (More different cards than the set holds: the card stays in the hand.)
                        g.players[p as usize].hand.add(c, 1);
                    }
                }
            }
            g.plu_refresh();
        }
        ids::SCROUNGE => g.stack.push(mode_frame(p, e, 1)),
        ids::JOURNEY => {
            g.turn.plu.journey = true;
            g.plu_refresh();
        }
        ids::MAELSTROM => {
            // Everyone else with 5 or more cards trashes one (not an Attack: no reactions), after the player's own three.
            let others: Vec<u8> = g.others(p).collect();
            for &v in others.iter().rev() {
                if g.players[v as usize].hand.total() >= 5 {
                    g.stack.push(select(v, e, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Nothing));
                }
            }
            g.stack.push(select(p, e, Zone::Hand, Trash, Filter::Any, 3, 3, Then::Nothing));
        }
        ids::LOOTING => g.push_loot(p, e, Dest::Discard, Then::Nothing),
        ids::INVASION => {
            g.push_loot(p, e, Dest::Discard, Then::Plu(T_LOOT_PLAY));
            g.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, e, u8::MAX, Filter::Action, Dest::DeckTop) });
            g.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, e, u8::MAX, Filter::Card(id::DUCHY), Dest::Discard) });
            g.stack.push(select(p, e, Zone::Hand, Play, Filter::Types(crate::state::TypeSet::Attack), 0, 1, Then::PlayPicked { times: 1 }));
        }
        ids::PROSPER => {
            g.turn.named_for_war_chest.clear();
            g.push_prosper_gain(p, e);
            g.push_loot(p, e, Dest::Discard, Then::Nothing);
        }
        _ => {}
    }
}

// -------------------------------------------------------------------------------------------
// The state's methods
// -------------------------------------------------------------------------------------------

impl GameState {
    /// Whether `c` is a card of the pile that has the Trait `t`.
    #[inline]
    pub fn has_trait(&self, t: CardId, c: CardId) -> bool {
        let i = (t - cards::FIRST_TRAIT) as usize;
        self.plu.traits & (1 << i) != 0 && self.plu.trait_pile[i] == cards::pile_of(c)
    }

    /// The Trait `t`'s pile (a card standing for it), `None` when the Trait is not in the game.
    pub fn trait_pile(&self, t: CardId) -> Option<CardId> {
        let i = (t - cards::FIRST_TRAIT) as usize;
        (self.plu.traits & (1 << i) != 0).then(|| self.plu.trait_pile[i])
    }

    /// Puts the Trait `t` on the pile of `card` (for tests and set-up tools): the effects it has are live from now on.
    pub fn put_trait(&mut self, t: CardId, card: CardId) {
        let i = (t - cards::FIRST_TRAIT) as usize;
        self.plu.traits |= 1 << i;
        self.plu.trait_pile[i] = cards::pile_of(card);
        self.plu.flags |= PF_ANY
            | match t {
                ids::CHEAP => PF_COST,
                ids::CURSED => PF_TRAIT_GAIN | PF_LOOT,
                ids::FAWNING | ids::HASTY | ids::NEARBY | ids::PIOUS | ids::RICH => PF_TRAIT_GAIN,
                ids::INSPIRING => PF_TRAIT_PLAY,
                ids::RECKLESS => PF_TRAIT_PLAY | PF_CLEAN,
                ids::FATED => PF_SHUFFLE,
                ids::SHY => PF_START,
                _ => PF_CLEAN,
            };
        if self.plu.flags & PF_LOOT != 0 && self.supply.get(cards::FIRST_LOOT) == 0 {
            for k in 0..cards::NUM_LOOT as CardId {
                self.supply.set(cards::FIRST_LOOT + k, 2);
            }
        }
        self.plu_refresh();
    }

    /// Cheap: a card of the Cheap pile costs $1 less.
    #[inline(never)]
    pub(crate) fn plu_cost_reduction(&self, c: CardId) -> u8 {
        self.has_trait(ids::CHEAP, c) as u8
    }

    /// A card with a Trait that changes how it is played (Inspiring, Reckless) is not played by the short path for Treasures.
    #[inline(never)]
    pub(crate) fn plu_trait_play(&self, c: CardId) -> bool {
        self.has_trait(ids::INSPIRING, c) || self.has_trait(ids::RECKLESS, c)
    }

    /// Recomputes `PluState::live` from the flags, the waiting Durations and this turn's effects.
    #[inline(never)]
    pub fn plu_refresh(&mut self) {
        let f = self.plu.flags;
        if f == 0 {
            self.plu.live = 0;
            return;
        }
        let mut waiting = false;
        let mut play_wait = false;
        let mut frigate = false;
        let mut aside = false;
        for ps in self.players.iter() {
            if ps.plu.wait_len > 0 {
                waiting = true;
                play_wait |= ps.plu.waiting(ids::FLAGSHIP) + ps.plu.waiting(ids::LANDING_PARTY) > 0;
            }
            frigate |= ps.plu.frigate > 0;
            aside |= !ps.plu.z.is_empty();
        }
        let t = &self.turn.plu;
        let mut l = 0u8;
        if waiting {
            l |= LV_WAIT | LV_GAIN;
        }
        if f & (PF_TRAIT_GAIN | PF_LOOT) != 0 || t.rush > 0 || t.mirror > 0 || t.road > 0 || t.insignia || t.deliver || t.tm_watch > 0 {
            l |= LV_GAIN;
        }
        if f & PF_TRAIT_PLAY != 0 || play_wait || frigate || t.harbor > 0 {
            l |= LV_PLAY;
        }
        if f & PF_SHUFFLE != 0 || t.avoid > 0 {
            l |= LV_SHUFFLE;
        }
        if f & PF_CLEAN != 0 || t.trickster > 0 || t.journey || t.deliver || aside {
            l |= LV_CLEAN;
        }
        self.plu.live = l;
    }

    // ---- the Loot pile ----

    /// `p` gains the top card of the Loot pile to `dest` (a frame: the card is drawn when it runs).
    pub fn push_loot(&mut self, p: u8, source: CardId, dest: Dest, then: Then) {
        self.stack.push(Frame { dest, then, ..Frame::new(K::Loot, p, source) });
    }

    /// The next Loot of the pile: its index. The pile's order is not stored: the next card is one of those left, in proportion to the copies, or (in chance mode) a chance
    /// outcome the search enumerates. `Ok(None)` when the pile is empty.
    fn draw_loot(&mut self) -> Result<Option<usize>, NeedChance> {
        let first = cards::FIRST_LOOT;
        let total: u32 = (0..cards::NUM_LOOT as CardId).map(|i| self.supply.get(first + i) as u32).sum();
        if total == 0 {
            return Ok(None);
        }
        if self.plu.loot_top != 0 {
            let i = self.plu.loot_top as usize - 1;
            self.plu.loot_top = 0;
            return Ok(Some(i));
        }
        if self.chance_mode {
            return Err(NeedChance(LOOT_CHANCE));
        }
        let mut k = self.rng.below(total);
        for i in 0..cards::NUM_LOOT {
            let n = self.supply.get(first + i as CardId) as u32;
            if k < n {
                return Ok(Some(i));
            }
            k -= n;
        }
        Ok(None)
    }

    /// `FrameKind::Loot`: draw the card (a chance outcome in chance mode), reveal it and gain it.
    #[inline(never)]
    pub(crate) fn run_plu_loot<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        match self.draw_loot() {
            Err(NeedChance(q)) => return Run::Chance(q),
            Ok(None) => {
                self.stack.pop();
            }
            Ok(Some(i)) => {
                self.stack.pop();
                let c = cards::FIRST_LOOT + i as CardId;
                sink.event(Event::Reveal { player: p, card: c });
                // The gain runs the triggers of the card (Doubloons) and of the others that watch gains; it is a gain from a pile outside the Supply.
                let base = self.stack.len as usize;
                if self.gain(p, c, f.dest, false, f.source, sink) {
                    for fr in &mut self.stack.frames[base..self.stack.len as usize] {
                        fr.depth = f.depth;
                    }
                    if f.then == Then::Plu(T_LOOT_PLAY) {
                        let zone = GameState::gain_dest_zone(f.dest);
                        self.push_under(base, Frame {
                            zone, act: Act::Play, subject: c, then: Then::Plu(T_FORCED_PLAY), depth: f.depth, ..Frame::new(K::YesNo, p, f.source)
                        });
                    }
                }
            }
        }
        Run::Continue
    }

    /// Pushes `f` below the frames at `base..` (the gain's triggers run first, then `f`).
    fn push_under(&mut self, base: usize, f: Frame) {
        self.stack.push(f);
        let len = self.stack.len as usize;
        self.stack.frames[base..len].rotate_right(1);
    }

    // ---- "next time" Durations ----

    /// The Duration `card`, just played by `p`, waits in play for its event.
    pub(crate) fn plu_wait_add(&mut self, p: u8, card: CardId) {
        let mult = if self.turn.multiplier_card != 0 && self.turn.multiplier_card != card { self.turn.multiplier_card } else { 0 };
        // A card played by a Throne Room-like card that is tallying (see `push_duration_pending`) keeps it in play too.
        if self.turn.multiplier_card != 0 {
            self.turn.multiplier_successes += 1;
        }
        if card == ids::LANDING_PARTY {
            // Playing it is a play of this turn: the first card of the turn is past.
            self.turn.plu.first_by |= 1 << p;
        }
        let empty_now = if card == ids::SEARCH { Some(self.empty_piles().min(255) as u8) } else { None };
        let ps = &mut self.players[p as usize];
        if let Some(i) = if mult != 0 { ps.plu.wait_index(card, mult) } else { None } {
            ps.plu.wait[i].times = ps.plu.wait[i].times.saturating_add(1);
        } else if (ps.plu.wait_len as usize) < crate::state::PLU_WAIT_CAP {
            let i = ps.plu.wait_len as usize;
            ps.plu.wait[i] = WaitEntry { card, mult, times: 1 };
            ps.plu.wait_len += 1;
            if ps.stays.get(card) < ps.in_play.get(card) {
                ps.stays.add(card, 1);
            }
            if mult != 0 && ps.stays.get(mult) < ps.in_play.get(mult) {
                ps.stays.add(mult, 1);
            }
        }
        if let Some(n) = empty_now {
            self.plu.empty_seen = n;
        }
        self.plu_refresh();
    }

    /// The waiting Durations `card` of `q` fire: they go (trashed, or discarded at the end of the turn), and how many plays they stood for is returned.
    pub(crate) fn plu_wait_take<S: EventSink>(&mut self, q: u8, card: CardId, trash: bool, sink: &mut S) -> u8 {
        let mut times = 0u8;
        loop {
            let ps = &mut self.players[q as usize];
            let Some(i) = ps.plu.wait[..ps.plu.wait_len as usize].iter().position(|e| e.card == card) else { break };
            let e = ps.plu.wait[i];
            ps.plu.wait_remove(i);
            times = times.saturating_add(e.times);
            ps.stays.remove(card);
            if e.mult != 0 {
                ps.stays.remove(e.mult);
                // A Throne Room that was held for this Duration in the turn it was played goes to the discard pile at this turn's Clean-up.
                if q == self.turn.player {
                    self.turn.duration_held.remove_one(e.mult);
                }
            }
            if trash {
                if self.remove_from_play(q, card) {
                    self.trash_card(q, card, true, sink);
                }
                if e.mult != 0 && q != self.turn.player {
                    self.players[q as usize].discard_next_cleanup.add(e.mult, 1);
                }
            } else if q != self.turn.player {
                let ps = &mut self.players[q as usize];
                ps.discard_next_cleanup.add(card, 1);
                if e.mult != 0 {
                    ps.discard_next_cleanup.add(e.mult, 1);
                }
            }
        }
        self.plu_refresh();
        times
    }

    /// Whether `q` has a waiting Duration `card`.
    #[inline]
    fn plu_waits(&self, q: u8, card: CardId) -> bool {
        self.players[q as usize].plu.wait_len > 0 && self.players[q as usize].plu.waiting(card) > 0
    }

    /// A Supply pile may just have emptied (a gain, a trash from the Supply, an Exile): the Searches in play fire if more piles are empty than when one last looked.
    pub(crate) fn plu_check_empty<S: EventSink>(&mut self, sink: &mut S) {
        if self.plu.live & LV_WAIT == 0 || !(0..self.num_players).any(|q| self.plu_waits(q, ids::SEARCH)) {
            return;
        }
        let now = self.empty_piles().min(255) as u8;
        let before = std::mem::replace(&mut self.plu.empty_seen, now);
        if now <= before {
            return;
        }
        let n = self.num_players;
        for i in (0..n).rev() {
            let q = (self.turn.player + i) % n;
            if self.plu_waits(q, ids::SEARCH) {
                let times = self.plu_wait_take(q, ids::SEARCH, true, sink);
                for _ in 0..times {
                    self.push_loot(q, ids::SEARCH, Dest::Discard, Then::Nothing);
                }
            }
        }
    }

    /// The Quartermaster `p` just played: a holder for its cards, and it stays in play for the rest of the game.
    pub(crate) fn plu_qm_play(&mut self, p: u8) {
        // A Throne Room-like card plays it twice but it is one Quartermaster with one set of cards: the holder the first play made is reused.
        let again = self.turn.multiplier_card != 0 && self.turn.multiplier_successes >= 1;
        let ps = &mut self.players[p as usize];
        if again {
            if let Some(i) = ps.pending_durations[..ps.pending_durations_len as usize].iter().rposition(|e| e.card == ids::QUARTERMASTER) {
                ps.pending_durations[i].times = ps.pending_durations[i].times.saturating_add(1);
                self.turn.multiplier_successes += 1;
                return;
            }
        }
        let Some(h) = (0..Z_HOLDERS).find(|&k| ps.plu.holders & (1 << k) == 0) else { return };
        ps.plu.holders |= 1 << h;
        ps.plu.qm |= 1 << h;
        if ps.stays.get(ids::QUARTERMASTER) < ps.in_play.get(ids::QUARTERMASTER) {
            ps.stays.add(ids::QUARTERMASTER, 1);
        }
        self.push_duration_pending(p, ids::QUARTERMASTER, 1, Z_HOLDER0 + h);
        self.plu_refresh();
    }

    /// Enlarge: trash a card from the hand and gain one costing up to $2 more.
    pub(crate) fn plu_remodel(&mut self, p: u8, source: CardId) {
        self.stack.push(select(
            p, source, Zone::Hand, Act::Trash, Filter::Any, 1, 1,
            Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
        ));
    }

    /// Prosper: an optional gain of a Treasure that has not been gained by it yet.
    pub(crate) fn push_prosper_gain(&mut self, p: u8, source: CardId) {
        self.stack.push(Frame {
            optional: true, excl_named: true, gf: GF_ANY_COST, then: Then::Plu(T_PROSPER), ..gain_frame(p, source, u8::MAX, Filter::Treasure, Dest::Discard)
        });
    }

    /// The Treasures `p` has in play, counted by name.
    fn plu_distinct_treasures(&self, p: u8) -> u32 {
        self.players[p as usize].in_play.iter().filter(|&(c, _)| self.is_treasure(c)).count() as u32
    }

    /// A frame for a card just gained (in `to`), naming it and its token: `subject` is the card, `dest` where it is.
    fn plu_gain_stamp(p: u8, source: CardId, c: CardId, to: Dest, tok: u8) -> Frame {
        Frame { subject: c, dest: to, tok, ..Frame::new(K::Select, p, source) }
    }

    /// Whether the card gained (`f.subject`, in `f.dest`) is still where it was gained.
    fn plu_gained_here(&self, f: &Frame) -> bool {
        (f.tok == 0 || self.tok_valid(f.tok)) && self.count_in(f.player, GameState::gain_dest_zone(f.dest), f.subject) > 0
    }

    /// Takes the card gained out of where it was gained; false when it is not there any more.
    fn plu_take_gained(&mut self, f: &Frame) -> bool {
        if !self.plu_gained_here(f) {
            return false;
        }
        let (p, c, z) = (f.player, f.subject, GameState::gain_dest_zone(f.dest));
        self.tok_moved(f.tok, p, z, c);
        let ps = &mut self.players[p as usize];
        match z {
            Zone::Hand => ps.hand.remove(c),
            Zone::Discard => ps.discard.remove(c),
            Zone::DeckTop => ps.deck_known.pop_top().is_some(),
            _ => false,
        }
    }

    // ---- gains ----

    /// The "when you gain" effects of Plunder: the cards' own, the Traits, the waiting Durations, this turn's effects and the reactions in the hands.
    #[inline(never)]
    pub(crate) fn plu_gain_triggers<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, sink: &mut S) {
        let pi = p as usize;
        let gt = self.turn.led.ingain;
        let zone = GameState::gain_dest_zone(to);
        let own_turn = p == self.turn.player;
        let waiting = self.plu.live & LV_WAIT != 0;
        // Siren: trashed unless an Action is trashed. Pushed first so it resolves after the other gain effects (Insignia may move it first).
        if c == ids::SIREN {
            self.stack.push(Frame { filter: Filter::Action, min: 0, max: 1, act: Act::Trash, ..Self::plu_gain_stamp(p, ids::SIREN, c, to, gt).with_then(Then::Plu(T_SIREN)) });
        }
        // ---- reactions in the hands (asked last of the things below that are pushed first: one offer for each copy) ----
        if self.in_supply(ids::MAPMAKER) && cards::is(c, VICTORY) {
            let n = self.num_players;
            for i in (0..n).rev() {
                let q = (self.turn.player + i) % n;
                for _ in 0..self.players[q as usize].hand.get(ids::MAPMAKER).min(4) {
                    self.stack.push(Frame { zone: Zone::Hand, act: Act::Play, subject: ids::MAPMAKER, ..Frame::new(K::YesNo, q, ids::MAPMAKER) });
                }
            }
        }
        if self.in_supply(ids::STOWAWAY) && cards::is(c, DURATION) {
            let n = self.num_players;
            for i in (0..n).rev() {
                let q = (self.turn.player + i) % n;
                for _ in 0..self.players[q as usize].hand.get(ids::STOWAWAY).min(4) {
                    self.stack.push(Frame { zone: Zone::Hand, act: Act::Play, subject: ids::STOWAWAY, ..Frame::new(K::YesNo, q, ids::STOWAWAY) });
                }
            }
        }
        // ---- this turn's effects (the turn player's gains) ----
        if own_turn && self.plu.live & LV_GAIN != 0 {
            // Mining Road: a Treasure may be played, once for each Road.
            if self.turn.plu.road > 0 && self.is_treasure(c) {
                self.turn.plu.road -= 1;
                self.stack.push(Frame { zone, act: Act::Play, subject: c, tok: gt, ..Frame::new(K::YesNo, p, ids::MINING_ROAD) });
            }
            // Insignia: the card may be put onto the deck.
            if self.turn.plu.insignia && to != Dest::DeckTop {
                self.stack.push(Frame { zone, act: Act::Topdeck, subject: c, tok: gt, ..Frame::new(K::YesNo, p, ids::INSIGNIA) });
            }
            // Mirror: a copy of the next Action gained, for each Mirror.
            if self.turn.plu.mirror > 0 && cards::is(c, ACTION) {
                let k = std::mem::take(&mut self.turn.plu.mirror);
                for _ in 0..k {
                    self.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, ids::MIRROR, u8::MAX, Filter::Card(c), Dest::Discard) });
                }
            }
            // Rush: the next Action gained is played.
            if self.turn.plu.rush > 0 && cards::is(c, ACTION) {
                // Every Rush waits for the same gain; the card can only be played once (the later ones find it gone).
                let k = std::mem::take(&mut self.turn.plu.rush);
                for _ in 0..k {
                    self.stack.push(Frame { zone, act: Act::Play, subject: c, then: Then::Plu(T_FORCED_PLAY), tok: gt, ..Frame::new(K::YesNo, p, ids::RUSH) });
                }
            }
            // Deliver: the card is set aside until the end of the turn.
            if self.turn.plu.deliver {
                self.stack.push(Self::plu_gain_stamp(p, ids::DELIVER, c, to, gt).with_then(Then::Plu(T_DELIVER)));
            }
            // Taskmaster: a card costing exactly $5 marks the Taskmasters played so far.
            if self.turn.plu.tm_watch > 0 && self.cost_when_gained(p, c, to) == 5 {
                let k = std::mem::take(&mut self.turn.plu.tm_watch);
                self.turn.plu.tm_marked = self.turn.plu.tm_marked.saturating_add(k);
                self.push_duration_pending(p, ids::TASKMASTER, k, 0);
            }
        }
        // ---- the waiting Durations ----
        if waiting {
            let n = self.num_players;
            // Cutthroat: any player's gain of a Treasure costing $5 or more gives a Loot to everyone who played one.
            if self.is_treasure(c) && self.cost(c) >= 5 {
                for i in (0..n).rev() {
                    let q = (self.turn.player + i) % n;
                    if self.plu_waits(q, ids::CUTTHROAT) {
                        let times = self.plu_wait_take(q, ids::CUTTHROAT, false, sink);
                        for _ in 0..times {
                            self.push_loot(q, ids::CUTTHROAT, Dest::Discard, Then::Nothing);
                        }
                    }
                }
            }
            // The gainer's own: Cage (a Victory card), Secluded Shrine (a Treasure), Abundance (an Action).
            if self.plu_waits(p, ids::CAGE) && cards::is(c, VICTORY) {
                let times = self.plu_wait_take(p, ids::CAGE, true, sink);
                if times > 0 {
                    let kept = self.players[pi].plu.ztake(Z_CAGE);
                    for (k, m) in kept.iter() {
                        if !self.players[pi].plu.zadd(Z_EOT_DOWN, k, m) {
                            self.players[pi].discard.add(k, m);
                        }
                    }
                }
            }
            if self.plu_waits(p, ids::SECLUDED_SHRINE) && self.is_treasure(c) {
                let times = self.plu_wait_take(p, ids::SECLUDED_SHRINE, false, sink);
                for _ in 0..times {
                    self.stack.push(select(p, ids::SECLUDED_SHRINE, Zone::Hand, Act::Trash, Filter::Any, 0, 2, Then::Nothing));
                }
            }
            if self.plu_waits(p, ids::ABUNDANCE) && cards::is(c, ACTION) {
                let times = self.plu_wait_take(p, ids::ABUNDANCE, false, sink);
                if times > 0 && own_turn {
                    self.turn.buys += times;
                    self.turn.coins += 3 * times as u16;
                    sink.bonus(p, ids::ABUNDANCE, 0, times, 3 * times as u16, 0);
                }
            }
            // Search: this gain may have emptied a pile.
            self.plu_check_empty(sink);
        }
        // ---- the Traits of the card's pile, and Fawning ----
        if self.plu.flags & PF_TRAIT_GAIN != 0 {
            if c == id::PROVINCE && self.plu.traits & trait_bit(ids::FAWNING) != 0 {
                self.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, ids::FAWNING, u8::MAX, Filter::Plu((ids::FAWNING - cards::FIRST_TRAIT) as u8), Dest::Discard) });
            }
            if self.plu.traits & trait_bit(ids::NEARBY) != 0 && self.has_trait(ids::NEARBY, c) && own_turn {
                self.turn.buys += 1;
                sink.bonus(p, ids::NEARBY, 0, 1, 0, 0);
            }
            if self.has_trait(ids::RICH, c) {
                self.stack.push(gain_frame(p, ids::RICH, u8::MAX, Filter::Card(id::SILVER), Dest::Discard));
            }
            if self.has_trait(ids::PIOUS, c) {
                self.stack.push(select(p, ids::PIOUS, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing));
            }
            if self.has_trait(ids::CURSED, c) {
                // The Loot, then the Curse (the card itself was gained first).
                self.stack.push(gain_frame(p, ids::CURSED, u8::MAX, Filter::Card(id::CURSE), Dest::Discard));
                self.push_loot(p, ids::CURSED, Dest::Discard, Then::Nothing);
            }
            if self.has_trait(ids::HASTY, c) {
                self.stack.push(Self::plu_gain_stamp(p, ids::HASTY, c, to, gt).with_then(Then::Plu(T_HASTY)));
            }
        }
        // ---- the card's own "when you gain this" ----
        match c {
            ids::WEALTHY_VILLAGE => {
                if self.plu_distinct_treasures(p) >= 3 {
                    self.push_loot(p, c, Dest::Discard, Then::Nothing);
                }
            }
            ids::GONDOLA => self.stack.push(select(p, c, Zone::Hand, Act::Play, Filter::Action, 0, 1, Then::PlayPicked { times: 1 })),
            ids::BURIED_TREASURE => {
                self.stack.push(Frame { zone, act: Act::Play, subject: c, then: Then::Plu(T_FORCED_PLAY), tok: gt, ..Frame::new(K::YesNo, p, c) });
            }
            ids::DOUBLOONS => self.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, c, u8::MAX, Filter::Card(id::GOLD), Dest::Discard) }),
            _ => {}
        }
    }

    /// A Plunder `Gain` frame gained `c` (to `dest`).
    #[inline(never)]
    pub(crate) fn plu_gained<S: EventSink>(&mut self, f: &Frame, c: CardId, dest: Dest, base: usize, _sink: &mut S) {
        let p = f.player;
        let Then::Plu(k) = f.then else { return };
        match k {
            T_QM_ASIDE => {
                // The card is set aside on the Quartermaster `f.subject`, if nothing moved it: after the gain's own triggers (a Siren trashes itself first).
                let gt = self.turn.led.gain;
                let frame = Frame { last: f.subject, depth: f.depth, ..Self::plu_gain_stamp(p, f.source, c, dest, gt) }.with_then(Then::Plu(T_QM_MOVE));
                self.push_under(base, frame);
            }
            T_SPELL_SCROLL => {
                if cards::is(c, ACTION) || self.is_treasure(c) {
                    // The play waits under the gain's own triggers (a Siren trashes itself first, and then cannot be played).
                    self.push_under(base, Frame {
                        zone: GameState::gain_dest_zone(dest), act: Act::Play, subject: c, depth: f.depth, ..Frame::new(K::YesNo, p, f.source)
                    });
                }
            }
            T_PROSPER => {
                self.turn.named_for_war_chest.add_or_drop(c, 1);
                self.push_prosper_gain(p, f.source);
            }
            _ => {}
        }
    }

    // ---- selections, questions, modes ----

    /// A `Select` frame of Plunder has finished (`f.count` picks, `f.last` the last).
    #[inline(never)]
    pub(crate) fn finish_plu_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let Then::Plu(k) = f.then else { return };
        let p = f.player;
        let pi = p as usize;
        match k {
            T_QM_MOVE => {
                let h = f.last;
                if self.players[pi].plu.holders & (1 << (h - Z_HOLDER0)) != 0 && self.plu_take_gained(f) {
                    if self.players[pi].plu.zadd(h, f.subject, 1) {
                        sink.event(Event::SetAside { player: p, card: f.subject });
                    } else {
                        self.players[pi].discard.add(f.subject, 1);
                    }
                }
            }
            T_CAGE | T_GROTTO => {
                let aside = self.players[pi].set_aside;
                self.players[pi].set_aside.clear();
                if k == T_CAGE {
                    let mut back = Counts::EMPTY;
                    for (c, n) in aside.iter() {
                        if !self.players[pi].plu.zadd(Z_CAGE, c, n) {
                            back.add(c, n);
                        }
                    }
                    self.players[pi].hand.add_all(&back);
                    self.plu_wait_add(p, f.source);
                } else if !aside.is_empty() {
                    let ps = &mut self.players[pi];
                    let free = (0..Z_HOLDERS).find(|&h| ps.plu.holders & (1 << h) == 0);
                    match free {
                        Some(h) => {
                            ps.plu.holders |= 1 << h;
                            for (c, n) in aside.iter() {
                                if !ps.plu.zadd(Z_HOLDER0 + h, c, n) {
                                    ps.hand.add(c, n);
                                }
                            }
                            self.push_duration_pending(p, f.source, 1, Z_HOLDER0 + h);
                        }
                        None => self.players[pi].hand.add_all(&aside),
                    }
                }
                self.plu_refresh();
            }
            T_SIREN => {
                if f.count == 0 && self.plu_take_gained(f) {
                    self.trash_card(p, ids::SIREN, true, sink);
                    self.flush_trash_triggers(f.depth, sink);
                }
            }
            T_MAROON => {
                if f.count > 0 {
                    let n = (2 * self.type_count(f.last)).min(u8::MAX as u32) as u8;
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, n) });
                }
            }
            T_FIGURINE => {
                if f.count > 0 && p == self.turn.player {
                    self.turn.buys += 1;
                    self.turn.coins += 1;
                    sink.bonus(p, f.source, 0, 1, 1, 0);
                }
            }
            T_PICKAXE => {
                if f.count > 0 && self.cost(f.last) >= 3 {
                    self.stack.push(Frame { dest: Dest::Hand, depth: f.depth, ..Frame::new(K::Loot, p, f.source) });
                }
            }
            T_FIRST_MATE_NAME => {
                if f.count > 0 {
                    self.stack.push(Frame {
                        depth: f.depth, ..select(p, f.source, Zone::Hand, Act::Play, Filter::Card(f.last), 0, 1, Then::Plu(T_FIRST_MATE_LOOP))
                    });
                }
            }
            T_FIRST_MATE_LOOP => {
                if f.count > 0 {
                    // The copy was played (its effects came first); another copy may be played too.
                    let next = Frame { count: 0, last: 0, ..*f };
                    self.stack.push(Frame { depth: f.depth, ..next });
                    self.push_plays(f, 1);
                }
            }
            T_KINGS_CACHE => {
                if f.count > 0 && !self.plain_treasure(f.last) {
                    self.push_plays(f, 3);
                }
            }
            T_FORAY => {
                // Three cards discarded with three different names (`subject` counts the names: picks come in card order).
                if f.count == 3 && f.subject == 3 {
                    self.push_loot(p, f.source, Dest::Discard, Then::Nothing);
                }
            }
            T_PERIL => {
                if f.count > 0 {
                    self.push_loot(p, f.source, Dest::Discard, Then::Nothing);
                }
            }
            T_PREPARE => self.prepare_next(f),
            T_SCROUNGE => {
                if f.count > 0 {
                    self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, 5, Filter::Any, Dest::Discard) });
                }
            }
            T_INHERITED => self.finish_inherited(f),
            T_FRIENDLY => {
                if f.count > 0 {
                    // A Friendly card (of the pile: a split pile gives any of its cards, the top one).
                    let t = ids::FRIENDLY;
                    self.stack.push(Frame {
                        gf: GF_ANY_COST, depth: f.depth, ..gain_frame(p, t, u8::MAX, Filter::Plu((t - cards::FIRST_TRAIT) as u8), Dest::Discard)
                    });
                }
            }
            T_SHY => {
                if f.count > 0 {
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 2) });
                }
            }
            T_PUZZLE => {
                let aside = self.players[pi].set_aside;
                self.players[pi].set_aside.clear();
                for (c, n) in aside.iter() {
                    if !self.players[pi].plu.zadd(Z_EOT_DOWN, c, n) {
                        self.players[pi].hand.add(c, n);
                    }
                }
                self.plu_refresh();
            }
            T_TRICKSTER => {
                let aside = self.players[pi].set_aside;
                self.players[pi].set_aside.clear();
                for (c, n) in aside.iter() {
                    if !self.players[pi].plu.zadd(Z_EOT_UP, c, n) {
                        self.players[pi].discard.add(c, n);
                    }
                }
                self.plu_refresh();
            }
            T_DELIVER => {
                // The card gained is set aside until the end of the turn, if it is still where it was gained.
                if self.plu_take_gained(f) {
                    if self.players[pi].plu.zadd(Z_EOT_UP, f.subject, 1) {
                        sink.event(Event::SetAside { player: p, card: f.subject });
                    } else {
                        self.players[pi].discard.add(f.subject, 1);
                    }
                    self.plu_refresh();
                }
            }
            T_HASTY => {
                if self.plu_take_gained(f) {
                    let ps = &mut self.players[pi];
                    if ps.farm_cards.has(f.subject) || ps.farm_cards.iter().count() < crate::state::FARM_CAP {
                        ps.farm_cards.add(f.subject, 1);
                        sink.event(Event::SetAside { player: p, card: f.subject });
                    } else {
                        ps.discard.add(f.subject, 1);
                    }
                }
            }
            T_EOT => self.plu_eot(p, sink),
            T_SHUFFLE => {
                let kept = self.players[pi].set_aside;
                self.players[pi].set_aside.clear();
                self.shuffle_discard(p, sink);
                self.players[pi].discard.add_all(&kept);
            }
            _ => {}
        }
    }

    /// A pick of a Plunder `Select` that moves the card at once (Patient's set-aside).
    pub(crate) fn plu_pick(&mut self, f: &Frame, c: CardId) {
        if f.then == Then::Plu(T_PATIENT) {
            let ps = &mut self.players[f.player as usize];
            if ps.set_aside.remove(c) {
                if ps.farm_cards.has(c) || ps.farm_cards.iter().count() < crate::state::FARM_CAP {
                    ps.farm_cards.add(c, 1);
                } else {
                    ps.hand.add(c, 1);
                }
            }
        }
    }

    /// Whether a forced play (a gained Buried Treasure, Rush's Action) can still happen: the copy is where it was gained.
    pub(crate) fn plu_forced_possible(&self, f: &Frame) -> bool {
        (f.tok == 0 || self.tok_valid(f.tok)) && self.count_in(f.player, f.zone, f.subject) > 0
    }

    /// A Plunder `YesNo` answered Yes: the card is already where its act put it.
    pub(crate) fn plu_yes<S: EventSink>(&mut self, _f: &Frame, _sink: &mut S) {}

    /// The frames of the Mode option `k` of `f.source` (pushed highest option first).
    pub(crate) fn plu_mode_frames(&mut self, f: &Frame, k: u8) {
        let p = f.player;
        match k {
            M_ORB_PLAY => self.stack.push(select(p, f.source, Zone::Discard, Act::Play, Filter::ActionOrTreasure, 1, 1, Then::PlayPicked { times: 1 })),
            M_SCROUNGE_GAIN => {
                if self.trash.has(id::ESTATE) {
                    self.stack.push(select(p, f.source, Zone::Trash, Act::Gain, Filter::Card(id::ESTATE), 1, 1, Then::Plu(T_SCROUNGE)));
                }
            }
            M_QM_GAIN => {
                self.stack.push(Frame { subject: f.subject, then: Then::Plu(T_QM_ASIDE), ..gain_frame(p, f.source, 4, Filter::Any, Dest::Discard) });
            }
            M_QM_TAKE => {
                let h = f.subject;
                self.stack.push(Frame { subject: h, ..select(p, f.source, Zone::PluAside, Act::ToHand, Filter::Any, 1, 1, Then::Plu(T_QM_TAKE)) });
            }
            _ => {}
        }
    }

    /// The immediate part of the Mode option `k`.
    pub(crate) fn plu_mode_now<S: EventSink>(&mut self, f: &Frame, k: u8, _sink: &mut S) {
        let p = f.player;
        if k == M_CABIN_TRASH && self.remove_from_play(p, f.source) {
            self.trash_card(p, f.source, true, _sink);
            self.flush_trash_triggers(f.depth, _sink);
            self.stack.push(Frame {
                gf: GF_ANY_COST, depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Types(crate::state::TypeSet::Duration), Dest::Discard)
            });
        }
    }

    /// Foray counts the names among the cards discarded; Patient moves each card as it is picked.
    #[inline(always)]
    pub(crate) fn plu_count_pick(&self, f: &mut Frame, c: CardId) {
        if f.then == Then::Plu(T_FORAY) && (f.count == 0 || c != f.last) {
            f.subject += 1;
        }
    }

    /// `Filter::Plu(k)` against the live game.
    pub(crate) fn plu_filter(&self, k: u8, c: CardId) -> bool {
        match k {
            0..=14 => self.has_trait(cards::FIRST_TRAIT + k as CardId, c),
            F_IN_PLAY => self.in_supply(c) && self.supply.get(c) > 0 && self.pile_open(c) && self.players.iter().any(|ps| ps.in_play.has(c)),
            _ => true,
        }
    }

    // ---- plays ----

    /// A card was played (`resolve_effects`) while Plunder watches plays: Flagship and Reckless play it again, and Harbor Village, Inspiring, a Frigate's Attack and Landing
    /// Party act after it. Called before the card's own frames are pushed.
    #[inline(never)]
    pub(crate) fn plu_play_hooks(&mut self, card: CardId, player: u8, tok: u8) {
        if std::mem::take(&mut self.turn.plu.skip_hooks) {
            return;
        }
        let pi = player as usize;
        let own = player == self.turn.player;
        let action = self.is_action(card);
        let first = self.turn.plu.first_by & (1 << player) == 0;
        self.turn.plu.first_by |= 1 << player;
        let mut flags = 0u8;
        let mut count = 0u8;
        if first && self.is_treasure(card) && self.plu_waits(player, ids::LANDING_PARTY) {
            flags |= A_LANDING;
        }
        if own && action && self.turn.plu.harbor > 0 {
            count = std::mem::take(&mut self.turn.plu.harbor);
            flags |= A_HARBOR;
        }
        if own && self.plu.traits & trait_bit(ids::INSPIRING) != 0 && self.has_trait(ids::INSPIRING, card) {
            flags |= A_INSPIRE;
        }
        if action {
            for q in self.others(player) {
                if self.players[q as usize].plu.frigate > 0 && self.players[q as usize].shielded & (1 << player) == 0 {
                    flags |= A_FRIGATE;
                }
            }
        }
        // The cards that play it again: Flagships waiting, and a Reckless card.
        let mut again = 0u8;
        if action && !cards::is(card, COMMAND) && self.plu_waits(player, ids::FLAGSHIP) {
            let times = self.plu_wait_take(player, ids::FLAGSHIP, false, &mut NoEvents);
            again = again.saturating_add(times);
        }
        let reckless = self.plu.traits & trait_bit(ids::RECKLESS) != 0 && self.has_trait(ids::RECKLESS, card);
        if flags != 0 {
            self.stack.push(Frame { subject: card, min: flags, count, cost_sum: self.turn.coins, tok, ..Frame::new(K::PluAfter, player, card) });
        }
        let zone = if tok == crate::track::NIP { Zone::Supply } else { Zone::Hand };
        let total = again + reckless as u8;
        for nth in (2..=total.saturating_add(1)).rev() {
            let source = if reckless && nth == total + 1 { card } else { ids::FLAGSHIP };
            self.stack.push(Frame { subject: card, count: nth, zone, tok, exact: true, ..Frame::new(K::PlayEffects, player, source) });
        }
        let _ = pi;
    }

    /// A token gave `n` $ with the play of a card: they are not the Action's own, so Harbor Village does not count them.
    pub(crate) fn plu_token_coins(&mut self, base: usize, n: u16) {
        for fr in &mut self.stack.frames[base..self.stack.len as usize] {
            if fr.kind == K::PluAfter && fr.min & A_HARBOR != 0 {
                fr.cost_sum += n;
            }
        }
    }

    /// After a card's effects: Harbor Village's $, Inspiring's extra play, a Frigate's discard, Landing Party going onto the deck.
    #[inline(never)]
    pub(crate) fn run_plu_after<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        self.stack.pop();
        let p = f.player;
        let pi = p as usize;
        if f.min & A_WAIT != 0 {
            self.plu_wait_add(p, f.source);
        }
        if f.min & A_HARBOR != 0 && self.turn.coins > f.cost_sum {
            self.turn.coins += f.count as u16;
            sink.bonus(p, ids::HARBOR_VILLAGE, 0, 0, f.count as u16, 0);
        }
        if f.min & A_INSPIRE != 0 {
            self.stack.push(Frame { depth: f.depth, ..select(p, ids::INSPIRING, Zone::Hand, Act::Play, Filter::NotInPlayAction, 0, 1, Then::PlayPicked { times: 1 }) });
        }
        if f.min & A_FRIGATE != 0 {
            self.stack.push(Frame { depth: f.depth, down_to_target: true, ..select(p, ids::FRIGATE, Zone::Hand, Act::Discard, Filter::Any, 0, 4, Then::Nothing) });
        }
        if f.min & A_LANDING != 0 {
            let times = self.plu_wait_take(p, ids::LANDING_PARTY, false, sink);
            for _ in 0..times.max(1).min(self.players[pi].in_play.get(ids::LANDING_PARTY)) {
                if self.remove_from_play(p, ids::LANDING_PARTY) {
                    self.cover_deck(p);
                    self.players[pi].deck_known.push_top(ids::LANDING_PARTY);
                    sink.event(Event::Topdeck { player: p, card: ids::LANDING_PARTY });
                }
            }
        }
        Run::Continue
    }

    /// The first Treasure of the turn is about to be played by the short path: Landing Party goes onto the deck (nothing of a plain Treasure draws, so before or after is the same).
    #[inline(never)]
    pub(crate) fn plu_first_treasure<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        if self.turn.plu.first_by & (1 << p) != 0 {
            return;
        }
        self.turn.plu.first_by |= 1 << p;
        if self.plu_waits(p, ids::LANDING_PARTY) {
            let times = self.plu_wait_take(p, ids::LANDING_PARTY, false, sink);
            for _ in 0..times.max(1) {
                if self.remove_from_play(p, ids::LANDING_PARTY) {
                    self.cover_deck(p);
                    self.players[p as usize].deck_known.push_top(ids::LANDING_PARTY);
                    sink.event(Event::Topdeck { player: p, card: ids::LANDING_PARTY });
                }
            }
        }
    }

    // ---- Prepare ----

    /// The start of the turn: the hand Prepare set aside is played, an Action or Treasure at a time in the order the player likes; the rest is discarded.
    pub(crate) fn prepare_start(&mut self, p: u8) {
        self.stack.push(Frame {
            subject: Z_PREP, ..select(p, ids::PREPARE, Zone::PluAside, Act::Play, Filter::ActionOrTreasure, 1, 1, Then::Plu(T_PREPARE))
        });
    }

    /// One of Prepare's cards was picked: it is played, and the next is picked after; with none left to play the rest is discarded.
    fn prepare_next(&mut self, f: &Frame) {
        let p = f.player;
        if f.count > 0 {
            let next = Frame { count: 0, last: 0, ..*f };
            self.stack.push(Frame { depth: f.depth, ..next });
            self.push_plays(f, 1);
        } else {
            let rest = self.players[p as usize].plu.ztake(Z_PREP);
            self.players[p as usize].discard.add_all(&rest);
        }
    }

    // ---- Inherited ----

    fn finish_inherited(&mut self, f: &Frame) {
        let p = f.player;
        if f.count == 0 {
            return;
        }
        let pile = self.plu.trait_pile[(ids::INHERITED - cards::FIRST_TRAIT) as usize];
        // The top card of the pile that is open (a split pile gives its first card in turn order).
        let card = self.supply_cards().find(|&x| cards::pile_of(x) == pile && self.supply.get(x) > 0 && self.pile_open(x));
        let Some(card) = card else { return };
        let ps = &mut self.players[p as usize];
        if !ps.deck_unknown.remove(f.last) {
            return;
        }
        // A replaced Copper goes back to its pile; anything else goes back to the box.
        if f.last == id::COPPER {
            self.supply.add(id::COPPER, 1);
        }
        self.supply.remove(card);
        self.players[p as usize].deck_unknown.add(card, 1);
        self.pile_taken(card);
    }

    // ---- the end of the turn, the shuffle ----

    /// Under the Clean-up draw: the cards set aside until the end of the turn go into the hand or onto the deck.
    fn plu_eot<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        let up = ps.plu.ztake(Z_EOT_UP);
        let down = ps.plu.ztake(Z_EOT_DOWN);
        let deck = ps.plu.ztake(Z_EOT_DECK);
        for hand in [up, down] {
            self.players[p as usize].hand.add_all(&hand);
            for (c, n) in hand.iter() {
                for _ in 0..n {
                    sink.event(Event::Draw { player: p, card: c });
                }
            }
        }
        for (c, n) in deck.iter() {
            for _ in 0..n {
                self.cover_deck(p);
                self.players[p as usize].deck_known.push_top(c);
                sink.event(Event::Topdeck { player: p, card: c });
            }
        }
        self.plu_refresh();
    }

    /// Cleanup: the cards of the play area that are discarded (`discard_part`) or kept (`kept`): Journey keeps them all, Tireless sets its cards aside for the deck, Reckless returns its
    /// cards to their pile.
    #[inline(never)]
    pub(crate) fn plu_cleanup_discards<S: EventSink>(&mut self, p: u8, play: &Counts, kept: &mut Counts, discard_part: &mut Counts, sink: &mut S) {
        if self.turn.plu.journey {
            // Nothing in play is discarded this turn; when no extra turn follows (Journey on an extra turn) it goes at the next Clean-up.
            if !self.turn.outpost_grants_extra {
                for (c, n) in discard_part.iter() {
                    self.players[p as usize].discard_next_cleanup.add(c, n);
                }
            }
            *kept = *play;
            *discard_part = Counts::EMPTY;
            return;
        }
        if self.plu.traits & (trait_bit(ids::TIRELESS) | trait_bit(ids::RECKLESS)) == 0 {
            return;
        }
        let cards: Vec<(CardId, u8)> = discard_part.iter().collect();
        for (c, n) in cards {
            if self.has_trait(ids::TIRELESS, c) {
                for _ in 0..n {
                    if self.players[p as usize].plu.zadd(Z_EOT_DECK, c, 1) {
                        discard_part.set(c, discard_part.get(c) - 1);
                        sink.event(Event::SetAside { player: p, card: c });
                    }
                }
            } else if self.has_trait(ids::RECKLESS, c) {
                discard_part.set(c, 0);
                for _ in 0..n {
                    self.return_to_own_pile(p, c, sink);
                }
            }
        }
    }

    /// A shuffle is about to happen for `p` (their deck is empty): Avoid lets the turn's player keep cards out of it, Fated lets them put Fated cards on top or at the bottom. A
    /// frame asks which, and shuffles once they have. Whether a frame was pushed.
    #[inline(never)]
    pub(crate) fn plu_shuffle_offer(&mut self, p: u8) -> bool {
        let pi = p as usize;
        let avoid = if p == self.turn.player { self.turn.plu.avoid as u32 * 3 } else { 0 };
        let fated = self.plu.traits & trait_bit(ids::FATED) != 0 && self.players[pi].discard.iter().any(|(c, _)| self.has_trait(ids::FATED, c));
        if avoid == 0 && !fated {
            return false;
        }
        if avoid > 0 {
            self.turn.plu.avoid = 0;
            self.plu_refresh();
        }
        // Pushed last-first: the cards kept out are picked first, then the Fated cards for the top, then those for the bottom, then the rest is shuffled.
        self.stack.push(select(p, ids::AVOID, Zone::Hand, Act::Discard, Filter::Any, 0, 0, Then::Plu(T_SHUFFLE)));
        if fated {
            let f = (ids::FATED - cards::FIRST_TRAIT) as u8;
            self.stack.push(select(p, ids::FATED, Zone::Discard, Act::Bottom, Filter::Plu(f), 0, ALL, Then::Nothing));
            self.stack.push(Frame { ordered: true, ..select(p, ids::FATED, Zone::Discard, Act::Topdeck, Filter::Plu(f), 0, ALL, Then::Nothing) });
        }
        if avoid > 0 {
            let n = avoid.min(self.players[pi].discard.total()).min(ALL as u32 - 1) as u8;
            self.stack.push(select(p, ids::AVOID, Zone::Discard, Act::SetAside, Filter::Any, 0, n, Then::Nothing));
        }
        true
    }
}

trait WithThen {
    fn with_then(self, then: Then) -> Self;
}

impl WithThen for Frame {
    fn with_then(self, then: Then) -> Self {
        Frame { then, ..self }
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
                "Jewelled Egg", "Search", "Secluded Shrine", "Siren", "Stowaway", "Rope",
                "Buried Treasure", "Crew", "Cutthroat", "Frigate", "Longship", "Quartermaster", "Trickster", "Wealthy Village",
                "Doubloons", "Endless Chalice", "Figurehead", "Jewels", "Shield",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            ["Grotto", "Shaman", "Fortune Hunter", "Mapmaker", "Maroon", "Enlarge", "First Mate", "Pilgrim"],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
        assert_eq!(
            names(cards::OnPlay::Choice),
            ["Cage", "Taskmaster", "Abundance", "Cabin Boy", "Crucible", "Flagship", "Gondola", "Harbor Village", "Landing Party", "Swamp Shacks", "Tools", "Figurine", "Mining Road", "Pendant", "Pickaxe", "Silver Mine", "Sack of Loot", "King's Cache", "Amphora", "Hammer", "Insignia", "Orb", "Prize Goat", "Puzzle Box", "Sextant", "Spell Scroll", "Staff", "Sword"]
        );
        assert_eq!(CARDS.len(), 40 + 15 + 15 + 15);
    }
}
