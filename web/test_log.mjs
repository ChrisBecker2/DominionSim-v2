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

console.log(failed ? `${failed} FAILED` : "ALL PASSED (log)");
if (failed) process.exit(1);
