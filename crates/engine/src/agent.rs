//! Player interface. Agents see the game only through an honest `PlayerView`.

use crate::cards::CardId;
use crate::rng::Rng;
use crate::state::KnownStack;
use crate::counts::Counts;
use crate::sets::dark_ages::MixedPile;
use crate::engine::{BuyAvail, Choice, ChoiceBuf, Decision, DecisionKind, EventSink, Step};
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
    /// For a multi-card selection pending (Artificer's discards): the cards picked so far and the last card picked (picks come in
    /// non-decreasing card order, so the next one is at least that). `None` when no selection is on top of the effect stack.
    pub fn select_progress(&self) -> Option<(u8, CardId)> {
        let f = self.state.stack.top()?;
        (f.kind == crate::state::FrameKind::Select).then_some((f.count, f.last))
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
    /// The Action cards `p`'s Summons have set aside (public, face up): played at the start of their next turn.
    pub fn summon_cards_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].summon_cards.counts()
    }
    /// The cards my Churches set aside face down (private to me).
    pub fn church(&self) -> Counts {
        let mut c = Counts::EMPTY;
        self.state.players[self.me as usize].church.add_into(&mut c);
        c
    }
    /// How many cards `p`'s Churches have set aside face down (public; only their owner sees which).
    pub fn church_count_of(&self, p: u8) -> usize {
        self.state.players[p as usize].church.total()
    }
    // --- Plunder ---
    /// The copies of the Loot `c` left in the Loot pile (public: the cards gained are revealed, the order is what is hidden).
    pub fn loot_left_of(&self, c: CardId) -> u8 {
        if crate::cards::is_loot(c) { self.state.supply.get(c) } else { 0 }
    }
    /// The Loot cards left in the pile.
    pub fn loot_left(&self) -> u32 {
        (0..crate::cards::NUM_LOOT as CardId).map(|i| self.state.supply.get(crate::cards::FIRST_LOOT + i) as u32).sum()
    }
    /// Whether the game has the Loot pile.
    pub fn has_loot_pile(&self) -> bool {
        self.state.plu.flags & crate::state::PF_LOOT != 0
    }
    /// The Traits of the game with the Supply pile (a card standing for it) each sits on.
    pub fn traits(&self) -> impl Iterator<Item = (CardId, CardId)> + '_ {
        (0..crate::cards::NUM_TRAITS).filter(|&i| self.state.plu.traits & (1 << i) != 0).map(|i| (crate::cards::FIRST_TRAIT + i as CardId, self.state.plu.trait_pile[i]))
    }
    /// Whether the card `c` is of the pile that has the Trait `t`.
    pub fn has_trait(&self, t: CardId, c: CardId) -> bool {
        crate::cards::is_trait(t) && self.state.has_trait(t, c)
    }
    /// The "next time" Durations `p` has waiting in play: `(card, plays)`.
    pub fn waiting_of(&self, p: u8) -> impl Iterator<Item = (CardId, u8)> + '_ {
        let ps = &self.state.players[p as usize];
        ps.plu.wait[..ps.plu.wait_len as usize].iter().map(|e| (e.card, e.times))
    }
    /// The cards `p` has set aside in Plunder zones, by zone code (`state::Z_*`): what a viewer can see (their own, face down ones too).
    pub fn plunder_aside_of(&self, p: u8) -> Vec<(u16, Counts)> {
        let ps = &self.state.players[p as usize].plu;
        let mut out: Vec<(u16, Counts)> = Vec::new();
        for (k, n) in ps.z.iter() {
            let code = k >> 10;
            if p != self.me && ps.private_zone(code) {
                continue;
            }
            match out.iter_mut().find(|(c, _)| *c == code) {
                Some((_, cs)) => cs.add(k & 1023, n),
                None => {
                    let mut cs = Counts::EMPTY;
                    cs.add(k & 1023, n);
                    out.push((code, cs));
                }
            }
        }
        out.sort_by_key(|(c, _)| *c);
        out
    }
    /// How many cards `p` has set aside face down in Plunder zones (public; only their owner sees which).
    pub fn plunder_hidden_count_of(&self, p: u8) -> u32 {
        self.state.players[p as usize].plu.private_cards().total()
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
        crate::cards::is_event(e) && self.state.events.has((e - crate::cards::FIRST_EVENT) as u32)
    }
    /// The Events of this game (card ids), in id order.
    pub fn events(&self) -> impl Iterator<Item = CardId> + '_ {
        self.state.events.events()
    }
    /// Whether `p` has bought the Event `e` this game, and whether this turn (Alms, Borrow, Save and Pilgrimage once a turn).
    pub fn event_bought_by(&self, p: u8, e: CardId) -> bool {
        crate::cards::is_event(e) && self.state.players[p as usize].events_bought.has((e - crate::cards::FIRST_EVENT) as u32)
    }
    pub fn event_bought_this_turn(&self, e: CardId) -> bool {
        crate::cards::is_event(e) && self.state.turn.adv.bought.has((e - crate::cards::FIRST_EVENT) as u32)
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

    // --- Renaissance ---
    /// My Villagers: each is +1 Action when spent in the Action phase.
    pub fn villagers(&self) -> u16 {
        self.state.players[self.me as usize].ren.villagers
    }
    /// `p`'s Villagers (public: the mat is face up).
    pub fn villagers_of(&self, p: u8) -> u16 {
        self.state.players[p as usize].ren.villagers
    }
    /// Whether the Project `pr` is in this game.
    pub fn has_project_in_game(&self, pr: CardId) -> bool {
        crate::cards::is_project(pr) && self.state.ren.projects & crate::sets::renaissance::project_bit(pr) != 0
    }
    /// The Projects of this game (card ids), in id order.
    pub fn projects(&self) -> impl Iterator<Item = CardId> + '_ {
        (0..crate::cards::NUM_PROJECTS as CardId).filter(|i| self.state.ren.projects & (1u32 << i) != 0).map(|i| crate::cards::FIRST_PROJECT + i)
    }
    /// Whether `p` has put a cube on the Project `pr`.
    pub fn has_project(&self, p: u8, pr: CardId) -> bool {
        crate::cards::is_project(pr) && self.state.has_project(p, pr)
    }
    /// How many cubes `p` has put on Projects (at most two).
    pub fn projects_owned_by(&self, p: u8) -> u32 {
        self.state.players[p as usize].ren.projects.count_ones()
    }
    /// Whether the Project `pr` is on offer to the turn's player (in the game, not owned, a cube left), apart from its cost.
    pub fn project_open(&self, pr: CardId) -> bool {
        crate::cards::is_project(pr) && self.state.project_open(pr)
    }
    /// Whether the Project `pr` could be bought now: open, and the turn's $ pay for it (the Buy is the caller's check).
    pub fn project_buyable(&self, pr: CardId) -> bool {
        crate::cards::is_project(pr) && self.state.project_buyable(pr)
    }
    /// Whether `p` holds the Artifact `a`.
    pub fn has_artifact(&self, p: u8, a: CardId) -> bool {
        crate::cards::is_artifact(a) && self.state.has_artifact(p, a)
    }
    /// The seat holding the Artifact `a`.
    pub fn artifact_holder(&self, a: CardId) -> Option<u8> {
        if crate::cards::is_artifact(a) { self.state.artifact_holder(a) } else { None }
    }
    /// The tokens `p` has on Sinister Plot.
    pub fn plot_tokens_of(&self, p: u8) -> u8 {
        self.state.ren.plot[p as usize]
    }
    /// The cards `p`'s Cargo Ships have set aside (face up, public).
    pub fn cargo_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].ren.cargo.counts()
    }
    /// The cards my Researches set aside face down (private to me).
    pub fn research(&self) -> Counts {
        self.state.players[self.me as usize].ren.research.counts()
    }
    /// How many cards `p`'s Researches have set aside (public; only their owner sees which).
    pub fn research_count_of(&self, p: u8) -> u32 {
        self.state.players[p as usize].ren.research.total()
    }
    /// Whether some player has Fleet (the game gives them an extra round of turns after it would end).
    pub fn fleet_in_game(&self) -> bool {
        self.state.ren.projects != 0 && self.state.fleet_in_game()
    }
    /// Whether the Fleet round (the extra turns after the game would have ended) is under way.
    pub fn fleet_round(&self) -> bool {
        self.state.ren.flags & crate::state::REN_FLEET_ROUND != 0
    }

    // --- Menagerie ---
    /// The cards on `p`'s Exile mat (public).
    pub fn exile_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].men.exile.counts()
    }
    /// The cards on my Exile mat.
    pub fn exile(&self) -> Counts {
        self.exile_of(self.me)
    }
    /// The copies on `p`'s mat that Invest put there.
    pub fn invested_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].men.invest.counts()
    }
    /// The cards `p` has set aside with Delay, Reap and Way of the Turtle, to be played at the start of their next turn.
    pub fn delayed_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].men.aside.counts()
    }
    /// The Ways of this game (card ids), in id order.
    pub fn ways(&self) -> impl Iterator<Item = CardId> + '_ {
        (0..crate::cards::NUM_WAYS as CardId).filter(|i| self.state.men.ways & (1u32 << i) != 0).map(|i| crate::cards::FIRST_WAY + i)
    }
    /// Whether the Way `w` is in this game.
    pub fn has_way(&self, w: CardId) -> bool {
        crate::cards::is_way(w) && self.state.men.ways & crate::sets::menagerie::way_bit(w) != 0
    }
    /// The card set aside for Way of the Mouse (0: none).
    pub fn mouse_card(&self) -> CardId {
        self.state.men.mouse
    }
    /// The Horses left in their pile (outside the Supply).
    pub fn horses_left(&self) -> u8 {
        self.state.horses_left()
    }
    /// Whether this game has the Horse pile.
    pub fn has_horse_pile(&self) -> bool {
        self.state.has_horse_pile()
    }
    /// The cards `p` trashed on their last turn (Goatherd of the player to their left).
    pub fn trashed_last_of(&self, p: u8) -> u8 {
        self.state.players[p as usize].men.trashed_last
    }
    /// Whether some Gatekeeper's Attack is on against me.
    pub fn gatekept(&self) -> bool {
        self.state.men.hook & crate::state::H_GATE != 0 && self.state.gatekeeper_hits(self.me)
    }

    // --- Allies ---
    /// The Ally of the game (a pseudo-card id), 0 when there is none.
    pub fn ally(&self) -> CardId {
        self.state.al.ally
    }
    /// Whether the Ally `a` is the Ally of this game.
    pub fn has_ally(&self, a: CardId) -> bool {
        a != 0 && self.state.al.ally == a
    }
    /// My Favors.
    pub fn favors(&self) -> u8 {
        self.state.players[self.me as usize].al.favors
    }
    /// `p`'s Favors (public: the mat is face up).
    pub fn favors_of(&self, p: u8) -> u8 {
        self.state.players[p as usize].al.favors
    }
    /// The cards `p`'s Coastal Haven kept in hand for the next turn (public).
    pub fn kept_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].al.keep.counts()
    }
    /// The turns `p` has to skip (Lich).
    pub fn skips_of(&self, p: u8) -> u8 {
        self.state.players[p as usize].al.skips
    }
    /// The top card of the Allies split pile that `c` is a card of, `None` when `c` is not one or its pile is empty.
    pub fn split_top(&self, c: CardId) -> Option<CardId> {
        crate::cards::allies_pile_index(c).and_then(|i| self.state.al_top(i))
    }
    /// The cards of the Allies split pile that `c` is a card of, top first (public: the order can be looked at any time).
    pub fn split_cards(&self, c: CardId) -> Vec<CardId> {
        match crate::cards::allies_pile_index(c) {
            Some(i) => {
                let (cards, n) = self.state.al_pile_cards(i);
                cards[..n].to_vec()
            }
            None => Vec::new(),
        }
    }
    /// The Favor tokens on the Supply pile of `c` (Family of Inventors): each makes its cards cost $1 less.
    pub fn fam_tokens(&self, c: CardId) -> u8 {
        self.state.al_fam_less(c)
    }
    /// Whether rotating the pile of `c` (a card of an Allies split pile) would uncover another card.
    pub fn rotation_uncovers(&self, c: CardId) -> bool {
        self.state.al_rotate_useful(crate::cards::pile_of(c))
    }

    // --- Rising Sun ---
    /// The Buys the turn's player can use: the Buys, and under Flourishing Trade the Actions too.
    pub fn buys_left(&self) -> u16 {
        self.state.buys_left()
    }
    /// The Prophecy of the game (a pseudo-card id), 0 when there is none.
    pub fn prophecy(&self) -> CardId {
        self.state.rs.prophecy
    }
    /// Whether the Prophecy has happened (the last Sun token is gone): its rule applies to everyone for the rest of the game.
    pub fn prophecy_active(&self) -> bool {
        self.state.rs.active != 0
    }
    /// Whether the Prophecy `p` is in effect now.
    pub fn prophecy_in_effect(&self, p: CardId) -> bool {
        self.state.rs.active == p && p != 0
    }
    /// The Sun tokens left on the Prophecy (0 once it has happened, or with none).
    pub fn sun_tokens(&self) -> u8 {
        self.state.rs.suns
    }
    /// The card Riverboat sets aside (0 with no Riverboat in the game): public.
    pub fn riverboat_card(&self) -> CardId {
        self.state.rs.riverboat
    }
    /// What `p`'s Prophecy has set aside (Rapid Expansion's cards, face up: public) or, with Biding Time, `p`'s set-aside hand (face down: only my own
    /// is known, for the others an empty set; see `aside_count_of`).
    pub fn aside_of(&self, p: u8) -> Counts {
        if self.state.rs.active == crate::cards::id::BIDING_TIME && p != self.me {
            Counts::EMPTY
        } else {
            self.state.players[p as usize].aside.counts()
        }
    }
    /// How many cards `p`'s Prophecy has set aside (public).
    pub fn aside_count_of(&self, p: u8) -> u32 {
        self.state.players[p as usize].aside.total()
    }
    /// The cards Foresight has set aside for `p` until the end of the turn (face up, public).
    pub fn foresight_of(&self, p: u8) -> Counts {
        self.state.players[p as usize].foresight.counts()
    }
    /// The Shadow cards in my deck, which I can play from there (whether or not a copy is in my hand too).
    pub fn shadows_in_deck(&self) -> Counts {
        let mut out = Counts::EMPTY;
        if self.state.rs.flags & crate::state::R_SHADOW != 0 {
            for &s in &crate::sets::rising_sun::SHADOWS {
                if self.state.players[self.me as usize].deck_has(s) {
                    out.add(s, 1);
                }
            }
        }
        out
    }
    /// Whether I have gained a Gold this game (Kintsugi; only recorded in a game with Kintsugi).
    pub fn gained_a_gold(&self) -> bool {
        self.state.players[self.me as usize].rs & crate::state::P_RS_GOLD != 0
    }
    /// The cards the turn's player has gained this turn (only counted in a game with Receive Tribute).
    pub fn rs_gains_this_turn(&self) -> u8 {
        self.state.turn.rs.gains
    }
    /// Whether `c` is an Action right now (a Treasure too, under Enlightenment).
    pub fn is_action(&self, c: CardId) -> bool {
        self.state.is_action(c)
    }
    /// How many Action cards `p` has in play (Treasures too, under Enlightenment).
    pub fn actions_in_play_of(&self, p: u8) -> u8 {
        self.state.actions_in_play(p)
    }

    // --- Nocturne ---
    /// Whether this game has Boons (a Fate card is in the kingdom) / Hexes (a Doom card is).
    pub fn has_boons(&self) -> bool {
        self.state.noc.flags & crate::state::N_BOONS != 0
    }
    pub fn has_hexes(&self) -> bool {
        self.state.noc.flags & crate::state::N_HEXES != 0
    }
    /// Whether a Night card can be in a hand in this game (a Night phase may follow the Buy phase).
    pub fn night_possible(&self) -> bool {
        self.state.noc.flags & crate::state::N_NIGHT != 0
    }
    /// Whether the Boon `b` is in the Boons deck (face down: a Boon not yet drawn since the last shuffle; their order is hidden, which ones they
    /// are is public), in the Boons discard pile, set aside face up for Druid, or in front of a player until Clean-up.
    pub fn boon_in_deck(&self, b: CardId) -> bool {
        crate::cards::is_boon(b) && self.state.noc.boon_deck & (1 << (b - crate::cards::FIRST_BOON)) != 0
    }
    pub fn boon_in_discard(&self, b: CardId) -> bool {
        crate::cards::is_boon(b) && self.state.noc.boon_discard & (1 << (b - crate::cards::FIRST_BOON)) != 0
    }
    pub fn boon_set_aside_for_druid(&self, b: CardId) -> bool {
        crate::cards::is_boon(b) && self.state.noc.boon_druid & (1 << (b - crate::cards::FIRST_BOON)) != 0
    }
    /// The Boons in the deck / discard pile / set aside for Druid, as bit masks (bit `i` is the Boon `FIRST_BOON + i`).
    pub fn boon_deck_mask(&self) -> u16 {
        self.state.noc.boon_deck
    }
    pub fn boon_discard_mask(&self) -> u16 {
        self.state.noc.boon_discard
    }
    pub fn druid_boon_mask(&self) -> u16 {
        self.state.noc.boon_druid
    }
    /// The same for the Hexes.
    pub fn hex_in_deck(&self, h: CardId) -> bool {
        crate::cards::is_hex(h) && self.state.noc.hex_deck & (1 << (h - crate::cards::FIRST_HEX)) != 0
    }
    pub fn hex_in_discard(&self, h: CardId) -> bool {
        crate::cards::is_hex(h) && self.state.noc.hex_discard & (1 << (h - crate::cards::FIRST_HEX)) != 0
    }
    pub fn hex_deck_mask(&self) -> u16 {
        self.state.noc.hex_deck
    }
    pub fn hex_discard_mask(&self) -> u16 {
        self.state.noc.hex_discard
    }
    /// Whether `p` has the State `state` (Deluded, Envious, Miserable, Twice Miserable, Lost in the Woods).
    pub fn has_state(&self, p: u8, state: CardId) -> bool {
        use crate::cards::id;
        use crate::state::*;
        let f = self.state.players[p as usize].noc;
        match state {
            id::DELUDED => f & P_DELUDED != 0,
            id::ENVIOUS => f & P_ENVIOUS != 0,
            id::MISERABLE => f & P_MISERABLE != 0 && f & P_TWICE_MISERABLE == 0,
            id::TWICE_MISERABLE => f & P_TWICE_MISERABLE != 0,
            id::LOST_IN_THE_WOODS => self.state.noc.lost == p + 1,
            _ => false,
        }
    }
    /// The seat of the player with Lost in the Woods, `None` when nobody has it.
    pub fn lost_in_the_woods(&self) -> Option<u8> {
        self.state.noc.lost.checked_sub(1)
    }
    /// The Faithful Hounds `p` has set aside until the end of the turn.
    pub fn hounds_set_aside_of(&self, p: u8) -> u8 {
        self.state.players[p as usize].hounds
    }
    /// The Boons Blessed Villages took for `p` (bit per Boon), to be received at the start of their next turn (face up, public).
    pub fn boons_waiting_of(&self, p: u8) -> u16 {
        self.state.players[p as usize].boons_later
    }
    /// Whether Guardian protects `p` from other players' Attacks until the start of their next turn.
    pub fn guarded_of(&self, p: u8) -> bool {
        self.state.players[p as usize].noc & crate::state::P_GUARDED != 0
    }
    /// The cards gained so far this turn (counted while Monastery, Devil's Workshop or a card gained to the hand is in the game).
    pub fn gains_this_turn(&self) -> u8 {
        self.state.turn.noc.gains
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
        } else if (crate::cards::id::WILL_O_WISP..=crate::cards::id::GHOST).contains(&c) {
            self.state.noc.piles[(c - crate::cards::id::WILL_O_WISP) as usize]
        } else if (c as usize) < crate::cards::NUM_ZONE_IDS {
            self.state.supply.get(c)
        } else {
            // An Event, a Project...: no pile.
            0
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
        let mut after = s.dup();
        after.supply.remove(card);
        after.players[self.me as usize].discard.add(card, 1);
        after.result_if_turn_ends()
    }

    /// `result_if_gained(card)` once `first` has been gained as well (a second buy in the same turn): the winners if gaining `first`, then `card`,
    /// ends the game at the end of this turn; `None` if it does not, or `first` cannot be gained.
    pub fn result_if_gained_after(&self, first: CardId, card: CardId) -> Option<u8> {
        let s = self.state;
        if first == card || !s.in_supply(first) || s.supply.get(first) == 0 {
            return None;
        }
        let mut after = s.dup();
        after.supply.remove(first);
        after.players[self.me as usize].discard.add(first, 1);
        PlayerView::new(&after, self.me).result_if_gained(card)
    }

    /// Whether `card` is the last copy of its pile (the one that empties it). Events and the like have no pile: false.
    #[inline]
    pub fn is_last_of_pile(&self, card: CardId) -> bool {
        (card as usize) < crate::cards::NUM_ZONE_IDS && self.state.supply.get(card) == 1
    }

    /// Whether gaining `card`, the second-to-last of its pile (the Province or Colony), would let an opponent end the game on their next turn by
    /// taking the last one and win outright: the card adds its VP to either side, so that is when an opponent's score beats mine now, or ties it
    /// with fewer turns by then (this turn counts for me, their next turn for them: the end-of-game tie rule). Everything is public; the
    /// opponent is assumed to be able to afford it, and the card to be worth the same VP to both.
    pub fn leaves_last_card_to_opponent(&self, card: CardId) -> bool {
        let s = self.state;
        if !s.in_supply(card) || s.supply.get(card) != 2 {
            return false;
        }
        let me = self.me;
        let v = crate::cards::def(card).vp as i32;
        let key = |p: u8| (s.player_vp(p) + v, -(s.players[p as usize].turns_taken as i32 + 1));
        let mine = key(me);
        (0..s.num_players).filter(|&o| o != me).any(|o| key(o) > mine)
    }

    /// A seed derived only from what `me` can see, so every path that samples hidden
    /// information for the same position (a bot's own search, the analysis) samples alike.
    pub fn stable_seed(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let s = self.state;
        let mut h = BufferedSip::new();
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
        let mut s = self.state.dup();
        s.chance_mode = true;
        s.pause_at_turn_start = false;
        s.ask_treasures = false;
        // The state's own RNG stream is the real game's: it would give the same Boons, Hexes and draws exactly as they will be, so a
        // determinization has a stream of its own.
        s.rng = Rng::new(rng.next_u64());
        // The face-down part of the Knights and Ruins piles is stored in the state in its true order: it is dealt again from the cards
        // nobody has seen (for the Knights, the ones the pile still holds), so the real order cannot be read from a determinization.
        for pile in [&crate::sets::dark_ages::RUINS_PILE, &crate::sets::dark_ages::KNIGHTS_PILE] {
            if s.has_pile(pile) {
                s.deal_face_down(pile);
            }
        }
        // The Boons and Hexes are drawn at random from the cards of their decks (the order is not stored), so nothing about upcoming ones leaks;
        // a pending chance outcome is the search's own.
        s.noc.boon_top = 0;
        s.noc.hex_top = 0;
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
            // Biding Time (Rising Sun): the hand set aside face down is as hidden as the hand it came from.
            let biding_size = if s.rs.active == crate::cards::id::BIDING_TIME { ps.aside.total() } else { 0 };
            if biding_size > 0 {
                ps.aside.add_into(&mut pool);
            }
            let archive_size = ps.archive.total();
            ps.archive.add_into(&mut pool);
            // Research (Renaissance): the cards set aside face down are as hidden as the deck they came from.
            let research_size = ps.ren.research.total();
            ps.ren.research.add_into(&mut pool);
            // Church (Promo): the cards set aside face down are as hidden as the hand they came from; only the sizes of the batches are public.
            let church_sizes = ps.church.sizes();
            let church_batches = ps.church.len();
            ps.church.add_into(&mut pool);
            // Plunder: the cards set aside face down (on Cages and Grottos, for Puzzle Box) are as hidden as the hand they came from.
            let (plu_priv, plu_priv_n) = ps.plu.take_private();
            for &(_, c, m) in &plu_priv[..plu_priv_n] {
                pool.add(c, m);
            }
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
            let mut church = crate::state::ChurchBatches::EMPTY;
            for i in 0..church_batches {
                church.begin();
                for _ in 0..church_sizes[i] {
                    let c = pool.nth(rng.below(pool.total()));
                    pool.remove(c);
                    church.add(c);
                }
            }
            ps.church = church;
            for &(code, _, m) in &plu_priv[..plu_priv_n] {
                for _ in 0..m {
                    let c = pool.nth(rng.below(pool.total()));
                    pool.remove(c);
                    if !ps.plu.zadd(code, c, 1) {
                        ps.discard.add(c, 1);
                    }
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
            if biding_size > 0 {
                ps.aside.clear();
                for _ in 0..biding_size {
                    let c = pool.nth(rng.below(pool.total()));
                    pool.remove(c);
                    ps.aside.add(c, 1);
                }
            }
            ps.native_village_mat.clear();
            for _ in 0..nv_mat_size {
                let c = pool.nth(rng.below(pool.total()));
                pool.remove(c);
                ps.native_village_mat.add(c, 1);
            }
            ps.ren.research.clear();
            for _ in 0..research_size {
                let c = pool.nth(rng.below(pool.total()));
                pool.remove(c);
                ps.ren.research.add(c, 1);
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

/// `DefaultHasher` fed through a stack buffer. The derived `Hash` of a state is hundreds of one-field writes (an effect-stack frame alone is
/// about fourteen), and SipHash pays its tail handling on every call; writing the same bytes in a few large pieces gives the same hash
/// (SipHash is a function of the byte stream, not of how it is cut up) for a fraction of the time.
struct BufferedSip {
    inner: std::collections::hash_map::DefaultHasher,
    buf: [u8; Self::CAP],
    len: usize,
}

impl BufferedSip {
    const CAP: usize = 4096;
    fn new() -> Self {
        BufferedSip { inner: std::collections::hash_map::DefaultHasher::new(), buf: [0; Self::CAP], len: 0 }
    }
    fn flush(&mut self) {
        use std::hash::Hasher as _;
        self.inner.write(&self.buf[..self.len]);
        self.len = 0;
    }
}

impl std::hash::Hasher for BufferedSip {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() > Self::CAP - self.len {
            self.flush();
            if bytes.len() >= Self::CAP {
                self.inner.write(bytes);
                return;
            }
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
    }
    fn finish(&self) -> u64 {
        let mut h = self.inner.clone();
        h.write(&self.buf[..self.len]);
        h.finish()
    }
}

pub trait Agent: Send {
    fn name(&self) -> &str;
    /// Pick one of `choices` (never empty) for `decision`.
    fn choose(&mut self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice;
    /// The same pick for a plain Buy decision (`GameState::buy_avail`), from a test on a card instead of the list: `Some(c)` must be what `choose` would
    /// return for the list of the cards `avail` has and Pass; `None` (the default) has `play_game` build the list and call `choose`.
    #[inline]
    fn choose_buy(&mut self, _view: &PlayerView, _avail: &BuyAvail) -> Option<Choice> {
        None
    }
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
                // A plain Buy decision: a bot that can answer from a test on a card skips building the list of every pile it could buy.
                if d.kind == DecisionKind::Buy {
                    let c = match state.buy_avail() {
                        Some(avail) => agents[d.player as usize].choose_buy(&PlayerView::new(state, d.player), &avail),
                        None => None,
                    };
                    if let Some(c) = c {
                        state.apply_trusted(c, sink);
                        continue;
                    }
                }
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
