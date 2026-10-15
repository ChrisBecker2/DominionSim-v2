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
    /// Potions available to spend this turn (Alchemy): Potion cards played, less those spent on
    /// buying cards with a Potion in their cost. Buying such a card needs `potions >= 1`.
    Potions,
    /// Cards I own with a Potion in their cost (Familiar, Golem, ...), wherever they are: handy for
    /// "buy a Potion only while I have fewer than N of them" rules.
    PotionCards,
    /// Coffers tokens I have (Cornucopia & Guilds): each is +$1 whenever I spend it in the Buy phase.
    Coffers,
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
    /// Colonies remaining in the supply (0 in games without Colony).
    ColoniesLeft,
    /// Number of empty supply piles.
    EmptyPiles,
    /// Number of supply piles with at most 2 cards left (the empty ones too): how close the game is to ending on piles.
    LowPiles,
    /// This player's current VP.
    MyVp,
    /// My VP minus the best opponent's VP (positive = I'm ahead).
    VpLead,
    /// 1 while my Journey token is face up (Adventures: Ranger and Pilgrimage act on the next flip when it is face down).
    Journey,
    /// 1 while my -1 Card token is on my deck / my -$1 token is in front of me.
    MinusCard,
    MinusCoin,
    /// Cards on my Tavern mat.
    MatCards,
    /// Debt tokens I have (Empires): while there are any I cannot buy.
    Debt,
    /// VP tokens I have.
    VpTokens,
    /// The most Debt any opponent has.
    OpponentDebt,
    /// 1 during my Night phase (Nocturne): a `[[play]]` rule for a Night card (Werewolf is both an Action and a Night card) can ask.
    Night,
    /// Boons in the deck / in the discard pile (Nocturne): how many are left to come, and how many have been seen since the last shuffle.
    BoonsInDeck,
    BoonsInDiscard,
    /// The same for the Hexes.
    HexesInDeck,
    HexesInDiscard,
    /// 1 while I have Lost in the Woods.
    LostInTheWoods,
    /// Cards I have gained so far this turn (counted in games with Monastery, Devil's Workshop or a card gained to the hand).
    GainsThisTurn,
    /// Sun tokens left on the Prophecy (Rising Sun; 0 once it has happened or with no Prophecy), and 1 once the Prophecy has happened.
    SunTokens,
    ProphecyActive,
    /// Villagers I have (Renaissance): each is +1 Action in the Action phase. `opp_villagers`: the most any opponent has.
    Villagers,
    OppVillagers,
    /// Projects I own (0 to 2), and the tokens I have on Sinister Plot.
    Projects,
    PlotTokens,
}

impl Var {
    fn eval(self, view: &PlayerView) -> i64 {
        match self {
            Var::Turn => view.turn().number as i64,
            Var::MyTurn => view.turns_taken_of(view.me()) as i64 + 1,
            Var::Coins => view.turn().coins as i64,
            Var::Potions => view.turn().potions as i64,
            Var::PotionCards => view.my_cards().iter().filter(|&(c, _)| cards::potion_cost(c)).map(|(_, n)| n as i64).sum(),
            Var::Coffers => view.coffers() as i64,
            Var::Buys => view.buys_left() as i64,
            Var::Actions => view.turn().actions as i64,
            Var::Money => money_value(&view.my_cards()),
            Var::TotalCards => view.my_cards().total() as i64,
            Var::ProvincesLeft => view.supply(id::PROVINCE) as i64,
            Var::ColoniesLeft => view.supply(id::COLONY) as i64,
            Var::EmptyPiles => view.empty_piles() as i64,
            Var::LowPiles => view.supply_cards().filter(|&c| view.supply(c) <= 2).count() as i64,
            Var::MyVp => view.my_vp() as i64,
            Var::VpLead => {
                let mine = view.my_vp();
                let best_other = (0..view.num_players()).filter(|&p| p != view.me()).map(|p| view.vp_of(p)).max().unwrap_or(0);
                (mine - best_other) as i64
            }
            Var::Journey => (view.adv_flags_of(view.me()) & dominion_engine::state::ADV_JOURNEY_DOWN == 0) as i64,
            Var::MinusCard => (view.adv_flags_of(view.me()) & dominion_engine::state::ADV_MINUS_CARD != 0) as i64,
            Var::MinusCoin => (view.adv_flags_of(view.me()) & dominion_engine::state::ADV_MINUS_COIN != 0) as i64,
            Var::MatCards => view.tavern_of(view.me()).total() as i64,
            Var::Debt => view.debt_of(view.me()) as i64,
            Var::VpTokens => view.vp_tokens_of(view.me()) as i64,
            Var::OpponentDebt => (0..view.num_players()).filter(|&p| p != view.me()).map(|p| view.debt_of(p)).max().unwrap_or(0) as i64,
            Var::Night => (view.phase() == dominion_engine::Phase::Night) as i64,
            Var::BoonsInDeck => view.boon_deck_mask().count_ones() as i64,
            Var::BoonsInDiscard => view.boon_discard_mask().count_ones() as i64,
            Var::HexesInDeck => view.hex_deck_mask().count_ones() as i64,
            Var::HexesInDiscard => view.hex_discard_mask().count_ones() as i64,
            Var::LostInTheWoods => (view.lost_in_the_woods() == Some(view.me())) as i64,
            Var::GainsThisTurn => view.gains_this_turn() as i64,
            Var::SunTokens => view.sun_tokens() as i64,
            Var::ProphecyActive => view.prophecy_active() as i64,
            Var::Villagers => view.villagers() as i64,
            Var::OppVillagers => (0..view.num_players()).filter(|&p| p != view.me()).map(|p| view.villagers_of(p)).max().unwrap_or(0) as i64,
            Var::Projects => view.projects_owned_by(view.me()) as i64,
            Var::PlotTokens => view.plot_tokens_of(view.me()) as i64,
        }
    }

    fn parse(name: &str) -> Option<Var> {
        Some(match name.to_ascii_lowercase().as_str() {
            "turn" => Var::Turn,
            "my_turn" | "myturn" => Var::MyTurn,
            "coins" => Var::Coins,
            "buys" => Var::Buys,
            "coffers" => Var::Coffers,
            "potions" => Var::Potions,
            "potion_cards" | "potioncards" => Var::PotionCards,
            "actions" => Var::Actions,
            "money" => Var::Money,
            "total_cards" | "totalcards" => Var::TotalCards,
            "provinces_left" | "provincesleft" => Var::ProvincesLeft,
            "colonies_left" | "coloniesleft" => Var::ColoniesLeft,
            "empty_piles" | "emptypiles" => Var::EmptyPiles,
            "low_piles" | "lowpiles" => Var::LowPiles,
            "my_vp" | "myvp" => Var::MyVp,
            "vp_lead" | "vplead" => Var::VpLead,
            "journey" => Var::Journey,
            "minus_card" | "minuscard" => Var::MinusCard,
            "minus_coin" | "minuscoin" => Var::MinusCoin,
            "mat_cards" | "matcards" => Var::MatCards,
            "debt" => Var::Debt,
            "vp_tokens" | "vptokens" => Var::VpTokens,
            "opponent_debt" | "opponentdebt" => Var::OpponentDebt,
            "night" => Var::Night,
            "boons_in_deck" => Var::BoonsInDeck,
            "boons_in_discard" => Var::BoonsInDiscard,
            "hexes_in_deck" => Var::HexesInDeck,
            "hexes_in_discard" => Var::HexesInDiscard,
            "lost_in_the_woods" => Var::LostInTheWoods,
            "gains_this_turn" => Var::GainsThisTurn,
            "sun_tokens" | "suns" => Var::SunTokens,
            "prophecy_active" => Var::ProphecyActive,
            "villagers" => Var::Villagers,
            "opp_villagers" | "opponent_villagers" => Var::OppVillagers,
            "projects" => Var::Projects,
            "plot_tokens" => Var::PlotTokens,
            _ => return None,
        })
    }
}

/// Total coin value of the treasures in `c` (every Treasure, e.g. Harem too).
fn money_value(c: &Counts) -> i64 {
    c.iter().filter(|&(card, _)| cards::is(card, cards::TREASURE)).map(|(card, n)| cards::def(card).coins as i64 * n as i64).sum()
}

/// The five cards of the Traveller line `card` belongs to, first card first (the Page line for a card of neither line: callers check
/// membership).
pub fn traveller_line(card: CardId) -> &'static [CardId] {
    use id::*;
    const PAGE_LINE: [CardId; 5] = [PAGE, TREASURE_HUNTER, WARRIOR, HERO, CHAMPION];
    const PEASANT_LINE: [CardId; 5] = [PEASANT, SOLDIER, FUGITIVE, DISCIPLE, TEACHER];
    if PEASANT_LINE.contains(&card) { &PEASANT_LINE } else { &PAGE_LINE }
}

/// A parsed condition. `Box`-allocated once at load time; `eval` never allocates.
#[derive(Clone, Debug)]
pub enum Expr {
    Num(i64),
    Var(Var),
    /// `count(Card)`: copies of `Card` this player owns anywhere (deck+hand+discard+in play).
    Count(CardId),
    /// `count_type(action|treasure|victory|curse|attack|reaction)`.
    CountType(u32),
    /// `count(Settlers/Bustling Village)`: copies of either half of a split pile.
    CountPair(CardId, CardId),
    /// `count_line(Page)`: copies owned of every card of a Traveller line (Page, Treasure Hunter, Warrior, Hero and Champion; or Peasant,
    /// Soldier, Fugitive, Disciple and Teacher), named by any card of the line. Holds the line's first card.
    CountLine(CardId),
    /// `pile_vp(Card)`: VP tokens on the card's Supply pile (Farmers' Market, Temple, Wild Hunt, Aqueduct and Defiled Shrine's piles).
    PileVp(CardId),
    /// `debt_cost`: the Debt in the cost of the gain-list entry's card (0 for most cards).
    DebtCost,
    /// `landmark(Name)`: 1 if the Landmark is in this game; `landmark_vp(Name)`: the VP tokens left on it.
    Landmark(CardId),
    LandmarkVp(CardId),
    /// `prophecy(Name)`: 1 if the Prophecy is in this game (Rising Sun; it may not have happened yet: see `prophecy_active`).
    Prophecy(CardId),
    /// `has_project(Name)` / `opp_has_project(Name)`: 1 if I (any opponent) have put a cube on the Project (Renaissance); `project(Name)`: 1 if it is in this game.
    HasProject(CardId),
    OppHasProject(CardId),
    ProjectInGame(CardId),
    /// `has_artifact(Name)` / `opp_has_artifact(Name)`: 1 if I (another player) hold the Artifact.
    HasArtifact(CardId),
    OppHasArtifact(CardId),
    /// `supply(Card)`: copies left in the supply pile.
    Supply(CardId),
    /// `mat(Card)`: copies of `Card` on my Tavern mat (Adventures).
    Mat(CardId),
    /// `bought(Event)`: 1 if I have bought the Event this game; `bought_turn(Event)`: 1 if I bought it this turn.
    Bought(CardId),
    BoughtTurn(CardId),
    /// `token_on(+1 Card, Smithy)`: 1 if my token is on that pile (the Knights pile by "Knights", the split pile by either half).
    TokenOn(usize, CardId),
    /// `has_state(Deluded)` / `opp_has_state(Envious)`: 1 if I (any opponent) have the State (Nocturne).
    HasState(CardId),
    OppHasState(CardId),
    /// `boon_in_discard(The Sea's Gift)`, `boon_in_deck(..)`, `hex_in_discard(..)`, `hex_in_deck(..)`: 1 if the Boon or Hex is there (public information).
    InPile(CardId, bool),
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
            Count(c) => match dominion_engine::sets::dark_ages::MixedPile::of_marker(*c) {
                // `count(Knights)`: Knights owned, of any of the ten names.
                Some(pile) => {
                    let mine = view.my_cards();
                    pile.members().map(|m| mine.get(m) as i64).sum()
                }
                // `count(Castles)`: Castles owned, of any of the eight names.
                None if *c == id::CASTLES => view.my_cards().count_type(cards::CASTLE) as i64,
                None => view.my_cards().get(*c) as i64,
            },
            CountLine(first) => {
                let mine = view.my_cards();
                traveller_line(*first).iter().map(|&c| mine.get(c) as i64).sum()
            }
            CountPair(a, b) => {
                let mine = view.my_cards();
                mine.get(*a) as i64 + mine.get(*b) as i64
            }
            PileVp(c) => view.pile_vp(*c) as i64,
            DebtCost => card.map_or(0, |c| cards::debt_cost(c) as i64),
            Landmark(l) => view.has_landmark(*l) as i64,
            LandmarkVp(l) => view.landmark_vp(*l) as i64,
            Prophecy(p) => (view.prophecy() == *p) as i64,
            HasProject(p) => view.has_project(view.me(), *p) as i64,
            OppHasProject(p) => (0..view.num_players()).any(|q| q != view.me() && view.has_project(q, *p)) as i64,
            ProjectInGame(p) => view.has_project_in_game(*p) as i64,
            HasArtifact(a) => view.has_artifact(view.me(), *a) as i64,
            OppHasArtifact(a) => view.artifact_holder(*a).is_some_and(|h| h != view.me()) as i64,
            CountType(flag) => view.my_cards().count_type(*flag) as i64,
            Supply(c) => view.supply(*c) as i64,
            Mat(c) => view.tavern_of(view.me()).get(*c) as i64,
            Bought(e) => (view.event_bought_by(view.me(), *e) || view.has_project(view.me(), *e)) as i64,
            BoughtTurn(e) => view.event_bought_this_turn(*e) as i64,
            TokenOn(tok, pile) => {
                let t = view.tokens_of(view.me());
                let on = if *tok < t.len() { t[*tok] } else if *tok == 6 { view.inherit_of(view.me()) } else { 0 };
                (on != 0 && (on == *pile || (*tok == 6 && on == *pile))) as i64
            }
            HasState(s) => view.has_state(view.me(), *s) as i64,
            OppHasState(s) => (0..view.num_players()).any(|p| p != view.me() && view.has_state(p, *s)) as i64,
            InPile(c, discard) => match (cards::is_boon(*c), *discard) {
                (true, true) => view.boon_in_discard(*c) as i64,
                (true, false) => view.boon_in_deck(*c) as i64,
                (false, true) => view.hex_in_discard(*c) as i64,
                (false, false) => view.hex_in_deck(*c) as i64,
            },
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
    /// Whether the condition uses `wins_game` or `loses_game`: the strategy says itself when to end
    /// the game, so the default guard against ending it in a loss stays out of its way.
    pub fn mentions_game_end(&self) -> bool {
        use Expr::*;
        match self {
            WinsGame | LosesGame => true,
            Neg(a) | Not(a) => a.mentions_game_end(),
            Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Eq(a, b) | Ne(a, b) | Lt(a, b) | Le(a, b) | Gt(a, b) | Ge(a, b) | And(a, b) | Or(a, b) => {
                a.mentions_game_end() || b.mentions_game_end()
            }
            _ => false,
        }
    }

    pub fn for_each_card_ref(&self, f: &mut impl FnMut(CardId)) {
        use Expr::*;
        match self {
            Num(_) | Var(_) | CountType(_) | WinsGame | LosesGame | DebtCost | HasState(_) | OppHasState(_) | InPile(..) => {}
            CountPair(a, _) => f(*a),
            CountLine(c) => f(*c),
            Count(c) | Supply(c) | Mat(c) | Bought(c) | BoughtTurn(c) | TokenOn(_, c) | PileVp(c) | Landmark(c) | LandmarkVp(c) | Prophecy(c) => f(*c),
            HasProject(c) | OppHasProject(c) | ProjectInGame(c) => f(*c),
            HasArtifact(_) | OppHasArtifact(_) => {}
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
                // A function call's argument is a card name, taken verbatim up to the `)`, so
                // names with spaces or apostrophes work: `count(Throne Room)`, `count(King's Court)`.
                let is_function = matches!(
                    word.to_ascii_lowercase().as_str(),
                    "count" | "supply" | "count_type" | "mat" | "bought" | "bought_turn" | "token_on" | "pile_vp" | "landmark" | "landmark_vp" | "prophecy"
                        | "has_state" | "opp_has_state" | "boon_in_discard" | "boon_in_deck" | "hex_in_discard" | "hex_in_deck"
                );
                let mut j = i;
                while j < b.len() && (b[j] as char).is_ascii_whitespace() {
                    j += 1;
                }
                if is_function && j < b.len() && b[j] == b'(' {
                    if let Some(close) = src[j + 1..].find(')') {
                        let arg = src[j + 1..j + 1 + close].trim();
                        if !arg.is_empty() {
                            out.push(Tok::LParen);
                            out.push(Tok::Ident(arg));
                            out.push(Tok::RParen);
                            i = j + 1 + close + 1;
                        }
                    }
                }
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
                        "debt_cost" | "debtcost" => Ok(Expr::DebtCost),
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
    // Events, Landmarks and Prophecies are never in a zone or a pile: they have no count, supply
    // or mat lane (reading one would index past the zone ids).
    let zone_card = |c: CardId, what: &str| {
        if cards::is_pseudo(c) {
            Err(format!("{arg:?} is not a card that can be {what} (an Event, Landmark, Prophecy, Project or Artifact), in condition {src:?}; for an Event use bought({arg}), for a Project has_project({arg}), for an Artifact has_artifact({arg})"))
        } else {
            Ok(c)
        }
    };
    match func.to_ascii_lowercase().as_str() {
        "count" => {
            let c = cards::by_name(arg).ok_or_else(|| format!("unknown card {arg:?} in condition {src:?}"))?;
            let c = zone_card(c, "owned")?;
            // A split pile named as `Top/Bottom` counts both halves.
            if arg.contains('/') {
                if let Some(b) = cards::split_bottom(c) {
                    return Ok(Expr::CountPair(c, b));
                }
            }
            Ok(Expr::Count(c))
        }
        "count_line" => {
            let c = cards::by_name(arg).ok_or_else(|| format!("unknown card {arg:?} in condition {src:?}"))?;
            let first = traveller_line(c)[0];
            if !traveller_line(c).contains(&c) {
                return Err(format!("{arg:?} is not a Traveller or a card of a Traveller line (Page, Peasant, ...), in condition {src:?}"));
            }
            Ok(Expr::CountLine(first))
        }
        "pile_vp" => {
            let c = cards::by_name(arg).ok_or_else(|| format!("unknown card {arg:?} in condition {src:?}"))?;
            Ok(Expr::PileVp(cards::pile_of(c)))
        }
        "landmark" | "landmark_vp" => {
            let l = cards::by_name(arg).filter(|&l| cards::is_landmark(l)).ok_or_else(|| format!("{arg:?} is not a Landmark, in condition {src:?}"))?;
            Ok(if func.eq_ignore_ascii_case("landmark") { Expr::Landmark(l) } else { Expr::LandmarkVp(l) })
        }
        "prophecy" => {
            let p = cards::by_name(arg).filter(|&p| cards::is_prophecy(p)).ok_or_else(|| format!("{arg:?} is not a Prophecy, in condition {src:?}"))?;
            Ok(Expr::Prophecy(p))
        }
        "has_project" | "opp_has_project" | "project" => {
            let p = cards::by_name(arg).filter(|&p| cards::is_project(p)).ok_or_else(|| format!("{arg:?} is not a Project, in condition {src:?}"))?;
            Ok(match func.to_ascii_lowercase().as_str() {
                "has_project" => Expr::HasProject(p),
                "opp_has_project" => Expr::OppHasProject(p),
                _ => Expr::ProjectInGame(p),
            })
        }
        "has_artifact" | "opp_has_artifact" => {
            let a = cards::by_name(arg).filter(|&a| cards::is_artifact(a)).ok_or_else(|| format!("{arg:?} is not an Artifact, in condition {src:?}"))?;
            Ok(if func.eq_ignore_ascii_case("has_artifact") { Expr::HasArtifact(a) } else { Expr::OppHasArtifact(a) })
        }
        "has_state" | "opp_has_state" => {
            let s = cards::by_name(arg).filter(|&s| cards::is_state(s)).ok_or_else(|| format!("{arg:?} is not a State, in condition {src:?}"))?;
            Ok(if func.eq_ignore_ascii_case("has_state") { Expr::HasState(s) } else { Expr::OppHasState(s) })
        }
        "boon_in_discard" | "boon_in_deck" => {
            let b = cards::by_name(arg).filter(|&b| cards::is_boon(b)).ok_or_else(|| format!("{arg:?} is not a Boon, in condition {src:?}"))?;
            Ok(Expr::InPile(b, func.eq_ignore_ascii_case("boon_in_discard")))
        }
        "hex_in_discard" | "hex_in_deck" => {
            let h = cards::by_name(arg).filter(|&h| cards::is_hex(h)).ok_or_else(|| format!("{arg:?} is not a Hex, in condition {src:?}"))?;
            Ok(Expr::InPile(h, func.eq_ignore_ascii_case("hex_in_discard")))
        }
        "supply" => {
            let c = cards::by_name(arg).ok_or_else(|| format!("unknown card {arg:?} in condition {src:?}"))?;
            Ok(Expr::Supply(zone_card(c, "in a pile")?))
        }
        "mat" => {
            let c = cards::by_name(arg).ok_or_else(|| format!("unknown card {arg:?} in condition {src:?}"))?;
            Ok(Expr::Mat(zone_card(c, "on a mat")?))
        }
        "bought" | "bought_turn" => {
            // A Project is "bought" once its cube is on it (`has_project` says the same); only Events can be bought this turn.
            let project_ok = func.eq_ignore_ascii_case("bought");
            let e = cards::by_name(arg)
                .filter(|&e| cards::is_event(e) || (project_ok && cards::is_project(e)))
                .ok_or_else(|| format!("{arg:?} is not an Event{}, in condition {src:?}", if project_ok { " or Project" } else { "" }))?;
            Ok(if project_ok { Expr::Bought(e) } else { Expr::BoughtTurn(e) })
        }
        "token_on" => {
            let (tok, pile) = arg.split_once(',').ok_or_else(|| format!("token_on needs a token and a pile, e.g. token_on(+1 Card, Smithy), in condition {src:?}"))?;
            let tok = cards::token_by_name(tok.trim()).filter(|&t| t < dominion_engine::state::NUM_PILE_TOKENS || t == 6).ok_or_else(|| format!("unknown token {:?} in condition {src:?}", tok.trim()))?;
            let pile = cards::by_name(pile.trim()).ok_or_else(|| format!("unknown card {:?} in condition {src:?}", pile.trim()))?;
            let pile = if pile >= cards::id::DAME_ANNA && pile < cards::NUM_ZONE_IDS as CardId { cards::id::KNIGHTS } else if tok == 6 { pile } else { cards::pile_of(pile) };
            Ok(Expr::TokenOn(tok, pile))
        }
        "count_type" => {
            let flag = match arg.to_ascii_lowercase().as_str() {
                "action" => cards::ACTION,
                "treasure" => cards::TREASURE,
                "victory" => cards::VICTORY,
                "curse" => cards::CURSE_T,
                "attack" => cards::ATTACK,
                "reaction" => cards::REACTION,
                "traveller" => cards::TRAVELLER,
                "reserve" => cards::RESERVE,
                "castle" => cards::CASTLE,
                "night" => cards::NIGHT,
                "fate" => cards::FATE,
                "doom" => cards::DOOM,
                "spirit" => cards::SPIRIT,
                "heirloom" => cards::HEIRLOOM,
                "duration" => cards::DURATION,
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
    fn events_have_no_count_supply_or_mat() {
        // They are never in a zone; reading their lane would index past the zone ids.
        for src in ["count(Kintsugi) < 1", "supply(Ball) > 0", "mat(Biding Time) > 0"] {
            let e = Expr::parse(src).unwrap_err();
            assert!(e.contains("not a card"), "{src}: {e}");
        }
        assert!(Expr::parse("count(Kintsugi) < 1").unwrap_err().contains("bought(Kintsugi)"), "the error suggests bought()");
        assert!(Expr::parse("bought(Kintsugi) < 1").is_ok());
    }

    #[test]
    fn parses_and_evaluates() {
        let g = GameState::new(&GameConfig::default());
        let view = PlayerView::new(&g, 0);
        assert_eq!(Expr::parse("1 + 2 * 3").unwrap().eval(&view), 7);
        assert_eq!(Expr::parse("(1 + 2) * 3").unwrap().eval(&view), 9);
        assert_eq!(Expr::parse("count(Copper) >= 7").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("count(Gold) >= 1").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("provinces_left <= 4 and buys > 0").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("colonies_left").unwrap().eval(&view), 0, "no Colony in this game");
        assert_eq!(Expr::parse("potions").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("potion_cards").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("not (count(Gold) >= 1)").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("supply(Province) == 8").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("count_type(treasure) >= 7").unwrap().eval(&view), 1);
    }

    #[test]
    fn colonies_left_reflects_the_colony_pile() {
        let mut kingdom = cards::FIRST_GAME.to_vec();
        kingdom.push(id::COLONY);
        let cfg = GameConfig { kingdom, ..GameConfig::default() };
        let g = GameState::new(&cfg);
        let view = PlayerView::new(&g, 0);
        assert_eq!(Expr::parse("colonies_left").unwrap().eval(&view), view.supply(id::COLONY) as i64);
        assert!(Expr::parse("colonies_left > 0").unwrap().eval(&view) == 1);
    }

    #[test]
    fn low_piles_counts_piles_with_at_most_two_cards() {
        let mut g = GameState::new(&GameConfig::default());
        assert_eq!(Expr::parse("low_piles").unwrap().eval(&PlayerView::new(&g, 0)), 0);
        g.supply.set(id::CURSE, 2);
        g.supply.set(id::DUCHY, 0);
        g.supply.set(id::SILVER, 3);
        let view = PlayerView::new(&g, 0);
        assert_eq!(Expr::parse("low_piles").unwrap().eval(&view), 2);
        assert_eq!(Expr::parse("low_piles >= 2 and empty_piles >= 1").unwrap().eval(&view), 1);
    }

    #[test]
    fn accepts_normalized_multiword_names() {
        let g = GameState::new(&GameConfig::default());
        let view = PlayerView::new(&g, 0);
        assert_eq!(Expr::parse("count(ThroneRoom)").unwrap().eval(&view), 0);
        assert_eq!(Expr::parse("count(Throne_Room)").unwrap().eval(&view), 0);
        // Card names as printed, with spaces and apostrophes.
        assert_eq!(Expr::parse("count(Throne Room) < 2").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("count( King's Court ) < 1 and supply(Worker's Village) >= 0").unwrap().eval(&view), 1);
        assert_eq!(Expr::parse("count(Copper) >= 7").unwrap().eval(&view), 1);
        // Keywords followed by a parenthesis are still grouping, not calls.
        assert_eq!(Expr::parse("not (count(Copper) < 7)").unwrap().eval(&view), 1);
        assert!(Expr::parse("count(Nonsense Card) < 2").is_err());
    }
}
