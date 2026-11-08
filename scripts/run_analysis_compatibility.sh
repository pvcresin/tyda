#!/usr/bin/env bash
set -euo pipefail

# Run the RBS output, diagnostics, and coverage checks for one compatibility subject.
# Every check runs even when an earlier one fails, so one run reports all changes.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBJECT_NAME="${TYDA_ANALYSIS_SUBJECT_NAME:-subject}"
SUBJECT_PATH="${TYDA_ANALYSIS_SUBJECT_PATH:-$ROOT_DIR/subject/sample}"
OUTPUT_DIR="${TYDA_ANALYSIS_OUTPUT_DIR:-$ROOT_DIR/target/analysis/$SUBJECT_NAME}"

if [[ "$SUBJECT_PATH" != /* ]]; then
  SUBJECT_PATH="$ROOT_DIR/$SUBJECT_PATH"
fi
if [[ "$OUTPUT_DIR" != /* ]]; then
  OUTPUT_DIR="$ROOT_DIR/$OUTPUT_DIR"
fi

mkdir -p "$OUTPUT_DIR"

set +e
statuses=()
for mode in rbs diagnostics; do
  TYDA_ANALYSIS_OUTPUT_DIR="$OUTPUT_DIR/output" \
    TYDA_ANALYSIS_SUBJECT_NAME="$SUBJECT_NAME" \
    TYDA_ANALYSIS_SUBJECT_PATH="$SUBJECT_PATH" \
    "$ROOT_DIR/scripts/compare_analysis_outputs.sh" "$mode"
  statuses+=("$mode=$?")
done

TYDA_COVERAGE_BINARY_DIR="${TYDA_ANALYSIS_BINARY_DIR:-$ROOT_DIR/target/analysis-runtime/bin}" \
  TYDA_COVERAGE_BASE_RBS_DIR="${TYDA_ANALYSIS_BASE_RBS_DIR:-$ROOT_DIR/target/analysis-runtime/rbs/base}" \
  TYDA_COVERAGE_HEAD_RBS_DIR="${TYDA_ANALYSIS_HEAD_RBS_DIR:-$ROOT_DIR/target/analysis-runtime/rbs/head}" \
  TYDA_COVERAGE_OUTPUT_DIR="$OUTPUT_DIR/coverage" \
  TYDA_COVERAGE_SUBJECT="$SUBJECT_PATH" \
  "$ROOT_DIR/scripts/coverage_ci.sh"
statuses+=("coverage=$?")
set -e

ruby "$ROOT_DIR/scripts/summarize_analysis.rb" subject "$OUTPUT_DIR" "$SUBJECT_NAME"

if [[ " ${statuses[*]} " =~ =[1-9] ]]; then
  echo "Analysis compatibility failed (${statuses[*]})." >&2
  exit 1
fi

echo "Analysis compatibility passed for $SUBJECT_NAME."
