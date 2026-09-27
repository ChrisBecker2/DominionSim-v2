//! Honesty: `GameState` as seen "from the outside" includes opponents' hidden hands, which a
//! real player (or the search agent) must not use directly. `determinize` produces a concrete
//! `GameState` consistent with everything `me` legitimately knows (see
//! `dominion_engine::PlayerView`'s doc comment): my own zones are exact, and every opponent's
//! *composition* is exact (all gains/trashes are public), but the split of an opponent's cards
//! between hand, deck-known and deck-unknown is not — from my point of view an opponent's whole
//! deck-known is just as unknown as their deck-unknown, since `PlayerView` never exposes it.
//!
//! So for each opponent we pool hand + deck_known + deck_unknown into one multiset and deal a
//! fresh random hand of the same size back out of it, leaving the remainder as `deck_unknown`
//! (with `deck_known` cleared — nothing about its order is assumed known any more). Discard is
//! left untouched: its composition is public and the engine never hides its order (there is no
//! "discard top" concept here beyond the multiset itself).
use dominion_engine::rng::Rng;
use dominion_engine::{GameState, PlayerView};

/// Return a copy of `state` with every opponent's hidden information (hand vs. deck split)
/// resampled consistently with what `me` honestly knows, and `chance_mode` turned on.
/// Thin wrapper over `PlayerView::determinize`.
pub fn determinize(state: &GameState, me: u8, rng: &mut Rng) -> GameState {
    PlayerView::new(state, me).determinize(rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dominion_engine::cards::id;
    use dominion_engine::text::parse_state;

    #[test]
    fn determinize_preserves_composition_and_my_zones() {
        let text = r#"
players: 2
kingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop
supply: Province=8, Duchy=8
turn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0
seed: 12345

[player 1]
hand: Village, Smithy, 3 Copper
deck top: Gold, Silver
deck: 5 Copper, 3 Estate
discard: Silver
in play:

[player 2]
hand: 5 Copper
deck top:
deck: 3 Copper, 3 Estate
discard:
in play:
"#;
        let s = parse_state(text).unwrap();
        let mut rng = Rng::new(42);
        for _ in 0..20 {
            let d = determinize(&s, 0, &mut rng);
            // My own zones are untouched.
            assert!(d.players[0].hand == s.players[0].hand);
            assert!(d.players[0].deck_known.counts().0 == s.players[0].deck_known.counts().0);
            assert!(d.players[0].deck_unknown == s.players[0].deck_unknown);
            assert!(d.players[0].discard == s.players[0].discard);
            assert!(d.chance_mode);

            // Opponent's total composition (hand+deck) is conserved.
            let before = {
                let mut c = s.players[1].deck_known.counts();
                c.add_all(&s.players[1].deck_unknown);
                c.add_all(&s.players[1].hand);
                c
            };
            let after = {
                let mut c = d.players[1].deck_known.counts();
                c.add_all(&d.players[1].deck_unknown);
                c.add_all(&d.players[1].hand);
                c
            };
            assert!(before == after, "composition changed");
            assert_eq!(d.players[1].hand.total(), s.players[1].hand.total());
            assert!(d.players[1].deck_known.is_empty());
        }
        let _ = id::VILLAGE; // silence unused import if cfg differs
    }
}
