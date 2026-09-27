//! Human-readable scripted strategies.
//!
//! A strategy is a small TOML file: an ordered **buy priority list** (each entry optionally
//! gated by a condition, see `expr.rs`), an ordered **action play priority list**, and a few
//! optional knobs for sub-decisions (Chapel/Sentry trashing, how much treasure to keep in hand).
//! Everything else (Cellar, Militia, Bureaucrat, Remodel, Mine, Throne Room, Bandit, Library,
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

#[derive(Deserialize, Default)]
struct StrategyFile {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    buy: Vec<BuyRuleRaw>,
    /// Action play priority, highest priority first.
    #[serde(default)]
    play: Vec<String>,
    /// Priority order for "junk" cards trashed by Chapel/Sentry/Remodel/the forced-trash
    /// fallback. Defaults to `["Curse", "Estate", "Copper"]`.
    #[serde(default)]
    trash_priority: Option<Vec<String>>,
    /// Chapel/Sentry won't trash a treasure from hand if doing so would leave fewer than this
    /// many treasures in hand. Default 2.
    #[serde(default)]
    keep_treasure: Option<u8>,
}

// -------------------------------------------------------------------------------------------
// Compiled strategy
// -------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct Strategy {
    pub name: String,
    pub description: String,
    buy: Vec<(CardId, Option<Expr>)>,
    /// Rank of each card in the `play` list (lower = earlier), or `-1` if unlisted.
    play_rank: [i16; NUM_CARDS],
    trash_priority: Vec<CardId>,
    keep_treasure: u8,
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

        let mut buy = Vec::with_capacity(raw.buy.len());
        for rule in &raw.buy {
            let card = cards::by_name(&rule.card).ok_or_else(|| format!("unknown card {:?} in buy list", rule.card))?;
            let cond = rule.cond.as_deref().map(Expr::parse).transpose()?;
            buy.push((card, cond));
        }

        let mut play_rank = [-1i16; NUM_CARDS];
        for (i, name) in raw.play.iter().enumerate() {
            let card = cards::by_name(name).ok_or_else(|| format!("unknown card {name:?} in play list"))?;
            if !cards::is(card, ACTION) {
                return Err(format!("{name:?} in play list is not an Action card"));
            }
            play_rank[card as usize] = i as i16;
        }

        let trash_priority = match raw.trash_priority {
            Some(names) => names.iter().map(|n| cards::by_name(n).ok_or_else(|| format!("unknown card {n:?} in trash_priority"))).collect::<Result<Vec<_>, _>>()?,
            None => vec![id::CURSE, id::ESTATE, id::COPPER],
        };

        Ok(Strategy { name: raw.name, description: raw.description, buy, play_rank, trash_priority, keep_treasure: raw.keep_treasure.unwrap_or(2) })
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
        for c in 0..NUM_CARDS as CardId {
            if self.play_rank[c as usize] >= 0 {
                push(c);
            }
        }
        out
    }

    /// Which of `choices` this strategy picks for `decision`. Allocation-free.
    pub fn decide(&self, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Choice {
        match decision.kind {
            DecisionKind::PlayAction => self.best_play(choices).map(Choice::Card).unwrap_or(Choice::Pass),
            DecisionKind::Buy => self.match_buy_list(view, choices).map(Choice::Card).unwrap_or(Choice::Pass),
            DecisionKind::Gain { .. } => self.choose_gain(view, decision, choices),
            DecisionKind::Select { from, act, filter, min, ordered, .. } => {
                let forced = min > 0;
                match act {
                    Act::Discard => self.choose_discard(choices, forced),
                    Act::Trash => self.choose_trash(view, decision, choices, from, forced),
                    Act::Topdeck => self.choose_topdeck(choices, from, filter, ordered),
                    Act::Play => self.best_play(choices).map(Choice::Card).unwrap_or(Choice::Pass),
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

    /// First card in the buy-priority list that's both a legal choice and whose condition (if
    /// any) holds. Used for the top-level Buy decision *and* as the primary policy for Gain
    /// decisions (Workshop/Remodel/Mine/Artisan): "use buy priority capped by cost."
    fn match_buy_list(&self, view: &PlayerView, choices: &[Choice]) -> Option<CardId> {
        for (card, cond) in &self.buy {
            if has_card(choices, *card) && cond.as_ref().map_or(true, |e| e.eval_bool(view)) {
                return Some(*card);
            }
        }
        None
    }

    fn choose_gain(&self, view: &PlayerView, _decision: &Decision, choices: &[Choice]) -> Choice {
        if let Some(c) = self.match_buy_list(view, choices) {
            return Choice::Card(c);
        }
        // Nothing in the buy list is affordable/legal here (e.g. a small Workshop gain that's
        // below everything we listed). Never gain Curse if there's an alternative; otherwise
        // take the most expensive legal option, since a bigger card is rarely a mistake to gain.
        let mut best: Option<CardId> = None;
        for c in iter_cards(choices) {
            let better = match best {
                None => true,
                Some(b) => (c != id::CURSE, cards::cost(c)) > (b != id::CURSE, cards::cost(b)),
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
        let mut score = if terminal { 500 } else { 0 };
        score -= d.actions as i32 * 20; // extra +actions: safely play early
        score -= d.cards as i32 * 5; // draw cards before non-draw cards
        score -= d.buys as i32 * 2;
        score -= d.coins as i32;
        if cards::is(card, cards::ATTACK) {
            score -= 50; // attack opponents before drawing more of our own hand
        }
        UNLISTED + score
    }

    fn best_play(&self, choices: &[Choice]) -> Option<CardId> {
        iter_cards(choices).min_by_key(|&c| (self.play_rank_of(c), c))
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

    fn choose_discard(&self, choices: &[Choice], forced: bool) -> Choice {
        let best = iter_cards(choices).min_by_key(|&c| (Strategy::discard_rank(c), c));
        match best {
            Some(c) if forced || Strategy::discard_rank(c) < 0 => Choice::Card(c),
            _ => Choice::Pass,
        }
    }

    // -----------------------------------------------------------------------------------------
    // Trash (Chapel / Sentry / Remodel / Mine / Moneylender / Bandit victim): junk first,
    // subject to a money floor in hand; special-cased upgrades for Remodel and Mine.
    // -----------------------------------------------------------------------------------------

    fn choose_trash(&self, view: &PlayerView, decision: &Decision, choices: &[Choice], from: Zone, forced: bool) -> Choice {
        // Remodel: trash Gold for a Province once that upgrade is live and we're not down to
        // our last Gold; otherwise junk-first like everything else.
        if decision.source == Some(id::REMODEL) {
            if has_card(choices, id::GOLD)
                && view.supply(id::PROVINCE) > 0
                && cards::cost(id::GOLD) + 2 >= cards::cost(id::PROVINCE)
                && (view.my_cards().get(id::GOLD) >= 2 || view.supply(id::PROVINCE) <= 4)
            {
                return Choice::Card(id::GOLD);
            }
        }
        // Mine: Silver -> Gold, else Copper -> Silver. A plain treasure isn't "junk" so the
        // generic junk-list wouldn't ever pick it.
        if decision.source == Some(id::MINE) {
            if has_card(choices, id::SILVER) {
                return Choice::Card(id::SILVER);
            }
            if has_card(choices, id::COPPER) {
                return Choice::Card(id::COPPER);
            }
        }

        for &jc in &self.trash_priority {
            if !has_card(choices, jc) {
                continue;
            }
            if from == Zone::Hand && cards::is(jc, TREASURE) && !forced {
                let treasures_in_hand = view.hand().count_type(TREASURE);
                if treasures_in_hand <= self.keep_treasure as u32 {
                    continue; // don't dip below our money floor; try the next junk card, if any
                }
            }
            return Choice::Card(jc);
        }
        if forced {
            cheapest_choice(choices)
        } else {
            Choice::Pass
        }
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
    }

    #[test]
    fn rejects_unknown_card() {
        let err = Strategy::parse("name = \"Bad\"\n[[buy]]\ncard = \"Not A Card\"\n").unwrap_err();
        assert!(err.contains("unknown card"), "{err}");
    }
}
