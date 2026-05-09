"use client";

import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import type {
  PermissionCheckResponse,
  RoleAdminAction,
  RoleAdminCapabilities,
} from "@/app/lib/generated/role-contract";
import {
  applyRole,
  checkPermission,
  createRole,
  deleteRole,
  editRole,
  fetchRoleCapabilities,
  formatRoleError,
  readPermissionBundle,
  readPermissionScope,
  readPrincipalRef,
  readRequiredField,
  readRoleScope,
} from "@/app/lib/role-client";

import {
  LabeledInput,
  PrincipalFields,
  RoleCard,
  ScopeFields,
} from "./RoleFormFields";

export function RoleWorkbench(): ReactNode {
  const [roleMessage, setRoleMessage] = useState<string>("");
  const [roleResult, setRoleResult] = useState<string>("");
  const [roleCapabilities, setRoleCapabilities] =
    useState<RoleAdminCapabilities | null>(null);
  const [roleCsrfToken, setRoleCsrfToken] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetchRoleCapabilities()
      .then((snapshot) => {
        if (!cancelled) {
          setRoleCapabilities(snapshot.capabilities);
          setRoleCsrfToken(snapshot.csrfToken);
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

  const runRoleAction = async <T,>(
    label: string,
    action: () => Promise<T>,
  ): Promise<void> => {
    try {
      const response = await action();
      setRoleMessage(`${label}: ok`);
      setRoleResult(JSON.stringify(response, null, 2));
    } catch (reason: unknown) {
      setRoleMessage(`${label}: ${formatRoleError(reason)}`);
      setRoleResult("");
    }
  };

  const onCreateRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("create role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    void runRoleAction("create role", () =>
      createRole(
        {
          scope: readRoleScope(form, "scope_"),
          name: readRequiredField(form, "name"),
          permissions: readPermissionBundle(form, "permissions"),
        },
        roleCsrfToken,
      ),
    );
  };

  const onEditRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("edit role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    void runRoleAction("edit role", () =>
      editRole(
        {
          role: {
            role_id: readRequiredField(form, "role_id"),
            scope: readRoleScope(form, "scope_"),
          },
          name: readRequiredField(form, "name"),
          permissions: readPermissionBundle(form, "permissions"),
        },
        roleCsrfToken,
      ),
    );
  };

  const onDeleteRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("delete role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    void runRoleAction("delete role", () =>
      deleteRole(
        {
          role: {
            role_id: readRequiredField(form, "role_id"),
            scope: readRoleScope(form, "scope_"),
          },
        },
        roleCsrfToken,
      ),
    );
  };

  const onApplyRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (roleCsrfToken === null) {
      setRoleMessage("apply role: CSRF token is unavailable");
      return;
    }
    const form = new FormData(event.currentTarget);
    void runRoleAction("apply role", () =>
      applyRole(
        {
          role: {
            role_id: readRequiredField(form, "role_id"),
            scope: readRoleScope(form, "role_scope_"),
          },
          principal: readPrincipalRef(form, "principal_"),
          grant_scope: readPermissionScope(form, "grant_scope_"),
        },
        roleCsrfToken,
      ),
    );
  };

  const onCheckPermission = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    void runRoleAction<PermissionCheckResponse>("check permission", () =>
      checkPermission({
        principal: readPrincipalRef(form, "principal_"),
        permission: readRequiredField(form, "permission"),
        scope: readPermissionScope(form, "scope_"),
      }),
    );
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
        <pre className="overflow-auto text-xs">{roleResult}</pre>
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
