#!/usr/bin/env bash
set -euo pipefail

run_full_ci=true
run_pages=true
run_analysis=false

classify_paths() {
  local changed_paths="$1"

  run_full_ci=false
  run_pages=false
  while IFS= read -r path; do
    if [[ "$path" == docs/* ]]; then
      run_pages=true
    elif [[ "$path" != *.md ]]; then
      run_pages=true
      if [[ "$path" != playground/* ]]; then
        run_full_ci=true
      fi
    fi
  done <<< "$changed_paths"
}

if [[ "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
  base_sha="${BASE_SHA:?BASE_SHA is required for pull requests}"
  head_sha="${HEAD_SHA:?HEAD_SHA is required for pull requests}"
  changed_paths="$(git diff --name-only "${base_sha}...${head_sha}")"

  if [[ "${GITHUB_EVENT_ACTION:-}" == labeled || "${GITHUB_EVENT_ACTION:-}" == unlabeled ]]; then
    run_full_ci=false
    run_pages=false
    run_analysis=true
  elif [[ -n "$changed_paths" ]]; then
    classify_paths "$changed_paths"
    run_analysis="$run_full_ci"
  else
    run_analysis=true
  fi
elif [[ "${GITHUB_EVENT_NAME:-}" == push && "${GITHUB_REF_TYPE:-}" == branch ]]; then
  base_sha="${BASE_SHA:-}"
  head_sha="${HEAD_SHA:-}"

  # A missing or zero push base can occur on a new branch. Keep the safe
  # defaults in that case instead of attempting a partial classification.
  if [[ -n "$base_sha" && -n "$head_sha" && ! "$base_sha" =~ ^0+$ ]]; then
    changed_paths="$(git diff --name-only "${base_sha}" "${head_sha}")"
    if [[ -n "$changed_paths" ]]; then
      classify_paths "$changed_paths"
    fi
  fi
elif [[ "${GITHUB_EVENT_NAME:-}" == workflow_dispatch ]]; then
  # A manual analysis run should exercise the full compatibility matrix.
  run_analysis=true
fi

{
  echo "run_full_ci=$run_full_ci"
  echo "run_pages=$run_pages"
  echo "run_analysis=$run_analysis"
} >> "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"
