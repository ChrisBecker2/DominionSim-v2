# Plan: Intrigue (2nd edition)

**Scope.** Intrigue 2nd edition only: 26 kingdom cards, playable alone (it ships its own base
cards) or mixed with Base. The 1st-edition-only cards are **not** included: Coppersmith, Great Hall,
Saboteur, Scout, Secret Chamber and Tribute.

> Card text below is from memory: the wiki blocks automated fetches. Check each card against the
> wiki or the physical cards before its tests are written. The tests are the specification.

## 1. The cards and what each needs from the engine

"New" marks engine capabilities that don't exist yet (§2). **Order** is the `OnPlay` marking
required by `cards.rs`: **free** means choice-free, **choice** means has-choice (see CLAUDE.md).

| $ | Card | Types | Text (summary) | Needs | Order |
|---|---|---|---|---|---|
| 2 | Courtyard | Action | +3 Cards. Put a card from your hand onto your deck. | Select/Topdeck (exists) | choice |
| 2 | Lurker | Action | +1 Action. Choose: trash an Action from the Supply; or gain an Action from the trash. | **Mode**, **trash from Supply**, **gain from trash** | choice |
| 2 | Pawn | Action | Choose two different: +1 Card, +1 Action, +1 Buy, +$1. | **Mode (pick 2 distinct)** | choice |
| 3 | Masquerade | Action | +2 Cards. Each player with cards in hand passes one to the next such player on their left, at once. Then you may trash a card from your hand. | **Pass (simultaneous)**, Select/Trash | choice |
| 3 | Shanty Town | Action | +2 Actions. Reveal your hand; if no Action cards in it, +2 Cards. | reveal-hand condition | choice (order-dependent) |
| 3 | Steward | Action | Choose: +2 Cards; or +$2; or trash 2 cards from your hand. | **Mode** | choice |
| 3 | Swindler | Action–Attack | +$2. Each other player trashes their top deck card and gains a card with the same cost that *you* choose. | **Gain for another player, chosen by attacker**; reveal from an opponent's unknown deck (chance) | choice |
| 3 | Wishing Well | Action | +1 Card +1 Action. Name a card, reveal your top card; if you named it, put it into your hand. | **Name a card**, chance | choice |
| 4 | Baron | Action | +1 Buy. You may discard an Estate for +$4; if you don't, gain an Estate. | YesNo (exists) | choice |
| 4 | Bridge | Action | +1 Buy, +$1. This turn, cards cost $1 less (min 0). | **Dynamic cost** | free |
| 4 | Conspirator | Action | +$2. If you've played 3+ Actions this turn (counting this), +1 Card +1 Action. | `TurnState.played` count (exists) | choice (order-dependent) |
| 4 | Diplomat | Action–Reaction | +2 Cards. If you then have ≤5 cards in hand, +2 Actions. Reaction: when another player plays an Attack, you may reveal this from a hand of 5+ cards to draw 2 then discard 3. | **Reaction with a draw/discard effect** (Moat reveal exists) | choice (order-dependent) |
| 4 | Ironworks | Action | Gain a card costing up to $4. Action → +1 Action; Treasure → +$1; Victory → +1 Card (all that apply). | Gain (exists) + **then-bonus by gained type** | choice |
| 4 | Mill | Action–Victory (1 VP) | +1 Card +1 Action. You may discard 2 cards for +$2. | Select/Discard min 0 max 2 + bonus if exactly 2 | choice |
| 4 | Mining Village | Action | +1 Card +2 Actions. You may trash this for +$2. | **Trash from play** (YesNo) | choice |
| 4 | Secret Passage | Action | +2 Cards +1 Action. Put a card from your hand anywhere in your deck. | **Deck position choice** (§2.6) | choice |
| 5 | Courtier | Action | Reveal a card from your hand; for each type it has choose a different one: +1 Action, +1 Buy, +$3, gain a Gold. | Select/Reveal + **Mode (pick N distinct)** | choice |
| 5 | Duke | Victory | 1 VP per Duchy you have. | VP function (like Gardens) | — |
| 5 | Minion | Action–Attack | +1 Action. Choose: +$2; or discard your hand, +4 Cards, and each other player with 5+ cards in hand discards it and draws 4. | **Mode** | choice |
| 5 | Patrol | Action | +3 Cards. Reveal the top 4; Victory cards and Curses go to your hand; put the rest back in any order. | Reveal/order (exists, like Sentry) | choice |
| 5 | Replace | Action–Attack | Trash a card from your hand; gain one costing up to $2 more. Action/Treasure → onto your deck; Victory → each other player gains a Curse. | Remodel-like + **destination/attack by gained type** | choice |
| 5 | Torturer | Action–Attack | +3 Cards. Each other player chooses: discard 2 cards, or gain a Curse to hand. | **Mode for the victim**, gain to hand | free |
| 5 | Trading Post | Action | Trash 2 cards from your hand; if you did, gain a Silver to your hand. | Select/Trash + gain to hand | choice |
| 5 | Upgrade | Action | +1 Card +1 Action. Trash a card from your hand; gain one costing exactly $1 more. | **Gain filter: exact cost** | choice |
| 6 | Harem | Treasure–Victory | $2; 2 VP. | dual type | — |
| 6 | Nobles | Action–Victory | 2 VP. Choose: +3 Cards; or +2 Actions. | **Mode** | choice |

Only **Bridge** and **Torturer** are choice-free. Shanty Town, Conspirator and Diplomat have no
decision for their player, but their effect depends on play order, which breaks the "obvious
order" shortcut, so they're marked has-choice. `OnPlay::Choice` already covers "order matters".

**Victory kingdom piles** (Mill, Duke, Harem, Nobles) use 8 cards with 2 players and 12 otherwise,
like Estates, Duchies and Provinces.

## 2. Engine work, all generic (no card-specific decision kinds)

1. **Card ids and sets.**
   - `NUM_CARDS` goes from 33 to 59.
   - Add `CardSet { Base, Intrigue }` to `CardDef`, with `cards::in_set(..)`.
   - Kingdom helpers become per set: presets, `random_kingdom(sets)`, first-game-style recommended kingdoms.
   - **Keep `Counts` at 64 lanes** (`[u8; 64]`, 64-byte aligned). The whole Counts then fits one
     cache line or SIMD register however many cards we add, up to 64. The next expansion will need
     `[u8; 128]` or a per-game card remap, so decide on that then.
2. **Dynamic cost.**
   - Add `TurnState.cost_reduction` and a state-aware `GameState::cost(c)` (base − reduction,
     minimum 0). Throne Room + Bridge gives −$2.
   - Every cost use goes through it: buying, "gain up to", Remodel/Replace "+$2", Upgrade "exactly
     +$1", Swindler "same cost", Ironworks.
   - The same applies in sim/search: `PlayerView::cost`, the Expr `coins` comparisons, and the
     forced-gain fallback.
   - `cards::cost` stays for base cost only. Add a grep lint test so no engine gain or buy path
     calls it directly.
3. **Mode decisions:** `DecisionKind::Mode { card, options: ModeSet, pick: u8, distinct: bool }`.
   - Options are generic effect atoms from card data: +N Cards, +N Actions, +N Buys, +$N, trash N
     from hand, gain X, discard hand and draw 4 with the attack, and so on.
   - The chooser can be another player (the Torturer victim).
   - Labels come from the atoms, so the UI and log need no per-card text.
4. **Name a card:** `DecisionKind::Name`. Choices are the card ids that could be in the chooser's
   deck; naming a card you can't hit is dominated, and pruning it keeps the search small.
5. **Pass:** Masquerade's simultaneous pass. Collect every player's pick first, then move all the
   cards at once.
   - Honest views: a player learns only the card they receive.
   - Needs a new `Act::Pass` for `Select`.
6. **Deck position (Secret Passage).** "Anywhere" in a deck made of a known top over an unknown
   multiset. Offer: top, 2nd … (known length + 1), and **bottom**.
   - Bottom needs a `deck_known_bottom` stack.
   - Middle-of-unknown positions are left out: they're rarely right, and supporting them would
     need a positional unknown model. Document this as a deliberate simplification.
7. **Gain sources and destinations.**
   - Sources: gain from trash (Lurker); trash from Supply (Lurker).
   - Destinations: hand (Trading Post, Torturer's Curse), deck top (Replace).
   - "Gain for another player, chosen by the attacker" (Swindler): `Gain` gets a `for_player` field.
8. **Then-bonuses by gained or trashed type** (Ironworks, Replace): a `Then` continuation that
   inspects the last gained card.
9. **Trash from play** (Mining Village), done once. Throne Room's second play still gets +1
   Card +2 Actions, but can't trash it again.
10. **Reactions with effects** (Diplomat): generalize Moat's reveal-on-attack into a reaction
    frame. Condition: hand size ≥ 5. Effect: draw 2, then discard 3.
11. **Scoring:** Duke (per Duchy) and Mill/Harem/Nobles fixed VP. Mixed types go through the
    existing type flags.
12. **Text format:** parse and format the new cards, `cost_reduction` on the turn line, Masquerade
    passes and `deck bottom:`. Round-trip tests for every new zone and field.

## 3. Bots, strategies and search

- **New decisions need defaults** that rank below stated rules (project rule), plus optional
  TOML rules:
  - **Mode:** `[[mode]] card = "Steward" choose = "+2 Cards" if = "..."`, in priority order.
    Default: search the turn when the chooser is the player whose turn it is (Pawn/Steward/Nobles
    act like play decisions). Otherwise use a heuristic, e.g. Torturer: gain the Curse only if
    discarding 2 would cost more money than a Curse costs later.
  - **Name** (Wishing Well): the card most likely on top, from the view's deck counts. It's exact
    given honest information.
  - **Pass** (Masquerade): the lowest card in the trash priority, i.e. Curse, then Estate, then Copper.
  - **Swindler (attacker's gain for a victim):** the worst card at that cost (Curse, Estate, …).
    Default rule; overridable.
  - **Secret Passage:** keep the best card on top for next turn, or bury a Victory card at the bottom.
- **Expr language:** add `cost_reduction`, `hand_size`, `actions_played`, and `count_type(victory)`
  for Duke/Mill/Harem/Nobles decks.
- **Search:** enumerate Mode, Name, Pass and position choices canonically. Chance nodes cover
  Swindler (opponent's top card), Wishing Well and Patrol, sampled from honest views.
- **Performance:** run `bench/bench.py` before and after the `NUM_CARDS` change, and keep the
  no-allocation test green.

## 4. UI, Lab and CLI

- **Kingdom:** New game gets a card-set choice (Base / Intrigue / both). Recommended Intrigue kingdoms as presets.
- **Supply and card chips:**
  - The supply fits more cards; 10 kingdom piles plus basics still fit two columns.
  - Dual-type chips (Action–Victory, Treasure–Victory, Action–Reaction) are split-colored.
- **Decision UI:**
  - Mode options appear as buttons labeled from their effect atoms.
  - Name-a-card is a dropdown ordered by likelihood.
  - Secret Passage offers deck-position buttons.
- **Lab:** set choice for tracks A/B. The genome's gainable cards come from the chosen sets, and
  `[[mode]]` rules become part of the genome.
- **CLI:** `--kingdom` accepts set presets (`intrigue-first-game`, …) and `random:base+intrigue`.

## 5. Order of work (each step ends with all tests green)

1. **Foundation.**
   - `NUM_CARDS`, card sets, Counts at 64 lanes, dynamic cost (Bridge), and Harem/Duke/Mill/Nobles
     victory handling.
   - Rerun the benchmark.
   - Cards: Bridge, Harem, Duke.
2. **Simple cards on existing frames:** Courtyard, Shanty Town, Conspirator, Baron, Mining Village,
   Mill, Patrol, Trading Post, Upgrade, Ironworks, Replace, Torturer (victim mode after step 3).
3. **Mode decisions:** Pawn, Steward, Nobles, Minion, Courtier, Lurker, and the Torturer victim.
   Includes the strategy `[[mode]]` rules and the UI buttons.
4. **Hidden information:** Wishing Well (name), Swindler (attacker-chosen gain for a victim),
   Masquerade (simultaneous pass), Secret Passage (deck position / bottom).
5. **Reactions:** Diplomat.
6. **Bots, Lab and presets:**
   - Intrigue sample strategies (e.g. Torturer BM, Courtyard BM, Wishing Well engine, Masquerade BM).
   - Recommended kingdoms.
   - Lab set selection.
   - A search for counters to Double Witch using Intrigue cards too.

Card tests live next to the existing ones (`crates/engine/tests`), mostly ported from
`IntrigueCardsTests.cpp` (§5a). The Intrigue cases in the Base suite are ported too: Mine ↔ Harem,
and Torturer as an attack. Following the project's working mode, Sonnet agents implement steps 2–5
in parallel against this spec and the ported tests, and I review, test and merge.

## 5a. Porting `IntrigueCardsTests.cpp`

The C++ suite (3,572 lines, 25 `TEST_METHOD`s) targets Intrigue **1st** edition. It goes to
`crates/engine/tests/intrigue_port.rs` in the same style as `dominionsim_port.rs`: states as text,
scripted choices, and each test citing the C++ `TEST_METHOD` it came from. It's ported card by card
as each card lands (the tests are its spec), so every step in §5 ends with that card's ported
cases green.

| C++ test (line) | 2nd edition? | Port notes |
|---|---|---|
| TestCourtyard (15) | yes | Topdeck choice; empty-deck cases |
| TestPawn (110) | yes | Every pair of the four options, including illegal same-option picks |
| TestHarem (247), TestDuke (264) | yes | VP and treasure value |
| TestShantyTown (638) | yes | Hands with/without actions; Throne Room × Shanty Town; Shanty Town × 2 |
| TestNobles (730) | yes | Both modes; empty deck; an illegal mode is rejected |
| TestSteward (775) | yes | All three modes; trash with fewer than 2 cards in hand |
| TestBaron (872) | yes | With/without Estate, discard declined (gains an Estate), empty Estate pile |
| TestConspirator (928) | yes | Fewer than 3 vs 3+ actions played; Village chains |
| TestIronworks (1017) | yes | Bonus per gained type, incl. dual types |
| TestTorturer (1376) | yes | Victim's discard-or-Curse choice, Curse to hand, empty Curse pile |
| TestTradingPost (1566) | yes | 0/1/2 cards to trash; Silver to hand |
| TestUpgrade (1670) | yes | Exact +$1 gains; nothing at that cost |
| TestBridge (1912) | yes | Cost reduction, including the C++ "Bridge play order" TODO (the search handles it here) |
| TestSwindler (2130) | yes | Attacker picks the victim's gain at the same cost; empty deck; no card at that cost |
| TestWishingWell (2407) | yes | Named hit/miss; empty deck |
| TestMinion (2518) | yes | Both modes; victims with fewer than 5 cards unaffected |
| TestMiningVillage (2686) | yes | Trash for +$2; Throne Room × Mining Village trashes only once |
| TestMasquerade (2940) | yes, **text changed** | 2nd edition: only players *with cards in hand* pass, to the next such player. Rewrite any 1st-edition cases where an empty-handed player passes or receives |
| TestGreatHall, TestSecretChamber, TestScout, TestSaboteur, TestCoppersmith, TestTribute | **no** (1st edition only) | Not ported |

**2nd-edition cards with no C++ tests** (new tests written from the card text): Lurker,
Diplomat, Mill, Secret Passage, Courtier, Patrol and Replace.

**Cross-cutting tests:**
- Throne Room with every new action.
- Bridge × Throne Room, and Bridge × Upgrade / Ironworks / Swindler costs.
- Duke/Mill/Harem/Nobles scoring and pile sizes.
- Text-format round trips of every new state field.
- `OnPlay` marking: `cards::tests`, and the obvious-play property test with Bridge and Torturer
  added to its pool.

## 6. Decisions to confirm

1. **Secret Passage positions:** top / N-th of known / bottom only, or full "anywhere"?
   Recommend the former.
2. **Masquerade in search:** exact simultaneous passing, or a fixed default policy for opponents
   (faster)? Recommend exact.
3. **Base + Intrigue mixed kingdoms from the start**, or Intrigue-only first? Recommend mixed:
   same work, more useful.
