//! Seaside, 2nd edition: Durations, the island and native village mats, and the gain-triggered reactions of Monkey, Blockade, Pirate and Sailor.

use super::prelude::*;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const HAVEN: CardId = 59;
    pub const LIGHTHOUSE: CardId = 60;
    pub const NATIVE_VILLAGE: CardId = 61;
    pub const ASTROLABE: CardId = 62;
    pub const FISHING_VILLAGE: CardId = 63;
    pub const LOOKOUT: CardId = 64;
    pub const MONKEY: CardId = 65;
    pub const SEA_CHART: CardId = 66;
    pub const SMUGGLERS: CardId = 67;
    pub const WAREHOUSE: CardId = 68;
    pub const BLOCKADE: CardId = 69;
    pub const CARAVAN: CardId = 70;
    pub const CUTPURSE: CardId = 71;
    pub const ISLAND: CardId = 72;
    pub const SALVAGER: CardId = 73;
    pub const SAILOR: CardId = 74;
    pub const TIDE_POOLS: CardId = 75;
    pub const TREASURE_MAP: CardId = 76;
    pub const BAZAAR: CardId = 77;
    pub const CORSAIR: CardId = 78;
    pub const MERCHANT_SHIP: CardId = 79;
    pub const OUTPOST: CardId = 80;
    pub const PIRATE: CardId = 81;
    pub const SEA_WITCH: CardId = 82;
    pub const TACTICIAN: CardId = 83;
    pub const TREASURY: CardId = 84;
    pub const WHARF: CardId = 85;
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::HAVEN, seaside(order_sensitive(c("Haven", 2, ACTION | DURATION, 0, 0, 1, 1, 0)))), // what to set aside (may be the +Actions card in hand)
    (ids::LIGHTHOUSE, seaside(choice_free(c("Lighthouse", 2, ACTION | DURATION, 1, 0, 0, 1, 0)))),
    (ids::NATIVE_VILLAGE, seaside(order_sensitive(c("Native Village", 2, ACTION, 0, 0, 0, 2, 0)))), // mat: add the deck top or take the mat
    (ids::ASTROLABE, seaside(choice_free(c("Astrolabe", 3, TREASURE | DURATION, 1, 0, 0, 0, 1)))),
    (ids::FISHING_VILLAGE, seaside(choice_free(c("Fishing Village", 3, ACTION | DURATION, 1, 0, 0, 2, 0)))),
    (ids::LOOKOUT, seaside(order_sensitive(c("Lookout", 3, ACTION, 0, 0, 0, 1, 0)))), // trash / discard / keep the top; a later draw takes the kept one
    // Draws when the player to its owner's right gains a card: played before a Curse-giving Attack (Witch,
    // Familiar, Soothsayer...) it draws for each Curse, so the order against those matters.
    (ids::MONKEY, seaside(has_choice(c("Monkey", 3, ACTION | DURATION, 0, 0, 0, 0, 0)))),
    (ids::SEA_CHART, seaside(order_sensitive(c("Sea Chart", 3, ACTION, 0, 0, 1, 1, 0)))), // reveals the top; a miss stays known for a later draw
    (ids::SMUGGLERS, seaside(has_choice(c("Smugglers", 3, ACTION, 0, 0, 0, 0, 0)))), // what to gain
    (ids::WAREHOUSE, seaside(order_sensitive(c("Warehouse", 3, ACTION, 0, 0, 3, 1, 0)))), // what to discard (may be the +Actions card in hand)
    (ids::BLOCKADE, seaside(has_choice(c("Blockade", 4, ACTION | DURATION | ATTACK, 0, 0, 0, 0, 0)))), // what to gain
    (ids::CARAVAN, seaside(choice_free(c("Caravan", 4, ACTION | DURATION, 0, 0, 1, 1, 0)))),
    (ids::CUTPURSE, seaside(choice_free(c("Cutpurse", 4, ACTION | ATTACK, 2, 0, 0, 0, 0)))), // only victims act
    (ids::ISLAND, seaside(order_sensitive(c("Island", 4, ACTION | VICTORY, 0, 2, 0, 0, 0)))), // what to put on the mat (may be the +Actions card in hand)
    (ids::SALVAGER, seaside(order_sensitive(c("Salvager", 4, ACTION, 0, 0, 0, 0, 1)))), // what to trash (may be the +Actions card in hand)
    (ids::SAILOR, seaside(choice_free(c("Sailor", 4, ACTION | DURATION, 0, 0, 0, 1, 0)))), // choices only on gains / next turn
    (ids::TIDE_POOLS, seaside(choice_free(c("Tide Pools", 4, ACTION | DURATION, 0, 0, 3, 1, 0)))), // the discard is next turn
    (ids::TREASURE_MAP, seaside(order_sensitive(c("Treasure Map", 4, ACTION, 0, 0, 0, 0, 0)))), // gains Golds onto the deck; a later draw takes them
    (ids::BAZAAR, seaside(choice_free(c("Bazaar", 5, ACTION, 1, 0, 1, 2, 0)))),
    (ids::CORSAIR, seaside(choice_free(c("Corsair", 5, ACTION | DURATION | ATTACK, 2, 0, 0, 0, 0)))),
    (ids::MERCHANT_SHIP, seaside(choice_free(c("Merchant Ship", 5, ACTION | DURATION, 2, 0, 0, 0, 0)))),
    (ids::OUTPOST, seaside(choice_free(c("Outpost", 5, ACTION | DURATION, 0, 0, 0, 0, 0)))),
    (ids::PIRATE, seaside(choice_free(c("Pirate", 5, ACTION | DURATION | REACTION, 0, 0, 0, 0, 0)))),
    (ids::SEA_WITCH, seaside(choice_free(c("Sea Witch", 5, ACTION | DURATION | ATTACK, 0, 0, 2, 0, 0)))), // the discard is next turn
    (ids::TACTICIAN, seaside(has_choice(c("Tactician", 5, ACTION | DURATION, 0, 0, 0, 0, 0)))), // discards the hand: order matters
    (ids::TREASURY, seaside(choice_free(c("Treasury", 5, ACTION, 1, 0, 1, 1, 0)))),
    (ids::WHARF, seaside(choice_free(c("Wharf", 5, ACTION | DURATION, 0, 0, 2, 0, 1)))),
];

/// The Seaside cards (see the module docs of `sets`).
pub struct Seaside;

impl SetRules for Seaside {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    const GAIN_LO: u128 = (1u128 << id::MONKEY) | (1u128 << id::BLOCKADE) | (1u128 << id::PIRATE) | (1u128 << id::SAILOR);
    const IMMUNE_IN_PLAY: &'static [CardId] = &[id::LIGHTHOUSE];

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            id::NATIVE_VILLAGE => &[ModeOpt::NativeVillageAdd, ModeOpt::NativeVillageTake],
            _ => &[],
        }
    }

    fn duration_bonus(card: CardId) -> (u8, u8, u8, u8) {
        match card {
            id::LIGHTHOUSE => (0, 0, 0, 1),
            id::ASTROLABE => (0, 0, 1, 1),
            id::FISHING_VILLAGE => (0, 1, 0, 1),
            id::MONKEY => (1, 0, 0, 0),
            id::CARAVAN => (1, 0, 0, 0),
            id::SAILOR => (0, 0, 0, 2),
            id::CORSAIR => (1, 0, 0, 0),
            id::MERCHANT_SHIP => (0, 0, 0, 2),
            id::SEA_WITCH => (2, 0, 0, 0),
            id::TACTICIAN => (5, 1, 1, 0),
            id::WHARF => (2, 0, 1, 0),
            _ => (0, 0, 0, 0),
        }
    }

    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, id::HAVEN | id::BLOCKADE | id::PIRATE | id::TIDE_POOLS | id::SEA_WITCH | id::SAILOR)
    }

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        match card {
            id::NATIVE_VILLAGE => g.stack.push(mode_frame(p, card, 1)),
            id::LOOKOUT => {
                // Look at the top 3: trash exactly one (mandatory if any), discard exactly one
                // of what's left (mandatory if any), put whatever remains (0 or 1 card, no real
                // reordering choice) back on top — reuses Sentry's RevealTop + Select stack.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 1, 1, Then::Nothing));
                g.stack.push(select(p, card, Zone::Revealed, Trash, Filter::Any, 1, 1, Then::Nothing));
                g.stack.push(Frame { max: 3, ..Frame::new(K::RevealTop, p, card) });
            }
            id::SEA_CHART => g.stack.push(Frame { max: 1, then: Then::SeaChartCheck, ..Frame::new(K::RevealTop, p, card) }),
            id::SMUGGLERS => {
                // "Gain a copy of a card costing up to $6 that the player to your right gained
                // on their last turn": candidates are further restricted to that player's
                // `last_turn_gains` by `gain_from_record` (checked in `frame_choices`/Gain).
                g.stack.push(Frame { gain_from_record: true, ..gain_frame(p, card, 6, Filter::Any, Dest::Discard) });
            }
            id::WAREHOUSE => g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 3, 3, Then::Nothing)),
            id::CUTPURSE => {
                // +$2 is vanilla (below). Each other player (Moat allowing) discards a Copper,
                // or reveals a hand with none — mirrors Bureaucrat's Victory-card check.
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    let hand = g.players[v as usize].hand;
                    if hand.has(id::COPPER) {
                        g.stack.push(select(v, card, Zone::Hand, Discard, Filter::Card(id::COPPER), 1, 1, Then::Nothing));
                    } else {
                        for (c, k) in hand.iter() {
                            for _ in 0..k {
                                g.reveal_card(v, c, sink);
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
                if g.leave_play_self_now(p, id::ISLAND) {
                    let ps = &mut g.players[p as usize];
                    ps.island_mat.add(id::ISLAND, 1);
                    sink.event(Event::SetAside { player: p, card: id::ISLAND });
                }
                g.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Any, 1, 1, Then::MoveToIslandMat));
            }
            id::SALVAGER => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::CoinsEqualToCostSum)),
            id::TREASURE_MAP => {
                // 2nd edition: "this" (the physical copy in play) is trashed only if it's still
                // in play — a 2nd Throne Room/King's Court resolution finds it already gone.
                let self_trashed = g.leave_play_self_now(p, id::TREASURE_MAP);
                if self_trashed {
                    g.trash_card(p, id::TREASURE_MAP, true, sink);
                    g.flush_trash_triggers(0, sink);
                }
                // "Trash this and a Treasure Map from your hand": the second one is not optional.
                g.stack.push(Frame {
                    self_trashed,
                    ..select(p, card, Zone::Hand, Trash, Filter::Card(id::TREASURE_MAP), 1, 1, Then::TreasureMapGold)
                });
            }
            id::HAVEN => g.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Any, 1, 1, Then::ScheduleDuration { times: 1 })),
            id::BLOCKADE => {
                g.shield_lingering_attack(p, sink);
                g.stack.push(Frame { then: Then::ScheduleDuration { times: 1 }, ..gain_frame(p, card, 4, Filter::Any, Dest::Discard) });
            }
            id::CORSAIR => g.shield_lingering_attack(p, sink),
            id::SEA_WITCH => {
                // "+2 Cards. Each other player gains a Curse": after the draw, like Witch.
                let (vs, n) = g.victims(sink);
                g.push_gain_to_each(&vs[..n], id::CURSE, card);
            }
            id::TACTICIAN => {
                // "If you have a card in hand" gates the *entire* effect, including staying in
                // play as a Duration: an empty hand means Tactician does nothing further and
                // discards normally at cleanup (excluded from the generic tail below; this arm
                // is the only place that schedules it).
                if !g.players[p as usize].hand.is_empty() {
                    g.discard_whole_hand(p, sink);
                    g.push_duration_pending(p, card, 1, 0);
                }
            }
            _ => {}
        }
    }

    fn schedules_own_duration(card: CardId) -> bool {
        matches!(card, id::HAVEN | id::BLOCKADE | id::TACTICIAN)
    }

    fn duration_start<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, sink: &mut S) {
        let times = e.times;
        match e.card {
            id::HAVEN | id::BLOCKADE => {
                // Put the set-aside/gained card (arg) back into hand. Only scheduled when a card was
                // set aside, so `arg` is always that card (Copper is id 0: no "0 = none" check).
                let ps = &mut g.players[p as usize];
                if ps.set_aside.remove(e.arg) {
                    ps.hand.add(e.arg, 1);
                    sink.event(Event::Draw { player: p, card: e.arg });
                }
            }
            // Once per play: a Throne Room'd (or King's Court'd) card does it `times` times.
            id::TIDE_POOLS | id::SEA_WITCH => {
                for _ in 0..times {
                    g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Hand, Act::Discard, Filter::Any, 2, 2, Then::Nothing) });
                }
            }
            id::SAILOR => {
                for _ in 0..times {
                    g.stack.push(Frame { depth: 1, ..select(p, e.card, Zone::Hand, Act::Trash, Filter::Any, 0, 1, Then::Nothing) });
                }
            }
            id::PIRATE => {
                for _ in 0..times {
                    g.stack.push(Frame { depth: 1, ..gain_frame(p, e.card, 6, g.treasure_filter(), Dest::Hand) });
                }
            }
            _ => {} // Outpost: no start-of-turn effect (handled at cleanup/turn transition).
        }
    }

    /// Cross-player "when [someone] gains a card" reactions of Seaside Durations (a watcher reacting to another player's
    /// gain, or to its own owner's with a per-Sailor use count): Monkey reacts to the player to its owner's right; Blockade
    /// reacts to any other player gaining its held card; Pirate reacts to any player's Treasure gain; Sailor to its own
    /// owner's gain of a Duration, "once this turn" tracked per Sailor instance via its `PendingDuration::used` count.
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, _on_buy: bool, sink: &mut S) {
        let n = g.num_players;
        // Monkey: "until your next turn, when the player to your right gains a card, +1 Card."
        // The owner is the player to whose right `p` sits, i.e. the next seat after `p` in turn
        // order (turns pass to the left, so "your right" is the player who acted just before you).
        // The effect lasts "until your next turn" (so not through the owner's own next turn, where
        // the Monkey is still in play) and each play of a Monkey is its own effect (a Throne
        // Roomed Monkey draws twice): both are what its pending next-turn entry records.
        if g.in_supply(id::MONKEY) {
            let owner = (p + 1) % n;
            let ps = &g.players[owner as usize];
            let mut draws = 0u8;
            for i in 0..ps.pending_durations_len as usize {
                if ps.pending_durations[i].card == id::MONKEY {
                    draws = draws.saturating_add(ps.pending_durations[i].times);
                }
            }
            if draws > 0 {
                g.stack.push(Frame { max: draws, ..Frame::new(FrameKind::Draw, owner, id::MONKEY) });
            }
        }
        // Blockade: "while its gained card is set aside, when another player gains a copy on
        // their turn, they gain a Curse." Checked against every player's live Blockade holds.
        // Edge case: if the blockaded card is itself a Curse, the Curse this grants is *also* "a
        // copy of the blockaded card", which by the letter of the rule keeps cursing the same
        // player again — bounded only by the Curse pile (up to 50, at 6 players), each further
        // gain of which can itself push more reaction frames (Watchtower...) before any of them
        // are resolved. Un-guarded, that can recurse deep enough to overflow the fixed-size
        // effect stack from an entirely ordinary action (simply buying a Curse). `blockading_curse`
        // guards against the *recursive* re-trigger specifically (a Blockade-granted Curse never
        // triggers another Blockade-granted Curse), capping this at one extra Curse per gain of
        // the blockaded card: a deliberate, narrow simplification of this degenerate combo, not
        // exercised by any ordinary game.
        if g.in_supply(id::BLOCKADE) && g.turn.player == p && !g.turn.blockading_curse {
            for owner in 0..n {
                if owner == p {
                    continue;
                }
                let ps = &g.players[owner as usize];
                // Immune when the Blockade was played (Moat, Lighthouse): no Curse, ever.
                if ps.shielded & (1 << p) != 0 {
                    continue;
                }
                for i in 0..ps.pending_durations_len as usize {
                    let e = ps.pending_durations[i];
                    if e.card == id::BLOCKADE && e.arg == c {
                        g.turn.blockading_curse = true;
                        g.gain(p, id::CURSE, Dest::Discard, false, id::BLOCKADE, sink);
                        g.turn.blockading_curse = false;
                        break;
                    }
                }
            }
        }
        // Pirate: "When any player gains a Treasure, you may play this from your hand." Each copy
        // in the hand is its own reaction (one offer per copy; a copy played away is skipped).
        // Players react in turn order starting with the turn's player: pushed last-first.
        if g.in_supply(id::PIRATE) && g.is_treasure(c) {
            for i in (0..n).rev() {
                let owner = (g.turn.player + i) % n;
                for _ in 0..g.players[owner as usize].hand.get(id::PIRATE) {
                    g.stack.push(Frame {
                        zone: Zone::Hand, act: Act::Play, subject: id::PIRATE,
                        ..Frame::new(FrameKind::YesNo, owner, id::PIRATE)
                    });
                }
            }
        }
        // Sailor: "once this turn, when you gain a Duration card, you may play it." Each play of a
        // Sailor (a Throne Room'd one is played twice) is one use, spent when it is offered; one
        // offer per gained card, since two Sailors cannot play the same card twice. Only the
        // gainer's own Sailors react. The card is played from where it landed, the deck top
        // included.
        if g.in_supply(id::SAILOR) && cards::is(c, cards::DURATION) {
            let zone = GameState::gain_dest_zone(to);
            let ps = &mut g.players[p as usize];
            for i in 0..ps.pending_durations_len as usize {
                let e = &mut ps.pending_durations[i];
                if e.card == id::SAILOR && e.used < e.times {
                    e.used += 1;
                    g.stack.push(Frame { zone, act: Act::Play, subject: c, ..Frame::new(FrameKind::YesNo, p, id::SAILOR) });
                    break;
                }
            }
        }
    }

    /// Treasury: "when you discard this from play, if you didn't buy a Victory card this turn, you may put this onto your
    /// deck" (not if a Victory card was gained this Buy phase). One offer per copy in play.
    fn offers_on_discard(g: &mut GameState) {
        if g.in_supply(id::TREASURY) && !g.turn.gained_victory_in_buy {
            let p = g.turn.player;
            for _ in 0..g.players[p as usize].in_play.get(id::TREASURY) {
                g.stack.push(Frame {
                    zone: Zone::InPlay, act: Act::Topdeck, subject: id::TREASURY,
                    ..Frame::new(K::YesNo, p, id::TREASURY)
                });
            }
        }
    }
}

impl GameState {
    /// Corsair (owned by anyone else): "until your next turn, each other player trashes the first
    /// Silver or Gold they play each turn". `c` was just played by `p` (by auto-play or by an
    /// effect); it is trashed from play if it is the first this turn and some other player's
    /// Corsair is in effect against `p` (not if `p` was immune when it was played, `shielded`).
    pub(crate) fn corsair_trashes_first_silver_or_gold<S: EventSink>(&mut self, p: u8, c: CardId, sink: &mut S) {
        if self.turn.corsair_trashed_first || !self.in_supply(id::CORSAIR) {
            return;
        }
        let hit = (0..self.num_players).any(|v| {
            let owner = &self.players[v as usize];
            v != p && owner.in_play.has(id::CORSAIR) && owner.shielded & (1 << p) == 0
        });
        if hit {
            self.turn.corsair_trashed_first = true;
            if self.remove_from_play(p, c) {
                self.trash_card(p, c, true, sink);
                self.flush_trash_triggers(0, sink);
            }
        }
    }

    /// A lingering Duration Attack (Corsair, Blockade) was just played by `p`: remember which other
    /// players it doesn't affect, those immune right now (a Moat is revealed, a Lighthouse is in
    /// play). Several plays this turn (Throne Room) keep a player only if every one missed them.
    pub(crate) fn shield_lingering_attack<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let (vs, nv) = self.victims(sink);
        let mut hit = 0u8;
        for &v in &vs[..nv] {
            hit |= 1 << v;
        }
        let mut immune = 0u8;
        for v in self.others(p) {
            immune |= (1 << v) & !hit;
        }
        let ps = &mut self.players[p as usize];
        let earlier_play = ps.pending_durations[..ps.pending_durations_len as usize]
            .iter()
            .any(|e| matches!(e.card, id::CORSAIR | id::BLOCKADE | crate::sets::menagerie::ids::GATEKEEPER));
        ps.shielded = if earlier_play { ps.shielded & immune } else { immune };
    }
}

/// Frame handlers of this set's cards (the generic frames' card-specific parts).
impl GameState {
    pub(crate) fn run_native_village_add<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        let pi = p as usize;
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

    pub(crate) fn finish_island(&mut self, f: &Frame) {
        let p = f.player;
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

    pub(crate) fn finish_treasure_map<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        if f.count > 0 && f.self_trashed {
            // Paid off: the Map that trashed itself was used. (A lone Map trashing itself
            // for nothing is a loss, so it isn't recorded.)
            self.turn.used_up.add_or_drop(id::TREASURE_MAP, 1);
            // One gain after another, each answered by its reactions (Watchtower) before the next one covers it on the deck (the
            // stop-moving rule: a card on top of a deck can't be moved once it is covered up).
            let _ = sink;
            for _ in 0..4 {
                self.stack.push(Frame { depth: f.depth, ..gain_frame(p, f.source, u8::MAX, Filter::Card(id::GOLD), Dest::DeckTop) });
            }
        }
    }

    pub(crate) fn finish_sea_chart<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
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

    pub(crate) fn mode_native_village_take<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
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
                "Lighthouse", "Astrolabe", "Fishing Village", "Caravan", "Cutpurse", "Sailor",
                "Tide Pools", "Bazaar", "Corsair", "Merchant Ship", "Outpost", "Pirate",
                "Sea Witch", "Treasury", "Wharf",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Haven", "Native Village", "Lookout", "Sea Chart", "Warehouse", "Island",
                "Salvager", "Treasure Map",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
