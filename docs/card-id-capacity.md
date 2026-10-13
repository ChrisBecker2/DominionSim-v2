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

