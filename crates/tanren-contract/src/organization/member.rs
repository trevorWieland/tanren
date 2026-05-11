//! Organization-member listing wire shapes.
//!
//! Request/response surface for listing members within an organization.
//! Mirrors the list-organizations contract so `OpenAPI` generation and
//! capability-projection helpers reuse cleanly.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use tanren_identity_policy::{
    AccountId, MembershipId, OrgId, OrganizationPermission, SessionToken,
};
use utoipa::{IntoParams, ToSchema};

use super::freshness::ReadModelFreshness;
use super::{OrganizationProofLink, OrganizationSourceLink};

/// List-organization-members request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationMembersRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account whose organization memberships authorize the query.
    pub account_id: AccountId,
    /// Organization whose members are requested.
    pub org_id: OrgId,
    /// Maximum page size the caller asks for.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<MembershipId>,
}

impl ListOrganizationMembersRequest {
    /// Build a list-organization-members request from authenticated transport
    /// context, path parameters, and query parameters.
    #[must_use]
    pub fn from_api_query(
        session_token: SessionToken,
        account_id: AccountId,
        path: &ListOrganizationMembersApiPath,
        query: &ListOrganizationMembersApiQuery,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id: path.org_id,
            limit: query.limit,
            cursor: query.cursor,
        }
    }
}

/// List-organization-members response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationMembersResponse {
    /// Members visible within the requested organization.
    pub members: Vec<OrganizationMemberView>,
    /// Opaque cursor callers can pass to fetch the next page.
    pub next_cursor: Option<MembershipId>,
    /// Canonical source link for organization-member lifecycle events.
    pub source_link: OrganizationSourceLink,
    /// Read-model freshness metadata for this response.
    pub freshness: ReadModelFreshness,
    /// Stable proof reference clients can render without event-log probing.
    pub proof_link: OrganizationProofLink,
}

/// Contract projection of a single organization member.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationMemberView {
    /// Account id of the organization member.
    pub account_id: AccountId,
    /// Identifier (derived from email) for display purposes.
    pub identifier: String,
    /// Wall-clock time the membership was created.
    pub joined_at: DateTime<Utc>,
    /// Permission grants active for this member in the organization.
    pub granted_permissions: Vec<OrganizationMemberPermissionGrant>,
}

/// A single permission grant for an organization member.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationMemberPermissionGrant {
    /// The organization-level permission that was granted.
    pub permission: OrganizationPermission,
    /// How this grant was sourced — directly assigned or via a role template.
    pub grant_source: GrantSource,
    /// Account that created this grant.
    pub granted_by_account_id: AccountId,
}

/// Origin of a permission grant.
///
/// `Direct` is the default emitted by the store today. `RoleTemplate` is
/// reserved for the role-template flow (not yet wired).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GrantSource {
    /// Permission was assigned directly to the member.
    Direct,
    /// Permission was inherited from a role template.
    RoleTemplate,
}

impl GrantSource {
    /// Canonical wire key for this grant source.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::RoleTemplate => "role_template",
        }
    }
}

impl fmt::Display for GrantSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for GrantSource {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "direct" => Ok(Self::Direct),
            "role_template" => Ok(Self::RoleTemplate),
            _ => Err("unknown grant source"),
        }
    }
}

/// Query parameters for `GET /organizations/{org_id}/members`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListOrganizationMembersApiQuery {
    /// Maximum page size the caller asks for.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<MembershipId>,
}

/// Path parameters for `GET /organizations/{org_id}/members`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListOrganizationMembersApiPath {
    /// Organization whose members are requested.
    pub org_id: OrgId,
}

/// Canonical behavior proof id for organization member listing.
pub const ORGANIZATION_MEMBER_LIST_BEHAVIOR_ID: &str = "B-0065";

/// Default page size for listing organization members.
pub const LIST_ORGANIZATION_MEMBERS_DEFAULT_LIMIT: u64 = 50;
/// Maximum allowed page size for listing organization members.
pub const LIST_ORGANIZATION_MEMBERS_MAX_LIMIT: u64 = 100;
