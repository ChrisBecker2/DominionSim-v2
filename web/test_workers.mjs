// Runs the page's real worker source (WORKER_SRC in app.js) in Node worker threads, with a
// small shim for the browser's onmessage/postMessage, and checks the analysis and simulation
// protocols end to end.
//
//   node web/test_workers.mjs
import { readFileSync } from "node:fs";
import { Worker } from "node:worker_threads";

const app = readFileSync(new URL("./app.js", import.meta.url), "utf8");
const a = app.indexOf("const WORKER_SRC = `") + "const WORKER_SRC = `".length;
const workerSrc = app.slice(a, app.indexOf("`;", a));
const shim = `
const { parentPort } = require("node:worker_threads");
globalThis.postMessage = (m) => parentPort.postMessage(m);
parentPort.on("message", (m) => globalThis.onmessage({ data: m }));
`;
const mod = await WebAssembly.compile(readFileSync(new URL("./dist/dominion_wasm.wasm", import.meta.url)));
const enc = new TextEncoder(), dec = new TextDecoder();
let failed = 0;
const check = (name, cond) => { console.log((cond ? "  ok   " : "  FAIL ") + name); if (!cond) failed++; };

function spawn() {
  const w = new Worker(shim + workerSrc, { eval: true });
  const ready = new Promise((res) => w.once("message", (m) => m.type === "ready" && res()));
  w.postMessage({ type: "init", module: mod });
  return { w, ready };
}
const ask = (wk, msg) => new Promise((res) => { wk.w.once("message", res); wk.w.postMessage(msg); });

// Main instance: a Double Witch vs Big Money Ultimate game with Witch in the kingdom.
const main = (await WebAssembly.instantiate(mod, {})).exports; main.init();
const k = enc.encode("Witch"); const kp = main.alloc(k.length); new Uint8Array(main.memory.buffer, kp, k.length).set(k);
main.new_game(2, kp, k.length, 1n, 0);
main.state_bytes_current(); const state = new Uint8Array(main.memory.buffer, main.result_ptr(), main.result_len()).slice();

const pool = [spawn(), spawn()];
await Promise.all(pool.map((p) => p.ready));
check("workers start", true);

const t0 = Date.now();
const sims = await Promise.all(pool.map((p, i) => ask(p, { type: "sim", id: i, state, seats: [4, 2], games: 200, seed: 1000 + i })));
check("simulation replies", sims.every((m) => m.type === "sim" && m.ok));
const r = sims.map((m) => JSON.parse(m.out));
const games = r.reduce((s, x) => s + x.games, 0), p1 = r.reduce((s, x) => s + x.players[0].wins, 0);
console.log(`    ${games} games in ${Date.now() - t0} ms; Double Witch wins ${(100 * p1 / games).toFixed(1)}%`);
check("Double Witch beats Big Money Ultimate", p1 / games > 0.7);

// Loaded strategies: the page adds a strategy file and replays it into workers (at init or with
// an "add" message); seat ids must line up everywhere.
const put = (inst, str) => { const b = enc.encode(str); const p = inst.alloc(b.length); new Uint8Array(inst.memory.buffer, p, b.length).set(b); return [p, b.length]; };
const result = (inst) => dec.decode(new Uint8Array(inst.memory.buffer, inst.result_ptr(), inst.result_len()));
const evolved = readFileSync(new URL("../strategies/evolved/sentry_merchant_vs_double_witch.toml", import.meta.url), "utf8");
check("add_strategy accepts a file", main.add_strategy(...put(main, evolved)) === 1);
const added = JSON.parse(result(main));
main.list_bots(); const lastId = JSON.parse(result(main)).length - 1; // ids: 0 = search, then shipped bots, then loaded ones
check("add_strategy returns the next seat id", added.id === lastId && !added.replaced);
check("adding the same name replaces it", main.add_strategy(...put(main, evolved)) === 1 && JSON.parse(result(main)).id === added.id);
check("add_strategy rejects a bad file", main.add_strategy(...put(main, "name = 1")) === 0);
check("the loaded strategy can be seated", main.set_seat(1, added.id) === 1);
const k2 = put(main, "Witch, Sentry, Merchant, Militia");
main.new_game(2, k2[0], k2[1], 7n, 0);
main.state_bytes_current(); const state2 = new Uint8Array(main.memory.buffer, main.result_ptr(), main.result_len()).slice();
const viaInit = new Worker(shim + workerSrc, { eval: true });
const initReady = new Promise((res) => viaInit.once("message", (m) => m.type === "ready" && res()));
viaInit.postMessage({ type: "init", module: mod, custom: [evolved] });
await initReady;
pool[0].w.postMessage({ type: "add", toml: evolved });
const sims2 = await Promise.all([{ w: viaInit }, pool[0]].map((p, i) => ask(p, { type: "sim", id: i, state: state2, seats: [4, added.id], games: 8, seed: 50 + i })));
check("workers simulate a loaded strategy", sims2.every((m) => m.type === "sim" && m.ok));
const r2 = sims2.filter((m) => m.ok).map((m) => JSON.parse(m.out));
const g2 = r2.reduce((s, x) => s + x.games, 0), w2 = r2.reduce((s, x) => s + x.players[1].wins, 0);
console.log(`    ${g2} games; loaded Sentry Merchant wins ${(100 * w2 / Math.max(1, g2)).toFixed(1)}% vs Double Witch`);
check("the loaded strategy plays as written (beats Double Witch)", g2 > 0 && w2 / g2 > 0.6);
viaInit.terminate();

pool.forEach((p) => p.w.terminate());
if (failed) { console.log(`${failed} FAILED`); process.exit(1); } else console.log("ALL PASSED (workers)");
