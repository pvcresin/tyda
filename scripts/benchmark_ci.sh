#!/usr/bin/env bash
set -euo pipefail

# Compare the current checkout with its base on the same CI runner.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBJECT_PATH="${TYDA_PERF_SUBJECT:-$ROOT_DIR/subject/gitlab/app}"
RUNS="${TYDA_PERF_RUNS:-5}"
THREADS="${TYDA_PERF_THREADS:-2}"
TIMEOUT_SECONDS="${TYDA_PERF_TIMEOUT_SECONDS:-180}"
BASE_REF="${TYDA_PERF_BASE_REF:-}"
BASE_SHA="${TYDA_PERF_BASE_SHA:-}"
HEAD_SHA="${TYDA_PERF_HEAD_SHA:-}"
OUTPUT_DIR="${TYDA_PERF_OUTPUT_DIR:-$ROOT_DIR/target/performance}"
ALLOW_BASE_TIMEOUT="${TYDA_PERF_ALLOW_BASE_TIMEOUT:-0}"
BINARY_DIR="${TYDA_PERF_BINARY_DIR:-}"
RBS_DIR="${TYDA_RBS_DIR:-$ROOT_DIR/vendor/rbs}"

if [[ "$OUTPUT_DIR" != /* ]]; then
  OUTPUT_DIR="$ROOT_DIR/$OUTPUT_DIR"
fi
if [[ -n "$BINARY_DIR" && "$BINARY_DIR" != /* ]]; then
  BINARY_DIR="$ROOT_DIR/$BINARY_DIR"
fi
if [[ "$RBS_DIR" != /* ]]; then
  RBS_DIR="$ROOT_DIR/$RBS_DIR"
fi

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
  echo "run ./scripts/setup_subjects.sh <subject> first" >&2
  exit 2
fi
if [[ ! -d "$RBS_DIR" ]]; then
  echo "vendor/rbs is missing: $RBS_DIR" >&2
  echo "run ./scripts/vendor-rbs.sh first" >&2
  exit 2
fi
if [[ "$ALLOW_BASE_TIMEOUT" != 0 && "$ALLOW_BASE_TIMEOUT" != 1 ]]; then
  echo "TYDA_PERF_ALLOW_BASE_TIMEOUT must be 0 or 1" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR"
RUN_DIR="$(mktemp -d "$OUTPUT_DIR/run.XXXXXX")"
METRICS_TSV="$RUN_DIR/metrics.tsv"
RESULT_JSON="$OUTPUT_DIR/result.json"
TARGET_ROOT="${TYDA_PERF_TARGET_DIR:-$OUTPUT_DIR/targets}"
if [[ "$TARGET_ROOT" != /* ]]; then
  TARGET_ROOT="$ROOT_DIR/$TARGET_ROOT"
fi
BASE_DIR="$RUN_DIR/base"
HEAD_DIR="$ROOT_DIR"
BASE_TARGET="$TARGET_ROOT/base"
HEAD_TARGET="$TARGET_ROOT/head"
BASE_WORKTREE_ADDED=0
mkdir -p "$TARGET_ROOT"
touch "$METRICS_TSV"
PERF_BINARY_DIR="$BINARY_DIR"

cleanup() {
  if [[ "$BASE_WORKTREE_ADDED" -eq 1 || -d "$BASE_DIR" ]]; then
    git -C "$ROOT_DIR" worktree remove --force "$BASE_DIR" >/dev/null 2>&1 || true
  fi
  rm -rf "$RUN_DIR"
}
trap cleanup EXIT

if [[ -z "$BASE_SHA" && ( -z "$BASE_REF" || "$BASE_REF" =~ ^0+$ ) ]]; then
  BASE_REF="$(git -C "$ROOT_DIR" rev-parse HEAD^)"
fi

if [[ -z "$BASE_SHA" ]]; then
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
fi
if [[ -z "$HEAD_SHA" ]]; then
  HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse 'HEAD^{commit}')"
fi

SUBJECT_REF="$(git -C "$SUBJECT_PATH" rev-parse HEAD 2>/dev/null || true)"

echo "=== Performance gate ==="
echo "subject: $SUBJECT_PATH"
echo "runs: $RUNS (paired, alternating, median)"
echo "threads: $THREADS"
echo "base ref: ${BASE_REF:-provided via SHA}"
echo "base sha: $BASE_SHA"
echo "head: $HEAD_SHA"
echo "subject revision: ${SUBJECT_REF:-unknown}"
echo ""

export CARGO_INCREMENTAL=0

build_variant() {
  local variant="$1"
  local repo="$2"
  local target="$3"
  local build_log="$RUN_DIR/$variant-build.log"
  local binary_dir="$RUN_DIR/bin/$variant"

  echo "Building $variant release binary..."
  if ! (cd "$repo" && CARGO_TARGET_DIR="$target" cargo build --locked --release --bin tyda >"$build_log" 2>&1); then
    cat "$build_log" >&2
    exit 1
  fi
  if [[ ! -x "$target/release/tyda" ]]; then
    echo "failed to find the $variant performance binary" >&2
    cat "$build_log" >&2
    exit 1
  fi
  mkdir -p "$binary_dir"
  cp "$target/release/tyda" "$binary_dir/tyda"
}

if [[ -n "$BINARY_DIR" ]]; then
  for variant in base head; do
    binary="$BINARY_DIR/$variant/tyda"
    if [[ -f "$binary" ]]; then
      chmod +x "$binary"
    fi
    if [[ ! -x "$binary" ]]; then
      echo "performance binary not found or not executable: $binary" >&2
      exit 2
    fi
  done
else
  git -C "$ROOT_DIR" worktree add --detach "$BASE_DIR" "$BASE_SHA" >/dev/null
  BASE_WORKTREE_ADDED=1
  mkdir -p "$BASE_DIR/vendor"
  ln -s "$RBS_DIR" "$BASE_DIR/vendor/rbs"
  build_variant base "$BASE_DIR" "$BASE_TARGET"
  build_variant head "$HEAD_DIR" "$HEAD_TARGET"
  PERF_BINARY_DIR="$RUN_DIR/bin"
fi

measure_process() {
  local meta_path="$1"
  local log_path="$2"
  local allow_timeout="$3"
  shift 3
  local exit_code

  MEASURE_TIMED_OUT=0
  set +e
  ruby "$ROOT_DIR/scripts/measure_process.rb" \
    --log "$log_path" \
    --output "$meta_path" \
    --timeout "$TIMEOUT_SECONDS" \
    -- "$@"
  exit_code=$?
  set -e
  if [[ "$exit_code" -ne 0 ]]; then
    if [[ "$exit_code" -eq 124 && "$allow_timeout" == 1 ]]; then
      MEASURE_TIMED_OUT=1
      echo "benchmark command timed out; accepting base timeout as the comparison limit: $*" >&2
      return 0
    fi
    echo "benchmark command failed (status=$exit_code): $*" >&2
    cat "$log_path" >&2
    exit "$exit_code"
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
  ruby - "$1" <<'RUBY'
text = File.read(ARGV.fetch(0))
match = text.match(/\[bench\] workspace scan: ([0-9.]+)ms/)
unless match
  warn text
  abort "failed to parse LSP workspace scan time"
end
puts match[1].to_f.round
RUBY
}

measure_cli() {
  local run="$1"
  local variant="$2"
  local binary="$PERF_BINARY_DIR/$variant/tyda"
  local meta="$RUN_DIR/cli-$variant-$run.meta"
  local log="$RUN_DIR/cli-$variant-$run.log"
  local allow_timeout=0
  if [[ "$variant" == base && "$ALLOW_BASE_TIMEOUT" == 1 ]]; then
    allow_timeout=1
  fi

  measure_process "$meta" "$log" "$allow_timeout" \
    env TYDA_CLI_ANALYSIS_THREADS="$THREADS" \
    TYDA_RBS_DIR="$RBS_DIR" \
    nice -n 19 "$binary" "$SUBJECT_PATH"
  local elapsed_ms rss_bytes
  IFS=$'\t' read -r _ rss_bytes < <(read_meta "$meta")
  if [[ "$MEASURE_TIMED_OUT" == 1 ]]; then
    elapsed_ms=$((TIMEOUT_SECONDS * 1000))
    rss_bytes=-1
  else
    IFS=$'\t' read -r elapsed_ms _ < <(read_meta "$meta")
  fi
  if [[ "$run" != "warmup" ]]; then
    record "$run" "$variant" cli_elapsed_ms "$elapsed_ms"
    record "$run" "$variant" cli_max_rss_bytes "$rss_bytes"
  fi
}

measure_lsp() {
  local run="$1"
  local variant="$2"
  local binary="$PERF_BINARY_DIR/$variant/tyda"
  local meta="$RUN_DIR/lsp-$variant-$run.meta"
  local log="$RUN_DIR/lsp-$variant-$run.log"
  local allow_timeout=0
  if [[ "$variant" == base && "$ALLOW_BASE_TIMEOUT" == 1 ]]; then
    allow_timeout=1
  fi

  measure_process "$meta" "$log" "$allow_timeout" \
    env TYDA_LSP_BENCH_ROOT="$SUBJECT_PATH" \
    TYDA_LSP_ANALYSIS_THREADS="$THREADS" \
    TYDA_RBS_DIR="$RBS_DIR" \
    nice -n 19 ruby "$ROOT_DIR/scripts/benchmark_lsp_client.rb" "$binary" "$SUBJECT_PATH"
  local scan_ms rss_bytes
  if [[ "$MEASURE_TIMED_OUT" == 1 ]]; then
    scan_ms=$((TIMEOUT_SECONDS * 1000))
    rss_bytes=-1
  else
    scan_ms="$(scan_ms_from_log "$log")"
    IFS=$'\t' read -r _ rss_bytes < <(read_meta "$meta")
  fi
  if [[ "$run" != "warmup" ]]; then
    record "$run" "$variant" lsp_scan_ms "$scan_ms"
    record "$run" "$variant" lsp_max_rss_bytes "$rss_bytes"
  fi
}

# The first paired sample warms the shared filesystem cache; the three-sample
# median excludes that cold-side sample without four extra full analyses.
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
ruby "$ROOT_DIR/scripts/compare_performance.rb" \
  --base-sha "$BASE_SHA" \
  --head-sha "$HEAD_SHA" \
  --metrics "$METRICS_TSV" \
  --output "$RESULT_JSON" \
  --runs "$RUNS" \
  --subject "$SUBJECT_PATH" \
  --subject-ref "$SUBJECT_REF" \
  --time-warn-percent "$TIME_WARN_PERCENT" \
  --time-fail-percent "$TIME_FAIL_PERCENT" \
  --memory-warn-percent "$MEMORY_WARN_PERCENT" \
  --memory-fail-percent "$MEMORY_FAIL_PERCENT" \
  --min-time-delta-ms "$MIN_TIME_DELTA_MS" \
  --min-memory-delta-bytes "$MIN_MEMORY_DELTA_BYTES"
