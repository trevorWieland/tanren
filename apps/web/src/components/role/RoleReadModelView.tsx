import type { ReactNode } from "react";

import type {
  PermissionScope,
  PrincipalRef,
  RoleReadModelResponse,
  RoleScope,
} from "@/app/lib/generated/role-contract";
import { assertNever } from "./role-action-descriptors";

export interface OperationSummary {
  label: string;
  lines: string[];
}

interface RoleOperationResultViewProps {
  message: string;
  summary: OperationSummary | null;
}

interface RoleReadModelViewProps {
  readModel: RoleReadModelResponse | null;
}

export function RoleOperationResultView(
  props: RoleOperationResultViewProps,
): ReactNode {
  return (
    <section className="max-w-3xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4">
      <h2 className="mb-2 text-lg font-medium">Role operation result</h2>
      <p className="mb-2 text-sm">{props.message}</p>
      {props.summary === null ? (
        <p className="text-sm text-[--color-fg-muted]">No operations yet.</p>
      ) : (
        <>
          <p className="mb-2 text-sm font-medium">{props.summary.label}</p>
          <ul className="list-disc space-y-1 pl-5 text-sm">
            {props.summary.lines.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

export function RoleReadModelView(props: RoleReadModelViewProps): ReactNode {
  return (
    <section className="max-w-4xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4">
      <h2 className="mb-2 text-lg font-medium">Role read model</h2>
      {props.readModel === null ? (
        <p className="text-sm text-[--color-fg-muted]">No snapshot loaded.</p>
      ) : (
        <>
          <p className="text-xs text-[--color-fg-muted]">
            Freshness: {props.readModel.freshness.observed_at}
          </p>
          <p className="mt-1 text-xs text-[--color-fg-muted]">
            Role scope: {formatRoleScope(props.readModel.role_scope)}
          </p>
          <p className="mt-1 text-xs text-[--color-fg-muted]">
            Grant principal: {formatPrincipal(props.readModel.grant_principal)}
          </p>
          <p className="mt-1 text-xs text-[--color-fg-muted]">
            Grant scope: {formatPermissionScope(props.readModel.grant_scope)}
          </p>
          <p className="mt-1 text-xs text-[--color-fg-muted]">
            Role next cursor:{" "}
            {formatRoleCursor(props.readModel.role_next_cursor)}
          </p>
          <p className="mt-1 text-xs text-[--color-fg-muted]">
            Grant next cursor:{" "}
            {formatGrantCursor(props.readModel.grant_next_cursor)}
          </p>

          <h3 className="mt-3 text-sm font-medium">Role templates</h3>
          {props.readModel.role_templates.length === 0 ? (
            <p className="text-sm text-[--color-fg-muted]">
              No roles in this scope.
            </p>
          ) : (
            <ul className="mt-1 space-y-1 text-sm">
              {props.readModel.role_templates.map((role) => (
                <li key={role.id}>
                  <span className="font-medium">{role.name}</span>
                  {` (${role.id})`}
                  {` permissions: ${role.permissions.join(", ")}`}
                </li>
              ))}
            </ul>
          )}

          <h3 className="mt-3 text-sm font-medium">Direct grants</h3>
          {props.readModel.direct_grants.length === 0 ? (
            <p className="text-sm text-[--color-fg-muted]">
              No direct grants in this page.
            </p>
          ) : (
            <ul className="mt-1 space-y-1 text-sm">
              {props.readModel.direct_grants.map((grant) => (
                <li
                  key={grant.id}
                >{`${grant.permission} at ${grant.granted_at}`}</li>
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}

function formatRoleScope(scope: RoleScope): string {
  switch (scope.scope) {
    case "organization":
      return `organization:${scope.org_id}`;
    case "project":
      return `project:${scope.project_id}`;
    case "account":
      return `account:${scope.account_id}`;
    default:
      return assertNever(scope, "role scope");
  }
}

function formatPermissionScope(scope: PermissionScope): string {
  switch (scope.scope) {
    case "organization":
      return `organization:${scope.org_id}`;
    case "project":
      return `project:${scope.project_id}`;
    case "account":
      return `account:${scope.account_id}`;
    default:
      return assertNever(scope, "permission scope");
  }
}

function formatPrincipal(principal: PrincipalRef): string {
  switch (principal.principal) {
    case "account":
      return `account:${principal.account_id}`;
    case "role":
      return `role:${principal.role_id}`;
    default:
      return assertNever(principal, "principal");
  }
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
