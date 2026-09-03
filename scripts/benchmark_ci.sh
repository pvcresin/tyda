#!/usr/bin/env bash
set -euo pipefail

# Compare the current checkout with its base on the same CI runner.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBJECT_PATH="${TYDA_PERF_SUBJECT:-$ROOT_DIR/subject/gitlab/app}"
RUNS="${TYDA_PERF_RUNS:-5}"
THREADS="${TYDA_PERF_THREADS:-2}"
TIMEOUT_SECONDS="${TYDA_PERF_TIMEOUT_SECONDS:-180}"
BASE_REF="${TYDA_PERF_BASE_REF:-}"
OUTPUT_DIR="${TYDA_PERF_OUTPUT_DIR:-$ROOT_DIR/target/performance}"

# A warning is useful for trend monitoring; only the larger limit fails PR CI.
TIME_WARN_PERCENT="15"
TIME_FAIL_PERCENT="30"
MEMORY_WARN_PERCENT="10"
MEMORY_FAIL_PERCENT="20"
MIN_TIME_DELTA_MS="100"
MIN_MEMORY_DELTA_BYTES="$((16 * 1024 * 1024))"

if ! [[ "$RUNS" =~ ^[1-9][0-9]*$ ]]; then
  echo "TYDA_PERF_RUNS must be a positive integer" >&2
  exit 2
fi
if ! [[ "$THREADS" =~ ^[1-9][0-9]*$ ]]; then
  echo "TYDA_PERF_THREADS must be a positive integer" >&2
  exit 2
fi
if [[ ! -d "$SUBJECT_PATH" ]]; then
  echo "performance subject not found: $SUBJECT_PATH" >&2
  echo "run ./scripts/setup_subjects.sh gitlab first" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR"
RUN_DIR="$(mktemp -d "$OUTPUT_DIR/run.XXXXXX")"
METRICS_TSV="$RUN_DIR/metrics.tsv"
RESULT_JSON="$OUTPUT_DIR/result.json"
TARGET_ROOT="$OUTPUT_DIR/targets"
BASE_DIR="$RUN_DIR/base"
HEAD_DIR="$ROOT_DIR"
BASE_TARGET="$TARGET_ROOT/base"
HEAD_TARGET="$TARGET_ROOT/head"
mkdir -p "$TARGET_ROOT"
touch "$METRICS_TSV"

cleanup() {
  if [[ -d "$BASE_DIR" ]]; then
    git -C "$ROOT_DIR" worktree remove --force "$BASE_DIR" >/dev/null 2>&1 || true
  fi
  rm -rf "$RUN_DIR"
}
trap cleanup EXIT

if [[ -z "$BASE_REF" || "$BASE_REF" =~ ^0+$ ]]; then
  BASE_REF="$(git -C "$ROOT_DIR" rev-parse HEAD^)"
fi

echo "=== Large application performance gate ==="
echo "subject: $SUBJECT_PATH"
echo "runs: $RUNS (paired, alternating, median)"
echo "threads: $THREADS"
echo "base: $BASE_REF"
echo "head: $(git -C "$ROOT_DIR" rev-parse HEAD)"
echo ""

if BASE_SHA="$(git -C "$ROOT_DIR" rev-parse --verify "${BASE_REF}^{commit}" 2>/dev/null)"; then
  :
else
  FETCH_REF="$BASE_REF"
  if [[ "$FETCH_REF" == origin/* ]]; then
    FETCH_REF="${FETCH_REF#origin/}"
  fi
  git -C "$ROOT_DIR" fetch --no-tags --depth=1 origin "$FETCH_REF" >/dev/null
  BASE_SHA="$(git -C "$ROOT_DIR" rev-parse 'FETCH_HEAD^{commit}')"
fi
HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse 'HEAD^{commit}')"
git -C "$ROOT_DIR" worktree add --detach "$BASE_DIR" "$BASE_SHA" >/dev/null

if [[ ! -e "$ROOT_DIR/vendor/rbs" ]]; then
  echo "vendor/rbs is missing; run ./scripts/vendor-rbs.sh first" >&2
  exit 2
fi
mkdir -p "$BASE_DIR/vendor"
ln -s "$ROOT_DIR/vendor/rbs" "$BASE_DIR/vendor/rbs"

export CARGO_INCREMENTAL=0

test_binary_for() {
  local json_path="$1"
  python3 - "$json_path" <<'PY'
import json
import sys

for line in open(sys.argv[1], encoding="utf-8"):
    try:
        value = json.loads(line)
    except json.JSONDecodeError:
        continue
    if value.get("reason") != "compiler-artifact":
        continue
    if (
        value.get("executable")
        and value.get("target", {}).get("name") == "tyda"
        and "lib" in value.get("target", {}).get("kind", [])
    ):
        print(value["executable"])
PY
}

build_variant() {
  local variant="$1"
  local repo="$2"
  local target="$3"
  local build_log="$RUN_DIR/$variant-build.log"
  local test_json="$RUN_DIR/$variant-test.json"
  local test_log="$RUN_DIR/$variant-test.log"

  echo "Building $variant release binary and LSP test harness..."
  if ! (cd "$repo" && CARGO_TARGET_DIR="$target" cargo build --locked --release --bin tyda >"$build_log" 2>&1); then
    cat "$build_log" >&2
    exit 1
  fi
  if ! (cd "$repo" && CARGO_TARGET_DIR="$target" cargo test --locked --release --lib --no-run --message-format=json >"$test_json" 2>"$test_log"); then
    cat "$test_log" >&2
    exit 1
  fi

  local test_binary
  test_binary="$(test_binary_for "$test_json")"
  if [[ -z "$test_binary" || ! -x "$test_binary" ]]; then
    echo "failed to find the $variant LSP test binary" >&2
    cat "$test_log" >&2
    exit 1
  fi
  printf '%s\n' "$test_binary" >"$RUN_DIR/$variant-test-binary"
}

build_variant base "$BASE_DIR" "$BASE_TARGET"
build_variant head "$HEAD_DIR" "$HEAD_TARGET"

measure_process() {
  local meta_path="$1"
  local log_path="$2"
  shift 2
  local status

  set +e
  python3 "$ROOT_DIR/scripts/measure_process.py" \
    --log "$log_path" \
    --output "$meta_path" \
    --timeout "$TIMEOUT_SECONDS" \
    -- "$@"
  status=$?
  set -e
  if [[ "$status" -ne 0 ]]; then
    echo "benchmark command failed (status=$status): $*" >&2
    cat "$log_path" >&2
    exit "$status"
  fi
}

record() {
  printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >>"$METRICS_TSV"
}

read_meta() {
  local meta_path="$1"
  local elapsed_ms rss_bytes
  IFS=$'\t' read -r elapsed_ms rss_bytes <"$meta_path"
  printf '%s\t%s\n' "$elapsed_ms" "$rss_bytes"
}

scan_ms_from_log() {
  python3 - "$1" <<'PY'
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read()
match = re.search(r"\[bench\] workspace scan: ([0-9.]+)ms", text)
if not match:
    print(text, file=sys.stderr)
    raise SystemExit("failed to parse LSP workspace scan time")
print(round(float(match.group(1))))
PY
}

measure_cli() {
  local run="$1"
  local variant="$2"
  local binary="$OUTPUT_DIR/targets/$variant/release/tyda"
  local meta="$RUN_DIR/cli-$variant-$run.meta"
  local log="$RUN_DIR/cli-$variant-$run.log"

  measure_process "$meta" "$log" \
    env TYDA_CLI_ANALYSIS_THREADS="$THREADS" \
    nice -n 19 "$binary" "$SUBJECT_PATH"
  local elapsed_ms rss_bytes
  IFS=$'\t' read -r elapsed_ms rss_bytes < <(read_meta "$meta")
  if [[ "$run" != "warmup" ]]; then
    record "$run" "$variant" cli_elapsed_ms "$elapsed_ms"
    record "$run" "$variant" cli_max_rss_bytes "$rss_bytes"
  fi
}

measure_lsp() {
  local run="$1"
  local variant="$2"
  local test_binary
  test_binary="$(<"$RUN_DIR/$variant-test-binary")"
  local meta="$RUN_DIR/lsp-$variant-$run.meta"
  local log="$RUN_DIR/lsp-$variant-$run.log"

  measure_process "$meta" "$log" \
    env TYDA_LSP_BENCH_ROOT="$SUBJECT_PATH" \
    TYDA_LSP_ANALYSIS_THREADS="$THREADS" \
    nice -n 19 "$test_binary" \
    lsp::tests::bench_display_analysis_mastodon_scale \
    --nocapture --test-threads=1
  local scan_ms rss_bytes
  scan_ms="$(scan_ms_from_log "$log")"
  IFS=$'\t' read -r _ rss_bytes < <(read_meta "$meta")
  if [[ "$run" != "warmup" ]]; then
    record "$run" "$variant" lsp_scan_ms "$scan_ms"
    record "$run" "$variant" lsp_max_rss_bytes "$rss_bytes"
  fi
}

echo ""
echo "=== Warmup ==="
measure_cli warmup base
measure_cli warmup head
measure_lsp warmup base
measure_lsp warmup head

echo ""
echo "=== Paired CLI runs ==="
for run in $(seq 1 "$RUNS"); do
  if ((run % 2 == 1)); then
    first="base"
    second="head"
  else
    first="head"
    second="base"
  fi
  measure_cli "$run" "$first"
  measure_cli "$run" "$second"
done

echo ""
echo "=== Paired LSP runs ==="
for run in $(seq 1 "$RUNS"); do
  if ((run % 2 == 1)); then
    first="base"
    second="head"
  else
    first="head"
    second="base"
  fi
  measure_lsp "$run" "$first"
  measure_lsp "$run" "$second"
done

echo ""
python3 "$ROOT_DIR/scripts/compare_performance.py" \
  --base-sha "$BASE_SHA" \
  --head-sha "$HEAD_SHA" \
  --metrics "$METRICS_TSV" \
  --output "$RESULT_JSON" \
  --runs "$RUNS" \
  --subject "$SUBJECT_PATH" \
  --time-warn-percent "$TIME_WARN_PERCENT" \
  --time-fail-percent "$TIME_FAIL_PERCENT" \
  --memory-warn-percent "$MEMORY_WARN_PERCENT" \
  --memory-fail-percent "$MEMORY_FAIL_PERCENT" \
  --min-time-delta-ms "$MIN_TIME_DELTA_MS" \
  --min-memory-delta-bytes "$MIN_MEMORY_DELTA_BYTES"
