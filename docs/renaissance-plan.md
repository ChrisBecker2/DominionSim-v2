# Renaissance

All 25 kingdom cards, the 5 Artifacts (Flag, Horn, Key, Lantern, Treasure Chest) and the 20 Projects, with Villagers, Artifacts held by one player at a time,
Projects bought like Events (a cube each, two per player), and the set-up rule that deals Projects into random kingdoms. Rulings are from the wiki copy
(`C:/explore/wiki`); tests are `crates/engine/tests/renaissance_cards.rs` (one per card, Artifact and Project, plus a random-game conservation fuzz) and
`crates/sim/tests/renaissance_defaults.rs`. Status: all cards, Artifacts and Projects are implemented; nothing is `todo`.

## 1. Ids and capacity

* The 25 kingdom cards are the new highest kingdom ids, 382..=406 (after Rice, before the Ruins and Knights, which moved up by 25 and stay the highest zone
  ids); `cards::LAST_KINGDOM` is Villain. Zone ids went 398 -> 423, so `Counts::LANES` went 416 -> 448 (ids below `LANES - 4`; 21 spare). `SupplyMask` is unchanged
  (four words, ids below 512: the 25 cards fit; Ducat is in `top`, the others in `ext`, so `GAIN_EXT` is a new constant of `SetRules`).
* Every pseudo id from the first Event moved up by 25; the 20 Projects (532..=551, `cards::FIRST_PROJECT`, in the order of the wiki's cost list) and the 5 Artifacts
  (552..=556, `FIRST_ARTIFACT`) are the highest ids. They are above 512, so `SupplyMask::has` answers false for them and `PlayerView::supply` returns 0.
  `cards::pseudo_cards_of(Renaissance)` are the Projects, so the Event dealer (`add_random_events`) deals Projects with Events and Landmarks: at most two of
  the three kinds together, as the rulebook recommends.

## 2. State

* `PlayerState`: `villagers: u16`, `projects: u32` (bit per Project, at most two set), `cargo` (Cargo Ship's set-aside cards, face up) and `research` (face down,
  pooled by `determinize`); both count as owned.
* `GameState::ren: RenState`: `projects` (the Projects of the game), `holder: [u8; 5]` (seat + 1 holding each Artifact), `plot` (tokens on Sinister Plot per seat),
  `flags` (`REN_FLEET_ROUND`) and `fleet` (the seats with a Fleet turn still to take), `hook` (what some player's Projects need the hot paths to look at: gains,
  trashes, shuffles, Capitalism's types; one byte test in a game without).
* `TurnState::ren: RenTurn`: `priests`, `cargo` (Cargo Ship plays that have not set a card aside), `innovation`, `acted` (an Action was played: Citadel's
  first), and the transient `no_chain` (the second Experiment) and `no_sewers`.
* Text format: `projects:` (the game's), `artifacts: Flag=1, Key=2` (holder seats), `fleet round:`; per player `villagers:`, `owned projects:`, `plot:`,
  `cargo:`, `research:`; the turn line adds `priests`, `cargo_uses`, `innovation`, `ren_acted` while nonzero.
* `PlayerView`: `villagers`, `villagers_of`, `projects`, `has_project`, `has_artifact`, `artifact_holder`, `plot_tokens_of`, `cargo_of`, `research`, `fleet_in_game`.

## 3. Mechanics

* **Villagers.** `Choice::Villager` in the Action phase *only when no Action is left* (spending one earlier is never better, so it is not offered, which keeps
  the search small): +1 Action. The phase asks for a decision when there are Villagers even with 0 Actions, as long as an Action card is in the hand.
* **Artifacts.** `take_artifact` takes it from whoever has it (`Event::Artifact`). The Flag adds a card to the Clean-up draw; the Key is a start-of-turn entry
  (+$1, a plain bonus like Fair and Barracks); the Treasure Chest gains a Gold at the start of every Buy phase (`enter_buy`); the Lantern makes Border Guard
  reveal 3; the Horn is an `offers_on_discard` offer (one Border Guard onto the deck, once per turn).
* **Projects.** Pseudo-cards bought in the Buy phase for the printed cost (never reduced) and a Buy (`buy_project`): `Choice::Card(project)` in the Buy decision,
  not while in Debt, at most two per player, not twice. The start-of-turn ones (Cathedral, City Gate, Silos, Sinister Plot, Crop Rotation, Piazza, Fair, Barracks)
  are `PendingDuration` entries named by the Project, so they join the `StartOrder` decision with the Durations (Cargo Ship and Research put cards into the hand
  and come first by default). Canal adds one to `cost_reduction` on its owner's turns (and at once when bought). Sewers is a queued trash trigger
  (`Q_SEWERS`, resolved with the other "when you trash" effects, so its own trash does not set it off: `no_sewers`); Priest's $2 is added in `trash_card`.
  Academy, Guildhall, Road Network and Innovation are gain triggers (gated by `RenState::hook`); Star Chart is a frame pushed by `take_top` when a
  shuffle is due (`STAR_STOP`): the owner picks a card of the discard pile to go on top and the rest is shuffled in (shuffles other than the draw's, such as
  Annex's, are not offered Star Chart: a gap). Capitalism makes the Actions whose text has a +$ amount Treasures on its owner's turns (`GameState::is_treasure`,
  the wiki's list of names in `capitalism_card`): in the Buy phase they are offered in `PlayTreasure` and played with all their effects (`plain_treasure` says a
  Treasure is played by the short path). Fleet: when the game would end (not the turn limit), each player with Fleet takes one more turn, in order from the player after
  the one whose turn ended it (`ren_game_over`); the extra turns set up then (Outpost, Seize the Day, ...) are taken first, and in between the Fleet turns, but none
  after the last one; the Fleet turns, being extra turns, do not count for the tie-break; `result_if_turn_ends` is `None` while Fleet is in the game.
* **Patron** gets +1 Coffers whenever a Patron is *revealed* in an Action phase: all 21 places that emit `Event::Reveal` go through `GameState::reveal_card`.
* **Cargo Ship** (a Duration that schedules itself, like Haven): each play lets one gained card be set aside (a `YesNo`, `Then::CargoAside`); the Ship stays in play
  only if it did, and the card comes into the hand at the start of the next turn (`PendingDuration::arg`). **Research** the same with the cards under it.

## 4. Cards

Mark: `free` = choice-free, `choice`, `order` = order-sensitive (see `CLAUDE.md`).

| Card | Cost | Mark | Notes |
|---|---|---|---|
| Border Guard | 2 | order | +1 Action; reveal 2 (3 with the Lantern), one to the hand, the rest discarded; all Actions: take the Lantern or Horn (with the Lantern, the Horn) |
| Ducat | 2 | free | Treasure: +1 Coffers +1 Buy; may trash a Copper from the hand when gained |
| Lackeys | 2 | free | +2 Cards; +2 Villagers when gained |
| Acting Troupe | 3 | free | +4 Villagers, trash this (the Villagers are given even when it is not in play to trash) |
| Cargo Ship | 3 | free | Duration, +$2, see above |
| Experiment | 3 | free | +2 Cards +1 Action, return to its pile; gains another, which does not chain (to the discard pile whatever the first one's destination) |
| Improve | 3 | free | +$2; at the start of Clean-up (once per play) may trash an Action that would be discarded (itself too) for a card costing exactly $1 more |
| Flag Bearer | 4 | free | +$2; take the Flag when gained or trashed (by whoever trashes it) |
| Hideout | 4 | order | +1 Card +2 Actions; trash a card, a Victory card gives a Curse |
| Inventor | 4 | choice | gain up to $4, then cards cost $1 less this turn |
| Mountain Village | 4 | order | +2 Actions; a card from the discard pile into the hand, or +1 Card if it is empty |
| Patron | 4 | free | +1 Villager +$2; reaction: +1 Coffers when revealed in an Action phase |
| Priest | 4 | order | +$2, trash a card, then each card trashed this turn is +$2 (Sewers' extra trash after the first does not count: it comes before the ability is set up) |
| Research | 4 | order | Duration, +1 Action; trash a card, set aside one card of the deck per $1 it costs, into the hand next turn |
| Silk Merchant | 4 | free | +2 Cards +1 Buy; +1 Coffers +1 Villager when gained or trashed |
| Old Witch | 5 | free | +3 Cards; each other player gains a Curse and may trash a Curse from the hand |
| Recruiter | 5 | order | +2 Cards, trash a card, +1 Villager per $1 it costs |
| Scepter | 5 | choice | Treasure-Command: +$2, or replay a non-Command Action played this turn that is still in play |
| Scholar | 5 | order | discard the hand, +7 Cards |
| Sculptor | 5 | choice | gain up to $4 to the hand; +1 Villager for a Treasure |
| Seer | 5 | order | +1 Card +1 Action; reveal 3, the ones costing $2-$4 to the hand, the rest back in any order |
| Spices | 5 | free | Treasure: $2 +1 Buy; +2 Coffers when gained |
| Swashbuckler | 5 | order | +3 Cards; with a card in the discard pile (after the draw) +1 Coffers, and with 4 or more Coffers the Treasure Chest |
| Treasurer | 5 | choice | +$3; trash a Treasure from the hand, or gain one from the trash to the hand, or take the Key |
| Villain | 5 | free | +2 Coffers; each other player with 5 or more cards discards one costing $2 or more (or reveals the hand) |

## 5. Bots (`crates/sim/src/defaults/renaissance.rs`, `strategy.rs`)

* Villagers: spent when no Action is left and the hand holds an Action worth playing (`Strategy::villager_choice`; the playout rules use it too).
* Which Project to buy is the gain list's job (`[[gain]] card = "Fair"`, with conditions); `GainListEvaluator` values a Project bought this turn as an Event is.
  A Villager banked is worth a little (0.05 on the preference scale); Coffers keep their value.
* Choices: Border Guard keeps the better card (Actions the strategy plays, then Treasures by $); Mountain Village and Star Chart take the best card; Hideout never
  trashes a Victory card while it has another; Silos discards every Copper; Crop Rotation a pure Victory card; City Gate topdecks the best card of the hand; Cargo
  Ship sets aside Treasures and Actions (not Coppers, Victory cards, Curses); Scepter replays the Action with the best draw/$/attack value; Treasurer trashes
  Coppers, regains a Gold, else takes the Key; Sinister Plot cashes in at 3-4 tokens (any near the end); the Lantern over the Horn.
* Expression variables: `villagers`, `opp_villagers`, `projects`, `has_project(Name)`, `has_artifact(Name)`, `plot_tokens`.
* Evolve: Projects of the game are gain-list entries capped with `bought(Name)` like Events (`is_bought_pseudo`); `EVENT_HINTS` has a placement for each of the 20; no new condition templates.
* Shipped strategies: `pageant_bm`, `silk_merchant_bm`, `seer_bm`, `canal_bm` (all beat Big Money Ultimate in `shipped_strategies.rs`).

## 6. Deferred / gaps

* Star Chart is not offered for the shuffles of Annex, Donate, Inn, Famine and the Boons' deck effects; only a draw's (and reveal's) reshuffle.
* Multi-Buy-phase turns (Villa, Cavalry, Continue): Exploration and Pageant look at the whole turn's gains and ask once at the end.
* A Throne Room that played a Cargo Ship stays in play with it only if a card was set aside while the Throne Room's plays were still unwinding.
