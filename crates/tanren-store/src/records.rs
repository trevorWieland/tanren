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
    AccountId, Identifier, InvitationToken, MembershipId, OrgId, PermissionEffectiveState,
    PermissionGrantSource, PermissionName, PolicyConstraintReason, PolicyConstraintSource,
    ProjectId, RoleTemplateName, SessionToken,
};

use crate::entity;
use crate::{
    PermissionConstraintId, PermissionGrantId, StoreError, parse_db_identifier,
    parse_db_invitation_token,
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

/// One policy-constraint detail attached to a constrained permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionConstraintRecord {
    /// Stable constraint id (opaque persistence key).
    pub id: PermissionConstraintId,
    /// Human-readable reason surfaced to callers.
    pub reason: PolicyConstraintReason,
    /// Scope that produced this constraint.
    pub source: PolicyConstraintSource,
}

impl From<entity::permission_constraints::Model> for PermissionConstraintRecord {
    fn from(model: entity::permission_constraints::Model) -> Self {
        let source = if model.is_project_policy {
            PolicyConstraintSource::ProjectPolicy
        } else {
            PolicyConstraintSource::OrganizationPolicy
        };
        Self {
            id: PermissionConstraintId::new(model.id),
            reason: PolicyConstraintReason::new(model.reason),
            source,
        }
    }
}

/// Persisted permission grant row used by test-hook seeders and
/// introspection queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionGrantRecord {
    /// Stable grant id (opaque persistence key).
    pub id: PermissionGrantId,
    /// Account this grant applies to.
    pub account_id: AccountId,
    /// Organization scope when this is an organization-level grant.
    pub org_id: Option<OrgId>,
    /// Project scope when this is a project-level grant.
    pub project_id: Option<ProjectId>,
    /// Canonical permission identifier.
    pub permission: PermissionName,
    /// How this permission grant entered the account's access set.
    pub grant_source: PermissionGrantSource,
    /// Wall-clock time the grant was recorded.
    pub created_at: DateTime<Utc>,
}

impl From<entity::permission_grants::Model> for PermissionGrantRecord {
    fn from(model: entity::permission_grants::Model) -> Self {
        let grant_source = match model.role_template_name {
            Some(role_template) => PermissionGrantSource::RoleTemplate {
                role_template: RoleTemplateName::new(role_template),
            },
            None => PermissionGrantSource::Direct,
        };

        Self {
            id: PermissionGrantId::new(model.id),
            account_id: AccountId::new(model.account_id),
            org_id: model.org_id.map(OrgId::new),
            project_id: model.project_id.map(ProjectId::new),
            permission: PermissionName::new(model.permission_name),
            grant_source,
            created_at: model.created_at,
        }
    }
}

/// One effective permission in a self-introspection response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyPermissionRecord {
    /// Stable grant id backing this effective permission row.
    pub grant_id: PermissionGrantId,
    /// Canonical permission identifier.
    pub permission: PermissionName,
    /// Effective state after policy constraints are applied.
    pub effective_state: PermissionEffectiveState,
    /// How this permission was granted.
    pub grant_source: PermissionGrantSource,
    /// Optional policy constraint when effective state is constrained.
    pub policy_constraint: Option<PermissionConstraintRecord>,
}

/// Organization-level self-introspection section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyOrganizationPermissionsRecord {
    /// Organization this section is scoped to.
    pub org_id: OrgId,
    /// Effective permissions for this organization.
    pub permissions: Vec<MyPermissionRecord>,
}

/// Project-level self-introspection section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyProjectPermissionsRecord {
    /// Project this section is scoped to.
    pub project_id: ProjectId,
    /// Effective permissions for this project.
    pub permissions: Vec<MyPermissionRecord>,
}

/// Read-model returned by self-permission introspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyPermissionsRecord {
    /// Cursor to continue pagination from this page, if any.
    pub next_cursor: Option<MyPermissionsCursor>,
    /// Read-model freshness metadata for this response.
    pub freshness: MyPermissionsFreshnessRecord,
    /// Organization-scoped permission sections.
    pub organizations: Vec<MyOrganizationPermissionsRecord>,
    /// Project-scoped permission sections.
    pub projects: Vec<MyProjectPermissionsRecord>,
}

/// Cursor payload for self-permissions pagination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyPermissionsCursor {
    /// Organization scope id of the last row, when cursor points to an
    /// organization-scoped permission.
    pub org_id: Option<OrgId>,
    /// Project scope id of the last row, when cursor points to a project-scoped
    /// permission.
    pub project_id: Option<ProjectId>,
    /// Permission name of the last row.
    pub permission_name: String,
    /// Role-template discriminator of the last row.
    pub role_template_name: Option<String>,
    /// Stable grant id tiebreaker of the last row.
    pub grant_id: PermissionGrantId,
}

/// Scope class discriminator used in cursor ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MyPermissionsScopeKind {
    /// Organization scope rows sort before project scope rows.
    Organization,
    /// Project scope rows sort after organization scope rows.
    Project,
}

impl MyPermissionsScopeKind {
    /// Integer rank used in SQL tuple ordering.
    #[must_use]
    pub const fn rank(self) -> i16 {
        match self {
            Self::Organization => 0,
            Self::Project => 1,
        }
    }
}

impl MyPermissionsCursor {
    /// Resolve the scope class from the cursor payload.
    pub fn scope_kind(&self) -> Result<MyPermissionsScopeKind, StoreError> {
        match (self.org_id, self.project_id) {
            (Some(_), None) => Ok(MyPermissionsScopeKind::Organization),
            (None, Some(_)) => Ok(MyPermissionsScopeKind::Project),
            _ => Err(StoreError::Invariant {
                entity: "permission_grants",
                detail: "cursor must carry exactly one scope id (org or project)",
            }),
        }
    }
}

/// Read-model freshness metadata for self-permission introspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyPermissionsFreshnessRecord {
    /// Canonical projection name serving this read model.
    pub projection: String,
    /// Source checkpoint describing what the read model has observed.
    pub checkpoint: Option<String>,
    /// Wall-clock instant when this snapshot was generated.
    pub generated_at: DateTime<Utc>,
    /// Whether this read model is stale for the requested consistency.
    pub is_stale: bool,
}

/// Pagination envelope for self-permission reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyPermissionsPage {
    /// Maximum number of permission entries to return.
    pub limit: u16,
    /// Opaque continuation token for cursor-based pagination.
    pub cursor: Option<MyPermissionsCursor>,
}

impl MyPermissionsPage {
    /// Workspace-wide default limit for self-permission reads.
    pub const DEFAULT_LIMIT: u16 = 100;
    /// Hard maximum limit for self-permission reads.
    pub const MAX_LIMIT: u16 = 200;

    /// Build a bounded page contract from an optional caller hint.
    #[must_use]
    pub fn bounded(limit: Option<u16>, cursor: Option<MyPermissionsCursor>) -> Self {
        let resolved = match limit {
            Some(0) | None => Self::DEFAULT_LIMIT,
            Some(value) => value.min(Self::MAX_LIMIT),
        };
        Self {
            limit: resolved,
            cursor,
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

/// Permission grant scope for test-hook seeding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionGrantScope {
    /// Organization-level grant.
    Organization(OrgId),
    /// Project-level grant.
    Project(ProjectId),
}

/// Input shape for [`crate::Store::seed_permission_grant`].
#[derive(Debug, Clone)]
pub struct NewPermissionGrant {
    /// Account receiving the grant.
    pub account_id: AccountId,
    /// Scope for this grant.
    pub scope: PermissionGrantScope,
    /// Canonical permission name.
    pub permission: PermissionName,
    /// Grant source metadata (direct vs role-template).
    pub grant_source: PermissionGrantSource,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
}

/// Input shape for [`crate::Store::seed_permission_constraint`].
#[derive(Debug, Clone)]
pub struct NewPermissionConstraint {
    /// Grant id this constraint decorates.
    pub grant_id: PermissionGrantId,
    /// Human-readable constraint reason.
    pub reason: PolicyConstraintReason,
    /// Scope that produced the constraint.
    pub source: PolicyConstraintSource,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
}
