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

    fn value_when_nothing_allowed(&self) -> Option<f64> {
        match self {
            SeatEval::General(e) => e.value_when_nothing_allowed(),
            SeatEval::Gains(e) => e.value_when_nothing_allowed(),
        }
    }

    fn policy(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        match self {
            SeatEval::General(e) => e.policy(state, me, decision, choices),
            SeatEval::Gains(e) => e.policy(state, me, decision, choices),
        }
    }

    fn playout_choice(&self, state: &GameState, me: u8, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
        match self {
            SeatEval::General(e) => e.playout_choice(state, me, decision, choices),
            SeatEval::Gains(e) => e.playout_choice(state, me, decision, choices),
        }
    }

    fn outcome_shows_play(&self, card: u8) -> bool {
        match self {
            SeatEval::General(e) => e.outcome_shows_play(card),
            SeatEval::Gains(e) => e.outcome_shows_play(card),
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
    if seat >= FIRST_STRATEGY_SEAT { seat - FIRST_STRATEGY_SEAT } else { u32::MAX }
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

/// Seat controller ids: 0 = search, 1.. = STRATEGY_SOURCES[i - 1].
const FIRST_STRATEGY_SEAT: u32 = 1;
const DEFAULT_BOT: u32 = FIRST_STRATEGY_SEAT + 1; // Big Money Ultimate
const DEFAULT_P1: u32 = FIRST_STRATEGY_SEAT + 3; // Double Witch

const HISTORY_CAP: usize = 1000;

/// Records events with the nesting depth the engine reports (effects indented under their card).
#[derive(Default)]
struct LogSink {
    events: Vec<(Event, u8)>,
    depth: u8,
}

impl dominion_engine::EventSink for LogSink {
    fn event(&mut self, e: Event) {
        self.events.push((e, self.depth));
    }
    fn depth(&mut self, depth: u8) {
        self.depth = depth;
    }
}

/// Per-turn record of the game being played in the page (for the charts after Run Game).
/// Values are written per (player, own turn number), so undo/redo just overwrite them.
#[derive(Default, Clone)]
struct GameRecord {
    vp: Vec<Vec<f64>>,
    money: Vec<Vec<f64>>,
    buys: Vec<Vec<f64>>,
    /// The turn in progress: player, global turn number, coins spent, cards bought, and the most
    /// coins seen in its buy phase (`leftover`).
    cur_player: usize,
    cur_number: u16,
    spent: u32,
    bought: u32,
    leftover: u32,
    over: bool,
}

impl GameRecord {
    fn new(state: &GameState) -> Self {
        let n = state.num_players as usize;
        GameRecord {
            vp: vec![Vec::new(); n],
            money: vec![Vec::new(); n],
            buys: vec![Vec::new(); n],
            cur_player: state.turn.player as usize,
            cur_number: state.turn.number,
            leftover: state.turn.coins as u32,
            ..Default::default()
        }
    }

    fn set(v: &mut Vec<f64>, i: usize, x: f64) {
        if v.len() <= i {
            v.resize(i + 1, f64::NAN);
        }
        v[i] = x;
    }

    fn on_event(&mut self, e: &Event) {
        if let Event::Buy { card, .. } = e {
            self.bought += 1;
            self.spent += cards::cost(*card) as u32;
        }
    }

    /// After the state moved on: close out a finished turn, remember coins still unspent.
    fn after_step(&mut self, s: &GameState) {
        if s.turn.number != self.cur_number || s.is_game_over() {
            if !self.over {
                let p = self.cur_player;
                if p < self.vp.len() {
                    let t = s.players[p].turns_taken.max(1) as usize - 1;
                    Self::set(&mut self.vp[p], t, s.players[p].vp() as f64);
                    // Coins available in the buy phase (the most seen; spending only lowers it).
                    Self::set(&mut self.money[p], t, self.leftover.max(self.spent) as f64);
                    Self::set(&mut self.buys[p], t, self.bought as f64);
                }
            }
            self.over = s.is_game_over();
            self.cur_player = s.turn.player as usize;
            self.cur_number = s.turn.number;
            self.spent = 0;
            self.bought = 0;
            self.leftover = 0;
        } else if s.turn.phase == Phase::Buy {
            self.leftover = self.leftover.max(s.turn.coins as u32);
        }
    }

    /// Same JSON shape as `simulate` (one game), so the page charts it the same way.
    fn json(&self, s: &GameState) -> String {
        let n = self.vp.len();
        let arr = |v: &[f64]| format!("[{}]", v.iter().map(|x| if x.is_nan() { "0".to_string() } else { format!("{x}") }).collect::<Vec<_>>().join(","));
        let count = |v: &[f64]| format!("[{}]", v.iter().map(|x| if x.is_nan() { "0" } else { "1" }).collect::<Vec<_>>().join(","));
        let winners = if s.is_game_over() { s.winners() } else { 0 };
        let k = winners.count_ones().max(1) as f64;
        let players: Vec<String> = (0..n)
            .map(|p| {
                let won = winners & (1 << p) != 0;
                let mut wins_at = vec![0f64; s.players[p].turns_taken as usize];
                if won && !wins_at.is_empty() {
                    let last = wins_at.len() - 1;
                    wins_at[last] = 1.0 / k;
                }
                format!(
                    "{{\"vp\":{},\"money\":{},\"buys\":{},\"count\":{},\"wins\":{},\"winsAt\":{}}}",
                    arr(&self.vp[p]),
                    arr(&self.money[p]),
                    arr(&self.buys[p]),
                    count(&self.vp[p]),
                    if won { 1.0 / k } else { 0.0 },
                    arr(&wins_at)
                )
            })
            .collect();
        let length = (0..n).map(|p| s.players[p].turns_taken).max().unwrap_or(0);
        format!(
            "{{\"games\":1,\"capped\":{},\"lengthSum\":{},\"finished\":{},\"players\":[{}]}}",
            u8::from(!s.is_game_over()),
            if s.is_game_over() { length } else { 0 },
            s.is_game_over(),
            players.join(",")
        )
    }
}

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
    /// Turn-by-turn record of the current game (reset by New game / Load state).
    record: GameRecord,
}

impl App {
    fn new() -> Self {
        let cfg = GameConfig::default();
        let mut state = GameState::new(&cfg);
        let mut sink = LogSink::default();
        state.deal_opening_hands(&mut sink);
        let start_text = dominion_engine::format_state(&state);
        // Wait at the start of turn 1 ("Start turn"), like every other turn boundary.
        state.pause_at_turn_start = true;
        let strategies = STRATEGY_SOURCES.iter().map(|src| Strategy::parse(src).expect("bundled strategy parses")).collect();
        let search_cfg = SearchConfig { tt_bits: 17, ..SearchConfig::default() };
        let mut seats = [DEFAULT_BOT; MAX_PLAYERS];
        seats[0] = DEFAULT_P1;
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
            record: GameRecord::default(),
        };
        app.record = GameRecord::new(&app.state);
        for (e, depth) in &sink.events {
            app.push_log_event(e, *depth);
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
    fn push_log_event(&mut self, e: &Event, depth: u8) {
        self.record.on_event(e);
        // Effects are indented under the card that caused them (non-breaking spaces survive HTML).
        let indent = "\u{a0}\u{a0}\u{a0}".repeat(depth as usize);
        if let Some((tag, player, card)) = groupable(e) {
            // Runs only condense at the same depth.
            let tag = tag * 16 + depth.min(15);
            if let Some((t, p, line)) = self.log_group {
                if t == tag && p == player && line + 1 == self.log.len() {
                    let last = self.log.last_mut().unwrap();
                    last.push_str(", ");
                    last.push_str(cards::name(card));
                    return;
                }
            }
            self.log.push(format!("{indent}{}", render_event(e)));
            self.log_group = Some((tag, player, self.log.len() - 1));
        } else {
            self.log.push(format!("{indent}{}", render_event(e)));
            self.log_group = None;
        }
    }

    fn set_result(&mut self, s: String) {
        self.result = s.into_bytes();
    }
}

thread_local! {
    static APP: RefCell<App> = RefCell::new(App::new());
    /// The last Rust panic message. WebAssembly only reports a panic as "unreachable", so the hook
    /// saves the real message here for the page to show (see `panic_message`).
    static PANIC: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        PANIC.with(|p| {
            if let Ok(mut p) = p.try_borrow_mut() {
                *p = msg.into_bytes();
            }
        });
    }));
}

/// Pointer/length of the last panic message (empty if none). Safe to call after a crash: it
/// doesn't touch the (possibly still-borrowed) game state.
#[no_mangle]
pub extern "C" fn panic_message_ptr() -> u32 {
    PANIC.with(|p| p.borrow().as_ptr() as u32)
}

#[no_mangle]
pub extern "C" fn panic_message_len() -> u32 {
    PANIC.with(|p| p.borrow().len() as u32)
}

/// Test hook: panics on purpose, so the page's crash reporting can be exercised.
#[no_mangle]
pub extern "C" fn debug_panic() {
    panic!("debug_panic called (test of crash reporting)");
}

/// Called by the page once at startup (and by workers) so panics are reported with their message.
#[no_mangle]
pub extern "C" fn init() {
    install_panic_hook();
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
        let mut sink = LogSink::default();
        state.deal_opening_hands(&mut sink);
        app.start_text = dominion_engine::format_state(&state);
        // Wait at the start of turn 1 ("Start turn"), like every other turn boundary.
        state.pause_at_turn_start = true;
        app.state = state;
        app.history.clear();
        app.record = GameRecord::new(&app.state);
        app.redo.clear();
        app.log.clear();
        for (e, depth) in &sink.events {
            app.push_log_event(e, *depth);
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
        app.record = GameRecord::new(&app.state);
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
    // The page steps turn by turn: stop at each turn boundary (see `resume`).
    app.state.pause_at_turn_start = true;
    let mut sink = LogSink::default();
    match app.state.advance(&mut sink) {
        Step::Decision(_) | Step::GameOver | Step::TurnStart { .. } => {}
        Step::Chance { .. } => unreachable!("chance_mode is never enabled by this crate"),
    }
    for (e, depth) in &sink.events {
        app.push_log_event(e, *depth);
        if matches!(e, Event::GameOver) {
            for line in game_over_summary(&app.state) {
                app.log.push(line);
            }
            app.log_group = None;
        }
    }
    let state = app.state;
    app.record.after_step(&state);
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
    let mut sink = LogSink::default();
    if let Err(e) = app.state.apply(choice, &mut sink) {
        // Shouldn't happen (callers only pass choices from `legal_choices`), but undo the
        // speculative history push rather than leave a no-op undo step behind.
        app.history.pop();
        return Err(e.to_string());
    }
    for (e, depth) in &sink.events {
        app.push_log_event(e, *depth);
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
    if app.state.pending_decision().is_none() {
        resume_impl(app);
        return Ok(());
    }
    let choice = bot_choice(app)?;
    apply_choice(app, choice)
}

/// Start the turn the game is paused at (a turn boundary): play until the next decision.
fn resume_impl(app: &mut App) {
    if app.state.pending_decision().is_none() && app.state.turn.phase != Phase::GameOver {
        app.push_history();
        advance_and_log(app);
    }
}

/// Continue from a turn boundary pause. No-op if a decision is pending or the game is over.
#[no_mangle]
pub extern "C" fn resume() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        resume_impl(&mut app);
        app.set_result(String::new());
        1
    })
}

/// The choice the deciding seat's controller (a strategy or the search) would make.
fn bot_choice(app: &mut App) -> Result<Choice, String> {
    let d = app.state.pending_decision().ok_or("no decision is pending")?;
    let state = app.state;
    Ok(controller_choice(&app.seats, &app.strategies, &mut app.searcher, &app.search_cfg, &state, &d))
}

/// What `state`'s deciding seat's controller chooses for decision `d`.
fn controller_choice(
    seats: &[u32; MAX_PLAYERS],
    strategies: &[Strategy],
    searcher: &mut Searcher,
    search_cfg: &SearchConfig,
    state: &GameState,
    d: &Decision,
) -> Choice {
    let mut buf = ChoiceBuf::default();
    state.legal_choices(&mut buf);
    if buf.len() == 1 {
        return buf.as_slice()[0];
    }
    let seat = seats[d.player as usize];
    let view = PlayerView::new(state, d.player);
    if seat >= FIRST_STRATEGY_SEAT {
        return strategies[(seat - FIRST_STRATEGY_SEAT) as usize].decide(&view, d, buf.as_slice());
    }
    let world = view.determinize(&mut Rng::new(view.stable_seed()));
    searcher.analyze(&world, d.player, search_cfg, &NextHandEvaluator::default()).best().choice
}

// -----------------------------------------------------------------------------------------
// Simulation from the current position (runs in workers; results are summed across workers)
// -----------------------------------------------------------------------------------------

/// Counts buys and coins spent (Buy events) during a turn.
#[derive(Default)]
struct BuyTally {
    bought: u32,
    spent: u32,
}

impl dominion_engine::EventSink for BuyTally {
    fn event(&mut self, e: Event) {
        if let Event::Buy { card, .. } = e {
            self.bought += 1;
            self.spent += cards::cost(card) as u32;
        }
    }
}

/// Raw bytes of the current position (for workers).
#[no_mangle]
pub extern "C" fn state_bytes_current() -> i32 {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let bytes = state_bytes(&app.state);
        app.result = bytes;
        1
    })
}

/// Play `games` games from the position at `state_ptr` with this instance's seat controllers,
/// seeding game g's shuffles from (`seed`, g). Writes JSON of per-player sums, indexed by the
/// player's own turn number t (index t-1): vp (after the turn), money (coins that turn: spent +
/// left over), buys (cards bought), count (games that reached that turn); wins and winsAt (by the
/// winner's turn count; a shared win counts 1/k); plus total game length in turns.
#[no_mangle]
pub extern "C" fn simulate(state_ptr: u32, games: u32, seed: u32) -> i32 {
    let start = state_from_ptr(state_ptr);
    let r = APP.with(|cell| {
        let mut guard = cell.borrow_mut();
        let app = &mut *guard;
        let n = start.num_players as usize;
        let mut vp = vec![Vec::<f64>::new(); n];
        let mut money = vec![Vec::<f64>::new(); n];
        let mut buys = vec![Vec::<f64>::new(); n];
        let mut count = vec![Vec::<f64>::new(); n];
        let mut wins = vec![0f64; n];
        let mut wins_at = vec![Vec::<f64>::new(); n];
        let mut length_sum = 0f64;
        let mut capped = 0u32;
        let bump = |v: &mut Vec<f64>, i: usize, x: f64| {
            if v.len() <= i {
                v.resize(i + 1, 0.0);
            }
            v[i] += x;
        };
        let search_cfg = SearchConfig { node_budget: 20_000, ..app.search_cfg.clone() };
        for g in 0..games {
            let mut s = start;
            s.chance_mode = false;
            s.pause_at_turn_start = false;
            s.rng = Rng::derive(seed as u64, g as u64);
            let mut tally = BuyTally::default();
            let (mut cur_player, mut cur_number) = (s.turn.player as usize, s.turn.number);
            let mut leftover = s.turn.coins as u32;
            for _ in 0..200_000 {
                let step = s.advance(&mut tally);
                let turn_ended = s.turn.number != cur_number || s.turn.phase == Phase::GameOver;
                if turn_ended {
                    let p = cur_player;
                    let t = s.players[p].turns_taken.max(1) as usize - 1;
                    bump(&mut vp[p], t, s.players[p].vp() as f64);
                    bump(&mut money[p], t, (tally.spent + leftover) as f64);
                    bump(&mut buys[p], t, tally.bought as f64);
                    bump(&mut count[p], t, 1.0);
                    tally = BuyTally::default();
                    cur_player = s.turn.player as usize;
                    cur_number = s.turn.number;
                    leftover = 0;
                }
                match step {
                    Step::GameOver => break,
                    Step::TurnStart { .. } | Step::Chance { .. } => {}
                    Step::Decision(d) => {
                        let c = controller_choice(&app.seats, &app.strategies, &mut app.searcher, &search_cfg, &s, &d);
                        if s.apply(c, &mut tally).is_err() {
                            break;
                        }
                        if s.turn.phase == Phase::Buy || s.turn.phase == Phase::CleanupDraw {
                            leftover = s.turn.coins as u32;
                        }
                    }
                }
            }
            if !s.is_game_over() {
                capped += 1;
                continue;
            }
            let w = s.winners();
            let k = w.count_ones() as f64;
            let mut longest = 0;
            for p in 0..n {
                longest = longest.max(s.players[p].turns_taken);
                if w & (1 << p) != 0 {
                    wins[p] += 1.0 / k;
                    bump(&mut wins_at[p], s.players[p].turns_taken.max(1) as usize - 1, 1.0 / k);
                }
            }
            length_sum += longest as f64;
        }
        let arr = |v: &[f64]| format!("[{}]", v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(","));
        let players: Vec<String> = (0..n)
            .map(|p| {
                format!(
                    "{{\"vp\":{},\"money\":{},\"buys\":{},\"count\":{},\"wins\":{},\"winsAt\":{}}}",
                    arr(&vp[p]),
                    arr(&money[p]),
                    arr(&buys[p]),
                    arr(&count[p]),
                    wins[p],
                    arr(&wins_at[p])
                )
            })
            .collect();
        Ok::<String, String>(format!(
            "{{\"games\":{games},\"capped\":{capped},\"lengthSum\":{length_sum},\"players\":[{}]}}",
            players.join(",")
        ))
    });
    result_of(r)
}

/// Let every seat's controller play until the game ends.
fn run_bots_impl(app: &mut App) -> Result<(), String> {
    for _ in 0..1_000_000 {
        if app.state.turn.phase == Phase::GameOver {
            return Ok(());
        }
        if app.state.pending_decision().is_none() {
            // Turn boundary: keep going into the next turn.
            resume_impl(app);
            continue;
        }
        let choice = bot_choice(app)?;
        apply_choice(app, choice)?;
    }
    Err("run_bots: too many steps".to_string())
}

fn analyze_json(app: &mut App) -> Result<String, String> {
    let d = app.state.pending_decision().ok_or("no decision is pending")?;
    let world = { let v = PlayerView::new(&app.state, d.player); v.determinize(&mut Rng::new(v.stable_seed())) };
    let (eval, scoring) = seat_eval(&app.strategies, scoring_strategy(app, d.player));
    let a = app.searcher.analyze(&world, d.player, &app.search_cfg, &eval);
    Ok(analysis_json(app, &d, &a, &scoring, rules_pick(app, &d)))
}

/// What the deciding seat's strategy would choose here (None for human/search seats).
fn rules_pick(app: &App, d: &Decision) -> Option<Choice> {
    let seat = app.seats[d.player as usize];
    let strategy = app.strategies.get(seat.checked_sub(FIRST_STRATEGY_SEAT)? as usize)?;
    let mut buf = ChoiceBuf::default();
    app.state.legal_choices(&mut buf);
    Some(strategy.decide(&PlayerView::new(&app.state, d.player), d, buf.as_slice()))
}

fn analysis_json(app: &App, d: &Decision, a: &dominion_search::Analysis, scoring: &str, pick: Option<Choice>) -> String {
    // A strategy seat's own choice is listed first: it is what Auto-step and Run to end of turn
    // play. Other options follow by value; `better` flags any that would score higher.
    const TIE: f64 = 0.005;
    let pick_ev = pick.and_then(|p| a.options.iter().find(|o| o.choice == p)).map(|o| o.ev);
    // The analysis pick: the highest-value option; the strategy's pick when it ties the best.
    let best_ev = a.options.iter().map(|o| o.ev).fold(f64::NEG_INFINITY, f64::max);
    let analysis_pick = match (pick, pick_ev) {
        (Some(p), Some(pe)) if pe >= best_ev - TIE => Some(p),
        _ => a.options.iter().find(|o| o.ev >= best_ev - TIE).map(|o| o.choice),
    };
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
            let outcomes: Vec<String> =
                o.outcomes.iter().map(|(l, p)| format!("{{\"label\":{},\"p\":{:.5}}}", jstr(l), p)).collect();
            format!(
                "{{\"label\":{},\"index\":{index},\"ev\":{:.4},\"exact\":{},\"rulesPick\":{},\"analysisPick\":{},\"better\":{better},\"pv\":{},\"outcomes\":[{}]}}",
                jstr(&choice_label(d, o.choice)),
                o.ev,
                o.exact,
                Some(o.choice) == pick,
                Some(o.choice) == analysis_pick,
                jstr(&o.pv),
                outcomes.join(",")
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
        let world = { let v = PlayerView::new(&app.state, d.player); v.determinize(&mut Rng::new(v.stable_seed())) };
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
    let outcomes: Vec<String> = r.outcomes.iter().map(|(l, p)| format!("{l}\u{2}{p}")).collect();
    let json = format!(
        "{{\"ev\":{},\"exact\":{},\"nodes\":{},\"ttHits\":{},\"pv\":{},\"outcomes\":{}}}",
        r.ev,
        r.exact,
        r.nodes,
        r.tt_hits,
        jstr(&r.pv.join("\u{1}")),
        jstr(&outcomes.join("\u{1}"))
    );
    result_of(Ok(json))
}

/// Main side: record task `i`'s result. `pv` is the line joined with U+0001; `outcomes` is
/// "label U+0002 probability" entries joined with U+0001.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn plan_put_result(i: u32, ev: f64, exact: u32, nodes: f64, tt_hits: f64, pv_ptr: u32, pv_len: u32, out_ptr: u32, out_len: u32) -> i32 {
    let pv = read_str(pv_ptr, pv_len);
    let outcomes_text = read_str(out_ptr, out_len);
    let outcomes: Vec<(String, f64)> = outcomes_text
        .split('\u{1}')
        .filter(|e| !e.is_empty())
        .filter_map(|e| e.split_once('\u{2}').map(|(l, p)| (l.to_string(), p.parse().unwrap_or(0.0))))
        .collect();
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
            outcomes,
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

/// The current game's per-turn record, in `simulate`'s JSON shape (one game).
#[no_mangle]
pub extern "C" fn game_stats() -> i32 {
    let r = APP.with(|cell| {
        let app = cell.borrow();
        Ok::<String, String>(app.record.json(&app.state))
    });
    result_of(r)
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
        let mut v = vec![jstr("Search (exact turn lookahead)")];
        v.extend(app.strategies.iter().map(|s| jstr(&s.name)));
        format!("[{}]", v.join(","))
    });
    result_of(Ok(names))
}

/// Kingdom implied by the seats' rules: the union of kingdom cards named by the strategies of
/// seats `0..players` (gain lists, play lists, conditions), in first-mentioned order. Writes
/// JSON {strategySeats, kingdom} where kingdom is a comma-separated list (possibly empty).
#[no_mangle]
pub extern "C" fn seat_kingdom(players: u32) -> i32 {
    let r = APP.with(|cell| {
        let app = cell.borrow();
        let n = (players as usize).clamp(2, MAX_PLAYERS);
        let mut cards: Vec<u8> = Vec::new();
        let mut strategy_seats = 0;
        for &seat in &app.seats[..n] {
            if let Some(s) = seat.checked_sub(FIRST_STRATEGY_SEAT).and_then(|i| app.strategies.get(i as usize)) {
                strategy_seats += 1;
                for c in s.kingdom_refs() {
                    if !cards.contains(&c) {
                        cards.push(c);
                    }
                }
            }
        }
        let names: Vec<&str> = cards.iter().map(|&c| cards::name(c)).collect();
        format!("{{\"strategySeats\":{strategy_seats},\"kingdom\":{}}}", jstr(&names.join(", ")))
    });
    result_of(Ok(r))
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
        let max = FIRST_STRATEGY_SEAT + app.strategies.len() as u32;
        if player as usize >= MAX_PLAYERS || bot >= max {
            return Err("bad seat or controller".to_string());
        }
        app.seats[player as usize] = bot;
        Ok(String::new())
    });
    result_of(r)
}

/// Add a strategy from TOML text as a new seat controller, or replace an earlier added one
/// with the same name (shipped strategies are never replaced). Writes JSON {id, name, replaced}.
#[no_mangle]
pub extern "C" fn add_strategy(ptr: u32, len: u32) -> i32 {
    let text = read_str(ptr, len);
    let r = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let strategy = Strategy::parse(&text)?;
        let name = strategy.name.clone();
        let existing = (STRATEGY_SOURCES.len()..app.strategies.len()).find(|&i| app.strategies[i].name == name);
        let (index, replaced) = match existing {
            Some(i) => {
                app.strategies[i] = strategy;
                (i, true)
            }
            None => {
                app.strategies.push(strategy);
                (app.strategies.len() - 1, false)
            }
        };
        Ok(format!("{{\"id\":{},\"name\":{},\"replaced\":{replaced}}}", FIRST_STRATEGY_SEAT + index as u32, jstr(&name)))
    });
    result_of(r)
}

/// Let every seat play until the game ends.
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

/// Play the current turn to its end: stops right after the current player's cleanup, at the
/// next turn's boundary (or at game over). From a boundary pause, plays that next turn.
fn run_to_end_of_turn_impl(app: &mut App) -> Result<(), String> {
    let start_turn = app.state.turn.number;
    for _ in 0..200_000 {
        if app.state.turn.phase == Phase::GameOver {
            return Ok(());
        }
        if app.state.pending_decision().is_none() {
            if app.state.turn.number != start_turn {
                return Ok(()); // paused at the next turn's start
            }
            resume_impl(app);
            continue;
        }
        let choice = bot_choice(app)?;
        apply_choice(app, choice)?;
    }
    Err("run_to_end_of_turn: too many steps, aborting".to_string())
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
    if state.pending_decision().is_none() && state.turn.phase != Phase::GameOver {
        return format!(
            "{{\"player\":{},\"source\":null,\"paused\":true,\"description\":{},\"choices\":[{{\"index\":-1,\"label\":\"Start turn\"}}]}}",
            state.turn.player,
            jstr(&format!("Turn {}: Player {} is up.", state.turn.number, state.turn.player + 1))
        );
    }
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
    // The whole game's log: the page appends only what's new.
    let log_json: Vec<String> = log.iter().map(|l| jstr(l)).collect();

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
