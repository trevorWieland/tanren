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

interface RoleOperationFormsProps {
  capabilities: RoleAdminCapabilities | null;
  onCreateRole: FormEventHandler<HTMLFormElement>;
  onEditRole: FormEventHandler<HTMLFormElement>;
  onDeleteRole: FormEventHandler<HTMLFormElement>;
  onApplyRole: FormEventHandler<HTMLFormElement>;
  onCheckPermission: FormEventHandler<HTMLFormElement>;
}

export function RoleOperationForms(props: RoleOperationFormsProps): ReactNode {
  return (
    <section className="grid gap-4 md:grid-cols-2">
      {hasRoleCapability(props.capabilities, "create_role") ? (
        <RoleCard title="Create role" onSubmit={props.onCreateRole}>
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

      {hasRoleCapability(props.capabilities, "edit_role") ? (
        <RoleCard title="Edit role" onSubmit={props.onEditRole}>
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

      {hasRoleCapability(props.capabilities, "delete_role") ? (
        <RoleCard title="Delete role" onSubmit={props.onDeleteRole}>
          <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
          <ScopeFields prefix="scope_" />
        </RoleCard>
      ) : null}

      {hasRoleCapability(props.capabilities, "apply_role") ? (
        <RoleCard title="Apply role" onSubmit={props.onApplyRole}>
          <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
          <ScopeFields prefix="role_scope_" legend="Role scope" />
          <PrincipalFields prefix="principal_" />
          <ScopeFields prefix="grant_scope_" legend="Grant scope" />
        </RoleCard>
      ) : null}

      {hasRoleCapability(props.capabilities, "check_permission") ? (
        <RoleCard title="Check permission" onSubmit={props.onCheckPermission}>
          <PrincipalFields prefix="principal_" allowRolePrincipal />
          <LabeledInput
            name="permission"
            label="Permission"
            placeholder="accounts.read"
          />
          <ScopeFields prefix="scope_" />
        </RoleCard>
      ) : null}

      {renderCapabilityState(props.capabilities)}
    </section>
  );
}

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
