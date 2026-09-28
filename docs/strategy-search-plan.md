# Plan: discovering human-readable strategies

**Goal.** Automatically find a strategy file (`strategies/*.toml`) that beats **Double Witch** without
relying on Witch itself, using other Base Set cards. The output must be a short, readable rule list
that a person could follow at the table. It must not be a neural network that makes decisions.

**Principle.** Search *in the space of strategy files*. Whatever the search finds is readable by
construction, because it is written in the same TOML/condition language the bots already run. Machine
learning is used to *guide* that search or to *explain* a stronger player as rules. It never becomes
the final decision-maker.

---

## 1. Define the target precisely

| Question | Proposed default |
|---|---|
| Opponent | `double_witch.toml`, unchanged, plus a small hall-of-fame pool later (§4.4) |
| Players | 2 |
| Kingdom | Witch + 9 other Base cards. Two tracks: **(A) fixed**, the full 25 non-Witch cards available (strongest possible answer, less realistic); **(B) random**, fresh 10-card kingdoms containing Witch, where the strategy must play well across them (general, human-usable) |
| Our cards | Any Base card except Witch (a "different set of cards"); Moat, Chapel, Library, Militia and similar counters are allowed |
| Success | Win rate > 50% with the 95% interval above 50% over ≥ 200,000 games on **held-out seeds**, with ≤ ~10 gain rules and ≤ ~5 play/trash rules |

Known answers the search should rediscover, as a sanity check: Moat-heavy Big Money, and Chapel/Militia
variants. If it can't find those, the search is broken.

## 2. The genome: a strategy file

A candidate is exactly what `Strategy::parse` accepts:

- **`[[gain]]`**: ordered entries of `card` plus an optional condition. Conditions come from a small
  grammar of templates, so they stay readable:
  - `count(C) < N`
  - `provinces_left <= N`
  - `coins >= N`
  - `count_type(action) < N`
  - `money >= N`
  - `vp_lead >= N`
  - `turn >= N`
  - `and` of at most 2 of the above
- **`[[play]]`**: ordered action preferences, optional conditions.
- **`[[trash]]`**: what to trash (defaults: Curse, Estate, Copper).
- **Numbers:** `keep_treasure`, `win_this_turn`.

**Mutation operators:**
- insert, delete or swap a rule;
- change a rule's card;
- nudge a threshold by ±1;
- add or remove a condition;
- flip `<`/`<=`;
- move a rule up or down.

**Crossover:** splice two parents' gain lists at a cut point, and pick each of the play and trash lists
from one parent.

**Seeding the population:** the 11 shipped strategies, plus random mutants of them, plus
"Big Money + X" / "Big Money + X + Y" templates for every card X and pair (X, Y).

## 3. Fitness: fast, fair, and hard to game

- **Evaluation:** head-to-head vs Double Witch with seats rotated. Use **common random numbers**
  (every candidate in a generation plays the same seeds), so differences come from the strategy, not
  luck. Draw fresh seeds each generation so nothing overfits a seed set.
- **Racing:** evaluate everyone at 2,000 games, keep the top half, add 10,000 more, keep the top
  quarter, and so on up to 100,000 games (successive halving). Most compute goes to the contenders.
- **Parsimony:** fitness = win rate − λ·(number of rules + conditions), with λ small (≈0.2 percentage
  points per rule). Ties go to the shorter file.
- **Robustness (track B):** fitness is the average over K random Witch kingdoms per generation (K≈8),
  same kingdoms for all candidates.

**Throughput budget.** Measured native speed is 0.2–0.7 M games/s for money-ish strategies on 32 cores.
- One generation of 256 candidates × 2,000 games ≈ 0.5 M games ≈ 1–3 s.
- That's 1,000 generations in under an hour.

Engine strategies are slower (≈ 1–4 k games/s) because the bot searches its action order. For the
inner loop, add a `play_search = false` evaluation mode that uses the rule order only, and re-validate
the finalists with full search.

## 4. Search algorithms (in order of priority)

### 4.1 Genetic algorithm over strategy files (start here)
- **Setup:** an island model with 8 islands × 64 candidates, and migration every 20 generations.
  Tournament selection, elitism (top 4 carried over unchanged), mutation-heavy (80% mutate / 20%
  crossover).
- **Stopping:** after N generations with no significant improvement.

### 4.2 MAP-Elites for diversity
Keep the best strategy in each cell of a small grid of human-meaningful descriptors:
- main kingdom card used,
- money vs engine (actions / total cards),
- whether it trashes.

This yields a *catalog* of different readable counters ("best Moat strategy", "best Chapel strategy",
…), not one winner, which is more useful to a human and less prone to a single local optimum.

### 4.3 Numeric tuning with CMA-ES
Once the GA has a good *structure*, freeze the rule list and tune its thresholds (the N's) with
CMA-ES on the same fitness. It's cheap and gives the final few percent.

### 4.4 Co-evolution / hall of fame
Guard against strategies that only exploit Double Witch's specific quirks. Periodically add champions
to an opponent pool (Double Witch, Big Money Ultimate, previous champions) and require winning against
the pool, weighted toward Double Witch.

## 5. Where ML and the GPU help, without becoming the decision-maker

1. **Surrogate model (guides the GA).** Train a small network (GPU) mapping a genome's features to its
   measured win rate, using every evaluation the GA has already run. Use it to pre-screen thousands of
   mutants per generation and only simulate the promising ones (a Bayesian-optimisation-style loop).
   The final strategies are still verified by simulation.
2. **Train strong, then distill to rules** (the most promising ML route):
   1. Train a neural policy by self-play or RL against Double Witch (GPU). It's allowed to be opaque.
      Its inputs are the same variables the condition language has (`count(C)`, coins,
      `provinces_left`, `vp_lead`, …).
   2. Log its decisions over millions of positions.
   3. Fit a **depth-limited decision list / tree** to those decisions (buys first, then play and trash).
   4. Translate the tree into `[[gain]]` / `[[play]]` rules with conditions, and simplify it: drop any
      rule whose removal doesn't change win rate beyond its confidence interval.
   5. Validate the extracted TOML by simulation. The network is thrown away; the file is the product.
3. **Explanations for humans.** For a found strategy, report which rules matter by ablation (turn each
   off, measure the drop), and add these as comments in the TOML.

The simulator itself stays on the CPU (branchy game logic doesn't suit a GPU). The GPU is for the
surrogate and for policy training in 5.2.

## 6. What needs building

| Piece | Where | Notes |
|---|---|---|
| In-memory strategy evaluation | `crates/sim` | `evaluate(&Strategy, &opponents, kingdoms, seeds) -> WinRate + CI`, no files; reuse the rayon batch runner |
| Genome ⇄ TOML | `crates/sim` (new `genome.rs`) | Generate, mutate, cross over, and pretty-print canonical, readable TOML |
| `dominion-evolve` binary | `crates/sim/src/bin` | GA / MAP-Elites / CMA-ES; writes champions to `strategies/evolved/` with a stats header comment; checkpoints to resume |
| `play_search = false` mode | `crates/sim` | Fast inner-loop evaluation |
| Validation run | `bench/` | Held-out seeds, 200k+ games vs Double Witch and the pool; added to the benchmark report |
| Python bridge (phase 2) | `pyo3` bindings or JSON over stdin | For the surrogate and RL/distillation in PyTorch on the GPU |
| Readability pass | `genome.rs` | Remove dead rules, merge redundant ones, sort, comment each rule with its measured impact |

## 7. Milestones

1. **Evaluator + genome + GA (track A).** Rediscover a known counter, e.g. Moat/Big Money beating
   Double Witch, from the shipped strategies as seeds. Output: `strategies/evolved/*.toml`.
2. **Racing, parsimony, fresh-seed validation, and the readability pass.** Output: a report of each
   champion's win rate, its CI, and rule ablations.
3. **Track B:** random Witch kingdoms, a robust strategy, and a MAP-Elites catalog.
4. **CMA-ES threshold tuning, and the hall-of-fame opponent pool.**
5. **Phase 2 ML:** a GPU surrogate to speed up the GA, then RL + decision-tree distillation into rules.

## 8. Risks and mitigations

- **Overfitting to seeds or one kingdom:** fresh seeds per generation, held-out validation, and
  multi-kingdom fitness (track B).
- **Exploiting bot quirks:** the hall-of-fame pool; review champions by hand in the UI with Analyze
  decision.
- **Unreadable bloat:** parsimony pressure, rule caps, and the dead-rule removal pass.
- **Condition language too weak** to express a counter (e.g. "buy Moat only if the opponent has
  Witches"): extend the language with a few opponent-aware variables (`opp_count(C)`, `curses_left`)
  when the GA plateaus. They're still readable.
- **Slow engine evaluations:** rule-order evaluation in the inner loop, full search only for finalists.
