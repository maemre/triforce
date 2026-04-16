#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: cloud/nuke.sh --bucket BUCKET [--zone ZONE]

Hard kill: delete ALL VMs whose name starts with "job-" in the given zone
(or all zones if omitted), regardless of job tracking or manifests.
Also deletes all job data in the bucket.

Use this to recover from botched initialization or orphaned instances.

  --bucket BUCKET    GCS bucket (required)
  --zone ZONE        Only delete instances in this zone (default: all zones)
EOF
    exit 1
}

ZONE_FILTER=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --bucket)  BUCKET="$2";  shift 2 ;;
        --zone)    ZONE_FILTER="$2"; shift 2 ;;
        *)  echo "Unknown argument: $1"; usage ;;
    esac
done

: "${BUCKET:?--bucket is required}"

echo "=== Deleting all job-* instances ==="

FILTER="name~^job-"
if [[ -n "$ZONE_FILTER" ]]; then
    FILTER="${FILTER} AND zone~${ZONE_FILTER}"
fi

instances=$(gcloud compute instances list \
    --filter="$FILTER" \
    --format="value(name,zone)" 2>/dev/null || true)

if [[ -z "$instances" ]]; then
    echo "No job-* instances found."
else
    echo "$instances" | while read -r name iz; do
        echo "  Deleting $name ($iz) ..."
        gcloud compute instances delete "$name" --zone="$iz" --quiet 2>&1 | grep -v "^Deleted" || true
    done
fi

echo ""
echo "=== Removing all job data from ${BUCKET}/jobs/ ==="
gsutil -m rm -r "${BUCKET}/jobs/" 2>/dev/null || echo "  No job data in bucket."

echo ""
echo "Done. Everything cleaned up."
