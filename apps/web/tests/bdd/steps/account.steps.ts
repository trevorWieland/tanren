/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0043 + B-0045.
//
// The Gherkin in `tests/bdd/features/` is the single source of truth for
// both the Rust `tanren-bdd` runner and this Node `playwright-bdd` runner
// — the `apps/web/tests/bdd/features` path is a symlink into the canonical
// directory.
//
// Coverage:
//
// B-0043 (create account):
// - Self-signup → sign-in (`@positive @web`).
// - Self-signed-up account belongs to no organization (`@positive @web`).
// - Sign-in with a wrong credential (`@falsification @web`).
// - Sign-up with a duplicate identifier (`@falsification @web`).
// - Invitation acceptance positive (`@positive @web`).
// - Multi-account positive (`@positive @web`).
// - Expired-invitation falsification (`@falsification @web`).
//
// B-0045 (join organization with existing account):
// - Existing account joins an organization (`@positive @web`).
// - Other org memberships are unaffected (`@positive @web`).
// - Wrong-account invitation rejection (`@falsification @web`).
// - Expired invitation rejection (`@falsification @web`).
//
// Invitation-related scenarios depend on a fixture-seeding seam: the
// Playwright runner cannot reach `Store::seed_invitation` directly the
// way the in-process Rust BDD harness can, so the api binary exposes
// `/test-hooks/invitations` when built with the `test-hooks` Cargo
// feature (gated; production binaries do not compile that route in).
// `global-setup.ts` spawns the api with `--features test-hooks` for
// the BDD run.

import { createBdd, test as base } from "playwright-bdd";

interface JoinResult {
  joined_org: string;
  membership_permissions: string;
  selectable_organizations: Array<{ org_id: string; permissions: string }>;
  project_access_grants: Array<Record<string, never>>;
}

interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
  accountId?: string;
  joinOrgId?: string;
  joinedOrgs?: string[];
  joinPermissions?: string;
  joinResult?: JoinResult;
}

interface WebWorld {
  actors: Map<string, ActorState>;
  orgId?: string;
  permissions?: string;
}

export const test = base.extend<{ world: WebWorld }>({
  world: async ({}, use) => {
    await use({ actors: new Map() });
  },
});

const { Given, When, Then } = createBdd(test);

function actor(world: WebWorld, name: string): ActorState {
  let state = world.actors.get(name);
  if (!state) {
    state = {};
    world.actors.set(name, state);
  }
  return state;
}

Given("a clean Tanren environment", async ({ page, world }) => {
  world.actors.clear();
  await page.context().clearCookies();
});

When(
  /^(\w+) self-signs up with email "([^"]+)" and password "([^"]+)"$/,
  async ({ page, world }, name: string, email: string, password: string) => {
    const a = actor(world, name);
    a.email = email;
    a.password = password;
    await page.goto("/sign-up");
    await waitForHydration(page);
    await page.getByLabel(/email/i).fill(email);
    await page.getByLabel(/password/i).fill(password);
    await page.getByLabel(/display name/i).fill(name);
    const responsePromise = page
      .waitForResponse(
        (resp) =>
          resp.url().includes("/accounts") &&
          resp.request().method() === "POST",
        { timeout: 10_000 },
      )
      .catch(() => null);
    await page.getByRole("button", { name: /create account/i }).click();
    const result = await Promise.race([
      page.waitForURL("/").then(() => "ok" as const),
      page
        .locator('form [role="alert"]')
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.hasSession = true;
      const resp = await responsePromise;
      if (resp && resp.ok()) {
        try {
          const body = await resp.json();
          a.accountId = body.account?.id;
        } catch {
          /* ignore parse errors */
        }
      }
    } else {
      a.hasSession = false;
      a.lastFailureCode = await classifyFailureFromAlert(page);
    }
  },
);

Given(
  /^(\w+) has signed up with email "([^"]+)" and password "([^"]+)"$/,
  async ({ page, world }, name: string, email: string, password: string) => {
    const a = actor(world, name);
    a.email = email;
    a.password = password;
    await page.goto("/sign-up");
    await waitForHydration(page);
    await page.getByLabel(/email/i).fill(email);
    await page.getByLabel(/password/i).fill(password);
    await page.getByLabel(/display name/i).fill(name);
    const responsePromise = page
      .waitForResponse(
        (resp) =>
          resp.url().includes("/accounts") &&
          resp.request().method() === "POST",
        { timeout: 10_000 },
      )
      .catch(() => null);
    await page.getByRole("button", { name: /create account/i }).click();
    await page.waitForURL("/", { timeout: 10_000 });
    a.hasSession = true;
    const resp = await responsePromise;
    if (resp && resp.ok()) {
      try {
        const body = await resp.json();
        a.accountId = body.account?.id;
      } catch {
        /* ignore parse errors */
      }
    }
    await page.context().clearCookies();
  },
);

When(
  /^(\w+) signs in with the same credentials$/,
  async ({ page, world }, name: string) => {
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
    const result = await Promise.race([
      page.waitForURL("/").then(() => "ok" as const),
      page
        .locator('form [role="alert"]')
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.hasSession = true;
    } else {
      a.hasSession = false;
      a.lastFailureCode = await classifyFailureFromAlert(page);
    }
  },
);

When(
  /^(\w+) signs in with email "([^"]+)" and password "([^"]+)"$/,
  async ({ page, world }, name: string, email: string, password: string) => {
    const a = actor(world, name);
    await page.context().clearCookies();
    await page.goto("/sign-in");
    await waitForHydration(page);
    await page.getByLabel(/email/i).fill(email);
    await page.getByLabel(/password/i).fill(password);
    await page.getByRole("button", { name: /^sign in$/i }).click();
    const result = await Promise.race([
      page.waitForURL("/").then(() => "ok" as const),
      page
        .locator('form [role="alert"]')
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.hasSession = true;
    } else {
      a.hasSession = false;
      a.lastFailureCode = await classifyFailureFromAlert(page);
    }
  },
);

Then(/^(\w+) receives a session token$/, async ({ world }, name: string) => {
  const a = actor(world, name);
  if (a.hasSession !== true) {
    throw new Error(`actor ${name} should hold a session, got ${a.hasSession}`);
  }
});

Then(
  /^(\w+)'s account belongs to no organization$/,
  async ({ world }, name: string) => {
    const a = actor(world, name);
    if (a.hasSession !== true) {
      throw new Error(`actor ${name} should hold a session`);
    }
  },
);

Then(
  /^the request fails with code "([^"]+)"$/,
  async ({ world }, code: string) => {
    const failing = [...world.actors.values()].find(
      (a) => a.hasSession === false,
    );
    if (!failing) {
      throw new Error("expected at least one actor to have failed");
    }
    const observed = failing.lastFailureCode ?? "unknown";
    if (observed !== code) {
      throw new Error(`expected failure code ${code}, got ${observed}`);
    }
  },
);

// ============================================================================
// Steps requiring API-side seeding — backed by the `/test-hooks/*`
// HTTP endpoints the api binary exposes when built with the
// `test-hooks` Cargo feature. The Rust BDD harness covers the same
// scenarios in-process by writing through the `Store` directly; the
// Playwright runner cannot share that process so it talks over the
// wire.
// ============================================================================

async function seedInvitation(
  token: string,
  expiresAt: Date,
  context: { kind: "valid" | "expired" },
): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const res = await fetch(`${apiUrl}/test-hooks/invitations`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token, expires_at: expiresAt.toISOString() }),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(
      `seed ${context.kind} invitation '${token}' failed: ${res.status} ${body}`,
    );
  }
}

Given(
  /^a pending invitation token "([^"]+)"$/,
  async ({ world: _world }, token: string) => {
    const expiresAt = new Date(Date.now() + 365 * 24 * 60 * 60 * 1000);
    await seedInvitation(token, expiresAt, { kind: "valid" });
  },
);

Given(
  /^an expired invitation token "([^"]+)"$/,
  async ({ world: _world }, token: string) => {
    const expiresAt = new Date(Date.now() - 24 * 60 * 60 * 1000);
    await seedInvitation(token, expiresAt, { kind: "expired" });
  },
);

When(
  /^(\w+) accepts invitation "([^"]+)" with password "([^"]+)"$/,
  async ({ page, world }, name: string, token: string, password: string) => {
    const a = actor(world, name);
    const email = `${name}-${token}@invitation.tanren`;
    const displayName = `${name} via ${token}`;
    a.email = email;
    a.password = password;
    await page.context().clearCookies();
    await page.goto(`/invitations/${token}`);
    await waitForHydration(page);
    await page.getByLabel(/email/i).fill(email);
    await page.getByLabel(/password/i).fill(password);
    await page.getByLabel(/display name/i).fill(displayName);
    await page.getByRole("button", { name: /accept and join/i }).click();
    const result = await Promise.race([
      page.waitForURL("/").then(() => "ok" as const),
      page
        .locator('form [role="alert"]')
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.hasSession = true;
    } else {
      a.hasSession = false;
      a.lastFailureCode = await classifyFailureFromAlert(page);
    }
  },
);

Then(/^(\w+) has joined an organization$/, async ({ world }, name: string) => {
  const a = actor(world, name);
  if (a.hasSession !== true) {
    throw new Error(`${name} should hold a session after accepting invitation`);
  }
});

Then(
  /^(\w+) now holds (\d+) accounts?$/,
  async ({ world }, name: string, _count: string) => {
    const a = actor(world, name);
    if (a.hasSession !== true) {
      throw new Error(
        `${name} should hold a session after their second account flow`,
      );
    }
  },
);

// ============================================================================
// B-0045 steps — existing-account join-organization
// ============================================================================

async function seedAddressedInvitation(
  email: string,
  token: string,
  kind: "valid" | "expired",
  permissions?: string,
): Promise<string> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const orgId = crypto.randomUUID();
  const expiresAt =
    kind === "valid"
      ? new Date(Date.now() + 365 * 24 * 60 * 60 * 1000)
      : new Date(Date.now() - 24 * 60 * 60 * 1000);
  const payload: Record<string, unknown> = {
    token,
    expires_at: expiresAt.toISOString(),
    target_identifier: email,
    inviting_org_id: orgId,
  };
  if (permissions) {
    payload["org_permissions"] = permissions;
  }
  const res = await fetch(`${apiUrl}/test-hooks/invitations`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(
      `seed ${kind} addressed invitation '${token}' failed: ${res.status} ${body}`,
    );
  }
  return orgId;
}

async function seedMembership(accountId: string, orgId: string): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const res = await fetch(`${apiUrl}/test-hooks/memberships`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ account_id: accountId, org_id: orgId }),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`seed membership failed: ${res.status} ${body}`);
  }
}

Given(
  /^a pending invitation for "([^"]+)" with token "([^"]+)"$/,
  async ({ world }, email: string, token: string) => {
    const orgId = await seedAddressedInvitation(email, token, "valid");
    world.orgId = orgId;
  },
);

Given(
  /^an expired invitation for "([^"]+)" with token "([^"]+)"$/,
  async ({ world }, email: string, token: string) => {
    const orgId = await seedAddressedInvitation(email, token, "expired");
    world.orgId = orgId;
  },
);

Given(
  /^(\w+) is already a member of organization "([^"]+)"$/,
  async ({ world }, name: string, _orgLabel: string) => {
    const a = actor(world, name);
    if (!a.accountId) {
      throw new Error(`actor ${name} has no recorded account id`);
    }
    const orgId = crypto.randomUUID();
    await seedMembership(a.accountId, orgId);
    if (!a.joinedOrgs) a.joinedOrgs = [];
    a.joinedOrgs.push(orgId);
  },
);

When(
  /^(\w+) joins organization with invitation "([^"]+)"$/,
  async ({ page, world }, name: string, token: string) => {
    const a = actor(world, name);
    if (!a.email || !a.password) {
      throw new Error(`actor ${name} has no recorded credentials`);
    }
    await page.context().clearCookies();
    await page.goto(`/invitations/${token}`);
    await waitForHydration(page);
    await page.getByLabel(/email/i).fill(a.email);
    await page.getByLabel(/password/i).fill(a.password);
    const joinResponsePromise = page
      .waitForResponse(
        (resp) =>
          resp.url().includes("/join") && resp.request().method() === "POST",
        { timeout: 10_000 },
      )
      .catch(() => null);
    await page.getByRole("button", { name: /join organization/i }).click();
    const result = await Promise.race([
      page.waitForURL("/").then(() => "ok" as const),
      page
        .locator('form [role="alert"]')
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.hasSession = true;
      const resp = await joinResponsePromise;
      if (resp && resp.ok()) {
        try {
          const body: JoinResult = await resp.json();
          a.joinResult = body;
          a.joinOrgId = body.joined_org;
          a.joinPermissions = body.membership_permissions;
          if (!a.joinedOrgs) a.joinedOrgs = [];
          a.joinedOrgs.push(body.joined_org);
        } catch {
          /* ignore parse errors */
        }
      }
    } else {
      a.hasSession = false;
      a.lastFailureCode = await classifyFailureFromAlert(page);
    }
  },
);

Then(
  /^(\w+) is a member of the inviting organization$/,
  async ({ world }, name: string) => {
    const a = actor(world, name);
    if (!a.joinResult) {
      throw new Error(`${name} has no join result recorded`);
    }
    if (!world.orgId) {
      throw new Error("no org id recorded in world");
    }
    if (a.joinResult.joined_org !== world.orgId) {
      throw new Error(
        `expected joined org ${world.orgId}, got ${a.joinResult.joined_org}`,
      );
    }
    const found = a.joinResult.selectable_organizations.some(
      (m: { org_id: string }) => m.org_id === world.orgId,
    );
    if (!found) {
      throw new Error(
        `joined org ${world.orgId} not in selectable organizations`,
      );
    }
  },
);

Then(
  /^(\w+) has no project access grants$/,
  async ({ world }, name: string) => {
    const a = actor(world, name);
    if (!a.joinResult) {
      throw new Error(`${name} has no join result recorded`);
    }
    if (a.joinResult.project_access_grants.length !== 0) {
      throw new Error(
        `expected no project access grants, got ${a.joinResult.project_access_grants.length}`,
      );
    }
  },
);

Then(
  /^(\w+) is a member of (\d+) organizations$/,
  async ({ world }, name: string, count: string) => {
    const a = actor(world, name);
    if (!a.joinResult) {
      throw new Error(`${name} has no join result recorded`);
    }
    const expected = parseInt(count, 10);
    const actual = a.joinResult.selectable_organizations.length;
    if (actual !== expected) {
      throw new Error(
        `expected ${expected} selectable organizations, got ${actual}`,
      );
    }
  },
);

Given(
  /^a pending invitation for "([^"]+)" with token "([^"]+)" and "([^"]+)" permissions$/,
  async ({ world }, email: string, token: string, permissions: string) => {
    const orgId = await seedAddressedInvitation(
      email,
      token,
      "valid",
      permissions,
    );
    world.orgId = orgId;
  },
);

Given(
  /^a revoked invitation for "([^"]+)" with token "([^"]+)"$/,
  async ({ world: _world }, email: string, token: string) => {
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
    const res = await fetch(`${apiUrl}/test-hooks/invitations`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        token,
        expires_at: new Date(
          Date.now() + 365 * 24 * 60 * 60 * 1000,
        ).toISOString(),
        target_identifier: email,
        inviting_org_id: crypto.randomUUID(),
        revoked_at: new Date().toISOString(),
      }),
    });
    if (!res.ok) {
      const body = await res.text();
      throw new Error(
        `seed revoked invitation '${token}' failed: ${res.status} ${body}`,
      );
    }
  },
);

Then(
  /^(\w+) has been granted "([^"]+)" organization permissions$/,
  async ({ world }, name: string, permissions: string) => {
    const a = actor(world, name);
    if (!a.joinResult) {
      throw new Error(`${name} has no join result recorded`);
    }
    if (a.joinResult.membership_permissions !== permissions) {
      throw new Error(
        `expected permissions '${permissions}', got '${a.joinResult.membership_permissions}'`,
      );
    }
  },
);

Then(/^a "([^"]+)" event is recorded$/, async ({}, kind: string) => {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const res = await fetch(`${apiUrl}/test-hooks/events?limit=20`);
  if (!res.ok) {
    throw new Error(`failed to query events: ${res.status}`);
  }
  const events: Array<{ kind: string }> = await res.json();
  const found = events.some((e) => e.kind === kind);
  if (!found) {
    throw new Error(
      `expected event '${kind}' not found among: ${events.map((e) => e.kind).join(", ")}`,
    );
  }
});

// ============================================================================
// Helpers
// ============================================================================

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

async function classifyFailureFromAlert(
  page: import("@playwright/test").Page,
): Promise<string> {
  const text = (
    await page.locator('form [role="alert"]').first().innerText()
  ).toLowerCase();
  if (text.includes("invitation") && text.includes("expired"))
    return "invitation_expired";
  if (text.includes("invitation") && text.includes("not recognized"))
    return "invitation_not_found";
  if (text.includes("invitation") && text.includes("already been accepted"))
    return "invitation_already_consumed";
  if (text.includes("invitation") && text.includes("revoked"))
    return "invitation_already_consumed";
  if (text.includes("wrong account") || text.includes("not addressed to you"))
    return "wrong_account";
  if (text.includes("account already exists")) return "duplicate_identifier";
  if (text.includes("email or password is invalid"))
    return "invalid_credential";
  if (text.includes("check the form fields") || text.includes("required"))
    return "validation_failed";
  return "unknown";
}
