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
  canReadRoles: boolean;
  readModel: RoleReadModelResponse | null;
  roleNextCursor: RoleReadModelResponse["role_next_cursor"];
  grantNextCursor: RoleReadModelResponse["grant_next_cursor"];
  isRefreshing: boolean;
  isLoadingMoreRoles: boolean;
  isLoadingMoreGrants: boolean;
  onRefreshReadModel: () => void;
  onLoadMoreRoles: () => void;
  onLoadMoreGrants: () => void;
}

const ROLE_PERMISSION_PREVIEW_LIMIT = 5;
const OPERATION_SUMMARY_LINE_LIMIT = 120;

export function RoleOperationResultView(
  props: RoleOperationResultViewProps,
): ReactNode {
  const summary = props.summary;
  return (
    <section className="max-w-3xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4">
      <h2 className="mb-2 text-lg font-medium">Role operation result</h2>
      <p className="mb-2 text-sm">{props.message}</p>
      {summary === null ? (
        <p className="text-sm text-[--color-fg-muted]">No operations yet.</p>
      ) : (
        <>
          <p className="mb-2 text-sm font-medium">{summary.label}</p>
          <ul className="list-disc space-y-1 pl-5 text-sm">
            {summary.lines.map((line, index) => (
              <li key={`${summary.label}-${String(index)}`}>
                {truncateLine(line, OPERATION_SUMMARY_LINE_LIMIT)}
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

export function RoleReadModelView(props: RoleReadModelViewProps): ReactNode {
  const hasRoleCursor = props.roleNextCursor !== null;
  const hasGrantCursor = props.grantNextCursor !== null;
  return (
    <section className="max-w-4xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4">
      <h2 className="mb-2 text-lg font-medium">Role read model</h2>
      <button
        type="button"
        className="mb-2 rounded-md border border-[--color-border] px-3 py-1 text-xs font-medium disabled:opacity-60"
        onClick={props.onRefreshReadModel}
        disabled={!props.canReadRoles || props.isRefreshing}
      >
        {props.isRefreshing ? "Refreshing..." : "Reload snapshot"}
      </button>
      {!props.canReadRoles ? (
        <p className="mb-2 text-sm text-[--color-fg-muted]">
          This account does not have read model capability.
        </p>
      ) : null}
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
            Role next cursor: {formatRoleCursor(props.roleNextCursor)}
          </p>
          <p className="mt-1 text-xs text-[--color-fg-muted]">
            Grant next cursor: {formatGrantCursor(props.grantNextCursor)}
          </p>

          <h3 className="mt-3 text-sm font-medium">Role templates</h3>
          {props.readModel.role_templates.length === 0 ? (
            <p className="text-sm text-[--color-fg-muted]">
              No roles in this scope.
            </p>
          ) : (
            <ul className="mt-1 space-y-1 text-sm">
              {props.readModel.role_templates.map((role) => (
                <li
                  key={role.id}
                  data-role-id={role.id}
                  data-role-permissions={role.permissions.join(",")}
                >
                  <span className="font-medium">{role.name}</span>
                  {` (${role.id})`}
                  {` permissions: ${role.permissions.length}`}
                  {` [${summarizePermissions(role.permissions)}]`}
                </li>
              ))}
            </ul>
          )}
          <button
            type="button"
            className="mt-2 rounded-md border border-[--color-border] px-3 py-1 text-xs font-medium disabled:opacity-60"
            onClick={props.onLoadMoreRoles}
            disabled={
              !props.canReadRoles || !hasRoleCursor || props.isLoadingMoreRoles
            }
          >
            {props.isLoadingMoreRoles ? "Loading roles..." : "Load more roles"}
          </button>

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
          <button
            type="button"
            className="mt-2 rounded-md border border-[--color-border] px-3 py-1 text-xs font-medium disabled:opacity-60"
            onClick={props.onLoadMoreGrants}
            disabled={
              !props.canReadRoles ||
              !hasGrantCursor ||
              props.isLoadingMoreGrants
            }
          >
            {props.isLoadingMoreGrants
              ? "Loading grants..."
              : "Load more grants"}
          </button>
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

function summarizePermissions(permissions: string[]): string {
  if (permissions.length === 0) {
    return "none";
  }
  const preview = permissions
    .slice(0, ROLE_PERMISSION_PREVIEW_LIMIT)
    .join(", ");
  if (permissions.length <= ROLE_PERMISSION_PREVIEW_LIMIT) {
    return preview;
  }
  return `${preview} +${permissions.length - ROLE_PERMISSION_PREVIEW_LIMIT} more`;
}

function truncateLine(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, maxLength - 3)}...`;
}
