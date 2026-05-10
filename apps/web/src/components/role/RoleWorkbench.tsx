"use client";

import { useEffect, useState } from "react";
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
  type RoleRequestContextInput,
  type RoleCapabilitySnapshot,
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

  const runRoleAction: RunRoleAction = async (
    descriptor,
    action: () => Promise<void>,
  ): Promise<void> => {
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
      setSubmitState((current) => ({
        ...current,
        [descriptor.submitStateKey]: false,
      }));
    }
  };

  const onCreateRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.create_role,
      capabilitySnapshot,
      csrfToken,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildCreateRoleRequest(new FormData(event.currentTarget)),
      (request, token) => createRole(request, token),
      runRoleAction,
    );
  };

  const onEditRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.edit_role,
      capabilitySnapshot,
      csrfToken,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildEditRoleRequest(new FormData(event.currentTarget)),
      (request, token) => editRole(request, token),
      runRoleAction,
    );
  };

  const onDeleteRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.delete_role,
      capabilitySnapshot,
      csrfToken,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildDeleteRoleRequest(new FormData(event.currentTarget)),
      (request, token) => deleteRole(request, token),
      runRoleAction,
    );
  };

  const onApplyRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    void runRoleMutation(
      ROLE_OPERATION_DESCRIPTORS.apply_role,
      capabilitySnapshot,
      csrfToken,
      readContext,
      refreshReadModel,
      setOperationSummary,
      () => buildApplyRoleRequest(new FormData(event.currentTarget)),
      (request, token) => applyRole(request, token),
      runRoleAction,
    );
  };

  const onCheckPermission = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const descriptor = ROLE_OPERATION_DESCRIPTORS.check_permission;
    void runRoleAction(descriptor, async () => {
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
      setOperationSummary(descriptor.buildSummary(response));
      await refreshReadModel(context);
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

      <RoleReadModelView readModel={readModel} />
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
  csrfToken: string | null,
  readContext: RoleReadContext | null,
  refreshReadModel: (context: RoleReadContext) => Promise<void>,
  setOperationSummary: (summary: OperationSummary) => void,
  buildSubmission: () => TSubmission,
  execute: (
    request: TSubmission["request"],
    csrfToken: string,
  ) => Promise<RoleOperationResponseMap[TAction]>,
  runRoleAction: RunRoleAction,
): Promise<void> {
  return runRoleAction(descriptor, async () => {
    const token = requireCsrfToken(csrfToken);
    const snapshot = requireCapabilitySnapshot(capabilitySnapshot);
    const submission = buildSubmission();
    const context = resolveRoleReadContext(
      readContext,
      snapshot.capabilities.actor.account_id,
      submission.context,
    );
    const response = await execute(submission.request, token);
    setOperationSummary(descriptor.buildSummary(response));
    await refreshReadModel(context);
  });
}

type RunRoleAction = <TAction extends RoleOperationAction>(
  descriptor: RoleOperationDescriptor<TAction>,
  action: () => Promise<void>,
) => Promise<void>;

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
