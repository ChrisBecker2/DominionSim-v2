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

pool.forEach((p) => p.w.terminate());
if (failed) { console.log(`${failed} FAILED`); process.exit(1); } else console.log("ALL PASSED (workers)");
