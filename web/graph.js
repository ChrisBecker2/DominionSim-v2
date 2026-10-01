// Pure helpers for the "Search graph" view: indexing, visible sets, layered layout, route marks.
// No DOM here, so web/test.mjs can unit-test it under Node. build.mjs inlines this file into
// the page ahead of app.js; in Node it is a CommonJS module.
//
// Graph shape (from wasm `search_graph`): nodes [{kind, depth, value, exact, inEdges, best, ...}],
// edges [{from, to, label, prob, best}]. Node 0 is the root.

(function (root) {
  "use strict";

  // CSR adjacency over edge indices: outList[outStart[v] .. outStart[v+1]) are v's outgoing edges.
  function buildIndex(g) {
    const n = g.nodes.length, m = g.edges.length;
    const efrom = new Int32Array(m), eto = new Int32Array(m);
    const inStart = new Int32Array(n + 1), outStart = new Int32Array(n + 1);
    for (let i = 0; i < m; i++) {
      const e = g.edges[i];
      efrom[i] = e.from;
      eto[i] = e.to;
      outStart[e.from + 1]++;
      inStart[e.to + 1]++;
    }
    for (let v = 0; v < n; v++) {
      inStart[v + 1] += inStart[v];
      outStart[v + 1] += outStart[v];
    }
    const inList = new Int32Array(m), outList = new Int32Array(m);
    const ic = inStart.slice(0, n), oc = outStart.slice(0, n);
    for (let i = 0; i < m; i++) {
      inList[ic[eto[i]]++] = i;
      outList[oc[efrom[i]]++] = i;
    }
    return { n, m, efrom, eto, inStart, inList, outStart, outList };
  }

  // Positions per depth, and how many of them were reached by more than one route.
  function depthStats(g) {
    let maxD = 0;
    for (const nd of g.nodes) if (nd.depth > maxD) maxD = nd.depth;
    const rows = [];
    for (let d = 0; d <= maxD; d++) rows.push({ depth: d, total: 0, merged: 0 });
    for (const nd of g.nodes) {
      const r = rows[nd.depth];
      r.total++;
      if (nd.inEdges > 1) r.merged++;
    }
    return rows;
  }

  function summary(g) {
    let merged = 0;
    for (const nd of g.nodes) if (nd.inEdges > 1) merged++;
    return {
      positions: g.nodes.length,
      routes: g.edges.length,
      merged,
      mergedPct: g.nodes.length ? (100 * merged) / g.nodes.length : 0,
      ttPct: g.searchedNodes ? (100 * g.ttHits) / g.searchedNodes : 0,
    };
  }

  // The nodes to show. opts: mode "best" | "all" | "under", firstEdge (edge index, for "under"),
  // minDepth, maxDepth, cap (for "best"), extra (Uint8Array of nodes to force visible).
  function visibleSet(g, idx, opts) {
    const o = opts || {};
    const mode = o.mode || "best";
    const minD = o.minDepth === undefined ? 0 : o.minDepth;
    const maxD = o.maxDepth === undefined ? Infinity : o.maxDepth;
    const cap = o.cap === undefined ? 800 : o.cap;
    const n = idx.n, nodes = g.nodes;
    const vis = new Uint8Array(n);
    const inRange = (v) => nodes[v].depth >= minD && nodes[v].depth <= maxD;
    if (mode === "all") {
      for (let v = 0; v < n; v++) if (inRange(v)) vis[v] = 1;
    } else if (mode === "under") {
      const e0 = o.firstEdge;
      if (e0 !== undefined && e0 >= 0 && e0 < idx.m) {
        const seen = new Uint8Array(n);
        const queue = [idx.eto[e0]];
        seen[queue[0]] = 1;
        for (let h = 0; h < queue.length; h++) {
          const v = queue[h];
          if (inRange(v)) vis[v] = 1;
          for (let k = idx.outStart[v]; k < idx.outStart[v + 1]; k++) {
            const w = idx.eto[idx.outList[k]];
            if (!seen[w]) {
              seen[w] = 1;
              queue.push(w);
            }
          }
        }
      }
      if (inRange(0)) vis[0] = 1;
    } else {
      // Best line first (always shown), then the most-merged positions up to the cap.
      let count = 0;
      const merged = [];
      for (let v = 0; v < n; v++) {
        if (!inRange(v)) continue;
        if (nodes[v].best || v === 0) {
          vis[v] = 1;
          count++;
        } else if (nodes[v].inEdges >= 2) merged.push(v);
      }
      merged.sort((a, b) => nodes[b].inEdges - nodes[a].inEdges || nodes[a].depth - nodes[b].depth || a - b);
      for (let i = 0; i < merged.length && count < cap; i++) {
        vis[merged[i]] = 1;
        count++;
      }
    }
    if (o.extra) for (let v = 0; v < n; v++) if (o.extra[v]) vis[v] = 1;
    return vis;
  }

  // Layered layout. Column = depth (relative to the shallowest visible depth); within a column,
  // best-line nodes first, the rest ordered by the mean y of their visible neighbours (barycenter
  // passes left to right, right to left, then left to right again). Each column is spread over
  // the full height so sparse columns do not bunch up. Returns world coordinates in units of
  // columns (x) and rows (y, centred on 0), plus the visible node and edge lists.
  function layout(g, idx, vis) {
    const n = idx.n, nodes = g.nodes;
    let minD = Infinity, maxD = -1;
    for (let v = 0; v < n; v++) {
      if (!vis[v]) continue;
      if (nodes[v].depth < minD) minD = nodes[v].depth;
      if (nodes[v].depth > maxD) maxD = nodes[v].depth;
    }
    const x = new Float32Array(n), y = new Float32Array(n);
    if (maxD < 0) return { x, y, minDepth: 0, maxDepth: 0, cols: 0, rows: 0, nodes: new Int32Array(0), edges: new Int32Array(0) };
    const ncols = maxD - minD + 1;
    const columns = [];
    for (let c = 0; c < ncols; c++) columns.push([]);
    for (let v = 0; v < n; v++) if (vis[v]) columns[nodes[v].depth - minD].push(v);
    let rows = 1;
    for (const c of columns) if (c.length > rows) rows = c.length;
    const spread = (col) => {
      const k = col.length;
      for (let r = 0; r < k; r++) y[col[r]] = ((r + 0.5) / k - 0.5) * rows;
    };
    const key = new Float64Array(n);
    const order = (col, useIn) => {
      const S = useIn ? idx.inStart : idx.outStart, L = useIn ? idx.inList : idx.outList, other = useIn ? idx.efrom : idx.eto;
      for (const v of col) {
        let s = 0, c = 0;
        for (let k = S[v]; k < S[v + 1]; k++) {
          const w = other[L[k]];
          if (vis[w] && w !== v) {
            s += y[w];
            c++;
          }
        }
        key[v] = c ? s / c : y[v];
      }
      col.sort((a, b) => (nodes[b].best === true) - (nodes[a].best === true) || key[a] - key[b] || a - b);
      spread(col);
    };
    for (let c = 0; c < ncols; c++) {
      columns[c].sort((a, b) => (nodes[b].best === true) - (nodes[a].best === true) || a - b);
      spread(columns[c]);
      for (const v of columns[c]) x[v] = c;
    }
    for (let c = 1; c < ncols; c++) order(columns[c], true);
    for (let c = ncols - 2; c >= 0; c--) order(columns[c], false);
    for (let c = 1; c < ncols; c++) order(columns[c], true);
    const vn = [];
    for (let v = 0; v < n; v++) if (vis[v]) vn.push(v);
    const ve = [];
    for (let i = 0; i < idx.m; i++) if (vis[idx.efrom[i]] && vis[idx.eto[i]]) ve.push(i);
    return { x, y, minDepth: minD, maxDepth: maxD, cols: ncols, rows, nodes: Int32Array.from(vn), edges: Int32Array.from(ve) };
  }

  // Every route from the root to `node` (all ancestors, via incoming edges), and its best
  // continuation (descendants through best edges only). Marks: nodes 1 = ancestor-or-self,
  // 2 = best descendant; edges 1 = on a route in, 2 = best continuation out.
  function routeMarks(g, idx, node) {
    const nm = new Uint8Array(idx.n), em = new Uint8Array(idx.m);
    nm[node] = 1;
    const q = [node];
    for (let h = 0; h < q.length; h++) {
      const v = q[h];
      for (let k = idx.inStart[v]; k < idx.inStart[v + 1]; k++) {
        const e = idx.inList[k];
        em[e] = 1;
        const u = idx.efrom[e];
        if (!nm[u]) {
          nm[u] = 1;
          q.push(u);
        }
      }
    }
    const q2 = [node];
    const seen = new Uint8Array(idx.n);
    seen[node] = 1;
    for (let h = 0; h < q2.length; h++) {
      const v = q2[h];
      for (let k = idx.outStart[v]; k < idx.outStart[v + 1]; k++) {
        const e = idx.outList[k];
        if (!g.edges[e].best) continue;
        em[e] = 2;
        const w = idx.eto[e];
        if (!seen[w]) {
          seen[w] = 1;
          if (!nm[w]) nm[w] = 2;
          q2.push(w);
        }
      }
    }
    return { nodes: nm, edges: em };
  }

  const api = { buildIndex, depthStats, summary, visibleSet, layout, routeMarks };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  else root.SearchGraph = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
