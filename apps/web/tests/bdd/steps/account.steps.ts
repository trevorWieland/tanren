/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0043 and R-0005.
//
// The Gherkin in `tests/bdd/features/B-0043-create-account.feature` is
// the single source of truth for both the Rust `tanren-bdd` runner and
// this Node `playwright-bdd` runner — the `apps/web/tests/bdd/features`
// path is a symlink into the canonical directory.
//
// Coverage:
//
// - Self-signup → sign-in (`@positive @web`).
// - Self-signed-up account belongs to no organization (`@positive @web`).
// - Sign-in with a wrong credential (`@falsification @web`).
// - Sign-up with a duplicate identifier (`@falsification @web`).
// - Invitation acceptance positive (`@positive @web`).
// - Multi-account positive (`@positive @web`).
// - Expired-invitation falsification (`@falsification @web`).
// - Organization invitation management (R-0005): create, list, revoke,
//   and recipient-visible invitation state.
//
// Invitation-related scenarios depend on a fixture-seeding seam: the
// Playwright runner cannot reach `Store::seed_invitation` directly the
// way the in-process Rust BDD harness can, so the api binary exposes
// `/test-hooks/invitations` when built with the `test-hooks` Cargo
// feature (gated; production binaries do not compile that route in).
// `global-setup.ts` spawns the api with `--features test-hooks` for
// the BDD run.

import { createBdd, test as base } from "playwright-bdd";
import { expect } from "@playwright/test";

interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
  lastInvitationToken?: string;
  accountId?: string;
  joinedOrgId?: string;
}

interface WebWorld {
  actors: Map<string, ActorState>;
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
    await page.getByRole("button", { name: /create account/i }).click();
    await page.waitForURL("/", { timeout: 10_000 });
    a.hasSession = true;
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
// Steps requiring API-side seeding
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
// Organization invitation management steps (R-0005)
// ============================================================================

async function seedOrgInvitation(params: {
  token: string;
  orgId: string;
  recipientIdentifier: string;
  permissions: string[];
  createdById: string;
  expiresAt: Date;
}): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const res = await fetch(`${apiUrl}/test-hooks/org-invitations`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      token: params.token,
      org_id: params.orgId,
      recipient_identifier: params.recipientIdentifier,
      permissions: params.permissions,
      created_by_account_id: params.createdById,
      expires_at: params.expiresAt.toISOString(),
    }),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(
      `seed org invitation '${params.token}' failed: ${res.status} ${body}`,
    );
  }
}

async function apiRevokeInvitation(
  orgId: string,
  token: string,
): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const res = await fetch(
    `${apiUrl}/organizations/${encodeURIComponent(orgId)}/invitations/${encodeURIComponent(token)}/revoke`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      credentials: "include",
    },
  );
  if (!res.ok) {
    const body = await res.text();
    throw new Error(
      `revoke invitation '${token}' failed: ${res.status} ${body}`,
    );
  }
}

Given(
  /^(\w+) has signed up and holds a session with email "([^"]+)" and password "([^"]+)"$/,
  async ({ page, world }, name: string, email: string, password: string) => {
    const a = actor(world, name);
    a.email = email;
    a.password = password;
    await page.goto("/sign-up");
    await waitForHydration(page);
    await page.getByLabel(/email/i).fill(email);
    await page.getByLabel(/password/i).fill(password);
    await page.getByLabel(/display name/i).fill(name);
    await page.getByRole("button", { name: /create account/i }).click();
    await page.waitForURL("/", { timeout: 10_000 });
    a.hasSession = true;
  },
);

When(
  /^(\w+) creates an org invitation for "([^"]+)" in org "([^"]+)" with permissions "([^"]+)"$/,
  async (
    { world },
    name: string,
    recipientIdentifier: string,
    orgId: string,
    permissionsCsv: string,
  ) => {
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
    const perms = permissionsCsv
      .split(",")
      .map((p) => p.trim())
      .filter((p) => p.length > 0);
    const expiresAt = new Date(Date.now() + 365 * 24 * 60 * 60 * 1000);
    const res = await fetch(
      `${apiUrl}/organizations/${encodeURIComponent(orgId)}/invitations`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          recipient_identifier: recipientIdentifier,
          permissions: perms,
          expires_at: expiresAt.toISOString(),
        }),
      },
    );
    if (!res.ok) {
      const body = await res.text();
      throw new Error(`create invitation failed: ${res.status} ${body}`);
    }
    const result = (await res.json()) as { invitation: { token: string } };
    const a = actor(world, name);
    a.lastInvitationToken = result.invitation.token;
  },
);

Given(
  /^a pending org invitation "([^"]+)" for org "([^"]+)" to "([^"]+)" with permissions "([^"]+)" created by "([^"]+)"$/,
  async (
    { world: _world },
    token: string,
    orgId: string,
    recipient: string,
    permsStr: string,
    createdById: string,
  ) => {
    const permissions = permsStr.split(",").map((p) => p.trim());
    const expiresAt = new Date(Date.now() + 365 * 24 * 60 * 60 * 1000);
    await seedOrgInvitation({
      token,
      orgId,
      recipientIdentifier: recipient,
      permissions,
      createdById,
      expiresAt,
    });
  },
);

Then(
  /^the recipient "([^"]+)" can see a pending invitation from org "([^"]+)"$/,
  async ({ world: _world }, recipientIdentifier: string, orgId: string) => {
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
    const res = await fetch(
      `${apiUrl}/invitations?recipient_identifier=${encodeURIComponent(recipientIdentifier)}`,
      {
        method: "GET",
        credentials: "include",
      },
    );
    if (!res.ok) {
      throw new Error(`list recipient invitations failed: ${res.status}`);
    }
    const result = (await res.json()) as {
      invitations: Array<{ org_id: string; status: string }>;
    };
    const found = result.invitations.find(
      (inv) => inv.org_id === orgId && inv.status === "pending",
    );
    if (!found) {
      throw new Error(
        `expected pending invitation from org ${orgId} for ${recipientIdentifier}, got ${JSON.stringify(result.invitations)}`,
      );
    }
  },
);

When(
  /^(\w+) navigates to the invitations page for org "([^"]+)"$/,
  async ({ page }, _name: string, orgId: string) => {
    await page.goto(`/organizations/${orgId}/invitations`);
    await waitForHydration(page);
  },
);

Then(
  /^(\w+) sees an invitation with status "([^"]+)" for "([^"]+)"$/,
  async (
    { page },
    _name: string,
    status: string,
    recipientIdentifier: string,
  ) => {
    const item = page
      .locator(`[data-invitation-status="${status}"]`)
      .filter({ hasText: recipientIdentifier });
    await item.waitFor({ state: "visible", timeout: 10_000 });
  },
);

When(
  /^(\w+) revokes the invitation for "([^"]+)" in org "([^"]+)"$/,
  async (
    { page },
    _name: string,
    recipientIdentifier: string,
    _orgId: string,
  ) => {
    const item = page
      .locator(`[data-invitation-status="pending"]`)
      .filter({ hasText: recipientIdentifier });
    const revokeBtn = item.getByRole("button", { name: /revoke/i });
    await revokeBtn.click();
  },
);

Then(
  /^(\w+) sees the invitation for "([^"]+)" is revoked$/,
  async ({ page }, _name: string, recipientIdentifier: string) => {
    const item = page
      .locator(`[data-invitation-status="revoked"]`)
      .filter({ hasText: recipientIdentifier });
    await item.waitFor({ state: "visible", timeout: 10_000 });
  },
);

Then(
  /^the invitation list shows a (\w+) invitation for "([^"]+)" with permission "([^"]+)"$/,
  async ({ page }, status: string, recipient: string, permission: string) => {
    const row = page.locator("li", { hasText: recipient });
    await row.waitFor({ state: "visible", timeout: 10_000 });
    const statusText =
      status === "pending"
        ? /pending/i
        : status === "revoked"
          ? /revoked/i
          : status;
    await expect(row.getByText(statusText)).toBeVisible();
    await expect(row.getByText(permission, { exact: true })).toBeVisible();
  },
);

When(
  /^(\w+) revokes invitation "([^"]+)" in org "([^"]+)" via the UI$/,
  async ({ page }, _name: string, _token: string, _orgId: string) => {
    const revokeButton = page.getByRole("button", { name: /revoke/i }).first();
    await revokeButton.waitFor({ state: "visible", timeout: 10_000 });
    await revokeButton.click();
    await expect(page.getByText(/revoked/i).first()).toBeVisible({
      timeout: 10_000,
    });
  },
);

When(
  /^the admin revokes invitation "([^"]+)" in org "([^"]+)" via the API$/,
  async ({ world: _world }, token: string, orgId: string) => {
    await apiRevokeInvitation(orgId, token);
  },
);

When(
  /^(\w+) looks up recipient invitations for "([^"]+)" on the home page$/,
  async ({ page }, _name: string, email: string) => {
    await page.goto("/");
    await waitForHydration(page);
    const input = page.getByPlaceholder(/email/i);
    await input.fill(email);
    await page.getByRole("button", { name: /lookup/i }).click();
  },
);

Then(
  /^the recipient sees a (\w+) invitation with permission "([^"]+)"$/,
  async ({ page }, status: string, permission: string) => {
    const statusText =
      status === "pending"
        ? /pending/i
        : status === "revoked"
          ? /revoked/i
          : status;
    await expect(page.getByText(statusText).first()).toBeVisible({
      timeout: 10_000,
    });
    await expect(
      page.getByText(permission, { exact: true }).first(),
    ).toBeVisible();
  },
);

Then(/^the recipient sees no invitations$/, async ({ page }) => {
  await expect(page.getByText(/no pending invitations/i)).toBeVisible({
    timeout: 10_000,
  });
});

Then(
  /^(\w+) holds permission "([^"]+)" in the joined organization$/,
  async ({ world: _world }, name: string, permission: string) => {
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
    const a = actor(_world, name);
    if (!a.email) {
      throw new Error(`actor ${name} has no email recorded`);
    }
    const lookupRes = await fetch(
      `${apiUrl}/test-hooks/account-by-email?email=${encodeURIComponent(a.email)}`,
    );
    if (!lookupRes.ok) {
      throw new Error(
        `account lookup for ${a.email} failed: ${lookupRes.status}`,
      );
    }
    const accountInfo = (await lookupRes.json()) as {
      account_id: string;
      org_id: string | null;
    };
    const accountId = accountInfo.account_id;
    const orgId = accountInfo.org_id;
    if (!orgId) {
      throw new Error(
        `account ${accountId} for ${name} has no org — expected a joined org`,
      );
    }
    const permRes = await fetch(
      `${apiUrl}/test-hooks/membership-permissions?account_id=${encodeURIComponent(accountId)}&org_id=${encodeURIComponent(orgId)}`,
    );
    if (!permRes.ok) {
      throw new Error(
        `membership permissions lookup failed: ${permRes.status}`,
      );
    }
    const permResult = (await permRes.json()) as { permissions: string[] };
    const found = permResult.permissions.includes(permission);
    if (!found) {
      throw new Error(
        `expected permission '${permission}' on membership, got: ${JSON.stringify(permResult.permissions)}`,
      );
    }
  },
);

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
  if (text.includes("invitation") && text.includes("revoked"))
    return "invitation_revoked";
  if (text.includes("invitation") && text.includes("expired"))
    return "invitation_expired";
  if (text.includes("invitation") && text.includes("not recognized"))
    return "invitation_not_found";
  if (text.includes("invitation") && text.includes("already been accepted"))
    return "invitation_already_consumed";
  if (text.includes("permission")) return "permission_denied";
  if (text.includes("account already exists")) return "duplicate_identifier";
  if (text.includes("email or password is invalid"))
    return "invalid_credential";
  if (text.includes("check the form fields") || text.includes("required"))
    return "validation_failed";
  return "unknown";
}
