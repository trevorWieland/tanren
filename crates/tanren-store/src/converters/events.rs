//! `EventEnvelope` <-> `events::Model` converters.
//!
//! The `entity_kind`, `entity_id`, and `event_type` columns are
//! derived from the envelope's `entity_ref` and `payload` fields by
//! round-tripping through `serde_json::Value`. The serde
//! representation is authoritative — it is the same tag the rest of
//! the codebase uses when filtering events by type, so deriving it at
//! the converter boundary guarantees consistency with every query
//! filter the store accepts.

use sea_orm::ActiveValue::Set;
use serde_json::Value as JsonValue;
use tanren_domain::{DomainEvent, EntityRef, EventEnvelope, EventId};

use crate::entity::events;
use crate::errors::StoreError;

/// Build an [`events::ActiveModel`] from an envelope ready for
/// `insert`.
pub(crate) fn envelope_to_active_model(
    envelope: &EventEnvelope,
) -> Result<events::ActiveModel, StoreError> {
    let payload_value = serde_json::to_value(&envelope.payload)?;
    let event_type = extract_event_type(&payload_value)?;

    let entity_ref_value = serde_json::to_value(envelope.entity_ref)?;
    let entity_id = extract_entity_id(&entity_ref_value)?;
    let entity_kind = envelope.entity_ref.kind().to_string();

    let schema_version =
        i32::try_from(envelope.schema_version).map_err(|_| StoreError::Conversion {
            context: "events::envelope_to_active_model",
            reason: "schema_version exceeds i32::MAX".to_owned(),
        })?;

    Ok(events::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        event_id: Set(envelope.event_id.into_uuid()),
        timestamp: Set(envelope.timestamp),
        entity_kind: Set(entity_kind),
        entity_id: Set(entity_id),
        event_type: Set(event_type),
        schema_version: Set(schema_version),
        payload: Set(payload_value),
    })
}

/// Reconstruct an [`EventEnvelope`] from a row.
pub(crate) fn model_to_envelope(model: events::Model) -> Result<EventEnvelope, StoreError> {
    let payload: DomainEvent =
        serde_json::from_value(model.payload).map_err(|err| StoreError::Conversion {
            context: "events::model_to_envelope",
            reason: format!("payload deserialize failed: {err}"),
        })?;

    // Round-trip `(entity_kind, entity_id)` through the same serde
    // shape the domain `EntityRef` uses on the wire. Keeps the
    // reconstruction authoritative against the serde representation.
    let entity_ref_value = serde_json::json!({
        "type": model.entity_kind,
        "id": model.entity_id,
    });
    let entity_ref: EntityRef =
        serde_json::from_value(entity_ref_value).map_err(|err| StoreError::Conversion {
            context: "events::model_to_envelope",
            reason: format!("entity_ref reconstruct failed: {err}"),
        })?;

    let schema_version =
        u32::try_from(model.schema_version).map_err(|_| StoreError::Conversion {
            context: "events::model_to_envelope",
            reason: "schema_version is negative".to_owned(),
        })?;

    Ok(EventEnvelope {
        schema_version,
        event_id: EventId::from_uuid(model.event_id),
        timestamp: model.timestamp,
        entity_ref,
        payload,
    })
}

/// Validate that an envelope's `entity_ref` matches the expected value.
///
/// Called by every store method that accepts a caller-supplied
/// [`EventEnvelope`] before committing the transaction. This catches
/// misrouted envelopes early rather than persisting inconsistent
/// routing metadata.
pub(crate) fn validate_envelope_entity_ref(
    envelope: &EventEnvelope,
    expected: EntityRef,
) -> Result<(), StoreError> {
    if envelope.entity_ref != expected {
        return Err(StoreError::Conversion {
            context: "envelope validation",
            reason: format!(
                "entity_ref mismatch: envelope={}, expected={}",
                envelope.entity_ref, expected,
            ),
        });
    }
    Ok(())
}

fn extract_event_type(payload: &JsonValue) -> Result<String, StoreError> {
    payload
        .get("event_type")
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| StoreError::Conversion {
            context: "events::envelope_to_active_model",
            reason: "serialized payload missing `event_type` discriminant".to_owned(),
        })
}

fn extract_entity_id(entity_ref: &JsonValue) -> Result<String, StoreError> {
    entity_ref
        .get("id")
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| StoreError::Conversion {
            context: "events::envelope_to_active_model",
            reason: "serialized entity_ref missing `id` field".to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sea_orm::ActiveValue;
    use sea_orm::ActiveValue::{Set as ActiveSet, Unchanged as ActiveUnchanged};
    use tanren_domain::{
        ActorContext, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot, FiniteF64,
        GraphRevision, Lane, NonEmptyString, OrgId, Outcome, Phase, TimeoutSecs, UserId,
    };
    use uuid::Uuid;

    use super::*;

    fn snapshot() -> DispatchSnapshot {
        DispatchSnapshot {
            project: NonEmptyString::try_new("proj".to_owned()).expect("project"),
            phase: Phase::DoTask,
            cli: tanren_domain::Cli::Claude,
            auth_mode: tanren_domain::AuthMode::ApiKey,
            branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
            spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
            workflow_id: NonEmptyString::try_new("wf".to_owned()).expect("wf"),
            timeout: TimeoutSecs::try_new(60).expect("timeout"),
            environment_profile: NonEmptyString::try_new("prof".to_owned()).expect("profile"),
            gate_cmd: None,
            context: None,
            model: None,
            project_env: ConfigKeys::default(),
            required_secrets: vec![],
            preserve_on_failure: false,
            created_at: Utc::now(),
        }
    }

    fn envelope_dispatch_completed() -> EventEnvelope {
        let dispatch_id = DispatchId::new();
        EventEnvelope {
            schema_version: tanren_domain::SCHEMA_VERSION,
            event_id: EventId::from_uuid(Uuid::now_v7()),
            timestamp: Utc::now(),
            entity_ref: EntityRef::Dispatch(dispatch_id),
            payload: DomainEvent::DispatchCompleted {
                dispatch_id,
                outcome: Outcome::Success,
                total_duration_secs: FiniteF64::try_new(12.5).expect("finite"),
            },
        }
    }

    fn envelope_dispatch_created() -> EventEnvelope {
        let dispatch_id = DispatchId::new();
        EventEnvelope {
            schema_version: tanren_domain::SCHEMA_VERSION,
            event_id: EventId::from_uuid(Uuid::now_v7()),
            timestamp: Utc::now(),
            entity_ref: EntityRef::Dispatch(dispatch_id),
            payload: DomainEvent::DispatchCreated {
                dispatch_id,
                dispatch: Box::new(snapshot()),
                mode: DispatchMode::Manual,
                lane: Lane::Impl,
                actor: ActorContext {
                    org_id: OrgId::new(),
                    user_id: UserId::new(),
                    team_id: None,
                    api_key_id: None,
                    project_id: None,
                },
                graph_revision: GraphRevision::INITIAL,
            },
        }
    }

    fn unwrap_active<T>(value: ActiveValue<T>) -> T
    where
        T: Clone + Into<sea_orm::Value> + sea_orm::sea_query::Nullable,
    {
        match value {
            ActiveSet(v) | ActiveUnchanged(v) => v,
            ActiveValue::NotSet => unreachable!("active value not set in test"),
        }
    }

    fn active_to_model(active: events::ActiveModel) -> events::Model {
        events::Model {
            id: 1,
            event_id: unwrap_active(active.event_id),
            timestamp: unwrap_active(active.timestamp),
            entity_kind: unwrap_active(active.entity_kind),
            entity_id: unwrap_active(active.entity_id),
            event_type: unwrap_active(active.event_type),
            schema_version: unwrap_active(active.schema_version),
            payload: unwrap_active(active.payload),
        }
    }

    #[test]
    fn round_trip_dispatch_completed() {
        let original = envelope_dispatch_completed();
        let active = envelope_to_active_model(&original).expect("to active");
        let model = active_to_model(active);
        let back = model_to_envelope(model).expect("to envelope");
        assert_eq!(original, back);
    }

    #[test]
    fn round_trip_dispatch_created() {
        let original = envelope_dispatch_created();
        let active = envelope_to_active_model(&original).expect("to active");
        let model = active_to_model(active);
        let back = model_to_envelope(model).expect("to envelope");
        assert_eq!(original, back);
    }

    #[test]
    fn event_type_column_matches_serde_tag() {
        let original = envelope_dispatch_completed();
        let active = envelope_to_active_model(&original).expect("to active");
        let event_type = unwrap_active(active.event_type);
        assert_eq!(event_type, "dispatch_completed");
    }

    #[test]
    fn entity_kind_column_matches_entity_ref_kind() {
        let original = envelope_dispatch_completed();
        let active = envelope_to_active_model(&original).expect("to active");
        let entity_kind = unwrap_active(active.entity_kind);
        assert_eq!(entity_kind, "dispatch");
    }
}
