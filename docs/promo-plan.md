# Promo cards and the order of start-of-turn effects

Promo cards are a set of their own (`CardSet::Promo`; page checkbox "Promo", wasm set-mask bit 32,
Lab set name "Promo"; off by default, like Alchemy). Today it holds Prince. The same work added
the player's choice of the order of start-of-turn effects, which Prince needed.

> Card text and rulings are from memory: the wiki blocks automated fetches (and the local
> `wiki/export` is empty). The tests are the specification. Items marked *unconfirmed* are the
> ones to check against the wiki page for Prince.

## 1. The order of start-of-turn effects

At the start of a turn the player may have several effects waiting, and the rules let them pick the
order: each pending Duration entry (`PlayerState::pending_durations`), Clerk's "you may play this
from your hand", and Prince's set-aside cards.

- `push_turn_start_frames` moves them into `TurnState::start_pending` (a `StartList`) and pushes one
  `FrameKind::StartOrder` frame. Moving them out keeps what the turn schedules for the *next* one
  (a Prince'd Caravan) from mixing with them.
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
  different cards (a Throne Room'd Haven) are one choice, resolved in list order; likewise a
  Prince'd Haven and a Haven Duration share the name.
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

"You may set this aside. If you do, set aside an Action card from your hand costing up to $4. At
the start of each of your turns, play that Action, setting it aside again when you discard it from
play. (Stop playing it if you fail to set it aside on a turn you play it.)"

State: `PlayerState::prince_cards` (a `SmallMultiset` of the held cards, one per Prince) and
`princes_empty` (Princes holding nothing). Both are public, count in `all_cards()` (the Prince and
its card are each a card the player owns: VP, Gardens) and are in `PlayerView`
(`prince_cards_of`, `princes_empty_of`). `TurnState::prince_played` records the cards a Prince played
this turn.

- Setting aside is a `YesNo` over the Prince in play; yes puts it in `princes_empty` and a `Select`
  of an Action in hand follows (`Filter::ActionCostUpTo(4)`: the *current* cost, so Bridge and
  Quarry count; no Potion). If none is eligible the Prince stays set aside holding nothing.
- Each held card is a start-of-turn effect (section 1). Playing it takes it out of `prince_cards`
  (the Prince goes to `princes_empty` meanwhile), puts it in play, logs "Player N's Prince plays
  X" (`Event::StartPlay`) and resolves it like any play. It uses no Action.
- At cleanup, each card in `prince_played` still in play and not held there by a Duration is set
  aside with its Prince again instead of being discarded. One that left play (trashed; Island) has
  no card to return: the Prince stays empty and stops.
- Duration (*unconfirmed*): a Prince'd Duration stays in play through its next turn, so it is not
  discarded from play at the first cleanup and the Prince stops; at the next cleanup it is
  discarded normally.
- Throne Room on Prince: the first play sets it aside; the second finds it no longer in play
  (`YesNo` skipped as nothing to act on), so it does nothing.
- Gap: the cards are tracked by name, not identity. A hand-played copy of the Prince'd card that
  is still in play when the Prince'd one has been trashed would be set aside in its place.

**Bots.** Set it aside when an Action costing up to $4 is in hand. The card: the strategy's `[[play]]`
rules first, then by what it gives (3 per card, 2 per +Action/+Buy/+$, a little for an Attack, a
Duration last since the Prince would stop after one play). `play_rank_of` orders plays within a turn
and doesn't say what is best to hold: a held terminal costs nothing (no Action used). These
choices follow the rules, not the search: a turn-long search can't see what a card played at every
later turn is worth. Example: `strategies/prince_smithy.toml`.

## 3. Performance

`dominion-sim match big_money_ultimate double_witch --games 400000` (32 threads, best of 6):
1.73M games/s before, 1.71M after (-1%; `GameState` 9600 to 9728 bytes). With the 10-card
Sentry/Militia/Witch kingdom: 1.275M before, 1.29M after. A Seaside Duration pairing (Sea Witch +
Haven vs Wharf + Tide Pools + Sailor + Caravan, 100000 games): 54k games/s before, 57k after.
Two things mattered: the state hash used by the search (`turn_hash`) walks `TurnState`, so
`StartList` hashes only its live entries (hashing a plain 21-entry array made every search node
slower); and the Prince code in the hot per-turn and per-select functions lives in `#[cold]`
helpers (a branch and loop in `cleanup`, and in the Select filter tests, cost 3-5%).
