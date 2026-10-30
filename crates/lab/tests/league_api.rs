//! Kingdom league through the Lab API: recommended kingdoms and a short league run.
use std::path::Path;
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
            "league": { "final_games": 100, "final_bots": "fast", "insight_games": 500 },
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
            // The kingdom findings the page shows: sentences, and the champion against each other strategy of the ranking.
            assert!(!l["findings"]["findings"].as_array().unwrap().is_empty(), "{}", l["findings_error"]);
            assert_eq!(l["findings"]["field"].as_array().unwrap().len(), l["crosstable"]["names"].as_array().unwrap().len() - 1);
            assert_eq!(st["progress"]["status"], "done");
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn run_to_end(state: &AppState, req: serde_json::Value) -> serde_json::Value {
    let (status, resp) = call(state, "POST", "/api/start", req.to_string().as_bytes());
    assert_eq!(status, 200, "{resp}");
    let start = Instant::now();
    loop {
        let (_, st) = call(state, "GET", "/api/status", b"");
        if st["running"] == false {
            assert!(st["error"].is_null(), "{st}");
            return st["progress"].clone();
        }
        assert!(start.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn names(v: &serde_json::Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

/// The result carries the kingdom the run played on, as set up: Events and the named Prophecy included, Platinum and Colony added
/// for a Prosperity card. The game page starts on exactly this list.
#[test]
fn a_league_result_carries_its_kingdom_with_events_prophecy_and_colonies() {
    let state = AppState::new(std::env::temp_dir());
    let k = ["Village", "Smithy", "Market", "Militia", "Moat", "Council Room", "Poet", "Bank", "Ball", "Biding Time"];
    let progress = run_to_end(
        &state,
        serde_json::json!({
            "config": {
                "track": { "kind": "fixed", "kingdom": k },
                "forbidden": [],
                "league": { "final_games": 100, "final_bots": "fast", "exploit_rounds": 0, "insight_games": 0 },
                "islands": 1, "island_size": 6, "generations": 1,
                "race_games": [100], "validate_games": 200, "polish_games": 0
            },
            "opponents": [], "seed_files": []
        }),
    );
    let got = names(&progress["kingdom"]);
    for c in k {
        assert!(got.contains(&c.to_string()), "{c} missing from {got:?}");
    }
    assert!(got.contains(&"Platinum".to_string()) && got.contains(&"Colony".to_string()), "{got:?}");
    // Every name parses back as a kingdom list for a new game.
    dominion_engine::parse_kingdom(&got.join(", ")).expect("the game page can start on it");
}

/// Opponent mode: a fixed kingdom is reported; with no kingdom named (every card) there is none to hand over.
#[test]
fn an_opponent_run_reports_its_fixed_kingdom_only_when_it_has_one() {
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies"));
    let state = AppState::new(dir.to_path_buf());
    let req = |kingdom: serde_json::Value| {
        serde_json::json!({
            "config": {
                "track": { "kind": "fixed", "kingdom": kingdom },
                "forbidden": ["Witch"],
                "islands": 1, "island_size": 4, "generations": 1,
                "race_games": [50], "validate_every": 1, "validate_games": 100, "polish_games": 0
            },
            "opponents": [{ "file": "double_witch.toml", "weight": 1.0 }], "seed_files": []
        })
    };
    let fixed = run_to_end(&state, req(serde_json::json!(["Village", "Smithy", "Market", "Militia", "Moat", "Witch", "Cellar", "Chapel", "Festival", "Laboratory"])));
    let got = names(&fixed["kingdom"]);
    assert!(got.contains(&"Smithy".to_string()) && got.contains(&"Witch".to_string()), "{got:?}");
    let all = run_to_end(&state, req(serde_json::json!([])));
    assert!(names(&all["kingdom"]).is_empty());
}
