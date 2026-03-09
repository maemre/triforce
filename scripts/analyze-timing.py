#!/usr/bin/env python3
"""
Analyze timing output from check-covers.

Reads a JSON object from stdin (or first argument) mapping timer names to
milliseconds, and prints a formatted breakdown with percentages.

Usage:
    echo '{"min_covers": 14762, "covers_loop/total": 575}' | python3 scripts/analyze-timing.py
    python3 scripts/analyze-timing.py '{"min_covers": 14762, "covers_loop/total": 575}'

You can also build the JSON from raw TIMER output lines:
    cargo run --release --bin check-covers -- ... 2>&1 \
        | grep '^TIMER' \
        | python3 scripts/analyze-timing.py
"""

import json
import sys


def parse_timer_lines(lines):
    """Parse 'TIMER key: value ms' lines into a dict."""
    timings = {}
    for line in lines:
        line = line.strip()
        if not line.startswith("TIMER "):
            continue
        # Format: TIMER <name>: <value>ms
        rest = line[len("TIMER "):]
        name, _, value_str = rest.rpartition(": ")
        timings[name] = float(value_str.rstrip("ms"))
    return timings


def analyze(timings):
    if not timings:
        print("No timings provided.")
        sys.exit(1)

    total = sum(timings.values())

    # Separate loop sub-timers from top-level timers
    top_level = {k: v for k, v in timings.items() if "/" not in k}
    sub_timers = {k: v for k, v in timings.items() if "/" in k}

    col_w = max(len(k) for k in timings) + 2

    def print_table(rows, grand_total):
        print(f"  {'Phase':<{col_w}} {'ms':>10} {'%':>8}")
        print("  " + "-" * (col_w + 20))
        for name, ms in sorted(rows.items(), key=lambda x: -x[1]):
            print(f"  {name:<{col_w}} {ms:>10.3f} {ms/grand_total*100:>7.2f}%")

    # Top-level table (% of grand total)
    print(f"\n=== Top-level phases (total: {total:.3f}ms) ===")
    print_table(top_level, total)

    # Sub-timers table, if present
    if sub_timers:
        # Find the loop total key (the one without a slash that matches the prefix)
        prefix = next(iter(sub_timers)).split("/")[0]
        loop_total_key = f"{prefix}/total"
        loop_total = timings.get(loop_total_key)

        sub_without_total = {k: v for k, v in sub_timers.items() if k != loop_total_key}
        sub_sum = sum(sub_without_total.values())

        print(f"\n=== {prefix} sub-timers ===")
        ref = loop_total if loop_total is not None else sub_sum
        print_table(sub_without_total, ref)

        if loop_total is not None:
            unaccounted = loop_total - sub_sum
            print(f"\n  {'sub-timer sum':<{col_w}} {sub_sum:>10.3f} {sub_sum/loop_total*100:>7.2f}%")
            print(f"  {'unaccounted overhead':<{col_w}} {unaccounted:>10.3f} {unaccounted/loop_total*100:>7.2f}%")

    print()


def main():
    # Try to read JSON from first argument, else from stdin
    if len(sys.argv) > 1:
        raw = sys.argv[1]
        try:
            timings = json.loads(raw)
        except json.JSONDecodeError:
            print(f"Error: could not parse argument as JSON: {raw}", file=sys.stderr)
            sys.exit(1)
    else:
        stdin_data = sys.stdin.read().strip()
        # Try JSON first, then fall back to TIMER line format
        try:
            timings = json.loads(stdin_data)
        except json.JSONDecodeError:
            timings = parse_timer_lines(stdin_data.splitlines())

    analyze(timings)


if __name__ == "__main__":
    main()
