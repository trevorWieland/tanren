//! Organization command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers create or list Tanren
//! organizations. They live in `tanren-contract` because every interface
//! binary serialises the same shapes — keeping them here is the
//! architectural guarantee that the surfaces stay equivalent.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, OrgAdminPermissions, OrgId, OrgName};
use utoipa::ToSchema;

/// Organization-creation request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationRequest {
    /// Human-readable name for the new organization. Globally unique,
    /// case-insensitive (enforced by the domain newtype's parse path).
    pub name: OrgName,
}

/// Successful organization-creation response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationResponse {
    /// View of the freshly created organization.
    pub organization: OrganizationView,
    /// Bootstrap administrative permissions granted to the creator.
    pub membership_permissions: OrgAdminPermissions,
}

/// External-facing view of a Tanren organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationView {
    /// Stable organization id.
    pub id: OrgId,
    /// Canonical display name.
    pub name: OrgName,
    /// Wall-clock time the organization was created.
    pub created_at: DateTime<Utc>,
}

/// Successful list-organizations response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationsResponse {
    /// Organizations visible to the caller.
    pub organizations: Vec<OrganizationView>,
}

/// External-facing view of an organization membership.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MembershipView {
    /// Organization the membership belongs to.
    pub org_id: OrgId,
    /// Account that holds this membership.
    pub account_id: AccountId,
    /// Administrative permissions granted by this membership.
    pub permissions: OrgAdminPermissions,
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
    /// The chosen organization name is already taken.
    DuplicateName,
    /// The caller is not authenticated.
    Unauthenticated,
    /// The caller is not authorized to perform this action.
    NotAuthorized,
}

impl OrganizationFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DuplicateName => "duplicate_name",
            Self::Unauthenticated => "unauthenticated",
            Self::NotAuthorized => "not_authorized",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::DuplicateName => "An organization with this name already exists.",
            Self::Unauthenticated => "Authentication is required to perform this action.",
            Self::NotAuthorized => "You are not authorized to perform this action.",
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::DuplicateName => 409,
            Self::Unauthenticated => 401,
            Self::NotAuthorized => 403,
        }
    }
}
