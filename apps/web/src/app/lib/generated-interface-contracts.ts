// Generated from Tanren's utoipa OpenAPI contract via:
//   cargo run -q -p tanren-xtask -- export-openapi --out <path>
// and openapi-typescript via apps/web/scripts/generate-interface-contracts.mjs
// Do not hand-edit this file.

export interface paths {
  "/accounts": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    get?: never;
    put?: never;
    /**
     * Self-signup: create a new personal account and mint a cookie-bound
     *     session.
     */
    post: operations["sign_up_route"];
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
  "/accounts/{account_id}/permissions": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    /**
     * Explicitly reject target-account permission introspection. API clients
     *     must use `/me/permissions`; the account id target is always resolved
     *     from the authenticated session.
     */
    get: operations["target_account_permissions_route"];
    put?: never;
    post?: never;
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
  "/health": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    /** Liveness probe. */
    get: operations["health_route"];
    put?: never;
    post?: never;
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
  "/invitations/{token}/accept": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    get?: never;
    put?: never;
    /** Accept an organization invitation and mint a cookie-bound session. */
    post: operations["accept_invitation_route"];
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
  "/me/capabilities": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    /** Read the authenticated account's effective permissions. */
    get: operations["my_permissions_capabilities_route"];
    put?: never;
    post?: never;
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
  "/me/permissions": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    /** Read the authenticated account's effective permissions. */
    get: operations["my_permissions_route"];
    put?: never;
    post?: never;
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
  "/sessions": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    get?: never;
    put?: never;
    /** Sign-in: mint a cookie-bound session for an existing account. */
    post: operations["sign_in_route"];
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
  "/sessions/revoke": {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    get?: never;
    put?: never;
    /**
     * Revoke (sign out) the current session. Clears the cookie via
     *     `Session::flush` and returns 204.
     */
    post: operations["revoke_route"];
    delete?: never;
    options?: never;
    head?: never;
    patch?: never;
    trace?: never;
  };
}
export type webhooks = Record<string, never>;
export interface components {
  schemas: {
    /**
     * @description Path body for `POST /invitations/{token}/accept`. Splits the password
     *     into a `String` here (then re-wraps as `SecretString` before handing
     *     off to app-services) so utoipa can document the schema; the secret
     *     stays in memory only for the lifetime of this function.
     */
    AcceptInvitationBody: {
      /** @description Display name. */
      display_name: string;
      /** @description Email the invitee chose. */
      email: components["schemas"]["Email"];
      /**
       * Format: password
       * @description Plaintext password.
       */
      password: string;
    };
    /** @description Cookie-transport projection of an invitation-acceptance response. */
    AcceptInvitationResponseCookie: {
      /** @description View of the newly created account. */
      account: components["schemas"]["AccountView"];
      /** @description Organization the new account joined. */
      joined_org: components["schemas"]["OrgId"];
      /** @description Cookie-projected session envelope. */
      session: components["schemas"]["SessionEnvelope"];
    };
    /**
     * Format: uuid
     * @description Stable identifier for a Tanren account. `UUIDv7` — sortable + unique.
     */
    AccountId: string;
    /** @description External-facing view of a Tanren account. */
    AccountView: {
      /** @description Display name. */
      display_name: string;
      /** @description Stable account id. */
      id: components["schemas"]["AccountId"];
      /** @description User-facing identifier (email). */
      identifier: components["schemas"]["Identifier"];
      org?: null | components["schemas"]["OrgId"];
    };
    /**
     * Format: email
     * @description Validated email address. Constructed via [`Email::parse`] which:
     *     trims surrounding whitespace, validates against RFC 5322 syntax via
     *     the [`email_validator_rfc5322`] crate (RFC 5321 length limits +
     *     quoted local parts), additionally requires a TLD-style domain (no
     *     dotless or IP-literal domains), and canonicalises to lower-case so
     *     case variants of the same address compare equal.
     *
     *     # Wire-input contract
     *
     *     `Email` does NOT derive `Deserialize` — the custom impl below routes
     *     every wire input through [`parse`](Self::parse). Without this,
     *     `#[serde(transparent)]` would let HTTP/MCP/CLI requests carry
     *     untrimmed/un-lowercased/RFC-invalid addresses, which would persist
     *     verbatim via `Identifier::from_email` and let two case variants of
     *     the same logical email register as separate accounts. Codex P1
     *     review on PR #133.
     *
     *     Validation invariants are exercised end-to-end by the @api / @web
     *     scenarios in `tests/bdd/features/B-0043-create-account.feature` —
     *     case-variant rejection and malformed-email rejection both run
     *     through the live wire surface, not through Rust unit tests.
     */
    Email: string;
    /** @description Liveness response. */
    HealthResponse: {
      /**
       * Format: int32
       * @description Wire-contract version.
       */
      contract_version: number;
      /** @description Static "ok" string. */
      status: string;
      /** @description Build-time package version. */
      version: string;
    };
    /**
     * @description User-facing identifier for an account. R-0001's chosen mechanism is
     *     identifier+password where the identifier is the canonical email; the
     *     type wraps the raw string so future mechanisms can lift constraints
     *     in one place.
     *
     *     `Identifier` does NOT derive `Deserialize` — the custom impl below
     *     routes every wire input through [`parse`](Self::parse) so untrimmed
     *     or differently-cased identifiers cannot bypass canonicalisation.
     *     Validation invariants are exercised end-to-end by the @api / @web
     *     scenarios in `tests/bdd/features/B-0043-create-account.feature`
     *     (case-variant rejection, malformed-input rejection); per the
     *     BDD-only test surface policy there are no Rust unit or doc-tests
     *     for these rules.
     */
    Identifier: string;
    /** @description Shared machine-readable interface error body. */
    InterfaceError: {
      /** @description Stable error code from the shared interfaces taxonomy. */
      code: components["schemas"]["InterfaceErrorCode"];
      /** @description Human-readable summary for the caller. */
      summary: string;
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
    /** @description Capability metadata for the authenticated actor's account surface. */
    MyAccountCapabilitiesResponse: {
      /** @description Whether the current actor may open the self-permissions view. */
      can_view_my_permissions: boolean;
    };
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
    /**
     * @description Optional target-account hint for capability discovery. Omitted in
     *     normal `/me` usage; present only when callers want to verify that a
     *     specific account id is self-scoped under the current session.
     */
    MyPermissionsCapabilityQuery: {
      account_id?: null | components["schemas"]["AccountId"];
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
    /** @description Read metadata returned with self-permission pages. */
    MyPermissionsReadMeta: {
      /**
       * Format: date-time
       * @description Wall-clock instant when this response snapshot was generated.
       */
      generated_at: string;
      /**
       * @description Canonical read source name serving this response.
       *
       *     This endpoint performs a direct read over permission tables. It does
       *     not report projection checkpoint or staleness guarantees.
       */
      source: string;
    };
    /** @description Response payload for self-permission introspection. */
    MyPermissionsResponse: {
      /** @description Organization-scoped permission sections visible to the caller. */
      organizations: components["schemas"]["MyOrganizationPermissions"][];
      /** @description Pagination metadata for this response page. */
      page: components["schemas"]["MyPermissionsPageMeta"];
      /** @description Project-scoped permission sections visible to the caller. */
      projects: components["schemas"]["MyProjectPermissions"][];
      /** @description Read metadata for the returned snapshot. */
      read_metadata: components["schemas"]["MyPermissionsReadMeta"];
    };
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
    /**
     * @description Transport-aware projection of a freshly minted session.
     *
     *     The `@web` and `@api` surfaces deliver session tokens via an
     *     `HttpOnly + Secure + SameSite=Strict` cookie set by the API; the body
     *     only exposes `account_id` + `expires_at` (`Cookie` variant). The
     *     `@cli`, `@mcp`, and `@tui` surfaces have no cookie jar — they receive
     *     the token in the response body (`Bearer` variant). Subsequent PRs map
     *     `SessionView` → `SessionEnvelope` per surface inside each binary
     *     (cookie session lands in PR 8). The discriminator is the transport,
     *     not the user.
     *
     *     See `docs/architecture/subsystems/interfaces.md` § "Canonical session,
     *     error, `OpenAPI`, and design-token decisions" and
     *     `profiles/rust-cargo/architecture/cookie-session.md`.
     */
    SessionEnvelope:
      | {
          /** @description Account this session is bound to. */
          account_id: components["schemas"]["AccountId"];
          /**
           * Format: date-time
           * @description Wall-clock time at which the session expires.
           */
          expires_at: string;
          /** @enum {string} */
          transport: "cookie";
        }
      | {
          /** @description Account this session is bound to. */
          account_id: components["schemas"]["AccountId"];
          /**
           * Format: date-time
           * @description Wall-clock time at which the session expires.
           */
          expires_at: string;
          /** @description Opaque session token. */
          token: components["schemas"]["SessionToken"];
          /** @enum {string} */
          transport: "bearer";
        };
    SessionToken: string;
    /** @description Sign-in request. */
    SignInRequest: {
      /** @description Email of the account being signed in to. */
      email: components["schemas"]["Email"];
      /**
       * Format: password
       * @description Plaintext password — verified against the stored hash.
       */
      password: string;
    };
    /** @description Cookie-transport projection of a sign-in response. */
    SignInResponseCookie: {
      /** @description View of the signed-in account. */
      account: components["schemas"]["AccountView"];
      /** @description Cookie-projected session envelope. */
      session: components["schemas"]["SessionEnvelope"];
    };
    /** @description Self-signup request. */
    SignUpRequest: {
      /** @description Human-readable display name for the new account. */
      display_name: string;
      /**
       * @description Email address that will own the new account. Lower-cased + trimmed
       *     during validation.
       */
      email: components["schemas"]["Email"];
      /**
       * Format: password
       * @description Plaintext password. Hashed by the handler before persistence.
       *     Wrapped in `SecretString` so accidental `Debug` / `Serialize`
       *     calls do not leak the credential.
       */
      password: string;
    };
    /**
     * @description Cookie-transport response shape for the api surface. Mirrors
     *     `SignUpResponse`/`SignInResponse`/`AcceptInvitationResponse` but
     *     projects the session into [`SessionEnvelope::Cookie`] (no token in
     *     body — it ships in the `Set-Cookie` header).
     */
    SignUpResponseCookie: {
      /** @description View of the freshly created account. */
      account: components["schemas"]["AccountView"];
      /** @description Cookie-projected session envelope. */
      session: components["schemas"]["SessionEnvelope"];
    };
  };
  responses: never;
  parameters: never;
  requestBodies: never;
  headers: never;
  pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
  sign_up_route: {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    requestBody: {
      content: {
        "application/json": components["schemas"]["SignUpRequest"];
      };
    };
    responses: {
      /** @description Account created */
      201: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["SignUpResponseCookie"];
        };
      };
      /** @description validation_failed */
      400: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description invalid_credential */
      401: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description duplicate_identifier */
      409: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
    };
  };
  target_account_permissions_route: {
    parameters: {
      query?: {
        /** @description Optional page size hint; values above max are clamped. */
        limit?: number;
        /** @description Opaque continuation token from a prior page. */
        cursor?: string;
      };
      header?: never;
      path: {
        /** @description Target account id (rejected; use /me/permissions) */
        account_id: components["schemas"]["AccountId"];
      };
      cookie?: never;
    };
    requestBody?: never;
    responses: {
      /** @description auth_required */
      401: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description permission_denied */
      403: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description internal_error */
      500: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
    };
  };
  health_route: {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    requestBody?: never;
    responses: {
      /** @description Service is live */
      200: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["HealthResponse"];
        };
      };
    };
  };
  accept_invitation_route: {
    parameters: {
      query?: never;
      header?: never;
      path: {
        /** @description Opaque invitation token */
        token: string;
      };
      cookie?: never;
    };
    requestBody: {
      content: {
        "application/json": components["schemas"]["AcceptInvitationBody"];
      };
    };
    responses: {
      /** @description Invitation accepted */
      201: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["AcceptInvitationResponseCookie"];
        };
      };
      /** @description validation_failed */
      400: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description invitation_not_found */
      404: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description invitation_expired or invitation_already_consumed */
      410: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
    };
  };
  my_permissions_capabilities_route: {
    parameters: {
      query?: {
        /** @description Optional account id to evaluate with self-permissions authorization semantics. */
        account_id?: components["schemas"]["AccountId"];
      };
      header?: never;
      path?: never;
      cookie?: never;
    };
    requestBody?: never;
    responses: {
      /** @description Capability metadata for self-permissions navigation */
      200: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["MyAccountCapabilitiesResponse"];
        };
      };
      /** @description auth_required */
      401: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description permission_denied */
      403: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description internal_error */
      500: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
    };
  };
  my_permissions_route: {
    parameters: {
      query?: {
        /** @description Optional page size hint; values above max are clamped. */
        limit?: number;
        /** @description Opaque continuation token from a prior page. */
        cursor?: string;
      };
      header?: never;
      path?: never;
      cookie?: never;
    };
    requestBody?: never;
    responses: {
      /** @description Self permissions loaded */
      200: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["MyPermissionsResponse"];
        };
      };
      /** @description auth_required */
      401: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description permission_denied */
      403: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description internal_error */
      500: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
    };
  };
  sign_in_route: {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    requestBody: {
      content: {
        "application/json": components["schemas"]["SignInRequest"];
      };
    };
    responses: {
      /** @description Sign-in succeeded */
      200: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["SignInResponseCookie"];
        };
      };
      /** @description validation_failed */
      400: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
      /** @description invalid_credential */
      401: {
        headers: {
          [name: string]: unknown;
        };
        content: {
          "application/json": components["schemas"]["InterfaceError"];
        };
      };
    };
  };
  revoke_route: {
    parameters: {
      query?: never;
      header?: never;
      path?: never;
      cookie?: never;
    };
    requestBody?: never;
    responses: {
      /** @description Session revoked */
      204: {
        headers: {
          [name: string]: unknown;
        };
        content?: never;
      };
    };
  };
}

// Re-export component schemas for ergonomic imports in web client code.
export type AcceptInvitationBody =
  components["schemas"]["AcceptInvitationBody"];
export type AcceptInvitationResponseCookie =
  components["schemas"]["AcceptInvitationResponseCookie"];
export type AccountId = components["schemas"]["AccountId"];
export type AccountView = components["schemas"]["AccountView"];
export type Email = components["schemas"]["Email"];
export type HealthResponse = components["schemas"]["HealthResponse"];
export type Identifier = components["schemas"]["Identifier"];
export type InterfaceError = components["schemas"]["InterfaceError"];
export type InterfaceErrorCode = components["schemas"]["InterfaceErrorCode"];
export type MyAccountCapabilitiesResponse =
  components["schemas"]["MyAccountCapabilitiesResponse"];
export type MyOrganizationPermissions =
  components["schemas"]["MyOrganizationPermissions"];
export type MyPermissionEntry = components["schemas"]["MyPermissionEntry"];
export type MyPermissionsCapabilityQuery =
  components["schemas"]["MyPermissionsCapabilityQuery"];
export type MyPermissionsPageMeta =
  components["schemas"]["MyPermissionsPageMeta"];
export type MyPermissionsReadMeta =
  components["schemas"]["MyPermissionsReadMeta"];
export type MyPermissionsResponse =
  components["schemas"]["MyPermissionsResponse"];
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
export type SessionEnvelope = components["schemas"]["SessionEnvelope"];
export type SessionToken = components["schemas"]["SessionToken"];
export type SignInRequest = components["schemas"]["SignInRequest"];
export type SignInResponseCookie =
  components["schemas"]["SignInResponseCookie"];
export type SignUpRequest = components["schemas"]["SignUpRequest"];
export type SignUpResponseCookie =
  components["schemas"]["SignUpResponseCookie"];

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
