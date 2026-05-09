import { createBdd, test as base } from "playwright-bdd";
import {
  parseRoleFailure,
  type ApplyRoleRequest,
  type ApplyRoleResponse,
  type CreateRoleRequest,
  type CreateRoleResponse,
  type DeleteRoleRequest,
  type DeleteRoleResponse,
  type EditRoleRequest,
  type EditRoleResponse,
  type PermissionCheckRequest,
  type PermissionCheckResponse,
  type PermissionScope,
  type PrincipalRef,
  type RoleScope,
  type ScopedRole,
} from "../../../src/app/lib/generated/role-contract";

interface RoleWorld {
  scope: RoleScope | undefined;
  activeRole: ScopedRole | undefined;
  activeRolePermissions: string[] | undefined;
  principals: Map<string, string>;
  lastPermissionCheck: boolean | undefined;
  lastErrorCode: string | undefined;
}

type RoleTest = ReturnType<typeof base.extend<{ world: RoleWorld }>>;

export const test: RoleTest = base.extend<{ world: RoleWorld }>({
  world: async ({ browserName: _browserName }, use) => {
    await use({
      scope: undefined,
      activeRole: undefined,
      activeRolePermissions: undefined,
      principals: new Map(),
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

Given("a clean role-template environment", async ({ world }) => {
  world.scope = undefined;
  world.activeRole = undefined;
  world.activeRolePermissions = undefined;
  world.principals.clear();
  world.lastPermissionCheck = undefined;
  world.lastErrorCode = undefined;
});

Given("an organization role scope", async ({ world }) => {
  world.scope = { scope: "organization", org_id: crypto.randomUUID() };
});

When(
  "the operator creates role template {string} with permissions {string}",
  async ({ world }, name: string, permissions: string) => {
    const request: CreateRoleRequest = {
      scope: roleScope(world),
      name,
      permissions: parsePermissionsCsv(permissions),
    };
    const response = await postJson<CreateRoleResponse>("/roles", request);
    if (!response.ok) {
      throw new Error(
        `create role failed (${response.code}): ${response.summary}`,
      );
    }
    world.activeRole = {
      role_id: response.json.role.id,
      scope: response.json.role.scope,
    };
    world.activeRolePermissions = normalizePermissions(
      response.json.role.permissions,
    );
    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator edits the active role template to name {string} and permissions {string}",
  async ({ world }, name: string, permissions: string) => {
    const activeRole = requiredActiveRole(world);
    const request: EditRoleRequest = {
      role: activeRole,
      name,
      permissions: parsePermissionsCsv(permissions),
    };
    const response = await postJson<EditRoleResponse>("/roles/edit", request);
    if (!response.ok) {
      throw new Error(
        `edit role failed (${response.code}): ${response.summary}`,
      );
    }
    world.activeRole = {
      role_id: response.json.role.id,
      scope: response.json.role.scope,
    };
    world.activeRolePermissions = normalizePermissions(
      response.json.role.permissions,
    );
    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When("the operator deletes the active role template", async ({ world }) => {
  const activeRole = requiredActiveRole(world);
  const request: DeleteRoleRequest = { role: activeRole };
  const response = await postJson<DeleteRoleResponse>("/roles/delete", request);
  if (!response.ok) {
    throw new Error(
      `delete role failed (${response.code}): ${response.summary}`,
    );
  }
  if (response.json.role.role_id !== activeRole.role_id) {
    throw new Error("delete response role_id mismatch");
  }
  world.lastPermissionCheck = undefined;
  world.lastErrorCode = undefined;
});

When(
  "the operator applies the role template to account principal {word}",
  async ({ world }, alias: string) => {
    const activeRole = requiredActiveRole(world);
    const principal: PrincipalRef = {
      principal: "account",
      account_id: accountId(world, alias),
    };
    const request: ApplyRoleRequest = {
      role: activeRole,
      principal,
      grant_scope: permissionScopeFromRoleScope(roleScope(world)),
    };
    const response = await postJson<ApplyRoleResponse>("/roles/apply", request);
    if (!response.ok) {
      throw new Error(
        `apply role failed (${response.code}): ${response.summary}`,
      );
    }
    if (response.json.grants.length === 0) {
      throw new Error("apply role should create at least one grant");
    }
    for (const grant of response.json.grants) {
      if (grant.principal.principal !== "account") {
        throw new Error("grant principal kind mismatch");
      }
      if (grant.principal.account_id !== principal.account_id) {
        throw new Error("grant principal account mismatch");
      }
      if (
        !permissionScopeEquals(
          grant.scope,
          permissionScopeFromRoleScope(roleScope(world)),
        )
      ) {
        throw new Error("grant scope mismatch");
      }
      if (grant.source_role_id !== activeRole.role_id) {
        throw new Error("grant source role mismatch");
      }
    }
    world.lastPermissionCheck = undefined;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator checks permission {string} for account principal {word}",
  async ({ world }, permission: string, alias: string) => {
    const request: PermissionCheckRequest = {
      principal: {
        principal: "account",
        account_id: accountId(world, alias),
      },
      permission,
      scope: permissionScopeFromRoleScope(roleScope(world)),
    };
    const response = await postJson<PermissionCheckResponse>(
      "/permissions/check",
      request,
    );
    if (!response.ok) {
      throw new Error(
        `permission check failed (${response.code}): ${response.summary}`,
      );
    }
    world.lastPermissionCheck = response.json.allowed;
    world.lastErrorCode = undefined;
  },
);

When(
  "the operator checks permission {string} for the role template principal",
  async ({ world }, permission: string) => {
    const activeRole = requiredActiveRole(world);
    const request: PermissionCheckRequest = {
      principal: {
        principal: "role",
        role_id: activeRole.role_id,
      },
      permission,
      scope: permissionScopeFromRoleScope(roleScope(world)),
    };
    const response = await postJson<PermissionCheckResponse>(
      "/permissions/check",
      request,
    );
    if (response.ok) {
      world.lastPermissionCheck = response.json.allowed;
      world.lastErrorCode = "unexpected_success";
      return;
    }
    world.lastPermissionCheck = undefined;
    world.lastErrorCode = response.code;
  },
);

Then(
  "the active role template has permissions {string}",
  async ({ world }, permissions: string) => {
    const expected = parsePermissionsCsv(permissions);
    const actual = world.activeRolePermissions;
    if (!actual) {
      throw new Error("active role template should exist");
    }
    assertPermissionSetEqual(actual, expected);
  },
);

Then("the active role template no longer exists", async ({ world }) => {
  const activeRole = requiredActiveRole(world);
  const request: ApplyRoleRequest = {
    role: activeRole,
    principal: {
      principal: "account",
      account_id: accountId(world, "deleted_role_probe"),
    },
    grant_scope: permissionScopeFromRoleScope(roleScope(world)),
  };
  const response = await postJson<ApplyRoleResponse>("/roles/apply", request);
  if (response.ok) {
    throw new Error("expected deleted role apply to fail with not_found");
  }
  if (response.code !== "not_found") {
    throw new Error(
      `expected deleted role apply to fail with not_found, got ${response.code}`,
    );
  }
});

Then(
  "account principal {word} has direct grants {string}",
  async ({ world }, alias: string, permissions: string) => {
    await assertDirectGrantPermissions(world, alias, permissions);
  },
);

Then(
  "account principal {word} retains direct grants {string}",
  async ({ world }, alias: string, permissions: string) => {
    await assertDirectGrantPermissions(world, alias, permissions);
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
    org_id: crypto.randomUUID(),
  };
  world.scope = scope;
  return scope;
}

function permissionScopeFromRoleScope(scope: RoleScope): PermissionScope {
  switch (scope.scope) {
    case "account":
      return { scope: "account", account_id: scope.account_id };
    case "organization":
      return { scope: "organization", org_id: scope.org_id };
    case "project":
      return { scope: "project", project_id: scope.project_id };
  }
}

function accountId(world: RoleWorld, alias: string): string {
  const existing = world.principals.get(alias);
  if (existing) {
    return existing;
  }
  const created = crypto.randomUUID();
  world.principals.set(alias, created);
  return created;
}

function requiredActiveRole(world: RoleWorld): ScopedRole {
  if (!world.activeRole) {
    throw new Error("active role template must be created first");
  }
  return world.activeRole;
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

function permissionScopeEquals(
  a: PermissionScope,
  b: PermissionScope,
): boolean {
  if (a.scope !== b.scope) return false;
  if (a.scope === "account" && b.scope === "account") {
    return a.account_id === b.account_id;
  }
  if (a.scope === "organization" && b.scope === "organization") {
    return a.org_id === b.org_id;
  }
  if (a.scope === "project" && b.scope === "project") {
    return a.project_id === b.project_id;
  }
  return false;
}

async function assertDirectGrantPermissions(
  world: RoleWorld,
  alias: string,
  permissionsCsv: string,
): Promise<void> {
  const expected = parsePermissionsCsv(permissionsCsv);
  const principal = {
    principal: "account" as const,
    account_id: accountId(world, alias),
  };
  const scope = permissionScopeFromRoleScope(roleScope(world));
  for (const permission of expected) {
    const request: PermissionCheckRequest = {
      principal,
      permission,
      scope,
    };
    const result = await postJson<PermissionCheckResponse>(
      "/permissions/check",
      request,
    );
    if (!result.ok) {
      throw new Error(
        `permission check failed (${result.code}) for ${alias}:${permission}`,
      );
    }
    if (!result.json.allowed) {
      throw new Error(
        `expected ${alias} to have permission ${permission}, but it was denied`,
      );
    }
  }
  for (const probe of DENY_PROBE_PERMISSIONS) {
    if (expected.includes(probe)) {
      continue;
    }
    const request: PermissionCheckRequest = {
      principal,
      permission: probe,
      scope,
    };
    const result = await postJson<PermissionCheckResponse>(
      "/permissions/check",
      request,
    );
    if (!result.ok) {
      throw new Error(
        `permission check failed (${result.code}) for ${alias}:${probe}`,
      );
    }
    if (result.json.allowed) {
      throw new Error(
        `expected ${alias} to be denied for non-granted permission ${probe}`,
      );
    }
  }
}

type ApiResult<T> =
  | { ok: true; json: T }
  | { ok: false; status: number; code: string; summary: string };

async function postJson<T>(path: string, body: unknown): Promise<ApiResult<T>> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const response = await fetch(`${apiUrl}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  const payload = (await response.json()) as unknown;
  if (response.ok) {
    return { ok: true, json: payload as T };
  }
  const failure = parseRoleFailure(payload);
  return {
    ok: false,
    status: response.status,
    code: failure.code,
    summary: failure.summary,
  };
}
