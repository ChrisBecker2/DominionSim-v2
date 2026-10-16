//! The promos Prince, Walled Village, Envoy, Captain, Sauna/Avanto, Governor, Dismantle, Church, Marchland and the Event Summon.

use super::prelude::*;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const PRINCE: CardId = 125;
    pub const WALLED_VILLAGE: CardId = 126;
    pub const ENVOY: CardId = 127;
    pub const CAPTAIN: CardId = 128;
    /// Governor, Dismantle, Church and Marchland follow Hinterlands (before the Ruins and Knights, which stay the highest zone ids).
    pub const GOVERNOR: CardId = 354;
    pub const DISMANTLE: CardId = 355;
    pub const CHURCH: CardId = 356;
    pub const MARCHLAND: CardId = 357;
    /// The Event Summon: the last Event, after Empires' (the Landmarks follow it).
    pub const SUMMON: CardId = 456;
    /// The split pile Sauna/Avanto (five Sauna on top of five Avanto): Sauna follows the Adventures kingdom cards; Avanto, a bottom half, is
    /// the first of the split-pile bottoms after the kingdom cards (`cards::is_split_bottom`).
    pub const SAUNA: CardId = 232;
    pub const AVANTO: CardId = 258;
}

const CARDS: &[(CardId, CardDef)] = &[
    // Action-Duration-Command since the 2022 errata. It schedules nothing as a Duration: its
    // start-of-turn effect is the Prince bookkeeping (`PlayerState::prince_cards`).
    (ids::PRINCE, promo(order_sensitive(c("Prince", 8, ACTION | DURATION | COMMAND, 0, 0, 0, 0, 0)))), // sets a non-Duration Action costing up to $4 aside (may be the +Actions card in hand)
    (ids::WALLED_VILLAGE, promo(choice_free(c("Walled Village", 4, ACTION, 0, 0, 1, 2, 0)))), // the topdeck is a cleanup offer, not a play decision
    (ids::ENVOY, promo(order_sensitive(c("Envoy", 4, ACTION, 0, 0, 0, 0, 0)))), // reveals the top 5 of the deck, an opponent discards one, the rest to hand (like Advisor)
    // Action-Duration-Command: plays a non-Duration, non-Command Action costing up to $4 from the Supply, now and at
    // the start of the next turn (it is a pending Duration entry, resolved in the start-of-turn order).
    (ids::CAPTAIN, promo(order_sensitive(c("Captain", 6, ACTION | DURATION | COMMAND, 0, 0, 0, 0, 0)))), // plays an Action from the Supply (it may need the +Actions card still in hand)
    // The split pile: five Sauna on top of five Avanto, one kingdom pile (Sauna stands for it; only the top card can be gained or bought).
    (ids::SAUNA, promo(order_sensitive(c("Sauna", 4, ACTION, 0, 0, 1, 1, 0)))), // may play an Avanto from the hand (needs it kept)
    (ids::AVANTO, promo(order_sensitive(c("Avanto", 5, ACTION, 0, 0, 3, 0, 0)))), // may play a Sauna from the hand
    (ids::GOVERNOR, promo(order_sensitive(c("Governor", 5, ACTION, 0, 0, 0, 1, 0)))), // +3 Cards, Gold, or Remodel: may trash an Action from the hand
    (ids::DISMANTLE, promo(order_sensitive(c("Dismantle", 4, ACTION, 0, 0, 0, 0, 0)))), // trashes a card of the hand
    // Action-Duration: sets aside up to 3 cards of the hand face down (`PlayerState::church`), schedules its own next-turn entry per play.
    (ids::CHURCH, promo(order_sensitive(c("Church", 3, ACTION | DURATION, 0, 0, 0, 1, 0)))), // sets aside any cards of the hand
    (ids::MARCHLAND, promo(c("Marchland", 5, VICTORY, 0, 0, 0, 0, 0))), // 1 VP per 3 Victory cards (`vp`); +1 Buy and discard for $ when gained
    // The Event: gains an Action card costing up to $4 and sets it aside to be played at the start of the next turn.
    (ids::SUMMON, promo(c("Summon", 5, 0, 0, 0, 0, 0, 0))),
];

/// The Promo cards (see the module docs of `sets`).
pub struct Promo;

impl SetRules for Promo {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;

    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, id::PRINCE | id::CAPTAIN | id::CHURCH | id::SUMMON)
    }

    /// Marchland's "when you gain this" (+1 Buy, discard for $) is a gain trigger.
    const GAIN_TOP: u128 = 1u128 << (ids::MARCHLAND - 256);

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            // Each option is for every player, the chooser getting the bracketed (bigger) version.
            id::GOVERNOR => &[ModeOpt::EachCards(3, 1), ModeOpt::EachGain(id::GOLD, id::SILVER), ModeOpt::EachTrashGain(2, 1)],
            _ => &[],
        }
    }

    /// Marchland: 1 VP per 3 Victory cards the player has (itself included), rounded down.
    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        if card == id::MARCHLAND {
            let victory: i32 = all.iter().filter(|&(c, _)| cards::is(c, VICTORY)).map(|(_, n)| n as i32).sum();
            victory / 3
        } else {
            cards::def(card).vp as i32
        }
    }

    /// Five of each half of the split pile.
    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        matches!(card, id::SAUNA | id::AVANTO).then_some(5)
    }

    /// Only the top card of the split pile can be bought: an Avanto not while a Sauna is left.
    const MAY_BUY_CARDS: &'static [CardId] = &[id::AVANTO];
    fn may_buy(g: &GameState, card: CardId) -> bool {
        card != id::AVANTO || g.supply.get(id::SAUNA) == 0
    }

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, _sink: &mut S) {
        use Act::*;
        match card {
            // "You may set aside a non-Duration Action from your hand costing $4 or
            // less": an optional pick (`Then::PrinceHold`); a Prince that holds one stays in play.
            // With Throne Room each play picks another.
            id::PRINCE => g.stack.push(select(
                p, card, Zone::Hand, Act::SetAside, Filter::ActionCostUpTo(4), 0, 1, Then::PrinceHold,
            )),
            // Captain, now: play a non-Duration Action costing up to $4 from the Supply, leaving it there
            // (mandatory when one exists). Its start-of-next-turn play is a pending Duration entry
            // (`resolve_duration_start`), scheduled by the generic Duration tail below.
            id::CAPTAIN => g.stack.push(captain_pick(p, card)),
            // Sauna: +1 Card +1 Action; you may play an Avanto from your hand (it uses no Action); this turn, when you play a Silver, you
            // may trash a card from your hand (once per Sauna played).
            id::SAUNA => {
                if p == g.turn.player {
                    g.turn.adv.sauna = g.turn.adv.sauna.saturating_add(1);
                }
                g.stack.push(select(p, card, Zone::Hand, Act::Play, Filter::Card(id::AVANTO), 0, 1, Then::PlayPicked { times: 1 }));
            }
            id::AVANTO => g.stack.push(select(p, card, Zone::Hand, Act::Play, Filter::Card(id::SAUNA), 0, 1, Then::PlayPicked { times: 1 })),
            id::GOVERNOR => g.stack.push(mode_frame(p, card, 1)),
            // Trash a card from the hand (mandatory); if it cost $1 or more, a cheaper card and a Gold.
            id::DISMANTLE => g.stack.push(select(p, card, Zone::Hand, Act::Trash, Filter::Any, 1, 1, Then::DismantleGain)),
            // Church: up to 3 cards face down into a new batch, and a Church entry for the next turn (one per play, never merged).
            id::CHURCH => {
                if g.players[p as usize].church.begin() {
                    g.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Any, 0, 3, Then::ChurchAside));
                    g.push_duration_pending(p, card, 1, card);
                }
            }
            id::ENVOY => {
                // Reveal the top 3 (Envoy: 5); the player to the left picks one to discard; the rest to my hand.
                let left = (p + 1) % g.num_players;
                g.stack.push(Frame { chooser: left, ..select(p, card, Zone::Revealed, Discard, Filter::Any, 1, 1, Then::RevealedToHand) });
                g.stack.push(Frame { max: 5, ..Frame::new(K::RevealTop, p, card) });
            }
            _ => {}
        }
    }

    /// Prince is a Duration by type, but its next-turn behaviour is its own bookkeeping (`PlayerState::prince_cards`), never
    /// a pending entry.
    fn schedules_own_duration(card: CardId) -> bool {
        matches!(card, id::PRINCE | id::CHURCH)
    }

    fn start_entry_named_by_arg(card: CardId) -> bool {
        matches!(card, id::PRINCE | id::SUMMON)
    }

    fn start_entry_repeats(card: CardId) -> bool {
        card == id::PRINCE
    }

    /// Each Prince plays the card it holds at the start of every turn.
    fn turn_start_entries(g: &mut GameState) {
        let pi = g.turn.player as usize;
        for (c, k) in g.players[pi].prince_cards.iter() {
            g.turn.start_pending.push(PendingDuration { card: id::PRINCE, times: k, arg: c, used: 0 });
        }
        // Summon: each Action set aside is played at the start of this turn.
        for (c, k) in g.players[pi].summon_cards.iter() {
            for _ in 0..k {
                g.turn.start_pending.push(PendingDuration { card: id::SUMMON, times: 1, arg: c, used: 0 });
            }
        }
    }

    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
        if e.card == id::SUMMON {
            // The Action set aside by Summon is played now, from set aside into play (no Action used), then it is an ordinary card in play.
            if g.players[p as usize].summon_cards.remove(e.arg) {
                g.players[p as usize].in_play.add(e.arg, 1);
                sink.depth(0);
                sink.event(Event::StartPlay { player: p, source: id::SUMMON, card: e.arg });
                g.resolve_effects(e.arg, p, 0, sink);
            }
            return true;
        }
        if e.card != id::PRINCE {
            return false;
        }
        // "Play that Action": mandatory, uses no Action, and the card isn't put into play
        // (it stays set aside, so a card that moves itself fails to: `NIP`).
        if g.players[p as usize].prince_cards.has(e.arg) {
            sink.depth(0);
            sink.event(Event::StartPlay { player: p, source: id::PRINCE, card: e.arg });
            g.resolve_effects_tok(e.arg, p, 0, crate::track::NIP, sink);
        }
        true
    }

    /// Once per play (a Throne Room'd Captain plays two cards): each is its own pick from the Supply.
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
        if e.card == id::CAPTAIN {
            for _ in 0..e.times {
                g.stack.push(Frame { depth: 1, ..captain_pick(p, e.card) });
            }
        }
        // Church: a batch comes into the hand, then the player may trash a card from the hand (one entry per Church play, so the
        // batches go in one at a time, each followed by its trash). `arg` is the index of the batch the player chose to go first
        // (`Choice::Position`), or the card Church itself when nothing was asked: the oldest.
        if e.card == id::CHURCH {
            let which = if e.arg == id::CHURCH { 0 } else { e.arg as usize };
            let (batch, n) = g.players[p as usize].church.take(which);
            let ps = &mut g.players[p as usize];
            for &c in &batch[..n] {
                ps.hand.add(c, 1);
                sink.event(Event::Draw { player: p, card: c });
            }
            g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing) });
        }
    }

    /// Marchland, when gained: +1 Buy, and discard any number of cards from the hand for +$1 each (both are for the player's own turn: gained
    /// on another player's turn the Buy and the $ are lost, but the discard is still allowed).
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, _to: Dest, _on_buy: bool, sink: &mut S) {
        if c == id::MARCHLAND {
            let own = p == g.turn.player;
            if own {
                g.turn.buys += 1;
                sink.bonus(p, c, 0, 1, 0, 0);
            }
            let then = if own { Then::CoinsPerPick(1) } else { Then::Nothing };
            g.stack.push(select(p, c, Zone::Hand, Act::Discard, Filter::Any, 0, ALL, then));
        }
    }

    /// Walled Village: "at the start of Clean-up, if you have this and no more than one other Action card in play, you may
    /// put this onto your deck", checked when each copy's offer comes up (`GameState::walled_village_blocked`, in the YesNo
    /// frame), so a topdecked Alchemist or Walled Village lowers the count for the next one.
    fn offers_at_cleanup(g: &mut GameState) {
        if g.in_supply(id::WALLED_VILLAGE) {
            let p = g.turn.player;
            for _ in 0..g.players[p as usize].in_play.get(id::WALLED_VILLAGE) {
                g.stack.push(Frame {
                    zone: Zone::InPlay, act: Act::Topdeck, subject: id::WALLED_VILLAGE,
                    ..Frame::new(K::YesNo, p, id::WALLED_VILLAGE)
                });
            }
        }
    }
}

/// Captain's pick: a non-Duration Action costing up to $4 in the Supply, played where it is (a play
/// from the Supply, `Then::PlayPicked`, is a `PlayEffects` frame with `zone: Supply`).
pub(crate) fn captain_pick(p: u8, source: CardId) -> Frame {
    select(p, source, Zone::Supply, Act::Play, Filter::ActionCostUpTo(4), 1, 1, Then::PlayPicked { times: 1 })
}

/// Summon is available when an Action card costing up to $4 can be gained.
pub(crate) fn event_available(g: &GameState, e: CardId) -> bool {
    debug_assert!(e == ids::SUMMON);
    g.supply_cards().any(|c| {
        g.supply.get(c) > 0 && g.pile_open(c) && cards::is(c, ACTION) && g.cost(c) <= 4 && !cards::potion_cost(c) && cards::debt_cost(c) == 0
    })
}

/// Summon: gain an Action card costing up to $4 (to the discard pile) and set it aside if it is still there once the gain is done.
pub(crate) fn event_effect<S: EventSink>(g: &mut GameState, e: CardId, p: u8, _sink: &mut S) {
    debug_assert!(e == ids::SUMMON);
    g.stack.push(Frame { then: Then::SummonAside, ..gain_frame(p, e, 4, Filter::Action, Dest::Discard) });
}

impl GameState {
    /// Governor's chosen option for every player, in turn order starting with the chooser (who has the bracketed version): the frames are pushed
    /// last player first so the chooser's resolves first.
    #[cold]
    #[inline(never)]
    pub(crate) fn governor_frames(&mut self, f: &Frame, opt: ModeOpt) {
        let n = self.num_players as u8;
        for k in (0..n).rev() {
            let q = (f.player + k) % n;
            let me = k == 0;
            match opt {
                ModeOpt::EachCards(a, b) => self.stack.push(draw_frame(q, f.source, if me { a } else { b })),
                ModeOpt::EachGain(a, b) => {
                    self.stack.push(gain_frame(q, f.source, u8::MAX, Filter::Card(if me { a } else { b }), Dest::Discard))
                }
                // "May trash a card from their hand and gain a card costing exactly $1 ($2) more": no card trashed, no gain.
                ModeOpt::EachTrashGain(a, b) => self.stack.push(select(
                    q, f.source, Zone::Hand, Act::Trash, Filter::Any, 0, 1,
                    Then::GainUpTo { plus: if me { a } else { b }, filter: Filter::Any, dest: Dest::Discard, exact: true, dest_by_type: false },
                )),
                _ => {}
            }
        }
    }

    /// Summon's gain is about to happen: a zero-pick selection under the gain's reactions will set the card aside if it is still in the discard
    /// pile once they have run (see `Then::SummonAside`).
    #[cold]
    #[inline(never)]
    pub(crate) fn push_summon_guard(&mut self, f: &Frame, c: CardId) {
        let p = f.player;
        if self.supply.get(c) > 0 {
            self.stack.push(Frame {
                depth: f.depth, subject: c,
                ..select(p, f.source, Zone::Discard, Act::SetAside, Filter::Any, 0, 0, Then::SummonAside)
            });
        }
    }

    /// The card Church just set aside (in `set_aside`) goes into the newest batch, face down.
    #[cold]
    #[inline(never)]
    pub(crate) fn church_add(&mut self, p: u8, c: CardId) {
        let ps = &mut self.players[p as usize];
        if ps.set_aside.remove(c) {
            ps.church.add(c);
        }
    }

    /// Dismantle's gains and Summon's set-aside, once the selection has finished.
    #[inline(never)]
    pub(crate) fn finish_promo_select<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        match f.then {
            // "If it costs $1 or more" (coins; a Potion- or Debt-only cost does not), a cheaper card, then a Gold. "Cheaper" is no part larger
            // and one smaller, the Debt of the card trashed allowed (a Fortune: a Province, an Overlord): the frame's `min` carries it.
            Then::DismantleGain => {
                if f.count > 0 && self.cost(f.last) >= 1 {
                    self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::GOLD), Dest::Discard) });
                    self.stack.push(Frame {
                        less: true, potion: cards::potion_cost(f.last), min: cards::debt_cost(f.last), depth: f.depth,
                        ..gain_frame(p, f.source, self.cost(f.last), Filter::Any, Dest::Discard)
                    });
                }
            }
            // The card gained is set aside only if it is still in the discard pile, where it was gained (a Watchtower or Tracker that moved it
            // made Summon lose track of it); it is played at the start of the next turn.
            Then::SummonAside => {
                let here = f.tok != 0 && self.tok_valid(f.tok);
                self.tok_moved(f.tok, p, Zone::Discard, f.subject);
                let ps = &mut self.players[p as usize];
                let room = ps.summon_cards.has(f.subject) || ps.summon_cards.iter().count() < crate::state::SUMMON_CAP;
                if here && room && ps.discard.remove(f.subject) {
                    ps.summon_cards.add(f.subject, 1);
                    sink.event(Event::SetAside { player: p, card: f.subject });
                }
                self.release_tok(f.tok);
            }
            _ => {}
        }
    }

    /// Walled Village's Clean-up offer needs "no more than one other Action card in play": more than two
    /// Action cards in play (itself included; Durations still in play from earlier turns count, a card
    /// played from the Supply or held by a Prince is not in play) rules it out.
    pub(crate) fn walled_village_blocked(&self, p: u8) -> bool {
        let n: u32 = self.players[p as usize].in_play.iter().filter(|&(c, _)| cards::is(c, ACTION)).map(|(_, n)| n as u32).sum();
        n > 2
    }

    /// Prince: the card just picked (in `set_aside`) is now held by the Prince, which stays in
    /// play for good; the same for a Throne Room / King's Court playing it (counted in the
    /// multiplier tally, see `FrameKind::MultiplierFinalize`).
    #[cold]
    #[inline(never)]
    pub(crate) fn prince_hold(&mut self, f: Frame) {
        let ps = &mut self.players[f.player as usize];
        if !ps.set_aside.remove(f.last) {
            return;
        }
        ps.prince_cards.add(f.last, 1);
        // One slot per Prince physically in play: a Throne Room'd Prince holds two cards but is
        // one card.
        if ps.stays.get(f.source) < ps.in_play.get(f.source) {
            ps.stays.add(f.source, 1);
        }
        if self.turn.multiplier_card != 0 {
            self.turn.multiplier_successes += 1;
        }
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
                "Walled Village",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Prince", "Envoy", "Captain", "Sauna", "Avanto", "Governor", "Dismantle", "Church",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
