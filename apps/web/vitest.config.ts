import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Two Vitest projects share this config:
//
// - `unit`     — fast Node-environment unit tests for non-component code.
//                F-0001 ships zero unit tests; the project exists so
//                `vitest run --project=unit` is a no-op rather than an
//                error, and so future PRs can plug coverage in by
//                dropping a `*.test.ts` next to the source.
// - `storybook` — runs every `*.stories.@(ts|tsx)` as a real-browser
//                Vitest component test via @storybook/addon-vitest. The
//                play function on each story drives the DOM; addon-a11y
//                runs axe-core after the play settles. Browser is
//                Chromium under Playwright (matches Storybook 9 + Vitest
//                addon's default browser provider).
//
// The shape mirrors profiles/react-ts-pnpm/testing/component-testing-via-storybook.md.
export default defineConfig({
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
    },
  },
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: "unit",
          environment: "node",
          include: ["src/**/*.{test,spec}.{ts,tsx}"],
          // Stories are owned by the `storybook` project — exclude them
          // from the unit project so an accidental match on a `.stories.ts`
          // file doesn't double-run.
          exclude: [
            "src/**/*.stories.@(ts|tsx)",
            "node_modules/**",
            ".next/**",
            "src/i18n/paraglide/**",
          ],
        },
      },
      {
        extends: true,
        plugins: [
          storybookTest({
            configDir: resolve(__dirname, ".storybook"),
          }),
        ],
        test: {
          name: "storybook",
          browser: {
            enabled: true,
            provider: playwright(),
            headless: true,
            instances: [{ browser: "chromium" }],
          },
          setupFiles: ["./vitest.storybook-setup.ts"],
        },
      },
    ],
  },
});
