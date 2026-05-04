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
    /// Permission bitfield. Bits map to [`tanren_identity_policy::OrgAdminPermissions`]
    /// flags (invite=0, `manage_access=1`, configure=2, `set_policy=3`, delete=4).
    pub permissions: u32,
}

impl From<entity::memberships::Model> for MembershipRecord {
    fn from(model: entity::memberships::Model) -> Self {
        Self {
            id: MembershipId::new(model.id),
            account_id: AccountId::new(model.account_id),
            org_id: OrgId::new(model.org_id),
            created_at: model.created_at,
            permissions: model.permissions as u32,
        }
    }
}

/// Persisted organization row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationRecord {
    /// Stable organization id (`UUIDv7`).
    pub id: OrgId,
    /// Display name as entered by the creator.
    pub name: String,
    /// Case-normalized name used for the uniqueness constraint.
    pub name_normalized: String,
    /// Wall-clock time the organization was created.
    pub created_at: DateTime<Utc>,
}

impl From<entity::organizations::Model> for OrganizationRecord {
    fn from(model: entity::organizations::Model) -> Self {
        Self {
            id: OrgId::new(model.id),
            name: model.name,
            name_normalized: model.name_normalized,
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
