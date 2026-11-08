#!/usr/bin/env bash
set -euo pipefail

# Compare one CLI output mode (rbs or diagnostics) for a base/head compatibility subject.
# Usage: compare_analysis_outputs.sh <rbs|diagnostics>

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-}"
SUBJECT_NAME="${TYDA_ANALYSIS_SUBJECT_NAME:-subject}"
SUBJECT_PATH="${TYDA_ANALYSIS_SUBJECT_PATH:-$ROOT_DIR/subject/sample}"
BINARY_DIR="${TYDA_ANALYSIS_BINARY_DIR:-$ROOT_DIR/target/analysis-runtime/bin}"
RBS_DIR="${TYDA_ANALYSIS_RBS_DIR:-$ROOT_DIR/target/analysis-runtime/rbs}"
OUTPUT_DIR="${TYDA_ANALYSIS_OUTPUT_DIR:-$ROOT_DIR/target/analysis/$SUBJECT_NAME}"
FAIL_ON_CHANGE="${TYDA_ANALYSIS_FAIL_ON_CHANGE:-1}"
TIMEOUT_SECONDS="${TYDA_ANALYSIS_TIMEOUT_SECONDS:-600}"
LOG_DIFF_LINES="${TYDA_ANALYSIS_LOG_DIFF_LINES:-5000}"

if [[ "$MODE" != rbs && "$MODE" != diagnostics ]]; then
  echo "usage: $0 <rbs|diagnostics>" >&2
  exit 2
fi

for variable in SUBJECT_PATH BINARY_DIR RBS_DIR OUTPUT_DIR; do
  value="${!variable}"
  if [[ "$value" != /* ]]; then
    value="$ROOT_DIR/$value"
    printf -v "$variable" '%s' "$value"
  fi
done

if [[ ! -d "$SUBJECT_PATH" ]]; then
  echo "analysis subject not found: $SUBJECT_PATH" >&2
  exit 2
fi
for variant in base head; do
  if [[ -f "$BINARY_DIR/$variant/tyda" ]]; then
    chmod +x "$BINARY_DIR/$variant/tyda"
  fi
  if [[ ! -x "$BINARY_DIR/$variant/tyda" ]]; then
    echo "analysis binary not found: $BINARY_DIR/$variant/tyda" >&2
    exit 2
  fi
  if [[ ! -d "$RBS_DIR/$variant" ]]; then
    echo "analysis RBS directory not found: $RBS_DIR/$variant" >&2
    exit 2
  fi
done
if ! [[ "$TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "TYDA_ANALYSIS_TIMEOUT_SECONDS must be a positive integer" >&2
  exit 2
fi
if ! [[ "$LOG_DIFF_LINES" =~ ^[0-9]+$ ]]; then
  echo "TYDA_ANALYSIS_LOG_DIFF_LINES must be a non-negative integer" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR"
rm -f "$OUTPUT_DIR"/*."$MODE".stdout "$OUTPUT_DIR"/*."$MODE".stderr \
  "$OUTPUT_DIR"/*."$MODE".meta "$OUTPUT_DIR"/*."$MODE".status \
  "$OUTPUT_DIR/$MODE.diff" "$OUTPUT_DIR/$MODE.result"

execution_failed=0

run_variant() {
  local variant="$1"
  local mode="$2"
  local binary="$BINARY_DIR/$variant/tyda"
  local rbs="$RBS_DIR/$variant"
  local stdout="$OUTPUT_DIR/$variant.$mode.stdout"
  local stderr="$OUTPUT_DIR/$variant.$mode.stderr"
  local meta="$OUTPUT_DIR/$variant.$mode.meta"
  local exit_code
  local -a args=("$binary")

  if [[ "$mode" == diagnostics ]]; then
    args+=(--diagnostics)
  fi
  args+=("$SUBJECT_PATH")

  set +e
  ruby "$ROOT_DIR/scripts/measure_process.rb" \
    --log "$stderr" \
    --output "$meta" \
    --stdout "$stdout" \
    --timeout "$TIMEOUT_SECONDS" \
    -- env TYDA_RBS_DIR="$rbs" nice -n 19 "${args[@]}"
  exit_code=$?
  set -e

  printf '%s\n' "$exit_code" > "$OUTPUT_DIR/$variant.$mode.status"
  if [[ "$exit_code" -ne 0 ]]; then
    echo "$variant $mode failed (status=$exit_code)" >&2
    cat "$stderr" >&2 || true
    execution_failed=1
  fi
}

run_variant base "$MODE"
run_variant head "$MODE"

if [[ "$execution_failed" -ne 0 ]]; then
  echo error > "$OUTPUT_DIR/$MODE.result"
  exit 1
fi

base_output="$OUTPUT_DIR/base.$MODE.stdout"
head_output="$OUTPUT_DIR/head.$MODE.stdout"
diff_file="$OUTPUT_DIR/$MODE.diff"
if cmp -s "$base_output" "$head_output"; then
  echo identical > "$OUTPUT_DIR/$MODE.result"
  echo "$SUBJECT_NAME $MODE output: identical"
  exit 0
fi

echo changed > "$OUTPUT_DIR/$MODE.result"
diff -u --label "base/$MODE" --label "head/$MODE" "$base_output" "$head_output" > "$diff_file" || true
diff_lines="$(wc -l < "$diff_file" | tr -d ' ')"
echo "$SUBJECT_NAME $MODE output: changed ($diff_lines diff lines)"
echo "::group::$MODE diff ($SUBJECT_NAME)"
head -n "$LOG_DIFF_LINES" "$diff_file"
if [[ "$diff_lines" -gt "$LOG_DIFF_LINES" ]]; then
  echo "... truncated after $LOG_DIFF_LINES of $diff_lines lines; the full diff is in the analysis-comparison-$SUBJECT_NAME artifact."
fi
echo "::endgroup::"

if [[ "$FAIL_ON_CHANGE" != 0 ]]; then
  echo "$MODE output changed." >&2
  exit 1
fi
echo "$MODE output change recorded; the analysis-compatibility gate decides whether it is approved."
