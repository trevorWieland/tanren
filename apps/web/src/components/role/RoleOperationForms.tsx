import type { FormEventHandler, ReactNode } from "react";

import type {
  RoleAdminAction,
  RoleAdminCapabilities,
} from "@/app/lib/generated/role-contract";

import {
  LabeledInput,
  PrincipalFields,
  RoleCard,
  ScopeFields,
} from "./RoleFormFields";
import {
  assertNever,
  ROLE_OPERATION_DESCRIPTORS,
  type RoleOperationAction,
  type RoleOperationSubmitStateKey,
} from "./role-action-descriptors";

interface RoleOperationFormsProps {
  capabilities: RoleAdminCapabilities | null;
  onSubmitByAction: Record<
    RoleOperationAction,
    FormEventHandler<HTMLFormElement>
  >;
  submitState: Record<RoleOperationSubmitStateKey, boolean>;
}

export function RoleOperationForms(props: RoleOperationFormsProps): ReactNode {
  return (
    <section className="grid gap-4 md:grid-cols-2">
      {ROLE_OPERATION_ORDER.map((action) => {
        const descriptor = ROLE_OPERATION_DESCRIPTORS[action];
        if (!hasRoleCapability(props.capabilities, descriptor.capability)) {
          return null;
        }
        const isSubmitting = props.submitState[descriptor.submitStateKey];
        return (
          <RoleCard
            key={descriptor.action}
            title={descriptor.title}
            onSubmit={props.onSubmitByAction[descriptor.action]}
            submitLabel={isSubmitting ? "Running..." : "Run"}
            submitDisabled={isSubmitting}
          >
            {renderOperationFields(descriptor.action)}
          </RoleCard>
        );
      })}

      {renderCapabilityState(props.capabilities)}
    </section>
  );
}

const ROLE_OPERATION_ORDER: readonly RoleOperationAction[] = [
  "create_role",
  "edit_role",
  "delete_role",
  "apply_role",
  "check_permission",
];

function hasRoleCapability(
  capabilities: RoleAdminCapabilities | null,
  action: RoleAdminAction,
): boolean {
  return capabilities?.actions.includes(action) ?? false;
}

function renderCapabilityState(
  capabilities: RoleAdminCapabilities | null,
): ReactNode {
  if (capabilities === null) {
    return (
      <div className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
        Loading role capabilities...
      </div>
    );
  }

  if (capabilities.actions.length === 0) {
    return (
      <div className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
        This account is authenticated but lacks role administration
        capabilities.
      </div>
    );
  }

  return null;
}

function renderOperationFields(action: RoleOperationAction): ReactNode {
  switch (action) {
    case "create_role":
      return (
        <>
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
        </>
      );
    case "edit_role":
      return (
        <>
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
        </>
      );
    case "delete_role":
      return (
        <>
          <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
          <ScopeFields prefix="scope_" />
        </>
      );
    case "apply_role":
      return (
        <>
          <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
          <ScopeFields prefix="role_scope_" legend="Role scope" />
          <PrincipalFields prefix="principal_" />
          <ScopeFields prefix="grant_scope_" legend="Grant scope" />
        </>
      );
    case "check_permission":
      return (
        <>
          <PrincipalFields prefix="principal_" allowRolePrincipal />
          <LabeledInput
            name="permission"
            label="Permission"
            placeholder="accounts.read"
          />
          <ScopeFields prefix="scope_" />
        </>
      );
    default:
      return assertNever(action, "role operation action");
  }
}
