//! Organization command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers create organizations, list
//! accessible organizations, and check organization permissions.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, IdempotencyKey, MembershipId, OrgId, OrganizationName, OrganizationPermission,
    SessionToken,
};
use utoipa::{IntoParams, ToSchema};

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
    /// Stable client idempotency key. Replays with the same actor and
    /// key return the same semantic result. Must be non-empty after
    /// trimming, max 128 chars, and free of control characters.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Create-organization response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationResponse {
    /// Freshly created organization.
    pub organization: OrganizationView,
    /// Administrative permissions granted to the creator.
    pub granted_permissions: Vec<OrganizationPermission>,
    /// New organizations always begin with zero projects.
    pub initial_project_count: u64,
    /// Stable proof reference clients can render without event-log probing.
    pub proof_link: OrganizationProofLink,
    /// Stable source reference for the canonical creation event.
    pub source_link: OrganizationSourceLink,
}

/// List-organizations request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationsRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account whose organization memberships are requested.
    pub account_id: AccountId,
    /// Maximum page size the caller asks for.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<MembershipId>,
}

/// List-organizations response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationsResponse {
    /// Organizations visible to the requested account.
    pub organizations: Vec<OrganizationView>,
    /// Opaque cursor callers can pass to fetch the next page.
    pub next_cursor: Option<MembershipId>,
}

/// Query parameters for `GET /organizations`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListOrganizationsApiQuery {
    /// Maximum page size the caller asks for.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<MembershipId>,
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

/// API body for create-organization routes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationApiRequest {
    /// Candidate organization name. Validated + normalized by
    /// `OrganizationName` during deserialization.
    pub name: OrganizationName,
    /// Stable client idempotency key. Replays with the same actor and
    /// key return the same semantic result. Must be non-empty after
    /// trimming, max 128 chars, and free of control characters.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// API body for check-organization-permission routes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CheckOrganizationPermissionApiRequest {
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

/// Shared failure taxonomy for create-organization command handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CreateOrganizationFailureReason {
    /// Name uniqueness conflict.
    DuplicateName,
    /// Idempotency key reused with conflicting request fingerprint.
    IdempotencyConflict,
}

impl CreateOrganizationFailureReason {
    /// Organization failure-code taxonomy variant for this reason.
    #[must_use]
    pub const fn failure_code(self) -> OrganizationFailureCode {
        match self {
            Self::DuplicateName => OrganizationFailureCode::Conflict,
            Self::IdempotencyConflict => OrganizationFailureCode::IdempotencyConflict,
        }
    }

    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.failure_code().code()
    }

    /// Human-readable summary for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::DuplicateName => "An organization already exists for the supplied name.",
            Self::IdempotencyConflict => {
                "The supplied idempotency key conflicts with a prior request."
            }
        }
    }

    /// Recommended HTTP status when projected over API/MCP.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        409
    }
}

/// Closed error-code taxonomy for organization operations across all
/// interfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationFailureCode {
    /// Name uniqueness conflict.
    Conflict,
    /// Idempotency key reused with conflicting request fingerprint.
    IdempotencyConflict,
    /// Request input failed validation.
    ValidationFailed,
    /// Authentication is required.
    AuthRequired,
    /// Authenticated actor lacks permission for the operation.
    PermissionDenied,
    /// Internal service/store failure.
    InternalError,
}

impl OrganizationFailureCode {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::ValidationFailed => "validation_failed",
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::InternalError => "internal_error",
        }
    }

    /// Recommended HTTP status for this failure.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::Conflict | Self::IdempotencyConflict => 409,
            Self::ValidationFailed => 400,
            Self::AuthRequired => 401,
            Self::PermissionDenied => 403,
            Self::InternalError => 500,
        }
    }
}

/// Shared `{code, summary}` body for organization-operation failures.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationFailureBody {
    /// Stable error code from the organization taxonomy.
    pub code: OrganizationFailureCode,
    /// Human-readable summary.
    pub summary: String,
}

/// Stable reference to behavior proof coverage for organization create.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationProofLink {
    /// Canonical behavior id proving this command contract.
    pub behavior_id: String,
}

/// Stable reference to source evidence for organization create.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSourceLink {
    /// Event family in the canonical event log.
    pub event_family: String,
    /// Event kind in the canonical event log.
    pub event_kind: String,
}

/// Event family used for organization lifecycle events.
pub const ORGANIZATION_EVENT_FAMILY: &str = "organization";
/// Event kind for organization-creation events.
pub const ORGANIZATION_CREATED_EVENT_KIND: &str = "organization_created";
/// Canonical behavior proof id for organization creation.
pub const ORGANIZATION_CREATE_BEHAVIOR_ID: &str = "B-0066";
/// Default page size for listing organizations.
pub const LIST_ORGANIZATIONS_DEFAULT_LIMIT: u64 = 50;
/// Maximum allowed page size for listing organizations.
pub const LIST_ORGANIZATIONS_MAX_LIMIT: u64 = 100;

/// Shared payload contract for `organization_created`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationCreatedEvent {
    /// Freshly created organization id.
    pub org_id: OrgId,
    /// Normalized organization name uniqueness key.
    pub name: OrganizationName,
    /// Account that created the organization.
    pub creator_account_id: AccountId,
    /// Creator permissions granted at bootstrap.
    pub granted_permissions: Vec<OrganizationPermission>,
    /// Initial project count for the new organization.
    pub initial_project_count: u64,
    /// Service-side timestamp for the create event.
    pub created_at: DateTime<Utc>,
}
