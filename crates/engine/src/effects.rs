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
//!
//! ## The Duration framework (Seaside step 3)
//!
//! A Duration card's "now" part plays exactly like any other card (vanilla bonuses via
//! `CardDef`, plus a match arm here for anything else: Blockade's gain, Sea Witch's attack,
//! Tactician's conditional discard). Its "next turn" part is scheduled as a
//! `state::PendingDuration` entry in the player's `PlayerState::pending_durations` (pushed by
//! `push_duration_pending`, called generically at the tail of `resolve_effects_inner` for every
//! Duration card except Haven/Blockade/Tactician, which schedule themselves once their own
//! argument — the set-aside/gained card, or whether the discard condition held — is known) and
//! resolved by `GameState::resolve_duration_start` as a `FrameKind::DurationStart` frame, pushed
//! for every pending entry at the very start of the owner's next turn
//! (`GameState::push_turn_start_frames`, called from `engine::run_phase`'s `Phase::Action` arm).
//!
//! The card stays in play (not discarded at cleanup) via a *separate* mechanism,
//! `TurnState::duration_held`: a running count, incremented wherever a card is actually placed
//! into `in_play` by being played (`put`'s `Act::Play` arm; the two direct plays in
//! `engine::apply_phase`; `GameState::play_choice_free_treasures` for Astrolabe), plus once more
//! for a Throne Room/King's Court that multiplies a Duration target (`finish_select`'s
//! `Then::PlayPicked`) — per the plan, the multiplier itself stays in play too. This is tracked
//! separately from `pending_durations` because the two don't always correspond 1:1: a Throne-
//! Roomed Duration card is ONE physical copy in play but may end up with TWO entries in
//! `pending_durations` (Haven set aside two different cards, one per resolution) or have its one
//! argless entry's `times` incremented to 2 (Fishing Village) — either way, only one physical
//! copy needs to survive cleanup, which `duration_held`, not the entry count, tracks correctly.

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
    /// Resolve the on-play effects of `card` for `player` (card already in play). `depth` is the
    /// nesting depth of the play itself; its effects are one level below it. `player` is almost
    /// always `self.turn.player` (every ordinary play, Vassal, Throne Room/King's Court are all
    /// the current turn's own cards) but isn't for a card played reactively by another player
    /// out of turn (Pirate's "when any player gains a Treasure, you may play this from your
    /// hand"): every caller passes the frame's own `player`, never assumes the current turn.
    pub(crate) fn resolve_effects<S: EventSink>(&mut self, card: CardId, player: u8, depth: u8, sink: &mut S) {
        sink.depth(depth + 1);
        let base = self.stack.len as usize;
        self.resolve_effects_inner(card, player, sink);
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            f.depth = depth + 1;
        }
    }

    fn resolve_effects_inner<S: EventSink>(&mut self, card: CardId, player: u8, sink: &mut S) {
        let p = player;
        let def = cards::def(card);
        self.turn.played.add(card, 1);
        self.turn.actions += def.actions;
        self.turn.buys += def.buys;
        self.turn.coins += def.coins as u16;
        self.turn.potions += def.potions;

        use Act::*;
        match card {
            id::CELLAR => self.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::DrawPerPick)),
            id::CHAPEL => self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, 4, Then::Nothing)),
            id::HARBINGER => self.stack.push(select(p, card, Zone::Discard, Topdeck, Filter::Any, 0, 1, Then::Nothing)),
            id::MERCHANT => self.turn.merchants += 1,
            id::VASSAL => self.stack.push(Frame::new(K::Vassal, p, card)),
            id::WORKSHOP => self.stack.push(gain_frame(p, card, 4, Filter::Any, Dest::Discard)),
            id::BUREAUCRAT => {
                self.gain(p, id::SILVER, Dest::DeckTop, false, card, sink);
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
                self.gain(p, id::GOLD, Dest::Discard, false, card, sink);
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
            id::WITCH | id::FAMILIAR => {
                // "+2 Cards. Each other player gains a Curse": the Curses are gain frames under
                // the +2 Cards draw, so they come after it (see `push_gain_to_each`).
                let (vs, n) = self.victims(sink);
                self.push_gain_to_each(&vs[..n], id::CURSE, card);
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
                // Reveal your hand: +1 Card per Treasure in it (Curse counts too, under Charlatan).
                let hand = self.players[p as usize].hand;
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                }
                let treasures = hand.iter().filter(|&(c, _)| self.is_treasure(c)).map(|(_, n)| n as u32).sum::<u32>() as u8;
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
            // ---- Prosperity (2nd edition), step 2. Treasures with a choice resolve through
            // this same match (they're already in play by the time `apply_phase`'s
            // `PlayTreasure` arm calls `resolve_effects`); their vanilla +$/+Buy is applied
            // generically above, same as any Action. ----
            id::ANVIL => self.stack.push(select(
                p, card, Zone::Hand, Discard, self.treasure_filter(), 0, 1,
                Then::GainFixedUpTo { max_cost: 4, filter: Filter::Any, dest: Dest::Discard },
            )),
            id::WATCHTOWER => {
                let have = self.players[p as usize].hand.total();
                let need = 6u32.saturating_sub(have) as u8;
                if need > 0 {
                    self.stack.push(draw_frame(p, card, need));
                }
            }
            id::BISHOP => {
                // +$1 is vanilla (above); +1 VP token is unconditional. The card's own trash is
                // mandatory if hand holds anything to trash ("Trash a card from your hand", no
                // "may"; each other player's is explicitly "may" and not an Attack: no Moat check).
                self.players[p as usize].vp_tokens += 1;
                let n = self.num_players;
                for i in (1..n).rev() {
                    let v = (p + i) % n;
                    self.stack.push(select(v, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing));
                }
                self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::VpPerCost { per: 2 }));
            }
            id::CLERK => {
                // +$2 is vanilla (above). Each other player (Moat allowing) with 5+ cards in
                // hand puts one onto their deck. Clerk's own start-of-turn reaction ("you may
                // play this from your hand") needs step 3's turn-start hook — TODO(step 3).
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    if self.players[v as usize].hand.total() >= 5 {
                        self.stack.push(select(v, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing));
                    }
                }
            }
            id::INVESTMENT => {
                self.stack.push(mode_frame(p, card, 1));
                self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Nothing));
            }
            id::TIARA => self.stack.push(select(p, card, Zone::Hand, Play, self.treasure_filter(), 0, 1, Then::PlayPicked { times: 2 })),
            id::CHARLATAN => {
                // +$3 is vanilla (above); Curse becomes a Treasure this game via `is_treasure`.
                let (vs, n) = self.victims(sink);
                self.push_gain_to_each(&vs[..n], id::CURSE, card);
            }
            id::CRYSTAL_BALL => {
                // Look at the top card: may trash it, discard it, or (if Action/Treasure) play
                // it; declining all three (or nothing was there) leaves it on top. Mutually
                // exclusive for free: each optional Select below only finds the card if an
                // earlier one didn't already take it.
                self.stack.push(Frame { min: ALL, max: ALL, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                self.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 0, 1, Then::Nothing));
                self.stack.push(select(p, card, Zone::Revealed, Trash, Filter::Any, 0, 1, Then::Nothing));
                self.stack.push(select(p, card, Zone::Revealed, Play, Filter::ActionOrTreasure, 0, 1, Then::PlayPicked { times: 1 }));
                self.stack.push(Frame { max: 1, ..Frame::new(K::RevealTop, p, card) });
            }
            id::MINT => self.stack.push(select(p, card, Zone::Hand, Reveal, self.treasure_filter(), 0, 1, Then::GainCopyOfRevealed { dest: Dest::Discard })),
            id::RABBLE => {
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    self.stack.push(Frame { ordered: true, ..select(v, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                    self.stack.push(Frame { max: 3, then: Then::MoveMatchingToDiscard(Filter::ActionOrTreasure), ..Frame::new(K::RevealTop, v, card) });
                }
            }
            id::VAULT => {
                let n = self.num_players;
                for i in (1..n).rev() {
                    let v = (p + i) % n;
                    self.stack.push(Frame { exact: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 2, Then::DrawIfCount { count: 2, draw: 1 }) });
                }
                self.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::CoinsPerPick(1)));
            }
            id::WAR_CHEST => {
                let left = (p + 1) % self.num_players;
                self.stack.push(Frame { chooser: left, zone: Zone::Supply, ..Frame::new(K::Name, p, card) });
            }
            id::EXPAND => self.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 3, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
            )),
            id::FORGE => self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, ALL, Then::GainExactCostSum { dest: Dest::Discard })),
            id::KINGS_COURT => self.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 3 })),
            // Bank's own value is dynamic ($1 per Treasure in play, counting itself, which is
            // already placed in `in_play` by the time this runs): no static `coins` in its
            // `CardDef` (that generic bonus applied above is 0), computed fresh here instead.
            id::BANK => self.turn.coins += self.treasures_in_play(p) as u16,
            // ---- Alchemy. Herbalist and Alchemist have only vanilla bonuses on play: their
            // "when you discard this from play" offers are made at the end of the Buy phase
            // (`push_end_of_turn_offers`). ----
            id::TRANSMUTE => self.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainPerType { action: id::DUCHY, treasure: id::TRANSMUTE, victory: id::GOLD },
            )),
            id::APOTHECARY => {
                // +1 Card +1 Action (vanilla), then reveal the top 4: Coppers and Potions to hand,
                // the rest back in any order (the same frames as Patrol).
                self.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                self.stack.push(Frame {
                    max: 4, then: Then::MoveMatchingToHand(Filter::Either(id::COPPER, id::POTION)),
                    ..Frame::new(K::RevealTop, p, card)
                });
            }
            id::SCRYING_POOL => {
                // +1 Action (vanilla). Each player, starting with me, reveals their top card and I
                // choose whether it is discarded or put back (Moat protects victims). Then I reveal
                // until a non-Action and take everything revealed. Pushed bottom-first.
                self.stack.push(Frame {
                    filter: Filter::NonAction, max: 1, then: Then::MoveMatchingToHand(Filter::Any),
                    ..Frame::new(K::RevealUntil, p, card)
                });
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    self.push_scry(v, p, card);
                }
                self.push_scry(p, p, card);
            }
            id::UNIVERSITY => self.stack.push(Frame { optional: true, ..gain_frame(p, card, 5, Filter::Action, Dest::Discard) }),
            id::PHILOSOPHERS_STONE => self.turn.coins += self.philosophers_stone_coins(p),
            id::GOLEM => {
                // Reveal until 2 Actions other than Golem; discard the rest; play the two Actions
                // in the order I choose. Pushed bottom-first.
                self.stack.push(select(p, card, Zone::Revealed, Act::Play, Filter::ActionNot(id::GOLEM), 1, 1, Then::PlayPickedThenRest));
                self.stack.push(Frame {
                    filter: Filter::ActionNot(id::GOLEM), max: 2, then: Then::DiscardRevealedNotMatching,
                    ..Frame::new(K::RevealUntil, p, card)
                });
            }
            id::APPRENTICE => self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::DrawPerCost { potion_extra: 2 })),
            // ---- Seaside (2nd edition), step 3: Durations. "Now" parts beyond vanilla
            // CardDef bonuses; "next turn" parts are in `resolve_duration_start`. Every other
            // Duration card here (Lighthouse, Astrolabe, Fishing Village, Monkey, Caravan,
            // Sailor, Tide Pools, Corsair, Merchant Ship, Outpost, Pirate, Wharf) has no "now"
            // specifics beyond `CardDef`, so it falls through to `_ => {}` and is scheduled by
            // the generic tail below. ----
            // ---- Seaside (2nd edition), step 4: the last 10 cards (none are Durations). ----
            id::NATIVE_VILLAGE => self.stack.push(mode_frame(p, card, 1)),
            id::LOOKOUT => {
                // Look at the top 3: trash exactly one (mandatory if any), discard exactly one
                // of what's left (mandatory if any), put whatever remains (0 or 1 card, no real
                // reordering choice) back on top — reuses Sentry's RevealTop + Select stack.
                self.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                self.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 1, 1, Then::Nothing));
                self.stack.push(select(p, card, Zone::Revealed, Trash, Filter::Any, 1, 1, Then::Nothing));
                self.stack.push(Frame { max: 3, ..Frame::new(K::RevealTop, p, card) });
            }
            id::SEA_CHART => self.stack.push(Frame { max: 1, then: Then::SeaChartCheck, ..Frame::new(K::RevealTop, p, card) }),
            id::SMUGGLERS => {
                // "Gain a copy of a card costing up to $6 that the player to your right gained
                // on their last turn": candidates are further restricted to that player's
                // `last_turn_gains` by `gain_from_record` (checked in `frame_choices`/Gain).
                self.stack.push(Frame { gain_from_record: true, ..gain_frame(p, card, 6, Filter::Any, Dest::Discard) });
            }
            id::WAREHOUSE => self.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 3, 3, Then::Nothing)),
            id::CUTPURSE => {
                // +$2 is vanilla (below). Each other player (Moat allowing) discards a Copper,
                // or reveals a hand with none — mirrors Bureaucrat's Victory-card check.
                let (vs, n) = self.victims(sink);
                for &v in vs[..n].iter().rev() {
                    let hand = self.players[v as usize].hand;
                    if hand.has(id::COPPER) {
                        self.stack.push(select(v, card, Zone::Hand, Discard, Filter::Card(id::COPPER), 1, 1, Then::Nothing));
                    } else {
                        for (c, k) in hand.iter() {
                            for _ in 0..k {
                                sink.event(Event::Reveal { player: v, card: c });
                            }
                        }
                    }
                }
            }
            id::ISLAND => {
                // Move Island itself off `in_play` onto the mat immediately (it never reaches
                // cleanup: this is a permanent move, not a Duration hold). Idempotent under
                // Throne Room/King's Court: a later resolution finds it already gone and just
                // skips this part, matching the ruling ("the second play can't move Island
                // again, but still moves another card").
                let ps = &mut self.players[p as usize];
                if ps.in_play.remove(id::ISLAND) {
                    ps.island_mat.add(id::ISLAND, 1);
                    sink.event(Event::SetAside { player: p, card: id::ISLAND });
                }
                self.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Any, 1, 1, Then::MoveToIslandMat));
            }
            id::SALVAGER => self.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::CoinsEqualToCostSum)),
            id::TREASURE_MAP => {
                // 2nd edition: "this" (the physical copy in play) is trashed only if it's still
                // in play — a 2nd Throne Room/King's Court resolution finds it already gone.
                let ps = &mut self.players[p as usize];
                let self_trashed = ps.in_play.remove(id::TREASURE_MAP);
                if self_trashed {
                    self.trash.add(id::TREASURE_MAP, 1);
                    sink.event(Event::Trash { player: p, card: id::TREASURE_MAP });
                }
                self.stack.push(Frame {
                    self_trashed,
                    ..select(p, card, Zone::Hand, Trash, Filter::Card(id::TREASURE_MAP), 0, 1, Then::TreasureMapGold)
                });
            }
            // Treasury: vanilla-only on play (+1 Card +1 Action +$1); its end-of-Buy-phase "you
            // may put this onto your deck" offer is a turn-boundary hook, not a play-time effect
            // — see `GameState::push_treasury_offers`, called from `engine::run_phase`.
            id::HAVEN => self.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Any, 1, 1, Then::ScheduleDuration { times: 1 })),
            id::BLOCKADE => self.stack.push(Frame { then: Then::ScheduleDuration { times: 1 }, ..gain_frame(p, card, 4, Filter::Any, Dest::Discard) }),
            id::SEA_WITCH => {
                // "+2 Cards. Each other player gains a Curse": after the draw, like Witch.
                let (vs, n) = self.victims(sink);
                self.push_gain_to_each(&vs[..n], id::CURSE, card);
            }
            id::TACTICIAN => {
                // "If you have a card in hand" gates the *entire* effect, including staying in
                // play as a Duration: an empty hand means Tactician does nothing further and
                // discards normally at cleanup (excluded from the generic tail below; this arm
                // is the only place that schedules it).
                if !self.players[p as usize].hand.is_empty() {
                    self.discard_whole_hand(p, sink);
                    self.push_duration_pending(p, card, 1, 0);
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

        // Generic Duration scheduling: every Duration card gets a pending start-of-next-turn
        // entry, except the three that schedule themselves above once their own argument (the
        // set-aside/gained card, or whether the "if you have a card in hand" condition held) is
        // known (Haven, Blockade: after a Select/Gain resolves; Tactician: only conditionally).
        if cards::is(card, cards::DURATION) && !matches!(card, id::HAVEN | id::BLOCKADE | id::TACTICIAN) {
            self.push_duration_pending(p, card, 1, 0);
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

    /// Scrying Pool for target `t` (me or a victim): reveal their top card; `chooser` (the Pool's
    /// owner) discards it or leaves it; whatever is left goes back on top. Pushed so that the
    /// reveal resolves first.
    fn push_scry(&mut self, t: u8, chooser: u8, source: CardId) {
        self.stack.push(Frame { ordered: true, ..select(t, source, Zone::Revealed, Act::Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
        self.stack.push(Frame { chooser, ..select(t, source, Zone::Revealed, Act::Discard, Filter::Any, 0, 1, Then::Nothing) });
        self.stack.push(Frame { max: 1, ..Frame::new(K::RevealTop, t, source) });
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

    /// "Each other player gains a `card`": one mandatory gain frame per victim, pushed now so it
    /// resolves after anything the attack pushes later (its +Cards draw is pushed last, so it
    /// resolves first, as the card text orders it). Pushed in reverse so the first victim in
    /// turn order resolves first: with a short pile, the leftmost opponent gets the card. Each
    /// gain goes through the normal gain path, so reactions like Watchtower resolve right after
    /// that player's gain.
    fn push_gain_to_each(&mut self, victims: &[u8], card: CardId, source: CardId) {
        for &v in victims.iter().rev() {
            self.stack.push(Frame { filter: Filter::Card(card), ..gain_frame(v, source, u8::MAX, Filter::Card(card), Dest::Discard) });
        }
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
            K::RevealUntil => {
                if f.count >= f.max {
                    self.finish_reveal_top(f, sink);
                    return Run::Continue;
                }
                match take_top!(self, p, sink) {
                    None => self.finish_reveal_top(f, sink),
                    Some(c) => {
                        self.players[pi].set_aside.add(c, 1);
                        sink.event(Event::Reveal { player: p, card: c });
                        if f.filter.matches(c) {
                            f.count += 1;
                        }
                        if f.count >= f.max { self.finish_reveal_top(f, sink); } else { self.stack.set_top(f); }
                    }
                }
                Run::Continue
            }
            K::NativeVillageAdd => {
                match take_top!(self, p, sink) {
                    None => {
                        self.stack.pop();
                    }
                    Some(c) => {
                        self.stack.pop();
                        self.players[pi].native_village_mat.add(c, 1);
                        sink.event(Event::SetAside { player: p, card: c });
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
                // A card held in the frame since it was set aside (Golem's second Action): it
                // enters play only now.
                if f.act == Act::Play {
                    self.players[pi].held.remove(f.subject);
                    self.players[pi].in_play.add(f.subject, 1);
                    sink.event(Event::Play { player: p, card: f.subject });
                }
                if f.count >= 2 {
                    sink.event(Event::PlayAgain { player: p, card: f.subject, source: f.source, nth: f.count });
                }
                self.resolve_effects(f.subject, p, f.depth, sink);
                Run::Continue
            }
            K::Gain => {
                let mut buf = ChoiceBuf::default();
                self.frame_choices(f, &mut buf);
                if buf.is_empty() {
                    self.stack.pop();
                    Run::Continue
                } else {
                    Run::Decide(
                        DecisionKind::Gain { max_cost: f.max, filter: f.filter, dest: f.dest, exact: f.exact, potion: f.potion, optional: f.optional },
                        0,
                    )
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
                // `f.zone == Zone::Supply` is War Chest's variant of the generic Name frame: the
                // player to the left names any supply card (not a guess about a deck), and once
                // named there's no "reveal and compare" step — it's just recorded, and a Gain
                // frame (excluding every name so far this turn) follows. Otherwise this is
                // Wishing Well's "name a card, reveal the top, draw it if it matches."
                let war_chest = f.zone == Zone::Supply;
                if f.count == 0 {
                    let has_options =
                        if war_chest { self.supply_cards().any(|c| self.supply.get(c) > 0) } else { !self.name_offer(p).is_empty() };
                    if !has_options {
                        self.stack.pop();
                        return Run::Continue;
                    }
                    return Run::Decide(DecisionKind::Name, 0);
                }
                if war_chest {
                    self.stack.pop();
                    self.turn.named_for_war_chest.add(f.subject, 1);
                    self.stack.push(Frame { excl_named: true, depth: f.depth, ..gain_frame(p, f.source, 5, Filter::Any, Dest::Discard) });
                    return Run::Continue;
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
                        self.stack.push(Frame {
                            chooser: f.chooser, max: cost, exact: true, potion: cards::potion_cost(c), dest: Dest::Discard, depth: f.depth,
                            ..Frame::new(K::Gain, p, f.source)
                        });
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
            K::DurationStart => {
                self.stack.pop();
                let entry = crate::state::PendingDuration { card: f.source, times: f.count, arg: f.subject, used: false };
                self.resolve_duration_start(p, entry, sink);
                Run::Continue
            }
            K::MultiplierFinalize => {
                self.stack.pop();
                // Throne Room / King's Court stays in play with the Duration it played if *any*
                // of its plays keeps that Duration in play ("even if only one play of the
                // Duration card is keeping it in play, Throne Room stays in play with it").
                if self.turn.multiplier_card == f.source && self.turn.multiplier_successes >= 1 {
                    self.turn.duration_held.add(f.source, 1);
                    // An Outpost played during an Outpost turn fails (no 3rd turn in a row): the
                    // multiplier stays in play with it until the next turn's cleanup (`cleanup`).
                    if f.subject == id::OUTPOST && self.turn.is_extra_turn {
                        self.players[p as usize].discard_next_cleanup.add(f.source, 1);
                    }
                }
                self.turn.multiplier_card = 0;
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
                    // Potion: "up to $X" admits Potion-cost cards only when the reference cost has a
                    // Potion; "exactly" needs the Potion to match.
                    let pc = cards::potion_cost(c);
                    let cost_ok = if f.exact { self.cost(c) == f.max && pc == f.potion } else { self.cost(c) <= f.max && (f.potion || !pc) };
                    let named_ok = !f.excl_named || !self.turn.named_for_war_chest.has(c);
                    // Smugglers: further restricted to cards the player to `f.player`'s right
                    // gained on their last turn.
                    let record_ok = !f.gain_from_record || self.players[self.right_of(f.player) as usize].last_turn_gains.has(c);
                    if self.supply.get(c) > 0 && cost_ok && named_ok && record_ok && f.filter.matches(c) {
                        out.push(Choice::Card(c));
                    }
                }
                if f.optional && !out.is_empty() {
                    out.push(Choice::Pass);
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
            K::Name if f.zone == Zone::Supply => {
                // War Chest: the player to the left may name any card in this game's supply.
                for c in self.supply_cards() {
                    if self.supply.get(c) > 0 {
                        out.push(Choice::Card(c));
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
            K::Draw | K::RevealTop | K::PlayEffects | K::Vassal | K::TrashTopThenGain | K::PassLeftBegin | K::PassLeftDeliver | K::DurationStart | K::MultiplierFinalize | K::NativeVillageAdd | K::RevealUntil => {}
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
                // Reactions the gain triggers (Watchtower) belong to this card effect: log them
                // at its depth.
                let base = self.stack.len as usize;
                let gained = self.gain(p, c, dest, false, f.source, sink);
                for fr in &mut self.stack.frames[base..self.stack.len as usize] {
                    fr.depth = f.depth;
                }
                if gained {
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
                            // Forward order: same leftmost-priority reasoning as Witch.
                            let (vs, n) = self.victims(sink);
                            for &v in &vs[..n] {
                                self.gain(v, id::CURSE, Dest::Discard, false, f.source, sink);
                            }
                        }
                        Then::ScheduleDuration { times } => {
                            // Blockade: move the just-gained card (landed in `dest`, above) into
                            // `set_aside` and schedule its return to hand. If a reaction already
                            // moved it elsewhere (Watchtower trashing/topdecking it), there's
                            // nothing to set aside or schedule.
                            let ps = &mut self.players[p as usize];
                            let moved = match dest {
                                Dest::Discard => ps.discard.remove(c),
                                Dest::Hand => ps.hand.remove(c),
                                Dest::DeckTop => false, // no known-top zone to pull back out of
                            };
                            if moved {
                                ps.set_aside.add(c, 1);
                                sink.event(Event::SetAside { player: p, card: c });
                                self.push_duration_pending(p, f.source, times, c);
                            }
                        }
                        _ => {}
                    }
                }
            }
            (K::Select, Choice::Card(c)) => {
                if f.reveal_source && f.count == 0 {
                    sink.event(Event::Reaction { player: p, card: f.source });
                }
                // Tracked unconditionally (cheap: one add); only `Then::GainExactCostSum`
                // (Forge) reads it. Cost is taken at the moment of picking, per the plan's
                // ruling: Bridge/Quarry-style reductions active now apply on both sides.
                f.cost_sum += self.cost(c) as u16;
                if f.act != Act::Reveal {
                    let removed = self.zone_mut(p, f.zone).remove(c);
                    debug_assert!(removed);
                }
                self.put(p, c, f.act, f.dest, f.source, sink);
                f.count += 1;
                f.last = c;
                self.stack.set_top(f);
            }
            (K::Gain, Choice::Pass) => {
                self.stack.pop();
            }
            (K::Select, _) => self.finish_select(f, sink),
            (K::YesNo, Choice::Yes) => {
                self.stack.pop();
                if f.act != Act::Reveal {
                    self.zone_mut(p, f.zone).remove(f.subject);
                }
                self.put(p, f.subject, f.act, f.dest, f.source, sink);
                if f.act == Act::Play {
                    self.resolve_effects(f.subject, p, f.depth, sink);
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
    fn put<S: EventSink>(&mut self, p: u8, c: CardId, act: Act, dest: Dest, source: CardId, sink: &mut S) {
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
                // Note: unlike Merchant Ship-style Durations, this does *not* mark the card
                // held for cleanup — that only happens once a next-turn effect is actually
                // scheduled (`push_duration_pending`), so a conditional Duration (Haven,
                // Blockade) that finds nothing to do still discards normally.
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
                sink.event(Event::Gain { player: p, card: c, to: dest, source });
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
                    self.stack.push(Frame { exact, then, potion: cards::potion_cost(f.last), ..gain_frame(p, f.source, self.cost(f.last) + plus, filter, dest) });
                }
            }
            Then::PlayPicked { times } => {
                if f.count > 0 {
                    // Throne Room / King's Court on a Duration card: the multiplier also stays in
                    // play if at least one resolution schedules a next-turn effect (a conditional
                    // Duration, e.g. Haven/Tactician with too few cards, may do nothing on some
                    // resolutions; one is enough). `MultiplierFinalize` sits below all `times`
                    // resolutions (and everything they push) and checks the tally once they've
                    // all unwound.
                    if cards::is(f.last, cards::DURATION) {
                        self.turn.multiplier_card = f.source;
                        self.turn.multiplier_expected = times;
                        self.turn.multiplier_successes = 0;
                        self.stack.push(Frame { depth: f.depth, subject: f.last, ..Frame::new(K::MultiplierFinalize, p, f.source) });
                    }
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
                    self.gain(p, card, dest, false, f.source, sink);
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
            Then::GainFixedUpTo { max_cost, filter, dest } => {
                if f.count > 0 {
                    self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, max_cost, filter, dest) });
                }
            }
            Then::GainCopyOfRevealed { dest } => {
                if f.count > 0 {
                    self.gain(p, f.last, dest, false, f.source, sink);
                }
            }
            Then::VpPerCost { per } => {
                if f.count > 0 {
                    self.players[p as usize].vp_tokens += (self.cost(f.last) / per) as u16;
                }
            }
            Then::GainExactCostSum { dest } => {
                // Always runs, even if nothing was trashed (total $0: a Copper or a Curse).
                let cost = f.cost_sum.min(u8::MAX as u16) as u8;
                self.stack.push(Frame { exact: true, depth: f.depth, ..gain_frame(p, f.source, cost, Filter::Any, dest) });
            }
            Then::DrawIfCount { count, draw } => {
                if f.count == count {
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, draw) });
                }
            }
            Then::ScheduleDuration { times } => {
                // Haven: the card set aside (already moved to `set_aside` by the pick itself,
                // `Act::SetAside`); nothing scheduled if the player had no card to set aside.
                if f.count > 0 {
                    self.push_duration_pending(p, f.source, times, f.last);
                }
            }
            Then::MoveToIslandMat => {
                // Island: the hand card just moved to `set_aside` by the pick itself
                // (`Act::SetAside`) joins the Island card already on the mat. No-op if nothing
                // was picked (an empty hand at the time of the 2nd Throne Room resolution, say).
                if f.count > 0 {
                    let ps = &mut self.players[p as usize];
                    if ps.set_aside.remove(f.last) {
                        ps.island_mat.add(f.last, 1);
                    }
                }
            }
            Then::CoinsEqualToCostSum => self.turn.coins += f.cost_sum,
            Then::GainPerType { action, treasure, victory } => {
                if f.count > 0 {
                    let c = f.last;
                    if cards::is(c, ACTION) && action != 0 {
                        self.gain(p, action, Dest::Discard, false, f.source, sink);
                    }
                    if self.is_treasure(c) && treasure != 0 {
                        self.gain(p, treasure, Dest::Discard, false, f.source, sink);
                    }
                    if cards::is(c, VICTORY) && victory != 0 {
                        self.gain(p, victory, Dest::Discard, false, f.source, sink);
                    }
                }
            }
            Then::DrawPerCost { potion_extra } => {
                if f.count > 0 {
                    let n = self.cost(f.last) as u32 + if cards::potion_cost(f.last) { potion_extra as u32 } else { 0 };
                    if n > 0 {
                        self.stack.push(draw_frame(p, f.source, n.min(u8::MAX as u32) as u8));
                    }
                }
            }
            Then::PlayPickedThenRest => {
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
            Then::TreasureMapGold => {
                if f.count > 0 && f.self_trashed {
                    for _ in 0..4 {
                        self.gain(p, id::GOLD, Dest::DeckTop, false, f.source, sink);
                    }
                }
            }
            // Not produced by a Select's `then`; only meaningful on YesNo/RevealTop/Gain frames.
            Then::GainedTypeStatBonus
            | Then::GainedTypeDestAttack
            | Then::YesCoinsElseGainSubject(_)
            | Then::MoveMatchingToHand(_)
            | Then::MoveMatchingToDiscard(_)
            | Then::ReactDrawDiscard { .. }
            | Then::SeaChartCheck
            | Then::DiscardRevealedNotMatching => {}
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
                ModeOpt::NativeVillageAdd => self.stack.push(Frame::new(K::NativeVillageAdd, p, f.source)),
                ModeOpt::Actions(_)
                | ModeOpt::Buys(_)
                | ModeOpt::Coins(_)
                | ModeOpt::Gain(..)
                | ModeOpt::DiscardHandDraw { .. }
                | ModeOpt::TrashSelfRevealVpPerTreasureType
                | ModeOpt::NativeVillageTake => {}
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
                    self.gain(p, c, dest, false, f.source, sink);
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
                ModeOpt::TrashSelfRevealVpPerTreasureType => {
                    // Trash the card that offered this choice (Investment; already in play),
                    // then reveal the hand for +1 VP token per differently-named Treasure in it
                    // (Curse counts too, under Charlatan).
                    let ps = &mut self.players[p as usize];
                    if ps.in_play.remove(f.source) {
                        self.trash.add(f.source, 1);
                        sink.event(Event::Trash { player: p, card: f.source });
                    }
                    let hand = self.players[p as usize].hand;
                    let mut distinct = 0u16;
                    for (c, n) in hand.iter() {
                        if self.is_treasure(c) {
                            distinct += 1;
                            for _ in 0..n {
                                sink.event(Event::Reveal { player: p, card: c });
                            }
                        }
                    }
                    self.players[p as usize].vp_tokens += distinct;
                }
                ModeOpt::NativeVillageTake => {
                    // "Put all the cards from your mat into your hand" (no decision: order among
                    // identical private cards never matters once they're all in hand).
                    let ps = &mut self.players[p as usize];
                    let mat = ps.native_village_mat.take_all();
                    for (c, n) in mat.iter() {
                        ps.hand.add(c, n);
                        for _ in 0..n {
                            sink.event(Event::Draw { player: p, card: c });
                        }
                    }
                }
                ModeOpt::Cards(_)
                | ModeOpt::TrashFromHand(_)
                | ModeOpt::DiscardFromHand(_)
                | ModeOpt::TrashFromSupply(_)
                | ModeOpt::GainFromTrash(_)
                | ModeOpt::NativeVillageAdd => {}
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
            self.gain(f.player, f.subject, Dest::Discard, false, f.source, sink);
        }
    }

    /// Schedule a pending start-of-`p`'s-next-turn Duration effect: `card` resolves `times`
    /// more times (added to an existing argless entry for the same card, so several plays or a
    /// Throne Room/King's Court multiply into one entry, rather than one new entry; an entry
    /// with a real `arg` (Haven/Blockade) never merges, since each carries its own card). See
    /// the Duration framework doc comment at the top of this file.
    pub(crate) fn push_duration_pending(&mut self, p: u8, card: CardId, times: u8, arg: CardId) {
        // Tally against an in-progress Throne Room/King's Court multiplier group, if any (see
        // `Then::PlayPicked` / `FrameKind::MultiplierFinalize`): nothing else can call this while
        // a group's resolutions (and everything they push) are still unwinding, so any call seen
        // here belongs to the active group.
        if self.turn.multiplier_card != 0 {
            self.turn.multiplier_successes += 1;
        }
        // A next-turn effect was actually scheduled: keep this physical card in play past
        // cleanup (see `put`'s `Act::Play` for why this isn't done unconditionally at play
        // time). Over-counting when Throne Room/King's Court multiplies a single physical card
        // (one call per resolution) is harmless: `cleanup` clamps to the real `in_play` count.
        self.turn.duration_held.add(card, 1);
        let ps = &mut self.players[p as usize];
        if arg == 0 {
            for i in 0..ps.pending_durations_len as usize {
                let e = &mut ps.pending_durations[i];
                if e.card == card && e.arg == 0 {
                    e.times = e.times.saturating_add(times);
                    return;
                }
            }
        }
        let i = ps.pending_durations_len as usize;
        assert!(i < crate::state::PENDING_DURATIONS_CAP, "pending duration list overflow");
        ps.pending_durations[i] = crate::state::PendingDuration { card, times, arg, used: false };
        ps.pending_durations_len += 1;
    }

    /// Push a `FrameKind::DurationStart` frame (and, for Clerk, a start-of-turn reaction YesNo
    /// per copy in hand) for `self.turn.player`'s turn, which has just begun. Called once, from
    /// `engine::run_phase`'s `Phase::Action` arm, before any `PlayAction` decision. Cheaply
    /// gated: Base-only games (no Clerk, no pending Duration entries) do nothing here beyond two
    /// field reads.
    pub(crate) fn push_turn_start_frames<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let pi = p as usize;
        // Duration effects resolve first, in the order their cards were played (pushed
        // last-first so entry 0 ends up on top and resolves first); Clerk's own reaction is
        // pushed on top of those (documented choice: Clerk is offered before durations resolve).
        let len = self.players[pi].pending_durations_len;
        if len > 0 {
            let entries = self.players[pi].pending_durations;
            self.players[pi].pending_durations_len = 0;
            for i in (0..len as usize).rev() {
                let e = entries[i];
                self.stack.push(Frame { count: e.times, subject: e.arg, ..Frame::new(K::DurationStart, p, e.card) });
            }
        }
        if self.in_supply(id::CLERK) {
            let n = self.players[pi].hand.get(id::CLERK);
            for _ in 0..n {
                self.stack.push(Frame {
                    zone: Zone::Hand, act: Act::Play, subject: id::CLERK,
                    ..Frame::new(K::YesNo, p, id::CLERK)
                });
            }
        }
        let _ = sink;
    }

    /// Resolve one pending Duration effect at the start of its owner's turn: the generic
    /// (cards, actions, buys, coins) bonus (`cards::duration_bonus`), `times` times, then any
    /// card-specific extra (pushed so it resolves before the generic draw, matching
    /// `resolve_effects_inner`'s "special frames first, +cards draw last" convention).
    pub(crate) fn resolve_duration_start<S: EventSink>(&mut self, p: u8, e: crate::state::PendingDuration, sink: &mut S) {
        let times = e.times;
        match e.card {
            id::HAVEN | id::BLOCKADE => {
                // Put the set-aside/gained card (arg) back into hand, if there was one.
                if e.arg != 0 {
                    let ps = &mut self.players[p as usize];
                    if ps.set_aside.remove(e.arg) {
                        ps.hand.add(e.arg, 1);
                        sink.event(Event::Draw { player: p, card: e.arg });
                    }
                }
            }
            id::TIDE_POOLS | id::SEA_WITCH => {
                self.stack.push(select(p, e.card, Zone::Hand, Act::Discard, Filter::Any, 2, 2, Then::Nothing));
            }
            id::SAILOR => {
                self.stack.push(select(p, e.card, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing));
            }
            id::PIRATE => {
                self.stack.push(gain_frame(p, e.card, 6, self.treasure_filter(), Dest::Hand));
            }
            _ => {} // Outpost: no start-of-turn effect (handled at cleanup/turn transition).
        }
        let (cards_n, actions_n, buys_n, coins_n) = cards::duration_bonus(e.card);
        self.turn.actions += actions_n * times;
        self.turn.buys += buys_n * times;
        self.turn.coins += coins_n as u16 * times as u16;
        if cards_n > 0 && times > 0 {
            self.stack.push(draw_frame(p, e.card, cards_n * times));
        }
    }

    /// A `RevealTop` frame finishing (either it revealed everything asked for, or the deck ran
    /// out): pop it and, for Patrol, move every revealed card matching the filter into hand.
    fn finish_reveal_top<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        self.stack.pop();
        match f.then {
            Then::SeaChartCheck => {
                // Sea Chart: reveal the top card (0 or 1, already in `set_aside`). If its owner
                // already has a copy of it in play, put it into hand; otherwise leave it on top
                // of the deck, now known.
                let p = f.player as usize;
                if let Some((c, _)) = self.players[p].set_aside.iter().next() {
                    self.players[p].set_aside.remove(c);
                    if self.players[p].in_play.has(c) {
                        self.players[p].hand.add(c, 1);
                        sink.event(Event::Draw { player: f.player, card: c });
                    } else {
                        self.players[p].deck_known.push_top(c);
                        sink.event(Event::Topdeck { player: f.player, card: c });
                    }
                }
            }
            Then::MoveMatchingToHand(filter) => {
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
            Then::DiscardRevealedNotMatching => {
                let p = f.player as usize;
                let revealed = self.players[p].set_aside;
                for (c, n) in revealed.iter() {
                    if !f.filter.matches(c) {
                        self.players[p].set_aside.set(c, 0);
                        self.players[p].discard.add(c, n);
                        for _ in 0..n {
                            sink.event(Event::Discard { player: f.player, card: c });
                        }
                    }
                }
            }
            Then::MoveMatchingToDiscard(filter) => {
                // Rabble: revealed Actions and Treasures are discarded automatically (no
                // decision); whatever's left (Victory/Curse cards) is handled by a `Select`
                // frame pushed under this one (Then::Nothing there, ordered Topdeck).
                let p = f.player as usize;
                let revealed = self.players[p].set_aside;
                for (c, n) in revealed.iter() {
                    if filter.matches(c) {
                        self.players[p].set_aside.set(c, 0);
                        self.players[p].discard.add(c, n);
                        for _ in 0..n {
                            sink.event(Event::Discard { player: f.player, card: c });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
