# Search in the league: fast bots, full bots, and selective search

Question: can the bots' turn search (what a bot does when it plays a single game or in Simulate) be used in the kingdom
league "to reasonable effect for specific cards or circumstances, even if not to the same depth"?

Short answer: yes, it is implemented (`SearchMode::Selective`), it removes most of the fast-versus-full difference for
the cards where there is one, and it costs a fraction of full search. But the experiments also show that on the kingdoms
tried a plain fast league with a larger population (10x40 instead of 4x16 islands, 60 instead of 25 generations) found champions as
strong as full search's, in about a tenth of its time (or less). So it is **opt-in, default off**; use it for the final stages (validation, exploiter rounds) or for
kingdoms with the cards in the first table below, not for the main evolution.

## 1. What a fast bot does not do

`arena::set_fast` sets `search_play = false` and `win_this_turn = false`. A bot with `search_play = false` decides these
by rule instead of by searching the turn (`Strategy::decide` and the helpers around it):

- which action to play, in what order (including a lone card that might be better left in hand: `lone_risky`);
- Treasure play with choices (Investment, Crystal Ball, Anvil...), Alms before Treasures;
- Mode decisions of the turn player (Pawn, Steward, Courtier, Minion...);
- Throne Room/King's Court targets, Captain/Prince picks;
- calls (Duplicate, Royal Carriage, Coin of the Realm), overpay, Debt payment when Change is in the game;
- Artificer's discards, the order of start-of-turn effects (Prince, Clerk).

Search is `eval::search_play_choice`: the rest of the turn is searched with expectimax (`crates/search`), first with a
1000-node budget and, if that is not exact, again with 200000 nodes, scoring leaves with `GainListEvaluator` (the
strategy's own gain list). Everything below the decision follows the strategy's rules.

## 2. Measuring the gap per card (`examples/search_gap.rs`)

For every kingdom card X that is an Action or Treasure (288 cards, Base to Nocturne): the league's own "BM + X" anchor
(`baselines.rs`; trashers get "Trash with X") on a kingdom of X plus nine Base cards (Village, Smithy, Market, Cellar,
Militia, Moat, Workshop, Mine, Laboratory). The same rules play as a **full** bot (all searches) against a **fast** bot
(`set_fast`), 4000 games, seats rotated. A third bot, **only-this-card**, searches only the decisions that involve X
(selective search with the mask `{X}`). Cost is CPU time of a game relative to a fast game (fast bots are so cheap
that their time is measured over 20x the games). Reproduce: `search_gap.exe 4000 [name-filter]`, env `TIMEONLY`, `SKIP`,
`AFTER`, `BUDGET`.

Findings:

- **174 of 288 cards play identically** (exactly 50.0%): choice-free cards, and most cards whose decisions the rules
  already make as the search would. 61 cards show a gap of 3+ points (about 2 sigma at 4000 games), 10 are
  *negative*.
- **Large gaps are mostly cards that must be played or declined with a trade-off**: trash-for-benefit cards
  (Remodel 65.6%, Trading Post 67.5%, Stonemason 88.6%, Remake 80.1%, Develop 77.1%, Forge, Upgrade, Replace,
  Salvager, Altar, Butcher), cards with a mode (Steward 63.1%, Courtier 63.2%, Pawn 56.9%), cards that copy or play other
  cards (Overlord 99.3%, Band of Misfits 89.7%, Crown 84.6%, Captain 64.6%, Necromancer 60.7%), and discard-for-value
  cards (Root Cellar 83.8%, Amulet 89.3%, Anvil 87.5%, Investment 76.7%, Crystal Ball 73.0%, Lookout 76.0%, Tormentor).
  The BM+X baseline plays these with no `[[play]]` conditions, so the gap measures how often playing X blindly is wrong,
  which evolved rules can partly fix. Treat the numbers as an upper bound for a specialised strategy.
- **The gap and the cost are largely independent.** Some of the largest gaps are cheap (Stonemason 5x, Remake 3.5x,
  Develop 4.7x, Tanuki 6x, Trading Post 5x, Remodel 5x, Forge/Salvager/Altar/Treasure Map 5-7x, Amulet 15x) and some
  are very expensive (Overlord 2100x, Captain 1800x, Band of Misfits 870x, Root Cellar 740x, Royal Blacksmith 650x,
  Crystal Ball 620x). Many cards have **no gap at all and still cost 20-100x** (table 3: Cellar, Library, Shanty Town,
  Wishing Well, Page, Dungeon...): searching them is pure waste, and they are the ones selective search skips.
- **Search could be worse than the rules** (fixed, see the table in section 6): Miser (5.6%), Pillage (25.8%), Bishop (28.7%), Temple (34.8%), Tactician (35.2%),
  Haven (36.9%), Sacrifice (40.9%), Chapel (42.2%, 38.2% once the fast bots' trash rules improved) lost to the fast bot playing the same strategy,
  because the leaf score only saw what the gain list scores this turn. Native Village (43.8%) and Wild Hunt (45.8%) are still below 50.
- Search cost is highest where the turn has many chance nodes (Storyteller 76000x, Fool 27000x, Minion 10000x, Leprechaun,
  Druid, Catacombs, Pixie, Rebuild, Idol, Shepherd: thousands of times a fast game). A fast game is about 1-4 us of wall
  time on all cores; a full game on these cards is 3-100 ms. Such cards cannot be searched at the default budget in a league.
- Selective with the mask {X} equals full for nearly all cards. Exceptions: Pixie (61.1 full, 49.6 selective; probably its
  Goat heirloom's decisions) and Urchin (59.1 -> 51.8: Mercenary). Heirloom/related cards are not in the mask
  by themselves.
- A smaller node budget keeps the gap: with 100 nodes (first pass only) Minion 53.9 (53.8 at default), Captain 61.8
  (64.6), Rebuild 55.1, Catacombs 56.3, Root Cellar 84.5 (83.8), Remodel 65.8 (65.6), at 3-5x less time than 1000 nodes and
  4-8x less than the default; on the Root Cellar kingdom even 30 nodes keep 84.0% and cost 89x instead of 740x.
- Artificer's gap with two copies is only 1.4 points, but with four copies the full bot beats the fast one 59-63%
  (earlier work), so it is in the table by hand.

## 3. Design

`SearchMode` in `EvolveConfig` (serde `search_mode`: `"off"` default, `"selective"`; `"full"` is `fast_eval = false`, as
before). Options for `selective`: `search_gap` (default 3.0 points), `search_max_cost` (100x a fast game),
`search_budget` (node budget of the second pass; the first pass is min(1000, budget); at or below 1000 only the first
pass runs; default 200000 = today's full search). In `league_run`: `SEARCH=off|selective|full`, `SEARCH_GAP`,
`SEARCH_COST`, `SEARCH_BUDGET`, `SEED`.

- `dominion_sim::search_gap::GAPS` is the measured table (name, gap, cost) for the 62 cards with a gap, and `mask(min_gap,
  max_cost)` turns it into a per-card-id bit mask. The mask is kingdom independent: a card that is not in the game never
  appears in a decision, so the mask needs no kingdom.
- `Strategy::set_search_cards(mask)`; `Strategy::searches(decision, choices)` replaces the plain `search_play` test at every
  search site (`decide`, `play_decision`, `play_treasure_decision`, `night_decision`, `choose_mode`, Captain). A decision is
  searched when `search_play` is on and, if there is a mask, its source card or one of the offered cards is in the mask (a Debt
  payment counts as Change). Every other decision follows the rules, as for a fast bot. The play-order shortcuts
  (`obvious_play`, `cantrip_first`) stay on for searched decisions. `Strategy::search_budget` is the (b) variant.
- `arena::set_play_mode(strategy, fast, Some(&Selective{cards, budget}))`; `Arena.search_cards` is set by
  `EvolveConfig::search_mask()` at every place an Arena is built (fast arenas only: validation, polish, measurement and
  exploiter arenas included; the final round robin keeps full bots).
- With the option off nothing changes: `league_run` output is byte-identical to master for a fixed seed (checked:
  `sel_id` diff of the niche table and crosstable; `cargo test --release --workspace` passes).
- (c) Stage-based needs no code: run the league with `SEARCH=off`, then validate with `validate_full`/`final_full` (already
  the default for the round robin). (d) Caching decisions per position was not tried: the search already runs with a fixed
  per-position seed, and a position rarely repeats within a match.
- `run_league` (the `dominion-sim league` command) now initialises the big-stack thread pool like `match` does; before, a
  league of Witch strategies overflowed a rayon thread's stack (also on master).

Tools: `examples/search_gap.rs` (the table), `examples/mode_cost.rs` (what each mode costs on a directory of
strategies on one kingdom).

## 4. Evaluation on real kingdoms

Setup: `league_run` with 4 islands x 16, 25 generations, races 200/400/1000, 1 exploiter round of 10 generations,
polish 2000, validation 4000, final round robin 1500 games. Modes: **off** (fast), **selective** (gap 3, cost 100,
default budget), **selb** (selective, gap 3, cost 300, budget 100), **full** (`fast_eval = false`), and **offbig**
(fast with 10 islands x 40 and 60 generations). Two seeds on the Throne-Room-style and the Rising Sun kingdom, one on the
control. Strength = every mode's champions plus the anchors play a round robin with **full bots**
(`dominion-sim league`, 600-2000 games per pair, one shared set of deals), so the comparison does not use the fast bots'
view of the world.

Kingdoms: **rs** = Biding Time, Kintsugi, Aristocrat, Artist, Kitsune, Root Cellar, Samurai, Artificer, Distant Lands,
Dungeon, Duplicate, Page. **mode** = Throne Room, Remodel, Steward, Courtier, Trading Post, Upgrade, Village, Smithy,
Militia, Moneylender (five gap cards). **ctl** = Village, Smithy, Militia, Witch, Market, Laboratory, Moat, Festival,
Council Room, Moneylender (no gap card).

### Wall time (seconds, one process on all cores; the final round robin with full bots is included in each)

| kingdom, seed | off | offbig | selective | selb | full |
|---|---|---|---|---|---|
| ctl, 1 | 12 | | 12 | 12 | 69 |
| mode, 1 | 10 | 31 | 292 | 73 | 99 |
| mode, 2 | 13 | 29 | 87 | 65 | 70 |
| rs, 1 | 45 | 70 | 832 | 367 | 1132 |
| rs, 2 | 116 | 74 | 5292 | 1218 | stopped after 1h at generation 22 |

Wall time of a whole run depends on the trajectory as much as on the mode (a run that finds Kitsune/Courtier decks searches
longer turns): selective took longer than full on mode seed 1 and 5x longer than selective seed 1 on rs seed 2. The controlled
cost is better: `mode_cost.exe` plays the same 33 (mode) and 41 (rs) strategies against each other in each mode:

| strategies from | fast | selective | selb | full |
|---|---|---|---|---|
| mode kingdom | 1.8 us/game | 41.6 us (23x) | 30.4 us (17x) | 54.6 us (30x) |
| rs kingdom | 4.1 us/game | 360 us (88x) | 223 us (54x) | 609 us (148x) |

(wall microseconds per game with all cores busy). So selective saves 41% of full's time on the rs kingdom (Kitsune, Root Cellar and Artificer
are in the mask) and 24% on mode; with the small budget, 63% and 44%. On the control it costs nothing (no card in the
mask: exactly the fast bot, same champions).

### Strength with full bots

Each cell: percent of games the first group's champions win against the second group's, in a round robin of all
champions plus the anchors played with **full** bots. "best" is the best champion (by win rate against the field) of each
group; "mean" averages every pair of champions. O = off, G = offbig, S = selective, B = selb, F = full.

| kingdom, seed | G vs O | S vs O | B vs O | F vs O | G vs S | F vs S | F vs G |
|---|---|---|---|---|---|---|---|
| ctl, 1 (mean) | | 50.0 | 50.0 | 51.0 | | 51.5 | |
| mode, 1 (mean) | 67.7 | 64.0 | 61.6 | 68.8 | 54.0 | 54.5 | 50.4 |
| mode, 1 (best) | 81.3 | 82.2 | 78.4 | 78.0 | 52.5 | 50.1 | 49.5 |
| mode, 2 (mean) | 54.9 | 58.5 | 56.1 | 53.5 | 46.7 | 45.2 | |
| rs, 1 (mean) | 67.9 | 48.6 | 51.8 | 54.7 | 68.3 | 56.4 | 36.4 |
| rs, 1 (best) | 66.7 | 63.9 | 62.3 | 70.1 | 60.0 | 64.5 | 51.8 |
| rs, 2 (mean) | 72.9 | 42.5 | 54.2 | | 79.8 | | |
| rs, 2 (best) | 65.6 | 55.3 | 52.8 | | 62.0 | | |

In the control kingdom the champions of all modes are the same strategies (selective and selb equal off exactly); full
differs only through its choice of one Moat + Witch variant. Nothing is lost or gained.

Reading it:

- On mode, rs seed 1 and mode seed 2 selective/selb/full/offbig champions beat plain `off` champions at the same small
  settings (best champions win 64-82% of games against off's best on mode seed 1 and rs seed 1; the mean over all champions on rs seed 1 is 48.6%, i.e. no better), but **the `off` run at 4 islands x 16 and 25 generations
  is simply under-searched**: `offbig` (10 islands x 40, 60 generations; 30-75 seconds) matches the full run (51.8% of
  games for full over offbig on rs, 50.4% mean on mode) and beats selective on both rs seeds (62%, 60%) and about equals it on mode. 
  The earlier worry that fast evaluation leads evolution to bad strategies is **not supported** here: with more search
  effort the fast league finds what the full league finds.
- rs seed 2: selective's mean champion is *worse* than off's (42.5%) though its best is slightly better (55.3%); the long run
  (5292 s) is the slowest of all and offbig, which found Distant Lands + Samurai + Kintsugi, beats it 62-80%.
  With one seed per cell the noise of a league run (which niches happen to be found) is as large as the effect of the mode.
- Selective is therefore not a clear win per unit of time. Per unit of time it is worse than just evolving more in fast mode.

The measurements say nothing against selective search for a specific stage: it is cheap relative to full, it does not
change kingdoms without gap cards, and for gap cards it removes most of the fast bot's blind spots (the table's gaps are
measured with the mirror BM+X, where the evolved strategy has no play rules for the card).

## 5. Recommendation

1. Keep the default (`search_mode = off`, fast evaluation). Do not spend the evolution budget on search: a 6x larger population and 2.4x the generations in fast mode
   was as good as the full-bot run in about a tenth of the time (rs and mode kingdoms, two seeds; one seed on the control).
2. Use `search_mode = selective` where evolution would otherwise evaluate a **high-gap, cheap card** blindly: Remodel,
   Trading Post, Stonemason, Remake, Develop, Tanuki, Trader, Forge, Salvager, Altar, Upgrade, Replace, Treasure Map,
   Steward, Courtier, Count, Graverobber, Craftsman, Amulet, Forager (gap 8-39 points, 5-15x). It is exact in those
   decisions (identical to full) and costs 4-6x a fast game per card. Settings: `search_gap = 3`, `search_max_cost = 100`.
   Reasonable for validation and the exploiter rounds (`validate_full` = false but the mask on), which is where a champion
   is re-checked against the current hall of fame.
3. Do not add the expensive cards (cost over 100x: Overlord, Captain, Band of Misfits, Root Cellar, Royal Blacksmith, Crystal
   Ball, Minion, Fool, Storyteller...) to evolution. With a small budget (`search_budget = 100`, `search_max_cost = 300`)
   they become 4-8x cheaper and keep the gap (Captain 61.8 of 64.6, Root Cellar 84.5 of 83.8, Minion 53.9 of 53.8), which makes them usable for the
   last stages, or for a single kingdom with one such card.
4. The final round robin keeps full bots, which is right for human-usable strategies, but note the negative cards: Miser,
   Pillage, Bishop, Temple, Tactician, Haven, Sacrifice, Chapel play *worse* when searched. A full-bot round robin
   under-rates strategies built on them. This was fixed in the evaluator (table in section 6); the round robin can keep full bots.
5. Other cost cleanups the table suggests: many cards with no gap cost 20-130x as full bots (Cellar, Library, Shanty Town,
   Wishing Well, Page, Dungeon, Diplomat, Lurker, Masquerade...). The single-game bot could skip search for them
   (a "never search" list) and get faster at no cost.

## 6. Tables (4000 games, mirror "BM + X" on X + nine Base cards)

`full v fast %` is the full bot's win rate against the fast bot with the same rules; `only-this-card v fast %` the win rate of a bot
that searches only decisions involving the card; costs are CPU time per game relative to a fast game.

### Cards where searching helps (gap of at least 3 points), largest first

| card | class | full v fast % | only-this-card v fast % | full cost | only-this-card cost |
|---|---|---|---|---|---|
| Rats | order | 99.9 | 99.9 | 203.1x | 208.1x |
| Overlord | order | 99.3 | 99.3 | 2068.4x | 2109.3x |
| Band of Misfits | order | 89.7 | 89.7 | 813.8x | 868.0x |
| Amulet | order | 89.3 | 89.3 | 14.7x | 14.8x |
| Stonemason | order | 88.6 | 88.6 | 5.1x | 5.2x |
| Anvil | choice | 87.5 | 87.5 | 69.6x | 68.4x |
| Crown | order | 84.6 | 84.6 | 77.2x | 76.9x |
| Root Cellar | choice | 83.8 | 83.8 | 743.0x | 741.4x |
| Remake | order | 80.1 | 80.1 | 3.4x | 3.5x |
| Royal Blacksmith | order | 78.5 | 78.5 | 637.3x | 650.5x |
| Develop | order | 77.1 | 77.1 | 4.8x | 4.7x |
| Investment | choice | 76.7 | 76.7 | 65.7x | 63.9x |
| Tanuki | order | 76.3 | 76.3 | 6.8x | 6.0x |
| Lookout | order | 76.0 | 76.0 | 129.2x | 128.8x |
| Crystal Ball | choice | 73.0 | 73.0 | 626.5x | 624.1x |
| Trader | order | 69.9 | 69.9 | 6.4x | 6.4x |
| Forager | order | 69.3 | 69.3 | 9.2x | 9.2x |
| Trading Post | order | 67.5 | 67.5 | 5.2x | 5.0x |
| Tormentor | order | 67.4 | 67.4 | 297.6x | 206.7x |
| Remodel | order | 65.6 | 65.6 | 6.2x | 5.3x |
| Dismantle | order | 65.4 | 65.4 | 18.2x | 9.0x |
| Captain | order | 64.6 | 64.6 | 1855.7x | 1816.3x |
| Count | order | 64.2 | 64.2 | 24.5x | 24.3x |
| Courtier | order | 63.2 | 63.2 | 16.1x | 15.9x |
| Steward | order | 63.1 | 63.1 | 61.4x | 64.6x |
| Encampment | choice | 62.9 | 62.9 | 156.5x | 156.7x |
| Apprentice | order | 62.5 | 62.5 | 202.7x | 208.2x |
| Graverobber | order | 62.0 | 62.0 | 10.9x | 10.7x |
| Craftsman | choice | 61.8 | 61.8 | 5.9x | 5.8x |
| Tiara | choice | 61.4 | 61.4 | 78.4x | 79.7x |
| Rice Broker | order | 60.9 | 60.9 | 47.4x | 49.8x |
| Necromancer | order | 60.7 | 60.7 | 141.3x | 141.0x |
| Counterfeit | choice | 60.6 | 60.6 | 44.8x | 45.2x |
| Forge | order | 60.3 | 60.3 | 12.3x | 6.4x |
| Salvager | order | 60.2 | 60.2 | 6.6x | 6.7x |
| Treasure Map | order | 59.2 | 59.2 | 5.4x | 5.3x |
| Junk Dealer | order | 58.9 | 58.9 | 20.4x | 20.3x |
| Raze | order | 58.8 | 58.8 | 27.7x | 28.5x |
| Replace | order | 58.6 | 58.6 | 11.0x | 6.6x |
| Butcher | order | 58.5 | 58.5 | 21.4x | 12.3x |
| Altar | order | 58.1 | 58.1 | 5.3x | 5.3x |
| Fool | order | 56.8 | 57.2 | 27198.0x | 27198.0x |
| Catacombs | order | 57.1 | 57.1 | 6343.1x | 6420.1x |
| Peasant | free | 57.4 | 57.1 | 935.3x | 371.6x |
| Pawn | choice | 56.9 | 56.9 | 167.1x | 168.5x |
| Gold Mine | choice | 56.6 | 56.6 | 22.3x | 22.9x |
| Upgrade | order | 56.3 | 56.3 | 10.8x | 10.9x |
| Squire | choice | 55.7 | 55.7 | 18.8x | 18.7x |
| Rebuild | order | 55.6 | 55.6 | 6213.5x | 5624.4x |
| Governor | order | 55.2 | 55.2 | 1339.6x | 1322.2x |
| River Shrine | order | 54.9 | 54.9 | 6.6x | 6.7x |
| Ferryman | order | 54.8 | 54.8 | 565.4x | 467.3x |
| Leprechaun | order | 54.0 | 54.0 | 6606.3x | 6331.3x |
| Minion | order | 53.8 | 53.8 | 10162.1x | 10408.9x |
| Spice Merchant | choice | 53.7 | 53.7 | 50.1x | 50.0x |
| Kitsune | choice | 53.7 | 53.7 | 32.8x | 33.0x |
| Expand | order | 53.6 | 53.6 | 3.9x | 3.9x |
| Druid | order | 53.6 | 53.6 | 21952.4x | 6707.2x |
| Farrier | free | 53.5 | 53.5 | 2.1x | 2.0x |
| Idol | choice | 53.5 | 53.5 | 5349.5x | 3613.2x |
| Infirmary | order | 53.2 | 53.2 | 17.3x | 17.2x |

### Cards where the full bot was worse than the fast one (fixed)

Before: measured on the code before this work. Master: the same strategy against the fast bots after the fast-bot trash rules landed (they raised the
fast bot, so the gaps of the trashers shrank; Chapel fell to 38.2). After: this branch on top of that master. 4000 games each.

| card | before | master | after | cause and fix |
|---|---|---|---|---|
| Miser | 5.6 | 5.6 | 48.2 | A Copper on the Tavern mat was worth nothing, so the search never built a mat. A mat Copper is now worth $1 in every future hand that plays a Miser, priced by the gain list's tiers (`mat_copper_value`). |
| Pillage | 25.8 | 25.8 | 47.8 | Trashing itself through a selection was not recorded as "used up", so the search charged the whole gain-list weight of Pillage. Engine: a card that trashes itself from play for its effect is used up. |
| Bishop | 28.7 | 30.4 | 50.7 | VP tokens were worth 1.0 against gain-list weights of 1000 and up. Tokens given for a card are now priced as the strategy rates VP in Victory cards or as money (`1 / vp_per_coin` coins of hand, priced by the tiers), whichever is more. |
| Temple | 34.8 | 35.0 | 55.2 | The same tokens, and the thinner deck (below). Search now helps. |
| Tactician | 35.2 | 35.2 | 59.0 | The next turn's ten cards counted for nothing, only the hand lost this turn. Durations with a choice (Tactician, Haven) now add their next-turn cards and $ to the score as money priced by the tiers (`next_turn_value`). Search now helps. |
| Haven | 36.9 | 36.9 | 49.7 | The set-aside pick was Secret Passage's ("the most valuable card"), which put a Silver aside and lost the buy. Haven now sets aside a Treasure the buy does not need, else a dead card, else the cheapest (`haven_pick`); its returning Treasure counts for next turn. |
| Sacrifice | 40.9 | 46.4 | 52.0 | Thinning (below). |
| Chapel | 42.2 | 38.2 | 49.0 | Thinning: a trash the rules want was a +0.3 preference, nothing next to the buy it costs. `thinning_value` prices the better deck (average hand money of the deck without the trashed cards, over the turns left, in gain-list units), 30% of it, never a charge, only when the deck ends smaller, and at most 0.7 of the cheapest gain-list entry per card (without the cap Amulet and Count lose 3 to 8 points). |

Money is priced by the gain list's tiers: the weight of the best entry affordable with $m, for m = 0..15, smoothed over a standard deviation of $1.5 of
hand money (`RootCache::tiers`). Controls after the change (4000 games, against the master numbers): Smithy, Witch, Militia, Throne Room,
Moneylender, Mine 50.0 (unchanged); Remodel 51.8 (51.8), Develop 51.6 (51.6), Stonemason 53.8 (53.8), Courtier 52.5 (52.5), Steward 67.4 (61.0),
Investment 56.6 (54.4), Count 56.4 (55.0), Counterfeit 52.7 (52.8), Amulet 50.3 (50.3), Trader 49.9 (49.9). Cost per game relative to a fast game is
unchanged within noise (Smithy 1.1x, Chapel 7.2x, Remodel 15.0x). Still below 50 and not in scope: Native Village 43.8, Wild Hunt 45.8.

### No gap, but searching costs 20x or more (search is pure waste here)

| card | class | full v fast % | only-this-card v fast % | full cost | only-this-card cost |
|---|---|---|---|---|---|
| Storyteller | order | 52.3 | 52.3 | 74724.0x | 76004.3x |
| Hunting Party | order | 50.0 | 50.0 | 71318.8x | 70970.6x |
| Cartographer | order | 50.0 | 50.0 | 11008.5x | 11010.6x |
| Wandering Minstrel | order | 50.0 | 50.0 | 10528.7x | 10593.4x |
| Shepherd | order | 50.1 | 50.1 | 9679.7x | 9782.5x |
| Patrol | order | 50.0 | 50.0 | 7339.6x | 7493.0x |
| Magpie | order | 50.0 | 50.0 | 5659.1x | 5755.4x |
| Joust | choice | 51.9 | 50.0 | 3509.7x | 3130.5x |
| Tracker | order | 52.1 | 52.1 | 2067.7x | 1283.7x |
| Sacred Grove | order | 51.7 | 51.7 | 1686.0x | 1477.6x |
| Sage | order | 50.0 | 50.0 | 1661.5x | 1668.1x |
| Advisor | order | 50.0 | 50.0 | 1627.7x | 1660.3x |
| Bard | order | 52.4 | 52.4 | 1147.9x | 876.8x |
| Envoy | order | 50.0 | 50.0 | 1135.3x | 1131.0x |
| Imperial Envoy | choice | 50.0 | 50.0 | 1002.2x | 1006.8x |
| Sentry | order | 49.4 | 49.4 | 930.3x | 955.0x |
| Warehouse | order | 50.5 | 50.5 | 892.2x | 877.9x |
| Forum | order | 50.0 | 50.0 | 818.3x | 829.9x |
| Ironmonger | order | 50.0 | 50.0 | 789.9x | 781.9x |
| Journeyman | order | 50.0 | 50.0 | 787.2x | 801.0x |
| Cursed Village | order | 49.9 | 49.9 | 620.8x | 581.1x |
| Archive | order | 50.0 | 50.0 | 510.1x | 519.9x |
| Pooka | order | 50.3 | 50.3 | 506.8x | 477.7x |
| Carnival | order | 50.0 | 50.0 | 389.7x | 391.8x |
| Stables | choice | 50.0 | 50.0 | 385.9x | 386.5x |
| Witch's Hut | order | 50.0 | 50.0 | 358.8x | 361.2x |
| Cultist | order | 51.4 | 50.4 | 332.9x | 255.3x |
| Riverboat | free | 51.2 | 50.0 | 318.0x | 1.7x |
| Crossroads | order | 50.0 | 50.0 | 273.3x | 318.3x |
| Library | order | 50.0 | 50.0 | 133.1x | 131.8x |
| Poet | order | 50.0 | 50.0 | 129.9x | 128.0x |
| Sauna | order | 52.4 | 52.4 | 124.8x | 125.8x |
| Courtyard | order | 50.1 | 50.1 | 122.2x | 123.2x |
| Sea Chart | order | 50.0 | 50.0 | 117.9x | 119.9x |
| Inn | order | 50.1 | 50.1 | 115.7x | 115.8x |
| Vagrant | order | 50.0 | 50.0 | 114.8x | 111.9x |
| Shanty Town | order | 50.0 | 50.0 | 110.9x | 110.5x |
| Herald | order | 50.0 | 50.0 | 107.4x | 108.1x |
| Magnate | choice | 50.0 | 50.0 | 107.0x | 112.7x |
| Litter | choice | 50.9 | 50.9 | 104.2x | 106.5x |
| Tragic Hero | order | 50.0 | 50.0 | 102.4x | 110.0x |
| Patrician | order | 50.0 | 50.0 | 100.7x | 102.0x |
| Page | free | 50.8 | 50.1 | 94.1x | 27.7x |
| Menagerie | order | 50.0 | 50.0 | 84.5x | 86.1x |
| Secret Cave | order | 50.0 | 50.0 | 82.1x | 58.8x |
| Wishing Well | order | 50.0 | 50.0 | 78.4x | 78.6x |
| Dungeon | order | 49.5 | 49.5 | 76.9x | 76.9x |
| Charm | choice | 50.3 | 50.3 | 65.3x | 64.9x |
| Scheme | choice | 50.0 | 50.0 | 58.9x | 59.4x |
| Secret Passage | order | 50.7 | 50.7 | 56.2x | 55.9x |
| Watchtower | order | 50.0 | 50.0 | 47.7x | 47.8x |
| Vault | order | 50.0 | 50.0 | 46.6x | 47.4x |
| Rustic Village | order | 50.0 | 50.0 | 45.8x | 46.0x |
| Alley | order | 50.0 | 50.0 | 45.4x | 45.7x |
| Cellar | order | 50.0 | 50.0 | 44.2x | 39.8x |
| War Chest | choice | 50.0 | 50.0 | 39.1x | 39.2x |
| Gear | order | 50.0 | 50.0 | 38.8x | 39.0x |
| Ronin | order | 50.0 | 50.0 | 37.9x | 38.0x |
| Marauder | choice | 47.5 | 47.0 | 36.9x | 20.5x |
| Swindler | choice | 49.9 | 49.9 | 36.4x | 8.1x |
| Diplomat | order | 50.0 | 50.0 | 36.4x | 36.4x |
| Cauldron | choice | 50.0 | 50.0 | 36.2x | 36.4x |
| Masquerade | order | 52.8 | 52.8 | 36.1x | 36.0x |
| Guard Dog | order | 50.0 | 50.0 | 36.0x | 35.8x |
| Young Witch | order | 49.4 | 49.4 | 35.1x | 35.0x |
| Treasure Trove | choice | 50.0 | 50.0 | 34.7x | 34.7x |
| Relic | choice | 50.0 | 50.0 | 34.3x | 34.3x |
| Artificer | order | 51.4 | 51.4 | 33.5x | 33.0x |
| Market Square | order | 50.0 | 50.0 | 29.1x | 29.4x |
| Blessed Village | free | 50.7 | 50.7 | 28.8x | 18.9x |
| Plaza | order | 50.0 | 50.0 | 26.2x | 26.3x |
| Mining Village | choice | 50.0 | 50.0 | 26.1x | 25.9x |
| Hamlet | order | 50.0 | 50.0 | 26.1x | 26.4x |
| Daimyo | order | 50.0 | 50.0 | 24.4x | 24.5x |
| Settlers | choice | 50.0 | 50.0 | 24.3x | 24.4x |
| Death Cart | order | 50.8 | 50.0 | 23.8x | 9.0x |
| Oasis | order | 50.0 | 50.0 | 23.6x | 23.5x |
| Storeroom | order | 50.0 | 50.0 | 23.4x | 23.6x |
| Snake Witch | order | 49.9 | 49.9 | 22.9x | 22.7x |
| Aristocrat | order | 50.0 | 50.0 | 22.1x | 21.5x |
| Poacher | order | 50.0 | 50.0 | 21.2x | 21.5x |
| Jack of All Trades | order | 51.6 | 51.6 | 20.4x | 20.4x |

### No gap and cheap (133 cards)

Armory, Artisan, Artist, Astrolabe, Baker, Bandit, Bandit Camp, Bank, Baron, Bazaar, Beggar, Berserker, Blockade, Border Village, Bridge, Bridge Troll, Bureaucrat, Candlestick Maker, Capital, Caravan, Caravan Guard, Catapult, Change, Chariot Race, Charlatan, Church, City, City Quarter, Clerk, Coin of the Realm, Collection, Conclave, Conspirator, Corsair, Council Room, Cutpurse, Duplicate, Enchantress, Engineer, Faithful Hound, Farmers' Market, Farmhands, Festival, Fishing Village, Fishmonger, Fool's Gold, Footpad, Fortress, Giant, Gladiator, Grand Market, Groundskeeper, Guide, Haggler, Harbinger, Harem, Haunted Woods, Herbalist, Hermit, Highway, Hireling, Hoard, Horn of Plenty, Hunting Grounds, Ironworks, Jester, King's Court, Laboratory, Legionary, Lighthouse, Lost City, Lurker, Margrave, Market, Merchant, Merchant Guild, Merchant Ship, Messenger, Militia, Mine, Mint, Moat, Moneylender, Monkey, Monument, Mountain Shrine, Mystic, Ninja, Nomads, Outpost, Peddler, Pirate, Poor House, Port, Prince, Procession, Quarry, Rabble, Ranger, Ratcatcher, Rice, Rogue, Royal Carriage, Sailor, Samurai, Scavenger, Sea Witch, Shop, Skulk, Smithy, Smugglers, Soothsayer, Souk, Swamp Hag, Tea House, Throne Room, Tide Pools, Torturer, Trail, Transmogrify, Treasury, Vassal, Villa, Village, Walled Village, Weaver, Werewolf, Wharf, Wheelwright, Wine Merchant, Witch, Worker's Village, Workshop.

## 7. Closing the gap: fast-bot defaults taken from what the search does

Method: `examples/fast_diff.rs <card>` plays fast-vs-fast games on the section 6 mirror and, at every decision of one seat, also asks the full
bot; it prints the disagreements grouped by (decision, card, fast choice, full choice), and with `DETAIL=1` the search's value and best line for
each option. `examples/fast_ab.rs <games> <cards>` plays the new fast bot against the old one (`Strategy::legacy_defaults`, a measurement switch
that restores the earlier defaults) and the full bot against each. `examples/fast_speed.rs` times them.

What the search does differently, in nearly every case: **it passes on a card that must trash something (or draw at a price) when the
play would only lose**. The baseline strategies carry an unconditional `[[play]]` rule, so the old fast bot played Remodel for a Silver swap,
Rats trashing a Gold, Lookout in a good deck, Root Cellar into a poor deck. Rules adopted (all on the scale of the search's own scoring: the gain list's
rank weights, `Strategy::weight_of` / `buy_weight` / `trash_value`; no search, no allocation):

| rule | cards |
|---|---|
| `SetDefaults::useless_play` (new): the card is not played, even when a `[[play]]` rule names it, if the best card of the hand to give up costs more (its own place in the gain list, plus the buy its coins pay for) than the effect gains (Remodel: the best listed card costing up to +$2; Remake/Upgrade: exactly +$1; Tanuki +$2; Expand +$3; Stonemason: two cheaper cards, none listed counts as a loss; Develop: +$1 and -$1; Trader: a Silver per $1; Dismantle: a Gold and a cheaper card; Rats/Forager: nothing, Forager counts the $ for the Treasures in the trash; Trading Post: the best pair against a Silver and its $2) | Remodel, Remake, Develop, Stonemason, Trading Post, Trader, Forager, Rats, Tanuki, Upgrade, Expand, Dismantle |
| the same sum picks the card to trash (Develop, Stonemason, Trader, Forager, Dismantle, the "trash a card, gain a card" frames); a forced trash takes the card that costs least to lose instead of the cheapest card; the built-in Estate-trashing is off while the gain list buys Estates (endgame) | all forced trashes |
| Crown needs another Action in hand to play twice (stated rules too), and does not pick a second Crown; a lone Treasure Map is not played | Crown, Treasure Map |
| Root Cellar and Royal Blacksmith: played when the $ of the hand plus the expected $ of the cards drawn (average $ per card of the deck, Coppers not counted for Royal Blacksmith) still buy as much as the hand does now (Root Cellar pays 3 Debt) | Root Cellar, Royal Blacksmith |
| Lookout: not played once the chance that all three cards it looks at are cards the strategy lists exceeds 2% | Lookout |
| Amulet: of +$1, a Silver and the best trash, the option the buy values most | Amulet |
| Anvil: discards the Treasure whose $ the best card up to $4 pays for; Investment: +$1 if it reaches a better buy, else trash for VP with two Treasure types in hand; Crystal Ball plays a Treasure it reveals, and is played after the plain Treasures | Anvil, Investment, Crystal Ball |
| Counterfeit: the Treasure whose second play brings more than losing it costs | Counterfeit |
| Courtier, Count (second pick): the mode the buy values most (+$ by the buy reached, a gain by its list weight) | Courtier, Count |
| Overlord, Band of Misfits: the Action worth most in this hand (its $, its cards at the deck's average $, +Actions for the Actions in hand, an Attack a little; nothing for $ and cards when the hand already reaches the best listed buy). Searched as before when the bot searches | Overlord, Band of Misfits |

Results (mirror "BM + X", 4000 games unless noted; win rate of the first bot, 95% intervals about +-1.5 at 4000 and +-3 at 1000 games; the first
column is new fast against old fast, then the full bot against the old and against the new fast bot; `gap before` is the section 6 figure):

| card | new fast v old fast | gap before (full v old fast) | gap after (full v new fast) | full(new) v full(old) |
|---|---|---|---|---|
| Rats | 98.9 | 99.9 | 30.5 (the new fast bot beats the full bot) | 49.8 |
| Overlord (1000) | 96.4 | 99.2 | 75.0 | 49.6 |
| Band of Misfits (1000) | 83.4 | 89.0 | 63.9 | 50.0 |
| Amulet | 87.0 | 89.3 | 50.3 | 50.0 |
| Stonemason | 89.5 | 88.6 | 53.8 | 64.0 |
| Anvil | 86.9 | 87.5 | 51.3 | 50.0 |
| Crown | 84.6 | 84.6 | 50.0 | 50.0 |
| Root Cellar | 79.9 | 83.8 | 57.0 | 50.0 |
| Remake | 82.3 | 80.1 | 52.6 | 51.4 |
| Royal Blacksmith | 78.8 | 78.5 | 50.9 | 50.0 |
| Develop | 77.8 | 77.1 | 51.6 | 54.7 |
| Investment | 72.1 | 76.7 | 54.4 | 50.2 |
| Tanuki | 78.7 | 76.3 | 54.5 | 57.7 |
| Lookout | 78.3 | 76.0 | 47.6 | 50.1 |
| Crystal Ball | 69.5 | 73.0 | 52.6 | 50.0 |
| Trader | 62.4 | 61.9 | 49.9 | 50.0 |
| Forager | 64.2 | 69.3 | 54.9 | 47.3 |
| Trading Post | 69.0 | 67.5 | 49.8 | 51.3 |
| Remodel | 67.2 | 65.6 | 51.6 | 52.1 |
| Captain (1000) | 47.5 (not adopted: the hand-aware pick did not help) | 65.0 | 68.1 | 50.0 |
| Dismantle (2000) | 73.6 | 66.2 | 50.8 | 57.5 |
| Count (2000) | 59.5 | 63.4 | 55.0 | 50.0 |
| Courtier (2000) | 62.0 | 63.8 | 51.8 | 50.0 |
| Counterfeit (2000) | 58.0 | 60.6 | 53.1 | 50.0 |
| Treasure Map (2000) | 67.6 | 59.2 | 40.4 (fast bot better) | 50.0 |
| Upgrade | 53.7 | 56.3 | 54.6 | 51.7 |
| Expand | 57.6 | 53.6 | 51.6 | 55.0 |

Control cards (BM + Smithy, Witch, Militia, Moneylender, Throne Room, Village, Laboratory): identical to the old bots (50.0 in every column: the
new rules never fire). BM + Chapel: the new fast bot beats the old one 54.4% (the Estate default is off in the endgame); the full-bot columns for
Chapel (full 42.2 / 38.2 against old / new fast) still show the search playing Chapel worse than the rules, the case the evaluator work addresses.
The full bot (which follows the rules in its playouts) changed by 50% except Stonemason (64.0), Tanuki (57.7), Expand (55.0), Develop (54.7)
and Replace/Junk Dealer-like small cards, where it improved; Forager (47.3) is the one that may have lost a little (about 2 sigma).

Speed (`fast_speed`, 20000 fast games): the ordinary Base kingdom is unchanged within noise (-6% to +8%, 0.02-0.04 s per run); a
kingdom full of the trasher cards above is slower in the games that play them (+30-70%) because the fast bot now evaluates the hand at a play decision
(a few gain-list lookups per candidate card, no allocation).

Not done (still 55-65 points): Captain, Tormentor, Steward, Encampment, Apprentice, Graverobber, Craftsman, Tiara, Rice Broker, Necromancer, Forge,
Salvager, Junk Dealer, Raze, Replace, Butcher, Altar, Pawn, Gold Mine, Squire (a mode rule by buy value lost to the old default, 47.8: not adopted),
Kitsune, Spice Merchant and the rest of the table; and Overlord/Band of Misfits remain 14-25 points behind (the search sees the follow-up play
that the pick score does not). What the rules cannot see: conditions on the deck size inside the gain list (`total_cards >= 14` for a second
terminal) change after the trash or the gain, so Develop and Tanuki still differ from the search in a few positions.

## 8. Diagnosing a card where search loses to the rules

`cargo run --release -p dominion-evolve --example search_diff -- <card> [games] [details]` plays "BM + card" as a full bot against the fast bot and
lists the decisions where the search overrides the rules (with `ANALYZE=1`, the value of every option and its best line, from
`eval::analyze_play`; `DUMP=1` prints the strategy). The usual cause is a payoff the leaf score cannot see (look for a line whose value
differs by exactly a gain-list weight). Positions that were wrong are tests in `crates/sim/tests/search_vs_rules.rs`.
