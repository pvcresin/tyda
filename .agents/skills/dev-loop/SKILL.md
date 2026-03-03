---
name: dev-loop
description: Token- and time-lean edit/verify loop for Rust changes. Use while iterating on code — run the full gate (check.sh) only once at the end.
---

# Dev loop

Escalate verification in stages; never iterate with `./scripts/check.sh` (clippy
all-targets + wasm clippy + full tests + release build — minutes per run).

## 0. Locate — symbols before whole files

Find code with Serena MCP symbol tools (`find_symbol`, references — see `AGENTS.md`)
or grep; read only the relevant spans, not entire files.

## 1. While editing — type errors only, short output

```bash
cargo check -q --message-format=short
```

## 2. Behavior — only the tests you touched

```bash
# Scenario tests: filter by substring of the path under tests/scenarios/
TYDA_SCENARIO_FILTER=rails/dsl cargo test -q --test scenario_runner -- --test-threads=1

# Other integration tests by file, unit tests by name filter
cargo test -q --test cli -- --test-threads=1
cargo test -q <name_substring> -- --test-threads=1
```

Parallel test runs produce spurious failures — always `--test-threads=1`.

## 3. Before finishing — the official gate, once

```bash
./scripts/check.sh
```

Update the matching living doc (map in `AGENTS.md`) in the same change.
