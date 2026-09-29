//! Strategy Lab: a native HTTP server (+ single-page UI) around `dominion-evolve`.
//!
//! The HTTP routing is a pure function, [`handle`], so it can be unit-tested without sockets.
//! [`accept_loop`] is the only piece that touches a real socket (via `tiny_http`), used by the
//! `dominion-lab` binary; [`build_config`] / [`save_strategy`] are also used directly by its
//! headless `run` subcommand.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use serde_json::json;

use dominion_engine::cards::{self, CardId, ACTION, ATTACK, CURSE_T, REACTION, TREASURE, VICTORY};
use dominion_evolve::{Control, EvolveConfig, OpponentSpec, Progress};
use dominion_sim::Strategy;

// ---------------------------------------------------------------------------------------------
// HTTP plumbing
// ---------------------------------------------------------------------------------------------

/// The result of routing one request: status code, content type and body bytes. Deliberately
/// socket-free so `handle` can be unit-tested directly.
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    fn json<T: Serialize>(status: u16, v: &T) -> Response {
        Response { status, content_type: "application/json", body: serde_json::to_vec(v).unwrap_or_default() }
    }
    fn html(status: u16, s: &str) -> Response {
        Response { status, content_type: "text/html; charset=utf-8", body: s.as_bytes().to_vec() }
    }
    fn err(status: u16, msg: impl Into<String>) -> Response {
        Response::json(status, &json!({ "error": msg.into() }))
    }
}

/// One evolution run's shared state, polled by `/api/status` and updated by the background
/// thread `/api/start` spawns. Only one run at a time; `running` guards that.
#[derive(Default)]
struct RunState {
    running: bool,
    progress: Option<Progress>,
    error: Option<String>,
    request: Option<LabRequest>,
    control: Option<Arc<Control>>,
}

/// Server state. Cheap to clone (an `Arc<Mutex<..>>` inside), so `accept_loop` clones it once
/// per accepted connection instead of wrapping it in an outer `Arc`.
#[derive(Clone)]
pub struct AppState {
    pub strategies_dir: PathBuf,
    run: Arc<Mutex<RunState>>,
}

impl AppState {
    pub fn new(strategies_dir: PathBuf) -> AppState {
        AppState { strategies_dir, run: Arc::new(Mutex::new(RunState::default())) }
    }
}

/// Route one request. Pure aside from reading the strategies directory and the shared run state.
pub fn handle(state: &AppState, method: &str, path: &str, body: &[u8]) -> Response {
    let path = path.split('?').next().unwrap_or(path);
    match (method, path) {
        ("GET", "/") => Response::html(200, include_str!("../web/lab.html")),
        ("GET", "/api/cards") => Response::json(200, &cards_response()),
        ("GET", "/api/strategies") => Response::json(200, &list_strategies(&state.strategies_dir)),
        ("GET", "/api/defaults") => Response::json(200, &default_request(&state.strategies_dir)),
        ("POST", "/api/start") => handle_start(state, body),
        ("POST", "/api/stop") => handle_stop(state),
        ("GET", "/api/status") => handle_status(state),
        ("POST", "/api/save") => handle_save(state, body),
        ("POST", "/api/assess") => handle_assess(state, body),
        ("GET", "/game") => game_page(state),
        _ => Response::err(404, "not found"),
    }
}

/// The game UI (`web/dist/index.html`, next to the strategies directory), read from disk so the
/// Lab can open a strategy in it (`/game#strategy=<base64 TOML>`).
fn game_page(state: &AppState) -> Response {
    let root = state.strategies_dir.canonicalize().unwrap_or_else(|_| state.strategies_dir.clone());
    let path = root.parent().map(|p| p.join("web").join("dist").join("index.html"));
    match path.as_ref().map(std::fs::read_to_string) {
        Some(Ok(html)) => Response::html(200, &html),
        _ => Response::html(
            404,
            "<p>The game page isn't built yet (web/dist/index.html). Run <code>./build-web.ps1</code> in the repository, then reload.</p>",
        ),
    }
}

/// Accept connections forever, serving each on its own thread (so a long `/api/assess` never
/// blocks `/api/status` polling). Used by the `dominion-lab serve` binary. Takes an `Arc` so a
/// caller (e.g. a test) can keep a handle to call `server.unblock()` for a graceful shutdown.
pub fn accept_loop(state: AppState, server: Arc<tiny_http::Server>) {
    for request in server.incoming_requests() {
        let state = state.clone();
        thread::spawn(move || serve_one(&state, request));
    }
}

fn serve_one(state: &AppState, mut request: tiny_http::Request) {
    let method = match request.method() {
        tiny_http::Method::Get => "GET",
        tiny_http::Method::Post => "POST",
        _ => "OTHER",
    };
    let url = request.url().to_string();
    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);
    let resp = handle(state, method, &url, &body);
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], resp.content_type.as_bytes()).unwrap();
    let http_resp = tiny_http::Response::from_data(resp.body).with_status_code(resp.status).with_header(header);
    let _ = request.respond(http_resp);
}

// ---------------------------------------------------------------------------------------------
// Requests shared between /api/start, /api/status (echo) and `dominion-lab run`
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpponentFile {
    pub file: String,
    #[serde(default = "one")]
    pub weight: f64,
}
fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LabRequest {
    #[serde(default)]
    pub config: EvolveConfig,
    #[serde(default)]
    pub opponents: Vec<OpponentFile>,
    #[serde(default)]
    pub seed_files: Vec<String>,
}

#[derive(Deserialize)]
struct SaveRequest {
    toml: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct AssessRequest {
    toml: String,
    #[serde(default)]
    opponents: Vec<OpponentFile>,
    #[serde(default)]
    config: EvolveConfig,
    #[serde(default = "default_games")]
    games: u64,
    #[serde(default)]
    full: bool,
}
fn default_games() -> u64 {
    20_000
}

#[derive(Serialize)]
struct StatusResponse {
    running: bool,
    progress: Option<Progress>,
    error: Option<String>,
    request: Option<LabRequest>,
}

// ---------------------------------------------------------------------------------------------
// /api/cards
// ---------------------------------------------------------------------------------------------

#[derive(Serialize)]
struct CardInfo {
    name: String,
    cost: u8,
    types: Vec<&'static str>,
    /// "Base" or "Intrigue".
    set: &'static str,
}

fn card_info(c: CardId) -> CardInfo {
    let mut types = Vec::new();
    for (flag, label) in [(ACTION, "Action"), (TREASURE, "Treasure"), (VICTORY, "Victory"), (CURSE_T, "Curse"), (ATTACK, "Attack"), (REACTION, "Reaction")] {
        if cards::is(c, flag) {
            types.push(label);
        }
    }
    let set = match cards::set_of(c) {
        cards::CardSet::Base => "Base",
        cards::CardSet::Intrigue => "Intrigue",
    };
    CardInfo { name: cards::name(c).to_string(), cost: cards::cost(c), types, set }
}

fn cards_response() -> serde_json::Value {
    let kingdom: Vec<CardInfo> = cards::kingdom_cards().map(card_info).collect();
    let basic: Vec<CardInfo> = (0..cards::FIRST_KINGDOM).map(card_info).collect();
    json!({ "kingdom": kingdom, "basic": basic })
}

// ---------------------------------------------------------------------------------------------
// /api/strategies
// ---------------------------------------------------------------------------------------------

#[derive(Serialize)]
pub struct StrategyInfo {
    file: String,
    name: String,
    description: String,
    toml: String,
    evolved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn list_strategies(dir: &Path) -> Vec<StrategyInfo> {
    let mut out = Vec::new();
    collect_strategy_dir(dir, "", false, &mut out);
    collect_strategy_dir(&dir.join("evolved"), "evolved/", true, &mut out);
    out
}

fn collect_strategy_dir(dir: &Path, prefix: &str, evolved: bool, out: &mut Vec<StrategyInfo>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "toml")).collect();
    paths.sort();
    for p in paths {
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        let file = format!("{prefix}{fname}");
        let stem = fname.trim_end_matches(".toml").to_string();
        match fs::read_to_string(&p) {
            Ok(text) => match Strategy::parse(&text) {
                Ok(s) => out.push(StrategyInfo { file, name: s.name.clone(), description: s.description.clone(), toml: text, evolved, error: None }),
                Err(e) => out.push(StrategyInfo { file, name: stem, description: String::new(), toml: text, evolved, error: Some(e) }),
            },
            Err(e) => out.push(StrategyInfo { file, name: stem, description: String::new(), toml: String::new(), evolved, error: Some(e.to_string()) }),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// /api/defaults
// ---------------------------------------------------------------------------------------------

pub fn default_request(dir: &Path) -> LabRequest {
    let cfg = EvolveConfig::default(); // Fixed{kingdom: []} (all cards), forbidden = ["Witch"]
    let Ok(entries) = fs::read_dir(dir) else {
        return LabRequest { config: cfg, opponents: vec![OpponentFile { file: "double_witch.toml".into(), weight: 1.0 }], seed_files: Vec::new() };
    };
    let mut seed_files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();
    seed_files.sort();
    LabRequest { config: cfg, opponents: vec![OpponentFile { file: "double_witch.toml".into(), weight: 1.0 }], seed_files }
}

// ---------------------------------------------------------------------------------------------
// File resolution shared by /api/start, /api/assess and `dominion-lab run`
// ---------------------------------------------------------------------------------------------

fn safe_join(dir: &Path, file: &str) -> Result<PathBuf, String> {
    if file.is_empty() || file.contains("..") || file.starts_with('/') || file.starts_with('\\') || file.contains(':') {
        return Err(format!("invalid strategy file path {file:?}"));
    }
    Ok(dir.join(file))
}

fn read_strategy_file(dir: &Path, file: &str) -> Result<String, String> {
    let path = safe_join(dir, file)?;
    fs::read_to_string(&path).map_err(|e| format!("reading {file}: {e}"))
}

fn display_name(file: &str, text: &str) -> String {
    match Strategy::parse(text) {
        Ok(s) => s.name,
        Err(_) => file.trim_end_matches(".toml").to_string(),
    }
}

fn resolve_opponents(dir: &Path, opps: &[OpponentFile]) -> Result<Vec<OpponentSpec>, String> {
    opps.iter()
        .map(|o| {
            let text = read_strategy_file(dir, &o.file)?;
            let name = display_name(&o.file, &text);
            Ok(OpponentSpec { name, toml: text, weight: o.weight })
        })
        .collect()
}

fn resolve_seeds(dir: &Path, files: &[String]) -> Result<Vec<String>, String> {
    files.iter().map(|f| read_strategy_file(dir, f)).collect()
}

/// Resolve a [`LabRequest`] into a runnable [`EvolveConfig`] by reading its opponent/seed files
/// from `dir`. Used by `/api/start` and by `dominion-lab run --config`.
pub fn build_config(dir: &Path, req: &LabRequest) -> Result<EvolveConfig, String> {
    let mut cfg = req.config.clone();
    cfg.opponents = resolve_opponents(dir, &req.opponents)?;
    cfg.seeds = resolve_seeds(dir, &req.seed_files)?;
    Ok(cfg)
}

// ---------------------------------------------------------------------------------------------
// /api/start, /api/stop, /api/status
// ---------------------------------------------------------------------------------------------

fn handle_start(state: &AppState, body: &[u8]) -> Response {
    let req: LabRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return Response::err(400, format!("bad request: {e}")),
    };
    match start_run(state, req) {
        Ok(()) => Response::json(200, &json!({ "ok": true })),
        Err((status, msg)) => Response::err(status, msg),
    }
}

/// Claim the "one run at a time" slot, resolve files into a config, and spawn the background
/// thread. Claiming happens before file resolution (which can fail) so two concurrent starts
/// can't both pass the "not running" check.
fn start_run(state: &AppState, req: LabRequest) -> Result<(), (u16, String)> {
    {
        let mut g = state.run.lock().unwrap();
        if g.running {
            return Err((409, "a run is already in progress".into()));
        }
        g.running = true;
    }
    let cfg = match build_config(&state.strategies_dir, &req) {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut g = state.run.lock().unwrap();
            g.running = false;
            return Err((400, e));
        }
    };
    let control = Arc::new(Control::default());
    {
        let mut g = state.run.lock().unwrap();
        g.error = None;
        g.request = Some(req);
        g.control = Some(control.clone());
    }
    let run_state = state.run.clone();
    thread::spawn(move || {
        let result = dominion_evolve::run(&cfg, &control, &mut |p: &Progress| {
            if let Ok(mut g) = run_state.lock() {
                g.progress = Some(p.clone());
            }
        });
        if let Ok(mut g) = run_state.lock() {
            match result {
                Ok(p) => {
                    g.progress = Some(p);
                    g.error = None;
                }
                Err(e) => g.error = Some(e),
            }
            g.running = false;
        }
    });
    Ok(())
}

fn handle_stop(state: &AppState) -> Response {
    if let Some(c) = state.run.lock().unwrap().control.clone() {
        c.stop.store(true, Ordering::Relaxed);
    }
    Response::json(200, &json!({ "ok": true }))
}

fn handle_status(state: &AppState) -> Response {
    let g = state.run.lock().unwrap();
    Response::json(200, &StatusResponse { running: g.running, progress: g.progress.clone(), error: g.error.clone(), request: g.request.clone() })
}

// ---------------------------------------------------------------------------------------------
// /api/save
// ---------------------------------------------------------------------------------------------

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_us = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_us = false;
        } else if !last_us && !out.is_empty() {
            out.push('_');
            last_us = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "strategy".into()
    } else {
        out
    }
}

/// Validate `toml` as a strategy and write it into `<dir>/evolved/`, never overwriting an
/// existing file (appends `_2`, `_3`, ...). Returns the path written, relative to `dir`'s parent
/// when `dir` is named "strategies" (matching the shipped layout), else relative to `dir` itself.
pub fn save_strategy(dir: &Path, toml_src: &str, name: Option<&str>) -> Result<String, String> {
    let strategy = Strategy::parse(toml_src)?;
    let base = name.filter(|s| !s.trim().is_empty()).map(str::to_string).unwrap_or(strategy.name);
    let slug = slugify(&base);
    let evolved_dir = dir.join("evolved");
    fs::create_dir_all(&evolved_dir).map_err(|e| e.to_string())?;
    let mut candidate = slug.clone();
    let mut n = 1;
    loop {
        let path = evolved_dir.join(format!("{candidate}.toml"));
        if !path.exists() {
            fs::write(&path, toml_src).map_err(|e| e.to_string())?;
            let label = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "strategies".into());
            return Ok(format!("{label}/evolved/{candidate}.toml"));
        }
        n += 1;
        candidate = format!("{slug}_{n}");
    }
}

fn handle_save(state: &AppState, body: &[u8]) -> Response {
    let req: SaveRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return Response::err(400, format!("bad request: {e}")),
    };
    match save_strategy(&state.strategies_dir, &req.toml, req.name.as_deref()) {
        Ok(path) => Response::json(200, &json!({ "path": path })),
        Err(e) => Response::err(400, e),
    }
}

// ---------------------------------------------------------------------------------------------
// /api/assess
// ---------------------------------------------------------------------------------------------

fn handle_assess(state: &AppState, body: &[u8]) -> Response {
    let req: AssessRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return Response::err(400, format!("bad request: {e}")),
    };
    let mut cfg = req.config;
    cfg.opponents = match resolve_opponents(&state.strategies_dir, &req.opponents) {
        Ok(v) => v,
        Err(e) => return Response::err(400, e),
    };
    match dominion_evolve::assess(&cfg, &req.toml, req.games, req.full) {
        Ok(entry) => Response::json(200, &entry),
        Err(e) => Response::err(400, e),
    }
}
