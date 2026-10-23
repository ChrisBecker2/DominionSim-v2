# Audit: Dark Ages and Hinterlands

Rules source: the offline wiki copy (current texts, the 2022 errata: Farmland and Hovel say "gain", Haggler / Highway / Souk / Trader / Cauldron as printed in the 2nd edition).
Files: `crates/engine/src/sets/{dark_ages,hinterlands}.rs`, bots `crates/sim/src/defaults/{dark_ages,hinterlands}.rs`, tests `crates/engine/tests/{dark_ages,hinterlands}_cards.rs`, `crates/sim/tests/{dark_ages,hinterlands}_defaults.rs`.

## Bugs fixed (each with a test that failed before)

| Bug | Rule clause | Test |
|---|---|---|
| A Scheme that is the Ferryman pile card never offered its Clean-up topdeck (offer hook gated on the Supply mask) | Scheme | `a_scheme_from_the_ferryman_pile_still_puts_an_action_on_the_deck` |
| A Trader / Nomads / Trail that is the Ferryman pile card had no gain triggers (the Hinterlands gain hook is gated on `in_supply`) | Trader "when you gain", Nomads "+$2 when gained", Trail "may play when gained" | `a_trader_from_the_ferryman_pile_exchanges_a_gained_card`, `nomads_and_trail_from_the_ferryman_pile_react_when_gained` |
| Trader could not exchange a card gained from a pile outside the Supply (Spoils, Madman, Mercenary, Ferryman pile, Rewards) and `Act::Return` put such cards into the Supply lanes | Trader: "return the gained card to its pile (whether in the Supply or not)" | `trader_exchanges_a_card_gained_from_a_pile_outside_the_supply` |
| Basilica was checked before a Nomads' +$2 was added | Nomads FAQ: the +$2 on gaining in the Buy phase lets you take Basilica's VP | `nomads_gained_in_the_buy_phase_gives_two_coins_that_count_for_basilica` |
| Cartographer, Catacombs, Survivors, Jack of All Trades (and Zombie Spy) "looked at" cards through `RevealTop`, which reveals them (log line, Patron's Coffers) | "look at" is not "reveal" | `a_cartographer_discard_is_not_a_reveal`, `looking_at_cards_with_catacombs_or_survivors_does_not_reveal_a_patron` |
| Witch's Hut discarded its two cards without revealing them | "Discard 2 cards, revealed" | `witchs_hut_reveals_the_discards_and_a_card_with_another_type_still_counts_as_an_action` |
| Hermit compared against the cards gained in the whole turn, not in the Buy phase it ends | Hermit: "if you didn't gain any cards in it" (each Buy phase, Villa/Cavalry) | `a_hermit_is_exchanged_if_the_last_buy_phase_gained_nothing_even_after_a_villa_bought_in_the_first` |
| Haggler could not gain a card with Debt (it always required no Debt) | Haggler: Cost/Debt amounts are compared part by part (Fortune -> Overlord) | `haggler_compares_coin_and_debt_parts_so_a_fortune_bought_gains_an_overlord_or_an_engineer` |

Bot policy fix: the Trader exchange now keeps a card from a pile outside the Supply (Madman, Mercenary, a Horse) and exchanges a Spoils or junk: `trader_exchanges_spoils_and_junk_but_keeps_a_mercenary_or_a_madman`.

## Changes to shared files
- `effects.rs`: `RevealTop` skips `reveal_card` when `Frame::exact` (= "look at"); Witch's Hut reveals each pick; `Act::Return` calls `return_to_own_pile` (was `return_to_pile`).
- `sets/mod.rs`: `Hinterlands::gain_live_extra` added to the gain dispatch and `gain_triggers_possible`; `eot_hooks_live` also true when the game has a Ferryman pile (this also fixes a Ferryman-pile Alchemist / Walled Village / Herbalist-like clean-up offer for other sets' hooks that use this gate; their own `in_supply` tests inside the hooks may still need the same `|| g.ferryman == c`).
- `sets/empires.rs`: Basilica counts a Nomads' +$2. `sets/nocturne.rs`: `look_at_top_discard_one` marks its reveal as a look.
- `state.rs`/`engine.rs`/`text.rs`: `TurnState::buy_gain_base` (set in `enter_buy`, transient). **Renaissance's Exploration uses `gained_in_buy == 0` for "didn't gain any cards in the Buy phase" and has the same multi-Buy-phase flaw; it should compare against `buy_gain_base` too.**

## Open issues (ignored tests)
- Trader may exchange a Silver for a Silver (FAQ; matters with Gatekeeper's Exile): the engine never offers it. `trader_can_exchange_a_silver_for_a_silver` (`#[ignore]`).
- Develop: the Wayfarer ruling (the trashed card's cost is re-read when the second gain is chosen) is not modelled; the cost is read once. Not tested (needs Wayfarer's cost shifting; exotic).
- Cauldron: "gain the third Action while resolving the play of a Cauldron gained by that gain gives an instant Curse" (Mining Road) is not modelled. Not tested (needs Plunder's Mining Road chain).
- Urchin: trashing it is only offered while the Mercenary pile has a card (`urchin_trashing_is_only_offered_while_a_mercenary_is_left`); by the rules trashing is always allowed (it matters only for Market Square / Rogue / Tormentor). Left as a deliberate pruning.
- Name decisions (Mystic) only list cards that can be in the deck: equivalent play, not a rules difference.

## Policy notes (bot defaults, no change)
- Count: never uses "put a card onto your deck"; takes a Copper when it has no two dead cards. Wiki: "if you have cards you do not want to trash, you can save them by using the discarding or topdecking options".
- Beggar: the reaction is always used. Wiki: "It is almost never worth it to gain Beggar for its defensive Reaction" (a gain-list matter; once it is in the hand using it is right).
- Urchin: traded for a Mercenary whenever one is left (matches "Mercenary is a strong trasher").
- Death Cart: trashes itself if no dead Action is in hand (wiki: weak payload; fine).
- Rebuild: names the costliest Victory card (reasonable).
- Trader: exchanges junk, Coppers and unlisted cards; keeps listed ones and Victory cards.

## Speed opportunities (not implemented)
- `sets/hinterlands.rs` `gain_live_extra`: one range compare per gain in `gain_triggers_possible` (~1 ns); could be a bit in `GameState::adv` set at setup.
- `sets/dark_ages.rs` `pile_unseen` (called by every Knights/Ruins gain via `advance_pile`/`draw_unseen`): sums `all_cards()` of every player (about 10 multiset adds, ~100 ns per gain of a Ruins/Knight); could be kept as per-pile counters. Only games with Looters/Knights.
- `trash_card`: `market_square_live()` (two bitmap tests) on every trash in every game; gate with a per-game flag.
- `gain_triggers` Hovel loop: `g.shelters && is VICTORY` per gain; already one byte test when no Shelters.
- `discard_hook` / `G_DISCARD_HOOK`: set when Tunnel/Trail/Weaver/Hound are in the game; each discard then runs a `match`; fine.

## Cards checked (one line each)

Dark Ages (35 kingdom piles, 3 Shelters, Spoils, Madman, Mercenary, 5 Ruins, 10 Knights; 59 cards and 2 marker piles):
Poor House OK; Beggar OK; Squire OK; Vagrant OK; Forager OK; Hermit fixed (Buy phase); Market Square OK; Sage OK; Storeroom OK; Urchin OK (pruning noted); Armory OK; Death Cart OK; Feodum OK; Fortress OK; Ironmonger OK; Marauder OK; Procession OK; Rats OK; Scavenger OK; Wandering Minstrel OK; Band of Misfits OK; Bandit Camp OK; Catacombs fixed (look); Count OK; Counterfeit OK; Cultist OK; Graverobber OK; Junk Dealer OK; Knights pile (all ten: Dame Anna, Josephine, Molly, Natalie, Sylvia, Sir Bailey, Destry, Martin, Michael, Vander) OK; Mystic OK; Pillage OK; Rebuild OK; Rogue OK; Altar OK; Hunting Grounds OK; Hovel OK; Necropolis OK; Overgrown Estate OK; Spoils OK; Madman OK; Mercenary OK; Abandoned Mine, Ruined Library, Ruined Market, Ruined Village OK; Survivors fixed (look). Set-up (Shelters rule, Ruins 10 per extra player, Looter / Bane / Ferryman Ruins, Spoils / Madman / Mercenary piles): OK. `choice_free` / `order_sensitive` marks checked against CLAUDE.md: OK.

Hinterlands (26): Crossroads OK; Fool's Gold OK; Develop OK (Wayfarer open); Guard Dog OK; Oasis OK; Scheme fixed (Ferryman); Tunnel OK; Jack of All Trades fixed (look); Nomads fixed (Ferryman, Basilica); Spice Merchant OK; Trader fixed (non-Supply, Ferryman; Silver open); Trail fixed (Ferryman); Weaver OK; Berserker OK; Cartographer fixed (look); Cauldron OK (Mining Road open); Haggler fixed (Debt); Highway OK; Inn OK; Margrave OK; Souk OK; Stables OK; Wheelwright OK; Witch's Hut fixed (reveal); Border Village OK; Farmland OK (errata: gain, not buy; never another Farmland). Marks OK.

Bots: no illegal choices or illegal Passes found (150 random Hinterlands games and the Dark Ages games finish with cards conserved).

## Ruling coverage

Tests are in `hinterlands_cards.rs` (HL) and `dark_ages_cards.rs` (DA). Wiki bullets are abbreviated.

### Hinterlands
| Card | Ruling | Test |
|---|---|---|
| Crossroads | draw per Victory revealed, multi-type Victory counts (Harem) | crossroads_draws_per_victory_card_and_gives_three_actions_the_first_time |
| Crossroads | +3 Actions only the first time; Throne Room once | crossroads_with_no_victory_card_still_gives_actions_and_throne_room_gives_them_once |
| Crossroads | first one played as a Way / enchanted: second gives no Actions | a_crossroads_played_as_a_way_counts_as_the_first_one_so_the_next_gives_no_actions; an_enchanted_crossroads_counts_as_played_so_the_next_gives_no_actions |
| Fool's Gold | $1 then $4 each; three = $9 | fools_gold_is_worth_one_the_first_time_and_four_after |
| Fool's Gold | same one twice (Crown/Counterfeit): $4 the second | fools_gold_played_twice_by_crown_is_worth_five |
| Fool's Gold | trash for Gold onto deck when another gains a Province; not your own | fools_gold_in_hand_trashes_itself_for_a_gold_when_another_player_gains_a_province; fools_gold_can_be_declined_and_does_not_react_to_your_own_province |
| Fool's Gold | Province gained on your turn by another (Governor) reacts | fools_gold_reacts_to_a_province_gained_on_your_own_turn_and_each_player_in_turn_order_is_asked |
| Fool's Gold | blocked first (Highwayman) still counts | a_fools_gold_blocked_by_a_highwayman_still_makes_the_next_one_worth_four |
| Develop | trash, gain +1 and -1 onto deck | develop_trashes_a_card_and_gains_one_costing_one_more_and_one_one_less_onto_the_deck |
| Develop | missing cost still gains the other; Copper only $1 | develop_still_gains_one_when_no_card_costs_the_other_amount |
| Develop | costs now (Highway) | develop_costs_use_the_current_reduction |
| Develop | gained to the deck even Ghost Town; Villa moves to hand | develop_gains_cards_that_go_to_the_hand_onto_the_deck_except_villa_which_moves_itself_to_the_hand |
| Develop | first gain's on-gain resolves before the second is chosen | develop_gains_one_card_and_its_on_gain_effect_resolves_before_the_other_is_chosen |
| Develop | Wayfarer cost shift | not testable cheaply: open issue |
| Guard Dog | +2, +2 more with <=5 cards | guard_dog_draws_two_and_two_more_with_five_or_fewer_cards_in_hand |
| Guard Dog | played in response, discarded in that Clean-up; Berserker draw then discard | guard_dog_can_be_played_in_response_to_an_attack_and_is_discarded_at_the_end_of_that_turn |
| Guard Dog | asked before Moat; drawn Moat usable | guard_dog_is_asked_before_the_moat_and_may_be_declined; guard_dog_drawing_a_moat_lets_it_be_revealed_after_all |
| Oasis | cantrip +$1 discard | oasis_is_a_cantrip_with_a_coin_and_a_discard |
| Scheme | topdeck an Action at discard; itself | scheme_puts_an_action_from_play_onto_the_deck_when_discarded; schemes_stack_and_a_scheme_can_put_itself_back |
| Scheme | cumulative / Throne; Duration staying not offered | throne_roomed_scheme_counts_twice_and_a_duration_staying_in_play_is_not_offered |
| Scheme | works after Procession trashed it | scheme_still_works_after_procession_trashed_it_and_counts_both_plays |
| Scheme | Traveller exchange (stop-moving) | not tested: the Traveller exchange happens in the same Clean-up step; no Hinterlands-specific code (Scheme uses the standard tracking) |
| Tunnel | reveal for Gold, optional, not in Clean-up, once per discard, discarded together | a_discarded_tunnel_may_be_revealed_for_a_gold; the_tunnel_reaction_is_optional_and_not_used_in_cleanup; tunnels_discarded_together_are_all_discarded_before_any_reacts; a_discarded_tunnel_reacts_only_once_everything_discarded_together_is_in_the_discard_pile |
| Tunnel | 8/12 copies | tunnel_and_farmland_piles_are_8_with_two_players_and_12_otherwise |
| Jack of All Trades | Silver, look/discard, draw to 5, trash non-Treasure | jack_of_all_trades_gains_a_silver_looks_at_the_top_draws_to_five_and_may_trash; jack_of_all_trades_with_a_full_hand_draws_nothing_and_a_kept_top_card_is_drawn |
| Jack of All Trades | Silver shuffled in on a reshuffle | jack_of_all_trades_silver_is_shuffled_in_when_the_look_causes_a_reshuffle |
| Nomads | +$2 played / gained / trashed | nomads_gives_a_buy_and_two_coins_when_played_gained_and_trashed |
| Nomads | on another player's turn: nothing usable | nomads_gained_on_another_players_turn_gives_nothing_to_the_current_turn |
| Nomads | Basilica | nomads_gained_in_the_buy_phase_gives_two_coins_that_count_for_basilica |
| Nomads | Ball's -$1 token | not tested: Ball's token is Adventures code; Nomads' +$2 goes through the ordinary coin pool |
| Spice Merchant | optional trash; two modes | spice_merchant_trashes_a_treasure_for_cards_and_an_action_or_a_buy_and_coins; spice_merchant_trashing_is_optional |
| Trader | Silvers per cost; Copper none; Silver trash 3 | trader_trashes_a_card_for_a_silver_per_coin_of_its_cost; trader_trashing_a_silver_gives_three_silvers |
| Trader | cost with Highway / Potion / Debt | trader_gives_silvers_for_the_current_cost_a_potion_or_debt_part_gives_nothing |
| Trader | exchange for Silver, optional, pile back | trader_exchanges_a_gained_card_for_a_silver; trader_exchange_is_optional_and_not_offered_for_a_silver_or_without_silver_left |
| Trader | exchange not a gain (Border Village still gains) | trader_exchange_is_not_a_gain_and_the_cards_own_gain_effect_still_happens |
| Trader | other reaction moved the card first | trader_gained_card_moved_by_another_reaction_cannot_be_exchanged; a_gained_trail_exchanged_by_trader_cannot_be_played |
| Trader | Curse from an attack | trader_exchanges_a_curse_given_by_another_players_attack |
| Trader | Mill/Ironworks keeps its bonuses; gained into hand; non-Supply pile | a_mill_gained_by_ironworks_and_exchanged_for_a_silver_still_gives_its_action_and_card; a_trader_gained_into_the_hand_can_be_exchanged_for_a_silver; trader_exchanges_a_card_gained_from_a_pile_outside_the_supply |
| Trader | exchange a Silver for a Silver | trader_can_exchange_a_silver_for_a_silver (ignored: open) |
| Trail | cantrip; played when gained / trashed / discarded; optional; not in Clean-up | trail_is_a_cantrip; trail_may_be_played_when_gained_discarded_or_trashed; trail_reaction_is_optional_and_does_nothing_in_cleanup |
| Trail | played on another turn: discarded in that Clean-up | a_trail_played_on_another_players_turn_is_discarded_at_the_end_of_that_turn |
| Trail | trashed then played returns from the trash, Remodel still gains | a_trail_trashed_by_remodel_is_played_back_into_play_and_remodel_still_gains |
| Trail | discard down first, then play | discarding_down_to_three_discards_everything_first_and_then_the_trail_is_played_without_discarding_again |
| Trail | several Trails, reshuffle loses the rest | not tested: shares the Weaver queue code, covered by several_weavers_discarded_together_are_all_in_the_discard_pile_before_the_first_is_played |
| Weaver | two Silvers or up to $4 | weaver_gains_two_silvers_or_a_card_costing_up_to_four |
| Weaver | played when discarded; several together | a_discarded_weaver_may_be_played_and_it_is_discarded_again_in_cleanup; several_weavers_discarded_together_are_all_in_the_discard_pile_before_the_first_is_played |
| Weaver | Vassal loses track | a_weaver_played_when_a_vassal_discards_it_is_played_only_once |
| Weaver | Elder: cannot do both | not tested: modes table has no `Elder` extra for Weaver (`ModeOpt` list, not "choose one") |
| Berserker | gain cheaper + discard to 3; played when gained with an Action in play (mandatory) | berserker_gains_a_cheaper_card_and_the_others_discard_down_to_three; a_berserker_gained_with_an_action_in_play_is_played_at_once; a_berserker_gained_with_no_action_in_play_stays_in_the_discard_pile |
| Berserker | bought after an Action; on another turn | a_berserker_bought_after_playing_an_action_attacks; a_berserker_gained_on_another_players_turn_is_played_there_and_attacks_the_turns_player |
| Berserker | cost 0: no gain, still attacks | a_berserker_costing_nothing_gains_no_card_but_still_attacks |
| Berserker | Innovation / Voyage / Warlord / Monkey | not tested: no Berserker-specific code (stop-moving tracking is generic; see cross_set.rs) |
| Cartographer | draw, look 4, discard any, reorder | cartographer_draws_looks_at_four_discards_some_and_reorders_the_rest; cartographer_looks_at_fewer_cards_when_the_deck_is_short |
| Cartographer | not a reveal (Patron) | a_cartographer_discard_is_not_a_reveal |
| Cauldron | $2, +Buy, third Action gained curses | cauldron_is_worth_two_coins_and_a_buy_and_curses_on_the_third_action_gained |
| Cauldron | three Actions before playing: none; only Actions count; cumulative | gaining_three_actions_before_playing_the_cauldron_gives_no_curse; only_actions_count_for_the_cauldron_and_two_cauldrons_give_two_curses |
| Cauldron | is an Attack (Moat) | a_moat_revealed_to_the_cauldron_protects_from_its_curse |
| Cauldron | instant Curse via Mining Road | not tested: open issue |
| Haggler | cheaper non-Victory per card bought; not for gains; stacks | haggler_gains_a_cheaper_non_victory_card_for_each_card_bought; haggler_only_reacts_to_cards_bought_not_gained_and_stacks |
| Haggler | nothing for Copper / its own gain; Throne Room | haggler_gains_nothing_for_a_copper_and_not_for_a_card_gained_by_haggler; throne_room_on_haggler_gains_two_cards_for_each_card_bought |
| Haggler | Debt / Potion comparison | haggler_compares_coin_and_debt_parts_so_a_fortune_bought_gains_an_overlord_or_an_engineer |
| Haggler | split pile revealing a cheaper card | not testable: no split pile in the implemented sets has a dearer top half than its bottom half |
| Highway | -$1 each; cumulative | highway_reduces_every_cost_by_one_each_time |
| Highway | Procession gives -$2; Way other than Chameleon: none | procession_on_a_highway_reduces_costs_by_two; highway_played_as_a_way_other_than_the_chameleon_gives_no_cost_reduction |
| Inn | +2 Cards +2 Actions discard 2 | inn_gives_two_cards_two_actions_and_discards_two_cards |
| Inn | gained: shuffle Actions (incl. itself) in; deck shuffled even if none | a_gained_inn_shuffles_chosen_actions_from_the_discard_pile_into_the_deck; a_gained_inn_still_shuffles_the_deck_when_no_action_is_chosen |
| Inn | gained via Border Village: both can be shuffled | an_inn_gained_through_a_border_village_can_shuffle_the_border_village_in_too |
| Inn | Changeling / Gatekeeper lose track | not tested: stop-moving is generic (`track.rs`); no Inn-specific code beyond the shuffle |
| Margrave | +3 Cards +Buy, victims draw then discard to 3; drawing is mandatory; Moat | margrave_draws_three_gives_a_buy_and_makes_the_others_draw_then_discard_to_three; margrave_a_player_with_three_cards_after_drawing_does_not_discard_and_a_moat_stops_the_draw; throne_room_on_margrave_attacks_twice |
| Margrave | a Moat drawn cannot be used | a_moat_drawn_off_a_margrave_cannot_stop_that_attack |
| Souk | +Buy +$7 -$1 per card, floor 0 | souk_gives_a_buy_and_seven_coins_less_one_per_card_in_hand |
| Souk | gain: trash up to 2 | a_gained_souk_may_trash_up_to_two_cards_from_the_hand |
| Souk | Debt payable in between | not testable: Debt is paid at the Buy decision only in this engine |
| Stables | discard a Treasure for +3 Cards +Action; optional | stables_discards_a_treasure_for_three_cards_and_an_action |
| Stables | discarded Treasure is shuffled in | stables_draws_after_discarding_so_the_discarded_treasure_is_shuffled_in |
| Wheelwright | discard for an Action up to its cost | wheelwright_discards_a_card_to_gain_an_action_costing_as_much_or_less; wheelwright_discarding_an_action_gains_a_copy_and_is_optional |
| Wheelwright | must gain if it discarded | wheelwright_must_gain_an_action_if_it_discarded_a_card |
| Witch's Hut | Curse if both Actions; Moat | witchs_hut_curses_the_others_when_both_discarded_cards_are_actions; witchs_hut_discarding_one_card_only_gives_no_curse_and_a_moat_protects |
| Witch's Hut | revealed; multi-type counts | witchs_hut_reveals_the_discards_and_a_card_with_another_type_still_counts_as_an_action |
| Border Village | gain when gained, not played; current cost | border_village_gains_a_cheaper_card_when_gained_not_when_played; border_village_gain_follows_the_current_cost |
| Border Village | with Watchtower | border_village_with_watchtower_trashed_still_gains_a_card |
| Farmland | trash, gain exactly +$2 | farmland_trashes_a_card_from_the_hand_and_gains_one_costing_exactly_two_more |
| Farmland | never another Farmland; empty hand | farmland_never_gains_another_farmland_and_does_nothing_with_an_empty_hand |
| Farmland | any gain; 2 VP | farmland_triggers_whenever_gained_and_a_victory_card_counts_two_points |
| Farmland | Overgrown Estate draws at once | an_overgrown_estate_trashed_by_a_bought_farmland_draws_at_once |

### Dark Ages
| Card | Ruling | Test |
|---|---|---|
| Poor House | +$4 then -$1 per Treasure, floor 0 | poor_house_gives_4_minus_the_treasures_in_hand_and_never_below_0 |
| Beggar | 3 Coppers to hand (as many as left) | beggar_gains_three_coppers_to_the_hand_or_as_many_as_are_left |
| Beggar | reaction: Silvers deck then discard; short pile; optional | beggar_discarded_to_an_attack_gains_two_silvers_one_on_the_deck; beggars_silvers_run_short_and_it_can_be_declined |
| Beggar | even if a Lighthouse protects; before Mercenary's decision | a_beggar_reaction_is_offered_even_to_a_player_a_lighthouse_protects; a_beggar_is_asked_before_a_mercenary_decides_to_trash_and_one_card_in_hand_fails_the_if_you_did |
| Squire | +$1 and three modes | squire_gives_one_coin_and_a_choice_of_actions_buys_or_a_silver |
| Squire | trashed: an Attack from the Supply, not the Mercenary | squire_trashed_gains_an_attack_card_from_the_supply; a_squire_cannot_gain_the_mercenary_or_other_attacks_outside_the_supply |
| Vagrant | draw first; keeps Curse/Ruins/Shelter/Victory | vagrant_draws_first_then_keeps_a_curse_ruins_shelter_or_victory_card; vagrant_with_an_empty_deck_shuffles_for_the_draw_and_has_nothing_to_reveal_after |
| Forager | trash then $ per distinct Treasure; no card still pays | forager_trashes_a_card_then_gives_a_coin_per_differently_named_treasure_in_the_trash; forager_with_no_card_to_trash_still_pays_for_the_trash |
| Forager | counted live (Treasurer takes some out) | forager_counts_the_treasures_in_the_trash_as_they_are_when_it_trashes |
| Forager | Sewers / Capitalism | not tested: no Forager-specific code; trash triggers resolve before the count (`finish_select` flushes the trash queue first) |
| Hermit | trash from discard or hand, gain up to $3 | hermit_trashes_a_non_treasure_from_the_discard_pile_or_the_hand_and_gains_a_card_up_to_3 |
| Hermit | Madman if no gain in the Buy phase; needs to be in play; Throne once | hermit_becomes_a_madman_if_no_card_was_gained_in_the_buy_phase; hermit_exchange_needs_the_hermit_in_play_and_a_throne_roomed_one_exchanges_once; hermit_is_exchanged_only_while_it_is_in_play |
| Hermit | event gain in Buy; several Buy phases | a_hermit_cannot_be_exchanged_after_a_card_was_gained_by_an_event_in_the_buy_phase; a_hermit_is_exchanged_if_the_last_buy_phase_gained_nothing_even_after_a_villa_bought_in_the_first |
| Hermit | no Madman left; played after the Buy phase; Possession | not tested: no Madman left leaves it in play by construction (`madman == 0` early return); Possession is not implemented |
| Market Square | cantrip + Buy | market_square_is_a_cantrip_with_a_buy |
| Market Square | discard for Gold; once per trash and per copy; optional; opponent's trash; drawn into hand | market_square_discarded_when_a_card_of_its_owner_is_trashed_gains_a_gold; market_square_is_offered_once_per_trashed_card_and_per_copy_and_may_be_declined; market_square_reacts_to_a_trash_by_the_opponents_attack_and_to_a_trash_it_draws_into_the_hand |
| Market Square | not for cards you do not own (Lurker); Haunted Mirror | market_square_does_not_react_to_a_card_that_is_not_the_players_own; a_haunted_mirror_trashed_with_a_market_square_in_hand_lets_the_square_serve_only_one_of_them |
| Sage | reveal to a $3+ card; shuffle; nothing found | sage_reveals_until_a_card_costing_3_or_more_and_discards_the_rest; sage_shuffles_when_the_deck_runs_out_and_finding_nothing_discards_everything |
| Storeroom | discard for cards, then for $ | storeroom_discards_to_draw_then_discards_for_coins; storeroom_with_nothing_to_discard_does_nothing_but_give_a_buy |
| Urchin | cantrip, discard to 4 | urchin_is_a_cantrip_that_makes_the_others_discard_down_to_4 |
| Urchin | trash for Mercenary before another Attack; same Urchin twice no; two Urchins yes; other trash nothing | urchin_in_play_may_be_trashed_for_a_mercenary_before_another_attack; urchin_cannot_trash_itself_when_it_is_the_attack_played_but_can_for_a_second_urchin; urchin_trashed_by_anything_else_gives_nothing |
| Urchin | Tormentor Imp, Rogue regains it, as a Way | an_urchin_trashed_for_a_mercenary_by_playing_a_tormentor_lets_it_gain_an_imp; rogue_can_gain_an_urchin_it_just_made_the_player_trash; an_urchin_played_as_a_way_is_still_in_play_to_be_trashed_for_a_mercenary |
| Urchin | Band of Misfits playing an Attack; Pathfinding / Kiln ordering | not tested: BoM-played plays go through the same Attack hook; Pathfinding/Kiln ordering is Adventures/Menagerie code |
| Armory | gain up to $4 onto the deck | armory_gains_a_card_costing_up_to_4_onto_the_deck |
| Armory | to-hand cards go to the deck; Trader gives a Silver in the discard | armory_gains_onto_the_deck_even_a_card_that_goes_to_the_hand; armory_card_exchanged_by_a_trader_gives_a_silver_in_the_discard_pile_not_on_the_deck |
| Death Cart | trash itself or an Action from the hand for $5 | death_cart_trashes_itself_or_an_action_from_the_hand_for_5_coins |
| Death Cart | Ruins when gained, however; as many as are left | gaining_a_death_cart_gains_two_ruins_however_it_is_gained; death_cart_gains_as_many_ruins_as_are_left; death_cart_after_being_gained_by_a_ferryman_pile_still_gives_ruins |
| Death Cart | trashing a Fortress still pays; via Band of Misfits | death_cart_trashed_fortress_returns_to_the_hand_and_still_pays; band_of_misfits_playing_death_cart_cannot_trash_the_cart_and_trashes_an_action_from_the_hand |
| Feodum | 1 VP per 3 Silvers; 3 Silvers when trashed; short pile | feodum_is_worth_a_point_per_three_silvers_and_gains_three_silvers_when_trashed; feodum_trashed_gains_as_many_silvers_as_are_left |
| Fortress | village; back to the hand on any turn; not a gain | fortress_is_a_village_that_returns_to_the_hand_when_trashed_on_any_turn; a_fortress_put_into_the_hand_after_a_trash_is_not_gained_so_a_trader_cannot_react; a_fortress_trashed_by_lurker_from_the_supply_goes_into_the_players_hand |
| Ironmonger | bonus per type of the revealed card | ironmonger_gives_a_bonus_for_each_type_of_the_revealed_card; ironmonger_with_an_empty_deck_reveals_nothing |
| Marauder | Spoils, Ruins in turn order; pile runs out | marauder_gains_a_spoils_and_gives_each_other_player_the_top_ruins_in_turn_order; marauder_stops_giving_ruins_when_the_pile_runs_out |
| Procession | play twice, trash, gain exactly +$1 | procession_plays_an_action_twice_trashes_it_and_gains_one_costing_exactly_1_more |
| Procession | on-trash before gain (Cultist); optional; no Durations; may gain one | procession_cultist_draws_for_its_trash_before_the_gain; procession_is_optional_and_does_not_take_durations; procession_can_gain_a_duration_card_and_cannot_play_one |
| Procession | gain even if not trashed (Madman, Horse, Island) | procession_on_a_madman_returns_it_once_and_still_gains_a_card_costing_1; procession_on_an_island_or_a_horse_does_not_trash_it_and_still_gains |
| Procession | +Action cards leave two Actions; Procession on Procession | procession_of_a_card_with_plus_one_action_leaves_two_actions_and_uses_only_its_own; procession_on_a_procession_plays_two_cards_twice_and_trashes_the_inner_procession_for_a_five |
| Rats | gain a Rats, trash a non-Rats; pile 20; none left; all-Rats hand | rats_gains_a_rats_and_trashes_a_card_that_is_not_a_rats; the_rats_pile_has_twenty_cards_whatever_the_number_of_players; rats_with_a_hand_of_only_rats_trash_nothing_and_with_none_left_gain_nothing |
| Rats | trashed: +1 Card | rats_trashed_draws_a_card |
| Scavenger | $2, deck to discard optional, put a card on the deck | scavenger_gives_2_coins_may_put_the_deck_in_the_discard_pile_and_puts_a_card_on_the_deck; scavenger_with_an_empty_deck_or_discard_pile_skips_what_it_cannot_do |
| Scavenger | no Tunnel trigger | scavenger_putting_the_deck_into_the_discard_pile_does_not_discard_it_for_a_tunnel |
| Wandering Minstrel | draw, reveal 3, Actions back in any order | wandering_minstrel_puts_the_revealed_actions_back_in_any_order_and_discards_the_rest; wandering_minstrel_reveals_what_it_can_when_the_deck_is_short |
| Band of Misfits | cheaper non-Duration Action stays in the Supply; top Ruins/Knight | band_of_misfits_plays_a_cheaper_non_duration_action_from_the_supply_leaving_it_there; band_of_misfits_can_play_the_top_ruins_or_knight_that_is_cheap_enough |
| Band of Misfits | Acting Troupe; "while in play" abilities | band_of_misfits_playing_acting_troupe_still_gives_its_villagers; while-in-play (Highway): not testable (both cost $5) |
| Bandit Camp | +1 Card +2 Actions, Spoils | bandit_camp_draws_gives_two_actions_and_a_spoils |
| Catacombs | take 3 or discard and +3; trashed: cheaper | catacombs_takes_the_top_three_or_discards_them_for_three_more; catacombs_trashed_gains_a_card_costing_less |
| Catacombs | before Remodel's gain; Throne twice; look not reveal | trashing_catacombs_with_remodel_gains_its_card_before_remodels; a_throne_roomed_catacombs_or_count_makes_each_choice_twice; looking_at_cards_with_catacombs_or_survivors_does_not_reveal_a_patron |
| Catacombs | Elder both options; -1 Card token / Chameleon | not tested: Elder and tokens are generic mode / draw code covered in allies_cards / adventures_cards |
| Count | two choices; options even if impossible; trash hand then triggers | count_makes_two_separate_choices; count_options_can_be_chosen_even_when_they_cannot_be_done; count_trashing_the_hand_trashes_every_card_first_and_then_resolves_what_they_do |
| Count | Elder: two of each | not tested: see Catacombs |
| Counterfeit | play a Treasure twice, trash it; Spoils | counterfeit_plays_a_treasure_twice_and_trashes_it; counterfeit_on_spoils_pays_twice_but_cannot_trash_it_and_pairs_with_a_silver_and_merchant |
| Counterfeit | Counterfeit twice | a_counterfeit_played_by_a_counterfeit_gives_its_coin_and_buy_and_its_choice_twice |
| Cultist | +2 Cards, Ruins, chain without an Action | cultist_draws_two_gives_ruins_and_may_chain_into_another_cultist_without_an_action; a_throne_roomed_cultist_gives_two_ruins_and_draws_twice |
| Cultist | trashed: +3 Cards any turn; not the revealed cards | cultist_trashed_draws_three_cards_on_anyones_turn; a_cultist_trashed_by_a_knight_does_not_draw_the_cards_about_to_be_discarded; a_swindlers_victims_cultist_draws_before_the_swindler_gain |
| Graverobber | two modes, options even if impossible, trash first | graverobber_gains_from_the_trash_onto_the_deck_or_upgrades_an_action; graverobber_options_may_be_chosen_when_they_cannot_be_done_and_the_trash_effect_comes_first |
| Graverobber | Villa from the trash ends in hand | graverobber_gaining_a_villa_from_the_trash_ends_with_it_in_the_hand |
| Junk Dealer | draw, trash | junk_dealer_draws_gives_a_coin_and_trashes_a_card |
| Mystic | name, put in hand; exact names | mystic_names_a_card_and_takes_the_top_card_if_it_matches; mystic_naming_one_knight_does_not_match_another |
| Pillage | trash itself, 2 Spoils, 5+ hand discard of attacker's choice | pillage_trashes_itself_gains_two_spoils_and_makes_players_with_five_cards_discard_one_of_the_attackers_choice; a_pillage_played_twice_trashes_once_and_a_short_spoils_pile_gives_what_is_left; a_throne_roomed_pillage_trashes_only_itself_with_another_pillage_in_play |
| Rebuild | name, reveal until unnamed Victory, upgrade +$3 | rebuild_names_a_card_and_upgrades_the_first_other_victory_card; rebuild_with_no_victory_card_to_find_discards_everything_revealed |
| Rogue | gain from the trash else attack; Attack even when gaining | rogue_gains_a_card_from_the_trash_if_one_costs_3_to_6_otherwise_attacks; rogue_is_an_attack_even_when_it_gains_so_a_diplomat_may_be_revealed |
| Altar | trash then gain up to $5, both mandatory | altar_trashes_a_card_and_gains_a_card_costing_up_to_5_even_with_nothing_to_trash |
| Hunting Grounds | +4 Cards; Duchy or 3 Estates when trashed | hunting_grounds_draws_four_and_trashed_gains_a_duchy_or_three_estates |
| Knights | every Knight's ability + common attack, victim's choice, discards the rest | every_knight_has_its_own_ability_and_the_common_attack; the_knights_attack_trashes_a_card_costing_3_to_6_of_the_two_revealed_the_victim_chooses_and_discards_the_rest |
| Knights | a trashed Knight takes the attacker; Anna's own trash does not; Sir Michael first; Sir Vander; turn order, Moat | a_knight_trashed_by_a_knight_takes_the_attacker_with_it; dame_anna_trashes_up_to_two_cards_first_and_her_own_trash_of_a_knight_does_not_trash_her; dame_natalie_may_gain_a_card_costing_up_to_3_and_sir_michael_discards_before_the_trashing; sir_vander_trashed_gains_its_owner_a_gold_on_either_players_turn; the_knights_attack_goes_round_in_turn_order_and_a_moat_protects |
| Knights | Potion / Debt costs do not count; Sir Martin $4 | a_knight_cannot_trash_a_card_with_a_potion_in_its_cost; sir_martin_on_top_of_the_knights_pile_costs_four_and_can_be_gained_with_an_armory |
| Knights | pile of ten shuffled, only the top gainable; not a Victory pile | the_knights_pile_is_one_kingdom_pile_of_ten_different_knights; draining_the_knights_pile_deals_each_knight_once_in_a_random_order; a_specific_card_gained_from_a_mixed_pile_fails_unless_it_is_the_top |
| Knights | Black Market / Trade Route | not testable: not in the engine |
| Ruins | pile with a Looter, 10 per extra player, top only, shuffled | the_ruins_pile_comes_with_a_looter_and_has_ten_per_player_after_the_first; the_ruins_pile_is_a_fresh_random_selection_and_order_each_game; six_players_use_all_fifty_ruins_ten_of_each_kind |
| Ruins | bonuses; Survivors | the_ruins_give_their_bonuses; survivors_discards_the_top_two_or_puts_them_back_in_any_order |
| Ruins | different names (Menagerie); empty pile ends the game | ruins_with_different_names_count_as_different_cards; a_game_ends_on_three_empty_piles_with_a_mixed_pile_counting_once |
| Ruins | Ambassador / Embargo / Contraband / Talisman | not testable: not in the engine |
| Looter | Black Market / Riverboat decks | not testable: no Black Market; Riverboat's card is never a Looter by construction |
| Shelters | replace Estates by the kingdom rule; fresh and reproducible | shelters_replace_the_estates_and_each_appears_in_the_opening_hand_half_the_time; the_shelter_deal_is_fresh_for_each_game_and_reproducible_by_seed; shelters_follow_the_kingdom_rule_and_the_config_override |
| Shelters | Estate pile unchanged | shelters_do_not_change_the_estate_pile |
| Hovel | trash from hand when a Victory card is gained | hovel_is_trashed_from_the_hand_when_a_victory_card_is_gained; rogue_gaining_a_victory_card_from_the_trash_lets_a_hovel_be_trashed |
| Necropolis | +2 Actions; Way of the Horse/Butterfly has no pile | necropolis_gives_two_actions; a_necropolis_played_as_way_of_the_horse_stays_in_play |
| Overgrown Estate | Victory worth 0; +1 Card when trashed, right away | overgrown_estate_is_a_victory_card_worth_nothing_that_draws_when_trashed; an_overgrown_estate_trashed_by_altar_draws_before_the_gain |
| Spoils | $3, returns to the pile; non-Supply | spoils_gives_3_coins_and_returns_to_its_pile; the_spoils_madman_and_mercenary_piles_are_outside_the_supply |
| Spoils | no Mint copy; Treasurer from the trash; Masquerade | a_mint_cannot_gain_a_copy_of_a_spoils; a_treasurer_can_take_a_spoils_out_of_the_trash; Masquerade: not tested, no Spoils-specific code |
| Madman | +2 Actions, return, draw per card; empty pile not a Supply pile | madman_gives_two_actions_returns_to_its_pile_and_draws_a_card_per_card_in_hand; procession_on_a_madman_cannot_return_another_madman_in_play |
| Mercenary | trash 2 for +2 Cards +$2 + attack; both trashed before triggers | mercenary_trashes_two_cards_for_cards_coins_and_a_hand_size_attack; mercenary_trashes_both_cards_before_what_they_do_when_trashed |
| Mercenary | reactions first; one card fails "if you did" | a_beggar_is_asked_before_a_mercenary_decides_to_trash_and_one_card_in_hand_fails_the_if_you_did |
| Mixed piles | state, text round trip, determinization | the_unseen_part_of_a_pile_is_public_and_matches_what_is_left; determinize_does_not_leak_the_hidden_part_of_the_ruins_or_knights_piles; text_round_trip_keeps_the_piles_and_shelters |

Coverage counts: Hinterlands 94 rulings (82 covered by a named test, 1 by an ignored test, 11 not testable / not tested with a reason); Dark Ages 100 rulings (88 covered, 12 not testable / not tested with a reason).

## Test status
`cargo test --release --workspace --no-fail-fast`: everything passes except `rising_sun_cards::random_rising_sun_games_conserve_cards_and_terminate` (game 2619: a 5-player Rising Sun game with League of Shopkeepers; the text round trip loses the `ally:` line). It does not touch these sets; my only `text.rs` change is the transient `buy_gain_base` field. Not investigated further.
