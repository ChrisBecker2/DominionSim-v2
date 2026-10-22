# Plunder

All of Plunder: the 40 kingdom cards, the 30 Loot cards (15 different, two of each: a pile outside the Supply, shuffled), the 15 Events and the 15 Traits. Rulings are from the
wiki copy (`C:/explore/wiki`: each card's page, `Loot`, `Trait`, `Plunder (expansion)`; first edition, with the 2023 errata of Journey: never more than two turns in a row).
Tests: `crates/engine/tests/plunder_cards.rs` and `crates/sim/tests/plunder_defaults.rs`. Status: see section 9.

## 1. Capacity (stage 1, done before any Plunder card)

* `Counts::LANES` 512 -> 576: 18 blocks of 32, a `u32` bitmap of the blocks that may hold a card (the `u16` one was full at 16 blocks) and six bookkeeping lanes (the bitmap, then the `u16`
  total): card ids must stay below 570. `Counts::hash` is unchanged (the 13 legacy blocks, then the blocks past them that hold a card, each with its index).
* `SupplyMask` has a fifth word, `ext2` (ids 512-639); `SupplyCards`, `any`, `buy_masks` and `empty_piles` take it.
* The Events are a 128-bit set (`state::EvMask`: two `u64`s that hash as the old `u64` while the second is zero): `events`, `events_bought`, `AdvTurn::bought`. `NUM_EVENTS` is 64 + 15.
* `sim::batch::PROFILE_IDS` follows `NUM_ZONE_IDS`.
* `_pad` re-tuned (section 8).

## 2. Ids

* The 40 kingdom cards take the next kingdom ids after Lich: 487..=526, in the wiki's cost order (`cards::LAST_KINGDOM` is King's Cache). The 15 Loot cards follow (527..=541; zone
  ids that are not kingdom cards, `cards::FIRST_LOOT`, `is_loot`). The Ruins and Knights move up by 55 and stay the highest zone ids (542..=557): `NUM_ZONE_IDS` 503 -> 558.
* The 15 Events follow the Menagerie Events (`cards::NUM_EVENTS` 79). Everything above them (Landmarks, Boons, Hexes, States, Prophecies, Projects, Artifacts, Ways, the Exile label, the
  Allies) moved up by 70; the 15 Traits are the highest ids (`cards::FIRST_TRAIT`, `is_trait`, `NUM_TRAITS` 15), pseudo-cards that are never in a zone. `NUM_CARDS` 701 -> 786.
* The Loot type is a type bit (`cards::LOOT`, bit 25).

## 3. State

* `GameState::plu: PluState`: `loot` (the cards left in the Loot pile: a count per Loot, 0..=2, no order is stored), `loot_top` (the Loot just drawn by a chance outcome), `traits` (bit per
  Trait of the game) with `trait_pile` (the Supply pile each sits on, by `cards::pile_of`), and `flags` / `live` (which Plunder effects are live, so other games pay one byte test).
* `PlayerState::plu: PluPlayer`: `wait` (the "next time" Durations in play, see 4), `z` (a small multiset keyed by card + 1024 * zone code: the cards set aside on a Cage / Grotto /
  Quartermaster, the cards put into the hand or onto the deck at the end of the turn, Prepare's hand) and the holders in use. Counted by `all_cards` / `count_of`, public or private as
  the cards are face up or down (`determinize` pools the private ones). Hashes as nothing while all zero.
* `TurnState::plu: PluTurn`: Rush / Mirror / Avoid / Mining Road / Insignia counters, Deliver, Launch and Journey flags, the Harbor Villages waiting for the next Action, Taskmasters watching for
  a $5 gain, Landing Party's "first card of the turn", Trickster's count, the Frigate hit list, Prosper's names. Hashes as nothing while all zero.
* The text format has `loot:`, `traits:` (`Cheap on Village, ...`), per player `waiting:` and `plunder aside:`, and the turn line's Plunder fields while nonzero.
* `PlayerView`: `loot_left()`, `trait_piles()`, `trait_on(card)`, `waiting_of(p)`, `aside_of(p)` (the public part), `aside_count_of(p)`.

## 4. Mechanics

* **Loot.** `FrameKind::Loot` gains the top card of the pile: the card is drawn when the frame runs (the game RNG, weighted by the copies left), or, in chance mode, as a
  `Step::Chance` of the pseudo player `LOOT_CHANCE` (the outcomes are the Loot cards left, lane = index, weight = copies). A Loot comes into the discard pile (or the frame's `dest`) as
  a gain from outside the Supply (`gain_side` machinery: no pile is emptied, Search does not see it); a Loot returned to its pile (Changeling) goes back among the cards left. An
  empty pile gains nothing. Loot is a Treasure costing $7* and cannot be bought.
* **"Next time" Durations** (Cage, Search, Secluded Shrine, Abundance, Cutthroat, Flagship, Landing Party): the card is in play and in `PlayerState::stays` (it is not discarded
  at Clean-up) and has an entry in `PluPlayer::wait` (card, the Throne Room / King's Cache-like card that played it, how many plays). The entry fires on its event (a gain, a pile
  emptying, a play, a first play of a turn): the plays' effects happen `times` times, the entry goes, and the card (and its multiplier) is discarded in that turn's Clean-up
  (`discard_next_cleanup` when it is not its owner's turn). Waiting entries of different players fire in turn order. `PlunderState::live` says one is waiting, so ordinary gains
  pay one byte test.
* **Durations that are Treasures** (Rope, Gondola, Buried Treasure, Abundance, Amphora, Endless Chalice, Figurehead, Jewels, Cage): the Astrolabe / Contract paths. "Now or next turn"
  (Gondola, Amphora) is Menagerie's `Then::NowOrNext`. Endless Chalice and Quartermaster re-schedule themselves each turn and stay in play for good (`stays`).
* **Traits** sit on a Supply pile (random among the Action and Treasure kingdom piles, or named in `GameConfig::trait_piles`): `GameState::trait_of(card)` is one mask test. Each
  trait hooks where its rule happens: Cheap in `cost`, Cursed / Fawning / Hasty / Nearby / Pious / Rich in the gain triggers, Fated and Avoid at the shuffle (`take_top`, as Order of
  Masons), Friendly / Patient / Tireless / Reckless at Clean-up, Inspiring after a play, Shy at the start of the turn, Inherited in the set-up.
* **Reckless** plays the card's effects twice (the Daimyo replay frames) and returns it to its pile when it is discarded from play.
* **Set-up.** A kingdom with a Trait puts it on a pile chosen from the seed; Inherited then asks each player in turn which starting card the Inherited card replaces. A kingdom with a
  Loot gainer (or a Trait / Event that gains Loot) shuffles the Loot pile (a mask, no order). The Shaman's start-of-turn gain is live for every player in a game with a Shaman.
* **Events** are bought as every Event is; their effects are frames and per-turn counters in `PluTurn`.

## 5. Cards

Mark: `free` = choice-free, `choice`, `order` = order-sensitive (see `CLAUDE.md`).

(The table is filled in with the cards as they are implemented: see `sets/plunder.rs`, where each card's definition has a comment with its text.)

## 6. Bots

(see `crates/sim/src/defaults/plunder.rs`)

## 7. Front ends

wasm (`sets_mask` bit, the Loot pile and Traits in the JSON), the page (set checkbox, the Loot pile in "not in the Supply", Traits on their pile and in the landscape row, log lines),
the Lab (`ALL_SETS`), `build-web.ps1`.

## 8. Measurements

(filled in at the end)

## 9. Approximated or left out

(filled in at the end)
