# Plan: Alchemy

> **Status (2026-09-30): implemented, except Possession, which is not implemented (by decision).**
> The 11 other kingdom cards and the Potion are playable and tested; the ported
> `AlchemyCardsTests.cpp` cases are in `crates/engine/tests/alchemy_port.rs`. Bot defaults:
> `crates/sim/tests/alchemy_defaults.rs`. Sample strategy: `strategies/familiar_bm.toml`.

Alchemy has one edition: 12 kingdom cards plus the basic Potion. This simulator has 11 of the 12.

> Card text is from memory: the wiki blocks automated fetches (an Anubis proof-of-work page), as
> it did for the earlier sets. The ported tests and the rulings below are the specification;
> check them against the physical cards when convenient.

## 1. Cards

P = a Potion in the cost.

| Cost | Card | Types | Text (summary) | Order |
|---|---|---|---|---|
| $4 | Potion | Treasure | +1 Potion to spend. Basic card; 16 in the supply, only in games with a P card. | free |
| $0P | Transmute | Action | Trash a card from your hand. Action: gain a Duchy; Treasure: a Transmute; Victory: a Gold (each that applies). | choice |
| $0P | Vineyard | Victory | 1 VP per 3 Action cards you have (rounded down). | n/a |
| $2 | Herbalist | Action | +1 Buy, +$1. When you discard this from play, you may put a Treasure from play onto your deck. | free |
| $2P | Apothecary | Action | +1 Card +1 Action. Reveal the top 4; Coppers and Potions to hand, the rest back in any order. | choice |
| $2P | Scrying Pool | Action-Attack | +1 Action. Each player (you too) reveals their top card; you choose discard or put back. Then reveal until a non-Action; all revealed to hand. | choice |
| $2P | University | Action | +2 Actions. You may gain an Action costing up to $5 (no Potion). | choice |
| $3P | Alchemist | Action | +2 Cards +1 Action. When you discard this from play, if you have a Potion in play, you may put this onto your deck. | free |
| $3P | Familiar | Action-Attack | +1 Card +1 Action. Each other player gains a Curse. | free |
| $3P | Philosopher's Stone | Treasure | Count your deck and discard pile: +$1 per 5 cards. | free |
| $4P | Golem | Action | Reveal until 2 Actions other than Golem; discard the rest; play the two in either order. | choice |
| $5 | Apprentice | Action | +1 Action. Trash a card from your hand: +1 Card per $1 it costs, +2 Cards if it has a P. | choice |
| $6P | Possession | Action | **Not implemented, by decision** (no stage 2). It is not in the card table, so it can't be put in a kingdom. | - |

Philosopher's Stone is marked choice-free although it is not a plain "+$": no choice-free
Treasure changes the size of the deck or discard pile, and a Treasure that does (Anvil, Crystal
Ball, War Chest...) already stops the auto-play of the hand, so the order of the choice-free
Treasures never matters for it. Herbalist and Alchemist are choice-free for the same reason
Treasury is: their offers are made at the end of the turn, not during the play.

## 2. Engine decisions (all generic: no card-specific frames or decisions)

1. **Potion cost.** `CardDef` gets `potion: bool` (the cost has a P) and `potions: u8` (the
   Potion card produces one). `GameState::cost(c)` still returns only the coins, so Bridge,
   Quarry and Peddler reduce only the $ part, and the Forge, Bishop and Apprentice arithmetic
   counts only $. `cards::potion_cost(c)` is the P; `cards::cost_string(c)` prints "$2P".
2. **Comparisons.** A `Gain` frame carries `potion: bool`, the P of the reference cost.
   "Costing up to $X": a card with a P is legal only if the reference has a P (Remodel on a
   Golem, $4P, may gain up to $6P; Remodel on an Estate may not gain a Golem). `exact` (Upgrade,
   Swindler): the gained card's P must equal the reference's. Forge, Workshop, Smugglers,
   University, Anvil, Artisan, Pirate, Bureaucrat, Blockade... have a plain `$X` reference: never
   a P card. `Then::GainUpTo` takes the P of the trashed card; `TrashTopThenGain` that of the
   trashed top card. Bishop (`$ / 2`) and Salvager (`+$ = cost`) use only the coins.
3. **Buying.** `TurnState::potions` (printed `potions: N` in the text format when nonzero) is
   what is available. Potion cards add one when played (`CardDef::potions`: in the choice-free
   Treasure path, and in `resolve_effects` for Tiara and Crystal Ball plays). `Buy` offers a card
   with a P only when `potions > 0`; buying it spends one. Potions don't carry over (a new
   `TurnState` starts at 0). Expression variables: `potions`, and `potion_cards` (cards I own
   with a P in the cost).
4. **Supply.** `GameState::new` adds the Potion pile (16) whenever a kingdom card has a P, even
   if the list doesn't name it; a list may name it too (like Platinum and Colony). The random
   kingdoms (`cards::random_kingdom_with_colonies`), the text format (`kingdom:` and `supply:`
   lines), the wasm `padded_kingdom`, the evolve kingdoms and `dominion-sim --kingdom` apply it
   through `cards::add_potion_if_needed`. Optional basics named in a required list no longer
   count toward the 10 kingdom cards. `Strategy::kingdom_refs` skips Potion, so a rule that
   mentions it doesn't take a kingdom slot.
5. **Sets.** `CardSet::Alchemy` (wasm sets-mask bit 16, the page's Cards checkbox, the Lab's
   sets, `random:alchemy`). Opt-in in the page and the Lab (unchecked by default), because the
   shipped strategies know nothing about Potions.
6. **End-of-turn offers.** `engine::push_end_of_turn_offers` (was `push_treasury_offers`) makes
   the offers at the end of the Buy phase, before cleanup: Treasury (a `YesNo`, `Act::Topdeck`
   from `Zone::InPlay`, unless a Victory card was gained this Buy phase), then Alchemist (the
   same `YesNo`, only if a Potion is in play), then Herbalist (a `Select`, `Act::Topdeck` from
   `Zone::InPlay`, 0..1 Treasures, one per copy in play). **Fixed order: Treasury, Alchemist,
   Herbalist.** A player may order such triggers; this order lets both the Alchemist and the
   Potion go back (Herbalist's Treasure goes on top of the Alchemist). One physical card is one
   offer, whatever Throne Room did to it.
7. **New generic pieces.** `FrameKind::RevealUntil` (reveal until `max` cards match `filter`:
   Scrying Pool until a non-Action, Golem 2 Actions other than Golem), `Filter::{Either,
   NonAction, ActionNot}`, `Frame::{potion, optional}` (an optional `Gain` offers `Pass`:
   University), `Then::{GainPerType, DrawPerCost, DiscardRevealedNotMatching,
   PlayPickedThenRest}`, and a `PlayerState::held` zone (see Golem). Apothecary reuses Patrol's
   frames.
8. **Scrying Pool.** For each target (me first, then the victims in turn order; Moat blocks a
   victim): a `RevealTop(1)`, then a `Select` (discard 0..1, **chosen by the Pool's owner**: a
   `Decision` with `player` = owner and `for_player` = the target), then what is left goes back
   on top. Last, the owner's reveal-until-a-non-Action. A card revealed from an empty deck
   reshuffles the discard first, so discarding your only card still draws it back.
9. **Golem.** After the reveal, the non-matching cards are discarded. A `Select` (`Act::Play`,
   from the Revealed zone) picks which Action goes first (a real decision only when the two
   differ). The other is moved into `PlayerState::held` (not the Revealed zone, which the first
   card's effects may use: Apothecary, Scrying Pool; and not `in_play`, which Sea Chart reads)
   and enters play (the Play event) only when its own turn comes. `held` counts as owned
   (`all_cards`), so cards are conserved at every step. With fewer than two Actions it plays what
   it found. Golem plays Durations, which stay in play. Throne Room on Golem resolves it twice.
10. **Vineyard** is `count_type(Action) / 3` in `state::vp_of_cards`, wherever the cards are. Its
    pile is a Victory pile (8, or 12 with 3+ players).
11. **Transmute** gains by type: Action: Duchy; Treasure: Transmute (a Curse counts as a Treasure
    under Charlatan); Victory: Gold. A multi-type card gives each; a missing or empty pile gives
    nothing; with no other card in hand nothing happens (the trash is mandatory if there is a
    card).
12. **Apprentice** draws `cost + 2 if P` cards, the cost being the current one (a Bridge discount
    counts; a Vineyard, $0P, always draws 2).
13. **Philosopher's Stone**: $1 per 5 cards in the deck (known top, unknown and known bottom) and
    the discard pile, each time it is played (twice with Tiara).

## 3. Part A: where a gain came from

`Event::Gain { player, card, to, source }`: `source` is the card whose effect or trigger caused
the gain, or `engine::NO_SOURCE` (255) for a plain buy. `GameState::gain` takes it; gain frames
carry it as `source`; triggered gains pass their card (Hoard's Gold: Hoard; Blockade's Curse:
Blockade). The page log prints it in parentheses: "Player 2 gains Curse to their discard pile
(Witch)"; a buy prints as before. `web/app.js` doesn't parse the line (it only wraps card names
in chips), so nothing changed there.

## 4. Bots

- **Transmute, Apprentice**: the trash rules (stated rules first, then Curse, Estate, Copper); a
  forced trash with nothing wanted takes the cheapest card.
- **Scrying Pool** (`Strategy::choose_scry`): on my own deck, discard Curses, Victory-only cards
  and Treasures worth less than the average non-Action card of my deck; keep Actions, Potions
  and good Treasures. On an opponent's deck, the reverse: discard Actions, Potions and Silver or
  better, and leave their junk on top.
- **Herbalist**: the best Treasure from play: a Potion if the gain list wants a card with a P
  that is still in the supply, else the one worth the most coins; never a Curse and never a
  worthless Treasure (an unwanted Potion): then `Pass`.
- **Alchemist**: always back on the deck (the existing `YesNo` / `Act::Topdeck` default).
- **University**: the first Action costing up to $5 in the gain list; otherwise declines (an
  optional gain with nothing wanted is `Pass`).
- **Golem's order, buying**: the play-order search (`sim::eval`) treats Golem's `Select`
  (`Act::Play`) like a Throne Room pick; Philosopher's Stone is choice-free (section 1), so it
  adds no search branching. `[[gain]]` rules can't buy a P card without a Potion, since the
  engine doesn't offer it.

## 5. Tests

`alchemy_port.rs` ports every C++ test (TestPotion, TestVineyard, TestTransmute, TestApothecary,
TestScryingPool, TestUniversity, TestFamiliar, TestApprentice, TestPhilosphersStone, TestGolem,
TestAlchemist, TestHerbalist). Adapted: the other-set decoys (Adventurer, Necropolis, Hovel,
Woodcutter, Poor House, Hamlet) are 2nd-edition cards with the same relevant property (Festival,
Village, Estate, Smithy, Witch); the Ruins, shelter, -1 Card token and Debt cases (Engineer, City
Quarter, Fortune) are not ported (those sets are not here); the "Simulation" cases (the C++ bots)
are covered by `sim/tests/alchemy_defaults.rs`. New: the Potion cost rules (buying with and
without a Potion, Remodel, Upgrade, Forge, Bridge, Quarry, Bishop, Workshop, Smugglers,
Swindler), Scrying Pool with Moat, Golem with Throne Room and Durations, Herbalist and Alchemist
order and Throne Room, text round trips, kingdom plumbing. Talisman (1st edition) is not in this
simulator, so its "ignores P cards" case does not apply. `gain_source.rs` and the wasm crate's
`log_tests` cover Part A; `web/test.mjs` and `web/test_consistency.mjs` cover the page.

## 6. Not done

- **Possession**: not implemented (by decision); it is not in the card table.
- Evolve has no Potion-aware templates ("Big Money + Familiar" can't buy it without a Potion).
  The Lab can pick Alchemy, but a search needs a Potion rule to get started.
- Sample strategies: only Familiar-BM.

## 7. Performance log

Release build; `dominion-sim match`, 2M games (300k for the action-heavy one), best of 3 runs.
"Before" is the build with Part A only (the event field), "after" is everything.

| Benchmark | Before | After |
|---|---|---|
| Full bots: BMU vs Double Witch, `--kingdom "Sentry,Militia,Witch,Village,Smithy,Market,Cellar,Moat,Workshop,Remodel"` | 1.331M/s | 1.343M/s |
| Fast mode: `bench/strategies/fast_big_money.toml` vs `fast_double_witch.toml`, `--kingdom auto` | 1.374M/s | 1.383M/s |
| Action-heavy: `smithy_bm` vs `village_smithy_engine`, Village/Smithy/Market/Laboratory/Festival/... | 190.5k/s | 188.7k/s |

No measurable change (within 1%): the Potion test in `Buy` is one lookup per supply card, the
gain-frame check one more, `PlayerState::held` is 9 bytes per player, `Frame` grew by 2 bytes.
The numbers are not comparable with the older tables in `seaside-prosperity-plan.md` (the
strategy files and the machine differ). 125 card ids still fit the 128 lanes of `Counts` and
the `u128` supply mask.
