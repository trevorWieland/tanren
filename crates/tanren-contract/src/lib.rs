//! Wire-shape contracts for Tanren's API and MCP boundaries.
//!
//! Types in this crate are the serialization surface that interface binaries
//! (`tanren-api`, `tanren-mcp`, the generated web client) expose to external
//! callers. Orchestration logic does not live here — this crate stays a pure
//! shape layer so that wire compatibility is reviewable in isolation.

pub mod account;
pub mod approval_policy;
pub mod organization;

pub use account::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, AccountView,
    SessionEnvelope, SessionView, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
pub use approval_policy::{
    ApprovalPolicyFailureReason, ApprovalPolicyId, ApprovalPolicyView, ApprovalRequirement,
    ApprovalRuleView, CreateApprovalPolicyRequest, CreateApprovalPolicyResponse,
    DeleteApprovalPolicyRequest, DeleteApprovalPolicyResponse, GatedAction,
    ListApprovalPoliciesRequest, ListApprovalPoliciesResponse,
    ORGANIZATION_APPROVAL_POLICY_CREATE_BEHAVIOR_ID,
    ORGANIZATION_APPROVAL_POLICY_DELETE_BEHAVIOR_ID, ORGANIZATION_APPROVAL_POLICY_LIST_BEHAVIOR_ID,
    ORGANIZATION_APPROVAL_POLICY_UPDATE_BEHAVIOR_ID, UpdateApprovalPolicyRequest,
    UpdateApprovalPolicyResponse,
};
pub use organization::{
    CheckOrganizationPermissionApiRequest, CheckOrganizationPermissionRequest,
    CheckOrganizationPermissionResponse, CreateOrganizationApiRequest,
    CreateOrganizationFailureReason, CreateOrganizationRequest, CreateOrganizationResponse,
    LIST_ORGANIZATIONS_DEFAULT_LIMIT, LIST_ORGANIZATIONS_MAX_LIMIT, ListOrganizationsApiQuery,
    ListOrganizationsRequest, ListOrganizationsResponse, ORGANIZATION_CREATE_BEHAVIOR_ID,
    ORGANIZATION_CREATED_EVENT_KIND, ORGANIZATION_EVENT_FAMILY, OrganizationBehaviorId,
    OrganizationCapabilityView, OrganizationCreatedEvent, OrganizationEventReference,
    OrganizationFailureBody, OrganizationFailureCode, OrganizationProjectSummary,
    OrganizationProofLink, OrganizationSourceLink, OrganizationView, ReadModelFreshness,
    organization_capability_projection, organization_permission_options,
};
pub use tanren_identity_policy::{
    AccountId, IdempotencyKey, MembershipId, OrgId, OrganizationName, OrganizationPermission,
    SessionToken,
};
pub use tanren_observation::{ClaimValueKind, CompletenessState, FreshnessState, VisibilityState};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Wire-shape version for Tanren's external contract surface.
///
/// Bumped on breaking changes to the request/response shapes this crate
/// exports. The wire version moves independently of [`tanren_domain`'s
/// `DomainVersion`](../tanren_domain/struct.DomainVersion.html) — the wire
/// format may stay stable across domain refactors and vice versa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContractVersion(u32);

impl ContractVersion {
    /// Current wire-contract version.
    pub const CURRENT: Self = Self(0);

    /// Construct a contract version from its numeric form.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// The numeric value of this wire version.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Errors raised when a wire payload fails contract-level validation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContractError {
    /// The payload's declared contract version is incompatible with this build.
    #[error("incompatible contract version: payload={payload:?}, supported={supported:?}")]
    IncompatibleVersion {
        /// Version declared by the incoming payload.
        payload: ContractVersion,
        /// Version this build understands.
        supported: ContractVersion,
    },
}
