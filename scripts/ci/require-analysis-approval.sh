#!/usr/bin/env bash
set -euo pipefail

# Succeed only when the newest approved-analysis-change run of analysis-approval.yml
# for HEAD_SHA finished successfully. An unlabeled run is newer than the labeled
# run it revokes, so removing the label also removes the approval.

head_sha="${HEAD_SHA:?HEAD_SHA is required}"
repository="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

if [[ -n "${TYDA_APPROVAL_RUNS_JSON:-}" ]]; then
  runs="$(cat "$TYDA_APPROVAL_RUNS_JSON")"
else
  runs="$(gh api "repos/$repository/actions/workflows/analysis-approval.yml/runs?head_sha=$head_sha&per_page=100")"
fi

latest="$(jq -r '
  [.workflow_runs[] | select(.display_title | startswith("Analysis approval ("))]
  | sort_by(.created_at) | last
  | if . == null then "none" else "\(.status) \(.conclusion) \(.display_title)" end
' <<< "$runs")"

if [[ "$latest" == "completed success Analysis approval (labeled)" ]]; then
  echo "Analysis changes for $head_sha were approved by the latest Analysis approval run."
  exit 0
fi

echo "Analysis checks failed, and the latest Analysis approval run for $head_sha is not an approval ($latest)." >&2
exit 1
