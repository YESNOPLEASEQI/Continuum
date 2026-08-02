import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e/tests",
  fullyParallel: false,
  workers: process.env.CI ? 2 : 1,
  use: {
    baseURL: "http://127.0.0.1:1420",
    trace: "on-first-retry",
    viewport: { width: 1380, height: 880 },
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: true,
  },
  retries: process.env.CI ? 2 : 0,
});
