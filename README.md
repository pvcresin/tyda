# Tyda

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/pvcresin/tyda)

Type inference for lazy Rubyists.

[Website](https://pvcresin.github.io/tyda/) · [Playground](https://pvcresin.github.io/tyda/play/) · [Documentation](https://pvcresin.github.io/tyda/docs/)

Tyda (/ˈtaɪdə/, “tie-duh”) is a Ruby type-inference tool that infers useful types from Ruby and Rails code without requiring type annotations. The name comes from 怠惰, the Japanese word for “laziness”.

> **Note:** Tyda is pre-1.0. Its API, CLI, language server, and inference behavior may change.

## What Tyda does

- **Editor integration** — Explore inferred types through a TypeProf-compatible language server with hover, CodeLens, definition, and type definition support.
- **CLI output** — Emit RBS and supplementary JSON Lines diagnostics for batch use.
- **Playground** — Try Ruby and RBS in the browser without installing Tyda.

## Get started

The [Playground](https://pvcresin.github.io/tyda/play/) is the quickest way to try Tyda.

To use Tyda in VS Code, install Tyda and the [Ruby TypeProf extension](https://marketplace.visualstudio.com/items?itemName=mame.ruby-typeprof):

```sh
gem install tyda
code --install-extension mame.ruby-typeprof
```

Set Tyda as the language server in your project's `.vscode/settings.json`:

```json
{
  "typeprof.server.path": "tyda"
}
```

Open a Ruby file to see inferred signatures and types. You do not need to install the `typeprof` gem separately.

## CLI

```sh
tyda path/to/file.rb                 # print inferred RBS
tyda --diagnostics path/to/file.rb  # print JSON Lines diagnostics
```

## Documentation

The [documentation](https://pvcresin.github.io/tyda/docs/) covers supported syntax, inference behavior, editor integration, architecture, and development.

## Development

```sh
mise trust
mise run setup-core
./scripts/check.sh
```

See the [Development guide](https://pvcresin.github.io/tyda/docs/development) for playground, benchmark, and CI tasks.

## License

MIT License ([`LICENSE`](LICENSE)). Third-party notices for bundled/linked components (ruby/rbs, prism, etc.) are in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
