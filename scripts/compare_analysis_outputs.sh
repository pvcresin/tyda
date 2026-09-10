#!/usr/bin/env bash
set -euo pipefail

# Compare CLI output and diagnostics for base/head compatibility subjects.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBJECT_NAME="${TYDA_ANALYSIS_SUBJECT_NAME:-subject}"
SUBJECT_PATH="${TYDA_ANALYSIS_SUBJECT_PATH:-$ROOT_DIR/subject/sample}"
BINARY_DIR="${TYDA_ANALYSIS_BINARY_DIR:-$ROOT_DIR/target/analysis-runtime/bin}"
RBS_DIR="${TYDA_ANALYSIS_RBS_DIR:-$ROOT_DIR/target/analysis-runtime/rbs}"
OUTPUT_DIR="${TYDA_ANALYSIS_OUTPUT_DIR:-$ROOT_DIR/target/analysis/$SUBJECT_NAME}"
APPROVED="${TYDA_ANALYSIS_APPROVED:-0}"
TIMEOUT_SECONDS="${TYDA_ANALYSIS_TIMEOUT_SECONDS:-600}"

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

mkdir -p "$OUTPUT_DIR"
rm -f "$OUTPUT_DIR"/*.stdout "$OUTPUT_DIR"/*.stderr "$OUTPUT_DIR"/*.meta \
  "$OUTPUT_DIR"/*.status "$OUTPUT_DIR"/*.diff "$OUTPUT_DIR/summary.md"

execution_failed=0
differences_found=0

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

compare_outputs() {
  local mode="$1"
  local base="$OUTPUT_DIR/base.$mode.stdout"
  local head="$OUTPUT_DIR/head.$mode.stdout"
  local diff_file="$OUTPUT_DIR/$mode.diff"

  if cmp -s "$base" "$head"; then
    echo "$mode output: identical"
    rm -f "$diff_file"
  else
    echo "$mode output: changed"
    diff -u "$base" "$head" > "$diff_file" || true
    differences_found=1
  fi
}

run_variant base rbs
run_variant head rbs
run_variant base diagnostics
run_variant head diagnostics
compare_outputs rbs
compare_outputs diagnostics

summary="$OUTPUT_DIR/summary.md"
{
  echo "## Analysis compatibility: $SUBJECT_NAME"
  echo
  echo "- Subject: \`$SUBJECT_PATH\`"
  if [[ "$APPROVED" == 1 ]]; then
    echo "- Approval label: present"
  else
    echo "- Approval label: absent"
  fi
  echo
  for mode in rbs diagnostics; do
    if cmp -s "$OUTPUT_DIR/base.$mode.stdout" "$OUTPUT_DIR/head.$mode.stdout"; then
      echo "- $mode output: identical"
    else
      echo "- $mode output: changed"
      echo
      echo "<details><summary>$mode diff (first 200 lines)</summary>"
      echo
      echo '<pre>'
      ruby -rcgi -e 'puts CGI.escapeHTML(File.read(ARGV.fetch(0)).lines.first(200).join)' \
        "$OUTPUT_DIR/$mode.diff"
      echo '</pre>'
      echo
      echo '</details>'
    fi
  done
} > "$summary"

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  cat "$summary" >> "$GITHUB_STEP_SUMMARY"
fi
cat "$summary"

if [[ "$execution_failed" -ne 0 ]]; then
  exit 1
fi
if [[ "$differences_found" -ne 0 && "$APPROVED" != 1 ]]; then
  echo "Analysis output changed without approved-analysis-change." >&2
  exit 1
fi
if [[ "$differences_found" -ne 0 ]]; then
  echo "Analysis output change approved by maintainer/admin label."
fi
