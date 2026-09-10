#!/usr/bin/env bash
set -euo pipefail

# Build base and head binaries together with the RBS revision used by each one.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-$ROOT_DIR/target/analysis-runtime}"
TARGET_ROOT="${TYDA_COMPAT_TARGET_DIR:-$ROOT_DIR/target/analysis-build}"
BASE_SHA="${TYDA_COMPAT_BASE_SHA:-$(git -C "$ROOT_DIR" rev-parse HEAD^)}"
HEAD_SHA="${TYDA_COMPAT_HEAD_SHA:-$(git -C "$ROOT_DIR" rev-parse HEAD)}"

if [[ "$OUTPUT_DIR" != /* ]]; then
  OUTPUT_DIR="$ROOT_DIR/$OUTPUT_DIR"
fi
if [[ "$TARGET_ROOT" != /* ]]; then
  TARGET_ROOT="$ROOT_DIR/$TARGET_ROOT"
fi

if [[ -z "$BASE_SHA" || -z "$HEAD_SHA" ]]; then
  echo "base and head revisions are required" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR"
rm -rf "$OUTPUT_DIR/bin" "$OUTPUT_DIR/rbs" "$TARGET_ROOT/base" "$TARGET_ROOT/head"
mkdir -p "$OUTPUT_DIR/bin/base" "$OUTPUT_DIR/bin/head" \
  "$OUTPUT_DIR/rbs/base" "$OUTPUT_DIR/rbs/head" "$TARGET_ROOT"

WORKTREE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/tyda-compat.XXXXXX")"
BASE_DIR="$WORKTREE_ROOT/base"
cleanup() {
  git -C "$ROOT_DIR" worktree remove --force "$BASE_DIR" >/dev/null 2>&1 || true
  rmdir "$WORKTREE_ROOT" 2>/dev/null || true
}
trap cleanup EXIT

git -C "$ROOT_DIR" worktree add --detach "$BASE_DIR" "$BASE_SHA" >/dev/null

prepare_and_build() {
  local variant="$1"
  local repo_dir="$2"
  local revision="$3"
  local target_dir="$TARGET_ROOT/$variant"

  echo "Preparing $variant ($revision)..."
  (
    cd "$repo_dir"
    if command -v mise >/dev/null 2>&1 && [[ -f mise.toml ]]; then
      mise trust mise.toml >/dev/null 2>&1 || true
    fi
    ./scripts/vendor-rbs.sh --force
    CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$target_dir" cargo build --locked --release --bin tyda
  )

  cp "$target_dir/release/tyda" "$OUTPUT_DIR/bin/$variant/tyda"
  chmod +x "$OUTPUT_DIR/bin/$variant/tyda"
  cp -R "$repo_dir/vendor/rbs/." "$OUTPUT_DIR/rbs/$variant/"
}

prepare_and_build base "$BASE_DIR" "$BASE_SHA"
prepare_and_build head "$ROOT_DIR" "$HEAD_SHA"

base_rbs="$(sed -n 's/^gem "rbs", "= \([^" ]*\)".*/\1/p' "$BASE_DIR/Gemfile" | head -n 1)"
head_rbs="$(sed -n 's/^gem "rbs", "= \([^" ]*\)".*/\1/p' "$ROOT_DIR/Gemfile" | head -n 1)"
cat > "$OUTPUT_DIR/metadata" <<EOF
base_sha=$BASE_SHA
head_sha=$HEAD_SHA
base_rbs=$base_rbs
head_rbs=$head_rbs
EOF

echo "Compatibility binaries written to $OUTPUT_DIR"
