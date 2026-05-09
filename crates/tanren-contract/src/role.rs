//! Role-template and permission-evaluation wire shapes.
//!
//! These contracts are shared by api, mcp, cli, tui, and web so role
//! management behaves identically across all first-party interfaces.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    PermissionGrantId, PermissionName, PermissionScope, PrincipalRef, RoleId, RoleName, RoleScope,
    ScopedRole,
};
use utoipa::ToSchema;

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
    /// Role template used as the grant source.
    pub source_role_id: RoleId,
    /// Grant creation timestamp.
    pub granted_at: DateTime<Utc>,
}

/// Canonical role failure envelope exposed by interface transports.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoleFailureBody {
    /// Stable error code from [`RoleFailureReason`].
    pub code: String,
    /// Human-readable failure summary.
    pub summary: String,
}

impl RoleFailureBody {
    /// Project a reason enum into the canonical role failure envelope.
    #[must_use]
    pub fn from_reason(reason: RoleFailureReason) -> Self {
        Self {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
        }
    }
}

/// Closed taxonomy of role-template and permission-evaluation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
}

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
        }
    }
}
