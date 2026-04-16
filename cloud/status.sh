#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "Usage: $0 --bucket BUCKET --job-id JOB_ID"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --bucket)  BUCKET="$2";  shift 2 ;;
        --job-id)  JOB_ID="$2";  shift 2 ;;
        *)  echo "Unknown argument: $1"; usage ;;
    esac
done

: "${BUCKET:?--bucket is required}"
: "${JOB_ID:?--job-id is required}"

GCS_INPUT="${BUCKET}/jobs/${JOB_ID}/input"
GCS_OUTPUT="${BUCKET}/jobs/${JOB_ID}/output"

MANIFEST_TMP=$(mktemp)
trap "rm -f $MANIFEST_TMP" EXIT

gsutil -q cat "${GCS_INPUT}/manifest.json" > "$MANIFEST_TMP" 2>/dev/null || {
    echo "ERROR: manifest not found at ${GCS_INPUT}/manifest.json"
    echo "Is the job ID correct?"
    exit 1
}

NUM_INSTANCES=$(python3 -c "import json; d=json.load(open('$MANIFEST_TMP')); print(d['num_instances'])")
TOTAL=$(python3 -c "import json; d=json.load(open('$MANIFEST_TMP')); print(d['total_covers'])")
ZONE=$(python3 -c "import json; d=json.load(open('$MANIFEST_TMP')); print(d['zone'])")
MACHINE_TYPE=$(python3 -c "import json; d=json.load(open('$MANIFEST_TMP')); print(d['machine_type'])")

echo "Job:       $JOB_ID"
echo "Instances: $NUM_INSTANCES  (covering $TOTAL covers)"
echo "Zone:      $ZONE  Machine: $MACHINE_TYPE"
echo ""

# Fetch names of live instances that belong to this job.
LIVE=$(gcloud compute instances list \
    --filter="name~^job-${JOB_ID}-" \
    --format="value(name)" 2>/dev/null || true)

for (( i=0; i<NUM_INSTANCES; i++ )); do
    NAME="job-${JOB_ID}-${i}"
    if gsutil -q stat "${GCS_OUTPUT}/instance-${i}.done" 2>/dev/null; then
        CODE=$(gsutil -q cat "${GCS_OUTPUT}/instance-${i}.done" 2>/dev/null | tr -d '[:space:]')
        if [[ "$CODE" == "0" ]]; then
            echo "  instance-${i}: DONE  exit=0  (pass)"
        else
            echo "  instance-${i}: DONE  exit=${CODE}  (FAILED)"
        fi
    elif echo "$LIVE" | grep -qF "$NAME"; then
        echo "  instance-${i}: RUNNING"
    else
        echo "  instance-${i}: MISSING (no done marker and not running -- preempted?)"
    fi
done

echo ""
if gsutil -q stat "${GCS_OUTPUT}/STOP" 2>/dev/null; then
    REASON=$(gsutil -q cat "${GCS_OUTPUT}/STOP" 2>/dev/null | tr -d '\n' || echo "(unreadable)")
    echo "STOP marker: ${REASON}"
else
    echo "No STOP marker."
fi
