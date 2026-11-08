#!/usr/bin/env bash
set -euo pipefail

# Write analysis_approved=1 only for a run started by adding approved-analysis-change,
# when the sender is a maintainer or admin and the label is still on the PR. Other
# events never approve: on a push the sender is the pusher, and the label from an
# older commit is being removed.

event_action="${EVENT_ACTION:?EVENT_ACTION is required}"
output="${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"
label=approved-analysis-change

approved=0
if [[ "$event_action" == labeled ]]; then
  sender="${SENDER_LOGIN:?SENDER_LOGIN is required}"
  repository="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
  pr_number="${PR_NUMBER:?PR_NUMBER is required}"
  if [[ -n "${TYDA_SENDER_PERMISSION+x}" ]]; then
    permission="$TYDA_SENDER_PERMISSION"
  else
    permission="$(gh api "repos/$repository/collaborators/$sender/permission" \
      --jq .permission 2>/dev/null || true)"
  fi
  if [[ -n "${TYDA_PR_LABELS_JSON:-}" ]]; then
    labels="$(cat "$TYDA_PR_LABELS_JSON")"
  else
    labels="$(gh api "repos/$repository/issues/$pr_number/labels?per_page=100" 2>/dev/null || echo '[]')"
  fi
  has_label="$(jq -r --arg label "$label" 'any(.[]; .name == $label)' <<< "$labels")"

  if [[ "$permission" != admin && "$permission" != maintain ]]; then
    echo "Ignoring $label from $sender (permission=$permission)."
  elif [[ "$has_label" != true ]]; then
    echo "Ignoring $label from $sender because the PR no longer has the label."
  else
    echo "Analysis changes approved by $sender ($permission)."
    approved=1
  fi
fi
echo "analysis_approved=$approved" >> "$output"
