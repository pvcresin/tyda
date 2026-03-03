#!/usr/bin/env bash
set -euo pipefail

# CLI output characterization snapshots.
#
# Tool to assert byte-identical `tyda <subject>` RBS output before/after backend
# unification (moving the CLI render path onto WorkspaceState shared APIs).
#
#   scripts/snapshot_outputs.sh            # save snapshots
#   scripts/snapshot_outputs.sh --verify   # diff current output vs saved
#
# Output: /tmp/tyda_snapshots/<subject>.rbs (override with TYDA_SNAPSHOT_DIR)
#
# gitlab is ~8s / ~1GB per run, so use `nice -n 19`, run each subject once, and
# never parallelize subjects. Small subjects (sample/rack/rake/rubygems/mastodon)
# are cheap.
#
# An extra local project can be rendered by setting TYDA_EXTRA_SUBJECT to its
# root; those runs are environment-specific, so no diagnostics bench here.

cd "$(dirname "$0")/.."

BIN="target/release/tyda"
SNAPSHOT_DIR="${TYDA_SNAPSHOT_DIR:-/tmp/tyda_snapshots}"

# Normal subjects: compare render stdout only. gitlab is heavy → nice.
SUBJECTS=(sample rack rake rubygems mastodon gitlab)
EXTRA_SUBJECT_PATH="${TYDA_EXTRA_SUBJECT:-}"

MODE="save"
if [[ "${1:-}" == "--verify" ]]; then
  MODE="verify"
elif [[ $# -gt 0 ]]; then
  echo "Usage: scripts/snapshot_outputs.sh [--verify]" >&2
  exit 2
fi

if [[ ! -x "$BIN" ]]; then
  echo "release binary not found ($BIN). Run: cargo build --release" >&2
  exit 1
fi

mkdir -p "$SNAPSHOT_DIR"

failures=0
checked=0

# Nice heavy subjects so they do not monopolize CPU/memory. Arg1 is heavy/light;
# remaining args are paths passed to tyda.
run_render() {
  local heavy="$1"
  shift
  if [[ "$heavy" == "heavy" ]]; then
    nice -n 19 "$BIN" "$@" 2>/dev/null
  else
    "$BIN" "$@" 2>/dev/null
  fi
}

# process_one <name> <heavy|light> <path...>
process_one() {
  local name="$1"
  local heavy="$2"
  shift 2
  local paths=("$@")
  local snap="$SNAPSHOT_DIR/$name.rbs"

  local existing=()
  local p
  for p in "${paths[@]}"; do
    if [[ -e "$p" ]]; then
      existing+=("$p")
    fi
  done
  if [[ "${#existing[@]}" -eq 0 ]]; then
    echo "skip $name (no path found: ${paths[*]})"
    return 0
  fi

  if [[ "$MODE" == "save" ]]; then
    echo "save $name ..."
    run_render "$heavy" "${existing[@]}" > "$snap"
    local lines
    lines=$(wc -l < "$snap" | tr -d ' ')
    echo "  -> $snap ($lines lines)"
  else
    if [[ ! -f "$snap" ]]; then
      echo "MISSING baseline for $name ($snap) — run save mode first" >&2
      failures=$((failures + 1))
      return 0
    fi
    echo "verify $name ..."
    local current="$SNAPSHOT_DIR/$name.current.rbs"
    run_render "$heavy" "${existing[@]}" > "$current"
    if diff -q "$snap" "$current" >/dev/null; then
      echo "  OK (diff zero)"
      rm -f "$current"
    else
      echo "  DIFF DETECTED for $name:" >&2
      diff "$snap" "$current" | head -40 >&2
      echo "  (full current output kept at $current)" >&2
      failures=$((failures + 1))
    fi
    checked=$((checked + 1))
  fi
}

for name in "${SUBJECTS[@]}"; do
  heavy="light"
  if [[ "$name" == "gitlab" ]]; then
    heavy="heavy"
  fi
  process_one "$name" "$heavy" "subject/$name"
done

# Extra local project: render-only opt-in, treated as heavy. A full tree is too
# costly for a big app, so scope it to app/lib/config like the perf baselines.
if [[ -n "$EXTRA_SUBJECT_PATH" ]]; then
  process_one "extra" "heavy" "$EXTRA_SUBJECT_PATH/app" "$EXTRA_SUBJECT_PATH/lib" "$EXTRA_SUBJECT_PATH/config"
fi

if [[ "$MODE" == "verify" ]]; then
  echo ""
  if [[ "$failures" -eq 0 ]]; then
    echo "All snapshots match ($checked subjects, diff zero)."
  else
    echo "$failures subject(s) diverged." >&2
    exit 1
  fi
fi
