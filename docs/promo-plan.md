# Promo cards and the order of start-of-turn effects

Promo cards are a set of their own (`CardSet::Promo`; page checkbox "Promo", wasm set-mask bit 32,
Lab set name "Promo"; off by default, like Alchemy). Today it holds Prince, Walled Village, Envoy and
Captain. The first piece of work added the player's choice of the order of start-of-turn effects,
which Prince needed; Captain's start-of-turn play joins it.

> The Prince rules are the official rules as provided by the user. The rulings for Walled Village,
> Envoy and Captain are from the DominionStrategy wiki (section 3). The tests are the specification.

## 1. The order of start-of-turn effects

At the start of a turn the player may have several effects waiting, and the rules let them pick the
order: each pending Duration entry (`PlayerState::pending_durations`), Clerk's "you may play this
from your hand", Prince's set-aside cards and Captain's play (a pending Duration entry).

- `push_turn_start_frames` moves them into `TurnState::start_pending` (a `StartList`) and pushes one
  `FrameKind::StartOrder` frame. Moving them out keeps what the turn schedules for the *next* one
  (a Clerk-played Caravan) from mixing with them.
- Each run of `StartOrder` takes the next effect. When two or more different effects remain and one
  is order-sensitive it asks `DecisionKind::StartOrder` (choices: `Choice::Card`, one per distinct
  effect; the effect's card, or for a Prince the card it plays). The effect resolves fully, then
  the rest are asked again.
- Order-sensitive (`cards::start_order_sensitive`): acts on the hand (Sea Witch, Tide Pools: discard
  2; Sailor: may trash), adds to the hand (Haven, Blockade: a card returns; Pirate: a Treasure),
  plays a card (Prince's card, Captain's pick, Clerk). Everything else is a plain bonus
  (`cards::duration_bonus`).
- Not asked: one effect; only plain bonuses; and, a step further, plain bonuses that draw nothing
  (+Actions/+Buys/+$, Outpost, a Throne Room kept in play) resolve first without a question, since
  nothing depends on when they happen. So Sea Witch with Lighthouse is not a question, Sea Witch
  with Caravan is (the draw changes what the discard can choose from).
- Entries with the same name are one choice and the first resolves. Gap: two Havens holding
  different cards (a Throne Room'd Haven) are one choice, resolved in list order.
- Clerk is one merged entry (`times` = copies in hand at the start of the turn) resolved one copy at
  a time; it is skipped if no Clerk is in hand when it resolves (discarded or trashed first, which
  is why the order matters).
- `times` (Throne Room, King's Court) is honoured: Sea Witch, Tide Pools, Sailor and Pirate now push
  their discard/trash/gain once per play (they did it once however many times they were played).
- Text format: see the module doc of `text.rs`. A state saved mid-ordering lists the effects still
  waiting in `durations:`; on load the turn starts its start-of-turn resolution again with them.

**Bots.** The default order (`Strategy::choose_start_order`): cards back into the hand (Haven,
Blockade, Pirate), then plain bonuses and draws, then plays (Clerk, a Prince's card, Captain), then the
effects that discard or trash (Sea Witch, Tide Pools, Sailor), so those choose from the fullest
hand. With `search_play` the decision is also searched (`eval::is_start_effect_decision` is part of
`is_play_decision`, so the Lab's analysis and the search's playouts try every order), but the bot
itself searches only the orders with a play in them (Clerk, a Prince's card or Captain): searching the
draw/return/discard orders made Seaside Duration games 5x slower for under 1 point of win rate
(`START_ORDER_SEARCH_ALL` in `strategy.rs` turns it on).

## 2. Prince ($8, Action)

Rules: the official rules as provided by the user (not confirmed against the wiki, which the
tooling can't fetch).

- "The turn you play Prince, you may set aside a non-Duration, non-Command Action card from your
  hand costing $4 or less; then every turn after that you play the Action at the start of the
  turn." (No Command cards exist in our sets.) Cost is the current cost, so a Bridge or Quarry
  makes a card that normally costs more eligible; cards costing a Potion (or Debt) never are.
- Without a card the Prince is discarded in Clean-up; otherwise it stays in play for the rest of
  the game.
- The set-aside card stays set aside and is played without entering play. If it tries to move
  itself it stays set aside and fails to move; a bonus conditional on moving is lost, one that
  isn't is kept. Playing it uses no Action, is mandatory, and doesn't happen on the turn it was set
  aside. The start of the turn is part of the Action phase; it also happens on extra turns.
- With Throne Room (King's Court) the Prince sets aside two (three) cards, played at the start of
  every turn, and the Throne Room and the Prince both stay in play for the rest of the game.
- At the end of the game the Prince and its cards count for scoring (they are part of
  `all_cards()`).

State: `PlayerState::prince_cards` (a `SmallMultiset`: every card the player's Princes hold, one
entry per card) and `prince_stays` (the Princes and Throne Rooms / King's Courts that hold cards,
which cleanup keeps in play: one slot per physical card, so a Throne Room'd Prince is one Prince
holding two cards). Both are public and in `PlayerView` (`prince_cards_of`). The Prince and the
Throne Room are ordinary cards in `in_play`; the held cards are counted by `all_cards()` (VP,
Gardens).

- Playing Prince is an optional `Select` of an Action in hand (`Filter::ActionCostUpTo(4)`,
  `Then::PrinceHold`: non-Duration, current cost, no Potion). A pick moves the card into
  `prince_cards` and marks the Prince (and a multiplier, via the `MultiplierFinalize` tally) as
  staying. Each Throne Room resolution is another Select.
- Each held card is a start-of-turn effect (section 1), built when the turn starts, so the card
  isn't played on the turn it is set aside. Playing it logs "Player N's Prince plays X"
  (`Event::StartPlay`) and resolves its effects with `TurnState::prince_play` set: the card is not
  in play, so Mining Village's "trash this" question isn't asked (no +$2), Treasure Map doesn't
  trash itself (its second Map is still trashed from hand, but no Golds), Island stays while its
  hand card still goes to the mat. The flag is per play, so a copy of the same card that is in
  play is never the one moved.
- Text format: `prince:` lists the held cards, `prince stays:` the Prince / Throne Room / King's
  Court cards that stay in play (the `in play:` line has them too). A `prince:` line with no
  `prince stays:` puts one Prince in play to stay.
- Gap: `prince_stays` is kept by name, so a hand-played extra Prince that holds nothing is
  discarded, but is kept if the stays count is higher than the Princes actually in play and one
  arrives later (not reachable in normal play).

**Bots.** Take the best eligible Action when one is in hand: the strategy's `[[play]]` rules first,
then by what it gives (3 per card, 2 per +Action/+Buy/+$, a little for an Attack). `play_rank_of`
orders plays within a turn and doesn't say what is best to hold, and a held card costs no Action.
Prince's choice follows the rules, not the search: a turn-long search can't see what a card
played at every later turn is worth. Example: `strategies/prince_smithy.toml`.

## 3. Walled Village, Envoy, Captain

Rulings come from the local copy of the DominionStrategy wiki (pages "Walled Village", "Envoy",
"Captain", "Prince", "Throne Room"; current versions: Walled Village 2016, Envoy 2016, Captain 2019).
The generic parts are reused: Envoy is Advisor's frame with 5 cards, Walled Village is a Clean-up
`YesNo` like Treasury's, and Captain plays a Supply card with the same "play without moving the card"
flag as Prince's held card.

### Walled Village ($4, Action: +1 Card, +2 Actions)

- "At the start of Clean-up, if you have this and no more than one other Action card in play, you may
  put this onto your deck." (Official FAQ.) The check is made when the offer comes, so the count is
  of Action cards in play at that moment: Durations still in play from earlier turns count (a Fishing
  Village, a Prince that stays), Action cards that left play do not, a card played from the Supply by
  Captain or held by a Prince is not in play. Two Walled Villages and nothing else: either or both may go
  (the second is checked after the first left). Three: none (each sees two others).
- It has to be in play to go, and "may": a `YesNo` (`Act::Topdeck` from `Zone::InPlay`), one per copy.
  A Throne Room'd Walled Village is one card in play. It does not matter whether it helped.
- Order of the Clean-up offers (`push_end_of_turn_offers`): Alchemist (also "at the start of Clean-up"),
  Walled Village, then Treasury and Herbalist (which trigger when discarded from play). A topdecked
  Alchemist leaves play and so can make room for Walled Village ("remove some at the start of Clean-up",
  wiki), never take it away; and Treasury is still in play when Walled Village counts, as the FAQ says it
  is. This moves Alchemist ahead of Treasury compared with before (they only differ in which of the two
  ends up on top of the deck).
- Marked choice-free: the offer is a Clean-up decision, not a play decision (like Alchemist).
- Bot: topdeck when allowed (the default for `YesNo` `Act::Topdeck`).

### Envoy ($4, Action)

- "Reveal the top 5 cards of your deck. The player to your left chooses one. Discard that one and put the
  rest into your hand." (Official FAQ.) With fewer than 5: reveal as many as you can after shuffling the
  discard pile in; the opponent still discards one of what was revealed (even a single card), the rest
  go to the hand. Nothing to reveal: nothing happens. The -1 Card token errata is irrelevant (no tokens).
- Implemented as Advisor with `max: 5` (`RevealTop` then a `Select` by `chooser` = the player to the left,
  `Then::RevealedToHand`); with Throne Room it is done twice. Terminal, not an Attack, marked order-sensitive
  like Advisor (it reveals from the deck).
- Bot: the player to the left discards the card that is best for the Envoy player (Advisor's rule,
  `Strategy::card_value`), which is denial: the strategy's own gain list is not consulted.

### Captain ($6, Action-Duration-Command)

- "Now and at the start of your next turn: Play a non-Duration, non-Command Action card from the Supply
  costing up to $4, leaving it there." There is no official FAQ; the wiki's clarifications:
  - The card stays in the Supply (not in play): a card that tries to move itself fails to (Island stays,
    Treasure Map is not trashed, Mining Village is not trashed so its +$2 is lost); Workshop may gain
    itself and Lurker may trash itself, since those move cards in the Supply. This is the same
    `TurnState::prince_play` flag as for Prince's held card, set around the play (`PlayEffects` frame with
    `zone: Supply`). Playing it uses no Action.
  - Only visible Supply cards: an empty pile is not offered, and neither are non-Supply cards (Rewards, the
    Ferryman pile). Cost reduction applies (a Market is $4 under a Bridge); Potion costs never qualify (the
    same filter as Prince, `Filter::ActionCostUpTo(4)`, which also excludes Durations and so Command cards).
  - The second turn is a fresh choice (the same card again if a copy is still there).
  - If nothing qualifies, Captain still stays in play and tries again next turn.
  - It is Duration-typed in play like any Duration: it stays until the start of the next turn and is then
    discarded; a Captain that plays a Throne Room (from the Supply) that plays a Caravan keeps the Caravan
    in play longer, which is the Caravan's own bookkeeping.
- Throne Room (King's Court) on Captain: two (three) picks now, the Throne Room stays in play with it, and the
  pending entry has `times` 2 (3): two (three) picks at the start of the next turn.
- The pick is mandatory (no `Pass`), as "play a card" is. This is a reading: the wiki does not say it is
  optional.
- Implemented: `Select` (`zone: Supply`, `act: Play`, `Then::PlayPicked { times: 1 }`; a play from the Supply
  neither removes the card nor logs a plain play) and a `PendingDuration` for the next turn, scheduled by the
  generic Duration tail. The start-of-turn half is a start-of-turn effect (section 1), order-sensitive, so it
  is ordered against a Prince's card, Haven and the others. The log line is "Player N's Captain plays X"
  (`Event::StartPlay`, as for Prince and Farmhands).
- Gap: a Throne Room / King's Court played from the Supply by Captain is not in play, so it does not stay
  with a Duration it plays; a real Throne Room in play at the same time is not confused with it.
- Bot: the strategy's `[[play]]` rules first, in their order; otherwise the best line of the turn found by the
  turn search (when `search_play` is on and there is a real choice), otherwise the most useful card (3 per
  card, 2 per +Action/+Buy/+$, a little for an Attack, as for Prince). Inside the search's playouts only the
  rules and the score are used. Not valued by `crates/search`'s next-turn heuristic yet (the Captain's second
  play is invisible to a search that ends with this turn).

### Card ids and `Counts`

Three more cards took the id count to 161, one past the 160 lanes of `Counts`. They are ids 126 to 128, next
to Prince (the Cornucopia & Guilds ids moved up by three; the Rewards are the last six). `Counts` has 192
lanes now (every `Counts` is 32 bytes bigger, and a `GameState` holds dozens). Measured (`big_money_ultimate`
vs `double_witch`, 200000 games, best of 8, runs of the old and new binaries interleaved): 1.78M games/s
before, 1.71M with only the wider `Counts`, 1.70M with everything (about -4%). 176 lanes with an alignment of
16 and 168 with an alignment of 8 were 5-7% slower (the widths the vectorized `Counts` loops like are
multiples of 32), so 192 it is. Getting back under 161 ids would need a card removed or a smaller `Counts`.

## 4. Performance

`dominion-sim match big_money_ultimate double_witch --games 400000` (32 threads, best of 6):
1.73M games/s before, 1.71M after (-1%; `GameState` 9600 to 9728 bytes). With the 10-card
Sentry/Militia/Witch kingdom: 1.275M before, 1.29M after. A Seaside Duration pairing (Sea Witch +
Haven vs Wharf + Tide Pools + Sailor + Caravan, 100000 games): 54k games/s before, 57k after.
Two things mattered: the state hash used by the search (`turn_hash`) walks `TurnState`, so
`StartList` hashes only its live entries (hashing a plain 21-entry array made every search node
slower); and the Prince code in the hot per-turn and per-select functions lives in `#[cold]`
helpers (a branch and loop in `cleanup`, and in the Select filter tests, cost 3-5%).

With the three cards (section 3): 1.78M games/s before, 1.70M after (-4%), nearly all of it the 20% wider
`Counts`; the cards' own code is a few rarely taken branches (a `YesNo` check on `WALLED_VILLAGE`, a
`zone == Supply` test when a card is played from a frame, one more `in_supply` test at the end of the Buy
phase).
