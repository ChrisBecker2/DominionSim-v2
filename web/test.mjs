#!/usr/bin/env node
// Smoke test for the dominion-wasm C ABI, run under plain Node (no browser needed).
// Usage: node web/test.mjs
// Prerequisite: cargo build -p dominion-wasm --release --target wasm32-unknown-unknown

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const webDir = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(webDir, "..", "target", "wasm32-unknown-unknown", "release", "dominion_wasm.wasm");

let failures = 0;
function check(cond, msg) {
  if (cond) {
    console.log(`  ok   ${msg}`);
  } else {
    failures++;
    console.error(`  FAIL ${msg}`);
  }
}
function section(name) {
  console.log(`\n${name}`);
}

async function main() {
  const bytes = readFileSync(wasmPath);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const wasm = instance.exports;

  function writeString(str) {
    const b = new TextEncoder().encode(str);
    const ptr = wasm.alloc(b.length);
    new Uint8Array(wasm.memory.buffer, ptr, b.length).set(b);
    return { ptr, len: b.length };
  }
  function freeString(s) {
    wasm.dealloc(s.ptr, s.len);
  }
  function readResult() {
    const ptr = wasm.result_ptr();
    const len = wasm.result_len();
    return new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, ptr, len));
  }
  function ok(status) {
    const msg = readResult();
    if (!status) throw new Error(msg || "call failed");
    return msg;
  }
  function view() {
    wasm.get_view();
    return JSON.parse(readResult());
  }
  function stateText() {
    wasm.get_state_text();
    return readResult();
  }
  function newGame(players, kingdom, seed, maxTurns) {
    const k = writeString(kingdom);
    const status = wasm.new_game(players, k.ptr, k.len, BigInt(seed), maxTurns || 0);
    freeString(k);
    return ok(status);
  }
  function loadState(text) {
    const s = writeString(text);
    const status = wasm.load_state(s.ptr, s.len);
    freeString(s);
    return ok(status);
  }
  function choose(i) {
    return ok(wasm.choose(i));
  }

  section("module shape");
  check(typeof wasm.memory === "object", "exports a memory");
  for (const name of [
    "alloc",
    "dealloc",
    "result_ptr",
    "result_len",
    "new_game",
    "load_state",
    "get_state_text",
    "get_view",
    "choose",
    "step_auto",
    "run_to_end_of_turn",
    "undo",
    "redo",
    "can_undo",
    "can_redo",
    "seat_kingdom",
    "padded_kingdom",
  ]) {
    check(typeof wasm[name] === "function", `exports ${name}()`);
  }

  section("default state on first use");
  {
    const v = view();
    check(v.numPlayers === 2, "default game has 2 players");
    check(Array.isArray(v.supply) && v.supply.length > 0, "supply is populated");
    check(v.pending !== null, "a decision is pending at game start");
    check(v.log.length > 0, "event log has entries from initial draws");
  }

  section("new_game");
  {
    newGame(3, "Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop", 42, 0);
    const v = view();
    check(v.numPlayers === 3, `new_game(3) -> numPlayers === 3 (got ${v.numPlayers})`);
    check(v.players.length === 3, "3 player panels");
    check(v.turn.number === 1, "turn 1");
  }
  {
    let threw = false;
    try {
      newGame(9, "Village", 1, 0);
    } catch (e) {
      threw = true;
      check(/player count/.test(e.message), `out-of-range player count rejected: ${e.message}`);
    }
    check(threw, "new_game(9, ...) throws");
  }

  section("padded_kingdom: New Game's card-set-aware kingdom padding");
  {
    // All 26 Intrigue (2nd edition) kingdom cards, transcribed independently of the engine's
    // card table, so this section checks the padding actually lands in the right set.
    const INTRIGUE = new Set([
      "Courtyard", "Lurker", "Pawn", "Masquerade", "Shanty Town", "Steward", "Swindler",
      "Wishing Well", "Baron", "Bridge", "Conspirator", "Diplomat", "Ironworks", "Mill",
      "Mining Village", "Secret Passage", "Courtier", "Duke", "Minion", "Patrol", "Replace",
      "Torturer", "Trading Post", "Upgrade", "Harem", "Nobles",
    ]);
    function paddedKingdom(players, setsMask, seed) {
      const r = JSON.parse(ok(wasm.padded_kingdom(players, setsMask, BigInt(seed))));
      return { strategySeats: r.strategySeats, kingdom: r.kingdom.split(",").map((s) => s.trim()).filter(Boolean) };
    }
    // Default seats: Player 1 = Double Witch (needs Witch, a Base card), others = Big Money
    // Ultimate (no kingdom cards needed).
    const required = JSON.parse(ok(wasm.seat_kingdom(2))).kingdom.split(",").map((s) => s.trim()).filter(Boolean);
    check(required.includes("Witch"), `default Player 1 (Double Witch) requires Witch (got: ${required.join(", ")})`);

    const both1 = paddedKingdom(2, 3, 99);
    const both2 = paddedKingdom(2, 3, 99);
    check(JSON.stringify(both1.kingdom) === JSON.stringify(both2.kingdom), "same seed + selection reproduces the same kingdom");
    check(both1.kingdom.length === 10, `padded to 10 cards (got ${both1.kingdom.length})`);
    check(new Set(both1.kingdom).size === 10, "no duplicates in the padded kingdom");

    const otherSeed = paddedKingdom(2, 3, 100);
    check(JSON.stringify(otherSeed.kingdom) !== JSON.stringify(both1.kingdom), "a different seed usually gives a different kingdom");

    const intrigueOnly = paddedKingdom(2, 2, 99); // sets_mask 2 = Intrigue
    check(intrigueOnly.kingdom.length === 10, "still 10 cards with Intrigue only");
    check(intrigueOnly.kingdom.includes("Witch"), "the seats' required card (Witch) stays in even though it's a Base card");
    const padding = intrigueOnly.kingdom.filter((c) => c !== "Witch");
    check(padding.every((c) => INTRIGUE.has(c)), `every padded card besides the required Witch is Intrigue (got: ${padding.join(", ")})`);

    const baseOnly = paddedKingdom(2, 1, 99); // sets_mask 1 = Base
    check(baseOnly.kingdom.every((c) => !INTRIGUE.has(c)), `Base-only padding has no Intrigue cards (got: ${baseOnly.kingdom.join(", ")})`);

    // All 25 Prosperity (2nd edition) kingdom cards, transcribed independently of the engine's
    // card table (Platinum/Colony are basic cards added by the Colony rule, not kingdom cards).
    const PROSPERITY = new Set([
      "Anvil", "Watchtower", "Bishop", "Clerk", "Investment", "Monument", "Quarry", "Tiara",
      "Worker's Village", "Charlatan", "City", "Collection", "Crystal Ball", "Magnate", "Mint",
      "Rabble", "Vault", "War Chest", "Grand Market", "Hoard", "Bank", "Expand", "Forge",
      "King's Court", "Peddler",
    ]);
    const seasideOnly = paddedKingdom(2, 4, 99); // sets_mask 4 = Seaside
    check(seasideOnly.kingdom.length >= 10, `Seaside-only padding produced a kingdom (got ${seasideOnly.kingdom.length})`);
    check(!seasideOnly.kingdom.includes("Platinum") && !seasideOnly.kingdom.includes("Colony"), "no Colonies without Prosperity");

    const prosperityOnly = paddedKingdom(2, 8, 99); // sets_mask 8 = Prosperity
    const prosperityPadding = prosperityOnly.kingdom.filter((c) => c !== "Witch" && c !== "Platinum" && c !== "Colony");
    check(prosperityPadding.every((c) => PROSPERITY.has(c)), `Prosperity-only padding is all Prosperity (got: ${prosperityPadding.join(", ")})`);
    // Official rule: a Prosperity-only draw always includes Platinum and Colony.
    check(prosperityOnly.kingdom.includes("Platinum") && prosperityOnly.kingdom.includes("Colony"), `Prosperity kingdom brings Platinum and Colony (got: ${prosperityOnly.kingdom.join(", ")})`);

    const allFour = paddedKingdom(3, 15, 42); // sets_mask 15 = Base+Intrigue+Seaside+Prosperity
    check(allFour.kingdom.length >= 10, `all-four-sets padding produced a kingdom (got ${allFour.kingdom.length})`);
    const allFourAgain = paddedKingdom(3, 15, 42);
    check(JSON.stringify(allFour.kingdom) === JSON.stringify(allFourAgain.kingdom), "all-four-sets padding is reproducible by seed");

    // All 11 Alchemy kingdom cards (Possession is not implemented), transcribed independently of
    // the engine's card table. Potion is a basic card that joins whenever a Potion-cost card does.
    const ALCHEMY = new Set([
      "Transmute", "Vineyard", "Herbalist", "Apothecary", "Scrying Pool", "University", "Alchemist",
      "Familiar", "Philosopher's Stone", "Golem", "Apprentice",
    ]);
    const alchemyOnly = paddedKingdom(2, 16, 99); // sets_mask 16 = Alchemy
    const alchemyPadding = alchemyOnly.kingdom.filter((c) => c !== "Witch" && c !== "Potion");
    check(alchemyPadding.every((c) => ALCHEMY.has(c)), `Alchemy-only padding is all Alchemy (got: ${alchemyOnly.kingdom.join(", ")})`);
    check(alchemyOnly.kingdom.includes("Potion"), "an Alchemy kingdom brings the Potion pile");
    check(!baseOnly.kingdom.includes("Potion"), "no Potion without Alchemy");
  }

  section("an Alchemy game: Potion pile, printed costs with a Potion, potions in the turn");
  {
    newGame(2, "Familiar, Apothecary, Golem, Herbalist, Village, Smithy, Witch, Market, Militia, Moat", 7, 0);
    const v = view();
    const potion = v.supply.find((c) => c.name === "Potion");
    check(potion && potion.count === 16 && potion.costLabel === "$4", `Potion pile of 16 costing $4 joins the supply (got ${JSON.stringify(potion)})`);
    const fam = v.supply.find((c) => c.name === "Familiar");
    check(fam && fam.costLabel === "$3P" && fam.cost === 3, `Familiar is shown as $3P (got ${JSON.stringify(fam)})`);
    check(v.supply.find((c) => c.name === "Herbalist").costLabel === "$2", "Herbalist is $2");
    check(v.turn.potions === 0, "no potions at the start of a turn");
    const info = JSON.parse(ok(wasm.card_info()));
    check(info.find((c) => c.name === "Golem").costLabel === "$4P", "card_info carries cost labels");
    const text = stateText();
    check(/Potion=16/.test(text), "the Potion pile is in the state text");
    loadState(text);
    check(view().supply.some((c) => c.name === "Potion"), "the state text round-trips with the Potion pile");
  }

  section("a Colony game's supply, VP tokens, durations and mats (Seaside + Prosperity view)");
  {
    const text = [
      "players: 2",
      "kingdom: Bishop, City, Colony, Island, Native Village, Platinum, Wharf",
      "supply: Copper=46, Silver=40, Gold=30, Estate=8, Duchy=8, Province=8, Curse=10, Native Village=10, Island=8, Wharf=10, Bishop=10, City=10, Platinum=12, Colony=8",
      "trash: ",
      // Player 2 is up; player 1's Wharf is still pending from their last turn (kept unresolved
      // by loading mid-turn for a different player, so nothing re-triggers its start-of-turn bonus).
      "turn: 1  player: 2  phase: action  actions: 1  buys: 1  coins: 0",
      "seed: 1",
      "",
      "[player 1]",
      "hand: Copper, Silver, Estate",
      "deck top: Gold, Duchy",
      "deck: ",
      "deck bottom: ",
      "discard: Copper",
      "in play: ",
      "set aside: ",
      "turns: 0",
      "vp tokens: 5",
      "durations: Wharf",
      "native village: Gold, Estate",
      "island: Duchy, Island",
      "",
      "[player 2]",
      "hand: 3 Copper, 2 Estate",
      "deck top: Silver",
      "deck: ",
      "deck bottom: ",
      "discard: ",
      "in play: ",
      "set aside: ",
      "turns: 0",
      "",
    ].join("\n");
    loadState(text);
    const v = view();
    const byName = Object.fromEntries(v.supply.map((c) => [c.name, c]));
    check(!!byName["Platinum"] && byName["Platinum"].count === 12, `Platinum is in the supply (${JSON.stringify(byName["Platinum"])})`);
    check(!!byName["Colony"] && byName["Colony"].count === 8, `Colony is in the supply (${JSON.stringify(byName["Colony"])})`);
    // Platinum/Colony show among the basics, highest-value first: Colony before Province,
    // Platinum before Gold.
    const names = v.supply.map((c) => c.name);
    check(names.indexOf("Colony") === names.indexOf("Province") - 1, `Colony sits right before Province (got: ${names.join(", ")})`);
    check(names.indexOf("Platinum") === names.indexOf("Gold") - 1, `Platinum sits right before Gold (got: ${names.join(", ")})`);

    const p1 = v.players[0];
    check(p1.vpTokens === 5, `player 1 has 5 VP tokens (got ${p1.vpTokens})`);
    check(p1.durations.length === 1 && p1.durations[0].card === "Wharf", `player 1 has a pending Wharf duration (got ${JSON.stringify(p1.durations)})`);
    check(p1.nativeVillageMat.length > 0, "player 1's Native Village mat is non-empty");
    check(p1.islandMat.length > 0, "player 1's Island mat is non-empty");
  }

  section("search_graph: positions, routes and transpositions");
  {
    // Village then Market and Market then Village reach the same position (two play orders).
    loadState(
      [
        "players: 2",
        "kingdom: Village, Market, Smithy, Cellar, Moat, Militia, Remodel, Workshop, Mine, Witch",
        "turn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0",
        "",
        "[player 1]",
        "hand: Village, Market, Copper, Copper, Estate",
        "deck: 4 Copper, 2 Estate, Silver",
        "discard: 2 Copper",
        "",
        "[player 2]",
        "hand: 3 Copper, 2 Estate",
        "deck: 5 Copper, 2 Estate, Silver",
        "",
      ].join("\n")
    );
    check(typeof wasm.search_graph === "function", "exports search_graph()");
    const g = JSON.parse(ok(wasm.search_graph(5000)));
    check(g.nodes.length > 1 && g.edges.length >= g.nodes.length - 1, `graph has ${g.nodes.length} nodes, ${g.edges.length} edges`);
    check(g.nodes[0].kind === "root" && g.nodes[0].depth === 0, "node 0 is the root at depth 0");
    check(g.edges.every((e) => Number.isInteger(e.from) && Number.isInteger(e.to) && e.from >= 0 && e.to >= 0 && e.from < g.nodes.length && e.to < g.nodes.length), "every edge references valid nodes");
    check(g.nodes.some((n) => n.inEdges > 1), "some positions are reached by more than one route");
    const indeg = new Array(g.nodes.length).fill(0);
    for (const e of g.edges) indeg[e.to]++;
    check(g.nodes.every((n, i) => n.inEdges === indeg[i]), "inEdges matches the edges that point at each node");
    check(g.nodes.every((n, i) => i === 0 || n.inEdges >= 1), "every non-root node has an incoming route");
    const sums = new Map();
    for (const e of g.edges) if (g.nodes[e.from].kind === "chance") sums.set(e.from, (sums.get(e.from) || 0) + (e.prob === null ? NaN : e.prob));
    check(sums.size > 0, `graph has chance nodes (${sums.size})`);
    check([...sums.values()].every((p) => Math.abs(p - 1) < 1e-3), "chance edges from each chance node sum to 1");
    check(g.nodes.every((n) => Array.isArray(n.hand) && Array.isArray(n.inPlay) && Array.isArray(n.gained) && Number.isFinite(n.value)), "nodes carry hand/inPlay/gained/value");
    check(g.edges.some((e) => e.best) && g.nodes[0].best, "a best line is marked from the root");

    section("graph.js: index, visible sets, layout, route marks");
    const SG = createRequire(import.meta.url)("./graph.js");
    const idx = SG.buildIndex(g);
    check(idx.n === g.nodes.length && idx.m === g.edges.length, "index sizes");
    let outTotal = 0;
    for (let v = 0; v < idx.n; v++) outTotal += idx.outStart[v + 1] - idx.outStart[v];
    check(outTotal === idx.m, "CSR out lists cover every edge");
    const stats = SG.depthStats(g);
    check(stats.reduce((a, r) => a + r.total, 0) === g.nodes.length, "depth stats count every node");
    check(stats.every((r) => r.merged <= r.total), "merged never exceeds total per depth");
    check(stats[0].total === 1, "one position at depth 0");
    const visBest = SG.visibleSet(g, idx, { mode: "best", cap: 800 });
    check(visBest[0] === 1 && g.nodes.every((n, i) => !n.best || visBest[i]), "best-line mode shows the root and every best node");
    const visAll = SG.visibleSet(g, idx, { mode: "all" });
    check(visAll.every((x) => x === 1), "all mode shows everything");
    const e0 = idx.outList[idx.outStart[0]];
    const visUnder = SG.visibleSet(g, idx, { mode: "under", firstEdge: e0 });
    check(visUnder[idx.eto[e0]] === 1 && visUnder[0] === 1, "under-one-choice mode includes that choice's target and the root");
    const visRange = SG.visibleSet(g, idx, { mode: "all", minDepth: 1, maxDepth: 2 });
    check(visRange.every((x, i) => !x || (g.nodes[i].depth >= 1 && g.nodes[i].depth <= 2)), "depth range filters");
    const lay = SG.layout(g, idx, visAll);
    check(lay.nodes.length === g.nodes.length && lay.edges.length === g.edges.length, "layout lists every visible node and edge");
    check(g.nodes.every((n, i) => lay.x[i] === n.depth - lay.minDepth), "x column is the depth");
    check(Array.from(lay.y).every(Number.isFinite), "every y is finite");
    const seen = new Set();
    let clash = false;
    for (let v = 0; v < idx.n; v++) {
      const key = lay.x[v] + "/" + lay.y[v].toFixed(4);
      if (seen.has(key)) clash = true;
      seen.add(key);
    }
    check(!clash, "no two nodes share a slot");
    const deep = g.nodes.findIndex((n) => n.depth >= 2 && n.inEdges > 1);
    if (deep >= 0) {
      const marks = SG.routeMarks(g, idx, deep);
      check(marks.nodes[0] === 1 && marks.nodes[deep] === 1, "route marks include the root and the node");
      let ok2 = true;
      for (let e = 0; e < idx.m; e++) if (marks.edges[e] === 1 && !marks.nodes[idx.eto[e]]) ok2 = false;
      check(ok2, "marked route edges end at marked nodes");
    }
    // A tiny hand-made diamond: root -> a, b -> c merges.
    const mk = (kind, depth, inEdges, best) => ({ kind, depth, inEdges, best: !!best, value: 0 });
    const d = {
      nodes: [mk("root", 0, 0, 1), mk("decision", 1, 1, 1), mk("decision", 1, 1), mk("leaf", 2, 2, 1)],
      edges: [{ from: 0, to: 1, best: true }, { from: 0, to: 2 }, { from: 1, to: 3, best: true }, { from: 2, to: 3 }],
    };
    const di = SG.buildIndex(d);
    const db = SG.layout(d, di, SG.visibleSet(d, di, { mode: "best" }));
    check(db.nodes.length === 3 && db.edges.length === 2, "diamond: best + merges shows the best line only (3 nodes, 2 edges)");
    const dl = SG.layout(d, di, SG.visibleSet(d, di, { mode: "all" }));
    check(dl.nodes.length === 4 && dl.edges.length === 4, "diamond: all shows every node and edge");
    check(dl.y[1] < dl.y[2], "diamond: best-line node is placed first in its column");
    const dm = SG.routeMarks(d, di, 3);
    check(dm.nodes[0] && dm.nodes[1] && dm.nodes[2] && dm.edges[1] === 1 && dm.edges[3] === 1, "diamond: every route to the merge is marked");
  }

  section("load_state + text round trip");
  {
    const text = [
      "players: 2",
      "kingdom: Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop",
      "trash:",
      "turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0",
      "seed: 7",
      "",
      "[player 1]",
      "hand: Village, Smithy, 3 Copper",
      "deck top: Gold, Silver",
      "deck: 5 Copper, 3 Estate",
      "discard:",
      "in play:",
      "",
      "[player 2]",
      "hand: 5 Copper",
      "deck top:",
      "deck: 3 Copper, 3 Estate",
      "discard:",
      "in play:",
      "",
    ].join("\n");
    loadState(text);
    const v = view();
    const p1 = v.players[0];
    check(p1.hand.some((c) => c.name === "Village"), "player 1 hand has Village");
    check(p1.deckTop[0] === "Gold", `deck top[0] is Gold (first listed = top) (got ${p1.deckTop[0]})`);
    check(v.pending && v.pending.description.includes("Player 1"), "decision is for player 1");
    check(v.pending.choices.some((c) => c.label === "Play Village"), "choice list includes 'Play Village'");

    const text2 = stateText();
    check(text2.includes("hand: Village, Smithy, 3 Copper") || text2.includes("hand: 3 Copper, Village, Smithy"), "state text still shows the loaded hand");
    check(/deck top: Gold, Silver/.test(text2), "state text preserves deck-top order");
  }

  section("bad text is rejected with a line number");
  {
    let threw = false;
    try {
      loadState("players: 2\nkingdom: Village\n[player 1]\nhand: Not A Real Card\n");
    } catch (e) {
      threw = true;
      check(/^line \d+:/.test(e.message), `error has a line number prefix: "${e.message}"`);
    }
    check(threw, "load_state rejects malformed text");
    // Previously loaded (valid) state must still be active.
    const v = view();
    check(v.players[0].hand.some((c) => c.name === "Village"), "state unchanged after a rejected load");
  }

  section("choose / play a card");
  {
    const before = view();
    const villageChoice = before.pending.choices.find((c) => c.label === "Play Village");
    check(!!villageChoice, "Play Village is offered");
    choose(villageChoice.index);
    const after = view();
    check(after.players[0].inPlay.some((c) => c.name === "Village"), "Village moved to in play");
    check(after.log.some((l) => l.includes("plays Village")), "log records 'plays Village'");
    check(after.turn.actions >= 1, "Village grants +1 action (still >=1 action left)");
  }

  section("undo / redo");
  {
    const beforeUndo = view();
    check(wasm.can_undo() === 1, "can_undo() is true after an action");
    ok(wasm.undo());
    const afterUndo = view();
    check(
      !afterUndo.players[0].inPlay.some((c) => c.name === "Village"),
      "undo removed Village from in play"
    );
    check(wasm.can_redo() === 1, "can_redo() is true after an undo");
    ok(wasm.redo());
    const afterRedo = view();
    check(
      afterRedo.players[0].inPlay.some((c) => c.name === "Village"),
      "redo restored Village to in play"
    );
    void beforeUndo;
  }

  section("step_auto and run_to_end_of_turn");
  {
    const startTurn = view().turn.number;
    ok(wasm.run_to_end_of_turn());
    const v = view();
    check(v.turn.number === startTurn + 1 || v.gameOver, `run_to_end_of_turn advances the turn (was ${startTurn}, now ${v.turn.number})`);
    check(v.turn.player === 1, "turn passed to player 2");
  }

  section("play to completion with step_auto (bounded, must terminate)");
  {
    let steps = 0;
    while (true) {
      const v = view();
      if (v.gameOver) break;
      ok(wasm.step_auto());
      steps++;
      if (steps > 20000) throw new Error("game did not terminate within 20000 auto-steps");
    }
    const v = view();
    check(v.gameOver, `game reached game-over after ${steps} auto-steps`);
    check(Array.isArray(v.winners) && v.winners.length > 0, "winners reported at game over");
  }

  console.log(`\n${failures === 0 ? "ALL PASSED" : failures + " FAILURE(S)"}`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((e) => {
  console.error("test script crashed:", e);
  process.exit(1);
});
