#!/usr/bin/env bash
# Runs on each GCE instance as the startup script.
# Not called directly -- injected via --metadata-from-file=startup-script.

META_BASE="http://metadata.google.internal/computeMetadata/v1/instance"

meta() {
    curl -sf "${META_BASE}/attributes/$1" -H "Metadata-Flavor: Google"
}

BUCKET=$(meta bucket)
JOB_ID=$(meta job-id)
INSTANCE_IDX=$(meta instance-idx)
RANGE_START=$(meta range-start)
RANGE_END=$(meta range-end)
TILE_SIZE=$(meta tile-size)
FLUSH_INTERVAL=$(meta flush-interval)
THREADS=$(meta threads)

GCS_INPUT="${BUCKET}/jobs/${JOB_ID}/input"
GCS_OUTPUT="${BUCKET}/jobs/${JOB_ID}/output"
WORK_DIR="/tmp/triforce"
STDOUT="${WORK_DIR}/stdout.txt"
STDERR="${WORK_DIR}/stderr.txt"

echo "triforce instance ${INSTANCE_IDX}: covers [${RANGE_START}, ${RANGE_END}), threads=${THREADS}"

mkdir -p "$WORK_DIR"

echo "Downloading inputs from ${GCS_INPUT}/ ..."
gsutil cp "${GCS_INPUT}/check-given-covers"  "${WORK_DIR}/check-given-covers"
gsutil cp "${GCS_INPUT}/covers.json"         "${WORK_DIR}/covers.json"
gsutil cp "${GCS_INPUT}/partial-tiling.json" "${WORK_DIR}/partial-tiling.json"
chmod +x "${WORK_DIR}/check-given-covers"

echo "Starting check-given-covers ..."
RAYON_NUM_THREADS="$THREADS" "${WORK_DIR}/check-given-covers" \
    "${WORK_DIR}/covers.json" \
    "$TILE_SIZE" \
    "${WORK_DIR}/partial-tiling.json" \
    "$RANGE_START" \
    "$RANGE_END" \
    >"$STDOUT" 2>"$STDERR" &
MAIN_PID=$!

# Background flush loop: upload output periodically and watch for STOP signal.
(
    while kill -0 "$MAIN_PID" 2>/dev/null; do
        sleep "$FLUSH_INTERVAL"
        gsutil -q cp "$STDOUT" "${GCS_OUTPUT}/instance-${INSTANCE_IDX}.stdout" 2>/dev/null || true
        gsutil -q cp "$STDERR" "${GCS_OUTPUT}/instance-${INSTANCE_IDX}.stderr" 2>/dev/null || true
        if gsutil -q stat "${GCS_OUTPUT}/STOP" 2>/dev/null; then
            echo "STOP marker detected, terminating" >&2
            kill "$MAIN_PID" 2>/dev/null || true
        fi
    done
) &
FLUSH_PID=$!

# Wait for the main process.
wait "$MAIN_PID" 2>/dev/null; EXIT_CODE=$?

kill "$FLUSH_PID" 2>/dev/null || true
wait "$FLUSH_PID" 2>/dev/null || true

# Final flush.
gsutil -q cp "$STDOUT" "${GCS_OUTPUT}/instance-${INSTANCE_IDX}.stdout" 2>/dev/null || true
gsutil -q cp "$STDERR" "${GCS_OUTPUT}/instance-${INSTANCE_IDX}.stderr" 2>/dev/null || true

echo "check-given-covers exited with code ${EXIT_CODE}"

# Write STOP only if this instance found a genuine counterexample.
if grep -q "some covers failed" "$STDOUT" 2>/dev/null; then
    printf "instance %s found counterexample (exit=%s)\n" "$INSTANCE_IDX" "$EXIT_CODE" \
        | gsutil cp - "${GCS_OUTPUT}/STOP" 2>/dev/null || true
fi

# Write done marker (contains the exit code).
printf "%s\n" "$EXIT_CODE" \
    | gsutil cp - "${GCS_OUTPUT}/instance-${INSTANCE_IDX}.done" || true

echo "Done marker written."

# Self-delete.
SELF_ZONE=$(curl -sf "${META_BASE}/zone" -H "Metadata-Flavor: Google" | cut -d/ -f4)
SELF_NAME=$(curl -sf "${META_BASE}/name" -H "Metadata-Flavor: Google")
echo "Self-deleting ${SELF_NAME} in ${SELF_ZONE} ..."
gcloud compute instances delete "$SELF_NAME" --zone="$SELF_ZONE" --quiet
