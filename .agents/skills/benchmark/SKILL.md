---
name: benchmark
description: Measure CLI/LSP speed and memory against recorded baselines. Use after perf-relevant changes (cache, dep graph, registry, type representation) to check for regressions.
---

# Benchmark

Baselines and full rules: `docs/performance.md` (基準値テーブル / 継続監視ルール).

## Rules

- Build release first: `cargo build --release`.
- Heavy subjects (gitlab): always `nice -n 19`, minimum runs (1 is fine), never parallelize.
- Reference scale: full gitlab render ≈ 7.5s / ~1GB peak. Compare against the table in
  `docs/performance.md`, not against memory.

## Commands

```bash
# CLI wall clock (small/medium subjects; use runs=1 for gitlab)
./scripts/benchmark.sh <path> [runs]

# Wall + max RSS in one shot (heavy subjects)
TYDA_MEMORY_BREAKDOWN=1 nice -n 19 target/release/tyda <path> >/dev/null

# LSP display-path benchmark (default: subject/gitlab/app)
./scripts/benchmark_lsp.sh [runs] [subject_path] [release|debug]
```

Subjects are fetched at pinned commits with `scripts/setup_subjects.sh` (all) or
`scripts/setup_subjects.sh gitlab` (one); `--list` shows the pinned table.

## Reporting

Update the baseline tables in `docs/performance.md` when numbers move; never mix
different subjects in one row.
