#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

export TYDA_LSP_BENCH_ROOT="${TYDA_LSP_BENCH_ROOT:-subject/sample}"

# Compile all test targets before starting parallel runs. Each target below is
# still executed; only independent test processes share the runner.
echo "=== Compile test targets ==="
cargo test --locked --no-run

pids=()
names=()

run_test() {
  local name="$1"
  shift
  echo "--- Tests: ${name} (started) ---"
  "$@" &
  pids+=("$!")
  names+=("$name")
}

run_test "unit (lib + bins)" cargo test --locked --lib --bins
for test_file in tests/*.rs; do
  test_name="$(basename "$test_file" .rs)"
  run_test "$test_name" cargo test --locked --test "$test_name"
done
run_test "doc" cargo test --locked --doc

failed_targets=()
for index in "${!pids[@]}"; do
  if wait "${pids[$index]}"; then
    echo "--- Tests: ${names[$index]} passed ---"
  else
    echo "--- Tests: ${names[$index]} failed ---"
    failed_targets+=("${names[$index]}")
  fi
done

echo "--- Test summary ---"
if [ "${#failed_targets[@]}" -gt 0 ]; then
  for name in "${failed_targets[@]}"; do
    echo "FAILED: $name"
  done
  echo "${#failed_targets[@]} test target(s) failed."
  exit 1
fi
echo "All test targets passed."
