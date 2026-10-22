//! Renaissance (25 kingdom cards, 5 Artifacts and 20 Projects). See `docs/renaissance-plan.md` for the design and the rulings.
//!
//! * **Villagers** are a counter per player (`PlayerState::villagers`); one is spent for +1 Action in the Action phase when no Action is left (`Choice::Villager`).
//! * **Artifacts** are pseudo-cards (ids above every zone id) held by at most one player each (`RenState::holder`); the effects are looked up where they
//!   apply (the cleanup draw, the start of the turn, the start of the Buy phase, Border Guard).
//! * **Projects** are pseudo-cards like the Events, bought in the Buy phase for their printed cost and a Buy, then owned (`PlayerState::projects`, at most
//!   two per player); the kingdom lists the Projects of the game (`RenState::projects`).

use super::prelude::*;
use crate::cards::{ren, DURATION};
use crate::state::{GameConfig, PendingDuration, REN_CAPITAL, REN_GAIN, REN_STAR, REN_TRASH};

/// Card ids of this set. The kingdom cards are the highest kingdom ids (after Rice, before the Ruins and Knights); the Projects follow the Landmarks and
/// the Artifacts are the highest ids of all (after the Prophecies).
pub mod ids {
    use crate::cards::CardId;
    pub const BORDER_GUARD: CardId = 382;
    pub const DUCAT: CardId = 383;
    pub const LACKEYS: CardId = 384;
    pub const ACTING_TROUPE: CardId = 385;
    pub const CARGO_SHIP: CardId = 386;
    pub const EXPERIMENT: CardId = 387;
    pub const IMPROVE: CardId = 388;
    pub const FLAG_BEARER: CardId = 389;
    pub const HIDEOUT: CardId = 390;
    pub const INVENTOR: CardId = 391;
    pub const MOUNTAIN_VILLAGE: CardId = 392;
    pub const PATRON: CardId = 393;
    pub const PRIEST: CardId = 394;
    pub const RESEARCH: CardId = 395;
    pub const SILK_MERCHANT: CardId = 396;
    pub const OLD_WITCH: CardId = 397;
    pub const RECRUITER: CardId = 398;
    pub const SCEPTER: CardId = 399;
    pub const SCHOLAR: CardId = 400;
    pub const SCULPTOR: CardId = 401;
    pub const SEER: CardId = 402;
    pub const SPICES: CardId = 403;
    pub const SWASHBUCKLER: CardId = 404;
    pub const TREASURER: CardId = 405;
    pub const VILLAIN: CardId = 406;
    // The Projects (after the Landmarks).
    pub const CATHEDRAL: CardId = 702;
    pub const CITY_GATE: CardId = 703;
    pub const PAGEANT: CardId = 704;
    pub const SEWERS: CardId = 705;
    pub const STAR_CHART: CardId = 706;
    pub const EXPLORATION: CardId = 707;
    pub const FAIR: CardId = 708;
    pub const SILOS: CardId = 709;
    pub const SINISTER_PLOT: CardId = 710;
    pub const ACADEMY: CardId = 711;
    pub const CAPITALISM: CardId = 712;
    pub const FLEET: CardId = 713;
    pub const GUILDHALL: CardId = 714;
    pub const PIAZZA: CardId = 715;
    pub const ROAD_NETWORK: CardId = 716;
    pub const BARRACKS: CardId = 717;
    pub const CROP_ROTATION: CardId = 718;
    pub const INNOVATION: CardId = 719;
    pub const CANAL: CardId = 720;
    pub const CITADEL: CardId = 721;
    // The Artifacts, the highest ids.
    pub const FLAG: CardId = 722;
    pub const HORN: CardId = 723;
    pub const KEY: CardId = 724;
    pub const LANTERN: CardId = 725;
    pub const TREASURE_CHEST: CardId = 726;
}

/// A Project: no types, no bonuses (what it does is looked up where it applies); its cost is the cost to buy it.
const fn project(name: &'static str, cost: u8) -> CardDef {
    ren(c(name, cost, 0, 0, 0, 0, 0, 0))
}

/// An Artifact: nothing to buy.
const fn artifact(name: &'static str) -> CardDef {
    ren(c(name, 0, 0, 0, 0, 0, 0, 0))
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::BORDER_GUARD, ren(order_sensitive(c("Border Guard", 2, ACTION, 0, 0, 0, 1, 0)))), // reveals the top 2 or 3 cards; one is kept
    (ids::DUCAT, ren(choice_free(c("Ducat", 2, TREASURE, 0, 0, 0, 0, 1)))), // +1 Coffers; may trash a Copper when gained
    (ids::LACKEYS, ren(choice_free(c("Lackeys", 2, ACTION, 0, 0, 2, 0, 0)))), // +2 Villagers when gained
    (ids::ACTING_TROUPE, ren(choice_free(c("Acting Troupe", 3, ACTION, 0, 0, 0, 0, 0)))), // +4 Villagers, trashes itself
    (ids::CARGO_SHIP, ren(choice_free(c("Cargo Ship", 3, ACTION | DURATION, 2, 0, 0, 0, 0)))), // sets aside a card gained this turn
    (ids::EXPERIMENT, ren(choice_free(c("Experiment", 3, ACTION, 0, 0, 2, 1, 0)))), // returns itself to its pile; comes with another
    (ids::IMPROVE, ren(choice_free(c("Improve", 3, ACTION, 2, 0, 0, 0, 0)))), // may trash an Action at Clean-up for one costing $1 more
    (ids::FLAG_BEARER, ren(on_trash(choice_free(c("Flag Bearer", 4, ACTION, 2, 0, 0, 0, 0))))), // takes the Flag when gained or trashed
    (ids::HIDEOUT, ren(order_sensitive(c("Hideout", 4, ACTION, 0, 0, 1, 2, 0)))), // trashes any card of the hand
    (ids::INVENTOR, ren(has_choice(c("Inventor", 4, ACTION, 0, 0, 0, 0, 0)))), // gains a card up to $4, cards cost $1 less
    (ids::MOUNTAIN_VILLAGE, ren(order_sensitive(c("Mountain Village", 4, ACTION, 0, 0, 0, 2, 0)))), // takes a card from the discard pile
    (ids::PATRON, ren(choice_free(c("Patron", 4, ACTION | REACTION, 2, 0, 0, 0, 0)))), // +1 Villager; +1 Coffers when revealed
    (ids::PRIEST, ren(order_sensitive(c("Priest", 4, ACTION, 2, 0, 0, 0, 0)))), // trashes any card of the hand
    (ids::RESEARCH, ren(order_sensitive(c("Research", 4, ACTION | DURATION, 0, 0, 0, 1, 0)))), // trashes any card of the hand, sets cards aside
    (ids::SILK_MERCHANT, ren(on_trash(choice_free(c("Silk Merchant", 4, ACTION, 0, 0, 2, 0, 1))))), // Coffers and a Villager when gained or trashed
    (ids::OLD_WITCH, ren(choice_free(c("Old Witch", 5, ACTION | ATTACK, 0, 0, 3, 0, 0)))), // each other player gains a Curse and may trash one
    (ids::RECRUITER, ren(order_sensitive(c("Recruiter", 5, ACTION, 0, 0, 2, 0, 0)))), // trashes any card of the hand
    (ids::SCEPTER, ren(has_choice(c("Scepter", 5, TREASURE | COMMAND, 0, 0, 0, 0, 0)))), // +$2 or replays an Action in play
    (ids::SCHOLAR, ren(order_sensitive(c("Scholar", 5, ACTION, 0, 0, 0, 0, 0)))), // discards the hand, +7 Cards
    (ids::SCULPTOR, ren(has_choice(c("Sculptor", 5, ACTION, 0, 0, 0, 0, 0)))), // gains a card to the hand
    (ids::SEER, ren(order_sensitive(c("Seer", 5, ACTION, 0, 0, 1, 1, 0)))), // reveals the top 3 cards and puts the rest back
    (ids::SPICES, ren(choice_free(c("Spices", 5, TREASURE, 2, 0, 0, 0, 1)))), // +2 Coffers when gained
    (ids::SWASHBUCKLER, ren(order_sensitive(c("Swashbuckler", 5, ACTION, 0, 0, 3, 0, 0)))), // needs a card in the discard pile
    (ids::TREASURER, ren(has_choice(c("Treasurer", 5, ACTION, 3, 0, 0, 0, 0)))), // trashes a Treasure, regains one, or takes the Key
    (ids::VILLAIN, ren(choice_free(c("Villain", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // +2 Coffers, a discard attack
    // The Projects.
    (ids::CATHEDRAL, project("Cathedral", 3)),
    (ids::CITY_GATE, project("City Gate", 3)),
    (ids::PAGEANT, project("Pageant", 3)),
    (ids::SEWERS, project("Sewers", 3)),
    (ids::STAR_CHART, project("Star Chart", 3)),
    (ids::EXPLORATION, project("Exploration", 4)),
    (ids::FAIR, project("Fair", 4)),
    (ids::SILOS, project("Silos", 4)),
    (ids::SINISTER_PLOT, project("Sinister Plot", 4)),
    (ids::ACADEMY, project("Academy", 5)),
    (ids::CAPITALISM, project("Capitalism", 5)),
    (ids::FLEET, project("Fleet", 5)),
    (ids::GUILDHALL, project("Guildhall", 5)),
    (ids::PIAZZA, project("Piazza", 5)),
    (ids::ROAD_NETWORK, project("Road Network", 5)),
    (ids::BARRACKS, project("Barracks", 6)),
    (ids::CROP_ROTATION, project("Crop Rotation", 6)),
    (ids::INNOVATION, project("Innovation", 6)),
    (ids::CANAL, project("Canal", 7)),
    (ids::CITADEL, project("Citadel", 8)),
    // The Artifacts.
    (ids::FLAG, artifact("Flag")),
    (ids::HORN, artifact("Horn")),
    (ids::KEY, artifact("Key")),
    (ids::LANTERN, artifact("Lantern")),
    (ids::TREASURE_CHEST, artifact("Treasure Chest")),
];

/// The bit of a Project in `PlayerState::projects` / `RenState::projects`.
#[inline(always)]
pub const fn project_bit(p: CardId) -> u32 {
    1 << (p - crate::cards::FIRST_PROJECT)
}

/// The Projects that act at the start of the owner's turn (as entries of the start-of-turn list, like the Durations).
const START_PROJECTS: [CardId; 9] =
    [ids::CATHEDRAL, ids::CITY_GATE, ids::SILOS, ids::SINISTER_PLOT, ids::CROP_ROTATION, ids::PIAZZA, ids::FAIR, ids::BARRACKS, ids::KEY];

/// The Actions with a "+$" amount in their text, which are also Treasures on their owner's turns while the owner has Capitalism (the wiki's list, by name;
/// the cards that are not implemented are skipped).
const CAPITALISM_NAMES: &[&str] = &[
    "Festival", "Market", "Merchant", "Militia", "Moneylender", "Poacher", "Vassal", "Baron", "Bridge", "Conspirator", "Courtier", "Ironworks", "Mill",
    "Mining Village", "Minion", "Pawn", "Steward", "Swindler", "Bazaar", "Corsair", "Cutpurse", "Fishing Village", "Lighthouse", "Merchant Ship",
    "Sailor", "Salvager", "Treasury", "Herbalist", "Bishop", "City", "Charlatan", "Clerk", "Grand Market", "Monument", "Peddler", "Vault", "Courser",
    "Jester", "Joust", "Merchant Guild", "Shop", "Haggler", "Nomads", "Oasis", "Souk", "Spice Merchant", "Abandoned Mine", "Count", "Dame Sylvia",
    "Death Cart", "Forager", "Ironmonger", "Junk Dealer", "Mercenary", "Mystic", "Poor House", "Rogue", "Scavenger", "Storeroom", "Squire", "Amulet",
    "Artificer", "Caravan Guard", "Giant", "Hero", "Messenger", "Miser", "Peasant", "Soldier", "Swamp Hag", "Teacher", "Treasure Hunter",
    "Wine Merchant", "Catapult", "Chariot Race", "Emporium", "Farmers' Market", "Gladiator", "Legionary", "Opulent Castle", "Sacrifice", "Villa", "Bard",
    "Conclave", "Sacred Grove", "Secret Cave", "Tormentor", "Tracker", "Cargo Ship", "Flag Bearer", "Improve", "Patron", "Priest", "Treasurer",
    "Aristocrat", "Change", "Fishmonger", "Kiln", "Kitsune", "Mountain Shrine", "Samurai", "Tea House", "Animal Fair", "Bounty Hunter", "Cardinal", "Coven",
    "Fisherman", "Gatekeeper", "Livery", "Paddock", "Scrap",
];

/// Whether `c` is an Action with a "+$" amount in its text (the cards Capitalism turns into Treasures): a lookup table built once.
pub fn capitalism_card(c: CardId) -> bool {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[bool; crate::cards::NUM_ZONE_IDS]> = OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [false; crate::cards::NUM_ZONE_IDS];
        for name in CAPITALISM_NAMES {
            if let Some(id) = cards::by_name(name) {
                if (id as usize) < t.len() && cards::is(id, ACTION) {
                    t[id as usize] = true;
                }
            }
        }
        t
    });
    (c as usize) < t.len() && t[c as usize]
}

/// The Renaissance cards (see the module docs of `sets`).
pub struct Renaissance;

impl SetRules for Renaissance {
    /// Seer alone: +1 Card +1 Action, and the cards costing $2-$4 it reveals go to the hand; never worse than not playing it.
    fn safe_alone(card: CardId) -> bool {
        card == ids::SEER
    }

    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Ducat (id 383), Lackeys, Experiment, Flag Bearer, Silk Merchant and Spices have a "when you gain" effect.
    const GAIN_TOP: u128 = 1u128 << (ids::DUCAT - 256);
    const GAIN_EXT: u128 = (1u128 << (ids::LACKEYS - 384))
        | (1u128 << (ids::EXPERIMENT - 384))
        | (1u128 << (ids::FLAG_BEARER - 384))
        | (1u128 << (ids::SILK_MERCHANT - 384))
        | (1u128 << (ids::SPICES - 384));

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            // Border Guard (all Actions revealed): the Lantern or the Horn.
            ids::BORDER_GUARD => &[ModeOpt::TakeArtifact(ids::LANTERN), ModeOpt::TakeArtifact(ids::HORN)],
            ids::SCEPTER => &[ModeOpt::Coins(2), ModeOpt::ReplayAction],
            ids::TREASURER => &[ModeOpt::TrashFiltered(Filter::Treasure), ModeOpt::GainFromTrashTo(Filter::Treasure, Dest::Hand), ModeOpt::TakeArtifact(ids::KEY)],
            _ => &[],
        }
    }

    /// The bonus of the start-of-turn effects that are plain: Fair's Buy, Barracks's Action and the Key's $1.
    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            ids::FAIR => (0, 0, 1, 0),
            ids::BARRACKS => (0, 1, 0, 0),
            ids::KEY => (0, 0, 0, 1),
            _ => (0, 0, 0, 0),
        }
    }

    /// The start-of-turn effects that act on the hand or the deck, or put cards into the hand: their order can matter.
    fn start_order_sensitive(card: CardId) -> bool {
        matches!(
            card,
            ids::CATHEDRAL | ids::CITY_GATE | ids::SILOS | ids::SINISTER_PLOT | ids::CROP_ROTATION | ids::PIAZZA | ids::CARGO_SHIP | ids::RESEARCH
        )
    }

    /// Cargo Ship and Research stay in play only when they set something aside, so they schedule themselves.
    fn schedules_own_duration(card: CardId) -> bool {
        matches!(card, ids::CARGO_SHIP | ids::RESEARCH)
    }

    /// The Projects (and the Key) that act at the start of every turn of their owner.
    fn turn_start_entries(g: &mut GameState) {
        let p = g.turn.player;
        let owned = g.players[p as usize].ren.projects;
        if owned != 0 {
            for &pr in &START_PROJECTS[..8] {
                if owned & project_bit(pr) != 0 {
                    g.turn.start_pending.push(PendingDuration { card: pr, times: 1, arg: 0, used: 0 });
                }
            }
        }
        if g.ren.holder[(ids::KEY - crate::cards::FIRST_ARTIFACT) as usize] == p + 1 {
            g.turn.start_pending.push(PendingDuration { card: ids::KEY, times: 1, arg: 0, used: 0 });
        }
    }

    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
        use Act::*;
        let card = e.card;
        if !crate::cards::is_project(card) {
            return false;
        }
        sink.depth(0);
        sink.event(Event::Bonus { player: p, source: card, actions: 0, buys: 0, coins: 0, vp: 0 });
        match card {
            ids::CATHEDRAL => g.stack.push(Frame { depth: 1, ..select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Nothing) }),
            ids::CITY_GATE => {
                g.stack.push(Frame { depth: 1, ..select(p, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing) });
                g.stack.push(Frame { depth: 1, ..draw_frame(p, card, 1) });
            }
            ids::SILOS => g.stack.push(Frame { depth: 1, ..select(p, card, Zone::Hand, Discard, Filter::Card(id::COPPER), 0, ALL, Then::DrawPerPick) }),
            ids::SINISTER_PLOT => {
                if g.ren.plot[p as usize] == 0 {
                    g.add_plot_token(p, sink);
                } else {
                    g.stack.push(Frame { depth: 1, then: Then::SinisterPlot, ..Frame::new(K::Ask, p, card) });
                }
            }
            ids::CROP_ROTATION => g.stack.push(Frame { depth: 1, ..select(p, card, Zone::Hand, Discard, Filter::Victory, 0, 1, Then::DrawNPerPick(2)) }),
            ids::PIAZZA => g.stack.push(Frame { depth: 1, max: 1, then: Then::PiazzaPlay, ..Frame::new(K::RevealTop, p, card) }),
            _ => return false,
        }
        true
    }

    /// Cargo Ship's card comes into the hand, and so do all the cards under the Researches.
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
        let ps = &mut g.players[p as usize];
        match e.card {
            ids::CARGO_SHIP => {
                if ps.ren.cargo.remove(e.arg) {
                    ps.hand.add(e.arg, 1);
                    sink.event(Event::Draw { player: p, card: e.arg });
                }
            }
            ids::RESEARCH => {
                let aside = ps.ren.research.take_all();
                for (c, n) in aside.iter() {
                    ps.hand.add(c, n);
                    for _ in 0..n {
                        sink.event(Event::Draw { player: p, card: c });
                    }
                }
            }
            _ => {}
        }
    }

    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        match card {
            ids::BORDER_GUARD => {
                // Reveal the top 2 cards, or the top 3 with the Lantern.
                let n = if g.has_artifact(p, ids::LANTERN) { 3 } else { 2 };
                g.stack.push(Frame { max: n, then: Then::BorderGuardReveal, ..Frame::new(K::RevealTop, p, card) });
            }
            ids::DUCAT => g.give_coffers(p, card, 1, sink),
            ids::ACTING_TROUPE => {
                g.give_villagers(p, card, 4, sink);
                g.stack.push(select(p, card, Zone::InPlay, Trash, Filter::Card(card), 1, 1, Then::Nothing));
            }
            ids::CARGO_SHIP => {
                if p == g.turn.player {
                    g.turn.ren.cargo = g.turn.ren.cargo.saturating_add(1);
                }
            }
            // Back to its pile (when it is in play; not a copy played from the Supply).
            ids::EXPERIMENT => {
                if g.leave_play_self_now(p, card) {
                    g.return_to_pile(p, card, sink);
                }
            }
            ids::HIDEOUT => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::HideoutCurse)),
            // A card costing up to $4, then cards cost $1 less (the reduction waits under the gain and its reactions).
            ids::INVENTOR => {
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::CostLess));
                g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard));
            }
            ids::MOUNTAIN_VILLAGE => {
                if g.players[pi].discard.is_empty() {
                    g.stack.push(draw_frame(p, card, 1));
                } else {
                    g.stack.push(select(p, card, Zone::Discard, ToHand, Filter::Any, 1, 1, Then::Nothing));
                }
            }
            ids::PATRON => g.give_villagers(p, card, 1, sink),
            ids::PRIEST => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::PriestSetup)),
            ids::RESEARCH => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::ResearchTrash)),
            ids::OLD_WITCH => {
                // Each other player gains a Curse and may trash a Curse from their hand (victim by victim).
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(select(v, card, Zone::Hand, Trash, Filter::Card(id::CURSE), 0, 1, Then::Nothing));
                    g.push_gain_to_each(&[v], id::CURSE, card);
                }
            }
            ids::RECRUITER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::RecruiterVillagers)),
            ids::SCEPTER => g.stack.push(mode_frame(p, card, 1)),
            ids::SCHOLAR => {
                g.discard_whole_hand(p, sink);
                g.stack.push(draw_frame(p, card, 7));
            }
            ids::SCULPTOR => g.stack.push(Frame { then: Then::VillagerIfTreasure, ..gain_frame(p, card, 4, Filter::Any, Dest::Hand) }),
            ids::SEER => {
                // Reveal the top 3: the cards costing $2 to $4 go to the hand, the rest back in any order.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(Frame { max: 3, then: Then::MoveMatchingToHand(Filter::CostRange(2, 4)), ..Frame::new(K::RevealTop, p, card) });
            }
            // After the +3 Cards (pushed above this by the generic tail of a play).
            ids::SWASHBUCKLER => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::Swashbuckler)),
            ids::TREASURER => g.stack.push(mode_frame(p, card, 1)),
            ids::VILLAIN => {
                g.give_coffers(p, card, 2, sink);
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    let hand = g.players[v as usize].hand;
                    if hand.total() >= 5 {
                        // A player with no card costing $2 or more reveals their hand instead.
                        if !hand.iter().any(|(c, _)| g.cost(c) >= 2) {
                            for (c, k) in hand.iter() {
                                for _ in 0..k {
                                    g.reveal_card(v, c, sink);
                                }
                            }
                        }
                        g.stack.push(select(v, card, Zone::Hand, Discard, Filter::CostAtLeast(2), 1, 1, Then::Nothing));
                    }
                }
            }
            _ => {}
        }
    }

    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, _on_buy: bool, sink: &mut S) {
        // The card's own "when you gain this".
        match c {
            ids::DUCAT => g.stack.push(Frame { zone: Zone::Hand, act: Act::Trash, subject: id::COPPER, ..Frame::new(K::YesNo, p, c) }),
            ids::LACKEYS => g.give_villagers(p, c, 2, sink),
            // Another Experiment (that does not come with another), to the discard pile.
            ids::EXPERIMENT if !g.turn.ren.no_chain => {
                g.turn.ren.no_chain = true;
                g.gain(p, c, Dest::Discard, false, c, sink);
                g.turn.ren.no_chain = false;
            }
            ids::FLAG_BEARER => g.take_artifact(p, ids::FLAG, sink),
            ids::SILK_MERCHANT => {
                g.give_coffers(p, c, 1, sink);
                g.give_villagers(p, c, 1, sink);
            }
            ids::SPICES => g.give_coffers(p, c, 2, sink),
            _ => {}
        }
        let zone = GameState::gain_dest_zone(to);
        let own_turn = p == g.turn.player;
        // Cargo Ship: the card gained may be set aside (once for each Cargo Ship played this turn).
        if own_turn && g.turn.ren.cargo > 0 {
            g.stack.push(Frame { zone, act: Act::SetAside, subject: c, then: Then::CargoAside, ..Frame::new(K::YesNo, p, ids::CARGO_SHIP) });
        }
        if g.ren.hook & REN_GAIN != 0 {
            let owned = g.players[p as usize].ren.projects;
            if owned & project_bit(ids::ACADEMY) != 0 && g.is_action(c) {
                g.give_villagers(p, ids::ACADEMY, 1, sink);
            }
            if owned & project_bit(ids::GUILDHALL) != 0 && g.is_treasure(c) {
                g.give_coffers(p, ids::GUILDHALL, 1, sink);
            }
            if owned & project_bit(ids::INNOVATION) != 0 && own_turn && !g.turn.ren.innovation && g.is_action(c) {
                g.stack.push(Frame { zone, act: Act::Play, subject: c, then: Then::InnovationPlay, ..Frame::new(K::YesNo, p, ids::INNOVATION) });
            }
            // Road Network: another player's Victory card gained is +1 Card for each of its owners.
            if cards::is(c, VICTORY) {
                let mut qs = [0u8; MAX_PLAYERS];
                let mut n = 0;
                for q in g.others(p) {
                    if g.players[q as usize].ren.projects & project_bit(ids::ROAD_NETWORK) != 0 {
                        qs[n] = q;
                        n += 1;
                    }
                }
                for &q in qs[..n].iter().rev() {
                    g.stack.push(draw_frame(q, ids::ROAD_NETWORK, 1));
                }
            }
        }
    }

    /// The Projects and Cargo Ship look at every gain once they are in play.
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.ren.hook & REN_GAIN != 0 || g.turn.ren.cargo > 0
    }

    /// Flag Bearer gives the Flag and Silk Merchant Coffers and a Villager to whoever trashes them.
    fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, sink: &mut S) {
        match card {
            ids::FLAG_BEARER => g.take_artifact(p, ids::FLAG, sink),
            ids::SILK_MERCHANT => {
                g.give_coffers(p, card, 1, sink);
                g.give_villagers(p, card, 1, sink);
            }
            _ => {}
        }
    }

    /// The end of the Buy phase: Exploration's Coffers and Villager (no card gained in the phase), Pageant's $1 for a Coffers.
    fn end_of_buy<S: EventSink>(g: &mut GameState, sink: &mut S) {
        let p = g.turn.player;
        let owned = g.players[p as usize].ren.projects;
        if owned == 0 {
            return;
        }
        if owned & project_bit(ids::EXPLORATION) != 0 && g.turn.gained_in_buy == 0 {
            g.give_coffers(p, ids::EXPLORATION, 1, sink);
            g.give_villagers(p, ids::EXPLORATION, 1, sink);
        }
        if owned & project_bit(ids::PAGEANT) != 0 && g.turn.coins >= 1 {
            g.stack.push(Frame { then: Then::PageantPay, ..Frame::new(K::Ask, p, ids::PAGEANT) });
        }
    }

    /// Improve: for each one played this turn, an Action card that would be discarded from play may be trashed for a card costing exactly $1 more.
    fn offers_at_cleanup(g: &mut GameState) {
        if g.in_supply(ids::IMPROVE) {
            let p = g.turn.player;
            let n = g.turn.played.get(ids::IMPROVE);
            for _ in 0..n {
                g.stack.push(select(
                    p, ids::IMPROVE, Zone::InPlay, Act::Trash, Filter::ActionToDiscard, 0, 1,
                    Then::GainUpTo { plus: 1, filter: Filter::Any, dest: Dest::Discard, exact: true, dest_by_type: false },
                ));
            }
        }
    }

    /// The Horn: once per turn, a Border Guard discarded from play may be put onto the deck.
    fn offers_on_discard(g: &mut GameState) {
        let p = g.turn.player;
        if g.ren.holder[(ids::HORN - crate::cards::FIRST_ARTIFACT) as usize] == p + 1 && g.players[p as usize].in_play.has(ids::BORDER_GUARD) {
            g.stack.push(Frame { zone: Zone::InPlay, act: Act::Topdeck, subject: ids::BORDER_GUARD, ..Frame::new(K::YesNo, p, ids::HORN) });
        }
    }
}

// -------------------------------------------------------------------------------------------
// Set-up and the state of the Artifacts, Villagers and Projects
// -------------------------------------------------------------------------------------------

/// Renaissance set-up: the Projects the kingdom lists.
pub(crate) fn setup(g: &mut GameState, cfg: &GameConfig, _n: usize) {
    for &k in &cfg.kingdom {
        if cards::is_project(k) {
            g.ren.projects |= project_bit(k);
        }
    }
}

impl GameState {
    /// `p` adds `n` Villagers to their mat (`source` gave them).
    pub(crate) fn give_villagers<S: EventSink>(&mut self, p: u8, source: CardId, n: u16, sink: &mut S) {
        if n > 0 {
            self.players[p as usize].ren.villagers += n;
            sink.event(Event::Villagers { player: p, source, n });
        }
    }

    /// `p` takes the Artifact `a`, from whoever has it.
    pub fn take_artifact<S: EventSink>(&mut self, p: u8, a: CardId, sink: &mut S) {
        let slot = &mut self.ren.holder[(a - crate::cards::FIRST_ARTIFACT) as usize];
        let from = std::mem::replace(slot, p + 1);
        sink.event(Event::Artifact { player: p, artifact: a, from: from.wrapping_sub(1) });
    }

    /// Whether `p` holds the Artifact `a`.
    #[inline]
    pub fn has_artifact(&self, p: u8, a: CardId) -> bool {
        self.ren.holder[(a - crate::cards::FIRST_ARTIFACT) as usize] == p + 1
    }

    /// The seat holding the Artifact `a`, if anyone does.
    pub fn artifact_holder(&self, a: CardId) -> Option<u8> {
        self.ren.holder[(a - crate::cards::FIRST_ARTIFACT) as usize].checked_sub(1)
    }

    /// Whether `p` has put a cube on the Project `pr`.
    #[inline]
    pub fn has_project(&self, p: u8, pr: CardId) -> bool {
        self.players[p as usize].ren.projects & project_bit(pr) != 0
    }

    /// Recomputes `RenState::hook` from the Projects the players own (after one is bought).
    pub fn ren_update_hooks(&mut self) {
        let owned = self.players.iter().fold(0u32, |m, ps| m | ps.ren.projects);
        let mut hook = 0;
        if owned & (project_bit(ids::ACADEMY) | project_bit(ids::GUILDHALL) | project_bit(ids::ROAD_NETWORK) | project_bit(ids::INNOVATION)) != 0 {
            hook |= REN_GAIN;
        }
        if owned & project_bit(ids::SEWERS) != 0 {
            hook |= REN_TRASH;
        }
        if owned & project_bit(ids::STAR_CHART) != 0 {
            hook |= REN_STAR;
        }
        if owned & project_bit(ids::CAPITALISM) != 0 {
            hook |= REN_CAPITAL;
        }
        self.ren.hook = hook;
    }

    /// Whether any player has Fleet.
    pub fn fleet_in_game(&self) -> bool {
        self.players.iter().any(|ps| ps.ren.projects & project_bit(ids::FLEET) != 0)
    }

    /// At the end of a turn of `p` (Clean-up done): whether the game is over. With Fleet, a game that would end gives one more turn to each player who
    /// has it (starting with the player after `p`, in turn order) before it does; `next` is then the seat that plays next, instead of the usual one.
    #[inline(never)]
    pub(crate) fn ren_game_over(&mut self, p: u8, next: &mut Option<u8>) -> bool {
        use crate::state::REN_FLEET_ROUND;
        let n = self.num_players;
        if self.ren.flags & REN_FLEET_ROUND == 0 {
            if !self.end_condition_met() {
                return false;
            }
            // The turn limit ends the game outright; so does a game with nobody holding Fleet.
            if self.end_reason() == Some(crate::state::EndReason::TurnLimit) || !self.fleet_in_game() {
                return true;
            }
            self.ren.flags |= REN_FLEET_ROUND;
            self.ren.fleet = 0;
            for q in 0..n {
                if self.players[q as usize].ren.projects & project_bit(ids::FLEET) != 0 {
                    self.ren.fleet |= 1 << q;
                }
            }
        } else {
            self.ren.fleet &= !(1 << p);
            if self.turn.number >= self.max_turns {
                return true;
            }
        }
        // The next seat after `p` that still has a Fleet turn to take.
        for i in 1..=n {
            let q = (p + i) % n;
            if self.ren.fleet & (1 << q) != 0 {
                *next = Some(q);
                return false;
            }
        }
        true
    }

    /// `p` puts a token on Sinister Plot.
    pub(crate) fn add_plot_token<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let t = &mut self.ren.plot[p as usize];
        *t = t.saturating_add(1);
        sink.event(Event::Plot { player: p, tokens: *t });
    }

    /// Whether the Project `pr` is on offer to the turn's player in the Buy phase, apart from its cost and the Buy: it is in the game, they do not have it,
    /// and they have a cube left (two per player).
    pub fn project_open(&self, pr: CardId) -> bool {
        let ps = &self.players[self.turn.player as usize];
        self.ren.projects & project_bit(pr) != 0 && ps.ren.projects & project_bit(pr) == 0 && ps.ren.projects.count_ones() < 2
    }

    /// Whether the turn's player may buy the Project `pr` now with the $ they have.
    pub fn project_buyable(&self, pr: CardId) -> bool {
        self.turn.coins >= cards::cost(pr) as u16 && self.project_open(pr)
    }

    /// Buys the Project `pr` for `p` (the turn's player): its cost and a Buy, then their cube goes on it.
    #[inline(never)]
    pub(crate) fn buy_project<S: EventSink>(&mut self, pr: CardId, p: u8, sink: &mut S) {
        self.turn.coins -= cards::cost(pr) as u16;
        self.spend_buy();
        self.turn.treasures_done = true;
        sink.depth(0);
        sink.event(Event::Buy { player: p, card: pr });
        self.players[p as usize].ren.projects |= project_bit(pr);
        self.ren_update_hooks();
        // Canal: cards cost $1 less, from now on this turn too.
        if pr == ids::CANAL {
            self.turn.cost_reduction = self.turn.cost_reduction.saturating_add(1);
        }
    }

    /// `p` reveals the card `c` (the event, and Patron's Coffers when it is revealed during an Action phase).
    #[inline]
    pub fn reveal_card<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        sink.event(Event::Reveal { player: p, card: c });
        if c == ids::PATRON && self.turn.phase == Phase::Action {
            self.give_coffers(p, ids::PATRON, 1, sink);
        }
    }

    /// An Action `card` was played by the turn's player (`resolve_effects`), with Projects in the game: Citadel plays the turn's first Action again.
    #[inline(never)]
    pub(crate) fn ren_on_action_play(&mut self, card: CardId, player: u8, tok: u8) {
        if player != self.turn.player || !self.is_action(card) || self.turn.ren.acted {
            return;
        }
        self.turn.ren.acted = true;
        if self.has_project(player, ids::CITADEL) {
            let duration = cards::is(card, DURATION);
            let in_play = self.players[player as usize].in_play.has(card);
            if duration && in_play && self.turn.multiplier_card == 0 {
                // A Duration played twice keeps the Citadel's second play too (nothing to keep: the Project stays).
            }
            let zone = if tok == crate::track::NIP { Zone::Supply } else { Zone::Hand };
            self.stack.push(Frame { subject: card, count: 2, zone, tok, ..Frame::new(K::PlayEffects, player, ids::CITADEL) });
        }
    }
}

// -------------------------------------------------------------------------------------------
// Frames: the continuations of selections, reveals and asks that belong to Renaissance
// -------------------------------------------------------------------------------------------

impl GameState {
    /// The `then` of a finished `Select` that belongs to Renaissance.
    #[inline(never)]
    pub(crate) fn finish_ren_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            // Border Guard: the rest of the revealed cards are discarded; with all Actions revealed an Artifact is taken.
            Then::BorderGuardPick => {
                self.discard_revealed(p, sink);
                if f.subject == 1 {
                    if self.has_artifact(p, ids::LANTERN) {
                        self.take_artifact(p, ids::HORN, sink);
                    } else {
                        self.stack.push(Frame { depth: f.depth, ..mode_frame(p, f.source, 1) });
                    }
                }
            }
            // Hideout: a Victory card trashed gives a Curse.
            Then::HideoutCurse => {
                if f.count > 0 && cards::is(f.last, VICTORY) {
                    self.gain(p, id::CURSE, Dest::Discard, false, f.source, sink);
                }
            }
            // Inventor: cards cost $1 less for the rest of the turn.
            Then::CostLess => {
                self.turn.cost_reduction = self.turn.cost_reduction.saturating_add(1);
            }
            // Priest: for the rest of the turn, each card trashed is +$2.
            Then::PriestSetup => {
                if p == self.turn.player {
                    self.turn.ren.priests = self.turn.ren.priests.saturating_add(1);
                }
            }
            Then::RecruiterVillagers => {
                if f.count > 0 {
                    let n = self.cost(f.last) as u16;
                    self.give_villagers(p, f.source, n, sink);
                }
            }
            // Research: one card from the top of the deck is set aside for each $1 the trashed card cost.
            Then::ResearchTrash => {
                if f.count > 0 {
                    let n = self.cost(f.last);
                    if n > 0 {
                        self.stack.push(Frame { depth: f.depth, max: n, then: Then::ResearchSet, ..Frame::new(K::RevealTop, p, f.source) });
                    }
                }
            }
            // Swashbuckler: with a card in the discard pile, +1 Coffers and, with at least 4 Coffers, the Treasure Chest.
            Then::Swashbuckler => {
                if !self.players[pi].discard.is_empty() {
                    self.give_coffers(p, f.source, 1, sink);
                    if self.players[pi].coffers >= 4 {
                        self.take_artifact(p, ids::TREASURE_CHEST, sink);
                    }
                }
            }
            // Scepter: the Action picked is played again.
            Then::ScepterReplay => {
                if f.count > 0 {
                    let card = f.last;
                    // The copy replayed is the one picked (in play, played this turn): its token tells the replay if it has moved by then.
                    let tok = self.track_replay(p, card);
                    self.stack.push(Frame { tok, depth: f.depth, ..Frame::new(K::Release, p, f.source) });
                    if cards::is(card, DURATION) && self.turn.multiplier_card == 0 && self.players[pi].in_play.has(f.source) {
                        self.turn.multiplier_card = f.source;
                        self.turn.multiplier_expected = 1;
                        self.turn.multiplier_successes = 0;
                        self.stack.push(Frame { depth: f.depth, subject: card, ..Frame::new(K::MultiplierFinalize, p, f.source) });
                    }
                    self.stack.push(Frame { subject: card, count: 2, depth: f.depth, tok, ..Frame::new(K::PlayEffects, p, f.source) });
                }
            }
            // Star Chart: the card picked goes on top, and the rest of the discard pile is shuffled into the deck.
            Then::StarChart => self.shuffle_discard(p, sink),
            _ => {}
        }
    }

    /// A `RevealTop` of Renaissance has finished.
    #[inline(never)]
    pub(crate) fn finish_ren_reveal<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            // Border Guard: pick one of the cards revealed for the hand. With every card an Action (and as many as asked for), an Artifact follows.
            Then::BorderGuardReveal => {
                let revealed = self.players[pi].set_aside;
                let all_actions = f.max == 0 && revealed.total() > 0 && revealed.iter().all(|(c, _)| self.is_action(c));
                self.stack.push(Frame {
                    depth: f.depth, subject: all_actions as CardId,
                    ..select(p, f.source, Zone::Revealed, Act::ToHand, Filter::Any, 1, 1, Then::BorderGuardPick)
                });
            }
            // Research: the cards are set aside face down; the Research stays in play for them.
            Then::ResearchSet => {
                let ps = &mut self.players[pi];
                let revealed = ps.set_aside;
                ps.set_aside.clear();
                if !revealed.is_empty() {
                    for (c, n) in revealed.iter() {
                        ps.ren.research.add(c, n);
                    }
                    self.push_duration_pending(p, ids::RESEARCH, 1, 0);
                }
            }
            // Piazza: the top card is played if it is an Action, else it goes back.
            Then::PiazzaPlay => {
                let Some(c) = self.players[pi].set_aside.iter().next().map(|(c, _)| c) else { return };
                self.players[pi].set_aside.remove(c);
                if self.is_action(c) {
                    self.players[pi].in_play.add(c, 1);
                    sink.event(Event::Play { player: p, card: c });
                    self.resolve_effects(c, p, f.depth, sink);
                } else {
                    self.players[pi].deck_known.push_top(c);
                }
            }
            _ => {}
        }
    }

    /// A `YesNo` of Renaissance was answered Yes (after the card moved, if it does).
    pub(crate) fn ren_yes<S: EventSink>(&mut self, f: &Frame, _sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            // Cargo Ship: the card gained (now in the set-aside zone) is set aside on the Cargo Ship, to come to the hand next turn.
            Then::CargoAside => {
                let c = f.subject;
                let ps = &mut self.players[pi];
                if ps.set_aside.remove(c) {
                    ps.ren.cargo.add(c, 1);
                    self.turn.ren.cargo = self.turn.ren.cargo.saturating_sub(1);
                    self.push_duration_pending(p, ids::CARGO_SHIP, 1, c);
                }
            }
            _ => {}
        }
    }

    /// Whether the `YesNo` frame `f` of Renaissance has nothing to ask any more.
    pub(crate) fn ren_yesno_pointless(&self, f: &Frame) -> bool {
        match f.then {
            Then::CargoAside => self.turn.ren.cargo == 0,
            Then::InnovationPlay => self.turn.ren.innovation,
            _ => false,
        }
    }

    /// An `Ask` of Renaissance is pointless: Pageant needs $1.
    pub(crate) fn ren_ask_pointless(&self, f: &Frame) -> bool {
        match f.then {
            Then::PageantPay => self.turn.coins < 1,
            Then::SinisterPlot => self.ren.plot[f.player as usize] == 0,
            _ => false,
        }
    }

    /// An `Ask` of Renaissance said Yes: Pageant's payment; Sinister Plot's new token (No removes the tokens for cards, see `ren_ask_no`).
    pub(crate) fn ren_ask_yes<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::PageantPay => {
                self.turn.coins -= 1;
                self.give_coffers(p, f.source, 1, sink);
            }
            Then::SinisterPlot => self.add_plot_token(p, sink),
            _ => {}
        }
    }

    /// An `Ask` of Renaissance said No: Sinister Plot's tokens are removed for +1 Card each.
    pub(crate) fn ren_ask_no<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        let p = f.player;
        if f.then == Then::SinisterPlot {
            let n = std::mem::take(&mut self.ren.plot[p as usize]);
            sink.event(Event::Plot { player: p, tokens: 0 });
            if n > 0 {
                self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, n) });
            }
        }
    }

    /// A gain of Renaissance finished with the card `c` gained: Sculptor's Villager for a Treasure.
    pub(crate) fn ren_gained<S: EventSink>(&mut self, f: &Frame, c: CardId, sink: &mut S) {
        if f.then == Then::VillagerIfTreasure && self.is_treasure(c) {
            self.give_villagers(f.player, f.source, 1, sink);
        }
    }

    /// Spends one of `p`'s Villagers for +1 Action (the Action phase).
    pub(crate) fn spend_villager<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        self.players[p as usize].ren.villagers -= 1;
        self.turn.actions += 1;
        sink.event(Event::SpendVillager { player: p });
    }

    /// The trash of a card by `p` with a Project in play that answers it: Priest's $2, and Sewers' extra trash (queued, see `flush_trash_triggers`).
    #[inline(never)]
    pub(crate) fn ren_trashed<S: EventSink>(&mut self, p: u8, _c: CardId, sink: &mut S) -> bool {
        if self.turn.ren.priests > 0 && p == self.turn.player {
            let k = 2 * self.turn.ren.priests as u16;
            self.turn.coins += k;
            sink.bonus(p, ids::PRIEST, 0, 0, k, 0);
        }
        self.ren.hook & REN_TRASH != 0 && !self.turn.ren.no_sewers && self.has_project(p, ids::SEWERS)
    }

    /// Sewers: the player may trash a card from the hand (the trash it makes does not trigger Sewers).
    pub(crate) fn sewers_frame(&mut self, p: u8) {
        self.stack.push(select(p, ids::SEWERS, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing));
    }

    /// `p`'s discard pile is shuffled into their deck (the deck is empty). The shuffle itself, as a draw does it.
    pub(crate) fn shuffle_discard<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        let d = ps.discard;
        if d.is_empty() {
            return;
        }
        ps.deck_unknown.add_all(&d);
        ps.discard.clear();
        // Rising Sun: the Shadow cards go to the bottom of the shuffled deck.
        if self.rs.flags & crate::state::R_SHADOW != 0 {
            ps.sink_shadows();
        }
        sink.event(Event::Shuffle { player: p });
        // Allies: Emissary counts the shuffles of the turn.
        if self.al.flags & crate::state::AF_STAMP != 0 {
            self.turn.al.shuffles = self.turn.al.shuffles.saturating_add(1);
        }
    }

    /// Star Chart, a shuffle is about to happen for `p`: if they have the Project and the discard pile holds two different cards, the player picks the one that
    /// goes on top (the frame that does the shuffle). Whether a frame was pushed.
    #[inline(never)]
    pub(crate) fn star_chart_shuffle(&mut self, p: u8) -> bool {
        let ps = &self.players[p as usize];
        if ps.ren.projects & project_bit(ids::STAR_CHART) == 0 || ps.discard.iter().nth(1).is_none() {
            return false;
        }
        self.stack.push(select(p, ids::STAR_CHART, Zone::Discard, Act::Topdeck, Filter::Any, 0, 1, Then::StarChart));
        true
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
                "Ducat", "Lackeys", "Acting Troupe", "Cargo Ship", "Experiment", "Improve", "Flag Bearer", "Patron", "Silk Merchant", "Old Witch",
                "Spices", "Villain",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Border Guard", "Hideout", "Mountain Village", "Priest", "Research", "Recruiter", "Scholar", "Seer", "Swashbuckler",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
        assert_eq!(names(cards::OnPlay::Choice), ["Inventor", "Scepter", "Sculptor", "Treasurer"]);
    }
}
