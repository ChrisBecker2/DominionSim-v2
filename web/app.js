// Dominion text-state stepper — plain JS, no framework, no bundler.
// Talks to the dominion-wasm module through a hand-rolled C ABI (see crates/wasm/src/lib.rs):
// strings cross via alloc/dealloc + linear memory, structured results via a JSON result buffer.

(function () {
  "use strict";

  let wasm = null; // wasm.instance.exports
  let wasmModule = null; // compiled module, shared with analysis workers

  function base64ToBytes(b64) {
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return bytes;
  }

  // ---- wasm bridge -------------------------------------------------------

  function writeString(str) {
    const bytes = new TextEncoder().encode(str);
    const ptr = wasm.alloc(bytes.length >>> 0);
    new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
    return { ptr, len: bytes.length };
  }
  function freeString(s) {
    wasm.dealloc(s.ptr, s.len);
  }
  function readResult() {
    const ptr = wasm.result_ptr();
    const len = wasm.result_len();
    const bytes = new Uint8Array(wasm.memory.buffer, ptr, len);
    return new TextDecoder().decode(bytes);
  }
  // A Rust panic surfaces as a WebAssembly "unreachable" trap. Recover the real message, and
  // since the engine can't be trusted afterwards, say so and show the last position.
  let lastStateText = "";
  let crashed = false;

  function panicMessage() {
    try {
      const ptr = wasm.panic_message_ptr(), len = wasm.panic_message_len();
      return len ? new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, ptr, len)) : "";
    } catch (e) {
      return "";
    }
  }

  function reportCrash(e, action) {
    if (!(e instanceof WebAssembly.RuntimeError) || crashed) return false;
    crashed = true;
    const msg = panicMessage() || String(e.message || e);
    const box = $("crash-panel");
    box.hidden = false;
    $("crash-text").textContent =
      `Engine crashed during "${action}": ${msg}\n\nLast position before the crash:\n\n${lastStateText}`;
    return true;
  }

  function ok(status) {
    const msg = readResult();
    if (!status) throw new Error(msg || "operation failed");
    return msg;
  }

  const api = {
    newGame(players, kingdomText, seed, maxTurns) {
      const k = writeString(kingdomText);
      const status = wasm.new_game(players >>> 0, k.ptr, k.len, BigInt(seed), (maxTurns || 0) >>> 0);
      freeString(k);
      ok(status);
    },
    loadState(text) {
      const s = writeString(text);
      const status = wasm.load_state(s.ptr, s.len);
      freeString(s);
      ok(status);
    },
    getStateText() {
      wasm.get_state_text();
      return readResult();
    },
    getStartText() {
      wasm.get_start_text();
      return readResult();
    },
    getView() {
      wasm.get_view();
      return JSON.parse(readResult());
    },
    choose(index) {
      ok(wasm.choose(index >>> 0));
    },
    stepAuto() {
      ok(wasm.step_auto());
    },
    runToEndOfTurn() {
      ok(wasm.run_to_end_of_turn());
    },
    undo() {
      ok(wasm.undo());
    },
    redo() {
      ok(wasm.redo());
    },
    canUndo() {
      return !!wasm.can_undo();
    },
    canRedo() {
      return !!wasm.can_redo();
    },
    listBots() {
      ok(wasm.list_bots());
      return JSON.parse(readResult());
    },
    getSeats() {
      ok(wasm.get_seats());
      return JSON.parse(readResult());
    },
    setSeat(player, bot) {
      ok(wasm.set_seat(player >>> 0, bot >>> 0));
    },
    getSeatSearch() {
      ok(wasm.get_seat_search());
      return JSON.parse(readResult());
    },
    setSeatSearch(player, on) {
      ok(wasm.set_seat_search(player >>> 0, on ? 1 : 0));
    },
    runBots() {
      ok(wasm.run_bots());
    },
    gameStats() {
      return JSON.parse(ok(wasm.game_stats()));
    },
    resume() {
      ok(wasm.resume());
    },
    addStrategy(toml) {
      const s = writeString(toml);
      const status = wasm.add_strategy(s.ptr, s.len);
      freeString(s);
      return JSON.parse(ok(status));
    },
    seatKingdom(players) {
      return JSON.parse(ok(wasm.seat_kingdom(players >>> 0)));
    },
    paddedKingdom(players, setsMask, seed) {
      return JSON.parse(ok(wasm.padded_kingdom(players >>> 0, setsMask >>> 0, BigInt(seed))));
    },
    analyze() {
      return JSON.parse(ok(wasm.analyze()));
    },
    searchGraph(maxNodes) {
      return JSON.parse(ok(wasm.search_graph(maxNodes >>> 0)));
    },
    stateId() {
      return wasm.state_id() >>> 0;
    },
  };

  let botNames = [];

  // ---- loaded strategies --------------------------------------------------------
  // Strategy files added in the page. Seat ids follow load order, so the same list is replayed
  // into every worker (init + "add" messages) and into the page on reload (localStorage).
  const CUSTOM_KEY = "dominion.loadedStrategies";
  let customStrategies = []; // [{ name, toml }]

  function storedStrategies() {
    try {
      const v = JSON.parse(localStorage.getItem(CUSTOM_KEY) || "[]");
      return Array.isArray(v) ? v.filter((x) => x && typeof x.toml === "string") : [];
    } catch (e) {
      return [];
    }
  }

  function persistStrategies() {
    try {
      localStorage.setItem(CUSTOM_KEY, JSON.stringify(customStrategies));
    } catch (e) {
      /* storage unavailable: strategies last until reload */
    }
  }

  // Add (or replace by name) a strategy; returns { id, name, replaced }. Throws on a bad file.
  function addStrategy(toml, persist = true) {
    const r = api.addStrategy(toml);
    const i = customStrategies.findIndex((c) => c.name === r.name);
    if (i >= 0) customStrategies[i] = { name: r.name, toml };
    else customStrategies.push({ name: r.name, toml });
    if (persist) persistStrategies();
    botNames = api.listBots();
    if (pool) pool.forEach((p) => p.worker.postMessage({ type: "add", toml }));
    return r;
  }

  // ---- card type highlighting ------------------------------------------------

  let cardClass = new Map(); // card name -> css classes
  let cardTypes = new Map(); // card name -> type names (lower case), e.g. ["action", "duration"]
  let cardRegex = null;

  // <chip-class> Pure: the CSS classes of a card chip from its type names (tested by web/test_log.mjs).
  function chipClass(t) {
    // Duration cards are orange and Reactions blue, whatever else they are (Astrolabe,
    // Pirate, Moat, Diplomat...).
    const primary = t.includes("boon")
      ? "boon"
      : t.includes("hex")
      ? "hex"
      : t.includes("state")
      ? "state"
      : t.includes("ruins")
      ? "ruins"
      : t.includes("curse")
      ? "curse"
      : t.includes("duration")
      ? "duration"
      : t.includes("treasure")
      ? "treasure"
      : t.includes("victory")
      ? "victory"
      : t.includes("reaction")
      ? "reaction"
      : "action";
    // Two-type cards get a split chip: Treasure-Victory (Harem), Action-Victory (Mill, Nobles,
    // Island), Duration-Treasure (Astrolabe: orange/gold), Duration-Reaction (Pirate:
    // orange/blue). A Duration that is only an Action (Caravan) stays solid orange, a Reaction
    // that is only an Action (Moat) solid blue.
    const has = (x) => t.includes(x);
    const dual = has("treasure") && has("victory")
      ? " dual-treasure-victory"
      : has("action") && has("victory")
      ? " dual-action-victory"
      : has("duration") && has("treasure")
      ? " dual-duration-treasure"
      : has("duration") && has("reaction")
      ? " dual-duration-reaction"
      : "";
    // Nocturne: a card that is only a Night card is dark; a Night-Duration card is split orange/dark.
    const prophecyOnly = has("prophecy");
    // Renaissance: Projects (pink) and Artifacts (light brown) are landscape cards, not cards of the deck.
    if (has("project")) return "card project";
    if (has("artifact")) return "card artifact";
    const nightOnly = has("night") && !has("action") && !has("duration") && !has("treasure");
    const dualNight = has("night") && has("duration") ? " dual-duration-night" : "";
    return "card " + (prophecyOnly ? "prophecy" : nightOnly ? "night" : primary + dual + dualNight) + (has("attack") ? " attack" : "") + (has("shelter") ? " shelter" : "") + (has("heirloom") ? " heirloom" : "");
  }
  // </chip-class>

  function initCards() {
    for (const c of JSON.parse(ok(wasm.card_info()))) {
      cardClass.set(c.name, chipClass(c.types));
      cardTypes.set(c.name, c.types);
    }
    // Longest names first so "Throne Room" wins over any shorter overlap.
    const names = [...cardClass.keys()].sort((a, b) => b.length - a.length);
    cardRegex = new RegExp("\\b(" + names.map((n) => n.replace(/ /g, "\\s")).join("|") + ")\\b", "g");
  }

  function cardChip(name, label) {
    return el("span", cardClass.get(name) || "card", label === undefined ? name : label);
  }

  // Text with every card name wrapped in a type-coloured chip ("Win Game!" is emphasized).
  function decorate(text) {
    const win = text.indexOf("Win Game!");
    if (win >= 0) {
      const frag = document.createDocumentFragment();
      frag.appendChild(decorate(text.slice(0, win)));
      frag.appendChild(el("span", "win-game", "Win Game!"));
      frag.appendChild(decorate(text.slice(win + "Win Game!".length)));
      return frag;
    }
    const frag = document.createDocumentFragment();
    let last = 0;
    for (const m of text.matchAll(cardRegex)) {
      if (m.index > last) frag.appendChild(document.createTextNode(text.slice(last, m.index)));
      frag.appendChild(cardChip(m[0]));
      last = m.index + m[0].length;
    }
    if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
    return frag;
  }

  // A zone's contents as chips: "3 Copper" "Village".
  function cardItems(items) {
    const frag = document.createDocumentFragment();
    items.forEach((it, i) => {
      if (i) frag.appendChild(document.createTextNode(" "));
      frag.appendChild(cardChip(it.name, it.count > 1 ? `${it.count} ${it.name}` : it.name));
    });
    return frag;
  }

  // ---- DOM helpers ---------------------------------------------------------

  const $ = (id) => document.getElementById(id);
  function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }
  function cardListText(items) {
    if (!items.length) return "";
    return items.map((it) => (it.count > 1 ? `${it.count} ${it.name}` : it.name)).join(", ");
  }

  let toastTimer = null;
  function showToast(msg) {
    const t = $("toast");
    t.textContent = msg;
    t.hidden = false;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (t.hidden = true), 6000);
  }
  $("toast").addEventListener("click", () => ($("toast").hidden = true));

  function showLoadError(msg) {
    const e = $("text-error");
    if (!msg) {
      e.hidden = true;
      e.textContent = "";
    } else {
      e.hidden = false;
      e.textContent = msg;
    }
  }
  // Loaded states start at a phase boundary; a buy-phase state can't play the actions in hand.
  function showLoadNotice(view) {
    const n = $("text-notice");
    const me = view.players[view.turn.player];
    const actions = me
      ? me.hand.filter((c) => (cardTypes.get(c.name) || []).includes("action")).map((c) => c.name)
      : [];
    if (view.turn.phase === "buy" && actions.length) {
      n.textContent =
        `Loaded in the buy phase, so ${actions.join(", ")} can't be played this turn. ` +
        `To play actions, set "phase: action" (and move cards from "in play" back to hand/deck).`;
      n.hidden = false;
    } else {
      n.hidden = true;
    }
  }

  function showNewGameError(msg) {
    const e = $("newgame-error");
    if (!msg) {
      e.hidden = true;
      e.textContent = "";
    } else {
      e.hidden = false;
      e.textContent = msg;
    }
  }

  // ---- rendering -----------------------------------------------------------

  let lastView = null;

  function syncTextFromGame() {
    $("state-text").value = api.getStateText();
  }

  function renderTurnBar(view) {
    const bar = $("turn-bar");
    bar.innerHTML = "";
    const t = view.turn;
    const add = (label, value) => {
      const s = el("span", "stat");
      s.innerHTML = `${label} <b></b>`;
      s.querySelector("b").textContent = value;
      bar.appendChild(s);
    };
    add("Turn", t.number);
    add("Player", t.player + 1);
    add("Phase", t.phase);
    add("Actions", t.actions);
    add("Buys", t.buys);
    add("Coins", "$" + t.coins);
    // Alchemy: only in games with a Potion pile.
    if (view.supply.some((c) => c.name === "Potion")) add("Potions", String(t.potions || 0));
    // Cornucopia & Guilds: the turn player's Coffers (spend them with the Buy phase's button).
    const cur = view.players[t.player];
    if (cur && cur.coffers) add("Coffers", String(cur.coffers));
    // Renaissance: the turn player's Villagers (spent in the Action phase when no Action is left).
    if (cur && cur.villagers) add("Villagers", String(cur.villagers));
    // Empires: the turn player's Debt (nothing can be bought until it is paid).
    if (cur && cur.debt) add("Debt", String(cur.debt));
    if (view.gameOver) {
      const g = el("span", "gameover");
      const names = view.winners.map((p) => "P" + (p + 1)).join(", ");
      g.textContent = `GAME OVER — winner(s): ${names} (scores: ${view.scores.join(", ")})`;
      bar.appendChild(g);
    }
  }

  function zoneRow(label, items) {
    const row = el("div", "zone");
    const l = el("span", "zone-label", label + ":");
    row.appendChild(l);
    if (items.length) {
      row.appendChild(document.createTextNode(" "));
      row.appendChild(cardItems(items));
    } else {
      row.appendChild(el("span", "empty", " (empty)"));
    }
    return row;
  }

  // What each Duration card does at the start of its owner's next turn (see
  // docs/seaside-prosperity-plan.md §1), shown as a chip tooltip in the Durations zone.
  const DURATION_TEXT = {
    Haven: "Next turn: the set-aside card goes into your hand.",
    Lighthouse: "Next turn: +$1. (Also blocks Attacks while in play.)",
    Astrolabe: "Next turn: +$1, +1 Buy.",
    "Fishing Village": "Next turn: +1 Action, +$1.",
    Monkey: "Next turn: +1 Card. (Also: +1 Card when the player to your right gains, until then.)",
    Caravan: "Next turn: +1 Card.",
    Blockade: "Next turn: the set-aside card goes into your hand. (Gaining a copy elsewhere gives that player a Curse until then.)",
    Sailor: "Next turn: +$2; you may trash a card from your hand.",
    Corsair: "Next turn: +1 Card. (Also: other players trash the first Silver/Gold they play, until then.)",
    "Merchant Ship": "Next turn: +$2.",
    Outpost: "Take an extra turn after this one (3-card hand; not two in a row).",
    Pirate: "Next turn: gain a Treasure costing up to $6 to your hand.",
    "Sea Witch": "Next turn: +2 Cards, then discard 2 cards.",
    Tactician: "Next turn: +5 Cards, +1 Action, +1 Buy.",
    Wharf: "Next turn: +2 Cards, +1 Buy.",
  };

  function durationTitle(d) {
    let text = DURATION_TEXT[d.card] || "";
    if (d.arg && !d.arg.startsWith("used")) text += ` Set aside: ${d.arg}.`;
    else if (d.arg === "used") text += " (Already used this turn.)";
    else if (d.arg) text += ` (Used ${d.arg.slice(5)} time(s) this turn.)`;
    return text;
  }

  function durationsZone(durations) {
    const row = el("div", "zone");
    row.appendChild(el("span", "zone-label", "Durations:"));
    row.appendChild(document.createTextNode(" "));
    durations.forEach((d, i) => {
      if (i) row.appendChild(document.createTextNode(" "));
      const label = d.card + (d.times > 1 ? ` (x${d.times})` : "");
      const chip = cardChip(d.card, label);
      chip.title = durationTitle(d);
      row.appendChild(chip);
    });
    return row;
  }

  function renderPlayers(view) {
    const row = $("players-row");
    row.innerHTML = "";
    for (const p of view.players) {
      const panel = el("div", "player-panel" + (p.isCurrent ? " current" : ""));
      const h = el("h3");
      const nameSpan = el("span", null, `Player ${p.index + 1}` + (p.isCurrent ? " (current)" : ""));
      const vpSpan = el("span", "vp", `${p.vp} VP` + (p.vpTokens ? ` (${p.vpTokens} token${p.vpTokens === 1 ? "" : "s"})` : ""));
      h.appendChild(nameSpan);
      const sel = el("select", "seat-select");
      botNames.forEach((name, i) => {
        const o = el("option", null, name);
        o.value = String(i);
        sel.appendChild(o);
      });
      sel.value = String(seats[p.index] ?? 0);
      sel.addEventListener("change", () => {
        const untouched = !api.canUndo() && lastView && lastView.turn.number === 1;
        doAction(() => {
          api.setSeat(p.index, parseInt(sel.value, 10));
          // The kingdom follows the seats' rules; restart if the game hasn't started yet.
          if (syncKingdomFromSeats() && untouched) startNewGame();
        });
      });
      h.appendChild(sel);
      // A strategy seat searches its turns by default; unchecked, it plays by its rules alone (the Strategy Lab's fast bot).
      if ((seats[p.index] ?? 0) > 0) {
        const lab = el("label", "seat-search");
        lab.title = "Checked: the strategy searches each turn (play order, choices) as in single games and Simulate. Unchecked: it follows its rules alone, as the Strategy Lab's fast bots do while evolving.";
        const cb = el("input");
        cb.type = "checkbox";
        cb.checked = seatSearch[p.index] !== false;
        cb.addEventListener("change", () => doAction(() => api.setSeatSearch(p.index, cb.checked)));
        lab.appendChild(cb);
        lab.appendChild(document.createTextNode(" search"));
        h.appendChild(lab);
      }
      h.appendChild(vpSpan);
      panel.appendChild(h);

      panel.appendChild(zoneRow(`Hand (${p.handSize})`, p.hand));
      const deckRow = el("div", "zone");
      deckRow.appendChild(el("span", "zone-label", `Deck (${p.deckSize}):`));
      if (p.deckTop.length) {
        deckRow.appendChild(document.createTextNode(" top: "));
        p.deckTop.forEach((n, i) => {
          if (i) deckRow.appendChild(document.createTextNode(" "));
          deckRow.appendChild(cardChip(n));
        });
      }
      if (p.deckUnknown.length) {
        deckRow.appendChild(document.createTextNode(p.deckTop.length ? "; shuffled: " : " shuffled: "));
        deckRow.appendChild(cardItems(p.deckUnknown));
      }
      if (!p.deckTop.length && !p.deckUnknown.length) deckRow.appendChild(el("span", "empty", " (empty)"));
      panel.appendChild(deckRow);

      panel.appendChild(zoneRow("Discard", p.discard));
      panel.appendChild(zoneRow("In play", p.inPlay));
      if (p.setAside.length) panel.appendChild(zoneRow("Revealed / set aside", p.setAside));
      if (p.durations.length) panel.appendChild(durationsZone(p.durations));
      if (p.nativeVillageMat.length) panel.appendChild(zoneRow("Native Village mat", p.nativeVillageMat));
      if (p.islandMat.length) panel.appendChild(zoneRow("Island mat", p.islandMat));
      // Cards set aside by Princes (public): each is played at the start of the owner's turns.
      if (p.princeCards.length) panel.appendChild(zoneRow("Set aside by Prince", p.princeCards));
      // Cornucopia & Guilds: Coffers tokens, cards a Farmhands set aside, a Province Joust set aside.
      if (p.coffers) panel.appendChild(el("div", "zone coffers", `Coffers: ${p.coffers}`));
      // Renaissance: Villagers, the Projects with a cube of this player (each player has two cubes), the tokens on Sinister Plot, and what the Cargo Ships
      // (face up) and Researches (face down) have set aside for the next turn.
      if (p.villagers) panel.appendChild(el("div", "zone villagers", `Villagers: ${p.villagers}`));
      if (p.ownedProjects && p.ownedProjects.length) {
        const row = el("div", "zone");
        row.appendChild(el("span", "zone-label", "Projects:"));
        for (const n of p.ownedProjects) {
          row.appendChild(document.createTextNode(" "));
          row.appendChild(cardChip(n));
        }
        panel.appendChild(row);
      }
      if (p.plotTokens) panel.appendChild(el("div", "zone", `Tokens on Sinister Plot: ${p.plotTokens}`));
      if (p.cargo && p.cargo.length) panel.appendChild(zoneRow("Set aside by Cargo Ship", p.cargo));
      if (p.research && p.research.length) panel.appendChild(zoneRow("Set aside by Research (face down)", p.research));
      // Empires: Debt tokens, and the cards set aside under Archives.
      if (p.debt) panel.appendChild(el("div", "zone debt", `Debt: ${p.debt}`));
      if (p.archive && p.archive.length) panel.appendChild(zoneRow("Archive", p.archive));
      // Adventures: the Tavern mat, the tokens, the Estate token and the Events bought.
      if (p.tavern && p.tavern.length) panel.appendChild(zoneRow("Tavern mat", p.tavern));
      if (p.tokens && p.tokens.length) panel.appendChild(el("div", "zone", "Tokens: " + p.tokens.map((t) => `${t.token} on ${t.pile}`).join(", ")));
      {
        const flags = [];
        if (p.minusCard) flags.push("-1 Card (on the deck)");
        if (p.minusCoin) flags.push("-$1");
        if (p.journeyDown) flags.push("Journey face down");
        if (p.inherit) flags.push("Estates play " + p.inherit);
        if (p.saved) flags.push("Saved: " + p.saved);
        if (flags.length) panel.appendChild(el("div", "zone", flags.join("; ")));
      }
      if (p.boughtEvents && p.boughtEvents.length) panel.appendChild(el("div", "zone", "Events bought: " + p.boughtEvents.join(", ")));
      // Nocturne: the States in front of the player, and Faithful Hounds set aside until the end of the turn.
      if (p.states && p.states.length) panel.appendChild(el("div", "zone", "States: " + p.states.join(", ")));
      if (p.hounds) panel.appendChild(el("div", "zone", `Faithful Hounds set aside until the end of the turn: ${p.hounds}`));
      if (p.farmCards.length) panel.appendChild(zoneRow("Set aside by Farmhands", p.farmCards));
      if (p.summonCards && p.summonCards.length) panel.appendChild(zoneRow("Set aside by Summon (played next turn)", p.summonCards));
      if (p.churchCards && p.churchCards.length) panel.appendChild(zoneRow("Set aside by Church (face down)", p.churchCards));
      if (p.cleanupAside.length) panel.appendChild(zoneRow("Set aside until Clean-up", p.cleanupAside));
      panel.appendChild(el("div", "zone", `Turns taken: ${p.turnsTaken}`));

      row.appendChild(panel);
    }
  }

  // The Supply piles, then (after a gap) the piles outside the Supply: Travellers, Spoils, Madman, Rewards, Ferryman's pile, Nocturne's Spirits.
  function renderSupply(view) {
    const side = view.supply.filter((c) => c.side);
    fillPiles($("supply-table"), view.supply.filter((c) => !c.side));
    fillPiles($("side-table"), side);
    $("side-panel").hidden = side.length === 0;
  }

  function fillPiles(table, piles) {
    const tbody = table.querySelector("tbody");
    tbody.innerHTML = "";
    tbody.style.setProperty("--supply-rows", Math.ceil(piles.length / 2));
    for (const c of piles) {
      const tr = document.createElement("tr");
      const tdName = document.createElement("td");
      tdName.appendChild(cardChip(c.name));
      // The printed cost, "$3" or "$2P" (P = Potion).
      tdName.appendChild(el("span", "cost", " " + (c.costLabel || "$" + c.cost)));
      // Young Witch's extra pile is a Supply pile; Ferryman's and the Rewards are not in the Supply.
      if (c.bane) tdName.appendChild(el("span", "cost", " Bane"));
      // The Knights and Ruins piles: one entry, showing the top card (the only one that can be bought) and how many are left.
      if (c.pile) tdName.appendChild(el("span", "cost", ` (${c.pile} pile, top card)`));
      // Adventures tokens on the pile, by owner.
      if (c.tokens) tdName.appendChild(el("span", "cost", " [" + c.tokens.join(", ") + "]"));
      // Empires: the halves of a split pile, and the VP and Debt tokens on a pile.
      if (c.half) tdName.appendChild(el("span", "cost", c.half === "top" ? " (split pile, top half)" : " (split pile, bottom half)"));
      if (c.vp) tdName.appendChild(el("span", "cost", ` [${c.vp} VP]`));
      if (c.debtTokens) tdName.appendChild(el("span", "cost", ` [${c.debtTokens} Debt]`));
      const tdCount = document.createElement("td");
      tdCount.textContent = c.count;
      if (c.count === 0) tdCount.className = "count-0";
      tr.appendChild(tdName);
      tr.appendChild(tdCount);
      tbody.appendChild(tr);
    }
  }

  // The horizontal cards, one row above the Supply: Events (bought in the Buy phase for their cost and a Buy; the button picks the matching
  // choice of the pending decision), Landmarks (always in effect; those holding VP tokens show how many are left) and the Prophecy (with the
  // Sun tokens left on it; it happens when the last one is removed).
  function renderLandscape(view) {
    const list = $("landscape-list");
    list.innerHTML = "";
    const events = view.events || [], landmarks = view.landmarks || [], p = view.prophecy;
    const projects = view.projects || [], artifacts = view.artifacts || [];
    $("landscape-panel").hidden = !events.length && !landmarks.length && !p && !projects.length && !artifacts.length;
    // Renaissance: the Artifacts (held by one player at a time) and the Projects (bought like Events; the cubes show who has each).
    for (const a of artifacts) {
      const row = el("div", "event-row");
      row.appendChild(el("span", "kind", "Artifact"));
      row.appendChild(cardChip(a.name));
      row.appendChild(el("span", "cost", a.holder === null ? " (nobody has it)" : ` held by P${a.holder + 1}`));
      list.appendChild(row);
    }
    for (const pr of projects) {
      const row = el("div", "event-row");
      row.appendChild(el("span", "kind", "Project"));
      row.appendChild(cardChip(pr.name));
      row.appendChild(el("span", "cost", " " + (pr.costLabel || "$" + pr.cost) + (pr.owners.length ? " [" + pr.owners.map((o) => "P" + (o + 1)).join(", ") + "] " : " ")));
      const b = el("button", null, "Buy");
      const choice = view.pending ? view.pending.choices.find((c) => c.label === pr.name) : null;
      b.disabled = !choice;
      if (choice) b.addEventListener("click", () => doAction(() => api.choose(choice.index), "Buy " + pr.name));
      row.appendChild(b);
      list.appendChild(row);
    }
    if (p) {
      const row = el("div", "event-row");
      row.appendChild(el("span", "kind", "Prophecy"));
      row.appendChild(cardChip(p.name));
      row.appendChild(el("span", "cost", p.active ? " in effect" : ` ${p.suns} Sun token${p.suns === 1 ? "" : "s"}`));
      list.appendChild(row);
    }
    for (const lm of landmarks) {
      const row = el("div", "event-row");
      row.appendChild(el("span", "kind", "Landmark"));
      row.appendChild(cardChip(lm.name));
      if (lm.vp) row.appendChild(el("span", "cost", ` ${lm.vp} VP`));
      list.appendChild(row);
    }
    for (const ev of events) {
      const row = el("div", "event-row");
      row.appendChild(el("span", "kind", "Event"));
      row.appendChild(cardChip(ev.name));
      row.appendChild(el("span", "cost", " " + (ev.costLabel || "$" + ev.cost) + " "));
      const b = el("button", null, "Buy");
      const choice = view.pending ? view.pending.choices.find((c) => c.label === ev.name) : null;
      b.disabled = !choice;
      if (choice) b.addEventListener("click", () => doAction(() => api.choose(choice.index), "Buy " + ev.name));
      row.appendChild(b);
      list.appendChild(row);
    }
  }

  // Nocturne: the Boons and Hexes decks (which are still to come, which are in the discard piles, which are set aside for Druid or kept in front of a
  // player), and Lost in the Woods. The States of each player are in the player's panel.
  // The collapsed "Piles" section: the order of the piles whose cards differ (the Castles in their
  // fixed order; Knights and Ruins as dealt, the top card face up and the rest dimmed; split piles
  // are not listed, the Supply shows both halves), then Nocturne's Boons and Hexes. Shown only when the game has any of them; collapsed by default.
  function renderNocturne(view) {
    const panel = $("piles-panel");
    const list = $("nocturne-list");
    const orderList = $("pile-order-list");
    list.innerHTML = "";
    orderList.innerHTML = "";
    const n = view.nocturne;
    const piles = view.pileOrder || [];
    const hasNocturne = !!n && !!(n.boons || n.hexes);
    panel.hidden = !hasNocturne && !piles.length;
    if (panel.hidden) return;
    for (const p of piles) {
      const r = el("div", "event-row pile-row");
      r.appendChild(el("span", "cost", `${p.name}: `));
      if (p.order === "fixed") {
        if (!p.cards.some(([, k]) => k > 0)) r.appendChild(document.createTextNode("empty"));
        p.cards.filter(([, k]) => k > 0).forEach(([nm, k]) => {
          r.appendChild(cardChip(nm, k > 1 ? `${k} ${nm}` : nm));
          r.appendChild(document.createTextNode(" "));
        });
      } else if (!p.cards.length) {
        r.appendChild(document.createTextNode("empty"));
      } else {
        // Shuffled piles (Knights, Ruins): every card in the dealt order, top first. The top card is face
        // up; the others are face down, drawn dimmed (this page shows everything). A long pile (50 Ruins
        // with 6 players) folds runs of the same card into "Ruined Village ×3".
        const fold = p.cards.length > 14;
        for (let i = 0; i < p.cards.length; ) {
          let n = 1;
          if (fold && i > 0) while (i + n < p.cards.length && p.cards[i + n] === p.cards[i]) n++;
          const nm = p.cards[i];
          const chip = cardChip(nm, n > 1 ? `${nm} ×${n}` : nm);
          if (i > 0) {
            chip.classList.add("face-down");
            chip.title = "Face down";
          }
          r.appendChild(chip);
          r.appendChild(document.createTextNode(" "));
          i += n;
        }
      }
      orderList.appendChild(r);
    }
    if (!hasNocturne) return;
    const row = (label, names) => {
      if (!names.length) return;
      const r = el("div", "event-row");
      r.appendChild(el("span", "cost", label + ": "));
      names.forEach((nm, i) => {
        if (i) r.appendChild(document.createTextNode(" "));
        r.appendChild(cardChip(nm));
      });
      list.appendChild(r);
    };
    if (n.boons) {
      row("Boons to come", n.boonDeck);
      row("Boons discarded", n.boonDiscard);
      row("Set aside for Druid", n.druid);
      row("Boons kept until Clean-up", n.boonsHeld);
    }
    if (n.hexes) {
      row("Hexes to come", n.hexDeck);
      row("Hexes discarded", n.hexDiscard);
    }
    if (n.lost !== null && n.lost !== undefined) list.appendChild(el("div", "event-row", `Lost in the Woods: Player ${n.lost + 1}`));
  }

  function renderTrash(view) {
    const div = $("trash-list");
    div.innerHTML = "";
    if (view.trash.length) div.appendChild(cardItems(view.trash));
    else div.textContent = "(empty)";
  }

  function renderDecision(view) {
    const desc = $("decision-desc");
    const choices = $("decision-choices");
    choices.innerHTML = "";
    if (!view.pending) {
      desc.textContent = view.gameOver ? "Game over." : "No decision pending.";
      return;
    }
    const who = botNames[seats[view.pending.player] ?? 0] || "";
    desc.textContent = `${view.pending.description}  [${who}]`;
    view.pending.choices.forEach((c, i) => {
      const b = el("button");
      b.appendChild(decorate(c.label));
      b.addEventListener("click", () => doAction(() => {
        if (c.index < 0) api.resume();
        else api.choose(c.index);
      }, c.label));
      choices.appendChild(b);
    });
  }

  // The log only grows during a game (the last line can still change when a run of draws or
  // plays is condensed onto it), so append new lines and refresh the last one; rebuild when the
  // log was reset or rewritten (new game, load).
  let renderedLog = [];

  function logRow(line) {
    const cls = line.startsWith("---")
      ? "turn-marker"
      : line.startsWith("::")
      ? "phase-marker"
      : line.startsWith("--")
      ? "sys-marker"
      : "";
    const row = el("div", cls || null);
    row.appendChild(decorate(line));
    return row;
  }

  function renderLog(view) {
    const list = $("log-list");
    const atBottom = list.scrollTop + list.clientHeight >= list.scrollHeight - 4;
    const log = view.log;
    const keep = renderedLog.length && log.length >= renderedLog.length && log[0] === renderedLog[0]
      ? renderedLog.length - 1
      : 0;
    if (keep === 0) {
      list.innerHTML = "";
    } else {
      while (list.children.length > keep) list.removeChild(list.lastChild);
    }
    for (let i = keep; i < log.length; i++) list.appendChild(logRow(log[i]));
    renderedLog = log.slice();
    if (atBottom || log.length <= 1) list.scrollTop = list.scrollHeight;
  }


  let seats = [];
  let seatSearch = []; // per seat: whether its strategy searches its turns (unchecked: its rules alone, like the Lab's fast bots)

  // ---- parallel analysis in Web Workers -------------------------------------------
  //
  // The main module splits the decision's search tree into independent subtrees (plan_start);
  // each worker runs its own instance of the same module and evaluates subtrees (eval_task);
  // results are folded back exactly (plan_put_result / plan_finish). States cross as raw bytes.

  const WORKER_SRC = `
    let w = null;
    onmessage = async (e) => {
      const m = e.data;
      const addStrategy = (toml) => {
        const bytes = new TextEncoder().encode(toml);
        const p = w.alloc(bytes.length);
        new Uint8Array(w.memory.buffer, p, bytes.length).set(bytes);
        w.add_strategy(p, bytes.length);
        w.dealloc(p, bytes.length);
      };
      if (m.type === "init") {
        const inst = await WebAssembly.instantiate(m.module, {});
        w = inst.exports;
        w.init();
        (m.custom || []).forEach(addStrategy);
        postMessage({ type: "ready" });
        return;
      }
      if (m.type === "add") {
        addStrategy(m.toml);
        return;
      }
      if (m.type === "sim") {
        m.seats.forEach((seat, p) => w.set_seat(p, seat));
        (m.search || []).forEach((on, p) => w.set_seat_search(p, on ? 1 : 0));
        const b = m.state, sp = w.alloc(b.length);
        new Uint8Array(w.memory.buffer, sp, b.length).set(b);
        let okSim, out;
        try {
          okSim = w.simulate(sp, m.games, m.seed, m.flags | 0, m.first | 0);
          out = new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()));
        } catch (e) {
          const len = w.panic_message_len();
          out = "worker crashed: " + (len ? new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.panic_message_ptr(), len)) : String(e.message || e));
          okSim = 0;
        }
        w.dealloc(sp, b.length);
        postMessage({ type: "sim", id: m.id, ok: !!okSim, out });
        return;
      }
      const put = (bytes) => {
        const p = w.alloc(bytes.length);
        new Uint8Array(w.memory.buffer, p, bytes.length).set(bytes);
        return p;
      };
      const rp = put(m.root), sp = put(m.state);
      let ok;
      try {
        ok = w.eval_task(rp, sp, m.me, m.budget, m.strategy);
      } catch (e) {
        const len = w.panic_message_len();
        const msg = len ? new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.panic_message_ptr(), len)) : String(e.message || e);
        postMessage({ type: "result", id: m.id, ok: false, out: "worker crashed: " + msg });
        return;
      }
      const out = new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()));
      w.dealloc(rp, m.root.length);
      w.dealloc(sp, m.state.length);
      postMessage({ type: "result", id: m.id, ok: !!ok, out });
    };
  `;

  const TASK_BUDGET = 200000; // nodes per subtree before sampling (hard cap 4x, then playouts)
  let pool = null; // array of { worker, ready: Promise }
  let analysisRun = 0; // bumped to cancel/ignore an in-flight analysis

  function workerCount() {
    return Math.max(1, Math.min(navigator.hardwareConcurrency || 4, 32));
  }

  function getPool() {
    if (pool) return pool;
    const url = URL.createObjectURL(new Blob([WORKER_SRC], { type: "text/javascript" }));
    pool = [];
    for (let i = 0; i < workerCount(); i++) {
      const worker = new Worker(url);
      const ready = new Promise((resolve, reject) => {
        worker.onmessage = (e) => e.data.type === "ready" && resolve();
        worker.onerror = (e) => reject(new Error(e.message || "worker failed"));
      });
      worker.postMessage({ type: "init", module: wasmModule, custom: customStrategies.map((c) => c.toml) });
      pool.push({ worker, ready });
    }
    return pool;
  }

  function killPool() {
    if (pool) pool.forEach((p) => p.worker.terminate());
    pool = null;
  }

  function resultBytes() {
    return new Uint8Array(wasm.memory.buffer, wasm.result_ptr(), wasm.result_len()).slice();
  }

  function showProgress(text) {
    const panel = $("analysis-panel");
    panel.hidden = false;
    $("analysis-table").querySelector("tbody").innerHTML = "";
    $("analysis-meta").textContent = "";
    $("analysis-progress").hidden = false;
    $("analysis-progress-text").textContent = text;
  }

  function setProgressBar(frac) {
    $("analysis-bar").style.width = (100 * frac).toFixed(1) + "%";
  }

  async function analyzeParallel() {
    const run = ++analysisRun;
    const t0 = performance.now();
    const workers = getPool();
    showProgress(`Starting ${workers.length} workers…`);
    setProgressBar(0);
    await Promise.all(workers.map((p) => p.ready));
    if (run !== analysisRun) return;

    const plan = JSON.parse(ok(wasm.plan_start(workers.length * 4)));
    ok(wasm.plan_root_bytes());
    const root = resultBytes();
    const tasks = [];
    for (let i = 0; i < plan.tasks; i++) {
      ok(wasm.plan_task_bytes(i));
      tasks.push(resultBytes());
    }

    let next = 0, done = 0, nodes = 0;
    const update = () => {
      const secs = (performance.now() - t0) / 1000;
      $("analysis-progress-text").textContent =
        `Searching: ${done}/${tasks.length} subtrees on ${workers.length} workers · ` +
        `${nodes.toLocaleString()} nodes · ${secs.toFixed(1)} s`;
      setProgressBar(tasks.length ? done / tasks.length : 1);
    };
    update();

    await new Promise((resolve, reject) => {
      if (!tasks.length) return resolve();
      const feed = (p) => {
        if (run !== analysisRun) return resolve();
        if (next >= tasks.length) return;
        const id = next++;
        p.worker.postMessage({ type: "task", id, root, state: tasks[id], me: plan.player, budget: TASK_BUDGET, strategy: plan.strategy });
      };
      workers.forEach((p) => {
        p.worker.onmessage = (e) => {
          const m = e.data;
          if (m.type !== "result" || run !== analysisRun) return;
          if (!m.ok) return reject(new Error(m.out));
          const r = JSON.parse(m.out);
          const pv = writeString(r.pv);
          const oc = writeString(r.outcomes);
          const st = wasm.plan_put_result(m.id, r.ev, r.exact ? 1 : 0, r.nodes, r.ttHits, pv.ptr, pv.len, oc.ptr, oc.len);
          freeString(pv);
          freeString(oc);
          ok(st);
          done++;
          nodes += r.nodes;
          update();
          if (done === tasks.length) resolve();
          else feed(p);
        };
        p.worker.onerror = (e) => reject(new Error(e.message || "worker failed"));
        feed(p);
      });
    });
    if (run !== analysisRun) return;

    const result = JSON.parse(ok(wasm.plan_finish()));
    result.seconds = (performance.now() - t0) / 1000;
    result.workers = workers.length;
    renderAnalysis(result);
  }

  function cancelAnalysis() {
    analysisRun++;
    killPool(); // workers may be mid-subtree; terminate and start fresh next time
    renderAnalysis(null);
  }

  function formatPct(p) {
    const v = p * 100;
    return v >= 99.95 ? "100%" : v >= 10 ? `${v.toFixed(0)}%` : v >= 1 ? `${v.toFixed(1)}%` : `${v.toFixed(2)}%`;
  }

  let shownAnalysis = null; // last analysis rendered (for Auto-step to follow)

  // The kingdom the seats' rules require (no padding); search seats name none. New Game pads
  // this out to 10 with random cards from the selected sets (see `paddedKingdom`), seeded from
  // the Seed input. `gameRequiredKingdom` is the required kingdom the current game was started
  // with, used only to detect when the seats now want different cards than the running game has.
  let gameRequiredKingdom = null;

  const SETS_KEY = "dominion.cardSets";
  // Bit 0 = Base, bit 1 = Intrigue, bit 2 = Seaside, bit 3 = Prosperity, bit 4 = Alchemy, bit 5 = Promo,
  // bit 6 = Cornucopia & Guilds, bit 7 = Dark Ages, bit 8 = Adventures, bit 9 = Empires, bit 10 = Rising Sun, bit 11 = Nocturne, bit 12 = Hinterlands (matches `padded_kingdom`).
  const SET_BITS = { base: 1, intrigue: 2, seaside: 4, prosperity: 8, alchemy: 16, promo: 32, cornucopia: 64, darkages: 128, adventures: 256, empires: 512, risingsun: 1024, nocturne: 2048, hinterlands: 4096, renaissance: 8192 };

  function setCheckboxes() {
    return Array.from(document.querySelectorAll("#ng-sets .ng-set"));
  }

  // The selected sets as a bitmask; 0 if every box is somehow unchecked (callers should treat
  // that as "nothing to draw from" -- `onSetsChanged` below prevents it from happening via the UI).
  function cardSetsMask() {
    let mask = 0;
    for (const b of setCheckboxes()) if (b.checked) mask |= SET_BITS[b.value] || 0;
    return mask;
  }

  function loadCardSetsSelection() {
    try {
      const v = localStorage.getItem(SETS_KEY);
      const mask = v == null ? NaN : parseInt(v, 10);
      if (Number.isFinite(mask) && mask > 0) {
        for (const b of setCheckboxes()) b.checked = (mask & (SET_BITS[b.value] || 0)) !== 0;
      }
    } catch (e) {
      /* localStorage unavailable: default selection (all four sets) stands */
    }
  }

  function persistCardSetsSelection() {
    try {
      localStorage.setItem(SETS_KEY, String(cardSetsMask()));
    } catch (e) {
      /* ignore: storage unavailable */
    }
  }

  // Never allow every set to end up unchecked (New Game would have no cards to draw from): a
  // box that would zero out the selection snaps back to checked.
  function onSetsChanged(e) {
    if (cardSetsMask() === 0) {
      e.target.checked = true;
      return;
    }
    persistCardSetsSelection();
  }

  function seatsKingdom() {
    const players = parseInt($("ng-players").value, 10) || 2;
    return api.seatKingdom(players).kingdom;
  }

  // Whether the seats now require different cards than the running game's kingdom was built for.
  function syncKingdomFromSeats() {
    return gameRequiredKingdom !== null && seatsKingdom() !== gameRequiredKingdom;
  }

  // ---- kingdom and strategies handed over by the Strategy Lab ----------------------------
  // index.html#lab=<base64 of UTF-8 JSON {kingdom: [card names], strategies: [TOML text]}>. The kingdom (when given)
  // is the one the strategies were found on; New Game keeps using it until "Random kingdoms" is pressed.
  let labKingdom = null; // comma-separated card names, or null for a kingdom built from the seats
  let labSeatNote = "";

  function showLabKingdomNote() {
    const note = $("lab-kingdom-note");
    if (!note) return;
    note.hidden = labKingdom === null;
    if (labKingdom !== null) $("lab-kingdom-text").textContent = "Strategy Lab kingdom: " + labKingdom + labSeatNote;
  }

  // Add the payload's strategies (names made unique within it: "X", "X (2)") and seat them: one strategy sits
  // as Player 2 against the default Player 1, several take seats 1.. in order. Throws a readable error.
  function applyLabPayload(p) {
    const tomls = p && Array.isArray(p.strategies) ? p.strategies.filter((s) => typeof s === "string" && s.trim()) : [];
    if (tomls.length < 1) throw new Error("the link carries no strategy");
    if (tomls.length > 6) throw new Error(`the link carries ${tomls.length} strategies, the game has at most 6 seats`);
    const names = Array.isArray(p.kingdom) ? p.kingdom.filter((s) => typeof s === "string" && s.trim()).map((s) => s.trim()) : [];
    const taken = new Set();
    const ids = tomls.map((text) => {
      let toml = text;
      const m = text.match(/^name\s*=\s*"((?:[^"\\]|\\.)*)"/m);
      if (m) {
        let name = m[1];
        for (let n = 2; taken.has(name); n++) name = `${m[1]} (${n})`;
        taken.add(name);
        if (name !== m[1]) toml = text.replace(m[0], () => `name = "${name}"`);
      }
      return addStrategy(toml).id;
    });
    $("ng-players").value = String(Math.max(2, ids.length));
    if (ids.length === 1) api.setSeat(1, ids[0]);
    else ids.forEach((id, i) => api.setSeat(i, id));
    labKingdom = names.length ? names.join(", ") : null;
    labSeatNote = "";
    if (ids.length === 1 && labKingdom !== null) {
      // The default Player 1 (Double Witch) may need cards this kingdom does not have: then Big Money Ultimate sits there.
      const have = new Set(names);
      const missing = seatsKingdom().split(",").map((x) => x.trim()).filter((x) => x && !have.has(x));
      const bm = botNames.indexOf("Big Money Ultimate");
      if (missing.length && bm >= 0) {
        api.setSeat(0, bm);
        labSeatNote = " (Player 1 is Big Money Ultimate: the default Player 1 needs " + missing.join(", ") + ")";
      }
    }
  }

  function startNewGame() {
    if (stopRunGame) stopRunGame();
    const players = parseInt($("ng-players").value, 10) || 2;
    // An empty Seed box: a fresh random seed for every game (shown in the box's tooltip, so a game can be replayed); a number: that seed.
    const typed = parseInt($("ng-seed").value, 10);
    const seed = Number.isFinite(typed) ? typed : 1 + Math.floor(Math.random() * 2147483646);
    $("ng-seed").title = (Number.isFinite(typed) ? "Seed " : "This game's random seed: ") + seed +
      ". Empty: a new random seed for every new game. A number: the same kingdom and shuffles every time (to replay a game).";
    cancelAnalysis();
    gameRequiredKingdom = seatsKingdom();
    let kingdom;
    if (labKingdom !== null) {
      // The Strategy Lab's kingdom, exactly as listed (Events, Landmarks, the Prophecy, Platinum / Colony included):
      // nothing is added or drawn, so the strategies meet the kingdom they were found on.
      const have = new Set(labKingdom.split(",").map((s) => s.trim()));
      const missing = gameRequiredKingdom.split(",").map((s) => s.trim()).filter((s) => s && !have.has(s));
      if (missing.length) {
        throw new Error(`The seated strategies need ${missing.join(", ")}, which the Strategy Lab's kingdom does not have.`);
      }
      kingdom = labKingdom;
    } else {
      const setsMask = cardSetsMask() || (SET_BITS.base | SET_BITS.intrigue | SET_BITS.seaside | SET_BITS.prosperity);
      kingdom = api.paddedKingdom(players, setsMask, seed).kingdom;
    }
    showLabKingdomNote();
    api.newGame(players, kingdom, seed, 0);
    showNewGameError("");
    $("state-text").value = api.getStartText();
  }

  function renderAnalysis(result) {
    shownAnalysis = result;
    const panel = $("analysis-panel");
    const tbody = $("analysis-table").querySelector("tbody");
    tbody.innerHTML = "";
    if (!result) {
      closeSearchGraph(); // the graph describes a position that is no longer current
      panel.hidden = true;
      $("analysis-progress").hidden = true;
      return;
    }
    panel.hidden = false;
    $("analysis-progress").hidden = true;
    // Compact search stats: nodes, share of nodes answered from the transposition table
    // (positions reached again by another order of play), time and workers.
    const count = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : String(n));
    const meta = [];
    if (result.nodes !== undefined) meta.push(`${count(result.nodes)} nodes`);
    if (result.nodes && result.ttHits !== undefined) meta.push(`${Math.round((100 * result.ttHits) / result.nodes)}% transpositions`);
    if (result.seconds !== undefined) meta.push(`${result.seconds.toFixed(2)} s on ${result.workers} worker${result.workers === 1 ? "" : "s"}`);
    $("analysis-meta").textContent = meta.join(" · ");
    result.options.forEach((o, i) => {
      const tr = el("tr", i === 0 ? "best" : null);
      const label = el("td");
      label.appendChild(decorate(o.label));
      if (o.rulesPick) label.appendChild(el("span", "rules-pick", "[strategy pick]"));
      if (o.analysisPick) label.appendChild(el("span", "rules-better", "[analysis pick]"));
      tr.appendChild(label);
      const ev = el("td", "ev", o.ev.toFixed(2));
      if (!o.exact) ev.appendChild(el("span", "approx", "~sampled"));
      tr.appendChild(ev);
      // Outcomes this turn: most likely first; a turn that wins the game is highlighted.
      const oc = el("td", "outcomes");
      const shown = o.outcomes.slice(0, 6);
      shown.forEach((x, k) => {
        const item = el("span", "outcome" + (/Win Game/.test(x.label) ? " win" : ""));
        item.appendChild(decorate(x.label));
        item.appendChild(el("b", null, ` ${formatPct(x.p)}`));
        oc.appendChild(item);
      });
      const rest = o.outcomes.slice(shown.length).reduce((a, x) => a + x.p, 0);
      if (rest > 0.0005) oc.appendChild(el("span", "outcome muted", `other ${formatPct(rest)}`));
      const pv = el("td", "pv");
      pv.appendChild(decorate(o.pv));
      tr.appendChild(pv);
      tr.appendChild(oc);
      tbody.appendChild(tr);
    });
  }

  // ---- search graph ------------------------------------------------------------
  // Every position the turn search valued, as a layered DAG (columns = steps from the decision).
  // Pure helpers (index, visible set, layout, route marks) live in graph.js; this is the view.

  const SG = window.SearchGraph;
  const SG_CAP = 5000; // positions requested from the engine
  const SG_BEST_CAP = 800; // positions drawn in "Best line + merges"
  const SG_COLW = 100, SG_ROWH = 14; // world units per column / per row
  const SG_PADX = 34, SG_PADY = 26;
  let sg = null; // the open graph view, or null
  let sgToken = 0; // bumped on close so an in-flight build is dropped
  const SG_KIND = { root: "Decision (start)", decision: "Decision", chance: "Draw", leaf: "Turn end" };

  function closeSearchGraph() {
    sgToken++;
    sg = null;
    const panel = $("sg-panel");
    if (panel) panel.hidden = true;
  }

  // At a pause ("Start turn", or "Continue" after Load State), take it first: Analyze, the
  // search graph and the test hooks all work on the decision that follows. False if there is
  // then no decision to work on.
  function resumeIfPaused(label) {
    if (lastView && lastView.pending && lastView.pending.paused) {
      doAction(() => api.resume(), label);
      if (crashed || !lastView.pending || lastView.pending.paused || lastView.gameOver) return false;
    }
    return !crashed;
  }

  async function openSearchGraph() {
    if (crashed) return;
    if (!resumeIfPaused("Continue")) return;
    const token = ++sgToken;
    sg = null;
    $("sg-panel").hidden = false;
    $("sg-body").hidden = true;
    $("sg-summary").textContent = "Building…";
    // Let the browser paint "Building…" before the synchronous search blocks the thread.
    await new Promise((resolve) => {
      let done = false;
      const go = () => {
        if (!done) {
          done = true;
          resolve();
        }
      };
      requestAnimationFrame(() => setTimeout(go, 0));
      setTimeout(go, 150);
    });
    if (token !== sgToken) return;
    let g;
    try {
      g = api.searchGraph(SG_CAP);
    } catch (e) {
      if (reportCrash(e, "Search graph")) return;
      $("sg-summary").textContent = "Search graph failed: " + String(e.message || e);
      return;
    }
    showSearchGraph(g);
  }

  function readGraphColors() {
    const cs = getComputedStyle(document.documentElement);
    const v = (name, d) => cs.getPropertyValue(name).trim() || d;
    return {
      accent: v("--accent", "#8a5a2b"),
      accent2: v("--accent-2", "#2b6b4a"),
      muted: v("--sg-muted", "#b3ae9c"),
      text: v("--text", "#23221d"),
      dim: v("--text-dim", "#6b6a60"),
      panel: v("--panel", "#ffffff"),
    };
  }

  function showSearchGraph(g) {
    const idx = SG.buildIndex(g);
    const stats = SG.depthStats(g);
    const sm = SG.summary(g);
    const fmt = (n) => n.toLocaleString("en-US");
    $("sg-summary").textContent =
      `${fmt(sm.positions)} positions · ${fmt(sm.routes)} routes · ${fmt(sm.merged)} positions reached by more than one route ` +
      `(${Math.round(sm.mergedPct)}%) · search valued ${fmt(g.searchedNodes)} nodes, ${Math.round(sm.ttPct)}% answered from the ` +
      `transposition table` + (g.truncated ? ` (graph capped at ${fmt(SG_CAP)} positions)` : "");
    const canvas = $("sg-canvas");
    const ebest = new Uint8Array(idx.m);
    for (let i = 0; i < idx.m; i++) ebest[i] = g.edges[i].best ? 1 : 0;
    const maxDepth = stats.length - 1;
    sg = {
      g, idx, stats, ebest, maxDepth,
      mode: "best", first: idx.outStart[1] > idx.outStart[0] ? idx.outList[idx.outStart[0]] : -1,
      dmin: 0, dmax: maxDepth,
      pin: -1, hover: -1, marks: null, lay: null, vis: null,
      view: { k: 1, tx: 0, ty: 0 }, fx: 1, fy: 1, userMoved: false, drag: null,
      w: 0, h: 0, dpr: 1, canvas, ctx: canvas.getContext("2d"), colors: readGraphColors(), raf: 0,
      ecls: new Uint8Array(idx.m),
    };
    // First-choice select: the root's outgoing routes.
    const sel = $("sg-first");
    sel.textContent = "";
    for (let k = idx.outStart[0]; k < idx.outStart[1]; k++) {
      const e = idx.outList[k];
      const o = document.createElement("option");
      o.value = String(e);
      o.textContent = `${g.edges[e].label} (${g.nodes[idx.eto[e]].value.toFixed(2)})`;
      sel.appendChild(o);
    }
    $("sg-mode").value = "best";
    sel.hidden = true;
    $("sg-dmin").value = "0";
    $("sg-dmin").max = $("sg-dmax").max = String(maxDepth);
    $("sg-dmax").value = String(maxDepth);
    $("sg-side").hidden = true;
    $("sg-tip").hidden = true;
    $("sg-body").hidden = false;
    renderSgChart();
    sizeGraphCanvas();
    recomputeGraph(true, -1);
  }

  // ---- per-step chart (SVG) ----
  function niceMax(v) {
    const p = Math.pow(10, Math.floor(Math.log10(Math.max(1, v))));
    for (const m of [1, 2, 2.5, 5, 10]) if (m * p >= v) return m * p;
    return 10 * p;
  }

  function renderSgChart() {
    const host = $("sg-chart");
    host.textContent = "";
    if (!sg) return;
    const NS = "http://www.w3.org/2000/svg";
    const rows = sg.stats;
    const W = Math.max(260, host.clientWidth || 640), H = 150;
    const padL = 40, padR = 6, padT = 8, padB = 34;
    const plotW = W - padL - padR, plotH = H - padT - padB;
    const top = niceMax(Math.max(1, ...rows.map((r) => r.total)));
    const ticks = [0, top / 2, top];
    const svg = document.createElementNS(NS, "svg");
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    svg.setAttribute("role", "img");
    svg.setAttribute("aria-label", "Positions per step from the decision, split by whether several routes reach them");
    const add = (tag, attrs, text) => {
      const e = document.createElementNS(NS, tag);
      for (const k in attrs) e.setAttribute(k, attrs[k]);
      if (text !== undefined) e.textContent = text;
      svg.appendChild(e);
      return e;
    };
    const yOf = (v) => padT + plotH - (v / top) * plotH;
    for (const t of ticks) {
      add("line", { class: "sg-grid", x1: padL, x2: W - padR, y1: yOf(t), y2: yOf(t) });
      add("text", { x: padL - 6, y: yOf(t) + 4, "text-anchor": "end" }, Math.round(t).toLocaleString("en-US"));
    }
    const slot = plotW / rows.length;
    const bw = Math.max(2, slot - Math.min(4, slot * 0.25));
    const every = Math.max(1, Math.ceil(20 / slot));
    const tip = document.createElement("div");
    tip.className = "sg-tip";
    tip.hidden = true;
    rows.forEach((r, i) => {
      const x = padL + i * slot + (slot - bw) / 2;
      const single = r.total - r.merged;
      const base = padT + plotH;
      const hS = (single / top) * plotH, hM = (r.merged / top) * plotH;
      const gap = single > 0 && r.merged > 0 ? 2 : 0; // 2px between the stacked segments
      if (single > 0) add("rect", { class: "sg-bar-single", x, y: base - hS + gap, width: bw, height: Math.max(1, hS - gap) });
      if (r.merged > 0) add("rect", { class: "sg-bar-multi", x, y: base - hS - Math.max(1, hM), width: bw, height: Math.max(1, hM) });
      if (i % every === 0) add("text", { x: x + bw / 2, y: H - padB + 14, "text-anchor": "middle" }, String(r.depth));
      const hit = add("rect", { class: "sg-hit", x: padL + i * slot, y: padT, width: slot, height: plotH });
      const show = (ev) => {
        const pct = r.total ? Math.round((100 * r.merged) / r.total) : 0;
        tip.textContent = "";
        tip.appendChild(el("b", null, `Step ${r.depth}`));
        tip.appendChild(document.createTextNode(`: ${r.total.toLocaleString("en-US")} positions, ${r.merged.toLocaleString("en-US")} reached by several routes (${pct}%)`));
        tip.hidden = false;
        const box = host.getBoundingClientRect();
        let lx = ev.clientX - box.left + 12;
        if (lx + tip.offsetWidth > box.width) lx = Math.max(0, ev.clientX - box.left - tip.offsetWidth - 12);
        tip.style.left = lx + "px";
        tip.style.top = Math.max(0, ev.clientY - box.top - tip.offsetHeight - 10) + "px";
      };
      hit.addEventListener("mouseenter", show);
      hit.addEventListener("mousemove", show);
      hit.addEventListener("mouseleave", () => (tip.hidden = true));
    });
    add("text", { x: padL + plotW / 2, y: H - 4, "text-anchor": "middle" }, "steps from the decision");
    host.appendChild(svg);
    host.appendChild(tip);
  }

  // ---- layout, view transform ----
  function sizeGraphCanvas() {
    if (!sg) return false;
    const wrap = $("sg-canvas-wrap");
    const w = Math.max(100, wrap.clientWidth), h = Math.max(100, wrap.clientHeight);
    const dpr = window.devicePixelRatio || 1;
    if (w === sg.w && h === sg.h && dpr === sg.dpr) return false;
    sg.w = w;
    sg.h = h;
    sg.dpr = dpr;
    sg.canvas.width = Math.round(w * dpr);
    sg.canvas.height = Math.round(h * dpr);
    return true;
  }

  function computeFit() {
    const s = sg, L = s.lay;
    const ww = Math.max(1, L.cols - 1) * SG_COLW, wh = Math.max(1, L.rows) * SG_ROWH;
    s.fx = Math.max(0.05, (s.w - 2 * SG_PADX) / ww);
    s.fy = Math.max(0.05, (s.h - 2 * SG_PADY) / wh);
  }

  function fitView() {
    const s = sg;
    computeFit();
    s.view.k = 1;
    s.view.tx = SG_PADX;
    s.view.ty = s.h / 2 + SG_PADY / 3;
    s.userMoved = false;
  }

  const sgX = (s, v) => s.view.tx + s.lay.x[v] * SG_COLW * s.fx * s.view.k;
  const sgY = (s, v) => s.view.ty + s.lay.y[v] * SG_ROWH * s.fy * s.view.k;

  function recomputeGraph(fit, anchor) {
    const s = sg;
    const had = s.lay && anchor >= 0 && s.vis[anchor] ? [sgX(s, anchor), sgY(s, anchor)] : null;
    s.vis = SG.visibleSet(s.g, s.idx, {
      mode: s.mode, firstEdge: s.first, minDepth: s.dmin, maxDepth: s.dmax, cap: SG_BEST_CAP,
      extra: s.marks ? s.marks.nodes : undefined,
    });
    s.lay = SG.layout(s.g, s.idx, s.vis);
    computeFit();
    if (fit) fitView();
    else if (had && s.vis[anchor]) {
      s.view.tx += had[0] - sgX(s, anchor);
      s.view.ty += had[1] - sgY(s, anchor);
    }
    s.hover = -1;
    $("sg-tip").hidden = true;
    $("sg-shown").textContent =
      `showing ${s.lay.nodes.length.toLocaleString("en-US")} of ${s.idx.n.toLocaleString("en-US")} positions, ` +
      `${s.lay.edges.length.toLocaleString("en-US")} routes`;
    requestGraphDraw();
  }

  function requestGraphDraw() {
    if (!sg || sg.raf) return;
    sg.raf = requestAnimationFrame(drawGraph);
  }

  function nodeRadius(s, v) {
    const k0 = Math.min(4.5, Math.max(1.3, 1.5 * Math.sqrt(s.view.k)));
    return Math.min(16, k0 * Math.sqrt(Math.max(1, s.g.nodes[v].inEdges)));
  }

  function nodePath(ctx, shape, X, Y, r) {
    if (shape === 0) {
      ctx.moveTo(X + r, Y);
      ctx.arc(X, Y, r, 0, 6.2832);
    } else if (shape === 1) {
      const d = r * 1.25;
      ctx.moveTo(X, Y - d);
      ctx.lineTo(X + d, Y);
      ctx.lineTo(X, Y + d);
      ctx.lineTo(X - d, Y);
      ctx.closePath();
    } else {
      const w = Math.max(1.5, r * 0.7), h = r * 1.5;
      ctx.rect(X - w / 2, Y - h, w, 2 * h);
    }
  }

  const shapeOf = (kind) => (kind === "chance" ? 1 : kind === "leaf" ? 2 : 0);

  function drawGraph() {
    const s = sg;
    if (!s || !s.lay) return;
    s.raf = 0;
    const ctx = s.ctx, C = s.colors, L = s.lay, idx = s.idx, nodes = s.g.nodes;
    const W = s.w, H = s.h;
    ctx.setTransform(s.dpr, 0, 0, s.dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const zx = s.fx * s.view.k, zy = s.fy * s.view.k, tx = s.view.tx, ty = s.view.ty;
    if (!L.nodes.length) {
      ctx.fillStyle = C.dim;
      ctx.font = "13px sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("No positions in this selection.", W / 2, H / 2);
      return;
    }
    // Column guides and step numbers.
    const colPx = SG_COLW * zx;
    const every = Math.max(1, Math.ceil(30 / colPx));
    ctx.font = "10px sans-serif";
    ctx.textAlign = "center";
    ctx.fillStyle = C.dim;
    ctx.strokeStyle = C.dim;
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.1;
    ctx.beginPath();
    for (let c = 0; c < L.cols; c += every) {
      const X = Math.round(tx + c * colPx) + 0.5;
      if (X < 0 || X > W) continue;
      ctx.moveTo(X, 16);
      ctx.lineTo(X, H);
    }
    ctx.stroke();
    ctx.globalAlpha = 1;
    for (let c = 0; c < L.cols; c += every) {
      const X = tx + c * colPx;
      if (X >= 0 && X <= W) ctx.fillText(String(L.minDepth + c), X, 11);
    }
    // Classify edges: 0 normal, 1 best, 2 into the hovered node, 3 route to the pinned node,
    // 4 best continuation from it; 255 = off screen.
    const pinned = s.pin >= 0, em = s.marks ? s.marks.edges : null, hv = s.hover;
    const ecls = s.ecls, ef = idx.efrom, et = idx.eto, ve = L.edges;
    for (let i = 0; i < ve.length; i++) {
      const e = ve[i];
      const X1 = tx + L.x[ef[e]] * colPx, X2 = tx + L.x[et[e]] * colPx;
      if ((X1 < 0 && X2 < 0) || (X1 > W && X2 > W)) { ecls[e] = 255; continue; }
      const Y1 = ty + L.y[ef[e]] * SG_ROWH * zy, Y2 = ty + L.y[et[e]] * SG_ROWH * zy;
      if ((Y1 < 0 && Y2 < 0) || (Y1 > H && Y2 > H)) { ecls[e] = 255; continue; }
      ecls[e] = em && em[e] ? (em[e] === 1 ? 3 : 4) : et[e] === hv ? 2 : s.ebest[e] ? 1 : 0;
    }
    let nBest = 0;
    for (let i = 0; i < ve.length; i++) if (ecls[ve[i]] === 1) nBest++;
    const dense = nBest > 150; // thinner best-line edges when hundreds are drawn
    const styles = [
      [C.dim, 0.75, pinned ? 0.05 : 0.17],
      [C.accent, dense ? 1 : 1.8, pinned ? 0.25 : dense ? 0.5 : 0.85],
      [C.text, 1.6, 0.9],
      [C.accent, 2, 0.95],
      [C.accent2, 2.2, 0.95],
    ];
    for (let cls = 0; cls < 5; cls++) {
      const st = styles[cls];
      ctx.strokeStyle = st[0];
      ctx.lineWidth = st[1];
      ctx.globalAlpha = st[2];
      ctx.beginPath();
      for (let i = 0; i < ve.length; i++) {
        const e = ve[i];
        if (ecls[e] !== cls) continue;
        const X1 = tx + L.x[ef[e]] * colPx, X2 = tx + L.x[et[e]] * colPx;
        const Y1 = ty + L.y[ef[e]] * SG_ROWH * zy, Y2 = ty + L.y[et[e]] * SG_ROWH * zy;
        const mx = (X1 + X2) / 2;
        ctx.moveTo(X1, Y1);
        ctx.bezierCurveTo(mx, Y1, mx, Y2, X2, Y2);
      }
      ctx.stroke();
    }
    // Nodes: neutral then accent (merged), each by shape; off-route nodes dimmed while pinned.
    const nm = s.marks ? s.marks.nodes : null, vn = L.nodes;
    for (let dim = pinned ? 1 : 0; dim >= 0; dim--) {
      ctx.globalAlpha = dim ? 0.3 : 1;
      for (let merged = 0; merged < 2; merged++) {
        ctx.fillStyle = merged ? C.accent : C.muted;
        for (let shape = 0; shape < 3; shape++) {
          ctx.beginPath();
          for (let i = 0; i < vn.length; i++) {
            const v = vn[i], nd = nodes[v];
            if ((nd.inEdges > 1 ? 1 : 0) !== merged || shapeOf(nd.kind) !== shape) continue;
            if ((pinned && !(nm && nm[v]) ? 1 : 0) !== dim) continue;
            const X = tx + L.x[v] * colPx, Y = ty + L.y[v] * SG_ROWH * zy;
            const r = nodeRadius(s, v);
            if (X < -r || X > W + r || Y < -r || Y > H + r) continue;
            nodePath(ctx, shape, X, Y, r);
          }
          ctx.fill();
        }
      }
    }
    ctx.globalAlpha = 1;
    // Rings on the pinned and hovered nodes.
    ctx.strokeStyle = C.text;
    for (const v of [s.pin, s.hover]) {
      if (v < 0 || !s.vis[v]) continue;
      ctx.lineWidth = v === s.pin ? 2.5 : 1.5;
      ctx.beginPath();
      nodePath(ctx, shapeOf(nodes[v].kind), sgX(s, v), sgY(s, v), nodeRadius(s, v) + 2);
      ctx.stroke();
    }
  }

  // ---- interaction ----
  function hitNode(s, mx, my) {
    const L = s.lay, nodes = s.g.nodes;
    let best = -1, bd = Infinity;
    for (let i = 0; i < L.nodes.length; i++) {
      const v = L.nodes[i];
      const dx = sgX(s, v) - mx;
      if (dx > 20 || dx < -20) continue;
      const dy = sgY(s, v) - my;
      const r = Math.max(4, nodeRadius(s, v) * (nodes[v].kind === "chance" ? 1.25 : 1)) + 2;
      const d2 = dx * dx + dy * dy;
      if (d2 <= r * r && d2 < bd) {
        bd = d2;
        best = v;
      }
    }
    return best;
  }

  const pairsItems = (pairs) => pairs.map(([name, count]) => ({ name, count }));

  function fillTip(tip, v) {
    const s = sg, nd = s.g.nodes[v], idx = s.idx;
    tip.textContent = "";
    const head = el("span", "row");
    head.appendChild(el("b", null, SG_KIND[nd.kind] || nd.kind));
    head.appendChild(document.createTextNode(` · step ${nd.depth} · value ${nd.value.toFixed(2)}`));
    if (!nd.exact) head.appendChild(el("span", "approx", " approx"));
    tip.appendChild(head);
    const nIn = idx.inStart[v + 1] - idx.inStart[v];
    tip.appendChild(el("span", "row", `${nd.inEdges} route${nd.inEdges === 1 ? "" : "s"} in` + (nd.inEdges > 1 ? " (several orders reach this position)" : "")));
    for (const [title, key] of [["Hand", "hand"], ["In play", "inPlay"], ["Gained", "gained"]]) {
      if (!nd[key].length) continue;
      const row = el("span", "row");
      row.appendChild(el("span", "dim", title + ": "));
      row.appendChild(cardItems(pairsItems(nd[key])));
      tip.appendChild(row);
    }
    tip.appendChild(el("span", "row", `$${nd.coins} · ${nd.actions} action${nd.actions === 1 ? "" : "s"} · ${nd.buys} buy${nd.buys === 1 ? "" : "s"}`));
    if (nIn) {
      const shown = Math.min(6, nIn);
      tip.appendChild(el("span", "row dim", "Routes in:"));
      for (let k = 0; k < shown; k++) {
        const row = el("span", "row");
        row.appendChild(decorate(s.g.edges[idx.inList[idx.inStart[v] + k]].label));
        tip.appendChild(row);
      }
      if (nIn > shown) tip.appendChild(el("span", "row dim", `… and ${nIn - shown} more`));
    }
  }

  function placeTip(mx, my) {
    const tip = $("sg-tip");
    let lx = mx + 14, ly = my + 14;
    if (lx + tip.offsetWidth > sg.w) lx = Math.max(0, mx - tip.offsetWidth - 14);
    if (ly + tip.offsetHeight > sg.h) ly = Math.max(0, sg.h - tip.offsetHeight - 4);
    tip.style.left = lx + "px";
    tip.style.top = ly + "px";
  }

  function setHover(v, mx, my) {
    const tip = $("sg-tip");
    if (v !== sg.hover) {
      sg.hover = v;
      if (v >= 0) {
        fillTip(tip, v);
        tip.hidden = false;
      } else tip.hidden = true;
      requestGraphDraw();
    }
    if (v >= 0) placeTip(mx, my);
  }

  // Pin a node (or -1 to clear): its routes in and best continuation are forced visible and
  // highlighted; the node stays where it is on screen.
  function setPin(v) {
    const s = sg;
    const anchor = v >= 0 ? v : s.pin;
    s.pin = v;
    s.marks = v >= 0 ? SG.routeMarks(s.g, s.idx, v) : null;
    recomputeGraph(false, anchor);
    renderSgSide();
  }

  function renderSgSide() {
    const s = sg, side = $("sg-side");
    side.textContent = "";
    if (s.pin < 0) {
      side.hidden = true;
      return;
    }
    side.hidden = false;
    const v = s.pin, nd = s.g.nodes[v], idx = s.idx, edges = s.g.edges;
    const head = el("div");
    const clear = el("button", "small-btn", "Clear");
    clear.style.float = "right";
    clear.addEventListener("click", () => setPin(-1));
    head.appendChild(clear);
    head.appendChild(el("b", null, `${SG_KIND[nd.kind]} · step ${nd.depth} · value ${nd.value.toFixed(2)}`));
    if (!nd.exact) head.appendChild(el("span", "approx", " approx"));
    side.appendChild(head);
    const list = (title, items) => {
      side.appendChild(el("h3", null, title));
      const ul = el("ul");
      items.forEach((it) => ul.appendChild(it));
      side.appendChild(ul);
    };
    const lab = (e, extra) => {
      const li = el("li", edges[e].best ? "best" : null);
      li.appendChild(decorate(edges[e].label));
      if (edges[e].prob !== null) li.appendChild(el("span", "v", formatPct(edges[e].prob)));
      if (extra) li.appendChild(el("span", "v", extra));
      return li;
    };
    const ins = [];
    const nIn = idx.inStart[v + 1] - idx.inStart[v];
    for (let k = idx.inStart[v]; k < idx.inStart[v + 1] && ins.length < 25; k++) ins.push(lab(idx.inList[k], ""));
    if (nIn > ins.length) ins.push(el("li", "sg-dim", `… and ${nIn - ins.length} more`));
    if (nIn) list(`Routes in (${nIn})`, ins);
    const outs = [];
    const nOut = idx.outStart[v + 1] - idx.outStart[v];
    for (let k = idx.outStart[v]; k < idx.outStart[v + 1] && outs.length < 30; k++) {
      const e = idx.outList[k];
      outs.push(lab(e, "→ " + s.g.nodes[idx.eto[e]].value.toFixed(2)));
    }
    if (nOut > outs.length) outs.push(el("li", "sg-dim", `… and ${nOut - outs.length} more`));
    if (nOut) list(nd.kind === "chance" ? `Draws (${nOut})` : `Choices (${nOut})`, outs);
    else side.appendChild(el("h3", null, nd.expanded ? "End of the turn." : "Not expanded (graph cap)."));
  }

  function zoomAbout(f, mx, my) {
    const v = sg.view;
    const k = Math.min(80, Math.max(0.5, v.k * f));
    f = k / v.k;
    v.tx = mx - (mx - v.tx) * f;
    v.ty = my - (my - v.ty) * f;
    v.k = k;
    sg.userMoved = true;
    requestGraphDraw();
  }

  function wireSearchGraph() {
    $("btn-search-graph").addEventListener("click", () => openSearchGraph());
    $("btn-close-sg").addEventListener("click", closeSearchGraph);
    const relayout = () => {
      if (!sg) return;
      if (sg.pin >= 0) {
        sg.pin = -1;
        sg.marks = null;
        renderSgSide();
      }
      recomputeGraph(true, -1);
    };
    $("sg-mode").addEventListener("change", () => {
      if (!sg) return;
      sg.mode = $("sg-mode").value;
      $("sg-first").hidden = sg.mode !== "under";
      relayout();
    });
    $("sg-first").addEventListener("change", () => {
      if (!sg) return;
      sg.first = parseInt($("sg-first").value, 10);
      relayout();
    });
    const depthChanged = () => {
      if (!sg) return;
      let a = parseInt($("sg-dmin").value, 10), b = parseInt($("sg-dmax").value, 10);
      if (!Number.isFinite(a)) a = 0;
      if (!Number.isFinite(b)) b = sg.maxDepth;
      a = Math.min(sg.maxDepth, Math.max(0, a));
      b = Math.min(sg.maxDepth, Math.max(0, b));
      if (a > b) [a, b] = [b, a];
      $("sg-dmin").value = String(a);
      $("sg-dmax").value = String(b);
      sg.dmin = a;
      sg.dmax = b;
      relayout();
    };
    $("sg-dmin").addEventListener("change", depthChanged);
    $("sg-dmax").addEventListener("change", depthChanged);
    $("sg-reset").addEventListener("click", () => {
      if (!sg) return;
      fitView();
      requestGraphDraw();
    });
    $("sg-zoom-in").addEventListener("click", () => sg && zoomAbout(1.5, sg.w / 2, sg.h / 2));
    $("sg-zoom-out").addEventListener("click", () => sg && zoomAbout(1 / 1.5, sg.w / 2, sg.h / 2));

    const canvas = $("sg-canvas");
    const pos = (ev) => {
      const r = canvas.getBoundingClientRect();
      return [ev.clientX - r.left, ev.clientY - r.top];
    };
    canvas.addEventListener("pointerdown", (ev) => {
      if (!sg) return;
      const [x, y] = pos(ev);
      sg.drag = { x, y, tx: sg.view.tx, ty: sg.view.ty, moved: false };
      canvas.setPointerCapture(ev.pointerId);
    });
    canvas.addEventListener("pointermove", (ev) => {
      if (!sg) return;
      const [x, y] = pos(ev);
      const d = sg.drag;
      if (d) {
        if (Math.abs(x - d.x) + Math.abs(y - d.y) > 4) {
          d.moved = true;
          canvas.classList.add("dragging");
        }
        if (d.moved) {
          sg.view.tx = d.tx + x - d.x;
          sg.view.ty = d.ty + y - d.y;
          sg.userMoved = true;
          setHover(-1, 0, 0);
          requestGraphDraw();
        }
        return;
      }
      setHover(hitNode(sg, x, y), x, y);
    });
    canvas.addEventListener("pointerup", (ev) => {
      if (!sg || !sg.drag) return;
      const d = sg.drag;
      sg.drag = null;
      canvas.classList.remove("dragging");
      if (!d.moved) {
        const [x, y] = pos(ev);
        const v = hitNode(sg, x, y);
        setPin(v === sg.pin ? -1 : v);
      }
    });
    canvas.addEventListener("pointercancel", () => {
      if (sg) sg.drag = null;
      canvas.classList.remove("dragging");
    });
    canvas.addEventListener("pointerleave", () => {
      if (sg && !sg.drag) setHover(-1, 0, 0);
    });
    canvas.addEventListener(
      "wheel",
      (ev) => {
        if (!sg) return;
        ev.preventDefault();
        const [x, y] = pos(ev);
        zoomAbout(Math.exp(-Math.max(-300, Math.min(300, ev.deltaY)) * 0.0016), x, y);
      },
      { passive: false }
    );
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && sg && sg.pin >= 0) setPin(-1);
    });
    // The canvas follows its wrapper; the view is refitted unless the user has panned or zoomed.
    const resized = () => {
      if (!sg) return;
      renderSgChart();
      if (sizeGraphCanvas()) {
        if (!sg.userMoved) fitView();
        requestGraphDraw();
      }
    };
    window.addEventListener("resize", resized);
    if (typeof ResizeObserver !== "undefined") new ResizeObserver(resized).observe($("sg-canvas-wrap"));
    if (window.matchMedia) {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const onScheme = () => {
        if (!sg) return;
        sg.colors = readGraphColors();
        requestGraphDraw();
      };
      if (mq.addEventListener) mq.addEventListener("change", onScheme);
      else if (mq.addListener) mq.addListener(onScheme);
    }
  }

  function render() {
    if (crashed) return;
    try {
      renderInner();
    } catch (e) {
      if (!reportCrash(e, "render")) throw e;
    }
  }

  function renderInner() {
    seats = api.getSeats();
    seatSearch = api.getSeatSearch();
    lastStateText = api.getStateText();
    const view = api.getView();
    lastView = view;
    renderTurnBar(view);
    renderPlayers(view);
    renderSupply(view);
    renderLandscape(view);
    renderNocturne(view);
    renderTrash(view);
    renderDecision(view);
    renderLog(view);
    // At a turn boundary ("Start turn") Analyze starts the turn and analyzes its first decision.
    $("btn-analyze").disabled = !view.pending || view.gameOver;
    $("btn-search-graph").disabled = !view.pending || view.gameOver;
    if (!stopRunGame) $("btn-run-bots").textContent = view.gameOver ? "New Game" : "Run Game";
    $("btn-undo").disabled = !api.canUndo();
    $("btn-redo").disabled = !api.canRedo();
    if ($("auto-sync").checked) syncTextFromGame();
  }

  let stopRunGame = null; // set while Run Game is playing; stops it after the current turn

  // Run Game plays the whole game without redrawing the page after every turn: it runs turns in
  // ~100 ms slices, yielding between slices so the page stays responsive (and Stop works), and only
  // updates the button with the turn reached ("Stop · turn 14"). The page is drawn once at the end
  // (or when stopped). Fast bots finish in one slice; searching bots show progress.
  async function runGame() {
    const btn = $("btn-run-bots");
    let stopped = false;
    stopRunGame = () => {
      stopped = true;
    };
    btn.textContent = "Stop";
    if ($("analysis-progress") && !$("analysis-progress").hidden) cancelAnalysis();
    renderAnalysis(null);
    let failed = false;
    try {
      let view = api.getView();
      while (!stopped && !crashed && !view.gameOver) {
        const sliceEnd = performance.now() + 100;
        let moved = true;
        do {
          const before = api.stateId();
          api.runToEndOfTurn();
          moved = api.stateId() !== before;
          // (Once the game is over a turn no longer moves the state, which ends the slice.)
        } while (moved && performance.now() < sliceEnd);
        view = api.getView();
        if (!moved) break; // game over, or nothing moved: the render below shows where it is
        btn.textContent = `Stop · turn ${view.turn.number}`;
        await new Promise((r) => setTimeout(r, 0));
      }
    } catch (e) {
      failed = true;
      if (!reportCrash(e, "Run Game")) showToast(String(e.message || e));
    } finally {
      stopRunGame = null;
      render();
      btn.textContent = lastView && lastView.gameOver ? "New Game" : "Run Game";
    }
    if (!crashed && !stopped && !failed) showGameChart();
  }

  function doAction(fn, action) {
    if (crashed) return;
    if (stopRunGame && action !== "Run Game") stopRunGame(); // any other action ends a running game
    if ($("analysis-progress") && !$("analysis-progress").hidden) cancelAnalysis();
    renderAnalysis(null);
    try {
      fn();
      showLoadError("");
      render();
    } catch (e) {
      if (reportCrash(e, action || "action")) return;
      showToast(String(e.message || e));
      render();
    }
  }

  // ---- simulation + chart ---------------------------------------------------------
  //
  // Plays N games from the current position with each seat's controller, split across the worker
  // pool; each worker returns per-turn sums which are added up here. Charts: one line per player
  // (categorical colors in fixed seat order), legend + end labels, crosshair tooltip, table view.

  let simResult = null;
  let simRun = 0;
  let cancelSim = null; // set while a simulation runs; stops it

  function setSimButton(running) {
    const b = $("btn-simulate");
    b.textContent = running ? "Cancel" : "Simulate from here";
    b.classList.toggle("primary", !running);
    b.title = running ? "Stop the simulation" : "Play this many games from the current position with each seat's controller";
  }

  // Same-thread fallback (no Web Workers): run the games in the page's own engine instance.
  // Chart the game just played (Run Game): the same charts, one game.
  function showGameChart() {
    const stats = api.gameStats();
    const seatsNow = api.getSeats();
    simResult = SS.mergeSims([stats], seatsNow.length);
    simResult.names = seatsNow.map((seat, i) => `P${i + 1} ${botNames[seat] || ""}`.trim());
    simResult.single = true;
    $("sim-status").textContent = stats.finished ? "This game (Run Game)" : "This game so far";
    renderSim();
  }

  // simulate() flags: 1 = record the cards each seat owns per turn (only while the Deck composition
  // chart is selected, so plain runs stay fast).
  function simFlags() {
    return $("sim-metric").value === "deck" ? 1 : 0;
  }

  function simulateHere(games) {
    wasm.state_bytes_current();
    const state = resultBytes();
    const seatsNow = api.getSeats();
    const p = wasm.alloc(state.length);
    new Uint8Array(wasm.memory.buffer, p, state.length).set(state);
    const out = JSON.parse(ok(wasm.simulate(p, games, 1000, simFlags(), 0)));
    wasm.dealloc(p, state.length);
    simResult = SS.mergeSims([out], seatsNow.length);
    simResult.names = seatsNow.map((seat, i) => `P${i + 1} ${botNames[seat] || ""}`.trim());
    $("sim-status").textContent = `${simResult.games.toLocaleString()} games (in the page)`;
    renderSim();
  }

  async function simulate() {
    if (typeof Worker === "undefined" || !wasmModule) {
      try {
        simulateHere(Math.max(10, Math.min(20000, parseInt($("sim-games").value, 10) || 1000)));
      } catch (e) {
        if (!reportCrash(e, "Simulate")) $("sim-status").textContent = "Simulation failed: " + String(e.message || e);
      }
      return;
    }
    const run = ++simRun;
    const games = Math.max(10, Math.min(10000000, parseInt($("sim-games").value, 10) || 1000));
    const workers = getPool();
    const t0 = performance.now();
    $("sim-status").textContent = `Starting ${workers.length} workers…`;
    const cancelled = new Promise((_, reject) => {
      cancelSim = () => reject(new Error("cancelled"));
    });
    setSimButton(true);
    try {
      await Promise.race([Promise.all(workers.map((p) => p.ready)), cancelled]);
      wasm.state_bytes_current();
      const state = resultBytes();
      const seatsNow = api.getSeats();
      const players = seatsNow.length;
      const per = Math.ceil(games / workers.length);
      const jobs = [];
      const flags = simFlags();
      let assigned = 0;
      workers.forEach((p, i) => {
        const n = Math.min(per, games - assigned);
        if (n <= 0) return;
        assigned += n;
        jobs.push(new Promise((resolve, reject) => {
          p.worker.onmessage = (e) => {
            const m = e.data;
            if (m.type !== "sim") return;
            if (!m.ok) return reject(new Error(m.out));
            resolve(JSON.parse(m.out));
          };
          p.worker.onerror = (e) => reject(new Error(e.message || "worker failed"));
          // Distinct shuffle seeds per worker; the same inputs give the same results.
          // Worker i plays games [first, first + n) of one seeded sequence, so the merged result
          // does not depend on how many workers there are.
          p.worker.postMessage({ type: "sim", id: i, state, seats: seatsNow, search: api.getSeatSearch(), games: n, seed: 1000, first: assigned - n, flags });
        }));
      });
      let done = 0;
      jobs.forEach((j) => j.then(() => {
        done++;
        if (run === simRun) $("sim-status").textContent = `Simulating: ${done}/${jobs.length} workers done`;
      }, () => {}));
      const parts = await Promise.race([Promise.all(jobs), cancelled]);
      if (run !== simRun) return;
      simResult = SS.mergeSims(parts, players);
      simResult.seconds = (performance.now() - t0) / 1000;
      simResult.names = seatsNow.map((seat, p) => `P${p + 1} ${botNames[seat] || ""}`.trim());
      $("sim-status").textContent = `${simResult.games.toLocaleString()} games in ${simResult.seconds.toFixed(2)} s`;
      renderSim();
    } catch (e) {
      if (e && e.message === "cancelled") {
        $("sim-status").textContent = "Simulation cancelled.";
      } else if (!reportCrash(e, "Simulate")) {
        $("sim-status").textContent = "Simulation failed: " + String(e.message || e);
      }
    } finally {
      if (run === simRun) {
        cancelSim = null;
        setSimButton(false);
      }
    }
  }

  const SS = window.SimStats;

  const seatColor = (p) => `var(--series-${p + 1})`;
  const signed = (v, d) => {
    const text = Math.abs(v).toFixed(d === undefined ? 1 : d);
    return +text === 0 ? text : (v >= 0 ? "+" : "−") + text;
  };
  const pctText = (frac) => (100 * frac).toFixed(1) + "%";

  // Each metric builds a chart description from the merged sums:
  //   lines:     { series: [{name, color, pts: [{x, y, lo?, hi?, sd?}]}], m }
  //   intervals: { rows: [{label, color, p, lo, hi, n}] }
  //   cards:     { cards: [{name, series}] }          (small multiples, one panel per card)
  //   message:   { text }
  const SIM_NOTE_SINGLE = "Needs a simulation of many games, not a single game.";

  function buildChart(metric) {
    const r = simResult, names = r.names;
    const seatSeries = (lists) => lists.map((pts, p) => ({ name: names[p], color: seatColor(p), pts }));
    const BAND = "Line: average over games. Shaded band: ±1 standard deviation across games (about two thirds of games fall inside). ";
    const OWN_TURN = "Turn = that seat's own turn number; each turn averages the games that reached it.";
    const stat = (metric !== "vp" && metric !== "money" && metric !== "buys" && metric !== "winturn") && r.single;
    if (stat) return { kind: "message", text: SIM_NOTE_SINGLE };
    switch (metric) {
      case "winrate": {
        const ws = SS.winStats(r);
        const rows = ws.seats.map((s, p) => ({ label: names[p], color: seatColor(p), p: s.p, lo: s.lo, hi: s.hi, n: r.games }));
        return {
          kind: "intervals",
          title: "Win rate with 95% confidence interval",
          caption: "Dot: share of games won (ties split). Bar: the range the true win rate probably lies in. Dashed line: 50%, an even matchup. Bars that overlap 50% (or each other) are not clearly different at this many games.",
          rows,
        };
      }
      case "first": {
        if (!r.fresh) return { kind: "message", text: "Turn-order results need a simulation from a new game (nobody has played yet)." };
        const ws = SS.winStats(r);
        const rows = [];
        ws.seats.forEach((s, p) => {
          if (s.first) rows.push({ label: `${names[p]} going first`, color: seatColor(p), p: s.first.p, lo: s.first.lo, hi: s.first.hi, n: s.first.n });
          if (s.second) rows.push({ label: `${names[p]} going second`, color: seatColor(p), p: s.second.p, lo: s.second.lo, hi: s.second.hi, n: s.second.n });
        });
        const fa = ws.firstAll;
        return {
          kind: "intervals",
          title: "Win rate by turn order",
          caption: `From a new game the seats are shuffled for every game, so each goes first about half the time. Overall the first player wins ${pctText(fa.p)} (95% CI ${pctText(fa.lo)}–${pctText(fa.hi)}); a seat that wins more when going first than second is helped by the turn order.`,
          rows,
        };
      }
      case "cumwin": {
        const series = seatSeries(SS.cumulativeWins(r));
        return {
          kind: "lines",
          title: "Cumulative win probability by turn",
          caption: "Height at turn t: share of all games that were over, with this seat the winner, by its t-th turn. The line levels off at the seat's overall win rate; a seat that climbs earlier wins faster.",
          series,
          m: { fmt: (v) => v.toFixed(1) + "%", xLabel: "turn" },
        };
      }
      case "length": {
        const d = SS.lengthDistribution(r);
        return {
          kind: "lines",
          title: "How long games last",
          caption: `Share of games by the number of turns each player took. Mean ${d.mean.toFixed(1)}, median ${d.median} turns` + (r.capped ? `; ${r.capped} games hit the turn limit and are left out.` : "."),
          series: [{ name: "games", color: "var(--text-dim)", pts: d.pts }],
          m: { fmt: (v) => v.toFixed(1) + "%", bars: true, noLegend: true, xLabel: "turns in the game", tipHead: (x) => `${x} turns` },
        };
      }
      case "vp":
        return {
          kind: "lines",
          title: "Average VP after each turn",
          caption: BAND + OWN_TURN,
          series: seatSeries(SS.meanSeries(r, "vp", "vpSq")),
          m: { fmt: (v) => v.toFixed(1), band: true, zero: true },
        };
      case "vpdiff": {
        const diffs = SS.vpDifference(r);
        const two = r.players.length === 2;
        const series = diffs.map((d) => ({ name: `${names[d.a]} − ${names[d.b]}`, color: seatColor(two ? d.a : d.b), pts: d.pts }));
        return {
          kind: "lines",
          title: "VP difference over time",
          caption: "Line: average VP of " + (two ? names[0] + " minus " + names[1] : "the first seat minus each other seat") + ", after turns both had played. Above the zero line the first seat is ahead. Shaded band: ±1 standard deviation across games. " + OWN_TURN,
          series,
          m: { fmt: (v) => signed(v), band: true, zero: true, zeroLabels: two ? [{ text: `${names[0]} ahead`, color: seatColor(0) }, { text: `${names[1]} ahead`, color: seatColor(1) }] : null },
        };
      }
      case "money":
        return {
          kind: "lines",
          title: "Average money per turn ($ spent + left over)",
          caption: BAND + "Money is what the seat could spend that turn. " + OWN_TURN,
          series: seatSeries(SS.meanSeries(r, "money", "moneySq", true)),
          m: { fmt: (v) => "$" + v.toFixed(2), band: true },
        };
      case "buys":
        return {
          kind: "lines",
          title: "Average cards bought per turn",
          caption: BAND + OWN_TURN,
          series: seatSeries(SS.meanSeries(r, "buys", "buysSq", true)),
          m: { fmt: (v) => v.toFixed(2), band: true },
        };
      case "provCum":
      case "duchyCum":
      case "colonyCum": {
        const word = { provCum: "Province", duchyCum: "Duchy", colonyCum: "Colony" }[metric];
        const plural = word === "Duchy" ? "Duchies" : word === "Colony" ? "Colonies" : word + "s";
        if (!r.single && !r.players.some((q) => q[metric].some((v) => v > 0))) return { kind: "message", text: `No ${plural} were bought in these games.` };
        return {
          kind: "lines",
          title: `${plural} bought by turn (cumulative average)`,
          caption: `Average number of ${plural} the seat has bought by the end of each of its turns (a finished game keeps its final total). The turn where a line takes off is when that seat starts greening. ` + OWN_TURN,
          series: seatSeries(r.single ? SS.meanSeries(r, metric) : SS.carriedSeries(r, metric, metric.replace("Cum", "End"))),
          m: { fmt: (v) => v.toFixed(2) },
        };
      }
      case "curse": {
        if (!r.players.some((q) => q.curse.some((v) => v > 0))) return { kind: "message", text: "Nobody had a Curse in these games." };
        return {
          kind: "lines",
          title: "Curses owned by turn (average)",
          caption: "Average number of Curses the seat owns after each of its turns, however they came (an opponent's attack, or bought or gained), less any it trashed (a finished game keeps its final count). " + OWN_TURN,
          series: seatSeries(SS.carriedSeries(r, "curse", "curseEnd")),
          m: { fmt: (v) => v.toFixed(2) },
        };
      }
      case "winturn": {
        const series = r.players.map((q, p) => ({ name: names[p], color: seatColor(p), pts: q.winsAt.map((w, i) => ({ x: i + 1, y: (100 * w) / r.games })) }));
        return {
          kind: "lines",
          title: "Games won, by the winner's turn number",
          caption: "Share of all games that this seat won on its t-th turn (the turn the game ended).",
          series: trimToActive(series),
          m: { fmt: (v) => v.toFixed(1) + "%" },
        };
      }
      case "deck": {
        const cards = SS.deckSeries(r, 0.5, 12);
        if (!cards) return { kind: "message", text: "Deck composition was not recorded for this run.", needsDeck: true };
        return {
          kind: "cards",
          title: "Cards owned after each turn",
          caption: `Average number of copies of each card a seat owns after its t-th turn, for cards that average more than 0.5 copies at some point (most-owned first, at most 12). ${OWN_TURN}`,
          cards: cards.map((c) => ({ name: c.name, series: seatSeries(c.perSeat) })),
          series: seatSeries(cards.length ? cards[0].perSeat : r.players.map(() => [])),
        };
      }
    }
    return { kind: "message", text: "Unknown chart." };
  }

  // Win-turn curves are zero for the early turns and after the last win; show only the span where
  // any player wins, plus one turn either side.
  function trimToActive(series) {
    const xs = series.flatMap((s) => s.pts.filter((p) => p.y > 0).map((p) => p.x));
    if (!xs.length) return series;
    const lo = Math.min(...xs) - 1, hi = Math.max(...xs) + 1;
    return series.map((s) => ({ ...s, pts: s.pts.filter((p) => p.x >= lo && p.x <= hi) }));
  }

  // The panel header: each seat's win rate with its 95% interval, and the head-to-head margin.
  function renderSimSummary(r) {
    const summary = $("sim-summary");
    summary.innerHTML = "";
    const seats = el("div", "sim-seats");
    if (r.single) {
      r.players.forEach((q, p) => {
        const item = el("span", "sim-stat");
        const key = el("span", "sim-key");
        key.style.background = seatColor(p);
        item.appendChild(key);
        item.appendChild(document.createTextNode(`${r.names[p]}: `));
        item.appendChild(el("b", null, q.wins > 0 ? "won" : "lost"));
        seats.appendChild(item);
      });
      summary.appendChild(seats);
      summary.appendChild(el("div", "sim-line muted", "One game: pick a chart to see its turn-by-turn record."));
      return;
    }
    const ws = SS.winStats(r);
    ws.seats.forEach((s, p) => {
      const item = el("span", "sim-stat sim-seat");
      const key = el("span", "sim-key");
      key.style.background = seatColor(p);
      item.appendChild(key);
      item.appendChild(el("span", "sim-seat-name", r.names[p]));
      item.appendChild(el("b", "sim-seat-rate", pctText(s.p)));
      item.appendChild(el("span", "muted", `95% CI ${pctText(s.lo)}–${pctText(s.hi)}`));
      seats.appendChild(item);
    });
    summary.appendChild(seats);
    ws.margins.forEach((m) => {
      const A = r.names[m.a], B = r.names[m.b];
      const ci = `95% CI ${signed(m.lo)} to ${signed(m.hi)}`;
      let text;
      if (m.lo > 0) text = `${A} beats ${B} by ${m.d.toFixed(1)} points (${ci})`;
      else if (m.hi < 0) text = `${B} beats ${A} by ${(-m.d).toFixed(1)} points (${ci})`;
      else text = `${A} and ${B} are not clearly different: ${signed(m.d)} points (${ci})`;
      summary.appendChild(el("div", "sim-line", text));
    });
    const finished = r.games - r.capped;
    const bits = [`${r.games.toLocaleString()} games`, `average game ${finished ? (r.lengthSum / finished).toFixed(1) : "-"} turns`];
    if (r.capped) bits.push(`${r.capped} hit the turn limit`);
    if (r.fresh && ws.firstAll) bits.push(`first player wins ${pctText(ws.firstAll.p)}`);
    summary.appendChild(el("div", "sim-line muted", bits.join(" · ")));
  }

  function renderSim() {
    if (!simResult) return;
    const r = simResult;
    renderSimSummary(r);
    const metric = $("sim-metric").value;
    const desc = buildChart(metric);
    const host = $("sim-chart");
    host.innerHTML = "";
    host.appendChild(el("div", "chart-title", desc.title || ""));
    if (desc.kind === "message") {
      host.appendChild(el("div", "muted chart-note", desc.text));
    } else {
      if (desc.kind === "lines" && !desc.m.noLegend) host.appendChild(legendFor(desc.series));
      if (desc.kind === "cards") host.appendChild(legendFor(desc.series));
      host.appendChild(el("div", "chart-caption", desc.caption));
      if (desc.kind === "lines") drawLineChart(host, desc.series, desc.m);
      else if (desc.kind === "intervals") drawIntervals(host, desc.rows);
      else if (desc.kind === "cards") {
        const grid = el("div", "chart-grid");
        host.appendChild(grid);
        desc.cards.forEach((c) => {
          const cell = el("div", "chart-cell");
          cell.appendChild(el("div", "chart-cell-title", c.name));
          grid.appendChild(cell);
          drawLineChart(cell, c.series, { fmt: (v) => v.toFixed(2), tickFmt: (v) => String(+v.toFixed(2)), compact: true });
        });
        if (!desc.cards.length) host.appendChild(el("div", "muted chart-note", "No card averages more than 0.5 copies."));
      }
    }
    renderSimTable(desc);
  }

  function legendFor(series) {
    const legend = el("div", "chart-legend");
    series.forEach((s) => {
      const item = el("span", "legend-item");
      const key = el("span", "legend-line");
      key.style.background = s.color;
      item.appendChild(key);
      item.appendChild(document.createTextNode(s.name));
      legend.appendChild(item);
    });
    return legend;
  }

  // The numbers behind the chart, as a table (scrolls inside its box on narrow screens).
  function renderSimTable(desc) {
    const box = $("sim-table");
    box.hidden = !$("sim-table-toggle").checked;
    box.innerHTML = "";
    if (desc.kind === "message") return;
    const table = el("table", "sim-table");
    const head = el("tr");
    const rowsOut = [];
    if (desc.kind === "intervals") {
      ["", "win rate", "95% low", "95% high", "games"].forEach((h) => head.appendChild(el("th", null, h)));
      desc.rows.forEach((row) => rowsOut.push([row.label, pctText(row.p), pctText(row.lo), pctText(row.hi), String(Math.round(row.n))]));
    } else {
      let cols = desc.series, fmt = desc.m ? desc.m.fmt : (v) => v.toFixed(2);
      if (desc.kind === "cards") {
        fmt = (v) => v.toFixed(2);
        cols = desc.cards.flatMap((c) => c.series.map((s) => ({ name: `${c.name} · ${s.name}`, pts: s.pts })));
      }
      const xs = [...new Set(cols.flatMap((s) => s.pts.map((p) => p.x)))].sort((a, b) => a - b);
      head.appendChild(el("th", null, desc.m && desc.m.bars ? "Turns" : "Turn"));
      cols.forEach((s) => head.appendChild(el("th", null, s.name)));
      xs.forEach((x) => {
        const row = [String(x)];
        cols.forEach((s) => {
          const pt = s.pts.find((p) => p.x === x);
          row.push(pt ? fmt(pt.y) + (pt.sd !== undefined && desc.kind === "lines" ? ` ± ${pt.sd.toFixed(1)}` : "") : "");
        });
        rowsOut.push(row);
      });
    }
    table.appendChild(head);
    rowsOut.forEach((cells) => {
      const tr = el("tr");
      cells.forEach((c) => tr.appendChild(el("td", null, c)));
      table.appendChild(tr);
    });
    const scroller = el("div", "sim-table-scroll");
    scroller.appendChild(table);
    box.appendChild(scroller);
  }

  const SVGNS = "http://www.w3.org/2000/svg";
  function svg(tag, attrs, parent) {
    const e = document.createElementNS(SVGNS, tag);
    for (const k in attrs) e.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(e);
    return e;
  }

  // Round tick values covering [lo, hi].
  function niceTicks(lo, hi, count) {
    if (!(hi > lo)) hi = lo + 1;
    const raw = (hi - lo) / count;
    const mag = Math.pow(10, Math.floor(Math.log10(raw)));
    const step = [1, 2, 2.5, 5, 10].map((f) => f * mag).find((st) => raw <= st * 1.0000001) || 10 * mag;
    const ticks = [];
    const first = Math.floor(lo / step + 1e-9) * step;
    for (let v = first; v < hi + step * 0.999; v += step) {
      ticks.push(+v.toFixed(10));
      if (v >= hi - step * 1e-9) break;
    }
    return ticks;
  }

  // One chart of lines (or bars) over x. Series points: {x, y, lo?, hi?, sd?}. Options (m):
  // fmt, band (draw lo..hi), zero (y axis may go below 0, with a zero line), bars, compact
  // (small multiple: no end labels), noLegend, xLabel, zeroLabels, tipHead.
  function drawLineChart(host, series, m) {
    const all = series.flatMap((s) => s.pts);
    if (!all.length) {
      host.appendChild(el("div", "muted chart-note", "No data."));
      return;
    }
    const compact = !!m.compact;
    const W = Math.max(compact ? 200 : 300, host.clientWidth || 800), H = compact ? 150 : 280;
    const pad = compact ? { l: 34, r: 8, t: 8, b: 24 } : { l: 44, r: 112, t: 10, b: 34 };
    if (!compact && W < 520) pad.r = 70; // short end labels on phones
    if (m.bars) pad.r = 16;
    const xs = all.map((p) => p.x);
    const xLo = Math.min(...xs) - (m.bars ? 0.5 : 0), xHi = Math.max(...xs) + (m.bars ? 0.5 : 0);
    const vals = all.flatMap((p) => (m.band && p.lo !== undefined ? [p.lo, p.hi] : [p.y]));
    const lowest = Math.min(...vals), highest = Math.max(...vals);
    const ticks = niceTicks(lowest < 0 || m.zero ? Math.min(lowest, 0) : 0, Math.max(highest, 0), compact ? 3 : 5);
    const yLo = ticks[0], yHi = ticks[ticks.length - 1];
    const X = (x) => pad.l + ((x - xLo) / Math.max(1e-9, xHi - xLo)) * (W - pad.l - pad.r);
    const Y = (y) => H - pad.b - ((y - yLo) / Math.max(1e-9, yHi - yLo)) * (H - pad.t - pad.b);
    const root = svg("svg", { width: W, height: H, viewBox: `0 0 ${W} ${H}`, class: "chart-svg", role: "img" }, null);
    // Recessive grid + axes.
    ticks.forEach((t) => {
      svg("line", { x1: pad.l, x2: W - pad.r, y1: Y(t), y2: Y(t), class: t === 0 && yLo < 0 ? "grid zero" : "grid" }, root);
      const lab = svg("text", { x: pad.l - 6, y: Y(t) + 4, class: "tick", "text-anchor": "end" }, root);
      lab.textContent = m.tickFmt ? m.tickFmt(t) : m.fmt(t).replace(/\.0+(?=%|$)/, "");
    });
    const xFirst = Math.ceil(xLo), xLast = Math.floor(xHi);
    const xStep = Math.max(1, Math.ceil((xLast - xFirst) / (compact ? 6 : 12)));
    for (let x = xFirst; x <= xLast; x += xStep) {
      const lab = svg("text", { x: X(x), y: H - pad.b + 15, class: "tick", "text-anchor": "middle" }, root);
      lab.textContent = String(x);
    }
    if (!compact) {
      const xl = svg("text", { x: (pad.l + W - pad.r) / 2, y: H - 3, class: "axis-label", "text-anchor": "middle" }, root);
      xl.textContent = m.xLabel || "turn";
    }
    if (m.zeroLabels) {
      m.zeroLabels.forEach((zl, i) => {
        const t = svg("text", { x: pad.l + 6, y: i === 0 ? pad.t + 12 : H - pad.b - 6, class: "zero-label", style: `fill:${zl.color}` }, root);
        t.textContent = (i === 0 ? "↑ " : "↓ ") + zl.text;
      });
    }
    const ends = [];
    series.forEach((s) => {
      if (!s.pts.length) return;
      if (m.bars) {
        const w = Math.max(1, ((W - pad.l - pad.r) / (xHi - xLo)) * 0.8);
        s.pts.forEach((p) => {
          svg("rect", { x: X(p.x) - w / 2, width: w, y: Y(p.y), height: Math.max(0, Y(yLo) - Y(p.y)), class: "bar", style: `fill:${s.color}` }, root);
        });
        return;
      }
      if (m.band && s.pts[0].lo !== undefined) {
        const top = s.pts.map((p, i) => `${i ? "L" : "M"}${X(p.x).toFixed(1)},${Y(p.hi).toFixed(1)}`).join("");
        const bottom = s.pts.slice().reverse().map((p) => `L${X(p.x).toFixed(1)},${Y(p.lo).toFixed(1)}`).join("");
        svg("path", { d: top + bottom + "Z", class: "series-band", style: `fill:${s.color}` }, root);
      }
      const d = s.pts.map((p, i) => `${i ? "L" : "M"}${X(p.x).toFixed(1)},${Y(p.y).toFixed(1)}`).join("");
      svg("path", { d, class: "series-line", style: `stroke:${s.color}` }, root);
      const last = s.pts[s.pts.length - 1];
      ends.push({ s, x: X(last.x), y: Y(last.y) });
    });
    // End labels (seat name) stay above the x axis and stack upward when they would overlap.
    if (!compact && !m.bars) {
      ends.sort((a, b) => b.y - a.y);
      ends.forEach((e, i) => {
        const floor = i === 0 ? H - pad.b - 8 : ends[i - 1].y - 14;
        e.y = Math.min(e.y, floor);
      });
      const room = Math.floor((pad.r - 12) / 6.2);
      ends.forEach((e) => {
        svg("circle", { cx: e.x, cy: Y(e.s.pts[e.s.pts.length - 1].y), r: 4, class: "end-dot", style: `fill:${e.s.color}` }, root);
        const t = svg("text", { x: e.x + 8, y: e.y + 4, class: "end-label" }, root);
        t.textContent = e.s.name.length > room ? e.s.name.slice(0, Math.max(2, room - 1)) + "…" : e.s.name;
      });
    }
    // Crosshair + tooltip listing every series at the hovered x.
    const cross = svg("line", { y1: pad.t, y2: H - pad.b, class: "crosshair", visibility: "hidden" }, root);
    const tip = el("div", "chart-tip");
    tip.hidden = true;
    const hit = svg("rect", { x: pad.l, y: pad.t, width: W - pad.l - pad.r, height: H - pad.t - pad.b, fill: "transparent", tabindex: 0 }, root);
    const show = (clientX) => {
      const box = root.getBoundingClientRect();
      const px = ((clientX - box.left) / box.width) * W;
      const lo = Math.ceil(xLo), hi = Math.floor(xHi);
      const x = Math.max(lo, Math.min(hi, Math.round(xLo + ((px - pad.l) / (W - pad.l - pad.r)) * (xHi - xLo))));
      cross.setAttribute("x1", X(x));
      cross.setAttribute("x2", X(x));
      cross.setAttribute("visibility", "visible");
      tip.innerHTML = "";
      tip.appendChild(el("div", "tip-head", m.tipHead ? m.tipHead(x) : `Turn ${x}`));
      series.forEach((s) => {
        const pt = s.pts.find((p) => p.x === x);
        const row = el("div", "tip-row");
        const key = el("span", "legend-line");
        key.style.background = s.color;
        row.appendChild(key);
        row.appendChild(el("b", null, pt ? m.fmt(pt.y) + (pt.sd !== undefined && m.band ? ` ± ${pt.sd.toFixed(1)}` : "") : "–"));
        row.appendChild(document.createTextNode(m.noLegend ? "" : " " + s.name));
        tip.appendChild(row);
      });
      tip.hidden = false;
      const left = (X(x) / W) * box.width;
      tip.style.left = `${Math.max(0, Math.min(left + 12, box.width - (compact ? 150 : 230)))}px`;
    };
    hit.addEventListener("pointermove", (e) => show(e.clientX));
    hit.addEventListener("pointerleave", () => {
      cross.setAttribute("visibility", "hidden");
      tip.hidden = true;
    });
    const wrap = el("div", "chart-wrap");
    wrap.appendChild(root);
    wrap.appendChild(tip);
    host.appendChild(wrap);
  }

  // Dot-and-whisker chart of win rates: rows {label, color, p, lo, hi, n} (fractions), a dashed
  // line at 50%, the value written above each whisker.
  function drawIntervals(host, rows) {
    const W = Math.max(280, host.clientWidth || 700), rowH = 58;
    const pad = { l: 14, r: 14, t: 6, b: 24 };
    const H = pad.t + rows.length * rowH + pad.b;
    let lo = Math.min(50, ...rows.map((r) => 100 * r.lo)) - 3, hi = Math.max(50, ...rows.map((r) => 100 * r.hi)) + 3;
    lo = Math.max(0, Math.floor(lo / 10) * 10);
    hi = Math.min(100, Math.ceil(hi / 10) * 10);
    const X = (v) => pad.l + ((v - lo) / (hi - lo)) * (W - pad.l - pad.r);
    const root = svg("svg", { width: W, height: H, viewBox: `0 0 ${W} ${H}`, class: "chart-svg", role: "img" }, null);
    const step = hi - lo <= 40 ? 5 : 10;
    for (let v = lo; v <= hi; v += step) {
      svg("line", { x1: X(v), x2: X(v), y1: pad.t, y2: H - pad.b, class: v === 50 ? "grid ref" : "grid" }, root);
      const lab = svg("text", { x: X(v), y: H - pad.b + 15, class: "tick", "text-anchor": "middle" }, root);
      lab.textContent = v + "%";
    }
    rows.forEach((row, i) => {
      const y0 = pad.t + i * rowH;
      const g = svg("g", {}, root);
      svg("title", {}, g).textContent = `${row.label}: ${pctText(row.p)} (95% CI ${pctText(row.lo)}–${pctText(row.hi)}), ${Math.round(row.n).toLocaleString()} games`;
      const name = svg("text", { x: pad.l, y: y0 + 14, class: "row-label" }, g);
      name.textContent = row.label;
      const val = svg("text", { x: pad.l, y: y0 + 29, class: "row-value" }, g);
      val.textContent = pctText(row.p);
      const ci = svg("tspan", { class: "row-ci", dx: 8 }, val);
      ci.textContent = `95% CI ${pctText(row.lo)}–${pctText(row.hi)}`;
      const y = y0 + 44;
      svg("line", { x1: X(100 * row.lo), x2: X(100 * row.hi), y1: y, y2: y, class: "whisker", style: `stroke:${row.color}` }, g);
      [row.lo, row.hi].forEach((v) => svg("line", { x1: X(100 * v), x2: X(100 * v), y1: y - 5, y2: y + 5, class: "whisker", style: `stroke:${row.color}` }, g));
      svg("circle", { cx: X(100 * row.p), cy: y, r: 5, class: "end-dot", style: `fill:${row.color}` }, g);
    });
    const wrap = el("div", "chart-wrap");
    wrap.appendChild(root);
    host.appendChild(wrap);
  }

  // ---- wiring ---------------------------------------------------------------

  function refreshStrategySeatChoices() {
    const sel = $("strategy-seat");
    const n = lastView ? lastView.players.length : parseInt($("ng-players").value, 10) || 2;
    const prev = sel.value;
    sel.innerHTML = "";
    const none = el("option", null, "(don't seat)");
    none.value = "";
    sel.appendChild(none);
    for (let p = 0; p < n; p++) {
      const o = el("option", null, `Player ${p + 1}`);
      o.value = String(p);
      sel.appendChild(o);
    }
    sel.value = prev !== "" && Number(prev) < n ? prev : n > 1 ? "1" : "";
  }

  function wireStrategyLoader() {
    const msg = (text, isError) => {
      const m = $("strategy-msg");
      m.textContent = text;
      m.className = "inline" + (isError ? " error" : "");
    };
    $("btn-load-strategy").addEventListener("click", () => {
      const panel = $("strategy-loader");
      panel.hidden = !panel.hidden;
      if (!panel.hidden) refreshStrategySeatChoices();
    });
    $("strategy-file").addEventListener("change", async () => {
      const f = $("strategy-file").files[0];
      if (f) $("strategy-text").value = await f.text();
    });
    $("btn-add-strategy").addEventListener("click", () => {
      const toml = $("strategy-text").value;
      if (!toml.trim()) return msg("Paste a strategy or choose a file first.", true);
      let r;
      try {
        r = addStrategy(toml);
      } catch (e) {
        return msg(String(e.message || e), true);
      }
      const seat = $("strategy-seat").value;
      const untouched = !api.canUndo() && lastView && lastView.turn.number === 1;
      doAction(() => {
        if (seat !== "") {
          api.setSeat(parseInt(seat, 10), r.id);
          if (syncKingdomFromSeats() && untouched) startNewGame();
        }
      });
      msg(`${r.replaced ? "Replaced" : "Added"} \u201c${r.name}\u201d` + (seat !== "" ? ` as Player ${Number(seat) + 1}.` : "."), false);
    });
    $("btn-forget-strategies").addEventListener("click", () => {
      customStrategies = [];
      persistStrategies();
      location.reload();
    });
  }

  function wire() {
    wireStrategyLoader();
    setCheckboxes().forEach((b) => b.addEventListener("change", onSetsChanged));
    $("btn-lab-kingdom-clear").addEventListener("click", () => {
      labKingdom = null;
      $("btn-new-game").click();
    });
    $("btn-new-game").addEventListener("click", () => {
      try {
        startNewGame();
        render();
      } catch (e) {
        if (!reportCrash(e, "New game")) showNewGameError(String(e.message || e));
      }
    });

    $("btn-load").addEventListener("click", () => {
      try {
        cancelAnalysis();
        api.loadState($("state-text").value);
        showLoadNotice(api.getView());
        showLoadError("");
        render();
      } catch (e) {
        if (!reportCrash(e, "Load state")) showLoadError(String(e.message || e));
      }
    });

    $("btn-sync").addEventListener("click", () => syncTextFromGame());

    // Auto-step plays the displayed analysis's top choice when it's for this exact position
    // (for strategy seats that's the rules' pick), otherwise the seat's controller decides.
    $("btn-auto").addEventListener("click", () => {
      const a = shownAnalysis;
      const top = a && a.options && a.options[0];
      const current = a && (a.stateId >>> 0) === api.stateId() && top && top.index >= 0;
      doAction(() => (current ? api.choose(top.index) : api.stepAuto()), "Auto-step");
    });
    $("btn-run-turn").addEventListener("click", () => doAction(() => api.runToEndOfTurn(), "Run to end of turn"));
    // Run Game: play the game out with every seat's controller, then chart that game.
    // Run Game plays one turn at a time and yields between turns, so the page stays responsive
    // (bots that search their turns can take a while) and the button turns into Stop.
    // Once the game is over the button starts a new one (same as New game).
    $("btn-run-bots").addEventListener("click", () => {
      if (stopRunGame) stopRunGame();
      else if (lastView && lastView.gameOver) $("btn-new-game").click();
      else runGame();
    });
    // The Simulate button turns into Cancel while a simulation runs.
    $("btn-simulate").addEventListener("click", () => {
      if (cancelSim) {
        const stop = cancelSim;
        cancelSim = null;
        simRun++;
        killPool(); // workers are busy mid-batch; stop them and start fresh next time
        stop();
        setSimButton(false);
      } else {
        simulate();
      }
    });
    $("sim-metric").addEventListener("change", () => {
      // Deck composition is only recorded when asked for: run again with it if this run lacks it.
      if ($("sim-metric").value === "deck" && simResult && !simResult.single && !simResult.players.every((q) => q.owned) && !cancelSim) {
        simulate();
        return;
      }
      renderSim();
    });
    $("sim-table-toggle").addEventListener("change", () => renderSim());
    window.addEventListener("resize", () => renderSim());
    $("btn-analyze").addEventListener("click", () => {
      if (!resumeIfPaused("Continue")) return;
      if (typeof Worker === "undefined" || !wasmModule) {
        try {
          renderAnalysis(api.analyze());
        } catch (e) {
          showToast(String(e.message || e));
        }
        return;
      }
      $("btn-analyze").disabled = true;
      analyzeParallel()
        .catch((e) => {
          if (reportCrash(e, "Analyze decision")) return;
          showToast("Analysis failed: " + String(e.message || e));
          cancelAnalysis();
        })
        .finally(() => ($("btn-analyze").disabled = false));
    });
    $("btn-close-analysis").addEventListener("click", () => {
      cancelAnalysis();
      $("btn-analyze").disabled = false;
    });
    $("btn-cancel-analysis").addEventListener("click", () => {
      cancelAnalysis();
      $("btn-analyze").disabled = false;
    });
    wireSearchGraph();
    $("btn-undo").addEventListener("click", () => doAction(() => api.undo(), "Undo"));
    $("btn-redo").addEventListener("click", () => doAction(() => api.redo(), "Redo"));

    window.addEventListener("keydown", (e) => {
      const tag = document.activeElement && document.activeElement.tagName;
      if (tag === "TEXTAREA" || tag === "INPUT") return;
      if (e.key >= "1" && e.key <= "9") {
        const i = e.key.charCodeAt(0) - "1".charCodeAt(0);
        if (lastView && lastView.pending && i < lastView.pending.choices.length) {
          doAction(() => {
            const idx = lastView.pending.choices[i].index;
            if (idx < 0) api.resume();
            else api.choose(idx);
          });
        }
      } else if (e.key === "z" || e.key === "u") {
        if (!$("btn-undo").disabled) doAction(() => api.undo());
      } else if (e.key === "y" || e.key === "r") {
        if (!$("btn-redo").disabled) doAction(() => api.redo());
      } else if (e.key === "a") {
        $("btn-analyze").click();
      } else if (e.key === " ") {
        e.preventDefault();
        doAction(() => api.stepAuto(), "Auto-step");
      }
    });
  }

  async function main() {
    const bytes = base64ToBytes(WASM_BASE64);
    const { module, instance } = await WebAssembly.instantiate(bytes, {});
    wasmModule = module;
    wasm = instance.exports;
    wasm.init();
    for (const c of storedStrategies()) {
      try {
        addStrategy(c.toml, false);
      } catch (e) {
        /* a stored file that no longer parses is dropped */
      }
    }
    persistStrategies();
    botNames = api.listBots();
    initCards();
    loadCardSetsSelection();
    wire();
    // index.html#strategy=<base64 TOML> (e.g. from the Strategy Lab): load it and seat it as
    // Player 2, so it faces the default Player 1 (Double Witch).
    if (location.hash.startsWith("#strategy=")) {
      try {
        const toml = decodeURIComponent(escape(atob(decodeURIComponent(location.hash.slice("#strategy=".length)))));
        const r = addStrategy(toml);
        api.setSeat(1, r.id);
        history.replaceState(null, "", location.pathname + location.search);
      } catch (e) {
        showNewGameError("Could not load the linked strategy: " + String(e.message || e));
      }
    }
    // index.html#lab=<base64 JSON>: the Strategy Lab's kingdom and strategies (see applyLabPayload).
    let labError = "";
    if (location.hash.startsWith("#lab=")) {
      try {
        applyLabPayload(JSON.parse(decodeURIComponent(escape(atob(decodeURIComponent(location.hash.slice("#lab=".length)))))));
        history.replaceState(null, "", location.pathname + location.search);
      } catch (e) {
        labError = "Could not load the Strategy Lab link: " + String(e.message || e);
        labKingdom = null;
      }
    }
    // Start with a kingdom matching the default seats' rules.
    try {
      startNewGame();
      if (labError) showNewGameError(labError);
    } catch (e) {
      labError = labError || String(e.message || e);
      // The lab's kingdom cannot take these strategies: say so, and start an ordinary game so the page still works.
      labKingdom = null;
      try {
        startNewGame();
      } catch (e2) {
        $("state-text").value = api.getStartText();
      }
      showNewGameError(labError);
    }
    render();
    // Test hook: open index.html#selftest-analyze to run a parallel analysis on load.
    if (location.hash === "#selftest-analyze") $("btn-analyze").click();
    // Test hook: open index.html#selftest-sim to run a simulation on load.
    if (location.hash === "#selftest-sim") simulate();
    // Test hook: play the whole game with Run Game (screenshots show the finished game and its chart).
    if (location.hash === "#selftest-rungame") $("btn-run-bots").click();
    // Test hook for the README screenshot: a base-game position, analyzed, plus a simulation chart.
    if (location.hash === "#selftest-readme") {
      $("state-text").value = [
        "players: 2",
        "kingdom: Village, Smithy, Market, Witch, Throne Room, Remodel, Cellar, Militia, Moat, Laboratory",
        "turn: 9  player: 1  phase: action  actions: 1  buys: 1  coins: 0",
        "",
        "[player 1]",
        "hand: Village, Smithy, Witch, Copper, Silver",
        "deck: 4 Copper, 2 Estate, Gold, Silver, Curse",
        "discard: 2 Copper, Estate",
        "",
        "[player 2]",
        "hand: 3 Copper, Silver, Estate",
        "deck: 3 Copper, 2 Estate, Silver, Gold",
        "discard: Curse, Copper",
        "",
      ].join("\n");
      $("btn-load").click();
      renderAnalysis(api.analyze());
      $("sim-metric").value = "vp";
      simulateHere(1000);
    }
    if (location.hash === "#selftest-simcancel") {
      $("sim-games").value = "10000000";
      simulate();
      setTimeout(() => $("btn-simulate").click(), 50);
    }
    // Test hook: index.html#selftest-load=<base64 state text> loads that state and analyzes it
    // ("&noanalyze": just loads it, leaving it paused as Load State does).
    if (location.hash.startsWith("#selftest-load=")) {
      $("state-text").value = decodeURIComponent(escape(atob(location.hash.slice("#selftest-load=".length).split("&")[0])));
      $("btn-load").click();
      if (!location.hash.includes("&noanalyze") && resumeIfPaused("Continue")) {
        renderAnalysis(api.analyze()); // same-thread, so headless screenshots see the result
      }
    }
    // Test hook: index.html#selftest-graph=<base64 state text> loads that state, analyzes it and
    // opens the search graph (a bare #selftest-graph opens it on the current position).
    if (location.hash.startsWith("#selftest-graph")) {
      if (location.hash.startsWith("#selftest-graph=")) {
        $("state-text").value = decodeURIComponent(escape(atob(location.hash.slice("#selftest-graph=".length).split("&")[0])));
        $("btn-load").click();
      }
      if (resumeIfPaused("Continue")) renderAnalysis(api.analyze());
      // Optional "&mode=all" / "&pin=<min depth>" (pins the first merged node at least that deep).
      const opts = new URLSearchParams(location.hash.split("&").slice(1).join("&"));
      openSearchGraph().then(() => {
        if (!sg) return;
        if (opts.get("mode")) {
          $("sg-mode").value = opts.get("mode");
          $("sg-mode").dispatchEvent(new Event("change"));
        }
        if (opts.get("pin")) {
          const d = parseInt(opts.get("pin"), 10);
          const v = sg.g.nodes.findIndex((n, i) => i > 0 && n.depth >= d && n.inEdges > 2 && sg.vis[i]);
          if (v >= 0) setPin(v);
        }
      });
    }
    // Test hook: open index.html#selftest-chart-<metric> to simulate in the page and draw the chart.
    // Optional "&seats=4,2" (bot ids per seat, see Show/seat lists), "&games=1000", "&table".
    if (location.hash.startsWith("#selftest-chart")) {
      const [head, ...rest] = location.hash.split("&");
      const opts = new URLSearchParams(rest.join("&"));
      $("sim-metric").value = head.split("-")[2] || "vp";
      if (opts.get("seats")) opts.get("seats").split(",").forEach((id, p) => api.setSeat(p, parseInt(id, 10)));
      if (opts.has("table")) $("sim-table-toggle").checked = true;
      if (opts.has("only")) {
        // Screenshots: show just the Simulation panel.
        document.querySelector("header").style.display = "none";
        $("text-panel").style.display = "none";
        document.querySelector("main").style.display = "block";
        [...$("game-panel").children].forEach((c) => { if (c.id !== "sim-panel") c.style.display = "none"; });
      }
      render();
      simulateHere(parseInt(opts.get("games") || "1000", 10));
    }
    if (location.hash === "#selftest-analyze2") { api.resume(); render(); $("btn-analyze").click(); }
    // Test hook: open index.html#selftest-crash to exercise the crash panel.
    if (location.hash === "#selftest-crash") doAction(() => wasm.debug_panic(), "self-test crash");
  }

  main().catch((e) => {
    document.body.innerHTML =
      '<pre style="color:#a33;padding:20px;white-space:pre-wrap">Failed to start: ' +
      (e && e.stack ? e.stack : e) +
      "</pre>";
  });
})();
