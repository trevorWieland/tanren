import { createBdd } from "playwright-bdd";

import { test, type ActorState, type WebWorld } from "./account.steps";

const { Given, When, Then } = createBdd(test);

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";

const ORG_PERMISSION = "org.members.view";
const PROJECT_ROLE_PERMISSION = "project.changes.review";
const PROJECT_CONSTRAINED_PERMISSION = "project.deploy.approve";
const ROLE_TEMPLATE_NAME = "release_manager";

interface PermissionsMemo {
  eventIdsBefore?: Set<string>;
}

interface SeedActorPermissionFixturesBody {
  account_email: string;
  organization_policy_reason: string;
}

interface PermissionEventsCheckpointResponse {
  event_ids: string[];
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

async function postJson<T>(path: string, body: unknown): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(
      `POST ${path} failed: ${response.status} ${await response.text()}`,
    );
  }
  const text = await response.text();
  if (text === "") {
    return undefined as T;
  }
  return JSON.parse(text) as T;
}

async function seedActorPermissionFixtures(
  payload: SeedActorPermissionFixturesBody,
): Promise<void> {
  await postJson("/test-hooks/permissions/fixtures/actor", payload);
}

async function snapshotPermissionEventsCheckpoint(): Promise<Set<string>> {
  const body = await postJson<PermissionEventsCheckpointResponse>(
    "/test-hooks/permissions/checkpoints/events",
    { limit: 200 },
  );
  return new Set(body.event_ids);
}

async function assertNoPermissionMutationEvents(
  eventIdsBefore: Set<string>,
): Promise<void> {
  await postJson(
    "/test-hooks/permissions/assertions/no-request-or-grant-events",
    {
      event_ids_before: [...eventIdsBefore],
      limit: 200,
    },
  );
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
    const resolved = await postJson<{ account_id: string }>(
      "/test-hooks/accounts/resolve",
      { account_email: target.email },
    );
    const response = await page.request.get(
      `${API_URL}/accounts/${resolved.account_id}/permissions`,
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
