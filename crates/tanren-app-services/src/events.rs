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
use tanren_contract::{AccountFailureReason, DriftConfigSource};
use tanren_identity_policy::{AccountId, InvitationToken, OrgId, ProjectId};
use uuid::Uuid;

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
    serde_json::json!({
        "family": EVENT_FAMILY,
        "kind": kind.as_str(),
        "payload": payload,
    })
}

// ---------------------------------------------------------------------------
// Projection drift events
// ---------------------------------------------------------------------------

/// Tag on the JSON envelope that disambiguates drift events from other
/// event families.
pub const DRIFT_EVENT_FAMILY: &str = "projection_drift";

/// Closed taxonomy of drift event kinds.
///
/// `xtask check-event-coverage` cross-references every variant against
/// BDD feature steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DriftEventKind {
    /// A projection drift evaluation completed.
    DriftEvaluated,
}

impl DriftEventKind {
    /// Stable wire `kind` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DriftEvaluated => "drift_evaluated",
        }
    }
}

/// Whether the drift run detected actionable drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftRemediationStatus {
    /// All checked assets match or are accepted.
    NoDrift,
    /// At least one asset is drifted or missing.
    DriftDetected,
}

/// A projection drift evaluation completed. Carries project scope,
/// checked asset identities, per-state counts, the effective policy
/// source, and whether remediation is required.
///
/// Relative paths only — no host-local absolute paths. No secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEvaluated {
    /// Project whose repository was checked.
    pub project_id: ProjectId,
    /// Unique id for this evaluation run (UUID v7, time-ordered).
    pub run_id: Uuid,
    /// Relative paths of every asset that was checked.
    pub checked_asset_paths: Vec<String>,
    /// Number of assets whose content differs from the canonical source.
    pub drift_count: usize,
    /// Number of assets absent from the repository.
    pub missing_count: usize,
    /// Number of preserved-standard assets accepted with user edits.
    pub accepted_count: usize,
    /// Number of assets matching the canonical content exactly.
    pub matches_count: usize,
    /// Effective drift and preservation policies applied during the check.
    pub config_source: DriftConfigSource,
    /// Whether the run detected actionable drift.
    pub remediation_status: DriftRemediationStatus,
}

/// Encode a typed drift event as the JSON envelope persisted in the
/// event log.
#[must_use]
pub fn drift_envelope<T: Serialize>(kind: DriftEventKind, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": DRIFT_EVENT_FAMILY,
        "kind": kind.as_str(),
        "payload": payload,
    })
}
