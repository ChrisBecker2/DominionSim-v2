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
    Step, Zone, NO_SOURCE,
};
use dominion_engine::cards::{self, CardId};
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

    fn begin_root(&self, root: &GameState, me: u8) {
        if let SeatEval::Gains(e) = self {
            e.begin_root(root, me);
        }
    }

    fn end_root(&self) {
        if let SeatEval::Gains(e) = self {
            e.end_root();
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

    fn outcome_shows_play(&self, card: CardId) -> bool {
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
    // Intrigue
    include_str!("../../../strategies/torturer_bm.toml"),
    include_str!("../../../strategies/courtyard_bm.toml"),
    include_str!("../../../strategies/masquerade_bm.toml"),
    include_str!("../../../strategies/duke_duchy.toml"),
    // Evolved by the Strategy Lab (appended, so earlier seat ids stay put).
    include_str!("../../../strategies/sentry_merchant.toml"),
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
    /// Per strategy, the same strategy with its turn search off (`search_play = false`): the rules alone, as the Strategy Lab's fast bots
    /// play during evolution. Used by the seats in `rules_only`.
    fast_strategies: Vec<Strategy>,
    /// Seats whose strategy plays by its rules alone (the game page's "search" box unchecked).
    rules_only: [bool; MAX_PLAYERS],
    searcher: Searcher,
    search_cfg: SearchConfig,
    /// (event tag, player, log line) of the run the last event started, for condensing.
    log_group: Option<(u8, u8, usize)>,
    /// Parallel analysis in progress: the split tree, results received so far, and the
    /// strategy index whose priorities score it (`u32::MAX` = general evaluator).
    plan: Option<(Plan, Vec<Option<TaskResult>>, u32)>,
    /// State text at the start of the current game (phase: setup, before the opening deal).
    start_text: String,
    /// Turn-by-turn record of the current game (reset by New game / Load state).
    record: GameRecord,
    /// The card most recently played in the log: a bonus from the same card is printed bare
    /// ("+2 Actions"), one from another card names its source ("Merchant: +$1").
    last_played: Option<CardId>,
    /// The bonus line just logged, so identical bonuses from several copies of one card in a row
    /// merge into one line ("Collection ×3: +3 VP"): (player, source, depth, one copy's
    /// (actions, buys, coins, vp), copies, log index). Only an identical bonus merges, so one
    /// card's two different bonuses (Monument's +$2, then +1 VP) stay one card. Cleared by any
    /// other event.
    last_bonus: Option<(u8, CardId, u8, (u16, u16, u16, u16), u16, usize)>,
    /// Biding Time handing back the set-aside hand: (player, log index, cards named so far). The cards it puts into the hand arrive as
    /// draws; they are named on its line ("Player 1 takes back the cards Biding Time set aside: Curse, Curse"), not as a draw from the deck.
    take_back: Option<(u8, usize, u16)>,
    /// The card just bought (player, card): its own gain to the discard pile is not logged again under the buy (a gain anywhere
    /// else is: "gains Nomad Camp to their deck (on top)").
    last_buy: Option<(u8, CardId)>,
}

impl App {
    fn new() -> Self {
        let cfg = GameConfig::default();
        let mut state = GameState::new(&cfg);
        let start_text = dominion_engine::format_state(&state);
        // Wait before the deal ("Deal"): Simulate from here deals fresh opening hands per game.
        state.pause_at_turn_start = true;
        let strategies: Vec<Strategy> = STRATEGY_SOURCES.iter().map(|src| Strategy::parse(src).expect("bundled strategy parses")).collect();
        let fast_strategies = strategies.iter().map(rules_only_copy).collect();
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
            fast_strategies,
            rules_only: [false; MAX_PLAYERS],
            searcher: Searcher::new(search_cfg.tt_bits),
            search_cfg,
            log_group: None,
            plan: None,
            start_text,
            record: GameRecord::default(),
            last_played: None,
            last_bonus: None,
            take_back: None,
            last_buy: None,
        };
        app.record = GameRecord::new(&app.state);
        app.log.push("New game: shuffled, opening hands not dealt yet.".to_string());
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
        let last_bonus = self.last_bonus.take();
        if let Some((pl, at, n)) = self.take_back.take() {
            if let Event::Draw { player, card } = *e {
                if player == pl && at + 1 == self.log.len() {
                    let line = &mut self.log[at];
                    line.push_str(if n == 0 { ": " } else { ", " });
                    line.push_str(cards::name(card));
                    self.take_back = Some((pl, at, n + 1));
                    return;
                }
            }
        }
        match *e {
            Event::TurnStart { .. } => self.last_played = None,
            Event::Play { card, .. } | Event::PlayAgain { card, .. } | Event::StartPlay { card, .. } => self.last_played = Some(card),
            // A buy (and what it triggers) isn't the last played card's doing.
            Event::Buy { player, card } => {
                self.last_played = None;
                self.last_buy = Some((player, card));
            }
            Event::Gain { player, card, to: Dest::Discard, source: NO_SOURCE } if self.last_buy == Some((player, card)) => {
                self.last_buy = None;
                return;
            }
            Event::Bonus { player, source, actions, buys, coins, vp } => {
                let unit = (actions as u16, buys as u16, coins, vp);
                // The same bonus again from another copy of the same card: merge into its line.
                let (copies, at) = match last_bonus {
                    Some((pl, src, d, u, n, at)) if pl == player && src == source && d == depth && u == unit && at + 1 == self.log.len() => {
                        (n + 1, Some(at))
                    }
                    _ => (1, None),
                };
                let (a, b, c, v) = (unit.0 * copies, unit.1 * copies, unit.2 * copies, unit.3 * copies);
                let text = bonus_text(a.min(255) as u8, b.min(255) as u8, c, v, cards::is(source, cards::TREASURE));
                // A Duration firing at the start of a turn: the engine merges the copies of a card into one entry ("+$2" for two
                // Samurai), so name the copies in play ("Samurai ×2"; one Throne Room'd Samurai stays "Samurai").
                let shown = if depth == 0 && (source as usize) < cards::NUM_ZONE_IDS { copies.max(self.state.players[player as usize].in_play.get(source) as u16) } else { copies };
                let name = if shown > 1 { format!("{} \u{d7}{shown}", cards::name(source)) } else { cards::name(source).to_string() };
                let take_back = depth == 0 && source == id::BIDING_TIME;
                let line = if take_back {
                    format!("Player {} takes back the cards Biding Time set aside", player + 1)
                } else if depth == 0 {
                    // A Duration firing at the start of its owner's turn (or a Prophecy such as Biding Time, which is no card in play).
                    let sep = if text.is_empty() { "" } else { ": " };
                    let kind = if (source as usize) < cards::NUM_ZONE_IDS { " (duration)" } else { "" };
                    format!("Player {}'s {name}{kind}{sep}{text}", player + 1)
                } else if copies == 1 && self.last_played == Some(source) {
                    text
                } else {
                    format!("{name}: {text}")
                };
                let line = format!("{indent}{line}");
                let at = match at {
                    Some(i) => {
                        self.log[i] = line;
                        i
                    }
                    None => {
                        self.log.push(line);
                        self.log.len() - 1
                    }
                };
                self.last_bonus = Some((player, source, depth, unit, copies, at));
                if take_back {
                    self.take_back = Some((player, at, 0));
                }
                self.log_group = None;
                return;
            }
            _ => {}
        }
        if !matches!(e, Event::Buy { .. } | Event::Overpay { .. }) {
            self.last_buy = None;
        }
        // A gain indented under the card that caused it does not name that card again.
        let bare;
        let e = match *e {
            Event::Gain { player, card, to, source } if depth > 0 && self.last_played == Some(source) => {
                bare = Event::Gain { player, card, to, source: NO_SOURCE };
                &bare
            }
            _ => e,
        };
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
            GameConfig { num_players: n, kingdom, seed, max_turns: if max_turns == 0 { 200 } else { max_turns as u16 }, ..Default::default() };
        let mut state = GameState::new(&cfg);
        // The game waits before the deal ("Deal"): the opening hands aren't drawn yet, so
        // Simulate from here deals fresh ones in every game (not one fixed 5/2 or 4/3 split).
        app.start_text = dominion_engine::format_state(&state);
        state.pause_at_turn_start = true;
        app.state = state;
        app.history.clear();
        app.record = GameRecord::new(&app.state);
        app.redo.clear();
        app.log.clear();
        app.log.push("New game: shuffled, opening hands not dealt yet.".to_string());
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
                // Parsing always clears `pending` (see text.rs docs). Show the position exactly
                // as loaded: if reaching the first decision would do anything automatic first
                // (play Treasures, draw, enter the Buy phase, resolve Durations), stay paused
                // ("Continue", like a turn boundary) and let the first user action advance.
                // Already at a real decision (nothing automatic pending): show it.
                app.state.pause_at_turn_start = true;
                let mut probe = app.state;
                let mut sink = LogSink::default();
                probe.advance(&mut sink);
                if sink.events.is_empty() {
                    app.state = probe;
                    let state = app.state;
                    app.record.after_step(&state);
                }
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
        Phase::Night => "Night phase",
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
        Some(EndReason::ColoniesGone) => "the Colony pile is empty".to_string(),
        Some(EndReason::PilesEmpty) => {
            let mut piles: Vec<&str> = (0..cards::FIRST_PILE_CARD)
                .filter(|&c| state.in_supply(c) && state.supply.get(c) == 0)
                .map(cards::name)
                .collect();
            for (pile, name) in [(&dominion_engine::sets::dark_ages::RUINS_PILE, "Ruins"), (&dominion_engine::sets::dark_ages::KNIGHTS_PILE, "Knights")] {
                if state.has_pile(pile) && state.pile_size(pile) == 0 {
                    piles.push(name);
                }
            }
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
    let pool = if app.rules_only[d.player as usize] { &app.fast_strategies } else { &app.strategies };
    Ok(controller_choice(&app.seats, pool, &mut app.searcher, &app.search_cfg, &state, &d))
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

/// Counts buys, coins spent and Province/Duchy/Colony buys (Buy events) during a turn.
#[derive(Default)]
struct BuyTally {
    bought: u32,
    spent: u32,
    /// Provinces, Duchies, Colonies bought.
    green: [u32; 3],
}

impl dominion_engine::EventSink for BuyTally {
    fn event(&mut self, e: Event) {
        if let Event::Buy { card, .. } = e {
            self.bought += 1;
            self.spent += cards::cost(card) as u32;
            match card {
                id::PROVINCE => self.green[0] += 1,
                id::DUCHY => self.green[1] += 1,
                id::COLONY => self.green[2] += 1,
                _ => {}
            }
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

/// `simulate` flag: also record, per turn, the cards each seat owns (JSON `owned`).
const SIM_DECK: u32 = 1;
/// Play `games` games from the position at `state_ptr` with this instance's seat controllers,
/// seeding game g's shuffles from (`seed`, g) for g in `first_game..first_game + games` (so a run can be split into ranges that add up to the whole). Writes JSON of sums (the page adds them across
/// workers and divides by counts), indexed by the player's own turn number t (index t-1):
/// - vp (after the turn) and vpSq (its squares), money (coins that turn: spent + left over) and
///   moneySq, buys (cards bought) and buysSq, count (games that reached that turn);
/// - provCum, duchyCum, colonyCum: Provinces / Duchies / Colonies bought up to and including the
///   turn, summed over the games that reached it; provEnd, duchyEnd, colonyEnd: the same totals
///   at the seat's last turn, indexed by that turn (to carry finished games forward: the average
///   over all games by turn t is (cum[t] + sum of end[<t]) / games);
/// - curse, curseEnd: Curses the seat owns after the turn (however they came), summed like provCum /
///   provEnd;
/// - wins and winsSq (sum of squared win shares) and winsAt (by the winner's turn count; a
///   shared win counts 1/k); firstGames / firstWins: games the seat went first in, and its wins
///   in them (so the second-player rate is the rest);
/// - owned (only with `SIM_DECK`): card name -> per turn, the sum over games of how many copies
///   the seat owned after the turn;
/// plus, per game, total length in turns (lengthSum, lengthHist by length), and `pairs`: for each
/// seat q > 0, the VP difference (seat 0 minus q) after each turn both had played (n games, sum,
/// sq) and the sum of squared win-share margins (marginSq). `fresh` says whether the start
/// position was a new game (so first-player numbers mean something).
#[no_mangle]
pub extern "C" fn simulate(state_ptr: u32, games: u32, seed: u32, flags: u32, first_game: u32) -> i32 {
    let start = state_from_ptr(state_ptr);
    let r = APP.with(|cell| {
        let mut guard = cell.borrow_mut();
        let app = &mut *guard;
        let n = start.num_players as usize;
        let fresh = start.turn.number == 1 && start.turn.player == 0 && (0..n).all(|p| start.players[p].turns_taken == 0);
        let want_deck = flags & SIM_DECK != 0;
        let mut vp = vec![Vec::<f64>::new(); n];
        let mut vp_sq = vec![Vec::<f64>::new(); n];
        let mut money = vec![Vec::<f64>::new(); n];
        let mut money_sq = vec![Vec::<f64>::new(); n];
        let mut buys = vec![Vec::<f64>::new(); n];
        let mut buys_sq = vec![Vec::<f64>::new(); n];
        let mut count = vec![Vec::<f64>::new(); n];
        let mut green = vec![vec![Vec::<f64>::new(); n]; 3];
        let mut green_end = vec![vec![Vec::<f64>::new(); n]; 3];
        let mut curse = vec![Vec::<f64>::new(); n];
        let mut curse_end = vec![Vec::<f64>::new(); n];
        let mut owned = vec![Vec::<f64>::new(); n]; // [turn * NUM_CARDS + card]
        let mut wins = vec![0f64; n];
        let mut wins_sq = vec![0f64; n];
        let mut wins_at = vec![Vec::<f64>::new(); n];
        let mut first_games = vec![0f64; n];
        let mut first_wins = vec![0f64; n];
        let mut length_hist = Vec::<f64>::new();
        let mut length_sum = 0f64;
        let mut capped = 0u32;
        // Seat 0 minus seat q (index q): VP difference per turn, and win-share margin squares.
        let mut pair_n = vec![Vec::<f64>::new(); n];
        let mut pair_sum = vec![Vec::<f64>::new(); n];
        let mut pair_sq = vec![Vec::<f64>::new(); n];
        let mut margin_sq = vec![0f64; n];
        let mut vp_hist = vec![Vec::<i32>::new(); n];
        let bump = |v: &mut Vec<f64>, i: usize, x: f64| {
            if v.len() <= i {
                v.resize(i + 1, 0.0);
            }
            v[i] += x;
        };
        let search_cfg = SearchConfig { node_budget: 20_000, ..app.search_cfg.clone() };
        // From a new game (before the deal) every game also seats the players in a random order,
        // so who goes first is random too. Physical seat i is played by seat `order[i]`'s
        // controller, and its results are reported under that seat.
        let new_game = start.turn.phase == Phase::Setup;
        for g in first_game..first_game + games {
            let mut s = start;
            s.chance_mode = false;
            s.pause_at_turn_start = false;
            s.rng = Rng::derive(seed as u64, g as u64);
            // A new game's Knights and Ruins piles are dealt again for each simulated game (the opening hands are drawn fresh below).
            if start.turn.phase == Phase::Setup {
                s.redeal_mixed_piles();
            }
            let mut order: [usize; MAX_PLAYERS] = std::array::from_fn(|i| i);
            if new_game {
                let mut r = Rng::derive(seed as u64 ^ 0x5EA7_0D3E, g as u64);
                for i in (1..n).rev() {
                    order.swap(i, r.below(i as u32 + 1) as usize);
                }
            }
            let seats: [u32; MAX_PLAYERS] = std::array::from_fn(|i| app.seats[order[i]]);
            let rules_only: [bool; MAX_PLAYERS] = std::array::from_fn(|i| app.rules_only[order[i]]);
            let first = order[s.turn.player as usize];
            first_games[first] += 1.0;
            for h in vp_hist.iter_mut() {
                h.clear();
            }
            let mut run_green = [[0u32; 3]; MAX_PLAYERS];
            let mut tally = BuyTally::default();
            let (mut cur_player, mut cur_number) = (s.turn.player as usize, s.turn.number);
            let mut leftover = s.turn.coins as u32;
            for _ in 0..200_000 {
                let step = s.advance(&mut tally);
                let turn_ended = s.turn.number != cur_number || s.turn.phase == Phase::GameOver;
                if turn_ended {
                    let p = cur_player;
                    let t = s.players[p].turns_taken.max(1) as usize - 1;
                    let seat = order[p];
                    let score = s.players[p].vp();
                    let m = (tally.spent + leftover) as f64;
                    bump(&mut vp[seat], t, score as f64);
                    bump(&mut vp_sq[seat], t, (score as f64) * (score as f64));
                    bump(&mut money[seat], t, m);
                    bump(&mut money_sq[seat], t, m * m);
                    bump(&mut buys[seat], t, tally.bought as f64);
                    bump(&mut buys_sq[seat], t, (tally.bought as f64) * (tally.bought as f64));
                    bump(&mut count[seat], t, 1.0);
                    for k in 0..3 {
                        run_green[seat][k] += tally.green[k];
                        bump(&mut green[k][seat], t, run_green[seat][k] as f64);
                    }
                    if vp_hist[seat].len() <= t {
                        vp_hist[seat].resize(t + 1, i32::MIN);
                    }
                    vp_hist[seat][t] = score;
                    bump(&mut curse[seat], t, s.players[p].count_of(cards::id::CURSE) as f64);
                    if want_deck {
                        if owned[seat].len() < (t + 1) * cards::NUM_CARDS {
                            owned[seat].resize((t + 1) * cards::NUM_CARDS, 0.0);
                        }
                        for (c, k) in s.players[p].all_cards().iter() {
                            owned[seat][t * cards::NUM_CARDS + c as usize] += k as f64;
                        }
                    }
                    tally = BuyTally::default();
                    cur_player = s.turn.player as usize;
                    cur_number = s.turn.number;
                    leftover = 0;
                }
                match step {
                    Step::GameOver => break,
                    Step::TurnStart { .. } | Step::Chance { .. } => {}
                    Step::Decision(d) => {
                        let pool = if rules_only[d.player as usize] { &app.fast_strategies } else { &app.strategies };
                        let c = controller_choice(&seats, pool, &mut app.searcher, &search_cfg, &s, &d);
                        if s.apply(c, &mut tally).is_err() {
                            break;
                        }
                        if s.turn.phase == Phase::Buy || s.turn.phase == Phase::CleanupDraw {
                            leftover = s.turn.coins as u32;
                        }
                    }
                }
            }
            for q in 1..n {
                for t in 0..vp_hist[0].len().min(vp_hist[q].len()) {
                    let (a, b) = (vp_hist[0][t], vp_hist[q][t]);
                    if a != i32::MIN && b != i32::MIN {
                        let d = (a - b) as f64;
                        bump(&mut pair_n[q], t, 1.0);
                        bump(&mut pair_sum[q], t, d);
                        bump(&mut pair_sq[q], t, d * d);
                    }
                }
            }
            // Each seat's running totals when its last turn ended (to carry forward past the end).
            for p in 0..n {
                if let Some(last) = vp_hist[p].len().checked_sub(1) {
                    for k in 0..3 {
                        bump(&mut green_end[k][p], last, run_green[p][k] as f64);
                    }
                    let q = order.iter().position(|&o| o == p).unwrap_or(p);
                    bump(&mut curse_end[p], last, s.players[q].count_of(cards::id::CURSE) as f64);
                }
            }
            if !s.is_game_over() {
                capped += 1;
                continue;
            }
            let w = s.winners();
            let k = w.count_ones() as f64;
            let mut longest = 0;
            let share = |p: usize| if w & (1 << p) != 0 { 1.0 / k } else { 0.0 };
            for p in 0..n {
                longest = longest.max(s.players[p].turns_taken);
                let o = share(p);
                if o > 0.0 {
                    let seat = order[p];
                    wins[seat] += o;
                    wins_sq[seat] += o * o;
                    bump(&mut wins_at[seat], s.players[p].turns_taken.max(1) as usize - 1, o);
                    if seat == first {
                        first_wins[seat] += o;
                    }
                }
            }
            let mut by_seat = [0f64; MAX_PLAYERS];
            for p in 0..n {
                by_seat[order[p]] = share(p);
            }
            for q in 1..n {
                let d = by_seat[0] - by_seat[q];
                margin_sq[q] += d * d;
            }
            bump(&mut length_hist, longest as usize, 1.0);
            length_sum += longest as f64;
        }
        let arr = |v: &[f64]| format!("[{}]", v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(","));
        let players: Vec<String> = (0..n)
            .map(|p| {
                let mut o = format!(
                    "{{\"vp\":{},\"vpSq\":{},\"money\":{},\"moneySq\":{},\"buys\":{},\"buysSq\":{},\"count\":{},\
                     \"provCum\":{},\"duchyCum\":{},\"colonyCum\":{},\"provEnd\":{},\"duchyEnd\":{},\"colonyEnd\":{},\"curse\":{},\"curseEnd\":{},\"wins\":{},\"winsSq\":{},\"winsAt\":{},\
                     \"firstGames\":{},\"firstWins\":{}",
                    arr(&vp[p]),
                    arr(&vp_sq[p]),
                    arr(&money[p]),
                    arr(&money_sq[p]),
                    arr(&buys[p]),
                    arr(&buys_sq[p]),
                    arr(&count[p]),
                    arr(&green[0][p]),
                    arr(&green[1][p]),
                    arr(&green[2][p]),
                    arr(&green_end[0][p]),
                    arr(&green_end[1][p]),
                    arr(&green_end[2][p]),
                    arr(&curse[p]),
                    arr(&curse_end[p]),
                    wins[p],
                    wins_sq[p],
                    arr(&wins_at[p]),
                    first_games[p],
                    first_wins[p]
                );
                if want_deck {
                    let turns = owned[p].len() / cards::NUM_CARDS;
                    let by_card: Vec<String> = (0..cards::NUM_CARDS)
                        .filter(|&c| (0..turns).any(|t| owned[p][t * cards::NUM_CARDS + c] > 0.0))
                        .map(|c| {
                            let per_turn: Vec<f64> = (0..turns).map(|t| owned[p][t * cards::NUM_CARDS + c]).collect();
                            format!("{}:{}", jstr(cards::name(c as CardId)), arr(&per_turn))
                        })
                        .collect();
                    o.push_str(&format!(",\"owned\":{{{}}}", by_card.join(",")));
                }
                o.push('}');
                o
            })
            .collect();
        let pairs: Vec<String> = (1..n)
            .map(|q| {
                format!(
                    "{{\"a\":0,\"b\":{q},\"n\":{},\"sum\":{},\"sq\":{},\"marginSq\":{}}}",
                    arr(&pair_n[q]),
                    arr(&pair_sum[q]),
                    arr(&pair_sq[q]),
                    margin_sq[q]
                )
            })
            .collect();
        Ok::<String, String>(format!(
            "{{\"games\":{games},\"capped\":{capped},\"lengthSum\":{length_sum},\"lengthHist\":{},\"fresh\":{fresh},\
             \"pairs\":[{}],\"players\":[{}]}}",
            arr(&length_hist),
            pairs.join(","),
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
                jstr(&choice_label(&app.state, d, o.choice)),
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

/// State text at the start of the current game: phase setup, before the opening deal (the
/// text's seed reproduces the same deal). Loading it replays the game from the start.
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
        (cards::DURATION, "duration"),
        (cards::REWARD, "reward"),
        (cards::COMMAND, "command"),
        (cards::RUINS, "ruins"),
        (cards::KNIGHT, "knight"),
        (cards::LOOTER, "looter"),
        (cards::SHELTER, "shelter"),
        (cards::NIGHT, "night"),
        (cards::FATE, "fate"),
        (cards::DOOM, "doom"),
        (cards::SPIRIT, "spirit"),
        (cards::ZOMBIE, "zombie"),
        (cards::HEIRLOOM, "heirloom"),
        (cards::OMEN, "omen"),
        (cards::SHADOW, "shadow"),
    ];
    let items: Vec<String> = (0..cards::NUM_CARDS as CardId).chain(cards::FIRST_PROPHECY..cards::FIRST_PROPHECY + cards::NUM_PROPHECIES as CardId)
        .map(|c| {
            let mut types: Vec<String> = flags.iter().filter(|(f, _)| cards::is(c, *f)).map(|(_, n)| jstr(n)).collect();
            for (yes, name) in [(cards::is_boon(c), "boon"), (cards::is_hex(c), "hex"), (cards::is_state(c), "state"), (cards::is_prophecy(c), "prophecy")] {
                if yes {
                    types.push(jstr(name));
                }
            }
            format!(
                "{{\"name\":{},\"cost\":{},\"costLabel\":{},\"types\":[{}]}}",
                jstr(cards::name(c)),
                cards::cost(c),
                jstr(&cards::cost_string(c)),
                types.join(",")
            )
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

/// The kingdom cards seats `0..players`' strategies need (gain lists, play lists, conditions),
/// in first-mentioned order, plus how many of those seats are strategies (vs. search/human).
fn required_seat_cards(app: &App, n: usize) -> (u32, Vec<CardId>) {
    let mut cards: Vec<CardId> = Vec::new();
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
    (strategy_seats, cards)
}

/// Kingdom implied by the seats' rules: the union of kingdom cards named by the strategies of
/// seats `0..players` (gain lists, play lists, conditions), in first-mentioned order. Writes
/// JSON {strategySeats, kingdom} where kingdom is a comma-separated list (possibly empty).
#[no_mangle]
pub extern "C" fn seat_kingdom(players: u32) -> i32 {
    let r = APP.with(|cell| {
        let app = cell.borrow();
        let n = (players as usize).clamp(2, MAX_PLAYERS);
        let (strategy_seats, cards) = required_seat_cards(&app, n);
        let names: Vec<&str> = cards.iter().map(|&c| cards::name(c)).collect();
        format!("{{\"strategySeats\":{strategy_seats},\"kingdom\":{}}}", jstr(&names.join(", ")))
    });
    result_of(Ok(r))
}

/// A stream tag distinguishing New Game's kingdom-padding randomness from the game's own RNG
/// (seeded from the same `seed` input, but a different derived stream).
const KINGDOM_PAD_STREAM: u64 = 0x4B69_6E67_444F_4D;

fn card_sets_from_mask(sets_mask: u32) -> Vec<cards::CardSet> {
    let mut v = Vec::new();
    if sets_mask & 1 != 0 {
        v.push(cards::CardSet::Base);
    }
    if sets_mask & 2 != 0 {
        v.push(cards::CardSet::Intrigue);
    }
    if sets_mask & 4 != 0 {
        v.push(cards::CardSet::Seaside);
    }
    if sets_mask & 8 != 0 {
        v.push(cards::CardSet::Prosperity);
    }
    if sets_mask & 16 != 0 {
        v.push(cards::CardSet::Alchemy);
    }
    if sets_mask & 32 != 0 {
        v.push(cards::CardSet::Promo);
    }
    if sets_mask & 64 != 0 {
        v.push(cards::CardSet::CornucopiaGuilds);
    }
    if sets_mask & 128 != 0 {
        v.push(cards::CardSet::DarkAges);
    }
    if sets_mask & 256 != 0 {
        v.push(cards::CardSet::Adventures);
    }
    if sets_mask & 512 != 0 {
        v.push(cards::CardSet::Empires);
    }
    if sets_mask & 1024 != 0 {
        v.push(cards::CardSet::RisingSun);
    }
    if sets_mask & 2048 != 0 {
        v.push(cards::CardSet::Nocturne);
    }
    if sets_mask & 4096 != 0 {
        v.push(cards::CardSet::Hinterlands);
    }
    v
}

/// The kingdom for New Game: the seats' required cards ([`seat_kingdom`]), padded to 10 with
/// cards drawn at random from `sets_mask` (bit 0 = Base, bit 1 = Intrigue, bit 2 = Seaside,
/// bit 3 = Prosperity, bit 4 = Alchemy, bit 5 = Promo, bit 6 = Cornucopia & Guilds, bit 7 = Dark Ages, bit 8 = Adventures, bit 9 = Empires, bit 10 = Rising Sun, bit 11 = Nocturne, bit 12 = Hinterlands: the Events and Landmarks of those sets join the kingdom list after the cards), seeded from `seed` so the same seed and set selection reproduce the same
/// kingdom. A required card outside the selected sets is kept anyway. Any Prosperity card ending
/// up in the kingdom (drawn or required) brings Platinum and Colony into it too (the official
/// rule; see `cards::random_kingdom_with_colonies`). Writes JSON {strategySeats, kingdom}
/// (kingdom: 10 comma-separated names, or more if the seats alone need more than 10, or if
/// Colonies were added).
#[no_mangle]
pub extern "C" fn padded_kingdom(players: u32, sets_mask: u32, seed: u64) -> i32 {
    let r = APP.with(|cell| {
        let app = cell.borrow();
        let n = (players as usize).clamp(2, MAX_PLAYERS);
        let (strategy_seats, required) = required_seat_cards(&app, n);
        let sets = card_sets_from_mask(sets_mask);
        let mut rng = Rng::derive(seed, KINGDOM_PAD_STREAM);
        let kingdom = cards::random_kingdom_with_colonies(&sets, &required, &mut rng);
        let names: Vec<&str> = kingdom.iter().map(|&c| cards::name(c)).collect();
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

/// Whether `player`'s strategy searches its turns (`on`, the default) or plays by its rules alone. Ignored by the Search seat.
#[no_mangle]
pub extern "C" fn set_seat_search(player: u32, on: u32) -> i32 {
    let r = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        if player as usize >= MAX_PLAYERS {
            return Err("bad seat".to_string());
        }
        app.rules_only[player as usize] = on == 0;
        Ok(String::new())
    });
    result_of(r)
}

/// JSON array: per seat of the current game, whether its strategy searches its turns.
#[no_mangle]
pub extern "C" fn get_seat_search() -> i32 {
    let s = APP.with(|cell| {
        let app = cell.borrow();
        let n = app.state.num_players as usize;
        format!("[{}]", app.rules_only[..n].iter().map(|&r| if r { "false" } else { "true" }).collect::<Vec<_>>().join(","))
    });
    result_of(Ok(s))
}

/// `s` with its turn search off: the rules alone.
fn rules_only_copy(s: &Strategy) -> Strategy {
    let mut f = s.clone();
    f.search_play = false;
    f
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
                app.fast_strategies[i] = rules_only_copy(&strategy);
                app.strategies[i] = strategy;
                (i, true)
            }
            None => {
                app.fast_strategies.push(rules_only_copy(&strategy));
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

/// The pending decision's search as a graph (see `dominion_search::SearchGraph`), scored like
/// `analyze`, with at most `max_nodes` positions. Writes JSON:
/// {searchedNodes, ttHits, truncated, nodes: [{kind, depth, value, exact, inEdges, expanded,
/// best, hand, inPlay, gained, coins, actions, buys}], edges: [{from, to, label, prob, best}]}.
#[no_mangle]
pub extern "C" fn search_graph(max_nodes: u32) -> i32 {
    let r = APP.with(|cell| {
        let mut guard = cell.borrow_mut();
        let app = &mut *guard;
        let d = app.state.pending_decision().ok_or("no decision is pending")?;
        let world = {
            let v = PlayerView::new(&app.state, d.player);
            v.determinize(&mut Rng::new(v.stable_seed()))
        };
        let (eval, _) = seat_eval(&app.strategies, scoring_strategy(app, d.player));
        let cfg = app.search_cfg.clone();
        let g = app.searcher.search_graph(&world, d.player, &cfg, &eval, max_nodes.clamp(2, 20_000) as usize);
        let pairs = |v: &[(CardId, u8)]| {
            let items: Vec<String> = v.iter().map(|&(c, n)| format!("[{},{n}]", jstr(cards::name(c)))).collect();
            format!("[{}]", items.join(","))
        };
        let nodes: Vec<String> = g
            .nodes
            .iter()
            .map(|n| {
                let kind = match n.kind {
                    dominion_search::NodeKind::Root => "root",
                    dominion_search::NodeKind::Decision => "decision",
                    dominion_search::NodeKind::Chance => "chance",
                    dominion_search::NodeKind::Leaf => "leaf",
                };
                format!(
                    "{{\"kind\":\"{kind}\",\"depth\":{},\"value\":{:.4},\"exact\":{},\"inEdges\":{},\"expanded\":{},\"best\":{},\"hand\":{},\"inPlay\":{},\"gained\":{},\"coins\":{},\"actions\":{},\"buys\":{}}}",
                    n.depth,
                    n.value,
                    n.exact,
                    n.in_edges,
                    n.expanded,
                    n.best,
                    pairs(&n.state.hand),
                    pairs(&n.state.in_play),
                    pairs(&n.state.gained),
                    n.state.coins,
                    n.state.actions,
                    n.state.buys
                )
            })
            .collect();
        let edges: Vec<String> = g
            .edges
            .iter()
            .map(|e| {
                let prob = e.prob.map_or("null".to_string(), |p| format!("{p:.6}"));
                format!("{{\"from\":{},\"to\":{},\"label\":{},\"prob\":{prob},\"best\":{}}}", e.from, e.to, jstr(&e.label), e.best)
            })
            .collect();
        Ok::<String, String>(format!(
            "{{\"searchedNodes\":{},\"ttHits\":{},\"truncated\":{},\"nodes\":[{}],\"edges\":[{}]}}",
            g.searched_nodes,
            g.tt_hits,
            g.truncated,
            nodes.join(","),
            edges.join(",")
        ))
    });
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
fn groupable(e: &Event) -> Option<(u8, u8, CardId)> {
    match *e {
        Event::Draw { player, card } => Some((0, player, card)),
        Event::Play { player, card } if cards::is(card, cards::TREASURE) => Some((1, player, card)),
        Event::Discard { player, card } => Some((2, player, card)),
        Event::Reveal { player, card } => Some((3, player, card)),
        Event::Trash { player, card } => Some((4, player, card)),
        Event::SetAside { player, card } => Some((5, player, card)),
        _ => None,
    }
}

/// "+2 Actions, +1 Buy, +$3, +1 VP" (only the non-zero parts; Cards show as draws). Treasures
/// list their coins first, as printed ("+$1, +1 Buy" for Astrolabe).
fn bonus_text(actions: u8, buys: u8, coins: u16, vp: u16, coins_first: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if coins_first && coins > 0 {
        parts.push(format!("+${coins}"));
    }
    if actions > 0 {
        parts.push(format!("+{actions} Action{}", if actions == 1 { "" } else { "s" }));
    }
    if buys > 0 {
        parts.push(format!("+{buys} Buy{}", if buys == 1 { "" } else { "s" }));
    }
    if !coins_first && coins > 0 {
        parts.push(format!("+${coins}"));
    }
    if vp > 0 {
        parts.push(format!("+{vp} VP"));
    }
    parts.join(", ")
}

fn render_event(e: &Event) -> String {
    match *e {
        Event::Bonus { .. } => String::new(), // rendered by `push_log_event`
        Event::TurnStart { player, turn } => format!("--- Turn {turn}: Player {} ---", player + 1),
        Event::Shuffle { player } => format!("Player {} shuffles their discard into their deck", player + 1),
        Event::Draw { player, card } => format!("Player {} draws {}", player + 1, cards::name(card)),
        Event::Play { player, card } => format!("Player {} plays {}", player + 1, cards::name(card)),
        Event::Buy { player, card } => format!("Player {} buys {}", player + 1, cards::name(card)),
        Event::Gain { player, card, to, source } => {
            let dest = match to {
                Dest::Discard => "discard pile",
                Dest::Hand => "hand",
                Dest::DeckTop => "deck (on top)",
            };
            let from = if source == NO_SOURCE { String::new() } else { format!(" ({})", cards::name(source)) };
            format!("Player {} gains {} to their {}{from}", player + 1, cards::name(card), dest)
        }
        Event::Trash { player, card } => format!("Player {} trashes {}", player + 1, cards::name(card)),
        Event::Discard { player, card } => format!("Player {} discards {}", player + 1, cards::name(card)),
        Event::Topdeck { player, card } => format!("Player {} puts {} on top of their deck", player + 1, cards::name(card)),
        Event::Reveal { player, card } => format!("Player {} reveals {}", player + 1, cards::name(card)),
        Event::SetAside { player, card } => format!("Player {} sets aside {}", player + 1, cards::name(card)),
        Event::StartPlay { player, source, card } => format!("Player {}'s {} plays {}", player + 1, cards::name(source), cards::name(card)),
        Event::Reaction { player, card } => format!("Player {} reveals {} (reaction)", player + 1, cards::name(card)),
        Event::Coffers { player, source, n } => format!("Player {} gets +{n} Coffers ({})", player + 1, cards::name(source)),
        Event::SpendCoffers { player, n } => {
            format!("Player {} spends {} Coffers (+${n})", player + 1, if n == 1 { "a token of".to_string() } else { n.to_string() })
        }
        Event::Overpay { player, card, coins, potion } => {
            format!("Player {} overpays ${coins}{} for {}", player + 1, if potion { " and a Potion" } else { "" }, cards::name(card))
        }
        Event::Pass { player, card, to } => format!("Player {} passes {} to Player {}", player + 1, cards::name(card), to + 1),
        Event::Return { player, card } => format!("Player {} returns {} to its pile", player + 1, cards::name(card)),
        Event::TakeFromTrash { player, card } => format!("Player {} puts {} from the trash into their hand", player + 1, cards::name(card)),
        Event::DeckToDiscard { player, n } => {
            format!("Player {} puts their deck ({n} card{}) into their discard pile", player + 1, if n == 1 { "" } else { "s" })
        }
        Event::ToTavern { player, card } => format!("Player {} puts {} on their Tavern mat", player + 1, cards::name(card)),
        Event::Call { player, card } => format!("Player {} calls {} from their Tavern mat", player + 1, cards::name(card)),
        Event::Exchange { player, from, to } => format!("Player {} exchanges {} for {}", player + 1, cards::name(from), cards::name(to)),
        Event::Token { player, token, pile } => {
            format!("Player {} moves their {} token to {}", player + 1, cards::TOKEN_NAMES[token as usize], cards::name(pile))
        }
        Event::Flip { player, up } => format!("Player {} turns their Journey token {}", player + 1, if up { "face up" } else { "face down" }),
        Event::MinusToken { player, token } => format!("Player {} takes their {} token", player + 1, cards::TOKEN_NAMES[token as usize]),
        Event::GameOver => "--- Game over ---".to_string(),
        Event::Boon { player, boon, source } => {
            let from = if cards::is_boon(source) || source == NO_SOURCE { String::new() } else { format!(" ({})", cards::name(source)) };
            format!("Player {} receives {}{from}", player + 1, cards::name(boon))
        }
        Event::Hex { player, hex, source } => format!("Player {} receives {} ({})", player + 1, cards::name(hex), cards::name(source)),
        Event::State { player, state, taken } => {
            if taken {
                format!("Player {} takes {}", player + 1, cards::name(state))
            } else {
                format!("Player {} returns {}", player + 1, cards::name(state))
            }
        }
        Event::TakeDebt { player, source, n } => format!("Player {} takes {n} Debt ({})", player + 1, cards::name(source)),
        Event::PayDebt { player, n } => format!("Player {} pays off {n} Debt", player + 1),
        Event::VpTokens { player, source, n } => format!("Player {} takes {n} VP token(s) ({})", player + 1, cards::name(source)),
        Event::PileVp { pile, n } => format!("{n} VP token(s) are put on {}", cards::name(pile)),
        Event::PileDebt { pile, n } => format!("{n} Debt is put on the {} pile", cards::name(pile)),
        Event::Sun { player, source, left } => {
            format!("Player {} removes a Sun token ({}): {left} left on {}", player + 1, cards::name(source), "the Prophecy")
        }
        Event::Prophecy { prophecy, .. } => {
            format!("*** The last Sun token is gone: {} is in effect ***", cards::name(prophecy))
        }
        Event::PlayAgain { player, card, source, nth } => {
            let again = if nth == 2 { "again".to_string() } else { format!("for the {} time", ordinal(nth)) };
            format!("Player {} plays {} {again} ({})", player + 1, cards::name(card), cards::name(source))
        }
        Event::PhaseStart { player, phase } => {
            let name = match phase {
                Phase::Action => "Action",
                Phase::Buy => "Buy",
                Phase::Night => "Night",
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
        Zone::InPlay => " from play",
        Zone::Supply => " from the Supply",
        Zone::Trash => " from the trash",
        Zone::DeckTop => " from the top of your deck",
        Zone::Tavern => " from your Tavern mat",
        Zone::Archive => " from the cards under your Archives",
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
        Filter::VictoryOrCurse => ("a Victory card or Curse".into(), "Victory cards and Curses".into()),
        Filter::ActionOrTreasure => ("an Action or Treasure card".into(), "Action or Treasure cards".into()),
        Filter::TreasureOrCurse => ("a Treasure".into(), "Treasures".into()),
        Filter::Either(a, b) => (format!("a {} or {}", cards::name(a), cards::name(b)), format!("{}s and {}s", cards::name(a), cards::name(b))),
        Filter::NonAction => ("a non-Action card".into(), "non-Action cards".into()),
        Filter::ActionNot(c) => (format!("an Action card other than {}", cards::name(c)), format!("Action cards other than {}", cards::name(c))),
        Filter::ActionCostUpTo(n) => (format!("an Action card costing up to ${n}"), format!("Action cards costing up to ${n}")),
        Filter::NotCard(c) => (format!("a card other than {}", cards::name(c)), format!("cards other than {}", cards::name(c))),
        Filter::NotInHand => ("a card you have no copy of in hand".into(), "cards you have no copy of in hand".into()),
        Filter::ActionNotReward => ("an Action card other than a Reward".into(), "Action cards other than Rewards".into()),
        Filter::TreasureNotReward => ("a Treasure other than a Reward".into(), "Treasures other than Rewards".into()),
        Filter::TreasureToDiscard => ("a Treasure".into(), "Treasures".into()),
        Filter::NotInPlayAction => (
            "an Action card you have no copy of in play".into(),
            "Action cards you have no copy of in play".into(),
        ),
        Filter::GainableCopy => ("a card you can gain a copy of".into(), "cards you can gain a copy of".into()),
        Filter::InheritCard => ("a non-Duration, non-Command Action card costing up to $4".into(), "such Action cards".into()),
        Filter::NonVictory => ("a non-Victory card".into(), "non-Victory cards".into()),
        Filter::ActionToDiscard => ("an Action card".into(), "Action cards".into()),
        Filter::CommandCostUpTo(n) => (
            format!("a non-Duration, non-Command Action card costing up to ${n}"),
            format!("non-Duration, non-Command Action cards costing up to ${n}"),
        ),
        Filter::Types(set) => match set {
            dominion_engine::state::TypeSet::Attack => ("an Attack card".into(), "Attack cards".into()),
            dominion_engine::state::TypeSet::Ruins => ("a Ruins".into(), "Ruins".into()),
            dominion_engine::state::TypeSet::Action => ("an Action card".into(), "Action cards".into()),
            dominion_engine::state::TypeSet::ActionNotDuration => ("a non-Duration Action card".into(), "non-Duration Action cards".into()),
            dominion_engine::state::TypeSet::CurseRuinsShelterVictory => {
                ("a Curse, Ruins, Shelter or Victory card".into(), "Curses, Ruins, Shelters and Victory cards".into())
            }
            dominion_engine::state::TypeSet::Castle => ("a Castle".into(), "Castles".into()),
            dominion_engine::state::TypeSet::ActionNotAttack => ("a non-Attack Action card".into(), "non-Attack Action cards".into()),
        },
        Filter::NonTreasure => ("a non-Treasure card".into(), "non-Treasure cards".into()),
        Filter::TreasureNonDuration => ("a non-Duration Treasure".into(), "non-Duration Treasures".into()),
        Filter::CostAtLeast(n) => (format!("a card costing ${n} or more"), format!("cards costing ${n} or more")),
        Filter::CostRange(lo, hi) => (format!("a card costing from ${lo} to ${hi}"), format!("cards costing from ${lo} to ${hi}")),
        Filter::VictoryNot(c) => (format!("a Victory card other than {}", cards::name(c)), format!("Victory cards other than {}", cards::name(c))),
        Filter::TreasureNot(c) => (format!("a Treasure other than {}", cards::name(c)), format!("Treasures other than {}", cards::name(c))),
        Filter::CopyInAttackersPlay => ("a copy of a card in play".into(), "copies of cards in play".into()),
        Filter::NecroTarget => ("a non-Duration Action card".into(), "non-Duration Action cards".into()),
        Filter::SharesTypeWith(c) => (format!("a card sharing a type with {}", cards::name(c)), format!("cards sharing a type with {}", cards::name(c))),
    }
}

/// Card-text style description, generic over the decision's shape:
/// "Player 1 - Throne Room: You may play an Action card from your hand twice."
/// A reaction revealed from hand to act on a card just gained (Watchtower): a Select whose source
/// is a Reaction card, acting on a card outside the hand. Returns (reaction, gained card, act).
fn gain_reaction(d: &Decision) -> Option<(cards::CardId, cards::CardId, Act)> {
    let src = d.source?;
    match d.kind {
        DecisionKind::Select { from, act, filter: Filter::Card(c), .. } if from != Zone::Hand && cards::is(src, cards::REACTION) => Some((src, c, act)),
        _ => None,
    }
}

/// A Reaction offered against an Attack that was just played (Moat): a YesNo to reveal the card,
/// whose source is the Attack. Returns (reaction, attack).
fn block_reaction(d: &Decision) -> Option<(cards::CardId, cards::CardId)> {
    let src = d.source?;
    match d.kind {
        DecisionKind::YesNo { act: Act::Reveal } if src != d.subject && cards::is(src, cards::ATTACK) && cards::is(d.subject, cards::REACTION) => {
            Some((d.subject, src))
        }
        _ => None,
    }
}

fn decision_description(state: &GameState, d: &Decision) -> String {
    let who = format!("Player {}", d.player + 1);
    if let Some((reaction, attack)) = block_reaction(d) {
        return format!("{who}: Reveal {} to block {}?", cards::name(reaction), cards::name(attack));
    }
    if let Some((src, c, act)) = gain_reaction(d) {
        let what = if act == Act::Trash { "trash" } else { "put onto your deck" };
        return format!("{who}: You may reveal {} from your hand to {what} the {} you just gained.", cards::name(src), cards::name(c));
    }
    let head = match d.source {
        Some(c) => format!("{who} \u{2014} {}: ", cards::name(c)),
        None => format!("{who}: "),
    };
    let body = match d.kind {
        DecisionKind::PlayAction => format!("You may play an Action card ({} action(s) left).", state.turn.actions),
        DecisionKind::Buy => {
            let potions = if state.in_supply(id::POTION) { format!(", {} Potion(s)", state.turn.potions) } else { String::new() };
            let coffers = state.players[d.player as usize].coffers;
            let coffers = if coffers > 0 { format!("; {coffers} Coffers to spend for +$1 each") } else { String::new() };
            format!("You may buy a card (${}{potions} available, {} buy(s) left{coffers}).", state.turn.coins, state.turn.buys)
        }
        DecisionKind::Overpay { card, max_coins, potion } => {
            let p = if potion { " (and a Potion)" } else { "" };
            format!("You may overpay for {} by up to ${max_coins}{p}.", cards::name(card))
        }
        DecisionKind::PlayTreasure => format!("You may play a Treasure card (${} so far).", state.turn.coins),
        DecisionKind::PlayNight => "Night phase: you may play a Night card.".to_string(),
        DecisionKind::PickBoon => "Choose a Boon to receive.".to_string(),
        DecisionKind::Gain { max_cost, filter, dest, exact, potion, optional } => {
            let (one, _) = filter_noun(filter);
            let top = state.stack.top().filter(|f| f.kind == dominion_engine::state::FrameKind::Gain);
            let less = top.is_some_and(|f| f.less);
            let both = top.is_some_and(|f| f.gf & dominion_engine::state::GF_BOTH != 0);
            let cost = if both { "exactly" } else if exact { "exactly" } else if less { "less than" } else { "up to" };
            let p = if potion { "P" } else { "" };
            let may = if optional { "You may gain" } else { "Gain" };
            let or = if both { format!(" or ${}{p}", max_cost + 2) } else { String::new() };
            format!("{may} {one} costing {cost} ${max_cost}{p}{or}{}.", dest_phrase(dest))
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
                Act::Gain => ("gain", String::new()),
                Act::Reveal => ("reveal", String::new()),
                Act::Pass => ("pass", " to the next player".to_string()),
                Act::Tavern => ("put", " onto your Tavern mat".to_string()),
                Act::Call => ("call", String::new()),
                Act::Exchange => ("exchange", String::new()),
                Act::ToHand => ("take", " into your hand".to_string()),
                Act::Return => ("return", " to its pile".to_string()),
                Act::Archive => ("set aside", " face down under this".to_string()),
                Act::ShuffleIn => ("shuffle", " into your deck".to_string()),
            };
            let zone = zone_phrase(from);
            let order = if ordered && act == Act::Topdeck { " (one at a time; the last one ends on top)" } else { "" };
            let upgrade = match d.upgrade {
                Some(u) if u.dest_by_type => format!(
                    " Then gain {} costing up to ${} more than it: onto your deck if it's an Action or Treasure; if it's a Victory card, each other player gains a Curse.",
                    filter_noun(u.filter).0,
                    u.plus
                ),
                Some(u) => {
                    let cost = if u.exact { "exactly" } else { "up to" };
                    format!(" Then gain {} costing {cost} ${} more than it{}.", filter_noun(u.filter).0, u.plus, dest_phrase(u.dest))
                }
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
                Act::Gain => format!("Gain {card}?"),
                Act::Reveal if cards::is_boon(d.subject) => format!("Receive {card}?"),
                Act::Reveal => format!("Reveal {card}?"),
                Act::Pass => format!("Pass {card}?"),
                Act::Tavern => format!("Put {card} on your Tavern mat?"),
                Act::Call => format!("Call {card}?"),
                Act::Exchange => format!("Exchange {card} for the next card of its line?"),
                Act::ToHand => format!("Take {card} into your hand?"),
                Act::Return if d.source == Some(id::CHANGELING) => format!("Exchange {card} for a Changeling?"),
                Act::Return if d.source == Some(id::TRADER) => format!("Reveal a Trader to exchange {card} for a Silver?"),
                Act::Return => format!("Return {card} to its pile?"),
                Act::Archive => format!("Set aside {card}?"),
                Act::ShuffleIn => format!("Shuffle {card} into your deck?"),
            }
        }
        DecisionKind::Token { token } if token as usize == dominion_engine::sets::empires::TOK_TAX => "Put 2 Debt on a Supply pile.".to_string(),
        DecisionKind::Token { token } => format!("Move your {} token to a pile.", cards::TOKEN_NAMES[token as usize]),
        DecisionKind::Bid { min } => format!("Mountain Pass: bid Debt for 8 VP (at least {min}, or no bid)."),
        DecisionKind::Mode { picks, distinct } => {
            let what = if picks == 1 {
                "one".to_string()
            } else if distinct {
                format!("{picks} different")
            } else {
                format!("{picks}")
            };
            format!("Choose {what}:")
        }
        DecisionKind::StartOrder => "Choose which start-of-turn effect resolves next.".to_string(),
        DecisionKind::Name => "Name a card.".to_string(),
        DecisionKind::DeckPosition { max_known } => {
            if max_known == 0 {
                "Put it into your deck.".to_string()
            } else {
                format!("Put it into your deck: on top, below one of the top {max_known} known card(s), or on the bottom.")
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

/// The label of one start-of-turn effect in a `DecisionKind::StartOrder` choice ("Sea Witch: +2
/// Cards, then discard 2", "Haven: put Gold into your hand", "Prince: play Smithy"). `name` is
/// the choice's card; the pending effect it stands for is looked up in the turn's list.
fn start_effect_label(state: &GameState, name: cards::CardId) -> String {
    let list = state.turn.start_pending.as_slice();
    let held = |e: &dominion_engine::state::PendingDuration| e.card == id::PRINCE || e.card == id::FARMHANDS;
    let entry = list.iter().find(|e| if held(e) { e.arg == name } else { e.card == name });
    if let Some(e) = entry.filter(|e| held(e)) {
        return format!("{}: play {}", cards::name(e.card), cards::name(name));
    }
    let card = cards::name(name);
    let arg = entry.map(|e| e.arg).filter(|&a| a != 0).map(cards::name).unwrap_or("its card");
    match name {
        id::HAVEN | id::BLOCKADE => format!("{card}: put {arg} into your hand"),
        id::PIRATE => format!("{card}: gain a Treasure costing up to $6 to your hand"),
        id::SEA_WITCH => format!("{card}: +2 Cards, then discard 2"),
        id::TIDE_POOLS => format!("{card}: discard 2 cards"),
        id::SAILOR => format!("{card}: +$2, then you may trash a card from your hand"),
        id::CLERK => format!("{card}: you may play it from your hand"),
        id::CAPTAIN => format!("{card}: play a non-Duration Action costing up to $4 from the Supply"),
        _ => {
            let (cards_n, actions, buys, coins) = cards::duration_bonus(name);
            let mut parts: Vec<String> = Vec::new();
            if cards_n > 0 {
                parts.push(format!("+{cards_n} Card{}", if cards_n == 1 { "" } else { "s" }));
            }
            let bonus = bonus_text(actions, buys, coins as u16, 0, false);
            if !bonus.is_empty() {
                parts.push(bonus);
            }
            if parts.is_empty() { card.to_string() } else { format!("{card}: {}", parts.join(", ")) }
        }
    }
}

fn choice_label(state: &GameState, d: &Decision, c: Choice) -> String {
    if let Some((reaction, _)) = block_reaction(d) {
        return match c {
            Choice::Yes => format!("Reveal {}", cards::name(reaction)),
            _ => "Don't reveal".to_string(),
        };
    }
    if let Some((src, card, act)) = gain_reaction(d) {
        return match c {
            Choice::Card(_) if act == Act::Trash => format!("Reveal {}: trash {}", cards::name(src), cards::name(card)),
            Choice::Card(_) => format!("Reveal {}: put {} on deck", cards::name(src), cards::name(card)),
            _ => "Don't reveal".to_string(),
        };
    }
    match c {
        Choice::Coffers => "Spend a Coffers token (+$1)".to_string(),
        Choice::PayDebt => "Pay off Debt".to_string(),
        Choice::PayDebtKeep(n) => format!("Pay off Debt, keeping {n}"),
        Choice::Bid(0) => "No bid".to_string(),
        Choice::Bid(n) => format!("Bid {n} Debt"),
        Choice::Overpay(0) => "Don't overpay".to_string(),
        Choice::Overpay(coins) => format!("Overpay ${coins}"),
        Choice::OverpayPotion(coins) => format!("Overpay ${coins} + Potion"),
        Choice::Pass => match d.kind {
            DecisionKind::PlayTreasure => "Done playing treasures".to_string(),
            DecisionKind::PlayAction => "Done playing actions".to_string(),
            DecisionKind::PlayNight => "Done (end the Night phase)".to_string(),
            DecisionKind::Buy => "Done buying".to_string(),
            DecisionKind::Gain { .. } => "Don't gain".to_string(),
            _ => "Done".to_string(),
        },
        Choice::Yes => "Yes".to_string(),
        Choice::No => "No".to_string(),
        Choice::Mode(i) => d.source.and_then(|src| cards::modes(src).get(i as usize)).map(|o| o.label()).unwrap_or_default(),
        Choice::Card(card) => {
            let name = cards::name(card);
            match d.kind {
                DecisionKind::PlayAction => format!("Play {name}"),
                DecisionKind::Buy => format!("Buy {name}"),
                DecisionKind::PlayTreasure if cards::is_event(card) => format!("Buy {name}"),
                DecisionKind::PlayTreasure => format!("Play {name}"),
                DecisionKind::Gain { .. } => format!("Gain {name}"),
                DecisionKind::Select { act, .. } => match act {
                    Act::Discard => format!("Discard {name}"),
                    Act::Trash => format!("Trash {name}"),
                    Act::Topdeck => format!("Put {name} on deck"),
                    Act::Play if d.play_times > 1 => format!("Play {name} (x{})", d.play_times),
                    Act::Play => format!("Play {name}"),
                    Act::SetAside => format!("Set aside {name}"),
                    Act::Gain => format!("Gain {name}"),
                    Act::Reveal => format!("Reveal {name}"),
                    Act::Pass => format!("Pass {name}"),
                    Act::Tavern => format!("Put {name} on the mat"),
                    Act::Call => format!("Call {name}"),
                    Act::Exchange => format!("Exchange {name}"),
                    Act::ToHand => format!("Take {name} into your hand"),
                    Act::Return => format!("Return {name}"),
                    Act::Archive => format!("Set aside {name}"),
                    Act::ShuffleIn => format!("Shuffle {name} into the deck"),
                },
                DecisionKind::PlayNight => format!("Play {name} (Night)"),
                DecisionKind::PickBoon => format!("Receive {name}"),
                DecisionKind::Token { .. } => format!("Put it on {name}"),
                DecisionKind::YesNo { .. } => name.to_string(),
                DecisionKind::Mode { .. } => name.to_string(),
                DecisionKind::Name => format!("Name {name}"),
                DecisionKind::StartOrder => start_effect_label(state, card),
                DecisionKind::DeckPosition { .. } => name.to_string(),
                DecisionKind::Overpay { .. } => name.to_string(),
                DecisionKind::Bid { .. } => name.to_string(),
            }
        }
        // Start of turn: which Church batch goes into the hand first.
        Choice::Position(k) if d.kind == DecisionKind::StartOrder => {
            let (batch, n) = state.players[state.turn.player as usize].church.batch(k as usize);
            let names: Vec<&str> = batch[..n].iter().map(|&c| cards::name(c)).collect();
            if names.is_empty() { "Church: an empty batch".to_string() } else { format!("Church: put {} into your hand", names.join(", ")) }
        }
        Choice::Position(255) => "Bottom".to_string(),
        Choice::Position(0) => "Top".to_string(),
        Choice::Position(k) => {
            let suffix = match (k % 10, k % 100) {
                (1, x) if x != 11 => "st",
                (2, x) if x != 12 => "nd",
                (3, x) if x != 13 => "rd",
                _ => "th",
            };
            format!("Below the {k}{suffix} known card")
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
fn sequence_json(iter: impl Iterator<Item = CardId>) -> String {
    let items: Vec<String> = iter.map(|id| jstr(cards::name(id))).collect();
    format!("[{}]", items.join(","))
}

/// `[{"name":"Alms","cost":0,"buyable":true},...]`: the Events of the game (Adventures), `buyable` for the turn's player right now.
fn events_json(state: &GameState) -> String {
    let items: Vec<String> = (0..cards::NUM_EVENTS as CardId)
        .filter(|i| state.events & (1u64 << i) != 0)
        .map(|i| {
            let e = cards::FIRST_EVENT + i;
            let buyable = state.turn.phase == Phase::Buy && dominion_engine::sets::adventures::event_buyable(state, e) && state.turn.buys > 0;
            format!("{{\"name\":{},\"cost\":{},\"costLabel\":{},\"buyable\":{buyable}}}", jstr(cards::name(e)), cards::cost(e), jstr(&cards::cost_string(e)))
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// `[{"name":"Arena","vp":12},...]`: the Landmarks of the game (Empires) and the VP tokens on each (`vp` is 0 for the scoring Landmarks).
fn landmarks_json(state: &GameState) -> String {
    let items: Vec<String> = (0..cards::NUM_LANDMARKS)
        .filter(|&i| state.emp.landmarks & (1 << i) != 0)
        .map(|i| format!("{{\"name\":{},\"vp\":{}}}", jstr(cards::name(cards::FIRST_LANDMARK + i as CardId)), state.emp.lm_vp[i]))
        .collect();
    format!("[{}]", items.join(","))
}

/// `{"name":"Bureaucracy","suns":3,"active":false}` or `null`: the Prophecy of the game (Rising Sun), the Sun tokens left on it and whether it has happened.
fn prophecy_json(state: &GameState) -> String {
    let p = state.rs.prophecy;
    if p == 0 {
        return "null".to_string();
    }
    format!("{{\"name\":{},\"suns\":{},\"active\":{}}}", jstr(cards::name(p)), state.rs.suns, state.rs.active)
}

fn supply_json(state: &GameState) -> String {
    // The Knights and Ruins piles are one entry each, showing the top card (the only one that can be bought) and the cards left.
    let mut ids: Vec<CardId> = (0..cards::FIRST_PILE_CARD).filter(|&c| state.in_supply(c) && !cards::is_castle(c)).collect();
    // The split pile Sauna/Avanto is one entry: the top half.
    if state.in_supply(id::SAUNA) {
        ids.retain(|&c| c != id::AVANTO && c != id::SAUNA);
        ids.push(if state.supply.get(id::SAUNA) > 0 { id::SAUNA } else { id::AVANTO });
    }
    let mut piles: Vec<(CardId, u8, &'static str)> = Vec::new(); // (card shown, cards left, pile name)
    for (pile, name) in [(&dominion_engine::sets::dark_ages::RUINS_PILE, "Ruins"), (&dominion_engine::sets::dark_ages::KNIGHTS_PILE, "Knights")] {
        if state.has_pile(pile) {
            piles.push((state.pile_top(pile).unwrap_or(pile.marker), state.pile_size(pile), name));
        }
    }
    // Empires: the Castles are one pile, showing the top Castle (the only one that can be bought).
    if let Some(top) = state.castle_top() {
        piles.push((top, state.castles_left() as u8, "Castles"));
    }
    ids.extend(piles.iter().map(|p| p.0));
    // Victory (Colony alongside Province), then treasure high to low (Platinum alongside Gold),
    // then Curse, then kingdom cards cheapest first.
    const BASE_ORDER: [CardId; 10] =
        [id::COLONY, id::PROVINCE, id::DUCHY, id::ESTATE, id::PLATINUM, id::GOLD, id::SILVER, id::COPPER, id::CURSE, id::POTION];
    ids.sort_by_key(|&c| match BASE_ORDER.iter().position(|&b| b == c) {
        Some(i) => (0, i as u8, ""),
        None => (1, cards::cost(c), cards::name(c)),
    });
    let items: Vec<String> = ids
        .iter()
        .map(|&c| {
            format!(
                "{{\"id\":{c},\"name\":{},\"cost\":{},\"costLabel\":{},\"count\":{}{}}}",
                jstr(cards::name(c)),
                cards::cost(c),
                jstr(&cards::cost_string(c)),
                match piles.iter().find(|p| p.0 == c) {
                    Some(p) => p.1,
                    None => state.supply.get(c),
                },
                match piles.iter().find(|p| p.0 == c) {
                    Some(p) => format!(",\"pile\":\"{}\"", p.2),
                    None if c == state.bane && c != 0 => ",\"bane\":true".to_string(),
                    None => String::new(),
                } + &{
                    // Adventures tokens on the pile, by owner.
                    let pile = cards::pile_of(c);
                    let mut ts: Vec<String> = Vec::new();
                    for q in 0..state.num_players as usize {
                        for (i, &t) in state.players[q].tok.iter().enumerate() {
                            if t != 0 && t == pile {
                                ts.push(jstr(&format!("P{} {}", q + 1, cards::TOKEN_NAMES[i])));
                            }
                        }
                    }
                    if ts.is_empty() { String::new() } else { format!(",\"tokens\":[{}]", ts.join(",")) }
                } + &{
                    // Empires: VP and Debt tokens on the pile, and which half of a split pile this is.
                    let (vp, debt) = (state.emp.piles.vp_on(c), state.emp.piles.debt_on(c));
                    let mut s = String::new();
                    if vp > 0 {
                        s += &format!(",\"vp\":{vp}");
                    }
                    if debt > 0 {
                        s += &format!(",\"debtTokens\":{debt}");
                    }
                    if cards::is_split_bottom(c) && c != id::AVANTO {
                        s += ",\"half\":\"bottom\"";
                    } else if cards::split_bottom(c).is_some() && c != id::SAUNA {
                        s += ",\"half\":\"top\"";
                    }
                    s
                }
            )
        })
        .collect();
    // Piles outside the Supply (Ferryman's pile and the Rewards), marked "side": they cannot be bought.
    let mut items = items;
    let mut side = |c: CardId, n: u8| {
        items.push(format!(
            "{{\"id\":{c},\"name\":{},\"cost\":{},\"costLabel\":{},\"count\":{n},\"side\":true}}",
            jstr(cards::name(c)),
            cards::cost(c),
            jstr(&format!("{}*", cards::cost_string(c)))
        ))
    };
    if state.ferryman != 0 {
        side(state.ferryman, state.ferryman_left);
    }
    if state.in_supply(id::JOUST) {
        for (i, &n) in state.rewards.iter().enumerate() {
            side(cards::FIRST_REWARD + i as CardId, n);
        }
    }
    // Adventures: the Traveller upgrade piles of the lines in the game (not in the Supply).
    for i in 0..cards::NUM_TRAV {
        let line = if i < 4 { id::PAGE } else { id::PEASANT };
        if state.in_supply(line) {
            side(cards::FIRST_TRAV + i as CardId, state.trav[i]);
        }
    }
    // Dark Ages: Spoils, Madman and Mercenary, while a card that gives them is in the game.
    for (c, n, gainers) in [
        (id::SPOILS, state.spoils, &[id::BANDIT_CAMP, id::MARAUDER, id::PILLAGE][..]),
        (id::MADMAN, state.madman, &[id::HERMIT][..]),
        (id::MERCENARY, state.mercenary, &[id::URCHIN][..]),
    ] {
        if n > 0 || gainers.iter().any(|&g| state.in_supply(g)) {
            side(c, n);
        }
    }
    // Nocturne: Will-o'-Wisp, Wish, Bat, Imp and Ghost, while a card that brings them is in the game (not in the Supply).
    let wanted = [
        (id::WILL_O_WISP, state.noc.flags & dominion_engine::state::N_BOONS != 0 || state.in_supply(id::EXORCIST)),
        (id::WISH, state.in_supply(id::LEPRECHAUN) || state.in_supply(id::SECRET_CAVE)),
        (id::BAT, state.in_supply(id::VAMPIRE)),
        (id::IMP, state.in_supply(id::DEVILS_WORKSHOP) || state.in_supply(id::TORMENTOR) || state.in_supply(id::EXORCIST)),
        (id::GHOST, state.in_supply(id::CEMETERY) || state.in_supply(id::EXORCIST)),
    ];
    for (i, (c, brought)) in wanted.into_iter().enumerate() {
        if brought || state.noc.piles[i] > 0 {
            side(c, state.noc.piles[i]);
        }
    }
    format!("[{}]", items.join(","))
}

/// `[{"card":"Fishing Village","times":2,"arg":null},...]`, the pending start-of-next-turn
/// Duration effects (Seaside): `arg` is the set-aside/gained card name (Haven/Blockade) or
/// `"used"` (Sailor's spent reactions; `"used 1"` with plays left over), else `null`.
fn durations_json(state: &GameState, p: usize) -> String {
    let ps = &state.players[p];
    let items: Vec<String> = ps.pending_durations[..ps.pending_durations_len as usize]
        .iter()
        .map(|e| {
            let arg = if e.arg != 0 {
                jstr(cards::name(e.arg))
            } else if e.used >= e.times.max(1) {
                jstr("used")
            } else if e.used > 0 {
                jstr(&format!("used {}", e.used))
            } else {
                "null".to_string()
            };
            format!("{{\"card\":{},\"times\":{},\"arg\":{arg}}}", jstr(cards::name(e.card)), e.times)
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn player_json(state: &GameState, p: usize) -> String {
    let ps = &state.players[p];
    // Native Village mat is normally private to its owner (opponents know only the count), but
    // this JSON view is omniscient (the same page shows every player's hand), so it's shown for
    // every player here too, same as `durations` already exposing Haven's set-aside card.
    format!(
        concat!(
            "{{\"index\":{p},\"isCurrent\":{cur},",
            "\"hand\":{hand},\"handSize\":{hand_n},",
            "\"deckTop\":{deck_top},\"deckUnknown\":{deck_unk},\"deckBottom\":{deck_bottom},\"deckSize\":{deck_n},",
            "\"discard\":{discard},\"inPlay\":{in_play},\"setAside\":{set_aside},\"durations\":{durations},",
            "\"nativeVillageMat\":{nv_mat},\"islandMat\":{island_mat},",
            "\"princeCards\":{prince_cards},\"coffers\":{coffers},\"farmCards\":{farm_cards},\"summonCards\":{summon_cards},\"churchCards\":{church_cards},\"cleanupAside\":{cleanup_aside},",
            "\"tavern\":{tavern},\"tokens\":{tokens},\"journeyDown\":{journey},\"minusCard\":{mcard},\"minusCoin\":{mcoin},\"inherit\":{inherit},",
            "\"saved\":{saved},\"boughtEvents\":{bought},\"debt\":{debt},\"archive\":{archive},",
            "\"states\":{states},\"hounds\":{hounds},",
            "\"vp\":{vp},\"vpTokens\":{vp_tokens},\"turnsTaken\":{turns}}}"
        ),
        tavern = counts_json(&ps.tavern.counts()),
        tokens = {
            let items: Vec<String> = ps
                .tok
                .iter()
                .enumerate()
                .filter(|&(_, &pile)| pile != 0)
                .map(|(i, &pile)| format!("{{\"token\":{},\"pile\":{}}}", jstr(cards::TOKEN_NAMES[i]), jstr(cards::name(pile))))
                .collect();
            format!("[{}]", items.join(","))
        },
        journey = ps.adv & dominion_engine::state::ADV_JOURNEY_DOWN != 0,
        mcard = ps.adv & dominion_engine::state::ADV_MINUS_CARD != 0,
        mcoin = ps.adv & dominion_engine::state::ADV_MINUS_COIN != 0,
        inherit = if ps.inherit != 0 { jstr(cards::name(ps.inherit)) } else { "null".to_string() },
        saved = if ps.saved != 0 { jstr(cards::name(ps.saved)) } else { "null".to_string() },
        bought = {
            let items: Vec<String> = (0..cards::NUM_EVENTS as CardId)
                .filter(|i| ps.events_bought & (1u64 << i) != 0)
                .map(|i| jstr(cards::name(cards::FIRST_EVENT + i)))
                .collect();
            format!("[{}]", items.join(","))
        },
        p = p,
        cur = state.turn.player as usize == p,
        hand = counts_json(&ps.hand),
        hand_n = ps.hand.total(),
        deck_top = sequence_json(ps.deck_known.iter_top_down()),
        deck_unk = counts_json(&ps.deck_unknown),
        deck_bottom = sequence_json(ps.deck_known_bottom.iter_front_to_back()),
        deck_n = ps.deck_size(),
        discard = counts_json(&ps.discard),
        in_play = counts_json(&ps.in_play),
        set_aside = counts_json(&ps.set_aside),
        durations = durations_json(state, p),
        nv_mat = counts_json(&ps.native_village_mat.counts()),
        island_mat = counts_json(&ps.island_mat.counts()),
        prince_cards = counts_json(&ps.prince_cards.counts()),
        coffers = ps.coffers,
        farm_cards = counts_json(&ps.farm_cards.counts()),
        summon_cards = counts_json(&ps.summon_cards.counts()),
        church_cards = {
            let mut c = Counts::EMPTY;
            ps.church.add_into(&mut c);
            counts_json(&c)
        },
        cleanup_aside = counts_json(&ps.cleanup_aside.counts()),
        debt = ps.debt,
        archive = counts_json(&ps.archive.counts()),
        states = states_json(state, p),
        hounds = ps.hounds,
        vp = ps.vp(),
        vp_tokens = ps.vp_tokens,
        turns = ps.turns_taken,
    )
}

fn pending_json(state: &GameState) -> String {
    if state.pending_decision().is_none() && state.turn.phase != Phase::GameOver {
        // A new game says "Deal"; a real turn boundary "Start turn"; a loaded or mid-turn
        // position "Continue".
        let setup = state.turn.phase == Phase::Setup;
        let label = if setup { "Deal" } else if state.turn.announced { "Continue" } else { "Start turn" };
        let what = if setup {
            "Opening hands not dealt yet; Simulate from here deals fresh ones in every game."
        } else if state.turn.announced {
            "Position loaded; the next step plays on from here."
        } else {
            ""
        };
        return format!(
            "{{\"player\":{},\"source\":null,\"paused\":true,\"description\":{},\"choices\":[{{\"index\":-1,\"label\":\"{label}\"}}]}}",
            state.turn.player,
            jstr(&format!("Turn {}: Player {} is up. {what}", state.turn.number, state.turn.player + 1).trim_end())
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
        .map(|(i, &c)| format!("{{\"index\":{i},\"label\":{}}}", jstr(&choice_label(state, &d, c))))
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
        Phase::Night => "night",
        Phase::CleanupDraw => "cleanup",
        Phase::GameOver => "gameover",
    }
}

/// Nocturne: which Boons and Hexes are in the game and where they are, the States, Lost in the Woods. Null in a game without Boons, Hexes or Night cards.
/// The piles whose cards are not all alike, in pile order (top first), for the "Piles" section:
/// `[{"name":"Castles","order":"fixed","cards":[["Humble Castle",2],...]},
///   {"name":"Knights","order":"dealt","cards":["Dame Anna","Sir Bailey",...]}]`.
/// The Castles are in a fixed order, known to everyone (split piles are not listed: the Supply shows both halves). Knights and Ruins are
/// shuffled: the cards are listed as dealt, one name each; only the first is face up in the game.
fn pile_order_json(state: &GameState) -> String {
    use dominion_engine::sets::dark_ages::{KNIGHTS_PILE, RUINS_PILE};
    let list = |v: &[(CardId, u8)]| -> String {
        let items: Vec<String> = v.iter().map(|&(c, n)| format!("[{},{n}]", jstr(cards::name(c)))).collect();
        format!("[{}]", items.join(","))
    };
    let mut out: Vec<String> = Vec::new();
    if state.castle_top().is_some() || (id::HUMBLE_CASTLE..id::HUMBLE_CASTLE + 8).any(|c| state.in_supply(c)) {
        let v: Vec<(CardId, u8)> = (id::HUMBLE_CASTLE..id::HUMBLE_CASTLE + 8).filter(|&c| state.supply.get(c) > 0).map(|c| (c, state.supply.get(c))).collect();
        out.push(format!("{{\"name\":\"Castles\",\"order\":\"fixed\",\"cards\":{}}}", list(&v)));
    }
    // Knights and Ruins: every card in the dealt order, top first. Only the top is face up; the page shows the others dimmed (it shows
    // everything, both hands and decks included).
    for (pile, name) in [(&KNIGHTS_PILE, "Knights"), (&RUINS_PILE, "Ruins")] {
        if state.has_pile(pile) {
            let (order, len) = state.pile_cards(pile);
            let names: Vec<String> = order[..len].iter().map(|&c| jstr(cards::name(c))).collect();
            out.push(format!("{{\"name\":\"{name}\",\"order\":\"dealt\",\"cards\":[{}]}}", names.join(",")));
        }
    }
    format!("[{}]", out.join(","))
}

fn nocturne_json(state: &GameState) -> String {
    use dominion_engine::state::{N_BOONS, N_HEXES, N_NIGHT};
    let n = &state.noc;
    if n.flags & (N_BOONS | N_HEXES | N_NIGHT) == 0 {
        return "null".to_string();
    }
    let names = |mask: u16, first: CardId| -> String {
        let v: Vec<String> = (0..12u16).filter(|i| mask & (1 << i) != 0).map(|i| jstr(cards::name(first + i))).collect();
        format!("[{}]", v.join(","))
    };
    format!(
        concat!(
            "{{\"boons\":{boons},\"hexes\":{hexes},\"night\":{night},",
            "\"boonDeck\":{bd},\"boonDiscard\":{bdi},\"druid\":{dr},\"boonsHeld\":{bh},\"hexDeck\":{hd},\"hexDiscard\":{hdi},\"lost\":{lost}}}"
        ),
        boons = n.flags & N_BOONS != 0,
        hexes = n.flags & N_HEXES != 0,
        night = n.flags & N_NIGHT != 0,
        bd = names(n.boon_deck, cards::FIRST_BOON),
        bdi = names(n.boon_discard, cards::FIRST_BOON),
        dr = names(n.boon_druid, cards::FIRST_BOON),
        bh = names(n.boon_held, cards::FIRST_BOON),
        hd = names(n.hex_deck, cards::FIRST_HEX),
        hdi = names(n.hex_discard, cards::FIRST_HEX),
        lost = if n.lost != 0 { (n.lost - 1).to_string() } else { "null".to_string() },
    )
}

/// The States in front of player `p` (names), Nocturne.
fn states_json(state: &GameState, p: usize) -> String {
    use dominion_engine::state::*;
    let f = state.players[p].noc;
    let mut v: Vec<&str> = Vec::new();
    if state.noc.lost == p as u8 + 1 {
        v.push("Lost in the Woods");
    }
    if f & P_DELUDED != 0 {
        v.push("Deluded");
    }
    if f & P_ENVIOUS != 0 {
        v.push("Envious");
    }
    if f & P_TWICE_MISERABLE != 0 {
        v.push("Twice Miserable");
    } else if f & P_MISERABLE != 0 {
        v.push("Miserable");
    }
    if f & P_GUARDED != 0 {
        v.push("Guarded");
    }
    let items: Vec<String> = v.iter().map(|s| jstr(s)).collect();
    format!("[{}]", items.join(","))
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
            "\"turn\":{{\"number\":{tn},\"player\":{tp},\"phase\":{phase},\"actions\":{ta},\"buys\":{tb},\"coins\":{tc},\"potions\":{tpot}}},",
            "\"supply\":{supply},\"events\":{events},\"landmarks\":{landmarks},\"prophecy\":{prophecy},\"nocturne\":{nocturne},\"pileOrder\":{pile_order},\"trash\":{trash},\"players\":[{players}],",
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
        tpot = state.turn.potions,
        supply = supply_json(state),
        events = events_json(state),
        landmarks = landmarks_json(state),
        prophecy = prophecy_json(state),
        nocturne = nocturne_json(state),
        pile_order = pile_order_json(state),
        trash = counts_json(&state.trash),
        players = players.join(","),
        scores = scores_json.join(","),
        go = game_over,
        winners = winners_json,
        pending = pending_json(state),
        log = log_json.join(","),
    )
}

#[cfg(test)]
mod log_tests {
    use super::*;
    use dominion_engine::{ChoiceBuf, NoEvents};

    fn log_of(events: &[Event]) -> Vec<String> {
        events.iter().filter(|e| matches!(e, Event::Gain { .. })).map(render_event).collect()
    }

    #[test]
    fn gain_lines_name_their_source_card() {
        let gain = |card, source| Event::Gain { player: 0, card, to: Dest::Discard, source };
        assert_eq!(render_event(&gain(id::GOLD, id::HOARD)), "Player 1 gains Gold to their discard pile (Hoard)");
        assert_eq!(render_event(&gain(id::CURSE, id::WITCH)), "Player 1 gains Curse to their discard pile (Witch)");
        // A plain buy prints as before, with no parentheses.
        assert_eq!(render_event(&gain(id::SILVER, NO_SOURCE)), "Player 1 gains Silver to their discard pile");
    }

    /// Play a real Witch and Hoard through the engine and render the events it produced.
    #[test]
    fn real_games_log_witch_curses_hoard_golds_and_plain_buys() {
        use dominion_engine::state::{GameConfig, PlayerState};
        let cfg = GameConfig { num_players: 2, kingdom: vec![id::WITCH, id::HOARD], seed: 1, max_turns: 100, ..Default::default() };
        let mut g = GameState::new(&cfg);
        g.advance(&mut NoEvents);
        for p in 0..2 {
            g.players[p] = PlayerState::default();
        }
        g.players[0].hand = Counts::EMPTY;
        g.players[0].hand.add(id::WITCH, 1);
        g.players[0].hand.add(id::HOARD, 1);
        g.stack = Default::default();
        g.pending = Pending::None;
        g.turn = dominion_engine::state::TurnState::start(0, 1);
        let mut ev: Vec<Event> = Vec::new();
        let mut buf = ChoiceBuf::default();
        // Witch, then (Buy phase: Hoard auto-plays for $2) buy an Estate.
        let step = g.advance(&mut ev);
        assert!(matches!(step, Step::Decision(_)));
        g.apply(Choice::Card(id::WITCH), &mut ev).unwrap();
        g.advance(&mut ev);
        g.legal_choices(&mut buf);
        g.apply(Choice::Card(id::ESTATE), &mut ev).unwrap();
        g.advance(&mut ev);
        assert_eq!(
            log_of(&ev),
            vec![
                "Player 2 gains Curse to their discard pile (Witch)".to_string(),
                "Player 1 gains Estate to their discard pile".to_string(),
                "Player 1 gains Gold to their discard pile (Hoard)".to_string(),
            ]
        );
    }
}

#[cfg(test)]
mod run_game_tests {
    use super::*;

    /// Run Game on a Rising Sun kingdom with an Event and a Prophecy (Biding Time), both seats one
    /// strategy: plays to the end through the page's own path (log, per-turn record).
    #[test]
    fn run_game_with_a_prophecy_and_an_event() {
        let kingdom = dominion_engine::parse_kingdom(
            "Aristocrat, Artist, Kitsune, Root Cellar, Samurai, Artificer, Distant Lands, Dungeon, Duplicate, Page, Kintsugi, Biding Time",
        )
        .unwrap();
        let text = "name = \"T\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Samurai\"\nif = \"count(Samurai) < 3\"\n[[gain]]\ncard = \"Kitsune\"\nif = \"count(Kitsune) < 2\"\n[[gain]]\ncard = \"Distant Lands\"\n[[gain]]\ncard = \"Kintsugi\"\n[[gain]]\ncard = \"Silver\"\n";
        for seed in 1..=5u64 {
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                let cfg = GameConfig { num_players: 2, kingdom: kingdom.clone(), seed, max_turns: 200, ..Default::default() };
                let mut state = GameState::new(&cfg);
                state.pause_at_turn_start = true;
                app.state = state;
                app.record = GameRecord::new(&app.state);
                app.history.clear();
                app.log.clear();
                if app.strategies.len() == STRATEGY_SOURCES.len() {
                    let s = Strategy::parse(text).unwrap();
                    app.fast_strategies.push(rules_only_copy(&s));
                    app.strategies.push(s);
                }
                let id = FIRST_STRATEGY_SEAT + STRATEGY_SOURCES.len() as u32;
                app.seats[0] = id;
                app.seats[1] = id;
                run_bots_impl(&mut app).unwrap();
                assert_eq!(app.state.turn.phase, Phase::GameOver);
                let _ = app.record.json(&app.state);
                // Biding Time hands back the set-aside cards at the start of a turn: it is named, but is no Duration.
                // The cards come back on its line, named, not as a draw from the deck.
                let lines: Vec<&String> = app.log.iter().filter(|l| l.contains("takes back the cards Biding Time set aside")).collect();
                assert!(!lines.is_empty());
                assert!(lines.iter().all(|l| l.contains("set aside: ")), "{lines:?}");
                assert!(!app.log.iter().any(|l| l.contains("Player 2 gains Curse to their discard pile (Kitsune)")), "the gain sits under Kitsune: no source");
                assert!(!app.log.iter().any(|l| l.contains("Player 1 gains Province") || l.contains("Player 2 gains Province")), "a bought card's own gain is the buy");
                assert!(app.log.iter().any(|l| l.contains("buys Province")));
                assert!(!app.log.iter().any(|l| l.contains("Biding Time (duration)")));
            });
        }
    }
}
