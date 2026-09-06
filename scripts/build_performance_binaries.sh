#!/usr/bin/env bash
set -euo pipefail

# Build the base and head binaries once for the performance matrix.

if [[ $# -ne 1 ]]; then
  echo "Usage: scripts/build_performance_binaries.sh OUTPUT_DIR" >&2
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="$1"
if [[ "$OUTPUT_DIR" != /* ]]; then
  OUTPUT_DIR="$ROOT_DIR/$OUTPUT_DIR"
fi

RBS_DIR="${TYDA_RBS_DIR:-$ROOT_DIR/vendor/rbs}"
if [[ "$RBS_DIR" != /* ]]; then
  RBS_DIR="$ROOT_DIR/$RBS_DIR"
fi
BASE_REF="${TYDA_PERF_BASE_REF:-}"
TARGET_ROOT="${TYDA_PERF_TARGET_DIR:-$ROOT_DIR/target/performance-build}"
if [[ "$TARGET_ROOT" != /* ]]; then
  TARGET_ROOT="$ROOT_DIR/$TARGET_ROOT"
fi

if [[ ! -d "$RBS_DIR" ]]; then
  echo "vendor/rbs is missing: $RBS_DIR" >&2
  echo "run ./scripts/vendor-rbs.sh first" >&2
  exit 2
fi

mkdir -p "$OUTPUT_DIR" "$TARGET_ROOT"

if [[ -z "$BASE_REF" || "$BASE_REF" =~ ^0+$ ]]; then
  BASE_REF="$(git -C "$ROOT_DIR" rev-parse HEAD^)"
fi

if BASE_SHA="$(git -C "$ROOT_DIR" rev-parse --verify "${BASE_REF}^{commit}" 2>/dev/null)"; then
  :
else
  FETCH_REF="$BASE_REF"
  if [[ "$FETCH_REF" == origin/* ]]; then
    FETCH_REF="${FETCH_REF#origin/}"
  fi
  git -C "$ROOT_DIR" fetch --no-tags --depth=1 origin "$FETCH_REF" >/dev/null
  BASE_SHA="$(git -C "$ROOT_DIR" rev-parse 'FETCH_HEAD^{commit}')"
fi
HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse 'HEAD^{commit}')"

metadata_path="$OUTPUT_DIR/metadata.env"
if [[ -x "$OUTPUT_DIR/bin/base/tyda" && -x "$OUTPUT_DIR/bin/head/tyda" && -f "$metadata_path" ]]; then
  cached_base="$(sed -n 's/^base_sha=//p' "$metadata_path")"
  cached_head="$(sed -n 's/^head_sha=//p' "$metadata_path")"
  if [[ "$cached_base" == "$BASE_SHA" && "$cached_head" == "$HEAD_SHA" && -d "$OUTPUT_DIR/vendor/rbs" ]]; then
    echo "performance binaries already match base=$BASE_SHA head=$HEAD_SHA"
    exit 0
  fi
fi

rm -rf "${OUTPUT_DIR:?}/bin" "${OUTPUT_DIR:?}/vendor" "${metadata_path:?}"

RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/tyda-perf-build.XXXXXX")"
BASE_DIR="$RUN_DIR/base"
BASE_WORKTREE_ADDED=0

cleanup() {
  if [[ "$BASE_WORKTREE_ADDED" -eq 1 || -d "$BASE_DIR" ]]; then
    git -C "$ROOT_DIR" worktree remove --force "$BASE_DIR" >/dev/null 2>&1 || true
  fi
  rm -rf "$RUN_DIR"
}
trap cleanup EXIT

git -C "$ROOT_DIR" worktree add --detach "$BASE_DIR" "$BASE_SHA" >/dev/null
BASE_WORKTREE_ADDED=1
mkdir -p "$BASE_DIR/vendor"
ln -s "$RBS_DIR" "$BASE_DIR/vendor/rbs"

build_variant() {
  local variant="$1"
  local repo="$2"
  local target="$3"
  local build_log="$RUN_DIR/$variant-build.log"

  echo "Building $variant release binary..."
  if ! (cd "$repo" && CARGO_TARGET_DIR="$target" cargo build --locked --release --bin tyda >"$build_log" 2>&1); then
    cat "$build_log" >&2
    exit 1
  fi
  if [[ ! -x "$target/release/tyda" ]]; then
    echo "failed to find the $variant performance binary" >&2
    cat "$build_log" >&2
    exit 1
  fi
  mkdir -p "$OUTPUT_DIR/bin/$variant"
  cp "$target/release/tyda" "$OUTPUT_DIR/bin/$variant/tyda"
  chmod +x "$OUTPUT_DIR/bin/$variant/tyda"
}

export CARGO_INCREMENTAL=0
build_variant base "$BASE_DIR" "$TARGET_ROOT/base"
build_variant head "$ROOT_DIR" "$TARGET_ROOT/head"

mkdir -p "$OUTPUT_DIR/vendor/rbs"
cp -R "$RBS_DIR/." "$OUTPUT_DIR/vendor/rbs/"
printf 'base_sha=%s\nhead_sha=%s\n' "$BASE_SHA" "$HEAD_SHA" >"$metadata_path"

echo "performance binaries ready: base=$BASE_SHA head=$HEAD_SHA"
