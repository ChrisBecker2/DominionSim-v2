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
    analyze() {
      return JSON.parse(ok(wasm.analyze()));
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
        doAction(() => {
          api.setSeat(p.index, parseInt(sel.value, 10));
          maybeRunBots();
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
    for (const c of view.supply) {
      const tr = document.createElement("tr");
      const tdName = document.createElement("td");
      tdName.appendChild(cardChip(c.name));
      const tdCost = document.createElement("td");
      tdCost.textContent = "$" + c.cost;
      const tdCount = document.createElement("td");
      tdCount.textContent = c.count;
      if (c.count === 0) tdCount.className = "count-0";
      tr.appendChild(tdName);
      tr.appendChild(tdCost);
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
    const who = botNames[seats[view.pending.player] ?? 0] || "Human";
    desc.textContent = `${view.pending.description}  [${who}]`;
    view.pending.choices.forEach((c, i) => {
      const b = el("button");
      if (i < 9) {
        const k = el("span", "key", String(i + 1));
        b.appendChild(k);
      }
      b.appendChild(decorate(c.label));
      b.addEventListener("click", () => doAction(() => {
        api.choose(c.index);
        maybeRunBots();
      }));
      choices.appendChild(b);
    });
  }

  function renderLog(view) {
    const list = $("log-list");
    const atBottom = list.scrollTop + list.clientHeight >= list.scrollHeight - 4;
    list.innerHTML = "";
    for (const line of view.log) {
      const cls = line.startsWith("---")
        ? "turn-marker"
        : line.startsWith("::")
        ? "phase-marker"
        : line.startsWith("--")
        ? "sys-marker"
        : "";
      const row = el("div", cls || null);
      row.appendChild(decorate(line));
      list.appendChild(row);
    }
    if (atBottom || view.log.length <= 1) list.scrollTop = list.scrollHeight;
  }

  let seats = [];

  // Auto-run plays bots only up to a human seat's decision. With no human seat it would play
  // the whole game in one go, so then the game waits for Auto-step / Run to end of turn.
  function maybeRunBots() {
    if (!$("auto-bots").checked) return;
    if (!api.getSeats().some((s) => s === 0)) return;
    try {
      api.runBots();
    } catch (e) {
      // A human seat is deciding: nothing to run.
    }
  }

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
        postMessage({ type: "ready" });
        return;
      }
      const put = (bytes) => {
        const p = w.alloc(bytes.length);
        new Uint8Array(w.memory.buffer, p, bytes.length).set(bytes);
        return p;
      };
      const rp = put(m.root), sp = put(m.state);
      const ok = w.eval_task(rp, sp, m.me, m.budget);
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
        p.worker.postMessage({ type: "task", id, root, state: tasks[id], me: plan.player, budget: TASK_BUDGET });
      };
      workers.forEach((p) => {
        p.worker.onmessage = (e) => {
          const m = e.data;
          if (m.type !== "result" || run !== analysisRun) return;
          if (!m.ok) return reject(new Error(m.out));
          const r = JSON.parse(m.out);
          const pv = writeString(r.pv);
          const st = wasm.plan_put_result(m.id, r.ev, r.exact ? 1 : 0, r.nodes, r.ttHits, pv.ptr, pv.len);
          freeString(pv);
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

  function renderAnalysis(result) {
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
    $("analysis-meta").textContent = `P${result.player + 1} to decide, ${result.nodes.toLocaleString()} nodes searched, ${result.ttHits.toLocaleString()} transpositions${timing}`;
    result.options.forEach((o, i) => {
      const tr = el("tr", i === 0 ? "best" : null);
      const label = el("td");
      label.appendChild(decorate(o.label));
      tr.appendChild(label);
      const ev = el("td", "ev", o.ev.toFixed(2));
      if (!o.exact) ev.appendChild(el("span", "approx", "~sampled"));
      tr.appendChild(ev);
      const pv = el("td", "pv");
      pv.appendChild(decorate(o.pv));
      tr.appendChild(pv);
      tbody.appendChild(tr);
    });
  }

  function render() {
    seats = api.getSeats();
    const view = api.getView();
    lastView = view;
    renderTurnBar(view);
    renderPlayers(view);
    renderSupply(view);
    renderTrash(view);
    renderDecision(view);
    renderLog(view);
    $("btn-undo").disabled = !api.canUndo();
    $("btn-redo").disabled = !api.canRedo();
    if ($("auto-sync").checked) syncTextFromGame();
  }

  function doAction(fn) {
    if ($("analysis-progress") && !$("analysis-progress").hidden) cancelAnalysis();
    renderAnalysis(null);
    try {
      fn();
      showLoadError("");
      render();
    } catch (e) {
      showToast(String(e.message || e));
      render();
    }
  }

  // ---- wiring ---------------------------------------------------------------

  function wire() {
    $("btn-new-game").addEventListener("click", () => {
      const players = parseInt($("ng-players").value, 10) || 2;
      const kingdom = $("ng-kingdom").value;
      const seed = parseInt($("ng-seed").value, 10) || 0;
      try {
        api.newGame(players, kingdom, seed, 0);
        maybeRunBots();
        showNewGameError("");
        syncTextFromGame();
        render();
      } catch (e) {
        showNewGameError(String(e.message || e));
      }
    });

    $("btn-load").addEventListener("click", () => {
      try {
        api.loadState($("state-text").value);
        showLoadNotice(api.getView());
        maybeRunBots();
        showLoadError("");
        render();
      } catch (e) {
        showLoadError(String(e.message || e));
      }
    });

    $("btn-sync").addEventListener("click", () => syncTextFromGame());

    $("btn-auto").addEventListener("click", () => doAction(() => api.stepAuto()));
    $("btn-run-turn").addEventListener("click", () => doAction(() => api.runToEndOfTurn()));
    $("btn-run-bots").addEventListener("click", () => doAction(() => api.runBots()));
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
          showToast("Analysis failed: " + String(e.message || e));
          cancelAnalysis();
        })
        .finally(() => ($("btn-analyze").disabled = false));
    });
    $("btn-cancel-analysis").addEventListener("click", () => {
      cancelAnalysis();
      $("btn-analyze").disabled = false;
    });
    $("btn-undo").addEventListener("click", () => doAction(() => api.undo()));
    $("btn-redo").addEventListener("click", () => doAction(() => api.redo()));

    window.addEventListener("keydown", (e) => {
      const tag = document.activeElement && document.activeElement.tagName;
      if (tag === "TEXTAREA" || tag === "INPUT") return;
      if (e.key >= "1" && e.key <= "9") {
        const i = e.key.charCodeAt(0) - "1".charCodeAt(0);
        if (lastView && lastView.pending && i < lastView.pending.choices.length) {
          doAction(() => {
            api.choose(lastView.pending.choices[i].index);
            maybeRunBots();
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
        doAction(() => api.stepAuto());
      }
    });
  }

  async function main() {
    const bytes = base64ToBytes(WASM_BASE64);
    const { module, instance } = await WebAssembly.instantiate(bytes, {});
    wasmModule = module;
    wasm = instance.exports;
    botNames = api.listBots();
    initCards();
    wire();
    syncTextFromGame();
    render();
    // Test hook: open index.html#selftest-analyze to run a parallel analysis on load.
    if (location.hash === "#selftest-analyze") $("btn-analyze").click();
  }

  main().catch((e) => {
    document.body.innerHTML =
      '<pre style="color:#a33;padding:20px;white-space:pre-wrap">Failed to start: ' +
      (e && e.stack ? e.stack : e) +
      "</pre>";
  });
})();
