# Audit: Prosperity, Alchemy, Promos, Cornucopia & Guilds

Sources: the offline wiki (current text, Official FAQ, Other rules clarifications, versions). Branch `worktree-agent-adf49e7f19ac36c2f`.

## Bugs found and fixed (each has a test that fails on the old source)

| Bug | Rule clause | Test |
|---|---|---|
| Peddler's cost ignored Treasures in play under Enlightenment | "Treasures are Actions for all purposes" | `cross_set::peddler_costs_2_less_per_treasure_in_play_under_enlightenment` |
| Walled Village's Clean-up check ignored Treasures as Actions | same | `promo_cards::walled_village_counts_the_treasures_in_play_as_actions_under_enlightenment` |
| Housecarl, Collection, Transmute, Scrying Pool (non-Action stop), Coronet's non-Reward Action filter, Summon availability ignored Enlightenment | same | `cross_set::housecarl_counts_...`, `collection_pays_a_vp_...`, `transmute_gains_a_duchy_...`, `scrying_pool_keeps_revealing_...` (Coronet/Summon use the same shared filters) |
| Vineyard score ignored Enlightenment ("Treasures remain Actions when scoring") | Vineyard FAQ | `cross_set::vineyard_counts_treasures_as_actions_when_scoring_under_enlightenment` |
| Prince and Captain accepted Command cards made cheap by cost reduction | "non-Duration, non-Command Action" | `promo_cards::prince_cannot_set_aside_a_command_card_...`, `captain_cannot_play_a_command_card_...` |
| A Treasure Herbalist puts on the deck did not pay its "when discarded from play" cost (Capital's Debt) | Herbalist: "you still have to do its ability" | `alchemy_cards::a_treasure_with_a_discard_from_play_effect_still_does_it_when_herbalist_topdecks_it` |
| Transmute did not count an Inheritance Estate as an Action | Transmute FAQ (Inheritance) | `alchemy_cards::transmute_treats_an_inherited_estate_as_an_action_and_a_victory_card` |
| Only the Bane's own card (top of a split pile) blocked Young Witch | "All cards from the split pile can be revealed" | `cornucopia_guilds_cards::every_card_of_a_split_pile_bane_blocks_young_witch` |
| A Liaison as Bane (or Ferryman card) brought no Ally or Favors | "Do any set-up the card requires" | `cornucopia_guilds_cards::a_bane_that_needs_setup_brings_it` |
| A Prince played at the start of the turn (Piazza) did not play its card that turn | Prince: "you'll play the set-aside card on the same turn" | `promo_cards::a_prince_played_at_the_start_of_the_turn_...` |
| A Hireling played at the start of the turn (Summon) did not draw that turn | Summon FAQ | `promo_cards::a_summoned_hireling_draws_...` |

Smoke test added: `crates/sim/tests/audit_prosperity_fuzz.rs` (every kingdom card of the four sets bought by a bot in games; no panics, no illegal Pass found).

## Open issues
- Naming a card that is not in the Supply for War Chest is not offered (the Name decision lists Supply cards). No observable effect (naming such a card only forfeits the namer's restriction). Not fixed, no test (deliberate).
- Stonemason / Remake / Expand / Forge with a trashed card that has Debt in its cost compare coins only and exclude all Debt cards from the gain (Dismantle alone carries the Debt in `min`). Rule is obscure; covered for Remodel-like cards by `alchemy_cards::remodel_on_a_debt_cost_card_keeps_the_limit_in_coins`. Left.
- Other sets' `setup` functions (Empires Tax, Plunder, Menagerie, Renaissance) read `cfg.kingdom` only, so a Bane / Ferryman card needing their set-up would miss it. Allies fixed (above); the others are a one-line widening but belong to those sets' auditors.

## Policy notes (no clear errors; all bot decisions legal)
- Mint: wiki "a Mint trash is also sometimes worthwhile even if you can only get four Coppers into play ... only rarely with less than that". The bot has no default guard: buying Mint is the strategy author's `[[gain]]` condition.
- Vault (victims): wiki "This benefit tends to be of little use in Big Money games: discarding two coppers risks drawing another copper". The bot's default discards only dead cards (Curse, Victory), never two Coppers. Consistent.
- War Chest namer: wiki "your opponent will ban Duchy" in the late game; the bot denies the costliest card up to $5, which agrees.
- Scrying Pool / Herbalist / Governor / Church / Marchland defaults follow the wiki Strategy lines (discard what the opponent wants, topdeck Crown or Gold only when useful, save Provinces with Church, discard junk for Marchland).

## Speed opportunities (not implemented)
- `sets/promo.rs` `turn_start_entries` (about line 147) iterates `prince_cards` and `summon_cards` and `cornucopia_guilds.rs:238` `farm_cards` every turn in every game: a 16-block bitmap walk each, roughly 50 ns per call per turn. Gate on `in_supply(PRINCE)` / any Summon or Farmhands in the game: under 0.5% of a plain game.
- `sets/prosperity.rs` `gain_triggers` loops all four `GAIN_TRIGGERS` entries per gain even when only Watchtower is in the game (4 count lookups); gate per entry with the `GAIN_LO` mask bit: tiny.
- `Cornucopia bane_in_hand` (new) walks the hand only for Young Witch curses: negligible.
- `GameState::cost` Peddler branch now calls `actions_in_play` (iterates `in_play`) only when `c == PEDDLER` in the Buy phase: no cost for other games.

## Card check (one line each: OK or the issue)

Prosperity: Anvil OK. Watchtower OK. Bishop OK. Clerk OK. Investment OK. Monument OK. Quarry OK (cost reduction cumulative per play, stays when trashed). Tiara OK. Worker's Village OK. Charlatan OK. City OK. Collection fixed (Enlightenment). Crystal Ball OK. Magnate OK. Mint OK. Rabble OK. Vault OK. War Chest OK (see open issue). Grand Market OK. Hoard OK. Bank OK. Expand OK. Forge OK. King's Court OK. Peddler fixed (Enlightenment). Platinum OK (12). Colony OK (8/12).
Alchemy: Transmute fixed (Inheritance, Enlightenment). Vineyard fixed (Enlightenment). Herbalist fixed (Capital). Apothecary OK. Scrying Pool fixed (Enlightenment). University OK. Alchemist OK. Familiar OK. Philosopher's Stone OK. Golem OK. Apprentice OK. Potion OK (16).
Promo: Prince fixed (Command, Piazza). Walled Village fixed (Enlightenment). Envoy OK. Captain fixed (Command). Sauna/Avanto OK (tests in `adventures_cards.rs`). Governor OK. Dismantle OK. Church OK. Marchland OK (8/12). Summon OK (Hireling fixed).
Cornucopia & Guilds: Candlestick Maker, Hamlet, Farrier, Stonemason, Menagerie, Shop, Infirmary, Advisor, Farmhands, Plaza, Remake, Herald, Baker, Butcher, Carnival, Footpad, Horn of Plenty, Hunting Party, Jester, Journeyman, Joust, Merchant Guild, Soothsayer, Fairgrounds, Ferryman: OK. Young Witch fixed (split Bane, Liaison Bane). Rewards: Coronet fixed (filter), Courser, Demesne, Housecarl fixed (Enlightenment), Huge Turnip, Renown: OK.
`choice_free` / `has_choice` / `order_sensitive` marks checked against the CLAUDE.md rule: no change needed.

## Ruling coverage

Test names are in `crates/engine/tests/<set>_cards.rs` unless noted (`xs` = `cross_set.rs`). "new" = added by this audit.

### Prosperity (`prosperity_cards.rs`)
| Card | Ruling | Test |
|---|---|---|
| Anvil | discarding optional; gain up to $4 to discard pile; +$1 | `anvil_declining_the_discard_gains_nothing`, `anvil_may_discard_a_treasure_to_gain_up_to_four`, `anvil_plays_for_one_coin` |
| Investment | trash happens even with last card | `investment_with_nothing_in_hand_to_trash_still_offers_both_options` (new) |
| Investment | choose +$1 or trash for VP per differently named Treasure | `investment_trash_then_plus_one_coin`, `investment_trash_self_for_vp_per_differently_named_treasure` |
| Investment | revealed Treasures can still be played | `the_treasures_investment_revealed_can_still_be_played` (new) |
| Investment | played twice, trashed once | `an_investment_played_twice_cannot_be_trashed_for_vp_by_its_second_play` |
| Crystal Ball | leave card on top | `crystal_ball_may_leave_the_top_card_in_place` |
| Crystal Ball | Action played in Buy phase gives no Action plays | `crystal_ball_playing_an_action_in_the_buy_phase_gives_no_more_action_plays` (new) |
| Tiara | later gains may all go on deck, bought or not | `tiara_may_put_every_card_gained_later_in_the_turn_on_the_deck`, `tiara_also_topdecks_a_card_a_war_chest_gains` (new) |
| Tiara | Tiara with Tiara: two Treasures twice each | `a_tiara_played_with_a_tiara_plays_two_treasures_twice_each` (new) |
| Tiara | Mint gained still lets Tiara topdeck | `tiara_still_offers_to_topdeck_a_mint_that_trashes_the_tiara` |
| Bishop | own trash mandatory, others optional, order | `bishop_own_trash_is_mandatory_when_something_is_available`, `bishop_other_players_may_each_trash_a_card`, `bishop_gives_vp_tokens_only_to_its_player` (new) |
| Bishop | Potion/Debt ignored; no card still +$1 +VP | `alchemy_cards::bishop_counts_only_coins`, `bishop_alone_gives_one_vp_token_and_one_coin` |
| Watchtower | draw to 6, none if already 6 | `watchtower_draws_to_six_in_hand`, `watchtower_already_at_six_draws_nothing`, `watchtower_not_enough_cards_to_reach_six` |
| Watchtower | any gain, bought or other; each gain separate; trash still gained | `watchtower_reacts_to_a_bought_card_by_trashing_it`, `..._topdecking_it`, `watchtower_reacts_to_an_opponents_witch_curse`, `watchtower_may_decline_to_react_to_a_gain` |
| Watchtower | gained to hand / reacts with itself | `watchtower_reacts_to_a_gain_to_hand` |
| Watchtower | stays in hand when revealed | `watchtower_stays_in_the_hand_after_being_revealed` (new) |
| Watchtower | Death Cart topdeck before Ruins | not testable: both orders give the same final state (Death Cart on the deck, Ruins gained) |
| Clerk | 5+ cards, topdeck one; empty deck | `clerk_gives_two_coins_and_attacks_full_handed_opponents`, `clerk_does_not_attack_a_player_with_fewer_than_five_cards`, `clerk_puts_the_card_on_an_empty_deck_as_its_only_card` (new) |
| Clerk | start of turn: any number, no Action | `any_number_of_clerks_may_be_played_at_the_start_of_the_turn_without_using_actions` (new); order: `start_order.rs` |
| Monument, Worker's Village | printed effects | `monument_gives_two_coins_and_a_vp_token`, `workers_village_card_two_actions_and_a_buy` |
| Quarry | cumulative; mixes with other reductions; Tiara/Crown/Counterfeit | `quarry_reduces_action_cost_by_two_while_in_play_and_stacks`, `quarry_mixes_with_a_flat_turn_wide_discount`, `a_quarry_played_twice_with_tiara_takes_four_off_action_costs`, `a_quarry_played_twice_by_crown_or_counterfeit_takes_four_off_action_costs` (new), `quarrys_discount_lasts_the_turn_even_if_a_mint_trashes_the_quarry` |
| Charlatan | Curse a Treasure everywhere; Bank/Mint/Courtier/Magnate | `charlatan_makes_curse_a_one_coin_treasure_and_bank_counts_it`, `magnate_counts_a_curse_in_the_hand_when_charlatan_makes_it_a_treasure` (new), `xs::courtier_counts_a_curse_as_two_types...` |
| Charlatan | still -1 VP; Curses in trash are Treasures (Forager) | `a_curse_still_scores_minus_one_vp_when_charlatan_makes_it_a_treasure`, `a_curse_in_the_trash_is_a_treasure_for_forager_under_charlatan` (new) |
| Charlatan | Black Market deck / Keep Landmark | not testable: Black Market is not implemented; Keep counts Treasures via the same `is_treasure` (no separate test) |
| City | draw/actions; 1 or 2 empty piles; no more at 3 | `city_turn`, `city_scales_with_empty_piles` |
| City | checked only when played; Spoils not a Supply pile | `city_only_checks_the_piles_when_it_is_played`, `city_counts_only_supply_piles_not_the_spoils` (new) |
| City | Star Chart separate draws | not testable here: the order of the two draws is internal (separate draw frames); Star Chart interaction in `renaissance_cards.rs` |
| Collection | per gained Action, cumulative, only after play, leaves play, played twice | `collection_gives_vp_only_for_gained_actions_while_in_play`, `a_collection_played_twice_with_tiara_gives_two_vp_per_gained_action_card` |
| Magnate | +1 Card per Treasure | `magnate_draws_one_card_per_treasure_in_hand` |
| Mint | on gain trashes non-Duration Treasures in play, any gain, not hand | `mint_buying_it_trashes_treasures_in_play_but_not_durations_2nd_edition`, `mint_gained_any_way_trashes_the_non_duration_treasures_in_play` |
| Mint | play: reveal, copy to discard, stays in hand | `mint_reveal_gains_a_copy_to_discard`, `mint_may_decline_to_reveal`, `mint_no_treasures_in_hand_is_a_no_op` |
| Mint | coppers gone then Grand Market | `after_mint_trashed_the_coppers_a_grand_market_can_be_bought_with_the_same_turn_money` (new) |
| Mint | Capitalism gains and trashes Action-Treasures | `mint_gains_and_trashes_an_action_treasure_under_capitalism` (new) |
| Mint | cannot copy Spoils; Heirlooms; Black Market cards | `mint_cannot_gain_a_copy_of_spoils` (new); Heirloom same code path (copy only from Supply); Black Market not implemented |
| Mint | Mandarin | not testable: Mandarin was removed from the project's Hinterlands (2nd edition) |
| Rabble | shuffle for 3 / reveal what exists; discard Action and Treasure; order | `rabble_discards_actions_and_treasures_from_victims_and_topdecks_the_rest`, `rabble_shuffles_for_its_victims_and_reveals_what_there_is` (new) |
| Rabble | Night cards stay | `rabble_does_not_discard_night_cards` (new) |
| Vault | any number incl. zero; discard drawn cards | `vault_own_discard_for_coins`, `vault_lets_its_player_discard_the_cards_it_just_drew` (new) |
| Vault | victims: choose, discard 2 draw 1; one card; reshuffle | `vault_victim_discards_two_to_draw_one`, `vault_victim_with_only_one_card_may_discard_it_but_does_not_draw`, `vault_victim_may_decline_to_discard_at_all`, `a_vault_victim_who_discards_their_last_cards_shuffles_them_back_in_to_draw` (new) |
| War Chest | left player names; excluded this turn; Supply to discard | `war_chest_left_player_names_then_owner_gains_up_to_five`, `war_chest_excludes_names_from_earlier_war_chests_this_turn` |
| War Chest | naming a card not in the Supply | not testable: engine offers only Supply cards (open issue, no effect on outcomes) |
| Grand Market | Copper in play blocks; in hand / earlier / $1 Treasures do not; Expand | `grand_market_cant_be_bought_with_a_copper_in_play`, `only_a_copper_in_play_blocks_grand_market`, `grand_market_can_be_gained_by_expand_with_a_copper_in_play`, `after_mint_trashed_the_coppers...` (new) |
| Hoard | bought Victory only; cumulative; Tiara/Crown; no Gold left | `hoard_only_reacts_to_a_bought_victory_card_while_in_play`, `two_hoards_double_the_gold_per_bought_victory_card`, `a_hoard_played_twice_with_tiara...`, `hoard_with_no_gold_left_gains_nothing_extra`, `hoard_pays_a_gold_for_a_bought_action_victory_card` (new) |
| Bank | counts itself; second Bank +1; later Treasures do not change | `bank_counts_treasures_in_play_including_itself`, `a_second_bank_is_worth_one_more_and_later_treasures_do_not_change_either` (new), `tiara_replaying_bank_recomputes_its_value_each_time` |
| Expand | no card, no gain; cost need not be exact; copy; Potion counts | `expand_with_an_empty_hand_gains_nothing`, `expand_copper_to_silver`, `expand_can_gain_a_copy_of_the_card_it_trashed` (new), `alchemy_cards::expand_on_transmute_gains_up_to_three_coins_and_a_potion` |
| Forge | any number incl. zero; zero total; no card at the cost; Potion/Debt not added | `forge_trashes_several_and_gains_exactly_the_summed_cost`, `forge_no_cards_gains_a_zero_cost_card`, `forge_gains_nothing_when_no_card_costs_exactly_the_total` (new), `alchemy_cards::forge_counts_only_coins_and_cannot_gain_a_potion_cost_card` |
| King's Court | optional; thrice; KC on KC; Actions not used | `kings_court_may_play_nothing`, `kings_court_plays_village_three_times`, `kings_court_on_kings_court_on_village` |
| Peddler | cost by Actions in play in Buy phase only; hands and decks; KC counts once | `peddler_costs_less_per_action_in_play_only_during_buy_phase`, `peddler_counts_itself_once_in_play`, `peddler_counts_a_kings_courted_card_once_toward_its_cost` (new) |
| Peddler | Black Market, Toil, Crown, Destrier, Villa cases | not tested: Black Market not implemented; the others are the same `cost()` branch (Phase::Buy and cards in play) already exercised |
| Platinum / Colony | pile sizes, values | `platinum_is_a_five_coin_treasure_with_a_pile_of_twelve`, `colony_is_worth_ten_and_its_pile_is_8_with_two_players_else_12` |

### Alchemy (`alchemy_cards.rs`)
| Card | Ruling | Test |
|---|---|---|
| Transmute | Curse nothing; multi-type both; Inheritance Estate | `transmute_a_curse_gains_nothing`, `transmute_a_multi_type_card_gains_each`, `transmute_treats_an_inherited_estate_as_an_action_and_a_victory_card` (new) |
| Vineyard | 1 VP per 3 Actions, multi-type, Enlightenment | `vineyard_is_worth_one_vp_per_three_actions`, `xs::vineyard_counts_treasures...` (new) |
| Herbalist | once per play, cumulative; Capitalism self; Astrolabe; Spoils/Counterfeit; Capital | `herbalist_puts_a_treasure_from_play_on_the_deck`, `two_herbalists_but_only_one_treasure`, `throne_room_on_herbalist_puts_back_up_to_two_treasures`, `herbalist_can_put_itself_on_the_deck_under_capitalism` (new), `herbalist_cannot_put_an_astrolabe_that_stays_in_play_onto_the_deck`, `herbalist_cannot_put_a_spoils_or_a_counterfeited_treasure_on_the_deck` (new), `a_treasure_with_a_discard_from_play_effect_still_does_it_when_herbalist_topdecks_it` (new) |
| Apothecary | draw first; Coppers and Potions mandatory | `apothecary_takes_coppers_and_potions_and_orders_the_rest`, `apothecary_with_a_short_deck`, `apothecary_reshuffles_the_discard_pile_when_the_deck_runs_out` |
| Scrying Pool | each player, order, run-out shuffle, only revealed go to hand, multi-type | `scrying_pool_as_an_attack_the_owner_decides_for_each_player`, `scrying_pool_discarding_the_only_card_reshuffles_it_back_in_and_draws_it`, `scrying_pool_harem_is_not_an_action_so_it_stops_the_reveal` |
| University | optional; no Potion; no Debt; multi-type | `university_gain_can_be_declined`, `university_may_gain_an_action_up_to_five_without_a_potion`, `university_cannot_gain_an_action_with_debt_in_its_cost` (new) |
| Alchemist | start of Clean-up; any number; Potion must be in play; Herbalist after | `alchemist_goes_back_on_the_deck_with_a_potion_in_play`, `alchemist_cannot_go_back_without_a_potion_in_play`, `alchemist_resolves_before_herbalist_so_both_it_and_the_potion_go_back`, `alchemist_cannot_go_back_when_the_potion_left_play_before_cleanup` (new) |
| Alchemist | played without activating (Way, Enchantress) | not tested: the Alchemist offer reads the play record, which Ways/Enchantress set in their own audited sets |
| Familiar | Curses in turn order; none left | `familiar_gives_every_other_player_a_curse`, `familiar_with_too_few_curses_goes_leftmost_first` |
| Philosopher's Stone | deck + discard in fives; each play recounts | `philosophers_stone_counts_deck_and_discard_in_fives`, `philosophers_stone_counts_each_copy_and_every_play`, `philosophers_stone_played_by_crystal_ball_counts_again` (new) |
| Golem | two non-Golem Actions, either order, not in hand | `golem_basics`, `golem_plays_the_two_actions_in_the_order_chosen`, `golem_second_action_is_held_while_the_first_resolves`, `golem_never_plays_another_golem` |
| Apprentice | cost draws; Potion +2; Debt ignored | `apprentice_draws_per_coin_of_cost_and_two_more_for_a_potion`, `apprentice_ignores_the_debt_in_the_trashed_cards_cost` (new) |
| Potion | basic Treasure, $4, Potion | `potion_basics`, `potion_gives_a_potion_when_played` |

### Promos (`promo_cards.rs`)
| Card | Ruling | Test |
|---|---|---|
| Prince | set aside non-Duration non-Command Action up to $4; stays in play; discarded otherwise | `prince_sets_an_action_costing_up_to_four_aside_and_stays_in_play`, `a_prince_that_sets_nothing_aside_is_discarded_at_cleanup`, `prince_cannot_set_aside_a_command_card_that_cost_reduction_made_cheap` (new) |
| Prince | cost reduction; Potion; Debt | `cost_reduction_lets_prince_take_a_card_that_normally_costs_more`, `durations_and_potion_costs_can_never_be_set_aside`, `prince_cannot_set_aside_a_card_with_debt_in_its_cost` (new) |
| Prince | played each turn without using an Action; mandatory; Outposts; moves fail | `the_held_action_is_played_each_turn_without_entering_play_or_using_an_action`, `it_is_played_on_an_outposts_extra_turn_too`, `a_held_mining_village_cannot_trash_itself_and_gives_no_bonus`, `a_held_island_stays_but_still_puts_a_hand_card_on_the_mat` |
| Prince | Throne Room two cards; end of game | `throne_room_on_prince_sets_two_aside_and_both_stay_in_play`, `the_prince_and_its_card_count_for_gardens` |
| Prince | Piazza: same turn; Crown at start of turn | `a_prince_played_at_the_start_of_the_turn_plays_the_card_it_sets_aside_that_same_turn`, `a_held_crown_is_played_in_the_action_phase_at_the_start_of_the_turn` (new) |
| Prince | Necromancer + Throne Room + Prince; Possession | not testable: Possession is not implemented; the Necromancer case is the Throne Room case (`throne_room_on_prince...`) |
| Walled Village | topdeck with at most 1 other Action; two WVs; Durations; Treasury | `walled_village_with_one_other_action_may_go_but_not_with_two`, `two_walled_villages_alone_may_both_go_onto_the_deck`, `durations_still_in_play_count_as_actions_in_play`, `walled_village_is_checked_before_treasury_may_leave_play`, `a_topdecked_alchemist_makes_room_for_walled_village`, `walled_village_counts_the_treasures_in_play_as_actions_under_enlightenment` (new) |
| Walled Village | Way / Enchantress still topdeck | not tested: the offer is independent of how it was played (it reads cards in play only) |
| Envoy | reveal 5, left player chooses, shuffle, fewer | `envoy_reveals_five_and_the_left_player_chooses_the_one_to_discard`, `envoy_with_fewer_than_five_cards_reveals_what_there_is`, `envoy_shuffles_the_discard_pile_in_to_reveal_the_rest` |
| Captain | now and next turn; leaves it there; visible non-Duration non-Command up to $4 | `captain_plays_a_card_from_the_supply_and_leaves_it_there`, `captain_stays_in_play_and_plays_another_card_at_the_start_of_the_next_turn`, `captain_only_offers_visible_non_duration_non_command_actions_up_to_four`, `captain_cannot_play_a_command_card_that_cost_reduction_made_cheap` (new) |
| Captain | moves fail; Workshop gains itself; Throne Room + Caravan | `a_card_captain_plays_cannot_move_itself`, `captain_can_play_a_workshop_that_gains_the_workshop_itself` (new), `captain_playing_throne_room_lets_a_duration_stay_longer`, `captain_with_nothing_to_play_still_stays_in_play` |
| Sauna / Avanto | free play only right after; alternate; Silver trashes per Sauna played, per Silver play | `adventures_cards.rs`: `sauna_draws_gives_an_action_and_may_play_an_avanto_for_free`, `sauna_and_avanto_can_chain_while_each_has_the_other_in_hand`, `a_played_sauna_lets_each_silver_played_trash_a_card_from_the_hand`, `two_saunas_trash_twice_per_silver`; `promo_cards`: `a_sauna_cannot_play_a_sauna_for_free_and_an_avanto_only_follows_the_sauna`, `a_sauna_trashes_a_card_each_time_a_crowned_silver_is_played` (new) |
| Governor | +1 Action; three options for all, turn order; exactly $1/$2 more | `governor_draws_three_for_you_and_one_for_each_other_player`, `governor_gains_a_gold_for_you_and_a_silver_for_each_other_player_in_turn_order`, `governor_with_the_last_silver_only_the_first_player_in_turn_order_gets_it`, `governor_remodel_you_gain_exactly_two_more_others_exactly_one_more_and_they_decide_in_turn_order`, `governor_cannot_trash_itself_but_can_trash_another_copy` |
| Governor | Way of the Chameleon; another player's Governor on your turn | not testable: needs Possession-like play on another's turn; Chameleon's +Cards substitution is Menagerie's audited mechanism |
| Dismantle | mandatory trash; $0 nothing; cheaper then Gold; Potion/Debt-only; Fortune | `dismantle_trashes_a_card_and_gains_a_cheaper_card_then_a_gold`, `dismantle_on_a_copper_or_an_empty_hand_does_nothing_more`, `dismantle_still_gains_a_gold_with_nothing_cheaper_and_a_cheaper_card_with_no_gold`, `dismantle_potion_and_debt_costs`, `dismantle_a_fortune_gains_a_province_or_an_overlord_and_a_gold` |
| Church | 0-3 face down; trash optional; multiple batches, chosen order | `church_sets_aside_up_to_three_cards_face_down_and_stays_in_play`, `church_puts_the_cards_into_the_hand_next_turn_and_then_may_trash_one`, `church_with_nothing_set_aside_may_still_trash_next_turn`, `two_church_plays_are_two_batches_each_followed_by_its_own_trash`, `the_player_chooses_which_church_batch_goes_into_the_hand_first` |
| Marchland | counts itself, rounds down; +Buy, discard for $; gained into hand; 8/12 copies | `marchland_is_worth_a_vp_per_three_victory_cards_counting_itself_rounded_down`, `buying_marchland_gives_a_buy_and_a_dollar_per_card_discarded`, `marchland_zero_discards_and_discarding_the_whole_hand`, `marchland_gained_into_the_hand_can_be_one_of_the_cards_discarded`, `marchland_has_eight_copies_with_two_players_and_twelve_with_more` (new) |
| Summon | gain Action up to $4, set aside, play next turn without an Action; loses track | `summon_is_bought_for_five_and_a_buy_to_gain_an_action_up_to_four_and_set_it_aside`, `the_summoned_card_is_played_at_the_start_of_the_next_turn_without_using_an_action`, `summon_loses_track_of_a_card_that_moved_after_it_was_gained`, `summon_loses_track_of_a_card_a_watchtower_trashes` |
| Summon | Hireling draws; Possession; Nomad Camp; Experiment + Gatekeeper | `a_summoned_hireling_draws_when_played_at_the_start_of_the_turn` (new); Possession and first-edition Nomad Camp not implemented; the Experiment/Gatekeeper case is the "moved away and back" rule, covered by the same token logic as `summon_loses_track_of_a_card_that_moved_after_it_was_gained` |

### Cornucopia & Guilds (`cornucopia_guilds_cards.rs`)
| Card | Ruling | Test |
|---|---|---|
| Candlestick Maker, Baker | printed effects; Baker start Coffers | `candlestick_maker_gives_action_buy_and_a_coffers_token`, `baker_gives_a_card_an_action_and_a_token_and_everyone_starts_with_one` |
| Hamlet | each discard optional, no multiples | `hamlet_draws_then_offers_two_independent_discards`, `hamlet_bonuses_are_each_optional_and_only_for_a_discard`, `hamlet_with_one_card_discards_it_for_one_bonus_and_never_twice` |
| Farrier | overpay +Cards at end of turn; not without paying | `farrier_overpay_draws_extra_cards_at_cleanup`, `overpay_is_not_available_when_gaining_without_buying` |
| Stonemason | trash mandatory; two cheaper, one at a time; overpay Actions exactly; Potion; Wayfarer; one copy left | `stonemason_trashes_a_card_and_gains_two_cheaper_cards_one_at_a_time`, `stonemason_trash_is_mandatory_and_a_cheaper_card_may_not_exist`, `stonemason_overpay_gains_two_actions_costing_exactly_the_amount`, `stonemason_overpay_with_a_potion_gains_actions_costing_a_potion`, `stonemason_is_gained_before_the_overpay_cards_so_a_wayfarer_costs_what_it_does` (new), `stonemason_may_gain_cheaper_cards_of_any_type_and_overpays_with_one_copy_left` |
| Stonemason | overpay not when gained; no Debt overpay; Destrier | `overpay_is_not_available_when_gaining_without_buying`, `overpay_potions_are_only_offered_for_stonemason`; Destrier: not tested (the first gain changes its cost; same exactness check) |
| Menagerie | all different draws 3 else 1; empty hand 3 | `menagerie_draws_three_when_every_name_is_different`, `menagerie_draws_one_card_with_a_duplicate_name`, `menagerie_with_an_empty_hand_draws_three_and_reshuffles_mid_draw` |
| Shop | Action not in play; played copy that left | `shop_cannot_play_an_action_with_a_copy_in_play`, `shop_can_play_an_action_whose_copy_was_played_earlier_but_is_no_longer_in_play` (new) |
| Infirmary | overpay plays once per $; Way | `infirmary_overpay_plays_it_once_per_dollar_from_where_it_lies`, `infirmary_gained_without_buying_is_not_played`; Way: not tested (Way choice on a discard-pile play is Menagerie's mechanism) |
| Advisor | top 3, left player picks, fewer | `advisor_reveals_three_and_the_left_player_chooses_the_one_to_discard`, `advisor_with_fewer_than_three_cards_still_discards_one` |
| Farmhands | optional, mandatory play, no Action, face up | `gaining_farmhands_may_set_aside_an_action_or_treasure_to_play_next_turn`, `the_set_aside_card_is_played_at_the_start_of_the_next_turn_without_an_action`, `a_set_aside_treasure_gives_its_coins_and_declining_sets_nothing_aside` |
| Plaza | draw first, may discard the drawn Treasure | `plaza_draws_then_discards_a_treasure_for_a_coffers_token`, `plaza_gives_nothing_for_declining_or_without_a_treasure` |
| Remake | trash then gain, twice; one card; none at cost; Fortress twice | `remake_trashes_and_gains_one_card_at_a_time`, `remake_with_one_card_or_no_exact_cost_card_gains_nothing_for_it`, `remake_can_trash_the_same_fortress_twice` (new) |
| Young Witch | draw, discard 2, Curse unless Bane/Moat; turn order; Bane pile in the end condition | `young_witch_draws_two_discards_two_and_curses_the_others`, `young_witch_is_blocked_by_revealing_a_bane_or_a_moat_and_the_bane_is_an_extra_pile`, `young_witch_curses_in_turn_order_and_the_bane_counts_as_a_pile_for_the_end` |
| Young Witch | split-pile Bane; Bane set-up; Reaction Bane; Black Market; Tax Debt on Bane | `every_card_of_a_split_pile_bane_blocks_young_witch`, `a_bane_that_needs_setup_brings_it`, `a_reaction_bane_that_leaves_the_hand_cannot_both_react_and_block` (all new); Black Market not implemented; Tax Debt on the Bane: not tested (Empires set-up reads the kingdom, see open issues) |
| Herald | play top Action free; overpay topdecks from discard, can be the Herald | `herald_plays_an_action_revealed_from_the_deck_without_using_an_action`, `herald_playing_a_duration_is_itself_discarded_normally`, `herald_overpay_puts_cards_from_the_discard_pile_onto_the_deck`, `herald_overpay_with_few_cards_in_the_discard_moves_what_there_is` |
| Butcher | +2 Coffers, may trash and spend any number; trash Butcher copy; Silk Merchant | `butcher_gives_two_tokens_then_trashes_and_gains_spending_only_what_it_needs`, `butcher_spends_nothing_for_a_card_that_costs_no_more_than_the_trashed_one`, `butcher_can_use_older_tokens_and_the_trash_is_optional`, `butcher_can_spend_the_coffers_a_trashed_silk_merchant_gave` (new) |
| Carnival | one of each name from top 4 | `carnival_keeps_one_of_each_name_from_the_top_four` |
| Ferryman | gain also pile card; pile cannot be gained otherwise; from trash; split pile | `gaining_a_ferryman_also_gains_a_copy_of_its_pile_card`, `the_ferryman_pile_cannot_be_bought_and_runs_out_without_ending_the_game`, `a_ferryman_gained_from_the_trash_also_gains_a_card_of_its_pile` (new); split-pile Ferryman card and Black Market: not tested (the pile card is a single named card in this engine) |
| Footpad | +2 Coffers, discard to 3; draw on any gain in an Action phase | `footpad_gives_two_tokens_and_makes_others_discard_down_to_three`, `footpad_draws_for_any_player_gaining_a_card_in_an_action_phase_in_games_using_it` |
| Horn of Plenty | names in play incl. itself; Victory trashes it; other gains do not | `horn_of_plenty_gains_a_card_costing_up_to_the_names_in_play`, `horn_of_plenty_gaining_a_victory_card_trashes_it`, `horn_of_plenty_victory_gained_other_ways_does_not_trash_it_and_its_gain_counts_for_the_guild` |
| Hunting Party | reveal until not a copy; shuffle; none | `hunting_party_takes_the_first_card_that_is_not_a_copy_of_one_in_hand`, `hunting_party_finding_nothing_discards_everything_revealed_after_a_reshuffle` |
| Jester | discard top; Victory Curse else copy to chooser's choice; no copies; not "revealed" (Patron); Tunnel | `jester_gives_two_coins_and_a_victory_card_discarded_means_a_curse`, `jester_lets_the_attacker_choose_who_gains_a_copy_of_a_non_victory_card`, `jester_edge_cases_empty_pile_empty_deck_moat_and_dual_types`, `a_patron_a_jester_makes_its_victim_discard_gives_no_coffers` (new); Tunnel's optional reveal: not tested here (Hinterlands' discard reaction, same prompt as for any discard) |
| Journeyman | name any card; three not-named; shuffle | `journeyman_names_a_card_and_draws_three_cards_without_that_name`, `journeyman_can_name_a_card_that_is_not_in_the_deck`, `journeyman_stops_when_the_deck_runs_out_and_reshuffles_the_discard` |
| Joust | set aside Province for any Reward to hand; discard Province in Clean-up; Reward counts 1 / 2; none left | `joust_sets_aside_a_province_for_a_reward_in_hand`, `joust_province_goes_to_the_discard_pile_in_cleanup_and_is_optional`, `joust_with_no_rewards_left_still_sets_the_province_aside`, `rewards_are_not_in_the_supply_cannot_be_bought_or_gained_by_other_cards`, `joust_sets_up_one_of_each_reward_with_two_players_and_two_with_more` (new) |
| Merchant Guild | Coffers per card gained in Buy phase, cumulative; not Action phase | `merchant_guild_pays_a_token_per_card_gained_in_the_buy_phase`, `merchant_guild_is_cumulative_and_ignores_gains_outside_the_buy_phase` |
| Merchant Guild | Villa multiple Buy-phase ends, Wine Merchant order | not tested: Buy-phase-end repetition is Empires' (Villa) audited mechanism; the Tavern call order is Adventures' |
| Soothsayer | Gold; Curse then draw per player in order; Watchtower trash still draws | `soothsayer_gains_a_gold_and_gives_curses_that_draw_a_card`, `soothsayer_deals_curses_in_turn_order_each_player_drawing_before_the_next_gains`, `soothsayer_curse_trashed_by_watchtower_still_draws` |
| Soothsayer | Sheepdog / Horse Traders drawn too late; Trader | not testable: Horse Traders was removed in 2nd edition; the reaction window precedes the gain (same test as turn order) |
| Fairgrounds | 2 VP per 5 names; 8/12; Shelters, Ruins, Knights differ | `fairgrounds_is_worth_two_vp_per_five_different_names`, `a_fairgrounds_game_has_no_other_surprises_in_supply_sizes`, `fairgrounds_counts_differently_named_shelters_ruins_and_knights_as_different_cards` (new) |
| Coronet | Action then Treasure, each optional, never a Reward; no Action use; Buy-phase Action | `coronet_plays_an_action_twice_then_a_treasure_twice_but_never_a_reward`, `coronet_in_the_buy_phase_is_a_treasure_and_both_parts_are_optional` |
| Courser | choose two different, in order | `courser_chooses_two_different_options_and_draws_before_the_silvers` |
| Demesne | +2 Actions +2 Buys, Gold; VP per Gold | `demesne_gives_actions_buys_a_gold_and_is_worth_a_vp_per_gold` |
| Housecarl | per differently named Action in play incl. itself | `housecarl_draws_a_card_per_differently_named_action_in_play`, `xs::housecarl_counts_the_treasures_in_play_as_actions_under_enlightenment` (new) |
| Huge Turnip | +2 Coffers, $ per token including them | `huge_turnip_gives_two_tokens_and_a_coin_per_token_including_the_new_ones` |
| Renown | +1 Buy, costs -$2 everywhere, stacks, floor 0 | `renown_gives_a_buy_and_reduces_every_cost_by_two_and_stacks`, `renown_lets_remake_turn_a_copper_into_a_silver` (new) |

## Counts
- Cards checked: 27 Prosperity (incl. Platinum, Colony), 12 Alchemy (incl. Potion), 12 Promo (incl. Avanto, Summon), 32 Cornucopia & Guilds (26 kingdom + 6 Rewards) = 83.
- Bugs found: 11 rows (the Enlightenment row spans eight cards). Fixed: all 11. Left open: 0 bugs; 3 notes under Open issues (no failing test is parked).
- New tests: about 110 (engine) plus the fuzz smoke test.
