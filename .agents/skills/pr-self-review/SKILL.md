---
name: pr-self-review
description: Review Tyda changes against project principles before opening a PR.
---

# PR self-review

Use after implementation and before creating a PR. This reviews changes; it does not open the PR. Read only docs relevant to the diff.

## Review

1. Inspect the complete diff, including untracked files. Confirm one purpose and clear module boundaries. Do not preserve an existing anti-pattern just because it is old: fix it when closely related and the scope stays focused. Avoid unrelated refactors. Prefer concise general rules; abstract when duplication appears.
2. In `docs/design.md`, check that inference follows general Ruby semantics and official RBS, keeps the core generic, and confines library-specific dynamic DSLs to plugins.
3. Preserve RBS, diagnostics, and LSP output by default. Intentional changes need a scenario and PR review agreement; follow the CI approval-label rule in `docs/testing.md`.
4. Check that scenarios cover user-facing behavior and unit tests cover internal invariants. Record unsupported patterns as minimal known-issue scenarios with provisional expectations.
5. Check safety against `docs/incomplete-code-policy.md`: no panic/hang; flaky failures are fixed, not retried/skipped; fixed depth caps are not performance fixes. Guards are for pathological input and return `untyped` with a diagnostic. Run only needed small Ruby examples in isolation; inspect large OSS statically or with a minimal reproduction.
6. For runtime changes, use the `benchmark` skill on all subjects and compare elapsed time and peak memory. Reproduce warnings and resolve substantive regressions. Review depth follows risk; inspect performance and resource impact every time.
7. Confirm new dependencies have a reason and comparison of alternatives, maintenance, license, and performance impact. Update the matching living docs.

For findings, state the issue, evidence, and affected behavior. Fix clear defects and repeat the review. Do not open the PR with unresolved correctness, compatibility, or substantive performance findings; explain any remaining limitation to the user.

Self-review does not replace PR review.
