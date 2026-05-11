//! Database access layer for Tanren.
//!
//! This crate is the **only** place in the workspace that owns SQL and
//! row-shape entities. Other crates consume typed envelopes through the
//! [`AccountStore`] port and the concrete [`Store`] adapter; the
//! underlying `SeaORM` entity types are intentionally crate-private
//! (`entity/` is a private module) so that row shape changes never leak
//! across the dependency boundary.

pub(crate) mod accept_invitation;
pub(crate) mod account_queries;
pub(crate) mod approval_policy;
pub(crate) mod create_organization;
mod entity;
mod impl_account_store;
mod migration;
mod organization_constraints;
mod records;
mod traits;

pub use migration::Migrator;
pub use records::{
    AccountRecord, ApprovalPolicyRecord, InvitationRecord, MembershipRecord, NewAccount,
    NewInvitation, OrganizationCreateIdempotencyRecord, OrganizationPermissionGrantRecord,
    OrganizationRecord, SessionRecord,
};
pub use traits::{
    AcceptInvitationAtomicOutput, AcceptInvitationAtomicRequest, AcceptInvitationError,
    AcceptInvitationEventContext, AcceptInvitationEventsBuilder, AccountStore, ApprovalPolicyError,
    ApprovalPolicyPage, ConsumeInvitationError, ConsumedInvitation, CreateOrganizationAtomicOutput,
    CreateOrganizationAtomicRequest, CreateOrganizationError, CreateOrganizationEventContext,
    CreateOrganizationEventsBuilder, EventReference, LastOrganizationAdminGuardError,
    ListApprovalPoliciesRequest, ListOrganizationsPage, ListedOrganizationRecord,
};

use chrono::{DateTime, Utc};
pub(crate) use organization_constraints::{
    OrganizationCreateConstraint, classify_organization_create_constraint,
};
#[cfg(feature = "test-hooks")]
use sea_orm::{ActiveModelTrait, Set};
use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    IdempotencyKey, Identifier, InvitationToken, OrganizationName, OrganizationPermission,
    ValidationError,
};
use thiserror::Error;
use uuid::Uuid;

/// A connected handle to Tanren's canonical event store.
///
/// Construct via [`Store::connect`]; apply pending migrations via
/// [`Store::migrate`]. The handle is cheap to clone — under the hood
/// `SeaORM` pools connections.
///
/// All account-flow methods are exposed via the [`AccountStore`] trait
/// impl below; handlers depend on `&dyn AccountStore`, not on `Store`
/// directly.
pub struct Store {
    conn: DatabaseConnection,
}
impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}
impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}
/// A row in Tanren's canonical event log.
///
/// Per architecture, payloads are JSON-serialised typed events. F-0001 ships
/// only the envelope shape; concrete event types arrive with later behavior
/// slices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// UUID v7 — globally unique, time-ordered.
    pub id: Uuid,
    /// Wall-clock time the event was appended.
    pub occurred_at: DateTime<Utc>,
    /// Opaque JSON payload.
    pub payload: serde_json::Value,
}
impl Store {
    /// Connect to a database by URL (e.g. `postgres://...`).
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the underlying `SeaORM` connect call
    /// fails.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let conn = Database::connect(url).await?;
        Ok(Self { conn })
    }
    /// Reference to the underlying `SeaORM` connection. Provided so app-services
    /// can run cross-cutting transactions; row-shape entity types remain
    /// crate-private.
    #[must_use]
    pub fn connection(&self) -> &DatabaseConnection {
        &self.conn
    }
    /// Apply all pending migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if migration execution fails.
    pub async fn migrate(&self) -> Result<(), StoreError> {
        Migrator::up(&self.conn, None).await?;
        Ok(())
    }
}
impl From<entity::events::Model> for EventEnvelope {
    fn from(model: entity::events::Model) -> Self {
        Self {
            id: model.id,
            occurred_at: model.occurred_at,
            payload: model.payload,
        }
    }
}
/// Test-only fixture seeders. Gated behind the `test-hooks` Cargo feature
/// so production binaries cannot accidentally seed test data; the testkit
/// (and only the testkit) enables the feature.
#[cfg(feature = "test-hooks")]
impl Store {
    /// Seed a fixture invitation row directly. Bypasses the (currently
    /// non-existent) invitation-creation flow so BDD scenarios can stage
    /// pending invitations without an inviting handler.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_invitation(
        &self,
        new: NewInvitation,
    ) -> Result<InvitationRecord, StoreError> {
        let model = entity::invitations::ActiveModel {
            token: Set(new.token.as_str().to_owned()),
            inviting_org_id: Set(new.inviting_org_id.as_uuid()),
            expires_at: Set(new.expires_at),
            consumed_at: Set(None),
        };
        let inserted = model.insert(&self.conn).await?;
        InvitationRecord::try_from(inserted)
    }
}
/// Convert a DB-stored identifier string into an [`Identifier`]. Any
/// failure is a DB-invariant violation (we wrote the row through our
/// own validated path), so it surfaces as a distinct
/// [`StoreError::DataInvariant`] for triage rather than masquerading as
/// a query failure.
pub(crate) fn parse_db_identifier(raw: &str) -> Result<Identifier, StoreError> {
    Identifier::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "identifier",
        cause: err,
    })
}
/// Convert a DB-stored invitation token into an [`InvitationToken`].
pub(crate) fn parse_db_invitation_token(raw: &str) -> Result<InvitationToken, StoreError> {
    InvitationToken::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "invitation_token",
        cause: err,
    })
}
/// Convert a DB-stored organization-name key into an
/// [`OrganizationName`].
pub(crate) fn parse_db_organization_name(raw: &str) -> Result<OrganizationName, StoreError> {
    OrganizationName::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "organization_name",
        cause: err,
    })
}
/// Convert a DB-stored idempotency key into an [`IdempotencyKey`].
pub(crate) fn parse_db_idempotency_key(raw: &str) -> Result<IdempotencyKey, StoreError> {
    IdempotencyKey::parse(raw).map_err(|err| StoreError::DataInvariant {
        column: "idempotency_key",
        cause: err,
    })
}
/// Convert a DB-stored permission key into an
/// [`OrganizationPermission`].
pub(crate) fn parse_db_organization_permission(
    raw: &str,
) -> Result<OrganizationPermission, StoreError> {
    raw.parse::<OrganizationPermission>()
        .map_err(|_| StoreError::InvalidPermissionKey {
            column: "permission",
            value: raw.to_owned(),
        })
}
/// Wrap a raw string into a [`SecretString`]. Re-exported so callers
/// can build a [`SecretString`] without taking a direct `secrecy`
/// dependency.
#[must_use]
pub fn secret_from_string(value: String) -> SecretString {
    SecretString::from(value)
}
/// Errors raised by the store layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The underlying `SeaORM` call failed.
    #[error("database error: {0}")]
    Database(#[from] DbErr),
    /// A row read out of the database failed validation against a
    /// domain newtype's invariants. Indicates DB-side corruption — we
    /// only ever write rows through validated newtype constructors.
    #[error("data invariant violation in column `{column}`: {cause}")]
    DataInvariant {
        /// The column whose value failed to validate.
        column: &'static str,
        /// The underlying validation error.
        #[source]
        cause: ValidationError,
    },
    /// A row contained an unknown permission key value.
    #[error("unknown permission key in column `{column}`: {value}")]
    InvalidPermissionKey {
        /// Column that contained the unknown value.
        column: &'static str,
        /// Raw unknown key.
        value: String,
    },
}
