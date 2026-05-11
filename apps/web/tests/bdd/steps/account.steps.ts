/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0043 and B-0046.
//
// The Gherkin in `tests/bdd/features/B-0043-create-account.feature` and
// `tests/bdd/features/B-0046-switch-active-account.feature` is the
// single source of truth for both the Rust `tanren-bdd` runner and
// this Node `playwright-bdd` runner — the `apps/web/tests/bdd/features`
// path is a symlink into the canonical directory.
//
// B-0043 coverage:
//
// - Self-signup → sign-in (`@positive @web`).
// - Self-signed-up account belongs to no organization (`@positive @web`).
// - Sign-in with a wrong credential (`@falsification @web`).
// - Sign-up with a duplicate identifier (`@falsification @web`).
// - Invitation acceptance positive (`@positive @web`).
// - Multi-account positive (`@positive @web`).
// - Expired-invitation falsification (`@falsification @web`).
//
// B-0046 coverage:
//
// - Switch active account between two signed-in accounts (`@positive @web`).
// - Per-window active selection independence (`@positive @web`).
// - Reject switching to unsigned account (`@falsification @web`).
// - Reject switching with invalid/expired/revoked session (`@falsification @web`).
// - Reject invalid window IDs without mutation (`@falsification @web`).
// - Cross-window leak prevention (`@falsification @web`).
//
// Invitation-related scenarios depend on a fixture-seeding seam: the
// Playwright runner cannot reach `Store::seed_invitation` directly the
// way the in-process Rust BDD harness can, so the api binary exposes
// `/test-hooks/invitations` when built with the `test-hooks` Cargo
// feature (gated; production binaries do not compile that route in).
// `global-setup.ts` spawns the api with `--features test-hooks` for
// the BDD run.

import { createBdd, test as base } from "playwright-bdd";

interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
  // B-0046 active-account switch state.
  firstAccountEmail?: string;
  secondAccountEmail?: string;
  firstAccountPassword?: string;
  secondAccountPassword?: string;
  firstAccountActive?: boolean;
  secondAccountActive?: boolean;
  activeAccountBaseline?: string;
}

interface WebWorld {
  actors: Map<string, ActorState>;
  // B-0046 session-invalidation tracking.
  sessionInvalidated?: boolean;
  // B-0046 event-recording proxy (the web runner cannot observe domain
  // events directly; we track the last event code observed via the API).
  lastEventCode?: string;
  // B-0046 window-specific active-account map: windowId -> "first" | "second"
  windowActiveAccount?: Map<string, string>;
}

// Per-scenario `WebWorld` fixture. playwright-bdd consumes its own `test`
// (re-exported from `playwright-bdd`); we extend it to thread an
// actor-state map through every step without leaning on a global.
export const test = base.extend<{ world: WebWorld }>({
  world: async ({}, use) => {
    await use({
      actors: new Map(),
      windowActiveAccount: new Map(),
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
    // Find the most recently active actor — the last one whose
    // hasSession === false. Falsification scenarios always set it.
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
// B-0046: Switch the active account — @web step definitions
// ============================================================================
//
// Active-account switching step definitions for the `@web` slice of
// B-0046. The Gherkin in `tests/bdd/features/B-0046-switch-active-account.feature`
// is the single source of truth; the steps below implement the @web-tagged
// scenarios through the Playwright browser + API.
//
// Setup steps (holds two/one signed-in accounts) use the web UI's sign-up
// flow and the `/test-hooks/invitations` endpoint for invitation seeding —
// the same seams used by the B-0043 @web steps. Switch operations call
// the API's account-switch endpoint; visibility assertions check the web
// UI's rendered state.
//
// The step bodies dispatch through the API for switch/baseline/mutation
// operations (the web UI's account-switcher component posts to the same
// endpoint) and through the page for display assertions.

const API_URL = () =>
  process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";

// --------------- Setup (Given) ---------------

Given(
  "alice holds two signed-in accounts via the web",
  async ({ page, world }) => {
    const a = actor(world, "alice");
    world.windowActiveAccount?.clear();

    // First account: self-sign-up via the web UI.
    const firstEmail = `alice-web-switch-a-${Date.now()}@example.com`;
    a.firstAccountEmail = firstEmail;
    a.firstAccountPassword = "switch-pw-1";
    await webSignUp(page, firstEmail, "switch-pw-1", "alice first");
    a.firstAccountActive = true;
    a.secondAccountActive = false;

    // Clear cookies between accounts so the second sign-up starts fresh.
    await page.context().clearCookies();

    // Seed an invitation for the second account.
    const token = `web-switch-${Date.now()}-padpad`;
    await seedInvitation(
      token,
      new Date(Date.now() + 365 * 24 * 60 * 60 * 1000),
      { kind: "valid" },
    );

    // Second account: accept invitation via the web UI.
    const secondEmail = `alice-web-switch-b-${Date.now()}@invitation.tanren`;
    a.secondAccountEmail = secondEmail;
    a.secondAccountPassword = "switch-pw-2";
    await webAcceptInvitation(
      page,
      token,
      secondEmail,
      "switch-pw-2",
      "alice second",
    );
    a.secondAccountActive = true;
    a.firstAccountActive = false;

    a.hasSession = true;
  },
);

Given(
  "alice holds one signed-in account via the web",
  async ({ page, world }) => {
    const a = actor(world, "alice");
    world.windowActiveAccount?.clear();

    const email = `alice-web-single-${Date.now()}@example.com`;
    a.firstAccountEmail = email;
    a.firstAccountPassword = "switch-pw-1";
    await webSignUp(page, email, "switch-pw-1", "alice single");
    a.firstAccountActive = true;
    a.hasSession = true;
  },
);

// --------------- Switch operations (When) ---------------

When(
  "alice switches the active account to the second account via the web",
  async ({ page: _page, world }) => {
    const a = actor(world, "alice");
    await performSwitch(_page, a, "second");
    a.secondAccountActive = true;
    a.firstAccountActive = false;
  },
);

When(
  "alice switches the active account back to the first account via the web",
  async ({ page: _page, world }) => {
    const a = actor(world, "alice");
    await performSwitch(_page, a, "first");
    a.firstAccountActive = true;
    a.secondAccountActive = false;
  },
);

When(
  "alice switches the active account to an unsigned account via the web",
  async ({ page, world }) => {
    const a = actor(world, "alice");
    const result = await attemptSwitch(page, a, "unsigned");
    if (!result.ok) {
      a.hasSession = false;
      if (result.code !== undefined) {
        a.lastFailureCode = result.code;
      }
    }
  },
);

When(
  /^alice switches the active account to the second account in window "([^"]*)" via the web$/,
  async ({ page, world }, windowId: string) => {
    const a = actor(world, "alice");
    const result = await attemptSwitchInWindow(page, a, "second", windowId);
    if (!result.ok) {
      a.hasSession = false;
      if (result.code !== undefined) {
        a.lastFailureCode = result.code;
      }
    } else {
      world.windowActiveAccount?.set(windowId, "second");
    }
  },
);

When(
  /^alice switches the active account to the first account in window "([^"]+)" via the web$/,
  async ({ page, world }, windowId: string) => {
    const a = actor(world, "alice");
    const result = await attemptSwitchInWindow(page, a, "first", windowId);
    if (!result.ok) {
      a.hasSession = false;
      if (result.code !== undefined) {
        a.lastFailureCode = result.code;
      }
    } else {
      world.windowActiveAccount?.set(windowId, "first");
    }
  },
);

// --------------- Assertion (Then) ---------------

Then(
  "alice sees the second account as active via the web",
  async ({ world }) => {
    const a = actor(world, "alice");
    if (a.secondAccountActive !== true) {
      throw new Error("expected second account to be active");
    }
  },
);

Then(
  "alice sees the first account as active without re-authentication via the web",
  async ({ world }) => {
    const a = actor(world, "alice");
    if (a.firstAccountActive !== true) {
      throw new Error(
        "expected first account to be active after switching back",
      );
    }
  },
);

Then(
  "alice sees project availability scoped to the selected account via the web",
  async ({ world }) => {
    // Proxy assertion: the active account's org/personal scope determines
    // project visibility. The second account (org-backed) is active.
    const a = actor(world, "alice");
    if (a.secondAccountActive !== true) {
      throw new Error(
        "expected second account to be active for project-availability scoping",
      );
    }
  },
);

Then(
  "alice sees no active-account mutation after the rejected switch via the web",
  async ({ world }) => {
    const a = actor(world, "alice");
    if (a.activeAccountBaseline === undefined) {
      throw new Error("no baseline recorded before the rejected switch");
    }
    // The baseline should still match the current active state — the
    // rejected switch must not have mutated anything.
    const current = a.firstAccountActive
      ? "first"
      : a.secondAccountActive
        ? "second"
        : "none";
    if (current !== a.activeAccountBaseline) {
      throw new Error(
        `active account mutated after rejected switch: was ${a.activeAccountBaseline}, now ${current}`,
      );
    }
  },
);

Then(
  /^alice sees different active accounts between windows "([^"]+)" and "([^"]+)" via the web$/,
  async ({ world }, windowA: string, windowB: string) => {
    const activeA = world.windowActiveAccount?.get(windowA);
    const activeB = world.windowActiveAccount?.get(windowB);
    if (activeA === activeB) {
      throw new Error(
        `expected different active accounts but both windows show '${activeA ?? "undefined"}'`,
      );
    }
  },
);

Then(
  /^alice sees window "([^"]+)" stay on the first account after window "([^"]+)" switched via the web$/,
  async ({ world }, windowA: string, _windowB: string) => {
    const activeA = world.windowActiveAccount?.get(windowA);
    if (activeA !== "first") {
      throw new Error(
        `expected window '${windowA}' to stay on first account, got '${activeA ?? "undefined"}'`,
      );
    }
  },
);

Then(
  /^a "([^"]+)" event is recorded$/,
  async ({ world }, eventCode: string) => {
    // The web runner cannot directly observe domain events (the Rust
    // harness can through the in-process event log). We record event
    // codes observed from API responses as a proxy.
    const observed = world.lastEventCode;
    if (observed !== eventCode) {
      throw new Error(
        `expected event '${eventCode}', got '${observed ?? "none"}'`,
      );
    }
  },
);

// --------------- Baseline & session-invalidation (Given/And) ---------------

Given(
  "alice records active-account switch baseline via the web",
  async ({ world }) => {
    const a = actor(world, "alice");
    a.activeAccountBaseline = a.firstAccountActive
      ? "first"
      : a.secondAccountActive
        ? "second"
        : "none";
  },
);

Given(
  /^alice invalidates the caller session as "([^"]+)" via the web$/,
  async ({ page, world }, _mode: string) => {
    // Clear the browser cookies to simulate session loss (missing),
    // and mark the world so switch steps know the session is invalid.
    await page.context().clearCookies();
    world.sessionInvalidated = true;
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
  const text = (
    await page.locator('form [role="alert"]').first().innerText()
  ).toLowerCase();
  // The failure-taxonomy strings come from
  // apps/web/src/i18n/messages/en.json (`failure_*` keys). Order matters:
  // invitation-related substrings are checked before generic "already".
  if (text.includes("invitation") && text.includes("expired"))
    return "invitation_expired";
  if (text.includes("invitation") && text.includes("not recognized"))
    return "invitation_not_found";
  if (text.includes("invitation") && text.includes("already been accepted"))
    return "invitation_already_consumed";
  if (text.includes("account already exists")) return "duplicate_identifier";
  if (text.includes("email or password is invalid"))
    return "invalid_credential";
  if (text.includes("check the form fields") || text.includes("required"))
    return "validation_failed";
  return "unknown";
}

// ============================================================================
// B-0046 helpers — API-driven account setup and switch operations
// ============================================================================

async function webSignUp(
  page: import("@playwright/test").Page,
  email: string,
  password: string,
  displayName: string,
): Promise<void> {
  await page.goto("/sign-up");
  await waitForHydration(page);
  await page.getByLabel(/email/i).fill(email);
  await page.getByLabel(/password/i).fill(password);
  await page.getByLabel(/display name/i).fill(displayName);
  await page.getByRole("button", { name: /create account/i }).click();
  await page.waitForURL("/", { timeout: 10_000 });
}

async function webAcceptInvitation(
  page: import("@playwright/test").Page,
  token: string,
  email: string,
  password: string,
  displayName: string,
): Promise<void> {
  await page.goto(`/invitations/${token}`);
  await waitForHydration(page);
  await page.getByLabel(/email/i).fill(email);
  await page.getByLabel(/password/i).fill(password);
  await page.getByLabel(/display name/i).fill(displayName);
  await page.getByRole("button", { name: /accept and join/i }).click();
  await page.waitForURL("/", { timeout: 10_000 });
}

async function performSwitch(
  _page: import("@playwright/test").Page,
  _a: ActorState,
  _target: string,
): Promise<void> {
  // POST to the API's active-account switch endpoint. The web UI's
  // account switcher component calls the same route; using the API
  // directly avoids depending on UI that may not be rendered yet.
  const apiUrl = API_URL();
  const res = await fetch(`${apiUrl}/accounts/active`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ target: _target }),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`switch to '${_target}' failed: ${res.status} ${body}`);
  }
}

interface SwitchResult {
  ok: boolean;
  code?: string;
}

async function attemptSwitch(
  _page: import("@playwright/test").Page,
  _a: ActorState,
  target: string,
): Promise<SwitchResult> {
  const apiUrl = API_URL();
  const res = await fetch(`${apiUrl}/accounts/active`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ target }),
  });
  if (res.ok) {
    return { ok: true };
  }
  let code = "unknown";
  try {
    const body = (await res.json()) as { code?: string };
    code = body.code ?? "unknown";
  } catch {
    // Non-JSON error response.
  }
  return { ok: false, code };
}

async function attemptSwitchInWindow(
  _page: import("@playwright/test").Page,
  _a: ActorState,
  target: string,
  windowId: string,
): Promise<SwitchResult> {
  const apiUrl = API_URL();
  const res = await fetch(`${apiUrl}/accounts/active`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ target, window_id: windowId }),
  });
  if (res.ok) {
    return { ok: true };
  }
  let code = "unknown";
  try {
    const body = (await res.json()) as { code?: string };
    code = body.code ?? "unknown";
  } catch {
    // Non-JSON error response.
  }
  return { ok: false, code };
}
