//! Organization command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers create or manage a Tanren
//! organization. They live in `tanren-contract` because every interface
//! binary serialises the same shapes — keeping them here is the
//! architectural guarantee that the surfaces stay equivalent.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, MembershipId, OrgId, OrgPermission, OrganizationName, SessionToken,
};
use utoipa::ToSchema;

/// Create-organization request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationRequest {
    /// Human-readable name for the new organization. Trimmed and
    /// case-folded during validation.
    pub name: OrganizationName,
    /// Caller-supplied idempotency key. Two requests that share the
    /// same key, account, and canonical name return the same result
    /// without duplicate projection rows or duplicate canonical events.
    /// When `None` the handler generates a fresh key so the request
    /// proceeds normally but is not retry-safe.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// Successful create-organization response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationResponse {
    /// View of the freshly created organization.
    pub organization: OrganizationView,
    /// Membership linking the creator to the organization with full
    /// bootstrap admin grants.
    pub membership: OrganizationMembershipView,
}

/// External-facing view of a Tanren organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationView {
    /// Stable organization id.
    pub id: OrgId,
    /// Canonical (trimmed + case-folded) organization name.
    pub name: OrganizationName,
    /// Wall-clock time the organization was created.
    pub created_at: DateTime<Utc>,
}

/// External-facing view of an organization membership.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationMembershipView {
    /// Stable membership id.
    pub id: MembershipId,
    /// Account that holds this membership.
    pub account_id: AccountId,
    /// Organization this membership belongs to.
    pub org_id: OrgId,
    /// Administrative permissions granted by this membership.
    pub permissions: Vec<OrgPermission>,
    /// Wall-clock time the membership was created.
    pub created_at: DateTime<Utc>,
}

/// Administrative operation on an organization.
///
/// Mirrors [`OrgPermission`]
/// variants at the wire level so callers can refer to operations by
/// name without importing the domain enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrganizationAdminOperation {
    /// Invite new members.
    InviteMembers,
    /// Manage member access.
    ManageAccess,
    /// Configure organization settings.
    Configure,
    /// Set organization policies.
    SetPolicy,
    /// Delete the organization.
    Delete,
}

impl OrganizationAdminOperation {
    /// Stable wire name for this operation (`snake_case`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InviteMembers => "invite_members",
            Self::ManageAccess => "manage_access",
            Self::Configure => "configure",
            Self::SetPolicy => "set_policy",
            Self::Delete => "delete",
        }
    }

    /// Resolve the [`OrgPermission`] required to perform this operation.
    ///
    /// Delegates to the authoritative policy function
    /// [`tanren_identity_policy::resolve_admin_operation_permission`].
    /// Returns `None` for unknown or future operations, which callers
    /// must treat as "permission denied."
    #[must_use]
    pub fn required_permission(self) -> Option<OrgPermission> {
        tanren_identity_policy::resolve_admin_operation_permission(self.as_str())
    }
}

/// Closed taxonomy of organization-flow failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects an `OrganizationFailureReason`
/// into the same wire shape so callers can match on `code` regardless of
/// transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrganizationFailureReason {
    /// The caller is not authenticated.
    AuthRequired,
    /// The caller lacks permission for the requested operation.
    PermissionDenied,
    /// An organization with the requested name already exists.
    DuplicateOrganizationName,
    /// User-supplied input failed validation before any verification
    /// could run (empty name, ...).
    ValidationFailed,
    /// The requested organization does not exist.
    NotFound,
    /// The operation would remove the last administrative-permission
    /// holder, violating the invariant that every organization must
    /// retain at least one admin.
    LastAdminHolder,
    /// An idempotency key was reused with a different account or name.
    IdempotencyConflict,
}

/// Closed taxonomy of organization-flow event kinds.
///
/// Each variant maps to a typed event payload struct in this module. The
/// kind serialises to the JSON envelope's `kind` field so log consumers
/// can filter without parsing the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrganizationEventKind {
    /// An organization was created and bootstrap admin permissions were
    /// granted to the creator.
    OrganizationCreated,
    /// An organization-creation attempt was rejected — duplicate name,
    /// validation failure, or other taxonomy reason.
    OrganizationCreationRejected,
}

impl OrganizationEventKind {
    /// Stable wire `kind` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrganizationCreated => "organization_created",
            Self::OrganizationCreationRejected => "organization_creation_rejected",
        }
    }
}

/// Event payload emitted when an organization is created.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationCreatedEvent {
    /// Stable organization id.
    pub org_id: OrgId,
    /// Account that created the organization.
    pub creator_account_id: AccountId,
    /// Canonical (trimmed + case-folded) organization name.
    pub canonical_name: String,
    /// Bootstrap admin permissions granted to the creator.
    pub granted_permissions: Vec<OrgPermission>,
    /// Wall-clock time the organization was created.
    pub at: DateTime<Utc>,
}

/// Event payload emitted when an organization-creation attempt is
/// rejected.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationCreationRejectedEvent {
    /// Why the creation attempt was rejected.
    pub reason: OrganizationFailureReason,
    /// Account that attempted to create the organization.
    pub creator_account_id: AccountId,
    /// Name the caller submitted.
    pub attempted_name: String,
    /// Wall-clock time the rejection was emitted.
    pub at: DateTime<Utc>,
}

/// Bounded request for listing organizations an account belongs to.
///
/// Callers that omit `limit` and `after` receive the default first
/// page (up to [`DEFAULT_ORG_PAGE_SIZE`] items, newest first).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationsRequest {
    /// Maximum number of organizations to return. When `None` the
    /// handler applies [`DEFAULT_ORG_PAGE_SIZE`].
    #[serde(default)]
    pub limit: Option<u32>,
    /// Opaque cursor from a prior [`ListOrganizationsResponse::next_cursor`].
    /// When present the response starts after the page boundary identified
    /// by the cursor.
    #[serde(default)]
    pub after: Option<String>,
}

/// Default page size used when the caller does not specify a limit.
pub const DEFAULT_ORG_PAGE_SIZE: u32 = 100;

/// Marker type to carry [`DEFAULT_ORG_PAGE_SIZE`] as a named associated
/// constant through the re-export in [`crate::ListOrganizationsResponse`].
#[derive(Debug)]
pub struct DefaultOrgPageSize;

impl DefaultOrgPageSize {
    /// The numeric default page size.
    #[must_use]
    pub const fn value() -> u32 {
        DEFAULT_ORG_PAGE_SIZE
    }
}

/// Paginated response for listing organizations an account belongs to.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationsResponse {
    /// Organizations visible to the requesting account for this page.
    pub organizations: Vec<OrganizationView>,
    /// Opaque cursor to pass as [`ListOrganizationsRequest::after`] to
    /// fetch the next page. `None` when no more pages remain.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// MCP tool request for creating an organization.
///
/// Wraps the domain-level [`CreateOrganizationRequest`] fields with an
/// MCP-specific `session_token` parameter. The MCP transport does not
/// use HTTP headers for authentication — the token is passed as a tool
/// parameter instead.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationCreateToolRequest {
    /// Opaque session token obtained from a prior sign-up or sign-in.
    #[serde(default)]
    pub session_token: Option<SessionToken>,
    /// Human-readable name for the new organization.
    pub name: OrganizationName,
}

/// MCP tool request for listing organizations the authenticated account
/// belongs to.
///
/// Mirrors the pagination fields of [`ListOrganizationsRequest`] with an
/// additional `session_token` for MCP-level authentication.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationListToolRequest {
    /// Opaque session token obtained from a prior sign-up or sign-in.
    #[serde(default)]
    pub session_token: Option<SessionToken>,
    /// Maximum number of organizations to return. When `None` the
    /// handler applies [`DEFAULT_ORG_PAGE_SIZE`].
    #[serde(default)]
    pub limit: Option<u32>,
    /// Opaque cursor from a prior response's `next_cursor` field.
    #[serde(default)]
    pub after: Option<String>,
}

/// MCP tool request for checking whether the authenticated account holds
/// a specific admin permission on an organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationAuthorizeToolRequest {
    /// Opaque session token obtained from a prior sign-up or sign-in.
    #[serde(default)]
    pub session_token: Option<SessionToken>,
    /// Organization to check permissions on.
    pub org_id: OrgId,
    /// Administrative operation to authorize.
    pub operation: OrganizationAdminOperation,
}

impl OrganizationFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::DuplicateOrganizationName => "duplicate_organization_name",
            Self::ValidationFailed => "validation_failed",
            Self::NotFound => "not_found",
            Self::LastAdminHolder => "last_admin_holder",
            Self::IdempotencyConflict => "idempotency_conflict",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::AuthRequired => "Authentication is required for this operation.",
            Self::PermissionDenied => "You do not have permission to perform this operation.",
            Self::DuplicateOrganizationName => "An organization with this name already exists.",
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::NotFound => "The requested organization was not found.",
            Self::LastAdminHolder => {
                "This operation would leave the organization without an administrative-permission holder."
            }
            Self::IdempotencyConflict => {
                "The idempotency key was already used with a different account or organization name."
            }
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces. Centralized so every transport reports the
    /// same status for the same failure code.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::AuthRequired => 401,
            Self::PermissionDenied => 403,
            Self::DuplicateOrganizationName | Self::LastAdminHolder | Self::IdempotencyConflict => {
                409
            }
            Self::ValidationFailed => 400,
            Self::NotFound => 404,
        }
    }
}
