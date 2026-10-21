# Menagerie

All of Menagerie: the 30 kingdom cards, the Horse (a non-Supply pile of 30), the 20 Events, the 20 Ways and the Exile mat. Rulings are from the wiki copy
(`C:/explore/wiki`: each card's page, `Exile`, `Way`, `Horse`, `Menagerie (expansion)`; first edition, with the 2025 changes to Gamble, Reap and Way of
the Mouse). Tests: `crates/engine/tests/menagerie_cards.rs` (one or more per card, Event and Way, rulings, and a random-game conservation fuzz) and
`crates/sim/tests/menagerie_defaults.rs`. Status: everything below is implemented; the approximations are listed in section 7.

## 1. Ids and capacity

* The 30 kingdom cards are the new highest kingdom ids, 407..=436 (after Villain, in the wiki's cost order); `cards::LAST_KINGDOM` is Animal Fair. The Horse is 437
  (a zone id that is not a kingdom card). The Ruins and Knights moved up by 31 and stay the highest zone ids (438..=453): `NUM_ZONE_IDS` 423 -> 454,
  `Counts::LANES` 448 -> 480 (15 blocks; the block bitmap is still a `u16`; ids must stay below `LANES - 4`: 22 spare). `SupplyMask` is unchanged (four words, ids
  below 512; the Menagerie cards are in `ext`, so `GAIN_EXT` gets their bits).
* The 20 Events follow the Rising Sun Events (498..=517, `cards::NUM_EVENTS` 44 -> 64: the `events` masks are `u64`, now full), so the Event dealer, the Buy
  decision and `events_bought` treat them like any other Event. Everything above (Landmarks, Boons, Hexes, States, Prophecies, Projects, Artifacts) moved up by 51.
* The 20 Ways are the highest ids (608..=627, `cards::FIRST_WAY`), after the Artifacts: pseudo-cards that are never in a zone. They are above 512, so `SupplyMask::has`
  answers false for them. A Way in a kingdom list is a landscape like an Event (`cards::pseudo_cards_of(Menagerie)` are its Events and Ways: the dealer deals at most two
  landscapes of any kind, and at most one Way).
* The Horse pile lives in `GameState::supply` (30, a Counts lane) but not in `in_supply`: not buyable, not offered by any gain "from the Supply", not a pile for the
  game end, shown by the page as a non-Supply pile. `gain` / `return_to_pile` work on it unchanged.

## 2. State

* `PlayerState::men: MenPlayer`: `exile` (the mat: a `SmallMultiset<20>`, public, counted by `all_cards` / `count_of`, so it scores and conserves), `invest` (the copies
  on the mat that Invest put there: a subset), `trashed_last` (Goatherd: the cards the player trashed on their last turn), `gk` (Gatekeepers played: their Attack is on
  until the player's next turn), `flags` (Seize the Day used). Hashes as nothing while all zero.
* `GameState::men: MenState`: `ways` (bit per Way in the game), `mouse` (the card set aside for Way of the Mouse), `flags` / `hook` (which Menagerie effects are live in
  this game, so other games pay one byte test: costs that change, gains that are recorded, Goatherd's trash count, Exile non-empty, Invest, a Gatekeeper's Attack).
* `TurnState::men: MenTurn`: Kiln's pending gains, Livery's count, Way of the Seal, Snowy Village's cap on Actions, the cards gained this turn (their number for Destrier, their
  names for Commerce, the last for Wayfarer), the turn's trash count (Goatherd), Way of the Squirrel's draws by seat, the cards Way of the Frog will put onto the deck,
  Seize the Day, the cards Populate has gained from. Hashes as nothing while all zero.
* Cards set aside to be played at the start of the next turn (Delay, Reap, Way of the Turtle) are `PlayerState::summon_cards` (`SUMMON_CAP` 3 -> 6) with a start-of-turn
  entry per card, exactly as Summon's.
* Text format: `ways:` (the game's), `mouse:`, per player `exile:`, `invested:`, `trashed last:`, `seize:`; the turn line adds the Menagerie fields while nonzero.
* `PlayerView`: `exile_of`, `invested_of`, `ways`, `has_way`, `mouse_card`, `horses_left`, `trashed_last_of`.

## 3. Mechanics

* **Exile.** `Act::Exile` / `Zone::Exile`: cards on the mat are the player's. Exiling is not gaining or trashing (no triggers). Exiling from the Supply is a Gain frame with
  `GF_EXILE` (the pile's top card moves; a mixed pile shows the next). *Gain a card: you may discard all copies of it from the Exile mat* (all or none; a discard, so
  Tunnel and Village Green react): a `YesNo` pushed by every gain of a player whose mat holds a copy (`H_EXILE` hook), resolved with `Then::ExileDiscard`.
* **Ways.** In a game with a Way, playing an Action card (from the hand, by Throne Room, Mastermind, a reaction, at the start of a turn, from the discard pile by March...)
  first lets its player pick *the card's own effect or one of the Ways* (`DecisionKind::Way`, choices `Pass` = its own effect, `Card(way)`), after Kiln's gain (the rules put
  "first" abilities before the choice). The Attack reaction window belongs to the card's own effect, so it opens only if the card's own effect is chosen. The choice is a `FrameKind::PrePlay` frame pushed by `resolve_effects_tok` only when the game has a Way or a Kiln is pending: a game without pays one
  byte test. A Way replaces what the card does (its +Cards / +Actions / +Buys / +$, its play effect and the scheduling of a Duration's next-turn part, so a Duration only
  stays in play if one of its plays was for its own effect); the play itself is recorded as usual (played this turn, Champion, Adventures tokens). Text below the line
  stays (cards that act while in play do). Way of the Mouse's card is played like Prince's: not in play (`NIP`), and not through Way of the Mouse again.
* **Reactions to a gain** (Sleigh, Sheepdog, Falconer, Black Cat): `YesNo` frames pushed by `gain_triggers` for cards in the hand, played with the generic `Act::Play`.
  Sleigh discards itself, then a Mode frame picks hand or deck for the gained card, if it is still where it was gained (the stop-moving rule: `Frame::tok` = the gain's
  token). Gatekeeper's forced Exile is the last thing of the gain, so a Sleigh can save the card. A card played out of turn is discarded at that turn's Clean-up.
* **Black Cat** out of turn is an Attack played by a player who is not the turn's: the victims are every other player, in turn order from the turn's player.
* **Village Green, Barge**: "now or at the start of your next turn" is an `Ask` frame on play (they schedule their own Duration); Village Green is also a discard reaction
  (`G_DISCARD_HOOK`, played from the discard pile).
* **Cavalry** returns to the Action phase when gained in the Buy phase, like Villa. **Hostelry**, **Camel Train** have gain triggers. **Livery** adds to a per-turn count.
* **Costs that move** (Fisherman: -$3 with an empty discard pile on its owner's turn... on the turn player's turns; Destrier: -$1 per card the turn's player gained;
  Wayfarer: the cost of the last other card gained this turn): `GameState::cost` asks `men_cost` when `H_COST` is set (so the Buy decision stops using the printed-cost
  masks in such games, as with Peddler). Animal Fair's *trash an Action instead of paying* is `Choice::Alt(card)` at the Buy decision.
* **Snowy Village**: the Actions are capped (`MenTurn::snowy`): after it, +Actions (and spending Villagers) change nothing; the cap falls with each Action played.
* **Goatherd**: the player to the right's trashes on their last turn are counted at their Clean-up.
* **Seize the Day**: an extra turn like Outpost's, once per game, with no check on the previous turn; it does not count for the tie-break.

## 4. Cards

Mark: `free` = choice-free, `choice`, `order` = order-sensitive (see `CLAUDE.md`). Costs marked * change (section 3).

| Card | Cost | Types | Mark | Text and notes |
|---|---|---|---|---|
| Black Cat | 2 | Action-Attack-Reaction | free | +2 Cards; if it isn't your turn, each other player gains a Curse. Reaction: another player gains a Victory card: play it from your hand |
| Sleigh | 2 | Action-Reaction | choice | Gain 2 Horses. Reaction: when you gain a card, discard this to put it into your hand or onto your deck |
| Supplies | 2 | Treasure | free | $1; gain a Horse onto your deck |
| Camel Train | 3 | Action | choice | Exile a non-Victory card from the Supply. When gained: Exile a Gold from the Supply |
| Goatherd | 3 | Action | order | +1 Action; may trash a card from the hand; +1 Card per card the player to your right trashed on their last turn |
| Scrap | 3 | Action | order | Trash a card from the hand; choose a different thing per $1 it costs: +1 Card, +1 Action, +1 Buy, +$1, gain a Silver, gain a Horse |
| Sheepdog | 3 | Action-Reaction | free | +2 Cards. Reaction: when you gain a card, play this from your hand |
| Snowy Village | 3 | Action | choice | +1 Card +4 Actions +1 Buy; ignore any further +Actions this turn |
| Stockpile | 3 | Treasure | free | $3 +1 Buy; Exile this |
| Bounty Hunter | 4 | Action | order | +1 Action; Exile a card from the hand; if you had no copy of it in Exile, +$3 |
| Cardinal | 4 | Action-Attack | free | +$2; each other player reveals the top 2 cards, Exiles one costing $3 to $6, discards the rest |
| Cavalry | 4 | Action | choice | Gain 2 Horses. When gained: +2 Cards +1 Buy; in the Buy phase, back to the Action phase |
| Groom | 4 | Action | choice | Gain a card costing up to $4; Action: gain a Horse; Treasure: gain a Silver; Victory: +1 Card +1 Action |
| Hostelry | 4 | Action | free | +1 Card +2 Actions. When gained: discard any number of Treasures (revealed) to gain that many Horses |
| Village Green | 4 | Action-Duration-Reaction | choice | Now or at the start of your next turn: +1 Card +2 Actions. When discarded other than in Clean-up, may be played |
| Barge | 5 | Action-Duration | choice | Now or at the start of your next turn: +3 Cards +1 Buy |
| Coven | 5 | Action-Attack | free | +1 Action +$2; each other player Exiles a Curse from the Supply; if they can't, they discard their Exiled Curses |
| Displace | 5 | Action | order | Exile a card from the hand; gain a differently named card costing up to $2 more |
| Falconer | 5 | Action-Reaction | choice | Gain a card to the hand costing less than this. Reaction: any player gains a card with 2 or more types: play this from your hand |
| Gatekeeper | 5 | Action-Duration-Attack | free | At the start of your next turn +$3. Until then, an Action or Treasure another player gains that they have no Exiled copy of is Exiled |
| Hunting Lodge | 5 | Action | order | +1 Card +2 Actions; may discard the hand for +5 Cards |
| Kiln | 5 | Action | choice | +$2; the next time you play a card this turn, you may first gain a copy of it |
| Livery | 5 | Action | free | +$3; this turn, when you gain a card costing $4 or more, gain a Horse |
| Mastermind | 5 | Action-Duration | free | At the start of your next turn you may play an Action card from your hand three times |
| Paddock | 5 | Action | choice | +$2; gain 2 Horses; +1 Action per empty Supply pile |
| Sanctuary | 5 | Action | order | +1 Card +1 Action +1 Buy; may Exile a card from the hand |
| Fisherman | 5* | Action | free | +1 Card +1 Action +$1; on your turns, with an empty discard pile this costs $3 less |
| Destrier | 6* | Action | free | +2 Cards +1 Action; on your turns this costs $1 less per card you have gained this turn |
| Wayfarer | 6* | Action | choice | +3 Cards; may gain a Silver; costs the same as the last other card gained this turn |
| Animal Fair | 7* | Action | free | +$4; +1 Buy per empty Supply pile; instead of paying its cost you may trash an Action card from your hand |
| Horse | 3* | Action | free | +2 Cards +1 Action; return this to its pile (not in the Supply) |

Events: Delay ($0: may set aside an Action, played at the start of the next turn), Desperation ($0, once per turn: may gain a Curse for +1 Buy +$2), Gamble ($2: +1 Buy;
discard the top card, if an Action or Treasure may play it), Pursue ($2: +1 Buy; name a card, reveal 4, matches back in any order, the rest discarded), Ride ($2: a Horse),
Toil ($2: +1 Buy; may play an Action from the hand), Enhance ($3: may trash a non-Victory card for one costing up to $2 more), March ($3: may play an Action from the
discard pile), Transport ($3: Exile an Action from the Supply, or put an Exiled Action onto the deck), Banish ($4: Exile any number of cards with the same name from the
hand), Bargain ($4: gain a non-Victory card up to $5; each other player gains a Horse), Invest ($4: Exile an Action from the Supply; +2 Cards whenever another player gains or
Invests in a copy), Seize the Day ($4: once per game, an extra turn), Commerce ($5: a Gold per differently named card gained this turn), Demand ($5: a Horse and a card up
to $4, both onto the deck), Stampede ($5: with 5 or fewer cards in play, 5 Horses onto the deck), Reap ($7: gain a Gold, set it aside, play it at the start of the next turn),
Enclave ($8: gain a Gold; Exile a Duchy from the Supply), Alliance ($10: gain a Province, Duchy, Estate, Gold, Silver, Copper, in that order), Populate ($10: gain one card
from each Action Supply pile, in an order you choose).

Ways: Butterfly (may return this to its pile to gain a card costing exactly $1 more), Camel (Exile a Gold from the Supply), Chameleon (follow the card; +Cards and +$ swap),
Frog (+1 Action; put this onto the deck when discarded from play), Goat (trash a card from the hand), Horse (+2 Cards +1 Action, return this to its pile), Mole (+1 Action,
discard the hand, +3 Cards), Monkey (+1 Buy +$1), Mouse (play the set-aside card; set-up: set aside an unused non-Duration Action costing $2 or $3), Mule (+1 Action +$1),
Otter (+2 Cards), Owl (draw to 6), Ox (+2 Actions), Pig (+1 Card +1 Action), Rat (may discard a Treasure to gain a copy of this), Seal (+$1; this turn gained cards may be
put onto the deck), Sheep (+$2), Squirrel (+2 Cards at the end of the turn), Turtle (set this aside; play it at the start of the next turn), Worm (Exile an Estate from the Supply).

## 5. Bots (`crates/sim/src/defaults/menagerie.rs`, `strategy.rs`, `eval.rs`)

* **Ways.** `DecisionKind::Way` is decided by a stated rule first (`[[way]] card = "Smithy" way = "Way of the Sheep" if = "..."`: the first rule for the card, or `card = "any"`, whose
  condition holds; `way = "none"` plays the card's own effect), then by a fast default (`SetDefaults::way_choice`): each option gets a value (cards, Actions, $, Buys by the numbers,
  an Attack or any text a little; Mouse's card by its own), and a Way is used when it is worth clearly more than the card (a terminal drawn dead, a Treasure-less Silver...).
  A bot that searches (`search_play`) searches a Way decision only when the two values are close (`way_is_close`), so a game with a Way costs a search only where it matters.
* Exile: Bounty Hunter / Displace / Sanctuary / Banish exile Victory cards and Curses first (they stay yours, so only score cards are costless to lose), then a card with no copy
  in Exile for Bounty Hunter's $3 (a Copper, a Curse); Cardinal exiles the best of the two; Camel Train takes a Gold or a Silver... : see the file. Discarding from Exile on a gain: always,
  unless the cards on the mat are Victory cards the player wants to keep for scoring.
* Animal Fair is bought for its price when affordable, else by trashing the worst Action in the hand (a Horse, a dead terminal, a Curse-giving...), when the hand holds an Action
  the strategy would not play. Populate and Transport order their gains by the gain list. Village Green and Barge: now when the Action phase can use it, else next turn.
* Expression variables: `exile` (my Exile mat size), `exile(Name)`, `opp_exile`, `horses` (left in the pile), `has_way(Name)`. Evolve: Events are gain-list entries capped with `bought(Name)`
  (`EVENT_HINTS` placement for all 20); the Lab's genome can state `[[way]]` rules.
* Shipped strategies: `livery_bm` (~79% against Big Money Ultimate) and `cardinal_bm` (~54%), both checked in `shipped_strategies.rs`. A Sleigh-and-Horse Big Money was tried and loses (33%); Ways are stated in `[[way]]` rules by hand, the Lab genome does not search them.

## 6. Front ends

wasm (`sets_mask` bit, the Exile mats and Ways in the JSON, the Horse pile as a non-Supply pile), the page (set checkbox, Exile mat in the player box, the Horse pile in the
"not in the Supply" section, Ways in the landscape row, the log lines for Exile, Ways, Horses), the Lab (`ALL_SETS`), `build-web.ps1`.

## 7. Approximated or left out

* **Way of the Chameleon** converts the card's printed +Cards / +$, the $ the card's play makes at once, and the Draw frames the play pushes; effects that come after a
  later decision (Cellar's draws, a Priest's $2 per trash) are not converted.
* **Wayfarer** copies the $ part of the last gained card's cost only; a Potion or Debt part is not copied.
* **Populate** orders its gains with a decision per pile; it counts piles at the time it is bought.
* **Invest**: cards Exiled by Invest are told apart from other Exiled copies by count only (Transport and a gain's discard take the non-Invested first).
* **Way of the Mouse**: the set-aside card is chosen from the cards that need no other set-up (no Looter, Knight, Traveller, Heirloom or Shelter-style piles).
* Digital-only or later errata beyond the three listed in the Menagerie page's "Versions" section are not applied.
* **Kiln** is order-sensitive (a +Actions card played first is not copied), found by the cantrip-first test. **Populate** only offers piles it has not gained from, **Village Green**
  is played from a discard: both were found by tests, not by reading.
* The `obvious_play.rs` pool for Menagerie leaves out its Treasures (Stockpile exiles itself and gives +Buy, so with a Big Money rule on money in the deck drawing it can change what the
  bot buys), and its cantrip pool uses a seed where no draw-two cantrip with money to spare turns up against a lone terminal (cycling good cards is a tiny loss in the leaf value).
