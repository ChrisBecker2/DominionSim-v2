// Consistency check: for every seat controller and a set of positions, the top choice of
// Analyze (serial and parallel), Auto-step, and Run to end of turn must all agree.
//
//   node web/test_consistency.mjs
import { readFileSync } from "node:fs";

const bytes = readFileSync(new URL("./dist/dominion_wasm.wasm", import.meta.url));
const mod = await WebAssembly.compile(bytes);
const enc = new TextEncoder(), dec = new TextDecoder();

async function instance() {
  const w = (await WebAssembly.instantiate(mod, {})).exports;
  const rd = () => dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()));
  const rb = () => new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()).slice();
  const put = (b) => { const p = w.alloc(b.length); new Uint8Array(w.memory.buffer, p, b.length).set(b); return p; };
  const load = (text) => { const b = enc.encode(text); const p = put(b); const ok = w.load_state(p, b.length); w.dealloc(p, b.length); if (!ok) throw new Error(rd()); };
  return { w, rd, rb, put, load };
}

const KINGDOM = "Cellar, Market, Merchant, Militia, Mine, Moat, Remodel, Smithy, Village, Workshop";
const position = (supply, hand, deck) => `players: 2
kingdom: ${KINGDOM}
supply: ${supply}
turn: 9  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: ${hand}
deck: ${deck}

[player 2]
hand: 3 Copper, 2 Estate
deck: 4 Copper, Estate
`;
const POSITIONS = [
  position("Province=8", "Village Smithy 3 Copper", "4 Copper, 2 Silver, 3 Estate, Gold"),
  position("Province=8", "Remodel Gold 3 Estate Copper", "2 Copper, 3 Estate"),
  position("Province=4, Gold=0, Estate=0, Duchy=2", "Gold Gold Throne Room Remodel", "2 Copper, 3 Estate"),
  position("Silver=0, Estate=0, Duchy=2, Province=3", "Throne Room Gold Gold Remodel Village Village", "2 Copper, 3 Estate"),
  position("Province=8", "Cellar Market Estate Estate Copper", "6 Copper, 3 Estate, 2 Silver, Gold"),
  position("Province=2", "Militia Mine Silver Gold Copper", "5 Copper, 2 Silver, Gold, 2 Estate"),
  position("Province=6", "Workshop Merchant Silver Silver Copper", "5 Copper, Estate, Smithy"),
];

const probe = await instance();
probe.w.list_bots();
const BOTS = JSON.parse(probe.rd());

let failed = 0, checked = 0;
for (const [pi, text] of POSITIONS.entries()) {
  for (let seat = 0; seat < BOTS.length; seat++) {
    const who = `pos ${pi + 1} / ${BOTS[seat]}`;
    // Serial analysis.
    const A = await instance(); A.w.set_seat(0, seat); A.load(text);
    if (!A.w.analyze()) { console.log(`  FAIL ${who}: analyze: ${A.rd()}`); failed++; continue; }
    const serial = JSON.parse(A.rd());
    const top = serial.options[0];

    // Parallel analysis (plan in one instance, subtrees evaluated in another).
    const P = await instance(), W = await instance(); P.w.set_seat(0, seat); W.w.set_seat(0, seat); P.load(text);
    const plan = JSON.parse((P.w.plan_start(8), P.rd()));
    P.w.plan_root_bytes(); const root = P.rb();
    for (let i = 0; i < plan.tasks; i++) {
      P.w.plan_task_bytes(i); const st = P.rb();
      const rp = W.put(root), sp = W.put(st);
      W.w.eval_task(rp, sp, plan.player, 200000, plan.strategy); const r = JSON.parse(W.rd());
      const pv = enc.encode(r.pv); const pp = P.put(pv);
      P.w.plan_put_result(i, r.ev, r.exact ? 1 : 0, r.nodes, r.ttHits, pp, pv.length);
    }
    const par = JSON.parse((P.w.plan_finish(), P.rd()));

    // Auto-step vs choosing the analysis top; Run to end of turn from each.
    const S = await instance(); S.w.set_seat(0, seat); S.load(text); S.w.step_auto(); const afterAuto = S.w.state_id() >>> 0;
    const C = await instance(); C.w.set_seat(0, seat); C.load(text); C.w.choose(top.index); const afterTop = C.w.state_id() >>> 0;
    const R = await instance(); R.w.set_seat(0, seat); R.load(text); R.w.run_to_end_of_turn(); const endRun = R.w.state_id() >>> 0;
    C.w.run_to_end_of_turn(); const endTop = C.w.state_id() >>> 0;

    const problems = [];
    if (par.options[0].label !== top.label && Math.abs(par.options[0].ev - top.ev) > 1e-6)
      problems.push(`parallel top "${par.options[0].label}" != serial top "${top.label}"`);
    if (afterAuto !== afterTop) problems.push(`Auto-step differs from analysis top "${top.label}"`);
    if (endRun !== endTop) problems.push(`Run to end of turn differs from playing analysis top then running`);
    const pick = serial.options.find((o) => o.rulesPick);
    if (pick && pick !== top) problems.push(`rules' pick "${pick.label}" is not listed first`);
    checked++;
    if (problems.length) {
      failed++;
      console.log(`  FAIL ${who}: ${problems.join("; ")}`);
    }
  }
}
console.log(failed ? `${failed} of ${checked} FAILED` : `ALL PASSED (consistency: ${checked} position/seat combinations)`);
if (failed) process.exit(1);
