/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0134.
// Shared Gherkin under `tests/bdd/features/` remains the single source of truth.

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
    await use({});
  },
});

const { Given, When, Then } = createBdd(test);

function driverFor(page: PlaywrightTestArgs["page"]): UpgradeDriver {
  return new UpgradeDriver(page);
}

Given("a clean repository fixture", async ({ page, world }) => {
  delete world.lastRun;
  await driverFor(page).resetFixture();
});

Given(
  /^repository file "([^"]+)" contains "([^"]+)"$/,
  async ({ page }, path: string, content: string) => {
    await driverFor(page).writeRepositoryFile(path, content);
  },
);

Given(
  /^repository file "([^"]+)" baseline is recorded$/,
  async ({ page }, path: string) => {
    await driverFor(page).recordBaseline(path);
  },
);

Given(
  /^an installed repository fixture snapshot "([^"]+)" with profile "([^"]+)" and integrations "([^"]+)"$/,
  async (
    { page },
    snapshotLabel: string,
    profile: string,
    integrations: string,
  ) => {
    await driverFor(page).seedInstalledSnapshot(
      snapshotLabel,
      profile,
      integrations,
    );
  },
);

Given(
  /^legacy standards path "([^"]+)" is marked as a migration concern$/,
  async ({ page }, path: string) => {
    await driverFor(page).markLegacyMigrationConcern(path);
  },
);

Given(
  /^repository snapshot "([^"]+)" is captured$/,
  async ({ page }, label: string) => {
    await driverFor(page).captureSnapshot(label);
  },
);

When("tanren-cli upgrade preview runs", async ({ page, world }) => {
  await driverFor(page).runUpgradePreview(world);
});

When(
  "tanren-cli upgrade apply runs with confirmation",
  async ({ page, world }) => {
    await driverFor(page).runUpgradeApply(world);
  },
);

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

Then("no files are written in the repository fixture", async ({ page }) => {
  await driverFor(page).assertNoWrites();
});

Then(
  /^the repository matches snapshot "([^"]+)"$/,
  async ({ page }, label: string) => {
    await driverFor(page).assertMatchesSnapshot(label);
  },
);

Then(
  /^repository file "([^"]+)" preserves its baseline content$/,
  async ({ page }, path: string) => {
    await driverFor(page).assertPreservesBaseline(path);
  },
);

Then(
  /^repository file "([^"]+)" is replaced from its baseline content$/,
  async ({ page }, path: string) => {
    await driverFor(page).assertReplacedFromBaseline(path);
  },
);

Then(
  /^repository file "([^"]+)" does not exist$/,
  async ({ page }, path: string) => {
    await driverFor(page).assertFileMissing(path);
  },
);
