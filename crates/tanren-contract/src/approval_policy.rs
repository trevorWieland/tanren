//! Approval-policy command/response wire shapes.
//!
//! Request/response surface for the api, mcp, cli, tui, and web client
//! when callers create, update, delete, or list organization approval
//! policies.  Shapes mirror the conventions in [`super::organization`]:
//! explicit session tokens, idempotency keys, and stable error taxonomy.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use tanren_identity_policy::{AccountId, IdempotencyKey, OrgId, SessionToken};
use utoipa::ToSchema;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Behavior IDs
// ---------------------------------------------------------------------------

/// Canonical behavior proof id for creating an organization approval policy.
pub const ORGANIZATION_APPROVAL_POLICY_CREATE_BEHAVIOR_ID: &str = "B-0040-create";
/// Canonical behavior proof id for updating an organization approval policy.
pub const ORGANIZATION_APPROVAL_POLICY_UPDATE_BEHAVIOR_ID: &str = "B-0040-update";
/// Canonical behavior proof id for deleting an organization approval policy.
pub const ORGANIZATION_APPROVAL_POLICY_DELETE_BEHAVIOR_ID: &str = "B-0040-delete";
/// Canonical behavior proof id for listing organization approval policies.
pub const ORGANIZATION_APPROVAL_POLICY_LIST_BEHAVIOR_ID: &str = "B-0040-list";

// ---------------------------------------------------------------------------
// Domain ID newtype
// ---------------------------------------------------------------------------

/// Stable identifier for an organization approval policy. `UUIDv7` — sortable + unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct ApprovalPolicyId(Uuid);

impl ApprovalPolicyId {
    /// Wrap a raw UUID.
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Allocate a fresh time-ordered id.
    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    /// The underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for ApprovalPolicyId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for ApprovalPolicyId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for ApprovalPolicyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ---------------------------------------------------------------------------
// View types
// ---------------------------------------------------------------------------

/// Actions that can be gated by an approval policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum GatedAction {
    /// Creating or modifying a behavior spec.
    EditSpec,
    /// Creating or modifying a roadmap item.
    EditRoadmap,
    /// Creating or modifying an architecture record.
    EditArchitecture,
    /// Merging or publishing a change.
    PublishChange,
}

impl GatedAction {
    /// Stable kebab-case wire key for this gated action.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EditSpec => "edit-spec",
            Self::EditRoadmap => "edit-roadmap",
            Self::EditArchitecture => "edit-architecture",
            Self::PublishChange => "publish-change",
        }
    }
}

impl fmt::Display for GatedAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for GatedAction {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "edit-spec" => Ok(Self::EditSpec),
            "edit-roadmap" => Ok(Self::EditRoadmap),
            "edit-architecture" => Ok(Self::EditArchitecture),
            "publish-change" => Ok(Self::PublishChange),
            _ => Err("unknown gated action"),
        }
    }
}

/// Whether a gated action requires approval and how many approvals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequirement {
    /// No approval needed — the action proceeds immediately.
    None,
    /// Any single designated approver suffices.
    SingleApproval,
    /// A majority of designated approvers must approve.
    MajorityApproval,
    /// All designated approvers must approve.
    UnanimousApproval,
}

impl ApprovalRequirement {
    /// Stable kebab-case wire key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SingleApproval => "single-approval",
            Self::MajorityApproval => "majority-approval",
            Self::UnanimousApproval => "unanimous-approval",
        }
    }
}

impl fmt::Display for ApprovalRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ApprovalRequirement {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "single-approval" => Ok(Self::SingleApproval),
            "majority-approval" => Ok(Self::MajorityApproval),
            "unanimous-approval" => Ok(Self::UnanimousApproval),
            _ => Err("unknown approval requirement"),
        }
    }
}

/// A single rule within an approval policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ApprovalRuleView {
    /// The action this rule gates.
    pub gated_action: GatedAction,
    /// What kind of approval is required for this action.
    pub requirement: ApprovalRequirement,
    /// Account IDs designated as approvers for this rule.
    pub approver_ids: Vec<AccountId>,
}

/// External-facing view of an organization approval policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ApprovalPolicyView {
    /// Stable approval-policy id.
    pub id: ApprovalPolicyId,
    /// Owning organization.
    pub org_id: OrgId,
    /// Human-readable name for this policy.
    pub name: String,
    /// Description of the policy's purpose.
    pub description: String,
    /// Ordered list of rules in this policy.
    pub rules: Vec<ApprovalRuleView>,
    /// Whether the policy is currently active.
    pub enabled: bool,
    /// Account that created the policy.
    pub created_by: AccountId,
    /// Timestamp when the policy was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the policy was last updated.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

/// Create-approval-policy request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateApprovalPolicyRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account creating the approval policy.
    pub account_id: AccountId,
    /// Organization that will own this policy.
    pub org_id: OrgId,
    /// Human-readable name for the new policy.
    pub name: String,
    /// Description of the policy's purpose.
    pub description: String,
    /// Initial rules for the policy.
    pub rules: Vec<ApprovalRuleView>,
    /// Whether the policy is active on creation.
    pub enabled: bool,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Create-approval-policy response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateApprovalPolicyResponse {
    /// Newly created approval policy.
    pub policy: ApprovalPolicyView,
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

/// Update-approval-policy request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateApprovalPolicyRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account updating the approval policy.
    pub account_id: AccountId,
    /// Policy to update.
    pub policy_id: ApprovalPolicyId,
    /// Updated human-readable name.
    pub name: Option<String>,
    /// Updated description.
    pub description: Option<String>,
    /// Updated rules (replaces all rules atomically).
    pub rules: Option<Vec<ApprovalRuleView>>,
    /// Updated enabled state.
    pub enabled: Option<bool>,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Update-approval-policy response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateApprovalPolicyResponse {
    /// Updated approval policy.
    pub policy: ApprovalPolicyView,
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

/// Delete-approval-policy request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeleteApprovalPolicyRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account deleting the approval policy.
    pub account_id: AccountId,
    /// Policy to delete.
    pub policy_id: ApprovalPolicyId,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Delete-approval-policy response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeleteApprovalPolicyResponse {
    /// Id of the deleted policy.
    pub policy_id: ApprovalPolicyId,
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

/// List-approval-policies request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListApprovalPoliciesRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account requesting the list.
    pub account_id: AccountId,
    /// Organization whose policies are requested.
    pub org_id: OrgId,
    /// Maximum page size the caller asks for.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<String>,
}

/// List-approval-policies response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListApprovalPoliciesResponse {
    /// Approval policies visible to the requesting account.
    pub policies: Vec<ApprovalPolicyView>,
    /// Opaque cursor callers can pass to fetch the next page.
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// Error taxonomy
// ---------------------------------------------------------------------------

/// Closed taxonomy of approval-policy failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects an
/// `ApprovalPolicyFailureReason` into the same wire shape so callers can
/// match on `code` regardless of transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ApprovalPolicyFailureReason {
    /// The request requires authentication and no valid session was supplied.
    AuthRequired,
    /// The actor is authenticated but cannot perform the requested action.
    PermissionDenied,
    /// The referenced approval policy does not exist.
    PolicyNotFound,
    /// The submitted policy payload failed validation.
    InvalidPolicy,
    /// Idempotency key reused with conflicting request fingerprint.
    IdempotencyConflict,
    /// Concurrent edit conflict on the same policy.
    ConflictConcurrentEdit,
}

impl ApprovalPolicyFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::PolicyNotFound => "policy_not_found",
            Self::InvalidPolicy => "invalid_policy",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::ConflictConcurrentEdit => "conflict_concurrent_edit",
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
            Self::PolicyNotFound => "The referenced approval policy does not exist.",
            Self::InvalidPolicy => "The submitted approval policy failed validation.",
            Self::IdempotencyConflict => {
                "The supplied idempotency key conflicts with a prior request."
            }
            Self::ConflictConcurrentEdit => {
                "A concurrent edit has been applied to the same policy."
            }
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::AuthRequired => 401,
            Self::PermissionDenied => 403,
            Self::PolicyNotFound => 404,
            Self::InvalidPolicy => 400,
            Self::IdempotencyConflict | Self::ConflictConcurrentEdit => 409,
        }
    }
}
