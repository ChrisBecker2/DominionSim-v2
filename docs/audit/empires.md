# Audit: Empires (cards, Events, Landmarks, split piles, Castles, Debt)

Branch `worktree-agent-a3d99b067aed71ece`. Sources: the wiki pages of every card, Event and Landmark (current text, FAQ, errata/versions), `Debt`, `Split pile`, `Castles`.
Current (2022 / 2025) texts are the reference: Charm, Forum, Basilica, Colonnade, Defiled Shrine, Tax say "gain"; Mountain Pass bids at once; Groundskeeper is
"this turn"; Chariot Race draws, then reveals; Donate triggers at the start of the next turn; Ritual checks the cost after trashing.

Counts: 36 cards checked (4 Debt kingdom cards, the 10 halves of the 5 split piles, 8 Castles, 14 others), 13 Events, 21 Landmarks = 70.
Bugs found 9, fixed 5 (Charm, Temple, Catapult order, Treasure pile tokens, Fortune token order), left open 4 (Encampment timing, Arena without tokens, separate Archive sets, hidden do-nothing Events: fixed 2026-10-28), plus 1 policy note (Mountain Pass bid).

## Cards checked (one line each)

Engineer OK. City Quarter OK. Overlord OK (non-Command non-Duration up to $5, top of pile only, `NIP` mark so stop-moving works). Royal Blacksmith OK.
Encampment: OK except the return timing (open, below). Plunder OK (VP token per play, also with Crown). Patrician OK. Emporium OK (counts Durations in play).
Settlers OK. Bustling Village OK. Catapult: BUG fixed (order of the Curses and the discards). Rocks OK. Gladiator OK (the top-of-pile rule under Allies rotation is not testable: Empires piles never rotate).
Fortune: BUG fixed (a +$ token on the pile now counts before the doubling). Castles pile: OK (8 / 12 cards, top only, one pile for the ending). Humble OK. Crumbling OK. Small OK.
Haunted OK (not an Attack, own turn only). Opulent OK. Sprawling OK (either option offered with the Duchies gone). Grand OK. King's OK.
Chariot Race OK (2025 text; ties, empty decks, -1 Card token, Debt and Potion comparisons). Enchantress OK. Farmers' Market OK. Sacrifice OK.
Temple: BUG fixed (the VP token is added to the pile with an empty hand). Villa OK. Archive: deviation (one pool for several Archives, open). Capital OK. Charm: BUG fixed (spent by Rocks' Silver).
Crown OK. Forum OK (gain, not buy). Groundskeeper OK. Legionary OK. Wild Hunt OK.
Events: Advance, Annex, Banquet, Conquest, Delve, Dominate, Donate, Ritual, Salt the Earth, Tax, Triumph, Wedding, Windfall all OK. Deviation: Events that would do nothing are not offered.
Landmarks: Aqueduct, Bandit Fort, Basilica, Baths, Battlefield, Colonnade, Defiled Shrine, Fountain, Keep, Labyrinth, Mountain Pass, Museum, Obelisk, Orchard,
Palace, Tomb, Tower, Triumphal Arch, Wall, Wolf Den OK. Arena: open (not offered with no tokens left).
`choice_free` / `has_choice` / `order_sensitive` marks checked against the CLAUDE.md rule: no change needed (each set's `choice_free_and_order_sensitive_cards` test passes).
Bot defaults checked: nothing illegal or crashing; Crown in the Buy phase is played with no Action in hand (test added), Sprawling Castle takes Estates with no Duchies (test added).

## Bugs fixed (each with a test that failed before the fix)

1. Charm: "the next time you gain a card this turn" was spent by the Silver that Rocks gains from its own gain trigger, instead of by the Rocks.
   Fix: `gain_triggers` takes the Charms before the card's own trigger runs. Test `charm_applies_to_the_card_bought_not_to_the_silver_its_gain_trigger_gains`.
2. Temple: "Add 1 VP to the Temple pile" is not conditional on trashing; with an empty hand nothing was added.
   Test `temple_adds_its_vp_token_to_the_pile_even_with_an_empty_hand`.
3. Catapult: "each other player gains a Curse ... (then) discards down to 3": the discards were pushed on top of the Curse gains, so they ran first (rulings page: Curses in turn order, then discards).
   Test `catapult_gives_the_curses_to_everyone_in_turn_order_before_anyone_discards`.
4. Split pile with an Adventures token (Training's +$1 on Catapult / Rocks): a Treasure of the pile played in the Buy phase got no bonus; and Fortune's token +$1 came after the doubling
   ("you'll get +$1 from playing Fortune before you double"). Tests `a_training_token_on_a_split_pile_works_for_both_of_its_halves`, `a_coin_token_on_the_gladiator_fortune_pile_counts_before_fortune_doubles`.

## Open issues (ignored tests)

* Encampment is returned to its pile at once instead of "at the start of Clean-up" (the pile is one card fuller during the Buy phase; only matters with 0-1 cards left in a pile).
  `encampment_stays_out_of_its_pile_until_cleanup` (ignored).
* Arena is not offered when its tokens are gone; the rules still let the player discard an Action (Village Green, Tunnel). `arena_still_offers_the_discard_when_it_has_no_tokens_left` (ignored).
  Not fixed: it would add a pointless decision for every bot in the rest of the game.
* Archive: several Archives share one pool of set-aside cards (`ARCHIVE_CAP`); the rules keep separate sets. `two_archives_keep_separate_sets_of_cards` (ignored).
* Events that would do nothing (Dominate with no Provinces, Ritual with no Curses, Triumph with no Estates, Windfall with no Golds) are hidden by `event_available`; the rules allow buying them.
  `an_event_that_does_nothing_can_still_be_bought` (ignored).

## Policy notes

* Mountain Pass: the bots bid at most 5 Debt, opening with 2. Wiki (Mountain Pass, Strategy): "you'll almost never want to bid as little as the price of a Province, because winning the
  bid is almost strictly better than buying one." Ignored test `the_first_bid_for_mountain_pass_is_not_less_than_the_price_of_a_province`.
* Donate (wiki: "Turn 2 Donate: one of the more common options") and Temple ("buy it just before the greening for its when-gain effect") are only used through stated gain lists; the defaults know nothing about them.

## Speed opportunities (not implemented)

* `empires.rs` `setup`: a `Vec` for Tax's piles and `landmark_score`'s `Vec` for Tower allocate (once per game / at the end of a game with Tower); an array like the one used for the Action piles would avoid it (about 50 ns per game).
* `pile_open` is called for every card the Buy decision considers; the Castle arm walks up to 7 ids. A cached "top castle" in `EmpState` would make it one compare (negligible except in Castle games).
* `Empires::gain_triggers` is entered for every gain by a card that has a bit in `GAIN_HI` / `GAIN_TOP`, and `gain_live_extra` reads three fields per gain; plain games pay only the mask test.
* `watch_gain` recomputes `treasure`, `action`, `victory` for each gain when any Landmark is in play; they could be computed once per type mask (a few ns per gain, Landmark games only).

## Ruling coverage

| Card | Ruling | Test |
|---|---|---|
| Engineer | cannot gain Debt-cost cards; gains, then may trash itself for a second gain (same card allowed) | `engineer_gains_cards_up_to_four_but_never_one_with_debt_and_may_trash_itself_for_another` |
| Engineer | Throne Room: trashes itself once | `a_throne_roomed_engineer_trashes_itself_once_with_another_engineer_in_play` |
| City Quarter | Crown (multi-type) counts as an Action | `city_quarter_counts_a_multi_type_action_such_as_crown` |
| City Quarter | +2 Actions, +1 Card per Action revealed | `city_quarter_gives_two_actions_and_a_card_per_action_in_the_hand` |
| Royal Blacksmith | discards the Coppers already in hand and the drawn ones | `royal_blacksmith_draws_five_then_discards_every_copper_in_the_hand` |
| Overlord | plays a non-Command Action up to $5 from the Supply and leaves it there; not Debt cost | `overlord_plays_an_action_costing_up_to_five_from_the_supply_and_leaves_it_there` |
| Overlord | no Duration / Command card | `overlord_cannot_play_a_duration_or_a_command_card` |
| Overlord | only the top card of a split pile; Encampment cannot set itself aside | `overlord_playing_encampment_cannot_set_it_aside_and_only_the_top_of_a_split_pile_is_offered` |
| Overlord | Farmers' Market fails to trash itself but takes the tokens | `overlord_playing_a_full_farmers_market_takes_the_tokens_but_cannot_trash_it` |
| Overlord | "while this is in play" abilities do nothing (Groundskeeper reworded) | not testable: Groundskeeper now says "this turn" |
| Overlord | stays in play until the Clean-up of the turn the card would leave | not tested separately: generic Duration / `NIP` machinery (`stop-moving-rule.md`) |
| Encampment | revealing is optional; Gold or Plunder keeps it | `encampment_stays_with_a_gold_or_plunder_and_goes_back_to_its_pile_without` |
| Encampment | returns on top of its pile (covering Plunder) | same test, plus `overlord_playing_encampment_...` for the cannot-move case |
| Encampment | returns at the start of Clean-up | `encampment_stays_out_of_its_pile_until_cleanup` (ignored, open) |
| Encampment | Throne Room: only one return | `a_throne_roomed_encampment_without_a_gold_goes_back_to_its_pile_once` |
| Encampment | cannot return (Black Market) stays set aside | not testable: Black Market is not implemented |
| Plunder | a VP token every time it is played | `plunder_gives_a_vp_token_each_time_it_is_played` |
| Patrician | Debt-only costs are not $5 or more (Fortune is, City Quarter / Royal Blacksmith are not) | `patrician_takes_a_revealed_fortune_but_not_a_card_with_only_debt`, `patrician_takes_a_revealed_card_costing_five_or_more_into_the_hand` |
| Emporium | counts Actions in play incl. Durations; needs 5 | `emporium_counts_duration_cards_in_play_and_needs_five_actions`, `emporium_gives_two_vp_tokens_when_gained_with_five_actions_in_play` |
| Emporium | Duplicate-ordering ruling | not testable: Duplicate (Adventures Reserve call timing) is not part of the order choice here |
| Settlers / Bustling Village | may take a Copper / Settlers from the discard pile; looking when none is there | `settlers_and_bustling_village_take_a_copper_or_a_settlers_from_the_discard_pile` (looking without a hit has no effect to test) |
| Catapult | Curse for $3+, discard to 3 for a Treasure, both for Silver, neither otherwise | `catapult_trashes_a_card_and_curses_for_three_or_more_and_cuts_hands_for_a_treasure` |
| Catapult | no card in hand: nothing | `catapult_with_no_card_left_in_hand_does_nothing` |
| Catapult | Curses in turn order, then the discards | `catapult_gives_the_curses_to_everyone_in_turn_order_before_anyone_discards` |
| Rocks | Silver on the deck in your Buy phase, else hand; trashed too | `rocks_give_a_silver_when_gained_or_trashed_onto_the_deck_in_the_buy_phase_else_into_the_hand` |
| Rocks | off-turn gain: Silver to the hand, never the discard pile | `rocks_silver_goes_straight_to_the_hand_or_deck_without_visiting_the_discard_pile_and_into_the_hand_off_turn` |
| Gladiator | +$1 and trash a Gladiator unless the left player reveals a copy | `gladiator_gives_a_coin_and_trashes_a_gladiator_unless_the_left_player_reveals_a_copy` |
| Gladiator | no Gladiators left: still +$1; empty hand: +$1 and trash | `gladiator_still_gives_the_coin_with_no_gladiators_in_the_supply_or_no_card_to_reveal` |
| Gladiator | not the top card of a rotated pile | not testable: rotation only exists for the Allies piles |
| Fortune | doubles once per turn; played after the other Treasures | `fortune_doubles_the_coins_once_and_is_played_after_the_other_treasures` |
| Fortune | gains a Gold per Gladiator in play when gained (Throne Room'd Gladiator counts once) | `fortune_gains_a_gold_per_gladiator_in_play_when_it_is_gained` |
| Fortune | a +$1 token on the pile counts before the doubling | `a_coin_token_on_the_gladiator_fortune_pile_counts_before_fortune_doubles` |
| Fortune | doubling only the $ held (Storyteller), -$1 token after doubling, Highwayman-blocked first Fortune | not testable: Storyteller / Ball / Highwayman interplay needs the Ally and Adventures Events together; the `doubled` flag is set only when a Fortune is played |
| Split piles | 5 + 5, bottom not for sale until the top is gone; empty top is not an empty pile | `split_piles_have_five_of_each_half_and_the_bottom_is_not_for_sale_until_the_top_is_gone`, `a_split_pile_is_one_pile_for_the_end_of_the_game` |
| Split piles | Training token works for both halves | `a_training_token_on_a_split_pile_works_for_both_of_its_halves` |
| Split piles | Ambassador returning a Plunder onto the pile, Young Witch Bane, randomizer cost | not testable: no randomizer / Bane over a split pile in the engine setup |
| Castles | 8 cards by cost (12 for 3+ players), top only | `the_castles_are_eight_cards_in_cost_order_with_the_top_one_for_sale` |
| Humble / King's Castle | count themselves | `castles_score_by_their_own_rules` |
| Crumbling Castle | +1 VP token and a Silver when gained or trashed | `crumbling_castle_gives_a_vp_token_and_a_silver_when_gained_or_trashed`, `salt_the_earth_trashing_a_crumbling_castle_does_its_trash_effect_and_tomb_counts_it` |
| Small Castle | trash itself or a Castle from hand; may pick with none in hand | `small_castle_trashes_itself_or_a_castle_from_the_hand_to_gain_the_top_castle`, `a_castle_in_hand_for_small_castle_is_optional_and_the_pile_top_is_gained` |
| Haunted Castle | own turn only; Gold; 5+ cards topdeck 2 | `haunted_castle_gains_a_gold_and_makes_the_others_with_five_cards_topdeck_two_but_only_on_its_owners_turn` |
| Haunted Castle | not an Attack (Moat does not stop it) | `haunted_castle_is_not_an_attack_a_moat_does_not_stop_it` |
| Haunted Castle | its ability comes before other players' when-gain abilities | not testable: Road Network / Black Cat are not implemented |
| Opulent Castle | discard Victory cards for $2 each | `opulent_castle_gives_two_coins_per_victory_card_discarded` |
| Sprawling Castle | Duchy or 3 Estates, either option whatever is left | `sprawling_castle_gains_a_duchy_or_three_estates_and_grand_castle_counts_victory_cards`, `sprawling_castle_offers_either_option_even_when_the_duchies_are_gone` |
| Grand Castle | VP tokens per Victory card in hand and in play | same as Sprawling test |
| Grand Castle | counts another player's Victory in play (Stronghold) | not testable: Stronghold is not implemented |
| Chariot Race | costs-more comparisons (Debt, ties, Fortune vs Overlord) | `chariot_race_draws_and_compares_the_cost_with_the_left_players_top_card` |
| Chariot Race | no card on either side: no bonus; -1 Card token | `chariot_race_gives_nothing_when_neither_player_has_a_card_to_reveal`, `chariot_race_with_a_minus_one_card_token_draws_and_reveals_nothing` |
| Chariot Race | Potion costs never more or less than Debt; Way of the Chameleon | not testable: Way of the Chameleon swaps +Cards and +$ (no reveal) and is covered by the Menagerie tests |
| Enchantress | first Action of each other player's turn is +1 Card +1 Action; ends at the owner's next turn; +2 Cards | `enchantress_makes_the_first_action_of_each_other_players_turn_a_cantrip_until_its_owners_next_turn` |
| Enchantress | several Enchantresses: still only the first card | `two_enchantresses_still_affect_only_the_first_action_of_the_other_players_turn` |
| Enchantress | below-the-line effects are not stopped; Sacrifice does nothing | `an_enchanted_sacrifice_is_a_cantrip_and_trashes_nothing` (the dividing-line part: not testable, no card with one remains) |
| Enchantress | Crown in the Buy phase gives +1 Card +1 Action | `an_enchanted_crown_in_the_buy_phase_gives_a_card_and_an_action` |
| Enchantress | an enchanted Duration is discarded this turn | `an_enchanted_duration_is_discarded_this_turn_and_does_nothing_next_turn` |
| Enchantress | Moat is revealed when it is played | `a_moat_must_be_revealed_when_enchantress_is_played_and_protects_from_it` |
| Enchantress | start-of-turn effects (Archive) are not enchanted; extra turns; Adventures tokens still apply | not tested: start-of-turn effects are not plays (`enchanted_play` is reached from the play path only); extra turns (Outpost) would need a full scenario |
| Farmers' Market | 1..4 then 4 VP and trash; works with an empty pile | `farmers_market_gathers_vp_for_four_plays_then_gives_them_to_the_fifth_and_trashes_itself`, `farmers_market_and_temple_still_gather_when_their_pile_is_empty` |
| Farmers' Market | Throne Room: second play starts the pile again | `a_throne_roomed_full_farmers_market_takes_the_tokens_and_the_second_play_starts_the_pile_again` |
| Farmers' Market | Black Market / Way of the Mouse has no pile | not testable: Black Market is not implemented, Way of the Mouse has no pile concept here |
| Sacrifice | bonuses by type (Crown gets all three); Curse gets nothing | `sacrifice_gives_a_bonus_for_each_type_of_the_trashed_card`, `sacrificing_a_crown_gives_all_three_bonuses_and_a_curse_gives_nothing` |
| Temple | +1 VP; 1-3 differently named cards; token added to the pile; gaining takes the pile | `temple_gathers_a_vp_token_on_its_pile_and_gaining_a_temple_takes_them` |
| Temple | token added even with an empty pile / empty hand | `farmers_market_and_temple_still_gather_when_their_pile_is_empty`, `temple_adds_its_vp_token_to_the_pile_even_with_an_empty_hand` |
| Villa | hand, +1 Action, back to the Action phase; Action phase gain; off-turn gain | `villa_goes_to_the_hand_with_an_action_and_returns_a_buyer_to_the_action_phase` |
| Villa | second Buy phase offers Arena again | `buying_a_villa_starts_a_second_buy_phase_in_which_arena_is_offered_again` |
| Villa | Replace / Summon fail to move it | `replace_cannot_move_a_villa_it_gained_because_it_has_gone_to_the_hand` (Summon: same code path) |
| Villa | Night / Clean-up gains; Gatekeeper; Cargo Ship set aside; Wine Merchant / Pageant count the Buy phase | not testable: no Night phase gain path to a Villa in the implemented sets for Empires, Gatekeeper interplay needs Menagerie (covered by the stop-moving tests of `cross_set.rs`) |
| Archive | 3 cards set aside, one now and at the start of the next two turns; stays in play | `archive_sets_aside_three_cards_and_gives_one_now_and_one_at_the_start_of_each_of_the_next_two_turns` |
| Archive | separate sets for two Archives | `two_archives_keep_separate_sets_of_cards` (ignored, open) |
| Archive | fewer than three cards; Throne Room stays in play | not tested separately: duration pending counts are `min(pool, times)`; the single-Archive test covers a full set |
| Capital | 6 Debt once per copy discarded from play; not for a trashed one; then pay with the $ | `capital_gives_six_debt_when_discarded_from_play_once_even_when_played_twice_and_the_coins_left_pay_it` |
| Capital | Highwayman-blocked Capital still takes the Debt | not testable: the Allies Highwayman cards run through the Allies rules, Capital is counted from `in_play` regardless |
| Charm | +1 Buy +$2, or the next gain also gains a differently named card of the same cost | `charm_gives_buy_and_coins_or_a_second_card_of_the_same_cost_with_the_next_gain` |
| Charm | spent by the card gained, not by what its trigger gains | `charm_applies_to_the_card_bought_not_to_the_silver_its_gain_trigger_gains` |
| Charm | Debt costs must match exactly (City Quarter for Overlord, not Fortune); no Debt for the Charm gain | `a_card_gained_by_charm_does_not_give_debt_even_if_it_costs_debt` |
| Charm | cumulative Charms; Destrier cost change; Knights; Mining Road | not testable: Destrier / Mining Road are not implemented; several Charms share one frame loop (`n` Charms push `n` gain frames) |
| Crown | Action phase an Action twice, Buy phase a Treasure twice | `crown_plays_an_action_twice_in_the_action_phase_and_a_treasure_twice_in_the_buy_phase` |
| Crown | Mining Village / Throne Room stop-moving | `a_crowned_mining_village_trashes_itself_once_with_another_in_play` |
| Crown | other phases: no play (Night, Clean-up) | not testable: those phases cannot play it in the implemented sets |
| Forum | +3 Cards +1 Action, discard 2; +1 Buy when gained (any way) | `forum_draws_three_discards_two_and_gives_a_buy_when_gained`, `forum_gives_its_buy_when_gained_any_way` |
| Groundskeeper | +1 VP per Victory gained this turn; cumulative; Crown / Throne Room doubles | `groundskeeper_gives_a_vp_token_for_each_victory_card_gained_this_turn_per_groundskeeper`, `a_throne_roomed_groundskeeper_gives_two_vp_tokens_per_victory_card_gained` |
| Legionary | reveal Gold: others discard to 2 then draw; Moat first | `legionary_reveals_a_gold_to_make_the_others_discard_to_two_and_draw_one`, `legionary_attack_waits_for_moat_before_the_gold_is_revealed` |
| Wild Hunt | 3 cards + token on the pile, or Estate and take the tokens; Estate option with no Estates gives no tokens | `wild_hunt_draws_and_adds_a_vp_token_or_gains_an_estate_and_takes_the_tokens`, `wild_hunt_offers_the_estate_option_even_when_the_estates_are_gone` |
| Wild Hunt | no pile (Black Market, Divine Wind) | not testable: neither exists |
| Debt | tokens stop buying cards and Events only; buying gives them; paid for $1 at any time; no Buy / Action used | `buying_a_card_with_debt_in_its_cost_takes_the_tokens_and_blocks_buying_until_paid`, `debt_does_not_stop_playing_cards_but_blocks_events_too`, `debt_is_paid_with_coins_at_any_buy_decision_and_leftover_coins_pay_it_at_the_end_of_the_turn` |
| Debt | gaining a Debt card other than by buying gives no Debt | `a_card_gained_by_charm_does_not_give_debt_even_if_it_costs_debt` |
| Debt | Bridge lowers only the $ part; Debt is not "up to $4" | `debt_in_a_cost_is_not_lowered_by_bridge`, `engineer_gains_cards_up_to_four_but_never_one_with_debt_and_may_trash_itself_for_another` |
| Debt | Knights 3-6 never Debt; Trader gives Silvers for $ only; Possession | not testable: no card with Debt and a $ cost 3-6 exists, Possession is not combined with Debt here |
| Advance | trash an Action for an Action up to $6; nothing otherwise | `advance_trashes_an_action_for_an_action_costing_up_to_six` |
| Annex | shuffle all but up to 5 (they stay in the discard pile), then a Duchy; works with no Duchies | `annex_shuffles_the_discard_pile_into_the_deck_but_up_to_five_chosen_cards_and_gains_a_duchy`, `annex_works_with_no_duchies_and_no_discard_pile` |
| Banquet | 2 Coppers and a non-Victory up to $5; no Coppers needed | `banquet_gains_two_coppers_and_a_card_costing_up_to_five_that_is_not_a_victory_card`, `banquet_gains_the_non_victory_card_even_with_no_coppers_left` |
| Conquest | 2 Silvers, VP per Silver gained this turn (earlier ones too; empty pile) | `conquest_gains_two_silvers_and_a_vp_token_per_silver_gained_this_turn`, `conquest_gives_vp_for_earlier_silvers_even_when_the_pile_is_empty` |
| Delve | +1 Buy, a Silver | `delve_gives_a_buy_back_and_a_silver` |
| Dominate | Province and +9 VP only if gained; | `dominate_gains_a_province_and_nine_vp_tokens_only_if_the_province_is_gained` |
| Dominate | still buyable with no Provinces; exchange via Trader / Changeling | `an_event_that_does_nothing_can_still_be_bought` (passes: Events are offered by `event_legal`; the bots skip useless ones via `event_available`); exchange: not testable (Changeling not implemented) |
| Donate | start of next turn, first; whole deck and discard into hand; trash; shuffle; draw 5; trash effects before the shuffle | `donate_trashes_from_the_whole_deck_at_the_start_of_the_next_turn_and_draws_five`, `donate_trashing_a_rocks_gives_the_silver_before_the_shuffle` |
| Donate | before Archive; Possession; Star Chart; game over first | not tested: ordering is by stack position (pushed over the other start effects); Star Chart / Possession not combined |
| Ritual | Curse, then trash a card, VP per $; nothing without Curses | `ritual_gains_a_curse_and_trashes_a_card_for_a_vp_token_per_coin_it_costs` |
| Ritual | cost is checked after trashing (Wayfarer / Market Square) | not testable cheaply: needs Wayfarer with a Market Square reaction in the trash queue |
| Salt the Earth | +1 VP, trash a Victory card from the Supply (top of a pile); trash effects | `salt_the_earth_gives_a_vp_token_and_trashes_a_victory_card_from_the_supply`, `salt_the_earth_trashing_a_crumbling_castle_does_its_trash_effect_and_tomb_counts_it` |
| Tax | 1 Debt per pile; +2 on a pile; taken by a gain in the Buy phase (also non-bought) | `tax_starts_with_one_debt_on_every_pile_adds_two_and_is_taken_by_a_gain_in_the_buy_phase`, `tax_is_taken_by_a_card_gained_in_the_buy_phase_by_an_event` |
| Tax | not taken by a gain outside the Buy phase / another player's Buy phase | `tax_leaves_the_debt_on_a_pile_gained_outside_the_buy_phase` (another player's Buy phase: Messenger is not implemented, same `buy_phase` test on the turn player) |
| Triumph | Estate, VP per card gained this turn, Events do not count | `triumph_gains_an_estate_and_a_vp_token_per_card_gained_this_turn`, `triumph_with_the_estates_gone_is_not_offered_and_gained_cards_count_but_events_do_not` |
| Wedding | +1 VP and a Gold even with no Golds | `wedding_gives_a_vp_token_and_a_gold_for_four_coins_and_three_debt`, `wedding_gives_its_vp_token_even_when_no_golds_are_left_and_windfall_takes_what_is_left` |
| Windfall | 3 Golds only with deck and discard empty; fewer left: the rest | `windfall_gains_three_golds_only_with_an_empty_deck_and_discard_pile`, `wedding_gives_its_vp_token_even_when_no_golds_are_left_and_windfall_takes_what_is_left` |
| Aqueduct | VP moves from a Treasure's pile; Victory takes them; Crown gives to Aqueduct and Shrine | `aqueduct_moves_a_vp_token_from_a_treasure_pile_and_a_victory_card_takes_them`, `aqueduct_and_defiled_shrine_each_take_a_token_when_a_crown_is_gained` |
| Aqueduct | Capitalism, Treasurer / Hoard gains | not testable: Capitalism, Treasurer not implemented; Hoard's Gold goes through the same gain path |
| Arena | discard an Action at the start of each Buy phase for 2 VP; again after Villa | `arena_lets_an_action_be_discarded_at_the_start_of_the_buy_phase_for_two_vp_tokens`, `buying_a_villa_starts_a_second_buy_phase_in_which_arena_is_offered_again` |
| Arena | discard allowed with no tokens left | `arena_still_offers_the_discard_when_it_has_no_tokens_left` (ignored, open) |
| Bandit Fort / Fountain / Palace | scoring, no bonus for 20 Coppers | `bandit_fort_fountain_and_palace_score_from_the_treasures` |
| Basilica | gain in the Buy phase with $2; Events gain counts, buying an Event does not | `basilica_gives_two_vp_tokens_for_a_gain_in_the_buy_phase_with_two_coins_left`, `basilica_pays_for_a_card_gained_by_an_event_but_not_for_buying_an_event` |
| Basilica | Nomads +$2 with $0; overpay | not testable: Nomads is not implemented; Stonemason overpay is paid before the gain by the generic overpay code |
| Baths | 2 VP at the end of a turn with no gain | `baths_gives_two_vp_tokens_to_a_player_who_ends_a_turn_without_gaining_a_card` |
| Baths | Possession; exchange (Bat); game-ending turn | not tested: Possession / Bat are not combined with Landmarks; Clean-up runs before the end-of-game check |
| Battlefield | 2 VP per Victory card gained, bought or not | `battlefield_gives_two_vp_tokens_for_each_victory_card_gained`, `battlefield_takes_tokens_for_a_victory_card_gained_outside_the_buy_phase_too` |
| Colonnade | Action gained in the Buy phase with a copy in play; per copy; not in the Action phase | `colonnade_rewards_gaining_an_action_in_the_buy_phase_while_a_copy_is_in_play`, `colonnade_gives_nothing_for_an_action_gained_in_the_action_phase_and_pays_per_copy_gained` |
| Colonnade | cards played by Overlord are not in play; Estates under Inheritance | not testable here: Overlord's card is never in `in_play` by construction; Inheritance is not combined |
| Defiled Shrine | 2 VP on non-Gathering Action piles; Action gained moves 1; Curse gained in your Buy phase takes them | `defiled_shrine_collects_vp_from_action_piles_and_a_curse_bought_takes_them`, `defiled_shrine_takes_the_tokens_for_a_curse_gained_by_an_event_in_the_buy_phase_but_not_in_another_players_buy_phase` |
| Defiled Shrine | Ruins pile gets tokens, Gathering piles do not | `defiled_shrine_puts_tokens_on_the_ruins_pile_and_not_on_the_gathering_piles` |
| Defiled Shrine | Temple gained takes all its tokens first | `temple_gathers_a_vp_token_on_its_pile_and_gaining_a_temple_takes_them` plus the order in `Empires::gain_triggers` (pile tokens taken before `watch_gain`) |
| Fountain | 15 VP for 10+ Coppers only once | `bandit_fort_fountain_and_palace_score_from_the_treasures` |
| Keep | per Treasure with most or tied; Curses count with Charlatan | `keep_gives_five_vp_per_treasure_you_have_the_most_or_tied_for_the_most_of`, `keep_counts_curses_as_treasures_in_a_game_with_charlatan` |
| Labyrinth | the second gain of a turn only | `labyrinth_gives_two_vp_tokens_for_the_second_card_gained_in_a_turn_only` |
| Mountain Pass | bids from the player to the left, ending with the gainer; high bid 8 VP and Debt; only the first Province | `mountain_pass_has_each_player_bid_debt_for_eight_vp_when_the_first_province_is_gained` |
| Mountain Pass | any way of gaining the Province; all pass: nothing; 40 cannot be beaten | `mountain_pass_starts_when_a_province_is_gained_by_an_event_and_all_passes_give_nothing`, `a_bid_of_forty_cannot_be_beaten` |
| Mountain Pass | Province removed by Salt the Earth does not matter; Possession | not tested: `pass_done` is set by the first gain only; Possession not combined |
| Museum | 2 VP per differently named card | `museum_obelisk_orchard_triumphal_arch_wall_and_wolf_den_score_from_the_deck` |
| Obelisk | all cards of the pile (split pile), Action-Victory card also scores its VP | `obelisk_counts_every_card_of_a_split_pile_and_an_action_victory_card_also_scores_its_vp` |
| Orchard | 4 VP per Action with 3+ copies; 6 is no more | `museum_obelisk_orchard_triumphal_arch_wall_and_wolf_den_score_from_the_deck` |
| Tomb | +1 VP per card trashed; on another player's turn; Salt the Earth | `tomb_gives_a_vp_token_for_each_card_trashed`, `tomb_gives_its_token_to_the_player_who_trashes_even_on_another_players_turn` |
| Tower | non-Victory cards of empty piles; a split pile with half left is not empty; Curses count | `tower_scores_a_point_for_each_non_victory_card_from_an_empty_supply_pile` |
| Triumphal Arch | second most common Action; ties either for first or second | `museum_obelisk_orchard_triumphal_arch_wall_and_wolf_den_score_from_the_deck`, `triumphal_arch_counts_a_tie_for_first_place_as_second` |
| Wall / Wolf Den | -1 per card after 15; -3 per single copy | `museum_obelisk_orchard_triumphal_arch_wall_and_wolf_den_score_from_the_deck` |
| Landmarks | 6 tokens per player; Defiled Shrine / Aqueduct set-up | `landmarks_with_tokens_start_with_six_per_player` |
