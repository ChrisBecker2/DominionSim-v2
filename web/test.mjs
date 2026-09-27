#!/usr/bin/env node
// Smoke test for the dominion-wasm C ABI, run under plain Node (no browser needed).
// Usage: node web/test.mjs
// Prerequisite: cargo build -p dominion-wasm --release --target wasm32-unknown-unknown

import { readFileSync } from "node:fs";
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
