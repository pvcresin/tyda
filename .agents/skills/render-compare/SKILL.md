---
name: render-compare
description: Gate a refactor or optimization with byte-identical RBS render output across subjects. Use for any change that must not alter inference results (perf work, internal refactors).
---

# Render byte comparison

`scripts/snapshot_outputs.sh` renders each subject (sample / rack / rake / rubygems /
mastodon / gitlab) with `target/release/tyda` and diffs stdout byte-for-byte.

## Workflow

```bash
cargo build --release

# 1. On the baseline commit (or before your change):
./scripts/snapshot_outputs.sh            # saves to /tmp/tyda_snapshots (TYDA_SNAPSHOT_DIR to override)

# 2. After your change (rebuild release first):
./scripts/snapshot_outputs.sh --verify   # non-zero exit on any byte diff
```

## Rules

- Any diff must be intentional: classify every changed line and mention it in the PR.
- To find why lines changed, save `tyda trace -o` with the base and head binaries in the same
  subject and follow `docs/development.md` (推論の変化を調べる): the slots whose `type` changed
  match the RBS diff, and `tyda why <id> --trace <file>` walks each one to the slot where the
  change starts.
- gitlab is heavy (~8s / ~1GB); the script already runs it with `nice -n 19`, one pass.
- `--diagnostics` output is a separate gate (JSONL comparison), not covered here.
