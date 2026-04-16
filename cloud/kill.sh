#!/usr/bin/env bash
set -euo pipefail

JOB_ID=""

usage() {
    cat <<'EOF'
Usage: cloud/kill.sh --bucket BUCKET [--job-id JOB_ID]

Kill running job instances by writing a STOP marker and deleting VMs.
If --job-id is omitted, kills all jobs in the bucket.

  --bucket BUCKET    GCS bucket (required)
  --job-id JOB_ID    Kill one specific job (default: kill all jobs)
EOF
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

if [[ -n "$JOB_ID" ]]; then
    JOB_IDS=("$JOB_ID")
else
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

for job in "${JOB_IDS[@]}"; do
    echo "Killing job: $job"

    # Write STOP marker so any instance that checks before deletion will exit.
    printf "killed by cloud/kill.sh\n" \
        | gsutil cp - "${BUCKET}/jobs/${job}/output/STOP" 2>/dev/null || true

    # Read zone from manifest.
    zone=$(gsutil -q cat "${BUCKET}/jobs/${job}/input/manifest.json" 2>/dev/null \
        | python3 -c "import json,sys; print(json.load(sys.stdin)['zone'])" 2>/dev/null \
        || echo "")

    # Find and delete live instances for this job.
    instances=$(gcloud compute instances list \
        --filter="name~^job-${job}-" \
        --format="value(name,zone)" 2>/dev/null || true)

    if [[ -z "$instances" ]]; then
        echo "  No running instances."
    else
        echo "$instances" | while read -r name iz; do
            echo "  Deleting $name ($iz) ..."
            gcloud compute instances delete "$name" --zone="$iz" --quiet 2>&1 | grep -v "^Deleted" || true
        done
    fi
done

echo "Done."
