//! WASM bindings for the browser UI, exposed as plain `extern "C"` functions (no wasm-bindgen).
//!
//! ## Memory / string contract
//!
//! Strings cross the boundary through wasm linear memory:
//! - To pass a string *in* (e.g. `load_state`, `new_game`'s kingdom list): the caller calls
//!   [`alloc`] with the UTF-8 byte length, writes the bytes at the returned pointer, calls the
//!   target function with `(ptr, len)`, then calls [`dealloc(ptr, len)`] itself. This module
//!   never frees a pointer it didn't allocate.
//! - To read a string *out*: after calling a function documented as "writes to the result
//!   buffer", call [`result_ptr`] and [`result_len`] and read that many UTF-8 bytes out of
//!   memory. The result buffer is overwritten by the next such call and is only valid until then.
//! - `u64` parameters (the RNG seed) cross as wasm `i64`/`u64`, which JS callers pass as
//!   `BigInt`.
//!
//! Every mutating export returns `1` on success or `0` on failure; on failure the result buffer
//! holds a human-readable error message (from `dominion_engine::text`'s line-numbered parse
//! errors, or a short message of our own).
//!
//! ## State held here
//!
//! A single game lives in a thread-local (wasm is single-threaded, so this is just an `unsafe`-free
//! way to get a mutable static): the current [`GameState`] (`Copy`, so snapshotting is a memcpy),
//! an undo/redo stack of snapshots, and a rendered text event log.

use dominion_engine::{
    id, Act, Choice, ChoiceBuf, Decision, DecisionKind, Dest, Event, Filter, GameConfig, GameState, Pending, Phase,
    Step, Zone,
};
use dominion_engine::cards;
use dominion_engine::counts::Counts;
use dominion_engine::state::MAX_PLAYERS;
use dominion_engine::rng::Rng;
use dominion_engine::EndReason;
use dominion_engine::PlayerView;
use dominion_search::{NextHandEvaluator, Plan, SearchConfig, Searcher, TaskResult};
use dominion_search::Evaluator;
use dominion_sim::{GainListEvaluator, Strategy};

/// The scoring used to analyze a seat's decision: a strategy seat is judged by its own gain
/// list; human and search seats by the general-purpose evaluator.
enum SeatEval<'a> {
    General(NextHandEvaluator),
    Gains(GainListEvaluator<'a>),
}

impl Evaluator for SeatEval<'_> {
    fn leaf_value(&self, root: &GameState, leaf: &GameState, me: u8) -> f64 {
        match self {
            SeatEval::General(e) => e.leaf_value(root, leaf, me),
            SeatEval::Gains(e) => e.leaf_value(root, leaf, me),
        }
    }

    fn allows(&self, state: &GameState, me: u8, decision: &Decision, choice: Choice) -> bool {
        match self {
            SeatEval::General(e) => e.allows(state, me, decision, choice),
            SeatEval::Gains(e) => e.allows(state, me, decision, choice),
        }
    }
}

/// `strategy`: index into the bundled strategies, or `u32::MAX` for the general evaluator.
fn seat_eval(strategies: &[Strategy], strategy: u32) -> (SeatEval<'_>, String) {
    match strategies.get(strategy as usize) {
        Some(s) => (SeatEval::Gains(GainListEvaluator::new(s)), format!("{}'s gain priorities, rest of turn played by its rules", s.name)),
        None => (SeatEval::General(NextHandEvaluator::default()), "search evaluator (VP + future money)".to_string()),
    }
}

/// Hidden-information samples are seeded from the position itself, so Analyze, Auto-step and
/// Run to end of turn all see the same sampled opponent hands for the same position.
fn position_rng(state: &GameState) -> Rng {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    state.hash(&mut h);
    Rng::new(h.finish())
}

/// A stable id for the current position, so the UI can tell whether an analysis is current.
#[no_mangle]
pub extern "C" fn state_id() -> u32 {
    APP.with(|cell| {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        cell.borrow().state.hash(&mut h);
        h.finish() as u32
    })
}

/// Strategy index whose priorities judge `player`'s decisions (`u32::MAX` = general).
fn scoring_strategy(app: &App, player: u8) -> u32 {
    let seat = app.seats[player as usize];
    if seat >= 2 { seat - 2 } else { u32::MAX }
}
use std::cell::RefCell;

/// Strategies shipped in `strategies/`, compiled in so the page stays a single file.
const STRATEGY_SOURCES: &[&str] = &[
    include_str!("../../../strategies/big_money.toml"),
    include_str!("../../../strategies/big_money_ultimate.toml"),
    include_str!("../../../strategies/smithy_bm.toml"),
    include_str!("../../../strategies/double_witch.toml"),
    include_str!("../../../strategies/militia_bm.toml"),
    include_str!("../../../strategies/moneylender_bm.toml"),
    include_str!("../../../strategies/council_room_bm.toml"),
    include_str!("../../../strategies/laboratory_bm.toml"),
    include_str!("../../../strategies/village_smithy_engine.toml"),
    include_str!("../../../strategies/chapel_witch.toml"),
    include_str!("../../../strategies/gardens_workshop.toml"),
];

/// Seat controller ids: 0 = human, 1 = search, 2.. = STRATEGY_SOURCES[i - 2].
const SEAT_HUMAN: u32 = 0;
const SEAT_SEARCH: u32 = 1;
const DEFAULT_BOT: u32 = 3; // Big Money Ultimate
const DEFAULT_P1: u32 = 5; // Double Witch

const HISTORY_CAP: usize = 1000;
const LOG_CAP: usize = 4000;
const LOG_VIEW_CAP: usize = 200;

struct App {
    state: GameState,
    history: Vec<GameState>,
    redo: Vec<GameState>,
    log: Vec<String>,
    result: Vec<u8>,
    seats: [u32; MAX_PLAYERS],
    strategies: Vec<Strategy>,
    searcher: Searcher,
    search_cfg: SearchConfig,
    /// (event tag, player, log line) of the run the last event started, for condensing.
    log_group: Option<(u8, u8, usize)>,
    /// Parallel analysis in progress: the split tree, results received so far, and the
    /// strategy index whose priorities score it (`u32::MAX` = general evaluator).
    plan: Option<(Plan, Vec<Option<TaskResult>>, u32)>,
    /// State text at the start of turn 1 of the current game (opening hands dealt).
    start_text: String,
}

impl App {
    fn new() -> Self {
        let cfg = GameConfig::default();
        let mut state = GameState::new(&cfg);
        let mut sink: Vec<Event> = Vec::new();
        state.deal_opening_hands(&mut sink);
        let start_text = dominion_engine::format_state(&state);
        let _ = state.advance(&mut sink);
        let strategies = STRATEGY_SOURCES.iter().map(|src| Strategy::parse(src).expect("bundled strategy parses")).collect();
        let search_cfg = SearchConfig { tt_bits: 17, ..SearchConfig::default() };
        let mut seats = [DEFAULT_BOT; MAX_PLAYERS];
        seats[0] = DEFAULT_P1;
        let _ = SEAT_SEARCH;
        let mut app = App {
            state,
            history: Vec::new(),
            redo: Vec::new(),
            log: Vec::new(),
            result: Vec::new(),
            seats,
            strategies,
            searcher: Searcher::new(search_cfg.tt_bits),
            search_cfg,
            log_group: None,
            plan: None,
            start_text,
        };
        for e in &sink {
            app.push_log_event(e);
        }
        app
    }

    fn push_history(&mut self) {
        self.history.push(self.state);
        if self.history.len() > HISTORY_CAP {
            self.history.remove(0);
        }
        self.redo.clear();
    }

    /// Append an event to the log, condensing runs: consecutive draws, treasure plays,
    /// discards, reveals and trashes by the same player share one line
    /// ("Player 1 draws Copper, Copper, Silver").
    fn push_log_event(&mut self, e: &Event) {
        if let Some((tag, player, card)) = groupable(e) {
            if let Some((t, p, line)) = self.log_group {
                if t == tag && p == player && line + 1 == self.log.len() {
                    let last = self.log.last_mut().unwrap();
                    last.push_str(", ");
                    last.push_str(cards::name(card));
                    return;
                }
            }
            self.log.push(render_event(e));
            self.log_group = Some((tag, player, self.log.len() - 1));
        } else {
            self.log.push(render_event(e));
            self.log_group = None;
        }
        if self.log.len() > LOG_CAP {
            let drop_n = self.log.len() - LOG_CAP;
            self.log.drain(0..drop_n);
            self.log_group = None;
        }
    }

    fn set_result(&mut self, s: String) {
        self.result = s.into_bytes();
    }
}

thread_local! {
    static APP: RefCell<App> = RefCell::new(App::new());
}

// -----------------------------------------------------------------------------------------
// Memory / result-buffer plumbing
// -----------------------------------------------------------------------------------------

/// Allocate `len` bytes in wasm linear memory and return a pointer to them. The caller must
/// eventually pass the same `(ptr, len)` to [`dealloc`].
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as u32
}

/// Free a buffer previously returned by [`alloc`]. `len` must be the same length passed to `alloc`.
#[no_mangle]
pub extern "C" fn dealloc(ptr: u32, len: u32) {
    unsafe {
        drop(Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize));
    }
}

/// Pointer to the last written result buffer (state text, JSON view, or an error message).
#[no_mangle]
pub extern "C" fn result_ptr() -> u32 {
    APP.with(|a| a.borrow().result.as_ptr() as u32)
}

/// Length in bytes of the last written result buffer.
#[no_mangle]
pub extern "C" fn result_len() -> u32 {
    APP.with(|a| a.borrow().result.len() as u32)
}

/// Read `len` UTF-8 bytes at `ptr` out of our own linear memory (caller-owned; not freed here).
fn read_str(ptr: u32, len: u32) -> String {
    unsafe {
        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        String::from_utf8_lossy(slice).into_owned()
    }
}

// -----------------------------------------------------------------------------------------
// Game control
// -----------------------------------------------------------------------------------------

/// Start a fresh game. `kingdom` (via `kingdom_ptr`/`kingdom_len`) is a comma-separated list of
/// 10 kingdom card names, e.g. `"Cellar, Market, Merchant, Militia, Mine, Moat, Remodel,
/// Smithy, Village, Workshop"`. `max_turns` of `0` means "use the engine default (200)".
/// Resets undo/redo history and the event log. Writes an error message on failure.
#[no_mangle]
pub extern "C" fn new_game(players: u32, kingdom_ptr: u32, kingdom_len: u32, seed: u64, max_turns: u32) -> i32 {
    let kingdom_text = read_str(kingdom_ptr, kingdom_len);
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let n = players as usize;
        if !(2..=MAX_PLAYERS).contains(&n) {
            app.set_result(format!("player count must be 2..={MAX_PLAYERS}, got {n}"));
            return 0;
        }
        let kingdom = match dominion_engine::parse_kingdom(&kingdom_text) {
            Ok(k) => k,
            Err(e) => {
                app.set_result(e);
                return 0;
            }
        };
        let cfg =
            GameConfig { num_players: n, kingdom, seed, max_turns: if max_turns == 0 { 200 } else { max_turns as u16 } };
        let mut state = GameState::new(&cfg);
        let mut sink: Vec<Event> = Vec::new();
        state.deal_opening_hands(&mut sink);
        app.start_text = dominion_engine::format_state(&state);
        let _ = state.advance(&mut sink);
        app.state = state;
        app.history.clear();
        app.redo.clear();
        app.log.clear();
        for e in &sink {
            app.push_log_event(e);
        }
        app.set_result(String::new());
        1
    })
}

/// Load a game state from text (see `dominion_engine::text`). Resets undo/redo history and
/// the event log. On a parse error, writes the (line-numbered) error message and leaves the
/// current game untouched.
#[no_mangle]
pub extern "C" fn load_state(ptr: u32, len: u32) -> i32 {
    let text = read_str(ptr, len);
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match dominion_engine::parse_state(&text) {
            Ok(state) => {
                app.state = state;
                app.history.clear();
                app.redo.clear();
                app.log.clear();
                app.log.push("Loaded state from text.".to_string());
                let summary = load_summary(&app.state);
                app.log.push(summary);
                // Parsing always clears `pending` (see text.rs docs); advance once to compute
                // the first real decision (or discover the game is already over).
                advance_and_log(&mut app);
                app.set_result(String::new());
                1
            }
            Err(e) => {
                app.set_result(e);
                0
            }
        }
    })
}

/// Writes the current state as human-editable text (see `dominion_engine::text::format_state`)
/// to the result buffer. Always succeeds.
#[no_mangle]
pub extern "C" fn get_state_text() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let text = dominion_engine::format_state(&app.state);
        app.set_result(text);
        1
    })
}

/// Writes a JSON view of the current game to the result buffer (see module docs for shape).
/// Always succeeds.
#[no_mangle]
pub extern "C" fn get_view() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let json = build_view_json(&app.state, &app.log);
        app.set_result(json);
        1
    })
}

/// Apply the `index`-th legal choice for the pending decision (same order `get_view`'s
/// `pending.choices` uses). Advances the engine to the next decision (or game over) and logs
/// the events along the way.
#[no_mangle]
pub extern "C" fn choose(index: u32) -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match choose_impl(&mut app, index as usize) {
            Ok(()) => {
                app.set_result(String::new());
                1
            }
            Err(e) => {
                app.set_result(e);
                0
            }
        }
    })
}

/// Apply a simple default choice for the pending decision (the first legal choice, in the
/// engine's own ordering: lowest-id eligible card, or `Yes` for a yes/no decision; `Pass`/`Done`
/// is only picked automatically when it's the only option).
#[no_mangle]
pub extern "C" fn step_auto() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match step_auto_impl(&mut app) {
            Ok(()) => {
                app.set_result(String::new());
                1
            }
            Err(e) => {
                app.set_result(e);
                0
            }
        }
    })
}

/// Repeatedly apply [`step_auto`]'s default choice until the current player's turn ends
/// (the turn counter advances) or the game ends.
#[no_mangle]
pub extern "C" fn run_to_end_of_turn() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match run_to_end_of_turn_impl(&mut app) {
            Ok(()) => {
                app.set_result(String::new());
                1
            }
            Err(e) => {
                app.set_result(e);
                0
            }
        }
    })
}

/// Undo the last decision. Fails if there is nothing to undo.
#[no_mangle]
pub extern "C" fn undo() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match app.history.pop() {
            Some(prev) => {
                let cur = app.state;
                app.redo.push(cur);
                app.state = prev;
                app.log.push("-- undo --".to_string());
                app.set_result(String::new());
                1
            }
            None => {
                app.set_result("nothing to undo".to_string());
                0
            }
        }
    })
}

/// Redo the last undone decision. Fails if there is nothing to redo.
#[no_mangle]
pub extern "C" fn redo() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match app.redo.pop() {
            Some(next) => {
                let cur = app.state;
                app.history.push(cur);
                app.state = next;
                app.log.push("-- redo --".to_string());
                app.set_result(String::new());
                1
            }
            None => {
                app.set_result("nothing to redo".to_string());
                0
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn can_undo() -> i32 {
    APP.with(|cell| i32::from(!cell.borrow().history.is_empty()))
}

#[no_mangle]
pub extern "C" fn can_redo() -> i32 {
    APP.with(|cell| i32::from(!cell.borrow().redo.is_empty()))
}

// -----------------------------------------------------------------------------------------
// Internal helpers
// -----------------------------------------------------------------------------------------

fn advance_and_log(app: &mut App) {
    let mut sink: Vec<Event> = Vec::new();
    match app.state.advance(&mut sink) {
        Step::Decision(_) | Step::GameOver => {}
        Step::Chance { .. } => unreachable!("chance_mode is never enabled by this crate"),
    }
    for e in &sink {
        app.push_log_event(e);
        if matches!(e, Event::GameOver) {
            for line in game_over_summary(&app.state) {
                app.log.push(line);
            }
            app.log_group = None;
        }
    }
}

/// Where a loaded turn starts: phase, coins/actions/buys already counted, cards already in play.
fn load_summary(state: &GameState) -> String {
    let t = &state.turn;
    let p = t.player as usize;
    let phase = match t.phase {
        Phase::Action => "action phase",
        Phase::Buy => "buy phase",
        Phase::CleanupDraw => "cleanup",
        Phase::Setup => "setup",
        Phase::GameOver => "game over",
    };
    let in_play = dominion_engine::format_counts(&state.players[p].in_play);
    let in_play = if in_play.is_empty() { "nothing".to_string() } else { in_play };
    let fresh = t.coins == 0 && state.players[p].in_play.is_empty();
    format!(
        ":: Turn {} starts{}: Player {}, {phase}, ${} already counted, {} action(s), {} buy(s); in play: {in_play}",
        t.number,
        if fresh { "" } else { " mid-turn" },
        p + 1,
        t.coins,
        t.actions,
        t.buys
    )
}

/// "Provinces ran out" / piles / turn cap, then each player's result, winner(s) first.
fn game_over_summary(state: &GameState) -> Vec<String> {
    let mut out = Vec::new();
    let reason = match state.end_reason() {
        Some(EndReason::ProvincesGone) => "the Province pile is empty".to_string(),
        Some(EndReason::PilesEmpty) => {
            let piles: Vec<&str> = (0..cards::NUM_CARDS as u8)
                .filter(|&c| state.in_supply(c) && state.supply.get(c) == 0)
                .map(cards::name)
                .collect();
            format!("{} supply piles are empty ({})", piles.len(), piles.join(", "))
        }
        Some(EndReason::TurnLimit) => format!("the turn limit ({}) was reached", state.max_turns),
        None => "unknown".to_string(),
    };
    out.push(format!("Game ends because {reason}."));
    let scores = state.scores();
    let winners = state.winners();
    let n = state.num_players as usize;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&p| (winners & (1 << p) == 0, -scores[p], state.players[p].turns_taken));
    let shared = winners.count_ones() > 1;
    for p in order {
        let tag = if winners & (1 << p) != 0 {
            if shared { "  - shared win" } else { "  - WINS" }
        } else {
            ""
        };
        out.push(format!("Player {}: {} VP in {} turns{tag}", p + 1, scores[p], state.players[p].turns_taken));
    }
    out
}

fn apply_choice(app: &mut App, choice: Choice) -> Result<(), String> {
    app.push_history();
    let mut sink: Vec<Event> = Vec::new();
    if let Err(e) = app.state.apply(choice, &mut sink) {
        // Shouldn't happen (callers only pass choices from `legal_choices`), but undo the
        // speculative history push rather than leave a no-op undo step behind.
        app.history.pop();
        return Err(e.to_string());
    }
    for e in &sink {
        app.push_log_event(e);
    }
    advance_and_log(app);
    Ok(())
}

fn choose_impl(app: &mut App, index: usize) -> Result<(), String> {
    if !matches!(app.state.pending(), Pending::Decision(_)) {
        return Err("no decision is pending".to_string());
    }
    let mut buf = ChoiceBuf::default();
    app.state.legal_choices(&mut buf);
    let choices = buf.as_slice();
    if index >= choices.len() {
        return Err(format!("choice index {index} out of range (0..{})", choices.len()));
    }
    apply_choice(app, choices[index])
}

fn step_auto_impl(app: &mut App) -> Result<(), String> {
    if app.state.turn.phase == Phase::GameOver {
        return Err("the game is over".to_string());
    }
    let choice = bot_choice(app, true)?;
    apply_choice(app, choice)
}

/// The choice the deciding seat's controller would make. Human seats get the search's choice
/// when `human_uses_search` (used by Auto-step as a suggestion).
fn bot_choice(app: &mut App, human_uses_search: bool) -> Result<Choice, String> {
    let d = app.state.pending_decision().ok_or("no decision is pending")?;
    let mut buf = ChoiceBuf::default();
    app.state.legal_choices(&mut buf);
    if buf.len() == 1 {
        return Ok(buf.as_slice()[0]);
    }
    let seat = app.seats[d.player as usize];
    if seat >= 2 {
        let view = PlayerView::new(&app.state, d.player);
        return Ok(app.strategies[(seat - 2) as usize].decide(&view, &d, buf.as_slice()));
    }
    if seat == SEAT_HUMAN && !human_uses_search {
        return Err("a human seat is deciding".to_string());
    }
    let world = PlayerView::new(&app.state, d.player).determinize(&mut position_rng(&app.state));
    let a = app.searcher.analyze(&world, d.player, &app.search_cfg, &NextHandEvaluator::default());
    Ok(a.best().choice)
}

fn run_bots_impl(app: &mut App) -> Result<(), String> {
    for step in 0..100_000 {
        let Some(d) = app.state.pending_decision() else { return Ok(()) };
        if app.seats[d.player as usize] == SEAT_HUMAN {
            if step == 0 {
                return Err(format!(
                    "Nothing for bots to do: Player {} is Human and must decide. Set their seat to a bot, or use Auto-step.",
                    d.player + 1
                ));
            }
            return Ok(());
        }
        let choice = bot_choice(app, false)?;
        apply_choice(app, choice)?;
    }
    Err("run_bots: too many steps".to_string())
}

fn analyze_json(app: &mut App) -> Result<String, String> {
    let d = app.state.pending_decision().ok_or("no decision is pending")?;
    let world = PlayerView::new(&app.state, d.player).determinize(&mut position_rng(&app.state));
    let (eval, scoring) = seat_eval(&app.strategies, scoring_strategy(app, d.player));
    let a = app.searcher.analyze(&world, d.player, &app.search_cfg, &eval);
    Ok(analysis_json(app, &d, &a, &scoring, rules_pick(app, &d)))
}

/// What the deciding seat's strategy would choose here (None for human/search seats).
fn rules_pick(app: &App, d: &Decision) -> Option<Choice> {
    let seat = app.seats[d.player as usize];
    let strategy = app.strategies.get(seat.checked_sub(2)? as usize)?;
    let mut buf = ChoiceBuf::default();
    app.state.legal_choices(&mut buf);
    Some(strategy.decide(&PlayerView::new(&app.state, d.player), d, buf.as_slice()))
}

fn analysis_json(app: &App, d: &Decision, a: &dominion_search::Analysis, scoring: &str, pick: Option<Choice>) -> String {
    // A strategy seat's own choice is listed first: it is what Auto-step and Run to end of turn
    // play. Other options follow by value; `better` flags any that would score higher.
    const TIE: f64 = 0.005;
    let pick_ev = pick.and_then(|p| a.options.iter().find(|o| o.choice == p)).map(|o| o.ev);
    let mut options = a.options.clone();
    options.sort_by(|x, y| {
        let (xp, yp) = (Some(x.choice) == pick, Some(y.choice) == pick);
        yp.cmp(&xp).then(y.ev.partial_cmp(&x.ev).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut buf = ChoiceBuf::default();
    app.state.legal_choices(&mut buf);
    let opts: Vec<String> = options
        .iter()
        .map(|o| {
            let index = buf.as_slice().iter().position(|&c| c == o.choice).map_or(-1, |i| i as i64);
            let better = pick_ev.is_some_and(|pe| Some(o.choice) != pick && o.ev > pe + TIE);
            format!(
                "{{\"label\":{},\"index\":{index},\"ev\":{:.4},\"exact\":{},\"rulesPick\":{},\"better\":{better},\"pv\":{}}}",
                jstr(&choice_label(d, o.choice)),
                o.ev,
                o.exact,
                Some(o.choice) == pick,
                jstr(&o.pv)
            )
        })
        .collect();
    format!(
        "{{\"player\":{},\"stateId\":{},\"scoring\":{},\"nodes\":{},\"ttHits\":{},\"options\":[{}]}}",
        d.player,
        {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            app.state.hash(&mut h);
            h.finish() as u32
        },
        jstr(scoring),
        a.nodes,
        a.tt_hits,
        opts.join(",")
    )
}

fn result_of(r: Result<String, String>) -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match r {
            Ok(s) => {
                app.set_result(s);
                1
            }
            Err(e) => {
                app.set_result(e);
                0
            }
        }
    })
}

// -----------------------------------------------------------------------------------------
// Parallel analysis across Web Workers (each worker runs its own instance of this module).
//
// Main instance: plan_start -> plan_root_bytes / plan_task_bytes(i) -> (workers) ->
//                plan_put_result(i, ...) for each task -> plan_finish.
// Worker instance: eval_task(root, state, me, budget) per task.
//
// Game states cross between instances as raw bytes. That is sound only because every instance
// is the same compiled module (identical layout) and the bytes always come from a valid state.
// -----------------------------------------------------------------------------------------

fn state_bytes(s: &GameState) -> Vec<u8> {
    let n = std::mem::size_of::<GameState>();
    let mut v = vec![0u8; n];
    unsafe { std::ptr::copy_nonoverlapping(s as *const GameState as *const u8, v.as_mut_ptr(), n) };
    v
}

fn state_from_ptr(ptr: u32) -> GameState {
    let mut m = std::mem::MaybeUninit::<GameState>::uninit();
    unsafe {
        std::ptr::copy_nonoverlapping(ptr as *const u8, m.as_mut_ptr() as *mut u8, std::mem::size_of::<GameState>());
        m.assume_init()
    }
}

/// Split the pending decision's search tree into about `target_tasks` independent subtrees.
/// Writes JSON {player, tasks}.
#[no_mangle]
pub extern "C" fn plan_start(target_tasks: u32) -> i32 {
    let r = APP.with(|cell| {
        let mut guard = cell.borrow_mut();
        let app = &mut *guard;
        let d = app.state.pending_decision().ok_or("no decision is pending")?;
        let world = PlayerView::new(&app.state, d.player).determinize(&mut position_rng(&app.state));
        let strategy = scoring_strategy(app, d.player);
        let (eval, _) = seat_eval(&app.strategies, strategy);
        let plan = Plan::build(&world, d.player, target_tasks.max(1) as usize, &eval);
        let n = plan.tasks.len();
        app.plan = Some((plan, vec![None; n], strategy));
        Ok(format!("{{\"player\":{},\"tasks\":{n},\"strategy\":{strategy}}}", d.player))
    });
    result_of(r)
}

/// Raw bytes of the plan's root state (the evaluator's reference point) into the result buffer.
#[no_mangle]
pub extern "C" fn plan_root_bytes() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let bytes = match &app.plan {
            Some((p, _, _)) => state_bytes(&p.root),
            None => return 0,
        };
        app.result = bytes;
        1
    })
}

/// Raw bytes of task `i`'s state into the result buffer.
#[no_mangle]
pub extern "C" fn plan_task_bytes(i: u32) -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let bytes = match &app.plan {
            Some((p, _, _)) if (i as usize) < p.tasks.len() => state_bytes(&p.tasks[i as usize]),
            _ => return 0,
        };
        app.result = bytes;
        1
    })
}

/// Size in bytes of a serialized state.
#[no_mangle]
pub extern "C" fn state_size() -> u32 {
    std::mem::size_of::<GameState>() as u32
}

thread_local! {
    static WORKER_SEARCHER: RefCell<Option<Searcher>> = const { RefCell::new(None) };
}

/// Worker side: evaluate one subtree. Writes JSON {ev, exact, nodes, ttHits, pv} where pv is
/// the continuation joined with U+0001.
#[no_mangle]
pub extern "C" fn eval_task(root_ptr: u32, state_ptr: u32, me: u32, budget: u32, strategy: u32) -> i32 {
    let root = state_from_ptr(root_ptr);
    let state = state_from_ptr(state_ptr);
    let cfg = SearchConfig { node_budget: budget as u64, tt_bits: 17, ..SearchConfig::default() };
    let r = APP.with(|app_cell| {
        let app = app_cell.borrow();
        let (eval, _) = seat_eval(&app.strategies, strategy);
        WORKER_SEARCHER.with(|cell| {
            let mut slot = cell.borrow_mut();
            let searcher = slot.get_or_insert_with(|| Searcher::new(cfg.tt_bits));
            searcher.evaluate(&root, &state, me as u8, &cfg, &eval)
        })
    });
    let json = format!(
        "{{\"ev\":{},\"exact\":{},\"nodes\":{},\"ttHits\":{},\"pv\":{}}}",
        r.ev,
        r.exact,
        r.nodes,
        r.tt_hits,
        jstr(&r.pv.join("\u{1}"))
    );
    result_of(Ok(json))
}

/// Main side: record task `i`'s result (pv joined with U+0001, passed as a string).
#[no_mangle]
pub extern "C" fn plan_put_result(i: u32, ev: f64, exact: u32, nodes: f64, tt_hits: f64, pv_ptr: u32, pv_len: u32) -> i32 {
    let pv = read_str(pv_ptr, pv_len);
    let r = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let (_, results, _) = app.plan.as_mut().ok_or("no analysis in progress")?;
        let slot = results.get_mut(i as usize).ok_or("bad task index")?;
        *slot = Some(TaskResult {
            ev,
            exact: exact != 0,
            nodes: nodes as u64,
            tt_hits: tt_hits as u64,
            pv: pv.split('\u{1}').map(str::to_string).collect(),
        });
        Ok(String::new())
    });
    result_of(r)
}

/// Main side: combine all task results. Writes the same JSON as `analyze`.
#[no_mangle]
pub extern "C" fn plan_finish() -> i32 {
    let r = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let (plan, results, strategy) = app.plan.take().ok_or("no analysis in progress")?;
        let results: Vec<TaskResult> = results.into_iter().collect::<Option<Vec<_>>>().ok_or("missing task results")?;
        let a = plan.finish(&results, std::time::Duration::ZERO);
        let d = app.state.pending_decision().ok_or("decision changed during analysis")?;
        let (_, scoring) = seat_eval(&app.strategies, strategy);
        Ok(analysis_json(&app, &d, &a, &scoring, rules_pick(&app, &d)))
    });
    result_of(r)
}

/// State text at the start of turn 1 of the current game (action phase, opening hands dealt,
/// nothing played). Loading it reproduces the game from its first decision.
#[no_mangle]
pub extern "C" fn get_start_text() -> i32 {
    let t = APP.with(|cell| cell.borrow().start_text.clone());
    result_of(Ok(t))
}

/// JSON array of every card: {name, cost, types: ["action", "attack", ...]}.
#[no_mangle]
pub extern "C" fn card_info() -> i32 {
    let flags = [
        (cards::ACTION, "action"),
        (cards::TREASURE, "treasure"),
        (cards::VICTORY, "victory"),
        (cards::CURSE_T, "curse"),
        (cards::ATTACK, "attack"),
        (cards::REACTION, "reaction"),
    ];
    let items: Vec<String> = (0..cards::NUM_CARDS as u8)
        .map(|c| {
            let types: Vec<String> = flags.iter().filter(|(f, _)| cards::is(c, *f)).map(|(_, n)| jstr(n)).collect();
            format!("{{\"name\":{},\"cost\":{},\"types\":[{}]}}", jstr(cards::name(c)), cards::cost(c), types.join(","))
        })
        .collect();
    result_of(Ok(format!("[{}]", items.join(","))))
}

/// JSON array of seat controller names; the index is the id for `set_seat`.
#[no_mangle]
pub extern "C" fn list_bots() -> i32 {
    let names = APP.with(|cell| {
        let app = cell.borrow();
        let mut v = vec![jstr("Human"), jstr("Search (exact turn lookahead)")];
        v.extend(app.strategies.iter().map(|s| jstr(&s.name)));
        format!("[{}]", v.join(","))
    });
    result_of(Ok(names))
}

/// JSON array: controller id for each seat in the current game.
#[no_mangle]
pub extern "C" fn get_seats() -> i32 {
    let s = APP.with(|cell| {
        let app = cell.borrow();
        let n = app.state.num_players as usize;
        format!("[{}]", app.seats[..n].iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))
    });
    result_of(Ok(s))
}

#[no_mangle]
pub extern "C" fn set_seat(player: u32, bot: u32) -> i32 {
    let r = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let max = 2 + app.strategies.len() as u32;
        if player as usize >= MAX_PLAYERS || bot >= max {
            return Err("bad seat or controller".to_string());
        }
        app.seats[player as usize] = bot;
        Ok(String::new())
    });
    result_of(r)
}

/// Let bot seats play until a human seat must decide or the game ends.
#[no_mangle]
pub extern "C" fn run_bots() -> i32 {
    let r = APP.with(|cell| run_bots_impl(&mut cell.borrow_mut()).map(|_| String::new()));
    result_of(r)
}

/// Exact within-turn search of the pending decision, from the decider's honest view.
/// Writes JSON: {player, nodes, ttHits, options: [{label, ev, exact, pv}]} (best first).
#[no_mangle]
pub extern "C" fn analyze() -> i32 {
    let r = APP.with(|cell| analyze_json(&mut cell.borrow_mut()));
    result_of(r)
}

fn run_to_end_of_turn_impl(app: &mut App) -> Result<(), String> {
    let start_turn = app.state.turn.number;
    let mut steps = 0u32;
    loop {
        if app.state.turn.phase == Phase::GameOver || app.state.turn.number != start_turn {
            return Ok(());
        }
        step_auto_impl(app)?;
        steps += 1;
        if steps > 200_000 {
            return Err("run_to_end_of_turn: too many steps, aborting".to_string());
        }
    }
}

// -----------------------------------------------------------------------------------------
// Event log rendering
// -----------------------------------------------------------------------------------------

/// Events that condense into one line when repeated: (tag, player, card).
fn groupable(e: &Event) -> Option<(u8, u8, u8)> {
    match *e {
        Event::Draw { player, card } => Some((0, player, card)),
        Event::Play { player, card } if cards::is(card, cards::TREASURE) => Some((1, player, card)),
        Event::Discard { player, card } => Some((2, player, card)),
        Event::Reveal { player, card } => Some((3, player, card)),
        Event::Trash { player, card } => Some((4, player, card)),
        _ => None,
    }
}

fn render_event(e: &Event) -> String {
    match *e {
        Event::TurnStart { player, turn } => format!("--- Turn {turn}: Player {} ---", player + 1),
        Event::Shuffle { player } => format!("Player {} shuffles their discard into their deck", player + 1),
        Event::Draw { player, card } => format!("Player {} draws {}", player + 1, cards::name(card)),
        Event::Play { player, card } => format!("Player {} plays {}", player + 1, cards::name(card)),
        Event::Buy { player, card } => format!("Player {} buys {}", player + 1, cards::name(card)),
        Event::Gain { player, card, to } => {
            let dest = match to {
                Dest::Discard => "discard pile",
                Dest::Hand => "hand",
                Dest::DeckTop => "deck (on top)",
            };
            format!("Player {} gains {} to their {}", player + 1, cards::name(card), dest)
        }
        Event::Trash { player, card } => format!("Player {} trashes {}", player + 1, cards::name(card)),
        Event::Discard { player, card } => format!("Player {} discards {}", player + 1, cards::name(card)),
        Event::Topdeck { player, card } => format!("Player {} puts {} on top of their deck", player + 1, cards::name(card)),
        Event::Reveal { player, card } => format!("Player {} reveals {}", player + 1, cards::name(card)),
        Event::SetAside { player, card } => format!("Player {} sets aside {}", player + 1, cards::name(card)),
        Event::Reaction { player, card } => format!("Player {} reveals {} (reaction)", player + 1, cards::name(card)),
        Event::GameOver => "--- Game over ---".to_string(),
        Event::PlayAgain { player, card, source, nth } => {
            let again = if nth == 2 { "again".to_string() } else { format!("for the {} time", ordinal(nth)) };
            format!("Player {} plays {} {again} ({})", player + 1, cards::name(card), cards::name(source))
        }
        Event::PhaseStart { player, phase } => {
            let name = match phase {
                Phase::Action => "Action",
                Phase::Buy => "Buy",
                Phase::CleanupDraw => "Cleanup",
                Phase::Setup => "Setup",
                Phase::GameOver => "Game over",
            };
            format!(":: {name} phase (Player {})", player + 1)
        }
    }
}

// -----------------------------------------------------------------------------------------
// Decision descriptions and choice labels (generic over DecisionKind, not per-card)
// -----------------------------------------------------------------------------------------


fn zone_phrase(z: Zone) -> &'static str {
    match z {
        Zone::Hand => " from your hand",
        Zone::Discard => " from your discard",
        Zone::Revealed => " from the revealed cards",
    }
}


fn dest_phrase(d: Dest) -> &'static str {
    match d {
        Dest::Hand => " to your hand",
        Dest::DeckTop => " onto your deck",
        Dest::Discard => "",
    }
}


fn ordinal(n: u8) -> String {
    match n {
        1 => "1st".into(),
        2 => "2nd".into(),
        3 => "3rd".into(),
        _ => format!("{n}th"),
    }
}

fn times_phrase(n: u8) -> String {
    match n {
        0 | 1 => String::new(),
        2 => " twice".into(),
        3 => " three times".into(),
        _ => format!(" {n} times"),
    }
}

/// Noun for a filtered card, singular and plural ("an Action card" / "Action cards").
fn filter_noun(f: Filter) -> (String, String) {
    match f {
        Filter::Any => ("a card".into(), "cards".into()),
        Filter::Action => ("an Action card".into(), "Action cards".into()),
        Filter::Treasure => ("a Treasure".into(), "Treasures".into()),
        Filter::Victory => ("a Victory card".into(), "Victory cards".into()),
        Filter::NonCopperTreasure => ("a Treasure other than Copper".into(), "Treasures other than Copper".into()),
        Filter::Card(c) => (format!("a {}", cards::name(c)), format!("{}s", cards::name(c))),
    }
}

/// Card-text style description, generic over the decision's shape:
/// "Player 1 - Throne Room: You may play an Action card from your hand twice."
fn decision_description(state: &GameState, d: &Decision) -> String {
    let who = format!("Player {}", d.player + 1);
    let head = match d.source {
        Some(c) => format!("{who} \u{2014} {}: ", cards::name(c)),
        None => format!("{who}: "),
    };
    let body = match d.kind {
        DecisionKind::PlayAction => format!("You may play an Action card ({} action(s) left).", state.turn.actions),
        DecisionKind::Buy => format!("You may buy a card (${} available, {} buy(s) left).", state.turn.coins, state.turn.buys),
        DecisionKind::Gain { max_cost, filter, dest } => {
            let (one, _) = filter_noun(filter);
            format!("Gain {one} costing up to ${max_cost}{}.", dest_phrase(dest))
        }
        DecisionKind::Select { from, act, filter, min, max, ordered } => {
            let (one, many) = filter_noun(filter);
            let what = if max >= 100 {
                format!("any number of {many}")
            } else if max == 1 {
                one
            } else if min == max {
                format!("{max} {many}")
            } else {
                format!("up to {max} {many}")
            };
            let (verb, tail) = match act {
                Act::Discard => ("discard", String::new()),
                Act::Trash => ("trash", String::new()),
                Act::Topdeck => ("put", " onto your deck".to_string()),
                Act::Play => ("play", times_phrase(d.play_times)),
                Act::SetAside => ("set aside", String::new()),
            };
            let zone = zone_phrase(from);
            let order = if ordered && act == Act::Topdeck { " (one at a time; the last one ends on top)" } else { "" };
            let upgrade = match d.upgrade {
                Some(u) => format!(" Then gain {} costing up to ${} more than it{}.", filter_noun(u.filter).0, u.plus, dest_phrase(u.dest)),
                None => String::new(),
            };
            let may = if min == 0 { "You may " } else { "" };
            let verb = if min == 0 { verb.to_string() } else { capitalize(verb) };
            format!("{may}{verb} {what}{zone}{tail}{order}.{upgrade}")
        }
        DecisionKind::YesNo { act } => {
            let card = cards::name(d.subject);
            match act {
                Act::Discard => format!("Discard {card}?"),
                Act::Trash => format!("Trash {card}?"),
                Act::Topdeck => format!("Put {card} onto your deck?"),
                Act::Play => format!("You may play {card}. Play it?"),
                Act::SetAside => format!("You may set aside {card} (skip drawing it). Set it aside?"),
            }
        }
    };
    format!("{head}{body}")
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn choice_label(d: &Decision, c: Choice) -> String {
    match c {
        Choice::Pass => "Done".to_string(),
        Choice::Yes => "Yes".to_string(),
        Choice::No => "No".to_string(),
        Choice::Card(card) => {
            let name = cards::name(card);
            match d.kind {
                DecisionKind::PlayAction => format!("Play {name}"),
                DecisionKind::Buy => format!("Buy {name}"),
                DecisionKind::Gain { .. } => format!("Gain {name}"),
                DecisionKind::Select { act, .. } => match act {
                    Act::Discard => format!("Discard {name}"),
                    Act::Trash => format!("Trash {name}"),
                    Act::Topdeck => format!("Put {name} on deck"),
                    Act::Play if d.play_times > 1 => format!("Play {name} (x{})", d.play_times),
                    Act::Play => format!("Play {name}"),
                    Act::SetAside => format!("Set aside {name}"),
                },
                DecisionKind::YesNo { .. } => name.to_string(),
            }
        }
    }
}

// -----------------------------------------------------------------------------------------
// JSON
// -----------------------------------------------------------------------------------------

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn jstr(s: &str) -> String {
    format!("\"{}\"", esc(s))
}

/// `[{"id":0,"name":"Copper","count":3}, ...]` in ascending card-id order.
fn counts_json(c: &Counts) -> String {
    let items: Vec<String> =
        c.iter().map(|(id, n)| format!("{{\"id\":{id},\"name\":{},\"count\":{n}}}", jstr(cards::name(id)))).collect();
    format!("[{}]", items.join(","))
}

/// `["Gold","Silver","Silver"]`, one entry per card, in order (first = top for a deck).
fn sequence_json(iter: impl Iterator<Item = u8>) -> String {
    let items: Vec<String> = iter.map(|id| jstr(cards::name(id))).collect();
    format!("[{}]", items.join(","))
}

fn supply_json(state: &GameState) -> String {
    let mut ids: Vec<u8> = (0..cards::NUM_CARDS as u8).filter(|&c| state.in_supply(c)).collect();
    // Victory, then treasure (high to low), then Curse, then kingdom cards cheapest first.
    const BASE_ORDER: [u8; 7] = [id::PROVINCE, id::DUCHY, id::ESTATE, id::GOLD, id::SILVER, id::COPPER, id::CURSE];
    ids.sort_by_key(|&c| match BASE_ORDER.iter().position(|&b| b == c) {
        Some(i) => (0, i as u8, ""),
        None => (1, cards::cost(c), cards::name(c)),
    });
    let items: Vec<String> = ids
        .iter()
        .map(|&c| {
            format!(
                "{{\"id\":{c},\"name\":{},\"cost\":{},\"count\":{}}}",
                jstr(cards::name(c)),
                cards::cost(c),
                state.supply.get(c)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn player_json(state: &GameState, p: usize) -> String {
    let ps = &state.players[p];
    format!(
        concat!(
            "{{\"index\":{p},\"isCurrent\":{cur},",
            "\"hand\":{hand},\"handSize\":{hand_n},",
            "\"deckTop\":{deck_top},\"deckUnknown\":{deck_unk},\"deckSize\":{deck_n},",
            "\"discard\":{discard},\"inPlay\":{in_play},\"setAside\":{set_aside},",
            "\"vp\":{vp},\"turnsTaken\":{turns}}}"
        ),
        p = p,
        cur = state.turn.player as usize == p,
        hand = counts_json(&ps.hand),
        hand_n = ps.hand.total(),
        deck_top = sequence_json(ps.deck_known.iter_top_down()),
        deck_unk = counts_json(&ps.deck_unknown),
        deck_n = ps.deck_size(),
        discard = counts_json(&ps.discard),
        in_play = counts_json(&ps.in_play),
        set_aside = counts_json(&ps.set_aside),
        vp = ps.vp(),
        turns = ps.turns_taken,
    )
}

fn pending_json(state: &GameState) -> String {
    let d = match state.pending_decision() {
        Some(d) => d,
        None => return "null".to_string(),
    };
    let mut buf = ChoiceBuf::default();
    state.legal_choices(&mut buf);
    let choices: Vec<String> = buf
        .as_slice()
        .iter()
        .enumerate()
        .map(|(i, &c)| format!("{{\"index\":{i},\"label\":{}}}", jstr(&choice_label(&d, c))))
        .collect();
    format!(
        "{{\"player\":{},\"source\":{},\"description\":{},\"choices\":[{}]}}",
        d.player,
        d.source.map(|c| jstr(cards::name(c))).unwrap_or_else(|| "null".to_string()),
        jstr(&decision_description(state, &d)),
        choices.join(",")
    )
}

fn phase_json(p: Phase) -> &'static str {
    match p {
        Phase::Setup => "setup",
        Phase::Action => "action",
        Phase::Buy => "buy",
        Phase::CleanupDraw => "cleanup",
        Phase::GameOver => "gameover",
    }
}

fn build_view_json(state: &GameState, log: &[String]) -> String {
    let n = state.num_players as usize;
    let players: Vec<String> = (0..n).map(|p| player_json(state, p)).collect();
    let scores = state.scores();
    let scores_json: Vec<String> = (0..n).map(|p| scores[p].to_string()).collect();
    let game_over = state.turn.phase == Phase::GameOver;
    let winners_json: String = if game_over {
        let mask = state.winners();
        let ws: Vec<String> = (0..n).filter(|&p| mask & (1 << p) != 0).map(|p| p.to_string()).collect();
        format!("[{}]", ws.join(","))
    } else {
        "[]".to_string()
    };
    let log_start = log.len().saturating_sub(LOG_VIEW_CAP);
    let log_json: Vec<String> = log[log_start..].iter().map(|l| jstr(l)).collect();

    format!(
        concat!(
            "{{\"numPlayers\":{n},\"currentPlayer\":{cur},",
            "\"turn\":{{\"number\":{tn},\"player\":{tp},\"phase\":{phase},\"actions\":{ta},\"buys\":{tb},\"coins\":{tc}}},",
            "\"supply\":{supply},\"trash\":{trash},\"players\":[{players}],",
            "\"scores\":[{scores}],\"gameOver\":{go},\"winners\":{winners},",
            "\"pending\":{pending},\"log\":[{log}]}}"
        ),
        n = n,
        cur = state.turn.player,
        tn = state.turn.number,
        tp = state.turn.player,
        phase = jstr(phase_json(state.turn.phase)),
        ta = state.turn.actions,
        tb = state.turn.buys,
        tc = state.turn.coins,
        supply = supply_json(state),
        trash = counts_json(&state.trash),
        players = players.join(","),
        scores = scores_json.join(","),
        go = game_over,
        winners = winners_json,
        pending = pending_json(state),
        log = log_json.join(","),
    )
}
