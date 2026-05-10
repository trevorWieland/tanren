//! Self-permission query wire shapes.
//!
//! These contract types model the read-only "my permissions" surface used
//! by api/mcp/cli/tui/web. The request is intentionally self-scoped: callers
//! cannot pass an arbitrary account id.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    OrgId, PermissionEffectiveState, PermissionGrantSource, PermissionName, PolicyConstraintReason,
    PolicyConstraintSource, ProjectId,
};
use utoipa::ToSchema;

/// Default number of permission entries returned by the self-permissions view
/// when callers do not provide a limit.
pub const MY_PERMISSIONS_DEFAULT_LIMIT: u16 = 100;
/// Maximum allowed `limit` for self-permissions reads across all interfaces.
pub const MY_PERMISSIONS_MAX_LIMIT: u16 = 200;

/// Request payload for self-permission introspection.
///
/// Identity comes from the current authenticated session; callers may only
/// provide pagination hints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsRequest {
    /// Maximum number of permission entries to return.
    ///
    /// Values above [`MY_PERMISSIONS_MAX_LIMIT`] are clamped to that maximum.
    /// Missing or zero values fall back to [`MY_PERMISSIONS_DEFAULT_LIMIT`].
    pub limit: Option<u16>,
    /// Opaque continuation token for cursor-based pagination.
    ///
    /// Omit this field for the first page. Empty strings are treated as
    /// no cursor.
    pub cursor: Option<String>,
}

impl Default for MyPermissionsRequest {
    fn default() -> Self {
        Self {
            limit: Some(MY_PERMISSIONS_DEFAULT_LIMIT),
            cursor: None,
        }
    }
}

impl MyPermissionsRequest {
    /// Resolve the caller-supplied limit to a bounded value.
    #[must_use]
    pub fn resolved_limit(&self) -> u16 {
        match self.limit {
            Some(0) | None => MY_PERMISSIONS_DEFAULT_LIMIT,
            Some(limit) => limit.min(MY_PERMISSIONS_MAX_LIMIT),
        }
    }

    /// Resolve the caller-provided cursor hint.
    #[must_use]
    pub fn resolved_cursor(&self) -> Option<String> {
        self.cursor
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    }
}

/// Response payload for self-permission introspection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsResponse {
    /// Pagination metadata for this response page.
    pub page: MyPermissionsPageMeta,
    /// Read metadata for the returned snapshot.
    pub read_metadata: MyPermissionsReadMeta,
    /// Organization-scoped permission sections visible to the caller.
    pub organizations: Vec<MyOrganizationPermissions>,
    /// Project-scoped permission sections visible to the caller.
    pub projects: Vec<MyProjectPermissions>,
}

/// Pagination metadata for a self-permissions response page.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsPageMeta {
    /// Maximum number of permission entries requested for this page.
    pub limit: u16,
    /// Number of permission entries included in this page.
    pub returned: u16,
    /// Cursor that produced this page; omitted for first-page reads.
    pub request_cursor: Option<String>,
    /// Cursor to fetch the next page; null when no continuation exists.
    pub next_cursor: Option<String>,
}

/// Read metadata returned with self-permission pages.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsReadMeta {
    /// Canonical read source name serving this response.
    pub source: String,
    /// Wall-clock instant when this response snapshot was generated.
    pub generated_at: DateTime<Utc>,
    /// Source checkpoint sampled after the permission rows were read.
    ///
    /// Checkpoint identifiers are the highest row ids seen in the
    /// account-scoped permission source tables at metadata capture time.
    pub source_checkpoint: MyPermissionsSourceCheckpoint,
    /// Freshness status relative to source-checkpoint drift during this read.
    ///
    /// `fresh` means the checkpoint before and after row enumeration matched.
    /// `potentially_stale` means the checkpoint advanced while rows were
    /// enumerated, so this page may lag newly appended grants/constraints.
    pub staleness: MyPermissionsStaleness,
}

/// Account-scoped checkpoint metadata for self-permission reads.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsSourceCheckpoint {
    /// Highest permission-grant row id visible for this account when read.
    pub max_permission_grant_id: Option<String>,
    /// Highest permission-constraint row id visible for this account when read.
    pub max_permission_constraint_id: Option<String>,
}

/// Freshness status for the self-permission snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MyPermissionsStaleness {
    /// Snapshot is current with the source checkpoint captured in metadata.
    Fresh,
    /// Snapshot may lag source-of-truth updates.
    PotentiallyStale,
}

/// Organization-level permission section for the current caller.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyOrganizationPermissions {
    /// Organization these effective permissions are scoped to.
    pub org_id: OrgId,
    /// Effective permission entries for this organization.
    pub permissions: Vec<MyPermissionEntry>,
}

/// Project-level permission section for the current caller.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyProjectPermissions {
    /// Project these effective permissions are scoped to.
    pub project_id: ProjectId,
    /// Effective permission entries for this project.
    pub permissions: Vec<MyPermissionEntry>,
}

/// One effective permission entry shown in the self-introspection view.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionEntry {
    /// Canonical permission identifier.
    pub permission: PermissionName,
    /// Effective state after policy is applied.
    pub effective_state: PermissionEffectiveState,
    /// How this permission was granted.
    pub grant_source: PermissionGrantSource,
    /// Stable proof/source reference for how this grant was derived.
    pub grant_source_reference: String,
    /// Why policy constrained this permission, when applicable.
    pub policy_constraint: Option<PermissionConstraintView>,
}

/// Optional policy-constraint detail for a permission entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PermissionConstraintView {
    /// Human-readable reason associated with the constraint.
    pub reason: PolicyConstraintReason,
    /// Scope that produced the constraint.
    pub source: PolicyConstraintSource,
    /// Stable proof/source reference for the constraining policy record.
    pub source_reference: String,
}

/// Shared machine-readable interface error body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InterfaceError {
    /// Stable error code from the shared interfaces taxonomy.
    pub code: InterfaceErrorCode,
    /// Human-readable summary for the caller.
    pub summary: String,
}

impl InterfaceError {
    /// Build a new interface error body.
    #[must_use]
    pub fn new(code: InterfaceErrorCode, summary: impl Into<String>) -> Self {
        Self {
            code,
            summary: summary.into(),
        }
    }
}

/// Shared interfaces error-code taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceErrorCode {
    /// No authenticated session was present, or it expired.
    AuthRequired,
    /// The authenticated actor is not allowed to perform this action.
    PermissionDenied,
    /// Request payload or parameters failed validation.
    ValidationFailed,
    /// Requested resource does not exist.
    NotFound,
    /// Request conflicts with current state.
    Conflict,
    /// Idempotency key does not match prior request payload.
    IdempotencyConflict,
    /// Read model is stale for the requested freshness guarantee.
    StaleProjection,
    /// Drift was detected between expected and observed state.
    DriftDetected,
    /// Request exceeded an enforced rate limit.
    RateLimited,
    /// Service dependency is currently unavailable.
    Unavailable,
    /// The action is recognized but unsupported for this actor or state.
    UnsupportedAction,
    /// Upstream provider returned a failure.
    ProviderFailure,
    /// Runtime execution failed.
    ExecutionFailure,
    /// Internal server error.
    InternalError,
    /// Submitted identifier already exists.
    DuplicateIdentifier,
    /// Submitted credential is invalid.
    InvalidCredential,
    /// Invitation token does not match a known invitation.
    InvitationNotFound,
    /// Invitation token is expired.
    InvitationExpired,
    /// Invitation token was already consumed or revoked.
    InvitationAlreadyConsumed,
}

impl InterfaceErrorCode {
    /// Stable wire `code` string used across every interface.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::ValidationFailed => "validation_failed",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::StaleProjection => "stale_projection",
            Self::DriftDetected => "drift_detected",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::UnsupportedAction => "unsupported_action",
            Self::ProviderFailure => "provider_failure",
            Self::ExecutionFailure => "execution_failure",
            Self::InternalError => "internal_error",
            Self::DuplicateIdentifier => "duplicate_identifier",
            Self::InvalidCredential => "invalid_credential",
            Self::InvitationNotFound => "invitation_not_found",
            Self::InvitationExpired => "invitation_expired",
            Self::InvitationAlreadyConsumed => "invitation_already_consumed",
        }
    }
}

/// Closed taxonomy of self-permission query failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MyPermissionsFailureReason {
    /// The caller attempted to introspect another account's permissions.
    PermissionDenied,
}

impl MyPermissionsFailureReason {
    /// Typed interface error-code for this failure.
    #[must_use]
    pub const fn interface_error_code(self) -> InterfaceErrorCode {
        match self {
            Self::PermissionDenied => InterfaceErrorCode::PermissionDenied,
        }
    }

    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PermissionDenied => "permission_denied",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::PermissionDenied => {
                "You can only view permissions for the authenticated account."
            }
        }
    }
}
