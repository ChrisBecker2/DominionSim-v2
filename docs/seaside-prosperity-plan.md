# Plan: Seaside and Prosperity (2nd edition)

**Scope.** Seaside 2nd edition (27 kingdom cards) and Prosperity 2nd edition (25 kingdom cards
plus Platinum and Colony), mixable with Base and Intrigue. 1st-edition-only cards are not
included:
- Seaside: Ambassador, Embargo, Explorer, Ghost Ship, Navigator, Pearl Diver, Pirate Ship, Sea Hag.
- Prosperity: Contraband, Counting House, Goons, Loan, Mountebank, Royal Seal, Talisman, Trade Route,
  Venture.

> Card text is from memory (the wiki blocks automated fetches). The tests and a check
> against the physical cards are the specification. Checked against the wiki afterwards: see
> section 6 (rules audit); where it differs from the text below, section 6 is right.

## 1. The cards

**Seaside 2nd edition.** D = Duration.

| $ | Card | Types | Text (summary) |
|---|---|---|---|
| 2 | Haven | Action–D | +1 Card +1 Action. Set aside a card from your hand face down; next turn put it into your hand. |
| 2 | Lighthouse | Action–D | +1 Action. Now and next turn: +$1. While in play, other players' Attacks don't affect you. |
| 2 | Native Village | Action | +2 Actions. Choose: put your deck's top card on your Native Village mat (private); or put all mat cards into your hand. |
| 3 | Astrolabe | Treasure–D | Now and next turn: +$1, +1 Buy. |
| 3 | Fishing Village | Action–D | +2 Actions +$1. Next turn: +1 Action +$1. |
| 3 | Lookout | Action | +1 Action. Look at the top 3: trash one, discard one, put one back. |
| 3 | Monkey | Action–D | Until your next turn, when the player to your right gains a card, +1 Card. Next turn: +1 Card. |
| 3 | Sea Chart | Action | +1 Card +1 Action. Reveal the top card; if you have a copy in play, put it into your hand. |
| 3 | Smugglers | Action | Gain a copy of a card costing up to $6 that the player to your right gained on their last turn. |
| 3 | Warehouse | Action | +3 Cards +1 Action. Discard 3 cards. |
| 4 | Blockade | Action–D–Attack | Gain a card up to $4, set aside; next turn put it into your hand. While set aside, when another player gains a copy on their turn, they gain a Curse. |
| 4 | Caravan | Action–D | +1 Card +1 Action. Next turn: +1 Card. |
| 4 | Cutpurse | Action–Attack | +$2. Each other player discards a Copper (or reveals a hand without one). |
| 4 | Island | Action–Victory (2 VP) | Put this and a card from your hand on your Island mat (public; counts for VP). |
| 4 | Salvager | Action | +1 Buy. Trash a card from your hand; +$ equal to its cost. |
| 4 | Sailor | Action–D | +1 Action. Once this turn, when you gain a Duration card, you may play it. Next turn: +$2, and you may trash a card from your hand. |
| 4 | Tide Pools | Action–D | +3 Cards +1 Action. Next turn: discard 2 cards. |
| 4 | Treasure Map | Action | Trash this and a Treasure Map from your hand; if you trashed two, gain 4 Golds onto your deck. |
| 5 | Bazaar | Action | +1 Card +2 Actions +$1. |
| 5 | Corsair | Action–D–Attack | +$2. Next turn: +1 Card. Until then, each other player trashes the first Silver or Gold they play each turn. |
| 5 | Merchant Ship | Action–D | Now and next turn: +$2. |
| 5 | Outpost | Action–D | Your next hand is 3 cards; take an extra turn after this one (not a 3rd in a row). |
| 5 | Pirate | Action–D–Reaction | Next turn: gain a Treasure up to $6 to your hand. When any player gains a Treasure, you may play this from your hand. |
| 5 | Sea Witch | Action–D–Attack | +2 Cards. Each other player gains a Curse. Next turn: +2 Cards, then discard 2 cards. |
| 5 | Tactician | Action–D | If you have a card in hand, discard your hand, and next turn: +5 Cards, +1 Action, +1 Buy. |
| 5 | Treasury | Action | +1 Card +1 Action +$1. At the end of your Buy phase, if you didn't gain a Victory card in it, you may put this onto your deck. |
| 5 | Wharf | Action–D | Now and next turn: +2 Cards +1 Buy. |

**Prosperity 2nd edition**, plus **Platinum** ($9, Treasure $5) and **Colony** ($11, 10 VP).

| $ | Card | Types | Text (summary) |
|---|---|---|---|
| 3 | Anvil | Treasure | $1. You may discard a Treasure to gain a card up to $4. |
| 3 | Watchtower | Action–Reaction | Draw until 6 cards in hand. When you gain a card, you may reveal this to trash it or put it onto your deck. |
| 4 | Bishop | Action | +$1 +1 VP. Trash a card from hand: +1 VP per $2 of its cost. Each other player may trash a card from hand. |
| 4 | Clerk | Action–Reaction–Attack | +$2. Each other player with 5+ cards in hand puts one onto their deck. At the start of your turn, you may play this from your hand. |
| 4 | Investment | Treasure | Trash a card from hand. Choose: +$1; or trash this to reveal your hand for +1 VP per differently named Treasure. |
| 4 | Monument | Action | +$2 +1 VP. |
| 4 | Quarry | Treasure | $1. This turn, Action cards cost $2 less. |
| 4 | Tiara | Treasure | +1 Buy. This turn, gains may go onto your deck. You may play a Treasure from your hand twice. |
| 4 | Worker's Village | Action | +1 Card +2 Actions +1 Buy. |
| 5 | Charlatan | Action–Attack | +$3. Each other player gains a Curse. In games using this, Curse is also a Treasure worth $1. |
| 5 | City | Action | +1 Card +2 Actions; 1+ empty piles: +1 Card; 2+: also +1 Buy +$1. |
| 5 | Collection | Treasure | $2 +1 Buy. This turn, when you gain an Action card, +1 VP. |
| 5 | Crystal Ball | Treasure | $1. Look at the top card: you may trash it, discard it, or, if it's an Action or Treasure, play it. |
| 5 | Magnate | Action | Reveal your hand: +1 Card per Treasure in it. |
| 5 | Mint | Action | You may reveal a Treasure from hand to gain a copy. When you gain this, trash all non-Duration Treasures you have in play. |
| 5 | Rabble | Action–Attack | +3 Cards. Each other player reveals their top 3, discards Actions and Treasures, puts the rest back in any order. |
| 5 | Vault | Action | +2 Cards. Discard any number for +$1 each. Each other player may discard 2 cards to draw 1. |
| 5 | War Chest | Treasure | The player to your left names a card; gain a card up to $5 not named for War Chests this turn. |
| 6 | Grand Market | Action | +1 Card +1 Action +1 Buy +$2. Can't buy it with a Copper in play. |
| 6 | Hoard | Treasure | $2. This turn, when you gain a Victory card, if you bought it, gain a Gold. |
| 7 | Bank | Treasure | +$1 per Treasure in play (counting this). |
| 7 | Expand | Action | Trash a card from hand; gain one up to $3 more. |
| 7 | Forge | Action | Trash any number from hand; gain one costing exactly their total. |
| 7 | King's Court | Action | You may play an Action from your hand three times. |
| 8 | Peddler | Action | +1 Card +1 Action +$1. In your Buy phase, costs $2 less per Action you have in play. |

## 2. Architecture (decisions made; all generic)

1. **Card ids.**
   - Ids grow to 113 (59 + 27 + 25 + Platinum + Colony). `CardSet` gains Seaside and Prosperity.
   - `Counts` goes to **128 lanes** and `in_supply` to `u128`.
   - Measure with the bench before and after. If fast-mode throughput drops more than ~15%, the
     follow-up is per-game compact card ids (a game never uses more than ~30 distinct cards).
2. **Buy phase with treasure decisions.** Treasures stay auto-played when *choice-free and
   order-independent* (the `OnPlay` marking extends to Treasures): Copper, Silver, Gold,
   Platinum, Harem, Quarry, Collection, Hoard, Astrolabe, and Curse-as-Treasure. The others
   (Anvil, Investment, Crystal Ball, Tiara, War Chest, Bank) are played through a new
   `DecisionKind::PlayTreasure` (Choice: a treasure in hand, or Done). This comes before the
   `Buy` decision, since in 2nd edition you can't play treasures after buying. Bank's value
   depends on order, so Bank is a choice card whose bot default is "play it last".
3. **State-aware card properties.** `GameState::is_treasure(c)` for Curse under Charlatan, and a
   `GameState::cost(c)` that also covers Quarry (Actions −$2 per Quarry in play) and Peddler (−$2
   per Action in play during the Buy phase). Buy legality covers Grand Market (no Copper in play).
4. **Gain pipeline.** Every gain goes through one path that, after moving the card, runs
   "when-gain" triggers as frames:
   - reactions by the gainer: Watchtower (trash or topdeck);
   - reactions by others: Pirate (play from hand on any Treasure gain);
   - statics in play: Hoard, Collection, Tiara topdeck, Sailor, Monkey, Blockade;
   - the per-turn gain record (Smugglers reads the right-hand player's last turn).

   Buy-only triggers (Hoard's "if you bought it") get an `on_buy` flag. Mint's trash is a plain
   "when you gain this" (2022 errata), whoever gains it and however.
5. **Durations.**
   - Cards with future effects stay in play past cleanup, in a per-player `duration` zone.
     They still count as "in play" for Sea Chart and Peddler.
   - A fixed-size per-player list of pending start-of-turn effects (`(card, times, arg)`) holds
     the future effects.
   - Throne Room or King's Court on a Duration stays in play with it.
   - Start-of-turn effects resolve as frames before the action phase. This is also the hook for
     Clerk's "you may play this at the start of your turn".
6. **Mats and set-aside zones**, all per player:
   - Native Village mat (owner-private; opponents know the count);
   - Island mat (public; counts toward VP);
   - Haven and Blockade set-asides (tracked per Duration effect).
   - `determinize` pools the private ones for opponents.
7. **Attack immunity:** Lighthouse in play, alongside Moat in hand. Corsair's lingering effect is
   a per-player "trash the first Silver/Gold played" flag.
8. **Extra turns (Outpost):** cleanup draws 3 and the next turn is the same player's (not a 3rd
   in a row). Extra turns do not count as turns for the "fewer turns wins" tie-break.
9. **VP tokens:** a `vp_tokens` field per player, added to scores (Monument, Bishop, Collection,
   Investment).
10. **Platinum and Colony:** a per-game `colonies` flag. Colony emptying also ends the game.
    Random kingdoms follow the official rule: if a randomly chosen kingdom card is from
    Prosperity, use Colonies.
11. **Text format:** new lines for the duration zone, mats, VP tokens and colonies. Old files
    keep parsing.

## 3. Order of work (each step ends all green, like Intrigue)

1. **Foundation (overseer):**
   - Ids, sets, 128 lanes, `is_treasure`/`cost` statics, VP tokens, Platinum/Colony and the end
     condition.
   - Vanilla-ish cards: Bazaar, Worker's Village, Monument, City, Grand Market, Magnate.
   - Bench before and after.
2. **Prosperity:** treasure decisions and the gain pipeline.
   - Treasures: Anvil, Investment, Crystal Ball, Tiara, War Chest, Bank, Quarry, Collection,
     Hoard, Charlatan.
   - The rest of Prosperity: Peddler, Mint, Watchtower, Bishop, Clerk (full reaction after
     step 3), Rabble, Vault, Expand, Forge, King's Court.
3. **Seaside Durations:**
   - The framework, plus Haven, Lighthouse, Astrolabe, Fishing Village, Monkey, Blockade,
     Caravan, Sailor, Tide Pools, Corsair, Merchant Ship, Outpost, Pirate, Sea Witch, Tactician
     and Wharf.
   - Throne Room / King's Court with Durations, and Clerk's start-of-turn reaction.
4. **The rest of Seaside:** Native Village, Lookout, Sea Chart, Smugglers, Warehouse, Cutpurse,
   Island, Salvager, Treasure Map, Treasury.
5. **Integration:**
   - Sample strategies and card-set selection (Seaside, Prosperity) in the page, the Lab and the
     CLI.
   - Colony games in the UI.
   - A search for Double Witch counters across all sets.

## 4. Card tests

- **Seaside**: Bazaar, Cutpurse, Lookout, Caravan, Wharf, Merchant Ship, Warehouse, Salvager,
  Treasure Map, Lighthouse, Tactician (+Throne Room), Fishing Village, Island, Haven (+Throne
  Room), Native Village, Smugglers, Outpost, Treasury.
  - Not covered (1st edition only): Seahag, Explorer, Navigator, Ambassador, Ghost Ship,
    Pirate Ship, Pearl Diver, Embargo.
  - Edition differences: Lighthouse and Wharf are unchanged in 2nd edition; Treasury changed
    (it's "may", at the end of the Buy phase); Haven's set-aside card returns to hand.
- **Prosperity**: Platinum, Monument, Workers Village, Bishop, City, Mint, Expand, Forge, Bank,
  Colony, Quarry, Peddler, Hoard, Rabble, Vault, King's Court (+Duration), Watchtower, Grand
  Market.
  - Not covered (1st edition only): Loan, Venture, Mountebank, Talisman, Goons, Counting House,
    Trade Route (+tokens), Royal Seal.
  - Mint changed in 2nd edition: it trashes only non-Duration Treasures, and when it is gained
    (any gain), not only bought.
- **2nd-edition-only cards** (tests written from the card text): Monkey, Sea Chart, Blockade,
  Sailor, Tide Pools, Corsair, Pirate, Sea Witch, Astrolabe, Anvil, Clerk, Investment, Tiara,
  Charlatan, Collection, Crystal Ball, Magnate and War Chest.
- Test files: `crates/engine/tests/seaside_cards.rs` and `prosperity_cards.rs`.

## 5. Performance log

Benchmarks (release build, 32 threads):

- **Full bots:** `dominion-sim match strategies/big_money_ultimate.toml strategies/double_witch.toml
  --games 200000 --kingdom "Sentry,Militia,Witch,Village,Smithy,Market,Cellar,Moat,Workshop,Remodel"`.
- **Fast mode:** `dominion-sim match bench/strategies/fast_big_money.toml
  bench/strategies/fast_double_witch.toml --games 1000000 --kingdom auto`. Both strategies have
  `win_this_turn = false`, which is where most of full mode's cost goes (Big Money has no actions,
  so `search_play` doesn't matter for it).

| Point | Full bots | Fast mode |
|---|---|---|
| Before Seaside/Prosperity (59 ids) | 282k/s | 1.75M/s |
| Step 1: 113 ids, 128-lane Counts, sparse iteration | 253k/s | 1.79M/s |
| Step 2: treasure decisions, gain pipeline, Prosperity | 228k/s | 1.60M/s |
| Step 3: Duration framework, Seaside Durations | 215k/s | 1.59M/s |
| Step 4: the last 10 Seaside cards (all 27 ready) | 207k/s | 1.53M/s |
| Step 5: integration; `ChoiceBuf` left uninitialized | 212k/s | 1.60M/s |

Step 2's ~10% has no single hot spot (GameState grew only 7872 -> 8192 bytes, gain triggers are
gated by one mask test); it needs a real profiler. The next structural option is per-game compact
card ids (a game never uses more than ~30 distinct cards), which shrinks every `Counts`.

Step 4 grew `GameState` from 8576 to 9408 bytes (+832): three new persistent per-player zones
(the Native Village and Island mats, each a capped `SmallMultiset<20>`; Smugglers' per-player
`last_turn_gains`, a `SmallMultiset<8>`) plus two `Frame` fields used by Smugglers/Treasure Map
(`STACK_CAP` = 48, so 2 bytes/slot adds up) and a handful of `TurnState` bookkeeping bytes. Full
bots dropped ~2-3% (215k/s -> ~208-211k/s over several runs), right at the edge of the ~2%
budget; this is the unavoidable cost of three more always-present zones, not runtime work (they're
gated to near-zero CPU cost when unused: one bitmask test in `GameState::gain`, one in
`push_treasury_offers`). The fast-mode number was remeasured by the overseer with the
benchmark files now in `bench/strategies/` (they previously lived outside the repo, which is why
it couldn't be reproduced).

**Totals since Base + Intrigue:** full bots 282k/s to 207k/s (-27%), fast mode 1.75M/s to 1.53M/s
(-13%). A dedicated performance pass (profiler, then possibly per-game compact card ids) is the
next item once the sets are integrated.

## 6. Rules audit against the wiki (2026-10)

Every Seaside and Prosperity card, Platinum and Colony were re-read against the DominionStrategy
wiki (card text at its latest version, FAQ sections, and the Duration, Attack, Attack immunity,
Reaction, Gain, Set aside, Reveal and Clean-up pages). Costs, types, vanilla bonuses and pile
sizes were compared by script (all equal). Tests: the set's test file (`crates/engine/tests/*_cards.rs`), and `cross_set.rs` for rules that cross sets (formerly `rules_audit.rs`).

**Fixed**

- **Mint** trashes the non-Duration Treasures in play when it is *gained* (2022 errata), not only
  bought; Tiara's "put it onto your deck" is still offered for that Mint.
- **Quarry, Hoard, Collection, Tiara** are "this turn" effects: they count each *play* (twice
  with Tiara: Quarry takes $4 off, Hoard gains two Golds, Collection gives 2 VP) and survive the
  card leaving play (a Mint trashing them). `GameState::plays_this_turn`.
- **Colony pile**: 8 with 2 players, 12 with 3 or more (it was the Province count: 15 and 18
  with 5 and 6 players).
- **Outpost's extra turn** does not count for the "fewer turns wins" tie-break.
- **Investment** played twice (Tiara): the second play cannot trash it again, so it reveals and
  scores nothing.
- **Monkey** draws only while its effect lasts ("until your next turn"; it was still in play, and
  still drawing, during its owner's next turn), and once per play (a Throne Roomed Monkey draws
  twice per gain). It reads its pending next-turn entry.
- **Corsair and Blockade**: "attack immunity only works if it is in force as the Attack is
  played". A Moat revealed (or a Lighthouse in play) when they were played now protects from the
  Silver/Gold trashing and from the Curse (`PlayerState::shielded`, kept in the text format);
  a Lighthouse played later does not. A Silver or Gold played by an effect (Crystal Ball, Tiara)
  is trashed by Corsair too.
- **Pirate**: each copy in hand is its own reaction (it offered one per player), players react
  in turn order from the turn's player, and a Pirate played on another player's turn no longer
  counts as one of that turn's played Actions (Conspirator) or as held for that turn's cleanup.
- **Clerk** counts the victim's hand when it resolves, after a Diplomat has been revealed (a
  hand left with 4 cards is not affected).
- **Herbalist**: counts plays ("once this turn, when you discard a Treasure", 2022): a Throne
  Roomed Herbalist puts back up to two; and it cannot put an Astrolabe that stays in play onto
  the deck (that was a bug: the Astrolabe's next-turn bonus would stay scheduled).
- Charlatan's Curse-as-Treasure also applies to **Ironworks** (+$1), **Replace** (onto the
  deck) and **Courtier** (two types); see the Intrigue plan.
- **Gains onto the deck can be reacted to** (Gain page: a card gained "onto your deck" is gained
  there directly). `Zone::DeckTop` is a one-card zone, the known top of the deck, so the generic
  `Select`/`YesNo` frames work on it: Watchtower may trash Bureaucrat's Silver or any of Treasure
  Map's four Golds (one offer per Gold), and Sailor plays a Duration gained onto the deck from the
  top. Putting a card onto the deck that is already there is not offered (Watchtower's and Tiara's
  topdeck are skipped for such a gain). **Replace** now gains to the discard pile as usual
  (Replace FAQ: "first gained to your discard pile ... then it is moved to the top of your deck
  if possible"), runs the gain's reactions, and only then moves an Action or Treasure onto the
  deck if it is still where it was gained (`Then::MoveSubjectIfPresent`, remembering how many
  copies the discard pile held): a Watchtower that trashed it, or a Tiara that already put it
  back, leaves nothing to move, and an older copy of the same card is never taken. Cards that gain
  straight onto the deck today: Bureaucrat, Treasure Map, nothing else (a test pushes a synthetic
  gain frame for Sailor).
- **Sailor with Throne Room or King's Court**: "once this turn" is per play, so a Throne Roomed
  Sailor can play two gained Durations and a King's Courted one three; two Sailors give one each.
  `PendingDuration::used` is now a count of spent uses (against `times`); an offer spends a use
  whether or not it is taken, and one gained card gets one offer (two Sailors cannot both play
  it, Sailor FAQ). Text format: `Sailor x2 (used 1)`; the old `Sailor (used)` still loads (all of
  its uses spent).
- **Moat is a choice** (Moat FAQ: "you *may* reveal"; "you may reveal Moat for as many of them as
  you want"). An Attack that another player could answer (a Moat in hand with no Lighthouse in
  play, or a Diplomat they may reveal) first opens a reaction window: its own effects wait under
  it (`FrameKind::AttackBody`), and each such player gets a `YesNo` per Attack, in turn order
  (`Then::BlockAttack`, `cards::BLOCK_REACTIONS`; revealing sets a bit of `TurnState::moated`,
  which `victims` reads). A player's Diplomat comes first, so a Moat it draws is still offered
  (the reactions can be used in either order, the result is the same). Nobody is asked when no
  other player can react, so games without Moat/Diplomat in hand are not slowed. Lighthouse still
  protects with no decision. The bot reveals always, except against **Minion** when it holds 5 or
  more cards and a new hand of 4 is worth more money than its hand (the comparison Minion's own
  default makes). **Scrying Pool**: the bot also always reveals: the Pool's owner picks, so
  having its top card judged cannot help the victim.

**Checked, no change**: Anvil, Watchtower, Bishop, Monument, Worker's Village, City, Crystal Ball,
Magnate, Rabble, Vault, War Chest, Grand Market, Bank, Expand, Forge, King's Court, Peddler,
Platinum; Haven, Lighthouse (an Attack-immunity card in play), Native Village, Astrolabe, Fishing
Village, Lookout, Sea Chart, Smugglers, Warehouse, Caravan, Cutpurse, Island, Salvager, Tide
Pools, Treasure Map, Bazaar, Merchant Ship, Outpost, Sea Witch, Tactician, Treasury (the January
2026 errata went back to "when you discard this from play, if you didn't gain a Victory card in
your Buy phase"; the end-of-Buy-phase offer here is the same thing), Wharf, Throne Room and
King's Court with Durations.

**Open (not fixed)**

- Watchtower's two offers (trash, then put onto the deck) look for the gained card's *name* in
  the zone, so with an older copy of it in the discard pile, a Watchtower that trashed the new
  card may still be offered to put the older one onto the deck (the card is the same).
- A Cutpurse or Bureaucrat checks the victim's hand when played; a Diplomat reveal that then
  draws a Copper or Victory card changes nothing for them (the Clerk, Militia and Minion check
  late).
- Harem was renamed Farm in 2023 (same card); the name here is the 2nd-edition printing's.
