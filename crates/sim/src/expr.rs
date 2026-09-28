//! A tiny expression language for strategy conditions, e.g. `count(Gold) >= 1 and buys > 1`.
//!
//! Parsing (tokenizing + building the AST) happens once, when a strategy is loaded, and may
//! allocate freely (`Vec<Token>`, `Box` nodes). `Expr::eval` walks the resulting tree and never
//! allocates: it is safe to call once per decision in the hot simulation loop.
//!
//! Grammar (lowest to highest precedence):
//! ```text
//! expr   := or
//! or     := and (("or" | "||") and)*
//! and    := not ("and" | "&&") not)*
//! not    := ("not" | "!") not | cmp
//! cmp    := add (("==" | "!=" | "<=" | ">=" | "<" | ">") add)?
//! add    := mul (("+" | "-") mul)*
//! mul    := unary (("*" | "/") unary)*
//! unary  := "-" unary | primary
//! primary:= NUMBER | IDENT | IDENT "(" IDENT ")" | "(" expr ")"
//! ```
//! `wins_game` / `loses_game` refer to the card of the gain-list entry being evaluated: gaining
//! it would end the game this turn with me winning outright / not winning outright.
//! Card names in `count(..)` / `supply(..)` must be a single token (no spaces): use
//! `ThroneRoom`, `Throne_Room`, or `throne-room` — all normalize to "Throne Room"
//! (see `cards::by_name`).

use dominion_engine::cards::{self, id, CardId};
use dominion_engine::{Counts, PlayerView};

#[derive(Clone, Copy, Debug)]
pub enum Var {
    /// Global turn counter (counts every player's turn).
    Turn,
    /// This player's own turn number (1-based).
    MyTurn,
    /// Coins available to spend this turn.
    Coins,
    /// Buys remaining this turn.
    Buys,
    /// Actions remaining this turn.
    Actions,
    /// Total treasure value owned (Copper=1, Silver=2, Gold=3), wherever the cards are.
    Money,
    /// Total cards owned, wherever they are.
    TotalCards,
    /// Provinces remaining in the supply.
    ProvincesLeft,
    /// Number of empty supply piles.
    EmptyPiles,
    /// This player's current VP.
    MyVp,
    /// My VP minus the best opponent's VP (positive = I'm ahead).
    VpLead,
}

impl Var {
    fn eval(self, view: &PlayerView) -> i64 {
        match self {
            Var::Turn => view.turn().number as i64,
            Var::MyTurn => view.turns_taken_of(view.me()) as i64 + 1,
            Var::Coins => view.turn().coins as i64,
            Var::Buys => view.turn().buys as i64,
            Var::Actions => view.turn().actions as i64,
            Var::Money => money_value(&view.my_cards()),
            Var::TotalCards => view.my_cards().total() as i64,
            Var::ProvincesLeft => view.supply(id::PROVINCE) as i64,
            Var::EmptyPiles => view.empty_piles() as i64,
            Var::MyVp => view.my_vp() as i64,
            Var::VpLead => {
                let mine = view.my_vp();
                let best_other = (0..view.num_players()).filter(|&p| p != view.me()).map(|p| view.vp_of(p)).max().unwrap_or(0);
                (mine - best_other) as i64
            }
        }
    }

    fn parse(name: &str) -> Option<Var> {
        Some(match name.to_ascii_lowercase().as_str() {
            "turn" => Var::Turn,
            "my_turn" | "myturn" => Var::MyTurn,
            "coins" => Var::Coins,
            "buys" => Var::Buys,
            "actions" => Var::Actions,
            "money" => Var::Money,
            "total_cards" | "totalcards" => Var::TotalCards,
            "provinces_left" | "provincesleft" => Var::ProvincesLeft,
            "empty_piles" | "emptypiles" => Var::EmptyPiles,
            "my_vp" | "myvp" => Var::MyVp,
            "vp_lead" | "vplead" => Var::VpLead,
            _ => return None,
        })
    }
}

fn money_value(c: &Counts) -> i64 {
    c.get(id::COPPER) as i64 + 2 * c.get(id::SILVER) as i64 + 3 * c.get(id::GOLD) as i64
}

/// A parsed condition. `Box`-allocated once at load time; `eval` never allocates.
#[derive(Clone, Debug)]
pub enum Expr {
    Num(i64),
    Var(Var),
    /// `count(Card)`: copies of `Card` this player owns anywhere (deck+hand+discard+in play).
    Count(CardId),
    /// `count_type(action|treasure|victory|curse|attack|reaction)`.
    CountType(u8),
    /// `supply(Card)`: copies left in the supply pile.
    Supply(CardId),
    /// `wins_game`: gaining this entry's card ends the game this turn with me winning outright.
    WinsGame,
    /// `loses_game`: gaining this entry's card ends the game this turn with me not winning
    /// outright (a loss or a shared win).
    LosesGame,
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Evaluate against the current view. Nonzero == true. Allocation-free.
    pub fn eval(&self, view: &PlayerView) -> i64 {
        self.eval_in(view, None)
    }

    /// Evaluate with the gain-list entry's card as context (for `wins_game` / `loses_game`).
    pub fn eval_in(&self, view: &PlayerView, card: Option<CardId>) -> i64 {
        use Expr::*;
        match self {
            Num(n) => *n,
            Var(v) => v.eval(view),
            Count(c) => view.my_cards().get(*c) as i64,
            CountType(flag) => view.my_cards().count_type(*flag) as i64,
            Supply(c) => view.supply(*c) as i64,
            WinsGame => card.is_some_and(|c| view.result_if_gained(c) == Some(1 << view.me())) as i64,
            LosesGame => card.is_some_and(|c| matches!(view.result_if_gained(c), Some(w) if w != 1 << view.me())) as i64,
            Neg(a) => -a.eval_in(view, card),
            Not(a) => (a.eval_in(view, card) == 0) as i64,
            Add(a, b) => a.eval_in(view, card).wrapping_add(b.eval_in(view, card)),
            Sub(a, b) => a.eval_in(view, card).wrapping_sub(b.eval_in(view, card)),
            Mul(a, b) => a.eval_in(view, card).wrapping_mul(b.eval_in(view, card)),
            Div(a, b) => {
                let d = b.eval_in(view, card);
                if d == 0 { 0 } else { a.eval_in(view, card) / d }
            }
            Eq(a, b) => (a.eval_in(view, card) == b.eval_in(view, card)) as i64,
            Ne(a, b) => (a.eval_in(view, card) != b.eval_in(view, card)) as i64,
            Lt(a, b) => (a.eval_in(view, card) < b.eval_in(view, card)) as i64,
            Le(a, b) => (a.eval_in(view, card) <= b.eval_in(view, card)) as i64,
            Gt(a, b) => (a.eval_in(view, card) > b.eval_in(view, card)) as i64,
            Ge(a, b) => (a.eval_in(view, card) >= b.eval_in(view, card)) as i64,
            And(a, b) => ((a.eval_in(view, card) != 0) && (b.eval_in(view, card) != 0)) as i64,
            Or(a, b) => ((a.eval_in(view, card) != 0) || (b.eval_in(view, card) != 0)) as i64,
        }
    }

    /// True/nonzero.
    pub fn eval_bool(&self, view: &PlayerView) -> bool {
        self.eval(view) != 0
    }

    /// True/nonzero, with the gain-list entry's card as context.
    pub fn eval_bool_for(&self, view: &PlayerView, card: CardId) -> bool {
        self.eval_in(view, Some(card)) != 0
    }

    /// Visits every card id this condition names via `count(..)` / `supply(..)`. Used to build
    /// an "auto" kingdom (the union of cards strategies reference).
    pub fn for_each_card_ref(&self, f: &mut impl FnMut(CardId)) {
        use Expr::*;
        match self {
            Num(_) | Var(_) | CountType(_) | WinsGame | LosesGame => {}
            Count(c) | Supply(c) => f(*c),
            Neg(a) | Not(a) => a.for_each_card_ref(f),
            Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Eq(a, b) | Ne(a, b) | Lt(a, b) | Le(a, b) | Gt(a, b) | Ge(a, b) | And(a, b) | Or(a, b) => {
                a.for_each_card_ref(f);
                b.for_each_card_ref(f);
            }
        }
    }

    pub fn parse(src: &str) -> Result<Expr, String> {
        let toks = lex(src)?;
        let mut p = Parser { toks: &toks, pos: 0, src };
        let e = p.parse_or()?;
        if p.pos != p.toks.len() {
            return Err(format!("unexpected trailing input in condition {src:?}"));
        }
        Ok(e)
    }
}

// ---------------------------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok<'a> {
    Num(i64),
    Ident(&'a str),
    LParen,
    RParen,
    Comma,
    Plus,
    Minus,
    Star,
    Slash,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
}

fn lex(src: &str) -> Result<Vec<Tok<'_>>, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => { out.push(Tok::LParen); i += 1; }
            ')' => { out.push(Tok::RParen); i += 1; }
            ',' => { out.push(Tok::Comma); i += 1; }
            '+' => { out.push(Tok::Plus); i += 1; }
            '-' => { out.push(Tok::Minus); i += 1; }
            '*' => { out.push(Tok::Star); i += 1; }
            '/' => { out.push(Tok::Slash); i += 1; }
            '=' if b.get(i + 1) == Some(&b'=') => { out.push(Tok::EqEq); i += 2; }
            '!' if b.get(i + 1) == Some(&b'=') => { out.push(Tok::Ne); i += 2; }
            '!' => { out.push(Tok::Not); i += 1; }
            '<' if b.get(i + 1) == Some(&b'=') => { out.push(Tok::Le); i += 2; }
            '<' => { out.push(Tok::Lt); i += 1; }
            '>' if b.get(i + 1) == Some(&b'=') => { out.push(Tok::Ge); i += 2; }
            '>' => { out.push(Tok::Gt); i += 1; }
            '&' if b.get(i + 1) == Some(&b'&') => { out.push(Tok::And); i += 2; }
            '|' if b.get(i + 1) == Some(&b'|') => { out.push(Tok::Or); i += 2; }
            _ if c.is_ascii_digit() => {
                let start = i;
                while i < b.len() && (b[i] as char).is_ascii_digit() {
                    i += 1;
                }
                let n: i64 = src[start..i].parse().map_err(|_| format!("bad number in {src:?}"))?;
                out.push(Tok::Num(n));
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                let word = &src[start..i];
                out.push(match word.to_ascii_lowercase().as_str() {
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "not" => Tok::Not,
                    _ => Tok::Ident(word),
                });
            }
            _ => return Err(format!("unexpected character {c:?} in condition {src:?}")),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Recursive-descent parser
// ---------------------------------------------------------------------------------------------

struct Parser<'a> {
    toks: &'a [Tok<'a>],
    pos: usize,
    src: &'a str,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Tok<'a>> {
        self.toks.get(self.pos).copied()
    }
    fn advance(&mut self) -> Option<Tok<'a>> {
        let t = self.peek();
        self.pos += 1;
        t
    }
    fn eat(&mut self, t: Tok<'a>) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok<'a>) -> Result<(), String> {
        if self.eat(t) { Ok(()) } else { Err(format!("expected {t:?} in condition {:?}", self.src)) }
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_and()?;
        while self.eat(Tok::Or) {
            let rhs = self.parse_and()?;
            e = Expr::Or(Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }
    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_not()?;
        while self.eat(Tok::And) {
            let rhs = self.parse_not()?;
            e = Expr::And(Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }
    fn parse_not(&mut self) -> Result<Expr, String> {
        if self.eat(Tok::Not) {
            return Ok(Expr::Not(Box::new(self.parse_not()?)));
        }
        self.parse_cmp()
    }
    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_add()?;
        let op = match self.peek() {
            Some(Tok::EqEq) => Expr::Eq as fn(_, _) -> Expr,
            Some(Tok::Ne) => Expr::Ne,
            Some(Tok::Le) => Expr::Le,
            Some(Tok::Ge) => Expr::Ge,
            Some(Tok::Lt) => Expr::Lt,
            Some(Tok::Gt) => Expr::Gt,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_add()?;
        Ok(op(Box::new(lhs), Box::new(rhs)))
    }
    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_mul()?;
        loop {
            if self.eat(Tok::Plus) {
                e = Expr::Add(Box::new(e), Box::new(self.parse_mul()?));
            } else if self.eat(Tok::Minus) {
                e = Expr::Sub(Box::new(e), Box::new(self.parse_mul()?));
            } else {
                return Ok(e);
            }
        }
    }
    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_unary()?;
        loop {
            if self.eat(Tok::Star) {
                e = Expr::Mul(Box::new(e), Box::new(self.parse_unary()?));
            } else if self.eat(Tok::Slash) {
                e = Expr::Div(Box::new(e), Box::new(self.parse_unary()?));
            } else {
                return Ok(e);
            }
        }
    }
    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.eat(Tok::Minus) {
            return Ok(Expr::Neg(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }
    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Some(Tok::Num(n)) => Ok(Expr::Num(n)),
            Some(Tok::LParen) => {
                let e = self.parse_or()?;
                self.expect(Tok::RParen)?;
                Ok(e)
            }
            Some(Tok::Ident(name)) => {
                if self.eat(Tok::LParen) {
                    let arg = match self.advance() {
                        Some(Tok::Ident(a)) => a,
                        _ => return Err(format!("expected an identifier argument in condition {:?}", self.src)),
                    };
                    self.expect(Tok::RParen)?;
                    make_call(name, arg, self.src)
                } else {
                    match name.to_ascii_lowercase().as_str() {
                        "wins_game" | "winsgame" => Ok(Expr::WinsGame),
                        "loses_game" | "losesgame" => Ok(Expr::LosesGame),
                        _ => Var::parse(name).map(Expr::Var).ok_or_else(|| format!("unknown variable {name:?} in condition {:?}", self.src)),
                    }
                }
            }
            other => Err(format!("unexpected token {other:?} in condition {:?}", self.src)),
        }
    }
}

fn make_call(func: &str, arg: &str, src: &str) -> Result<Expr, String> {
    match func.to_ascii_lowercase().as_str() {
        "count" => {
            let c = cards::by_name(arg).ok_or_else(|| format!("unknown card {arg:?} in condition {src:?}"))?;
            Ok(Expr::Count(c))
        }
        "supply" => {
            let c = cards::by_name(arg).ok_or_else(|| format!("unknown card {arg:?} in condition {src:?}"))?;
            Ok(Expr::Supply(c))
        }
        "count_type" => {
            let flag = match arg.to_ascii_lowercase().as_str() {
                "action" => cards::ACTION,
                "treasure" => cards::TREASURE,
                "victory" => cards::VICTORY,
                "curse" => cards::CURSE_T,
                "attack" => cards::ATTACK,
                "reaction" => cards::REACTION,
                _ => return Err(format!("unknown card type {arg:?} in condition {src:?}")),
            };
            Ok(Expr::CountType(flag))
        }
        _ => Err(format!("unknown function {func:?} in condition {src:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dominion_engine::{GameConfig, GameState};

    #[test]
    fn parses_and_evaluates() {
        let g = GameState::new(&GameConfig::default());
        let view = PlayerView::new(&g, 0);
        assert_eq!(Expr::parse("1 + 2 * 3").unwrap().eval(&view), 7);
        assert_eq!(Expr::parse("(1 + 2) * 3").unwrap().eval(&view), 9);
        assert_eq!(Expr::parse("count(Copper) >= 7").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("count(Gold) >= 1").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("provinces_left <= 4 and buys > 0").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("not (count(Gold) >= 1)").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("supply(Province) == 8").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("count_type(treasure) >= 7").unwrap().eval(&view), 1);
    }

    #[test]
    fn accepts_normalized_multiword_names() {
        let g = GameState::new(&GameConfig::default());
        let view = PlayerView::new(&g, 0);
        assert_eq!(Expr::parse("count(ThroneRoom)").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("count(Throne_Room)").unwrap().eval(&view), 0);
    }
}
