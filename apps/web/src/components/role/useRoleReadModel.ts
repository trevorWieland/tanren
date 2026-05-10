import { useCallback, useEffect, useRef, useState } from "react";

import type {
  AccountId,
  AccountPrincipalRef,
  PermissionGrantView,
  RoleId,
  RoleReadModelResponse,
  RoleTemplateView,
} from "@/app/lib/generated/role-contract";
import {
  permissionScopeFromRoleScope,
  readRoleModel,
  requireRoleActionSnapshot,
  ROLE_READ_MODEL_DEFAULT_GRANT_PAGE_SIZE,
  ROLE_READ_MODEL_DEFAULT_ROLE_PAGE_SIZE,
  type RoleCapabilitySnapshot,
  type RoleRequestContextInput,
} from "@/app/lib/role-client";

export interface RoleReadContext {
  roleScope: RoleRequestContextInput["roleScope"];
  grantPrincipal: AccountPrincipalRef;
  grantScope: RoleRequestContextInput["grantScope"];
}

type ReadModelRequestMode = "replace" | "append_roles" | "append_grants";

interface UseRoleReadModelResult {
  readModel: RoleReadModelResponse | null;
  readContext: RoleReadContext | null;
  roleNextCursor: RoleReadModelResponse["role_next_cursor"];
  grantNextCursor: RoleReadModelResponse["grant_next_cursor"];
  isRefreshing: boolean;
  isLoadingMoreRoles: boolean;
  isLoadingMoreGrants: boolean;
  refreshReadModel: (context: RoleReadContext) => Promise<void>;
  initializeReadModel: (actorAccountId: AccountId) => Promise<void>;
  loadMoreRoleTemplates: () => Promise<void>;
  loadMoreDirectGrants: () => Promise<void>;
  upsertLocalRoleTemplate: (
    context: RoleReadContext,
    role: RoleTemplateView,
  ) => boolean;
  removeLocalRoleTemplate: (
    context: RoleReadContext,
    roleId: RoleId,
  ) => boolean;
  appendLocalDirectGrants: (
    context: RoleReadContext,
    grants: PermissionGrantView[],
  ) => boolean;
}

interface ReadModelRequestInput {
  context: RoleReadContext;
  roleCursor: RoleReadModelResponse["role_next_cursor"];
  grantCursor: RoleReadModelResponse["grant_next_cursor"];
  mode: ReadModelRequestMode;
}

export function buildDefaultRoleReadContext(
  actorAccountId: AccountId,
): RoleReadContext {
  const roleScope = { scope: "account", account_id: actorAccountId } as const;
  return {
    roleScope,
    grantPrincipal: { principal: "account", account_id: actorAccountId },
    grantScope: permissionScopeFromRoleScope(roleScope),
  };
}

export function resolveRoleReadContext(
  current: RoleReadContext | null,
  actorAccountId: AccountId,
  next: RoleRequestContextInput,
): RoleReadContext {
  return {
    roleScope: next.roleScope,
    grantPrincipal: next.grantPrincipal ??
      current?.grantPrincipal ?? {
        principal: "account",
        account_id: actorAccountId,
      },
    grantScope: next.grantScope,
  };
}

export function isSameRoleReadContext(
  left: RoleReadContext,
  right: RoleReadContext,
): boolean {
  return (
    roleScopeKey(left.roleScope) === roleScopeKey(right.roleScope) &&
    permissionScopeKey(left.grantScope) ===
      permissionScopeKey(right.grantScope) &&
    left.grantPrincipal.account_id === right.grantPrincipal.account_id
  );
}

export function useRoleReadModel(
  capabilitySnapshot: RoleCapabilitySnapshot | null,
): UseRoleReadModelResult {
  const [readModel, setReadModel] = useState<RoleReadModelResponse | null>(
    null,
  );
  const [readContext, setReadContext] = useState<RoleReadContext | null>(null);
  const [roleNextCursor, setRoleNextCursor] =
    useState<RoleReadModelResponse["role_next_cursor"]>(null);
  const [grantNextCursor, setGrantNextCursor] =
    useState<RoleReadModelResponse["grant_next_cursor"]>(null);
  const [requestMode, setRequestMode] = useState<ReadModelRequestMode | null>(
    null,
  );

  const requestIdRef = useRef(0);
  const activeControllerRef = useRef<AbortController | null>(null);

  useEffect(() => {
    return () => {
      activeControllerRef.current?.abort();
      activeControllerRef.current = null;
    };
  }, []);

  const runReadModelRequest = useCallback(
    async ({
      context,
      roleCursor,
      grantCursor,
      mode,
    }: ReadModelRequestInput): Promise<void> => {
      requestIdRef.current += 1;
      const requestId = requestIdRef.current;

      activeControllerRef.current?.abort();
      const controller = new AbortController();
      activeControllerRef.current = controller;
      setRequestMode(mode);

      try {
        const snapshot = requireRoleActionSnapshot(
          capabilitySnapshot,
          "read_roles",
        );
        const next = await readRoleModel(
          {
            role_scope: context.roleScope,
            role_cursor: roleCursor,
            role_limit: ROLE_READ_MODEL_DEFAULT_ROLE_PAGE_SIZE,
            grant_principal: context.grantPrincipal,
            grant_scope: context.grantScope,
            grant_cursor: grantCursor,
            grant_limit: ROLE_READ_MODEL_DEFAULT_GRANT_PAGE_SIZE,
          },
          snapshot,
          controller.signal,
        );

        if (requestId !== requestIdRef.current) {
          return;
        }

        setReadContext(context);
        setReadModel((current) => {
          switch (mode) {
            case "replace":
              return next;
            case "append_roles": {
              if (current === null) {
                return next;
              }
              return {
                ...next,
                role_templates: mergeRoleTemplates(
                  current.role_templates,
                  next.role_templates,
                ),
                grant_principal: current.grant_principal,
                grant_scope: current.grant_scope,
                direct_grants: current.direct_grants,
                grant_next_cursor: current.grant_next_cursor,
              };
            }
            case "append_grants": {
              if (current === null) {
                return next;
              }
              return {
                ...next,
                role_scope: current.role_scope,
                role_templates: current.role_templates,
                role_next_cursor: current.role_next_cursor,
                direct_grants: mergeDirectGrants(
                  current.direct_grants,
                  next.direct_grants,
                ),
              };
            }
            default:
              return assertNever(mode, "read model request mode");
          }
        });

        switch (mode) {
          case "replace":
            setRoleNextCursor(next.role_next_cursor);
            setGrantNextCursor(next.grant_next_cursor);
            break;
          case "append_roles":
            setRoleNextCursor(next.role_next_cursor);
            break;
          case "append_grants":
            setGrantNextCursor(next.grant_next_cursor);
            break;
          default:
            assertNever(mode, "read model request mode");
        }
      } catch (reason: unknown) {
        if (isAbortError(reason)) {
          return;
        }
        throw reason;
      } finally {
        if (requestId === requestIdRef.current) {
          activeControllerRef.current = null;
          setRequestMode(null);
        }
      }
    },
    [capabilitySnapshot],
  );

  const refreshReadModel = useCallback(
    async (context: RoleReadContext) => {
      await runReadModelRequest({
        context,
        roleCursor: null,
        grantCursor: null,
        mode: "replace",
      });
    },
    [runReadModelRequest],
  );

  const initializeReadModel = useCallback(
    async (actorAccountId: AccountId): Promise<void> => {
      await refreshReadModel(buildDefaultRoleReadContext(actorAccountId));
    },
    [refreshReadModel],
  );

  const loadMoreRoleTemplates = useCallback(async (): Promise<void> => {
    if (readContext === null || roleNextCursor === null) {
      return;
    }
    await runReadModelRequest({
      context: readContext,
      roleCursor: roleNextCursor,
      grantCursor: null,
      mode: "append_roles",
    });
  }, [readContext, roleNextCursor, runReadModelRequest]);

  const loadMoreDirectGrants = useCallback(async (): Promise<void> => {
    if (readContext === null || grantNextCursor === null) {
      return;
    }
    await runReadModelRequest({
      context: readContext,
      roleCursor: null,
      grantCursor: grantNextCursor,
      mode: "append_grants",
    });
  }, [grantNextCursor, readContext, runReadModelRequest]);

  const upsertLocalRoleTemplate = useCallback(
    (context: RoleReadContext, role: RoleTemplateView): boolean => {
      if (
        readContext === null ||
        !isSameRoleReadContext(readContext, context)
      ) {
        return false;
      }
      setReadModel((current) => {
        if (current === null) {
          return current;
        }
        return {
          ...current,
          role_templates: mergeRoleTemplates(current.role_templates, [role]),
        };
      });
      return true;
    },
    [readContext],
  );

  const removeLocalRoleTemplate = useCallback(
    (context: RoleReadContext, roleId: RoleId): boolean => {
      if (
        readContext === null ||
        !isSameRoleReadContext(readContext, context)
      ) {
        return false;
      }
      setReadModel((current) => {
        if (current === null) {
          return current;
        }
        return {
          ...current,
          role_templates: current.role_templates.filter(
            (role) => role.id !== roleId,
          ),
        };
      });
      return true;
    },
    [readContext],
  );

  const appendLocalDirectGrants = useCallback(
    (context: RoleReadContext, grants: PermissionGrantView[]): boolean => {
      if (
        readContext === null ||
        !isSameRoleReadContext(readContext, context)
      ) {
        return false;
      }
      setReadModel((current) => {
        if (current === null) {
          return current;
        }
        return {
          ...current,
          direct_grants: mergeDirectGrants(current.direct_grants, grants),
        };
      });
      return true;
    },
    [readContext],
  );

  return {
    readModel,
    readContext,
    roleNextCursor,
    grantNextCursor,
    isRefreshing: requestMode === "replace",
    isLoadingMoreRoles: requestMode === "append_roles",
    isLoadingMoreGrants: requestMode === "append_grants",
    refreshReadModel,
    initializeReadModel,
    loadMoreRoleTemplates,
    loadMoreDirectGrants,
    upsertLocalRoleTemplate,
    removeLocalRoleTemplate,
    appendLocalDirectGrants,
  };
}

function roleScopeKey(scope: RoleReadContext["roleScope"]): string {
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

function permissionScopeKey(scope: RoleReadContext["grantScope"]): string {
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

function mergeRoleTemplates(
  current: RoleTemplateView[],
  incoming: RoleTemplateView[],
): RoleTemplateView[] {
  const byId = new Map<RoleTemplateView["id"], RoleTemplateView>();
  for (const role of current) {
    byId.set(role.id, role);
  }
  for (const role of incoming) {
    byId.set(role.id, role);
  }
  return [...byId.values()].sort(compareRoleTemplates);
}

function compareRoleTemplates(
  left: RoleTemplateView,
  right: RoleTemplateView,
): number {
  const nameDiff = left.name.localeCompare(right.name);
  if (nameDiff !== 0) {
    return nameDiff;
  }
  return left.id.localeCompare(right.id);
}

function mergeDirectGrants(
  current: PermissionGrantView[],
  incoming: PermissionGrantView[],
): PermissionGrantView[] {
  const byId = new Map<PermissionGrantView["id"], PermissionGrantView>();
  for (const grant of current) {
    byId.set(grant.id, grant);
  }
  for (const grant of incoming) {
    byId.set(grant.id, grant);
  }
  return [...byId.values()].sort(compareDirectGrants);
}

function compareDirectGrants(
  left: PermissionGrantView,
  right: PermissionGrantView,
): number {
  const grantedAtDiff = left.granted_at.localeCompare(right.granted_at);
  if (grantedAtDiff !== 0) {
    return grantedAtDiff;
  }
  return left.id.localeCompare(right.id);
}

function isAbortError(reason: unknown): boolean {
  return (
    (reason instanceof DOMException && reason.name === "AbortError") ||
    (reason instanceof Error && reason.name === "AbortError")
  );
}

function assertNever(value: never, context: string): never {
  throw new Error(`${context} received unsupported variant: ${String(value)}`);
}
