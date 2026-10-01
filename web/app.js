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
  let cardRegex = null;

  function initCards() {
    for (const c of JSON.parse(ok(wasm.card_info()))) {
      const t = c.types;
      // Duration cards are orange and Reactions blue, whatever else they are (Astrolabe,
      // Pirate, Moat, Diplomat...).
      const primary = t.includes("curse")
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
      // Treasure-Victory (Harem) and Action-Victory (Mill, Nobles, Island) get a split chip.
      const has = (x) => t.includes(x);
      const dual = has("treasure") && has("victory")
        ? " dual-treasure-victory"
        : has("action") && has("victory")
        ? " dual-action-victory"
        : "";
      cardClass.set(c.name, "card " + primary + dual + (t.includes("attack") ? " attack" : ""));
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
      ? me.hand.filter((c) => /action|reaction/.test(cardClass.get(c.name) || "")).map((c) => c.name)
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
    if (d.arg && d.arg !== "used") text += ` Set aside: ${d.arg}.`;
    else if (d.arg === "used") text += " (Already used this turn.)";
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
      panel.appendChild(el("div", "zone", `Turns taken: ${p.turnsTaken}`));

      row.appendChild(panel);
    }
  }

  function renderSupply(view) {
    const tbody = $("supply-table").querySelector("tbody");
    tbody.innerHTML = "";
    tbody.style.setProperty("--supply-rows", Math.ceil(view.supply.length / 2));
    for (const c of view.supply) {
      const tr = document.createElement("tr");
      const tdName = document.createElement("td");
      tdName.appendChild(cardChip(c.name));
      const tdCount = document.createElement("td");
      tdCount.textContent = c.count;
      if (c.count === 0) tdCount.className = "count-0";
      tr.appendChild(tdName);
      tr.appendChild(tdCount);
      tbody.appendChild(tr);
    }
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
        const b = m.state, sp = w.alloc(b.length);
        new Uint8Array(w.memory.buffer, sp, b.length).set(b);
        let okSim, out;
        try {
          okSim = w.simulate(sp, m.games, m.seed);
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
  // Bit 0 = Base, bit 1 = Intrigue, bit 2 = Seaside, bit 3 = Prosperity (matches `padded_kingdom`).
  const SET_BITS = { base: 1, intrigue: 2, seaside: 4, prosperity: 8 };

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

  function startNewGame() {
    if (stopRunGame) stopRunGame();
    const players = parseInt($("ng-players").value, 10) || 2;
    const seed = parseInt($("ng-seed").value, 10) || 0;
    cancelAnalysis();
    gameRequiredKingdom = seatsKingdom();
    const setsMask = cardSetsMask() || (SET_BITS.base | SET_BITS.intrigue | SET_BITS.seaside | SET_BITS.prosperity);
    const kingdom = api.paddedKingdom(players, setsMask, seed).kingdom;
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

  async function openSearchGraph() {
    if (crashed) return;
    if (lastView && lastView.pending && lastView.pending.paused) {
      doAction(() => api.resume(), "Start turn");
      if (crashed || !lastView.pending || lastView.pending.paused || lastView.gameOver) return;
    }
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
    lastStateText = api.getStateText();
    const view = api.getView();
    lastView = view;
    renderTurnBar(view);
    renderPlayers(view);
    renderSupply(view);
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

  async function runGame() {
    const btn = $("btn-run-bots");
    let stopped = false;
    stopRunGame = () => {
      stopped = true;
    };
    btn.textContent = "Stop";
    try {
      while (!stopped && !crashed && lastView && !lastView.gameOver) {
        const before = api.stateId();
        doAction(() => api.runToEndOfTurn(), "Run Game");
        if (api.stateId() === before) break; // nothing moved (error shown by doAction)
        await new Promise((r) => setTimeout(r, 0));
      }
    } finally {
      stopRunGame = null;
      btn.textContent = lastView && lastView.gameOver ? "New Game" : "Run Game";
    }
    if (!crashed && !stopped) showGameChart();
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
    simResult = mergeSims([stats], seatsNow.length);
    simResult.names = seatsNow.map((seat, i) => `P${i + 1} ${botNames[seat] || ""}`.trim());
    simResult.single = true;
    $("sim-status").textContent = stats.finished ? "This game (Run Game)" : "This game so far";
    renderSim();
  }

  function simulateHere(games) {
    wasm.state_bytes_current();
    const state = resultBytes();
    const seatsNow = api.getSeats();
    const p = wasm.alloc(state.length);
    new Uint8Array(wasm.memory.buffer, p, state.length).set(state);
    const out = JSON.parse(ok(wasm.simulate(p, games, 1000)));
    wasm.dealloc(p, state.length);
    simResult = mergeSims([out], seatsNow.length);
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
          p.worker.postMessage({ type: "sim", id: i, state, seats: seatsNow, games: n, seed: 1000 + i });
        }));
      });
      let done = 0;
      jobs.forEach((j) => j.then(() => {
        done++;
        if (run === simRun) $("sim-status").textContent = `Simulating: ${done}/${jobs.length} workers done`;
      }, () => {}));
      const parts = await Promise.race([Promise.all(jobs), cancelled]);
      if (run !== simRun) return;
      simResult = mergeSims(parts, players);
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

  function mergeSims(parts, players) {
    const add = (a, b) => {
      const out = a.slice();
      b.forEach((x, i) => (out[i] = (out[i] || 0) + x));
      return out;
    };
    const r = { games: 0, capped: 0, lengthSum: 0, players: [] };
    for (let p = 0; p < players; p++) r.players.push({ vp: [], money: [], buys: [], count: [], wins: 0, winsAt: [] });
    for (const part of parts) {
      r.games += part.games;
      r.capped += part.capped;
      r.lengthSum += part.lengthSum;
      part.players.forEach((q, p) => {
        const t = r.players[p];
        t.vp = add(t.vp, q.vp);
        t.money = add(t.money, q.money);
        t.buys = add(t.buys, q.buys);
        t.count = add(t.count, q.count);
        t.winsAt = add(t.winsAt, q.winsAt);
        t.wins += q.wins;
      });
    }
    return r;
  }

  const METRICS = {
    vp: { title: "Average VP after each turn", y: "VP", fmt: (v) => v.toFixed(1) },
    money: { title: "Average money per turn ($ spent + left over)", y: "$", fmt: (v) => "$" + v.toFixed(2) },
    buys: { title: "Average cards bought per turn", y: "buys", fmt: (v) => v.toFixed(2) },
    winturn: { title: "Games won, by the winner's turn number", y: "% of games", fmt: (v) => v.toFixed(1) + "%" },
  };

  // Series per player: points {x: turn, y: value}. Per-turn averages only where enough games
  // reached that turn (at least 5% of games) so the tail isn't a handful of long games.
  function seriesFor(metric) {
    const r = simResult;
    return r.players.map((q, p) => {
      const pts = [];
      if (metric === "winturn") {
        q.winsAt.forEach((w, i) => pts.push({ x: i + 1, y: (100 * w) / r.games }));
      } else {
        q.count.forEach((c, i) => {
          if (c >= Math.max(1, r.games * 0.05)) pts.push({ x: i + 1, y: q[metric][i] / c });
        });
      }
      return { name: r.names[p], color: `var(--series-${p + 1})`, pts };
    });
  }

  // Win-turn curves are zero for the early turns and after the last win; show only the span where
  // any player wins, plus one turn either side.
  function trimToActive(series) {
    const xs = series.flatMap((s) => s.pts.filter((p) => p.y > 0).map((p) => p.x));
    if (!xs.length) return series;
    const lo = Math.min(...xs) - 1, hi = Math.max(...xs) + 1;
    return series.map((s) => ({ ...s, pts: s.pts.filter((p) => p.x >= lo && p.x <= hi) }));
  }

  function renderSim() {
    if (!simResult) return;
    const r = simResult;
    const summary = $("sim-summary");
    summary.innerHTML = "";
    r.players.forEach((q, p) => {
      const item = el("span", "sim-stat");
      const key = el("span", "sim-key");
      key.style.background = `var(--series-${p + 1})`;
      item.appendChild(key);
      item.appendChild(document.createTextNode(`${r.names[p]}: `));
      item.appendChild(el("b", null, r.single ? (q.wins > 0 ? "won" : "lost") : `${((100 * q.wins) / r.games).toFixed(1)}% wins`));
      summary.appendChild(item);
    });
    const finished = r.games - r.capped;
    summary.appendChild(el("span", "sim-stat muted", `average game ${finished ? (r.lengthSum / finished).toFixed(1) : "-"} turns each` + (r.capped ? ` · ${r.capped} hit the turn limit` : "")));
    const metric = $("sim-metric").value;
    const series = metric === "winturn" ? trimToActive(seriesFor(metric)) : seriesFor(metric);
    drawLineChart($("sim-chart"), series, METRICS[metric]);
    renderSimTable(series, METRICS[metric]);
  }

  function renderSimTable(series, m) {
    const box = $("sim-table");
    box.hidden = !$("sim-table-toggle").checked;
    box.innerHTML = "";
    const xs = [...new Set(series.flatMap((s) => s.pts.map((p) => p.x)))].sort((a, b) => a - b);
    const table = el("table", "sim-table");
    const head = el("tr");
    head.appendChild(el("th", null, "Turn"));
    series.forEach((s) => head.appendChild(el("th", null, s.name)));
    table.appendChild(head);
    xs.forEach((x) => {
      const tr = el("tr");
      tr.appendChild(el("td", null, String(x)));
      series.forEach((s) => {
        const pt = s.pts.find((p) => p.x === x);
        tr.appendChild(el("td", null, pt ? m.fmt(pt.y) : ""));
      });
      table.appendChild(tr);
    });
    box.appendChild(table);
  }

  const SVGNS = "http://www.w3.org/2000/svg";
  function svg(tag, attrs, parent) {
    const e = document.createElementNS(SVGNS, tag);
    for (const k in attrs) e.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(e);
    return e;
  }

  function niceTicks(max, count) {
    if (max <= 0) return [0, 1];
    const raw = max / count;
    const mag = Math.pow(10, Math.floor(Math.log10(raw)));
    const step = [1, 2, 2.5, 5, 10].map((f) => f * mag).find((st) => raw <= st) || 10 * mag;
    const ticks = [];
    for (let v = 0; v <= max + step * 0.001; v += step) ticks.push(+v.toFixed(10));
    if (ticks[ticks.length - 1] < max) ticks.push(ticks[ticks.length - 1] + step);
    return ticks;
  }

  function drawLineChart(host, series, m) {
    host.innerHTML = "";
    const all = series.flatMap((s) => s.pts);
    if (!all.length) {
      host.appendChild(el("div", "muted", "No data."));
      return;
    }
    host.appendChild(el("div", "chart-title", m.title));
    // Legend: line keys in seat order (mirrors the marks).
    const legend = el("div", "chart-legend");
    series.forEach((s) => {
      const item = el("span", "legend-item");
      const key = el("span", "legend-line");
      key.style.background = s.color;
      item.appendChild(key);
      item.appendChild(document.createTextNode(s.name));
      legend.appendChild(item);
    });
    host.appendChild(legend);

    const W = Math.max(320, host.clientWidth || 800), H = 280;
    const pad = { l: 48, r: 130, t: 10, b: 30 };
    const xMax = Math.max(...all.map((p) => p.x)), xMin = Math.min(...all.map((p) => p.x));
    const ticks = niceTicks(Math.max(...all.map((p) => p.y)), 5);
    const yMax = ticks[ticks.length - 1] || 1;
    const X = (x) => pad.l + ((x - xMin) / Math.max(1, xMax - xMin)) * (W - pad.l - pad.r);
    const Y = (y) => H - pad.b - (y / yMax) * (H - pad.t - pad.b);
    const root = svg("svg", { width: W, height: H, viewBox: `0 0 ${W} ${H}`, class: "chart-svg", role: "img", "aria-label": m.title });
    // Recessive grid + axes.
    ticks.forEach((t) => {
      svg("line", { x1: pad.l, x2: W - pad.r, y1: Y(t), y2: Y(t), class: "grid" }, root);
      const lab = svg("text", { x: pad.l - 8, y: Y(t) + 4, class: "tick", "text-anchor": "end" }, root);
      lab.textContent = m.fmt(t).replace(/\.0+(?=%|$)/, "");
    });
    const xStep = Math.max(1, Math.ceil((xMax - xMin) / 12));
    for (let x = xMin; x <= xMax; x += xStep) {
      const lab = svg("text", { x: X(x), y: H - pad.b + 18, class: "tick", "text-anchor": "middle" }, root);
      lab.textContent = String(x);
    }
    const xl = svg("text", { x: (pad.l + W - pad.r) / 2, y: H - 2, class: "axis-label", "text-anchor": "middle" }, root);
    xl.textContent = "turn";
    // Lines (2px), with a direct label at each line's end.
    const ends = [];
    series.forEach((s) => {
      if (!s.pts.length) return;
      const d = s.pts.map((p, i) => `${i ? "L" : "M"}${X(p.x).toFixed(1)},${Y(p.y).toFixed(1)}`).join("");
      svg("path", { d, class: "series-line", style: `stroke:${s.color}` }, root);
      const last = s.pts[s.pts.length - 1];
      ends.push({ s, x: X(last.x), y: Y(last.y) });
    });
    // End labels stay above the x axis and stack upward when they would overlap.
    ends.sort((a, b) => b.y - a.y);
    ends.forEach((e, i) => {
      const floor = i === 0 ? H - pad.b - 8 : ends[i - 1].y - 14;
      e.y = Math.min(e.y, floor);
    });
    ends.forEach((e) => {
      svg("circle", { cx: e.x, cy: Y(e.s.pts[e.s.pts.length - 1].y), r: 4, class: "end-dot", style: `fill:${e.s.color}` }, root);
      const t = svg("text", { x: e.x + 8, y: e.y + 4, class: "end-label" }, root);
      t.textContent = e.s.name;
    });
    // Crosshair + tooltip listing every series at the hovered turn.
    const cross = svg("line", { y1: pad.t, y2: H - pad.b, class: "crosshair", visibility: "hidden" }, root);
    const tip = el("div", "chart-tip");
    tip.hidden = true;
    const hit = svg("rect", { x: pad.l, y: pad.t, width: W - pad.l - pad.r, height: H - pad.t - pad.b, fill: "transparent", tabindex: 0 }, root);
    const show = (clientX) => {
      const box = root.getBoundingClientRect();
      const px = ((clientX - box.left) / box.width) * W;
      const x = Math.max(xMin, Math.min(xMax, Math.round(xMin + ((px - pad.l) / (W - pad.l - pad.r)) * (xMax - xMin))));
      cross.setAttribute("x1", X(x));
      cross.setAttribute("x2", X(x));
      cross.setAttribute("visibility", "visible");
      tip.innerHTML = "";
      tip.appendChild(el("div", "tip-head", `Turn ${x}`));
      series.forEach((s) => {
        const pt = s.pts.find((p) => p.x === x);
        const row = el("div", "tip-row");
        const key = el("span", "legend-line");
        key.style.background = s.color;
        row.appendChild(key);
        row.appendChild(el("b", null, pt ? m.fmt(pt.y) : "–"));
        row.appendChild(document.createTextNode(" " + s.name));
        tip.appendChild(row);
      });
      tip.hidden = false;
      const left = (X(x) / W) * box.width;
      tip.style.left = `${Math.min(left + 12, box.width - 220)}px`;
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
    $("sim-metric").addEventListener("change", () => renderSim());
    $("sim-table-toggle").addEventListener("change", () => renderSim());
    window.addEventListener("resize", () => renderSim());
    $("btn-analyze").addEventListener("click", () => {
      if (lastView && lastView.pending && lastView.pending.paused) {
        doAction(() => api.resume(), "Start turn");
        if (crashed || !lastView.pending || lastView.pending.paused || lastView.gameOver) return;
      }
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
    // Start with a kingdom matching the default seats' rules.
    try {
      startNewGame();
    } catch (e) {
      showNewGameError(String(e.message || e));
      $("state-text").value = api.getStartText();
    }
    render();
    // Test hook: open index.html#selftest-analyze to run a parallel analysis on load.
    if (location.hash === "#selftest-analyze") $("btn-analyze").click();
    // Test hook: open index.html#selftest-sim to run a simulation on load.
    if (location.hash === "#selftest-sim") simulate();
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
    // Test hook: index.html#selftest-load=<base64 state text> loads that state and analyzes it.
    if (location.hash.startsWith("#selftest-load=")) {
      $("state-text").value = decodeURIComponent(escape(atob(location.hash.slice("#selftest-load=".length))));
      $("btn-load").click();
      renderAnalysis(api.analyze()); // same-thread, so headless screenshots see the result
    }
    // Test hook: index.html#selftest-graph=<base64 state text> loads that state, analyzes it and
    // opens the search graph (a bare #selftest-graph opens it on the current position).
    if (location.hash.startsWith("#selftest-graph")) {
      if (location.hash.startsWith("#selftest-graph=")) {
        $("state-text").value = decodeURIComponent(escape(atob(location.hash.slice("#selftest-graph=".length).split("&")[0])));
        $("btn-load").click();
      }
      renderAnalysis(api.analyze());
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
    // Test hook: open index.html#selftest-chart to simulate in the page and draw the chart.
    if (location.hash.startsWith("#selftest-chart")) {
      $("sim-metric").value = location.hash.split("-")[2] || "vp";
      simulateHere(1000);
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
