import { useCallback, useState } from "react";

import type {
  AccountId,
  AccountPrincipalRef,
  RoleReadModelResponse,
} from "@/app/lib/generated/role-contract";
import {
  permissionScopeFromRoleScope,
  readRoleModel,
  type RoleRequestContextInput,
} from "@/app/lib/role-client";

export interface RoleReadContext {
  roleScope: RoleRequestContextInput["roleScope"];
  grantPrincipal: AccountPrincipalRef;
  grantScope: RoleRequestContextInput["grantScope"];
}

interface UseRoleReadModelResult {
  readModel: RoleReadModelResponse | null;
  readContext: RoleReadContext | null;
  refreshReadModel: (context: RoleReadContext) => Promise<void>;
  initializeReadModel: (actorAccountId: AccountId) => Promise<void>;
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

export function useRoleReadModel(): UseRoleReadModelResult {
  const [readModel, setReadModel] = useState<RoleReadModelResponse | null>(
    null,
  );
  const [readContext, setReadContext] = useState<RoleReadContext | null>(null);

  const refreshReadModel = useCallback(async (context: RoleReadContext) => {
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
  }, []);

  const initializeReadModel = useCallback(
    async (actorAccountId: AccountId): Promise<void> => {
      await refreshReadModel(buildDefaultRoleReadContext(actorAccountId));
    },
    [refreshReadModel],
  );

  return {
    readModel,
    readContext,
    refreshReadModel,
    initializeReadModel,
  };
}
