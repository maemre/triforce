#!/usr/bin/env bash
set -euo pipefail

TAIL=10
JOB_ID=""

usage() {
    cat <<'EOF'
Usage: cloud/status.sh --bucket BUCKET [--job-id JOB_ID] [--tail N]

  --bucket BUCKET    GCS bucket (required)
  --job-id JOB_ID    Show one specific job (default: show all jobs in bucket)
  --tail N           Print last N lines of each instance's stdout (default: 10, 0 to suppress)
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --bucket)  BUCKET="$2";  shift 2 ;;
        --job-id)  JOB_ID="$2";  shift 2 ;;
        --tail)    TAIL="$2";    shift 2 ;;
        *)  echo "Unknown argument: $1"; usage ;;
    esac
done

: "${BUCKET:?--bucket is required}"

# Collect job IDs to report on.
if [[ -n "$JOB_ID" ]]; then
    JOB_IDS=("$JOB_ID")
else
    # List all jobs in the bucket.
    mapfile -t JOB_IDS < <(
        gsutil ls "${BUCKET}/jobs/" 2>/dev/null \
            | sed 's|.*/jobs/||; s|/||' \
            | sort
    )
    if [[ ${#JOB_IDS[@]} -eq 0 ]]; then
        echo "No jobs found in ${BUCKET}/jobs/"
        exit 0
    fi
fi

show_job() {
    local job="$1"
    local gcs_input="${BUCKET}/jobs/${job}/input"
    local gcs_output="${BUCKET}/jobs/${job}/output"

    local manifest_tmp
    manifest_tmp=$(mktemp)
    trap "rm -f $manifest_tmp" RETURN

    gsutil -q cat "${gcs_input}/manifest.json" > "$manifest_tmp" 2>/dev/null || {
        echo "  [manifest missing -- job may still be uploading]"
        return
    }

    local n total zone machine
    n=$(python3       -c "import json; d=json.load(open('$manifest_tmp')); print(d['num_instances'])")
    total=$(python3   -c "import json; d=json.load(open('$manifest_tmp')); print(d['total_covers'])")
    zone=$(python3    -c "import json; d=json.load(open('$manifest_tmp')); print(d['zone'])")
    machine=$(python3 -c "import json; d=json.load(open('$manifest_tmp')); print(d['machine_type'])")

    echo "  instances: $n  covers: $total  zone: $zone  machine: $machine"

    local live
    live=$(gcloud compute instances list \
        --filter="name~^job-${job}-" \
        --format="value(name)" 2>/dev/null || true)

    local all_done=true
    local any_failed=false

    for (( i=0; i<n; i++ )); do
        local iname="job-${job}-${i}"
        local label code

        if gsutil -q stat "${gcs_output}/instance-${i}.done" 2>/dev/null; then
            code=$(gsutil -q cat "${gcs_output}/instance-${i}.done" 2>/dev/null | tr -d '[:space:]')
            if [[ "$code" == "0" ]]; then
                label="DONE    exit=0  pass"
            else
                label="DONE    exit=${code}  FAILED"
                any_failed=true
            fi
        elif echo "$live" | grep -qF "$iname"; then
            label="RUNNING"
            all_done=false
        else
            label="MISSING (not running, no done marker)"
            all_done=false
        fi

        echo "  instance-${i}: ${label}"

        if [[ "$TAIL" -gt 0 ]]; then
            local stdout_lines
            stdout_lines=$(gsutil -q cat "${gcs_output}/instance-${i}.stdout" 2>/dev/null \
                | tail -n "$TAIL" || true)
            if [[ -n "$stdout_lines" ]]; then
                echo "$stdout_lines" | sed "s/^/    | /"
            fi
        fi
    done

    if gsutil -q stat "${gcs_output}/STOP" 2>/dev/null; then
        local reason
        reason=$(gsutil -q cat "${gcs_output}/STOP" 2>/dev/null | tr -d '\n' || echo "(unreadable)")
        echo "  STOP: ${reason}"
    fi
}

SEP="──────────────────────────────────────────────"

for job in "${JOB_IDS[@]}"; do
    echo "$SEP"
    echo "Job: $job"
    show_job "$job"
    echo ""
done
