// Generated from Tanren's utoipa OpenAPI contract via:
//   cargo run -q -p tanren-xtask -- export-openapi --out <path>
// and openapi-typescript via apps/web/scripts/generate-interface-contracts.mjs
// Do not hand-edit this file.

export type paths = Record<string, never>;
export type webhooks = Record<string, never>;
export interface components {
  schemas: {
    /** @description Shared machine-readable interface error body. */
    InterfaceError: {
      /** @description Stable error code from the shared interfaces taxonomy. */
      code: components["schemas"]["InterfaceErrorCode"];
      /** @description Human-readable summary for the caller. */
      summary: string;
    };
    /** @description Capability metadata for the authenticated actor's account surface. */
    MyAccountCapabilitiesResponse: {
      /** @description Whether the current actor may open the self-permissions view. */
      can_view_my_permissions: boolean;
    };
    /** @description Response payload for self-permission introspection. */
    MyPermissionsResponse: {
      /** @description Read-model freshness metadata for the returned snapshot. */
      freshness: components["schemas"]["MyPermissionsFreshnessMeta"];
      /** @description Organization-scoped permission sections visible to the caller. */
      organizations: components["schemas"]["MyOrganizationPermissions"][];
      /** @description Pagination metadata for this response page. */
      page: components["schemas"]["MyPermissionsPageMeta"];
      /** @description Project-scoped permission sections visible to the caller. */
      projects: components["schemas"]["MyProjectPermissions"][];
    };
    /**
     * @description Shared interfaces error-code taxonomy.
     * @enum {string}
     */
    InterfaceErrorCode:
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
    /** @description Organization-level permission section for the current caller. */
    MyOrganizationPermissions: {
      /** @description Organization these effective permissions are scoped to. */
      org_id: components["schemas"]["OrgId"];
      /** @description Effective permission entries for this organization. */
      permissions: components["schemas"]["MyPermissionEntry"][];
    };
    /** @description One effective permission entry shown in the self-introspection view. */
    MyPermissionEntry: {
      /** @description Effective state after policy is applied. */
      effective_state: components["schemas"]["PermissionEffectiveState"];
      /** @description How this permission was granted. */
      grant_source: components["schemas"]["PermissionGrantSource"];
      /** @description Stable proof/source reference for how this grant was derived. */
      grant_source_reference: string;
      /** @description Canonical permission identifier. */
      permission: components["schemas"]["PermissionName"];
      policy_constraint?:
        | null
        | components["schemas"]["PermissionConstraintView"];
    };
    /** @description Read-model freshness metadata returned with self-permission pages. */
    MyPermissionsFreshnessMeta: {
      /** @description Source checkpoint identifier for the returned read-model slice. */
      checkpoint?: string | null;
      /**
       * Format: date-time
       * @description Wall-clock instant when this response snapshot was generated.
       */
      generated_at: string;
      /** @description Canonical read-model/projection name serving this response. */
      projection: string;
      /** @description Whether the read-model is stale relative to requested freshness. */
      staleness: components["schemas"]["MyPermissionsStaleness"];
    };
    /** @description Pagination metadata for a self-permissions response page. */
    MyPermissionsPageMeta: {
      /**
       * Format: int32
       * @description Maximum number of permission entries requested for this page.
       */
      limit: number;
      /** @description Cursor to fetch the next page; null when no continuation exists. */
      next_cursor?: string | null;
      /** @description Cursor that produced this page; omitted for first-page reads. */
      request_cursor?: string | null;
      /**
       * Format: int32
       * @description Number of permission entries included in this page.
       */
      returned: number;
    };
    /**
     * @description Staleness classification for a read-model response.
     * @enum {string}
     */
    MyPermissionsStaleness: "fresh" | "stale";
    /** @description Project-level permission section for the current caller. */
    MyProjectPermissions: {
      /** @description Effective permission entries for this project. */
      permissions: components["schemas"]["MyPermissionEntry"][];
      /** @description Project these effective permissions are scoped to. */
      project_id: components["schemas"]["ProjectId"];
    };
    /**
     * Format: uuid
     * @description Stable identifier for a Tanren organization.
     */
    OrgId: string;
    /** @description Optional policy-constraint detail for a permission entry. */
    PermissionConstraintView: {
      /** @description Human-readable reason associated with the constraint. */
      reason: components["schemas"]["PolicyConstraintReason"];
      /** @description Scope that produced the constraint. */
      source: components["schemas"]["PolicyConstraintSource"];
      /** @description Stable proof/source reference for the constraining policy record. */
      source_reference: string;
    };
    /**
     * @description Effective runtime state for a permission after policy has been applied.
     * @enum {string}
     */
    PermissionEffectiveState: "granted" | "constrained";
    /** @description How a permission grant entered the actor's authorization set. */
    PermissionGrantSource:
      | {
          /** @enum {string} */
          kind: "direct";
        }
      | {
          /** @enum {string} */
          kind: "role_template";
          /** @description Template that produced this grant. */
          role_template: components["schemas"]["RoleTemplateName"];
        };
    /** @description Canonical permission identifier used by policy and interfaces. */
    PermissionName: string;
    /** @description Human-readable reason describing why policy constrained a grant. */
    PolicyConstraintReason: string;
    /**
     * @description Policy scope that produced a constraint on a permission grant.
     * @enum {string}
     */
    PolicyConstraintSource: "organization_policy" | "project_policy";
    /**
     * Format: uuid
     * @description Stable identifier for a Tanren project.
     */
    ProjectId: string;
    /** @description Name of a role template used as the source of a permission grant. */
    RoleTemplateName: string;
  };
  responses: never;
  parameters: never;
  requestBodies: never;
  headers: never;
  pathItems: never;
}
export type $defs = Record<string, never>;
export type operations = Record<string, never>;

// Re-export only interface-contract schemas reachable from /me endpoints.
export type InterfaceError = components["schemas"]["InterfaceError"];
export type MyAccountCapabilitiesResponse =
  components["schemas"]["MyAccountCapabilitiesResponse"];
export type MyPermissionsResponse =
  components["schemas"]["MyPermissionsResponse"];
export type InterfaceErrorCode = components["schemas"]["InterfaceErrorCode"];
export type MyOrganizationPermissions =
  components["schemas"]["MyOrganizationPermissions"];
export type MyPermissionEntry = components["schemas"]["MyPermissionEntry"];
export type MyPermissionsFreshnessMeta =
  components["schemas"]["MyPermissionsFreshnessMeta"];
export type MyPermissionsPageMeta =
  components["schemas"]["MyPermissionsPageMeta"];
export type MyPermissionsStaleness =
  components["schemas"]["MyPermissionsStaleness"];
export type MyProjectPermissions =
  components["schemas"]["MyProjectPermissions"];
export type OrgId = components["schemas"]["OrgId"];
export type PermissionConstraintView =
  components["schemas"]["PermissionConstraintView"];
export type PermissionEffectiveState =
  components["schemas"]["PermissionEffectiveState"];
export type PermissionGrantSource =
  components["schemas"]["PermissionGrantSource"];
export type PermissionName = components["schemas"]["PermissionName"];
export type PolicyConstraintReason =
  components["schemas"]["PolicyConstraintReason"];
export type PolicyConstraintSource =
  components["schemas"]["PolicyConstraintSource"];
export type ProjectId = components["schemas"]["ProjectId"];
export type RoleTemplateName = components["schemas"]["RoleTemplateName"];

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
