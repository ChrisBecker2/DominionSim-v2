# Menagerie audit

Scope: the 30 kingdom cards, the Horse, the 20 Events, the 20 Ways, the Exile mat and the bot defaults of Menagerie. Rules source: the offline wiki copy (each card's page, the `Way`, `Exile`, `Horse` and `Moat` pages).
Tests: `crates/engine/tests/menagerie_cards.rs` (C), `crates/engine/tests/menagerie_rulings.rs` (R, new: one test per wiki ruling that was not covered), `crates/sim/tests/menagerie_defaults.rs` (D).

Counts: 71 items checked (30 kingdom cards, the Horse, 20 Events, 20 Ways). 15 problems found, 9 fixed, 6 left open (each with an `#[ignore = "audit: ..."]` test; 2 of them are
the documented approximations of `docs/menagerie-plan.md` section 7 that cannot be made exact cheaply).

## Bugs fixed (test in brackets)

1. **Demand**: the Horse is gained and its reactions resolve before the card costing up to $4 is gained ("The Horse goes on top first, then the card"); the Horse's reaction frames used to sit under the card's gain frame, so a Sleigh could no longer move the Horse. Fix: push the card's gain frame first, then gain the Horse (menagerie.rs `event_effect`). [R/C `demand_resolves_the_horses_reactions_before_the_second_card_is_gained`]
2. **Alliance**: "gain a Province, a Duchy, ... in that order": the six cards were gained at once and their reactions resolved afterwards in reverse order. Now six single-card gain frames, one at a time. [`alliance_gains_one_card_at_a_time_so_a_reaction_to_the_first_comes_first`]
3. **Commerce**: same fault for the Golds. [`commerce_gains_its_golds_one_at_a_time`]
4. **Kiln + a Treasure that has follow-ups (Stockpile)**: Kiln's gain frame was pushed before the Treasure's own frames, so Stockpile exiled itself before the copy was gained ("you gain a Stockpile first, may discard Stockpiles from Exile, and then you Exile your played Stockpile"). The Kiln hook now runs at the end of `play_free_treasure_from` (engine.rs; this also puts it above Corsair's / Sauna's frames). [`kiln_gains_the_copy_of_a_stockpile_before_the_played_one_is_exiled`]
5. **Livery + Fisherman**: a Fisherman gained to an empty discard pile cost $2 when gained ("you will not gain a Horse"), but `cost_when_gained` read its cost after the card was in the discard pile ($5). [`livery_does_not_give_a_horse_for_a_fisherman_gained_while_it_cost_two`]
6. **Way + Enchantress** (Empires): "you can use a Way instead of getting the +1 Card and +1 Action that Enchantress's effect would give you"; playing the first Action as a Way did not use up the Enchantress effect, so the second Action was enchanted. Fix in `play_as_way`. [`a_way_used_on_the_first_action_uses_up_enchantresss_effect`]
7. **Way + Great Leader / Approaching Army** (Rising Sun): their "after playing an Action / an Attack" bonuses were skipped for a card played as a Way. Fix in `play_as_way` (calls `rs_after_play`). [`great_leader_gives_an_action_after_an_action_played_as_a_way`, `approaching_army_gives_a_coin_for_an_attack_played_as_a_way`]
8. **Goatherd as Way of the Mouse's card**: the trashes were counted only when Goatherd is a Supply pile (`M_TRASHES`), so the Mouse card Goatherd drew nothing. [`goatherd_as_way_of_the_mouses_card_counts_the_trashes`]
9. **Bot policy, Barge** (wiki Barge, Strategy: "Most of the time, it's best to play your Barges to draw this turn"): the bot kept the Barge for the next turn exactly when an Action was left to play (when drawing now is best). Now: draw now on my turn unless the deck holds fewer than 3 cards (a reshuffle). [D `barge_is_played_to_draw_now_even_when_actions_are_left`]

## Open issues (ignored tests)

1. **Conclave gives +1 Action before the card it plays** (nocturne.rs `Then::ConclavePlay`); the Snowy Village page says Snowy Village prevents Conclave's +1 Action, so it comes after the play. Needs a generic "after the play" bonus frame. [R `conclave_gives_no_action_after_it_played_snowy_village`, ignored] (Nocturne agent's file.)
2. **Groom's bonus comes before the gain's reactions and also for an exchanged card**: ruling "you get the bonuses after any effects triggered by gaining the card (Cemetery)" and "if you didn't gain the card (a when-would-gain ability) you get no bonus" (Trader). Needs a deferred bonus frame under the reactions (a `push_gain_guard` variant) and a way to know the gain was exchanged. [R `groom_gives_no_bonus_when_the_card_was_not_gained`, ignored]
3. **Moat is revealed after the Way is chosen, the wiki says before** (Moat page: "Other players must reveal Moat before ... deciding whether to play your Attack via a Way"; also Caravan Guard). The Attack window opens inside the card's own effect. Fix: open the window from `run_preplay` and let `preplay_normal` skip it; about 25 lines in effects.rs/menagerie.rs, and it changes the order of decisions in every game with a Way and Attacks, so not done. [R `moat_is_revealed_before_the_attacker_decides_whether_to_play_the_attack_as_a_way`, ignored]
4. **Way of the Chameleon** converts only the +Cards / +$ made at once (section 7). Later conversions (Cellar's draws, Priest's $2 per trash, Improve) happen in scattered `turn.coins +=` / draw frames of every set; not cheap to make exact. [R `way_of_the_chameleon_converts_a_cellars_later_draw_into_coins`, ignored]
5. **Wayfarer copies only the coin part of the last cost** (section 7): the cost model has a static Potion/Debt part per card (`cards::potion_cost`), used by the buy and gain legality everywhere; a dynamic one is not cheap. [R `wayfarer_costs_a_potion_after_a_card_with_a_potion_cost_was_gained`, ignored]
6. **Bot policy, Camel Train**: the wiki says its main use is to stock up on a card the deck wants several copies of; the bot Exiles the dearest card its strategy does not list (denial). Needs a value model for the gain list, so listed as a policy note. [D `camel_train_exiles_a_card_the_strategy_wants_several_copies_of`, ignored]

## Re-check of the approximations of `docs/menagerie-plan.md` section 7

* Way of the Chameleon, Wayfarer: open issues 4 and 5 above; not cheap.
* Populate (a decision per pile, piles counted when bought): equivalent except for piles that appear while it resolves (Divine Wind after an Omen played by Innovation); that needs re-counting piles after each gain: left.
* Invest (Invested copies told apart by count): Transport and a gain's discard take the non-Invested copies first. The player's choice would only matter for choosing the Invested copy to topdeck, which is never better; exact enough, left.
* Way of the Mouse's card: cards that need set-up are not offered (a conservative subset of the rule); exactness would need each set's set-up run for the card: left.
* Snowy Village: Villagers can only be converted before Snowy Village is played, not between its draw and its sentence; no observable difference (nothing can be decided in between).
* Not applied digital-only errata: unchanged. The current texts of Gamble, Reap, Way of the Mouse, Supplies and Stockpile (2025 / cosmetic changes) match the code.

## Other notes

* `event_available` makes Delay unbuyable with no Action in hand; the rules allow buying it for nothing (it only matters for "when you buy" effects). Left.
* Sleigh, Sheepdog, Falconer and Black Cat reactions are offered for at most 4 copies of the card in a hand (`.min(4)`); more copies in one hand are not seen.
* Kiln: a first Treasure that Highwayman (Allies) blocks does not use up the pending Kiln (engine.rs early return); very rare.
* Exile mat capacity: 20 different cards (`EXILE_CAP`); a full mat discards the card instead (documented in `put`).

## Speed opportunities (not implemented)

* effects.rs:180 tests `men.flags & M_WAYS` and then `turn.men.kiln` on every card played: two loads per play in every game; one combined byte (`men.live`) would be one. Estimate: below 0.5%.
* engine.rs:716 and :1021: a `turn.men.flags` load per Action-phase decision and per Action played; could be gated by the game flag `men.flags` (same cache line as the other Menagerie byte tests). Below 0.3%.
* menagerie.rs `men_gain_triggers` (about line 851-883): four separate `in_supply` bit tests (Black Cat, Falconer, Sleigh, Sheepdog) and hand lookups on every gain of a Menagerie game; a set-up mask `reactions_live` would skip all four in games with none. Games with Menagerie only: 2-3% of gain cost.
* menagerie.rs:683 `exile_from_supply` counts the mat's distinct cards on every Exile (`exile.iter().count()`): negligible (Exile is rare), but `SmallMultiset::len` exists.
* menagerie.rs `default_mouse`: scans every kingdom id twice at set-up (`pool().count()` then `nth`): set-up only, per game.
* crates/sim `way_values` / `own_value` recompute `cards::def` and hand scans for every Way option at every Way decision (each played Action in a Way game): about 20 table lookups and a hand walk per Action played; only in Way games.

## Policy notes (bot defaults against the wiki Strategy sections)

* Camel Train: see open issue 6.
* Village Green: the bot takes the bonus now only if an Action is in hand; the wiki says to keep it for the next turn "when you value reliability over continuing your current turn": consistent.
* Sanctuary / Bounty Hunter exile Victory cards and Curses first: matches "Exiling ... allows you to keep your starting VP" and "Exiling a Curse is worse than trashing" (a trasher is preferred when cursing: not modelled).
* Gatekeeper is bought like any 5 by the generic gain lists; the wiki calls it "one of the weakest cards from Menagerie": nothing to change.

## Cards checked

All against the current text, the FAQ and the clarifications; "OK" = no difference found.

| Card | Result |
|---|---|
| Black Cat | OK (reaction turn order, Curses in the turn player's order, Moat as normal) |
| Sleigh | OK (stop-moving, from the deck, gained to the hand) |
| Supplies | OK |
| Camel Train | OK (policy note) |
| Goatherd | fixed: as Way of the Mouse's card (bug 8) |
| Scrap | OK |
| Sheepdog | OK |
| Snowy Village | OK (Conclave: open issue 1) |
| Stockpile | fixed: with Kiln (bug 4) |
| Bounty Hunter | OK |
| Cardinal | OK |
| Cavalry | OK |
| Groom | open issue 2 |
| Hostelry | OK |
| Village Green | OK |
| Barge | OK; bot fixed (bug 9) |
| Coven | OK |
| Displace | OK |
| Falconer | OK |
| Gatekeeper | OK |
| Hunting Lodge | OK |
| Kiln | fixed (bug 4) |
| Livery | fixed with Fisherman (bug 5) |
| Mastermind | OK |
| Paddock | OK |
| Sanctuary | OK |
| Fisherman | OK |
| Destrier | OK |
| Wayfarer | open issue 5 (approximation) |
| Animal Fair | OK |
| Horse | OK |
| Delay | OK (note) |
| Desperation | OK |
| Gamble | OK |
| Pursue | OK |
| Ride | OK |
| Toil | OK |
| Enhance | OK |
| March | OK |
| Transport | OK |
| Banish | OK |
| Bargain | OK |
| Invest | OK |
| Seize the Day | OK |
| Commerce | fixed (bug 3) |
| Demand | fixed (bug 1) |
| Stampede | OK |
| Reap | OK |
| Enclave | OK |
| Alliance | fixed (bug 2) |
| Populate | OK |
| Way of the Butterfly | OK |
| Way of the Camel | OK |
| Way of the Chameleon | open issue 4 (approximation) |
| Way of the Frog | OK |
| Way of the Goat | OK |
| Way of the Horse | OK |
| Way of the Mole | OK |
| Way of the Monkey | OK |
| Way of the Mouse | OK (Goatherd: bug 8) |
| Way of the Mule | OK |
| Way of the Otter | OK |
| Way of the Owl | OK |
| Way of the Ox | OK |
| Way of the Pig | OK |
| Way of the Rat | OK |
| Way of the Seal | OK |
| Way of the Sheep | OK |
| Way of the Squirrel | OK |
| Way of the Turtle | OK |
| Way of the Worm | OK |
| The Ways rules (Enchantress, Great Leader, Approaching Army, Moat) | fixed bugs 6, 7; open issue 3 |

`choice_free` / `has_choice` / `order_sensitive` marks: checked against the CLAUDE.md rule; the per-set test `choice_free_and_order_sensitive_cards` lists them. Snowy Village `has_choice` (its play changes later +Actions), Kiln
`order_sensitive`, Goatherd / Scrap / Bounty Hunter / Displace / Hunting Lodge / Sanctuary `order_sensitive` (they look at or move the hand): all consistent. Black Cat `choice_free`
(its Curses are the opponents' gains) is fine.

Bot defaults: no illegal choice found (every `Pass` returned is for a Select with `min 0`: Sanctuary, Banish's name, Delay, Way of the Rat; Scrap, Bounty Hunter, Displace, Cardinal, Banish's copies are `min >= 1` and return a card).

## Ruling coverage

C = menagerie_cards.rs, R = menagerie_rulings.rs, D = sim menagerie_defaults.rs.

| Card | Ruling | Test |
|---|---|---|
| Black Cat | played on your turn: +2 Cards, no Curses | C black_cat_draws_two_and_curses_nobody_on_its_owners_turn |
| Black Cat | played out of turn: draw, then Curses in turn order from the turn's player | R black_cat_played_out_of_turn_curses_in_turn_order_starting_with_the_turns_player; C black_cat_in_hand_is_played_when_another_player_gains_a_victory_card |
| Black Cat | reacts to any other player's Victory gain, even on your turn (no Curses then) | R black_cat_may_be_played_on_its_owners_turn_when_another_player_gains_a_victory_card_and_curses_nobody |
| Black Cat | only Victory cards | C black_cat_does_not_react_to_a_card_that_is_not_victory |
| Black Cat | Moat as normal | R black_cat_played_out_of_turn_can_be_answered_with_moat_by_the_turns_player |
| Sleigh | moves the card from the discard pile although Sleigh was discarded on top | C sleigh_in_hand_puts_a_gained_card_onto_the_deck_or_into_the_hand |
| Sleigh | moves a card gained elsewhere (onto the deck) | R sleigh_moves_a_card_gained_directly_onto_the_deck |
| Sleigh | stop-moving rule | R sleigh_cannot_move_a_card_something_else_has_moved |
| Sleigh | gained to the hand: discards itself, cannot move itself back | R sleigh_gained_to_the_hand_discards_itself_but_cannot_move_itself_back |
| Sleigh | Sleigh A and B: A lost track | not testable: the reactions are counted when the gain happens, and a Sleigh reacting to its own gain has no observable effect |
| Sleigh | Cemetery: put into hand, then trash it | not testable: the order of the two on-gain effects is the engine's (own trigger first); same issue as open issue 2 |
| Supplies | $1 and a Horse onto the deck | C supplies_is_a_dollar_and_gains_a_horse_onto_the_deck |
| Camel Train | Exile non-Victory on play, a Gold when gained | C camel_train_exiles_a_non_victory_card_and_a_gold_when_gained |
| Goatherd | +1 Card per trash of the player to the right, trash optional | C goatherd_trashes_and_draws_for_the_trashes_of_the_player_to_the_right; R goatherd_draws_even_when_it_trashes_nothing |
| Goatherd | the right-hand player in a 3-player game | R goatherd_looks_at_the_player_on_its_right_in_a_three_player_game |
| Goatherd | the same card trashed twice counts twice | R goatherd_counts_each_trash_even_of_the_same_card_twice |
| Scrap | one different thing per $1, in order; 6+ gives all | C scrap_gives_one_different_thing_per_dollar_of_the_card_trashed; C scrap_with_a_six_cost_card_gives_all_six |
| Scrap | draw before gaining, so a drawn Reaction reacts | R scrap_draws_before_gaining_so_a_drawn_sheepdog_reacts_to_the_gain |
| Sheepdog | on buying | C sheepdog_in_hand_is_played_when_its_owner_gains_a_card |
| Sheepdog | on another player's turn, discarded at that Clean-up | R sheepdog_is_played_when_a_card_is_gained_on_another_players_turn_and_is_discarded_at_clean_up |
| Sheepdog | gained to the hand | R sheepdog_gained_to_the_hand_can_be_played_for_that_gain |
| Sheepdog | resolve Hostelry before or after Sheepdog | not testable: the order of two independent triggers is a player choice the engine fixes |
| Snowy Village | earlier +Actions stay | C snowy_village_ignores_the_actions_that_come_after_it |
| Snowy Village | Villagers give nothing after it | C a_villager_gives_nothing_after_snowy_village |
| Snowy Village | does not stop Throne Room | R snowy_village_does_not_stop_a_throne_room_from_playing_cards |
| Snowy Village | convert Villagers between the draw and the sentence | not testable: nothing can be decided there (see re-check) |
| Snowy Village | Champion | R a_champion_played_before_snowy_village_gives_its_action_once |
| Snowy Village | Great Leader | R great_leader_gives_nothing_after_snowy_village |
| Snowy Village | Conclave | R conclave_gives_no_action_after_it_played_snowy_village (ignored: open issue 1) |
| Stockpile | $3 +1 Buy, Exile it; buying another brings them back | C stockpile_exiles_itself_and_buying_another_brings_them_all_back |
| Stockpile | with Crown: $6, 2 Buys, Exiled once | R stockpile_played_twice_with_crown_gives_six_and_two_buys_and_is_exiled_once |
| Stockpile | after Kiln: gain, discard from Exile, then Exile | C kiln_gains_the_copy_of_a_stockpile_before_the_played_one_is_exiled |
| Stockpile | Mining Road | not tested: Mining Road plays a gained Treasure through the Plunder code; no Menagerie-side difference |
| Bounty Hunter | not optional; no card to Exile: no $3 | R bounty_hunter_with_an_empty_hand_gives_no_coins |
| Bounty Hunter | $3 only if it is the only copy in Exile | C bounty_hunter_gives_three_dollars_only_for_a_card_with_no_copy_in_exile; R bounty_hunter_exiling_the_first_copy_of_a_card_gives_three_coins_even_with_another_in_hand |
| Cardinal | the attacked player chooses between two eligible cards | C cardinal_exiles_a_card_costing_three_to_six_from_the_top_two_and_discards_the_rest |
| Cavalry | gained in the Buy phase: +2 Cards +1 Buy, back to Action | C cavalry_gains_two_horses_and_returns_to_the_action_phase_when_bought |
| Cavalry | other phase / other turn: +2 Cards +1 Buy only | C cavalry_gained_on_another_players_turn_draws_two_but_gives_no_buy; R cavalry_gained_in_the_action_phase_draws_and_gives_a_buy_but_stays_in_that_phase |
| Cavalry | gained onto the deck draws itself | R cavalry_gained_onto_the_deck_draws_itself |
| Cavalry | returning gives no Actions | R cavalry_returning_to_the_action_phase_gives_no_actions |
| Cavalry | Treasury / Changeling / Innovation interplay, start/end-of-phase abilities | not tested: generic return-to-Action-phase machinery shared with Villa (Empires tests) |
| Groom | a bonus per type; Mill gives two | C groom_gains_a_horse_for_an_action_a_silver_for_a_treasure_and_a_card_for_a_victory_card |
| Groom | no bonus if the card was not gained; bonuses after the gain's effects | R groom_gives_no_bonus_when_the_card_was_not_gained (ignored: open issue 2) |
| Hostelry | discard any number of Treasures for Horses; none needed | C hostelry_turns_treasures_into_horses_when_gained; R hostelry_discarded_treasures_are_discarded_not_trashed_and_need_not_be_discarded |
| Village Green | now / next turn | C village_green_now_gives_a_card_and_two_actions; C village_green_for_next_turn_stays_in_play_and_gives_its_bonus_then |
| Village Green | played when discarded, not in Clean-up | C village_green_is_played_when_discarded_outside_clean_up; C village_green_discarded_in_clean_up_is_not_offered |
| Village Green | several plays by Mastermind choose each time; stays if one was for next turn | R mastermind_playing_village_green_chooses_now_or_next_turn_each_time_and_stays_if_any_was_for_next_turn |
| Village Green | discarded from the deck (Cardinal), played out of turn | R village_green_discarded_from_the_revealed_cards_of_cardinal_may_be_played_out_of_turn |
| Village Green | Militia: discard all, then react | R village_green_discarded_to_militia_together_can_each_be_played |
| Village Green | Vassal loses track | R a_village_green_discarded_by_vassal_played_by_its_reaction_leaves_vassal_unable_to_play_it |
| Village Green | a reshuffle loses the other Village Greens | R a_reshuffle_caused_by_one_village_green_loses_the_other_one |
| Village Green | not for a card put in the discard pile without discarding | not testable: the engine only hooks `Act::Discard` (every other move is not a discard); no test card exists in Menagerie |
| Barge | now / next turn | C barge_now_draws_three_and_gives_a_buy; C barge_for_next_turn_draws_at_the_start_of_it |
| Barge | Mastermind plays: stays if one play was for next turn | R mastermind_playing_village_green_... (same code, `schedules_own_duration`) |
| Coven | in turn order one Exiles, another discards theirs | R coven_one_player_exiles_the_last_curse_the_next_discards_theirs; C coven_exiles_a_curse_for_each_other_player_and_when_there_is_none_they_discard_theirs |
| Coven | not a gain: Watchtower / Trader cannot avoid it | R coven_is_not_a_gain_so_watchtower_cannot_avoid_it |
| Displace | the gained card may cost less | R displace_may_gain_a_card_costing_less_than_the_one_exiled |
| Falconer | gains from the Supply only (no Horse) | R falconer_cannot_gain_a_horse |
| Falconer | reacts to any gain with 2+ types; to a gain to the hand | C falconer_is_played_when_a_card_with_two_types_is_gained_by_anyone; R falconer_gained_to_the_hand_can_be_played |
| Falconer | the turn's player reacts first | R falconer_reacts_in_turn_order_starting_with_the_turns_player |
| Falconer | Charlatan makes Curses two types | R falconer_counts_a_curse_as_two_types_with_charlatan |
| Falconer | one type: no reaction | R falconer_does_not_react_to_a_single_type_card |
| Falconer | Capitalism / Inheritance change the type count | not tested: needs Renaissance / Adventures project and Event state; `type_count` has the Capitalism branch |
| Gatekeeper | +$3 next turn, Exiles Actions / Treasures others gain | C gatekeeper_gives_three_dollars_next_turn_and_exiles_what_others_gain |
| Gatekeeper | Victory cards unaffected; copy on the mat lets you discard | C gatekeeper_gives_three_dollars_next_turn_and_exiles_what_others_gain |
| Gatekeeper | Sleigh beats it | R a_sleigh_beats_gatekeeper_by_moving_the_gained_card_first |
| Gatekeeper | two Horses: the first Exiled, the second not | R gatekeeper_exiles_the_first_of_two_horses_but_not_the_second |
| Gatekeeper | gained directly onto the deck is still Exiled | R gatekeeper_exiles_a_card_gained_directly_onto_the_deck |
| Gatekeeper | works on the turn it is played (Bargain's Horse) | R a_gatekeeper_exiles_the_horse_bargain_gives |
| Gatekeeper | ends when the next turn starts | C a_gatekeepers_attack_ends_when_its_owners_next_turn_starts |
| Gatekeeper | Moat | R a_moat_revealed_to_gatekeeper_protects_from_its_exiling |
| Gatekeeper | Nomad Camp + Watchtower does not lose track | not tested: it restates the stop-moving rule (covered by the Sleigh tests) |
| Hunting Lodge | draw, then discard the hand (drawn card included) for 5 | C hunting_lodge_may_discard_the_hand_for_five_cards |
| Kiln | gains a copy of the next card, before its instructions | C kiln_gains_a_copy_of_the_next_card_played |
| Kiln | Throne Room on Kiln | R a_throne_roomed_kiln_gains_a_kiln_with_the_second_play_and_still_copies_the_next_card |
| Kiln | before Moat | R kiln_gains_its_copy_before_the_victims_choose_to_reveal_moat |
| Kiln | Ways have no interaction | R kiln_still_gains_a_copy_when_the_card_is_played_as_a_way |
| Kiln | Stockpile order | C kiln_gains_the_copy_of_a_stockpile_before_the_played_one_is_exiled |
| Kiln | a Livery gained with Kiln gives no Horse | R a_livery_gained_with_kiln_does_not_give_a_horse |
| Kiln | +1 Card token, Sheepdog / Conspirator / Innovation | not tested: only restate that independent effects may be ordered; the engine fixes one order |
| Livery | cumulative | R livery_is_cumulative; C livery_gains_a_horse_for_each_card_costing_four_or_more_gained_this_turn |
| Livery | cost as it is gained (Fisherman) | C livery_does_not_give_a_horse_for_a_fisherman_gained_while_it_cost_two (menagerie_cards.rs) |
| Livery | Destrier | C livery_checks_destriers_cost_as_it_is_gained |
| Livery | Innovation / Disciple / Specialist | not tested: Disciple / Specialist are Travellers outside the test kingdoms; same code path as the Kiln test (the Livery count is read at the gain) |
| Mastermind | plays an Action three times next turn | C mastermind_plays_an_action_three_times_next_turn |
| Mastermind | Horse: returned once | R mastermind_playing_a_horse_three_times_returns_it_once |
| Mastermind | plays another Mastermind: both stay | R a_mastermind_that_plays_another_mastermind_keeps_both_in_play |
| Mastermind | via Throne Room: does not stay | R a_mastermind_that_plays_throne_room_which_plays_a_duration_does_not_stay |
| Mastermind | the Duration removed by Way of the Horse | R a_mastermind_whose_duration_is_removed_from_play_by_way_of_the_horse_does_not_stay |
| Paddock | Actions per empty Supply pile, counted after the Horses | C paddock_gains_two_horses_and_gives_an_action_per_empty_pile |
| Paddock | the Horse pile is not a Supply pile | R paddock_with_an_empty_horse_pile_still_gives_its_coins |
| Paddock | Changeling exchange empties a pile | not tested: needs the Changeling exchange of Horses; the count is read after the Horses are gained |
| Sanctuary | Exile optional | C sanctuary_may_exile_a_card_from_the_hand |
| Fisherman | $3 less on the turn player's turns with an empty discard pile; two cannot both cost $2 | C fisherman_and_destrier_cost_less_on_your_turn; R two_fishermen_cannot_both_be_bought_for_two_dollars; R fisherman_looks_at_the_discard_pile_of_the_player_whose_turn_it_is |
| Destrier | $1 less per card the turn's player gained (Sleigh + Workshop; not Curses given) | R destrier_costs_less_for_horses_gained_but_not_for_curses_given_to_others |
| Destrier | Stonemason overpay: the 2nd card cannot also be a Destrier | not tested: overpay flow; the cost is read when each card is gained (covered by the Livery / Destrier test) |
| Wayfarer | +3 Cards, may gain a Silver | C wayfarer_draws_three_and_may_gain_a_silver |
| Wayfarer | costs the last other card gained, by anyone; resets each turn; a Wayfarer gained does not change it | R wayfarer_costs_what_the_last_other_card_gained_did |
| Wayfarer | Bridge applies to the other card only | R bridge_reduces_wayfarers_cost_only_through_the_card_gained |
| Wayfarer | follows Fisherman's cost as it changes | R wayfarer_follows_the_cost_of_a_fisherman_as_it_changes |
| Wayfarer | Exiling does not change it | R exiling_a_card_does_not_change_wayfarers_cost |
| Wayfarer | Animal Fair bought by trashing: $7 | R buying_animal_fair_by_trashing_an_action_sets_wayfarers_cost_to_seven |
| Wayfarer | Potion / Debt in the cost | R wayfarer_costs_a_potion_after_a_card_with_a_potion_cost_was_gained (ignored: open issue 5) |
| Wayfarer | Stonemason trash; Family of Inventors token | not tested: Stonemason's gains read `cost` live (covered by the cost tests); Allies tokens are in `men_base_cost` (`fam`) without a test |
| Animal Fair | $4, +1 Buy per empty pile | C animal_fair_gives_four_dollars_and_a_buy_per_empty_pile |
| Animal Fair | trash an Action instead of paying; needs an Action; uses a Buy | C animal_fair_can_be_bought_by_trashing_an_action; R animal_fair_cannot_be_bought_by_trashing_without_an_action_in_hand |
| Animal Fair | Haggler / Rats trash ordering | not tested: both orders are fixed (trash, then gain) and are in `buy_alt`; no assertion beyond that |
| Horse | +2 Cards +1 Action, returns to its pile; played twice returns once | C the_horse_is_two_cards_and_an_action_and_returns_to_its_pile; C a_horse_played_twice_gives_the_cards_each_time_but_returns_once |
| Horse | non-Supply: cannot be gained by Supply gains | C a_gain_from_the_supply_cannot_take_a_horse; R falconer_cannot_gain_a_horse |
| Horse | none left: the gain fails | R gaining_a_horse_from_an_empty_pile_fails |
| Delay | set aside, played at the start of next turn, uses no Action; optional | C delay_sets_aside_an_action_and_plays_it_next_turn; R a_card_set_aside_by_delay_is_played_without_using_an_action; R delay_may_set_nothing_aside |
| Desperation | once per turn, Curse gained -> +1 Buy +$2; Curse pile empty -> nothing | C desperation_gains_a_curse_once_per_turn_for_a_buy_and_two_dollars; R desperation_with_no_curse_left_gives_nothing |
| Desperation | gained then removed (Watchtower / Trader) still gives the bonus; Possession | not tested: the bonus runs after the gain frame whatever the reactions do; Possession is not part of the game |
| Gamble | +1 Buy, discard the top, may play an Action / Treasure, no Action used | C gamble_may_play_the_top_card_if_it_is_an_action_or_treasure; R gamble_discards_a_card_that_is_neither_action_nor_treasure_and_may_play_an_action |
| Pursue | naming a card not in the game discards all four | R pursue_naming_a_card_that_is_not_there_discards_all_four |
| Ride | gain a Horse | C ride_gains_a_horse |
| Toil | +1 Buy, play an Action without using one | C toil_plays_an_action_from_the_hand_and_march_from_the_discard_pile; R toil_and_march_play_the_action_without_using_one |
| Enhance | must gain after trashing | R enhance_must_gain_after_trashing |
| March | play from the discard pile, no Action used | R toil_and_march_play_the_action_without_using_one |
| Transport | Exile an Action card of the Supply (not pile); put an Exiled Action on the deck, from any source | R transport_can_exile_an_action_card_of_a_pile_that_is_not_all_actions; R transport_puts_an_exiled_action_onto_the_deck_even_one_exiled_by_another_card |
| Banish | any number with the same name | C banish_exiles_all_copies_of_a_card_from_the_hand |
| Bargain | non-Victory up to $5; each other player gains a Horse in turn order, cannot decline | C bargain_gains_a_non_victory_card_and_gives_the_others_a_horse; R bargain_gives_a_horse_to_each_other_player |
| Invest | cumulative | R invest_is_cumulative |
| Invest | another player Investing in a copy draws too | R a_player_investing_in_the_same_card_draws_the_first_investors_cards |
| Invest | draws when another player gains a copy | C invest_draws_two_cards_when_another_player_gains_the_exiled_card |
| Invest | Transport takes a non-Invested copy | R transport_takes_a_copy_not_exiled_by_invest_and_the_investment_stays |
| Invest | discarding from the mat ends the draws | C exile_discard_on_gain_conserves_cards (the copies leave the mat, `exile_remove` drops the Invested ones) |
| Seize the Day | extra turn, not counted for the tie-break | R the_extra_turn_of_seize_the_day_does_not_count_for_the_tie_break; C seize_the_day_is_once_per_game_and_gives_an_extra_turn |
| Seize the Day | no check of the previous turn | R seize_the_day_does_not_check_whether_the_previous_turn_was_an_extra_turn |
| Commerce | count first, then gain | C commerce_gains_a_gold_per_differently_named_card_gained; R commerce_counts_the_differently_named_cards_before_gaining_any_gold |
| Demand | Horse first, then the card, both onto the deck | C demand_gains_a_horse_and_a_card_onto_the_deck; C demand_resolves_the_horses_reactions_before_the_second_card_is_gained |
| Demand | Destrier reduced by the Horse | not tested: covered by destrier_costs_less_for_horses_gained... (the cost is live) |
| Stampede | 5 or fewer cards in play; as many Horses as are left | C stampede_puts_five_horses_on_the_deck_with_five_or_fewer_cards_in_play; R stampede_gains_as_many_horses_as_are_left |
| Reap | Gold set aside, played at the start of next turn | C reap_gains_a_gold_and_plays_it_next_turn; C reap_plays_its_gold_at_the_start_of_the_next_turn |
| Enclave | Gold and a Duchy Exiled from the Supply | C enclave_gains_a_gold_and_exiles_a_duchy |
| Alliance | each present in the Supply, in order | C alliance_gains_six_cards; R alliance_gains_only_the_cards_still_in_the_supply; C alliance_gains_one_card_at_a_time_so_a_reaction_to_the_first_comes_first |
| Populate | top of each Action Supply pile; no Horse / Stockpile; empty pile skipped; mixed piles | C populate_gains_one_of_each_action_supply_pile; R populate_gains_the_top_card_of_a_mixed_pile_and_skips_empty_and_non_action_piles |
| Populate | Cavalry returns to the Action phase but Populate continues | R populate_finishes_gaining_after_a_cavalry_returns_it_to_the_action_phase |
| Populate | Inheritance / Enlightenment do not change piles; new piles mid-resolution | not tested: the pile type is the pile's card (Estate stays a Victory pile); new piles mid-resolution is the documented approximation |
| Ways (rules) | the Way replaces the card's effect; none = the card | C a_way_is_offered_when_a_card_is_played_and_none_leaves_the_card_alone; C a_way_played_card_is_not_the_card |
| Ways (rules) | choose per play (Throne Room, Mastermind) | C throne_room_plays_a_card_twice_each_with_its_own_way |
| Ways (rules) | an Attack played as a Way attacks nobody | R an_attack_played_as_a_way_attacks_nobody |
| Ways (rules) | the Way choice comes after Moat / Kiln | R moat_is_revealed_before_the_attacker_decides_whether_to_play_the_attack_as_a_way (ignored: open issue 3); Kiln: R kiln_still_gains_a_copy_when_the_card_is_played_as_a_way |
| Ways (rules) | a Duration stays only if one play was its own | R a_duration_played_as_a_way_does_not_stay_in_play_or_come_back; R a_duration_played_twice_stays_if_one_of_the_plays_was_its_own |
| Ways (rules) | usable on cards played at unusual times (Sheepdog) | R sheepdog_played_as_a_reaction_may_be_played_as_a_way |
| Ways (rules) | Enchantress | C a_way_used_on_the_first_action_uses_up_enchantresss_effect |
| Ways (rules) | Adventures tokens still apply | R a_way_leaves_the_adventures_minus_one_card_token_in_force |
| Ways (rules) | Ways are not cards | R ways_are_not_cards_pursue_cannot_name_one |
| Ways (rules) | at most one Way dealt, landscapes capped | C random_menagerie_games_conserve_cards_and_terminate (the dealer is in `cards::pseudo_cards_of`) |
| Way of the Butterfly | returns a Horse (non-Supply pile); a card with no pile fails | R way_of_the_butterfly_returns_a_horse_and_cannot_return_a_card_with_no_pile |
| Way of the Butterfly | exactly $1 more; no card costing that: nothing | C way_of_the_butterfly_returns_the_card_for_one_costing_exactly_one_more; R way_of_the_butterfly_gains_nothing_when_no_pile_costs_exactly_one_more |
| Way of the Butterfly | Black Market | not testable: no Black Market in the project |
| Way of the Camel / Worm | Exile a Gold / Estate from the Supply | C way_of_the_camel_and_worm_exile_from_the_supply |
| Way of the Chameleon | +Cards <-> +$; Sheepdog, Merchant Ship | C way_of_the_chameleon_swaps_cards_and_coins; R way_of_the_chameleon_turns_a_sheepdogs_cards_into_coins_and_a_merchant_ships_coins_into_cards |
| Way of the Chameleon | draw-to-6 not changed; Enchantress still applies | R way_of_the_chameleon_leaves_draw_to_six_alone_and_enchantress_still_applies |
| Way of the Chameleon | later effects (Cellar, Priest, Improve) | R way_of_the_chameleon_converts_a_cellars_later_draw_into_coins (ignored: open issue 4) |
| Way of the Chameleon | Governor, Harbor Village, Lantern, Elder, Reckless, -Cards token, Poor House / Souk | not tested: they need the other set's state; the Chameleon code path touches only `def.cards` / `def.coins` and the Draw frames of the play |
| Way of the Frog | +1 Action, topdeck when discarded from play | C way_of_the_frog_puts_the_card_on_the_deck_when_discarded_from_play |
| Way of the Frog | Throne-roomed Duration does not topdeck | R way_of_the_frog_on_a_throned_duration_does_not_topdeck_it |
| Way of the Goat | trash a card from the hand | C way_of_the_goat_trashes_a_card_from_the_hand |
| Way of the Horse | +2 Cards +1 Action, return; a card with no pile still gives the cards | C way_of_the_horse_returns_the_card_to_its_pile; R way_of_the_horse_gives_the_cards_even_when_the_card_has_no_pile_to_return_to |
| Way of the Horse | a card played from the trash does not move | R way_of_the_horse_on_a_card_played_from_the_trash_does_not_move_it |
| Way of the Horse | Black Market | not testable: no Black Market in the project |
| Way of the Mole | draws 3 even with nothing to discard | R way_of_the_mole_draws_three_with_an_empty_hand; C way_of_the_mole_discards_the_hand_and_draws_three |
| Way of the Monkey / Mule / Otter / Ox / Pig / Sheep | their bonuses | C simple_ways_give_their_bonuses |
| Way of the Mouse | plays the set-aside card, leaving it there; not offered on that card | C way_of_the_mouse_plays_the_set_aside_card; R way_of_the_mouse_plays_the_set_aside_card_which_stays_set_aside_and_is_not_offered_the_mouse |
| Way of the Mouse | the set-aside card cannot move itself | R (same test: the card is not in play); Ratcatcher: not tested, needs Adventures set-up |
| Way of the Owl | draw to 6; nothing at 6+ | C simple_ways_give_their_bonuses; R way_of_the_owl_draws_nothing_with_six_cards_or_more |
| Way of the Rat | discard a Treasure for a copy; Supply only | C way_of_the_rat_may_discard_a_treasure_for_a_copy_of_the_card; R way_of_the_rat_cannot_gain_a_copy_of_a_card_that_is_not_in_the_supply |
| Way of the Seal | +$1, gains may go to the deck; not if moved | C way_of_the_seal_lets_gains_go_onto_the_deck; R way_of_the_seal_cannot_move_a_card_that_something_else_moved |
| Way of the Squirrel | +2 Cards at the end of the turn, also out of turn | C way_of_the_squirrel_draws_at_the_end_of_the_turn; R way_of_the_squirrel_played_on_another_players_turn_draws_at_the_end_of_that_turn |
| Way of the Turtle | set aside, played next turn, can repeat | C way_of_the_turtle_sets_the_card_aside_for_next_turn; C a_card_set_aside_by_way_of_the_turtle_is_played_at_the_start_of_the_next_turn; R a_card_played_at_the_start_of_a_turn_by_way_of_the_turtle_can_be_set_aside_again |
| Way of the Turtle | Throne Room twice: one play; Throne Room does not stay | R throne_room_and_way_of_the_turtle_twice_sets_the_card_aside_once |
| Way of the Turtle | Reserve called at the start of the turn; Throne Room + Duration once + Turtle | not tested: the Reserve case needs a Tavern Reserve in play at a start-of-turn; the Duration case is the Mastermind / Throne Room multiplier code (tested above) |
| Exile (rules) | cards on the mat are yours (score, count) | C exiled_cards_count_for_scoring_and_belong_to_their_player |
| Exile (rules) | gaining a copy: discard all or none | C the_copies_on_the_mat_may_be_left_there_when_a_copy_is_gained; C exile_discard_on_gain_conserves_cards |
| Exile (rules) | Exiling is not gaining (no triggers) | C camel_train_exiles_a_non_victory_card_and_a_gold_when_gained |

Coverage count: 117 rulings listed; 85 covered by a passing test, 5 by an ignored test (open issues 1-5), 1 policy ignored test (D), 26 not tested with a reason (all rows marked "not tested" / "not testable").
