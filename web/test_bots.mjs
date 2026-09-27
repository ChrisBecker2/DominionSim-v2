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

// ---- parallel protocol: main instance plans, a second instance evaluates subtrees ----
{
  const mod = await WebAssembly.compile(bytes);
  const main = (await WebAssembly.instantiate(mod, {})).exports;
  const wk = (await WebAssembly.instantiate(mod, {})).exports;
  const rd = (x) => dec.decode(new Uint8Array(x.memory.buffer, x.result_ptr(), x.result_len()));
  const rb = (x) => new Uint8Array(x.memory.buffer, x.result_ptr(), x.result_len()).slice();
  const put = (x, b) => { const p = x.alloc(b.length); new Uint8Array(x.memory.buffer, p, b.length).set(b); return p; };
  const text = `players: 2
kingdom: Village, Smithy, Moneylender, Chapel, Cellar, Laboratory, Market, Militia, Throne Room, Library
turn: 5  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: Village, Smithy, 3 Copper
deck: 4 Copper, 2 Silver, 3 Estate, Gold

[player 2]
hand: 5 Copper
deck: 2 Copper, 3 Estate
`;
  const tb = enc.encode(text); const tp = put(main, tb); main.load_state(tp, tb.length); main.dealloc(tp, tb.length);
  main.analyze(); const serial = JSON.parse(rd(main));
  const plan = JSON.parse((main.plan_start(16), rd(main)));
  main.plan_root_bytes(); const root = rb(main);
  check("state bytes have expected size", root.length === main.state_size());
  for (let i = 0; i < plan.tasks; i++) {
    main.plan_task_bytes(i); const st = rb(main);
    const rp = put(wk, root), sp = put(wk, st);
    wk.eval_task(rp, sp, plan.player, 200000);
    const r = JSON.parse(rd(wk));
    wk.dealloc(rp, root.length); wk.dealloc(sp, st.length);
    const pv = enc.encode(r.pv); const pp = put(main, pv);
    main.plan_put_result(i, r.ev, r.exact ? 1 : 0, r.nodes, r.ttHits, pp, pv.length); main.dealloc(pp, pv.length);
  }
  const par = JSON.parse((main.plan_finish(), rd(main)));
  console.log(`    ${plan.tasks} subtrees; serial best ${serial.options[0].label} ${serial.options[0].ev}, parallel best ${par.options[0].label} ${par.options[0].ev}`);
  check("parallel values equal serial", serial.options.every((o) => Math.abs(par.options.find((p) => p.label === o.label).ev - o.ev) < 1e-3));
  check("parallel pv is readable", par.options[0].pv.startsWith("Play Village") && par.options[0].pv.includes("end turn"));
}
if (failed) { console.log(`${failed} FAILED`); process.exit(1); } else console.log("ALL PASSED (parallel)");
