# Open items

Questions and known gaps to look at later. Not being worked on.

## Rules

- **Farmhands and the start-of-turn order.** Farmhands plays its set-aside cards (`farm_cards`) at the
  start of the owner's next turn. Check whether those plays join the start-of-turn order decision
  (`DecisionKind::StartOrder`, `cards::start_order_sensitive`, see `docs/promo-plan.md`) like Durations,
  Prince's card and Captain, so the player can interleave them in any order, or whether they always
  resolve at a fixed point. Same question for any other "at the start of your turn" effect added since
  (Nocturne, Hinterlands). Wiki: "Start of turn" and the Farmhands FAQ.

- **Church: which batch first.** Several Church plays make several batches; the wiki lets the player choose which batch
  is put into the hand first. The batches come in the order they were set aside (the start-of-turn question names an
  effect by its card, and all Churches are "Church"). See `docs/promo-plan.md` section 5.

## Bots

- **The last Province with several buys.** The don't-lose guard checks each buy alone: "Duchy, then the
  last Province" could win where "Province first" loses.
- **The second-to-last Province.** Buying it while behind lets the opponent take the last one and win;
  nothing guards against that.
- **Ending the game on piles.** The don't-lose guard only checks the last Province or Colony, not the card
  that empties the game-ending pile.
