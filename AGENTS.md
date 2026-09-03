# Agent Guide

Tyda: a fast static type-inference engine for Ruby in Rust — RBS-emitting CLI +
TypeProf-compatible LSP. Layout: `src/` (core + CLI + LSP), `crates/rbs-sys` (RBS C
parser FFI), `vscode/` (editor extension), `playground/` (wasm demo), `docs/` (living docs).

## Rules (always apply)

- **Respond to the user in Japanese** (identifiers and code stay as-is). `docs/` is
  Japanese; root entry files, code, comments, and commit messages are English.
- Code comments: English, minimal, why-not only — prefer no comment.
- Commits: English, ≤50 chars, start with an imperative verb.
- PRs: title and body in English. Use an imperative title like commits and follow
  [`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md), keeping
  its `## Summary` and `## Verification` sections.
- When finishing: update the matching living doc (map below) and run `./scripts/check.sh`.

## Skills — task recipes

Reusable step-by-step recipes live in [`.agents/skills/`](.agents/skills/)
(`.claude/skills` is a symlink to it): `env-setup`, `dep-pr`, `dev-loop`,
`add-scenario-test`, `benchmark`, `render-compare`, `worktree-setup`, `submit-pr`.
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
