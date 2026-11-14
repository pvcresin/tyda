---
name: record-known-issue
description: Record wrong or missing inference you find while working (even outside the task) as a known-issue test. Use whenever you notice Tyda output that does not match Ruby semantics / RBS, instead of only mentioning it to the user.
---

# Record a known issue

When you find code that Tyda handles wrongly and you do not fix it in the current
change, pin it as a known-issue test in the same branch. Do not leave it only as a
note in chat or the PR body.

## Steps

1. Reduce it to a minimal Ruby example and confirm the wrong output.
2. Check where it reproduces. The scenario runner, playground, and LSP use the
   full display path; the CLI uses the compact path and can differ.
   - Scenario: `TYDA_SCENARIO_FILTER=<path> cargo test -q --test scenario_runner -- --test-threads=1`
   - CLI: `target/release-fast/tyda <file.rb>`
   - Hover / playground: `analysis::hover_at` or `analysis::playground_analyze`
3. Put it in the place that matches where it reproduces:

| Reproduces in | Where | Form |
|---|---|---|
| RBS output (scenario path) | `tests/scenarios/known-issues/<topic>.md` | Scenario with the correct expected RBS (see `add-scenario-test`) |
| CLI only | `tests/cli.rs` | `known_issue_*` test that asserts the current wrong output, with a `// Known issue:` comment and a "drop this check" message. Add the correct case to a normal scenario, and add the item to `docs/roadmap.md` (CLI / scenario alignment) |
| Hover / LSP only | `tests/hover_probe.rs` | `known_issue_*` test that asserts the current wrong display and says what is expected |
| Diagnostics | `tests/flow_diagnostics.rs` or the matching diagnostics test | `known_issue_*` test as above |

4. Base the expected value on Ruby semantics and official RBS, not on the current output.
5. Run the new test and make sure it passes as an open known issue.
6. Tell the user briefly what you recorded and where.

When a later change fixes it, the known-issue test fails on purpose: promote the
scenario (remove it from `known-issues/` or drop `known_issue: true`) or delete the
`known_issue_*` check and add a normal assertion.
