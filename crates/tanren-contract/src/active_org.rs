//! Active-organization command/response wire shapes.
//!
//! Request/response surface for switching the active organization within
//! an account. Personal accounts with no memberships see no
//! organization-scoped actions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, OrgId, SessionToken};
use utoipa::ToSchema;

use crate::organization::{OrganizationCapabilityView, OrganizationView};

/// Request to switch the active organization for the caller's session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchActiveOrgRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account requesting the switch.
    pub account_id: AccountId,
    /// Organization to activate. The caller must be a member.
    pub org_id: OrgId,
}

/// Response after switching the active organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchActiveOrgResponse {
    /// Currently active organization (if any).
    pub active_org: Option<OrganizationView>,
    /// Capabilities available in the active organization.
    pub capabilities: Vec<OrganizationCapabilityView>,
}

/// Active-org context and switchable organizations for the session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListActiveOrgContextResponse {
    /// Currently active organization. `None` when no org is active.
    pub active_org: Option<OrganizationView>,
    /// Organizations the account can switch to.
    pub available_organizations: Vec<OrganizationView>,
}
