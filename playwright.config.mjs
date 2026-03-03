import { defineConfig, devices } from "@playwright/test";

// E2E for the Tyda Playground. Serves the built playground (`vite preview` of
// playground/dist) and drives it in headless Chromium. Asserts the wasm's
// behavior (inferred RBS + CodeLens + diagnostics + hover + URL restore), not
// binary identity — so local (macOS) and CI (Ubuntu) builds can differ.
// Build first (`mise run build` / `npm run build`); `mise run e2e` does that.
const PORT = 8123;

export default defineConfig({
  testDir: "playground/e2e",
  timeout: 60_000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: "list",
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: "on-first-retry",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: `npm run preview`,
    url: `http://localhost:${PORT}/`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
