---
name: env-setup
description: Bootstrap a fresh clone or cloud VM (Codespaces, Devin, Codex, Claude cloud) so cargo/check.sh work. Use at session start when tools, clang, mise, or vendor/rbs are missing.
---

# Environment setup

Details: `docs/development.md` (ツールとタスク / クラウド). Worktrees only need
`vendor/rbs` — use skill `worktree-setup`.

## Fresh Linux / cloud VM

```bash
./scripts/bootstrap.sh
```

Installs clang (if apt), mise (if missing), then `mise run setup-core`
(rust + ruby + `vendor/rbs`). Enough for `./scripts/check.sh` and cargo /
bundler dependency PRs.

Playground / wasm / Playwright:

```bash
./scripts/bootstrap.sh --full
```

## Already have mise (local laptop)

```bash
mise trust && mise run setup-core   # or: mise run setup
```

## Verify

```bash
./scripts/check.sh
```

`vendor/rbs` is gitignored. A missing tree silently degrades stdlib inference —
bootstrap / `setup-core` before trusting hover or scenario results.
