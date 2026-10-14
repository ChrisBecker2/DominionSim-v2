# Rising Sun

All 25 kingdom cards (Change came with Empires), the 10 Events and the 15 Prophecies, with the Omen and Shadow card types, Sun tokens and the setup rule.
Rulings are from the wiki copy (`C:/explore/wiki`); tests in `crates/engine/tests/rising_sun_cards.rs` (one per card ruling) and
`crates/sim/tests/rising_sun_defaults.rs`.

## 1. Prophecies

* A Prophecy is a Landmark-like pseudo-card (name, cost 0, an id from `FIRST_PROPHECY`, never in a zone). A game has at most one, and only when the kingdom
  has an Omen (Kitsune, Tanuki... any card with the `OMEN` type); `cards::add_random_prophecy` picks one for random kingdoms. Strategies, kingdom lists and
  `--kingdom` may name it; the evolved side never gains it (`space.rs`).
* State is `GameState.rs` (`RsState`): `prophecy`, `suns` (Sun tokens left), `active`, a `hook` byte that gates the gain hooks so a game without a
  Prophecy pays one byte test, and `flags`. Each Omen played removes a Sun token; the last one makes the Prophecy active for the rest of the game.
  `PlayerView::sun_tokens()` / `prophecy()` / `prophecy_active()` are public information; expression variables `sun_tokens`, `prophecy_active` and
  `prophecy(Name)`.
* Start-of-turn effects (Biding Time, Kind Emperor, Rapid Expansion, Riverboat, Sickness) are `PendingDuration` entries named by the Prophecy's id, so they join
  the existing `StartOrder` decision; the bot groups are in `defaults/rising_sun.rs`.
* Others: River Shrine's cleanup gains through `offers_at_cleanup`; Continue through a guard frame under the gain; Asceticism through the Overpay frame; Daimyo
  through `turn.rs.daimyo` and replay frames; Flourishing Trade through `turn.cost_reduction` and the buy checks; Enlightenment through `is_action` and
  `filter_ok`; Divine Wind swaps the Kingdom piles with the game RNG (cards that need their own setup piles are not in the pool).
* One `aside` multiset per player serves Rapid Expansion (public) or Biding Time (private).

## 2. Shadow and Sun-token cards

Shadow cards sink to `deck_known_bottom` on a shuffle (`sink_shadows`) and may be played from the deck wherever they are (known stack, unknown, bottom); the
deck copy is preferred over a hand copy. `take_top` draws the bottom stack before reshuffling.

## 3. Capacity

`Counts` has 416 lanes (ids below `LANES - 4`), `SupplyMask`/`SupplyCards` four words, `NUM_EVENTS` 44, 15 Prophecies. `GameState` is about 22.5 KB
(2-player copy about 10.6 KB). See `docs/card-id-capacity.md` for the measured cost.

## 4. Bots

`defaults/rising_sun.rs`: Kitsune and Sickness options, Snake Witch and Gold Mine yes/no, Asceticism overpay (one $ per card the strategy wants trashed), start
order groups, worth-playing for the Debt cards late in the game and for Daimyo. `strategy.rs`: a Treasure played in the Action phase (Enlightenment) is worth it
for Coppers only. Evolve baselines have Event rules for the ten Events (`EVENT_HINTS`). Sample strategies (vs Big Money Ultimate, Village/Smithy/Market/Militia/
Moat/Cellar/Chapel/Workshop/Bandit + the card): `imperial_envoy_bm` 73%, `samurai_bm` 69%, `kitsune_bm` 68%, `credit_bm` 53% (with Gardens).

## 5. Deferred

* Sun tokens are valued only coarsely (section 6); a Prophecy's own effects on play are not modelled in the search beyond what the engine does while a line is searched.
* No Prophecy-specific evolved strategies yet; the Lab lists Prophecies as cards and keeps them out of the gainable pool.
* The recommended kingdoms with Riverboat, Ferryman-style extras (`riverboat`) are still skipped by `tools/gen_recommended.py`.

## 6. Valuing Sun tokens

`GainListEvaluator` (`crates/sim/src/eval.rs`, `sun_value`) adds a small term on the preference scale for the Sun tokens a line removes from the Prophecy this turn (an Omen played): each token is worth
`SUN_TOKEN = 0.4` times the Prophecy's *edge* for the player, a number from -1 to 1 (`prophecy_edge`): positive when the Prophecy, once in effect, helps the player more than the other players, negative when it
helps them more. The edge is read from the cards each player owns (public): 4 times the difference in the density of the card kind the Prophecy rewards (25 points of density is the whole range).

| Prophecy | Edge from |
|---|---|
| Approaching Army | more Attacks |
| Great Leader | more Actions |
| Enlightenment, Kind Emperor | more Actions (half weight) |
| Good Harvest, Growth | more Treasures |
| Panic | fewer Treasures (Treasures go back to their piles at Clean-up) |
| Biding Time | fewer dead cards (Curses, plain Victory cards: they would pile up in a carried-over hand) and more Artificers (a bigger hand to discard from) |
| Bureaucracy, Divine Wind, Flourishing Trade, Harsh Winter, Progress, Rapid Expansion, Sickness | 0: symmetric, or it depends on play the density cannot see |

A token is at most 0.4 of the preference scale: below a stated `[[play]]` rule (5 or more), and far below any difference between gain-list entries (1000 or more), so it only breaks ties between lines that gain
the same cards (play Kitsune now or hold it). A Prophecy that already happened has no tokens left, and a game without a Prophecy pays one comparison. Test: `rising_sun_defaults.rs`
(`removing_a_sun_token_is_worth_what_the_prophecy_does_for_the_player`).
