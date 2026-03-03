#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: scripts/benchmark.sh <path> [runs]" >&2
  exit 1
fi

TARGET_PATH="$1"
RUNS="${2:-5}"

if [[ ! -e "$TARGET_PATH" ]]; then
  echo "benchmark target not found: $TARGET_PATH" >&2
  exit 1
fi

echo "=== Benchmark Target ==="
echo "path: $TARGET_PATH"
echo "runs: $RUNS"
echo ""

echo "=== Release Build ==="
cargo build --release --bin tyda >/dev/null

echo "=== Timed Runs ==="

total_ms=0
min_ms=""
max_ms=0

for run in $(seq 1 "$RUNS"); do
  start_ns=$(python3 - <<'PY'
import time
print(time.time_ns())
PY
)
  cargo run --release --bin tyda -- "$TARGET_PATH" >/dev/null
  end_ns=$(python3 - <<'PY'
import time
print(time.time_ns())
PY
)
  elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
  echo "run=$run elapsed_ms=$elapsed_ms"
  total_ms=$(( total_ms + elapsed_ms ))
  if [[ -z "$min_ms" || "$elapsed_ms" -lt "$min_ms" ]]; then
    min_ms="$elapsed_ms"
  fi
  if [[ "$elapsed_ms" -gt "$max_ms" ]]; then
    max_ms="$elapsed_ms"
  fi
done

avg_ms=$(( total_ms / RUNS ))

echo ""
echo "=== Summary ==="
echo "avg_ms=$avg_ms"
echo "min_ms=$min_ms"
echo "max_ms=$max_ms"
