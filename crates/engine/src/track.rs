//! The stop-moving rule: which copy of a card an effect may still move (`docs/stop-moving-rule.md`).
//!
//! Zones are multisets, so a card has no identity. An effect that moves a card it put somewhere earlier ("this", the card it played,
//! the card it gained) may only do so while that card is still where it left it, and a different copy with the same name sitting in
//! the zone is not it. The copies an effect cares about are given a *token*: a serial (`u8`, never 0 or `NIP`) naming a [`Mark`] in
//! the turn's small [`Ledger`] (zone, owner, card). Frames carry the token (`Frame::tok`), the play of a card has one for the card's own
//! effects (`Ledger::cur`), and:
//!
//! * a move that targets the tracked copy (a card moving itself, a trigger moving the card it reacts to) calls `lose` first, so the
//!   effects that still hold the token find the copy gone, even when another copy of the same name sits in the zone;
//! * a move that does not know about the token (an untracked effect picks a card of that name from the zone) is read as moving an
//!   untracked copy as long as the zone holds more copies than the live tokens of that card (the pigeonhole check in `tok_valid` and
//!   `left_zone`); only when every copy left is tracked does it cost a token (the newest);
//! * a card on top of a deck is lost once anything is put on top of that deck (`cover_deck`) or the top is taken (`take_top`); a card
//!   in a discard pile is not lost by being covered up (the 2019 rule), only by being moved.
//!
//! A token with value 0 means "not tracked": the effect is a plain, single resolution, and the old zone-count check is exact (nothing
//! else can have moved the copy). `NIP` ("not in play") is the token of a card played from where it stays (Prince's set-aside card,
//! Captain, Necromancer, Band of Misfits, Overlord): an effect that moves "this" out of play can't.
//!
//! Cost: `Ledger::n` is 0 unless a tracking effect is active (a Throne-like play, a Procession, a Royal Carriage on the mat, a gain
//! with a watcher in the game, ...). Every hook starts with that one compare.

use crate::cards::CardId;
use crate::state::{Frame, GameState, SmallMultiset, Zone};

/// How many copies the ledger can track at once (a deep Throne Room chain, several gains with triggers waiting).
pub const LEDGER_CAP: usize = 6;

/// The token of a card played without being put into play (see the module docs).
pub const NIP: u8 = 255;

/// Where a tracked copy is expected to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TZone {
    #[default]
    InPlay,
    Discard,
    Hand,
    /// On top of the deck, uncovered.
    DeckTop,
}

/// One tracked copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Mark {
    pub tok: u8,
    pub owner: u8,
    pub zone: TZone,
    /// How many frames still rely on the token (it is dropped when the last lets go), with `DEAD` set once the copy has moved (or been
    /// covered up): the token is kept until its holders are done, so that it can't be mistaken for a new one.
    pub holders: u8,
    pub card: CardId,
}

const DEAD: u8 = 0x80;

impl Mark {
    #[inline]
    pub fn dead(&self) -> bool {
        self.holders & DEAD != 0
    }
}

/// Equality and hashing look at the marks and the cards held over only: the cursors (`cur`, `gain`, `ingain`, `want`, `disc`) are scratch
/// that is meaningless between steps, and states that differ only in them are the same position (search tables must not tell them apart).
#[derive(Clone, Copy, Debug)]
pub struct Ledger {
    /// Slots in use (live or dead marks).
    pub n: u8,
    next: u8,
    /// The token of the play being resolved right now (`resolve_effects`): what `play` arms read through `leave_play_self_now`.
    pub cur: u8,
    /// The token of the card the last gain moved (0 when nothing tracks it), for the gain effect that pushed a guard under the triggers.
    pub gain: u8,
    /// The token of the gain whose triggers are running right now (the ones that move the gained card at once, Villa and Blockade, tell
    /// the ledger through it); a gain a trigger makes has its own while it runs.
    pub ingain: u8,
    /// Set by a gain frame that wants the gained copy tracked even when no trigger is in the game (Summon, Replace, Continue, ...).
    pub want: bool,
    /// The token of the card a Vassal has just discarded, while its discard reactions are queued (0 outside).
    pub disc: u8,
    /// Where the card of the last finished gain landed (0 discard pile, 1 hand, 2 deck top): a gain effect that moves the card at once
    /// (Blockade) reads it, as a Ghost Town goes to the hand and Progress puts the card onto the deck.
    pub landed: u8,
    /// Cards that were in play when the turn started (Durations held over): not "played this turn", whatever the counts say.
    pub carried: SmallMultiset<4>,
    marks: [Mark; LEDGER_CAP],
}

impl PartialEq for Ledger {
    fn eq(&self, o: &Self) -> bool {
        self.n == o.n && self.marks[..self.n as usize] == o.marks[..o.n as usize] && self.carried == o.carried
    }
}
impl Eq for Ledger {}
/// An idle ledger hashes as the single `false` that the flag it replaced (`prince_play`) hashed, so states without tokens hash as before.
impl std::hash::Hash for Ledger {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if self.n == 0 && self.carried.is_empty() {
            false.hash(state);
        } else {
            true.hash(state);
            self.n.hash(state);
            self.marks[..self.n as usize].hash(state);
            self.carried.hash(state);
        }
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger::EMPTY
    }
}

impl Ledger {
    pub const EMPTY: Ledger = Ledger {
        n: 0,
        next: 1,
        cur: 0,
        gain: 0,
        ingain: 0,
        want: false,
        disc: 0,
        landed: 0,
        carried: SmallMultiset::EMPTY,
        marks: [Mark { tok: 0, owner: 0, zone: TZone::InPlay, holders: 0, card: 0 }; LEDGER_CAP],
    };

    pub fn get(&self, tok: u8) -> Option<&Mark> {
        self.marks[..self.n as usize].iter().find(|m| m.tok == tok)
    }

    fn index(&self, tok: u8) -> Option<usize> {
        self.marks[..self.n as usize].iter().position(|m| m.tok == tok)
    }

    /// A new token for a copy of `card` that is now in `zone` of `owner`, with one holder.
    pub fn mark(&mut self, owner: u8, zone: TZone, card: CardId) -> u8 {
        if self.n as usize == LEDGER_CAP {
            // Full: drop a dead mark, else the oldest (its holders then find it lost).
            let victim = self.marks.iter().position(|m| m.dead()).unwrap_or(0);
            self.remove_at(victim);
        }
        let mut t = self.next;
        loop {
            if t == 0 || t == NIP {
                t = 1;
            }
            if self.index(t).is_none() {
                break;
            }
            t += 1;
        }
        self.next = t.wrapping_add(1);
        self.marks[self.n as usize] = Mark { tok: t, owner, zone, holders: 1, card };
        self.n += 1;
        t
    }

    fn remove_at(&mut self, i: usize) {
        let n = self.n as usize;
        for j in i..n - 1 {
            self.marks[j] = self.marks[j + 1];
        }
        self.marks[n - 1] = Mark::default();
        self.n -= 1;
        if self.n == 0 {
            self.next = 1;
        }
    }

    /// The newest live mark of a copy of `card` in play for `owner`, if any.
    pub fn find_in_play(&self, owner: u8, card: CardId) -> Option<u8> {
        self.marks[..self.n as usize].iter().rev().find(|m| !m.dead() && m.zone == TZone::InPlay && m.owner == owner && m.card == card).map(|m| m.tok)
    }

    /// One more frame relies on `tok`.
    pub fn hold(&mut self, tok: u8) {
        if let Some(i) = self.index(tok) {
            let h = &mut self.marks[i].holders;
            if *h & !DEAD < 0x7f {
                *h += 1;
            }
        }
    }

    /// A holder is done with `tok`: the last one drops it.
    pub fn release(&mut self, tok: u8) {
        if let Some(i) = self.index(tok) {
            if self.marks[i].holders & !DEAD <= 1 {
                self.remove_at(i);
            } else {
                self.marks[i].holders -= 1;
            }
        }
    }

    /// The tracked copy was moved: whoever still holds the token finds it gone.
    pub fn lose(&mut self, tok: u8) {
        if let Some(i) = self.index(tok) {
            self.marks[i].holders |= DEAD;
        }
    }

    /// Live marks of `card` in `zone` of `owner`.
    pub fn live(&self, owner: u8, zone: TZone, card: CardId) -> u8 {
        self.marks[..self.n as usize].iter().filter(|m| !m.dead() && m.zone == zone && m.owner == owner && m.card == card).count() as u8
    }

    /// How many older live marks name the same place as `tok`'s.
    fn rank(&self, tok: u8) -> u8 {
        let mut r = 0;
        let Some(m) = self.get(tok) else { return 0 };
        for o in &self.marks[..self.n as usize] {
            if o.tok == tok {
                break;
            }
            if !o.dead() && o.zone == m.zone && o.owner == m.owner && o.card == m.card {
                r += 1;
            }
        }
        r
    }

    /// The newest live mark of that place is lost (a copy left and every copy that remains is tracked).
    pub fn kill_newest(&mut self, owner: u8, zone: TZone, card: CardId) {
        if let Some(i) = self.marks[..self.n as usize].iter().rposition(|m| !m.dead() && m.zone == zone && m.owner == owner && m.card == card) {
            self.marks[i].holders |= DEAD;
        }
    }

    /// Everything tracked in `zone` of `owner` is lost (the deck was covered or drawn from, the discard pile was shuffled away).
    pub fn kill_zone(&mut self, owner: u8, zone: TZone) {
        for m in &mut self.marks[..self.n as usize] {
            if m.zone == zone && m.owner == owner {
                m.holders |= DEAD;
            }
        }
    }
}

fn tzone(z: Zone) -> Option<TZone> {
    match z {
        Zone::InPlay => Some(TZone::InPlay),
        Zone::Discard => Some(TZone::Discard),
        Zone::Hand => Some(TZone::Hand),
        Zone::DeckTop => Some(TZone::DeckTop),
        _ => None,
    }
}

impl GameState {
    /// The copies of `card` in `zone` of `owner`.
    fn tracked_count(&self, owner: u8, zone: TZone, card: CardId) -> u8 {
        let ps = &self.players[owner as usize];
        match zone {
            TZone::InPlay => ps.in_play.get(card),
            TZone::Discard => ps.discard.get(card),
            TZone::Hand => ps.hand.get(card),
            TZone::DeckTop => (ps.deck_known.peek_top() == Some(card)) as u8,
        }
    }

    /// A tracked copy for `owner`: `card` has just been put into `zone` (by a play, a gain, a discard). Returns its token, with one holder.
    #[inline(never)]
    pub(crate) fn track(&mut self, owner: u8, zone: Zone, card: CardId) -> u8 {
        match tzone(zone) {
            Some(z) => self.turn.led.mark(owner, z, card),
            None => 0,
        }
    }

    /// A token for a copy of `card` that is in `owner`'s play area and is about to be played again, or called on, after a play that has
    /// finished (Scepter's pick, Daimyo, Citadel, Royal Carriage): which copy that is, is not known. If a play of that card is already tracked
    /// (the outer play that a Scepter replays) it is that one's token again, one holder more, so that the same copy is not counted twice;
    /// otherwise a new token.
    #[inline(never)]
    pub(crate) fn track_replay(&mut self, owner: u8, card: CardId) -> u8 {
        match self.turn.led.find_in_play(owner, card) {
            Some(t) => {
                self.turn.led.hold(t);
                t
            }
            None => self.turn.led.mark(owner, TZone::InPlay, card),
        }
    }

    /// Whether the tracked copy `tok` is still where it was put. 0 (untracked) counts as present; the caller checks the zone itself.
    #[inline(never)]
    pub(crate) fn tok_valid(&self, tok: u8) -> bool {
        match tok {
            0 => true,
            NIP => false,
            t => {
                let Some(m) = self.turn.led.get(t) else { return false };
                if m.dead() {
                    return false;
                }
                match m.zone {
                    // On top and nothing was put over it (`cover_deck` kills the mark) nor taken from it.
                    TZone::DeckTop => self.tracked_count(m.owner, m.zone, m.card) > 0,
                    z => self.turn.led.rank(t) < self.tracked_count(m.owner, z, m.card),
                }
            }
        }
    }

    /// May the card being played (or the frame's play, `tok`) move itself out of play: it is in play and nothing has moved it.
    #[inline(always)]
    pub(crate) fn self_in_play(&self, p: u8, card: CardId, tok: u8) -> bool {
        match tok {
            0 => self.players[p as usize].in_play.has(card),
            NIP => false,
            t => self.tok_valid(t) && self.players[p as usize].in_play.has(card),
        }
    }

    /// `self_in_play` for the play being resolved right now (`resolve_effects`).
    #[inline]
    pub(crate) fn self_in_play_now(&self, p: u8, card: CardId) -> bool {
        self.self_in_play(p, card, self.turn.led.cur)
    }

    /// A card moves itself out of play: removes the copy and tells the ledger which one went. False (nothing moves) when it isn't there.
    #[inline]
    pub(crate) fn leave_play_self(&mut self, p: u8, card: CardId, tok: u8) -> bool {
        if !self.self_in_play(p, card, tok) {
            return false;
        }
        self.players[p as usize].in_play.remove(card);
        if tok != 0 {
            self.turn.led.lose(tok);
        }
        true
    }

    /// `leave_play_self` for the play being resolved right now.
    #[inline]
    pub(crate) fn leave_play_self_now(&mut self, p: u8, card: CardId) -> bool {
        let t = self.turn.led.cur;
        self.leave_play_self(p, card, t)
    }

    /// A move by a frame that holds `tok` takes the tracked copy out of `zone` of `p` (`card` is what the frame moves): it is gone for every
    /// other holder. Does nothing for a frame without a token or about another card.
    #[inline(always)]
    pub(crate) fn tok_moved(&mut self, tok: u8, p: u8, zone: Zone, card: CardId) {
        if tok != 0 {
            self.tok_moved_slow(tok, p, zone, card);
        }
    }

    #[cold]
    #[inline(never)]
    fn tok_moved_slow(&mut self, tok: u8, p: u8, zone: Zone, card: CardId) {
        if tok != NIP {
            if let (Some(m), Some(z)) = (self.turn.led.get(tok), tzone(zone)) {
                if !m.dead() && m.zone == z && m.owner == p && m.card == card {
                    self.turn.led.lose(tok);
                }
            }
        }
    }

    /// Some effect that does not hold a token removed a copy of `card` from `zone` of `p`: if every copy that remains is tracked, the
    /// newest token of that place has lost its copy.
    #[inline]
    pub(crate) fn left_zone(&mut self, p: u8, zone: Zone, card: CardId) {
        if self.turn.led.n == 0 {
            return;
        }
        self.left_zone_slow(p, zone, card);
    }

    #[inline(never)]
    fn left_zone_slow(&mut self, p: u8, zone: Zone, card: CardId) {
        let Some(z) = tzone(zone) else { return };
        if z == TZone::DeckTop {
            self.turn.led.kill_zone(p, TZone::DeckTop);
            return;
        }
        while self.turn.led.live(p, z, card) > self.tracked_count(p, z, card) {
            self.turn.led.kill_newest(p, z, card);
        }
    }

    /// Something is put on top of `p`'s deck (or the top card is taken): the cards tracked on top are covered up.
    #[inline]
    pub(crate) fn cover_deck(&mut self, p: u8) {
        if self.turn.led.n != 0 {
            self.turn.led.kill_zone(p, TZone::DeckTop);
        }
    }

    /// An untracked removal of a copy of `card` from `p`'s play area (Mint's trash, an end-of-buy exchange): `in_play.remove` plus the ledger.
    #[inline]
    pub(crate) fn remove_from_play(&mut self, p: u8, card: CardId) -> bool {
        let ok = self.players[p as usize].in_play.remove(card);
        if ok {
            self.left_zone(p, Zone::InPlay, card);
        }
        ok
    }

    /// Whether the frame `f`, which moves or plays the copy its token tracks, has lost it: the token names that card in that zone of that
    /// player, and the copy is gone. A frame without a token, or about anything else, is never lost.
    #[inline(never)]
    pub(crate) fn frame_lost(&self, f: &Frame) -> bool {
        use crate::state::{Filter, FrameKind as K};
        if f.tok == 0 || f.tok == NIP {
            return false;
        }
        let Some(m) = self.turn.led.get(f.tok) else { return false };
        let card = match f.kind {
            K::YesNo => f.subject,
            K::Select => match f.filter {
                Filter::Card(c) => c,
                _ => return false,
            },
            _ => return false,
        };
        if m.card != card || m.owner != f.player || tzone(f.zone) != Some(m.zone) {
            return false;
        }
        !self.tok_valid(f.tok)
    }

    /// Frames that hold `tok` are done with it.
    #[inline]
    pub(crate) fn release_tok(&mut self, tok: u8) {
        if tok != 0 && tok != NIP {
            self.turn.led.release(tok);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::id;
    use crate::state::GameConfig;

    fn game() -> GameState {
        let mut g = GameState::new(&GameConfig { num_players: 2, seed: 1, ..Default::default() });
        g.players[0].discard.clear();
        g.players[0].deck_known.clear();
        g
    }

    #[test]
    fn a_card_on_top_of_the_deck_is_lost_once_covered_but_not_in_the_discard_pile() {
        let mut g = game();
        g.players[0].deck_known.push_top(id::SILVER);
        let top = g.track(0, Zone::DeckTop, id::SILVER);
        g.players[0].discard.add(id::GOLD, 1);
        let pile = g.track(0, Zone::Discard, id::GOLD);
        assert!(g.tok_valid(top) && g.tok_valid(pile));
        // Covering the discard pile with another card changes nothing; covering the deck loses the card on top.
        g.players[0].discard.add(id::COPPER, 1);
        assert!(g.tok_valid(pile));
        g.cover_deck(0);
        g.players[0].deck_known.push_top(id::COPPER);
        assert!(!g.tok_valid(top), "covered up on the deck");
        assert!(g.tok_valid(pile));
    }

    #[test]
    fn moving_an_untracked_copy_does_not_cost_the_tracked_one_but_moving_the_last_copy_does() {
        let mut g = game();
        g.players[0].discard.add(id::SILVER, 2);
        let t = g.track(0, Zone::Discard, id::SILVER);
        // An untracked effect takes a Silver: the other copy is the tracked one.
        g.players[0].discard.remove(id::SILVER);
        g.left_zone(0, Zone::Discard, id::SILVER);
        assert!(g.tok_valid(t));
        // Now the only copy left is the tracked one.
        g.players[0].discard.remove(id::SILVER);
        g.left_zone(0, Zone::Discard, id::SILVER);
        assert!(!g.tok_valid(t));
    }

    #[test]
    fn a_move_that_names_the_token_takes_that_copy_even_when_another_of_the_name_remains() {
        let mut g = game();
        g.players[0].discard.add(id::SILVER, 2);
        let t = g.track(0, Zone::Discard, id::SILVER);
        g.tok_moved(t, 0, Zone::Discard, id::SILVER);
        g.players[0].discard.remove(id::SILVER);
        assert!(!g.tok_valid(t), "gone, although a Silver is still in the pile");
        // A fresh token is never mistaken for it, whatever the serial.
        let t2 = g.track(0, Zone::Discard, id::SILVER);
        assert_ne!(t, t2);
        assert!(g.tok_valid(t2));
        g.release_tok(t);
        g.release_tok(t2);
        assert_eq!(g.turn.led.n, 0);
    }

    #[test]
    fn a_card_moved_away_and_back_is_not_the_tracked_copy() {
        let mut g = game();
        g.players[0].discard.add(id::SILVER, 1);
        let t = g.track(0, Zone::Discard, id::SILVER);
        // Shuffled into the deck (the reshuffle in `take_top` does this) and later discarded again.
        g.players[0].discard.clear();
        g.turn.led.kill_zone(0, TZone::Discard);
        g.players[0].discard.add(id::SILVER, 1);
        assert!(!g.tok_valid(t));
    }

    #[test]
    fn the_ledger_drops_the_oldest_mark_when_full() {
        let mut l = Ledger::EMPTY;
        let first = l.mark(0, TZone::InPlay, id::COPPER);
        for _ in 0..LEDGER_CAP {
            l.mark(0, TZone::InPlay, id::COPPER);
        }
        assert!(l.get(first).is_none(), "the oldest was dropped");
        assert_eq!(l.n as usize, LEDGER_CAP);
    }
}
