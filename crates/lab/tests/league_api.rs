//! Kingdom league through the Lab API: recommended kingdoms and a short league run.
use std::time::{Duration, Instant};

use dominion_lab::{handle, AppState};

fn call(state: &AppState, method: &str, path: &str, body: &[u8]) -> (u16, serde_json::Value) {
    let resp = handle(state, method, path, body);
    (resp.status, serde_json::from_slice(&resp.body).unwrap_or(serde_json::Value::Null))
}

#[test]
fn recommended_kingdoms_are_listed_and_only_use_implemented_cards() {
    let state = AppState::new(std::env::temp_dir());
    let (status, v) = call(&state, "GET", "/api/recommended", b"");
    assert_eq!(status, 200);
    let list = v.as_array().unwrap();
    assert!(list.len() > 50, "{}", list.len());
    let bm = list.iter().find(|k| k["name"] == "Biggest Money").expect("Biggest Money");
    assert_eq!(bm["cards"].as_array().unwrap().len(), 10);
    assert_eq!(bm["colony"], true);
    for k in list {
        for c in k["cards"].as_array().unwrap() {
            let id = dominion_engine::cards::by_name(c.as_str().unwrap()).expect("known card");
            assert!(dominion_engine::cards::is_ready(id));
        }
    }
}

#[test]
fn a_league_run_through_the_api_reports_niches_and_a_crosstable() {
    let state = AppState::new(std::env::temp_dir());
    let k = ["Village", "Smithy", "Laboratory", "Market", "Chapel", "Militia", "Moat", "Council Room", "Moneylender", "Throne Room"];
    let req = serde_json::json!({
        "config": {
            "track": { "kind": "fixed", "kingdom": k },
            "forbidden": [],
            "league": { "final_games": 100, "final_full": false },
            "islands": 2, "island_size": 8, "generations": 2,
            "race_games": [100, 200], "validate_games": 200, "polish_games": 0
        },
        "opponents": [], "seed_files": []
    });
    let (status, resp) = call(&state, "POST", "/api/start", req.to_string().as_bytes());
    assert_eq!(status, 200, "{resp}");
    let start = Instant::now();
    loop {
        let (_, st) = call(&state, "GET", "/api/status", b"");
        if st["running"] == false {
            assert!(st["error"].is_null(), "{st}");
            let l = &st["progress"]["league"];
            assert!(l["niches"].as_array().unwrap().len() >= 1);
            assert!(l["crosstable"]["names"].as_array().unwrap().len() >= 2);
            assert_eq!(st["progress"]["status"], "done");
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(100));
    }
}
