//! Database access layer for Tanren.
//!
//! This crate is the **only** place in the workspace that owns SQL and
//! row-shape entities. Other crates consume typed envelopes through the
//! [`Store`] handle; the underlying `SeaORM` entity types are intentionally
//! crate-private (`entity/` is a private module) so that row shape changes
//! never leak across the dependency boundary.

mod entity;
mod migration;

pub use migration::Migrator;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use sea_orm_migration::MigratorTrait;
use serde::{Deserialize, Serialize};
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

    /// Append a payload to the canonical event log.
    ///
    /// The event id is allocated as UUID v7 and the timestamp is taken from
    /// `chrono::Utc::now()`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn append_event(
        &self,
        payload: serde_json::Value,
    ) -> Result<EventEnvelope, StoreError> {
        let envelope = EventEnvelope {
            id: Uuid::now_v7(),
            occurred_at: Utc::now(),
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
        // Order by `occurred_at` first for human-meaningful recency, then by
        // `id` (UUIDv7) as a stable tie-breaker. Without the secondary key,
        // events landing inside the same timestamp bucket can come back in
        // different orders across reads — replay and projection correctness
        // demands a total order.
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

/// Persisted account row, exposed as a typed envelope so other crates
/// never see `SeaORM` `Model` types directly. R-0001 stores password
/// hash + salt as opaque bytes so the hashing scheme is swappable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecord {
    /// Stable account id.
    pub id: Uuid,
    /// User-facing identifier (email).
    pub identifier: String,
    /// Display name.
    pub display_name: String,
    /// Opaque password hash bytes.
    pub password_hash: Vec<u8>,
    /// Salt that produced the password hash.
    pub password_salt: Vec<u8>,
    /// Wall-clock time the account was created.
    pub created_at: DateTime<Utc>,
    /// Owning organization — `None` for personal (self-signup) accounts.
    pub org_id: Option<Uuid>,
}

impl From<entity::accounts::Model> for AccountRecord {
    fn from(model: entity::accounts::Model) -> Self {
        Self {
            id: model.id,
            identifier: model.identifier,
            display_name: model.display_name,
            password_hash: model.password_hash,
            password_salt: model.password_salt,
            created_at: model.created_at,
            org_id: model.org_id,
        }
    }
}

/// Persisted invitation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvitationRecord {
    /// Opaque invitation token (PK).
    pub token: String,
    /// Organization the new account joins on acceptance.
    pub inviting_org_id: Uuid,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
    /// Set when the invitation has been accepted (or revoked).
    pub consumed_at: Option<DateTime<Utc>>,
}

impl From<entity::invitations::Model> for InvitationRecord {
    fn from(model: entity::invitations::Model) -> Self {
        Self {
            token: model.token,
            inviting_org_id: model.inviting_org_id,
            expires_at: model.expires_at,
            consumed_at: model.consumed_at,
        }
    }
}

/// Persisted membership row — links an account to an organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipRecord {
    /// Stable membership id.
    pub id: Uuid,
    /// Account this membership belongs to.
    pub account_id: Uuid,
    /// Organization the account is a member of.
    pub org_id: Uuid,
    /// Wall-clock time the membership was created.
    pub created_at: DateTime<Utc>,
}

impl From<entity::memberships::Model> for MembershipRecord {
    fn from(model: entity::memberships::Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            org_id: model.org_id,
            created_at: model.created_at,
        }
    }
}

/// Persisted session row — issued by `tanren-app-services` on
/// successful sign-up / sign-in / invitation acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Opaque session token (PK).
    pub token: String,
    /// Account this session belongs to.
    pub account_id: Uuid,
    /// Wall-clock time the session was issued.
    pub created_at: DateTime<Utc>,
}

impl From<entity::account_sessions::Model> for SessionRecord {
    fn from(model: entity::account_sessions::Model) -> Self {
        Self {
            token: model.token,
            account_id: model.account_id,
            created_at: model.created_at,
        }
    }
}

/// Input shape for [`Store::insert_account`].
#[derive(Debug, Clone)]
pub struct NewAccount {
    /// Stable id allocated by the caller (`UUIDv7`).
    pub id: Uuid,
    /// User-facing identifier (email).
    pub identifier: String,
    /// Display name.
    pub display_name: String,
    /// Opaque password hash bytes.
    pub password_hash: Vec<u8>,
    /// Salt that produced the password hash.
    pub password_salt: Vec<u8>,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
    /// Owning organization — `None` for personal (self-signup) accounts.
    pub org_id: Option<Uuid>,
}

/// Input shape for [`Store::seed_invitation`].
#[derive(Debug, Clone)]
pub struct NewInvitation {
    /// Opaque token shared with the invitee out of band.
    pub token: String,
    /// Organization the new account joins on acceptance.
    pub inviting_org_id: Uuid,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
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
            id: Set(new.id),
            identifier: Set(new.identifier),
            display_name: Set(new.display_name),
            password_hash: Set(new.password_hash),
            password_salt: Set(new.password_salt),
            created_at: Set(new.created_at),
            org_id: Set(new.org_id),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(inserted.into())
    }

    /// Look up an account by its case-sensitive identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the query fails.
    pub async fn find_account_by_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<AccountRecord>, StoreError> {
        let row = entity::accounts::Entity::find()
            .filter(entity::accounts::Column::Identifier.eq(identifier))
            .one(&self.conn)
            .await?;
        Ok(row.map(AccountRecord::from))
    }

    /// Insert a membership linking an account to an organization.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails (including
    /// the unique-(account,org) constraint).
    pub async fn insert_membership(
        &self,
        account_id: Uuid,
        org_id: Uuid,
    ) -> Result<(), StoreError> {
        let model = entity::memberships::ActiveModel {
            id: Set(Uuid::now_v7()),
            account_id: Set(account_id),
            org_id: Set(org_id),
            created_at: Set(Utc::now()),
        };
        model.insert(&self.conn).await?;
        Ok(())
    }

    /// Seed a fixture invitation. Real invitations are minted by R-0005's
    /// invite flow; R-0001 only models acceptance, so BDD seeds the row
    /// directly. Documented as test-only — production code should not
    /// call this.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn seed_invitation(
        &self,
        new: NewInvitation,
    ) -> Result<InvitationRecord, StoreError> {
        let model = entity::invitations::ActiveModel {
            token: Set(new.token),
            inviting_org_id: Set(new.inviting_org_id),
            expires_at: Set(new.expires_at),
            consumed_at: Set(None),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(inserted.into())
    }

    /// Look up an invitation by token.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the query fails.
    pub async fn find_invitation_by_token(
        &self,
        token: &str,
    ) -> Result<Option<InvitationRecord>, StoreError> {
        let row = entity::invitations::Entity::find_by_id(token.to_owned())
            .one(&self.conn)
            .await?;
        Ok(row.map(InvitationRecord::from))
    }

    /// Mark an invitation consumed at the supplied instant.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the update fails.
    pub async fn mark_invitation_consumed(
        &self,
        token: &str,
        consumed_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let row = entity::invitations::Entity::find_by_id(token.to_owned())
            .one(&self.conn)
            .await?;
        if let Some(row) = row {
            let mut active: entity::invitations::ActiveModel = row.into();
            active.consumed_at = Set(Some(consumed_at));
            active.update(&self.conn).await?;
        }
        Ok(())
    }

    /// Issue a session for the supplied account.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the insert fails.
    pub async fn insert_session(
        &self,
        token: String,
        account_id: Uuid,
    ) -> Result<SessionRecord, StoreError> {
        let model = entity::account_sessions::ActiveModel {
            token: Set(token),
            account_id: Set(account_id),
            created_at: Set(Utc::now()),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(inserted.into())
    }
}

/// Errors raised by the store layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The underlying `SeaORM` call failed.
    #[error("database error: {0}")]
    Database(#[from] DbErr),
}
