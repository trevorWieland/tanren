//! Database access layer for Tanren.
//!
//! This crate is the **only** place in the workspace that owns SQL and
//! row-shape entities. Other crates consume typed envelopes through the
//! [`Store`] handle; the underlying `SeaORM` entity types are intentionally
//! crate-private (`entity/` is a private module) so that row shape changes
//! never leak across the dependency boundary.

mod entity;
mod migration;
mod records;

pub use migration::Migrator;
pub use records::{
    AccountRecord, InvitationRecord, MembershipRecord, NewAccount, NewInvitation, SessionRecord,
};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use sea_orm_migration::MigratorTrait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, MembershipId, OrgId, SessionToken,
    ValidationError,
};
use thiserror::Error;
use uuid::Uuid;

/// A connected handle to Tanren's canonical event store.
///
/// Construct via [`Store::connect`]; apply pending migrations via
/// [`Store::migrate`]; append events via [`Store::append_event`]; read recent
/// events via [`Store::recent_events`]. The handle is cheap to clone — under
/// the hood `SeaORM` pools connections.
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

    /// Append a payload to the canonical event log at the supplied
    /// instant. Caller threads `clock.now()` in — the store does not
    /// read time directly.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn append_event(
        &self,
        payload: serde_json::Value,
        occurred_at: DateTime<Utc>,
    ) -> Result<EventEnvelope, StoreError> {
        let envelope = EventEnvelope {
            id: Uuid::now_v7(),
            occurred_at,
            payload,
        };
        let model = entity::events::ActiveModel {
            id: Set(envelope.id),
            occurred_at: Set(envelope.occurred_at),
            payload: Set(envelope.payload.clone()),
        };
        model.insert(&self.conn).await?;
        Ok(envelope)
    }

    /// Read the most recent `limit` events, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the query fails.
    pub async fn recent_events(&self, limit: u64) -> Result<Vec<EventEnvelope>, StoreError> {
        // Order by `occurred_at` first, then by `id` (UUIDv7) as a stable
        // tie-breaker. Without the secondary key, events landing inside the
        // same timestamp bucket can come back in different orders across
        // reads — replay correctness demands a total order.
        let rows = entity::events::Entity::find()
            .order_by_desc(entity::events::Column::OccurredAt)
            .order_by_desc(entity::events::Column::Id)
            .limit(limit)
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(EventEnvelope::from).collect())
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

impl Store {
    /// Insert a new account row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails — including
    /// the unique-index violation that fires on duplicate `identifier`.
    pub async fn insert_account(&self, new: NewAccount) -> Result<AccountRecord, StoreError> {
        let model = entity::accounts::ActiveModel {
            id: Set(new.id.as_uuid()),
            identifier: Set(new.identifier.as_str().to_owned()),
            display_name: Set(new.display_name),
            password_hash: Set(new.password_hash),
            password_salt: Set(new.password_salt),
            created_at: Set(new.created_at),
            org_id: Set(new.org_id.map(OrgId::as_uuid)),
        };
        let inserted = model.insert(&self.conn).await?;
        AccountRecord::try_from(inserted)
    }

    /// Look up an account by its case-sensitive identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the query fails.
    pub async fn find_account_by_identifier(
        &self,
        identifier: &Identifier,
    ) -> Result<Option<AccountRecord>, StoreError> {
        let row = entity::accounts::Entity::find()
            .filter(entity::accounts::Column::Identifier.eq(identifier.as_str()))
            .one(&self.conn)
            .await?;
        row.map(AccountRecord::try_from).transpose()
    }

    /// Look up an account by an [`Email`]. R-0001 derives identifier
    /// from the canonical email; this is a thin alias kept around so
    /// the email-driven sign-in path reads naturally.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the query fails.
    pub async fn find_account_by_email(
        &self,
        email: &Email,
    ) -> Result<Option<AccountRecord>, StoreError> {
        let identifier = Identifier::from_email(email);
        self.find_account_by_identifier(&identifier).await
    }

    /// Insert a membership linking an account to an organization.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn insert_membership(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        created_at: DateTime<Utc>,
    ) -> Result<MembershipId, StoreError> {
        let id = MembershipId::fresh();
        let model = entity::memberships::ActiveModel {
            id: Set(id.as_uuid()),
            account_id: Set(account_id.as_uuid()),
            org_id: Set(org_id.as_uuid()),
            created_at: Set(created_at),
        };
        model.insert(&self.conn).await?;
        Ok(id)
    }

    /// Seed a fixture invitation. PR 4 will gate this behind the
    /// `test-hooks` feature.
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

    /// Look up an invitation by token.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the query fails.
    pub async fn find_invitation_by_token(
        &self,
        token: &InvitationToken,
    ) -> Result<Option<InvitationRecord>, StoreError> {
        let row = entity::invitations::Entity::find_by_id(token.as_str().to_owned())
            .one(&self.conn)
            .await?;
        row.map(InvitationRecord::try_from).transpose()
    }

    /// Mark an invitation consumed at the supplied instant.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the update fails.
    pub async fn mark_invitation_consumed(
        &self,
        token: &InvitationToken,
        consumed_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let row = entity::invitations::Entity::find_by_id(token.as_str().to_owned())
            .one(&self.conn)
            .await?;
        if let Some(row) = row {
            let mut active: entity::invitations::ActiveModel = row.into();
            active.consumed_at = Set(Some(consumed_at));
            active.update(&self.conn).await?;
        }
        Ok(())
    }

    /// Issue a session for the supplied account. The DB column for
    /// `expires_at` lands in PR 4; the value is currently held only in
    /// the returned [`SessionRecord`].
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn insert_session(
        &self,
        token: SessionToken,
        account_id: AccountId,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<SessionRecord, StoreError> {
        let model = entity::account_sessions::ActiveModel {
            token: Set(token.expose_secret().to_owned()),
            account_id: Set(account_id.as_uuid()),
            created_at: Set(created_at),
        };
        model.insert(&self.conn).await?;
        Ok(SessionRecord {
            token,
            account_id,
            created_at,
            expires_at,
        })
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

/// Wrap a raw secret string into a [`SecretString`]. Re-exported so
/// `tanren-app-services` (and tests) can build a [`SecretString`]
/// without taking a direct `secrecy` dependency.
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
}
