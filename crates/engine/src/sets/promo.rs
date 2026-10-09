//! The promos Prince, Walled Village, Envoy and Captain.

use super::prelude::*;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const PRINCE: CardId = 125;
    pub const WALLED_VILLAGE: CardId = 126;
    pub const ENVOY: CardId = 127;
    pub const CAPTAIN: CardId = 128;
    /// The split pile Sauna/Avanto (five Sauna on top of five Avanto): ids after the Adventures kingdom cards, which it comes with.
    pub const SAUNA: CardId = 232;
    pub const AVANTO: CardId = 233;
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
];

/// The Promo cards (see the module docs of `sets`).
pub struct Promo;

impl SetRules for Promo {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;

    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, id::PRINCE | id::CAPTAIN)
    }

    /// Five of each half of the split pile.
    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        matches!(card, id::SAUNA | id::AVANTO).then_some(5)
    }

    /// Only the top card of the split pile can be bought: an Avanto not while a Sauna is left.
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
        card == id::PRINCE
    }

    fn start_entry_named_by_arg(card: CardId) -> bool {
        card == id::PRINCE
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
    }

    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) -> bool {
        if e.card != id::PRINCE {
            return false;
        }
        // "Play that Action": mandatory, uses no Action, and the card isn't put into play
        // (it stays set aside, so a card that moves itself fails to: `prince_play`).
        if g.players[p as usize].prince_cards.has(e.arg) {
            sink.depth(0);
            sink.event(Event::StartPlay { player: p, source: id::PRINCE, card: e.arg });
            g.turn.prince_play = true;
            g.resolve_effects(e.arg, p, 0, sink);
            g.turn.prince_play = false;
        }
        true
    }

    /// Once per play (a Throne Room'd Captain plays two cards): each is its own pick from the Supply.
    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) {
        if e.card == id::CAPTAIN {
            for _ in 0..e.times {
                g.stack.push(Frame { depth: 1, ..captain_pick(p, e.card) });
            }
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

impl GameState {
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
                "Prince", "Envoy", "Captain", "Sauna", "Avanto",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
