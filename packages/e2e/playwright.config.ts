import { defineConfig, devices } from "@playwright/test";

/**
 * Nova Dashboard E2E test configuration.
 *
 * Tests run against the deployed dev environment. Set BASE_URL to override.
 * When DASHBOARD_TOKEN is unset the dashboard runs in unauthenticated dev mode.
 */

const BASE_URL = process.env.BASE_URL ?? "http://localhost:3000";

export default defineConfig({
  testDir: "./tests",
  timeout: 60_000,
  expect: { timeout: 5_000 },
  retries: process.env.CI ? 2 : 0,
  fullyParallel: true,
  reporter: process.env.CI ? "github" : "list",

  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
