import { createBdd } from "playwright-bdd";

import { test, type ActorState, type WebWorld } from "./account.steps";
import {
  assertNoPermissionMutationEvents,
  resolveAccountIdByEmail,
  seedActorPermissionFixtures,
  snapshotPermissionEventsCheckpoint,
} from "../support/test-hooks";

const { Given, When, Then } = createBdd(test);

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";

const ORG_PERMISSION = "org.members.view";
const PROJECT_ROLE_PERMISSION = "project.changes.review";
const PROJECT_CONSTRAINED_PERMISSION = "project.deploy.approve";
const ROLE_TEMPLATE_NAME = "release_manager";

interface PermissionsMemo {
  eventIdsBefore?: Set<string>;
}

const permissionsMemos = new WeakMap<WebWorld, PermissionsMemo>();

function actor(world: WebWorld, name: string): ActorState {
  let existing = world.actors.get(name);
  if (!existing) {
    existing = {};
    world.actors.set(name, existing);
  }
  return existing;
}

function permissionsMemo(world: WebWorld): PermissionsMemo {
  const existing = permissionsMemos.get(world);
  if (existing) {
    return existing;
  }
  const next: PermissionsMemo = {};
  permissionsMemos.set(world, next);
  return next;
}

async function signInActor(
  page: import("@playwright/test").Page,
  world: WebWorld,
  name: string,
): Promise<void> {
  const a = actor(world, name);
  if (!a.email || !a.password) {
    throw new Error(`actor ${name} has no recorded credentials`);
  }
  await page.context().clearCookies();
  await page.goto("/sign-in");
  await waitForHydration(page);
  await page.getByLabel(/email/i).fill(a.email);
  await page.getByLabel(/password/i).fill(a.password);
  await page.getByRole("button", { name: /^sign in$/i }).click();
  await page.waitForURL("/");
  a.hasSession = true;
}

Given(
  /^(\w+) has direct and role-template permissions with organization policy reason "([^"]+)"$/,
  async ({ world }, name: string, reason: string) => {
    const a = actor(world, name);
    if (!a.email) {
      throw new Error(
        `actor ${name} is missing an email; sign-up step must run first`,
      );
    }

    await seedActorPermissionFixtures({
      account_email: a.email,
      organization_policy_reason: reason,
    });
  },
);

When(
  /^([a-zA-Z]\w*) views their own permissions$/,
  async ({ page, world }, name: string) => {
    await signInActor(page, world, name);
    permissionsMemo(world).eventIdsBefore =
      await snapshotPermissionEventsCheckpoint();
    await page.goto("/my-permissions");
    await waitForHydration(page);
    await page.getByRole("heading", { name: /my permissions/i }).waitFor();
  },
);

When(
  /^(\w+) attempts to view (\w+)'s permissions through the self view$/,
  async ({ page, world }, actorName: string, targetName: string) => {
    await signInActor(page, world, actorName);
    permissionsMemo(world).eventIdsBefore =
      await snapshotPermissionEventsCheckpoint();

    const target = actor(world, targetName);
    if (!target.email) {
      throw new Error(`target actor ${targetName} has no recorded email`);
    }
    const resolvedAccountId = await resolveAccountIdByEmail(target.email);
    const response = await page.request.get(
      `${API_URL}/accounts/${resolvedAccountId}/permissions`,
      {
        headers: {
          cookie: await page
            .context()
            .cookies()
            .then((cookies) =>
              cookies.map((c) => `${c.name}=${c.value}`).join("; "),
            ),
        },
      },
    );

    const failingActor = actor(world, actorName);
    if (response.ok()) {
      failingActor.hasSession = false;
      failingActor.lastFailureCode = "unexpected_success";
      return;
    }
    const body = (await response.json()) as { code?: string };
    failingActor.hasSession = false;
    failingActor.lastFailureCode = body.code ?? "unknown";
  },
);

When(
  "an unauthenticated client views their own permissions",
  async ({ page, world }) => {
    await page.context().clearCookies();
    permissionsMemo(world).eventIdsBefore =
      await snapshotPermissionEventsCheckpoint();

    const response = await page.request.get(`${API_URL}/me/permissions`);
    const unauth = actor(world, "unauthenticated");
    if (response.ok()) {
      unauth.hasSession = false;
      unauth.lastFailureCode = "unexpected_success";
      return;
    }
    const body = (await response.json()) as { code?: string };
    unauth.hasSession = false;
    unauth.lastFailureCode = body.code ?? "unknown";
  },
);

Then(
  /^(\w+) sees organization and project permission sections$/,
  async ({ page }, _name: string) => {
    await page.getByRole("heading", { name: /organizations/i }).waitFor();
    await page.getByRole("heading", { name: /projects/i }).waitFor();
  },
);

Then(
  /^(\w+) sees the direct organization permission entry$/,
  async ({ page }) => {
    await page.getByText(ORG_PERMISSION, { exact: false }).first().waitFor();
    await page
      .getByText(/source/i)
      .first()
      .waitFor();
    await page
      .getByText(/direct/i)
      .first()
      .waitFor();
  },
);

Then(
  /^(\w+) sees the role-template project permission entry$/,
  async ({ page }) => {
    await page
      .getByText(PROJECT_ROLE_PERMISSION, { exact: false })
      .first()
      .waitFor();
    await page
      .getByText(new RegExp(`role template\\s*:\\s*${ROLE_TEMPLATE_NAME}`, "i"))
      .first()
      .waitFor();
  },
);

Then(
  /^(\w+) sees constrained permission reason "([^"]+)" from source "([^"]+)"$/,
  async ({ page }, _name: string, reason: string, source: string) => {
    await page
      .getByText(PROJECT_CONSTRAINED_PERMISSION, { exact: false })
      .first()
      .waitFor();
    await page.getByText(reason, { exact: false }).first().waitFor();
    await page
      .getByText(new RegExp(source.replace("_", " "), "i"), { exact: false })
      .first()
      .waitFor();
  },
);

Then(
  "the permissions page limit defaults to {int}",
  async ({ page }, expectedLimit: number) => {
    const cookieHeader = await page
      .context()
      .cookies()
      .then((cookies) => cookies.map((c) => `${c.name}=${c.value}`).join("; "));
    const response = await page.request.get(`${API_URL}/me/permissions`, {
      headers: { cookie: cookieHeader },
    });
    if (!response.ok()) {
      throw new Error(
        `expected /me/permissions to succeed, got ${response.status()}`,
      );
    }
    const body = (await response.json()) as {
      page?: { limit?: number };
      read_metadata?: {
        source?: string;
        generated_at?: string;
        staleness?: string;
        source_checkpoint?: {
          max_permission_grant_id?: null | string;
          max_permission_constraint_id?: null | string;
        };
      };
    };
    const observed = body.page?.limit;
    if (observed !== expectedLimit) {
      throw new Error(
        `expected default page limit ${expectedLimit}, got ${String(observed)}`,
      );
    }
    const readMetadata = body.read_metadata;
    if (!readMetadata?.source) {
      throw new Error("expected read_metadata.source to be populated");
    }
    if (
      !readMetadata.generated_at ||
      Number.isNaN(Date.parse(readMetadata.generated_at))
    ) {
      throw new Error(
        `expected read_metadata.generated_at to be an RFC3339 timestamp, got ${String(readMetadata?.generated_at)}`,
      );
    }
    if (
      readMetadata.staleness !== "fresh" &&
      readMetadata.staleness !== "potentially_stale"
    ) {
      throw new Error(
        `expected read_metadata.staleness to be a known value, got ${String(readMetadata.staleness)}`,
      );
    }
    if (!readMetadata.source_checkpoint) {
      throw new Error("expected read_metadata.source_checkpoint to be present");
    }
    if (
      !readMetadata.source_checkpoint.max_permission_grant_id &&
      !readMetadata.source_checkpoint.max_permission_constraint_id
    ) {
      throw new Error(
        "expected read_metadata.source_checkpoint to contain at least one row id",
      );
    }
  },
);

Then(
  "on a phone viewport the web permissions page shows the role-template source, source proof references, and constraint reason",
  async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/my-permissions");
    await waitForHydration(page);

    const organizationPermission = page
      .getByText(ORG_PERMISSION, { exact: false })
      .first();
    const rolePermission = page
      .getByText(PROJECT_ROLE_PERMISSION, { exact: false })
      .first();
    const constrainedPermission = page
      .getByText(PROJECT_CONSTRAINED_PERMISSION, { exact: false })
      .first();
    const directSource = page.getByText(/^direct$/i).first();
    const roleTemplateSource = page
      .getByText(new RegExp(`role template\\s*:\\s*${ROLE_TEMPLATE_NAME}`, "i"))
      .first();
    const grantSourceReference = page
      .getByText(/permission_grant:/i, { exact: false })
      .first();
    const constraintSource = page.getByText(/^organization policy$/i).first();
    const constraintReason = page
      .getByText(/organization policy requires approval ticket\./i)
      .first();
    const constraintSourceReference = page
      .getByText(/permission_constraint:/i, { exact: false })
      .first();

    await organizationPermission.waitFor();
    await rolePermission.waitFor();
    await constrainedPermission.waitFor();
    await directSource.waitFor();
    await roleTemplateSource.waitFor();
    await grantSourceReference.waitFor();
    await constraintSource.waitFor();
    await constraintReason.waitFor();
    await constraintSourceReference.waitFor();

    const roleSourceBox = await requiredBox(
      roleTemplateSource,
      "role-template source",
    );
    const constraintReasonBox = await requiredBox(
      constraintReason,
      "constraint reason",
    );
    const grantSourceReferenceBox = await requiredBox(
      grantSourceReference,
      "grant source reference",
    );
    const constraintSourceReferenceBox = await requiredBox(
      constraintSourceReference,
      "constraint source reference",
    );
    const orgPermissionBox = await requiredBox(
      organizationPermission,
      "organization permission",
    );
    const constrainedPermissionBox = await requiredBox(
      constrainedPermission,
      "constrained permission",
    );
    assertNoOverlap(
      roleSourceBox,
      constraintReasonBox,
      "role-template source",
      "constraint reason",
    );
    assertNoOverlap(
      grantSourceReferenceBox,
      constraintSourceReferenceBox,
      "grant source reference",
      "constraint source reference",
    );
    assertNoOverlap(
      orgPermissionBox,
      constrainedPermissionBox,
      "organization permission",
      "constrained permission",
    );
  },
);

Then(
  "the permissions view does not create permission request or grant events",
  async ({ world }) => {
    const memo = permissionsMemo(world);
    if (!memo.eventIdsBefore) {
      throw new Error("missing pre-query event snapshot");
    }
    await assertNoPermissionMutationEvents(memo.eventIdsBefore);
  },
);

async function waitForHydration(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.waitForFunction(
    () => {
      const root = document as Document & Record<string, unknown>;
      const keys = Object.keys(root).filter(
        (k) =>
          k.startsWith("__reactContainer") ||
          k.startsWith("_reactRootContainer"),
      );
      if (keys.length > 0) return true;
      return Array.from(document.querySelectorAll("*")).some((el) =>
        Object.keys(el).some((k) => k.startsWith("__reactProps$")),
      );
    },
    { timeout: 30_000 },
  );
}

type Box = { x: number; y: number; width: number; height: number };

async function requiredBox(
  locator: import("@playwright/test").Locator,
  label: string,
): Promise<Box> {
  const box = await locator.boundingBox();
  if (!box) {
    throw new Error(`${label} is not visible in the phone viewport`);
  }
  return box;
}

function assertNoOverlap(a: Box, b: Box, aLabel: string, bLabel: string): void {
  const overlap =
    a.x < b.x + b.width &&
    a.x + a.width > b.x &&
    a.y < b.y + b.height &&
    a.y + a.height > b.y;
  if (overlap) {
    throw new Error(`${aLabel} overlaps ${bLabel} on phone viewport`);
  }
}
