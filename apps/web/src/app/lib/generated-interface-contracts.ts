// Generated from Tanren's utoipa OpenAPI contract via:
//   cargo run -q -p tanren-xtask -- export-openapi --out <path>
// and apps/web/scripts/generate-interface-contracts.mjs
// Do not hand-edit this file.

export interface MyAccountCapabilitiesResponse {
  can_view_my_permissions: boolean;
}
export type PermissionGrantSource =
  | {
      kind: "direct";
    }
  | {
      kind: "role_template";
      role_template: RoleTemplateName;
    };
export interface PermissionConstraintView {
  reason: PolicyConstraintReason;
  source: PolicyConstraintSource;
}
export interface MyPermissionEntry {
  effective_state: PermissionEffectiveState;
  grant_source: PermissionGrantSource;
  permission: PermissionName;
  policy_constraint?: null | PermissionConstraintView;
}
export interface MyOrganizationPermissions {
  org_id: OrgId;
  permissions: MyPermissionEntry[];
}
export interface MyProjectPermissions {
  permissions: MyPermissionEntry[];
  project_id: ProjectId;
}
export interface MyPermissionsPageMeta {
  limit: number;
  next_cursor?: string | null;
  request_cursor?: string | null;
  returned: number;
}
export interface MyPermissionsResponse {
  freshness: MyPermissionsFreshnessMeta;
  organizations: MyOrganizationPermissions[];
  page: MyPermissionsPageMeta;
  projects: MyProjectPermissions[];
}
export type InterfaceErrorCode =
  | "auth_required"
  | "permission_denied"
  | "validation_failed"
  | "not_found"
  | "conflict"
  | "idempotency_conflict"
  | "stale_projection"
  | "drift_detected"
  | "rate_limited"
  | "unavailable"
  | "unsupported_action"
  | "provider_failure"
  | "execution_failure"
  | "internal_error"
  | "duplicate_identifier"
  | "invalid_credential"
  | "invitation_not_found"
  | "invitation_expired"
  | "invitation_already_consumed";
export interface InterfaceError {
  code: InterfaceErrorCode;
  summary: string;
}
export interface MyPermissionsFreshnessMeta {
  checkpoint?: string | null;
  generated_at: string;
  projection: string;
  staleness: MyPermissionsStaleness;
}
export type MyPermissionsStaleness = "fresh" | "stale";
export type OrgId = string;
export type PermissionEffectiveState = "granted" | "constrained";
export type PermissionName = string;
export type PolicyConstraintReason = string;
export type PolicyConstraintSource = "organization_policy" | "project_policy";
export type ProjectId = string;
export type RoleTemplateName = string;

export const INTERFACE_ERROR_CODES = [
  "auth_required",
  "permission_denied",
  "validation_failed",
  "not_found",
  "conflict",
  "idempotency_conflict",
  "stale_projection",
  "drift_detected",
  "rate_limited",
  "unavailable",
  "unsupported_action",
  "provider_failure",
  "execution_failure",
  "internal_error",
  "duplicate_identifier",
  "invalid_credential",
  "invitation_not_found",
  "invitation_expired",
  "invitation_already_consumed",
] as const;

const INTERFACE_ERROR_CODE_SET = new Set<string>(INTERFACE_ERROR_CODES);

export function isInterfaceErrorCode(
  value: string,
): value is InterfaceErrorCode {
  return INTERFACE_ERROR_CODE_SET.has(value);
}
