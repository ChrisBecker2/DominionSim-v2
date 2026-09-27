// Smoke test for seat bots and analysis exports.
import { readFileSync } from "node:fs";
const bytes = readFileSync(new URL("./dist/dominion_wasm.wasm", import.meta.url));
const { instance } = await WebAssembly.instantiate(bytes, {});
const w = instance.exports;
const enc = new TextEncoder(), dec = new TextDecoder();
const res = () => dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()));
const call = (st) => { if (!st) throw new Error(res()); return res(); };
function load(text) { const b = enc.encode(text); const p = w.alloc(b.length); new Uint8Array(w.memory.buffer, p, b.length).set(b); const st = w.load_state(p, b.length); w.dealloc(p, b.length); return call(st); }
let failed = 0;
const check = (name, cond) => { console.log((cond ? "  ok   " : "  FAIL ") + name); if (!cond) failed++; };

const bots = JSON.parse(call(w.list_bots()));
check("player 1 defaults to Double Witch", bots[JSON.parse(call(w.get_seats()))[0]] === "Double Witch");
check("bots listed", bots[0] === "Human" && bots.length === 13);

load(`players: 2
kingdom: Village, Smithy, Moneylender, Chapel, Cellar, Laboratory, Market, Militia, Throne Room, Library
turn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: Village, Smithy, 3 Copper
deck: 4 Copper, 2 Silver, 3 Estate, Gold

[player 2]
hand: 5 Copper
deck: 2 Copper, 3 Estate
`);
const a = JSON.parse(call(w.analyze()));
console.log(a.options.map((o) => `    ${o.ev.toFixed(2)}  ${o.label}  ::  ${o.pv}`).join("\n"));
check("analysis ranks Play Village first", a.options[0].label.includes("Village"));
check("smithy line never plays village", !a.options.find((o) => o.label.includes("Smithy")).pv.includes("Play Village"));

// Seat 1 = Double Witch bot, seat 0 = search; run a whole game.
call(w.set_seat(0, 0));
check("run_bots reports when a human must decide", w.run_bots() === 0 && /Human/.test(res()));
call(w.set_seat(0, 1));
call(w.set_seat(1, 5));
call(w.run_bots());
const view = JSON.parse((w.get_view(), res()));
const drawLine = view.log.find((l) => /draws \w+, \w+/.test(l));
const copperLine = view.log.find((l) => /plays Copper, Copper/.test(l));
console.log("    " + drawLine + "\n    " + copperLine);
check("draws condensed onto one line", !!drawLine);
check("treasure plays condensed", !!copperLine);
check("no single-card draw lines between condensed ones", !view.log.some((l, i) => i > 0 && /draws/.test(l) && /draws/.test(view.log[i - 1]) && !/Shuffle|shuffles/.test(l)));
check("bots played to game over", view.gameOver === true);
console.log("    scores", view.scores, "winners", view.winners);
if (failed) { console.log(`${failed} FAILED`); process.exit(1); } else console.log("ALL PASSED");
