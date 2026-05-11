//! Event-envelope construction helpers.
//!
//! Extracted from `lib.rs` to stay under the per-file line budget.
//! Provides the full-envelope `EventEnvelope` struct, the `SeaORM` model
//! factory `event_active_model`, and the `From<Model>` conversion.

use chrono::{DateTime, Utc};
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity;

/// A row in Tanren's canonical event log.
///
/// Carries the full envelope the state architecture mandates: global
/// position, event type, schema version, actor, scope, resource,
/// correlation/causation/idempotency, and visibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// UUID v7 — durable event identity, time-ordered.
    pub id: Uuid,
    /// Global position, assigned by the database sequence
    /// `events_position_seq`. Authority for replay order, cursors,
    /// and subscriptions.
    pub position: i64,
    /// Wall-clock time the event was appended.
    pub occurred_at: DateTime<Utc>,
    /// Stable namespaced event type (e.g. "`provider_connection.initiated`").
    pub event_type: Option<String>,
    /// Payload schema version for evolution.
    pub schema_version: Option<String>,
    /// Actor that caused the change.
    pub actor: Option<String>,
    /// Scope (account, org, project, repository, or installation).
    pub scope: Option<String>,
    /// Primary resource identity affected by the event.
    pub resource_id: Option<Uuid>,
    /// Operation/workflow/request/trace grouping.
    pub correlation_id: Option<Uuid>,
    /// Command/event/job that caused this event.
    pub causation_id: Option<Uuid>,
    /// Command idempotency key.
    pub idempotency_key: Option<String>,
    /// Read restrictions and allowed audience metadata.
    pub visibility: Option<String>,
    /// Typed event-specific data — opaque JSON payload.
    pub payload: serde_json::Value,
}

/// Build an `events::ActiveModel` with full envelope metadata.
/// The `position` column is intentionally left unset so the Postgres
/// sequence default (`nextval('events_position_seq')`) assigns the
/// value. Callers that need the assigned position must read the row
/// back after insert.
pub(crate) fn event_active_model(
    id: Uuid,
    occurred_at: DateTime<Utc>,
    payload: serde_json::Value,
) -> entity::events::ActiveModel {
    entity::events::ActiveModel {
        id: Set(id),
        position: sea_orm::ActiveValue::NotSet,
        occurred_at: Set(occurred_at),
        event_type: Set(None),
        schema_version: Set(None),
        actor: Set(None),
        scope: Set(None),
        resource_id: Set(None),
        correlation_id: Set(None),
        causation_id: Set(None),
        idempotency_key: Set(None),
        visibility: Set(None),
        payload: Set(payload),
    }
}

impl From<entity::events::Model> for EventEnvelope {
    fn from(model: entity::events::Model) -> Self {
        Self {
            id: model.id,
            position: model.position,
            occurred_at: model.occurred_at,
            event_type: model.event_type,
            schema_version: model.schema_version,
            actor: model.actor,
            scope: model.scope,
            resource_id: model.resource_id,
            correlation_id: model.correlation_id,
            causation_id: model.causation_id,
            idempotency_key: model.idempotency_key,
            visibility: model.visibility,
            payload: model.payload,
        }
    }
}
