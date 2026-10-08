//! Base Set bot defaults. None yet: the Base cards are handled by the shape-based defaults in `strategy.rs` (a discard is
//! a discard, a trash a trash). Put a card's own policy here when it needs one (a `SetDefaults` method, called for the
//! card's set).

use super::SetDefaults;

pub(super) struct Base;

impl SetDefaults for Base {}
