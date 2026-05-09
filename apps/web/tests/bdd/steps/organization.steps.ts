// playwright-bdd step definitions for the `@web` slice of B-0066.
//
// These steps drive the web-owned organization wire surface (`/organizations`).

import { createBdd } from "playwright-bdd";

import {
  ORGANIZATION_WIRE_TEST_IDS,
  organizationRowTestId,
} from "@/lib/organization-routes";

import { signInActorViaUi } from "./api-client";
import { test } from "./account.steps";
import {
  actor,
  orgState,
  requireOrganizationWorld,
} from "./organization-world";
import {
  checkOrganizationPermissionViaWire,
  createOrganizationViaWire,
  listOrganizationsViaWire,
  openOrganizationWireSurface,
  organizationKey,
} from "./organization-wire";

const { Then, When } = createBdd(test);

When(
  /^(\w+) creates organization "([^"]+)"$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    await signInActorViaUi(page, typedWorld, name);

    const operation = await createOrganizationViaWire(page, organizationName);
    if (operation.outcome.status !== "success" || !operation.response.ok) {
      a.hasSession = false;
      a.lastFailureCode =
        operation.outcome.failureCode ??
        (operation.response.ok ? "unknown" : operation.response.error.code);
      state.lastOperationSucceeded = false;
      throw new Error(
        `create organization failed: ${
          operation.outcome.failureDetail ??
          (operation.response.ok ? "unknown" : operation.response.error.summary)
        }`,
      );
    }
    if (!operation.snapshot) {
      throw new Error(
        "organization snapshot missing after successful create operation",
      );
    }

    const normalized = organizationKey(organizationName);
    state.organizationsByName.set(normalized, operation.snapshot);
    state.lastCreateResponse = operation.response.body;
    state.lastListResponse = null;
    state.lastCheckResponse = null;
    state.lastOperationSucceeded = true;
    a.hasSession = true;
    delete a.lastFailureCode;
  },
);

When(
  /^(\w+) creates organization "([^"]+)" without signing in$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    await page.context().clearCookies();
    const operation = await createOrganizationViaWire(page, organizationName);

    if (operation.outcome.status === "success" && operation.response.ok) {
      a.hasSession = true;
      delete a.lastFailureCode;
      state.lastOperationSucceeded = true;
      return;
    }

    a.hasSession = false;
    a.lastFailureCode =
      operation.outcome.failureCode ??
      (operation.response.ok ? "unknown" : operation.response.error.code);
    state.lastOperationSucceeded = false;
  },
);

When(
  /^(\w+) lists available organizations$/,
  async ({ page, world }, name: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    await signInActorViaUi(page, typedWorld, name);
    const operation = await listOrganizationsViaWire(page);

    if (operation.outcome.status !== "success" || !operation.response.ok) {
      a.hasSession = false;
      a.lastFailureCode =
        operation.outcome.failureCode ??
        (operation.response.ok ? "unknown" : operation.response.error.code);
      state.lastOperationSucceeded = false;
      throw new Error(
        `list organizations failed: ${
          operation.outcome.failureDetail ??
          (operation.response.ok ? "unknown" : operation.response.error.summary)
        }`,
      );
    }

    state.lastListResponse = operation.response.body;
    state.lastCheckResponse = null;

    for (const organization of operation.response.body.organizations) {
      const key = organizationKey(organization.name);
      const prior = state.organizationsByName.get(key);
      state.organizationsByName.set(key, {
        id: organization.id,
        name: organization.name,
        grantedPermissions: prior?.grantedPermissions ?? [],
        initialProjectCount: prior?.initialProjectCount ?? null,
        proofLink: prior?.proofLink ?? null,
        sourceLink: prior?.sourceLink ?? null,
      });
    }

    state.lastOperationSucceeded = true;
    a.hasSession = true;
    delete a.lastFailureCode;
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
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    await signInActorViaUi(page, typedWorld, name);

    const org = state.organizationsByName.get(
      organizationKey(organizationName),
    );
    if (!org) {
      throw new Error(
        `organization ${organizationName} must be created or listed before permission checks`,
      );
    }

    const operation = await checkOrganizationPermissionViaWire(
      page,
      org.id,
      permission,
    );
    if (operation.outcome.status !== "success" || !operation.response.ok) {
      a.hasSession = false;
      a.lastFailureCode =
        operation.outcome.failureCode ??
        (operation.response.ok ? "unknown" : operation.response.error.code);
      state.lastOperationSucceeded = false;
      return;
    }

    state.lastCheckResponse = operation.response.body;
    state.lastOperationSucceeded = true;
    a.hasSession = true;
    delete a.lastFailureCode;
  },
);

Then("the operation succeeds", async ({ world }) => {
  const typedWorld = requireOrganizationWorld(world);
  const state = orgState(typedWorld);
  if (!state.lastOperationSucceeded) {
    throw new Error("expected operation success");
  }
});

Then(
  /^(\w+) holds all organization admin permissions in "([^"]+)"$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const org = state.organizationsByName.get(
      organizationKey(organizationName),
    );
    if (!org) {
      throw new Error(
        `organization ${organizationName} must exist before permission assertions`,
      );
    }

    const actual = [...new Set(org.grantedPermissions)].sort();
    const expected = [
      ...new Set(state.lastCreateResponse?.granted_permissions ?? []),
    ].sort();
    if (expected.length === 0) {
      throw new Error(
        "expected create response to include granted_permissions for admin checks",
      );
    }
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(
        `expected admin permissions ${expected.join(",")}, got ${actual.join(",")}`,
      );
    }

    await signInActorViaUi(page, typedWorld, name);
    for (const permission of expected) {
      const operation = await checkOrganizationPermissionViaWire(
        page,
        org.id,
        permission,
      );
      if (operation.outcome.status !== "success" || !operation.response.ok) {
        throw new Error(
          `admin permission check failed for ${permission}: ${
            operation.outcome.failureDetail ??
            (operation.response.ok
              ? "unknown"
              : operation.response.error.summary)
          }`,
        );
      }

      if (!operation.response.body.allowed) {
        throw new Error(
          `permission ${permission} should be granted but was denied`,
        );
      }
    }
  },
);

Then(
  /^organization "([^"]+)" has zero initial projects$/,
  async ({ world }, organizationName: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const snapshot = state.organizationsByName.get(
      organizationKey(organizationName),
    );
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
    await openOrganizationWireSurface(page);
    await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.listSubmit)
      .click({ timeout: 30_000 });
    await page
      .getByTestId(organizationRowTestId(organizationName))
      .waitFor({ state: "visible", timeout: 30_000 });
  },
);
