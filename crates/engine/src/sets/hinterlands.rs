//! Hinterlands, 2nd edition (26 kingdom cards). See `docs/hinterlands-plan.md` for the rulings, the trigger timing design and the bot defaults.

use super::prelude::*;
use crate::cards::hl;
use crate::state::{G_DISCARD_HOOK, N_HOUND, Q_DISCARDED, TRASH_QUEUE_CAP};

/// Card ids of this set (dense, contiguous, after Nocturne's non-Supply cards and before the Ruins and Knights, which stay the highest zone ids).
pub mod ids {
    use crate::cards::CardId;
    pub const CROSSROADS: CardId = 328;
    pub const FOOLS_GOLD: CardId = 329;
    pub const DEVELOP: CardId = 330;
    pub const GUARD_DOG: CardId = 331;
    pub const OASIS: CardId = 332;
    pub const SCHEME: CardId = 333;
    pub const TUNNEL: CardId = 334;
    pub const JACK_OF_ALL_TRADES: CardId = 335;
    pub const NOMADS: CardId = 336;
    pub const SPICE_MERCHANT: CardId = 337;
    pub const TRADER: CardId = 338;
    pub const TRAIL: CardId = 339;
    pub const WEAVER: CardId = 340;
    pub const BERSERKER: CardId = 341;
    pub const CARTOGRAPHER: CardId = 342;
    pub const CAULDRON: CardId = 343;
    pub const HAGGLER: CardId = 344;
    pub const HIGHWAY: CardId = 345;
    pub const INN: CardId = 346;
    pub const MARGRAVE: CardId = 347;
    pub const SOUK: CardId = 348;
    pub const STABLES: CardId = 349;
    pub const WHEELWRIGHT: CardId = 350;
    pub const WITCHS_HUT: CardId = 351;
    pub const BORDER_VILLAGE: CardId = 352;
    pub const FARMLAND: CardId = 353;
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::CROSSROADS, hl(order_sensitive(c("Crossroads", 2, ACTION, 0, 0, 0, 0, 0)))), // draws per Victory card in the hand; +3 Actions the first time
    (ids::FOOLS_GOLD, hl(choice_free(c("Fool's Gold", 2, TREASURE | REACTION, 1, 0, 0, 0, 0)))), // $1 the first time, $4 after
    (ids::DEVELOP, hl(order_sensitive(c("Develop", 3, ACTION, 0, 0, 0, 0, 0)))), // trashes any card of the hand
    (ids::GUARD_DOG, hl(order_sensitive(c("Guard Dog", 3, ACTION | REACTION, 0, 0, 2, 0, 0)))), // +2 Cards more with a small hand
    (ids::OASIS, hl(order_sensitive(c("Oasis", 3, ACTION, 1, 0, 1, 1, 0)))), // discards any card of the hand
    (ids::SCHEME, hl(has_choice(c("Scheme", 3, ACTION, 0, 0, 1, 1, 0)))), // may put Actions onto the deck at Clean-up
    (ids::TUNNEL, hl(c("Tunnel", 3, VICTORY | REACTION, 0, 2, 0, 0, 0))),
    (ids::JACK_OF_ALL_TRADES, hl(order_sensitive(c("Jack of All Trades", 4, ACTION, 0, 0, 0, 0, 0)))), // looks at the deck top, draws to 5, trashes
    (ids::NOMADS, hl(on_trash(choice_free(c("Nomads", 4, ACTION, 2, 0, 0, 0, 1))))), // +$2 when gained or trashed
    (ids::SPICE_MERCHANT, hl(has_choice(c("Spice Merchant", 4, ACTION, 0, 0, 0, 0, 0)))), // may trash a Treasure
    (ids::TRADER, hl(order_sensitive(c("Trader", 4, ACTION | REACTION, 0, 0, 0, 0, 0)))), // trashes any card of the hand
    (ids::TRAIL, hl(on_trash(choice_free(c("Trail", 4, ACTION | REACTION, 0, 0, 1, 1, 0))))), // may be played when gained, trashed or discarded
    (ids::WEAVER, hl(has_choice(c("Weaver", 4, ACTION | REACTION, 0, 0, 0, 0, 0)))), // two Silvers or a card up to $4; may be played when discarded
    (ids::BERSERKER, hl(has_choice(c("Berserker", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // gains a cheaper card; played when gained
    (ids::CARTOGRAPHER, hl(order_sensitive(c("Cartographer", 5, ACTION, 0, 0, 1, 1, 0)))), // looks at and reorders the top 4
    (ids::CAULDRON, hl(has_choice(c("Cauldron", 5, TREASURE | ATTACK, 2, 0, 0, 0, 1)))), // an Attack: played through the reaction window
    (ids::HAGGLER, hl(choice_free(c("Haggler", 5, ACTION, 2, 0, 0, 0, 0)))), // gains a cheaper card for each card bought
    (ids::HIGHWAY, hl(choice_free(c("Highway", 5, ACTION, 0, 0, 1, 1, 0)))),
    (ids::INN, hl(order_sensitive(c("Inn", 5, ACTION, 0, 0, 2, 2, 0)))), // discards any 2 cards of the hand
    (ids::MARGRAVE, hl(choice_free(c("Margrave", 5, ACTION | ATTACK, 0, 0, 3, 0, 1)))),
    (ids::SOUK, hl(order_sensitive(c("Souk", 5, ACTION, 0, 0, 0, 0, 1)))), // -$1 per card in the hand
    (ids::STABLES, hl(has_choice(c("Stables", 5, ACTION, 0, 0, 0, 0, 0)))), // may discard a Treasure
    (ids::WHEELWRIGHT, hl(order_sensitive(c("Wheelwright", 5, ACTION, 0, 0, 1, 1, 0)))), // may discard any card
    (ids::WITCHS_HUT, hl(order_sensitive(c("Witch's Hut", 5, ACTION | ATTACK, 0, 0, 4, 0, 0)))), // discards 2 cards
    (ids::BORDER_VILLAGE, hl(choice_free(c("Border Village", 6, ACTION, 0, 0, 1, 2, 0)))), // gains a cheaper card when gained
    (ids::FARMLAND, hl(c("Farmland", 6, VICTORY, 0, 2, 0, 0, 0))),
];

/// The Hinterlands cards (see the module docs of `sets`).
pub struct Hinterlands;

impl SetRules for Hinterlands {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    /// Cards with a "when you gain" effect or reaction: Border Village, Farmland, Inn, Souk, Nomads, Berserker and Trail (their own), Haggler,
    /// Cauldron, Fool's Gold (a Province gained by another player) and Trader (the exchange).
    const GAIN_TOP: u128 = (1u128 << (ids::BORDER_VILLAGE - 256))
        | (1u128 << (ids::FARMLAND - 256))
        | (1u128 << (ids::INN - 256))
        | (1u128 << (ids::SOUK - 256))
        | (1u128 << (ids::NOMADS - 256))
        | (1u128 << (ids::BERSERKER - 256))
        | (1u128 << (ids::TRAIL - 256))
        | (1u128 << (ids::HAGGLER - 256))
        | (1u128 << (ids::CAULDRON - 256))
        | (1u128 << (ids::FOOLS_GOLD - 256))
        | (1u128 << (ids::TRADER - 256));
    /// Guard Dog: played from the hand in response to an Attack.
    const ATTACK_REACTIONS: &'static [(CardId, Act, Then)] = &[(ids::GUARD_DOG, Act::Play, Then::Nothing)];

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            // Two Silvers or a card costing up to $4.
            ids::WEAVER => &[ModeOpt::GainMany(id::SILVER, Dest::Discard, 2), ModeOpt::GainUpToCost(4)],
            // After trashing a Treasure: +2 Cards and +1 Action, or +1 Buy and +$2.
            ids::SPICE_MERCHANT => &[ModeOpt::CardsActions(2, 1), ModeOpt::BuysCoins(1, 2)],
            _ => &[],
        }
    }

    #[inline(never)]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        let pi = p as usize;
        // A card played out of turn (in response to an Attack, or when gained or discarded on another player's turn) is discarded in that
        // turn's Clean-up.
        if p != g.turn.player && matches!(card, ids::GUARD_DOG | ids::TRAIL | ids::WEAVER | ids::BERSERKER) {
            g.players[pi].discard_next_cleanup.add(card, 1);
        }
        match card {
            ids::CROSSROADS => {
                let hand = g.players[pi].hand;
                for (c, n) in hand.iter() {
                    for _ in 0..n {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                }
                let victory = hand.count_type(cards::VICTORY).min(u8::MAX as u32) as u8;
                // +3 Actions the first time a Crossroads is played this turn (the play being resolved is counted).
                if g.turn.played.get(card) <= 1 {
                    g.turn.actions += 3;
                    sink.bonus(p, card, 3, 0, 0, 0);
                }
                if victory > 0 {
                    g.stack.push(draw_frame(p, card, victory));
                }
            }
            // The second play and later are worth $4 (the first, and the Treasures auto-played, are `play_free_treasure_from`).
            ids::FOOLS_GOLD => {
                if g.turn.played.get(card) > 1 {
                    g.turn.coins += 3;
                    sink.bonus(p, card, 0, 0, 3, 0);
                }
            }
            ids::DEVELOP => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::DevelopGain)),
            // +2 Cards, then +2 Cards more with 5 or fewer cards in the hand (the first draw is pushed by the generic tail of a play).
            ids::GUARD_DOG => {
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::DrawIfHandAtMost { max_hand: 5, draw: 2 }))
            }
            ids::OASIS => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 1, 1, Then::Nothing)),
            ids::JACK_OF_ALL_TRADES => {
                // Gain a Silver; look at the top card of the deck and discard it or not; draw until 5 cards are in the hand; may trash a non-Treasure.
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::NonTreasure, 0, 1, Then::Nothing));
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::DrawUpTo(5)));
                g.look_at_top_discard_one(p, card);
                g.stack.push(gain_frame(p, card, u8::MAX, Filter::Card(id::SILVER), Dest::Discard));
            }
            ids::SPICE_MERCHANT => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Treasure, 0, 1, Then::ModeIfPicked)),
            ids::TRADER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::GainPerCost { card: id::SILVER })),
            ids::WEAVER => g.stack.push(mode_frame(p, card, 1)),
            ids::BERSERKER => {
                // Gain a card costing less than this, then the other players discard down to 3 (they may include the turn's player: this can be
                // played on another player's turn, when it is gained there).
                let (vs, n) = victims_of(g, p);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::Nothing) });
                }
                g.stack.push(Frame { less: true, ..gain_frame(p, card, g.cost(card), Filter::Any, Dest::Discard) });
            }
            ids::CARTOGRAPHER => {
                // Look at the top 4; discard any, put the rest back in any order.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 0, ALL, Then::Nothing));
                g.stack.push(Frame { max: 4, ..Frame::new(K::RevealTop, p, card) });
            }
            ids::CAULDRON => {
                g.turn.hint.cauldrons = g.turn.hint.cauldrons.saturating_add(1);
                // Players who were unaffected by this Attack (a Moat revealed, a Lighthouse in play) get none of its Curses.
                let (vs, n) = g.victims(sink);
                for v in g.others(p) {
                    if !vs[..n].contains(&v) {
                        g.turn.hint.caul_shield |= 1 << v;
                    }
                }
            }
            ids::HIGHWAY => g.turn.cost_reduction = g.turn.cost_reduction.saturating_add(1),
            ids::INN => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 2, 2, Then::Nothing)),
            ids::MARGRAVE => {
                // Each other player draws a card, then discards down to 3 (the draw is pushed above the discard, per victim, in turn order).
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::Nothing) });
                    g.stack.push(draw_frame(v, card, 1));
                }
            }
            ids::SOUK => {
                // +$7, then -$1 per card in the hand, not below $0.
                let before = g.turn.coins;
                let after = (before + 7).saturating_sub(g.players[pi].hand.total() as u16);
                g.turn.coins = after;
                if after > before {
                    sink.bonus(p, card, 0, 0, after - before, 0);
                }
            }
            ids::STABLES => {
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Treasure, 0, 1, Then::DrawActionsIfPicked { draw: 3, actions: 1 }))
            }
            ids::WHEELWRIGHT => g.stack.push(select(
                p, card, Zone::Hand, Discard, Filter::Any, 0, 1,
                Then::GainUpTo { plus: 0, filter: Filter::Action, dest: Dest::Discard, exact: false, dest_by_type: false },
            )),
            ids::WITCHS_HUT => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 2, 2, Then::WitchsHut)),
            _ => {}
        }
    }

    #[inline(never)]
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, on_buy: bool, sink: &mut S) {
        let own_turn = p == g.turn.player;
        // Fool's Gold: another player's Province gained; each other player may trash a Fool's Gold from their hand for a Gold onto their deck
        // (asked after the gainer's own effects, in turn order from the gainer's left).
        if c == id::PROVINCE && g.in_supply(ids::FOOLS_GOLD) {
            let mut qs = [0u8; MAX_PLAYERS];
            let mut n = 0;
            for q in g.others(p) {
                qs[n] = q;
                n += 1;
            }
            for &q in qs[..n].iter().rev() {
                for _ in 0..g.players[q as usize].hand.get(ids::FOOLS_GOLD).min(4) {
                    g.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Trash, subject: ids::FOOLS_GOLD, then: Then::FoolsGoldGain,
                        ..Frame::new(K::YesNo, q, ids::FOOLS_GOLD)
                    });
                }
            }
        }
        // Haggler: for each Haggler played this turn, a cheaper non-Victory card for a card bought.
        if on_buy && own_turn && g.in_supply(ids::HAGGLER) {
            for _ in 0..g.turn.played.get(ids::HAGGLER) {
                g.stack.push(Frame {
                    less: true, potion: cards::potion_cost(c), ..gain_frame(p, ids::HAGGLER, g.cost(c), Filter::NonVictory, Dest::Discard)
                });
            }
        }
        // Cauldron: the third Action the turn's player gains this turn gives each other player a Curse for every Cauldron played before it.
        if own_turn && g.in_supply(ids::CAULDRON) && cards::is(c, ACTION) && g.turn.hint.acts_gained < 3 {
            g.turn.hint.acts_gained += 1;
            if g.turn.hint.acts_gained == 3 {
                let (vs, n) = g.victims_unshielded(g.turn.hint.caul_shield);
                for _ in 0..g.turn.hint.cauldrons {
                    g.push_gain_to_each(&vs[..n], id::CURSE, ids::CAULDRON);
                }
            }
        }
        // The card's own "when you gain this".
        match c {
            ids::NOMADS if own_turn => {
                g.turn.coins += 2;
                sink.bonus(p, c, 0, 0, 2, 0);
            }
            ids::BORDER_VILLAGE => {
                // A cheaper card than this (at its current cost).
                g.stack.push(Frame { less: true, ..gain_frame(p, c, g.cost(c), Filter::Any, Dest::Discard) });
            }
            ids::FARMLAND => g.stack.push(select(
                p, c, Zone::Hand, Act::Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 2, filter: Filter::NotCard(ids::FARMLAND), dest: Dest::Discard, exact: true, dest_by_type: false },
            )),
            ids::INN => g.stack.push(select(p, c, Zone::Discard, Act::ShuffleIn, Filter::Action, 0, ALL, Then::InnShuffle)),
            ids::SOUK => g.stack.push(select(p, c, Zone::Hand, Act::Trash, Filter::Any, 0, 2, Then::Nothing)),
            _ => {}
        }
        let zone = GameState::gain_dest_zone(to);
        // Berserker: played when gained with an Action in play (checked again when the frame runs).
        if c == ids::BERSERKER && g.players[p as usize].in_play.any_type(ACTION) {
            g.stack.push(Frame { zone, act: Act::Play, subject: c, then: Then::BerserkerPlay, ..Frame::new(K::YesNo, p, c) });
        }
        // Trail: may be played when gained (not in Clean-up).
        if c == ids::TRAIL && g.turn.phase != Phase::CleanupDraw {
            g.stack.push(Frame { zone, act: Act::Play, subject: c, ..Frame::new(K::YesNo, p, c) });
        }
        // Trader: a card gained may be exchanged for a Silver by revealing a Trader (asked first). A Silver is never worth exchanging.
        if g.in_supply(ids::TRADER) && c != id::SILVER && g.players[p as usize].hand.has(ids::TRADER) && g.supply.get(id::SILVER) > 0
            && (g.in_supply(c) || g.in_supply(cards::pile_of(c)))
        {
            g.stack.push(Frame { zone, act: Act::Return, subject: c, then: Then::TraderSwap, ..Frame::new(K::YesNo, p, ids::TRADER) });
        }
    }

    /// Nomads: +$2 when trashed. Trail: may be played when trashed (back from the trash).
    #[inline(never)]
    fn on_trash<S: EventSink>(g: &mut GameState, p: u8, card: CardId, sink: &mut S) {
        match card {
            ids::NOMADS if p == g.turn.player => {
                g.turn.coins += 2;
                sink.bonus(p, card, 0, 0, 2, 0);
            }
            ids::TRAIL if g.turn.phase != Phase::CleanupDraw => {
                g.stack.push(Frame { zone: Zone::Trash, act: Act::Play, subject: card, ..Frame::new(K::YesNo, p, card) });
            }
            _ => {}
        }
    }

    /// Scheme: for each one played this turn, an Action card may be put onto the deck instead of being discarded from play.
    #[inline(never)]
    fn offers_on_discard(g: &mut GameState) {
        if g.in_supply(ids::SCHEME) {
            let n = g.turn.played.get(ids::SCHEME);
            if n > 0 {
                let p = g.turn.player;
                g.stack.push(Frame {
                    ordered: true,
                    ..select(p, ids::SCHEME, Zone::InPlay, Act::Topdeck, Filter::ActionToDiscard, 0, n, Then::Nothing)
                });
            }
        }
    }
}

/// The other players `attacker` hits with an Attack (not immune), in turn order from their left. Unlike `GameState::victims` the attacker
/// need not be the turn's player (a Berserker gained on another player's turn).
fn victims_of(g: &GameState, attacker: u8) -> ([u8; MAX_PLAYERS], usize) {
    let mut out = [0u8; MAX_PLAYERS];
    let mut n = 0;
    for v in g.others(attacker) {
        if !g.immune(v) {
            out[n] = v;
            n += 1;
        }
    }
    (out, n)
}

/// Setup: discards are watched when a card in the game reacts to being discarded.
pub(crate) fn setup(g: &mut GameState) {
    let reacts = |c: CardId| g.in_supply(c) || g.ferryman == c;
    if reacts(ids::TUNNEL) || reacts(ids::TRAIL) || reacts(ids::WEAVER) || g.noc.flags & N_HOUND != 0 {
        g.adv |= G_DISCARD_HOOK;
    }
}

impl GameState {
    /// The turn's other players that a Cauldron's Curse can hit: not those in `shield` (unaffected when it was played).
    fn victims_unshielded(&self, shield: u8) -> ([u8; MAX_PLAYERS], usize) {
        let mut out = [0u8; MAX_PLAYERS];
        let mut n = 0;
        for v in self.others(self.turn.player) {
            if shield & (1 << v) == 0 {
                out[n] = v;
                n += 1;
            }
        }
        (out, n)
    }

    /// `n` copies of `c` were just discarded (not in Clean-up) by `p`: a Faithful Hound may be set aside at once, and a Tunnel, Trail or Weaver
    /// waits in the queue for its reaction until everything discarded together is in the discard pile (`flush_trash_triggers`).
    #[inline(never)]
    pub(crate) fn discard_hook(&mut self, p: u8, c: CardId, n: u8) {
        if self.turn.phase == Phase::CleanupDraw {
            return;
        }
        match c {
            id::FAITHFUL_HOUND => {
                if self.noc.flags & N_HOUND != 0 {
                    self.hounds_discarded(p, n);
                }
            }
            ids::TUNNEL | ids::TRAIL | ids::WEAVER => {
                for _ in 0..n {
                    let q = &mut self.trash_queue;
                    if (q.len as usize) < TRASH_QUEUE_CAP {
                        q.items[q.len as usize] = (p, c, Q_DISCARDED);
                        q.len += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// `discard_hook` for a pile of cards discarded together, then the reactions they bring are put on the stack.
    #[inline(never)]
    pub(crate) fn discard_hook_counts<S: EventSink>(&mut self, p: u8, discarded: &Counts, depth: u8, sink: &mut S) {
        for c in [id::FAITHFUL_HOUND, ids::TUNNEL, ids::TRAIL, ids::WEAVER] {
            let n = discarded.get(c);
            if n > 0 {
                self.discard_hook(p, c, n);
            }
        }
        self.flush_trash_triggers(depth, sink);
    }

    /// The reaction of a discarded Tunnel (reveal it for a Gold), Trail or Weaver (play it from the discard pile).
    #[inline(never)]
    pub(crate) fn discard_reaction(&mut self, p: u8, c: CardId) {
        match c {
            ids::TUNNEL => self.stack.push(Frame {
                zone: Zone::Discard, act: Act::Reveal, subject: c, then: Then::TunnelGold, ..Frame::new(K::YesNo, p, c)
            }),
            ids::TRAIL | ids::WEAVER => self.stack.push(Frame { zone: Zone::Discard, act: Act::Play, subject: c, ..Frame::new(K::YesNo, p, c) }),
            _ => {}
        }
    }

    /// Whether a Trader exchange (`TraderSwap`) can still be made, or a Berserker (`BerserkerPlay`) is still to be played, when its frame runs: the
    /// Trader is still in the hand and there is a Silver to take; an Action is still in play.
    #[inline(never)]
    pub(crate) fn hint_yesno_possible(&self, f: &Frame) -> bool {
        let ps = &self.players[f.player as usize];
        match f.then {
            Then::TraderSwap => ps.hand.has(ids::TRADER) && (self.supply.get(id::SILVER) > 0 || f.subject == id::SILVER),
            _ => ps.in_play.any_type(ACTION),
        }
    }

    /// The `then` of a Hinterlands reaction answered Yes (after the card was moved by the frame's `act`).
    #[inline(never)]
    pub(crate) fn hint_yes<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            Then::TunnelGold => {
                self.gain(p, id::GOLD, Dest::Discard, false, ids::TUNNEL, sink);
            }
            Then::FoolsGoldGain => {
                self.gain(p, id::GOLD, Dest::DeckTop, false, ids::FOOLS_GOLD, sink);
            }
            // The card gained (`subject`) went back to its pile; a Silver comes to the discard pile instead (an exchange is not a gain).
            Then::TraderSwap => {
                sink.event(Event::Reaction { player: p, card: ids::TRADER });
                if self.supply.remove(id::SILVER) {
                    self.players[p as usize].discard.add(id::SILVER, 1);
                    sink.event(Event::Exchange { player: p, from: f.subject, to: id::SILVER });
                }
            }
            _ => {}
        }
    }

    /// The `then` of a finished `Select` that belongs to Hinterlands.
    #[inline(never)]
    pub(crate) fn finish_hint_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match f.then {
            // Inn: the Actions picked are in the deck's unknown part; the whole deck (what was known of its order too) is shuffled.
            Then::InnShuffle => {
                let ps = &mut self.players[pi];
                let known = ps.deck_known.counts();
                ps.deck_unknown.add_all(&known);
                ps.deck_known.clear();
                let bottom = ps.deck_known_bottom.counts();
                ps.deck_unknown.add_all(&bottom);
                ps.deck_known_bottom.clear();
                sink.event(Event::Shuffle { player: p });
            }
            Then::DrawUpTo(n) => {
                let have = self.players[pi].hand.total();
                if have < n as u32 {
                    self.stack.push(draw_frame(p, f.source, (n as u32 - have) as u8));
                }
            }
            Then::DrawIfHandAtMost { max_hand, draw } => {
                if self.players[pi].hand.total() <= max_hand as u32 {
                    self.stack.push(draw_frame(p, f.source, draw));
                }
            }
            Then::ModeIfPicked => {
                if f.count > 0 {
                    self.stack.push(mode_frame(p, f.source, 1));
                }
            }
            Then::WitchsHut => {
                if f.count == 2 && f.subject == 2 {
                    let (vs, n) = self.victims(sink);
                    self.push_gain_to_each(&vs[..n], id::CURSE, f.source);
                }
            }
            // Develop: a card costing exactly $1 more and one costing exactly $1 less, onto the deck, in either order (one choice offers both costs).
            Then::DevelopGain => {
                if f.count > 0 {
                    let cost = self.cost(f.last);
                    let potion = cards::potion_cost(f.last);
                    if cost == 0 {
                        self.stack.push(Frame { exact: true, potion, ..gain_frame(p, f.source, 1, Filter::Any, Dest::DeckTop) });
                    } else {
                        self.stack.push(Frame {
                            exact: true, potion, gf: crate::state::GF_BOTH, ..gain_frame(p, f.source, cost - 1, Filter::Any, Dest::DeckTop)
                        });
                    }
                }
            }
            // Trader: a Silver per $1 the card trashed cost.
            Then::GainPerCost { card } => {
                if f.count > 0 {
                    for _ in 0..self.cost(f.last) {
                        if !self.gain(p, card, Dest::Discard, false, f.source, sink) {
                            break;
                        }
                    }
                }
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
        let free = [ids::FOOLS_GOLD, ids::NOMADS, ids::TRAIL, ids::HAGGLER, ids::HIGHWAY, ids::MARGRAVE, ids::BORDER_VILLAGE];
        let order = [
            ids::CROSSROADS, ids::DEVELOP, ids::GUARD_DOG, ids::OASIS, ids::JACK_OF_ALL_TRADES, ids::TRADER, ids::CARTOGRAPHER, ids::INN,
            ids::SOUK, ids::WHEELWRIGHT, ids::WITCHS_HUT,
        ];
        let choice = [ids::SCHEME, ids::SPICE_MERCHANT, ids::WEAVER, ids::BERSERKER, ids::CAULDRON, ids::STABLES];
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
        assert_eq!(CARDS.len(), 26);
    }
}
