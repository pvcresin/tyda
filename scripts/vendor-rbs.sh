#!/usr/bin/env bash
# Regenerate vendor/rbs from the rbs gem.
#
# Expands the Gemfile-pinned rbs gem's stdlib type defs into vendor/rbs/:
#   - core/, stdlib/  : stdlib RBS (foundation for inference / checking)
#   - config.yml      : default rbs collection config
#   - BSDL, COPYING   : license texts (BSD-2-Clause / Ruby; keep when distributing)
#
# The C parser is vendored/compiled by ruby-rbs-sys (via crates/rbs-sys);
# this script does not expand C sources (src/include).
#
# vendor/rbs/ is generated (.gitignore'd). Versions come from Gemfile.lock;
# Dependabot (bundler) bumps it. Ruby + bundler are **build-time only**;
# the shipped CLI / wasm have no Ruby runtime dependency.
#
# No-op when the same version is already vendored (cheap as a task dependency).
# Force refresh with `scripts/vendor-rbs.sh --force`.
set -euo pipefail

cd "$(dirname "$0")/.."

# Install gems if missing (CI / first-time setup).
if ! bundle check >/dev/null 2>&1; then
  bundle install
fi

GEM_DIR="$(bundle show rbs)"
DEST="vendor/rbs"
VERSION="$(basename "$GEM_DIR")"
MARKER="$DEST/.vendored-version"

if [ "${1:-}" != "--force" ] && [ -f "$MARKER" ] && [ "$(cat "$MARKER")" = "$VERSION" ]; then
  echo "vendor/rbs already at $VERSION (use --force to refresh)"
  exit 0
fi

rm -rf "$DEST"
mkdir -p "$DEST"
for item in core stdlib config.yml BSDL COPYING README.md CHANGELOG.md; do
  if [ -e "$GEM_DIR/$item" ]; then
    cp -R "$GEM_DIR/$item" "$DEST/"
  fi
done
echo "$VERSION" >"$MARKER"

echo "vendored $VERSION (core/stdlib) -> $DEST"
