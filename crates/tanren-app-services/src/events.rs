//! Typed event payloads written to the canonical Tanren event log on
//! every account-flow side effect. Payloads are serialised into the
//! existing `events.payload` JSON column — no migration is required.
//!
//! Newtype IDs flow through transparently: `AccountId` /  `OrgId`
//! serialise as the bare UUID via `#[serde(transparent)]`, so the
//! on-disk JSON shape is unchanged across the type substitution that
//! lands in PR 3.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{AccountFailureReason, RoleFailureReason};
use tanren_identity_policy::{
    AccountId, InvitationToken, OrgId, PermissionGrantId, PermissionName, PermissionScope,
    PrincipalRef, RoleName, ScopedRole,
};

/// Tag on the JSON envelope that disambiguates account events from
/// future event families.
pub const EVENT_FAMILY: &str = "account";
/// Tag on the JSON envelope that disambiguates role events from other
/// event families.
pub const ROLE_EVENT_FAMILY: &str = "role";

/// Closed taxonomy of account-flow event kinds.
///
/// `xtask check-event-coverage` cross-references every variant against
/// BDD feature steps to ensure each kind has at least one assertion. The
/// kind also serialises to the JSON envelope's `kind` field so log
/// consumers can filter without parsing the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountEventKind {
    /// A new account was created (self-signup or invitation acceptance).
    AccountCreated,
    /// An existing account signed in.
    SignedIn,
    /// An invitation was accepted (paired with `AccountCreated` for the
    /// new invitee account).
    InvitationAccepted,
    /// A sign-up was rejected — duplicate identifier, validation failure,
    /// or other taxonomy reason.
    SignUpRejected,
    /// A sign-in was rejected — invalid credential or validation failure.
    SignInFailed,
    /// An invitation acceptance was rejected — not found / expired /
    /// already consumed / validation failure.
    InvitationAcceptFailed,
}

impl AccountEventKind {
    /// Stable wire `kind` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AccountCreated => "account_created",
            Self::SignedIn => "signed_in",
            Self::InvitationAccepted => "invitation_accepted",
            Self::SignUpRejected => "sign_up_rejected",
            Self::SignInFailed => "sign_in_failed",
            Self::InvitationAcceptFailed => "invitation_accept_failed",
        }
    }
}

/// A new account was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCreated {
    /// Stable account id.
    pub account_id: AccountId,
    /// User-facing identifier (email).
    pub identifier: String,
    /// Owning organization — `None` for self-signup, `Some` for invitation flows.
    pub org: Option<OrgId>,
    /// Wall-clock time the account was created.
    pub created_at: DateTime<Utc>,
}

/// An existing account signed in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedIn {
    /// Account that signed in.
    pub account_id: AccountId,
    /// Wall-clock time the session was minted.
    pub at: DateTime<Utc>,
}

/// An invitation was accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationAccepted {
    /// Token that was consumed.
    pub token: InvitationToken,
    /// Account that resulted from acceptance.
    pub account_id: AccountId,
    /// Organization the new account joined.
    pub joined_org: OrgId,
    /// Wall-clock time of acceptance.
    pub at: DateTime<Utc>,
}

/// A sign-up attempt was rejected. Carries the [`AccountFailureReason`]
/// so audit consumers can distinguish duplicate identifiers from
/// validation failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignUpRejected {
    /// Why the attempt was rejected.
    pub reason: AccountFailureReason,
    /// Email the caller submitted (case-folded; safe to log).
    pub identifier: String,
    /// Wall-clock time the rejection was emitted.
    pub at: DateTime<Utc>,
}

/// A sign-in attempt was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignInFailed {
    /// Why the attempt failed.
    pub reason: AccountFailureReason,
    /// Email the caller submitted (case-folded; safe to log).
    pub identifier: String,
    /// Wall-clock time the rejection was emitted.
    pub at: DateTime<Utc>,
}

/// An invitation-acceptance attempt was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationAcceptFailed {
    /// Why the attempt was rejected.
    pub reason: AccountFailureReason,
    /// Token the caller submitted.
    pub token: InvitationToken,
    /// Wall-clock time the rejection was emitted.
    pub at: DateTime<Utc>,
}

/// Encode a typed event as the JSON envelope persisted in the event log.
#[must_use]
pub fn envelope<T: Serialize>(kind: AccountEventKind, payload: &T) -> serde_json::Value {
    envelope_with_family(EVENT_FAMILY, kind.as_str(), payload)
}

/// Role lifecycle event labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleKind {
    /// A new role template was created.
    Created,
    /// An existing role template was edited.
    Edited,
    /// A role template was deleted.
    Deleted,
    /// A role template was applied to a principal.
    Applied,
    /// A direct permission grant was revoked.
    GrantRevoked,
    /// Authorization check rejected because principal was a role id.
    AuthorizationPrincipalRejected,
}

impl RoleKind {
    /// Stable wire `kind` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "role_created",
            Self::Edited => "role_edited",
            Self::Deleted => "role_deleted",
            Self::Applied => "role_applied",
            Self::GrantRevoked => "permission_grant_revoked",
            Self::AuthorizationPrincipalRejected => "authorization_principal_rejected",
        }
    }
}

/// A role template was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleCreated {
    /// Role identifier and scope.
    pub role: ScopedRole,
    /// Role template name.
    pub name: RoleName,
    /// Permission bundle captured at creation.
    pub permissions: Vec<PermissionName>,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
}

/// A role template was edited.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleEdited {
    /// Role identifier and scope.
    pub role: ScopedRole,
    /// Replacement role template name.
    pub name: RoleName,
    /// Replacement permission bundle.
    pub permissions: Vec<PermissionName>,
    /// Wall-clock edit time.
    pub edited_at: DateTime<Utc>,
}

/// A role template was deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDeleted {
    /// Deleted role identifier and scope.
    pub role: ScopedRole,
    /// Wall-clock deletion time.
    pub deleted_at: DateTime<Utc>,
}

/// A role template was applied to a principal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleApplied {
    /// Applied role identifier and scope.
    pub role: ScopedRole,
    /// Principal that received direct grants.
    pub principal: PrincipalRef,
    /// Scope where direct grants were written.
    pub grant_scope: PermissionScope,
    /// Stable ids of grants returned by apply-role.
    pub grant_ids: Vec<PermissionGrantId>,
    /// Permissions snapshot applied during this operation.
    pub permissions: Vec<PermissionName>,
    /// Wall-clock apply time.
    pub applied_at: DateTime<Utc>,
}

/// A direct permission grant was revoked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionGrantRevoked {
    /// Stable grant identifier.
    pub grant_id: PermissionGrantId,
    /// Principal the grant belongs to.
    pub principal: PrincipalRef,
    /// Scope the grant was evaluated in.
    pub grant_scope: PermissionScope,
    /// Permission that was revoked.
    pub permission: PermissionName,
    /// Actor that revoked the grant.
    pub revoked_by: PrincipalRef,
    /// Wall-clock revocation time.
    pub revoked_at: DateTime<Utc>,
}

/// An authorization check was rejected for role-as-principal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationPrincipalRejected {
    /// Rejected principal.
    pub principal: PrincipalRef,
    /// Permission that was requested.
    pub permission: PermissionName,
    /// Scope where permission was requested.
    pub scope: PermissionScope,
    /// Rejection reason.
    pub reason: RoleFailureReason,
    /// Wall-clock rejection time.
    pub at: DateTime<Utc>,
}

/// Encode a typed role event as the JSON envelope persisted in the event log.
#[must_use]
pub fn role_envelope<T: Serialize>(kind: RoleKind, payload: &T) -> serde_json::Value {
    envelope_with_family(ROLE_EVENT_FAMILY, kind.as_str(), payload)
}

fn envelope_with_family<T: Serialize>(family: &str, kind: &str, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": family,
        "kind": kind,
        "payload": payload,
    })
}
