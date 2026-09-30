# Plan: Seaside and Prosperity (2nd edition)

**Scope.** Seaside 2nd edition (27 kingdom cards) and Prosperity 2nd edition (25 kingdom cards
plus Platinum and Colony), mixable with Base and Intrigue. 1st-edition-only cards are not
included:
- Seaside: Ambassador, Embargo, Explorer, Ghost Ship, Navigator, Pearl Diver, Pirate Ship, Sea Hag.
- Prosperity: Contraband, Counting House, Goons, Loan, Mountebank, Royal Seal, Talisman, Trade Route,
  Venture.

> Card text is from memory (the wiki blocks automated fetches). The ported tests and a check
> against the physical cards are the specification.

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
| 4 | Quarry | Treasure | $1. While in play, Action cards cost $2 less. |
| 4 | Tiara | Treasure | +1 Buy. This turn, gains may go onto your deck. You may play a Treasure from your hand twice. |
| 4 | Worker's Village | Action | +1 Card +2 Actions +1 Buy. |
| 5 | Charlatan | Action–Attack | +$3. Each other player gains a Curse. In games using this, Curse is also a Treasure worth $1. |
| 5 | City | Action | +1 Card +2 Actions; 1+ empty piles: +1 Card; 2+: also +1 Buy +$1. |
| 5 | Collection | Treasure | $2 +1 Buy. This turn, when you gain an Action card, +1 VP. |
| 5 | Crystal Ball | Treasure | $1. Look at the top card: you may trash it, discard it, or, if it's an Action or Treasure, play it. |
| 5 | Magnate | Action | Reveal your hand: +1 Card per Treasure in it. |
| 5 | Mint | Action | You may reveal a Treasure from hand to gain a copy. When you buy this, trash all non-Duration Treasures you have in play. |
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

   Buy-only triggers (Mint's trash, Hoard's "if you bought it") get an `on_buy` flag.
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
   in a row). Extra turns count as turns for the tie-break.
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

## 4. Porting the C++ tests (1st edition)

- **Seaside** (`SeasideCardsTests.cpp`), ported: Bazaar, Cutpurse, Lookout, Caravan, Wharf,
  MerchantShip, Warehouse, Salvager, TreasureMap, Lighthouse, Tactitian (+ThroneRoom),
  FishingVillage, Island, Haven (+ThroneRoom), NativeVillage, Smugglers, Outpost, Treasury.
  - Not ported (1st edition only): Seahag, Explorer, Navigator, Ambassador, GhostShip,
    PirateShip, PearlDiver, Embargo.
  - Differences to adapt: Lighthouse and Wharf are unchanged in 2nd edition; Treasury changed
    (it's "may", at the end of the Buy phase); Haven's set-aside card returns to hand.
- **Prosperity** (`PropserityCardsTests.cpp`), ported: Platinum, Monument, WorkersVillage,
  Bishop, City, Mint, Expand, Forge, Bank, Colony, Quarry, Peddler, Hoard, Rabble, Vault,
  KingsCourt (+Duration), Watchtower, GrandMarket.
  - Not ported (1st edition only): Loan, Venture, Mountebank, Talisman, Goons, CountingHouse,
    TradeRoute(+Tokens), RoyalSeal. LoseTrack is checked when ported.
  - Mint changed in 2nd edition: its on-buy trash skips Durations.
- **New tests** for the 2nd-edition-only cards: Monkey, Sea Chart, Blockade, Sailor, Tide Pools,
  Corsair, Pirate, Sea Witch, Astrolabe, Anvil, Clerk, Investment, Tiara, Charlatan, Collection,
  Crystal Ball, Magnate and War Chest.
- Test files: `crates/engine/tests/seaside_port.rs` and `prosperity_port.rs`.

## 5. Performance log

Benchmark: `dominion-sim match big_money_ultimate double_witch --games 200000` on a Base-only
kingdom (full bots), plus a fast-mode (rule-order, no win lookahead) Double Witch vs Big Money
match at 1M games. 32 threads.

| Point | Full bots | Fast mode |
|---|---|---|
| Before Seaside/Prosperity (59 ids) | 282k/s | 1.75M/s |
| Step 1: 113 ids, 128-lane Counts, sparse iteration | 253k/s | 1.79M/s |
| Step 2: treasure decisions, gain pipeline, Prosperity | 228k/s | 1.60M/s |
| Step 3: Duration framework, Seaside Durations | 215k/s | 1.59M/s |

Step 2's ~10% has no single hot spot (GameState grew only 7872 -> 8192 bytes, gain triggers are
gated by one mask test); it needs a real profiler. The next structural option is per-game compact
card ids (a game never uses more than ~30 distinct cards), which shrinks every `Counts`.
