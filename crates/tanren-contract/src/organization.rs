//! Organization command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers create organizations, list
//! accessible organizations, and check organization permissions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, OrgId, OrganizationName, OrganizationPermission, SessionToken,
};
use utoipa::ToSchema;

/// Create-organization request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account creating the organization.
    pub account_id: AccountId,
    /// Candidate organization name. Validated + normalized by
    /// `OrganizationName` during deserialization.
    pub name: OrganizationName,
}

/// Create-organization response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationResponse {
    /// Freshly created organization.
    pub organization: OrganizationView,
    /// Administrative permissions granted to the creator.
    pub granted_permissions: Vec<OrganizationPermission>,
}

/// List-organizations request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationsRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account whose organization memberships are requested.
    pub account_id: AccountId,
}

/// List-organizations response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationsResponse {
    /// Organizations visible to the requested account.
    pub organizations: Vec<OrganizationView>,
}

/// Check-organization-permission request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CheckOrganizationPermissionRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account being checked.
    pub account_id: AccountId,
    /// Organization in which permission is checked.
    pub org_id: OrgId,
    /// Permission being checked.
    pub permission: OrganizationPermission,
}

/// Check-organization-permission response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CheckOrganizationPermissionResponse {
    /// Account that was checked.
    pub account_id: AccountId,
    /// Organization in which permission was checked.
    pub org_id: OrgId,
    /// Permission that was checked.
    pub permission: OrganizationPermission,
    /// Whether the requested permission is currently granted.
    pub allowed: bool,
}

/// External-facing view of a Tanren organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationView {
    /// Stable organization id.
    pub id: OrgId,
    /// Organization name uniqueness key.
    pub name: OrganizationName,
}
