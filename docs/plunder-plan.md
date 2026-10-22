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

Mark: `free` = choice-free, `choice`, `order` = order-sensitive (see `CLAUDE.md`). The text of each card is the comment on its line in `sets/plunder.rs`; the table is generated from it.
Cards that look choice-free but are not, found by `obvious_play.rs`'s Plunder pool (the shortcut "play everything / cantrips first" was worse than the exact search): Flagship
(which Action it replays depends on the order), Taskmaster, Harbor Village, Landing Party, Mining Road, Cabin Boy (the bonus is tied to a later gain or play), Swamp Shacks (it reads the
number of cards in play, and it also gives +Actions, so it wants to be late and early at once).

| Kind | Name | Cost | Types | Mark | Note |
|---|---|---|---|---|---|
| Card | Cage | 2 | TREASURE, DURATION | choice | sets aside up to 4 cards; waits for a Victory gain |
| Card | Grotto | 2 | ACTION, DURATION | order | sets aside up to 4 cards, discards them and draws as many next turn |
| Card | Jewelled Egg | 2 | TREASURE | free | a Loot when trashed |
| Card | Search | 2 | ACTION, DURATION | free | a Loot when a Supply pile empties |
| Card | Shaman | 2 | ACTION | order | may trash a card; every start of turn gains from the trash |
| Card | Secluded Shrine | 3 | ACTION, DURATION | free | trashes up to 2 when a Treasure is gained |
| Card | Siren | 3 | ACTION, DURATION, ATTACK | free | Curses; draws to 8 next turn; trashed when gained unless an Action is trashed |
| Card | Stowaway | 3 | ACTION, DURATION, REACTION | free | +2 Cards next turn; played when a Duration is gained |
| Card | Taskmaster | 3 | ACTION, DURATION | choice | repeats next turn if a $5 card was gained |
| Card | Abundance | 4 | TREASURE, DURATION | choice | +1 Buy +$3 when an Action is gained |
| Card | Cabin Boy | 4 | ACTION, DURATION | choice | next turn +$2 or trash it for a Duration |
| Card | Crucible | 4 | TREASURE | choice | trashes a card for $ equal to its cost |
| Card | Flagship | 4 | ACTION, DURATION, COMMAND | choice | replays the next non-Command Action |
| Card | Fortune Hunter | 4 | ACTION | order | may play a Treasure from the top 3 |
| Card | Gondola | 4 | TREASURE, DURATION | choice | $2 now or next turn; plays an Action when gained |
| Card | Harbor Village | 4 | ACTION | choice | +$1 after the next Action if it gave +$ |
| Card | Landing Party | 4 | ACTION, DURATION | choice | goes onto the deck when the first card of a turn is a Treasure |
| Card | Mapmaker | 4 | ACTION, REACTION | order | takes 2 of the top 4; played when a Victory card is gained |
| Card | Maroon | 4 | ACTION | order | trashes a card for 2 Cards per type |
| Card | Rope | 4 | TREASURE, DURATION | free | next turn +1 Card and a trash |
| Card | Swamp Shacks | 4 | ACTION | choice | +1 Card per 3 cards in play |
| Card | Tools | 4 | TREASURE | choice | gains a copy of a card anyone has in play |
| Card | Buried Treasure | 5 | TREASURE, DURATION | free | next turn +1 Buy +$3; played when gained |
| Card | Crew | 5 | ACTION, DURATION | free | goes onto the deck next turn |
| Card | Cutthroat | 5 | ACTION, DURATION, ATTACK | free | discard attack; a Loot when a $5 Treasure is gained |
| Card | Enlarge | 5 | ACTION, DURATION | order | remodels now and next turn |
| Card | Figurine | 5 | TREASURE | choice | may discard an Action for +1 Buy +$1 |
| Card | First Mate | 5 | ACTION | order | plays copies of an Action, draws to 6 |
| Card | Frigate | 5 | ACTION, DURATION, ATTACK | free | others discard to 4 after each Action they play |
| Card | Longship | 5 | ACTION, DURATION | free | +2 Cards next turn |
| Card | Mining Road | 5 | ACTION | choice | may play a gained Treasure |
| Card | Pendant | 5 | TREASURE | choice | $1 per differently named Treasure in play |
| Card | Pickaxe | 5 | TREASURE | choice | trashes a card; a Loot to the hand for $3 or more |
| Card | Pilgrim | 5 | ACTION | order | puts a card onto the deck |
| Card | Quartermaster | 5 | ACTION, DURATION | free | each turn gains a card aside or takes one into the hand |
| Card | Silver Mine | 5 | TREASURE | choice | gains a cheaper Treasure to the hand |
| Card | Trickster | 5 | ACTION, ATTACK | free | Curses; may keep a Treasure for the next hand |
| Card | Wealthy Village | 5 | ACTION | free | a Loot when gained with 3 differently named Treasures in play |
| Card | Sack of Loot | 6 | TREASURE | choice | gains a Loot |
| Card | King's Cache | 7 | TREASURE | choice | plays a Treasure three times |
| Card | Amphora | 7 | TREASURE, DURATION, LOOT | choice | +1 Buy +$3 now or next turn |
| Card | Doubloons | 7 | TREASURE, LOOT | free | a Gold when gained |
| Card | Endless Chalice | 7 | TREASURE, DURATION, LOOT | free | $1 +1 Buy every turn for the rest of the game |
| Card | Figurehead | 7 | TREASURE, DURATION, LOOT | free | +2 Cards next turn |
| Card | Hammer | 7 | TREASURE, LOOT | choice | gains a card costing up to $4 |
| Card | Insignia | 7 | TREASURE, LOOT | choice | gains may go onto the deck |
| Card | Jewels | 7 | TREASURE, DURATION, LOOT | free | goes to the bottom of the deck next turn |
| Card | Orb | 7 | TREASURE, LOOT | choice | plays a card from the discard pile, or +1 Buy +$3 |
| Card | Prize Goat | 7 | TREASURE, LOOT | choice | may trash a card |
| Card | Puzzle Box | 7 | TREASURE, LOOT | choice | keeps a card for the next hand |
| Card | Sextant | 7 | TREASURE, LOOT | choice | sifts the top 5 |
| Card | Shield | 7 | TREASURE, REACTION, LOOT | free | blocks an Attack |
| Card | Spell Scroll | 7 | ACTION, TREASURE, LOOT | choice | trashes itself to gain (and play) a cheaper card |
| Card | Staff | 7 | TREASURE, LOOT | choice | may play an Action |
| Card | Sword | 7 | TREASURE, ATTACK, LOOT | choice | others discard down to 4 (an Attack: played with its effects, not by the short path) |
| Event | Bury | 1 |  |  |  |
| Event | Avoid | 2 |  |  |  |
| Event | Deliver | 2 |  |  |  |
| Event | Peril | 2 |  |  |  |
| Event | Rush | 2 |  |  |  |
| Event | Foray | 3 |  |  |  |
| Event | Launch | 3 |  |  |  |
| Event | Mirror | 3 |  |  |  |
| Event | Prepare | 3 |  |  |  |
| Event | Scrounge | 3 |  |  |  |
| Event | Journey | 4 |  |  |  |
| Event | Maelstrom | 4 |  |  |  |
| Event | Looting | 6 |  |  |  |
| Event | Invasion | 10 |  |  |  |
| Event | Prosper | 10 |  |  |  |
| Trait | Cheap |  |  |  |  |
| Trait | Cursed |  |  |  |  |
| Trait | Fated |  |  |  |  |
| Trait | Fawning |  |  |  |  |
| Trait | Friendly |  |  |  |  |
| Trait | Hasty |  |  |  |  |
| Trait | Inherited |  |  |  |  |
| Trait | Inspiring |  |  |  |  |
| Trait | Nearby |  |  |  |  |
| Trait | Patient |  |  |  |  |
| Trait | Pious |  |  |  |  |
| Trait | Reckless |  |  |  |  |
| Trait | Rich |  |  |  |  |
| Trait | Shy |  |  |  |  |
| Trait | Tireless |  |  |  |  |

Events (cost): Bury 1 (+1 Buy, a card from the discard pile to the bottom of the deck), Avoid 2 (+1 Buy, up to 3 cards stay out of the next shuffle), Deliver 2 (+1 Buy, cards gained
this turn are set aside and come to the hand at the end of the turn), Peril 2 (may trash an Action from the hand for a Loot), Rush 2 (+1 Buy, the next Action gained this turn is played),
Foray 3 (discard 3 cards: three different names gain a Loot), Launch 3 (once a turn: +1 Card +1 Action +1 Buy and back to the Action phase), Mirror 3 (+1 Buy, the next Action gained is
gained again), Prepare 3 (the hand is set aside; at the start of next turn play its Actions and Treasures in any order, discard the rest), Scrounge 3 (trash a card from the hand or take an
Estate from the trash; gain a card up to $5), Journey 4 (an extra turn, the cards in play stay; never a third turn in a row), Maelstrom 4 (trash 3 from the hand; the others with 5 or more cards
trash one), Looting 6 (gain a Loot), Invasion 10 (play an Attack from the hand, gain a Duchy, an Action onto the deck and a Loot, which is played), Prosper 10 (gain a Loot and
differently named Treasures).

Traits (on one Action or Treasure pile): Cheap (costs $1 less), Cursed (a gain also gains a Loot and a Curse), Fated (put on top or bottom when shuffling), Fawning (gaining a Province
gains one), Friendly (may discard one at Clean-up to gain another), Hasty (set aside when gained, played at the start of the next turn), Inherited (replaces a starting card),
Inspiring (play another Action with no copy in play after playing one), Nearby (a gain gives +1 Buy), Patient (may be set aside at Clean-up, played next turn), Pious (a gain may
trash a card from the hand), Reckless (played twice, back to its pile when discarded from play), Rich (a gain gains a Silver), Shy (may be discarded at the start of the turn for +2
Cards), Tireless (goes onto the deck when discarded from play).

## 6. Bots

`crates/sim/src/defaults/plunder.rs`: Inherited replaces a starting Estate; Cage, Grotto and Avoid set aside dead cards (Curses, Victory cards, then Coppers) first; Fated's
bottom-of-the-deck picks are declined; Pendant is played after the other Treasures (`plays_last_rank`). Everything else (trashes, discards, gains, modes) follows the generic policy
and the strategy's own gain list: Events are bought from the gain list like any Event (`bought(Event)` in conditions). Not done: Lab genome hints for the Plunder Events, expression
variables for the Loot pile and the Traits.

## 7. Front ends

wasm (`sets_mask` bit, the Loot pile and Traits in the JSON), the page (set checkbox, the Loot pile in "not in the Supply", Traits on their pile and in the landscape row, log lines),
the Lab (`ALL_SETS`), `build-web.ps1`.

## 8. Measurements

(filled in at the end)

## 9. Approximated or left out

* Taskmaster counts only the $5 cards gained after it was played this turn (the card says "this turn"; a $5 gain in the Action phase before the play is not counted).
* Pile-emptying checks for Search / Cutthroat-like "when a pile empties" triggers run from the gain triggers; piles emptied by a trash (Lurker, Gladiator) or an Exile from the Supply
  do not wake Search (`plu_check_empty` is not called there).
* Inherited asks for the starting card only (Copper or Estate); a split-pile Inherited is not offered.
* The Lab's `/api/cards` lists the Events of Plunder but not its Traits; a Lab kingdom cannot name a Trait (the CLI and `GameConfig::trait_piles` can).
* The shortcuts for Plunder cards are narrower than the card text would allow: seven Actions are `choice` rather than `free` (section 5), so a bot with them in hand searches.
