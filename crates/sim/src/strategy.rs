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
use dominion_engine::cards::{self, id, CardId, ModeOpt, ACTION, CURSE_T, NUM_CARDS, TREASURE, VICTORY};
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

/// `[[mode]] card = "Steward" choose = "+2 Cards" if = "..."`.
#[derive(Deserialize)]
struct ModeRuleRaw {
    card: String,
    choose: String,
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

/// Case/space-insensitive normalization for matching a `[[mode]] choose = "..."` string against
/// `ModeOpt::label()`.
fn norm_label(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect()
}

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
    /// Mode ("choose one/many") rules, in priority order.
    #[serde(default)]
    mode: Vec<ModeRuleRaw>,
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
    /// Cards never gained, even when a forced gain (Workshop, Remodel...) has nothing listed.
    #[serde(default)]
    never_gain: Vec<String>,
    /// Choose which action to play by searching the turn (true, default) or by rule order alone
    /// (false: much faster; used when evaluating many candidate strategies).
    #[serde(default = "default_true")]
    search_play: bool,
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
    /// `[[mode]]` rules in priority order: (card, index into `cards::modes(card)`, condition).
    mode: Vec<(CardId, u8, Option<Expr>)>,
    keep_treasure: u8,
    pub win_this_turn: bool,
    never_gain: Vec<CardId>,
    pub search_play: bool,
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

        let mut mode = Vec::with_capacity(raw.mode.len());
        for rule in &raw.mode {
            let card = cards::by_name(&rule.card).ok_or_else(|| format!("unknown card {:?} in mode list", rule.card))?;
            let table = cards::modes(card);
            if table.is_empty() {
                return Err(format!("{:?} in mode list has no mode choices", rule.card));
            }
            let want = norm_label(&rule.choose);
            let idx = table.iter().position(|o| norm_label(&o.label()) == want).ok_or_else(|| {
                let valid: Vec<String> = table.iter().map(|o| o.label()).collect();
                format!("unknown mode choice {:?} for {:?} (valid: {})", rule.choose, rule.card, valid.join(", "))
            })?;
            let cond = rule.cond.as_deref().map(Expr::parse).transpose()?;
            mode.push((card, idx as u8, cond));
        }

        Ok(Strategy {
            name: raw.name,
            description: raw.description,
            buy,
            play,
            play_rank,
            trash,
            mode,
            keep_treasure: raw.keep_treasure.unwrap_or(2),
            win_this_turn: raw.win_this_turn,
            never_gain: raw
                .never_gain
                .iter()
                .map(|n| cards::by_name(n).ok_or_else(|| format!("unknown card {n:?} in never_gain")))
                .collect::<Result<_, _>>()?,
            search_play: raw.search_play,
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
            DecisionKind::Gain { .. } if decision.for_player != decision.player => {
                // Swindler: this gain is forced on `for_player` (the victim), not chosen for
                // ourselves. Give them the worst option offered (all cost the same, by rule).
                self.worst_gain_for_victim(choices)
            }
            DecisionKind::Gain { .. } => self.choose_gain(view, decision, choices),
            DecisionKind::Select { from, act, filter, min, max, ordered } => {
                match act {
                    Act::Discard => self.choose_discard(view, from, filter, choices, min, max, ordered),
                    Act::Trash => self.choose_trash(view, decision, choices, from, filter, min, max, ordered),
                    Act::Topdeck => self.choose_topdeck(choices, from, filter, ordered),
                    Act::Play => self.play_decision(view, decision, choices, decision.play_times > 1),
                    Act::SetAside => self.choose_setaside(choices),
                    Act::Gain => self.choose_gain_from_zone(view, choices),
                    Act::Reveal => self.choose_reveal(choices),
                    Act::Pass => self.choose_pass(view, choices),
                }
            }
            DecisionKind::Mode { .. } => self.choose_mode(view, decision, choices),
            DecisionKind::YesNo { act } => match act {
                // A free extra play is (almost) always worth taking (Vassal).
                Act::Play => Choice::Yes,
                // Library: skip (set aside) an Action only when there's no room left to play
                // more actions this turn; otherwise keep it.
                Act::SetAside => {
                    if view.turn().actions == 0 { Choice::Yes } else { Choice::No }
                }
                // Trash-in-place-for-a-bonus (Mining Village): only when the trash rules name
                // this specific card, same as any other optional trash.
                Act::Trash => {
                    if self.wants_trash(view, decision.subject) { Choice::Yes } else { Choice::No }
                }
                // Discard-for-a-bonus (Baron: discard an Estate for +$4): worth it by default.
                Act::Discard => Choice::Yes,
                // Diplomat's reaction: only offered once the hand already has 5+ cards.
                Act::Reveal => Choice::Yes,
                _ => Choice::No,
            },
            DecisionKind::Name => self.choose_name(view, choices),
            DecisionKind::DeckPosition { .. } => self.choose_deck_position(view, decision, choices),
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
    /// most `max_cost` (or exactly `max_cost` when `exact`, for Upgrade), matching `filter`,
    /// with its pile non-empty and its condition true.
    fn best_gain_rank(&self, view: &PlayerView, max_cost: u8, filter: Filter, exact: bool) -> Option<usize> {
        self.buy.iter().position(|(card, cond)| {
            let cost_ok = if exact { view.cost(*card) == max_cost } else { view.cost(*card) <= max_cost };
            cost_ok
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
        for c in view.supply_cards() {
            if view.supply(c) > 0 {
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
        for c in iter_cards(choices).filter(|c| !self.never_gain.contains(c)) {
            let better = match best {
                None => true,
                Some(b) => key(c) > key(b),
            };
            if better {
                best = Some(c);
            }
        }
        match best {
            Some(c) => Choice::Card(c),
            None if choices.contains(&Choice::Pass) || choices.is_empty() => Choice::Pass,
            // Only never_gain cards are legal and the gain is mandatory: take the cheapest.
            None => iter_cards(choices).min_by_key(|&c| cards::cost(c)).map(Choice::Card).unwrap_or(choices[0]),
        }
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
        // No real choice (every action choice-free, all playable): the obvious order, no search.
        if self.search_play && matches!(decision.kind, DecisionKind::PlayAction) && decision.player == view.me() {
            if let Some(c) = obvious_play(view, choices) {
                return Choice::Card(c);
            }
        }
        if self.search_play && iter_cards(choices).count() >= 2 && decision.player == view.me() {
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
        // Lurker's "trash an Action card from the Supply": the costliest one the gain list
        // doesn't want, so we're not destroying a pile we're building toward.
        if from == Zone::Supply {
            return iter_cards(choices)
                .filter(|&c| !self.lists(c))
                .max_by_key(|&c| cards::cost(c))
                .or_else(|| iter_cards(choices).max_by_key(|&c| cards::cost(c)))
                .map(Choice::Card)
                .unwrap_or(Choice::Pass);
        }
        let forced = min > 0;
        // Upgrades (Remodel, Mine, ...): trash whichever card unlocks the best gain in the gain
        // list, strictly by list order; ties trash the cheaper card.
        if let Some(up) = decision.upgrade {
            let mut best: Option<(usize, u8, CardId)> = None;
            for c in iter_cards(choices) {
                if let Some(rank) = self.best_gain_rank(view, view.cost(c) + up.plus, up.filter, up.exact) {
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
    // Gain-from-zone (Lurker: gain an Action from the trash) and Reveal (Courtier).
    // -----------------------------------------------------------------------------------------

    /// A forced-if-possible gain from a non-Supply zone (currently only Lurker's trash): the
    /// best-ranked gain-list card offered, else the costliest (a bigger card is rarely wrong).
    fn choose_gain_from_zone(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        if let Some(c) = self.match_gain_list(view, choices) {
            return Choice::Card(c);
        }
        iter_cards(choices).max_by_key(|&c| cards::cost(c)).map(Choice::Card).unwrap_or(Choice::Pass)
    }

    /// Which hand card to reveal (Courtier): the one with the most types, to unlock the most
    /// mode picks.
    fn choose_reveal(&self, choices: &[Choice]) -> Choice {
        iter_cards(choices).max_by_key(|&c| cards::def(c).types.count_ones()).map(Choice::Card).unwrap_or(Choice::Pass)
    }

    /// Swindler: every offered card costs the same (the trashed card's cost); give the victim
    /// the worst one — a Curse if one is offered, else a Victory card, else the cheapest junk
    /// (Copper over other Treasures, terminal Actions over useful ones).
    fn worst_gain_for_victim(&self, choices: &[Choice]) -> Choice {
        let rank = |c: CardId| -> i32 {
            if c == id::CURSE {
                0
            } else if cards::is(c, VICTORY) {
                1
            } else if c == id::COPPER {
                2
            } else if cards::is(c, TREASURE) {
                3
            } else if cards::is(c, ACTION) && cards::def(c).actions == 0 && cards::def(c).cards == 0 {
                4 // a dead terminal
            } else {
                5
            }
        };
        iter_cards(choices).min_by_key(|&c| (rank(c), c)).map(Choice::Card).unwrap_or_else(|| choices[0])
    }

    // -----------------------------------------------------------------------------------------
    // Hidden information (Wishing Well / Secret Passage / Masquerade).
    // -----------------------------------------------------------------------------------------

    /// Wishing Well: name the most likely card from the honest view (highest count in the deck,
    /// or the discard if the deck is empty — the same source the engine offers from), ties broken
    /// by whichever the strategy values most (its rank in the gain list, cheapest first as a
    /// fallback so a tie still resolves the same way every time).
    fn choose_name(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        let source = if view.deck_size() > 0 { view.deck() } else { *view.discard() };
        let value = |c: CardId| -> i32 {
            match self.gain_rank(view, c) {
                Some(rank) => (self.gain_list_len() - rank) as i32,
                None => -(cards::cost(c) as i32),
            }
        };
        iter_cards(choices).max_by_key(|&c| (source.get(c), value(c))).map(Choice::Card).unwrap_or_else(|| choices[0])
    }

    /// Secret Passage's card pick (`Select`, `Act::SetAside`): the single most valuable card in
    /// hand (Gold, then any Action, then other Treasures), which `choose_deck_position` then puts
    /// on top; if hand is all Victory/Curse, any of them will do (they get buried at the bottom).
    fn choose_setaside(&self, choices: &[Choice]) -> Choice {
        let value = |c: CardId| -> i32 {
            if cards::is(c, VICTORY) || c == id::CURSE {
                -1
            } else if c == id::GOLD {
                3
            } else if cards::is(c, ACTION) {
                2
            } else {
                1
            }
        };
        iter_cards(choices).max_by_key(|&c| (value(c), cards::cost(c))).map(Choice::Card).unwrap_or_else(|| choices[0])
    }

    /// Secret Passage: keep a good card on top for next turn (a Gold, or an Action the strategy
    /// wants to play); bury pure junk (Victory/Curse) at the bottom when that's offered; otherwise
    /// top.
    fn choose_deck_position(&self, _view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        let card = decision.subject;
        let junk = cards::is(card, VICTORY) || cards::is(card, CURSE_T);
        if junk && choices.contains(&Choice::Position(255)) {
            return Choice::Position(255);
        }
        Choice::Position(0)
    }

    /// Masquerade: pass the card the strategy values least (trash-rule order: Curse, then
    /// Estate, then Copper, then the cheapest). Simplification: this doesn't special-case a
    /// custom `[[trash]]` rule that only wants a specific *other* card gone this turn (the
    /// built-in defaults already target the same junk cards this priority order does, so
    /// deferring to Masquerade's own follow-up trash for them would be redundant, not better).
    fn choose_pass(&self, _view: &PlayerView, choices: &[Choice]) -> Choice {
        for &c in &[id::CURSE, id::ESTATE, id::COPPER] {
            if has_card(choices, c) {
                return Choice::Card(c);
            }
        }
        iter_cards(choices).min_by_key(|&c| (cards::cost(c), c)).map(Choice::Card).unwrap_or_else(|| choices[0])
    }

    // -----------------------------------------------------------------------------------------
    // Mode ("choose one/many": Pawn, Steward, Nobles, Minion, Courtier, Lurker, Torturer victim).
    // -----------------------------------------------------------------------------------------

    /// Which mode option to pick for one remaining choice of a Mode decision (the engine asks
    /// this once per remaining pick, excluding indices already chosen, so a multi-pick card like
    /// Pawn or Courtier is handled by repeated single calls).
    fn choose_mode(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        let card = decision.source.expect("Mode decision always has a source card");
        // Stated `[[mode]]` rules always come first.
        if let Some(c) = self.stated_mode_rule(view, card, choices) {
            return c;
        }
        // The turn player's own modes shape the turn like a play decision: search it with the
        // strategy's own scoring, exactly like `play_decision`. Never applies to a reactive mode
        // decision during someone else's turn (Torturer's victim), which always uses the fast
        // heuristic below regardless of `search_play`.
        if self.search_play && decision.player == view.me() && view.is_my_turn() {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        self.default_mode(view, card, choices)
    }

    /// A matching stated `[[mode]]` rule, if any, in priority order.
    fn stated_mode_rule(&self, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
        self.mode.iter().find_map(|&(c, idx, ref cond)| {
            (c == card && choices.contains(&Choice::Mode(idx)) && cond.as_ref().map_or(true, |e| e.eval_bool(view))).then_some(Choice::Mode(idx))
        })
    }

    /// The mode choice by stated rules then defaults, without searching. Used in search
    /// playouts: calling `choose_mode` there would re-enter `search_play_choice`'s own search on
    /// the same thread-local searcher (it's already mid-call), which panics on the double borrow.
    pub fn rule_mode(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        let card = decision.source.expect("Mode decision always has a source card");
        self.stated_mode_rule(view, card, choices).unwrap_or_else(|| self.default_mode(view, card, choices))
    }

    /// Fast rule-order defaults (`search_play = false`, or the Torturer victim, which is never
    /// searched): one cheap heuristic per mode card.
    fn default_mode(&self, view: &PlayerView, card: CardId, choices: &[Choice]) -> Choice {
        match card {
            id::PAWN => self.default_pawn(view, choices),
            id::STEWARD => self.default_steward(view, choices),
            id::NOBLES => self.default_nobles(view, choices),
            id::MINION => self.default_minion(view, choices),
            id::COURTIER => self.default_courtier(choices),
            id::LURKER => self.default_lurker(view, choices),
            id::TORTURER => self.default_torturer(view, choices),
            _ => choices[0],
        }
    }

    /// Pawn: +1 Card and +$1, unless the hand holds an action but there are no actions left to
    /// play it with, then +1 Action first.
    fn default_pawn(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        let want_action = view.hand().any_type(ACTION) && view.turn().actions == 0;
        let prefs: [fn(&ModeOpt) -> bool; 4] =
            if want_action { [is_actions, is_cards, is_coins, is_buys] } else { [is_cards, is_coins, is_actions, is_buys] };
        for pred in prefs {
            if let Some(c) = find_mode(id::PAWN, choices, pred) {
                return c;
            }
        }
        choices[0]
    }

    /// Steward: trash 2 if the hand has 2+ cards the trash rules want gone, else +2 Cards.
    fn default_steward(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        let junk: u32 = view.hand().iter().filter(|&(c, _)| self.wants_trash(view, c)).map(|(_, n)| n as u32).sum();
        let pred: fn(&ModeOpt) -> bool = if junk >= 2 { is_trash_hand } else { is_cards };
        find_mode(id::STEWARD, choices, pred).unwrap_or(choices[0])
    }

    /// Nobles: +2 Actions if the hand holds 2+ Action cards, else +3 Cards.
    fn default_nobles(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        let pred: fn(&ModeOpt) -> bool = if view.hand().count_type(ACTION) >= 2 { is_actions } else { is_cards };
        find_mode(id::NOBLES, choices, pred).unwrap_or(choices[0])
    }

    /// Minion: +$2, unless a fresh 4-card hand is worth more: keep the hand when the treasure
    /// in hand plus $2 (and any other Minions in hand, each good for at least $2 more) beats 4
    /// cards drawn at the deck's average money per card.
    fn default_minion(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        let money = |c: &Counts| -> u32 {
            c.iter().filter(|&(x, _)| cards::is(x, TREASURE)).map(|(x, n)| cards::def(x).coins as u32 * n as u32).sum()
        };
        let hand = view.hand();
        let keep = money(hand) + 2 + 2 * hand.get(id::MINION) as u32;
        // What a new hand draws from: the deck, reshuffling the discard in if the deck is short.
        let mut pool = view.deck();
        if pool.total() < 4 {
            pool.add_all(view.discard());
        }
        let redraw = if pool.total() == 0 { 0.0 } else { 4.0 * money(&pool) as f64 / pool.total() as f64 };
        let pred: fn(&ModeOpt) -> bool = if redraw > keep as f64 { is_discard_hand_draw } else { is_coins };
        find_mode(id::MINION, choices, pred).unwrap_or(choices[0])
    }

    /// Courtier: +$3, then gain a Gold, then +1 Buy, then +1 Action (in that priority order,
    /// across however many picks the revealed card unlocked).
    fn default_courtier(&self, choices: &[Choice]) -> Choice {
        let prefs: [fn(&ModeOpt) -> bool; 4] = [is_coins, is_gain, is_buys, is_actions];
        for pred in prefs {
            if let Some(c) = find_mode(id::COURTIER, choices, pred) {
                return c;
            }
        }
        choices[0]
    }

    /// Lurker: gain an Action from the trash if it holds one the gain list wants; otherwise
    /// trash from the Supply the costliest Action the strategy doesn't want (the actual trash
    /// target is then chosen by `choose_trash`'s `Zone::Supply` branch).
    fn default_lurker(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        let trash_has_wanted = view.trash().iter().any(|(c, _)| cards::is(c, ACTION) && self.gain_rank(view, c).is_some());
        let pred: fn(&ModeOpt) -> bool = if trash_has_wanted { is_gain_trash } else { is_trash_supply };
        find_mode(id::LURKER, choices, pred).unwrap_or(choices[0])
    }

    /// Torturer's victim: gain the Curse only if discarding 2 would throw away more value than a
    /// Curse costs later. Heuristic: discard 2 if the hand has at least 2 cards that are
    /// Victory/Curse/Copper or otherwise excess (unwanted per the trash rules); otherwise gain
    /// the Curse to hand. Always used regardless of `search_play` (a reactive decision during
    /// someone else's turn, not this player's own play to search).
    fn default_torturer(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        let expendable: u32 = view
            .hand()
            .iter()
            .filter(|&(c, _)| c == id::COPPER || cards::is(c, VICTORY) || cards::is(c, CURSE_T) || self.wants_trash(view, c))
            .map(|(_, n)| n as u32)
            .sum();
        let pred: fn(&ModeOpt) -> bool = if expendable >= 2 { is_discard_hand } else { is_gain };
        find_mode(id::TORTURER, choices, pred).unwrap_or(choices[0])
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

/// The action to play when the play order is not a real choice: every Action card in hand is
/// [`OnPlay::ChoiceFree`](dominion_engine::cards::OnPlay) and all of them can be played with the
/// actions available (playing the +Actions cards first). The order is then +Actions first (most
/// first), then terminals that draw (most first), then the rest: that keeps the most actions
/// for whatever gets drawn. `None` when there is a real choice (a card with a decision, or more
/// terminals than actions), which the bot searches instead.
pub fn obvious_play(view: &PlayerView, choices: &[Choice]) -> Option<CardId> {
    let mut terminals: i32 = 0;
    // Actions left after playing every +Actions card in hand.
    let mut spare = view.turn().actions as i32;
    for (c, n) in view.hand().iter() {
        if !cards::is(c, ACTION) {
            continue;
        }
        if !cards::is_choice_free(c) {
            return None;
        }
        let d = cards::def(c);
        if d.actions == 0 {
            terminals += n as i32;
        } else {
            spare += (d.actions as i32 - 1) * n as i32;
        }
    }
    if terminals > spare {
        return None;
    }
    iter_cards(choices).filter(|&c| cards::is_choice_free(c)).max_by_key(|&c| {
        let d = cards::def(c);
        (d.actions > 0, d.actions, d.cards, d.coins, d.buys, std::cmp::Reverse(c))
    })
}

fn iter_cards(choices: &[Choice]) -> impl Iterator<Item = CardId> + '_ {
    choices.iter().filter_map(|c| if let Choice::Card(x) = c { Some(*x) } else { None })
}

// Shape predicates over `ModeOpt`, used by the mode defaults to find a table index by what kind
// of atom it is (rather than its exact values), so they read as "the +Actions option" etc.
fn is_cards(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::Cards(_))
}
fn is_actions(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::Actions(_))
}
fn is_buys(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::Buys(_))
}
fn is_coins(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::Coins(_))
}
fn is_trash_hand(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::TrashFromHand(_))
}
fn is_discard_hand(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::DiscardFromHand(_))
}
fn is_gain(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::Gain(..))
}
fn is_discard_hand_draw(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::DiscardHandDraw { .. })
}
fn is_trash_supply(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::TrashFromSupply(_))
}
fn is_gain_trash(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::GainFromTrash(_))
}

/// The offered `Choice::Mode(i)` (if any) whose table entry for `card` matches `pred`.
fn find_mode(card: CardId, choices: &[Choice], pred: impl Fn(&ModeOpt) -> bool) -> Option<Choice> {
    cards::modes(card).iter().enumerate().find_map(|(i, o)| {
        let ch = Choice::Mode(i as u8);
        (pred(o) && choices.contains(&ch)).then_some(ch)
    })
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
        Zone::InPlay => *view.in_play(),
        Zone::Trash => *view.trash(),
        Zone::Supply => {
            let mut c = Counts::EMPTY;
            for card in view.supply_cards() {
                c.set(card, view.supply(card));
            }
            c
        }
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
