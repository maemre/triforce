#!/usr/bin/env bash
set -euo pipefail

OUTPUT_DIR="./outputs"

usage() {
    echo "Usage: $0 --bucket BUCKET --job-id JOB_ID [--output-dir DIR]"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --bucket)      BUCKET="$2";     shift 2 ;;
        --job-id)      JOB_ID="$2";     shift 2 ;;
        --output-dir)  OUTPUT_DIR="$2"; shift 2 ;;
        *)  echo "Unknown argument: $1"; usage ;;
    esac
done

: "${BUCKET:?--bucket is required}"
: "${JOB_ID:?--job-id is required}"

GCS_OUTPUT="${BUCKET}/jobs/${JOB_ID}/output"

mkdir -p "$OUTPUT_DIR"
echo "Downloading outputs from ${GCS_OUTPUT}/ to ${OUTPUT_DIR}/ ..."

gsutil -m cp "${GCS_OUTPUT}/**" "$OUTPUT_DIR/" 2>/dev/null || {
    echo "No output files found at ${GCS_OUTPUT}"
    exit 1
}

echo ""
echo "Summary:"
FAIL=false
for f in "${OUTPUT_DIR}"/instance-*.stdout; do
    [[ -f "$f" ]] || continue
    INST=$(basename "$f" .stdout)
    if grep -q "all pass" "$f"; then
        echo "  ${INST}: PASS"
    elif grep -q "some covers failed" "$f"; then
        echo "  ${INST}: FAIL"
        FAIL=true
    else
        echo "  ${INST}: incomplete (no final verdict line)"
    fi
done

if [[ -f "${OUTPUT_DIR}/STOP" ]]; then
    echo ""
    echo "STOP marker: $(cat "${OUTPUT_DIR}/STOP")"
fi

if [[ "$FAIL" == "true" ]]; then
    echo ""
    echo "Some instances reported failure. Check stdout files in ${OUTPUT_DIR}/ for details."
    exit 1
fi
