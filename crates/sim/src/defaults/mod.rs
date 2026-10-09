//! Bot defaults, one module per expansion: what a scripted `Strategy` does about the decisions of a set's cards when its
//! rules say nothing. The generic policy (the gain list, the shape of a discard, a trash, a topdeck) stays in `strategy.rs`;
//! a card that needs its own policy (Scrying Pool's keep-or-discard, a mode card's fast choice, Stonemason's overpay)
//! implements the matching [`SetDefaults`] method in its set's file. Every method has a "no opinion" default, so a set
//! implements only what its cards need, and `strategy.rs` falls back to the shape-based defaults.
//!
//! This is a child module of `strategy` (see the `#[path]` there), so these files see `Strategy`'s private fields and
//! helpers. To add a set: a file here with a unit struct implementing `SetDefaults`, a `mod` line, and one arm in
//! [`with_set!`] (see `docs/adding-a-set.md`).

use dominion_engine::agent::PlayerView;
use dominion_engine::cards::{self, CardId, CardSet};
use dominion_engine::engine::{Choice, Decision};
use dominion_engine::state::{Act, Zone};

use crate::strategy::Strategy;

mod alchemy;
mod base;
mod cornucopia_guilds;
mod dark_ages;
mod intrigue;
mod promo;
mod prosperity;
mod seaside;

/// Expands `$body` once per set with `$S` naming that set's [`SetDefaults`] type, as a `match` on `$set`.
macro_rules! with_set {
    ($set:expr, $S:ident => $body:expr) => {
        match $set {
            CardSet::Base => {
                type $S = base::Base;
                $body
            }
            CardSet::Intrigue => {
                type $S = intrigue::Intrigue;
                $body
            }
            CardSet::Seaside => {
                type $S = seaside::Seaside;
                $body
            }
            CardSet::Prosperity => {
                type $S = prosperity::Prosperity;
                $body
            }
            CardSet::Alchemy => {
                type $S = alchemy::Alchemy;
                $body
            }
            CardSet::Promo => {
                type $S = promo::Promo;
                $body
            }
            CardSet::CornucopiaGuilds => {
                type $S = cornucopia_guilds::CornucopiaGuilds;
                $body
            }
            CardSet::DarkAges => {
                type $S = dark_ages::DarkAges;
                $body
            }
        }
    };
}

/// What a set's cards need beyond the generic bot policy. `None` / `false` mean "no opinion".
pub(super) trait SetDefaults {
    /// A `Select` decision caused by this set's card `source` that has its own policy, before the shape-based defaults.
    fn select(
        _s: &Strategy, _view: &PlayerView, _decision: &Decision, _source: CardId, _act: Act, _from: Zone, _choices: &[Choice],
    ) -> Option<Choice> {
        None
    }
    /// The fast rule-order choice for the mode decision of `card` (see `Strategy::default_mode`).
    fn default_mode(_s: &Strategy, _view: &PlayerView, _card: CardId, _choices: &[Choice]) -> Option<Choice> {
        None
    }
    /// A yes/no decision caused by `source` (a card of this set) that has its own policy, before the shape-based defaults.
    fn yes_no(_s: &Strategy, _view: &PlayerView, _decision: &Decision, _source: CardId) -> Option<Choice> {
        None
    }
    /// A `Name` decision caused by `source`.
    fn name(_s: &Strategy, _view: &PlayerView, _decision: &Decision, _source: CardId, _choices: &[Choice]) -> Option<Choice> {
        None
    }
    /// Whether to reveal a blocking Reaction (Moat) to `attack` (a card of this set): `Some(Choice::No)` to decline.
    fn reveal_to_attack(_s: &Strategy, _view: &PlayerView, _decision: &Decision, _attack: CardId) -> Option<Choice> {
        None
    }
    /// How much to overpay for `card`, when the card has a policy of its own.
    fn overpay(_s: &Strategy, _view: &PlayerView, _card: CardId, _max_coins: u8, _potion: bool) -> Option<Choice> {
        None
    }
    /// The most worth overpaying for `card` (the default: no cap).
    fn overpay_cap(_view: &PlayerView, _card: CardId) -> u32 {
        u32::MAX
    }
    /// The group of `card`'s start-of-turn effect when the bot orders them (lower first); see `Strategy::choose_start_order`.
    fn start_order_group(_card: CardId) -> Option<u8> {
        None
    }
    /// Whether `card`'s start-of-turn effect is a play (so that ordering it is a real trade-off worth searching).
    fn start_order_play(_card: CardId) -> bool {
        false
    }
    /// Whether the Action `card` is worth playing by default (it can cost more than it gives).
    fn worth_playing(_s: &Strategy, _view: &PlayerView, _card: CardId) -> bool {
        true
    }
    /// A Treasure that is best played after every other one, ranked among such Treasures (lower first).
    fn plays_last_rank(_card: CardId) -> Option<u8> {
        None
    }
    /// The play choice by rule order alone (no search), for a play decision of this set's card.
    fn rule_play(_s: &Strategy, _view: &PlayerView, _decision: &Decision, _choices: &[Choice]) -> Option<Choice> {
        None
    }
    /// A gain from outside the Supply (a Reward) when the gain list names none of the choices.
    fn default_side_gain(_choices: &[Choice]) -> Option<CardId> {
        None
    }
}

pub(super) fn select(
    s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, act: Act, from: Zone, choices: &[Choice],
) -> Option<Choice> {
    with_set!(cards::set_of(source), S => S::select(s, view, decision, source, act, from, choices))
}

pub(super) fn default_mode(s: &Strategy, view: &PlayerView, card: CardId, choices: &[Choice]) -> Option<Choice> {
    with_set!(cards::set_of(card), S => S::default_mode(s, view, card, choices))
}

pub(super) fn yes_no(s: &Strategy, view: &PlayerView, decision: &Decision) -> Option<Choice> {
    let source = decision.source?;
    with_set!(cards::set_of(source), S => S::yes_no(s, view, decision, source))
}

pub(super) fn name(s: &Strategy, view: &PlayerView, decision: &Decision, source: CardId, choices: &[Choice]) -> Option<Choice> {
    with_set!(cards::set_of(source), S => S::name(s, view, decision, source, choices))
}

pub(super) fn reveal_to_attack(s: &Strategy, view: &PlayerView, decision: &Decision, attack: CardId) -> Option<Choice> {
    with_set!(cards::set_of(attack), S => S::reveal_to_attack(s, view, decision, attack))
}

pub(super) fn overpay(s: &Strategy, view: &PlayerView, card: CardId, max_coins: u8, potion: bool) -> Option<Choice> {
    with_set!(cards::set_of(card), S => S::overpay(s, view, card, max_coins, potion))
}

pub(super) fn overpay_cap(view: &PlayerView, card: CardId) -> u32 {
    with_set!(cards::set_of(card), S => S::overpay_cap(view, card))
}

pub(super) fn start_order_group(card: CardId) -> Option<u8> {
    with_set!(cards::set_of(card), S => S::start_order_group(card))
}

pub(super) fn start_order_play(card: CardId) -> bool {
    with_set!(cards::set_of(card), S => S::start_order_play(card))
}

pub(super) fn worth_playing(s: &Strategy, view: &PlayerView, card: CardId) -> bool {
    with_set!(cards::set_of(card), S => S::worth_playing(s, view, card))
}

pub(super) fn plays_last_rank(card: CardId) -> Option<u8> {
    with_set!(cards::set_of(card), S => S::plays_last_rank(card))
}

/// By the card that caused the decision (`decision.source`).
pub(super) fn rule_play(s: &Strategy, view: &PlayerView, decision: &Decision, choices: &[Choice]) -> Option<Choice> {
    let source = decision.source?;
    with_set!(cards::set_of(source), S => S::rule_play(s, view, decision, choices))
}

pub(super) fn default_side_gain(choices: &[Choice]) -> Option<CardId> {
    base::Base::default_side_gain(choices)
        .or_else(|| intrigue::Intrigue::default_side_gain(choices))
        .or_else(|| seaside::Seaside::default_side_gain(choices))
        .or_else(|| prosperity::Prosperity::default_side_gain(choices))
        .or_else(|| alchemy::Alchemy::default_side_gain(choices))
        .or_else(|| promo::Promo::default_side_gain(choices))
        .or_else(|| cornucopia_guilds::CornucopiaGuilds::default_side_gain(choices))
        .or_else(|| dark_ages::DarkAges::default_side_gain(choices))
}
