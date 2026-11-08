#!/usr/bin/env bash
set -euo pipefail

# Find the newest unexpired analysis-compatibility-runtime artifact that a Pull
# request CI run in this repository built for HEAD_SHA. Writes run_id and
# artifact_id to GITHUB_OUTPUT, or nothing when no artifact can be reused. The
# caller must still check the base and head revisions in the artifact metadata.

head_sha="${HEAD_SHA:?HEAD_SHA is required}"
repository="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
output="${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"

if [[ -n "${TYDA_PR_CI_RUNS_JSON:-}" ]]; then
  runs="$(cat "$TYDA_PR_CI_RUNS_JSON")"
else
  runs="$(gh api "repos/$repository/actions/workflows/pull-request-ci.yml/runs?head_sha=$head_sha&event=pull_request&per_page=100")"
fi
if [[ -n "${TYDA_RUNTIME_ARTIFACTS_JSON:-}" ]]; then
  artifacts="$(cat "$TYDA_RUNTIME_ARTIFACTS_JSON")"
else
  artifacts="$(gh api "repos/$repository/actions/artifacts?name=analysis-compatibility-runtime&per_page=100")"
fi

selected="$(jq -r --arg head "$head_sha" --argjson runs "$runs" '
  [$runs.workflow_runs[]
   | select(.head_sha == $head and .event == "pull_request"
            and .path == ".github/workflows/pull-request-ci.yml")
   | .id] as $run_ids
  | [.artifacts[]
     | select(.name == "analysis-compatibility-runtime" and (.expired | not)
              and .workflow_run.head_sha == $head
              and (.workflow_run.id as $id | $run_ids | index($id)))]
  | sort_by(.created_at) | last
  | if . == null then "" else "\(.workflow_run.id) \(.id)" end
' <<< "$artifacts")"

if [[ -z "$selected" ]]; then
  echo "No reusable analysis build from Pull request CI for $head_sha."
  exit 0
fi

read -r run_id artifact_id <<< "$selected"
echo "Found analysis build artifact $artifact_id from Pull request CI run $run_id."
{
  echo "run_id=$run_id"
  echo "artifact_id=$artifact_id"
} >> "$output"
