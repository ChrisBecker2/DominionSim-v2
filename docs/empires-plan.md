# Empires and Change (Rising Sun): plan, rulings, and the Debt analysis

Status: Empires complete (every kingdom pile incl. the five split piles and the Castles, the 13 Events, the 21 Landmarks); Change is the
only Rising Sun card (it is here because it is the one card that makes Debt a real decision; the rest of Rising Sun is not planned yet).
Engine: `crates/engine/src/sets/empires.rs` and `rising_sun.rs`; bots: `crates/sim/src/defaults/empires.rs`, `strategy.rs`, `eval.rs`; tests:
`crates/engine/tests/empires_cards.rs` (75 tests incl. a random-game conservation test), `crates/sim/tests/empires_defaults.rs` (22),
Debt-cost tests at the end of `alchemy_cards.rs`, `obvious_play.rs`, `shipped_strategies.rs` (4 samples).

Sources: the wiki pages `Debt`, `Change`, `Cost`, `Split pile`, `Castles`, and the page of each card, Event and Landmark
(`C:\explore\wiki\dominion-wiki\pages\0\<Title>.json`; nothing from the wiki is copied into the repo).

## 1. What was added

| Area | Where |
|---|---|
| Card ids: the zone ids are 0..296 (Empires 233-295, Change 257), the pseudo-cards (33 Events, 21 Landmarks) are 296..350 and never sit in a zone or a supply mask | `cards.rs` (`NUM_ZONE_IDS`, `FIRST_EVENT`, `FIRST_LANDMARK`, `is_pseudo`) |
| A fourth cost part, `debt: u8` in `CardDef`; `cost_string` prints "$8 and 8 Debt" / "8 Debt" | `cards.rs` |
| Debt tokens per player (`PlayerState::debt`), `Choice::PayDebt` / `PayDebtKeep(n)`, buying a Debt-cost card or Event, Capital, Change, Tax, Wedding, Mountain Pass | `engine.rs`, `empires.rs` |
| VP tokens on Supply piles (`EmpState::piles`: Gathering cards Farmers' Market / Temple / Wild Hunt, Defiled Shrine, Aqueduct; Debt tokens: Tax) | `state.rs` (`PileTokens`) |
| Split piles (Settlers / Bustling Village, Catapult / Rocks, Encampment / Plunder, Gladiator / Fortune, Patrician / Emporium; Sauna / Avanto generalized) | `cards::SPLIT_PILES`, `pile_open`, `may_buy` |
| Castles: one pile of eight ids (Humble ... King's), the lowest non-empty one is on top; 8 cards for two players, 12 for more | `state.rs`, `cards::is_castle` |
| 13 Events, 21 Landmarks (end-of-game scoring ones in `landmark_score`, the token ones through `watch_gain`) | `empires.rs` |
| Bots: Debt payment (below), every new decision, `[[debt]]` rules, strategy conditions `debt`, `opponent_debt`, `debt_cost`, `pile_vp(Card)`, the pile `Castles`, a split pile named by either half or as `Settlers/Bustling Village` | `sim` |
| Lab: Empires and Rising Sun set checkboxes, Events and Landmarks listed with the kingdom cards (type `Event` / `Landmark`), random kingdoms draw 0 to 2 Events and Landmarks together, `[[debt]]` rules are searched when Change is in the pool | `lab`, `evolve` |
| Page: the sets' checkboxes (mask bits 512 and 1024), Debt in the turn bar and per player, the Archive, VP and Debt tokens on piles, both halves of a split pile, the Castles as one pile, the Landmarks with their VP | `web/`, `crates/wasm` |
| Text format (`text.rs`): `landmarks:`, `landmark vp:`, `pile vp:`, `pile debt:`, `obelisk:`, `pass: done`, per player `debt:`, `archive:`, `enchanted by:`, `donate:`; old saves still load | `text.rs` |

## 2. Rulings used (wiki source in brackets)

* Debt tokens prevent buying cards and Events (and nothing else); a card or Event with Debt in its cost gives those tokens when bought; Debt is removed at
  any time in the turn for $1 each, using neither a Buy nor an Action; it does not let Treasures be played at other times; no effect at the end of the game
  [Debt]. The Buy decision therefore offers `PayDebt` whenever the player has $ and Debt.
* A cost in Debt is orthogonal to $: "up to $4" never includes a card with Debt in its cost (Engineer, $0 and 4 Debt, is not up to $4), "$5 or more" is
  coins only (Fortune counts, Overlord does not), and a pure Debt cost has an implicit $0 [Debt, Cost].
* Change: with any Debt, +$3; otherwise trash a card from the hand and gain a card costing more $ than it, taking Debt equal to the difference in $
  (the costs are re-checked right before: a Wayfarer trashed and a Province gained is no Debt if both cost $8 at that moment; a gain that now costs less
  still takes Debt for the difference; no Debt if nothing was gained). Potion and Debt of the gained card are ignored in the comparison [Change].
* Capital: 6 Debt when discarded from play in Clean-up, once however many times it was played [Capital]. Fortune: doubles the $ once, played after the other
  Treasures; gaining it gives a Gold per Gladiator in play [Fortune].
* Split piles: five of each half; the bottom half is in the Supply with its own count but cannot be bought or gained while the top half is there; the pile
  is one pile for the three-pile ending and empty only when both halves are gone [Split pile].
* Castles: eight different cards in cost order, only the top one can be gained, the pile is one pile for the ending; with 3 or more players the pile has
  12 cards (two each of Humble, Small, Opulent and King's Castle, one each of the others; test `the_castles_are_eight_cards_in_cost_order...`) [Castles].
* Encampment: reveals a Gold or a Plunder or goes back to its pile; Patrician/Emporium: Patrician takes a revealed card costing $5 or more into the hand
  (coins only; a Debt-only card is not "$5 or more"); Emporium gives 2 VP tokens when gained with 5 Actions in play; Plunder gives a VP token each play.
* Gathering cards (Farmers' Market, Temple, Wild Hunt) put VP tokens on their pile and give them to whoever gains them / plays the "take" side.
* Landmarks: the six with tokens start with 6 per player; Aqueduct starts with 8 VP on the Silver and Gold piles, moves a token from a Treasure's pile onto itself when
  that Treasure is gained and gives all its tokens to the player who gains a Victory card; Defiled Shrine collects on the Action piles that are not Gathering piles and a bought Curse takes them; Mountain Pass starts a
  bidding (clockwise from the player to the left of the gainer) when the first Province is gained, the high bid is Debt and 8 VP.
* Events are bought with a Buy for their cost in $ (and Debt); Windfall needs an empty deck and discard pile, Ritual needs a Curse and a card in hand.
* Tax: one Debt on every Supply pile at the start; buying it adds 2 to a pile of the buyer's choice; gaining a card from a pile with Debt tokens takes them
  (only in the Buy phase).

## 3. The Debt analysis: when should Debt be paid?

The rules above fix most of it. Debt tokens cost nothing by themselves (no interest, no end-game effect); they only stop buying until paid, and paying
is $1 per token at any time. So the question is never "whether to ever pay" but "when, and with what".

**Why paying early is right.** Every $ spent on a card needs Debt to be 0 first, so all Debt must be paid before the first purchase of the turn; paying
after the purchases is possible only for Debt taken during the Buy phase (the Debt of the card or Event just bought, which is paid with what is left of the
$). Paying later in the turn than needed changes nothing about the money, and Debt left at the end of the turn costs the same $ next turn as it costs now:
a $ is a $ in whichever hand it appears, but a hand that has to pay Debt first is a hand that cannot buy with that $. Leaving Debt unpaid while having
spare $ is therefore never better than paying it, except where holding Debt itself is useful.

**Where Debt itself is useful: Change.** With Debt, Change gives +$3 (a Gold for the Action phase); without, it remodels with Debt equal to the $ difference.
One unpaid token turns every Change in the next hand into +$3, for the price of $1 later: a net +$2 for the hand (the token is itself paid with the first
$ of that hand, which is the $1). This is the only place in Empires + Rising Sun's Change where keeping Debt is a decision. The cases where "pay everything
as soon as there is $" is wrong, each with a test:

1. **Change in hand, Debt owed, and a remodel wanted**: the Debt must be paid in the Action phase *before* playing Change so that it remodels (a stated
   `[[debt]] pay = "all"` rule does this; the default search sees the line `PayDebt, Change` against `Change` for $3). Test
   `paying_debt_in_the_action_phase_lets_change_remodel` (engine), `in_a_game_with_change_a_stated_pay_all_rule_pays_in_the_action_phase...` (bots).
2. **A Change in the deck and leftover $ at the end of the turn**: the rules pay nothing automatically at the end of the turn in a game with Change (a
   player may want to keep one token); the Buy decision offers `PayDebt` (all), `PayDebtKeep(1)` (all but one) and `Pass`. Test
   `a_game_with_change_does_not_pay_the_debt_for_the_player_at_the_end_of_the_turn`. In a game without Change the leftover $ pay Debt automatically
   at the end of the turn (test `debt_is_paid_with_coins_at_any_buy_decision_and_leftover_coins_pay_it_at_the_end_of_the_turn`), which is the only sensible
   choice.
3. **Not enough $ to clear the Debt and nothing else to buy**: pay as far as the $ go (the default pays all it can; `debt_that_the_coins_cannot_cover_is_paid_as_far_as_it_goes`).
4. **Coffers**: a Coffers token is $1 for this purpose too, spent one at a time through the same decision; the default spends $ before Coffers. Test
   `coffers_pay_debt_one_token_at_a_time_through_the_coins`.
5. **Mountain Pass**: Debt taken voluntarily as a bid; it is the price of 8 VP, so the bots bid up to 5 tokens (about the worth of a Province's points in
   $ delayed), with a first bid of 2.

**Implemented policy.**

* Default (`Strategy::buy_decision`): at a Buy decision with Debt and $, pay all of it before anything is bought. No search.
* In a game with Change (`PlayerView::debt_decision_matters`), a Buy decision with `PayDebt` or `PayDebtKeep` among the choices is *searched* like a play
  decision (`eval::search_play_choice`) and the Action phase offers `PayDebt` too when a Change is in the hand.
* The search leaf values Debt as negative future money (`Evaluator::debt_value`): each token left is `vp_per_coin` less (nothing in the last turn); in a
  game with Change one token left is worth up to +$3 times the chance that a Change is in the next hand (5 cards out of the deck), so that keeping a token
  beats paying it only when the deck has Changes.
* Stated rules: `[[debt]] pay = "all" | "keep1" | "none"` with an optional `if` (the usual conditions, plus `debt`, `opponent_debt`); the first rule whose
  condition holds decides, before any search; the Lab searches such rules (at most 2) when the pool has Change.
* Never pay what the $ do not cover; never leave more than one token to keep (keeping 2 or more gains nothing: Change checks "any").

**What remains approximate.** The search sees only this turn plus the leaf term, so it cannot plan a deck of many Changes; a stated rule can. A better leaf
would model the expected number of Changes in the next hand exactly (it uses a linear chance capped at 1).

## 4. Bot defaults (every new decision has a test in `empires_defaults.rs` or `obvious_play.rs`)

Archive: an Action first when an Action is left to play, else the best Treasure; Crown: the Treasure with the most $; Change: trash junk first (every card
unlocks the best listed gain, Debt being the difference), else the card whose best unlock ranks highest; Gladiator: reveal the card the left player holds
fewest of; Annex: keep junk (Curses, Ruins, plain Victory cards) out of the shuffle; Advance: trash the cheapest Action when a listed Action costing up to $6
is worth more; Ritual: trash the card whose cost in VP tokens beats its points (a Province: 8 for 6), never one the strategy wants; Salt the Earth: the most
valuable Victory card; Small Castle: trash a cheap Castle in hand before itself; Opulent Castle: discard the plain Victory cards; Catapult / Sacrifice /
Temple: Rocks first (it pays a Silver); Wild Hunt: draw below 4 tokens on the pile and take the tokens from 4 (searched when search is on); Charm: +Buy and
$2 unless two cards of one cost are both wanted; Sprawling Castle: the Duchy; Tax: the pile the opponents own most of, not on the gain list; Mountain Pass:
a bid up to 5. Events are listed in `[[gain]]` like in Adventures; Castles, a split pile (either half, or both halves as `Settlers/Bustling Village`).
Strategies: `conquest_bm.toml` (~80% vs Big Money Ultimate), `forum_bm.toml` (~74%), `wild_hunt_bm.toml` (~70%), `castles_bm.toml` (~67%).

## 5. Known deviations and open points

* **Archive**: the cards set aside by several Archives are one pool (`ARCHIVE_CAP` 12) and each start-of-turn pick is made from the whole pool, not per Archive.
* **Mountain Pass bids** are naive (a fixed ceiling), not game-state aware.
* **Change in a game** turns off the end-of-turn automatic payment of Debt for everyone (the rule is the same for all players, but only Change decks care).
* **Tower** scoring does not look inside mixed piles (Knights, Ruins): it counts non-Victory cards of empty Supply piles by pile id.
* **Exact-cost gains on a Debt-cost trash** (`alchemy-plan.md` section 5b): the Debt part of the reference is not copied.
* **Footpad** is left out of the property tests of `obvious_play.rs`: a pre-existing search quirk with Familiar-like attacks, unchanged by this work (it
  also fails on the previous commit).
* Rising Sun other than Change is not implemented (25 kingdom cards, 10 Events, 15 Prophecies): see `docs/card-id-capacity.md` for the room left.

## 6. Performance

`dominion-sim match strategies/big_money_ultimate.toml strategies/double_witch.toml --games 200000` (best of three, same machine): 1.82M games/s before
this work, 1.76M after (-3%). `GameState` grew from 16.8 KB to 18.4 KB (`Counts` 288 to 320 bytes, `PlayerState` 2.2 KB to 2.4 KB; bounds in `state_size.rs`).
