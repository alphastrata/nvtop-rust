#!/usr/bin/env bash
set -euo pipefail
set -e
echo "=== STEP 1: CLEANING UP ==="
rm -f /tmp/telemetry_raw.jsonl telemetry_1k.jsonl

echo "=== STEP 2: RUNNING RUST DAEMON (60s) ==="
# Run for exactly 60 seconds to ensure we get >1000 lines at ~50ms intervals
timeout 60 uv run cargo run --bin nvtop -- -D -o /tmp/telemetry_raw.jsonl -d 30 > /dev/null 2>&1

echo "=== STEP 3: RAW FILE CHECK ==="
wc -l < /tmp/telemetry_raw.jsonl

echo "=== STEP 4: RUNNING PYTHON FETCHER ==="
uv run python3 scripts/fetch_telemetry.py --input /tmp/telemetry_raw.jsonl --target 1000 -o telemetry_1k.jsonl

echo "=== STEP 5: FINAL VERIFICATION ==="
TOTAL_LINES=$(wc -l < telemetry_1k.jsonl)
echo "Total valid JSONL lines: $TOTAL_LINES"

# Show a sample
echo "--- Sample Entry (jq -c): ---"
head -n 1 telemetry_1k.jsonl | jq -c .
