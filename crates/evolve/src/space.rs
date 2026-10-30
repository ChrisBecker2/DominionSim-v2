//! The search space: which cards a candidate may gain, play and trash, and how big it may get.

use dominion_engine::cards::{self, id, CardId, ACTION};

use crate::genome::{Genome, List, Rule, Var};

/// Whether a rule for `c` plays an Action or a Night card (Nocturne: Night cards are ranked in the same `[[play]]` list): an Action or Night card, or the Knights pile (a rule for "Knights" plays whichever Knight is in hand).
pub fn is_action(c: CardId) -> bool {
    cards::is(c, ACTION | cards::NIGHT) || c == id::KNIGHTS
}

#[derive(Clone, Debug)]
pub struct SearchSpace {
    /// Cards a candidate may put in `[[gain]]` rules (basic cards plus allowed kingdom cards).
    pub gainable: Vec<CardId>,
    /// Cards a candidate may put in `[[trash]]` rules.
    pub trashable: Vec<CardId>,
    /// Cards the candidate must never gain (written as `never_gain` so forced gains respect it too).
    pub forbidden: Vec<CardId>,
    /// Cards with a choose-one effect (`cards::modes` non-empty) in the kingdom pool: the only
    /// cards a `[[mode]]` rule may name.
    pub modal: Vec<CardId>,
    /// Action piles a `[[token]]` rule may name (the Actions of the kingdom pool).
    pub token_piles: Vec<CardId>,
    /// Tokens (indices into `cards::TOKEN_NAMES`) that some card or Event of the pool puts on a pile.
    pub token_kinds: Vec<u8>,
    /// Reserve cards of the pool whose call a `[[call]]` rule may state.
    pub reserves: Vec<CardId>,
    pub max_gain_rules: usize,
    pub max_play_rules: usize,
    pub max_trash_rules: usize,
    pub max_mode_rules: usize,
    pub max_token_rules: usize,
    pub max_call_rules: usize,
    /// Whether `[[debt]]` rules are worth searching: the pool has Change (Rising Sun), the one card that makes it matter whether Debt is paid.
    pub debt_rules: bool,
    pub max_debt_rules: usize,
    pub max_conds: usize,
    /// Kingdom cards whose effect may trash from the hand (the Actions and Treasures of the pool): the cards a `by(..)` condition on a
    /// `[[trash]]` rule may name.
    pub trashers: Vec<CardId>,
    /// Whether differently named cards score (Museum, Fairgrounds): `distinct_cards` conditions are searched.
    pub names_matter: bool,
    /// Whether cards are gained outside the Buy phase every turn (Kind Emperor): `buy_phase < 1` conditions are searched.
    pub free_gains: bool,
    /// Pairs of kingdom cards that need each other (enabler, partner; [`crate::baselines::combos`]): a mutation adds both at once.
    pub combos: Vec<(CardId, CardId)>,
}

impl SearchSpace {
    /// `kingdom_pool`: kingdom cards that can appear in games; `forbidden`: cards we may not use.
    pub fn new(kingdom_pool: &[CardId], forbidden: &[CardId]) -> SearchSpace {
        let basics = [id::PROVINCE, id::DUCHY, id::ESTATE, id::GOLD, id::SILVER];
        // Landmarks are never gained.
        let mut gainable: Vec<CardId> = basics.iter().chain(kingdom_pool).copied().filter(|c| !forbidden.contains(c) && !cards::is_landmark(*c) && !cards::is_prophecy(*c)).collect();
        // Joust brings the Rewards, which are not in the Supply but can be gained from it: a rule may
        // name one (their order in the gain list is the strategy's preference when Joust offers them).
        if gainable.contains(&id::JOUST) {
            gainable.extend((cards::FIRST_REWARD..cards::FIRST_REWARD + cards::NUM_REWARDS as CardId).filter(|c| !forbidden.contains(c)));
        }
        let has = |c: CardId| kingdom_pool.contains(&c) && !forbidden.contains(&c);
        // With Gardens (worth more for every card) buying Copper can pay: it may be named then.
        if kingdom_pool.contains(&id::GARDENS) && !forbidden.contains(&id::GARDENS) {
            gainable.push(id::COPPER);
        }
        // The tokens that something in the pool places (Teacher comes with the Peasant line).
        let mut token_kinds: Vec<u8> = Vec::new();
        for (cards_, kinds) in [
            (&[id::LOST_ARTS][..], &[1u8][..]),
            (&[id::PATHFINDING], &[0]),
            (&[id::SEAWAY], &[2]),
            (&[id::TRAINING], &[3]),
            (&[id::FERRY], &[4]),
            (&[id::PLAN], &[5]),
            (&[id::PEASANT], &[0, 1, 2, 3]),
        ] {
            if cards_.iter().any(|&c| has(c)) {
                for &k in kinds {
                    if !token_kinds.contains(&k) {
                        token_kinds.push(k);
                    }
                }
            }
        }
        let token_piles: Vec<CardId> =
            if token_kinds.is_empty() { Vec::new() } else { kingdom_pool.iter().copied().filter(|&c| is_action(c) && !forbidden.contains(&c)).collect() };
        let reserves: Vec<CardId> = [
            id::COIN_OF_THE_REALM, id::RATCATCHER, id::GUIDE, id::TRANSMOGRIFY, id::DUPLICATE, id::ROYAL_CARRIAGE, id::WINE_MERCHANT,
        ]
        .into_iter()
        .filter(|&c| has(c))
        .collect();
        let trashers: Vec<CardId> =
            kingdom_pool.iter().copied().filter(|&c| !forbidden.contains(&c) && (is_action(c) || cards::is(c, cards::TREASURE)) && c >= cards::FIRST_KINGDOM).collect();
        SearchSpace {
            combos: crate::baselines::combos(kingdom_pool, forbidden).into_iter().map(|(a, b, _)| (a, b)).collect(),
            trashers,
            names_matter: has(id::MUSEUM) || has(id::FAIRGROUNDS),
            free_gains: has(id::KIND_EMPEROR),
            token_piles,
            token_kinds,
            reserves,
            max_token_rules: 3,
            max_call_rules: 3,
            debt_rules: has(id::CHANGE),
            max_debt_rules: 2,
            gainable,
            trashable: vec![id::CURSE, id::ESTATE, id::COPPER, id::SILVER],
            forbidden: forbidden.to_vec(),
            modal: kingdom_pool.iter().copied().filter(|&c| !cards::modes(c).is_empty() && !forbidden.contains(&c)).collect(),
            max_gain_rules: 10,
            max_play_rules: 5,
            max_trash_rules: 4,
            max_mode_rules: 3,
            max_conds: 2,
        }
    }

    /// Action cards a candidate can gain (the only cards worth a `[[play]]` rule).
    pub fn actions(&self) -> impl Iterator<Item = CardId> + '_ {
        self.gainable.iter().copied().filter(|&c| is_action(c))
    }

    pub fn max_rules(&self, list: List) -> usize {
        match list {
            List::Gain => self.max_gain_rules,
            List::Play => self.max_play_rules,
            List::Trash => self.max_trash_rules,
            List::Mode => self.max_mode_rules,
            List::Token => self.max_token_rules,
            List::Call => self.max_call_rules,
            List::Debt => self.max_debt_rules,
        }
    }

    /// Make a genome legal and tidy: drop rules for cards outside the space, play rules for
    /// cards the strategy never gains, rules shadowed by an identical earlier rule, and
    /// conditions beyond the cap; clamp thresholds and list lengths. Never leaves the gain list empty.
    pub fn sanitize(&self, g: &mut Genome) {
        g.gain.retain(|r| self.gainable.contains(&r.card));
        let gained: Vec<CardId> = g.gain.iter().map(|r| r.card).collect();
        g.play.retain(|r| is_action(r.card) && gained.contains(&r.card));
        g.trash.retain(|r| self.trashable.contains(&r.card));
        // A mode rule needs a card the strategy gains, offered in this kingdom pool, and a real option.
        g.mode.retain(|r| self.modal.contains(&r.card) && gained.contains(&r.card) && (r.opt as usize) < cards::modes(r.card).len());
        // Token rules name a pile of this kingdom and a token something places; call rules a Reserve card of the pool.
        g.token.retain(|r| self.token_piles.contains(&r.card) && self.token_kinds.contains(&r.opt));
        g.call.retain(|r| self.reserves.contains(&r.card));
        if !self.debt_rules {
            g.debt.clear();
        }
        for list in List::ALL {
            let max = self.max_rules(list);
            let rules = g.list_mut(list);
            for r in rules.iter_mut() {
                // `by(..)` only means something on a trash rule, and only for a card of the pool.
                r.conds.retain(|a| !matches!(a.var, Var::By(c) if list != List::Trash || !self.trashers.contains(&c)));
                tidy_conds(r, self.max_conds);
            }
            // An unconditional rule makes later rules for the same card unreachable.
            let mut out: Vec<Rule> = Vec::with_capacity(rules.len());
            for r in rules.drain(..) {
                // A token rule is shadowed by an earlier one for the same token (any pile) with no condition, or the same pile and condition.
                let same = |p: &Rule| if list == List::Token { p.opt == r.opt && (p.conds.is_empty() || p.card == r.card && p.conds == r.conds) } else { p.card == r.card && (p.conds.is_empty() || p.conds == r.conds) };
                let shadowed = out.iter().any(same);
                if !shadowed {
                    out.push(r);
                }
            }
            out.truncate(max);
            *rules = out;
        }
        if g.gain.is_empty() {
            g.gain.push(Rule::new(id::PROVINCE));
        }
        g.keep_treasure = g.keep_treasure.min(4);
    }
}

fn tidy_conds(r: &mut Rule, max: usize) {
    for a in r.conds.iter_mut() {
        let (lo, hi) = a.var.range();
        a.n = a.n.clamp(lo, hi);
    }
    // One atom per (variable, op): keep the first.
    let mut seen = Vec::new();
    r.conds.retain(|a| {
        let k = (a.var, a.op);
        let dup = seen.contains(&k);
        seen.push(k);
        !dup
    });
    r.conds.truncate(max);
}
