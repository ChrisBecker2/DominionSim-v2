# Plunder audit

Branch `worktree-agent-a0a4b4e3a64e1313d`. Sources: the wiki copy (each card's page, `Loot`, `Trait`, `Plunder (expansion)`), `docs/plunder-plan.md`,
`crates/engine/src/sets/plunder.rs`, `crates/sim/src/defaults/plunder.rs`. Tests are in `crates/engine/tests/plunder_cards.rs` (engine), `crates/sim/tests/plunder_defaults.rs`
(bots) and `crates/lab/tests/api.rs` (Lab).

## Bugs fixed (each has a test that fails on the old code)

| Rule clause | Test |
|---|---|
| Rush: "if you Rush twice you'll still only play the Action once": every Rush waits for the same gain (the old code used one Rush per Action, so the second Rush played the next Action) | `two_rushes_are_both_used_up_by_the_first_action_gained` |
| Cutthroat: "the next-time effect gets set up after the other players discard": a Tunnel discarded to the attack and its Gold woke the Cutthroat that made them discard (the wait entry is now added by a `PluAfter` frame under the victims' discards) | `a_gold_gained_by_a_tunnel_the_cutthroat_made_them_discard_does_not_wake_it` |
| Spell Scroll: "gain a cheaper card" compared with the printed $7, not its cost now (Bridge) | `spell_scroll_gains_a_card_cheaper_than_its_cost_under_a_reduction` |
| Search: "trashing a card empties a Supply pile (Lurker)" and an Exile from the Supply also wake it (`effects.rs`: two gated calls to `plu_check_empty`) | `search_fires_when_a_trash_from_the_supply_empties_a_pile`, `search_fires_when_a_card_exiled_from_the_supply_empties_a_pile` |
| Siren: its trash-unless-an-Action is resolved after the other gain effects, so Insignia (a "when you gain" effect) can move it first: the stop-moving rule | `a_siren_gained_under_insignia_can_be_put_on_the_deck_before_it_is_trashed` |
| Siren gained with Spell Scroll "trashes itself and the Scroll fails to play it": the play of a gained card (Spell Scroll, Invasion's Loot) waits *under* the gain's own triggers (`push_under`) | `a_siren_gained_with_a_spell_scroll_trashes_itself_and_the_scroll_cannot_play_it`, `a_loot_gained_by_invasion_that_is_moved_by_a_gain_effect_is_not_played` |
| Quartermaster: "if you gain a Siren with this it will still trash itself": the card is set aside after the gain's triggers (`T_QM_MOVE`) | `a_siren_gained_with_a_quartermaster_still_trashes_itself` |
| Throne Room / King's Court on a "next time" Duration that fires in the turn it was played: "discard Throne Room that turn" (it stayed in play until the next turn: `duration_held.remove_one`) | `a_throned_search_is_trashed_once_and_gains_two_loot` |
| Harbor Village only counts the +$ the Action itself gave, not a Training token's | `harbor_village_does_not_count_the_coin_of_a_token` |

Shared-file changes: `effects.rs` (the `plu_gained` call passes `base`; two gated `plu_check_empty` calls for a Supply trash and an Exile; the token-coin hook after
`play_tokens`), `state.rs` (`DurationHeld::remove_one`). All gated by `plu.live`, so non-Plunder games are unchanged.

## Open issues (ignored tests)

| Issue | Test |
|---|---|
| A Mapmaker taken by a Mapmaker that was played in response to a gain is not offered (offers are made per copy in hand when the card is gained). FAQ: "you can immediately play another Mapmaker afterwards - even one you just got" | `a_mapmaker_taken_by_a_mapmaker_played_in_response_can_be_played_in_response_too` |
| Reckless Attack: the reaction window opens again for the second iteration (one Moat / Shield reveal should protect against both) | `a_reckless_attack_is_blocked_by_one_reveal_for_both_iterations` |
| Avoid: the cards left out go to the discard pile and are shuffled again for the rest of the same draw (FAQ: "does not trigger another shuffle, you just draw what you can") | `avoid_leaving_too_few_cards_does_not_shuffle_again` |
| Taskmaster: a $5 card whose gain *causes* the Taskmaster to be gained and played (Haggler then Rush / Innovation) should count; the triggers of that gain were looked at before | `a_five_whose_gain_causes_the_taskmaster_to_be_played_lets_it_repeat` |
| The Lab's `/api/cards` lists neither the Traits nor the Ways | `cards_lists_the_plunder_traits` (`crates/lab/tests/api.rs`) |
| Policy: Cage should be played before the Coppers so it can set them aside | `cage_is_played_before_the_coppers_so_that_it_can_set_them_aside` |

Known limitations without a cheap test: Search keeps a count of empty piles (`plu.empty_seen`), so a pile refilled (Swap) and emptied again between two checks is
missed; Bury and the other Events are not offered when they could do nothing (an empty discard pile for Bury: the wiki allows buying it blind for the +1 Buy).

## Re-check of the approximations in `docs/plunder-plan.md` section 9

* Taskmaster "counts only gains after it was played": this is the ruling ("Taskmaster does not count cards gained before playing it"), not an approximation. Only the
  Haggler-style case above is open.
* Search and trash / Exile: fixed (Lurker, Exile from the Supply). Piles emptied by other means (Swap refills) are open.
* Inherited: the plan was stale. Split piles are handled (the first player to choose gets the top card; see `an_inherited_split_pile_gives_the_first_card_to_the_first_players`),
  and Shelters are offered with the Coppers.
* Traits in the Lab: still open (ignored Lab test).

## Policy notes (bots, `crates/sim/src/defaults/plunder.rs` and the generic policy)

* Cage: the bot plays every Treasure first and the Cage last, so only Victory cards and Curses are left to set aside. Wiki (Cage, Strategy): "Cage is initially a lot like
  a Chapel that can only be used once". Test (ignored): `cage_is_played_before_the_coppers_so_that_it_can_set_them_aside`.
* Grotto: sets aside the dead cards (Estate, Curse, Coppers) and keeps the good Treasures. Wiki (Grotto, Strategy): "In most circumstances, it is best to set aside your entire
  hand ... a 9-card following turn". Current behaviour is documented by `grotto_sets_the_dead_cards_aside_and_keeps_the_good_treasures`. No change: the advice is about the
  early game and the bot cannot tell it from a late Province turn.
* Inherited replaces an Estate (`inherited_replaces_a_starting_estate`); Shaman and Maroon trash the Curse first (`shaman_and_maroon_trash_the_curse`).
* Pilgrim puts a Copper on the deck (cheapest card that is not junk): fine.
* The bots never choose the Fated bottom option.

## Speed opportunities (not implemented; results must stay identical)

* `plunder.rs` `plu_refresh` (about line 892) walks every player on every call (the flags test returns early only for games with no Plunder card). Called from play of a dozen
  cards; each call is about 20 ns. A per-player summary byte kept up to date by `plu_wait_add` / `plu_wait_take` would remove the loop.
* `plu_gain_triggers`: in a Plunder game every gain pays the `has_trait` calls for each Trait present (`pile_of` lookup). A 128-bit mask of the card ids on Traited piles
  built at set-up would make it one test. Estimated 1-2% of gain-heavy fast bots in Plunder games only.
* `event_effect` (Maelstrom: `Vec<u8>`) and `plu_cleanup_discards` (`Vec<(CardId, u8)>`, only with Tireless / Reckless) allocate on the heap in the game loop; a fixed array
  would do. Rare paths.
* `turn_start_entries` pushes a Shaman entry for every player's turn in a game with a Shaman; needed by the rules, but it could skip the push when the trash holds nothing costing
  $6 or less (`trash.is_empty()` is a single test).
* `draw_loot` sums the 15 Loot lanes for every Loot gained; negligible.

## Cards checked (one line each)

OK means the implementation matches the current text and the wiki rulings after the fixes above.

Kingdom cards (40): Cage OK. Grotto OK. Jewelled Egg OK. Search OK (after the Lurker / Exile fix; Swap refill open). Shaman OK. Secluded Shrine OK. Siren OK (after the
ordering fix). Stowaway OK. Taskmaster OK (Haggler case open). Abundance OK. Cabin Boy OK. Crucible OK. Flagship OK. Fortune Hunter OK. Gondola OK. Harbor Village OK (after
the token fix). Landing Party OK. Mapmaker OK (re-offer open). Maroon OK. Rope OK. Swamp Shacks OK. Tools OK. Buried Treasure OK. Crew OK. Cutthroat OK (after the Tunnel
fix). Enlarge OK. Figurine OK. First Mate OK. Frigate OK. Longship OK. Mining Road OK. Pendant OK. Pickaxe OK. Pilgrim OK. Quartermaster OK (after the Siren fix). Silver Mine
OK. Trickster OK. Wealthy Village OK. Sack of Loot OK. King's Cache OK.

Loot (15): Amphora OK. Doubloons OK. Endless Chalice OK. Figurehead OK. Hammer OK. Insignia OK. Jewels OK. Orb OK. Prize Goat OK. Puzzle Box OK. Sextant OK. Shield OK. Spell
Scroll OK (after the cost fix). Staff OK. Sword OK. The Loot pile (30 cards, outside the Supply, revealed when gained, shuffled only in games with a Loot gainer) OK.

Events (15): Bury OK (offered with an empty discard pile too since 2026-10-28: its +1 Buy still counts). Avoid OK (too-few-cards case open). Deliver OK. Peril OK. Rush OK (after the fix). Foray OK. Launch OK. Mirror
OK. Prepare OK. Scrounge OK. Journey OK. Maelstrom OK. Looting OK. Invasion OK. Prosper OK.

Traits (15): Cheap OK. Cursed OK. Fated OK. Fawning OK. Friendly OK. Hasty OK. Inherited OK. Inspiring OK. Nearby OK. Patient OK. Pious OK. Reckless OK (reveal window open).
Rich OK. Shy OK. Tireless OK.

`choice_free` / `has_choice` / `order_sensitive` marks: checked against the rule in `CLAUDE.md`; Swamp Shacks, Harbor Village, Taskmaster, Mining Road, Landing Party, Flagship
and Cabin Boy are `has_choice` although they look choice-free (they read or arm state that a later play uses): conservative and correct. No mark changed.

Existing tests whose name promised more than they test: `rush_plays_the_next_action_gained_this_turn` ("only once" was about the next Action, not about two Rushes: now
`two_rushes_...`); `mirror_gains_copies_of_the_next_action_gained` OK.

## Ruling coverage

Test names are in `crates/engine/tests/plunder_cards.rs` unless said otherwise. "not testable" lists the reason.

| Card | Ruling | Test |
|---|---|---|
| Cage | cards go to the hand after the regular hand is drawn | `cage_sets_cards_aside_and_returns_them_at_the_end_of_the_turn_when_a_victory_card_is_gained` |
| Cage | sets aside nothing: still waits in play | `a_cage_with_nothing_set_aside_still_waits_and_stays_in_play_over_turns` |
| Cage | played because of a Victory gain (Haggler, Mining Road) does not trigger on that gain | `a_cage_gained_by_haggler_and_played_by_mining_road_waits_for_the_next_victory_card` |
| Grotto | discard the set-aside cards then draw as many | `grotto_sets_cards_aside_then_discards_and_draws_that_many_next_turn` |
| Grotto | set aside 0: does not stay in play | `grotto_with_nothing_set_aside_is_just_discarded` |
| Jewelled Egg | the player trashing it gets the Loot whoever caused it | `jewelled_egg_gives_the_loot_to_the_player_who_trashes_it_whoever_caused_it` |
| Search | Throne Room: trashed once, two Loot, Throne Room discarded that turn | `a_throned_search_is_trashed_once_and_gains_two_loot` |
| Search | players trash and gain in turn order | `search_waits_through_other_gains_and_fires_for_every_player_who_played_one` |
| Search | a pile already empty when played does not count; another pile does | `search_ignores_piles_already_empty_when_played_and_fires_for_the_next_one` |
| Search | refilled (Swap) then emptied again triggers again | not testable cheaply: needs Swap; known limitation (`empty_seen`), see above |
| Search | a non-Supply pile (Loot) emptying does not trigger | `search_is_woken_by_the_last_province_and_not_by_the_loot_pile_running_out` |
| Search | ending the game by the last Province triggers it (Loot gained first) | `search_is_woken_by_the_last_province_and_not_by_the_loot_pile_running_out` |
| Search | a trash that empties a pile (Lurker) | `search_fires_when_a_trash_from_the_supply_empties_a_pile` (and Exile: `search_fires_when_a_card_exiled_from_the_supply_empties_a_pile`) |
| Search | Invest in the last card: ordering of Invest's +2 Cards and the Search trash | not testable: players do not order simultaneous triggers across players |
| Search | played by the gain that emptied the pile (Gondola): does not trigger | `a_search_played_by_a_gondola_gained_from_the_last_pile_does_not_fire` |
| Shaman | mandatory gain at every start of turn, to the discard pile, triggers Cage | `shaman_gain_is_mandatory_and_can_trigger_a_cage`, `shaman_is_a_cantrip_that_may_trash_and_everyone_gains_from_the_trash_at_the_start_of_every_turn` |
| Shaman | no card to gain: nothing | `shaman_gains_nothing_with_an_empty_trash` |
| Shaman | applies even if no one has a Shaman | `shaman_gain_is_mandatory_and_can_trigger_a_cage` (nobody owns one) |
| Shaman | applies on the first turn (Necromancer) | not testable: the trash is empty on turn 1 except through Necromancer's own flow |
| Shaman | ordering with other start-of-turn abilities (Rope, Cabin Boy) | not testable beyond the generic start-of-turn ordering (`start_order_defaults.rs`) |
| Secluded Shrine | any player's turn | `secluded_shrine_fires_on_the_players_own_gain_on_another_turn_and_is_discarded_that_turn` |
| Secluded Shrine | triggers even if nothing is trashed; discarded that turn | `secluded_shrine_trashes_up_to_two_cards_when_a_treasure_is_gained_and_is_then_discarded` |
| Secluded Shrine | played by a gained Gondola: only the other Shrines fire | `a_secluded_shrine_played_by_a_gained_gondola_waits_for_the_next_treasure` |
| Siren | trashed unless an Action is trashed | `siren_gives_curses_draws_to_eight_next_turn_and_is_trashed_when_gained_unless_an_action_is_trashed` |
| Siren | moved by a gain effect (Insignia): not trashed | `a_siren_gained_under_insignia_can_be_put_on_the_deck_before_it_is_trashed` |
| Siren | gained straight onto the deck: still trashed | `a_siren_gained_straight_onto_the_deck_is_still_trashed` |
| Siren | Spell Scroll fails to play it | `a_siren_gained_with_a_spell_scroll_trashes_itself_and_the_scroll_cannot_play_it` |
| Siren | may decline the Action to trigger when-trash effects | `a_siren_may_be_trashed_for_its_when_trash_effects_by_declining_the_action` |
| Siren | Quartermaster gain still trashes | `a_siren_gained_with_a_quartermaster_still_trashes_itself` |
| Stowaway | played when you or another player gains a Duration | `stowaway_is_played_from_the_hand_when_anyone_gains_a_duration_and_draws_two_next_turn`, `stowaway_is_not_asked_for_a_gain_that_is_not_a_duration` |
| Stowaway | plays like Menagerie reactions | not testable here: shared reaction machinery (`reactions.rs`) |
| Taskmaster | repeats turn after turn while a $5 is gained | `taskmaster_repeats_next_turn_after_gaining_a_five_and_not_otherwise` |
| Taskmaster | the cost when gained counts (Destrier) | `taskmaster_looks_at_the_cost_a_card_had_when_it_was_gained` |
| Taskmaster | cards gained before playing it do not count | `taskmaster_does_not_count_a_five_gained_before_it_was_played` |
| Taskmaster | repeating is not playing (Ways, Conspirator) | not testable: the repeat is a start-of-turn bonus and never a play, by construction |
| Taskmaster | Importer after the repeat | not testable: Importer is an Ally effect in a separate set flow |
| Taskmaster | a $5 whose gain causes the Taskmaster to be played counts | `a_five_whose_gain_causes_the_taskmaster_to_be_played_lets_it_repeat` (ignored: open) |
| Abundance | any Action gain, +1 Buy +$3 | `abundance_gives_a_buy_and_three_coins_when_an_action_is_gained` |
| Abundance | on another player's turn nothing useful, but used up | `abundance_gained_action_on_another_players_turn_uses_it_up_for_nothing` |
| Abundance | played by the Action gain (Innovation): next Action | not testable cheaply: same mechanism as the Cage and Shrine tests above |
| Cabin Boy | trash it to gain a Duration, even another Cabin Boy | `a_cabin_boy_can_be_trashed_to_gain_another_cabin_boy`, `cabin_boy_draws_and_next_turn_gives_two_coins_or_turns_into_a_duration` |
| Crucible | $1 per $1 of cost | `crucible_trashes_a_card_for_coins_equal_to_its_cost` |
| Crucible | no $ for Potion or Debt | `crucible_gives_nothing_for_potions_and_debt` |
| Flagship | not optional; replays the next non-Command Action | `flagship_replays_the_next_non_command_action` |
| Flagship | replays a card that trashed itself | `flagship_replays_a_card_that_trashed_itself` |
| Flagship | Command cards are not replayed; two Flagships replay twice | `two_flagships_replay_a_card_twice_and_a_flagship_does_not_replay_a_command_card` |
| Flagship | replays even on another player's turn | not testable cheaply: needs a Flagship holder to play an Action in another turn (Stowaway flow) |
| Flagship | Band of Misfits / Necromancer | not testable cheaply: needs their choose-a-card flows; the replay is the generic `PlayEffects` frame |
| Fortune Hunter | the Treasure is fully resolved before the others go back (Figurine) | `fortune_hunter_resolves_the_treasure_before_the_other_cards_go_back` |
| Fortune Hunter | may play a Treasure from the top 3 | `fortune_hunter_may_play_a_treasure_from_the_top_three` |
| Gondola | $2 now or next turn; Action on gain | `gondola_gives_two_coins_now_or_next_turn_and_plays_an_action_when_gained` |
| Gondola | multiple plays (King's Cache): choose each time, both stay | `king_cache_on_a_gondola_chooses_each_time_and_stays_with_it` |
| Gondola | playing the Action uses no Action | `gondola_gives_two_coins_now_or_next_turn_and_plays_an_action_when_gained` |
| Gondola | the "next time" exception for the played card | `a_secluded_shrine_played_by_a_gained_gondola_waits_for_the_next_treasure` |
| Gondola | Treasures drawn after the Buy phase started cannot be played; Sailor | not testable here: generic Buy-phase rule / Seaside Sailor flow |
| Harbor Village | only the Action's own +$ counts (Training token) | `harbor_village_does_not_count_the_coin_of_a_token` |
| Harbor Village | +$ from a Way counts | `harbor_village_counts_the_coins_a_way_gave` |
| Harbor Village | Throne Room, then Militia: only the second play pays | `harbor_village_played_twice_by_throne_room_pays_only_for_the_second_play` |
| Harbor Village | the next Action only | `harbor_village_adds_a_coin_after_the_next_action_if_that_gave_coins` |
| Harbor Village | +$0, Coffers, Storyteller, Bridge Troll token, Steward / Royal Carriage, Enlightenment | not tested: they follow from "the Action's own +$ at least 1" and need the other sets' flows; delta-based check documented in `run_plu_after` |
| Landing Party | resolve the Treasure first (Figurine) | `landing_party_resolves_a_figurine_draw_before_going_onto_the_deck` |
| Landing Party | first card a Treasure: onto the deck | `landing_party_goes_onto_the_deck_when_the_first_card_of_a_turn_is_a_treasure` |
| Landing Party | first card an Action: stays | `landing_party_stays_when_the_first_card_played_is_an_action` |
| Landing Party | Duration start-of-turn effects do not count | `landing_party_goes_onto_the_deck_when_the_first_card_of_a_turn_is_a_treasure` (nothing played at the start of the turn) |
| Landing Party | start-of-turn plays (Piazza, Royal Galley) count; Spell Scroll plays one; Treasure on another turn | not tested: need Renaissance / Adventures / Barbarian flows; the hook is `plu_play_hooks` for every play |
| Mapmaker | fewer than four cards | `mapmaker_with_fewer_than_four_cards_looks_at_what_is_left` |
| Mapmaker | played when any player gains a Victory card | `mapmaker_takes_two_of_the_top_four_and_is_played_when_a_victory_card_is_gained` |
| Mapmaker | play another Mapmaker afterwards, even a just-taken one | `a_mapmaker_taken_by_a_mapmaker_played_in_response_can_be_played_in_response_too` (ignored: open) |
| Maroon | +2 Cards per type | `maroon_trashes_a_card_for_two_cards_per_type`, `maroon_counts_the_loot_type_and_ignores_traits` |
| Maroon | Traits are not types | `maroon_counts_the_loot_type_and_ignores_traits` |
| Maroon | Inheritance / Capitalism / Charlatan / Enlightenment add types | not tested: other sets' effects (the Capitalism / Enlightenment Treasure type is in `type_count`) |
| Rope | draw first, then trash | `rope_gives_a_coin_a_buy_and_next_turn_a_card_and_a_trash` |
| Swamp Shacks | counts itself, Durations, Treasures; not set-aside cards; rounds down | `swamp_shacks_draws_a_card_per_three_cards_in_play`, `swamp_shacks_counts_durations_in_play_but_not_set_aside_cards` |
| Tools | a copy of a card any player has in play, even Tools itself | `tools_gains_a_copy_of_a_card_anyone_has_in_play` |
| Tools | not a Loot | `tools_cannot_gain_a_loot_another_player_has_in_play` |
| Buried Treasure | gain: must play | `buried_treasure_is_played_when_gained_and_pays_next_turn` |
| Crew | topdeck is mandatory | `crew_draws_three_and_puts_itself_on_the_deck_next_turn` |
| Cutthroat | Loot gained triggers Cutthroats | `cutthroat_is_triggered_by_a_loot_gained` |
| Cutthroat | a Gold gain by anyone triggers every player's Cutthroat | `cutthroat_attacks_and_gives_a_loot_when_a_treasure_costing_five_is_gained_by_anyone` |
| Cutthroat | set up after the discards (Tunnel) | `a_gold_gained_by_a_tunnel_the_cutthroat_made_them_discard_does_not_wake_it` |
| Cutthroat | played by the Gold gain: next Treasure | not tested cheaply: same mechanism as the Cage / Shrine tests |
| Enlarge | mandatory trash at the start of the next turn | `enlarge_remodels_now_and_at_the_start_of_the_next_turn` |
| Figurine | +2 Cards, may discard an Action for +1 Buy +$1 | `figurine_draws_two_and_may_discard_an_action_for_a_buy_and_a_coin` |
| First Mate | draw to 6 even with no Action | `first_mate_with_no_action_just_draws_to_six` |
| First Mate | a drawn copy can be played too | `first_mate_plays_copies_of_an_action_and_draws_to_six` |
| First Mate | can play First Mates | not tested: the same loop frame with `Card(First Mate)`; the bookkeeping is `T_FIRST_MATE_LOOP` |
| Frigate | each Action the others play, they discard down to 4 afterwards, until your next turn | `frigate_makes_the_others_discard_down_to_four_after_each_action_until_its_next_turn` |
| Frigate | nobody affected: discarded at once | `a_frigate_nobody_is_hit_by_is_discarded_at_once` |
| Frigate | a reaction that plays itself (Guard Dog) is not followed by a discard | not tested: needs Guard Dog (Menagerie) |
| Longship | +2 Actions now, +2 Cards next turn | `longship_gives_two_actions_and_two_cards_next_turn` |
| Mining Road | optional; once per Road | `mining_road_may_play_a_gained_treasure_once_for_each_road` |
| Mining Road | two Roads cannot play the same Treasure twice | `two_mining_roads_cannot_play_the_same_treasure_twice` |
| Mining Road | works in the Action phase | `mining_road_may_play_a_gained_treasure_once_for_each_road` (the Gold is gained in the Action phase) |
| Mining Road | nested gains played in any order | not tested: the offers are independent YesNo frames |
| Pendant | counts itself and Duration Treasures | `pendant_and_wealthy_village_count_duration_treasures_and_loot_in_play`, `pendant_is_worth_a_coin_per_differently_named_treasure_in_play` |
| Pickaxe | mandatory trash; Loot to the hand for $3+ | `pickaxe_trashes_a_card_and_gains_a_loot_to_the_hand_for_three_or_more` |
| Pilgrim | the card put back need not be one drawn | `pilgrim_draws_four_and_puts_a_card_back_on_the_deck` |
| Quartermaster | stays in play for good; gain or take | `quartermaster_sets_gains_aside_and_takes_them_into_the_hand_on_later_turns` |
| Quartermaster | Throne Room: one set of cards, twice a turn | `quartermaster_throned_has_one_set_of_cards_and_adds_two_each_turn` |
| Quartermaster | cards set aside are still yours at the end | the conservation check of `random_plunder_games_conserve_cards_and_terminate` and `all_cards` |
| Quartermaster | Ghost Town gained is set aside, not put in the hand | not tested: needs Nocturne's Ghost Town; the gain moves nothing before the aside |
| Silver Mine | a Treasure costing less, to the hand | `silver_mine_gains_a_cheaper_treasure_to_the_hand` |
| Trickster | cumulative | `two_tricksters_keep_two_treasures` |
| Trickster | the Treasure comes after the new hand is drawn | `trickster_curses_and_keeps_a_treasure_for_the_next_hand` |
| Wealthy Village | Duration Treasures and Loot count | `pendant_and_wealthy_village_count_duration_treasures_and_loot_in_play` |
| Sack of Loot | $1 +1 Buy, Loot | `sack_of_loot_gives_a_coin_a_buy_and_a_loot` |
| King's Cache | three more times each (King's Cache on King's Cache) | `king_cache_on_a_king_cache_plays_three_more_treasures_three_times_each` |
| King's Cache | Duration Treasure: stays with it | `king_cache_on_a_gondola_chooses_each_time_and_stays_with_it` |
| King's Cache | Capital: 6 Debt, not 18 | `king_cache_on_a_capital_takes_the_debt_once` |
| Amphora | choose each play (King's Cache) | `amphora_gives_a_buy_and_three_coins_now_or_next_turn` and the Gondola test (same `NowOrNext` mechanism) |
| Doubloons / Endless Chalice / Figurehead / Hammer / Prize Goat / Puzzle Box / Staff / Sword | the texts | `doubloons_come_with_a_gold`, `endless_chalice_pays_a_coin_and_a_buy_every_turn_for_the_rest_of_the_game`, `figurehead_pays_three_and_draws_two_next_turn`, `hammer_pays_three_and_gains_a_card_up_to_four`, `prize_goat_gives_three_a_buy_and_a_trash`, `puzzle_box_keeps_a_card_for_the_next_hand`, `staff_pays_three_and_a_buy_and_may_play_an_action`, `sword_makes_the_others_discard_down_to_four` |
| Insignia | applies to each card gained | `insignia_lets_the_cards_gained_this_turn_go_onto_the_deck` |
| Jewels | played by King's Cache: it stays in play until the Clean-up | `jewels_played_by_a_king_cache_leaves_it_in_play_until_the_clean_up` |
| Orb | look through the discard pile, then choose | `orb_plays_a_card_from_the_discard_pile_or_gives_a_buy_and_three_coins` |
| Sextant | all/none/any back; a card put on top meanwhile stays under | `sextant_looks_at_five_discards_some_and_puts_the_rest_back`; the Progress case is not tested (needs the Project flow; `cover_deck` is the generic path) |
| Shield | blocks like a Moat, every Attack of the turn, stays in hand | `shield_blocks_an_attack_like_a_moat`, `a_shield_blocks_every_attack_of_the_turn_and_stays_in_the_hand` |
| Spell Scroll | Action or Buy phase; gained card played uses no Action | `spell_scroll_trashes_itself_to_gain_a_cheaper_card_which_may_be_played`, `spell_scroll_can_be_played_in_the_buy_phase` |
| Spell Scroll | a gained Action played in the Buy phase gives no more Actions | `a_spell_scroll_played_in_the_buy_phase_gives_no_more_actions` |
| Moat | blocks Sword | `a_moat_blocks_a_sword` |
| Loot pile | 15 x 2, shuffled only with a gainer, revealed, not buyable | `a_game_with_a_loot_gainer_has_the_loot_pile_and_others_do_not`, `the_ids_and_piles_line_up`, `the_loot_is_drawn_by_chance_in_the_search` |
| Loot pile | exchanged Loot (Changeling) goes back on top | not tested: needs Changeling's flow; `return_to_own_pile` is shared with the Reckless test |
| Bury | mandatory; +1 Buy | `bury_puts_a_card_of_the_discard_pile_on_the_bottom_of_the_deck` |
| Bury | cannot look at the discard pile before buying | offered with an empty discard pile (fixed 2026-10-28) |
| Avoid | cumulative (2 Avoids: up to 6) | `avoid_is_cumulative` |
| Avoid | no shuffle: nothing | `avoid_does_nothing_without_a_shuffle` |
| Avoid | too few cards left: no second shuffle | `avoid_leaving_too_few_cards_does_not_shuffle_again` (ignored: open) |
| Avoid | the entire deck is the shuffle (Inn); putting into the discard is not discarding | not tested: Inn's shuffle path; no discard event is emitted by the Avoid pick |
| Deliver | buying it twice does nothing extra; the cards come after drawing | `buying_deliver_twice_does_nothing_extra`, `deliver_sets_the_cards_gained_aside_until_the_end_of_the_turn` |
| Peril | Loot only if an Action was trashed | `peril_trashes_an_action_for_a_loot` |
| Rush | once per Rush; buy it again for another Action; +Actions only in the Action phase | `rush_plays_the_next_action_gained_this_turn`, `two_rushes_are_both_used_up_by_the_first_action_gained`, `rush_can_be_bought_again_for_a_second_action_and_the_actions_do_not_help_in_the_buy_phase` |
| Foray | three different names; fewer than three cards: no Loot | `foray_gains_a_loot_for_three_different_names`, `foray_with_fewer_than_three_cards_discards_them_and_gains_nothing` |
| Launch | once per turn; draws after ending the Buy phase | `launch_returns_to_the_action_phase_once_per_turn` |
| Launch | Treasury put on the deck at the end of the Buy phase is drawn | not testable: Treasury here is a discard-from-play trigger (Seaside's file) |
| Mirror | cumulative | `mirror_gains_copies_of_the_next_action_gained` |
| Prepare | mandatory plays next turn | `prepare_sets_the_hand_aside_and_plays_it_next_turn` |
| Prepare | single start-of-turn effect; no hand plays in between | not tested: the picks are one frame (`prepare_start`), so no other start entry can interleave |
| Scrounge | trash, or an Estate from the trash then a card up to $5; nothing happens with no Estate | `scrounge_trashes_a_card_or_gains_an_estate_from_the_trash_and_a_card_up_to_five` |
| Journey | extra turn, cards stay, never a third in a row | `journey_takes_an_extra_turn_keeping_the_cards_in_play_but_not_a_third_in_a_row` |
| Journey | a second Journey in the turn fails | `journey_cannot_be_bought_twice_in_a_turn` |
| Journey | Lich, Voyage, Possession, tiebreaker, Improve / Scheme, other players' cards | not tested: other sets' flows; the tiebreaker is the engine's turn count |
| Maelstrom | trash 3, others with 5+ trash one (mandatory) | `maelstrom_trashes_three_and_makes_the_others_with_five_cards_trash_one`, `maelstrom_with_exactly_three_cards_still_asks_the_others` |
| Looting | gain a Loot | `looting_gains_a_loot` |
| Invasion | the four things in order; Attack optional | `invasion_plays_an_attack_gains_a_duchy_an_action_onto_the_deck_and_a_loot_to_play` |
| Invasion | the Loot is gained to the discard pile, triggers resolve, then it is played unless moved | `a_loot_gained_by_invasion_that_is_moved_by_a_gain_effect_is_not_played` |
| Prosper | the Loot first, then Treasures one at a time | `prosper_gains_a_loot_and_differently_named_treasures` |
| Prosper | names by type (Capitalism, Charlatan) | not tested: other sets' type changes |
| Cheap | $1 less for the whole game; not below $0; coin cost only | `cheap_cards_cost_one_less`, `cheap_lowers_the_coin_cost_only` |
| Cheap | not applied during set-up (Young Witch bane) | not testable: the bane is never a Traited kingdom pile |
| Cursed | the card, then the Loot, then the Curse; no Curses: still the Loot | `cursed_gives_a_loot_and_a_curse` |
| Fated | top / bottom of the shuffled cards | `fated_cards_may_be_put_on_top_or_at_the_bottom_when_shuffling` |
| Fated | Patron, Shadow cards | not tested: Guilds / Rising Sun flows |
| Fawning | mandatory gain on a Province | `fawning_gains_a_card_of_its_pile_when_a_province_is_gained` |
| Friendly | one discard per turn, at the start of Clean-up | `friendly_gains_a_friendly_card_for_discarding_one_at_the_start_of_clean_up` |
| Friendly | split piles; Encampment | not tested: pile rotation flows (`pile_open`) |
| Hasty | set aside, played at the start of the next turn | `hasty_cards_are_set_aside_when_gained_and_played_at_the_start_of_the_next_turn` |
| Hasty / Patient | cards that cannot be played (Territory) | not tested: Allies' Territory |
| Inherited | replaces a starting card; Coppers return, Estates go to the box | `inherited_replaces_a_starting_card_with_a_card_of_its_pile` |
| Inherited | split pile: players take cards in turn order | `an_inherited_split_pile_gives_the_first_card_to_the_first_players` |
| Inherited | Shelters / Heirlooms go to the box | `inherited_can_replace_a_shelter` |
| Inherited | not "gained" | `inherited_replaces_a_starting_card_with_a_card_of_its_pile` (no gain triggers run) |
| Inspiring | an Action without a copy in play, after resolving | `inspiring_lets_the_player_play_an_action_without_a_copy_in_play_after_it` |
| Nearby | +1 Buy per gain | `nearby_gives_a_buy_when_gained` |
| Patient | set aside several, play next turn | `patient_cards_may_be_set_aside_at_clean_up_to_be_played_next_turn` |
| Pious | optional trash on each gain | `pious_lets_the_player_trash_a_card_when_one_is_gained` |
| Reckless | twice, then back to the pile | `reckless_cards_are_played_twice_and_return_to_their_pile_when_discarded_from_play` |
| Reckless | one Shield / Moat reveal covers both iterations | `a_reckless_attack_is_blocked_by_one_reveal_for_both_iterations` (ignored: open) |
| Reckless | the pile empty: still works | `a_trait_works_after_its_pile_is_empty` |
| Reckless | Conspirator, Fool's Gold, Harbor Village, Way, Enchantress, Scheme, Divine Wind | not tested: other sets' flows (the repeat is a `PlayEffects` frame with `exact`, hooks stand aside) |
| Rich | a Silver per gain | `rich_gains_a_silver` |
| Shy | one discard per turn, +2 Cards | `shy_cards_may_be_discarded_at_the_start_of_the_turn_for_two_cards` |
| Tireless | on the deck after the new hand is drawn | `tireless_cards_go_onto_the_deck_when_discarded_from_play` |
| Traits | one pile each, Action or Treasure pile | `a_kingdom_with_a_trait_puts_it_on_an_action_or_treasure_pile` |
| Traits | Divine Wind removes the pile | not testable: Divine Wind is disabled in this build |
