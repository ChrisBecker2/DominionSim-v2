# Plan: Cornucopia & Guilds (2nd edition, the 2024 box)

Cornucopia & Guilds is a set of its own (`CardSet::CornucopiaGuilds`; page checkbox "Cornucopia &
Guilds", wasm set-mask bit 64, Lab set name "Cornucopia & Guilds"; off by default, like Alchemy and
Promo: the shipped strategies know nothing about Coffers). 26 kingdom cards and 6 Rewards.

> **Status: implemented** (all 26 kingdom cards, the 6 Rewards, Coffers, Overpay, the Bane and
> Ferryman piles, bots, page, Lab, three sample strategies). Card tests:
> `crates/engine/tests/cornucopia_guilds_cards.rs`; bots: `crates/sim/tests/cornucopia_defaults.rs`;
> Lab: `crates/evolve/tests/reward_rules.rs`; page: `web/test_log.mjs`. Samples:
> `strategies/young_witch_bm.toml`, `baker_guild.toml`, `joust_courser.toml`.
>
> Card text and rulings are from the DominionStrategy wiki's pages for the cards and mechanics
> (named per ruling below), using the current text (2nd edition, 2024) and the newest FAQ. The tests
> are the specification. The removed first-edition cards (Fortune Teller, Doctor, Masterpiece,
> Farming Village, Horse Traders, Taxman, Tournament and its Prizes, Harvest) are not implemented.

## 1. Cards

C = Coffers. "+" after a cost = Overpay. Order = the `OnPlay` marking (free = `choice_free`, choice =
`has_choice`, order = `order_sensitive`).

| Cost | Card | Types | Text (summary) | Order |
|---|---|---|---|---|
| $2 | Candlestick Maker | Action | +1 Action, +1 Buy, +1 Coffers. | free |
| $2 | Hamlet | Action | +1 Card +1 Action. You may discard a card for +1 Action. You may discard a card for +1 Buy. | order |
| $2+ | Farrier | Action | +1 Card +1 Action +1 Buy. Overpay: +1 Card at end of this turn per $1 overpaid. | free |
| $2+ | Stonemason | Action | Trash a card from your hand. Gain 2 cards each costing less than it. Overpay: gain 2 Actions each costing exactly the amount overpaid. | order |
| $3 | Menagerie | Action | +1 Action. Reveal your hand: +3 Cards if all names differ, else +1 Card. | order |
| $3 | Shop | Action | +1 Card, +$1. You may play an Action from your hand that you have no copy of in play. | choice |
| $3+ | Infirmary | Action | +1 Card. You may trash a card from your hand. Overpay: play this once per $1 overpaid. | order |
| $4 | Advisor | Action | +1 Action. Reveal the top 3 cards: the player to your left picks one; discard it, the rest to your hand. | order |
| $4 | Farmhands | Action | +1 Card +2 Actions. When you gain this, you may set aside an Action or Treasure from your hand and play it at the start of your next turn. | free |
| $4 | Plaza | Action | +1 Card +2 Actions. You may discard a Treasure for +1 Coffers. | order |
| $4 | Remake | Action | Do this twice: trash a card from your hand, then gain a card costing exactly $1 more. | order |
| $4 | Young Witch | Action-Attack | +2 Cards. Discard 2 cards. Each other player gains a Curse unless they reveal a Bane from their hand. Setup: a Bane pile. | order |
| $4+ | Herald | Action | +1 Card +1 Action. Reveal the top card; if it is an Action, play it. Overpay: put a card from your discard pile onto your deck per $1 overpaid. | order |
| $5 | Baker | Action | +1 Card +1 Action +1 Coffers. Setup: each player gets +1 Coffers. | free |
| $5 | Butcher | Action | +2 Coffers. You may trash a card from your hand and spend Coffers: gain a card costing up to the trashed card's cost plus the Coffers spent. | order |
| $5 | Carnival | Action | Reveal the top 4 cards. Put one of each differently named card into your hand, discard the rest. | order |
| $5 | Ferryman | Action | +2 Cards +1 Action. Discard a card. Setup: a Ferryman pile (non-Supply). When you gain this, gain a copy of that pile's card. | order |
| $5 | Footpad | Action-Attack | +2 Coffers. Each other player discards down to 3 cards. In games using this, when you gain a card in an Action phase, +1 Card. | free |
| $5 | Horn of Plenty | Treasure | Gain a card costing up to $1 per differently named card you have in play (counting this). If it is a Victory card, trash this. Worth $0. | choice |
| $5 | Hunting Party | Action | +1 Card +1 Action. Reveal your hand; reveal cards from your deck until one that is not a copy of a card in your hand; it goes to your hand, discard the rest. | order |
| $5 | Jester | Action-Attack | +$2. Each other player discards their top card; a Victory card gives them a Curse, otherwise you choose: they gain a copy or you do. | choice |
| $5 | Journeyman | Action | Name a card. Reveal cards until 3 without that name; they go to your hand, discard the rest. | order |
| $5 | Joust | Action | +1 Card +1 Action +$1. You may set aside a Province from your hand to gain a Reward to your hand. Discard the Province in Clean-up. | choice |
| $5 | Merchant Guild | Action | +1 Buy +$1. At the end of your Buy phase, +1 Coffers per card you gained in it. | free |
| $5 | Soothsayer | Action-Attack | Gain a Gold. Each other player gains a Curse, and if they did, draws a card. | choice |
| $6 | Fairgrounds | Victory | 2 VP per 5 differently named cards you have (round down). | n/a |

Rewards ($0*, not in the Supply, gained only by Joust; 1 of each with 2 players, 2 each with 3-6):

| Reward | Types | Text | Order |
|---|---|---|---|
| Coronet | Action-Treasure-Reward | You may play a non-Reward Action from your hand twice. You may play a non-Reward Treasure from your hand twice. | choice |
| Courser | Action-Reward | Choose two different: +2 Cards; +2 Actions; +$2; gain 4 Silvers (in that order). | choice |
| Demesne | Action-Victory-Reward | +2 Actions +2 Buys. Gain a Gold. Worth 1 VP per Gold you have. | choice |
| Housecarl | Action-Reward | +1 Card per differently named Action you have in play (counting this). | free |
| Huge Turnip | Treasure-Reward | +2 Coffers. +$1 per Coffers you have (counting these). | free |
| Renown | Action-Reward | +1 Buy. This turn, cards cost $2 less. | free |

Notes on the markings. Choice-free needs "nothing revealed from, gained to, or put on its player's
deck" (see `cards.rs`): Candlestick Maker, Farrier, Farmhands (the set-aside is on gain, not on
play), Baker, Footpad, Merchant Guild, Housecarl, Huge Turnip, Renown only add +Cards/+Actions/
+Buys/+$/Coffers/cost reduction for their player or attack others. Huge Turnip is a Treasure whose
value reads Coffers: the only things that change Coffers while Treasures are played are Turnips
themselves (order of identical cards), and Coffers can be spent only at the Buy decision, after
every plain Treasure is played, so playing the plain Treasures in any order is equivalent.

Everything that reveals from the deck (Advisor, Carnival, Herald, Hunting Party, Journeyman,
Menagerie reads the hand) or takes any card from the hand (Hamlet, Stonemason, Infirmary, Remake,
Young Witch, Ferryman, Butcher, Plaza) is `order_sensitive`, per the rule of thumb in `cards.rs`.
`obvious_play.rs` checks the choice-free markings by exact search.

## 2. Mechanics and how each fits the engine

### 2.1 More than 128 cards

126 + 26 + 6 = 158 card ids. `counts::LANES` goes from 128 to 160 (`Counts` is 160 bytes, align
32 instead of 64 so it does not round up to 192) and `GameState::in_supply` from a `u128` to a
`SupplyMask` (a `u128` for ids below 128 and a `u64` above). The new masks for gain-trigger gating
live in the second word. The card type byte becomes a `u16` (Reward and Command types).
`CHOICE_CAP` stays 128 (an overpay offers at most 60 amounts, with and without a Potion).

### 2.2 Coffers

`PlayerState::coffers: u16`. Gained by Candlestick Maker, Baker (and its setup), Plaza, Butcher,
Footpad, Huge Turnip, Merchant Guild. Ruling (Coffers, "Official rules"): tokens can be removed at
any time during your turn for +$1 each, any number at once; only tokens on the Coffers mat count.
Engine: the Buy decision offers `Choice::Coffers` ("remove one token, +$1") while the player has
tokens and a buy; the bot or human repeats it as needed. Because the Buy decision comes before every
buy (and before the Overpay decision of the card bought) this is the "any time" rule in every spot
where coins matter: before a buy, between buys, and to raise an overpay. Butcher is the one card
that uses tokens inside its own effect: its gain frame carries a base cost and the number of
Coffers the player has (`Frame::pay_coffers`); picking a card costing more than the base spends
exactly the difference (spending more would gain nothing, so the player is never asked "how many").
`Event::Coffers { player, source, n }` and `Event::SpendCoffers { player, n }`. Text format:
`coffers: N` per player; `PlayerView::coffers_of`; page: a chip with the count and a "Spend $1"
button; `coffers` expression variable for strategies.

Unspent tokens are never lost. `NextHandEvaluator` counts them as money for the search's tie-break.

### 2.3 Overpay

Cards: Farrier, Stonemason, Infirmary, Herald (`cards::has_overpay`). Rulings (Overpay, "Official
rules"; each card's FAQ): only when buying; the amount is what you pay beyond the card's current
cost (a Renown does not make overpaying cheaper); a card with "+" still has its printed cost; you
cannot overpay $0 (to overpay you must pay more than the cost) but a Potion alone is an overpay for
Stonemason ("$0P": Transmute); Potions can be overpaid with, "only meaningful with Stonemason" (the
engine offers the Potion option only for Stonemason). The ability happens when the card is gained,
with the other when-gain abilities.

Engine: buying an overpay card with something to spare pushes a `FrameKind::Overpay` frame; its
`DecisionKind::Overpay { card, max_coins, potion }` offers `Choice::Overpay(coins)` for every amount
0..=max_coins (at most 60), and `Choice::OverpayPotion(coins)` likewise when the Potion is allowed. On the answer the coins (and
Potion) are paid, the ability's frames are pushed, and then the card is gained, so its reactions
(Watchtower, Footpad...) are asked before the ability resolves. The abilities are generic frames:
Farrier adds to `TurnState::extra_draw` (cleanup draws 5 + n; an Outpost turn 3 + n), Stonemason is
two exact-cost `Gain` frames (Action filter, the Potion matching), Herald is an `ordered` `Select`
from the discard to the deck (min = max = n, clamped), Infirmary is n `PlayEffects` frames on the
card itself (it stays in the discard pile: it is "played" without moving; the first play logs as a
play, the rest as repeats).

### 2.4 Non-Supply and set-up piles

`GameConfig` gets `bane: Option<CardId>` and `ferryman: Option<CardId>` (None = chosen
deterministically from the kingdom when needed, so a fixed kingdom has the same piles in every game
of a batch, see `cards::default_bane` / `default_ferryman`). `GameState` stores `bane`, `ferryman`,
`ferryman_left` and `rewards: [u8; 6]`.

- **Bane** (Young Witch; Young Witch, Setup): a Kingdom card costing $2 or $3 (a Potion cost
  never qualifies: "$2P" is not "$2"), not already in the kingdom. Its pile is an ordinary Supply
  pile (in `in_supply`, counts toward the three-pile ending, can be bought and gained) and is
  named in `state.bane`. Young Witch's victims auto-reveal a Bane from their hand, like Moat
  (logged as a Reaction); Moat and other reactions come first (Young Witch, FAQ). The check is made
  when each victim's Curse is about to be gained (`Frame::bane_blocks`), after Young Witch's own
  draw and discard.
- **Ferryman's pile** (Ferryman, Preparation): an unused Kingdom card costing $3 or $4 ("+" is
  only a reminder: Infirmary, $3+, qualifies; "cost" is the card's own cost, Potion cards do not).
  Not in the Supply: not in `in_supply`, not on the end condition, cannot be bought or gained by
  Workshop-like gains. It is gained only by gaining a Ferryman (any way, including a Jester copy or
  Joust-free gains; also from the trash, Ferryman FAQ), through `gain_side`. Do any setup the pile
  needs: Young Witch as the Ferryman pile needs a Bane, Baker gives starting Coffers
  (`needs setup` is computed over the kingdom plus both side piles).
- **Rewards** (Reward, Joust): six single-card piles outside the Supply, 1 each (2 players) or 2
  each (3-6). Only Joust gains them ("any Reward"), to the hand, through `gain_side`; an empty
  Rewards pile does not count toward the end; trashed Rewards stay in the trash (Lurker can take
  them back, Lurker is not changed); a Reward cannot be bought, gained by Horn of Plenty, or taken by
  Workshop-like gains; Jester's/Mint's "copy" gains come from the Supply only (no copy of a Reward).
  Rewards have a Reward type (so Coronet can exclude them) and count for Fairgrounds, scoring and
  Courtier's type count.

`Zone::Supply` selections (Lurker's trash from the Supply) look only at `in_supply` piles because
the side piles are kept outside `GameState::supply`.

### 2.5 Other new generic pieces

- `Frame` gets `less` (Gain: strictly cheaper, Stonemason), `pay_coffers` (Gain: Butcher), `bane_blocks`
  (Gain: Young Witch's Curse), `from_side` (Gain: choose among the Rewards, Joust). `FrameKind::
  Overpay`, `FrameKind::TopCard` (Jester: take the deck top, apply an act, then a continuation).
- `Then` continuations: `BonusIfPicked` (Hamlet), `CoffersPerPick` (Plaza), `GainLess` (Stonemason),
  `GainWithCoffers` (Butcher), `RevealedToHand` (Advisor), `OneOfEachToHand` (Carnival),
  `KeepMatchingDiscardRest` (Journeyman, Hunting Party), `HoldUntilNextTurn` (Farmhands),
  `JoustReward`, `DrawIfGained` (Soothsayer), `HornTrash`, `JesterGain`.
- Filters: `NotCard(c)`, `NotInHand` (dynamic), `NotInPlayAction` (dynamic: Shop),
  `ActionNotReward`, `TreasureNotReward` (Coronet).
- `ModeOpt`: `GainMany` (Courser's 4 Silvers, done as gain frames so the order "do the options in
  the order listed" holds even across a reshuffle), `GainSubject` (Jester's choice). Courser, Jester
  are `Mode` frames; Jester's chooser is the attacker.
- Per-player zones: `farm_cards` (Farmhands' set-aside cards, public) and `cleanup_aside`
  (Joust's Province until Clean-up; the card is not in play and not in `set_aside`, which other
  effects empty).
- Per-turn: `extra_draw`, `guilds` (Merchant Guild plays) and `gained_in_buy`.

### 2.6 Card by card

- **Candlestick Maker / Baker / Footpad / Plaza / Huge Turnip / Merchant Guild**: coffers as in 2.2.
  Merchant Guild (FAQ): +1 Coffers per card *you* gained in your Buy phase, whether bought or
  gained other ways (Ferryman's copy counts; Butcher's gain in the Action phase does not),
  cumulative with several plays (`guilds` counts plays, `gained_in_buy` the gains; paid when the
  Buy phase ends, before the end-of-turn offers). Plaza (FAQ): draw first; the Treasure may be the
  one just drawn; +1 Coffers only if it was discarded.
- **Hamlet** (FAQ): draw, +1 Action, then two independent optional discards, the +Action one first;
  at most one card per bonus.
- **Menagerie** (FAQ): +1 Action, reveal the hand; names decide, not types; an empty hand is "all
  different": +3 Cards.
- **Shop** (FAQ): +1 Card +$1; play an Action from the hand that has no copy in `in_play`
  (whether or not a copy was played earlier this turn and left play, only "in play now" counts);
  the play costs no Action.
- **Infirmary**: +1 Card, then an optional trash. Overpay plays it n times (the whole card each
  time, draw then trash).
- **Advisor** (FAQ): reveal up to 3 (fewer after a reshuffle); the player to the left chooses one
  to discard even if only one card was revealed; the rest go to the hand.
- **Farmhands** (FAQ): the gain trigger is optional and offered whenever the player gains it, on
  any turn; the set-aside card is face up and public; it must be played at the start of the next
  turn (not an Action play, mandatory); it enters play normally (a Treasure gives its $, a Duration
  stays), and is then discarded as usual. It is one of the start-of-turn effects (`StartOrder`,
  order-sensitive); the entry is named after the card it plays.
- **Remake** (FAQ): trash, gain, then trash, gain: one at a time, so the on-trash/on-gain effects
  interleave; with no card to trash nothing happens; with no exact-cost card nothing is gained.
  Potion rule: exactly +$1 with the trashed card's Potion matching (the Upgrade row of the table in
  `alchemy-plan.md` 5b).
- **Stonemason** (FAQ): the trash is mandatory if there is a card; the two gains are separate
  choices of cards costing less than it (the same or different); a card is "less" when no part of
  its cost is larger and it is not equal (so a Golem $4P can be turned into a $4 card or a $3P card,
  Potion-free cards always fit under a Potion card); no cheaper card = no gain. Overpay: two Actions
  each costing exactly the overpaid coins (and Potion); none at that cost = none.
- **Young Witch** (FAQ): +2 Cards, then discard 2 (mandatory), then each other player in turn
  order gains a Curse unless they reveal a Bane; the attack hits in turn order, matters when the
  Curses run low.
- **Herald** (FAQ): draw, +1 Action, reveal the top card; a revealed Action is played (mandatory,
  no Action used) and a Duration it plays stays (Herald is discarded normally); a non-Action goes
  back on top. Overpay: for each $1 put a card from the discard pile onto the deck (the Herald
  itself may be one), as many as there are; the order of the topdecks is chosen.
- **Butcher** (FAQ): +2 Coffers first; the trash is optional; the tokens removed (zero is fine,
  the ones just added count) only raise the cost limit and give no $.
- **Carnival** (FAQ): reveal 4 (reshuffling if needed); one of each name to the hand, the rest
  discarded.
- **Ferryman** (FAQ): when gained (any way) also gain the pile's card to the discard pile.
- **Footpad** (FAQ): the passive draw applies to every player in games using it, whenever they
  gain a card during an Action phase (including a Curse an attacker gives them, a start-of-turn
  gain) and is not optional; not in the Buy phase or Clean-up. The attack makes others discard down
  to 3.
- **Horn of Plenty** (FAQ): worth $0; the limit is the number of differently named cards in play
  right now (counting itself, Treasures played, Actions, and Durations from last turn; not Farmhands
  or Prince cards that are set aside); a Victory gain trashes this Horn (not for other Victory
  gains); the gain is from the Supply and goes to the discard pile. Played last of the Treasures by
  the bots (like Bank).
- **Hunting Party** (FAQ): draw, +1 Action, reveal the hand, then reveal until a card that is not a
  copy (by name) of a card in the hand; it goes to the hand, the rest are discarded; none found
  after a reshuffle = discard everything revealed.
- **Jester** (FAQ and clarifications): +$2; each other player (Moat/Lighthouse protect) discards
  the top card of the deck (an empty deck and discard = unaffected); a Victory card gives them a
  Curse; otherwise the attacker chooses who gains a copy (none if the pile is empty or the card is
  not in the Supply).
- **Journeyman** (FAQ): name any card (the engine offers the cards in the deck and discard pile, and
  the Supply piles: naming anything else is the same as naming nothing); reveal until 3 without that
  name or everything is revealed; those go to the hand and the rest are discarded.
- **Joust** (FAQ): the Province is set aside (not in the hand, not in play) and discarded in
  Clean-up; with the Rewards gone the Province may still be set aside for nothing; with 3+ players
  a player can have two of one Reward.
- **Soothsayer** (FAQ and clarifications): Gold, then each other player in turn order gains a Curse
  and, if they did (a Watchtower trash still counts), draws a card before the next player does;
  none if no Curse was gained.
- **Fairgrounds** (FAQ): 2 VP per 5 differently named cards among all the player's cards (deck,
  hand, discard, play, mats, set aside). Piles of 8/12.
- **Coronet** (FAQ): the Action is played first, both parts optional; cannot play Rewards; no Action
  or buy is used; in the Buy phase it is played as a Treasure and the Action it plays does not
  let other Actions be played. Gap: with Charlatan, a Curse is not offered as a Treasure for it.
- **Courser** (FAQ): choose any 2 different options first, then do them in order (draw before
  gaining the Silvers).
- **Demesne**: +2 Actions +2 Buys, gain a Gold; VP = Golds the player owns at the end.
- **Housecarl**: draws one card per differently named Action in play, itself included.
- **Renown** (FAQ): +1 Buy, costs $2 less; Throne Room doubles it.
- Rewards and Prince: a Prince can hold a Courser (Action costing $0*), a Housecarl or a Renown.

### 2.7 Prince is a Duration

Prince's current types are Action-Duration-Command (Prince, 2022 and 2024 versions). `cards.rs`
gives it `DURATION | COMMAND`; the generic Duration scheduling at the tail of
`resolve_effects_inner` skips it (its next-turn behaviour is the `prince_cards` start-of-turn
effect, already implemented); `Command` has no card to interact with in this simulator, so it is a
type flag and a test only ("it only stops Command cards playing each other"). The page colours
Durations orange, so Prince changes colour with the type.

## 3. Cost-comparison rules (extends `alchemy-plan.md` 5b)

| Kind | How | Cards here |
|---|---|---|
| "Up to $X more than the trashed card, X = Coffers spent": the limit keeps the trashed card's Potion | `GainWithCoffers`: a Gain frame with `max` = cost + Coffers, `potion` = the trashed card's | Butcher |
| "Exactly $1 more": exact, the Potion copied | `GainUpTo { plus: 1, exact }` | Remake |
| "Costing less than" (strictly): every part of the cost no larger, and not equal | Gain frame with `less`, `max` = coins, `potion` = trashed card's Potion | Stonemason's trash gains |
| "Exactly the overpaid amount", Potion included | Gain frame, `exact`, `potion` from the Overpay choice | Stonemason's overpay |
| "Up to $N", fixed: never a Potion card | `gain_frame(.., N, ..)` | Horn of Plenty (N = names in play) |
| A cost range for a set-up pile ($2-$3, $3-$4): coins only, Potion cards excluded | `cards::pile_eligible` | Bane, Ferryman |
| Reductions change coins only | `GameState::cost` (`cost_reduction`) | Renown (-$2, stacks) |

## 4. Bots

Defaults live in `crates/sim/src/strategy.rs` (stated rules first), each with a test in
`crates/sim/tests/cornucopia_defaults.rs`, including: +$ now versus +Coffers, spending Coffers to
reach a Province, saving when spending would not change the buy, overpay right and wrong, a
strategy preferring Courser against one preferring Renown taking different Rewards from Joust, and
a Lab genome with a Reward rule round-tripping.

- **Coffers**: the Buy decision: `match_gain_list` over the cards affordable with coins plus
  Coffers; if the first listed card is not affordable with coins alone, spend (`Choice::Coffers`)
  until it is. Unspent tokens stay; in the endgame (the same check as `win_this_turn`'s gate)
  everything is spendable anyway since the list is walked in order. An evaluator term counts tokens
  as money.
- **Overpay**: Stonemason: the amount that gains two cards the gain list wants (the highest-ranked
  Action cost affordable), else none; Potion included when a listed Potion-cost Action exists;
  Herald and Infirmary: each spare $1 if it is wanted (Herald: a card worth topdecking in the
  discard pile; Infirmary: a Curse, Estate or Copper to trash in hand... simplified to "overpay with
  all spare coins that cannot buy a listed card"); Farrier: spare coins that cannot buy anything
  listed. Spare = coins left after the card the bot would buy next.
- **Hamlet**: discard dead cards (Curse, Victory) for the bonuses; **Plaza**: discard a Copper only
  when the Coffers are wanted... a Treasure worth $1 or less (Copper) unless the hand needs it.
- **Stonemason / Remake / Butcher / Infirmary trashes**: the trash rules; upgrades use the gain list.
  Butcher gains the best listed card reachable; **Young Witch/Ferryman discards**: the discard rules.
- **Advisor** (the chooser, an opponent): discard the revealed card the revealer would most like to
  keep (highest value: Gold-like Treasures, Actions, over Victory and Curses).
- **Jester choice**: the copy goes to the bot if its gain list names the card, else to the victim.
- **Journeyman**: name the card that costs the most when drawn: Curse, then Victory cards, then
  Copper, weighted by the copies in deck and discard.
- **Farmhands**: set aside the best Action or Treasure in hand that the bot cannot play anyway, else
  a Treasure; **Joust**: set aside a Province when a Reward is left, then the Reward the bot's
  strategy rates highest (Renown > Courser > Demesne > Coronet > Housecarl > Huge Turnip by default).
- **Courser**: +$2 and +2 Actions... by the default Mode picks: "+$2" and "gain 4 Silvers"
  unless a stated `[[mode]]` rule says otherwise.
- **Coronet**: the best Action and the best Treasure to double (the Throne Room default).
- **Horn of Plenty**: last of the Treasures, the gain list decides the gain.
- Stated `[[mode]]` rules work for Courser and Jester through the usual labels.

### 4.1 Valuing Coffers, evaluating overpay, naming non-Supply cards

- **Coffers in the search's scoring.** `GainListEvaluator::preferences` values the Coffers gained
  this turn like the VP tokens (`vp_token_value`): a token is +$1 in a future hand of the player's
  choosing, priced with the evaluator's own `vp_per_coin` over the turns the game has left, at the
  preference scale (more than any play preference, never more than a gain-list difference). A token
  spent this turn is not a loss: it bought what the buy decision chose, which the gain list already
  scores. `NextHandEvaluator` (the tie-break) counts the tokens on the mat as money in the next hand.
  Spending is decided by the Buy rule above (spend only to reach a card listed above the best one
  affordable without the tokens), and the search sees both lines: +$ now versus +Coffers (Plaza's
  discard, Huge Turnip against another Treasure) and "spend now" versus "save".
- **Overpay is a searched decision.** `DecisionKind::Overpay` is one of the decisions that shape the
  turn, so with `search_play` the bot searches the amounts with its own scoring: Stonemason's two
  gains count at their gain-list weights, Herald's topdecked cards through the known deck top,
  Infirmary's draws and trashes, Farrier's cards through `NextHandEvaluator` (the evaluator reads
  `TurnState::extra_draw`). Without `search_play` the default rule above applies.
- **Non-Supply cards in strategies.** A strategy may name a Reward in a `[[gain]]` rule (and in
  conditions such as `count(Coronet)`); Joust's gain frame then follows the list, so the order of
  the Rewards in the list is the strategy's preference. A Reward is not a kingdom card: it never
  takes a kingdom slot (`kingdom_refs` skips it and asks for Joust instead), cannot be bought and is
  skipped by the buy rules. The Lab/evolve search space includes the Rewards as gainable cards when
  Joust is in the kingdom, so a genome can carry a Reward rule. Ferryman's pile card is gained
  automatically (no decision).

## 5. Tests

`crates/engine/tests/cornucopia_guilds_cards.rs` covers every card: the printed effect, edge cases
(empty deck and discard, empty piles, a reshuffle in the middle of an effect), the FAQ rulings
listed above, Throne Room and Coronet/Tiara interactions, Potion and cost-reduction interactions
(Stonemason with Golem/Transmute, Butcher, Remake, Renown, Bridge, Quarry), setup (Bane, Ferryman,
Rewards counts, Baker's Coffers), the Prince type change, text round trips, `cards::tests` lists
and `obvious_play.rs`. Bots: `sim/tests/cornucopia_defaults.rs`. Page: `web/test*.mjs` and the
wasm `log_tests`.

## 6. Steps

1. Infrastructure: 160 lanes, `SupplyMask`, 16-bit types, set plumbing, Coffers, Overpay, side piles,
   setup in kingdoms, text format, Prince type. (No cards yet except what tests need.)
2. Simple cards (Candlestick Maker, Hamlet, Menagerie, Plaza, Remake, Baker, Merchant Guild, Hunting
   Party, Fairgrounds, Farmhands, Shop, Carnival, Footpad, Advisor, Soothsayer, Journeyman, Jester,
   Butcher, Horn of Plenty) with tests.
3. Overpay, Bane and side-pile cards (Stonemason, Herald, Infirmary, Farrier, Young Witch,
   Ferryman, Joust and the six Rewards) with tests.
4. Bots (Coffers valuation, overpay search, Reward rules), the page, the Lab and sample strategies.

## 7. Performance log

`dominion-sim match big_money_ultimate double_witch --games 200000` (full bots, 32 threads).
Before starting: 1.714M, 1.738M, 1.719M games/s (best 1.738M). Per-step numbers below.

| Step | Games/s (best of 5, A/B against a build of the starting commit on the same machine) |
|---|---|
| Start | 1.725M (the build of the starting commit) |
| 1 Infrastructure | 1.79M (1.73-1.79M across runs) |
| 2 Simple cards | 1.75M (base 1.73M) |
| 3 Overpay, Bane, side piles | 1.82M (base 1.72M; 1.77-1.82M across runs) |
| 4 Bots, page, Lab | 1.82M (base 1.75M; 1.70-1.82M across runs) |

Step 1 first measured 12% slower (1.52M). Causes and fixes: a `Choice` of three bytes made the
`ChoiceBuf` every decision creates three times larger (`Choice::Overpay`/`OverpayPotion` are two
variants so a `Choice` stays two bytes, and `CHOICE_CAP` stays 128; overpay offers at most 60 amounts);
`Counts` at 160 lanes made `total`, `is_empty` and the Clean-up copies 25% longer, which a SSE2
byte-sum (`_mm_sad_epu8`) for `total`, an OR-fold `is_empty`, one fewer pass in `take_top`, and a
Clean-up fast path (nothing stays in play: add the hand and the play area, clear them) more than
paid for. `GameState` went from 9728 to 11424 bytes, `Frame` from 34 to 38.

## 8. Deviations from the first plan, and what is not covered

- `Monkey` is now `has_choice` (it draws when the player to its right gains a card, so playing it
  before a Curse-giving Attack matters): the exact-search check in `obvious_play.rs` found it once
  Cornucopia's cards changed the sampled hands. The same test now leaves out Philosopher's Stone (its
  value falls when an Action draws first, so "play everything" can lose to playing less; the order
  of the Actions is still never worse).
- The search's leaf test (`search::is_leaf`) no longer ends a turn that has a pending effect: a
  last Buy that leaves frames to resolve (an overpay's gains, a reaction) is searched through.
- A card that plays one other card once (Crystal Ball, Shop, Herald, Joust-style plays) no longer
  stays in play with a Duration it plays: the "multiplier" rule (Throne Room, King's Court, Coronet)
  needs two or more plays.
- A Silver played by an effect (Farmhands, Tiara, Crystal Ball, Coronet) now gets Merchant's bonus.
- Coronet does not offer a Curse as a Treasure under Charlatan. The Reward gain from Joust and the
  Ferryman copy are never offered a Potion rule (none of them has one).
- Hunting Party reveals its hand as an event only conceptually (no log line for the hand: the
  hand is revealed after the draw and nothing depends on it).
- `Command` is a type flag only (no Command card but Prince exists here).
- Journeyman offers the cards in the deck and discard pile plus the Supply piles; naming any other
  card is the same as naming nothing.
- Not done: Reward gains from the Black Market (no Black Market here); a Ferryman pile that is a
  split pile (none exists in this edition); Young Witch's Bane being a Reaction that leaves the hand.
