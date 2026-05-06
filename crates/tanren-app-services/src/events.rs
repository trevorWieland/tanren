//! Typed event payloads written to the canonical Tanren event log on
//! every account-flow side effect. Payloads are serialised into the
//! existing `events.payload` JSON column — no migration is required.
//!
//! Newtype IDs flow through transparently: `AccountId` /  `OrgId`
//! serialise as the bare UUID via `#[serde(transparent)]`, so the
//! on-disk JSON shape is unchanged across the type substitution that
//! lands in PR 3.
//!
//! The standards event family (`"standards"`) captures configuration
//! changes to the installed-standards root. The read-model in
//! [`crate::standards`] replays these events to derive the effective
//! standards root — the filesystem config (`tanren.yml`) is never the
//! canonical source of truth for effective configuration.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{AccountFailureReason, StandardSchema};
use tanren_identity_policy::{AccountId, InvitationToken, OrgId};

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

// ── Standards event family ───────────────────────────────────────────

/// Tag on the JSON envelope that disambiguates standards events from
/// account events.
pub const STANDARDS_EVENT_FAMILY: &str = "standards";

/// Closed taxonomy of standards event kinds.
///
/// Each variant corresponds to a configuration change that the
/// [`crate::standards::StandardsReadModel`] replays to derive the
/// effective standards root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StandardsEventKind {
    /// The standards root directory was configured.
    StandardsRootConfigured,
    /// The standards root configuration was cleared.
    StandardsRootCleared,
}

impl StandardsEventKind {
    /// Stable wire `kind` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StandardsRootConfigured => "standards_root_configured",
            Self::StandardsRootCleared => "standards_root_cleared",
        }
    }
}

/// A standards root directory was configured.
///
/// Emitted when Tanren assets are installed (R-0023) and the standards
/// root is recorded in the canonical event log. The
/// [`crate::standards::StandardsReadModel`] projects this event to
/// derive the effective standards root — callers must not read
/// `tanren.yml` directly for effective configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardsRootConfigured {
    /// Absolute path to the configured standards root directory.
    pub standards_root: PathBuf,
    /// Schema version of the standards configuration.
    pub schema: StandardSchema,
    /// Wall-clock time the configuration was recorded.
    pub at: DateTime<Utc>,
}

/// The standards root configuration was cleared.
///
/// Emitted when the installed standards are removed or reset. The
/// read-model projects this to an empty state so subsequent queries
/// return [`crate::standards::StandardsResolutionError::NotConfigured`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardsRootCleared {
    /// Wall-clock time the configuration was cleared.
    pub at: DateTime<Utc>,
}

/// Encode a typed standards event as the JSON envelope persisted in the
/// event log. Mirrors [`envelope`] for the account family.
#[must_use]
pub fn standards_envelope<T: Serialize>(
    kind: StandardsEventKind,
    payload: &T,
) -> serde_json::Value {
    serde_json::json!({
        "family": STANDARDS_EVENT_FAMILY,
        "kind": kind.as_str(),
        "payload": payload,
    })
}
