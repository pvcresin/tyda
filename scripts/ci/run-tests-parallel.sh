#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

export TYDA_LSP_BENCH_ROOT="${TYDA_LSP_BENCH_ROOT:-subject/sample}"

shard="${1:-all}"
integration_tests=()
target_args=()

case "$shard" in
all)
  ;;
unit)
  target_args=(--lib --bins)
  ;;
integration)
  for test_file in tests/*.rs; do
    test_name="$(basename "$test_file" .rs)"
    case "$test_name" in
    mutation_robustness|pathological_inputs)
      continue
      ;;
    esac
    integration_tests+=("$test_name")
    target_args+=(--test "$test_name")
  done
  ;;
mutation)
  integration_tests=(mutation_robustness)
  target_args=(--test mutation_robustness)
  ;;
pathological)
  integration_tests=(pathological_inputs)
  target_args=(--test pathological_inputs)
  ;;
doc)
  target_args=(--doc)
  ;;
*)
  echo "Unknown test shard: $shard" >&2
  exit 2
  ;;
esac

# Compile the selected test targets before starting parallel runs. Each target
# below is still executed; only independent test processes share the runner.
echo "=== Compile test targets ==="
if [ "$shard" = "doc" ]; then
  echo "Doc tests are compiled by the test command."
elif [ "${#target_args[@]}" -eq 0 ]; then
  cargo test --locked --no-run
else
  cargo test --locked --no-run "${target_args[@]}"
fi

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

case "$shard" in
all)
  run_test "unit (lib + bins)" cargo test --locked --lib --bins
  for test_file in tests/*.rs; do
    test_name="$(basename "$test_file" .rs)"
    case "$test_name" in
    mutation_robustness|pathological_inputs)
      continue
      ;;
    esac
    run_test "$test_name" cargo test --locked --test "$test_name"
  done
  run_test "doc" cargo test --locked --doc
  ;;
unit)
  run_test "unit (lib + bins)" cargo test --locked --lib --bins
  ;;
integration|mutation|pathological)
  for test_name in "${integration_tests[@]}"; do
    run_test "$test_name" cargo test --locked --test "$test_name"
  done
  ;;
doc)
  run_test "doc" cargo test --locked --doc
  ;;
esac

failed_targets=()
wait_for_tests() {
  for index in "${!pids[@]}"; do
    if wait "${pids[$index]}"; then
      echo "--- Tests: ${names[$index]} passed ---"
    else
      echo "--- Tests: ${names[$index]} failed ---"
      failed_targets+=("${names[$index]}")
    fi
  done
  pids=()
  names=()
}

# The default local mode also avoids running the two CPU-heavy targets
# together. CI uses separate Windows shards, while this keeps the helper safe
# on a constrained developer machine.
wait_for_tests
if [ "$shard" = "all" ]; then
  for test_name in mutation_robustness pathological_inputs; do
    run_test "$test_name" cargo test --locked --test "$test_name"
    wait_for_tests
  done
fi

echo "--- Test summary ---"
if [ "${#failed_targets[@]}" -gt 0 ]; then
  for name in "${failed_targets[@]}"; do
    echo "FAILED: $name"
  done
  echo "${#failed_targets[@]} test target(s) failed."
  exit 1
fi
echo "All test targets passed."
