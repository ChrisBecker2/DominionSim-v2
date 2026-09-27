//! Asserts that simulating games allocates zero times per game, once agents/strategies/game
//! config are set up. Uses a counting `#[global_allocator]` so any stray `Vec`/`Box`/`String`
//! anywhere in the call path (engine or `dominion-sim`) shows up as a test failure.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use dominion_engine::{play_game, Agent, GameConfig, GameState, NoEvents};
use dominion_sim::{Strategy, StrategyAgent};

struct CountingAlloc;

static ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// Play one game with pre-built agents against a pre-built (only `seed` mutated) `GameConfig`.
/// Every argument is already allocated; this function must not allocate.
fn run_one(a: &mut StrategyAgent, b: &mut StrategyAgent, cfg: &mut GameConfig, seed: u64) {
    cfg.seed = seed;
    let mut state = GameState::new(cfg);
    let mut agents: [&mut dyn Agent; 2] = [a, b];
    let _ = play_game(&mut state, &mut agents, &mut NoEvents);
}

#[test]
fn zero_allocations_per_game_after_warmup() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let bm = Strategy::load(&repo_root.join("strategies/big_money.toml")).expect("load big_money.toml");
    let smithy = Strategy::load(&repo_root.join("strategies/smithy_bm.toml")).expect("load smithy_bm.toml");

    let mut agent_a = StrategyAgent::new(&bm);
    let mut agent_b = StrategyAgent::new(&smithy);
    let mut cfg = GameConfig { num_players: 2, kingdom: dominion_engine::cards::FIRST_GAME.to_vec(), seed: 0, max_turns: 200 };

    // Warm-up: absorb any one-time lazy initialization (allocator arenas, TLS, etc.) before
    // we start counting.
    for seed in 0..50u64 {
        run_one(&mut agent_a, &mut agent_b, &mut cfg, seed);
    }

    const N: u64 = 200;
    let before = ALLOC_CALLS.load(Ordering::Relaxed);
    for seed in 1000..1000 + N {
        run_one(&mut agent_a, &mut agent_b, &mut cfg, seed);
    }
    let after = ALLOC_CALLS.load(Ordering::Relaxed);

    assert_eq!(after, before, "expected zero allocations across {N} games after warm-up, saw {} alloc/realloc calls ({:.2}/game)", after - before, (after - before) as f64 / N as f64);
}
