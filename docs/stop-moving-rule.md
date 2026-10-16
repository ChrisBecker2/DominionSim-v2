# The stop-moving rule

Dominion's rule for when an effect may still move a card (the 2019 Errata's *stop-moving rule*, which replaced the *lose track
rule* of Dark Ages). This document is the design of how the engine applies it, and the list of every implemented card that
depends on it. Code: `crates/engine/src/track.rs`; the mechanism's hooks are in `effects.rs`, `engine.rs` and the sets.

## The rule

From the wiki (*Stop moving rule*, with the *Lose track rule* page for the history):

> An effect can move a card if it specifies where the card is coming from, or if the effect put the card where it is now.
>
> If a card isn't where the effect would expect it to be, or has moved away from there and then back, it can't move the card.
> * Played cards expect to be in play; they can't move themselves if they aren't.
> * Gained cards are expected to be where they were gained to, even if this isn't the discard pile.
>
> Cards in discard piles can be moved even if covered up by other cards; cards on top of a deck can't be moved once covered up.

And the clarifications:

* Only *movement* is prevented. "Other effects related to a card that has been moved elsewhere first by another effect still
  happen, as long as they are not contingent on the movement that was prevented" (a Madman played by Procession still gives
  +2 Actions the second time, but not the cards: "if you do"; a Traveller that was topdecked by Scheme can't be exchanged,
  since the exchange is contingent on returning it to its pile).
* "An effect that won't happen is playing a card": when a card tells you to play a card it has lost track of (Vassal's
  discarded Action, Sailor's gained Duration, Trail, ...) it doesn't. The exception is the Throne-like cards' replay: Throne
  Room plays its card again even if the first play moved it.
* "Cards do not lose track of cards that they move": Procession putting a Madman into play does not lose track of it; it is
  Madman moving itself away that does.

## The problem with multisets

A zone is a multiset (`Counts`): a card has no identity, only a name. The rule was approximated per card by "is a copy of that
name still in that zone?" (the old `Frame::min` counts, `prince_play`, `in_play.has(..)` checks). That is wrong whenever
*another copy* with the same name sits where the moved card was expected: a Royal Carriage replaying a Reserve card that had
gone to the mat because a different one was in play, a Throne Room'd Mining Village trashing the Mining Village played last
turn, a Watchtower topdecking an older Copper after another Watchtower trashed the one gained, a Scepter replaying a Duration
from an earlier turn.

## The mechanism: tokens for the copies that effects track

Only the copies an effect is *tracking* need an identity, and only for as long as it tracks them. The turn has a small ledger
(`TurnState::led`, `track::Ledger`: 8 marks of 6 bytes, a few cursors, and a 4-entry multiset of cards held over from the previous
turn; about 90 bytes in all, no heap). A **mark** is `(token, owner, zone, card, holders, dead)`: "one copy of `card` that was
put into `zone` of `owner`". The token is a `u8` serial (never 0 or `NIP`); frames carry it in `Frame::tok`.

* `0` means *not tracked*: the effect is a plain single resolution and the old zone-count check is exact (nothing else can have
  moved the copy in between). This is every ordinary game; the ledger is empty (`n == 0`) and every hook starts with that one
  compare.
* `NIP` (255) means *not in play*: the card was played from where it stays (Prince's set-aside card, Captain's pick from the
  Supply, Necromancer's from the trash, Band of Misfits, Overlord). An effect that moves "this" out of play can't.
* Any other value names a mark.

**What a mark means.** The mark is *live* while the copy has not moved. It dies (kept as `dead`, so a stale frame can never
mistake a later mark for it, until its holders are done) in three ways:

1. *A move that targets the copy.* A card moving itself (`leave_play_self`), a gain trigger moving the card it reacts to
   (`tok_moved` when its frame's pick or yes is applied) tells the ledger first. Every other frame that holds the token then
   finds the copy gone, **even when another copy of the same name is in the zone**.
2. *A move that doesn't know about the mark* (an untracked effect picks a card of that name from the zone: `zone_take`,
   `remove_from_play`) is read as moving an untracked copy as long as the zone holds more copies than the live marks of that
   card there (the pigeonhole check, `left_zone`, and again in `tok_valid`). Only when every copy that remains is tracked does it
   cost the newest mark. So a move of an untracked copy never invalidates a tracked one while untracked copies remain.
3. *Covering up.* A mark on top of a deck dies when anything is put on that deck (`cover_deck`: topdecking, gaining to the
   deck) or the top is taken (`take_top`); a mark in a discard pile is **not** killed by covering, only by a move or by the pile
   being shuffled away (`take_top`'s reshuffle). "Moved away and then back" is covered by 1 and 2 together: a copy that was
   shuffled into the deck and revealed back onto the discard pile is a different, untracked copy.

**Who makes marks.** The effect that puts the card somewhere and has something left to do with it:

| Mark | Made by | Zone | Held by |
|---|---|---|---|
| a played copy | `push_plays` (Throne Room, King's Court, Crown, Disciple: a card played more than once), `PlayTrash` (Procession, Counterfeit), `resolve_effects_tok` when a Daimyo, Citadel or a Royal Carriage on the mat will act after the play, Scepter's pick | in play | a `Release` frame under the plays; the trash frame of Procession / Counterfeit |
| a gained copy | `finish_gain`, when a trigger is possible (`gain_triggers_possible`), a Prophecy hooks gains, or the gainer wants to move it afterwards (`led.want`: Replace, Summon, Continue) | where it was gained (discard, hand, deck top) | a `Release` frame under the gain's reactions; the guard frame of Replace / Summon / Continue / Rapid Expansion |
| a discarded copy | `run_vassal`, when a card reacts to discards (`G_DISCARD_HOOK`) | discard pile | a `Release` frame under Vassal's offer and the discard reactions |

Frames that move or play the tracked copy carry the token: the frames a play pushes (stamped in `resolve_effects_tok`, so
Mining Village's trash, Raze's, Death Cart's, Pillage's pick find the copy gone after a replay), the reactions a gain triggers
(stamped in `finish_gain`: Watchtower, Tiara, Travelling Fair, Tracker, Changeling, Trader, Trail, Weaver, Berserker, Sailor,
Innovation, Cargo Ship), the reactions to a Vassal's discard (Faithful Hound, Trail, Weaver, Tunnel). A `YesNo` or `Select`
whose token says its copy is gone is skipped (`frame_lost`); the effect's other parts still happen.

**Cost.** None while nothing tracks anything: `Ledger::n == 0` is one byte compare in `left_zone`, `cover_deck`, `take_top`
and `resolve_effects_tok`. Gains cost a mark and a `Release` frame only in games with a gain watcher; plays only in games with
a Royal Carriage, a Daimyo or a Citadel, and when a card is played more than once.

## Cards and rules that depend on the stop-moving rule

*Tracks* says what the mechanism holds for the card; "(counts)" marks cards whose per-copy offers are symmetric (every copy
is in the same place, so a count of copies is exact) and are only *tested*.

### Base
* **Throne Room**: replays a card; the card is tracked from its first play (a mark on the copy in play) so that the later plays
  see what the earlier ones did to it. Throne-like replays still happen when the card has moved.
* **Vassal**: the discarded card is tracked in the discard pile when a card in the game reacts to discards (Hound, Trail,
  Weaver, Tunnel); lost → not played.

### Intrigue
* **Mining Village**: trashes itself; tracked by whatever replays it (Throne Room, King's Court, Procession, Scepter, ...).
* **Replace**: the gained Action/Treasure goes onto the deck if it is still in the discard pile (guard frame, gain mark); covered
  in the discard pile (Skulk's Gold) is fine, moved by a Watchtower is not.

### Seaside
* **Island**, **Treasure Map**: move themselves; tracked play. **Treasure Map's four Golds** are four gains, each answered by
  its Watchtower before the next covers it on the deck.
* **Blockade**: the gained card is set aside at once and the gain's reactions (Watchtower, Trader, Trail, ...) find it gone
  (`tok_moved` on the gain mark).
* **Sailor**: plays the gained Duration only if it is still where it was gained (gain mark, also on the deck top).
* **Treasury**, **Corsair**: Treasury's put-onto-deck offers are per copy (counts); Corsair trashes the first Silver/Gold played
  through `remove_from_play`, which costs a tracked copy's mark.

### Prosperity
* **Watchtower**, **Tiara**: move the gained card; gain mark + frames. **Mint**: trashes Treasures in play (`remove_from_play`).
* **Investment**, **Horn of Plenty** (Cornucopia & Guilds): trash themselves; tracked play.
* **Royal Seal** is not implemented; **King's Court**: as Throne Room.

### Alchemy
* **Alchemist**, **Herbalist**: topdeck from play at cleanup: per copy offers (counts).

### Cornucopia & Guilds
* **Horn of Plenty** (above).

### Hinterlands
* **Scheme**: topdecks cards from play at cleanup, before the offers of the other discard-from-play cards (Treasury, Traveller
  exchanges): those are one offer per copy still in play (counts), so a Traveller Scheme put on the deck is not offered an
  exchange. (Hermit's exchange is at the end of the Buy phase, before Scheme: the current Hermit text, not the old trash.)
* **Trail**, **Weaver**: play themselves when gained or discarded (gain mark / Vassal's discard mark); **Faithful Hound**
  (Nocturne) sets itself aside when discarded (same mark); **Tunnel** reveals itself (same mark).
* **Trader**: the gained card returns to its pile if it is still where it was gained. **Berserker**: played when gained.
* **Inn**, **Border Village**: move other cards, no tracking needed.

### Dark Ages
* **Counterfeit**, **Procession**: play a card twice and then trash it: the card is tracked in play; the trash frame finds it
  gone after **Madman**, **Spoils**, **Pillage**, **Mining Village**, **Island** moved themselves. Procession still gains the Action
  costing $1 more than the card (the gain doesn't depend on the trash).
* **Madman**, **Spoils**, **Pillage**, **Death Cart**, **Urchin**, **Knights** (the attacker trashes the Knight that was played),
  **Band of Misfits** (the card played is not in play), **Hermit** (exchange at the end of the Buy phase, only for Hermits still in
  play: `remove_from_play`), **Fortress** (counts), **Cultist**.

### Adventures
* **Reserve cards** move themselves to the Tavern mat when played: tracked play; **Royal Carriage** replays an Action only if
  it is still in play (the played copy's mark, no longer a special case for Reserve cards); **Coin of the Realm**.
* **Raze**, **Soldier**, **Disciple** (Throne-like), the **Travellers** (exchange when discarded from play: counts), **Bonfire**
  (Coppers in play), **Travelling Fair** (gained card to the deck).

### Empires
* **Crown** (Throne-like), **Overlord** (plays a card from the Supply: not in play), **Engineer**, **Small Castle**,
  **Farmers' Market**, **Encampment**: move themselves; tracked play. **Villa** moves the gained card to the hand at once (gain
  mark), **Rocks**/**Crumbling Castle**/**Haunted Castle**: cover cards on the deck or discard pile.
* **Capital**, **Charm**, **Fortune**: gain more cards; nothing to track.

### Nocturne
* **Changeling**, **Magic Lamp**, **Wish**, **Bat**, **Vampire**, **Tragic Hero**, **Pixie** (trashes itself to receive the Boon
  twice; the play's token goes to the Boon frames): move themselves; tracked play. **Tracker**, **Changeling**'s exchange of a
  gained card: gain mark. **Necromancer**: plays a card from the trash (not in play). **Skulk**, **Cemetery**, **Cursed Village**:
  gain things that cover the card in the discard pile (fine).
* **Crypt**, **Monastery**: move Treasures/Coppers in play (counts).

### Rising Sun
* **Daimyo**: replays the next Action (tracked play, `Release` frame). **Snake Witch** returns itself. **Riverboat**, Biding
  Time: set aside cards played later. **Rapid Expansion**, **Continue**: the gained card is set aside / played if it is still where
  it was gained (gain mark). **Foresight**, **Change**.

### Renaissance
* **Scepter**: replays an Action *played this turn* that is still in play: copies in play beyond the ones held over from an
  earlier turn (`Ledger::carried`); the replay is tracked (`track_replay`, the outer play's mark when there is one).
* **Citadel** (Project): replays the turn's first Action (tracked play). **Experiment** returns itself. **Improve**, **Horn**
  (Artifact), **Cargo Ship** (the gained card set aside, gain mark), **Innovation** (gained Action played, gain mark),
  **Piazza**.

### Promos
* **Prince**: the set-aside card is played at the start of each turn, never in play (`NIP`); **Captain**: plays a card from the
  Supply (`NIP`); **Summon** (Event): the gained Action is set aside if it is still in the discard pile (gain mark);
  **Walled Village**, **Church**, **Sauna/Avanto**: per copy (counts).

## What stays approximate

* Two marks for the *same* copy (an outer play and a Scepter replay of it) are reconciled by sharing the outer token
  (`track_replay`); two different copies of a card tracked at once by unrelated effects, with one of them gone by an untracked
  move, may lose the wrong one (the newest). The ledger holds 8 marks; with more, the oldest dead (else oldest) is dropped and
  its holders find it lost.
* The multiplier bookkeeping of Throne-like cards playing a Duration (`TurnState::multiplier_*`, `duration_held`) is about
  *which cards stay in play*, not about identity; it is unchanged.
* The text format does not save the ledger or the effect stack: a state loaded mid-resolution has no tokens (the old
  count-based checks apply to it).

## Rulings this changed (found by diffing tens of thousands of random games of the old and new engine)

* A second reaction to a gain can't move a card the first one moved, even if an older card of that name lies in the discard
  pile: Watchtower trash then Watchtower topdeck, Watchtower topdeck then trash, Trader's exchange then Watchtower/Trail/Weaver,
  Trail played then Watchtower, Blockade's set-aside then Weaver/Trader/Cargo Ship.
* Vassal can't play a discarded card that a reaction (Faithful Hound) set aside or that Weaver/Trail already played, whatever else of
  that name is in the discard pile.
* Throne Room, King's Court, Crown, Daimyo, Citadel, Procession, Counterfeit, Scepter, Royal Carriage: a card that moved itself
  (Mining Village, Island, Treasure Map, Pillage, Raze, Death Cart, Pixie, Madman, Spoils, any Reserve card going to the mat)
  does not move another copy of itself on its later plays, and Royal Carriage is not offered for it.
* A card played from where it stays (Prince, Captain, Band of Misfits, Overlord, Necromancer) cannot move itself even if the
  same card is in play (Pixie's Boon trash was offered).
* "Moved away and then back": a gained card that a Hex shuffled into the deck and revealed back onto the discard pile is not the
  card the gain's reactions (Changeling) were about.
* Scepter does not replay a Duration held over from an earlier turn when this turn's copy has left play.
* Treasure Map's four Golds are four gains, each with its own reactions before the next covers it on the deck (it was a loop that
  gained all four first and answered the reactions afterwards).

## Tests

One per dependent card in its set's test file (`throne_roomed_*`, `a_second_watchtower_*`, `procession_on_a_madman_*`, ...),
the cross-set cases in `cross_set.rs`, `track.rs`'s unit tests (deck cover, discard cover, pigeonhole, moved and back, capacity),
and two example programs for differential checks: `rand_diff` (a hash of every event and choice of random games, to diff two
builds; `TRACE=<game>` prints one) and `rand_leak` (no token may stay held when the effect stack is empty).
