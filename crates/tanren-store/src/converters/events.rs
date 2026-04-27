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
    let spec_id = match &envelope.payload {
        DomainEvent::Methodology { event } => event.spec_id().map(tanren_domain::SpecId::into_uuid),
        _ => None,
    };

    Ok(events::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        event_id: Set(envelope.event_id.into_uuid()),
        timestamp: Set(envelope.timestamp),
        entity_kind: Set(entity_kind),
        entity_id: Set(entity_id),
        event_type: Set(event_type),
        spec_id: Set(spec_id),
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

    // Verify the reconstructed entity_ref matches the payload's own
    // root. If these disagree, the row was written with a misrouted
    // envelope — surface the inconsistency now rather than returning
    // a silently corrupt envelope.
    let expected = EventEnvelope::expected_entity_ref(&payload);
    if entity_ref != expected {
        return Err(StoreError::Conversion {
            context: "events::model_to_envelope",
            reason: format!(
                "stored entity_ref ({entity_ref}) disagrees with payload root ({expected})",
            ),
        });
    }

    Ok(EventEnvelope {
        schema_version,
        event_id: EventId::from_uuid(model.event_id),
        timestamp: model.timestamp,
        entity_ref,
        payload,
    })
}

/// Map a [`DispatchStatus`] to the expected `event_type` tag of its
/// companion lifecycle event. Returns an error for `Pending` because
/// that status is only set at creation time via
/// [`create_dispatch_projection`] — there is no valid lifecycle event
/// that transitions a dispatch *to* `Pending`.
pub(crate) fn dispatch_status_event_tag(
    status: tanren_domain::DispatchStatus,
) -> Result<&'static str, StoreError> {
    match status {
        tanren_domain::DispatchStatus::Running => Ok("dispatch_started"),
        tanren_domain::DispatchStatus::Completed => Ok("dispatch_completed"),
        tanren_domain::DispatchStatus::Failed => Ok("dispatch_failed"),
        tanren_domain::DispatchStatus::Cancelled => Ok("dispatch_cancelled"),
        tanren_domain::DispatchStatus::Pending => Err(StoreError::InvalidTransition {
            entity: "dispatch".to_owned(),
            from: "any".to_owned(),
            to: "pending".to_owned(),
        }),
    }
}

pub(crate) fn validate_routing(envelope: &EventEnvelope) -> Result<(), StoreError> {
    let canonical = envelope.payload.entity_root();
    if envelope.entity_ref != canonical {
        return Err(StoreError::Conversion {
            context: "envelope validation",
            reason: format!(
                "entity_ref mismatch: envelope={}, canonical={}",
                envelope.entity_ref, canonical,
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
