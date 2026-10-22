//! Bot policies for the Adventures cards: Duplicate's call is searched (a Gold now or a better gain later this turn), Wine Merchant
//! returns to the deck when $2 is left over (the evaluator values it), stated `[[call]]`, `[[exchange]]` and `[[token]]` rules,
//! default token placement, Events in gain lists, and whole games.

use dominion_engine::cards::{self, id};
use dominion_engine::text::parse_state;
use dominion_engine::{Act, Choice, ChoiceBuf, Decision, DecisionKind, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

const KINGDOM: &str = "Duplicate, Wine Merchant, Royal Carriage, Coin of the Realm, Page, Peasant, Ratcatcher, Guide, Smithy, Village";

/// Player 1's turn, with the given zones and turn line; player 2 holds 5 Coppers over 5 Estates.
fn state(phase: &str, coins: u16, buys: u8, p1: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: {phase}  actions: 1  buys: {buys}  coins: {coins}\n\n[player 1]\n{p1}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap()
}

fn next_decision(g: &mut GameState) -> (Decision, Vec<Choice>) {
    let d = match g.advance(&mut NoEvents) {
        Step::Decision(d) => d,
        s => panic!("expected a decision, got {s:?}"),
    };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    (d, buf.as_slice().to_vec())
}

fn decide(strat: &Strategy, g: &GameState, d: &Decision, choices: &[Choice]) -> Choice {
    strat.decide(&PlayerView::new(g, d.player), d, choices)
}

fn strategy(extra: &str, gains: &[&str]) -> Strategy {
    let mut s = format!("name = \"T\"\n{extra}\n");
    for g in gains {
        s.push_str(&format!("[[gain]]\ncard = \"{g}\"\n"));
    }
    Strategy::parse(&s).unwrap()
}

fn money(extra: &str) -> Strategy {
    strategy(extra, &["Province", "Gold", "Silver"])
}

/// Advance to the next decision of the given kind.
fn until(g: &mut GameState, f: impl Fn(&Decision) -> bool) -> (Decision, Vec<Choice>) {
    for _ in 0..50 {
        let (d, ch) = next_decision(g);
        if f(&d) {
            return (d, ch);
        }
        // Pass at the buy; take the first choice elsewhere.
        let c = if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) { Choice::Pass } else { ch[0] };
        g.apply(c, &mut NoEvents).unwrap();
    }
    panic!("no such decision");
}

fn is_call(d: &Decision) -> bool {
    d.kind == (DecisionKind::YesNo { act: Act::Call })
}

// --- Duplicate (searched) ----------------------------------------------------------------------------------

#[test]
fn duplicate_is_called_on_a_gold_when_nothing_better_comes_this_turn() {
    let strat = money("");
    let mut g = state("buy", 6, 1, "hand: Copper\ndeck: 8 Estate\ntavern: Duplicate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, is_call);
    assert_eq!(d.subject, id::DUPLICATE);
    assert_eq!(d.target, id::GOLD);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Yes);
}

#[test]
fn duplicate_is_held_for_a_better_gain_later_in_the_same_turn() {
    // $9 and two buys: a Silver first, then a Gold. Calling on the Silver would use the Duplicate on the lesser card.
    let strat = money("");
    let mut g = state("buy", 9, 2, "hand: Copper\ndeck: 8 Estate\ntavern: Duplicate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::SILVER), &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, is_call);
    assert_eq!(d.target, id::SILVER);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::No, "keep it for the Gold");
    // And when the Gold comes, it is called.
    g.apply(Choice::No, &mut NoEvents).unwrap();
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    assert!(ch.contains(&Choice::Card(id::GOLD)));
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, is_call);
    assert_eq!(d.target, id::GOLD);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Yes);
}

#[test]
fn a_stated_call_rule_overrides_the_search() {
    let never = money("[[call]]\ncard = \"Duplicate\"\nif = \"provinces_left < 0\"\n");
    let mut g = state("buy", 6, 1, "hand: Copper\ndeck: 8 Estate\ntavern: Duplicate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::GOLD), &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, is_call);
    assert_eq!(decide(&never, &g, &d, &ch), Choice::No);
    let always = money("[[call]]\ncard = \"Duplicate\"\n");
    let mut g = state("buy", 9, 2, "hand: Copper\ndeck: 8 Estate\ntavern: Duplicate");
    next_decision(&mut g);
    g.apply(Choice::Card(id::SILVER), &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, is_call);
    assert_eq!(decide(&always, &g, &d, &ch), Choice::Yes, "the rule says call, whatever comes later");
}

// --- Wine Merchant (valued by the evaluator) -----------------------------------------------------------------

#[test]
fn wine_merchant_is_discarded_from_the_mat_when_two_coins_are_left() {
    let strat = money("");
    let mut g = state("buy", 2, 1, "hand: Estate\ndeck: 8 Estate\ntavern: Wine Merchant");
    next_decision(&mut g);
    g.apply(Choice::Pass, &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, |d| d.kind == (DecisionKind::YesNo { act: Act::Discard }) && d.subject == id::WINE_MERCHANT);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Yes);
}

#[test]
fn the_evaluator_prefers_leaving_two_coins_to_return_wine_merchant_over_an_unlisted_copper() {
    // $2, one Buy, a gain list without Copper: the options are to buy nothing (and return the Wine Merchant) or to buy a Copper.
    let strat = money("");
    let mut g = state("buy", 2, 1, "hand: Estate\ndeck: 8 Estate\ntavern: Wine Merchant");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Buy);
    let c = decide(&strat, &g, &d, &ch);
    assert_eq!(c, Choice::Pass, "no Copper: the Wine Merchant is worth more");
}

#[test]
fn a_stated_rule_overrides_the_wine_merchant_default() {
    let keep = money("[[call]]\ncard = \"Wine Merchant\"\nif = \"provinces_left < 0\"\n");
    let mut g = state("buy", 2, 1, "hand: Estate\ndeck: 8 Estate\ntavern: Wine Merchant");
    next_decision(&mut g);
    g.apply(Choice::Pass, &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, |d| d.kind == (DecisionKind::YesNo { act: Act::Discard }) && d.subject == id::WINE_MERCHANT);
    assert_eq!(decide(&keep, &g, &d, &ch), Choice::No);
}

// --- Tokens -----------------------------------------------------------------------------------------------

/// A buy phase with Events in the game; `p1` holds the zones.
fn with_events(events: &str, coins: u16, buys: u8, p1: &str) -> GameState {
    parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nevents: {events}\nturn: 5  player: 1  phase: buy  actions: 1  buys: {buys}  coins: {coins}\n\n[player 1]\n{p1}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap()
}

fn token_decision(strat: &Strategy, event: &str, coins: u16, p1: &str) -> Choice {
    let mut g = with_events(event, coins, 1, p1);
    next_decision(&mut g);
    g.apply(Choice::Card(cards::by_name(event).unwrap()), &mut NoEvents).unwrap();
    let (d, ch) = until(&mut g, |d| matches!(d.kind, DecisionKind::Token { .. }));
    decide(strat, &g, &d, &ch)
}

#[test]
fn a_token_goes_by_default_on_the_action_pile_the_player_owns_most_of() {
    let strat = strategy("", &["Province", "Gold", "Smithy", "Village", "Silver"]);
    // +$1 (Training): any Action pile fits, so the one owned most of.
    let zones = "hand: Copper\ndeck: 4 Estate\ndiscard: 3 Smithy, Village";
    assert_eq!(token_decision(&strat, "Training", 6, zones), Choice::Card(id::SMITHY));
    let zones = "hand: Copper\ndeck: 4 Estate\ndiscard: Smithy, 3 Village";
    assert_eq!(token_decision(&strat, "Training", 6, zones), Choice::Card(id::VILLAGE));
    // +1 Action (Lost Arts): a non-terminal is preferred to the pile owned most of.
    let zones = "hand: Copper\ndeck: 4 Estate\ndiscard: 3 Smithy, Village";
    assert_eq!(token_decision(&strat, "Lost Arts", 6, zones), Choice::Card(id::VILLAGE));
    // +1 Card (Pathfinding): a terminal draw.
    assert_eq!(token_decision(&strat, "Pathfinding", 8, zones), Choice::Card(id::SMITHY));
}

#[test]
fn a_stated_token_rule_places_the_token_and_rule_order_is_priority() {
    let zones = "hand: Copper\ndeck: 4 Estate\ndiscard: 3 Smithy, Village";
    let v = money("[[token]]\ntoken = \"+$1\"\npile = \"Village\"\n");
    assert_eq!(token_decision(&v, "Training", 6, zones), Choice::Card(id::VILLAGE));
    // The first rule whose condition holds wins; a later rule is the fallback.
    let v = money(
        "[[token]]\ntoken = \"+$1\"\npile = \"Village\"\nif = \"count(Village) >= 2\"\n[[token]]\ntoken = \"+$1\"\npile = \"Smithy\"\n",
    );
    assert_eq!(token_decision(&v, "Training", 6, zones), Choice::Card(id::SMITHY));
    // Unicode minus and ASCII minus both name the same token.
    assert!(Strategy::parse("name = \"T\"\n[[token]]\ntoken = \"\u{2212}$2 cost\"\npile = \"Smithy\"\n[[gain]]\ncard = \"Gold\"\n").is_ok());
    assert!(Strategy::parse("name = \"T\"\n[[token]]\ntoken = \"-$2 cost\"\npile = \"Smithy\"\n[[gain]]\ncard = \"Gold\"\n").is_ok());
}

#[test]
fn token_rules_are_checked_when_the_file_is_read() {
    let bad = |t: &str, p: &str| Strategy::parse(&format!("name = \"T\"\n[[token]]\ntoken = \"{t}\"\npile = \"{p}\"\n[[gain]]\ncard = \"Gold\"\n"));
    assert!(bad("+1 Card", "Smithy").is_ok());
    assert!(bad("+7 Cards", "Smithy").unwrap_err().contains("unknown token"));
    assert!(bad("+1 Card", "Nonesuch").unwrap_err().contains("unknown card"));
    assert!(bad("-1 Card", "Smithy").is_err(), "the -1 Card token is not placed on a pile");
}

#[test]
fn token_on_reads_where_the_players_token_is() {
    // Gold only while the +$1 token sits on the Village; otherwise Silver. The token is on the Village in the second state.
    let strat = Strategy::parse(
        "name = \"T\"\n[[gain]]\ncard = \"Gold\"\nif = \"token_on(+$1, Village)\"\n[[gain]]\ncard = \"Silver\"\n",
    )
    .unwrap();
    let mut g = with_events("Training", 6, 1, "hand: Copper\ndeck: 4 Estate\ndiscard: 3 Smithy, Village");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SILVER));
    g.players[0].tok[dominion_engine::state::TOK_COIN] = id::VILLAGE;
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::GOLD));
    g.players[0].tok[dominion_engine::state::TOK_COIN] = id::SMITHY;
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SILVER));
}

// --- Events in gain lists -----------------------------------------------------------------------------------

#[test]
fn an_event_is_bought_only_when_the_gain_list_names_it() {
    let mut g = with_events("Training, Save", 6, 1, "hand: Copper\ndeck: 4 Estate\ndiscard: 3 Smithy, Village");
    let (d, ch) = next_decision(&mut g);
    assert!(ch.contains(&Choice::Card(id::TRAINING)), "the Event is a legal buy");
    assert_eq!(decide(&money(""), &g, &d, &ch), Choice::Card(id::GOLD), "never an Event unless listed");
    let listed = strategy("", &["Province", "Training", "Gold", "Silver"]);
    assert_eq!(decide(&listed, &g, &d, &ch), Choice::Card(id::TRAINING), "list position is the priority");
    let below = strategy("", &["Province", "Gold", "Training", "Silver"]);
    assert_eq!(decide(&below, &g, &d, &ch), Choice::Card(id::GOLD));
}

#[test]
fn a_bought_event_can_be_asked_about_in_a_condition() {
    let once = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Training\"\nif = \"bought(Training) < 1\"\n[[gain]]\ncard = \"Gold\"\n").unwrap();
    let mut g = with_events("Training", 6, 1, "hand: Copper\ndeck: 4 Estate\ndiscard: 3 Smithy, Village");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&once, &g, &d, &ch), Choice::Card(id::TRAINING));
    g.players[0].events_bought.insert((id::TRAINING - cards::FIRST_EVENT) as u32);
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&once, &g, &d, &ch), Choice::Card(id::GOLD));
}

// --- Alms before the Treasures -------------------------------------------------------------------------------

/// The first decision of a Buy phase with Alms in the game and `hand` in the hand (the Treasures are not played yet).
fn alms_decision(hand: &str, buys: u8) -> (GameState, Decision, Vec<Choice>) {
    let mut g = with_events("Alms", 0, buys, &format!("hand: {hand}
deck: 4 Estate
discard: 3 Smithy, Village"));
    let (d, ch) = next_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::PlayTreasure, "Alms is offered before the Treasures: {ch:?}");
    assert!(ch.contains(&Choice::Card(id::ALMS)));
    (g, d, ch)
}

#[test]
fn by_the_rules_alms_comes_before_the_treasures_when_the_gain_list_reaches_it_first() {
    let rules = |gains: &[&str]| strategy("search_play = false", gains);
    // $4 in the hand: Alms (listed before Silver) is the first listed card reached, so it is bought before the Treasures are played.
    let (g, d, ch) = alms_decision("Silver, Copper, Copper", 2);
    assert_eq!(decide(&rules(&["Province", "Gold", "Alms", "Silver"]), &g, &d, &ch), Choice::Card(id::ALMS));
    // Silver is listed first: the Treasures are played and Silver is bought.
    assert_eq!(decide(&rules(&["Province", "Gold", "Silver", "Alms"]), &g, &d, &ch), Choice::Pass);
    // Alms not listed at all: never bought.
    assert_eq!(decide(&rules(&["Province", "Gold", "Silver"]), &g, &d, &ch), Choice::Pass);
    // With $9 in the hand the Province comes first.
    let (g, d, ch) = alms_decision("Gold, Gold, Gold", 2);
    assert_eq!(decide(&rules(&["Province", "Gold", "Alms", "Silver"]), &g, &d, &ch), Choice::Pass);
}

#[test]
fn the_searching_bot_buys_alms_first_when_it_gains_a_card_for_a_spare_buy() {
    // Two Buys and $4: Alms gains a Silver from the gain list, and the money still buys another Silver.
    let (g, d, ch) = alms_decision("Silver, Copper, Copper", 2);
    let listed = strategy("", &["Province", "Gold", "Alms", "Silver"]);
    assert_eq!(decide(&listed, &g, &d, &ch), Choice::Card(id::ALMS));
    // Not listed: the search may not buy it.
    let unlisted = strategy("", &["Province", "Gold", "Silver"]);
    assert_eq!(decide(&unlisted, &g, &d, &ch), Choice::Pass);
}

#[test]
fn a_bot_game_with_alms_listed_plays_to_the_end_and_gains_cards_with_it() {
    use dominion_engine::{play_game, Agent, GameConfig};
    use dominion_sim::StrategyAgent;
    let strat = strategy("", &["Province", "Gold", "Alms", "Silver"]);
    let mut kingdom = dominion_engine::cards::FIRST_GAME.to_vec();
    kingdom.push(id::ALMS);
    let cfg = GameConfig { num_players: 2, kingdom, seed: 3, max_turns: 100, ..Default::default() };
    let mut g = GameState::new(&cfg);
    let mut a = StrategyAgent::new(&strat);
    let mut b = StrategyAgent::new(&strat);
    let mut agents: [&mut dyn Agent; 2] = [&mut a, &mut b];
    let result = play_game(&mut g, &mut agents[..], &mut NoEvents);
    assert!(result.turns[0] > 0);
    assert!(g.players.iter().take(2).any(|p| p.events_bought.has(0)), "Alms was bought");
}

// --- Whole games ---------------------------------------------------------------------------------------------

#[test]
fn games_with_events_tokens_calls_and_exchanges_run_to_the_end() {
    use dominion_sim::batch::{run_match, MatchConfig};
    let rules = Strategy::parse(
        r#"
name = "Rules"
[[gain]]
card = "Province"
[[gain]]
card = "Training"
if = "bought(Training) < 1"
[[gain]]
card = "Lost Arts"
if = "bought(Lost Arts) < 1 and my_turn >= 3"
[[gain]]
card = "Gold"
[[gain]]
card = "Teacher"
if = "count(Teacher) + count(Peasant) < 1"
[[gain]]
card = "Peasant"
if = "count(Peasant) < 1"
[[gain]]
card = "Ratcatcher"
if = "count(Ratcatcher) < 1"
[[gain]]
card = "Wine Merchant"
[[gain]]
card = "Silver"
[[token]]
token = "+$1"
pile = "Silver"
[[token]]
token = "+1 Action"
pile = "Wine Merchant"
if = "count(Wine Merchant) >= 1"
[[call]]
card = "Ratcatcher"
if = "count(Estate) >= 1"
[[exchange]]
card = "Peasant"
if = "my_turn >= 4"
[[play]]
card = "Teacher"
"#,
    )
    .unwrap();
    let bmu = Strategy::parse("name = \"B\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let kingdom = dominion_sim::kingdom::resolve("Peasant,Teacher,Ratcatcher,Wine Merchant,Village,Smithy,Militia,Moat,Cellar,Workshop,Training,Lost Arts", &[&rules, &bmu], 1);
    // Teacher is not a kingdom card: it comes with the Peasant line.
    assert!(kingdom.is_err());
    let kingdom = dominion_sim::kingdom::resolve("Peasant,Ratcatcher,Wine Merchant,Village,Smithy,Militia,Moat,Cellar,Workshop,Market,Training,Lost Arts", &[&rules, &bmu], 1).unwrap();
    let cfg = MatchConfig { games: 300, kingdom, seed: 9, max_turns: 200 };
    let stats = run_match(&[&rules, &bmu], &cfg);
    assert_eq!(stats[0].games, 300);
}

// --- Discarding down to 3 (Samurai) ------------------------------------------------------------------------

/// Player 1 plays Samurai; player 2 holds `hand` and must discard down to 3. Returns what the strategy discards, sorted.
fn samurai_discards(strat: &Strategy, hand: &str) -> Vec<u16> {
    let mut g = parse_state(&format!(
        "players: 2\nkingdom: Samurai, Artificer, Distant Lands, Page, Kitsune, Root Cellar, Aristocrat, Artist, Dungeon, Duplicate\n\
         turn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Samurai\ndeck: 5 Copper\n\n[player 2]\nhand: {hand}\ndeck: 5 Estate\n"
    ))
    .unwrap();
    next_decision(&mut g);
    g.apply(Choice::Card(id::SAMURAI), &mut NoEvents).unwrap();
    let mut out = vec![];
    for _ in 0..6 {
        let (d, ch) = next_decision(&mut g);
        assert_eq!(d.player, 1, "expected the victim's discard, got {d:?}");
        let c = decide(strat, &g, &d, &ch);
        if let Choice::Card(x) = c {
            out.push(x);
        }
        g.apply(c, &mut NoEvents).unwrap();
        if g.players[1].hand.total() <= 3 {
            break;
        }
    }
    out.sort();
    out
}

#[test]
fn discarding_down_to_three_keeps_distant_lands_to_play() {
    let hand = "Artificer, Distant Lands, 3 Copper";
    let full = std::fs::read_to_string(format!("{}/../../strategies/kitsune_bm.toml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    for strat in [money("search_play = false"), money("search_play = true"), Strategy::parse(&full).unwrap()] {
        assert_eq!(samurai_discards(&strat, hand), vec![id::COPPER, id::COPPER], "search={}", strat.search_play);
    }
}

// --- Exchanging a Hero for a Champion ----------------------------------------------------------------------

/// The answer to the exchange of a Hero discarded from play at cleanup, with `deck` as the rest of player 1's cards.
fn hero_exchange(strat: &Strategy, deck: &str) -> Choice {
    let mut g = parse_state(&format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: buy  actions: 0  buys: 1  coins: 0\n\n[player 1]\nhand: Copper\nin play: Hero\ndeck: {deck}\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n"
    ))
    .unwrap();
    let (_, _) = until(&mut g, |d| d.kind == (DecisionKind::YesNo { act: Act::Exchange }));
    let (d, ch) = next_decision(&mut g);
    decide(strat, &g, &d, &ch)
}

#[test]
fn a_hero_is_not_exchanged_for_a_second_champion() {
    for strat in [money("search_play = false"), money("search_play = true")] {
        assert_eq!(hero_exchange(&strat, "5 Copper, 5 Estate"), Choice::Yes, "the first Champion");
        assert_eq!(hero_exchange(&strat, "Champion, 5 Copper"), Choice::No, "a Champion is owned already");
        let rule = strategy("search_play = false\n[[exchange]]\ncard = \"Hero\"\n", &["Province", "Gold", "Silver"]);
        assert_eq!(hero_exchange(&rule, "Champion, 5 Copper"), Choice::Yes, "a stated rule overrides the default");
    }
}

// --- count_line(..) ----------------------------------------------------------------------------------------

#[test]
fn count_line_counts_every_card_of_a_traveller_line() {
    use dominion_sim::expr::Expr;
    let g = state("buy", 0, 1, "hand: Page, Warrior\nin play: Treasure Hunter\ndiscard: Page, Peasant, Soldier\ndeck: Champion, 5 Copper");
    let view = PlayerView::new(&g, 0);
    let eval = |s: &str| Expr::parse(s).unwrap().eval(&view);
    assert_eq!(eval("count_line(Page)"), 5, "2 Pages + Treasure Hunter + Warrior + Champion");
    assert_eq!(eval("count_line(Hero)"), 5, "named by any card of the line");
    assert_eq!(eval("count_line(Peasant)"), 2, "the other line: Peasant + Soldier");
    assert_eq!(eval("count_line(Teacher)"), 2);
    assert_eq!(eval("count(Page)"), 2, "count(Page) is unchanged: Pages only");
    assert_eq!(eval("count_line(Page) < 6 and count(Page) < 3"), 1);
    assert!(Expr::parse("count_line(Silver)").is_err(), "not a Traveller");
    assert!(Expr::parse("count_line(Nonsense)").is_err());
    // A strategy file takes it in a gain condition, and it names the line's first card for the kingdom.
    let capped = Strategy::parse("name = \"T\"\n[[gain]]\ncard = \"Silver\"\nif = \"count_line(Peasant) < 3\"\n[[gain]]\ncard = \"Gold\"\n").unwrap();
    assert!(capped.kingdom_refs().contains(&id::PEASANT));
}
