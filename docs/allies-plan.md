# Allies

All of Allies: the 25 single kingdom piles, the 6 split piles of 4 different cards (Augurs, Clashes, Forts, Odysseys, Townsfolk, Wizards) with
their rotation, the 23 Allies, Favors and the Liaisons. Rulings are from the wiki copy (`C:/explore/wiki`: each card's page, `Ally`, `Favor`,
`Liaison`, `Rotate`, `Split pile`, `Allies`; first edition, with the 2023 errata of Island Folk and Voyage and the Royal Galley text of the 2026 announcement:
"don't set the played card aside"). Tests: `crates/engine/tests/allies_cards.rs` and `crates/sim/tests/allies_defaults.rs`.

## 1. Ids and capacity

* The 31 kingdom piles are the new highest kingdom ids, 438..=468 (after the Horse, before the Ruins and Knights), in the wiki's cost order. A split pile
  is named by its first (cheapest) card, as Sauna names Sauna/Avanto: Town Crier (Townsfolk), Herb Gatherer (Augurs), Battle Plan (Clashes), Tent
  (Forts), Old Map (Odysseys), Student (Wizards). `cards::LAST_KINGDOM` is Marquis (468).
* The 18 other pile cards are 469..=486, three per pile in the order Augurs, Clashes, Forts, Odysseys, Townsfolk, Wizards (`cards::ALLIES_SPLIT_FIRST`):
  zone ids that are not kingdom cards. The Ruins and Knights move up by 49 and stay the highest zone ids (487..=502): `NUM_ZONE_IDS` 454 -> 503,
  `Counts::LANES` 480 -> 512 (16 blocks: the `u16` block bitmap is full; ids must stay below `LANES - 4` = 508: 5 spare). `SupplyMask` is unchanged (four words,
  ids below 512).
* The 23 Allies are pseudo-cards after the Exile label (`cards::FIRST_ALLY`, `NUM_ALLIES` 23; `cards::is_ally`): never in a zone or a supply mask. Everything
  from the Events up moved up by 49. `NUM_CARDS` 629 -> 701.
* The `u16` bitmap is exactly full at 512 lanes. Plunder (558 zone ids) needs the `u32` step of `docs/card-id-capacity.md` section 10.

## 2. State

* `PlayerState::al: AlPlayer`: `favors` (the Favors mat: public), `keep` (the cards Coastal Haven keeps in hand: set aside until the new hand is
  drawn, public, owned), `aside` (the Action Contract set aside: played at the start of the next turn), `skips` (turns Lich makes the player skip),
  `hw` (Highwaymen the player has played: their Attack is on until the player's next turn), `warlords` likewise, `tokens` (Garrison's). Hashes as nothing while all zero.
* `GameState::al: AlState`: `ally` (the Ally of the game, 0 = none), `flags` / `hook` (which Allies effects are live, so other games pay one byte test),
  `fam` (the Favor tokens of Family of Inventors on Supply piles, a `SmallMultiset`), `piles` (the order of each split pile: a `u32` of 2-bit member
  indices, top first, and its length). Hashes as nothing while all zero.
* `TurnState::al: AlTurn`: Galleria, Guildmaster, Skirmisher and Bauble plays (their "this turn, when you gain" effects), Garrison plays, the Voyage limit
  (`limited`, cards played from the hand), Highwayman's "first Treasure played", Emissary's shuffle stamp, Elder's extra choice (`elder`: set
  only while the card Elder played resolves) and the once-per-turn guards of the end-of-turn Ally ability.
* Text format: `ally:`, `favors:` / `keep:` / `skips:` per player, `fam:`, and `pile order:` while a split pile is not in its dealt order.
* `PlayerView`: `ally()`, `favors()`, `favors_of(p)`, `pile_top(card)`, `pile_cards(card)`, `fam_tokens(pile)`.

## 3. Mechanics

* **Favors.** A counter (`AlPlayer::favors`, a `u8`), gained by `GameState::give_favors` and spent by `spend_favors`; every player starts with 1 when the game has an Ally
  (5 with Importer in the kingdom). A game with a Liaison (the Liaison type, bit 24) has one Ally: the kingdom's if it lists one (`cards::is_ally`),
  else one chosen from the game's seed (`cards::add_random_ally` for random kingdoms).
* **Spending Favors** is an `Ask` / `YesNo` frame whose `source` is the Ally (so the bots' `SetDefaults` and a `[[favor]]` rule decide it), pushed at the
  Ally's moment: a gain (`SetRules::gain_triggers`: Architects' Guild, Band of Nomads, City-state, Trappers' Lodge; `gain_live_extra` is one byte test),
  the start of the turn (a start-of-turn entry for the Ally, `turn_start_entries`: Cave Dwellers, Crafters' Guild, Desert Guides, Forest Dwellers,
  Gang of Pickpockets, Mountain Folk), the start of the Buy phase (`enter_buy`: Family of Inventors, League of Bankers, Market Towns, Peaceful Cult,
  Woodworkers' Guild; one byte test), after a play (`FrameKind::AfterPlay`: Circle of Witches, Fellowship of Scribes, League of Shopkeepers), the
  Clean-up (Coastal Haven, an end-of-turn offer), the end of the turn (Island Folk, once the next hand is drawn), a shuffle (`take_top`, the Star Chart
  hook: Order of Astrologers, Order of Masons) and scoring (Plateau Shepherds, `player_vp`).
* **Split piles.** All four cards of a pile are in the Supply (`in_supply`, a lane each); `AlState::piles[i]` is the order (top first, 2 bits per card), which is the
  authority for which card is on top. `pile_open(card)` is true for the top card only; the others are not offered by the Buy decision (`MAY_BUY_CARDS`),
  a Gain frame or a Supply selection. `empty_piles` counts a pile once, when all four are gone. Gaining the top pops the order; returning a card to its pile
  (Swap, Way of the Horse) pushes it on top, so a pile can be out of order, exactly as the rules allow. **Rotating** takes the top card and every copy of it directly
  under it and puts them at the bottom (`rotate_pile`). Battle Plan rotates any Supply pile (see section 7 for what is not covered).
* **Elder.** "When it gives you a choice (with "choose") this turn, you may choose an extra, different option": `TurnState::al.elder` is set while the card Elder
  played pushes its frames; `Mode` frames (and the `Then` continuations that push one, `effects::elder_mode_frame`) pushed by that play get one more pick. The extra pick is
  optional (the FAQ: "you may take an extra choice, but don't have to"): the frame is `optional` and its last pick offers `Pass`. Each of Count's two "choose one"s
  (`Then::ModeWindow`) gets its own extra pick.
* **Extra turns and skips.** Voyage and Island Folk use the Outpost machinery (`outpost_grants_extra`, no third turn in a row); a Voyage turn limits the cards
  played from the hand to 3 (`AlTurn::limited`; the Buy phase offers the Treasures one by one then). Lich's skip is `AlPlayer::skips`, taken when the
  next player is chosen; the skipped turn counts for the tie-break.
* **Highwayman / Warlord** are lingering Attacks like Gatekeeper's (`PlayerState::shielded` records who was immune when it was played). The first Treasure a
  player plays each turn does nothing (`play_free_treasure_from`, `resolve_effects_inner`, and the plain path is skipped while the Attack is on); a player cannot
  play from the hand an Action of which they have two copies in play (`phase_choices`, Select `Act::Play`).
* **Royal Galley, Contract, Conjurer, Garrison, Importer, Stronghold, Voyage, Highwayman, Warlord** are Durations (`push_duration_pending`); the ones that
  put a card aside or into play set up their own entries (`schedules_own_duration`).
* **Types.** `LIAISON` is a type bit; the six pile types (Augur, Clash, Fort, Odyssey, Townsfolk, Wizard) are not bits (the free bits are few): `cards::sub_type`
  names them, and `Filter::SharesTypeWith` (Barbarian) compares them too.
* **Cost.** Family of Inventors' tokens lower a pile's cost for everyone (`GameState::cost` asks `men_base_cost` when `M_COST` is set, which Family sets too).

## 4. Cards

Mark: `free` = choice-free, `choice`, `order` = order-sensitive (see `CLAUDE.md`).

| Card | Cost | Types | Mark | Text |
|---|---|---|---|---|
| Bauble | 2 | Treasure-Liaison | choice | Choose two different: +1 Buy; +$1; +1 Favor; this turn, when you gain a card, you may put it onto your deck |
| Sycophant | 2 | Action-Liaison | order | +1 Action; discard 3 cards, if you discarded any +$3. When you gain or trash this, +2 Favors |
| Town Crier | 2 | Action-Townsfolk | choice | Choose one: +$2; gain a Silver; +1 Card +1 Action. May rotate the Townsfolk |
| Herb Gatherer | 3 | Action-Augur | order | +1 Buy; put your deck into your discard pile, look through it and may play a Treasure from it. May rotate the Augurs |
| Battle Plan | 3 | Action-Clash | order | +1 Card +1 Action; may reveal an Attack for +1 Card; may rotate any Supply pile |
| Tent | 3 | Action-Fort | choice | +$2; may rotate the Forts. When you discard this from play, may put it onto your deck |
| Importer | 3 | Action-Duration-Liaison | choice | Next turn gain a card costing up to $5. Setup: each player +4 Favors |
| Merchant Camp | 3 | Action | choice | +2 Actions +$1. When you discard this from play, may put it onto your deck |
| Old Map | 3 | Action-Odyssey | order | +1 Card +1 Action; discard a card, +1 Card; may rotate the Odysseys |
| Sentinel | 3 | Action | order | Look at the top 5 cards, may trash up to 2, put the rest back in any order |
| Underling | 3 | Action-Liaison | free | +1 Card +1 Action +1 Favor |
| Student | 3 | Action-Wizard-Liaison | order | +1 Action; may rotate the Wizards; trash a card from your hand, if a Treasure +1 Favor and put this onto your deck |
| Broker | 4 | Action-Liaison | order | Trash a card from hand, choose one: +1 Card / +1 Action / +$1 / +1 Favor per $1 it costs |
| Carpenter | 4 | Action | order | If no Supply pile is empty +1 Action and gain a card up to $4; else trash a card and gain one costing up to $2 more |
| Courier | 4 | Action | choice | +$1; discard the top card of your deck; look through your discard pile and may play an Action or Treasure from it |
| Innkeeper | 4 | Action | order | +1 Action; choose one: +1 Card; +3 Cards then discard 3; +5 Cards then discard 6 |
| Royal Galley | 4 | Action-Duration | choice | +1 Card; may play a non-Duration Action from your hand; next turn, if it is still in play, replay it. Neither is discarded until then |
| Town | 4 | Action | choice | Choose one: +1 Card +2 Actions; or +1 Buy +$2 |
| Acolyte | 4 | Action-Augur | order | May trash an Action or Victory card from your hand to gain a Gold; may trash this to gain an Augur |
| Voyage | 4 | Action-Duration-Odyssey | free | +1 Action; take an extra turn after this one (not a 3rd in a row), in which you can only play 3 cards from your hand |
| Miller | 4 | Action-Townsfolk | choice | +1 Action; look at the top 4 cards, put one into your hand, discard the rest |
| Conjurer | 4 | Action-Duration-Wizard | choice | Gain a card costing up to $4; next turn put this into your hand |
| Archer | 4 | Action-Attack-Clash | choice | +$2; each other player with 5 or more cards in hand reveals all but one, you choose one of those to be discarded |
| Garrison | 4 | Action-Duration-Fort | free | +$2; this turn, when you gain a card add a token here; next turn +1 Card per token |
| Barbarian | 5 | Action-Attack | choice | +$2; each other player trashes the top card of their deck; if it costs $3 or more they gain a cheaper card that shares a type, else a Curse |
| Capital City | 5 | Action | choice | +1 Card +2 Actions; may discard 2 for +$2; may pay $2 for +2 Cards |
| Contract | 5 | Treasure-Duration-Liaison | choice | $2, +1 Favor; may set aside an Action from your hand to play at the start of your next turn |
| Emissary | 5 | Action-Liaison | free | +3 Cards; if that shuffled, +1 Action and +2 Favors |
| Galleria | 5 | Action | free | +$3; this turn, when you gain a card costing $3 or $4, +1 Buy |
| Guildmaster | 5 | Action-Liaison | free | +$3; this turn, when you gain a card, +1 Favor |
| Highwayman | 5 | Action-Duration-Attack | choice | Next turn discard this and +3 Cards; until then the first Treasure each other player plays each turn does nothing |
| Hunter | 5 | Action | order | +1 Action; reveal the top 3, put an Action, a Treasure and a Victory card from them into your hand, discard the rest |
| Modify | 5 | Action | order | Trash a card from your hand; choose one: +1 Card +1 Action; gain a card costing up to $2 more |
| Skirmisher | 5 | Action-Attack | free | +1 Card +1 Action +$1; this turn, when you gain an Attack, each other player discards down to 3 |
| Specialist | 5 | Action | order | May play an Action or Treasure from your hand; choose one: play it again; gain a copy of it |
| Swap | 5 | Action | order | +1 Card +1 Action; may return an Action from your hand to its pile to gain to your hand a different Action costing up to $5 |
| Sorceress | 5 | Action-Attack-Augur | order | +1 Action; name a card, put the top card of your deck into your hand; if it was named each other player gains a Curse |
| Warlord | 5 | Action-Duration-Attack-Clash | free | +1 Action; next turn +2 Cards; until then others cannot play from the hand an Action they have 2 copies of in play |
| Elder | 5 | Action-Townsfolk | order | +$2; may play an Action from your hand; its "choose" gives you an extra, different option |
| Sunken Treasure | 5 | Treasure-Odyssey | choice | Gain an Action card you have no copy of in play |
| Sorcerer | 5 | Action-Attack-Wizard | order | +1 Card +1 Action; each other player names a card and reveals their top card; if wrong they gain a Curse (it goes back) |
| Hill Fort | 5 | Action-Fort | choice | Gain a card costing up to $4; choose one: put it into your hand; or +1 Card +1 Action |
| Marquis | 6 | Action | order | +1 Buy; +1 Card per card in your hand; discard down to 10 |
| Sibyl | 6 | Action-Augur | order | +4 Cards +1 Action; put a card from your hand on top of your deck and another on the bottom |
| Distant Shore | 6 | Action-Victory-Odyssey | free | +2 Cards +1 Action; gain an Estate. 2 VP |
| Lich | 6 | Action-Wizard | order | +6 Cards +2 Actions; skip a turn. When you trash this, discard it and gain a cheaper card from the trash |
| Stronghold | 6 | Action-Duration-Victory-Fort | choice | Choose one: +$3; or next turn +3 Cards. 2 VP |
| Territory | 6 | Victory-Clash | free | 1 VP per differently named Victory card. When you gain this, gain a Gold per empty Supply pile |

Split pile order (top first, 4 of each): Townsfolk Town Crier, Blacksmith, Miller, Elder; Augurs Herb Gatherer, Acolyte, Sorceress, Sibyl; Clashes Battle Plan, Archer,
Warlord, Territory; Forts Tent, Garrison, Hill Fort, Stronghold; Odysseys Old Map, Voyage, Sunken Treasure, Distant Shore; Wizards Student, Conjurer, Sorcerer, Lich.
Blacksmith (3, Action-Townsfolk, choice): choose one: draw until you have 6 cards in hand; +2 Cards; +1 Card +1 Action. (Marks of the pile cards are in the code.)

The 23 Allies: Architects' Guild (2 Favors on a gain: gain a cheaper non-Victory card), Band of Nomads (a Favor on gaining a card costing $3+: +1 Card, +1 Action or +1 Buy),
Cave Dwellers (start of turn, a Favor: discard a card, draw a card; repeat), Circle of Witches (after a Liaison, 3 Favors: each other player gains a Curse), City-state (2 Favors
on gaining an Action on your turn: play it), Coastal Haven (Clean-up: keep that many cards for as many Favors), Crafters' Guild (start of turn, 2 Favors: gain a card up to $4 onto your
deck), Desert Guides (start of turn, a Favor: discard the hand, draw 5; repeat), Family of Inventors (start of Buy, a Favor: a token on a non-Victory pile; -$1 each),
Fellowship of Scribes (after an Action with 4 or fewer cards in hand, a Favor: +1 Card), Forest Dwellers (start of turn, a Favor: look at the top 3, discard any, put back the rest),
Gang of Pickpockets (start of turn: spend a Favor or discard down to 4), Island Folk (end of turn, 5 Favors: an extra turn), League of Bankers (start of Buy: +$1 per 4 Favors),
League of Shopkeepers (after a Liaison: $1 with 5 Favors, +1 Action +1 Buy with 10), Market Towns (start of Buy, a Favor: play an Action; repeat), Mountain Folk (start of turn, 5 Favors:
+3 Cards), Order of Astrologers (shuffle: a Favor per card put on top), Order of Masons (shuffle: a Favor per 2 cards kept out), Peaceful Cult (start of Buy: a Favor per card trashed),
Plateau Shepherds (scoring: 2 VP per Favor paired with a $2 card), Trappers' Lodge (a gain, a Favor: put it onto your deck), Woodworkers' Guild (start of Buy, a Favor: trash an Action, gain an Action).

## 5. Bots (`crates/sim/src/defaults/allies.rs`, `strategy.rs`, `eval.rs`)

* **Favors** are spent by a stated rule first (`[[favor]] ally = "Cave Dwellers" use = true if = "..."` or `[[favor]] use = "never"`: the first rule whose condition
  holds), then by a fast default per Ally (`SetDefaults::favor_use`): the cheap abilities are used while the Favors are not needed for something worth more, the costly ones
  (Island Folk, Mountain Folk) when affordable and the game is not nearly over. No search: a Favor decision is never in `eval::is_play_decision`.
* **Rotation** (`Ask` of a card with source a pile card): rotate when the top card is not in the strategy's gain list (or has run out for the player) and a card deeper in the pile is.
* Expression variables: `favors` (mine), `opp_favors`, `ally == "Name"` (`has_ally(Name)`), `pile_top(Name)`. Evolve: no genome change beyond the placement hints.

## 6. Front ends

wasm (`sets_mask` bit, the Ally and the Favors in the JSON, split piles with their top card and count), the page (set checkbox, Favors in the player box, the Ally in the landscape
row, split piles showing the top card and the cards left in the pile, log lines for Favors and rotations), the Lab (`ALL_SETS`), `build-web.ps1`.

## 7. Approximated or left out

Everything of the set is implemented: the 25 single piles, the 6 split piles with rotation, the 23 Allies, Favors and the Liaisons. What is approximated or decided:

* **One Ally per game, chosen from the seed.** A kingdom with a Liaison and no Ally gets one from the game's seed (`cards::add_random_ally` for random kingdoms, `allies::setup`
  for a listed kingdom: `FIRST_ALLY + seed % 23`); a kingdom that lists an Ally with no Liaison plays it (the Favors are still dealt out). A kingdom without either has no Favors.
* **Battle Plan** offers only the piles that rotating changes (a split pile with different cards under its top run): rotating any other pile is a no-op in the rules too.
* **"Gain an Augur" (Acolyte)** takes the top card of the Augurs only, as only the top of a pile can be gained.
* **Royal Galley** follows the 2026 text ("don't set the played card aside"): the card stays in play and is played again next turn if it still is.
* **Order of Astrologers / Order of Masons** are offered at the shuffle (the `take_top` hook of Star Chart), with the cards picked in a Select; the bots use a value ordering, not a search.
* **Plateau Shepherds** counts cards costing exactly $2 in the whole deck, the Favors pair up one for one.
* **Territory** gains at most 12 Golds (more than the number of piles in any game).
* **Evolve / Lab**: the Lab can select the set and the kingdoms get an Ally, but the genome has no `[[favor]]` rules and no placement hints for the Allies' cards (the Liaisons are gained
  by the generic rules); a strategy file can state `[[favor]]` rules by hand.
* **Bots** spend Favors by a cheap default per Ally (`SetDefaults::favor_use`), not by search; a Favor decision is never in `eval::is_play_decision`.

## 8. Measurements

* `GameState` about 26,300 -> 27,232 bytes (of which 128 are `_pad`, see below), `PlayerState` 3,648 bytes, a 2-player copy (`dup`) 12,640 bytes (`tests/state_size.rs` bounds 27,300 / 3,700 / 12,700);
  `Counts` 480 -> 512 bytes.
* Speed, `dominion-sim match big_money_ultimate double_witch --games 1000000`, ALL threads (16 cores, 32 threads), 10 rounds interleaved: the build before 1.34M games/s (mean), `LANES` 512 only 1.36M,
  with Allies 1.32M mean / 1.36M best (-1.2% best, -2.7% mean against `LANES` 512). With 4 threads, or with one thread per core, there is no difference at all.
* **The leak was the layout, not the code.** With the Allies fields in place the all-threads run lost 7-12% while one thread per core lost nothing: two games on the two hyperthreads of a core share the
  L1 data cache, and where the hot fields (`turn`, the seats' hands) fall decides the speed. Adding `_pad` bytes after `GameState::al` moves them: 0 bytes -13%, 32 -7%, 64 -4%, 96 -5%, **128 -1%**,
  160 -6%, 192 -8%, 224 -9%, 288 -3% (best of 6 rounds each, against `LANES` 512). The Highwayman test in `play_all_choice_free_treasures` and the Voyage limit were moved out of the hot loop as well (a Highwayman's Attack makes
  `treasure_order_matters` true until the first Treasure is played, which is also the right rule: the player picks which one is wasted). Same-seed outputs are identical to the build before.
