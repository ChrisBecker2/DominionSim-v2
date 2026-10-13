#!/usr/bin/env bash
# Code-performance benchmark: speed and result identity of a pure optimisation.
#
#   bench/perf.sh BASE_DIR [NEW_DIR] [RUNS]
#
# BASE_DIR / NEW_DIR hold copies of dominion-sim.exe, bench_analyze.exe and league_run.exe (default NEW_DIR:
# target/release, with the examples taken from target/release/examples). Each benchmark runs RUNS times (default 5)
# alternating base and new; the best time of each is reported. The outputs (timing lines stripped) of base and new
# are diffed: they must be identical, since a speed-up must not change any result.
#
# BENCHES="plain engine" runs only those. Build the pieces first: cargo build --release --workspace --bins --examples
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASE="$1"; NEW="${2:-$ROOT/target/release}"; RUNS="${3:-5}"
S="$ROOT/strategies"
OUT="$(mktemp -d)"
exe() { # dir name
  if [ -f "$1/$2" ]; then echo "$1/$2"; else echo "$1/examples/$2"; fi
}
KING_RS="Biding Time, Kintsugi, Aristocrat, Artist, Kitsune, Root Cellar, Samurai, Artificer, Distant Lands, Dungeon, Duplicate, Page"
KING_LG="Village, Smithy, Militia, Witch, Market, Laboratory, Moat, Cellar, Moneylender, Chapel"
# The match benchmarks run on 4 threads: with all cores (SMT included) the run-to-run noise is larger than the effects measured.
export RAYON_NUM_THREADS=4
export ISLANDS=2 ISLAND_SIZE=8 RACES=200,400 EXPLOIT_ROUNDS=1 EXPLOIT_GENS=3 VALIDATE=2000 POLISH=1000

# The time a match took, in ms, from its "N games in T (G games/sec)" line.
gps_to_ms() { grep -o '^[0-9]* games in.*games/sec' "$1" | sed -E 's/^([0-9]+) games in .*\(([0-9]+) games\/sec/\1 \2/' | awk '{printf "%.1f", $1 * 1000 / $2}'; }

# bench NAME CMD... : the command is run with $BIN set to the dir's executables via a function
run_one() { # tag dir name
  local tag="$1" dir="$2" name="$3"
  local sim; sim="$(exe "$dir" dominion-sim.exe)"
  local t0 t1
  t0=$(date +%s%N)
  case "$name" in
    plain)   "$sim" match "$S/big_money_ultimate.toml" "$S/double_witch.toml" --games 300000 --seed 1 ;;
    engine)  "$sim" match "$S/village_smithy_engine.toml" "$S/kitsune_bm.toml" --kingdom auto --games 8000 --seed 1 ;;
    rising)  "$sim" match "$S/kitsune_bm.toml" "$S/samurai_bm.toml" --kingdom "$KING_RS" --games 8000 --seed 1 ;;
    analyze) "$(exe "$dir" bench_analyze.exe)" 3 ;;
    league)  "$(exe "$dir" league_run.exe)" "$KING_LG" 5 500 ;;
  esac > "$OUT/$name.$tag.raw"
  t1=$(date +%s%N)
  echo $(( (t1 - t0) / 1000000 )) > "$OUT/$name.$tag.ms"
  # strip everything that is timing
  grep -v -E 'games/sec|^TIME' "$OUT/$name.$tag.raw" | sed -E 's/^\[ *[0-9]+s\]/[T]/' > "$OUT/$name.$tag.txt"
}
for name in ${BENCHES:-plain engine rising analyze league}; do
  best_b=999999999; best_n=999999999
  for ((i = 0; i < RUNS; i++)); do
    run_one base "$BASE" "$name"; run_one new "$NEW" "$name"
    # prefer the program's own timing where it prints one (excludes process start and game setup)
    if [ "$name" = analyze ]; then
      b=$(grep '^TIME total' "$OUT/$name.base.raw" | awk '{print $3}'); n=$(grep '^TIME total' "$OUT/$name.new.raw" | awk '{print $3}')
    elif [ "$name" = league ]; then
      b=$(cat "$OUT/$name.base.ms"); n=$(cat "$OUT/$name.new.ms")
    else
      b=$(gps_to_ms "$OUT/$name.base.raw"); n=$(gps_to_ms "$OUT/$name.new.raw")
    fi
    best_b=$(echo "$b $best_b" | awk '{print ($1<$2)?$1:$2}'); best_n=$(echo "$n $best_n" | awk '{print ($1<$2)?$1:$2}')
  done
  if cmp -s "$OUT/$name.base.txt" "$OUT/$name.new.txt"; then same="identical"; else same="DIFFERENT (diff $OUT/$name.base.txt $OUT/$name.new.txt)"; fi
  echo "$name: base ${best_b} ms, new ${best_n} ms, ratio $(echo "$best_b $best_n" | awk '{printf "%.3f", $1/$2}') (>1 = faster); results $same"
done
