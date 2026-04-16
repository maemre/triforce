#!/usr/bin/env bash
set -euo pipefail

COVERS=size-2/triangle-6-top-covers.json

# Generate the covers file if it doesn't exist yet
if [ ! -f "$COVERS" ]; then
    echo "Generating $COVERS ..."
    cargo run --release --bin dump-potential-covers --no-default-features -- \
        file=size-2/triangle-6.json triangle=7 2 > "$COVERS"
fi

N=$(python3 -c "import json; print(len(json.load(open('$COVERS'))))")
echo "Total covers: $N"

cargo build --release --bin check-covers --bin check-given-covers --no-default-features 2>/dev/null

for i in 1 2 3 4 5; do
    PT="size-2/${i}-lines.json"
    echo "=== partial tiling: $PT ==="

    out_orig=$(./target/release/check-covers \
        file=size-2/triangle-6.json triangle=7 2 "$PT" 2>/dev/null)

    out_new=$(./target/release/check-given-covers \
        "$COVERS" 2 "$PT" 0 "$N" 2>/dev/null)

    echo "check-covers:       $out_orig"
    echo "check-given-covers: $out_new"

    orig_pass=$(echo "$out_orig" | grep -c "^found" || true)
    new_pass=$(echo  "$out_new"  | grep -c "^all pass" || true)

    if [ "$orig_pass" -gt 0 ] && [ "$new_pass" -gt 0 ]; then
        echo "MATCH (both pass)"
    elif [ "$orig_pass" -eq 0 ] && [ "$new_pass" -eq 0 ]; then
        echo "MATCH (both fail)"
    else
        echo "MISMATCH"
        exit 1
    fi
    echo
done

echo "All tests passed."
