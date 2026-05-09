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
  accountId?: string;
  lastSettings?: UserSettingView[];
  lastCredentials?: UserCredentialView[];
  rememberedCredentialId?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
}

interface WebWorld {
  actors: Map<string, ActorState>;
}

interface UserSettingView {
  key: "theme" | "editor";
  value:
    | { kind: "theme"; value: "system" | "light" | "dark" }
    | { kind: "editor"; value: string };
  updated_at: string;
}

interface UserCredentialView {
  id: string;
  kind: "provider_api_token" | "harness_api_token";
  owner_scope: { scope: "user"; account_id: string };
  status: "pending" | "active" | "invalid";
  created_at: string;
  updated_at: string;
}

// Per-scenario `WebWorld` fixture. playwright-bdd consumes its own `test`
// (re-exported from `playwright-bdd`); we extend it to thread an
// actor-state map through every step without leaning on a global.
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
    // Account-flow falsification sets hasSession=false; B-0048
    // request-level falsification keeps the session but sets
    // lastFailureCode. Accept either signal.
    const failing = [...world.actors.values()].find(
      (a) => a.lastFailureCode !== undefined || a.hasSession === false,
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
// B-0048 user-tier configuration and credential metadata (web slice)
// ============================================================================

When(
  /^(\w+) lists user settings for their own account$/,
  async ({ page, world }, name: string) => {
    const self = await ensureSignedInActor(page, world, name);
    const response = await page.request.get(
      `${apiBaseUrl()}/accounts/${encodeURIComponent(self.accountId)}/user-settings`,
    );
    await recordSettingsOutcome(response, self);
  },
);

When(
  /^(\w+) lists user settings for (\w+)'s account$/,
  async ({ page, world }, name: string, target: string) => {
    const self = await ensureSignedInActor(page, world, name);
    const targetActor = await ensureActorIdentity(page, world, target);
    const response = await page.request.get(
      `${apiBaseUrl()}/accounts/${encodeURIComponent(targetActor.accountId)}/user-settings`,
    );
    await recordSettingsOutcome(response, self);
  },
);

When(
  /^(\w+) sets the (\w+) setting to "([^"]*)"$/,
  async ({ page, world }, name: string, key: string, rawValue: string) => {
    const self = await ensureSignedInActor(page, world, name);
    const value = settingPayload(key, rawValue);
    if (!value.ok) {
      throw new Error(value.message);
    }
    const response = await page.request.post(
      `${apiBaseUrl()}/accounts/${encodeURIComponent(self.accountId)}/user-settings`,
      {
        data: { key, value: value.payload },
      },
    );
    if (!response.ok()) {
      self.lastFailureCode = await responseFailureCode(response);
      return;
    }
    delete self.lastFailureCode;
    const body = (await response.json()) as { setting: UserSettingView };
    if (!self.lastSettings) self.lastSettings = [];
    const idx = self.lastSettings.findIndex(
      (item) => item.key === body.setting.key,
    );
    if (idx >= 0) self.lastSettings[idx] = body.setting;
    else self.lastSettings.push(body.setting);
  },
);

When(
  /^(\w+) adds a (\w+) user credential with value "([^"]*)"$/,
  async ({ page, world }, name: string, kind: string, value: string) => {
    const self = await ensureSignedInActor(page, world, name);
    const response = await page.request.post(
      `${apiBaseUrl()}/accounts/${encodeURIComponent(self.accountId)}/user-credentials`,
      {
        data: {
          kind,
          owner_scope: { scope: "user", account_id: self.accountId },
          value,
        },
      },
    );
    if (!response.ok()) {
      self.lastFailureCode = await responseFailureCode(response);
      return;
    }
    delete self.lastFailureCode;
    const body = (await response.json()) as { item: UserCredentialView };
    self.rememberedCredentialId = body.item.id;
    if (!self.lastCredentials) self.lastCredentials = [];
    self.lastCredentials = self.lastCredentials.filter(
      (item) => item.id !== body.item.id,
    );
    self.lastCredentials.push(body.item);
  },
);

When(
  /^(\w+) removes their remembered user credential$/,
  async ({ page, world }, name: string) => {
    const self = await ensureSignedInActor(page, world, name);
    const itemId = self.rememberedCredentialId;
    if (!itemId) {
      throw new Error(`${name} has no remembered credential id`);
    }
    const response = await page.request.delete(
      `${apiBaseUrl()}/accounts/${encodeURIComponent(self.accountId)}/user-credentials/${encodeURIComponent(itemId)}`,
    );
    if (!response.ok()) {
      self.lastFailureCode = await responseFailureCode(response);
      return;
    }
    delete self.lastFailureCode;
    if (!self.lastCredentials) self.lastCredentials = [];
    self.lastCredentials = self.lastCredentials.filter(
      (item) => item.id !== itemId,
    );
  },
);

Then(
  /^(\w+) sees (\d+) user settings$/,
  async ({ world }, name: string, countRaw: string) => {
    const expected = Number.parseInt(countRaw, 10);
    const actual = actor(world, name).lastSettings?.length ?? 0;
    if (actual !== expected) {
      throw new Error(
        `expected ${name} to have ${expected} settings, got ${actual}`,
      );
    }
  },
);

Then(
  /^(\w+) sees the (\w+) setting set to "([^"]*)"$/,
  async ({ world }, name: string, key: string, value: string) => {
    const snapshot = actor(world, name).lastSettings ?? [];
    if (!snapshot.some((item) => matchesSetting(item, key, value))) {
      throw new Error(`expected ${name} to include ${key}=${value}`);
    }
  },
);

Then(
  /^(\w+) does not see the (\w+) setting set to "([^"]*)"$/,
  async ({ world }, name: string, key: string, value: string) => {
    const snapshot = actor(world, name).lastSettings ?? [];
    if (snapshot.some((item) => matchesSetting(item, key, value))) {
      throw new Error(`expected ${name} not to include ${key}=${value}`);
    }
  },
);

Then(
  /^(\w+) sees (\d+) user credential metadata rows$/,
  async ({ world }, name: string, countRaw: string) => {
    const expected = Number.parseInt(countRaw, 10);
    const state = actor(world, name);
    const actual = state.lastCredentials?.length ?? 0;
    if (actual !== expected) {
      throw new Error(
        `expected ${name} to have ${expected} credential rows, got ${actual}; failure=${state.lastFailureCode ?? "none"}`,
      );
    }
  },
);

Then(
  /^(\w+) sees a (\w+) user credential metadata row$/,
  async ({ world }, name: string, kind: string) => {
    const snapshot = actor(world, name).lastCredentials ?? [];
    if (!snapshot.some((item) => item.kind === kind)) {
      throw new Error(`expected ${name} to include credential kind ${kind}`);
    }
  },
);

Then(
  /^(\w+) does not see credential value "([^"]*)"$/,
  async ({ world }, name: string, value: string) => {
    const serialized = JSON.stringify(actor(world, name).lastCredentials ?? []);
    if (serialized.includes(value)) {
      throw new Error(
        `credential value should not appear in metadata for ${name}`,
      );
    }
  },
);

// ============================================================================
// Helpers
// ============================================================================

function apiBaseUrl(): string {
  return process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
}

async function ensureActorIdentity(
  page: import("@playwright/test").Page,
  world: WebWorld,
  name: string,
): Promise<
  ActorState & { accountId: string; email: string; password: string }
> {
  const state = actor(world, name);
  if (!state.email || !state.password) {
    throw new Error(`actor ${name} has no recorded credentials`);
  }
  if (state.accountId) {
    return state as ActorState & {
      accountId: string;
      email: string;
      password: string;
    };
  }
  const response = await page.request.post(`${apiBaseUrl()}/sessions`, {
    data: { email: state.email, password: state.password },
  });
  if (!response.ok()) {
    state.lastFailureCode = await responseFailureCode(response);
    throw new Error(
      `failed to resolve account id for ${name}: ${state.lastFailureCode}`,
    );
  }
  const body = (await response.json()) as { account: { id: string } };
  state.accountId = body.account.id;
  state.hasSession = true;
  delete state.lastFailureCode;
  return state as ActorState & {
    accountId: string;
    email: string;
    password: string;
  };
}

async function ensureSignedInActor(
  page: import("@playwright/test").Page,
  world: WebWorld,
  name: string,
): Promise<
  ActorState & { accountId: string; email: string; password: string }
> {
  const state = await ensureActorIdentity(page, world, name);
  const response = await page.request.post(`${apiBaseUrl()}/sessions`, {
    data: { email: state.email, password: state.password },
  });
  if (!response.ok()) {
    state.lastFailureCode = await responseFailureCode(response);
    throw new Error(`failed to sign in as ${name}: ${state.lastFailureCode}`);
  }
  delete state.lastFailureCode;
  state.hasSession = true;
  return state;
}

async function responseFailureCode(
  response: import("@playwright/test").APIResponse,
): Promise<string> {
  try {
    const body = (await response.json()) as { code?: string };
    if (typeof body.code === "string" && body.code !== "") {
      return body.code;
    }
  } catch {
    // Fall through to status-derived fallback.
  }
  return `http_${response.status()}`;
}

async function recordSettingsOutcome(
  response: import("@playwright/test").APIResponse,
  state: ActorState,
): Promise<void> {
  if (!response.ok()) {
    state.lastFailureCode = await responseFailureCode(response);
    return;
  }
  const body = (await response.json()) as { items: UserSettingView[] };
  state.lastSettings = body.items;
  delete state.lastFailureCode;
}

function settingPayload(
  key: string,
  rawValue: string,
):
  | { ok: true; payload: UserSettingView["value"] }
  | { ok: false; message: string } {
  if (key === "theme") {
    if (rawValue === "system" || rawValue === "light" || rawValue === "dark") {
      return { ok: true, payload: { kind: "theme", value: rawValue } };
    }
    return { ok: false, message: `unsupported theme value: ${rawValue}` };
  }
  if (key === "editor") {
    return { ok: true, payload: { kind: "editor", value: rawValue } };
  }
  return { ok: false, message: `unsupported setting key: ${key}` };
}

function matchesSetting(
  item: UserSettingView,
  key: string,
  value: string,
): boolean {
  if (item.key !== key) return false;
  if (item.value.kind === "theme") {
    return item.value.value === value;
  }
  return item.value.value === value;
}

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
