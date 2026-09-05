#!/usr/bin/env bash
set -euo pipefail

# Fetch the public OSS benchmark subjects into subject/, pinned to the commits
# the baselines in docs/performance.md were measured against.
#
#   scripts/setup_subjects.sh              # all subjects
#   scripts/setup_subjects.sh rack rake    # only the named ones
#   scripts/setup_subjects.sh --list       # show the pinned table

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SUBJECT_DIR="${TYDA_SUBJECT_DIR:-$ROOT_DIR/subject}"

# name|remote|commit|sparse-checkout paths (empty = whole tree)
SUBJECTS=(
  "rack|https://github.com/rack/rack.git|ca8a404704ed043797c4f9d482c97d722c0dc719|"
  "rake|https://github.com/ruby/rake.git|353f51da83616397b50b01ccc5c39607811ad691|"
  "rubygems|https://github.com/ruby/rubygems.git|f72d9d9f9e42a246e5301f8f6492e8258134baee|"
  "optcarrot|https://github.com/mame/optcarrot.git|c215378a27b2dce8d8e5d98a3ed75e0354c5a840|"
  "mastodon|https://github.com/mastodon/mastodon.git|eb848d082afc8864b2aa15858f414e4867902c65|"
  "redmine|https://github.com/redmine/redmine.git|890812e49cc60e96c7c252b7dedbd881a4edba55|/app/ /config/ /db/ /lib/ /test/ /Gemfile /Gemfile.lock /.ruby-version /.rubocop.yml"
  "gitlab|https://github.com/gitlabhq/gitlabhq.git|088eaeded42c93b4cbb0389b567e2f48d5b08b7c|/ /app/ /config/ /db/ /lib/ /gems/ /Gemfile /Gemfile.lock /.ruby-version /.rubocop.yml"
)

list_subjects() {
  printf "%-10s %-46s %s\n" "subject" "commit" "remote"
  local entry name remote commit
  for entry in "${SUBJECTS[@]}"; do
    IFS='|' read -r name remote commit _ <<<"$entry"
    printf "%-10s %-46s %s\n" "$name" "$commit" "$remote"
  done
}

if [[ "${1:-}" == "--list" ]]; then
  list_subjects
  exit 0
fi

wanted=("$@")

setup_one() {
  local name="$1" remote="$2" commit="$3" sparse="$4"
  local dir="$SUBJECT_DIR/$name"

  if [[ -d "$dir/.git" ]] && [[ "$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)" == "$commit" ]]; then
    echo "ok   $name (already at ${commit:0:9})"
    return 0
  fi

  echo "sync $name -> ${commit:0:9}"
  mkdir -p "$dir"
  if [[ ! -d "$dir/.git" ]]; then
    git -C "$dir" init --quiet
    git -C "$dir" remote add origin "$remote"
  else
    git -C "$dir" remote set-url origin "$remote"
  fi

  # Sparse paths must be set before checkout so a huge tree never materializes.
  if [[ -n "$sparse" ]]; then
    # shellcheck disable=SC2086
    git -C "$dir" sparse-checkout set --no-cone $sparse
  fi

  # Blob filtering keeps the fetch small; not every host supports it.
  git -C "$dir" fetch --quiet --depth 1 --filter=blob:none origin "$commit" \
    || git -C "$dir" fetch --quiet --depth 1 origin "$commit" \
    || git -C "$dir" fetch --quiet origin "$commit"
  git -C "$dir" checkout --quiet --detach FETCH_HEAD

  echo "     $(du -sh "$dir" | cut -f1) at $(git -C "$dir" rev-parse --short HEAD)"
}

matched=0
for entry in "${SUBJECTS[@]}"; do
  IFS='|' read -r name remote commit sparse <<<"$entry"
  if [[ ${#wanted[@]} -gt 0 ]]; then
    skip=1
    for w in "${wanted[@]}"; do
      [[ "$w" == "$name" ]] && skip=0
    done
    [[ "$skip" -eq 1 ]] && continue
  fi
  matched=$((matched + 1))
  setup_one "$name" "$remote" "$commit" "$sparse"
done

if [[ ${#wanted[@]} -gt 0 && "$matched" -eq 0 ]]; then
  echo "no subject matched: ${wanted[*]}" >&2
  list_subjects >&2
  exit 2
fi

echo ""
echo "subjects ready under $SUBJECT_DIR (subject/sample ships with the repo)."
