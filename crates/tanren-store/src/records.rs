//! Typed envelopes for persisted rows.
//!
//! Other workspace crates consume rows through these envelopes — the
//! underlying `SeaORM` `Model` types stay crate-private. This file is
//! the seam between the `SeaORM`-shaped DB row and the domain newtype
//! shape produced by `tanren-identity-policy`.

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, MembershipId, OrgId, OrganizationPermission,
    SessionToken,
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

/// Parse a JSON array value into a `Vec<OrganizationPermission>`.
pub(crate) fn parse_permissions_json(
    value: &serde_json::Value,
) -> Result<Vec<OrganizationPermission>, StoreError> {
    let arr = value.as_array().ok_or_else(|| StoreError::DataInvariant {
        column: "granted_permissions",
        cause: tanren_identity_policy::ValidationError::EmptyPermissionName,
    })?;
    let mut perms = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item.as_str().ok_or_else(|| StoreError::DataInvariant {
            column: "granted_permissions",
            cause: tanren_identity_policy::ValidationError::EmptyPermissionName,
        })?;
        perms.push(
            OrganizationPermission::parse(s).map_err(|err| StoreError::DataInvariant {
                column: "granted_permissions",
                cause: err,
            })?,
        );
    }
    Ok(perms)
}

/// Serialize a `Vec<OrganizationPermission>` into a JSON array value.
fn permissions_to_json(perms: &[OrganizationPermission]) -> serde_json::Value {
    serde_json::Value::Array(
        perms
            .iter()
            .map(|p| serde_json::Value::String(p.as_str().to_owned()))
            .collect(),
    )
}

/// Persisted invitation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvitationRecord {
    /// Opaque invitation token (PK).
    pub token: InvitationToken,
    /// Organization the new account joins on acceptance.
    pub inviting_org_id: OrgId,
    /// Recipient identifier (the invitee's email / identifier).
    pub recipient_identifier: Identifier,
    /// Permissions granted on acceptance.
    pub granted_permissions: Vec<OrganizationPermission>,
    /// Account id of the admin who created this invitation.
    pub created_by_account_id: AccountId,
    /// Wall-clock time the invitation was created.
    pub created_at: DateTime<Utc>,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
    /// Set when the invitation has been consumed.
    pub consumed_at: Option<DateTime<Utc>>,
    /// Set when the invitation has been revoked before consumption.
    pub revoked_at: Option<DateTime<Utc>>,
}

impl TryFrom<entity::invitations::Model> for InvitationRecord {
    type Error = StoreError;

    fn try_from(model: entity::invitations::Model) -> Result<Self, Self::Error> {
        let token = parse_db_invitation_token(&model.token)?;
        let recipient_identifier = parse_db_identifier(&model.recipient_identifier)?;
        let granted_permissions = parse_permissions_json(&model.granted_permissions)?;
        Ok(Self {
            token,
            inviting_org_id: OrgId::new(model.inviting_org_id),
            recipient_identifier,
            granted_permissions,
            created_by_account_id: AccountId::new(model.created_by_account_id),
            created_at: model.created_at,
            expires_at: model.expires_at,
            consumed_at: model.consumed_at,
            revoked_at: model.revoked_at,
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
    /// Permissions granted within the organization.
    pub permissions: Vec<OrganizationPermission>,
    /// Wall-clock time the membership was created.
    pub created_at: DateTime<Utc>,
}

impl TryFrom<entity::memberships::Model> for MembershipRecord {
    type Error = StoreError;

    fn try_from(model: entity::memberships::Model) -> Result<Self, Self::Error> {
        let permissions = parse_permissions_json(&model.permissions)?;
        Ok(Self {
            id: MembershipId::new(model.id),
            account_id: AccountId::new(model.account_id),
            org_id: OrgId::new(model.org_id),
            permissions,
            created_at: model.created_at,
        })
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

/// Input shape for [`crate::AccountStore::create_invitation`].
#[derive(Debug, Clone)]
pub struct CreateInvitation {
    /// Opaque token shared with the invitee out of band.
    pub token: InvitationToken,
    /// Organization the new account joins on acceptance.
    pub inviting_org_id: OrgId,
    /// Recipient identifier (the invitee's email / identifier).
    pub recipient_identifier: Identifier,
    /// Permissions granted on acceptance.
    pub granted_permissions: Vec<OrganizationPermission>,
    /// Account id of the admin who created this invitation.
    pub created_by_account_id: AccountId,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
}

pub(crate) fn granted_permissions_json(perms: &[OrganizationPermission]) -> serde_json::Value {
    permissions_to_json(perms)
}
