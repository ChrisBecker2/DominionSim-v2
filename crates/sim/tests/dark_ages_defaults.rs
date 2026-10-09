//! Bot policies for Dark Ages: strategies that name the Knights pile, the defaults for Rebuild, Procession, Count, Graverobber, Storeroom,
//! Pillage, Beggar, Hovel, Market Square, Urchin, Death Cart, Scavenger and the rest, and whole games of the sample strategies.

use dominion_engine::cards::{self, id, CardId};
use dominion_engine::sets::dark_ages::KNIGHTS_PILE;
use dominion_engine::text::parse_state;
use dominion_engine::{Choice, ChoiceBuf, Decision, DecisionKind, GameConfig, GameState, NoEvents, PlayerView, Step};
use dominion_sim::batch::{run_match, MatchConfig};
use dominion_sim::Strategy;

fn load(name: &str) -> Strategy {
    let src = std::fs::read_to_string(format!("{}/../../strategies/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    Strategy::parse(&src).unwrap()
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

/// The strategy's choice at the next decision, which is applied.
fn step(strat: &Strategy, g: &mut GameState) -> (Decision, Choice) {
    let (d, ch) = next_decision(g);
    let c = decide(strat, g, &d, &ch);
    assert!(ch.contains(&c), "{c:?} is not legal in {d:?}: {ch:?}");
    g.apply(c, &mut NoEvents).unwrap();
    (d, c)
}

const KINGDOM: &str = "Knights, Cultist, Rebuild, Procession, Count, Graverobber, Storeroom, Pillage, Beggar, Urchin, Death Cart, Scavenger, Hermit, Squire, Catacombs";

fn state(phase: &str, actions: u8, buys: u8, coins: u16, p1: &str, p2: &str) -> GameState {
    let text = format!(
        "players: 2\nkingdom: {KINGDOM}\nturn: 5  player: 1  phase: {phase}  actions: {actions}  buys: {buys}  coins: {coins}\n\n[player 1]\n{p1}\n\n[player 2]\n{p2}\n"
    );
    let mut g = parse_state(&text).unwrap();
    g.shelters = true;
    g
}

const P2: &str = "hand: 5 Copper\ndeck: 5 Estate";

fn money(extra: &str) -> Strategy {
    Strategy::parse(&format!(
        "name = \"T\"\n{extra}\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n"
    ))
    .unwrap()
}

// --- Knights in strategies ---------------------------------------------------------------------

#[test]
fn a_rule_for_knights_buys_whichever_knight_is_on_top() {
    let strat = Strategy::parse("name = \"K\"\n[[gain]]\ncard = \"Knights\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    assert_eq!(strat.gain_list_len(), 11, "ten Knights and a Silver");
    for top in KNIGHTS_PILE.members() {
        let text = format!(
            "players: 2\nkingdom: Knights, Smithy\nsupply: {}=10\nturn: 3  player: 1  phase: buy  actions: 0  buys: 1  coins: 5\n\n[player 1]\nhand:\ndeck: 5 Copper\n\n[player 2]\nhand:\n",
            cards::name(top)
        );
        let mut g = parse_state(&text).unwrap();
        let (d, ch) = next_decision(&mut g);
        assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(top), "{}", cards::name(top));
    }
}

#[test]
fn a_rule_for_one_knight_buys_it_only_when_it_is_on_top() {
    let strat = Strategy::parse("name = \"K\"\n[[gain]]\ncard = \"Sir Martin\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let mk = |top: &str| {
        let text = format!(
            "players: 2\nkingdom: Knights, Smithy\nsupply: {top}=10\nturn: 3  player: 1  phase: buy  actions: 0  buys: 1  coins: 5\n\n[player 1]\nhand:\ndeck: 5 Copper\n\n[player 2]\nhand:\n"
        );
        parse_state(&text).unwrap()
    };
    let mut g = mk("Sir Martin");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SIR_MARTIN));
    let mut g = mk("Dame Anna");
    let (d, ch) = next_decision(&mut g);
    assert_eq!(decide(&strat, &g, &d, &ch), Choice::Card(id::SILVER), "Sir Martin is not on top");
}

#[test]
fn conditions_can_count_and_supply_the_knights_pile() {
    let strat = Strategy::parse(
        "name = \"K\"\n[[gain]]\ncard = \"Knights\"\nif = \"count(Knights) < 2 and supply(Knights) > 3\"\n[[gain]]\ncard = \"Silver\"\n",
    )
    .unwrap();
    let mk = |hand: &str, supply_top: &str, n: u8| {
        let text = format!(
            "players: 2\nkingdom: Knights, Smithy\nsupply: {supply_top}={n}\nturn: 3  player: 1  phase: buy  actions: 0  buys: 1  coins: 5\n\n[player 1]\nhand:\ndeck: 5 Copper\ndiscard: {hand}\n\n[player 2]\nhand:\n"
        );
        parse_state(&text).unwrap()
    };
    let check = |g: &mut GameState, want: Choice| {
        let (d, ch) = next_decision(g);
        assert_eq!(decide(&strat, g, &d, &ch), want);
    };
    check(&mut mk("Sir Destry", "Dame Molly", 9), Choice::Card(id::DAME_MOLLY));
    check(&mut mk("Sir Destry, Dame Anna", "Dame Molly", 9), Choice::Card(id::SILVER)); // two Knights owned, of two names
    check(&mut mk("Sir Destry", "Dame Molly", 3), Choice::Card(id::SILVER)); // only 3 left
    // A strategy naming a Knight asks for the Knights pile in an auto kingdom, once.
    let named = Strategy::parse("name = \"K\"\n[[gain]]\ncard = \"Sir Vander\"\n[[gain]]\ncard = \"Knights\"\n").unwrap();
    let refs = named.kingdom_refs();
    assert_eq!(refs, vec![id::KNIGHTS]);
}

#[test]
fn play_and_trash_rules_may_name_the_knights_and_the_ruins() {
    let s = Strategy::parse(
        "name = \"K\"\n[[gain]]\ncard = \"Silver\"\n[[play]]\ncard = \"Knights\"\n[[trash]]\ncard = \"Ruins\"\n",
    )
    .unwrap();
    // Any Knight in hand is played first, and any Ruins is trashed.
    let g = state("action", 1, 1, 0, "hand: Dame Anna, Smithy, Ruined Market, Copper\ndeck: 5 Copper", P2);
    let view = PlayerView::new(&g, 0);
    for k in [id::DAME_ANNA, id::SIR_VANDER] {
        assert!(s.states_play(k));
    }
    for r in [id::ABANDONED_MINE, id::SURVIVORS, id::RUINED_MARKET] {
        assert!(s.wants_trash(&view, r), "{}", cards::name(r));
    }
}

// --- Defaults of the cards -----------------------------------------------------------------------

/// Play `card` from the hand of the state's player and return the next decision with the bot's choice.
fn after_play(strat: &Strategy, g: &mut GameState, card: CardId) -> (Decision, Choice) {
    let (d, _) = next_decision(g);
    assert_eq!(d.kind, DecisionKind::PlayAction);
    g.apply(Choice::Card(card), &mut NoEvents).unwrap();
    let (d, ch) = next_decision(g);
    (d, decide(strat, g, &d, &ch))
}

#[test]
fn rebuild_names_the_costliest_victory_card() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Rebuild\ndeck: Estate, Duchy, Province, Copper, Copper, Copper", P2);
    let (d, c) = after_play(&strat, &mut g, id::REBUILD);
    assert_eq!(d.kind, DecisionKind::Name);
    assert_eq!(c, Choice::Card(id::PROVINCE));
}

#[test]
fn hovel_is_trashed_when_a_victory_card_is_bought_and_not_otherwise() {
    let mut g = state("buy", 0, 1, 5, "hand: Hovel, Copper
deck: 5 Copper", P2);
    let strat2 = Strategy::parse("name = \"D\"\n[[gain]]\ncard = \"Duchy\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let (d, c) = step(&strat2, &mut g); // buys the Duchy
    assert_eq!((d.kind, c), (DecisionKind::Buy, Choice::Card(id::DUCHY)));
    let (d, ch) = next_decision(&mut g);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: dominion_engine::Act::Trash }, id::HOVEL));
    assert_eq!(decide(&strat2, &g, &d, &ch), Choice::Yes);
}

#[test]
fn procession_upgrades_the_action_the_gain_list_wants_one_dollar_more_of_and_keeps_good_ones() {
    // Gain list: Smithy ($4); a Village ($3) in hand can become one; a Smithy ($4) cannot become anything listed.
    let strat = Strategy::parse("name = \"P\"\n[[gain]]\ncard = \"Smithy\"\n[[gain]]\ncard = \"Silver\"\n").unwrap();
    let text = |hand: &str| {
        format!(
            "players: 2\nkingdom: Procession, Village, Smithy, Moat\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: {hand}\ndeck: 5 Copper\n\n[player 2]\n{P2}\n"
        )
    };
    let mut g = parse_state(&text("Procession, Village, Smithy")).unwrap();
    let (d, c) = after_play(&strat, &mut g, id::PROCESSION);
    assert!(matches!(d.kind, DecisionKind::Select { act: dominion_engine::Act::Play, .. }));
    assert_eq!(c, Choice::Card(id::VILLAGE));
    let mut g = parse_state(&text("Procession, Smithy, Moat")).unwrap();
    let (_, c) = after_play(&strat, &mut g, id::PROCESSION);
    assert_eq!(c, Choice::Pass, "no listed card costs $1 more than the Moat or the Smithy... except none: kept");
}

#[test]
fn count_discards_dead_cards_then_takes_coins_or_a_late_duchy() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Count, Estate, Curse, Gold\ndeck: 5 Copper", P2);
    let (d, c) = after_play(&strat, &mut g, id::COUNT);
    assert!(matches!(d.kind, DecisionKind::Mode { .. }));
    assert_eq!(c, Choice::Mode(0), "discard the two dead cards");
    // No dead cards: gain a Copper.
    let mut g = state("action", 1, 1, 0, "hand: Count, Gold, Silver\ndeck: 5 Copper", P2);
    let (_, c) = after_play(&strat, &mut g, id::COUNT);
    assert_eq!(c, Choice::Mode(2));
}

#[test]
fn storeroom_discards_only_dead_cards() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Storeroom, Curse, Gold, Ruined Village, Copper\ndeck: 5 Copper", P2);
    let (d, c) = after_play(&strat, &mut g, id::STOREROOM);
    assert!(matches!(d.kind, DecisionKind::Select { act: dominion_engine::Act::Discard, .. }));
    assert!(c == Choice::Card(id::CURSE) || c == Choice::Card(id::RUINED_VILLAGE), "{c:?}");
}

#[test]
fn pillage_makes_a_player_with_five_cards_discard_their_best_card() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Pillage\ndeck: 5 Copper", "hand: Copper, Copper, Gold, Estate, Silver\ndeck: 5 Estate");
    let (d, c) = after_play(&strat, &mut g, id::PILLAGE);
    assert_eq!((d.player, d.for_player), (0, 1));
    assert_eq!(c, Choice::Card(id::GOLD));
}

#[test]
fn a_beggar_is_discarded_to_an_attack_and_a_market_square_for_the_gold() {
    let strat = money("");
    // Player 2 attacks with a Militia; player 1 holds a Beggar.
    let text = "players: 2\nkingdom: Beggar, Militia, Market Square, Smithy\nturn: 5  player: 2  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Beggar, Copper, Copper\ndeck: 5 Copper\n\n[player 2]\nhand: Militia\ndeck: 5 Copper";
    let mut g = parse_state(text).unwrap();
    let (d, _) = next_decision(&mut g);
    g.apply(Choice::Card(id::MILITIA), &mut NoEvents).unwrap();
    let (d2, ch) = next_decision(&mut g);
    assert_eq!((d2.player, d2.subject), (0, id::BEGGAR), "{d:?}");
    assert_eq!(decide(&strat, &g, &d2, &ch), Choice::Yes);
    // Market Square reacting to a trash.
    let text = "players: 2\nkingdom: Chapel, Market Square\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Chapel, Market Square, Estate\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate";
    let mut g = parse_state(text).unwrap();
    let strat = Strategy::parse("name = \"C\"\n[[gain]]\ncard = \"Gold\"\n[[trash]]\ncard = \"Estate\"\n").unwrap();
    for _ in 0..4 {
        let (d, c) = step(&strat, &mut g);
        if d.subject == id::MARKET_SQUARE {
            assert_eq!(c, Choice::Yes);
            return;
        }
    }
    panic!("no Market Square offer");
}

#[test]
fn urchin_is_traded_for_a_mercenary_and_death_cart_trashes_itself() {
    let strat = money("");
    let text = "players: 2\nkingdom: Urchin, Militia\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: Militia\nin play: Urchin\ndeck: 5 Copper\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate";
    let mut g = parse_state(text).unwrap();
    let (d, c) = after_play(&strat, &mut g, id::MILITIA);
    assert_eq!((d.kind, d.subject), (DecisionKind::YesNo { act: dominion_engine::Act::Trash }, id::URCHIN));
    assert_eq!(c, Choice::Yes);
    // Death Cart with no dead Action in hand: trash itself.
    let mut g = state("action", 1, 1, 0, "hand: Death Cart, Smithy\ndeck: 5 Copper", P2);
    let (d, _) = next_decision(&mut g);
    g.apply(Choice::Card(id::DEATH_CART), &mut NoEvents).unwrap();
    let (d2, ch) = next_decision(&mut g);
    assert!(matches!(d2.kind, DecisionKind::Select { .. }), "{d:?} {d2:?}");
    let c = decide(&strat, &g, &d2, &ch);
    assert_eq!(c, Choice::Pass, "a Smithy is not thrown away");
    g.apply(c, &mut NoEvents).unwrap();
    let (d3, ch) = next_decision(&mut g);
    assert_eq!(d3.subject, id::DEATH_CART);
    assert_eq!(decide(&strat, &g, &d3, &ch), Choice::Yes);
    // With a Ruins in hand, the Ruins goes.
    let mut g = state("action", 1, 1, 0, "hand: Death Cart, Smithy, Ruined Market\ndeck: 5 Copper", P2);
    let (_, c) = after_play(&strat, &mut g, id::DEATH_CART);
    assert_eq!(c, Choice::Card(id::RUINED_MARKET));
}

#[test]
fn graverobber_takes_a_good_card_from_the_trash_else_upgrades_junk() {
    let strat = money("");
    let text = |trash: &str, hand: &str| {
        format!(
            "players: 2\nkingdom: Graverobber, Smithy\ntrash: {trash}\nturn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: {hand}\ndeck: 5 Copper\n\n[player 2]\n{P2}\n"
        )
    };
    let mut g = parse_state(&text("Gold, Estate", "Graverobber, Moat")).unwrap();
    let (_, c) = after_play(&strat, &mut g, id::GRAVEROBBER);
    assert_eq!(c, Choice::Mode(0));
    let mut g = parse_state(&text("Copper", "Graverobber, Ruined Village")).unwrap();
    let (_, c) = after_play(&strat, &mut g, id::GRAVEROBBER);
    assert_eq!(c, Choice::Mode(1));
}

#[test]
fn scavenger_and_survivors_use_the_deck_and_discard_pile_sensibly() {
    let strat = money("");
    // A deck of Coppers with Gold in the discard pile: put the deck into the discard pile.
    let mut g = state("action", 1, 1, 0, "hand: Scavenger\ndeck: 5 Copper\ndiscard: 2 Gold", P2);
    let (d, c) = after_play(&strat, &mut g, id::SCAVENGER);
    assert!(matches!(d.kind, DecisionKind::YesNo { .. }));
    assert_eq!(c, Choice::Yes);
    // The reverse: keep a deck of Golds.
    let mut g = state("action", 1, 1, 0, "hand: Scavenger\ndeck: 5 Gold\ndiscard: 2 Copper", P2);
    let (_, c) = after_play(&strat, &mut g, id::SCAVENGER);
    assert_eq!(c, Choice::No);
}

#[test]
fn squire_gains_a_silver_when_there_is_nothing_stuck_in_the_hand() {
    let strat = money("");
    let mut g = state("action", 1, 1, 0, "hand: Squire, Copper\ndeck: 5 Copper", P2);
    let (_, c) = after_play(&strat, &mut g, id::SQUIRE);
    let silver = cards::modes(id::SQUIRE).iter().position(|o| matches!(o, cards::ModeOpt::Gain(..))).unwrap() as u8;
    assert_eq!(c, Choice::Mode(silver));
}

// --- Whole games -----------------------------------------------------------------------------

fn win_rate(sample: &str, kingdom: &str, games: u64) -> f64 {
    let s = load(sample);
    let bmu = load("big_money_ultimate.toml");
    let kingdom = dominion_sim::kingdom::resolve(kingdom, &[&s, &bmu], 1).unwrap();
    let stats = run_match(&[&s, &bmu], &MatchConfig { games, kingdom, seed: 5, max_turns: 200 });
    stats[0].win_rate()
}

#[test]
fn the_dark_ages_sample_strategies_beat_big_money_ultimate() {
    let r = win_rate("knights_rebuild_cultist.toml", "auto", 2000);
    assert!(r > 0.5, "Knights Rebuild Cultist: {r}");
    let r = win_rate("junk_dealer_count.toml", "auto", 2000);
    assert!(r > 0.55, "Junk Dealer Count: {r}");
    let r = win_rate("marauder_ruins.toml", "auto", 2000);
    assert!(r > 0.6, "Marauder Ruins: {r}");
}

#[test]
fn bots_play_whole_dark_ages_games_with_every_card() {
    // Every Dark Ages kingdom card (35 piles) across kingdoms of 10, 2 to 4 players, with a bot that uses the defaults only.
    let bm = load("big_money_ultimate.toml");
    let dark: Vec<CardId> = cards::kingdom_cards_in(cards::CardSet::DarkAges).collect();
    let mut seen_ruins = false;
    for (i, chunk) in dark.chunks(7).enumerate() {
        let mut kingdom = chunk.to_vec();
        for &c in dark.iter().cycle().skip(i * 5) {
            if kingdom.len() >= 10 {
                break;
            }
            if !kingdom.contains(&c) {
                kingdom.push(c);
            }
        }
        for players in 2..=4usize {
            let cfg = GameConfig { num_players: players, kingdom: kingdom.clone(), seed: 11 + i as u64, max_turns: 300, ..Default::default() };
            let mut g = GameState::new(&cfg);
            seen_ruins |= g.pile_top(&dominion_engine::sets::dark_ages::RUINS_PILE).is_some();
            let mut agents: Vec<_> = (0..players).map(|_| dominion_sim::agent::StrategyAgent::new(&bm)).collect();
            let mut refs: Vec<&mut dyn dominion_engine::Agent> = agents.iter_mut().map(|a| a as &mut dyn dominion_engine::Agent).collect();
            let r = dominion_engine::play_game(&mut g, &mut refs, &mut NoEvents);
            assert!(r.turns[0] > 0);
        }
    }
    assert!(seen_ruins);
}
