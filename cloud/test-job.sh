#!/usr/bin/env bash
# Quick test: 2 e4-micro spot VMs in europe-west9-a against the size-2 test data.
# Set TRIFORCE_BUCKET to your GCS bucket before running.
#
# Usage:
#   TRIFORCE_BUCKET=gs://my-bucket ./cloud/test-job.sh
set -euo pipefail

: "${TRIFORCE_BUCKET:?set TRIFORCE_BUCKET to your GCS bucket (e.g. gs://my-bucket)}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$REPO_ROOT"

bash build-musl.sh

exec "${SCRIPT_DIR}/run.sh" \
    --bucket          "$TRIFORCE_BUCKET" \
    --covers          size-2/triangle-6-top-covers.json \
    --partial-tiling  size-2/1-lines.json \
    --tile-size       2 \
    --instances       2 \
    --machine-type    e2-standard-4 \
    --zone            europe-west9-a \
    --threads         4 \
    --flush-interval  10 \
    --job-id          "test-$(date +%s)" \
    --wait
