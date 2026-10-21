//! Menagerie (30 kingdom cards, the Horse, 20 Events and 20 Ways). See `docs/menagerie-plan.md` for the design and the rulings.
//!
//! * **Exile** is a mat per player (`PlayerState::men.exile`, a `SmallMultiset`); its cards are the player's. A gain may discard all the copies of the card from
//!   the mat (`H_EXILE` keeps games without a card on any mat from looking).
//! * **Horses** are a pile of 30 outside the Supply: they sit in `GameState::supply` (a lane) but not in `in_supply`, so nothing that gains "from the Supply" can take one.
//! * **Ways** are pseudo-cards (the highest ids): in a game with one, playing an Action card first asks which of its own effect and the Ways to use (`FrameKind::PrePlay`).
//! * The **Events** are the last 20 of the Events (`cards::NUM_EVENTS` 64); the dispatch is `event_effect` / `event_available` below, called from `sets::adventures`.

use super::prelude::*;
use crate::cards::{men, DURATION, SHELTER, ZOMBIE};
use crate::state::{
    GameConfig, PendingDuration, EXILE_CAP, GF_ANY_COST, GF_EXILE, H_EXILE, H_GATE, H_INVEST, MP_SEIZE_USED, MT_SEAL, MT_SEIZE, MT_SNOWY, M_COST, M_GAINS, M_TRASHES,
    M_WAYS,
};

/// Card ids of this set. The kingdom cards are the highest kingdom ids (after Villain, before the Ruins and Knights) and the Horse follows them; the Events follow the
/// Rising Sun Events; the Ways are the highest ids of all (after the Artifacts).
pub mod ids {
    use crate::cards::CardId;
    pub const BLACK_CAT: CardId = 407;
    pub const SLEIGH: CardId = 408;
    pub const SUPPLIES: CardId = 409;
    pub const CAMEL_TRAIN: CardId = 410;
    pub const GOATHERD: CardId = 411;
    pub const SCRAP: CardId = 412;
    pub const SHEEPDOG: CardId = 413;
    pub const SNOWY_VILLAGE: CardId = 414;
    pub const STOCKPILE: CardId = 415;
    pub const BOUNTY_HUNTER: CardId = 416;
    pub const CARDINAL: CardId = 417;
    pub const CAVALRY: CardId = 418;
    pub const GROOM: CardId = 419;
    pub const HOSTELRY: CardId = 420;
    pub const VILLAGE_GREEN: CardId = 421;
    pub const BARGE: CardId = 422;
    pub const COVEN: CardId = 423;
    pub const DISPLACE: CardId = 424;
    pub const FALCONER: CardId = 425;
    pub const GATEKEEPER: CardId = 426;
    pub const HUNTING_LODGE: CardId = 427;
    pub const KILN: CardId = 428;
    pub const LIVERY: CardId = 429;
    pub const MASTERMIND: CardId = 430;
    pub const PADDOCK: CardId = 431;
    pub const SANCTUARY: CardId = 432;
    pub const FISHERMAN: CardId = 433;
    pub const DESTRIER: CardId = 434;
    pub const WAYFARER: CardId = 435;
    pub const ANIMAL_FAIR: CardId = 436;
    /// The Horse (a zone id, not a kingdom card: it has no pile in the Supply).
    pub const HORSE: CardId = 437;
    // The Events (after the Rising Sun Events).
    pub const DELAY: CardId = 547;
    pub const DESPERATION: CardId = 548;
    pub const GAMBLE: CardId = 549;
    pub const PURSUE: CardId = 550;
    pub const RIDE: CardId = 551;
    pub const TOIL: CardId = 552;
    pub const ENHANCE: CardId = 553;
    pub const MARCH: CardId = 554;
    pub const TRANSPORT: CardId = 555;
    pub const BANISH: CardId = 556;
    pub const BARGAIN: CardId = 557;
    pub const INVEST: CardId = 558;
    pub const SEIZE_THE_DAY: CardId = 559;
    pub const COMMERCE: CardId = 560;
    pub const DEMAND: CardId = 561;
    pub const STAMPEDE: CardId = 562;
    pub const REAP: CardId = 563;
    pub const ENCLAVE: CardId = 564;
    pub const ALLIANCE: CardId = 565;
    pub const POPULATE: CardId = 566;
    // The Ways, the highest ids.
    pub const WAY_OF_THE_BUTTERFLY: CardId = 657;
    pub const WAY_OF_THE_CAMEL: CardId = 658;
    pub const WAY_OF_THE_CHAMELEON: CardId = 659;
    pub const WAY_OF_THE_FROG: CardId = 660;
    pub const WAY_OF_THE_GOAT: CardId = 661;
    pub const WAY_OF_THE_HORSE: CardId = 662;
    pub const WAY_OF_THE_MOLE: CardId = 663;
    pub const WAY_OF_THE_MONKEY: CardId = 664;
    pub const WAY_OF_THE_MOUSE: CardId = 665;
    pub const WAY_OF_THE_MULE: CardId = 666;
    pub const WAY_OF_THE_OTTER: CardId = 667;
    pub const WAY_OF_THE_OWL: CardId = 668;
    pub const WAY_OF_THE_OX: CardId = 669;
    pub const WAY_OF_THE_PIG: CardId = 670;
    pub const WAY_OF_THE_RAT: CardId = 671;
    pub const WAY_OF_THE_SEAL: CardId = 672;
    pub const WAY_OF_THE_SHEEP: CardId = 673;
    pub const WAY_OF_THE_SQUIRREL: CardId = 674;
    pub const WAY_OF_THE_TURTLE: CardId = 675;
    pub const WAY_OF_THE_WORM: CardId = 676;
    /// The Exile mat as a label: the source of the offer to discard the copies of a card gained from it.
    pub const EXILE: CardId = 677;
}

/// An Event: no types, no bonuses (what it does is `event_effect`); its cost is the cost to buy it.
const fn event(name: &'static str, cost: u8) -> CardDef {
    men(c(name, cost, 0, 0, 0, 0, 0, 0))
}

/// A Way: nothing to buy (what it does is `way_effect`).
const fn way(name: &'static str) -> CardDef {
    men(c(name, 0, 0, 0, 0, 0, 0, 0))
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::BLACK_CAT, men(choice_free(c("Black Cat", 2, ACTION | ATTACK | REACTION, 0, 0, 2, 0, 0)))), // a Curse for each other player when not played on its owner's turn
    (ids::SLEIGH, men(has_choice(c("Sleigh", 2, ACTION | REACTION, 0, 0, 0, 0, 0)))), // 2 Horses; moves a gained card
    (ids::SUPPLIES, men(choice_free(c("Supplies", 2, TREASURE, 1, 0, 0, 0, 0)))), // a Horse onto the deck
    (ids::CAMEL_TRAIN, men(has_choice(c("Camel Train", 3, ACTION, 0, 0, 0, 0, 0)))), // Exiles a non-Victory card from the Supply; a Gold when gained
    (ids::GOATHERD, men(order_sensitive(c("Goatherd", 3, ACTION, 0, 0, 0, 1, 0)))), // may trash a card; draws for the right-hand player's trashes
    (ids::SCRAP, men(order_sensitive(c("Scrap", 3, ACTION, 0, 0, 0, 0, 0)))), // trashes any card of the hand
    (ids::SHEEPDOG, men(choice_free(c("Sheepdog", 3, ACTION | REACTION, 0, 0, 2, 0, 0)))), // played when a card is gained
    (ids::SNOWY_VILLAGE, men(has_choice(c("Snowy Village", 3, ACTION, 0, 0, 1, 4, 1)))), // further +Actions are ignored: it is played after the others
    (ids::STOCKPILE, men(choice_free(c("Stockpile", 3, TREASURE, 3, 0, 0, 0, 1)))), // Exiles itself
    (ids::BOUNTY_HUNTER, men(order_sensitive(c("Bounty Hunter", 4, ACTION, 0, 0, 0, 1, 0)))), // Exiles any card of the hand
    (ids::CARDINAL, men(choice_free(c("Cardinal", 4, ACTION | ATTACK, 2, 0, 0, 0, 0)))), // Exiles a card costing $3 to $6 from the top 2 of each other deck
    (ids::CAVALRY, men(has_choice(c("Cavalry", 4, ACTION, 0, 0, 0, 0, 0)))), // 2 Horses; +2 Cards +1 Buy when gained
    (ids::GROOM, men(has_choice(c("Groom", 4, ACTION, 0, 0, 0, 0, 0)))), // a gain, then a bonus by its types
    (ids::HOSTELRY, men(choice_free(c("Hostelry", 4, ACTION, 0, 0, 1, 2, 0)))), // Treasures for Horses when gained
    (ids::VILLAGE_GREEN, men(has_choice(c("Village Green", 4, ACTION | DURATION | REACTION, 0, 0, 0, 0, 0)))), // now or next turn; played when discarded
    (ids::BARGE, men(has_choice(c("Barge", 5, ACTION | DURATION, 0, 0, 0, 0, 0)))), // now or next turn
    (ids::COVEN, men(choice_free(c("Coven", 5, ACTION | ATTACK, 2, 0, 0, 1, 0)))), // Exiles a Curse from the Supply for each other player
    (ids::DISPLACE, men(order_sensitive(c("Displace", 5, ACTION, 0, 0, 0, 0, 0)))), // Exiles any card of the hand
    (ids::FALCONER, men(has_choice(c("Falconer", 5, ACTION | REACTION, 0, 0, 0, 0, 0)))), // a cheaper card to the hand; played when a multi-type card is gained
    (ids::GATEKEEPER, men(choice_free(c("Gatekeeper", 5, ACTION | DURATION | ATTACK, 0, 0, 0, 0, 0)))), // +$3 next turn; Exiles the Actions and Treasures others gain until then
    (ids::HUNTING_LODGE, men(order_sensitive(c("Hunting Lodge", 5, ACTION, 0, 0, 1, 2, 0)))), // may discard the hand for 5 cards
    (ids::KILN, men(order_sensitive(c("Kiln", 5, ACTION, 2, 0, 0, 0, 0)))), // the next card played comes with a copy: a card played before it is not copied
    (ids::LIVERY, men(choice_free(c("Livery", 5, ACTION, 3, 0, 0, 0, 0)))), // a Horse for each card gained costing $4 or more
    (ids::MASTERMIND, men(choice_free(c("Mastermind", 5, ACTION | DURATION, 0, 0, 0, 0, 0)))), // plays an Action from the hand 3 times next turn
    (ids::PADDOCK, men(has_choice(c("Paddock", 5, ACTION, 2, 0, 0, 0, 0)))), // 2 Horses; +1 Action per empty pile
    (ids::SANCTUARY, men(order_sensitive(c("Sanctuary", 5, ACTION, 0, 0, 1, 1, 1)))), // may Exile any card of the hand
    (ids::FISHERMAN, men(choice_free(c("Fisherman", 5, ACTION, 1, 0, 1, 1, 0)))), // costs $3 less on its owner's turns with an empty discard pile
    (ids::DESTRIER, men(choice_free(c("Destrier", 6, ACTION, 0, 0, 2, 1, 0)))), // costs $1 less per card gained this turn
    (ids::WAYFARER, men(has_choice(c("Wayfarer", 6, ACTION, 0, 0, 3, 0, 0)))), // may gain a Silver; costs what the last card gained did
    (ids::ANIMAL_FAIR, men(choice_free(c("Animal Fair", 7, ACTION, 4, 0, 0, 0, 0)))), // +1 Buy per empty pile; may be bought by trashing an Action
    (ids::HORSE, men(choice_free(c("Horse", 3, ACTION, 0, 0, 2, 1, 0)))), // returns to its pile; not in the Supply
    // The Events.
    (ids::DELAY, event("Delay", 0)),
    (ids::DESPERATION, event("Desperation", 0)),
    (ids::GAMBLE, event("Gamble", 2)),
    (ids::PURSUE, event("Pursue", 2)),
    (ids::RIDE, event("Ride", 2)),
    (ids::TOIL, event("Toil", 2)),
    (ids::ENHANCE, event("Enhance", 3)),
    (ids::MARCH, event("March", 3)),
    (ids::TRANSPORT, event("Transport", 3)),
    (ids::BANISH, event("Banish", 4)),
    (ids::BARGAIN, event("Bargain", 4)),
    (ids::INVEST, event("Invest", 4)),
    (ids::SEIZE_THE_DAY, event("Seize the Day", 4)),
    (ids::COMMERCE, event("Commerce", 5)),
    (ids::DEMAND, event("Demand", 5)),
    (ids::STAMPEDE, event("Stampede", 5)),
    (ids::REAP, event("Reap", 7)),
    (ids::ENCLAVE, event("Enclave", 8)),
    (ids::ALLIANCE, event("Alliance", 10)),
    (ids::POPULATE, event("Populate", 10)),
    // The Ways.
    (ids::WAY_OF_THE_BUTTERFLY, way("Way of the Butterfly")),
    (ids::WAY_OF_THE_CAMEL, way("Way of the Camel")),
    (ids::WAY_OF_THE_CHAMELEON, way("Way of the Chameleon")),
    (ids::WAY_OF_THE_FROG, way("Way of the Frog")),
    (ids::WAY_OF_THE_GOAT, way("Way of the Goat")),
    (ids::WAY_OF_THE_HORSE, way("Way of the Horse")),
    (ids::WAY_OF_THE_MOLE, way("Way of the Mole")),
    (ids::WAY_OF_THE_MONKEY, way("Way of the Monkey")),
    (ids::WAY_OF_THE_MOUSE, way("Way of the Mouse")),
    (ids::WAY_OF_THE_MULE, way("Way of the Mule")),
    (ids::WAY_OF_THE_OTTER, way("Way of the Otter")),
    (ids::WAY_OF_THE_OWL, way("Way of the Owl")),
    (ids::WAY_OF_THE_OX, way("Way of the Ox")),
    (ids::WAY_OF_THE_PIG, way("Way of the Pig")),
    (ids::WAY_OF_THE_RAT, way("Way of the Rat")),
    (ids::WAY_OF_THE_SEAL, way("Way of the Seal")),
    (ids::WAY_OF_THE_SHEEP, way("Way of the Sheep")),
    (ids::WAY_OF_THE_SQUIRREL, way("Way of the Squirrel")),
    (ids::WAY_OF_THE_TURTLE, way("Way of the Turtle")),
    (ids::WAY_OF_THE_WORM, way("Way of the Worm")),
    (ids::EXILE, way("Exile")),
];

/// The bit of a Way in `MenState::ways`.
#[inline(always)]
pub const fn way_bit(w: CardId) -> u32 {
    1 << (w - crate::cards::FIRST_WAY)
}

/// The cards that gain Horses: a game with one of them (or an Event of the set that does) has the Horse pile.
const HORSE_GAINERS: [CardId; 8] = [ids::SLEIGH, ids::SUPPLIES, ids::SCRAP, ids::CAVALRY, ids::GROOM, ids::HOSTELRY, ids::LIVERY, ids::PADDOCK];
const HORSE_EVENTS: [CardId; 4] = [ids::RIDE, ids::BARGAIN, ids::DEMAND, ids::STAMPEDE];

/// The Menagerie cards (see the module docs of `sets`).
pub struct Menagerie;

impl SetRules for Menagerie {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Cards with a "when you gain" effect (Camel Train, Cavalry, Hostelry) or a reaction to gains (Black Cat, Sleigh, Sheepdog, Falconer).
    const GAIN_EXT: u128 = (1u128 << (ids::BLACK_CAT - 384))
        | (1u128 << (ids::SLEIGH - 384))
        | (1u128 << (ids::CAMEL_TRAIN - 384))
        | (1u128 << (ids::SHEEPDOG - 384))
        | (1u128 << (ids::CAVALRY - 384))
        | (1u128 << (ids::HOSTELRY - 384))
        | (1u128 << (ids::FALCONER - 384));

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            // The card gained goes into the hand or onto the deck.
            ids::SLEIGH => &[ModeOpt::MoveGained(Dest::Hand), ModeOpt::MoveGained(Dest::DeckTop)],
            // One different thing per $1 the card trashed cost, in this order.
            ids::SCRAP => &[
                ModeOpt::Cards(1),
                ModeOpt::Actions(1),
                ModeOpt::Buys(1),
                ModeOpt::Coins(1),
                ModeOpt::GainMany(id::SILVER, Dest::Discard, 1),
                ModeOpt::GainMany(ids::HORSE, Dest::Discard, 1),
            ],
            ids::TRANSPORT => &[ModeOpt::ExileFromSupply(Filter::Action), ModeOpt::ExileToDeck(Filter::Action)],
            _ => &[],
        }
    }

    /// The start-of-turn parts of the Durations: Village Green's and Barge's second option and Gatekeeper's $3.
    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::VILLAGE_GREEN => (1, 2, 0, 0),
            ids::BARGE => (3, 0, 1, 0),
            ids::GATEKEEPER => (0, 0, 0, 3),
            _ => (0, 0, 0, 0),
        }
    }

    /// The start-of-turn effects that play a card (their order can matter).
    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, ids::MASTERMIND | ids::DELAY | ids::REAP | ids::WAY_OF_THE_TURTLE)
    }

    /// Delay, Reap and Way of the Turtle: two entries holding different cards are different effects.
    fn start_entry_named_by_arg(card: CardId) -> bool {
        matches!(card, ids::DELAY | ids::REAP | ids::WAY_OF_THE_TURTLE)
    }

    /// Village Green and Barge ask "now or next turn" on play, and schedule themselves if it is next turn.
    fn schedules_own_duration(card: CardId) -> bool {
        matches!(card, ids::VILLAGE_GREEN | ids::BARGE)
    }

    /// Paddock's Actions and Animal Fair's Buys count the empty piles.
    fn reads_empty_piles(card: CardId) -> bool {
        matches!(card, ids::PADDOCK | ids::ANIMAL_FAIR)
    }

    /// The Horse pile has 30 cards (it is in `supply` but not in the Supply).
    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        if card == ids::HORSE { Some(30) } else { None }
    }

    /// A card set aside by Delay, Reap or Way of the Turtle is played at the start of the next turn (it uses no Action).
    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
        if !matches!(e.card, ids::DELAY | ids::REAP | ids::WAY_OF_THE_TURTLE) {
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

    /// Mastermind: an Action card from the hand is played three times.
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) {
        if e.card == ids::MASTERMIND {
            for _ in 0..e.times {
                g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Hand, Act::Play, Filter::Action, 0, 1, Then::PlayPicked { times: 3 }) });
            }
        }
    }

    #[inline(never)]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        let own = p == g.turn.player;
        match card {
            ids::BLACK_CAT => {
                if !own {
                    // Each other player gains a Curse, in turn order starting with the turn's player (not with the player of the Cat).
                    let n = g.num_players;
                    let mut order = [0u8; MAX_PLAYERS];
                    let mut k = 0;
                    for i in 0..n {
                        let q = (g.turn.player + i) % n;
                        if q != p && !g.immune(q) {
                            order[k] = q;
                            k += 1;
                        }
                    }
                    g.push_gain_to_each(&order[..k], id::CURSE, card);
                    g.players[pi].discard_next_cleanup.add(card, 1);
                }
            }
            ids::SLEIGH | ids::CAVALRY => {
                g.gain_horses(p, card, 2, Dest::Discard, sink);
            }
            ids::CAMEL_TRAIN => g.stack.push(Frame { gf: GF_EXILE | GF_ANY_COST, ..gain_frame(p, card, u8::MAX, Filter::NonVictory, Dest::Discard) }),
            ids::GOATHERD => {
                let n = g.players[g.right_of(p) as usize].men.trashed_last;
                if n > 0 {
                    g.stack.push(draw_frame(p, card, n));
                }
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing));
            }
            ids::SCRAP => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::ModePerCost)),
            ids::SHEEPDOG | ids::FALCONER => {
                if !own {
                    g.players[pi].discard_next_cleanup.add(card, 1);
                }
                if card == ids::FALCONER {
                    // A card costing less than this, into the hand.
                    g.stack.push(Frame { less: true, ..gain_frame(p, card, g.cost(card), Filter::Any, Dest::Hand) });
                }
            }
            ids::SNOWY_VILLAGE => {
                // Further +Actions this turn are ignored: the Actions are capped at what there are now.
                if own {
                    if g.turn.men.flags & MT_SNOWY != 0 {
                        g.turn.actions = g.turn.actions.min(g.turn.men.cap);
                    } else {
                        g.turn.men.flags |= MT_SNOWY;
                        g.turn.men.cap = g.turn.actions;
                    }
                }
            }
            ids::BOUNTY_HUNTER => g.stack.push(select(p, card, Zone::Hand, Exile, Filter::Any, 1, 1, Then::BountyHunter)),
            ids::CARDINAL => {
                // Each other player reveals the top 2 cards, Exiles one costing $3 to $6 and discards the rest.
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(select(v, card, Zone::Revealed, Exile, Filter::CostRange(3, 6), 1, 1, Then::DiscardRevealed));
                    g.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, v, card) });
                }
            }
            ids::GROOM => g.stack.push(Frame { then: Then::GroomBonus, ..gain_frame(p, card, 4, Filter::Any, Dest::Discard) }),
            ids::PADDOCK => {
                g.gain_horses(p, card, 2, Dest::Discard, sink);
                let empty = g.empty_piles().min(u8::MAX as u32) as u8;
                if own && empty > 0 {
                    g.turn.actions += empty;
                    sink.bonus(p, card, empty, 0, 0, 0);
                }
            }
            ids::VILLAGE_GREEN | ids::BARGE => {
                g.stack.push(Frame { then: Then::NowOrNext, ..Frame::new(K::Ask, p, card) });
            }
            ids::COVEN => {
                // Each other player Exiles a Curse from the Supply; one that can't discards the Curses on their mat.
                let (vs, n) = g.victims(sink);
                for &v in &vs[..n] {
                    if g.supply.get(id::CURSE) > 0 {
                        g.exile_from_supply(v, id::CURSE, false, sink);
                    } else {
                        g.discard_exiled(v, id::CURSE, sink);
                    }
                }
            }
            ids::DISPLACE => g.stack.push(select(p, card, Zone::Hand, Exile, Filter::Any, 1, 1, Then::DisplaceGain)),
            ids::GATEKEEPER => {
                if own {
                    g.players[pi].men.gk = g.players[pi].men.gk.saturating_add(1);
                    g.men.hook |= H_GATE;
                    g.shield_lingering_attack(p, sink);
                }
            }
            ids::HUNTING_LODGE => g.stack.push(Frame { then: Then::HuntingLodge, ..Frame::new(K::Ask, p, card) }),
            ids::KILN => {
                if own {
                    g.turn.men.kiln = g.turn.men.kiln.saturating_add(1);
                }
            }
            ids::LIVERY => {
                if own {
                    g.turn.men.livery = g.turn.men.livery.saturating_add(1);
                }
            }
            ids::SANCTUARY => g.stack.push(select(p, card, Zone::Hand, Exile, Filter::Any, 0, 1, Then::Nothing)),
            ids::WAYFARER => g.stack.push(Frame { optional: true, ..gain_frame(p, card, u8::MAX, Filter::Card(id::SILVER), Dest::Discard) }),
            ids::ANIMAL_FAIR => {
                let empty = g.empty_piles().min(u8::MAX as u32) as u8;
                if own && empty > 0 {
                    g.turn.buys += empty;
                    sink.bonus(p, card, 0, empty, 0, 0);
                }
            }
            ids::HORSE => {
                // Back to its pile (when it is in play: not a copy played from where it stays).
                let t = g.turn.led.cur;
                g.return_to_own_pile_from_play(p, card, t, sink);
            }
            ids::SUPPLIES | ids::STOCKPILE => g.men_treasure(card, 1, sink),
            _ => {}
        }
    }

    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, _on_buy: bool, sink: &mut S) {
        g.men_gain_triggers(p, c, to, sink);
    }

    /// The effects that look at every gain once they are on: a card on an Exile mat, Invest, a Gatekeeper's Attack, Livery and Way of the Seal this turn.
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.men.hook != 0 || g.turn.men.livery != 0 || g.turn.men.flags & MT_SEAL != 0
    }

    /// Way of the Frog: the cards played as the Way are put onto the deck as they are discarded from play.
    #[inline(never)]
    fn offers_on_discard(g: &mut GameState) {
        if g.turn.men.frogs.is_empty() {
            return;
        }
        let p = g.turn.player;
        let ps = &g.players[p as usize];
        let mut n = 0u32;
        for (c, k) in g.turn.men.frogs.iter() {
            let discarded = ps.in_play.get(c).saturating_sub(g.turn.duration_held.get(c).saturating_add(ps.stays.get(c)));
            n += k.min(discarded) as u32;
        }
        if n > 0 {
            g.stack.push(Frame { ordered: true, ..select(p, ids::WAY_OF_THE_FROG, Zone::InPlay, Act::Topdeck, Filter::Frogged, n.min(ALL as u32 - 1) as u8, n.min(ALL as u32 - 1) as u8, Then::Nothing) });
        }
    }
}

// -------------------------------------------------------------------------------------------
// Set-up
// -------------------------------------------------------------------------------------------

/// Whether `c` could be Way of the Mouse's card in a game where `in_game` says which cards are in it: an unused non-Duration Action costing $2 or $3 that needs no
/// set-up of its own (no pile outside the Supply, no Heirloom, no Boons or Hexes, no Shelters).
fn mouse_ok(in_game: &dyn Fn(CardId) -> bool, c: CardId) -> bool {
    let t = cards::def(c).types;
    let ok_type = t & ACTION != 0
        && t & (DURATION | cards::FATE | cards::DOOM | cards::LOOTER | cards::KNIGHT | cards::TRAVELLER | cards::RESERVE | cards::NIGHT | cards::OMEN | cards::SHADOW) == 0;
    ok_type
        && (2..=3).contains(&cards::cost(c))
        && !cards::potion_cost(c)
        && cards::debt_cost(c) == 0
        && cards::is_ready(c)
        && cards::is_kingdom(c)
        && cards::heirloom_of(c) == 0
        && !in_game(c)
        && !matches!(c, id::YOUNG_WITCH | id::HERMIT | id::URCHIN | id::FERRYMAN)
        && !(cards::is_split_bottom(c) && cards::split_top(c).is_some_and(in_game))
}

/// The card set aside for Way of the Mouse when none was named: a candidate picked by the seed (the same one for the same kingdom and seed).
pub fn default_mouse(in_game: &dyn Fn(CardId) -> bool, seed: u64) -> CardId {
    let pool = || (cards::FIRST_KINGDOM..cards::NUM_ZONE_IDS as CardId).filter(|&c| mouse_ok(in_game, c));
    let n = pool().count();
    if n == 0 {
        return 0;
    }
    let mut z = seed.wrapping_add(0x4D4F_5553_4557_4159).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 32;
    pool().nth((z % n as u64) as usize).unwrap_or(0)
}

/// Menagerie set-up: the Horse pile, the Ways of the game, Way of the Mouse's card, and the flags of the effects this kingdom has.
pub(crate) fn setup(g: &mut GameState, cfg: &GameConfig, _n: usize) {
    let has = |c: CardId| cfg.kingdom.contains(&c);
    let mut ways = 0u32;
    for &k in &cfg.kingdom {
        if cards::is_way(k) {
            ways |= way_bit(k);
        }
    }
    g.men.ways = ways;
    if ways != 0 {
        g.men.flags |= M_WAYS;
    }
    let in_game = |c: CardId| g.in_supply(c) || has(c);
    if ways & way_bit(ids::WAY_OF_THE_MOUSE) != 0 {
        g.men.mouse = cfg.mouse.filter(|&c| mouse_ok(&in_game, c)).unwrap_or_else(|| default_mouse(&in_game, cfg.seed));
    }
    // The Horse pile is in the game when something in it gains a Horse (Way of the Mouse's card too).
    if HORSE_GAINERS.iter().any(|&c| g.in_supply(c) || c == g.men.mouse) || HORSE_EVENTS.iter().any(|&e| has(e)) {
        g.supply.set(ids::HORSE, 30);
    }
    if g.in_supply(ids::FISHERMAN) || g.in_supply(ids::DESTRIER) || g.in_supply(ids::WAYFARER) {
        g.men.flags |= M_COST;
    }
    if g.in_supply(ids::DESTRIER) || g.in_supply(ids::WAYFARER) || has(ids::COMMERCE) {
        g.men.flags |= M_GAINS;
    }
    if g.in_supply(ids::GOATHERD) {
        g.men.flags |= M_TRASHES;
    }
    // Discards are watched when Village Green can be played from a discard.
    if g.in_supply(ids::VILLAGE_GREEN) {
        g.adv |= crate::state::G_DISCARD_HOOK;
    }
}

// -------------------------------------------------------------------------------------------
// Events
// -------------------------------------------------------------------------------------------

/// Whether the Menagerie Event `e` can be bought now apart from its cost and the once-per-turn rule.
pub(crate) fn event_available(g: &GameState, e: CardId) -> bool {
    let p = g.turn.player as usize;
    match e {
        // Once per game.
        ids::SEIZE_THE_DAY => g.players[p].men.flags & MP_SEIZE_USED == 0,
        // Something to set aside.
        ids::DELAY => g.players[p].hand.iter().any(|(c, _)| g.is_action(c)),
        _ => true,
    }
}

/// What buying the Menagerie Event `e` does, after its cost and Buy are paid.
#[inline(never)]
pub(crate) fn event_effect<S: EventSink>(g: &mut GameState, e: CardId, p: u8, sink: &mut S) {
    use Act::*;
    let pi = p as usize;
    match e {
        ids::DELAY => g.stack.push(select(p, e, Zone::Hand, Choose, Filter::Action, 0, 1, Then::DelayAside)),
        ids::DESPERATION => {
            g.stack.push(Frame { optional: true, gf: GF_ANY_COST, then: Then::DesperationBonus, ..gain_frame(p, e, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) })
        }
        ids::GAMBLE => {
            g.turn.buys += 1;
            sink.bonus(p, e, 0, 1, 0, 0);
            // The top card is discarded; an Action or Treasure may be played from the discard pile.
            g.stack.push(Frame { filter: Filter::ActionOrTreasure, ..Frame::new(K::Vassal, p, e) });
        }
        ids::PURSUE => {
            g.turn.buys += 1;
            sink.bonus(p, e, 0, 1, 0, 0);
            g.stack.push(Frame { then: Then::PursueNamed, ..Frame::new(K::Name, p, e) });
        }
        ids::RIDE => g.gain_horses(p, e, 1, Dest::Discard, sink),
        ids::TOIL => {
            g.turn.buys += 1;
            sink.bonus(p, e, 0, 1, 0, 0);
            g.stack.push(select(p, e, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 1 }));
        }
        ids::ENHANCE => g.stack.push(select(
            p, e, Zone::Hand, Trash, Filter::NonVictory, 0, 1,
            Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
        )),
        ids::MARCH => g.stack.push(select(p, e, Zone::Discard, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 1 })),
        ids::TRANSPORT => g.stack.push(mode_frame(p, e, 1)),
        ids::BANISH => g.stack.push(select(p, e, Zone::Hand, Choose, Filter::Any, 0, 1, Then::BanishPick)),
        ids::BARGAIN => {
            // The card first, then a Horse for each other player (in turn order).
            let mut qs = [0u8; MAX_PLAYERS];
            let mut n = 0;
            for q in g.others(p) {
                qs[n] = q;
                n += 1;
            }
            for &q in qs[..n].iter().rev() {
                g.stack.push(gain_frame(q, e, u8::MAX, Filter::Card(ids::HORSE), Dest::Discard));
            }
            g.stack.push(gain_frame(p, e, 5, Filter::NonVictory, Dest::Discard));
        }
        ids::INVEST => g.stack.push(Frame { gf: GF_EXILE | GF_ANY_COST, then: Then::InvestExile, ..gain_frame(p, e, u8::MAX, Filter::Action, Dest::Discard) }),
        ids::SEIZE_THE_DAY => {
            g.turn.men.flags |= MT_SEIZE;
            g.players[pi].men.flags |= MP_SEIZE_USED;
        }
        ids::COMMERCE => {
            // First count the differently named cards gained this turn, then gain that many Golds.
            let n = g.turn.men.names.iter().count();
            for _ in 0..n {
                g.gain(p, id::GOLD, Dest::Discard, false, e, sink);
            }
        }
        ids::DEMAND => {
            g.gain_horses(p, e, 1, Dest::DeckTop, sink);
            g.stack.push(gain_frame(p, e, 4, Filter::Any, Dest::DeckTop));
        }
        ids::STAMPEDE => {
            if g.players[pi].in_play.total() <= 5 {
                g.gain_horses(p, e, 5, Dest::DeckTop, sink);
            }
        }
        ids::REAP => g.stack.push(Frame { then: Then::ReapAside, ..gain_frame(p, e, u8::MAX, Filter::Card(id::GOLD), Dest::Discard) }),
        ids::ENCLAVE => {
            g.gain(p, id::GOLD, Dest::Discard, false, e, sink);
            g.stack.push(Frame { gf: GF_EXILE | GF_ANY_COST, ..gain_frame(p, e, u8::MAX, Filter::Card(id::DUCHY), Dest::Discard) });
        }
        ids::ALLIANCE => {
            for c in [id::PROVINCE, id::DUCHY, id::ESTATE, id::GOLD, id::SILVER, id::COPPER] {
                g.gain(p, c, Dest::Discard, false, e, sink);
            }
        }
        ids::POPULATE => {
            // One card from each Action pile, in an order the player chooses: one gain for each pile that has a card to gain now.
            g.turn.men.pop.clear();
            let piles = g.supply_cards().filter(|&c| g.populate_ok(c)).count();
            for _ in 0..piles {
                g.stack.push(Frame { gf: GF_ANY_COST, then: Then::PopulateGain, ..gain_frame(p, e, u8::MAX, Filter::PopulateLeft, Dest::Discard) });
            }
        }
        _ => {}
    }
}

// -------------------------------------------------------------------------------------------
// The state's Menagerie parts
// -------------------------------------------------------------------------------------------

impl GameState {
    /// `n` Horses for `p` from the pile, one at a time (a pile with fewer gives what is left), to `dest`.
    pub(crate) fn gain_horses<S: EventSink>(&mut self, p: u8, source: CardId, n: u8, dest: Dest, sink: &mut S) {
        for _ in 0..n {
            if !self.gain_side_horse(p, dest, source, sink) {
                break;
            }
        }
    }

    /// One Horse from the pile (outside the Supply, but its count is in `supply`) for `p`, to `dest`, with the gain triggers.
    fn gain_side_horse<S: EventSink>(&mut self, p: u8, dest: Dest, source: CardId, sink: &mut S) -> bool {
        self.gain(p, ids::HORSE, dest, false, source, sink)
    }

    /// The cards left in the Horse pile.
    pub fn horses_left(&self) -> u8 {
        self.supply.get(ids::HORSE)
    }

    /// Whether the game has the Horse pile: some card or Event of it gains Horses (Way of the Mouse's card too).
    pub fn has_horse_pile(&self) -> bool {
        self.supply.get(ids::HORSE) > 0
            || HORSE_GAINERS.iter().any(|&c| self.in_supply(c) || c == self.men.mouse)
            || HORSE_EVENTS.iter().any(|&e| self.events & (1u64 << (e - crate::cards::FIRST_EVENT)) != 0)
    }

    /// Recomputes `MenState::hook` from the Exile mats, Invest and the Gatekeepers.
    #[inline(never)]
    pub(crate) fn men_update_hook(&mut self) {
        let mut h = 0;
        for ps in self.players.iter() {
            if !ps.men.exile.is_empty() {
                h |= H_EXILE;
            }
            if !ps.men.invest.is_empty() {
                h |= H_INVEST;
            }
            if ps.men.gk > 0 {
                h |= H_GATE;
            }
        }
        self.men.hook = h;
    }

    /// One copy of `c` leaves `p`'s Exile mat (a discard, a card put onto the deck); the Invested copies are the last to go.
    pub(crate) fn exile_remove(&mut self, p: u8, c: CardId) -> bool {
        let m = &mut self.players[p as usize].men;
        if !m.exile.remove(c) {
            return false;
        }
        if m.invest.get(c) > m.exile.get(c) {
            m.invest.remove(c);
        }
        true
    }

    /// `p` Exiles a card from the Supply: the top card of the pile `c` goes onto their mat (not a gain: no triggers). `invested` marks it as Invested.
    pub(crate) fn exile_from_supply<S: EventSink>(&mut self, p: u8, c: CardId, invested: bool, sink: &mut S) -> bool {
        // (A mat with 24 different cards on it is full: the Exile fails and the card stays in the Supply.)
        let full = {
            let m = &self.players[p as usize].men;
            !m.exile.has(c) && m.exile.iter().count() >= EXILE_CAP
        };
        if full || !self.supply.remove(c) {
            return false;
        }
        self.pile_taken(c);
        let m = &mut self.players[p as usize].men;
        m.exile.add(c, 1);
        if invested {
            m.invest.add_or_drop(c, 1);
        }
        sink.event(Event::Exile { player: p, card: c });
        self.men_update_hook();
        if invested {
            self.invest_draws(p, c);
        }
        true
    }

    /// Other players who have Invested in `c` draw 2 Cards for each: `p` gained or Invested in a copy.
    pub(crate) fn invest_draws(&mut self, p: u8, c: CardId) {
        if self.men.hook & H_INVEST == 0 {
            return;
        }
        let mut qs = [(0u8, 0u8); MAX_PLAYERS];
        let mut n = 0;
        for q in self.others(p) {
            let k = self.players[q as usize].men.invest.get(c);
            if k > 0 && !(q == p) {
                qs[n] = (q, k);
                n += 1;
            }
        }
        for &(q, k) in qs[..n].iter().rev() {
            self.stack.push(draw_frame(q, ids::INVEST, k.saturating_mul(2)));
        }
    }

    /// Every copy of `c` on `p`'s Exile mat is discarded (Coven: a player who can't Exile a Curse).
    pub(crate) fn discard_exiled<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        let n = self.players[p as usize].men.exile.get(c);
        for _ in 0..n {
            if self.exile_remove(p, c) {
                self.put(p, c, Act::Discard, Dest::Discard, ids::COVEN, sink);
            }
        }
        self.men_update_hook();
        if n > 0 && self.adv & crate::state::G_DISCARD_HOOK != 0 {
            self.flush_trash_triggers(0, sink);
        }
    }

    /// The number of types `c` has now (Falconer): its printed types, a Curse that is a Treasure too (Charlatan), an Action that is a Treasure (Capitalism).
    pub(crate) fn type_count(&self, c: CardId) -> u32 {
        let d = cards::def(c);
        d.types.count_ones() + (d.types & TREASURE == 0 && self.is_treasure(c)) as u32
    }

    /// Whether `p` is under the Attack of some Gatekeeper (one that was not shielded against: Moat, Lighthouse when it was played).
    pub(crate) fn gatekeeper_hits(&self, p: u8) -> bool {
        (0..self.num_players).any(|a| {
            let owner = &self.players[a as usize];
            a != p && owner.men.gk > 0 && owner.shielded & (1 << p) == 0
        })
    }

    /// Whether `c` is the top card of an Action pile that Populate has not gained from yet this turn.
    pub(crate) fn populate_ok(&self, c: CardId) -> bool {
        if self.supply.get(c) == 0 || !self.pile_open(c) {
            return false;
        }
        let pile = cards::pile_of(c);
        let action_pile = pile == id::KNIGHTS || pile == id::RUINS || cards::is(pile, ACTION);
        action_pile && !self.turn.men.pop.has(pile)
    }

    /// What `c` cost when it was gained to `to` (Livery): a Fisherman gained to an empty discard pile cost $2 then.
    pub(crate) fn cost_when_gained(&self, p: u8, c: CardId, to: Dest) -> u8 {
        if self.men.flags & M_COST != 0 && p == self.turn.player {
            let discard_was_empty = self.players[p as usize].discard.total() == (to == Dest::Discard) as u32;
            if c == ids::FISHERMAN && !discard_was_empty {
                return self.cost(c);
            }
            if c == ids::DESTRIER {
                return (self.cost(c) as u32 + 1).min(6) as u8;
            }
        }
        self.cost(c)
    }

    /// The printed-cost part of `c` when it is one of the cards whose cost moves (Fisherman, Destrier, Wayfarer): `Ok(base)` before the turn's reductions, `Err(cost)` when
    /// the card costs exactly that (Wayfarer, after a card was gained).
    #[inline(never)]
    pub(crate) fn men_base_cost(&self, c: CardId) -> Result<u32, u8> {
        // Allies: Family of Inventors' tokens on the card's pile (this flag is set with `M_COST`, so the one test in `cost` covers both sets).
        let fam = if self.al.flags & crate::state::AF_FAMILY != 0 { self.al_fam_less(c) as u32 } else { 0 };
        match c {
            ids::FISHERMAN => Ok((if self.players[self.turn.player as usize].discard.is_empty() { 2u32 } else { 5 }).saturating_sub(fam)),
            ids::DESTRIER => Ok(6u32.saturating_sub(self.turn.men.gains as u32).saturating_sub(fam)),
            ids::WAYFARER if self.turn.men.last != 0 => Err(self.cost(self.turn.men.last)),
            _ => Ok((cards::cost(c) as u32).saturating_sub(fam)),
        }
    }

    /// A card was gained (anyone's): Wayfarer's cost follows the last one; the turn's player's gains are counted and named (Destrier, Commerce).
    #[inline(never)]
    pub(crate) fn men_track_gain(&mut self, p: u8, c: CardId) {
        if c != ids::WAYFARER {
            self.turn.men.last = c;
        }
        if p == self.turn.player {
            self.turn.men.gains = self.turn.men.gains.saturating_add(1);
            self.turn.men.names.add_or_drop(c, 1);
        }
    }

    /// The "when you gain" triggers of the set: the gained card's own, the cards of the hand that react, the Exile mat and Invest, Livery, Way of the Seal, a Gatekeeper's
    /// Attack. Frames are pushed so that the last pushed is asked first (the Exile mat's offer) and the Gatekeeper's forced Exile resolves last, after Sleigh could save the card.
    #[inline(never)]
    pub(crate) fn men_gain_triggers<S: EventSink>(&mut self, p: u8, c: CardId, to: Dest, sink: &mut S) {
        let pi = p as usize;
        let zone = GameState::gain_dest_zone(to);
        let gt = self.turn.led.ingain;
        let own_turn = p == self.turn.player;
        // Gatekeeper: an Action or Treasure the player has no Exiled copy of is Exiled (the last thing to resolve).
        if self.men.hook & H_GATE != 0
            && (cards::is(c, ACTION) || self.is_treasure(c))
            && !self.players[pi].men.exile.has(c)
            && self.gatekeeper_hits(p)
        {
            self.stack.push(Frame { zone, act: Act::Exile, subject: c, then: Then::ForcedExile, tok: gt, ..Frame::new(K::YesNo, p, ids::GATEKEEPER) });
        }
        // Way of the Seal: the card gained may be put onto the deck.
        if self.turn.men.flags & MT_SEAL != 0 && own_turn && to != Dest::DeckTop {
            self.stack.push(Frame { zone, act: Act::Topdeck, subject: c, tok: gt, ..Frame::new(K::YesNo, p, ids::WAY_OF_THE_SEAL) });
        }
        // The card's own "when you gain this".
        match c {
            ids::CAMEL_TRAIN => self.stack.push(Frame { gf: GF_EXILE | GF_ANY_COST, ..gain_frame(p, c, u8::MAX, Filter::Card(id::GOLD), Dest::Discard) }),
            ids::HOSTELRY => self.stack.push(select(p, c, Zone::Hand, Act::Discard, Filter::Treasure, 0, ALL, Then::GainPerPick { card: ids::HORSE })),
            ids::CAVALRY => {
                self.stack.push(draw_frame(p, c, 2));
                if own_turn {
                    self.turn.buys += 1;
                    sink.bonus(p, c, 0, 1, 0, 0);
                    // Gained in the Buy phase: back to the Action phase (no Actions are given).
                    if self.turn.phase == Phase::Buy {
                        self.turn.phase = Phase::Action;
                        self.turn.treasures_done = false;
                        sink.event(Event::PhaseStart { player: p, phase: Phase::Action });
                    }
                }
            }
            _ => {}
        }
        // Livery: a Horse for each Livery played, for a card costing $4 or more.
        if self.turn.men.livery > 0 && own_turn && self.cost_when_gained(p, c, to) >= 4 {
            for _ in 0..self.turn.men.livery {
                if !self.gain_side_horse(p, Dest::Discard, ids::LIVERY, sink) {
                    break;
                }
            }
        }
        // Invest: the other players who Invested in this card draw.
        if self.men.hook & H_INVEST != 0 {
            self.invest_draws(p, c);
        }
        // Reactions from the hand: asked in turn order from the turn's player; one offer per copy.
        if self.in_supply(ids::BLACK_CAT) && cards::is(c, VICTORY) {
            let n = self.num_players;
            for i in (0..n).rev() {
                let q = (self.turn.player + i) % n;
                if q != p {
                    for _ in 0..self.players[q as usize].hand.get(ids::BLACK_CAT).min(4) {
                        self.stack.push(Frame { zone: Zone::Hand, act: Act::Play, subject: ids::BLACK_CAT, ..Frame::new(K::YesNo, q, ids::BLACK_CAT) });
                    }
                }
            }
        }
        if self.in_supply(ids::FALCONER) && self.type_count(c) >= 2 {
            let n = self.num_players;
            for i in (0..n).rev() {
                let q = (self.turn.player + i) % n;
                for _ in 0..self.players[q as usize].hand.get(ids::FALCONER).min(4) {
                    self.stack.push(Frame { zone: Zone::Hand, act: Act::Play, subject: ids::FALCONER, ..Frame::new(K::YesNo, q, ids::FALCONER) });
                }
            }
        }
        if self.in_supply(ids::SLEIGH) {
            for _ in 0..self.players[pi].hand.get(ids::SLEIGH).min(4) {
                self.stack.push(Frame {
                    zone: Zone::Hand, act: Act::Discard, subject: ids::SLEIGH, then: Then::SleighMove, last: c, dest: to, tok: gt,
                    ..Frame::new(K::YesNo, p, ids::SLEIGH)
                });
            }
        }
        if self.in_supply(ids::SHEEPDOG) {
            for _ in 0..self.players[pi].hand.get(ids::SHEEPDOG).min(4) {
                self.stack.push(Frame { zone: Zone::Hand, act: Act::Play, subject: ids::SHEEPDOG, ..Frame::new(K::YesNo, p, ids::SHEEPDOG) });
            }
        }
        // The Exile mat: all the copies of the card gained may be discarded from it (asked first).
        if self.men.hook & H_EXILE != 0 && self.players[pi].men.exile.has(c) {
            self.stack.push(Frame { zone: Zone::Exile, act: Act::Discard, subject: c, then: Then::ExileDiscard, ..Frame::new(K::YesNo, p, ids::EXILE) });
        }
    }

    /// The Treasures of the set that do more than give $ when played: Supplies gains a Horse onto the deck, Stockpile Exiles itself. `n` copies were played at once.
    pub(crate) fn men_treasure<S: EventSink>(&mut self, c: CardId, n: u8, sink: &mut S) {
        let p = self.turn.player;
        match c {
            ids::SUPPLIES => self.gain_horses(p, c, n, Dest::DeckTop, sink),
            ids::STOCKPILE => {
                for _ in 0..n {
                    self.stack.push(Frame { zone: Zone::InPlay, act: Act::Exile, subject: c, then: Then::ForcedExile, tok: self.turn.led.cur, ..Frame::new(K::YesNo, p, c) });
                }
            }
            _ => {}
        }
    }

    /// The next card played this turn is `card`: each Kiln played so far may bring a copy of it first (asked before anything else of the play).
    pub(crate) fn kiln_hook(&mut self, card: CardId, p: u8) {
        let n = std::mem::take(&mut self.turn.men.kiln);
        for _ in 0..n {
            self.stack.push(Frame { optional: true, gf: GF_ANY_COST, ..gain_frame(p, ids::KILN, u8::MAX, Filter::Card(card), Dest::Discard) });
        }
    }

    /// The card `card` (played by `p` with the token `tok`) goes back to its pile if it is still in play: false when it can't (not in play, or a card with no pile).
    pub(crate) fn return_to_own_pile_from_play<S: EventSink>(&mut self, p: u8, card: CardId, tok: u8, sink: &mut S) -> bool {
        if cards::is(card, SHELTER) || cards::is(card, ZOMBIE) || !self.self_in_play(p, card, tok) {
            return false;
        }
        if !self.leave_play_self(p, card, tok) {
            return false;
        }
        self.return_to_own_pile(p, card, sink);
        true
    }

    /// `card` goes back to its pile, wherever that is: the Supply, a Reward or Traveller pile, Spoils, Madman, Mercenary, Ferryman's pile, the Horse pile.
    pub(crate) fn return_to_own_pile<S: EventSink>(&mut self, p: u8, card: CardId, sink: &mut S) {
        if cards::is_reward(card) {
            sink.event(Event::Return { player: p, card });
            self.rewards[(card - cards::FIRST_REWARD) as usize] += 1;
        } else if card >= cards::FIRST_TRAV && ((card - cards::FIRST_TRAV) as usize) < cards::NUM_TRAV {
            sink.event(Event::Return { player: p, card });
            self.trav[(card - cards::FIRST_TRAV) as usize] += 1;
        } else if card == id::SPOILS {
            sink.event(Event::Return { player: p, card });
            self.spoils += 1;
        } else if card == id::MADMAN {
            sink.event(Event::Return { player: p, card });
            self.madman += 1;
        } else if card == id::MERCENARY {
            sink.event(Event::Return { player: p, card });
            self.mercenary += 1;
        } else if card == self.ferryman && card != 0 {
            sink.event(Event::Return { player: p, card });
            self.ferryman_left += 1;
        } else {
            self.return_to_pile(p, card, sink);
        }
    }
}

// -------------------------------------------------------------------------------------------
// Ways and the play of an Action card
// -------------------------------------------------------------------------------------------

impl GameState {
    /// A card was just played in a game with a Way or a pending Kiln: its effects wait in a `PrePlay` frame, under the Kiln's gains.
    #[inline(never)]
    pub(crate) fn push_preplay(&mut self, card: CardId, p: u8, tok: u8) {
        self.stack.push(Frame { subject: card, tok, ..Frame::new(K::PrePlay, p, card) });
        if self.turn.men.kiln > 0 && p == self.turn.player {
            self.kiln_hook(card, p);
        }
    }

    /// The Ways `f.subject` can be played as.
    fn ways_for(&self, f: &Frame) -> u32 {
        if !cards::is(f.subject, ACTION) {
            return 0;
        }
        let mut m = self.men.ways;
        // The card Way of the Mouse plays can't be played as Way of the Mouse.
        if f.tok == crate::track::NIP && f.subject == self.men.mouse {
            m &= !way_bit(ids::WAY_OF_THE_MOUSE);
        }
        m
    }

    /// The `PrePlay` frame: the choice between the card's own effect and the Ways, then the effects.
    pub(crate) fn run_preplay<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        if self.men.flags & M_WAYS != 0 && self.ways_for(&f) != 0 {
            return Run::Decide(DecisionKind::Way, f.subject);
        }
        self.stack.pop();
        self.preplay_normal(&f, sink);
        Run::Continue
    }

    /// The legal choices of the Way decision: the card's own effect (`Pass`) and each Way of the game.
    pub(crate) fn way_choices(&self, f: Frame, out: &mut ChoiceBuf) {
        out.push(Choice::Pass);
        let mut m = self.ways_for(&f);
        while m != 0 {
            out.push(Choice::Card(crate::cards::FIRST_WAY + m.trailing_zeros() as CardId));
            m &= m - 1;
        }
    }

    pub(crate) fn apply_preplay<S: EventSink>(&mut self, f: Frame, choice: Choice, sink: &mut S) {
        self.stack.pop();
        match choice {
            Choice::Card(w) => self.play_as_way(w, &f, sink),
            _ => self.preplay_normal(&f, sink),
        }
    }

    /// The card's own effects, as `resolve_effects_tok` runs them without a `PrePlay` frame.
    fn preplay_normal<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let base = self.stack.len as usize;
        let outer = std::mem::replace(&mut self.turn.led.cur, f.tok);
        self.resolve_effects_inner::<S, false>(f.subject, f.player, sink);
        self.turn.led.cur = outer;
        self.stamp_frames(base, f);
    }

    /// The frames a play pushed since `base` belong to its card effect: its depth and its token.
    fn stamp_frames(&mut self, base: usize, f: &Frame) {
        for fr in &mut self.stack.frames[base..self.stack.len as usize] {
            fr.depth = f.depth;
            if fr.tok == 0 && fr.kind != K::PlayEffects {
                fr.tok = f.tok;
            }
        }
    }

    /// `p` plays the Action card `f.subject` as the Way `way`: what the Way says instead of what the card says. The play is still a play (played this turn,
    /// Champion, the tokens on the card's pile); a Duration is not scheduled for next turn.
    fn play_as_way<S: EventSink>(&mut self, way: CardId, f: &Frame, sink: &mut S) {
        let (card, p) = (f.subject, f.player);
        sink.event(Event::Way { player: p, way, card });
        let base = self.stack.len as usize;
        let outer = std::mem::replace(&mut self.turn.led.cur, f.tok);
        if way == ids::WAY_OF_THE_CHAMELEON {
            // The card's own instructions, with +Cards and +$ trading places.
            self.resolve_effects_inner::<S, true>(card, p, sink);
        } else {
            if p == self.turn.player {
                self.turn.played.add(card, 1);
                if self.adv & crate::state::G_CHAMPION != 0 {
                    let champions = self.players[p as usize].in_play.get(id::CHAMPION).saturating_sub((card == id::CHAMPION) as u8);
                    self.turn.actions += champions;
                    sink.bonus(p, id::CHAMPION, champions, 0, 0, 0);
                }
            }
            self.way_effect(way, card, p, f.tok, sink);
        }
        self.turn.led.cur = outer;
        self.stamp_frames(base, f);
    }

    /// +Actions, +Buys and +$ from a Way: only for the turn's player.
    fn way_bonus<S: EventSink>(&mut self, p: u8, way: CardId, actions: u8, buys: u8, coins: u16, sink: &mut S) {
        if p == self.turn.player {
            self.turn.actions += actions;
            self.turn.buys += buys;
            self.turn.coins += coins;
            sink.bonus(p, way, actions, buys, coins, 0);
        }
    }

    /// What the Way `way` does for `p`, who played `card` as it (`tok` names that copy of the card in play).
    #[inline(never)]
    fn way_effect<S: EventSink>(&mut self, way: CardId, card: CardId, p: u8, tok: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        match way {
            ids::WAY_OF_THE_BUTTERFLY => {
                // May return this to its pile; if it did, gain a card costing exactly $1 more.
                if !cards::is(card, SHELTER) && !cards::is(card, ZOMBIE) && self.self_in_play(p, card, tok) {
                    self.stack.push(Frame { zone: Zone::InPlay, act: Return, subject: card, then: Then::ButterflyGain, tok, ..Frame::new(K::YesNo, p, way) });
                }
            }
            ids::WAY_OF_THE_CAMEL => self.stack.push(Frame { gf: GF_EXILE | GF_ANY_COST, ..gain_frame(p, way, u8::MAX, Filter::Card(id::GOLD), Dest::Discard) }),
            ids::WAY_OF_THE_FROG => {
                self.way_bonus(p, way, 1, 0, 0, sink);
                if p == self.turn.player && tok != crate::track::NIP {
                    self.turn.men.frogs.add_or_drop(card, 1);
                }
            }
            ids::WAY_OF_THE_GOAT => self.stack.push(select(p, way, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Nothing)),
            ids::WAY_OF_THE_HORSE => {
                self.way_bonus(p, way, 1, 0, 0, sink);
                self.stack.push(draw_frame(p, way, 2));
                self.return_to_own_pile_from_play(p, card, tok, sink);
            }
            ids::WAY_OF_THE_MOLE => {
                self.way_bonus(p, way, 1, 0, 0, sink);
                self.discard_whole_hand(p, sink);
                self.stack.push(draw_frame(p, way, 3));
            }
            ids::WAY_OF_THE_MONKEY => self.way_bonus(p, way, 0, 1, 1, sink),
            ids::WAY_OF_THE_MOUSE => {
                if self.men.mouse != 0 {
                    self.stack.push(Frame { subject: self.men.mouse, zone: Zone::Supply, count: 1, ..Frame::new(K::PlayEffects, p, way) });
                }
            }
            ids::WAY_OF_THE_MULE => self.way_bonus(p, way, 1, 0, 1, sink),
            ids::WAY_OF_THE_OTTER => self.stack.push(draw_frame(p, way, 2)),
            ids::WAY_OF_THE_OWL => self.stack.push(select(p, way, Zone::Hand, Discard, Filter::Any, 0, 0, Then::DrawUpTo(6))),
            ids::WAY_OF_THE_OX => self.way_bonus(p, way, 2, 0, 0, sink),
            ids::WAY_OF_THE_PIG => {
                self.way_bonus(p, way, 1, 0, 0, sink);
                self.stack.push(draw_frame(p, way, 1));
            }
            ids::WAY_OF_THE_RAT => {
                let f = self.treasure_filter();
                self.stack.push(Frame { subject: card, ..select(p, way, Zone::Hand, Discard, f, 0, 1, Then::GainSubjectCopy) });
            }
            ids::WAY_OF_THE_SEAL => {
                self.way_bonus(p, way, 0, 0, 1, sink);
                if p == self.turn.player {
                    self.turn.men.flags |= MT_SEAL;
                }
            }
            ids::WAY_OF_THE_SHEEP => self.way_bonus(p, way, 0, 0, 2, sink),
            ids::WAY_OF_THE_SQUIRREL => self.turn.men.squirrel[pi] = self.turn.men.squirrel[pi].saturating_add(2),
            ids::WAY_OF_THE_TURTLE => {
                // Set this aside; if it was, it is played at the start of the next turn.
                let room = self.players[pi].men.aside.has(card) || self.players[pi].men.aside.iter().count() < crate::state::MEN_ASIDE_CAP;
                if room && !cards::is(card, SHELTER) && self.leave_play_self(p, card, tok) {
                    self.players[pi].men.aside.add(card, 1);
                    sink.event(Event::SetAside { player: p, card });
                    self.schedule_aside(p, way, card);
                }
            }
            ids::WAY_OF_THE_WORM => self.stack.push(Frame { gf: GF_EXILE | GF_ANY_COST, ..gain_frame(p, way, u8::MAX, Filter::Card(id::ESTATE), Dest::Discard) }),
            _ => {}
        }
    }

    /// A start-of-turn entry for `card`, set aside by `source` (Delay, Reap, Way of the Turtle): played at the start of `p`'s next turn.
    pub(crate) fn schedule_aside(&mut self, p: u8, source: CardId, card: CardId) {
        let ps = &mut self.players[p as usize];
        let i = ps.pending_durations_len as usize;
        assert!(i < crate::state::PENDING_DURATIONS_CAP, "pending duration list overflow");
        ps.pending_durations[i] = PendingDuration { card: source, times: 1, arg: card, used: 0 };
        ps.pending_durations_len += 1;
    }

    /// Way of the Chameleon, after the card's play arm ran: the $ it made at once became Cards, the Draw frames it pushed became $.
    pub(crate) fn cham_convert<S: EventSink>(&mut self, card: CardId, p: u8, coins0: u16, base: usize, sink: &mut S) {
        let own = p == self.turn.player;
        let made = if own { self.turn.coins.saturating_sub(coins0) } else { 0 };
        if made > 0 {
            self.turn.coins = coins0;
        }
        let (mut i, mut j, mut gained) = (base, base, 0u16);
        let len = self.stack.len as usize;
        while i < len {
            let fr = self.stack.frames[i];
            if fr.kind == K::Draw && fr.player == p && fr.source == card {
                gained += fr.max as u16;
            } else {
                self.stack.frames[j] = fr;
                j += 1;
            }
            i += 1;
        }
        self.stack.len = j as u8;
        if own && gained > 0 {
            self.turn.coins += gained;
            sink.bonus(p, card, 0, 0, gained, 0);
        }
        if made > 0 {
            self.stack.push(draw_frame(p, card, made.min(u8::MAX as u16) as u8));
        }
    }

    /// Cleanup, Way of the Squirrel: the other players' draws wait under the turn's player's; the turn's player's are added to their hand.
    pub(crate) fn squirrel_draws(&mut self, p: u8, draw_n: u8) -> u8 {
        let sq = self.turn.men.squirrel;
        for q in 0..self.num_players {
            if q != p && sq[q as usize] > 0 {
                self.stack.push(draw_frame(q, ids::WAY_OF_THE_SQUIRREL, sq[q as usize]));
            }
        }
        draw_n.saturating_add(sq[p as usize])
    }

    /// Whether the card gained (`f.last`, in `f.dest`) is still where Sleigh may move it.
    pub(crate) fn sleigh_possible(&self, f: &Frame) -> bool {
        (f.tok == 0 || self.tok_valid(f.tok)) && self.count_in(f.player, GameState::gain_dest_zone(f.dest), f.last) > 0
    }

    /// How many copies of `c` `p` has in a zone a gained card can be in.
    fn count_in(&self, p: u8, z: Zone, c: CardId) -> u8 {
        let ps = &self.players[p as usize];
        match z {
            Zone::Hand => ps.hand.get(c),
            Zone::Discard => ps.discard.get(c),
            Zone::DeckTop => (ps.deck_known.peek_top() == Some(c)) as u8,
            Zone::InPlay => ps.in_play.get(c),
            _ => 0,
        }
    }

    /// Sleigh's Mode frame (`f.subject` = the card gained, `f.zone` = where it is): into the hand or onto the deck, if nothing moved it.
    pub(crate) fn move_gained(&mut self, f: &Frame, dest: Dest) {
        let (p, c, z) = (f.player, f.subject, f.zone);
        if !((f.tok == 0 || self.tok_valid(f.tok)) && self.count_in(p, z, c) > 0) {
            return;
        }
        self.tok_moved(f.tok, p, z, c);
        let ps = &mut self.players[p as usize];
        let took = match z {
            Zone::Hand => ps.hand.remove(c),
            Zone::Discard => ps.discard.remove(c),
            Zone::DeckTop => ps.deck_known.pop_top().is_some(),
            _ => false,
        };
        if !took {
            return;
        }
        match dest {
            Dest::Hand => self.players[p as usize].hand.add(c, 1),
            _ => {
                self.cover_deck(p);
                self.players[p as usize].deck_known.push_top(c);
            }
        }
    }

    /// The offer of a Pursue's name: any card the player owns (the top of the deck can hold any of them) and any card of the Supply (naming one the deck has none of
    /// discards all four).
    pub(crate) fn pursue_offer(&self, p: u8, out: &mut ChoiceBuf) {
        let mut offer = self.players[p as usize].all_cards();
        for c in self.supply_cards() {
            if offer.get(c) == 0 {
                offer.add(c, 1);
            }
        }
        for (c, _) in offer.iter() {
            out.push(Choice::Card(c));
        }
    }

    /// Pursue's 4 cards are revealed: the ones named go back on the deck in any order, the rest are discarded.
    pub(crate) fn finish_pursue<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let named = f.subject;
        let ps = &mut self.players[p as usize];
        let mut rest = Counts::EMPTY;
        for (c, n) in ps.set_aside.iter() {
            if c != named {
                rest.add(c, n);
            }
        }
        for (c, n) in rest.iter() {
            ps.set_aside.set(c, 0);
            for _ in 0..n {
                sink.event(Event::Discard { player: p, card: c });
            }
        }
        ps.discard.add_all(&rest);
        let keep = ps.set_aside.total();
        if keep > 0 {
            self.stack.push(Frame { depth: f.depth, ordered: true, ..select(p, f.source, Zone::Revealed, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
        }
        if self.adv & crate::state::G_DISCARD_HOOK != 0 {
            self.discard_hook_counts(p, &rest, f.depth, sink);
        }
    }

    // ---- continuations of the generic frames ----

    /// The `then` of a Menagerie `YesNo` answered Yes (after the card was moved by the frame's `act`).
    #[inline(never)]
    pub(crate) fn men_yes<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            // The card gained was discarded from the mat; so are the rest of its copies.
            Then::ExileDiscard => {
                let n = self.players[p as usize].men.exile.get(f.subject);
                for _ in 0..n {
                    if self.exile_remove(p, f.subject) {
                        self.put(p, f.subject, Act::Discard, Dest::Discard, f.source, sink);
                    }
                }
                self.men_update_hook();
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.flush_trash_triggers(f.depth, sink);
                }
            }
            // Sleigh was discarded: the card gained goes into the hand or onto the deck.
            Then::SleighMove => {
                self.stack.push(Frame { subject: f.last, zone: GameState::gain_dest_zone(f.dest), tok: f.tok, depth: f.depth, ..mode_frame(p, ids::SLEIGH, 1) });
            }
            // The card went back to its pile: a card costing exactly $1 more.
            Then::ButterflyGain => {
                let cost = self.cost(f.subject);
                self.stack.push(Frame { exact: true, potion: cards::potion_cost(f.subject), depth: f.depth, ..gain_frame(p, f.source, cost.saturating_add(1), Filter::Any, Dest::Discard) });
            }
            _ => {}
        }
    }

    /// What a Menagerie `Gain` frame does once the card `c` has been gained (or Exiled: Invest, Populate's piles).
    #[inline(never)]
    pub(crate) fn men_gained<S: EventSink>(&mut self, f: &Frame, c: CardId, dest: Dest, sink: &mut S) {
        let p = f.player;
        match f.then {
            // Groom: an Action, a Horse; a Treasure, a Silver; a Victory card, +1 Card and +1 Action.
            Then::GroomBonus => {
                if cards::is(c, ACTION) {
                    self.gain_horses(p, f.source, 1, Dest::Discard, sink);
                }
                if self.is_treasure(c) {
                    self.gain(p, id::SILVER, Dest::Discard, false, f.source, sink);
                }
                if cards::is(c, VICTORY) {
                    if p == self.turn.player {
                        self.turn.actions += 1;
                        sink.bonus(p, f.source, 1, 0, 0, 0);
                    }
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 1) });
                }
            }
            Then::DesperationBonus => {
                if p == self.turn.player {
                    self.turn.buys += 1;
                    self.turn.coins += 2;
                    sink.bonus(p, f.source, 0, 1, 2, 0);
                }
            }
            Then::PopulateGain => {
                self.turn.men.pop.add_or_drop(cards::pile_of(c), 1);
            }
            // Reap's Gold is set aside at once (the reactions the gain brings find it gone) and played at the start of the next turn.
            Then::ReapAside => {
                let gt = self.turn.led.gain;
                self.tok_moved(gt, p, GameState::gain_dest_zone(dest), c);
                let ps = &mut self.players[p as usize];
                let room = ps.men.aside.has(c) || ps.men.aside.iter().count() < crate::state::MEN_ASIDE_CAP;
                let moved = room
                    && match dest {
                        Dest::Discard => ps.discard.remove(c),
                        Dest::Hand => ps.hand.remove(c),
                        Dest::DeckTop => ps.deck_known.peek_top() == Some(c) && ps.deck_known.pop_top().is_some(),
                    };
                if moved {
                    ps.men.aside.add(c, 1);
                    sink.event(Event::SetAside { player: p, card: c });
                    self.schedule_aside(p, f.source, c);
                }
            }
            _ => {}
        }
    }

    /// The `then` of a Menagerie `Select` that has finished.
    #[inline(never)]
    pub(crate) fn finish_men_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            // Scrap: one different thing per $1 the trashed card cost.
            Then::ModePerCost => {
                if f.count > 0 {
                    // (Allies: a card Elder played gives one more pick.)
                    let picks = (self.cost(f.last) as usize + (f.gf & crate::state::GF_ELDER != 0) as usize).min(cards::modes(f.source).len()) as u8;
                    if picks > 0 {
                        self.stack.push(mode_frame(p, f.source, picks));
                    }
                }
            }
            // Displace: a differently named card costing up to $2 more than the one Exiled.
            Then::DisplaceGain => {
                if f.count > 0 {
                    self.stack.push(Frame {
                        potion: cards::potion_cost(f.last),
                        ..gain_frame(p, f.source, self.cost(f.last).saturating_add(2), Filter::NotCard(f.last), Dest::Discard)
                    });
                }
            }
            // Delay: the Action is set aside and played at the start of the next turn.
            Then::DelayAside => {
                if f.count > 0 {
                    let ps = &mut self.players[p as usize];
                    let room = ps.men.aside.has(f.last) || ps.men.aside.iter().count() < crate::state::MEN_ASIDE_CAP;
                    if room && ps.hand.remove(f.last) {
                        ps.men.aside.add(f.last, 1);
                        sink.event(Event::SetAside { player: p, card: f.last });
                        self.schedule_aside(p, f.source, f.last);
                    }
                }
            }
            // Banish: the name is chosen; any number of the copies in the hand are Exiled.
            Then::BanishPick => {
                if f.count > 0 {
                    self.stack.push(select(p, f.source, Zone::Hand, Act::Exile, Filter::Card(f.last), 1, ALL, Then::Nothing));
                }
            }
            // Bounty Hunter: +$3 if the card Exiled is the only copy on the mat.
            Then::BountyHunter => {
                if f.count > 0 && self.players[p as usize].men.exile.get(f.last) == 1 {
                    self.turn.coins += 3;
                    sink.bonus(p, f.source, 0, 0, 3, 0);
                }
            }
            // Way of the Rat: a copy of the card played.
            Then::GainSubjectCopy => {
                if f.count > 0 {
                    self.stack.push(Frame { gf: GF_ANY_COST, ..gain_frame(p, f.source, u8::MAX, Filter::Card(f.subject), Dest::Discard) });
                }
            }
            // Animal Fair, bought by trashing an Action: the trash comes first, then the card is gained as a bought card.
            Then::AltBuyGain => {
                if f.count > 0 {
                    self.gain(p, f.subject, Dest::Discard, true, NO_SOURCE, sink);
                    self.on_buy_effects(p, f.subject, sink);
                }
            }
            _ => {}
        }
    }

    /// A `K::Ask` of the set answered Yes: Village Green and Barge take their bonus now, Hunting Lodge discards the hand for 5 Cards.
    #[inline(never)]
    pub(crate) fn men_ask_yes<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::NowOrNext => {
                let (cards_n, actions, buys, coins) = cards::duration_bonus(f.source);
                self.way_bonus(p, f.source, actions, buys, coins as u16, sink);
                if cards_n > 0 {
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, cards_n) });
                }
                // Played out of turn (a Village Green when discarded), it is discarded at the end of that turn.
                if p != self.turn.player {
                    self.players[p as usize].discard_next_cleanup.add(f.source, 1);
                }
            }
            Then::HuntingLodge => {
                self.discard_whole_hand(p, sink);
                self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 5) });
            }
            _ => {}
        }
    }

    /// A `K::Ask` of the set answered No: Village Green and Barge are saved for the start of the next turn.
    pub(crate) fn men_ask_no(&mut self, f: Frame) {
        if f.then == Then::NowOrNext {
            self.push_duration_pending(f.player, f.source, 1, 0);
        }
    }

    // ---- buying with an Action card (Animal Fair) ----

    /// Whether `card` could be bought by trashing an Action card from the hand: a Buy, a pile with a card, an Action to trash.
    pub(crate) fn alt_buy_possible(&self, card: CardId) -> bool {
        let p = self.turn.player as usize;
        !self.turn.adv.no_buy
            && self.players[p].debt == 0
            && self.can_buy()
            && self.supply.get(card) > 0
            && self.pile_open(card)
            && self.may_buy(card)
            && self.players[p].hand.iter().any(|(c, _)| self.is_action(c))
    }

    /// Buy `card` for a Buy and an Action card trashed from the hand instead of its cost.
    #[inline(never)]
    pub(crate) fn buy_alt<S: EventSink>(&mut self, card: CardId, p: u8, sink: &mut S) {
        self.spend_buy();
        self.turn.treasures_done = true;
        sink.depth(0);
        sink.event(Event::Buy { player: p, card });
        sink.depth(1);
        self.stack.push(Frame { depth: 1, subject: card, ..select(p, card, Zone::Hand, Act::Trash, Filter::Action, 1, 1, Then::AltBuyGain) });
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
        let free = [
            ids::BLACK_CAT, ids::SUPPLIES, ids::SHEEPDOG, ids::STOCKPILE, ids::CARDINAL, ids::HOSTELRY, ids::COVEN, ids::GATEKEEPER, ids::LIVERY, ids::MASTERMIND,
            ids::FISHERMAN, ids::DESTRIER, ids::ANIMAL_FAIR, ids::HORSE,
        ];
        let order = [ids::GOATHERD, ids::SCRAP, ids::BOUNTY_HUNTER, ids::DISPLACE, ids::HUNTING_LODGE, ids::SANCTUARY, ids::KILN];
        let choice = [
            ids::SLEIGH, ids::CAMEL_TRAIN, ids::SNOWY_VILLAGE, ids::CAVALRY, ids::GROOM, ids::VILLAGE_GREEN, ids::BARGE, ids::FALCONER, ids::PADDOCK,
            ids::WAYFARER,
        ];
        for (cid, def) in CARDS {
            let acts = def.types & (ACTION | TREASURE) != 0;
            if !acts {
                assert_eq!(def.on_play, cards::OnPlay::NotAction, "{}", def.name);
                continue;
            }
            let want = if free.contains(cid) {
                cards::OnPlay::ChoiceFree
            } else if order.contains(cid) {
                cards::OnPlay::OrderSensitive
            } else if choice.contains(cid) {
                cards::OnPlay::Choice
            } else {
                panic!("{} is not in any list", def.name)
            };
            assert_eq!(def.on_play, want, "{}", def.name);
        }
        assert_eq!(CARDS.len(), 31 + 20 + 20 + 1);
    }
}
