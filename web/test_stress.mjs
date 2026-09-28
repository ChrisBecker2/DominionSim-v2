// Stress test for the page's engine exports: random seats, random actions, many games.
// Reports the first crash ("unreachable" = a Rust panic inside wasm) with the position text.
//
//   node web/test_stress.mjs [games]
import { readFileSync } from "node:fs";

const bytes = readFileSync(new URL("./dist/dominion_wasm.wasm", import.meta.url));
const mod = await WebAssembly.compile(bytes);
const enc = new TextEncoder(), dec = new TextDecoder();
const GAMES = parseInt(process.argv[2] || "40", 10);

let seed = 12345;
const rand = (n) => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed % n; };

async function fresh() {
  const w = (await WebAssembly.instantiate(mod, {})).exports;
  const rd = () => dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()));
  const rb = () => new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()).slice();
  const put = (b) => { const p = w.alloc(b.length); new Uint8Array(w.memory.buffer, p, b.length).set(b); return p; };
  return { w, rd, rb, put };
}

const worker = await fresh();
let crashes = 0;
for (let g = 0; g < GAMES && crashes === 0; g++) {
  const { w, rd, rb, put } = await fresh();
  w.list_bots(); const bots = JSON.parse(rd());
  const players = 2 + rand(5);
  for (let p = 0; p < players; p++) w.set_seat(p, rand(bots.length));
  w.seat_kingdom(players); const k = JSON.parse(rd()).kingdom || "Village, Smithy";
  const kb = enc.encode(k); const kp = put(kb);
  w.new_game(players, kp, kb.length, BigInt(g + 1), 0);
  let lastText = "", lastAction = "new_game";
  try {
    for (let step = 0; step < 4000; step++) {
      w.get_view(); const v = JSON.parse(rd());
      if (v.gameOver) break;
      w.get_state_text(); lastText = rd();
      const r = rand(100);
      if (r < 40) { lastAction = "step_auto"; w.step_auto(); }
      else if (r < 55) { lastAction = "run_to_end_of_turn"; w.run_to_end_of_turn(); }
      else if (r < 62 && v.pending && !v.pending.paused) { lastAction = "analyze"; w.analyze(); }
      else if (r < 67 && v.pending && !v.pending.paused) {
        lastAction = "parallel analyze";
        if (w.plan_start(8)) {
          const plan = JSON.parse(rd());
          w.plan_root_bytes(); const root = rb();
          for (let i = 0; i < plan.tasks; i++) {
            w.plan_task_bytes(i); const st = rb();
            const rp = worker.put(root), sp = worker.put(st);
            worker.w.eval_task(rp, sp, plan.player, 50000, plan.strategy); const res = JSON.parse(worker.rd());
            worker.w.dealloc(rp, root.length); worker.w.dealloc(sp, st.length);
            const pv = enc.encode(res.pv); const pp = put(pv);
            const ob = enc.encode(res.outcomes); const op = put(ob);
            w.plan_put_result(i, res.ev, res.exact ? 1 : 0, res.nodes, res.ttHits, pp, pv.length, op, ob.length);
          }
          w.plan_finish();
        }
      }
      else if (r < 75 && v.pending) {
        lastAction = "choose random";
        const cs = v.pending.choices; const c = cs[rand(cs.length)];
        if (c.index < 0) w.resume(); else w.choose(c.index);
      }
      else if (r < 80) { lastAction = "undo"; w.undo(); }
      else if (r < 83) { lastAction = "redo"; w.redo(); }
      else if (r < 88) {
        lastAction = "load synced text";
        const tb = enc.encode(lastText); const tp = put(tb); w.load_state(tp, tb.length);
      }
      else { lastAction = "run_bots (few)"; if (rand(10) === 0) w.run_bots(); else w.step_auto(); }
    }
  } catch (e) {
    crashes++;
    console.log(`CRASH in game ${g + 1} during "${lastAction}": ${e.message}`);
    console.log(`seats: ${JSON.stringify(Array.from({ length: players }, (_, p) => p))} kingdom: ${k}`);
    console.log("--- position before the action ---\n" + lastText);
  }
}
console.log(crashes ? "FAILED" : `ALL PASSED (stress: ${GAMES} games)`);
if (crashes) process.exit(1);
