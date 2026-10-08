//! Alchemy (one edition; Possession is not implemented, by decision). Potion is a basic card that joins the supply whenever a card with a Potion in its cost is in the kingdom.

use super::prelude::*;
use crate::cards::potion_cost;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const TRANSMUTE: CardId = 113;
    pub const VINEYARD: CardId = 114;
    pub const HERBALIST: CardId = 115;
    pub const APOTHECARY: CardId = 116;
    pub const SCRYING_POOL: CardId = 117;
    pub const UNIVERSITY: CardId = 118;
    pub const ALCHEMIST: CardId = 119;
    pub const FAMILIAR: CardId = 120;
    pub const PHILOSOPHERS_STONE: CardId = 121;
    pub const GOLEM: CardId = 122;
    pub const APPRENTICE: CardId = 123;
    pub const POTION: CardId = 124;
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::TRANSMUTE, alchemy(with_p(order_sensitive(c("Transmute", 0, ACTION, 0, 0, 0, 0, 0))))), // what to trash (may be the +Actions card in hand)
    (ids::VINEYARD, alchemy(with_p(c("Vineyard", 0, VICTORY, 0, 0, 0, 0, 0)))), // 1 VP per 3 Actions (`state::vp_of_cards`)
    (ids::HERBALIST, alchemy(choice_free(c("Herbalist", 2, ACTION, 1, 0, 0, 0, 1)))), // the cleanup offer is not a play decision
    (ids::APOTHECARY, alchemy(with_p(order_sensitive(c("Apothecary", 2, ACTION, 0, 0, 1, 1, 0))))), // reveals the top 4, reorders; a later draw takes them
    (ids::SCRYING_POOL, alchemy(with_p(order_sensitive(c("Scrying Pool", 2, ACTION | ATTACK, 0, 0, 0, 1, 0))))), // discards or keeps tops, draws; order matters
    (ids::UNIVERSITY, alchemy(with_p(has_choice(c("University", 2, ACTION, 0, 0, 0, 2, 0))))), // what to gain
    (ids::ALCHEMIST, alchemy(with_p(choice_free(c("Alchemist", 3, ACTION, 0, 0, 2, 1, 0))))), // the cleanup offer is not a play decision
    (ids::FAMILIAR, alchemy(with_p(choice_free(c("Familiar", 3, ACTION | ATTACK, 0, 0, 1, 1, 0))))), // only victims gain
    // $1 per 5 cards in deck + discard: no choice-free Treasure changes those counts, so the
    // order among choice-free Treasures never matters (has-choice Treasures stop auto-play anyway).
    (ids::PHILOSOPHERS_STONE, alchemy(with_p(choice_free(c("Philosopher's Stone", 3, TREASURE, 0, 0, 0, 0, 0))))),
    (ids::GOLEM, alchemy(with_p(order_sensitive(c("Golem", 4, ACTION, 0, 0, 0, 0, 0))))), // plays Actions from the deck for free; a draw first changes what it finds
    (ids::APPRENTICE, alchemy(order_sensitive(c("Apprentice", 5, ACTION, 0, 0, 0, 1, 0)))), // what to trash (may be the +Actions card in hand)
    (ids::POTION, alchemy(potion_card(choice_free(c("Potion", 4, TREASURE, 0, 0, 0, 0, 0))))),
];

/// The Alchemy cards (see the module docs of `sets`).
pub struct Alchemy;

impl SetRules for Alchemy {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        match card {
            id::FAMILIAR => {
                // "+2 Cards. Each other player gains a Curse": the Curses are gain frames under
                // the +2 Cards draw, so they come after it (see `push_gain_to_each`).
                let (vs, n) = g.victims(sink);
                g.push_gain_to_each(&vs[..n], id::CURSE, card);
            }
            id::TRANSMUTE => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainPerType { action: id::DUCHY, treasure: id::TRANSMUTE, victory: id::GOLD },
            )),
            id::APOTHECARY => {
                // +1 Card +1 Action (vanilla), then reveal the top 4: Coppers and Potions to hand,
                // the rest back in any order (the same frames as Patrol).
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(Frame {
                    max: 4, then: Then::MoveMatchingToHand(Filter::Either(id::COPPER, id::POTION)),
                    ..Frame::new(K::RevealTop, p, card)
                });
            }
            id::SCRYING_POOL => {
                // +1 Action (vanilla). Each player, starting with me, reveals their top card and I
                // choose whether it is discarded or put back (Moat protects victims). Then I reveal
                // until a non-Action and take everything revealed. Pushed bottom-first.
                g.stack.push(Frame {
                    filter: Filter::NonAction, max: 1, then: Then::MoveMatchingToHand(Filter::Any),
                    ..Frame::new(K::RevealUntil, p, card)
                });
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.push_scry(v, p, card);
                }
                g.push_scry(p, p, card);
            }
            id::UNIVERSITY => g.stack.push(Frame { optional: true, ..gain_frame(p, card, 5, Filter::Action, Dest::Discard) }),
            id::PHILOSOPHERS_STONE => {
                let n = g.philosophers_stone_coins(p);
                g.turn.coins += n;
                sink.bonus(p, card, 0, 0, n, 0);
            }
            id::GOLEM => {
                // Reveal until 2 Actions other than Golem; discard the rest; play the two Actions
                // in the order I choose. Pushed bottom-first.
                g.stack.push(select(p, card, Zone::Revealed, Act::Play, Filter::ActionNot(id::GOLEM), 1, 1, Then::PlayPickedThenRest));
                g.stack.push(Frame {
                    filter: Filter::ActionNot(id::GOLEM), max: 2, then: Then::DiscardRevealedNotMatching,
                    ..Frame::new(K::RevealUntil, p, card)
                });
            }
            id::APPRENTICE => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::DrawPerCost { potion_extra: 2 })),
            _ => {}
        }
    }

    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        match card {
            id::VINEYARD => (all.count_type(cards::ACTION) / 3) as i32,
            _ => cards::def(card).vp as i32,
        }
    }

    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        (card == id::POTION).then_some(16)
    }

    /// Herbalist, 2022 text: "Once this turn, when you discard a Treasure from play, you may put it onto your deck", so
    /// every play counts (a Throne Roomed Herbalist is two).
    fn offers_on_discard(g: &mut GameState) {
        if g.in_supply(id::HERBALIST) {
            let p = g.turn.player;
            for _ in 0..g.plays_this_turn(p, id::HERBALIST) {
                g.stack.push(Frame {
                    zone: Zone::InPlay, act: Act::Topdeck, filter: Filter::TreasureToDiscard, max: 1,
                    ..Frame::new(K::Select, p, id::HERBALIST)
                });
            }
        }
    }

    /// Alchemist: "at the start of Clean-up, if you have a Potion in play, you may put this onto your deck".
    fn offers_at_cleanup(g: &mut GameState) {
        let p = g.turn.player;
        let play = g.players[p as usize].in_play;
        if g.in_supply(id::ALCHEMIST) && play.has(id::POTION) {
            for _ in 0..play.get(id::ALCHEMIST) {
                g.stack.push(Frame {
                    zone: Zone::InPlay, act: Act::Topdeck, subject: id::ALCHEMIST,
                    ..Frame::new(K::YesNo, p, id::ALCHEMIST)
                });
            }
        }
    }
}

impl GameState {
    /// Scrying Pool for target `t` (me or a victim): reveal their top card; `chooser` (the Pool's
    /// owner) discards it or leaves it; whatever is left goes back on top. Pushed so that the
    /// reveal resolves first.
    pub(crate) fn push_scry(&mut self, t: u8, chooser: u8, source: CardId) {
        self.stack.push(Frame { ordered: true, ..select(t, source, Zone::Revealed, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
        self.stack.push(Frame { chooser, ..select(t, source, Zone::Revealed, Act::Discard, Filter::Any, 0, 1, Then::Nothing) });
        self.stack.push(Frame { max: 1, ..Frame::new(K::RevealTop, t, source) });
    }

    /// $1 per 5 cards in `p`'s deck and discard pile together (Philosopher's Stone).
    pub(crate) fn philosophers_stone_coins(&self, p: u8) -> u16 {
        let ps = &self.players[p as usize];
        ((ps.deck_size() + ps.discard.total()) / 5) as u16
    }
}

/// Appends Potion to a kingdom list when any card in it has a Potion in its cost (re-sorted).
pub fn add_potion_if_needed(k: &mut Vec<CardId>) {
    if !k.contains(&id::POTION) && k.iter().any(|&c| potion_cost(c)) {
        k.push(id::POTION);
        k.sort_unstable();
    }
}

/// Frame handlers of this set's cards (the generic frames' card-specific parts).
impl GameState {
    pub(crate) fn finish_golem(&mut self, f: &Frame) {
        let p = f.player;
        if f.count > 0 {
            // The remaining Actions leave the Revealed zone (which the first card's effects
            // may use) for the player's `held` zone now, and enter play only when their
            // turn comes (`K::PlayEffects` with `act: Play`).
            let rest = self.players[p as usize].set_aside;
            self.players[p as usize].set_aside.clear();
            for (c, n) in rest.iter() {
                for _ in 0..n {
                    self.players[p as usize].held.add(c, 1);
                    self.stack.push(Frame { subject: c, act: Act::Play, count: 1, ..Frame::new(K::PlayEffects, p, f.source) });
                }
            }
            self.stack.push(Frame { subject: f.last, count: 1, ..Frame::new(K::PlayEffects, p, f.source) });
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
                "Herbalist", "Alchemist", "Familiar", "Philosopher's Stone", "Potion",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Transmute", "Apothecary", "Scrying Pool", "Golem", "Apprentice",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
