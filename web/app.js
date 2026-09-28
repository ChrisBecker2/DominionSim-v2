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
    resume() {
      ok(wasm.resume());
    },
    seatKingdom(players) {
      return JSON.parse(ok(wasm.seat_kingdom(players >>> 0)));
    },
    analyze() {
      return JSON.parse(ok(wasm.analyze()));
    },
    stateId() {
      return wasm.state_id() >>> 0;
    },
  };

  let botNames = [];

  // ---- card type highlighting ------------------------------------------------

  let cardClass = new Map(); // card name -> css classes
  let cardRegex = null;

  function initCards() {
    for (const c of JSON.parse(ok(wasm.card_info()))) {
      const t = c.types;
      const primary = t.includes("curse")
        ? "curse"
        : t.includes("treasure")
        ? "treasure"
        : t.includes("victory")
        ? "victory"
        : t.includes("reaction")
        ? "reaction"
        : "action";
      cardClass.set(c.name, "card " + primary + (t.includes("attack") ? " attack" : ""));
    }
    // Longest names first so "Throne Room" wins over any shorter overlap.
    const names = [...cardClass.keys()].sort((a, b) => b.length - a.length);
    cardRegex = new RegExp("\\b(" + names.map((n) => n.replace(/ /g, "\\s")).join("|") + ")\\b", "g");
  }

  function cardChip(name, label) {
    return el("span", cardClass.get(name) || "card", label === undefined ? name : label);
  }

  // Text with every card name wrapped in a type-coloured chip.
  function decorate(text) {
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

  function renderPlayers(view) {
    const row = $("players-row");
    row.innerHTML = "";
    for (const p of view.players) {
      const panel = el("div", "player-panel" + (p.isCurrent ? " current" : ""));
      const h = el("h3");
      const nameSpan = el("span", null, `Player ${p.index + 1}` + (p.isCurrent ? " (current)" : ""));
      const vpSpan = el("span", "vp", `${p.vp} VP`);
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
      if (m.type === "init") {
        const inst = await WebAssembly.instantiate(m.module, {});
        w = inst.exports;
        w.init();
        postMessage({ type: "ready" });
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
      worker.postMessage({ type: "init", module: wasmModule });
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

  // The kingdom is always the union of the kingdom cards the seats' rules name (no padding);
  // Search seats name none. `gameKingdom` is the kingdom the current game was started with.
  let gameKingdom = null;

  function seatsKingdom() {
    const players = parseInt($("ng-players").value, 10) || 2;
    return api.seatKingdom(players).kingdom;
  }

  // Whether the seats now imply a different kingdom than the current game's.
  function syncKingdomFromSeats() {
    return gameKingdom !== null && seatsKingdom() !== gameKingdom;
  }

  function startNewGame() {
    const players = parseInt($("ng-players").value, 10) || 2;
    const seed = parseInt($("ng-seed").value, 10) || 0;
    cancelAnalysis();
    gameKingdom = seatsKingdom();
    api.newGame(players, gameKingdom, seed, 0);
    showNewGameError("");
    $("state-text").value = api.getStartText();
  }

  function renderAnalysis(result) {
    shownAnalysis = result;
    const panel = $("analysis-panel");
    const tbody = $("analysis-table").querySelector("tbody");
    tbody.innerHTML = "";
    if (!result) {
      panel.hidden = true;
      $("analysis-progress").hidden = true;
      return;
    }
    panel.hidden = false;
    $("analysis-progress").hidden = true;
    const timing = result.seconds !== undefined ? `, ${result.seconds.toFixed(2)} s on ${result.workers} workers` : "";
    $("analysis-meta").textContent = "";
    result.options.forEach((o, i) => {
      const tr = el("tr", i === 0 ? "best" : null);
      const label = el("td");
      label.appendChild(decorate(o.label));
      if (o.rulesPick) label.appendChild(el("span", "rules-pick", "rules' pick"));
      if (o.better) label.appendChild(el("span", "rules-better", "scores higher than rules' pick"));
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
      tr.appendChild(oc);
      const pv = el("td", "pv");
      pv.appendChild(decorate(o.pv));
      tr.appendChild(pv);
      tbody.appendChild(tr);
    });
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
    // Nothing to analyze while the game waits at a turn boundary ("Start turn").
    $("btn-analyze").disabled = !view.pending || !!view.pending.paused || view.gameOver;
    $("btn-undo").disabled = !api.canUndo();
    $("btn-redo").disabled = !api.canRedo();
    if ($("auto-sync").checked) syncTextFromGame();
  }

  function doAction(fn, action) {
    if (crashed) return;
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

  // Same-thread fallback (no Web Workers): run the games in the page's own engine instance.
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
    const games = Math.max(10, Math.min(100000, parseInt($("sim-games").value, 10) || 1000));
    const workers = getPool();
    const t0 = performance.now();
    $("sim-status").textContent = `Starting ${workers.length} workers…`;
    $("btn-simulate").disabled = true;
    try {
      await Promise.all(workers.map((p) => p.ready));
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
      const parts = await Promise.all(jobs);
      if (run !== simRun) return;
      simResult = mergeSims(parts, players);
      simResult.seconds = (performance.now() - t0) / 1000;
      simResult.names = seatsNow.map((seat, p) => `P${p + 1} ${botNames[seat] || ""}`.trim());
      $("sim-status").textContent = `${simResult.games.toLocaleString()} games in ${simResult.seconds.toFixed(2)} s`;
      renderSim();
    } catch (e) {
      if (!reportCrash(e, "Simulate")) $("sim-status").textContent = "Simulation failed: " + String(e.message || e);
    } finally {
      $("btn-simulate").disabled = false;
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
      item.appendChild(el("b", null, `${((100 * q.wins) / r.games).toFixed(1)}% wins`));
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

  function wire() {
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
    $("btn-run-bots").addEventListener("click", () => doAction(() => api.runBots(), "Run bots"));
    $("btn-simulate").addEventListener("click", () => simulate());
    $("sim-metric").addEventListener("change", () => renderSim());
    $("sim-table-toggle").addEventListener("change", () => renderSim());
    window.addEventListener("resize", () => renderSim());
    $("btn-analyze").addEventListener("click", () => {
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
    botNames = api.listBots();
    initCards();
    wire();
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
