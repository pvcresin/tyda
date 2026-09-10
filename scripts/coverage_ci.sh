#!/usr/bin/env bash
set -euo pipefail

# Compare one coverage run from the base and head release binaries.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBJECT_PATH="${TYDA_COVERAGE_SUBJECT:-$ROOT_DIR/subject/gitlab/app}"
THREADS="${TYDA_COVERAGE_THREADS:-2}"
TIMEOUT_SECONDS="${TYDA_COVERAGE_TIMEOUT_SECONDS:-180}"
BASE_SHA="${TYDA_COVERAGE_BASE_SHA:-unknown}"
HEAD_SHA="${TYDA_COVERAGE_HEAD_SHA:-}"
OUTPUT_DIR="${TYDA_COVERAGE_OUTPUT_DIR:-$ROOT_DIR/target/coverage}"
ALLOW_BASE_TIMEOUT="${TYDA_COVERAGE_ALLOW_BASE_TIMEOUT:-0}"
ALLOW_REGRESSIONS="${TYDA_COVERAGE_ALLOW_REGRESSIONS:-0}"
BINARY_DIR="${TYDA_COVERAGE_BINARY_DIR:-$ROOT_DIR/target/performance-runtime/bin}"
RBS_DIR="${TYDA_RBS_DIR:-$ROOT_DIR/vendor/rbs}"
BASE_RBS_DIR="${TYDA_COVERAGE_BASE_RBS_DIR:-$RBS_DIR}"
HEAD_RBS_DIR="${TYDA_COVERAGE_HEAD_RBS_DIR:-$RBS_DIR}"

if [[ "$SUBJECT_PATH" != /* ]]; then
  SUBJECT_PATH="$ROOT_DIR/$SUBJECT_PATH"
fi
if [[ "$OUTPUT_DIR" != /* ]]; then
  OUTPUT_DIR="$ROOT_DIR/$OUTPUT_DIR"
fi
if [[ "$BINARY_DIR" != /* ]]; then
  BINARY_DIR="$ROOT_DIR/$BINARY_DIR"
fi
if [[ "$RBS_DIR" != /* ]]; then
  RBS_DIR="$ROOT_DIR/$RBS_DIR"
fi
if [[ "$BASE_RBS_DIR" != /* ]]; then
  BASE_RBS_DIR="$ROOT_DIR/$BASE_RBS_DIR"
fi
if [[ "$HEAD_RBS_DIR" != /* ]]; then
  HEAD_RBS_DIR="$ROOT_DIR/$HEAD_RBS_DIR"
fi
if [[ -z "$HEAD_SHA" ]]; then
  HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
fi

if ! [[ "$THREADS" =~ ^[1-9][0-9]*$ ]]; then
  echo "TYDA_COVERAGE_THREADS must be a positive integer" >&2
  exit 2
fi
if ! [[ "$TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "TYDA_COVERAGE_TIMEOUT_SECONDS must be a positive integer" >&2
  exit 2
fi
if [[ "$ALLOW_BASE_TIMEOUT" != 0 && "$ALLOW_BASE_TIMEOUT" != 1 ]]; then
  echo "TYDA_COVERAGE_ALLOW_BASE_TIMEOUT must be 0 or 1" >&2
  exit 2
fi
if [[ "$ALLOW_REGRESSIONS" != 0 && "$ALLOW_REGRESSIONS" != 1 ]]; then
  echo "TYDA_COVERAGE_ALLOW_REGRESSIONS must be 0 or 1" >&2
  exit 2
fi
if [[ ! -d "$SUBJECT_PATH" ]]; then
  echo "coverage subject not found: $SUBJECT_PATH" >&2
  echo "run ./scripts/setup_subjects.sh <subject> first" >&2
  exit 2
fi
for rbs_dir in "$BASE_RBS_DIR" "$HEAD_RBS_DIR"; do
  if [[ ! -d "$rbs_dir" ]]; then
    echo "RBS directory is missing: $rbs_dir" >&2
    echo "run ./scripts/vendor-rbs.sh first" >&2
    exit 2
  fi
done

BASE_BINARY="$BINARY_DIR/base/tyda"
HEAD_BINARY="$BINARY_DIR/head/tyda"
for binary in "$BASE_BINARY" "$HEAD_BINARY"; do
  if [[ -f "$binary" ]]; then
    chmod +x "$binary"
  fi
  if [[ ! -x "$binary" ]]; then
    echo "coverage binary not found or not executable: $binary" >&2
    exit 2
  fi
done

mkdir -p "$OUTPUT_DIR"
BASE_REPORT="$OUTPUT_DIR/base.json"
HEAD_REPORT="$OUTPUT_DIR/head.json"
BASE_LOG="$OUTPUT_DIR/base.log"
HEAD_LOG="$OUTPUT_DIR/head.log"
BASE_META="$OUTPUT_DIR/base.meta"
HEAD_META="$OUTPUT_DIR/head.meta"
RESULT_JSON="$OUTPUT_DIR/result.json"
rm -f "$BASE_REPORT" "$HEAD_REPORT" "$BASE_LOG" "$HEAD_LOG" "$BASE_META" "$HEAD_META" "$RESULT_JSON"

SUBJECT_REF="$(git -C "$SUBJECT_PATH" rev-parse HEAD 2>/dev/null || true)"
BASE_TIMED_OUT=0

echo "=== Coverage gate ==="
echo "subject: $SUBJECT_PATH"
echo "threads: $THREADS"
echo "timeout: ${TIMEOUT_SECONDS}s"
echo "base: $BASE_SHA"
echo "head: $HEAD_SHA"
echo "base RBS: $BASE_RBS_DIR"
echo "head RBS: $HEAD_RBS_DIR"
echo "subject revision: ${SUBJECT_REF:-unknown}"
echo ""

run_coverage() {
  local variant="$1"
  local binary="$2"
  local report="$3"
  local log="$4"
  local meta="$5"
  local rbs_dir="$6"
  local exit_code

  echo "Running $variant coverage..."
  set +e
  ruby "$ROOT_DIR/scripts/measure_process.rb" \
    --log "$log" \
    --output "$meta" \
    --stdout "$report" \
    --timeout "$TIMEOUT_SECONDS" \
    -- env TYDA_CLI_ANALYSIS_THREADS="$THREADS" \
      TYDA_RBS_DIR="$rbs_dir" \
      nice -n 19 "$binary" --coverage "$SUBJECT_PATH"
  exit_code=$?
  set -e

  if [[ "$exit_code" -eq 124 && "$variant" == base && "$ALLOW_BASE_TIMEOUT" == 1 ]]; then
    BASE_TIMED_OUT=1
    echo "Base coverage timed out; comparison will be skipped."
    return 0
  fi
  if [[ "$exit_code" -ne 0 ]]; then
    echo "$variant coverage failed (status=$exit_code)" >&2
    cat "$log" >&2
    exit "$exit_code"
  fi
  if ! ruby -rjson -e 'JSON.parse(File.read(ARGV.fetch(0)))' "$report" >/dev/null; then
    echo "$variant coverage did not produce valid JSON: $report" >&2
    cat "$log" >&2
    exit 1
  fi
}

run_coverage base "$BASE_BINARY" "$BASE_REPORT" "$BASE_LOG" "$BASE_META" "$BASE_RBS_DIR"
run_coverage head "$HEAD_BINARY" "$HEAD_REPORT" "$HEAD_LOG" "$HEAD_META" "$HEAD_RBS_DIR"

comparison_args=(
  --base "$BASE_REPORT"
  --head "$HEAD_REPORT"
  --base-sha "$BASE_SHA"
  --head-sha "$HEAD_SHA"
  --output "$RESULT_JSON"
  --subject "$SUBJECT_PATH"
  --subject-ref "${SUBJECT_REF:-unknown}"
)
if [[ "$BASE_TIMED_OUT" -eq 1 ]]; then
  comparison_args+=(--base-timeout)
fi
if [[ "$ALLOW_REGRESSIONS" -eq 1 ]]; then
  comparison_args+=(--allow-regressions)
fi

ruby "$ROOT_DIR/scripts/compare_coverage.rb" "${comparison_args[@]}"
