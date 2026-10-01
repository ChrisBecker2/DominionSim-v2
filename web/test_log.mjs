// Log text for card bonuses and Duration firings, Load State pausing, and card chip classes.
//
//   node web/test_log.mjs
import { readFileSync } from "node:fs";

const bytes = readFileSync(new URL("./dist/dominion_wasm.wasm", import.meta.url));
const mod = await WebAssembly.compile(bytes);
const enc = new TextEncoder(), dec = new TextDecoder();

async function instance() {
  const w = (await WebAssembly.instantiate(mod, {})).exports;
  const rd = () => dec.decode(new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()));
  const load = (text) => {
    const b = enc.encode(text);
    const p = w.alloc(b.length);
    new Uint8Array(w.memory.buffer, p, b.length).set(b);
    const ok = w.load_state(p, b.length);
    w.dealloc(p, b.length);
    if (!ok) throw new Error(rd());
  };
  const view = () => { w.get_view(); return JSON.parse(rd()); };
  return { w, rd, load, view };
}

let failed = 0;
const check = (name, cond, extra) => {
  console.log((cond ? "  ok   " : "  FAIL ") + name);
  if (!cond) { failed++; if (extra) console.log("       " + extra); }
};
const IND = "   ";
const KINGDOM = "Village, Market, Merchant, Astrolabe, Caravan, Wharf, Monument, Steward, Smithy, Moat";
const state = (phase, p1, coins = 0) => `players: 2
kingdom: ${KINGDOM}
turn: 3  player: 1  phase: ${phase}  actions: 1  buys: 1  coins: ${coins}

[player 1]
${p1}

[player 2]
hand: 5 Copper
deck: 2 Copper, 3 Estate
`;

// --- Load State shows the position as loaded --------------------------------------------
{
  const text = state("buy", "hand: 3 Copper, Silver, Estate\ndeck: 4 Copper, 2 Silver, 3 Estate, Gold");
  const I = await instance();
  I.load(text);
  let v = I.view();
  check("loaded Buy phase: log holds only the load lines", v.log.length === 2 && v.log[0].startsWith("Loaded state"), v.log.join(" | "));
  check("loaded Buy phase: nothing auto-played (hand and coins as loaded)",
    v.players[0].hand.reduce((n, c) => n + c.count, 0) === 5 && v.turn.coins === 0 && v.players[0].inPlay.length === 0,
    JSON.stringify(v.turn));
  check("loaded Buy phase: paused at Continue", v.pending && v.pending.paused && v.pending.choices[0].label === "Continue");
  I.w.get_state_text();
  check("state text unchanged by loading", I.rd().includes("hand: 3 Copper, Silver, Estate"));
  I.w.run_to_end_of_turn();
  v = I.view();
  check("Run to end of turn from the loaded state plays the Treasures", v.log.some((l) => /plays Copper, Copper, Copper, Silver/.test(l)), v.log.join(" | "));

  // Every first action advances (and logs) the same way.
  const S = await instance();
  S.load(text); S.w.step_auto();
  check("Auto-step from the loaded state plays the Treasures first", S.view().log.some((l) => /plays Copper/.test(l)) && !S.view().pending.paused);
  const U = await instance();
  U.load(text); U.w.resume(); U.w.undo();
  check("Undo after Continue returns to the loaded position", U.view().pending.paused && U.view().players[0].hand.reduce((n, c) => n + c.count, 0) === 5);

  // Analyze: the page continues first, then analyzes the decision that follows.
  const A = await instance(), B = await instance();
  A.load(text); A.w.resume(); A.w.analyze(); const ra = JSON.parse(A.rd());
  B.load(text); B.w.step_auto(); B.w.analyze(); const rb = JSON.parse(B.rd());
  check("Analyze from the loaded state = analyze after advancing",
    ra.options.length > 0 && JSON.stringify(ra.options.map((o) => [o.label, o.ev])) === JSON.stringify(rb.options.map((o) => [o.label, o.ev])));
  const C = await instance();
  C.load(text);
  check("Analyze refuses to run while paused (the page continues first)", !C.w.analyze());

  const D = await instance();
  D.load(state("action", "hand: Village, Smithy, 3 Copper\ndeck: 4 Copper, 2 Silver, 3 Estate, Gold"));
  v = D.view();
  check("loaded at a real decision: shown as is, no pause", v.pending && !v.pending.paused && v.pending.choices.some((c) => c.label === "Play Village") && v.log.length === 2);
}

// --- Bonus lines -----------------------------------------------------------------------
async function play(text, label) {
  const I = await instance();
  I.load(text);
  if (I.view().pending.paused) I.w.resume();
  const c = I.view().pending.choices.find((x) => x.label === label);
  if (!c) throw new Error("no choice " + label + ": " + JSON.stringify(I.view().pending.choices));
  I.w.choose(c.index);
  return { I, log: I.view().log };
}
{
  const filler = "deck: 6 Copper, 3 Estate, Gold";
  let { log } = await play(state("action", `hand: Village, Smithy, 3 Copper\n${filler}`), "Play Village");
  const i = log.findIndex((l) => l === "Player 1 plays Village");
  check("Village shows +2 Actions under its play", i >= 0 && log[i + 1] === IND + "+2 Actions", log.join(" | "));
  check("no +N Cards lines", !log.some((l) => /\+\d+ Cards?/.test(l)));
  check("draws stay as indented draw lines", log[i + 2] === IND + "Player 1 draws Copper", log[i + 2]);

  ({ log } = await play(state("action", `hand: Market, Smithy, 3 Copper\n${filler}`), "Play Market"));
  check("Market shows one combined line", log.includes(IND + "+1 Action, +1 Buy, +$1"), log.join(" | "));

  ({ log } = await play(state("action", `hand: Merchant, Silver, Silver, Estate\n${filler}`), "Play Merchant"));
  check("Merchant: its own +1 Action under it", log.includes(IND + "+1 Action"), log.join(" | "));
  check("Merchant's +$1 appears under the first Silver, once", log.filter((l) => l === IND + "Merchant: +$1").length === 1, log.join(" | "));
  check("Silver's printed value is not printed as a bonus", !log.some((l) => /^ *\+\$2/.test(l)));

  ({ log } = await play(state("action", `hand: Monument, Estate\n${filler}`), "Play Monument"));
  check("Monument shows +$2 and +1 VP", log.includes(IND + "+$2") && log.includes(IND + "+1 VP"), log.join(" | "));

  const { I } = await play(state("action", `hand: Steward, Estate\n${filler}`), "Play Steward");
  const co = I.view().pending.choices.findIndex((c) => /\$2/.test(c.label));
  I.w.choose(co >= 0 ? co : 0);
  check("Steward's mode choice prints its +$2", I.view().log.includes(IND + "+$2"), I.view().log.join(" | "));
}

// --- Durations at the start of the turn --------------------------------------------------
{
  const text = state("action", "hand: Estate, Estate, Copper, Copper, Copper\ndeck: Gold, Silver, 5 Copper\ndurations: Astrolabe, Caravan, Wharf");
  const I = await instance();
  I.load(text);
  check("Duration positions load paused", I.view().pending.paused && I.view().log.length === 2);
  I.w.resume();
  const log = I.view().log;
  const a = log.findIndex((l) => l === "Player 1's Astrolabe (duration): +$1, +1 Buy");
  check("Astrolabe at turn start shows +$1, +1 Buy", a >= 0, log.join(" | "));
  const c = log.findIndex((l) => l === "Player 1's Caravan (duration)");
  check("Caravan firing is logged", c >= 0, log.join(" | "));
  check("Caravan's draw follows, indented", /^ {3}Player 1 draws \w+$/.test(log[c + 1] || ""), log[c + 1]);
  const w = log.findIndex((l) => l === "Player 1's Wharf (duration): +1 Buy");
  check("Wharf shows +1 Buy then draws two cards", w >= 0 && /^ {3}Player 1 draws \w+, \w+$/.test(log[w + 1] || ""), log.join(" | "));
  check("no +N Cards lines at turn start", !log.some((l) => /\+\d+ Cards?/.test(l)));
}

// --- A played Caravan, through a whole round -----------------------------------------------
{
  const I = await instance();
  const text = `players: 2
kingdom: ${KINGDOM}
turn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: Caravan, 4 Copper
deck: 4 Copper, 2 Silver, 3 Estate, Gold, 5 Copper

[player 2]
hand: 5 Copper
deck: 5 Copper, 3 Estate, 2 Copper
`;
  I.load(text); I.w.resume(); I.w.choose(0);
  I.w.run_to_end_of_turn(); I.w.run_to_end_of_turn(); I.w.run_to_end_of_turn();
  const log = I.view().log;
  const c = log.findIndex((l) => l === "Player 1's Caravan (duration)");
  check("a played Caravan's start-of-turn draw shows in the log after its firing", c >= 0 && /draws Copper/.test(log[c + 1]), log.join(" | "));
}

// --- Card chip classes -----------------------------------------------------------------
{
  const src = readFileSync(new URL("./app.js", import.meta.url), "utf8");
  const m = src.match(/\/\/ <chip-class>[^\n]*\n([\s\S]*?)\/\/ <\/chip-class>/);
  check("app.js exposes the chip-class function", !!m);
  const chipClass = new Function(m[1] + "\nreturn chipClass;")();
  const I = await instance();
  I.w.card_info();
  const types = Object.fromEntries(JSON.parse(I.rd()).map((c) => [c.name, c.types]));
  const cls = (n) => chipClass(types[n]).split(" ");
  check("Astrolabe: split orange/gold", cls("Astrolabe").includes("dual-duration-treasure") && cls("Astrolabe").includes("duration"), chipClass(types["Astrolabe"]));
  check("Pirate: split orange/blue", cls("Pirate").includes("dual-duration-reaction"), chipClass(types["Pirate"]));
  check("Caravan: solid orange", cls("Caravan").includes("duration") && !cls("Caravan").some((x) => x.startsWith("dual")), chipClass(types["Caravan"]));
  check("Moat: solid blue", cls("Moat").includes("reaction") && !cls("Moat").some((x) => x.startsWith("dual")), chipClass(types["Moat"]));
  check("Harem still treasure/victory", cls("Harem").includes("dual-treasure-victory"));
  const css = readFileSync(new URL("./style.css", import.meta.url), "utf8");
  check("split chip CSS uses the duration/treasure and duration/reaction tokens",
    /\.card\.dual-duration-treasure\s*\{[^}]*var\(--c-duration\)[^}]*var\(--c-treasure\)/.test(css) &&
    /\.card\.dual-duration-reaction\s*\{[^}]*var\(--c-duration\)[^}]*var\(--c-reaction\)/.test(css));
  const dark = css.slice(css.indexOf("prefers-color-scheme: dark"));
  check("light and dark tokens exist for duration, treasure, reaction", ["--c-duration", "--c-treasure", "--c-reaction"].every((t) => css.includes(t + ":") && dark.includes(t + ":")));
}

console.log(failed ? `${failed} FAILED` : "ALL PASSED (log)");
if (failed) process.exit(1);
