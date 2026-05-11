//! Invitation command/response wire shapes.
//!
//! Request/response surface for the api, mcp, cli, tui, and web client
//! when callers create, list, look up, and revoke organization invitations.
//! Per the architecture, invitations carry an opaque recipient identifier
//! (token-based in R-0006), the target org, selected organization
//! permissions, lifecycle status, and proof/source links. Existing-account
//! acceptance (B-0045) remains out of scope for R-0006.

use crate::organization::ReadModelFreshness;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use tanren_identity_policy::{
    AccountId, IdempotencyKey, InvitationId, InvitationToken, OrgId, OrganizationPermission,
    SessionToken,
};
use tanren_observation::VisibilityState;
use utoipa::{IntoParams, ToSchema};

/// Event family for invitation lifecycle events.
pub const INVITATION_EVENT_FAMILY: &str = "invitation";
/// Event kind for invitation-created events.
pub const INVITATION_CREATED_EVENT_KIND: &str = "invitation_created";
/// Event kind for invitation-revoked events.
pub const INVITATION_REVOKED_EVENT_KIND: &str = "invitation_revoked";
/// Canonical behavior proof id for invitation operations.
pub const INVITATION_CREATE_BEHAVIOR_ID: &str = "B-0044";
/// Default page size for listing invitations.
pub const LIST_INVITATIONS_DEFAULT_LIMIT: u64 = 50;
/// Maximum allowed page size for listing invitations.
pub const LIST_INVITATIONS_MAX_LIMIT: u64 = 100;

/// Create-invitation request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateInvitationRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account creating the invitation.
    pub account_id: AccountId,
    /// Organization the invitee will join.
    pub org_id: OrgId,
    /// Opaque recipient identifier — token-based in R-0006.
    pub recipient_identifier: InvitationToken,
    /// Organization-level permissions the invitee will receive on acceptance.
    pub permissions: Vec<OrganizationPermission>,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

impl CreateInvitationRequest {
    /// Build from authenticated transport context plus the validated API body.
    #[must_use]
    pub fn from_api(
        session_token: SessionToken,
        account_id: AccountId,
        org_id: OrgId,
        body: CreateInvitationApiRequest,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id,
            recipient_identifier: body.recipient_identifier,
            permissions: body.permissions,
            idempotency_key: body.idempotency_key,
        }
    }
}

/// API body for create-invitation routes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateInvitationApiRequest {
    /// Opaque recipient identifier — token-based in R-0006.
    pub recipient_identifier: InvitationToken,
    /// Organization-level permissions the invitee will receive on acceptance.
    pub permissions: Vec<OrganizationPermission>,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Create-invitation response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateInvitationResponse {
    /// Newly created invitation view.
    pub invitation: InvitationView,
    /// Stable proof reference.
    pub proof_link: InvitationProofLink,
    /// Stable source reference for the creation event.
    pub source_link: InvitationSourceLink,
}

/// List-invitations request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListInvitationsRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account whose invitations are requested.
    pub account_id: AccountId,
    /// Organization whose invitations are listed.
    pub org_id: OrgId,
    /// Maximum page size.
    pub limit: Option<u64>,
    /// Opaque page cursor returned by a previous list call.
    pub cursor: Option<InvitationId>,
}

impl ListInvitationsRequest {
    /// Build from authenticated transport context and query parameters.
    #[must_use]
    pub fn from_api_query(
        session_token: SessionToken,
        account_id: AccountId,
        org_id: OrgId,
        query: &ListInvitationsApiQuery,
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

/// List-invitations response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListInvitationsResponse {
    /// Invitations visible to the requesting account.
    pub invitations: Vec<InvitationView>,
    /// Opaque cursor for the next page.
    pub next_cursor: Option<InvitationId>,
    /// Source link for invitation lifecycle events in this view.
    pub source_link: InvitationSourceLink,
    /// Read-model freshness metadata.
    pub freshness: ReadModelFreshness,
}

/// Query parameters for listing invitations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListInvitationsApiQuery {
    /// Maximum page size.
    pub limit: Option<u64>,
    /// Opaque page cursor.
    pub cursor: Option<InvitationId>,
}

/// Lookup-invitation request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct LookupInvitationRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account performing the lookup.
    pub account_id: AccountId,
    /// Organization the invitation belongs to.
    pub org_id: OrgId,
    /// Invitation id to look up.
    pub invitation_id: InvitationId,
}

impl LookupInvitationRequest {
    /// Build from authenticated transport context and path parameters.
    #[must_use]
    pub fn from_api(
        session_token: SessionToken,
        account_id: AccountId,
        org_id: OrgId,
        invitation_id: InvitationId,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id,
            invitation_id,
        }
    }
}

/// Lookup-invitation response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct LookupInvitationResponse {
    /// The requested invitation view.
    pub invitation: InvitationView,
    /// Stable proof reference.
    pub proof_link: InvitationProofLink,
    /// Source reference for invitation events.
    pub source_link: InvitationSourceLink,
}

/// Revoke-invitation request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RevokeInvitationRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account revoking the invitation.
    pub account_id: AccountId,
    /// Organization the invitation belongs to.
    pub org_id: OrgId,
    /// Invitation id to revoke.
    pub invitation_id: InvitationId,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

impl RevokeInvitationRequest {
    /// Build from authenticated transport context, path parameters, and API body.
    #[must_use]
    pub fn from_api(
        session_token: SessionToken,
        account_id: AccountId,
        org_id: OrgId,
        invitation_id: InvitationId,
        body: RevokeInvitationApiRequest,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id,
            invitation_id,
            idempotency_key: body.idempotency_key,
        }
    }
}

/// API body for revoke-invitation routes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RevokeInvitationApiRequest {
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Revoke-invitation response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RevokeInvitationResponse {
    /// The revoked invitation view.
    pub invitation: InvitationView,
    /// Stable proof reference for the revocation.
    pub proof_link: InvitationProofLink,
    /// Source reference for the revocation event.
    pub source_link: InvitationSourceLink,
}

/// External-facing view of an invitation.
///
/// Exposes recipient identifier, org id, selected organization
/// permissions, status, proof/source links, and non-secret visibility.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InvitationView {
    /// Stable invitation id.
    pub id: InvitationId,
    /// Opaque recipient identifier — token-based in R-0006.
    pub recipient_identifier: InvitationToken,
    /// Organization the invitee will join.
    pub org_id: OrgId,
    /// Account that created the invitation.
    pub created_by: AccountId,
    /// Organization-level permissions the invitee will receive.
    pub permissions: Vec<OrganizationPermission>,
    /// Current lifecycle status.
    pub status: InvitationStatus,
    /// Visibility of recipient details for the requesting actor.
    pub recipient_visibility: VisibilityState,
    /// Wall-clock time the invitation was created.
    pub created_at: DateTime<Utc>,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
    /// Wall-clock time the invitation was consumed. `None` while pending.
    pub consumed_at: Option<DateTime<Utc>>,
}

/// Lifecycle status of an invitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InvitationStatus {
    /// Sent and not yet accepted or revoked.
    Pending,
    /// Accepted; the invitee is now a member.
    Accepted,
    /// Revoked by an authorized caller.
    Revoked,
    /// Expired without being accepted.
    Expired,
}

impl InvitationStatus {
    /// Stable wire key for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

impl fmt::Display for InvitationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InvitationStatus {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "revoked" => Ok(Self::Revoked),
            "expired" => Ok(Self::Expired),
            _ => Err("unknown invitation status"),
        }
    }
}

/// Reference to behavior proof coverage for invitation operations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InvitationProofLink {
    /// Canonical behavior id proving this command contract.
    pub behavior_id: InvitationBehaviorId,
}

/// Reference to source evidence for invitation events.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InvitationSourceLink {
    /// Event family in the canonical event log.
    pub event_family: String,
    /// Event kind in the canonical event log.
    pub event_kind: String,
}

/// Source event reference for invitation responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InvitationEventReference {
    /// Event family.
    pub event_family: String,
    /// Event kind.
    pub event_kind: String,
    /// Stable event id.
    pub event_id: String,
    /// Cursor for event position.
    pub cursor: String,
    /// Event-log append timestamp.
    pub occurred_at: DateTime<Utc>,
}

/// Canonical behavior id taxonomy for invitation operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub enum InvitationBehaviorId {
    /// `B-0044` — Invite a person to an organization.
    #[serde(rename = "B-0044")]
    B0044InviteToOrganization,
}

impl InvitationBehaviorId {
    /// Stable string form used in wire contracts and witnesses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::B0044InviteToOrganization => INVITATION_CREATE_BEHAVIOR_ID,
        }
    }
}

impl fmt::Display for InvitationBehaviorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InvitationBehaviorId {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            INVITATION_CREATE_BEHAVIOR_ID => Ok(Self::B0044InviteToOrganization),
            _ => Err("unknown invitation behavior id"),
        }
    }
}

/// Canonical contract projection of invitation-related organization permissions.
#[must_use]
pub fn invitation_permission_options() -> Vec<OrganizationPermission> {
    OrganizationPermission::ALL.to_vec()
}
