# Dominion simulator

Rust workspace (`cargo` is at `~/.cargo/bin/cargo`). Base Set, Intrigue, Seaside, Prosperity, Alchemy, Cornucopia & Guilds, Dark Ages, Adventures (Events, tokens, Reserve cards on the Tavern mat), Empires (Debt, split piles, Castles, Events, Landmarks), Change from Rising Sun and a few
promos (all 2nd edition where there is one).

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

Layout: one module per expansion, so reading a card means reading one place. `crates/engine/src/sets/{base,
intrigue,seaside,prosperity,alchemy,promo,cornucopia_guilds,dark_ages,adventures}.rs` each hold, for their cards: the ids (`ids`),
the `(id, CardDef)` table, and an `impl SetRules` with the play effect, modes table, Duration start-of-turn
effect, gain / buy / cleanup triggers, overpay, bonus VP, reaction data and masks (anything not overridden is a
no-op). Their card-specific frame handlers (`impl GameState`) sit below. `sets/mod.rs` builds the one `CARDS`
array (a const fn that fails the build on a duplicate or missing id) and dispatches by the card's set with a
static `match` (`with_set!`), so lookups and dispatch cost what the old single table and `match` did. Generic
machinery (frames, `Then` continuations, filters, the turn loop) stays in `effects.rs` / `engine.rs`. Bot
defaults are per set too: `crates/sim/src/defaults/{set}.rs` (`SetDefaults`), generic policy in `strategy.rs`.
Tests: `crates/engine/tests/{set}_cards.rs` per set (card tests, rulings, edge cases) and
`crates/sim/tests/{set}_defaults.rs`; interactions between sets in `cross_set.rs` and `reactions.rs`.
New expansion: follow `docs/adding-a-set.md`.

Adding cards: every Action card in its set's `CARDS` must be wrapped in `choice_free(..)`,
`has_choice(..)` or `order_sensitive(..)` (`OnPlay`). Choice-free = only +Cards/+Actions/+Buys/+$
for its player (attacks on others and order-independent bonuses like Merchant are fine), no
decision, nothing gained to / revealed from / put on its player's deck. Order-sensitive = a
choice-free +Actions card (Village, Lab) played before it can make it worse: it multiplies, depends
on hand size, looks at or puts cards on its player's deck, or could take an Action from their
hand (trash/discard/set aside/pass). Otherwise `has_choice`. A choice-free card whose bonus reads
empty piles (City) says so in its set's `SetRules::reads_empty_piles`, and only blocks the shortcuts
when something in hand could empty a pile first. Bots skip the play-order
search when every action in hand is choice-free and all can be played, and play choice-free
+Actions cards first when nothing in hand is order-sensitive (and no reshuffle can happen this
turn); each set's `choice_free_and_order_sensitive_cards` test (list the new card in it
deliberately), `cards::tests` and `sim/tests/obvious_play.rs` check both against exact search.
A new card: its id in the set's `ids` and its line in `CARDS` (ids are dense; the next free one), its
`play` arm, then whichever other `SetRules` methods it needs, then tests in the set's test file.

Dark Ages adds two generic mechanisms (details in `docs/dark-ages-plan.md`): *mixed piles* (the Knights and the Ruins:
one Supply pile of different cards, only the top card known; the members are in `in_supply`, the pile's size sits on the
top card's `supply` count, and the next top is drawn from the cards nobody has seen when the top is taken, so the state
holds no hidden order; `KNIGHTS` / `RUINS` are marker ids that name the pile in kingdoms and strategies) and the *trash
queue* (every trash goes through `GameState::trash_card`; "when you trash this" effects and Market Square are put on the
stack by `flush_trash_triggers` after the selection that trashed the cards has finished picking).

Conventions: agents only see `PlayerView` (honest information). Multi-card picks use canonical
non-decreasing card-id order (`canonical_picks`). Goal of the project: discover strategies that
are *usable by humans* — prefer readable rule-based strategies over opaque ones.

Test: `cargo test --release`. Keep `cargo build` warning-free.
