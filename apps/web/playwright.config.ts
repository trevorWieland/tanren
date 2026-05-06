import { defineConfig, devices } from "@playwright/test";
import { defineBddConfig } from "playwright-bdd";

const testDir = defineBddConfig({
  features: ["./tests/bdd/features/**/*.feature"],
  steps: ["./tests/bdd/steps/**/*.ts"],
  outputDir: "./tests/bdd/.bdd-gen",
  tags: "@web",
});

const webPort = process.env["PLAYWRIGHT_WEB_PORT"] ?? "3000";
const webBaseUrl = process.env["WEB_BASE_URL"] ?? `http://127.0.0.1:${webPort}`;

export default defineConfig({
  testDir,
  fullyParallel: false,
  forbidOnly: process.env["CI"] === "true",
  reporter: process.env["CI"] === "true" ? [["list"], ["github"]] : "list",
  use: {
    baseURL: webBaseUrl,
    trace: "on-first-retry",
    headless: true,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  ...(process.env["PLAYWRIGHT_NO_SERVER"]
    ? {}
    : {
        webServer: {
          command: "pnpm dev",
          url: webBaseUrl,
          reuseExistingServer: process.env["CI"] !== "true",
          timeout: 240_000,
        },
      }),
  globalSetup: "./tests/bdd/global-setup.ts",
  globalTeardown: "./tests/bdd/global-teardown.ts",
  timeout: 60_000,
});
