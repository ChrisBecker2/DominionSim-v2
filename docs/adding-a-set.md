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

## 2. Bots: `crates/sim/src/defaults/<set>.rs`

A unit struct implementing `SetDefaults` (fast defaults for mode cards, a card's own `Select` policy, overpay, naming,
start-of-turn order group, whether a card is worth playing, ...), a `mod` line and an arm in `with_set!` in
`defaults/mod.rs`. Also: new expression variables for strategy conditions in `expr.rs` (`coffers`-style), the set's
name in `kingdom.rs` and in the `--kingdom` help of `bin/dominion_sim.rs`.

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
  (about 1.7M games/s) and one pairing that uses the new set; best of five, before and after. Dispatch by set is a
  `match` on the card's set (no indirect call); keep it that way.
* The plan doc's status line, and `CLAUDE.md` if the layout changed.
