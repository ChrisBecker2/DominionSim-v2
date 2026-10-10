//! The expansions, one module each. Reading a card means reading one place: a set's file holds, for
//! its cards, the definitions (cost, types, vanilla bonuses, the choice-free / order-sensitive
//! marking), the play effect, and the card's triggers: gain and buy effects, the start-of-turn part
//! of a Duration, cleanup offers, the reaction, a modes table, bonus VP. What a set does not
//! override is a no-op here (see [`SetRules`]); the generic machinery (frames, `Then`
//! continuations, filters, the turn loop) stays in `effects.rs` / `engine.rs`, and the set files
//! compose it.
//!
//! ## How it is wired (no cost at run time)
//!
//! * **The table.** Each set lists `(id, CardDef)` pairs; [`CARDS`] is built from them by a `const fn`
//!   that puts every definition at index `id` and fails the *build* on a duplicate id, a gap, or a
//!   card whose `set` is not the set that lists it. So `def(id)` / `is(id, flag)` stay a single array index,
//!   and the `id::` constants live next to the cards they name (`base::ids`, ...; re-exported by
//!   `cards::id`).
//! * **Dispatch.** Per-card behaviour is a static [`SetRules`] method. [`with_set!`] expands to a `match`
//!   on the card's [`CardSet`] that calls the method of the concrete set type: a jump into the
//!   set's own (inlined) `match card`, no trait object, no function pointer, no indirect call.
//!   Hooks that are not about one card (gain triggers, cleanup offers, ...) are called for each set in a fixed
//!   order, written down once below; a set with nothing to do compiles to nothing.
//! * **Masks.** The gating bitmasks (`GAIN_LO`/`GAIN_HI` per set) are OR-ed into one constant at compile
//!   time, so the hot path still tests one `u128`.
//!
//! To add a set, see `docs/adding-a-set.md`.

use crate::cards::{CardDef, CardId, CardSet, ModeOpt};
use crate::counts::Counts;
use crate::engine::EventSink;
use crate::state::{Dest, GameState, PendingDuration};

pub mod adventures;
pub mod alchemy;
pub mod base;
pub mod cornucopia_guilds;
pub mod dark_ages;
pub mod empires;
pub mod intrigue;
pub mod promo;
pub mod prosperity;
pub mod rising_sun;
pub mod seaside;

use adventures::Adventures;
use alchemy::Alchemy;
use base::Base;
use cornucopia_guilds::CornucopiaGuilds;
use dark_ages::DarkAges;
use empires::Empires;
use intrigue::Intrigue;
use promo::Promo;
use prosperity::Prosperity;
use rising_sun::RisingSun;
use seaside::Seaside;

/// What the set files import: the card-table helpers and the generic effect machinery.
pub(crate) mod prelude {
    pub(crate) use super::SetRules;
    pub(crate) use crate::cards::{
        self, choice_free, has_choice, id, order_sensitive, plays, potion_card, with_p, CardDef, CardId, CardSet, ModeOpt,
        ACTION, ATTACK, COMMAND, CURSE_T, DURATION, KNIGHT, LOOTER, REACTION, REWARD, RUINS, SHELTER, TREASURE, VICTORY,
    };
    pub(crate) use crate::cards::{alchemy, c, cg, da, intrigue, on_trash, promo, prosperity, seaside};
    pub(crate) use crate::counts::Counts;
    pub(crate) use crate::effects::{draw_frame, gain_frame, mode_frame, select, take_top, ALL};
    pub(crate) use crate::engine::*;
    pub(crate) use crate::state::{
        Act, Dest, Filter, Frame, FrameKind, FrameKind as K, GameState, PendingDuration, Phase, Then, Zone, MAX_PLAYERS,
    };
}

/// Expands `$body` once per set with `$S` naming that set's [`SetRules`] type, as a `match` on `$set`.
macro_rules! with_set {
    ($set:expr, $S:ident => $body:expr) => {
        match $set {
            $crate::cards::CardSet::Base => {
                type $S = $crate::sets::base::Base;
                $body
            }
            $crate::cards::CardSet::Intrigue => {
                type $S = $crate::sets::intrigue::Intrigue;
                $body
            }
            $crate::cards::CardSet::Seaside => {
                type $S = $crate::sets::seaside::Seaside;
                $body
            }
            $crate::cards::CardSet::Prosperity => {
                type $S = $crate::sets::prosperity::Prosperity;
                $body
            }
            $crate::cards::CardSet::Alchemy => {
                type $S = $crate::sets::alchemy::Alchemy;
                $body
            }
            $crate::cards::CardSet::Promo => {
                type $S = $crate::sets::promo::Promo;
                $body
            }
            $crate::cards::CardSet::CornucopiaGuilds => {
                type $S = $crate::sets::cornucopia_guilds::CornucopiaGuilds;
                $body
            }
            $crate::cards::CardSet::DarkAges => {
                type $S = $crate::sets::dark_ages::DarkAges;
                $body
            }
            $crate::cards::CardSet::Adventures => {
                type $S = $crate::sets::adventures::Adventures;
                $body
            }
            $crate::cards::CardSet::Empires => {
                type $S = $crate::sets::empires::Empires;
                $body
            }
            $crate::cards::CardSet::RisingSun => {
                type $S = $crate::sets::rising_sun::RisingSun;
                $body
            }
        }
    };
}

/// Everything a set contributes beyond its table of cards. Every method has a no-op default, so a
/// set implements only what its cards need. Methods taking a `card` are called for a card of this set
/// only (the dispatchers below pick the set from the card's definition).
pub(crate) trait SetRules {
    /// The set's cards, each with its id (see the module docs).
    const CARDS: &'static [(CardId, CardDef)];

    // ---- data ----

    /// Cards with a "when you gain" effect or passive, as bits of `GameState::in_supply.lo` (ids below 128)
    /// and `.hi` (ids from 128): when none is in the game, `GameState::gain` skips every trigger.
    const GAIN_LO: u128 = 0;
    const GAIN_HI: u128 = 0;
    /// The same for ids from 256 (bits of `in_supply.top`).
    const GAIN_TOP: u128 = 0;
    /// The cards of this set whose `may_buy` can say no (Grand Market, Avanto): every other card is buyable whenever it is affordable,
    /// and the Buy choices skip the call for it.
    const MAY_BUY_CARDS: &'static [CardId] = &[];
    /// Reaction cards that, revealed from the hand when another player plays an Attack, leave their
    /// owner unaffected by it (Moat). Revealing is the owner's choice (`Then::BlockAttack`).
    const BLOCK_REACTIONS: &'static [CardId] = &[];
    /// Reactions that, discarded or revealed from the hand when another player plays an Attack, do something of their own:
    /// `(card, how it leaves the hand, what follows)` (Beggar: discard it to gain two Silvers). Asked in the Attack's reaction window.
    const ATTACK_REACTIONS: &'static [(CardId, crate::state::Act, crate::state::Then)] = &[];
    /// Reactions that offer an optional effect (not a block) when another player plays an Attack:
    /// `(card, minimum hand size to reveal, cards drawn, cards then discarded)` (Diplomat).
    const REACTION_EFFECTS: &'static [(CardId, u8, u8, u8)] = &[];
    /// Cards that, while in play, leave their owner unaffected by other players' Attacks (Lighthouse).
    const IMMUNE_IN_PLAY: &'static [CardId] = &[];

    /// The ordered "choose one/many" options `card` offers (see `ModeOpt`).
    fn modes(_card: CardId) -> &'static [ModeOpt] {
        &[]
    }
    /// (cards, actions, buys, coins) a Duration `card` grants at the start of the owner's next turn, per play.
    fn duration_bonus(_card: CardId) -> (u8, u8, u8, u8) {
        (0, 0, 0, 0)
    }
    /// Whether the start-of-turn effect of `card` is order-sensitive (see `cards::start_order_sensitive`).
    fn start_order_sensitive(_card: CardId) -> bool {
        false
    }
    /// A start-of-turn entry for `card` is named in `DecisionKind::StartOrder` by the card it plays
    /// (`PendingDuration::arg`), not by itself (Prince, Farmhands).
    fn start_entry_named_by_arg(_card: CardId) -> bool {
        false
    }
    /// A start-of-turn entry for `card` stands for `times` identical effects that resolve one at a time (Clerk, Prince).
    fn start_entry_repeats(_card: CardId) -> bool {
        false
    }
    /// Whether buying `card` may be overpaid for (a "+" in the cost).
    fn has_overpay(_card: CardId) -> bool {
        false
    }
    /// Whether `card`'s bonus depends on how many supply piles are empty (City).
    fn reads_empty_piles(_card: CardId) -> bool {
        false
    }
    /// The card's victory points in a collection `all` of `total` cards, for cards whose score is not a fixed
    /// number (Gardens, Duke, ...). The default is the printed VP.
    fn vp(card: CardId, _all: &Counts, _total: i32) -> i32 {
        crate::cards::def(card).vp as i32
    }
    /// Card-specific rules on whether `card` may be bought right now, beyond cost and pile (Grand Market).
    fn may_buy(_g: &GameState, _card: CardId) -> bool {
        true
    }
    /// How many cards of this pile a game starts with, when it is not the usual 10 (12 for Platinum, ...).
    fn pile_size(_card: CardId, _num_players: usize) -> Option<u8> {
        None
    }

    // ---- effects ----

    /// What playing `card` does beyond its vanilla bonuses (+Cards/+Actions/+Buys/+$ and, for a Duration, its
    /// scheduling, which are generic): usually pushes frames. Runs once per play, after any reaction window.
    #[inline]
    fn play<S: EventSink>(_g: &mut GameState, _card: CardId, _p: u8, _sink: &mut S) {}
    /// Whether the Duration `card` is not scheduled by the generic tail of a play (a pending start-of-next-turn entry): it
    /// schedules itself once its argument is known (Haven, Blockade, Tactician), or has no pending entry (Prince).
    fn schedules_own_duration(_card: CardId) -> bool {
        false
    }
    /// A pending Duration effect of `e.card` firing at the start of `p`'s turn, beyond the vanilla
    /// `duration_bonus` (applied by the caller).
    fn duration_start<S: EventSink>(_g: &mut GameState, _p: u8, _e: &PendingDuration, _sink: &mut S) {}
    /// A start-of-turn entry that is not a pending Duration (Clerk, Prince, Farmhands): resolve it and return true.
    fn take_start_effect<S: EventSink>(_g: &mut GameState, _p: u8, _e: &PendingDuration, _sink: &mut S) -> bool {
        false
    }
    /// Adds this set's start-of-turn entries (cards held for this turn, Clerk in hand) to `TurnState::start_pending`.
    fn turn_start_entries(_g: &mut GameState) {}
    /// "When you gain" triggers for `p` gaining `c`, already moved to `to`. Called when the set's `GAIN_LO` /
    /// `GAIN_HI` bits (or `gain_live_extra`) say a card of the set is in the game.
    #[inline]
    fn gain_triggers<S: EventSink>(_g: &mut GameState, _p: u8, _c: CardId, _to: Dest, _on_buy: bool, _sink: &mut S) {}
    /// Extra condition, besides the mask, for the set's gain triggers to be live (Ferryman's pile).
    #[inline(always)]
    fn gain_live_extra(_g: &GameState) -> bool {
        false
    }
    /// The end of the Buy phase, before the cleanup offers (Merchant Guild's Coffers).
    fn end_of_buy<S: EventSink>(_g: &mut GameState, _sink: &mut S) {}
    /// The "when you trash this" effect of `card` (the cards marked `on_trash`) for `p`, who was told to trash it: pushes frames or acts at once.
    fn on_trash<S: EventSink>(_g: &mut GameState, _p: u8, _card: CardId, _sink: &mut S) {}
    /// Offers "when you discard this from play" (Treasury, Herbalist), pushed as frames.
    fn offers_on_discard(_g: &mut GameState) {}
    /// Offers "at the start of Clean-up" (Alchemist, Walled Village), pushed as frames.
    fn offers_at_cleanup(_g: &mut GameState) {}
    /// Whether the buyer of `card`, with `g`'s coins and Potions, is asked how much to overpay (see `has_overpay`).
    fn overpay_possible(_g: &GameState, _card: CardId) -> bool {
        false
    }
    /// Whether the overpay decision for `card` also offers paying a Potion (Stonemason).
    fn overpay_with_potion(_g: &GameState, _card: CardId) -> bool {
        false
    }
    /// The overpay ability of `card`, bought and overpaid by `coins` (and a Potion): pay it, push the ability.
    fn overpay<S: EventSink>(_g: &mut GameState, _p: u8, _card: CardId, _coins: u8, _potion: bool, _depth: u8, _sink: &mut S) {}
}

// -------------------------------------------------------------------------------------------
// The table
// -------------------------------------------------------------------------------------------

/// Number of cards of every set (ids are dense: `0..NUM_CARDS`).
pub const NUM_CARDS: usize =
    Base::CARDS.len() + Intrigue::CARDS.len() + Seaside::CARDS.len() + Prosperity::CARDS.len() + Alchemy::CARDS.len() + Promo::CARDS.len()
        + CornucopiaGuilds::CARDS.len() + DarkAges::CARDS.len() + Adventures::CARDS.len() + Empires::CARDS.len() + RisingSun::CARDS.len();

const fn build_cards() -> [CardDef; NUM_CARDS] {
    let parts: [(CardSet, &[(CardId, CardDef)]); 11] = [
        (CardSet::Base, Base::CARDS),
        (CardSet::Intrigue, Intrigue::CARDS),
        (CardSet::Seaside, Seaside::CARDS),
        (CardSet::Prosperity, Prosperity::CARDS),
        (CardSet::Alchemy, Alchemy::CARDS),
        (CardSet::Promo, Promo::CARDS),
        (CardSet::CornucopiaGuilds, CornucopiaGuilds::CARDS),
        (CardSet::DarkAges, DarkAges::CARDS),
        (CardSet::Adventures, Adventures::CARDS),
        (CardSet::Empires, Empires::CARDS),
        (CardSet::RisingSun, RisingSun::CARDS),
    ];
    let mut out = [crate::cards::c("", 0, 0, 0, 0, 0, 0, 0); NUM_CARDS];
    let mut filled = [false; NUM_CARDS];
    let mut k = 0;
    while k < parts.len() {
        let (set, cards) = parts[k];
        let mut i = 0;
        while i < cards.len() {
            let (id, def) = cards[i];
            assert!((id as usize) < NUM_CARDS, "a card id is not below NUM_CARDS (ids must be dense)");
            assert!(!filled[id as usize], "two cards have the same id");
            assert!(def.set as u8 == set as u8, "a card is listed in a set module that is not its CardDef::set");
            out[id as usize] = def;
            filled[id as usize] = true;
            i += 1;
        }
        k += 1;
    }
    let mut i = 0;
    while i < NUM_CARDS {
        assert!(filled[i], "a card id has no definition (ids must be dense)");
        i += 1;
    }
    out
}

/// Every card, indexed by id: one array index per lookup.
pub static CARDS: [CardDef; NUM_CARDS] = build_cards();

const fn concat<T: Copy, const N: usize>(parts: &[&[T]], zero: T) -> [T; N] {
    let mut out = [zero; N];
    let mut n = 0;
    let mut k = 0;
    while k < parts.len() {
        let mut i = 0;
        while i < parts[k].len() {
            out[n] = parts[k][i];
            n += 1;
            i += 1;
        }
        k += 1;
    }
    assert!(n == N);
    out
}

/// A static slice that concatenates the associated slice constant `$name` of every set.
macro_rules! concat_sets {
    ($T:ty, $zero:expr, $name:ident) => {{
        const N: usize = Base::$name.len()
            + Intrigue::$name.len()
            + Seaside::$name.len()
            + Prosperity::$name.len()
            + Alchemy::$name.len()
            + Promo::$name.len()
            + CornucopiaGuilds::$name.len()
            + DarkAges::$name.len()
            + Adventures::$name.len()
            + Empires::$name.len()
            + RisingSun::$name.len();
        const A: [$T; N] = concat::<$T, N>(
            &[
                Base::$name, Intrigue::$name, Seaside::$name, Prosperity::$name, Alchemy::$name, Promo::$name, CornucopiaGuilds::$name,
                DarkAges::$name, Adventures::$name, Empires::$name, RisingSun::$name,
            ],
            $zero,
        );
        &A
    }};
}

/// Reaction cards that block an Attack when revealed (every set's `BLOCK_REACTIONS`).
pub static BLOCK_REACTIONS: &[CardId] = concat_sets!(CardId, 0, BLOCK_REACTIONS);
/// Reactions with an effect of their own to an Attack (every set's `ATTACK_REACTIONS`).
pub static ATTACK_REACTIONS: &[(CardId, crate::state::Act, crate::state::Then)] =
    concat_sets!((CardId, crate::state::Act, crate::state::Then), (0, crate::state::Act::Discard, crate::state::Then::Nothing), ATTACK_REACTIONS);
/// Reactions with an optional effect (every set's `REACTION_EFFECTS`).
pub static REACTION_EFFECTS: &[(CardId, u8, u8, u8)] = concat_sets!((CardId, u8, u8, u8), (0, 0, 0, 0), REACTION_EFFECTS);
/// Cards whose owner is unaffected by Attacks while they are in play (every set's `IMMUNE_IN_PLAY`).
pub static IMMUNE_IN_PLAY: &[CardId] = concat_sets!(CardId, 0, IMMUNE_IN_PLAY);

/// Bits of `in_supply.lo` / `.hi` of every card with a gain trigger, over all sets.
pub const GAIN_LO: u128 = Base::GAIN_LO | Intrigue::GAIN_LO | Seaside::GAIN_LO | Prosperity::GAIN_LO | Alchemy::GAIN_LO | Promo::GAIN_LO
    | CornucopiaGuilds::GAIN_LO | DarkAges::GAIN_LO | Adventures::GAIN_LO;
pub const GAIN_HI: u128 = Base::GAIN_HI | Intrigue::GAIN_HI | Seaside::GAIN_HI | Prosperity::GAIN_HI | Alchemy::GAIN_HI | Promo::GAIN_HI
    | CornucopiaGuilds::GAIN_HI | DarkAges::GAIN_HI | Adventures::GAIN_HI | Empires::GAIN_HI | RisingSun::GAIN_HI;
pub const GAIN_TOP: u128 = Empires::GAIN_TOP | RisingSun::GAIN_TOP | Adventures::GAIN_TOP;

// -------------------------------------------------------------------------------------------
// Dispatch by card
// -------------------------------------------------------------------------------------------

#[inline(always)]
fn set_of(card: CardId) -> CardSet {
    CARDS[card as usize].set
}

/// `SetRules::modes` of the card's set.
pub(crate) fn modes(card: CardId) -> &'static [ModeOpt] {
    with_set!(set_of(card), S => S::modes(card))
}

pub(crate) fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
    with_set!(set_of(card), S => S::duration_bonus(card))
}

pub(crate) fn start_order_sensitive(card: CardId) -> bool {
    with_set!(set_of(card), S => S::start_order_sensitive(card))
}

pub(crate) fn start_entry_named_by_arg(card: CardId) -> bool {
    with_set!(set_of(card), S => S::start_entry_named_by_arg(card))
}

pub(crate) fn start_entry_repeats(card: CardId) -> bool {
    with_set!(set_of(card), S => S::start_entry_repeats(card))
}

pub(crate) fn has_overpay(card: CardId) -> bool {
    with_set!(set_of(card), S => S::has_overpay(card))
}

pub(crate) fn reads_empty_piles(card: CardId) -> bool {
    with_set!(set_of(card), S => S::reads_empty_piles(card))
}

pub(crate) fn vp(card: CardId, all: &Counts, total: i32) -> i32 {
    with_set!(set_of(card), S => S::vp(card, all, total))
}

/// Whether a card has a `may_buy` restriction at all (every set's `MAY_BUY_CARDS`).
static MAY_BUY_RESTRICTED: [bool; NUM_CARDS] = {
    let list = concat_sets!(CardId, 0, MAY_BUY_CARDS);
    let mut t = [false; NUM_CARDS];
    let mut i = 0;
    while i < list.len() {
        t[list[i] as usize] = true;
        i += 1;
    }
    t
};

#[inline(always)]
pub(crate) fn may_buy(g: &GameState, card: CardId) -> bool {
    !MAY_BUY_RESTRICTED[card as usize] || may_buy_restricted(g, card)
}

/// The cards with a `may_buy` restriction (rare in a Buy decision): out of line, so that the table test above is all the usual card costs.
#[inline(never)]
fn may_buy_restricted(g: &GameState, card: CardId) -> bool {
    with_set!(set_of(card), S => S::may_buy(g, card))
}

pub(crate) fn pile_size(card: CardId, num_players: usize) -> Option<u8> {
    with_set!(set_of(card), S => S::pile_size(card, num_players))
}

/// The play effect of `card` (a card of set `set`). One jump into the set's `match card`.
#[inline(always)]
pub(crate) fn play<S: EventSink>(g: &mut GameState, set: CardSet, card: CardId, p: u8, sink: &mut S) {
    with_set!(set, T => T::play(g, card, p, sink))
}

pub(crate) fn schedules_own_duration(set: CardSet, card: CardId) -> bool {
    with_set!(set, S => S::schedules_own_duration(card))
}

pub(crate) fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
    with_set!(set_of(e.card), T => T::duration_start(g, p, e, sink))
}

pub(crate) fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
    with_set!(set_of(e.card), T => T::take_start_effect(g, p, e, sink))
}

pub(crate) fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, sink: &mut S) {
    with_set!(set_of(card), T => T::on_trash(g, p, card, sink))
}

pub(crate) fn overpay_possible(g: &GameState, card: CardId) -> bool {
    with_set!(set_of(card), S => S::overpay_possible(g, card))
}

pub(crate) fn overpay_with_potion(g: &GameState, card: CardId) -> bool {
    with_set!(set_of(card), S => S::overpay_with_potion(g, card))
}

pub(crate) fn overpay<S: EventSink>(g: &mut GameState, p: u8, card: CardId, coins: u8, potion: bool, depth: u8, sink: &mut S) {
    with_set!(set_of(card), T => T::overpay(g, p, card, coins, potion, depth, sink))
}

// -------------------------------------------------------------------------------------------
// Hooks called for every set, in a fixed order
// -------------------------------------------------------------------------------------------

/// Start-of-turn entries, after the pending Durations: the order of the entries in the list.
pub(crate) fn turn_start_entries(g: &mut GameState) {
    Promo::turn_start_entries(g);
    CornucopiaGuilds::turn_start_entries(g);
    Prosperity::turn_start_entries(g);
    Adventures::turn_start_entries(g);
}

/// Whether any "when you gain" trigger can be live: one mask test (plus the sets' extra conditions).
#[inline(always)]
pub(crate) fn gain_triggers_possible(g: &GameState) -> bool {
    g.in_supply.any(GAIN_LO, GAIN_HI, GAIN_TOP)
        || CornucopiaGuilds::gain_live_extra(g)
        || DarkAges::gain_live_extra(g)
        || Adventures::gain_live_extra(g)
        || Empires::gain_live_extra(g)
}

/// Every "when you gain" trigger for `p` gaining `c` (moved to `to`). The order is the order the
/// frames are pushed in (a later set's frames are asked first); a set's triggers are skipped when
/// none of its cards is in the game.
#[inline]
pub(crate) fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, on_buy: bool, sink: &mut S) {
    if g.in_supply.hi & Empires::GAIN_HI != 0 || g.in_supply.top & Empires::GAIN_TOP != 0 || Empires::gain_live_extra(g) {
        Empires::gain_triggers(g, p, c, to, on_buy, sink);
    }
    if g.in_supply.hi & Adventures::GAIN_HI != 0 || Adventures::gain_live_extra(g) {
        Adventures::gain_triggers(g, p, c, to, on_buy, sink);
    }
    if g.in_supply.hi & CornucopiaGuilds::GAIN_HI != 0 || CornucopiaGuilds::gain_live_extra(g) {
        CornucopiaGuilds::gain_triggers(g, p, c, to, on_buy, sink);
    }
    if g.in_supply.hi & DarkAges::GAIN_HI != 0 || DarkAges::gain_live_extra(g) {
        DarkAges::gain_triggers(g, p, c, to, on_buy, sink);
    }
    if g.in_supply.lo & Seaside::GAIN_LO != 0 {
        Seaside::gain_triggers(g, p, c, to, on_buy, sink);
    }
    if g.in_supply.lo & Prosperity::GAIN_LO != 0 {
        Prosperity::gain_triggers(g, p, c, to, on_buy, sink);
    }
}

/// The end of the Buy phase (before the cleanup offers).
pub(crate) fn end_of_buy<S: EventSink>(g: &mut GameState, sink: &mut S) {
    CornucopiaGuilds::end_of_buy(g, sink);
    DarkAges::end_of_buy(g, sink);
    Adventures::end_of_buy(g, sink);
}

/// The offers made at the end of the Buy phase, pushed in the order below: the stack pops the last first, so
/// they are asked in the reverse order. "When you discard this from play" offers (Treasury, Herbalist) come
/// last, and "at the start of Clean-up" offers (Alchemist, Walled Village) first, as the rules order them.
pub(crate) fn end_of_turn_offers(g: &mut GameState) {
    Alchemy::offers_on_discard(g);
    Seaside::offers_on_discard(g);
    Adventures::offers_on_discard(g);
    Promo::offers_at_cleanup(g);
    Alchemy::offers_at_cleanup(g);
}
