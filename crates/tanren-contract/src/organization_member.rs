//! Organization-member list wire shapes.
//!
//! Request/response surface for the api, mcp, cli, tui, and web client
//! when callers list members of an organization. Behavior: B-0065
//! "See existing members' access to an organization".

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, MembershipId, OrgId, OrganizationPermission};
use utoipa::{IntoParams, ToSchema};

use crate::{ListLimit, ReadModelFreshness};

/// Canonical behavior proof id for organization-member listing.
pub const MEMBER_LIST_BEHAVIOR_ID: &str = "B-0065";

/// Default page size for listing organization members.
pub const LIST_MEMBERS_DEFAULT_LIMIT: u64 = 50;

/// Maximum allowed page size for listing organization members.
pub const LIST_MEMBERS_MAX_LIMIT: u64 = 100;

/// API path parameters for `GET /organizations/{org_id}/members`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListOrganizationMembersApiPath {
    /// Organization whose members are listed.
    pub org_id: OrgId,
}

/// Query parameters for `GET /organizations/{org_id}/members`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListOrganizationMembersApiQuery {
    /// Maximum page size the caller asks for. Clamped to
    /// `[1, LIST_MEMBERS_MAX_LIMIT]` server-side; omitted defaults to
    /// `LIST_MEMBERS_DEFAULT_LIMIT`.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<MembershipId>,
}

/// List-organization-members request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationMembersRequest {
    /// Session proving the caller is authenticated.
    pub session_token: tanren_identity_policy::SessionToken,
    /// Account requesting the member list. Must be a member of `org_id`.
    pub account_id: AccountId,
    /// Organization whose members are listed.
    pub org_id: OrgId,
    /// Bounded page-size limit.
    pub limit: ListLimit,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<MembershipId>,
}

impl ListOrganizationMembersRequest {
    /// Build a list-organization-members request from authenticated transport
    /// context, path parameters, and query parameters.
    #[must_use]
    pub fn from_api(
        session_token: tanren_identity_policy::SessionToken,
        account_id: AccountId,
        path: &ListOrganizationMembersApiPath,
        query: &ListOrganizationMembersApiQuery,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id: path.org_id,
            limit: ListLimit::new(
                query.limit,
                LIST_MEMBERS_DEFAULT_LIMIT,
                LIST_MEMBERS_MAX_LIMIT,
            ),
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
    /// Read-model freshness metadata for this response.
    pub freshness: ReadModelFreshness,
}

/// External-facing view of a single organization member.
///
/// Visible to every member of the organization (no hiding). Each entry
/// carries the member's account identity, membership id, organization-level
/// permissions, and how each permission grant was sourced.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationMemberView {
    /// Stable membership id linking this account to this organization.
    pub membership_id: MembershipId,
    /// Account id of the organization member.
    pub account_id: AccountId,
    /// Human-readable display name of the member.
    pub display_name: String,
    /// Organization-level permissions held by this member.
    pub permissions: Vec<OrganizationPermission>,
    /// Grant source for each permission the member holds.
    pub grant_sources: Vec<OrganizationMemberGrantSource>,
    /// When this membership was created.
    pub joined_at: DateTime<Utc>,
}

/// Closed taxonomy of how a member received an organization-level
/// permission grant.
///
/// Distinguishes direct grants from role-template grants so that the
/// access model is transparent (B-0065). The grant source is informational;
/// authorization checks evaluate the effective permission set, not the
/// source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationMemberGrantSource {
    /// Permission was granted directly to the member by an administrator.
    DirectGrant,
    /// Permission was derived from a role template applied to the member.
    RoleTemplate {
        /// Opaque identifier of the role template that conferred this permission.
        role_template_id: String,
    },
}

impl OrganizationMemberGrantSource {
    /// Stable wire `kind` for this grant source.
    #[must_use]
    pub fn kind(self) -> &'static str {
        match self {
            Self::DirectGrant => "direct_grant",
            Self::RoleTemplate { .. } => "role_template",
        }
    }
}
