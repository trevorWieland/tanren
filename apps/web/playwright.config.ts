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

const webPort = process.env["PLAYWRIGHT_WEB_PORT"] ?? "3100";
const webBaseUrl = process.env["WEB_BASE_URL"] ?? `http://127.0.0.1:${webPort}`;

const webServerCommand = [
  "pnpm exec concurrently -n paraglide,next",
  '"paraglide-js compile --project ./src/i18n/project.inlang --outdir ./src/i18n/paraglide --emit-ts-declarations --watch"',
  `"next dev --turbopack --hostname 127.0.0.1 --port ${webPort}"`,
].join(" ");

// NOTE: NEXT_PUBLIC_API_URL is intentionally NOT captured here at
// config-load. globalSetup (./tests/bdd/global-setup.ts) chooses the API
// port at runtime — possibly falling back to a kernel-picked port when
// 8081 is busy — and writes the resolved URL to BOTH process.env and
// `apps/web/.env.local`. The webServer block below relies on
// inheritance: `pnpm dev` reads .env.local automatically (Next.js
// loads it ahead of .env), and any explicit `env:` here would override
// that with a stale value. Keeping the block absent fixes the
// nondeterministic-port bug Codex flagged on PR #133.

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
  // ephemeral SQLite DB; the Next.js dev server below picks up the API
  // URL from .env.local (written by globalSetup). PLAYWRIGHT_NO_SERVER
  // skips the Next.js spin-up when the developer has intentionally booted
  // the matching dev server in another tab.
  ...(process.env["PLAYWRIGHT_NO_SERVER"]
    ? {}
    : {
        webServer: {
          // `next dev` is used over `next start` so `NEXT_PUBLIC_API_URL`
          // resolves at runtime from .env.local (production builds
          // bake the value at build time, which is incompatible with
          // globalSetup picking an ephemeral API port).
          command: webServerCommand,
          url: webBaseUrl,
          reuseExistingServer: false,
          timeout: 240_000,
          // No `env` block — see comment above. NEXT_PUBLIC_API_URL is
          // sourced from .env.local, which globalSetup writes after
          // it has bound to the actual port.
        },
      }),
  globalSetup: "./tests/bdd/global-setup.ts",
  globalTeardown: "./tests/bdd/global-teardown.ts",
  timeout: 60_000,
});
