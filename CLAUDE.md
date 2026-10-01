# Dominion simulator

Rust workspace (`cargo` is at `~/.cargo/bin/cargo`). Base Set 2nd edition only.

- `crates/engine` — rules engine. `GameState` is fixed-size `Copy`; **no heap allocation in the game loop**
  (no Vec/String/Box in engine hot paths). Deck = `deck_known` (known top cards, top = last) over
  `deck_unknown` (multiset). Randomness only when drawing from the unknown multiset.
  Card effects are resumable frames on `GameState::stack` (see `effects.rs` header comment).
  Drive with `advance()` → `Step::{Decision, Chance, GameOver}` and `apply(choice)`.
- `crates/sim` — strategies (scripted agents), batch runner (rayon), CLI.
- `crates/wasm` + `web/` — browser UI running the engine compiled to wasm32.
- `crates/evolve` — strategy discovery: evolves strategy files (genome = rule lists with template
  conditions), scored head-to-head with common random numbers; see `docs/strategy-search-plan.md`.
  Kept separate from the engine: the engine is for play testing and analysis, `evolve` generates
  and explores strategies. `crates/lab` — native server + Strategy Lab page driving `evolve`.

Adding cards: every Action card in `cards.rs` must be wrapped in `choice_free(..)`,
`has_choice(..)` or `order_sensitive(..)` (`OnPlay`). Choice-free = only +Cards/+Actions/+Buys/+$
for its player (attacks on others and order-independent bonuses like Merchant are fine), no
decision, nothing gained to / revealed from / put on its player's deck. Order-sensitive = a
choice-free +Actions card (Village, Lab) played before it can make it worse: it multiplies, depends
on hand size, looks at or puts cards on its player's deck, or could take an Action from their
hand (trash/discard/set aside/pass). Otherwise `has_choice`. A choice-free card whose bonus reads
empty piles (City) is listed in `cards::reads_empty_piles` instead, and only blocks the shortcuts
when something in hand could empty a pile first. Bots skip the play-order
search when every action in hand is choice-free and all can be played, and play choice-free
+Actions cards first when nothing in hand is order-sensitive (and no reshuffle can happen this
turn); `cards::tests` and `sim/tests/obvious_play.rs` check both against exact search.

Conventions: agents only see `PlayerView` (honest information). Multi-card picks use canonical
non-decreasing card-id order (`canonical_picks`). Goal of the project: discover strategies that
are *usable by humans* — prefer readable rule-based strategies over opaque ones.

Test: `cargo test --release`. Keep `cargo build` warning-free.
