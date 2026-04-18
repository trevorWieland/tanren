//! `events` table — the canonical append-only event log.
//!
//! Every event ever emitted by the orchestrator lands here. The table
//! is append-only; rows are never updated or deleted. All projection
//! tables are derived from these rows and could in principle be
//! rebuilt by replaying them.
//!
//! The `payload` column uses `SeaORM`'s `JsonBinary` column type, which
//! emits `TEXT` on `SQLite` and `JSONB` on `Postgres` — letting a single
//! column definition serve both backends. Lane 0.2's
//! `event_value_roundtrip` test certified that every `DomainEvent`
//! variant survives the `serde_json::Value` path that `SeaORM` uses for
//! JSON columns, so this column is safe to read and write without
//! string-based intermediate conversions.

use sea_orm::entity::prelude::*;

/// Row shape of the `events` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    /// Auto-incrementing surrogate key — used only by the store for
    /// stable ordering inside a single server; never exposed on the
    /// wire. Domain code keys by [`event_id`](Self::event_id).
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Domain-level event identifier (UUID v7). Globally unique.
    #[sea_orm(unique, indexed)]
    pub event_id: Uuid,

    /// Wall-clock timestamp at which the event was emitted.
    pub timestamp: DateTimeUtc,

    /// Snake-case discriminant of [`EntityKind`](tanren_domain::EntityKind).
    pub entity_kind: String,

    /// String form of the routing root's id. For entity kinds backed
    /// by a UUID newtype this is the UUID's hyphenated string. We use
    /// `String` rather than `Uuid` so future entity kinds with
    /// non-UUID identifiers do not require a schema change.
    pub entity_id: String,

    /// Snake-case tag of the [`DomainEvent`](tanren_domain::DomainEvent)
    /// variant carried in [`payload`](Self::payload).
    pub event_type: String,

    /// Denormalized spec id for methodology-scoped query paths.
    /// Populated only for methodology events carrying a spec id.
    pub spec_id: Option<Uuid>,

    /// Envelope schema version. Matches
    /// [`tanren_domain::SCHEMA_VERSION`] at insertion time.
    pub schema_version: i32,

    /// Serialized [`DomainEvent`](tanren_domain::DomainEvent) payload.
    /// Other envelope fields (`event_id`, `timestamp`, `entity_ref`,
    /// `schema_version`) are stored as dedicated columns so filter
    /// queries don't need JSON path operators.
    #[sea_orm(column_type = "JsonBinary")]
    pub payload: Json,
}

/// No relations — projections are derived out-of-band, not via JOINs.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
