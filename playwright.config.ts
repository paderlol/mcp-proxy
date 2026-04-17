import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright config for E2E tests against the Vite dev build with Tauri
 * `invoke` mocked at the `window.__TAURI_INTERNALS__` level. See
 * tests/e2e/setup.ts for the mock.
 *
 * These tests do NOT require a Tauri runtime — they run against plain Chromium
 * so CI doesn't need platform-specific tauri-driver / WebDriver binaries.
 */
export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? [["github"], ["html", { open: "never" }]] : "list",

  use: {
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
    // Collect a screenshot + video only on failure so the reports stay small.
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },

  projects: [
    {
      name: "chromium",
      // Use whichever Chromium-flavored browser is already present, so a fresh
      // clone doesn't have to pull Playwright's bundled 150 MB build.
      //
      // Local (macOS): `channel: "chrome"` picks up `/Applications/Google
      // Chrome.app` directly. Set PLAYWRIGHT_CHROMIUM=1 to force the bundled
      // build instead (CI does this — see .github/workflows/ci.yml).
      use: {
        ...devices["Desktop Chrome"],
        ...(process.env.PLAYWRIGHT_CHROMIUM ? {} : { channel: "chrome" }),
      },
    },
  ],

  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
