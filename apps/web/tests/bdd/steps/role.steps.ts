import { createBdd, test as base } from "playwright-bdd";

import type {
  PermissionScope,
  RoleFailureCode,
  RoleScope,
} from "../../../src/app/lib/generated/role-contract";
import { asOrgId } from "../../../src/app/lib/generated/role-contract";

interface PrincipalAccount {
  accountId: string;
  email: string;
  password: string;
}

interface RoleWorld {
  scope: RoleScope | undefined;
  activeRoleId: string | undefined;
  activeRoleName: string | undefined;
  principals: Map<string, PrincipalAccount>;
  operator: PrincipalAccount | undefined;
  lastPermissionCheck: boolean | undefined;
  lastErrorCode: string | undefined;
}

type RoleTest = ReturnType<typeof base.extend<{ world: RoleWorld }>>;

export const test: RoleTest = base.extend<{ world: RoleWorld }>({
  world: async ({ browserName: _browserName }, use) => {
    await use({
      scope: undefined,
      activeRoleId: undefined,
      activeRoleName: undefined,
      principals: new Map(),
      operator: undefined,
      lastPermissionCheck: undefined,
      lastErrorCode: undefined,
    });
  },
});

const { Given, When, Then } = createBdd(test);

const DENY_PROBE_PERMISSIONS = [
  "project.read",
  "project.comment",
  "project.merge",
  "project.audit",
] as const;

Given("a clean role-template environment", async ({ page, world }) => {
  world.scope = undefined;
  world.activeRoleId = undefined;
  world.activeRoleName = undefined;
  world.principals.clear();
  world.operator = undefined;
  world.lastPermissionCheck = undefined;
  world.lastErrorCode = undefined;
  await page.context().clearCookies();

  const operator = await signUpActorViaUi(page, {
    email: `role-operator-${crypto.randomUUID()}@tanren.test`,
    password: "role-operator-password",
    displayName: "Role Operator",
  });
  world.operator = operator;
});

Given("an organization role scope", async ({ page, world }) => {
  const scope: RoleScope = {
    scope: "organization",
    org_id: asOrgId(crypto.randomUUID()),
  };
  world.scope = scope;
  await seedRoleAdminGrants(requiredOperator(world).accountId, scope);
  await openRoleWorkbench(page);
});

When(
  "the operator creates role template {string} with permissions {string}",
  async ({ page, world }, name: string, permissions: string) => {
    const scope = roleScope(world);
    const createCard = roleCard(page, "Create role");

    await setScope(createCard, "scope_", scope);
    await createCard.locator('[name="name"]').fill(name);
    await createCard.locator('[name="permissions"]').fill(permissions);

    await submitRoleCard(page, createCard, {
      path: "/roles",
      label: "create role",
    });

    const summary = await readOperationSummary(page, "Create role");
    const roleId = summaryLine(summary, "role");
    if (roleId.length === 0) {
      throw new Error("create role summary should include role id");
    }

    world.activeRoleId = roleId;
    world.activeRoleName = name;
    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator edits the active role template to name {string} and permissions {string}",
  async ({ page, world }, name: string, permissions: string) => {
    const scope = roleScope(world);
    const editCard = roleCard(page, "Edit role");

    await editCard
      .locator('[name="role_id"]')
      .fill(requiredActiveRoleId(world));
    await setScope(editCard, "scope_", scope);
    await editCard.locator('[name="name"]').fill(name);
    await editCard.locator('[name="permissions"]').fill(permissions);

    await submitRoleCard(page, editCard, {
      path: "/roles/edit",
      label: "edit role",
    });

    const summary = await readOperationSummary(page, "Edit role");
    const roleId = summaryLine(summary, "role");
    if (roleId.length > 0) {
      world.activeRoleId = roleId;
    }
    world.activeRoleName = name;
    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator deletes the active role template",
  async ({ page, world }) => {
    const scope = roleScope(world);
    const deleteCard = roleCard(page, "Delete role");

    await deleteCard
      .locator('[name="role_id"]')
      .fill(requiredActiveRoleId(world));
    await setScope(deleteCard, "scope_", scope);

    await submitRoleCard(page, deleteCard, {
      path: "/roles/delete",
      label: "delete role",
    });

    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator applies the role template to account principal {word}",
  async ({ page, world }, alias: string) => {
    const scope = roleScope(world);
    const principal = await ensureScenarioPrincipalAccount(page, world, alias);
    const applyCard = roleCard(page, "Apply role");

    await applyCard
      .locator('[name="role_id"]')
      .fill(requiredActiveRoleId(world));
    await setScope(applyCard, "role_scope_", scope);
    await applyCard.locator('[name="principal_kind"]').selectOption("account");
    await applyCard.locator('[name="principal_id"]').fill(principal.accountId);
    await setPermissionScope(
      applyCard,
      "grant_scope_",
      permissionScopeFromRoleScope(scope),
    );

    await submitRoleCard(page, applyCard, {
      path: "/roles/apply",
      label: "apply role",
    });

    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator checks permission {string} for account principal {word}",
  async ({ page, world }, permission: string, alias: string) => {
    const principal = await ensureScenarioPrincipalAccount(page, world, alias);
    const result = await runPermissionCheck(page, {
      principalKind: "account",
      principalId: principal.accountId,
      permission,
      scope: permissionScopeFromRoleScope(roleScope(world)),
    });
    if (!result.ok) {
      throw new Error(
        `permission check failed (${result.code}): expected successful response`,
      );
    }
    world.lastPermissionCheck = result.allowed;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator checks permission {string} for the role template principal",
  async ({ page, world }, permission: string) => {
    const result = await runPermissionCheck(page, {
      principalKind: "role",
      principalId: requiredActiveRoleId(world),
      permission,
      scope: permissionScopeFromRoleScope(roleScope(world)),
    });

    if (result.ok) {
      world.lastPermissionCheck = result.allowed;
      world.lastErrorCode = "unexpected_success";
      return;
    }

    world.lastPermissionCheck = undefined;
    world.lastErrorCode = result.code;
  },
);

Then(
  "the active role template has permissions {string}",
  async ({ page, world }, permissions: string) => {
    const roleId = requiredActiveRoleId(world);
    const expected = parsePermissionsCsv(permissions);
    const actual = await readRoleTemplatePermissions(page, roleId);
    assertPermissionSetEqual(actual, expected);
  },
);

Then("the active role template no longer exists", async ({ page, world }) => {
  const roleId = requiredActiveRoleId(world);
  const roleTemplates = await readRoleTemplateLines(page);
  const hasDeletedRole = roleTemplates.some((line) =>
    line.includes(`(${roleId})`),
  );
  if (hasDeletedRole) {
    throw new Error("deleted role should not appear in the role read model");
  }

  const probePrincipal = await ensureScenarioPrincipalAccount(
    page,
    world,
    "deleted_role_probe",
  );
  const result = await runApplyRole(page, {
    roleId,
    roleScope: roleScope(world),
    principalAccountId: probePrincipal.accountId,
    grantScope: permissionScopeFromRoleScope(roleScope(world)),
  });
  if (result.ok) {
    throw new Error("expected deleted role apply to fail with not_found");
  }
  if (result.code !== "not_found") {
    throw new Error(
      `expected deleted role apply to fail with not_found, got ${result.code}`,
    );
  }
});

Then(
  "account principal {word} has direct grants {string}",
  async ({ page, world }, alias: string, permissions: string) => {
    await assertDirectGrantPermissions(page, world, alias, permissions);
  },
);

Then(
  "account principal {word} retains direct grants {string}",
  async ({ page, world }, alias: string, permissions: string) => {
    await assertDirectGrantPermissions(page, world, alias, permissions);
  },
);

Then(
  "the permission check result is {word}",
  async ({ world }, outcome: string) => {
    if (outcome !== "allowed" && outcome !== "denied") {
      throw new Error(`unknown permission check outcome: ${outcome}`);
    }
    const actual = world.lastPermissionCheck;
    if (actual === undefined) {
      throw new Error("permission check response is missing");
    }
    const expected = outcome === "allowed";
    if (actual !== expected) {
      throw new Error(
        `permission check mismatch: expected ${expected}, got ${actual}`,
      );
    }
  },
);

Then(
  "the role request fails with code {string}",
  async ({ world }, code: string) => {
    const actual = world.lastErrorCode ?? "no_error";
    if (actual !== code) {
      throw new Error(
        `unexpected role failure code: expected ${code}, got ${actual}`,
      );
    }
  },
);

function roleScope(world: RoleWorld): RoleScope {
  if (world.scope) {
    return world.scope;
  }
  const scope: RoleScope = {
    scope: "organization",
    org_id: asOrgId(crypto.randomUUID()),
  };
  world.scope = scope;
  return scope;
}

function permissionScopeFromRoleScope(scope: RoleScope): PermissionScope {
  if (scope.scope === "organization") {
    return { scope: "organization", org_id: scope.org_id };
  }
  if (scope.scope === "project") {
    return { scope: "project", project_id: scope.project_id };
  }
  return { scope: "account", account_id: scope.account_id };
}

function requiredActiveRoleId(world: RoleWorld): string {
  if (world.activeRoleId === undefined) {
    throw new Error("active role template must be created first");
  }
  return world.activeRoleId;
}

function requiredOperator(world: RoleWorld): PrincipalAccount {
  if (world.operator === undefined) {
    throw new Error("operator account is missing");
  }
  return world.operator;
}

function parsePermissionsCsv(raw: string): string[] {
  return normalizePermissions(raw.split(","));
}

function normalizePermissions(values: string[]): string[] {
  return values
    .map((value) => value.trim().toLowerCase())
    .filter((value) => value.length > 0);
}

function assertPermissionSetEqual(actual: string[], expected: string[]): void {
  const actualSet = new Set(actual);
  const expectedSet = new Set(expected);
  if (actualSet.size !== expectedSet.size) {
    throw new Error(
      `permission set size mismatch: expected ${expectedSet.size}, got ${actualSet.size}`,
    );
  }
  for (const permission of expectedSet) {
    if (!actualSet.has(permission)) {
      throw new Error(`missing permission: ${permission}`);
    }
  }
}

async function assertDirectGrantPermissions(
  page: import("@playwright/test").Page,
  world: RoleWorld,
  alias: string,
  permissionsCsv: string,
): Promise<void> {
  const expected = parsePermissionsCsv(permissionsCsv);
  const principal = await ensureScenarioPrincipalAccount(page, world, alias);
  const scope = permissionScopeFromRoleScope(roleScope(world));

  for (const permission of expected) {
    const result = await runPermissionCheck(page, {
      principalKind: "account",
      principalId: principal.accountId,
      permission,
      scope,
    });
    if (!result.ok) {
      throw new Error(
        `permission check failed (${result.code}) for ${alias}:${permission}`,
      );
    }
    if (!result.allowed) {
      throw new Error(
        `expected ${alias} to have permission ${permission}, but it was denied`,
      );
    }
  }

  for (const probe of DENY_PROBE_PERMISSIONS) {
    if (expected.includes(probe)) {
      continue;
    }
    const result = await runPermissionCheck(page, {
      principalKind: "account",
      principalId: principal.accountId,
      permission: probe,
      scope,
    });
    if (!result.ok) {
      throw new Error(
        `permission check failed (${result.code}) for ${alias}:${probe}`,
      );
    }
    if (result.allowed) {
      throw new Error(
        `expected ${alias} to be denied for non-granted permission ${probe}`,
      );
    }
  }
}

async function ensureScenarioPrincipalAccount(
  page: import("@playwright/test").Page,
  world: RoleWorld,
  alias: string,
): Promise<PrincipalAccount> {
  const existing = world.principals.get(alias);
  if (existing !== undefined) {
    return existing;
  }

  const email = `${alias}-${crypto.randomUUID()}@tanren.test`;
  const password = `${alias}-password`;
  const principal = await signUpActorViaUi(page, {
    email,
    password,
    displayName: `Principal ${alias}`,
  });

  world.principals.set(alias, principal);

  const operator = requiredOperator(world);
  await signInActorViaUi(page, {
    email: operator.email,
    password: operator.password,
  });
  await openRoleWorkbench(page);

  return principal;
}

async function signUpActorViaUi(
  page: import("@playwright/test").Page,
  input: { email: string; password: string; displayName: string },
): Promise<PrincipalAccount> {
  await page.context().clearCookies();
  await page.goto("/sign-up");
  await waitForHydration(page);

  await page.getByLabel(/email/i).fill(input.email);
  await page.getByLabel(/password/i).fill(input.password);
  await page.getByLabel(/display name/i).fill(input.displayName);

  const accountResponsePromise = waitForApiResponse(page, "/accounts");
  await page.getByRole("button", { name: /create account/i }).click();

  const accountResponse = await accountResponsePromise;
  const payload = (await accountResponse.json()) as {
    account?: { id?: string };
  };
  const accountId = payload.account?.id;
  if (typeof accountId !== "string" || accountId.length === 0) {
    throw new Error("sign-up response should include account id");
  }

  await page.waitForURL("/");

  return {
    accountId,
    email: input.email,
    password: input.password,
  };
}

async function signInActorViaUi(
  page: import("@playwright/test").Page,
  input: { email: string; password: string },
): Promise<void> {
  await page.context().clearCookies();
  await page.goto("/sign-in");
  await waitForHydration(page);

  await page.getByLabel(/email/i).fill(input.email);
  await page.getByLabel(/password/i).fill(input.password);

  const signInResponsePromise = waitForApiResponse(page, "/sessions");
  await page.getByRole("button", { name: /^sign in$/i }).click();
  await signInResponsePromise;
  await page.waitForURL("/");
}

async function openRoleWorkbench(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.goto("/");
  await waitForHydration(page);
  await roleCard(page, "Create role").waitFor({ state: "visible" });
}

async function seedRoleAdminGrants(
  actorAccountId: string,
  scope: RoleScope,
): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const response = await fetch(`${apiUrl}/test-hooks/role-admin-grants`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      actor_account_id: actorAccountId,
      scope,
      permissions: ["roles.manage", "roles.read"],
    }),
  });
  if (!response.ok) {
    throw new Error(
      `seed role admin grants failed: ${response.status} ${await response.text()}`,
    );
  }
}

function roleCard(
  page: import("@playwright/test").Page,
  title: string,
): import("@playwright/test").Locator {
  return page
    .locator("form")
    .filter({ has: page.getByRole("heading", { name: title, exact: true }) })
    .first();
}

function roleOperationSection(
  page: import("@playwright/test").Page,
): import("@playwright/test").Locator {
  return page
    .locator("section")
    .filter({
      has: page.getByRole("heading", {
        name: "Role operation result",
        exact: true,
      }),
    })
    .first();
}

function roleReadModelSection(
  page: import("@playwright/test").Page,
): import("@playwright/test").Locator {
  return page
    .locator("section")
    .filter({
      has: page.getByRole("heading", {
        name: "Role read model",
        exact: true,
      }),
    })
    .first();
}

async function setScope(
  card: import("@playwright/test").Locator,
  prefix: string,
  scope: RoleScope,
): Promise<void> {
  const kindField = card.locator(`[name="${prefix}kind"]`);
  const idField = card.locator(`[name="${prefix}id"]`);
  if (scope.scope === "organization") {
    await kindField.selectOption("organization");
    await idField.fill(scope.org_id);
    return;
  }
  if (scope.scope === "project") {
    await kindField.selectOption("project");
    await idField.fill(scope.project_id);
    return;
  }
  await kindField.selectOption("account");
  await idField.fill(scope.account_id);
}

async function setPermissionScope(
  card: import("@playwright/test").Locator,
  prefix: string,
  scope: PermissionScope,
): Promise<void> {
  const kindField = card.locator(`[name="${prefix}kind"]`);
  const idField = card.locator(`[name="${prefix}id"]`);
  if (scope.scope === "organization") {
    await kindField.selectOption("organization");
    await idField.fill(scope.org_id);
    return;
  }
  if (scope.scope === "project") {
    await kindField.selectOption("project");
    await idField.fill(scope.project_id);
    return;
  }
  await kindField.selectOption("account");
  await idField.fill(scope.account_id);
}

async function submitRoleCard(
  page: import("@playwright/test").Page,
  card: import("@playwright/test").Locator,
  input: { path: string; label: string },
): Promise<string> {
  const responsePromise = waitForApiResponse(page, input.path);
  await card.getByRole("button", { name: /^run$/i }).click();
  await responsePromise;
  return await waitForRoleMessage(page, input.label);
}

async function runApplyRole(
  page: import("@playwright/test").Page,
  input: {
    roleId: string;
    roleScope: RoleScope;
    principalAccountId: string;
    grantScope: PermissionScope;
  },
): Promise<{ ok: true } | { ok: false; code: string }> {
  const applyCard = roleCard(page, "Apply role");
  await applyCard.locator('[name="role_id"]').fill(input.roleId);
  await setScope(applyCard, "role_scope_", input.roleScope);
  await applyCard.locator('[name="principal_kind"]').selectOption("account");
  await applyCard
    .locator('[name="principal_id"]')
    .fill(input.principalAccountId);
  await setPermissionScope(applyCard, "grant_scope_", input.grantScope);

  const message = await submitRoleCard(page, applyCard, {
    path: "/roles/apply",
    label: "apply role",
  });
  if (message.endsWith(": ok")) {
    return { ok: true };
  }
  return {
    ok: false,
    code: parseRoleErrorCode(message, "apply role") ?? "transport_error",
  };
}

async function runPermissionCheck(
  page: import("@playwright/test").Page,
  input: {
    principalKind: "account" | "role";
    principalId: string;
    permission: string;
    scope: PermissionScope;
  },
): Promise<{ ok: true; allowed: boolean } | { ok: false; code: string }> {
  const checkCard = roleCard(page, "Check permission");

  await checkCard
    .locator('[name="principal_kind"]')
    .selectOption(input.principalKind);
  await checkCard.locator('[name="principal_id"]').fill(input.principalId);
  await checkCard.locator('[name="permission"]').fill(input.permission);
  await setPermissionScope(checkCard, "scope_", input.scope);

  const message = await submitRoleCard(page, checkCard, {
    path: "/permissions/check",
    label: "check permission",
  });

  if (!message.endsWith(": ok")) {
    return {
      ok: false,
      code:
        parseRoleErrorCode(message, "check permission") ?? "transport_error",
    };
  }

  const summary = await readOperationSummary(page, "Permission check");
  const allowedLine = summaryLine(summary, "allowed");
  if (allowedLine !== "true" && allowedLine !== "false") {
    throw new Error(
      `permission summary missing allowed flag, got '${allowedLine}'`,
    );
  }
  return { ok: true, allowed: allowedLine === "true" };
}

async function waitForRoleMessage(
  page: import("@playwright/test").Page,
  label: string,
): Promise<string> {
  const message = roleOperationSection(page).locator("p").first();
  const prefix = `${label}:`;
  for (let attempt = 0; attempt < 80; attempt += 1) {
    const text = ((await message.textContent()) ?? "").trim();
    if (text.startsWith(prefix)) {
      return text;
    }
    await page.waitForTimeout(100);
  }
  throw new Error(`timed out waiting for role operation message '${prefix}'`);
}

function parseRoleErrorCode(
  message: string,
  label: string,
): RoleFailureCode | "transport_error" | undefined {
  const prefix = `${label}:`;
  if (!message.startsWith(prefix)) {
    return undefined;
  }
  const rest = message.slice(prefix.length).trim();
  if (rest === "ok") {
    return undefined;
  }
  const firstColon = rest.indexOf(":");
  if (firstColon <= 0) {
    return "transport_error";
  }
  const code = rest.slice(0, firstColon).trim();
  return code.length === 0
    ? "transport_error"
    : (code as RoleFailureCode | "transport_error");
}

async function readOperationSummary(
  page: import("@playwright/test").Page,
  expectedLabel: string,
): Promise<{ label: string; lines: string[] }> {
  const section = roleOperationSection(page);
  const label = (
    (await section.locator("p").nth(1).textContent()) ?? ""
  ).trim();
  if (label !== expectedLabel) {
    throw new Error(
      `expected operation summary '${expectedLabel}', got '${label || "<empty>"}'`,
    );
  }
  const lines = (await section.locator("li").allTextContents()).map((line) =>
    line.trim(),
  );
  return { label, lines };
}

function summaryLine(summary: { lines: string[] }, prefix: string): string {
  const line = summary.lines.find((candidate) =>
    candidate.startsWith(`${prefix}: `),
  );
  if (line === undefined) {
    return "";
  }
  return line.slice(prefix.length + 2).trim();
}

async function readRoleTemplatePermissions(
  page: import("@playwright/test").Page,
  roleId: string,
): Promise<string[]> {
  const lines = await readRoleTemplateLines(page);
  const roleLine = lines.find((line) => line.includes(`(${roleId})`));
  if (roleLine === undefined) {
    throw new Error(`role ${roleId} was not found in the read model`);
  }

  const permissionsMarker = " permissions: ";
  const markerIndex = roleLine.indexOf(permissionsMarker);
  if (markerIndex === -1) {
    throw new Error(`role read-model line missing permissions: ${roleLine}`);
  }

  return parsePermissionsCsv(
    roleLine.slice(markerIndex + permissionsMarker.length),
  );
}

async function readRoleTemplateLines(
  page: import("@playwright/test").Page,
): Promise<string[]> {
  const section = roleReadModelSection(page);
  const allLines = (await section.locator("li").allTextContents()).map((line) =>
    line.trim(),
  );
  return allLines.filter((line) => line.includes(" permissions: "));
}

async function waitForApiResponse(
  page: import("@playwright/test").Page,
  path: string,
): Promise<import("@playwright/test").Response> {
  return await page.waitForResponse((response) => {
    if (response.request().method() !== "POST") {
      return false;
    }
    try {
      return new URL(response.url()).pathname === path;
    } catch {
      return false;
    }
  });
}

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
