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
//! Start-of-turn bookkeeping (Seaside Durations, Clerk, Prince): the effects of the turn about to
//! start are rebuilt on load from the `durations:` line (pending Durations), the hand (Clerk) and
//! the `prince:` line, so a state saved while the player is ordering them (`DecisionKind::
//! StartOrder`) lists the effects still waiting in `durations:` and, on load, starts the turn's
//! start-of-turn resolution again with just those. What was already resolved before the save is
//! not replayed. Gap: effects scheduled for the *next* turn during this one share that line, so a
//! state saved mid-turn and reloaded fires them at the start of the reloaded turn (the same gap
//! as for any Duration played earlier in the turn). The Prince's holdings are saved per player:
//! `prince:` lists the Action cards its Princes set aside, `prince stays:` the Princes (and Throne
//! Rooms / King's Courts) that hold cards and so stay in play (cleanup keeps them). If a
//! hand-written state has `prince:` but no `prince stays:`, one Prince in play is taken to hold them.
//!
//! Cornucopia & Guilds: `bane: <card>` and `ferryman: <card>` name the set-up piles (the Bane is also
//! in `kingdom:`, being a Supply pile; Ferryman's is not), `side piles: Card=N, ...` the cards left in
//! the piles outside the Supply (Ferryman's and the Rewards), and per player `coffers:`, `farmhands:`
//! (the cards set aside by Farmhands, played at the start of the next turn) and `cleanup aside:`
//! (Joust's Province). The turn line adds `extra_draw`, `guilds` and `gained_in_buy` while nonzero.
//!
//! Dark Ages: the Knights pile is one `kingdom:` entry (`Knights`; the Ruins pile comes with a Looter and is not listed), `supply:` lists the
//! top card of each of those piles with the number of cards left in the pile (`Sir Martin=9`; the other cards of the pile are not listed and
//! have none), and `side piles:` also holds `Spoils=`, `Madman=` and `Mercenary=`. The dealt order of those piles is the line `ruins pile:` /
//! `knights pile:` (every card, top first: `Survivors, 2 Ruined Village, ...`); a state without it gets the face-down part dealt from the cards
//! nobody has seen, with the state's RNG, and a line that is present decides the pile (and its count in `supply:`). Shelters are implied by the players' decks (`Hovel`,
//! `Necropolis`, `Overgrown Estate` anywhere in them).
//!
//! Empires: the split piles' halves and the Castles are listed in `supply:` (with their counts) like any pile; `kingdom:` names a split pile by its
//! top half and the Castles as `Castles`. `landmarks:` lists the Landmarks of the game (and `events:` the Events, of either set), `landmark vp:`
//! the VP tokens left on them (`Arena=10, ...`), `pile vp:` and `pile debt:` the VP and Debt tokens on Supply piles (`Silver=8, Temple=3`; a pile is
//! named as `cards::pile_of` names it), `obelisk:` the Obelisk's pile and `pass: done` that Mountain Pass has been used. Per player `debt:`,
//! `archive:` (the cards set aside under Archives) and `enchanted by:` (the seats whose Enchantress affects them) while nonzero, `donate: true`; the
//! turn line adds `gains`, `silvers`, `groundskeepers`, `charms`, `acted` and `doubled` while nonzero.
//!
//! Promo (Governor, Dismantle, Church, Marchland, Summon): per player `summoned:` lists the Action cards set aside by Summon (played at the start of the
//! next turn) and `church:` the cards each Church play set aside face down, one batch per play separated by `;` (`-` is an empty batch), oldest first
//! (`church: Copper, 2 Gold; -`); each batch has a `Church (Church)` entry in `durations:`.
//!
//! Rising Sun: `prophecy:` names the Prophecy of the game and `suns:` the Sun tokens left on it (none left: it is in effect), `riverboat:` the card
//! Riverboat sets aside. Per player `biding:` (the hand Biding Time set aside), `rapid:` (the cards Rapid Expansion set aside), `foresight:` (the cards
//! Foresight set aside) and `rs gold: true` (Kintsugi: a Gold gained this game) while nonzero; the turn line adds `daimyo` and `rs_gains` while nonzero.
//! Gap: Kind Emperor's and Sickness's start-of-turn effects are rebuilt from the Prophecy when a state is loaded at the start of a turn, so a state saved after
//! they resolved and loaded again resolves them once more.
//!
//! Menagerie: `ways:` lists the Ways of the game (with the Events in `events:`), `mouse:` the card set aside for Way of the Mouse, and `side piles:` holds
//! `Horse=` (the Horses left; the pile is outside the Supply). Per player `exile:` (the Exile mat), `invested:` (the copies on it that Invest put there),
//! `delayed:` (the cards set aside by Delay, Reap and Way of the Turtle, each with a `Delay (Card)` entry in `durations:`), `trashed last:` (Goatherd),
//! `gatekeepers:` (Gatekeepers whose Attack is on) and `seize: true` (Seize the Day used) while nonzero. A line `men turn:` holds the turn's bookkeeping, entries
//! separated by `;` (`kiln 1; livery 2; flags 3; cap 4; gains 2; last Silver; trashed 1; squirrel 0 0 2 0 0 0; names Silver, Gold; frogs Village`).
//!
//! Allies: `ally:` names the Ally of the game, `fam:` the Favor tokens on Supply piles (Family of Inventors), and one `pile order:` line for each split pile in the game lists
//! its cards top first as runs (`Herb Gatherer x4, Acolyte x4, Sorceress x4, Sibyl x4`; a line is optional while the pile is in the order its counts give, one of
//! each card in turn). Per player `favors:`, `keep:` (the cards Coastal Haven keeps in hand), `skips:` (Lich), `highwaymen:` and `warlords:` (their Attacks that are on) and
//! `garrison tokens:` while nonzero. A line `al turn:` holds the turn's bookkeeping, entries separated by `;` (`galleria 1; guild 2; skirm 1; garrison 1; bauble; limited; plays 2;
//! first treasure; shuffles 1; voyage; voyage next; island done`).
//!
//! Sailor: a Sailor played more than once (Throne Room, King's Court, several copies) is one entry
//! `Sailor x2`; `(used 1)` after it counts the gained Durations it has already offered to play, and
//! plain `(used)` (the older format) means all of them.
//!
//! Corsair and Blockade: per player, `shielded: 2 3` lists the players (1-based) whose Moat or
//! Lighthouse protected them when this player's lingering Attack was played, until that player's
//! next turn starts.
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
use crate::state::{
    AdvTurn, AlTurn, DurationHeld, EmpTurn, FrameStack, MenTurn, NocTurn, RenTurn, RsTurn, GameConfig, GameState, PendingDuration, PlayerState, SmallMultiset, TurnState, ADV_JOURNEY_DOWN, ADV_MINUS_CARD,
    ADV_MINUS_COIN, FARM_CAP, SUMMON_CAP, G_CHAMPION, G_COST_TOKEN, G_PLAY_TOKENS, G_TRASH_TOKEN, KNOWN_CAP, NUM_PILE_TOKENS, PENDING_DURATIONS_CAP, PRINCE_CAP,
    STAYS_CAP, StartList, TAVERN_CAP, TOK_COST, TOK_TRASH,
};
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
    const MAX_NAME_WORDS: usize = 4;
    // Words, and for each whether a comma follows it: a card name never runs across a comma
    // ("Gold, Mine" is two cards, not Rising Sun's Gold Mine).
    let mut toks: Vec<&str> = Vec::new();
    let mut comma_after: Vec<bool> = Vec::new();
    for part in s.split(',') {
        for t in part.split_whitespace() {
            toks.push(t);
            comma_after.push(false);
        }
        if let Some(last) = comma_after.last_mut() {
            *last = true;
        }
    }
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
            if i + words > toks.len() || comma_after[i..i + words - 1].iter().any(|&c| c) {
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
fn counts_of(cards: &[CardId]) -> Counts {
    let mut c = Counts::EMPTY;
    for &card in cards {
        c.add(card, 1);
    }
    c
}

/// Parse the `church:` line: batches separated by `;`, each a list of cards (or `-` for none), oldest first.
fn parse_church(s: &str) -> Result<crate::state::ChurchBatches, String> {
    let mut out = crate::state::ChurchBatches::EMPTY;
    for part in s.split(';') {
        let part = part.trim();
        let counts = if part == "-" || part.is_empty() { Counts::EMPTY } else { parse_counts(part)? };
        if counts.total() > 3 {
            return Err("a Church batch holds at most 3 cards".to_string());
        }
        if !out.begin() {
            return Err(format!("too many Church batches (max {})", crate::state::CHURCH_CAP));
        }
        for (c, n) in counts.iter() {
            for _ in 0..n {
                out.add(c);
            }
        }
    }
    Ok(out)
}

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
        // Either half of a split pile names the pile.
        let id = if cards::is_split_bottom(id) { cards::split_top(id).unwrap_or(id) } else { id };
        // Platinum and Colony may be listed to include them (Prosperity games with Colonies); so may the Events and the Landmarks.
        if !cards::is_kingdom(id) && !cards::is_optional_basic(id) && !cards::is_event(id) && !cards::is_landmark(id) && !cards::is_prophecy(id) && !cards::is_project(id) && !cards::is_way(id) && !cards::is_ally(id) {
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
        Phase::Night => "night",
        Phase::CleanupDraw => "cleanup",
        Phase::GameOver => "gameover",
    }
}

fn parse_phase(s: &str) -> Result<Phase, String> {
    match s.to_lowercase().as_str() {
        "setup" => Ok(Phase::Setup),
        "action" => Ok(Phase::Action),
        "buy" => Ok(Phase::Buy),
        "night" => Ok(Phase::Night),
        "gameover" | "game_over" | "game-over" => Ok(Phase::GameOver),
        other => Err(format!("unknown phase '{other}' (expected setup, action, buy, night, or gameover)")),
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
    /// Cornucopia & Guilds per-turn counters: Farrier's extra Clean-up cards, Merchant Guild plays,
    /// cards gained this Buy phase.
    extra_draw: u8,
    guilds: u8,
    gained_in_buy: u8,
    /// Adventures per-turn state (the Events bought, Travelling Fair, Sauna, Mission).
    adv: AdvTurn,
    /// Empires per-turn state.
    emp: EmpTurn,
    /// Nocturne per-turn state (the Boons' and Hexes' effects of this turn; the rest is reset on load).
    noc: NocTurn,
    /// Rising Sun per-turn state.
    rs: RsTurn,
    /// Renaissance per-turn state (Priests played, Cargo Ship plays left, Innovation used, an Action played).
    ren: RenTurn,
}

impl Default for TurnFields {
    fn default() -> Self {
        TurnFields {
            number: 1, player: 0, phase: Phase::Action, actions: 1, buys: 1, coins: 0, potions: 0, cost_reduction: 0,
            extra_turn: false, corsair_trashed: false, extra_draw: 0, guilds: 0, gained_in_buy: 0, adv: AdvTurn::default(), emp: EmpTurn::default(),
            noc: NocTurn::default(), rs: RsTurn::default(), ren: RenTurn::default(),
        }
    }
}

/// The `men turn:` line: Menagerie's per-turn bookkeeping, `key value` entries separated by `;`.
fn format_men_turn(m: &MenTurn) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in [("kiln", m.kiln), ("livery", m.livery), ("flags", m.flags), ("cap", m.cap), ("gains", m.gains), ("trashed", m.trashed)] {
        if v > 0 {
            parts.push(format!("{k} {v}"));
        }
    }
    if m.last != 0 {
        parts.push(format!("last {}", cards::name(m.last)));
    }
    if m.squirrel.iter().any(|&x| x != 0) {
        let v: Vec<String> = m.squirrel.iter().map(|x| x.to_string()).collect();
        parts.push(format!("squirrel {}", v.join(" ")));
    }
    if !m.names.is_empty() {
        parts.push(format!("names {}", format_counts(&m.names.counts())));
    }
    if !m.frogs.is_empty() {
        parts.push(format!("frogs {}", format_counts(&m.frogs.counts())));
    }
    parts.join("; ")
}

fn parse_men_turn(s: &str) -> Result<MenTurn, String> {
    let mut m = MenTurn::default();
    for part in s.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, val) = part.split_once(' ').unwrap_or((part, ""));
        let val = val.trim();
        let num = |what: &str| -> Result<u8, String> { val.parse().map_err(|_| format!("invalid {what} '{val}'")) };
        match key {
            "kiln" => m.kiln = num("kiln")?,
            "livery" => m.livery = num("livery")?,
            "flags" => m.flags = num("flags")?,
            "cap" => m.cap = num("cap")?,
            "gains" => m.gains = num("gains")?,
            "trashed" => m.trashed = num("trashed")?,
            "last" => m.last = cards::by_name(val).ok_or_else(|| format!("unknown card '{val}'"))?,
            "squirrel" => {
                for (i, w) in val.split_whitespace().enumerate().take(crate::state::MAX_PLAYERS) {
                    m.squirrel[i] = w.parse().map_err(|_| format!("invalid squirrel '{w}'"))?;
                }
            }
            "names" => m.names = parse_small_multiset::<{ crate::state::GAIN_NAMES_CAP }>(val, "names")?,
            "frogs" => m.frogs = parse_small_multiset::<{ crate::state::FROG_CAP }>(val, "frogs")?,
            other => return Err(format!("unknown men turn field '{other}'")),
        }
    }
    Ok(m)
}

/// The `al turn:` line: Allies' per-turn bookkeeping, `key value` entries separated by `;` (flags are a key alone).
fn format_al_turn(a: &AlTurn) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in [("galleria", a.galleria), ("guild", a.guild), ("skirm", a.skirm), ("skirm imm", a.skirm_imm), ("garrison", a.garrison), ("plays", a.hand_plays), ("shuffles", a.shuffles)] {
        if v > 0 {
            parts.push(format!("{k} {v}"));
        }
    }
    for (k, v) in [
        ("bauble", a.bauble),
        ("limited", a.limited),
        ("first treasure", a.t1),
        ("elder", a.elder),
        ("voyage", a.voyage),
        ("voyage next", a.voyage_next),
        ("island done", a.island_done),
    ] {
        if v {
            parts.push(k.to_string());
        }
    }
    parts.join("; ")
}

fn parse_al_turn(s: &str) -> Result<AlTurn, String> {
    let mut a = AlTurn::default();
    for part in s.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, val) = part.rsplit_once(' ').filter(|(_, v)| v.parse::<u8>().is_ok()).unwrap_or((part, ""));
        let num = || -> Result<u8, String> { val.parse().map_err(|_| format!("invalid {key} '{val}'")) };
        match key {
            "galleria" => a.galleria = num()?,
            "guild" => a.guild = num()?,
            "skirm" => a.skirm = num()?,
            "skirm imm" => a.skirm_imm = num()?,
            "garrison" => a.garrison = num()?,
            "plays" => a.hand_plays = num()?,
            "shuffles" => a.shuffles = num()?,
            "bauble" => a.bauble = true,
            "limited" => a.limited = true,
            "first treasure" => a.t1 = true,
            "elder" => a.elder = true,
            "voyage" => a.voyage = true,
            "voyage next" => a.voyage_next = true,
            "island done" => a.island_done = true,
            other => return Err(format!("unknown al turn field '{other}'")),
        }
    }
    a.live = (a.galleria > 0 || a.guild > 0 || a.skirm > 0 || a.garrison > 0 || a.bauble) as u8;
    Ok(a)
}

/// The `plu turn:` line: Plunder's per-turn bookkeeping, `key value` entries separated by `;` (flags are a key alone).
fn format_plu_turn(t: &crate::state::PluTurn) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in [
        ("rush", t.rush),
        ("mirror", t.mirror),
        ("avoid", t.avoid),
        ("road", t.road),
        ("harbor", t.harbor),
        ("watching", t.tm_watch),
        ("marked", t.tm_marked),
        ("first", t.first_by),
        ("tricksters", t.trickster),
    ] {
        if v > 0 {
            parts.push(format!("{k} {v}"));
        }
    }
    for (k, v) in [("insignia", t.insignia), ("deliver", t.deliver), ("journey", t.journey), ("prosper", t.prosper)] {
        if v {
            parts.push(k.to_string());
        }
    }
    parts.join("; ")
}

fn parse_plu_turn(s: &str) -> Result<crate::state::PluTurn, String> {
    let mut t = crate::state::PluTurn::default();
    for part in s.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, val) = part.rsplit_once(' ').filter(|(_, v)| v.parse::<u8>().is_ok()).unwrap_or((part, ""));
        let num = || -> Result<u8, String> { val.parse().map_err(|_| format!("invalid {key} '{val}'")) };
        match key {
            "rush" => t.rush = num()?,
            "mirror" => t.mirror = num()?,
            "avoid" => t.avoid = num()?,
            "road" => t.road = num()?,
            "harbor" => t.harbor = num()?,
            "watching" => t.tm_watch = num()?,
            "marked" => t.tm_marked = num()?,
            "first" => t.first_by = num()?,
            "tricksters" => t.trickster = num()?,
            "insignia" => t.insignia = true,
            "deliver" => t.deliver = true,
            "journey" => t.journey = true,
            "prosper" => t.prosper = true,
            other => return Err(format!("unknown plu turn field '{other}'")),
        }
    }
    Ok(t)
}

/// The names of the zones of `PluPlayer::z` in the text format.
fn plu_zone_name(code: u16) -> String {
    use crate::state::*;
    match code {
        Z_CAGE => "cage".to_string(),
        Z_EOT_UP => "eot up".to_string(),
        Z_EOT_DOWN => "eot down".to_string(),
        Z_EOT_DECK => "eot deck".to_string(),
        Z_PREP => "prep".to_string(),
        h => format!("holder{h}"),
    }
}

fn plu_zone_code(name: &str) -> Option<u16> {
    use crate::state::*;
    match name {
        "cage" => Some(Z_CAGE),
        "eot up" => Some(Z_EOT_UP),
        "eot down" => Some(Z_EOT_DOWN),
        "eot deck" => Some(Z_EOT_DECK),
        "prep" => Some(Z_PREP),
        h => h.strip_prefix("holder").and_then(|n| n.parse::<u16>().ok()).filter(|n| (Z_HOLDER0..Z_HOLDER0 + Z_HOLDERS).contains(n)),
    }
}

/// `plunder aside:` is `zone:Card=count` entries; `waiting:` is `Card[ xN][ via Card]` entries; `plunder:` is `key value` entries separated by `;`.
fn format_plu_aside(p: &crate::state::PluPlayer) -> String {
    let mut items: Vec<(u16, CardId, u8)> = p.z.iter().map(|(k, n)| (k >> 10, k & 1023, n)).collect();
    items.sort_unstable();
    items.iter().map(|&(z, c, n)| format!("{}:{}={}", plu_zone_name(z), cards::name(c), n)).collect::<Vec<_>>().join(", ")
}

fn parse_plu_aside(s: &str, p: &mut crate::state::PluPlayer) -> Result<(), String> {
    for part in split_parts(s) {
        let (zone, rest) = part.split_once(':').ok_or_else(|| format!("expected 'zone:Card=count' in plunder aside, found '{part}'"))?;
        let (name, n) = rest.rsplit_once('=').ok_or_else(|| format!("expected 'Card=count' in '{part}'"))?;
        let code = plu_zone_code(zone.trim()).ok_or_else(|| format!("unknown zone '{zone}'"))?;
        let card = cards::by_name(name.trim()).ok_or_else(|| format!("unknown card '{}'", name.trim()))?;
        let n: u8 = n.trim().parse().map_err(|_| format!("invalid count in '{part}'"))?;
        if !p.zadd(code, card, n) {
            return Err("too many different cards set aside".to_string());
        }
    }
    Ok(())
}

fn format_plu_waiting(p: &crate::state::PluPlayer) -> String {
    p.wait[..p.wait_len as usize]
        .iter()
        .map(|e| {
            let mut s = cards::name(e.card).to_string();
            if e.times > 1 {
                s.push_str(&format!(" x{}", e.times));
            }
            if e.mult != 0 {
                s.push_str(&format!(" via {}", cards::name(e.mult)));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_plu_waiting(s: &str, p: &mut crate::state::PluPlayer) -> Result<(), String> {
    for part in split_parts(s) {
        let (head, mult) = match part.split_once(" via ") {
            Some((h, m)) => (h.trim(), Some(m.trim())),
            None => (part, None),
        };
        let (name, times) = match head.rsplit_once(" x") {
            Some((n, t)) if t.parse::<u8>().is_ok() => (n.trim(), t.parse::<u8>().unwrap()),
            _ => (head, 1),
        };
        let card = cards::by_name(name).ok_or_else(|| format!("unknown card '{name}'"))?;
        let mult = match mult {
            Some(m) => cards::by_name(m).ok_or_else(|| format!("unknown card '{m}'"))?,
            None => 0,
        };
        if p.wait_len as usize >= crate::state::PLU_WAIT_CAP {
            return Err("too many waiting Durations".to_string());
        }
        p.wait[p.wait_len as usize] = crate::state::WaitEntry { card, mult, times };
        p.wait_len += 1;
    }
    Ok(())
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
            "extra_draw" => f.extra_draw = val.parse().map_err(|_| format!("line {lineno}: invalid extra_draw '{val}'"))?,
            "guilds" => f.guilds = val.parse().map_err(|_| format!("line {lineno}: invalid guilds '{val}'"))?,
            "gained_in_buy" => {
                f.gained_in_buy = val.parse().map_err(|_| format!("line {lineno}: invalid gained_in_buy '{val}'"))?
            }
            "ev_bought" => f.adv.bought = crate::state::EvMask::from_bits(val.parse().map_err(|_| format!("line {lineno}: invalid ev_bought '{val}'"))?),
            "fair" => f.adv.fair = val.parse().map_err(|_| format!("line {lineno}: invalid fair '{val}'"))?,
            "sauna" => f.adv.sauna = val.parse().map_err(|_| format!("line {lineno}: invalid sauna '{val}'"))?,
            "mission" => f.adv.mission = val.parse().map_err(|_| format!("line {lineno}: invalid mission '{val}'"))?,
            "no_buy" => f.adv.no_buy = val.parse().map_err(|_| format!("line {lineno}: invalid no_buy '{val}'"))?,
            "gains" => f.emp.gains = val.parse().map_err(|_| format!("line {lineno}: invalid gains '{val}'"))?,
            "silvers" => f.emp.silvers = val.parse().map_err(|_| format!("line {lineno}: invalid silvers '{val}'"))?,
            "groundskeepers" => f.emp.groundskeepers = val.parse().map_err(|_| format!("line {lineno}: invalid groundskeepers '{val}'"))?,
            "charms" => f.emp.charms = val.parse().map_err(|_| format!("line {lineno}: invalid charms '{val}'"))?,
            "acted" => f.emp.acted = val.parse().map_err(|_| format!("line {lineno}: invalid acted '{val}'"))?,
            "doubled" => f.emp.doubled = val.parse().map_err(|_| format!("line {lineno}: invalid doubled '{val}'"))?,
            "noc_gains" => f.noc.gains = val.parse().map_err(|_| format!("line {lineno}: invalid noc_gains '{val}'"))?,
            "envy" => f.noc.envy = val.parse().map_err(|_| format!("line {lineno}: invalid envy '{val}'"))?,
            "deluded" => f.noc.deluded = val.parse().map_err(|_| format!("line {lineno}: invalid deluded '{val}'"))?,
            "tracker" => f.noc.tracker = val.parse().map_err(|_| format!("line {lineno}: invalid tracker '{val}'"))?,
            "daimyo" => f.rs.daimyo = val.parse().map_err(|_| format!("line {lineno}: invalid daimyo '{val}'"))?,
            "rs_gains" => f.rs.gains = val.parse().map_err(|_| format!("line {lineno}: invalid rs_gains '{val}'"))?,
            "priests" => f.ren.priests = val.parse().map_err(|_| format!("line {lineno}: invalid priests '{val}'"))?,
            "cargo_uses" => f.ren.cargo = val.parse().map_err(|_| format!("line {lineno}: invalid cargo_uses '{val}'"))?,
            "innovation" => f.ren.innovation = val.parse().map_err(|_| format!("line {lineno}: invalid innovation '{val}'"))?,
            "ren_acted" => f.ren.acted = val.parse().map_err(|_| format!("line {lineno}: invalid ren_acted '{val}'"))?,
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

    // The members of the Knights and Ruins piles are not listed (the Knights pile is one kingdom card; the Ruins come with a Looter).
    let mut kingdom: Vec<CardId> = (cards::FIRST_KINGDOM..=cards::LAST_KINGDOM)
        .filter(|&c| (cards::is_kingdom(c) || cards::is_optional_basic(c)) && state.in_supply(c))
        .collect();
    if state.has_pile(&crate::sets::dark_ages::KNIGHTS_PILE) {
        kingdom.push(cards::id::KNIGHTS);
    }
    if state.emp.flags & crate::state::E_CASTLES != 0 {
        kingdom.push(cards::id::CASTLES);
    }
    kingdom.sort_by_key(|&c| cards::name(c));
    let kingdom_str = kingdom.iter().map(|&c| cards::name(c)).collect::<Vec<_>>().join(", ");
    out.push_str(&format!("kingdom: {kingdom_str}\n"));
    // Adventures: the Events of the game.
    if !state.events.is_empty() {
        let events: Vec<&str> = state.events.events().map(cards::name).collect();
        out.push_str(&format!("events: {}\n", events.join(", ")));
    }
    // Rising Sun: the Prophecy, the Sun tokens on it, Riverboat's card.
    if state.rs.prophecy != 0 {
        out.push_str(&format!("prophecy: {}\n", cards::name(state.rs.prophecy)));
        out.push_str(&format!("suns: {}\n", state.rs.suns));
    }
    if state.rs.riverboat != 0 {
        out.push_str(&format!("riverboat: {}\n", cards::name(state.rs.riverboat)));
    }
    // Renaissance: the Projects of the game, who holds each Artifact (by seat), the Fleet round.
    if state.ren.projects != 0 {
        let names: Vec<&str> = (0..cards::NUM_PROJECTS as CardId)
            .filter(|i| state.ren.projects & (1u32 << i) != 0)
            .map(|i| cards::name(cards::FIRST_PROJECT + i))
            .collect();
        out.push_str(&format!("projects: {}\n", names.join(", ")));
    }
    if state.ren.holder.iter().any(|&h| h != 0) {
        let held: Vec<String> = (0..cards::NUM_ARTIFACTS)
            .filter(|&i| state.ren.holder[i] != 0)
            .map(|i| format!("{}={}", cards::name(cards::FIRST_ARTIFACT + i as CardId), state.ren.holder[i]))
            .collect();
        out.push_str(&format!("artifacts: {}\n", held.join(", ")));
    }
    if state.ren.flags & crate::state::REN_FLEET_ROUND != 0 {
        let seats: Vec<String> = (0..state.num_players).filter(|q| state.ren.fleet & (1 << q) != 0).map(|q| (q + 1).to_string()).collect();
        out.push_str(&format!("fleet round: {}\n", seats.join(" ")));
    }
    // Menagerie: the Ways of the game, Way of the Mouse's card, the turn's bookkeeping.
    if state.men.ways != 0 {
        let names: Vec<&str> =
            (0..cards::NUM_WAYS as CardId).filter(|i| state.men.ways & (1u32 << i) != 0).map(|i| cards::name(cards::FIRST_WAY + i)).collect();
        out.push_str(&format!("ways: {}\n", names.join(", ")));
    }
    if state.men.mouse != 0 {
        out.push_str(&format!("mouse: {}\n", cards::name(state.men.mouse)));
    }
    if state.turn.men != MenTurn::default() {
        out.push_str(&format!("men turn: {}\n", format_men_turn(&state.turn.men)));
    }
    // Allies: the Ally, Family of Inventors' tokens, the split piles' order, the turn's bookkeeping.
    if state.al.ally != 0 {
        out.push_str(&format!("ally: {}\n", cards::name(state.al.ally)));
    }
    if !state.al.fam.is_empty() {
        out.push_str(&format!("fam: {}\n", format_counts(&state.al.fam.counts())));
    }
    for i in 0..crate::state::AL_PILES {
        let (order, len) = state.al_pile_cards(i);
        if len > 0 {
            let mut parts: Vec<String> = Vec::new();
            let mut k = 0;
            while k < len {
                let run = order[k..len].iter().take_while(|&&c| c == order[k]).count();
                parts.push(qty_str(order[k], run as u8));
                k += run;
            }
            out.push_str(&format!("pile order: {}\n", parts.join(", ")));
        }
    }
    if state.turn.al != AlTurn::default() {
        out.push_str(&format!("al turn: {}\n", format_al_turn(&state.turn.al)));
    }
    // Plunder: the Traits and their piles, the turn's bookkeeping.
    if state.plu.traits != 0 {
        let items: Vec<String> = (0..cards::NUM_TRAITS)
            .filter(|i| state.plu.traits & (1 << i) != 0)
            .map(|i| format!("{} on {}", cards::name(cards::FIRST_TRAIT + i as CardId), cards::name(state.plu.trait_pile[i])))
            .collect();
        out.push_str(&format!("traits: {}\n", items.join(", ")));
    }
    if state.turn.plu != crate::state::PluTurn::default() {
        out.push_str(&format!("plu turn: {}\n", format_plu_turn(&state.turn.plu)));
    }
    // Empires: the Landmarks and the tokens on them and on the piles.
    format_empires(state, &mut out);
    // Nocturne: the Boons and Hexes decks, Lost in the Woods.
    format_nocturne(state, &mut out);
    // Cornucopia & Guilds set-up piles (see the module docs).
    if state.bane != 0 {
        out.push_str(&format!("bane: {}\n", cards::name(state.bane)));
    }
    if state.ferryman != 0 {
        out.push_str(&format!("ferryman: {}\n", cards::name(state.ferryman)));
    }
    let mut side = Vec::new();
    if state.ferryman != 0 {
        side.push(format!("{}={}", cards::name(state.ferryman), state.ferryman_left));
    }
    if state.in_supply(cards::id::JOUST) {
        for (i, &n) in state.rewards.iter().enumerate() {
            side.push(format!("{}={}", cards::name(cards::FIRST_REWARD + i as CardId), n));
        }
    }
    // Dark Ages: Spoils, Madman and Mercenary, while a card that gains them is in the game (or a pile has cards).
    let da_pile = |n: u8, gainers: &[CardId], card: CardId, side: &mut Vec<String>| {
        if n > 0 || gainers.iter().any(|&g| state.in_supply(g)) {
            side.push(format!("{}={}", cards::name(card), n));
        }
    };
    da_pile(state.spoils, &[cards::id::BANDIT_CAMP, cards::id::MARAUDER, cards::id::PILLAGE], cards::id::SPOILS, &mut side);
    da_pile(state.madman, &[cards::id::HERMIT], cards::id::MADMAN, &mut side);
    da_pile(state.mercenary, &[cards::id::URCHIN], cards::id::MERCENARY, &mut side);
    // Adventures: the Traveller upgrade piles of the lines in the game.
    for i in 0..cards::NUM_TRAV {
        let line = if i < 4 { cards::id::PAGE } else { cards::id::PEASANT };
        if state.in_supply(line) {
            side.push(format!("{}={}", cards::name(cards::FIRST_TRAV + i as CardId), state.trav[i]));
        }
    }
    // Plunder: the Loot cards left in the pile.
    if state.plu.flags & crate::state::PF_LOOT != 0 {
        for i in 0..cards::NUM_LOOT as CardId {
            side.push(format!("{}={}", cards::name(cards::FIRST_LOOT + i), state.supply.get(cards::FIRST_LOOT + i)));
        }
    }
    // Menagerie: the Horses left.
    if state.has_horse_pile() {
        side.push(format!("{}={}", cards::name(crate::sets::menagerie::ids::HORSE), state.supply.get(crate::sets::menagerie::ids::HORSE)));
    }
    // Nocturne: the piles outside the Supply that the cards of the game bring.
    for (i, c) in [cards::id::WILL_O_WISP, cards::id::WISH, cards::id::BAT, cards::id::IMP, cards::id::GHOST].into_iter().enumerate() {
        if state.noc.piles[i] > 0 || nocturne_pile_wanted(state, c) {
            side.push(format!("{}={}", cards::name(c), state.noc.piles[i]));
        }
    }
    if !side.is_empty() {
        out.push_str(&format!("side piles: {}\n", side.join(", ")));
    }

    let mut supply_parts = Vec::new();
    for c in 0..cards::NUM_ZONE_IDS as CardId {
        // A mixed pile lists only its top card (the other members have none).
        if state.in_supply(c) && (c < cards::FIRST_PILE_CARD || state.supply.get(c) > 0) {
            supply_parts.push(format!("{}={}", cards::name(c), state.supply.get(c)));
        }
    }
    out.push_str(&format!("supply: {}\n", supply_parts.join(", ")));
    // Dark Ages: the Ruins and Knights piles in their dealt order, top first (the top card is the one in `supply`).
    for (pile, key) in [(&crate::sets::dark_ages::RUINS_PILE, "ruins pile"), (&crate::sets::dark_ages::KNIGHTS_PILE, "knights pile")] {
        let (order, len) = state.pile_cards(pile);
        if len > 0 {
            let mut parts: Vec<String> = Vec::new();
            let mut i = 0;
            while i < len {
                let run = order[i..len].iter().take_while(|&&c| c == order[i]).count();
                parts.push(qty_str(order[i], run as u8));
                i += run;
            }
            out.push_str(&format!("{key}: {}\n", parts.join(", ")));
        }
    }

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
    // Cornucopia & Guilds: only while nonzero.
    if state.turn.extra_draw > 0 {
        out.push_str(&format!("  extra_draw: {}", state.turn.extra_draw));
    }
    if state.turn.guilds > 0 {
        out.push_str(&format!("  guilds: {}", state.turn.guilds));
    }
    if state.turn.gained_in_buy > 0 {
        out.push_str(&format!("  gained_in_buy: {}", state.turn.gained_in_buy));
    }
    // Adventures: only while nonzero.
    let a = &state.turn.adv;
    if !a.bought.is_empty() {
        out.push_str(&format!("  ev_bought: {}", a.bought.bits()));
    }
    if a.fair {
        out.push_str("  fair: true");
    }
    if a.sauna > 0 {
        out.push_str(&format!("  sauna: {}", a.sauna));
    }
    if a.mission {
        out.push_str("  mission: true");
    }
    if a.no_buy {
        out.push_str("  no_buy: true");
    }
    // Empires: only while nonzero.
    let e = &state.turn.emp;
    for (name, v) in [("gains", e.gains), ("silvers", e.silvers), ("groundskeepers", e.groundskeepers), ("charms", e.charms)] {
        if v > 0 {
            out.push_str(&format!("  {name}: {v}"));
        }
    }
    if e.acted {
        out.push_str("  acted: true");
    }
    if e.doubled {
        out.push_str("  doubled: true");
    }
    // Nocturne: only while nonzero.
    let nt = &state.turn.noc;
    if nt.gains > 0 {
        out.push_str(&format!("  noc_gains: {}", nt.gains));
    }
    for (name, v) in [("envy", nt.envy), ("deluded", nt.deluded), ("tracker", nt.tracker)] {
        if v {
            out.push_str(&format!("  {name}: true"));
        }
    }
    // Rising Sun: only while nonzero.
    for (name, v) in [("daimyo", state.turn.rs.daimyo), ("rs_gains", state.turn.rs.gains)] {
        if v > 0 {
            out.push_str(&format!("  {name}: {v}"));
        }
    }
    // Renaissance: only while nonzero.
    let rt = &state.turn.ren;
    for (name, v) in [("priests", rt.priests), ("cargo_uses", rt.cargo)] {
        if v > 0 {
            out.push_str(&format!("  {name}: {v}"));
        }
    }
    for (name, v) in [("innovation", rt.innovation), ("ren_acted", rt.acted)] {
        if v {
            out.push_str(&format!("  {name}: true"));
        }
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
        if ps.ren.villagers > 0 {
            out.push_str(&format!("villagers: {}\n", ps.ren.villagers));
        }
        if ps.ren.projects != 0 {
            let names: Vec<&str> = (0..cards::NUM_PROJECTS as CardId)
                .filter(|i| ps.ren.projects & (1u32 << i) != 0)
                .map(|i| cards::name(cards::FIRST_PROJECT + i))
                .collect();
            out.push_str(&format!("owned projects: {}\n", names.join(", ")));
        }
        if state.ren.plot[p] > 0 {
            out.push_str(&format!("plot: {}\n", state.ren.plot[p]));
        }
        if !ps.ren.cargo.is_empty() {
            out.push_str(&format!("cargo: {}\n", format_counts(&ps.ren.cargo.counts())));
        }
        if !ps.ren.research.is_empty() {
            out.push_str(&format!("research: {}\n", format_counts(&ps.ren.research.counts())));
        }
        if ps.coffers > 0 {
            out.push_str(&format!("coffers: {}\n", ps.coffers));
        }
        if !ps.men.exile.is_empty() {
            out.push_str(&format!("exile: {}\n", format_counts(&ps.men.exile.counts())));
        }
        if !ps.men.invest.is_empty() {
            out.push_str(&format!("invested: {}\n", format_counts(&ps.men.invest.counts())));
        }
        if !ps.men.aside.is_empty() {
            out.push_str(&format!("delayed: {}\n", format_counts(&ps.men.aside.counts())));
        }
        if ps.men.trashed_last > 0 {
            out.push_str(&format!("trashed last: {}\n", ps.men.trashed_last));
        }
        if ps.men.gk > 0 {
            out.push_str(&format!("gatekeepers: {}\n", ps.men.gk));
        }
        if ps.men.flags & crate::state::MP_SEIZE_USED != 0 {
            out.push_str("seize: true\n");
        }
        if ps.al.favors > 0 {
            out.push_str(&format!("favors: {}\n", ps.al.favors));
        }
        if !ps.al.keep.is_empty() {
            out.push_str(&format!("keep: {}\n", format_counts(&ps.al.keep.counts())));
        }
        for (key, v) in [("skips", ps.al.skips), ("highwaymen", ps.al.hw), ("warlords", ps.al.wl), ("garrison tokens", ps.al.tokens)] {
            if v > 0 {
                out.push_str(&format!("{key}: {v}\n"));
            }
        }
        if !ps.farm_cards.is_empty() {
            out.push_str(&format!("farmhands: {}\n", format_counts(&ps.farm_cards.counts())));
        }
        if !ps.summon_cards.is_empty() {
            out.push_str(&format!("summoned: {}\n", format_counts(&ps.summon_cards.counts())));
        }
        if !ps.plu.z.is_empty() {
            out.push_str(&format!("plunder aside: {}\n", format_plu_aside(&ps.plu)));
        }
        if ps.plu.wait_len > 0 {
            out.push_str(&format!("waiting: {}\n", format_plu_waiting(&ps.plu)));
        }
        if ps.plu.holders != 0 || ps.plu.qm != 0 || ps.plu.frigate != 0 {
            out.push_str(&format!("plunder: holders {}; qm {}; frigate {}\n", ps.plu.holders, ps.plu.qm, ps.plu.frigate));
        }
        if !ps.church.is_empty() {
            let batches: Vec<String> = (0..ps.church.len())
                .map(|i| {
                    let (cards, n) = ps.church.batch(i);
                    if n == 0 { "-".to_string() } else { format_counts(&counts_of(&cards[..n])) }
                })
                .collect();
            out.push_str(&format!("church: {}\n", batches.join("; ")));
        }
        if !ps.cleanup_aside.is_empty() {
            out.push_str(&format!("cleanup aside: {}\n", format_counts(&ps.cleanup_aside.counts())));
        }
        if !ps.discard_next_cleanup.is_empty() {
            let mut c = Counts::EMPTY;
            for (card, n) in ps.discard_next_cleanup.iter() {
                c.set(card, n);
            }
            out.push_str(&format!("discard at next cleanup: {}
", format_counts(&c)));
        }
        // Start-of-turn effects still waiting to resolve (mid-`StartOrder`) are listed ahead of
        // the effects scheduled for the next turn; Clerk and Prince need no entry (they are
        // rebuilt from the hand and the `prince:` line). On load they all start the turn again.
        let waiting: &[PendingDuration] = if p == state.turn.player as usize {
            state.turn.start_pending.as_slice()
        } else {
            &[]
        };
        let mut list: Vec<PendingDuration> =
            waiting.iter().copied().filter(|e| e.card != cards::id::CLERK && e.card != cards::id::PRINCE).collect();
        list.extend_from_slice(&ps.pending_durations[..ps.pending_durations_len as usize]);
        if !list.is_empty() {
            out.push_str(&format!("durations: {}\n", format_pending_durations(&list)));
        }
        if !ps.prince_cards.is_empty() {
            out.push_str(&format!("prince: {}\n", format_counts(&ps.prince_cards.counts())));
        }
        if !ps.stays.is_empty() {
            out.push_str(&format!("prince stays: {}\n", format_counts(&ps.stays.counts())));
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
        if ps.shielded != 0 {
            let who: Vec<String> = (0..n).filter(|q| ps.shielded & (1 << q) != 0).map(|q| (q + 1).to_string()).collect();
            out.push_str(&format!("shielded: {}\n", who.join(" ")));
        }
        // Adventures: the Tavern mat, the tokens, the Estate token, Save's card and the Events bought.
        if !ps.tavern.is_empty() {
            out.push_str(&format!("tavern: {}\n", format_counts(&ps.tavern.counts())));
        }
        let mut toks: Vec<String> = Vec::new();
        for (i, &pile) in ps.tok.iter().enumerate() {
            if pile != 0 {
                toks.push(format!("{}={}", cards::TOKEN_NAMES[i], cards::name(pile)));
            }
        }
        if ps.adv & ADV_MINUS_CARD != 0 {
            toks.push(cards::TOKEN_NAMES[7].to_string());
        }
        if ps.adv & ADV_MINUS_COIN != 0 {
            toks.push(cards::TOKEN_NAMES[8].to_string());
        }
        if ps.adv & ADV_JOURNEY_DOWN != 0 {
            toks.push(format!("{}=down", cards::TOKEN_NAMES[9]));
        }
        if !toks.is_empty() {
            out.push_str(&format!("tokens: {}\n", toks.join(", ")));
        }
        if ps.inherit != 0 {
            out.push_str(&format!("inherit: {}\n", cards::name(ps.inherit)));
        }
        if ps.saved != 0 {
            out.push_str(&format!("saved: {}\n", cards::name(ps.saved)));
        }
        // Empires: Debt, the Archives' cards, the Enchantresses that affect the player, Donate.
        if ps.debt > 0 {
            out.push_str(&format!("debt: {}\n", ps.debt));
        }
        if !ps.archive.is_empty() {
            out.push_str(&format!("archive: {}\n", format_counts(&ps.archive.counts())));
        }
        if ps.ench_by != 0 {
            let who: Vec<String> = (0..n).filter(|q| ps.ench_by & (1 << q) != 0).map(|q| (q + 1).to_string()).collect();
            out.push_str(&format!("enchanted by: {}\n", who.join(" ")));
        }
        if ps.emp & crate::state::P_DONATE != 0 {
            out.push_str("donate: true\n");
        }
        // Nocturne: the States, Guardian's protection, the Hounds set aside, the Boons Blessed Villages took.
        if ps.noc != 0 {
            let mut st: Vec<&str> = Vec::new();
            if ps.noc & crate::state::P_DELUDED != 0 {
                st.push("Deluded");
            }
            if ps.noc & crate::state::P_ENVIOUS != 0 {
                st.push("Envious");
            }
            if ps.noc & crate::state::P_TWICE_MISERABLE != 0 {
                st.push("Twice Miserable");
            } else if ps.noc & crate::state::P_MISERABLE != 0 {
                st.push("Miserable");
            }
            if !st.is_empty() {
                out.push_str(&format!("states: {}\n", st.join(", ")));
            }
            if ps.noc & crate::state::P_GUARDED != 0 {
                out.push_str("guarded: true\n");
            }
        }
        if ps.hounds > 0 {
            out.push_str(&format!("hounds: {}\n", ps.hounds));
        }
        if ps.boons_later != 0 {
            out.push_str(&format!("boons later: {}\n", boon_names(ps.boons_later, cards::FIRST_BOON)));
        }
        // Rising Sun: the hand Biding Time set aside, Rapid Expansion's and Foresight's cards, a Gold gained (Kintsugi).
        if !ps.aside.is_empty() {
            let key = if state.rs.prophecy == cards::id::BIDING_TIME { "biding" } else { "rapid" };
            out.push_str(&format!("{key}: {}\n", format_counts(&ps.aside.counts())));
        }
        if !ps.foresight.is_empty() {
            out.push_str(&format!("foresight: {}\n", format_counts(&ps.foresight.counts())));
        }
        if ps.rs & crate::state::P_RS_GOLD != 0 {
            out.push_str("rs gold: true\n");
        }
        if !ps.events_bought.is_empty() {
            let events: Vec<&str> = ps.events_bought.events().map(cards::name).collect();
            out.push_str(&format!("events bought: {}\n", events.join(", ")));
        }
    }

    out
}

/// The names of the Boons (or Hexes, with `first` the first of them) in the bit mask `mask`.
fn boon_names(mask: u16, first: CardId) -> String {
    (0..12u16).filter(|i| mask & (1 << i) != 0).map(|i| cards::name(first + i)).collect::<Vec<_>>().join(", ")
}

/// Parse a list of Boons (or Hexes) into a bit mask.
fn parse_boon_mask(s: &str, first: CardId, ln: usize) -> Result<u16, String> {
    let mut mask = 0u16;
    for name in split_parts(s) {
        let c = cards::by_name(name).filter(|&c| c >= first && c < first + 12).ok_or_else(|| format!("line {ln}: '{name}' is not one of the expected cards"))?;
        mask |= 1 << (c - first);
    }
    Ok(mask)
}

/// Whether a pile outside the Supply is listed even when empty: the game has a card that brings it.
fn nocturne_pile_wanted(state: &GameState, pile: CardId) -> bool {
    use cards::id;
    let has = |c: CardId| state.in_supply(c);
    let fate = state.noc.flags & crate::state::N_BOONS != 0;
    match pile {
        id::WILL_O_WISP => fate || has(id::EXORCIST),
        id::WISH => has(id::LEPRECHAUN) || has(id::SECRET_CAVE),
        id::BAT => has(id::VAMPIRE),
        id::IMP => has(id::DEVILS_WORKSHOP) || has(id::TORMENTOR) || has(id::EXORCIST),
        id::GHOST => has(id::CEMETERY) || has(id::EXORCIST),
        _ => false,
    }
}

/// Nocturne: the Boons and Hexes decks and Lost in the Woods (see the module docs).
fn format_nocturne(state: &GameState, out: &mut String) {
    let n = &state.noc;
    if n.flags & crate::state::N_BOONS != 0 {
        out.push_str(&format!("boon deck: {}\n", boon_names(n.boon_deck, cards::FIRST_BOON)));
        out.push_str(&format!("boon discard: {}\n", boon_names(n.boon_discard, cards::FIRST_BOON)));
        if n.boon_druid != 0 {
            out.push_str(&format!("druid boons: {}\n", boon_names(n.boon_druid, cards::FIRST_BOON)));
        }
        if n.boon_held != 0 {
            out.push_str(&format!("boons held: {}\n", boon_names(n.boon_held, cards::FIRST_BOON)));
        }
    }
    if n.flags & crate::state::N_HEXES != 0 {
        out.push_str(&format!("hex deck: {}\n", boon_names(n.hex_deck, cards::FIRST_HEX)));
        out.push_str(&format!("hex discard: {}\n", boon_names(n.hex_discard, cards::FIRST_HEX)));
    }
    if n.lost != 0 {
        out.push_str(&format!("lost in the woods: {}\n", n.lost));
    }
}

/// Empires: the Landmarks and the tokens (see the module docs).
fn format_empires(state: &GameState, out: &mut String) {
    let e = &state.emp;
    if e.landmarks != 0 {
        let names: Vec<&str> = (0..cards::NUM_LANDMARKS as CardId)
            .filter(|i| e.landmarks & (1 << i) != 0)
            .map(|i| cards::name(cards::FIRST_LANDMARK + i))
            .collect();
        out.push_str(&format!("landmarks: {}\n", names.join(", ")));
        let vp: Vec<String> = (0..cards::NUM_LANDMARKS)
            .filter(|&i| e.landmarks & (1 << i) != 0 && e.lm_vp[i] > 0)
            .map(|i| format!("{}={}", cards::name(cards::FIRST_LANDMARK + i as CardId), e.lm_vp[i]))
            .collect();
        out.push_str(&format!("landmark vp: {}\n", vp.join(", ")));
        if e.obelisk != 0 {
            out.push_str(&format!("obelisk: {}\n", cards::name(e.obelisk)));
        }
        if e.pass_done {
            out.push_str("pass: done\n");
        }
    }
    let on_piles = e.landmarks != 0 || e.flags & crate::state::E_TAX != 0 || e.piles.iter().next().is_some();
    if on_piles {
        // In card order (the order the tokens were put on the piles is no part of the state).
        let mut piles: Vec<(CardId, u8, u8)> = e.piles.iter().collect();
        piles.sort_by_key(|&(c, _, _)| c);
        let vp: Vec<String> = piles.iter().filter(|&&(_, v, _)| v > 0).map(|&(c, v, _)| format!("{}={}", cards::name(c), v)).collect();
        out.push_str(&format!("pile vp: {}\n", vp.join(", ")));
        let debt: Vec<String> = piles.iter().filter(|&&(_, _, d)| d > 0).map(|&(c, _, d)| format!("{}={}", cards::name(c), d)).collect();
        out.push_str(&format!("pile debt: {}\n", debt.join(", ")));
    }
}

/// `Name=N, Name=N` pairs (token counts).
fn parse_pairs(s: &str, ln: usize) -> Result<Vec<(String, u8)>, String> {
    let mut out = Vec::new();
    for pair in split_parts(s) {
        let (name, n) = pair.split_once('=').ok_or_else(|| format!("line {ln}: expected 'Name=Count', found '{pair}'"))?;
        let n: u8 = n.trim().parse().map_err(|_| format!("line {ln}: invalid count in '{pair}'"))?;
        out.push((name.trim().to_string(), n));
    }
    Ok(out)
}

/// Format the pending Duration effects list: `"Fishing Village x2, Haven (Silver), Sailor (used)"`.
/// A Sailor with plays left over shows how many it has spent: `Sailor x2 (used 1)`.
fn format_pending_durations(list: &[PendingDuration]) -> String {
    list.iter()
        .map(|e| {
            let mut s = cards::name(e.card).to_string();
            if e.times != 1 {
                s.push_str(&format!(" x{}", e.times));
            }
            if e.arg != 0 {
                s.push_str(&format!(" ({})", cards::name(e.arg)));
            } else if e.used >= e.times.max(1) {
                s.push_str(" (used)");
            } else if e.used > 0 {
                s.push_str(&format!(" (used {})", e.used));
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
        let mut used = 0u8;
        if let Some(open) = rest.find('(') {
            let inner = rest[open + 1..].trim_end().trim_end_matches(')').trim();
            if inner.eq_ignore_ascii_case("used") {
                used = u8::MAX; // every play spent (clamped to `times` below); the older format
            } else if let Some(n) = inner.strip_prefix("used ").and_then(|n| n.trim().parse::<u8>().ok()) {
                used = n;
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
        out.push(PendingDuration { card, times, arg, used: used.min(times) });
    }
    Ok(out)
}

/// Parse a full `GameState` from text produced by (or compatible with) `format_state`.
/// Errors are prefixed with `"line N: "` where possible.
pub fn parse_state(text: &str) -> Result<GameState, String> {
    let lines: Vec<&str> = text.lines().collect();

    let mut players_val: Option<(usize, String)> = None;
    let mut kingdom_val: Option<(usize, String)> = None;
    let mut events_val: Option<(usize, String)> = None;
    let mut landmarks_val: Option<(usize, String)> = None;
    let mut landmark_vp_val: Option<(usize, String)> = None;
    let mut pile_vp_val: Option<(usize, String)> = None;
    let mut pile_debt_val: Option<(usize, String)> = None;
    let mut obelisk_val: Option<(usize, String)> = None;
    let mut prophecy_val: Option<(usize, String)> = None;
    let mut projects_val: Option<(usize, String)> = None;
    let mut artifacts_val: Option<(usize, String)> = None;
    let mut fleet_val: Option<(usize, String)> = None;
    let mut suns_val: Option<(usize, String)> = None;
    let mut riverboat_val: Option<(usize, String)> = None;
    let mut mouse_val: Option<(usize, String)> = None;
    let mut ways_val: Option<(usize, String)> = None;
    let mut men_turn_val: Option<(usize, String)> = None;
    let mut ally_val: Option<(usize, String)> = None;
    let mut fam_val: Option<(usize, String)> = None;
    let mut al_turn_val: Option<(usize, String)> = None;
    let mut traits_val: Option<(usize, String)> = None;
    let mut plu_turn_val: Option<(usize, String)> = None;
    let mut pile_order_vals: Vec<(usize, String)> = Vec::new();
    let mut pass_done = false;
    let mut supply_val: Option<(usize, String)> = None;
    let mut ruins_val: Option<(usize, String)> = None;
    let mut knights_val: Option<(usize, String)> = None;
    let mut trash_val: Option<(usize, String)> = None;
    let mut held_val: Option<(usize, String)> = None;
    let mut bane_val: Option<(usize, String)> = None;
    let mut ferryman_val: Option<(usize, String)> = None;
    let mut side_val: Option<(usize, String)> = None;
    let mut seed_val: Option<(usize, String)> = None;
    let mut noc_vals: Vec<(usize, String, String)> = Vec::new();
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
            "events" => events_val = Some((lineno, rest.to_string())),
            "landmarks" => landmarks_val = Some((lineno, rest.to_string())),
            "landmark vp" => landmark_vp_val = Some((lineno, rest.to_string())),
            "pile vp" => pile_vp_val = Some((lineno, rest.to_string())),
            "pile debt" => pile_debt_val = Some((lineno, rest.to_string())),
            "obelisk" => obelisk_val = Some((lineno, rest.to_string())),
            "prophecy" => prophecy_val = Some((lineno, rest.to_string())),
            "projects" => projects_val = Some((lineno, rest.to_string())),
            "artifacts" => artifacts_val = Some((lineno, rest.to_string())),
            "fleet round" => fleet_val = Some((lineno, rest.to_string())),
            "suns" => suns_val = Some((lineno, rest.to_string())),
            "riverboat" => riverboat_val = Some((lineno, rest.to_string())),
            "mouse" => mouse_val = Some((lineno, rest.to_string())),
            "ways" => ways_val = Some((lineno, rest.to_string())),
            "men turn" => men_turn_val = Some((lineno, rest.to_string())),
            "ally" => ally_val = Some((lineno, rest.to_string())),
            "fam" => fam_val = Some((lineno, rest.to_string())),
            "al turn" => al_turn_val = Some((lineno, rest.to_string())),
            "traits" => traits_val = Some((lineno, rest.to_string())),
            "plu turn" => plu_turn_val = Some((lineno, rest.to_string())),
            "pile order" => pile_order_vals.push((lineno, rest.to_string())),
            "pass" => pass_done = rest.eq_ignore_ascii_case("done"),
            "supply" => supply_val = Some((lineno, rest.to_string())),
            "ruins pile" => ruins_val = Some((lineno, rest.to_string())),
            "knights pile" => knights_val = Some((lineno, rest.to_string())),
            "trash" => trash_val = Some((lineno, rest.to_string())),
            "held" => held_val = Some((lineno, rest.to_string())),
            "bane" => bane_val = Some((lineno, rest.to_string())),
            "ferryman" => ferryman_val = Some((lineno, rest.to_string())),
            "side piles" => side_val = Some((lineno, rest.to_string())),
            "seed" => seed_val = Some((lineno, rest.to_string())),
            "max_turns" | "max turns" => max_turns_val = Some((lineno, rest.to_string())),
            "turn" => turn_fields = Some((lineno, parse_turn_line(line, lineno)?)),
            "boon deck" | "boon discard" | "druid boons" | "boons held" | "hex deck" | "hex discard" | "lost in the woods" => {
                noc_vals.push((lineno, key.clone(), rest.to_string()))
            }
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
    let mut kingdom = parse_kingdom_list(&kingdom_str).map_err(|e| format!("line {kingdom_line}: {e}"))?;
    if let Some((ln, s)) = &events_val {
        for e in parse_kingdom_list(s).map_err(|e| format!("line {ln}: {e}"))? {
            if !cards::is_event(e) {
                return Err(format!("line {ln}: '{}' is not an Event", cards::name(e)));
            }
            kingdom.push(e);
        }
    }
    // Plunder: each Trait with the pile it sits on.
    let mut trait_piles: Vec<(CardId, CardId)> = Vec::new();
    if let Some((ln, s)) = &traits_val {
        for part in split_parts(s) {
            let (t, pile) = part.split_once(" on ").ok_or_else(|| format!("line {ln}: expected 'Trait on Card', found '{part}'"))?;
            let t = cards::by_name(t.trim()).filter(|&t| cards::is_trait(t)).ok_or_else(|| format!("line {ln}: '{}' is not a Trait", t.trim()))?;
            let pile = cards::by_name(pile.trim()).ok_or_else(|| format!("line {ln}: unknown card '{}'", pile.trim()))?;
            kingdom.push(t);
            trait_piles.push((t, pile));
        }
    }
    if let Some((ln, s)) = &landmarks_val {
        for l in parse_kingdom_list(s).map_err(|e| format!("line {ln}: {e}"))? {
            if !cards::is_landmark(l) {
                return Err(format!("line {ln}: '{}' is not a Landmark", cards::name(l)));
            }
            kingdom.push(l);
        }
    }
    if let Some((ln, s)) = &projects_val {
        for pr in parse_kingdom_list(s).map_err(|e| format!("line {ln}: {e}"))? {
            if !cards::is_project(pr) {
                return Err(format!("line {ln}: '{}' is not a Project", cards::name(pr)));
            }
            kingdom.push(pr);
        }
    }
    if let Some((ln, s)) = &ways_val {
        for w in parse_kingdom_list(s).map_err(|e| format!("line {ln}: {e}"))? {
            if !cards::is_way(w) {
                return Err(format!("line {ln}: '{}' is not a Way", cards::name(w)));
            }
            kingdom.push(w);
        }
    }
    if let Some((ln, s)) = &prophecy_val {
        for p in parse_kingdom_list(s).map_err(|e| format!("line {ln}: {e}"))? {
            if !cards::is_prophecy(p) {
                return Err(format!("line {ln}: '{}' is not a Prophecy", cards::name(p)));
            }
            kingdom.push(p);
        }
    }
    if let Some((ln, s)) = &ally_val {
        for a in parse_kingdom_list(s).map_err(|e| format!("line {ln}: {e}"))? {
            if !cards::is_ally(a) {
                return Err(format!("line {ln}: '{}' is not an Ally", cards::name(a)));
            }
            kingdom.push(a);
        }
    }

    let seed: u64 = match &seed_val {
        Some((ln, s)) => s.trim().parse().map_err(|_| format!("line {ln}: invalid seed '{s}'"))?,
        None => 0,
    };
    let max_turns: u16 = match &max_turns_val {
        Some((ln, s)) => s.trim().parse().map_err(|_| format!("line {ln}: invalid max_turns '{s}'"))?,
        None => GameConfig::default().max_turns,
    };

    let named = |v: &Option<(usize, String)>, what: &str| -> Result<Option<CardId>, String> {
        match v {
            Some((ln, s)) => {
                let c = cards::by_name(s.trim()).ok_or_else(|| format!("line {ln}: unknown card '{s}' for {what}"))?;
                Ok(Some(c))
            }
            None => Ok(None),
        }
    };
    let cfg = GameConfig {
        num_players, kingdom, seed, max_turns,
        bane: named(&bane_val, "bane")?,
        ferryman: named(&ferryman_val, "ferryman")?,
        obelisk: named(&obelisk_val, "obelisk")?,
        riverboat: named(&riverboat_val, "riverboat")?,
        mouse: named(&mouse_val, "mouse")?,
        // The decks are read from the text; `shelters` is derived from them below.
        shelters: Some(false),
        trait_piles,
    };
    let (riverboat, bane, ferryman) = (cfg.riverboat, cfg.bane, cfg.ferryman);
    let mut state = GameState::new(&cfg);
    // Riverboat's card, the Bane and Ferryman's pile are read as they are (the cards that brought them may be gone: Divine Wind).
    if let Some(c) = riverboat {
        state.rs.riverboat = c;
    }
    if let Some(b) = bane {
        state.bane = b;
    }
    if let Some(f) = ferryman {
        state.ferryman = f;
    }
    state.stack = FrameStack::default();
    state.pending = Pending::None;
    for p in 0..num_players {
        state.players[p] = PlayerState::default();
    }

    if let Some((ln, s)) = &supply_val {
        // The piles `new` dealt are replaced by what the text says: only the listed top card of a mixed pile has cards.
        for pile in [&crate::sets::dark_ages::KNIGHTS_PILE, &crate::sets::dark_ages::RUINS_PILE] {
            for m in pile.members() {
                state.supply.set(m, 0);
            }
        }
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

    // Empires: the tokens as the text says them (a line that is present replaces what a new game starts with).
    if let Some((ln, s)) = &landmark_vp_val {
        state.emp.lm_vp = [0; cards::NUM_LANDMARKS];
        for (name, n) in parse_pairs(s, *ln)? {
            let l = cards::by_name(&name).filter(|&l| cards::is_landmark(l)).ok_or_else(|| format!("line {ln}: '{name}' is not a Landmark"))?;
            state.emp.lm_vp[crate::sets::empires::lm_index(l)] = n;
        }
    }
    if let Some((ln, s)) = &pile_vp_val {
        state.emp.piles.clear_vp();
        for (name, n) in parse_pairs(s, *ln)? {
            let c = cards::by_name(&name).ok_or_else(|| format!("line {ln}: unknown card '{name}'"))?;
            state.emp.piles.add_vp(c, n);
        }
    }
    if let Some((ln, s)) = &pile_debt_val {
        state.emp.piles.clear_debt();
        for (name, n) in parse_pairs(s, *ln)? {
            let c = cards::by_name(&name).ok_or_else(|| format!("line {ln}: unknown card '{name}'"))?;
            state.emp.piles.add_debt(c, n);
        }
    }
    state.emp.pass_done = pass_done;
    // Rising Sun: the Sun tokens left (none: the Prophecy has happened).
    if let Some((ln, s)) = &suns_val {
        let n: u8 = s.trim().parse().map_err(|_| format!("line {ln}: invalid suns '{s}'"))?;
        if state.rs.prophecy == 0 {
            return Err(format!("line {ln}: suns without a prophecy"));
        }
        state.rs.suns = n;
        state.rs.active = if n == 0 { state.rs.prophecy } else { 0 };
        state.rs_update_hooks();
    }

    // Renaissance: the Artifacts' holders (seats, 1-based) and the Fleet round.
    if let Some((ln, s)) = &artifacts_val {
        for (name, n) in parse_pairs(s, *ln)? {
            let a = cards::by_name(&name).filter(|&a| cards::is_artifact(a)).ok_or_else(|| format!("line {ln}: '{name}' is not an Artifact"))?;
            if n == 0 || n as usize > num_players {
                return Err(format!("line {ln}: the {name} is held by seat {n}, but there are {num_players} players"));
            }
            state.ren.holder[(a - cards::FIRST_ARTIFACT) as usize] = n;
        }
    }
    if let Some((ln, s)) = &fleet_val {
        state.ren.flags |= crate::state::REN_FLEET_ROUND;
        for w in s.split_whitespace() {
            match w.parse::<u8>() {
                Ok(q) if (1..=num_players as u8).contains(&q) => state.ren.fleet |= 1 << (q - 1),
                _ => return Err(format!("line {ln}: invalid seat '{w}' in fleet round")),
            }
        }
    }

    if let Some((ln, s)) = &side_val {
        for pair in split_parts(s) {
            let (name_part, count_part) =
                pair.split_once('=').ok_or_else(|| format!("line {ln}: expected 'Card=Count' in side piles, found '{pair}'"))?;
            let id = cards::by_name(name_part.trim()).ok_or_else(|| format!("line {ln}: unknown card '{}'", name_part.trim()))?;
            let count: u8 = count_part.trim().parse().map_err(|_| format!("line {ln}: invalid count in '{pair}'"))?;
            if id >= cards::FIRST_TRAV && ((id - cards::FIRST_TRAV) as usize) < cards::NUM_TRAV {
                state.trav[(id - cards::FIRST_TRAV) as usize] = count;
            } else if cards::is_reward(id) {
                state.rewards[(id - cards::FIRST_REWARD) as usize] = count;
            } else if id == state.ferryman && id != 0 {
                state.ferryman_left = count;
            } else if id == cards::id::SPOILS {
                state.spoils = count;
            } else if id == cards::id::MADMAN {
                state.madman = count;
            } else if id == cards::id::MERCENARY {
                state.mercenary = count;
            } else if id == crate::sets::menagerie::ids::HORSE || cards::is_loot(id) {
                state.supply.set(id, count);
            } else if let Some(i) =
                [cards::id::WILL_O_WISP, cards::id::WISH, cards::id::BAT, cards::id::IMP, cards::id::GHOST].iter().position(|&c| c == id)
            {
                state.noc.piles[i] = count;
            } else {
                return Err(format!("line {ln}: '{}' is not a pile outside the Supply in this game", name_part.trim()));
            }
        }
    }

    // Nocturne: the decks as the text says them (a line that is present replaces what a new game starts with).
    for (ln, key, val) in &noc_vals {
        match key.as_str() {
            // (The decks are in the game even if the Fate or Doom card that brought them is not any more: Divine Wind.)
            "boon deck" => {
                state.noc.flags |= crate::state::N_BOONS;
                state.noc.boon_deck = parse_boon_mask(val, cards::FIRST_BOON, *ln)?
            }
            "boon discard" => state.noc.boon_discard = parse_boon_mask(val, cards::FIRST_BOON, *ln)?,
            "druid boons" => state.noc.boon_druid = parse_boon_mask(val, cards::FIRST_BOON, *ln)?,
            "boons held" => state.noc.boon_held = parse_boon_mask(val, cards::FIRST_BOON, *ln)?,
            "hex deck" => {
                state.noc.flags |= crate::state::N_HEXES;
                state.noc.hex_deck = parse_boon_mask(val, cards::FIRST_HEX, *ln)?
            }
            "hex discard" => state.noc.hex_discard = parse_boon_mask(val, cards::FIRST_HEX, *ln)?,
            _ => {
                let q: u8 = val.trim().parse().map_err(|_| format!("line {ln}: invalid player '{val}'"))?;
                if q as usize > num_players {
                    return Err(format!("line {ln}: player {q} out of range"));
                }
                state.noc.lost = q;
            }
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
        named_for_war_chest: SmallMultiset::EMPTY,
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
        // Transient (the start-of-turn effects are rebuilt when the turn starts, see
        // `push_turn_start_frames`).
        start_pending: StartList::EMPTY,
        led: crate::track::Ledger::EMPTY,
        used_up: SmallMultiset::EMPTY,
        moated: 0,
        extra_draw: tf.extra_draw,
        guilds: tf.guilds,
        gained_in_buy: tf.gained_in_buy,
        adv: tf.adv,
        emp: tf.emp,
        noc: tf.noc,
        // Per-turn bookkeeping, reset on load (the Cauldrons in play are counted below).
        hint: Default::default(),
        rs: tf.rs,
        ren: tf.ren,
        men: match &men_turn_val {
            Some((ln, s)) => parse_men_turn(s).map_err(|e| format!("line {ln}: {e}"))?,
            None => MenTurn::default(),
        },
        al: match &al_turn_val {
            Some((ln, s)) => parse_al_turn(s).map_err(|e| format!("line {ln}: {e}"))?,
            None => AlTurn::default(),
        },
        plu: match &plu_turn_val {
            Some((ln, s)) => parse_plu_turn(s).map_err(|e| format!("line {ln}: {e}"))?,
            None => Default::default(),
        },
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
        let mut plot_tokens = 0u8;
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
                "coffers" => ps.coffers = val.parse().map_err(|_| format!("line {ln2}: invalid coffers '{val}'"))?,
                "villagers" => ps.ren.villagers = val.parse().map_err(|_| format!("line {ln2}: invalid villagers '{val}'"))?,
                "owned projects" => {
                    for pr in parse_kingdom_list(val).map_err(|e| format!("line {ln2}: {e}"))? {
                        if !cards::is_project(pr) {
                            return Err(format!("line {ln2}: '{}' is not a Project", cards::name(pr)));
                        }
                        ps.ren.projects |= 1u32 << (pr - cards::FIRST_PROJECT);
                    }
                }
                "plot" => plot_tokens = val.parse().map_err(|_| format!("line {ln2}: invalid plot '{val}'"))?,
                "cargo" => ps.ren.cargo = parse_small_multiset::<{ crate::state::CARGO_CAP }>(val, "cargo").map_err(|e| format!("line {ln2}: {e}"))?,
                "research" => ps.ren.research = parse_small_multiset::<{ crate::state::RESEARCH_CAP }>(val, "research").map_err(|e| format!("line {ln2}: {e}"))?,
                "exile" => ps.men.exile = parse_small_multiset::<{ crate::state::EXILE_CAP }>(val, "exile").map_err(|e| format!("line {ln2}: {e}"))?,
                "invested" => ps.men.invest = parse_small_multiset::<{ crate::state::INVEST_CAP }>(val, "invested").map_err(|e| format!("line {ln2}: {e}"))?,
                "delayed" => ps.men.aside = parse_small_multiset::<{ crate::state::MEN_ASIDE_CAP }>(val, "delayed").map_err(|e| format!("line {ln2}: {e}"))?,
                "trashed last" => ps.men.trashed_last = val.parse().map_err(|_| format!("line {ln2}: invalid trashed last '{val}'"))?,
                "gatekeepers" => ps.men.gk = val.parse().map_err(|_| format!("line {ln2}: invalid gatekeepers '{val}'"))?,
                "seize" => {
                    if val.eq_ignore_ascii_case("true") {
                        ps.men.flags |= crate::state::MP_SEIZE_USED;
                    }
                }
                "favors" => ps.al.favors = val.parse().map_err(|_| format!("line {ln2}: invalid favors '{val}'"))?,
                "keep" => ps.al.keep = parse_small_multiset::<{ crate::state::AL_KEEP_CAP }>(val, "keep").map_err(|e| format!("line {ln2}: {e}"))?,
                "skips" => ps.al.skips = val.parse().map_err(|_| format!("line {ln2}: invalid skips '{val}'"))?,
                "highwaymen" => ps.al.hw = val.parse().map_err(|_| format!("line {ln2}: invalid highwaymen '{val}'"))?,
                "warlords" => ps.al.wl = val.parse().map_err(|_| format!("line {ln2}: invalid warlords '{val}'"))?,
                "garrison tokens" => ps.al.tokens = val.parse().map_err(|_| format!("line {ln2}: invalid garrison tokens '{val}'"))?,
                "farmhands" => ps.farm_cards = parse_small_multiset::<FARM_CAP>(val, "farmhands").map_err(|e| format!("line {ln2}: {e}"))?,
                "summoned" => ps.summon_cards = parse_small_multiset::<SUMMON_CAP>(val, "summoned").map_err(|e| format!("line {ln2}: {e}"))?,
                "church" => ps.church = parse_church(val).map_err(|e| format!("line {ln2}: {e}"))?,
                "plunder aside" => parse_plu_aside(val, &mut ps.plu).map_err(|e| format!("line {ln2}: {e}"))?,
                "waiting" => parse_plu_waiting(val, &mut ps.plu).map_err(|e| format!("line {ln2}: {e}"))?,
                "plunder" => {
                    for part in val.split(';') {
                        let part = part.trim();
                        if part.is_empty() {
                            continue;
                        }
                        let (k, v) = part.rsplit_once(' ').ok_or_else(|| format!("line {ln2}: malformed plunder entry '{part}'"))?;
                        let v: u8 = v.parse().map_err(|_| format!("line {ln2}: invalid number in '{part}'"))?;
                        match k {
                            "holders" => ps.plu.holders = v,
                            "qm" => ps.plu.qm = v,
                            "frigate" => ps.plu.frigate = v,
                            other => return Err(format!("line {ln2}: unknown plunder field '{other}'")),
                        }
                    }
                }
                "cleanup aside" => ps.cleanup_aside = parse_small_multiset::<2>(val, "cleanup aside").map_err(|e| format!("line {ln2}: {e}"))?,
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
                "tavern" => ps.tavern = parse_small_multiset::<TAVERN_CAP>(val, "tavern").map_err(|e| format!("line {ln2}: {e}"))?,
                "tokens" => {
                    for part in split_parts(val) {
                        let (name, pile) = match part.split_once('=') {
                            Some((n, p)) => (n.trim(), Some(p.trim())),
                            None => (part, None),
                        };
                        let tok = cards::token_by_name(name).ok_or_else(|| format!("line {ln2}: unknown token '{name}'"))?;
                        match (tok, pile) {
                            (t, Some(pile)) if t < NUM_PILE_TOKENS => {
                                ps.tok[t] = cards::by_name(pile).ok_or_else(|| format!("line {ln2}: unknown card '{pile}' for the {name} token"))?;
                            }
                            (7, None) => ps.adv |= ADV_MINUS_CARD,
                            (8, None) => ps.adv |= ADV_MINUS_COIN,
                            (9, Some(side)) if side.eq_ignore_ascii_case("down") => ps.adv |= ADV_JOURNEY_DOWN,
                            (9, Some(side)) if side.eq_ignore_ascii_case("up") => {}
                            _ => return Err(format!("line {ln2}: cannot read the token entry '{part}'")),
                        }
                    }
                }
                "inherit" => ps.inherit = cards::by_name(val).ok_or_else(|| format!("line {ln2}: unknown card '{val}'"))?,
                "saved" => ps.saved = cards::by_name(val).ok_or_else(|| format!("line {ln2}: unknown card '{val}'"))?,
                "biding" | "rapid" => ps.aside = parse_small_multiset(val, "biding").map_err(|e| format!("line {ln2}: {e}"))?,
                "foresight" => ps.foresight = parse_small_multiset(val, "foresight").map_err(|e| format!("line {ln2}: {e}"))?,
                "rs gold" => {
                    if val.eq_ignore_ascii_case("true") {
                        ps.rs |= crate::state::P_RS_GOLD;
                    }
                }
                "events bought" => {
                    for e in parse_kingdom_list(val).map_err(|e| format!("line {ln2}: {e}"))? {
                        if !cards::is_event(e) {
                            return Err(format!("line {ln2}: '{}' is not an Event", cards::name(e)));
                        }
                        ps.events_bought.insert((e - cards::FIRST_EVENT) as u32);
                    }
                }
                "debt" => ps.debt = val.parse().map_err(|_| format!("line {ln2}: invalid debt '{val}'"))?,
                "states" => {
                    for name in split_parts(val) {
                        ps.noc |= match name.to_lowercase().as_str() {
                            "deluded" => crate::state::P_DELUDED,
                            "envious" => crate::state::P_ENVIOUS,
                            "miserable" => crate::state::P_MISERABLE,
                            "twice miserable" => crate::state::P_MISERABLE | crate::state::P_TWICE_MISERABLE,
                            other => return Err(format!("line {ln2}: unknown state '{other}'")),
                        };
                    }
                }
                "guarded" => {
                    if val.eq_ignore_ascii_case("true") {
                        ps.noc |= crate::state::P_GUARDED;
                    }
                }
                "hounds" => ps.hounds = val.parse().map_err(|_| format!("line {ln2}: invalid hounds '{val}'"))?,
                "boons later" => ps.boons_later = parse_boon_mask(val, cards::FIRST_BOON, ln2)?,
                "archive" => ps.archive = parse_small_multiset(val, "archive").map_err(|e| format!("line {ln2}: {e}"))?,
                "enchanted by" => {
                    for w in val.split_whitespace() {
                        match w.parse::<u8>() {
                            Ok(q) if (1..=crate::state::MAX_PLAYERS as u8).contains(&q) => ps.ench_by |= 1 << (q - 1),
                            _ => return Err(format!("line {ln2}: invalid player '{w}' in enchanted by")),
                        }
                    }
                }
                "donate" => {
                    if val.eq_ignore_ascii_case("true") {
                        ps.emp |= crate::state::P_DONATE;
                    }
                }
                "prince" => ps.prince_cards = parse_small_multiset::<PRINCE_CAP>(val, "prince").map_err(|e| format!("line {ln2}: {e}"))?,
                "stays" | "prince stays" => ps.stays = parse_small_multiset::<STAYS_CAP>(val, "stays").map_err(|e| format!("line {ln2}: {e}"))?,
                "native village" => ps.native_village_mat = parse_small_multiset(val, "native village").map_err(|e| format!("line {ln2}: {e}"))?,
                "island" => ps.island_mat = parse_small_multiset(val, "island").map_err(|e| format!("line {ln2}: {e}"))?,
                "last turn gains" => ps.last_turn_gains = parse_small_multiset(val, "last turn gains").map_err(|e| format!("line {ln2}: {e}"))?,
                "shielded" => {
                    for w in val.split_whitespace() {
                        match w.parse::<u8>() {
                            Ok(q) if (1..=crate::state::MAX_PLAYERS as u8).contains(&q) => ps.shielded |= 1 << (q - 1),
                            _ => return Err(format!("line {ln2}: invalid player '{w}' in shielded")),
                        }
                    }
                }
                other => return Err(format!("line {ln2}: unknown field '{other}' in player block")),
            }
            i += 1;
        }
        state.players[p] = ps;
        state.ren.plot[p] = plot_tokens;
    }
    if seen != num_players {
        return Err(format!("expected {num_players} '[player N]' blocks, found {seen}"));
    }

    // Renaissance: which Projects some player owns decides which hooks are live.
    state.ren_update_hooks();
    // Plunder: what is live follows the waiting Durations and the turn's bookkeeping just read.
    if state.plu.flags != 0 {
        state.plu.empty_seen = state.empty_piles().min(255) as u8;
        state.plu_refresh();
    }
    // Shelters were in the starting decks if any player holds one anywhere.
    state.shelters = state.players.iter().any(|ps| {
        let all = ps.all_cards();
        all.has(cards::id::HOVEL) || all.has(cards::id::NECROPOLIS) || all.has(cards::id::OVERGROWN_ESTATE)
    });
    for ps in state.players.iter_mut() {
        if ps.stays.is_empty() && !ps.prince_cards.is_empty() && ps.in_play.has(cards::id::PRINCE) {
            ps.stays.add(cards::id::PRINCE, 1);
        }
    }
    // Nocturne: a Night phase can follow a Buy phase if anyone owns a Night card (even when its pile is not in the Supply: Divine Wind).
    if state.players.iter().any(|ps| ps.all_cards().any_type(cards::NIGHT)) {
        state.noc.flags |= crate::state::N_NIGHT;
    }
    // Adventures: which token effects are live, and the -$1 token of the turn's player.
    for ps in state.players.iter() {
        if ps.tok[..TOK_COST].iter().any(|&t| t != 0) {
            state.adv |= G_PLAY_TOKENS;
        }
        if ps.tok[TOK_COST] != 0 {
            state.adv |= G_COST_TOKEN;
        }
        if ps.tok[TOK_TRASH] != 0 {
            state.adv |= G_TRASH_TOKEN;
        }
        if ps.in_play.has(cards::id::CHAMPION) {
            state.adv |= G_CHAMPION;
        }
    }
    if state.players[state.turn.player as usize].adv & ADV_MINUS_COIN != 0 {
        state.turn.adv.tax_live = true;
        state.turn.adv.tax_base = state.turn.coins;
    }
    if state.players.iter().any(|ps| ps.ench_by != 0) {
        state.emp.flags |= crate::state::E_ENCHANT;
    }
    // Dark Ages: the dealt order of the Ruins and Knights piles. A "ruins pile:" / "knights pile:" line (top first) decides the pile's cards;
    // without one (a save from before the order was kept, a hand-written state) the face-down part is dealt from the cards nobody has seen,
    // with the state's RNG.
    for (pile, val, key) in [
        (&crate::sets::dark_ages::RUINS_PILE, &ruins_val, "ruins pile"),
        (&crate::sets::dark_ages::KNIGHTS_PILE, &knights_val, "knights pile"),
    ] {
        match val {
            Some((ln, s)) => {
                if !state.has_pile(pile) {
                    return Err(format!("line {ln}: '{key}' but that pile is not in this game"));
                }
                let seq = parse_card_sequence(s).map_err(|e| format!("line {ln}: {e}"))?;
                if seq.is_empty() || seq.len() > pile.capacity() {
                    return Err(format!("line {ln}: '{key}' must list 1 to {} cards", pile.capacity()));
                }
                let mut held = [0u8; 10];
                for &c in &seq {
                    if !pile.members().contains(&c) {
                        return Err(format!("line {ln}: {} is not in the {key}", cards::name(c)));
                    }
                    held[(c - pile.first) as usize] += 1;
                    if held[(c - pile.first) as usize] > pile.copies {
                        return Err(format!("line {ln}: too many {} in the {key}", cards::name(c)));
                    }
                }
                for m in pile.members() {
                    state.supply.set(m, 0);
                }
                state.supply.set(seq[0], seq.len() as u8);
                let off = pile.offset as usize;
                state.pile_order[off..off + pile.capacity()].fill(0);
                for (d, &c) in seq.iter().enumerate() {
                    state.pile_order[off + seq.len() - 1 - d] = (c - pile.first) as u8;
                }
            }
            None => state.deal_face_down(pile),
        }
    }
    // Allies: the split piles' order. Without a "pile order:" line a pile is in the order its counts give (each card's copies together, cheapest on top).
    state.al_orders_from_counts();
    for (ln, s) in &pile_order_vals {
        let seq = parse_card_sequence(s).map_err(|e| format!("line {ln}: {e}"))?;
        let Some(&first) = seq.first() else { return Err(format!("line {ln}: an empty 'pile order'")) };
        let Some(i) = cards::allies_pile_index(first) else {
            return Err(format!("line {ln}: {} is not in an Allies split pile", cards::name(first)));
        };
        if seq.len() > 16 || seq.iter().any(|&c| cards::allies_pile_index(c) != Some(i)) {
            return Err(format!("line {ln}: 'pile order' must list up to 16 cards of one split pile"));
        }
        let mut held = [0u8; 4];
        for &c in &seq {
            held[(c - cards::ALLIES_SPLIT_FIRST) as usize % 4] += 1;
        }
        for k in 0..4 {
            let c = cards::ALLIES_SPLIT_FIRST + 4 * i as CardId + k as CardId;
            if held[k] != state.supply.get(c) {
                return Err(format!("line {ln}: 'pile order' has {} {}, the supply {}", held[k], cards::name(c), state.supply.get(c)));
            }
        }
        state.al.piles[i] = crate::state::PileOrder { order: 0, len: 0 };
        for &c in seq.iter().rev() {
            state.al.piles[i].order = (state.al.piles[i].order << 2) | ((c - cards::ALLIES_SPLIT_FIRST) % 4) as u32;
            state.al.piles[i].len += 1;
        }
    }
    if let Some((ln, s)) = &fam_val {
        state.al.fam = parse_small_multiset::<{ crate::state::AL_FAM_CAP }>(s, "fam").map_err(|e| format!("line {ln}: {e}"))?;
    }
    if state.turn.al.limited {
        state.al.hook |= crate::state::AH_LIMIT;
    }
    state.al_update_hook();
    // "Played this turn" isn't stored in the text; the current player's in-play cards are the
    // best reconstruction (a card played twice by Throne Room counts once).
    state.turn.played = state.players[state.turn.player as usize].in_play;
    state.turn.hint.cauldrons = state.turn.played.get(cards::id::CAULDRON);
    // "phase: setup" is a new game before the deal: the opening hands are drawn on advancing.
    if state.turn.phase == Phase::Setup {
        if (0..num_players).any(|p| !state.players[p].hand.is_empty()) {
            return Err("phase: setup is before the deal, so every hand must be empty".into());
        }
        state.push_opening_deal();
    }
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

    #[test]
    fn a_card_name_never_runs_across_a_comma() {
        // "Gold, Mine" is a Gold and a Mine; "Gold Mine" (no comma) is Rising Sun's Gold Mine.
        let c = parse_counts("Copper, Silver, Gold, Mine, River Shrine").unwrap();
        assert_eq!((c.get(id::GOLD), c.get(id::MINE), c.get(id::GOLD_MINE)), (1, 1, 0));
        let c = parse_counts("2 Gold Mine, Gold").unwrap();
        assert_eq!((c.get(id::GOLD), c.get(id::MINE), c.get(id::GOLD_MINE)), (1, 0, 2));
        let c = parse_counts("3 Copper 2 Silver").unwrap();
        assert_eq!((c.get(id::COPPER), c.get(id::SILVER)), (3, 2), "entries separated by spaces only still parse");
    }
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
