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
    OwnerScope, UserCredentialId, UserCredentialKind, UserCredentialStatus, UserSettingKey,
    UserSettingValueKind,
};
use tanren_contract::AccountFailureReason;
use tanren_identity_policy::{AccountId, InvitationToken, OrgId};

/// Tag on the JSON envelope that disambiguates account events from
/// future event families.
pub const EVENT_FAMILY: &str = "account";
/// Event family tag for user-tier configuration and user-owned credential
/// lifecycle metadata changes.
pub const CONFIGURATION_EVENT_FAMILY: &str = "configuration";

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

/// Closed taxonomy of user-tier configuration / credential metadata events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConfigurationEventType {
    /// A user-tier setting was created or updated.
    UserSettingChanged,
    /// A user-tier setting was removed.
    UserSettingRemoved,
    /// A user-owned credential metadata row was created or updated.
    UserCredentialChanged,
    /// A user-owned credential metadata row was removed.
    UserCredentialRemoved,
    /// A configuration operation was rejected (denied/not-found/validation).
    ConfigurationOperationRejected,
}

impl ConfigurationEventType {
    /// Stable wire `kind` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserSettingChanged => "user_setting_changed",
            Self::UserSettingRemoved => "user_setting_removed",
            Self::UserCredentialChanged => "user_credential_changed",
            Self::UserCredentialRemoved => "user_credential_removed",
            Self::ConfigurationOperationRejected => "configuration_operation_rejected",
        }
    }
}

/// Redaction posture for credential lifecycle event payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialEventRedactionState {
    /// Event includes metadata only; raw secret value is never present.
    MetadataOnly,
}

/// Stable configuration operation key for rejection audit events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationOperation {
    /// List user-tier settings.
    ListUserSettings,
    /// Create or update one user-tier setting.
    UpsertUserSetting,
    /// Remove one user-tier setting.
    RemoveUserSetting,
    /// List user-owned credential metadata.
    ListUserCredentials,
    /// Create one user-owned credential.
    AddUserCredential,
    /// Update one user-owned credential value.
    UpdateUserCredential,
    /// Remove one user-owned credential.
    RemoveUserCredential,
}

/// Closed safe-failure taxonomy for configuration rejection audit events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationOperationFailureReason {
    /// Contract/domain validation failed.
    ValidationFailed,
    /// Setting was denied or not found.
    SettingNotFound,
    /// Credential metadata row was denied or not found.
    ItemNotFound,
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

/// A user-tier setting was created or updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettingChanged {
    /// Actor that performed the write.
    pub actor: AccountId,
    /// Scope impacted by the write.
    pub scope: OwnerScope,
    /// Setting key changed.
    pub key: UserSettingKey,
    /// Value kind changed (`theme` / `editor`), never the raw value.
    pub value_kind: UserSettingValueKind,
    /// Write timestamp.
    pub updated_at: DateTime<Utc>,
}

/// A user-tier setting was removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettingRemoved {
    /// Actor that performed the removal.
    pub actor: AccountId,
    /// Scope impacted by the removal.
    pub scope: OwnerScope,
    /// Setting key removed.
    pub key: UserSettingKey,
    /// Removal timestamp.
    pub removed_at: DateTime<Utc>,
}

/// A user-owned credential metadata row was created or updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCredentialChanged {
    /// Actor that performed the write.
    pub actor: AccountId,
    /// Scope impacted by the write.
    pub scope: OwnerScope,
    /// Stable metadata id.
    pub item_id: UserCredentialId,
    /// Credential kind.
    pub kind: UserCredentialKind,
    /// Metadata lifecycle status.
    pub status: UserCredentialStatus,
    /// Credential-value redaction posture.
    pub redaction_state: CredentialEventRedactionState,
    /// Metadata write timestamp.
    pub updated_at: DateTime<Utc>,
}

/// A user-owned credential metadata row was removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCredentialRemoved {
    /// Actor that performed the removal.
    pub actor: AccountId,
    /// Scope impacted by the removal.
    pub scope: OwnerScope,
    /// Stable metadata id.
    pub item_id: UserCredentialId,
    /// Credential kind for auditability after deletion.
    pub kind: UserCredentialKind,
    /// Credential-value redaction posture.
    pub redaction_state: CredentialEventRedactionState,
    /// Removal timestamp.
    pub removed_at: DateTime<Utc>,
}

/// Safe rejection audit signal for configuration operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationOperationRejected {
    /// Actor that attempted the operation.
    pub actor: AccountId,
    /// Requested scope for the operation.
    pub scope: OwnerScope,
    /// Operation attempted.
    pub operation: ConfigurationOperation,
    /// Safe reason code (no hidden-resource detail).
    pub reason: ConfigurationOperationFailureReason,
    /// Requested setting key, when the operation targets a setting key.
    pub setting_key: Option<UserSettingKey>,
    /// Requested metadata id, when the operation targets a credential row.
    pub metadata_item_id: Option<UserCredentialId>,
    /// Rejection timestamp.
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

/// Encode a typed configuration/credential metadata event as a JSON envelope.
#[must_use]
pub fn configuration_envelope<T: Serialize>(
    kind: ConfigurationEventType,
    payload: &T,
) -> serde_json::Value {
    serde_json::json!({
        "family": CONFIGURATION_EVENT_FAMILY,
        "kind": kind.as_str(),
        "payload": payload,
    })
}
