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
    AccountId, Identifier, InvitationToken, MembershipId, OrgId, PermissionGrantId, PermissionName,
    PermissionScope, PrincipalRef, RoleId, RoleName, RoleScope, ScopedRole, SessionToken,
};

use crate::entity;
use crate::{
    StoreError, parse_db_identifier, parse_db_invitation_token, parse_db_permission_name,
    parse_db_permission_scope, parse_db_principal_ref, parse_db_role_name, parse_db_role_scope,
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

/// Persisted role-template row with its permission bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleRecord {
    /// Stable role-template id.
    pub id: RoleId,
    /// Scope where the template is defined.
    pub scope: RoleScope,
    /// Human-readable template name.
    pub name: RoleName,
    /// Permissions bundled by this template.
    pub permissions: Vec<PermissionName>,
    /// Wall-clock time the role was created.
    pub created_at: DateTime<Utc>,
    /// Wall-clock time the role was last updated.
    pub updated_at: DateTime<Utc>,
}

impl RoleRecord {
    pub(crate) fn from_parts(
        role: entity::roles::Model,
        permissions: Vec<PermissionName>,
    ) -> Result<Self, StoreError> {
        let mut record = Self::try_from(role)?;
        record.permissions = permissions;
        Ok(record)
    }

    /// Role identity paired with scope.
    #[must_use]
    pub fn scoped_role(&self) -> ScopedRole {
        ScopedRole {
            role_id: self.id,
            scope: self.scope,
        }
    }
}

impl TryFrom<entity::roles::Model> for RoleRecord {
    type Error = StoreError;

    fn try_from(model: entity::roles::Model) -> Result<Self, Self::Error> {
        let scope = parse_db_role_scope(&model.scope_kind, model.scope_ref)?;
        let name = parse_db_role_name(&model.name)?;
        Ok(Self {
            id: RoleId::new(model.id),
            scope,
            name,
            permissions: Vec::new(),
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

/// Persisted role-template permission row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolePermissionRecord {
    /// Role template id this permission belongs to.
    pub role_id: RoleId,
    /// Granted permission in the role template bundle.
    pub permission: PermissionName,
    /// Wall-clock time the permission row was created.
    pub created_at: DateTime<Utc>,
}

impl TryFrom<entity::role_permissions::Model> for RolePermissionRecord {
    type Error = StoreError;

    fn try_from(model: entity::role_permissions::Model) -> Result<Self, Self::Error> {
        let permission = parse_db_permission_name(&model.permission_name)?;
        Ok(Self {
            role_id: RoleId::new(model.role_id),
            permission,
            created_at: model.created_at,
        })
    }
}

/// Persisted direct permission grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionGrantRecord {
    /// Stable grant id.
    pub id: PermissionGrantId,
    /// Principal receiving this grant.
    pub principal: PrincipalRef,
    /// Scope where this grant applies.
    pub scope: PermissionScope,
    /// Granted permission.
    pub permission: PermissionName,
    /// Role template id that produced this grant.
    pub source_role_id: RoleId,
    /// Actor that granted this permission.
    pub granted_by: PrincipalRef,
    /// Wall-clock time the grant was created.
    pub granted_at: DateTime<Utc>,
}

impl TryFrom<entity::permission_grants::Model> for PermissionGrantRecord {
    type Error = StoreError;

    fn try_from(model: entity::permission_grants::Model) -> Result<Self, Self::Error> {
        let principal = parse_db_principal_ref(&model.grantee_kind, model.grantee_ref)?;
        let scope = parse_db_permission_scope(&model.scope_kind, model.scope_ref)?;
        let permission = parse_db_permission_name(&model.permission_name)?;
        let granted_by = parse_db_principal_ref(&model.granted_by_kind, model.granted_by_ref)?;
        Ok(Self {
            id: PermissionGrantId::new(model.id),
            principal,
            scope,
            permission,
            source_role_id: RoleId::new(model.source_role_id),
            granted_by,
            granted_at: model.granted_at,
        })
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

/// Input shape for [`crate::RoleStore::create_role`].
#[derive(Debug, Clone)]
pub struct NewRole {
    /// Stable role id allocated by the caller.
    pub id: RoleId,
    /// Role scope.
    pub scope: RoleScope,
    /// Human-readable role name.
    pub name: RoleName,
    /// Permission bundle for this role.
    pub permissions: Vec<PermissionName>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-update timestamp (same as `created_at` for initial insert).
    pub updated_at: DateTime<Utc>,
}

/// Input shape for [`crate::RoleStore::edit_role`].
#[derive(Debug, Clone)]
pub struct EditRole {
    /// Role identifier + scope.
    pub role: ScopedRole,
    /// Replacement name.
    pub name: RoleName,
    /// Replacement permission bundle.
    pub permissions: Vec<PermissionName>,
    /// Mutation timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Input shape for [`crate::RoleStore::apply_role`].
#[derive(Debug, Clone)]
pub struct ApplyRole {
    /// Role to apply as the source template.
    pub role: ScopedRole,
    /// Principal receiving the direct grants.
    pub principal: PrincipalRef,
    /// Scope where grants are created.
    pub grant_scope: PermissionScope,
    /// Actor that performed the grant.
    pub granted_by: PrincipalRef,
    /// Wall-clock grant timestamp.
    pub granted_at: DateTime<Utc>,
}
