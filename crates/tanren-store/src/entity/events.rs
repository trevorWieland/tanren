//! `SeaORM` entity for the canonical `events` table.
//!
//! The events table is Tanren's append-only durable mutation log. Every
//! durable state transition writes a row through this entity. The
//! envelope columns (`event_type`, `schema_version`, `actor`, `scope`,
//! `resource_id`, `correlation_id`, `causation_id`, `idempotency_key`, `visibility`)
//! carry the metadata the state architecture mandates for replay, audit,
//! visibility, and subscription cursors. These columns are nullable so
//! existing rows remain valid; new writes populate every field. The
//! `position` column is NOT NULL — the database assigns a globally
//! monotonic value on every insert via the `events_position_seq` sequence.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Global position — assigned by the database, used for total
    /// ordering, replay cursors, and subscriptions.
    pub position: i64,
    /// Wall-clock time the event was appended.
    pub occurred_at: DateTimeUtc,
    /// Stable namespaced event type (e.g.
    /// "`provider_connection.initiated`").
    pub event_type: Option<String>,
    /// Event payload schema version for evolution.
    pub schema_version: Option<String>,
    /// Actor that caused the change — user, service account, worker,
    /// provider, or system.
    pub actor: Option<String>,
    /// Account, organization, project, repository, or installation
    /// scope.
    pub scope: Option<String>,
    /// Primary resource identity affected by the event.
    pub resource_id: Option<Uuid>,
    /// Operation / workflow / request / trace grouping.
    pub correlation_id: Option<Uuid>,
    /// Command / event / job that caused this event.
    pub causation_id: Option<Uuid>,
    /// Command idempotency key.
    pub idempotency_key: Option<String>,
    /// Read restrictions, redaction class, and allowed audience.
    pub visibility: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub payload: Json,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
