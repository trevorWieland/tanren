//! Active-organization command/response wire shapes.
//!
//! Request/response surface for switching the active organization context
//! within an authenticated session and for listing projects scoped to the
//! active organization. Supports B-0047 ("Switch the active organization
//! within an account").

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, IdempotencyKey, OrgId, ProjectId, ProjectName, SessionToken,
};
use utoipa::{IntoParams, ToSchema};

use crate::organization::{OrganizationCapabilityView, ReadModelFreshness};

/// Switch-active-organization request.
///
/// Sets the active organization for the caller's session. The caller must
/// be an authenticated member of the target organization. A personal
/// account (no memberships) may not switch to any organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchActiveOrganizationRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account whose active organization is being switched.
    pub account_id: AccountId,
    /// Target organization to activate. The account must be a member.
    pub org_id: OrgId,
    /// Stable client idempotency key. Replays with the same actor and key
    /// return the same semantic result.
    pub idempotency_key: Option<IdempotencyKey>,
}

impl SwitchActiveOrganizationRequest {
    /// Build a switch-active-organization request from authenticated transport
    /// context plus the validated API body.
    #[must_use]
    pub fn from_api(
        session_token: SessionToken,
        account_id: AccountId,
        body: SwitchActiveOrganizationApiRequest,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id: body.org_id,
            idempotency_key: body.idempotency_key,
        }
    }
}

/// API body for switch-active-organization routes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchActiveOrganizationApiRequest {
    /// Target organization to activate.
    pub org_id: OrgId,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Switch-active-organization response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchActiveOrganizationResponse {
    /// The now-active organization.
    pub active_org: OrgId,
    /// Capabilities available in the active organization for the caller.
    pub capabilities: Vec<OrganizationCapabilityView>,
    /// Projects visible in the active organization for the caller.
    pub projects: Vec<ProjectListItem>,
    /// Read-model freshness metadata for this response.
    pub freshness: ReadModelFreshness,
}

/// Read-active-organization request.
///
/// Returns the caller's current active organization context, including
/// the active org (if any) and projects scoped to that org.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReadActiveOrganizationRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account whose active organization is being read.
    pub account_id: AccountId,
}

/// Read-active-organization response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReadActiveOrganizationResponse {
    /// Currently active organization, or `None` for a personal account
    /// with no active organization context.
    pub active_org: Option<ActiveOrganizationContext>,
    /// Organizations the account belongs to and can switch to.
    pub available_organizations: Vec<OrganizationSwitchOption>,
    /// Read-model freshness metadata for this response.
    pub freshness: ReadModelFreshness,
}

/// Context for the currently active organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ActiveOrganizationContext {
    /// Active organization id.
    pub org_id: OrgId,
    /// Capabilities available in this organization for the caller.
    pub capabilities: Vec<OrganizationCapabilityView>,
    /// Projects visible in this organization for the caller.
    pub projects: Vec<ProjectListItem>,
}

/// An organization the account can switch to.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSwitchOption {
    /// Organization id.
    pub org_id: OrgId,
    /// Whether this organization is currently active.
    pub is_active: bool,
}

/// Summary of a project within an organization's project list.
///
/// Minimal fixture-visible projection for project summaries. Full
/// project lifecycle management is out of scope for this contract.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectListItem {
    /// Stable project id.
    pub project_id: ProjectId,
    /// Project display name.
    pub name: ProjectName,
    /// Wall-clock time the project was registered.
    pub created_at: DateTime<Utc>,
}

/// List-organization-projects request.
///
/// Returns projects owned by the specified organization that are visible
/// to the caller.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationProjectsRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account requesting the project list.
    pub account_id: AccountId,
    /// Organization whose projects are being listed.
    pub org_id: OrgId,
    /// Maximum page size the caller asks for.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<ProjectId>,
}

impl ListOrganizationProjectsRequest {
    /// Build a list-organization-projects request from authenticated
    /// transport context and query parameters.
    #[must_use]
    pub fn from_api_query(
        session_token: SessionToken,
        account_id: AccountId,
        org_id: OrgId,
        query: &ListOrganizationProjectsApiQuery,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id,
            limit: query.limit,
            cursor: query.cursor,
        }
    }
}

/// List-organization-projects response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationProjectsResponse {
    /// Projects visible to the caller in the organization.
    pub projects: Vec<ProjectListItem>,
    /// Opaque cursor callers can pass to fetch the next page.
    pub next_cursor: Option<ProjectId>,
    /// Read-model freshness metadata for this response.
    pub freshness: ReadModelFreshness,
}

/// Query parameters for `GET /organizations/{org_id}/projects`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListOrganizationProjectsApiQuery {
    /// Maximum page size the caller asks for.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<ProjectId>,
}

/// Closed taxonomy of active-organization operation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActiveOrganizationFailureReason {
    /// Authentication is required.
    AuthRequired,
    /// Authenticated actor lacks permission (e.g. not a member).
    PermissionDenied,
    /// The target organization does not exist.
    OrganizationNotFound,
    /// The account does not belong to the target organization.
    NotAMember,
    /// Request input failed validation.
    ValidationFailed,
}

impl ActiveOrganizationFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::OrganizationNotFound => "organization_not_found",
            Self::NotAMember => "not_a_member",
            Self::ValidationFailed => "validation_failed",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::AuthRequired => {
                "The request requires authentication and the supplied session is missing or expired."
            }
            Self::PermissionDenied => {
                "The authenticated actor lacks permission to perform this action."
            }
            Self::OrganizationNotFound => "The target organization does not exist.",
            Self::NotAMember => "The account does not belong to the target organization.",
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
        }
    }

    /// Recommended HTTP status for this failure.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::AuthRequired => 401,
            Self::PermissionDenied | Self::NotAMember => 403,
            Self::OrganizationNotFound => 404,
            Self::ValidationFailed => 400,
        }
    }
}

/// Canonical behavior proof id for active-organization switching.
pub const ACTIVE_ORGANIZATION_SWITCH_BEHAVIOR_ID: &str = "B-0047";

/// Default page size for listing organization projects.
pub const LIST_ORGANIZATION_PROJECTS_DEFAULT_LIMIT: u64 = 50;
/// Maximum allowed page size for listing organization projects.
pub const LIST_ORGANIZATION_PROJECTS_MAX_LIMIT: u64 = 100;
