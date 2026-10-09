# Plan: Adventures (and the Sauna/Avanto split pile)

Adventures is a set of its own (`CardSet::Adventures`; page checkbox "Adventures", wasm set-mask bit 256, Lab set name
"Adventures"; off by default like the other sets the shipped strategies know nothing about). 30 kingdom piles (Page and
Peasant start the two Traveller lines), the 8 non-Supply Traveller upgrade cards, and 20 Events (a new kind of "landscape" that is
bought, not gained). The promo split pile Sauna/Avanto (`CardSet::Promo`) comes with it: it needs the same machinery for
a pile whose top card alone may be gained.

> **Status: done** (steps in 5, performance log in 6, what is left in 7). Card text and rulings are from the
> DominionStrategy wiki's pages for the cards, Events and mechanics (named per ruling below), using the current text and
> the newest FAQ: Adventures has no second edition, but the wiki's "announced changes for a future printing" are in force
> (Inheritance cannot set aside a Duration since the 2025 errata; Mission cannot take a third turn in a row since the 2023
> errata). The tests are the specification.

## 1. Cards

Marking: free = `choice_free`, choice = `has_choice`, order = `order_sensitive` (see "Adding cards" in `CLAUDE.md`).
Types are Action unless listed.

| Cost | Card | Types | Text (summary) | Marking |
|---|---|---|---|---|
| $2 | Coin of the Realm | Treasure-Reserve | $1. Put this on your Tavern mat. After you play an Action card, you may call this for +2 Actions. | choice (goes to the mat) |
| $2 | Page | Action-Traveller | +1 Card +1 Action. Exchange for Treasure Hunter when discarded from play. | free |
| $2 | Peasant | Action-Traveller | +1 Buy +$1. Exchange for Soldier. | free |
| $2 | Ratcatcher | Action-Reserve | +1 Card +1 Action. Mat. At the start of your turn, call it to trash a card from your hand. | order |
| $2 | Raze | Action | +1 Action. Trash this or a card from your hand; look at one card from the top of your deck per $1 the trashed card costs, put one into your hand, discard the rest. | order |
| $3 | Amulet | Action-Duration | Now and at the start of your next turn: choose one: +$1; trash a card from your hand; gain a Silver. | order |
| $3 | Caravan Guard | Action-Duration-Reaction | +1 Card +1 Action; at the start of your next turn +$1. When another player plays an Attack, you may first play this from your hand. | order |
| $3 | Dungeon | Action-Duration | +1 Action. Now and at the start of your next turn: +2 Cards, then discard 2 cards. | order |
| $3 | Gear | Action-Duration | +2 Cards. Set aside up to 2 cards from your hand face down; put them into your hand at the start of your next turn. | order |
| $3 | Guide | Action-Reserve | +1 Card +1 Action. Mat. At the start of your turn, call it to discard your hand and draw 5 cards. | order |
| $4 | Duplicate | Action-Reserve | Mat. When you gain a card costing up to $6, you may call this to gain a copy of it. | order |
| $4 | Magpie | Action | +1 Card +1 Action. Reveal the top card: a Treasure goes to your hand; an Action or Victory card gains a Magpie. | order (known top) |
| $4 | Messenger | Action | +1 Buy +$2. You may put your deck into your discard pile. When this is the first card you gain in your Buy phase, gain a card costing up to $4 and each other player gains a copy. | order |
| $4 | Miser | Action | Choose one: put a Copper from your hand onto your Tavern mat; or +$1 per Copper on your Tavern mat. | choice |
| $4 | Port | Action | +1 Card +2 Actions. When you gain this, gain another Port (12 in the pile). | free |
| $4 | Ranger | Action | +1 Buy. Turn your Journey token over (it starts face up); if it is face up, +5 Cards. | free |
| $4 | Transmogrify | Action-Reserve | +1 Action. Mat. At the start of your turn, call it to trash a card from your hand and gain a card to your hand costing up to $1 more. | order |
| $5 | Artificer | Action | +1 Card +1 Action +$1. Discard any number of cards; you may gain a card onto your deck costing exactly $1 per card discarded. | order |
| $5 | Bridge Troll | Action-Duration-Attack | Each other player takes their -$1 token. On this turn and your next turn cards cost $1 less. Now and at the start of your next turn +1 Buy. | free (only victims act) |
| $5 | Distant Lands | Action-Reserve-Victory | Mat. Worth 4 VP if on your Tavern mat at the end of the game, else 0. (12 or 8 cards.) | choice |
| $5 | Giant | Action-Attack | Turn your Journey token over; face down: +$1; face up: +$5 and each other player reveals the top card, trashes it if it costs $3 to $6, else discards it and gains a Curse. | free |
| $5 | Haunted Woods | Action-Duration-Attack | At the start of your next turn +3 Cards. Until then, when another player gains a card they bought, they put their hand onto their deck in any order. | free |
| $5 | Lost City | Action | +2 Cards +2 Actions. When you gain this, each other player draws a card. | free |
| $5 | Relic | Treasure-Attack | $2. Each other player puts their -1 Card token on their deck. | choice |
| $5 | Royal Carriage | Action-Reserve | +1 Action. Mat. After you play an Action card, if it is still in play, you may call this to replay it. | order |
| $5 | Storyteller | Action | +1 Action. Play up to 3 Treasures from your hand. Then +1 Card and pay all your $ for +1 Card per $1. | order |
| $5 | Swamp Hag | Action-Duration-Attack | At the start of your next turn +$3. Until then, when another player gains a card they bought, they gain a Curse. | free |
| $5 | Treasure Trove | Treasure | $2. Gain a Gold and a Copper. | choice |
| $5 | Wine Merchant | Action-Reserve | +1 Buy +$4. Mat. At the end of your Buy phase, if you have at least $2 unspent, you may discard this from your Tavern mat. | free (card goes to the mat) |
| $6 | Hireling | Action-Duration | At the start of each of your turns for the rest of the game, +1 Card. | free |
| $3* | Treasure Hunter | Action-Traveller | +1 Action +$1. Gain a Silver per card the player to your right gained on their last turn. Exchange for Warrior. | choice |
| $4* | Warrior | Action-Attack-Traveller | +2 Cards. For each Traveller you have in play (including this) each other player discards the top card of their deck and trashes it if it costs $3 or $4. Exchange for Hero. | free |
| $5* | Hero | Action-Traveller | +$2. Gain a Treasure. Exchange for Champion. | choice |
| $6* | Champion | Action-Duration | +1 Action. For the rest of the game other players' Attacks don't affect you and when you play an Action card you first get +1 Action. | free |
| $3* | Soldier | Action-Attack-Traveller | +$2, +$1 per other Attack in play; each other player with 4 or more cards in hand discards a card. Exchange for Fugitive. | free |
| $4* | Fugitive | Action-Traveller | +2 Cards +1 Action; discard a card. Exchange for Disciple. | order |
| $5* | Disciple | Action-Traveller | You may play an Action card from your hand twice; gain a copy of it. Exchange for Teacher. | order |
| $6* | Teacher | Action-Reserve | Mat. At the start of your turn, call it to move your +1 Card / +1 Action / +1 Buy / +$1 token to an Action Supply pile you have no tokens on. | order |
| $4 | Sauna (Promo, 5 on top of 5 Avanto) | Action | +1 Card +1 Action. You may play an Avanto from your hand (it uses no Action). This turn, when you play a Silver, you may trash a card from your hand. | order |
| $5 | Avanto | Action | +3 Cards. You may play a Sauna from your hand (no Action used). | order |

(The starred cost is the non-Supply upgrade cost: it is the cost for every rule that compares costs.)

### Events (never gained, never in a deck; bought in the Buy phase for their cost and a Buy)

| Cost | Event | Effect | Search |
|---|---|---|---|
| $0 | Alms | Once per turn: if you have no Treasures in play, gain a card costing up to $4. | searched |
| $0 | Borrow | Once per turn: +1 Buy. If your -1 Card token is not on your deck, put it there and +$1. | searched |
| $0 | Quest | You may discard an Attack, two Curses, or six cards; if you do, gain a Gold. | searched |
| $1 | Save | Once per turn: +1 Buy. Set aside a card from your hand; put it into your hand at end of turn (after drawing). | searched |
| $2 | Scouting Party | +1 Buy. Look at the top 5 cards of your deck; discard 3 and put the rest back in any order. | searched |
| $2 | Travelling Fair | +2 Buys. When you gain a card this turn, you may put it onto your deck. | searched |
| $3 | Bonfire | Trash up to 2 Coppers you have in play. | searched |
| $3 | Expedition | Draw 2 extra cards for your next hand. | rule-only |
| $3 | Ferry | Move your -$2 cost token to an Action Supply pile. | rule-only |
| $3 | Plan | Move your Trashing token to an Action Supply pile. | rule-only |
| $4 | Mission | Take an extra turn after this one (not a 3rd turn in a row), during which you cannot buy cards. | rule-only |
| $4 | Pilgrimage | Once per turn: turn your Journey token over; if it is face up, gain a copy of up to 3 differently named cards you have in play. | searched |
| $5 | Ball | Take your -$1 token. Gain 2 cards each costing up to $4. | searched |
| $5 | Raid | Gain a Silver per Silver you have in play. Each other player puts their -1 Card token on their deck. | searched |
| $5 | Seaway | Gain an Action card costing up to $4. Move your +1 Buy token to its pile. | rule-only |
| $5 | Trade | Trash up to 2 cards from your hand; gain a Silver per card trashed. | searched |
| $6 | Lost Arts | Move your +1 Action token to an Action Supply pile. | rule-only |
| $6 | Training | Move your +$1 token to an Action Supply pile. | rule-only |
| $7 | Inheritance | Once per game: set aside a non-Command non-Duration Action from the Supply costing up to $4; move your Estate token to it. During your turns Estates are also Command Actions that play that card, leaving it there. | rule-only |
| $8 | Pathfinding | Move your +1 Card token to an Action Supply pile. | rule-only |

"Searched" = its whole effect resolves this turn: the bot's turn search plays the buy and its frames out and the leaf
scores what they gained or trashed (a gain at the gain-list weight of the card). "Rule-only" = its value is in later turns,
so it is bought only when the strategy's rules name it (position and condition), never speculatively by the search.

## 2. Mechanics and their design

### 2.1 Capacity

`CardId` is now `u16`: the real cards alone are 258 (the 30 Adventures kingdom cards, the 8 upgrades, Sauna and Avanto
bring the ids past 256, which a `u8` cannot name). The Events are given ids too (20 more, 278 in all), see 2.2.
`Counts::LANES` 224 -> 288 (a multiple of 32), `SupplyMask` has a third word for ids 256 and up (only the members of the mixed
piles, the highest ids, are ever in the Supply there). The id layout: 0-201 as before; 202-231 the Adventures kingdom cards;
232-233 Sauna and Avanto (defined in `promo.rs`); 234-241 the Traveller upgrades; 242-261 the Events; 262 and up the Ruins
pile marker, the Ruins and the Knights (the two mixed piles are still the highest ids, so `c >= FIRST_PILE_CARD` still means "a mixed pile member").

### 2.2 Events: pseudo-cards

An Event has a card id (a `CardDef` with no type bits, cost = its cost) so that everything that names things by id works unchanged:
a strategy's `[[gain]] card = "Ball"`, expression variables, the text format, logs, the Lab's search space, the page. An Event id is
never in a `Counts`, a zone, `supply` or `in_supply` (`cards::is_event(c)` is an id range test). `GameConfig::events` lists the Events of
a game, `GameState::events` is the mask. The `Buy` decision offers `Choice::Card(event)` for each affordable Event (cost, a Buy; Events are
not affected by cost reduction, Mission's turn still offers them, "once per turn" and "once per game" Events are offered only when allowed); applying it pays, uses a
Buy, ends Treasure playing (as a buy does: 2nd edition) and runs `sets::adventures::event_effect`, which pushes the generic frames.

### 2.3 Tavern mat and Reserve cards

`PlayerState::tavern: SmallMultiset<10>` (public; counted as owned by `all_cards`; Distant Lands adds 4 VP per copy on it in `vp()`). A Reserve card's play puts
itself on the mat if it is still in play (a Throne Room'd second play does not), with the rest of its text above the line unaffected. A *call* moves the card from
the mat into play (it is discarded at cleanup like any card in play; Wine Merchant is discarded straight from the mat) and is not a play: no Action used, no
"when you play" triggers. Calls are `YesNo` decisions with `Act::Call` and `Zone::Tavern`:

* **After an Action is played** (Coin of the Realm, Royal Carriage): `resolve_effects` pushes a `FrameKind::AfterPlay` frame under the card's frames when the player
  has a Reserve of that kind on the mat; it runs when the card has been completely resolved and offers each call. Royal Carriage needs the card still in play. A replay is a
  play, so a second Royal Carriage can be called after it, and the Action plays (Throne Room's, Disciple's, Sauna/Avanto's) each open their own window.
* **At the start of the turn** (Guide, Ratcatcher, Transmogrify, Teacher): entries of the start-of-turn list (`turn_start_entries`), resolved in the order the player picks
  like Durations (they are marked `start_order_sensitive`: they act on the hand), each asking whether to call.
* **When you gain** (Duplicate): a gain trigger, one `YesNo` per Duplicate on the mat, for a card costing up to $6 with a copy left in its Supply pile.
* **End of the Buy phase** (Wine Merchant): `end_of_buy` offers to discard each Wine Merchant when at least $2 is unspent.

### 2.4 Tokens

Per player (`PlayerState`): the pile each pile token is on (`tok: [CardId; 6]` for +1 Card, +1 Action, +1 Buy, +$1, -$2 cost, Trashing; the pile is named by
its Supply id, `cards::pile_of`, so a token on the Knights pile is on the Knights marker and one on Sauna/Avanto is on Sauna), the Estate token's card (`inherit`), and flags: the -1 Card token on the
deck, the -$1 token in front of the player, the Journey token face down. `GameState::adv` (a flags byte) says which are live in this game (set when a token is first placed or a
token-giving card is in the game), so a game without them pays one byte test on its hot paths:

* **Play bonuses** (+1 Card, +1 Action, +1 Buy, +$1): `resolve_effects` applies them *before* the card's own effect and, for +1 Card, on top of the stack so it is drawn
  before anything else the card does (even before an Attack's reactions). They apply to every play of a card from the pile (Throne Room's second play too) by the token's owner.
* **-$2 cost**: `GameState::cost` subtracts 2 for cards of that pile on the owner's turn (all players buying or gaining then: "on your turns").
* **Trashing**: the gain trigger offers a trash from the hand when the owner gains a card from that pile (by any means, on any turn).
* **-1 Card**: the next `Draw` frame of the player that draws at least one card draws one fewer and removes the token ("draws" only: revealing, Envoy's put-into-hand and
  Scouting Party do not); Library's draw-to-7 skips a draw. Empty deck and discard: the token is still removed.
* **-$1**: the next time the player gets $ (a coin total that rises) they get $1 less (`TurnState::tax`): applied after the step that raised the coins.
* **Journey**: Ranger, Giant and Pilgrimage turn it over and then check it.
* **Estate** (Inheritance): see 2.6.

Placement is a generic frame: `FrameKind::Token` (decision `DecisionKind::Token { token }`, choices `Choice::Card(pile)`): the Action Supply piles (Knights counts as one; the Ruins
pile does not take tokens: it is legal by the rules but never useful) satisfying the frame's restriction (Teacher: no token of the player's on it; Seaway: the pile of the card just gained).

### 2.5 Travellers

The eight upgrade cards are non-Supply piles of five (`GameState::trav: [u8; 8]`, shown in `PlayerView::supply`, saved in `side piles:`), present when Page or Peasant is in the kingdom.
"When you discard this from play, you may exchange it for X" is a cleanup offer (`offers_on_discard`): a `YesNo` (`Act::Exchange`) for each Traveller in play, highest upgrade first so a short pile is used where it matters;
exchanging returns the card to its pile and puts the upgrade into the discard pile (not a gain, not a trash). It is skipped when the next pile is empty. Page and Peasant themselves are Supply piles (10 cards) and the
returned copy goes back to the Supply pile. An empty non-Supply pile does not count for the end of the game.

### 2.6 Inheritance

`PlayerState::inherit` is the set-aside card. During the owner's turn Estates in their hand are Action (and Command) cards: they are offered by `PlayAction`, picked by
Throne Room-like selections from the hand, and playing one plays `inherit` from where it is (the Captain path: `PlayEffects` with `Zone::Supply`, so a card that tries to move itself fails). The Estate pile and other players' Estates are unaffected.

### 2.7 Mission

`TurnState::mission` set by the Event; cleanup grants an extra turn when the turn is not itself an extra turn (the Outpost flag, shared: no third turn in a row). Outpost wins if both would grant. The Mission turn has `no_buy`: the `Buy` decision offers no Supply card (Events still).

### 2.8 Sauna/Avanto

Both ids are in `in_supply` with 5 cards each; the pile is one kingdom pile (`Sauna` is the kingdom id). `Avanto` is not legal to gain or buy while a Sauna is left (`may_buy`
and the Gain frame's choices), and `empty_piles` counts the pair as one pile that is empty when both are. Naming either half in a kingdom list selects the pile.
Sauna's trash is a count of Saunas played this turn (`TurnState::sauna`): each Silver played offers that many optional trashes (errata 2022: not tied to the Sauna staying in play), and
with a Sauna played the Silvers in hand are not auto-played (the player orders them against their trashing).

### 2.9 Generic pieces added

`Act::Call`, `Act::Exchange`, `Zone::Tavern`, `FrameKind::AfterPlay`, `FrameKind::Token`, `DecisionKind::Token`, a `distinct` flag on selections (Pilgrimage), a Frame `Then` per card where
the card has a bespoke continuation (listed in `state.rs`), `ModeOpt` atoms for Miser and Quest.

## 3. Strategy semantics (bots)

### 3.1 Events in the gain list

Events are listed in the same priority list as cards: `[[gain]] card = "Ball" if = "..."`. Position is priority against the cards; the list doubles as the buy list, but gains that are not
buys (Workshop, Remodel, Ball's own gains, Alms') skip Events (Events are never gain choices). Conditions work as usual. The variables `bought(Event)` (times bought this game) and `bought_turn(Event)` (this turn) are available. Bots that name no Event never buy one.

### 3.2 Searched Events and rule-only Events

The Buy decision inside the turn search follows the rules (as for every buy); a searched Event is one whose effect is scored by the leaf (gains at their gain-list weight, trashes at the trashed cards' list value, as for
any card gained or trashed this turn) so the *play order* search sees "play Coppers and Silvers, then buy Alms for a Gold" as a line. A rule-only Event contributes nothing to the leaf, so the search never plays toward it; it is bought when the rule list reaches it.

### 3.3 Tokens

`[[token]] token = "+1 Card" pile = "Laboratory" if = "..."`. Token names (case-insensitive): `+1 Card`, `+1 Action`, `+1 Buy`, `+$1`, `-$2 cost` (also `−$2 cost`, `-$2`), `Trashing`, `Estate`, `-1 Card`, `-$1`, `Journey`; the last three are not placed by a
choice and are accepted only for forward compatibility (a rule for them is an error). The rules are tried in order; the first whose pile is legal now (an Action Supply pile in the game, a legal placement for the frame asking, condition true) is chosen. Defaults when no rule applies:

* +1 Card / +1 Action / +1 Buy / +$1: the Action pile the player owns the most copies of (ties: the gain-list order, then the lower id); +1 Action prefers a non-terminal and +1 Card a terminal draw when tied by count... (details in `defaults/adventures.rs`).
* -$2 cost (Ferry): the best Action pile the gain list still wants (the first listed Action card with copies left).
* Trashing (Plan): the Action pile the strategy gains most (the earliest gain-list Action).
* Estate (Inheritance): the best non-Victory, non-Duration, non-Command Action costing up to $4 that the strategy plays (play rules), else a draw or +Actions card.
* Teacher moves the token kind with no pile yet first (card, then action, buy, coin), else the one whose pile placement scores best.

`token_on(+1 Card, Smithy)` is an expression variable (1 when the player's token is on that pile).

### 3.4 Calls and exchanges

`[[call]] card = "Royal Carriage" if = "..."`: the first rule for the card whose condition holds decides yes; with rules stated for the card and none holding, no. Calls with no rule use the defaults below. Conditions are
evaluated for the call being asked (`hand(...)`, `coins`, `actions`, `count(...)`). `[[exchange]] card = "Page" if = "..."` (the card being exchanged) decides the cleanup exchange; the default is always exchange. A condition-only
form `[[exchange]] card = "Page" if = "false"` keeps the Page.

### 3.5 Searched decisions

Duplicate's call, Royal Carriage's and Coin of the Realm's calls, and Wine Merchant's discard are searched when `search_play` is on, like the play order: the choice maximizing the strategy's own leaf scoring. The start-of-turn
calls (Guide, Ratcatcher, Transmogrify, Teacher) follow the rules below. A stated `[[call]]` rule wins over the search. The leaf values a Reserve card returned from the mat to the deck cycle (`mat_return_value`): the
coins its next play is worth times the chance it is drawn and played in the turns left, in `vp_per_coin` like Coffers.

### 3.6 Defaults for the other new decisions

(Each is tested in `crates/sim/tests/adventures_defaults.rs`; the code is `crates/sim/src/defaults/adventures.rs`.)

| Decision | Default |
|---|---|
| Buying an Event | never, unless the gain list names it (and its condition holds, it is affordable and available) |
| Token placement | by rule, else 3.3 |
| Coin of the Realm | call when no Action is left to play and an Action waits in the hand (searched, 3.5) |
| Royal Carriage | call after a card that draws, pays, buys or attacks (searched) |
| Duplicate | call for a card the gain list lists or any Treasure/Action worth having, never for a Copper or junk (searched, with the leaf pricing the extra copy at its gain-list weight) |
| Wine Merchant | discard from the mat whenever $2 is left (searched; the evaluator values the returned card, see 3.5) |
| Guide | call with 3 or more dead cards in hand, or a hand of 3 or fewer with a deck to draw |
| Ratcatcher | call when the trash rules want a card of the hand gone |
| Transmogrify | call when the trash rules want a card gone, or a listed card costing more can be gained |
| Teacher | call while one of the four tokens has no pile yet |
| Traveller exchange | always exchange (a stated `[[exchange]]` rule can keep it) |
| Amulet | trash a card the rules want gone; else a Silver when the gain list wants Silver; else +$1 |
| Miser | a Copper onto the mat while the mat holds few and the hand has one; else cash in |
| Quest | two Curses, else an Attack, else six cards when nothing in the hand is worth keeping, else nothing |
| Pilgrimage, Gear, Inheritance | copies of listed cards in play; set aside what cannot be played this turn; the best Action costing up to $4 the strategy plays (`choose_prince_card`) |
| Messenger, Raze, Travelling Fair, Save | by the gain-list value of the cards involved (see the code) |

### 3.7 Rulings and their sources

Rules text is from the DominionStrategy wiki page of the card, Event or mechanic named (current text, newest FAQ); the tests named are in `adventures_cards.rs`.

| Ruling | Wiki page | Test |
|---|---|---|
| Reserve cards go to the mat only if still in play; a call is not a play (no Action used, no "when you play"); calling Royal Carriage needs the card still in play; a second Royal Carriage may follow | Royal Carriage, Tavern mat (Reserve) | `royal_carriage_*`, `reserve_cards_go_onto_the_tavern_mat...` |
| Coin of the Realm: +2 Actions after an Action is played; works on a Reserve card that went to the mat, which Royal Carriage cannot replay | Coin of the Realm | `coin_of_the_realm_*` |
| Duplicate: a gained card costing up to $6 with a copy left in its pile; also on other players' turns and for non-buy gains | Duplicate | `duplicate_*` |
| Wine Merchant: at the end of the Buy phase with $2+ unspent, each may be discarded from the mat | Wine Merchant | `wine_merchant_*` |
| Traveller exchange: when discarded from play only (not from the hand), highest upgrade first, the card returns to its pile, the upgrade goes to the discard pile; skipped when its pile is empty; non-Supply upgrade piles do not count for the end of the game | Traveller, Page, Peasant | `a_traveller_*`, `the_higher_traveller_*` |
| Champion: attacks played earlier no longer affect the player; +1 Action per Action played, not applied to itself | Champion | `champion_*` |
| Warrior: counts Travellers in play including itself; trashes only cards costing exactly $3 or $4 | Warrior | `warrior_*` |
| Treasure Hunter: Silvers per card the right-hand player gained last turn (every gain) | Treasure Hunter | `treasure_hunter_*` |
| Disciple on a non-Supply card plays it twice, gains nothing; on a Duration both plays stay | Disciple | `disciple_*` |
| Haunted Woods / Swamp Hag trigger on cards *bought* by another player, once per card bought per Hag | Haunted Woods, Swamp Hag | `haunted_woods_*`, `swamp_hag*` |
| Messenger: only when it is the first card gained in the Buy phase; each other player gains a copy | Messenger | `messenger_*` |
| Giant: the Journey token alternates; a revealed card costing $3-$6 is trashed, otherwise discarded and a Curse gained (also with no card to reveal) | Giant | `giant*` |
| Ranger / Journey token starts face up; the token flips even when the effect does nothing | Ranger, Journey token | `ranger_*`, `journey_token_*` |
| Raze: trashes itself or a hand card; looks at as many cards as the trashed card costs | Raze | `raze_*` |
| Magpie: reveal; a Treasure goes to the hand, an Action or Victory gains a Magpie | Magpie | `magpie_*` |
| Artificer: gain onto the deck exactly $1 per card discarded | Artificer | `artificer_*` |
| Bridge Troll: the -$1 token is taken by each other player; cards cost $1 less this turn and next, stacking | Bridge Troll, -$1 token | `bridge_troll_*`, `two_bridge_trolls_*`, `the_minus_one_coin_token_*` |
| -1 Card token: the next draw of at least one card is one shorter ("draws" only); removed even from an empty deck | -1 Card token, Relic | `the_minus_one_card_token_*`, `relic_*` |
| Relic can be blocked by a Moat; Raid's token hand-out is not blocked by Moat or Champion | Relic, Raid | `relic_can_be_blocked...`, `raid_cannot_be_blocked...` |
| Events: the cost and a Buy, never reduced; Treasures stop being auto-played after a buy; once-per-turn and once-per-game limits; Mission's turn buys Events but no cards, and no third turn in a row (2023 errata) | Event, Mission | `events_are_offered...`, `mission_*` |
| Alms: once per turn, only with no Treasures in play | Alms | `alms_*` |
| Borrow, Save, Quest, Scouting Party, Travelling Fair, Bonfire, Trade, Pilgrimage, Seaway, Ball, Raid, Expedition | each Event's page | one or more tests each |
| Inheritance: once per game; a non-Command non-Duration Action up to $4 (2025 errata); Estates in hand are Command Actions that play it in place | Inheritance | `inheritance_*` |
| Tokens on a pile apply to every play of that pile's cards by the owner, before the card's own effect; Ferry's -$2 on the owner's turns; Plan's trashing on any gain from the pile | Tokens (pile), Ferry, Plan | `tokens_*`, `ferry*`, `plan_*` |
| Teacher moves one of four tokens to a pile with none of the player's tokens | Teacher | `teacher_*` |
| Sauna/Avanto: five of each, only the top card for sale, one pile at the end of the game; each Silver played after a Sauna may trash a card (2022 errata); Avanto/Sauna play each other free | Sauna, Avanto, Split pile | `sauna_*`, `the_split_pile_*`, `two_saunas_*` |
| Distant Lands: 4 VP only on the mat | Distant Lands | `distant_lands_*` |

## 4. Test plan

* `crates/engine/tests/adventures_cards.rs`: every card, Event and token (printed effect, edge cases, rulings), set-up (piles, Traveller piles, Sauna/Avanto, kingdom counts, ids), text round trip.
* `crates/sim/tests/adventures_defaults.rs`: bot defaults and searched decisions; `obvious_play.rs` with the Adventures cards; `shipped_strategies.rs` for the samples.
* Lab: `crates/evolve/tests` (Events/tokens/calls in the search space); page: `web/test_log.mjs`.

## 5. Steps

1. This plan; `CardId` `u16`, capacity (Counts 288, SupplyMask), the id layout, perf check.
2. Engine state, the cards of Adventures and Sauna/Avanto with tests.
3. Events, tokens, calls, exchanges with tests.
4. Text format, `PlayerView`, wasm.
5. Bots: defaults, strategy rules (`[[token]]`, `[[call]]`, `[[exchange]]`), the search, the evaluator.
6. Page, Lab/evolve, samples.
7. Docs and the final checks.

## 6. Performance log

`dominion-sim match strategies/big_money_ultimate.toml strategies/double_witch.toml --games 200000`, games per second (interleaved runs on one machine; the committed HEAD built in a second worktree is the baseline).

| Step | Best | Note |
|---|---|---|
| Baseline, before any change | 1.64M | 1.61-1.64M |
| `CardId` u16 + `LANES` 224 -> 288 + `SupplyMask` third word (no Adventures yet) | 1.54M | -6%: the widening, reported separately |
| Everything (cards, Events, tokens, calls, bots), before the clean-up below | 1.37M | -18% against the baseline measured at the same time (1.68M); engine-only random playouts +8% per step (260 vs 242 ns), `GameState::new` 430 vs 250 ns |
| Cold Adventures paths marked `#[inline(never)]`, `canonical_picks` shortcut for one pick, Event buy outlined | 1.37-1.41M | within noise of the line above; code layout moves the number by 3% either way |

Findings: `GameState` grew from 14 016 to 20 320 bytes (`CardId` and `Counts` width, per-player Tavern mat, tokens and Estate token). Raising `LANES` to 320 (a
multiple of 64) changed nothing, so the loss is not the Counts width: it is the wider ids and structures plus the new branches in `advance`, `apply`, `resolve_effects`
and the Buy choices. No single hot spot was found (no profiler is available here without Administrator rights); a sampled profile is the next step if the loss matters.
Adventures games run at about 1.0M games/s for the Ball strategy against Big Money Ultimate and 2.7k games/s for the Reserve engine (full bots with the turn search).

## 7. Deferred

* **Alms before the Treasures.** The engine plays choice-free Treasures when the Buy phase starts, so Alms ("no Treasures in play") can only be bought with no Treasures in the hand. Offering Alms
  (and Bonfire's Coppers in play) before the Treasures needs a Buy decision ahead of the Treasure plays; the samples therefore do not use Alms.
* Lurker can pick Avanto from the Supply while Sauna remains (the Lurker choice list is not filtered by `may_buy`).
* Royal Carriage replaying a Duration while another multiplier group (Throne Room, Procession) is active is skipped.
* `[[exchange]]` rules are not part of the Lab's search space (the default always exchanges); `[[token]]`, `[[call]]` rules and Events are.
* Speed: see 6. The 18% loss against the baseline is documented, not recovered.
* Dedicated text round-trip and `determinize` leak tests for each new field: covered only through the card-conservation fuzz (`random_adventures_games_conserve_cards_and_terminate`).
