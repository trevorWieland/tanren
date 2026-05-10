// playwright-bdd step definitions for the `@web` slice of B-0066.
//
// These steps drive the harness-owned B-0066 web witness surface
// (`/harness/b-0066/organizations`).

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
const ORGANIZATION_CREATE_BEHAVIOR_ID = "B-0066";
const ORGANIZATION_EVENT_FAMILY = "organization";
const ORGANIZATION_CREATED_EVENT_KIND = "organization_created";

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
    state.lastReplayExpectedOrganizationId = null;
    state.lastReplayObservedOrganizationId = null;
    state.lastCreateResponse = operation.response.body;
    state.lastListResponse = null;
    state.lastCheckResponse = null;
    state.lastOperationSucceeded = true;
    a.hasSession = true;
    delete a.lastFailureCode;
  },
);

When(
  /^(\w+) creates organization "([^"]+)" using idempotency key "([^"]+)"$/,
  async (
    { page, world },
    name: string,
    organizationName: string,
    idempotencyKey: string,
  ) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    await signInActorViaUi(page, typedWorld, name);
    const operation = await createOrganizationViaWire(
      page,
      organizationName,
      idempotencyKey,
    );
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
    state.createdOrganizationByIdempotencyKey.set(
      idempotencyKey,
      operation.snapshot.id,
    );
    state.lastReplayExpectedOrganizationId = null;
    state.lastReplayObservedOrganizationId = null;
    state.lastCreateResponse = operation.response.body;
    state.lastListResponse = null;
    state.lastCheckResponse = null;
    state.lastOperationSucceeded = true;
    a.hasSession = true;
    delete a.lastFailureCode;
  },
);

When(
  /^(\w+) attempts to create organization "([^"]+)" using idempotency key "([^"]+)"$/,
  async (
    { page, world },
    name: string,
    organizationName: string,
    idempotencyKey: string,
  ) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    await signInActorViaUi(page, typedWorld, name);
    const operation = await createOrganizationViaWire(
      page,
      organizationName,
      idempotencyKey,
    );

    if (operation.outcome.status === "success" && operation.response.ok) {
      if (!operation.snapshot) {
        throw new Error(
          "organization snapshot missing after successful create operation",
        );
      }
      const normalized = organizationKey(organizationName);
      state.organizationsByName.set(normalized, operation.snapshot);
      state.createdOrganizationByIdempotencyKey.set(
        idempotencyKey,
        operation.snapshot.id,
      );
      state.lastCreateResponse = operation.response.body;
      state.lastOperationSucceeded = true;
      a.hasSession = true;
      delete a.lastFailureCode;
      return;
    }

    state.lastOperationSucceeded = false;
    a.hasSession = false;
    a.lastFailureCode =
      operation.outcome.failureCode ??
      (operation.response.ok ? "unknown" : operation.response.error.code);
  },
);

When(
  /^(\w+) replays organization create "([^"]+)" using idempotency key "([^"]+)"$/,
  async (
    { page, world },
    name: string,
    organizationName: string,
    idempotencyKey: string,
  ) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    const expectedOrganizationId =
      state.createdOrganizationByIdempotencyKey.get(idempotencyKey);
    if (!expectedOrganizationId) {
      throw new Error(
        `idempotency key ${idempotencyKey} must be seeded by a prior successful create`,
      );
    }

    await signInActorViaUi(page, typedWorld, name);
    const operation = await createOrganizationViaWire(
      page,
      organizationName,
      idempotencyKey,
    );
    if (operation.outcome.status !== "success" || !operation.response.ok) {
      a.hasSession = false;
      a.lastFailureCode =
        operation.outcome.failureCode ??
        (operation.response.ok ? "unknown" : operation.response.error.code);
      state.lastOperationSucceeded = false;
      throw new Error(
        `idempotent replay failed: ${
          operation.outcome.failureDetail ??
          (operation.response.ok ? "unknown" : operation.response.error.summary)
        }`,
      );
    }
    if (!operation.snapshot) {
      throw new Error(
        "organization snapshot missing after successful replay operation",
      );
    }

    const normalized = organizationKey(organizationName);
    state.organizationsByName.set(normalized, operation.snapshot);
    state.createdOrganizationByIdempotencyKey.set(
      idempotencyKey,
      operation.snapshot.id,
    );
    state.lastReplayExpectedOrganizationId = expectedOrganizationId;
    state.lastReplayObservedOrganizationId = operation.snapshot.id;
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
  /^(\w+) lists available organizations without signing in$/,
  async ({ page, world }, name: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);

    await page.context().clearCookies();
    const operation = await listOrganizationsViaWire(page);
    state.lastListResponse = operation.response.ok
      ? operation.response.body
      : null;
    state.lastCheckResponse = null;

    if (operation.outcome.status === "success" && operation.response.ok) {
      state.lastOperationSucceeded = true;
      a.hasSession = true;
      delete a.lastFailureCode;
      return;
    }

    state.lastOperationSucceeded = false;
    a.hasSession = false;
    a.lastFailureCode =
      operation.outcome.failureCode ??
      (operation.response.ok ? "unknown" : operation.response.error.code);
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

When(
  /^(\w+) checks organization permission "([^"]+)" in "([^"]+)" without signing in$/,
  async (
    { page, world },
    name: string,
    permission: string,
    organizationName: string,
  ) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const a = actor(typedWorld, name);
    const org = state.organizationsByName.get(
      organizationKey(organizationName),
    );
    if (!org) {
      throw new Error(
        `organization ${organizationName} must be created or listed before permission checks`,
      );
    }

    await page.context().clearCookies();
    const operation = await checkOrganizationPermissionViaWire(
      page,
      org.id,
      permission,
    );

    if (operation.outcome.status === "success" && operation.response.ok) {
      state.lastCheckResponse = operation.response.body;
      state.lastOperationSucceeded = true;
      a.hasSession = true;
      delete a.lastFailureCode;
      return;
    }

    state.lastCheckResponse = null;
    state.lastOperationSucceeded = false;
    a.hasSession = false;
    a.lastFailureCode =
      operation.outcome.failureCode ??
      (operation.response.ok ? "unknown" : operation.response.error.code);
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
  /^organization "([^"]+)" is not listed for (\w+)$/,
  async ({ world }, organizationName: string, _name: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const listed = orgState(typedWorld).lastListResponse;
    if (!listed) {
      throw new Error("organization list response must be captured first");
    }

    const found = listed.organizations.some(
      (org) => organizationKey(org.name) === organizationKey(organizationName),
    );
    if (found) {
      throw new Error(
        `expected organization ${organizationName} to be hidden from non-member`,
      );
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
  /^organization "([^"]+)" exposes canonical proof and source links$/,
  async ({ world }, organizationName: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const snapshot = state.organizationsByName.get(
      organizationKey(organizationName),
    );
    if (!snapshot) {
      throw new Error(
        `organization ${organizationName} must exist before proof/source assertions`,
      );
    }

    if (!snapshot.proofLink || !snapshot.sourceLink) {
      throw new Error(
        `organization ${organizationName} is missing proof/source links`,
      );
    }

    if (snapshot.proofLink.behavior_id !== ORGANIZATION_CREATE_BEHAVIOR_ID) {
      throw new Error(
        `expected proof behavior_id=${ORGANIZATION_CREATE_BEHAVIOR_ID}, got ${snapshot.proofLink.behavior_id}`,
      );
    }
    if (snapshot.sourceLink.event_family !== ORGANIZATION_EVENT_FAMILY) {
      throw new Error(
        `expected source event_family=${ORGANIZATION_EVENT_FAMILY}, got ${snapshot.sourceLink.event_family}`,
      );
    }
    if (snapshot.sourceLink.event_kind !== ORGANIZATION_CREATED_EVENT_KIND) {
      throw new Error(
        `expected source event_kind=${ORGANIZATION_CREATED_EVENT_KIND}, got ${snapshot.sourceLink.event_kind}`,
      );
    }
  },
);

Then(
  /^idempotent replay for "([^"]+)" keeps the same organization id$/,
  async ({ world }, organizationName: string) => {
    const typedWorld = requireOrganizationWorld(world);
    const state = orgState(typedWorld);
    const expected = state.lastReplayExpectedOrganizationId;
    const observed = state.lastReplayObservedOrganizationId;
    if (!expected || !observed) {
      throw new Error(
        `idempotent replay for ${organizationName} must capture both expected and observed ids`,
      );
    }
    if (expected !== observed) {
      throw new Error(
        `idempotent replay changed organization id for ${organizationName}: expected ${expected}, got ${observed}`,
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
