// Pure helpers for the Simulation panel: adding up the per-worker sums that wasm `simulate`
// returns, and turning the merged sums into the numbers the charts show. No DOM here, so the
// tests can run it under Node. build.mjs inlines this file into the page ahead of app.js; in Node
// it is a CommonJS module.
//
// Everything `simulate` returns is a sum (or count) so parts add up exactly; means, spreads and
// confidence intervals are computed here, after merging. Shapes (see crates/wasm/src/lib.rs):
//   { games, capped, lengthSum, lengthHist[], fresh, pairs[{a,b,n[],sum[],sq[],marginSq}],
//     players[{ vp[], vpSq[], money[], moneySq[], buys[], buysSq[], count[], provCum[], duchyCum[],
//               colonyCum[], provEnd[], duchyEnd[], colonyEnd[], curse[], curseEnd[], wins, winsSq, winsAt[], firstGames, firstWins, owned?{card: sums[]} }] }
// Arrays are indexed by the player's own turn number minus 1.

(function (root) {
  "use strict";

  // Per-turn averages are shown only where at least this share of games reached the turn, so the
  // tail isn't a handful of long games.
  const MIN_SHARE = 0.05;
  const Z95 = 1.959964;

  const PLAYER_ARRAYS = ["vp", "vpSq", "money", "moneySq", "buys", "buysSq", "count", "provCum", "duchyCum", "colonyCum", "provEnd", "duchyEnd", "colonyEnd", "curse", "curseEnd", "winsAt"];
  const PLAYER_NUMBERS = ["wins", "winsSq", "firstGames", "firstWins"];

  function addArr(a, b) {
    if (!b) return a;
    const out = a.slice();
    for (let i = 0; i < b.length; i++) out[i] = (out[i] || 0) + b[i];
    return out;
  }

  // Add up the results of several `simulate` calls (one per worker). A part may lack the newer
  // fields (the single-game record of Run Game): they stay empty.
  function mergeSims(parts, players) {
    const r = { games: 0, capped: 0, lengthSum: 0, lengthHist: [], fresh: parts.length > 0, pairs: [], players: [] };
    for (let p = 0; p < players; p++) {
      const q = { owned: null };
      PLAYER_ARRAYS.forEach((f) => (q[f] = []));
      PLAYER_NUMBERS.forEach((f) => (q[f] = 0));
      r.players.push(q);
    }
    for (const part of parts) {
      r.games += part.games;
      r.capped += part.capped;
      r.lengthSum += part.lengthSum;
      r.lengthHist = addArr(r.lengthHist, part.lengthHist);
      if (!part.fresh) r.fresh = false;
      (part.pairs || []).forEach((pr, i) => {
        const t = r.pairs[i] || (r.pairs[i] = { a: pr.a, b: pr.b, n: [], sum: [], sq: [], marginSq: 0 });
        t.n = addArr(t.n, pr.n);
        t.sum = addArr(t.sum, pr.sum);
        t.sq = addArr(t.sq, pr.sq);
        t.marginSq += pr.marginSq;
      });
      part.players.forEach((q, p) => {
        const t = r.players[p];
        PLAYER_ARRAYS.forEach((f) => (t[f] = addArr(t[f], q[f])));
        PLAYER_NUMBERS.forEach((f) => (t[f] += q[f] || 0));
        if (q.owned) {
          t.owned = t.owned || {};
          for (const name in q.owned) t.owned[name] = addArr(t.owned[name] || [], q.owned[name]);
        }
      });
    }
    return r;
  }

  // Wilson score interval for `wins` out of `n` (wins may be fractional: shared wins count 1/k).
  function wilson(wins, n, z) {
    z = z || Z95;
    if (!(n > 0)) return { p: 0, lo: 0, hi: 0, n: 0 };
    const p = wins / n, z2 = z * z, d = 1 + z2 / n;
    const centre = (p + z2 / (2 * n)) / d;
    const half = (z * Math.sqrt((p * (1 - p)) / n + z2 / (4 * n * n))) / d;
    return { p, lo: Math.max(0, centre - half), hi: Math.min(1, centre + half), n };
  }

  // Win rates with 95% intervals: per seat (overall, as first player, as second player), the
  // first-player rate over all seats, and the head-to-head margin of seat 0 against each other
  // seat (percentage points; the interval uses the per-game paired difference).
  function winStats(r) {
    const N = r.games;
    const seats = r.players.map((q) => ({
      ...wilson(q.wins, N),
      wins: q.wins,
      first: r.fresh && q.firstGames > 0 ? wilson(q.firstWins, q.firstGames) : null,
      second: r.fresh && N - q.firstGames > 0 ? wilson(q.wins - q.firstWins, N - q.firstGames) : null,
    }));
    let firstAll = null;
    if (r.fresh) {
      const w = r.players.reduce((s, q) => s + q.firstWins, 0);
      firstAll = wilson(w, r.players.reduce((s, q) => s + q.firstGames, 0));
    }
    const margins = r.pairs.map((pr) => {
      const d = (r.players[pr.a].wins - r.players[pr.b].wins) / N;
      const se = Math.sqrt(Math.max(0, pr.marginSq / N - d * d) / N);
      return { a: pr.a, b: pr.b, d: 100 * d, lo: 100 * (d - Z95 * se), hi: 100 * (d + Z95 * se) };
    });
    return { seats, firstAll, margins };
  }

  const enough = (r, c) => c >= Math.max(1, r.games * MIN_SHARE);

  // Mean (and standard deviation when squares are known) of a per-turn sum, per seat.
  function meanSeries(r, field, sqField, floorZero) {
    return r.players.map((q) => {
      const pts = [];
      q.count.forEach((c, i) => {
        if (!enough(r, c)) return;
        const mean = q[field][i] / c;
        const pt = { x: i + 1, y: mean };
        if (sqField && q[sqField].length) {
          pt.sd = Math.sqrt(Math.max(0, q[sqField][i] / c - mean * mean));
          pt.lo = floorZero ? Math.max(0, mean - pt.sd) : mean - pt.sd;
          pt.hi = mean + pt.sd;
        }
        pts.push(pt);
      });
      return pts;
    });
  }

  // Cards bought so far by the end of the seat's turn t, averaged over all games: a game that
  // ended earlier keeps its final total (so lines never fall, unlike a mean over the games still
  // running). `cum` is the running total summed over games that reached t, `end` the totals of
  // games whose last turn was t.
  function carriedSeries(r, cum, end) {
    return r.players.map((q) => {
      const pts = [];
      let finished = 0;
      q.count.forEach((c, i) => {
        if (enough(r, c)) pts.push({ x: i + 1, y: ((q[cum][i] || 0) + finished) / r.games });
        finished += q[end][i] || 0;
      });
      return pts;
    });
  }

  // P(the game is over by the seat's turn t and the seat won), in percent.
  function cumulativeWins(r) {
    return r.players.map((q) => {
      const pts = [];
      let acc = 0;
      q.winsAt.forEach((w, i) => {
        acc += w;
        pts.push({ x: i + 1, y: (100 * acc) / r.games });
      });
      return pts;
    });
  }

  // Seat a minus seat b, averaged over games where both had played the turn.
  function vpDifference(r) {
    return r.pairs.map((pr) => {
      const pts = [];
      pr.n.forEach((c, i) => {
        if (!enough(r, c)) return;
        const mean = pr.sum[i] / c, sd = Math.sqrt(Math.max(0, pr.sq[i] / c - mean * mean));
        pts.push({ x: i + 1, y: mean, sd, lo: mean - sd, hi: mean + sd, n: c });
      });
      return { a: pr.a, b: pr.b, pts };
    });
  }

  // Share of finished games (percent) by length in turns, plus mean and median.
  function lengthDistribution(r) {
    const finished = r.games - r.capped;
    const pts = [];
    let lo = r.lengthHist.findIndex((c) => c > 0), hi = -1;
    r.lengthHist.forEach((c, i) => c > 0 && (hi = i));
    if (lo < 0 || !finished) return { pts, mean: 0, median: 0, finished };
    let acc = 0, median = lo;
    for (let i = lo; i <= hi; i++) {
      const c = r.lengthHist[i] || 0;
      if (acc < finished / 2 && acc + c >= finished / 2) median = i;
      acc += c;
      pts.push({ x: i, y: (100 * c) / finished, n: c });
    }
    return { pts, mean: r.lengthSum / finished, median, finished };
  }

  // Cards owned after each turn (mean per seat). Only cards whose mean ever exceeds `threshold`
  // for some seat, most-owned first, at most `limit`.
  function deckSeries(r, threshold, limit) {
    if (!r.players.every((q) => q.owned)) return null;
    const names = new Set();
    r.players.forEach((q) => Object.keys(q.owned).forEach((n) => names.add(n)));
    const cards = [];
    for (const name of names) {
      const perSeat = r.players.map((q) => {
        const sums = q.owned[name] || [];
        const pts = [];
        q.count.forEach((c, i) => {
          if (enough(r, c)) pts.push({ x: i + 1, y: (sums[i] || 0) / c });
        });
        return pts;
      });
      const peak = Math.max(0, ...perSeat.flatMap((pts) => pts.map((p) => p.y)));
      if (peak > threshold) cards.push({ name, peak, perSeat });
    }
    cards.sort((a, b) => b.peak - a.peak || (a.name < b.name ? -1 : 1));
    return cards.slice(0, limit || 12);
  }

  const api = { MIN_SHARE, mergeSims, wilson, winStats, meanSeries, carriedSeries, cumulativeWins, vpDifference, lengthDistribution, deckSeries };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  else root.SimStats = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
