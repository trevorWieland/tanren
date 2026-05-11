// playwright-bdd step definitions for the `@web` slice of B-0134.
// Shared Gherkin under `tests/bdd/features/` remains the single source of truth.
//
// The driver calls the test-hooks upgrade-fixture API directly from Node
// (not through a browser witness page). The test-hook secret
// (TANREN_TEST_HOOK_SECRET) is set in the Playwright driver process env
// by global-setup.ts and never projected into the Next.js client bundle.

import {
  type PlaywrightTestArgs,
  type PlaywrightTestOptions,
  type PlaywrightWorkerArgs,
  type PlaywrightWorkerOptions,
  type TestType,
} from "@playwright/test";
import {
  type BddTestFixtures,
  type BddWorkerFixtures,
  createBdd,
  test as base,
} from "playwright-bdd";

import type { UpgradeWorld } from "../support/asset-fixture";
import {
  assertPreviewContainsConcern,
  assertPreviewContainsPath,
  assertUpgradeApplyNoop,
  assertUpgradeApplySuccess,
  assertUpgradePreviewReported,
  assertUpgradePreviewSuccess,
  assertUpgradeRequiresConfirmation,
  assertUpgradeApplyContainsPath,
  assertPreviewApplyPreviewIdCorrelation,
  extractPreviewIdFromStdout,
} from "../support/upgrade-assertions";
import { UpgradeDriver } from "../support/upgrade-driver";

type UpgradeTest = TestType<
  PlaywrightTestArgs &
    PlaywrightTestOptions &
    BddTestFixtures & { world: UpgradeWorld },
  PlaywrightWorkerArgs & PlaywrightWorkerOptions & BddWorkerFixtures
>;

export const test: UpgradeTest = base.extend<{ world: UpgradeWorld }>({
  world: async ({}, use) => {
    await use({ lastPreviewId: undefined });
  },
});

const { Given, When, Then } = createBdd(test);

const driver = new UpgradeDriver();

Given("a clean repository fixture", async ({ world }) => {
  delete world.lastRun;
  await driver.resetFixture();
});

Given(
  /^repository file "([^"]+)" contains "([^"]+)"$/,
  async ({}, path: string, content: string) => {
    await driver.writeRepositoryFile(path, content);
  },
);

Given(
  /^repository file "([^"]+)" baseline is recorded$/,
  async ({}, path: string) => {
    await driver.recordBaseline(path);
  },
);

Given(
  /^an installed repository fixture snapshot "([^"]+)" with profile "([^"]+)" and integrations "([^"]+)"$/,
  async ({}, snapshotLabel: string, profile: string, integrations: string) => {
    await driver.seedInstalledSnapshot(snapshotLabel, profile, integrations);
  },
);

Given(
  /^legacy standards path "([^"]+)" is marked as a migration concern$/,
  async ({}, path: string) => {
    await driver.markLegacyMigrationConcern(path);
  },
);

Given(
  /^repository snapshot "([^"]+)" is captured$/,
  async ({}, label: string) => {
    await driver.captureSnapshot(label);
  },
);

When("tanren-cli upgrade preview runs", async ({ world }) => {
  await driver.runUpgradePreview(world);
  if (world.lastRun) {
    world.lastPreviewId =
      extractPreviewIdFromStdout(world.lastRun.stdout) ?? undefined;
  }
});

When("tanren-cli upgrade apply runs with confirmation", async ({ world }) => {
  await driver.runUpgradeApply(world);
});

Then("the upgrade preview command succeeds", async ({ world }) => {
  assertUpgradePreviewSuccess(world);
});

Then("the upgrade preview is reported", async ({ world }) => {
  assertUpgradePreviewReported(world);
});

Then("the upgrade command requests confirmation", async ({ world }) => {
  assertUpgradeRequiresConfirmation(world);
});

Then("the upgrade apply command succeeds", async ({ world }) => {
  assertUpgradeApplySuccess(world);
});

Then("the upgrade apply command reports noop", async ({ world }) => {
  assertUpgradeApplyNoop(world);
});

Then(
  /^the upgrade preview lists compatibility concern "([^"]+)"$/,
  async ({ world }, concern: string) => {
    assertPreviewContainsConcern(world, concern);
  },
);

Then(
  /^the upgrade preview lists path "([^"]+)"$/,
  async ({ world }, path: string) => {
    assertPreviewContainsPath(world, path);
  },
);

Then(
  /^the upgrade apply lists path "([^"]+)"$/,
  async ({ world }, path: string) => {
    assertUpgradeApplyContainsPath(world, path);
  },
);

Then("no files are written in the repository fixture", async ({}) => {
  await driver.assertNoWrites();
});

Then(
  /^the repository matches snapshot "([^"]+)"$/,
  async ({}, label: string) => {
    await driver.assertMatchesSnapshot(label);
  },
);

Then(
  /^repository file "([^"]+)" preserves its baseline content$/,
  async ({}, path: string) => {
    await driver.assertPreservesBaseline(path);
  },
);

Then(
  /^repository file "([^"]+)" is replaced from its baseline content$/,
  async ({}, path: string) => {
    await driver.assertReplacedFromBaseline(path);
  },
);

Then(/^repository file "([^"]+)" does not exist$/, async ({}, path: string) => {
  await driver.assertFileMissing(path);
});

Then(
  /^repository file "([^"]+)" includes "([^"]+)"$/,
  async ({}, path: string, content: string) => {
    await driver.assertFileContains(path, content);
  },
);

Then(
  /^repository file "([^"]+)" does not include "([^"]+)"$/,
  async ({}, path: string, content: string) => {
    await driver.assertFileNotContains(path, content);
  },
);

Then(
  "the upgrade preview and apply share the same preview id",
  async ({ world }) => {
    assertPreviewApplyPreviewIdCorrelation(world);
  },
);
