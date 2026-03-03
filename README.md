# Tyda

> **"Tyda" is a working codename (subject to change).**

A fast static analysis / query-style type inference engine for Ruby, plus an RBS-emitting CLI and LSP server built on it.

## Overview

- Fast, low-memory type inference for Ruby / Rails (inference-focused, not a full type checker)
- Keeps CLI RBS output; treat RBS as a projection of the query inference engine
- TypeProf-compatible LSP: CodeLens / Hover / definition / typeDefinition
- Type-info priority: Ruby code / `.rbs` inference (highest) → inline RBS comments `#:` (next) → type checking (assistive). Sorbet (`sig` / `.rbi`) syntax-based inference is experimental

## Documentation

See [`docs/`](docs/) ([`docs/README.md`](docs/README.md) is the index). Living docs are written in Japanese.

- [Design](docs/design.md) / [Architecture](docs/architecture.md)
- [Features](docs/features.md) / [Capability matrix](docs/capability-matrix.md)
- [Testing](docs/testing.md) / [Performance](docs/performance.md)
- [Development guide](docs/development.md) / [Roadmap](docs/roadmap.md)

## Development

[mise](https://mise.jdx.dev) is the only local prerequisite. Then:

```bash
mise trust && mise run setup-core   # rust + ruby + vendor/rbs; enough for ./scripts/check.sh
# mise run setup                    # + wasm target, npm, Playwright (playground)
# ./scripts/bootstrap.sh            # bare Linux / Codespaces / cloud agents (clang + mise too)
```

Benchmarks run against real OSS projects, fetched at pinned commits so the numbers in
[`docs/performance.md`](docs/performance.md) are reproducible:

```bash
./scripts/setup_subjects.sh          # rack, rake, rubygems, mastodon, redmine, gitlab
./scripts/setup_subjects.sh --list   # the pinned commit table
```

Details: [`docs/development.md`](docs/development.md).

## License

MIT License ([`LICENSE`](LICENSE)). Third-party notices for bundled/linked components
(ruby/rbs, prism, etc.) are in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
