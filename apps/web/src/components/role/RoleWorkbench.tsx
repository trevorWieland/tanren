"use client";

import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import type {
  AccountId,
  PermissionCheckResponse,
  PermissionScope,
  PrincipalRef,
  RoleAdminAction,
  RoleAdminCapabilities,
  RoleReadModelResponse,
  RoleScope,
} from "@/app/lib/generated/role-contract";
import { asAccountId } from "@/app/lib/generated/role-contract";
import {
  applyRole,
  checkPermission,
  createRole,
  deleteRole,
  editRole,
  fetchRoleCapabilities,
  formatRoleError,
  readPermissionBundle,
  readPermissionNameField,
  readPermissionScope,
  readPrincipalRef,
  readRequiredField,
  readRoleIdField,
  readRoleModel,
  readRoleScope,
} from "@/app/lib/role-client";

import {
  LabeledInput,
  PrincipalFields,
  RoleCard,
  ScopeFields,
} from "./RoleFormFields";

interface RoleReadContext {
  roleScope: RoleScope;
  grantPrincipal: PrincipalRef;
  grantScope: PermissionScope;
}

interface OperationSummary {
  label: string;
  lines: string[];
}

export function RoleWorkbench(): ReactNode {
  const [roleMessage, setRoleMessage] = useState<string>("");
  const [roleCapabilities, setRoleCapabilities] =
    useState<RoleAdminCapabilities | null>(null);
  const [roleCsrfToken, setRoleCsrfToken] = useState<string | null>(null);
  const [readModel, setReadModel] = useState<RoleReadModelResponse | null>(
    null,
  );
  const [readContext, setReadContext] = useState<RoleReadContext | null>(null);
  const [operationSummary, setOperationSummary] =
    useState<OperationSummary | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetchRoleCapabilities()
      .then((snapshot) => {
        if (!cancelled) {
          setRoleCapabilities(snapshot.capabilities);
          setRoleCsrfToken(snapshot.csrfToken);
          const initialContext = buildDefaultReadContext(
            snapshot.capabilities.actor.account_id,
          );
          setReadContext(initialContext);
          void refreshReadModel(initialContext);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setRoleMessage(`capabilities: ${formatRoleError(reason)}`);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const refreshReadModel = async (context: RoleReadContext): Promise<void> => {
    try {
      const next = await readRoleModel({
        role_scope: context.roleScope,
        role_cursor: null,
        role_limit: null,
        grant_principal: context.grantPrincipal,
        grant_scope: context.grantScope,
        grant_cursor: null,
        grant_limit: null,
      });
      setReadModel(next);
      setReadContext(context);
    } catch (reason: unknown) {
      setRoleMessage(`read model: ${formatRoleError(reason)}`);
    }
  };

  const runRoleAction = async (
    label: string,
    action: () => Promise<void>,
  ): Promise<void> => {
    try {
      await action();
      setRoleMessage(`${label}: ok`);
    } catch (reason: unknown) {
      setRoleMessage(`${label}: ${formatRoleError(reason)}`);
    }
  };

  const onCreateRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("create role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    const roleScope = readRoleScope(form, "scope_");
    const context = resolveContext(
      readContext,
      roleCapabilities,
      roleScope,
      readContext?.grantPrincipal,
      permissionScopeFromRoleScope(roleScope),
    );
    void runRoleAction("create role", async () => {
      const response = await createRole(
        {
          scope: roleScope,
          name: readRequiredField(form, "name"),
          permissions: readPermissionBundle(form, "permissions"),
        },
        roleCsrfToken,
      );
      setOperationSummary({
        label: "Create role",
        lines: [
          `role: ${response.role.id}`,
          `name: ${response.role.name}`,
          `permissions: ${response.role.permissions.length}`,
        ],
      });
      await refreshReadModel(context);
    });
  };

  const onEditRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("edit role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    const roleScope = readRoleScope(form, "scope_");
    const context = resolveContext(
      readContext,
      roleCapabilities,
      roleScope,
      readContext?.grantPrincipal,
      permissionScopeFromRoleScope(roleScope),
    );
    void runRoleAction("edit role", async () => {
      const response = await editRole(
        {
          role: {
            role_id: readRoleIdField(form, "role_id"),
            scope: roleScope,
          },
          name: readRequiredField(form, "name"),
          permissions: readPermissionBundle(form, "permissions"),
        },
        roleCsrfToken,
      );
      setOperationSummary({
        label: "Edit role",
        lines: [
          `role: ${response.role.id}`,
          `name: ${response.role.name}`,
          `permissions: ${response.role.permissions.length}`,
        ],
      });
      await refreshReadModel(context);
    });
  };

  const onDeleteRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("delete role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    const roleScope = readRoleScope(form, "scope_");
    const context = resolveContext(
      readContext,
      roleCapabilities,
      roleScope,
      readContext?.grantPrincipal,
      permissionScopeFromRoleScope(roleScope),
    );
    void runRoleAction("delete role", async () => {
      const response = await deleteRole(
        {
          role: {
            role_id: readRoleIdField(form, "role_id"),
            scope: roleScope,
          },
        },
        roleCsrfToken,
      );
      setOperationSummary({
        label: "Delete role",
        lines: [
          `role: ${response.role.role_id}`,
          `scope: ${formatRoleScope(response.role.scope)}`,
        ],
      });
      await refreshReadModel(context);
    });
  };

  const onApplyRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("apply role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    const roleScope = readRoleScope(form, "role_scope_");
    const principal = readPrincipalRef(form, "principal_");
    const grantScope = readPermissionScope(form, "grant_scope_");
    const context = resolveContext(
      readContext,
      roleCapabilities,
      roleScope,
      principal,
      grantScope,
    );
    void runRoleAction("apply role", async () => {
      const response = await applyRole(
        {
          role: {
            role_id: readRoleIdField(form, "role_id"),
            scope: roleScope,
          },
          principal,
          grant_scope: grantScope,
        },
        roleCsrfToken,
      );
      setOperationSummary({
        label: "Apply role",
        lines: [
          `role: ${response.role.role_id}`,
          `grants created: ${response.grants.length}`,
          `permissions: ${summarizeGrantPermissions(response.grants.map((grant) => grant.permission))}`,
        ],
      });
      await refreshReadModel(context);
    });
  };

  const onCheckPermission = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const principal = readPrincipalRef(form, "principal_");
    const scope = readPermissionScope(form, "scope_");
    const context = resolveContext(
      readContext,
      roleCapabilities,
      roleScopeFromPermissionScope(scope),
      principal,
      scope,
    );
    void runRoleAction("check permission", async () => {
      const response = await checkPermission({
        principal,
        permission: readPermissionNameField(form, "permission"),
        scope,
      });
      setOperationSummary(buildPermissionSummary(response));
      await refreshReadModel(context);
    });
  };

  return (
    <>
      <section className="grid gap-4 md:grid-cols-2">
        {hasRoleCapability(roleCapabilities, "create_role") ? (
          <RoleCard title="Create role" onSubmit={onCreateRole}>
            <ScopeFields prefix="scope_" />
            <LabeledInput
              name="name"
              label="Name"
              placeholder="workspace-admin"
            />
            <LabeledInput
              name="permissions"
              label="Permissions"
              placeholder="accounts.read,accounts.write"
            />
          </RoleCard>
        ) : null}

        {hasRoleCapability(roleCapabilities, "edit_role") ? (
          <RoleCard title="Edit role" onSubmit={onEditRole}>
            <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
            <ScopeFields prefix="scope_" />
            <LabeledInput
              name="name"
              label="Name"
              placeholder="workspace-admin"
            />
            <LabeledInput
              name="permissions"
              label="Permissions"
              placeholder="accounts.read,accounts.write"
            />
          </RoleCard>
        ) : null}

        {hasRoleCapability(roleCapabilities, "delete_role") ? (
          <RoleCard title="Delete role" onSubmit={onDeleteRole}>
            <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
            <ScopeFields prefix="scope_" />
          </RoleCard>
        ) : null}

        {hasRoleCapability(roleCapabilities, "apply_role") ? (
          <RoleCard title="Apply role" onSubmit={onApplyRole}>
            <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
            <ScopeFields prefix="role_scope_" legend="Role scope" />
            <PrincipalFields prefix="principal_" />
            <ScopeFields prefix="grant_scope_" legend="Grant scope" />
          </RoleCard>
        ) : null}

        {hasRoleCapability(roleCapabilities, "check_permission") ? (
          <RoleCard title="Check permission" onSubmit={onCheckPermission}>
            <PrincipalFields prefix="principal_" />
            <LabeledInput
              name="permission"
              label="Permission"
              placeholder="accounts.read"
            />
            <ScopeFields prefix="scope_" />
          </RoleCard>
        ) : null}

        {roleCapabilities === null ? (
          <div className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
            Loading role capabilities...
          </div>
        ) : roleCapabilities.actions.length > 0 ? null : (
          <div className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
            This account is authenticated but lacks role administration
            capabilities.
          </div>
        )}
      </section>

      <section className="max-w-3xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4">
        <h2 className="mb-2 text-lg font-medium">Role operation result</h2>
        <p className="mb-2 text-sm">{roleMessage}</p>
        {operationSummary === null ? (
          <p className="text-sm text-[--color-fg-muted]">No operations yet.</p>
        ) : (
          <>
            <p className="mb-2 text-sm font-medium">{operationSummary.label}</p>
            <ul className="list-disc space-y-1 pl-5 text-sm">
              {operationSummary.lines.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
          </>
        )}
      </section>

      <section className="max-w-4xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4">
        <h2 className="mb-2 text-lg font-medium">Role read model</h2>
        {readModel === null ? (
          <p className="text-sm text-[--color-fg-muted]">No snapshot loaded.</p>
        ) : (
          <>
            <p className="text-xs text-[--color-fg-muted]">
              Freshness: {readModel.freshness.observed_at}
            </p>
            <p className="mt-1 text-xs text-[--color-fg-muted]">
              Role scope: {formatRoleScope(readModel.role_scope)}
            </p>
            <p className="mt-1 text-xs text-[--color-fg-muted]">
              Grant principal: {formatPrincipal(readModel.grant_principal)}
            </p>
            <p className="mt-1 text-xs text-[--color-fg-muted]">
              Grant scope: {formatPermissionScope(readModel.grant_scope)}
            </p>
            <p className="mt-1 text-xs text-[--color-fg-muted]">
              Role next cursor: {formatRoleCursor(readModel.role_next_cursor)}
            </p>
            <p className="mt-1 text-xs text-[--color-fg-muted]">
              Grant next cursor:{" "}
              {formatGrantCursor(readModel.grant_next_cursor)}
            </p>

            <h3 className="mt-3 text-sm font-medium">Role templates</h3>
            {readModel.role_templates.length === 0 ? (
              <p className="text-sm text-[--color-fg-muted]">
                No roles in this scope.
              </p>
            ) : (
              <ul className="mt-1 space-y-1 text-sm">
                {readModel.role_templates.map((role) => (
                  <li key={role.id}>
                    <span className="font-medium">{role.name}</span>
                    {` (${role.id})`}
                    {` permissions: ${role.permissions.join(", ")}`}
                  </li>
                ))}
              </ul>
            )}

            <h3 className="mt-3 text-sm font-medium">Direct grants</h3>
            {readModel.direct_grants.length === 0 ? (
              <p className="text-sm text-[--color-fg-muted]">
                No direct grants in this page.
              </p>
            ) : (
              <ul className="mt-1 space-y-1 text-sm">
                {readModel.direct_grants.map((grant) => (
                  <li key={grant.id}>
                    {`${grant.permission} at ${grant.granted_at}`}
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </section>
    </>
  );
}

function hasRoleCapability(
  capabilities: RoleAdminCapabilities | null,
  action: RoleAdminAction,
): boolean {
  return capabilities?.actions.includes(action) ?? false;
}

function buildDefaultReadContext(actorAccountId: AccountId): RoleReadContext {
  const scope: RoleScope = { scope: "account", account_id: actorAccountId };
  return {
    roleScope: scope,
    grantPrincipal: { principal: "account", account_id: actorAccountId },
    grantScope: permissionScopeFromRoleScope(scope),
  };
}

function resolveContext(
  current: RoleReadContext | null,
  capabilities: RoleAdminCapabilities | null,
  roleScope: RoleScope,
  grantPrincipal: PrincipalRef | undefined,
  grantScope: PermissionScope,
): RoleReadContext {
  if (grantPrincipal !== undefined) {
    return { roleScope, grantPrincipal, grantScope };
  }
  if (current !== null) {
    return {
      roleScope,
      grantPrincipal: current.grantPrincipal,
      grantScope,
    };
  }
  const fallbackAccountId = capabilities?.actor.account_id ?? asAccountId("");
  return {
    roleScope,
    grantPrincipal: {
      principal: "account",
      account_id: fallbackAccountId,
    },
    grantScope,
  };
}

function buildPermissionSummary(
  response: PermissionCheckResponse,
): OperationSummary {
  return {
    label: "Permission check",
    lines: [
      `allowed: ${String(response.allowed)}`,
      `matching grants: ${response.matching_grant_ids.length}`,
    ],
  };
}

function summarizeGrantPermissions(permissions: string[]): string {
  if (permissions.length === 0) {
    return "none";
  }
  const preview = permissions.slice(0, 5).join(", ");
  if (permissions.length <= 5) {
    return preview;
  }
  return `${preview} +${permissions.length - 5} more`;
}

function roleScopeFromPermissionScope(scope: PermissionScope): RoleScope {
  if (scope.scope === "organization") {
    return { scope: "organization", org_id: scope.org_id };
  }
  if (scope.scope === "project") {
    return { scope: "project", project_id: scope.project_id };
  }
  return { scope: "account", account_id: scope.account_id };
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

function formatRoleScope(scope: RoleScope): string {
  if (scope.scope === "organization") {
    return `organization:${scope.org_id}`;
  }
  if (scope.scope === "project") {
    return `project:${scope.project_id}`;
  }
  return `account:${scope.account_id}`;
}

function formatPermissionScope(scope: PermissionScope): string {
  if (scope.scope === "organization") {
    return `organization:${scope.org_id}`;
  }
  if (scope.scope === "project") {
    return `project:${scope.project_id}`;
  }
  return `account:${scope.account_id}`;
}

function formatPrincipal(principal: PrincipalRef): string {
  if (principal.principal === "role") {
    return `role:${principal.role_id}`;
  }
  return `account:${principal.account_id}`;
}

function formatRoleCursor(
  cursor: RoleReadModelResponse["role_next_cursor"],
): string {
  if (cursor === null) {
    return "none";
  }
  return `${cursor.name} (${cursor.id})`;
}

function formatGrantCursor(
  cursor: RoleReadModelResponse["grant_next_cursor"],
): string {
  if (cursor === null) {
    return "none";
  }
  return `${cursor.granted_at} (${cursor.id})`;
}
