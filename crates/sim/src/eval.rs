//! Scoring a turn by a strategy's own priorities, for analysis of a strategy seat's decisions.

use dominion_engine::cards::{id, CardId};
use dominion_engine::{GameState, PlayerView};
use dominion_engine::{Choice, Decision, DecisionKind, Step};
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
    /// What `leaf_value` needs of the search's root, computed once per root (`begin_root` .. `end_root`) instead of for every leaf.
    root: RefCell<Option<RootCache>>,
}

/// The parts of `leaf_value` that depend on the root alone.
struct RootCache {
    me: u8,
    /// Everything `me` owns at the root.
    before: dominion_engine::Counts,
    /// The Curses each player owns at the root.
    curses: [u8; dominion_engine::state::MAX_PLAYERS],
    /// What the gain list's best entry affordable with $m is worth (its weight, 0 when none), for m = 0..TIERS: the price of money in gain-list units.
    tiers: [f64; TIERS],
}

const TIERS: usize = 16;

/// How much of the average-hand estimate of a thinner deck counts: it ignores that a thinner deck also cycles faster and that a trash often comes
/// with an effect of its own (priced separately), and taken in full it overrates trashing in a few cards (Count 64 -> 53% against the rules at 1.0, 56 at 0.3).
const THINNING_SHARE: f64 = 0.3;

/// No more than this many of the gain list's last (cheapest) entries for each card thinned: a thinner deck is worth about a Silver at most, so a
/// trash that comes with a better choice (Amulet's Silver, Count's gains) is not preferred by it (Amulet 50 -> 47% against the rules without the cap).
const THINNING_CAP: f64 = 0.7;

/// The gain list's price of $m of hand (`RootCache::tiers`, interpolated between whole dollars).
fn tier_price(tiers: &[f64; TIERS], m: f64) -> f64 {
    let m = m.clamp(0.0, (TIERS - 1) as f64);
    let (lo, f) = (m.floor() as usize, m.fract());
    if lo + 1 < TIERS { tiers[lo] * (1.0 - f) + tiers[lo + 1] * f } else { tiers[lo] }
}

impl RootCache {
    fn new(root: &GameState, me: u8, strategy: &Strategy) -> Self {
        let view = PlayerView::new(root, me);
        let steps: [f64; TIERS] = std::array::from_fn(|m| strategy.best_gain_rank_up_to(&view, m as u8).map_or(0.0, |r| strategy.rank_weight(r)));
        // A hand's money varies around its average (a standard deviation of about $1.5), so the average hand is worth what hands around it are worth
        // on average: the steps smoothed with a Gaussian of that width.
        let kernel: [f64; 9] = std::array::from_fn(|i| (-((i as f64 - 4.0) / 1.5).powi(2) / 2.0).exp());
        let total: f64 = kernel.iter().sum();
        let tiers = std::array::from_fn(|m| {
            kernel.iter().enumerate().map(|(i, k)| k * steps[(m as i32 + i as i32 - 4).clamp(0, TIERS as i32 - 1) as usize]).sum::<f64>() / total
        });
        let mut curses = [0u8; dominion_engine::state::MAX_PLAYERS];
        for (o, c) in curses.iter_mut().enumerate().take(root.num_players as usize) {
            *c = root.players[o].count_of(id::CURSE).min(255) as u8;
        }
        RootCache { me, before: root.players[me as usize].all_cards(), curses, tiers }
    }
}

impl<'a> GainListEvaluator<'a> {
    pub fn new(strategy: &'a Strategy) -> Self {
        GainListEvaluator { strategy, tie_break: NextHandEvaluator::default(), root: RefCell::new(None) }
    }

    fn weight(&self, view: &PlayerView, card: CardId) -> f64 {
        match self.strategy.gain_rank(view, card) {
            // Tier 1: the gain list. Silver-at-the-bottom is worth 1000, each entry up is 10x.
            Some(rank) => self.strategy.rank_weight(rank),
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
    fn preferences(&self, root: &GameState, leaf: &GameState, me: u8, before: &dominion_engine::Counts, after: &dominion_engine::Counts, tiers: &[f64; TIERS]) -> f64 {
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
        let mut thinned = dominion_engine::Counts::EMPTY;
        let mut lost_any = !leaf.turn.used_up.is_empty() || leaf.trash.total() > root.trash.total();
        for (c, _) in before.iter() {
            // A card that trashed itself as part of its effect was used, not lost.
            let gone = before.get(c) as i32 - after.get(c) as i32 - leaf.turn.used_up.get(c) as i32;
            // Trashing what the trash rules want is a plus; losing anything else (a lone
            // Treasure Map trashing itself, a forced trash) a small minus.
            if gone > 0 {
                lost_any = true;
                let wanted = self.strategy.wants_trash(&view, c);
                p += if wanted { 0.3 } else { -0.3 } * gone as f64;
                if wanted {
                    thinned.add(c, gone.min(255) as u8);
                }
            }
        }
        p + self.thinning_value(root, before.total(), after, &thinned, tiers)
            + self.vp_token_value(root, leaf, me, tiers, lost_any)
            + self.coffers_value(root, leaf, me)
            + self.mat_return_value(root, leaf, me)
            + self.mat_copper_value(root, leaf, me, tiers)
            + self.next_turn_value(root, leaf, me, tiers)
            + self.debt_value(root, leaf, me)
            + sun_value(root, leaf, me)
    }

    /// What a deck without the cards the trash rules wanted gone (`thinned`, trashed this turn) is worth over the one with them, in gain-list units: each
    /// remaining turn's hand holds `average_hand_money` of the deck, and money is priced by the gain list (`tiers`: the weight of the best entry
    /// affordable, interpolated between whole dollars). Without it the search sees a trash only as the buy it costs this turn, and a trasher's play
    /// looks worse than passing; the cards lost that the rules did not want are charged by their own gain-list weight (above).
    fn thinning_value(&self, root: &GameState, before_total: u32, after: &dominion_engine::Counts, thinned: &dominion_engine::Counts, tiers: &[f64; TIERS]) -> f64 {
        if thinned.is_empty() {
            return 0.0;
        }
        let turns = (dominion_search::estimated_turns_left(root, &self.tie_break.weights) - 1.0).max(0.0);
        if turns <= 0.0 {
            return 0.0;
        }
        let price = |m: f64| tier_price(tiers, m);
        // Only a deck that shrinks: a trash that comes with gains (Remodel, Develop, Count) is priced by those gains.
        if after.total() >= before_total {
            return 0.0;
        }
        let mut with = *after;
        with.add_all(thinned);
        // Never a charge: a card the rules want trashed is not a loss, and the average hand ignores that a thinner deck cycles faster.
        (THINNING_SHARE * turns * (price(dominion_search::average_hand_money(after)) - price(dominion_search::average_hand_money(&with)))).max(0.0).min(thinned.total() as f64 * THINNING_CAP * self.strategy.rank_weight(self.strategy.gain_list_len().saturating_sub(1)))
    }

    /// Debt is money owed out of a later hand: each token is the opposite of a Coffers token, `vp_per_coin` while the game has turns left to pay it
    /// in (nothing in the last turn). In a game with Change, one token left unpaid is worth something too: Change gives +$3 instead of remodeling
    /// while its player has Debt, so for the chance that a Change is in the next hand (5 cards of the deck) a token is worth +$3 of that hand.
    /// On the preference scale like Coffers: more than any play preference, never more than a gain-list difference.
    fn debt_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let (before, after) = (root.players[me as usize].debt as f64, leaf.players[me as usize].debt as f64);
        if before == 0.0 && after == 0.0 {
            return 0.0;
        }
        let w = &self.tie_break.weights;
        if dominion_search::estimated_turns_left(root, w) - 1.0 <= 0.0 {
            return 0.0;
        }
        let mut v = -(after - before) * w.vp_per_coin;
        if after > 0.0 && root.emp.flags & dominion_engine::state::E_KEEP_DEBT != 0 {
            let mine = root.players[me as usize].all_cards();
            let changes = mine.get(dominion_engine::cards::id::CHANGE) as f64;
            let n = (mine.total() as f64).max(5.0);
            v += 3.0 * w.vp_per_coin * (5.0 * changes / n).min(1.0);
        }
        v
    }

    /// Reserve cards that left the Tavern mat this turn go back into the deck cycle (a called card is discarded at Clean-up, a Wine Merchant
    /// goes straight to the discard pile): each is worth what its next play gives (`reserve_play_coins`) in a future hand, priced like Coffers
    /// and VP tokens: `vp_per_coin` per coin, over the turns left, each card in a hand of 5 out of the deck's N cards. On the preference scale:
    /// above the play preferences, far below any gain-list difference, so a line that leaves $2 unspent to return a Wine Merchant beats one
    /// that buys a card nothing lists with it, and loses to a listed buy.
    fn mat_return_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let before = &root.players[me as usize].tavern;
        if before.is_empty() {
            return 0.0;
        }
        let after = &leaf.players[me as usize].tavern;
        let w = &self.tie_break.weights;
        let turns = (dominion_search::estimated_turns_left(root, w) - 1.0).max(0.0);
        if turns <= 0.0 {
            return 0.0;
        }
        let n = (root.players[me as usize].all_cards().total() as f64).max(5.0);
        let mut v = 0.0;
        for (c, k) in before.iter() {
            let returned = k.saturating_sub(after.get(c)) as f64;
            v += returned * reserve_play_coins(c) * w.vp_per_coin * turns * (5.0 / n);
        }
        v
    }

    /// Coppers put on the Tavern mat this turn (Miser): each is +$1 in every future hand that plays a Miser to cash the mat in, so over the turns left
    /// it is worth that many dollars of hand, priced by the gain list's tiers around the deck's average hand. Without it the search sees the Copper
    /// only as $1 lost from this turn's buy and never builds a mat. (The dollars a cash-in gives this turn are counted by the buy it makes.)
    fn mat_copper_value(&self, root: &GameState, leaf: &GameState, me: u8, tiers: &[f64; TIERS]) -> f64 {
        let delta = leaf.players[me as usize].tavern.get(id::COPPER) as f64 - root.players[me as usize].tavern.get(id::COPPER) as f64;
        if delta == 0.0 {
            return 0.0;
        }
        let turns = (dominion_search::estimated_turns_left(root, &self.tie_break.weights) - 1.0).max(0.0);
        let all = root.players[me as usize].all_cards();
        let n = (all.total() as f64).max(5.0);
        let p_miser = (5.0 * all.get(id::MISER) as f64 / n).min(1.0);
        let m = dominion_search::average_hand_money(&all);
        let slope = (tier_price(tiers, m + 1.0) - tier_price(tiers, m)).max(0.0);
        delta * turns * p_miser * slope
    }

    /// What the Durations played this turn (and the cards a Haven set aside) bring to next turn's hand, as money priced by the gain list's tiers:
    /// the extra cards, the $ and the returning Treasures, on top of the average hand (not Farrier's or Expedition's extra draw, which is an overpay the rules size; and only for Durations with a choice, Tactician and Haven: a choice-free Duration's value must not depend on the order it is played in, which the play shortcuts rely on). Without it the search sees only what a Tactician
    /// costs this turn (the hand discarded), and a Haven's set-aside Treasure as a loss. Debt and Coffers have their own terms.
    fn next_turn_value(&self, root: &GameState, leaf: &GameState, me: u8, tiers: &[f64; TIERS]) -> f64 {
        let (r, l) = (&root.players[me as usize], &leaf.players[me as usize]);
        if l.pending_durations_len == 0 && r.pending_durations_len == 0 {
            return 0.0;
        }
        if dominion_search::estimated_turns_left(root, &self.tie_break.weights) - 1.0 <= 0.0 {
            return 0.0;
        }
        let bonus = |ps: &dominion_engine::state::PlayerState| -> f64 {
            let (mut cards, mut coins) = (0u32, 0u32);
            for e in ps.pending_durations[..ps.pending_durations_len as usize].iter().filter(|e| !dominion_engine::cards::is_choice_free(e.card)) {
                let (c, _, _, k) = dominion_engine::cards::duration_bonus(e.card);
                cards += c as u32 * e.times as u32;
                coins += k as u32 * e.times as u32;
                // Haven: the Treasure set aside is back in the hand.
                if e.card == id::HAVEN && e.arg != 0 && dominion_engine::cards::is(e.arg, dominion_engine::cards::TREASURE) {
                    coins += dominion_engine::cards::def(e.arg).coins as u32;
                }
            }
            // An extra card is worth an average card of the whole deck, not of what is left to draw: what the cards drawn this turn were must not
            // matter, or the order of two cantrips would.
            coins as f64 + cards as f64 * dominion_search::average_hand_money(&ps.all_cards()) / 5.0
        };
        let m = dominion_search::average_hand_money(&l.all_cards());
        tier_price(tiers, m + bonus(l)) - tier_price(tiers, m + bonus(r))
    }

    /// Coffers tokens gained this turn, net of those spent: a token is +$1 in a future hand, worth
    /// `vp_per_coin` while the game has turns left to spend it (nothing in the last turn). On the
    /// preference scale: more than any play preference, never more than a gain-list difference, so a
    /// token spent to reach a listed card is repaid by that card, while one spent without changing
    /// the buy is a loss, and +Coffers beats nothing but not a better buy.
    fn coffers_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let delta = leaf.players[me as usize].coffers as f64 - root.players[me as usize].coffers as f64;
        if delta == 0.0 {
            return 0.0;
        }
        let w = &self.tie_break.weights;
        if dominion_search::estimated_turns_left(root, w) - 1.0 <= 0.0 {
            return 0.0;
        }
        delta * w.vp_per_coin
    }

    /// VP tokens gained this turn (Investment, Monument, Bishop), in VP, on the preference scale
    /// (1 VP = 1.0: above the play preferences, far below any gain-list difference), net of what
    /// a card that trashed itself for them gives up: Investment's +$1 in every future hand it
    /// would have turned up in. Priced like `NextHandEvaluator`: `vp_per_coin` VP per coin over the
    /// `estimated_turns_left`, each card in a hand of 5 out of the deck's N cards.
    fn vp_token_value(&self, root: &GameState, leaf: &GameState, me: u8, tiers: &[f64; TIERS], traded: bool) -> f64 {
        let tokens = leaf.players[me as usize].vp_tokens as f64 - root.players[me as usize].vp_tokens as f64;
        let mut v = tokens;
        let m = dominion_search::average_hand_money(&root.players[me as usize].all_cards());
        let slope = ((tier_price(tiers, m + 2.0) - tier_price(tiers, m - 2.0)) / 4.0).max(0.0);
        // Tokens given for a card (Bishop, Temple, Investment) are weighed against the card's worth; tokens that cost nothing (Monument) stay a plain
        // preference, so that when to play them never outweighs a buy.
        if tokens > 0.0 && traded {
            // What the strategy would pay for the same VP in Victory cards, or what the same VP are worth as money (`vp_per_coin`) at the price of
            // the gain list's tiers around the deck's average hand, whichever is more.
            let as_cards = tokens * self.strategy.vp_token_weight(&PlayerView::new(root, me));
            let as_money = tokens * slope / self.tie_break.weights.vp_per_coin;
            v += as_cards.max(as_money);
        }
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
                    // The same price as the VP above: a coin in every future hand it would have turned up in.
                    // The same price as the VP above: a coin in every future hand it would have turned up in.
                    v -= k as f64 * coin * slope.max(w.vp_per_coin) * turns * (5.0 / n);
                }
            }
        }
        v
    }
}

impl Evaluator for GainListEvaluator<'_> {
    fn begin_root(&self, root: &GameState, me: u8) {
        *self.root.borrow_mut() = Some(RootCache::new(root, me, self.strategy));
    }

    fn end_root(&self) {
        *self.root.borrow_mut() = None;
    }

    fn leaf_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        let view = PlayerView::new(root, me);
        let cached = self.root.borrow();
        let local;
        let cache = match &*cached {
            Some(c) if c.me == me => c,
            _ => {
                local = RootCache::new(root, me, self.strategy);
                &local
            }
        };
        let before = &cache.before;
        let mut after = leaf.players[me as usize].all_cards();
        // A Traveller exchanged for its upgrade is neither lost nor gained (the pile is not the Supply's): count the pair as unchanged, and
        // prefer the exchange a little, so that playing a Traveller to upgrade it is not a loss.
        let mut exchanges = 0u32;
        if leaf.trav != root.trav {
            for c in [id::PAGE, id::TREASURE_HUNTER, id::WARRIOR, id::HERO, id::PEASANT, id::SOLDIER, id::FUGITIVE, id::DISCIPLE] {
                let n = dominion_engine::cards::traveller_upgrade(c).expect("a Traveller");
                let ex = after.get(n).saturating_sub(before.get(n)).min(before.get(c).saturating_sub(after.get(c)));
                if ex > 0 {
                    after.add(c, ex);
                    after.set(n, after.get(n) - ex);
                    exchanges += ex as u32;
                }
            }
        }
        let mut v = exchanges as f64 * 0.5;
        // Only cards owned before or after, or used up, can have changed.
        let mut touched = after;
        touched.max_assign(before);
        for (c, _) in leaf.turn.used_up.iter() {
            touched.set(c, touched.get(c).max(1));
        }
        for (c, _) in touched.iter() {
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
                    let mut owned = root.dup();
                    owned.players[me as usize].discard.add(c, k as u8);
                    owned.supply.set(c, owned.supply.get(c).saturating_sub(k as u8));
                    self.weight(&PlayerView::new(&owned, me), c)
                };
            }
        }
        // Curses handed to the other players this turn (Kitsune's option, a chosen attack): each is
        // worth one and a half Silvers in the gain list, more than a Silver for me and less than
        // the next listed card. Witch-style attacks give theirs anyway; this decides the optional ones.
        if leaf.turn.player == me {
            let mut given = 0i32;
            for o in 0..leaf.num_players {
                if o != me {
                    given += leaf.players[o as usize].count_of(id::CURSE) as i32 - cache.curses[o as usize] as i32;
                }
            }
            if given > 0 {
                v += given as f64 * 1.5 * self.weight(&view, id::SILVER).max(1000.0);
            }
        }
        // Events bought this turn: each is worth its place in the gain list, like a card gained (its effects are counted above as gains and
        // trashes, or are later turns' and have no other value here).
        if leaf.turn.number == root.turn.number {
            let bought = leaf.turn.adv.bought & !root.turn.adv.bought;
            for i in 0..dominion_engine::cards::NUM_EVENTS as CardId {
                if bought & (1 << i) != 0 {
                    v += self.weight(&view, dominion_engine::cards::FIRST_EVENT + i);
                }
            }
        }
        // Strictly the strategy's own priorities. The general evaluator's game-end term is removed
        // from the tie-break: winning only matters if the rules say so. With `win_this_turn` (the
        // default) a turn that wins the game outright, acquiring only listed cards, outranks every
        // list priority, matching what the bot itself plays.
        let tie = self.tie_break.leaf_value(root, leaf, me) - game_end_value(leaf, me);
        // A turn that wins the game outright outranks everything, whatever it gains.
        let win = if self.strategy.win_this_turn && leaf.result_if_turn_ends() == Some(1 << me) { 1e15 } else { 0.0 };
        win + v + self.preferences(root, leaf, me, before, &after, &cache.tiers) + 1e-6 * tie
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
            DecisionKind::PlayNight => self.strategy.rule_night(&view, choices),
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
            if matches!(decision.kind, DecisionKind::PlayNight) && decision.player == me && self.strategy.search_play {
                return crate::strategy::night_obvious(&view, choices).map(Choice::Card);
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
            // An Event offered before the Treasures (Alms): only when the strategy's rules would buy it.
            (DecisionKind::PlayTreasure, Choice::Card(e)) if dominion_engine::cards::is_event(e) => {
                let view = PlayerView::new(state, me);
                self.strategy.win_check_applies(&view) || self.strategy.allows_buy(&view, e)
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
            DecisionKind::PlayNight => self.strategy.rule_night(&view, choices),
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
            | DecisionKind::PlayNight
            | DecisionKind::PickBoon
            | DecisionKind::PlayTreasure
            | DecisionKind::Select { act: dominion_engine::Act::Play, .. }
            | DecisionKind::Select { act: dominion_engine::Act::Reveal, .. }
    ) || (matches!(d.kind, DecisionKind::Mode { .. }) && d.player == turn_player)
        || is_treasure_effect_decision(turn_player, d)
        || is_overpay_decision(turn_player, d)
        || is_start_effect_decision(turn_player, d)
        || is_call_decision(turn_player, d)
}

/// The turn player's call of a Reserve card whose use shapes the turn: Duplicate on a card just gained, Royal Carriage after an Action, Coin
/// of the Realm, and the discarding of a Wine Merchant from the mat at the end of the Buy phase. Searched like the play order (a stated
/// `[[call]]` rule wins). The calls at the start of the turn (Guide, Ratcatcher, Transmogrify, Teacher) follow the rules.
pub fn is_call_decision(turn_player: u8, d: &Decision) -> bool {
    use dominion_engine::cards::id;
    d.player == turn_player
        && match d.kind {
            // Royal Carriage's call follows its rule (replay an Action that draws, pays or attacks): searching each call was 27x slower in a
            // deck of Royal Carriages and Bridges and lost to the rule (51% against 88% for the same strategy facing Samurai/Kintsugi).
            DecisionKind::YesNo { act: dominion_engine::Act::Call } => matches!(d.subject, id::DUPLICATE | id::COIN_OF_THE_REALM),
            DecisionKind::YesNo { act: dominion_engine::Act::Discard } => d.subject == id::WINE_MERCHANT,
            _ => false,
        }
}

/// What one Sun token removed from the Prophecy is worth to a player the Prophecy helps as much as can be (and, negated, to one it hurts): on the
/// preference scale, a fraction of a stated `[[play]]` rule (5 or more), far below any gain-list difference (1000 or more).
const SUN_TOKEN: f64 = 0.4;

/// Sun tokens taken off the Prophecy this turn (an Omen played), valued by who the Prophecy helps: each token brings the Prophecy closer for everyone,
/// so it is a gain for `me` when the Prophecy (see [`prophecy_edge`]) helps `me` more than the opponents, and a loss when it helps them more. Zero in
/// a game without a Prophecy, and for any turn that removes no token.
fn sun_value(root: &GameState, leaf: &GameState, me: u8) -> f64 {
    let removed = root.rs.suns.saturating_sub(leaf.rs.suns);
    if removed == 0 || root.rs.prophecy == 0 {
        return 0.0;
    }
    removed as f64 * SUN_TOKEN * prophecy_edge(root.rs.prophecy, root, me)
}

/// How much the Prophecy, once it is in effect, favours `me` over the other players, from -1 to 1, judged from the cards each owns (all public). Simple
/// and coarse by design: the difference in the density of the card kind the Prophecy rewards (4 times the difference, so 25 points of density is
/// the whole range), signed by whether it rewards or punishes owning it:
///
/// | Prophecy | favours the player with more | note |
/// |---|---|---|
/// | Approaching Army (+$1 per Attack played) | Attacks | |
/// | Great Leader (+1 Action per Action played) | Actions | |
/// | Enlightenment, Kind Emperor (Actions to hand) | Actions | half weight |
/// | Good Harvest (+$1 +Buy for the first Treasure), Growth (a Silver per Treasure gained) | Treasures | |
/// | Panic (Treasures return to their piles at Clean-up) | fewer Treasures | |
/// | Biding Time (the hand carries over) | fewer dead cards, and Artificers (a bigger hand to discard from) | dead cards: Curses and Victory cards that do nothing |
///
/// The others (Bureaucracy, Divine Wind, Flourishing Trade, Harsh Winter, Progress, Rapid Expansion, Sickness) are valued 0: they apply to everyone
/// alike, or depend on play this model does not see.
pub fn prophecy_edge(prophecy: CardId, state: &GameState, me: u8) -> f64 {
    use dominion_engine::cards::{ATTACK, TREASURE};
    let n = state.num_players;
    let mine = state.players[me as usize].all_cards();
    // The average of the other players' cards (each player's density, then the mean).
    let density = |all: &dominion_engine::Counts, f: &dyn Fn(CardId) -> bool| -> f64 {
        let total = all.total();
        if total == 0 {
            return 0.0;
        }
        all.iter().filter(|&(c, _)| f(c)).map(|(_, k)| k as f64).sum::<f64>() / total as f64
    };
    let diff = |f: &dyn Fn(CardId) -> bool| -> f64 {
        let theirs: f64 = (0..n).filter(|&o| o != me).map(|o| density(&state.players[o as usize].all_cards(), f)).sum::<f64>() / (n.max(2) - 1) as f64;
        density(&mine, f) - theirs
    };
    let is_action = |c: CardId| dominion_engine::cards::is(c, dominion_engine::cards::ACTION);
    let e = match prophecy {
        id::APPROACHING_ARMY => 4.0 * diff(&|c| dominion_engine::cards::is(c, ATTACK)),
        id::GREAT_LEADER => 4.0 * diff(&is_action),
        id::ENLIGHTENMENT | id::KIND_EMPEROR => 2.0 * diff(&is_action),
        id::GOOD_HARVEST | id::GROWTH => 4.0 * diff(&|c| dominion_engine::cards::is(c, TREASURE)),
        id::PANIC => -4.0 * diff(&|c| dominion_engine::cards::is(c, TREASURE)),
        id::BIDING_TIME => {
            let dead = |c: CardId| c == id::CURSE || (dominion_engine::cards::is(c, dominion_engine::cards::VICTORY) && !is_action(c) && !dominion_engine::cards::is(c, TREASURE));
            let artificers = |c: CardId| c == id::ARTIFICER;
            -4.0 * diff(&dead) + 8.0 * diff(&artificers)
        }
        _ => 0.0,
    };
    e.clamp(-1.0, 1.0)
}

/// The coins a Reserve card is worth in a hand when it is played (and so what returning it to the deck cycle is worth).
pub fn reserve_play_coins(card: CardId) -> f64 {
    use dominion_engine::cards::id;
    match card {
        id::WINE_MERCHANT => 4.0,
        id::COIN_OF_THE_REALM => 1.5,
        id::ROYAL_CARRIAGE => 2.0,
        id::GUIDE | id::DUPLICATE => 1.5,
        id::RATCATCHER | id::TRANSMOGRIFY | id::TEACHER => 1.0,
        _ => 0.0,
    }
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

/// How much to overpay for the card just bought shapes the turn (Stonemason's gains, Herald's
/// topdecks, Infirmary's draws and trashes, Farrier's cards): searched like the play order.
pub fn is_overpay_decision(turn_player: u8, d: &Decision) -> bool {
    d.player == turn_player && matches!(d.kind, DecisionKind::Overpay { .. })
}

/// Bank is worth $1 per Treasure in play, and Horn of Plenty gains up to $1 per name in play, so
/// they are best played after every other Treasure: while another Treasure is still in `me`'s
/// hand, playing them now is never considered.
fn bank_too_early(state: &GameState, me: u8, c: CardId) -> bool {
    use dominion_engine::cards::id;
    let hand = &state.players[me as usize].hand;
    match c {
        id::BANK => hand.iter().any(|(x, _)| x != id::BANK && state.is_treasure(x)),
        // Horn of Plenty counts the names in play: after every other Treasure (Bank may follow it).
        id::HORN_OF_PLENTY => hand.iter().any(|(x, _)| x != id::HORN_OF_PLENTY && x != id::BANK && state.is_treasure(x)),
        // Fortune doubles the $ so far: after every other Treasure (Coffers are spent before it by the Treasure decision).
        id::FORTUNE => hand.iter().any(|(x, _)| x != id::FORTUNE && state.is_treasure(x)),
        _ => false,
    }
}

const PLAY_TT_BITS: u32 = 16;
/// Node budgets tried in turn until the play search is exact: fast for ordinary turns, and as
/// deep as the analysis (`SearchConfig::default()`) for big ones.
pub const PLAY_BUDGETS: [u64; 2] = [1_000, 200_000];

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
        let first = PLAY_BUDGETS[0].min(strategy.search_budget);
        for (i, budget) in [first, strategy.search_budget].into_iter().enumerate() {
            if i == 1 && budget <= first {
                break;
            }
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

/// Every option of the play decision `view` faces, with the value the strategy's scoring gives it (best first) and its best line as text:
/// what `search_play_choice` ranks, for diagnosing why the search prefers a move.
pub fn analyze_play(strategy: &Strategy, view: &PlayerView) -> Vec<(Choice, f64, String)> {
    let world = view.determinize(&mut Rng::new(view.stable_seed()));
    let eval = GainListEvaluator::new(strategy);
    let mut searcher = Searcher::new(PLAY_TT_BITS);
    let cfg = SearchConfig { node_budget: PLAY_BUDGETS[1], tt_bits: PLAY_TT_BITS, ..SearchConfig::default() };
    searcher.analyze(&world, view.me(), &cfg, &eval).options.into_iter().map(|o| (o.choice, o.ev, o.pv)).collect()
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

/// Node budgets for each candidate of `search_artificer_choice`: the rest of the turn after the discards is small.
const ARTIFICER_BUDGETS: [u64; 2] = [1_000, 30_000];

/// Artificer's discards, searched: each candidate discard set (`Strategy::artificer_candidates`: the cheapest cards of the hand, for each
/// number that a listed card costs exactly) is played out on the sampled world, and the rest of the turn (the gain onto the deck, the
/// remaining Actions, the Treasures and the buy) is searched with the strategy's own scoring: what the discards cost the Buy phase is weighed
/// against the card gained, on the gain list's scale. The best candidate wins, stopping first on ties. `None` when there is nothing to
/// weigh (the rule decides). Picks come in card order, so the answer is the lowest card of the best plan, or `Pass` to stop; it is chosen
/// afresh at each pick, from what is left of the hand, and reaches the same plan.
pub fn search_artificer_choice(strategy: &Strategy, view: &PlayerView, choices: &[Choice]) -> Option<Choice> {
    let cands = strategy.artificer_candidates(view);
    if cands.len <= 1 {
        return None;
    }
    let me = view.me();
    let world = view.determinize(&mut Rng::new(view.stable_seed()));
    let eval = GainListEvaluator::new(strategy);
    let mut best: Option<(f64, usize)> = None;
    PLAY_SEARCHER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let searcher = slot.get_or_insert_with(|| Searcher::new(PLAY_TT_BITS));
        for i in 0..cands.len {
            let mut w = world.dup();
            w.chance_mode = true;
            let mut ok = true;
            for (c, n) in cands.sets[i].iter() {
                for _ in 0..n {
                    ok &= matches!(w.advance(&mut dominion_engine::NoEvents), Step::Decision(_)) && w.apply(Choice::Card(c), &mut dominion_engine::NoEvents).is_ok();
                }
            }
            if !ok {
                continue;
            }
            let Step::Decision(mut d) = w.advance(&mut dominion_engine::NoEvents) else { continue };
            // The selection ends by itself once the whole hand is picked; otherwise stop picking.
            if matches!(d.kind, DecisionKind::Select { .. }) && d.source == Some(id::ARTIFICER) {
                if w.apply(Choice::Pass, &mut dominion_engine::NoEvents).is_err() {
                    continue;
                }
                match w.advance(&mut dominion_engine::NoEvents) {
                    Step::Decision(d2) => d = d2,
                    _ => continue,
                }
            }
            if d.player != me {
                continue;
            }
            let mut value = None;
            for budget in ARTIFICER_BUDGETS {
                let cfg = SearchConfig { node_budget: budget, tt_bits: PLAY_TT_BITS, ..SearchConfig::default() };
                let Some((_, v, exact)) = searcher.best_choice(&w, me, &cfg, &eval) else { break };
                value = Some(v);
                if exact {
                    break;
                }
            }
            if let Some(v) = value {
                if best.map_or(true, |(b, _)| v > b + 1e-9) {
                    best = Some((v, i));
                }
            }
        }
    });
    let (_, i) = best?;
    if i == 0 {
        return Some(Choice::Pass);
    }
    let pick = cands.sets[i].iter().map(|(c, _)| c).find(|&c| choices.contains(&Choice::Card(c)))?;
    Some(Choice::Card(pick))
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
