---
name: dep-pr
description: Land Dependabot or Renovate (or similar) dependency-update PRs from a cloud environment. Use when reviewing or merging cargo, bundler, npm, or GitHub Actions bumps.
---

# Dependency-update PRs

Goal: verify the bump on a cloud VM and merge. Do not expand the PR's scope.
Setup recipe: skill `env-setup`. Conventions: `AGENTS.md`.

## 1. Bootstrap

```bash
./scripts/bootstrap.sh
```

If the bump is playground npm / wasm / E2E, use `./scripts/bootstrap.sh --full`.

## 2. Pick the gate from the diff

| Touches | Gate |
|---|---|
| `Cargo.lock` / `Cargo.toml` / `rust-toolchain.toml` / `crates/` | `./scripts/check.sh` |
| `Gemfile.lock` / `Gemfile` | `mise run vendor-rbs` then `./scripts/check.sh` |
| `package-lock.json` / `package.json` / `playground/` | `npm install` and `npx oxfmt --check playground`; E2E only if wasm/runtime changed (`mise run e2e`) |
| `.github/workflows/` only | CI on the PR is the gate; no local compile unless a script changed |

## 3. Finish

- Living docs only if the bump changes a documented version/tool (map in `AGENTS.md`).
- Commit/PR conventions: skill `submit-pr`. Do not re-title Dependabot/Renovate PRs
  unless asked.
- Merge only when the user asked and the gate (or GitHub `Test` + `pages` checks)
  is green: `gh pr merge --squash` (or the repo default).
