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
    AccountId, ApprovalPolicyId, GatedAction, IdempotencyKey, Identifier, InvitationToken,
    MembershipId, OrgId, OrganizationName, OrganizationPermission, SessionToken, ValidationError,
};

use crate::entity;
use crate::{
    StoreError, parse_db_idempotency_key, parse_db_identifier, parse_db_invitation_token,
    parse_db_organization_name, parse_db_organization_permission,
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

/// Persisted organization row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationRecord {
    /// Stable organization id.
    pub id: OrgId,
    /// Normalized organization name uniqueness key.
    pub name: OrganizationName,
    /// Account that created this organization row.
    pub created_by_account_id: AccountId,
    /// Wall-clock time the organization was created.
    pub created_at: DateTime<Utc>,
}

impl TryFrom<entity::organizations::Model> for OrganizationRecord {
    type Error = StoreError;

    fn try_from(model: entity::organizations::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: OrgId::new(model.id),
            name: parse_db_organization_name(&model.name)?,
            created_by_account_id: AccountId::new(model.created_by_account_id),
            created_at: model.created_at,
        })
    }
}

/// Persisted organization-create idempotency record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationCreateIdempotencyRecord {
    /// Account that issued the create request.
    pub account_id: AccountId,
    /// Stable idempotency key scoped to the account.
    pub key: IdempotencyKey,
    /// Organization created by the idempotent request.
    pub organization_id: OrgId,
    /// Normalized organization name from the original create request.
    pub organization_name: OrganizationName,
    /// Stable request fingerprint used for conflict checks.
    pub request_fingerprint: String,
    /// Wall-clock time the idempotency record was created.
    pub created_at: DateTime<Utc>,
}

impl TryFrom<entity::organization_create_idempotency::Model>
    for OrganizationCreateIdempotencyRecord
{
    type Error = StoreError;

    fn try_from(
        model: entity::organization_create_idempotency::Model,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: AccountId::new(model.account_id),
            key: parse_db_idempotency_key(&model.key)?,
            organization_id: OrgId::new(model.organization_id),
            organization_name: parse_db_organization_name(&model.organization_name)?,
            request_fingerprint: model.request_fingerprint,
            created_at: model.created_at,
        })
    }
}

/// Persisted organization-level permission grant row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationPermissionGrantRecord {
    /// Organization in which the permission applies.
    pub org_id: OrgId,
    /// Account receiving this grant.
    pub account_id: AccountId,
    /// Granted organization-level permission.
    pub permission: OrganizationPermission,
    /// Account that granted this permission.
    pub granted_by_account_id: AccountId,
    /// Wall-clock time the grant was created.
    pub created_at: DateTime<Utc>,
}

impl TryFrom<entity::organization_permission_grants::Model> for OrganizationPermissionGrantRecord {
    type Error = StoreError;

    fn try_from(model: entity::organization_permission_grants::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            org_id: OrgId::new(model.org_id),
            account_id: AccountId::new(model.account_id),
            permission: parse_db_organization_permission(&model.permission)?,
            granted_by_account_id: AccountId::new(model.granted_by_account_id),
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

/// Persisted approval-policy row — gates an organization action behind
/// required approvals from members holding a specific permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalPolicyRecord {
    /// Stable policy row id.
    pub id: ApprovalPolicyId,
    /// Organization that owns this policy.
    pub org_id: OrgId,
    /// The action gated by this policy.
    pub gated_action: GatedAction,
    /// Number of distinct approvals required (> 0).
    pub required_approvals: u16,
    /// Permission an approver must hold to satisfy this gate.
    pub permitted_approver_permission: String,
    /// Optimistic-concurrency version — incremented on each update.
    pub version: u16,
    /// Wall-clock time the policy was created.
    pub created_at: DateTime<Utc>,
    /// Wall-clock time the policy was last updated.
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<entity::approval_policies::Model> for ApprovalPolicyRecord {
    type Error = StoreError;

    fn try_from(model: entity::approval_policies::Model) -> Result<Self, Self::Error> {
        let gated_action =
            GatedAction::parse(&model.gated_action).map_err(|err| StoreError::DataInvariant {
                column: "gated_action",
                cause: err,
            })?;
        Ok(Self {
            id: ApprovalPolicyId::new(model.id),
            org_id: OrgId::new(model.org_id),
            gated_action,
            required_approvals: u16::try_from(model.required_approvals).map_err(|_| {
                StoreError::DataInvariant {
                    column: "required_approvals",
                    cause: ValidationError::GatedActionEmpty,
                }
            })?,
            permitted_approver_permission: model.permitted_approver_permission,
            version: u16::try_from(model.version).map_err(|_| StoreError::DataInvariant {
                column: "version",
                cause: ValidationError::GatedActionEmpty,
            })?,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}
