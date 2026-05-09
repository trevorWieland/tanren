/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0066.
//
// These steps drive a web-owned organization wire surface (`/organizations`)
// instead of issuing direct Playwright API requests.

import { createBdd } from "playwright-bdd";

import {
  ORGANIZATION_WIRE_TEST_IDS,
  organizationRowTestId,
  normalizeOrganizationName,
} from "@/lib/organization-routes";

import {
  openOrganizationWireSurface,
  readWireSequence,
  readOrganizationSnapshot,
  signInActorViaUi,
  waitForWireOutcome,
} from "./api-client";
import { test } from "./account.steps";
import {
  ALL_ADMIN_PERMISSIONS,
  actor,
  orgState,
  type OrganizationWorld,
} from "./organization-world";

const { Then, When } = createBdd(test);

When(
  /^(\w+) creates organization "([^"]+)"$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const state = orgState(world as OrganizationWorld);
    const a = actor(world as OrganizationWorld, name);

    await signInActorViaUi(page, world as OrganizationWorld, name);
    await openOrganizationWireSurface(page);
    await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.createNameInput)
      .fill(organizationName);
    const previousSequence = await readWireSequence(page);
    await page.getByTestId(ORGANIZATION_WIRE_TEST_IDS.createSubmit).click();

    const outcome = await waitForWireOutcome(page, previousSequence);
    if (outcome.status !== "success") {
      a.hasSession = false;
      a.lastFailureCode = outcome.failureCode ?? "unknown";
      state.lastOperationSucceeded = false;
      throw new Error(
        `create organization failed: ${outcome.failureDetail ?? outcome.failureCode ?? "unknown"}`,
      );
    }

    const snapshot = await readOrganizationSnapshot(page, organizationName);
    state.organizationsByName.set(normalizeOrganizationName(organizationName), {
      id: snapshot.id,
      grantedPermissions: snapshot.grantedPermissions,
      initialProjectCount: snapshot.initialProjectCount,
    });
    state.lastOperationSucceeded = true;
  },
);

When(
  /^(\w+) creates organization "([^"]+)" without signing in$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const state = orgState(world as OrganizationWorld);
    const a = actor(world as OrganizationWorld, name);

    await page.context().clearCookies();
    await openOrganizationWireSurface(page);
    await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.createNameInput)
      .fill(organizationName);
    const previousSequence = await readWireSequence(page);
    await page.getByTestId(ORGANIZATION_WIRE_TEST_IDS.createSubmit).click();

    const outcome = await waitForWireOutcome(page, previousSequence);
    if (outcome.status === "success") {
      a.hasSession = true;
      delete a.lastFailureCode;
      state.lastOperationSucceeded = true;
      return;
    }

    a.hasSession = false;
    a.lastFailureCode = outcome.failureCode ?? "unknown";
    state.lastOperationSucceeded = false;
  },
);

When(
  /^(\w+) lists available organizations$/,
  async ({ page, world }, name: string) => {
    const state = orgState(world as OrganizationWorld);
    const a = actor(world as OrganizationWorld, name);

    await signInActorViaUi(page, world as OrganizationWorld, name);
    await openOrganizationWireSurface(page);
    const previousSequence = await readWireSequence(page);
    await page.getByTestId(ORGANIZATION_WIRE_TEST_IDS.listSubmit).click();

    const outcome = await waitForWireOutcome(page, previousSequence);
    if (outcome.status !== "success") {
      a.hasSession = false;
      a.lastFailureCode = outcome.failureCode ?? "unknown";
      state.lastOperationSucceeded = false;
      throw new Error(
        `list organizations failed: ${outcome.failureDetail ?? outcome.failureCode ?? "unknown"}`,
      );
    }

    state.lastOperationSucceeded = true;
  },
);

When(
  /^(\w+) checks organization permission "([^"]+)" in "([^"]+)"$/,
  async (
    { page, world },
    name: string,
    permission: string,
    organizationName: string,
  ) => {
    const state = orgState(world as OrganizationWorld);
    const a = actor(world as OrganizationWorld, name);

    await signInActorViaUi(page, world as OrganizationWorld, name);

    const org = state.organizationsByName.get(
      normalizeOrganizationName(organizationName),
    );
    if (!org) {
      throw new Error(
        `organization ${organizationName} must be created or listed before permission checks`,
      );
    }

    await openOrganizationWireSurface(page);
    await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionOrgIdInput)
      .fill(org.id);
    await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionSelect)
      .selectOption(permission);
    const previousSequence = await readWireSequence(page);
    await page.getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionSubmit).click();

    const outcome = await waitForWireOutcome(page, previousSequence);
    if (outcome.status !== "success") {
      a.hasSession = false;
      a.lastFailureCode = outcome.failureCode ?? "unknown";
      state.lastOperationSucceeded = false;
      return;
    }

    state.lastOperationSucceeded = true;
  },
);

Then("the operation succeeds", async ({ world }) => {
  const state = orgState(world as OrganizationWorld);
  if (!state.lastOperationSucceeded) {
    throw new Error("expected operation success");
  }
});

Then(
  /^(\w+) holds all organization admin permissions in "([^"]+)"$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const state = orgState(world as OrganizationWorld);
    const org = state.organizationsByName.get(
      normalizeOrganizationName(organizationName),
    );
    if (!org) {
      throw new Error(
        `organization ${organizationName} must exist before permission assertions`,
      );
    }

    const actual = [...org.grantedPermissions].sort();
    const expected = [...ALL_ADMIN_PERMISSIONS].sort();
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(
        `expected admin permissions ${expected.join(",")}, got ${actual.join(",")}`,
      );
    }

    await signInActorViaUi(page, world as OrganizationWorld, name);
    for (const permission of ALL_ADMIN_PERMISSIONS) {
      await openOrganizationWireSurface(page);
      await page
        .getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionOrgIdInput)
        .fill(org.id);
      await page
        .getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionSelect)
        .selectOption(permission);
      const previousSequence = await readWireSequence(page);
      await page
        .getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionSubmit)
        .click();
      const outcome = await waitForWireOutcome(page, previousSequence);
      if (outcome.status !== "success") {
        throw new Error(
          `admin permission check failed for ${permission}: ${outcome.failureDetail ?? outcome.failureCode ?? "unknown"}`,
        );
      }
    }
  },
);

Then(
  /^organization "([^"]+)" has zero initial projects$/,
  async ({ world }, organizationName: string) => {
    const state = orgState(world as OrganizationWorld);
    const normalized = normalizeOrganizationName(organizationName);
    const snapshot = state.organizationsByName.get(normalized);
    if (!snapshot) {
      throw new Error(
        `organization ${organizationName} must exist before zero-project assertion`,
      );
    }

    if (snapshot.initialProjectCount !== 0) {
      throw new Error(
        `expected initial_project_count=0 for ${organizationName}, got ${String(snapshot.initialProjectCount)}`,
      );
    }
  },
);

Then(
  /^organization "([^"]+)" is listed for (\w+)$/,
  async ({ page }, organizationName: string, _name: string) => {
    await page
      .getByTestId(organizationRowTestId(organizationName))
      .waitFor({ state: "visible", timeout: 30_000 });
  },
);
