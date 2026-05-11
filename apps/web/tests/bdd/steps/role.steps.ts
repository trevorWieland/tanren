import { createBdd, test as base } from "playwright-bdd";

import type {
  ApplyRoleResponse,
  PermissionCheckResponse,
  PermissionGrantId,
  PermissionGrantView,
  PermissionScope,
  PrincipalRef,
  RoleFailureCode,
  RoleScope,
  ScopedRole,
} from "../../../src/app/lib/generated/role-contract";
import {
  asAccountId,
  asOrgId,
  asRoleId,
  isRoleServerFailureCode,
  parseApplyRoleResponse,
  parsePermissionCheckResponse,
  parseRoleCapabilitySnapshot,
  parseRoleReadModelResponse,
} from "../../../src/app/lib/generated/role-contract";

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
  lastPermissionCheck: PermissionCheckResponse | undefined;
  lastErrorCode: string | undefined;
  grantIdsByAlias: Map<string, Map<string, PermissionGrantId>>;
  applyGrantSnapshots: Map<string, Array<Map<string, PermissionGrantId>>>;
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
      grantIdsByAlias: new Map(),
      applyGrantSnapshots: new Map(),
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
  world.grantIdsByAlias.clear();
  world.applyGrantSnapshots.clear();
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
  "the operator attempts to create role template {string} with {int} synthetic permissions",
  async ({ page, world }, name: string, permissionCount: number) => {
    const scope = roleScope(world);
    const createCard = roleCard(page, "Create role");

    await setScope(createCard, "scope_", scope);
    await createCard.locator('[name="name"]').fill(name);
    await createCard
      .locator('[name="permissions"]')
      .fill(syntheticPermissions(permissionCount).join(","));

    const message =
      permissionCount === 0
        ? await submitRoleCardWithoutRequest(page, createCard, "create role")
        : await submitRoleCard(page, createCard, {
            path: "/roles",
            label: "create role",
          });
    if (message.endsWith(": ok")) {
      const summary = await readOperationSummary(page, "Create role");
      const roleId = summaryLine(summary, "role");
      if (roleId.length > 0) {
        world.activeRoleId = roleId;
      }
      world.activeRoleName = name;
      world.lastErrorCode = "unexpected_success";
      world.lastPermissionCheck = undefined;
      return;
    }

    world.lastErrorCode =
      parseRoleErrorCode(message, "create role") ?? "transport_error";
    world.lastPermissionCheck = undefined;
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
    const result = await runApplyRole(page, {
      roleId: requiredActiveRoleId(world),
      roleScope: scope,
      principalAccountId: principal.accountId,
      grantScope: permissionScopeFromRoleScope(scope),
    });
    if (!result.ok) {
      throw new Error(`apply role failed (${result.code})`);
    }
    recordApplyGrantSnapshot(world, alias, result.response.grants);

    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator attempts to apply the role template to missing account principal {word}",
  async ({ page, world }, alias: string) => {
    const result = await runApplyRole(page, {
      roleId: requiredActiveRoleId(world),
      roleScope: roleScope(world),
      principalAccountId: missingPrincipalAccountId(alias),
      grantScope: permissionScopeFromRoleScope(roleScope(world)),
    });
    if (result.ok) {
      world.lastErrorCode = "unexpected_success";
      world.lastPermissionCheck = undefined;
      return;
    }
    world.lastErrorCode = result.code;
    world.lastPermissionCheck = undefined;
  },
);

When(
  "the operator attempts to apply the role template with an account grant-scope mismatch",
  async ({ page, world }) => {
    const scopeMismatch = await ensureScenarioPrincipalAccount(
      page,
      world,
      "scope_mismatch",
    );
    await seedRoleAdminGrants(requiredOperator(world).accountId, {
      scope: "account",
      account_id: asAccountId(scopeMismatch.accountId),
    });
    const result = await runApplyRole(page, {
      roleId: requiredActiveRoleId(world),
      roleScope: roleScope(world),
      principalAccountId: scopeMismatch.accountId,
      grantScope: {
        scope: "account",
        account_id: asAccountId(scopeMismatch.accountId),
      },
    });
    if (result.ok) {
      world.lastErrorCode = "unexpected_success";
      world.lastPermissionCheck = undefined;
      return;
    }
    world.lastErrorCode = result.code;
    world.lastPermissionCheck = undefined;
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
    world.lastPermissionCheck = result.response;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator checks permission {string} for missing account principal {word}",
  async ({ page, world }, permission: string, alias: string) => {
    const result = await runPermissionCheck(page, {
      principalKind: "account",
      principalId: missingPrincipalAccountId(alias),
      permission,
      scope: permissionScopeFromRoleScope(roleScope(world)),
    });
    if (result.ok) {
      world.lastPermissionCheck = result.response;
      world.lastErrorCode = "unexpected_success";
      return;
    }
    world.lastPermissionCheck = undefined;
    world.lastErrorCode = result.code;
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
      world.lastPermissionCheck = result.response;
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
    if (actual.allowed !== expected) {
      throw new Error(
        `permission check mismatch: expected ${expected}, got ${actual.allowed}`,
      );
    }
  },
);

Then(
  "the permission check matches direct grant ids for account principal {word} and permission {string}",
  async ({ world }, alias: string, permission: string) => {
    const check = world.lastPermissionCheck;
    if (check === undefined) {
      throw new Error("permission check response is missing");
    }
    const principal = world.principals.get(alias);
    if (principal === undefined) {
      throw new Error(`principal state missing for alias '${alias}'`);
    }
    if (check.principal.principal !== "account") {
      throw new Error(
        `permission check principal mismatch: expected account, got ${check.principal.principal}`,
      );
    }
    if (check.principal.account_id !== principal.accountId) {
      throw new Error(
        `permission check principal id mismatch: expected ${principal.accountId}, got ${check.principal.account_id}`,
      );
    }
    const expectedByPermission = world.grantIdsByAlias.get(alias);
    if (expectedByPermission === undefined) {
      throw new Error(`grant id state missing for alias '${alias}'`);
    }
    const expectedScope = permissionScopeFromRoleScope(roleScope(world));
    if (
      permissionScopeIdentity(check.scope) !==
      permissionScopeIdentity(expectedScope)
    ) {
      throw new Error(
        `permission check scope mismatch: expected ${permissionScopeIdentity(expectedScope)}, got ${permissionScopeIdentity(check.scope)}`,
      );
    }
    const normalizedPermission = normalizePermissions([permission])[0] ?? "";
    const expectedGrantId = expectedByPermission.get(normalizedPermission);
    if (expectedGrantId === undefined) {
      throw new Error(
        `no expected grant id tracked for ${alias}:${normalizedPermission}`,
      );
    }
    if (normalizePermissions([check.permission])[0] !== normalizedPermission) {
      throw new Error(
        `permission check mismatch: expected ${normalizedPermission}, got ${check.permission}`,
      );
    }
    if (!check.allowed) {
      throw new Error(
        "permission check should be allowed for granted permission",
      );
    }
    const actual = new Set(check.matching_grant_ids);
    if (actual.size !== 1 || !actual.has(expectedGrantId)) {
      throw new Error(
        `matching_grant_ids mismatch for ${alias}:${normalizedPermission}`,
      );
    }
  },
);

Then("the permission check has no matching grant ids", async ({ world }) => {
  const check = world.lastPermissionCheck;
  if (check === undefined) {
    throw new Error("permission check response is missing");
  }
  if (check.allowed) {
    throw new Error("denied permission checks must report allowed=false");
  }
  if (check.matching_grant_ids.length !== 0) {
    throw new Error(
      "denied permission checks must return empty matching_grant_ids",
    );
  }
});

Then(
  "applying the role template to account principal {word} is idempotent for permissions {string}",
  async ({ page, world }, alias: string, permissions: string) => {
    const snapshots = world.applyGrantSnapshots.get(alias);
    if (snapshots === undefined || snapshots.length < 2) {
      throw new Error(
        `idempotency witness requires two apply snapshots for alias '${alias}'`,
      );
    }
    const expected = parsePermissionsCsv(permissions);
    const previous = snapshots[snapshots.length - 2];
    const latest = snapshots[snapshots.length - 1];
    if (previous === undefined || latest === undefined) {
      throw new Error(
        `idempotency witness snapshots missing for alias '${alias}'`,
      );
    }
    assertGrantSnapshotMatchesPermissions(previous, expected, "first apply");
    assertGrantSnapshotMatchesPermissions(latest, expected, "second apply");
    assertGrantSnapshotEqual(previous, latest, "duplicate apply");
    await assertDirectGrantPermissions(page, world, alias, permissions);
    const direct = world.grantIdsByAlias.get(alias);
    if (direct === undefined) {
      throw new Error(`direct grant state missing for alias '${alias}'`);
    }
    assertGrantSnapshotEqual(
      direct,
      latest,
      "direct grants after duplicate apply",
    );
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

When(
  "{int} concurrent applications apply the role template to account principal {word}",
  async ({ page, world }, count: number, alias: string) => {
    const scope = roleScope(world);
    const roleId = requiredActiveRoleId(world);
    const principal = await ensureScenarioPrincipalAccount(page, world, alias);
    const grantScope = permissionScopeFromRoleScope(scope);
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
    const cookies = await page.context().cookies();

    const body = {
      role: { role_id: asRoleId(roleId), scope } satisfies ScopedRole,
      principal: {
        principal: "account",
        account_id: asAccountId(principal.accountId),
      } satisfies PrincipalRef,
      grant_scope: grantScope,
    };

    const cookieHeader = cookies.map((c) => `${c.name}=${c.value}`).join("; ");

    const capabilitiesResponse = await fetch(`${apiUrl}/roles/capabilities`, {
      method: "GET",
      headers: { cookie: cookieHeader },
    });
    if (!capabilitiesResponse.ok) {
      throw new Error(
        `capabilities request failed: ${capabilitiesResponse.status} ${await capabilitiesResponse.text()}`,
      );
    }
    const capabilitiesPayload = (await capabilitiesResponse.json()) as unknown;
    const capabilitiesSnapshot =
      parseRoleCapabilitySnapshot(capabilitiesPayload);
    const csrfToken = capabilitiesSnapshot.csrf_token;

    const requests = Array.from({ length: count }, () =>
      fetch(`${apiUrl}/roles/apply`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          cookie: cookieHeader,
          "x-csrf-token": csrfToken,
        },
        body: JSON.stringify(body),
      }),
    );

    const responses = await Promise.all(requests);
    const outcomes: Array<{ ok: boolean; grants: PermissionGrantView[] }> = [];
    for (const response of responses) {
      if (!response.ok) {
        throw new Error(
          `concurrent apply-role returned ${response.status}: ${await response.text()}`,
        );
      }
      const payload = (await response.json()) as unknown;
      const parsed = parseApplyRoleResponse(payload);
      outcomes.push({ ok: true, grants: parsed.grants });
    }

    for (let i = 0; i < outcomes.length; i += 1) {
      if (!outcomes[i]?.ok) {
        throw new Error(`concurrent apply ${i} failed unexpectedly`);
      }
    }

    const readModelGrants = await readDirectGrants(
      apiUrl,
      cookieHeader,
      {
        principal: "account",
        account_id: asAccountId(principal.accountId),
      } satisfies PrincipalRef,
      grantScope,
      scope,
    );
    const grantIds = grantIdsByPermission(readModelGrants);
    world.grantIdsByAlias.set(alias, new Map(grantIds));
    const existingSnapshots = world.applyGrantSnapshots.get(alias);
    if (existingSnapshots !== undefined) {
      existingSnapshots.push(new Map(grantIds));
    } else {
      world.applyGrantSnapshots.set(alias, [new Map(grantIds)]);
    }
    world.lastErrorCode = undefined;
    world.lastPermissionCheck = undefined;
  },
);

Then(
  "exactly one direct grant per permission exists for account principal {word} with permissions {string}",
  async ({ page, world }, alias: string, permissions: string) => {
    const principal = await ensureScenarioPrincipalAccount(page, world, alias);
    const expected = parsePermissionsCsv(permissions);
    const scope = roleScope(world);
    const grantScope = permissionScopeFromRoleScope(scope);
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
    const cookies = await page.context().cookies();
    const cookieHeader = cookies.map((c) => `${c.name}=${c.value}`).join("; ");

    const grants = await readDirectGrants(
      apiUrl,
      cookieHeader,
      {
        principal: "account",
        account_id: asAccountId(principal.accountId),
      } satisfies PrincipalRef,
      grantScope,
      scope,
    );

    if (grants.length !== expected.length) {
      throw new Error(
        `concurrent apply must produce exactly one grant per permission, got ${grants.length} grants for ${expected.length} permissions`,
      );
    }

    const actualPermissions = new Set(
      grants.map((g) => normalizePermissions([g.permission])[0] ?? ""),
    );
    const expectedPermissions = new Set(expected);
    if (actualPermissions.size !== expectedPermissions.size) {
      throw new Error(
        `grant permission set size mismatch: expected ${expectedPermissions.size}, got ${actualPermissions.size}`,
      );
    }
    for (const permission of expectedPermissions) {
      if (!actualPermissions.has(permission)) {
        throw new Error(`missing permission in grants: ${permission}`);
      }
    }
  },
);

Then(
  "the direct grant ids for account principal {word} match across {int} repeated observations of permissions {string}",
  async (
    { page, world },
    alias: string,
    observationCount: number,
    permissions: string,
  ) => {
    const principal = await ensureScenarioPrincipalAccount(page, world, alias);
    const expected = parsePermissionsCsv(permissions);
    const expectedSet = new Set(expected);
    const scope = roleScope(world);
    const grantScope = permissionScopeFromRoleScope(scope);
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
    const cookies = await page.context().cookies();
    const cookieHeader = cookies.map((c) => `${c.name}=${c.value}`).join("; ");

    const initial = world.grantIdsByAlias.get(alias);
    if (initial === undefined) {
      throw new Error(
        "initial grant ids must be recorded from concurrent apply",
      );
    }

    for (let obs = 1; obs <= observationCount; obs += 1) {
      const grants = await readDirectGrants(
        apiUrl,
        cookieHeader,
        {
          principal: "account",
          account_id: asAccountId(principal.accountId),
        } satisfies PrincipalRef,
        grantScope,
        scope,
      );
      const current = grantIdsByPermission(grants);
      const currentPermissions = new Set(current.keys());
      if (currentPermissions.size !== expectedSet.size) {
        throw new Error(
          `observation ${obs} permission count mismatch: expected ${expectedSet.size}, got ${currentPermissions.size}`,
        );
      }
      for (const permission of expectedSet) {
        if (!currentPermissions.has(permission)) {
          throw new Error(
            `observation ${obs} missing permission '${permission}'`,
          );
        }
      }
      assertGrantSnapshotEqual(current, initial, `observation ${obs}`);
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

function syntheticPermissions(permissionCount: number): string[] {
  const values: string[] = [];
  for (let index = 0; index < permissionCount; index += 1) {
    values.push(`project.synthetic_${index}`);
  }
  return values;
}

function normalizePermissions(values: string[]): string[] {
  return values
    .map((value) => value.trim().toLowerCase())
    .filter((value) => value.length > 0);
}

function permissionScopeIdentity(scope: PermissionScope): string {
  switch (scope.scope) {
    case "account":
      return `account:${scope.account_id}`;
    case "organization":
      return `organization:${scope.org_id}`;
    case "project":
      return `project:${scope.project_id}`;
  }
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
  const grantIdsByPermission = new Map<string, PermissionGrantId>();

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
    if (!result.response.allowed) {
      throw new Error(
        `expected ${alias} to have permission ${permission}, but it was denied`,
      );
    }
    if (result.response.matching_grant_ids.length !== 1) {
      throw new Error(
        `expected one matching grant id for ${alias}:${permission}, got ${result.response.matching_grant_ids.length}`,
      );
    }
    const grantId = result.response.matching_grant_ids[0];
    if (grantId === undefined) {
      throw new Error(
        `matching grant id missing for ${alias}:${permission} despite length check`,
      );
    }
    grantIdsByPermission.set(permission, grantId);
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
    if (result.response.allowed) {
      throw new Error(
        `expected ${alias} to be denied for non-granted permission ${probe}`,
      );
    }
    if (result.response.matching_grant_ids.length !== 0) {
      throw new Error(
        `expected no matching grant ids for denied permission ${alias}:${probe}`,
      );
    }
  }
  world.grantIdsByAlias.set(alias, grantIdsByPermission);
}

function recordApplyGrantSnapshot(
  world: RoleWorld,
  alias: string,
  grants: PermissionGrantView[],
): void {
  const byPermission = grantIdsByPermission(grants);
  world.grantIdsByAlias.set(alias, byPermission);
  const snapshots = world.applyGrantSnapshots.get(alias) ?? [];
  snapshots.push(new Map(byPermission));
  world.applyGrantSnapshots.set(alias, snapshots);
}

function grantIdsByPermission(
  grants: PermissionGrantView[],
): Map<string, PermissionGrantId> {
  const ids = new Map<string, PermissionGrantId>();
  for (const grant of grants) {
    const permission = normalizePermissions([grant.permission])[0] ?? "";
    if (permission.length === 0) {
      throw new Error("apply response permission should not be empty");
    }
    if (ids.has(permission)) {
      throw new Error(
        `duplicate grant id observed for permission '${permission}' in apply response`,
      );
    }
    ids.set(permission, grant.id);
  }
  return ids;
}

function assertGrantSnapshotMatchesPermissions(
  snapshot: Map<string, PermissionGrantId>,
  permissions: string[],
  label: string,
): void {
  const snapshotPermissions = new Set(snapshot.keys());
  const expectedPermissions = new Set(permissions);
  if (snapshotPermissions.size !== expectedPermissions.size) {
    throw new Error(
      `${label} permission count mismatch: expected ${expectedPermissions.size}, got ${snapshotPermissions.size}`,
    );
  }
  for (const permission of expectedPermissions) {
    if (!snapshotPermissions.has(permission)) {
      throw new Error(`${label} missing permission '${permission}'`);
    }
  }
}

function assertGrantSnapshotEqual(
  left: Map<string, PermissionGrantId>,
  right: Map<string, PermissionGrantId>,
  label: string,
): void {
  if (left.size !== right.size) {
    throw new Error(`${label} grant-id map size mismatch`);
  }
  for (const [permission, grantId] of left.entries()) {
    const rightGrantId = right.get(permission);
    if (rightGrantId === undefined) {
      throw new Error(`${label} missing permission '${permission}'`);
    }
    if (rightGrantId !== grantId) {
      throw new Error(
        `${label} grant id mismatch for permission '${permission}': expected ${grantId}, got ${rightGrantId}`,
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

function missingPrincipalAccountId(_alias: string): string {
  return asAccountId(crypto.randomUUID());
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
  const outcome = await submitRoleCardWithResponse(page, card, input);
  return outcome.message;
}

async function submitRoleCardWithoutRequest(
  page: import("@playwright/test").Page,
  card: import("@playwright/test").Locator,
  label: string,
): Promise<string> {
  await card.getByRole("button", { name: /^run$/i }).click();
  return await waitForRoleMessage(page, label);
}

async function runApplyRole(
  page: import("@playwright/test").Page,
  input: {
    roleId: string;
    roleScope: RoleScope;
    principalAccountId: string;
    grantScope: PermissionScope;
  },
): Promise<
  { ok: true; response: ApplyRoleResponse } | { ok: false; code: string }
> {
  const applyCard = roleCard(page, "Apply role");
  await applyCard.locator('[name="role_id"]').fill(input.roleId);
  await setScope(applyCard, "role_scope_", input.roleScope);
  await applyCard.locator('[name="principal_kind"]').selectOption("account");
  await applyCard
    .locator('[name="principal_id"]')
    .fill(input.principalAccountId);
  await setPermissionScope(applyCard, "grant_scope_", input.grantScope);

  const outcome = await submitRoleCardWithResponse(page, applyCard, {
    path: "/roles/apply",
    label: "apply role",
  });
  if (outcome.message.endsWith(": ok")) {
    const payload = (await outcome.response.json()) as unknown;
    return { ok: true, response: parseApplyRoleResponse(payload) };
  }
  return {
    ok: false,
    code:
      parseRoleErrorCode(outcome.message, "apply role") ?? "transport_error",
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
): Promise<
  { ok: true; response: PermissionCheckResponse } | { ok: false; code: string }
> {
  const checkCard = roleCard(page, "Check permission");

  await checkCard
    .locator('[name="principal_kind"]')
    .selectOption(input.principalKind);
  await checkCard.locator('[name="principal_id"]').fill(input.principalId);
  await checkCard.locator('[name="permission"]').fill(input.permission);
  await setPermissionScope(checkCard, "scope_", input.scope);

  const outcome = await submitRoleCardWithResponse(page, checkCard, {
    path: "/permissions/check",
    label: "check permission",
  });

  if (!outcome.message.endsWith(": ok")) {
    return {
      ok: false,
      code:
        parseRoleErrorCode(outcome.message, "check permission") ??
        "transport_error",
    };
  }

  const payload = (await outcome.response.json()) as unknown;
  return { ok: true, response: parsePermissionCheckResponse(payload) };
}

async function submitRoleCardWithResponse(
  page: import("@playwright/test").Page,
  card: import("@playwright/test").Locator,
  input: { path: string; label: string },
): Promise<{ response: import("@playwright/test").Response; message: string }> {
  const responsePromise = waitForApiResponse(page, input.path);
  await card.getByRole("button", { name: /^run$/i }).click();
  const response = await responsePromise;
  const message = await waitForRoleMessage(page, input.label);
  return { response, message };
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
  if (code.length === 0) {
    return "transport_error";
  }
  if (code === "transport_error" || isRoleServerFailureCode(code)) {
    return code;
  }
  return "transport_error";
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
  const row = await readRoleTemplateRow(page, roleId);
  if (row === null) {
    throw new Error(`role ${roleId} was not found in the read model`);
  }

  if (row.permissionsCsv.length === 0) {
    throw new Error(`role read-model row missing permissions for ${roleId}`);
  }

  return parsePermissionsCsv(row.permissionsCsv);
}

async function readRoleTemplateLines(
  page: import("@playwright/test").Page,
): Promise<string[]> {
  const section = roleReadModelSection(page);
  const rows = section.locator("li[data-role-id]");
  const count = await rows.count();
  const lines: string[] = [];
  for (let index = 0; index < count; index += 1) {
    lines.push(((await rows.nth(index).textContent()) ?? "").trim());
  }
  return lines;
}

async function readRoleTemplateRow(
  page: import("@playwright/test").Page,
  roleId: string,
): Promise<{ permissionsCsv: string } | null> {
  const row = roleReadModelSection(page).locator(
    `li[data-role-id="${roleId}"]`,
  );
  if ((await row.count()) === 0) {
    return null;
  }
  const permissionsCsv = await row.getAttribute("data-role-permissions");
  return {
    permissionsCsv: (permissionsCsv ?? "").trim(),
  };
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

async function readDirectGrants(
  apiUrl: string,
  cookieHeader: string,
  grantPrincipal: PrincipalRef,
  grantScope: PermissionScope,
  roleScope: RoleScope,
): Promise<PermissionGrantView[]> {
  const request = {
    role_scope: roleScope,
    role_cursor: null,
    role_limit: null,
    grant_principal: grantPrincipal,
    grant_scope: grantScope,
    grant_cursor: null,
    grant_limit: null,
  };
  const response = await fetch(`${apiUrl}/roles/read-model`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      cookie: cookieHeader,
    },
    body: JSON.stringify(request),
  });
  if (!response.ok) {
    throw new Error(
      `read-model request failed: ${response.status} ${await response.text()}`,
    );
  }
  const payload = (await response.json()) as unknown;
  const parsed = parseRoleReadModelResponse(payload);
  return parsed.direct_grants;
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
