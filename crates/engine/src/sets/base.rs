//! Base Set, 2nd edition, with the basic cards (Copper, Silver, Gold, Estate, Duchy, Province, Curse).

use super::prelude::*;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const COPPER: CardId = 0;
    pub const SILVER: CardId = 1;
    pub const GOLD: CardId = 2;
    pub const ESTATE: CardId = 3;
    pub const DUCHY: CardId = 4;
    pub const PROVINCE: CardId = 5;
    pub const CURSE: CardId = 6;
    pub const CELLAR: CardId = 7;
    pub const CHAPEL: CardId = 8;
    pub const MOAT: CardId = 9;
    pub const HARBINGER: CardId = 10;
    pub const MERCHANT: CardId = 11;
    pub const VASSAL: CardId = 12;
    pub const VILLAGE: CardId = 13;
    pub const WORKSHOP: CardId = 14;
    pub const BUREAUCRAT: CardId = 15;
    pub const GARDENS: CardId = 16;
    pub const MILITIA: CardId = 17;
    pub const MONEYLENDER: CardId = 18;
    pub const POACHER: CardId = 19;
    pub const REMODEL: CardId = 20;
    pub const SMITHY: CardId = 21;
    pub const THRONE_ROOM: CardId = 22;
    pub const BANDIT: CardId = 23;
    pub const COUNCIL_ROOM: CardId = 24;
    pub const FESTIVAL: CardId = 25;
    pub const LABORATORY: CardId = 26;
    pub const LIBRARY: CardId = 27;
    pub const MARKET: CardId = 28;
    pub const MINE: CardId = 29;
    pub const SENTRY: CardId = 30;
    pub const WITCH: CardId = 31;
    pub const ARTISAN: CardId = 32;
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::COPPER, choice_free(c("Copper", 0, TREASURE, 1, 0, 0, 0, 0))),
    (ids::SILVER, choice_free(c("Silver", 3, TREASURE, 2, 0, 0, 0, 0))),
    (ids::GOLD, choice_free(c("Gold", 6, TREASURE, 3, 0, 0, 0, 0))),
    (ids::ESTATE, c("Estate", 2, VICTORY, 0, 1, 0, 0, 0)),
    (ids::DUCHY, c("Duchy", 5, VICTORY, 0, 3, 0, 0, 0)),
    (ids::PROVINCE, c("Province", 8, VICTORY, 0, 6, 0, 0, 0)),
    // Coins=1: only relevant when Curse is also a Treasure this game (Charlatan; see
    // `GameState::is_treasure`). Choice-free: playing it (when it's a Treasure) is just +$1.
    (ids::CURSE, choice_free(c("Curse", 0, CURSE_T, 1, -1, 0, 0, 0))),
    (ids::CELLAR, order_sensitive(c("Cellar", 2, ACTION, 0, 0, 0, 1, 0))), // what to discard (may be the +Actions card in hand)
    (ids::CHAPEL, order_sensitive(c("Chapel", 2, ACTION, 0, 0, 0, 0, 0))), // what to trash (may be the +Actions card in hand)
    (ids::MOAT, choice_free(c("Moat", 2, ACTION | REACTION, 0, 0, 2, 0, 0))),
    (ids::HARBINGER, order_sensitive(c("Harbinger", 3, ACTION, 0, 0, 1, 1, 0))), // what to topdeck from the discard; a later draw takes it
    (ids::MERCHANT, choice_free(c("Merchant", 3, ACTION, 0, 0, 1, 1, 0))), // +$1 on the first Silver: order-free
    (ids::VASSAL, order_sensitive(c("Vassal", 3, ACTION, 2, 0, 0, 0, 0))), // plays the deck top for free; a draw first changes what it finds
    (ids::VILLAGE, choice_free(c("Village", 3, ACTION, 0, 0, 1, 2, 0))),
    (ids::WORKSHOP, has_choice(c("Workshop", 3, ACTION, 0, 0, 0, 0, 0))), // what to gain
    (ids::BUREAUCRAT, order_sensitive(c("Bureaucrat", 4, ACTION | ATTACK, 0, 0, 0, 0, 0))), // gains Silver onto the deck; a later draw takes it
    (ids::GARDENS, c("Gardens", 4, VICTORY, 0, 0, 0, 0, 0)),
    (ids::MILITIA, choice_free(c("Militia", 4, ACTION | ATTACK, 2, 0, 0, 0, 0))),
    (ids::MONEYLENDER, has_choice(c("Moneylender", 4, ACTION, 0, 0, 0, 0, 0))), // may trash a Copper
    (ids::POACHER, order_sensitive(c("Poacher", 4, ACTION, 1, 0, 1, 1, 0))), // discards per empty pile (may be the +Actions card in hand)
    (ids::REMODEL, order_sensitive(c("Remodel", 4, ACTION, 0, 0, 0, 0, 0))), // what to trash and gain (may be the +Actions card in hand)
    (ids::SMITHY, choice_free(c("Smithy", 4, ACTION, 0, 0, 3, 0, 0))),
    (ids::THRONE_ROOM, order_sensitive(plays(c("Throne Room", 4, ACTION, 0, 0, 0, 0, 0), 2))), // what to play twice (maybe the +Actions card itself)
    (ids::BANDIT, has_choice(c("Bandit", 5, ACTION | ATTACK, 0, 0, 0, 0, 0))), // gains a Gold (a reshuffle can draw it)
    (ids::COUNCIL_ROOM, choice_free(c("Council Room", 5, ACTION, 0, 0, 4, 0, 1))),
    (ids::FESTIVAL, choice_free(c("Festival", 5, ACTION, 2, 0, 0, 2, 1))),
    (ids::LABORATORY, choice_free(c("Laboratory", 5, ACTION, 0, 0, 2, 1, 0))),
    (ids::LIBRARY, order_sensitive(c("Library", 5, ACTION, 0, 0, 0, 0, 0))), // draws to 7: cards drawn first are wasted
    (ids::MARKET, choice_free(c("Market", 5, ACTION, 1, 0, 1, 1, 1))),
    (ids::MINE, has_choice(c("Mine", 5, ACTION, 0, 0, 0, 0, 0))), // which treasure to upgrade
    (ids::SENTRY, order_sensitive(c("Sentry", 5, ACTION, 0, 0, 1, 1, 0))), // trash / discard / reorder the top 2; a later draw takes the kept ones
    (ids::WITCH, choice_free(c("Witch", 5, ACTION | ATTACK, 0, 0, 2, 0, 0))),
    (ids::ARTISAN, order_sensitive(c("Artisan", 6, ACTION, 0, 0, 0, 0, 0))), // what to gain and topdeck; a later draw takes it
];

/// The Base cards (see the module docs of `sets`).
pub struct Base;

impl SetRules for Base {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    const BLOCK_REACTIONS: &'static [CardId] = &[id::MOAT];

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        match card {
            id::CELLAR => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::DrawPerPick)),
            id::CHAPEL => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 4, Then::Nothing)),
            id::HARBINGER => g.stack.push(select(p, card, Zone::Discard, Topdeck, Filter::Any, 0, 1, Then::Nothing)),
            id::MERCHANT => g.turn.merchants += 1,
            id::VASSAL => g.stack.push(Frame::new(K::Vassal, p, card)),
            id::WORKSHOP => g.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard)),
            id::BUREAUCRAT => {
                g.gain(p, id::SILVER, Dest::DeckTop, false, card, sink);
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    let hand = g.players[v as usize].hand;
                    if hand.any_type(cards::VICTORY) {
                        g.stack.push(select(v, card, Zone::Hand, Topdeck, Filter::Victory, 1, 1, Then::Nothing));
                    } else {
                        for (c, k) in hand.iter() {
                            for _ in 0..k {
                                g.reveal_card(v, c, sink);
                            }
                        }
                    }
                }
            }
            id::MILITIA => {
                // `max` names the target hand size (3); the actual discard count is computed
                // live when this frame is first run (see `Frame::down_to_target`), so a Diplomat
                // reaction that draws/discards first (resolving before this frame) is picked up.
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::Nothing) });
                }
            }
            id::MONEYLENDER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Card(id::COPPER), 0, 1, Then::CoinsPerPick(3))),
            id::POACHER => {
                // Piles can't change during the +1 Card, so counting now is equivalent.
                let empty = g.empty_piles() as u8;
                if empty > 0 {
                    g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, empty, empty, Then::Nothing));
                }
            }
            id::REMODEL => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
            )),
            id::THRONE_ROOM => g.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 2 })),
            // Inheritance: on its owner's turn an Estate is a Command Action that plays the card under their Estate token, leaving it there.
            id::ESTATE if g.estates_are_actions(p) => {
                let inherit = g.players[p as usize].inherit;
                g.stack.push(Frame { subject: inherit, count: 1, zone: Zone::Supply, ..Frame::new(K::PlayEffects, p, card) });
            }
            id::BANDIT => {
                g.gain(p, id::GOLD, Dest::Discard, false, card, sink);
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(select(v, card, Zone::Revealed, Trash, Filter::NonCopperTreasure, 1, 1, Then::DiscardRevealed));
                    g.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, v, card) });
                }
            }
            id::COUNCIL_ROOM => {
                let n = g.num_players;
                for i in (1..n).rev() {
                    g.stack.push(draw_frame((p + i) % n, card, 1));
                }
            }
            id::LIBRARY => g.stack.push(Frame::new(K::Library, p, card)),
            id::MINE => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Treasure, 0, 1,
                Then::GainUpTo { plus: 3, filter: Filter::Treasure, dest: Dest::Hand, exact: false, dest_by_type: false },
            )),
            id::SENTRY => {
                // Look at the top 2; trash any, discard any, put the rest back in any order.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 0, ALL, Then::Nothing));
                g.stack.push(select(p, card, Zone::Revealed, Trash, Filter::Any, 0, ALL, Then::Nothing));
                g.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, p, card) });
            }
            id::WITCH => {
                // "+2 Cards. Each other player gains a Curse": the Curses are gain frames under
                // the +2 Cards draw, so they come after it (see `push_gain_to_each`).
                let (vs, n) = g.victims(sink);
                g.push_gain_to_each(&vs[..n], id::CURSE, card);
            }
            id::ARTISAN => {
                g.stack.push(select(p, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing));
                g.stack.push(gain_frame(p, card, 5, Filter::Any, Dest::Hand));
            }
            // A Silver played by an effect (Farmhands, Tiara, Crystal Ball): the Merchant bonus too,
            // and a Silver or Gold is the first one Corsair trashes, if it is.
            id::SILVER => {
                let m = if g.turn.silvers_played == 0 { g.turn.merchants as u16 } else { 0 };
                g.turn.coins += m;
                g.turn.silvers_played = g.turn.silvers_played.saturating_add(1);
                sink.bonus(p, id::MERCHANT, 0, 0, m, 0);
                g.corsair_trashes_first_silver_or_gold(p, id::SILVER, sink);
            }
            id::GOLD => g.corsair_trashes_first_silver_or_gold(p, id::GOLD, sink),
            _ => {}
        }
    }

    fn vp(card: CardId, _all: &Counts, total: i32) -> i32 {
        match card {
            id::GARDENS => total / 10,
            _ => cards::def(card).vp as i32,
        }
    }
}

/// Frame handlers of this set's cards (the generic frames' card-specific parts).
impl GameState {
    pub(crate) fn run_vassal<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        let pi = p as usize;

        match take_top!(self, p, sink) {
            None => {
                self.stack.pop();
            }
            Some(c) => {
                self.players[pi].discard.add(c, 1);
                sink.event(Event::Discard { player: p, card: c });
                // The stop-moving rule: a card that reacts to being discarded (a Hound, a Trail) may move this very copy before Vassal
                // plays it, and then Vassal can't, whatever else of that name is in the discard pile. The token names the copy (a Release
                // frame lets it go); the reactions below carry it.
                let hook = self.adv & crate::state::G_DISCARD_HOOK != 0;
                // Gamble (Menagerie) plays an Action or a Treasure; Vassal only an Action.
                let playable = cards::is(c, ACTION) || (f.filter == Filter::ActionOrTreasure && self.is_treasure(c));
                let tok = if hook && playable { self.track(p, Zone::Discard, c) } else { 0 };
                let mut base = 0;
                if playable {
                    if tok != 0 {
                        self.stack.set_top(Frame { tok, depth: f.depth, ..Frame::new(K::Release, p, f.source) });
                        self.stack.push(Frame { zone: Zone::Discard, act: Act::Play, subject: c, depth: f.depth, tok, ..Frame::new(K::YesNo, p, f.source) });
                    } else {
                        self.stack.set_top(Frame { zone: Zone::Discard, act: Act::Play, subject: c, depth: f.depth, ..Frame::new(K::YesNo, p, f.source) });
                    }
                    base = self.stack.len as usize;
                } else {
                    self.stack.pop();
                }
                // Reactions to the discard (a Hound set aside, a Tunnel revealed) are asked before the offer to play it.
                if hook {
                    self.discard_hook(p, c, 1);
                    self.flush_trash_triggers(f.depth, sink);
                    if tok != 0 {
                        for fr in &mut self.stack.frames[base..self.stack.len as usize] {
                            if fr.tok == 0 && fr.zone == Zone::Discard && fr.subject == c && fr.kind == K::YesNo {
                                fr.tok = tok;
                            }
                        }
                    }
                }
            }
        }
        Run::Continue
    }

    pub(crate) fn run_library<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        let pi = p as usize;
        if f.count == 1 {
            return Run::Decide(DecisionKind::YesNo { act: Act::SetAside }, f.subject);
        }
        if self.players[pi].hand.total() >= 7 {
            self.finish_library(p, sink);
            return Run::Continue;
        }
        // The -1 Card token: Library skips one draw, which it makes up for by drawing on: the token just goes.
        self.players[pi].adv &= !crate::state::ADV_MINUS_CARD;
        match take_top!(self, p, sink) {
            None => self.finish_library(p, sink),
            Some(c) => {
                if cards::is(c, ACTION) {
                    // Held in set_aside until the player decides.
                    self.players[pi].set_aside.add(c, 1);
                    self.reveal_card(p, c, sink);
                    f.count = 1;
                    f.subject = c;
                    self.stack.set_top(f);
                } else {
                    self.players[pi].hand.add(c, 1);
                    sink.event(Event::Draw { player: p, card: c });
                }
            }
        }
        Run::Continue
    }

    pub(crate) fn library_set_aside<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) {
        let p = f.player;
        // Stays in set_aside; discarded when Library finishes.
        sink.event(Event::SetAside { player: p, card: f.subject });
        f.count = 0;
        self.stack.set_top(f);
    }

    pub(crate) fn library_take<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        let ps = &mut self.players[pi];
        ps.set_aside.remove(f.subject);
        ps.hand.add(f.subject, 1);
        sink.event(Event::Draw { player: p, card: f.subject });
        f.count = 0;
        self.stack.set_top(f);
    }

    pub(crate) fn finish_library<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        self.stack.pop();
        let ps = &mut self.players[p as usize];
        let aside = ps.set_aside;
        for (c, n) in aside.iter() {
            for _ in 0..n {
                sink.event(Event::Discard { player: p, card: c });
            }
        }
        ps.discard.add_all(&aside);
        ps.set_aside.clear();
        if self.adv & crate::state::G_DISCARD_HOOK != 0 {
            self.discard_hook_counts(p, &aside, 0, sink);
        }
    }}

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
                "Copper", "Silver", "Gold", "Curse", "Moat", "Merchant",
                "Village", "Militia", "Smithy", "Council Room", "Festival", "Laboratory",
                "Market", "Witch",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Cellar", "Chapel", "Harbinger", "Vassal", "Bureaucrat", "Poacher",
                "Remodel", "Throne Room", "Library", "Sentry", "Artisan",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
