#!/usr/bin/env bash
set -euo pipefail

run_full_ci=true
run_pages=true
run_analysis=false

if [[ "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
  base_sha="${BASE_SHA:?BASE_SHA is required for pull requests}"
  head_sha="${HEAD_SHA:?HEAD_SHA is required for pull requests}"
  changed_paths="$(git diff --name-only "${base_sha}...${head_sha}")"

  if [[ "${GITHUB_EVENT_ACTION:-}" == labeled || "${GITHUB_EVENT_ACTION:-}" == unlabeled ]]; then
    run_full_ci=false
    run_pages=false
    run_analysis=true
  elif [[ -n "$changed_paths" ]]; then
    run_full_ci=false
    run_pages=false
    while IFS= read -r path; do
      if [[ "$path" != *.md ]]; then
        run_pages=true
        if [[ "$path" != playground/* ]]; then
          run_full_ci=true
        fi
      fi
    done <<< "$changed_paths"
    run_analysis="$run_full_ci"
  else
    run_analysis=true
  fi
fi

{
  echo "run_full_ci=$run_full_ci"
  echo "run_pages=$run_pages"
  echo "run_analysis=$run_analysis"
} >> "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"
