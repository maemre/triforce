#!/usr/bin/env bash
set -euo pipefail

BINARY="target/x86_64-unknown-linux-musl/release/check-given-covers"
MACHINE_TYPE="e2-standard-4"
ZONE="europe-west9-a"
NUM_INSTANCES=10
THREADS=4
FLUSH_INTERVAL=30
WAIT=false
JOB_ID="$(date +%s)"

usage() {
    cat <<'EOF'
Usage: cloud/run.sh --bucket BUCKET --covers FILE --partial-tiling FILE --tile-size N [options]

Required:
  --bucket BUCKET           GCS bucket (e.g. gs://my-bucket)
  --covers FILE             JSON covers file (output of dump-potential-covers)
  --partial-tiling FILE     JSON partial tiling file
  --tile-size N             Tile size

Optional:
  --job-id ID               Job identifier (default: Unix timestamp)
  --binary FILE             musl static binary (default: target/.../check-given-covers)
  --instances N             Number of spot VMs to create (default: 10)
  --machine-type TYPE       GCE machine type (default: e4-micro)
  --zone ZONE               GCE zone (default: europe-west9-a)
  --threads N               RAYON_NUM_THREADS on each instance (default: 4)
  --flush-interval N        Seconds between GCS output flushes (default: 30)
  --wait                    Block until all instances finish
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --bucket)          BUCKET="$2";          shift 2 ;;
        --covers)          COVERS="$2";          shift 2 ;;
        --partial-tiling)  PARTIAL_TILING="$2";  shift 2 ;;
        --tile-size)       TILE_SIZE="$2";        shift 2 ;;
        --job-id)          JOB_ID="$2";           shift 2 ;;
        --binary)          BINARY="$2";           shift 2 ;;
        --instances)       NUM_INSTANCES="$2";    shift 2 ;;
        --machine-type)    MACHINE_TYPE="$2";     shift 2 ;;
        --zone)            ZONE="$2";             shift 2 ;;
        --threads)         THREADS="$2";          shift 2 ;;
        --flush-interval)  FLUSH_INTERVAL="$2";   shift 2 ;;
        --wait)            WAIT=true;             shift   ;;
        *)  echo "Unknown argument: $1"; usage ;;
    esac
done

: "${BUCKET:?--bucket is required}"
: "${COVERS:?--covers is required}"
: "${PARTIAL_TILING:?--partial-tiling is required}"
: "${TILE_SIZE:?--tile-size is required}"

if [[ ! -f "$BINARY" ]]; then
    echo "Binary not found: $BINARY"
    echo "Run: bash build-musl.sh"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GCS_INPUT="${BUCKET}/jobs/${JOB_ID}/input"
GCS_OUTPUT="${BUCKET}/jobs/${JOB_ID}/output"

TOTAL=$(python3 -c "import json; print(len(json.load(open('${COVERS}'))))")

echo "Job ID:    $JOB_ID"
echo "Covers:    $TOTAL total, split across $NUM_INSTANCES instances"
echo "Zone:      $ZONE  Machine: $MACHINE_TYPE  Threads: $THREADS"
echo "GCS base:  ${BUCKET}/jobs/${JOB_ID}/"
echo ""

# Upload inputs
echo "Uploading inputs ..."
gsutil -q cp "$BINARY"          "${GCS_INPUT}/check-given-covers"
gsutil -q cp "$COVERS"          "${GCS_INPUT}/covers.json"
gsutil -q cp "$PARTIAL_TILING"  "${GCS_INPUT}/partial-tiling.json"

MANIFEST_TMP=$(mktemp)
trap "rm -f $MANIFEST_TMP" EXIT
python3 -c "
import json
print(json.dumps({
    'job_id':        '${JOB_ID}',
    'num_instances': ${NUM_INSTANCES},
    'total_covers':  ${TOTAL},
    'tile_size':     ${TILE_SIZE},
    'zone':          '${ZONE}',
    'machine_type':  '${MACHINE_TYPE}',
    'threads':       ${THREADS},
}, indent=2))
" > "$MANIFEST_TMP"
gsutil -q cp "$MANIFEST_TMP" "${GCS_INPUT}/manifest.json"

echo "Inputs uploaded."
echo ""

# Create spot instances
for (( i=0; i<NUM_INSTANCES; i++ )); do
    RANGE_START=$(( i * TOTAL / NUM_INSTANCES ))
    RANGE_END=$(( (i + 1) * TOTAL / NUM_INSTANCES ))
    INSTANCE_NAME="job-${JOB_ID}-${i}"
    echo "Creating ${INSTANCE_NAME}  covers [${RANGE_START}, ${RANGE_END}) ..."
    gcloud compute instances create "$INSTANCE_NAME" \
        --zone="$ZONE" \
        --machine-type="$MACHINE_TYPE" \
        --provisioning-model=SPOT \
        --instance-termination-action=DELETE \
        --scopes=cloud-platform \
        --metadata="bucket=${BUCKET},job-id=${JOB_ID},instance-idx=${i},range-start=${RANGE_START},range-end=${RANGE_END},tile-size=${TILE_SIZE},flush-interval=${FLUSH_INTERVAL},threads=${THREADS}" \
        --metadata-from-file=startup-script="${SCRIPT_DIR}/startup.sh" \
        --quiet 2>&1 | grep -v "^Created\|^NAME\|^job-" || true
done

echo ""
echo "All $NUM_INSTANCES instances created."
echo ""
echo "Check status:   ./cloud/status.sh --bucket ${BUCKET} --job-id ${JOB_ID}"
echo "Fetch outputs:  ./cloud/fetch.sh  --bucket ${BUCKET} --job-id ${JOB_ID}"

if [[ "$WAIT" == "true" ]]; then
    echo ""
    echo "Waiting for completion ..."
    while true; do
        DONE_COUNT=0
        for (( i=0; i<NUM_INSTANCES; i++ )); do
            if gsutil -q stat "${GCS_OUTPUT}/instance-${i}.done" 2>/dev/null; then
                DONE_COUNT=$(( DONE_COUNT + 1 ))
            fi
        done

        STOP_MSG=""
        if gsutil -q stat "${GCS_OUTPUT}/STOP" 2>/dev/null; then
            STOP_MSG="  [STOP]"
        fi

        echo "$(date '+%H:%M:%S')  done: ${DONE_COUNT}/${NUM_INSTANCES}${STOP_MSG}"

        if [[ "$DONE_COUNT" -eq "$NUM_INSTANCES" ]] || \
           [[ -n "$STOP_MSG" && "$DONE_COUNT" -gt 0 ]]; then
            break
        fi

        sleep 15
    done
    echo ""
    echo "Fetch outputs with:"
    echo "  ./cloud/fetch.sh --bucket ${BUCKET} --job-id ${JOB_ID}"
fi
