# Promo cards and the order of start-of-turn effects

Promo cards are a set of their own (`CardSet::Promo`; page checkbox "Promo", wasm set-mask bit 32,
Lab set name "Promo"; off by default, like Alchemy). Today it holds Prince. The same work added
the player's choice of the order of start-of-turn effects, which Prince needed.

> The Prince rules are the official rules as provided by the user. The tests are the
> specification.

## 1. The order of start-of-turn effects

At the start of a turn the player may have several effects waiting, and the rules let them pick the
order: each pending Duration entry (`PlayerState::pending_durations`), Clerk's "you may play this
from your hand", and Prince's set-aside cards.

- `push_turn_start_frames` moves them into `TurnState::start_pending` (a `StartList`) and pushes one
  `FrameKind::StartOrder` frame. Moving them out keeps what the turn schedules for the *next* one
  (a Clerk-played Caravan) from mixing with them.
- Each run of `StartOrder` takes the next effect. When two or more different effects remain and one
  is order-sensitive it asks `DecisionKind::StartOrder` (choices: `Choice::Card`, one per distinct
  effect; the effect's card, or for a Prince the card it plays). The effect resolves fully, then
  the rest are asked again.
- Order-sensitive (`cards::start_order_sensitive`): acts on the hand (Sea Witch, Tide Pools: discard
  2; Sailor: may trash), adds to the hand (Haven, Blockade: a card returns; Pirate: a Treasure),
  plays a card (Prince's card, Clerk). Everything else is a plain bonus (`cards::duration_bonus`).
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
Blockade, Pirate), then plain bonuses and draws, then plays (Clerk, a Prince's card), then the
effects that discard or trash (Sea Witch, Tide Pools, Sailor), so those choose from the fullest
hand. With `search_play` the decision is also searched (`eval::is_start_effect_decision` is part of
`is_play_decision`, so the Lab's analysis and the search's playouts try every order), but the bot
itself searches only the orders with a play in them (Clerk or a Prince's card): searching the
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

## 3. Performance

`dominion-sim match big_money_ultimate double_witch --games 400000` (32 threads, best of 6):
1.73M games/s before, 1.71M after (-1%; `GameState` 9600 to 9728 bytes). With the 10-card
Sentry/Militia/Witch kingdom: 1.275M before, 1.29M after. A Seaside Duration pairing (Sea Witch +
Haven vs Wharf + Tide Pools + Sailor + Caravan, 100000 games): 54k games/s before, 57k after.
Two things mattered: the state hash used by the search (`turn_hash`) walks `TurnState`, so
`StartList` hashes only its live entries (hashing a plain 21-entry array made every search node
slower); and the Prince code in the hot per-turn and per-select functions lives in `#[cold]`
helpers (a branch and loop in `cleanup`, and in the Select filter tests, cost 3-5%).
