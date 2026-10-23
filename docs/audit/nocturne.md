# Nocturne audit

Scope: the 33 kingdom cards, the 7 Heirlooms, Will-o'-Wisp, Wish, Bat, Imp, Ghost and the 3 Zombies, the 12 Boons, the 12 Hexes, the 5 States, the Night phase
and the set-up rules (Heirlooms, piles outside the Supply, Druid, Necromancer, Fool). Source: the wiki copy (every card, Boon, Hex and State page, plus
`Nocturne`, `Night`, `Boon`, `Hex`, `State`, `Heirloom`, `Exchange`, `Zombie`). Worktree branch: `worktree-agent-a003b86eb4c7b48ac`, based on `9f3a560`.

Items checked: 33 + 7 + 8 + 12 + 12 + 5 = 77 cards/effects, plus the Night phase and set-up.

## Bugs found and fixed

| # | Bug | Rule clause | Test |
|---|---|---|---|
| 1 | Exorcist trashing a card with Debt in its cost (Engineer, `$0` + 4 Debt) offered no Spirit, although Will-o'-Wisp (`$0`) is cheaper | "Gain a cheaper Spirit": cost compares Coin, Potion and Debt | `exorcist_trashing_a_debt_card_can_gain_a_spirit_that_costs_less_debt` |
| 2 | Locusts on a Debt card never gained anything (City Quarter should allow Engineer) | Locusts page: "cheaper cards are those that cost less $, P or Debt and also don't cost more" (Ruined Village and Engineer are cheaper than City Quarter) | `locusts_on_a_debt_card_gains_a_card_with_less_debt` |
| 3 | The Changeling exchange used the cost after the gain: a Fisherman gained to an empty discard pile (cost $2 then) could be exchanged. The same wrong cost sat in Menagerie's `cost_when_gained` (Livery, Galleria-like uses): a Fisherman gained to an empty discard pile read `$5` | Changeling: "This checks the cost of the card as you're gaining it, not after" | `changeling_exchange_uses_the_cost_at_the_time_of_the_gain` |
| 4 | Leprechaun counted the cards in play before the reactions to its Gold ran, so a Sheepdog played in reaction did not count toward exactly seven | Leprechaun: "This checks the number of cards you have in play after gaining the Gold" (Sheepdog example) | `leprechaun_counts_after_the_gold_is_gained_and_reacted_to` |
| 5 | Bot: Secret Cave's "discard 3" chose Pass where none was offered once two cards were gone (illegal choice, crashed a search playout and whole games) | -- (bot default) | `secret_cave_finishes_the_three_discards_it_starts`, `bots_with_no_rules_play_every_nocturne_card_legally` |
| 6 | Bot: The Sky's Gift received from an Idol in the Buy phase discarded Treasures still to be played | Boon strategy: discard dead cards | `skys_gift_in_the_buy_phase_keeps_the_treasures_left_in_hand` |

## Issues left open

None with a failing test. Known simplifications that are documented in `docs/nocturne-plan.md` section 8 (not changed): Crypt with two copies pools the
set-aside Treasures; a Knight or Ruins returned by Changeling forgets the old top card; no Possession. One behaviour that the wiki does not settle: if every
victim of a Hex attack is immune no Hex is turned over (the plan's ruling; the rules text does not say; it only changes which Hex comes later).

## Card by card

All OK unless noted. (Text, costs, types, numbers, clauses and order, "may" against mandatory, who chooses, where cards go, empty deck/pile/hand, Durations, gain/trash/discard
triggers, Attack immunity, replays, stop-moving rule, VP, set-up.)

* Druid OK. Faithful Hound OK. Guardian OK (protection ends at the start of the owner's next turn; all Attacks, not only the first). Monastery OK. Pixie OK. Tracker OK
  (current "this turn" text; topdecks before the Boon). Changeling: bug 3. Fool OK. Ghost Town OK. Leprechaun: bug 4. Night Watchman OK. Secret Cave OK.
* Bard OK. Blessed Village OK. Cemetery OK. Conclave OK. Devil's Workshop OK. Exorcist: bug 1. Necromancer OK (2021 text). Shepherd OK. Skulk OK.
* Cobbler OK. Crypt OK (no Durations). Cursed Village OK. Den of Sin OK. Idol OK. Pooka OK. Sacred Grove OK. Tormentor OK. Tragic Hero OK. Vampire OK. Werewolf OK. Raider OK.
* Heirlooms (Haunted Mirror, Magic Lamp, Goat, Pasture, Pouch, Cursed Gold, Lucky Coin) OK; each replaces a starting Copper, the Coppers return to the pile.
* Will-o'-Wisp, Wish, Bat, Imp, Ghost, Zombie Apprentice / Mason / Spy OK. Pile sizes (Wisp 12, Wish 12, Bat 10, Imp 13, Ghost 6) OK.
* Boons: all 12 OK (Field's, Forest's and River's kept until Clean-up; River's draws after the next hand). Hexes: all 12 OK except Locusts (bug 2). States OK
  (Miserable and Twice Miserable are -2 / -4 VP, not cumulative).
* Night phase: OK (after Buy, before Clean-up; Treasury etc. after it; Werewolf checks for its own player's Night phase).

`choice_free` / `has_choice` / `order_sensitive` marks: all agree with the rule in `CLAUDE.md` (the `choice_free_and_order_sensitive_cards` tests pass).
Existing test names versus content: all test names match what they check; two older tests (`monastery_...`, `devils_workshop_...`) were extended, not changed.

## Ruling coverage

"E" = existing test, "N" = added in this audit (all in `crates/engine/tests/nocturne_cards.rs`, bot tests in `crates/sim/tests/nocturne_defaults.rs`).

| Card | Ruling | Test |
|---|---|---|
| Druid | 3 Boons set aside at set-up; other Fate cards use the other nine | `druid_sets_aside_three_boons_face_up` |
| Druid | receive one and leave it there, even a "keep until Clean-up" Boon | `druid_receives_one_of_the_set_aside_boons_and_leaves_it_there` |
| Faithful Hound | set aside when discarded outside Clean-up, back in hand after the draw | `faithful_hound_draws_two_and_can_be_set_aside_...` |
| Faithful Hound | works on other players' turns (Militia) | `a_hound_discarded_to_an_attack_returns_at_the_end_of_that_turn` |
| Faithful Hound | need not be in hand (Night Watchman discards it from the deck) | `faithful_hound_is_offered_when_discarded_from_the_deck_but_not_when_gained` |
| Faithful Hound | no effect when gained or when the deck goes to the discard pile without discarding | same test; `bad_omens_is_not_a_discard` |
| Faithful Hound | optional; none in Clean-up | `faithful_hound_draws_two_and_can_be_set_aside_...` |
| Faithful Hound | set aside from Vassal: Vassal cannot play it | `a_hound_set_aside_from_vassal_is_not_played` |
| Faithful Hound | Arena (Landmark) | not tested: the Landmark interaction is Empires' to cover |
| Guardian | protects until the owner's next turn, +$1 then | `guardian_protects_its_owner_until_their_next_turn` |
| Guardian | Reactions still usable | `guardian_does_not_stop_reactions_to_the_attack` |
| Guardian | gained to the hand; onto the deck it stays there | `night_cards_are_gained_to_the_hand_and_can_be_played_the_turn_they_are_bought`, `night_cards_gained_onto_the_deck_stay_there` |
| Guardian | protects the rest of the turn it is played (Vampire gain, Black Cat) | `guardian_protects_from_an_attack_later_in_the_same_night_phase` |
| Guardian | no protection during the next turn | `guardian_protects_its_owner_until_their_next_turn` |
| Monastery | one trash per card gained; hand card or Copper in play, any mix | `monastery_trashes_one_card_per_card_gained_this_turn` |
| Monastery | exchange (Vampire to Bat) is not a gain | `an_exchange_is_not_a_gain_for_devils_workshop` |
| Monastery | gains while trashing do not add trashes | `monastery_gains_while_trashing_do_not_add_trashes` |
| Monastery | cards drawn mid-trash can be trashed | not tested: no card in the set trashes and draws in one step; the hand pick is recomputed each pick |
| Pixie | discard top Boon; may trash to receive twice | `pixie_discards_the_top_boon_and_may_be_trashed_to_receive_it_twice` |
| Pixie | kept Boon received twice | `pixie_and_a_kept_boon_twice` |
| Pixie | Band of Misfits / Necromancer / Overlord play: cantrip only | `pixie_played_by_band_of_misfits_cannot_be_trashed`, `necromancer_playing_a_pixie_cannot_trash_it`, `a_pixie_played_by_overlord_is_not_in_play_to_be_trashed` |
| Pixie | Throne Room trashes it once | `a_throne_roomed_pixie_is_trashed_once_even_with_another_pixie_in_play` |
| Pixie | Goat Heirloom | `heirlooms_replace_a_starting_copper_and_the_coppers_go_to_the_pile` |
| Tracker | topdeck each gained card, before the Boon, bought or not | `tracker_gives_a_coin_a_boon_and_topdecks_what_is_gained` |
| Tracker | Necromancer play (current "this turn" text) | `necromancer_on_tracker_gives_the_topdecking_this_turn` |
| Changeling | exchange when gaining a card costing $3+, only if returnable and a Changeling is in the pile | `a_card_costing_three_or_more_may_be_exchanged_for_a_changeling_when_gained`, `no_exchange_for_a_changeling_when_the_pile_is_empty` |
| Changeling | goes to the discard pile; other gain effects still happen (Skulk) | `a_card_costing_three_or_more_...` |
| Changeling | optional; below $3 not offered | `a_card_costing_three_or_more_...` |
| Changeling | cost at the time of the gain (Fisherman) | `changeling_exchange_uses_the_cost_at_the_time_of_the_gain` |
| Changeling | non-Supply card with a pile (Ghost) can be exchanged | `a_ghost_can_be_exchanged_for_a_changeling` |
| Changeling | the exchange is not a gain | `a_card_costing_three_or_more_...` (third part) |
| Changeling | play: trash itself, gain a copy of a card in play; cannot copy itself | `changeling_trashes_itself_and_gains_a_copy_of_a_card_in_play` |
| Changeling | nothing for non-Supply piles (Imp), empty piles | `changeling_gains_nothing_for_non_supply_or_empty_piles` |
| Changeling | split piles by top-card name, Possession, Black Market / Ferryman pile, Loot back on top, Exile, Villa lose-track order | not tested: Possession is not in the engine; the rest need set combinations that the Changeling code path does not special-case (the exchange is always asked first, so a card that moves itself is still tracked) |
| Fool | take Lost in the Woods (from others), 3 Boons in any order, one at a time | `fool_takes_lost_in_the_woods_and_receives_three_boons_in_any_order` |
| Fool | nothing if you already have it | same test |
| Fool | Lost in the Woods: optional discard each turn | `lost_in_the_woods_discards_a_card_at_the_start_of_each_turn_for_a_boon` |
| Ghost Town | +1 Card +1 Action next turn; gained to hand | `durations_played_at_night_stay_in_play_and_fire_next_turn`, `night_cards_are_gained_to_the_hand_...` |
| Leprechaun | Gold, then exactly 7 cards in play: Wish, else Hex; trashed cards do not count | `leprechaun_gains_a_gold_and_a_wish_with_exactly_seven_cards_in_play_else_a_hex` |
| Leprechaun | counts after the Gold's reactions (Sheepdog) | `leprechaun_counts_after_the_gold_is_gained_and_reacted_to` |
| Night Watchman | top 5, discard any, rest back in any order; gained to hand | `night_watchman_looks_at_the_top_five` |
| Night Watchman | a card put on the deck meanwhile (Progress) ends under the cards put back | `night_watchman_puts_the_rest_back_on_top_of_a_card_put_on_the_deck_meanwhile` |
| Secret Cave | +1 Card +1 Action; discard 3 for +$3 next turn | `secret_cave_discards_three_for_three_coins_next_turn` |
| Secret Cave | fewer than three: discard what there is, no bonus, leaves play | same test |
| Secret Cave | Magic Lamp Heirloom | `magic_lamp_trashes_itself_for_three_wishes_with_six_single_cards_in_play` |
| Bard | +$2 and a Boon | `bard_gives_two_coins_and_a_boon` |
| Blessed Village | Boon seen first, now or next turn | `blessed_village_takes_a_boon_when_gained_to_receive_now_or_next_turn` |
| Blessed Village | a kept Boon saved for next turn is kept until that Clean-up | `a_kept_boon_saved_by_blessed_village_is_kept_until_the_end_of_the_next_turn` |
| Blessed Village | Gatekeeper / Changeling lose-track examples | not tested: they depend on Menagerie's Gatekeeper tracking, covered by the generic stop-moving tests in `cross_set.rs` |
| Cemetery | trash 0-4 from hand when gained; Haunted Mirror Heirloom | `cemetery_trashes_up_to_four_cards_when_gained_and_comes_with_haunted_mirror` |
| Conclave | play an Action you have no copy of in play, then +1 Action | `conclave_plays_an_action_you_do_not_have_in_play_for_an_extra_action` |
| Conclave / Imp | not a Duration from an earlier turn; not an Imp with an Imp in play | `imp_and_conclave_cannot_play_a_copy_of_a_card_in_play` |
| Devil's Workshop | 0 / 1 / 2+ gains; counts Night gains; no choice of lesser benefit | `devils_workshop_depends_on_the_cards_gained_this_turn`, `devils_workshop_counts_night_gains_and_cannot_choose_a_lesser_benefit` |
| Devil's Workshop | Possession | not testable: no Possession in the engine |
| Exorcist | trash a card, gain a cheaper Spirit | `exorcist_trashes_a_card_and_gains_a_cheaper_spirit` |
| Exorcist | Debt in the trashed cost | `exorcist_trashing_a_debt_card_can_gain_a_spirit_that_costs_less_debt` |
| Necromancer | non-Duration face-up Action from the trash, once each per turn, stays there | `necromancer_plays_an_action_from_the_trash_once_per_turn_each` |
| Necromancer | can play Necromancer; Command cards; Encampment stays in the trash | `necromancer_can_play_a_necromancer_from_the_trash`, `necromancer_plays_command_cards_and_leaves_a_card_that_would_move_in_the_trash` |
| Necromancer | Tragic Hero works (gains), not trashed | `necromancer_on_tragic_hero_gains_the_treasure_without_trashing_it` |
| Necromancer | Island stays in the trash; Lurker can gain itself | `necromancer_playing_island_leaves_it_in_the_trash`, `necromancer_playing_lurker_can_gain_it_out_of_the_trash` |
| Necromancer | trashing a card in the trash is not trashing (Tomb, Sewers) | not tested: Tomb/Sewers not exercised; the engine trashes nothing there (Tragic Hero test checks the trash count) |
| Necromancer | gained face-down card trashed again becomes playable | not tested: needs a gain from the trash and a second trash in one turn; the per-turn ledger counts plays, not cards |
| Shepherd | +1 Action, discard Victory cards for 2 Cards each | `shepherd_discards_victory_cards_for_two_cards_each` |
| Shepherd | the reshuffle includes the discarded Victory cards (Tunnel's Gold too) | `shepherds_reshuffle_includes_the_discarded_victory_cards` |
| Skulk | +1 Buy, next Hex to the others; Gold when gained any way | `skulk_hexes_the_others_with_one_hex_and_buys`, `skulk_gains_a_gold_when_gained_without_buying` |
| Cobbler | gain up to $4 to the hand next turn | `cobbler_gains_a_card_costing_up_to_four_to_the_hand_next_turn` |
| Cobbler | Nomad Camp goes to the hand | not testable: Nomad Camp is not in the engine |
| Crypt | set aside any number of non-Duration Treasures; one per turn | `crypt_sets_treasures_aside_and_returns_one_each_turn` |
| Crypt | Duration Treasures stay out | `crypt_leaves_duration_treasures_in_play` |
| Crypt | Crown (a Throne variant) as a Treasure | not testable: Crown is not in the engine |
| Cursed Village | +2 Actions, draw to 6 (none at 6+); Hex when gained | `cursed_village_draws_to_six_and_hexes_its_owner_when_gained`, `cursed_village_with_six_cards_draws_nothing` |
| Den of Sin | +2 Cards next turn; gained to hand | `den_of_sin_draws_two_cards_next_turn` |
| Idol | odd count: Boon, even: Curses; counts Idols in play | `idol_alternates_between_a_boon_and_curses`, `idol_counts_idols_in_play_not_plays` |
| Idol | no Curses left: no Boon instead | `idol_with_an_even_count_and_no_curses_does_nothing` |
| Pooka | trash a Treasure except Cursed Gold for +4 Cards | `pooka_trashes_a_treasure_other_than_cursed_gold_for_four_cards` |
| Sacred Grove | Boon for you, others may, not Field's/Forest's | `sacred_grove_shares_the_boon_unless_it_gives_a_coin` |
| Sacred Grove | River's Gift: each player who takes it draws at the end of the turn | `sacred_groves_rivers_gift_draws_for_the_players_who_took_it` |
| Tormentor | Imp with no other cards in play, else Hex | `tormentor_gains_an_imp_with_no_other_cards_in_play_else_hexes` |
| Tormentor | played by Overlord: the Overlord counts | `tormentor_played_by_overlord_hexes_because_the_overlord_is_in_play` |
| Tormentor | Way of the Horse then Citadel (nothing in play) | not tested: needs a Way and a Project together; the code takes the play area as it is |
| Tragic Hero | +3 Cards +1 Buy; 8+ cards: trash and gain a Treasure | `tragic_hero_draws_three_and_with_eight_cards_trashes_itself_for_a_treasure` |
| Tragic Hero | cannot be trashed (second Throne Room play): still gains | `tragic_hero_still_gains_a_treasure_when_it_cannot_be_trashed` |
| Vampire | Hex, gain up to $5 not a Vampire, exchange for a Bat in order | `vampire_hexes_gains_a_card_up_to_five_and_turns_into_a_bat` |
| Vampire | no Bat left: the rest happens | `vampire_without_a_bat_left_keeps_the_vampire` |
| Werewolf | +3 Cards by day, Hex at Night | `werewolf_draws_by_day_and_hexes_at_night` |
| Werewolf | still an Attack by day | `werewolf_by_day_is_still_an_attack_for_reactions` |
| Werewolf | Scepter in the Buy phase: +3 Cards | `werewolf_replayed_by_scepter_in_the_buy_phase_draws` |
| Werewolf | Royal Carriage at Night; played at the start of the turn (Delay) | not tested: Royal Carriage's call is covered in `adventures_cards.rs`; the Night check reads `phase == Night` and the turn player, tested above |
| Raider | discard a copy of a card in play (5+ cards), +$3 next turn | `raider_attacks_players_with_five_cards_and_pays_three_coins_next_turn` |
| Raider | no copy: reveal, discard nothing | `raider_vs_a_hand_with_no_copy_discards_nothing` |
| Heirlooms | replace a Copper; Coppers to the pile; Shelters too; not returned by Panic | `heirlooms_replace_a_starting_copper_and_the_coppers_go_to_the_pile`, `a_game_with_shelters_and_an_heirloom_has_both`, `panic_does_not_return_an_heirloom` |
| Heirlooms | Ambassador / Trader | not testable: Ambassador is not in the engine; an Heirloom is never gained so Trader cannot see it |
| Haunted Mirror | trash: discard an Action to gain a Ghost | `haunted_mirror_gains_a_ghost_when_trashed_for_an_action_discard` |
| Magic Lamp | six single cards, counts itself, two Coppers count none | `magic_lamp_trashes_itself_for_three_wishes_with_six_single_cards_in_play` |
| Goat, Pouch, Pasture | trash optional; +1 Buy; 1 VP per Estate | `the_heirlooms`, `lucky_coin_can_be_left_in_the_hand_and_goat_may_trash_nothing` |
| Cursed Gold, Lucky Coin | may be left unplayed | `the_heirlooms`, `lucky_coin_can_be_left_in_the_hand_and_goat_may_trash_nothing` |
| Will-o'-Wisp | reveal; $2 or less to hand; Potion/Debt cost more | `will_o_wisp_takes_the_revealed_card_only_if_it_costs_two_or_less`, `will_o_wisp_leaves_cards_with_potion_or_debt_in_the_cost_on_the_deck` |
| Wish | return, if returned gain up to $6 to hand | `wish_returns_to_its_pile_and_gains_a_card_costing_up_to_six_to_the_hand` |
| Bat | trash up to 2, exchange for a Vampire (not if none left) | `bat_trashes_up_to_two_cards_and_if_it_did_turns_into_a_vampire`, `bat_without_a_vampire_left_still_trashes` |
| Imp | +2 Cards, play an Action not in play | `imp_draws_two_and_may_play_an_action_that_is_not_in_play` |
| Ghost | reveal until an Action, shuffle discard but not revealed, play twice next turn | `ghost_sets_an_action_aside_and_plays_it_twice_next_turn`, `ghost_shuffles_the_discard_pile_but_not_the_revealed_cards` |
| Ghost | no Action: discard, Ghost leaves; uses no Action plays; Duration kept; self-trasher; Horse | `ghost_sets_an_action_aside_...`, `ghost_plays_the_action_twice_without_using_an_action`, `ghost_stays_in_play_with_a_duration_it_played`, `ghost_plays_a_card_that_trashed_itself_again`, `ghost_plays_a_horse_twice` |
| Ghost | order among several start-of-turn effects | not tested: start order is a generic frame (`StartOrder`), no Ghost-specific code |
| Zombies | Apprentice, Mason (optional gain, may gain same), Spy (draws first) | `the_zombies`, `zombie_mason_may_gain_nothing_and_zombie_spy_draws_before_looking` |
| Boons | each of the 12 | one test per Boon, `the_earths_gift_...` to `the_winds_gift_...` |
| Boons | Moon's with an empty discard; Sky's with fewer than three cards; Sun's mid-resolution topdeck | `moons_gift_with_an_empty_discard_pile_and_skys_gift_with_two_cards`, `night_watchman_puts_the_rest_back_on_top_of_a_card_put_on_the_deck_meanwhile` (same mechanism) |
| Boons | reshuffle, none available, random order | `the_boons_deck_is_reshuffled_from_the_discard_pile_when_it_runs_out`, `boons_are_drawn_at_random_from_the_deck_and_every_one_comes_up` |
| Hexes | each of the 12 | `bad_omens_...`, `delusion_...`, `envy_...`, `famine_...`, `fear_...`, `greed_haunting_plague_and_poverty`, `locusts_...`, `misery_...`, `war_...` |
| Hexes | one Hex for all victims; Moat; none if nobody affected | `skulk_hexes_the_others_with_one_hex_and_buys`, `a_hex_is_blocked_by_moat` |
| Hexes | Bad Omens not a discard; Famine all-Actions shuffle / empty deck; Fear reveal; Poverty at 3; War with no 3-4; Locusts with Debt / Potion | `bad_omens_is_not_a_discard`, `famine_shuffles_the_deck_even_if_all_three_were_actions_and_never_the_discard_pile`, `fear_only_reveals_a_hand_without_actions_or_treasures`, `poverty_leaves_three_cards_alone`, `war_without_a_three_or_four_in_the_deck_discards_the_deck_and_trashes_nothing`, `locusts_on_a_debt_card_gains_a_card_with_less_debt`, `locusts_on_a_potion_card_gains_cheaper_cards_of_a_shared_type` |
| States | Deluded: from the Buy phase; applies the turn it is taken before Buy; gaining Actions allowed | `delusion_stops_buying_actions_for_one_turn`, `deluded_taken_in_the_action_phase_applies_to_that_turn`, `deluded_does_not_stop_gaining_actions` |
| States | Deluded and Black Market, second Buy phase | not testable: no Black Market buy path before the Buy phase in the engine; the flag is per turn |
| States | Envious: Silver and Gold only, in the Buy phase, for the turn | `envy_makes_silver_and_gold_worth_one_coin_for_a_turn`, `envious_only_changes_silver_and_gold_played_in_the_buy_phase` |
| States | Envious with Villa / Enlightenment / Highwayman; Storyteller plays before the Buy phase | not tested: the flag is per turn (persists), set only at the Buy phase start (so earlier plays are unaffected) |
| States | only one of Deluded/Envious; Miserable then Twice Miserable, a third does nothing | `a_player_has_only_one_of_deluded_and_envious`, `misery_takes_two_vp_then_four` |
| Night | after Buy, any number, no Action used; Night cards in hand only | `night_follows_the_buy_phase_only_for_a_player_holding_a_night_card`, `a_game_without_night_cards_has_no_night_phase` |
| Set-up | Zombies, piles, Boons/Hexes only with Fate/Doom, Exorcist's three piles | `the_extra_piles_come_with_the_cards_that_use_them`, `boons_and_hexes_are_present_only_with_a_fate_or_doom_card` |

Coverage counts: 117 rows (rulings or groups of rulings): 102 name a test, 15 are "not tested" / "not testable" with the reason given. 41 of the tests named were added in this audit.

## Policy notes

* Fixed (with tests): Secret Cave and The Sky's Gift, above.
* Checked with tests, no change needed: Pooka trashes the cheapest Treasure (`pooka_trashes_the_cheapest_treasure`); Tragic Hero gains Gold (`tragic_hero_gains_the_best_treasure`);
  Exorcist trashes a Curse before a Gold (`exorcist_style_trashers_do_not_trash_gold`).
* Pixie: the bot declines to trash Pixie for a doubled Field's or Forest's Gift. Doubling gives +2 Actions / +$2 or +2 Buys / +$2, which can be worth a cantrip in a
  Buy-heavy turn, but the wiki only says "you can simply save your Pixie till later" (Pixie strategy). Left alone.
* Fool: the bot plays Fool even when it already holds Lost in the Woods (a dead terminal). Wiki: "Fool carries a heavy downside in that it is a terminal stop card that
  generally provides no benefit at all during turns when you already have Lost in the Woods" and "you may consider trashing Fool if your opponent doesn't gain one". Default `worth_playing`
  returns true; a rule `if = "lost_in_the_woods == 1"` skipping it is available to strategies. Not changed (no test shows a loss).
* Night Watchman: wiki "Only discard up to one Estate ... because discarding more than that will trigger a premature reshuffle" (opening trick); the bot searches, which finds it only when the leaf value sees the next hand.
* Cursed Gold: wiki "you have to decide whether the $3 is worth the penalty"; the default always plays it when a card is wanted. Not changed.
* Faithful Hound: set aside whenever possible (always right: no downside).

## Speed opportunities (not implemented)

* `effects.rs` `push_turn_start_frames`: `self.players[pi].noc &= !P_GUARDED` is an unconditional byte write each turn (negligible; could sit behind `noc.flags`).
* `engine.rs` `immune()` reads `players[victim].noc` for every Attack victim: one byte test, fine.
* Bots with search spend 10-30 s per game on some all-Nocturne kingdoms (see `bots_with_no_rules_play_every_nocturne_card_legally`: games 0, 2, 5, 9 of 12). The cost is Boon / Hex chance nodes
  (`BOON_CHANCE`, `HEX_CHANCE`: 12-way branching at every Fate / Doom play) multiplied by the Night phase's play orders. Estimate: capping chance expansion at the turn's first Boon / Hex (sampling later ones) or
  excluding Boons whose effect is irrelevant to the turn would cut those games by several times. `search/src/search.rs` `chance_value`.
* `sets/nocturne.rs` `turn_start_entries` is dispatched each turn for every game with Nocturne cards but returns on one flag test (fine); `nocturne_cleanup` loops over the players per Clean-up whenever `noc.flags != 0`: could be gated on "a Boon is held, a River's Gift is waiting or a Hound is set aside" (a three-field test), saving a few ns per turn.
