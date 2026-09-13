import { defineConfig, devices } from "@playwright/test";

// E2E for the Tyda Playground. Serves the combined Pages artifact and drives
// the Playground mounted at /play/ in headless Chromium. Asserts the wasm's
// behavior (inferred RBS + CodeLens + diagnostics + hover + URL restore), not
// binary identity — so local (macOS) and CI (Ubuntu) builds can differ.
// `mise run e2e` builds the combined Pages artifact first.
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
    command: `node scripts/serve-pages.mjs --port ${PORT}`,
    url: `http://localhost:${PORT}/`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
