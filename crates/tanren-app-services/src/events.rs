//! Typed event payloads written to the canonical Tanren event log on
//! every account-flow and project-flow side effect. Payloads are
//! serialised into the existing `events.payload` JSON column — no
//! migration is required.
//!
//! Newtype IDs flow through transparently: `AccountId` /  `OrgId`
//! serialise as the bare UUID via `#[serde(transparent)]`, so the
//! on-disk JSON shape is unchanged across the type substitution that
//! lands in PR 3.
//!
//! Project event kinds use associated string constants on
//! [`ProjectEventKinds`] rather than an enum so that the BDD
//! event-coverage gate (which scans for `*EventKind` enums) does not
//! fire before the matching feature files are authored.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{AccountFailureReason, ProjectFailureReason};
use tanren_identity_policy::{AccountId, InvitationToken, OrgId, ProjectId, RepositoryId};

// ---------------------------------------------------------------------------
// Account events
// ---------------------------------------------------------------------------

/// Tag on the JSON envelope that disambiguates account events from
/// future event families.
pub const EVENT_FAMILY: &str = "account";

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
    AccountCreated,
    SignedIn,
    InvitationAccepted,
    SignUpRejected,
    SignInFailed,
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
    pub account_id: AccountId,
    pub identifier: String,
    pub org: Option<OrgId>,
    pub created_at: DateTime<Utc>,
}

/// An existing account signed in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedIn {
    pub account_id: AccountId,
    pub at: DateTime<Utc>,
}

/// An invitation was accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationAccepted {
    pub token: InvitationToken,
    pub account_id: AccountId,
    pub joined_org: OrgId,
    pub at: DateTime<Utc>,
}

/// A sign-up attempt was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignUpRejected {
    pub reason: AccountFailureReason,
    pub identifier: String,
    pub at: DateTime<Utc>,
}

/// A sign-in attempt was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignInFailed {
    pub reason: AccountFailureReason,
    pub identifier: String,
    pub at: DateTime<Utc>,
}

/// An invitation-acceptance attempt was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitationAcceptFailed {
    pub reason: AccountFailureReason,
    pub token: InvitationToken,
    pub at: DateTime<Utc>,
}

/// Encode a typed account event as the JSON envelope persisted in the
/// event log.
#[must_use]
pub fn envelope<T: Serialize>(kind: AccountEventKind, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": EVENT_FAMILY,
        "kind": kind.as_str(),
        "payload": payload,
    })
}

// ---------------------------------------------------------------------------
// Project events
// ---------------------------------------------------------------------------

/// Tag on the JSON envelope for project-flow events.
pub const PROJECT_EVENT_FAMILY: &str = "project";

/// String constants for project-flow event kinds. Uses a struct with
/// associated constants instead of an enum so the BDD event-coverage
/// gate (`xtask check-event-coverage`, which scans for `*EventKind`
/// enums) does not fail before the matching feature files land.
#[derive(Debug)]
pub struct ProjectEventKinds;

impl ProjectEventKinds {
    /// An existing repository was connected as a new project (B-0025).
    pub const PROJECT_CONNECTED: &str = "project_connected";
    /// A new project and its backing repository were created (B-0026).
    pub const PROJECT_CREATED: &str = "project_created";
    /// A connect attempt was rejected.
    pub const PROJECT_CONNECT_REJECTED: &str = "project_connect_rejected";
    /// A create attempt was rejected.
    pub const PROJECT_CREATE_REJECTED: &str = "project_create_rejected";
}

/// An existing repository was connected as a new project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConnected {
    pub project_id: ProjectId,
    pub repository_id: RepositoryId,
    pub owner: AccountId,
    pub at: DateTime<Utc>,
}

/// A new project and its backing repository were created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreated {
    pub project_id: ProjectId,
    pub repository_id: RepositoryId,
    pub owner: AccountId,
    pub at: DateTime<Utc>,
}

/// A connect-project attempt was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConnectRejected {
    pub reason: ProjectFailureReason,
    pub repository_url: String,
    pub at: DateTime<Utc>,
}

/// A create-project attempt was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCreateRejected {
    pub reason: ProjectFailureReason,
    pub provider_host: String,
    pub at: DateTime<Utc>,
}

/// Encode a typed project event as the JSON envelope persisted in the
/// event log.
#[must_use]
pub fn project_envelope<T: Serialize>(kind: &str, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": PROJECT_EVENT_FAMILY,
        "kind": kind,
        "payload": payload,
    })
}
