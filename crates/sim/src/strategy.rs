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

/// `[[way]] card = "Smithy" way = "Way of the Sheep" if = "..."`: which Way to play an Action card as (Menagerie); `card = "any"` is any Action card, `way = "none"`
/// plays the card for its own effect. The first rule for the card whose condition holds decides.
#[derive(Deserialize)]
struct WayRuleRaw {
    card: String,
    way: String,
    #[serde(rename = "if")]
    cond: Option<String>,
}

/// `[[favor]] ally = "Cave Dwellers" use = true if = "..."`: whether to spend Favors on the Ally's ability when it is offered (Allies); `ally = "any"` (or no `ally`) is any
/// Ally, `use = false` declines. The first rule for the Ally whose condition holds decides; with none, the Ally's default (`defaults::allies`) does. The condition is read with the
/// card the ability is about (the card gained, for an Ally that acts on a gain) as the context.
#[derive(Deserialize)]
struct FavorRuleRaw {
    #[serde(default)]
    ally: Option<String>,
    #[serde(rename = "use", default = "default_true")]
    spend: bool,
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
    // An Allies split pile named as the pile (`Townsfolk`, `Wizards`...) stands for all four of its cards, top first.
    if let Some(first) = cards::allies_pile_by_name(name) {
        return Some((first..first + 4).collect());
    }
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
    /// Rules for the Way an Action card is played as (Menagerie): the first rule for the card whose condition holds decides.
    #[serde(default)]
    way: Vec<WayRuleRaw>,
    /// Rules for spending Favors on the Ally's ability (Allies): the first rule for the Ally whose condition holds decides.
    #[serde(default)]
    favor: Vec<FavorRuleRaw>,
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

/// What the don't-lose guard says about gaining a card.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Guard {
    Free,
    /// Gaining it ends the game with me not winning.
    Loses,
    /// The second-to-last Province or Colony, which lets an opponent win by taking the last one.
    Penultimate,
}

#[derive(Clone, Debug)]
pub struct Strategy {
    pub name: String,
    pub description: String,
    buy: Vec<(CardId, Option<Expr>)>,
    /// The don't-lose guard (see `Strategy::guard`): whether the gain list skips a gain that would end the game with
    /// this player not winning (the last Province or Colony, the card that empties the game-ending pile), or the
    /// second-to-last Province or Colony when an opponent could then win by taking the last (only for a Victory card
    /// listed after it); with a second buy, a listed card that makes the last Province a win comes first. On unless a
    /// rule's condition uses `wins_game` / `loses_game` itself.
    guard_game_end: bool,
    /// `[[play]]` rules in priority order.
    play: Vec<(CardId, Option<Expr>)>,
    /// Rank of each card's first `[[play]]` rule (lower = earlier), or `-1` if unlisted.
    play_rank: [i16; NUM_CARDS],
    /// `[[trash]]` rules followed by the defaults (`DEFAULT_TRASH`).
    trash: Vec<(CardId, Option<Expr>)>,
    /// How many of the `trash` rules are the strategy's own (the rest are the built-in defaults).
    stated_trash: usize,
    /// `[[mode]]` rules in priority order: (card, index into `cards::modes(card)`, condition).
    mode: Vec<(CardId, u8, Option<Expr>)>,
    /// `[[token]]` rules in priority order: (token index, pile, condition).
    token: Vec<(usize, CardId, Option<Expr>)>,
    /// `[[debt]]` rules in priority order.
    debt: Vec<(DebtPay, Option<Expr>)>,
    /// `[[way]]` rules in priority order: (the Action card, `None` for any; the Way, `None` for the card's own effect; condition).
    way: Vec<(Option<CardId>, Option<CardId>, Option<Expr>)>,
    /// `[[favor]]` rules in priority order: (the Ally, `None` for any; whether to spend; condition).
    favor: Vec<(Option<CardId>, bool, Option<Expr>)>,
    /// `[[call]]` and `[[exchange]]` rules in priority order.
    call: Vec<(CardId, Option<Expr>)>,
    exchange: Vec<(CardId, Option<Expr>)>,
    keep_treasure: u8,
    pub win_this_turn: bool,
    never_gain: Vec<CardId>,
    pub search_play: bool,
    /// With `search_play`: search only the decisions that offer one of these cards or belong to one (selective search, see
    /// `docs/search-in-league.md`); every other decision follows the rules like a fast bot. `None`: search them all.
    search_cards: Option<Vec<bool>>,
    /// Node budget of the second, deeper pass of the turn search (the first pass uses `eval::PLAY_BUDGETS[0]` nodes, or this many if
    /// that is fewer); a value at or below the first budget turns the second pass off. Default `eval::PLAY_BUDGETS[1]`.
    pub search_budget: u64,
    /// Measurement aid: play the defaults as they were before the search-gap work (`docs/search-in-league.md` section 7), so that a new
    /// bot can be played against an old one in the same binary. Off by default; the rule-based bots never set it.
    pub legacy_defaults: bool,
}

impl Strategy {
    /// Restrict the turn searches (`search_play`) to decisions involving the cards of `mask` (indexed by card id). Decisions that
    /// involve none of them are decided by the rules, as in a bot with `search_play` off.
    pub fn set_search_cards(&mut self, mask: Vec<bool>) {
        self.search_cards = Some(mask);
    }

    /// Whether this decision is searched: `search_play`, and under selective search an offered card or the deciding card is in the mask
    /// (a Debt payment counts as Change's).
    pub(crate) fn searches(&self, decision: &Decision, choices: &[Choice]) -> bool {
        if !self.search_play {
            return false;
        }
        let Some(m) = &self.search_cards else { return true };
        let on = |c: CardId| m.get(c as usize).copied().unwrap_or(false);
        decision.source.is_some_and(on) || iter_cards(choices).any(on) || (on(id::CHANGE) && choices.iter().any(|c| matches!(c, Choice::PayDebt | Choice::PayDebtKeep(_))))
    }

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
        let stated_trash = trash.len();
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
        let mut way = Vec::with_capacity(raw.way.len());
        for rule in &raw.way {
            let card = if rule.card.trim().eq_ignore_ascii_case("any") {
                None
            } else {
                Some(
                    cards::by_name(&rule.card)
                        .filter(|&c| cards::is(c, ACTION))
                        .ok_or_else(|| format!("{:?} in a [[way]] rule is not an Action card (or \"any\")", rule.card))?,
                )
            };
            let w = if rule.way.trim().eq_ignore_ascii_case("none") {
                None
            } else {
                Some(cards::by_name(&rule.way).filter(|&w| cards::is_way(w)).ok_or_else(|| format!("{:?} in a [[way]] rule is not a Way (or \"none\")", rule.way))?)
            };
            way.push((card, w, rule.cond.as_deref().map(Expr::parse).transpose()?));
        }
        let mut favor = Vec::with_capacity(raw.favor.len());
        for rule in &raw.favor {
            let ally = match rule.ally.as_deref().map(str::trim) {
                None | Some("") => None,
                Some(a) if a.eq_ignore_ascii_case("any") => None,
                Some(a) => Some(cards::by_name(a).filter(|&c| cards::is_ally(c)).ok_or_else(|| format!("{a:?} in a [[favor]] rule is not an Ally (or \"any\")"))?),
            };
            favor.push((ally, rule.spend, rule.cond.as_deref().map(Expr::parse).transpose()?));
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
            way,
            favor,
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
            search_cards: None,
            search_budget: crate::eval::PLAY_BUDGETS[1],
            legacy_defaults: false,
            stated_trash,
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
            } else if cards::is_allies_split(c) {
                // A card of an Allies split pile asks for the pile (named by its first card).
                cards::pile_of(c)
            } else if cards::is_castle(c) {
                id::CASTLES
            } else if c >= cards::FIRST_TRAV && ((c - cards::FIRST_TRAV) as usize) < cards::NUM_TRAV {
                // A Traveller upgrade comes with the first card of its line.
                if c < cards::id::SOLDIER { id::PAGE } else { id::PEASANT }
            } else {
                c
            };
            let kingdom = cards::is_kingdom(c) || cards::is_event(c) || cards::is_landmark(c) || cards::is_prophecy(c) || cards::is_project(c) || cards::is_way(c) || cards::is_ally(c);
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
        // The Ways the `[[way]]` rules name are asked for like the cards (the Action cards they name are not: they are not what a kingdom is built on).
        for (_, w, cond) in &self.way {
            if let Some(w) = w {
                push(*w);
            }
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
        let searching = self.searches(decision, choices);
        // Stated rules come before any search.
        if let Some(c) = self.stated_effect_choice(view, decision, choices) {
            return c;
        }
        // Menagerie: which Way to play an Action as is searched only when the numbers of the fast default are close (a stated rule decided above).
        if searching && decision.kind == DecisionKind::Way && decision.player == view.me() && view.is_my_turn() && choices.len() > 1 && defaults::way_is_close(self, view, decision, choices) {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // Debt: pay it as soon as there is $ for it. In a game with Change, whether to keep a token (Change gives +$3 with Debt) is searched.
        if searching
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
        if searching && view.is_my_turn() && crate::eval::is_treasure_effect_decision(view.me(), decision) && choices.len() > 1 {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // Calls that shape the turn (Duplicate on a gain, Coin of the Realm, Wine Merchant's discard) are searched like the
        // play order, unless a `[[call]]` rule has spoken (above).
        if searching && view.is_my_turn() && crate::eval::is_call_decision(view.me(), decision) && choices.len() > 1 {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // How much to overpay shapes the turn too (what it buys is scored by the gain list).
        if searching && view.is_my_turn() && crate::eval::is_overpay_decision(view.me(), decision) && choices.len() > 1 {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        // Artificer's discards: the candidate sets are searched against the gain it buys (see `eval::search_artificer_choice`).
        if searching
            && decision.source == Some(id::ARTIFICER)
            && decision.player == view.me()
            && view.is_my_turn()
            && matches!(decision.kind, DecisionKind::Select { act: Act::Discard, from: Zone::Hand, .. })
            && choices.len() > 1
        {
            if let Some(c) = crate::eval::search_artificer_choice(self, view, choices) {
                return c;
            }
        }
        // The order of the start-of-turn effects, and Prince's set-aside choices, are searched too:
        // they decide what the turn starts with. Orders of draws, returns and discards are left to
        // the rules (see `START_ORDER_SEARCH_ALL`); only an order with a play in it (Prince, Clerk)
        // is a real trade-off.
        if decision.kind == DecisionKind::StartOrder
            && searching
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

    /// The Buy decision from a test on a card, when the state is a plain Buy phase (`GameState::buy_avail`): what `decide` returns for the list of the cards
    /// `avail` has and Pass, without building it. There the only rule that can speak before the gain list is the lookahead of `win_this_turn` (the searched
    /// decisions of `decide` all need a Debt payment, a Coffers token or an Event among the choices): `None` when it applies.
    pub fn decide_buy(&self, view: &PlayerView, avail: &dominion_engine::BuyAvail) -> Option<Choice> {
        if self.win_this_turn && Self::game_could_end_soon(view) {
            return None;
        }
        Some(self.match_gain_list(view, avail).map(Choice::Card).unwrap_or(Choice::Pass))
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
                    Act::Tavern | Act::Call | Act::Exchange | Act::Return | Act::Archive | Act::ShuffleIn | Act::Choose | Act::Exile => {
                        iter_cards(choices).next().map(Choice::Card).unwrap_or(Choice::Pass)
                    }
                    // Sibyl's card for the bottom of the deck (the set's own policy chose it above): the most valuable, which stays in the deck.
                    Act::Bottom => iter_cards(choices).max_by_key(|&c| (Strategy::card_value(c), c)).map(Choice::Card).unwrap_or(Choice::Pass),
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
            DecisionKind::Way => self.stated_way(view, decision, choices).unwrap_or_else(|| defaults::way_choice(self, view, decision, choices)),
        }
    }

    /// The choice a stated `[[way]]` rule makes for a Way decision (`decision.subject` is the Action card played), if one applies: the Way, or `Pass` for the card's
    /// own effect. A rule naming a Way that is not in the game is skipped.
    pub fn stated_way(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        let card = decision.subject;
        self.way.iter().find_map(|(c, w, cond)| {
            if c.is_some_and(|c| c != card) || !cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)) {
                return None;
            }
            match w {
                None => Some(Choice::Pass),
                Some(w) if choices.contains(&Choice::Card(*w)) => Some(Choice::Card(*w)),
                Some(_) => None,
            }
        })
    }

    /// The answer a stated `[[favor]]` rule gives to the Ally decision `decision` (an offer to spend Favors on the Ally's ability): `Some(yes)` when a rule for the Ally holds.
    pub fn stated_favor(&self, view: &PlayerView, decision: &Decision) -> Option<bool> {
        let ally = decision.source.filter(|&a| cards::is_ally(a))?;
        let card = decision.subject;
        self.favor.iter().find_map(|(a, spend, cond)| {
            if a.is_some_and(|a| a != ally) || !cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)) {
                return None;
            }
            Some(*spend)
        })
    }

    /// Whether any `[[favor]]` rule exists.
    pub fn states_favor(&self) -> bool {
        !self.favor.is_empty()
    }

    /// Whether the Way decision is close enough that a searching bot searches it (no stated rule applies).
    pub fn way_close(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> bool {
        self.stated_way(view, decision, choices).is_none() && defaults::way_is_close(self, view, decision, choices)
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
        // Menagerie: Animal Fair bought by trashing an Action card, when the gain list wants it more than what the $ buy and the hand holds an Action it does not play.
        let best = self.match_gain_list(view, choices);
        if let Some(a) = self.alt_buy(view, choices, best) {
            return a;
        }
        best.map(Choice::Card).unwrap_or(Choice::Pass)
    }

    /// The alternative way to buy a card (`Choice::Alt`, Animal Fair: trash an Action from the hand) when the gain list ranks the card above `best`, the card the $ would
    /// buy, and the hand holds an Action card the strategy has no use for.
    fn alt_buy(&self, view: &PlayerView, choices: &[Choice], best: Option<CardId>) -> Option<Choice> {
        if !choices.iter().any(|c| matches!(c, Choice::Alt(_))) {
            return None;
        }
        let rank_of = |c: CardId| self.buy.iter().position(|(x, cond)| *x == c && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, c)));
        let best_rank = best.and_then(rank_of).unwrap_or(usize::MAX);
        let junk = view.hand().iter().any(|(c, _)| view.is_action(c) && (c == id::HORSE || cards::is(c, cards::RUINS) || !(self.states_play(c) || self.lists(c))));
        if !junk {
            return None;
        }
        choices
            .iter()
            .filter_map(|ch| match ch {
                Choice::Alt(c) => rank_of(*c).filter(|&r| r < best_rank).map(|r| (r, *ch)),
                _ => None,
            })
            .min_by_key(|&(r, _)| r)
            .map(|(_, ch)| ch)
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
        self.reachable_rank_with(view, money, potions).map(|r| self.buy[r].0)
    }

    /// The position in the gain list of the first entry that `money` and `potions` reach.
    pub(crate) fn reachable_rank_with(&self, view: &PlayerView, money: u32, potions: u8) -> Option<usize> {
        self.buy.iter().position(|(card, cond)| {
            (if cards::is_event(*card) {
                view.event_available(*card) && view.event_cost(*card) as u32 <= money && !self.useless_buy(view, *card)
            } else if cards::is_project(*card) {
                view.project_open(*card) && cards::cost(*card) as u32 <= money
            } else {
                view.in_supply(*card)
                    && view.supply(*card) > 0
                    && view.cost(*card) as u32 <= money
                    && (potions > 0 || !cards::potion_cost(*card))
                    && view.may_buy(*card)
            }) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card))
        })
    }

    /// What the gain-list entry at position `rank` is worth on the scale of `GainListEvaluator`: the last entry 1000, each entry up ten times
    /// more, so one entry outranks everything below it together.
    pub fn rank_weight(&self, rank: usize) -> f64 {
        1000.0 * 10f64.powi((self.buy.len() - 1 - rank) as i32)
    }

    /// What one VP token is worth on the scale of `rank_weight`: the cheapest VP the gain list buys now, an allowed Victory card's weight per VP
    /// (an Estate's, once it buys Estates; a Duchy's third before that), so VP are worth as much as the strategy rates VP: nothing while it lists no
    /// Victory card (Provinces and Colonies do not count: a token is not a sixth of one). Linear in the tokens, so that the order of two plays that give tokens never matters by itself.
    pub fn vp_token_weight(&self, view: &PlayerView) -> f64 {
        let mut best: Option<f64> = None;
        for (c, _) in &self.buy {
            let vp = cards::def(*c).vp;
            if (1..=3).contains(&vp) && cards::is(*c, VICTORY) {
                if let Some(rank) = self.gain_rank(view, *c) {
                    let per_vp = self.rank_weight(rank) / vp as f64;
                    best = Some(best.map_or(per_vp, |b| b.min(per_vp)));
                }
            }
        }
        best.unwrap_or(0.0)
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

    // -----------------------------------------------------------------------------------------
    // What a card or a coin is worth to the gain list, on the scale `eval::GainListEvaluator` uses (the turn search's own scoring). The
    // fast bot uses it to see, without a search, when a card that must trash something is not worth playing.
    // -----------------------------------------------------------------------------------------

    /// The weight of `c` in the gain list as it stands now (0: the strategy would not gain it).
    pub(crate) fn weight_of(&self, view: &PlayerView, c: CardId) -> f64 {
        self.gain_rank(view, c).map_or(0.0, |r| self.rank_weight(r))
    }

    /// The weight of the best gain costing up to `max_cost` (exactly, if `exact`) that matches `filter`.
    pub(crate) fn gain_weight(&self, view: &PlayerView, max_cost: u8, filter: Filter, exact: bool, potion: bool) -> f64 {
        self.best_gain_rank(view, max_cost, filter, exact, potion).map_or(0.0, |r| self.rank_weight(r))
    }

    /// The weight of what `money` buys by the gain list (the single best buy).
    pub(crate) fn buy_weight(&self, view: &PlayerView, money: u32) -> f64 {
        self.reachable_rank_with(view, money, view.turn().potions).map_or(0.0, |r| self.rank_weight(r))
    }

    /// The $ this turn's buy has: the $ already made plus the Treasures in the hand.
    pub(crate) fn money_in_hand(view: &PlayerView) -> u32 {
        view.turn().coins as u32 + view.hand().iter().filter(|&(c, _)| view.is_treasure(c)).map(|(c, n)| cards::def(c).coins as u32 * n as u32).sum::<u32>()
    }

    /// What losing `c` from the hand costs this turn: its own weight, plus the buy its coins would have paid for.
    /// `extra` is the $ the trashing itself brings (Forager).
    pub(crate) fn trash_cost(&self, view: &PlayerView, c: CardId, money: u32, extra: u32) -> f64 {
        let coins = if view.is_treasure(c) { cards::def(c).coins as u32 } else { 0 };
        let lost = if coins > 0 || extra > 0 { self.buy_weight(view, money) - self.buy_weight(view, (money + extra).saturating_sub(coins)) } else { 0.0 };
        self.weight_of(view, c) + lost
    }

    /// The net value of using a "trash a card" effect worth `gain(card)` on `c` (on the weight scale), with the turn search's tie-break of
    /// 0.3 for a card the trash rules want gone and -0.3 for one they do not.
    pub(crate) fn trash_value(&self, view: &PlayerView, c: CardId, money: u32, gain: f64, extra: u32) -> f64 {
        gain - self.trash_cost(view, c, money, extra) + if self.wants_trash(view, c) { 0.3 } else { -0.3 }
    }

    /// The best of `candidates` to use a "trash a card" effect on, worth `gain(card)` each: the card and the net value
    /// ([`Strategy::trash_value`]); the cheaper card first among equals.
    pub(crate) fn best_trash_among(&self, view: &PlayerView, candidates: impl Iterator<Item = CardId>, gain: impl Fn(&Self, CardId) -> f64) -> Option<(CardId, f64)> {
        self.best_trash_with_coins(view, candidates, gain, |_, _| 0)
    }

    /// [`Strategy::best_trash_among`] for an effect that also brings `extra(card)` $ (Forager).
    pub(crate) fn best_trash_with_coins(
        &self, view: &PlayerView, candidates: impl Iterator<Item = CardId>, gain: impl Fn(&Self, CardId) -> f64, extra: impl Fn(&Self, CardId) -> u32,
    ) -> Option<(CardId, f64)> {
        let money = Self::money_in_hand(view);
        let mut best: Option<(f64, std::cmp::Reverse<i32>, std::cmp::Reverse<u8>, std::cmp::Reverse<CardId>)> = None;
        for c in candidates {
            // Ties: the card worth least in a deck (a Curse before a Copper), then the cheaper, then the lower id.
            let key = (self.trash_value(view, c, money, gain(self, c), extra(self, c)), std::cmp::Reverse(Self::card_value(c)), std::cmp::Reverse(cards::cost(c)), std::cmp::Reverse(c));
            if best.map_or(true, |b| key > b) {
                best = Some(key);
            }
        }
        best.map(|(v, _, _, c)| (c.0, v))
    }

    /// [`Strategy::best_trash_among`] the cards of the hand, for an effect that is about to be played: one copy of `source`, the card
    /// whose effect it is and which is still in the hand, is no candidate.
    pub(crate) fn best_hand_trash(&self, view: &PlayerView, source: CardId, gain: impl Fn(&Self, CardId) -> f64) -> Option<(CardId, f64)> {
        self.best_trash_among(view, view.hand().iter().filter(|&(c, n)| c != source || n >= 2).map(|(c, _)| c), gain)
    }

    /// Whether the Action `card` can only do harm played now (SetDefaults::useless_play, for cards that must trash something and
    /// have nothing worth trashing). Applies to stated `[[play]]` rules too: a rule says which card to play first, not to play it at a loss.
    pub(crate) fn useless_play(&self, view: &PlayerView, card: CardId) -> bool {
        !self.legacy_defaults && defaults::useless_play(self, view, card)
    }

    /// Whether buying the Event `card` now can only do harm (SetDefaults::useless_buy: Kintsugi with nothing in the hand worth trashing).
    pub(crate) fn useless_buy(&self, view: &PlayerView, card: CardId) -> bool {
        !self.legacy_defaults && defaults::useless_buy(self, view, card)
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

    /// First entry in the gain list that's a legal choice and whose condition holds, skipping the gains the don't-lose guard (`Strategy::guard`)
    /// advises against. The guard's cheap precheck (`guard_may_apply`) is the only extra work in the common case.
    fn match_gain_list<C: Offered + ?Sized>(&self, view: &PlayerView, choices: &C) -> Option<CardId> {
        for (card, cond) in &self.buy {
            if choices.offers(*card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)) && !(cards::is_event(*card) && self.useless_buy(view, *card)) {
                if self.guard_game_end && Self::guard_may_apply(view, *card) {
                    return self.match_gain_list_guarded(view, choices);
                }
                return Some(*card);
            }
        }
        None
    }

    /// `match_gain_list` when the entry it reaches is one the guard may object to (the end of the game is near: rare).
    #[cold]
    #[inline(never)]
    fn match_gain_list_guarded<C: Offered + ?Sized>(&self, view: &PlayerView, choices: &C) -> Option<CardId> {
        // The second-to-last Province (or Colony) that the guard would rather not take: passed over for a Victory card that follows it in the
        // list (a Duchy), taken after all when the next card is not worth points or nothing else is wanted.
        let mut penultimate: Option<CardId> = None;
        for (card, cond) in &self.buy {
            if choices.offers(*card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)) && !(cards::is_event(*card) && self.useless_buy(view, *card)) {
                match Self::guard(view, *card) {
                    Guard::Free => {
                        return match penultimate {
                            // Declining the Province only helps when the alternative scores: a bot that gains Gold instead stalls, and the
                            // opponent in front keeps its lead (measured: Big Money, Double Witch and Smithy lose win rate otherwise).
                            Some(p) if !cards::is(*card, VICTORY) => Some(p),
                            _ => Some(*card),
                        };
                    }
                    Guard::Penultimate => {
                        penultimate.get_or_insert(*card);
                    }
                    // The last Province or Colony loses on its own: with a second buy a listed card first may turn it into a win.
                    Guard::Loses => {
                        if let Some(first) = self.gain_to_set_up_last(view, choices, *card) {
                            return Some(first);
                        }
                    }
                }
            }
        }
        penultimate
    }

    /// Whether the guard could have a say about gaining `card`: it is a Province or Colony with at most two left, or the last card of a pile.
    #[inline(always)]
    fn guard_may_apply(view: &PlayerView, card: CardId) -> bool {
        if card == id::PROVINCE || card == id::COLONY { view.supply(card) <= 2 } else { view.is_last_of_pile(card) }
    }

    /// The don't-lose guard (on unless a gain rule uses `wins_game` / `loses_game` itself): gaining `card` would end the game this turn with me not
    /// among the winners (a shared win is fine), or, for the second-to-last Province or Colony, would let an opponent win by taking the last one
    /// on their next turn. The ending gains: the last Province or Colony; the last card of a pile when that is the third empty pile (two empty,
    /// in 2-4 players) or the pile limit's last. Any other gain costs one or two comparisons.
    #[inline]
    fn guard(view: &PlayerView, card: CardId) -> Guard {
        let me_loses = |w: Option<u8>| matches!(w, Some(w) if w & (1 << view.me()) == 0);
        if card == id::PROVINCE || card == id::COLONY {
            match view.supply(card) {
                1 if me_loses(view.result_if_gained(card)) => Guard::Loses,
                // A second buy at the price of two takes the last one too: that is the last-copy case when the bot gets there.
                2 if !(view.buys_left() >= 2 && view.turn().coins as u32 >= 2 * view.cost(card) as u32) && view.leaves_last_card_to_opponent(card) => Guard::Penultimate,
                _ => Guard::Free,
            }
        } else if view.is_last_of_pile(card) && me_loses(view.result_if_gained(card)) {
            Guard::Loses
        } else {
            Guard::Free
        }
    }

    /// With the last Province or Colony (`last`) losing on its own and a second buy to spend: the first card of the gain list that can be bought
    /// together with it (the $ and Buys allow both) and after which gaining `last` does not lose. Only in that rare endgame position.
    fn gain_to_set_up_last<C: Offered + ?Sized>(&self, view: &PlayerView, choices: &C, last: CardId) -> Option<CardId> {
        if !self.guard_game_end
            || !(last == id::PROVINCE || last == id::COLONY)
            || view.phase() != dominion_engine::Phase::Buy
            || view.buys_left() < 2
            || view.supply(last) != 1
        {
            return None;
        }
        let money = view.turn().coins as u32 + view.coffers() as u32;
        let me = view.me();
        self.buy.iter().find_map(|(x, cond)| {
            (*x != last
                && !cards::is_event(*x)
                && !cards::is_project(*x)
                && choices.offers(*x)
                && cards::debt_cost(*x) == 0
                && !cards::potion_cost(*x)
                && view.cost(*x) as u32 + view.cost(last) as u32 <= money
                && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *x))
                && matches!(view.result_if_gained_after(*x, last), Some(w) if w & (1 << me) != 0))
            .then_some(*x)
        })
    }

    /// Whether the strategy's rules would ever buy `card` here: it's in the gain list with its
    /// condition true.
    pub fn allows_buy(&self, view: &PlayerView, card: CardId) -> bool {
        self.gain_rank(view, card).is_some()
    }

    fn choose_gain(&self, view: &PlayerView, _decision: &Decision, choices: &[Choice]) -> Choice {
        // Menagerie: a card Exiled from the Supply is not one the gain list wants to gain.
        if let Some(c) = _decision.source.and_then(|src| defaults::gain(self, view, _decision, src, choices)) {
            return c;
        }
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
            if has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, *card)) && (for_multiplier || !self.useless_play(view, *card)) {
                return Some(*card);
            }
        }
        iter_cards(choices)
            .filter(|&c| cards::def(c).plays == 0 || multiplier_ok)
            .filter(|&c| for_multiplier || (self.default_worth_playing(view, c) && !self.useless_play(view, c)))
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
        if let Some(c) = self.villager_choice(view, decision, choices) {
            return c;
        }
        let searching = self.searches(decision, choices);
        // A stated `[[debt]] pay = "all"` rule pays the Debt off before a Change is played (so that it remodels).
        if matches!(decision.kind, DecisionKind::PlayAction) && choices.contains(&Choice::PayDebt) && self.stated_debt_choice(view, choices) == Some(Choice::PayDebt) {
            return Choice::PayDebt;
        }
        // No real choice (every action choice-free, all playable): the obvious order, no search.
        if searching && matches!(decision.kind, DecisionKind::PlayAction) && decision.player == view.me() {
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
            && iter_cards(choices).all(|c| !cards::is_choice_free(c) && !cards::safe_alone(c));
        if searching && (cards_offered >= 2 || lone_risky) && decision.player == view.me() {
            if let Some(c) = crate::eval::search_play_choice(self, view) {
                if choices.contains(&c) {
                    return c;
                }
            }
        }
        if let Some(c) = self.best_play(view, choices, for_multiplier) {
            return Choice::Card(c);
        }
        // A play that can't be declined (Orb: an Action or Treasure from the discard pile) with no Action worth playing: the Treasure worth the most.
        if !choices.contains(&Choice::Pass) {
            if let Some(c) = iter_cards(choices).max_by_key(|&c| (cards::def(c).coins, std::cmp::Reverse(c))) {
                return Choice::Card(c);
            }
        }
        Choice::Pass
    }

    /// The Action phase with no Action left (the only choices are a Villager and Pass): spend a Villager when the hand holds an Action worth playing
    /// (a Villager is +1 Action, banked until it buys a play), else keep them. `None` for any other decision.
    fn villager_choice(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        if decision.kind != DecisionKind::PlayAction || !choices.contains(&Choice::Villager) {
            return None;
        }
        let worth = view.hand().iter().any(|(c, _)| {
            cards::is(c, ACTION) && (self.play.iter().any(|(x, cond)| *x == c && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, c))) || (self.default_worth_playing(view, c) && !self.useless_play(view, c)))
        });
        Some(if worth { Choice::Villager } else { Choice::Pass })
    }

    /// The play choice by rule order alone (stated [[play]] rules, then the default order), without
    /// searching. Used in search playouts.
    pub fn rule_play(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        if let Some(c) = self.villager_choice(view, decision, choices) {
            return c;
        }
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
        card == id::ROCKS
            || self
                .trash
                .iter()
                .enumerate()
                .any(|(i, (c, cond))| *c == card && cond.as_ref().map_or(true, |e| e.eval_bool_for(view, card)) && !self.default_trash_off(i, view, card))
    }

    /// The built-in rule that trashes Estates is off while the strategy itself gains Estates (an endgame rule of the gain list): they are
    /// points then, not junk.
    fn default_trash_off(&self, i: usize, view: &PlayerView, card: CardId) -> bool {
        !self.legacy_defaults && i >= self.stated_trash && card == id::ESTATE && self.gain_rank(view, id::ESTATE).is_some()
    }

    // -----------------------------------------------------------------------------------------
    // Discard (Cellar / Militia / Poacher victim): worst card first.
    // -----------------------------------------------------------------------------------------

    /// Lower = more worth discarding. Curse and Victory cards are dead weight in hand during
    /// the action phase; beyond that, discard the cheapest card first.
    fn discard_rank(card: CardId) -> i32 {
        if card == id::CURSE {
            -1000
        } else if cards::is(card, VICTORY) && !cards::is(card, ACTION) && !cards::is(card, TREASURE) {
            -500
        } else if cards::is(card, cards::RUINS) || card == id::HOVEL {
            -400
        } else if cards::is(card, VICTORY) && cards::is(card, cards::RESERVE) {
            // A Reserve card that is also a Victory card (Distant Lands) is worth its points once it is played onto the Tavern mat:
            // far more to keep than a Copper, and no Gold either.
            cards::cost(card) as i32 + 4
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
        if let Some(up) = decision.upgrade.filter(|_| !self.legacy_defaults) {
            // What the turn search would pick: the card whose upgrade gains most over what losing it costs (a Treasure's coins count).
            let gain = |s: &Strategy, c: CardId| s.gain_weight(view, view.cost(c).saturating_add(up.plus), up.filter, up.exact, cards::potion_cost(c));
            if let Some((c, v)) = self.best_trash_among(view, iter_cards(choices), gain) {
                return if forced || v >= 0.0 { Choice::Card(c) } else { Choice::Pass };
            }
        }
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
        for (i, (jc, cond)) in self.trash.iter().enumerate() {
            let jc = *jc;
            if left == 0 {
                break;
            }
            if !cond.as_ref().map_or(true, |e| e.eval_bool_for(view, jc)) || self.default_trash_off(i, view, jc) {
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
            // The card that costs the least to lose (its place in the gain list, and the buy its coins pay for; a Copper or an Estate
            // before a Silver, a Silver before a Gold), then the cheapest, ties by id, among the zone's cards not yet fully wanted.
            let money = Self::money_in_hand(view);
            while need > 0 {
                let Some((_, _, c)) = zone
                    .iter()
                    .filter(|&(c, n)| n > wanted.get(c))
                    .map(|(c, _)| (if self.legacy_defaults || from != Zone::Hand { 0.0 } else { self.trash_cost(view, c, money, 0) }, cards::cost(c), c))
                    .min_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))))
                else {
                    break;
                };
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
        let searching = self.searches(decision, choices);
        // An Event offered before the Treasures are played (Alms) is searched like the play order.
        let event = iter_cards(choices).find(|&c| cards::is_event(c));
        // Nothing left whose order or choices matter (only plain Treasures, and Bank, which goes
        // last): the rule order, no search needed.
        if event.is_none() && !iter_cards(choices).any(|c| !plays_last(c) && !cards::is_choice_free(c)) {
            return self.rule_play_treasure(choices);
        }
        if searching && decision.player == view.me() {
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
        let searching = self.searches(decision, choices);
        if searching && decision.player == view.me() {
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
        // Crystal Ball looks at the top card and may play it: the plain Treasures are played first, so that the card played from the deck
        // is chosen with the hand's $ known.
        let plain_left = !self.legacy_defaults && iter_cards(choices).any(|c| cards::is_choice_free(c) && !plays_last(c));
        let hand_dependent = |c: CardId| plain_left && c == id::CRYSTAL_BALL;
        if let Some(c) = iter_cards(choices).find(|&c| !plays_last(c) && !cards::is_choice_free(c) && !cards::is_event(c) && !hand_dependent(c)) {
            return Choice::Card(c);
        }
        // Only plain Treasures left (the game page offers them one by one): Done plays them all at once, the same as playing each.
        let all_plain = iter_cards(choices).all(|c| cards::is_choice_free(c) && !plays_last(c));
        if plain_left && !all_plain {
            if let Some(c) = iter_cards(choices).find(|&c| cards::is_choice_free(c) && !plays_last(c)) {
                return Choice::Card(c);
            }
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
        let searching = self.searches(decision, choices);
        let card = decision.source.expect("Mode decision always has a source card");
        // Stated `[[mode]]` rules always come first.
        if let Some(c) = self.stated_mode_rule(view, card, choices) {
            return c;
        }
        // The turn player's own modes shape the turn like a play decision: search it with the
        // strategy's own scoring, exactly like `play_decision`. Never applies to a reactive mode
        // decision during someone else's turn (Torturer's victim), which always uses the fast
        // heuristic below regardless of `search_play`.
        if searching && decision.player == view.me() && view.is_my_turn() {
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
            DecisionKind::Way => self.stated_way(view, decision, choices),
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

/// What a decision offers, asked one card at a time: its list of choices, or (for a plain Buy decision) the engine's test on a card, which saves
/// building the list. The gain list is matched against either.
pub(crate) trait Offered {
    fn offers(&self, c: CardId) -> bool;
}

impl Offered for [Choice] {
    #[inline(always)]
    fn offers(&self, c: CardId) -> bool {
        has_card(self, c)
    }
}

impl Offered for dominion_engine::BuyAvail<'_> {
    #[inline(always)]
    fn offers(&self, c: CardId) -> bool {
        self.has(c)
    }
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
        Zone::Exile => view.exile(),
        // (The starting deck and the cards set aside in a Plunder holder: only the cards on offer are known.)
        Zone::StartDeck | Zone::PluAside => Counts::EMPTY,
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
