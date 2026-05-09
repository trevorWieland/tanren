/* eslint-disable */

import { createBdd } from "playwright-bdd";

import { test } from "./account.steps";

const { Given, When, Then } = createBdd(test);

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";

const ORG_PERMISSION = "org.members.view";
const PROJECT_ROLE_PERMISSION = "project.changes.review";
const PROJECT_CONSTRAINED_PERMISSION = "project.deploy.approve";
const ROLE_TEMPLATE_NAME = "release_manager";
const ORG_SCOPE_ID = "11111111-1111-7111-8111-111111111139";
const PROJECT_SCOPE_ID = "22222222-2222-7222-8222-222222222239";
type ScopeKind = "organization" | "project";
type GrantSourceKind = "direct" | "role_template";
type ConstraintSource = "organization_policy" | "project_policy";

interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
}

interface PermissionsMemo {
  eventIdsBefore?: Set<string>;
}

function actor(world: unknown, name: string): ActorState {
  const actors = (world as { actors: Map<string, ActorState> }).actors;
  const existing = actors.get(name) ?? {};
  if (!actors.has(name)) {
    actors.set(name, existing);
  }
  return existing;
}

function permissionsMemo(world: unknown): PermissionsMemo {
  const w = world as Record<string, unknown>;
  if (!w["__permissionsMemo"]) {
    w["__permissionsMemo"] = {};
  }
  return w["__permissionsMemo"] as PermissionsMemo;
}

type RecentEvent = { id: string; kind: string | null };

interface SeedPolicyConstraintPayload {
  reason: string;
  source: ConstraintSource;
}

interface SeedPermissionGrantPayload {
  account_email: string;
  scope_kind: ScopeKind;
  scope_id: string;
  permission: string;
  grant_source_kind: GrantSourceKind;
  role_template_name?: string;
  policy_constraint?: SeedPolicyConstraintPayload;
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

async function snapshotEventIds(): Promise<Set<string>> {
  const body = await postJson<{ events: RecentEvent[] }>(
    "/test-hooks/events/recent",
    {
      limit: 200,
    },
  );
  return new Set(body.events.map((event) => event.id));
}

async function seedPermissionGrant(
  payload: SeedPermissionGrantPayload,
): Promise<void> {
  await postJson("/test-hooks/permission-grants", payload);
}

async function signInActor(
  page: import("@playwright/test").Page,
  world: unknown,
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

    await seedPermissionGrant({
      account_email: a.email,
      scope_kind: "organization",
      scope_id: ORG_SCOPE_ID,
      permission: ORG_PERMISSION,
      grant_source_kind: "direct",
    });
    await seedPermissionGrant({
      account_email: a.email,
      scope_kind: "project",
      scope_id: PROJECT_SCOPE_ID,
      permission: PROJECT_ROLE_PERMISSION,
      grant_source_kind: "role_template",
      role_template_name: ROLE_TEMPLATE_NAME,
    });
    await seedPermissionGrant({
      account_email: a.email,
      scope_kind: "project",
      scope_id: PROJECT_SCOPE_ID,
      permission: PROJECT_CONSTRAINED_PERMISSION,
      grant_source_kind: "direct",
      policy_constraint: {
        reason,
        source: "organization_policy",
      },
    });
  },
);

When(
  /^([a-zA-Z]\w*) views their own permissions$/,
  async ({ page, world }, name: string) => {
    await signInActor(page, world, name);
    permissionsMemo(world).eventIdsBefore = await snapshotEventIds();
    await page.goto("/my-permissions");
    await waitForHydration(page);
    await page.getByRole("heading", { name: /my permissions/i }).waitFor();
  },
);

When(
  /^(\w+) attempts to view (\w+)'s permissions through the self view$/,
  async ({ page, world }, actorName: string, targetName: string) => {
    await signInActor(page, world, actorName);
    permissionsMemo(world).eventIdsBefore = await snapshotEventIds();

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
  "on a phone viewport the web permissions page shows the role-template source and constraint reason",
  async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/my-permissions");
    await waitForHydration(page);

    const roleTemplateVisible = await page
      .getByText(new RegExp(`role template\\s*:\\s*${ROLE_TEMPLATE_NAME}`, "i"))
      .first()
      .isVisible();
    if (!roleTemplateVisible) {
      throw new Error("role-template source is not visible on phone viewport");
    }

    const reasonVisible = await page
      .getByText(/organization policy requires approval ticket\./i)
      .first()
      .isVisible();
    if (!reasonVisible) {
      throw new Error("constraint reason is not visible on phone viewport");
    }

    const backLinkVisible = await page
      .getByRole("link", { name: /back to home/i })
      .isVisible();
    if (!backLinkVisible) {
      throw new Error(
        "permissions page controls are not usable on phone viewport",
      );
    }
  },
);

Then(
  "the permissions view does not create permission request or grant events",
  async ({ world }) => {
    const memo = permissionsMemo(world);
    if (!memo.eventIdsBefore) {
      throw new Error("missing pre-query event snapshot");
    }
    const response = await postJson<{ events: RecentEvent[] }>(
      "/test-hooks/events/recent",
      { limit: 200 },
    );
    const forbidden = response.events
      .filter((event) => !memo.eventIdsBefore?.has(event.id))
      .filter(
        (event) =>
          event.kind === "permission_requested" ||
          event.kind === "permission_granted",
      );
    if (forbidden.length > 0) {
      throw new Error(
        `permissions view created forbidden events: ${JSON.stringify(forbidden)}`,
      );
    }
  },
);

async function waitForHydration(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.waitForFunction(
    () => {
      const root = document as unknown as Record<string, unknown>;
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
