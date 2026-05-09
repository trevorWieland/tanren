//! Typed envelopes for persisted rows.
//!
//! Other workspace crates consume rows through these envelopes — the
//! underlying `SeaORM` `Model` types stay crate-private. This file is
//! the seam between the `SeaORM`-shaped DB row and the domain newtype
//! shape produced by `tanren-identity-policy`.

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::{
    OwnerScope, UserCredentialKind, UserCredentialMetadata, UserCredentialStatus, UserSettingKey,
    UserSettingValue,
};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, MembershipId, OrgId, SessionToken,
};

use crate::entity;
use crate::{StoreError, parse_db_identifier, parse_db_invitation_token};

/// Persisted account row, exposed as a typed envelope so other crates
/// never see `SeaORM` `Model` types directly. R-0001 stores the
/// password as an Argon2id PHC string (`$argon2id$v=19$m=...$<salt>$<hash>`)
/// — salt is embedded in the string so the row carries a single TEXT
/// column, not a hash + salt pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecord {
    /// Stable account id.
    pub id: AccountId,
    /// User-facing identifier (email).
    pub identifier: Identifier,
    /// Display name.
    pub display_name: String,
    /// PHC-format hash string. Public-by-design hash output — the
    /// embedded salt is per-row, the parameters are recoverable, and
    /// no plaintext leaks. Verification goes through
    /// `CredentialVerifier::verify`.
    pub password_phc: String,
    /// Wall-clock time the account was created.
    pub created_at: DateTime<Utc>,
    /// Owning organization — `None` for personal (self-signup) accounts.
    pub org_id: Option<OrgId>,
}

impl TryFrom<entity::accounts::Model> for AccountRecord {
    type Error = StoreError;

    fn try_from(model: entity::accounts::Model) -> Result<Self, Self::Error> {
        let identifier = parse_db_identifier(&model.identifier)?;
        Ok(Self {
            id: AccountId::new(model.id),
            identifier,
            display_name: model.display_name,
            password_phc: model.password_phc,
            created_at: model.created_at,
            org_id: model.org_id.map(OrgId::new),
        })
    }
}

/// Persisted invitation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvitationRecord {
    /// Opaque invitation token (PK).
    pub token: InvitationToken,
    /// Organization the new account joins on acceptance.
    pub inviting_org_id: OrgId,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
    /// Set when the invitation has been accepted (or revoked).
    pub consumed_at: Option<DateTime<Utc>>,
}

impl TryFrom<entity::invitations::Model> for InvitationRecord {
    type Error = StoreError;

    fn try_from(model: entity::invitations::Model) -> Result<Self, Self::Error> {
        let token = parse_db_invitation_token(&model.token)?;
        Ok(Self {
            token,
            inviting_org_id: OrgId::new(model.inviting_org_id),
            expires_at: model.expires_at,
            consumed_at: model.consumed_at,
        })
    }
}

/// Persisted membership row — links an account to an organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipRecord {
    /// Stable membership id.
    pub id: MembershipId,
    /// Account this membership belongs to.
    pub account_id: AccountId,
    /// Organization the account is a member of.
    pub org_id: OrgId,
    /// Wall-clock time the membership was created.
    pub created_at: DateTime<Utc>,
}

impl From<entity::memberships::Model> for MembershipRecord {
    fn from(model: entity::memberships::Model) -> Self {
        Self {
            id: MembershipId::new(model.id),
            account_id: AccountId::new(model.account_id),
            org_id: OrgId::new(model.org_id),
            created_at: model.created_at,
        }
    }
}

/// Persisted session row — issued by `tanren-app-services` on
/// successful sign-up / sign-in / invitation acceptance.
///
/// The matching DB column for `expires_at` lands in
/// `m20260503_000002_account_sessions_expires_at`; callers thread the
/// computed expiry (`now + 30 days`) on every insert and the verifier
/// path filters `WHERE expires_at > now`.
#[derive(Debug, Clone)]
pub struct SessionRecord {
    /// Opaque session token (PK).
    pub token: SessionToken,
    /// Account this session belongs to.
    pub account_id: AccountId,
    /// Wall-clock time the session was issued.
    pub created_at: DateTime<Utc>,
    /// Wall-clock time the session expires.
    pub expires_at: DateTime<Utc>,
}

impl From<entity::account_sessions::Model> for SessionRecord {
    fn from(model: entity::account_sessions::Model) -> Self {
        Self {
            token: SessionToken::from_secret(SecretString::from(model.token)),
            account_id: AccountId::new(model.account_id),
            created_at: model.created_at,
            expires_at: model.expires_at,
        }
    }
}

/// Persisted user-tier setting row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserSettingRecord {
    /// Stable row id.
    pub id: String,
    /// Owning account.
    pub account_id: AccountId,
    /// Owning scope for this setting (`user`).
    pub owner_scope: OwnerScope,
    /// Setting key.
    pub key: UserSettingKey,
    /// Typed setting value.
    pub value: UserSettingValue,
    /// Wall-clock time the setting was created.
    pub created_at: DateTime<Utc>,
    /// Wall-clock time the setting was last updated.
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<entity::user_config_values::Model> for UserSettingRecord {
    type Error = StoreError;

    fn try_from(model: entity::user_config_values::Model) -> Result<Self, Self::Error> {
        let account_id = AccountId::new(model.account_id);
        let owner_scope = parse_owner_scope(&model.owner_scope, account_id)?;
        let key = parse_user_setting_key(&model.key)?;
        let value: UserSettingValue =
            serde_json::from_value(model.value_json).map_err(|source| StoreError::DataDecode {
                column: "user_config_values.value_json",
                source,
            })?;
        let expected_kind = user_setting_kind_to_db(&value);
        if model.value_kind != expected_kind {
            return Err(StoreError::InvalidStoreValue {
                column: "user_config_values.value_kind",
                detail: model.value_kind,
            });
        }
        Ok(Self {
            id: model.id.to_string(),
            account_id,
            owner_scope,
            key,
            value,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

/// Persisted user-owned credential metadata row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOwnedItemRecord {
    /// Stable credential metadata id.
    pub id: String,
    /// Owning account.
    pub account_id: AccountId,
    /// Credential kind.
    pub kind: UserCredentialKind,
    /// Owning scope (`user`).
    pub owner_scope: OwnerScope,
    /// Lifecycle status.
    pub status: UserCredentialStatus,
    /// Wall-clock time the metadata row was created.
    pub created_at: DateTime<Utc>,
    /// Wall-clock time the metadata row was last updated.
    pub updated_at: DateTime<Utc>,
}

impl UserOwnedItemRecord {
    /// Convert to the contract/domain redacted metadata shape.
    #[must_use]
    pub fn into_metadata(self) -> UserCredentialMetadata {
        UserCredentialMetadata {
            id: self.id,
            kind: self.kind,
            owner_scope: self.owner_scope,
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl TryFrom<entity::user_credentials::Model> for UserOwnedItemRecord {
    type Error = StoreError;

    fn try_from(model: entity::user_credentials::Model) -> Result<Self, Self::Error> {
        let account_id = AccountId::new(model.account_id);
        let owner_scope = parse_owner_scope(&model.owner_scope, account_id)?;
        let kind = parse_user_item_kind(&model.kind)?;
        let status = parse_user_item_status(&model.status)?;
        Ok(Self {
            id: model.id.to_string(),
            account_id,
            kind,
            owner_scope,
            status,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

/// Persisted encrypted-value row metadata for a user-owned credential.
///
/// This intentionally omits nonce/ciphertext bytes; callers can inspect
/// ownership and write freshness without ever receiving secret material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOwnedValueRecord {
    /// Stable encrypted value row id.
    pub id: String,
    /// Metadata id this value belongs to.
    pub item_id: String,
    /// Owning account.
    pub account_id: AccountId,
    /// Last encrypted write timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<entity::user_credential_values::Model> for UserOwnedValueRecord {
    fn from(model: entity::user_credential_values::Model) -> Self {
        Self {
            id: model.id.to_string(),
            item_id: model.item_id.to_string(),
            account_id: AccountId::new(model.account_id),
            updated_at: model.updated_at,
        }
    }
}

/// Input shape for [`crate::AccountStore::insert_account`].
#[derive(Debug, Clone)]
pub struct NewAccount {
    /// Stable id allocated by the caller (`UUIDv7`).
    pub id: AccountId,
    /// User-facing identifier (email).
    pub identifier: Identifier,
    /// Display name.
    pub display_name: String,
    /// Argon2id PHC string (`$argon2id$v=19$...$<salt>$<hash>`). The
    /// caller threads this in from
    /// `CredentialVerifier::hash(&request.password)`.
    pub password_phc: String,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
    /// Owning organization — `None` for personal (self-signup) accounts.
    pub org_id: Option<OrgId>,
}

/// Input shape for [`crate::Store::seed_invitation`].
#[derive(Debug, Clone)]
pub struct NewInvitation {
    /// Opaque token shared with the invitee out of band.
    pub token: InvitationToken,
    /// Organization the new account joins on acceptance.
    pub inviting_org_id: OrgId,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
}

pub(crate) fn user_setting_key_to_db(key: UserSettingKey) -> &'static str {
    match key {
        UserSettingKey::Theme => "theme",
        UserSettingKey::Editor => "editor",
    }
}

pub(crate) fn user_setting_kind_to_db(value: &UserSettingValue) -> &'static str {
    match value {
        UserSettingValue::Theme(_) => "theme",
        UserSettingValue::Editor(_) => "editor",
    }
}

pub(crate) fn owner_scope_to_db(scope: OwnerScope) -> (&'static str, AccountId) {
    match scope {
        OwnerScope::User { account_id } => ("user", account_id),
    }
}

pub(crate) fn user_item_kind_to_db(kind: UserCredentialKind) -> &'static str {
    match kind {
        UserCredentialKind::ProviderApiToken => "provider_api_token",
        UserCredentialKind::HarnessApiToken => "harness_api_token",
    }
}

pub(crate) fn user_item_status_to_db(status: UserCredentialStatus) -> &'static str {
    match status {
        UserCredentialStatus::Pending => "pending",
        UserCredentialStatus::Active => "active",
        UserCredentialStatus::Invalid => "invalid",
    }
}

fn parse_user_setting_key(raw: &str) -> Result<UserSettingKey, StoreError> {
    match raw {
        "theme" => Ok(UserSettingKey::Theme),
        "editor" => Ok(UserSettingKey::Editor),
        _ => Err(StoreError::InvalidStoreValue {
            column: "user_config_values.key",
            detail: raw.to_owned(),
        }),
    }
}

fn parse_owner_scope(raw: &str, account_id: AccountId) -> Result<OwnerScope, StoreError> {
    match raw {
        "user" => Ok(OwnerScope::User { account_id }),
        _ => Err(StoreError::InvalidStoreValue {
            column: "owner_scope",
            detail: raw.to_owned(),
        }),
    }
}

fn parse_user_item_kind(raw: &str) -> Result<UserCredentialKind, StoreError> {
    match raw {
        "provider_api_token" => Ok(UserCredentialKind::ProviderApiToken),
        "harness_api_token" => Ok(UserCredentialKind::HarnessApiToken),
        _ => Err(StoreError::InvalidStoreValue {
            column: "user_credentials.kind",
            detail: raw.to_owned(),
        }),
    }
}

fn parse_user_item_status(raw: &str) -> Result<UserCredentialStatus, StoreError> {
    match raw {
        "pending" => Ok(UserCredentialStatus::Pending),
        "active" => Ok(UserCredentialStatus::Active),
        "invalid" => Ok(UserCredentialStatus::Invalid),
        _ => Err(StoreError::InvalidStoreValue {
            column: "user_credentials.status",
            detail: raw.to_owned(),
        }),
    }
}
