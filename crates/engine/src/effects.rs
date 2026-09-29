//! Card effects, built from a handful of generic frames (see `state::FrameKind`):
//! `Draw`, `Gain`, `Select` (pick cards from a zone to discard/trash/topdeck/play, with a
//! continuation), `RevealTop`, `YesNo`, `PlayEffects`, plus two small bespoke loops
//! (`Vassal`, `Library`). Adding a card usually means composing these in `resolve_effects`.
//!
//! Frame handlers:
//!  - `run_frame`: advance the frame; returns `Decide` (without mutating) when input is needed,
//!    `Chance` when a draw/reveal must be answered (re-entrant: nothing is mutated before
//!    `take_top` in that step except an idempotent shuffle), or `Continue`.
//!  - `frame_choices`: legal choices while the frame awaits a decision.
//!  - `apply_frame`: apply a chosen option.
//!
//! Convention: vanilla bonuses (+actions/+buys/+$) apply immediately on play; the card's
//! special frames are pushed next, and the +cards Draw frame is pushed last so it resolves
//! first ("+1 Card +1 Action, then ...").

use crate::cards::{self, id, CardId, ACTION, TREASURE, VICTORY};
use crate::counts::Counts;
use crate::engine::*;
use crate::state::{Act, Dest, Filter, Frame, FrameKind as K, GameState, Then, Zone, MAX_PLAYERS};

macro_rules! take_top {
    ($s:expr, $p:expr, $sink:expr) => {
        match $s.take_top($p, $sink) {
            Ok(c) => c,
            Err(NeedChance(p)) => return Run::Chance(p),
        }
    };
}

/// Pick up to `max` (at least `min`, clamped to what's available) cards from `zone`.
fn select(p: u8, source: CardId, zone: Zone, act: Act, filter: Filter, min: u8, max: u8, then: Then) -> Frame {
    Frame { zone, act, filter, min, max, then, ..Frame::new(K::Select, p, source) }
}

fn gain_frame(p: u8, source: CardId, max_cost: u8, filter: Filter, dest: Dest) -> Frame {
    Frame { max: max_cost, filter, dest, ..Frame::new(K::Gain, p, source) }
}

fn draw_frame(p: u8, source: CardId, n: u8) -> Frame {
    Frame { max: n, ..Frame::new(K::Draw, p, source) }
}

const ALL: u8 = u8::MAX;

impl GameState {
    /// Resolve the on-play effects of `card` for the current player (card already in play).
    /// `depth` is the nesting depth of the play itself; its effects are one level below it.
    pub(crate) fn resolve_effects<S: EventSink>(&mut self, card: CardId, depth: u8, sink: &mut S) {
        sink.depth(depth + 1);
        let base = self.stack.len as usize;
        self.resolve_effects_inner(card, sink);
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            f.depth = depth + 1;
        }
    }

    fn resolve_effects_inner<S: EventSink>(&mut self, card: CardId, sink: &mut S) {
        let p = self.turn.player;
        let def = cards::def(card);
        self.turn.played.add(card, 1);
        self.turn.actions += def.actions;
        self.turn.buys += def.buys;
        self.turn.coins += def.coins as u16;

        use Act::*;
        match card {
            id::CELLAR => self.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::DrawPerPick)),
            id::CHAPEL => self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 4, Then::Nothing)),
            id::HARBINGER => self.stack.push(select(p, card, Zone::Discard, Topdeck, Filter::Any, 0, 1, Then::Nothing)),
            id::MERCHANT => self.turn.merchants += 1,
            id::VASSAL => self.stack.push(Frame::new(K::Vassal, p, card)),
            id::WORKSHOP => self.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard)),
            id::BUREAUCRAT => {
                self.gain(p, id::SILVER, Dest::DeckTop, sink);
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    let hand = self.players[v as usize].hand;
                    if hand.any_type(cards::VICTORY) {
                        self.stack.push(select(v, card, Zone::Hand, Topdeck, Filter::Victory, 1, 1, Then::Nothing));
                    } else {
                        for (c, k) in hand.iter() {
                            for _ in 0..k {
                                sink.event(Event::Reveal { player: v, card: c });
                            }
                        }
                    }
                }
            }
            id::MILITIA => {
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    let excess = self.players[v as usize].hand.total().saturating_sub(3) as u8;
                    if excess > 0 {
                        self.stack.push(select(v, card, Zone::Hand, Discard, Filter::Any, excess, excess, Then::Nothing));
                    }
                }
            }
            id::MONEYLENDER => self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Card(id::COPPER), 0, 1, Then::CoinsPerPick(3))),
            id::POACHER => {
                // Piles can't change during the +1 Card, so counting now is equivalent.
                let empty = self.empty_piles() as u8;
                if empty > 0 {
                    self.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, empty, empty, Then::Nothing));
                }
            }
            id::REMODEL => self.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
            )),
            id::THRONE_ROOM => self.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 2 })),
            id::BANDIT => {
                self.gain(p, id::GOLD, Dest::Discard, sink);
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    self.stack.push(select(v, card, Zone::Revealed, Trash, Filter::NonCopperTreasure, 1, 1, Then::DiscardRevealed));
                    self.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, v, card) });
                }
            }
            id::COUNCIL_ROOM => {
                let n = self.num_players;
                for i in (1..n).rev() {
                    self.stack.push(draw_frame((p + i) % n, card, 1));
                }
            }
            id::LIBRARY => self.stack.push(Frame::new(K::Library, p, card)),
            id::MINE => self.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Treasure, 0, 1,
                Then::GainUpTo { plus: 3, filter: Filter::Treasure, dest: Dest::Hand, exact: false, dest_by_type: false },
            )),
            id::SENTRY => {
                // Look at the top 2; trash any, discard any, put the rest back in any order.
                self.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                self.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 0, ALL, Then::Nothing));
                self.stack.push(select(p, card, Zone::Revealed, Trash, Filter::Any, 0, ALL, Then::Nothing));
                self.stack.push(Frame { max: 2, ..Frame::new(K::RevealTop, p, card) });
            }
            id::WITCH => {
                let (vs, n) = self.victims(sink);
                for &v in &vs[..n] {
                    self.gain(v, id::CURSE, Dest::Discard, sink);
                }
            }
            id::ARTISAN => {
                self.stack.push(select(p, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing));
                self.stack.push(gain_frame(p, card, 5, Filter::Any, Dest::Hand));
            }
            id::BRIDGE => self.turn.cost_reduction += 1,
            // ---- Intrigue (2nd edition) ----
            id::COURTYARD => self.stack.push(select(p, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing)),
            id::SHANTY_TOWN => {
                // Reveal the whole hand (logged), then +2 Cards only if it holds no Action.
                let hand = self.players[p as usize].hand;
                for (c, n) in hand.iter() {
                    for _ in 0..n {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                }
                if !hand.any_type(ACTION) {
                    self.stack.push(draw_frame(p, card, 2));
                }
            }
            id::CONSPIRATOR => {
                // `turn.played` already counts this play (added above); count Action cards only,
                // since it also holds treasures played this turn.
                let actions_played: u32 = self.turn.played.iter().filter(|&(c, _)| cards::is(c, ACTION)).map(|(_, n)| n as u32).sum();
                if actions_played >= 3 {
                    self.turn.actions += 1;
                    self.stack.push(draw_frame(p, card, 1));
                }
            }
            id::BARON => self.stack.push(Frame {
                zone: Zone::Hand, act: Discard, subject: id::ESTATE, then: Then::YesCoinsElseGainSubject(4),
                ..Frame::new(K::YesNo, p, card)
            }),
            id::MINING_VILLAGE => self.stack.push(Frame {
                zone: Zone::InPlay, act: Trash, subject: card, then: Then::CoinsPerPick(2),
                ..Frame::new(K::YesNo, p, card)
            }),
            // "You may discard 2 cards, for +$2": all or nothing (with 1 card in hand, discarding
            // it is allowed but gives no $).
            id::MILL => self.stack.push(Frame { exact: true, ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 2, Then::CoinsIfCount { count: 2, coins: 2 }) }),
            id::PATROL => {
                // Reorders whatever's left after Victory/Curse cards are pulled to hand, same as
                // Sentry's put-back-in-any-order pattern.
                self.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                self.stack.push(Frame {
                    max: 4, then: Then::MoveMatchingToHand(Filter::VictoryOrCurse),
                    ..Frame::new(K::RevealTop, p, card)
                });
            }
            id::TRADING_POST => self.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 2, 2,
                Then::GainCardIfCount { count: 2, card: id::SILVER, dest: Dest::Hand },
            )),
            id::UPGRADE => self.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 1, filter: Filter::Any, dest: Dest::Discard, exact: true, dest_by_type: false },
            )),
            id::IRONWORKS => self.stack.push(Frame { then: Then::GainedTypeStatBonus, ..gain_frame(p, card, 4, Filter::Any, Dest::Discard) }),
            id::REPLACE => self.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: true },
            )),
            _ => {} // Moat, Village, Smithy, Festival, Laboratory, Market: vanilla only.
        }

        if def.cards > 0 {
            self.stack.push(draw_frame(p, card, def.cards));
        }
    }

    /// Other players affected by an attack, in turn order starting to the left.
    /// Reactions are checked when the attack is played (Moat is auto-revealed).
    fn victims<S: EventSink>(&self, sink: &mut S) -> ([u8; MAX_PLAYERS], usize) {
        let mut out = [0u8; MAX_PLAYERS];
        let mut n = 0;
        for v in self.others(self.turn.player) {
            if !self.immune(v, sink) {
                out[n] = v;
                n += 1;
            }
        }
        (out, n)
    }

    fn zone(&self, p: u8, z: Zone) -> &Counts {
        let ps = &self.players[p as usize];
        match z {
            Zone::Hand => &ps.hand,
            Zone::Discard => &ps.discard,
            Zone::Revealed => &ps.set_aside,
            Zone::InPlay => &ps.in_play,
        }
    }

    fn zone_mut(&mut self, p: u8, z: Zone) -> &mut Counts {
        let ps = &mut self.players[p as usize];
        match z {
            Zone::Hand => &mut ps.hand,
            Zone::Discard => &mut ps.discard,
            Zone::Revealed => &mut ps.set_aside,
            Zone::InPlay => &mut ps.in_play,
        }
    }

    /// (remaining min, remaining max, eligible cards at/above the canonical bound) for a Select.
    fn select_bounds(&self, f: &Frame) -> (u8, u8, u32) {
        let z = self.zone(f.player, f.zone);
        let lo = if f.ordered || f.count == 0 { 0 } else { f.last };
        let mut avail_all = 0u32;
        let mut avail_from = 0u32;
        for (c, n) in z.iter() {
            if f.filter.matches(c) {
                avail_all += n as u32;
                if c >= lo {
                    avail_from += n as u32;
                }
            }
        }
        // Every act removes the card from the zone, so count + avail_all is invariant.
        // All-or-nothing selections become mandatory up to `max` once the first pick is made.
        let min = if f.exact && f.count > 0 { f.max } else { f.min };
        let min_total = (min as u32).min(f.count as u32 + avail_all);
        let rem_min = min_total.saturating_sub(f.count as u32) as u8;
        let rem_max = f.max.saturating_sub(f.count);
        (rem_min, rem_max, avail_from)
    }

    pub(crate) fn run_frame<S: EventSink>(&mut self, mut f: Frame, sink: &mut S) -> Run {
        sink.depth(f.depth);
        let p = f.player;
        let pi = p as usize;
        match f.kind {
            K::Draw => {
                if f.max == 0 {
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
                        f.max -= 1;
                        if f.max == 0 { self.stack.pop(); } else { self.stack.set_top(f); }
                    }
                }
                Run::Continue
            }
            K::RevealTop => {
                if f.max == 0 {
                    self.finish_reveal_top(f, sink);
                    return Run::Continue;
                }
                match take_top!(self, p, sink) {
                    None => {
                        self.finish_reveal_top(f, sink);
                    }
                    Some(c) => {
                        self.players[pi].set_aside.add(c, 1);
                        sink.event(Event::Reveal { player: p, card: c });
                        f.max -= 1;
                        if f.max == 0 { self.finish_reveal_top(f, sink); } else { self.stack.set_top(f); }
                    }
                }
                Run::Continue
            }
            K::PlayEffects => {
                self.stack.pop();
                if f.count >= 2 {
                    sink.event(Event::PlayAgain { player: p, card: f.subject, source: f.source, nth: f.count });
                }
                self.resolve_effects(f.subject, f.depth, sink);
                Run::Continue
            }
            K::Gain => {
                let mut buf = ChoiceBuf::default();
                self.frame_choices(f, &mut buf);
                if buf.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(DecisionKind::Gain { max_cost: f.max, filter: f.filter, dest: f.dest, exact: f.exact }, 0)
                }
            }
            K::Select => {
                let (rem_min, rem_max, avail) = self.select_bounds(&f);
                if rem_max == 0 || avail == 0 {
                    self.finish_select(f, sink);
                    return Run::Continue;
                }
                Run::Decide(
                    DecisionKind::Select { from: f.zone, act: f.act, filter: f.filter, min: rem_min, max: rem_max, ordered: f.ordered },
                    0,
                )
            }
            K::YesNo => {
                // Nothing to act on (e.g. Mining Village already trashed by an earlier Throne
                // Room play, or Baron with no Estate in hand): skip straight to the "no" outcome.
                if self.zone(p, f.zone).get(f.subject) == 0 {
                    self.stack.pop();
                    self.yesno_decline(f, sink);
                    return Run::Continue;
                }
                Run::Decide(DecisionKind::YesNo { act: f.act }, f.subject)
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
                            self.stack.set_top(Frame { zone: Zone::Discard, act: Act::Play, subject: c, depth: f.depth, ..Frame::new(K::YesNo, p, f.source) });
                        } else {
                            self.stack.pop();
                        }
                    }
                }
                Run::Continue
            }
            K::Library => {
                if f.count == 1 {
                    return Run::Decide(DecisionKind::YesNo { act: Act::SetAside }, f.subject);
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
        }
    }

    pub(crate) fn frame_choices(&self, f: Frame, out: &mut ChoiceBuf) {
        match f.kind {
            K::Gain => {
                for c in self.supply_cards() {
                    let cost_ok = if f.exact { self.cost(c) == f.max } else { self.cost(c) <= f.max };
                    if self.supply.get(c) > 0 && cost_ok && f.filter.matches(c) {
                        out.push(Choice::Card(c));
                    }
                }
            }
            K::Select => {
                let (rem_min, rem_max, avail) = self.select_bounds(&f);
                let z = self.zone(f.player, f.zone);
                // All-or-nothing: a first pick must leave enough cards (in canonical order) to
                // complete the full set.
                let need = if f.exact && f.count == 0 { (rem_max as u32).min(avail) } else { rem_min as u32 };
                if f.ordered {
                    for (c, _) in z.iter() {
                        if f.filter.matches(c) {
                            out.push(Choice::Card(c));
                        }
                    }
                } else {
                    let lo = if f.count == 0 { 0 } else { f.last };
                    canonical_picks(z, lo, need, |c| f.filter.matches(c), out);
                }
                if rem_min == 0 {
                    out.push(Choice::Pass);
                }
            }
            K::YesNo | K::Library => {
                out.push(Choice::Yes);
                out.push(Choice::No);
            }
            K::Draw | K::RevealTop | K::PlayEffects | K::Vassal => {}
        }
    }

    pub(crate) fn apply_frame<S: EventSink>(&mut self, mut f: Frame, choice: Choice, sink: &mut S) {
        sink.depth(f.depth);
        let p = f.player;
        let pi = p as usize;
        match (f.kind, choice) {
            (K::Gain, Choice::Card(c)) => {
                self.stack.pop();
                // Replace: the gain's destination (and any attack) is keyed off the gained
                // card's type rather than the frame's normal `dest`.
                let dest = if f.then == Then::GainedTypeDestAttack && (cards::is(c, ACTION) || cards::is(c, TREASURE)) {
                    Dest::DeckTop
                } else {
                    f.dest
                };
                if self.gain(p, c, dest, sink) {
                    match f.then {
                        Then::GainedTypeStatBonus => {
                            if cards::is(c, ACTION) {
                                self.turn.actions += 1;
                            }
                            if cards::is(c, TREASURE) {
                                self.turn.coins += 1;
                            }
                            if cards::is(c, VICTORY) {
                                self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 1) });
                            }
                        }
                        Then::GainedTypeDestAttack if cards::is(c, VICTORY) => {
                            let (vs, n) = self.victims(sink);
                            for &v in &vs[..n] {
                                self.gain(v, id::CURSE, Dest::Discard, sink);
                            }
                        }
                        _ => {}
                    }
                }
            }
            (K::Select, Choice::Card(c)) => {
                let removed = self.zone_mut(p, f.zone).remove(c);
                debug_assert!(removed);
                self.put(p, c, f.act, sink);
                f.count += 1;
                f.last = c;
                self.stack.set_top(f);
            }
            (K::Select, _) => self.finish_select(f, sink),
            (K::YesNo, Choice::Yes) => {
                self.stack.pop();
                self.zone_mut(p, f.zone).remove(f.subject);
                self.put(p, f.subject, f.act, sink);
                if f.act == Act::Play {
                    self.resolve_effects(f.subject, f.depth, sink);
                }
                match f.then {
                    Then::CoinsPerPick(n) | Then::YesCoinsElseGainSubject(n) => self.turn.coins += n as u16,
                    _ => {}
                }
            }
            (K::YesNo, _) => {
                self.stack.pop();
                self.yesno_decline(f, sink);
            }
            (K::Library, Choice::Yes) => {
                // Stays in set_aside; discarded when Library finishes.
                sink.event(Event::SetAside { player: p, card: f.subject });
                f.count = 0;
                self.stack.set_top(f);
            }
            (K::Library, _) => {
                let ps = &mut self.players[pi];
                ps.set_aside.remove(f.subject);
                ps.hand.add(f.subject, 1);
                sink.event(Event::Draw { player: p, card: f.subject });
                f.count = 0;
                self.stack.set_top(f);
            }
            (kind, ch) => unreachable!("choice {ch:?} not valid for frame {kind:?}"),
        }
    }

    /// Move card `c` (already removed from its zone) according to `act`.
    fn put<S: EventSink>(&mut self, p: u8, c: CardId, act: Act, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        match act {
            Act::Discard => {
                ps.discard.add(c, 1);
                sink.event(Event::Discard { player: p, card: c });
            }
            Act::Trash => {
                self.trash.add(c, 1);
                sink.event(Event::Trash { player: p, card: c });
            }
            Act::Topdeck => {
                ps.deck_known.push_top(c);
                sink.event(Event::Topdeck { player: p, card: c });
            }
            Act::Play => {
                ps.in_play.add(c, 1);
                sink.event(Event::Play { player: p, card: c });
            }
            Act::SetAside => {
                ps.set_aside.add(c, 1);
                sink.event(Event::SetAside { player: p, card: c });
            }
        }
    }

    fn finish_select<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        self.stack.pop();
        let base = self.stack.len as usize;
        let p = f.player;
        match f.then {
            Then::Nothing => {}
            Then::DrawPerPick => {
                if f.count > 0 {
                    self.stack.push(draw_frame(p, f.source, f.count));
                }
            }
            Then::CoinsPerPick(n) => self.turn.coins += n as u16 * f.count as u16,
            Then::GainUpTo { plus, filter, dest, exact, dest_by_type } => {
                if f.count > 0 {
                    let then = if dest_by_type { Then::GainedTypeDestAttack } else { Then::Nothing };
                    self.stack.push(Frame { exact, then, ..gain_frame(p, f.source, self.cost(f.last) + plus, filter, dest) });
                }
            }
            Then::PlayPicked { times } => {
                if f.count > 0 {
                    // Pushed last-first so the 1st resolution is on top; `count` = which play.
                    for nth in (1..=times).rev() {
                        self.stack.push(Frame { subject: f.last, count: nth, ..Frame::new(K::PlayEffects, p, f.source) });
                    }
                }
            }
            Then::DiscardRevealed => {
                let ps = &mut self.players[p as usize];
                let rest = ps.set_aside;
                for (c, n) in rest.iter() {
                    for _ in 0..n {
                        sink.event(Event::Discard { player: p, card: c });
                    }
                }
                ps.discard.add_all(&rest);
                ps.set_aside.clear();
            }
            Then::CoinsIfCount { count, coins } => {
                if f.count == count {
                    self.turn.coins += coins as u16;
                }
            }
            Then::GainCardIfCount { count, card, dest } => {
                if f.count == count {
                    self.gain(p, card, dest, sink);
                }
            }
            // Not produced by a Select's `then`; only meaningful on YesNo/RevealTop/Gain frames.
            Then::GainedTypeStatBonus | Then::GainedTypeDestAttack | Then::YesCoinsElseGainSubject(_) | Then::MoveMatchingToHand(_) => {}
        }
        // Frames spawned by finishing a selection belong to the same card effect.
        for fr in &mut self.stack.frames[base..self.stack.len as usize] {
            fr.depth = f.depth;
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

    /// A declined (or skipped, nothing-to-act-on) `YesNo`: the only generic consequence is
    /// Baron's "if you don't, gain an Estate" (any `Then::YesCoinsElseGainSubject`).
    fn yesno_decline<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        if let Then::YesCoinsElseGainSubject(_) = f.then {
            self.gain(f.player, f.subject, Dest::Discard, sink);
        }
    }

    /// A `RevealTop` frame finishing (either it revealed everything asked for, or the deck ran
    /// out): pop it and, for Patrol, move every revealed card matching the filter into hand.
    fn finish_reveal_top<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        self.stack.pop();
        if let Then::MoveMatchingToHand(filter) = f.then {
            let p = f.player as usize;
            let revealed = self.players[p].set_aside;
            for (c, n) in revealed.iter() {
                if filter.matches(c) {
                    self.players[p].set_aside.set(c, 0);
                    self.players[p].hand.add(c, n);
                    for _ in 0..n {
                        sink.event(Event::Draw { player: f.player, card: c });
                    }
                }
            }
        }
    }
}
