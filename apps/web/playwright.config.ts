import { defineConfig, devices } from "@playwright/test";
import { defineBddConfig } from "playwright-bdd";

// playwright-bdd transforms the feature directory into a Playwright
// `testDir`. The `apps/web/tests/bdd/features` path is a symlink to
// `tests/bdd/features/` (the canonical Gherkin source for both the Rust
// `tanren-bdd` runner and the Node `playwright-bdd` runner). We tag-filter
// to `@web` so only the Web-surface scenarios run here — the other
// interface tags belong to the Rust harness.
const testDir = defineBddConfig({
  features: ["./tests/bdd/features/**/*.feature"],
  steps: ["./tests/bdd/steps/**/*.ts"],
  outputDir: "./tests/bdd/.bdd-gen",
  tags: "@web",
});

const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
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
  // The `tanren-api` Rust binary is spawned by globalSetup against an
  // ephemeral SQLite DB; the Next.js dev server below points at it via
  // NEXT_PUBLIC_API_URL. PLAYWRIGHT_NO_SERVER skips the Next.js spin-up
  // when the developer has already booted the dev server in another tab.
  ...(process.env["PLAYWRIGHT_NO_SERVER"]
    ? {}
    : {
        webServer: {
          // `next dev` is used over `next start` so `NEXT_PUBLIC_API_URL`
          // resolves at runtime (production builds bake the value at
          // build time, which is incompatible with our globalSetup that
          // picks an ephemeral API port).
          command: "pnpm dev",
          url: webBaseUrl,
          reuseExistingServer: process.env["CI"] !== "true",
          timeout: 240_000,
          env: {
            NEXT_PUBLIC_API_URL: apiUrl,
          },
        },
      }),
  globalSetup: "./tests/bdd/global-setup.ts",
  globalTeardown: "./tests/bdd/global-teardown.ts",
  timeout: 60_000,
});
