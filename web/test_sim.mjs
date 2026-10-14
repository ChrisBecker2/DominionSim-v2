// Checks the Simulation panel's data: the fields wasm `simulate` returns are all present and
// consistent, merging results from several workers equals one run, and the numbers the charts
// show (web/simstats.js) agree with the sums.
//
//   node web/test_sim.mjs
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

const SS = createRequire(import.meta.url)("./simstats.js");
const wasmBytes = readFileSync(new URL("./dist/dominion_wasm.wasm", import.meta.url));
const w = (await WebAssembly.instantiate(wasmBytes, {})).instance.exports;
w.init();
let failed = 0;
const check = (name, cond) => { console.log((cond ? "  ok   " : "  FAIL ") + name); if (!cond) failed++; };
const near = (a, b, eps = 1e-9) => Math.abs(a - b) <= eps * Math.max(1, Math.abs(a), Math.abs(b));
const sum = (a) => (a || []).reduce((s, x) => s + x, 0);

// A kingdom with no gainers/trashers of Provinces, so Provinces owned == Provinces bought.
const kingdom = new TextEncoder().encode("Smithy, Village, Market, Moat, Cellar, Militia, Laboratory, Festival, Chapel, Library");
const kp = w.alloc(kingdom.length);
new Uint8Array(w.memory.buffer, kp, kingdom.length).set(kingdom);
w.new_game(2, kp, kingdom.length, 3n, 0);
w.set_seat(0, 4);
w.set_seat(1, 2);
w.state_bytes_current();
const state = new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()).slice();
const sp = w.alloc(state.length);
new Uint8Array(w.memory.buffer, sp, state.length).set(state);
const text = () => new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()));
function sim(games, seed, flags, first) {
  const ok = w.simulate(sp, games, seed, flags, first);
  const out = text();
  if (!ok) throw new Error(out);
  return JSON.parse(out);
}

const N = 600;
const full = sim(N, 7, 3, 0);
const r = SS.mergeSims([full], 2);

console.log("fields");
const arrays = ["vp", "vpSq", "money", "moneySq", "buys", "buysSq", "count", "provCum", "duchyCum", "colonyCum", "winsAt"];
check("every per-player array is present", r.players.every((q) => arrays.every((f) => Array.isArray(q[f]) && q[f].length > 0)));
check("every per-player number is present", r.players.every((q) => ["wins", "winsSq", "firstGames", "firstWins"].every((f) => typeof q[f] === "number")));
check("pairs, lengthHist and fresh are present", r.pairs.length === 1 && r.lengthHist.length > 0 && r.fresh === true);
check("no sum is negative", r.players.every((q) => arrays.every((f) => q[f].every((x) => x >= 0 || f === "vp" || f === "vpSq"))) && r.lengthHist.every((x) => x >= 0));
check("squares are at least the square of the sum / count", r.players.every((q) => q.count.every((c, i) => c === 0 || q.vpSq[i] * c + 1e-6 >= q.vp[i] * q.vp[i] && q.moneySq[i] * c + 1e-6 >= q.money[i] * q.money[i])));
check("every game plays turn 1", r.players.every((q) => q.count[0] === N));
check("counts never rise with the turn", r.players.every((q) => q.count.every((c, i) => i === 0 || c <= q.count[i - 1])));

console.log("shares");
const finished = r.games - r.capped;
check("wins add up to the finished games (shares sum to 1)", near(sum(r.players.map((q) => q.wins)), finished));
check("winsAt adds up to wins", r.players.every((q) => near(sum(q.winsAt), q.wins)));
check("wins squared never exceed wins", r.players.every((q) => q.winsSq <= q.wins + 1e-9));
check("game lengths add up to the finished games", near(sum(r.lengthHist), finished));
check("lengthSum matches the histogram", near(r.lengthSum, r.lengthHist.reduce((s, c, i) => s + c * i, 0)));
const ws = SS.winStats(r);
check("win rates sum to 1", near(sum(ws.seats.map((s) => s.p)), finished / N));
check("each win rate lies inside its interval", ws.seats.every((s) => s.lo <= s.p && s.p <= s.hi && s.lo >= 0 && s.hi <= 1));
check("the margin is the difference of win rates (points)", near(ws.margins[0].d, 100 * (ws.seats[0].p - ws.seats[1].p)));
check("the margin interval brackets the margin", ws.margins[0].lo <= ws.margins[0].d && ws.margins[0].d <= ws.margins[0].hi);
const cum = SS.cumulativeWins(r);
check("cumulative win probability ends at the win rate", cum.every((pts, p) => near(pts[pts.length - 1].y, 100 * ws.seats[p].p)));
check("cumulative win probability never falls", cum.every((pts) => pts.every((pt, i) => i === 0 || pt.y >= pts[i - 1].y)));
const ld = SS.lengthDistribution(r);
check("game length distribution sums to 100%", near(sum(ld.pts.map((p) => p.y)), 100) && near(ld.mean, r.lengthSum / finished));

console.log("first-player split");
check("seating is random from a new game (each seat first about half the time)", r.players.every((q) => q.firstGames > 0.4 * N && q.firstGames < 0.6 * N) && sum(r.players.map((q) => q.firstGames)) === N);
check("split adds up to the overall rate", ws.seats.every((s, p) => near(s.first.p * s.first.n + s.second.p * s.second.n, r.players[p].wins) && s.first.n + s.second.n === N));
check("first-player wins add up to the first player's rate", near(ws.firstAll.p * N, sum(r.players.map((q) => q.firstWins))));
const fixed = SS.mergeSims([sim(100, 7, 0, 0)], 2);
w.set_seat(0, 2); w.set_seat(1, 4);
const swapped = SS.winStats(SS.mergeSims([sim(N, 7, 0, 0)], 2));
w.set_seat(0, 4); w.set_seat(1, 2);
check("results are reported per seat: swapping the controllers swaps the win rates", Math.abs(swapped.seats[1].p - ws.seats[0].p) < 0.1 && Math.abs(ws.seats[0].p - 0.5) > 0.05 || Math.abs(ws.seats[0].p - 0.5) <= 0.05);

console.log("VP difference");
const diff = SS.vpDifference(r)[0];
const means = SS.meanSeries(r, "vp", "vpSq");
check("VP difference equals the difference of means (turns both played equally often)", diff.pts.length > 5 && diff.pts.every((p) => {
  const i = p.x - 1;
  if (r.players[0].count[i] !== r.pairs[0].n[i] || r.players[1].count[i] !== r.pairs[0].n[i]) return true;
  const a = means[0].find((q) => q.x === p.x), b = means[1].find((q) => q.x === p.x);
  return near(p.y, a.y - b.y);
}));
check("pairs are counted at most once per game", r.pairs[0].n.every((c) => c <= N));
check("at least some turns compare exactly", diff.pts.some((p) => r.players[0].count[p.x - 1] === r.pairs[0].n[p.x - 1]));
check("spread is non-negative", diff.pts.every((p) => p.sd >= 0 && p.lo <= p.y && p.y <= p.hi));

console.log("deck composition");
check("owned is present with the flag and absent without", r.players.every((q) => q.owned) && fixed.players.every((q) => !q.owned));
const cards = SS.deckSeries(r, 0.5, 12);
check("some cards are shown, most-owned first", cards.length > 3 && cards.every((c, i) => i === 0 || cards[i - 1].peak >= c.peak));
check("Copper is shown", cards.some((c) => c.name === "Copper"));
const total = r.players.map((q) => q.count.map((c, i) => Object.values(q.owned).reduce((s, a) => s + (a[i] || 0), 0) / c));
// Late turns average only the few games still running, so their mean can dip: compare turns most games reach.
check("a seat owns at least its 10 starting cards, more every turn", total.every((t, s) => t[0] >= 10 && t.every((x, i) => i === 0 || r.players[s].count[i] < 30 || x >= t[i - 1] - 1e-9)));
check("Provinces owned equal Provinces bought (no gainers in this kingdom)", r.players.every((q) => q.owned.Province.every((x, i) => near(x, q.provCum[i]))));
check("run with deck tracking has the same results otherwise", (() => {
  const a = SS.mergeSims([sim(100, 9, 2, 0)], 2), b = SS.mergeSims([sim(100, 9, 3, 0)], 2);
  b.players.forEach((q) => delete q.owned);
  a.players.forEach((q) => delete q.owned);
  return JSON.stringify(a) === JSON.stringify(b);
})());

console.log("greening");
const carried = SS.carriedSeries(r, "provCum", "provEnd");
check("carried Province averages never fall", carried.every((pts) => pts.length > 5 && pts.every((pt, i) => i === 0 || pt.y >= pts[i - 1].y - 1e-12)));
check("turn 1 is the plain average", carried.every((pts, p) => near(pts[0].y, r.players[p].provCum[0] / N)));
check("the final totals add up to at most the 8 Provinces of a 2-player game per game", sum(r.players.map((q) => sum(q.provEnd))) <= 8 * N);
check("the carried average ends at the average final total", carried.every((pts, p) => pts[pts.length - 1].y <= sum(r.players[p].provEnd) / N + 1e-9));
check("Duchies and Colonies have the same fields", ["duchyEnd", "colonyEnd"].every((f) => r.players.every((q) => q[f].length > 0 || f === "colonyEnd")));

console.log("merging");
const A = sim(250, 7, 3, 0), B = sim(350, 7, 3, 250);
check("two halves merge to the one run", JSON.stringify(SS.mergeSims([A, B], 2)) === JSON.stringify(r));
check("merge order does not matter", JSON.stringify(SS.mergeSims([B, A], 2)) === JSON.stringify(r));
const C = sim(100, 7, 3, 0), D = sim(200, 7, 3, 100), E = sim(300, 7, 3, 300);
check("three parts merge to the one run", JSON.stringify(SS.mergeSims([C, D, E], 2)) === JSON.stringify(r));
check("the same call twice gives the same answer", JSON.stringify(sim(50, 3, 3, 0)) === JSON.stringify(sim(50, 3, 3, 0)));
const noDeck = SS.mergeSims([sim(100, 7, 2, 0), sim(100, 7, 2, 100)], 2);
check("merging runs without deck tracking leaves owned unset", noDeck.players.every((q) => q.owned === null));
const single = SS.mergeSims([{ games: 1, capped: 0, lengthSum: 20, finished: true, players: [{ vp: [3, 5], money: [7, 8], buys: [1, 1], count: [1, 1], wins: 1, winsAt: [0, 1] }, { vp: [3, 4], money: [7, 8], buys: [1, 1], count: [1, 1], wins: 0, winsAt: [] }] }], 2);
check("a single-game record (older fields only) merges", single.games === 1 && single.fresh === false && single.players[0].vpSq.length === 0 && SS.meanSeries(single, "vp", "vpSq")[0].length === 2);

console.log("worker ranges");
const per = [0, 1, 2, 3].map((i) => sim(150, 7, 3, 150 * i));
check("any number of workers gives the same result", JSON.stringify(SS.mergeSims(per, 2)) === JSON.stringify(r));

console.log("curses");
check("no Curses without a Curse giver in the kingdom", r.players.every((q) => q.curse.every((x) => x === 0) && q.curseEnd.every((x) => x === 0)));
{
  // Double Witch against Big Money Ultimate on a kingdom with Witch: Curses owned match the deck record.
  const wk = new TextEncoder().encode("Witch, Village, Market, Moat, Cellar, Militia, Laboratory, Festival, Smithy, Library");
  const wp = w.alloc(wk.length);
  new Uint8Array(w.memory.buffer, wp, wk.length).set(wk);
  w.new_game(2, wp, wk.length, 5n, 0);
  w.set_seat(0, 4);
  w.set_seat(1, 2);
  w.state_bytes_current();
  const st = new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()).slice();
  const p2 = w.alloc(st.length);
  new Uint8Array(w.memory.buffer, p2, st.length).set(st);
  if (!w.simulate(p2, 200, 11, 1, 0)) throw new Error(text());
  const c = SS.mergeSims([JSON.parse(text())], 2);
  check("Curses owned equal the deck record's Curses", c.players.every((q) => q.curse.every((x, i) => near(x, (q.owned.Curse || [])[i] || 0))));
  const seen = SS.carriedSeries(c, "curse", "curseEnd");
  check("Big Money Ultimate ends with more Curses than Double Witch", seen[1][seen[1].length - 1].y > seen[0][seen[0].length - 1].y + 1);
  check("at most 10 Curses per game in a 2-player game", sum(c.players.map((q) => sum(q.curseEnd))) <= 10 * 200);
}

if (failed) { console.log(`${failed} FAILED`); process.exit(1); } else console.log("ALL PASSED (sim)");
