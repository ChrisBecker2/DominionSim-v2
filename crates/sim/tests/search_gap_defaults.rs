//! Fast-bot defaults taken from what the turn search does (`docs/search-in-league.md`, section 7): a card that must trash something is passed
//! over when it would only lose, even when a `[[play]]` rule names it; the trash is the card that costs least to lose, on the gain list's scale.

use dominion_engine::cards::{self, id};
use dominion_engine::{CardId, Choice, ChoiceBuf, GameConfig, GameState, NoEvents, PlayerView, Step};
use dominion_sim::Strategy;

/// Big Money with Gold/Silver and the Province, plus a play rule for `card` (fast: no search).
fn bm_with(card: &str) -> Strategy {
    let mut s = Strategy::parse(&format!(
        "name = \"t\"\n[[gain]]\ncard = \"Province\"\n[[gain]]\ncard = \"Gold\"\n[[gain]]\ncard = \"Silver\"\n[[play]]\ncard = \"{card}\"\n"
    ))
    .unwrap();
    s.search_play = false;
    s.win_this_turn = false;
    s
}

/// The choice made at the first decision of a turn that starts with `hand`.
fn first_choice(s: &Strategy, kingdom: &[CardId], hand: &[(CardId, u8)]) -> Choice {
    let mut g = GameState::new(&GameConfig { kingdom: kingdom.to_vec(), seed: 3, ..Default::default() });
    let p = g.turn.player as usize;
    // The hand, and nothing left to draw at the start of the turn.
    g.players[p].hand = Default::default();
    for &(c, n) in hand {
        g.players[p].hand.add(c, n);
    }
    g.players[p].deck_known.clear();
    g.players[p].deck_unknown = Default::default();
    g.players[p].discard = Default::default();
    let Step::Decision(d) = g.advance(&mut NoEvents) else { panic!("no decision") };
    let mut buf = ChoiceBuf::default();
    g.legal_choices(&mut buf);
    s.decide_by_rules(&PlayerView::new(&g, d.player), &d, buf.as_slice())
}

fn plays(c: CardId) -> Choice {
    Choice::Card(c)
}

#[test]
fn remodel_is_played_for_junk_and_passed_over_for_a_silver_swap() {
    let s = bm_with("Remodel");
    let k = [id::REMODEL, id::CELLAR, id::MARKET];
    // Estate -> Silver is a gain; Silver -> Silver costs this turn's $2 for nothing.
    assert_eq!(first_choice(&s, &k, &[(id::COPPER, 2), (id::SILVER, 1), (id::ESTATE, 1), (id::REMODEL, 1)]), plays(id::REMODEL));
    assert_eq!(first_choice(&s, &k, &[(id::COPPER, 2), (id::SILVER, 2), (id::REMODEL, 1)]), Choice::Pass);
    // Gold -> Province is the best remodel there is.
    assert_eq!(first_choice(&s, &k, &[(id::COPPER, 1), (id::SILVER, 1), (id::GOLD, 1), (id::REMODEL, 1)]), plays(id::REMODEL));
}

#[test]
fn remodel_is_not_played_for_a_copper_for_a_copper() {
    // Nothing of the gain list costs $2 or less: trashing a Copper gains a Copper back (Remodel must gain), the deck unchanged.
    let s = bm_with("Remodel");
    let k = [id::REMODEL, id::CELLAR, id::MARKET];
    assert_eq!(first_choice(&s, &k, &[(id::COPPER, 4), (id::REMODEL, 1)]), Choice::Pass);
    // With an Estate in the hand, Estate -> Silver is still worth it.
    assert_eq!(first_choice(&s, &k, &[(id::COPPER, 3), (id::ESTATE, 1), (id::REMODEL, 1)]), plays(id::REMODEL));
}

#[test]
fn forced_trashers_with_nothing_to_lose_are_passed_over() {
    // Rats and Forager trash a card for nothing: with only Silver and Gold in the hand they lose.
    for (name, card, kingdom) in [("Forager", id::FORAGER, [id::FORAGER, id::CELLAR]), ("Rats", id::RATS, [id::RATS, id::CELLAR])] {
        let s = bm_with(name);
        assert_eq!(first_choice(&s, &kingdom, &[(id::SILVER, 2), (id::GOLD, 2), (card, 1)]), Choice::Pass, "{name}");
    }
    // ... and with a Copper to lose, Rats is played (Forager's $ for the Copper in the trash pay for it too).
    let s = bm_with("Rats");
    assert_eq!(first_choice(&s, &[id::RATS, id::CELLAR], &[(id::COPPER, 3), (id::SILVER, 1), (id::RATS, 1)]), plays(id::RATS));
}

#[test]
fn trading_post_needs_two_cards_worth_less_than_the_silver() {
    let s = bm_with("Trading Post");
    let k = [id::TRADING_POST, id::CELLAR];
    // Two Estates for a Silver: yes. Two Silvers (or a Silver and a Gold) for a Silver: no. Gold -> Province needs less than $8 in hand.
    assert_eq!(first_choice(&s, &k, &[(id::ESTATE, 2), (id::COPPER, 2), (id::TRADING_POST, 1)]), plays(id::TRADING_POST));
    assert_eq!(first_choice(&s, &k, &[(id::SILVER, 3), (id::GOLD, 1), (id::TRADING_POST, 1)]), Choice::Pass);
}

#[test]
fn crown_and_a_lone_treasure_map_are_not_played_without_a_target() {
    let s = bm_with("Crown");
    assert_eq!(first_choice(&s, &[id::CROWN, id::CELLAR], &[(id::COPPER, 3), (id::SILVER, 1), (id::CROWN, 1)]), Choice::Pass);
    let s = bm_with("Treasure Map");
    assert_eq!(first_choice(&s, &[id::TREASURE_MAP, id::CELLAR], &[(id::COPPER, 3), (id::SILVER, 1), (id::TREASURE_MAP, 1)]), Choice::Pass);
}

#[test]
fn legacy_defaults_still_play_the_named_card() {
    // The measurement switch: the old bot plays Remodel for the Silver swap.
    let mut s = bm_with("Remodel");
    s.legacy_defaults = true;
    let k = [id::REMODEL, id::CELLAR, id::MARKET];
    assert_eq!(first_choice(&s, &k, &[(id::COPPER, 2), (id::SILVER, 2), (id::REMODEL, 1)]), plays(id::REMODEL));
    let _ = cards::name(id::REMODEL);
}
