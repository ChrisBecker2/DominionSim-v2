# Nocturne: plan, rulings, and the design of Night, Boons, Hexes and Heirlooms

Status: complete. All 33 kingdom cards, the 7 Heirlooms, Will-o'-Wisp, Wish, Bat, Imp, Ghost and the three Zombies, the 12 Boons, the 12 Hexes, the 5 States
(Lost in the Woods, Deluded, Envious, Miserable, Twice Miserable) and the Night phase. Engine: `crates/engine/src/sets/nocturne.rs`; bots:
`crates/sim/src/defaults/nocturne.rs`, `strategy.rs`, `expr.rs`, `eval.rs`; tests: `crates/engine/tests/nocturne_cards.rs` (87 tests incl. a random-game
conservation test over 3000 games that plays every card, Boon and Hex), `crates/sim/tests/nocturne_defaults.rs` (8), the Nocturne samples in
`shipped_strategies.rs` (3), `obvious_play.rs` (now covers the Nocturne choice-free cards), `web/test_log.mjs` (a Nocturne section).

Sources: the wiki pages `Nocturne`, `Night`, `Boon`, `Hex`, `State`, `Heirloom`, `Exchange`, and the page of each card, Boon, Hex and State
(`C:\explore\wiki\dominion-wiki\pages\0\<Title>.json`; nothing from the wiki is copied into the repo). Card texts are the latest versions: Necromancer
(2021: turned face down), Crypt (does not set aside Durations), Tracker (announced "this turn" form).

## 1. What was added

| Area | Where |
|---|---|
| Card ids: Nocturne zone ids 280-327 (33 kingdom, 7 Heirlooms, 8 non-Supply), the Ruins/Knights moved up to 328-343, the Events 344-376, the Landmarks 377-397, then the pseudo-cards of this set: Boons 398-409, Hexes 410-421, States 422-426. `NUM_CARDS` 427, `NUM_ZONE_IDS` 344, `Counts::LANES` 352 | `sets/nocturne.rs`, `cards.rs` |
| Six new type bits (Night, Fate, Doom, Spirit, Zombie, Heirloom): `CardDef::types` and the flag constants are `u32` now | `cards.rs` |
| A new turn phase, `Phase::Night`, and `DecisionKind::PlayNight` | `engine.rs`, `state.rs` |
| Boons and Hexes: two decks of pseudo-cards as bit masks in `NocState`; the next one is drawn at random from the deck's cards (chance-aware) | `state.rs`, `nocturne.rs` |
| States: bits in `PlayerState::noc`, Lost in the Woods in `NocState::lost` | `state.rs`, `nocturne.rs` |
| Heirlooms in the starting decks (set-up), the Zombies in the trash, the piles outside the Supply (`NocState::piles`) | `nocturne::setup` |
| Four new frames (`ReceiveBoon`, `ReceiveHex`, `PickBoon`) and `Act::Return`, `Act::Archive`; new filters (`TreasureNot`, `CopyInAttackersPlay`, `NecroTarget`, `SharesTypeWith`) | `state.rs`, `effects.rs` |
| Bots: Night in `[[play]]` rules, the Night search, Boon picks, now-or-later, the Changeling exchange, conditions `night`, `boons_in_deck`, `boons_in_discard`, `hexes_in_deck`, `hexes_in_discard`, `has_state(..)`, `opp_has_state(..)`, `boon_in_discard(..)`, `boon_in_deck(..)`, `hex_in_discard(..)`, `hex_in_deck(..)`, `lost_in_the_woods`, `gains_this_turn`, `count_type(night|fate|doom|spirit|heirloom|duration)` | `sim` |
| Search: the Boon and Hex draws are chance steps (`BOON_CHANCE`, `HEX_CHANCE`), a Night phase is part of the turn the search values | `search` |
| Lab: the Nocturne checkbox; Night cards are in the `[[play]]` search space | `lab`, `evolve` |
| Page: the Nocturne checkbox (mask bit 2048), the Night phase in the log and the decisions, a Boons/Hexes/States panel, States and set-aside Hounds per player, chips for Night, Heirloom, Boon, Hex and State, the non-Supply piles marked | `web/`, `crates/wasm` |
| Text format: `boon deck:`, `boon discard:`, `druid boons:`, `boons held:`, `hex deck:`, `hex discard:`, `lost in the woods:`, side piles for the five Nocturne piles, per player `states:`, `guarded:`, `hounds:`, `boons later:`, phase `night`, turn fields `noc_gains`, `envy`, `deluded`, `tracker` | `text.rs` |

## 2. Design

### The Night phase

`Phase::Night` sits between Buy and Clean-up. It is entered only when the game can have a Night card in a hand (`N_NIGHT`: a Night card in the kingdom,
or Cemetery, whose Ghost is one) and the turn's player actually holds one, so a game without Night cards pays one byte test where the Buy phase ends,
and a player without a Night card in hand has no Night phase (no event, no decision). `end_buy_phase` runs the end-of-Buy hooks, then the Night phase,
then the end-of-turn offers (Treasury, Alchemist, Walled Village, Herbalist, Travellers) and Clean-up: those offers belong to Clean-up, which is after Night.

`DecisionKind::PlayNight` offers each Night card in the hand and `Pass` (no more Night cards this turn). Playing a Night card costs no Action; its effects
are resolved by the same machinery as an Action (`resolve_effects`), and Durations stay out (a Night-Duration is scheduled like any Duration). Werewolf is
an Action and a Night card: its `play` checks `phase == Night && player == turn player` for the Hex, else it draws 3 (so Scepter or a Throne Room in the
Action phase draw). Night cards gained by buying or another way come to the hand (Guardian, Ghost Town, Night Watchman, Den of Sin, in `finish_gain`),
so they can be played the turn they are bought.

Search: the turn's leaf is the start of Clean-up, so a Night phase is searched like the Action phase (the leaf test `search::is_leaf` does not stop at a
Buy phase with no Buys left while a Night card is in hand). **Obvious play** for Night: when every Night card in hand is choice-free (Guardian, Ghost Town,
Den of Sin, Cobbler, Raider, Werewolf) the order cannot matter and they are all played, lowest id first, without a search (`strategy::night_obvious`); Monastery,
Devil's Workshop, Changeling, Exorcist, Vampire, Bat, Night Watchman, Crypt and Ghost are searched (they read the gains of the turn, the hand, the deck or the cards in
play). `OnPlay` marking is required of Night cards too (the unit test in `cards.rs` checks `ACTION | TREASURE | NIGHT`). Cantrips-first and obvious play in
the Action phase also give up when an Urchin could meet an Attack (a bug that the new cards made the random test find: Sir Bailey before Hunting Grounds).

### Boons and Hexes: hidden order

The state stores which Boons are in the deck, the discard pile, set aside for Druid and held in front of a player until Clean-up (`boon_deck`, `boon_discard`,
`boon_druid`, `boon_held`: 12-bit masks), and the same for Hexes (`hex_deck`, `hex_discard`). The *order* of the deck is not stored: nobody knows it, the
next Boon is a uniformly random card of the deck (`GameState::draw_boon`, with the game's RNG, like the mixed piles of Dark Ages). When the deck is empty the
discard pile becomes the deck. Which Boons are in the deck is public (the discard pile is face up), so `PlayerView` exposes the masks and strategies can
count on them (`boon_in_discard(The Sea's Gift)`).

* **Search:** `GameState::determinize` replaces the RNG and clears any pending outcome, so no upcoming Boon or Hex leaks. In chance mode (the bots' search worlds) a
  draw returns `Run::Chance(BOON_CHANCE)` / `HEX_CHANCE` and `chance_outcomes` is a `Counts` with one count in lane `i` for each Boon `i` of the deck, so the existing
  chance machinery enumerates the outcomes and `resolve_chance` fixes the next draw (`boon_top`). The choices inside a Boon the receiving player makes are ordinary
  decisions (searched for the turn player); which Boon arrives is chance.
* **Simulate from a new game:** `redeal_mixed_piles` (the page's per-game redeal) now also redeals Druid's three Boons (`redeal_druid_boons`); everything else is drawn when needed from each game's own RNG.
* **Frames:** `ReceiveBoon` (draws if the Boon is unknown, optionally asks whether to receive it, receives it `count` times), `ReceiveHex` (draws the Hex once, then every
  player in a victim mask follows it in turn order), `PickBoon` (Druid's three, the Fool's three). A Boon or Hex is a pseudo-card with a `CardDef` so that the log, the
  strategies and the Lab name it; it is never in a zone.

### States

Per-player flags: Deluded and Envious are the two sides of one card (a player has at most one), Miserable and Twice Miserable the two sides of another. Deluded
and Envious are returned at the start of the Buy phase (`enter_buy`) and set `turn.noc.deluded` / `envy` for the rest of the turn (no Action can be bought; Silver
and Gold played in the Buy phase make $1). Miserable is -2 VP and Twice Miserable -4 VP in `PlayerState::vp`. Lost in the Woods is a single card (`NocState::lost`): the
Fool takes it from whoever has it, and its holder may discard a card at the start of each of their turns for a Boon (a start-of-turn entry, searched in the start order).

### Heirlooms

`nocturne::setup` replaces one starting Copper per Heirloom of a kingdom card (including the Bane card) in each player's *unknown* deck, so they are shuffled with the other
nine cards and appear in the opening hand at the right rate (tested: a Goat in the first hand of 5 in half of 4000 games). The Coppers they replace go to the Copper pile (`supply +
players x heirlooms`). Shelters and Heirlooms combine (6 Coppers, a Goat, 3 Shelters).

## 3. Rulings (wiki page in brackets)

* Night: any number of Night cards in the Night phase, after Buy and before Clean-up; no Action is used [Night]. A Night-Duration gained to hand is played the turn it is bought [Guardian, Ghost Town, Den of Sin].
* Boons are not cards: never in play, receiving one is not gaining a card. The deck is shuffled with the discard pile when empty; with no Boon anywhere none is received [Boon]. The Field's, Forest's and River's Gifts stay in front of the player until Clean-up (so they cannot come up again that turn) [Boon]. River's Gift draws after the next hand is drawn [River's Gift]. Druid receives one of its three and leaves it there, even one that is kept [Druid].
* Hexes: "each other player receives the next Hex" turns over one Hex that all follow, in turn order from the Doom card's player; a Moat/Guardian-protected player is not affected; if nobody is affected no Hex is turned over (the Hexes are an Attack's effect) [Hex].
* Pixie: the top Boon is discarded; the Pixie may then be trashed to receive it twice (a kept Boon counts twice) [Pixie]. Sacred Grove: the player receives the Boon, then each other player may receive it unless it gives +$1 (the Field's and Forest's Gifts) [Sacred Grove]. Blessed Village: the player sees the Boon taken when it is gained and chooses to receive it now or at the start of their next turn [Blessed Village].
* Fool: if the player has Lost in the Woods nothing happens; otherwise they take it (from another player if need be), take 3 Boons and receive them in the order they choose [Fool].
* Leprechaun: a Gold first, then the count of cards in play (Leprechaun itself counts); exactly 7 gives a Wish, else a Hex for its player [Leprechaun]. Tormentor: "other cards in play" excludes itself, including when it is not in play [Tormentor]. Cursed Village draws to 6 cards; a Hex when gained [Cursed Village].
* Monastery counts the cards its player gained this turn (an exchange is not a gain), and trashes that many cards from the hand or Coppers in play, in any mix [Monastery]. Devil's Workshop: 0 gains a Gold, 1 a card up to $4, 2 or more an Imp [Devil's Workshop]. This implementation lets the hand's trashes come first, then the Coppers.
* Changeling: the played Changeling trashes itself first (so it cannot copy itself), then gains a copy of a card in play (nothing if its pile is empty, not on top, or non-Supply). Gaining a card costing $3 or more while Changelings are in the Supply may be exchanged for one: the card goes back to its pile, the Changeling to the discard pile, the card's other gain effects still happen; the offer is asked before them [Changeling].
* Vampire: Hexes, gain a card up to $5 that is not a Vampire, exchange itself for a Bat (a Bat is not gained). Bat: trash up to 2; if one was trashed, exchange for a Vampire; if either pile is empty nothing is exchanged [Vampire, Bat, Exchange].
* Cemetery trashes up to 4 on gain; Haunted Mirror when trashed lets its owner discard an Action for a Ghost [Cemetery, Haunted Mirror]. Magic Lamp trashes itself with six single cards in play and gains 3 Wishes [Magic Lamp]. Pasture is worth 1 VP per Estate [Pasture]. Goat, Lucky Coin, Cursed Gold and Pouch as printed.
* Necromancer plays a non-Duration Action from the trash, once per card per turn (the card is turned face down for the turn: `TurnState::noc.necro`), leaving it there; Command cards may be played, and the played card is not "in play" [Necromancer].
* Crypt sets aside any number of non-Duration Treasures from play (the Archive zone is reused, one entry per Crypt with the number it holds); one comes to the hand at the start of each turn while any remain [Crypt]. With two Crypts the cards are pooled: the player takes one card per Crypt per turn from the pool (a simplification: the cards of different Crypts are interchangeable).
* Ghost reveals until an Action, sets it aside and plays it twice at the start of the next turn, staying in play with a Duration it plays; with no Action it is discarded that turn [Ghost]. Will-o'-Wisp takes the revealed card if it costs $2 or less (a Potion or Debt in the cost is more) [Will-o'-Wisp]. Wish returns to its pile to gain a card up to $6 to the hand only if it returned [Wish].
* Faithful Hound: when discarded other than in Clean-up (by Cellar, an Attack, Night Watchman, Famine ...) its owner may set it aside; it returns to the hand after the Clean-up draw of that turn, whoever's turn it is [Faithful Hound].
* Hexes: Famine reveals 3, discards the Actions, shuffles the rest into the deck (no shuffle of the discard pile when the deck is empty and nothing returns); Locusts trashes the top card; Copper or Estate gives a Curse, else the victim gains a cheaper card sharing a type (the `less` rule of Stonemason: no part of the cost larger, not equal; cards with Debt in the cost are never the cheaper card, a documented simplification); War trashes the first card costing $3 or $4 (not a Potion card); Bad Omens moves the deck to the discard pile and up to two Coppers back; Misery gives Miserable, then Twice Miserable, a third does nothing [Hex pages].
* Knights/Ruins returned by an exchange become the pile's top card (the old top card is forgotten and redrawn at random later).

## 4. Bot defaults

* Night: choice-free hands are played without a search; otherwise the turn is searched (`PlayNight` is a play decision). Without search (`search_play = false`, playouts): stated `[[play]]` rules (Night cards are named in the same list as Actions, with `night` as a condition if the rule is meant for the Night phase only), then the cards with no decision, then Devil's Workshop/Changeling/Vampire, then Monastery/Exorcist/Bat, then Night Watchman/Ghost. No separate `[[night]]` list: one rule list ranks everything a player may play, and Werewolf is both.
* Boons: the Fool's order is Field's, Sea's, Sun's, Moon's, River's, Wind's, Sky's, Earth's, Flame's, Mountain's, Swamp's, Forest's (actions and cards first, the ones that act on the hand later); Druid takes the best of its three by a fixed order (Earth's, Mountain's, Sky's, Swamp's, Flame's, Field's ...). Blessed Village: receive now unless the Boon wants a fresh hand (Earth's, Sea's, Wind's wait). Sacred Grove's shared Boon is always taken. Pixie is trashed unless the Boon is a Field's or Forest's Gift. Lost in the Woods discards a dead card (Curse, Ruins, a Victory card) or nothing.
* The Sky's Gift discards three cards when the Buy phase is over (the hand is spent) or three dead cards; the Earth's Gift discards the least valuable Treasure when the gain list wants a card up to $4; Secret Cave discards three only from a hand of dead cards.
* Changeling exchange: take it for a card low on the gain list (the bottom third) or unlisted, never a Victory card or a card in the top two thirds; a stated `[[exchange]] card = "Gold"` rule (any card costing $3 or more) wins either way. Changeling copies the card in play the gain list likes best.
* Crypt sets aside the Treasures worth $2 or more; Haunted Mirror discards the cheapest Action for the Ghost; Faithful Hound is set aside whenever it can be; Tracker topdecks every gained card that is not junk or Copper.
* Hex victims use the generic shapes (discard the worst card, topdeck the cheapest, gain the best of the cheaper cards by their own list).

## 5. Strategy syntax added

* `[[play]] card = "Werewolf"` (or any Night card) with an optional `if`; `night` is 1 in the Night phase.
* `[[exchange]] card = "<a card costing $3 or more>"` (Changeling: the card being given up) in addition to the Travellers.
* Conditions: `night`, `boons_in_deck`, `boons_in_discard`, `hexes_in_deck`, `hexes_in_discard`, `lost_in_the_woods`, `gains_this_turn`, `has_state(Deluded)`, `opp_has_state(Envious)`, `boon_in_discard(The Sea's Gift)`, `boon_in_deck(..)`, `hex_in_discard(War)`, `hex_in_deck(..)`, `count_type(night|fate|doom|spirit|heirloom|duration)`.
* The Lab searches Night cards in `[[play]]` rules. Not in the Lab's genome: `[[exchange]]` rules for Changeling and conditions on Boons and States (the genome's variable set is unchanged); both can be written by hand and shipped strategies may use them.

Samples (`strategies/`): `werewolf_bm.toml` (Night Hex + Smithy by day, ~69% vs Big Money Ultimate), `skulk_bm.toml` (Doom attacker, ~80%), `sacred_grove_bm.toml` (a Boon strategy, ~72%). Monastery, Druid and Leprechaun Big Money were tried and lose to Big Money Ultimate (about 28%, 13% and 26%): Monastery trashes only as many cards as gains, Druid's three Boons are fixed for the game, Leprechaun hexes its owner unless exactly seven cards are in play; they are not shipped.

## 6. Performance log

`dominion-sim match big_money_ultimate double_witch --games 200000` (best of 3-6 runs, games per second):

| Point | Games/s |
|---|---|
| Before (HEAD) | 1.77M |
| `Counts::LANES` 320 -> 352 only (needed for the 48 new zone ids) | 1.725M (-2.5%) |
| + the Nocturne code, first version | 1.655M |
| + `#[inline(never)]` on the rarely used frame handlers and `end_night` | 1.70M |
| Final | 1.70-1.72M (-3% in all: -2.5% the wider `Counts`, about -0.5% the Night gate and the other hooks, which are single byte tests: the gain hook, the hound check in `put`, envy in `play_free_treasure_from`, the Deluded filter in the Buy choices, the cleanup flag) |

Two things were learned: an unconditional call to a new `pub(crate)` function from a frame handler (`run_frame`) can be inlined and cost 4% by itself, so handlers for rare frames are marked `#[inline(never)]`; and the id budget (`docs/card-id-capacity.md`) is what costs speed, not the phase. A Nocturne game costs more because of the searches (Hexes and Boons are chance steps, a Night phase is part of the turn): Werewolf BM runs 1.2M games/s, Skulk BM 0.69M.

`GameState` grew 18.4 KB -> 19.55 KB (the 32 extra lanes are 0.9 KB; `NocState` 22 bytes, 4 bytes per player, `NocTurn` about 45 bytes); `tests/state_size.rs` bounds updated (GameState 19.7 KB, PlayerState 2.6 KB, TurnState 800 B, Counts 352 B).

## 7. Tests

* `nocturne_cards.rs`: set-up (ids, costs and types, Heirlooms in the deck and the Copper pile, the opening-hand rate, Shelters with Heirlooms, the piles, Boons/Hexes only with Fate/Doom, Druid's three and their redeal), the Night phase (follows Buy only with a Night card, gained to hand and played at once, Werewolf by day and night, Durations), each of the 12 Boons, the Boons deck (reshuffle, random draw covering all 12, chance mode, determinize does not leak, Druid, Pixie, Tracker, Sacred Grove, Idol, Blessed Village, Fool, Lost in the Woods), each of the 12 Hexes, Moat, Guardian, States (Deluded, Envious, Misery), and every kingdom card and Heirloom, Spirit, Wish and Zombie, the text round trip, the views.
* `nocturne_defaults.rs`: Night obvious play and rule order, `[[play]]` for Night cards, Boon now/later, Druid's pick, Lost in the Woods, the Changeling exchange and its `[[exchange]]` override, the new conditions, whole games on random Nocturne kingdoms with the shipped bots.
* `obvious_play.rs`: passes with the Nocturne cards in the pools (400 + 400 positions) after the Urchin rule.

## 8. Deferred

* Lab genome: `[[exchange]]` rules for Changeling and Boon/State conditions are not evolved. Night cards are.
* Crypt with two copies pools the set-aside cards (see rulings).
* Knights or Ruins returned by Changeling forget the old top card (see rulings).
* Possession-like effects do not exist in the engine, so none of Nocturne's rules about Possession (a Possessed Devil's Workshop) apply.
* Boons or Hexes "received" while not their player's turn only occur through Sacred Grove (other players' Boons) and Hex attacks; the Boon's effects use the turn player's counters (Field's Gift's +Action goes to the turn's actions), so a Boon that adds Actions or $ is not offered to the other players (Sacred Grove excludes them exactly as the cards do).
