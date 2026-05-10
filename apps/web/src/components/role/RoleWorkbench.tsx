"use client";

import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import type {
  PermissionCheckResponse,
  RoleScope,
} from "@/app/lib/generated/role-contract";
import {
  applyRole,
  buildApplyRoleRequest,
  buildCreateRoleRequest,
  buildDeleteRoleRequest,
  buildEditRoleRequest,
  buildPermissionCheckRequest,
  checkPermission,
  checkPermissionRolePrincipalRejection,
  createRole,
  deleteRole,
  editRole,
  formatRoleError,
  type RoleCapabilitySnapshot,
} from "@/app/lib/role-client";

import { RoleOperationForms } from "./RoleOperationForms";
import {
  RoleOperationResultView,
  RoleReadModelView,
} from "./RoleReadModelView";
import type { OperationSummary } from "./RoleReadModelView";
import { useRoleCapabilities } from "./useRoleCapabilities";
import { resolveRoleReadContext, useRoleReadModel } from "./useRoleReadModel";

export function RoleWorkbench(): ReactNode {
  const [roleMessage, setRoleMessage] = useState<string>("");
  const [operationSummary, setOperationSummary] =
    useState<OperationSummary | null>(null);
  const {
    snapshot: capabilitySnapshot,
    capabilities,
    csrfToken,
    errorMessage: capabilityError,
  } = useRoleCapabilities();
  const { readModel, readContext, refreshReadModel, initializeReadModel } =
    useRoleReadModel();

  useEffect(() => {
    if (capabilityError !== null) {
      setRoleMessage(`capabilities: ${capabilityError}`);
    }
  }, [capabilityError]);

  useEffect(() => {
    if (capabilitySnapshot === null) {
      return;
    }
    void initializeReadModel(
      capabilitySnapshot.capabilities.actor.account_id,
    ).catch((reason: unknown) => {
      setRoleMessage(`read model: ${formatRoleError(reason)}`);
    });
  }, [capabilitySnapshot, initializeReadModel]);

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
    void runRoleAction("create role", async () => {
      const token = requireCsrfToken(csrfToken);
      const snapshot = requireCapabilitySnapshot(capabilitySnapshot);
      const submission = buildCreateRoleRequest(
        new FormData(event.currentTarget),
      );
      const context = resolveRoleReadContext(
        readContext,
        snapshot.capabilities.actor.account_id,
        submission.context,
      );
      const response = await createRole(submission.request, token);
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
    void runRoleAction("edit role", async () => {
      const token = requireCsrfToken(csrfToken);
      const snapshot = requireCapabilitySnapshot(capabilitySnapshot);
      const submission = buildEditRoleRequest(
        new FormData(event.currentTarget),
      );
      const context = resolveRoleReadContext(
        readContext,
        snapshot.capabilities.actor.account_id,
        submission.context,
      );
      const response = await editRole(submission.request, token);
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
    void runRoleAction("delete role", async () => {
      const token = requireCsrfToken(csrfToken);
      const snapshot = requireCapabilitySnapshot(capabilitySnapshot);
      const submission = buildDeleteRoleRequest(
        new FormData(event.currentTarget),
      );
      const context = resolveRoleReadContext(
        readContext,
        snapshot.capabilities.actor.account_id,
        submission.context,
      );
      const response = await deleteRole(submission.request, token);
      setOperationSummary({
        label: "Delete role",
        lines: [
          `role: ${response.role.role_id}`,
          `scope: ${formatScopeLabel(response.role.scope)}`,
        ],
      });
      await refreshReadModel(context);
    });
  };

  const onApplyRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleAction("apply role", async () => {
      const token = requireCsrfToken(csrfToken);
      const snapshot = requireCapabilitySnapshot(capabilitySnapshot);
      const submission = buildApplyRoleRequest(
        new FormData(event.currentTarget),
      );
      const context = resolveRoleReadContext(
        readContext,
        snapshot.capabilities.actor.account_id,
        submission.context,
      );
      const response = await applyRole(submission.request, token);
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
    void runRoleAction("check permission", async () => {
      const snapshot = requireCapabilitySnapshot(capabilitySnapshot);
      const submission = buildPermissionCheckRequest(
        new FormData(event.currentTarget),
      );
      const context = resolveRoleReadContext(
        readContext,
        snapshot.capabilities.actor.account_id,
        submission.context,
      );
      const response =
        submission.principalKind === "role"
          ? await checkPermissionRolePrincipalRejection(submission.request)
          : await checkPermission(submission.request);
      setOperationSummary(buildPermissionSummary(response));
      await refreshReadModel(context);
    });
  };

  return (
    <>
      <RoleOperationForms
        capabilities={capabilities}
        onCreateRole={onCreateRole}
        onEditRole={onEditRole}
        onDeleteRole={onDeleteRole}
        onApplyRole={onApplyRole}
        onCheckPermission={onCheckPermission}
      />

      <RoleOperationResultView
        message={roleMessage}
        summary={operationSummary}
      />

      <RoleReadModelView readModel={readModel} />
    </>
  );
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

function requireCapabilitySnapshot(
  actorSnapshot: RoleCapabilitySnapshot | null,
): RoleCapabilitySnapshot {
  if (actorSnapshot === null) {
    throw new Error("role capabilities are still loading");
  }
  return actorSnapshot;
}

function requireCsrfToken(token: string | null): string {
  if (token === null) {
    throw new Error("CSRF token is unavailable");
  }
  return token;
}

function formatScopeLabel(scope: RoleScope): string {
  if (scope.scope === "organization") {
    return `organization:${scope.org_id}`;
  }
  if (scope.scope === "project") {
    return `project:${scope.project_id}`;
  }
  return `account:${scope.account_id}`;
}
