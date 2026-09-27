# Dominion Simulator — Plan

Scope: **Dominion Base Set, 2nd edition** kingdom (26 cards) + basic supply. The six cards removed in 2E (Adventurer, Chancellor, Feast, Spy, Thief, Woodcutter) are out of scope. 2+ players.

Goals, in priority order:
1. A rules engine that is correct, deterministic, fast, and does no heap allocation in the inner loop.
2. Massively parallel batch simulation (1000s–millions of games) to compare strategies.
3. A "perfect" turn analyzer: enumerate every legal line of play, handle hidden draws exactly with probabilities, and pick the best line.
4. An HTML front end: set up any game or mid-game state, step through it, and watch it as an animated table or a flat card list.
5. Later: strategy discovery (genetic algorithms, ML, "queries" like "which strategy reaches 30 VP fastest").

---

## 1. Technology recommendation

| Layer | Choice | Why |
|---|---|---|
| Engine + search + sim | **Rust** | Speed on par with C++, but data races are compile errors, which matters for heavy multithreading. `rayon` provides work-stealing parallelism, and `Copy` structs make state cloning free and allocation-free. It compiles to WebAssembly (next row). |
| Browser bindings | **Same Rust engine compiled to WASM** (`wasm-bindgen`) | The UI runs the *real* rules engine locally, so step-through, editing and undo have no server round-trips and the rules can't drift between two implementations. |
| Heavy jobs from UI | Small native server (`axum` + WebSocket) | The UI sends a "run 100k games" or "deep-analyze this position" job to the native multithreaded binary and streams progress back. |
| Front end | **TypeScript + Vite**, plain DOM/CSS for the list view, CSS transforms/FLIP (or PixiJS if needed) for the animated table | Flexible and framework-light. The engine's event log drives both views. |
| RNG | `rand_xoshiro` / PCG, one seeded stream per game | Fast, reproducible: `(base_seed, game_index)` → identical game. |

C++ would work equally well for raw speed. Rust is recommended because of WASM reuse and safe threading.

---

## 2. Repository layout

```
dominion/
  crates/
    engine/     # cards, state, rules, decisions, events. No deps, no allocation in hot paths.
    search/     # turn enumerator, expectimax, ISMCTS, evaluators, transposition table
    agents/     # scripted (Big Money, Smithy-BM, Double Witch...), rule-DSL agents, search agents
    sim/        # CLI batch runner (rayon), stats, tournaments, queries
    server/     # axum WebSocket server for the UI's heavy jobs
    wasm/       # wasm-bindgen bindings for the UI
  web/          # TypeScript front end
  assets/cards/ # card images (fetched by script; see §9)
  scenarios/    # JSON game setups / mid-game positions
  strategies/   # strategy definitions (TOML/JSON)
```

---

## 3. Engine design

### 3.1 Card IDs and definitions
- `CardId = u8`. There are 33 cards: Copper, Silver, Gold, Estate, Duchy, Province, Curse + 26 kingdom cards.
- A static `CARDS: [CardDef; 33]` table holds cost, type bitflags (Action/Treasure/Victory/Curse/Attack/Reaction), and the "vanilla" bonuses (+cards, +actions, +buys, +coins).
- Non-vanilla effects are resolved by a `match card_id` in the effect resolver. There is **no `dyn` dispatch and no boxed closures**.

### 3.2 Card zones as counts
Most zones are **unordered**, so they are stored as fixed count arrays, `[u8; 33]` (33 bytes, `Copy`):

| Zone | Representation | Notes |
|---|---|---|
| Hand | counts | Order never matters. Duplicate lines of play collapse for free (see §4). |
| Discard | counts + `top: Option<CardId>` | Only the top card is visible; order matters for nothing in the base set. |
| **Deck** | **`known_top: small stack` + `unknown: counts`** | This is the key representation (§3.3). |
| In play | small ordered array (inline, cap ~64) | Order matters for Throne Room display, Merchant, and cleanup. |
| Set aside | small inline array | Library. |
| Trash | counts (shared) | |
| Supply | `[u8; 33]` pile counts (shared) | |

A player's state is ~200 bytes, and a 4-player game state is < 1 KB. The whole `GameState` is `Copy`: cloning is a `memcpy` and never allocates. For pathological decks (e.g. a 100+ card Workshop/Gardens deck), counts still work because a `u8` count per card ID is enough. Only `in_play` needs a capacity, so give it a generous cap plus an overflow assert.

### 3.3 Deck = known top + unknown multiset (how hidden information is handled)
This representation directly models "a perfect player knows *what* is left, but not the *order*":

- **Shuffle** is deterministic bookkeeping: `deck.unknown += discard; discard = 0`. No randomness happens here.
- **Drawing** from `known_top` is deterministic. Drawing from `unknown` is a **chance event**: card *c* comes with probability `unknown[c] / total`.
- A deck with 1 card left gives that card with 100% probability automatically.
- Cards that become known (Sentry looks at them, Bureaucrat/Artisan/Harbinger put them on top, Vassal/Library reveal them) are pushed onto `known_top`.
- **Drawing k cards at once** follows a multivariate hypergeometric distribution. The search enumerates *distinct resulting hand multisets* with exact probabilities, not orderings. Example: drawing 5 from {7 Copper, 3 Estate} gives only 4 outcomes, not 30,240 orderings. This is the main reason exact enumeration is tractable.
- **In sampled (fast simulation) mode**, the same code samples the draw with the RNG. That is statistically identical to shuffling up front, so there is only one representation.
- **UI "true order" mode**: when a user wants a specific deck order in a scenario, they fill `known_top` completely. The engine then behaves deterministically.

### 3.4 Decisions: the engine is a resumable state machine, not a callback caller
The engine **never calls into a player** mid-resolution. It advances until it needs input, then returns:

```rust
enum Step {
    Decision(DecisionRequest),   // who must choose, and what kind of choice
    Chance(ChanceRequest),       // a draw from an unknown deck (search mode)
    GameOver(Outcome),
}
```

`DecisionRequest` kinds are a small closed enum: `PlayAction`, `PlayTreasures`, `Buy`, `DiscardDownTo(n)` (Militia), `ChooseCardsFromHand{min,max,filter}` (Cellar, Chapel, Poacher...), `GainUpTo(cost, filter)` (Workshop, Artisan, Remodel, Mine), `TopdeckFromHand`, `SentryArrange`, `YesNo` (Library skip, Vassal play, Moat reveal, Moneylender), `ThroneTarget`...

Pending effects (Throne Room nesting, attacks resolving around the table, Library draw loops) live on a **fixed-capacity effect stack inside `GameState`**. That keeps the state `Copy`, which makes these operations trivial:
- **Steppable** (UI: one decision at a time)
- **Snapshot/undo** (just keep old states)
- **Searchable** (clone the state, try each choice)
- **Agent-agnostic** (scripted, search, human, ML: all see the same `DecisionRequest`)

`legal_choices(&state, &req, &mut out: ChoiceBuf)` writes into a caller-owned fixed buffer, so it doesn't allocate.

### 3.5 Events
Every state change emits an `Event` (`Draw`, `Shuffle`, `Play`, `Gain{to}`, `Trash`, `Discard`, `Reveal`, `Topdeck`, `AttackBlocked`, `TurnStart`...) into an optional sink.
- The simulation passes a no-op sink, which compiles away.
- The UI passes a recorder. The **animated view animates events** and the **list view renders state**.

### 3.6 Setup rules (configurable, defaulting to official)
- Victory piles: 8 each for 2 players, 12 for 3+. Provinces: 12 for 3–4, 15 for 5, 18 for 6, extrapolated beyond that.
- Curses: 10 × (players − 1). Copper: 60 − 7×players (auto-extend for large N).
- Game end: Provinces empty, or 3 supply piles empty (4 piles for 5+ players).
- Tiebreak: fewer turns wins, then shared victory.
- A **turn cap** (e.g. 80) prevents non-terminating sims.

### 3.7 Rules edge cases to test explicitly
Throne Room on Throne Room; Throne Room on a drawer when the deck runs out; Moat vs. each attack; Militia with ≤3 cards; Witch when Curses run out, dealt in turn order; Bureaucrat with no Victory in hand; Library setting aside actions and reshuffling mid-draw; Vassal playing a discarded Action; Merchant (only the first Silver, per Merchant); Poacher counting empty piles; Sentry trash/discard/reorder; Harbinger from an empty discard; Cellar drawing after a reshuffle; Gardens scoring; Mine/Artisan gaining to hand; Remodel/Mine on empty piles; draws with an empty deck *and* empty discard.

Also test invariants: **card conservation** (a property test: every card is always in exactly one zone), determinism (same seed gives the same game), and snapshot equality.

---

## 4. Turn-level deep iteration (the "perfect" enumerator)

### 4.1 The tree
Within a turn, the search builds an **expectimax tree**:
- **Decision nodes** (the player picks: which action, which targets, stop) take the **max** over children.
- **Chance nodes** (draws from `deck.unknown`) take the **probability-weighted sum**, branching over distinct hand multisets.
- The **leaf** is the end of the action phase. It is scored by the buy-phase optimizer plus an evaluator (§5).

With Village + Smithy in hand the tree naturally contains:
`Village → (draw chance) → Smithy → (draw 3 chance) → …`,
`Smithy → (draw 3) → [Village unplayable: 0 actions] → end`,
`Village → … → stop without Smithy`, `stop now`.
The engine's legality check discovers that Smithy-first strands the Village. No special logic is needed.

### 4.2 Keeping it tractable
- **Multiset hands** mean that playing "Village #1" vs. "Village #2" is the same child, so it is never duplicated.
- **Transposition table** keyed by a Zobrist hash of the (turn-relevant) state merges different orderings that reach the same state, e.g. Market→Lab vs. Lab→Market when no draws intervene.
- **Chance outcome grouping**: when drawing, only the distinction between action cards and the total treasure value may matter to the rest of the turn. An optional abstraction can merge outcomes that are equivalent for the remainder of the turn. This must be opt-in, because it is exact only under stated conditions.
- **Node budget with graceful fallback**: if a subtree exceeds the budget (long Lab/Library chains, big Cellar choices), switch from exact enumeration to sampling K draws at that chance node (sparse sampling) and report the result as an estimate with a confidence value.
- **Safe pruning only by default**: e.g. "play all non-terminal +cards/+actions cantrips before terminals" is *usually* right, but not always (with Library, Sentry, or when drawing dead matters). Heuristic prunes are flags, and the default is exact.
- **Allocation-free**: nodes live in a per-thread **bump arena** that is reset after each decision. Choice buffers and hash tables are pre-sized and reused per thread.

### 4.3 Buy phase
Coins and buys at the leaf are usually small. The search enumerates buy *multisets* within budget (with pile limits) and scores each with the evaluator, or delegates to the agent's buy policy. Treasures are always auto-played in the base set, except for rare Merchant/Mine interactions, which the engine handles.

---

## 5. Making the final decision under uncertainty (recommendation)

The core question is: *the order of the deck is unknown, so what is the "best" play?* The recommendation is a **layered evaluator** in which each layer is exact where it's cheap and approximate where it isn't:

**Layer 0: exact within the current turn.** Run the expectimax in §4 over all action sequences and draw outcomes. Draws are exact probabilities from the known remaining multiset, so a 1-card deck is 100%. This alone captures "don't play Smithy before Village" and "Cellar away Estates because the deck has 2 Golds left".

**Layer 1: exact next-hand distribution (the reshuffle-aware part).** After cleanup, the next 5-card hand is also a chance node over `deck.unknown` (plus the discard if a reshuffle happens mid-draw). This is where the perfect tracking pays off:
- It knows when the deck has 3 cards left, so the next hand = those 3 + 2 from a fresh shuffle *that includes whatever is bought now*.
- It values "trigger the reshuffle now vs. later", Cellar/Harbinger/Sentry manipulation, and the classic "don't buy Gold right before a reshuffle misses it" timing.

Score each leaf by the **expected value of next turn's hand** (coins, action potential) plus the long-term value of the card(s) gained.

**Layer 2: long-horizon value V(state).** Beyond next turn, exact enumeration explodes, so use one of these (all pluggable behind an `Evaluator` trait):
1. **Heuristic**: money density, VP, Province pile pressure, and "turns to end", with hand-tuned weights. This is the starting point.
2. **Rollouts**: from the leaf, play out the game several times with a fast scripted policy (Monte Carlo). For multiplayer hidden information, use **determinization** (sample opponents' hidden hands/decks consistent with what's been seen), i.e. **ISMCTS**.
3. **Learned value function**: a small NN or linear model trained on millions of simulated positions (§8).

**Objective: maximize win probability, not expected VP.** Early game, E[money] and E[VP] are good proxies. Late game (Provinces ≤ ~4, or a pile-out near), switch the objective to **P(win)**. Example: when behind, a risky line with a 30% chance of the Province you need beats a safe Duchy. The evaluator should expose both, with a phase-based switch.

**Opponent hidden information.** Gains are public, so every player knows each opponent's full card *composition*. What isn't known is the split between hand, deck, and discard; the discard top and revealed cards are visible. Model each opponent as "known composition, hidden partition" and sample it for determinization. Provide an **omniscient** flag too, which is useful for upper-bound studies ("how much is perfect information worth?").

**Pragmatic default for batch sims:** scripted bots for opponents, and Layer 0 + Layer 1 + heuristic V for the search agent. That is fast enough for 10k-game tournaments. Enable ISMCTS only for deep single-position analysis from the UI.

---

## 6. Agents

```rust
trait Agent: Send {
    fn decide(&mut self, view: &PlayerView, req: &DecisionRequest, choices: &[Choice], scratch: &mut Scratch) -> Choice;
}
```
- `PlayerView` gives *honest* information (own deck multiset, opponents' composition, visible zones) or omniscient information, based on a flag.
- **Scripted agents** are defined as data (TOML/JSON) with a small priority-rule DSL, similar to Geronimoo's simulator and Dominiate:
  ```toml
  name = "Double Witch"
  [[buy]]  card = "Province";  if = "total_money >= 16"
  [[buy]]  card = "Witch";     if = "count(Witch) < 2"
  [[buy]]  card = "Gold"
  [[buy]]  card = "Duchy";     if = "supply(Province) <= 4"
  [[buy]]  card = "Silver"
  [play]   order = ["Village", "Witch", "Smithy"]   # action priority
  ```
  Conditions compile once to a tiny bytecode/AST that is evaluated without allocation. The same format is the **genome** for the genetic algorithm.
- **Seed strategies**: Big Money (Ultimate: Province @8, Gold @6–7, Silver @3–5, Duchy/Estate endgame rules), Smithy-BM, Double Witch, Militia-BM, Moneylender-BM, Village/Smithy engine, Chapel/Lab.
- **Search agent** = base policy (for the buys it doesn't search / rollouts) + depth settings + evaluator.
- **Human agent** (UI) receives the `DecisionRequest` and renders the choices.
- Sub-decisions (Militia discards, Chapel trashing, Sentry) get sensible defaults in scripted agents, e.g. "discard lowest value" and "trash Curse > Estate > Copper while money ≥ X". These are overridable.

---

## 7. Batch simulation and parallelism

- **Embarrassingly parallel over games.** `(0..n_games).into_par_iter()` with rayon. Each game is independent, and its seed is derived from `(base_seed, game_idx)` via SplitMix, so the results are reproducible regardless of thread count.
- **Per-thread scratch** (`thread_local!` or rayon `map_init`) holds the arena, choice buffers, and agent scratch. All are allocated once per thread and reused for every game.
- **No shared mutable state in the loop.** Each thread folds results into a local accumulator (win/tie/loss, VP histograms, turn counts, card-buy frequencies). Accumulators are reduced once at the end. Accumulators are cache-line padded to avoid false sharing.
- **Variance reduction:**
  - rotate seats (every matchup is played from every seat order);
  - **common random numbers**: the same seed stream is used for both sides of a strategy comparison;
  - report Wilson confidence intervals;
  - optional **SPRT** early stopping ("A beats B at 95%, stop").
- **Zero-allocation check**: a counting `#[global_allocator]` in tests/benchmarks asserts **0 allocations per game** after warm-up. `criterion` benchmarks track games/sec and search nodes/sec, with a regression check in CI.
- **Within-search parallelism** is used for a single deep analysis only: root-parallel ISMCTS, or splitting top-level choices across threads.
- **Target**: measure first. A simple bot game of about 20 turns should reach 10⁴–10⁵ games/sec per core. At that rate a 1M-game tournament on a 16-core machine takes seconds.

CLI examples:
```
dominion-sim match   --p strategies/bm.toml --p strategies/double_witch.toml --games 100000 --kingdom base-first-game
dominion-sim league  --dir strategies/ --games 20000          # round-robin table
dominion-sim analyze --scenario scenarios/village_smithy.json --depth turn+1 --explain
dominion-sim query   "fastest to 30 VP" --dir strategies/
```

---

## 8. Strategy discovery (later phases)

- **Queries**: define a metric over game traces (first turn reaching X VP, P(4 Provinces by turn 17), average turns to game end) and rank strategies by it. Traces come from the event stream, reduced in-thread.
- **Genetic algorithm**: the genome is the TOML rule list (buy priorities, thresholds, play order). Fitness is win rate vs. a benchmark pool using common random numbers. Parallelism is across both individuals and games. Include elitism, a hall of fame, and co-evolution to avoid overfitting a single opponent.
- **ML**:
  1. Train a value function V(state features) → P(win) on positions from millions of sim games. This plugs into Layer 2 (§5).
  2. Later, a policy network plus self-play (AlphaZero-style ISMCTS).
  3. The feature extraction (counts vectors, supply, turn) lives in Rust so training data generation is fast; export to Parquet/NPZ for PyTorch; run inference in Rust with a small MLP (hand-written or `candle`/`ort`).

---

## 9. Front end

**Views** (all driven by the same WASM engine state + event log):
1. **Animated tabletop**: supply in the center and each player's area around the table (deck, hand, in play, discard). Cards fly between zones using FLIP animations keyed on events. Speed control, pause, step. Opponent hands are face-down unless the view is omniscient.
2. **Flat list**: a column per player with zone counts, plus the supply, VP, coins/actions/buys, and turn number. This is dense and good for analysis.
3. **Analysis panel**: for the current decision, every legal option with its expected value, P(win) estimate, and an expandable subtree ("Village → draws {Gold,Copper} 23% → Smithy …"). It also shows the next-hand distribution and the deck-remaining composition.
4. **Batch runner**: pick strategies and kingdom, send to the native server, stream progress, and show win-rate tables and charts (VP over turns, buy frequencies).

**Scenario editor:**
- Choose the kingdom (10 cards, with presets such as "First Game") and player count.
- Per player: edit the **hand, deck (known top order + unknown multiset), discard, in play**, actions/buys/coins, and turn/phase.
- Set supply counts and the trash.
- Assign an agent per seat (human / scripted file / search).
- Validate (card conservation vs. supply is optional, because users may want impossible "what-if" states).
- Save/load as JSON in `scenarios/`.

**Stepping**: step one decision, step one turn, run to the end of the game, **undo/redo** (a snapshot stack of `Copy` states), and fork from here (branch the timeline).

**Card images**: a script (`tools/fetch_images`) downloads images from the Dominion Strategy wiki into `assets/cards/` under their card IDs. Note that the card art is copyrighted (Rio Grande Games / Donald X. Vaccarino). Keep the images local or in `.gitignore`, not in a public repo, and fall back to generated text cards (name, cost, type color).

---

## 10. Milestones

| # | Deliverable | Done when |
|---|---|---|
| M1 | Engine core: state, zones, treasures/victory, buy, cleanup, shuffle-as-multiset, game end. The Big Money bot runs | A deterministic BM vs. BM game runs; conservation property test passes; 0 allocs/game |
| M2 | All 26 kingdom cards + effect stack + all decision kinds | Per-card rules tests incl. §3.7 edge cases |
| M3 | Rayon batch runner + stats + seat rotation + CRN; strategy DSL; seed strategies | `match` / `league` CLI; BM vs. Smithy-BM results match known community numbers (a sanity check: Smithy-BM should beat BM by a wide margin) |
| M4 | WASM bindings + list view + scenario editor + step/undo | Can build the Village+Smithy scenario in the UI and step through it |
| M5 | Turn enumerator (expectimax, multiset chance nodes, TT, arena) + analysis panel | The UI shows EVs for all lines of the Village+Smithy hand; Smithy-first flagged as stranding the Village |
| M6 | Layer 1 next-hand evaluation + heuristic V; search agent in batch sims | Search agent beats its own base policy |
| M7 | Animated tabletop view | |
| M8 | Native server for heavy jobs from UI | |
| M9 | ISMCTS / determinization; P(win) objective | |
| M10 | Queries, GA, ML value function | |

---

## 11. Open questions / decisions for you

1. **Rust vs. C++**: Rust is recommended (WASM reuse, safe threading). Is that OK?
2. **Honest vs. omniscient default** for the search agent's view of opponents. The recommendation is honest by default, with omniscient as a flag.
3. **Max player count** for UI layout. The engine handles N; the tabletop view is probably designed for 2–6.
4. **Tie handling in stats**: count a tie as ½ win (recommended) or separately.
5. **Rules pedantry level**: e.g. whether players may choose *not* to play treasures (it matters rarely in the base set, only for Merchant/Mine). The recommendation is to auto-play all treasures by default, with an option to disable.
