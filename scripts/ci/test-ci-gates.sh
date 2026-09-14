#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
classifier="$repo_root/scripts/ci/classify-changed-paths.sh"
gate="$repo_root/scripts/ci/require-ci-results.sh"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

expect_success() {
  local name="$1"
  shift
  if ! env "$@" bash "$gate" >/dev/null 2>&1; then
    fail "$name should succeed"
  fi
}

expect_failure() {
  local name="$1"
  shift
  if env "$@" bash "$gate" >/dev/null 2>&1; then
    fail "$name should fail"
  fi
}

assert_output() {
  local output="$1"
  local key="$2"
  local expected="$3"

  grep -qx "$key=$expected" "$output" || {
    echo "Unexpected $key (expected $expected):" >&2
    cat "$output" >&2
    exit 1
  }
}

new_fixture() {
  local name="$1"
  local fixture="$tmp_dir/$name"

  mkdir -p "$fixture"
  git -c init.templateDir= -C "$fixture" init -q
  git -C "$fixture" config user.name CI
  git -C "$fixture" config user.email ci@example.invalid
  git -C "$fixture" commit --allow-empty -qm base
  printf '%s\n' "$fixture"
}

commit_paths() {
  local fixture="$1"
  shift

  local path
  for path in "$@"; do
    mkdir -p "$fixture/$(dirname "$path")"
    printf 'fixture\n' > "$fixture/$path"
  done
  git -C "$fixture" add --all
  if (($# == 0)); then
    git -C "$fixture" commit --allow-empty -qm head
  else
    git -C "$fixture" commit -qm head
  fi
}

assert_push_classification() {
  local name="$1"
  local expected_full="$2"
  local expected_pages="$3"
  shift 3

  local fixture output base head
  fixture="$(new_fixture "$name")"
  base="$(git -C "$fixture" rev-parse HEAD)"
  commit_paths "$fixture" "$@"
  head="$(git -C "$fixture" rev-parse HEAD)"
  output="$fixture/output"

  (
    cd "$fixture"
    GITHUB_EVENT_NAME=push \
      GITHUB_REF_TYPE=branch \
      BASE_SHA="$base" \
      HEAD_SHA="$head" \
      GITHUB_OUTPUT="$output" \
      bash "$classifier"
  )
  assert_output "$output" run_full_ci "$expected_full"
  assert_output "$output" run_pages "$expected_pages"
  assert_output "$output" run_analysis false
}

assert_pr_classification() {
  local name="$1"
  local action="$2"
  local expected_full="$3"
  local expected_pages="$4"
  local expected_analysis="$5"
  shift 5

  local fixture output base head
  fixture="$(new_fixture "$name")"
  base="$(git -C "$fixture" rev-parse HEAD)"
  commit_paths "$fixture" "$@"
  head="$(git -C "$fixture" rev-parse HEAD)"
  output="$fixture/output"

  (
    cd "$fixture"
    GITHUB_EVENT_NAME=pull_request \
      GITHUB_EVENT_ACTION="$action" \
      BASE_SHA="$base" \
      HEAD_SHA="$head" \
      GITHUB_OUTPUT="$output" \
      bash "$classifier"
  )
  assert_output "$output" run_full_ci "$expected_full"
  assert_output "$output" run_pages "$expected_pages"
  assert_output "$output" run_analysis "$expected_analysis"
}

assert_push_classification readme false false README.md
assert_push_classification docs false true docs/index.md
assert_push_classification playground false true playground/src/main.ts
assert_push_classification source true true src/lib.rs
assert_push_classification dependency-bundler true true Gemfile.lock
assert_push_classification dependency-cargo true true Cargo.lock
assert_push_classification dependency-npm true true package-lock.json
assert_push_classification dependency-vscode true true vscode/package-lock.json
assert_push_classification mixed true true README.md src/lib.rs

assert_pr_classification pr-readme opened false false false README.md
assert_pr_classification pr-docs opened false true false docs/index.md
assert_pr_classification pr-playground opened false true false playground/src/main.ts
assert_pr_classification pr-source opened true true true src/lib.rs
assert_pr_classification pr-analysis-label labeled false false true src/lib.rs
assert_pr_classification pr-empty opened true true true

fixture="$(new_fixture workflow-dispatch)"
output="$fixture/output"
(
  cd "$fixture"
  GITHUB_EVENT_NAME=workflow_dispatch GITHUB_OUTPUT="$output" bash "$classifier"
)
assert_output "$output" run_full_ci true
assert_output "$output" run_pages true
assert_output "$output" run_analysis true

fixture="$(new_fixture tag-push)"
output="$fixture/output"
(
  cd "$fixture"
  GITHUB_EVENT_NAME=push GITHUB_REF_TYPE=tag GITHUB_OUTPUT="$output" bash "$classifier"
)
assert_output "$output" run_full_ci true
assert_output "$output" run_pages true
assert_output "$output" run_analysis false

fixture="$(new_fixture initial-push)"
output="$fixture/output"
(
  cd "$fixture"
  GITHUB_EVENT_NAME=push \
    GITHUB_REF_TYPE=branch \
    BASE_SHA=0000000000000000000000000000000000000000 \
    HEAD_SHA="$(git rev-parse HEAD)" \
    GITHUB_OUTPUT="$output" \
    bash "$classifier"
)
assert_output "$output" run_full_ci true
assert_output "$output" run_pages true
assert_output "$output" run_analysis false

expect_success \
  "not-required gate" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=false \
  REQUIRED_RESULTS="unit=skipped"
expect_failure \
  "unexpected job on not-required scope" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=false \
  REQUIRED_RESULTS="unit=success"
expect_failure \
  "failed job on not-required scope" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=false \
  REQUIRED_RESULTS="unit=failure"
expect_success \
  "successful required gate" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=true \
  REQUIRED_RESULTS=$'unit=success\nintegration=success'
expect_failure \
  "skipped required job" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=true \
  REQUIRED_RESULTS=$'unit=success\nintegration=skipped'
expect_failure \
  "failed required job" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=true \
  REQUIRED_RESULTS=$'unit=success\nintegration=failure'
expect_failure \
  "failed classification" \
  CHANGES_RESULT=failure \
  RUN_REQUIRED=false \
  REQUIRED_RESULTS="unit=skipped"
expect_failure \
  "unknown required flag" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=unknown \
  REQUIRED_RESULTS="unit=success"
expect_failure \
  "missing required jobs" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=true
expect_failure \
  "malformed required job" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=true \
  REQUIRED_RESULTS=$'unit=success\nmalformed'
expect_failure \
  "empty required jobs" \
  CHANGES_RESULT=success \
  RUN_REQUIRED=true \
  REQUIRED_RESULTS=$'\n'

echo "CI helper tests passed."
