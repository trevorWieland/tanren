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
    AccountId, DesignatedHost, Identifier, InvitationToken, MembershipId, OrgId, ProjectId,
    ProviderFamily, RepositoryRef, SessionToken,
};

use crate::entity;
use crate::{
    StoreError, parse_db_designated_host, parse_db_identifier, parse_db_invitation_token,
    parse_db_project_id, parse_db_provider_family, parse_db_repository_ref,
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

/// Persisted project row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    /// Stable project id.
    pub id: ProjectId,
    /// Account that owns the project.
    pub owning_account_id: AccountId,
    /// Wall-clock time the project was created.
    pub created_at: DateTime<Utc>,
    /// Wall-clock time the project was selected as active, if active.
    pub active_selected_at: Option<DateTime<Utc>>,
}

impl TryFrom<entity::projects::Model> for ProjectRecord {
    type Error = StoreError;

    fn try_from(model: entity::projects::Model) -> Result<Self, Self::Error> {
        let id = parse_db_project_id(model.id, "projects.id")?;
        Ok(Self {
            id,
            owning_account_id: AccountId::new(model.owning_account_id),
            created_at: model.created_at,
            active_selected_at: model.active_selected_at,
        })
    }
}

/// Persisted project repository row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRepositoryRecord {
    /// Project bound to this repository.
    pub project_id: ProjectId,
    /// Account that owns the project/repository binding.
    pub owning_account_id: AccountId,
    /// Canonical repository identity (`owner/name`).
    pub repository_ref: RepositoryRef,
    /// Source-control provider family for this binding.
    pub provider_family: ProviderFamily,
    /// Designated host key used for repository operations.
    pub designated_host: DesignatedHost,
    /// Wall-clock time the binding was created.
    pub created_at: DateTime<Utc>,
}

impl TryFrom<entity::project_repositories::Model> for ProjectRepositoryRecord {
    type Error = StoreError;

    fn try_from(model: entity::project_repositories::Model) -> Result<Self, Self::Error> {
        let repository_ref = parse_db_repository_ref(&model.repository_ref)?;
        let provider_family = parse_db_provider_family(&model.provider_family)?;
        let designated_host = parse_db_designated_host(&model.designated_host)?;
        let project_id = parse_db_project_id(model.project_id, "project_repositories.project_id")?;
        Ok(Self {
            project_id,
            owning_account_id: AccountId::new(model.owning_account_id),
            repository_ref,
            provider_family,
            designated_host,
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

/// Input shape for [`crate::ProjectStore::insert_project`].
#[derive(Debug, Clone)]
pub struct NewProject {
    /// Stable id allocated by the caller (`UUIDv7`).
    pub id: ProjectId,
    /// Account that owns the project.
    pub owning_account_id: AccountId,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
    /// Active-selection timestamp. `None` means inactive.
    pub active_selected_at: Option<DateTime<Utc>>,
}

/// Input shape for [`crate::ProjectStore::insert_project_repository`].
#[derive(Debug, Clone)]
pub struct NewProjectRepository {
    /// Project bound to this repository.
    pub project_id: ProjectId,
    /// Account that owns the binding.
    pub owning_account_id: AccountId,
    /// Canonical repository identity (`owner/name`).
    pub repository_ref: RepositoryRef,
    /// Source-control provider family for this binding.
    pub provider_family: ProviderFamily,
    /// Designated host key used for repository operations.
    pub designated_host: DesignatedHost,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
}

/// Read model for project setup/listing surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSetupRecord {
    /// Project row data.
    pub project: ProjectRecord,
    /// Repository bound to the project.
    pub repository: ProjectRepositoryRecord,
    /// Whether the project is currently active for the owning account.
    pub is_active: bool,
    /// Count of specs in the project.
    pub spec_count: u64,
    /// Count of milestones in the project.
    pub milestone_count: u64,
    /// Count of initiatives in the project.
    pub initiative_count: u64,
}
