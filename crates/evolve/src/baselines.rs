//! Automatic baseline strategies for any kingdom: the anchors of a kingdom league.
//!
//! - Big Money Ultimate (always);
//! - "Big Money + X" for each kingdom card X that a money deck would buy one or two of (Actions
//!   and Treasures, not Victory-only cards), with sensible caps and the usual endgame rules;
//! - "trasher + money" when the kingdom has a trasher;
//! - "village + draw" when it has both a village and a draw card;
//! - "BM + <Event>" for each Event of the kingdom (Landmarks are never bought): Big Money with one
//!   rule buying the Event where a human would (see [`EVENT_HINTS`]), capped with `bought(Event)`
//!   where repeating it is pointless, and with the `[[token]]` rule the Event needs.
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
    // (Bots never take a card that ends the game in a loss: that is the default, no rule needed.)
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

/// Where an Event's buy rule goes in Big Money's list.
#[derive(Clone, Copy)]
enum At {
    /// Before Province/Colony (an Event nothing else outranks).
    Top,
    /// Before Gold: the Event is bought instead of Gold-or-less.
    Gold,
    /// Before Silver: instead of Silver-or-less.
    Silver,
    /// After Silver: only when no card is affordable (the Event is nearly free).
    End,
}

/// A condition of an Event's buy rule.
#[derive(Clone, Copy)]
enum Cond {
    /// `bought(Event) < 1`: once per game (`bought(Event)` is 1 once the Event has been bought, not a count).
    Bought(i16),
    /// `provinces_left <= n`: the endgame.
    ProvincesLeft(i16),
    /// At least n Silvers owned.
    Silvers(i16),
    /// `provinces_left >= n`: not in the endgame.
    EarlyProvinces(i16),
    /// `my_turn < n`.
    TurnLt(i16),
}

/// Per-Event placement hints for "Big Money + Event": (name, position, conditions, token the Event
/// places, as an index of `cards::TOKEN_NAMES`). Events that grant a token also buy a partner Action
/// to put it on (the token pile). Once-per-turn and once-per-game limits are the engine's: the
/// Event is simply not offered again, so they need no condition here; the `Bought` caps are for
/// Events worth buying once (token moves, Inheritance, Mission, Annex, Donate, Tax).
const EVENT_HINTS: &[(&str, At, &[Cond], Option<u8>)] = &[
    // Adventures. Alms ($0, once per turn, only with no Treasure in play) is a free Silver for a poor hand: the bot
    // plays it before its Treasures when it is the first rule the hand reaches, i.e. after Silver on the list.
    ("Alms", At::End, &[], None),
    ("Borrow", At::End, &[], None),
    ("Quest", At::End, &[], None),
    ("Save", At::End, &[], None),
    ("Scouting Party", At::End, &[], None),
    ("Travelling Fair", At::End, &[], None),
    ("Bonfire", At::Silver, &[], None),
    ("Expedition", At::Silver, &[], None),
    ("Ferry", At::Silver, &[Cond::Bought(1)], Some(4)),
    ("Plan", At::Silver, &[Cond::Bought(1)], Some(5)),
    ("Mission", At::Silver, &[Cond::Bought(1)], None),
    ("Pilgrimage", At::Silver, &[], None),
    ("Ball", At::Silver, &[], None),
    ("Raid", At::Silver, &[Cond::Silvers(3)], None),
    ("Seaway", At::Silver, &[Cond::Bought(1)], Some(2)),
    ("Trade", At::Silver, &[Cond::EarlyProvinces(3)], None),
    ("Lost Arts", At::Gold, &[Cond::Bought(1)], Some(1)),
    ("Training", At::Gold, &[Cond::Bought(1)], Some(3)),
    ("Inheritance", At::Gold, &[Cond::Bought(1)], None),
    ("Pathfinding", At::Top, &[Cond::Bought(1)], Some(0)),
    // Empires. The Events with Debt in their cost wait for the endgame or a single early buy.
    ("Advance", At::End, &[], None),
    ("Annex", At::End, &[Cond::Bought(1), Cond::ProvincesLeft(4)], None),
    ("Banquet", At::End, &[], None),
    ("Conquest", At::Gold, &[Cond::ProvincesLeft(4)], None),
    ("Delve", At::Silver, &[], None),
    ("Dominate", At::Top, &[], None),
    ("Donate", At::End, &[Cond::Bought(1), Cond::TurnLt(8)], None),
    ("Ritual", At::End, &[Cond::ProvincesLeft(2)], None),
    ("Salt the Earth", At::Silver, &[Cond::ProvincesLeft(3)], None),
    ("Tax", At::End, &[Cond::Bought(1)], None),
    ("Triumph", At::End, &[Cond::ProvincesLeft(4)], None),
    ("Wedding", At::Silver, &[], None),
    ("Windfall", At::Gold, &[], None),
    // Promo. Summon gains an Action costing up to $4 and plays it at the start of the next turn: like Lost Arts, one buy early, with a cheap Action to gain.
    ("Summon", At::Gold, &[Cond::Bought(1)], None),
    // Rising Sun. Credit is a Gold (or a Market...) for the Debt of its cost, Continue ($0 and 8 Debt) a play of a cheap Action after the buys: both once.
    // Gather is three gains for $7, the rest are small effects that need something in hand or in play and are bought with the $ left over.
    ("Amass", At::End, &[], None),
    ("Asceticism", At::End, &[Cond::Bought(1)], None),
    ("Continue", At::End, &[Cond::Bought(1)], None),
    ("Credit", At::End, &[Cond::Bought(1)], None),
    ("Foresight", At::End, &[], None),
    ("Gather", At::Gold, &[], None),
    ("Kintsugi", At::End, &[], None),
    ("Practice", At::End, &[], None),
    ("Receive Tribute", At::End, &[], None),
    ("Sea Trade", At::End, &[], None),
    // Renaissance Projects (bought once, the engine does not offer an owned one again; a cube is scarce, two per player).
    // Pageant and Canal are good with plain money, the rest are small and wait behind the Silvers.
    ("Academy", At::End, &[], None),
    ("Barracks", At::Silver, &[], None),
    ("Canal", At::Gold, &[], None),
    ("Capitalism", At::End, &[], None),
    ("Cathedral", At::End, &[], None),
    ("Citadel", At::End, &[], None),
    ("City Gate", At::End, &[], None),
    ("Crop Rotation", At::End, &[], None),
    ("Exploration", At::Silver, &[], None),
    ("Fair", At::Silver, &[], None),
    ("Fleet", At::End, &[Cond::ProvincesLeft(3)], None),
    ("Guildhall", At::End, &[], None),
    ("Innovation", At::End, &[], None),
    ("Pageant", At::Gold, &[], None),
    ("Piazza", At::End, &[], None),
    ("Road Network", At::End, &[], None),
    ("Sewers", At::End, &[], None),
    ("Silos", At::End, &[], None),
    ("Sinister Plot", At::Silver, &[], None),
    ("Star Chart", At::End, &[], None),
    // Menagerie. Most are small effects behind the Silvers; Alliance, Populate, Reap and Enclave are the big-hand ones (Alliance and Populate want $10).
    ("Alliance", At::Top, &[], None),
    ("Banish", At::End, &[], None),
    ("Bargain", At::Silver, &[], None),
    ("Commerce", At::End, &[], None),
    ("Delay", At::End, &[], None),
    ("Demand", At::Silver, &[], None),
    ("Desperation", At::End, &[], None),
    ("Enclave", At::Gold, &[], None),
    ("Enhance", At::End, &[], None),
    ("Gamble", At::End, &[], None),
    ("Invest", At::End, &[], None),
    ("March", At::End, &[], None),
    ("Populate", At::Top, &[], None),
    ("Pursue", At::End, &[], None),
    ("Reap", At::Gold, &[], None),
    ("Ride", At::End, &[], None),
    ("Seize the Day", At::End, &[Cond::Bought(1)], None),
    ("Stampede", At::End, &[], None),
    ("Toil", At::End, &[], None),
    ("Transport", At::End, &[], None),
];

fn event_conds(e: CardId, conds: &[Cond]) -> Vec<Atom> {
    conds
        .iter()
        .map(|c| match *c {
            Cond::Bought(n) => atom(Var::Bought(e), Op::Lt, n),
            Cond::ProvincesLeft(n) => atom(Var::ProvincesLeft, Op::Lt, n + 1),
            Cond::Silvers(n) => atom(Var::Count(id::SILVER), Op::Ge, n),
            Cond::EarlyProvinces(n) => atom(Var::ProvincesLeft, Op::Ge, n),
            Cond::TurnLt(n) => atom(Var::MyTurn, Op::Lt, n),
        })
        .collect()
}

/// The Action pile an Event token goes on: a non-terminal if there is one, else any Action; costing at most `max_cost`.
fn token_pile(actions: &[CardId], max_cost: u8) -> Option<CardId> {
    let cheap: Vec<CardId> = actions.iter().copied().filter(|&c| cards::cost(c) <= max_cost).collect();
    cheap.iter().copied().find(|&c| !is_terminal(c)).or_else(|| cheap.first().copied())
}

/// Big Money + the Event `e`, or `None` when the kingdom lacks what the Event needs (an Action to
/// put its token on or to set aside).
pub fn big_money_plus_event(e: CardId, colonies: bool, actions: &[CardId]) -> Option<Genome> {
    let name = cards::name(e);
    let &(_, at, conds, token) = EVENT_HINTS.iter().find(|h| h.0 == name)?;
    let mut g = big_money_ultimate(colonies);
    let mut partner = None;
    if token.is_some() || matches!(name, "Inheritance" | "Summon") {
        // Seaway gains an Action costing up to $4, Inheritance sets one aside and Summon gains one: all need one that cheap.
        let cap = if matches!(name, "Seaway" | "Inheritance" | "Summon") { 4 } else { 8 };
        let p = token_pile(actions, cap)?;
        insert_rules(&mut g, p, partner_rules(p));
        add_play(&mut g, p);
        partner = Some(p);
    }
    let r = rule(e, event_conds(e, conds));
    let pos = |g: &Genome, c: CardId| g.gain.iter().position(|r| r.card == c);
    let index = match at {
        At::Top => 0,
        At::Gold => pos(&g, id::GOLD).unwrap_or(g.gain.len()),
        At::Silver => pos(&g, id::SILVER).unwrap_or(g.gain.len()),
        At::End => pos(&g, id::SILVER).map_or(g.gain.len(), |i| i + 1),
    };
    g.gain.insert(index, r);
    if let (Some(t), Some(p)) = (token, partner) {
        g.token.push(Rule { card: p, opt: t, conds: Vec::new() });
    }
    Some(g)
}

fn plain_name(c: CardId) -> &'static str {
    cards::name(c)
}

/// The anchors for a kingdom: `kingdom` lists its cards (basics and Events are ignored),
/// `colonies` says whether Colony/Platinum are in the supply, and `forbidden` cards are never used.
pub fn baselines(kingdom: &[CardId], colonies: bool, forbidden: &[CardId]) -> Vec<Baseline> {
    let cards_in: Vec<CardId> = kingdom.iter().copied().filter(|&c| cards::is_kingdom(c) && !forbidden.contains(&c)).collect();
    let events_in: Vec<CardId> = kingdom.iter().copied().filter(|&c| cards::is_bought_pseudo(c) && !forbidden.contains(&c)).collect();
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
    // Big Money + each Event (Landmarks are never bought).
    let actions: Vec<CardId> = cards_in.iter().copied().filter(|&c| is_money_partner(c) && cards::is(c, ACTION)).collect();
    for &e in &events_in {
        if let Some(g) = big_money_plus_event(e, colonies, &actions) {
            let on = g.token.first().map(|t| format!(" on {}", plain_name(t.card))).unwrap_or_default();
            push(&mut out, format!("BM + {}{on}", plain_name(e)), g, format!("Big Money that also buys the Event {}.", plain_name(e)));
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
    fn events_of_the_kingdom_get_a_big_money_anchor_and_landmarks_never_do() {
        let k = kingdom(&["Coin of the Realm", "Page", "Peasant", "Port", "Ranger", "Amulet", "Duplicate", "Lost City", "Magpie", "Miser", "Alms", "Ball"]);
        let b = baselines(&k, false, &[]);
        let n = names(&b);
        assert!(n.contains(&"BM + Ball") && n.contains(&"BM + Alms"), "{n:?}");
        let ball = &b.iter().find(|b| b.name == "BM + Ball").unwrap().genome;
        let at = |c| ball.gain.iter().position(|r| r.card == c).unwrap();
        assert!(at(id::BALL) < at(id::SILVER) && at(id::GOLD) < at(id::BALL), "Ball replaces Silver, not Gold");
        let alms = &b.iter().find(|b| b.name == "BM + Alms").unwrap().genome;
        assert_eq!(alms.gain.last().unwrap().card, id::ALMS, "Alms after Silver");
        // No Events, no event anchors.
        let plain = baselines(&kingdom(&["Village", "Smithy", "Laboratory", "Market", "Chapel"]), false, &[]);
        assert!(plain.iter().all(|b| b.genome.gain.iter().all(|r| !cards::is_event(r.card))));
        // Landmarks give no buy rule; Events do.
        let e = baselines(&kingdom(&["Smithy", "Aqueduct", "Delve", "Tomb"]), false, &[]);
        let all_rules: Vec<CardId> = e.iter().flat_map(|b| b.genome.gain.iter().map(|r| r.card)).collect();
        assert!(all_rules.iter().all(|&c| !cards::is_landmark(c)));
        assert!(names(&e).contains(&"BM + Delve"), "{:?}", names(&e));
    }

    #[test]
    fn every_event_anchor_loads_and_plays_whole_games() {
        use dominion_sim::{batch, Strategy};
        let adv = ["Alms", "Borrow", "Quest", "Save", "Scouting Party", "Travelling Fair", "Bonfire", "Expedition", "Ferry", "Plan", "Mission", "Pilgrimage",
            "Ball", "Raid", "Seaway", "Trade", "Lost Arts", "Training", "Inheritance", "Pathfinding"];
        let emp = ["Advance", "Annex", "Banquet", "Conquest", "Delve", "Dominate", "Donate", "Ritual", "Salt the Earth", "Tax", "Triumph", "Wedding", "Windfall", "Summon"];
        let base = baselines(&kingdom(&["Village", "Smithy", "Market", "Port", "Magpie"]), false, &[]);
        let bmu = Strategy::parse(&base[0].genome.to_toml(&Default::default())).unwrap();
        for e in adv.iter().chain(emp.iter()) {
            let mut k = kingdom(&["Village", "Smithy", "Market", "Port", "Magpie", "Peasant", "Page", "Lost City", "Festival", "Militia"]);
            k.push(cards::by_name(e).unwrap());
            let b = baselines(&k, false, &[]);
            let a = b.iter().find(|b| b.name.starts_with(&format!("BM + {e}"))).unwrap_or_else(|| panic!("no anchor for {e}"));
            let text = a.genome.to_toml(&Default::default());
            let s = Strategy::parse(&text).unwrap_or_else(|er| panic!("{e}: {er}\n{text}"));
            assert_eq!(Genome::from_toml(&text).unwrap(), a.genome, "{e}");
            let cfg = batch::MatchConfig { games: 20, kingdom: k.clone(), seed: 5, max_turns: 60 };
            let r = batch::run_match(&[&s, &bmu], &cfg);
            assert_eq!(r[0].games, 20, "{e}");
        }
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
