# Agent Guide

Tyda: a fast static type-inference engine for Ruby in Rust — RBS-emitting CLI +
TypeProf-compatible LSP. Layout: `src/` (core + CLI + LSP), `crates/rbs-sys` (RBS C
parser FFI), `vscode/` (editor extension), `playground/` (wasm demo), `docs/` (living docs).

## Rules (always apply)

- **Respond to the user in Japanese** (identifiers and code stay as-is). `docs/` is
  Japanese; code, comments, commit messages, and PRs are English. Use simple English.
- Keep comments minimal; explain why only when the code does not make it clear.
- Keep code readable with clear responsibilities. Generalize clear Ruby / RBS rules;
  keep core logic generic and put dynamic library DSL support in plugins. Prefer
  official RBS fixes; until then, safely return `untyped`.
- Do not assume existing code is ideal. Improve a closely related anti-pattern when
  the change stays focused; avoid unrelated refactors.
- Preserve inference quality, speed, and memory. Do not add fixed depth limits as a
  performance fix. Safety guards are for pathological input; return `untyped` and a
  diagnostic for the affected part.
- Scenarios define user-facing behavior; unit tests cover internal invariants.
  Reproduce regressions and flaky tests; never hide them with retries or skips.
- Keep each PR to one purpose and include related tests and docs. Preserve public
  output by default; intentional changes need a scenario and review agreement. Run
  `pr-self-review` before PR creation; review performance and resources every time.
- Avoid new dependencies. Explain why one is needed and compare alternatives,
  maintenance, license, and performance impact case by case.
- Commits: English, ≤50 chars, start with an imperative verb.
- PRs: English title and body, imperative title, and the sections in
  [`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md).
- Update the matching living doc (map below). Run checks for changed areas and
  `git diff --check`; see [`docs/development.md`](docs/development.md).

## Skills — task recipes

Reusable step-by-step recipes live in [`.agents/skills/`](.agents/skills/)
(`.claude/skills` is a symlink to it): `env-setup`, `dep-pr`, `dev-loop`,
`add-scenario-test`, `benchmark`, `render-compare`, `worktree-setup`,
`pr-self-review`, `submit-pr`.
If your agent does not load skills automatically, open the matching `SKILL.md`
before doing that task.

## Environment

Cloud / fresh clone (Codespaces, Devin, Codex, Claude cloud):
`./scripts/bootstrap.sh`. Playground / wasm / E2E: `./scripts/bootstrap.sh --full`.
Recipes: skills `env-setup` (bootstrap) and `dep-pr` (Dependabot / Renovate).
Benchmark subjects: `./scripts/setup_subjects.sh` fetches them at pinned commits.

## Code navigation

When Serena MCP tools are available (preconfigured in [`.mcp.json`](.mcp.json)), prefer
symbol-level lookup (`find_symbol`, references) over reading whole files.

## Docs — read on demand only

Do not preload docs. Open the one matching your task, and for large files jump to the
relevant heading instead of reading the whole file. The same table is the update map:
when your change touches a row's topic, update that doc in the same change.

| Topic | Doc |
|---|---|
| Conventions, setup, tasks (mise), release/versioning | [`docs/development.md`](docs/development.md) |
| Design rationale, principles | [`docs/design.md`](docs/design.md) |
| Code structure, components, pipeline | [`docs/architecture.md`](docs/architecture.md) |
| Implemented features, commands | [`docs/features.md`](docs/features.md) |
| Syntax / DSL / version support status | [`docs/capability-matrix.md`](docs/capability-matrix.md) |
| Test policy, scenario format | [`docs/testing.md`](docs/testing.md) |
| Benchmarks, perf baselines | [`docs/performance.md`](docs/performance.md) |
| LSP design / editor compatibility | [`docs/architecture.md`](docs/architecture.md) |
| Broken / incomplete code handling | [`docs/incomplete-code-policy.md`](docs/incomplete-code-policy.md) |
| Open items | [`docs/roadmap.md`](docs/roadmap.md) |
