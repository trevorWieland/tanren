// Generated from Tanren's OpenAPI contract. Do not hand-edit.

export type PermissionGrantSource =
  | { kind: "direct" }
  | { kind: "role_template"; role_template: string };

export interface PermissionConstraintView {
  reason: string;
  source: "organization_policy" | "project_policy";
}

export interface MyPermissionEntry {
  permission: string;
  effective_state: "granted" | "constrained";
  grant_source: PermissionGrantSource;
  policy_constraint: PermissionConstraintView | null;
}

export interface MyOrganizationPermissions {
  org_id: string;
  permissions: MyPermissionEntry[];
}

export interface MyProjectPermissions {
  project_id: string;
  permissions: MyPermissionEntry[];
}

export interface MyPermissionsPageMeta {
  limit: number;
  returned: number;
}

export interface MyPermissionsResponse {
  page: MyPermissionsPageMeta;
  organizations: MyOrganizationPermissions[];
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
  code: InterfaceErrorCode | string;
  summary: string;
}
