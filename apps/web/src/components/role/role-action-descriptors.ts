import type {
  ApplyRoleResponse,
  CreateRoleResponse,
  DeleteRoleResponse,
  EditRoleResponse,
  PermissionCheckResponse,
  RoleAdminAction,
  RoleScope,
} from "@/app/lib/generated/role-contract";

import type { OperationSummary } from "./RoleReadModelView";

export type RoleOperationAction = Exclude<RoleAdminAction, "read_roles">;

export type RoleOperationSubmitStateKey =
  | "create_role"
  | "edit_role"
  | "delete_role"
  | "apply_role"
  | "check_permission";

export interface RoleOperationResponseMap {
  create_role: CreateRoleResponse;
  edit_role: EditRoleResponse;
  delete_role: DeleteRoleResponse;
  apply_role: ApplyRoleResponse;
  check_permission: PermissionCheckResponse;
}

export interface RoleOperationDescriptor<TAction extends RoleOperationAction> {
  action: TAction;
  capability: TAction;
  label: string;
  title: string;
  submitStateKey: RoleOperationSubmitStateKey;
  buildSummary: (
    response: RoleOperationResponseMap[TAction],
  ) => OperationSummary;
}

export type AnyRoleOperationDescriptor = {
  [TAction in RoleOperationAction]: RoleOperationDescriptor<TAction>;
}[RoleOperationAction];

export const ROLE_OPERATION_DESCRIPTORS: {
  [TAction in RoleOperationAction]: RoleOperationDescriptor<TAction>;
} = {
  create_role: {
    action: "create_role",
    capability: "create_role",
    label: "create role",
    title: "Create role",
    submitStateKey: "create_role",
    buildSummary: (response) => ({
      label: "Create role",
      lines: [
        `role: ${response.role.id}`,
        `name: ${response.role.name}`,
        `permissions: ${response.role.permissions.length}`,
      ],
    }),
  },
  edit_role: {
    action: "edit_role",
    capability: "edit_role",
    label: "edit role",
    title: "Edit role",
    submitStateKey: "edit_role",
    buildSummary: (response) => ({
      label: "Edit role",
      lines: [
        `role: ${response.role.id}`,
        `name: ${response.role.name}`,
        `permissions: ${response.role.permissions.length}`,
      ],
    }),
  },
  delete_role: {
    action: "delete_role",
    capability: "delete_role",
    label: "delete role",
    title: "Delete role",
    submitStateKey: "delete_role",
    buildSummary: (response) => ({
      label: "Delete role",
      lines: [
        `role: ${response.role.role_id}`,
        `scope: ${formatScopeLabel(response.role.scope)}`,
      ],
    }),
  },
  apply_role: {
    action: "apply_role",
    capability: "apply_role",
    label: "apply role",
    title: "Apply role",
    submitStateKey: "apply_role",
    buildSummary: (response) => ({
      label: "Apply role",
      lines: [
        `role: ${response.role.role_id}`,
        `grants created: ${response.grants.length}`,
        `permissions: ${summarizeGrantPermissions(response.grants.map((grant) => grant.permission))}`,
      ],
    }),
  },
  check_permission: {
    action: "check_permission",
    capability: "check_permission",
    label: "check permission",
    title: "Check permission",
    submitStateKey: "check_permission",
    buildSummary: (response) => ({
      label: "Permission check",
      lines: [
        `allowed: ${String(response.allowed)}`,
        `matching grants: ${response.matching_grant_ids.length}`,
      ],
    }),
  },
};

export function assertNever(value: never, context: string): never {
  throw new Error(`${context} received unsupported variant: ${String(value)}`);
}

function formatScopeLabel(scope: RoleScope): string {
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
