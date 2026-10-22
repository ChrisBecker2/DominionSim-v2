# Dominion simulator

Rust workspace (`cargo` is at `~/.cargo/bin/cargo`). Base Set, Intrigue, Seaside, Prosperity, Alchemy, Cornucopia & Guilds, Dark Ages, Adventures (Events, tokens, Reserve cards on the Tavern mat), Empires (Debt, split piles, Castles, Events, Landmarks), Rising Sun (Debt, Events, Prophecies with Sun tokens, Omen and Shadow cards), Renaissance (Villagers, Artifacts, Projects with cubes), Menagerie (Exile mat, the Horse pile, Ways), Allies (Favors, Allies, Liaisons, split piles that rotate), Plunder (Loot pile outside the Supply, Traits on piles, "next time" Durations) and a few
promos (Prince, Walled Village, Envoy, Captain, Sauna/Avanto, Governor, Dismantle, Church, Marchland, the Event Summon; all 2nd edition where there is one).

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
intrigue,seaside,prosperity,alchemy,promo,cornucopia_guilds,dark_ages,adventures,renaissance,menagerie}.rs` (and the other sets') each hold, for their cards: the ids (`ids`),
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

Renaissance (`docs/renaissance-plan.md`) keeps per-player Villagers / Project cubes / Cargo Ship and Research cards in `PlayerState::ren`, the game's Projects, Artifact holders and Sinister Plot tokens in `GameState::ren`, and hashes all of it as nothing while it is all zero (so non-Renaissance games hash and play exactly as before). Projects are bought like Events (`is_bought_pseudo`); start-of-turn Projects run as `PendingDuration` entries.

Plunder (`docs/plunder-plan.md`) keeps its state in `PluState` (game), `PluPlayer` (cards set aside under zone codes, "next time" Durations waiting) and `PluTurn`; the 30 Loot cards are a pile in `supply` lanes outside `in_supply`, drawn by the game RNG or as a `Step::Chance` of the pseudo player `LOOT_CHANCE`; a Trait sits on one pile (`PluState::trait_pile`). `PluState::flags` (fixed at set-up) and `live` (kept up to date) gate every hook, so other games pay one byte test. All of it hashes as nothing while zero.

Menagerie (`docs/menagerie-plan.md`) keeps each player's Exile mat (a `SmallMultiset`; its cards count as theirs for scoring and for every "cards you own" count), Invest's copies, the cards set aside by Delay / Reap / Way of the Turtle and Gatekeeper counts in `PlayerState::men`, and the Ways, Way of the Mouse's card and the live-effect hooks (`H_EXILE`, `H_INVEST`, `H_GATE`, `H_COST`) in `GameState::men`; it hashes as nothing while all zero. The Horse pile is outside the Supply (its count sits in the `supply` lane, not in `in_supply`). A game with a Way pays one byte test per Action played: the Way choice is a `PrePlay` frame (`DecisionKind::Way`) pushed only then, and bots decide it with a stated `[[way]]` rule or `SetDefaults::way_choice`.

Allies (`docs/allies-plan.md`) keeps each player's Favors, Coastal Haven's kept cards, Lich skips and lingering Attacks in `PlayerState::al`, the Ally, which effects are live (`flags` / `hook`),
the order of the six *split piles that rotate* (a `u32` of 2-bit indices per pile: all four cards of a pile are in the Supply, the order says which one is on top and `pile_open` is true for
that one only) and Family of Inventors' tokens in `GameState::al`, and the per-turn "this turn, when you gain" effects in `TurnState::al`; it hashes as nothing while all zero. The Allies are
pseudo-cards (never in a zone); spending Favors is an `Ask` / `YesNo` frame whose source is the Ally, decided by a stated `[[favor]]` rule or `SetDefaults::favor_use`. `GameState::_pad` is layout tuning for all-thread speed (see `docs/allies-plan.md` section 8). `Counts` has 512 lanes (the
`u16` block bitmap is full: the next set needs the `u32` step of `docs/card-id-capacity.md`).

Dark Ages adds two generic mechanisms (details in `docs/dark-ages-plan.md`): *mixed piles* (the Knights and the Ruins:
one Supply pile of different cards, only the top card known; the members are in `in_supply`, the pile's size sits on the
top card's `supply` count, and the next top is drawn from the cards nobody has seen when the top is taken, so the state
holds no hidden order; `KNIGHTS` / `RUINS` are marker ids that name the pile in kingdoms and strategies) and the *trash
queue* (every trash goes through `GameState::trash_card`; "when you trash this" effects and Market Square are put on the
stack by `flush_trash_triggers` after the selection that trashed the cards has finished picking).

The *stop-moving rule* (an effect moves a card it put somewhere only while that very copy is still there; details and the list of
dependent cards in `docs/stop-moving-rule.md`) is `crates/engine/src/track.rs`: zones are multisets, so the copies an effect is
tracking get a token (`Frame::tok`, a mark in the turn's small `TurnState::led` ledger; 0 = untracked plain play, `NIP` = played
without being in play). A card that moves itself says `leave_play_self_now` / `self_in_play_now` (never `in_play.remove(card)`),
a frame that moves a tracked copy goes through `zone_take` after `tok_moved`, an untracked removal from play is `remove_from_play`,
a push onto a deck top calls `cover_deck`. Plays that will be repeated (Throne-likes, Procession, Counterfeit, Daimyo, Citadel, Royal
Carriage, Scepter) and gains with reactions (or a guard: Replace, Summon, Continue, Rapid Expansion) make the marks and push a
`Release` frame under the effects; `ledger.n == 0` is the whole cost in an ordinary game. New cards that move themselves or the card
they gained must use these, and get a test with a same-named copy elsewhere (see `cross_set.rs`).

Conventions: agents only see `PlayerView` (honest information). Multi-card picks use canonical
non-decreasing card-id order (`canonical_picks`). Goal of the project: discover strategies that
are *usable by humans* — prefer readable rule-based strategies over opaque ones.

Test: `cargo test --release`. Keep `cargo build` warning-free.
