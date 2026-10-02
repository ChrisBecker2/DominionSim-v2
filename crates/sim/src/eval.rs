//! Scoring a turn by a strategy's own priorities, for analysis of a strategy seat's decisions.

use dominion_engine::cards::{CardId, NUM_CARDS};
use dominion_engine::{GameState, PlayerView};
use dominion_engine::{Choice, Decision, DecisionKind};
use dominion_search::{game_end_value, Evaluator, NextHandEvaluator};

use crate::Strategy;
use dominion_engine::rng::Rng;
use dominion_search::{SearchConfig, Searcher};
use std::cell::RefCell;

/// Scores the end of a turn by the strategy's gain list (plus, with `win_this_turn`, a winning
/// turn above everything): every card gained this turn is worth
/// its place in the list, each entry outranking everything below it (weights are powers of 10),
/// and every card trashed this turn costs its own list value (so Remodel Gold -> Gold nets zero).
/// Conditions are evaluated at the start of the analyzed decision. Below every stated priority:
/// default preferences (attacking, trashing unwanted cards, playing actions/treasures), then a
/// tiny general-purpose economy term to break remaining ties.
pub struct GainListEvaluator<'a> {
    pub strategy: &'a Strategy,
    pub tie_break: NextHandEvaluator,
}

impl<'a> GainListEvaluator<'a> {
    pub fn new(strategy: &'a Strategy) -> Self {
        GainListEvaluator { strategy, tie_break: NextHandEvaluator::default() }
    }

    fn weight(&self, view: &PlayerView, card: CardId) -> f64 {
        match self.strategy.gain_rank(view, card) {
            // Tier 1: the gain list. Silver-at-the-bottom is worth 1000, each entry up is 10x.
            Some(rank) => 1000.0 * 10f64.powi((self.strategy.gain_list_len() - 1 - rank) as i32),
            None => 0.0,
        }
    }
}

impl GainListEvaluator<'_> {
    /// Below every stated gain priority (the smallest gain weight is 1000):
    /// tier 2, stated `[[play]]` rules: each play of a card whose rule's condition holds is worth
    /// 5 + (rules below it), so earlier rules count more;
    /// tier 3, defaults: each Attack played +0.5, each card trashed that the trash rules want gone
    /// +0.3, each Action played +0.1, each Treasure played +0.01.
    fn preferences(&self, root: &GameState, leaf: &GameState, me: u8, before: &dominion_engine::Counts, after: &dominion_engine::Counts) -> f64 {
        if leaf.turn.player != me {
            return 0.0;
        }
        let view = PlayerView::new(root, me);
        let mut p = 0.0;
        for (c, n) in leaf.turn.played.iter() {
            let n = n as f64;
            if let Some(below) = self.strategy.play_rules_below(&view, c) {
                p += (5.0 + below as f64) * n;
            }
            if dominion_engine::cards::is(c, dominion_engine::cards::ATTACK) {
                p += 0.5 * n;
            }
            // Cards that play other actions (Throne Room) are worth only what they play.
            if dominion_engine::cards::is(c, dominion_engine::cards::ACTION) && dominion_engine::cards::def(c).plays == 0 {
                p += 0.1 * n;
            }
            if dominion_engine::cards::is(c, dominion_engine::cards::TREASURE) {
                p += 0.01 * n;
                // A Treasure with a mode choice (Investment) that the strategy gains or states a
                // rule for is meant to be played, even though its forced trash of a card the
                // trash rules don't name costs 0.3: worth 0.5 per play.
                if !dominion_engine::cards::modes(c).is_empty() && (self.strategy.lists(c) || self.strategy.states_mode(c)) {
                    p += 0.5 * n;
                }
            }
        }
        for c in 0..NUM_CARDS as CardId {
            // A card that trashed itself as part of its effect was used, not lost.
            let gone = before.get(c) as i32 - after.get(c) as i32 - leaf.turn.used_up.get(c) as i32;
            // Trashing what the trash rules want is a plus; losing anything else (a lone
            // Treasure Map trashing itself, a forced trash) a small minus.
            if gone > 0 {
                p += if self.strategy.wants_trash(&view, c) { 0.3 } else { -0.3 } * gone as f64;
            }
        }
        p + self.vp_token_value(root, leaf, me)
    }

    /// VP tokens gained this turn (Investment, Monument, Bishop), in VP, on the preference scale
    /// (1 VP = 1.0: above the play preferences, far below any gain-list difference), net of what
    /// a card that trashed itself for them gives up: Investment's +$1 in every future hand it
    /// would have turned up in. Priced like `NextHandEvaluator`: `vp_per_coin` VP per coin over the
    /// `estimated_turns_left`, each card in a hand of 5 out of the deck's N cards.
    fn vp_token_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let tokens = leaf.players[me as usize].vp_tokens as f64 - root.players[me as usize].vp_tokens as f64;
        let mut v = tokens;
        if !leaf.turn.used_up.is_empty() {
            let w = &self.tie_break.weights;
            let turns = (dominion_search::estimated_turns_left(root, w) - 1.0).max(0.0);
            let n = (root.players[me as usize].all_cards().total() as f64).max(5.0);
            for (c, k) in leaf.turn.used_up.iter() {
                // Coins the card would have given each time played.
                let coin: f64 = dominion_engine::cards::modes(c)
                    .iter()
                    .map(|o| if let dominion_engine::cards::ModeOpt::Coins(x) = o { *x as f64 } else { 0.0 })
                    .sum();
                if dominion_engine::cards::is(c, dominion_engine::cards::TREASURE) {
                    v -= k as f64 * coin * w.vp_per_coin * turns * (5.0 / n);
                }
            }
        }
        v
    }
}

impl Evaluator for GainListEvaluator<'_> {
    fn leaf_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let view = PlayerView::new(root, me);
        let before = root.players[me as usize].all_cards();
        let after = leaf.players[me as usize].all_cards();
        let mut v = 0.0;
        for c in 0..NUM_CARDS as CardId {
            let d = after.get(c) as i32 - before.get(c) as i32;
            // A card that trashed itself as part of its effect (Investment, Mining Village) was
            // used, not lost: no charge for it.
            let d = d + leaf.turn.used_up.get(c) as i32;
            if d < 0 {
                v += d as f64 * self.weight(&view, c);
            }
            // Each gained copy is valued with its rule's condition checked as if the earlier
            // copies were already owned (e.g. `Witch if count(Witch) < 2` stops counting after one).
            for k in 0..d.max(0) {
                v += if k == 0 {
                    self.weight(&view, c)
                } else {
                    let mut owned = *root;
                    owned.players[me as usize].discard.add(c, k as u8);
                    owned.supply.set(c, owned.supply.get(c).saturating_sub(k as u8));
                    self.weight(&PlayerView::new(&owned, me), c)
                };
            }
        }
        // Strictly the strategy's own priorities. The general evaluator's game-end term is removed
        // from the tie-break: winning only matters if the rules say so. With `win_this_turn` (the
        // default) a turn that wins the game outright, acquiring only listed cards, outranks every
        // list priority, matching what the bot itself plays.
        let tie = self.tie_break.leaf_value(root, leaf, me) - game_end_value(leaf, me);
        // A turn that wins the game outright outranks everything, whatever it gains.
        let win = if self.strategy.win_this_turn && leaf.result_if_turn_ends() == Some(1 << me) { 1e15 } else { 0.0 };
        win + v + self.preferences(root, leaf, me, &before, &after) + 1e-6 * tie
    }

    /// Below the analyzed decision, play exactly as the strategy's rules do, so the analysis
    /// values each option by what the bot will actually do next.
    /// Outcomes call out only the actions the strategy's `[[play]]` rules name.
    fn outcome_shows_play(&self, card: CardId) -> bool {
        self.strategy.states_play(card)
    }

    /// In playouts past the node budget, play actions in the strategy's rule order (no search).
    fn playout_choice(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        let view = PlayerView::new(state, me);
        // `rule_play`/`rule_mode`/`rule_play_treasure` (not `decide_by_rules`) for play/mode/
        // treasure decisions: those would otherwise re-enter the strategy's own turn search
        // (`search_play_choice`) from inside a playout of that same search, re-borrowing its
        // thread-local searcher.
        Some(match decision.kind {
            DecisionKind::PlayAction | DecisionKind::Select { act: dominion_engine::Act::Play, .. } => self.strategy.rule_play(&view, decision, choices),
            DecisionKind::Mode { .. } => self.strategy.rule_mode(&view, decision, choices),
            DecisionKind::PlayTreasure => self.strategy.rule_play_treasure(choices),
            _ => self.strategy.decide_by_rules(&view, decision, choices),
        })
    }

    /// Below the root, follow the strategy's rules, except for decisions that shape the turn
    /// player's own turn (which action to play, what a Throne Room plays, which card to reveal,
    /// and any Mode decision belonging to the turn player themselves — Pawn/Steward/Nobles/
    /// Minion/Courtier/Lurker, never a reactive Mode decision during someone else's turn such as
    /// Torturer's victim): those are searched with this same scoring, which is exactly how the
    /// bot itself decides them.
    fn policy(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        let view = PlayerView::new(state, me);
        // Stated rules (a `[[mode]]` rule, a wanted trash inside a Treasure) are followed, not searched.
        if decision.player == me {
            if let Some(c) = self.strategy.stated_effect_choice(&view, decision, choices) {
                return Some(c);
            }
        }
        if is_play_decision(state.turn.player, decision) {
            // Play order is searched, unless it's no real choice (the bot plays the obvious order).
            if matches!(decision.kind, DecisionKind::PlayAction) && decision.player == me && self.strategy.search_play {
                let shortcut = crate::strategy::obvious_play(&view, choices).or_else(|| {
                    if self.strategy.states_play_for_hand(&view) {
                        None
                    } else {
                        crate::strategy::cantrip_first(&view, choices)
                    }
                });
                return shortcut.map(Choice::Card);
            }
            return None;
        }
        // When the game could end this turn, a `win_this_turn` strategy's decisions are maximized
        // here, as its win check does (buys/gains still restricted to listed cards by `allows`);
        // the win bonus in `leaf_value` finds the same wins without a search inside every node.
        if self.strategy.win_check_applies(&view) {
            return None;
        }
        Some(self.strategy.decide_by_rules(&view, decision, choices))
    }

    /// The strategy only buys cards in its gain list whose conditions hold, else Done; except that
    /// while its `win_this_turn` check applies, any buy is considered (a winning line may need an
    /// unlisted card, and the bot would play it).
    fn allows(&self, state: &GameState, me: u8, decision: &Decision, choice: Choice) -> bool {
        match (decision.kind, choice) {
            (DecisionKind::Buy, Choice::Card(c)) => {
                let view = PlayerView::new(state, me);
                self.strategy.win_check_applies(&view) || self.strategy.allows_buy(&view, c)
            }
            (DecisionKind::PlayTreasure, Choice::Card(c)) => !bank_too_early(state, me, c),
            _ => true,
        }
    }
}

/// Scores 1 if the turn ends the game with `me` winning outright, else 0. Every buy and gain is
/// considered (a winning line may need any card). Expectimax over this is the probability of a
/// certain win, so a value of 1 means a guaranteed win this turn.
pub struct WinFinder<'a> {
    pub strategy: &'a Strategy,
}

impl Evaluator for WinFinder<'_> {
    fn leaf_value(&self, _root: &GameState, leaf: &GameState, me: u8) -> f64 {
        if leaf.result_if_turn_ends() == Some(1 << me) { 1.0 } else { 0.0 }
    }

    fn value_when_nothing_allowed(&self) -> Option<f64> {
        Some(0.0)
    }

    /// Past the budget a line can't be proven (playouts are inexact), so keep playouts cheap.
    fn playout_choice(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        let view = PlayerView::new(state, me);
        // `rule_play`/`rule_mode`/`rule_play_treasure` (not `decide_by_rules`) for play/mode/
        // treasure decisions: those would otherwise re-enter the strategy's own turn search
        // (`search_play_choice`) from inside a playout of that same search, re-borrowing its
        // thread-local searcher.
        Some(match decision.kind {
            DecisionKind::PlayAction | DecisionKind::Select { act: dominion_engine::Act::Play, .. } => self.strategy.rule_play(&view, decision, choices),
            DecisionKind::Mode { .. } => self.strategy.rule_mode(&view, decision, choices),
            DecisionKind::PlayTreasure => self.strategy.rule_play_treasure(choices),
            _ => self.strategy.decide_by_rules(&view, decision, choices),
        })
    }
}

thread_local! {
    static WIN_SEARCHER: RefCell<Option<Searcher>> = const { RefCell::new(None) };
    static PLAY_SEARCHER: RefCell<Option<Searcher>> = const { RefCell::new(None) };
}

/// Whether `d` shapes the turn player's own turn the way an action-play choice does: which
/// action to play, which Treasure to play (`PlayTreasure`: Anvil, Bank, ...), what a Throne
/// Room/King's Court/Tiara plays, which card to reveal (Courtier), or any Mode decision
/// belonging to the player whose turn it currently is (Pawn/Steward/Nobles/Minion/Courtier/
/// Lurker). A Mode decision during someone else's turn (Torturer's victim) is a reactive
/// decision, not a play choice, so it's excluded even though `d.kind` matches.
pub fn is_play_decision(turn_player: u8, d: &Decision) -> bool {
    matches!(
        d.kind,
        DecisionKind::PlayAction
            | DecisionKind::PlayTreasure
            | DecisionKind::Select { act: dominion_engine::Act::Play, .. }
            | DecisionKind::Select { act: dominion_engine::Act::Reveal, .. }
    ) || (matches!(d.kind, DecisionKind::Mode { .. }) && d.player == turn_player)
        || is_treasure_effect_decision(turn_player, d)
        || is_start_effect_decision(turn_player, d)
}

/// A choice the turn player makes about the start of their own turn: the order the start-of-turn
/// effects resolve in (`DecisionKind::StartOrder`: a Haven's card returned before a Sea Witch
/// discards keeps it, a Sailor trashes after the draws). Searched like the play order.
///
/// Prince's own choices (set it aside, and which Action it holds) are not: a search that ends with
/// this turn can't see what a card played at the start of every later turn is worth, so it
/// would pick on this turn's tie-breaks alone. The bot follows its rules for them
/// (`Strategy::choose_prince_card`).
pub fn is_start_effect_decision(turn_player: u8, d: &Decision) -> bool {
    d.player == turn_player && d.kind == DecisionKind::StartOrder
}

/// A choice the turn player makes while one of their Treasures resolves (Investment's trash and
/// its +$1-or-VP choice, Crystal Ball's trash/discard/play, Anvil's discard): these change which
/// Treasures are left to play, so they're searched like the play order itself. Gains stay
/// list-driven.
pub fn is_treasure_effect_decision(turn_player: u8, d: &Decision) -> bool {
    d.player == turn_player
        && d.source.is_some_and(|c| dominion_engine::cards::is(c, dominion_engine::cards::TREASURE))
        && matches!(d.kind, DecisionKind::Select { .. } | DecisionKind::YesNo { .. } | DecisionKind::Mode { .. })
}

/// Bank is worth $1 per Treasure in play, so it's best played after every other Treasure: while
/// another Treasure is still in `me`'s hand, playing Bank now is never considered.
fn bank_too_early(state: &GameState, me: u8, c: CardId) -> bool {
    c == dominion_engine::cards::id::BANK
        && state.players[me as usize].hand.iter().any(|(x, _)| x != dominion_engine::cards::id::BANK && state.is_treasure(x))
}

const PLAY_TT_BITS: u32 = 16;
/// Node budgets tried in turn until the play search is exact: fast for ordinary turns, and as
/// deep as the analysis (`SearchConfig::default()`) for big ones.
const PLAY_BUDGETS: [u64; 2] = [1_000, 200_000];

/// The strategy's own search for a play decision: the turn is searched with the strategy's
/// scoring (gain list, then stated [[play]] rules, then defaults), its rules followed for every
/// other decision. Deterministic for a given position. Allocation-free after warm-up.
pub fn search_play_choice(strategy: &Strategy, view: &PlayerView) -> Option<Choice> {
    let world = view.determinize(&mut Rng::new(view.stable_seed()));
    let eval = GainListEvaluator::new(strategy);
    PLAY_SEARCHER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let searcher = slot.get_or_insert_with(|| Searcher::new(PLAY_TT_BITS));
        // Ordinary turns are solved exactly within the small budget. When they aren't (big turns
        // with many actions and draws), search again with a budget matching the analysis, so the
        // bot's choice doesn't come from shortcut playouts. Deterministic for a given position.
        let mut last = None;
        for budget in PLAY_BUDGETS {
            let cfg = SearchConfig { node_budget: budget, tt_bits: PLAY_TT_BITS, ..SearchConfig::default() };
            let r = searcher.best_choice(&world, view.me(), &cfg, &eval)?;
            last = Some(r.0);
            if r.2 {
                break;
            }
        }
        last
    })
}

/// `search_play_choice` for the start-of-turn decisions (the order of the start-of-turn effects,
/// Prince's choices): only the small node budget, and only an exact answer counts. The rules'
/// default order (cards back into the hand first, discards and trashes last) is right in nearly
/// every position, so a turn too big to solve within the small budget is not worth the larger one.
pub fn search_start_choice(strategy: &Strategy, view: &PlayerView) -> Option<Choice> {
    let world = view.determinize(&mut Rng::new(view.stable_seed()));
    let eval = GainListEvaluator::new(strategy);
    PLAY_SEARCHER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let searcher = slot.get_or_insert_with(|| Searcher::new(PLAY_TT_BITS));
        let cfg = SearchConfig { node_budget: PLAY_BUDGETS[0], tt_bits: PLAY_TT_BITS, ..SearchConfig::default() };
        let (choice, _, exact) = searcher.best_choice(&world, view.me(), &cfg, &eval)?;
        exact.then_some(choice)
    })
}

const WIN_TT_BITS: u32 = 16;

/// `win_this_turn`: the choice that starts a line of play winning the game this turn with
/// certainty (every draw outcome), acquiring only cards the strategy wants. Deterministic: the
/// hidden-information sample and the search are seeded identically for a given position.
/// `None` if no such line exists or it can't be proven within the node budget.
pub fn certain_win_choice(strategy: &Strategy, view: &PlayerView, choices: &[Choice]) -> Option<Choice> {
    let world = view.determinize(&mut Rng::new(view.stable_seed()));
    if world.pending_decision().map(|d| d.player) != Some(view.me()) {
        return None;
    }
    let cfg = SearchConfig { node_budget: 1_000, tt_bits: WIN_TT_BITS, ..SearchConfig::default() };
    let eval = WinFinder { strategy };
    let choice = WIN_SEARCHER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let searcher = slot.get_or_insert_with(|| Searcher::new(WIN_TT_BITS));
        searcher.first_choice_where(&world, view.me(), &cfg, &eval, |ev, exact| exact && ev >= 1.0 - 1e-9)
    })?;
    choices.contains(&choice).then_some(choice)
}
