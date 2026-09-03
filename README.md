# Tyda

> Type inference for lazy Rubyists.

Tyda is a fast static type inference engine for Ruby. It aims to infer useful types
without requiring type annotations, with a TypeProf-compatible language server as its
main interface. It also provides a CLI for RBS output and supplementary type checking.

## Overview

- Infer useful types from Ruby / Rails code without writing type annotations
- Explore inferred types in the editor through TypeProf-compatible CodeLens, Hover, definition, and typeDefinition support
- Use the CLI to emit RBS and run supplementary type checks and diagnostics
- Type-info priority: Ruby code / `.rbs` inference (highest) → inline RBS comments `#:` (next) → type checking (assistive). Sorbet (`sig` / `.rbi`) syntax-based inference is experimental

## Quick start

Tyda's main interface is the TypeProf-compatible language server. Install the gem,
connect it to the Ruby TypeProf VS Code extension, and open a Ruby file to see inferred
types in the editor.

### 1. Install Tyda

```bash
gem install tyda
```

### 2. Install the Ruby TypeProf extension

Install [Ruby TypeProf](https://marketplace.visualstudio.com/items?itemName=mame.ruby-typeprof)
from the VS Code Marketplace, or run:

```bash
code --install-extension mame.ruby-typeprof
```

### 3. Configure the extension to use Tyda

Point the extension at the `tyda` executable installed by RubyGems. Add this to your
project's `.vscode/settings.json`:

```json
{
  "typeprof.server.path": "tyda"
}
```

If VS Code cannot find `tyda` on `PATH`, run `which tyda` and use the resulting absolute
path instead. You do not need to install the `typeprof` gem separately.

### 4. Open a Ruby file

Try this example:

```ruby
def greet(name)
  "Hello, #{name}!"
end

greet("Tyda")
```

Open the file in VS Code. The TypeProf extension shows inferred method signatures and
types on hover. Restart the TypeProf language server after changing the server path.

## Optional CLI

The editor is the main way to explore Tyda's inference. The CLI is also available for
batch RBS output and supplementary diagnostics:

```bash
tyda path/to/file.rb                 # print inferred RBS
tyda --diagnostics path/to/file.rb  # print JSON Lines diagnostics
```

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
