#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# vendor/rbs comes from the rbs gem (scripts/vendor-rbs.sh). Expand it first so
# direct runs / fresh checkouts can still build rbs-sys via cargo.
if [ ! -d "vendor/rbs/core" ]; then
  echo "=== Vendoring RBS (vendor/rbs missing) ==="
  ./scripts/vendor-rbs.sh
fi

echo "=== Formatting ==="
cargo fmt -- --check

echo "=== Clippy ==="
cargo clippy --all-targets -- -D warnings

# The playground (`mise run dev`) builds the core for wasm with a reduced feature
# set whose cfg differs from the default build. Check it so wasm-only warnings
# don't slip through. mimalloc is feature-gated off, so no wasi-sdk is needed.
echo "=== Clippy (wasm feature) ==="
cargo clippy --no-default-features --features wasm -- -D warnings

echo "=== Tests ==="
export TYDA_LSP_BENCH_ROOT="${TYDA_LSP_BENCH_ROOT:-subject/sample}"

# Run every test target even when an earlier one fails, then report all
# failures at once; a bare `cargo test` stops at the first failing target.
failed_targets=()

run_tests() {
  local name="$1"
  shift
  echo "--- Tests: ${name} ---"
  if ! cargo test "$@" -- --test-threads=1; then
    failed_targets+=("$name")
  fi
}

run_tests "unit (lib + bins)" --lib --bins
for test_file in tests/*.rs; do
  test_name="$(basename "$test_file" .rs)"
  run_tests "$test_name" --test "$test_name"
done
run_tests "doc" --doc

echo "--- Test summary ---"
if [ "${#failed_targets[@]}" -gt 0 ]; then
  for name in "${failed_targets[@]}"; do
    echo "FAILED: $name"
  done
  echo "${#failed_targets[@]} test target(s) failed."
  exit 1
fi
echo "All test targets passed."

echo "=== Release Build ==="
cargo build --release

echo ""
echo "All checks passed."
