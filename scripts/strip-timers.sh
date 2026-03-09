#!/usr/bin/env bash
# Strip TIMER lines from check-covers output so the remainder can be diff'd
# against a baseline.
#
# Usage (pipe):
#   cargo run --release --bin check-covers -- ... | bash scripts/strip-timers.sh
#
# Usage (file):
#   bash scripts/strip-timers.sh output.txt
grep -v '^TIMER ' "$@"
