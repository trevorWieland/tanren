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
    AccountId, Identifier, InvitationToken, MembershipId, OrgId, ProjectId, SessionToken, SpecId,
};
#[cfg(feature = "test-hooks")]
use uuid::Uuid;

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

/// Input shape for [`crate::ProjectStore::insert_project`] and
/// [`crate::Store::seed_project`]. Bundles every field needed to
/// persist a new project row so the trait and seeder stay under
/// clippy's too-many-arguments threshold.
#[derive(Debug, Clone)]
pub struct NewProject {
    /// Stable project id (allocated by the caller, `UUIDv7`).
    pub id: ProjectId,
    /// Owning organization.
    pub org_id: OrgId,
    /// Human-readable name.
    pub name: String,
    /// Source-control provider connection the project is connected through.
    pub provider_connection_id: tanren_identity_policy::ProviderConnectionId,
    /// Opaque repository resource identifier within the provider connection.
    pub resource_id: String,
    /// Redacted display reference (never contains credentials).
    pub display_ref: String,
    /// Wall-clock time the project was connected.
    pub connected_at: DateTime<Utc>,
}

/// Connection status of a project. `Disconnected` carries the wall-clock
/// instant the disconnect was applied so callers can render it without
/// needing a separate field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    /// Project is connected and active.
    Connected,
    /// Project was disconnected at the given instant. The underlying
    /// repository is untouched — only the Tanren link is severed.
    Disconnected(DateTime<Utc>),
}

/// Persisted project row. Disconnected projects are retained alongside
/// their specs so that reconnection (B-0025) can restore access to the
/// prior spec history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    /// Stable project id.
    pub id: ProjectId,
    /// Owning organization.
    pub org_id: OrgId,
    /// Human-readable name.
    pub name: String,
    /// Source-control provider connection the project is connected through.
    pub provider_connection_id: tanren_identity_policy::ProviderConnectionId,
    /// Opaque repository resource identifier within the provider connection.
    pub resource_id: String,
    /// Redacted display reference (never contains credentials).
    pub display_ref: String,
    /// Wall-clock time the project was originally connected.
    pub connected_at: DateTime<Utc>,
    /// Current connection status.
    pub status: ProjectStatus,
}

impl From<entity::projects::Model> for ProjectRecord {
    fn from(model: entity::projects::Model) -> Self {
        Self {
            id: ProjectId::new(model.id),
            org_id: OrgId::new(model.org_id),
            name: model.name,
            provider_connection_id: tanren_identity_policy::ProviderConnectionId::new(
                model.provider_connection_id,
            ),
            resource_id: model.resource_id,
            display_ref: model.display_ref,
            connected_at: model.connected_at,
            status: match model.disconnected_at {
                Some(ts) => ProjectStatus::Disconnected(ts),
                None => ProjectStatus::Connected,
            },
        }
    }
}

/// Persisted spec row attached to a project. Specs are retained when a
/// project is disconnected so the reconnection path (B-0025) can restore
/// the full spec history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSpecRecord {
    /// Stable spec id.
    pub id: SpecId,
    /// Owning project.
    pub project_id: ProjectId,
    /// Human-readable title.
    pub title: String,
    /// Spec body text.
    pub body: String,
    /// Wall-clock time the spec was created.
    pub created_at: DateTime<Utc>,
}

impl From<entity::project_specs::Model> for ProjectSpecRecord {
    fn from(model: entity::project_specs::Model) -> Self {
        Self {
            id: SpecId::new(model.id),
            project_id: ProjectId::new(model.project_id),
            title: model.title,
            body: model.body,
            created_at: model.created_at,
        }
    }
}

/// Cross-project dependency link. When the target project is disconnected
/// or unknown, the dependency surfaces as an unresolved-link signal
/// (M-0007 owns the lookup; the wire shape is
/// `ProjectDependencyResponse` in the `tanren-contract` crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDependencyRecord {
    /// Project that owns the dependency reference.
    pub source_project_id: ProjectId,
    /// Spec within the source project carrying the reference.
    pub source_spec_id: SpecId,
    /// Target project that may be disconnected or unknown.
    pub target_project_id: ProjectId,
    /// Wall-clock time the dependency was detected.
    pub detected_at: DateTime<Utc>,
}

impl From<entity::project_dependencies::Model> for ProjectDependencyRecord {
    fn from(model: entity::project_dependencies::Model) -> Self {
        Self {
            source_project_id: ProjectId::new(model.source_project_id),
            source_spec_id: SpecId::new(model.source_spec_id),
            target_project_id: ProjectId::new(model.target_project_id),
            detected_at: model.detected_at,
        }
    }
}

/// M-0003 fixture seam for the no-active-loops precondition. M-0011 owns
/// real implementation loops; this record is replaced when that module
/// lands. The fixture exists so disconnect can enforce the precondition
/// without depending on M-0011's table shape.
///
/// Only available when the `test-hooks` feature is enabled. Production
/// code uses the [`crate::traits::ActiveLoopRead`] port instead.
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectLoopFixtureRecord {
    pub id: Uuid,
    pub project_id: ProjectId,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[cfg(feature = "test-hooks")]
impl From<entity::project_loop_fixtures::Model> for ProjectLoopFixtureRecord {
    fn from(model: entity::project_loop_fixtures::Model) -> Self {
        Self {
            id: model.id,
            project_id: ProjectId::new(model.project_id),
            is_active: model.is_active,
            created_at: model.created_at,
        }
    }
}
