//! Card effects, built from a handful of generic frames (see `state::FrameKind`):
//! `Draw`, `Gain`, `Select` (pick cards from a zone to discard/trash/topdeck/play, with a
//! continuation), `RevealTop`, `YesNo`, `PlayEffects`, plus two small bespoke loops
//! (`Vassal`, `Library`). Adding a card usually means composing these in its set's `SetRules::play` (`sets/<set>.rs`; this file keeps the generic
//! machinery).
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
//! Attacks and reactions: playing an Attack that another player could react to (a Moat in their
//! hand, or a Diplomat) pushes a `FrameKind::AttackBody` frame holding the card, with the reaction
//! window (`push_reaction_window`: one `YesNo` per Reaction per player, in turn order) above it. The
//! card's own effects (`resolve_card_body`) run only once every player has answered, so a Moat
//! revealed (`Then::BlockAttack`, recorded in `TurnState::moated`) is known to `victims`, and one a
//! Diplomat drew is still offered. With no such player there is no window and the card resolves at
//! once. A card gained straight onto the deck is reachable by reactions as `Zone::DeckTop`.
//!
//! ## The Duration framework (Seaside step 3)
//!
//! A Duration card's "now" part plays exactly like any other card (vanilla bonuses via
//! `CardDef`, plus an arm in its set's `SetRules::play` for anything else: Blockade's gain, Sea Witch's attack,
//! Tactician's conditional discard). Its "next turn" part is scheduled as a
//! `state::PendingDuration` entry in the player's `PlayerState::pending_durations` (pushed by
//! `push_duration_pending`, called generically at the tail of `resolve_card_body` for every
//! Duration card except Haven/Blockade/Tactician, which schedule themselves once their own
//! argument — the set-aside/gained card, or whether the discard condition held — is known) and
//! resolved by `GameState::resolve_duration_start` at the very start of the owner's next turn.
//! `GameState::push_turn_start_frames` (called from `engine::run_phase`'s `Phase::Action` arm)
//! moves every pending entry, plus Clerk's play and each Prince's card (Promo), into
//! `TurnState::start_pending` and pushes one `FrameKind::StartOrder` frame over them. Each time it
//! runs it takes the next effect (`run_start_order`): the player is asked which only when two or
//! more different effects remain and one is order-sensitive (`cards::start_order_sensitive`:
//! acts on the hand, adds to the hand, or plays a card), otherwise the first one goes. The chosen
//! effect resolves fully (its frames sit above `StartOrder`) before the next is taken.
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
use crate::state::{Act, Dest, Filter, Frame, FrameKind as K, GameState, Then, Zone, ADV_MINUS_CARD, G_CHAMPION, G_PLAY_TOKENS, MAX_PLAYERS};

/// The most coins an overpay decision offers (and the most it can offer in one go).
pub const OVERPAY_CAP: u16 = 60;

macro_rules! take_top {
    ($s:expr, $p:expr, $sink:expr) => {
        match $s.take_top($p, $sink) {
            Ok(c) => c,
            Err(NeedChance(p)) => return Run::Chance(p),
        }
    };
}
pub(crate) use take_top;

/// A zone's cards: borrowed from the state, or (the deck top) built on the spot.
enum ZoneRef<'a> {
    Ref(&'a Counts),
    Top(Counts),
}

impl std::ops::Deref for ZoneRef<'_> {
    type Target = Counts;
    #[inline(always)]
    fn deref(&self) -> &Counts {
        match self {
            ZoneRef::Ref(c) => c,
            ZoneRef::Top(c) => c,
        }
    }
}

/// Pick up to `max` (at least `min`, clamped to what's available) cards from `zone`.
pub(crate) fn select(p: u8, source: CardId, zone: Zone, act: Act, filter: Filter, min: u8, max: u8, then: Then) -> Frame {
    Frame { zone, act, filter, min, max, then, ..Frame::new(K::Select, p, source) }
}

pub(crate) fn gain_frame(p: u8, source: CardId, max_cost: u8, filter: Filter, dest: Dest) -> Frame {
    Frame { max: max_cost, filter, dest, ..Frame::new(K::Gain, p, source) }
}

pub(crate) fn draw_frame(p: u8, source: CardId, n: u8) -> Frame {
    Frame { max: n, ..Frame::new(K::Draw, p, source) }
}

/// Choose `picks` options from `cards::modes(source)` (see `state::FrameKind::Mode`).
pub(crate) fn mode_frame(p: u8, source: CardId, picks: u8) -> Frame {
    Frame { max: picks, ..Frame::new(K::Mode, p, source) }
}

pub(crate) const ALL: u8 = u8::MAX;

/// The card choices of a `Select`: every eligible card when `ordered`, else the canonical
/// (non-decreasing) picks from `lo` that leave `need` more available.
#[inline(always)]
fn select_choices(z: &Counts, ordered: bool, lo: CardId, need: u32, ok: impl Fn(CardId) -> bool, out: &mut ChoiceBuf) {
    if ordered {
        for (c, _) in z.iter() {
            if ok(c) {
                out.push(Choice::Card(c));
            }
        }
    } else {
        canonical_picks(z, lo, need, ok, out);
    }
}

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
        // Adventures: the calls that follow an Action's play (Coin of the Realm, Royal Carriage) wait under its effects.
        if self.after_play_possible(player) && cards::is(card, ACTION) {
            self.stack.push(Frame { subject: card, ..Frame::new(K::AfterPlay, player, card) });
        }
        self.resolve_effects_inner(card, player, sink);
        // The tokens on the card's pile: the bonus comes before anything the card does (the +1 Card token's draw is on top of the stack).
        if self.adv & G_PLAY_TOKENS != 0 {
            self.play_tokens(card, player, sink);
        }
        for f in &mut self.stack.frames[base..self.stack.len as usize] {
            f.depth = depth + 1;
        }
    }

    fn resolve_effects_inner<S: EventSink>(&mut self, card: CardId, player: u8, sink: &mut S) {
        let p = player;
        let def = cards::def(card);
        // Enchantress: the first Action the turn's player plays on their turn gives +1 Card +1 Action instead of what it says.
        if self.emp.flags & crate::state::E_ENCHANT != 0 && p == self.turn.player && cards::is(card, ACTION) {
            if self.enchanted_play(card, p, sink) {
                return;
            }
        }
        // What this turn's player has played (Conspirator counts Actions, Quarry and Hoard their own
        // plays): not what another player plays from their hand on this turn (Pirate).
        // The bonuses are the turn's: a card another player plays on this turn (Pirate, Caravan Guard) gives them nothing.
        if p == self.turn.player {
            self.turn.played.add(card, 1);
            self.turn.actions += def.actions;
            self.turn.buys += def.buys;
            self.turn.coins += def.coins as u16;
            self.turn.potions += def.potions;
            // Champion: when you play an Action card, you first get +1 Action (per Champion in play, this one only once it is out).
            if self.adv & G_CHAMPION != 0 && cards::is(card, ACTION) {
                let champions = self.players[p as usize].in_play.get(id::CHAMPION).saturating_sub((card == id::CHAMPION) as u8);
                self.turn.actions += champions;
            }
        }
        // A Treasure's printed value is shown by its own play line; Action cards' +$ is a bonus.
        let shown_coins = if cards::is(card, TREASURE) && !cards::is(card, cards::DURATION) { 0 } else { def.coins as u16 };
        sink.bonus(p, card, def.actions, def.buys, shown_coins, 0);

        // An Attack opens a reaction window first: each other player may reveal Moat (and Diplomat)
        // before the Attack does anything, so its own effects wait (`FrameKind::AttackBody`) under
        // the window. With nobody holding a Reaction there is no window and no wait.
        if cards::is(card, cards::ATTACK) {
            self.turn.moated = 0;
            // Urchin (in play): the player may first trash it for a Mercenary, before the reactions.
            let urchin = self.in_supply(id::URCHIN) && self.urchin_offer_possible(card, p);
            if urchin || self.reaction_window_open(p) {
                self.stack.push(Frame { subject: card, ..Frame::new(K::AttackBody, p, card) });
                self.push_reaction_window(p, card);
                if urchin {
                    self.stack.push(Frame {
                        zone: Zone::InPlay, act: Act::Trash, subject: id::URCHIN, then: Then::UrchinMercenary,
                        ..Frame::new(K::YesNo, p, id::URCHIN)
                    });
                }
                return;
            }
        }
        self.resolve_card_body(card, p, sink);
    }

    /// Everything a played card does beyond its vanilla bonuses (see `resolve_effects_inner`).
    fn resolve_card_body<S: EventSink>(&mut self, card: CardId, p: u8, sink: &mut S) {
        let def = cards::def(card);
        crate::sets::play(self, def.set, card, p, sink);

        // Generic Duration scheduling: every Duration card gets a pending start-of-next-turn
        // entry, except those that schedule themselves once their own argument (the set-aside/gained card, or whether
        // the "if you have a card in hand" condition held) is known, or have no pending entry at all
        // (`SetRules::schedules_own_duration`: Haven, Blockade, Tactician, Prince).
        if cards::is(card, cards::DURATION) && !crate::sets::schedules_own_duration(def.set, card) {
            self.push_duration_pending(p, card, 1, 0);
        }

        if def.cards > 0 {
            self.stack.push(draw_frame(p, card, def.cards));
        }
    }

    /// Whether any other player has a Reaction to offer against an Attack now: a Moat to reveal (no
    /// Lighthouse already protecting them), or a card that draws (Diplomat, with enough cards in
    /// hand) and so may bring a Moat into the hand.
    fn reaction_window_open(&self, attacker: u8) -> bool {
        // Cheap exit for the many games with no such Reaction in the kingdom.
        if !cards::BLOCK_REACTIONS.iter().any(|&c| self.in_supply(c))
            && !cards::REACTION_EFFECTS.iter().any(|&(c, ..)| self.in_supply(c))
            && !cards::ATTACK_REACTIONS.iter().any(|&(c, ..)| self.in_supply(c))
        {
            return false;
        }
        self.others(attacker).any(|v| {
            let ps = &self.players[v as usize];
            cards::ATTACK_REACTIONS.iter().any(|&(c, ..)| ps.hand.has(c))
                || (!self.protected_while_in_play(v) && cards::BLOCK_REACTIONS.iter().any(|&c| ps.hand.has(c)))
                || cards::REACTION_EFFECTS.iter().any(|&(c, min_hand, _, _)| ps.hand.has(c) && ps.hand.total() >= min_hand as u32)
        })
    }

    /// Open a reaction window for an Attack just played: every other player is offered a `YesNo`
    /// for each Reaction they may reveal, in turn order from the attacker's left. A player's
    /// Diplomat comes first (it can be revealed again once it has resolved, if the hand still
    /// allows it, see the `YesNo` arm of `run_frame`), so that a Moat it draws can still be
    /// revealed; then each blocking Reaction (Moat: `Then::BlockAttack`, naming the Attack as the
    /// source), skipped when the player is already protected by a Lighthouse in play or holds none
    /// by then. Pushed in reverse so the first reaction resolves first.
    fn push_reaction_window(&mut self, attacker: u8, attack: CardId) {
        let n = self.num_players;
        for i in (1..n).rev() {
            let v = (attacker + i) % n;
            let ps = &self.players[v as usize];
            // A card that draws may bring a Moat into the hand (Diplomat, Guard Dog played in response).
            let draws = cards::REACTION_EFFECTS.iter().any(|&(c, min_hand, _, _)| ps.hand.has(c) && ps.hand.total() >= min_hand as u32)
                || cards::ATTACK_REACTIONS.iter().any(|&(c, act, _)| act == Act::Play && ps.hand.has(c));
            if !self.protected_while_in_play(v) && (draws || cards::BLOCK_REACTIONS.iter().any(|&c| ps.hand.has(c))) {
                for &block in cards::BLOCK_REACTIONS {
                    self.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Reveal, subject: block, then: Then::BlockAttack,
                        ..Frame::new(K::YesNo, v, attack)
                    });
                }
            }
            // Beggar: discard it to gain two Silvers (a Moat or Lighthouse does not stop it: it protects against the Attack's effects, not
            // against choosing to react).
            for &(react_card, act, then) in cards::ATTACK_REACTIONS {
                if self.players[v as usize].hand.has(react_card) {
                    self.stack.push(Frame { zone: Zone::Hand, act, subject: react_card, then, ..Frame::new(K::YesNo, v, attack) });
                }
            }
            for &(react_card, min_hand, draw, discard) in cards::REACTION_EFFECTS {
                let hand = &self.players[v as usize].hand;
                if hand.get(react_card) > 0 && hand.total() >= min_hand as u32 {
                    self.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Reveal, subject: react_card,
                        then: Then::ReactDrawDiscard { draw, discard }, min: min_hand,
                        ..Frame::new(K::YesNo, v, react_card)
                    });
                }
            }
        }
    }

    /// Other players affected by an attack, in turn order starting to the left.
    /// Reactions were resolved when the attack was played (`push_reaction_window`).
    pub(crate) fn victims<S: EventSink>(&self, _sink: &mut S) -> ([u8; MAX_PLAYERS], usize) {
        let mut out = [0u8; MAX_PLAYERS];
        let mut n = 0;
        for v in self.others(self.turn.player) {
            if !self.immune(v) {
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
    pub(crate) fn push_gain_to_each(&mut self, victims: &[u8], card: CardId, source: CardId) {
        for &v in victims.iter().rev() {
            self.stack.push(Frame { filter: Filter::Card(card), ..gain_frame(v, source, u8::MAX, Filter::Card(card), Dest::Discard) });
        }
    }

    /// A non-Duration Action costing up to `n` now (Prince's pick: Bridge and Quarry count), no Potion.
    /// `Filter::matches` can only use the printed cost, so selections with this filter test it
    /// here. The selection helpers are generic over the test so every other filter keeps its
    /// plain `Filter::matches` loop.
    fn action_cost_ok(&self, n: u8, c: CardId) -> bool {
        cards::is(c, ACTION) && !cards::is(c, cards::DURATION) && self.cost(c) <= n && !cards::potion_cost(c) && cards::debt_cost(c) == 0
    }

    fn zone(&self, p: u8, z: Zone) -> ZoneRef<'_> {
        match z {
            Zone::Supply => ZoneRef::Ref(&self.supply),
            Zone::Trash => ZoneRef::Ref(&self.trash),
            _ => {
                let ps = &self.players[p as usize];
                ZoneRef::Ref(match z {
                    Zone::Hand => &ps.hand,
                    Zone::Discard => &ps.discard,
                    Zone::Revealed => &ps.set_aside,
                    Zone::InPlay => &ps.in_play,
                    // The known top card of the deck, as a one-card zone.
                    Zone::DeckTop => {
                        let mut top = Counts::EMPTY;
                        if let Some(c) = ps.deck_known.peek_top() {
                            top.add(c, 1);
                        }
                        return ZoneRef::Top(top);
                    }
                    Zone::Tavern => return ZoneRef::Top(ps.tavern.counts()),
                    Zone::Archive => return ZoneRef::Top(ps.archive.counts()),
                    Zone::Supply | Zone::Trash => unreachable!(),
                })
            }
        }
    }

    /// Take one `c` out of `z` for `p`; whether there was one (the deck top: only if it is the top card).
    fn zone_take(&mut self, p: u8, z: Zone, c: CardId) -> bool {
        if z == Zone::DeckTop {
            let ps = &mut self.players[p as usize];
            if ps.deck_known.peek_top() == Some(c) {
                ps.deck_known.pop_top();
                return true;
            }
            return false;
        }
        if z == Zone::Tavern {
            return self.players[p as usize].tavern.remove(c);
        }
        if z == Zone::Archive {
            return self.players[p as usize].archive.remove(c);
        }
        self.zone_mut(p, z).remove(c)
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
                    Zone::DeckTop | Zone::Tavern | Zone::Archive | Zone::Supply | Zone::Trash => unreachable!("{z:?} is not a counted zone"),
                }
            }
        }
    }

    /// (remaining min, remaining max, eligible cards at/above the canonical bound) for a Select.
    fn select_bounds(&self, f: &Frame) -> (u8, u8, u32) {
        self.select_bounds_by(f, |c| self.select_ok(f, c))
    }

    /// Whether `c` may be picked by the selection `f`: its filter, and (Inheritance) an Estate in the hand of a player whose Estates are
    /// Action cards for a pick of an Action card.
    #[inline(always)]
    fn select_ok(&self, f: &Frame, c: CardId) -> bool {
        // From the Supply only the top card of a pile can be taken (not a split pile's bottom half, nor a Castle behind a cheaper one).
        (f.zone != Zone::Supply || self.pile_open(c)) && self.filter_ok(f.player, f.filter, c)
            || (c == id::ESTATE && f.zone == Zone::Hand && f.filter == Filter::Action && self.estates_are_actions(f.player))
    }

    /// Whether `c` passes `filter` for player `p` right now: the filters that read the state are
    /// decided here (the cost of an Action now, the live hand and play area, and "Treasure",
    /// which includes Curse in games with Charlatan), the rest by `Filter::matches`.
    pub(crate) fn filter_ok(&self, p: u8, filter: Filter, c: CardId) -> bool {
        match filter {
            Filter::Treasure => self.is_treasure(c),
            Filter::NonCopperTreasure => c != id::COPPER && self.is_treasure(c),
            Filter::ActionOrTreasure => cards::is(c, ACTION) || self.is_treasure(c),
            Filter::TreasureNotReward => self.is_treasure(c) && !cards::is(c, cards::REWARD),
            // A Duration Treasure only counts for the copies this cleanup discards: those played
            // on an earlier turn, not the ones held for the next.
            Filter::TreasureToDiscard => {
                self.is_treasure(c)
                    && (!cards::is(c, cards::DURATION) || self.players[p as usize].in_play.get(c) > self.turn.duration_held.get(c))
            }
            Filter::ActionCostUpTo(n) => self.action_cost_ok(n, c),
            Filter::NonTreasure => !self.is_treasure(c),
            Filter::TreasureNonDuration => self.is_treasure(c) && !cards::is(c, cards::DURATION),
            // Costs are the current costs (Bridge and Quarry count).
            Filter::CostAtLeast(n) => self.cost(c) >= n,
            Filter::CostRange(lo, hi) => !cards::potion_cost(c) && cards::debt_cost(c) == 0 && (lo..=hi).contains(&self.cost(c)),
            Filter::GainableCopy => self.in_supply(c) && self.supply.get(c) > 0 && self.pile_open(c),
            Filter::InheritCard => self.action_cost_ok(4, c) && !cards::is(c, cards::COMMAND),
            Filter::CommandCostUpTo(n) => self.action_cost_ok(n, c) && !cards::is(c, cards::COMMAND),
            Filter::NotInHand => !self.players[p as usize].hand.has(c),
            Filter::TreasureNot(x) => c != x && self.is_treasure(c),
            // Raider: a copy of a card the Attack's player has in play (the turn's player).
            Filter::CopyInAttackersPlay => self.players[self.turn.player as usize].in_play.has(c),
            // Necromancer: a non-Duration Action in the trash, not already turned face down this turn.
            Filter::NecroTarget => {
                cards::is(c, ACTION) && !cards::is(c, cards::DURATION) && self.trash.get(c) > self.turn.noc.necro.get(c)
            }
            Filter::NotInPlayAction => cards::is(c, ACTION) && !self.players[p as usize].in_play.has(c),
            Filter::ActionToDiscard => {
                let ps = &self.players[p as usize];
                cards::is(c, ACTION) && ps.in_play.get(c) > self.turn.duration_held.get(c).saturating_add(ps.stays.get(c))
            }
            filter => filter.matches(c),
        }
    }

    #[inline(always)]
    fn select_bounds_by(&self, f: &Frame, ok: impl Fn(CardId) -> bool) -> (u8, u8, u32) {
        let z = self.zone(f.player, f.zone);
        let lo = if f.ordered || f.count == 0 { 0 } else { f.last + f.distinct as CardId };
        let mut avail_all = 0u32;
        let mut avail_from = 0u32;
        for (c, n) in z.iter() {
            if ok(c) {
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
                // The -1 Card token: the first time this frame draws, one card fewer (and the token goes).
                if f.count == 0 {
                    f.count = 1;
                    if self.players[pi].adv & ADV_MINUS_CARD != 0 {
                        self.players[pi].adv &= !ADV_MINUS_CARD;
                        f.max -= 1;
                        if f.max == 0 {
                            self.stack.pop();
                            return Run::Continue;
                        }
                        self.stack.set_top(f);
                    }
                }
                // The whole draw in one go: nothing between two draws of the frame looks at anything but the deck and the hand. The frame slot
                // is written as the one-card-per-step version left it (after a draw but the last, and when the deck runs out or needs a chance
                // outcome): a popped frame stays in the array beyond `len`, and `PlayerView::stable_seed` hashes the whole array.
                let mut drew = false;
                loop {
                    match self.take_top(p, sink) {
                        Err(NeedChance(q)) => {
                            if drew {
                                self.stack.set_top(f);
                            }
                            return Run::Chance(q);
                        }
                        Ok(None) => {
                            if drew {
                                self.stack.set_top(f);
                            }
                            self.stack.pop();
                            break;
                        }
                        Ok(Some(c)) => {
                            self.players[pi].hand.add(c, 1);
                            sink.event(Event::Draw { player: p, card: c });
                            if f.max == 1 {
                                if drew {
                                    self.stack.set_top(f);
                                }
                                self.stack.pop();
                                break;
                            }
                            f.max -= 1;
                            drew = true;
                        }
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
                        if self.filter_ok(p, f.filter, c) {
                            f.count += 1;
                            f.last = c;
                        }
                        if f.count >= f.max { self.finish_reveal_top(f, sink); } else { self.stack.set_top(f); }
                    }
                }
                Run::Continue
            }
            K::NativeVillageAdd => self.run_native_village_add(f, sink),
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
                // Played from the discard pile where it stays (Infirmary's overpay): the first play
                // is logged here, the card did not move.
                if f.zone == Zone::Discard && f.count == 1 {
                    sink.event(Event::Play { player: p, card: f.subject });
                }
                if f.count >= 2 {
                    sink.event(Event::PlayAgain { player: p, card: f.subject, source: f.source, nth: f.count });
                }
                if f.zone == Zone::Supply || f.zone == Zone::Trash {
                    // Played from the Supply (Captain) or the trash (Necromancer): the card stays there, so it is not in play and a card
                    // that tries to move itself fails to (`prince_play`, shared with Prince's held card).
                    sink.event(Event::StartPlay { player: p, source: f.source, card: f.subject });
                    self.turn.prince_play = true;
                    self.resolve_effects(f.subject, p, f.depth, sink);
                    self.turn.prince_play = false;
                    return Run::Continue;
                }
                self.resolve_effects(f.subject, p, f.depth, sink);
                Run::Continue
            }
            K::Gain => {
                // Young Witch: a player who reveals a Bane from their hand is not given the Curse.
                if f.gf & crate::state::GF_BANE_BLOCKS != 0 && self.bane != 0 && self.players[pi].hand.has(self.bane) {
                    sink.event(Event::Reaction { player: p, card: self.bane });
                    self.stack.pop();
                    return Run::Continue;
                }
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
                    let avail: u32 = z.iter().filter(|&(c, _)| self.select_ok(&f, c)).map(|(_, n)| n as u32).sum();
                    // Threshold form (`exact`, Clerk): pick one card if the zone holds `max` or more.
                    let excess = if f.exact { u32::from(avail >= f.max as u32) } else { avail.saturating_sub(f.max as u32) } as u8;
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
                // A hand-size reaction (Diplomat: `min` is the hand it needs) is only on offer
                // while the hand still has that many cards, which is what lets it be revealed
                // again after it resolved, as often as another copy and the hand allow.
                if self.zone(p, f.zone).get(f.subject) == 0
                    || (f.subject == id::WALLED_VILLAGE && self.walled_village_blocked(p))
                    || (matches!(f.then, Then::ReactDrawDiscard { .. }) && self.players[pi].hand.total() < f.min as u32)
                    || (f.then == Then::BlockAttack && self.turn.moated & (1 << p) != 0)
                    || (f.act == Act::Exchange && !self.exchange_possible(f.subject))
                    || (matches!(f.then, Then::TraderSwap | Then::BerserkerPlay) && !self.hint_yesno_possible(&f))
                {
                    self.stack.pop();
                    self.yesno_decline(f, sink);
                    return Run::Continue;
                }
                // A Berserker gained with an Action in play is played, whatever its player thinks.
                if f.then == Then::BerserkerPlay {
                    self.apply_frame(f, Choice::Yes, sink);
                    return Run::Continue;
                }
                Run::Decide(DecisionKind::YesNo { act: f.act }, f.subject)
            }
            K::Vassal => self.run_vassal(f, sink),
            K::Mode => {
                if f.count >= f.max {
                    self.finish_mode(f, sink);
                    return Run::Continue;
                }
                Run::Decide(DecisionKind::Mode { picks: f.max - f.count, distinct: f.max > 1 }, f.subject)
            }
            K::Library => self.run_library(f, sink),
            K::Ask => {
                // Nothing to put into the discard pile / nothing was revealed: nothing to ask.
                let pointless = match f.then {
                    Then::DeckToDiscard => self.players[pi].deck_size() == 0,
                    Then::DiscardRevealedAsk => self.players[pi].set_aside.is_empty(),
                    _ => false,
                };
                if pointless {
                    self.stack.pop();
                    return Run::Continue;
                }
                Run::Decide(DecisionKind::YesNo { act: f.act }, f.subject)
            }
            K::Name => {
                // `f.zone == Zone::Supply` is War Chest's variant of the generic Name frame: the
                // player to the left names any supply card (not a guess about a deck), and once
                // named there's no "reveal and compare" step — it's just recorded, and a Gain
                // frame (excluding every name so far this turn) follows. Otherwise this is
                // Wishing Well's "name a card, reveal the top, draw it if it matches."
                let war_chest = f.zone == Zone::Supply;
                let journeyman = f.then == Then::JourneymanDraw;
                if f.count == 0 {
                    let has_options = if war_chest {
                        self.supply_cards().any(|c| self.supply.get(c) > 0)
                    } else if journeyman || f.then == Then::RebuildNamed {
                        true
                    } else {
                        !self.name_offer(p).is_empty()
                    };
                    if !has_options {
                        self.stack.pop();
                        return Run::Continue;
                    }
                    return Run::Decide(DecisionKind::Name, 0);
                }
                if f.then == Then::RebuildNamed {
                    // Named: reveal until a Victory card that was not named (discarding the rest), trash it, and gain a Victory card
                    // costing up to $3 more (the trash's own effects come before the gain).
                    self.stack.pop();
                    self.stack.push(Frame {
                        depth: f.depth,
                        ..select(
                            p, f.source, Zone::Revealed, Act::Trash, Filter::Any, 1, 1,
                            Then::GainUpTo { plus: 3, filter: Filter::Victory, dest: Dest::Discard, exact: false, dest_by_type: false },
                        )
                    });
                    self.stack.push(Frame {
                        depth: f.depth, filter: Filter::VictoryNot(f.subject), max: 1, then: Then::DiscardRevealedNotMatching,
                        ..Frame::new(K::RevealUntil, p, f.source)
                    });
                    return Run::Continue;
                }
                if journeyman {
                    // Named: reveal until 3 cards without that name; they go to the hand, the rest are discarded.
                    self.stack.pop();
                    self.stack.push(Frame {
                        depth: f.depth, filter: Filter::NotCard(f.subject), max: 3, then: Then::KeepMatchingDiscardRest,
                        ..Frame::new(K::RevealUntil, p, f.source)
                    });
                    return Run::Continue;
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
            K::DeckPosition => self.run_deck_position(f),
            K::TrashTopThenGain => self.run_swindler(f, sink),
            K::PassLeftBegin => self.run_pass_left_begin(f),
            K::PassLeftDeliver => {
                self.stack.pop();
                self.deliver_left_passes(sink);
                Run::Continue
            }
            K::StartOrder => self.run_start_order(sink),
            K::TopCard if matches!(f.then, Then::GiantReveal | Then::WarriorHit) => self.run_adv_top_card(f, sink),
            K::TopCard if f.then == Then::ChariotRace => self.run_chariot(f, sink),
            K::Bid => self.run_bid(f, sink),
            K::TopCard if matches!(f.then, Then::LocustsHit | Then::MasonTrash) => self.run_noc_top_card(f, sink),
            K::TopCard => self.run_jester_top_card(f, sink),
            K::ReceiveBoon => self.run_receive_boon(f, sink),
            K::ReceiveHex => self.run_receive_hex(f, sink),
            K::PickBoon => self.run_pick_boon(f, sink),
            K::AfterPlay => self.run_after_play(f),
            K::Token => self.run_token(f),
            K::Overpay => self.run_overpay(f),
            K::AttackBody => {
                self.stack.pop();
                let base = self.stack.len as usize;
                self.resolve_card_body(f.subject, p, sink);
                for fr in &mut self.stack.frames[base..self.stack.len as usize] {
                    fr.depth = f.depth;
                }
                Run::Continue
            }
            K::MultiplierFinalize => {
                self.stack.pop();
                // Throne Room / King's Court stays in play with the Duration it played if *any*
                // of its plays keeps that Duration in play ("even if only one play of the
                // Duration card is keeping it in play, Throne Room stays in play with it").
                if self.turn.multiplier_card == f.source
                    && self.turn.multiplier_successes >= 1
                    && matches!(f.subject, id::PRINCE | id::HIRELING | id::CHAMPION)
                {
                    // A Throne Room / King's Court whose Prince set cards aside stays in play with
                    // it for the rest of the game ("both will stay in play").
                    let ps = &mut self.players[p as usize];
                    if ps.stays.get(f.source) < ps.in_play.get(f.source) {
                        ps.stays.add(f.source, 1);
                    }
                } else if self.turn.multiplier_card == f.source && self.turn.multiplier_successes >= 1 {
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

    pub(crate) fn frame_choices(&self, f: Frame, out: &mut ChoiceBuf) {
        match f.kind {
            K::Gain if f.gf & crate::state::GF_FROM_SIDE != 0 => {
                // The Rewards (outside the Supply): any one that is left.
                for i in 0..cards::NUM_REWARDS {
                    if self.rewards[i] > 0 {
                        out.push(Choice::Card(cards::FIRST_REWARD + i as CardId));
                    }
                }
            }
            K::Gain if f.gf & crate::state::GF_SPIRITS != 0 => {
                // Exorcist: a Spirit pile with a card left that costs less than the card trashed (`max`, with `potion` for a Potion in it).
                for c in [id::WILL_O_WISP, id::IMP, id::GHOST] {
                    let cost = self.cost(c);
                    if self.noc.piles[(c - id::WILL_O_WISP) as usize] > 0 && cost <= f.max && !(cost == f.max && !f.potion) {
                        out.push(Choice::Card(c));
                    }
                }
            }
            K::Gain => {
                for c in self.supply_cards() {
                    // Potion: "up to $X" admits Potion-cost cards only when the reference cost has a
                    // Potion; "exactly" needs the Potion to match; "less than" is no part larger and not equal.
                    let pc = cards::potion_cost(c);
                    let dc = cards::debt_cost(c);
                    let cost = self.cost(c);
                    // A card with Debt in its cost is never "up to" or "less than" an amount of $; an exact gain needs the same Debt (`min`, 0 for
                    // an ordinary gain); Change (`more`) only compares the $.
                    let cost_ok = if f.more {
                        cost > f.max
                    } else if f.exact {
                        (cost == f.max || (f.gf & crate::state::GF_BOTH != 0 && cost == f.max + 2)) && pc == f.potion && dc == f.min
                    } else if f.less {
                        cost <= f.max && (f.potion || !pc) && dc == 0 && !(cost == f.max && pc == f.potion)
                    } else {
                        cost <= f.max && (f.potion || !pc) && dc == 0
                    };
                    let named_ok = !f.excl_named || !self.turn.named_for_war_chest.has(c);
                    // Smugglers: further restricted to cards the player to `f.player`'s right
                    // gained on their last turn.
                    let record_ok = !f.gain_from_record || self.players[self.right_of(f.player) as usize].last_turn_gains.has(c);
                    // Only the top card of a split pile or of the Castles can be gained.
                    let top_ok = self.pile_open(c);
                    if self.supply.get(c) > 0 && top_ok && cost_ok && named_ok && record_ok && self.filter_ok(f.player, f.filter, c) {
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
                let lo = if f.count == 0 { 0 } else { f.last + f.distinct as CardId };
                select_choices(&z, f.ordered, lo, need, |c| self.select_ok(&f, c), out);
                if rem_min == 0 {
                    out.push(Choice::Pass);
                }
            }
            K::YesNo | K::Library | K::Ask | K::ReceiveBoon => {
                out.push(Choice::Yes);
                out.push(Choice::No);
            }
            K::PickBoon => self.pick_boon_choices(f, out),
            K::ReceiveHex => {}
            K::Mode => {
                // Every option is always legal to pick (a player may choose one they can't fully
                // perform, e.g. Torturer with fewer than 2 cards in hand); only already-chosen
                // indices (the `min` bitmask) are excluded, so a multi-pick decision offers each
                // remaining index in increasing order.
                let (lo, hi) = match f.then {
                    Then::ModeWindow { first, count } => (first as usize, (first + count) as usize),
                    _ => (0, usize::MAX),
                };
                for (i, _) in cards::modes(f.source).iter().enumerate() {
                    if f.min & (1 << i) == 0 && i >= lo && i < hi {
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
            K::Name if f.then == Then::JourneymanDraw => {
                for (c, _) in self.journeyman_offer(f.player).iter() {
                    out.push(Choice::Card(c));
                }
            }
            K::Name if f.then == Then::RebuildNamed => self.rebuild_offer(out),
            K::Name => {
                for (c, _) in self.name_offer(f.player).iter() {
                    out.push(Choice::Card(c));
                }
            }
            K::Bid => self.bid_choices(out),
            K::DeckPosition => self.deck_position_choices(f, out),
            K::Draw | K::RevealTop | K::PlayEffects | K::Vassal | K::TrashTopThenGain | K::PassLeftBegin | K::PassLeftDeliver | K::MultiplierFinalize | K::AttackBody | K::NativeVillageAdd | K::RevealUntil | K::TopCard => {}
            K::StartOrder => self.start_order_choices(out),
            K::Overpay => self.overpay_choices(f, out),
            K::AfterPlay => {}
            K::Token => self.token_choices(f, out),
        }
    }

    pub(crate) fn apply_frame<S: EventSink>(&mut self, mut f: Frame, choice: Choice, sink: &mut S) {
        sink.depth(f.depth);
        let p = f.player;
        let pi = p as usize;
        match (f.kind, choice) {
            (K::Gain, Choice::Card(c)) => {
                self.stack.pop();
                // Butcher: the Coffers a card costing more than the base needs are spent now.
                if f.pay_coffers {
                    let spend = self.cost(c).saturating_sub(f.min) as u16;
                    if spend > 0 {
                        self.players[pi].coffers -= spend;
                        sink.event(Event::SpendCoffers { player: p, n: spend });
                    }
                }
                // Develop: once one of the two costs is gained, a gain of exactly the other follows (under the triggers of the first).
                if f.gf & crate::state::GF_BOTH != 0 {
                    let other = if self.cost(c) == f.max { f.max + 2 } else { f.max };
                    self.stack.push(Frame { gf: 0, max: other, ..f });
                }
                // Soothsayer: a player who gains the Curse draws a card; the draw goes below the gain's own
                // reactions (Watchtower) so they resolve first.
                if f.then == Then::DrawIfGained && self.supply.get(c) > 0 {
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 1) });
                }
                // Replace: the card is gained to the discard pile as usual; an Action or Treasure is
                // then moved onto the deck if it is still there once the gain's reactions have
                // run (Watchtower, Tiara). The move waits under them.
                let dest = f.dest;
                if f.then == Then::GainedTypeDestAttack && self.supply.get(c) > 0 && (cards::is(c, ACTION) || self.is_treasure(c)) {
                    self.stack.push(Frame {
                        depth: f.depth, subject: c, min: self.players[pi].discard.get(c) + 1,
                        ..select(p, f.source, Zone::Discard, Act::Topdeck, Filter::Any, 0, 0, Then::MoveSubjectIfPresent)
                    });
                }
                // Reactions the gain triggers (Watchtower) belong to this card effect: log them
                // at its depth.
                let base = self.stack.len as usize;
                let gained = if f.gf & (crate::state::GF_FROM_SIDE | crate::state::GF_SPIRITS) != 0 {
                    self.gain_side(p, c, dest, f.source, sink)
                } else {
                    self.gain(p, c, dest, false, f.source, sink)
                };
                for fr in &mut self.stack.frames[base..self.stack.len as usize] {
                    fr.depth = f.depth;
                }
                if gained {
                    match f.then {
                        Then::MessengerCopies => {
                            // Each other player, in turn order, gains a copy.
                            let mut buf = [0u8; MAX_PLAYERS];
                            let mut n = 0;
                            for q in self.others(p) {
                                buf[n] = q;
                                n += 1;
                            }
                            self.push_gain_to_each(&buf[..n], c, f.source);
                        }
                        Then::TokenOnGained { tok } => self.place_token(p, tok as usize, cards::pile_of(c), sink),
                        Then::GainedTypeStatBonus => {
                            let (a, k) = (cards::is(c, ACTION) as u8, self.is_treasure(c) as u16);
                            self.turn.actions += a;
                            self.turn.coins += k;
                            sink.bonus(p, f.source, a, 0, k, 0);
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
                        // Horn of Plenty: a Victory card gained trashes the Horn (if it is still in play).
                        Then::HornTrash if cards::is(c, VICTORY) => {
                            if self.players[pi].in_play.remove(f.source) {
                                self.trash_card(p, f.source, true, sink);
                                self.flush_trash_triggers(f.depth, sink);
                            }
                        }
                        Then::ChangeDebt | Then::DominateVp | Then::TriumphVp | Then::RitualTrash | Then::TakePileVp => {
                            self.empires_gained(&f, c, sink)
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
                                Dest::DeckTop => ps.deck_known.peek_top() == Some(c) && ps.deck_known.pop_top().is_some(),
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
                // Record the pick before the card moves: a gain from the trash can push frames (gain reactions) above this one.
                f.count += 1;
                f.last = c;
                if f.then == Then::WitchsHut && cards::is(c, ACTION) {
                    f.subject += 1;
                }
                self.stack.set_top(f);
                // A card played from the Supply stays there (Captain); it is neither moved nor logged
                // as played until its `PlayEffects` frame runs.
                // Storyteller's Treasure is played from the hand by its own path.
                let storyteller = f.then == Then::StorytellerPlay;
                // Crown in the Buy phase: a choice-free Treasure is played from the hand by the usual path, twice.
                let crown = f.then == Then::CrownTreasure && cards::is_choice_free(c);
                let in_place = f.act == Act::Reveal || (f.act == Act::Play && (f.zone == Zone::Supply || f.zone == Zone::Trash)) || storyteller || crown;
                // Necromancer: the Action is turned face down in the trash for the rest of the turn.
                if f.act == Act::Play && f.zone == Zone::Trash {
                    self.turn.noc.necro.add_or_drop(c, 1);
                }
                if !in_place {
                    let removed = self.zone_take(p, f.zone, c);
                    debug_assert!(removed);
                }
                if storyteller {
                    self.storyteller_play(&f, c, sink);
                } else if crown {
                    self.play_free_treasure(c, 1, sink);
                    self.play_free_treasure_from(c, 1, false, sink);
                } else if f.act == Act::Trash && f.zone == Zone::Supply {
                    // A Supply card trashed (Lurker) is not the trasher's own: no Market Square. The top card of a mixed pile leaves it.
                    self.trash_card(p, c, false, sink);
                    if c >= cards::FIRST_PILE_CARD {
                        self.advance_pile(c);
                    }
                } else if !(f.act == Act::Play && (f.zone == Zone::Supply || f.zone == Zone::Trash)) {
                    self.put(p, c, f.act, f.dest, f.source, sink);
                }
                match f.then {
                    // Gear: each card set aside returns at the start of the next turn.
                    Then::GearAside => self.push_duration_pending(p, f.source, 1, c),
                    // Pilgrimage: a copy of each card picked.
                    Then::GainCopyEachPick => {
                        self.gain(p, c, Dest::Discard, false, f.source, sink);
                    }
                    _ => {}
                }
            }
            (K::Gain, Choice::Pass) => {
                self.stack.pop();
            }
            (K::ReceiveBoon, Choice::Yes) => self.answer_receive_boon(f, true),
            (K::ReceiveBoon, _) => self.answer_receive_boon(f, false),
            (K::PickBoon, Choice::Card(b)) => self.apply_pick_boon(f, b),
            (K::Select, _) => self.finish_select(f, sink),
            (K::StartOrder, Choice::Card(c)) => self.take_start_effect(c, sink),
            (K::Bid, Choice::Bid(n)) => self.apply_bid(f, n),
            (K::Token, Choice::Card(pile)) => {
                self.stack.pop();
                self.place_token(p, f.subject as usize, pile, sink);
            }
            (K::Overpay, Choice::Overpay(_) | Choice::OverpayPotion(_)) => self.apply_overpay(f, choice, sink),
            (K::YesNo, Choice::Yes) => {
                self.stack.pop();
                if f.then == Then::BlockAttack {
                    // Revealed to an Attack: unaffected by it (nothing moves).
                    sink.event(Event::Reaction { player: p, card: f.subject });
                    self.turn.moated |= 1 << p;
                } else {
                    if f.act != Act::Reveal {
                        self.zone_take(p, f.zone, f.subject);
                    }
                    self.put(p, f.subject, f.act, f.dest, f.source, sink);
                }
                if f.act == Act::Trash || (f.act == Act::Discard && self.adv & crate::state::G_DISCARD_HOOK != 0) {
                    self.flush_trash_triggers(f.depth, sink);
                }
                if f.act == Act::Trash && f.zone == Zone::InPlay && f.subject == f.source {
                    // Mining Village trashing itself for its bonus: used, not lost.
                    self.turn.used_up.add_or_drop(f.source, 1);
                }
                if f.act == Act::Play {
                    self.resolve_effects(f.subject, p, f.depth, sink);
                }
                match f.then {
                    Then::CoinsPerPick(n) | Then::YesCoinsElseGainSubject(n) => {
                        self.turn.coins += n as u16;
                        sink.bonus(p, f.source, 0, 0, n as u16, 0);
                    }
                    Then::MarketSquareGold => {
                        self.gain(p, id::GOLD, Dest::Discard, false, f.source, sink);
                        // "You may discard multiple Market Squares when a single card is trashed": offered again while one is left.
                        if self.players[pi].hand.has(f.subject) {
                            self.stack.push(Frame { depth: f.depth, ..f });
                        }
                    }
                    // Two Silvers, one at a time: the first onto the deck, then the second to the discard pile.
                    Then::BeggarGain => {
                        self.gain(p, id::SILVER, Dest::DeckTop, false, f.subject, sink);
                        self.gain(p, id::SILVER, Dest::Discard, false, f.subject, sink);
                    }
                    Then::UrchinMercenary => {
                        self.gain_side(p, id::MERCENARY, Dest::Discard, f.subject, sink);
                    }
                    Then::Called => self.call_effect(&f, sink),
                    Then::HoundAside | Then::PixieTrash | Then::ChangelingSwap => self.noc_yes(&f, sink),
                    Then::TunnelGold | Then::FoolsGoldGain | Then::TraderSwap => self.hint_yes(&f, sink),
                    Then::EngineerGain | Then::SmallCastle => self.empires_yes(&f, sink),
                    // Raze trashed itself: look at as many cards as it cost.
                    Then::RazeSelf => {
                        let n = self.cost(id::RAZE).min(u8::MAX as u8);
                        self.raze_look(p, f.source, n, f.depth);
                    }
                    Then::ReactDrawDiscard { draw, discard } => {
                        // Offered again once this has resolved (see `K::YesNo`).
                        self.stack.push(Frame { depth: f.depth, ..f });
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
            (K::Ask, Choice::Yes) => {
                self.stack.pop();
                self.ask_yes(f, sink);
            }
            (K::Ask, _) => {
                self.stack.pop();
            }
            (K::Library, Choice::Yes) => self.library_set_aside(f, sink),
            (K::Library, _) => self.library_take(f, sink),
            (K::Name, Choice::Card(c)) => {
                f.subject = c;
                f.count = 1;
                self.stack.set_top(f);
            }
            (K::DeckPosition, Choice::Position(k)) => self.apply_deck_position(f, k),
            (kind, ch) => unreachable!("choice {ch:?} not valid for frame {kind:?}"),
        }
    }

    /// The card `card` was just bought and its overpay (`coins`, `potion`; 0 and false = none) is
    /// settled: pay it, push the card's overpay ability (below the frames its gain triggers push,
    /// so they are asked first: the ability is a when-gain effect like the others), gain the card.
    pub(crate) fn finish_buy<S: EventSink>(&mut self, p: u8, card: CardId, coins: u8, potion: bool, depth: u8, sink: &mut S) {
        if coins > 0 || potion {
            self.turn.coins -= coins as u16;
            if potion {
                self.turn.potions -= 1;
            }
            sink.event(Event::Overpay { player: p, card, coins, potion });
            crate::sets::overpay(self, p, card, coins, potion, depth, sink);
        }
        self.gain(p, card, Dest::Discard, true, NO_SOURCE, sink);
        self.on_buy_effects(p, card, sink);
    }

    /// Move card `c` (already removed from its zone, except `Reveal`) according to `act`.
    /// `dest` only matters for `Act::Gain`.
    pub(crate) fn put<S: EventSink>(&mut self, p: u8, c: CardId, act: Act, dest: Dest, source: CardId, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        match act {
            Act::Discard => {
                ps.discard.add(c, 1);
                sink.event(Event::Discard { player: p, card: c });
                // A card that reacts to being discarded (Faithful Hound, Tunnel, Trail, Weaver) is in the game.
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.discard_hook(p, c, 1);
                }
            }
            // Nocturne: back to its pile (Wish, Bat, Vampire, a card exchanged for a Changeling).
            Act::Return => self.return_to_pile(p, c, sink),
            Act::Archive => ps.archive.add(c, 1),
            // Queued for its "when you trash this" effects, which go on the stack when the effect that trashed it is done picking.
            Act::Trash => self.trash_card(p, c, true, sink),
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
            // From the trash (Lurker, Rogue, Graverobber): a gain like any other, with the gain triggers.
            Act::Gain => self.finish_gain(p, c, dest, false, source, false, sink),
            Act::Reveal => sink.event(Event::Reveal { player: p, card: c }),
            // Held secretly until `deliver_left_passes` moves it; no event here, since what's
            // passed isn't revealed until it's received (see `Event::Pass`).
            Act::Pass => ps.passed.add(c, 1),
            Act::ShuffleIn => {
                ps.deck_unknown.add(c, 1);
                sink.event(Event::Reveal { player: p, card: c });
            }
            Act::Tavern => {
                ps.tavern.add(c, 1);
                sink.event(Event::ToTavern { player: p, card: c });
            }
            // A called Reserve card is in play (and discarded at Clean-up like any other).
            Act::Call => {
                ps.in_play.add(c, 1);
                sink.event(Event::Call { player: p, card: c });
            }
            Act::Exchange => self.exchange_traveller(p, c, sink),
            Act::ToHand => {
                ps.hand.add(c, 1);
                sink.event(Event::Draw { player: p, card: c });
            }
        }
    }

    fn finish_select<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        self.stack.pop();
        // Cards trashed by the picks: all are in the trash now, so what they do "when trashed" resolves before whatever this selection
        // leads to (a Cultist trashed to Remodel draws before the gain; trashed by a Knight, before the rest is discarded). The finished
        // selection waits under those effects and finishes again (nothing left to pick) once they are done.
        if !self.trash_queue.is_empty() {
            self.stack.push(Frame { max: f.count, ..f });
            self.flush_trash_triggers(f.depth, sink);
            return;
        }
        let base = self.stack.len as usize;
        let p = f.player;
        match f.then {
            Then::Nothing => {}
            Then::MoveSubjectIfPresent => {
                if self.zone(p, f.zone).get(f.subject) >= f.min && self.zone_take(p, f.zone, f.subject) {
                    self.put(p, f.subject, f.act, f.dest, f.source, sink);
                }
            }
            Then::DrawPerPick => {
                if f.count > 0 {
                    self.stack.push(draw_frame(p, f.source, f.count));
                }
            }
            Then::CoinsPerPick(n) => {
                self.turn.coins += n as u16 * f.count as u16;
                sink.bonus(p, f.source, 0, 0, n as u16 * f.count as u16, 0);
            }
            Then::GainUpTo { plus, filter, dest, exact, dest_by_type } => {
                if f.count > 0 {
                    let then = if dest_by_type { Then::GainedTypeDestAttack } else { Then::Nothing };
                    self.stack.push(Frame { exact, then, potion: cards::potion_cost(f.last), ..gain_frame(p, f.source, self.cost(f.last) + plus, filter, dest) });
                }
            }
            Then::PlayPicked { times } => {
                if f.count > 0 {
                    self.push_plays(&f, times);
                }
            }
            // Crown's Treasure: a choice-free one was played twice as it was picked; any other is played twice like an Action.
            Then::CrownTreasure => {
                if f.count > 0 && !cards::is_choice_free(f.last) {
                    self.push_plays(&f, 2);
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
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.discard_hook_counts(p, &rest, f.depth, sink);
                }
            }
            Then::CoinsIfCount { count, coins } => {
                if f.count == count {
                    self.turn.coins += coins as u16;
                    sink.bonus(p, f.source, 0, 0, coins as u16, 0);
                }
            }
            Then::GainCardIfCount { count, card, dest } => {
                if f.count == count {
                    self.gain(p, card, dest, false, f.source, sink);
                }
            }
            Then::ModePerType => {
                if f.count > 0 {
                    // A Curse is also a Treasure in a game with Charlatan: two types.
                    let types = cards::def(f.last).types.count_ones() + (f.last == id::CURSE && self.is_treasure(f.last)) as u32;
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
                    sink.bonus(p, f.source, actions, 0, 0, 0);
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
                    let vp = (self.cost(f.last) / per) as u16;
                    self.players[p as usize].vp_tokens += vp;
                    sink.bonus(p, f.source, 0, 0, 0, vp);
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
            Then::MoveToIslandMat => self.finish_island(&f),
            Then::CoinsEqualToCostSum => {
                self.turn.coins += f.cost_sum;
                sink.bonus(p, f.source, 0, 0, f.cost_sum, 0);
            }
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
            Then::PlayPickedThenRest => self.finish_golem(&f),
            Then::PrinceHold => {
                if f.count > 0 {
                    self.prince_hold(f);
                }
            }
            Then::GainLess { count } => {
                if f.count > 0 {
                    // Each card is chosen and gained on its own (the second choice sees the first gain).
                    for _ in 0..count {
                        self.stack.push(Frame {
                            less: true, potion: cards::potion_cost(f.last), depth: f.depth,
                            ..gain_frame(p, f.source, self.cost(f.last), Filter::Any, Dest::Discard)
                        });
                    }
                }
            }
            Then::JoustReward => self.finish_joust(&f),
            Then::HermitHand => {
                // A non-Treasure from the discard pile or from the hand: the hand is offered when none was picked from the discard pile.
                if f.count == 0 {
                    self.stack.push(Frame {
                        depth: f.depth,
                        ..select(p, f.source, Zone::Hand, Act::Trash, Filter::NonTreasure, 0, 1, Then::Nothing)
                    });
                }
            }
            Then::ForagerCoins => {
                // +$1 per differently named Treasure in the trash (counted after the trash's own effects).
                let n = self.trash.iter().filter(|&(c, _)| self.is_treasure(c)).count() as u16;
                self.turn.coins += n;
                sink.bonus(p, f.source, 0, 0, n, 0);
            }
            Then::PlayTrash { gain, plus } => {
                if f.count > 0 {
                    // Pushed bottom to top: the gain, the trash (of the card that was played, if it is still in play), the two plays.
                    let card = f.last;
                    let in_play = self.players[p as usize].in_play.get(card);
                    if gain {
                        self.stack.push(Frame {
                            exact: true, potion: cards::potion_cost(card), depth: f.depth,
                            ..gain_frame(p, f.source, self.cost(card).saturating_add(plus), Filter::Action, Dest::Discard)
                        });
                    }
                    self.stack.push(Frame {
                        subject: card, min: in_play, depth: f.depth,
                        ..select(p, f.source, Zone::InPlay, Act::Trash, Filter::Any, 0, 0, Then::MoveSubjectIfPresent)
                    });
                    for nth in (1..=2).rev() {
                        self.stack.push(Frame { subject: card, count: nth, depth: f.depth, ..Frame::new(K::PlayEffects, p, f.source) });
                    }
                }
            }
            Then::KnightTrashed => {
                self.discard_revealed(p, sink);
                // A Knight trashed by the attack makes the attacker trash the Knight played: not one that is in the Supply (played by Band of
                // Misfits) or has already gone.
                if f.count > 0 && cards::is(f.last, cards::KNIGHT) {
                    let a = self.turn.player;
                    if !self.turn.prince_play && self.players[a as usize].in_play.remove(f.source) {
                        self.trash_card(a, f.source, true, sink);
                    }
                }
            }
            Then::MercenaryBonus => {
                if f.count == 2 {
                    self.turn.coins += 2;
                    sink.bonus(p, f.source, 0, 0, 2, 0);
                    let (vs, n) = self.victims(sink);
                    for &v in vs[..n].iter().rev() {
                        self.stack.push(Frame { depth: f.depth, down_to_target: true, ..select(v, f.source, Zone::Hand, Act::Discard, Filter::Any, 0, 3, Then::Nothing) });
                    }
                    self.stack.push(Frame { depth: f.depth, ..draw_frame(p, f.source, 2) });
                }
            }
            Then::PillageRest => self.finish_pillage(&f, sink),
            Then::DeathCartHand => {
                if f.count > 0 {
                    self.turn.coins += 5;
                    sink.bonus(p, f.source, 0, 0, 5, 0);
                } else if !self.turn.prince_play {
                    // Not trashed from the hand: the Death Cart itself may be (if it is still in play).
                    self.stack.push(Frame {
                        depth: f.depth, zone: Zone::InPlay, act: Act::Trash, subject: f.source, then: Then::CoinsPerPick(5),
                        ..Frame::new(K::YesNo, p, f.source)
                    });
                }
            }
            Then::BonusIfPicked { actions, buys } => {
                if f.count > 0 {
                    self.turn.actions += actions;
                    self.turn.buys += buys;
                    sink.bonus(p, f.source, actions, buys, 0, 0);
                }
            }
            Then::CoffersPerPick(n) => {
                if f.count > 0 {
                    self.give_coffers(p, f.source, n as u16 * f.count as u16, sink);
                }
            }
            Then::GainWithCoffers => {
                if f.count > 0 {
                    // Up to the trashed card's cost plus every Coffers token: tokens are only spent for
                    // what the card chosen costs beyond that base (see `Frame::pay_coffers`).
                    let base = self.cost(f.last);
                    let max = (base as u16 + self.players[p as usize].coffers).min(u8::MAX as u16) as u8;
                    self.stack.push(Frame {
                        min: base, pay_coffers: true, potion: cards::potion_cost(f.last), depth: f.depth,
                        ..gain_frame(p, f.source, max, Filter::Any, Dest::Discard)
                    });
                }
            }
            Then::RevealedToHand => {
                let ps = &mut self.players[p as usize];
                let rest = ps.set_aside;
                ps.set_aside.clear();
                ps.hand.add_all(&rest);
                for (c, n) in rest.iter() {
                    for _ in 0..n {
                        sink.event(Event::Draw { player: p, card: c });
                    }
                }
            }
            Then::HoldUntilNextTurn => self.finish_farmhands(&f),
            Then::TreasureMapGold => self.finish_treasure_map(&f, sink),
            Then::ArtificerGain => {
                // Any gain costing exactly $1 per card discarded (nothing discarded: a card costing $0), onto the deck, if the player likes.
                self.stack.push(Frame { exact: true, optional: true, depth: f.depth, ..gain_frame(p, f.source, f.count, Filter::Any, Dest::DeckTop) });
            }
            Then::RazeLook => {
                if f.count > 0 {
                    let n = self.cost(f.last).min(u8::MAX as u8);
                    self.raze_look(p, f.source, n, f.depth);
                }
            }
            Then::StorytellerPay => self.storyteller_pay(&f),
            Then::SaveAside => {
                if f.count > 0 {
                    let ps = &mut self.players[p as usize];
                    if ps.set_aside.remove(f.last) {
                        ps.saved = f.last;
                    }
                }
            }
            Then::SavedToHand => {
                let ps = &mut self.players[p as usize];
                if ps.saved != 0 {
                    let c = std::mem::take(&mut ps.saved);
                    ps.hand.add(c, 1);
                    sink.event(Event::Draw { player: p, card: c });
                }
            }
            Then::GainPerPick { card } => {
                for _ in 0..f.count {
                    self.gain(p, card, Dest::Discard, false, f.source, sink);
                }
            }
            Then::PlayTwiceCopy => self.push_play_twice_copy(&f),
            Then::Inherit => self.finish_inherit(&f),
            Then::InnShuffle
            | Then::DrawUpTo(_)
            | Then::DrawIfHandAtMost { .. }
            | Then::ModeIfPicked
            | Then::WitchsHut
            | Then::DevelopGain
            | Then::GainPerCost { .. } => self.finish_hint_select(&f, sink),
            Then::DrawNPerPick(_)
            | Then::DrawActionsIfPicked { .. }
            | Then::ConclavePlay
            | Then::MirrorGhost
            | Then::ExorcistGain
            | Then::MonasteryCopper
            | Then::TragicHero
            | Then::SecretCave
            | Then::BatExchange
            | Then::VampireBat
            | Then::LostBoon
            | Then::CryptSet
            | Then::HoundsToHand => self.finish_noc_select(&f, sink),
            Then::ChangeGain
            | Then::EncampmentReveal
            | Then::CatapultHit
            | Then::GladiatorReveal
            | Then::LegionaryHit
            | Then::SacrificeBonus
            | Then::AddPileVp
            | Then::AnnexShuffle
            | Then::ConquestVp
            | Then::ArenaVp
            | Then::DonateShuffle => self.empires_finish_select(&f, sink),
            // Not produced by a Select's `then`; only meaningful on YesNo/RevealTop/Gain frames.
            Then::BlockAttack
            | Then::MarketSquareGold
            | Then::BeggarGain
            | Then::UrchinMercenary
            | Then::IronmongerReveal
            | Then::RebuildNamed
            | Then::DeckToDiscard
            | Then::DiscardRevealedAsk
            | Then::ModeWindow { .. }
            | Then::DrawIfGained
            | Then::HornTrash
            | Then::JesterGain
            | Then::JourneymanDraw
            | Then::OneOfEachToHand
            | Then::KeepMatchingDiscardRest
            | Then::GainedTypeStatBonus
            | Then::GainedTypeDestAttack
            | Then::Called
            | Then::GiantReveal
            | Then::WarriorHit
            | Then::MagpieReveal
            | Then::MessengerCopies
            | Then::RazeSelf
            | Then::StorytellerPlay
            | Then::GainCopyEachPick
            | Then::TokenOnGained { .. }
            | Then::GearAside
            | Then::YesCoinsElseGainSubject(_)
            | Then::MoveMatchingToHand(_)
            | Then::MoveMatchingToDiscard(_)
            | Then::ReactDrawDiscard { .. }
            | Then::SeaChartCheck
            | Then::DiscardRevealedNotMatching
            | Then::ChangeDebt
            | Then::EngineerGain
            | Then::PatricianCheck
            | Then::GladiatorMatch
            | Then::DominateVp
            | Then::TriumphVp
            | Then::RitualTrash
            | Then::TakePileVp
            | Then::ArchiveSet
            | Then::ChariotRace
            | Then::SmallCastle
            | Then::MountainPass
            | Then::PixieDraw
            | Then::PixieTrash
            | Then::VillageDraw
            | Then::GroveBoon
            | Then::WispCheck
            | Then::ChangelingSwap
            | Then::HoundAside
            | Then::LocustsHit
            | Then::MasonTrash
            | Then::FamineSort
            | Then::GhostSet
            | Then::TunnelGold
            | Then::FoolsGoldGain
            | Then::TraderSwap
            | Then::BerserkerPlay => {}
        }
        // Cards the continuation trashed (a Procession's trash step) resolve what they do right away.
        self.flush_trash_triggers(f.depth, sink);
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
                ModeOpt::GainMany(c, dest, n) => {
                    for _ in 0..n {
                        self.stack.push(gain_frame(p, f.source, u8::MAX, Filter::Card(c), dest));
                    }
                }
                ModeOpt::TopdeckFromHand(n) => self.stack.push(select(p, f.source, Zone::Hand, Act::Topdeck, Filter::Any, n, n, Then::Nothing)),
                ModeOpt::TrashGain { filter, plus, dest } => self.stack.push(select(
                    p, f.source, Zone::Hand, Act::Trash, filter, 1, 1,
                    Then::GainUpTo { plus, filter: Filter::Any, dest, exact: false, dest_by_type: false },
                )),
                ModeOpt::GainFromTrashTo(filt, dest) => {
                    self.stack.push(Frame { dest, ..select(p, f.source, Zone::Trash, Act::Gain, filt, 1, 1, Then::Nothing) })
                }
                ModeOpt::DiscardRevealedDraw(n) => self.stack.push(draw_frame(p, f.source, n)),
                ModeOpt::TavernCopper => self.stack.push(select(p, f.source, Zone::Hand, Act::Tavern, Filter::Card(id::COPPER), 1, 1, Then::Nothing)),
                ModeOpt::MoveToken(t) => self.push_token_frame(p, f.source, t as usize, true),
                ModeOpt::DiscardFor { filter, n, gain } => self.stack.push(select(
                    p, f.source, Zone::Hand, Act::Discard, filter, n, n,
                    Then::GainCardIfCount { count: n, card: gain, dest: Dest::Discard },
                )),
                ModeOpt::CardsGather(n, _) => self.stack.push(draw_frame(p, f.source, n)),
                ModeOpt::GainTakeVp(c) => {
                    self.stack.push(Frame { then: Then::TakePileVp, ..gain_frame(p, f.source, u8::MAX, Filter::Card(c), Dest::Discard) })
                }
                ModeOpt::GainUpToCost(n) => self.stack.push(gain_frame(p, f.source, n, Filter::Any, Dest::Discard)),
                ModeOpt::CardsActions(n, _) => self.stack.push(draw_frame(p, f.source, n)),
                ModeOpt::CoinsPerTavernCopper
                | ModeOpt::Nothing
                | ModeOpt::BuysCoins(..)
                | ModeOpt::NextGainDouble
                | ModeOpt::TrashHand
                | ModeOpt::TakeRevealed
                | ModeOpt::Actions(_)
                | ModeOpt::Buys(_)
                | ModeOpt::Coins(_)
                | ModeOpt::Gain(..)
                | ModeOpt::GainSubject { .. }
                | ModeOpt::DiscardHandDraw { .. }
                | ModeOpt::TrashSelfRevealVpPerTreasureType
                | ModeOpt::NativeVillageTake => {}
            }
        }
        for fr in &mut self.stack.frames[base..self.stack.len as usize] {
            fr.depth = f.depth;
        }
        // Vanilla bonuses from the chosen atoms are logged as one line.
        let (mut b_actions, mut b_buys, mut b_coins) = (0u8, 0u8, 0u16);
        for i in 0..table.len() as u8 {
            if f.min & (1 << i) == 0 {
                continue;
            }
            match table[i as usize] {
                ModeOpt::Actions(n) => {
                    self.turn.actions += n;
                    b_actions += n;
                }
                ModeOpt::CardsActions(_, a) => {
                    self.turn.actions += a;
                    b_actions += a;
                }
                ModeOpt::Buys(n) => {
                    self.turn.buys += n;
                    b_buys += n;
                }
                ModeOpt::Coins(n) => {
                    self.turn.coins += n as u16;
                    b_coins += n as u16;
                }
                ModeOpt::Gain(c, dest) => {
                    self.gain(p, c, dest, false, f.source, sink);
                }
                // Jester: a copy of the discarded card (`subject`) for the victim or for the attacker.
                ModeOpt::GainSubject { to_chooser } => {
                    let who = if to_chooser { f.chooser } else { p };
                    self.gain(who, f.subject, Dest::Discard, false, f.source, sink);
                }
                ModeOpt::TrashHand => {
                    let hand = self.players[p as usize].hand;
                    self.players[p as usize].hand.clear();
                    for (c, n) in hand.iter() {
                        for _ in 0..n {
                            self.trash_card(p, c, true, sink);
                        }
                    }
                }
                ModeOpt::TakeRevealed => {
                    let ps = &mut self.players[p as usize];
                    let revealed = ps.set_aside;
                    ps.set_aside.clear();
                    ps.hand.add_all(&revealed);
                    for (c, n) in revealed.iter() {
                        for _ in 0..n {
                            sink.event(Event::Draw { player: p, card: c });
                        }
                    }
                }
                ModeOpt::DiscardRevealedDraw(_) => self.discard_revealed(p, sink),
                ModeOpt::CoinsPerTavernCopper => {
                    let n = self.players[p as usize].tavern.get(id::COPPER) as u16;
                    self.turn.coins += n;
                    b_coins += n;
                }
                ModeOpt::DiscardHandDraw { draw, attack_min_hand } => self.mode_discard_hand_draw(&f, draw, attack_min_hand, sink),
                ModeOpt::TrashSelfRevealVpPerTreasureType => self.mode_trash_self_for_vp(&f, sink),
                ModeOpt::NativeVillageTake => self.mode_native_village_take(&f, sink),
                ModeOpt::CardsGather(_, v) => {
                    self.emp.piles.add_vp(cards::pile_of(f.source), v);
                    sink.event(Event::PileVp { pile: f.source, n: v });
                }
                ModeOpt::BuysCoins(b, k) => {
                    self.turn.buys += b;
                    self.turn.coins += k as u16;
                    b_buys += b;
                    b_coins += k as u16;
                }
                ModeOpt::NextGainDouble => self.turn.emp.charms = self.turn.emp.charms.saturating_add(1),
                ModeOpt::GainTakeVp(_) | ModeOpt::GainUpToCost(_) => {}
                ModeOpt::Cards(_)
                | ModeOpt::GainMany(..)
                | ModeOpt::TopdeckFromHand(_)
                | ModeOpt::TrashGain { .. }
                | ModeOpt::GainFromTrashTo(..)
                | ModeOpt::TrashFromHand(_)
                | ModeOpt::DiscardFromHand(_)
                | ModeOpt::TrashFromSupply(_)
                | ModeOpt::GainFromTrash(_)
                | ModeOpt::TavernCopper
                | ModeOpt::MoveToken(_)
                | ModeOpt::DiscardFor { .. }
                | ModeOpt::Nothing
                | ModeOpt::NativeVillageAdd => {}
            }
        }
        sink.bonus(p, f.source, b_actions, b_buys, b_coins, 0);
        // Cards a chosen option trashed (Count's "trash your hand").
        self.flush_trash_triggers(f.depth, sink);
    }

    /// The plays of the card picked (`f.last`) by a Throne Room-like selection `f`: `times` `PlayEffects`, the 1st on top. A Duration (or Prince)
    /// played more than once keeps the multiplier in play with it if at least one play schedules a next-turn effect: `MultiplierFinalize`
    /// sits below all of them (and everything they push) and checks the tally once they have all unwound. (Only for a card that plays another
    /// more than once: a card that plays one Action once, Crystal Ball or Shop, does not stay in play with it. Not when the multiplier is not
    /// in play itself, as when Captain or a Prince plays it: there is no card to keep.)
    pub(crate) fn push_plays(&mut self, f: &Frame, times: u8) {
        let p = f.player;
        if times >= 2
            && (cards::is(f.last, cards::DURATION) || f.last == id::PRINCE)
            && self.players[p as usize].in_play.has(f.source)
        {
            self.turn.multiplier_card = f.source;
            self.turn.multiplier_expected = times;
            self.turn.multiplier_successes = 0;
            self.stack.push(Frame { depth: f.depth, subject: f.last, ..Frame::new(K::MultiplierFinalize, p, f.source) });
        }
        // Pushed last-first so the 1st resolution is on top; `count` = which play.
        for nth in (1..=times).rev() {
            self.stack.push(Frame {
                subject: f.last, count: nth, zone: if f.zone == Zone::Supply || f.zone == Zone::Trash { f.zone } else { Zone::Hand },
                ..Frame::new(K::PlayEffects, p, f.source)
            });
        }
    }

    /// Discard every card in `p`'s hand (Minion's "discard your hand" for the acting player and
    /// each affected victim).
    pub(crate) fn discard_whole_hand<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let ps = &mut self.players[p as usize];
        let hand = ps.hand;
        ps.hand.clear();
        ps.discard.add_all(&hand);
        for (c, n) in hand.iter() {
            for _ in 0..n {
                sink.event(Event::Discard { player: p, card: c });
            }
        }
        if self.adv & crate::state::G_DISCARD_HOOK != 0 {
            self.discard_hook_counts(p, &hand, 0, sink);
        }
    }

    /// A declined (or skipped, nothing-to-act-on) `YesNo`: the only generic consequence is
    /// Baron's "if you don't, gain an Estate" (any `Then::YesCoinsElseGainSubject`).
    fn yesno_decline<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        if let Then::YesCoinsElseGainSubject(_) = f.then {
            self.gain(f.player, f.subject, Dest::Discard, false, f.source, sink);
        }
        if matches!(f.then, Then::GladiatorMatch | Then::SmallCastle) {
            self.empires_decline(&f, sink);
        }
        self.decline_adv(&f);
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
        // Only for the turn's own player: a Pirate another player plays from their hand during
        // this turn stays out until their own next turn's cleanup, not this one's.
        if p == self.turn.player {
            self.turn.duration_held.add(card, 1);
        }
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
        ps.pending_durations[i] = crate::state::PendingDuration { card, times, arg, used: 0 };
        ps.pending_durations_len += 1;
    }

    /// Collect the start-of-turn effects of `self.turn.player`'s turn, which has just begun, into
    /// `TurnState::start_pending` and push the `FrameKind::StartOrder` frame that resolves them.
    /// The effects are: each pending Duration entry (moved out of `pending_durations`, so what
    /// the turn schedules for the next one doesn't mix with them), one Prince entry per card its
    /// Princes hold (`arg` = the card it plays; the card stays in `prince_cards` for good) and
    /// Clerk ("you may play this from your hand", one merged entry for the copies in hand, which
    /// only counts those still in hand when it resolves). Called once, from `engine::run_phase`'s
    /// `Phase::Action` arm, before any `PlayAction` decision. Cheaply gated: games with none of
    /// these do nothing here beyond three field reads.
    pub(crate) fn push_turn_start_frames<S: EventSink>(&mut self, sink: &mut S) {
        let p = self.turn.player;
        let pi = p as usize;
        // Corsair and Blockade end now ("until your next turn"), and so do Enchantress's marks on the other players.
        self.players[pi].shielded = 0;
        // Nocturne: Guardian's protection lasts until this turn starts.
        self.players[pi].noc &= !crate::state::P_GUARDED;
        if self.emp.flags & crate::state::E_ENCHANT != 0 {
            self.clear_enchantments(p);
        }
        let len = self.players[pi].pending_durations_len as usize;
        if len > 0 {
            let ps = &mut self.players[pi];
            for i in 0..len {
                self.turn.start_pending.push(ps.pending_durations[i]);
                ps.pending_durations[i] = Default::default();
            }
            ps.pending_durations_len = 0;
        }
        crate::sets::turn_start_entries(self);
        if !self.turn.start_pending.is_empty() {
            self.stack.push(Frame::new(K::StartOrder, p, 0));
        }
        // Donate comes before anything else at the start of the turn.
        if self.players[pi].emp & crate::state::P_DONATE != 0 {
            self.start_donate(p, sink);
        }
    }

    /// The card a pending start-of-turn effect is offered under in `DecisionKind::StartOrder`: its
    /// own card, except that a Prince is named by the card it plays (two Princes holding different
    /// cards are different effects). Entries with the same name are interchangeable here (one
    /// choice; the first of them resolves).
    fn start_effect_name(e: &crate::state::PendingDuration) -> CardId {
        if crate::sets::start_entry_named_by_arg(e.card) { e.arg } else { e.card }
    }

    /// An effect no other effect's order can matter to: nothing to decide, no cards drawn or put
    /// into or taken out of the hand (the +Actions/+Buys/+$ bonuses, Outpost, a Throne Room kept
    /// in play). Resolved first, without a question.
    fn start_effect_inert(e: &crate::state::PendingDuration) -> bool {
        !cards::start_order_sensitive(e.card) && cards::duration_bonus(e.card).0 == 0
    }

    /// One step of `FrameKind::StartOrder`: resolve the next pending start-of-turn effect, or ask
    /// which. Inert effects (`start_effect_inert`) resolve first without asking. The player is
    /// asked (`DecisionKind::StartOrder`) only when two or more different effects remain and one
    /// is order-sensitive (`cards::start_order_sensitive`); otherwise the first resolves.
    #[inline(never)]
    fn run_start_order<S: EventSink>(&mut self, sink: &mut S) -> Run {
        let list = self.turn.start_pending.as_slice();
        if list.is_empty() {
            self.stack.pop();
            return Run::Continue;
        }
        if let Some(i) = list.iter().position(Self::start_effect_inert) {
            self.take_start_effect_at(i, sink);
            return Run::Continue;
        }
        let mut sensitive = false;
        let mut distinct = 0;
        for (i, e) in list.iter().enumerate() {
            sensitive |= cards::start_order_sensitive(e.card);
            let name = Self::start_effect_name(e);
            if !list[..i].iter().any(|x| Self::start_effect_name(x) == name) {
                distinct += 1;
            }
        }
        if sensitive && distinct >= 2 {
            return Run::Decide(DecisionKind::StartOrder, 0);
        }
        self.take_start_effect_at(0, sink);
        Run::Continue
    }

    /// Legal choices of `DecisionKind::StartOrder`: each distinct pending effect, by name.
    #[inline(never)]
    fn start_order_choices(&self, out: &mut ChoiceBuf) {
        let list = self.turn.start_pending.as_slice();
        for (i, e) in list.iter().enumerate() {
            let name = Self::start_effect_name(e);
            if !list[..i].iter().any(|x| Self::start_effect_name(x) == name) {
                out.push(Choice::Card(name));
            }
        }
    }

    /// Resolve the pending effect named `name` (the first one with that name).
    #[inline(never)]
    fn take_start_effect<S: EventSink>(&mut self, name: CardId, sink: &mut S) {
        let i = self.turn.start_pending.as_slice().iter().position(|e| Self::start_effect_name(e) == name).expect("pending start-of-turn effect");
        self.take_start_effect_at(i, sink);
    }

    /// Take pending effect `i` off the list and resolve it. Clerk and Prince entries stand for
    /// several identical effects (`times`) and resolve one at a time, so the rest stay pending.
    fn take_start_effect_at<S: EventSink>(&mut self, i: usize, sink: &mut S) {
        let p = self.turn.player;
        let mut e = self.turn.start_pending.as_slice()[i];
        if crate::sets::start_entry_repeats(e.card) && e.times > 1 {
            self.turn.start_pending.as_mut_slice()[i].times -= 1;
            e.times = 1;
        } else {
            self.turn.start_pending.remove(i);
        }
        if !crate::sets::take_start_effect(self, p, &e, sink) {
            self.resolve_duration_start(p, e, sink);
        }
    }

    /// Resolve one pending Duration effect at the start of its owner's turn: the generic
    /// (cards, actions, buys, coins) bonus (`cards::duration_bonus`), `times` times, then any
    /// card-specific extra (pushed so it resolves before the generic draw, matching
    /// `resolve_effects_inner`'s "special frames first, +cards draw last" convention).
    pub(crate) fn resolve_duration_start<S: EventSink>(&mut self, p: u8, e: crate::state::PendingDuration, sink: &mut S) {
        let times = e.times;
        // The firing is logged at the turn's top level, with its bonuses (always emitted, even
        // with none: Haven, Caravan); what it then does (draws, gains, ...) is indented below.
        let (cards_n, actions_n, buys_n, coins_n) = cards::duration_bonus(e.card);
        sink.event(Event::Bonus {
            player: p,
            source: e.card,
            actions: actions_n * times,
            buys: buys_n * times,
            coins: coins_n as u16 * times as u16,
            vp: 0,
        });
        sink.depth(1);
        crate::sets::duration_start(self, p, &e, sink);
        self.turn.actions += actions_n * times;
        self.turn.buys += buys_n * times;
        self.turn.coins += coins_n as u16 * times as u16;
        if cards_n > 0 && times > 0 {
            self.stack.push(Frame { depth: 1, ..draw_frame(p, e.card, cards_n * times) });
        }
    }

    /// A `RevealTop` frame finishing (either it revealed everything asked for, or the deck ran
    /// out): pop it and, for Patrol, move every revealed card matching the filter into hand.
    fn finish_reveal_top<S: EventSink>(&mut self, f: Frame, sink: &mut S) {
        self.stack.pop();
        match f.then {
            Then::SeaChartCheck => self.finish_sea_chart(&f, sink),
            Then::MagpieReveal => self.finish_magpie(&f, sink),
            Then::OneOfEachToHand => self.finish_carnival(&f, sink),
            Then::IronmongerReveal => self.finish_ironmonger(&f, sink),
            Then::KeepMatchingDiscardRest => self.finish_keep_matching(&f, sink),
            Then::PatricianCheck | Then::ArchiveSet => self.empires_reveal_done(&f, sink),
            Then::WispCheck | Then::FamineSort | Then::GhostSet => self.finish_noc_reveal(&f, sink),
            Then::MoveMatchingToHand(filter) => {
                let p = f.player as usize;
                let revealed = self.players[p].set_aside;
                for (c, n) in revealed.iter() {
                    if self.filter_ok(f.player, filter, c) {
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
                    if !self.filter_ok(f.player, f.filter, c) {
                        self.players[p].set_aside.set(c, 0);
                        self.players[p].discard.add(c, n);
                        for _ in 0..n {
                            sink.event(Event::Discard { player: f.player, card: c });
                        }
                        if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                            self.discard_hook(f.player, c, n);
                        }
                    }
                }
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.flush_trash_triggers(f.depth, sink);
                }
            }
            Then::MoveMatchingToDiscard(filter) => {
                // Rabble: revealed Actions and Treasures are discarded automatically (no
                // decision); whatever's left (Victory/Curse cards) is handled by a `Select`
                // frame pushed under this one (Then::Nothing there, ordered Topdeck).
                let p = f.player as usize;
                let revealed = self.players[p].set_aside;
                for (c, n) in revealed.iter() {
                    if self.filter_ok(f.player, filter, c) {
                        self.players[p].set_aside.set(c, 0);
                        self.players[p].discard.add(c, n);
                        for _ in 0..n {
                            sink.event(Event::Discard { player: f.player, card: c });
                        }
                        if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                            self.discard_hook(f.player, c, n);
                        }
                    }
                }
                if self.adv & crate::state::G_DISCARD_HOOK != 0 {
                    self.flush_trash_triggers(f.depth, sink);
                }
            }
            _ => {}
        }
    }
}
