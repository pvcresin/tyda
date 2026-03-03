# Tyda — Ruby type inference (VS Code / Cursor)

Fast Ruby type inference (RBS) powered by the [tyda](https://github.com/pvcresin/tyda)
engine. The extension launches `tyda --lsp` and connects to it as a
TypeProf-compatible language server.

## Features

- **Inline signatures (code lens)** — the inferred `(args) -> return` is shown
  above each method. Click it to insert the signature as a `#:` comment.
- **Diagnostics** — unresolved calls, argument-type mismatches, and (with
  `tyda.experimentalChecks`) arity errors are reported inline.
- **Completion** — method/constant completion (triggered on `.` and `::`).
- **Go to definition / type definition** — jump to a symbol or its inferred type.
- **Hover** — inferred types on hover.
- **Toggle** — show/hide the inline signatures from the status bar or the
  `Tyda: Toggle inline type signatures` command.

> **Status: not yet published.** This directory scaffolds the extension and its
> release pipeline; the publish step is intentionally disabled. See
> [`docs/development.md`](../docs/development.md).

## How it finds the `tyda` binary

1. The `tyda.server.path` setting, if set (point it at `target/release/tyda` for
   local development).
2. The binary bundled into the extension at `bin/tyda` (or `bin/tyda.exe` on
   Windows, staged per-platform by the release pipeline).
3. `tyda` on your `PATH`.

## Settings

- `tyda.server.path` — path to the `tyda` binary (see above).
- `tyda.experimentalChecks` — run the server with `TYDA_EXPERIMENTAL_CHECKS=1`
  to surface experimental diagnostics (arity).
- `tyda.trace.server` — trace LSP traffic (`off` / `messages` / `verbose`).

## Build locally

From the repository root:

```sh
mise run vscode-build      # type-check + bundle to vscode/dist
mise run vscode-package    # build tyda, stage binary + stdlib RBS, make vscode/tyda.vsix
```

Or directly in this directory:

```sh
npm install
npm run compile            # dist/extension.js (esbuild)
npm run package            # tyda.vsix (vsce) — bundles whatever is in bin/
```

Install the resulting `.vsix` with “Extensions: Install from VSIX…”.

Release VSIX targets are Linux x64, Windows x64, macOS Intel, and macOS ARM64.
