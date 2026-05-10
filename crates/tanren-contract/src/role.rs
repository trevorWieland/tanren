//! Role-template and permission-evaluation wire shapes.
//!
//! These contracts are shared by api, mcp, cli, tui, and web so role
//! management behaves identically across all first-party interfaces.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, PermissionGrantId, PermissionGrantRevocation, PermissionGrantSource, PermissionName,
    PermissionScope, PrincipalRef, ROLE_TEMPLATE_EMPTY_BUNDLE_ALLOWED,
    ROLE_TEMPLATE_MAX_PERMISSIONS, RoleId, RoleName, RoleScope, ScopedRole,
};
use utoipa::ToSchema;

/// Documented maximum size of a role-template permission bundle.
pub const MAX_ROLE_TEMPLATE_PERMISSIONS: usize = ROLE_TEMPLATE_MAX_PERMISSIONS;
/// Explicit empty-bundle policy for role templates.
pub const ROLE_TEMPLATE_ALLOW_EMPTY_BUNDLE: bool = ROLE_TEMPLATE_EMPTY_BUNDLE_ALLOWED;
/// Maximum page size accepted by role read-model requests.
pub const ROLE_READ_MODEL_PAGE_MAX: u64 = 200;
/// Default page size used when role read-model requests omit a limit.
pub const ROLE_READ_MODEL_PAGE_DEFAULT: u64 = 50;

/// Create-role request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateRoleRequest {
    /// Scope where the role template is defined.
    pub scope: RoleScope,
    /// Human-readable role template name.
    pub name: RoleName,
    /// Permission names bundled by this role template.
    pub permissions: Vec<PermissionName>,
}

/// Successful create-role response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateRoleResponse {
    /// Newly created role template.
    pub role: RoleTemplateView,
}

/// Edit-role request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct EditRoleRequest {
    /// Role template being edited.
    pub role: ScopedRole,
    /// Replacement role template name.
    pub name: RoleName,
    /// Replacement role permission bundle.
    pub permissions: Vec<PermissionName>,
}

/// Successful edit-role response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct EditRoleResponse {
    /// Updated role template snapshot.
    pub role: RoleTemplateView,
}

/// Delete-role request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeleteRoleRequest {
    /// Role template to delete.
    pub role: ScopedRole,
}

/// Successful delete-role response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeleteRoleResponse {
    /// Deleted role template identifier.
    pub role: ScopedRole,
}

/// Apply-role request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ApplyRoleRequest {
    /// Role template being applied.
    pub role: ScopedRole,
    /// Principal that receives grants.
    pub principal: PrincipalRef,
    /// Scope where grants are created.
    pub grant_scope: PermissionScope,
}

/// Successful apply-role response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ApplyRoleResponse {
    /// Role template that was applied.
    pub role: ScopedRole,
    /// Individual permission grants created by this operation.
    pub grants: Vec<PermissionGrantView>,
}

/// Cursor for role-template listing (`name`, then `id`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleTemplateCursorView {
    /// Last-seen role name.
    pub name: RoleName,
    /// Last-seen role id (tie-breaker for duplicate names).
    pub id: RoleId,
}

/// Cursor for direct-grant listing (`granted_at`, then `id`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PermissionGrantCursorView {
    /// Last-seen grant timestamp.
    pub granted_at: DateTime<Utc>,
    /// Last-seen grant id (tie-breaker for equal timestamps).
    pub id: PermissionGrantId,
}

/// Read-model query for role administration state.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleReadModelRequest {
    /// Scope where role templates are listed.
    pub role_scope: RoleScope,
    /// Cursor for continuing role-template pagination.
    pub role_cursor: Option<RoleTemplateCursorView>,
    /// Requested role-template page size.
    pub role_limit: Option<u64>,
    /// Principal whose direct grants are listed.
    pub grant_principal: PrincipalRef,
    /// Scope where direct grants are listed.
    pub grant_scope: PermissionScope,
    /// Cursor for continuing direct-grant pagination.
    pub grant_cursor: Option<PermissionGrantCursorView>,
    /// Requested direct-grant page size.
    pub grant_limit: Option<u64>,
}

/// Freshness metadata for role read-model responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleReadModelFreshness {
    /// Timestamp when the response snapshot was observed.
    pub observed_at: DateTime<Utc>,
}

/// Role administration read-model response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleReadModelResponse {
    /// Scope used for the role-template listing.
    pub role_scope: RoleScope,
    /// Principal used for the direct-grant listing.
    pub grant_principal: PrincipalRef,
    /// Scope used for the direct-grant listing.
    pub grant_scope: PermissionScope,
    /// Role templates returned for the current page.
    pub role_templates: Vec<RoleTemplateView>,
    /// Cursor for the next role-template page.
    pub role_next_cursor: Option<RoleTemplateCursorView>,
    /// Direct grants returned for the current page.
    pub direct_grants: Vec<PermissionGrantView>,
    /// Cursor for the next direct-grant page.
    pub grant_next_cursor: Option<PermissionGrantCursorView>,
    /// Freshness metadata for this snapshot.
    pub freshness: RoleReadModelFreshness,
}

/// Permission-check request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PermissionCheckRequest {
    /// Principal being evaluated.
    pub principal: PrincipalRef,
    /// Permission being checked.
    pub permission: PermissionName,
    /// Scope where the permission is evaluated.
    pub scope: PermissionScope,
}

/// Permission-check response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PermissionCheckResponse {
    /// Principal that was evaluated.
    pub principal: PrincipalRef,
    /// Permission that was evaluated.
    pub permission: PermissionName,
    /// Scope where the permission was evaluated.
    pub scope: PermissionScope,
    /// Whether the permission is granted.
    pub allowed: bool,
    /// Matching grant ids. Empty when `allowed` is `false`.
    pub matching_grant_ids: Vec<PermissionGrantId>,
}

/// Authenticated actor context for role administration requests.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleActor {
    /// Account identity resolved by the interface authentication layer.
    pub account_id: AccountId,
}

/// Individual role-administration action surfaced by capability discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoleAdminAction {
    /// Create role templates.
    CreateRole,
    /// Edit role templates.
    EditRole,
    /// Delete role templates.
    DeleteRole,
    /// Apply role templates to principals.
    ApplyRole,
    /// List and read role templates.
    ReadRoles,
    /// Run direct permission checks.
    CheckPermission,
}

/// Capability metadata for role administration controls.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleAdminCapabilities {
    /// Authenticated actor this capability snapshot applies to.
    pub actor: RoleActor,
    /// Role administration actions this actor is currently allowed to execute.
    pub actions: Vec<RoleAdminAction>,
}

/// External-facing view of a role template.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleTemplateView {
    /// Stable role template id.
    pub id: RoleId,
    /// Scope where the role template is defined.
    pub scope: RoleScope,
    /// Human-readable role template name.
    pub name: RoleName,
    /// Permission bundle this role applies.
    pub permissions: Vec<PermissionName>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// External-facing view of a direct permission grant.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PermissionGrantView {
    /// Stable permission-grant id.
    pub id: PermissionGrantId,
    /// Principal receiving the grant.
    pub principal: PrincipalRef,
    /// Scope where the permission grant applies.
    pub scope: PermissionScope,
    /// Granted permission.
    pub permission: PermissionName,
    /// Provenance for how this direct grant was created.
    pub source: PermissionGrantSource,
    /// Revocation metadata when this grant is no longer effective.
    pub revocation: Option<PermissionGrantRevocation>,
    /// Grant creation timestamp.
    pub granted_at: DateTime<Utc>,
}

/// Canonical role failure envelope exposed by interface transports.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleFailureBody {
    /// Stable error code from [`RoleFailureReason`].
    pub code: RoleFailureReason,
    /// Human-readable failure summary.
    pub summary: String,
}

impl RoleFailureBody {
    /// Project a reason enum into the canonical role failure envelope.
    #[must_use]
    pub fn from_reason(reason: RoleFailureReason) -> Self {
        Self {
            code: reason,
            summary: reason.summary().to_owned(),
        }
    }
}

/// Closed taxonomy of role-template and permission-evaluation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RoleFailureReason {
    /// Submitted input failed contract-level validation.
    ValidationFailed,
    /// Requested role or grant target was not found.
    NotFound,
    /// Requested mutation conflicts with current state.
    Conflict,
    /// Caller lacks the required permission.
    PermissionDenied,
    /// Role identifiers cannot be used as authorization principals.
    RoleAsPrincipalRejected,
    /// Internal transport or server failure outside role-domain state transitions.
    InternalError,
}

/// Shared interface error taxonomy used by account + role surfaces.
///
/// Role flows extend this base set with
/// [`RoleFailureReason::RoleAsPrincipalRejected`].
pub const SHARED_INTERFACE_ROLE_FAILURE_REASONS: [RoleFailureReason; 5] = [
    RoleFailureReason::ValidationFailed,
    RoleFailureReason::NotFound,
    RoleFailureReason::Conflict,
    RoleFailureReason::PermissionDenied,
    RoleFailureReason::InternalError,
];

/// Role-specific failure code extending the shared interface taxonomy.
pub const ROLE_FAILURE_EXTENSION_REASON: RoleFailureReason =
    RoleFailureReason::RoleAsPrincipalRejected;

/// Complete role server-side failure taxonomy exposed on the wire.
pub const ROLE_SERVER_FAILURE_REASONS: [RoleFailureReason; 6] = [
    RoleFailureReason::ValidationFailed,
    RoleFailureReason::NotFound,
    RoleFailureReason::Conflict,
    RoleFailureReason::PermissionDenied,
    RoleFailureReason::RoleAsPrincipalRejected,
    RoleFailureReason::InternalError,
];

impl RoleFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ValidationFailed => "validation_failed",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::PermissionDenied => "permission_denied",
            Self::RoleAsPrincipalRejected => "role_as_principal_rejected",
            Self::InternalError => "internal_error",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::NotFound => "The requested role, principal, or scope does not exist.",
            Self::Conflict => "The request conflicts with current role or grant state.",
            Self::PermissionDenied => {
                "The caller is authenticated but not authorized for this operation."
            }
            Self::RoleAsPrincipalRejected => {
                "Roles are permission templates and cannot be used as authorization principals."
            }
            Self::InternalError => "Tanren encountered an internal error.",
        }
    }

    /// Recommended HTTP status for this failure reason.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::ValidationFailed | Self::RoleAsPrincipalRejected => 400,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::PermissionDenied => 403,
            Self::InternalError => 500,
        }
    }
}
