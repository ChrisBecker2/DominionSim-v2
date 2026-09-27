//! Card effects. Each multi-step or interactive effect is a `Frame` on the effect stack
//! with three handlers:
//!  - `run`: advance the frame; returns `Decide` (no mutation) when input is needed,
//!    `Chance` when a draw/reveal must be answered (must be re-entrant: nothing is mutated
//!    before `take_top` in that step except an idempotent shuffle), or `Continue`.
//!  - `frame_choices`: legal choices while the frame awaits a decision.
//!  - `apply_frame`: apply a chosen option.
//!
//! Convention: vanilla bonuses (+actions/+buys/+$) apply immediately on play; the card's
//! special frame is pushed next, and the +cards Draw frame is pushed last so it resolves
//! first ("+1 Card +1 Action, then ...").

use crate::cards::{self, id, CardId, ACTION, TREASURE, VICTORY};
use crate::engine::*;
use crate::state::{Frame, FrameKind as K, GameState};

macro_rules! take_top {
    ($s:expr, $p:expr, $sink:expr) => {
        match $s.take_top($p, $sink) {
            Ok(c) => c,
            Err(NeedChance(p)) => return Run::Chance(p),
        }
    };
}

impl Frame {
    /// The card whose effect this frame belongs to (for display).
    pub fn source_card(&self) -> CardId {
        match self.kind {
            K::Draw => 0,
            K::Gain => self.d,
            _ => self.card,
        }
    }
}

fn fr(kind: K, player: u8, card: CardId) -> Frame {
    Frame { card, ..Frame::new(kind, player) }
}

impl GameState {
    /// Resolve the on-play effects of `card` for the current player (card already in play).
    pub(crate) fn resolve_effects<S: EventSink>(&mut self, card: CardId, sink: &mut S) {
        let p = self.turn.player;
        let def = cards::def(card);
        self.turn.actions += def.actions;
        self.turn.buys += def.buys;
        self.turn.coins += def.coins as u16;

        match card {
            id::CELLAR => self.stack.push(fr(K::Cellar, p, card)),
            id::CHAPEL => self.stack.push(fr(K::Chapel, p, card)),
            id::HARBINGER => self.stack.push(fr(K::Harbinger, p, card)),
            id::MERCHANT => self.turn.merchants += 1,
            id::VASSAL => self.stack.push(fr(K::Vassal, p, card)),
            id::WORKSHOP => self.stack.push(gain_frame(p, 4, FILTER_ANY, DEST_DISCARD, card)),
            id::BUREAUCRAT => {
                self.gain(p, id::SILVER, DEST_DECK, sink);
                self.push_victims(K::BureaucratVictim, card);
            }
            id::MILITIA => self.push_victims(K::MilitiaVictim, card),
            id::MONEYLENDER => self.stack.push(fr(K::Moneylender, p, card)),
            id::POACHER => self.stack.push(fr(K::Poacher, p, card)),
            id::REMODEL => self.stack.push(fr(K::Remodel, p, card)),
            id::THRONE_ROOM => self.stack.push(fr(K::ThroneRoom, p, card)),
            id::BANDIT => {
                self.gain(p, id::GOLD, DEST_DISCARD, sink);
                self.push_victims(K::BanditVictim, card);
            }
            id::COUNCIL_ROOM => {
                let n = self.num_players;
                for i in (1..n).rev() {
                    let v = (p + i) % n;
                    self.stack.push(Frame { a: 1, ..Frame::new(K::Draw, v) });
                }
            }
            id::LIBRARY => self.stack.push(fr(K::Library, p, card)),
            id::MINE => self.stack.push(fr(K::Mine, p, card)),
            id::SENTRY => self.stack.push(fr(K::Sentry, p, card)),
            id::WITCH => {
                let n = self.num_players;
                for i in 1..n {
                    let v = (p + i) % n;
                    if !self.immune(v, sink) {
                        self.gain(v, id::CURSE, DEST_DISCARD, sink);
                    }
                }
            }
            id::ARTISAN => {
                self.stack.push(fr(K::ArtisanTopdeck, p, card));
                self.stack.push(gain_frame(p, 5, FILTER_ANY, DEST_HAND, card));
            }
            _ => {} // Moat, Village, Smithy, Festival, Laboratory, Market: vanilla only.
        }

        if def.cards > 0 {
            self.stack.push(Frame { a: def.cards, ..Frame::new(K::Draw, p) });
        }
    }

    /// Push one frame per other player so they resolve in turn order starting to the left.
    fn push_victims(&mut self, kind: K, card: CardId) {
        let p = self.turn.player;
        let n = self.num_players;
        for i in (1..n).rev() {
            self.stack.push(fr(kind, (p + i) % n, card));
        }
    }

    pub(crate) fn run_frame<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        let pi = p as usize;
        match f.kind {
            K::Draw => {
                if f.a == 0 {
                    self.stack.pop();
                    return Run::Continue;
                }
                match take_top!(self, p, sink) {
                    None => {
                        self.stack.pop();
                    }
                    Some(c) => {
                        self.players[pi].hand.add(c, 1);
                        sink.event(Event::Draw { player: p, card: c });
                        f.a -= 1;
                        if f.a == 0 { self.stack.pop(); } else { self.stack.set_top(f); }
                    }
                }
                Run::Continue
            }
            K::PlayEffects => {
                self.stack.pop();
                self.resolve_effects(f.card, sink);
                Run::Continue
            }
            K::Gain => {
                let mut buf = ChoiceBuf::default();
                self.frame_choices(f, &mut buf);
                if buf.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::Gain, 0)
                }
            }
            K::Cellar => {
                if self.players[pi].hand.is_empty() {
                    self.finish_cellar(f);
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::CellarDiscard, 0)
                }
            }
            K::Chapel => {
                if f.a >= 4 || self.players[pi].hand.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::ChapelTrash, 0)
                }
            }
            K::Harbinger => {
                if self.players[pi].discard.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::HarbingerTopdeck, 0)
                }
            }
            K::Vassal => {
                match take_top!(self, p, sink) {
                    None => {
                        self.stack.pop();
                    }
                    Some(c) => {
                        self.players[pi].discard.add(c, 1);
                        sink.event(Event::Discard { player: p, card: c });
                        if cards::is(c, ACTION) {
                            self.stack.set_top(Frame { kind: K::VassalPlay, a: c, ..f });
                        } else {
                            self.stack.pop();
                        }
                    }
                }
                Run::Continue
            }
            K::VassalPlay => Run::Decide(DecisionKind::VassalPlay, f.a),
            K::BureaucratVictim => {
                if f.a == 0 {
                    // First step: Moat check.
                    if self.immune(p, sink) {
                        self.stack.pop();
                        return Run::Continue;
                    }
                    f.a = 1;
                    self.stack.set_top(f);
                }
                if !self.players[pi].hand.any_type(VICTORY) {
                    for (c, n) in self.players[pi].hand.iter() {
                        for _ in 0..n {
                            sink.event(Event::Reveal { player: p, card: c });
                        }
                    }
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::BureaucratTopdeck, 0)
                }
            }
            K::MilitiaVictim => {
                if f.a == 0 {
                    if self.immune(p, sink) {
                        self.stack.pop();
                        return Run::Continue;
                    }
                    f.a = 1;
                    self.stack.set_top(f);
                }
                if self.players[pi].hand.total() <= 3 {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::MilitiaDiscard, 0)
                }
            }
            K::Moneylender => {
                if self.players[pi].hand.has(id::COPPER) {
                    Run::Decide(DecisionKind::MoneylenderTrash, 0)
                } else {
                    self.stack.pop();
                    Run::Continue
                }
            }
            K::Poacher => {
                if f.d == 0 {
                    // Count empty piles when the ability resolves (after the +1 Card).
                    f.a = self.empty_piles() as u8;
                    f.d = 1;
                    self.stack.set_top(f);
                }
                if f.a == 0 || self.players[pi].hand.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::PoacherDiscard, 0)
                }
            }
            K::Remodel => {
                if self.players[pi].hand.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::RemodelTrash, 0)
                }
            }
            K::ThroneRoom => {
                if self.players[pi].hand.any_type(ACTION) {
                    Run::Decide(DecisionKind::ThroneRoomTarget, 0)
                } else {
                    self.stack.pop();
                    Run::Continue
                }
            }
            K::BanditVictim => {
                // b: 0 = not started, 1 = revealing, 2 = revealed. a = count, c/d = cards.
                if f.b == 0 {
                    if self.immune(p, sink) {
                        self.stack.pop();
                        return Run::Continue;
                    }
                    f.b = 1;
                    self.stack.set_top(f);
                }
                if f.b == 1 {
                    if f.a < 2 {
                        match take_top!(self, p, sink) {
                            None => f.b = 2,
                            Some(c) => {
                                self.players[pi].set_aside.add(c, 1);
                                sink.event(Event::Reveal { player: p, card: c });
                                if f.a == 0 { f.c = c } else { f.d = c }
                                f.a += 1;
                            }
                        }
                    } else {
                        f.b = 2;
                    }
                    self.stack.set_top(f);
                    return Run::Continue;
                }
                let cand = |c: CardId| cards::is(c, TREASURE) && c != id::COPPER;
                let c1 = f.a >= 1 && cand(f.c);
                let c2 = f.a >= 2 && cand(f.d);
                if c1 && c2 && f.c != f.d {
                    return Run::Decide(DecisionKind::BanditTrash, 0);
                }
                let trash = if c1 { Some(f.c) } else if c2 { Some(f.d) } else { None };
                self.finish_bandit(f, trash, sink);
                Run::Continue
            }
            K::Library => {
                if f.a == 1 {
                    return Run::Decide(DecisionKind::LibrarySetAside, f.c);
                }
                if self.players[pi].hand.total() >= 7 {
                    self.finish_library(p, sink);
                    return Run::Continue;
                }
                match take_top!(self, p, sink) {
                    None => self.finish_library(p, sink),
                    Some(c) => {
                        if cards::is(c, ACTION) {
                            // Held in set_aside until the player decides.
                            self.players[pi].set_aside.add(c, 1);
                            sink.event(Event::Reveal { player: p, card: c });
                            f.a = 1;
                            f.c = c;
                            self.stack.set_top(f);
                        } else {
                            self.players[pi].hand.add(c, 1);
                            sink.event(Event::Draw { player: p, card: c });
                        }
                    }
                }
                Run::Continue
            }
            K::Mine => {
                if self.players[pi].hand.any_type(TREASURE) {
                    Run::Decide(DecisionKind::MineTrash, 0)
                } else {
                    self.stack.pop();
                    Run::Continue
                }
            }
            K::Sentry => {
                // b stage: 0 revealing, 1 fate of c, 2 fate of d, 3 order kept.
                // a = revealed count; c/d = cards; flags in `f.a` high bits: 4 = keep c, 8 = keep d.
                let count = f.a & 3;
                match f.b {
                    0 => {
                        if count < 2 {
                            match take_top!(self, p, sink) {
                                None => f.b = 1,
                                Some(x) => {
                                    self.players[pi].set_aside.add(x, 1);
                                    if count == 0 { f.c = x } else { f.d = x }
                                    f.a += 1;
                                }
                            }
                        } else {
                            f.b = 1;
                        }
                        self.stack.set_top(f);
                        Run::Continue
                    }
                    1 if count >= 1 => Run::Decide(DecisionKind::SentryFate, f.c),
                    2 if count >= 2 => Run::Decide(DecisionKind::SentryFate, f.d),
                    1 | 2 => {
                        f.b = 3;
                        self.stack.set_top(f);
                        Run::Continue
                    }
                    _ => {
                        let keep_c = f.a & 4 != 0;
                        let keep_d = f.a & 8 != 0;
                        if keep_c && keep_d && f.c != f.d {
                            return Run::Decide(DecisionKind::SentryOrder, 0);
                        }
                        // Zero, one, or two identical kept cards: order is irrelevant.
                        if keep_d {
                            self.sentry_topdeck(p, f.d, sink);
                        }
                        if keep_c {
                            self.sentry_topdeck(p, f.c, sink);
                        }
                        self.stack.pop();
                        Run::Continue
                    }
                }
            }
            K::ArtisanTopdeck => {
                if self.players[pi].hand.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::ArtisanTopdeck, 0)
                }
            }
        }
    }

    pub(crate) fn frame_choices(&self, f: Frame, out: &mut ChoiceBuf) {
        let ps = &self.players[f.player as usize];
        match f.kind {
            K::Gain => {
                for c in 0..cards::NUM_CARDS as CardId {
                    if self.in_supply(c)
                        && self.supply.get(c) > 0
                        && cards::cost(c) <= f.a
                        && (f.b != FILTER_TREASURE || cards::is(c, TREASURE))
                    {
                        out.push(Choice::Card(c));
                    }
                }
            }
            K::Cellar | K::Chapel => {
                canonical_picks(&ps.hand, f.b, 0, |_| true, out);
                out.push(Choice::Pass);
            }
            K::Harbinger => {
                for (c, _) in ps.discard.iter() {
                    out.push(Choice::Card(c));
                }
                out.push(Choice::Pass);
            }
            K::VassalPlay | K::Moneylender | K::Library => {
                out.push(Choice::Yes);
                out.push(Choice::No);
            }
            K::BureaucratVictim => {
                for (c, _) in ps.hand.iter() {
                    if cards::is(c, VICTORY) {
                        out.push(Choice::Card(c));
                    }
                }
            }
            K::MilitiaVictim => {
                let need = ps.hand.total().saturating_sub(3);
                canonical_picks(&ps.hand, f.b, need, |_| true, out);
            }
            K::Poacher => {
                let need = (f.a as u32).min(ps.hand.total());
                canonical_picks(&ps.hand, f.b, need, |_| true, out);
            }
            K::Remodel | K::ArtisanTopdeck => {
                for (c, _) in ps.hand.iter() {
                    out.push(Choice::Card(c));
                }
            }
            K::ThroneRoom => {
                for (c, _) in ps.hand.iter() {
                    if cards::is(c, ACTION) {
                        out.push(Choice::Card(c));
                    }
                }
                out.push(Choice::Pass);
            }
            K::Mine => {
                for (c, _) in ps.hand.iter() {
                    if cards::is(c, TREASURE) {
                        out.push(Choice::Card(c));
                    }
                }
                out.push(Choice::Pass);
            }
            K::BanditVictim => {
                out.push(Choice::Card(f.c));
                out.push(Choice::Card(f.d));
            }
            K::Sentry => {
                if f.b == 3 {
                    out.push(Choice::Card(f.c));
                    out.push(Choice::Card(f.d));
                } else {
                    out.push(Choice::Opt(SENTRY_TRASH));
                    out.push(Choice::Opt(SENTRY_DISCARD));
                    out.push(Choice::Opt(SENTRY_KEEP));
                }
            }
            K::Draw | K::PlayEffects | K::Vassal => {}
        }
    }

    pub(crate) fn apply_frame<S: EventSink>(&mut self, mut f: Frame, choice: Choice, sink: &mut S) {
        let p = f.player;
        let pi = p as usize;
        match (f.kind, choice) {
            (K::Gain, Choice::Card(c)) => {
                self.stack.pop();
                self.gain(p, c, f.c, sink);
            }
            (K::Cellar, Choice::Card(c)) => {
                self.discard_from_hand(p, c, sink);
                f.a += 1;
                f.b = c;
                self.stack.set_top(f);
            }
            (K::Cellar, _) => self.finish_cellar(f),
            (K::Chapel, Choice::Card(c)) => {
                self.trash_from_hand(p, c, sink);
                f.a += 1;
                f.b = c;
                self.stack.set_top(f);
            }
            (K::Chapel, _) => {
                self.stack.pop();
            }
            (K::Harbinger, Choice::Card(c)) => {
                self.stack.pop();
                let ps = &mut self.players[pi];
                ps.discard.remove(c);
                ps.deck_known.push_top(c);
                sink.event(Event::Topdeck { player: p, card: c });
            }
            (K::Harbinger, _) => {
                self.stack.pop();
            }
            (K::VassalPlay, Choice::Yes) => {
                self.stack.pop();
                let c = f.a;
                let ps = &mut self.players[pi];
                ps.discard.remove(c);
                ps.in_play.add(c, 1);
                sink.event(Event::Play { player: p, card: c });
                self.resolve_effects(c, sink);
            }
            (K::VassalPlay, _) => {
                self.stack.pop();
            }
            (K::BureaucratVictim, Choice::Card(c)) => {
                self.stack.pop();
                let ps = &mut self.players[pi];
                ps.hand.remove(c);
                ps.deck_known.push_top(c);
                sink.event(Event::Reveal { player: p, card: c });
                sink.event(Event::Topdeck { player: p, card: c });
            }
            (K::MilitiaVictim, Choice::Card(c)) => {
                self.discard_from_hand(p, c, sink);
                f.b = c;
                self.stack.set_top(f);
            }
            (K::Moneylender, Choice::Yes) => {
                self.stack.pop();
                self.trash_from_hand(p, id::COPPER, sink);
                self.turn.coins += 3;
            }
            (K::Moneylender, _) => {
                self.stack.pop();
            }
            (K::Poacher, Choice::Card(c)) => {
                self.discard_from_hand(p, c, sink);
                f.a -= 1;
                f.b = c;
                self.stack.set_top(f);
            }
            (K::Remodel, Choice::Card(c)) => {
                self.stack.pop();
                self.trash_from_hand(p, c, sink);
                self.stack.push(gain_frame(p, cards::cost(c) + 2, FILTER_ANY, DEST_DISCARD, id::REMODEL));
            }
            (K::ThroneRoom, Choice::Card(c)) => {
                self.stack.pop();
                let ps = &mut self.players[pi];
                ps.hand.remove(c);
                ps.in_play.add(c, 1);
                sink.event(Event::Play { player: p, card: c });
                self.stack.push(fr(K::PlayEffects, p, c));
                self.stack.push(fr(K::PlayEffects, p, c));
            }
            (K::ThroneRoom, _) => {
                self.stack.pop();
            }
            (K::BanditVictim, Choice::Card(c)) => self.finish_bandit(f, Some(c), sink),
            (K::Library, Choice::Yes) => {
                // Stays in set_aside; discarded when Library finishes.
                sink.event(Event::SetAside { player: p, card: f.c });
                f.a = 0;
                self.stack.set_top(f);
            }
            (K::Library, _) => {
                let ps = &mut self.players[pi];
                ps.set_aside.remove(f.c);
                ps.hand.add(f.c, 1);
                sink.event(Event::Draw { player: p, card: f.c });
                f.a = 0;
                self.stack.set_top(f);
            }
            (K::Mine, Choice::Card(c)) => {
                self.stack.pop();
                self.trash_from_hand(p, c, sink);
                self.stack.push(gain_frame(p, cards::cost(c) + 3, FILTER_TREASURE, DEST_HAND, id::MINE));
            }
            (K::Mine, _) => {
                self.stack.pop();
            }
            (K::Sentry, Choice::Card(top)) => {
                // Stage 3: `top` goes on top, the other beneath it.
                let other = if top == f.c { f.d } else { f.c };
                self.sentry_topdeck(p, other, sink);
                self.sentry_topdeck(p, top, sink);
                self.stack.pop();
            }
            (K::Sentry, Choice::Opt(o)) => {
                let x = if f.b == 1 { f.c } else { f.d };
                match o {
                    SENTRY_TRASH => {
                        self.players[pi].set_aside.remove(x);
                        self.trash.add(x, 1);
                        sink.event(Event::Trash { player: p, card: x });
                    }
                    SENTRY_DISCARD => {
                        let ps = &mut self.players[pi];
                        ps.set_aside.remove(x);
                        ps.discard.add(x, 1);
                        sink.event(Event::Discard { player: p, card: x });
                    }
                    _ => f.a |= if f.b == 1 { 4 } else { 8 },
                }
                f.b += 1;
                self.stack.set_top(f);
            }
            (K::ArtisanTopdeck, Choice::Card(c)) => {
                self.stack.pop();
                let ps = &mut self.players[pi];
                ps.hand.remove(c);
                ps.deck_known.push_top(c);
                sink.event(Event::Topdeck { player: p, card: c });
            }
            (kind, ch) => unreachable!("choice {ch:?} not valid for frame {kind:?}"),
        }
    }

    fn finish_cellar(&mut self, f: Frame) {
        self.stack.pop();
        if f.a > 0 {
            self.stack.push(Frame { a: f.a, ..Frame::new(K::Draw, f.player) });
        }
    }

    fn finish_bandit<S: EventSink>(&mut self, f: Frame, trash: Option<CardId>, sink: &mut S) {
        self.stack.pop();
        let p = f.player;
        let mut trashed = false;
        for (i, x) in [f.c, f.d].into_iter().enumerate() {
            if i as u8 >= f.a {
                break;
            }
            let ps = &mut self.players[p as usize];
            ps.set_aside.remove(x);
            if !trashed && Some(x) == trash {
                trashed = true;
                self.trash.add(x, 1);
                sink.event(Event::Trash { player: p, card: x });
            } else {
                ps.discard.add(x, 1);
                sink.event(Event::Discard { player: p, card: x });
            }
        }
    }

    fn finish_library<S: EventSink>(&mut self, p: u8, sink: &mut S) {
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
    }

    fn sentry_topdeck<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        ps.set_aside.remove(c);
        ps.deck_known.push_top(c);
        sink.event(Event::Topdeck { player: p, card: c });
    }
}

fn gain_frame(p: u8, max_cost: u8, filter: u8, dest: u8, source: CardId) -> Frame {
    Frame { a: max_cost, b: filter, c: dest, d: source, ..Frame::new(K::Gain, p) }
}
