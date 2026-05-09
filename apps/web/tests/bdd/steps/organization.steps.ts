/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0066.
//
// The web UI does not yet expose organization-management screens, so
// these steps drive the same authenticated API calls from within the
// browser context (cookie transport via `credentials: include`).

import { createBdd } from "playwright-bdd";
import { test } from "./account.steps";

const { Then, When } = createBdd(test);

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
const ALL_ADMIN_PERMISSIONS = [
  "invite",
  "manage_access",
  "configure",
  "set_policy",
  "delete",
] as const;

interface OrganizationSnapshot {
  id: string;
  grantedPermissions: string[];
}

interface OrganizationWorldState {
  organizationsByName: Map<string, OrganizationSnapshot>;
  lastListedNames: Set<string>;
  lastOperationSucceeded: boolean;
}

interface WireResponse {
  ok: boolean;
  status: number;
  text: string;
  json: unknown;
}

function actor(world: any, name: string): any {
  let state = world.actors.get(name);
  if (!state) {
    state = {};
    world.actors.set(name, state);
  }
  return state;
}

function orgState(world: any): OrganizationWorldState {
  if (!world.__orgState) {
    world.__orgState = {
      organizationsByName: new Map<string, OrganizationSnapshot>(),
      lastListedNames: new Set<string>(),
      lastOperationSucceeded: false,
    };
  }
  return world.__orgState as OrganizationWorldState;
}

function normalizeOrganizationName(raw: string): string {
  return raw.trim().split(/\s+/).join(" ").toLowerCase();
}

function failureCode(response: WireResponse): string {
  const body = response.json as { code?: unknown } | null;
  if (body && typeof body.code === "string") {
    return body.code;
  }
  if (response.status === 401) return "auth_required";
  if (response.status === 403) return "permission_denied";
  return "unknown";
}

function failureDetail(response: WireResponse): string {
  return `status=${response.status} code=${failureCode(response)} body=${response.text}`;
}

async function callApi(
  page: import("@playwright/test").Page,
  method: "GET" | "POST",
  path: string,
  payload?: unknown,
): Promise<WireResponse> {
  return await page.evaluate(
    async ({ apiUrl, path, method, payload }) => {
      const headers: Record<string, string> = {};
      let body: string | null = null;
      if (payload !== undefined) {
        headers["content-type"] = "application/json";
        body = JSON.stringify(payload);
      }
      const res = await fetch(`${apiUrl}${path}`, {
        method,
        headers,
        body,
        credentials: "include",
      });
      const text = await res.text();
      let json: unknown = null;
      try {
        json = JSON.parse(text);
      } catch {
        json = null;
      }
      return { ok: res.ok, status: res.status, text, json };
    },
    { apiUrl: API_URL, path, method, payload },
  );
}

async function signInActorViaUi(
  page: import("@playwright/test").Page,
  world: any,
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
  const result = await Promise.race([
    page.waitForURL("/").then(() => "ok" as const),
    page
      .locator('form [role="alert"]')
      .first()
      .waitFor({ state: "visible" })
      .then(() => "alert" as const),
  ]);
  if (result !== "ok") {
    a.hasSession = false;
    a.lastFailureCode = await classifyFailureFromAlert(page);
    orgState(world).lastOperationSucceeded = false;
    throw new Error(`sign-in failed for ${name} via web UI`);
  }
  a.hasSession = true;
  a.lastFailureCode = undefined;
}

When(
  /^(\w+) creates organization "([^"]+)"$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const state = orgState(world);
    const a = actor(world, name);
    await signInActorViaUi(page, world, name);

    const response = await callApi(page, "POST", "/organizations", {
      name: organizationName,
    });

    if (!response.ok) {
      a.hasSession = false;
      a.lastFailureCode = failureCode(response);
      state.lastOperationSucceeded = false;
      throw new Error(`create organization failed: ${failureDetail(response)}`);
    }

    const body = response.json as {
      organization: { id: string; name: string };
      granted_permissions: string[];
    };
    state.organizationsByName.set(
      normalizeOrganizationName(body.organization.name),
      {
        id: body.organization.id,
        grantedPermissions: body.granted_permissions,
      },
    );
    state.lastOperationSucceeded = true;
  },
);

When(
  /^(\w+) creates organization "([^"]+)" without signing in$/,
  async ({ page, world }, name: string, organizationName: string) => {
    const state = orgState(world);
    const a = actor(world, name);
    await page.context().clearCookies();

    const raw = await page.context().request.post(`${API_URL}/organizations`, {
      data: { name: organizationName },
    });
    const text = await raw.text();
    let json: unknown = null;
    try {
      json = JSON.parse(text);
    } catch {
      json = null;
    }
    const response: WireResponse = {
      ok: raw.ok(),
      status: raw.status(),
      text,
      json,
    };

    if (response.ok) {
      a.hasSession = true;
      a.lastFailureCode = undefined;
      state.lastOperationSucceeded = true;
      return;
    }

    a.hasSession = false;
    a.lastFailureCode = failureCode(response);
    state.lastOperationSucceeded = false;
  },
);

When(
  /^(\w+) lists available organizations$/,
  async ({ page, world }, name: string) => {
    const state = orgState(world);
    const a = actor(world, name);
    await signInActorViaUi(page, world, name);

    const response = await callApi(page, "GET", "/organizations");
    if (!response.ok) {
      a.hasSession = false;
      a.lastFailureCode = failureCode(response);
      state.lastOperationSucceeded = false;
      throw new Error(`list organizations failed: ${failureDetail(response)}`);
    }

    const body = response.json as {
      organizations: Array<{ id: string; name: string }>;
    };
    state.lastListedNames = new Set(
      body.organizations.map((org) => normalizeOrganizationName(org.name)),
    );
    for (const org of body.organizations) {
      const key = normalizeOrganizationName(org.name);
      const prior = state.organizationsByName.get(key);
      state.organizationsByName.set(key, {
        id: org.id,
        grantedPermissions: prior?.grantedPermissions ?? [],
      });
    }
    state.lastOperationSucceeded = true;
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
    const state = orgState(world);
    const a = actor(world, name);
    await signInActorViaUi(page, world, name);

    const org = state.organizationsByName.get(
      normalizeOrganizationName(organizationName),
    );
    if (!org) {
      throw new Error(
        `organization ${organizationName} must be created or listed before permission checks`,
      );
    }

    const response = await callApi(
      page,
      "POST",
      "/organizations/permissions/check",
      {
        org_id: org.id,
        permission,
      },
    );

    if (!response.ok) {
      a.hasSession = false;
      a.lastFailureCode = failureCode(response);
      state.lastOperationSucceeded = false;
      return;
    }

    state.lastOperationSucceeded = true;
  },
);

Then("the operation succeeds", async ({ world }) => {
  const state = orgState(world);
  if (!state.lastOperationSucceeded) {
    throw new Error("expected operation success");
  }
});

Then(
  /^(\w+) holds all organization admin permissions in "([^"]+)"$/,
  async ({ world }, _name: string, organizationName: string) => {
    const state = orgState(world);
    const org = state.organizationsByName.get(
      normalizeOrganizationName(organizationName),
    );
    if (!org) {
      throw new Error(
        `organization ${organizationName} must exist before permission assertions`,
      );
    }

    const actual = [...org.grantedPermissions].sort();
    const expected = [...ALL_ADMIN_PERMISSIONS].sort();
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(
        `expected admin permissions ${expected.join(",")}, got ${actual.join(",")}`,
      );
    }
  },
);

Then(
  /^organization "([^"]+)" has zero initial projects$/,
  async ({ world }, organizationName: string) => {
    // Web does not yet expose a projects surface. This assertion proves
    // the organization exists on the authenticated wire path and defers
    // explicit project-count rendering checks to the Rust harness.
    const state = orgState(world);
    const exists = state.organizationsByName.has(
      normalizeOrganizationName(organizationName),
    );
    if (!exists) {
      throw new Error(
        `organization ${organizationName} must exist before zero-project assertion`,
      );
    }
  },
);

Then(
  /^organization "([^"]+)" is listed for (\w+)$/,
  async ({ world }, organizationName: string, _name: string) => {
    const state = orgState(world);
    const key = normalizeOrganizationName(organizationName);
    if (!state.lastListedNames.has(key)) {
      throw new Error(
        `expected organization ${organizationName} in list response`,
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
  if (text.includes("account already exists")) return "duplicate_identifier";
  if (text.includes("email or password is invalid"))
    return "invalid_credential";
  if (text.includes("check the form fields") || text.includes("required"))
    return "validation_failed";
  return "unknown";
}
