# Plan: Dark Ages

Dark Ages is a set of its own (`CardSet::DarkAges`; page checkbox "Dark Ages", wasm set-mask bit 128, Lab
set name "Dark Ages"; off by default, like Alchemy, Promo and Cornucopia & Guilds: the shipped strategies know
nothing about Ruins or Knights). 35 kingdom piles (34 single cards and the Knights pile of ten different Knights),
and the non-kingdom cards: three Shelters, the Ruins (five kinds), Spoils, Madman and Mercenary.

> **Status: implemented** (all 35 kingdom piles, the Shelters, Ruins, Spoils, Madman and Mercenary, the mixed-pile and trash
> machinery, bots, page, Lab, three sample strategies). Card tests: `crates/engine/tests/dark_ages_cards.rs`; bots:
> `crates/sim/tests/dark_ages_defaults.rs`; Lab: `crates/evolve/tests/knights_rules.rs` and `ga_smoke.rs`; page: `web/test_log.mjs`.
> Samples: `strategies/knights_rebuild_cultist.toml`, `junk_dealer_count.toml`, `marauder_ruins.toml`.
>
> Card text and rulings are from the DominionStrategy wiki's pages for the cards and mechanics (named per ruling
> below), using the current text (the 2017 rulebook, Storeroom's "draw" wording) and the newest FAQ. The tests are the
> specification. Nothing in the Dark Ages box is left out; the cross-set pieces (Ambassador's return to the Supply,
> Possession, the Black Market, Trade Route and Embargo tokens) are not in the engine, so their Dark Ages notes do not
> apply.

## 1. Cards

C/F/O = the `OnPlay` marking (free = `choice_free`, choice = `has_choice`, order = `order_sensitive`).

| Cost | Card | Types | Text (summary) | Marking |
|---|---|---|---|---|
| $1 | Poor House | Action | +$4. Reveal your hand: -$1 per Treasure in it (not below $0). | order (cards drawn first add Treasures) |
| $2 | Beggar | Action-Reaction | Gain 3 Coppers to your hand. When another player plays an Attack, you may first discard this to gain 2 Silvers, one onto your deck. | choice |
| $2 | Squire | Action | +$1. Choose one: +2 Actions; +2 Buys; gain a Silver. When you trash this, gain an Attack card (from the Supply). | choice |
| $2 | Vagrant | Action | +1 Card +1 Action. Reveal the top card: a Curse, Ruins, Shelter or Victory card goes to your hand. | order (known top) |
| $3 | Forager | Action | +1 Action +1 Buy. Trash a card from your hand, then +$1 per differently named Treasure in the trash. | order |
| $3 | Hermit | Action | Look through your discard pile. You may trash a non-Treasure from it or from your hand. Gain a card costing up to $3. At the end of your Buy phase, if you gained no cards in it, exchange this for a Madman. | order |
| $3 | Market Square | Action-Reaction | +1 Card +1 Action +1 Buy. When one of your cards is trashed, you may discard this from your hand to gain a Gold. | order (kept in the hand it is a reaction to a trash: playing it first can lose the Gold) |
| $3 | Sage | Action | +1 Action. Reveal cards until one costing $3 or more; it goes to your hand, discard the rest. | order |
| $3 | Storeroom | Action | +1 Buy. Discard any number of cards, then draw that many. Then discard any number of cards for +$1 each. | order |
| $3 | Urchin | Action-Attack | +1 Card +1 Action. Each other player discards down to 4. When you play another Attack with this in play, you may first trash this to gain a Mercenary. | order (it can be the trashed card) |
| $4 | Armory | Action | Gain a card costing up to $4 onto your deck. | order (a later draw takes it) |
| $4 | Death Cart | Action-Looter | You may trash this or an Action card from your hand, for +$5. When you gain this, gain 2 Ruins. | order |
| $4 | Feodum | Victory | 1 VP per 3 Silvers you have (round down). When you trash this, gain 3 Silvers. | n/a |
| $4 | Fortress | Action | +1 Card +2 Actions. When you trash this, put it into your hand. | free |
| $4 | Ironmonger | Action | +1 Card +1 Action. Reveal the top card; you may discard it. If it is an Action +1 Action; a Treasure +$1; a Victory card +1 Card. | order |
| $4 | Marauder | Action-Attack-Looter | Gain a Spoils. Each other player gains a Ruins. | free |
| $4 | Procession | Action | You may play a non-Duration Action from your hand twice. Trash it. Gain an Action costing exactly $1 more. | order |
| $4 | Rats | Action | +1 Card +1 Action. Gain a Rats. Trash a card from your hand other than a Rats (or reveal a hand of Rats). When you trash this, +1 Card. Pile of 20. | order |
| $4 | Scavenger | Action | +$2. You may put your deck into your discard pile. Put a card from your discard pile onto your deck. | order |
| $4 | Wandering Minstrel | Action | +1 Card +2 Actions. Reveal the top 3 cards; put the Actions back in any order, discard the rest. | order |
| $5 | Band of Misfits | Action-Command | Play a non-Command non-Duration Action from the Supply that costs less than this, leaving it there. | order |
| $5 | Bandit Camp | Action | +1 Card +2 Actions. Gain a Spoils. | free |
| $5 | Catacombs | Action | Look at the top 3 cards. Choose one: put them into your hand; or discard them and +3 Cards. When you trash this, gain a cheaper card. | order |
| $5 | Count | Action | Choose one: discard 2 cards; or put a card from your hand onto your deck; or gain a Copper. Then choose one: +$3; or trash your hand; or gain a Duchy. | order |
| $5 | Counterfeit | Treasure | $1, +1 Buy. You may play a non-Duration Treasure from your hand twice. Trash it. | choice |
| $5 | Cultist | Action-Attack-Looter | +2 Cards. Each other player gains a Ruins. You may play a Cultist from your hand. When you trash this, +3 Cards. | order |
| $5 | Graverobber | Action | Choose one: gain a card from the trash costing $3 to $6 onto your deck; or trash an Action from your hand and gain a card costing up to $3 more. | order |
| $5 | Junk Dealer | Action | +1 Card +1 Action +$1. Trash a card from your hand. | order |
| $5 | Knights | Action-Attack-Knight | A pile of the 10 different Knights (below). | order |
| $5 | Mystic | Action | +1 Action +$2. Name a card, reveal the top card; if you named it, it goes to your hand. | order |
| $5 | Pillage | Action-Attack | Trash this. If you did, gain 2 Spoils, and each other player with 5 or more cards in hand reveals it and discards a card you choose. | choice |
| $5 | Rebuild | Action | +1 Action. Name a card. Reveal cards until a Victory card you did not name; discard the rest, trash it, gain a Victory card costing up to $3 more. | order |
| $5 | Rogue | Action-Attack | +$2. If a card costing $3 to $6 is in the trash, gain one; otherwise each other player reveals 2, trashes one costing $3 to $6, discards the rest. | choice |
| $6 | Altar | Action | Trash a card from your hand. Gain a card costing up to $5. | order |
| $6 | Hunting Grounds | Action | +4 Cards. When you trash this, gain a Duchy or 3 Estates. | free |

The Knights (all Action-Attack-Knight; every one but Sir Martin costs $5): each does its own ability first, then the common
attack "each other player reveals the top 2 cards of their deck, trashes one of them costing $3 to $6 and discards the
rest; if a Knight was trashed, trash this card".

| Knight | Own ability |
|---|---|
| Dame Anna | You may trash up to 2 cards from your hand. |
| Dame Josephine | Also a Victory card, 2 VP. |
| Dame Molly | +2 Actions. |
| Dame Natalie | You may gain a card costing up to $3. |
| Dame Sylvia | +$2. |
| Sir Bailey | +1 Card +1 Action. |
| Sir Destry | +2 Cards. |
| Sir Martin ($4) | +2 Buys. |
| Sir Michael | Each other player discards down to 3 cards (before the trashing). |
| Sir Vander | When you trash this, gain a Gold. |

Non-kingdom cards:

| Card | Types | Text |
|---|---|---|
| Hovel ($1) | Reaction-Shelter | When you gain a Victory card, you may trash this from your hand. |
| Necropolis ($1) | Action-Shelter | +2 Actions. |
| Overgrown Estate ($1) | Victory-Shelter | 0 VP. When you trash this, +1 Card. |
| Abandoned Mine ($0) | Action-Ruins | +$1. |
| Ruined Library ($0) | Action-Ruins | +1 Card. |
| Ruined Market ($0) | Action-Ruins | +1 Buy. |
| Ruined Village ($0) | Action-Ruins | +1 Action. |
| Survivors ($0) | Action-Ruins | Look at the top 2 cards of your deck. Discard them or put them back in any order. |
| Spoils ($0*) | Treasure | $3. When you play this, return it to the Spoils pile. 15, not in the Supply. |
| Madman ($0*) | Action | +2 Actions. Return this to the Madman pile. If you do, +1 Card per card in your hand. 10, not in the Supply. |
| Mercenary ($0*) | Action-Attack | You may trash 2 cards from your hand. If you did, +2 Cards, +$2, and each other player discards down to 3. 10, not in the Supply. |

## 2. Mechanics and their design

### 2.1 Capacity

Dark Ages adds 57 ids (34 kingdom cards, the Knights and Ruins pile markers, ten Knights, five Ruins, three
Shelters, Spoils, Madman, Mercenary): `NUM_CARDS` 161 -> 218. `Counts::LANES` 192 -> 224 (a multiple of 32), `SupplyMask`
becomes two `u128` words (ids 0-127 and 128-255) and `GAIN_HI` a `u128`. The Rewards keep their ids (155-160, in the
middle now): the places that looped `FIRST_REWARD..NUM_CARDS` loop over `FIRST_REWARD..FIRST_REWARD + NUM_REWARDS`.
The Dark Ages kingdom cards take 161-195, the non-kingdom cards 196 and up, so `cards::is_kingdom` is a range test.
The Ruins and Knights are the highest ids, so "is this card a member of a mixed pile" is one comparison on the gain path.

### 2.2 Mixed piles: the Knights and the Ruins (one mechanism)

Both are Supply piles of different cards, shuffled, with only the top card visible and gainable. They are one
generic mechanism, `MixedPile` (`state.rs` / `sets/dark_ages.rs`):

* **Pile marker ids.** `KNIGHTS` (a kingdom pile: it is what `GameConfig::kingdom`, the kingdom picker, the text format's
  `kingdom:` line, strategies and the Lab name) and `RUINS` (the pile that comes with a Looter; strategies name it in
  `count(Ruins)` / `supply(Ruins)` / `[[gain]]`). A marker is never in any zone; it has no types.
* **State.** The members (ten Knights, five Ruins) are in `in_supply` (the pile is in the game) and `supply` holds the
  pile's size on the *top card only*: every other member is 0. So "only the top card may be bought or gained" costs
  nothing anywhere: the Buy and Gain choices, `Zone::Supply` selections (Band of Misfits, Lurker), costs and types
  are the top card's own (Sir Martin on top can be gained with Armory, Dame Josephine is a Victory card for Ironworks, ...).
  "Cards that try to get specific cards fail if it is not the top" (Talisman) is automatic.
* **Revealing the next top lazily.** When the top card is gained, the next one is drawn *at that moment*, with the
  game's RNG, uniformly from the cards of the pile that have not been seen (see below), and the pile's size moves to it.
  This is the same distribution as shuffling the 50 Ruins (or the ten Knights) face down before the game and dealing
  `10 x (players - 1)` of them (the first `n` cards of a uniform shuffle: the next card is uniform over the cards not yet
  seen), but there is *no hidden order stored in the state at all*. The unseen cards of a pile are exactly: all copies of
  each member (10 of each Ruins, 1 of each Knight) less the copies that are in any player's zones, in the trash or on top
  of the pile; so the state needs nothing but the top card and the size, the "pool" is derived when a new top is
  needed, and a state loaded from text or edited by hand stays consistent. The setup (the first top card) is drawn in
  `GameState::new` with the game's seed, so every game of a batch has its own selection and order, never one shuffle
  reused.
* **Honest information.** A player sees the pile's top card and size (`PlayerView::supply(card)`, `pile_top`, `pile_size`)
  and the composition is public (50 Ruins in 5 kinds): the unseen part is `PlayerView::pile_unseen(marker)`. The search
  (`determinize`) needs no resampling of the pile because the state holds none of its hidden part, but it does re-seed
  the state's RNG from the searcher's own RNG, so a determinized state cannot replay the real game's next reveal.
* **Empty piles.** A mixed pile counts as one pile for the three-pile ending: it is empty when its size is 0. The members
  that are not on top do not count as empty piles (`GameState::empty_piles`).
* **Sizes.** Ruins: 10 for each player after the first (10 for 2 players, 20 for 3, ... 50 for 6), the same as the Curses.
  Knights: all ten whatever the number of players.
* **Strategies** (`card = "Knights"`, `count(Knights)`, `supply(Knights)`): see 2.9.

### 2.3 Shelters

A game uses Shelters when (Shelter page, "Preparation") all the kingdom cards are from Dark Ages, never when none is, and
otherwise when one random kingdom card is (`cards::uses_shelters`: the card is picked by hashing the game's seed, so a
fixed kingdom has the same decision for the same seed whatever the strategies do, and over many seeds the Dark Ages
share of the kingdom is the chance); `GameConfig::shelters` (`Some(true/false)`) overrides it. With Shelters each player's
starting deck is the seven Coppers and Hovel, Necropolis and Overgrown Estate in place of the three Estates, all in
`deck_unknown`: the deck is dealt from the unknown multiset by the game's RNG, so over many seeds each Shelter is
in the opening five-card hand 50% of the time and no shuffle is ever fixed once and reused (tested over thousands of seeds
and for the web's "Simulate from here"). The Estate pile is unchanged (8 or 12). The Platinum/Colony card is a
different random card (the rule says not to use the same one); the sim's random kingdoms pick the Shelter card
separately from the Colony card.

### 2.4 Non-Supply piles: Spoils, Madman, Mercenary

Three more counters on `GameState` next to Ferryman's pile and the Rewards (`gain_side`): 15 / 10 / 10 at the start of a
game with a card that gains them (Bandit Camp, Marauder, Pillage -> Spoils; Hermit -> Madman; Urchin ->
Mercenary). They are not in the Supply and not an empty-pile count (Spoils FAQ, Madman FAQ). Spoils and Madman *return* to their
pile when played; a played-twice Spoils (Counterfeit) or Madman (Procession) returns only once ("stop-moving rule"). Spoils
cannot be gained with Mint or Workshop (not in the Supply), but can be gained from the trash by Graverobber
and Rogue (it costs $0). Both piles run out only temporarily.

### 2.5 "When you trash this" (Cultist, Rats, Squire, Catacombs, Fortress, Hunting Grounds, Overgrown Estate, Feodum, Sir Vander) and Market Square

Every trash goes through one place, `GameState::trash_card` (the old `put(Act::Trash)` arm and the direct
`trash.add` sites in the Intrigue, Seaside and Prosperity sets). A trash puts the card in the trash and *queues* it
(`GameState::trash_queue`, a small fixed array: only cards with a trigger, and every card of a player's when Market Square is
in the game). The queue is flushed (`flush_trash_triggers`) into frames:

* after a `Select` whose picks trashed cards has *finished picking* and *before its `then`* ("first trash them all, then
  resolve the triggers", Count / Mercenary FAQ; a Cultist trashed to Graverobber draws three first, then Graverobber
  gains; Knights: you do not draw the revealed cards about to be discarded). A finished Select with a nonempty queue
  re-pushes itself under the trigger frames, so its `then` runs after them;
* immediately after any direct trash (Pillage, Death Cart, Procession, Horn of Plenty, Treasure Map, Count's "trash
  your hand", the Hermit exchange does not trash).

The trigger belongs to the player told to trash (the Knights' victim, not the attacker) and works on every turn
(`SetRules::on_trash`, gated by a `CardDef` flag so other cards cost one table read). Market Square's offer is one `YesNo`
per queued card for the player, repeated while another Market Square is in the hand ("you may discard multiple Market
Squares when a single card is trashed"), and not for a card the player does not own (a Supply card trashed by Lurker).
Choosing the order of simultaneous triggers is not offered: they resolve in the order the cards were trashed, a card's
own trigger before Market Square (the order only matters for a Count trashing two triggering cards while holding the
Market Square that the first one draws; see "Deferred").

Fortress and Overgrown Estate / Rats / Cultist act directly or by frame as above. Squire and Catacombs gain a
card; Hunting Grounds is a `Mode` of two options (Duchy; 3 Estates, which you may choose with the Estates gone).

### 2.6 Gaining from the trash (Graverobber, Rogue; the existing Lurker)

`Act::Gain` from `Zone::Trash` now goes through the full gain path (`finish_gain`: the gain record, Hoard / Watchtower /
Hovel triggers), not just the move. This matters here because Hovel reacts to *any* gain of a Victory card (a Duchy from
Rogue) and Death Cart gives its Ruins whatever the way it was gained.

### 2.7 New generic pieces

* Types: `RUINS`, `KNIGHT`, `LOOTER`, `SHELTER`.
* `Filter`: `Types(TypeSet)` (a one-byte named group: Attack, Ruins, Action, non-Duration Action, Curse/Ruins/Shelter/Victory), `NonTreasure` (Charlatan-aware),
  `TreasureNonDuration`, `CostAtLeast(n)` (Sage; Potion cards qualify by their coins), `CostRange(lo, hi)` (Rogue,
  Knights, Graverobber; Potion cards never qualify, see the Alchemy plan's 5b table), `VictoryNot(card)` (Rebuild).
* `FrameKind::Ask`: an optional effect with no subject card (Scavenger's "put your deck into your discard pile", Survivors' "discard
  them"), `then` says what Yes does.
* `ModeOpt`: `TopdeckFromHand`, `TrashHand`, `TrashGain` (Graverobber's second option), `GainFromTrashTo`, `TakeRevealed`,
  `DiscardRevealedDraw`; a Mode frame's `Then::ModeWindow { first, count }` offers only part of the card's table, so that Count makes its
  two separate choices from one table (indices 0-2, then 3-5).
* `SetRules`: `ATTACK_REACTIONS` (Beggar: a card in hand discarded to an Attack, with its own consequence), `on_trash`; the Urchin's offer is
  made in `resolve_effects_inner` before the reaction window; Hermit's exchange is `end_of_buy`; `gain_live_extra` keeps Hovel's reaction live
  in games with Shelters.
* A gain "from the trash" (`Act::Gain`, Lurker, Rogue, Graverobber) now runs the full gain path; a Select records its pick before the card
  moves, since a gain's reactions push frames above it.

### 2.8 Rulings (wiki page names)

* Poor House ("Poor House" FAQ): +$4 first, then reveal and lose $1 per Treasure (Curse counts under Charlatan), floor $0 on the whole
  coin total.
* Beggar ("Beggar"): the 3 Coppers go to the hand (fewer if the pile is short); the reaction is to an Attack played by another
  player, discarding first; the two Silvers one at a time, the first onto the deck, then the discard pile; a short Silver pile gives the deck one.
* Squire ("Squire"): the Attack must be in the Supply: Mercenary is not, a Ferryman pile's card is not; the pile's top Knight is.
* Vagrant ("Vagrant"): draw first, then reveal. Hovel, Necropolis and Overgrown Estate are Shelters.
* Forager ("Forager"): the trashed card counts, and so do the cards the trash gains triggered; counted after all
  triggers.
* Hermit ("Hermit"): looks through the discard pile; the trash is optional; the gain is mandatory (a card costing up to $3
  that exists); at the end of the Buy phase, if the Hermit is *in play* and no card was gained *in the Buy phase*, it goes back to
  the Hermit pile and the player gains a Madman to the discard pile; no Madman left, nothing happens. A Throne Roomed Hermit is one
  card and exchanges once.
* Market Square ("Market Square"): see 2.5.
* Sage ("Sage"): shuffles when the deck runs out (not the revealed cards); with nothing of $3 or more, discards everything.
* Storeroom ("Storeroom", draw wording): discards first, draws that many, then discards again (cards just drawn can be discarded).
* Urchin ("Urchin"): when you play another Attack with it in play, before the Attack's reactions: the same Urchin played twice (Procession) cannot; two
  Urchins can; an Urchin played from the Supply by Band of Misfits does not count as the one in play.
* Armory ("Armory"): onto the deck, a gained card is "directly" on the deck (Watchtower / Trader-style reactions see it as the deck's top).
* Death Cart ("Death Cart"): may trash itself or an Action card in hand (not both, not required); gaining it, however it is gained, gains two Ruins (as many as are left).
* Feodum ("Feodum"): Victory pile of 8 / 12; 1 VP per three Silvers however many Feodums.
* Fortress ("Fortress"): trashed means "put into your hand" at once, not a gain; it still counts as trashed (Death Cart's +$5).
* Ironmonger ("Ironmonger"): draw first, reveal, discard or not, then the bonuses of every type the revealed card has; +1 Card draws the card put back.
* Marauder ("Marauder"): Spoils first, then each other player's Ruins in turn order, the top Ruins each time.
* Procession ("Procession"): play twice, trash it (if it can still be trashed: a card that moved itself, like Madman, stays where it went), gain an Action
  costing exactly $1 more (even if the trash failed; the gained card may be a Duration; no card of that cost: nothing), without using extra Actions.
  The trash triggers (a Cultist's 3 cards) resolve before the gain.
* Rats ("Rats"): 20 in the pile whatever the number of players; draw, gain a Rats, trash a non-Rats (or reveal a hand of Rats).
* Scavenger ("Scavenger"): the deck into the discard pile is optional (not a discard: no "when you discard" triggers), the card from the discard pile onto the deck is not.
* Wandering Minstrel ("Wandering Minstrel"): draw, reveal 3 (shuffling the discard pile if short), Actions back in any order, the rest discarded.
* Band of Misfits ("Band of Misfits"): a non-Command non-Duration Action in the Supply costing *less than this card's current cost*, only the top card of a mixed pile; it
  stays in the Supply, so a card that moves itself fails (Pillage and Death Cart do not trash a Supply card; Madman cannot be played this way).
* Bandit Camp ("Bandit Camp"): draw before the Spoils.
* Catacombs ("Catacombs"): the discard-and-draw option can reshuffle the discarded cards back in; on trash it gains a card costing *less than it*.
* Count ("Count"): both choices are made even if impossible; the first is done before the second; Duchy / Copper may fail.
* Counterfeit ("Counterfeit"): $1 and +1 Buy, and the Treasure's value twice; the Treasure is trashed even if its second play did nothing; Spoils twice gives $6 and cannot be trashed.
* Cultist ("Cultist"): +2 Cards, Ruins, then optionally another Cultist from the hand for free (no Action used), which may chain.
* Graverobber ("Graverobber"): either option may be chosen even if it cannot be done; the card from the trash goes onto the deck (cost $3 to $6 and no Potion); the other option
  gains to the discard pile.
* Junk Dealer ("Junk Dealer"): draw, then trash a card (mandatory).
* Mystic ("Mystic"): names exactly; Sir Destry does not match Sir Martin; any card may be named.
* Pillage ("Pillage"): trash first, then 2 Spoils, then the attack in turn order on players with 5 or more cards; a Pillage played twice only trashes once.
* Rebuild ("Rebuild"): any card may be named; the Victory card it finds is trashed and the replacement is a Victory card costing up to $3 more (Colony if it is in the game;
  Dame Josephine and Overgrown Estate are Victory cards).
* Rogue ("Rogue"): the trash is looked at when the Attack resolves: a card costing $3 to $6 (Potion cards do not) -> gain one (mandatory); else the Knights-style attack. Rogue is an Attack
  even when it gains.
* Altar, Hunting Grounds, Knights ("Knights"): see above. The Knights attack: the victim chooses only when both cards cost $3 to $6; a Knight trashed by it makes the attacker trash the
  Knight played (not one in the Supply or already gone); "Resolve in turn order"; Sir Michael discards first; Dame Anna's own trash does not trash the Knight.
* Mercenary ("Mercenary"): may trash 2 (with one card in hand it may trash that one, but then nothing else happens); the discard attack is made after the trash, the reactions (Beggar) before.
* Madman ("Madman"): +2 Actions, return it; if it returned, +1 Card per card in hand; a second play from Procession / Throne Room does not return and so does not draw.
* Spoils ("Spoils"): +$3, returns to the pile when played; may be left unplayed in the hand.
* Hovel, Necropolis, Overgrown Estate ("Hovel", "Necropolis", "Overgrown Estate", "Shelter"): Hovel is trashed from the hand when its owner gains any Victory card (any means, also with Rogue
  from the trash); Necropolis +2 Actions; Overgrown Estate is a Victory card worth 0 and draws when trashed.
* Ruins ("Ruins", each Ruins page): Survivors: look at 2, discard both or put both back in any order (shuffling if the deck is short).
* Looters ("Looter"): Cultist, Death Cart and Marauder; any of them in the kingdom puts the Ruins in the Supply. An extra Looter in the Bane or Ferryman pile also does.

### 2.9 Strategies, kingdoms, front ends

* A strategy may say `card = "Knights"` in `[[gain]]` (the top Knight whichever it is), in `[[play]]` (any Knight) and in `[[trash]]` (any Knight), and `count(Knights)`
  (Knights owned, any of the ten), `supply(Knights)` (the pile's size); the same for `Ruins`. Specific Knights stay nameable: `card = "Sir Martin"` buys only when it is on top.
  A rule for a marker is expanded when the strategy is loaded into one rule per member in the same place with the same condition (so rank, `lists`, and the play and trash
  rules all see them), and a condition's `count` / `supply` of a marker add up its members.
* `kingdom_refs` turns a named Knight into the `Knights` kingdom pile; Ruins are never a kingdom card (a Looter brings them).
* The text format writes the Knights as one kingdom entry (`kingdom: ... Knights`), the supply with the top card of each mixed pile (`Dame Anna=10, Dame Josephine=0, ...`
  is shortened to the nonzero entries and the markers' own counts are derived), and `shelters` is implied by the players' decks. The hidden part of the Ruins pile is not in the
  text; on load it is drawn as above, when needed.
* The page: the "Dark Ages" checkbox; the Knights and Ruins piles are drawn as one pile each showing the top card, the number left and the card text; the kingdom picker offers Knights as one card.
* The Lab: `ALL_SETS` and a checkbox; the search space offers "Knights" as one gainable entry (and Ruins are never worth gaining).

## 3. Bot defaults

(Each default has a test in `crates/sim/tests/dark_ages_defaults.rs`. The decisions that shape a turn (modes, plays) go through the same
play-order search as every other card's; these are the rules used when a decision is not searched, and the policy of the opponents'
and reactive decisions.)

* **Rebuild** names the costliest Victory card of the game (Province, or Colony), so what it finds is cheaper and gets upgraded.
* **Mystic** names the card the deck holds most of (the Wishing Well rule).
* **Hovel**: trashed when a Victory card is gained (the default trash rules list Hovel, Overgrown Estate and the Ruins after the Curse, before
  Estate and Copper).
* **Procession** plays the Action whose upgrade (a card costing exactly $1 more) the gain list ranks highest, the cheaper card among equals; an Action
  with no listed upgrade is kept (Pass). **Counterfeit**: a Spoils (for $6), else a Copper that the trash rules want gone, else nothing.
* **Count**: first choice: discard 2 when the hand holds two dead cards (Curse, Ruins, Victory, Shelter), else gain a Copper; second: trash the
  hand when it is three or more cards, all dead; a Duchy late in the game (4 or fewer Provinces) when the strategy lists Duchy; else +$3.
* **Graverobber**: the trash when it holds a card costing $3 to $6 that the strategy lists (or costing $5 or more); else the upgrade when there
  is a dead Action (Ruins or a card the trash rules want) in hand; else the trash.
* **Catacombs**: take the three cards unless two of them are dead (junk or Copper), then discard them and draw three. **Squire**: +2 Actions
  when Actions in hand are stuck, else a Silver, else +2 Buys. **Hunting Grounds**: a Duchy while any is left, else the Estates.
* **Storeroom, Ironmonger, Urchin's victims, Sir Michael, Mercenary**: the generic discard / trash rules (dead cards first, nothing optional that is
  not dead); Ruins and Hovel rank with the dead cards.
* **Pillage**: the attacker takes the victim's best card (by the generic card value). **Knights' victims**: forced to trash, the cheaper of two
  qualifying cards. **Dame Anna** trashes what the trash rules want (up to two); **Dame Natalie** gains the first listed card up to $3.
* **Beggar** is discarded to an Attack whenever offered (two Silvers for a $2 card). **Market Square** is discarded for the Gold whenever offered.
* **Death Cart**: an Action from the hand only if it is a Ruins or the trash rules want it gone; otherwise it trashes itself (always, for the +$5).
  **Urchin**: trashed for a Mercenary whenever one is offered.
* **Scavenger**: put the deck into the discard pile when the discard pile is richer than the deck (average card value); the card put on the
  deck is the best by the gain list. **Survivors**: discard both when both are dead, else put them back (worse first, so the better is on top).
* **Poor House** is not worth playing with four or more Treasures in hand.

## 4. Test plan

* `crates/engine/tests/dark_ages_cards.rs`: every card's printed effect, the edge cases (empty deck and discard, a reshuffle in the middle, empty piles, Madman/Spoils pile empty), the rulings above,
  Throne Room / Procession interactions, each trash trigger on every trash path, Hovel on every gain path, Urchin before the reaction window, the Knights pile, Ruins pile and Shelter
  set-up (sizes for 2-6 players, composition varying over seeds, each kind about 1/5, the top card visible, determinize not leaking, each Shelter in the opening hand
  about 50%, never a fixed shuffle, the 3-pile ending with a Knights pile, text round trip).
* `crates/sim/tests/obvious_play.rs` with the Dark Ages cards; `crates/sim/tests/dark_ages_defaults.rs`; `shipped_strategies.rs` for the sample strategies.
* Lab: `crates/evolve/tests` (the search space offers Knights); page: `web/test_log.mjs`; `cards::set_tests` (counts per set).

## 5. Steps

1. This plan; baseline speed; capacity (Counts, SupplyMask, GAIN_HI), the card table, the set skeleton, `cargo test` green.
2. Mixed piles, Shelters, Spoils / Madman / Mercenary, trash triggers, gain from the trash: engine tests.
3. The cards, in groups, each with tests.
4. Bots and sample strategies.
5. Sim: kingdoms, strategies (Knights), the CLI; search (`determinize`).
6. Front ends: wasm, page, Lab and evolve.
7. Docs and the final checks.

## 6. Performance log

`dominion-sim match strategies/big_money_ultimate.toml strategies/double_witch.toml --games 200000`, best of five to seven runs, games per second:

| Step | Best | Note |
|---|---|---|
| Baseline, before any change | 1.77M | 1.72-1.77M |
| `Counts::LANES` 192 -> 224 alone (no Dark Ages ids yet) | 1.67M | the unavoidable widening: -5.7%, reported separately |
| + `SupplyMask`/`GAIN_HI` widened, the 57 ids and the card table | 1.71M | within the noise of the previous row |
| + mixed piles, trash queue, side piles, Shelters, all the rules | 1.66M | no further slowdown (`GameState` is 14.0 KB: 12.7 KB before, 1.2 KB of it the widening) |
| Final (after bots, page and Lab) | 1.66M | the same |

A Dark Ages pairing (`marauder_ruins.toml` vs Big Money Ultimate, `--kingdom auto`, 100000 games): 9.7k games/s (the bots search their turns).
`GameState::new` for a Base-only kingdom costs one extra pass over the kingdom (set check per card). The state grew past what a rayon worker's default
stack held with the bots' search recursion (`shipped_strategies` overflowed), so `batch::init_thread_pool` gives the workers 64 MB stacks.

## 7. Deferred

* **Order of simultaneous "when you trash" effects.** Trashed together (Count's hand, Mercenary's two cards, Chapel's four), they resolve in the
  order trashed with a card's own effect before Market Square's offer; the rules let the player choose. It only matters with two different
  triggering cards trashed at once, where one draws the other's Market Square.
* **Urchin's trash is offered only while the Mercenary pile has a card** (trashing the Urchin for nothing is never useful).
* **A second player's out-of-turn plays** (Possession, Black Market, Ambassador, Trade Route/Embargo tokens, Landmarks) do not exist here; the
  notes on them in the wiki pages are not modelled.
* **The Ruins/Knights hidden order is never stored.** The pile is drawn lazily from the unseen cards (see 2.2), so nothing in a saved state or a
  determinization can leak it; the cost is that two states reached by the same moves differ in RNG draws, not in distribution.
* **Hermit's "look through your discard pile"** is the Select over the discard pile; no separate reveal event is logged.
* The bots do not use Rebuild's name, Count or Graverobber beyond the defaults above (no card-specific search); the sample strategies show the
  shape of rules that work.
