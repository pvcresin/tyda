# Tyda - Type inference tool for lazy Rubyists

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/pvcresin/tyda)

[Website](https://pvcresin.github.io/tyda/) · [Playground](https://pvcresin.github.io/tyda/play/) · [Documentation](https://pvcresin.github.io/tyda/docs/)

Tyda (pronounced “tie-duh”, /ˈtaɪdə/) is a type inference tool for Ruby and Rails. It infers useful types without requiring type annotations; its name comes from 怠惰, the Japanese word for “laziness”.

> **Note:** Tyda is pre-1.0. Its API, CLI, language server, and inference behavior may change.

## Features

- 🧠 **Inference without annotations** — Infer useful types from Ruby and Rails code without annotating every method.
- 🧑‍💻 **Editor integration** — Explore inferred signatures and types in VS Code with hover, CodeLens, and navigation.
- 📄 **RBS output and diagnostics** — Emit inferred RBS and supplementary JSON Lines diagnostics from the CLI.
- 🎮 **Browser Playground** — Try Ruby and RBS in the browser without installing Tyda.

## Get started

### Try the Playground

The [Playground](https://pvcresin.github.io/tyda/play/) is the quickest way to try Tyda. No installation is required.

### Use Tyda in VS Code

Tyda provides the language server, while the [Ruby TypeProf extension](https://marketplace.visualstudio.com/items?itemName=mame.ruby-typeprof) displays its inferred signatures and types in VS Code. Install both:

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

Open a Ruby file to see inferred signatures and types. If VS Code cannot find `tyda` on `PATH`, use the absolute path returned by `which tyda`. You do not need to install the `typeprof` gem separately.

## CLI

Use the CLI for batch RBS output and supplementary diagnostics:

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
