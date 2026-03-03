import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

// Vite project for the Tyda Playground (wasm browser demo). `root` is pinned to
// this file's directory so it can be invoked from the repo root with
// `--config playground/vite.config.ts` regardless of cwd. `base: "./"` keeps
// asset paths relative, so it works under a GitHub Pages subpath and the
// `fetch("./tyda.wasm")` / `fetch("./rbs-bundle.json")` loads resolve in both
// dev and the built `dist/`. tyda.wasm / rbs-bundle.json live in `public/`
// (copied verbatim into `dist/`).
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  base: "./",
  server: { port: 8123, strictPort: true },
  preview: { port: 8123, strictPort: true },
  // Monaco's editor core is ~2.6 MB raw (~670 kB gzip) even trimmed to the
  // CodeLens + hover features; that's the expected single chunk, so lift the
  // warning threshold above it.
  build: { outDir: "dist", emptyOutDir: true, target: "es2022", chunkSizeWarningLimit: 3000 },
});
