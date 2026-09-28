//! Unit tests of the HTTP routing (`dominion_lab::handle`) against a temp copy of `strategies/`,
//! with no sockets involved.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dominion_lab::{handle, AppState};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A fresh temp directory containing a copy of the real `strategies/*.toml` files (not the
/// `evolved/` subdirectory), so tests can write into `evolved/` without touching the real repo
/// and without interfering with each other.
fn strategies_fixture() -> PathBuf {
    let src = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies"));
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dst = std::env::temp_dir().join(format!("dominion-lab-test-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "toml") {
            std::fs::copy(&path, dst.join(path.file_name().unwrap())).unwrap();
        }
    }
    dst
}

fn call(state: &AppState, method: &str, path: &str, body: &[u8]) -> (u16, serde_json::Value) {
    let resp = handle(state, method, path, body);
    let v: serde_json::Value = serde_json::from_slice(&resp.body).unwrap_or(serde_json::Value::Null);
    (resp.status, v)
}

fn wait_until_not_running(state: &AppState, timeout: Duration) {
    let start = Instant::now();
    loop {
        let (_, st) = call(state, "GET", "/api/status", b"");
        if st["running"] == false {
            return;
        }
        if start.elapsed() > timeout {
            panic!("run did not stop within {timeout:?}: {st}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn cards_has_26_kingdom_and_7_basic() {
    let state = AppState::new(strategies_fixture());
    let (status, resp) = call(&state, "GET", "/api/cards", b"");
    assert_eq!(status, 200);
    assert_eq!(resp["kingdom"].as_array().unwrap().len(), 26);
    assert_eq!(resp["basic"].as_array().unwrap().len(), 7);
    assert!(resp["kingdom"].as_array().unwrap().iter().any(|c| c["name"] == "Witch"));
}

#[test]
fn strategies_lists_double_witch() {
    let state = AppState::new(strategies_fixture());
    let (status, resp) = call(&state, "GET", "/api/strategies", b"");
    assert_eq!(status, 200);
    let arr = resp.as_array().unwrap();
    assert!(arr.iter().any(|s| s["file"] == "double_witch.toml" && s["evolved"] == false && s["error"].is_null()));
}

#[test]
fn defaults_round_trip_into_a_valid_start() {
    let state = AppState::new(strategies_fixture());
    let (status, defaults) = call(&state, "GET", "/api/defaults", b"");
    assert_eq!(status, 200);
    assert_eq!(defaults["opponents"][0]["file"], "double_witch.toml");

    let (status, resp) = call(&state, "POST", "/api/start", defaults.to_string().as_bytes());
    assert_eq!(status, 200, "{resp}");
    assert_eq!(resp["ok"], true);

    call(&state, "POST", "/api/stop", b"");
    wait_until_not_running(&state, Duration::from_secs(60));
}

#[test]
fn tiny_run_produces_a_result() {
    let state = AppState::new(strategies_fixture());
    let req = serde_json::json!({
        "config": {
            "track": {"kind": "fixed", "kingdom": []},
            "forbidden": ["Witch"],
            "islands": 1,
            "island_size": 4,
            "generations": 1,
            "race_games": [50],
            "validate_every": 1,
            "validate_games": 100,
            "polish_games": 50
        },
        "opponents": [{"file": "double_witch.toml", "weight": 1.0}],
        "seed_files": ["big_money.toml"]
    });
    let (status, resp) = call(&state, "POST", "/api/start", req.to_string().as_bytes());
    assert_eq!(status, 200, "{resp}");

    wait_until_not_running(&state, Duration::from_secs(120));

    let (_, st) = call(&state, "GET", "/api/status", b"");
    assert!(st["error"].is_null(), "run errored: {st}");
    assert!(st["progress"]["result"].is_object(), "expected a result: {st}");
}

#[test]
fn save_writes_and_never_overwrites() {
    let dir = strategies_fixture();
    let state = AppState::new(dir.clone());
    let toml = "name = \"Test Save Strategy\"\ndescription = \"x\"\n\n[[gain]]\ncard = \"Province\"\n";
    let body = serde_json::json!({ "toml": toml, "name": "test save" });

    let (status1, resp1) = call(&state, "POST", "/api/save", body.to_string().as_bytes());
    assert_eq!(status1, 200, "{resp1}");
    assert!(dir.join("evolved").join("test_save.toml").exists());

    let (status2, resp2) = call(&state, "POST", "/api/save", body.to_string().as_bytes());
    assert_eq!(status2, 200, "{resp2}");
    assert_ne!(resp1["path"], resp2["path"]);
    assert!(dir.join("evolved").join("test_save_2.toml").exists());
}

#[test]
fn assess_big_money_ultimate_win_rate_in_range() {
    let dir = strategies_fixture();
    let state = AppState::new(dir.clone());
    let toml = std::fs::read_to_string(dir.join("big_money_ultimate.toml")).unwrap();
    let body = serde_json::json!({
        "toml": toml,
        "opponents": [{"file": "double_witch.toml", "weight": 1.0}],
        "config": {"track": {"kind": "fixed", "kingdom": []}},
        "games": 2000,
        "full": false
    });
    let (status, resp) = call(&state, "POST", "/api/assess", body.to_string().as_bytes());
    assert_eq!(status, 200, "{resp}");
    let wr = resp["win_rate"].as_f64().expect("win_rate");
    assert!((0.0..=1.0).contains(&wr), "win_rate out of range: {wr}");
    assert!(resp["games"].as_u64().unwrap() > 0);
}

#[test]
fn bad_json_gives_400() {
    let state = AppState::new(strategies_fixture());
    let (status, resp) = call(&state, "POST", "/api/start", b"not json at all");
    assert_eq!(status, 400);
    assert!(resp["error"].is_string());

    let (status, resp) = call(&state, "POST", "/api/save", b"{");
    assert_eq!(status, 400);
    assert!(resp["error"].is_string());

    let (status, resp) = call(&state, "POST", "/api/assess", b"{");
    assert_eq!(status, 400);
    assert!(resp["error"].is_string());
}

#[test]
fn second_start_while_running_gives_409() {
    let state = AppState::new(strategies_fixture());
    let req = serde_json::json!({
        "config": {
            "track": {"kind": "fixed", "kingdom": []},
            "islands": 1,
            "island_size": 4,
            "generations": 3,
            "race_games": [500],
            "validate_games": 200,
            "polish_games": 100
        },
        "opponents": [{"file": "double_witch.toml", "weight": 1.0}],
        "seed_files": []
    });
    let (status1, resp1) = call(&state, "POST", "/api/start", req.to_string().as_bytes());
    assert_eq!(status1, 200, "{resp1}");

    // running is set synchronously before /api/start returns, so this is deterministic.
    let (status2, resp2) = call(&state, "POST", "/api/start", req.to_string().as_bytes());
    assert_eq!(status2, 409, "{resp2}");
    assert!(resp2["error"].is_string());

    call(&state, "POST", "/api/stop", b"");
    wait_until_not_running(&state, Duration::from_secs(60));
}

#[test]
fn unknown_route_is_404() {
    let state = AppState::new(strategies_fixture());
    let (status, _) = call(&state, "GET", "/nope", b"");
    assert_eq!(status, 404);
}
