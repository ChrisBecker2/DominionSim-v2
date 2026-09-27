//! Dominion (Base Set, 2nd edition) rules engine.
//!
//! Design notes:
//! - `GameState` is fixed-size and `Copy`; no heap allocation while playing.
//! - Hidden deck order is modelled exactly: a deck is a stack of *known* top cards over a
//!   multiset of *unknown* cards. Shuffling just merges the discard into the unknown multiset;
//!   randomness enters only when an unknown card is drawn/revealed (sampled with the state's RNG,
//!   or surfaced as `Step::Chance` in `chance_mode` for exact search).
//! - The engine is a resumable state machine (`advance` / `apply`); it never calls players.

pub mod agent;
pub mod cards;
pub mod counts;
pub mod effects;
pub mod engine;
pub mod rng;
pub mod state;
pub mod text;

pub use agent::{play_game, Agent, GameResult, PlayerView};
pub use cards::{id, CardId};
pub use counts::Counts;
pub use engine::{Choice, ChoiceBuf, Decision, DecisionKind, Event, EventSink, NoEvents, Pending, Step};
pub use state::{Act, Dest, EndReason, Filter, GameConfig, GameState, Phase, Zone};
pub use text::{format_counts, format_state, parse_counts, parse_kingdom, parse_state};
