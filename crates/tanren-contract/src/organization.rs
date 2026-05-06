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
use tanren_identity_policy::{AccountId, MembershipId, OrgId, OrgPermission, OrganizationName};
use utoipa::ToSchema;

/// Create-organization request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationRequest {
    /// Human-readable name for the new organization. Trimmed and
    /// case-folded during validation.
    pub name: OrganizationName,
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
/// Mirrors [`OrgPermission`](tanren_identity_policy::OrgPermission)
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

/// Closed taxonomy of organization-flow failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects an `OrganizationFailureReason`
/// into the same wire shape so callers can match on `code` regardless of
/// transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
            Self::DuplicateOrganizationName | Self::LastAdminHolder => 409,
            Self::ValidationFailed => 400,
            Self::NotFound => 404,
        }
    }
}
