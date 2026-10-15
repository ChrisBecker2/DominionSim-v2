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
  // The page asks which Treasures to play (one by one, or Done for the rest): the loaded Buy phase is that decision, not a pause.
  check("loaded Buy phase: the Treasures are offered one by one", v.pending && !v.pending.paused && v.pending.choices.some((c) => c.label === "Play Copper") && v.pending.choices.some((c) => c.label === "Play Silver"),
    JSON.stringify(v.pending && v.pending.choices.map((c) => c.label)));
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
  U.load(text); U.w.step_auto(); U.w.undo();
  check("Undo after the first step returns to the loaded position", U.view().players[0].hand.reduce((n, c) => n + c.count, 0) === 5 && U.view().pending.choices.some((c) => c.label === "Play Copper"));

  // Analyze runs on the Treasure decision as loaded.
  const A = await instance();
  A.load(text);
  check("Analyze runs on the loaded Treasure decision", !!A.w.analyze() && JSON.parse(A.rd()).options.length > 0);

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

  {
    // The Buy phase then asks for the Treasures: Done plays them (Auto-step's choice).
    const r = await play(state("action", `hand: Merchant, Silver, Silver, Estate\n${filler}`), "Play Merchant");
    r.I.w.step_auto();
    log = r.I.view().log;
  }
  check("Merchant: its own +1 Action under it", log.includes(IND + "+1 Action"), log.join(" | "));
  check("Merchant's +$1 appears under the first Silver, once", log.filter((l) => l === IND + "Merchant: +$1").length === 1, log.join(" | "));
  check("Silver's printed value is not printed as a bonus", !log.some((l) => /^ *\+\$2/.test(l)));
  // Treasures other than Copper, Silver, Gold, Platinum and Potion show their $ under the play; two Fool's Golds: +$1, then +$4.
  {
    const text = `players: 2
kingdom: Fool's Gold, Harem, Village, Smithy, Market, Moat, Cellar, Festival, Laboratory, Mine
turn: 3  player: 1  phase: buy  actions: 0  buys: 1  coins: 0

[player 1]
hand: 2 Fool's Gold, Harem, Copper, Silver
deck: 5 Copper

[player 2]
hand: 5 Copper
deck: 5 Estate
`;
    const F = await instance();
    F.load(text);
    if (F.view().pending.paused) F.w.resume();
    const pick = (label) => { const c = F.view().pending.choices.find((x) => x.label === label); if (!c) throw new Error(label + ": " + JSON.stringify(F.view().pending.choices.map((x) => x.label))); F.w.choose(c.index); };
    pick("Play Copper"); pick("Play Fool's Gold"); pick("Play Fool's Gold"); pick("Play Harem"); pick("Play Silver");
    const flog = F.view().log;
    const plays = flog.map((l, i) => [l, i]).filter(([l]) => l === "Player 1 plays Fool's Gold").map(([, i]) => i);
    check("two Fool's Gold plays, each on its own line", plays.length === 2, flog.join(" | "));
    check("the first Fool's Gold shows +$1, the second +$4", flog[plays[0] + 1] === IND + "+$1" && flog[plays[1] + 1] === IND + "+$4", flog.join(" | "));
    check("Harem shows +$2", flog.some((l, i) => l === "Player 1 plays Harem" && flog[i + 1] === IND + "+$2"), flog.join(" | "));
    check("Copper and Silver show no bonus", !flog.some((l, i) => /plays (Copper|Silver)$/.test(l) && /^ *\+\$/.test(flog[i + 1] || "")), flog.join(" | "));
    check("the $ add up", F.view().turn.coins === 1 + 1 + 4 + 2 + 2, String(F.view().turn.coins));
    // Done plays the rest at once: still one line per Fool's Gold, each with its own $.
    const G = await instance();
    G.load(text);
    if (G.view().pending.paused) G.w.resume();
    G.w.choose(G.view().pending.choices.find((x) => /^Done/.test(x.label)).index);
    const glog = G.view().log;
    const gplays = glog.map((l, i) => [l, i]).filter(([l]) => l === "Player 1 plays Fool's Gold").map(([, i]) => i);
    check("Done: each Fool's Gold on its own line, +$1 then +$4", gplays.length === 2 && glog[gplays[0] + 1] === IND + "+$1" && glog[gplays[1] + 1] === IND + "+$4", glog.join(" | "));
  }

  // Three Collections in play, then an Action bought: one line naming them and the count.
  {
    const text = `players: 2
kingdom: Collection, Witch, Village, Smithy, Market, Moat, Cellar, Festival, Laboratory, Mine
turn: 1  player: 1  phase: buy  actions: 0  buys: 1  coins: 0

[player 1]
hand: 3 Collection, Copper, Copper
deck: 5 Copper

[player 2]
hand: 5 Copper
deck: 5 Estate
`;
    const C = await instance();
    C.load(text);
    if (C.view().pending.paused) C.w.resume();
    // The page asks for the Treasures first: Done plays them.
    const done = C.view().pending.choices.find((x) => /^Done/.test(x.label));
    if (done && C.view().pending.choices.some((x) => /^Play /.test(x.label))) C.w.choose(done.index);
    const buy = C.view().pending.choices.find((x) => x.label === "Buy Witch");
    C.w.choose(buy.index);
    const clog = C.view().log;
    check("3 Collections' VP on one line naming them", clog.includes(IND + "Collection ×3: +3 VP"), clog.join(" | "));
    check("no bare +3 VP line", !clog.some((l) => l.trim() === "+3 VP"));
  }

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

// --- Two Samurai at the start of the turn: the engine merges them into one +$2, the log names both ---
{
  const text = `players: 2
kingdom: Samurai, Smithy, Village, Market, Militia, Moat, Cellar, Workshop, Remodel, Mine
turn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: Estate, Estate, Copper, Copper, Copper
deck: 5 Copper
durations: Samurai, Samurai

[player 2]
hand: 5 Copper
deck: 5 Copper
`;
  const I = await instance();
  I.load(text);
  if (I.view().pending.paused) I.w.resume();
  const log = I.view().log;
  check("two Samurai: 'Samurai ×2 (duration): +$2'", log.includes("Player 1's Samurai ×2 (duration): +$2"), log.join(" | "));
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
  check("Fool's Gold: split gold/blue (Treasure-Reaction)", cls("Fool's Gold").includes("dual-treasure-reaction") && cls("Fool's Gold").includes("treasure"), chipClass(types["Fool's Gold"]));
  check("Tunnel: split green/blue (Victory-Reaction)", cls("Tunnel").includes("dual-victory-reaction"), chipClass(types["Tunnel"]));
  // Prince is an Action-Duration-Command since the 2022 errata: orange like the other Durations.
  check("Prince: solid orange (a Duration)", cls("Prince").includes("duration") && !cls("Prince").some((x) => x.startsWith("dual")), chipClass(types["Prince"]));
  check("card_info carries the Reward and Command types", types["Courser"].includes("reward") && types["Prince"].includes("command"));
  const css = readFileSync(new URL("./style.css", import.meta.url), "utf8");
  check("split chip CSS uses the duration/treasure and duration/reaction tokens",
    /\.card\.dual-duration-treasure\s*\{[^}]*var\(--c-duration\)[^}]*var\(--c-treasure\)/.test(css) &&
    /\.card\.dual-duration-reaction\s*\{[^}]*var\(--c-duration\)[^}]*var\(--c-reaction\)/.test(css));
  const dark = css.slice(css.indexOf("prefers-color-scheme: dark"));
  check("light and dark tokens exist for duration, treasure, reaction", ["--c-duration", "--c-treasure", "--c-reaction"].every((t) => css.includes(t + ":") && dark.includes(t + ":")));
}

// --- Dark Ages: chips, the Knights and Ruins piles in the supply, Shelters in the starting decks -----
{
  const src = readFileSync(new URL("./app.js", import.meta.url), "utf8");
  const a = src.indexOf("// <chip-class>");
  const z = src.indexOf("// </chip-class>");
  const body = src.slice(src.indexOf("\n", a) + 1, z);
  const chipClass = new Function(body + "\nreturn chipClass;")();
  const I = await instance();
  I.w.card_info();
  const types = Object.fromEntries(JSON.parse(I.rd()).map((c) => [c.name, c.types]));
  const cls = (n) => chipClass(types[n]).split(" ");
  check("Ruins: their own colour", cls("Survivors").includes("ruins") && cls("Abandoned Mine").includes("ruins"));
  check("Shelters carry the shelter mark over their own type", cls("Hovel").includes("reaction") && cls("Hovel").includes("shelter") && cls("Overgrown Estate").includes("victory") && cls("Necropolis").includes("action") && cls("Necropolis").includes("shelter"));
  check("Knights are attacks; Dame Josephine is also a Victory card", cls("Sir Martin").includes("attack") && cls("Dame Josephine").includes("dual-action-victory"));
  check("card_info carries the Dark Ages types", types["Cultist"].includes("looter") && types["Sir Vander"].includes("knight") && types["Hovel"].includes("shelter"));
  const css = readFileSync(new URL("./style.css", import.meta.url), "utf8");
  const dark = css.slice(css.indexOf("prefers-color-scheme: dark"));
  check("light and dark tokens exist for ruins and the shelter edge", ["--c-ruins", "--c-shelter-edge"].every((t) => css.includes(t + ":") && dark.includes(t + ":")));

  // A Dark Ages kingdom: the Knights and Ruins are one entry each, with the top card and the count; the starting decks have the Shelters.
  const names = "Knights, Cultist, Hermit, Urchin, Bandit Camp, Rats, Market Square, Forager, Armory, Sage";
  const k = enc.encode(names);
  const p = I.w.alloc(k.length);
  new Uint8Array(I.w.memory.buffer, p, k.length).set(k);
  I.w.new_game(2, p, k.length, 5n, 0);
  let v = I.view();
  const pile = (name) => v.supply.find((c) => c.pile === name);
  check("the Knights pile is one supply entry (the top Knight, 10 left)", pile("Knights") && pile("Knights").count === 10 && /^(Dame|Sir) /.test(pile("Knights").name), JSON.stringify(pile("Knights")));
  check("the Ruins pile is one supply entry (the top Ruins, 10 left)", pile("Ruins") && pile("Ruins").count === 10 && types[pile("Ruins").name].includes("ruins"), JSON.stringify(pile("Ruins")));
  check("no other Knight or Ruins is listed", v.supply.filter((c) => types[c.name].includes("knight") || types[c.name].includes("ruins")).length === 2);
  check("Spoils, Madman and Mercenary are listed as piles outside the Supply", ["Spoils", "Madman", "Mercenary"].every((n) => v.supply.some((c) => c.name === n && c.side)));
  I.w.resume();
  v = I.view();
  const owned = (pl) => pl.hand.concat(pl.inPlay, pl.deckUnknown, pl.deckTop).reduce((n, c) => n + c.count, 0);
  const has = (pl, name) => pl.hand.concat(pl.inPlay, pl.deckUnknown).some((c) => c.name === name);
  check("the decks hold Hovel, Necropolis and Overgrown Estate instead of Estates", v.players.every((pl) => ["Hovel", "Necropolis", "Overgrown Estate"].every((n) => has(pl, n)) && !has(pl, "Estate")), JSON.stringify(v.players[0].hand));
  check("padded_kingdom draws Dark Ages cards for set bit 128", (() => { I.w.padded_kingdom(2, 128, 3n); const r = JSON.parse(I.rd()); return r.kingdom.split(", ").length >= 10; })());
}

// New game waits before the deal: Simulate from here deals fresh opening hands per game.
{
  const { w, view } = await instance();
  const k = enc.encode("Village, Smithy");
  const p = w.alloc(k.length);
  new Uint8Array(w.memory.buffer, p, k.length).set(k);
  w.new_game(2, p, k.length, 5n, 0);
  let v = view();
  const handSize = (pl) => pl.hand.reduce((n, c) => n + c.count, 0);
  check("new game waits before the deal ('Deal')", v.pending && v.pending.paused && v.pending.choices[0].label === "Deal",
    JSON.stringify(v.pending));
  check("no opening hands yet", v.players.every((pl) => handSize(pl) === 0));
  w.resume();
  v = view();
  // (Treasures may already be in play: with no Actions, turn 1 starts in the Buy phase.)
  const dealt = (pl) => handSize(pl) + pl.inPlay.reduce((n, c) => n + c.count, 0);
  check("Deal draws 5 cards each and starts turn 1", v.players.every((pl) => dealt(pl) === 5) && !(v.pending && v.pending.paused),
    JSON.stringify(v.players.map(dealt)));
}

// Simulate from a new game seats the players randomly (who goes first is random): a Big Money
// mirror then splits about evenly; from turn 1 already started, player 1 keeps the first move.
{
  const { w, rd } = await instance();
  const sim = () => {
    w.state_bytes_current();
    const st = new Uint8Array(w.memory.buffer, w.result_ptr(), w.result_len()).slice();
    const p = w.alloc(st.length);
    new Uint8Array(w.memory.buffer, p, st.length).set(st);
    const ok = w.simulate(p, 2000, 7);
    w.dealloc(p, st.length);
    if (!ok) throw new Error(rd());
    const r = JSON.parse(rd());
    return r.players[0].wins / (r.players[0].wins + r.players[1].wins);
  };
  w.list_bots();
  const bmu = JSON.parse(rd()).indexOf("Big Money Ultimate");
  w.set_seat(0, bmu); w.set_seat(1, bmu);
  const k = enc.encode("Village, Smithy");
  const p = w.alloc(k.length);
  new Uint8Array(w.memory.buffer, p, k.length).set(k);
  w.new_game(2, p, k.length, 9n, 0);
  const fromNew = sim();
  w.resume();
  const fromDealt = sim();
  check("mirror from a new game: about 50/50 (random seating)", Math.abs(fromNew - 0.5) < 0.04, fromNew.toFixed(3));
  check("mirror after the deal: player 1 moves first and wins more", fromDealt > fromNew, `${fromDealt.toFixed(3)} vs ${fromNew.toFixed(3)}`);
}

// --- Prince: the start-of-turn play is logged under its Prince and the mat is in the view -------
{
  const I = await instance();
  I.load(`players: 2
kingdom: Prince, Smithy, Village, Sea Witch, Haven
turn: 3  player: 1  phase: action  actions: 1  buys: 1  coins: 0

[player 1]
hand: Estate, Estate
deck top: Copper, Copper, Copper
deck: 3 Estate
set aside: Gold
durations: Haven (Gold), Sea Witch
prince: Smithy
in play: Prince
prince stays: Prince

[player 2]
hand: 5 Copper
deck: 2 Copper, 3 Estate
`);
  let v = I.view();
  check("view shows the Prince mat", v.players[0].princeCards.length === 1 && v.players[0].princeCards[0].name === "Smithy",
    JSON.stringify(v.players[0].princeCards));
  I.w.resume();
  v = I.view();
  const asked = v.pending && /start-of-turn effect resolves next/.test(v.pending.description);
  check("start of turn: asks which effect resolves next", asked, v.pending && v.pending.description);
  const labels = (v.pending ? v.pending.choices : []).map((c) => c.label);
  check("the choices name each effect", labels.includes("Haven: put Gold into your hand") && labels.includes("Prince: play Smithy") &&
    labels.some((l) => l.startsWith("Sea Witch: +2 Cards, then discard 2")), labels.join(" | "));
  const smithy = v.pending.choices.find((c) => c.label === "Prince: play Smithy");
  I.w.choose(smithy.index);
  v = I.view();
  check("log: Prince plays its card at the start of the turn", v.log.some((l) => l === "Player 1's Prince plays Smithy"), v.log.join(" | "));
}

// --- Cornucopia & Guilds: Coffers, the piles outside the Supply, Overpay ------------------------
{
  const I = await instance();
  I.load(`players: 2
kingdom: Young Witch, Joust, Ferryman, Farrier, Smithy, Village
bane: Hamlet
ferryman: Shop
side piles: Shop=9, Coronet=1, Courser=0, Demesne=1, Housecarl=1, Huge Turnip=1, Renown=1
turn: 3  player: 1  phase: buy  actions: 0  buys: 1  coins: 5

[player 1]
hand: Estate
deck: 5 Copper
coffers: 2
farmhands: Village

[player 2]
hand: 5 Copper
deck: 5 Estate
`);
  let v = I.view();
  const side = v.supply.filter((c) => c.side).map((c) => c.name);
  check("the piles outside the Supply are listed and marked", side.includes("Shop") && side.includes("Courser") && side.length === 7, side.join(","));
  check("the Bane pile is marked", v.supply.some((c) => c.name === "Hamlet" && c.bane));
  check("the view shows the Coffers", v.players[0].coffers === 2 && v.players[0].farmCards.length === 1);
  const labels = v.pending.choices.map((c) => c.label);
  check("the Buy phase offers spending a token", labels.includes("Spend a Coffers token (+$1)"), labels.join(" | "));
  const farrier = v.pending.choices.find((c) => c.label === "Buy Farrier");
  I.w.choose(farrier.index);
  v = I.view();
  check("overpaying is asked after the buy", /overpay for Farrier by up to \$3/.test(v.pending.description), v.pending.description);
  const o = v.pending.choices.map((c) => c.label);
  check("the overpay choices", o[0] === "Don't overpay" && o.includes("Overpay $3"), o.join(" | "));
  I.w.choose(v.pending.choices.find((c) => c.label === "Overpay $2").index);
  v = I.view();
  check("log: the overpay", v.log.some((l) => l.trim() === "Player 1 overpays $2 for Farrier"), v.log.join(" | "));
}

// --- Nocturne: chips, the Night phase, Boons, Hexes and States in the view and the log --------------------
{
  const src = readFileSync(new URL("./app.js", import.meta.url), "utf8");
  const a = src.indexOf("// <chip-class>");
  const z = src.indexOf("// </chip-class>");
  const chipClass = new Function(src.slice(src.indexOf("\n", a) + 1, z) + "\nreturn chipClass;")();
  const I = await instance();
  I.w.card_info();
  const types = Object.fromEntries(JSON.parse(I.rd()).map((c) => [c.name, c.types]));
  const cls = (n) => chipClass(types[n]).split(" ");
  check("a Night card is dark", cls("Monastery").includes("night") && cls("Bat").includes("night"));
  check("a Night-Duration card is split orange/dark", cls("Ghost Town").includes("dual-duration-night") && cls("Raider").includes("dual-duration-night"));
  check("Werewolf stays an Action chip; Heirlooms are marked", cls("Werewolf").includes("action") && cls("Goat").includes("heirloom") && cls("Goat").includes("treasure"));
  check("Boons, Hexes and States have chips of their own", cls("The Sea's Gift").includes("boon") && cls("War").includes("hex") && cls("Deluded").includes("state"));
  check("card_info carries the Nocturne types", types["Werewolf"].includes("night") && types["Bard"].includes("fate") && types["Skulk"].includes("doom") && types["Imp"].includes("spirit"));
  const css = readFileSync(new URL("./style.css", import.meta.url), "utf8");
  const dark = css.slice(css.indexOf("prefers-color-scheme: dark"));
  check("light and dark tokens exist for the Nocturne chips", ["--c-night", "--c-boon", "--c-hex", "--c-state", "--c-heirloom"].every((t) => css.includes(t + ":") && dark.includes(t + ":")));

  const text = `players: 2
kingdom: Monastery, Ghost Town, Bard, Skulk, Druid, Village, Smithy, Market, Militia, Moat
boon deck: The Earth's Gift, The Sun's Gift
boon discard: The Sea's Gift
druid boons: The Moon's Gift
hex deck: Greed, War
hex discard: Envy
lost in the woods: 2
side piles: Will-o'-Wisp=12
turn: 3  player: 1  phase: buy  actions: 0  buys: 1  coins: 0

[player 1]
hand: Monastery, Estate
deck: 5 Copper
states: Deluded, Miserable

[player 2]
hand: 5 Copper
deck: 5 Estate
`;
  const J = await instance();
  J.load(text);
  let v = J.view();
  check("the view carries the Boons and Hexes", v.nocturne && v.nocturne.boons && v.nocturne.hexes && v.nocturne.boonDeck.length === 2 && v.nocturne.hexDiscard[0] === "Envy" && v.nocturne.lost === 1, JSON.stringify(v.nocturne));
  check("the States are in the player's view", v.players[0].states.join(",") === "Deluded,Miserable", JSON.stringify(v.players[0].states));
  check("Will-o'-Wisp is listed as a pile outside the Supply", v.supply.some((c) => c.name === "Will-o'-Wisp" && c.side && c.count === 12));
  // Pass the Buy phase: the Night phase offers the Monastery.
  const pass = v.pending.choices.find((c) => /Done buying/.test(c.label));
  J.w.choose(pass.index);
  v = J.view();
  check("the Night phase asks which Night card to play", v.turn.phase === "night" && /Night/.test(v.pending.description) && v.pending.choices.some((c) => c.label === "Play Monastery (Night)"), v.pending.description);
  check("log: the Night phase starts", v.log.some((l) => /:: Night phase \(Player 1\)/.test(l)), v.log.join(" | "));
  // A game without Boons or Hexes has no panel data.
  const K = await instance();
  K.load("players: 2\nkingdom: Village, Smithy, Market, Militia, Moat, Cellar, Mine, Remodel, Workshop, Merchant\nturn: 1  player: 1  phase: action  actions: 1  buys: 1  coins: 0\n\n[player 1]\nhand: 5 Copper\ndeck: 5 Estate\n\n[player 2]\nhand: 5 Copper\ndeck: 5 Estate\n");
  check("no Nocturne data in other games", K.view().nocturne === null);
}

// The "Piles" section's data: pile orders for the Castles (fixed), Knights and Ruins (every card as dealt, top first). Split piles are not listed.
{
  const { w, view } = await instance();
  const k = enc.encode("Knights, Marauder, Castles, Settlers, Village, Smithy, Market, Cellar, Moat, Festival");
  const p = w.alloc(k.length);
  new Uint8Array(w.memory.buffer, p, k.length).set(k);
  w.new_game(2, p, k.length, 4n, 0);
  const po = view().pileOrder || [];
  const by = (nm) => po.find((x) => x.name === nm);
  check("pile order lists the Castles top first", by("Castles") && by("Castles").order === "fixed" && by("Castles").cards[0][0] === "Humble Castle", JSON.stringify(po));
  check("pile order does not list split piles", po.length === 3 && !po.some((x) => /\//.test(x.name)), JSON.stringify(po.map((x) => x.name)));
  const kn = by("Knights");
  check("Knights: all ten listed, top first, each once", kn && kn.order === "dealt" && kn.cards.length === 10 && new Set(kn.cards).size === 10, JSON.stringify(kn));
  const sup = view().supply.find((x) => x.pile === "Knights");
  check("the first Knight listed is the Supply's top Knight", kn && sup && sup.name === kn.cards[0], JSON.stringify(sup));
  check("Ruins (Marauder): 10 listed", by("Ruins") && by("Ruins").cards.length === 10 && by("Ruins").cards[0] === view().supply.find((x) => x.pile === "Ruins").name);
}

// Ruins pile size: 10 per opponent (2 players: 10), whatever the 50 Ruins it is dealt from.
{
  const pasted = `players: 2
kingdom: Bard, Candlestick Maker, Druid, Faithful Hound, Marauder, Monastery, Overlord, Transmogrify, Villa, Witch
landmarks: Colonnade, Triumphal Arch
landmark vp: Colonnade=12
boon deck: The Earth's Gift, The Flame's Gift, The Forest's Gift, The Moon's Gift, The Mountain's Gift, The River's Gift, The Sea's Gift, The Swamp's Gift, The Wind's Gift
druid boons: The Field's Gift, The Sky's Gift, The Sun's Gift
side piles: Spoils=15, Will-o'-Wisp=12
supply: Copper=46, Silver=40, Gold=30, Estate=8, Duchy=8, Province=8, Curse=10, Witch=10, Candlestick Maker=10, Marauder=10, Transmogrify=10, Overlord=10, Villa=10, Druid=10, Faithful Hound=10, Monastery=10, Bard=10, Survivors=10
turn: 1  player: 1  phase: setup  actions: 1  buys: 1  coins: 0
seed: 3406718355780431780

[player 1]
hand:
deck: 7 Copper, 3 Estate

[player 2]
hand:
deck: 7 Copper, 3 Estate
`;
  const I = await instance();
  I.load(pasted);
  const ru = (I.view().pileOrder || []).find((x) => x.name === "Ruins");
  check("2 players: the Ruins pile lists 10 cards, top first (Survivors on top)", ru && ru.cards.length === 10 && ru.cards[0] === "Survivors", JSON.stringify(ru));
  check("every listed Ruins is one of the five", ru && ru.cards.every((c) => ["Abandoned Mine", "Ruined Library", "Ruined Market", "Ruined Village", "Survivors"].includes(c)), JSON.stringify(ru));
  // A save without the pile's order deals it the same way every time it is loaded (the state's seed decides), and a saved order is kept.
  const I2 = await instance();
  I2.load(pasted);
  check("loading the same save deals the same face-down Ruins", JSON.stringify((I2.view().pileOrder || []).find((x) => x.name === "Ruins")) === JSON.stringify(ru));
  const withOrder = pasted.replace("seed:", "ruins pile: Survivors, 2 Ruined Village, 3 Abandoned Mine, 2 Ruined Market, 2 Ruined Library\nseed:");
  const I3 = await instance();
  I3.load(withOrder);
  const ru3 = (I3.view().pileOrder || []).find((x) => x.name === "Ruins");
  check("a 'ruins pile:' line sets the order", ru3 && ru3.cards.join() === "Survivors,Ruined Village,Ruined Village,Abandoned Mine,Abandoned Mine,Abandoned Mine,Ruined Market,Ruined Market,Ruined Library,Ruined Library", JSON.stringify(ru3));
  const sup = I.view().supply.find((x) => x.pile === "Ruins");
  check("the Supply shows the Ruins pile once, with its 10 cards", sup && sup.count === 10 && sup.name === "Survivors", JSON.stringify(sup));
  for (const [players, size] of [[2, 10], [3, 20], [4, 30]]) {
    const { w, view } = await instance();
    const k = enc.encode("Marauder, Village, Smithy, Market, Cellar, Moat, Festival, Laboratory, Witch, Mine");
    const p = w.alloc(k.length);
    new Uint8Array(w.memory.buffer, p, k.length).set(k);
    w.new_game(players, p, k.length, 11n, 0);
    const r = (view().pileOrder || []).find((x) => x.name === "Ruins");
    check(`${players} players: ${size} Ruins in the pile`, r && r.cards.length === size, JSON.stringify(r));
  }
}

// --- Renaissance: Villagers, Projects and Artifacts in the view ---------------------------
{
  const text = `players: 2
kingdom: Border Guard, Flag Bearer, Silk Merchant, Smithy, Village, Market, Moat, Cellar, Militia, Chapel
projects: Fair, Pageant
artifacts: Flag=2
turn: 3  player: 1  phase: buy  actions: 1  buys: 1  coins: 4

[player 1]
hand: 3 Copper, Estate
deck: 4 Copper, 3 Estate, Silver
villagers: 3
owned projects: Fair

[player 2]
hand: 5 Copper
deck: 2 Copper, 3 Estate
`;
  const I = await instance();
  I.load(text);
  const v = I.view();
  check("the Projects of the game are listed", (v.projects || []).map((p) => p.name).sort().join() === "Fair,Pageant", JSON.stringify(v.projects));
  const fair = (v.projects || []).find((p) => p.name === "Fair");
  const pageant = (v.projects || []).find((p) => p.name === "Pageant");
  check("a Project bought shows its owner and is not buyable again", fair && fair.owners.join() === "0" && !fair.buyable, JSON.stringify(fair));
  check("the other Project is buyable with the $ in hand", pageant && pageant.buyable && pageant.costLabel === "$3", JSON.stringify(pageant));
  check("the Flag shows its holder", (v.artifacts || []).some((a) => a.name === "Flag" && a.holder === 1), JSON.stringify(v.artifacts));
  check("Villagers and owned Projects are reported per player", v.players[0].villagers === 3 && v.players[0].ownedProjects.join() === "Fair" && v.players[1].villagers === 0, JSON.stringify(v.players[0].ownedProjects));
  check("the loaded Buy phase is shown as loaded (its Treasures not played)", v.pending && v.players[0].inPlay.length === 0 && v.pending.choices.some((c) => c.label === "Play Copper"), JSON.stringify(v.pending && v.pending.choices.map((c) => c.label)));
  I.w.get_state_text();
  const back = I.rd();
  check("the state text keeps Villagers, Projects and the Artifact", /villagers: 3/.test(back) && /owned projects: Fair/.test(back) && /Flag=2/.test(back), back);
}

console.log(failed ? `${failed} FAILED` : "ALL PASSED (log)");
if (failed) process.exit(1);
