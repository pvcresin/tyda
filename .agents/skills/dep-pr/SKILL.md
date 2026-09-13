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

## 3. Choose a canonical dependency resolution

Before editing a lockfile, classify the affected package as direct or transitive,
runtime or development-only, and inspect the parent package's declared requirement.
For a vulnerability, confirm the advisory's affected range, first patched version,
and whether the vulnerable package can be reached in this project.

Prefer these resolutions in order:

1. Upgrade the direct dependency to a release that carries the patched transitive
   dependency.
2. Update a direct dependency within its declared range and regenerate the lockfile.
3. For a transitive dependency, use a newer parent release or an upstream fix that
   relaxes the parent's pin. Adding the same package as a new top-level dependency
   is not sufficient if the lockfile still contains a vulnerable nested copy.

Treat 0.x minor updates and all major updates as potentially breaking; validate the
API, build, and runtime behavior rather than relying on semver alone. Do not force a
downgrade to make the resolver succeed.

Do not introduce npm `overrides`, Yarn `resolutions`, or equivalent patch mechanisms
merely to silence an alert. Use one only after confirming that no compatible direct
or upstream release exists and the user explicitly accepts the compatibility risk.
If one is approved, record the owning dependency, why the patched version is
compatible, and a concrete removal condition. Verify it with a clean install,
dependency-tree/lockfile inspection, the relevant audit, and the normal runtime gate.
When the parent package ships a bundled copy of the transitive library, also inspect
the generated artifact: a lockfile override may clear the package-manager alert
without changing the vulnerable code that users execute.
If no canonical resolution exists, report the blocker and the available upstream
or mitigation options instead of forcing a resolver result.

## 4. Finish

- Living docs only if the bump changes a documented version/tool (map in `AGENTS.md`).
- Commit/PR conventions: skill `submit-pr`. Do not re-title Dependabot/Renovate PRs
  unless asked.
- Merge only when the user asked and the gate (or GitHub `Test` + `pages` checks)
  is green: `gh pr merge --squash` (or the repo default).
