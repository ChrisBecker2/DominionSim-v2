# Dominion simulator

Rust workspace (`cargo` is at `~/.cargo/bin/cargo`). Base Set 2nd edition only.

- `crates/engine` — rules engine. `GameState` is fixed-size `Copy`; **no heap allocation in the game loop**
  (no Vec/String/Box in engine hot paths). Deck = `deck_known` (known top cards, top = last) over
  `deck_unknown` (multiset). Randomness only when drawing from the unknown multiset.
  Card effects are resumable frames on `GameState::stack` (see `effects.rs` header comment).
  Drive with `advance()` → `Step::{Decision, Chance, GameOver}` and `apply(choice)`.
- `crates/sim` — strategies (scripted agents), batch runner (rayon), CLI.
- `crates/wasm` + `web/` — browser UI running the engine compiled to wasm32.

Conventions: agents only see `PlayerView` (honest information). Multi-card picks use canonical
non-decreasing card-id order (`canonical_picks`). Goal of the project: discover strategies that
are *usable by humans* — prefer readable rule-based strategies over opaque ones.

Test: `cargo test --release`. Keep `cargo build` warning-free.
