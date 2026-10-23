# Audit: Base Set, Intrigue, Seaside

Sources: `docs/*-plan.md`, `crates/engine/src/sets/{base,intrigue,seaside}.rs`, `crates/sim/src/defaults/{base,intrigue,seaside}.rs`, the generic
machinery in `effects.rs` / `engine.rs` / `state.rs` / `track.rs`, and every card's wiki page (current text, Official FAQ, Other rules clarifications).
Cards are 2nd edition (Base 25 + 7 basics, Intrigue 26, Seaside 27 = 85 cards checked).

## Bugs fixed

| Bug | Rule | Tests |
| --- | --- | --- |
| Shanty Town, Ironworks, Courtier and Conspirator read a card's printed types, not the types it has now. An Estate under Inheritance is an Action card (and so Shanty Town did draw with it in hand, Ironworks gave no +1 Action for a gained Estate, Courtier counted one type, Conspirator did not count the Estate played). Treasures under Enlightenment / Capitalism now count as Action / Treasure too. New `GameState::acts_as_action` and `types_now` in `sets/intrigue.rs`. | Ironworks FAQ: "If you gain an Estate after buying Inheritance, you get +1 Action and +1 Card"; Courtier FAQ: "Inheritance, Charlatan, Capitalism, Enlightenment can change the number of types a card has, and Courtier takes that into account"; Shanty Town FAQ: Action cards in hand | `audit_shanty_town_sees_an_inherited_estate_as_an_action`, `audit_ironworks_gaining_an_inherited_estate_gives_action_and_card`, `audit_courtier_counts_the_action_type_of_an_inherited_estate`, `audit_conspirator_counts_the_estate_played_with_inheritance` (all four fail without the fix) |
| Blockade did not set aside a card that was gained somewhere other than the discard pile (a Ghost Town is gained to the hand, Progress puts every gain onto the deck): the card stayed there and no Duration was scheduled. `Ledger::landed` (scratch, not hashed) records where the last gain landed; `ScheduleDuration` reads it. | Blockade FAQ: "The card you gain is immediately set aside, and doesn't visit your discard pile. So if you gain a Nomad Camp or Ghost Town, their abilities won't trigger" | `ruling_blockade_sets_the_gained_card_aside_without_visiting_the_discard_pile` (fails without the fix) |

## Open issues

None that need an ignored test. Limitations (not rule bugs, listed for completeness):

* Secret Passage offers the positions inside the known part of the deck and the bottom, not a position inside the unknown part (the deck is an unordered
  multiset there). Strategically the two outcomes that matter (top, bottom) are covered.
* Wishing Well may only name a card that is in the deck (or in the discard pile when the deck is empty). Naming any other card can never match, so this is
  equivalent in outcome.
* Corsair: treasures are auto-played in id order, so a victim holding Silver and Gold always loses the Silver (the better outcome for them anyway).

## Policy notes (bot defaults)

All defaults pick legal options (checked by reading each `SetDefaults` method, and by the existing `smoke` / `shipped_strategies` runs).

* Pawn never takes +Buy by default (wiki, Pawn: "It offers +Buy, and is therefore sometimes essential as the best or only available source of this"); there is
  no deterministic claim to test, a strategy file can state the rule.
* Courtier: coins first, then Gold, then +Buy, then +Action (wiki, Courtier: "a non-terminal Gold gainer" / "adjustable amount of +Buy"); fine as a fast default,
  `mode_by_value` overrides it when the strategy lists the cards.
* Nobles: `audit_nobles_with_a_terminal_draw_card_in_hand_takes_the_actions` (bot test) shows the wiki advice ("play the Nobles for +Actions even though you may
  have acquired it primarily for supplementary draw") is followed.

## Speed opportunities (not implemented)

* `sets/base.rs` Poacher: `g.empty_piles()` scans the whole supply per play; a counter kept by pile-emptying code would make it free (only Poacher games).
* `engine.rs` `finish_gain`: the one added byte store (`led.landed`) is the only cost of the Blockade fix; it could be gated on `source == BLOCKADE`.
* `sets/seaside.rs` `gain_triggers`: Monkey and Blockade loop over `pending_durations` per gain, gated by `in_supply`; a per-game "a Monkey/Blockade is pending" flag
  would skip the loop in Seaside games between plays (small).
* `effects.rs` `reaction_window_open` already exits early for kingdoms with no reaction; `push_reaction_window` rebuilds the same list per attack (only with a reaction in the game).

## Cards checked

### Base

* Copper, Silver, Gold, Estate, Duchy, Province, Curse: OK (Curse is a Treasure worth $1 with Charlatan; Silver play handles Merchant and Corsair).
* Cellar: OK. Chapel: OK. Moat: OK (reaction window at play time, before the attacker chooses). Harbinger: OK. Merchant: OK. Vassal: OK (Hound / play exclusive).
* Village: OK. Workshop: OK. Bureaucrat: OK. Gardens: OK (all zones, mats). Militia: OK (`down_to_target` computed after reactions). Moneylender: OK.
* Poacher: OK. Remodel: OK. Smithy: OK. Throne Room: OK. Bandit: OK. Council Room: OK. Festival: OK. Laboratory: OK. Library: OK (-1 Card token, set-aside not reshuffled).
* Market: OK. Mine: OK. Sentry: OK. Witch: OK. Artisan: OK.
* `choice_free` / `has_choice` / `order_sensitive` marks: all consistent with CLAUDE.md (Moat, Merchant, Village, Militia, Smithy, Council Room, Festival, Laboratory, Market, Witch choice-free).

### Intrigue

* Courtyard: OK. Lurker: OK. Pawn: OK. Masquerade: OK. Shanty Town: fixed (Inheritance / Enlightenment). Steward: OK. Swindler: OK. Wishing Well: OK (limitation above).
* Baron: OK. Bridge: OK. Conspirator: fixed (Action plays under Inheritance / Enlightenment). Diplomat: OK. Ironworks: fixed. Mill: OK. Mining Village: OK.
* Secret Passage: OK (limitation above). Courtier: fixed. Duke: OK. Minion: OK. Patrol: OK. Replace: OK. Torturer: OK. Trading Post: OK. Upgrade: OK. Harem: OK. Nobles: OK.

### Seaside

* Haven: OK. Lighthouse: OK. Native Village: OK. Astrolabe: OK. Fishing Village: OK. Lookout: OK. Monkey: OK. Sea Chart: OK. Smugglers: OK. Warehouse: OK.
* Blockade: fixed (landing zone). Caravan: OK. Cutpurse: OK. Island: OK. Salvager: OK. Sailor: OK. Tide Pools: OK. Treasure Map: OK.
* Bazaar: OK. Corsair: OK. Merchant Ship: OK. Outpost: OK. Pirate: OK. Sea Witch: OK. Tactician: OK. Treasury: OK. Wharf: OK.

## Ruling coverage

"existing" tests are in `crates/engine/tests/{base,intrigue,seaside}_cards.rs` unless noted; new tests are the `ruling_*` / `audit_*` ones.

### Base

| Card | Ruling | Test |
| --- | --- | --- |
| Cellar | discard all at once, then draw as many; reshuffle includes the discards | `cellar_discard_and_get_it_straight_back_via_reshuffle`, `cellar_draws_after_reshuffle` |
| Cellar | cannot discard itself | `ruling_cellar_and_chapel_cannot_pick_themselves` |
| Cellar | number discarded public / only top card seen | not testable: table-talk information, not a game state |
| Chapel | cannot trash itself | `ruling_cellar_and_chapel_cannot_pick_themselves` |
| Chapel | zero cards is allowed | `chapel_may_trash_nothing` |
| Chapel | trash all first, then order the when-trashed effects | `ruling_chapel_trashes_all_chosen_cards_then_resolves_their_trash_effects` (and dark_ages_cards `count_trashing_the_hand_trashes_every_card_first_...`) |
| Moat | only Attacks; list of Base Attacks; reveal before the Attack does anything | `moat_blocks_militia`, `moat_blocks_witch`, `moat_blocks_bureaucrat`, `moat_blocks_bandit`, `ruling_masquerade_is_not_an_attack_and_passing_is_not_gaining` |
| Moat | stays in hand | `moat_revealed_so_the_attack_is_blocked_and_the_reveal_is_logged` |
| Moat | does not stop what the Attack does for its player | `ruling_militia_and_moat_the_attacker_keeps_the_coin_when_the_victim_reveals_moat`, `ruling_moat_revealed_to_bureaucrat_still_lets_the_attacker_gain_the_silver` |
| Moat | +2 Cards only when played | `moat_played_as_action_draws_two` |
| Moat | may reveal for several Attacks | `moat_is_chosen_again_for_every_attack` |
| Moat | reveal comes before the attacker's choice (Minion) | `moat_declined_against_minion_leaves_the_player_with_a_new_hand_of_four` (intrigue), `a_moat_drawn_by_a_diplomat_reaction_can_still_be_revealed` |
| Harbinger | draw first, then look; putting a card is optional | `harbinger_topdecks_from_discard` |
| Merchant | +$1 for the first Silver only; each Merchant adds $1 | `merchant_bonus_only_on_first_silver`, `two_merchants_one_silver_gives_plus_two` |
| Merchant | Throne Room'd Merchant gives +$1 twice | `throne_room_merchant_gives_plus_two`, `throne_room_merchant_bonus_counts_both_plays_but_only_the_first_silver` |
| Merchant | played after the first Silver: nothing | `ruling_merchant_played_after_the_first_silver_gives_nothing` |
| Merchant | Sauna-gained Merchant played by Innovation after a Silver | not testable in this set: needs Innovation + Sauna set-up (covered by the generic `silvers_played` counter used by the test above) |
| Vassal | may play the Action, no Action used | `vassal_plays_the_discarded_action_without_spending_an_action`, `vassal_reveals_a_non_action_and_just_discards_it` |
| Vassal | Faithful Hound: reaction or play, not both | `ruling_vassal_discarding_a_faithful_hound_uses_the_hound_or_the_play_not_both` |
| Vassal | Weaver plays itself or Vassal's play, once | not testable: Weaver (Allies) set-up of two reaction sources; Hound test covers the shared mechanism |
| Village | +1 Card +2 Actions | `village_draws_and_gives_two_actions` |
| Workshop | from Supply to discard; cannot use $ to gain more; current costs | `workshop_gains_a_card_costing_up_to_four`, `ruling_workshop_and_artisan_use_the_costs_cards_have_now` |
| Bureaucrat | empty deck: the card becomes the only card | `ruling_bureaucrat_victim_with_an_empty_deck_gets_the_victory_card_as_the_only_deck_card` |
| Bureaucrat | no Victory card: reveal hand | `bureaucrat_victim_with_no_victory_card_is_untouched` |
| Gardens | counts all cards incl. in play, set aside, mats | `gardens_finds_cards_everywhere`, `island_and_native_village_mats_both_count_for_vp_and_all_cards` (seaside) |
| Gardens | 37 cards = 3 VP; pile 8 / 12 | `gardens_scores_floor_of_total_cards_over_ten`, `ruling_victory_kingdom_piles_hold_8_cards_with_two_players_and_12_with_more` |
| Militia | 3 or fewer cards: no discard | `militia_hand_of_three_or_fewer_is_untouched` |
| Militia | cards discarded privately | not testable: table-talk information |
| Moneylender | +$3 only if a Copper was trashed | `moneylender_decline_leaves_copper_and_no_bonus`, `moneylender_no_copper_offers_no_decision` |
| Poacher | draw before discarding; counts every empty Supply pile; Spoils etc. do not count; discard all if short | `poacher_with_zero_empty_piles_has_no_discard`, `poacher_with_one_empty_pile_discards_one`, `poacher_with_two_empty_piles_discards_two`, `poacher_discards_whole_hand_when_short_of_empty_piles`, `ruling_poacher_counts_every_empty_supply_pile_including_curses_and_itself_but_not_side_piles` |
| Remodel | cannot trash itself; no card, no gain; gain from Supply to discard; may be a copy; no $ | `remodel_nothing_in_hand_is_a_no_op`, `remodel_trash_is_mandatory_when_something_is_available`, `remodel_upgrade_cap_is_trashed_cost_plus_two`, `ruling_remodel_may_gain_a_copy_of_the_trashed_card_but_not_use_coins_to_gain_more` |
| Smithy | draws 3 | `smithy_draws_three` |
| Throne Room | optional; resolve fully; no action use; TR on TR; different results per play | `throne_room_is_optional_in_2e`, `throne_room_village`, `throne_room_on_throne_room_chains_two_more_actions`, `throne_room_on_throne_room_with_a_single_target_only_doubles_once`, `throne_room_with_mode_cards` (intrigue) |
| Throne Room | Horse leaves play, still played twice | `ruling_throne_room_plays_a_card_that_leaves_play_a_second_time` |
| Throne Room | Mining Village / Madman contingent effects not repeated | `mining_village_with_throne_room_trashes_only_once`, `throne_roomed_mining_village_cannot_trash_another_mining_village_in_play` (intrigue) |
| Throne Room | Duration: Throne Room stays in play | `throne_room_wharf_doubles_now_and_next_turn_and_both_stay_until_then` (seaside) |
| Bandit | Gold first; reveal 2, trash a non-Copper Treasure, discard rest | `bandit_silver_and_gold_is_a_choice`, `bandit_copper_and_gold_only_gold_is_trashable`, `bandit_two_coppers_neither_trashed`, `ruling_bandit_gains_the_gold_before_the_attack` |
| Council Room | others draw whether they want to or not | `ruling_council_room_other_players_draw_whether_they_want_to_or_not`, `council_room_draws_four_and_each_other_player_draws_one` |
| Festival, Laboratory, Market | bonuses | `festival_gives_actions_buy_and_coin`, `laboratory_draws_two_keeps_action`, `market_gives_everything` |
| Library | see cards one at a time, discard set aside after; shuffle does not include them; 7 in hand: no draw; -1 Card token | `library_skips_actions_and_discards_them_at_the_end`, `library_reshuffle_mid_draw_does_not_shuffle_in_the_set_aside_card`, `ruling_library_does_not_shuffle_in_the_set_aside_cards_and_discards_them_afterwards`, `ruling_library_with_the_minus_one_card_token_removes_it_and_draws_to_seven` |
| Library | set-aside Action with a discard Reaction (Village Green, Trail) reacts after finishing | not tested: needs the Menagerie / Hinterlands reaction set-up; the shared `discard_hook_counts` path is tested in `hinterlands_cards.rs` (Tunnel / Trail) |
| Mine | optional trash; gain to hand; same Treasure allowed; no Treasure no gain | `mine_is_optional_and_gains_to_hand_in_2e`, `mine_gold_to_gold`, `mine_no_treasures_in_hand_is_a_no_op` |
| Sentry | trash, then discard, then put back; any combination | `sentry_can_trash_one_and_discard_the_other`, `sentry_can_keep_both_and_choose_the_order`, `ruling_sentry_resolves_trash_then_discard_then_the_put_back` |
| Sentry | a card put on the deck mid-effect (Tunnel + Progress): returned cards go on top of it | not tested: needs a bought Progress Event and Tunnel; the stop-moving machinery is tested in `cross_set.rs` |
| Witch | Curses in turn order; still +2 Cards when gone | `witch_curses_run_out_leftmost_players_get_priority`, `witch_no_curses_left` |
| Artisan | gain to hand, then topdeck any hand card incl. the gained one; current costs | `artisan_gains_to_hand_then_topdecks_a_card`, `ruling_artisan_may_put_the_card_it_just_gained_onto_the_deck`, `ruling_workshop_and_artisan_use_the_costs_cards_have_now` |
| Curse | Treasure with Charlatan; pile 10 per extra player | `curse_basics_and_vp`, `with_charlatan_mine_and_bandit_treat_a_curse_as_a_treasure`, `ironworks_gaining_a_curse_gives_a_coin_when_charlatan_makes_it_a_treasure` (cross_set); pile size per player: not tested |

### Intrigue

| Card | Ruling | Test |
| --- | --- | --- |
| Courtyard | the card put back need not be one drawn | `ruling_courtyard_may_put_back_a_card_that_was_not_drawn` |
| Lurker | trashed / gained card may have other types (Nobles) | `ruling_lurker_can_trash_and_gain_a_dual_type_action` |
| Lurker | gain goes to the discard pile | `lurker_trashes_from_supply_or_gains_from_trash` |
| Lurker | trashing from the Supply triggers when-trashed (Fortress) | `ruling_lurker_trashing_a_fortress_from_the_supply_triggers_its_when_trashed_effect` |
| Lurker | no Market Square reaction for a Supply card | not tested: the Supply trash path passes `own = false` (`trash_card(.., false, ..)`), asserted indirectly by the Fortress test; Market Square set-up is in `dark_ages_cards.rs` |
| Lurker | an option with nothing to do may still be chosen | `lurker_trashes_from_supply_or_gains_from_trash` (skipped silently cases) |
| Lurker | only the top card of a split pile / Knights | not tested: needs Knights / split-pile set-up; `select_ok` uses `pile_open` for Supply picks (`empires_cards.rs` split pile tests cover `pile_open`) |
| Pawn | choose both first; different options; per play choices | `pawn_two_different_bonuses`, `throne_room_with_mode_cards` |
| Masquerade | draw first; simultaneous pass; empty hands skipped; alone: pass to self | `masquerade_base_case_simultaneous_pass`, `masquerade_empty_handed_player_neither_passes_nor_receives`, `masquerade_self_pass_when_no_other_player_has_cards` |
| Masquerade | not an Attack; passing is not gaining | `ruling_masquerade_is_not_an_attack_and_passing_is_not_gaining` |
| Shanty Town | Nobles counts as Action; reveal again when Throne Room'd | `ruling_shanty_town_counts_a_dual_type_card_as_an_action`, `shanty_town_with_throne_room_and_two_shanty_towns` |
| Steward | choose then do; 1 card: trash it; 2+: exactly 2 | `steward_choose_one_of_three`, `ruling_steward_trash_option_with_one_or_two_plus_cards` |
| Swindler | turn order; same cost; may give a copy back; none at that cost: nothing | `swindler_attacker_picks_the_victims_gain_at_the_trashed_cost`, `swindler_must_swindle_estate_for_estate`, `swindler_with_no_card_at_that_cost_gains_nothing` |
| Swindler | when-trashed effects before the attacker's choice | `ruling_swindler_victims_when_trashed_effect_comes_before_the_attackers_choice` |
| Wishing Well | name a card, not a type; hit goes to hand | `wishing_well_names_hit_and_miss` |
| Baron | not forced to discard; otherwise gain an Estate | `baron_discards_estate_or_gains_one` |
| Bridge | all cards everywhere, cumulative, not below $0 | `bridge_gives_a_buy_a_coin_and_a_discount`, `two_throne_rooms_and_two_bridges_buy_two_provinces_and_three_silvers`, `bridge_reduces_gain_costs_too_and_resets_next_turn` |
| Bridge | Procession on Bridge still reduces | not testable: needs Procession on a Bridge in Dark Ages; the effect is a counter on the turn (`cost_reduction`) independent of the card |
| Conspirator | counts Actions played, Throne Room twice | `conspirator_counts_action_cards_played_this_turn`, `conspirator_with_throne_room_counts_both_plays` |
| Conspirator | counts Actions that left play; not Durations from before | `ruling_conspirator_counts_actions_that_left_play_but_not_durations_from_before` |
| Conspirator | Command variant: two Actions | `audit_conspirator_counts_the_estate_played_with_inheritance` (Inheritance's Estate, a Command Action) |
| Diplomat | count after drawing; 5 cards exactly | `diplomat_actions_threshold_exactly_five_vs_six` |
| Diplomat | reveal at 5+ cards; again while 5+; Moat before / after; not simultaneously | `diplomat_reacts_to_militia_before_the_forced_discard`, `a_diplomat_can_be_revealed_again_while_the_hand_still_has_five_cards`, `a_moat_drawn_by_a_diplomat_reaction_can_still_be_revealed`, `diplomat_reaction_can_be_declined_or_skipped_below_five_cards` |
| Diplomat | can react while immune | `ruling_diplomat_can_be_revealed_while_protected_by_a_lighthouse` |
| Ironworks | bonuses by type; two types both; none for Curse; reaction before bonus; bonus even if gain trashed; Inheritance Estate | `ironworks_bonus_matches_the_gained_card_type`, `ruling_ironworks_bonus_comes_even_if_the_gained_card_was_trashed_and_curse_gives_none`, `audit_ironworks_gaining_an_inherited_estate_gives_action_and_card` |
| Ironworks | no bonus during Possession | not testable: Possession is not implemented in this simulator |
| Mill | discard 2 only if 2; one card: allowed, no $ ; piles 8 / 12 | `mill_basics_and_discard_bonus`, `ruling_intrigue_victory_kingdom_piles_hold_8_cards_with_two_players_and_12_with_more` |
| Mining Village | trash only right then; not twice with Throne Room; no $ when trashed otherwise | `mining_village_may_trash_itself_for_two_coins`, `mining_village_with_throne_room_trashes_only_once`, `ruling_mining_village_is_not_worth_coins_when_trashed_in_another_way` |
| Secret Passage | draw then place; anywhere; empty deck: only card; location public | `secret_passage_top_and_middle_then_verifies_order_by_drawing`, `secret_passage_when_the_deck_is_totally_empty_has_only_one_position`, `secret_passage_to_bottom_and_deck_bottom_text_round_trip` |
| Courtier | count types; choices differ; may reveal different card per play | `courtier_reveals_and_chooses_per_type`, `throne_room_with_mode_cards` |
| Courtier | Charlatan / Inheritance change the types | `courtier_reveals_and_chooses_per_type` (Curse with Charlatan: `ironworks_gaining_a_curse_gives_a_coin_when_charlatan_makes_it_a_treasure` in cross_set), `audit_courtier_counts_the_action_type_of_an_inherited_estate` |
| Courtier | 5 types: only 4 bonuses (Capitalism); Elder: one extra choice | not tested: needs a Capitalism project owner / Allies Elder set-up; `elder_mode_frame` is tested in `allies_cards.rs` |
| Courtier | no card in hand: no bonus | `courtier_reveals_and_chooses_per_type` (last case) |
| Duke | VP per Duchy; piles 8 / 12 | `duke_is_worth_one_per_duchy`, `ruling_intrigue_victory_kingdom_piles_hold_8_cards_with_two_players_and_12_with_more` |
| Minion | Moat / Diplomat before the choice; Moat: no discard, no draw | `moat_declined_against_minion_leaves_the_player_with_a_new_hand_of_four`, `minion_coins_or_new_hands` |
| Minion | empty hand still draws 4 | `ruling_minion_redraw_with_no_cards_left_in_hand_still_draws_four` |
| Minion | Horse Traders blocks it | not testable: Horse Traders is not implemented (Hinterlands is partial) |
| Patrol | draw 3 first, reveal 4, take all Victory and Curses, rest back in any order | `patrol_moves_victory_and_curse_to_hand_reorders_the_rest`, `patrol_with_throne_room` |
| Replace | like Remodel; Action / Treasure to deck; Victory: Curses; both for dual types | `replace_action_or_treasure_goes_onto_the_deck`, `replace_victory_gain_curses_other_players_unless_moat`, `replace_dual_type_harem_goes_on_the_deck_and_curses` |
| Replace | not subject to no-visiting; moved away first: not topdecked | `replace_puts_the_gained_card_on_the_deck_not_an_older_copy_in_the_discard_pile`, prosperity `a_second_watchtower_cannot_trash_the_gained_card_the_first_put_on_the_deck` |
| Replace | Innovation / Gatekeeper moved it first | not tested: needs Renaissance Innovation / Menagerie Exile set-up; the token machinery is covered by `cross_set.rs` |
| Torturer | turn order; may pick an option they cannot do; Curse to hand | `torturer_victim_discards_or_gains_a_curse`, `ruling_torturer_victim_may_pick_an_option_they_cannot_do` |
| Trading Post | one card: trash it only; 2+: exactly 2 | `trading_post_gains_a_silver_only_for_exactly_two_trashed` |
| Upgrade | exact +$1; none at that cost: nothing; trash effects before the gain | `upgrade_trashes_and_gains_exactly_one_more`, `ruling_upgrade_resolves_the_trashed_cards_effects_before_the_gain` |
| Harem | Treasure-Victory | `harem_basics_vp_and_worth` |
| Nobles | different choices per play | `nobles_choose_cards_or_actions`, `throne_room_with_mode_cards` |

### Seaside

| Card | Ruling | Test |
| --- | --- | --- |
| Haven | draw first; must set aside; no card: discarded same turn; returns at start of turn | `haven_sets_aside_a_card_and_schedules_its_return`, `haven_with_no_card_to_set_aside_is_not_a_duration`, `haven_next_turn_returns_the_set_aside_card_to_hand` |
| Haven | still in play at game end: counts for scoring | `ruling_haven_and_its_set_aside_card_count_for_scoring_while_in_play` |
| Lighthouse | protects while in play, even from later Attacks this turn; not from Duration Attacks played after | `lighthouse_blocks_attacks_while_in_play`, `a_lighthouse_in_play_when_corsair_is_played_protects_but_one_played_later_does_not` |
| Lighthouse | Reactions still allowed | `ruling_diplomat_can_be_revealed_while_protected_by_a_lighthouse` (intrigue) |
| Lighthouse | no protection during the next turn (Black Cat) | not tested: needs Menagerie Black Cat set-up; Lighthouse's `IMMUNE_IN_PLAY` check reads only `victims()` of an Attack |
| Native Village | either option with an empty deck / mat; cards return at game end | `native_village_set_aside_from_an_empty_deck_is_a_no_op`, `native_village_takes_an_empty_mat`, `island_and_native_village_mats_both_count_for_vp_and_all_cards` |
| Native Village | cannot look at the top before choosing | not testable: it is the UI, the engine decides before it moves the card |
| Astrolabe | now and next turn: $1 + Buy | `astrolabe_now_and_next_turn_give_a_coin_and_a_buy` |
| Fishing Village | +2 Actions +$1, then +1 Action +$1 | `fishing_village_now_and_next_turn` |
| Lookout | order; one card: trash; two: trash then discard; on-trash draws from below | `lookout_with_one_card_only_trashes_it`, `lookout_with_two_cards_trashes_one_and_discards_the_other`, `ruling_lookout_trash_effects_draw_from_below_the_looked_at_cards` |
| Lookout | a card put on the deck mid-effect (Tunnel + Progress) goes under the returned card | not tested: needs a bought Progress Event and Tunnel (see Sentry) |
| Monkey | counts gains on others' turns (Witch); ends when the next turn starts | `monkey_draws_from_an_attackers_gains_during_the_attackers_own_turn`, `monkey_stops_drawing_when_its_owners_next_turn_starts` |
| Monkey | gainer's on-gain effects before the draw (Skirmisher / Berserker order); a drawn Pirate / Fool's Gold reaction | not tested: needs Allies Skirmisher / Hinterlands Berserker interleavings; the draw frame is pushed below the gain's own reactions (`gain_triggers` order) |
| Sea Chart | includes itself and Durations from earlier turns | `sea_chart_reveals_and_draws_a_duplicate_of_itself`, `ruling_sea_chart_counts_a_duration_in_play_from_a_previous_turn` |
| Smugglers | up to $6 now; most recent turn; any gain not only buys | `smugglers_excludes_a_card_that_currently_costs_more_than_six`, `smugglers_reads_the_right_hand_players_actual_last_turn` |
| Smugglers | potion / debt cards never; not an Attack | `ruling_smugglers_never_offers_cards_with_potions_and_is_not_an_attack` |
| Smugglers | may choose a card whose pile is empty and gain nothing; Ruins / Knights top | not testable: the engine offers only cards that can be gained (same outcome, nothing is gained); Ruins / Knights need a Dark Ages kingdom |
| Warehouse | fewer than 3 draws still discards 3 | `warehouse_discards_everything_drawn_when_fewer_than_three` |
| Blockade | set aside, not discarded; returns next turn | `blockade_gains_a_card_sets_it_aside_and_schedules_its_return`, `blockade_next_turn_returns_the_gained_card_to_hand` |
| Blockade | no visit (Ghost Town) | `ruling_blockade_sets_the_gained_card_aside_without_visiting_the_discard_pile` |
| Blockade | Curse on gains on their own turn only; not the owner | `blockade_curses_another_player_who_gains_a_copy_on_their_own_turn`, `blockade_does_not_curse_its_own_owner`, `ruling_blockade_only_curses_a_gain_on_the_gainers_own_turn` |
| Blockade | Moat revealed at play protects; moved away (Watchtower) | `a_moat_revealed_when_blockade_is_played_protects_from_its_curse`, `blockade_sets_the_gained_card_aside_before_watchtower_and_trader_can_move_it` |
| Blockade | Rapid Expansion / Possession / Curse-Curse chain | not tested: Possession not implemented; Rapid Expansion (Rising Sun) and the Curse-on-Curse chain are the deliberate simplification documented in `gain_triggers` |
| Caravan | +1 Card +1 Action, next turn +1 Card | `caravan_now_gives_a_card_and_action_and_schedules_next_turn`, `caravan_next_turn_draws_one_card` |
| Cutpurse | discard a Copper or reveal hand | `cutpurse_makes_each_other_player_discard_a_copper`, `cutpurse_does_nothing_if_the_victim_has_no_copper_in_hand` |
| Island | has to set aside a card if it can; Throne Room sets aside two cards; not itself twice | `island_puts_a_hand_card_on_the_mat_too`, `island_through_throne_room_moves_only_hand_cards_the_second_time`, `throne_roomed_island_does_not_move_another_island_in_play` |
| Island | mat face up; piles 8 / 12 | `ruling_island_pile_holds_8_cards_with_two_players_and_12_with_more` (face-up mat: not testable, UI) |
| Island | Procession / Command variant | not testable here: Procession and the Command-card interactions are covered in `dark_ages_cards.rs` / `band_of_misfits.rs` |
| Salvager | $ per $1 of cost; nothing for P / Debt | `salvager_trashes_a_card_for_its_current_cost`, `ruling_salvager_gives_nothing_for_the_potion_in_a_cost` |
| Sailor | optional; cumulative; two Sailors cannot play the same card; gained other ways; Buy phase | `sailor_may_play_a_gained_duration_card_once_per_turn`, `two_separate_sailors_give_one_use_each`, `declining_the_offer_still_spends_that_sailors_use`, `a_throne_roomed_sailor_plays_two_gained_durations_not_three` |
| Sailor | Treasure-Durations; moved by a reaction | `ruling_sailor_may_play_a_gained_treasure_duration`, `sailor_cannot_play_a_gained_duration_that_watchtower_trashed_with_another_copy_in_the_discard_pile` |
| Sailor | Hero / Garrison / Guardian | not tested: Hero (Empires Traveller) exchange and Garrison are covered by their own sets' tests; no Sailor-specific code path |
| Tide Pools | discard 2 at start; 1 or 0 cards | `tide_pools_next_turn_forces_a_discard_of_two`, `ruling_tide_pools_discards_one_or_none_when_the_hand_is_short` |
| Tide Pools | order of start effects (4 Tide Pools then Wharf) | `start_order_defaults.rs` (sim) and the `StartOrder` decision in `tide_pools_next_turn_forces_a_discard_of_two` |
| Treasure Map | trash this and another; 4 Golds on deck; alone: only itself; Throne Room is not two Maps; both at once | `treasure_map_trashes_two_and_gains_four_golds`, `treasure_map_alone_just_trashes_itself`, `treasure_map_through_throne_room_with_zero_hand_copies_gains_nothing`, `throne_roomed_treasure_map_does_not_trash_another_treasure_map_in_play` |
| Treasure Map | Command variant cannot trash the Supply copy; Possession | `band_of_misfits.rs` plays Supply cards; Possession not implemented |
| Bazaar | +1 Card +2 Actions +$1 | `bazaar_card_two_actions_and_a_coin` |
| Corsair | trashed Silver / Gold still made its $; only one trashed under several Corsairs | `corsair_trashes_the_first_silver_or_gold_another_player_plays_each_turn`, `ruling_only_one_card_is_trashed_when_under_several_corsairs` |
| Merchant Ship | $2 now and next turn | `merchant_ship_now_and_next_turn_give_two_coins` |
| Outpost | 3 cards even when failing; one extra turn only; no 3rd turn; discarded at the next cleanup; no extra turn after the end; not in the tiebreak | `outpost_marks_extra_turn_and_draws_three_for_the_same_player`, `two_outposts_the_same_turn_grant_only_one_extra_turn`, `outpost_twice_in_a_row_does_not_grant_a_third_turn`, `a_failed_outpost_still_draws_three_and_is_discarded_at_the_next_turns_cleanup`, `outpost_grants_no_extra_turn_once_the_game_has_ended`, `an_outposts_extra_turn_does_not_count_towards_the_fewer_turns_tiebreak` |
| Outpost | Merchant Ship + Outpost; Throne Room stays | `ruling_outpost_and_merchant_ship_the_extra_turn_is_the_next_turn`, `throne_room_on_a_failed_outpost_stays_with_it_until_the_next_cleanup` |
| Outpost | Fleet / Lich / Voyage skipping | not tested here: Voyage is in `allies_cards.rs`; Fleet and Lich are not implemented |
| Pirate | play from hand when any player gains a Treasure; own next turn gains | `pirate_may_be_played_from_hand_when_any_player_gains_a_treasure`, `pirate_next_turn_gains_a_treasure_to_hand`, `each_pirate_in_hand_may_be_played_when_a_treasure_is_gained`, `players_react_with_pirates_in_turn_order_starting_with_the_turn_player` |
| Pirate | Capitalism | not tested: needs a Capitalism owner (`is_treasure` honours it in `gain_triggers`) |
| Sea Witch | +2 Cards, Curses; next turn +2 then discard 2 | `sea_witch_now_draws_two_and_curses_opponents`, `sea_witch_next_turn_draws_two_then_discards_two` |
| Tactician | no hand: discarded same turn; Throne Room discards once; stays with TR | `tactician_with_an_empty_hand_does_nothing_and_is_not_a_duration`, `tactician_through_throne_room_keeps_throne_room_when_one_play_activates`, `tactician_through_throne_room_with_an_empty_hand_keeps_neither` |
| Tactician | +1 Card token makes TR useful | not tested: needs the Adventures token set-up; `tactician_through_throne_room_*` cover the empty-hand rule |
| Treasury | only Victory cards gained in the Buy phase count; offered per copy | `treasury_cannot_go_back_if_a_victory_card_was_bought`, `treasury_can_go_back_after_a_victory_card_gained_in_the_action_phase`, `treasury_offered_once_per_copy_in_play` |
| Wharf | +2 Cards +1 Buy now and next turn | `wharf_now_draws_two_and_a_buy_and_schedules_next_turn`, `wharf_next_turn_draws_two_and_a_buy` |
