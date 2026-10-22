//! Allies: 25 kingdom piles and 6 split piles of 4 different cards that rotate, 23 Allies, Favors and the Liaisons. See `docs/allies-plan.md` for the design and the rulings.
//!
//! * **Favors** are a counter per player (`PlayerState::al.favors`). Spending them is the Ally's business: each Ally has one moment, at which an `Ask` / `YesNo` frame whose
//!   `source` is the Ally offers the ability (a `Then::AllyAsk` / `Then::AllyFavor` frame, `min` = what it costs), so bots and the page decide it like any other yes/no.
//! * **Split piles** keep each card's count in `supply` (all four cards of a pile are in `in_supply`), and their order in `AlState::piles`; only the top card can be gained or bought
//!   (`GameState::pile_open`, which the Buy decision reaches through `MAY_BUY_CARDS`). A card returned to its pile goes on top; rotating moves the top card and the copies of it directly
//!   under it to the bottom.
//! * The **Allies** are pseudo-cards (the highest ids), one per game; the Ally is in `AlState::ally`.

use super::prelude::*;
use crate::cards::{al, LIAISON};
use crate::state::{
    GameConfig, PileOrder, AF_ALLY, AF_BUY, AF_DISCARD, AF_FAMILY, AF_GAIN, AF_HW, AF_ISLAND, AF_KEEP, AF_PLAYS, AF_SCORE, AF_SHUFFLE, AF_SPLIT, AF_STAMP, AF_START, AH_HW,
    AH_LIMIT, AH_SKIP, AH_WL, AL_PILES, GF_ANY_COST, GF_ELDER, M_COST, VOYAGE_PLAYS,
};

/// Card ids of this set. The 25 single kingdom piles are the highest kingdom ids before the split piles (after the Horse); the 24 cards of the six split piles follow, four per pile
/// in the order top first (the first of each names the pile); the Ruins and Knights come after them. The 23 Allies are the highest ids of all (after the Exile label).
pub mod ids {
    use crate::cards::CardId;
    pub const BAUBLE: CardId = 438;
    pub const SYCOPHANT: CardId = 439;
    pub const IMPORTER: CardId = 440;
    pub const MERCHANT_CAMP: CardId = 441;
    pub const SENTINEL: CardId = 442;
    pub const UNDERLING: CardId = 443;
    pub const BROKER: CardId = 444;
    pub const CARPENTER: CardId = 445;
    pub const COURIER: CardId = 446;
    pub const INNKEEPER: CardId = 447;
    pub const ROYAL_GALLEY: CardId = 448;
    pub const TOWN: CardId = 449;
    pub const BARBARIAN: CardId = 450;
    pub const CAPITAL_CITY: CardId = 451;
    pub const CONTRACT: CardId = 452;
    pub const EMISSARY: CardId = 453;
    pub const GALLERIA: CardId = 454;
    pub const GUILDMASTER: CardId = 455;
    pub const HIGHWAYMAN: CardId = 456;
    pub const HUNTER: CardId = 457;
    pub const MODIFY: CardId = 458;
    pub const SKIRMISHER: CardId = 459;
    pub const SPECIALIST: CardId = 460;
    pub const SWAP: CardId = 461;
    pub const MARQUIS: CardId = 462;
    // The Augurs.
    pub const HERB_GATHERER: CardId = 463;
    pub const ACOLYTE: CardId = 464;
    pub const SORCERESS: CardId = 465;
    pub const SIBYL: CardId = 466;
    // The Clashes.
    pub const BATTLE_PLAN: CardId = 467;
    pub const ARCHER: CardId = 468;
    pub const WARLORD: CardId = 469;
    pub const TERRITORY: CardId = 470;
    // The Forts.
    pub const TENT: CardId = 471;
    pub const GARRISON: CardId = 472;
    pub const HILL_FORT: CardId = 473;
    pub const STRONGHOLD: CardId = 474;
    // The Odysseys.
    pub const OLD_MAP: CardId = 475;
    pub const VOYAGE: CardId = 476;
    pub const SUNKEN_TREASURE: CardId = 477;
    pub const DISTANT_SHORE: CardId = 478;
    // The Townsfolk.
    pub const TOWN_CRIER: CardId = 479;
    pub const BLACKSMITH: CardId = 480;
    pub const MILLER: CardId = 481;
    pub const ELDER: CardId = 482;
    // The Wizards.
    pub const STUDENT: CardId = 483;
    pub const CONJURER: CardId = 484;
    pub const SORCERER: CardId = 485;
    pub const LICH: CardId = 486;
    // The Allies.
    pub const ARCHITECTS_GUILD: CardId = 748;
    pub const BAND_OF_NOMADS: CardId = 749;
    pub const CAVE_DWELLERS: CardId = 750;
    pub const CIRCLE_OF_WITCHES: CardId = 751;
    pub const CITY_STATE: CardId = 752;
    pub const COASTAL_HAVEN: CardId = 753;
    pub const CRAFTERS_GUILD: CardId = 754;
    pub const DESERT_GUIDES: CardId = 755;
    pub const FAMILY_OF_INVENTORS: CardId = 756;
    pub const FELLOWSHIP_OF_SCRIBES: CardId = 757;
    pub const FOREST_DWELLERS: CardId = 758;
    pub const GANG_OF_PICKPOCKETS: CardId = 759;
    pub const ISLAND_FOLK: CardId = 760;
    pub const LEAGUE_OF_BANKERS: CardId = 761;
    pub const LEAGUE_OF_SHOPKEEPERS: CardId = 762;
    pub const MARKET_TOWNS: CardId = 763;
    pub const MOUNTAIN_FOLK: CardId = 764;
    pub const ORDER_OF_ASTROLOGERS: CardId = 765;
    pub const ORDER_OF_MASONS: CardId = 766;
    pub const PEACEFUL_CULT: CardId = 767;
    pub const PLATEAU_SHEPHERDS: CardId = 768;
    pub const TRAPPERS_LODGE: CardId = 769;
    pub const WOODWORKERS_GUILD: CardId = 770;
}

/// An Ally: nothing to buy, never in a zone (what it does is the code below).
const fn ally(name: &'static str) -> CardDef {
    al(c(name, 0, 0, 0, 0, 0, 0, 0))
}

/// The pile type bits: none (see `cards::sub_type`).
const CARDS: &[(CardId, CardDef)] = &[
    (ids::BAUBLE, al(has_choice(c("Bauble", 2, TREASURE | LIAISON, 0, 0, 0, 0, 0)))), // choose two: +1 Buy, +$1, +1 Favor, top-decking the gains
    (ids::SYCOPHANT, al(on_trash(order_sensitive(c("Sycophant", 2, ACTION | LIAISON, 0, 0, 0, 1, 0))))), // discards 3; +2 Favors when gained or trashed
    (ids::IMPORTER, al(choice_free(c("Importer", 3, ACTION | DURATION | LIAISON, 0, 0, 0, 0, 0)))), // gains a card next turn; +4 Favors in set-up
    (ids::MERCHANT_CAMP, al(choice_free(c("Merchant Camp", 3, ACTION, 1, 0, 0, 2, 0)))), // may go onto the deck when discarded from play
    (ids::SENTINEL, al(order_sensitive(c("Sentinel", 3, ACTION, 0, 0, 0, 0, 0)))), // trashes up to 2 of the top 5
    (ids::UNDERLING, al(choice_free(c("Underling", 3, ACTION | LIAISON, 0, 0, 1, 1, 0)))), // +1 Favor
    (ids::BROKER, al(order_sensitive(c("Broker", 4, ACTION | LIAISON, 0, 0, 0, 0, 0)))), // trashes a card, a benefit per $1
    (ids::CARPENTER, al(order_sensitive(c("Carpenter", 4, ACTION, 0, 0, 0, 0, 0)))), // a gain, or a remodel once a pile is empty
    (ids::COURIER, al(order_sensitive(c("Courier", 4, ACTION, 1, 0, 0, 0, 0)))), // discards the top card, plays from the discard pile
    (ids::INNKEEPER, al(order_sensitive(c("Innkeeper", 4, ACTION, 0, 0, 0, 1, 0)))), // draws and discards
    (ids::ROYAL_GALLEY, al(order_sensitive(c("Royal Galley", 4, ACTION | DURATION, 0, 0, 1, 0, 0)))), // plays an Action now and again next turn
    (ids::TOWN, al(has_choice(c("Town", 4, ACTION, 0, 0, 0, 0, 0)))), // +1 Card +2 Actions, or +1 Buy +$2
    (ids::BARBARIAN, al(choice_free(c("Barbarian", 5, ACTION | ATTACK, 2, 0, 0, 0, 0)))), // trashes the top card of each other deck
    (ids::CAPITAL_CITY, al(order_sensitive(c("Capital City", 5, ACTION, 0, 0, 1, 2, 0)))), // discards 2 for $2, pays $2 for 2 Cards
    (ids::CONTRACT, al(has_choice(c("Contract", 5, TREASURE | DURATION | LIAISON, 2, 0, 0, 0, 0)))), // +1 Favor; sets aside an Action for the next turn
    (ids::EMISSARY, al(choice_free(c("Emissary", 5, ACTION | LIAISON, 0, 0, 3, 0, 0)))), // +1 Action +2 Favors if the draw shuffled
    (ids::GALLERIA, al(choice_free(c("Galleria", 5, ACTION, 3, 0, 0, 0, 0)))), // +1 Buy when a card costing $3 or $4 is gained
    (ids::GUILDMASTER, al(choice_free(c("Guildmaster", 5, ACTION | LIAISON, 3, 0, 0, 0, 0)))), // +1 Favor per gain this turn
    (ids::HIGHWAYMAN, al(choice_free(c("Highwayman", 5, ACTION | DURATION | ATTACK, 0, 0, 0, 0, 0)))), // +3 Cards next turn; blocks the first Treasure of each other player
    (ids::HUNTER, al(order_sensitive(c("Hunter", 5, ACTION, 0, 0, 0, 1, 0)))), // an Action, a Treasure and a Victory card from the top 3
    (ids::MODIFY, al(order_sensitive(c("Modify", 5, ACTION, 0, 0, 0, 0, 0)))), // trashes a card, +1 Card +1 Action or a gain
    (ids::SKIRMISHER, al(choice_free(c("Skirmisher", 5, ACTION | ATTACK, 1, 0, 1, 1, 0)))), // a gained Attack makes the others discard to 3
    (ids::SPECIALIST, al(order_sensitive(c("Specialist", 5, ACTION, 0, 0, 0, 0, 0)))), // plays a card, then again or gains a copy
    (ids::SWAP, al(order_sensitive(c("Swap", 5, ACTION, 0, 0, 1, 1, 0)))), // returns an Action for another
    (ids::MARQUIS, al(order_sensitive(c("Marquis", 6, ACTION, 0, 0, 0, 0, 1)))), // a card per card in hand, discards down to 10
    // The Augurs.
    (ids::HERB_GATHERER, al(order_sensitive(c("Herb Gatherer", 3, ACTION, 0, 0, 0, 0, 1)))), // deck into the discard pile, may play a Treasure from it
    (ids::ACOLYTE, al(order_sensitive(c("Acolyte", 4, ACTION, 0, 0, 0, 0, 0)))), // trashes an Action or Victory card for a Gold; may trash itself for an Augur
    (ids::SORCERESS, al(order_sensitive(c("Sorceress", 5, ACTION | ATTACK, 0, 0, 0, 1, 0)))), // names the top card of its deck
    (ids::SIBYL, al(order_sensitive(c("Sibyl", 6, ACTION, 0, 0, 4, 1, 0)))), // a card on top of the deck, another on the bottom
    // The Clashes.
    (ids::BATTLE_PLAN, al(order_sensitive(c("Battle Plan", 3, ACTION, 0, 0, 1, 1, 0)))), // reveals an Attack for +1 Card; rotates any pile
    (ids::ARCHER, al(has_choice(c("Archer", 4, ACTION | ATTACK, 2, 0, 0, 0, 0)))), // discard one of the cards others reveal
    (ids::WARLORD, al(choice_free(c("Warlord", 5, ACTION | DURATION | ATTACK, 0, 0, 0, 1, 0)))), // +2 Cards next turn; others cannot play a third copy
    (ids::TERRITORY, al(c("Territory", 6, VICTORY, 0, 0, 0, 0, 0))), // 1 VP per differently named Victory card; Golds for empty piles when gained
    // The Forts.
    (ids::TENT, al(has_choice(c("Tent", 3, ACTION, 2, 0, 0, 0, 0)))), // may rotate; may go onto the deck when discarded from play
    (ids::GARRISON, al(choice_free(c("Garrison", 4, ACTION | DURATION, 2, 0, 0, 0, 0)))), // a token for each gain this turn, a card for each next turn
    (ids::HILL_FORT, al(has_choice(c("Hill Fort", 5, ACTION, 0, 0, 0, 0, 0)))), // a gain, into the hand or +1 Card +1 Action
    (ids::STRONGHOLD, al(has_choice(c("Stronghold", 6, ACTION | DURATION | VICTORY, 0, 2, 0, 0, 0)))), // +$3 or +3 Cards next turn
    // The Odysseys.
    (ids::OLD_MAP, al(order_sensitive(c("Old Map", 3, ACTION, 0, 0, 1, 1, 0)))), // discards a card and draws one; may rotate
    (ids::VOYAGE, al(choice_free(c("Voyage", 4, ACTION | DURATION, 0, 0, 0, 1, 0)))), // an extra turn with 3 plays
    (ids::SUNKEN_TREASURE, al(has_choice(c("Sunken Treasure", 5, TREASURE, 0, 0, 0, 0, 0)))), // gains an Action not in play
    (ids::DISTANT_SHORE, al(has_choice(c("Distant Shore", 6, ACTION | VICTORY, 0, 2, 2, 1, 0)))), // gains an Estate
    // The Townsfolk.
    (ids::TOWN_CRIER, al(has_choice(c("Town Crier", 2, ACTION, 0, 0, 0, 0, 0)))), // +$2, a Silver, or +1 Card +1 Action; may rotate
    (ids::BLACKSMITH, al(has_choice(c("Blacksmith", 3, ACTION, 0, 0, 0, 0, 0)))), // draws up to 6, 2 Cards, or +1 Card +1 Action
    (ids::MILLER, al(has_choice(c("Miller", 4, ACTION, 0, 0, 0, 1, 0)))), // one of the top 4 into the hand
    (ids::ELDER, al(order_sensitive(c("Elder", 5, ACTION, 2, 0, 0, 0, 0)))), // plays an Action with an extra choice
    // The Wizards.
    (ids::STUDENT, al(order_sensitive(c("Student", 3, ACTION | LIAISON, 0, 0, 0, 1, 0)))), // trashes a card; a Treasure gives a Favor and the Student goes onto the deck
    (ids::CONJURER, al(has_choice(c("Conjurer", 4, ACTION | DURATION, 0, 0, 0, 0, 0)))), // gains a card, comes back to the hand
    (ids::SORCERER, al(choice_free(c("Sorcerer", 5, ACTION | ATTACK, 0, 0, 1, 1, 0)))), // each other player names their top card
    (ids::LICH, al(on_trash(choice_free(c("Lich", 6, ACTION, 0, 0, 6, 2, 0))))), // skip a turn; a trashed Lich is discarded for a cheaper card from the trash
    // The Allies.
    (ids::ARCHITECTS_GUILD, ally("Architects' Guild")),
    (ids::BAND_OF_NOMADS, ally("Band of Nomads")),
    (ids::CAVE_DWELLERS, ally("Cave Dwellers")),
    (ids::CIRCLE_OF_WITCHES, ally("Circle of Witches")),
    (ids::CITY_STATE, ally("City-state")),
    (ids::COASTAL_HAVEN, ally("Coastal Haven")),
    (ids::CRAFTERS_GUILD, ally("Crafters' Guild")),
    (ids::DESERT_GUIDES, ally("Desert Guides")),
    (ids::FAMILY_OF_INVENTORS, ally("Family of Inventors")),
    (ids::FELLOWSHIP_OF_SCRIBES, ally("Fellowship of Scribes")),
    (ids::FOREST_DWELLERS, ally("Forest Dwellers")),
    (ids::GANG_OF_PICKPOCKETS, ally("Gang of Pickpockets")),
    (ids::ISLAND_FOLK, ally("Island Folk")),
    (ids::LEAGUE_OF_BANKERS, ally("League of Bankers")),
    (ids::LEAGUE_OF_SHOPKEEPERS, ally("League of Shopkeepers")),
    (ids::MARKET_TOWNS, ally("Market Towns")),
    (ids::MOUNTAIN_FOLK, ally("Mountain Folk")),
    (ids::ORDER_OF_ASTROLOGERS, ally("Order of Astrologers")),
    (ids::ORDER_OF_MASONS, ally("Order of Masons")),
    (ids::PEACEFUL_CULT, ally("Peaceful Cult")),
    (ids::PLATEAU_SHEPHERDS, ally("Plateau Shepherds")),
    (ids::TRAPPERS_LODGE, ally("Trappers' Lodge")),
    (ids::WOODWORKERS_GUILD, ally("Woodworkers' Guild")),
];

/// The 24 cards of the split piles can each be on top of their pile: every one has a `may_buy` check (the top card only).
const SPLIT_CARDS: [CardId; 24] = {
    let mut out = [0; 24];
    let mut i = 0;
    while i < 24 {
        out[i] = crate::cards::ALLIES_SPLIT_FIRST + i as CardId;
        i += 1;
    }
    out
};

/// The Allies that act when a card is gained, at the start of the turn, at the start of the Buy phase and after a play.
const fn ally_flags(a: CardId) -> u16 {
    match a {
        ids::ARCHITECTS_GUILD | ids::BAND_OF_NOMADS | ids::CITY_STATE | ids::TRAPPERS_LODGE => AF_GAIN,
        ids::CAVE_DWELLERS | ids::CRAFTERS_GUILD | ids::DESERT_GUIDES | ids::FOREST_DWELLERS | ids::GANG_OF_PICKPOCKETS | ids::MOUNTAIN_FOLK => crate::state::AF_START,
        ids::FAMILY_OF_INVENTORS => AF_BUY | AF_FAMILY,
        ids::LEAGUE_OF_BANKERS | ids::MARKET_TOWNS | ids::PEACEFUL_CULT | ids::WOODWORKERS_GUILD => AF_BUY,
        ids::CIRCLE_OF_WITCHES | ids::FELLOWSHIP_OF_SCRIBES | ids::LEAGUE_OF_SHOPKEEPERS => AF_PLAYS,
        ids::COASTAL_HAVEN => AF_KEEP,
        ids::ISLAND_FOLK => AF_ISLAND,
        ids::ORDER_OF_ASTROLOGERS | ids::ORDER_OF_MASONS => AF_SHUFFLE,
        ids::PLATEAU_SHEPHERDS => AF_SCORE,
        _ => 0,
    }
}

/// The Allies cards (see the module docs of `sets`).
pub struct Allies;

impl SetRules for Allies {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Cards with a "when you gain" effect: Sycophant's Favors and Territory's Golds.
    const GAIN_EXT: u128 = (1u128 << (ids::SYCOPHANT - 384)) | (1u128 << (ids::TERRITORY - 384));
    /// Every card of a split pile can be on top of it, and only that one can be bought.
    const MAY_BUY_CARDS: &'static [CardId] = &SPLIT_CARDS;

    fn may_buy(g: &GameState, card: CardId) -> bool {
        g.pile_open(card)
    }

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            ids::BAUBLE => &[ModeOpt::Buys(1), ModeOpt::Coins(1), ModeOpt::Favors(1), ModeOpt::TopdeckGains],
            ids::TOWN => &[ModeOpt::CardsActions(1, 2), ModeOpt::BuysCoins(1, 2)],
            ids::TOWN_CRIER => &[ModeOpt::Coins(2), ModeOpt::GainMany(id::SILVER, Dest::Discard, 1), ModeOpt::CardsActions(1, 1)],
            ids::BLACKSMITH => &[ModeOpt::DrawUpTo(6), ModeOpt::Cards(2), ModeOpt::CardsActions(1, 1)],
            ids::INNKEEPER => &[ModeOpt::Cards(1), ModeOpt::DrawThenDiscard(3, 3), ModeOpt::DrawThenDiscard(5, 6)],
            ids::BROKER => &[ModeOpt::CostCards, ModeOpt::CostActions, ModeOpt::CostCoins, ModeOpt::CostFavors],
            ids::MODIFY => &[ModeOpt::CardsActions(1, 1), ModeOpt::GainPlus(2)],
            ids::SPECIALIST => &[ModeOpt::PlaySubjectAgain, ModeOpt::GainCopy],
            ids::HILL_FORT => &[ModeOpt::MoveGained(Dest::Hand), ModeOpt::CardsActions(1, 1)],
            ids::STRONGHOLD => &[ModeOpt::Coins(3), ModeOpt::DurationNext],
            ids::BAND_OF_NOMADS => &[ModeOpt::Cards(1), ModeOpt::Actions(1), ModeOpt::Buys(1)],
            _ => &[],
        }
    }

    /// The start-of-turn parts of the Durations that are plain bonuses.
    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::HIGHWAYMAN | ids::STRONGHOLD => (3, 0, 0, 0),
            ids::WARLORD => (2, 0, 0, 0),
            _ => (0, 0, 0, 0),
        }
    }

    /// The start-of-turn effects that change the hand or play a card (their order can matter): the Allies' abilities too.
    fn start_order_sensitive(card: CardId) -> bool {
        matches!(
            card,
            ids::ROYAL_GALLEY | ids::CONTRACT | ids::CONJURER | ids::GARRISON | ids::CAVE_DWELLERS | ids::CRAFTERS_GUILD | ids::DESERT_GUIDES | ids::FOREST_DWELLERS
                | ids::GANG_OF_PICKPOCKETS | ids::MOUNTAIN_FOLK
        )
    }

    /// Two entries of Royal Galley or Contract holding different cards are different effects.
    fn start_entry_named_by_arg(card: CardId) -> bool {
        matches!(card, ids::ROYAL_GALLEY | ids::CONTRACT)
    }

    /// Royal Galley and Contract schedule themselves once they know the card; Stronghold when it is the next-turn option that is chosen.
    fn schedules_own_duration(card: CardId) -> bool {
        matches!(card, ids::ROYAL_GALLEY | ids::CONTRACT | ids::STRONGHOLD)
    }

    /// Territory counts the differently named Victory cards.
    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        match card {
            ids::TERRITORY => all.iter().filter(|&(c, _)| cards::is(c, VICTORY)).count() as i32,
            _ => cards::def(card).vp as i32,
        }
    }

    /// Four of each card of a split pile.
    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        cards::is_allies_split(card).then_some(4)
    }

    #[inline(never)]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        let own = p == g.turn.player;
        match card {
            ids::BAUBLE => g.stack.push(mode_frame(p, card, 2)),
            ids::SYCOPHANT => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 3, 3, Then::CoinsIfPicked(3))),
            ids::GALLERIA | ids::GUILDMASTER | ids::SKIRMISHER | ids::GARRISON => g.al_this_turn(card, p, sink),
            ids::SENTINEL => {
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Trash, Filter::Any, 0, 2, Then::Nothing));
                g.stack.push(Frame { max: 5, ..Frame::new(K::RevealTop, p, card) });
            }
            ids::UNDERLING => g.give_favors(p, card, 1, sink),
            ids::BROKER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::BrokerMode)),
            ids::CARPENTER => {
                if g.empty_piles() == 0 {
                    if own {
                        g.turn.actions += 1;
                        sink.bonus(p, card, 1, 0, 0, 0);
                    }
                    g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard));
                } else {
                    g.stack.push(select(
                        p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                        Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
                    ));
                }
            }
            ids::COURIER => g.stack.push(Frame { then: Then::CourierTop, ..Frame::new(K::TopCard, p, card) }),
            ids::ROYAL_GALLEY => g.stack.push(select(p, card, Zone::Hand, Play, Filter::Types(crate::state::TypeSet::ActionNotDuration), 0, 1, Then::GalleyPlay)),
            ids::BARBARIAN => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { then: Then::BarbarianHit, ..Frame::new(K::TopCard, v, card) });
                }
            }
            ids::CAPITAL_CITY => {
                g.stack.push(Frame { then: Then::CapitalPay, ..Frame::new(K::Ask, p, card) });
                g.stack.push(Frame { exact: true, ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 2, Then::CoinsIfCount { count: 2, coins: 2 }) });
            }
            ids::CONTRACT => {
                g.give_favors(p, card, 1, sink);
                g.stack.push(select(p, card, Zone::Hand, Choose, Filter::Action, 0, 1, Then::ContractAside));
            }
            ids::EMISSARY => {
                // Below the draw (which the generic tail pushes after this): did the draw shuffle?
                g.stack.push(Frame { cost_sum: g.turn.al.shuffles as u16, ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::EmissaryCheck) });
            }
            ids::HIGHWAYMAN | ids::WARLORD => {
                if own {
                    if card == ids::HIGHWAYMAN {
                        g.players[pi].al.hw = g.players[pi].al.hw.saturating_add(1);
                    } else {
                        g.players[pi].al.wl = g.players[pi].al.wl.saturating_add(1);
                    }
                    g.al_update_hook();
                    g.shield_lingering_attack(p, sink);
                }
            }
            ids::HUNTER => g.stack.push(Frame { max: 3, then: Then::HunterReveal, ..Frame::new(K::RevealTop, p, card) }),
            ids::MODIFY => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::ModifyMode)),
            ids::SPECIALIST => g.stack.push(select(p, card, Zone::Hand, Play, Filter::ActionOrTreasure, 0, 1, Then::SpecialistPlay)),
            ids::SWAP => g.stack.push(select(p, card, Zone::Hand, Return, Filter::Action, 0, 1, Then::SwapGain)),
            ids::MARQUIS => {
                let n = g.players[pi].hand.total().min(u8::MAX as u32) as u8;
                g.stack.push(Frame { down_to_target: true, ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 10, Then::Nothing) });
                if n > 0 {
                    g.stack.push(draw_frame(p, card, n));
                }
            }
            ids::HERB_GATHERER => {
                g.stack.push(Frame { subject: card, then: Then::RotateAsk, ..Frame::new(K::Ask, p, card) });
                g.stack.push(select(p, card, Zone::Discard, Play, Filter::Treasure, 0, 1, Then::PlayPicked { times: 1 }));
                g.deck_to_discard(p, sink);
            }
            ids::ACOLYTE => {
                g.stack.push(Frame { zone: Zone::InPlay, act: Trash, subject: card, then: Then::AcolyteGain, ..Frame::new(K::YesNo, p, card) });
                g.stack.push(select(
                    p, card, Zone::Hand, Trash, Filter::Types(crate::state::TypeSet::ActionOrVictory), 0, 1,
                    Then::GainCardIfCount { count: 1, card: id::GOLD, dest: Dest::Discard },
                ));
            }
            ids::SORCERESS => g.stack.push(Frame { then: Then::SorceressNamed, ..Frame::new(K::Name, p, card) }),
            ids::SORCERER => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { then: Then::SorcererNamed, ..Frame::new(K::Name, v, card) });
                }
            }
            ids::SIBYL => {
                g.stack.push(select(p, card, Zone::Hand, Bottom, Filter::Any, 1, 1, Then::Nothing));
                g.stack.push(select(p, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing));
            }
            ids::BATTLE_PLAN => {
                g.stack.push(select(p, card, Zone::Supply, Choose, Filter::Rotatable, 0, 1, Then::RotatePicked));
                g.stack.push(select(p, card, Zone::Hand, Reveal, Filter::Types(crate::state::TypeSet::Attack), 0, 1, Then::DrawPerPick));
            }
            ids::ARCHER => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    if g.players[v as usize].hand.total() >= 5 {
                        // The victim keeps one card to themselves, the attacker picks one of the others to be discarded, the card kept comes back.
                        g.stack.push(Frame { chooser: p, ..select(v, card, Zone::Hand, Discard, Filter::Any, 1, 1, Then::RevealedToHand) });
                        g.stack.push(select(v, card, Zone::Hand, SetAside, Filter::Any, 1, 1, Then::Nothing));
                    }
                }
            }
            ids::TENT | ids::OLD_MAP => {
                g.stack.push(Frame { subject: card, then: Then::RotateAsk, ..Frame::new(K::Ask, p, card) });
                if card == ids::OLD_MAP {
                    g.stack.push(draw_frame(p, card, 1));
                    g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 1, 1, Then::Nothing));
                }
            }
            ids::HILL_FORT => {
                g.stack.push(mode_frame(p, card, 1));
                g.stack.push(Frame { then: Then::HillFortGain, ..gain_frame(p, card, 4, Filter::Any, Dest::Discard) });
            }
            ids::STRONGHOLD => g.stack.push(mode_frame(p, card, 1)),
            ids::VOYAGE => {
                if own && !g.turn.is_extra_turn {
                    g.turn.al.voyage = true;
                }
            }
            ids::SUNKEN_TREASURE => g.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, card, u8::MAX, Filter::NotInPlayAction, Dest::Discard) }),
            ids::DISTANT_SHORE => {
                g.gain(p, id::ESTATE, Dest::Discard, false, card, sink);
            }
            ids::MILLER => g.stack.push(Frame { max: 4, then: Then::MillerPick, ..Frame::new(K::RevealTop, p, card) }),
            ids::ELDER => g.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::ElderPlay)),
            ids::STUDENT => {
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::StudentTrash));
                g.stack.push(Frame { subject: card, then: Then::RotateAsk, ..Frame::new(K::Ask, p, card) });
            }
            ids::CONJURER => g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard)),
            ids::LICH => {
                g.players[pi].al.skips = g.players[pi].al.skips.saturating_add(1);
                g.al_update_hook();
            }
            _ => {}
        }
        // The cards with a "choose" decision of their own.
        match card {
            ids::TOWN | ids::BLACKSMITH | ids::INNKEEPER => g.stack.push(mode_frame(p, card, 1)),
            ids::TOWN_CRIER => {
                g.stack.push(Frame { subject: card, then: Then::RotateAsk, ..Frame::new(K::Ask, p, card) });
                g.stack.push(mode_frame(p, card, 1));
            }
            _ => {}
        }
    }

    /// The effects that are not on a pile or a Treasure (Merchant Camp's discard offer is in `offers_on_discard`).
    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, _on_buy: bool, sink: &mut S) {
        g.al_gain_triggers(p, c, to, sink);
    }

    /// The effects that look at every gain: the Ally's (Architects' Guild, Band of Nomads, City-state, Trappers' Lodge) and what the cards played this turn do (Galleria,
    /// Guildmaster, Skirmisher, Bauble, Garrison).
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.al.flags & AF_GAIN != 0 || g.turn.al.live != 0
    }

    /// The Ally's start-of-turn ability waits in the start-of-turn list with the Durations.
    #[inline]
    fn turn_start_entries(g: &mut GameState) {
        if g.al.flags & AF_START == 0 {
            return;
        }
        g.al_turn_start_entry();
    }

    /// A card set aside by Contract is played at the start of the next turn (it uses no Action).
    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
        if cards::is_ally(e.card) {
            g.al_start_ability(p, e.card);
            return true;
        }
        if e.card != ids::CONTRACT {
            return false;
        }
        if g.players[p as usize].men.aside.remove(e.arg) {
            g.players[p as usize].in_play.add(e.arg, 1);
            sink.depth(0);
            sink.event(Event::StartPlay { player: p, source: e.card, card: e.arg });
            g.resolve_effects(e.arg, p, 0, sink);
        }
        true
    }

    /// The next-turn parts of the Durations that need more than a bonus: Importer's gain, Garrison's tokens, Conjurer's return to the hand, Highwayman's discard, Royal Galley's replay.
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
        let pi = p as usize;
        match e.card {
            ids::IMPORTER => {
                for _ in 0..e.times {
                    g.stack.push(Frame { depth: 1, ..gain_frame(p, e.card, 5, Filter::Any, Dest::Discard) });
                }
            }
            ids::GARRISON => {
                // The tokens are removed once, whatever the number of Garrisons or plays.
                let n = std::mem::take(&mut g.players[pi].al.tokens);
                if n > 0 {
                    g.stack.push(Frame { depth: 1, ..draw_frame(p, e.card, n) });
                }
            }
            ids::CONJURER => {
                for _ in 0..e.times {
                    if g.remove_from_play(p, e.card) {
                        g.players[pi].hand.add(e.card, 1);
                        sink.event(Event::Draw { player: p, card: e.card });
                    }
                }
            }
            ids::HIGHWAYMAN => {
                // The Attack ends; each play discards one Highwayman from play (and draws 3) first.
                g.players[pi].al.hw = 0;
                g.al_update_hook();
                for _ in 0..e.times {
                    if g.remove_from_play(p, e.card) {
                        g.players[pi].discard.add(e.card, 1);
                        sink.event(Event::Discard { player: p, card: e.card });
                    }
                }
            }
            ids::WARLORD => {
                g.players[pi].al.wl = 0;
                g.al_update_hook();
            }
            ids::ROYAL_GALLEY => {
                // The card it played is played again if it is still in play.
                if e.arg != 0 && g.players[pi].in_play.has(e.arg) {
                    g.stack.push(Frame { subject: e.arg, count: 2, depth: 1, ..Frame::new(K::PlayEffects, p, e.card) });
                }
            }
            _ => {}
        }
    }

    /// Merchant Camp and Tent: "when you discard this from play, you may put it onto your deck" (each copy being discarded).
    #[inline(never)]
    fn offers_on_discard(g: &mut GameState) {
        let p = g.turn.player;
        for card in [ids::MERCHANT_CAMP, ids::TENT] {
            if !g.in_supply(card) {
                continue;
            }
            let ps = &g.players[p as usize];
            let discarded = ps.in_play.get(card).saturating_sub(g.turn.duration_held.get(card).saturating_add(ps.stays.get(card)));
            for _ in 0..discarded {
                g.stack.push(Frame { zone: Zone::InPlay, act: Act::Topdeck, subject: card, ..Frame::new(K::YesNo, p, card) });
            }
        }
    }

    /// Coastal Haven: while the hand is discarded, any number of cards are kept in it for as many Favors.
    #[inline(never)]
    fn offers_at_cleanup(g: &mut GameState) {
        let p = g.turn.player;
        let ps = &g.players[p as usize];
        let n = (ps.al.favors as u32).min(ps.hand.total()).min(ALL as u32 - 1) as u8;
        if n > 0 && g.al.ally == ids::COASTAL_HAVEN {
            g.stack.push(select(p, ids::COASTAL_HAVEN, Zone::Hand, Act::SetAside, Filter::Any, 0, n, Then::HavenKeep));
        }
    }

    /// A trashed Sycophant gives 2 Favors; a trashed Lich goes to the discard pile and gains a cheaper card from the trash.
    fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, sink: &mut S) {
        match card {
            ids::SYCOPHANT => g.give_favors(p, card, 2, sink),
            ids::LICH => {
                if g.trash.remove(card) {
                    g.players[p as usize].discard.add(card, 1);
                    sink.event(Event::Discard { player: p, card });
                }
                let max = cards::cost(card) - 1;
                g.stack.push(select(p, card, Zone::Trash, Act::Gain, Filter::CostRange(0, max), 1, 1, Then::Nothing));
            }
            _ => {}
        }
    }
}

// -------------------------------------------------------------------------------------------
// Set-up
// -------------------------------------------------------------------------------------------

/// The order of a new split pile: four of each of its cards, the first card on top.
fn initial_order() -> PileOrder {
    let mut order = 0u32;
    for pos in 0..16u32 {
        order |= (pos / 4) << (2 * pos);
    }
    PileOrder { order, len: 16 }
}

/// Allies set-up: the Ally and the Favors (1 each, 5 with Importer), the split piles' other cards and their order, which effects are live.
pub(crate) fn setup(g: &mut GameState, cfg: &GameConfig, n: usize) {
    // Games without a card of the set (all but a few) stop here: the state is as `new` made it.
    let mut ally = 0;
    let mut any = false;
    for &k in &cfg.kingdom {
        if cards::is_ally(k) {
            ally = k;
            any = true;
        } else if (ids::BAUBLE..=cards::LAST_KINGDOM).contains(&k) {
            any = true;
        }
    }
    if !any {
        return;
    }
    let liaison = cfg.kingdom.iter().any(|&k| cards::is(k, LIAISON));
    // A game with a Liaison has exactly one Ally: the kingdom's if it lists one, else one chosen from the seed.
    if ally == 0 && liaison {
        ally = cards::FIRST_ALLY + (cfg.seed % cards::NUM_ALLIES as u64) as CardId;
    }
    g.al.ally = ally;
    let mut flags = ally_flags(ally);
    for i in 0..AL_PILES {
        let first = cards::ALLIES_SPLIT_FIRST + 4 * i as CardId;
        if g.in_supply(first) {
            flags |= AF_SPLIT;
            for k in 1..4 {
                g.supply.set(first + k, 4);
                g.in_supply.insert(first + k);
            }
            g.al.piles[i] = initial_order();
        }
    }
    if ally != 0 || liaison {
        flags |= AF_ALLY;
        let favors = if g.in_supply(ids::IMPORTER) { 5 } else { 1 };
        for p in 0..n {
            g.players[p].al.favors = favors;
        }
    }
    if g.in_supply(ids::MERCHANT_CAMP) || g.in_supply(ids::TENT) {
        flags |= AF_DISCARD;
    }
    if g.in_supply(ids::HIGHWAYMAN) {
        flags |= AF_HW;
    }
    if g.in_supply(ids::EMISSARY) {
        flags |= AF_STAMP;
    }
    if flags & AF_FAMILY != 0 {
        g.men.flags |= M_COST;
    }
    g.al.flags = flags;
}

// -------------------------------------------------------------------------------------------
// The state's Allies parts
// -------------------------------------------------------------------------------------------

impl GameState {
    /// `n` Favors for `p`, from `source`.
    pub(crate) fn give_favors<S: EventSink>(&mut self, p: u8, source: CardId, n: u8, sink: &mut S) {
        if n == 0 {
            return;
        }
        let f = &mut self.players[p as usize].al.favors;
        *f = f.saturating_add(n);
        sink.event(Event::Favor { player: p, source, n, gained: true });
    }

    /// `p` spends `n` Favors (as many as there are) on `source`.
    pub(crate) fn spend_favors<S: EventSink>(&mut self, p: u8, source: CardId, n: u8, sink: &mut S) {
        let f = &mut self.players[p as usize].al.favors;
        let n = n.min(*f);
        if n == 0 {
            return;
        }
        *f -= n;
        sink.event(Event::Favor { player: p, source, n, gained: false });
    }

    /// The Favors `p` has (public).
    pub fn favors_of(&self, p: u8) -> u8 {
        self.players[p as usize].al.favors
    }

    /// Recomputes `AlState::hook` from the Highwaymen, Warlords, Lich skips and the Voyage turn.
    #[inline(never)]
    pub fn al_update_hook(&mut self) {
        let mut h = self.al.hook & AH_LIMIT;
        for ps in self.players.iter() {
            if ps.al.hw > 0 {
                h |= AH_HW;
            }
            if ps.al.wl > 0 {
                h |= AH_WL;
            }
            if ps.al.skips > 0 {
                h |= AH_SKIP;
            }
        }
        self.al.hook = h;
    }

    // ---- the split piles ----

    /// `(pile index, member index)` of a card of a split pile.
    #[inline(always)]
    fn al_slot(c: CardId) -> (usize, u32) {
        let i = (c - cards::ALLIES_SPLIT_FIRST) as usize;
        (i / 4, (i % 4) as u32)
    }

    /// The top card of the Allies split pile `i`, `None` when it is empty or not in the game.
    pub fn al_top(&self, i: usize) -> Option<CardId> {
        let po = &self.al.piles[i];
        (po.len > 0).then(|| cards::ALLIES_SPLIT_FIRST + 4 * i as CardId + (po.order & 3) as CardId)
    }

    /// The cards of the Allies split pile `i`, top first (at most 16), and how many.
    pub fn al_pile_cards(&self, i: usize) -> ([CardId; 16], usize) {
        let po = &self.al.piles[i];
        let mut out = [0 as CardId; 16];
        for pos in 0..po.len as usize {
            out[pos] = cards::ALLIES_SPLIT_FIRST + 4 * i as CardId + ((po.order >> (2 * pos)) & 3) as CardId;
        }
        (out, po.len as usize)
    }

    /// Whether `c`, a card of a split pile, is on top of it.
    #[inline]
    pub(crate) fn al_pile_open(&self, c: CardId) -> bool {
        let (i, k) = Self::al_slot(c);
        let po = &self.al.piles[i];
        po.len > 0 && po.order & 3 == k && self.supply.get(c) > 0
    }

    /// A card of a split pile was taken from the Supply (gained, trashed, Exiled): it leaves the order (the top one, or the first of its name from the top).
    #[inline(never)]
    pub(crate) fn al_took(&mut self, c: CardId) {
        let (i, k) = Self::al_slot(c);
        let po = &mut self.al.piles[i];
        for pos in 0..po.len as u32 {
            if (po.order >> (2 * pos)) & 3 == k {
                let low = po.order & ((1u32 << (2 * pos)) - 1);
                let high = if pos + 1 >= 16 { 0 } else { po.order >> (2 * (pos + 1)) };
                po.order = low | (high << (2 * pos));
                po.len -= 1;
                return;
            }
        }
    }

    /// A card was taken from the Supply: the top card of a Knights or Ruins pile is replaced by the next one; a card of an Allies split pile leaves its order.
    #[inline(always)]
    pub(crate) fn pile_taken(&mut self, c: CardId) {
        if c >= cards::FIRST_PILE_CARD {
            self.advance_pile(c);
        } else if cards::is_allies_split(c) {
            self.al_took(c);
        }
    }

    /// `c` goes back to its split pile, on top of it.
    #[inline(never)]
    pub(crate) fn al_return(&mut self, c: CardId) {
        self.supply.add(c, 1);
        let (i, k) = Self::al_slot(c);
        let po = &mut self.al.piles[i];
        if po.len < 16 {
            po.order = (po.order << 2) | k;
            po.len += 1;
        }
    }

    /// The length of the run of one card on top of pile `i`.
    fn al_top_run(&self, i: usize) -> usize {
        let po = &self.al.piles[i];
        let top = po.order & 3;
        (0..po.len as usize).take_while(|&pos| (po.order >> (2 * pos)) & 3 == top).count()
    }

    /// Whether rotating the pile of `c` (a card of an Allies split pile that is on top) changes its top card.
    pub(crate) fn al_rotatable(&self, c: CardId) -> bool {
        if !cards::is_allies_split(c) {
            return false;
        }
        let (i, _) = Self::al_slot(c);
        self.al_pile_open(c) && self.al_top_run(i) < self.al.piles[i].len as usize
    }

    /// Whether rotating the pile whose first card is `first` (0: none) would uncover another card.
    pub(crate) fn al_rotate_useful(&self, first: CardId) -> bool {
        if !cards::is_allies_split(first) {
            return false;
        }
        let (i, _) = Self::al_slot(first);
        self.al.piles[i].len > 0 && self.al_top_run(i) < self.al.piles[i].len as usize
    }

    /// `p` rotates the pile of `c` (any card of it), `source` being what let them: the top card and the copies of it directly under it go to the bottom.
    pub fn al_rotate_for<S: EventSink>(&mut self, p: u8, source: CardId, c: CardId, sink: &mut S) {
        if !cards::is_allies_split(c) {
            return;
        }
        let (i, _) = Self::al_slot(c);
        let Some(before) = self.al_top(i) else { return };
        let r = self.al_top_run(i);
        let po = &mut self.al.piles[i];
        let len = po.len as usize;
        if r >= len {
            return;
        }
        let block = po.order & ((1u32 << (2 * r)) - 1);
        let rest = po.order >> (2 * r);
        po.order = rest | (block << (2 * (len - r)));
        let now = self.al_top(i).unwrap_or(before);
        sink.event(Event::Rotate { player: p, source, top: before, now });
    }

    /// Every Allies split pile in the game takes the order its counts give: each card's copies together, in card order, the first card on top.
    pub(crate) fn al_orders_from_counts(&mut self) {
        for i in 0..AL_PILES {
            let first = cards::ALLIES_SPLIT_FIRST + 4 * i as CardId;
            let mut po = PileOrder::default();
            if self.in_supply(first) {
                for k in 0..4u32 {
                    for _ in 0..self.supply.get(first + k as CardId) {
                        if po.len < 16 {
                            po.order |= k << (2 * po.len as u32);
                            po.len += 1;
                        }
                    }
                }
            }
            self.al.piles[i] = po;
        }
    }

    /// The number of Allies split piles in the game that have no card left.
    pub(crate) fn al_empty_piles(&self) -> u32 {
        let mut n = 0;
        for i in 0..AL_PILES {
            let first = cards::ALLIES_SPLIT_FIRST + 4 * i as CardId;
            if self.in_supply(first) && (0..4).all(|k| self.supply.get(first + k) == 0) {
                n += 1;
            }
        }
        n
    }

    // ---- Family of Inventors ----

    /// How much cheaper the cards of `c`'s pile are for the tokens on it.
    #[inline]
    pub(crate) fn al_fam_less(&self, c: CardId) -> u8 {
        if self.al.fam.is_empty() { 0 } else { self.al.fam.get(cards::pile_of(c)) }
    }

    /// Whether `c`, the top card of a Supply pile, is a pile that is not a Victory pile (Family of Inventors' tokens): the pile's own type, not the card's (a Castle pile, a
    /// pile with a Victory card on top).
    pub(crate) fn al_token_pile(&self, c: CardId) -> bool {
        let pile = cards::pile_of(c);
        if pile == id::CASTLES || cards::is_castle(c) {
            return false;
        }
        pile == id::KNIGHTS || pile == id::RUINS || !cards::is(pile, VICTORY)
    }

    // ---- scoring ----

    /// Plateau Shepherds: 2 VP for each Favor that can be paired with a card costing $2.
    pub(crate) fn al_score(&self, p: u8) -> i32 {
        if self.al.ally != ids::PLATEAU_SHEPHERDS {
            return 0;
        }
        let less = (self.rs.active == id::FLOURISHING_TRADE) as u8;
        let twos: u32 = self
            .players[p as usize]
            .all_cards()
            .iter()
            .filter(|&(c, _)| cards::cost(c).saturating_sub(less) == 2 && !cards::potion_cost(c) && cards::debt_cost(c) == 0)
            .map(|(_, n)| n as u32)
            .sum();
        2 * (self.players[p as usize].al.favors as u32).min(twos) as i32
    }

    // ---- plays and gains ----

    /// A card played this turn that does something for the rest of it: Merchant Camp, Tent, Galleria, Guildmaster, Skirmisher, Garrison (a play).
    pub(crate) fn al_this_turn<S: EventSink>(&mut self, card: CardId, p: u8, sink: &mut S) {
        if p != self.turn.player {
            return;
        }
        let t = &mut self.turn.al;
        match card {
            ids::GALLERIA => t.galleria = t.galleria.saturating_add(1),
            ids::GUILDMASTER => t.guild = t.guild.saturating_add(1),
            ids::SKIRMISHER => t.skirm = t.skirm.saturating_add(1),
            ids::GARRISON => t.garrison = t.garrison.saturating_add(1),
            _ => {}
        }
        t.live |= 1;
        if card == ids::SKIRMISHER {
            self.shield_lingering_attack(p, sink);
        }
    }

    /// The "when you gain" triggers of the set: the card's own (Sycophant, Territory), the effects of the cards played this turn and the Ally's ability.
    #[inline(never)]
    pub(crate) fn al_gain_triggers<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, sink: &mut S) {
        let pi = p as usize;
        let own = p == self.turn.player;
        let zone = GameState::gain_dest_zone(to);
        let gt = self.turn.led.ingain;
        match c {
            ids::SYCOPHANT => self.give_favors(p, c, 2, sink),
            ids::TERRITORY => {
                for _ in 0..self.empty_piles().min(12) {
                    self.stack.push(gain_frame(p, c, u8::MAX, Filter::Card(id::GOLD), Dest::Discard));
                }
            }
            _ => {}
        }
        // What the cards played this turn do (the turn's player's gains only).
        if own && self.turn.al.live != 0 {
            let t = self.turn.al;
            if t.galleria > 0 && matches!(self.cost_when_gained(p, c, to), 3 | 4) {
                self.turn.buys = self.turn.buys.saturating_add(t.galleria);
                sink.bonus(p, ids::GALLERIA, 0, t.galleria, 0, 0);
            }
            if t.guild > 0 {
                self.give_favors(p, ids::GUILDMASTER, t.guild, sink);
            }
            if t.garrison > 0 {
                let tokens = &mut self.players[pi].al.tokens;
                *tokens = tokens.saturating_add(t.garrison);
            }
            if t.skirm > 0 && cards::is(c, ATTACK) {
                for _ in 0..t.skirm {
                    let (vs, n) = self.victims(sink);
                    for &v in vs[..n].iter().rev() {
                        self.stack.push(Frame { down_to_target: true, ..select(v, ids::SKIRMISHER, Zone::Hand, Act::Discard, Filter::Any, 0, 3, Then::Nothing) });
                    }
                }
            }
            if t.bauble && to != Dest::DeckTop {
                self.stack.push(Frame { zone, act: Act::Topdeck, subject: c, tok: gt, ..Frame::new(K::YesNo, p, ids::BAUBLE) });
            }
        }
        // The Ally's ability, if the player can pay for it.
        let favors = self.players[pi].al.favors;
        let ally = self.al.ally;
        match ally {
            ids::ARCHITECTS_GUILD if favors >= 2 => {
                self.stack.push(Frame { subject: c, min: 2, then: Then::AllyAsk, ..Frame::new(K::Ask, p, ally) });
            }
            ids::BAND_OF_NOMADS if favors >= 1 && self.cost_when_gained(p, c, to) >= 3 => {
                self.stack.push(Frame { subject: c, min: 1, then: Then::AllyAsk, ..Frame::new(K::Ask, p, ally) });
            }
            ids::CITY_STATE if own && favors >= 2 && self.is_action(c) => {
                self.stack.push(Frame { zone, act: Act::Play, subject: c, min: 2, then: Then::AllyFavor, tok: gt, ..Frame::new(K::YesNo, p, ally) });
            }
            ids::TRAPPERS_LODGE if favors >= 1 && to != Dest::DeckTop => {
                self.stack.push(Frame { zone, act: Act::Topdeck, subject: c, min: 1, then: Then::AllyFavor, tok: gt, ..Frame::new(K::YesNo, p, ally) });
            }
            _ => {}
        }
    }

    /// Whether `p` may not play the card `c` from the hand now: a Warlord's Attack (two copies of an Action already in play), a Voyage turn that has used its plays.
    pub(crate) fn al_play_blocked(&self, p: u8, c: CardId) -> bool {
        let h = self.al.hook;
        if h & AH_LIMIT != 0 && p == self.turn.player && self.turn.al.hand_plays >= VOYAGE_PLAYS {
            return true;
        }
        h & AH_WL != 0 && self.is_action(c) && self.players[p as usize].in_play.get(c) >= 2 && self.al_hit_by(p, |a| a.wl)
    }

    /// Whether some other player has a lingering Attack (`count` > 0 for them) that was not shielded against by `p`.
    fn al_hit_by(&self, p: u8, count: impl Fn(&crate::state::AlPlayer) -> u8) -> bool {
        (0..self.num_players).any(|a| {
            let owner = &self.players[a as usize];
            a != p && count(&owner.al) > 0 && owner.shielded & (1 << p) == 0
        })
    }

    /// `Some(answer)` to "does the order the hand's Treasures are played in matter?" when a Voyage turn or a Highwayman's Attack decides it, `None` when they do not.
    #[inline(never)]
    pub(crate) fn al_treasures_by_choice(&self) -> Option<bool> {
        let p = self.turn.player;
        let hand = &self.players[p as usize].hand;
        if self.al.hook & AH_LIMIT != 0 {
            return Some(self.turn.al.hand_plays < VOYAGE_PLAYS && hand.iter().any(|(c, _)| self.is_treasure(c)));
        }
        // The first Treasure played is wasted: the player picks which.
        if !self.turn.al.t1 && self.al_hit_by(p, |a| a.hw) && hand.iter().any(|(c, _)| self.is_treasure(c)) {
            return Some(true);
        }
        None
    }

    /// Highwayman: the Treasure `p` is playing is the first of the turn, and a Highwayman's Attack is on against them: it does nothing. (Also remembers that the first
    /// Treasure has been played.)
    pub(crate) fn al_first_treasure_blocked(&mut self, p: u8) -> bool {
        let first = !self.turn.al.t1;
        self.turn.al.t1 = true;
        first && self.al.hook & AH_HW != 0 && self.al_hit_by(p, |a| a.hw)
    }

    /// Elder's play: the Mode frames the card pushed give one more pick, and the Select frames that lead to one are marked.
    #[inline(never)]
    pub(crate) fn al_elder_stamp(&mut self, base: usize) {
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            match f.kind {
                K::Mode if f.count == 0 => {
                    // A window onto the card's table (Count's two "choose one"s) is a choice of its own: each gets the extra pick.
                    let len = match f.then {
                        Then::ModeWindow { count, .. } => count,
                        Then::Nothing => cards::modes(f.source).len() as u8,
                        _ => 0,
                    };
                    if f.max < len {
                        f.max += 1;
                        // The extra option is a "may": its pick offers to stop.
                        f.optional = true;
                    }
                }
                K::Select => f.gf |= GF_ELDER,
                _ => {}
            }
        }
    }

    /// The deck goes into the discard pile (Herb Gatherer): not a discard, no reactions.
    pub(crate) fn deck_to_discard<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        self.cover_deck(p);
        let ps = &mut self.players[p as usize];
        let n = ps.deck_size();
        let deck = ps.deck_counts();
        ps.deck_known.clear();
        ps.deck_known_bottom.clear();
        ps.deck_unknown.clear();
        ps.discard.add_all(&deck);
        sink.event(Event::DeckToDiscard { player: p, n: n as u16 });
    }

    // ---- the Ally's moments ----

    /// The start of the turn: the Ally's ability (if the player can use it) is a start-of-turn entry.
    #[inline(never)]
    pub(crate) fn al_turn_start_entry(&mut self) {
        let p = self.turn.player;
        let favors = self.players[p as usize].al.favors;
        let ally = self.al.ally;
        let wanted = match ally {
            ids::CAVE_DWELLERS | ids::DESERT_GUIDES | ids::FOREST_DWELLERS => favors >= 1,
            ids::CRAFTERS_GUILD => favors >= 2,
            ids::MOUNTAIN_FOLK => favors >= 5,
            ids::GANG_OF_PICKPOCKETS => true,
            _ => false,
        };
        if wanted {
            self.turn.start_pending.push(PendingDuration { card: ally, times: 1, arg: 0, used: 0 });
        }
    }

    /// The Ally's start-of-turn ability resolves: the frames that offer it.
    pub(crate) fn al_start_ability(&mut self, p: u8, ally: CardId) {
        let cost = match ally {
            ids::CRAFTERS_GUILD => 2,
            ids::MOUNTAIN_FOLK => 5,
            _ => 1,
        };
        if ally == ids::GANG_OF_PICKPOCKETS && self.players[p as usize].al.favors == 0 {
            // No Favor to spend: the hand is discarded down to 4.
            self.stack.push(Frame { depth: 1, down_to_target: true, ..select(p, ally, Zone::Hand, Act::Discard, Filter::Any, 0, 4, Then::Nothing) });
        } else {
            self.stack.push(Frame { depth: 1, min: cost, then: Then::AllyAsk, ..Frame::new(K::Ask, p, ally) });
        }
    }

    /// The start of the Buy phase: the Ally's ability (League of Bankers' $ at once, the rest offered).
    #[inline(never)]
    pub(crate) fn al_buy_start<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let favors = self.players[p as usize].al.favors;
        let ally = self.al.ally;
        match ally {
            ids::LEAGUE_OF_BANKERS => {
                let k = (favors / 4) as u16;
                if k > 0 {
                    self.turn.coins += k;
                    sink.bonus(p, ally, 0, 0, k, 0);
                }
            }
            ids::FAMILY_OF_INVENTORS | ids::MARKET_TOWNS | ids::WOODWORKERS_GUILD if favors >= 1 => {
                self.stack.push(Frame { depth: 0, min: 1, then: Then::AllyAsk, ..Frame::new(K::Ask, p, ally) });
            }
            ids::PEACEFUL_CULT if favors >= 1 && !self.players[p as usize].hand.is_empty() => {
                let n = (favors as u32).min(ALL as u32 - 1) as u8;
                self.stack.push(select(p, ally, Zone::Hand, Act::Trash, Filter::Any, 0, n, Then::CultSpend));
            }
            _ => {}
        }
    }

    /// A card was played in the Ally's game: if the Ally acts after this play (Circle of Witches and League of Shopkeepers after a Liaison, Fellowship of Scribes after an Action),
    /// a frame waits under the card's effects for it to be done.
    pub(crate) fn al_push_after_play(&mut self, card: CardId, p: u8, tok: u8) {
        let ally = self.al.ally;
        let relevant = match ally {
            ids::CIRCLE_OF_WITCHES | ids::LEAGUE_OF_SHOPKEEPERS => cards::is(card, LIAISON),
            ids::FELLOWSHIP_OF_SCRIBES => cards::is(card, ACTION),
            _ => false,
        };
        if relevant {
            self.stack.push(Frame { subject: card, tok, then: Then::AllyAsk, ..Frame::new(K::AfterPlay, p, ally) });
        }
    }

    /// The card `f.subject` was played and has fully resolved: the Ally's after-play ability.
    pub(crate) fn al_after_play<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        self.stack.pop();
        let p = f.player;
        let favors = self.players[p as usize].al.favors;
        match f.source {
            ids::CIRCLE_OF_WITCHES if favors >= 3 => {
                self.stack.push(Frame { depth: f.depth, subject: f.subject, min: 3, then: Then::AllyAsk, ..Frame::new(K::Ask, p, f.source) });
            }
            ids::FELLOWSHIP_OF_SCRIBES if favors >= 1 && self.players[p as usize].hand.total() <= 4 => {
                self.stack.push(Frame { depth: f.depth, subject: f.subject, min: 1, then: Then::AllyAsk, ..Frame::new(K::Ask, p, f.source) });
            }
            ids::LEAGUE_OF_SHOPKEEPERS => {
                let (coins, extra) = (favors >= 5, favors >= 10);
                if coins {
                    self.turn.coins += 1;
                    if extra {
                        self.turn.actions += 1;
                        self.turn.buys += 1;
                    }
                    sink.bonus(p, f.source, extra as u8, extra as u8, 1, 0);
                }
            }
            _ => {}
        }
        Run::Continue
    }

    /// Whether the `Ask` frame `f` (an Ally's ability, a rotation, Capital City's payment) has nothing to ask any more.
    pub(crate) fn al_ask_pointless(&self, f: &Frame) -> bool {
        let p = f.player;
        let ps = &self.players[p as usize];
        match f.then {
            Then::RotateAsk => !self.al_rotate_useful(f.subject),
            Then::CapitalPay => self.turn.coins < 2,
            _ => {
                if ps.al.favors < f.min {
                    return true;
                }
                match f.source {
                    ids::MARKET_TOWNS | ids::WOODWORKERS_GUILD => !ps.hand.iter().any(|(c, _)| self.is_action(c)),
                    ids::FELLOWSHIP_OF_SCRIBES => ps.hand.total() > 4,
                    ids::GANG_OF_PICKPOCKETS => ps.hand.total() <= 4,
                    ids::FOREST_DWELLERS => ps.deck_size() + ps.discard.total() == 0,
                    ids::ISLAND_FOLK => self.turn.is_extra_turn || self.turn.outpost_grants_extra,
                    _ => false,
                }
            }
        }
    }

    /// An `Ask` of the set was answered Yes: Capital City's payment, a rotation, an Ally's ability (the Favors are spent first).
    pub(crate) fn al_ask_yes<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        let (p, src, d) = (f.player, f.source, f.depth);
        match f.then {
            Then::RotateAsk => self.al_rotate_for(p, src, f.subject, sink),
            Then::CapitalPay => {
                self.turn.coins -= 2;
                self.stack.push(Frame { depth: d, ..draw_frame(p, src, 2) });
            }
            _ => {
                self.spend_favors(p, src, f.min, sink);
                let again = Frame { depth: d, ..f };
                match src {
                    ids::ARCHITECTS_GUILD => {
                        let c = f.subject;
                        self.stack.push(Frame { less: true, potion: cards::potion_cost(c), depth: d, ..gain_frame(p, src, self.cost(c), Filter::NonVictory, Dest::Discard) });
                    }
                    ids::BAND_OF_NOMADS => {
                        if p == self.turn.player {
                            self.stack.push(Frame { depth: d, ..mode_frame(p, src, 1) });
                        } else {
                            self.stack.push(Frame { depth: d, ..draw_frame(p, src, 1) });
                        }
                    }
                    ids::CAVE_DWELLERS => {
                        // Discard a card, then draw a card (even if nothing was discarded); the offer is made again.
                        self.stack.push(again);
                        self.stack.push(Frame { depth: d, ..draw_frame(p, src, 1) });
                        self.stack.push(Frame { depth: d, ..select(p, src, Zone::Hand, Act::Discard, Filter::Any, 1, 1, Then::Nothing) });
                    }
                    ids::CIRCLE_OF_WITCHES => {
                        let mut order = [0u8; MAX_PLAYERS];
                        let mut k = 0;
                        for q in self.others(p) {
                            order[k] = q;
                            k += 1;
                        }
                        self.push_gain_to_each(&order[..k], id::CURSE, src);
                    }
                    ids::CRAFTERS_GUILD => self.stack.push(Frame { depth: d, ..gain_frame(p, src, 4, Filter::Any, Dest::DeckTop) }),
                    ids::DESERT_GUIDES => {
                        self.discard_whole_hand(p, sink);
                        self.stack.push(again);
                        self.stack.push(Frame { depth: d, ..draw_frame(p, src, 5) });
                    }
                    ids::FELLOWSHIP_OF_SCRIBES => self.stack.push(Frame { depth: d, ..draw_frame(p, src, 1) }),
                    ids::FOREST_DWELLERS => {
                        // The top 3: discard any, put the rest back in any order.
                        self.stack.push(Frame { depth: d, ordered: true, ..select(p, src, Zone::Revealed, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                        self.stack.push(Frame { depth: d, ..select(p, src, Zone::Revealed, Act::Discard, Filter::Any, 0, ALL, Then::Nothing) });
                        self.stack.push(Frame { depth: d, max: 3, ..Frame::new(K::RevealTop, p, src) });
                    }
                    ids::ISLAND_FOLK => self.turn.outpost_grants_extra = true,
                    ids::MARKET_TOWNS => {
                        self.stack.push(again);
                        self.stack.push(Frame { depth: d, ..select(p, src, Zone::Hand, Act::Play, Filter::Action, 1, 1, Then::PlayPicked { times: 1 }) });
                    }
                    ids::MOUNTAIN_FOLK => self.stack.push(Frame { depth: d, ..draw_frame(p, src, 3) }),
                    ids::WOODWORKERS_GUILD => {
                        self.stack.push(Frame { depth: d, ..select(p, src, Zone::Hand, Act::Trash, Filter::Action, 1, 1, Then::WoodworkersGain) });
                    }
                    ids::FAMILY_OF_INVENTORS => {
                        self.stack.push(Frame { depth: d, ..select(p, src, Zone::Supply, Act::Choose, Filter::TokenPile, 1, 1, Then::FamilyToken) });
                    }
                    _ => {}
                }
            }
        }
    }

    /// An `Ask` of an Ally was answered No: Gang of Pickpockets discards the hand down to 4.
    pub(crate) fn al_ask_no<S: EventSink>(&mut self, f: Frame, _sink: &mut S) {
        if f.source == ids::GANG_OF_PICKPOCKETS {
            self.stack.push(Frame { depth: f.depth, down_to_target: true, ..select(f.player, f.source, Zone::Hand, Act::Discard, Filter::Any, 0, 4, Then::Nothing) });
        }
    }

    /// A `YesNo` answered Yes: an Ally's ability on a card (the card moved; the Favors are spent), Acolyte trashed itself.
    pub(crate) fn al_yes<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        match f.then {
            Then::AllyFavor => self.spend_favors(f.player, f.source, f.min, sink),
            Then::AcolyteGain => {
                self.stack.push(Frame { gf: GF_ANY_COST, depth: f.depth, ..gain_frame(f.player, f.source, u8::MAX, Filter::InPile(ids::HERB_GATHERER), Dest::Discard) });
            }
            _ => {}
        }
    }

    /// The Island Folk offer at the end of the turn: 5 Favors for an extra turn, if there is none already, this is not one, and the game goes on. Whether a frame was pushed.
    pub(crate) fn al_island_offer(&mut self) -> bool {
        let p = self.turn.player;
        if self.al.ally != ids::ISLAND_FOLK
            || self.players[p as usize].al.favors < 5
            || self.turn.is_extra_turn
            || self.turn.outpost_grants_extra
            || self.end_condition_met()
        {
            return false;
        }
        self.stack.push(Frame { min: 5, then: Then::AllyAsk, ..Frame::new(K::Ask, p, ids::ISLAND_FOLK) });
        true
    }

    /// A shuffle is about to happen for `p` (their deck is empty): with Order of Astrologers or Order of Masons and a Favor, they pick the cards to put on top or to keep out
    /// (a frame; it shuffles the rest). Whether a frame was pushed.
    #[inline(never)]
    pub(crate) fn al_shuffle_offer(&mut self, p: u8) -> bool {
        let ps = &self.players[p as usize];
        let favors = ps.al.favors as u32;
        let cards_left = ps.discard.total();
        if favors == 0 {
            return false;
        }
        match self.al.ally {
            ids::ORDER_OF_ASTROLOGERS => {
                let n = favors.min(cards_left).min(ALL as u32 - 1) as u8;
                self.stack.push(Frame { ordered: true, ..select(p, ids::ORDER_OF_ASTROLOGERS, Zone::Discard, Act::Topdeck, Filter::Any, 0, n, Then::AstrologersShuffle) });
                true
            }
            ids::ORDER_OF_MASONS => {
                let n = (2 * favors).min(cards_left).min(ALL as u32 - 1) as u8;
                self.stack.push(select(p, ids::ORDER_OF_MASONS, Zone::Discard, Act::SetAside, Filter::Any, 0, n, Then::MasonsShuffle));
                true
            }
            _ => false,
        }
    }

    // ---- frames ----

    /// The Name frame of Sorceress (the player names a card) or Sorcerer (a victim names one) has its card: the top card of the deck is revealed.
    #[inline(never)]
    pub(crate) fn run_al_named<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        let top = take_top!(self, p, sink);
        self.stack.pop();
        let Some(c) = top else { return Run::Continue };
        self.reveal_card(p, c, sink);
        if f.then == Then::SorceressNamed {
            // Into the hand either way; a match gives each other player a Curse.
            self.players[p as usize].hand.add(c, 1);
            sink.event(Event::Draw { player: p, card: c });
            if c == f.subject {
                let (vs, n) = self.victims(sink);
                self.push_gain_to_each(&vs[..n], id::CURSE, f.source);
            }
        } else {
            // Back on top of the deck; a wrong name is a Curse.
            self.players[p as usize].deck_known.push_top(c);
            if c != f.subject {
                self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
            }
        }
        Run::Continue
    }

    /// The top card of the deck of `f.player`: Courier discards it (then an Action or Treasure may be played from the discard pile), Barbarian trashes it.
    #[inline(never)]
    pub(crate) fn run_al_top_card<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        let top = take_top!(self, p, sink);
        self.stack.pop();
        if f.then == Then::BarbarianHit {
            match top {
                // No card to trash: not a card costing $3 or more, so a Curse.
                None => self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) }),
                Some(c) => {
                    let cost = self.cost(c);
                    if cost >= 3 {
                        // A cheaper card that shares a type with it (every part of the cost no larger, and not equal).
                        self.stack.push(Frame {
                            less: true, potion: cards::potion_cost(c), depth: f.depth, ..gain_frame(p, f.source, cost, Filter::SharesTypeWith(c), Dest::Discard)
                        });
                    } else {
                        self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
                    }
                    self.trash_card(p, c, true, sink);
                    self.flush_trash_triggers(f.depth, sink);
                }
            }
            return Run::Continue;
        }
        // Courier.
        if let Some(c) = top {
            self.players[p as usize].discard.add(c, 1);
            sink.event(Event::Discard { player: p, card: c });
            self.stack.push(Frame {
                depth: f.depth,
                ..select(p, f.source, Zone::Discard, Act::Play, Filter::ActionOrTreasure, 0, 1, Then::PlayPicked { times: 1 })
            });
            if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                self.discard_hook(p, c, 1);
                self.flush_trash_triggers(f.depth, sink);
            }
        }
        Run::Continue
    }

    /// A `RevealTop` of the set has finished: Miller (one of the top 4 into the hand) and Hunter (an Action, a Treasure and a Victory card of the top 3).
    pub(crate) fn al_reveal_done<S: EventSink>(&mut self, f: &Frame, _sink: &mut S) {
        let (p, src, d) = (f.player, f.source, f.depth);
        match f.then {
            Then::MillerPick => {
                self.stack.push(Frame { depth: d, ..select(p, src, Zone::Revealed, Act::ToHand, Filter::Any, 1, 1, Then::DiscardRevealed) });
            }
            _ => {
                // Pushed last-first: an Action, then a Treasure, then a Victory card (which discards the rest).
                self.stack.push(Frame { depth: d, ..select(p, src, Zone::Revealed, Act::ToHand, Filter::Victory, 1, 1, Then::DiscardRevealed) });
                self.stack.push(Frame { depth: d, ..select(p, src, Zone::Revealed, Act::ToHand, Filter::Treasure, 1, 1, Then::Nothing) });
                self.stack.push(Frame { depth: d, ..select(p, src, Zone::Revealed, Act::ToHand, Filter::Action, 1, 1, Then::Nothing) });
            }
        }
    }

    /// What a gain of the set does once the card `c` has been gained: Hill Fort's Mode frame (under the gain's reactions) is told which card it may move.
    pub(crate) fn al_gained(&mut self, f: &Frame, c: CardId, dest: Dest) {
        let zone = GameState::gain_dest_zone(dest);
        let gt = self.turn.led.gain;
        let len = self.stack.len as usize;
        for fr in self.stack.frames[..len].iter_mut().rev() {
            if fr.kind == K::Mode && fr.source == f.source && fr.player == f.player && fr.subject == 0 {
                fr.subject = c;
                fr.zone = zone;
                fr.tok = gt;
                break;
            }
        }
    }

    /// The `then` of an Allies `Select` that has finished.
    #[inline(never)]
    pub(crate) fn finish_al_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let (p, src, d) = (f.player, f.source, f.depth);
        let pi = p as usize;
        match f.then {
            // Battle Plan: the pile of the card picked rotates.
            Then::RotatePicked => {
                if f.count > 0 {
                    self.al_rotate_for(p, src, f.last, sink);
                }
            }
            Then::CoinsIfPicked(n) => {
                if f.count > 0 {
                    self.turn.coins += n as u16;
                    sink.bonus(p, src, 0, 0, n as u16, 0);
                }
            }
            // Student: a Treasure trashed gives a Favor and the Student goes onto the deck (if it is still in play).
            Then::StudentTrash => {
                if f.count > 0 && self.is_treasure(f.last) {
                    self.give_favors(p, src, 1, sink);
                    if self.leave_play_self(p, src, f.tok) {
                        self.cover_deck(p);
                        self.players[pi].deck_known.push_top(src);
                        sink.event(Event::Topdeck { player: p, card: src });
                    }
                }
            }
            // Broker, Modify: choose what the card trashed buys (the Mode frame knows the card).
            Then::BrokerMode | Then::ModifyMode => {
                if f.count > 0 {
                    self.stack.push(Frame { subject: f.last, depth: d, ..crate::effects::elder_mode_frame(p, src, 1, f.gf & GF_ELDER != 0) });
                }
            }
            // Royal Galley: the card is played; afterwards the Galley (and the card, if it is still in play) wait for the next turn.
            Then::GalleyPlay => {
                if f.count > 0 {
                    let card = f.last;
                    let tok = self.track(p, Zone::InPlay, card);
                    self.stack.push(Frame { subject: card, tok, depth: d, ..select(p, src, Zone::Hand, Act::Choose, Filter::Any, 0, 0, Then::GalleySchedule) });
                    self.stack.push(Frame { subject: card, count: 1, tok, depth: d, ..Frame::new(K::PlayEffects, p, src) });
                }
            }
            Then::GalleySchedule => {
                let card = f.subject;
                if self.self_in_play(p, card, f.tok) {
                    self.push_duration_pending(p, src, 1, card);
                    if p == self.turn.player {
                        self.turn.duration_held.add(card, 1);
                    }
                }
                self.release_tok(f.tok);
            }
            // Contract: the Action is set aside to be played at the start of the next turn; the Contract stays in play for it.
            Then::ContractAside => {
                if f.count > 0 {
                    let ps = &mut self.players[pi];
                    let room = ps.men.aside.has(f.last) || ps.men.aside.iter().count() < crate::state::MEN_ASIDE_CAP;
                    if room && ps.hand.remove(f.last) {
                        ps.men.aside.add(f.last, 1);
                        sink.event(Event::SetAside { player: p, card: f.last });
                        self.push_duration_pending(p, src, 1, f.last);
                    }
                }
            }
            // Specialist: the card is played, then (once that is done) played again or copied.
            Then::SpecialistPlay => {
                if f.count > 0 {
                    self.stack.push(Frame { subject: f.last, depth: d, ..mode_frame(p, src, 1) });
                    self.push_plays(f, 1);
                }
            }
            // Elder: the card is played with an extra pick for the "choose" decisions it makes.
            Then::ElderPlay => {
                if f.count > 0 {
                    self.stack.push(Frame { subject: f.last, count: 1, gf: GF_ELDER, depth: d, ..Frame::new(K::PlayEffects, p, src) });
                }
            }
            Then::SwapGain => {
                if f.count > 0 {
                    self.stack.push(Frame { depth: d, ..gain_frame(p, src, 5, Filter::ActionNot(f.last), Dest::Hand) });
                }
            }
            // Emissary: if the draw shuffled, +1 Action and +2 Favors.
            Then::EmissaryCheck => {
                if self.turn.al.shuffles as u16 != f.cost_sum {
                    if p == self.turn.player {
                        self.turn.actions += 1;
                        sink.bonus(p, src, 1, 0, 0, 0);
                    }
                    self.give_favors(p, src, 2, sink);
                }
            }
            // Family of Inventors: a token on the pile of the card picked.
            Then::FamilyToken => {
                if f.count > 0 {
                    let pile = cards::pile_of(f.last);
                    let fam = &mut self.al.fam;
                    if fam.has(pile) || fam.iter().count() < crate::state::AL_FAM_CAP {
                        fam.add(pile, 1);
                    }
                }
            }
            // Peaceful Cult: a Favor for each card trashed.
            Then::CultSpend => self.spend_favors(p, src, f.count, sink),
            // Woodworkers' Guild: an Action trashed, an Action gained.
            Then::WoodworkersGain => {
                if f.count > 0 {
                    self.stack.push(Frame { gf: GF_ANY_COST, depth: d, ..gain_frame(p, src, u8::MAX, Filter::Action, Dest::Discard) });
                }
            }
            // Coastal Haven: the cards set aside are kept for the next hand.
            Then::HavenKeep => {
                let ps = &mut self.players[pi];
                let kept = ps.set_aside;
                let n = kept.total().min(u8::MAX as u32) as u8;
                ps.set_aside.clear();
                for (c, k) in kept.iter() {
                    if ps.al.keep.has(c) || ps.al.keep.iter().count() < crate::state::AL_KEEP_CAP {
                        ps.al.keep.add(c, k);
                    } else {
                        ps.discard.add(c, k);
                    }
                }
                self.spend_favors(p, src, n, sink);
            }
            Then::KeepToHand => {
                let ps = &mut self.players[pi];
                let kept = ps.al.keep.take_all();
                for (c, k) in kept.iter() {
                    ps.hand.add(c, k);
                    for _ in 0..k {
                        sink.event(Event::Draw { player: p, card: c });
                    }
                }
            }
            // Order of Astrologers: the cards picked are on top of the deck; the rest is shuffled below them.
            Then::AstrologersShuffle => {
                self.spend_favors(p, src, f.count, sink);
                self.shuffle_discard(p, sink);
            }
            // Order of Masons: the cards set aside stay in the discard pile; the rest is shuffled into the deck.
            Then::MasonsShuffle => {
                let kept = self.players[pi].set_aside;
                self.players[pi].set_aside.clear();
                self.spend_favors(p, src, (f.count + 1) / 2, sink);
                self.shuffle_discard(p, sink);
                self.players[pi].discard.add_all(&kept);
            }
            _ => {}
        }
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
                "Importer", "Merchant Camp", "Underling", "Barbarian", "Emissary", "Galleria", "Guildmaster", "Highwayman", "Skirmisher", "Warlord", "Garrison",
                "Voyage", "Sorcerer", "Lich",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Sycophant", "Sentinel", "Broker", "Carpenter", "Courier", "Innkeeper", "Royal Galley", "Capital City", "Hunter", "Modify", "Specialist", "Swap",
                "Marquis", "Herb Gatherer", "Acolyte", "Sorceress", "Sibyl", "Battle Plan", "Old Map", "Elder", "Student",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
        assert_eq!(
            names(cards::OnPlay::Choice),
            [
                "Bauble", "Town", "Contract", "Archer", "Tent", "Hill Fort", "Stronghold", "Sunken Treasure", "Distant Shore", "Town Crier", "Blacksmith",
                "Miller", "Conjurer",
            ]
        );
        assert_eq!(CARDS.len(), 25 + 24 + 23);
    }
}
