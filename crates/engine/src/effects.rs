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

use crate::cards::{self, id, CardId, ModeOpt, ACTION, TREASURE, VICTORY};
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

/// Choose `picks` options from `cards::modes(source)` (see `state::FrameKind::Mode`).
fn mode_frame(p: u8, source: CardId, picks: u8) -> Frame {
    Frame { max: picks, ..Frame::new(K::Mode, p, source) }
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
                // `max` names the target hand size (3); the actual discard count is computed
                // live when this frame is first run (see `Frame::down_to_target`), so a Diplomat
                // reaction that draws/discards first (resolving before this frame) is picked up.
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    self.stack.push(Frame { down_to_target: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 3, Then::Nothing) });
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
            id::MONUMENT => self.players[p as usize].vp_tokens += 1,
            id::CITY => {
                // +1 Card +2 Actions (vanilla); with 1+ empty piles +1 Card more, with 2+ also +1 Buy +$1.
                let empty = self.empty_piles();
                if empty >= 1 {
                    self.stack.push(draw_frame(p, card, 1));
                }
                if empty >= 2 {
                    self.turn.buys += 1;
                    self.turn.coins += 1;
                }
            }
            id::MAGNATE => {
                // Reveal your hand: +1 Card per Treasure in it.
                let hand = self.players[p as usize].hand;
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                }
                let treasures = hand.count_type(cards::TREASURE) as u8;
                if treasures > 0 {
                    self.stack.push(draw_frame(p, card, treasures));
                }
            }
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
            // ---- Mode decisions ("choose one/many"): see `cards::modes` and `FrameKind::Mode`. ----
            id::PAWN => self.stack.push(mode_frame(p, card, 2)),
            id::STEWARD | id::NOBLES | id::MINION | id::LURKER => self.stack.push(mode_frame(p, card, 1)),
            id::COURTIER => self.stack.push(select(p, card, Zone::Hand, Reveal, Filter::Any, 1, 1, Then::ModePerType)),
            id::TORTURER => {
                // Choice-free for the player: +3 Cards is vanilla (below); only the victims
                // (Moat allowing) get a Mode decision, each over the same `modes(TORTURER)` table.
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    self.stack.push(mode_frame(v, card, 1));
                }
            }
            // ---- Hidden information (step 4) ----
            id::WISHING_WELL => self.stack.push(Frame::new(K::Name, p, card)),
            id::SWINDLER => {
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    self.stack.push(Frame { chooser: p, ..Frame::new(K::TrashTopThenGain, v, card) });
                }
            }
            // Deferred (see `FrameKind::PassLeftBegin`) so "who has cards to pass" is checked
            // after this player's own +2 Cards (pushed below it) has resolved, not before.
            id::MASQUERADE => self.stack.push(Frame::new(K::PassLeftBegin, p, card)),
            id::SECRET_PASSAGE => {
                self.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Any, 1, 1, Then::TakeToDeckPosition))
            }
            // ---- Reactions (step 5) ----
            id::DIPLOMAT => self.stack.push(Frame {
                min: 0, max: 0, then: Then::ActionsIfHandAtMost { max_hand: 5, actions: 2 },
                ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::Nothing)
            }),
            _ => {} // Moat, Village, Smithy, Festival, Laboratory, Market: vanilla only.
        }

        if def.cards > 0 {
            self.stack.push(draw_frame(p, card, def.cards));
        }
        // Any Attack, once played, opens a reaction window for every other player: today only
        // Diplomat (Moat's auto-reveal immunity is handled separately by `immune`/`victims`).
        // Pushed last so it resolves before this card's own per-victim frames (pushed above).
        if cards::is(card, cards::ATTACK) {
            self.push_reaction_window(card, sink);
        }
    }

    /// Open a reaction window for an Attack just played: every other player holding a reaction
    /// card whose table entry applies (today, just Diplomat) is offered a `YesNo` to reveal it.
    /// Simplification: offered once per Attack play per player, even if they hold several copies
    /// of the same reaction card (2nd-edition Diplomat allows re-revealing another copy if the
    /// hand is still large enough; not modelled).
    fn push_reaction_window<S: EventSink>(&mut self, attack: CardId, _sink: &mut S) {
        let n = self.num_players;
        let attacker = self.turn.player;
        for i in (1..n).rev() {
            let v = (attacker + i) % n;
            for &(react_card, min_hand, draw, discard) in cards::REACTION_EFFECTS {
                let hand = &self.players[v as usize].hand;
                if hand.get(react_card) > 0 && hand.total() >= min_hand as u32 {
                    self.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Reveal, subject: react_card,
                        then: Then::ReactDrawDiscard { draw, discard },
                        ..Frame::new(K::YesNo, v, react_card)
                    });
                }
            }
        }
        let _ = attack;
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
        match z {
            Zone::Supply => &self.supply,
            Zone::Trash => &self.trash,
            _ => {
                let ps = &self.players[p as usize];
                match z {
                    Zone::Hand => &ps.hand,
                    Zone::Discard => &ps.discard,
                    Zone::Revealed => &ps.set_aside,
                    Zone::InPlay => &ps.in_play,
                    Zone::Supply | Zone::Trash => unreachable!(),
                }
            }
        }
    }

    fn zone_mut(&mut self, p: u8, z: Zone) -> &mut Counts {
        match z {
            Zone::Supply => &mut self.supply,
            Zone::Trash => &mut self.trash,
            _ => {
                let ps = &mut self.players[p as usize];
                match z {
                    Zone::Hand => &mut ps.hand,
                    Zone::Discard => &mut ps.discard,
                    Zone::Revealed => &mut ps.set_aside,
                    Zone::InPlay => &mut ps.in_play,
                    Zone::Supply | Zone::Trash => unreachable!(),
                }
            }
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
                if f.down_to_target {
                    // First touch: `max` was a target zone size (Militia's "discard down to 3");
                    // any reaction that resolves first (Diplomat) already ran, so the zone's
                    // current size is the right one to compute the mandatory count from.
                    let z = self.zone(p, f.zone);
                    let avail: u32 = z.iter().filter(|&(c, _)| f.filter.matches(c)).map(|(_, n)| n as u32).sum();
                    let excess = avail.saturating_sub(f.max as u32) as u8;
                    f.min = excess;
                    f.max = excess;
                    f.down_to_target = false;
                    self.stack.set_top(f);
                }
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
            K::Mode => {
                if f.count >= f.max {
                    self.finish_mode(f, sink);
                    return Run::Continue;
                }
                Run::Decide(DecisionKind::Mode { picks: f.max - f.count, distinct: f.max > 1 }, 0)
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
            K::Name => {
                if f.count == 0 {
                    if self.name_offer(p).is_empty() {
                        self.stack.pop();
                        return Run::Continue;
                    }
                    return Run::Decide(DecisionKind::Name, 0);
                }
                // A card was named (held in `subject`); reveal the real top card and compare.
                match take_top!(self, p, sink) {
                    None => {
                        self.stack.pop();
                    }
                    Some(c) => {
                        self.stack.pop();
                        sink.event(Event::Reveal { player: p, card: c });
                        if c == f.subject {
                            self.players[pi].hand.add(c, 1);
                            sink.event(Event::Draw { player: p, card: c });
                        } else {
                            self.players[pi].deck_known.push_top(c);
                        }
                    }
                }
                Run::Continue
            }
            K::DeckPosition => {
                let max_known = self.players[pi].deck_known.len;
                Run::Decide(DecisionKind::DeckPosition { max_known }, f.subject)
            }
            K::TrashTopThenGain => {
                match take_top!(self, p, sink) {
                    None => {
                        self.stack.pop();
                    }
                    Some(c) => {
                        self.stack.pop();
                        self.trash.add(c, 1);
                        sink.event(Event::Trash { player: p, card: c });
                        let cost = self.cost(c);
                        self.stack.push(Frame { chooser: f.chooser, max: cost, exact: true, dest: Dest::Discard, depth: f.depth, ..Frame::new(K::Gain, p, f.source) });
                    }
                }
                Run::Continue
            }
            K::PassLeftBegin => {
                self.stack.pop();
                // Every player currently holding cards passes one, in turn order starting here;
                // once all have chosen, a `PassLeftDeliver` delivers them at once; then the
                // current player may trash a card from hand.
                self.stack.push(Frame { depth: f.depth, ..select(p, f.source, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing) });
                self.stack.push(Frame { depth: f.depth, ..Frame::new(K::PassLeftDeliver, p, f.source) });
                let n = self.num_players;
                let mut with_cards = [0u8; MAX_PLAYERS];
                let mut count = 0u8;
                for i in 0..n {
                    let pl = (p + i) % n;
                    if !self.players[pl as usize].hand.is_empty() {
                        with_cards[count as usize] = pl;
                        count += 1;
                    }
                }
                for idx in (0..count).rev() {
                    let pl = with_cards[idx as usize];
                    self.stack.push(Frame { depth: f.depth, ..select(pl, f.source, Zone::Hand, Act::Pass, Filter::Any, 1, 1, Then::Nothing) });
                }
                Run::Continue
            }
            K::PassLeftDeliver => {
                self.stack.pop();
                self.deliver_left_passes(sink);
                Run::Continue
            }
        }
    }

    /// The distinct cards Wishing Well may name: the player's whole deck (known + unknown), or
    /// their discard if the deck is empty (since drawing it would shuffle the discard in first).
    fn name_offer(&self, p: u8) -> Counts {
        let ps = &self.players[p as usize];
        if ps.deck_size() > 0 { ps.deck_counts() } else { ps.discard }
    }

    /// "Each player with cards passes one to the next such player to their left, at once"
    /// (Masquerade): once every passing player has chosen (their pick sits in their own
    /// `passed` zone), deliver each held card to the next passing player to the left, all at
    /// once.
    fn deliver_left_passes<S: EventSink>(&mut self, sink: &mut S) {
        let n = self.num_players;
        let mut w = [0u8; MAX_PLAYERS];
        let mut wn = 0usize;
        for i in 0..n {
            if !self.players[i as usize].passed.is_empty() {
                w[wn] = i;
                wn += 1;
            }
        }
        if wn == 0 {
            return;
        }
        let mut moved = [(0u8, 0 as CardId); MAX_PLAYERS];
        for idx in 0..wn {
            let giver = w[idx];
            let ps = &mut self.players[giver as usize];
            let card = ps.passed.iter().next().expect("passing player holds exactly one card").0;
            ps.passed.remove(card);
            moved[idx] = (w[(idx + 1) % wn], card);
        }
        for idx in 0..wn {
            let giver = w[idx];
            let (recipient, card) = moved[idx];
            self.players[recipient as usize].hand.add(card, 1);
            sink.event(Event::Pass { player: giver, card, to: recipient });
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
            K::Mode => {
                // Every option is always legal to pick (a player may choose one they can't fully
                // perform, e.g. Torturer with fewer than 2 cards in hand); only already-chosen
                // indices (the `min` bitmask) are excluded, so a multi-pick decision offers each
                // remaining index in increasing order.
                for (i, _) in cards::modes(f.source).iter().enumerate() {
                    if f.min & (1 << i) == 0 {
                        out.push(Choice::Mode(i as u8));
                    }
                }
            }
            K::Name => {
                for (c, _) in self.name_offer(f.player).iter() {
                    out.push(Choice::Card(c));
                }
            }
            K::DeckPosition => {
                let ps = &self.players[f.player as usize];
                for k in 0..=ps.deck_known.len {
                    out.push(Choice::Position(k));
                }
                // "Bottom" is only a distinct outcome when there's something beneath the known
                // section for the new card to go under; otherwise it coincides with position
                // `deck_known.len` (append at the very end) and isn't offered separately.
                if !ps.deck_unknown.is_empty() || !ps.deck_known_bottom.is_empty() {
                    out.push(Choice::Position(255));
                }
            }
            K::Draw | K::RevealTop | K::PlayEffects | K::Vassal | K::TrashTopThenGain | K::PassLeftBegin | K::PassLeftDeliver => {}
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
                if f.act != Act::Reveal {
                    let removed = self.zone_mut(p, f.zone).remove(c);
                    debug_assert!(removed);
                }
                self.put(p, c, f.act, f.dest, sink);
                f.count += 1;
                f.last = c;
                self.stack.set_top(f);
            }
            (K::Select, _) => self.finish_select(f, sink),
            (K::YesNo, Choice::Yes) => {
                self.stack.pop();
                if f.act != Act::Reveal {
                    self.zone_mut(p, f.zone).remove(f.subject);
                }
                self.put(p, f.subject, f.act, f.dest, sink);
                if f.act == Act::Play {
                    self.resolve_effects(f.subject, f.depth, sink);
                }
                match f.then {
                    Then::CoinsPerPick(n) | Then::YesCoinsElseGainSubject(n) => self.turn.coins += n as u16,
                    Then::ReactDrawDiscard { draw, discard } => {
                        self.stack.push(Frame { depth: f.depth, ..select(p, f.source, Zone::Hand, Act::Discard, Filter::Any, discard, discard, Then::Nothing) });
                        self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, draw) });
                    }
                    _ => {}
                }
            }
            (K::YesNo, _) => {
                self.stack.pop();
                self.yesno_decline(f, sink);
            }
            (K::Mode, Choice::Mode(i)) => {
                f.min |= 1 << i;
                f.count += 1;
                self.stack.set_top(f);
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
            (K::Name, Choice::Card(c)) => {
                f.subject = c;
                f.count = 1;
                self.stack.set_top(f);
            }
            (K::DeckPosition, Choice::Position(k)) => {
                self.stack.pop();
                let ps = &mut self.players[pi];
                ps.set_aside.remove(f.subject);
                if k == 255 {
                    ps.deck_known_bottom.push_top(f.subject);
                } else {
                    ps.deck_known.insert_from_top(k, f.subject);
                }
            }
            (kind, ch) => unreachable!("choice {ch:?} not valid for frame {kind:?}"),
        }
    }

    /// Move card `c` (already removed from its zone, except `Reveal`) according to `act`.
    /// `dest` only matters for `Act::Gain`.
    fn put<S: EventSink>(&mut self, p: u8, c: CardId, act: Act, dest: Dest, sink: &mut S) {
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
            Act::Gain => {
                match dest {
                    Dest::Hand => ps.hand.add(c, 1),
                    Dest::DeckTop => ps.deck_known.push_top(c),
                    Dest::Discard => ps.discard.add(c, 1),
                }
                sink.event(Event::Gain { player: p, card: c, to: dest });
            }
            Act::Reveal => sink.event(Event::Reveal { player: p, card: c }),
            // Held secretly until `deliver_left_passes` moves it; no event here, since what's
            // passed isn't revealed until it's received (see `Event::Pass`).
            Act::Pass => ps.passed.add(c, 1),
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
            Then::ModePerType => {
                if f.count > 0 {
                    let types = cards::def(f.last).types.count_ones();
                    let picks = types.min(cards::modes(f.source).len() as u32) as u8;
                    if picks > 0 {
                        self.stack.push(mode_frame(p, f.source, picks));
                    }
                }
            }
            Then::TakeToDeckPosition => {
                if f.count > 0 {
                    self.stack.push(Frame { subject: f.last, ..Frame::new(K::DeckPosition, p, f.source) });
                }
            }
            Then::ActionsIfHandAtMost { max_hand, actions } => {
                if self.players[p as usize].hand.total() <= max_hand as u32 {
                    self.turn.actions += actions;
                }
            }
            // Not produced by a Select's `then`; only meaningful on YesNo/RevealTop/Gain frames.
            Then::GainedTypeStatBonus
            | Then::GainedTypeDestAttack
            | Then::YesCoinsElseGainSubject(_)
            | Then::MoveMatchingToHand(_)
            | Then::ReactDrawDiscard { .. } => {}
        }
        // Frames spawned by finishing a selection belong to the same card effect.
        for fr in &mut self.stack.frames[base..self.stack.len as usize] {
            fr.depth = f.depth;
        }
    }

    /// A `Mode` frame with every pick made: resolve the chosen atoms (`f.min`'s bitmask) in
    /// increasing table-index order. Atoms that need further input (`TrashFromHand`,
    /// `DiscardFromHand`, `TrashFromSupply`, `GainFromTrash`, `Cards`) push the existing generic
    /// frames, pushed highest-index-first so the lowest index ends up on top and resolves first
    /// (matching the "choose all picks first, then resolve them in order" rule: e.g. Pawn's
    /// +1 Card must not be drawn before its other pick is chosen, which is already guaranteed
    /// since this only runs once every pick has been made). Atoms with no further input
    /// (`Actions`/`Buys`/`Coins`/`Gain`/`DiscardHandDraw`) apply immediately; their relative
    /// order never matters since each is independent of the others.
    fn finish_mode<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        self.stack.pop();
        let base = self.stack.len as usize;
        let p = f.player;
        let table = cards::modes(f.source);
        for i in (0..table.len() as u8).rev() {
            if f.min & (1 << i) == 0 {
                continue;
            }
            match table[i as usize] {
                ModeOpt::Cards(n) => self.stack.push(draw_frame(p, f.source, n)),
                ModeOpt::TrashFromHand(n) => self.stack.push(select(p, f.source, Zone::Hand, Act::Trash, Filter::Any, n, n, Then::Nothing)),
                ModeOpt::DiscardFromHand(n) => self.stack.push(select(p, f.source, Zone::Hand, Act::Discard, Filter::Any, n, n, Then::Nothing)),
                ModeOpt::TrashFromSupply(filt) => self.stack.push(select(p, f.source, Zone::Supply, Act::Trash, filt, 1, 1, Then::Nothing)),
                ModeOpt::GainFromTrash(filt) => self.stack.push(select(p, f.source, Zone::Trash, Act::Gain, filt, 1, 1, Then::Nothing)),
                ModeOpt::Actions(_) | ModeOpt::Buys(_) | ModeOpt::Coins(_) | ModeOpt::Gain(..) | ModeOpt::DiscardHandDraw { .. } => {}
            }
        }
        for fr in &mut self.stack.frames[base..self.stack.len as usize] {
            fr.depth = f.depth;
        }
        for i in 0..table.len() as u8 {
            if f.min & (1 << i) == 0 {
                continue;
            }
            match table[i as usize] {
                ModeOpt::Actions(n) => self.turn.actions += n,
                ModeOpt::Buys(n) => self.turn.buys += n,
                ModeOpt::Coins(n) => self.turn.coins += n as u16,
                ModeOpt::Gain(c, dest) => {
                    self.gain(p, c, dest, sink);
                }
                ModeOpt::DiscardHandDraw { draw, attack_min_hand } => {
                    let base2 = self.stack.len as usize;
                    let (vs, n) = self.victims(sink);
                    for &v in vs[..n].iter().rev() {
                        if self.players[v as usize].hand.total() >= attack_min_hand as u32 {
                            self.discard_whole_hand(v, sink);
                            self.stack.push(draw_frame(v, f.source, draw));
                        }
                    }
                    // Pushed last so my own discard+draw resolves before any victim's.
                    self.discard_whole_hand(p, sink);
                    self.stack.push(draw_frame(p, f.source, draw));
                    for fr in &mut self.stack.frames[base2..self.stack.len as usize] {
                        fr.depth = f.depth;
                    }
                }
                ModeOpt::Cards(_) | ModeOpt::TrashFromHand(_) | ModeOpt::DiscardFromHand(_) | ModeOpt::TrashFromSupply(_) | ModeOpt::GainFromTrash(_) => {}
            }
        }
    }

    /// Discard every card in `p`'s hand (Minion's "discard your hand" for the acting player and
    /// each affected victim).
    fn discard_whole_hand<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        let hand = ps.hand;
        ps.hand.clear();
        ps.discard.add_all(&hand);
        for (c, n) in hand.iter() {
            for _ in 0..n {
                sink.event(Event::Discard { player: p, card: c });
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
