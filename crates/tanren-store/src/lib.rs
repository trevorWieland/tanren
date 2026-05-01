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
    ActiveModelTrait, Database, DatabaseConnection, DbErr, EntityTrait, QueryOrder, QuerySelect,
    Set,
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
        let rows = entity::events::Entity::find()
            .order_by_desc(entity::events::Column::OccurredAt)
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

/// Errors raised by the store layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The underlying `SeaORM` call failed.
    #[error("database error: {0}")]
    Database(#[from] DbErr),
}
