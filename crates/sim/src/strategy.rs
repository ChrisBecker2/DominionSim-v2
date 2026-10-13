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
//! An optional lookahead rule, `win_this_turn = true` (off by default): when the game is close to
//! ending (Provinces or Colonies low, or three piles under 3 cards), if some line of play this turn
//! wins for certain, gaining any cards it needs, play it. Measured in mirror matches it rarely
//! changes results and costs a lot of speed, so it's opt-in.
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
use dominion_engine::cards::{self, id, CardId, ModeOpt, ACTION, CURSE_T, NUM_CARDS, NUM_ZONE_IDS, TREASURE, VICTORY};
use dominion_engine::engine::{Choice, Decision, DecisionKind};
use dominion_engine::Counts;
use dominion_engine::state::{Act, Filter, Zone};
use serde::Deserialize;

use crate::expr::Expr;

// The bot defaults of each expansion's cards (child module: sees this file's private items).
#[path = "defaults/mod.rs"]
mod defaults;

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

/// `[[token]] token = "+1 Card" pile = "Laboratory" if = "..."`: where to put a token when a card or Event asks (Adventures).
#[derive(Deserialize)]
struct TokenRuleRaw {
    token: String,
    pile: String,
    #[serde(rename = "if")]
    cond: Option<String>,
}

/// `[[debt]] pay = "all" if = "..."`: what to do about Debt when the Buy decision (or, for `all`, the Action phase with a Change in hand) offers to
/// pay it off (Empires). The first rule whose condition holds decides; with none, the default is to pay all of it as soon as the $ are there,
/// and in a game with Change (where one token of Debt makes Change give +$3) to search the turn for the best way.
#[derive(Deserialize)]
struct DebtRuleRaw {
    /// `all` (pay as much as the $ allow), `keep1` (leave one token unpaid) or `none` (pay nothing now).
    pay: String,
    #[serde(rename = "if")]
    cond: Option<String>,
}

/// What a `[[debt]]` rule says to pay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebtPay {
    All,
    Keep1,
    None,
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
const DEFAULT_TRASH: &[(&str, Option<&str>)] = &[
    ("Curse", None),
    // Dark Ages junk: the Ruins, and the Shelters that do nothing (Hovel) or draw when trashed (Overgrown Estate).
    ("Abandoned Mine", None),
    ("Ruined Library", None),
    ("Ruined Market", None),
    ("Ruined Village", None),
    ("Survivors", None),
    ("Hovel", None),
    ("Overgrown Estate", None),
    ("Estate", None),
    ("Copper", None),
];

/// Case/space-insensitive normalization for matching a `[[mode]] choose = "..."` string against
/// `ModeOpt::label()`.
fn norm_label(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect()
}

/// The cards a rule naming `card` stands for: the card itself, or for the Knights or Ruins pile ("Knights", "Ruins") each of the pile's
/// cards in turn (so a rule for "Knights" applies to whichever Knight is on top, or in the hand, and the rule's position and condition
/// are the same for every one).
fn expand_pile(card: CardId) -> Vec<CardId> {
    match dominion_engine::sets::dark_ages::MixedPile::of_marker(card) {
        Some(pile) => pile.members().collect(),
        // The Castles: each of the eight.
        None if card == id::CASTLES => (id::HUMBLE_CASTLE..id::HUMBLE_CASTLE + 8).collect(),
        None => vec![card],
    }
}

/// The cards a rule naming `name` stands for: the card, the cards of a Knights or Ruins pile, or both halves of `Sauna/Avanto`.
fn rule_cards(name: &str) -> Option<Vec<CardId>> {
    let norm: String = name.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect();
    // A split pile named as `Top/Bottom` stands for both halves.
    for (top, bottom) in cards::SPLIT_PILES {
        let both: String = format!("{}/{}", cards::name(top), cards::name(bottom)).chars().filter(|c| !c.is_whitespace() && *c != '\'').flat_map(|c| c.to_lowercase()).collect();
        if norm.replace('\'', "") == both {
            return Some(vec![top, bottom]);
        }
    }
    cards::by_name(name).map(expand_pile)
}

fn compile_rules(raw: &[BuyRuleRaw], what: &str) -> Result<Vec<(CardId, Option<Expr>)>, String> {
    let mut out = Vec::new();
    for r in raw {
        let cards = rule_cards(&r.card).ok_or_else(|| format!("unknown card {:?} in {what} list", r.card))?;
        let cond = r.cond.as_deref().map(Expr::parse).transpose()?;
        for c in cards {
            out.push((c, cond.clone()));
        }
    }
    Ok(out)
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
    /// Token placement rules (Adventures), in priority order.
    #[serde(default)]
    token: Vec<TokenRuleRaw>,
    /// Debt payment rules (Empires), in priority order.
    #[serde(default)]
    debt: Vec<DebtRuleRaw>,
    /// Rules for calling a Reserve card (Adventures): the first rule for the card whose condition holds says yes.
    #[serde(default)]
    call: Vec<BuyRuleRaw>,
    /// Rules for exchanging a Traveller when it is discarded from play: the first rule for the card whose condition holds says yes
    /// (exchanging is the default).
    #[serde(default)]
    exchange: Vec<BuyRuleRaw>,
    /// Chapel/Sentry won't trash a treasure from hand if doing so would leave fewer than this
    /// many treasures in hand. Default 2.
    #[serde(default)]
    keep_treasure: Option<u8>,
    /// Before following the lists, when the game is close to ending, look for a line of play that
    /// wins the game this turn for certain (gaining any cards); play it if found. Default false
    /// (opt-in: it rarely changes results and is expensive).
    #[serde(default)]
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
    /// Whether the gain list skips the last Province or Colony when taking it would end the game
    /// with this player not winning. On unless a rule's condition uses `wins_game` / `loses_game`
    /// itself.
    guard_game_end: bool,
    /// `[[play]]` rules in priority order.
    play: Vec<(CardId, Option<Expr>)>,
    /// Rank of each card's first `[[play]]` rule (lower = earlier), or `-1` if unlisted.
    play_rank: [i16; NUM_CARDS],
    /// `[[trash]]` rules followed by the defaults (`DEFAULT_TRASH`).
    trash: Vec<(CardId, Option<Expr>)>,
    /// `[[mode]]` rules in priority order: (card, index into `cards::modes(card)`, condition).
    mode: Vec<(CardId, u8, Option<Expr>)>,
    /// `[[token]]` rules in priority order: (token index, pile, condition).
    token: Vec<(usize, CardId, Option<Expr>)>,
    /// `[[debt]]` rules in priority order.
    debt: Vec<(DebtPay, Option<Expr>)>,
    /// `[[call]]` and `[[exchange]]` rules in priority order.
    call: Vec<(CardId, Option<Expr>)>,
    exchange: Vec<(CardId, Option<Expr>)>,
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
            let cards = rule_cards(&rule.card).ok_or_else(|| format!("unknown card {:?} in gain list", rule.card))?;
            let cond = rule.cond.as_deref().map(Expr::parse).transpose()?;
            for c in cards {
                buy.push((c, cond.clone()));
            }
        }

        let play = compile_rules(&raw.play.into_rules(), "play")?;
        let mut play_rank = [-1i16; NUM_CARDS];
        for (i, (card, _)) in play.iter().enumerate() {
            if !cards::is(*card, ACTION | cards::NIGHT) {
                return Err(format!("{:?} in play list is not an Action or Night card", cards::name(*card)));
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

        // Token placement rules: the token's name, the pile (a card; the Knights pile by "Knights", the split pile by either half), a condition.
        let mut token = Vec::with_capacity(raw.token.len());
        for rule in &raw.token {
            let tok = cards::token_by_name(&rule.token).ok_or_else(|| format!("unknown token {:?} in [[token]] rule", rule.token))?;
            if tok >= dominion_engine::state::NUM_PILE_TOKENS && tok != 6 {
                return Err(format!("the {:?} token is not placed on a pile: a [[token]] rule can name +1 Card, +1 Action, +1 Buy, +$1, -$2 cost, Trashing or Estate", rule.token));
            }
            let pile = rule_cards(&rule.pile).and_then(|v| v.first().copied()).ok_or_else(|| format!("unknown card {:?} in [[token]] rule", rule.pile))?;
            // A Knight names the Knights pile, Avanto the split pile.
            let pile = if pile >= cards::id::DAME_ANNA && pile < NUM_ZONE_IDS as CardId { id::KNIGHTS } else { cards::pile_of(pile) };
            let cond = rule.cond.as_deref().map(Expr::parse).transpose()?;
            token.push((tok, pile, cond));
        }
        let mut debt = Vec::with_capacity(raw.debt.len());
        for rule in &raw.debt {
            let pay = match norm_label(&rule.pay).as_str() {
                "all" => DebtPay::All,
                "keep1" | "keepone" | "keep_1" => DebtPay::Keep1,
                "none" => DebtPay::None,
                other => return Err(format!("unknown pay {other:?} in a [[debt]] rule (expected \"all\", \"keep1\" or \"none\")")),
            };
            debt.push((pay, rule.cond.as_deref().map(Expr::parse).transpose()?));
        }
        let call = compile_rules(&raw.call, "call")?;
        for (c, _) in &call {
            if !cards::is(*c, cards::RESERVE) {
                return Err(format!("{:?} in the call list is not a Reserve card", cards::name(*c)));
            }
        }
        let exchange = compile_rules(&raw.exchange, "exchange")?;
        for (c, _) in &exchange {
            // A Traveller discarded from play, or (Nocturne) a card costing $3 or more that may be exchanged for a Changeling when gained.
            if cards::traveller_upgrade(*c).is_none() && (cards::is_pseudo(*c) || cards::cost(*c) < 3) {
                return Err(format!("{:?} in the exchange list is not a Traveller or a card costing $3 or more", cards::name(*c)));
            }
        }

        let guard_game_end = !buy.iter().any(|(_, c): &(CardId, Option<Expr>)| c.as_ref().is_some_and(|e| e.mentions_game_end()));
        Ok(Strategy {
            name: raw.name,
            description: raw.description,
            guard_game_end,
            buy,
            play,
            play_rank,
            trash,
            mode,
            token,
            debt,
            call,
            exchange,
            keep_treasure: raw.keep_treasure.unwrap_or(2),
            win_this_turn: raw.win_this_turn,
            never_gain: raw
                .never_gain
                .iter()
                .map(|n| cards::by_name(n).ok_or_else(|| format!("unknown card {n:?} in never_gain")))
                .collect::<Result<Vec<CardId>, _>>()?
                .into_iter()
                .flat_map(expand_pile)
                .collect(),
            search_play: raw.search_play,
        })
    }

    /// Every kingdom card (id >= `FIRST_KINGDOM`) this strategy names, in its buy list, play
    /// list, or buy conditions. Used to build an "auto" kingdom selection. Allocates (fine: not
    /// called during simulation).
    pub fn kingdom_refs(&self) -> Vec<CardId> {
        let mut out = Vec::new();
        let mut push = |c: CardId| {
            // Potion joins the supply by itself whenever a Potion-cost card is in the kingdom.
            // A Reward is never in the kingdom (Joust gains it): naming one asks for Joust. The same for Madman (Hermit), Mercenary
            // (Urchin) and a Knight (the Knights pile); the Ruins come with a Looter and Spoils with its gainers, so naming them
            // asks for nothing.
            let c = if cards::is_reward(c) {
                id::JOUST
            } else if c == id::MADMAN {
                id::HERMIT
            } else if c == id::MERCENARY {
                id::URCHIN
            } else if c >= cards::id::DAME_ANNA && c < NUM_ZONE_IDS as CardId {
                id::KNIGHTS
            } else if cards::is_split_bottom(c) {
                cards::split_top(c).unwrap_or(c)
            } else if cards::is_castle(c) {
                id::CASTLES
            } else if c >= cards::FIRST_TRAV && ((c - cards::FIRST_TRAV) as usize) < cards::NUM_TRAV {
                // A Traveller upgrade comes with the first card of its line.
                if c < cards::id::SOLDIER { id::PAGE } else { id::PEASANT }
            } else {
                c
            };
            let kingdom = cards::is_kingdom(c) || cards::is_event(c) || cards::is_landmark(c) || cards::is_prophecy(c);
            if c >= cards::FIRST_KINGDOM && c != id::POTION && (kingdom || cards::is_optional_basic(c)) && !out.contains(&c) {
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
        // Stated rules come before any search.
        if let Some(c) = self.stated_effect_choice(view, decision, choices) {
            return c;
        }
        // Debt: pay it as soon as there is $ for it. In a game with Change, whether to keep a token (Change gives +$3 with Debt) is searched.
        if self.search_play
            && decision.kind == DecisionKind::Buy
            && decision.player == view.me()
            && view.is_my_turn()
            && view.debt_decision_matters()
            && choices.iter().any(|c| matches!(c, Choice::PayDebt | Choice::PayDebtKeep(_)))
        {
            if let Some(c) = self.stated_debt_choice(view, choices) {
                return c;
            }
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // Choices inside a Treasure being played (Investment, Crystal Ball, Anvil) change which
        // Treasures are left to play: searched like the play order when `search_play` is on.
        if self.search_play && view.is_my_turn() && crate::eval::is_treasure_effect_decision(view.me(), decision) && choices.len() > 1 {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // Calls that shape the turn (Duplicate on a gain, Royal Carriage, Coin of the Realm, Wine Merchant's discard) are searched like the
        // play order, unless a `[[call]]` rule has spoken (above).
        if self.search_play && view.is_my_turn() && crate::eval::is_call_decision(view.me(), decision) && choices.len() > 1 {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // How much to overpay shapes the turn too (what it buys is scored by the gain list).
        if self.search_play && view.is_my_turn() && crate::eval::is_overpay_decision(view.me(), decision) && choices.len() > 1 {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // The order of the start-of-turn effects, and Prince's set-aside choices, are searched too:
        // they decide what the turn starts with. Orders of draws, returns and discards are left to
        // the rules (see `START_ORDER_SEARCH_ALL`); only an order with a play in it (Prince, Clerk)
        // is a real trade-off.
        if decision.kind == DecisionKind::StartOrder
            && self.search_play
            && view.is_my_turn()
            && choices.len() > 1
            && crate::eval::is_start_effect_decision(view.me(), decision)
            && (START_ORDER_SEARCH_ALL || start_order_has_play(choices))
        {
            if let Some(c) = crate::eval::search_start_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
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
            DecisionKind::Buy => self.buy_decision(view, choices),
            DecisionKind::Overpay { card, max_coins, potion } => self.choose_overpay(view, card, max_coins, potion),
            DecisionKind::Gain { .. } if decision.for_player != decision.player => {
                // Swindler: this gain is forced on `for_player` (the victim), not chosen for
                // ourselves. Give them the worst option offered (all cost the same, by rule).
                self.worst_gain_for_victim(choices)
            }
            DecisionKind::Gain { .. } => self.choose_gain(view, decision, choices),
            DecisionKind::Select { from, act, filter, min, max, ordered } => {
                // A card with a policy of its own for this decision (Scrying Pool, Prince, Captain, Advisor, Plaza,
                // Farmhands, Joust, Herbalist...): the `SetDefaults` of its set.
                if let Some(c) = decision.source.and_then(|src| defaults::select(self, view, decision, src, act, from, choices)) {
                    return c;
                }
                match act {
                    Act::Discard => self.choose_discard(view, from, filter, choices, min, max, ordered),
                    Act::Trash => self.choose_trash(view, decision, choices, from, filter, min, max, ordered),
                    Act::Topdeck => self.choose_topdeck(view, choices, from, filter, ordered),
                    Act::Play => self.play_decision(view, decision, choices, decision.play_times > 1),
                    Act::SetAside => self.choose_setaside(choices),
                    Act::Gain => self.choose_gain_from_zone(view, choices),
                    Act::Reveal => self.choose_reveal(choices),
                    Act::Pass => self.choose_pass(view, choices),
                    // Miser's Copper, and the picks the engine makes of calls and exchanges (they are YesNo decisions): the first offered.
                    Act::Tavern | Act::Call | Act::Exchange | Act::Return | Act::Archive | Act::ShuffleIn => {
                        iter_cards(choices).next().map(Choice::Card).unwrap_or(Choice::Pass)
                    }
                    // Archive's card to take (the set's own policy chose it above); any other: the best card by value.
                    Act::ToHand => iter_cards(choices).max_by_key(|&c| (Strategy::card_value(c), c)).map(Choice::Card).unwrap_or(Choice::Pass),
                }
            }
            DecisionKind::Mode { .. } => self.choose_mode(view, decision, choices),
            DecisionKind::Bid { min } => defaults::choose_bid(view, min, choices),
            DecisionKind::Token { token } => defaults::choose_token(self, view, token as usize, choices),
            // A stated rule for a call or an exchange decides; the defaults are the set's (`defaults::adventures`).
            DecisionKind::YesNo { act: Act::Call | Act::Exchange | Act::Return } | DecisionKind::YesNo { act: Act::Discard }
                if self.stated_call_or_exchange(view, decision).is_some() =>
            {
                if self.stated_call_or_exchange(view, decision).unwrap() { Choice::Yes } else { Choice::No }
            }
            DecisionKind::YesNo { .. } if defaults::yes_no(self, view, decision).is_some() => {
                defaults::yes_no(self, view, decision).unwrap()
            }
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
                // Diplomat's reaction: only offered once the hand already has 5+ cards. Moat
                // (the source is the Attack): see `reveal_to_attack`.
                Act::Reveal => {
                    if decision.source.is_some_and(|s| s != decision.subject && cards::is(s, cards::ATTACK)) {
                        self.reveal_to_attack(view, decision)
                    } else {
                        Choice::Yes
                    }
                }
                // Treasury: always put it back unless the strategy says otherwise (there's no
                // stated-rule override for this shape yet, so this is the whole policy for now).
                Act::Topdeck => Choice::Yes,
                _ => Choice::No,
            },
            DecisionKind::Name => self.choose_name(view, decision, choices),
            DecisionKind::DeckPosition { .. } => self.choose_deck_position(view, decision, choices),
            DecisionKind::PlayTreasure => self.play_treasure_decision(view, decision, choices),
            DecisionKind::PlayNight => self.night_decision(view, decision, choices),
            DecisionKind::PickBoon => defaults::pick_boon(self, view, decision, choices),
            DecisionKind::StartOrder => self.choose_start_order(choices),
        }
    }

    /// Which pending start-of-turn effect resolves first. Cards put into the hand come first
    /// (Haven and Blockade returning their card, Pirate's Treasure), so the discards and trashes
    /// below pick from the fullest hand; then the plain bonuses and draws; then plays (Clerk, and
    /// a card a Prince plays: the choice names that card); and last the effects that discard or
    /// trash from the hand (Sea Witch and Tide Pools discarding, Sailor trashing). Within a
    /// group, by card.
    fn choose_start_order(&self, choices: &[Choice]) -> Choice {
        let group = |c: CardId| -> u8 {
            defaults::start_order_group(c).unwrap_or(match c {
                // Any other Action is a Prince's card, unless it is a plain bonus Duration.
                _ if cards::is(c, cards::DURATION) => 1,
                _ if cards::is(c, ACTION) => 2,
                _ => 1,
            })
        };
        iter_cards(choices).min_by_key(|&c| (group(c), c)).map(Choice::Card).unwrap_or(choices[0])
    }

    // -----------------------------------------------------------------------------------------
    // Buy / Gain: shared "walk the priority list" logic.
    // -----------------------------------------------------------------------------------------


    /// The Buy decision: the first card of the gain list that is affordable now, but when Coffers
    /// would reach a card listed earlier, spend a token (one at a time: each is its own decision) until it
    /// is affordable. Tokens not needed for the first listed card the bot can reach are kept.
    fn buy_decision(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        // Debt: pay it off as soon as there is $ for it, before anything is bought (the exceptions are searched: see `decide`).
        if view.debt_of(view.me()) > 0 && choices.contains(&Choice::PayDebt) {
            return self.stated_debt_choice(view, choices).unwrap_or(Choice::PayDebt);
        }
        if choices.contains(&Choice::Coffers) {
            if let Some(target) = self.first_reachable(view) {
                return if has_card(choices, target) { Choice::Card(target) } else { Choice::Coffers };
            }
            return Choice::Pass;
        }
        self.match_gain_list(view, choices).map(Choice::Card).unwrap_or(Choice::Pass)
    }

    /// The choice a stated `[[debt]]` rule makes among `choices` (a Buy decision with Debt to pay), if one applies.
    pub fn stated_debt_choice(&self, view: &PlayerView, choices: &[Choice]) -> Option<Choice> {
        let (pay, _) = self.debt.iter().find(|(_, cond)| cond.as_ref().map_or(true, |e| e.eval_bool(view)))?;
        let debt = view.debt_of(view.me());
        Some(match pay {
            DebtPay::All if choices.contains(&Choice::PayDebt) => Choice::PayDebt,
            DebtPay::Keep1 if choices.contains(&Choice::PayDebtKeep(1)) => Choice::PayDebtKeep(1),
            // One token (or none) is already all that would be kept; with more but no $ to keep it with, pay what there is.
            DebtPay::Keep1 if debt > 1 && choices.contains(&Choice::PayDebt) => Choice::PayDebt,
            _ => Choice::Pass,
        })
    }

    /// Whether a `[[debt]]` rule exists.
    pub fn states_debt(&self) -> bool {
        !self.debt.is_empty()
    }

    /// The first gain-list card that can be bought with the coins and every Coffers token.
    fn first_reachable(&self, view: &PlayerView) -> Option<CardId> {
        // Debt is paid out of the money first.
        let money = (view.turn().coins as u32 + view.coffers() as u32).checked_sub(view.debt_of(view.me()) as u32)?;
        self.first_reachable_with(view, money, view.turn().potions)
    }

    /// The first gain-list card that `money` and `potions` reach.
    fn first_reachable_with(&self, view: &PlayerView, money: u32, potions: u8) -> Option<CardId> {
        self.buy.iter().find_map(|(card, cond)| {
            let ok = if cards::is_event(*card) {
                view.event_available(*card) && view.event_cost(*card) as u32 <= money
            } else {
                view.in_supply(*card)
                    && view.supply(*card) > 0
                    && view.cost(*card) as u32 <= money
                    && (potions > 0 || !cards::potion_cost(*card))
                    && view.may_buy(*card)
            } && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card));
            ok.then_some(*card)
        })
    }

    /// How much to overpay for the card just bought, by rule (the bot searches it when `search_play`
    /// is on). Stonemason: the amount at which the gain list ranks an Action costing exactly that
    /// highest (with the Potion when that is a listed Potion card), none if there is no listed one.
    /// The others: the coins the bot has no listed use for (the cheapest-first buy it would still
    /// make with the remaining Buys is left alone): Farrier, Herald (up to the cards in the discard
    /// pile) and Infirmary (up to 3).
    fn choose_overpay(&self, view: &PlayerView, card: CardId, max_coins: u8, potion: bool) -> Choice {
        if let Some(c) = defaults::overpay(self, view, card, max_coins, potion) {
            return c;
        }
        let mut spare = max_coins as u32;
        if view.buys_left() > 0 {
            // Keep what the best listed card the bot could still buy costs.
            if let Some(c) = self.best_affordable(view, max_coins) {
                spare = spare.saturating_sub(view.cost(c) as u32);
            }
        }
        let cap = defaults::overpay_cap(view, card);
        Choice::Overpay(spare.min(cap).min(max_coins as u32) as u8)
    }

    /// The best listed card buyable with `coins` (ignoring Potions).
    fn best_affordable(&self, view: &PlayerView, coins: u8) -> Option<CardId> {
        self.buy.iter().find_map(|(card, cond)| {
            let ok = view.in_supply(*card)
                && view.supply(*card) > 0
                && view.cost(*card) <= coins
                && !cards::potion_cost(*card)
                && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card));
            ok.then_some(*card)
        })
    }

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
    fn best_gain_rank(&self, view: &PlayerView, max_cost: u8, filter: Filter, exact: bool, potion: bool) -> Option<usize> {
        self.buy.iter().position(|(card, cond)| {
            // Potion: "up to $X" admits Potion-cost cards only when the reference cost has one;
            // "exactly" needs it to match (the engine's gain rule, see `Frame::potion`). A card with Debt in its cost is never gained that way.
            let pc = cards::potion_cost(*card);
            let cost_ok = cards::debt_cost(*card) == 0
                && if exact { view.cost(*card) == max_cost && pc == potion } else { view.cost(*card) <= max_cost && (potion || !pc) };
            cost_ok
                && filter.matches(*card)
                && view.in_supply(*card)
                && view.supply(*card) > 0
                && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card))
        })
    }

    /// Rank of the best gain-list card costing up to `max_cost` (and no Potion or Debt) that could be gained now.
    pub(crate) fn best_gain_rank_up_to(&self, view: &PlayerView, max_cost: u8) -> Option<usize> {
        self.best_gain_rank(view, max_cost, Filter::Any, false, false)
    }

    /// Rank of the best gain-list card costing more $ than `above` that could be gained now (Change's gain; Debt is the difference).
    pub(crate) fn best_gain_rank_more(&self, view: &PlayerView, above: u8) -> Option<usize> {
        self.buy.iter().position(|(card, cond)| {
            !cards::is_pseudo(*card)
                && view.cost(*card) > above
                && view.in_supply(*card)
                && view.supply(*card) > 0
                && view.pile_open(*card)
                && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card))
        })
    }

    /// The best-ranked Action in the gain list costing up to `max_cost`, in its pile now.
    pub(crate) fn best_listed_action_up_to(&self, view: &PlayerView, max_cost: u8) -> Option<CardId> {
        self.buy.iter().find_map(|(card, cond)| {
            (cards::is(*card, ACTION)
                && view.cost(*card) <= max_cost
                && cards::debt_cost(*card) == 0
                && !cards::potion_cost(*card)
                && view.in_supply(*card)
                && view.supply(*card) > 0
                && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)))
            .then_some(*card)
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
        // Colony games end when the Colony pile empties too (0 when Colony isn't in play).
        if view.in_supply(id::COLONY) && (view.supply(id::COLONY) as u32) <= MAX_ENDING_GAINS {
            return true;
        }
        // The pile ending: as many piles as end the game (3; 4 with 5+ players) each have fewer
        // than 3 cards left, counting empty ones.
        let pile_limit: u32 = if view.num_players() >= 5 { 4 } else { 3 };
        let low = view.supply_cards().filter(|&c| view.supply(c) < 3).count() as u32;
        low >= pile_limit
    }

    /// First entry in the gain list that's a legal choice and whose condition holds.
    fn match_gain_list(&self, view: &PlayerView, choices: &[Choice]) -> Option<CardId> {
        for (card, cond) in &self.buy {
            if has_card(choices, *card)
                && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card))
                && !(self.guard_game_end && Self::gain_loses(view, *card))
            {
                return Some(*card);
            }
        }
        None
    }

    /// Gaining `card` would end the game this turn with me not among the winners (a shared win is
    /// fine). Only the last Province or Colony is checked (emptying a third pile is not, for now):
    /// any other gain costs one comparison.
    #[inline]
    fn gain_loses(view: &PlayerView, card: CardId) -> bool {
        (card == id::PROVINCE || card == id::COLONY)
            && view.supply(card) == 1
            && matches!(view.result_if_gained(card), Some(w) if w & (1 << view.me()) == 0)
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
        // An optional gain (University) that the gain list doesn't ask for is declined.
        if choices.contains(&Choice::Pass) {
            return Choice::Pass;
        }
        // Summon's Action is played free at the start of the next turn, like a Prince's: the most useful card.
        if _decision.source == Some(id::SUMMON) {
            return defaults::promo_summon_pick(choices);
        }
        // Joust's Reward (a gain from outside the Supply) when the list names none of them.
        if let Some(r) = defaults::default_side_gain(choices) {
            return Choice::Card(r);
        }
        // Change with no listed gain: the cheapest card that costs more (the Debt is the difference), the best of those.
        if _decision.source == Some(id::CHANGE) {
            return iter_cards(choices).min_by_key(|&c| (cards::cost(c), -Self::card_value(c), c)).map(Choice::Card).unwrap_or(Choice::Pass);
        }
        // Nothing in the buy list is affordable/legal here (e.g. a small Workshop/Remodel gain
        // below everything we listed). Never gain Curse if there's an alternative, and avoid
        // dead Victory cards unless the game is ending; otherwise take the most expensive legal
        // option, since a bigger card is rarely a mistake to gain.
        let endgame = view.supply(id::PROVINCE) <= 3
            || (view.in_supply(id::COLONY) && view.supply(id::COLONY) <= 3)
            || view.empty_piles() >= 2;
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
            .filter(|&c| for_multiplier || self.default_worth_playing(view, c))
            .min_by_key(|&c| (self.play_rank_of(c), c))
    }

    /// Default (unstated) check for Actions that can cost more than they give (Tactician, Treasure Map: the card's set
    /// says, `SetDefaults::worth_playing`). Everything else is worth playing.
    fn default_worth_playing(&self, view: &PlayerView, card: CardId) -> bool {
        // Enlightenment: a Treasure played in the Action phase is a card and an Action instead of its $; only a Copper is worth that.
        if !cards::is(card, ACTION) && view.is_treasure(card) {
            return card == id::COPPER && view.phase() == dominion_engine::Phase::Action;
        }
        defaults::worth_playing(self, view, card)
    }

    /// Which action to play (or what a Throne Room plays). With a real choice, the bot searches
    /// its own turn with its own scoring (gain list > stated [[play]] rules > defaults) and plays
    /// the best line; a single playable card is simply played. The rule order in `best_play` is
    /// only a fallback if the search can't run.
    fn play_decision(&self, view: &PlayerView, decision: &Decision, choices: &[Choice], for_multiplier: bool) -> Choice {
        // A stated `[[debt]] pay = "all"` rule pays the Debt off before a Change is played (so that it remodels).
        if matches!(decision.kind, DecisionKind::PlayAction) && choices.contains(&Choice::PayDebt) && self.stated_debt_choice(view, choices) == Some(Choice::PayDebt) {
            return Choice::PayDebt;
        }
        // No real choice (every action choice-free, all playable): the obvious order, no search.
        if self.search_play && matches!(decision.kind, DecisionKind::PlayAction) && decision.player == view.me() {
            if let Some(c) = obvious_play(view, choices) {
                return Choice::Card(c);
            }
            // A real choice, but the +Actions cantrips go first (unless the hand is
            // order-sensitive or the stated play rules speak to it).
            if !self.states_play_for_hand(view) {
                if let Some(c) = cantrip_first(view, choices) {
                    return Choice::Card(c);
                }
            }
        }
        // Search when there's more than one card to choose from, or when the only one isn't
        // choice-free: playing it can be worse than not (Tactician discards the hand, a lone
        // Treasure Map trashes itself, Remodel must trash something).
        let cards_offered = iter_cards(choices).count();
        let lone_risky = cards_offered == 1
            && choices.contains(&Choice::Pass)
            && matches!(decision.kind, DecisionKind::PlayAction)
            && iter_cards(choices).all(|c| !cards::is_choice_free(c));
        if self.search_play && (cards_offered >= 2 || lone_risky) && decision.player == view.me() {
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
        if let Some(c) = defaults::rule_play(self, view, decision, choices) {
            return c;
        }
        self.best_play(view, choices, decision.play_times > 1).map(Choice::Card).unwrap_or(Choice::Pass)
    }

    /// Whether the strategy's `[[play]]` rules name an Action in hand that isn't choice-free
    /// (then its stated order, not `cantrip_first`, decides).
    pub fn states_play_for_hand(&self, view: &PlayerView) -> bool {
        self.play.iter().any(|(c, _)| view.hand().has(*c) && !cards::is_choice_free(*c))
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
        // Rocks (Empires) gains a Silver when trashed, so it is always worth trashing (but not named: that would ask for its split pile).
        card == id::ROCKS || self.trash.iter().any(|(c, cond)| *c == card && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)))
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
        } else if cards::is(card, cards::RUINS) || card == id::HOVEL {
            -400
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
        // Worst card first, ties by id: pick the least (rank, id) among the zone's cards not yet taken (a zone holds few distinct cards).
        while left > 0 {
            let Some((rank, c)) = zone.iter().filter(|&(c, _)| wanted.get(c) == 0).map(|(c, _)| (Strategy::discard_rank(c), c)).min() else { break };
            if min == 0 && rank >= 0 {
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
                if let Some(rank) = self.best_gain_rank(view, view.cost(c) + up.plus, up.filter, up.exact, cards::potion_cost(c)) {
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
            // Cheapest first, ties by id, among the zone's cards not yet fully wanted.
            while need > 0 {
                let Some((_, c)) = zone.iter().filter(|&(c, n)| n > wanted.get(c)).map(|(c, _)| (cards::cost(c), c)).min() else { break };
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
    // Alchemy: Scrying Pool's keep-or-discard and Herbalist's Treasure.
    // -----------------------------------------------------------------------------------------

    // -----------------------------------------------------------------------------------------
    // Hidden information (Wishing Well / Secret Passage / Masquerade).
    // -----------------------------------------------------------------------------------------

    /// Name a card: Wishing Well (guessing our own deck top) and War Chest (as the player to the
    /// War Chest owner's left, naming a card to deny them) are opposite in spirit, so they're
    /// told apart by `decision.source`, the same way `choose_mode` dispatches per-card.
    fn choose_name(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        if let Some(c) = decision.source.and_then(|src| defaults::name(self, view, decision, src, choices)) {
            return c;
        }
        // Wishing Well: name the most likely card from the honest view (highest count in the
        // deck, or the discard if the deck is empty — the same source the engine offers from),
        // ties broken by whichever the strategy values most (its rank in the gain list, cheapest
        // first as a fallback so a tie still resolves the same way every time).
        let source = if view.deck_size() > 0 { view.deck() } else { *view.discard() };
        let value = |c: CardId| -> i32 {
            match self.gain_rank(view, c) {
                Some(rank) => (self.gain_list_len() - rank) as i32,
                None => -(cards::cost(c) as i32),
            }
        };
        iter_cards(choices).max_by_key(|&c| (source.get(c), value(c))).map(Choice::Card).unwrap_or_else(|| choices[0])
    }

    /// `PlayTreasure`: with `search_play`, searched with the strategy's own scoring exactly like
    /// `play_decision` does for `PlayAction` (`is_play_decision` in `eval.rs` treats it the same
    /// way); `rule_play_treasure` is the fast/default fallback otherwise.
    fn play_treasure_decision(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        // An Event offered before the Treasures are played (Alms) is searched like the play order.
        let event = iter_cards(choices).find(|&c| cards::is_event(c));
        // Nothing left whose order or choices matter (only plain Treasures, and Bank, which goes
        // last): the rule order, no search needed.
        if event.is_none() && !iter_cards(choices).any(|c| !plays_last(c) && !cards::is_choice_free(c)) {
            return self.rule_play_treasure(choices);
        }
        if self.search_play && decision.player == view.me() {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        if let Some(e) = event {
            if self.event_before_treasures(view, e) {
                return Choice::Card(e);
            }
        }
        self.rule_play_treasure(choices)
    }

    /// By the rules, whether the Event `e` (Alms) is bought before the Treasures are played: when it is the first gain-list card that the
    /// money the hand holds would reach (the Buy decision after the Treasures would pick it just the same, if it still did anything).
    fn event_before_treasures(&self, view: &PlayerView, e: CardId) -> bool {
        let mut money = view.turn().coins as u32 + view.coffers() as u32;
        let mut potions = view.turn().potions;
        let debt = view.debt_of(view.me()) as u32;
        for (c, n) in view.hand().iter().filter(|&(c, _)| cards::is(c, cards::TREASURE)) {
            money += cards::def(c).coins as u32 * n as u32;
            potions += cards::def(c).potions * n;
        }
        money.checked_sub(debt).and_then(|m| self.first_reachable_with(view, m, potions)) == Some(e)
    }

    /// `PlayNight`: the Night cards are played by the same machinery as the Actions: when every Night card in hand is choice-free the order does
    /// not matter and they are all played (no search); otherwise the turn is searched with the strategy's own scoring, and a stated `[[play]]`
    /// rule for a Night card (with its condition) decides how they rank.
    fn night_decision(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        if self.search_play && decision.player == view.me() {
            if let Some(c) = night_obvious(view, choices) {
                return Choice::Card(c);
            }
            if iter_cards(choices).count() >= 2 || iter_cards(choices).all(|c| !cards::is_choice_free(c)) {
                if let Some(c) = crate::eval::search_play_choice(self, view) {
                    if choices.contains(&c) {
                        return c;
                    }
                }
            }
        }
        self.rule_night(view, choices)
    }

    /// The Night play by rule order alone (stated `[[play]]` rules, then the cards that cost nothing to play, then the rest by id), without
    /// searching. Used in search playouts.
    pub fn rule_night(&self, view: &PlayerView, choices: &[Choice]) -> Choice {
        for (card, cond) in &self.play {
            if has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)) {
                return Choice::Card(*card);
            }
        }
        defaults::rule_night(self, view, choices)
    }

    /// The `PlayTreasure` choice by rule order alone, without searching:
    /// 1. Treasures with a choice, except Bank (Investment, Anvil, Tiara, Crystal Ball, War
    ///    Chest), while the plain Treasures are still in hand for them to count, trash, discard or
    ///    replay;
    /// 2. if Bank is waiting, the plain Treasures one by one, then Bank (it counts every Treasure
    ///    in play, so it goes last);
    /// 3. Done (which plays any plain Treasures left).
    /// Used both as the `search_play = false` default and inside search playouts (avoiding
    /// re-entering `search_play_choice` from within its own playout).
    pub fn rule_play_treasure(&self, choices: &[Choice]) -> Choice {
        if let Some(c) = iter_cards(choices).find(|&c| !plays_last(c) && !cards::is_choice_free(c) && !cards::is_event(c)) {
            return Choice::Card(c);
        }
        if iter_cards(choices).any(plays_last) {
            if let Some(c) = iter_cards(choices).find(|&c| cards::is_choice_free(c)) {
                return Choice::Card(c);
            }
            // The Treasures that count the others, in their order (Horn of Plenty counts names, before Bank, which counts
            // Treasures, the Horn too).
            if let Some(c) = iter_cards(choices).filter_map(|c| defaults::plays_last_rank(c).map(|r| (r, c))).min() {
                return Choice::Card(c.1);
            }
        }
        Choice::Pass
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

    /// A choice dictated by a stated rule, for decisions a search would otherwise take over:
    /// a matching `[[mode]]` rule, or (inside a Treasure's effect: Investment, Crystal Ball) a
    /// hand trash when the `[[trash]]` rules want one of the offered cards gone.
    pub fn stated_effect_choice(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        match decision.kind {
            DecisionKind::Mode { .. } => self.stated_mode_rule(view, decision.source?, choices),
            DecisionKind::YesNo { act: Act::Call | Act::Exchange | Act::Discard | Act::Return } => {
                self.stated_call_or_exchange(view, decision).map(|yes| if yes { Choice::Yes } else { Choice::No })
            }
            DecisionKind::Select { from: Zone::Hand, act: Act::Trash, filter, min, max, ordered }
                if decision.source.is_some_and(|c| cards::is(c, TREASURE)) && iter_cards(choices).any(|c| self.wants_trash(view, c)) =>
            {
                Some(self.choose_trash(view, decision, choices, Zone::Hand, filter, min, max, ordered))
            }
            _ => None,
        }
    }

    /// The answer a stated `[[call]]` rule (a Reserve card's call, Wine Merchant's discard) or `[[exchange]]` rule (a Traveller discarded from
    /// play) gives to `decision`: `Some(true)` for yes, `Some(false)` for no, `None` when no rule speaks to this card.
    pub fn stated_call_or_exchange(&self, view: &PlayerView, decision: &Decision) -> Option<bool> {
        match decision.kind {
            DecisionKind::YesNo { act: Act::Call } => defaults::stated_call(self, view, decision.subject),
            DecisionKind::YesNo { act: Act::Discard } if decision.subject == id::WINE_MERCHANT => defaults::stated_call(self, view, id::WINE_MERCHANT),
            DecisionKind::YesNo { act: Act::Exchange } => defaults::stated_exchange(self, view, decision.subject),
            // A card gained that may be exchanged for a Changeling (Nocturne): the rule names the gained card.
            DecisionKind::YesNo { act: Act::Return } if matches!(decision.source, Some(id::CHANGELING | id::TRADER)) => {
                defaults::stated_exchange(self, view, decision.subject)
            }
            _ => None,
        }
    }

    /// Whether a `[[mode]]` rule names `card`.
    pub fn states_mode(&self, card: CardId) -> bool {
        self.mode.iter().any(|(c, _, _)| *c == card)
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
        defaults::default_mode(self, view, card, choices).unwrap_or(choices[0])
    }

    /// Revealing a blocking Reaction (Moat) to an Attack: always, except against an Attack that can
    /// help the victim. Minion hands a victim with 5+ cards a new hand of 4: declined when that
    /// is worth more than the hand held (the comparison `default_minion` makes). Scrying Pool is
    /// always blocked: its player judges my top card, and judging never helps the one judged.
    fn reveal_to_attack(&self, view: &PlayerView, decision: &Decision) -> Choice {
        decision.source.and_then(|attack| defaults::reveal_to_attack(self, view, decision, attack)).unwrap_or(Choice::Yes)
    }

    // -----------------------------------------------------------------------------------------
    // Topdeck (Harbinger / Bureaucrat victim / Artisan / Sentry order).
    // -----------------------------------------------------------------------------------------

    fn choose_topdeck(&self, _view: &PlayerView, choices: &[Choice], from: Zone, filter: Filter, ordered: bool) -> Choice {
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
    if empty_pile_risk(view) || call_order_risk(view) || urchin_risk(view) || rising_sun_risk(view) {
        return None;
    }
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
        (d.actions > 0, d.actions, d.cards.max(day_draw(c)), d.coins, d.buys, std::cmp::Reverse(c))
    })
}

/// The cards a choice-free Action draws that its definition does not say (the Werewolf draws 3 by day): terminals that draw go first.
fn day_draw(c: CardId) -> u8 {
    if c == id::WEREWOLF { 3 } else { 0 }
}

/// The Night card to play when the order is not a real choice: every Night card in hand is choice-free, so all of them are played in
/// any order (the lowest id first). `None` when one has a decision or reads what the others did (Monastery, Devil's Workshop, Vampire...).
pub fn night_obvious(view: &PlayerView, choices: &[Choice]) -> Option<CardId> {
    if !view.hand().iter().all(|(c, _)| !cards::is(c, cards::NIGHT) || cards::is_choice_free(c)) {
        return None;
    }
    iter_cards(choices).filter(|&c| cards::is_choice_free(c)).min()
}

/// The choice-free +Actions card (Village, Laboratory, Market, Bazaar...) to play before
/// anything else, when the order is otherwise a real choice. Playing it first never hurts unless
/// a card in hand is [`OnPlay::OrderSensitive`](dominion_engine::cards::OnPlay) (Throne Room,
/// Library, Harbinger, City...): it costs no action and only adds cards to choose from. Not when
/// this turn could reshuffle: a card gained or discarded before the shuffle (Workshop's Silver)
/// would be in it, so when the drawing happens matters. That's ruled out when the deck holds
/// more cards than every Action in hand and deck could draw (a choice card counted as drawing
/// 3, for draws picked as a mode: Pawn, Nobles). Most +Actions first, then fewest cards (seeing
/// less before the next choice); `None` if there is no such card or something in hand is
/// order-sensitive. Callers skip it when the strategy states [[play]] rules for the hand.
pub fn cantrip_first(view: &PlayerView, choices: &[Choice]) -> Option<CardId> {
    if empty_pile_risk(view) || call_order_risk(view) || urchin_risk(view) || rising_sun_risk(view) {
        return None;
    }
    let mut could_draw: u32 = 0;
    for (c, n) in view.hand().iter().chain(view.deck().iter()) {
        if !cards::is(c, ACTION) {
            continue;
        }
        if view.hand().has(c) && cards::is_order_sensitive(c) {
            return None;
        }
        let d = cards::def(c);
        let draws = if cards::is_choice_free(c) { d.cards } else { d.cards.max(3) };
        could_draw += draws as u32 * n as u32;
    }
    if could_draw >= view.deck_size() {
        return None;
    }
    iter_cards(choices)
        .filter(|&c| cards::is_choice_free(c) && cards::def(c).actions > 0)
        .max_by_key(|&c| {
            let d = cards::def(c);
            (d.actions, std::cmp::Reverse(d.cards), d.coins, d.buys, std::cmp::Reverse(c))
        })
}

/// Whether an Urchin (Dark Ages) is in hand, in play or in the deck (it may be drawn) while an Attack is in the hand or could be drawn: the Urchin is trashed for a Mercenary when
/// another Attack is played with it in play, so the order of the plays (the Urchin first) is a real choice.
pub fn urchin_risk(view: &PlayerView) -> bool {
    if !view.in_supply(id::URCHIN) || !(view.hand().has(id::URCHIN) || view.in_play().has(id::URCHIN) || view.deck().has(id::URCHIN)) {
        return false;
    }
    view.hand().any_type(cards::ATTACK) || view.deck().any_type(cards::ATTACK)
}

/// Whether a Rising Sun rule makes the play order a real choice: a Shadow card in the deck can be played as well as the Actions of the hand,
/// Enlightenment lets Treasures be played as Actions, and under Flourishing Trade an Action play left over is worth a Buy.
pub fn rising_sun_risk(view: &PlayerView) -> bool {
    !view.shadows_in_deck().is_empty()
        || view.prophecy_in_effect(id::ENLIGHTENMENT)
        || view.prophecy_in_effect(id::FLOURISHING_TRADE)
}

/// Whether the player owns a Royal Carriage (Adventures): which Action it replays depends on the order the Actions are played in (the
/// best card to follow with it is a real choice), so the play order is searched.
pub fn call_order_risk(view: &PlayerView) -> bool {
    view.in_supply(id::ROYAL_CARRIAGE) && view.my_cards().has(id::ROYAL_CARRIAGE)
}

/// Whether a card in hand reads empty piles (City) and another could empty one first: an Attack
/// (it may hand out the last Curses) or a card with a choice (it may gain) while some supply
/// pile is down to 3 cards. Then the play order is a real choice.
pub fn empty_pile_risk(view: &PlayerView) -> bool {
    let hand = view.hand();
    if !hand.iter().any(|(c, _)| cards::reads_empty_piles(c)) {
        return false;
    }
    let attack = hand.iter().any(|(c, _)| cards::is(c, cards::ATTACK));
    let gainer = hand.iter().any(|(c, _)| cards::is(c, ACTION) && !cards::is_choice_free(c));
    attack || (gainer && view.supply_cards().any(|c| view.supply(c) <= 3))
}

/// Whether the bot searches every `DecisionKind::StartOrder` decision (as it does the play order)
/// or only those with a play among the options. The rules' order (cards back into the hand first,
/// then draws, then plays, then discards and trashes) is right in nearly every position: on a
/// Seaside Duration pairing, searching all of them made games 5x slower for under 1 point of win
/// rate, so the default searches only the order with a Prince's card or Clerk in it.
const START_ORDER_SEARCH_ALL: bool = false;

/// Whether a start-of-turn order decision offers a play: Clerk, or a card a Prince plays (any
/// Action that isn't a Duration, which are the effects' own cards).
fn start_order_has_play(choices: &[Choice]) -> bool {
    iter_cards(choices).any(|c| defaults::start_order_play(c) || (cards::is(c, ACTION) && !cards::is(c, cards::DURATION)))
}

/// How good a card is to have played free at the start of every turn: cards, actions, buys and
/// coins it gives, a little for an Attack.
fn prince_card_score(c: CardId) -> i32 {
    let d = cards::def(c);
    let mut s = 3 * d.cards as i32 + 2 * d.coins as i32 + 2 * d.actions as i32 + 2 * d.buys as i32;
    if cards::is(c, cards::ATTACK) {
        s += 3;
    }
    s
}

/// Treasures best played after every other one (Bank, Horn of Plenty: `SetDefaults::plays_last_rank`).
fn plays_last(c: CardId) -> bool {
    defaults::plays_last_rank(c).is_some()
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
fn is_trash_self_investment(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::TrashSelfRevealVpPerTreasureType)
}
fn is_gain_many(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::GainMany(..))
}
fn is_native_village_add(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::NativeVillageAdd)
}
fn is_native_village_take(o: &ModeOpt) -> bool {
    matches!(o, ModeOpt::NativeVillageTake)
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
        Zone::Tavern => view.tavern_of(view.me()),
        Zone::Archive => view.archive(),
        Zone::DeckTop => {
            let mut top = Counts::EMPTY;
            if let Some(c) = view.deck_known_top().next() {
                top.add(c, 1);
            }
            top
        }
        Zone::Supply => {
            let mut c = Counts::EMPTY;
            for card in view.supply_cards() {
                c.set(card, view.supply(card));
            }
            c
        }
    };
    for (c, _) in z.clone().iter() {
        if !filter.matches(c) {
            z.set(c, 0);
        }
    }
    // At least one of each offered card.
    for c in iter_cards(choices) {
        if (c as usize) < NUM_ZONE_IDS && z.get(c) == 0 {
            z.set(c, 1);
        }
    }
    z
}

/// Multi-card selections offer picks in non-decreasing card order (so each set of cards has one
/// path). Decide the whole `wanted` set first, then take its lowest offered card; `Pass` once
/// nothing wanted is offered (or the cheapest offered card if a pick is still required).
fn pick_wanted(wanted: &Counts, choices: &[Choice], ordered: bool, forced: bool) -> Choice {
    let pick = if ordered {
        iter_cards(choices).find(|&c| wanted.has(c))
    } else {
        wanted.iter().map(|(c, _)| c).find(|&c| has_card(choices, c))
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
        // With no stated trash rules, the defaults: Curse, the Dark Ages junk (Ruins, Hovel, Overgrown Estate), Estate, Copper.
        let names: Vec<&str> = s.trash.iter().map(|(c, _)| cards::name(*c)).collect();
        assert_eq!(names.first(), Some(&"Curse"));
        assert_eq!(&names[names.len() - 2..], ["Estate", "Copper"]);
        assert!(names.contains(&"Hovel") && names.contains(&"Survivors"));
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
        // Stated Estate rule first (replacing the default Estate rule), then the other defaults.
        let names: Vec<&str> = s.trash.iter().map(|(c, _)| cards::name(*c)).collect();
        assert_eq!(names[0], "Estate");
        assert_eq!(names[1], "Curse");
        assert_eq!(*names.last().unwrap(), "Copper");
        assert_eq!(names.iter().filter(|&&n| n == "Estate").count(), 1);
    }

    #[test]
    fn rejects_unknown_card() {
        let err = Strategy::parse("name = \"Bad\"\n[[buy]]\ncard = \"Not A Card\"\n").unwrap_err();
        assert!(err.contains("unknown card"), "{err}");
    }
}
