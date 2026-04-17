import { defineConfig } from "vitest/config";

/**
 * Vitest-only config. Playwright's `tests/e2e/**` specs must not be collected
 * here — they use `@playwright/test`'s `test.describe`, which blows up inside
 * Vitest ("two different versions of @playwright/test").
 *
 * Run the E2E specs via `npm run test:e2e` instead.
 */
export default defineConfig({
  test: {
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "tests/e2e/**",
      "**/.{idea,git,cache,output,temp}/**",
    ],
  },
});
