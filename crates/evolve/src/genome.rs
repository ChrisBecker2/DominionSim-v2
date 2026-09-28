//! The genome: a strategy file as data. Conditions are conjunctions of simple, readable atoms
//! (`count(Moat) < 2`, `provinces_left <= 4`, `money >= 12`, ...), a subset of the strategy
//! condition language chosen so every mutation still reads naturally.

use std::fmt::Write as _;

use dominion_engine::cards::{self, CardId};
use serde::Deserialize;

/// A quantity a condition can test.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Var {
    /// Copies of a card I own.
    Count(CardId),
    /// Action cards I own.
    CountActions,
    ProvincesLeft,
    /// Total coin value of my treasures.
    Money,
    /// My VP minus the best opponent's.
    VpLead,
    /// My turn number.
    MyTurn,
    /// Cards I own.
    TotalCards,
    EmptyPiles,
}

/// `Lt` prints as `<` (`<=` for `provinces_left`/`empty_piles`, which read better that way);
/// `Ge` prints as `>=`. Any `<=`, `>` in parsed files is normalized onto these two.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Op {
    Lt,
    Ge,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Atom {
    pub var: Var,
    pub op: Op,
    pub n: i16,
}

/// One rule: the card, plus a condition that is the `and` of its atoms (empty = always).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Rule {
    pub card: CardId,
    pub conds: Vec<Atom>,
}

/// Which rule list a rule belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum List {
    Gain,
    Play,
    Trash,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Genome {
    pub gain: Vec<Rule>,
    pub play: Vec<Rule>,
    /// Stated trash rules; the engine appends its defaults (Curse, Estate, Copper) after them.
    pub trash: Vec<Rule>,
    pub keep_treasure: u8,
}

/// Text written into a genome's TOML besides the rules themselves.
#[derive(Default, Clone)]
pub struct TomlMeta {
    pub name: String,
    pub description: String,
    /// Comment lines at the top of the file (without the leading `# `).
    pub header: Vec<String>,
    /// Cards the strategy must never gain (written as `never_gain`).
    pub never_gain: Vec<CardId>,
    /// Comment written above a rule, e.g. its measured impact.
    pub notes: Vec<(List, usize, String)>,
}

impl Var {
    /// Sensible range for thresholds of this variable.
    pub fn range(self) -> (i16, i16) {
        match self {
            Var::Count(_) => (1, 12),
            Var::CountActions => (1, 15),
            Var::ProvincesLeft => (1, 8),
            Var::Money => (3, 40),
            Var::VpLead => (-20, 20),
            Var::MyTurn => (2, 30),
            Var::TotalCards => (8, 50),
            Var::EmptyPiles => (1, 2),
        }
    }

    /// Step used when nudging a threshold.
    pub fn step(self) -> i16 {
        match self {
            Var::Money | Var::TotalCards | Var::VpLead => 2,
            _ => 1,
        }
    }

    fn text(self) -> String {
        match self {
            Var::Count(c) => format!("count({})", cards::name(c).replace(' ', "")),
            Var::CountActions => "count_type(action)".into(),
            Var::ProvincesLeft => "provinces_left".into(),
            Var::Money => "money".into(),
            Var::VpLead => "vp_lead".into(),
            Var::MyTurn => "my_turn".into(),
            Var::TotalCards => "total_cards".into(),
            Var::EmptyPiles => "empty_piles".into(),
        }
    }

    fn parse(s: &str) -> Option<Var> {
        let s = s.trim();
        let lower = s.to_ascii_lowercase();
        if let Some(inner) = lower.strip_prefix("count(").and_then(|r| r.strip_suffix(')')) {
            let inner_orig = &s[6..s.len() - 1];
            return cards::by_name(inner_orig).map(Var::Count).or_else(|| (inner.trim() == "action").then_some(Var::CountActions));
        }
        if lower.replace(' ', "") == "count_type(action)" {
            return Some(Var::CountActions);
        }
        Some(match lower.as_str() {
            "provinces_left" | "provincesleft" => Var::ProvincesLeft,
            "money" => Var::Money,
            "vp_lead" | "vplead" => Var::VpLead,
            "my_turn" | "myturn" => Var::MyTurn,
            "total_cards" | "totalcards" => Var::TotalCards,
            "empty_piles" | "emptypiles" => Var::EmptyPiles,
            _ => return None,
        })
    }

    /// Whether `x <= n` reads better than `x < n+1`.
    fn prefers_le(self) -> bool {
        matches!(self, Var::ProvincesLeft | Var::EmptyPiles)
    }
}

impl Atom {
    pub fn text(&self) -> String {
        match self.op {
            Op::Lt if self.var.prefers_le() => format!("{} <= {}", self.var.text(), self.n - 1),
            Op::Lt => format!("{} < {}", self.var.text(), self.n),
            Op::Ge => format!("{} >= {}", self.var.text(), self.n),
        }
    }

    /// Parse one comparison of the template language (`lhs op number`).
    pub fn parse(s: &str) -> Result<Atom, String> {
        for (tok, len) in [("<=", 2), (">=", 2), ("<", 1), (">", 1)] {
            if let Some(i) = s.find(tok) {
                let var = Var::parse(&s[..i]).ok_or_else(|| format!("unsupported condition term {:?}", s[..i].trim()))?;
                let n: i16 = s[i + len..].trim().parse().map_err(|_| format!("expected a number in {s:?}"))?;
                let (op, n) = match tok {
                    "<" => (Op::Lt, n),
                    "<=" => (Op::Lt, n + 1),
                    ">=" => (Op::Ge, n),
                    _ => (Op::Ge, n + 1),
                };
                return Ok(Atom { var, op, n });
            }
        }
        Err(format!("unsupported condition {s:?} (expected e.g. `count(Moat) < 2`)"))
    }
}

impl Rule {
    pub fn new(card: CardId) -> Rule {
        Rule { card, conds: Vec::new() }
    }

    pub fn cond_text(&self) -> Option<String> {
        (!self.conds.is_empty()).then(|| self.conds.iter().map(Atom::text).collect::<Vec<_>>().join(" and "))
    }

    fn parse(card: &str, cond: Option<&str>) -> Result<Rule, String> {
        let card = cards::by_name(card).ok_or_else(|| format!("unknown card {card:?}"))?;
        let mut conds = Vec::new();
        if let Some(c) = cond {
            for part in split_and(c) {
                conds.push(Atom::parse(part)?);
            }
        }
        Ok(Rule { card, conds })
    }
}

fn split_and(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let lower = s.to_ascii_lowercase();
    let mut start = 0;
    while let Some(i) = lower[start..].find(" and ") {
        out.push(&s[start..start + i]);
        start += i + 5;
    }
    out.push(&s[start..]);
    out
}

// ---- parsing (for seeding the search from existing strategy files) ----

#[derive(Deserialize)]
struct RawRule {
    card: String,
    #[serde(rename = "if")]
    cond: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawRules {
    Names(Vec<String>),
    Rules(Vec<RawRule>),
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    gain: Vec<RawRule>,
    #[serde(default)]
    buy: Vec<RawRule>,
    play: Option<RawRules>,
    #[serde(default)]
    trash: Vec<RawRule>,
    trash_priority: Option<Vec<String>>,
    keep_treasure: Option<u8>,
}

impl Genome {
    /// Read a strategy file. Fails if a condition is outside the template language.
    pub fn from_toml(src: &str) -> Result<Genome, String> {
        let raw: RawFile = toml::from_str(src).map_err(|e| format!("TOML error: {e}"))?;
        let rules = |v: &[RawRule]| v.iter().map(|r| Rule::parse(&r.card, r.cond.as_deref())).collect::<Result<Vec<_>, _>>();
        let gain = if raw.gain.is_empty() { rules(&raw.buy)? } else { rules(&raw.gain)? };
        let play = match raw.play {
            None => Vec::new(),
            Some(RawRules::Names(names)) => names.iter().map(|n| Rule::parse(n, None)).collect::<Result<_, _>>()?,
            Some(RawRules::Rules(r)) => rules(&r)?,
        };
        let trash = match raw.trash_priority {
            Some(names) => names.iter().map(|n| Rule::parse(n, None)).collect::<Result<_, _>>()?,
            None => rules(&raw.trash)?,
        };
        Ok(Genome { gain, play, trash, keep_treasure: raw.keep_treasure.unwrap_or(2) })
    }

    /// Rules plus condition atoms: the complexity charged by the parsimony penalty.
    pub fn size(&self) -> usize {
        self.lists().map(|(_, l)| l.iter().map(|r| 1 + r.conds.len()).sum::<usize>()).sum()
    }

    pub fn lists(&self) -> impl Iterator<Item = (List, &Vec<Rule>)> {
        [(List::Gain, &self.gain), (List::Play, &self.play), (List::Trash, &self.trash)].into_iter()
    }

    pub fn list_mut(&mut self, list: List) -> &mut Vec<Rule> {
        match list {
            List::Gain => &mut self.gain,
            List::Play => &mut self.play,
            List::Trash => &mut self.trash,
        }
    }

    /// A stable identity for deduplication (the rules, without names or comments).
    pub fn key(&self) -> String {
        self.to_toml(&TomlMeta::default())
    }

    /// One-line summary, e.g. `Province > Moat[count(Moat) < 2] > Gold > Silver | play Moat`.
    pub fn summary(&self) -> String {
        let fmt = |rules: &[Rule]| {
            rules
                .iter()
                .map(|r| match r.cond_text() {
                    Some(c) => format!("{} [{c}]", cards::name(r.card)),
                    None => cards::name(r.card).to_string(),
                })
                .collect::<Vec<_>>()
                .join(" > ")
        };
        let mut s = fmt(&self.gain);
        if !self.play.is_empty() {
            let _ = write!(s, " | play {}", fmt(&self.play));
        }
        if !self.trash.is_empty() {
            let _ = write!(s, " | trash {}", fmt(&self.trash));
        }
        s
    }

    /// The genome as a strategy file that `dominion_sim::Strategy::parse` accepts.
    pub fn to_toml(&self, meta: &TomlMeta) -> String {
        let mut s = String::new();
        for line in &meta.header {
            let _ = writeln!(s, "# {line}");
        }
        if !meta.header.is_empty() {
            s.push('\n');
        }
        let _ = writeln!(s, "name = {}", quote(if meta.name.is_empty() { "Evolved" } else { &meta.name }));
        if !meta.description.is_empty() {
            let _ = writeln!(s, "description = {}", quote(&meta.description));
        }
        if !meta.never_gain.is_empty() {
            let names: Vec<String> = meta.never_gain.iter().map(|&c| quote(cards::name(c))).collect();
            let _ = writeln!(s, "never_gain = [{}]", names.join(", "));
        }
        if self.keep_treasure != 2 {
            let _ = writeln!(s, "keep_treasure = {}", self.keep_treasure);
        }
        for (list, rules) in self.lists() {
            let table = match list {
                List::Gain => "gain",
                List::Play => "play",
                List::Trash => "trash",
            };
            for (i, r) in rules.iter().enumerate() {
                s.push('\n');
                for (_, _, note) in meta.notes.iter().filter(|(l, j, _)| *l == list && *j == i) {
                    let _ = writeln!(s, "# {note}");
                }
                let _ = writeln!(s, "[[{table}]]");
                let _ = writeln!(s, "card = {}", quote(cards::name(r.card)));
                if let Some(c) = r.cond_text() {
                    let _ = writeln!(s, "if = {}", quote(&c));
                }
            }
        }
        s
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dominion_engine::cards::id;

    #[test]
    fn round_trips_through_toml_and_the_strategy_parser() {
        let g = Genome {
            gain: vec![
                Rule::new(id::PROVINCE),
                Rule { card: id::DUCHY, conds: vec![Atom { var: Var::ProvincesLeft, op: Op::Lt, n: 5 }] },
                Rule { card: id::MOAT, conds: vec![Atom { var: Var::Count(id::MOAT), op: Op::Lt, n: 2 }, Atom { var: Var::MyTurn, op: Op::Ge, n: 3 }] },
                Rule::new(id::GOLD),
                Rule::new(id::SILVER),
            ],
            play: vec![Rule::new(id::MOAT)],
            trash: vec![],
            keep_treasure: 3,
        };
        let meta = TomlMeta { name: "Test".into(), never_gain: vec![id::WITCH], header: vec!["hello".into()], ..Default::default() };
        let text = g.to_toml(&meta);
        assert!(text.contains("if = \"provinces_left <= 4\""), "{text}");
        assert!(text.contains("if = \"count(Moat) < 2 and my_turn >= 3\""), "{text}");
        dominion_sim::Strategy::parse(&text).unwrap();
        assert_eq!(Genome::from_toml(&text).unwrap(), g);
        assert_eq!(g.size(), 5 + 1 + 3);
    }

    #[test]
    fn parses_shipped_style_conditions() {
        let a = Atom::parse("count(Throne Room) <= 1").unwrap();
        assert_eq!(a, Atom { var: Var::Count(id::THRONE_ROOM), op: Op::Lt, n: 2 });
        assert_eq!(Atom::parse("money > 15").unwrap(), Atom { var: Var::Money, op: Op::Ge, n: 16 });
        assert!(Atom::parse("coins * 2 == 3").is_err());
    }
}
