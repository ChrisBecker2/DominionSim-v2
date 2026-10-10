//! Player interface. Agents see the game only through an honest `PlayerView`.

use crate::cards::CardId;
use crate::rng::Rng;
use crate::state::KnownStack;
use crate::counts::Counts;
use crate::sets::dark_ages::MixedPile;
use crate::engine::{Choice, ChoiceBuf, Decision, EventSink, Step};
use crate::state::{GameState, Phase, TurnState, MAX_PLAYERS};

/// What player `me` legitimately knows. A perfect-memory player knows the full contents of
/// their own deck (not its order, except cards they've seen placed on top), and the full card
/// composition of every opponent (all gains/trashes are public), but not how an opponent's
/// cards are split between hand, deck and discard.
#[derive(Clone, Copy)]
pub struct PlayerView<'a> {
    state: &'a GameState,
    me: u8,
}

impl<'a> PlayerView<'a> {
    pub fn new(state: &'a GameState, me: u8) -> Self {
        PlayerView { state, me }
    }
    pub fn me(&self) -> u8 {
        self.me
    }
    pub fn num_players(&self) -> u8 {
        self.state.num_players
    }
    pub fn turn(&self) -> &TurnState {
        &self.state.turn
    }
    pub fn phase(&self) -> Phase {
        self.state.turn.phase
    }
    pub fn is_my_turn(&self) -> bool {
        self.state.turn.player == self.me
    }

    // --- Own zones (fully known) ---
    pub fn hand(&self) -> &Counts {
        &self.state.players[self.me as usize].hand
    }
    pub fn in_play(&self) -> &Counts {
        &self.state.players[self.me as usize].in_play
    }
    pub fn discard(&self) -> &Counts {
        &self.state.players[self.me as usize].discard
    }
    /// My revealed / looked-at / set-aside cards (Sentry, Library, Bandit victim).
    pub fn revealed(&self) -> &Counts {
        &self.state.players[self.me as usize].set_aside
    }
    /// Remaining deck as a multiset (order unknown except `deck_known_top`).
    pub fn deck(&self) -> Counts {
        self.state.players[self.me as usize].deck_counts()
    }
    /// Known cards on top of my deck, top first.
    pub fn deck_known_top(&self) -> impl Iterator<Item = CardId> + '_ {
        self.state.players[self.me as usize].deck_known.iter_top_down()
    }
    /// Known cards at the very bottom of my deck (Secret Passage), nearest-to-unknown first.
    pub fn deck_known_bottom(&self) -> impl Iterator<Item = CardId> + '_ {
        self.state.players[self.me as usize].deck_known_bottom.iter_front_to_back()
    }
    pub fn deck_size(&self) -> u32 {
        self.state.players[self.me as usize].deck_size()
    }
    pub fn my_cards(&self) -> Counts {
        self.state.players[self.me as usize].all_cards()
    }
    pub fn my_vp(&self) -> i32 {
        self.state.player_vp(self.me)
    }
    /// The card the pending decision is about (`Decision::subject`: the card a Jester discarded, for its
    /// Mode decision), or `None` with no decision pending.
    pub fn pending_subject(&self) -> Option<CardId> {
        self.state.pending_decision().map(|d| d.subject)
    }
    /// My Coffers tokens (Cornucopia & Guilds).
    pub fn coffers(&self) -> u16 {
        self.state.players[self.me as usize].coffers
    }
    /// `p`'s Coffers tokens: public (the mat is face up).
    pub fn coffers_of(&self, p: u8) -> u16 {
        self.state.players[p as usize].coffers
    }
    /// Young Witch's Bane card, 0 when there is none.
    pub fn bane(&self) -> CardId {
        self.state.bane
    }
    /// Ferryman's pile card (0 when there is none) and how many of it are left.
    pub fn ferryman_pile(&self) -> (CardId, u8) {
        (self.state.ferryman, self.state.ferryman_left)
    }
    /// Rewards left in their pile (`cards::FIRST_REWARD + i`, outside the Supply).
    pub fn rewards_left(&self) -> [u8; crate::cards::NUM_REWARDS] {
        self.state.rewards
    }
    /// The Action and Treasure cards `p`'s Farmhands have set aside (public): played at the start of
    /// their next turn.
    pub fn farm_cards_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].farm_cards.counts()
    }
    /// Whether the rules let the current player buy `c` right now beyond cost and pile (Grand
    /// Market with a Copper in play).
    pub fn may_buy(&self, c: CardId) -> bool {
        self.state.may_buy(c)
    }
    /// My Native Village mat (Seaside): full contents, since it's private to its owner.
    pub fn native_village_mat(&self) -> Counts {
        self.state.players[self.me as usize].native_village_mat.counts()
    }
    /// Cards `p` gained on their own last completed turn (Seaside: Smugglers). Public
    /// information (all gains are public).
    pub fn last_turn_gains_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].last_turn_gains.counts()
    }

    // --- Adventures ---
    /// `p`'s Tavern mat (public).
    pub fn tavern_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].tavern.counts()
    }
    /// The Supply pile each of `p`'s pile tokens is on (`state::TOK_*` order), 0 = not on a pile.
    pub fn tokens_of(&self, p: u8) -> [CardId; crate::state::NUM_PILE_TOKENS] {
        self.state.players[p as usize].tok
    }
    /// The card under `p`'s Estate token (Inheritance), 0 = none.
    pub fn inherit_of(&self, p: u8) -> CardId {
        self.state.players[p as usize].inherit
    }
    /// `p`'s `ADV_*` flags: the -1 Card token on their deck, the -$1 token, the Journey token face down.
    pub fn adv_flags_of(&self, p: u8) -> u8 {
        self.state.players[p as usize].adv
    }
    /// Whether the Event `e` is in this game.
    pub fn has_event(&self, e: CardId) -> bool {
        crate::cards::is_event(e) && self.state.events & (1u64 << (e - crate::cards::FIRST_EVENT)) != 0
    }
    /// The Events of this game (card ids), in id order.
    pub fn events(&self) -> impl Iterator<Item = CardId> + '_ {
        (0..crate::cards::NUM_EVENTS as CardId).filter(|i| self.state.events & (1u64 << i) != 0).map(|i| crate::cards::FIRST_EVENT + i)
    }
    /// Whether `p` has bought the Event `e` this game, and whether this turn (Alms, Borrow, Save and Pilgrimage once a turn).
    pub fn event_bought_by(&self, p: u8, e: CardId) -> bool {
        crate::cards::is_event(e) && self.state.players[p as usize].events_bought & (1u64 << (e - crate::cards::FIRST_EVENT)) != 0
    }
    pub fn event_bought_this_turn(&self, e: CardId) -> bool {
        crate::cards::is_event(e) && self.state.turn.adv.bought & (1u64 << (e - crate::cards::FIRST_EVENT)) != 0
    }

    // --- Empires ---
    /// `p`'s Debt tokens (public).
    pub fn debt_of(&self, p: u8) -> u16 {
        self.state.players[p as usize].debt
    }
    /// `p`'s VP tokens (public).
    pub fn vp_tokens_of(&self, p: u8) -> u16 {
        self.state.players[p as usize].vp_tokens
    }
    /// VP tokens on the Supply pile of `c` (named by any of its cards: a split pile by either half, the Castles by any Castle).
    pub fn pile_vp(&self, c: CardId) -> u8 {
        self.state.emp.piles.vp_on(crate::cards::pile_of(c))
    }
    /// Debt tokens on the Supply pile of `c` (Tax).
    pub fn pile_debt(&self, c: CardId) -> u8 {
        self.state.emp.piles.debt_on(crate::cards::pile_of(c))
    }
    /// Whether the Landmark `l` is in this game.
    pub fn has_landmark(&self, l: CardId) -> bool {
        crate::cards::is_landmark(l) && self.state.emp.landmarks & crate::sets::empires::lm_bit(l) != 0
    }
    /// The Landmarks of this game (card ids), in id order.
    pub fn landmarks(&self) -> impl Iterator<Item = CardId> + '_ {
        (0..crate::cards::NUM_LANDMARKS as CardId).filter(|i| self.state.emp.landmarks & (1 << i) != 0).map(|i| crate::cards::FIRST_LANDMARK + i)
    }
    /// VP tokens left on the Landmark `l`.
    pub fn landmark_vp(&self, l: CardId) -> u8 {
        if crate::cards::is_landmark(l) { self.state.emp.lm_vp[crate::sets::empires::lm_index(l)] } else { 0 }
    }
    /// The pile the Obelisk scores (0 = none).
    pub fn obelisk_pile(&self) -> CardId {
        self.state.emp.obelisk
    }
    /// What the Landmarks would add to `p`'s score if the game ended now.
    pub fn landmark_score_of(&self, p: u8) -> i32 {
        self.state.landmark_score(p)
    }
    /// Whether Debt can be worth keeping in this game (Change is in it): then paying is a decision rather than automatic at the end of the turn.
    pub fn debt_decision_matters(&self) -> bool {
        self.state.emp.flags & crate::state::E_KEEP_DEBT != 0
    }
    /// The cards set aside under my Archives (private to me).
    pub fn archive(&self) -> Counts {
        self.state.players[self.me as usize].archive.counts()
    }
    /// The Castles left in the Castles pile, and its top Castle (the one that can be bought).
    pub fn castles_left(&self) -> u32 {
        self.state.castles_left()
    }
    pub fn castle_top(&self) -> Option<CardId> {
        self.state.castle_top()
    }
    /// Whether the card `c` could be gained or bought from its pile now (the top of a split pile or of the Castles).
    pub fn pile_open(&self, c: CardId) -> bool {
        self.state.pile_open(c)
    }
    /// Cost of buying the Event `e`.
    pub fn event_cost(&self, e: CardId) -> u8 {
        crate::cards::cost(e)
    }
    /// Whether the Event `e` could be bought now (cost, once-per-turn and once-per-game limits, something to do).
    pub fn event_buyable(&self, e: CardId) -> bool {
        crate::cards::is_event(e) && crate::sets::adventures::event_buyable(self.state, e)
    }
    /// `event_buyable` without the cost.
    pub fn event_available(&self, e: CardId) -> bool {
        crate::cards::is_event(e) && crate::sets::adventures::event_available(self.state, e)
    }

    // --- Public information ---
    /// Cards left in `c`'s pile: a Supply pile, Ferryman's pile or a Reward (the piles outside the
    /// Supply are counted here too, so `supply(Courser)` reads the Rewards pile).
    pub fn supply(&self, c: CardId) -> u8 {
        if crate::cards::is_reward(c) {
            self.state.rewards[(c - crate::cards::FIRST_REWARD) as usize]
        } else if c == self.state.ferryman && c != 0 && !self.state.in_supply(c) {
            self.state.ferryman_left
        } else if let Some(pile) = MixedPile::of_marker(c) {
            // The Knights or Ruins pile (named by its marker): the cards left in it.
            self.state.pile_size(pile)
        } else if c == crate::cards::id::CASTLES {
            self.state.castles_left().min(u8::MAX as u32) as u8
        } else if c == crate::cards::id::SPOILS {
            self.state.spoils
        } else if c == crate::cards::id::MADMAN {
            self.state.madman
        } else if c == crate::cards::id::MERCENARY {
            self.state.mercenary
        } else if c >= crate::cards::FIRST_TRAV && ((c - crate::cards::FIRST_TRAV) as usize) < crate::cards::NUM_TRAV {
            self.state.trav[(c - crate::cards::FIRST_TRAV) as usize]
        } else {
            self.state.supply.get(c)
        }
    }
    pub fn in_supply(&self, c: CardId) -> bool {
        match MixedPile::of_marker(c) {
            Some(pile) => self.state.has_pile(pile),
            None if c == crate::cards::id::CASTLES => self.state.in_supply(crate::cards::id::HUMBLE_CASTLE),
            None => self.state.in_supply(c),
        }
    }
    /// The top card of the Knights or Ruins pile (named by `pile`: `KNIGHTS`, `RUINS`, or any of its cards): the one card of it that may be
    /// bought or gained, `None` when the pile is empty or not in the game. Public.
    pub fn pile_top(&self, pile: CardId) -> Option<CardId> {
        MixedPile::of(pile).and_then(|p| self.state.pile_top(p))
    }
    /// The cards of the Knights or Ruins pile nobody has seen: the cards that the pile's hidden part is drawn from (all copies of each
    /// card less those in any player's zones, the trash and the top of the pile), in the order of the pile's cards. Public.
    pub fn pile_unseen(&self, pile: CardId) -> Counts {
        let mut out = Counts::EMPTY;
        if let Some(p) = MixedPile::of(pile) {
            let unseen = self.state.pile_unseen(p);
            for (i, m) in p.members().enumerate() {
                out.set(m, unseen[i]);
            }
        }
        out
    }
    /// Whether the players started with Shelters.
    pub fn shelters(&self) -> bool {
        self.state.shelters
    }
    /// The cards left in the Spoils, Madman and Mercenary piles (outside the Supply).
    pub fn spoils_left(&self) -> u8 {
        self.state.spoils
    }
    /// This game's supply piles (card ids, in id order).
    pub fn supply_cards(&self) -> impl Iterator<Item = CardId> + '_ {
        self.state.supply_cards()
    }
    /// What `c` costs right now (printed cost less this turn's reductions, e.g. Bridge).
    pub fn cost(&self, c: CardId) -> u8 {
        self.state.cost(c)
    }
    /// Whether `c` is a Treasure right now (Curse counts too, under Charlatan).
    pub fn is_treasure(&self, c: CardId) -> bool {
        self.state.is_treasure(c)
    }
    pub fn empty_piles(&self) -> u32 {
        self.state.empty_piles()
    }
    pub fn trash(&self) -> &Counts {
        &self.state.trash
    }
    /// Full card composition of any player (public via tracking gains/trashes).
    pub fn cards_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].all_cards()
    }
    /// `p`'s score if the game ended now: their cards, VP tokens and what the Landmarks add.
    pub fn vp_of(&self, p: u8) -> i32 {
        self.state.player_vp(p)
    }
    pub fn hand_size_of(&self, p: u8) -> u32 {
        self.state.players[p as usize].hand.total()
    }
    pub fn deck_size_of(&self, p: u8) -> u32 {
        self.state.players[p as usize].deck_size()
    }
    pub fn discard_size_of(&self, p: u8) -> u32 {
        self.state.players[p as usize].discard.total()
    }
    pub fn in_play_of(&self, p: u8) -> &Counts {
        &self.state.players[p as usize].in_play
    }
    pub fn turns_taken_of(&self, p: u8) -> u16 {
        self.state.players[p as usize].turns_taken
    }
    /// How many cards are on `p`'s Native Village mat (public count; its exact contents are
    /// private to `p` — see `native_village_mat`, which only exposes my own).
    pub fn native_village_mat_size_of(&self, p: u8) -> u32 {
        self.state.players[p as usize].native_village_mat.total()
    }
    /// `p`'s Island mat (Seaside): public, so every player can see its exact contents.
    pub fn island_mat_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].island_mat.counts()
    }
    /// The Action cards set aside by `p`'s Princes (Promo): public. Each is played at the start of
    /// `p`'s turns.
    pub fn prince_cards_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].prince_cards.counts()
    }

    /// If gaining `card` now would end the game at the end of this turn (last Province, or the
    /// final empty pile), the winners bitmask it would end with; otherwise `None`. Uses only
    /// public information (supply, everyone's VP from their known card composition).
    pub fn result_if_gained(&self, card: CardId) -> Option<u8> {
        let s = self.state;
        if !s.in_supply(card) || s.supply.get(card) == 0 {
            return None;
        }
        // Cheap precheck: only the last card of a pile can trigger the end.
        let pile_limit = if s.num_players >= 5 { 4 } else { 3 };
        let last_province = card == crate::cards::id::PROVINCE && s.supply.get(card) == 1;
        let last_pile = s.supply.get(card) == 1 && s.empty_piles() + 1 >= pile_limit;
        if !last_province && !last_pile {
            return None;
        }
        let mut after = *s;
        after.supply.remove(card);
        after.players[self.me as usize].discard.add(card, 1);
        after.result_if_turn_ends()
    }

    /// A seed derived only from what `me` can see, so every path that samples hidden
    /// information for the same position (a bot's own search, the analysis) samples alike.
    pub fn stable_seed(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let s = self.state;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.me.hash(&mut h);
        s.players[self.me as usize].hash(&mut h);
        s.turn.hash(&mut h);
        s.supply.hash(&mut h);
        s.trash.hash(&mut h);
        s.stack.hash(&mut h);
        for p in 0..s.num_players {
            if p != self.me {
                let ps = &s.players[p as usize];
                ps.all_cards().hash(&mut h);
                ps.hand.total().hash(&mut h);
                ps.discard.hash(&mut h);
                ps.in_play.hash(&mut h);
            }
        }
        h.finish()
    }

    /// A concrete `GameState` consistent with everything `me` honestly knows, for search.
    /// My own zones are exact. For each opponent, hand + deck are pooled (their split and any
    /// order are hidden from me) and a random hand of the same size is dealt back out; their
    /// discard, in-play and set-aside cards are public and kept. `chance_mode` is turned on,
    /// so draws from any unknown deck surface as `Step::Chance`.
    pub fn determinize(&self, rng: &mut Rng) -> GameState {
        let mut s = *self.state;
        s.chance_mode = true;
        s.pause_at_turn_start = false;
        // The state's own RNG stream is the real game's: it would reveal the next card of the Knights or Ruins pile exactly as it will
        // be. The hidden part of those piles is not stored in the state at all (only the top card is), so a determinization needs nothing
        // but a stream of its own to draw the next reveals from.
        s.rng = Rng::new(rng.next_u64());
        for p in 0..s.num_players {
            if p == self.me {
                continue;
            }
            let ps = &mut s.players[p as usize];
            let hand_size = ps.hand.total();
            let mut pool = ps.deck_known.counts();
            pool.add_all(&ps.deck_unknown);
            pool.add_all(&ps.deck_known_bottom.counts());
            pool.add_all(&ps.hand);
            // Masquerade: a card another player has already committed to pass is just as hidden
            // from me as their hand or deck until it's delivered (see `state::PlayerState::passed`).
            ps.passed.add_into(&mut pool);
            // Haven (Seaside): the card set aside from hand is exactly as hidden as the hand it
            // came from, unlike Blockade's set-aside card (already publicly gained). Pool every
            // live Haven's card in, then deal each a fresh (still-hidden) one back below, after
            // dealing the hand — so the two poolings don't sample from each other's cards.
            let mut haven_slots = [false; crate::state::PENDING_DURATIONS_CAP];
            for i in 0..ps.pending_durations_len as usize {
                let e = ps.pending_durations[i];
                if matches!(e.card, crate::cards::id::HAVEN | crate::cards::id::GEAR) && e.arg != 0 && ps.set_aside.remove(e.arg) {
                    pool.add(e.arg, 1);
                    haven_slots[i] = true;
                }
            }
            // The same for a Haven whose return is still waiting at the start of this player's
            // own turn (`TurnState::start_pending`).
            let mut start_haven_slots = [false; crate::state::START_PENDING_CAP];
            if s.turn.player == p {
                for i in 0..s.turn.start_pending.as_slice().len() {
                    let e = s.turn.start_pending.as_slice()[i];
                    if matches!(e.card, crate::cards::id::HAVEN | crate::cards::id::GEAR) && e.arg != 0 && ps.set_aside.remove(e.arg) {
                        pool.add(e.arg, 1);
                        start_haven_slots[i] = true;
                    }
                }
            }
            // Native Village mat (Seaside): private to its owner, so as hidden from me as the
            // hand/deck it's pooled with; only its *size* is public (unlike Blockade's set-aside
            // card, already publicly gained, or Island's mat, public outright). Pool it in, then
            // deal the same number of cards back out below, after the hand and Haven slots, so
            // none of the three poolings sample from each other's cards.
            let nv_mat_size = ps.native_village_mat.total();
            pool.add_all(&ps.native_village_mat.counts());
            let archive_size = ps.archive.total();
            ps.archive.add_into(&mut pool);
            if ps.saved != 0 {
                pool.add(ps.saved, 1);
            }
            let mut hand = Counts::EMPTY;
            for _ in 0..hand_size {
                let c = pool.nth(rng.below(pool.total()));
                pool.remove(c);
                hand.add(c, 1);
            }
            ps.hand = hand;
            for i in 0..ps.pending_durations_len as usize {
                if haven_slots[i] {
                    let c = pool.nth(rng.below(pool.total()));
                    pool.remove(c);
                    ps.set_aside.add(c, 1);
                    ps.pending_durations[i].arg = c;
                }
            }
            for i in 0..start_haven_slots.len() {
                if start_haven_slots[i] {
                    let c = pool.nth(rng.below(pool.total()));
                    pool.remove(c);
                    ps.set_aside.add(c, 1);
                    s.turn.start_pending.as_mut_slice()[i].arg = c;
                }
            }
            // Save's card (Adventures) is face down: as hidden as the hand it came from.
            if ps.saved != 0 {
                let c = pool.nth(rng.below(pool.total()));
                pool.remove(c);
                ps.saved = c;
            }
            ps.native_village_mat.clear();
            for _ in 0..nv_mat_size {
                let c = pool.nth(rng.below(pool.total()));
                pool.remove(c);
                ps.native_village_mat.add(c, 1);
            }
            // Cards set aside under an Archive are face down: as hidden as the deck they came from (only their number is public).
            ps.archive.clear();
            for _ in 0..archive_size {
                let c = pool.nth(rng.below(pool.total()));
                pool.remove(c);
                ps.archive.add(c, 1);
            }
            ps.deck_known = KnownStack::default();
            ps.deck_known_bottom = KnownStack::default();
            ps.deck_unknown = pool;
            ps.passed.clear();
        }
        s
    }
}

pub trait Agent: Send {
    fn name(&self) -> &str;
    /// Pick one of `choices` (never empty) for `decision`.
    fn choose(&mut self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice;
}

#[derive(Clone, Copy, Debug)]
pub struct GameResult {
    pub num_players: u8,
    pub scores: [i32; MAX_PLAYERS],
    pub turns: [u16; MAX_PLAYERS],
    /// Bitmask of winning seats (more than one bit = shared win).
    pub winners: u8,
    /// True if the game hit the turn cap rather than ending normally.
    pub capped: bool,
}

/// Play `state` to completion. `agents[i]` plays seat i. Allocation-free.
pub fn play_game<S: EventSink>(state: &mut GameState, agents: &mut [&mut dyn Agent], sink: &mut S) -> GameResult {
    assert!(!state.chance_mode, "play_game samples draws; disable chance_mode");
    let mut buf = ChoiceBuf::default();
    loop {
        match state.advance(sink) {
            Step::Decision(d) => {
                state.legal_choices(&mut buf);
                let view = PlayerView::new(state, d.player);
                let c = agents[d.player as usize].choose(&view, &d, buf.as_slice());
                state.apply_listed(c, &buf, sink).expect("agent chose an illegal option");
            }
            Step::Chance { .. } => unreachable!(),
            Step::TurnStart { .. } => continue,
            Step::GameOver => break,
        }
    }
    let n = state.num_players as usize;
    let mut turns = [0; MAX_PLAYERS];
    for p in 0..n {
        turns[p] = state.players[p].turns_taken;
    }
    GameResult {
        num_players: state.num_players,
        scores: state.scores(),
        turns,
        winners: state.winners(),
        capped: state.turn.number >= state.max_turns,
    }
}
