"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import type { RoleAdminAction } from "@/app/lib/generated/role-contract";
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
  hasRoleActionCapability,
  requireRoleActionSnapshot,
  type RoleCapabilitySnapshot,
  type RoleRequestContextInput,
} from "@/app/lib/role-client";

import { RoleOperationForms } from "./RoleOperationForms";
import {
  RoleOperationResultView,
  RoleReadModelView,
} from "./RoleReadModelView";
import type { OperationSummary } from "./RoleReadModelView";
import {
  ROLE_OPERATION_DESCRIPTORS,
  type RoleOperationAction,
  type RoleOperationDescriptor,
  type RoleOperationResponseMap,
  type RoleOperationSubmitStateKey,
} from "./role-action-descriptors";
import { useRoleCapabilities } from "./useRoleCapabilities";
import {
  buildDefaultRoleReadContext,
  isSameRoleReadContext,
  resolveRoleReadContext,
  type RoleReadContext,
  useRoleReadModel,
} from "./useRoleReadModel";

export function RoleWorkbench(): ReactNode {
  const [roleMessage, setRoleMessage] = useState<string>("");
  const [operationSummary, setOperationSummary] =
    useState<OperationSummary | null>(null);
  const [submitState, setSubmitState] = useState<
    Record<RoleOperationSubmitStateKey, boolean>
  >(buildSubmitState(false));
  const submitInFlightRef = useRef<
    Record<RoleOperationSubmitStateKey, boolean>
  >(buildSubmitState(false));
  const {
    snapshot: capabilitySnapshot,
    capabilities,
    errorMessage: capabilityError,
  } = useRoleCapabilities();
  const {
    readModel,
    readContext,
    roleNextCursor,
    grantNextCursor,
    isRefreshing,
    isLoadingMoreRoles,
    isLoadingMoreGrants,
    refreshReadModel,
    initializeReadModel,
    loadMoreRoleTemplates,
    loadMoreDirectGrants,
    upsertLocalRoleTemplate,
    removeLocalRoleTemplate,
    appendLocalDirectGrants,
  } = useRoleReadModel(capabilitySnapshot);
  const canReadRoles = hasRoleActionCapability(capabilities, "read_roles");

  useEffect(() => {
    if (capabilityError !== null) {
      setRoleMessage(`capabilities: ${capabilityError}`);
    }
  }, [capabilityError]);

  useEffect(() => {
    if (capabilitySnapshot === null) {
      return;
    }
    if (!canReadRoles) {
      return;
    }
    void initializeReadModel(
      capabilitySnapshot.capabilities.actor.account_id,
    ).catch((reason: unknown) => {
      setRoleMessage(`read model: ${formatRoleError(reason)}`);
    });
  }, [canReadRoles, capabilitySnapshot, initializeReadModel]);

  const runRoleAction: RunRoleAction = async (
    descriptor,
    action: () => Promise<void>,
  ): Promise<void> => {
    if (submitInFlightRef.current[descriptor.submitStateKey]) {
      return;
    }
    submitInFlightRef.current[descriptor.submitStateKey] = true;
    setSubmitState((current) => ({
      ...current,
      [descriptor.submitStateKey]: true,
    }));
    try {
      await action();
      setRoleMessage(`${descriptor.label}: ok`);
    } catch (reason: unknown) {
      setRoleMessage(`${descriptor.label}: ${formatRoleError(reason)}`);
    } finally {
      submitInFlightRef.current[descriptor.submitStateKey] = false;
      setSubmitState((current) => ({
        ...current,
        [descriptor.submitStateKey]: false,
      }));
    }
  };

  const onRefreshReadModel = useCallback((): void => {
    let snapshot: RoleCapabilitySnapshot;
    try {
      snapshot = requireActionSnapshot(capabilitySnapshot, "read_roles");
    } catch (reason: unknown) {
      setRoleMessage(`read model: ${formatRoleError(reason)}`);
      return;
    }
    const actorAccountId = snapshot.capabilities.actor.account_id;
    const context = readContext ?? buildDefaultRoleReadContext(actorAccountId);
    void refreshReadModel(context).catch((reason: unknown) => {
      setRoleMessage(`read model: ${formatRoleError(reason)}`);
    });
  }, [capabilitySnapshot, readContext, refreshReadModel]);

  const onLoadMoreRoles = useCallback((): void => {
    try {
      requireActionSnapshot(capabilitySnapshot, "read_roles");
    } catch (reason: unknown) {
      setRoleMessage(`load more roles: ${formatRoleError(reason)}`);
      return;
    }
    void loadMoreRoleTemplates().catch((reason: unknown) => {
      setRoleMessage(`load more roles: ${formatRoleError(reason)}`);
    });
  }, [capabilitySnapshot, loadMoreRoleTemplates]);

  const onLoadMoreGrants = useCallback((): void => {
    try {
      requireActionSnapshot(capabilitySnapshot, "read_roles");
    } catch (reason: unknown) {
      setRoleMessage(`load more grants: ${formatRoleError(reason)}`);
      return;
    }
    void loadMoreDirectGrants().catch((reason: unknown) => {
      setRoleMessage(`load more grants: ${formatRoleError(reason)}`);
    });
  }, [capabilitySnapshot, loadMoreDirectGrants]);

  const onCreateRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.create_role,
      capabilitySnapshot,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildCreateRoleRequest(new FormData(event.currentTarget)),
      (request, snapshot) => createRole(request, snapshot),
      (context, response) => upsertLocalRoleTemplate(context, response.role),
      runRoleAction,
    );
  };

  const onEditRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.edit_role,
      capabilitySnapshot,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildEditRoleRequest(new FormData(event.currentTarget)),
      (request, snapshot) => editRole(request, snapshot),
      (context, response) => upsertLocalRoleTemplate(context, response.role),
      runRoleAction,
    );
  };

  const onDeleteRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.delete_role,
      capabilitySnapshot,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildDeleteRoleRequest(new FormData(event.currentTarget)),
      (request, snapshot) => deleteRole(request, snapshot),
      (context, response) =>
        removeLocalRoleTemplate(context, response.role.role_id),
      runRoleAction,
    );
  };

  const onApplyRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.apply_role,
      capabilitySnapshot,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildApplyRoleRequest(new FormData(event.currentTarget)),
      (request, snapshot) => applyRole(request, snapshot),
      (context, response) => appendLocalDirectGrants(context, response.grants),
      runRoleAction,
    );
  };

  const onCheckPermission = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const descriptor = ROLE_OPERATION_DESCRIPTORS.check_permission;
    void runRoleAction(descriptor, async () => {
      const snapshot = requireActionSnapshot(
        capabilitySnapshot,
        descriptor.capability,
      );
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
          ? await checkPermissionRolePrincipalRejection(
              submission.request,
              snapshot,
            )
          : await checkPermission(submission.request, snapshot);
      setOperationSummary(descriptor.buildSummary(response));
      if (shouldRefreshReadModel(readContext, context)) {
        await refreshReadModel(context);
      }
    });
  };

  return (
    <>
      <RoleOperationForms
        capabilities={capabilities}
        submitState={submitState}
        onSubmitByAction={{
          create_role: onCreateRole,
          edit_role: onEditRole,
          delete_role: onDeleteRole,
          apply_role: onApplyRole,
          check_permission: onCheckPermission,
        }}
      />

      <RoleOperationResultView
        message={roleMessage}
        summary={operationSummary}
      />

      <RoleReadModelView
        canReadRoles={canReadRoles}
        readModel={readModel}
        roleNextCursor={roleNextCursor}
        grantNextCursor={grantNextCursor}
        isRefreshing={isRefreshing}
        isLoadingMoreRoles={isLoadingMoreRoles}
        isLoadingMoreGrants={isLoadingMoreGrants}
        onRefreshReadModel={onRefreshReadModel}
        onLoadMoreRoles={onLoadMoreRoles}
        onLoadMoreGrants={onLoadMoreGrants}
      />
    </>
  );
}

function buildSubmitState(
  value: boolean,
): Record<RoleOperationSubmitStateKey, boolean> {
  return {
    create_role: value,
    edit_role: value,
    delete_role: value,
    apply_role: value,
    check_permission: value,
  };
}

async function runRoleMutation<
  TAction extends Exclude<RoleAdminAction, "check_permission" | "read_roles">,
  TSubmission extends { context: RoleRequestContextInput; request: unknown },
>(
  descriptor: RoleOperationDescriptor<TAction>,
  capabilitySnapshot: RoleCapabilitySnapshot | null,
  readContext: RoleReadContext | null,
  refreshReadModel: (context: RoleReadContext) => Promise<void>,
  setOperationSummary: (summary: OperationSummary) => void,
  buildSubmission: () => TSubmission,
  execute: (
    request: TSubmission["request"],
    snapshot: RoleCapabilitySnapshot,
  ) => Promise<RoleOperationResponseMap[TAction]>,
  applyLocalMutation: (
    context: RoleReadContext,
    response: RoleOperationResponseMap[TAction],
  ) => boolean,
  runRoleAction: RunRoleAction,
): Promise<void> {
  return runRoleAction(descriptor, async () => {
    const snapshot = requireActionSnapshot(
      capabilitySnapshot,
      descriptor.capability,
    );
    const submission = buildSubmission();
    const context = resolveRoleReadContext(
      readContext,
      snapshot.capabilities.actor.account_id,
      submission.context,
    );
    const response = await execute(submission.request, snapshot);
    setOperationSummary(descriptor.buildSummary(response));
    if (shouldRefreshReadModel(readContext, context)) {
      await refreshReadModel(context);
      return;
    }
    if (!applyLocalMutation(context, response)) {
      await refreshReadModel(context);
    }
  });
}

type RunRoleAction = <TAction extends RoleOperationAction>(
  descriptor: RoleOperationDescriptor<TAction>,
  action: () => Promise<void>,
) => Promise<void>;

function shouldRefreshReadModel(
  readContext: RoleReadContext | null,
  nextContext: RoleReadContext,
): boolean {
  return (
    readContext === null || !isSameRoleReadContext(readContext, nextContext)
  );
}

function requireActionSnapshot<TAction extends RoleAdminAction>(
  actorSnapshot: RoleCapabilitySnapshot | null,
  action: TAction,
): RoleCapabilitySnapshot {
  return requireRoleActionSnapshot(actorSnapshot, action);
}
