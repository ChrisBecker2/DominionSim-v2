//! Human-editable text format for game states and card lists.
//!
//! Two related formats live here:
//!
//! - **Card lists** (`parse_counts` / `format_counts`): an unordered multiset, e.g.
//!   `"Village, Smithy, 3 Copper, 2x Estate"`. Names are matched case-insensitively with
//!   spaces ignored (see `cards::by_name`); a leading count may be `"N "` or `"Nx "` and
//!   defaults to 1.
//! - **Card sequences** (`parse_card_sequence` / `format_card_sequence`): an *ordered* list
//!   using the same token syntax, for the known-order top of a deck (first = top). A count
//!   prefix expands to that many consecutive identical entries.
//!
//! `parse_state` / `format_state` cover a full `GameState`: supply, trash, whose turn it is
//! and every player's zones. Parsing always resets the effect stack and pending decision
//! (`GameState::stack` empty, `GameState::pending = Pending::None`) — the format only
//! describes phase boundaries (start of the Action or Buy phase, or game over), never a
//! card effect that's mid-resolution. A state produced by `format_state` while a decision is
//! pending (e.g. mid-Cellar) is still valid text, but reloading it drops that in-progress
//! effect and starts fresh at the top of the current phase.
//!
//! `GameState`'s RNG (`rng: Rng`) cannot be read back out (its internal words are private to
//! `rng.rs`), so the `seed:` line is not a faithful memento of prior randomness: parsing always
//! seeds a *fresh* `Rng::new(seed)`, and `format_state` prints a number derived from the current
//! RNG stream (not the original construction seed) purely so the file has something to load. This
//! is fine for the intended use (editing states and continuing play), but future draws after a
//! round trip will not match what they would have been without the round trip.

use crate::cards::{self, CardId};
use crate::counts::Counts;
use crate::engine::Pending;
use crate::state::{DurationHeld, FrameStack, GameConfig, GameState, PendingDuration, PlayerState, SmallMultiset, TurnState, KNOWN_CAP, PENDING_DURATIONS_CAP};
use crate::state::Phase;

// ---------------------------------------------------------------------------------------
// Card lists (unordered multisets)
// ---------------------------------------------------------------------------------------

/// Format a multiset as `"3 Copper, Estate"` (ascending card-id order, count omitted when 1).
pub fn format_counts(c: &Counts) -> String {
    c.iter().map(|(id, n)| qty_str(id, n)).collect::<Vec<_>>().join(", ")
}

/// Parse a comma-separated multiset like `"Village, Smithy, 3 Copper, 2x Estate"`.
/// Blank/whitespace-only input parses to an empty multiset.
pub fn parse_counts(s: &str) -> Result<Counts, String> {
    let mut counts = Counts::EMPTY;
    for (n, id) in parse_entries(s)? {
        let cur = counts.get(id) as u32;
        if cur + n > 255 {
            return Err(format!("count too large for '{}'", cards::name(id)));
        }
        counts.add(id, n as u8);
    }
    Ok(counts)
}

// ---------------------------------------------------------------------------------------
// Card sequences (ordered; first = top for a deck)
// ---------------------------------------------------------------------------------------

/// Format an ordered sequence, run-length-encoding consecutive equal cards
/// (`"Gold, 2 Silver"`). Order is preserved (first item = first in `iter`).
pub fn format_card_sequence(iter: impl Iterator<Item = CardId>) -> String {
    let v: Vec<CardId> = iter.collect();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let c = v[i];
        let mut n = 1usize;
        while i + n < v.len() && v[i + n] == c {
            n += 1;
        }
        parts.push(qty_str(c, u8_or_u32_clamped(n)));
        i += n;
    }
    parts.join(", ")
}

// helper: run lengths beyond u8 are clamped only for display; decks never get that long
// (KNOWN_CAP caps them), so this is effectively exact.
#[allow(non_camel_case_types)]
type u8_ = u8;
fn u8_or_u32_clamped(n: usize) -> u8_ {
    n.min(255) as u8
}

/// Parse an ordered comma-separated sequence like `"Gold, Silver"` or `"3 Copper, Gold"`
/// (the latter expands to `[Copper, Copper, Copper, Gold]`). Blank input parses to `[]`.
pub fn parse_card_sequence(s: &str) -> Result<Vec<CardId>, String> {
    let mut out = Vec::new();
    for (n, id) in parse_entries(s)? {
        for _ in 0..n {
            out.push(id);
        }
    }
    Ok(out)
}

fn qty_str(id: CardId, n: u8) -> String {
    if n == 1 {
        cards::name(id).to_string()
    } else {
        format!("{} {}", n, cards::name(id))
    }
}

fn split_parts(s: &str) -> Vec<&str> {
    s.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).collect()
}

/// Parse a card list into `(count, card)` entries. Commas are optional: entries may be separated
/// by commas and/or spaces, e.g. `"Village, 3 Copper"` or `"Village Remodel 2x Gold Smithy"`.
/// Multi-word names ("Throne Room") are matched longest-first, and spacing/case are ignored
/// (`ThroneRoom`, `throne room`). A count is `N` or `Nx` before the name (`3 Copper`, `3x Copper`,
/// `3xCopper`).
fn parse_entries(s: &str) -> Result<Vec<(u32, CardId)>, String> {
    const MAX_NAME_WORDS: usize = 3;
    let toks: Vec<&str> = s.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        // Optional count: "3", "3x", or glued to the name as "3xCopper" / "3Copper".
        let tok = toks[i];
        let digits = tok.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(tok.len());
        let mut n = 1u32;
        let mut first = tok;
        if digits > 0 {
            n = tok[..digits].parse().map_err(|_| format!("invalid count in '{tok}'"))?;
            let rest = &tok[digits..];
            let rest = rest.strip_prefix('x').or_else(|| rest.strip_prefix('X')).unwrap_or(rest);
            if rest.is_empty() {
                i += 1;
                first = *toks.get(i).ok_or_else(|| format!("missing card name after '{tok}'"))?;
            } else {
                first = rest;
            }
        }
        // Longest card name starting here.
        let mut matched = None;
        for words in (1..=MAX_NAME_WORDS).rev() {
            if i + words > toks.len() {
                continue;
            }
            let mut name = first.to_string();
            for t in &toks[i + 1..i + words] {
                name.push(' ');
                name.push_str(t);
            }
            if let Some(id) = cards::by_name(&name) {
                matched = Some((id, words));
                break;
            }
        }
        let (id, words) = matched.ok_or_else(|| format!("unknown card '{first}'"))?;
        out.push((n, id));
        i += words;
    }
    Ok(out)
}

/// Parse a `Counts` multiset into a capped `SmallMultiset<N>` (Native Village/Island mats,
/// Smugglers' gain record), erroring instead of overflowing if the file names more distinct
/// cards than the cap allows: `SmallMultiset::add` would otherwise abort the whole process
/// (`panic = "abort"` in the release profile) on a malformed or hand-edited file.
fn parse_small_multiset<const N: usize>(s: &str, field: &str) -> Result<SmallMultiset<N>, String> {
    let counts = parse_counts(s)?;
    if counts.iter().count() > N {
        return Err(format!("too many distinct cards in '{field}' (max {N})"));
    }
    let mut m = SmallMultiset::EMPTY;
    for (c, n) in counts.iter() {
        m.add(c, n);
    }
    Ok(m)
}

/// Parse a plain comma-separated list of kingdom card names (no counts), e.g.
/// `"Cellar, Market, Village"`. Rejects non-kingdom cards and duplicates.
pub fn parse_kingdom(s: &str) -> Result<Vec<CardId>, String> {
    parse_kingdom_list(s)
}

fn parse_kingdom_list(s: &str) -> Result<Vec<CardId>, String> {
    let mut out = Vec::new();
    for (n, id) in parse_entries(s)? {
        let name = cards::name(id);
        if n != 1 {
            return Err(format!("kingdom cards must be listed individually, found '{n} {name}'"));
        }
        // Platinum and Colony may be listed to include them (Prosperity games with Colonies).
        if !cards::is_kingdom(id) && !cards::is_optional_basic(id) {
            return Err(format!("'{name}' is not a kingdom card"));
        }
        if !cards::is_ready(id) {
            return Err(format!("'{name}' is not implemented yet"));
        }
        if out.contains(&id) {
            return Err(format!("duplicate kingdom card '{name}'"));
        }
        out.push(id);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Phase
// ---------------------------------------------------------------------------------------

fn phase_str(p: Phase) -> &'static str {
    match p {
        Phase::Setup => "setup",
        Phase::Action => "action",
        Phase::Buy => "buy",
        Phase::CleanupDraw => "cleanup",
        Phase::GameOver => "gameover",
    }
}

fn parse_phase(s: &str) -> Result<Phase, String> {
    match s.to_lowercase().as_str() {
        "action" => Ok(Phase::Action),
        "buy" => Ok(Phase::Buy),
        "gameover" | "game_over" | "game-over" => Ok(Phase::GameOver),
        other => Err(format!("unknown phase '{other}' (expected action, buy, or gameover)")),
    }
}

// ---------------------------------------------------------------------------------------
// Full-state format
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct TurnFields {
    number: u16,
    player: u8, // zero-based, range-checked later against num_players
    phase: Phase,
    actions: u8,
    buys: u8,
    coins: u16,
    potions: u8,
    cost_reduction: u8,
    /// This turn was granted as an extra turn by an Outpost played last turn (Seaside).
    extra_turn: bool,
    /// A Silver or Gold played by this turn's player has already been trashed by someone else's
    /// Corsair this turn (Seaside).
    corsair_trashed: bool,
}

impl Default for TurnFields {
    fn default() -> Self {
        TurnFields {
            number: 1, player: 0, phase: Phase::Action, actions: 1, buys: 1, coins: 0, potions: 0, cost_reduction: 0,
            extra_turn: false, corsair_trashed: false,
        }
    }
}

fn parse_turn_line(line: &str, lineno: usize) -> Result<TurnFields, String> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    if toks.len() % 2 != 0 {
        return Err(format!("line {lineno}: malformed turn line (expected 'key: value' pairs)"));
    }
    let mut f = TurnFields::default();
    let mut i = 0;
    while i < toks.len() {
        let keytok = toks[i];
        let key = keytok
            .strip_suffix(':')
            .ok_or_else(|| format!("line {lineno}: expected 'key:' near '{keytok}'"))?
            .to_lowercase();
        let val = toks[i + 1];
        match key.as_str() {
            "turn" => f.number = val.parse().map_err(|_| format!("line {lineno}: invalid turn number '{val}'"))?,
            "player" => {
                let p: u32 = val.parse().map_err(|_| format!("line {lineno}: invalid player '{val}'"))?;
                if p == 0 {
                    return Err(format!("line {lineno}: player is 1-based (found 0)"));
                }
                f.player = (p - 1) as u8;
            }
            "phase" => f.phase = parse_phase(val).map_err(|e| format!("line {lineno}: {e}"))?,
            "actions" => f.actions = val.parse().map_err(|_| format!("line {lineno}: invalid actions '{val}'"))?,
            "buys" => f.buys = val.parse().map_err(|_| format!("line {lineno}: invalid buys '{val}'"))?,
            "coins" => f.coins = val.parse().map_err(|_| format!("line {lineno}: invalid coins '{val}'"))?,
            "potions" => f.potions = val.parse().map_err(|_| format!("line {lineno}: invalid potions '{val}'"))?,
            "cost_reduction" => {
                f.cost_reduction = val.parse().map_err(|_| format!("line {lineno}: invalid cost_reduction '{val}'"))?
            }
            "extra_turn" => {
                f.extra_turn = val.parse().map_err(|_| format!("line {lineno}: invalid extra_turn '{val}'"))?
            }
            "corsair_trashed" => {
                f.corsair_trashed = val.parse().map_err(|_| format!("line {lineno}: invalid corsair_trashed '{val}'"))?
            }
            other => return Err(format!("line {lineno}: unknown turn field '{other}'")),
        }
        i += 2;
    }
    Ok(f)
}

/// Render a full `GameState` as human-editable text. See the module docs for the shape.
pub fn format_state(state: &GameState) -> String {
    let n = state.num_players as usize;
    let mut out = String::new();

    out.push_str(&format!("players: {n}\n"));

    let mut kingdom: Vec<CardId> =
        (cards::FIRST_KINGDOM..cards::NUM_CARDS as CardId).filter(|&c| state.in_supply(c)).collect();
    kingdom.sort_by_key(|&c| cards::name(c));
    let kingdom_str = kingdom.iter().map(|&c| cards::name(c)).collect::<Vec<_>>().join(", ");
    out.push_str(&format!("kingdom: {kingdom_str}\n"));

    let mut supply_parts = Vec::new();
    for c in 0..cards::NUM_CARDS as CardId {
        if state.in_supply(c) {
            supply_parts.push(format!("{}={}", cards::name(c), state.supply.get(c)));
        }
    }
    out.push_str(&format!("supply: {}\n", supply_parts.join(", ")));

    out.push_str(&format!("trash: {}\n", format_counts(&state.trash)));

    out.push_str(&format!(
        "turn: {}  player: {}  phase: {}  actions: {}  buys: {}  coins: {}",
        state.turn.number,
        state.turn.player + 1,
        phase_str(state.turn.phase),
        state.turn.actions,
        state.turn.buys,
        state.turn.coins
    ));
    // Alchemy: only while a Potion is available.
    if state.turn.potions > 0 {
        out.push_str(&format!("  potions: {}", state.turn.potions));
    }
    // Only while something (Bridge) has reduced costs this turn.
    if state.turn.cost_reduction > 0 {
        out.push_str(&format!("  cost_reduction: {}", state.turn.cost_reduction));
    }
    // Seaside: only while relevant, so Base-only saves are unaffected.
    if state.turn.is_extra_turn {
        out.push_str("  extra_turn: true");
    }
    if state.turn.corsair_trashed_first {
        out.push_str("  corsair_trashed: true");
    }
    out.push('\n');

    // See module docs: this is not the original construction seed, just a deterministic
    // number derived from the live RNG so the file has something loadable to seed from.
    let mut r = state.rng;
    out.push_str(&format!("seed: {}\n", r.next_u64()));
    out.push_str(&format!("max_turns: {}\n", state.max_turns));
    // Duration cards (and Throne Room/King's Court multipliers) held in play this turn, not
    // discarded at cleanup (Seaside); only while relevant.
    if !state.turn.duration_held.is_empty() {
        let mut held = Counts::EMPTY;
        for (c, n) in state.turn.duration_held.iter() {
            held.set(c, n);
        }
        out.push_str(&format!("held: {}\n", format_counts(&held)));
    }

    for p in 0..n {
        out.push('\n');
        out.push_str(&format!("[player {}]\n", p + 1));
        let ps = &state.players[p];
        out.push_str(&format!("hand: {}\n", format_counts(&ps.hand)));
        out.push_str(&format!("deck top: {}\n", format_card_sequence(ps.deck_known.iter_top_down())));
        out.push_str(&format!("deck: {}\n", format_counts(&ps.deck_unknown)));
        out.push_str(&format!("deck bottom: {}\n", format_card_sequence(ps.deck_known_bottom.iter_front_to_back())));
        out.push_str(&format!("discard: {}\n", format_counts(&ps.discard)));
        out.push_str(&format!("in play: {}\n", format_counts(&ps.in_play)));
        out.push_str(&format!("set aside: {}\n", format_counts(&ps.set_aside)));
        out.push_str(&format!("turns: {}\n", ps.turns_taken));
        if ps.vp_tokens > 0 {
            out.push_str(&format!("vp tokens: {}\n", ps.vp_tokens));
        }
        if !ps.discard_next_cleanup.is_empty() {
            let mut c = Counts::EMPTY;
            for (card, n) in ps.discard_next_cleanup.iter() {
                c.set(card, n);
            }
            out.push_str(&format!("discard at next cleanup: {}
", format_counts(&c)));
        }
        if ps.pending_durations_len > 0 {
            let list = &ps.pending_durations[..ps.pending_durations_len as usize];
            out.push_str(&format!("durations: {}\n", format_pending_durations(list)));
        }
        // Seaside step 4 mats/records: only while relevant, so Base-only (and most Seaside)
        // saves are unaffected.
        if !ps.native_village_mat.is_empty() {
            out.push_str(&format!("native village: {}\n", format_counts(&ps.native_village_mat.counts())));
        }
        if !ps.island_mat.is_empty() {
            out.push_str(&format!("island: {}\n", format_counts(&ps.island_mat.counts())));
        }
        if !ps.last_turn_gains.is_empty() {
            out.push_str(&format!("last turn gains: {}\n", format_counts(&ps.last_turn_gains.counts())));
        }
    }

    out
}

/// Format the pending Duration effects list: `"Fishing Village x2, Haven (Silver), Sailor (used)"`.
fn format_pending_durations(list: &[PendingDuration]) -> String {
    list.iter()
        .map(|e| {
            let mut s = cards::name(e.card).to_string();
            if e.times != 1 {
                s.push_str(&format!(" x{}", e.times));
            }
            if e.arg != 0 {
                s.push_str(&format!(" ({})", cards::name(e.arg)));
            } else if e.used {
                s.push_str(" (used)");
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parse the `durations:` line (see `format_pending_durations`).
fn parse_pending_durations(s: &str) -> Result<Vec<PendingDuration>, String> {
    let mut out = Vec::new();
    for part in split_parts(s) {
        let mut rest = part;
        let mut arg = 0;
        let mut used = false;
        if let Some(open) = rest.find('(') {
            let inner = rest[open + 1..].trim_end().trim_end_matches(')').trim();
            if inner.eq_ignore_ascii_case("used") {
                used = true;
            } else {
                arg = cards::by_name(inner).ok_or_else(|| format!("unknown card '{inner}' in durations"))?;
            }
            rest = rest[..open].trim();
        }
        let mut times = 1u8;
        if let Some(xpos) = rest.rfind(" x") {
            if let Ok(n) = rest[xpos + 2..].trim().parse::<u32>() {
                times = n.min(u8::MAX as u32) as u8;
                rest = rest[..xpos].trim();
            }
        }
        let card = cards::by_name(rest).ok_or_else(|| format!("unknown card '{rest}' in durations"))?;
        out.push(PendingDuration { card, times, arg, used });
    }
    Ok(out)
}

/// Parse a full `GameState` from text produced by (or compatible with) `format_state`.
/// Errors are prefixed with `"line N: "` where possible.
pub fn parse_state(text: &str) -> Result<GameState, String> {
    let lines: Vec<&str> = text.lines().collect();

    let mut players_val: Option<(usize, String)> = None;
    let mut kingdom_val: Option<(usize, String)> = None;
    let mut supply_val: Option<(usize, String)> = None;
    let mut trash_val: Option<(usize, String)> = None;
    let mut held_val: Option<(usize, String)> = None;
    let mut seed_val: Option<(usize, String)> = None;
    let mut max_turns_val: Option<(usize, String)> = None;
    let mut turn_fields: Option<(usize, TurnFields)> = None;

    let mut i = 0usize;
    while i < lines.len() {
        let lineno = i + 1;
        let line = lines[i].trim();
        if line.is_empty() || line.starts_with('#') {
            i += 1;
            continue;
        }
        if line.starts_with('[') {
            break;
        }
        let colon = line.find(':').ok_or_else(|| format!("line {lineno}: expected 'key: value'"))?;
        let key = line[..colon].trim().to_lowercase();
        let rest = line[colon + 1..].trim();
        match key.as_str() {
            "players" => {
                if players_val.is_some() {
                    return Err(format!("line {lineno}: duplicate 'players' field"));
                }
                players_val = Some((lineno, rest.to_string()));
            }
            "kingdom" => kingdom_val = Some((lineno, rest.to_string())),
            "supply" => supply_val = Some((lineno, rest.to_string())),
            "trash" => trash_val = Some((lineno, rest.to_string())),
            "held" => held_val = Some((lineno, rest.to_string())),
            "seed" => seed_val = Some((lineno, rest.to_string())),
            "max_turns" | "max turns" => max_turns_val = Some((lineno, rest.to_string())),
            "turn" => turn_fields = Some((lineno, parse_turn_line(line, lineno)?)),
            other => return Err(format!("line {lineno}: unknown field '{other}'")),
        }
        i += 1;
    }

    let (players_line, players_str) =
        players_val.ok_or_else(|| "missing 'players' field".to_string())?;
    let num_players: usize = players_str
        .trim()
        .parse()
        .map_err(|_| format!("line {players_line}: invalid player count '{players_str}'"))?;
    if !(2..=crate::state::MAX_PLAYERS).contains(&num_players) {
        return Err(format!(
            "line {players_line}: player count must be 2..={}, got {num_players}",
            crate::state::MAX_PLAYERS
        ));
    }

    let (kingdom_line, kingdom_str) =
        kingdom_val.ok_or_else(|| "missing 'kingdom' field".to_string())?;
    let kingdom = parse_kingdom_list(&kingdom_str).map_err(|e| format!("line {kingdom_line}: {e}"))?;

    let seed: u64 = match &seed_val {
        Some((ln, s)) => s.trim().parse().map_err(|_| format!("line {ln}: invalid seed '{s}'"))?,
        None => 0,
    };
    let max_turns: u16 = match &max_turns_val {
        Some((ln, s)) => s.trim().parse().map_err(|_| format!("line {ln}: invalid max_turns '{s}'"))?,
        None => GameConfig::default().max_turns,
    };

    let cfg = GameConfig { num_players, kingdom, seed, max_turns };
    let mut state = GameState::new(&cfg);
    state.stack = FrameStack::default();
    state.pending = Pending::None;
    for p in 0..num_players {
        state.players[p] = PlayerState::default();
    }

    if let Some((ln, s)) = &supply_val {
        for pair in split_parts(s) {
            let (name_part, count_part) =
                pair.split_once('=').ok_or_else(|| format!("line {ln}: expected 'Card=Count' in supply, found '{pair}'"))?;
            let name = name_part.trim();
            let id = cards::by_name(name).ok_or_else(|| format!("line {ln}: unknown card '{name}'"))?;
            if !state.in_supply(id) {
                return Err(format!("line {ln}: '{name}' is not in this game's supply"));
            }
            let count: u32 =
                count_part.trim().parse().map_err(|_| format!("line {ln}: invalid count in '{pair}'"))?;
            if count > 255 {
                return Err(format!("line {ln}: count too large for '{name}'"));
            }
            state.supply.set(id, count as u8);
        }
    }

    if let Some((ln, s)) = &trash_val {
        state.trash = parse_counts(s).map_err(|e| format!("line {ln}: {e}"))?;
    }

    let (tf_line, tf) = turn_fields.unwrap_or((0, TurnFields::default()));
    if tf.player as usize >= num_players {
        return Err(format!(
            "line {tf_line}: player {} out of range (players: {num_players})",
            tf.player + 1
        ));
    }
    state.turn = TurnState {
        player: tf.player,
        phase: tf.phase,
        actions: tf.actions,
        buys: tf.buys,
        coins: tf.coins,
        potions: tf.potions,
        merchants: 0,
        silvers_played: 0,
        number: tf.number,
        // A loaded position starts mid-turn from the reader's point of view; don't re-announce.
        announced: true,
        // But do (re)attempt the start-of-turn resolution (Clerk, pending Durations): see
        // `TurnState::turn_start_resolved`'s doc comment.
        turn_start_resolved: false,
        played: Counts::EMPTY,
        cost_reduction: tf.cost_reduction,
        // Per-turn, within-turn bookkeeping: not part of the text format (see `format_state`'s
        // module docs on `played`), reset fresh on load. Same gap as `treasures_done` for
        // Treasury's two flags: a state saved mid-Buy-phase after already gaining a Victory card
        // (or after Treasury's offer already ran) loses that fact on reload. Accepted for now,
        // consistent with the existing gap.
        treasures_done: false,
        named_for_war_chest: Counts::EMPTY,
        duration_held: match &held_val {
            Some((ln, s)) => {
                let counts = parse_counts(s).map_err(|e| format!("line {ln}: {e}"))?;
                let mut held = DurationHeld::EMPTY;
                for (c, n) in counts.iter() {
                    held.add(c, n);
                }
                held
            }
            None => DurationHeld::EMPTY,
        },
        is_extra_turn: tf.extra_turn,
        outpost_grants_extra: false,
        corsair_trashed_first: tf.corsair_trashed,
        // Transient mid-resolution bookkeeping (never a text-format rest state): fresh on load.
        multiplier_card: 0,
        multiplier_expected: 0,
        multiplier_successes: 0,
        // Per-turn bookkeeping, not part of the text format: see the comment on `treasures_done`.
        gained_this_turn: SmallMultiset::EMPTY,
        gained_victory_in_buy: false,
        treasury_offered: false,
        blockading_curse: false,
    };

    // Player blocks.
    let mut seen = 0usize;
    while i < lines.len() {
        let lineno = i + 1;
        let line = lines[i].trim();
        if line.is_empty() || line.starts_with('#') {
            i += 1;
            continue;
        }
        if !line.starts_with('[') {
            return Err(format!("line {lineno}: expected '[player N]', found '{line}'"));
        }
        let inner = line.trim_start_matches('[').trim_end_matches(']').trim();
        let rest = inner
            .strip_prefix("player")
            .ok_or_else(|| format!("line {lineno}: expected '[player N]', found '{line}'"))?
            .trim();
        let pnum: usize = rest.parse().map_err(|_| format!("line {lineno}: expected '[player N]', found '{line}'"))?;
        seen += 1;
        if pnum != seen {
            return Err(format!("line {lineno}: expected '[player {seen}]', found '{line}'"));
        }
        if seen > num_players {
            return Err(format!("line {lineno}: player {seen} exceeds 'players: {num_players}'"));
        }
        let p = seen - 1;
        i += 1;

        let mut ps = PlayerState::default();
        while i < lines.len() {
            let ln2 = i + 1;
            let l2 = lines[i].trim();
            if l2.is_empty() || l2.starts_with('#') {
                i += 1;
                continue;
            }
            if l2.starts_with('[') {
                break;
            }
            let colon = l2.find(':').ok_or_else(|| format!("line {ln2}: expected 'key: value'"))?;
            let key = l2[..colon].trim().to_lowercase();
            let val = l2[colon + 1..].trim();
            match key.as_str() {
                "hand" => ps.hand = parse_counts(val).map_err(|e| format!("line {ln2}: {e}"))?,
                "deck top" => {
                    let seq = parse_card_sequence(val).map_err(|e| format!("line {ln2}: {e}"))?;
                    if seq.len() > KNOWN_CAP {
                        return Err(format!("line {ln2}: too many known deck cards (max {KNOWN_CAP})"));
                    }
                    for &c in seq.iter().rev() {
                        ps.deck_known.push_top(c);
                    }
                }
                "deck" => ps.deck_unknown = parse_counts(val).map_err(|e| format!("line {ln2}: {e}"))?,
                "deck bottom" => {
                    let seq = parse_card_sequence(val).map_err(|e| format!("line {ln2}: {e}"))?;
                    if seq.len() > KNOWN_CAP {
                        return Err(format!("line {ln2}: too many known deck-bottom cards (max {KNOWN_CAP})"));
                    }
                    for &c in seq.iter() {
                        ps.deck_known_bottom.push_top(c);
                    }
                }
                "discard" => ps.discard = parse_counts(val).map_err(|e| format!("line {ln2}: {e}"))?,
                "in play" => ps.in_play = parse_counts(val).map_err(|e| format!("line {ln2}: {e}"))?,
                "discard at next cleanup" => {
                    let c = parse_counts(val).map_err(|e| format!("line {ln2}: {e}"))?;
                    let mut d = DurationHeld::EMPTY;
                    for (card, n) in c.iter() {
                        d.add(card, n);
                    }
                    ps.discard_next_cleanup = d;
                }
                "set aside" => ps.set_aside = parse_counts(val).map_err(|e| format!("line {ln2}: {e}"))?,
                "turns" => ps.turns_taken = val.parse().map_err(|_| format!("line {ln2}: invalid turns '{val}'"))?,
                "vp tokens" | "vp_tokens" => {
                    ps.vp_tokens = val.parse().map_err(|_| format!("line {ln2}: invalid vp tokens '{val}'"))?
                }
                "durations" => {
                    let list = parse_pending_durations(val).map_err(|e| format!("line {ln2}: {e}"))?;
                    if list.len() > PENDING_DURATIONS_CAP {
                        return Err(format!("line {ln2}: too many pending durations (max {PENDING_DURATIONS_CAP})"));
                    }
                    ps.pending_durations_len = list.len() as u8;
                    for (i, e) in list.into_iter().enumerate() {
                        ps.pending_durations[i] = e;
                    }
                }
                "native village" => ps.native_village_mat = parse_small_multiset(val, "native village").map_err(|e| format!("line {ln2}: {e}"))?,
                "island" => ps.island_mat = parse_small_multiset(val, "island").map_err(|e| format!("line {ln2}: {e}"))?,
                "last turn gains" => ps.last_turn_gains = parse_small_multiset(val, "last turn gains").map_err(|e| format!("line {ln2}: {e}"))?,
                other => return Err(format!("line {ln2}: unknown field '{other}' in player block")),
            }
            i += 1;
        }
        state.players[p] = ps;
    }
    if seen != num_players {
        return Err(format!("expected {num_players} '[player N]' blocks, found {seen}"));
    }

    // "Played this turn" isn't stored in the text; the current player's in-play cards are the
    // best reconstruction (a card played twice by Throne Room counts once).
    state.turn.played = state.players[state.turn.player as usize].in_play;
    Ok(state)
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

#[cfg(test)]
mod no_comma_tests {
    use super::*;
    use crate::cards::id;

    #[test]
    fn card_lists_without_commas() {
        let c = parse_counts("Village Remodel Gold Smithy").unwrap();
        assert_eq!(c.total(), 4);
        assert!(c.has(id::VILLAGE) && c.has(id::REMODEL) && c.has(id::GOLD) && c.has(id::SMITHY));

        let c = parse_counts("Throne Room 3 Copper council room 2x Estate 2xSilver").unwrap();
        assert_eq!(c.get(id::THRONE_ROOM), 1);
        assert_eq!(c.get(id::COUNCIL_ROOM), 1);
        assert_eq!(c.get(id::COPPER), 3);
        assert_eq!(c.get(id::ESTATE), 2);
        assert_eq!(c.get(id::SILVER), 2);

        // Mixed commas and spaces; order preserved for sequences.
        assert_eq!(parse_card_sequence("Gold, Silver Throne Room").unwrap(), vec![id::GOLD, id::SILVER, id::THRONE_ROOM]);
        assert_eq!(parse_kingdom("Cellar Market Merchant Militia Mine Moat Remodel Smithy Village Workshop").unwrap().len(), 10);

        assert!(parse_counts("Village Bogus").unwrap_err().contains("Bogus"));
        assert!(parse_counts("3").is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::id;

    #[test]
    fn counts_round_trip() {
        let c = parse_counts("Village, Smithy, 3 Copper, 2x Estate").unwrap();
        assert_eq!(c.get(id::VILLAGE), 1);
        assert_eq!(c.get(id::SMITHY), 1);
        assert_eq!(c.get(id::COPPER), 3);
        assert_eq!(c.get(id::ESTATE), 2);

        let text = format_counts(&c);
        let c2 = parse_counts(&text).unwrap();
        assert!(c == c2);
    }

    #[test]
    fn counts_empty() {
        assert!(parse_counts("").unwrap() == Counts::EMPTY);
        assert!(parse_counts("   ").unwrap() == Counts::EMPTY);
        assert_eq!(format_counts(&Counts::EMPTY), "");
    }

    #[test]
    fn counts_case_and_space_insensitive() {
        let c = parse_counts("throneroom, THRONE ROOM, Throne_Room").unwrap();
        assert_eq!(c.get(id::THRONE_ROOM), 3);
    }

    #[test]
    fn counts_unknown_card_errors() {
        assert!(parse_counts("Not A Card").is_err());
    }

    #[test]
    fn sequence_preserves_order_and_run_length_encodes() {
        let seq = vec![id::GOLD, id::SILVER, id::SILVER];
        let text = format_card_sequence(seq.iter().copied());
        assert_eq!(text, "Gold, 2 Silver");
        let parsed = parse_card_sequence(&text).unwrap();
        assert_eq!(parsed, seq);
    }

    #[test]
    fn sequence_empty() {
        assert!(parse_card_sequence("").unwrap().is_empty());
        assert_eq!(format_card_sequence(std::iter::empty()), "");
    }

    fn assert_states_equal_ignoring_rng(a: &GameState, b: &GameState) {
        assert_eq!(a.num_players, b.num_players);
        assert!(a.supply == b.supply);
        assert_eq!(a.in_supply, b.in_supply);
        assert!(a.trash == b.trash);
        assert_eq!(TurnState { turn_start_resolved: false, ..a.turn }, TurnState { turn_start_resolved: false, ..b.turn });
        // `stack`/`pending` are deliberately not compared: parsing always resets them (see
        // module docs), so they're only meaningful to compare between two *parsed* states
        // (where both are trivially empty/None), not between a live mid-decision state and
        // its text round trip. `turn.turn_start_resolved` is excluded for the same reason: a
        // parsed state always attempts its start-of-turn resolution again on the next `advance`
        // (see `TurnState::turn_start_resolved`'s doc comment), even if the live state it came
        // from had already done so.
        assert_eq!(a.max_turns, b.max_turns);
        assert_eq!(a.chance_mode, b.chance_mode);
        assert_eq!(a.auto_single, b.auto_single);
        for p in 0..a.num_players as usize {
            assert!(a.players[p] == b.players[p], "player {p} zones differ");
        }
    }

    const EXAMPLE: &str = r#"
players: 2
kingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop
supply: Province=8, Duchy=8
trash:
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

    #[test]
    fn full_state_parses() {
        let s = parse_state(EXAMPLE).unwrap();
        assert_eq!(s.num_players, 2);
        assert_eq!(s.turn.number, 3);
        assert_eq!(s.turn.player, 0);
        assert_eq!(s.turn.phase, Phase::Action);
        assert_eq!(s.supply.get(id::PROVINCE), 8);
        assert_eq!(s.supply.get(id::DUCHY), 8);
        assert_eq!(s.players[0].hand.get(id::VILLAGE), 1);
        assert_eq!(s.players[0].hand.get(id::COPPER), 3);
        // "deck top: Gold, Silver" -- first listed = top.
        assert_eq!(s.players[0].deck_known.peek_top(), Some(id::GOLD));
        let top_down: Vec<CardId> = s.players[0].deck_known.iter_top_down().collect();
        assert_eq!(top_down, vec![id::GOLD, id::SILVER]);
        assert_eq!(s.players[0].deck_unknown.get(id::COPPER), 5);
        assert_eq!(s.players[0].discard.get(id::SILVER), 1);
        assert!(s.stack.is_empty());
        assert_eq!(s.pending, Pending::None);
    }

    #[test]
    fn full_state_round_trips() {
        let s1 = parse_state(EXAMPLE).unwrap();
        let text2 = format_state(&s1);
        let s2 = parse_state(&text2).unwrap();
        assert_states_equal_ignoring_rng(&s1, &s2);

        // And a further round trip is stable too.
        let text3 = format_state(&s2);
        let s3 = parse_state(&text3).unwrap();
        assert_states_equal_ignoring_rng(&s2, &s3);
    }

    #[test]
    fn fresh_game_round_trips() {
        use crate::engine::{ChoiceBuf, DecisionKind, NoEvents, Step};
        let cfg = GameConfig::default();
        let mut s = GameState::new(&cfg);
        let mut sink = NoEvents;
        let mut buf = ChoiceBuf::default();
        // `format_state` only describes phase boundaries (no in-progress card effect), so
        // drive arbitrary choices until we land on a top-level PlayAction/Buy decision
        // (equivalently: an empty effect stack), which is what the text format can represent.
        loop {
            match s.advance(&mut sink) {
                Step::Decision(d) => {
                    if matches!(d.kind, DecisionKind::PlayAction | DecisionKind::Buy) {
                        break;
                    }
                    s.legal_choices(&mut buf);
                    let c = buf.as_slice()[0];
                    s.apply(c, &mut sink).unwrap();
                }
                Step::GameOver => break,
                Step::TurnStart { .. } => {}
                Step::Chance { .. } => unreachable!(),
            }
        }
        assert!(s.stack.is_empty());
        let text = format_state(&s);
        let s2 = parse_state(&text).unwrap();
        assert_states_equal_ignoring_rng(&s, &s2);
    }

    #[test]
    fn buy_and_gameover_phases_parse() {
        let text = r#"
players: 2
kingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop
turn: 5  player: 2  phase: buy  actions: 0  buys: 1  coins: 5

[player 1]
hand:
deck top:
deck: 10 Copper
discard:
in play:

[player 2]
hand:
deck top:
deck: 10 Copper
discard:
in play: 5 Copper
"#;
        let s = parse_state(text).unwrap();
        assert_eq!(s.turn.phase, Phase::Buy);
        assert_eq!(s.turn.player, 1);
        assert_eq!(s.turn.coins, 5);

        let text2 = text.replace("phase: buy", "phase: gameover");
        let s2 = parse_state(&text2).unwrap();
        assert_eq!(s2.turn.phase, Phase::GameOver);
    }

    fn expect_err(text: &str) -> String {
        match parse_state(text) {
            Ok(_) => panic!("expected an error, got Ok"),
            Err(e) => e,
        }
    }

    #[test]
    fn errors_have_line_numbers() {
        let err = parse_counts("Not A Real Card").unwrap_err();
        assert!(err.contains("unknown card"));

        let bad = "players: 2\nkingdom: Village\n[player 1]\nhand: Nonexistent Card\n";
        let e = expect_err(bad);
        assert!(e.starts_with("line 4:"), "got: {e}");

        let missing_players = "kingdom: Village\n[player 1]\n";
        let e2 = expect_err(missing_players);
        assert!(e2.contains("players"));

        let bad_order = "players: 2\nkingdom: Village\n[player 2]\n";
        let e3 = expect_err(bad_order);
        assert!(e3.contains("expected '[player 1]'"), "got: {e3}");
    }

    #[test]
    fn kingdom_rejects_non_kingdom_and_duplicates() {
        assert!(parse_kingdom_list("Copper").is_err());
        assert!(parse_kingdom_list("Village, Village").is_err());
        assert!(parse_kingdom_list("Village, Smithy").is_ok());
    }
}
