import { defineConfig, devices } from "@playwright/test";

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? "http://127.0.0.1:8080";

export default defineConfig({
  testDir: "./e2e",
  outputDir: "./build/playwright/test-results",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  reporter: [
    ["line"],
    ["html", { open: "never", outputFolder: "./build/playwright/report" }],
  ],
  use: {
    ...devices["Desktop Chrome"],
    baseURL,
    channel: "chromium",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "readiness",
      testMatch: /readiness\.setup\.ts/,
      timeout: 600_000,
    },
    {
      name: "fullstack",
      testMatch: /.*\.spec\.ts/,
      dependencies: ["readiness"],
    },
  ],
});
