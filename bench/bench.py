#!/usr/bin/env python3
"""Strategy benchmark: every shipped strategy against every other, with fixed settings, so runs
can be compared across code changes.

    python bench/bench.py                 # 10,000 games per pairing
    python bench/bench.py --games 2000    # quicker, noisier
    python bench/bench.py --compare bench/results/<older>.json   # diff against a specific run

Each run writes bench/results/<timestamp>_<commit>.json (raw numbers) and .md (readable report),
and compares against the previous run in bench/results/ (or --compare): win-rate changes larger
than the combined 95% confidence intervals are flagged, and speed is shown as a ratio.

Settings are fixed for comparability: 2 players, seats rotated, kingdom "auto" (the cards the two
strategies name, padded deterministically), seed 1. Pairings run one at a time, each using all
cores, so games/sec is comparable on the same machine.
"""
import argparse
import datetime
import glob
import json
import math
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RESULTS = os.path.join(ROOT, "bench", "results")
CARGO = os.path.expanduser("~/.cargo/bin/cargo")
EXE = os.path.join(ROOT, "target", "release", "dominion-sim.exe" if os.name == "nt" else "dominion-sim")

ROW = re.compile(r"^(?P<name>.+?)\s+(?P<games>\d+)\s+(?P<win>[\d.]+)% \[\s*(?P<lo>[\d.]+)%,\s*(?P<hi>[\d.]+)%\]\s+(?P<vp>[\d.]+)\s+(?P<turns>[\d.]+)\s*$")
TIME = re.compile(r"^(?P<games>\d+) games in (?P<ms>[\d.]+)(?P<unit>ms|s|µs|us) \((?P<gps>\d+) games/sec\)")


def git(*args):
    try:
        return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    except Exception:
        return ""


def run_pair(a, b, games, seed):
    out = subprocess.run(
        [EXE, "match", a, b, "--games", str(games), "--kingdom", "auto", "--seed", str(seed)],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    rows, timing, kingdom = [], None, ""
    for line in out.splitlines():
        line = line.rstrip()
        if line.startswith("Kingdom:"):
            kingdom = line[len("Kingdom:"):].strip()
        m = ROW.match(line)
        if m:
            rows.append({"name": m["name"].strip(), "win": float(m["win"]), "lo": float(m["lo"]), "hi": float(m["hi"]),
                         "vp": float(m["vp"]), "turns": float(m["turns"])})
        t = TIME.match(line)
        if t:
            scale = {"ms": 1.0, "s": 1000.0, "µs": 0.001, "us": 0.001}[t["unit"]]
            timing = {"ms": float(t["ms"]) * scale, "games_per_sec": int(t["gps"])}
    if len(rows) != 2 or timing is None:
        raise RuntimeError(f"could not parse output for {a} vs {b}:\n{out}")
    return {"a": rows[0], "b": rows[1], "kingdom": kingdom, **timing}


def previous_result(exclude):
    files = sorted(f for f in glob.glob(os.path.join(RESULTS, "*.json")) if os.path.abspath(f) != os.path.abspath(exclude))
    return files[-1] if files else None


def fmt_ms(ms):
    return f"{ms / 1000:.1f} s" if ms >= 1000 else f"{ms:.0f} ms"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--games", type=int, default=10000, help="games per pairing (default 10000)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--compare", help="an earlier results .json to compare against (default: the previous run)")
    ap.add_argument("--no-build", action="store_true", help="skip `cargo build --release -p dominion-sim`")
    args = ap.parse_args()

    if not args.no_build:
        subprocess.run([CARGO, "build", "--release", "-q", "-p", "dominion-sim"], cwd=ROOT, check=True)

    strategies = sorted(glob.glob(os.path.join(ROOT, "strategies", "*.toml")))
    rel = [os.path.relpath(s, ROOT).replace("\\", "/") for s in strategies]
    commit = git("rev-parse", "--short", "HEAD") or "nogit"
    dirty = bool(git("status", "--porcelain", "--untracked-files=no"))
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    base = os.path.join(RESULTS, f"{stamp}_{commit}{'-dirty' if dirty else ''}")
    os.makedirs(RESULTS, exist_ok=True)

    pairs = []
    total = len(rel) * (len(rel) - 1) // 2
    for i in range(len(rel)):
        for j in range(i + 1, len(rel)):
            r = run_pair(rel[i], rel[j], args.games, args.seed)
            pairs.append({"a_file": rel[i], "b_file": rel[j], **r})
            print(f"[{len(pairs):>3}/{total}] {r['a']['name']:>24} {r['a']['win']:5.1f}%  vs  {r['b']['win']:5.1f}% {r['b']['name']:<24} "
                  f"{r['games_per_sec']:>9,} games/s", flush=True)

    names = {}
    for p in pairs:
        names[p["a_file"]] = p["a"]["name"]
        names[p["b_file"]] = p["b"]["name"]
    order = [names[f] for f in rel]

    # Aggregates per strategy: average win rate over its pairings, and total time spent.
    agg = {n: {"win_sum": 0.0, "n": 0, "ms": 0.0} for n in order}
    for p in pairs:
        for side in ("a", "b"):
            agg[p[side]["name"]]["win_sum"] += p[side]["win"]
            agg[p[side]["name"]]["n"] += 1
            agg[p[side]["name"]]["ms"] += p["ms"]
    total_ms = sum(p["ms"] for p in pairs)
    total_games = args.games * len(pairs)

    result = {
        "timestamp": stamp, "commit": commit, "dirty": dirty, "games_per_pair": args.games, "seed": args.seed,
        "cpus": os.cpu_count(), "platform": sys.platform, "total_ms": total_ms, "total_games": total_games,
        "pairs": pairs,
    }
    with open(base + ".json", "w", encoding="utf-8") as f:
        json.dump(result, f, indent=1)

    prev_path = args.compare or previous_result(base + ".json")
    prev = None
    if prev_path and os.path.exists(prev_path):
        with open(prev_path, encoding="utf-8") as f:
            prev = json.load(f)
    prev_pairs = {}
    if prev:
        for p in prev["pairs"]:
            prev_pairs[(p["a"]["name"], p["b"]["name"])] = p

    # ---- Markdown report ----
    lines = [f"# Strategy benchmark {stamp} ({commit}{', uncommitted changes' if dirty else ''})", ""]
    lines.append(f"{len(order)} strategies, {len(pairs)} pairings x {args.games:,} games, seed {args.seed}, "
                 f"kingdom auto, seats rotated, {os.cpu_count()} CPUs. Total {total_games:,} games in {fmt_ms(total_ms)} "
                 f"({total_games / (total_ms / 1000):,.0f} games/s overall).")
    if prev:
        lines.append(f"Compared with {os.path.basename(prev_path)} ({prev['commit']}{', dirty' if prev.get('dirty') else ''}).")
    lines += ["", "## Win rate of row vs column", ""]
    abbrev = [n if len(n) <= 14 else n[:13] + "." for n in order]
    lines.append("| | " + " | ".join(abbrev) + " | avg |")
    lines.append("|---" * (len(order) + 2) + "|")
    cell = {}
    for p in pairs:
        cell[(p["a"]["name"], p["b"]["name"])] = p["a"]["win"]
        cell[(p["b"]["name"], p["a"]["name"])] = p["b"]["win"]
    for r in sorted(order, key=lambda n: -agg[n]["win_sum"] / agg[n]["n"]):
        row = [f"{cell[(r, c)]:.1f}" if (r, c) in cell else "-" for c in order]
        lines.append(f"| **{r}** | " + " | ".join(row) + f" | **{agg[r]['win_sum'] / agg[r]['n']:.1f}** |")

    lines += ["", "## Speed per pairing", "", "| pairing | games/s | time | vs previous |", "|---|---:|---:|---:|"]
    for p in sorted(pairs, key=lambda p: p["games_per_sec"]):
        key = (p["a"]["name"], p["b"]["name"])
        cmp = ""
        if key in prev_pairs and prev_pairs[key]["games_per_sec"]:
            ratio = p["games_per_sec"] / prev_pairs[key]["games_per_sec"]
            cmp = f"{ratio:.2f}x"
        lines.append(f"| {p['a']['name']} vs {p['b']['name']} | {p['games_per_sec']:,} | {fmt_ms(p['ms'])} | {cmp} |")

    if prev:
        lines += ["", "## Significant win-rate changes vs previous", ""]
        changed = []
        for p in pairs:
            key = (p["a"]["name"], p["b"]["name"])
            q = prev_pairs.get(key)
            if not q:
                continue
            d = p["a"]["win"] - q["a"]["win"]
            half = (p["a"]["hi"] - p["a"]["lo"]) / 2 + (q["a"]["hi"] - q["a"]["lo"]) / 2
            if abs(d) > half:
                changed.append(f"- {key[0]} vs {key[1]}: {q['a']['win']:.1f}% -> {p['a']['win']:.1f}% ({d:+.1f})")
        lines += changed or ["None (all within the combined 95% confidence intervals)."]
        prev_ms = prev.get("total_ms") or 0
        if prev_ms:
            lines += ["", f"Total time {fmt_ms(prev_ms)} -> {fmt_ms(total_ms)} ({prev_ms / total_ms:.2f}x speed)."]

    with open(base + ".md", "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    print("\n" + "\n".join(lines))
    print(f"\nWrote {os.path.relpath(base, ROOT)}.json and .md")


if __name__ == "__main__":
    main()
