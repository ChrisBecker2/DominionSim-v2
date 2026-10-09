//! Intrigue, 2nd edition.

use super::prelude::*;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const COURTYARD: CardId = 33;
    pub const LURKER: CardId = 34;
    pub const PAWN: CardId = 35;
    pub const MASQUERADE: CardId = 36;
    pub const SHANTY_TOWN: CardId = 37;
    pub const STEWARD: CardId = 38;
    pub const SWINDLER: CardId = 39;
    pub const WISHING_WELL: CardId = 40;
    pub const BARON: CardId = 41;
    pub const BRIDGE: CardId = 42;
    pub const CONSPIRATOR: CardId = 43;
    pub const DIPLOMAT: CardId = 44;
    pub const IRONWORKS: CardId = 45;
    pub const MILL: CardId = 46;
    pub const MINING_VILLAGE: CardId = 47;
    pub const SECRET_PASSAGE: CardId = 48;
    pub const COURTIER: CardId = 49;
    pub const DUKE: CardId = 50;
    pub const MINION: CardId = 51;
    pub const PATROL: CardId = 52;
    pub const REPLACE: CardId = 53;
    pub const TORTURER: CardId = 54;
    pub const TRADING_POST: CardId = 55;
    pub const UPGRADE: CardId = 56;
    pub const HAREM: CardId = 57;
    pub const NOBLES: CardId = 58;
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::COURTYARD, intrigue(order_sensitive(c("Courtyard", 2, ACTION, 0, 0, 3, 0, 0)))), // what to put on the deck; a later draw takes it
    (ids::LURKER, intrigue(has_choice(c("Lurker", 2, ACTION, 0, 0, 0, 1, 0)))), // trash from Supply or gain from trash
    (ids::PAWN, intrigue(has_choice(c("Pawn", 2, ACTION, 0, 0, 0, 0, 0)))), // two of four bonuses
    (ids::MASQUERADE, intrigue(order_sensitive(c("Masquerade", 3, ACTION, 0, 0, 2, 0, 0)))), // what to pass / trash (may be the +Actions card in hand)
    (ids::SHANTY_TOWN, intrigue(order_sensitive(c("Shanty Town", 3, ACTION, 0, 0, 0, 2, 0)))), // draws only with no Actions in hand: order matters
    (ids::STEWARD, intrigue(order_sensitive(c("Steward", 3, ACTION, 0, 0, 0, 0, 0)))), // one of three (may be the +Actions card in hand)
    (ids::SWINDLER, intrigue(has_choice(c("Swindler", 3, ACTION | ATTACK, 2, 0, 0, 0, 0)))), // what the victims gain
    (ids::WISHING_WELL, intrigue(order_sensitive(c("Wishing Well", 3, ACTION, 0, 0, 1, 1, 0)))), // name a card, reveal the top; a miss stays known for a later draw
    (ids::BARON, intrigue(has_choice(c("Baron", 4, ACTION, 0, 0, 0, 0, 1)))), // discard an Estate?
    (ids::BRIDGE, intrigue(choice_free(c("Bridge", 4, ACTION, 1, 0, 0, 0, 1)))), // cost reduction only (no gains of its own)
    (ids::CONSPIRATOR, intrigue(has_choice(c("Conspirator", 4, ACTION, 2, 0, 0, 0, 0)))), // depends on actions played: order matters
    (ids::DIPLOMAT, intrigue(order_sensitive(c("Diplomat", 4, ACTION | REACTION, 0, 0, 2, 0, 0)))), // depends on hand size: order matters
    (ids::IRONWORKS, intrigue(has_choice(c("Ironworks", 4, ACTION, 0, 0, 0, 0, 0)))), // what to gain
    (ids::MILL, intrigue(order_sensitive(c("Mill", 4, ACTION | VICTORY, 0, 1, 1, 1, 0)))), // discard 2? (may be the +Actions card in hand)
    (ids::MINING_VILLAGE, intrigue(has_choice(c("Mining Village", 4, ACTION, 0, 0, 1, 2, 0)))), // trash it?
    (ids::SECRET_PASSAGE, intrigue(order_sensitive(c("Secret Passage", 4, ACTION, 0, 0, 2, 1, 0)))), // what to put where in the deck; a later draw may take it
    (ids::COURTIER, intrigue(order_sensitive(c("Courtier", 5, ACTION, 0, 0, 0, 0, 0)))), // what to reveal, which bonuses (may be the +Actions card in hand)
    (ids::DUKE, intrigue(c("Duke", 5, VICTORY, 0, 0, 0, 0, 0))), // 1 VP per Duchy (`state::vp_of_cards`)
    (ids::MINION, intrigue(order_sensitive(c("Minion", 5, ACTION | ATTACK, 0, 0, 0, 1, 0)))), // +$2 or new hands: cards drawn first may be discarded
    (ids::PATROL, intrigue(order_sensitive(c("Patrol", 5, ACTION, 0, 0, 3, 0, 0)))), // order of the cards put back; a later draw takes them
    (ids::REPLACE, intrigue(order_sensitive(c("Replace", 5, ACTION | ATTACK, 0, 0, 0, 0, 0)))), // may gain onto the deck; a later draw takes it
    (ids::TORTURER, intrigue(choice_free(c("Torturer", 5, ACTION | ATTACK, 0, 0, 3, 0, 0)))), // only the victims choose
    (ids::TRADING_POST, intrigue(order_sensitive(c("Trading Post", 5, ACTION, 0, 0, 0, 0, 0)))), // what to trash (may be the +Actions card in hand)
    (ids::UPGRADE, intrigue(order_sensitive(c("Upgrade", 5, ACTION, 0, 0, 1, 1, 0)))), // what to trash and gain (may be the +Actions card in hand)
    (ids::HAREM, intrigue(choice_free(c("Harem", 6, TREASURE | VICTORY, 2, 2, 0, 0, 0)))),
    (ids::NOBLES, intrigue(has_choice(c("Nobles", 6, ACTION | VICTORY, 0, 2, 0, 0, 0)))), // +3 Cards or +2 Actions
];

/// The Intrigue cards (see the module docs of `sets`).
pub struct Intrigue;

impl SetRules for Intrigue {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    const REACTION_EFFECTS: &'static [(CardId, u8, u8, u8)] = &[(id::DIPLOMAT, 5, 2, 3)];

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            id::PAWN => &[ModeOpt::Cards(1), ModeOpt::Actions(1), ModeOpt::Buys(1), ModeOpt::Coins(1)],
            id::STEWARD => &[ModeOpt::Cards(2), ModeOpt::Coins(2), ModeOpt::TrashFromHand(2)],
            id::NOBLES => &[ModeOpt::Cards(3), ModeOpt::Actions(2)],
            id::MINION => &[ModeOpt::Coins(2), ModeOpt::DiscardHandDraw { draw: 4, attack_min_hand: 5 }],
            id::COURTIER => &[ModeOpt::Actions(1), ModeOpt::Buys(1), ModeOpt::Coins(3), ModeOpt::Gain(id::GOLD, Dest::Discard)],
            id::LURKER => &[ModeOpt::TrashFromSupply(Filter::Action), ModeOpt::GainFromTrash(Filter::Action)],
            id::TORTURER => &[ModeOpt::DiscardFromHand(2), ModeOpt::Gain(id::CURSE, Dest::Hand)],
            _ => &[],
        }
    }

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        match card {
            id::BRIDGE => g.turn.cost_reduction += 1,
            id::COURTYARD => g.stack.push(select(p, card, Zone::Hand, Topdeck, Filter::Any, 1, 1, Then::Nothing)),
            id::SHANTY_TOWN => {
                // Reveal the whole hand (logged), then +2 Cards only if it holds no Action.
                let hand = g.players[p as usize].hand;
                for (c, n) in hand.iter() {
                    for _ in 0..n {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                }
                if !hand.any_type(ACTION) {
                    g.stack.push(draw_frame(p, card, 2));
                }
            }
            id::CONSPIRATOR => {
                // `turn.played` already counts this play (added above); count Action cards only,
                // since it also holds treasures played this turn.
                let actions_played: u32 = g.turn.played.iter().filter(|&(c, _)| cards::is(c, ACTION)).map(|(_, n)| n as u32).sum();
                if actions_played >= 3 {
                    g.turn.actions += 1;
                    sink.bonus(p, card, 1, 0, 0, 0);
                    g.stack.push(draw_frame(p, card, 1));
                }
            }
            id::BARON => g.stack.push(Frame {
                zone: Zone::Hand, act: Discard, subject: id::ESTATE, then: Then::YesCoinsElseGainSubject(4),
                ..Frame::new(K::YesNo, p, card)
            }),
            // A card a Prince plays isn't in play: it can't trash itself, so no +$2 either.
            id::MINING_VILLAGE if !g.turn.prince_play => g.stack.push(Frame {
                zone: Zone::InPlay, act: Trash, subject: card, then: Then::CoinsPerPick(2),
                ..Frame::new(K::YesNo, p, card)
            }),
            // "You may discard 2 cards, for +$2": all or nothing (with 1 card in hand, discarding
            // it is allowed but gives no $).
            id::MILL => g.stack.push(Frame { exact: true, ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 2, Then::CoinsIfCount { count: 2, coins: 2 }) }),
            id::PATROL => {
                // Reorders whatever's left after Victory/Curse cards are pulled to hand, same as
                // Sentry's put-back-in-any-order pattern.
                g.stack.push(Frame { ordered: true, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(Frame {
                    max: 4, then: Then::MoveMatchingToHand(Filter::VictoryOrCurse),
                    ..Frame::new(K::RevealTop, p, card)
                });
            }
            id::TRADING_POST => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 2, 2,
                Then::GainCardIfCount { count: 2, card: id::SILVER, dest: Dest::Hand },
            )),
            id::UPGRADE => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 1, filter: Filter::Any, dest: Dest::Discard, exact: true, dest_by_type: false },
            )),
            id::IRONWORKS => g.stack.push(Frame { then: Then::GainedTypeStatBonus, ..gain_frame(p, card, 4, Filter::Any, Dest::Discard) }),
            id::REPLACE => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 2, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: true },
            )),
            id::PAWN => g.stack.push(mode_frame(p, card, 2)),
            id::STEWARD | id::NOBLES | id::MINION | id::LURKER => g.stack.push(mode_frame(p, card, 1)),
            id::COURTIER => g.stack.push(select(p, card, Zone::Hand, Reveal, Filter::Any, 1, 1, Then::ModePerType)),
            id::TORTURER => {
                // Choice-free for the player: +3 Cards is vanilla (below); only the victims
                // (Moat allowing) get a Mode decision, each over the same `modes(TORTURER)` table.
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(mode_frame(v, card, 1));
                }
            }
            id::WISHING_WELL => g.stack.push(Frame::new(K::Name, p, card)),
            id::SWINDLER => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { chooser: p, ..Frame::new(K::TrashTopThenGain, v, card) });
                }
            }
            // Deferred (see `FrameKind::PassLeftBegin`) so "who has cards to pass" is checked
            // after this player's own +2 Cards (pushed below it) has resolved, not before.
            id::MASQUERADE => g.stack.push(Frame::new(K::PassLeftBegin, p, card)),
            id::SECRET_PASSAGE => {
                g.stack.push(select(p, card, Zone::Hand, Act::SetAside, Filter::Any, 1, 1, Then::TakeToDeckPosition))
            }
            id::DIPLOMAT => g.stack.push(Frame {
                min: 0, max: 0, then: Then::ActionsIfHandAtMost { max_hand: 5, actions: 2 },
                ..select(p, card, Zone::Hand, Discard, Filter::Any, 0, 0, Then::Nothing)
            }),
            _ => {}
        }
    }

    fn vp(card: CardId, all: &Counts, _total: i32) -> i32 {
        match card {
            id::DUKE => all.get(id::DUCHY) as i32,
            _ => cards::def(card).vp as i32,
        }
    }
}

impl GameState {
    /// The distinct cards Wishing Well may name: the player's whole deck (known + unknown), or
    /// their discard if the deck is empty (since drawing it would shuffle the discard in first).
    pub(crate) fn name_offer(&self, p: u8) -> Counts {
        let ps = &self.players[p as usize];
        if ps.deck_size() > 0 { ps.deck_counts() } else { ps.discard }
    }

    /// "Each player with cards passes one to the next such player to their left, at once"
    /// (Masquerade): once every passing player has chosen (their pick sits in their own
    /// `passed` zone), deliver each held card to the next passing player to the left, all at
    /// once.
    pub(crate) fn deliver_left_passes<S: EventSink>(&mut self, sink: &mut S) {
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
}

/// Frame handlers of this set's cards (the generic frames' card-specific parts).
impl GameState {
    pub(crate) fn run_pass_left_begin(&mut self, f: Frame) -> Run {
        let p = f.player;
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

    pub(crate) fn run_swindler<S: EventSink>(&mut self, f: Frame, sink: &mut S) -> Run {
        let p = f.player;
        match take_top!(self, p, sink) {
            None => {
                self.stack.pop();
            }
            Some(c) => {
                self.stack.pop();
                let cost = self.cost(c);
                self.stack.push(Frame {
                    chooser: f.chooser, max: cost, exact: true, potion: cards::potion_cost(c), dest: Dest::Discard, depth: f.depth,
                    ..Frame::new(K::Gain, p, f.source)
                });
                // The victim's "when you trash this" effects come before the gain.
                self.trash_card(p, c, true, sink);
                self.flush_trash_triggers(f.depth, sink);
            }
        }
        Run::Continue
    }

    pub(crate) fn run_deck_position(&self, f: Frame) -> Run {
        let p = f.player;
        let pi = p as usize;
        let max_known = self.players[pi].deck_known.len;
        Run::Decide(DecisionKind::DeckPosition { max_known }, f.subject)
    }

    pub(crate) fn deck_position_choices(&self, f: Frame, out: &mut ChoiceBuf) {
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

    pub(crate) fn apply_deck_position(&mut self, f: Frame, k: u8) {
        let pi = f.player as usize;
        self.stack.pop();
        let ps = &mut self.players[pi];
        ps.set_aside.remove(f.subject);
        if k == 255 {
            ps.deck_known_bottom.push_top(f.subject);
        } else {
            ps.deck_known.insert_from_top(k, f.subject);
        }
    }

    pub(crate) fn mode_discard_hand_draw<S: EventSink>(&mut self, f: &Frame, draw: u8, attack_min_hand: u8, sink: &mut S) {
        let p = f.player;
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
                "Bridge", "Torturer", "Harem",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Courtyard", "Masquerade", "Shanty Town", "Steward", "Wishing Well", "Diplomat",
                "Mill", "Secret Passage", "Courtier", "Minion", "Patrol", "Replace",
                "Trading Post", "Upgrade",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
