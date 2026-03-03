---
name: add-scenario-test
description: Add a type-inference scenario test (Markdown-based). Use when fixing inference bugs or adding syntax/DSL support — every inference change should be pinned by a scenario.
---

# Add a scenario test

Scenario tests are Markdown files under `tests/scenarios/` run by `tests/scenario_runner.rs`.
Full rules: `docs/testing.md` (## scenario test, ## scenario カテゴリ).

## Format (one case)

```markdown
## <case name>

```ruby
def foo = 1
```

### result

```rbs
class Object
  def foo: () -> Integer
end
```
```

- `##` separates cases; first ```yaml right after `##` is case config
  (`ruby_version` / `rails_version` / `include_synthetic_dsl_methods` /
  `known_issue`).
- ```rbs before `### result` is input RBS; ```rbi is input RBI.
- Fixture files (e.g. `db/schema.rb`): put the path as inline code on the line before the block.
- Style: single-expression method bodies use endless methods unless it hurts readability.

## Placement

Pick the existing category: `ruby/{class,control,literal,method,method/blocks,rbs_comment,rbs_input,runtime,variable}`,
`rails/{active_record,active_support,dsl,routes,schema}`, `sorbet/{rbs_comment,sig}`.
Prefer appending to an existing file on the same topic over creating a new file.
If the expected RBS is correct Ruby but Tyda does not match yet, put the case in
`known-issues/` (or set `known_issue: true`). The runner keeps a mismatch as
open; a sudden match fails until the case is promoted. Keep the directory even
when empty (`.gitkeep`).

## Verify

- Quick, scoped (substring of the path under `tests/scenarios/`):
  `TYDA_SCENARIO_FILTER=rails/dsl cargo test -q --test scenario_runner -- --test-threads=1`
- Official gate before finishing: `./scripts/check.sh` (runs all tests with `--test-threads=1`).
