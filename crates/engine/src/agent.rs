//! Player interface. Agents see the game only through an honest `PlayerView`.

use crate::cards::CardId;
use crate::rng::Rng;
use crate::state::KnownStack;
use crate::counts::Counts;
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
    /// Remaining deck as a multiset (order unknown except `deck_known_top`).
    pub fn deck(&self) -> Counts {
        self.state.players[self.me as usize].deck_counts()
    }
    /// Known cards on top of my deck, top first.
    pub fn deck_known_top(&self) -> impl Iterator<Item = CardId> + '_ {
        self.state.players[self.me as usize].deck_known.iter_top_down()
    }
    pub fn deck_size(&self) -> u32 {
        self.state.players[self.me as usize].deck_size()
    }
    pub fn my_cards(&self) -> Counts {
        self.state.players[self.me as usize].all_cards()
    }
    pub fn my_vp(&self) -> i32 {
        self.state.players[self.me as usize].vp()
    }

    // --- Public information ---
    pub fn supply(&self, c: CardId) -> u8 {
        self.state.supply.get(c)
    }
    pub fn in_supply(&self, c: CardId) -> bool {
        self.state.in_supply(c)
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
    pub fn vp_of(&self, p: u8) -> i32 {
        self.state.players[p as usize].vp()
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

    /// A concrete `GameState` consistent with everything `me` honestly knows, for search.
    /// My own zones are exact. For each opponent, hand + deck are pooled (their split and any
    /// order are hidden from me) and a random hand of the same size is dealt back out; their
    /// discard, in-play and set-aside cards are public and kept. `chance_mode` is turned on,
    /// so draws from any unknown deck surface as `Step::Chance`.
    pub fn determinize(&self, rng: &mut Rng) -> GameState {
        let mut s = *self.state;
        s.chance_mode = true;
        s.pause_at_turn_start = false;
        for p in 0..s.num_players {
            if p == self.me {
                continue;
            }
            let ps = &mut s.players[p as usize];
            let hand_size = ps.hand.total();
            let mut pool = ps.deck_known.counts();
            pool.add_all(&ps.deck_unknown);
            pool.add_all(&ps.hand);
            let mut hand = Counts::EMPTY;
            for _ in 0..hand_size {
                let c = pool.nth(rng.below(pool.total()));
                pool.remove(c);
                hand.add(c, 1);
            }
            ps.hand = hand;
            ps.deck_known = KnownStack::default();
            ps.deck_unknown = pool;
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
                state.apply(c, sink).expect("agent chose an illegal option");
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
