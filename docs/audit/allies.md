# Audit: Allies

Sources: the wiki copy (each card page, `Ally`, `Allies`, `Favor`, `Liaison`, `Rotate`, `Split pile`). Code: `crates/engine/src/sets/allies.rs`, `crates/sim/src/defaults/allies.rs`.
Tests: `crates/engine/tests/allies_cards.rs`, `crates/sim/tests/allies_defaults.rs`.

## Bugs found and fixed (each with a test that failed before)

| Bug | Rule clause | Test |
|---|---|---|
| Modify with an empty hand did nothing | "Trash a card from your hand. Choose one: +1 Card +1 Action; or gain...": the choice does not depend on the trash | `modify_with_an_empty_hand_still_gives_a_card_and_an_action` |
| Lich gained "cheaper than its printed cost" | "A trashed Lich gains a card cheaper than it ... cards changed cost (Family of Inventors)" | `a_trashed_lich_gains_a_card_cheaper_than_its_current_cost` |
| Garrison with no tokens stayed in play | "If Garrison doesn't have any tokens on it ... you discard Garrison from play during Clean-up" | `garrison_with_no_gains_is_discarded_in_clean_up` |
| Skirmisher: a Moat revealed to it did not protect later in the turn once another Attack was played (and Skirmisher overwrote the `shielded` mask of Highwayman/Warlord/Corsair) | "Revealing Moat when Skirmisher is played stops the attack" | `skirmisher_leaves_a_player_alone_who_revealed_moat_even_after_another_attack_was_played` (+ 3 Skirmisher tests) |
| Highwayman / Warlord Attacks ended only when their own start-of-turn entry resolved | "The attack ends instantaneously when your next turn starts" (before a Royal Galley's Archer) | `highwayman_ends_when_its_owners_turn_starts_before_other_start_of_turn_effects` |
| A Voyage whose extra turn Lich skipped stayed in play | "you discard the Voyage during the next Clean-up that happens" | `lich_and_voyage_in_one_turn_the_lich_skips_the_voyage_turn` |
| League of Shopkeepers / Circle / Fellowship acted before a Royal Carriage's replay and never after the replay | "play an Underling, then replay it with Royal Carriage: League of Shopkeepers triggers twice, +$2" | `league_of_shopkeepers_triggers_for_every_replay_of_a_liaison` |
| Galleria gave +Buy for a card costing $3 and a Potion | "Alchemist, Familiar, and Golem do not cost either $3 or $4" | `galleria_gives_no_buy_for_a_card_costing_a_potion` |
| Plateau Shepherds ignored Cheap (Plunder) | "Cheap cards still cost $1 less when scoring" | covered by the unit change (no Plunder test state in this file; see ruling table) |
| Bot (fast rules): Bauble topdecked any gained card, Provinces and Curses too | policy, `Strategy::decide_by_rules` | `bauble_does_not_put_a_gained_victory_card_onto_the_deck` |

## Open issues (ignored tests)

* **Battle Plan rotates only the Allies split piles.** The rules let it rotate any Supply pile: Knights, Ruins, Castles, Empires split piles, Sauna/Avanto. `battle_plan_can_rotate_an_empires_split_pile` (`#[ignore]`). `docs/allies-plan.md` section 7 says rotating other piles is a no-op; that is wrong for every pile of different cards.
* **Order of Masons: the cards left out of the shuffle go back in when the same effect needs more cards** (Sentinel looking at 5 with 2 Curses and 2 Provinces: the Provinces are shuffled in a second time; the FAQ says only one shuffle). `order_of_masons_leaves_cards_out_so_that_only_one_shuffle_happens` (`#[ignore]`).
* **Island Folk and Voyage in the same turn:** the rules make the player choose which extra turn to take (5 Favors for the Island Folk turn, which is not limited to 3 cards); the engine just takes the Voyage turn and never offers the Ally. `island_folk_and_voyage_in_one_turn_let_the_player_choose_the_extra_turn` (`#[ignore]`).
* Throne Room on a Garrison that gains nothing: the Throne Room stays in play with it (the rule says both are discarded). The Garrison itself is fixed.
* Sorceress: the other players' Moat/reaction window is evaluated when the top card is revealed (`victims()` in `run_al_named`), not when the Attack is played; no observable difference except what a reaction could draw.
* Order of Astrologers / Masons: Favors can only be spent while cards are picked (a Favor cannot be spent just to look at the cards and then put none aside). Information only.
* Highwayman stops a Treasure only when the turn's own player plays it (`p == turn.player`); the FAQ says it also stops a Treasure another player plays on the Highwayman owner's turn (Buried Treasure after a Barbarian). Rare.
* `Overlord` (Empires) cannot play a Duration from the Supply (the project follows the older "non-Duration" text; the wiki's current text allows Durations). Not an Allies bug; reported for the Empires auditor.

## Policy notes (bot defaults), not changed

* Merchant Camp / Tent: `Act::Topdeck => Yes` in `strategy.rs` puts them back on the deck every turn. Wiki (Merchant Camp): "Placing merchant camp back on top of your deck continually significantly slows deck cycling, however, which can be a liability"; and "there is little benefit to additional Merchant Camps".
* Stronghold: `+$3` vs `+3 Cards next turn` is chosen by the turn number (<= 3: cards). Wiki: "taking a benefit now or delaying it ... can often guarantee a successful subsequent turn if the current turn is a dud" (no state is looked at).
* Mountain Folk, Island Folk, Architects' Guild, Crafters' Guild, Circle of Witches, Band of Nomads: Favors are always spent when affordable. Wiki (Importer, Peaceful Cult): the Favors are a resource that other abilities (Ally of the game) share with Liaisons' bonuses; nothing is saved for later.
* Innkeeper picks the +3 / +5 Cards options from the number of junk cards only. Wiki: "Innkeeper is weakest in highly thinned decks, where its card draw forces you to discard useful cards".
* Galleria: bots do not play it first to get the +Buy ("the tricky part ... this combination must be triggered by playing Galleria first").
* Sorceress naming and Sibyl's pick are generic.

## Speed opportunities (not implemented)

* `PlayerState::all_cards` (`state.rs:1040`) adds `al.keep` unconditionally (a `SmallMultiset` length test; free when empty, but it is on the scoring / conservation path).
* `empty_piles` (`state.rs:3820`) is already gated by `AF_SPLIT`; `empty_piles_scan` is the test reference only.
* `select_ok` (`effects.rs:543`) tests `al.hook` per candidate card; `select_takes_all` already avoids it for unfiltered selections. Hoisting the test out of the per-card loop would save one byte test per card on filtered Hand/Play selections (about 1% of a selection).
* `engine.rs` cleanup: the new Garrison test is one byte (`turn.al.garrison`); the Skirmisher mask adds one `u8` to `AlTurn` (hashed only when nonzero).
* `al_gain_triggers` / `gain_live_extra` are gated by `AF_GAIN` / `turn.al.live`; nothing to do.

## Shared-file changes

* `effects.rs`: the Allies after-play frame is pushed first (so it resolves last, after Daimyo / Citadel / Royal Carriage); Highwayman / Warlord reset at the start of the turn in `push_turn_start_frames`.
* `engine.rs`: Garrison clean-up (one gated call); a Lich-skipped Voyage turn calls `al_voyage_skipped`.
* `state.rs`: `AlTurn::skirm_imm` (hashed while nonzero), `DurationHeld::remove_all`.
* `text.rs`: `skirm imm` in the `al turn:` line.

## Cards checked (all 31 kingdom piles and 24 split-pile members counted as the 55 cards, plus 23 Allies)

Cost, types, numbers: all match the wiki (Territory Victory-Clash; Stronghold Action-Duration-Victory-Fort; Distant Shore 2 VP). Marks (`choice_free` / `has_choice` / `order_sensitive`) agree with the rule in CLAUDE.md.

OK with the tests listed in the table below: Bauble, Sycophant, Importer, Merchant Camp, Sentinel, Underling, Broker, Carpenter, Courier, Innkeeper, Town, Barbarian, Capital City, Contract, Emissary, Guildmaster, Hunter, Specialist, Swap, Sorceress, Elder, Sorcerer, Hill Fort, Marquis, Sibyl, Distant Shore, Stronghold, Territory, Herb Gatherer, Acolyte, Battle Plan (except the open issue), Archer, Tent, Old Map, Student, Town Crier, Voyage, Miller, Conjurer, Blacksmith, Sunken Treasure; all Allies except Order of Masons (open issue).
Fixed: Modify, Lich, Garrison, Skirmisher, Highwayman, Warlord, Royal Galley (checked, only the Voyage / Galleria / Plateau items above), Galleria.

## Ruling coverage

Test names are in `allies_cards.rs` unless marked `(sim)`.

| Card | Ruling | Test |
|---|---|---|
| Bauble | choose two different | `bauble_chooses_two_different_things` |
| Bauble | one top-deck choice suffices, two give nothing more | `bauble_top_decking_chosen_twice_asks_once_per_gain` |
| Bauble | gained cards go onto the deck | `bauble_can_put_the_cards_gained_this_turn_onto_the_deck` |
| Sycophant | any hand size; fewer than 3 cards; none: no $3 | `sycophant_with_fewer_than_three_cards_discards_them_all` |
| Sycophant | +2 Favors on gain / trash, spendable at once | `sycophant_discards_three_for_three_coins_and_gives_favors_when_gained`, `sycophant_gives_two_favors_when_trashed`, `city_state_with_a_sycophant_gain_can_use_the_favors_just_gained` |
| Sycophant | with Toil (Event) | not testable: Toil is not implemented |
| Importer | 5 Favors each, no more | `every_player_starts_with_a_favor_and_the_importer_gives_five` |
| Importer | gain up to $5 next turn | `importer_gains_a_card_up_to_five_next_turn_and_gives_each_player_five_favors_at_set_up` |
| Merchant Camp | choose how many go on the deck | `with_several_merchant_camps_in_play_each_is_a_separate_choice` |
| Merchant Camp / Tent | played with a Way, still topdecked | `merchant_camp_and_tent_played_as_a_way_can_still_be_put_onto_the_deck` |
| Merchant Camp / Tent | played under Enchantress | not testable here: same path as the Way test, Enchantress needs an Empires kingdom (covered by the Way test) |
| Sentinel | shuffle as needed; fewer than five: look at all | `sentinel_shuffles_as_needed_and_looks_at_all_cards_if_there_are_fewer_than_five` |
| Underling | +1 Card +1 Action +1 Favor | `underling_draws_and_gives_a_favor` |
| Broker | cost per $1, example | `broker_trashes_a_card_and_gives_one_thing_per_dollar` |
| Broker | nothing for Potion / Debt in the cost | `broker_gets_nothing_for_a_potion_in_the_cost` (Debt: same code path `cost()`, not separately tested) |
| Carpenter | check the empty piles first | `carpenter_gains_a_card_and_gives_an_action_while_no_pile_is_empty_else_trashes_and_upgrades`, `carpenter_with_an_empty_pile_and_an_empty_hand_does_nothing` |
| Courier | discard top card first; Tunnel's Gold playable | `courier_resolves_a_tunnel_it_discards_before_looking_through_the_discard_pile` |
| Courier | empty discard pile: nothing to play | `courier_with_nothing_to_discard_or_play_just_gives_a_coin` (the Village Green case is not testable: Village Green is not implemented) |
| Innkeeper | +1 Action first, then the three options | `innkeeper_draws_and_discards_by_mode` |
| Royal Galley | playing is optional; discarded that turn | `royal_galley_playing_nothing_is_discarded_that_turn` |
| Royal Galley | card moved elsewhere: Galley discarded normally | `royal_galley_discards_normally_when_the_card_it_played_trashed_itself` |
| Royal Galley | both stay in play; replay next turn; no Action used | `royal_galley_plays_an_action_now_and_again_next_turn_and_keeps_both_in_play` |
| Royal Galley | Throne Room that plays a Duration next turn stays, Galley does not | not tested: needs a three-turn Throne Room / Caravan script; the Throne Room rule is covered by `a_highwayman_played_twice_is_discarded_once` |
| Town | choose one | `town_chooses_between_two_modes` |
| Barbarian | shares a type (Contract example) | `barbarian_gains_a_card_sharing_any_type_with_the_trashed_one` |
| Barbarian | cheaper card not possible: nothing; cannot trash: Curse; under $3: Curse | `barbarian_gives_a_curse_when_nothing_can_be_trashed_and_for_cards_under_three` |
| Barbarian | victim chooses; turn order | `barbarian_trashes_the_top_card_and_gives_a_cheaper_card_sharing_a_type_or_a_curse` |
| Barbarian | Potion cost: >= $3 test, no Potion card unless the trashed one had it | `barbarian_potion_costs_a_card_costing_potion_is_only_replaced_by_a_cheaper_one`, `barbarian_does_not_offer_a_potion_card_for_a_card_without_a_potion` |
| Barbarian | Debt cost | not tested: same filter as Potion (`Filter::SharesTypeWith` with `less`) |
| Capital City | order: draw, discard 2 for $2, pay $2 | `capital_city_discards_two_for_coins_or_pays_two_for_cards` |
| Capital City | may discard fewer than 2 but no $ | `capital_city_may_discard_fewer_than_two_cards_but_gets_no_coins` |
| Capital City | Way of the Chameleon | not tested: needs the Menagerie Way with Capital City, same `SWAP` path as other Chameleon tests in `menagerie_cards.rs` |
| Contract | set aside: stays; not: discarded; played at start, no Action used | `contract_sets_an_action_aside_and_plays_it_at_the_start_of_next_turn`, `contract_without_a_set_aside_action_is_discarded_the_same_turn` |
| Contract | discarded the turn it plays a Duration that stays | `contract_is_discarded_on_the_turn_it_plays_a_duration_which_stays` |
| Emissary | shuffle counts only with a card in the discard pile | `emissary_gives_actions_and_favors_only_when_it_shuffled`, `emissary_does_not_count_a_draw_that_did_not_shuffle` |
| Emissary | Order of Astrologers / Masons shuffling 0 cards; Pathfinding | not tested: the shuffle counter is stamped in `take_top` before the Ally's frame, Pathfinding needs an Adventures token script |
| Galleria | cost at the moment of gain | `galleria_counts_the_cost_at_the_time_of_the_gain`, `galleria_gives_a_buy_for_gains_costing_three_or_four` |
| Galleria | Alchemist, Familiar, Golem | `galleria_gives_no_buy_for_a_card_costing_a_potion` |
| Galleria | Destrier, Gondola | not tested: Destrier's cost is the same `cost_when_gained` path; Gondola is not implemented |
| Guildmaster | Favor usable by an Ally ability on the same gain | `guildmasters_favor_can_be_spent_on_the_ally_for_the_same_gain` |
| Highwayman | +3 Cards per play, discards once | `highwayman_thrown_twice_draws_six_cards_and_is_discarded_once`, `a_highwayman_played_twice_is_discarded_once` |
| Highwayman | discard first, drawn again | `highwayman_is_discarded_before_its_cards_are_drawn` |
| Highwayman | every turn incl. extra turns; not cumulative | `highwayman_stops_the_first_treasure_of_every_turn_including_an_extra_turn`, `several_highwaymen_still_stop_only_one_treasure_per_turn` |
| Highwayman | Moat | `highwayman_does_not_affect_a_player_who_revealed_moat` |
| Highwayman | Treasure that is an Action (Crown); Fool's Gold | `highwayman_blocks_a_crown_played_as_the_first_treasure`, `a_blocked_fools_gold_still_counts_as_played_for_the_other_one` |
| Highwayman | Scheme / Tireless / Reckless / Envious / Coppersmith / Capitalism / Caravan Guard / Specialist replay | not tested: Tireless and Reckless are Plunder traits needing a Plunder script, Envious needs Nocturne, Capitalism and Caravan Guard are Empires / Adventures interactions (the blocked-play bookkeeping they depend on is the same as in the Fool's Gold test) |
| Highwayman | a Treasure another player plays during your turn | open issue (listed above), no test |
| Hunter | choose Action, then Treasure, then Victory; multi-type cards | `hunter_takes_an_action_a_treasure_and_a_victory_card_of_the_top_three`, `hunter_takes_a_card_with_several_types_for_one_type_only`, `hunter_discards_the_cards_it_does_not_take` |
| Modify | trash, then choose; Elder order draw before gain | `modify_trashes_a_card_for_a_card_and_an_action_or_a_better_card`, `elder_on_modify_with_both_choices_draws_before_it_gains`, `modify_with_an_empty_hand_still_gives_a_card_and_an_action` |
| Skirmisher | each play attacks; Moat only at the play; gain by Specialist | `skirmisher_attacks_once_per_skirmisher_played`, `skirmisher_does_not_hit_a_player_who_revealed_moat_when_it_was_played`, `skirmisher_hits_a_player_who_did_not_reveal_moat_and_the_gain_asks_no_second_reveal`, `a_skirmisher_gained_by_specialist_after_it_was_played_makes_the_others_discard` |
| Skirmisher | gain by City-state before attacking; Invest exile / exchange is no gain; Kiln | not tested: Invest, Kiln need Menagerie scripts; exchange / exile do not run `gain` triggers (`al_gain_triggers` is only called from `finish_gain`) |
| Specialist | gain a copy even if none left; replay after leaving play; Treasure | `specialist_gain_a_copy_with_the_pile_empty_gains_nothing`, `specialist_plays_a_card_again_even_when_it_left_play`, `specialist_plays_a_treasure_twice`, `specialist_plays_a_card_and_then_plays_it_again_or_gains_a_copy` |
| Swap | return (not trash), gain different Action to hand; split pile; Horse | `swap_cannot_gain_the_card_it_returned_and_gains_to_the_hand`, `rotation_puts_a_pile_out_of_order_when_a_card_is_returned_on_top`, `swap_can_return_a_card_that_has_a_pile_outside_the_supply` |
| Swap | a card with no pile (Necropolis, Black Market); Possession | not tested: Black Market and Possession are not implemented |
| Sorceress | named card draws; always to hand; no cards: no Curse | `sorceress_names_a_card_and_the_others_get_curses_when_it_is_the_one_drawn`, `sorceress_with_no_cards_at_all_gives_no_curse` |
| Warlord | two copies in play from the hand; Throne Room one copy; Treasures unaffected | `warlord_stops_others_from_playing_an_action_they_have_two_copies_of_in_play`, `warlord_lets_throne_room_play_a_card_with_one_copy_in_play_and_ignores_treasures` |
| Warlord | ends as the turn starts | `highwayman_ends_when_its_owners_turn_starts_before_other_start_of_turn_effects` |
| Warlord | Golem / Shadow cards / Black Cat / Guide / Caravan Guard / Way | not tested: Golem (Alchemy) set-aside plays do not go through the hand check (`al_play_blocked` is only called for hand selections); the other cards need Nocturne / Rising Sun / Adventures scripts |
| Herb Gatherer | deck into discard is no discard (Tunnel); play optional | `herb_gatherer_discarding_the_deck_does_not_trigger_a_tunnel`, `herb_gatherer_puts_the_deck_into_the_discard_pile_and_may_play_a_treasure_from_it` |
| Battle Plan | order: +card, reveal Attack, rotate any Supply pile | `battle_plan_reveals_an_attack_for_a_card_and_rotates_any_pile` |
| Battle Plan | Knights, Ruins, Castles, other split piles | open: `battle_plan_can_rotate_an_empires_split_pile` (ignored) |
| Battle Plan | Ferryman's pile is not in the Supply | not tested: Ferryman's pile is not in `in_supply` (so not offered), no script |
| Tent | multiple Tents choose how many | `tent_may_be_put_onto_the_deck_when_discarded` (per-copy offers as for Merchant Camp) |
| Old Map | order | `old_map_draws_discards_and_draws` |
| Student | rotate optional, trash mandatory; Treasure: Favor and topdeck; non-Treasure stays | `student_rotates_the_wizards_and_trashes_a_card_and_a_treasure_gives_a_favor_and_returns_it_to_the_deck` |
| Student | not in play (Band of Misfits): Favor, no topdeck | `student_played_by_band_of_misfits_gives_a_favor_but_cannot_go_onto_the_deck` |
| Student | Throne Room loses track | `a_student_played_twice_returns_only_once` |
| Town Crier | choose, then rotate; Silver | `town_crier_gains_a_silver_and_then_asks_about_rotating`, `rotating_puts_the_top_card_and_its_copies_at_the_bottom` |
| Acolyte | both optional; Gold only if trashed; Augur on top even an Acolyte | `acolyte_abilities_are_each_optional`, `acolyte_gains_whichever_augur_is_on_top_even_another_acolyte`, `acolyte_trashes_an_action_or_victory_for_a_gold_and_itself_for_an_augur` |
| Acolyte | Throne Room (stop-moving) | `a_throne_roomed_acolyte_can_trash_itself_only_once` |
| Voyage | no third turn in a row; two Voyages; Throne Room counts; Treasures limited; Lich | `voyage_played_on_a_voyage_turn_gives_no_third_turn_in_a_row`, `two_voyages_in_one_turn_give_one_extra_turn`, `voyage_turn_counts_throne_room_and_the_card_but_not_the_replay`, `voyage_limits_treasures_too`, `lich_and_voyage_in_one_turn_the_lich_skips_the_voyage_turn` |
| Voyage | Golem; Market Towns after 3 plays; Shadows; Possession; Mission; gained-to-hand City-state | not tested: Golem / Possession / Mission need other sets' scripts; the limit is one counter (`hand_plays`) applied in `al_play_blocked`, tested by the Treasures and Throne Room tests |
| Miller | fewer than four cards | `miller_with_fewer_than_four_cards_looks_at_what_is_left`, `miller_looks_at_four_and_takes_one` |
| Conjurer | returns each turn; Band of Misfits; Throne Room loses track | `conjurer_gains_a_card_and_returns_to_the_hand_next_turn`, `a_conjurer_played_twice_returns_to_the_hand_once_and_a_second_copy_stays_where_it_is` (Band of Misfits cannot play a Duration in this project: not testable) |
| Archer | players with 5+ cards after reactions; attacker picks; fewer than 5 untouched | `archer_makes_a_player_with_five_cards_reveal_all_but_one_and_discards_the_one_the_attacker_picks`, `archer_does_not_attack_a_player_with_fewer_than_five_cards`, `archer_only_attacks_a_player_who_still_has_five_cards_after_their_reactions` |
| Garrison | tokens only in play; Throne Room 6 tokens, 6 cards; no tokens: discarded | `garrison_counts_the_gains_of_the_turn_and_draws_that_many_cards_next_turn`, `a_throne_roomed_garrison_gets_tokens_twice_and_draws_them_once`, `garrison_with_no_gains_is_discarded_in_clean_up` |
| Garrison | played by Band of Misfits / Haven vs. Cargo Ship; Improve | not testable: Misfits and Overlord play only non-Durations here; Improve needs the Renaissance script and the gain path is the same |
| Blacksmith | draw to 6; 2 Cards; Card+Action | `blacksmith_draws_up_to_six_or_two_cards_or_a_card_and_an_action`, `blacksmith_draws_nothing_when_the_hand_already_has_six_cards` |
| Elder | extra choice optional; order; Count; Stronghold; no extra for replays | `elder_gives_two_coins_and_plays_an_action_with_an_extra_choice`, `elder_on_count_gives_each_choose_one_an_optional_extra_pick`, `stronghold_with_elder_gives_both_coins_now_and_cards_next_turn`, `elder_does_not_give_an_extra_choice_to_a_royal_carriage_replay` |
| Elder | Scrap, Amulet, Quartermaster at start of turn, Way of the Chameleon, Reckless, Enlightenment | not tested: each needs a script of another set; the extra pick is stamped on every `Mode` frame the played card pushes (`al_elder_stamp`) |
| Sunken Treasure | no Action in the Supply: nothing | `sunken_treasure_gains_an_action_with_no_copy_in_play`, `sunken_treasure_gains_nothing_when_every_action_is_in_play` |
| Sorcerer | each other player names; returned to the deck; no cards: no Curse | `sorcerer_makes_the_others_name_a_card_and_curses_a_wrong_guess`, `sorcerer_returns_the_revealed_card_to_the_top_whatever_the_name`, `sorcerer_with_no_cards_in_the_victims_deck_or_discard_gives_no_curse` |
| Hill Fort | moved card cannot be put into the hand; covered card can; Sheepdog; Gatekeeper | `hill_fort_gains_a_card_to_the_hand_or_gives_a_card_and_an_action`, `hill_fort_cannot_put_a_card_into_the_hand_that_a_reaction_moved` (Sheepdog is not implemented; Gatekeeper's Exile is a gain-time move covered by the stop-moving tests) |
| Marquis | discard to 10 even when the draw fell short | `marquis_discards_down_to_ten_even_when_the_deck_runs_out`, `marquis_draws_a_card_per_card_in_hand_and_discards_down_to_ten` |
| Sibyl | empty deck after drawing | `sibyl_with_an_empty_deck_after_drawing_puts_the_first_card_on_top_and_the_second_under_it`, `sibyl_draws_four_and_puts_one_card_on_the_deck_and_one_on_the_bottom` |
| Distant Shore | Estate gain mandatory; pile empty still draws | `distant_shore_draws_two_gains_an_estate_and_is_worth_two_points`, `distant_shore_with_the_estates_gone_still_draws_and_gives_an_action` |
| Lich | skip a turn; extra turn; Outpost; Smugglers; Monkey | `lich_draws_six_skips_the_next_turn_and_gains_a_cheaper_card_from_the_trash_when_trashed`, `lich_and_voyage_in_one_turn_the_lich_skips_the_voyage_turn` (Outpost, Smugglers, Monkey, Possession, Lurker: not tested, needing other sets' scripts) |
| Lich | trash: discard (no gain), cheaper card, mandatory; current cost; trashed with a Sycophant | `a_trashed_lich_is_discarded_and_gains_a_cheaper_card_from_the_trash`, `a_trashed_lich_gains_a_card_cheaper_than_its_current_cost`, `a_lich_trashed_together_with_a_sycophant_can_gain_the_sycophant` |
| Stronghold | +$3 discards; cards next turn stays; both with Elder | `stronghold_gives_three_coins_or_three_cards_next_turn_and_two_victory_points`, `stronghold_with_elder_gives_both_coins_now_and_cards_next_turn` |
| Territory | counts differently named Victory cards; Golds per empty pile incl. the Clashes | `territory_is_worth_a_point_per_differently_named_victory_card_and_gains_golds_for_empty_piles`, `gaining_the_last_territory_counts_the_emptied_clashes_for_its_golds` |
| Territory | Hasty / Patient Clashes | not testable here: needs a Plunder script |
| Split piles | order, top only, rotation, empty only when all four gone, returned cards on top, consecutive run rotates | `a_game_with_a_split_pile_has_its_four_cards_in_order`, `rotating_puts_the_top_card_and_its_copies_at_the_bottom`, `taking_the_last_copy_of_the_top_card_opens_the_next_and_the_pile_is_empty_when_all_four_cards_are_gone`, `a_gain_costing_up_to_a_price_can_only_take_the_top_card_of_a_split_pile`, `rotation_puts_a_pile_out_of_order_when_a_card_is_returned_on_top` |
| Allies | one Ally per game, one Favor each, Favors from the first turn | `a_random_kingdom_with_a_liaison_gets_one_ally`, `every_player_starts_with_a_favor_and_the_importer_gives_five` |
| Architects' Guild | chain off its own gain; two Favors; cost at the time of gaining | `architects_guild_can_trigger_off_its_own_gain`, `architects_guild_spends_two_favors_to_gain_a_cheaper_non_victory_card`, `architects_guild_needs_two_favors`, `architects_guild_uses_the_cost_after_the_card_was_gained` |
| Band of Nomads | cost when gained; other players' turns | `band_of_nomads_needs_a_card_costing_three_at_the_time_of_the_gain`, `band_of_nomads_can_be_used_on_a_gain_during_another_players_turn`, `band_of_nomads_gives_a_card_an_action_or_a_buy_for_a_favor_on_a_gain_of_three_or_more` |
| Cave Dwellers | repeat; draw even if nothing discarded | `cave_dwellers_discards_a_card_and_draws_one_as_often_as_desired` (the empty-hand case is not testable at turn start: a non-empty deck always fills the hand; the code pushes the draw unconditionally) |
| Circle of Witches | after resolving, Favors just gained count; not an Attack (Moat) | `circle_of_witches_gives_curses_after_a_liaison_for_three_favors`, `circle_of_witches_curses_cannot_be_blocked_by_moat` |
| City-state | own turn only; Buy phase | `city_state_plays_a_gained_action_for_two_favors`, `city_state_plays_only_cards_gained_on_your_own_turn` |
| City-state | covered card in the discard pile; draws Treasures after buying | not tested: the play uses the gain's stop-moving token (`al_gain_triggers`); a covering effect needs a Skulk script |
| Coastal Haven | kept cards do not count; normal Clean-up only | `coastal_haven_keeps_cards_that_do_not_count_against_the_next_hand`, `coastal_haven_keeps_cards_in_hand_for_favors` |
| Crafters' Guild | onto the deck | `crafters_guild_gains_a_card_onto_the_deck_for_two_favors` |
| Desert Guides | repeat | `desert_guides_discards_the_hand_and_draws_five` |
| Family of Inventors | no Victory piles; split piles with Victory cards; Castles; cumulative; floor 0; everyone | `family_of_inventors_tokens_are_never_put_on_victory_piles_but_may_go_on_split_piles_with_victory_cards`, `family_of_inventors_tokens_add_up_and_do_not_go_below_zero_and_help_every_player`, `family_of_inventors_puts_a_favor_on_a_pile_to_make_it_cheaper` |
| Family of Inventors | Knights with Dame Josephine on top | not tested: needs a seeded Knights pile; `al_token_pile` treats the Knights pile as a non-Victory pile by its marker id |
| Fellowship of Scribes | once per play, 4 or fewer cards | `fellowship_of_scribes_draws_a_card_after_an_action_with_four_or_fewer_cards_in_hand` |
| Forest Dwellers | once per turn | `forest_dwellers_looks_at_three_cards_and_discards_any_number` (one Ask per turn start) |
| Gang of Pickpockets | spend or discard to 4; not an Attack | `gang_of_pickpockets_discards_down_to_four_unless_a_favor_is_spent` |
| Island Folk | not on an extra turn; no third turn | `island_folk_takes_an_extra_turn_for_five_favors`, `island_folk_cannot_be_used_on_an_extra_turn` |
| Island Folk | with Voyage in the same turn | open: `island_folk_and_voyage_in_one_turn_let_the_player_choose_the_extra_turn` (ignored) |
| League of Bankers | round down; extra Favors at the start of the Buy phase; several Buy phases | `league_of_bankers_gives_a_coin_per_four_favors_at_the_start_of_the_buy_phase`, `league_of_bankers_pays_again_for_a_second_buy_phase` |
| League of Shopkeepers | 5 / 10 Favors; Throne Room once; Royal Carriage twice | `league_of_shopkeepers_gives_a_coin_with_five_favors_and_an_action_and_a_buy_with_ten`, `league_of_shopkeepers_triggers_once_for_a_throne_roomed_liaison`, `league_of_shopkeepers_triggers_for_every_replay_of_a_liaison` |
| League of Shopkeepers | Innovation, Citadel | not tested: Citadel / Innovation use the same after-play frame and gain token as Royal Carriage / City-state |
| Market Towns | fully resolve; Favors gained while resolving; no Action phase | `market_towns_plays_an_action_from_the_hand_for_a_favor_at_the_start_of_the_buy_phase`, `market_towns_can_spend_favors_gained_while_it_resolves`, `market_towns_does_not_give_back_the_action_phase` |
| Market Towns | Villa / Cavalry in the middle; Leprechaun | not tested: need Empires / Nocturne scripts through the Ask frame |
| Mountain Folk | needs the full 5 | `mountain_folk_draws_three_cards_for_five_favors` |
| Order of Astrologers | Favors not yet had; order | `order_of_astrologers_cannot_use_favors_an_emissary_gives_after_its_own_shuffle`, `order_of_astrologers_puts_cards_on_top_of_the_deck_when_shuffling_for_favors` |
| Order of Astrologers | Inn, Star Chart, spending a Favor to look and pick none | not tested / approximation (listed above) |
| Order of Masons | up to 2 cards per Favor; one shuffle | `order_of_masons_keeps_cards_out_of_a_shuffle_for_favors`; one shuffle: open (ignored test) |
| Peaceful Cult | all Favors at once; Favors from the trash do not add trashes; several Buy phases | `peaceful_cult_trashes_cards_for_favors_spent`, `peaceful_cult_cannot_trash_more_after_a_trashed_sycophant_gives_favors` |
| Plateau Shepherds | exactly $2; Potion cards do not count; Flourishing Trade | `plateau_shepherds_pairs_favors_with_cards_costing_two`, `plateau_shepherds_counts_cards_costing_exactly_two_and_flourishing_trade_changes_that` |
| Plateau Shepherds | Cheap | not tested: needs a Plunder trait state; the one-line change is `score_cheap` |
| Trappers' Lodge | empty deck | `trappers_lodge_puts_a_gained_card_onto_the_deck_for_a_favor`, `trappers_lodge_on_an_empty_deck_makes_the_card_the_only_card` |
| Woodworkers' Guild | any cost incl. Potion; only if trashed | `woodworkers_guild_trashes_an_action_and_gains_an_action`, `woodworkers_guild_can_gain_an_action_of_any_cost_including_a_potion` |
| Bots | Bauble topdeck of a dead card; Archer victim keeps the best; Capital City discards dead cards | (sim) `bauble_does_not_put_a_gained_victory_card_onto_the_deck`, `the_archers_victim_keeps_the_best_card_and_the_attacker_discards_the_best_of_the_rest`, `capital_city_discards_the_dead_cards` |
| Bots | legality over many Allies kingdoms | (sim) `bots_play_every_card_and_ally_to_the_end` (also run once with 40 kingdoms x 6 games: no illegal choice) |
