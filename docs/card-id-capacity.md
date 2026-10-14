# Card-id capacity: how many ids, what they cost, and whether to compact them per game

Written with the Empires work (`docs/empires-plan.md`). Numbers are from this repository at that point (HEAD plus the uncommitted Empires + Change work).

## 1. Counting card-like ids

A "zone id" is a card that can sit in a hand, deck, discard pile, play area, the trash or a supply pile: it needs a lane in `Counts`. A "pseudo id" (Event,
Landmark, and later Prophecy, Way, Project, ...) has a name, a cost and an id so strategies, text and the Lab work unchanged, but is never in a zone.

| Set | Zone ids | of which kingdom piles | Pseudo ids |
|---|---|---|---|
| Base (2E) incl. basics, Curse | 33 | 26 | 0 |
| Intrigue | 26 | 26 | 0 |
| Seaside | 27 | 27 | 0 |
| Prosperity (incl. Platinum, Colony) | 27 | 25 | 0 |
| Alchemy (incl. Potion) | 12 | 11 | 0 |
| Promo (Sauna/Avanto and the other 2E promos) | 6 | 5 | 0 |
| Cornucopia & Guilds (incl. the Joust Rewards) | 32 | 26 | 0 |
| Dark Ages (Knights 10, Ruins 5, Shelters 3, Spoils, Madman, Mercenary) | 57 | 35 | 0 |
| Adventures (Traveller lines, Reward cards) | 38 | 30 | 20 Events |
| Empires (split halves 10, Castles 8) | 37 | 24 | 13 Events + 21 Landmarks |
| Rising Sun, today (Change only) | 1 | 1 | 0 |
| **Now** | **296** | | **54** (ids 296 to 349) |
| Rising Sun, rest: 24 more kingdom cards, 10 Events, 15 Prophecies | +24 | | +10 Events, +15 Prophecies |
| **Through Rising Sun** | **320** | | **79** (43 Events, 21 Landmarks, 15 Prophecies) |

So through Rising Sun there are 399 card-like ids, 320 of which need a `Counts` lane. (Rising Sun has no split piles and no non-Supply piles in the
wiki's card gallery; the Omen and Shadow types and the Sun tokens are card-level state, not ids.)

## 2. What the id space costs today

* `Counts` is one byte per zone id: `LANES` is now 416 (was 320 for Rising Sun's 320 zone ids; 416 gives Nocturne and Hinterlands room). Hands, decks, discards, play areas and the trash are `Counts`
  (four per player plus two per game); `SmallMultiset` is used for the rare zones (mats, Archive).
* `SupplyMask` is four `u128` words (512 bits), widened with Rising Sun; the pseudo ids are not in it.
* `GameState` grew 16.8 KB to 18.4 KB with Empires (the 32 extra lanes are about 1 KB, since there are about 28 `Counts` in a state;
  the rest is the Empires state: pile tokens, Archive, Debt, the Landmark record). `tests/state_size.rs` pins it.
* Speed (`dominion-sim match big_money_ultimate double_witch --games 200000`, best of three): 1.82M games/s before, 1.76M after (-3%), widening and Empires
  together. The widening to 320 lanes alone is inside the noise of that figure. The wasm build runs on its 8 MiB stack.

Rising Sun update: widening to 416 lanes and four mask words, measured alone, costs about 0% on `dominion-sim match` Big Money Ultimate vs Double Witch (200000 games,
interleaved best of 5, about 1.9M games/s); the whole Rising Sun work measured about -3%, inside the +-4% run-to-run layout noise (a bisect showed code layout, not
the sizes). `GameState` is about 22.5 KB, a 2-player copy about 10.6 KB.

## 3. What compaction would be

Per-game compact ids: a game uses about 17 basic and set-up piles, 10 kingdom piles and the non-Supply piles of the cards in the kingdom (a Knights game 10,
a Traveller game 9, Castles 8, Ruins 5): at most about 60, usually 25 to 35. A `Counts` indexed by a per-game dense id would be 32, 64 or 96 lanes.

Estimated gain, from the numbers above: `Counts` becomes about a fifth of its size, `GameState` about 11 to 12 KB instead of 18.4 KB, and the bots' copy-bound
paths (search nodes, rollouts) get faster by roughly 10 to 15% (the Adventures experience: +6 KB made whole games 18% slower).

Cost, which is the problem:

* Every `Counts` access takes a card id (`hand.get(id::SILVER)`, the SIMD walk, `canonical_picks`). A per-game map must be consulted at every one (a `LANE_OF`
  table in the `GameState`, or a thread-local): an extra dependent load on the hottest paths, and `Counts` methods no longer standalone (they need the map), which
  touches hundreds of call sites in the engine, `sim`, `search`, `evolve`, `wasm` and the tests.
* A copy of the state mid-search still has to map back to card ids for display, text, `PlayerView`, strategies (`count(Gold)`) and hashing: the map must travel
  with the state, and states from different kingdoms cannot be merged or compared.
* Determinization, `parse_state` and the Lab's fixed-kingdom evaluation all build states from names; each would need the map built first.
* The ids of the cards the engine mentions as constants (basics, Curse, Potion, Estate for Shelters, Silver and Gold from many effects) would have to keep
  fixed lanes, the rest dynamic: a two-tier scheme that is harder to reason about than either extreme.

## 4. Recommendation

Do not compact now. Through Rising Sun the plain widening fits exactly (`LANES` 320, `SupplyMask` 384), its measured cost is small, and the sets that come
after Rising Sun (Nocturne alone would add about 33 kingdom cards, 12 Heirloom-like and set-up cards, 12 Boons and 12 Hexes and a handful of States and
Artifacts) are the point at which it stops being enough: past 384 zone ids `SupplyMask` needs a fourth word and `Counts` 416 or 448 lanes.

Revisit when any of these happens, in this order of cost:

1. The next widening makes the benchmark slower by more than about 5% (do the widening and the speed pass first, as with Adventures).
2. `GameState` passes about 24 KB (`state_size.rs` fails): first look for more `SmallMultiset` zones; they are cheap.
3. A set arrives that needs more than about 450 zone ids. Then design the two-tier scheme (fixed lanes for basics and the cards named in engine constants,
   a per-game dense range for the rest, the map stored in `GameState` and passed to `Counts` by reference) as its own change with its own benchmark, before
   adding the set, not during it.

Smaller steps that need no redesign and buy headroom: put the cards of mixed piles (Knights, Ruins) and the Prizes/Rewards/Traveller upgrades, which are only
ever in a few zones, into `SmallMultiset` side zones; keep the Castle ids consecutive and the split bottoms in one range (already so), and keep the new pseudo
ids above `NUM_ZONE_IDS` so that no loop over zone ids ever visits them.

## 5. After Nocturne (actual numbers)

* Zone ids 0..344 (Nocturne 280-327, Ruins/Knights 328-343); pseudo ids 344..427 (Events 344-376, Landmarks 377-397, Boons 398-409, Hexes 410-421, States 422-426),
  `NUM_CARDS` 427, `Counts::LANES` 352 (the next multiple of 32), `SupplyMask` still 384 bits (the 344 zone ids fit; 40 ids remain).
* Rising Sun's remaining ~100 cards do not fit: a further +128 ids would need LANES 480 and a larger supply mask, so per-game compaction (section 3) is the next step if Rising Sun is done.
* Cost: LANES 320 -> 352 alone cost 2.5% speed (1.77M -> 1.725M games/s); the Nocturne code another 0.5% (final 1.70-1.72M). `GameState` 18.4 KB -> 19.55 KB (`PlayerState` 2560 B, `TurnState` 768 B,
  `Counts` 352 B, `Frame` 48 B, `CardDef` 40 B); `tests/state_size.rs` bounds: 19,700 / 2,600 / 800 / 48 / 352.
* Boons, Hexes and States are pseudo-cards kept in masks (`NocState`), not in `Counts` lanes, so they cost nothing in the zone width.

## 6. After Hinterlands (actual numbers)

* Hinterlands' 26 kingdom cards took ids 328-353; the Ruins/Knights (354-369) stay the highest zone ids, Events start at 370. `NUM_ZONE_IDS` 370, `NUM_CARDS` 453,
  `Counts::LANES` 352 -> 384 (the next multiple of 32), `SupplyMask` still 384 bits (370 fit; 14 ids remain).
* Cost: whole games 1.69M -> about 1.57M/s right after the widening, 1.59-1.63M with the full set. `GameState` about 20.7 KB (`PlayerState` 2720, `Counts` 384).
  The next set needs per-game compaction or a cached total in `Counts` first (sections 3-4).

## 8. After the Promo cards (Governor, Dismantle, Church, Marchland, Summon)

* Four kingdom cards took ids 354-357 (after Hinterlands; the Ruins and Knights are 358-373), so `NUM_ZONE_IDS` is 374 and **6 ids are spare** below the 380 bookkeeping limit. The Event Summon
  is 407, after Windfall (the Landmarks follow it): `NUM_EVENTS` 34 (the `events` masks are `u64`, so 30 spare), `NUM_CARDS` 458. A set of ~6 more zone ids needs `LANES` 416 (see section 7).
* Cost: BMU vs Double Witch 1.978M to 1.951M games/s (-1.4%, best of 12 interleaved); `GameState` 20 704 to 20 896 bytes (two per-player fields, `church` and `summon_cards`).

## 7. After the second speed pass (`Counts` bookkeeping, seats)

* `Counts` keeps 4 bookkeeping lanes at the end (a bitmap of the 32-lane blocks that may hold a card, and the cached total), so card ids must be below `LANES - 4` = 380: 370 are used,
  **10 are spare** before the next widening (it was 14).
* Because `total`, `is_empty`, `iter`, `add_all` and `clear` now cost with the blocks that hold cards, not with `LANES`, the widening is cheap: `LANES` 384 -> 416 measured at
  -0% (BMU vs Double Witch, 2.28M vs 2.31M games/s) and -3% (Seaside/Prosperity pairing, 2.09M vs 2.02M), in the noise of the first. A set that needs up to 412 zone ids is a plain
  widening to 416 (13 blocks; the bitmap is a `u16`, so at most 16 blocks = 512 lanes); `SupplyMask` is still 384 bits and needs its fourth word past 384 ids.
* The per-game compaction of section 3 stays unneeded for now; the "cached total" idea of section 6 is done (and gave about 1%: the block bitmap is what paid).
* `GameState` is 20 704 bytes, but `GameState::dup()` copies 9.8 KB in a 2-player game (see `docs/adding-a-set.md`, "Keeping `GameState` small"): the size matters less than the live part.

## 9. The full card count (plan for every remaining expansion)

Counted from the wiki copy (`C:/explore/wiki/dominion-wiki/pages/0`: `Sets`, `Renaissance`, `Menagerie (expansion)_6737`, `Allies`, `Plunder (expansion)`, `Promo`). Today (after Rising Sun and the Promo cards):
398 zone ids (`NUM_ZONE_IDS`; 14 spare below the 412 that `LANES` 416 allows), 109 pseudo ids (44 Events, 21 Landmarks, 15 Prophecies, 12 Boons, 12 Hexes, 5 States), `NUM_CARDS` 507.

| Still to come | Zone ids | Pseudo ids | Notes |
|---|---|---|---|
| Renaissance | 25 | 25 (5 Artifacts, 20 Projects) | Villagers and Coffers are counters; Projects are a per-player bitmask, Artifacts a holder per Artifact |
| Menagerie | 31 (30 kingdom + Horse) | 40 (20 Events, 20 Ways) | an Exile mat per player (a `SmallMultiset`) |
| Allies | 49 (25 single piles + 6 split piles of 4 cards) | 23 Allies | the 6 split piles *rotate* (4 cards, not 2); Favors are a counter; Liaisons are ordinary kingdom cards |
| Plunder | 55 (40 kingdom + 15 Loot) | 30 (15 Events, 15 Traits) | a Loot pile outside the Supply; a Trait sits on a Supply pile (a per-pile record, like Empires' tokens) |
| rest of the Promos | 2 (Black Market, Stash) | 0 | the Black Market deck is the kingdom cards not in the game: a mask, not a zone |
| **Sum** | **+162** | **+118** | |
| Arcana (announced for 2026: 500 cards, 37 kingdom piles, mechanics unknown) | about +50 | about +30 | an assumption: 37 kingdom piles and about a dozen non-Supply cards |

So **560 zone ids and 227 pseudo ids** (`NUM_CARDS` 787) without Arcana, **about 610 zone ids and 257 pseudo ids** (`NUM_CARDS` about 870) with it. Nothing else in the wiki's set list is missing:
the later 2nd-edition reprints change cards inside sets that are already in, and the Promo page lists 12 releases, 10 of them done. Planning figures: **640 `Counts` lanes, 5 `SupplyMask` words (640 bits), `CardId` stays 16 bits**.

When each limit is crossed, in the order the sets are likely to come (Renaissance, Menagerie, Allies, Plunder, Arcana):

| After | Zone ids | `LANES` needed | `SupplyMask` | Block bitmap |
|---|---|---|---|---|
| now | 398 | 416 | 4 words | `u16` |
| Renaissance | 423 | 448 | 4 | `u16` |
| Menagerie | 454 | 480 | 4 | `u16` |
| Allies | 503 | 512 (ids below 508: the last that fit with a `u16` bitmap) | 4 (ids below 512) | `u16` (16 blocks) |
| Plunder | 558 | 576 | 5 words | `u32` (18 blocks) |
| Arcana (est.) | 610 | 640 | 5 | `u32` |

`GameState` at 640 lanes: a `Counts` grows by 224 bytes and a state holds 33 of them (5 per seat times 6, the Supply, the trash, `TurnState::played`), so +7.4 KB: about 30 KB, 31.5 KB with the new sets' own state.
A 2-player `dup()` copies 13 `Counts`, +2.9 KB: about 13.5 KB (14.5 KB with the new state). The wasm stack (8 MiB) and the search's recursion are the things to re-check at that size (`tests/state_size.rs`).

## 10. Measurements for the plan (third speed pass)

`bench/perf.sh` (see its header) runs the benchmarks of this section with two builds interleaved, best of 5, and diffs the outputs. All the numbers below are from one machine (32 logical CPUs; the match benchmarks on 4 threads, because
with all of them the run-to-run noise is larger than most of the effects; the same binary measured against itself differs by up to +-3%).

**Widening cost.** Only `LANES` changed (and the bitmap to a `u32` for 576), `dominion-sim match` games per second (best of 5, 4 threads), nothing else touched:

| `LANES` | 128 | 224 | 320 | **416 (today)** | 512 | 576 |
|---|---|---|---|---|---|---|
| Big Money Ultimate vs Double Witch | 444k (+14.6%) | 389k | 388k | 387k | 367k (-5.3%) | 363k (-6.2%) |
| Smithy BM vs Laboratory BM | 462k (+13.2%) | 414k | 405k | 408k | 392k (-4.0%) | 386k (-5.4%) |

(The 128-lane build is only valid for base-set kingdoms, which is all those two pairings use.) Two facts: the cost is **flat between 224 and 416 and steps at about 500 lanes** (probably where the 2-player copy stops fitting in a cache level), so
the widening to 640 is a one-time step of about -7%, not a slope; and the *only* shrinking that pays is to about 128 lanes (+15%), nothing in between.

For the bots that search, the widening costs much more, because **`turn_hash` walks every lane of every `Counts` it hashes** (six of them, plus `TurnState::played`): 340 ns at 416 lanes, 435 ns at 576 (+28%). The match benchmarks with search bots lost 19% (village/smithy engine vs
Kitsune BM) and 11% (the Rising Sun kingdom), the position analysis 8%, at 576 lanes. This part of the cost is avoidable: see the recommendation.

**Compaction, with the numbers.** Per-game dense card ids would bring a `Counts` to 64-128 lanes, i.e. the +15% of the 128-lane column against today, and about +25% against a state widened to 640 lanes (444k against about 360k games/s) for the plain bots; the search bots add whatever the hash costs
(below). The price is unchanged from section 3 (a lane map consulted at every `Counts` access, the map travelling with the state, a two-tier scheme for the ids the engine names as constants) and the gain is not available in small steps (nothing between 416 and 224). Not implemented.

**Recommendation (replaces section 4's).**

1. *Do not compact.* Widen in the stages of the table above. `LANES` 448 and 480 are the same code as today. At Allies (`LANES` 512) the 16-block `u16` bitmap is full: from Plunder on the bitmap is a `u32` (the bookkeeping lanes become 6),
   and `SupplyMask` gets a fifth word when an id passes 511.
2. *(Done: `Counts::hash` hashes the 13 legacy blocks and, past them, only blocks that hold a card, each with its index; the values are unchanged at 416 lanes, and `upper_blocks_are_hashed_only_when_they_hold_a_card` pins the wider layout on a 640-lane array.)* *Before the first widening past 416, make `Counts`' `Hash` independent of `LANES`*: hash the 13 legacy blocks exactly as today (length prefix 416, so every existing key is unchanged and the results stay identical), and the blocks above them only when the bitmap says they hold a card (never, for
   today's cards). Otherwise every added block costs 4 more dependent hash steps in each of the six `Counts` of every search node (the 28% above). `Counts::hash` already goes block by block and skips the multiply-xor for empty blocks, so this is a small change to it. `stable_seed` (SipHash over the same bytes) is unaffected.
3. If the plain bots' speed matters more than the cost of a larger redesign after Plunder: keep `Counts` dense for the Supply and the trash (they are indexed by pile all the time) and make the *player zones* sparse (sorted `(id, count)` pairs, capacity about 32; the `Counts` methods are the whole interface, so the layout can change behind them).
   A 2-player state would copy about 6 KB instead of 13 KB, which is what the 128-lane column measures (+15%), minus the dearer `get` and `add`, plus a cheaper draw (`nth` walks pairs, not 32-lane blocks). Prototype it with the real benchmark first; the 224-416 plateau says that only a large shrink is worth anything.

## 11. What the third speed pass changed (results unchanged)

Profile first (`tools/winprof`: a sampling profiler for Windows that needs no admin rights; `bench/perf.sh` for before/after). The result for the bots that search: **55-60% of the time was the transposition-table key** (`search::hash::turn_hash`), a chain of FxHash steps (rotate, xor, multiply: 5 cycles of latency each, nothing to run in parallel)
over 6 + 1 `Counts` of 52 words each, nearly all of them zero. The key must not change (it indexes a direct-mapped table: a different hash changes which entries evict which, hence the node counts), and the chain cannot be shortened or reordered, so the work went into doing the same steps faster and fewer times:

* `Counts::hash` feeds the lanes in 32-byte pieces and a block without a card (the bitmap says so) as a constant piece, so the search's `FxHasher` runs its steps without the xor (4 cycles each instead of 5) and no 416-byte copy is made; a stream hasher (`stable_seed`'s SipHash) takes the same bytes in 32-byte writes. A test pins the equality with the hash of the plain `[u8; LANES]` for both kinds of hasher. `turn_hash` 410 ns to 338 ns on states of random games (-17%).
* `HashCache` (in `Searcher`): the first part of the chain (`me`, hand, known deck, unknown deck) is stored with the zones it was computed for. A node's children are hashed one after another and most of them (every Buy that gains a card, every choice that changes only the coins) share those zones with the previous state,
  so the hash resumes from the stored chain: the hit rate is 76% on the analysis positions, 41% in the village/smithy engine games, 91% in the Rising Sun kingdom, and it saves 3 of 6 `Counts` per hit. The equality of cached and plain hashes is a test (`tests/hash.rs`); the search results are the same (`bench/perf.sh` diffs them).

| Benchmark (best of 5, interleaved) | before | after | |
|---|---|---|---|
| plain bots, BMU vs Double Witch, 300k games | 792 ms | 771 ms | +2.8% (noise: the plain path does not use the hash) |
| search bots, village/smithy engine vs Kitsune BM, 8000 games | 1331 ms | 1216 ms | +9.5% |
| search bots, Rising Sun kingdom, Kitsune BM vs Samurai BM, 8000 games | 747 ms | 686 ms | +8.9% |
| `Searcher::analyze` on four positions (`bench_analyze`) | 370 ms | 339 ms | +9.2% |
| league run (2 islands x 8, 5 generations) | 7883 ms | 7608 ms | +3.6% |

Resuming the chain inside the discard pile for the children of a Buy was tried and dropped: the chain is sequential, so only the blocks *before* the first one that differs can be reused, and the cards gained (Copper, Silver, Gold, the Victory cards, the Curse and the cheap kingdom cards) sit in block 0; the saving was within the noise of the benchmark (+-1%), for an extra copy of the pile and 14 stored states. `tests/hash.rs` keeps the test that the children of every decision hash like `turn_hash` with one cache.

Tried and left alone: a hash cache with more entries (8 entries: 47% hits instead of 41% in the engine games), fat LTO (no gain on the analysis benchmark or the league run). Left for later, in the order of value: resuming the chain inside the discard pile for the children of a Buy (they differ in one lane: saves about a third of a `Counts` per node),
caching the root's `all_cards` and the opponents' Curse counts across the leaves of one search (`GainListEvaluator::leaf_value` rebuilds them for each leaf; roughly 5-8% of the bots' search time is `all_cards`/`add_all`), and the cost of `legal_choices` for the Buy phase (13% of the plain bots).
