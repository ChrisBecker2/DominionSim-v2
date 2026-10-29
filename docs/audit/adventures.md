# Audit: Adventures (cards, Events, tokens, Travellers, Reserve cards)

Source of rules: the offline wiki copy (card pages, FAQ / Official FAQ / Other rules clarifications, errata/versions, `Traveller`, `Reserve`, `Adventures tokens`, `Debt`).
Branch: `worktree-agent-adb8ebb4df3f989c2`. Tests: `crates/engine/tests/adventures_cards.rs` (engine), `crates/sim/tests/adventures_defaults.rs` (bots).
(Sauna/Avanto belong to the Promo set and were not audited here.)

## Bugs found and fixed (each with a test that failed before the fix)

| Bug | Rule | Test |
|---|---|---|
| Warrior put a $3/$4 card straight into the trash: a discarded Tunnel could not be revealed for its Gold, a Village Green / Trail / Weaver could not be played. | Warrior FAQ: "If another player discards a Tunnel, they can reveal it to gain a Gold, but they will still trash the Tunnel"; "they can play a Village Green and have it not be trashed". | `warrior_discarding_a_tunnel_lets_its_owner_reveal_it_for_a_gold_and_it_is_still_trashed`, `warrior_discarding_a_village_green_lets_it_be_played_and_then_it_is_not_trashed` |
| Duplicate was offered for a card with Debt in its cost (Engineer, ...). | Duplicate FAQ: "Cards with Potion or Debt in their cost cannot be Duplicated." | `duplicate_is_not_offered_for_a_card_with_debt_in_its_cost` |
| Seaway was "available" when the only Action costing "up to $4" had Debt in its cost (the buy did nothing). | Debt page: "'Up to $4' ... does not include costs with Debt." | `seaway_is_not_available_when_only_a_debt_cost_action_is_left` |
| The -$1 token's reference level (`tax_base`) went stale: after Ball took the token and the player then spent $ below that level, the next $ gained (Borrow's +$1) was not reduced. | -$1 token: "It causes the player to get $1 less the next time they get $". | `the_minus_one_coin_token_still_takes_a_coin_after_the_player_spent_money_below_the_level_they_had` |
| Only one copy of Caravan Guard (and Guard Dog) could be played against an Attack; a copy drawn by the first Guard was not offered. | Caravan Guard FAQ: "After reacting with a Caravan Guard, you can use another one, even one you just drew". | `several_caravan_guards_can_all_be_played_against_one_attack`, `a_caravan_guard_drawn_by_a_caravan_guard_can_be_played_against_the_same_attack` |
| Champion gave no +1 Action for an Inheritance Estate (an Action card on its owner's turn). | Inheritance + Champion: "when you play an Action card, you first get +1 Action". | `an_inherited_estate_is_an_action_card_so_champion_gives_an_action_for_it_and_for_the_card_it_plays` |

Counts: 6 bugs found, 6 fixed, 3 left open (below) + 1 policy note with an ignored test.

## Open issues (ignored `audit:` tests)

1. **Fixed (2026-10-28):** `GameState::back_to_action_phase` (Villa, Cavalry, Continue, Launch) runs the end-of-Buy-phase hooks for each Buy phase; Merchant Guild and Exploration count that Buy phase only. Was: **End-of-Buy-phase offers once per turn.** `end_buy_phase` runs `end_of_buy` once (`treasury_offered`), so Wine Merchant's discard is offered only at the true end of the turn, not when Villa / Cavalry / Continue / Launch return the player to the Action phase (Wine Merchant FAQ). Test: `wine_merchant_may_be_discarded_when_a_villa_returns_the_player_to_the_action_phase_with_two_unspent` (ignored). Needs the shared turn flow.
2. **Fixed (2026-10-28):** Messenger compares with `buy_gain_base`. Was: **Messenger in a second Buy phase.** `adventures.rs:472` tests `gained_in_buy == 1`, a per-turn count shared with Merchant Guild / Exploration / Hunting Grounds; a Messenger bought in the second Buy phase of a Villa turn does nothing (Messenger FAQ: "you can use Messenger's on-gain effect multiple times in the same turn"). Test: `messenger_works_again_in_a_second_buy_phase_taken_with_villa` (ignored). Fix: a per-Buy-phase counter reset where the Buy phase starts.
3. **Seaway cannot gain an Estate for a player with Inheritance** (the gain filter tests printed types; Estates are Actions on the Inheriting player's turn; Seaway FAQ: "with Inheritance, you can put the +1 Buy token on Estates"). Test: `seaway_can_gain_an_estate_for_a_player_with_inheritance_and_put_the_token_on_the_estate_pile` (ignored).

Known limitation of the Warrior fix: for Tunnel/Trail/Weaver/Village Green the trash is a forced `Select` of "that name from the discard pile", so if the victim already had another copy of the same card in the discard pile and plays the discarded one (Village Green / Trail / Weaver), the other copy is trashed instead (stop-moving rule not tracked here).

## Policy notes (bots)

- Treasure Hunter is always played (`worth_playing` default true). Wiki (Treasure Hunter): "gaining two or more Silvers is less likely to be desirable ... Later in the game, it can often be preferable to avoid playing Treasure Hunter". Test: `a_treasure_hunter_is_not_played_when_it_would_gain_a_pile_of_silvers` (ignored, fails today).
- Hero is not exchanged for a second Champion. The old reason ("its +1 Action does not stack") was wrong: each Champion effect gives +1 Action per Action played (`two_champions_each_give_an_action_per_action_played`). The default is kept on measurement: with one Champion every Action is already free to play, and keeping the Hero won 51.1% against exchanging it (two Pages and Smithies, 20000 games).
- Resolved: Champion effects are counted per play (`PlayerState::champions`), so a Throne-Roomed Champion gives +2 Actions per Action (as Hireling's FAQ counts each play); a Champion taken out of play (Throne Room with Way of the Horse) works until the end of that turn (wiki), and its Throne Room is then discarded. Tests: `a_throne_roomed_champion_is_two_champion_effects_for_the_rest_of_the_game`, `a_champion_returned_by_way_of_the_horse_works_until_the_end_of_the_turn`, `a_champion_played_as_way_of_the_horse_first_then_as_itself_works_this_turn_only`, `two_champions_give_their_actions_before_snowy_village_and_nothing_after`.
- Gear's default set-aside (`gear_pick`) has dead code (`coins_in_play * 0`); only sets aside Actions when no Actions are left or Treasures at $11+. Wiki (Gear): "set aside junk to miss a reshuffle", "set aside a terminal that collides" are not modelled. Not changed.
- Wiki (Page / Peasant): "Most importantly, exchanging for Teacher causes you to lose out on the explosive power of Disciple"; default always exchanges (a stated `[[exchange]]` rule can keep the card).
- Whole-set bot fuzz added: `bots_that_buy_every_adventures_card_and_event_play_whole_games_without_an_illegal_choice` (no illegal Pass / choice found).

## Cards checked (one line each)

All 30 kingdom cards, the 8 Traveller upgrades, the 20 Events, the 10 tokens and the Reserve / Traveller rules: text, costs, types, clause order, "may", who chooses, zones, Duration behaviour, Throne Room replays, Attack/immunity, mark (`choice_free` / `has_choice` / `order_sensitive`) checked against the CLAUDE.md rule.

| Card | Result |
|---|---|
| Coin of the Realm | OK (call window after any Action, also on other turns; Royal Carriage vs mat rules ok) |
| Page, Peasant | OK |
| Ratcatcher, Guide, Transmogrify, Teacher | OK (calls at start of turn, any order, no Action used) |
| Raze | OK (coin cost only; self or hand card; Throne Room second play) |
| Amulet, Dungeon, Gear | OK |
| Caravan Guard | Fixed (multiple copies) |
| Duplicate | Fixed (Debt) |
| Magpie | OK |
| Messenger | OK (second Buy phase fixed) |
| Miser | OK (Coppers on the mat count for Gardens; mat capacity 10 distinct ids is exactly enough) |
| Port | OK |
| Ranger, Giant | OK (shared Journey token; Giant reactions even when face down; $3-$6 test excludes Potion) |
| Artificer | OK |
| Bridge Troll | OK |
| Distant Lands | OK |
| Haunted Woods, Swamp Hag | OK (once per card bought per Hag; Moat only when played) |
| Lost City | OK |
| Relic | OK |
| Royal Carriage | OK |
| Storyteller | OK (2022 text) |
| Treasure Trove | OK |
| Wine Merchant | OK (return to the Action phase fixed) |
| Hireling | OK |
| Treasure Hunter | OK (counts every gain; exchanges are not gains) |
| Warrior | Fixed (Tunnel etc.) |
| Hero | OK |
| Champion | Fixed (Inheritance Estate); see policy notes |
| Soldier, Fugitive, Disciple | OK |
| Alms, Borrow, Quest, Save, Scouting Party, Travelling Fair, Bonfire, Expedition, Ferry, Plan, Mission, Pilgrimage, Ball, Raid, Seaway, Trade, Lost Arts, Training, Pathfinding | OK (Seaway: open issue 3) |
| Inheritance | OK (2025 errata: no Duration, no Command) |
| Tokens (+1 Card/Action/Buy/$, -$2 cost, Trashing, -1 Card, -$1, Journey, Estate) | OK; -$1 fixed |

## Ruling coverage

"existing" tests are in `adventures_cards.rs` before the audit; "new" were added by it.

| Card | Ruling | Test |
|---|---|---|
| Coin of the Realm | $1 Treasure, played in Buy phase, goes to the mat | `coin_of_the_realm_is_a_copper_that_waits_on_the_mat` |
| Coin of the Realm | called after an Action: +2 Actions; the Coin is not $1 when called; discarded that turn | `coin_of_the_realm_called_after_an_action_gives_two_actions` |
| Coin of the Realm | callable on a Reserve that went to the mat (Royal Carriage cannot) | `coin_of_the_realm_works_on_a_reserve_card_that_went_to_the_mat_but_royal_carriage_does_not` |
| Coin of the Realm | callable on other players' turns but gives nothing | `coin_of_the_realm_called_on_another_players_turn_does_nothing` (new) |
| Coin of the Realm | several may be called; may decline | `coin_of_the_realm_can_be_declined_and_two_can_be_called_for_one_play` |
| Coin of the Realm | callable in a Crown / Toil / Werewolf play outside the Action phase | not testable cheaply: needs Crown/Toil state; the call window is the same `AfterPlay` frame as tested above |
| Page / Peasant | +1 Card +1 Action / +1 Buy +$1; exchange on discard from play | `page_and_peasant_are_a_cantrip_and_a_buy_with_a_coin`, `a_traveller_is_exchanged_when_discarded_from_play` |
| Page / Peasant | returned to its pile, upgrade to the discard pile | `a_traveller_is_exchanged_when_discarded_from_play` |
| Traveller | not from the hand; not with an empty upgrade pile; highest first | `a_traveller_discarded_from_the_hand_or_with_an_empty_pile_is_not_exchanged`, `the_higher_traveller_is_exchanged_first_so_a_short_pile_is_used_well` |
| Traveller | Champion and Teacher are not Travellers | `champion_and_teacher_are_not_travellers` |
| Traveller | Band of Misfits play cannot exchange | `a_band_of_misfits_play_of_a_page_cannot_be_exchanged` (new) |
| Traveller | Black Market purchase cannot be exchanged; a Way play can; Lurker gains from the trash; Harsh Winter | not testable: Black Market not implemented; the others are not Adventures behaviour |
| Traveller | non-Supply pile empty is not an empty pile | `set_up_has_the_piles_the_traveller_lines_and_the_events` |
| Ratcatcher / Guide / Transmogrify / Teacher | put on the mat when played; stay until called; called at the start of the turn in any order; discarded that turn | `reserve_cards_go_onto_the_tavern_mat_when_played_and_stay_owned`, `ratcatcher_trashes_a_card_at_the_start_of_a_turn`, `guide_discards_the_hand_and_draws_five_at_the_start_of_a_turn`, `transmogrify_trashes_a_card_and_gains_one_costing_up_to_a_coin_more_into_the_hand`, `start_of_turn_calls_may_be_declined_and_several_cards_are_each_asked` |
| Reserve | calling is not playing (no Action used, no Conspirator count), called cards are in play | `reserve_cards_called_are_in_play_but_calling_is_not_playing_an_action` (new) |
| Reserve | may be called with no effect (Ratcatcher with an empty hand) | `a_reserve_card_may_be_called_with_no_effect` (new) |
| Reserve | Pilgrimage copies a called Reserve card but not one on the mat | `pilgrimage_can_copy_a_called_reserve_card_but_not_one_on_the_mat` (new) |
| Reserve | played at the start of the turn (Way of the Turtle) can be called at once | not tested: needs Menagerie Way state; would need `Way of the Turtle` set-up |
| Transmogrify | no card to trash: no gain; copy of the same card allowed; gain goes to the hand | `transmogrify_with_nothing_in_the_hand_gains_nothing_and_may_gain_a_copy_of_the_trashed_card` (new) |
| Raze | trash itself or a hand card; cost in $; nothing for Potion/Debt; Bridge Troll lowers it | `raze_trashes_itself_and_looks_at_two_cards`, `raze_trashes_a_card_from_the_hand_and_looks_at_as_many_as_it_costs`, `raze_looks_at_the_coin_cost_only_and_a_bridge_troll_lowers_it` (new) |
| Raze | $0 card: nothing; empty hand allowed; -1 Card token not affected | `raze_on_a_card_costing_nothing_looks_at_nothing_and_may_trash_with_an_empty_hand`, `raze_is_not_affected_by_the_minus_one_card_token_and_replaces_it` |
| Raze | Throne Room: either choice even if not in play | `a_throne_roomed_raze_trashes_itself_once_and_a_card_from_the_hand_the_second_time` |
| Raze | Royal Carriage cannot respond to a Raze that trashed itself | `royal_carriage_is_not_offered_for_a_raze_that_trashed_itself_while_another_raze_is_in_play` (new) |
| Amulet | choice now and next turn, may differ | `amulet_gives_a_choice_now_and_at_the_start_of_the_next_turn`, `amulet_can_trash_a_card_from_the_hand` |
| Caravan Guard | +1 Card +1 Action now, +$1 next turn | `caravan_guard_draws_now_and_gives_a_coin_next_turn` |
| Caravan Guard | played in reaction: draws, no benefit to the attacker, +1 Action useless | `caravan_guard_is_played_from_the_hand_against_an_attack_without_giving_the_attacker_anything` |
| Caravan Guard | another / a just-drawn Guard may follow | `several_caravan_guards_can_all_be_played_against_one_attack`, `a_caravan_guard_drawn_by_a_caravan_guard_can_be_played_against_the_same_attack` (new) |
| Caravan Guard | against Relic: draws, then the token | `caravan_guard_played_against_a_relic_draws_before_the_token_is_put_on_the_deck` (new) |
| Caravan Guard | Royal Carriage on a Guard played on another turn stays in play; Coin of the Realm does not | `royal_carriage_called_on_a_caravan_guard_played_in_reaction_stays_in_play` (new); Coin: `coin_of_the_realm_called_on_another_players_turn_does_nothing` |
| Caravan Guard | Trained Guard vs Bridge Troll: +$1 before the token | `a_bridge_troll_token_is_taken_after_the_victims_caravan_guard_reaction` (new; the +$1 token is not applied on another player's turn, so the token stays) |
| Dungeon | +1 Action; draw 2 discard 2 now and next turn | `dungeon_draws_and_discards_now_and_next_turn` |
| Gear | set aside 0-2, face down; returns next turn; 0 aside: discarded that turn | `gear_sets_aside_up_to_two_cards_and_returns_them_next_turn`, `gear_that_sets_nothing_aside_is_discarded_the_same_turn` |
| Gear | Throne Room keeps in play when set aside once | `gear_with_a_throne_room_that_sets_aside_once_keeps_the_throne_room_in_play` (new; weak assertion) |
| Duplicate | cost up to $6, copy from the Supply, discard pile | `duplicate_gains_a_copy_of_a_card_costing_up_to_six` |
| Duplicate | not for costs over $6 / no copy | `duplicate_is_not_offered_for_a_card_costing_more_or_for_a_card_with_no_copy` |
| Duplicate | other players' turns; non-buy gains | `duplicate_works_on_another_players_turn_and_for_cards_gained_other_ways` |
| Duplicate | not Potion / Debt cards | `duplicate_is_not_offered_for_a_card_with_debt_in_its_cost` (new) |
| Duplicate | several Duplicates for one card | `two_duplicates_gain_two_copies` (new) |
| Duplicate | cannot gain non-Supply cards (Teacher) | by construction (`g.in_supply(c)` test); no cheap test (needs a Teacher gain) |
| Duplicate | Innovation chain | not testable cheaply: needs a Project state |
| Magpie | Treasure to hand, Action/Victory gains a Magpie, non-Treasure stays on top | `magpie_takes_a_treasure_and_gains_a_magpie_for_an_action_or_victory_card`, `magpie_leaves_a_non_treasure_on_top_and_ignores_a_curse_and_an_empty_pile` (new) |
| Magpie | Treasure-Victory does both | `magpie_gains_a_magpie_for_a_treasure_victory_card_and_takes_it_too` (new) |
| Messenger | +1 Buy +$2, may discard the deck | `messenger_gives_a_buy_and_two_coins_and_may_discard_the_deck` |
| Messenger | first gain in the Buy phase; others copy; not otherwise | `messenger_gains_a_card_for_everyone_when_it_is_the_first_gain_of_the_buy_phase`, `messenger_does_nothing_extra_when_it_is_not_the_first_gain_or_not_in_the_buy_phase` |
| Messenger | gained by Ball counts | `messenger_gained_by_ball_first_in_the_buy_phase_gives_everyone_a_card` (new) |
| Messenger | second Buy phase (Villa) | `messenger_works_again_in_a_second_buy_phase_taken_with_villa` (passes) |
| Messenger | no Tunnel trigger; copies run out for later players; Watchtower trick | not tested: copies running out is `push_gain_to_each` generic code; Tunnel/Watchtower belong to other sets |
| Miser | both choices with nothing to do; mat Coppers score | `miser_puts_coppers_on_the_mat_or_pays_for_them`, `miser_with_no_copper_in_the_hand_does_nothing_and_mat_coppers_score` (new), `a_tavern_mat_holds_many_coppers_and_other_reserves_together` (new) |
| Port | gain another Port that has none; pile of 12 | `port_comes_with_another_port_that_does_not_come_with_another`, `set_up_has_the_piles_the_traveller_lines_and_the_events` |
| Port | gained elsewhere: second to the discard pile; gained from the trash | the "other place" case: `port_comes_with_another_port...` gains to the discard pile; deck-top case by construction (`g.gain(.., Dest::Discard)`); Lurker case not tested |
| Ranger | Journey token alternates, shared with Giant / Pilgrimage | `ranger_draws_five_every_other_play`, `journey_token_flips_between_cards_and_events` |
| Artificer | exactly $1 per card; zero discards: $0 card; decline; onto the deck | `artificer_gains_a_card_costing_exactly_one_per_card_discarded_onto_the_deck`, `artificer_discarding_nothing_may_gain_a_card_costing_nothing`; hand-gain redirection (Guardian etc.) not tested (other sets) |
| Bridge Troll | -$1 token to others; -$1 cost this and next turn; +1 Buy twice; stacking; Throne Room | `bridge_troll_gives_the_other_players_a_minus_coin_token_and_cheapens_cards_two_turns`, `two_bridge_trolls_cheapen_cards_by_two`, `a_throned_bridge_troll_cheapens_two_on_both_turns` (new) |
| Distant Lands | 4 VP only on the mat; counts as owned for Gardens; pile 8/12 | `distant_lands_is_worth_four_victory_points_only_on_the_mat`, `distant_lands_counts_as_a_card_owned_for_gardens_on_and_off_the_mat` (new), `set_up_has_the_piles_the_traveller_lines_and_the_events` |
| Giant | alternates; +$1 / +$5 and the attack | `giant_alternates_between_a_coin_and_a_big_attack` |
| Giant | $3-$6 trashed, else discard + Curse; no card: Curse | `giants_attack_discards_and_curses_a_card_outside_three_to_six`, `giant_with_no_cards_to_reveal_still_curses` |
| Giant | asterisk cost trashed; Potion not | `giant_trashes_a_non_supply_card_with_an_asterisk_cost_between_three_and_six` (new), `giant_does_not_trash_cards_with_a_potion_in_the_cost` (new); Debt: no coin cost between 3 and 6 exists, by construction |
| Giant | reactions even when face down | `giant_played_face_down_still_lets_other_players_react_to_the_attack` (new) |
| Haunted Woods | +3 Cards next turn; topdecks the hand of a buyer | `haunted_woods_draws_three_next_turn_and_topdecks_the_hand_of_a_buying_player`, `haunted_woods_gives_three_cards_at_the_start_of_its_owners_next_turn` |
| Haunted Woods | not for non-bought gains | `haunted_woods_does_not_touch_a_card_gained_without_buying_it` |
| Haunted Woods | Moat only when played; Events are not cards | `haunted_woods_is_blocked_by_a_moat_only_when_played` (new), `swamp_hag_curse_comes_per_card_bought_and_not_for_events` (new) |
| Haunted Woods | Mission extra turn, Black Market, Cavalry ordering | Mission: not tested (Hag/Woods discarded on the extra turn: covered by Duration cleanup); Black Market not implemented; Cavalry ordering not tested |
| Swamp Hag | Curse per card bought; stacks | `swamp_hag_curses_a_player_who_buys_a_card_and_gives_three_coins_next_turn`, `swamp_hags_stack_for_each_card_bought` |
| Lost City | +2 Cards +2 Actions; others draw when gained, any way | `lost_city_makes_the_other_players_draw_when_gained` |
| Relic | $2 Attack Treasure; token on the deck; Moat | `relic_is_a_two_coin_attack_treasure_that_gives_the_minus_one_card_token`, `relic_can_be_blocked_by_a_moat` |
| Royal Carriage | replays after full resolution; several; still in play; Duration stays | `royal_carriage_replays_the_action_it_follows`, `several_royal_carriages_replay_the_same_action_one_after_the_other`, `royal_carriage_cannot_replay_a_card_that_left_play`, `a_replayed_duration_keeps_royal_carriage_in_play`, `royal_carriage_is_not_offered_for_a_reserve_that_went_to_the_mat`, `royal_carriage_is_not_offered_for_a_card_played_from_the_supply` |
| Royal Carriage | Disciple / Sauna chains, leave-and-return, Tragic Hero, Underling / League of Shopkeepers | not tested: Disciple-played Carriage order is generic `AfterPlay` stacking (`several_royal_carriages...`); the others need Renaissance / Allies / Promo set-up |
| Storyteller | plays up to 3 Treasures, +1 Card, pays all $ | `storyteller_plays_treasures_draws_and_pays_all_its_coins_for_cards`, `storyteller_with_nothing_to_play_just_draws` |
| Storyteller | Potion kept, Treasures stay in play | `storyteller_keeps_potions_and_leaves_the_played_treasures_in_play` (new) |
| Storyteller | Quarry keeps its reduction; Counterfeit/Crown extra plays; Coffers before paying; Chameleon | not tested: need other sets' state (Quarry, Counterfeit, Coffers, Way) |
| Treasure Trove | $2; Gold and Copper; one pile empty | `treasure_trove_gains_a_gold_and_a_copper`, `treasure_trove_still_gains_the_other_card_when_one_pile_is_empty` (new) |
| Wine Merchant | +1 Buy +$4; mat; $2 total for several; Throne Room | `wine_merchant_is_discarded_from_the_mat_with_two_coins_unspent`, `wine_merchant_needs_two_unspent_in_total_and_is_kept_otherwise`, `wine_merchant_throned_gives_its_bonus_both_times_and_goes_to_the_mat_once` (new) |
| Wine Merchant | unplayed Treasures are not unspent | `unplayed_treasures_in_hand_are_not_unspent_money_for_wine_merchant` (new) |
| Wine Merchant | discard on return to the Action phase (Villa) | `wine_merchant_may_be_discarded_when_a_villa_returns_the_player_to_the_action_phase_with_two_unspent` (passes) |
| Hireling | +1 Card every turn; stays in play; Throne Room draws two and keeps the Throne Room | `hireling_draws_a_card_at_the_start_of_every_turn_for_the_rest_of_the_game`, `a_throne_roomed_hireling_draws_two_and_keeps_the_throne_room_with_it` |
| Hireling | Royal Carriage replay draws two | `a_hireling_played_twice_by_royal_carriage_draws_two_cards_each_turn` (new) |
| Hireling | played at the start of a turn (Piazza) draws at once; removed from play stops; tokens only on new plays | not tested: Piazza needs Renaissance Project state; "removed from play" needs a Way |
| Treasure Hunter | Silver per card the right player gained, all gains; exchanges do not count | `treasure_hunter_gains_a_silver_per_card_the_player_to_the_right_gained_last_turn`, `treasure_hunter_counts_every_gain_the_player_made_not_only_their_buys` |
| Warrior | +2 Cards; one hit per Traveller in play | `warrior_draws_two_and_attacks_once_per_traveller_in_play`, `warrior_trashes_four_cost_non_supply_cards_and_counts_only_travellers` (new) |
| Warrior | only exactly $3/$4; Potion not | `warrior_trashes_only_cards_costing_exactly_three_or_four_and_discards_the_rest` |
| Warrior | discards one at a time, not a reveal; Tunnel, Village Green | `warrior_discarding_a_tunnel_lets_its_owner_reveal_it_for_a_gold_and_it_is_still_trashed`, `warrior_discarding_a_village_green_lets_it_be_played_and_then_it_is_not_trashed` (new) |
| Hero | $2, gains a Treasure from the Supply | `hero_gains_a_treasure_of_the_players_choice`, `hero_cannot_gain_a_treasure_that_is_not_in_the_supply` (new) |
| Champion | stays; Attacks no longer affect; +1 Action per Action; not applied to itself | `champion_stays_in_play_protects_from_attacks_and_gives_an_action_per_action`, `champions_action_is_reported_as_a_bonus` |
| Champion | earlier Attacks still hit | `champion_ignores_attacks_played_earlier` |
| Champion | non-Attacks (Possession, Masquerade) and Snowy Village exceptions | not tested: Possession / Snowy Village not applicable or other sets |
| Champion | stacking and Inheritance Estate | `two_champions_each_give_an_action_per_action_played` (new), `an_inherited_estate_is_an_action_card_so_champion_gives_an_action_for_it_and_for_the_card_it_plays` (new) |
| Soldier | $2 +$1 per other Attack in play; hand-size attack; Disciple does not add | `soldier_gives_coins_per_other_attack_in_play_and_makes_big_hands_discard`; Duration Attack from last turn: `soldier_counts_a_duration_attack_played_last_turn` (new) |
| Fugitive | +2 Cards +1 Action, discard a card | `fugitive_draws_two_gives_an_action_and_discards_a_card` |
| Disciple | optional; play twice; copy; non-Supply: no copy; Duration stays | `disciple_plays_an_action_twice_and_gains_a_copy`, `disciple_on_a_non_supply_card_plays_it_twice_but_gains_nothing`, `disciple_playing_a_duration_stays_in_play_with_it` |
| Disciple | uses one Action only | covered by `disciple_plays_an_action_twice_and_gains_a_copy` (Actions asserted) |
| Teacher | moves one of four tokens to a pile without the player's tokens (also -$2 cost, Trashing); other players' tokens do not matter; Estate token ok | `teacher_moves_one_of_the_four_tokens_to_a_pile_without_the_players_tokens`, `teacher_cannot_use_a_pile_holding_the_players_minus_two_cost_or_trashing_token` (new), `teacher_may_use_a_pile_whose_card_is_set_aside_under_the_estate_token` (new) |
| Teacher | VP token on Temple | not tested: Empires pile token interplay |
| Alms | once per turn; no Treasures in play; up to $4 | `alms_gains_a_card_up_to_four_only_with_no_treasures_in_play_and_once_a_turn` and the `alms_*` buy-phase tests |
| Borrow | once per turn; +1 Buy; token | `borrow_gives_a_buy_and_a_coin_for_a_minus_one_card_token_once_a_turn`, `borrow_gives_no_coin_when_the_minus_one_card_token_is_already_on_the_deck` (new) |
| Quest | Attack / two Curses / six cards; fewer than required: discard all, no Gold | `quest_discards_an_attack_two_curses_or_six_cards_for_a_gold`, `quest_with_one_curse_or_too_few_cards_discards_them_all_and_gains_nothing` (new) |
| Save | once per turn; +1 Buy; returns after the next hand | `save_sets_a_card_aside_and_returns_it_after_the_next_hand_is_drawn` |
| Scouting Party | look at 5, discard 3, rest back in any order; fewer than 5; -1 Card token unaffected | `scouting_party_discards_three_of_the_top_five_and_puts_two_back`, `scouting_party_with_fewer_than_five_cards_still_discards_three` ; token: reveal path shared with `raze_is_not_affected_by_the_minus_one_card_token_and_replaces_it`; Tunnel / Progress interplay not tested (other sets) |
| Travelling Fair | +2 Buys; topdeck any gained card this turn, optional; not exchanges | `travelling_fair_gives_two_buys_and_lets_gained_cards_go_onto_the_deck_this_turn`, `travelling_fair_works_for_cards_gained_by_other_effects_and_ends_with_the_turn`, `treasure_hunter_does_not_count_exchanged_cards` is about gains; exchange is not a gain by construction in `exchange_traveller` |
| Bonfire | up to two Coppers in play; $ stays | `bonfire_trashes_up_to_two_coppers_in_play`, `bonfire_keeps_the_coins_of_the_trashed_coppers` (new) |
| Expedition | +2 cards for the next hand; stacks; Outpost 3+2 | `expedition_draws_two_extra_cards_for_the_next_hand_and_stacks`, `expedition_adds_two_to_an_outposts_three_card_hand` (new) |
| Ferry | -$2 cost on the owner's turns; moves; other players' cards on that pile | `ferrys_minus_two_cost_token_cheapens_a_pile_on_its_owners_turns`, `ferry_moves_the_token_to_a_pile_of_the_players_choice`, `a_ferry_token_moves_when_it_is_placed_again`, `a_minus_two_cost_token_lets_transmogrify_turn_a_copper_into_a_two_cost_cheaper_card` (new) |
| Plan | Trashing token on gain from the pile, also other turns | `plan_lets_a_card_gained_from_its_pile_trash_a_card_from_the_hand`, `plan_triggers_on_any_gain_from_the_pile_including_other_players_turns` |
| Plan | gained from the trash (Lurker) counts as the pile it started in; non-Action card from an Action pile (Odysseys) | not tested: the trigger compares `pile_of(card)`, covered by `plan_triggers_on_any_gain...`; Lurker path / Odysseys need other sets |
| Mission | extra turn, no cards, Events ok; no third turn in a row; not twice | `mission_gives_an_extra_turn_in_which_no_cards_can_be_bought_but_events_can`, `mission_bought_twice_gives_one_extra_turn`, `outposts_extra_turn_wins_over_missions`, `a_missions_extra_turn_does_not_count_towards_the_fewer_turns_tiebreak` |
| Mission | Black Market, Voyage + Mission, Possession | not testable: Black Market / Possession not implemented; Voyage is Allies-era promo not covered here |
| Pilgrimage | Journey flip; up to 3 differently named; Supply copies only; Potion ok | `pilgrimage_flips_the_journey_token_and_copies_up_to_three_differently_named_cards_in_play`, `pilgrimage_chooses_before_gaining_and_cannot_copy_cards_off_the_supply` (new) |
| Ball | -$1 token once; two cards up to $4; repeatable | `ball_takes_the_minus_coin_token_once_and_gains_two_cards_up_to_four`, `ball_costing_five_with_the_token_makes_the_next_money_a_coin_short` |
| Raid | Silver per Silver in play; what is left; token to others; not an Attack | `raid_gains_a_silver_per_silver_in_play_and_gives_the_others_the_minus_one_card_token`, `raid_cannot_be_blocked_by_a_moat_or_a_champion`, `raid_gains_what_is_left_when_there_are_not_enough_silvers` (new) |
| Seaway | Action up to $4, token to its pile (after the gain) | `seaway_gains_an_action_up_to_four_and_puts_the_buy_token_on_its_pile`, `seaway_puts_the_buy_token_on_the_knights_pile_when_a_knight_is_gained` (new, conditional on the Knights top card), `seaway_is_not_available_when_only_a_debt_cost_action_is_left` (new) |
| Seaway | Castles / Estates with Inheritance; token after on-gain effects (Innovation) | Estates: `seaway_can_gain_an_estate_...` (passes); Castles / Innovation not tested |
| Trade | up to 2; Silver per card trashed | `trade_trashes_up_to_two_cards_for_a_silver_each`, `trade_may_trash_nothing` |
| Lost Arts / Training / Pathfinding | token to an Action pile; bonus before the card | `lost_arts_training_and_pathfinding_move_their_tokens`, `a_token_on_a_pile_gives_its_bonus_each_time_a_card_from_it_is_played`, `tokens_apply_to_every_play_of_a_throne_roomed_card_and_not_to_other_piles` |
| Inheritance | once per game; non-Command non-Duration Action up to $4; Estates play it | `inheritance_sets_an_action_aside_and_makes_estates_play_it`, `inheritance_is_once_per_game`, `inheritance_does_not_offer_duration_or_command_cards_or_cards_over_four` (new) |
| Inheritance | token on the pile applies; card counts for scoring | `an_inherited_card_applies_the_token_on_its_pile_and_counts_for_scoring` (new) |
| Inheritance | other players' / Supply Estates, Estates in the trash, Landmarks at scoring, Courtier / Falconer types | not tested: the engine only turns the owner's hand Estates into Actions on their own turn; the rest need other sets' state |
| +1 Card token | draw before the card's other effects (Urchin example) | `the_plus_one_card_token_draws_before_the_cards_own_effects` |
| -$2 cost token | not below $0 | `ferrys_minus_two_cost_token_cheapens_a_pile_on_its_owners_turns` |
| Trashing token | Action piles only | `lost_arts_training_and_pathfinding_move_their_tokens` (piles offered) |
| -1 Card token | next draw one shorter, removed; empty deck; zero draw keeps it; Library skips a draw; Envoy | `the_minus_one_card_token_makes_the_next_draw_one_card_shorter_and_goes`, `the_minus_one_card_token_is_removed_even_by_a_draw_of_one_card_from_an_empty_deck`, `drawing_no_cards_does_not_remove_the_minus_one_card_token`, `the_minus_one_card_token_makes_a_draw_to_seven_skip_one_draw_and_end_at_seven` (new); Envoy (reveal then put into hand, not a draw): not tested (Promo card; the reveal path is the one `raze_is_not_affected_by_the_minus_one_card_token_and_replaces_it` covers) |
| -$1 token | next $, removed; not removed by +$0; applies to Treasures and Actions | `the_minus_one_coin_token_takes_a_coin_from_the_next_coins_and_goes`, `the_minus_one_coin_token_applies_to_a_card_that_gives_coins_and_not_to_a_buy`, `the_minus_one_coin_token_is_not_removed_by_a_miser_that_gives_nothing` (new), `the_minus_one_coin_token_still_takes_a_coin_after_the_player_spent_money_below_the_level_they_had` (new) |
| Journey token | starts up; turned over then checked; Pilgrimage/Giant/Ranger share it | `journey_token_flips_between_cards_and_events`, `ranger_draws_five_every_other_play` |
| Tokens | tokens on the Knights pile affect every Knight | `tokens_on_the_knights_pile_apply_to_every_knight` |
| Events | cost + a Buy, never gained, not affected by cost reduction; Treasures stop | `events_are_offered_with_the_cards_they_cost_a_buy_and_are_never_gained`, `buying_an_event_uses_a_buy_and_stops_the_treasures` |

## Speed opportunities (not implemented)

- `event_available` (`sets/adventures.rs:554-588`): Inheritance and Seaway scan `supply_cards()` with a filter on every Buy decision while the Event is in the game (about 17-30 pile tests per decision). Estimate under 1% of a Buy decision in games with those Events; cacheable per turn.
- `event_available`/`event_effect` chain of `cards::set_of(e) == ...` tests before the Adventures `match` (`adventures.rs:563-577`): up to 5 compares per Event per Buy decision; reorder so Adventures Events (ids below the other sets') are tested first or dispatch by `set_of` once.
- `settle_coin_tax` (`adventures.rs:737`) is `#[cold]` but called after every frame run while `tax_live` is set (a whole turn for a player holding the -$1 token); a flag check per frame, negligible.
- `gain_triggers` (`adventures.rs:422`) runs the Swamp Hag / Haunted Woods scan over the other players' `pending_durations` for every buy once either card is in the Supply; the per-gain cost could be gated by a per-game flag "a Hag/Woods is pending" (`GameState::adv` bit set by `shield_lingering_attack`, cleared when the last one resolves). Estimate: a few ns per gain, only in games with those cards.
- `offers_on_discard` is already gated by `g.trav == [0; 8]`; with Travellers it does eight `in_play.get` lookups per cleanup (could be a lane mask).
- `turn_start_entries` scans four ids when the Tavern mat is non-empty, once per turn: negligible.
