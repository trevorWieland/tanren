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
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, NotificationEventType, UserSettingKey,
};
use tanren_contract::{AccountFailureReason, ConfigurationFailureReason};
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

pub const CONFIGURATION_EVENT_FAMILY: &str = "configuration";

pub const USER_CONFIG_SET_REJECTED_KIND: &str = "user_config_set_rejected";
pub const CREDENTIAL_ADD_REJECTED_KIND: &str = "credential_add_rejected";
pub const CREDENTIAL_UPDATE_REJECTED_KIND: &str = "credential_update_rejected";
pub const CREDENTIAL_REMOVE_REJECTED_KIND: &str = "credential_remove_rejected";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfigSetRejected {
    pub account_id: AccountId,
    pub key: UserSettingKey,
    pub reason: ConfigurationFailureReason,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialAddRejected {
    pub account_id: AccountId,
    pub name: String,
    pub kind: CredentialKind,
    pub reason: ConfigurationFailureReason,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialUpdateRejected {
    pub id: CredentialId,
    pub account_id: AccountId,
    pub reason: ConfigurationFailureReason,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialRemoveRejected {
    pub id: CredentialId,
    pub account_id: AccountId,
    pub reason: ConfigurationFailureReason,
    pub at: DateTime<Utc>,
}

#[must_use]
pub fn configuration_envelope<T: Serialize>(kind: &str, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": CONFIGURATION_EVENT_FAMILY,
        "kind": kind,
        "payload": payload,
    })
}

pub const NOTIFICATION_EVENT_FAMILY: &str = "notification";

pub const NOTIFICATION_PREFERENCE_REJECTED_KIND: &str = "notification_preference_rejected";
pub const NOTIFICATION_ORG_OVERRIDE_REJECTED_KIND: &str = "notification_org_override_rejected";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferenceRejected {
    pub account_id: AccountId,
    pub event_type: NotificationEventType,
    pub reason: ConfigurationFailureReason,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationOrgOverrideRejected {
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub reason: ConfigurationFailureReason,
    pub at: DateTime<Utc>,
}

#[must_use]
pub fn notification_envelope<T: Serialize>(kind: &str, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": NOTIFICATION_EVENT_FAMILY,
        "kind": kind,
        "payload": payload,
    })
}
