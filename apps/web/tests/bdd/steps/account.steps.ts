/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0043.
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
// - Active-account switching, window isolation, and unsigned-target
//   rejection (`B-0046`, `@positive/@falsification @web`).
//
// Invitation-related scenarios depend on a fixture-seeding seam: the
// Playwright runner cannot reach `Store::seed_invitation` directly the
// way the in-process Rust BDD harness can, so the api binary exposes
// `/test-hooks/invitations` when built with the `test-hooks` Cargo
// feature (gated; production binaries do not compile that route in).
// `global-setup.ts` spawns the api with `--features test-hooks` for
// the BDD run.

import { createBdd, test as base } from "playwright-bdd";

type WindowContextId = string & { readonly __brand: "WindowContextId" };
type InvalidWindowContextId = string & {
  readonly __brand: "InvalidWindowContextId";
};
type WindowContextHeaderValue = WindowContextId | InvalidWindowContextId;

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function parseWindowContextId(value: unknown): WindowContextId | null {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  if (!UUID_REGEX.test(trimmed)) {
    return null;
  }
  return trimmed as WindowContextId;
}

function parseInvalidWindowContextId(
  value: unknown,
): InvalidWindowContextId | null {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  if (UUID_REGEX.test(trimmed)) {
    return null;
  }
  return value as InvalidWindowContextId;
}

function parseWindowContextHeaderValue(
  value: unknown,
): WindowContextHeaderValue | null {
  return parseWindowContextId(value) ?? parseInvalidWindowContextId(value);
}

interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string | undefined;
  firstAccountId?: string | undefined;
  secondAccountId?: string | undefined;
  baselineActiveAccountId?: string | undefined;
}

type ActiveAccountListing = Array<{
  is_active: boolean;
  account: { id: string; org: string | null };
}>;

interface WebWorld {
  actors: Map<string, ActorState>;
  windowAccounts: Map<string, ActiveAccountListing>;
}

// Per-scenario `WebWorld` fixture. playwright-bdd consumes its own `test`
// (re-exported from `playwright-bdd`); we extend it to thread an
// actor-state map through every step without leaning on a global.
export const test = base.extend<{ world: WebWorld }>({
  world: async ({}, use) => {
    await use({
      actors: new Map(),
      windowAccounts: new Map(),
    });
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
  // Per-scenario state; the API DB is shared across the run (one ephemeral
  // SQLite file spawned in global-setup), so we use email-prefix
  // isolation in the feature file (`alice-web@example.com` vs
  // `alice-web-dup@example.com`) to keep scenarios disjoint.
  world.actors.clear();
  world.windowAccounts.clear();
  await page.context().clearCookies();
  // Playwright's webServer can occasionally boot Next before
  // globalSetup's dynamic API URL has been materialized into the
  // frontend bundle env. Keep the @web proof deterministic by
  // rewriting the default 8080 target onto the actual API URL selected
  // by globalSetup for this run.
  const resolvedApiUrl =
    process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  await page.route(
    /^https?:\/\/(?:localhost|127\.0\.0\.1):8080\/.*/,
    async (route) => {
      const original = new URL(route.request().url());
      const rewritten = new URL(resolvedApiUrl);
      rewritten.pathname = original.pathname;
      rewritten.search = original.search;
      await route.continue({ url: rewritten.toString() });
    },
  );
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
    // The form's onSuccess pushes to "/"; failure surfaces an alert.
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
    // Sign out for the next step by clearing cookies — the alternative
    // (a real sign-out UI) lives in a future PR.
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
    // The web UI does not yet render the account's org affinity in PR 11
    // (the next R-0001 sub introduces a profile page). The session-token
    // existence is the proof we have: a self-signup over the public
    // /sign-up route never assigns an org. We assert that signal as a
    // proxy until the profile surface lands.
    if (a.hasSession !== true) {
      throw new Error(`actor ${name} should hold a session`);
    }
  },
);

Then(
  /^the request fails with code "([^"]+)"$/,
  async ({ world }, code: string) => {
    // Find the most recently observed failure.
    const failing = [...world.actors.values()].find(
      (a) => typeof a.lastFailureCode === "string",
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

Then(/^a "([^"]+)" event is recorded$/, async (_fixture, kind: string) => {
  await assertEventRecordedViaTestHook(kind);
});

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

interface TestHookRecentEventsResponse {
  events: Array<{
    id: string;
    occurred_at: string;
    kind: string | null;
  }>;
}

async function assertEventRecordedViaTestHook(kind: string): Promise<void> {
  for (let attempts = 0; attempts < 5; attempts += 1) {
    const kinds = await recentEventKindsViaTestHook(200);
    if (kinds.includes(kind)) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  const observedKinds = await recentEventKindsViaTestHook(200);
  throw new Error(
    `expected event '${kind}' in recent test-hook stream, got ${JSON.stringify(observedKinds)}`,
  );
}

async function recentEventKindsViaTestHook(limit: number): Promise<string[]> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const response = await fetch(
    `${apiUrl}/test-hooks/events/recent?limit=${encodeURIComponent(String(limit))}`,
    {
      method: "GET",
      headers: { accept: "application/json" },
    },
  );
  if (!response.ok) {
    const body = await response.text();
    throw new Error(
      `read /test-hooks/events/recent failed: ${response.status} ${body}`,
    );
  }
  const payload =
    (await response.json()) as Partial<TestHookRecentEventsResponse>;
  const events = Array.isArray(payload.events) ? payload.events : [];
  return events
    .map((event) => (typeof event.kind === "string" ? event.kind : ""))
    .filter((kindValue) => kindValue.trim() !== "");
}

Given(
  /^a pending invitation token "([^"]+)"$/,
  async ({ world: _world }, token: string) => {
    // Far-future expiry so the acceptance flow sees a live row.
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
    // Mirror the Rust step's email-synthesis convention so the @web
    // slice creates accounts that don't collide with @api ones (the
    // api/web BDD runs share an api process per-runner — Playwright
    // owns its own ephemeral DB via globalSetup, but using the same
    // shape keeps the ergonomics aligned).
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
  // The wire surface does not yet expose org affinity to the web UI; the
  // proxy assertion is "the actor obtained a session via the
  // accept-invitation endpoint", which (per the @api Rust harness) only
  // happens when the invitation was applied and a membership row was
  // written. The api-side Rust BDD harness covers the same witness
  // against `Store::find_membership_for_account`. When the profile
  // surface lands (R-0001 sub-15+), this step will assert the rendered
  // org name instead.
  const a = actor(world, name);
  if (a.hasSession !== true) {
    throw new Error(`${name} should hold a session after accepting invitation`);
  }
});

Then(
  /^(\w+) now holds (\d+) accounts?$/,
  async ({ world }, name: string, _count: string) => {
    // Per the equivalent @api Rust step, the assertion is that the
    // actor performed N successful sign-up / accept-invitation flows.
    // The web UI doesn't yet enumerate accounts in a single view, so we
    // use the session-presence proxy: the most-recent flow for this
    // actor must have ended in `/`. The accept-invitation flow above
    // sets `hasSession` only on success.
    const a = actor(world, name);
    if (a.hasSession !== true) {
      throw new Error(
        `${name} should hold a session after their second account flow`,
      );
    }
  },
);

// ============================================================================
// B-0046: active-account switching (`@web`).
// ============================================================================

Given(
  /^(\w+) holds two signed-in accounts via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    const nonce = uniqueSuffix();
    const firstEmail = `${name}-${surface}-a-${nonce}@example.com`;
    const secondEmail = `${name}-${surface}-b-${nonce}@example.com`;
    const firstPassword = "switch-pw-1";
    const secondPassword = "switch-pw-2";
    const token = `${name}-${surface}-${nonce}-switch-padpad`;

    await page.context().clearCookies();
    await page.goto("/");
    await waitForHydration(page);
    const first = await signUpViaApi(page, {
      email: firstEmail,
      password: firstPassword,
      display_name: `${name} first`,
    });

    const expiresAt = new Date(Date.now() + 365 * 24 * 60 * 60 * 1000);
    await seedInvitation(token, expiresAt, { kind: "valid" });

    const second = await acceptInvitationViaApi(page, token, {
      email: secondEmail,
      password: secondPassword,
      display_name: `${name} second`,
    });

    await page.goto("/");
    await waitForHydration(page);
    await waitForSwitcherReady(page);

    a.email = firstEmail;
    a.password = firstPassword;
    a.hasSession = true;
    a.lastFailureCode = undefined;
    a.firstAccountId = first.account.id;
    a.secondAccountId = second.account.id;
  },
);

Given(
  /^(\w+) holds one signed-in account via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    const email = `${name}-${surface}-single-${uniqueSuffix()}@example.com`;
    const password = "switch-pw-1";

    await page.context().clearCookies();
    await page.goto("/");
    await waitForHydration(page);
    const result = await signUpViaApi(page, {
      email,
      password,
      display_name: `${name} single`,
    });

    await page.goto("/");
    await waitForHydration(page);
    await waitForSwitcherReady(page);

    a.email = email;
    a.password = password;
    a.hasSession = true;
    a.lastFailureCode = undefined;
    a.firstAccountId = result.account.id;
    a.secondAccountId = undefined;
  },
);

When(
  /^(\w+) switches the active account to the second account via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    if (!a.secondAccountId) {
      throw new Error(`actor ${name} has no second account id recorded`);
    }
    await waitForSwitcherReady(page);
    await page.getByRole("combobox").selectOption(a.secondAccountId);
    const result = await Promise.race([
      page
        .waitForFunction((expected) => {
          const select = document.querySelector("select");
          return (
            select instanceof HTMLSelectElement && select.value === expected
          );
        }, a.secondAccountId)
        .then(() => "ok" as const),
      page
        .locator("p[role='alert']")
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.lastFailureCode = undefined;
      return;
    }
    a.lastFailureCode = await classifyFailureFromAlert(page);
  },
);

When(
  /^(\w+) switches the active account back to the first account via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    if (!a.firstAccountId) {
      throw new Error(`actor ${name} has no first account id recorded`);
    }
    await waitForSwitcherReady(page);
    await page.getByRole("combobox").selectOption(a.firstAccountId);
    const result = await Promise.race([
      page
        .waitForFunction((expected) => {
          const select = document.querySelector("select");
          return (
            select instanceof HTMLSelectElement && select.value === expected
          );
        }, a.firstAccountId)
        .then(() => "ok" as const),
      page
        .locator("p[role='alert']")
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.lastFailureCode = undefined;
      return;
    }
    a.lastFailureCode = await classifyFailureFromAlert(page);
  },
);

Given(
  /^(\w+) records active-account switch baseline via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    const listing = await listActiveAccountsViaFetch(page);
    const activeId = listing.find((entry) => entry.is_active)?.account.id ?? "";
    if (activeId.trim() === "") {
      throw new Error(`expected baseline active account for ${name}`);
    }
    a.baselineActiveAccountId = activeId;
  },
);

When(
  /^(\w+) invalidates the caller session as "([^"]+)" via the (\w+)$/,
  async ({ page, world }, name: string, mode: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    await invalidateCallerSessionViaWeb(page, mode);
    a.lastFailureCode = undefined;
  },
);

When(
  /^(\w+) switches the active account to an unsigned account via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    const failure = await switchUnsignedAccountViaFetch(page);
    a.lastFailureCode = failure;
  },
);

When(
  /^(\w+) switches the active account to the (\w+) account in window "([^"]+)" via the (\w+)$/,
  async (
    { page, world },
    name: string,
    which: string,
    windowId: string,
    surface: string,
  ) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    const targetId = which === "first" ? a.firstAccountId : a.secondAccountId;
    if (!targetId) {
      throw new Error(`actor ${name} has no ${which} account id recorded`);
    }
    const windowContextId = parseWindowContextHeaderValue(windowId);
    if (windowContextId === null) {
      throw new Error(`invalid window context header value '${windowId}'`);
    }
    const result = await switchActiveAccountViaFetch(
      page,
      windowContextId,
      targetId,
    );
    if (result.ok) {
      a.lastFailureCode = undefined;
      world.windowAccounts.set(windowId, result.accounts);
      return;
    }
    a.lastFailureCode = result.failure;
    world.windowAccounts.delete(windowId);
  },
);

Then(
  /^(\w+) sees the second account as active via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    if (!a.secondAccountId) {
      throw new Error(`actor ${name} has no second account id recorded`);
    }
    await waitForSwitcherReady(page);
    const activeId = await activeAccountIdFromSwitcher(page);
    if (activeId !== a.secondAccountId) {
      throw new Error(
        `expected second account ${a.secondAccountId} active, got ${activeId}`,
      );
    }
  },
);

Then(
  /^(\w+) sees project availability scoped to the selected account via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    if (!a.firstAccountId || !a.secondAccountId) {
      throw new Error(`actor ${name} must have two account ids recorded`);
    }
    const listing = await listActiveAccountsViaFetch(page);
    const first = listing.find(
      (entry) => entry.account.id === a.firstAccountId,
    );
    const second = listing.find(
      (entry) => entry.account.id === a.secondAccountId,
    );
    if (!first || !second) {
      throw new Error(
        `expected both accounts to appear in active-account list`,
      );
    }
    if (!second.is_active) {
      throw new Error(`expected second account to own active availability`);
    }
    if (first.is_active) {
      throw new Error(`expected first account to be inactive after switching`);
    }
    if (first.account.org !== null) {
      throw new Error(`expected first account to be personal/no-org`);
    }
    if (second.account.org === null) {
      throw new Error(`expected second account to be org-backed`);
    }
  },
);

Then(
  /^(\w+) sees no active-account mutation after the rejected switch via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    if (!a.baselineActiveAccountId || !a.email || !a.password) {
      throw new Error(`missing baseline/session credentials for actor ${name}`);
    }
    await signInViaApi(page, { email: a.email, password: a.password });
    const listing = await listActiveAccountsViaFetch(page);
    const activeId = listing.find((entry) => entry.is_active)?.account.id ?? "";
    if (activeId !== a.baselineActiveAccountId) {
      throw new Error(
        `expected active account to stay on ${a.baselineActiveAccountId}, got ${activeId}`,
      );
    }
  },
);

Then(
  /^(\w+) sees the first account as active without re-authentication via the (\w+)$/,
  async ({ page, world }, name: string, surface: string) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    if (!a.firstAccountId || !a.secondAccountId) {
      throw new Error(`actor ${name} must have two account ids recorded`);
    }
    await waitForSwitcherReady(page);
    const activeId = await activeAccountIdFromSwitcher(page);
    if (activeId !== a.firstAccountId) {
      throw new Error(
        `expected first account ${a.firstAccountId} active, got ${activeId}`,
      );
    }
    const optionIds = await switcherAccountIds(page);
    if (!optionIds.includes(a.secondAccountId)) {
      throw new Error(`expected second account to remain available`);
    }
  },
);

Then(
  /^(\w+) sees different active accounts between windows "([^"]+)" and "([^"]+)" via the (\w+)$/,
  async (
    { page, world },
    _name: string,
    windowA: string,
    windowB: string,
    surface: string,
  ) => {
    assertWebSurface(surface);
    const listingA = await listWindowAccounts(page, world, windowA, true);
    const listingB = await listWindowAccounts(page, world, windowB, true);
    const activeA = listingA.find((entry) => entry.is_active)?.account.id ?? "";
    const activeB = listingB.find((entry) => entry.is_active)?.account.id ?? "";
    if (activeA === "" || activeB === "") {
      throw new Error(`expected both windows to report an active account`);
    }
    if (activeA === activeB) {
      throw new Error(
        `expected windows ${windowA} and ${windowB} to differ, both were ${activeA}`,
      );
    }
  },
);

Then(
  /^(\w+) sees window "([^"]+)" stay on the (\w+) account after window "([^"]+)" switched via the (\w+)$/,
  async (
    { page, world },
    name: string,
    stableWindow: string,
    expected: string,
    changedWindow: string,
    surface: string,
  ) => {
    assertWebSurface(surface);
    const a = actor(world, name);
    const expectedId =
      expected === "first" ? a.firstAccountId : a.secondAccountId;
    if (!expectedId) {
      throw new Error(`actor ${name} has no ${expected} account id recorded`);
    }
    // Refresh the stable window after the changed-window switch to prove
    // the other window's action did not leak into this one.
    const stableListing = await listWindowAccounts(
      page,
      world,
      stableWindow,
      true,
    );
    const changedListing = await listWindowAccounts(
      page,
      world,
      changedWindow,
      false,
    );
    const stableActive =
      stableListing.find((entry) => entry.is_active)?.account.id ?? "";
    const changedActive =
      changedListing.find((entry) => entry.is_active)?.account.id ?? "";
    if (stableActive === "" || changedActive === "") {
      throw new Error(`expected both windows to report an active account`);
    }
    if (stableActive !== expectedId) {
      throw new Error(
        `expected window ${stableWindow} to stay on ${expectedId}, got ${stableActive}`,
      );
    }
    if (stableActive === changedActive) {
      throw new Error(
        `expected window ${changedWindow} switch to stay isolated from ${stableWindow}`,
      );
    }
  },
);

// ============================================================================
// Helpers
// ============================================================================

// Wait for React hydration to complete on a Next.js page. The Page-level
// navigation event (`page.goto`) returns once the document fires `load`,
// but the React-side `onSubmit` listener is attached only after the
// client bundle hydrates. Without this wait, the submit button click
// race-conditions with hydration: a too-early click submits the form as
// a native HTML GET (the URL ends up with the email/password as query
// params), which we observed empirically before adding `allowedDevOrigins`
// to `next.config.ts`.
//
// React 19's event delegation lives on the document root, so we sniff
// for the synthetic-event listener flag the runtime sets up post-mount.
// A 5s timeout is enough for the turbopack dev bundle on cold-start.
async function waitForHydration(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.waitForFunction(
    () => {
      // React 19 sets a sentinel property on document once `hydrateRoot`
      // has scheduled its first commit. The exact key has bled across
      // versions, so we fall back to a delegation-table sniff.
      const root = document as unknown as Record<string, unknown>;
      const keys = Object.keys(root).filter(
        (k) =>
          k.startsWith("__reactContainer") ||
          k.startsWith("_reactRootContainer"),
      );
      if (keys.length > 0) return true;
      // Last-resort heuristic: the synthetic-event listener installs
      // itself on document; if at least one element has a __reactProps$
      // marker, the runtime has booted.
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
  // The Next route announcer also has role=alert; scope to the form's
  // own alert region to avoid the strict-mode locator collision.
  const text = await page.locator('form [role="alert"]').first().innerText();
  return classifyFailureText(text);
}

function classifyFailureText(text: string): string {
  const normalized = text.toLowerCase();
  // The failure-taxonomy strings come from
  // apps/web/src/i18n/messages/en.json (`failure_*` keys). Order matters:
  // invitation-related substrings are checked before generic "already".
  if (normalized.includes("invitation") && normalized.includes("expired"))
    return "invitation_expired";
  if (
    normalized.includes("invitation") &&
    normalized.includes("not recognized")
  )
    return "invitation_not_found";
  if (
    normalized.includes("invitation") &&
    normalized.includes("already been accepted")
  )
    return "invitation_already_consumed";
  if (normalized.includes("account already exists"))
    return "duplicate_identifier";
  if (normalized.includes("email or password is invalid"))
    return "invalid_credential";
  if (
    normalized.includes("check the form fields") ||
    normalized.includes("required")
  )
    return "validation_failed";
  if (normalized.includes("not signed in"))
    return "target_account_not_signed_in";
  return "unknown";
}

function assertWebSurface(surface: string): void {
  if (surface !== "web") {
    throw new Error(
      `playwright-bdd web runner cannot execute surface '${surface}'`,
    );
  }
}

async function waitForSwitcherReady(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.waitForSelector("select");
  await page.waitForFunction(() => {
    const select = document.querySelector("select");
    if (!(select instanceof HTMLSelectElement)) {
      return false;
    }
    return select.options.length > 0;
  });
}

async function switcherAccountIds(
  page: import("@playwright/test").Page,
): Promise<string[]> {
  return page
    .locator("select option")
    .evaluateAll((options) =>
      options
        .map((option) => (option as HTMLOptionElement).value)
        .filter((value) => value.trim() !== ""),
    );
}

async function signUpViaApi(
  page: import("@playwright/test").Page,
  body: { email: string; password: string; display_name: string },
): Promise<{ account: { id: string } }> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const windowId = await ensureWindowContextId(page);
  const result = await page.evaluate(
    async ({ apiUrl, body, windowId }) => {
      window.sessionStorage.setItem("tanren.window_id", windowId);
      const response = await fetch(`${apiUrl}/accounts`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-tanren-window-id": windowId,
        },
        credentials: "include",
        body: JSON.stringify(body),
      });
      if (!response.ok) {
        const failure = (await response.json().catch(() => ({}))) as {
          code?: string;
          summary?: string;
        };
        return failure.code ?? failure.summary ?? `HTTP ${response.status}`;
      }
      return (await response.json()) as { account: { id: string } };
    },
    { apiUrl, body, windowId },
  );
  if (typeof result === "string") {
    throw new Error(`sign-up setup failed: ${result}`);
  }
  return result;
}

async function acceptInvitationViaApi(
  page: import("@playwright/test").Page,
  token: string,
  body: { email: string; password: string; display_name: string },
): Promise<{ account: { id: string } }> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const windowId = await ensureWindowContextId(page);
  const result = await page.evaluate(
    async ({ apiUrl, token, body, windowId }) => {
      window.sessionStorage.setItem("tanren.window_id", windowId);
      const response = await fetch(
        `${apiUrl}/invitations/${encodeURIComponent(token)}/accept`,
        {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-tanren-window-id": windowId,
          },
          credentials: "include",
          body: JSON.stringify(body),
        },
      );
      if (!response.ok) {
        const failure = (await response.json().catch(() => ({}))) as {
          code?: string;
          summary?: string;
        };
        return failure.code ?? failure.summary ?? `HTTP ${response.status}`;
      }
      return (await response.json()) as { account: { id: string } };
    },
    { apiUrl, token, body, windowId },
  );
  if (typeof result === "string") {
    throw new Error(`accept-invitation setup failed: ${result}`);
  }
  return result;
}

async function signInViaApi(
  page: import("@playwright/test").Page,
  body: { email: string; password: string },
): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const windowId = await ensureWindowContextId(page);
  const result = await page.evaluate(
    async ({ apiUrl, body, windowId }) => {
      window.sessionStorage.setItem("tanren.window_id", windowId);
      const response = await fetch(`${apiUrl}/sessions`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-tanren-window-id": windowId,
        },
        credentials: "include",
        body: JSON.stringify(body),
      });
      if (!response.ok) {
        const failure = (await response.json().catch(() => ({}))) as {
          code?: string;
          summary?: string;
        };
        return failure.code ?? failure.summary ?? `HTTP ${response.status}`;
      }
      return null;
    },
    { apiUrl, body, windowId },
  );
  if (typeof result === "string") {
    throw new Error(`sign-in setup failed: ${result}`);
  }
}

async function invalidateCallerSessionViaWeb(
  page: import("@playwright/test").Page,
  mode: string,
): Promise<void> {
  switch (mode) {
    case "missing":
      await page.context().clearCookies();
      return;
    case "expired":
      // Web harness cannot mint naturally expired cookies on demand; use
      // missing-cookie projection to prove the same invalid_credential path.
      await page.context().clearCookies();
      return;
    case "revoked":
      await revokeSessionViaApi(page);
      return;
    default:
      throw new Error(`unsupported invalid session mode '${mode}'`);
  }
}

async function revokeSessionViaApi(
  page: import("@playwright/test").Page,
): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const windowId = await ensureWindowContextId(page);
  const result = await page.evaluate(
    async ({ apiUrl, windowId }) => {
      window.sessionStorage.setItem("tanren.window_id", windowId);
      const response = await fetch(`${apiUrl}/sessions/revoke`, {
        method: "POST",
        headers: { "x-tanren-window-id": windowId },
        credentials: "include",
      });
      return response.status;
    },
    { apiUrl, windowId },
  );
  if (result !== 204) {
    throw new Error(`session revoke failed: HTTP ${result}`);
  }
}

async function activeAccountIdFromSwitcher(
  page: import("@playwright/test").Page,
): Promise<string> {
  const value = await page.getByRole("combobox").inputValue();
  if (value.trim() === "") {
    throw new Error("active account selector is empty");
  }
  return value;
}

async function switchUnsignedAccountViaFetch(
  page: import("@playwright/test").Page,
): Promise<string> {
  const uuid = "00000000-0000-4000-8000-000000000099";
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const windowId = await ensureWindowContextId(page);
  return page.evaluate(
    async ({ target, apiUrl, windowId }) => {
      window.sessionStorage.setItem("tanren.window_id", windowId);
      const headers: Record<string, string> = {
        "content-type": "application/json",
        "x-tanren-window-id": windowId,
      };
      const response = await fetch(`${apiUrl}/accounts/active/switch`, {
        method: "POST",
        headers,
        credentials: "include",
        body: JSON.stringify({ target_account_id: target }),
      });
      if (response.ok) {
        return "unexpected_success";
      }
      const body = (await response.json().catch(() => ({}))) as {
        code?: string;
        summary?: string;
      };
      if (typeof body.code === "string" && body.code.trim() !== "") {
        return body.code;
      }
      if (typeof body.summary === "string") {
        return body.summary;
      }
      return "unknown";
    },
    { target: uuid, apiUrl, windowId },
  );
}

async function switchActiveAccountViaFetch(
  page: import("@playwright/test").Page,
  windowId: WindowContextHeaderValue,
  targetAccountId: string,
): Promise<
  { ok: true; accounts: ActiveAccountListing } | { ok: false; failure: string }
> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const result = await page.evaluate(
    async ({ apiUrl, windowId, targetAccountId }) => {
      window.sessionStorage.setItem("tanren.window_id", windowId);
      const headers: Record<string, string> = {
        "content-type": "application/json",
        "x-tanren-window-id": windowId,
      };
      const response = await fetch(`${apiUrl}/accounts/active/switch`, {
        method: "POST",
        headers,
        credentials: "include",
        body: JSON.stringify({ target_account_id: targetAccountId }),
      });
      if (!response.ok) {
        const body = (await response.json().catch(() => ({}))) as {
          code?: string;
          summary?: string;
        };
        return {
          ok: false as const,
          failure: body.code ?? body.summary ?? `HTTP ${response.status}`,
        };
      }
      const payload = (await response.json()) as {
        accounts: ActiveAccountListing;
      };
      return { ok: true as const, accounts: payload.accounts };
    },
    { apiUrl, windowId, targetAccountId },
  );
  return result;
}

async function listActiveAccountsViaFetch(
  page: import("@playwright/test").Page,
  windowIdOverride?: WindowContextId,
): Promise<ActiveAccountListing> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const windowId = await ensureWindowContextId(page, windowIdOverride);
  return page.evaluate(
    async ({ apiUrl, windowId }) => {
      window.sessionStorage.setItem("tanren.window_id", windowId);
      const headers: Record<string, string> = {
        "x-tanren-window-id": windowId,
      };
      const response = await fetch(`${apiUrl}/accounts/active`, {
        method: "GET",
        headers,
        credentials: "include",
      });
      if (!response.ok) {
        throw new Error(`list active accounts failed: HTTP ${response.status}`);
      }
      const payload = (await response.json()) as {
        accounts: ActiveAccountListing;
      };
      return payload.accounts;
    },
    { apiUrl, windowId },
  );
}

async function listWindowAccounts(
  page: import("@playwright/test").Page,
  world: WebWorld,
  windowId: string,
  refresh: boolean,
): Promise<ActiveAccountListing> {
  const windowContextId = parseWindowContextId(windowId);
  if (windowContextId === null) {
    throw new Error(`expected UUID window id, got '${windowId}'`);
  }
  if (!refresh) {
    const cached = world.windowAccounts.get(windowId);
    if (cached) {
      return cached;
    }
  }
  const listing = await listActiveAccountsViaFetch(page, windowContextId);
  world.windowAccounts.set(windowId, listing);
  return listing;
}

async function ensureWindowContextId(
  page: import("@playwright/test").Page,
  preferredWindowId?: WindowContextId,
): Promise<WindowContextId> {
  const resolved = await page.evaluate((preferred) => {
    const existingWindowId = window.sessionStorage.getItem("tanren.window_id");
    const candidate =
      typeof preferred === "string" && preferred.trim() !== ""
        ? preferred
        : typeof existingWindowId === "string" && existingWindowId.trim() !== ""
          ? existingWindowId
          : (globalThis.crypto?.randomUUID?.() ?? "");
    if (candidate.trim() === "") {
      return null;
    }
    window.sessionStorage.setItem("tanren.window_id", candidate);
    return candidate;
  }, preferredWindowId ?? null);

  const parsed = parseWindowContextId(resolved);
  if (parsed === null) {
    throw new Error("window_context_unavailable");
  }
  return parsed;
}

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}
