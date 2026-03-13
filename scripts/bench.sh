#!/usr/bin/env bash
# Run the check-covers benchmark N times and report median timings.
#
# Usage:
#   bash scripts/bench.sh [N] [-- check-covers args...]
#
# Defaults to 3 runs with the standard benchmark args.
# Passes all args after -- to check-covers.
#
# Examples:
#   bash scripts/bench.sh
#   bash scripts/bench.sh 5
#   bash scripts/bench.sh 3 -- triangle=5 triangle=6 2 line-2.json

set -euo pipefail

RUNS=3
BENCH_ARGS=(triangle=6 triangle=7 2 line-2.json)

# Parse optional run count and passthrough args
if [[ $# -gt 0 && $1 != "--" ]]; then
    RUNS=$1
    shift
fi
if [[ $# -gt 0 && $1 == "--" ]]; then
    shift
    BENCH_ARGS=("$@")
fi

echo "Building check-covers..." >&2
cargo build --release --bin check-covers 2>&1

echo "Running $RUNS iterations of: check-covers ${BENCH_ARGS[*]}" >&2
echo >&2

# Collect all TIMER lines from each run into an array, one entry per run
declare -a ALL_RUNS

for i in $(seq 1 "$RUNS"); do
    echo -n "  Run $i/$RUNS ... " >&2
    output=$(taskset -c 0-7 env RAYON_NUM_THREADS=8 cargo run --release --bin check-covers -- "${BENCH_ARGS[@]}" 2>/dev/null)
    ALL_RUNS+=("$(echo "$output" | grep '^TIMER')")
    echo "done" >&2
done

echo >&2

# Pass all runs to the Python script for median analysis
python3 - "$RUNS" "${ALL_RUNS[@]}" <<'PYEOF'
import sys
import statistics

n_runs = int(sys.argv[1])
run_blocks = sys.argv[2:]  # one block of TIMER lines per run

def parse_block(block):
    timings = {}
    for line in block.splitlines():
        line = line.strip()
        if not line.startswith("TIMER "):
            continue
        rest = line[len("TIMER "):]
        name, _, value_str = rest.rpartition(": ")
        timings[name] = float(value_str.rstrip("ms"))
    return timings

all_runs = [parse_block(b) for b in run_blocks]

# Collect all known keys
keys = list(all_runs[0].keys())

# Compute median per key
medians = {}
for k in keys:
    vals = [r[k] for r in all_runs if k in r]
    medians[k] = statistics.median(vals)

# Print individual runs
for i, r in enumerate(all_runs):
    print(f"  Run {i+1}: min_covers={r.get('min_covers', float('nan')):.1f}ms")
print()

# Feed medians into analyze-timing logic inline
total = sum(medians.values())
top_level = {k: v for k, v in medians.items() if "/" not in k}
sub_timers = {k: v for k, v in medians.items() if "/" in k}

col_w = max(len(k) for k in medians) + 2

def print_table(rows, grand_total):
    print(f"  {'Phase':<{col_w}} {'median ms':>12} {'%':>8}")
    print("  " + "-" * (col_w + 22))
    for name, ms in sorted(rows.items(), key=lambda x: -x[1]):
        print(f"  {name:<{col_w}} {ms:>12.3f} {ms/grand_total*100:>7.2f}%")

print(f"=== Top-level phases — median of {n_runs} runs (total: {total:.3f}ms) ===")
print_table(top_level, total)

if sub_timers:
    prefix = next(iter(sub_timers)).split("/")[0]
    loop_total_key = f"{prefix}/total"
    loop_total = medians.get(loop_total_key)
    sub_without_total = {k: v for k, v in sub_timers.items() if k != loop_total_key}
    sub_sum = sum(sub_without_total.values())

    print(f"\n=== {prefix} sub-timers ===")
    ref = loop_total if loop_total is not None else sub_sum
    print_table(sub_without_total, ref)

    if loop_total is not None:
        unaccounted = loop_total - sub_sum
        print(f"\n  {'sub-timer sum':<{col_w}} {sub_sum:>12.3f} {sub_sum/loop_total*100:>7.2f}%")
        print(f"  {'unaccounted overhead':<{col_w}} {unaccounted:>12.3f} {unaccounted/loop_total*100:>7.2f}%")

print()
PYEOF
