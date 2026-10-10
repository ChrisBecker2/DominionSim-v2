//! Cornucopia & Guilds, 2nd edition: Coffers, overpay, the Rewards (gained only by Joust), Ferryman's pile and Young Witch's Bane.

use super::prelude::*;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const CANDLESTICK_MAKER: CardId = 129;
    pub const HAMLET: CardId = 130;
    pub const FARRIER: CardId = 131;
    pub const STONEMASON: CardId = 132;
    pub const MENAGERIE: CardId = 133;
    pub const SHOP: CardId = 134;
    pub const INFIRMARY: CardId = 135;
    pub const ADVISOR: CardId = 136;
    pub const FARMHANDS: CardId = 137;
    pub const PLAZA: CardId = 138;
    pub const REMAKE: CardId = 139;
    pub const YOUNG_WITCH: CardId = 140;
    pub const HERALD: CardId = 141;
    pub const BAKER: CardId = 142;
    pub const BUTCHER: CardId = 143;
    pub const CARNIVAL: CardId = 144;
    pub const FERRYMAN: CardId = 145;
    pub const FOOTPAD: CardId = 146;
    pub const HORN_OF_PLENTY: CardId = 147;
    pub const HUNTING_PARTY: CardId = 148;
    pub const JESTER: CardId = 149;
    pub const JOURNEYMAN: CardId = 150;
    pub const JOUST: CardId = 151;
    pub const MERCHANT_GUILD: CardId = 152;
    pub const SOOTHSAYER: CardId = 153;
    pub const FAIRGROUNDS: CardId = 154;
    pub const CORONET: CardId = 155;
    pub const COURSER: CardId = 156;
    pub const DEMESNE: CardId = 157;
    pub const HOUSECARL: CardId = 158;
    pub const HUGE_TURNIP: CardId = 159;
    pub const RENOWN: CardId = 160;
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::CANDLESTICK_MAKER, cg(choice_free(c("Candlestick Maker", 2, ACTION, 0, 0, 0, 1, 1)))), // +1 Coffers
    (ids::HAMLET, cg(order_sensitive(c("Hamlet", 2, ACTION, 0, 0, 1, 1, 0)))), // discard for +Action / +Buy (may be the +Actions card in hand)
    (ids::FARRIER, cg(choice_free(c("Farrier", 2, ACTION, 0, 0, 1, 1, 1)))), // overpay is at the buy, not a play decision
    (ids::STONEMASON, cg(order_sensitive(c("Stonemason", 2, ACTION, 0, 0, 0, 0, 0)))), // what to trash and gain (may be the +Actions card in hand)
    (ids::MENAGERIE, cg(order_sensitive(c("Menagerie", 3, ACTION, 0, 0, 0, 1, 0)))), // reads the hand: cards drawn first can make a duplicate
    (ids::SHOP, cg(order_sensitive(c("Shop", 3, ACTION, 1, 0, 1, 0, 0)))), // plays an Action from the hand for free: better before spending an Action on it
    (ids::INFIRMARY, cg(order_sensitive(c("Infirmary", 3, ACTION, 0, 0, 1, 0, 0)))), // what to trash (may be the +Actions card in hand)
    (ids::ADVISOR, cg(order_sensitive(c("Advisor", 4, ACTION, 0, 0, 0, 1, 0)))), // reveals the top 3 of the deck, an opponent discards one
    (ids::FARMHANDS, cg(choice_free(c("Farmhands", 4, ACTION, 0, 0, 1, 2, 0)))), // the set-aside is on gain, not on play
    (ids::PLAZA, cg(order_sensitive(c("Plaza", 4, ACTION, 0, 0, 1, 2, 0)))), // may discard a Treasure
    (ids::REMAKE, cg(order_sensitive(c("Remake", 4, ACTION, 0, 0, 0, 0, 0)))), // what to trash and gain, twice (may be the +Actions card in hand)
    (ids::YOUNG_WITCH, cg(order_sensitive(c("Young Witch", 4, ACTION | ATTACK, 0, 0, 2, 0, 0)))), // what to discard (may be the +Actions card in hand)
    (ids::HERALD, cg(order_sensitive(c("Herald", 4, ACTION, 0, 0, 1, 1, 0)))), // reveals the top card and plays it
    (ids::BAKER, cg(choice_free(c("Baker", 5, ACTION, 0, 0, 1, 1, 0)))), // +1 Coffers
    (ids::BUTCHER, cg(order_sensitive(c("Butcher", 5, ACTION, 0, 0, 0, 0, 0)))), // what to trash and gain, Coffers spent (may be the +Actions card in hand)
    (ids::CARNIVAL, cg(order_sensitive(c("Carnival", 5, ACTION, 0, 0, 0, 0, 0)))), // reveals the top 4 of the deck
    (ids::FERRYMAN, cg(order_sensitive(c("Ferryman", 5, ACTION, 0, 0, 2, 1, 0)))), // what to discard (may be the +Actions card in hand)
    (ids::FOOTPAD, cg(choice_free(c("Footpad", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // +2 Coffers; only victims act
    (ids::HORN_OF_PLENTY, cg(has_choice(c("Horn of Plenty", 5, TREASURE, 0, 0, 0, 0, 0)))), // gains a card; best played after the other Treasures
    (ids::HUNTING_PARTY, cg(order_sensitive(c("Hunting Party", 5, ACTION, 0, 0, 1, 1, 0)))), // reveals the hand and the top of the deck
    (ids::JESTER, cg(has_choice(c("Jester", 5, ACTION | ATTACK, 2, 0, 0, 0, 0)))), // who gains the copy
    (ids::JOURNEYMAN, cg(order_sensitive(c("Journeyman", 5, ACTION, 0, 0, 0, 0, 0)))), // names a card and reveals from the deck
    (ids::JOUST, cg(has_choice(c("Joust", 5, ACTION, 1, 0, 1, 1, 0)))), // may set aside a Province for a Reward
    (ids::MERCHANT_GUILD, cg(choice_free(c("Merchant Guild", 5, ACTION, 1, 0, 0, 0, 1)))), // the Coffers come at the end of the Buy phase
    (ids::SOOTHSAYER, cg(has_choice(c("Soothsayer", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // gains a Gold (a reshuffle can draw it)
    (ids::FAIRGROUNDS, cg(c("Fairgrounds", 6, VICTORY, 0, 0, 0, 0, 0))), // 2 VP per 5 differently named cards (`state::vp_of_cards`)
    (ids::CORONET, cg(has_choice(c("Coronet", 0, ACTION | TREASURE | REWARD, 0, 0, 0, 0, 0)))), // which Action and Treasure to play twice
    (ids::COURSER, cg(has_choice(c("Courser", 0, ACTION | REWARD, 0, 0, 0, 0, 0)))), // two of four options
    (ids::DEMESNE, cg(has_choice(c("Demesne", 0, ACTION | VICTORY | REWARD, 0, 0, 0, 2, 2)))), // gains a Gold (a reshuffle can draw it)
    (ids::HOUSECARL, cg(has_choice(c("Housecarl", 0, ACTION | REWARD, 0, 0, 0, 0, 0)))), // draws per Action in play: best after the +Actions cards
    (ids::HUGE_TURNIP, cg(choice_free(c("Huge Turnip", 0, TREASURE | REWARD, 0, 0, 0, 0, 0)))), // +2 Coffers, +$1 per Coffers
    (ids::RENOWN, cg(choice_free(c("Renown", 0, ACTION | REWARD, 0, 0, 0, 0, 1)))), // cards cost $2 less
];

/// The Cornucopia & Guilds cards (see the module docs of `sets`).
pub struct CornucopiaGuilds;

impl SetRules for CornucopiaGuilds {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    const GAIN_HI: u128 = (1u128 << (id::FOOTPAD - 128)) | (1u128 << (id::FARMHANDS - 128)) | (1u128 << (id::FERRYMAN - 128));

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            id::JESTER => &[ModeOpt::GainSubject { to_chooser: false }, ModeOpt::GainSubject { to_chooser: true }],
            id::COURSER => &[ModeOpt::Cards(2), ModeOpt::Actions(2), ModeOpt::Coins(2), ModeOpt::GainMany(id::SILVER, Dest::Discard, 4)],
            _ => &[],
        }
    }

    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, id::FARMHANDS)
    }

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        match card {
            id::CANDLESTICK_MAKER | id::BAKER => g.give_coffers(p, card, 1, sink),
            id::HAMLET => {
                // Two independent optional discards, the +Action one first (pushed last, so on top).
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 1, Then::BonusIfPicked { actions: 0, buys: 1 }));
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, 1, Then::BonusIfPicked { actions: 1, buys: 0 }));
            }
            id::MENAGERIE => {
                // Reveal the hand: +3 Cards if every name is different (an empty hand counts), else +1.
                let hand = g.players[p as usize].hand;
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                }
                let all_different = hand.iter().count() as u32 == hand.total();
                g.stack.push(draw_frame(p, card, if all_different { 3 } else { 1 }));
            }
            id::SHOP => g.stack.push(select(p, card, Zone::Hand, Play, Filter::NotInPlayAction, 0, 1, Then::PlayPicked { times: 1 })),
            id::PLAZA => g.stack.push(select(p, card, Zone::Hand, Discard, g.treasure_filter(), 0, 1, Then::CoffersPerPick(1))),
            id::REMAKE => {
                // Do this twice, one trash and gain at a time (the gain sits above the second trash).
                for _ in 0..2 {
                    g.stack.push(select(
                        p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                        Then::GainUpTo { plus: 1, filter: Filter::Any, dest: Dest::Discard, exact: true, dest_by_type: false },
                    ));
                }
            }
            id::MERCHANT_GUILD => g.turn.guilds = g.turn.guilds.saturating_add(1),
            id::HUNTING_PARTY => {
                // Reveal until a card that is not a copy of one in the hand (the hand as it is once
                // the +1 Card has been drawn: the draw is pushed last and resolves first).
                g.stack.push(Frame {
                    filter: Filter::NotInHand, max: 1, then: Then::KeepMatchingDiscardRest,
                    ..Frame::new(K::RevealUntil, p, card)
                });
            }
            id::CARNIVAL => g.stack.push(Frame { max: 4, then: Then::OneOfEachToHand, ..Frame::new(K::RevealTop, p, card) }),
            id::ADVISOR => {
                // Reveal the top 3 (Envoy: 5); the player to the left picks one to discard; the rest to my hand.
                let left = (p + 1) % g.num_players;
                g.stack.push(Frame { chooser: left, ..select(p, card, Zone::Revealed, Discard, Filter::Any, 1, 1, Then::RevealedToHand) });
                g.stack.push(Frame { max: 3, ..Frame::new(K::RevealTop, p, card) });
            }
            id::FOOTPAD => {
                g.give_coffers(p, card, 2, sink);
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::Nothing) });
                }
            }
            id::SOOTHSAYER => {
                // Gain a Gold, then each other player in turn gains a Curse and draws a card if they did.
                g.gain(p, id::GOLD, Dest::Discard, false, card, sink);
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame {
                        then: Then::DrawIfGained,
                        ..gain_frame(v, card, u8::MAX, Filter::Card(id::CURSE), Dest::Discard)
                    });
                }
            }
            id::JESTER => {
                // Each other player discards the top card of their deck (in turn order); I choose what follows.
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { chooser: p, act: Discard, then: Then::JesterGain, ..Frame::new(K::TopCard, v, card) });
                }
            }
            id::JOURNEYMAN => g.stack.push(Frame { then: Then::JourneymanDraw, ..Frame::new(K::Name, p, card) }),
            id::STONEMASON => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::GainLess { count: 2 })),
            id::INFIRMARY => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing)),
            id::FERRYMAN => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 1, 1, Then::Nothing)),
            id::HERALD => {
                // Reveal the top card: an Action is played (mandatory, no Action used); anything else goes back.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Play, Filter::Action, 1, 1, Then::PlayPicked { times: 1 }));
                g.stack.push(Frame { max: 1, ..Frame::new(K::RevealTop, p, card) });
            }
            id::YOUNG_WITCH => {
                // +2 Cards, discard 2, then each other player gains a Curse unless they reveal a Bane.
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { gf: crate::state::GF_BANE_BLOCKS, ..gain_frame(v, card, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
                }
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 2, 2, Then::Nothing));
            }
            id::JOUST => g.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Card(id::PROVINCE), 0, 1, Then::JoustReward)),
            id::CORONET => {
                // A non-Reward Action twice, then a non-Reward Treasure twice (the Action is pushed last, so it comes first).
                g.stack.push(select(p, card, Zone::Hand, Play, Filter::TreasureNotReward, 0, 1, Then::PlayPicked { times: 2 }));
                g.stack.push(select(p, card, Zone::Hand, Play, Filter::ActionNotReward, 0, 1, Then::PlayPicked { times: 2 }));
            }
            id::COURSER => g.stack.push(mode_frame(p, card, 2)),
            id::DEMESNE => {
                g.gain(p, id::GOLD, Dest::Discard, false, card, sink);
            }
            id::HOUSECARL => {
                let names = g.players[p as usize].in_play.iter().filter(|&(c, _)| cards::is(c, ACTION)).count() as u8;
                if names > 0 {
                    g.stack.push(draw_frame(p, card, names));
                }
            }
            id::HUGE_TURNIP => {
                g.give_coffers(p, card, 2, sink);
                let n = g.players[p as usize].coffers.min(u16::MAX);
                g.turn.coins += n;
                sink.bonus(p, card, 0, 0, n, 0);
            }
            id::RENOWN => g.turn.cost_reduction = g.turn.cost_reduction.saturating_add(2),
            id::BUTCHER => {
                g.give_coffers(p, card, 2, sink);
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::GainWithCoffers));
            }
            id::HORN_OF_PLENTY => {
                // Up to $1 per differently named card in play, this one included.
                let names = g.players[p as usize].in_play.iter().count() as u8;
                g.stack.push(Frame { then: Then::HornTrash, ..gain_frame(p, card, names, Filter::Any, Dest::Discard) });
            }
            _ => {}
        }
    }

    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        match card {
            // 2 VP per 5 differently named cards; Demesne: 1 VP per Gold.
            id::FAIRGROUNDS => 2 * (all.iter().count() as i32 / 5),
            id::DEMESNE => all.get(id::GOLD) as i32,
            _ => cards::def(card).vp as i32,
        }
    }

    fn start_entry_named_by_arg(card: CardId) -> bool {
        card == id::FARMHANDS
    }

    /// Farmhands: each card set aside is played at the start of this turn.
    fn turn_start_entries(g: &mut GameState) {
        let pi = g.turn.player as usize;
        // Farmhands: each card set aside is played at the start of this turn.
        for (c, k) in g.players[pi].farm_cards.iter() {
            for _ in 0..k {
                g.turn.start_pending.push(PendingDuration { card: id::FARMHANDS, times: 1, arg: c, used: 0 });
            }
        }
    }

    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
        if e.card != id::FARMHANDS {
            return false;
        }
        // The card set aside when the Farmhands was gained is played now, from set aside into play
        // (no Action used), then it is an ordinary card in play.
        if g.players[p as usize].farm_cards.remove(e.arg) {
            g.players[p as usize].in_play.add(e.arg, 1);
            sink.depth(0);
            sink.event(Event::StartPlay { player: p, source: id::FARMHANDS, card: e.arg });
            g.resolve_effects(e.arg, p, 0, sink);
        }
        true
    }

    /// Merchant Guild: +1 Coffers per card gained this Buy phase, per play (at the end of the Buy phase).
    fn end_of_buy<S: EventSink>(g: &mut GameState, sink: &mut S) {
        if g.turn.guilds > 0 {
            let n = g.turn.guilds as u16 * g.turn.gained_in_buy as u16;
            if n > 0 {
                let p = g.turn.player;
                g.players[p as usize].coffers += n;
                sink.depth(0);
                sink.event(Event::Coffers { player: p, source: id::MERCHANT_GUILD, n });
            }
        }
    }

    /// Footpad's draw (any player gaining a card in an Action phase, in games using it), Ferryman's copy of its pile's
    /// card and Farmhands' set-aside.
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, _to: Dest, _on_buy: bool, sink: &mut S) {
        if c == id::FERRYMAN && g.ferryman != 0 {
            g.gain_side(p, g.ferryman, Dest::Discard, id::FERRYMAN, sink);
        }
        // Farmhands: may set aside an Action or Treasure from the hand to play at the start of the next
        // turn (below Footpad's draw, so a card it draws can be the one set aside).
        if c == id::FARMHANDS {
            g.stack.push(Frame {
                zone: Zone::Hand, act: Act::SetAside, filter: Filter::ActionOrTreasure, min: 0, max: 1,
                then: Then::HoldUntilNextTurn,
                ..Frame::new(K::Select, p, id::FARMHANDS)
            });
        }
        if g.in_supply(id::FOOTPAD) && g.turn.phase == Phase::Action {
            g.stack.push(draw_frame(p, id::FOOTPAD, 1));
        }
    }

    /// Ferryman's pile is in the game, so a gain of its card needs the trigger even with no Ferryman mask bit set.
    #[inline(always)]
    fn gain_live_extra(g: &GameState) -> bool {
        g.ferryman != 0
    }

    fn has_overpay(card: CardId) -> bool {
        matches!(card, id::FARRIER | id::STONEMASON | id::INFIRMARY | id::HERALD)
    }

    fn overpay_with_potion(g: &GameState, card: CardId) -> bool {
        card == id::STONEMASON && g.turn.potions > 0
    }

    fn overpay_possible(g: &GameState, card: CardId) -> bool {
        Self::has_overpay(card) && (g.turn.coins > 0 || Self::overpay_with_potion(g, card))
    }

    /// The card `card` was just bought and overpaid (`coins`, `potion`): push its overpay ability (below the frames its gain
    /// triggers push, so they are asked first: the ability is a when-gain effect like the others).
    fn overpay<S: EventSink>(g: &mut GameState, p: u8, card: CardId, coins: u8, potion: bool, depth: u8, _sink: &mut S) {
        match card {
            id::FARRIER => g.turn.extra_draw = g.turn.extra_draw.saturating_add(coins),
            // Two Actions each costing exactly the amount overpaid (Potion included), one at a time.
            id::STONEMASON => {
                for _ in 0..2 {
                    g.stack.push(Frame {
                        exact: true, potion, depth,
                        ..gain_frame(p, card, coins, Filter::Action, Dest::Discard)
                    });
                }
            }
            // Per $1: a card from the discard pile onto the deck, in the order chosen.
            id::HERALD if coins > 0 => g.stack.push(Frame {
                ordered: true, depth,
                ..select(p, card, Zone::Discard, Act::Topdeck, Filter::Any, coins, coins, Then::Nothing)
            }),
            // Played once per $1, from where it is (the discard pile), not moving.
            id::INFIRMARY => {
                for nth in (1..=coins).rev() {
                    g.stack.push(Frame {
                        zone: Zone::Discard, subject: card, count: nth, depth,
                        ..Frame::new(K::PlayEffects, p, card)
                    });
                }
            }
            _ => {}
        }
    }
}

use crate::cards::{is_kingdom, kingdom_cards, pile_eligible, set_of};
use crate::state::GameConfig;

impl GameState {
    /// `p` adds `n` Coffers tokens to their mat (`source` gave them).
    pub(crate) fn give_coffers<S: EventSink>(&mut self, p: u8, source: CardId, n: u16, sink: &mut S) {
        self.players[p as usize].coffers += n;
        sink.event(Event::Coffers { player: p, source, n });
    }

    /// The cards Journeyman may name: what is in `p`'s deck and discard pile (the cards a reveal can
    /// meet) and the Supply piles (so there is always a card that is in neither: naming one of those is
    /// naming nothing). Any other name is the same as naming nothing.
    pub(crate) fn journeyman_offer(&self, p: u8) -> Counts {
        let ps = &self.players[p as usize];
        let mut c = ps.deck_counts();
        c.add_all(&ps.discard);
        for sc in self.supply_cards() {
            if !c.has(sc) {
                c.set(sc, 1);
            }
        }
        c
    }
}

/// A pick among the unused playable Kingdom cards costing `lo..=hi` that depends only on the
/// kingdom (not on a game's seed, and not on the order it is listed in), so a fixed kingdom has
/// the same set-up piles in every game of a batch. Eligible cards come from the sets the kingdom
/// draws on (plus Cornucopia & Guilds, whose cards ask for them), never a card already in the
/// kingdom, `taken`, or one that needs a set-up pile of its own (Young Witch, Ferryman).
/// Allocation-free: `GameState::new` runs once per simulated game.
fn default_setup_pile(kingdom: &[CardId], taken: CardId, salt: u64, lo: u8, hi: u8) -> Option<CardId> {
    let in_sets = |c: CardId| {
        set_of(c) == CardSet::CornucopiaGuilds || kingdom.iter().any(|&k| is_kingdom(k) && set_of(k) == set_of(c))
    };
    let eligible = |c: CardId| {
        pile_eligible(c, lo, hi) && in_sets(c) && !kingdom.contains(&c) && c != taken && !matches!(c, id::YOUNG_WITCH | id::FERRYMAN)
    };
    let n = kingdom_cards().filter(|&c| eligible(c)).count();
    if n == 0 {
        return None;
    }
    let mut h: u64 = salt;
    for &c in kingdom {
        let mut x = (c as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 29;
        h = h.wrapping_add(x.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    }
    kingdom_cards().filter(|&c| eligible(c)).nth((h >> 20) as usize % n)
}

/// Young Witch's Bane when the game does not name one: a $2 or $3 Kingdom card not in `kingdom`
/// and not `ferryman` (0 = none).
pub fn default_bane(kingdom: &[CardId], ferryman: CardId) -> Option<CardId> {
    default_setup_pile(kingdom, ferryman, 0xBA, 2, 3)
}

/// Ferryman's pile when the game does not name one: a $3 or $4 Kingdom card not in `kingdom` and
/// not `bane` (0 = none).
pub fn default_ferryman(kingdom: &[CardId], bane: CardId) -> Option<CardId> {
    default_setup_pile(kingdom, bane, 0xFE, 3, 4)
}

/// What a game with Cornucopia & Guilds cards starts with: Ferryman's pile and Young Witch's Bane (the card of each pile,
/// 0 for none), the Rewards, and Baker's Coffers.
pub(crate) struct Setup {
    pub ferryman: CardId,
    pub bane: CardId,
    pub rewards: [u8; cards::NUM_REWARDS],
    pub starting_coffers: u16,
}

/// Set-up for `cfg`: Ferryman's pile (outside the Supply) and Young Witch's Bane (an extra Supply pile; needed too when
/// the Ferryman pile is Young Witch), 1 or 2 of each Reward with Joust, and Baker's +1 Coffers for each player (also when
/// the Baker is the Bane or Ferryman card).
pub(crate) fn setup(cfg: &GameConfig, num_players: usize) -> Setup {
    let has = |c: CardId| cfg.kingdom.contains(&c);
    let ferryman = if has(id::FERRYMAN) {
        default_ferryman_pile(cfg).unwrap_or(0)
    } else {
        0
    };
    let bane = if has(id::YOUNG_WITCH) || ferryman == id::YOUNG_WITCH {
        cfg.bane.or_else(|| default_bane(&cfg.kingdom, ferryman)).unwrap_or(0)
    } else {
        0
    };
    let mut rewards = [0u8; cards::NUM_REWARDS];
    if has(id::JOUST) {
        rewards = [if num_players == 2 { 1 } else { 2 }; cards::NUM_REWARDS];
    }
    let baker = has(id::BAKER) || bane == id::BAKER || ferryman == id::BAKER;
    Setup { ferryman, bane, rewards, starting_coffers: baker as u16 }
}

fn default_ferryman_pile(cfg: &GameConfig) -> Option<CardId> {
    cfg.ferryman.or_else(|| default_ferryman(&cfg.kingdom, cfg.bane.unwrap_or(0)))
}

/// Frame handlers of this set's cards (the generic frames' card-specific parts).
impl GameState {
    pub(crate) fn run_jester_top_card<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        match take_top!(self, p, sink) {
            None => {
                self.stack.pop();
            }
            Some(c) => {
                self.stack.pop();
                self.put(p, c, f.act, Dest::Discard, f.source, sink);
                if f.then == Then::JesterGain {
                    // A Victory card: a Curse for the victim. Otherwise the attacker picks who gains a copy
                    // (nobody if there is none in the Supply).
                    if cards::is(c, VICTORY) {
                        self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::CURSE), Dest::Discard) });
                    } else if self.in_supply(c) && self.supply.get(c) > 0 {
                        self.stack.push(Frame { chooser: f.chooser, subject: c, depth: f.depth, ..mode_frame(p, f.source, 1) });
                    }
                }
            }
        }
        Run::Continue
    }

    pub(crate) fn run_overpay(&self, f: Frame) -> Run {
        let max_coins = self.turn.coins.min(OVERPAY_CAP) as u8;
        let potion = crate::sets::overpay_with_potion(self, f.source);
        Run::Decide(DecisionKind::Overpay { card: f.source, max_coins, potion }, 0)
    }

    pub(crate) fn finish_joust(&mut self, f: &Frame) {
        let p = f.player;
        if f.count > 0 {
            // The Province stays set aside until Clean-up; the Reward (if any is left) comes to the hand.
            let ps = &mut self.players[p as usize];
            if ps.set_aside.remove(f.last) {
                ps.cleanup_aside.add(f.last, 1);
            }
            self.stack.push(Frame { gf: crate::state::GF_FROM_SIDE, depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Any, Dest::Hand) });
        }
    }

    pub(crate) fn finish_farmhands(&mut self, f: &Frame) {
        let p = f.player;
        if f.count > 0 {
            let ps = &mut self.players[p as usize];
            if ps.set_aside.remove(f.last) {
                // (More distinct cards than the zone holds: the card stays in the hand.)
                if ps.farm_cards.has(f.last) || ps.farm_cards.iter().count() < crate::state::FARM_CAP {
                    ps.farm_cards.add(f.last, 1);
                } else {
                    ps.hand.add(f.last, 1);
                }
            }
        }
    }

    pub(crate) fn finish_carnival<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        // Carnival: one of each differently named card to the hand, the rest discarded.
        let p = f.player as usize;
        let revealed = self.players[p].set_aside;
        self.players[p].set_aside.clear();
        for (c, n) in revealed.iter() {
            self.players[p].hand.add(c, 1);
            sink.event(Event::Draw { player: f.player, card: c });
            if n > 1 {
                self.players[p].discard.add(c, n - 1);
                for _ in 1..n {
                    sink.event(Event::Discard { player: f.player, card: c });
                }
            }
        }
    }

    pub(crate) fn finish_keep_matching<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        // Hunting Party, Journeyman: the revealed cards that match go to the hand, the rest are
        // discarded. Matching is decided against the hand as it was before any of them join it.
        let p = f.player as usize;
        let revealed = self.players[p].set_aside;
        self.players[p].set_aside.clear();
        let mut keep = Counts::EMPTY;
        for (c, n) in revealed.iter() {
            if self.filter_ok(f.player, f.filter, c) {
                keep.set(c, n);
            }
        }
        for (c, n) in revealed.iter() {
            let kept = keep.get(c);
            if kept > 0 {
                self.players[p].hand.add(c, kept);
                for _ in 0..kept {
                    sink.event(Event::Draw { player: f.player, card: c });
                }
            }
            if n > kept {
                self.players[p].discard.add(c, n - kept);
                for _ in kept..n {
                    sink.event(Event::Discard { player: f.player, card: c });
                }
            }
        }
    }

    pub(crate) fn overpay_choices(&self, f: Frame, out: &mut ChoiceBuf) {
        let max = self.turn.coins.min(OVERPAY_CAP) as u8;
        for coins in 0..=max {
            out.push(Choice::Overpay(coins));
        }
        if crate::sets::overpay_with_potion(self, f.source) {
            for coins in 0..=max {
                out.push(Choice::OverpayPotion(coins));
            }
        }
    }

    pub(crate) fn apply_overpay<S: EventSink>(&mut self, f: Frame, choice: Choice, sink: &mut S) {
        let p = f.player;
        let (coins, potion) = choice.as_overpay().unwrap();
        self.stack.pop();
        self.finish_buy(p, f.source, coins, potion, f.depth, sink);
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
                "Candlestick Maker", "Farrier", "Farmhands", "Baker", "Footpad", "Merchant Guild",
                "Huge Turnip", "Renown",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Hamlet", "Stonemason", "Menagerie", "Shop", "Infirmary", "Advisor",
                "Plaza", "Remake", "Young Witch", "Herald", "Butcher", "Carnival",
                "Ferryman", "Hunting Party", "Journeyman",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
