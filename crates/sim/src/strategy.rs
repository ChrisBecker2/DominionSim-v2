//! Human-readable scripted strategies.
//!
//! A strategy is a small TOML file: an ordered **gain priority list** (`[[gain]]`, each entry
//! optionally gated by a condition, see `expr.rs`), an ordered **action play priority list**, and
//! a few optional knobs for sub-decisions (Chapel/Sentry trashing, how much treasure to keep in hand).
//!
//! Which action to play is found by searching the turn with the strategy's own scoring: the gain
//! list first, then stated `[[play]]` rules (earlier rules worth more), then defaults (attacks,
//! wanted trashes, playing actions/treasures). The rule order is only a fallback.
//! `[[play]]` (action play order) and `[[trash]]` (what to trash when given the chance) are rule
//! lists like `[[gain]]`, each entry `card` + optional `if`. Stated rules always come first;
//! built-in defaults only apply below them (default trash: Curse, then Estate, then Copper;
//! treasures are never trashed from hand below `keep_treasure`), and a default for a card is
//! dropped when the strategy states its own rule for that card.
//!
//! The gain list drives every gain, strictly in order: buys, Workshop/Artisan gains, and
//! "trash a card, gain a better one" upgrades (Remodel, Mine): for an upgrade, the strategy trashes
//! whichever card unlocks the highest-ranked gain in the list (ties: trash the cheaper card).
//! `[[buy]]` is accepted as an older alias for `[[gain]]`.
//!
//! One lookahead rule applies to every strategy unless the file sets `win_this_turn = false`: when
//! the game is close to ending, if some line of play this turn wins for certain while acquiring
//! only gain-list cards, play it.
//!
//! Everything else (Cellar, Militia, Bureaucrat, Throne Room, Bandit, Library,
//! Harbinger, Vassal, Moneylender, Artisan, Sentry, Poacher...) is handled by sensible,
//! table-driven defaults keyed on the *shape* of the decision (which zone, which action) rather
//! than the specific card, per `dominion_engine::engine::DecisionKind::Select` /
//! `DecisionKind::YesNo`. See `Strategy::decide` below for the full policy.
//!
//! Parsing (TOML -> AST) happens once, when the strategy is loaded; `Strategy::decide` never
//! allocates, so a `Strategy` can be shared (by reference) across millions of simulated games.

use std::path::Path;

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, id, CardId, ACTION, NUM_CARDS, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::Counts;
use dominion_engine::state::{Act, Filter, Zone};
use serde::Deserialize;

use crate::expr::Expr;

// -------------------------------------------------------------------------------------------
// TOML schema
// -------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct BuyRuleRaw {
    card: String,
    #[serde(rename = "if")]
    cond: Option<String>,
}

fn default_true() -> bool {
    true
}

/// `play = ["Village", "Smithy"]` or `[[play]] card = "..." if = "..."`.
#[derive(Deserialize)]
#[serde(untagged)]
enum RulesOrNames {
    Names(Vec<String>),
    Rules(Vec<BuyRuleRaw>),
}

impl Default for RulesOrNames {
    fn default() -> Self {
        RulesOrNames::Names(Vec::new())
    }
}

impl RulesOrNames {
    fn into_rules(self) -> Vec<BuyRuleRaw> {
        match self {
            RulesOrNames::Names(n) => n.into_iter().map(|card| BuyRuleRaw { card, cond: None }).collect(),
            RulesOrNames::Rules(r) => r,
        }
    }
}

/// Built-in trash rules, used below the strategy's own `[[trash]]` rules (skipping any card the
/// strategy lists itself): Curse, then Estate, then Copper (treasures never below
/// `keep_treasure` in hand).
const DEFAULT_TRASH: &[(&str, Option<&str>)] = &[("Curse", None), ("Estate", None), ("Copper", None)];

fn compile_rules(raw: &[BuyRuleRaw], what: &str) -> Result<Vec<(CardId, Option<Expr>)>, String> {
    raw.iter()
        .map(|r| {
            let card = cards::by_name(&r.card).ok_or_else(|| format!("unknown card {:?} in {what} list", r.card))?;
            let cond = r.cond.as_deref().map(Expr::parse).transpose()?;
            Ok((card, cond))
        })
        .collect()
}

#[derive(Deserialize, Default)]
struct StrategyFile {
    name: String,
    #[serde(default)]
    description: String,
    /// Gain priority list (used for buys and every other gain).
    #[serde(default)]
    gain: Vec<BuyRuleRaw>,
    /// Older name for `gain`.
    #[serde(default)]
    buy: Vec<BuyRuleRaw>,
    /// Action play priority (`[[play]]` rules, or the shorthand `play = ["Witch", ...]`).
    #[serde(default)]
    play: RulesOrNames,
    /// Trash priority for optional trashing (Chapel, Sentry, Moneylender...) and forced trashing.
    #[serde(default)]
    trash: Vec<BuyRuleRaw>,
    /// Older shorthand for `[[trash]]` without conditions.
    #[serde(default)]
    trash_priority: Option<Vec<String>>,
    /// Chapel/Sentry won't trash a treasure from hand if doing so would leave fewer than this
    /// many treasures in hand. Default 2.
    #[serde(default)]
    keep_treasure: Option<u8>,
    /// Before following the lists, look for a line of play that wins the game this turn for
    /// certain, acquiring only cards in the gain list; play it if found. Default true.
    #[serde(default = "default_true")]
    win_this_turn: bool,
}

// -------------------------------------------------------------------------------------------
// Compiled strategy
// -------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct Strategy {
    pub name: String,
    pub description: String,
    buy: Vec<(CardId, Option<Expr>)>,
    /// `[[play]]` rules in priority order.
    play: Vec<(CardId, Option<Expr>)>,
    /// Rank of each card's first `[[play]]` rule (lower = earlier), or `-1` if unlisted.
    play_rank: [i16; NUM_CARDS],
    /// `[[trash]]` rules followed by the defaults (`DEFAULT_TRASH`).
    trash: Vec<(CardId, Option<Expr>)>,
    keep_treasure: u8,
    pub win_this_turn: bool,
}

impl Strategy {
    pub fn load(path: &Path) -> Result<Strategy, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        Strategy::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(toml_src: &str) -> Result<Strategy, String> {
        let raw: StrategyFile = toml::from_str(toml_src).map_err(|e| format!("TOML error: {e}"))?;
        if raw.name.trim().is_empty() {
            return Err("strategy is missing a `name`".to_string());
        }

        if !raw.gain.is_empty() && !raw.buy.is_empty() {
            return Err("use either [[gain]] or [[buy]] (an alias), not both".to_string());
        }
        let rules = if raw.gain.is_empty() { &raw.buy } else { &raw.gain };
        let mut buy = Vec::with_capacity(rules.len());
        for rule in rules {
            let card = cards::by_name(&rule.card).ok_or_else(|| format!("unknown card {:?} in gain list", rule.card))?;
            let cond = rule.cond.as_deref().map(Expr::parse).transpose()?;
            buy.push((card, cond));
        }

        let play = compile_rules(&raw.play.into_rules(), "play")?;
        let mut play_rank = [-1i16; NUM_CARDS];
        for (i, (card, _)) in play.iter().enumerate() {
            if !cards::is(*card, ACTION) {
                return Err(format!("{:?} in play list is not an Action card", cards::name(*card)));
            }
            if play_rank[*card as usize] < 0 {
                play_rank[*card as usize] = i as i16;
            }
        }

        if !raw.trash.is_empty() && raw.trash_priority.is_some() {
            return Err("use either [[trash]] or trash_priority (older shorthand), not both".to_string());
        }
        let stated: Vec<BuyRuleRaw> = match raw.trash_priority {
            Some(names) => names.into_iter().map(|card| BuyRuleRaw { card, cond: None }).collect(),
            None => raw.trash,
        };
        let mut trash = compile_rules(&stated, "trash")?;
        for (name, cond) in DEFAULT_TRASH {
            let card = cards::by_name(name).expect("default trash card");
            if !trash.iter().any(|(c, _)| *c == card) {
                trash.push((card, cond.map(|c| Expr::parse(c).expect("default trash condition"))));
            }
        }

        Ok(Strategy {
            name: raw.name,
            description: raw.description,
            buy,
            play,
            play_rank,
            trash,
            keep_treasure: raw.keep_treasure.unwrap_or(2),
            win_this_turn: raw.win_this_turn,
        })
    }

    /// Every kingdom card (id >= `FIRST_KINGDOM`) this strategy names, in its buy list, play
    /// list, or buy conditions. Used to build an "auto" kingdom selection. Allocates (fine: not
    /// called during simulation).
    pub fn kingdom_refs(&self) -> Vec<CardId> {
        let mut out = Vec::new();
        let mut push = |c: CardId| {
            if c >= cards::FIRST_KINGDOM && !out.contains(&c) {
                out.push(c);
            }
        };
        for (card, cond) in &self.buy {
            push(*card);
            if let Some(e) = cond {
                e.for_each_card_ref(&mut push);
            }
        }
        for (card, cond) in self.play.iter().chain(self.trash.iter()) {
            push(*card);
            if let Some(e) = cond {
                e.for_each_card_ref(&mut push);
            }
        }
        out
    }

    /// Which of `choices` this strategy picks for `decision`. Allocation-free.
    pub fn decide(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        if self.win_this_turn && view.is_my_turn() && decision.player == view.me() && Self::game_could_end_soon(view) {
            if let Some(c) = crate::eval::certain_win_choice(self, view, choices) {
                return c;
            }
        }
        self.decide_by_rules(view, decision, choices)
    }

    /// Whether `win_this_turn` could apply here (a search inside the strategy's own scoring then
    /// maximizes such decisions instead of following the rules, finding the same wins).
    pub fn win_check_applies(&self, view: &PlayerView) -> bool {
        self.win_this_turn && Self::game_could_end_soon(view)
    }

    /// `decide` without the `win_this_turn` lookahead: the rules only.
    pub fn decide_by_rules(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        match decision.kind {
            DecisionKind::PlayAction => self.play_decision(view, decision, choices, false),
            DecisionKind::Buy => self.match_gain_list(view, choices).map(Choice::Card).unwrap_or(Choice::Pass),
            DecisionKind::Gain { .. } => self.choose_gain(view, decision, choices),
            DecisionKind::Select { from, act, filter, min, max, ordered } => {
                match act {
                    Act::Discard => self.choose_discard(view, from, filter, choices, min, max, ordered),
                    Act::Trash => self.choose_trash(view, decision, choices, from, filter, min, max, ordered),
                    Act::Topdeck => self.choose_topdeck(choices, from, filter, ordered),
                    Act::Play => self.play_decision(view, decision, choices, decision.play_times > 1),
                    Act::SetAside => Choice::Pass, // not used via Select in the base set
                }
            }
            DecisionKind::YesNo { act } => match act {
                // A free extra play is (almost) always worth taking (Vassal).
                Act::Play => Choice::Yes,
                // Library: skip (set aside) an Action only when there's no room left to play
                // more actions this turn; otherwise keep it.
                Act::SetAside => {
                    if view.turn().actions == 0 { Choice::Yes } else { Choice::No }
                }
                _ => Choice::No,
            },
        }
    }

    // -----------------------------------------------------------------------------------------
    // Buy / Gain: shared "walk the priority list" logic.
    // -----------------------------------------------------------------------------------------


    /// Position of `card` in the gain list (first entry for it whose condition holds), or None.
    pub fn gain_rank(&self, view: &PlayerView, card: CardId) -> Option<usize> {
        self.buy.iter().position(|(c, cond)| *c == card && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)))
    }

    /// Whether `card` appears anywhere in the gain list (regardless of conditions).
    pub fn lists(&self, card: CardId) -> bool {
        self.buy.iter().any(|(c, _)| *c == card)
    }

    /// Number of entries in the gain list.
    pub fn gain_list_len(&self) -> usize {
        self.buy.len()
    }

    /// Rank (index in the gain list) of the best entry that could be gained right now for at
    /// most `max_cost`, matching `filter`, with its pile non-empty and its condition true.
    fn best_gain_rank(&self, view: &PlayerView, max_cost: u8, filter: Filter) -> Option<usize> {
        self.buy.iter().position(|(card, cond)| {
            cards::cost(*card) <= max_cost
                && filter.matches(*card)
                && view.in_supply(*card)
                && view.supply(*card) > 0
                && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card))
        })
    }

    /// Cheap gate for `win_this_turn`: only search when the game could end with at most
    /// `MAX_ENDING_GAINS` more gains: the Province pile, or the smallest piles needed to reach
    /// the empty-pile limit. Allocation-free.
    fn game_could_end_soon(view: &PlayerView) -> bool {
        const MAX_ENDING_GAINS: u32 = 3;
        if (view.supply(id::PROVINCE) as u32) <= MAX_ENDING_GAINS {
            return true;
        }
        let pile_limit: u32 = if view.num_players() >= 5 { 4 } else { 3 };
        let need = pile_limit.saturating_sub(view.empty_piles()).min(4);
        // Sum of the `need` smallest non-empty piles, via a small fixed-size selection.
        let mut smallest = [u32::MAX; 4];
        for c in 0..NUM_CARDS as CardId {
            if view.in_supply(c) && view.supply(c) > 0 {
                let n = view.supply(c) as u32;
                let mut i = smallest.len();
                while i > 0 && smallest[i - 1] > n {
                    i -= 1;
                }
                if i < smallest.len() {
                    smallest.copy_within(i..3, i + 1);
                    smallest[i] = n;
                }
            }
        }
        let total: u32 = smallest[..need as usize].iter().fold(0u32, |a, &b| a.saturating_add(b));
        total <= MAX_ENDING_GAINS
    }

    /// First entry in the gain list that's a legal choice and whose condition holds.
    fn match_gain_list(&self, view: &PlayerView, choices: &[Choice]) -> Option<CardId> {
        for (card, cond) in &self.buy {
            if has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)) {
                return Some(*card);
            }
        }
        None
    }

    /// Whether the strategy's rules would ever buy `card` here: it's in the gain list with its
    /// condition true.
    pub fn allows_buy(&self, view: &PlayerView, card: CardId) -> bool {
        self.gain_rank(view, card).is_some()
    }

    fn choose_gain(&self, view: &PlayerView, _decision: &Decision, choices: &[Choice]) -> Choice {
        if let Some(c) = self.match_gain_list(view, choices) {
            return Choice::Card(c);
        }
        // Nothing in the buy list is affordable/legal here (e.g. a small Workshop/Remodel gain
        // below everything we listed). Never gain Curse if there's an alternative, and avoid
        // dead Victory cards unless the game is ending; otherwise take the most expensive legal
        // option, since a bigger card is rarely a mistake to gain.
        let endgame = view.supply(id::PROVINCE) <= 3 || view.empty_piles() >= 2;
        let key = |c: CardId| (c != id::CURSE, endgame || !cards::is(c, cards::VICTORY), cards::cost(c));
        let mut best: Option<CardId> = None;
        for c in iter_cards(choices) {
            let better = match best {
                None => true,
                Some(b) => key(c) > key(b),
            };
            if better {
                best = Some(c);
            }
        }
        best.map(Choice::Card).unwrap_or(Choice::Pass)
    }

    // -----------------------------------------------------------------------------------------
    // PlayAction / Throne Room target: shared priority-list-then-heuristic ranking.
    // -----------------------------------------------------------------------------------------

    /// Rank of `card` for the action-play order: explicit `play` list entries win (in listed
    /// order); everything else falls back to a heuristic that plays non-terminal / +actions
    /// cards first, then draw, then attacks, then plain terminals (lower = played sooner).
    fn play_rank_of(&self, card: CardId) -> i32 {
        let listed = self.play_rank[card as usize];
        if listed >= 0 {
            return listed as i32;
        }
        const UNLISTED: i32 = 10_000;
        let d = cards::def(card);
        let terminal = d.actions == 0;
        // Non-terminals (+Actions) first, then cards that play other actions (Throne Room), so
        // they multiply a terminal rather than a village, then plain terminals.
        let mut score = if d.plays > 0 { 250 } else if terminal { 500 } else { 0 };
        score -= d.actions as i32 * 20; // extra +actions: safely play early
        score -= d.cards as i32 * 5; // draw cards before non-draw cards
        score -= d.buys as i32 * 2;
        score -= d.coins as i32;
        if cards::is(card, cards::ATTACK) {
            score -= 50; // attack opponents before drawing more of our own hand
        }
        UNLISTED + score
    }

    /// Best action to play. Cards that play other actions (Throne Room) are only worth playing
    /// when the hand holds an ordinary action for them to multiply; when choosing what a
    /// multiplier plays (`for_multiplier`), another multiplier is picked only if at least two
    /// ordinary actions remain to use its extra plays on.
    fn best_play(&self, view: &PlayerView, choices: &[Choice], for_multiplier: bool) -> Option<CardId> {
        let ordinary_in_hand: u32 =
            view.hand().iter().filter(|&(c, _)| cards::is(c, ACTION) && cards::def(c).plays == 0).map(|(_, n)| n as u32).sum();
        let multiplier_ok = if for_multiplier { ordinary_in_hand >= 2 } else { ordinary_in_hand >= 1 };
        // Stated [[play]] rules first, in order, when their condition holds; then the default order.
        for (card, cond) in &self.play {
            if has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)) {
                return Some(*card);
            }
        }
        iter_cards(choices)
            .filter(|&c| cards::def(c).plays == 0 || multiplier_ok)
            .min_by_key(|&c| (self.play_rank_of(c), c))
    }

    /// Which action to play (or what a Throne Room plays). With a real choice, the bot searches
    /// its own turn with its own scoring (gain list > stated [[play]] rules > defaults) and plays
    /// the best line; a single playable card is simply played. The rule order in `best_play` is
    /// only a fallback if the search can't run.
    fn play_decision(&self, view: &PlayerView, decision: &Decision, choices: &[Choice], for_multiplier: bool) -> Choice {
        if iter_cards(choices).count() >= 2 && decision.player == view.me() {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        self.best_play(view, choices, for_multiplier).map(Choice::Card).unwrap_or(Choice::Pass)
    }

    /// The play choice by rule order alone (stated [[play]] rules, then the default order), without
    /// searching. Used in search playouts.
    pub fn rule_play(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        self.best_play(view, choices, decision.play_times > 1).map(Choice::Card).unwrap_or(Choice::Pass)
    }

    /// Whether the strategy's `[[play]]` rules name `card`.
    pub fn states_play(&self, card: CardId) -> bool {
        self.play.iter().any(|(c, _)| *c == card)
    }

    /// For a card played this turn: how many stated [[play]] rules rank below its first rule
    /// whose condition holds (`None` if no stated rule applies). Used for scoring.
    pub fn play_rules_below(&self, view: &PlayerView, card: CardId) -> Option<usize> {
        let n = self.play.len();
        self.play
            .iter()
            .position(|(c, cond)| *c == card && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)))
            .map(|i| n - 1 - i)
    }

    /// Whether the trash rules (stated, then defaults) want `card` gone right now.
    pub fn wants_trash(&self, view: &PlayerView, card: CardId) -> bool {
        self.trash.iter().any(|(c, cond)| *c == card && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)))
    }

    // -----------------------------------------------------------------------------------------
    // Discard (Cellar / Militia / Poacher victim): worst card first.
    // -----------------------------------------------------------------------------------------

    /// Lower = more worth discarding. Curse and Victory cards are dead weight in hand during
    /// the action phase; beyond that, discard the cheapest card first.
    fn discard_rank(card: CardId) -> i32 {
        if card == id::CURSE {
            -1000
        } else if cards::is(card, VICTORY) {
            -500
        } else {
            cards::cost(card) as i32
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn choose_discard(&self, view: &PlayerView, from: Zone, filter: Filter, choices: &[Choice], min: u8, max: u8, ordered: bool) -> Choice {
        // Optional discards: only dead cards (Curse/Victory). Forced: worst first up to `min`.
        let zone = zone_of(view, from, filter, choices);
        let mut wanted = Counts::EMPTY;
        let mut left = if min > 0 { min as u32 } else { max as u32 };
        let mut order: [CardId; NUM_CARDS] = [0; NUM_CARDS];
        for (i, o) in order.iter_mut().enumerate() {
            *o = i as CardId;
        }
        order.sort_by_key(|&c| (Strategy::discard_rank(c), c));
        for c in order {
            if left == 0 {
                break;
            }
            if min == 0 && Strategy::discard_rank(c) >= 0 {
                break;
            }
            let take = (zone.get(c) as u32).min(left);
            wanted.add(c, take as u8);
            left -= take;
        }
        pick_wanted(&wanted, choices, ordered, min > 0)
    }

    // -----------------------------------------------------------------------------------------
    // Trash (Chapel / Sentry / Remodel / Mine / Moneylender / Bandit victim): junk first,
    // subject to a money floor in hand; special-cased upgrades for Remodel and Mine.
    // -----------------------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn choose_trash(&self, view: &PlayerView, decision: &Decision, choices: &[Choice], from: Zone, filter: Filter, min: u8, max: u8, ordered: bool) -> Choice {
        let forced = min > 0;
        // Upgrades (Remodel, Mine, ...): trash whichever card unlocks the best gain in the gain
        // list, strictly by list order; ties trash the cheaper card.
        if let Some(up) = decision.upgrade {
            let mut best: Option<(usize, u8, CardId)> = None;
            for c in iter_cards(choices) {
                if let Some(rank) = self.best_gain_rank(view, cards::cost(c) + up.plus, up.filter) {
                    let key = (rank, cards::cost(c), c);
                    if best.map_or(true, |b| key < b) {
                        best = Some(key);
                    }
                }
            }
            if let Some((_, _, c)) = best {
                return Choice::Card(c);
            }
            if !forced {
                return Choice::Pass;
            }
            // Forced and nothing unlocks a listed gain: fall through to junk-first.
        }

        // The whole set the trash rules want (stated rules first, then defaults), up to `max`,
        // never dipping below `keep_treasure` treasures in hand unless forced; then fill a forced
        // minimum with the cheapest cards.
        let zone = zone_of(view, from, filter, choices);
        let mut wanted = Counts::EMPTY;
        let mut left = max as u32;
        let mut treasure_budget = if from == Zone::Hand && !forced {
            view.hand().count_type(TREASURE).saturating_sub(self.keep_treasure as u32)
        } else {
            u32::MAX
        };
        for (jc, cond) in &self.trash {
            let jc = *jc;
            if left == 0 {
                break;
            }
            if !cond.as_ref().map_or(true, |e| e.eval_bool_for(view, jc)) {
                continue;
            }
            let mut take = ((zone.get(jc) - wanted.get(jc)) as u32).min(left);
            if cards::is(jc, TREASURE) {
                take = take.min(treasure_budget);
                treasure_budget -= take;
            }
            wanted.add(jc, take as u8);
            left -= take;
        }
        let mut need = (min as u32).saturating_sub(wanted.total());
        if need > 0 {
            let mut rest: [CardId; NUM_CARDS] = [0; NUM_CARDS];
            for (i, r) in rest.iter_mut().enumerate() {
                *r = i as CardId;
            }
            rest.sort_by_key(|&c| (cards::cost(c), c));
            for c in rest {
                if need == 0 {
                    break;
                }
                let take = ((zone.get(c) - wanted.get(c)) as u32).min(need);
                wanted.add(c, take as u8);
                need -= take;
            }
        }
        pick_wanted(&wanted, choices, ordered, forced)
    }

    // -----------------------------------------------------------------------------------------
    // Topdeck (Harbinger / Bureaucrat victim / Artisan / Sentry order).
    // -----------------------------------------------------------------------------------------

    fn choose_topdeck(&self, choices: &[Choice], from: Zone, filter: Filter, ordered: bool) -> Choice {
        if ordered {
            // Sentry: put cards back one at a time; the LAST pick ends up on top, so place the
            // worse remaining card first and save the better one for last.
            return iter_cards(choices).min_by_key(|&c| Self::card_value(c)).map(Choice::Card).unwrap_or(Choice::Pass);
        }
        if filter == Filter::Victory {
            // Bureaucrat victim: minimize the damage, topdeck the cheapest Victory card.
            return cheapest_choice(choices);
        }
        if from == Zone::Discard {
            // Harbinger: bring back the best card per our own buy priorities, if any is worth it.
            return self.best_by_buy_rank(choices).map(Choice::Card).unwrap_or(Choice::Pass);
        }
        // Artisan: keep a good non-terminal action for next turn if we have one, else shed the
        // cheapest (usually junk) card.
        if let Some(c) = iter_cards(choices).filter(|&c| cards::is(c, ACTION)).min_by_key(|&c| self.play_rank_of(c)) {
            return Choice::Card(c);
        }
        cheapest_choice(choices)
    }

    fn buy_rank_of(&self, card: CardId) -> Option<usize> {
        self.buy.iter().position(|(c, _)| *c == card)
    }

    fn best_by_buy_rank(&self, choices: &[Choice]) -> Option<CardId> {
        iter_cards(choices).filter_map(|c| self.buy_rank_of(c).map(|r| (r, c))).min_by_key(|&(r, c)| (r, c)).map(|(_, c)| c)
    }

    /// Rough "how good is this card to have on top of the deck" score, higher is better.
    fn card_value(card: CardId) -> i32 {
        if card == id::CURSE {
            return -100;
        }
        let cost = cards::cost(card) as i32;
        if cards::is(card, VICTORY) {
            cost - 10
        } else if cards::is(card, TREASURE) {
            cost * 2
        } else {
            cost * 2 + 5 // actions
        }
    }
}

fn iter_cards(choices: &[Choice]) -> impl Iterator<Item = CardId> + '_ {
    choices.iter().filter_map(|c| if let Choice::Card(x) = c { Some(*x) } else { None })
}

fn has_card(choices: &[Choice], c: CardId) -> bool {
    choices.contains(&Choice::Card(c))
}

/// The cards eligible for a selection: its zone's cards that match the selection's filter (and at
/// least one of each offered card).
fn zone_of(view: &PlayerView, from: Zone, filter: Filter, choices: &[Choice]) -> Counts {
    let mut z = match from {
        Zone::Hand => *view.hand(),
        Zone::Discard => *view.discard(),
        Zone::Revealed => *view.revealed(),
    };
    for c in 0..NUM_CARDS as CardId {
        if !filter.matches(c) {
            z.set(c, 0);
        }
    }
    z.max_with(|c| u8::from(has_card(choices, c)))
}

/// Multi-card selections offer picks in non-decreasing card order (so each set of cards has one
/// path). Decide the whole `wanted` set first, then take its lowest offered card; `Pass` once
/// nothing wanted is offered (or the cheapest offered card if a pick is still required).
fn pick_wanted(wanted: &Counts, choices: &[Choice], ordered: bool, forced: bool) -> Choice {
    let pick = if ordered {
        iter_cards(choices).find(|&c| wanted.has(c))
    } else {
        (0..NUM_CARDS as CardId).find(|&c| wanted.has(c) && has_card(choices, c))
    };
    match pick {
        Some(c) => Choice::Card(c),
        None if forced => cheapest_choice(choices),
        None => {
            if choices.contains(&Choice::Pass) {
                Choice::Pass
            } else {
                cheapest_choice(choices)
            }
        }
    }
}

fn cheapest_choice(choices: &[Choice]) -> Choice {
    iter_cards(choices).min_by_key(|&c| (cards::cost(c), c)).map(Choice::Card).unwrap_or(Choice::Pass)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_strategy() {
        let s = Strategy::parse(
            r#"
            name = "Test"
            play = ["Village", "Smithy"]
            [[buy]]
            card = "Province"
            if = "coins >= 8"
            [[buy]]
            card = "Silver"
            "#,
        )
        .unwrap();
        assert_eq!(s.name, "Test");
        assert_eq!(s.buy.len(), 2);
        assert_eq!(s.play_rank[id::VILLAGE as usize], 0);
        assert_eq!(s.play_rank[id::SMITHY as usize], 1);
        // With no stated trash rules, the defaults: Curse, Estate, Copper.
        assert_eq!(s.trash.iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![id::CURSE, id::ESTATE, id::COPPER]);
    }

    #[test]
    fn play_and_trash_rules_with_conditions() {
        let s = Strategy::parse(
            r#"
            name = "Rules"
            [[play]]
            card = "Witch"
            [[play]]
            card = "Smithy"
            if = "actions >= 1"
            [[trash]]
            card = "Estate"
            if = "provinces_left > 6"
            [[gain]]
            card = "Silver"
            "#,
        )
        .unwrap();
        assert_eq!(s.play.len(), 2);
        // Stated Estate rule first (replacing the default Estate rule), then Curse and Copper defaults.
        assert_eq!(s.trash.iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![id::ESTATE, id::CURSE, id::COPPER]);
    }

    #[test]
    fn rejects_unknown_card() {
        let err = Strategy::parse("name = \"Bad\"\n[[buy]]\ncard = \"Not A Card\"\n").unwrap_err();
        assert!(err.contains("unknown card"), "{err}");
    }
}
