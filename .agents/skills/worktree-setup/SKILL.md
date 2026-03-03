---
name: worktree-setup
description: Prepare a git worktree of this repo for building/testing. Use at the start of any work inside a worktree — a missing vendor/rbs silently degrades stdlib type inference.
---

# Worktree setup

`vendor/rbs/` (stdlib RBS) and `vendor/bundle/` are gitignored build artifacts, so a
fresh worktree lacks them. Tyda still runs but stdlib inference silently degrades —
no warning is emitted. Fix it before testing inference.

## Options (either works)

```bash
# A. Generate in the worktree (needs ruby+bundler via mise)
mise run vendor-rbs
#    On a VM that is not set up yet: ./scripts/bootstrap.sh  (skill env-setup)
```

```bash
# B. Reuse the main checkout's vendor/rbs
ln -s <main-checkout>/vendor/rbs vendor/rbs
```

(Or set `TYDA_RBS_DIR` to an existing rbs dir; lookup order is
`TYDA_RBS_DIR` → next to the exe → `CARGO_MANIFEST_DIR`.)

## Testing in worktrees

Parallel `cargo test` produces spurious failures. The official gate is
`./scripts/check.sh`, which runs tests with `--test-threads=1`.
