//! The order of start-of-turn effects: each pending Duration effect (and Clerk's "play this from
//! your hand", and Prince's set-aside card) is resolved in an order the turn's player chooses,
//! one at a time, but only when the order can matter (`cards::start_order_sensitive`).
//!
//! As in `seaside_cards.rs`, the effects are injected into `pending_durations` of a state that
//! sits at the very start of player 0's turn; the first `advance` collects them
//! (`push_turn_start_frames`) exactly as a real second turn would.

mod common;
use common::*;
use dominion_engine::cards::{self, id};
use dominion_engine::state::PendingDuration;
use dominion_engine::text::{format_state, parse_state};
use dominion_engine::*;

fn inject(g: &mut GameState, card: CardId, times: u8, arg: CardId) {
    let ps = &mut g.players[0];
    let i = ps.pending_durations_len as usize;
    ps.pending_durations[i] = PendingDuration { card, times, arg, used: false };
    ps.pending_durations_len += 1;
}

/// The cards a `StartOrder` decision currently offers.
fn offered(g: &GameState) -> Vec<CardId> {
    choices(g)
        .into_iter()
        .map(|c| match c {
            Choice::Card(x) => x,
            other => panic!("unexpected choice {other:?}"),
        })
        .collect()
}

/// Hand and in-play cards together: reaching the Buy phase plays the Treasures from the hand.
fn hand_and_play(g: &GameState) -> Counts {
    let mut c = g.players[0].hand;
    c.add_all(&g.players[0].in_play);
    c
}

fn is_discard_select(d: &Decision) -> bool {
    matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. })
}

fn is_turn_decision(d: &Decision) -> bool {
    matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy)
}

// ---------------------------------------------------------------------------------------
// When the question appears
// ---------------------------------------------------------------------------------------

#[test]
fn a_single_effect_is_never_asked() {
    let mut g = new_state(&[id::SEA_WITCH], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    let d = expect_decision(&mut g);
    // Straight to Sea Witch's own discard (after its +2 Cards), no ordering question.
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 2, max: 2, ordered: false });
    assert_eq!(g.players[0].hand.total(), 5);
}

#[test]
fn plain_bonuses_are_never_asked() {
    let mut g = new_state(&[id::CARAVAN, id::WHARF, id::MONKEY, id::FISHING_VILLAGE, id::LIGHTHOUSE, id::MERCHANT_SHIP], 2);
    set_deck_known(&mut g, 0, &[id::COPPER; 8]);
    for c in [id::CARAVAN, id::WHARF, id::MONKEY, id::FISHING_VILLAGE, id::LIGHTHOUSE, id::MERCHANT_SHIP] {
        inject(&mut g, c, 1, 0);
    }
    let d = expect_decision(&mut g);
    assert_ne!(d.kind, DecisionKind::StartOrder);
    assert!(is_turn_decision(&d));
    // All six resolved: +1 +2 +1 Cards, +1 Action, +$1 +$1 +$2, +1 Buy.
    // +1 +2 +1 Cards (the 4 Coppers are then played), +1 Action, +1 Buy, +$1 +$1 +$2.
    assert_eq!(g.players[0].in_play.get(id::COPPER), 4);
    assert_eq!((g.turn.actions, g.turn.buys, g.turn.coins), (2, 2, 8));
    assert_eq!(g.players[0].pending_durations_len, 0);
}

#[test]
fn an_order_sensitive_effect_with_a_draw_is_asked() {
    let mut g = new_state(&[id::SEA_WITCH, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD, id::GOLD, id::GOLD]);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    inject(&mut g, id::CARAVAN, 1, 0);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(d.source, None);
    assert_eq!(offered(&g), vec![id::SEA_WITCH, id::CARAVAN]);
    // Nothing has resolved yet.
    assert_eq!(g.players[0].hand.total(), 2);
}

#[test]
fn effects_that_cannot_matter_are_resolved_without_a_question() {
    // Lighthouse (+$1) and Fishing Village (+1 Action +$1) draw nothing and touch no hand, so
    // they never make the order of Sea Witch's discard a question; Sea Witch is then alone.
    let mut g = new_state(&[id::SEA_WITCH, id::LIGHTHOUSE, id::FISHING_VILLAGE, id::OUTPOST], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::GOLD, id::GOLD]);
    inject(&mut g, id::LIGHTHOUSE, 1, 0);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    inject(&mut g, id::FISHING_VILLAGE, 1, 0);
    inject(&mut g, id::OUTPOST, 0, 0);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 2, max: 2, ordered: false });
    assert_eq!((g.turn.actions, g.turn.coins), (2, 2), "the plain bonuses already resolved");
}

#[test]
fn identical_effects_are_one_choice_and_the_same_card_twice_is_not_a_question() {
    // Two Sea Witch entries (separate plays, e.g. a Throne Room and a later turn's merge is by
    // `times`, but a state may hold two entries): the same effect, so nothing to choose.
    let mut g = new_state(&[id::SEA_WITCH], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::COPPER]);
    set_deck_known(&mut g, 0, &[id::GOLD; 4]);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }), "no StartOrder: {:?}", d.kind);

    // A Sea Witch with a Caravan: two different effects, one of them order-sensitive.
    let mut g = new_state(&[id::SEA_WITCH, id::CARAVAN], 2);
    set_deck_known(&mut g, 0, &[id::GOLD; 4]);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    inject(&mut g, id::CARAVAN, 1, 0);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(offered(&g), vec![id::SEA_WITCH, id::CARAVAN], "one choice per distinct effect");
}

// ---------------------------------------------------------------------------------------
// Resolving in the chosen order
// ---------------------------------------------------------------------------------------

#[test]
fn returning_a_haven_card_first_lets_the_discard_choose_among_it() {
    // Hand {Gold, Silver}; Haven holds an Estate; Sea Witch must discard 2.
    // Haven first: the discard picks from {Gold, Silver, Estate} and keeps the Gold.
    // Sea Witch first: it must discard Gold and Silver, and only then does the Estate return.
    let build = || {
        let mut g = new_state(&[id::SEA_WITCH, id::HAVEN], 2);
        set_hand(&mut g, 0, &[id::GOLD, id::SILVER]);
        set_set_aside(&mut g, 0, &[id::ESTATE]);
        inject(&mut g, id::SEA_WITCH, 1, 0);
        inject(&mut g, id::HAVEN, 1, id::ESTATE);
        g
    };

    let mut g = build();
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(offered(&g), vec![id::SEA_WITCH, id::HAVEN]);
    choose(&mut g, Choice::Card(id::HAVEN));
    // Haven resolved: the Estate is back in hand and Sea Witch's discard (no deck: no draw) is next.
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::Select { from: Zone::Hand, act: Act::Discard, filter: Filter::Any, min: 2, max: 2, ordered: false });
    assert_eq!(g.players[0].hand, counts_of(&[id::GOLD, id::SILVER, id::ESTATE]));
    assert!(g.players[0].set_aside.is_empty());
    choose(&mut g, Choice::Card(id::SILVER));
    choose(&mut g, Choice::Card(id::ESTATE));
    let d = expect_decision(&mut g);
    assert!(is_turn_decision(&d));
    assert_eq!(hand_and_play(&g), counts_of(&[id::GOLD]), "the Gold is kept");

    let mut g = build();
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::SEA_WITCH));
    // Hand {Gold, Silver}: both must go (no choice, so no decision), and only then does the
    // Estate come back.
    let d = expect_decision(&mut g);
    assert!(is_turn_decision(&d));
    assert_eq!(hand_and_play(&g), counts_of(&[id::ESTATE]), "the Estate returns after the discard");
}

/// Drive `g` (at a StartOrder decision or just before) through every possible sequence of
/// order choices; every other decision takes its first legal choice. Returns each finished
/// state with the order chosen.
fn all_orders(g: GameState, chosen: Vec<CardId>, out: &mut Vec<(GameState, Vec<CardId>)>) {
    let mut g = g;
    loop {
        let d = match adv(&mut g) {
            Step::Decision(d) => d,
            other => panic!("unexpected {other:?}"),
        };
        if is_turn_decision(&d) {
            out.push((g, chosen));
            return;
        }
        if d.kind == DecisionKind::StartOrder {
            for c in offered(&g) {
                let mut next = g;
                next.apply(Choice::Card(c), &mut NoEvents).unwrap();
                let mut path = chosen.clone();
                path.push(c);
                all_orders(next, path, out);
            }
            return;
        }
        let first = choices(&g)[0];
        g.apply(first, &mut NoEvents).unwrap();
    }
}

#[test]
fn every_order_is_legal_and_each_effect_resolves_exactly_once() {
    let mut g = new_state(&[id::HAVEN, id::CARAVAN, id::SEA_WITCH, id::SAILOR], 2);
    set_hand(&mut g, 0, &[id::COPPER, id::COPPER, id::ESTATE]);
    set_set_aside(&mut g, 0, &[id::GOLD]);
    set_deck_known(&mut g, 0, &[id::SILVER; 6]);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    inject(&mut g, id::CARAVAN, 1, 0);
    inject(&mut g, id::HAVEN, 1, id::GOLD);
    inject(&mut g, id::SAILOR, 1, 0);
    let before = total_cards(&g);

    let mut out = Vec::new();
    all_orders(g, Vec::new(), &mut out);
    // Four distinct, mutually relevant effects: 4! sequences.
    assert_eq!(out.len(), 24);
    let mut seen: Vec<Vec<CardId>> = out.iter().map(|(_, p)| p.clone()).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 24, "every order is reachable exactly once");
    for (g, order) in &out {
        assert_eq!(total_cards(g), before, "{order:?}: cards are conserved");
        assert_eq!(g.turn.start_pending.as_slice().len(), 0, "{order:?}");
        assert_eq!(g.players[0].pending_durations_len, 0, "{order:?}");
        assert!(g.players[0].set_aside.is_empty(), "{order:?}: Haven's card came back");
        // Haven +1 card; Caravan +1; Sea Witch +2 and -2; Sailor -1 or 0 (the default first choice
        // is Pass for the optional trash is not offered first, so a card may or may not go).
        let kept = hand_and_play(g).total() as i32 + g.trash.total() as i32;
        assert_eq!(kept, 3 + 1 + 1 + 2 - 2, "{order:?}");
        assert!(g.turn.coins >= 2, "{order:?}: Sailor's +$2");
    }
}

#[test]
fn a_throne_roomed_duration_resolves_its_times() {
    // Sea Witch played twice (entry times 2): +4 Cards, then discard 2 twice.
    let mut g = new_state(&[id::SEA_WITCH], 2);
    set_hand(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER; 6]);
    inject(&mut g, id::SEA_WITCH, 2, 0);
    let d = expect_decision(&mut g);
    assert!(is_discard_select(&d), "{:?}", d.kind);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::COPPER);
    let d = expect_decision(&mut g);
    assert!(is_turn_decision(&d));
    assert_eq!(g.players[0].discard.total(), 4, "two discards per play, two plays");
    assert_eq!(g.players[0].hand.total(), 1);

    // Mixed with another effect: one question, then the doubled effect runs both times.
    let mut g = new_state(&[id::TIDE_POOLS, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::ESTATE, id::ESTATE, id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER; 3]);
    inject(&mut g, id::TIDE_POOLS, 2, 0);
    inject(&mut g, id::CARAVAN, 1, 0);
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    choose(&mut g, Choice::Card(id::CARAVAN));
    let d = expect_decision(&mut g);
    assert!(is_discard_select(&d), "no second question: {:?}", d.kind);
    pick_while(&mut g, Zone::Hand, Act::Discard, id::ESTATE);
    let d = expect_decision(&mut g);
    assert!(is_turn_decision(&d));
    assert_eq!(g.players[0].discard.total(), 4, "Tide Pools' discard of 2 happened twice");
    assert_eq!(hand_and_play(&g).total(), 0, "3 Estates + 1 Copper drawn - 4 discarded; the Copper was played");
}

#[test]
fn an_outposts_extra_turn_starts_with_its_effects_in_order() {
    // Turn 1: Caravan, Sea Witch and Outpost are played; the next hand is 3 cards and the next
    // turn is player 0's again, starting with Caravan and Sea Witch pending (Outpost has nothing
    // to resolve).
    let mut g = new_state(&[id::CARAVAN, id::SEA_WITCH, id::OUTPOST, id::VILLAGE], 2);
    set_hand(&mut g, 0, &[id::VILLAGE, id::CARAVAN, id::SEA_WITCH, id::OUTPOST]);
    let mix = [id::COPPER, id::SILVER, id::GOLD, id::ESTATE, id::DUCHY, id::PROVINCE];
    set_deck_known(&mut g, 0, &[mix, mix, mix].concat());
    for c in [id::VILLAGE, id::CARAVAN, id::SEA_WITCH, id::OUTPOST] {
        // Village +2 Actions, Caravan +1, Sea Witch and Outpost one each.
        if g.turn.actions == 0 {
            break;
        }
        play(&mut g, c);
    }
    assert!(g.players[0].in_play.has(id::OUTPOST) && g.players[0].in_play.has(id::SEA_WITCH));
    // End the turn: pass the Buy phase's decisions until it's a turn decision again.
    loop {
        let d = expect_decision(&mut g);
        if d.kind == DecisionKind::StartOrder || g.turn.number > 1 {
            break;
        }
        pass(&mut g);
    }
    assert_eq!(g.turn.player, 0, "Outpost: the same player takes the next turn");
    assert!(g.turn.is_extra_turn);
    assert_eq!(g.players[0].hand.total(), 3, "Outpost's hand is 3 cards");
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(offered(&g), vec![id::CARAVAN, id::SEA_WITCH]);
    choose(&mut g, Choice::Card(id::CARAVAN));
    let d = expect_decision(&mut g);
    assert!(is_discard_select(&d), "{:?} {:?}", d.kind, g.players[0].hand);
    assert_eq!(g.players[0].hand.total(), 3 + 1 + 2, "the Caravan card and Sea Witch's 2 are in hand to discard from");
}

// ---------------------------------------------------------------------------------------
// Clerk
// ---------------------------------------------------------------------------------------

#[test]
fn clerk_is_one_of_the_effects_and_is_skipped_once_discarded() {
    let build = || {
        let mut g = new_state(&[id::CLERK, id::SEA_WITCH], 2);
        set_hand(&mut g, 0, &[id::CLERK, id::ESTATE, id::COPPER]);
        set_deck_known(&mut g, 0, &[id::COPPER, id::COPPER]);
        inject(&mut g, id::SEA_WITCH, 1, 0);
        g
    };
    // Clerk first: its play is offered (and played from hand).
    let mut g = build();
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(offered(&g), vec![id::SEA_WITCH, id::CLERK]);
    choose(&mut g, Choice::Card(id::CLERK));
    let d = expect_decision(&mut g);
    assert_eq!(d.kind, DecisionKind::YesNo { act: Act::Play });
    assert_eq!(d.subject, id::CLERK);
    choose(&mut g, Choice::Yes);
    assert_eq!(g.turn.coins, 2, "Clerk's +$2");
    let d = expect_decision(&mut g);
    assert!(matches!(d.kind, DecisionKind::Select { act: Act::Discard, .. }));

    // Sea Witch first and the Clerk is discarded: there is nothing left to play.
    let mut g = build();
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::SEA_WITCH));
    expect_decision(&mut g);
    choose(&mut g, Choice::Card(id::ESTATE)); // picks come in card order: Estate, then the forced Clerk
    let d = expect_decision(&mut g);
    assert!(is_turn_decision(&d), "no Clerk offer: {:?}", d.kind);
    assert_eq!(g.turn.coins, 3, "the three Coppers; Clerk was discarded before it could be played");
}

// ---------------------------------------------------------------------------------------
// Text format
// ---------------------------------------------------------------------------------------

#[test]
fn a_state_saved_mid_ordering_resumes_with_the_remaining_effects() {
    let mut g = new_state(&[id::SEA_WITCH, id::HAVEN, id::CARAVAN], 2);
    set_hand(&mut g, 0, &[id::GOLD, id::SILVER]);
    set_set_aside(&mut g, 0, &[id::ESTATE]);
    set_deck_known(&mut g, 0, &[id::COPPER; 4]);
    inject(&mut g, id::SEA_WITCH, 1, 0);
    inject(&mut g, id::HAVEN, 1, id::ESTATE);
    inject(&mut g, id::CARAVAN, 1, 0);
    expect_decision(&mut g);

    // Saved at the first question: every effect is still waiting (listed in `durations:`).
    let text = format_state(&g);
    assert!(text.contains("durations: Sea Witch, Haven (Estate), Caravan"), "{text}");
    let mut back = parse_state(&text).unwrap();
    let d = expect_decision(&mut back);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(offered(&back), offered(&g));

    // After Haven resolved, the effects still waiting are saved and come back.
    choose(&mut g, Choice::Card(id::HAVEN));
    assert_eq!(g.players[0].hand, counts_of(&[id::GOLD, id::SILVER, id::ESTATE]));
    let text = format_state(&g);
    assert!(text.contains("durations: Sea Witch, Caravan"), "{text}");
    let mut back = parse_state(&text).unwrap();
    let d = expect_decision(&mut back);
    assert_eq!(d.kind, DecisionKind::StartOrder);
    assert_eq!(offered(&back), vec![id::SEA_WITCH, id::CARAVAN]);
}

#[test]
fn the_decision_kind_is_not_a_play_decision_for_other_players() {
    // The decision belongs to the turn's player (player 1's turn: player 1 chooses).
    let mut g = new_state(&[id::SEA_WITCH, id::CARAVAN], 2);
    reset_turn(&mut g, 1);
    g.players[1].pending_durations[0] = PendingDuration { card: id::SEA_WITCH, times: 1, arg: 0, used: false };
    g.players[1].pending_durations[1] = PendingDuration { card: id::CARAVAN, times: 1, arg: 0, used: false };
    g.players[1].pending_durations_len = 2;
    let d = expect_decision(&mut g);
    assert_eq!((d.kind, d.player), (DecisionKind::StartOrder, 1));
    assert!(cards::start_order_sensitive(id::SEA_WITCH) && !cards::start_order_sensitive(id::CARAVAN));
}
