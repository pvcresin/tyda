# Tyda Playground

Runs the Tyda inference core (wasm32-wasip1) in the browser under a WASI shim,
with `vendor/rbs` mounted into a virtual fs. Ruby in → **LSP-style type
inference out**: inferred RBS, diagnostics (squiggles), and hover types — a
VSCode-like experience, fully client-side.

The wasm reads Ruby from stdin and prints one line of JSON on stdout:
`{ rbs, diagnostics, hovers, code_lens }`. The front-end is a **Vite + TypeScript**
app (`src/main.ts`): it bundles Monaco / lz-string / the WASI shim from npm (no
CDN), renders the RBS in the output pane, diagnostics as Monaco markers, hover
types via a hover provider, and inferred signatures as CodeLens.

Select one or more lines in either editor and press `Ctrl+/` on Windows/Linux or
`Command+/` on macOS to toggle Ruby line comments. Press the same shortcut again
to remove them.

## Quick start

```sh
mise trust         # first time: trust mise.toml
mise install       # first time: install tools (node)
mise run setup     # first time: wasm target + npm deps + Playwright browser
mise run dev       # build wasm+RBS and start Vite → http://localhost:8123
```

## Commands

Run tasks from the repo root with **`mise run <task>`** (unified cargo / npm / vite).
See `mise tasks` for the full list.

| command | what it does |
|---|---|
| `mise run dev` | build wasm+RBS → Vite dev server (HMR). Use this day-to-day |
| `mise run build` | production wasm + RBS + Vite build → `playground/dist` |
| `mise run preview` | serve the built `dist` |
| `mise run build-wasm` / `mise run build-rbs` | individual builds (`build-wasm` needs wasi-sdk) |
| `mise run e2e` | Playwright E2E against the production build |
| `mise run ci` | reproduce CI (build + E2E) in an Ubuntu container via `act` (needs Docker) |

`public/{tyda.wasm,rbs-bundle.json}` (generated) and `dist/` (Vite output) are gitignored.
Run `mise run build` (or `dev`) once before first use.

## CI / Deploy (GitHub Pages)

`.github/workflows/pages.yml` has two jobs:

- **build-and-test** — build wasm + RBS + Vite in CI and run E2E against that `dist`. Uses the same mise tasks as local (`mise run e2e`); reproduce with `mise run ci` (= `act -j build-and-test`) on Ubuntu (wasi-sdk follows host arch to avoid QEMU).
- **deploy** — `main` pushes only. Publishes `playground/dist` to GitHub Pages. Skipped under act (needs Pages OIDC).

E2E gates deploy: `main` publishes only when build-and-test is green. We require behavioral parity via E2E, not bit-identical binaries. Artifacts are CI-generated and **not committed**.

First-time setup: repo Settings → Pages → Source = "GitHub Actions".
