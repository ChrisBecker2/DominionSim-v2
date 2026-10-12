# Hinterlands (2nd edition)

Status: implemented (26 kingdom cards, engine, bot defaults, page and Lab checkboxes, recommended kingdoms, three sample strategies).
Code: `crates/engine/src/sets/hinterlands.rs`, `crates/sim/src/defaults/hinterlands.rs`; tests `crates/engine/tests/hinterlands_cards.rs`
(82 tests including a 4000-game conservation fuzz) and `crates/sim/tests/hinterlands_defaults.rs`.

## 1. Cards (2E list; the removed 1E cards are not implemented)

Source: wiki page "Hinterlands" (kingdom list; "Removed first-edition kingdom cards": Duchess, Oracle, Noble Brigand, Nomad Camp, Silk Road, Cache,
Embassy, Ill-Gotten Gains, Mandarin) and each card page.

| Cost | Cards (marking: free / choice / order) |
|---|---|
| 2 | Crossroads (order), Fool's Gold (free, Treasure-Reaction) |
| 3 | Develop (order), Guard Dog (order, Reaction), Oasis (order), Scheme (choice), Tunnel (Victory-Reaction) |
| 4 | Jack of All Trades (order), Nomads (free), Spice Merchant (choice), Trader (order, Reaction), Trail (free, Reaction), Weaver (choice, Reaction) |
| 5 | Berserker (choice, Attack), Cartographer (order), Cauldron (choice, Treasure-Attack), Haggler (free), Highway (free), Inn (order), Margrave (free, Attack), Souk (order), Stables (choice), Wheelwright (order), Witch's Hut (order, Attack) |
| 6 | Border Village (free), Farmland (Victory) |

Ids 328-353. Everything from 328 up in the old numbering moved up by 26 (Ruins/Knights 354-369, Events from 370; all referenced by name).

## 2. Rulings (wiki page in brackets)

* Fool's Gold: $1 the first play in a turn, $4 after; trash from hand for a Gold onto the deck when *another* player gains a Province, also on your own
  turn [Fool's Gold]. Counted by `TurnState::played`, so a Crown / Counterfeit replay gives $4. Each copy in hand may react.
* Crossroads: +3 Actions only if it is the first one played this turn (`played`; an Enchantress play counts) [Crossroads].
* Develop: trash, then gain a card costing exactly $1 more and one exactly $1 less, onto the deck, in either order, each with its own gain triggers; a
  Copper gains only the $1 card; either gain may fail [Develop]. One Gain frame with `GF_BOTH` offers both costs; after the first gain a frame for the
  other cost follows (under the first gain's triggers). Costs are fixed when the card is trashed (the Wayfarer FAQ example is out of scope).
* Guard Dog: +2 Cards, then +2 more with 5 or fewer cards in hand; a reaction to any Attack, asked before Moat (it may draw one); played out of turn it is
  discarded at that turn's Clean-up (`discard_next_cleanup`) [Guard Dog].
* Scheme: counts plays this turn (Throne Room'd = 2); at Clean-up that many Actions among those being discarded (not Durations that stay) may go onto the deck [Scheme].
* Tunnel / Trail / Weaver: reaction to a discard other than in Clean-up, from anywhere; cards discarded together are all discarded first, then the
  reactions are asked one at a time; a reshuffle caused by one loses the others [Hinterlands, "When discarded"].
* Trail also reacts to being gained and trashed (played from the trash it returns to play), not in Clean-up [Trail].
* Nomads: +$2 when gained or trashed; on another player's turn it gives the current turn nothing [Nomads].
* Trader: trash for Silvers equal to the cost; reaction to any gain: exchange the gained card for a Silver (back to its pile, Silver to the discard
  pile, not a gain, both must be available; the card's own "when you gain" still happens) [Trader]. Not offered for a Silver (pointless).
* Berserker: gain a card costing less, others discard to 3; when gained with an Action in play it is played, no question, also on another player's turn
  (it then attacks the turn's player and is discarded at that Clean-up) [Berserker]. The reaction window and victims are relative to the attacker.
* Cauldron: Treasure-Attack $2 +1 Buy; the third Action the turn's player gains this turn (counted from the start of the turn) gives each other player a
  Curse, once per Cauldron played before it; players unaffected when it was played (Moat, Lighthouse) are skipped [Cauldron]. Not modelled: playing a
  Cauldron during the resolution of the third gain.
* Haggler: +$2; per Haggler played, each *bought* card gains a cheaper non-Victory card [Haggler].
* Souk: +1 Buy, +$7, then -$1 per card in hand, not below $0 in total; gain: trash up to 2 from hand [Souk].
* Inn: gain: reveal any Actions from the discard pile and shuffle them into the deck (the whole deck is shuffled, even with none) [Inn].
* Farmland: gain: trash a card from hand, gain one costing exactly $2 more, not a Farmland [Farmland]. Border Village: gain a cheaper card [Border Village].
* Jack of All Trades, Spice Merchant, Stables, Wheelwright, Witch's Hut, Cartographer, Highway, Margrave, Oasis: as printed [each page].

## 3. Trigger and reaction design

* Gain triggers: `Hinterlands::gain_triggers`, behind `GAIN_TOP` bits (a game without these cards pays one mask test). Resolution order is the reverse of
  push order: Trader's exchange, Trail's play, Berserker's play, the card's own effect, Cauldron's Curses, Haggler's gains, then Fool's Gold for the others.
  `on_buy` separates buys (Haggler) from gains.
* Discards: `G_DISCARD_HOOK` (game flag, set when Faithful Hound, Tunnel, Trail or Weaver is in the game) makes every discard site call `discard_hook`;
  Tunnel / Trail / Weaver go to the existing `trash_queue` (entry kind `Q_DISCARDED`) and are flushed when the selection finishes (`finish_select`, or
  at the bulk sites), which gives "discarded together, reactions one by one". The Hound keeps its immediate frame. Nothing in Clean-up.
* Trash: Trail and Nomads use `on_trash`.
* Generic pieces added: `Act::ShuffleIn`, `Filter::ActionToDiscard`, `ModeOpt::CardsActions` / `GainUpToCost`, `Then::{DrawUpTo, DrawIfHandAtMost,
  ModeIfPicked, GainPerCost, ...}`, `GF_BOTH`, a mandatory `YesNo` (`Then::BerserkerPlay`), the reaction window parametrised by the attacker.
* Per-turn state: `TurnState::hint` (Cauldron counters), not in the text format (Cauldrons in play are recounted on load).

## 4. Bot defaults (`defaults/hinterlands.rs`)

Rule-driven: Fool's Gold always trashed, Tunnel always revealed, Trail / Weaver always played, Trader exchange: an `[[exchange]]` rule for the gained card,
else junk, Copper and unlisted non-Victory cards yes, listed and Victory cards no. Small choices: Jack and Cartographer discard poor cards (junk, Copper),
Stables discards a Copper, Wheelwright discards a cheap card for the best listed Action it pays for, Witch's Hut discards dead Actions first (a Curse),
Inn shuffles in every Action but Ruins, Scheme puts back listed Actions (one terminal), Develop trashes wanted-trash cards whose neighbours the list
likes, Weaver gains a card only if the list ranks one above Silver, Spice Merchant takes Buy+$2 only when it reaches a better listed card, Souk is played
with 6 or fewer other cards in hand. These choices follow rules, they are not searched (deferred).

## 5. Tests

Per card: effect, edge cases (short deck, empty pile, hand sizes); buy vs gain (Haggler, Nomads); own vs other player's turn (Nomads, Trail, Berserker,
Fool's Gold, Trader on a Curse); Moat / Guard Dog / Watchtower; Throne Room (Crossroads, Haggler, Scheme, Margrave); simultaneous discards (Tunnel
through Cellar and Militia); text round trip; conservation fuzz. `sim`: bots finish games; shipped strategies beat Big Money Ultimate.

## 6. Sample strategies (vs Big Money Ultimate)

`jack_of_all_trades_bm.toml` ~78%, `fools_gold_bm.toml` ~63% (Fool's Gold on every hand with nothing better), `border_village_engine.toml` ~65%.
A Border Village / Inn list that spent hands on Inn did worse than Smithy and Market alone in the test kingdom.

## 7. Performance (`match big_money_ultimate double_witch --games 200000`, games/s)

| Step | Speed |
|---|---|
| Before (HEAD) | 1.69M |
| Ids + dormant table, `LANES` 352 to 384 | 1.57M (about 1.64M with LANES kept at 352: the widening is -4%) |
| Complete set (final build) | 1.59-1.63M (-3.7% vs before) |

`GameState` 19.6 KB to about 20.7 KB; `PlayerState` 2560 to 2720; `Counts` 384. Rare handlers are `#[inline(never)]`.

## 8. Deferred

Searching Develop / Jack / Cartographer / Scheme like the play order; cost changes between Develop's gains; Cauldron played during the third gain;
Trader exchange of a Silver; a cached total in `Counts` to win the widening back.
