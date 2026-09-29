//! A fixed, un-searched policy for decisions that belong to someone other than the searching
//! player during their turn (Militia/Bureaucrat/Bandit victims). It is deliberately simple and
//! generic over `(Zone, Act)` rather than per-card, matching the engine's generic decision
//! shape: any future attack card that boils down to "discard/trash/topdeck a card from a zone"
//! is handled the same way without new code here.
//!
//! Policy: discard/trash the worst card to keep (Curse, then Victory, then cheapest else);
//! topdeck the cheapest Victory card. `Pass` is taken whenever it's legal and nothing is offered
//! that looks worth doing (Bureaucrat/Bandit only ever offer mandatory or clearly-forced picks
//! in the base set, so this mostly matters for the `min == 0` cases).

use dominion_engine::cards::{self, id, CardId, ModeOpt, VICTORY};
use dominion_engine::state::Act;
use dominion_engine::{Choice, Decision, DecisionKind, GameState};

/// Lower = worse to keep = preferred to discard/trash first.
fn worst_first_rank(c: CardId) -> u32 {
    if c == id::CURSE {
        0
    } else if cards::is(c, VICTORY) {
        1
    } else {
        // Cheap treasures/actions before expensive ones; id as a final, arbitrary tie-break.
        1_000 + cards::cost(c) as u32 * 64 + c as u32
    }
}

/// Higher = better to topdeck when forced to keep a Victory card around (put back the least
/// useful one — the cheapest — so the better ones stay in hand for scoring / reshuffle).
fn cheapest_first_rank(c: CardId) -> (u8, CardId) {
    (cards::cost(c), c)
}

/// Lower = a mode option a fixed (unmodeled) opponent would rather pick, by the atom's shape
/// rather than any specific card: gaining a Curse is the worst outcome, discarding/trashing
/// costs more the more cards it takes, and everything else is neutral. Generic over any future
/// mode card, not just Torturer.
fn mode_avoid_rank(opt: Option<ModeOpt>) -> i32 {
    match opt {
        Some(ModeOpt::Gain(c, _)) if c == id::CURSE => 1_000,
        Some(ModeOpt::TrashFromHand(n)) | Some(ModeOpt::DiscardFromHand(n)) => n as i32 * 10,
        Some(ModeOpt::DiscardHandDraw { .. }) => 50,
        _ => 0,
    }
}

/// Pick a `Choice` for a decision belonging to a player other than the searcher. Never
/// searched: always resolved on the spot with this one fixed rule of thumb.
pub fn default_policy(_state: &GameState, d: &Decision, choices: &[Choice]) -> Choice {
    debug_assert!(!choices.is_empty());
    match d.kind {
        DecisionKind::Select { act: Act::Discard, .. } | DecisionKind::Select { act: Act::Trash, .. } => {
            *choices
                .iter()
                .min_by_key(|c| match c {
                    Choice::Card(id) => worst_first_rank(*id),
                    Choice::Pass => u32::MAX, // prefer acting over passing when both offered
                    _ => u32::MAX,
                })
                .unwrap()
        }
        DecisionKind::Select { act: Act::Topdeck, .. } => *choices
            .iter()
            .min_by_key(|c| match c {
                Choice::Card(id) => cheapest_first_rank(*id),
                Choice::Pass => (u8::MAX, CardId::MAX),
                _ => (u8::MAX, CardId::MAX),
            })
            .unwrap(),
        // A reactive Mode decision (Torturer's victim, or any future mode card): pick the least
        // damaging option by shape, e.g. discard rather than gain a Curse.
        DecisionKind::Mode { .. } => {
            let table = d.source.map(cards::modes).unwrap_or(&[]);
            *choices
                .iter()
                .min_by_key(|c| match c {
                    Choice::Mode(i) => mode_avoid_rank(table.get(*i as usize).copied()),
                    _ => i32::MAX,
                })
                .unwrap()
        }
        _ => choices[0],
    }
}
