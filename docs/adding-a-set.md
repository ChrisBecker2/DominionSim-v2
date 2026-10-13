# Adding an expansion

A set is added by adding files, plus a handful of one-line registrations. The seven sets so far (Base,
Intrigue, Seaside, Prosperity, Alchemy, the Promos, Cornucopia & Guilds) were each done in these steps; the
plan docs (`docs/*-plan.md`) say what each set needed beyond them. Use any finished set as the model
(`sets/promo.rs` is the smallest, `sets/seaside.rs` the most varied).

## 0. Before writing code

* Write the plan: the cards (cost, types, text at its latest version), the rulings from the wiki and FAQs, the
  mechanics the set adds (new zones, tokens, set-up piles, new reaction timing), and what is out of scope.
* Mark each card in the plan `free` / `choice` / `order` (see "Adding cards" in `CLAUDE.md`): it decides
  whether bots search the play order.
* Look for what is not generic yet. If a mechanic needs a new frame or `Then` continuation, add it to
  `state.rs` / `effects.rs` as a *generic* piece (no card-specific decision kinds); the card-specific body goes in the
  set file.

## 1. The engine: `crates/engine/src/sets/<set>.rs`

One file holds everything about the set's cards. Copy the layout of an existing set:

1. `pub mod ids { pub const X: CardId = N; ... }`: the set's ids, the next free ids after the last set (ids are
   dense; the `CARDS` assembly fails the build on a gap, a duplicate, or a card whose `set` is another set's).
   `cards::id` re-exports them, so `id::X` keeps working everywhere.
2. `const CARDS: &[(CardId, CardDef)]`: one line per card, `c(name, cost, types, coins, vp, cards, actions, buys)`
   wrapped in `choice_free(..)` / `has_choice(..)` / `order_sensitive(..)` (every Action and Treasure needs one; a
   test fails otherwise), `with_p(..)` for a Potion cost, `plays(..)` for a card that plays others, and the set's
   wrapper (`seaside(..)`, add `dark_ages(..)` next to it in `cards.rs`).
3. `pub struct <Set>; impl SetRules for <Set>`: the card behaviour. Everything has a no-op default; override
   what the cards need:
   * `play`: what a play does beyond the vanilla bonuses: usually pushes frames (`select`, `gain_frame`,
     `draw_frame`, `mode_frame`, `Frame::new(K::..)`). Match arms by card id, pushed so the first thing to resolve is
     last (see the header of `effects.rs`).
   * `modes`: the options of "choose one / many" cards (`ModeOpt` atoms).
   * `duration_bonus`, `duration_start`, `start_order_sensitive`, `schedules_own_duration`: the next-turn part of
     Durations. A start-of-turn effect that is not a pending Duration (a card held for this turn, a card played from the
     hand) is `turn_start_entries` + `take_start_effect` (+ `start_entry_named_by_arg` / `start_entry_repeats`).
   * `gain_triggers` with `GAIN_LO` / `GAIN_HI` (bits of the cards that have one, so that games without them skip the
     lot with one mask test) and `gain_live_extra`; `offers_on_discard` / `offers_at_cleanup` (cleanup offers);
     `end_of_buy`; `has_overpay` / `overpay_possible` / `overpay_with_potion` / `overpay`.
   * Reactions: `BLOCK_REACTIONS` (a revealed card that blocks an Attack), `REACTION_EFFECTS` (a revealed card that
     acts), `IMMUNE_IN_PLAY` (a card in play that protects its owner).
   * `vp` (a score that is not a fixed number), `may_buy`, `pile_size`, `reads_empty_piles`.
4. Below the impl: the card-specific parts of generic frames as `impl GameState { ... }` methods (the body of a
   `FrameKind` or `Then` that only your card uses), helper functions, and set-up (`setup`) if the set has piles
   outside the Supply.
5. A `#[cfg(test)]` test listing the set's choice-free and order-sensitive cards (copy it from another set: edit the
   lists deliberately when a card is added).

Registration, all in `sets/mod.rs` unless said otherwise (the compiler points at what is missing: the `match`es are
exhaustive):

* `pub mod <set>;` and `use <set>::<Set>;`.
* `CardSet::<Set>` in `cards.rs` (the enum, `ALL`, `name`, `set_by_name`) and a `cards::dark_ages(..)` wrapper.
* The set in `NUM_CARDS`, in the `parts` of `build_cards`, in `concat_sets!`, in `GAIN_LO` / `GAIN_HI`, and an arm in
  `with_set!`.
* The ordered hooks it takes part in: `turn_start_entries`, `gain_triggers`, `end_of_buy`, `end_of_turn_offers`. The
  order there is the order frames are pushed in; read the comment above each before choosing where the set goes.
* `pub use` of the set's ids in `cards::id` (`cards.rs`).

Capacity (the build asserts it): `counts.rs` `LANES` must cover `NUM_CARDS` (a multiple of 32 keeps whole-game speed;
224 since Dark Ages, which cost whole games about 5%), and `SupplyMask` in `state.rs` is two `u128`s (ids 0-255; `GAIN_HI` is
a `u128` too), so ids at 256 and above need a third word. `strategy.rs` and `eval.rs` size arrays by `NUM_CARDS`.

Things a set may need that the Dark Ages added, all generic: a *mixed pile* (`sets::dark_ages::MixedPile`: members with
consecutive ids at the top of the id range, a marker id for the pile, the top card holds the pile size; see the plan);
`SetRules::on_trash` (+ the `on_trash(..)` flag on the card's definition) for a "when you trash this" effect, with every
trash going through `GameState::trash_card` and `flush_trash_triggers`; `SetRules::ATTACK_REACTIONS` (a card in hand that is
discarded or revealed when another player plays an Attack and then does something); `Filter::Types`, `CostRange`,
`CostAtLeast`; `FrameKind::Ask` (an optional effect with no card to act on); a Mode frame's `Then::ModeWindow` (one table
serving two separate choices); non-Supply piles as `GameState` counters (`spoils`, `madman`, `mercenary`, taken with
`gain_side`).

New game state (a mat, a token, a per-turn counter): a field in `PlayerState` / `TurnState` (`state.rs`), counted in
`PlayerState::all_cards` if its cards are the player's for scoring, written and parsed in `text.rs` (old saves must
still load: a field is written only while nonzero), shown to agents in `PlayerView` (`agent.rs`), and an `Event` if
the log should say something (`engine.rs`, `web/app.js`'s log).

### Capacity and unusual card kinds (learned with Adventures)

* **Ids.** `CardId` is `u16`; `Counts::LANES` (a multiple of 32) must cover every id that can be in a zone, and `SupplyMask` has three words (ids 0-127, 128-255, 256 and up). Adding cards past a
  limit means widening those, and a speed check (the widening alone cost about 6%). Keep the mixed-pile members (Knights, Ruins) the highest ids.
* **Pseudo-cards that are never owned (Events).** Give them a `CardDef` and an id so names, strategies, text and the Lab work unchanged; keep them out of every `Counts`, zone and supply mask
  (`cards::is_event` is an id range test); the `Buy` decision offers `Choice::Card(event)`; the effect pushes the generic frames. Strategies list them in `[[gain]]`; gains that are not buys skip them.
* **Split piles (Sauna/Avanto).** One kingdom id names the pile; the other half is in the Supply mask with its count, is not legal to gain while the top half remains (`may_buy` and the Gain frame),
  and `empty_piles` counts the pair once.
* **A new mat or token.** Per-player public state (`tavern`, `tok`, `inherit`), counted by `all_cards` where the cards are still owned; a flag in `GameState::adv` so games without it pay one byte test;
  `PlayerView` accessors; the text format (`tavern:`, `tokens:`, `events:` lines) and the determinize/hash code; a generic frame for the choice (`FrameKind::Token`, `Act::Call`).
* **Non-Supply piles (Traveller upgrades)** are fields of `GameState`, listed in `side_pile_cards`, shown by `PlayerView::supply` and by the page as "not in the Supply".

### Learned with Empires (and Change)

* **Ids.** Zone ids are `0..NUM_ZONE_IDS`; the pseudo-cards (Events, Landmarks) are above and never in a `Counts` or a supply mask. A loop over card ids that indexes a `Counts`
  or a supply mask runs to `NUM_ZONE_IDS`, never `NUM_CARDS`. `LANES` is asserted against `NUM_ZONE_IDS`; the Events mask in `GameState::events` is a `u64` (`NUM_EVENTS` <= 64).
  `docs/card-id-capacity.md` has the counts through Rising Sun and the case against per-game compaction.
* **A new cost part (Debt).** `CardDef::debt`, `cost_string`, and one place that compares costs: `Frame::more`, `exact` (with `min` as the Debt), the "up to" test requires
  `debt_cost == 0` (`frame_choices`, `Filter::CostRange`, `ActionCostUpTo`). Buying takes the tokens (`engine::debt_buy_choices` offers `PayDebt` first); the table is in
  `alchemy-plan.md` section 5b and every row needs a Debt-cost card on each side.
* **Split piles** are `cards::SPLIT_PILES` (top, bottom): `pile_open` is the one test for "can this card be gained from its pile now" (also for the Castles, whose top is the lowest non-empty
  id); `empty_piles` counts a split pile or the Castles once.
* **Tokens on Supply piles** (VP, Debt) are `EmpState::piles` (`PileTokens`, a small record keyed by pile, not a per-card array); Landmarks that watch gains hook `watch_gain`.
* **Landmarks** are pseudo-cards with a bit in `EmpState::landmarks`; the ones that score at the end of the game are in `landmark_score` (and visible to agents through `PlayerView`), the
  token ones are updated by gain/trash hooks.
* **A decision that is almost never interesting (Debt payment)** is an ordinary `Choice` offered at the Buy decision, with a default that needs no search and a search only in the games where
  it can matter (`PlayerView::debt_decision_matters`); stated rules (`[[debt]]`) win over the search.

### Keeping `GameState` small

`GameState` is copied by every search node and bot rollout, and `MAX_PLAYERS` (6) `PlayerState`s are reserved in every game whatever the player count. Adventures added 6 KB and made whole games
18% slower until this was undone (`tests/state_size.rs` pins the sizes); the second speed pass (`docs/adventures-plan.md` section 6) made the player count matter less and the zones cheaper.

* **Copy with `GameState::dup()`, not `*state`, in anything hot** (search nodes, rollouts, the bots' own copies). The seats beyond the game's players are `MaybeUninit` (`state::Seats`) and `dup` leaves them
  out (`repr(C)` with `players` last: it copies a prefix of the struct): a 2-player game copies 9.8 KB of the 20.7 KB. `GameState::new` does not initialize them either. So reading a seat
  `>= num_players` is a bug (a `debug_assert` catches it; release reads uninitialized memory), and a per-player byte is paid once per player *in the game* on a `dup`, not six times.
  `state_size.rs` bounds the bytes a 2-player `dup` copies. Plain `*state` still works (it copies everything) and is what the cold code does.
* A zone that is empty in nearly every game (a mat, a set-aside area, a record of this turn's gains) is a `SmallMultiset<N>` (a few distinct cards, about 3N bytes), never a `Counts`
  (384 bytes). Pick `N` as the most distinct cards the zone can hold at once; adding past it aborts the game, so be generous but not wasteful (`MAT_CAP` 20, `TAVERN_CAP` 10, `WAR_CHEST_CAP` 24).
  Only the zones that every turn touches (hand, deck, discard, in play) are `Counts`.
* A stack that is nearly always short gets its own small capacity (`KnownStack<BOTTOM_CAP>` for the bottom of the deck, not `KNOWN_CAP`).
* Per-turn scratch state goes in `TurnState` (one copy), per-game constants in `GameState` (one copy); only genuinely per-player state belongs in `PlayerState`.
* Keep new fields out of the hot paths of games that do not use them: gate on a per-game mask or flag (`events`, `adv`, `in_supply` bits, `GAIN_LO`/`GAIN_HI`, `MAY_BUY_CARDS`) so other games pay one bit test.
* **`Counts` is not a plain array any more.** Its last four lanes are bookkeeping (a bitmap of the 32-lane blocks that may hold a card, and the total), so `total`/`is_empty` are a load and `iter`,
  `add_all` and `clear` visit only the blocks that have cards (a hand: two or three of twelve). Use its methods only (the field is private); a loop over a hand is cheap, a loop over every card id
  (`0..NUM_ZONE_IDS`) is not: walk `iter()` of the zones involved (`choose_discard`, `zone_of` and the evaluators did sort or scan all 370 ids on every call before the second speed pass). Card ids
  must stay below `LANES - 4`.
* **The search's table key walks the `Counts`.** `search::hash::turn_hash` hashes six `Counts` (about 55-60% of the time of a search-heavy game), so a wider `LANES` costs search bots far more than plain ones (`docs/card-id-capacity.md`
  sections 9-10 have the measurements and the staged plan: make `Counts::hash` independent of `LANES` before the first widening past 416). The key is also the transposition table's index: a speed change must not alter its value.
  A new per-player field that matters to the search goes into `finish_turn_hash`; the part before it (`chain_start`: `me`, hand, known and unknown deck) is what `HashCache` keeps between nodes, so anything added *there* must be added to the cache's comparison too
  (`tests/hash.rs` checks the cached hash against the plain one).
* **Measure with `bench/perf.sh`** (two builds interleaved, best of 5, outputs diffed: a speed change must leave every result identical). Build with `cargo build --release --workspace --bins --examples`: `--examples` alone does not rebuild
  `dominion-sim`, and a stale binary makes a before/after comparison meaningless. `tools/winprof` is a sampling profiler that works on Windows without admin rights.
* Loops over the supply (`phase_choices`' Buy list) go through masks over the card ids (`cards::buy_masks`: cards by cost, no-Potion cards) and `SupplyCards`, not one pile at a time.
* **Hidden state is state.** `PlayerView::stable_seed` hashes the whole effect stack array, popped frames beyond `len` included, and the bots' searches are seeded from it: a fast path that leaves a
  different stale frame (or any dead data that is hashed) than the step-by-step code changes bot play, whatever it does to the visible state. The cure is the check the speed passes use: the same seeds
  must give the same `match` output (md5 of the stats on several pairings, searched strategies and 3-6 players included) before and after.
* `GameState::phase_choices`, `Counts` and the Draw/cleanup steps are the hottest code: check the speed table in the plan doc after touching any of them.

## 2. Bots: `crates/sim/src/defaults/<set>.rs`

A unit struct implementing `SetDefaults` (fast defaults for mode cards, a card's own `Select` policy, overpay, naming,
start-of-turn order group, whether a card is worth playing, ...), a `mod` line and an arm in `with_set!` in
`defaults/mod.rs`. Also: new expression variables for strategy conditions in `expr.rs` (`coffers`-style), the set's
name in `kingdom.rs` and in the `--kingdom` help of `bin/dominion_sim.rs`. New strategy tables (`[[token]]`, `[[call]]`, `[[exchange]]`) are
parsed in `strategy.rs` and listed in the Lab's genome (`crates/evolve/src/genome.rs`, `space.rs`) when the Lab should be able to search them. A decision that shapes the turn (a call, an overpay) is
searched like the play order: add it to `eval::is_play_decision` and have a stated rule win over the search.

## 3. Front ends

* wasm (`crates/wasm/src/lib.rs`): the set's bit in the set mask (`sets_mask`) and the docs of `padded_kingdom`.
* Page: the checkbox in `web/template.html`, `SET_BITS` in `web/app.js`, drawing of any new zone or token, the card
  colours; off by default for a set the shipped strategies know nothing about. Then `build-web.ps1`.
* Lab: `ALL_SETS` and the checkbox in `crates/lab/web/lab.html`, the docs in `crates/lab/src/lib.rs`, the set-name lists in
  `crates/evolve` (`ga.rs`, `config.rs`).

## 4. Tests

* `crates/engine/tests/<set>_cards.rs`: one or more tests per card (the printed effect, the empty-deck / reshuffle /
  empty-pile edge cases, rulings), set-up and counts (`CardSet::ALL.len()`, kingdom counts in `cards::set_tests`,
  `NUM_CARDS` where a test pins it). Rules that need two sets go in `cross_set.rs` or `reactions.rs`.
* `crates/sim/tests/<set>_defaults.rs`: the bot defaults; `obvious_play.rs` checks every choice-free card against
  exact search (add the set's cards to its card lists); `shipped_strategies.rs` if a strategy ships.
* A sample strategy in `strategies/` and a line in the Lab if the set is meant to be searched; `web/test_log.mjs` for the page.

## 5. Checks before it is done

* `cargo test --release --workspace` and `cargo build --release --workspace --tests` warning-free.
* `powershell -File build-web.ps1`: every suite ALL PASSED.
* Speed did not move: `dominion-sim match strategies/big_money_ultimate.toml strategies/double_witch.toml --games 200000`
  (about 1.8M games/s since the speed pass of the Adventures plan, section 6; 1.76M with Empires) and one pairing that uses the new set; best of five, before and after. Dispatch by set is a
  `match` on the card's set (no indirect call); keep it that way.
* The plan doc's status line, and `CLAUDE.md` if the layout changed.

## 6. Learned with Nocturne

* **Widen before you run out.** Six type bits pushed `CardDef::types` and the flag constants from `u16` to `u32`; check the free bits (`cards.rs`) first.
  The ids moved too: Nocturne took 280-327 in the zone range, so Ruins/Knights, Events and Landmarks were renumbered (all by name in the code; only tests that pin
  `NUM_CARDS` or the last Landmark needed a change) and `Counts::LANES` went 320 -> 352, which alone costs 2.5% speed (`docs/card-id-capacity.md`).
* **A new phase must be gated.** `Phase::Night` is entered only when the game has Night cards (`NocState::flags`) and the player holds one; the Buy phase's end
  calls `night_follows()` first, so other games pay a byte test. Move the end-of-turn offers behind the new phase (they belong to Clean-up).
* **Decks with hidden order are masks plus chance.** Boons and Hexes store *which* cards are in the deck, never the order; the next one is drawn with the game RNG,
  or, in chance mode, returned as `Step::Chance` with the cards of the deck as outcome lanes of a temporary `Counts` (pseudo player ids 254/255), so the search,
  `determinize` and the plan/graph tools need only labels. Store a revealed-but-not-yet-applied card (`boon_top`) and clear it in `determinize`.
* **Pseudo-cards for rules objects.** Boons, Hexes and States have `CardDef`s (names for the log, strategies and Lab) after the Landmarks, outside every zone, `Counts` and supply mask.
* **`#[inline(never)]` the rare handlers.** The first version of the set cost 6.5%; marking the rarely used frame handlers and `end_night` `#[inline(never)]` recovered 2.5%
  of it (an inlined handler grows `run_frame` and costs the hot path registers). Measure after every new frame, not at the end.
* **Test with a conservation fuzz.** `nocturne_cards.rs` plays thousands of random games over random kingdoms, checking the card count and the piles after each step and asserting
  coverage (every card played, every Boon and Hex received). It found real bugs (Guardian's protection never ending; Urchin versus an Attack in the obvious-play rule).

## 7. Learned with Hinterlands

* **Renumbering is cheap when ids are used by name**: inserting 26 kingdom cards below the Ruins/Knights shifted every later id (Events, Landmarks, Boons...)
  with a byte-safe script; only tests pinning `NUM_CARDS` changed. Kingdom ids are no longer one range (`is_kingdom` excludes Nocturne's extras).
* **A discard is a trigger**: `G_DISCARD_HOOK` + `discard_hook` at every site that moves cards to the discard pile; reactions wait in `trash_queue`
  (`Q_DISCARDED`) until the selection finishes, so cards discarded together react after all are discarded.
* **Out-of-turn plays** (Guard Dog, a gained Berserker, Trail / Weaver on another player's turn): the reaction window and victims take the attacker as a
  parameter, and the played card is added to `discard_next_cleanup` so it leaves play at that turn's Clean-up.
* **Auto-single decisions hide tests**: a Select with one candidate is applied at once; tests should assert state, not expect the decision.
* Gains "less than this" are Gain frames with `less`; the decision shows `max_cost` = the card's cost (the page text reads the frame).

## 8. Learned with Rising Sun

* **A new pseudo-card kind (Prophecy)** needs: ids above `NUM_ZONE_IDS` guarded where a source id indexes a zone (`push_plays` skips `is_pseudo(source)`),
  a state gate byte so games without it pay one test, text round-trip (parse restores derived state directly), the view accessors, the wasm JSON and a
  panel in the web page, and exclusion from the evolved gainable pool.
* **Widening `Counts` / `SupplyMask`** grows every state copy (about 1 KB per 32 lanes): measure with the interleaved benchmark, and expect +-4% code-layout noise.
* **Test setup pitfalls**: after a terminal the engine enters Buy and auto-plays Treasures (use non-Treasure fillers); Debt without coins makes the Buy decision
  auto-pass; a state from `new_state` has not resolved its start of turn.
