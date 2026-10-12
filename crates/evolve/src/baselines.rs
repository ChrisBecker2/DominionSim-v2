//! Automatic baseline strategies for any kingdom: the anchors of a kingdom league.
//!
//! - Big Money Ultimate (always);
//! - "Big Money + X" for each kingdom card X that a money deck would buy one or two of (Actions
//!   and Treasures, not Victory-only cards), with sensible caps and the usual endgame rules;
//! - "trasher + money" when the kingdom has a trasher;
//! - "village + draw" when it has both a village and a draw card.
//!
//! Every baseline is a [`Genome`], so it prints as an ordinary strategy file.

use dominion_engine::cards::{self, id, CardId, ACTION, TREASURE, VICTORY};

use crate::genome::{Atom, Genome, Op, Rule, Var};
use crate::space::is_action;

/// A named anchor strategy.
#[derive(Clone, Debug)]
pub struct Baseline {
    pub name: String,
    pub genome: Genome,
    /// What the strategy does, in a sentence.
    pub description: String,
}

fn atom(var: Var, op: Op, n: i16) -> Atom {
    Atom { var, op, n }
}

fn rule(card: CardId, conds: Vec<Atom>) -> Rule {
    Rule { card, opt: 0, conds }
}

/// Cards that remove other cards from the deck: a baseline with one of these thins the deck.
const TRASHERS: &[&str] = &[
    "Chapel", "Moneylender", "Remodel", "Trading Post", "Upgrade", "Steward", "Lookout", "Salvager", "Apprentice", "Forge", "Bishop", "Expand",
    "Ambassador", "Trader", "Junk Dealer", "Rats", "Death Cart", "Replace", "Raze", "Develop", "Doctor", "Altar", "Counterfeit", "Spice Merchant",
    "Transmogrify", "Procession", "Graverobber", "Sentry",
];

/// Whether `c` is a card whose main job is to remove cards (the "trasher" role).
pub fn is_trasher(c: CardId) -> bool {
    TRASHERS.contains(&cards::name(c))
}

fn is_terminal(c: CardId) -> bool {
    cards::is(c, ACTION) && cards::def(c).actions == 0
}

/// A village: gives two or more Actions.
pub fn is_village(c: CardId) -> bool {
    cards::is(c, ACTION) && cards::def(c).actions >= 2
}

/// A card that draws two or more cards.
pub fn is_draw(c: CardId) -> bool {
    cards::is(c, ACTION) && cards::def(c).cards >= 2
}

/// Whether Big Money + `c` is a sensible baseline: an Action or Treasure kingdom card (not
/// Victory-only, not a Curse), not payable only with a Potion.
pub fn is_money_partner(c: CardId) -> bool {
    cards::is_kingdom(c)
        && !cards::potion_cost(c)
        && (cards::is(c, ACTION) || cards::is(c, TREASURE))
        && !(cards::is(c, VICTORY) && !cards::is(c, TREASURE) && !cards::is(c, ACTION))
        && !(cards::is(c, VICTORY) && cards::is(c, ACTION))
}

/// Big Money Ultimate for a supply with or without Colony.
pub fn big_money_ultimate(colonies: bool) -> Genome {
    let mut gain = Vec::new();
    if colonies {
        gain.push(Rule::new(id::COLONY));
    }
    gain.push(Rule::new(id::PROVINCE));
    gain.push(rule(id::DUCHY, vec![atom(Var::ProvincesLeft, Op::Lt, 5)]));
    gain.push(rule(id::ESTATE, vec![atom(Var::ProvincesLeft, Op::Lt, 3)]));
    if colonies {
        gain.push(Rule::new(id::PLATINUM));
    }
    gain.push(Rule::new(id::GOLD));
    gain.push(Rule::new(id::SILVER));
    Genome { gain, keep_treasure: 2, ..Genome::default() }
}

/// Insert rules for `x` into `g`: before Gold when it costs $6 or more, otherwise before Silver.
fn insert_rules(g: &mut Genome, x: CardId, rules: Vec<Rule>) {
    let anchor = if cards::cost(x) >= 6 { id::GOLD } else { id::SILVER };
    let at = g.gain.iter().position(|r| r.card == anchor).unwrap_or(g.gain.len());
    for (k, r) in rules.into_iter().enumerate() {
        g.gain.insert(at + k, r);
    }
}

/// The purchase rules for "up to the usual number of X".
fn partner_rules(x: CardId) -> Vec<Rule> {
    let own = |n: i16| atom(Var::Count(x), Op::Lt, n);
    if cards::is(x, ACTION) && is_terminal(x) {
        // One early; a second only once the deck is big enough to carry two terminals.
        vec![rule(x, vec![own(1)]), rule(x, vec![own(2), atom(Var::TotalCards, Op::Ge, 14)])]
    } else {
        vec![rule(x, vec![own(2)])]
    }
}

fn add_play(g: &mut Genome, x: CardId) {
    if is_action(x) && !g.play.iter().any(|r| r.card == x) {
        // Cards that keep the turn going are played before terminals.
        if is_terminal(x) {
            g.play.push(Rule::new(x));
        } else {
            g.play.insert(0, Rule::new(x));
        }
    }
}

/// Big Money + one partner card `x` (see [`partner_rules`]).
pub fn big_money_plus(x: CardId, colonies: bool) -> Genome {
    let mut g = big_money_ultimate(colonies);
    insert_rules(&mut g, x, partner_rules(x));
    add_play(&mut g, x);
    g
}

fn trasher_plan(t: CardId, extra: Option<CardId>, colonies: bool) -> Genome {
    let mut g = big_money_ultimate(colonies);
    insert_rules(&mut g, t, vec![rule(t, vec![atom(Var::Count(t), Op::Lt, 1)])]);
    add_play(&mut g, t);
    if let Some(x) = extra {
        insert_rules(&mut g, x, vec![rule(x, vec![atom(Var::Count(x), Op::Lt, 2)])]);
        add_play(&mut g, x);
    }
    g.trash = vec![rule(id::ESTATE, vec![atom(Var::ProvincesLeft, Op::Ge, 5)]), Rule::new(id::COPPER)];
    g
}

fn engine_plan(village: CardId, draw: CardId, colonies: bool) -> Genome {
    let mut g = big_money_ultimate(colonies);
    let gold = g.gain.iter().position(|r| r.card == id::GOLD).unwrap_or(g.gain.len());
    let silver_at = |g: &Genome| g.gain.iter().position(|r| r.card == id::SILVER).unwrap_or(g.gain.len());
    g.gain.insert(gold, rule(village, vec![atom(Var::Count(village), Op::Lt, 2)]));
    g.gain.insert(gold + 1, rule(draw, vec![atom(Var::Count(draw), Op::Lt, 3)]));
    let s = silver_at(&g);
    g.gain.insert(s, rule(village, vec![atom(Var::Count(village), Op::Lt, 4)]));
    add_play(&mut g, draw);
    add_play(&mut g, village);
    g
}

fn plain_name(c: CardId) -> &'static str {
    cards::name(c)
}

/// The anchors for a kingdom: `kingdom` lists its cards (basics and Events are ignored),
/// `colonies` says whether Colony/Platinum are in the supply, and `forbidden` cards are never used.
pub fn baselines(kingdom: &[CardId], colonies: bool, forbidden: &[CardId]) -> Vec<Baseline> {
    let cards_in: Vec<CardId> = kingdom.iter().copied().filter(|&c| cards::is_kingdom(c) && !forbidden.contains(&c)).collect();
    let mut out: Vec<Baseline> = vec![Baseline {
        name: "Big Money Ultimate".into(),
        genome: big_money_ultimate(colonies),
        description: "Buys money and green cards only (Duchy and Estate late).".into(),
    }];
    let push = |out: &mut Vec<Baseline>, name: String, genome: Genome, description: String| {
        if !out.iter().any(|b| b.genome == genome) {
            out.push(Baseline { name, genome, description });
        }
    };
    for &x in cards_in.iter().filter(|&&c| is_money_partner(c)) {
        push(&mut out, format!("BM + {}", plain_name(x)), big_money_plus(x, colonies), format!("Big Money with up to the usual number of {}.", plain_name(x)));
    }
    // Trasher + money, with the first cantrip/draw card alongside when there is one.
    let helper = cards_in.iter().copied().find(|&c| is_money_partner(c) && cards::is(c, ACTION) && !is_terminal(c) && !is_trasher(c));
    for &t in cards_in.iter().filter(|&&c| is_trasher(c) && is_money_partner(c)) {
        push(
            &mut out,
            format!("Trash with {} + money", plain_name(t)),
            trasher_plan(t, helper, colonies),
            format!("Trashes Coppers (and Estates early) with {}, then buys money.", plain_name(t)),
        );
    }
    // Village + draw engine, for the best draw cards the kingdom has.
    let village = cards_in.iter().copied().find(|&c| is_village(c) && !cards::potion_cost(c));
    if let Some(v) = village {
        let mut draws: Vec<CardId> = cards_in.iter().copied().filter(|&c| is_draw(c) && is_terminal(c) && !cards::potion_cost(c)).collect();
        draws.sort_by_key(|&c| std::cmp::Reverse((cards::def(c).cards, cards::cost(c))));
        for d in draws.into_iter().take(2) {
            push(
                &mut out,
                format!("Engine: {} + {}", plain_name(v), plain_name(d)),
                engine_plan(v, d, colonies),
                format!("{} chain {} for a growing hand, then money and green.", plain_name(v), plain_name(d)),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(b: &[Baseline]) -> Vec<&str> {
        b.iter().map(|b| b.name.as_str()).collect()
    }

    fn kingdom(names: &[&str]) -> Vec<CardId> {
        names.iter().map(|n| cards::by_name(n).unwrap_or_else(|| panic!("{n}"))).collect()
    }

    #[test]
    fn biggest_money_has_bmu_and_a_money_baseline_for_each_useful_card() {
        let k = kingdom(&["Artisan", "Harbinger", "Laboratory", "Mine", "Moneylender", "Bank", "Crystal Ball", "Grand Market", "Mint", "Tiara"]);
        let b = baselines(&k, true, &[]);
        let n = names(&b);
        assert_eq!(n[0], "Big Money Ultimate");
        for want in ["BM + Laboratory", "BM + Mine", "BM + Moneylender", "BM + Bank", "BM + Grand Market", "BM + Mint", "BM + Artisan"] {
            assert!(n.contains(&want), "{n:?} lacks {want}");
        }
        assert!(n.iter().any(|s| s.starts_with("Trash with Moneylender")), "{n:?}");
        assert!(!n.iter().any(|s| s.starts_with("Engine")), "no village or draw terminal: {n:?}");
        // Colony games buy Colony and Platinum.
        let bmu = &b[0].genome;
        assert_eq!(bmu.gain[0].card, id::COLONY);
        assert!(bmu.gain.iter().any(|r| r.card == id::PLATINUM));
    }

    #[test]
    fn every_baseline_is_a_valid_strategy_that_round_trips() {
        for k in [
            kingdom(&["Village", "Smithy", "Laboratory", "Market", "Chapel", "Militia", "Moat", "Witch", "Festival", "Gardens"]),
            kingdom(&["Artisan", "Harbinger", "Laboratory", "Mine", "Moneylender", "Bank", "Crystal Ball", "Grand Market", "Mint", "Tiara"]),
        ] {
            for colonies in [false, true] {
                for b in baselines(&k, colonies, &[]) {
                    let text = b.genome.to_toml(&Default::default());
                    dominion_sim::Strategy::parse(&text).unwrap_or_else(|e| panic!("{}: {e}\n{text}", b.name));
                    assert_eq!(Genome::from_toml(&text).unwrap(), b.genome, "{}", b.name);
                }
            }
        }
    }

    #[test]
    fn an_engine_kingdom_gets_trasher_and_engine_templates_and_no_victory_only_partners() {
        let k = kingdom(&["Village", "Smithy", "Laboratory", "Market", "Chapel", "Militia", "Moat", "Witch", "Festival", "Gardens"]);
        let b = baselines(&k, false, &[id::WITCH]);
        let n = names(&b);
        assert!(n.contains(&"Engine: Village + Smithy"), "{n:?}");
        assert!(n.iter().any(|s| s.starts_with("Trash with Chapel")), "{n:?}");
        assert!(!n.contains(&"BM + Gardens"), "Victory-only");
        assert!(!n.contains(&"BM + Witch"), "forbidden");
        assert!(n.contains(&"BM + Militia"));
        // Big Money, not colony, in a Colony-free kingdom.
        assert!(!b[0].genome.gain.iter().any(|r| r.card == id::COLONY));
        // Terminals get a cap of one early and a second only later.
        let smithy = b.iter().find(|b| b.name == "BM + Smithy").unwrap();
        assert_eq!(smithy.genome.gain.iter().filter(|r| r.card == id::SMITHY).count(), 2);
        assert_eq!(smithy.genome.play, vec![Rule::new(id::SMITHY)]);
    }
}
