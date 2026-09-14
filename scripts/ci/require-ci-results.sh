#!/usr/bin/env bash
set -euo pipefail

changes_result="${CHANGES_RESULT:?CHANGES_RESULT is required}"
run_required="${RUN_REQUIRED-}"
required_results="${REQUIRED_RESULTS-}"

if [[ "$changes_result" != success ]]; then
  echo "Change classification failed: $changes_result" >&2
  exit 1
fi

if [[ -z "$required_results" ]]; then
  echo "No required job results were provided." >&2
  exit 1
fi

checked=0
failed=0
while IFS= read -r entry; do
  [[ -n "$entry" ]] || continue

  if [[ "$entry" != *=* ]]; then
    echo "Malformed job result: $entry" >&2
    failed=1
    continue
  fi

  job_name="${entry%%=*}"
  result="${entry#*=}"
  if [[ -z "$job_name" || -z "$result" ]]; then
    echo "Malformed job result: $entry" >&2
    failed=1
    continue
  fi
  checked=$((checked + 1))
  case "$run_required:$result" in
    true:success)
      ;;
    false:skipped)
      ;;
    true:*|false:*)
      echo "$job_name: $result" >&2
      failed=1
      ;;
    *)
      echo "Unexpected required-check flag: $run_required" >&2
      failed=1
      ;;
  esac
done <<< "$required_results"

if [[ "$checked" -eq 0 ]]; then
  echo "No valid required job results were provided." >&2
  exit 1
fi

if [[ "$failed" -ne 0 ]]; then
  echo "Required CI jobs did not reach the expected result." >&2
  exit 1
fi

if [[ "$run_required" == true ]]; then
  echo "All required jobs succeeded."
else
  echo "Checks are not required and all dependent jobs were skipped."
fi
