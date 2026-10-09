//! Prosperity, 2nd edition, with Platinum and Colony: VP tokens, "when you gain" cards (Watchtower, Hoard, Collection, Tiara, Mint) and the Treasures with a choice.

use super::prelude::*;

/// Card ids of this set (dense, contiguous, and stable: saved states and the text format name cards, but
/// the ids index the card table).
pub mod ids {
    use crate::cards::CardId;
    pub const ANVIL: CardId = 86;
    pub const WATCHTOWER: CardId = 87;
    pub const BISHOP: CardId = 88;
    pub const CLERK: CardId = 89;
    pub const INVESTMENT: CardId = 90;
    pub const MONUMENT: CardId = 91;
    pub const QUARRY: CardId = 92;
    pub const TIARA: CardId = 93;
    pub const WORKERS_VILLAGE: CardId = 94;
    pub const CHARLATAN: CardId = 95;
    pub const CITY: CardId = 96;
    pub const COLLECTION: CardId = 97;
    pub const CRYSTAL_BALL: CardId = 98;
    pub const MAGNATE: CardId = 99;
    pub const MINT: CardId = 100;
    pub const RABBLE: CardId = 101;
    pub const VAULT: CardId = 102;
    pub const WAR_CHEST: CardId = 103;
    pub const GRAND_MARKET: CardId = 104;
    pub const HOARD: CardId = 105;
    pub const BANK: CardId = 106;
    pub const EXPAND: CardId = 107;
    pub const FORGE: CardId = 108;
    pub const KINGS_COURT: CardId = 109;
    pub const PEDDLER: CardId = 110;
    pub const PLATINUM: CardId = 111;
    pub const COLONY: CardId = 112;
}

const CARDS: &[(CardId, CardDef)] = &[
    (ids::ANVIL, prosperity(has_choice(c("Anvil", 3, TREASURE, 1, 0, 0, 0, 0)))), // discard a Treasure to gain up to $4?
    (ids::WATCHTOWER, prosperity(order_sensitive(c("Watchtower", 3, ACTION | REACTION, 0, 0, 0, 0, 0)))), // draws to 6: cards drawn first are wasted
    (ids::BISHOP, prosperity(order_sensitive(c("Bishop", 4, ACTION, 1, 0, 0, 0, 0)))), // what to trash (may be the +Actions card in hand)
    (ids::CLERK, prosperity(choice_free(c("Clerk", 4, ACTION | REACTION | ATTACK, 2, 0, 0, 0, 0)))), // only victims act
    (ids::INVESTMENT, prosperity(has_choice(c("Investment", 4, TREASURE, 0, 0, 0, 0, 0)))), // trash a card, then +$1 or trash this for VP
    (ids::MONUMENT, prosperity(choice_free(c("Monument", 4, ACTION, 2, 0, 0, 0, 0)))), // +1 VP token
    (ids::QUARRY, prosperity(choice_free(c("Quarry", 4, TREASURE, 1, 0, 0, 0, 0)))), // flat $1; the cost reduction is passive
    (ids::TIARA, prosperity(has_choice(c("Tiara", 4, TREASURE, 0, 0, 0, 0, 1)))), // which Treasure (if any) to play twice
    (ids::WORKERS_VILLAGE, prosperity(choice_free(c("Worker's Village", 4, ACTION, 0, 0, 1, 2, 1)))),
    (ids::CHARLATAN, prosperity(choice_free(c("Charlatan", 5, ACTION | ATTACK, 3, 0, 0, 0, 0)))),
    (ids::CITY, prosperity(choice_free(c("City", 5, ACTION, 0, 0, 1, 2, 0)))), // bonus from empty piles: see `reads_empty_piles`
    (ids::COLLECTION, prosperity(choice_free(c("Collection", 5, TREASURE, 2, 0, 0, 0, 1)))), // passive +1 VP on gain while in play
    (ids::CRYSTAL_BALL, prosperity(has_choice(c("Crystal Ball", 5, TREASURE, 1, 0, 0, 0, 0)))), // trash / discard / play the top card
    (ids::MAGNATE, prosperity(has_choice(c("Magnate", 5, ACTION, 0, 0, 0, 0, 0)))), // draws per treasure in hand: order matters
    (ids::MINT, prosperity(has_choice(c("Mint", 5, ACTION, 0, 0, 0, 0, 0)))), // which treasure to copy
    (ids::RABBLE, prosperity(choice_free(c("Rabble", 5, ACTION | ATTACK, 0, 0, 3, 0, 0)))), // only victims act
    (ids::VAULT, prosperity(order_sensitive(c("Vault", 5, ACTION, 0, 0, 2, 0, 0)))), // what to discard (may be the +Actions card in hand)
    (ids::WAR_CHEST, prosperity(has_choice(c("War Chest", 5, TREASURE, 0, 0, 0, 0, 0)))), // gain up to $5, not named
    (ids::GRAND_MARKET, prosperity(choice_free(c("Grand Market", 6, ACTION, 2, 0, 1, 1, 1)))), // can't be bought with a Copper in play
    (ids::HOARD, prosperity(choice_free(c("Hoard", 6, TREASURE, 2, 0, 0, 0, 0)))), // passive bonus Gold on bought Victory gains
    (ids::BANK, prosperity(has_choice(c("Bank", 7, TREASURE, 0, 0, 0, 0, 0)))), // value depends on play order: play it last
    (ids::EXPAND, prosperity(order_sensitive(c("Expand", 7, ACTION, 0, 0, 0, 0, 0)))), // what to trash and gain (may be the +Actions card in hand)
    (ids::FORGE, prosperity(order_sensitive(c("Forge", 7, ACTION, 0, 0, 0, 0, 0)))), // what to trash and gain (may be the +Actions card in hand)
    (ids::KINGS_COURT, prosperity(order_sensitive(plays(c("King's Court", 7, ACTION, 0, 0, 0, 0, 0), 3)))), // what to play three times (maybe the +Actions card itself)
    (ids::PEDDLER, prosperity(choice_free(c("Peddler", 8, ACTION, 1, 0, 1, 1, 0)))),
    (ids::PLATINUM, prosperity(choice_free(c("Platinum", 9, TREASURE, 5, 0, 0, 0, 0)))),
    (ids::COLONY, prosperity(c("Colony", 11, VICTORY, 0, 10, 0, 0, 0))),
];

/// The Prosperity cards (see the module docs of `sets`).
pub struct Prosperity;

impl SetRules for Prosperity {
    const CARDS: &'static [(CardId, CardDef)] = CARDS;
    const GAIN_LO: u128 =
        (1u128 << id::WATCHTOWER) | (1u128 << id::HOARD) | (1u128 << id::COLLECTION) | (1u128 << id::TIARA) | (1u128 << id::MINT);

    fn modes(card: CardId) -> &'static [ModeOpt] {
        match card {
            id::INVESTMENT => &[ModeOpt::Coins(1), ModeOpt::TrashSelfRevealVpPerTreasureType],
            _ => &[],
        }
    }

    fn start_order_sensitive(card: CardId) -> bool {
        matches!(card, id::CLERK)
    }

    #[inline]
    fn play<S: EventSink>(g: &mut GameState, card: CardId, p: u8, sink: &mut S) {
        use Act::*;
        match card {
            id::MONUMENT => {
                g.players[p as usize].vp_tokens += 1;
                sink.bonus(p, card, 0, 0, 0, 1);
            }
            id::CITY => {
                // +1 Card +2 Actions (vanilla); with 1+ empty piles +1 Card more, with 2+ also +1 Buy +$1.
                let empty = g.empty_piles();
                if empty >= 1 {
                    g.stack.push(draw_frame(p, card, 1));
                }
                if empty >= 2 {
                    g.turn.buys += 1;
                    g.turn.coins += 1;
                    sink.bonus(p, card, 0, 1, 1, 0);
                }
            }
            id::MAGNATE => {
                // Reveal your hand: +1 Card per Treasure in it (Curse counts too, under Charlatan).
                let hand = g.players[p as usize].hand;
                for (c, k) in hand.iter() {
                    for _ in 0..k {
                        sink.event(Event::Reveal { player: p, card: c });
                    }
                }
                let treasures = hand.iter().filter(|&(c, _)| g.is_treasure(c)).map(|(_, n)| n as u32).sum::<u32>() as u8;
                if treasures > 0 {
                    g.stack.push(draw_frame(p, card, treasures));
                }
            }
            id::ANVIL => g.stack.push(select(
                p, card, Zone::Hand, Discard, g.treasure_filter(), 0, 1,
                Then::GainFixedUpTo { max_cost: 4, filter: Filter::Any, dest: Dest::Discard },
            )),
            id::WATCHTOWER => {
                let have = g.players[p as usize].hand.total();
                let need = 6u32.saturating_sub(have) as u8;
                if need > 0 {
                    g.stack.push(draw_frame(p, card, need));
                }
            }
            id::BISHOP => {
                // +$1 is vanilla (above); +1 VP token is unconditional. The card's own trash is
                // mandatory if hand holds anything to trash ("Trash a card from your hand", no
                // "may"; each other player's is explicitly "may" and not an Attack: no Moat check).
                g.players[p as usize].vp_tokens += 1;
                sink.bonus(p, card, 0, 0, 0, 1);
                let n = g.num_players;
                for i in (1..n).rev() {
                    let v = (p + i) % n;
                    g.stack.push(select(v, card, Zone::Hand, Trash, Filter::Any, 0, 1, Then::Nothing));
                }
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::VpPerCost { per: 2 }));
            }
            id::CLERK => {
                // +$2 is vanilla (above). Each other player (Moat allowing) with 5+ cards in hand puts
                // one onto their deck. The hand is counted when the frame first runs, after a
                // Diplomat reaction (draw 2, discard 3) has resolved: a victim left with 4 cards
                // is not affected (`Frame::down_to_target` in its threshold form: `exact`).
                // Clerk's own start-of-turn play is `take_start_effect_at`.
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { down_to_target: true, exact: true, ..select(v, card, Zone::Hand, Topdeck, Filter::Any, 0, 5, Then::Nothing) });
                }
            }
            id::INVESTMENT => {
                g.stack.push(mode_frame(p, card, 1));
                g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 1, 1, Then::Nothing));
            }
            id::TIARA => g.stack.push(select(p, card, Zone::Hand, Play, g.treasure_filter(), 0, 1, Then::PlayPicked { times: 2 })),
            id::CHARLATAN => {
                // +$3 is vanilla (above); Curse becomes a Treasure this game via `is_treasure`.
                let (vs, n) = g.victims(sink);
                g.push_gain_to_each(&vs[..n], id::CURSE, card);
            }
            id::CRYSTAL_BALL => {
                // Look at the top card: may trash it, discard it, or (if Action/Treasure) play
                // it; declining all three (or nothing was there) leaves it on top. Mutually
                // exclusive for free: each optional Select below only finds the card if an
                // earlier one didn't already take it.
                g.stack.push(Frame { min: ALL, max: ALL, ..select(p, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                g.stack.push(select(p, card, Zone::Revealed, Discard, Filter::Any, 0, 1, Then::Nothing));
                g.stack.push(select(p, card, Zone::Revealed, Trash, Filter::Any, 0, 1, Then::Nothing));
                g.stack.push(select(p, card, Zone::Revealed, Play, Filter::ActionOrTreasure, 0, 1, Then::PlayPicked { times: 1 }));
                g.stack.push(Frame { max: 1, ..Frame::new(K::RevealTop, p, card) });
            }
            id::MINT => g.stack.push(select(p, card, Zone::Hand, Reveal, g.treasure_filter(), 0, 1, Then::GainCopyOfRevealed { dest: Dest::Discard })),
            id::RABBLE => {
                let (vs, n) = g.victims(sink);
                for &v in vs[..n].iter().rev() {
                    g.stack.push(Frame { ordered: true, ..select(v, card, Zone::Revealed, Topdeck, Filter::Any, ALL, ALL, Then::Nothing) });
                    g.stack.push(Frame { max: 3, then: Then::MoveMatchingToDiscard(Filter::ActionOrTreasure), ..Frame::new(K::RevealTop, v, card) });
                }
            }
            id::VAULT => {
                let n = g.num_players;
                for i in (1..n).rev() {
                    let v = (p + i) % n;
                    g.stack.push(Frame { exact: true, ..select(v, card, Zone::Hand, Discard, Filter::Any, 0, 2, Then::DrawIfCount { count: 2, draw: 1 }) });
                }
                g.stack.push(select(p, card, Zone::Hand, Discard, Filter::Any, 0, ALL, Then::CoinsPerPick(1)));
            }
            id::WAR_CHEST => {
                let left = (p + 1) % g.num_players;
                g.stack.push(Frame { chooser: left, zone: Zone::Supply, ..Frame::new(K::Name, p, card) });
            }
            id::EXPAND => g.stack.push(select(
                p, card, Zone::Hand, Trash, Filter::Any, 1, 1,
                Then::GainUpTo { plus: 3, filter: Filter::Any, dest: Dest::Discard, exact: false, dest_by_type: false },
            )),
            id::FORGE => g.stack.push(select(p, card, Zone::Hand, Trash, Filter::Any, 0, ALL, Then::GainExactCostSum { dest: Dest::Discard })),
            id::KINGS_COURT => g.stack.push(select(p, card, Zone::Hand, Play, Filter::Action, 0, 1, Then::PlayPicked { times: 3 })),
            // Bank's own value is dynamic ($1 per Treasure in play, counting itself, which is
            // already placed in `in_play` by the time this runs): no static `coins` in its
            // `CardDef` (that generic bonus applied above is 0), computed fresh here instead.
            id::BANK => {
                let n = g.treasures_in_play(p) as u16;
                g.turn.coins += n;
                sink.bonus(p, card, 0, 0, n, 0);
            }
            _ => {}
        }
    }

    const MAY_BUY_CARDS: &'static [CardId] = &[id::GRAND_MARKET];
    fn may_buy(g: &GameState, card: CardId) -> bool {
        match card {
            id::GRAND_MARKET => !g.players[g.turn.player as usize].in_play.has(id::COPPER),
            _ => true,
        }
    }

    fn pile_size(card: CardId, _num_players: usize) -> Option<u8> {
        (card == id::PLATINUM).then_some(12)
    }

    fn start_entry_repeats(card: CardId) -> bool {
        card == id::CLERK
    }

    /// City's bonus depends on how many Supply piles are empty. It is choice-free, but a card played before it that empties
    /// a pile changes it: a Curse-giving Attack (Witch, Sea Witch, Familiar, Charlatan) or a card that gains. Bots check for
    /// that before taking a play-order shortcut (`sim::strategy::empty_pile_risk`).
    fn reads_empty_piles(card: CardId) -> bool {
        card == id::CITY
    }

    /// Clerk: "at the start of your turn, you may play this from your hand", one merged entry for the copies in hand, which
    /// only counts those still in hand when it resolves.
    fn turn_start_entries(g: &mut GameState) {
        let pi = g.turn.player as usize;
        if g.in_supply(id::CLERK) {
            let k = g.players[pi].hand.get(id::CLERK);
            if k > 0 {
                g.turn.start_pending.push(PendingDuration { card: id::CLERK, times: k, arg: 0, used: 0 });
            }
        }
    }

    fn take_start_effect<S: EventSink>(g: &mut GameState, p: u8, e: &PendingDuration, _sink: &mut S) -> bool {
        if e.card != id::CLERK {
            return false;
        }
        // "You may play this from your hand": only if it's still there.
        if g.players[p as usize].hand.has(id::CLERK) {
            g.stack.push(Frame {
                zone: Zone::Hand, act: Act::Play, subject: id::CLERK,
                ..Frame::new(K::YesNo, p, id::CLERK)
            });
        }
        true
    }

    /// The "when you gain" triggers: Mint's trash, Hoard's Gold and Collection's VP (a table of decision-free bonuses), then
    /// the optional reactions (Watchtower, Tiara) as chained optional `Select` frames over the single gained card. Each is
    /// skipped automatically once an earlier one has already moved it (`avail == 0`), so "trash OR topdeck" falls out of
    /// the generic Select machinery for free. Order: Watchtower's trash, then its topdeck, then Tiara's topdeck; a bonus
    /// gain a trigger causes (Hoard's Gold) runs its own triggers first, so this gain's own reactions are asked first.
    fn gain_triggers<S: EventSink>(g: &mut GameState, p: u8, c: CardId, to: Dest, on_buy: bool, sink: &mut S) {
        let pi = p as usize;
        // Mint: "when you gain this, trash all non-Duration Treasures you have in play". The
        // "this turn" passives below count plays, so they survive the trash.
        if c == id::MINT {
            g.mint_trashes_treasures_in_play(p, sink);
        }
        for &(watcher, zone, trigger) in GAIN_TRIGGERS {
            // Every play of a card triggers independently (two Hoards on one bought Victory card
            // gain two Golds; one Hoard played twice with Tiara does too).
            let count = match zone {
                GainTriggerZone::Hand => g.players[pi].hand.get(watcher),
                GainTriggerZone::InPlay => g.plays_this_turn(p, watcher),
            };
            if count == 0 {
                continue;
            }
            match trigger {
                GainTrigger::HoardBoughtVictory => {
                    if on_buy && cards::is(c, cards::VICTORY) {
                        for _ in 0..count {
                            g.gain(p, id::GOLD, Dest::Discard, false, watcher, sink);
                        }
                    }
                }
                GainTrigger::CollectionAction => {
                    if cards::is(c, cards::ACTION) {
                        g.players[pi].vp_tokens += count as u16;
                        // One event per copy, so the log can say how many Collections paid.
                        for _ in 0..count {
                            sink.bonus(p, watcher, 0, 0, 0, 1);
                        }
                    }
                }
                // Decision-based: handled below (a frame per copy, not an immediate effect).
                GainTrigger::WatchtowerReact | GainTrigger::TiaraTopdeck => {}
            }
        }
        let zone = GameState::gain_dest_zone(to);
        let react = |source: CardId, act: Act| Frame {
            zone,
            act,
            filter: Filter::Card(c),
            max: 1,
            ..Frame::new(K::Select, p, source)
        };
        // A 2nd (or later) copy's reaction naturally finds nothing left to act on once an
        // earlier one has already moved the card (`avail == 0`, auto-skipped), so pushing one
        // frame per copy is both correct and cheap (copy counts are always small).
        // One offer however many Tiaras (or plays of one) are active: they all say the same.
        // A card gained onto the deck is already there: nothing to put (Watchtower can still
        // trash it).
        let on_deck = to == Dest::DeckTop;
        if !on_deck && g.plays_this_turn(p, id::TIARA) > 0 {
            g.stack.push(react(id::TIARA, Act::Topdeck));
        }
        for _ in 0..g.players[pi].hand.get(id::WATCHTOWER) {
            // Revealed from hand when used: the pick logs a Reaction event (`reveal_source`).
            if !on_deck {
                g.stack.push(Frame { reveal_source: true, ..react(id::WATCHTOWER, Act::Topdeck) });
            }
            g.stack.push(Frame { reveal_source: true, ..react(id::WATCHTOWER, Act::Trash) });
        }
    }
}

/// Where a "when you gain" watcher card must be for its trigger to be live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GainTriggerZone {
    /// A reaction in the gainer's hand (Watchtower).
    Hand,
    /// A card the gainer played this turn (Hoard, Collection, Tiara): once per play, and still
    /// active if it has left play since (`GameState::plays_this_turn`).
    InPlay,
}

/// A "when you gain a card" behaviour a watcher card contributes for its owner, once it's found in the right zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GainTrigger {
    /// May reveal to trash the gained card or put it onto the deck instead (Watchtower).
    WatchtowerReact,
    /// Gaining a Victory card you bought also gains a Gold (Hoard).
    HoardBoughtVictory,
    /// Gaining an Action card gives +1 VP token (Collection).
    CollectionAction,
    /// May put the gained card onto the deck instead of its normal destination (Tiara).
    TiaraTopdeck,
}

/// (watcher card, zone it must be found in, trigger).
static GAIN_TRIGGERS: &[(CardId, GainTriggerZone, GainTrigger)] = &[
    (id::WATCHTOWER, GainTriggerZone::Hand, GainTrigger::WatchtowerReact),
    (id::HOARD, GainTriggerZone::InPlay, GainTrigger::HoardBoughtVictory),
    (id::COLLECTION, GainTriggerZone::InPlay, GainTrigger::CollectionAction),
    (id::TIARA, GainTriggerZone::InPlay, GainTrigger::TiaraTopdeck),
];

impl GameState {
    /// Mint, 2nd edition: "When you gain this, trash all non-Duration Treasures you have in
    /// play". Any gain triggers it (bought, Artisan, ...), for whoever gains it; Treasures in
    /// hand or elsewhere are untouched, and the coins they made stay spent.
    pub(crate) fn mint_trashes_treasures_in_play<S: EventSink>(&mut self, p: u8, sink: &mut S) {
        let treasures = self.players[p as usize].in_play;
        for (tc, n) in treasures.iter() {
            if self.is_treasure(tc) && !cards::is(tc, cards::DURATION) {
                for _ in 0..n {
                    self.players[p as usize].in_play.remove(tc);
                    self.trash_card(p, tc, true, sink);
                }
            }
        }
        self.flush_trash_triggers(1, sink);
    }
}

/// Frame handlers of this set's cards (the generic frames' card-specific parts).
impl GameState {
    pub(crate) fn mode_trash_self_for_vp<S: EventSink>(&mut self, f: &Frame, sink: &mut S) {
        let p = f.player;
        // Trash the card that offered this choice (Investment; already in play),
        // then reveal the hand for +1 VP token per differently-named Treasure in it
        // (Curse counts too, under Charlatan). Only if this really trashed it: an
        // Investment played twice (Tiara) that was already trashed by its first play
        // has nothing left to trash, so the second play reveals nothing and scores
        // nothing.
        let ps = &mut self.players[p as usize];
        if !ps.in_play.remove(f.source) {
            return;
        }
        self.trash_card(p, f.source, true, sink);
        self.flush_trash_triggers(f.depth, sink);
        self.turn.used_up.add_or_drop(f.source, 1);
        let hand = self.players[p as usize].hand;
        let mut distinct = 0u16;
        for (c, n) in hand.iter() {
            if self.is_treasure(c) {
                distinct += 1;
                for _ in 0..n {
                    sink.event(Event::Reveal { player: p, card: c });
                }
            }
        }
        self.players[p as usize].vp_tokens += distinct;
        sink.bonus(p, f.source, 0, 0, 0, distinct);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Action or Treasure of the set says whether playing it is choice-free (`choice_free(..)`), has a
    /// choice (`has_choice(..)`) or is order-sensitive (`order_sensitive(..)`); bots skip the play-order search
    /// only for choice-free hands. When adding cards, mark each one and update the lists below deliberately.
    #[test]
    fn choice_free_and_order_sensitive_cards() {
        let names = |kind| CARDS.iter().map(|(_, d)| d).filter(|d| d.on_play == kind).map(|d| d.name).collect::<Vec<_>>();
        assert_eq!(
            names(cards::OnPlay::ChoiceFree),
            [
                "Clerk", "Monument", "Quarry", "Worker's Village", "Charlatan", "City",
                "Collection", "Rabble", "Grand Market", "Hoard", "Peddler", "Platinum",
            ],
            "the choice-free set changed: make sure each card really gives its player no decision and doesn't touch their deck"
        );
        assert_eq!(
            names(cards::OnPlay::OrderSensitive),
            [
                "Watchtower", "Bishop", "Vault", "Expand", "Forge", "King's Court",
            ],
            "the order-sensitive set changed: a card belongs here if playing a choice-free +Actions card before it can make it worse"
        );
    }
}
