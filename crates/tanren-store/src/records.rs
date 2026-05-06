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
    CredentialId, CredentialKind, CredentialScope, UserSettingKey, UserSettingValue,
};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, MembershipId, OrgId, SessionToken,
};

use crate::entity;
use crate::{
    StoreError, credential_kind_from_db, credential_scope_from_db, parse_db_identifier,
    parse_db_invitation_token, setting_key_from_db,
};

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

/// Persisted user-tier configuration row. Exposed as a typed envelope;
/// the `key` and `value` carry the validated domain types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserConfigRecord {
    /// Account this setting belongs to.
    pub account_id: AccountId,
    /// The setting key.
    pub key: UserSettingKey,
    /// The validated value.
    pub value: UserSettingValue,
    /// Wall-clock time the value was last set.
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<entity::user_config_values::Model> for UserConfigRecord {
    type Error = StoreError;

    fn try_from(model: entity::user_config_values::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: AccountId::new(model.account_id),
            key: setting_key_from_db(&model.key)?,
            value: UserSettingValue::parse(&model.value).map_err(|cause| {
                StoreError::Deserialization {
                    entity: "user_config_values",
                    column: "value",
                    cause: cause.to_string(),
                }
            })?,
            updated_at: model.updated_at,
        })
    }
}

/// Persisted credential row — metadata only. The encrypted secret value is
/// intentionally excluded from this envelope so that callers of the store
/// read/list paths never receive raw (or even encrypted) credential
/// material.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialRecord {
    /// Stable credential identifier.
    pub id: CredentialId,
    /// Account this credential belongs to.
    pub account_id: AccountId,
    /// Credential kind from the typed registry.
    pub kind: CredentialKind,
    /// Ownership scope.
    pub scope: CredentialScope,
    /// Human-readable name chosen by the user.
    pub name: String,
    /// Optional longer description.
    pub description: Option<String>,
    /// Provider or adapter this credential targets.
    pub provider: Option<String>,
    /// Wall-clock time the credential was first stored.
    pub created_at: DateTime<Utc>,
    /// Wall-clock time the credential value was last replaced.
    pub updated_at: Option<DateTime<Utc>>,
    /// `true` when an encrypted value has been written.
    pub present: bool,
}

impl TryFrom<entity::user_credentials::Model> for CredentialRecord {
    type Error = StoreError;

    fn try_from(model: entity::user_credentials::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: CredentialId::new(model.id),
            account_id: AccountId::new(model.account_id),
            kind: credential_kind_from_db(&model.kind)?,
            scope: credential_scope_from_db(&model.scope)?,
            name: model.name,
            description: model.description,
            provider: model.provider,
            created_at: model.created_at,
            updated_at: model.updated_at,
            present: !model.encrypted_value.is_empty(),
        })
    }
}

/// Input shape for [`crate::AccountStore::set_user_config`].
#[derive(Debug, Clone)]
pub struct NewUserConfigValue {
    /// Account the setting belongs to.
    pub account_id: AccountId,
    /// Which user-tier setting to set.
    pub key: UserSettingKey,
    /// Validated setting value.
    pub value: UserSettingValue,
    /// Wall-clock time for the upsert.
    pub now: DateTime<Utc>,
}

/// Input shape for [`crate::AccountStore::add_credential`]. The caller
/// encrypts the secret value before handing it to the store.
#[derive(Debug, Clone)]
pub struct NewCredential {
    /// Account the credential belongs to.
    pub account_id: AccountId,
    /// Credential kind from the typed registry.
    pub kind: CredentialKind,
    /// Ownership scope.
    pub scope: CredentialScope,
    /// Human-readable name unique per owner + kind.
    pub name: String,
    /// Optional longer description.
    pub description: Option<String>,
    /// Provider or adapter this credential targets, if applicable.
    pub provider: Option<String>,
    /// Encrypted secret value. The store persists this blob as-is; the
    /// app-service layer is responsible for encryption.
    pub encrypted_value: Vec<u8>,
    /// Wall-clock time for the insert.
    pub now: DateTime<Utc>,
}

/// Input shape for [`crate::AccountStore::update_credential`]. Only
/// non-`None` fields are overwritten.
#[derive(Debug, Clone)]
pub struct UpdateCredential {
    /// Stable credential identifier to update.
    pub id: CredentialId,
    /// Updated human-readable name, if changing.
    pub name: Option<String>,
    /// Updated description, if changing.
    pub description: Option<String>,
    /// Replacement encrypted secret value.
    pub encrypted_value: Vec<u8>,
    /// Wall-clock time for the update.
    pub now: DateTime<Utc>,
}
