# Audit: Rising Sun and Renaissance

Scope: `crates/engine/src/sets/{rising_sun,renaissance}.rs`, their bot defaults, tests and the generic machinery they call. Rules source: the offline wiki copy
(card pages, FAQ / Official FAQ / Other rules clarifications, the Shadow / Omen / Prophecy / Artifact / Project / Villager / Coffers pages).
Divine Wind is deliberately disabled (`todo`); its code was read anyway.

Test files: `crates/engine/tests/{rising_sun,renaissance}_cards.rs`, `crates/sim/tests/{rising_sun,renaissance}_defaults.rs`. Branch `worktree-agent-aac9f5e80ce0a9a99`.

## Bugs fixed (each with its test)

| # | Bug | Rule clause | Fix | Test |
|---|---|---|---|---|
| 1 | Panic left Loot Treasures in the discard pile | Panic FAQ: "Loot is returned to the top of the pile" | `rs_panic_cleanup` also returns `is_loot` cards | `panic_returns_a_loot_treasure_to_the_loot_pile` |
| 2 | Foresight could not be bought with no Action in the deck / discard pile | Foresight: reveal until an Action (the whole deck is revealed and discarded) | `event_available(FORESIGHT)` is always true | `foresight_can_be_bought_with_no_action_in_the_deck_and_reveals_the_whole_deck` |
| 3 | Approaching Army's $1 and Great Leader's +1 Action were skipped when the card was played as a Way | Approaching Army FAQ: "You get the +$1 even if ... you used a Way"; Great Leader: after each Action card you play | `menagerie.rs::play_as_way` calls `rs_after_play` (shared file, 4 lines) | `approaching_army_gives_its_coin_even_when_the_attack_was_played_as_a_way`, `great_leader_gives_its_action_after_a_card_played_as_a_way` |
| 4 | Shadow cards were not put on the bottom when Annex, Donate, Inn or Famine shuffled | Shadow: "When shuffling Shadow cards, put them on the bottom" | `rs_shuffled(p)` called at the 4 shuffle sites (empires.rs x2, hinterlands.rs, nocturne.rs: one line each) | `a_shadow_card_goes_to_the_bottom_when_annex_shuffles_the_discard_pile_into_the_deck`, `..._when_inn_shuffles_actions_into_the_deck` (Donate and Famine use the same helper, untested) |
| 5 | Riverboat's card was set aside without its set-up (Bandit Camp / Pillage had no Spoils, Joust no Rewards, Baker no Coffers; Liaisons, Pooka, Ferryman, Wealthy Village could not work) | Riverboat FAQ: "If that card also requires setup, do that setup too" | `setup` does the Spoils / Rewards / Baker set-up; the cards whose set-up cannot be done are not chosen | `riverboats_card_gets_the_set_up_it_needs_or_is_not_chosen` |
| 6 | Divine Wind could deal a Liaison without an Ally | Divine Wind FAQ: deal an Ally with a Liaison | Liaisons are not dealt (disabled card) | `divine_wind_never_deals_a_liaison_it_could_not_set_up` |
| 7 | Capitalism ignored the Allies and Plunder Actions with a +$ amount | Capitalism: "Cards that Capitalism affects" list | 26 names added to `CAPITALISM_NAMES` | `capitalism_affects_every_action_the_wiki_lists` |
| 8 | The Key taken while the start of the turn is being resolved (Treasurer played by Piazza) gave no $1 | Treasurer: "If you play Treasurer at the start of your turn (with e.g. Piazza) and take the Key, you immediately get +$1" | `take_artifact` | `a_treasurer_played_at_the_start_of_the_turn_gives_the_key_at_once` |
| 9 | A card set aside by a Cargo Ship that had left play (Improve trashed it) was brought to the hand | Improve: "the set-aside card will be stranded and never be put in your hand" | `ren_yes(CargoAside)` only schedules the return when a Cargo Ship is in play | `a_cargo_ship_trashed_by_improve_leaves_the_card_it_sets_aside_stranded` |
| 10 | Bot: Sickness discarded a whole small hand (even three Golds) instead of taking the Curse | policy | `defaults/rising_sun.rs` compares the worst three cards only | `sickness_does_not_make_the_bot_discard_a_small_hand_of_good_cards` |
| 11 | Bot: Border Guard took the Lantern before the Horn | Border Guard strategy: "it is usually best to take the Horn first" | `defaults/renaissance.rs` | `border_guard_takes_the_horn_before_the_lantern` |

Changes to shared files (small): `menagerie.rs` (4 lines), `empires.rs` (2 lines), `hinterlands.rs` (1), `nocturne.rs` (1). `rs_shuffled` lives in `rising_sun.rs`.

## Open issues (ignored tests)

| Issue | Test (`#[ignore = "audit: ..."]`) |
|---|---|
| Events that can do nothing right now (Amass with an Action in play, Kintsugi with an empty hand, Sea Trade with no Action in play ...) are not offered; the wiki treats buying them as legal | `amass_with_an_action_in_play_may_be_bought_and_gains_nothing` |
| With Capitalism a Fishmonger in the deck should be playable in the Buy phase like a Treasure (Fishmonger FAQ) | `fishmonger_can_be_played_from_the_deck_in_the_buy_phase_with_capitalism` |
| Highwayman is checked before Enlightenment: the first Treasure played in the Action phase does nothing, the wiki says Highwayman can't stop its +1 Card +1 Action | `highwayman_cannot_stop_the_first_treasure_played_in_the_action_phase_under_enlightenment` |
| Star Chart is only offered for a draw's shuffle, not Annex, Donate, Inn, Famine (known gap in the plan doc) | `star_chart_is_offered_for_the_shuffle_of_annex` |
| Policy: Cargo Ship never sets aside a Victory card while greening ("During greening it can also be useful to set aside the Victory cards to reduce your odds of dudding") | `cargo_ship_sets_aside_a_victory_card_while_greening` |
| Harsh Winter on Loot / Rewards: `PILE_TOKEN_CAP` is 32 piles with tokens and `add_debt` asserts when it is exceeded; Loot is not given a Debt pile (wiki: Loot and Rewards are piles that can have Debt) | none (not reachable in normal games; needs 33 different piles gained from) |
| Foresight keeps at most `FORESIGHT_CAP` = 4 different cards; a 5th is discarded instead of kept | none (needs 5 Foresight buys in one turn) |

## Cards checked

Rising Sun kingdom cards (25): Mountain Shrine OK; Daimyo OK; Artist OK; Fishmonger OK (Capitalism-from-deck open); Snake Witch OK; Aristocrat OK; Craftsman OK;
Riverboat fixed (#5); Root Cellar OK; Alley OK; Ninja OK; Poet OK; River Shrine OK; Rustic Village OK; Gold Mine OK; Imperial Envoy OK; Kitsune OK; Litter OK;
Rice Broker OK; Ronin OK; Tanuki OK; Tea House OK (mark: choice-free is slightly optimistic when the last Sun token is removed with Kind Emperor, whose gain is a
decision; the card is still played correctly, only the bot's shortcut); Samurai OK; Rice OK; Change OK (tests in `empires_cards.rs`).
Events (10): Amass OK (offered with an Action in play since 2026-10-28), Asceticism OK, Continue OK, Credit OK, Foresight fixed (#2), Gather OK, Kintsugi OK, Practice OK, Receive Tribute OK, Sea Trade OK.
Prophecies (15): Approaching Army fixed (#3), Biding Time OK, Bureaucracy OK, Divine Wind (disabled; #6), Enlightenment OK (Highwayman open), Flourishing Trade OK,
Good Harvest OK, Great Leader fixed (#3), Growth OK, Harsh Winter OK (caps noted), Kind Emperor OK, Panic fixed (#1), Progress OK, Rapid Expansion OK, Sickness OK.
Omen, Shadow, Sun token set-up rules: OK (Shadow shuffles fixed, #4).

Renaissance kingdom cards (25): Border Guard OK; Ducat OK; Lackeys OK (gain only); Acting Troupe OK; Cargo Ship fixed (#9); Experiment OK; Improve OK; Flag Bearer OK;
Hideout OK; Inventor OK; Mountain Village OK; Patron OK; Priest OK; Research OK; Silk Merchant OK; Old Witch OK; Recruiter OK; Scepter OK; Scholar OK; Sculptor OK;
Seer OK; Spices OK; Swashbuckler OK; Treasurer fixed (#8); Villain OK.
Artifacts (5): Flag, Horn, Key (#8), Lantern, Treasure Chest OK.
Projects (20): Cathedral, City Gate, Pageant, Sewers, Star Chart (gap open), Exploration, Fair, Silos, Sinister Plot, Academy, Capitalism (#7), Fleet, Guildhall, Piazza,
Road Network, Barracks, Crop Rotation, Innovation, Canal, Citadel all OK.
Villagers / Coffers rules: OK.

Total: 25 + 10 + 15 + 25 + 5 + 20 = 100 cards, Events, Prophecies, Artifacts and Projects checked; 9 engine bugs found and fixed, 2 bot policy errors fixed; 7 open issues
(5 with ignored tests).

## Policy notes (bot defaults)

* Border Guard (fixed): wiki "it is usually best to take the Horn first, and only take the Lantern instead if you strongly want to find a specific card".
* Sickness (fixed): see #10.
* Cargo Ship (open, ignored test): never sets aside Victory cards; wiki "During greening it can also be useful to set aside the Victory cards to reduce your odds of dudding".
* Kitsune takes the Curse option first whenever Curses remain; wiki "In Kingdoms with little to no trashing and/or draw, Kitsune's junking attack as a curser can be powerful",
  "If the Kingdom does not have a risk of becoming a Slog, Kitsune can still see value as a dual village and +$2" - the bot does not weigh trashing / draw in the kingdom. Left as is.
* Snake Witch: always returns when the hand has no duplicates and Curses are left; reasonable.
* Searching bots (`search_play`) choose modes by their search, so these defaults matter for non-searching bots and playouts (tests use `rule_mode`).

## Computational opportunities

* `shadow_in_deck` / `push_shadow_choices` / `plays_from_deck` (`rising_sun.rs`): in a game with a Shadow card, every Action-phase decision does up to 5 linear scans of `deck_known`, `deck_unknown` and `deck_known_bottom`
  (`PlayerState::deck_has`). A per-player Shadow-in-deck counter kept by the shuffle / gain / draw paths would make it O(1). Estimate: a few hundred ns per decision in Shadow games only.
* `default_riverboat` (`rising_sun.rs`) walks all zone ids twice (`pool().count()` then `.nth`) per Riverboat game: one pass into a small array saves ~10-20 us per game; irrelevant elsewhere.
* `Panic`: `rs_panic_cleanup` calls `has_pile_of` (several range checks and a bit test) per Treasure in play at every cleanup under Panic; negligible, only with that Prophecy.
* `Harsh Winter` gain hook: `has_pile_of` plus a linear `PileTokens::slot` search per gain; only with that Prophecy.
* `capitalism_card` uses a `OnceLock` table lookup per `is_treasure` call under Capitalism (only when `REN_CAPITAL`); fine.
* `Rice::rice_coins` iterates the cards in play and `is_gathering` per card; only when a Rice is played.
* `actions_in_play` iterates `in_play` with `is_action` (Sea Trade, Amass, `event_available` for both on every Buy decision when those Events are in the game: it is called by the availability check, so once per Buy decision per Event).
* Renaissance `turn_start_entries` runs a loop over 8 Project ids each turn when a Project is owned; a precomputed mask AND would skip it (tiny).

## Ruling coverage

(RS = `rising_sun_cards.rs`, REN = `renaissance_cards.rs`.)

### Rising Sun cards

| Card | Ruling | Test |
|---|---|---|
| Mountain Shrine | Costs 5 Debt | RS `debt_costs_take_the_tokens_when_the_card_is_bought` |
| Mountain Shrine | Any Action in the trash counts, whoever trashed it, also the one just trashed | RS `mountain_shrine_trashes_a_card_and_draws_two_if_the_trash_has_an_action` |
| Mountain Shrine | Fortress / Lich leave the trash before the check | RS `mountain_shrine_checks_the_trash_after_a_fortress_has_left_it` (Lich: not tested, same path) |
| Daimyo | Not optional; replays even a card that trashed itself | RS `a_daimyo_replay_of_mining_village_cannot_trash_another_mining_village`, `daimyo_replays_the_next_non_command_action_once_for_each_daimyo` |
| Daimyo | Command cards are not replayed; waits for a non-Command Action | RS `daimyo_waits_for_the_next_action_even_across_non_actions` |
| Daimyo | Two Daimyos + Craftsman = three plays | RS `daimyo_replays_the_next_non_command_action_once_for_each_daimyo` |
| Daimyo | Costs 6 Debt | RS `debt_costs_take_the_tokens_when_the_card_is_bought` |
| Artist | Counts itself with one Artist; counts earlier-turn Durations | RS `artist_draws_a_card_per_card_with_exactly_one_copy_in_play`, `artist_counts_the_cards_that_stayed_in_play_from_earlier_turns` |
| Fishmonger | +1 Buy, +$1 | RS `fishmonger_gives_a_buy_and_a_coin` |
| Fishmonger | With Capitalism playable from the deck in the Buy phase | RS `fishmonger_can_be_played_from_the_deck_in_the_buy_phase_with_capitalism` (ignored, open) |
| Snake Witch | Reveal is optional; return to pile, then each other player gains a Curse | RS `snake_witch_returns_to_its_pile_to_curse_the_others_when_the_hand_has_no_duplicates` |
| Snake Witch | Cannot return -> no Curse | RS `a_throne_roomed_snake_witch_curses_only_once` |
| Snake Witch | Reveal after the +1 Card; empty hand has no duplicates | RS `snake_witch_does_nothing_with_a_duplicate_in_the_hand_and_may_reveal_an_empty_hand` |
| Aristocrat | Counts Aristocrats in play, not plays; 0 / 9 / 10 nothing | RS `aristocrat_depends_on_the_aristocrats_in_play`, `aristocrat_counts_the_aristocrats_in_play_not_the_plays_this_turn` |
| Aristocrat | Command variants leave it where it is: nothing | RS `a_band_of_misfits_imitating_aristocrat_finds_no_aristocrat_in_play` |
| Craftsman | Gains even with Debt | RS `craftsman_takes_debt_and_gains_a_card_even_with_debt` |
| Riverboat | Set-up choice (non-Duration 5 not in the game), set-up of that card | RS `riverboat_sets_aside_a_five_cost_action_in_set_up_and_plays_it_at_the_start_of_the_next_turn`, `riverboats_card_gets_the_set_up_it_needs_or_is_not_chosen` |
| Riverboat | Split-pile 5s and Knights as choices | RS `riverboat_can_choose_a_card_of_a_split_pile_and_a_knight_only_without_the_knights` |
| Riverboat | Plays the card at the start of next turn, leaving it set aside | RS `riverboat_sets_aside_a_five_cost_action_...` |
| Riverboat | Not a Command; chosen card has no pile (Wild Hunt cannot gather) | not tested: Riverboat plays through its own `PlayEffects`, no Command path exists; Wild Hunt pile VP is a Plunder / Empires rule with no pile for the set-aside card (no code path adds VP to it) |
| Riverboat | Stays in play as long as the played card would | not tested: only non-Duration Actions can be chosen, so it never outlives its turn |
| Root Cellar, Imperial Envoy, Litter | Debt even when already in Debt | RS `root_cellar_imperial_envoy_and_litter_take_debt` |
| Litter | Counters Possession | not testable: Possession is not implemented |
| Alley | Draw, +Action, discard a card; cannot discard a Shadow of the deck | RS `alley_discards_a_card_after_drawing_and_ninja_cuts_hands_to_three`, `an_alley_played_from_the_deck_discards_a_card_of_the_hand_not_a_shadow_of_the_deck` |
| Ninja | +1 Card, others discard down to 3 | RS `alley_discards_a_card_after_drawing_and_ninja_cuts_hands_to_three` |
| Poet | Debt-cost cards are not "$3 or less"; goes back on top | RS `poet_takes_the_top_card_into_the_hand_if_it_costs_three_or_less` |
| River Shrine | Only Buy-phase gains count | RS `river_shrine_trashes_up_to_two_and_gains_in_cleanup_unless_a_card_was_gained_in_the_buy_phase` |
| River Shrine | Every River Shrine gains | RS `every_play_of_river_shrine_gains_in_cleanup` |
| River Shrine | Trashing optional | RS `river_shrine_trashes_up_to_two_...` (a gain without trashing) |
| River Shrine | Multiple Buy phases: Continue's gain counts | RS `river_shrine_does_not_gain_in_cleanup_when_continue_gained_a_card_in_the_buy_phase` |
| River Shrine | Rush; played on another player's turn | not tested: Rush playing a River Shrine gained during Clean-up needs a card played inside Clean-up (exotic); a River Shrine played on another player's turn needs a Prince-like effect on it (none of the implemented ones can) |
| Rustic Village | Sun first, +1 Card, discard 2 for +1 Card incl. the drawn card | RS `rustic_village_draws_gives_two_actions_and_discards_two_for_a_card` |
| Gold Mine | Gold costs 4 Debt, even with Debt; both or neither | RS `gold_mine_may_gain_a_gold_for_four_debt` |
| Kitsune | Sun first; two different options in order | RS `kitsune_does_two_different_options_in_the_order_listed`, `kitsune_curse_is_stopped_by_a_moat` |
| Rice Broker | Treasure +2, Action +5, both = both, neither none | RS `rice_broker_draws_two_for_a_treasure_and_five_for_an_action`, `enlightenment_treasures_trashed_to_rice_broker_count_as_both` |
| Ronin | Draw one at a time to 7; none at 7+ | RS `ronin_draws_until_seven_cards_in_hand` |
| Tanuki | Remodel by $2 | RS `tanuki_trashes_and_gains_up_to_two_more` |
| Tea House | Sun, +1 Card +1 Action +$2 | RS `tea_house_is_a_cantrip_with_two_coins` |
| Samurai | Attack once; $1 every turn; stays in play | RS `samurai_attacks_once_and_gives_a_coin_at_the_start_of_every_turn`, `a_second_samurai_play_by_throne_room_does_not_make_the_others_discard_twice` |
| Rice | Example count of types | RS `rice_counts_the_different_types_among_the_cards_in_play` |
| Change | Debt -> +$3; else remodel for more $ with Debt = difference; costs re-checked; no Debt if no gain | `empires_cards.rs` Change tests (Wayfarer / Fisherman re-check: not tested, needs Menagerie cost tracking in the Change test; Possession not implemented) |
| Shadow rules | Not sunk when gained; sunk on every shuffle; play from the deck anywhere; Throne Room / Practice can pick them; not in hand for discards | RS `a_gained_shadow_is_not_put_on_the_bottom_until_the_next_shuffle`, `shadow_cards_go_to_the_bottom_of_the_deck_when_the_discard_pile_is_shuffled`, `a_shadow_card_in_the_deck_can_be_played_as_if_in_the_hand`, `a_shadow_play_uses_an_action_and_prefers_the_copy_in_the_deck`, `throne_room_can_play_a_shadow_card_from_the_deck_twice`, `practice_can_play_a_shadow_card_from_the_deck_twice`, Annex / Inn tests (#4) |
| Omen / Prophecy | Tokens by player count; last token activates once; Sun does nothing afterwards; one Prophecy per game with an Omen | RS `sun_tokens_depend_on_the_number_of_players`, `an_omen_removes_a_sun_token_and_the_last_one_makes_the_prophecy_happen_once`, `throne_room_on_an_omen_removes_two_tokens_and_a_game_without_a_prophecy_ignores_the_sun`, `a_randomized_kingdom_gets_a_prophecy_exactly_when_it_has_an_omen` |

### Rising Sun Events

| Event | Ruling | Test |
|---|---|---|
| Amass | Only with no Action in play; earlier-turn Durations stop it; Horse (no longer in play) does not | RS `amass_gains_an_action_up_to_five_only_with_no_action_in_play`, `amass_is_stopped_by_a_duration_played_on_an_earlier_turn`, `amass_works_after_a_horse_has_returned_to_its_pile`; buying with an Action in play: open (ignored) |
| Asceticism | Pay any amount to trash that many | RS `asceticism_pays_any_amount_to_trash_that_many_cards` |
| Continue | Once per turn; non-Attack Action up to $4; returns to the Action phase, +1 Action +1 Buy, no Action play used | RS `continue_returns_to_the_action_phase_and_plays_the_card_gained` |
| Continue | Gained card moved (Rapid Expansion): still returns | RS `continue_still_returns_to_the_action_phase_when_the_gained_card_was_set_aside_by_rapid_expansion` |
| Continue | Gain is in the Buy phase (River Shrine) | RS `river_shrine_does_not_gain_in_cleanup_when_continue_gained_a_card_in_the_buy_phase` |
| Continue | Buyable with nothing to gain | RS `continue_can_be_bought_when_no_non_attack_action_up_to_four_is_left` |
| Continue | Crown's phase; Debt 8 | not testable: Crown costs $5, Continue gains up to $4 (no interaction); Debt: RS `continue_returns_...` |
| Credit | Not Debt / Potion cards; Debt = cost | RS `credit_gains_an_action_or_treasure_up_to_eight_and_takes_debt_equal_to_its_cost`, `credit_cannot_gain_a_card_with_debt_or_potion_in_its_cost` |
| Credit | Cost changed at gain time (Destrier) / Possession | not testable here: Possession is not implemented; Destrier: not tested (the Debt reads `cost()` after the gain, `rs_gained`) |
| Foresight | Set aside the first Action, discard the rest, into the hand after the next draw | RS `foresight_sets_the_first_action_aside_and_puts_it_into_the_next_hand`, `foresight_can_be_bought_with_no_action_in_the_deck_and_reveals_the_whole_deck` |
| Gather | In order, none optional, still gains the others | RS `gather_gains_cards_costing_exactly_three_four_and_five`, `gather_still_gains_the_other_cards_when_nothing_costs_exactly_four` |
| Kintsugi | Gold ever gained; gain even without the Gold; trash-caused Gold counts | RS `kintsugi_trashes_and_remodels_once_a_gold_has_been_gained` |
| Practice | Play an Action from the hand twice | RS `practice_plays_an_action_from_the_hand_twice` |
| Practice | Treasures cannot be played after a buy | not tested: the Buy-phase rule (`treasures_done`) belongs to the engine core and is covered by the core tests |
| Receive Tribute | 3 gains; up to 3 differently named Actions not in play; any order; Debt / Potion cards without paying | RS `receive_tribute_needs_three_gains_and_gains_up_to_three_different_actions_not_in_play`, `receive_tribute_gains_actions_with_potion_or_debt_in_the_cost_without_paying` |
| Sea Trade | Draw first, then trash up to that many; nothing without Actions | RS `sea_trade_draws_a_card_per_action_in_play_and_trashes_up_to_that_many` |

### Rising Sun Prophecies

| Prophecy | Ruling | Test |
|---|---|---|
| Approaching Army | Extra Attack pile; Duration Attack only the turn played; last effect; with a Way | RS `approaching_army_adds_an_attack_pile_to_a_kingdom_with_an_omen`, `approaching_army_gives_a_coin_after_each_attack`, `approaching_army_gives_the_coin_for_a_duration_attack_only_on_the_turn_it_is_played`, `approaching_army_gives_its_coin_even_when_the_attack_was_played_as_a_way`; split-pile randomizers: not tested (the pool filter uses the pile's top card, which is the randomizer) |
| Biding Time | Hand set aside face down at the start of Clean-up, back at the start of the next turn | RS `biding_time_sets_the_hand_aside_at_cleanup_and_returns_it_at_the_start_of_the_next_turn` |
| Bureaucracy | Copper for a non-$0 card; Loot; Debt costs | RS `bureaucracy_gains_a_copper_with_every_card_that_does_not_cost_zero`, `bureaucracy_gains_a_copper_for_a_loot_but_not_for_a_copper`, `bureaucracy_gains_a_copper_for_a_card_with_debt_in_its_cost` |
| Divine Wind | all rulings | not testable: the Prophecy is disabled (`divine_wind_is_disabled_for_now`); the dealing code has `divine_wind_replaces_the_kingdom_piles_with_ten_new_ones` and `divine_wind_never_deals_a_liaison_it_could_not_set_up` |
| Enlightenment | Treasures are Actions; +1 Card +1 Action in the Action phase, normal in the Buy phase; Crown; Rice Broker | RS `enlightenment_makes_treasures_actions_that_draw_and_give_an_action_in_the_action_phase`, `enlightenment_makes_a_crown_played_in_the_action_phase_a_cantrip`, `enlightenment_treasures_trashed_to_rice_broker_count_as_both` |
| Enlightenment | Highwayman cannot stop it | RS `highwayman_cannot_stop_the_first_treasure_played_in_the_action_phase_under_enlightenment` (ignored, open) |
| Enlightenment | Ways, Black Market / Storyteller / Herb Gatherer plays, pile is not an Action pile | not tested: Treasures use the same play path as other plays (`resolve_effects_inner`); no pile Action test exists for Training / Populate (Event rules, other agents) |
| Flourishing Trade | Cost $1 less everywhere, cumulative with Bridge; Action plays as Buys | RS `flourishing_trade_lowers_costs_and_lets_actions_be_used_as_buys` |
| Flourishing Trade | Plateau Shepherds scoring | not tested here: `allies.rs::al_score` reads it (tested in the Allies file by its owner) |
| Good Harvest | First play of each named Treasure; not retroactive | RS `good_harvest_gives_a_buy_and_a_coin_for_the_first_play_of_each_treasure_each_turn`, `good_harvest_is_not_retroactive` |
| Great Leader | +1 Action after each Action, the Omen that activated it too | RS `great_leader_gives_an_action_after_each_action_including_the_omen_that_made_it_happen`, `great_leader_gives_its_action_after_a_card_played_as_a_way` |
| Growth | Mandatory cheaper card, chains | RS `growth_gains_a_cheaper_card_when_a_treasure_is_gained_and_chains` |
| Harsh Winter | Debt on / from the pile; own turn only; Possession | RS `harsh_winter_puts_debt_on_a_pile_and_the_next_gain_from_it_takes_it`, `harsh_winter_ignores_gains_on_other_players_turns`; Possession not implemented; Reward / Loot piles: open (see above) |
| Kind Emperor | Gains an Action to the hand at once (the remover) and at every turn start | RS `kind_emperor_gains_an_action_to_the_hand_when_it_happens_and_at_the_start_of_every_turn`; two Actions with a start-of-turn Poet (Delay): not tested, needs Delay to play an Omen at the start of the turn |
| Panic | +2 Buys per Treasure; Treasures return to piles; no pile = discard; Loot; activated mid-play | RS `panic_gives_buys_for_playing_treasures_and_returns_them_to_their_piles`, `panic_returns_a_loot_treasure_to_the_loot_pile`, `panic_activated_while_a_treasure_is_in_play_returns_it_but_gave_no_buys`; Heirloom stays: not tested (same `has_pile_of` test); Jewels never discarded: not tested (stays in play by Plunder rules) |
| Progress | Every gained card on the deck, also other turns; Ghost Town vs Villa; Sleigh | RS `progress_puts_every_gained_card_onto_the_deck`, `progress_overrules_a_gain_to_the_hand_but_not_a_card_that_moves_itself_after_being_gained`; Sleigh: not tested (same stop-moving path as Villa) |
| Rapid Expansion | Mandatory set-aside of Actions and Treasures, played at the start of the next turn in the owner's order; also cards gained to the hand | RS `rapid_expansion_sets_aside_gained_actions_and_treasures_to_be_played_next_turn`, `rapid_expansion_sets_aside_a_treasure_gained_to_the_hand`; Villa vs Rapid Expansion: not tested (same stop-moving path as Progress) |
| Sickness | Choose Curse onto the deck or discard 3; both always offered | RS `sickness_asks_at_the_start_of_every_turn_for_a_curse_on_the_deck_or_three_discards`, `sickness_offers_both_options_with_an_empty_curse_pile_and_a_small_hand` |

### Renaissance cards

| Card | Ruling | Test |
|---|---|---|
| Border Guard | Reveal 2 (3 with the Lantern), one to hand, rest discarded; all Actions -> Lantern or Horn (Horn with the Lantern); fewer cards -> no Artifact; both work at once | REN `border_guard_keeps_one_of_two_revealed_cards_and_discards_the_other`, `border_guard_with_two_actions_takes_the_lantern_or_the_horn`, `the_lantern_makes_border_guard_reveal_three_and_all_three_must_be_actions_for_the_horn`, `border_guard_reveals_fewer_cards_and_takes_no_artifact_when_the_deck_is_short` |
| Border Guard | Lantern with Band of Misfits, Way of the Chameleon, Reckless; Horn with Reckless copies | REN `the_lantern_affects_a_border_guard_played_by_band_of_misfits_and_as_way_of_the_chameleon`, `a_reckless_border_guard_reveals_twice_with_the_lantern_and_the_horn_saves_only_one`, `with_reckless_border_guards_the_horn_saves_one_and_the_others_return_to_the_supply` |
| Ducat | +1 Coffers +1 Buy, no $; optional Copper trash on gain | REN `ducat_gives_a_coffers_and_a_buy_and_may_trash_a_copper_when_gained` |
| Lackeys | +2 Cards; +2 Villagers on gain | REN `lackeys_draws_two_and_gives_two_villagers_when_gained` |
| Acting Troupe | +4 Villagers even if not trashed | REN `acting_troupe_gives_four_villagers_and_trashes_itself_even_under_a_throne_room`, `trashing_an_acting_troupe_with_priest_in_effect_gives_two_dollars` |
| Cargo Ship | Any later gain; one card per Ship; discarded if unused; stranded after Improve | REN `cargo_ship_sets_aside_a_card_gained_this_turn_for_the_next_hand`, `a_cargo_ship_that_set_nothing_aside_is_discarded_and_each_one_serves_one_card_once`, `a_cargo_ship_trashed_by_improve_leaves_the_card_it_sets_aside_stranded` |
| Experiment | Return to the pile; gain another (no chain); second goes to the discard pile; not in play -> stays | REN `experiment_returns_to_its_pile_and_comes_with_another_one`, `an_experiment_gained_to_the_hand_brings_the_second_one_to_the_discard_pile`, `an_experiment_played_without_being_in_play_stays_where_it_is`; gained from the trash: not tested (the second copy always comes from the Supply) |
| Improve | Action to be discarded only; itself; optional but gain mandatory; Duration that stays; Fortress; each play | REN `improve_trashes_an_action_at_cleanup_for_a_card_costing_exactly_one_more`, `improve_may_trash_itself_or_nothing_and_each_play_gives_its_own_offer`, `improve_cannot_trash_a_duration_that_stays_in_play_but_can_trash_one_that_is_discarded`, `improve_trashing_a_fortress_sends_it_to_the_hand_and_it_is_discarded_with_the_rest_of_cleanup`; Trail / Taskmaster / Garrison / Den of Sin: not tested (cards that react in Clean-up; same gain / trash paths) |
| Flag Bearer | Flag on gain or trash, by whoever trashes it; Flag draws +1 whatever the hand size | REN `flag_bearer_takes_the_flag_when_gained_or_trashed_from_whoever_has_it`, `the_flag_adds_a_card_to_the_cleanup_draw_whatever_the_size_of_the_hand` |
| Hideout | Trash mandatory; Victory -> Curse | REN `hideout_trashes_a_card_and_a_victory_card_gives_a_curse` |
| Inventor | Gain, then reduction; cumulative; with Innovation the reduction is not yet in effect | REN `inventor_gains_up_to_four_and_then_cards_cost_one_less_cumulatively`, `an_inventor_gained_by_inventor_and_played_by_innovation_gains_up_to_four_only`; Throne Room'd / trashed Inventor: covered by the cumulative test (reduction is turn state) |
| Mountain Village | Must take a card if the pile has any | REN `mountain_village_takes_a_card_from_the_discard_pile_or_draws_when_it_is_empty` |
| Patron | +1 Villager +$2; Coffers on a reveal in an Action phase only; a discard is not a reveal | REN `patron_gives_a_villager_and_two_dollars_and_a_coffers_when_revealed_in_an_action_phase`, `border_guard_revealing_two_patrons_gives_two_coffers_and_a_discarded_patron_gives_none`; Hostelry / Black Market: not testable (Black Market not implemented) |
| Priest | +$2 per later trash, cumulative, Sewers' trash does not count | REN `priest_trashes_a_card_and_every_later_trash_this_turn_is_two_dollars`, `sewers_trashes_before_priests_ability_is_set_up`, `trashing_an_acting_troupe_with_priest_in_effect_gives_two_dollars`; Way of the Chameleon: not tested (+Cards replace the $ in `resolve_effects_inner<SWAP>`; Priest's bonus is not routed through it) |
| Research | $ cost = cards set aside, face down, fewer if the deck is short | REN `research_trashes_a_card_and_sets_aside_a_card_per_dollar_for_next_turn`, `research_sets_aside_what_there_is_and_is_discarded_when_it_sets_nothing_aside`, `the_cards_set_aside_under_research_are_hidden_from_other_players_but_conserved` |
| Silk Merchant | Coffers + Villager on gain or trash, trasher takes it | REN `silk_merchant_gives_coffers_and_a_villager_when_gained_or_trashed` |
| Old Witch | Curse then optional trash; with an empty pile; Moat stops both; Sheepdog | REN `old_witch_draws_three_and_gives_a_curse_that_may_be_trashed_with_a_curse_from_the_hand`, `old_witch_trashing_works_with_the_curse_pile_empty_and_a_moat_stops_both_halves`, `old_witch_lets_a_sheepdog_draw_the_curse_that_is_then_trashed` |
| Recruiter | Draw first; mandatory trash; $ part only | REN `recruiter_draws_two_trashes_a_card_and_gives_a_villager_per_dollar_it_cost`, `recruiter_counts_only_the_dollar_part_of_the_cost` |
| Scepter | +$2 or replay; no previous-turn Duration; Duration stays; Priest cumulative | REN `scepter_is_two_dollars_or_replays_an_action_played_this_turn`, `scepter_cannot_replay_a_card_that_is_not_in_play_or_a_duration_of_an_earlier_turn`, `scepter_replaying_a_duration_played_this_turn_stays_in_play_until_the_duration_leaves`, `scepters_replay_of_mining_village_can_trash_it_when_it_is_still_in_play`; Storyteller / Duplicate / Barge: not tested (rare combos of unfinished plays; stop-moving tests cover the shared path) |
| Scholar | Discard all at once, then draw 7 (reshuffles include them) | REN `scholar_discards_the_hand_and_draws_seven`; Trail: not tested (needs Trail's reaction ordering) |
| Sculptor | Gain to the hand; Villager for a Treasure | REN `sculptor_gains_to_the_hand_and_a_treasure_gives_a_villager`; Nomad Camp: not testable, not implemented |
| Seer | +1 Card +1 Action; $2-$4 to hand; P / Debt excluded; rest back in any order | REN `seer_puts_the_cards_costing_two_to_four_into_the_hand_and_the_rest_back_in_any_order`, `seer_does_not_take_cards_with_potion_or_debt_in_the_cost` |
| Spices | $2 +1 Buy; +2 Coffers on gain | REN `spices_is_two_dollars_and_a_buy_and_gives_two_coffers_when_gained` |
| Swashbuckler | Check after the draw; Chest at 4 Coffers | REN `swashbuckler_gives_a_coffers_if_the_discard_pile_is_not_empty_after_the_draw`, `four_coffers_make_swashbuckler_take_the_treasure_chest_which_gains_a_gold_every_buy_phase`; Order of Masons / Market Towns: not testable by me (Allies cards of another set; the Ally dealing is the owner's) |
| Treasurer | Three options; Key even when held; Treasure regained triggers Ducat; start-of-turn Key at once; Elder | REN `treasurer_is_three_dollars_and_trashes_a_treasure_or_regains_one_or_takes_the_key`, `a_treasurer_played_at_the_start_of_the_turn_gives_the_key_at_once`, `capitalism_lets_a_treasurer_regain_a_card_that_is_a_treasure_on_the_owners_turn`; Elder: not tested (Allies, replay of the mode) |
| Villain | +2 Coffers; 5+ cards discard a $2+ card or reveal; Estate under Canal | REN `villain_gives_two_coffers_and_makes_players_with_five_cards_discard_a_card_costing_two_or_more`, `canal_makes_an_estate_too_cheap_for_villain` |
| Villagers | +1 Action each, any number, only in the Action phase | REN `a_villager_is_spent_for_an_action_only_when_no_action_is_left`, `villagers_are_not_asked_about_without_an_action_card_in_hand_and_may_be_kept` |
| Coffers | spent any time | `sim` / core tests (Guilds file); `renaissance_cards.rs` Pageant and Swashbuckler tests use them |

### Renaissance Artifacts and Projects

| Item | Ruling | Test |
|---|---|---|
| Artifacts | One copy, taken from whoever has it | REN `an_artifact_changes_hands_and_the_event_says_from_whom`, `the_view_reports_villagers_projects_and_artifacts_to_everyone` |
| Key | +$1 at the start of the turn | REN `treasurer_is_three_dollars_...` |
| Treasure Chest | Gold at every Buy phase, also the turn taken | REN `four_coffers_make_swashbuckler_take_the_treasure_chest_which_gains_a_gold_every_buy_phase` |
| Horn / Lantern / Flag | see Border Guard / Flag Bearer | as above |
| Projects (general) | Cost not reduced, uses a Buy, two cubes, not twice, not in Debt, not "buying a card" (Exploration) | REN `a_project_is_bought_for_its_printed_cost_and_a_buy_and_belongs_to_the_buyer`, `a_player_has_two_cubes_and_cannot_buy_a_project_with_debt_or_without_a_buy`, `a_projects_price_is_not_reduced_by_cards_that_make_cards_cheaper` |
| Cathedral | Mandatory trash at the start of each turn | REN `cathedral_trashes_a_card_at_the_start_of_every_turn` |
| City Gate | +1 Card then topdeck | REN `city_gate_draws_a_card_and_puts_a_card_from_the_hand_onto_the_deck` |
| Pageant | Pay $1 once for +1 Coffers | REN `pageant_turns_a_leftover_dollar_into_a_coffers_at_the_end_of_the_buy_phase`; Villa / Cavalry repeated phases and Wine Merchant: not tested (the plan lists multi-Buy-phase turns as a gap; Wine Merchant is a Tavern call from a card of the Adventures owner) |
| Sewers | Not on its own trash; any card of the hand; one per card; Priest, Old Witch, Acting Troupe | REN `sewers_lets_a_player_trash_a_card_from_the_hand_for_every_card_they_trash_otherwise`, `sewers_trashes_before_priests_ability_is_set_up`, `trashing_an_acting_troupe_with_priest_in_effect_gives_two_dollars` |
| Star Chart | Pick a card to go on top of a shuffle | REN `star_chart_lets_the_owner_put_a_card_of_the_shuffle_on_top_of_the_new_deck`; other shuffles: `star_chart_is_offered_for_the_shuffle_of_annex` (ignored, open) |
| Exploration | No gain in the Buy phase; Action-phase gains do not count | REN `exploration_gives_a_coffers_and_a_villager_when_no_card_was_gained_in_the_buy_phase`, `exploration_does_not_count_cards_gained_in_the_action_phase`; repeated Buy phases: not tested (plan gap) |
| Fair, Barracks | +1 Buy / +1 Action every turn | REN `fair_and_barracks_give_a_buy_and_an_action_at_the_start_of_every_turn` |
| Silos | Discard Coppers revealed, then draw (shuffle includes them) | REN `silos_discards_any_number_of_coppers_at_the_start_of_the_turn_to_draw_that_many` |
| Sinister Plot | Add a token or remove all | REN `sinister_plot_adds_a_token_or_removes_them_all_for_a_card_each` |
| Academy, Guildhall | Villager per Action / Coffers per Treasure gained | REN `academy_and_guildhall_pay_for_every_action_or_treasure_gained_by_their_owner` |
| Capitalism | +$ Actions are Treasures on the owner's turn, played in the Buy phase with all effects, no Action used; full list; Treasurer regains an Improve | REN `capitalism_makes_actions_with_a_dollar_amount_treasures_playable_in_the_buy_phase`, `capitalism_affects_every_action_the_wiki_lists`, `capitalism_lets_a_treasurer_regain_a_card_that_is_a_treasure_on_the_owners_turn`; Aristocrat / Steward / Caravan Guard examples: covered by the list test (type is by name) |
| Fleet | Extra round for players with it, in order after the last player; blocks later extra turns; limit | REN `fleet_gives_an_extra_turn_after_the_game_would_end_to_each_player_who_has_it`, `a_fleet_bought_in_the_fleet_round_gets_no_turn_and_fleet_does_not_extend_the_turn_limit`; Possession: not testable (not implemented) |
| Piazza | Reveal; play if Action, else back | REN `piazza_plays_the_top_card_of_the_deck_at_the_start_of_the_turn_if_it_is_an_action` |
| Road Network | Another player's Victory gain, also on your turn | REN `road_network_draws_a_card_whenever_another_player_gains_a_victory_card` |
| Crop Rotation | One Victory card for +2 Cards | REN `crop_rotation_discards_a_victory_card_for_two_cards`, `crop_rotation_discards_only_one_victory_card` |
| Innovation | Optional; once per turn, declining keeps it; Buy-phase gain playable; only own gains | REN `innovation_plays_the_first_action_gained_each_turn_if_the_player_likes`; Duplicate / Sleigh stop-moving: not tested (shared stop-moving machinery has `cross_set.rs` tests) |
| Canal | $1 less on the owner's turns only | REN `canal_makes_every_card_cost_one_less_on_its_owners_turns_only`, `canal_makes_an_estate_too_cheap_for_villain`; Plateau Shepherds: Allies' owner |
| Citadel | First Action each turn twice, outside the Action phase, Durations, Enchantress | REN `citadel_plays_the_first_action_each_turn_twice`, `citadel_plays_a_duration_twice_and_keeps_both_next_turn_effects`, `citadel_with_enchantress_gives_the_cantrip_and_then_replays_the_card_normally`, `citadels_replay_of_mining_village_cannot_trash_another_mining_village`; Citadel + Capitalism in the Buy phase, Werewolf: not tested |

### Counts

Rulings enumerated: about 190 bullets across the two sets. With a named test: about 150. Not tested with a stated reason: about 40 (Possession, Black Market, Nomad Camp, Sheepdog
not implemented; rare third-party combos such as Trail, Taskmaster, Garrison, Storyteller, Duplicate, Werewolf and repeated Buy phases; the disabled Divine Wind). Open issues with ignored tests: 5.
