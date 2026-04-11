//! Tests for the version-aware envelope decode path.

use chrono::{TimeZone, Utc};
use tanren_domain::entity::EntityRef;
use tanren_domain::events::{
    DomainEvent, EnvelopeDecodeError, EventEnvelope, RawEventEnvelope, SCHEMA_VERSION,
};
use tanren_domain::ids::{DispatchId, EventId};

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0)
        .single()
        .expect("valid timestamp")
}

fn fixed_uuid() -> uuid::Uuid {
    uuid::Uuid::parse_str("01966a00-0000-7000-8000-000000000001").expect("valid uuid")
}

fn did() -> DispatchId {
    DispatchId::from_uuid(fixed_uuid())
}

#[test]
fn envelope_carries_current_schema_version() {
    let envelope = EventEnvelope::new(
        EventId::from_uuid(fixed_uuid()),
        ts(),
        DomainEvent::DispatchStarted { dispatch_id: did() },
    );
    assert_eq!(envelope.schema_version, SCHEMA_VERSION);
    let json = serde_json::to_string(&envelope).expect("serialize");
    assert!(json.contains("\"schema_version\":1"));
}

#[test]
fn envelope_new_derives_entity_ref_from_payload() {
    // Regression: the only supported constructor derives entity_ref
    // from the payload, so the two can never disagree.
    let envelope = EventEnvelope::new(
        EventId::from_uuid(fixed_uuid()),
        ts(),
        DomainEvent::DispatchStarted { dispatch_id: did() },
    );
    assert_eq!(envelope.entity_ref, EntityRef::Dispatch(did()));
}

#[test]
fn envelope_uses_timestamp_as_single_source_of_truth() {
    // Regression: timestamps live on the envelope only. Payload variants
    // no longer carry their own timestamp field, so there is no way for
    // envelope.timestamp and payload.timestamp to disagree.
    let envelope = EventEnvelope::new(
        EventId::from_uuid(fixed_uuid()),
        ts(),
        DomainEvent::DispatchStarted { dispatch_id: did() },
    );
    let json = serde_json::to_string(&envelope).expect("serialize");
    // `timestamp` appears exactly once in the wire form — on the envelope.
    let occurrences = json.matches("\"timestamp\"").count();
    assert_eq!(occurrences, 1, "expected one timestamp, got {json}");
    assert_eq!(envelope.timestamp, ts());
}

#[test]
fn envelope_deserializes_legacy_without_schema_version() {
    // Legacy records written before the schema_version field existed
    // must still decode (the default fills in the current version).
    let legacy = r#"{
        "event_id": "01966a00-0000-7000-8000-000000000001",
        "timestamp": "2025-06-15T12:00:00Z",
        "entity_ref": {"type": "dispatch", "id": "01966a00-0000-7000-8000-000000000001"},
        "payload": {
            "event_type": "dispatch_started",
            "dispatch_id": "01966a00-0000-7000-8000-000000000001"
        }
    }"#;
    let envelope: EventEnvelope = serde_json::from_str(legacy).expect("deserialize legacy");
    assert_eq!(envelope.schema_version, SCHEMA_VERSION);
}

#[test]
fn raw_envelope_decode_succeeds_for_valid_payload() {
    let raw_json = r#"{
        "schema_version": 1,
        "event_id": "01966a00-0000-7000-8000-000000000001",
        "timestamp": "2025-06-15T12:00:00Z",
        "entity_ref": {"type": "dispatch", "id": "01966a00-0000-7000-8000-000000000001"},
        "payload": {
            "event_type": "dispatch_started",
            "dispatch_id": "01966a00-0000-7000-8000-000000000001"
        }
    }"#;
    let raw: RawEventEnvelope = serde_json::from_str(raw_json).expect("raw decode");
    assert_eq!(raw.schema_version, 1);
    let envelope = raw.try_decode().expect("typed decode");
    assert!(matches!(
        envelope.payload,
        DomainEvent::DispatchStarted { .. }
    ));
}

#[test]
fn raw_envelope_decode_rejects_future_version() {
    let future_json = format!(
        r#"{{
            "schema_version": {future},
            "event_id": "01966a00-0000-7000-8000-000000000001",
            "timestamp": "2025-06-15T12:00:00Z",
            "entity_ref": {{"type": "dispatch", "id": "01966a00-0000-7000-8000-000000000001"}},
            "payload": {{
                "event_type": "dispatch_started",
                "dispatch_id": "01966a00-0000-7000-8000-000000000001"
            }}
        }}"#,
        future = SCHEMA_VERSION + 1
    );
    let raw: RawEventEnvelope = serde_json::from_str(&future_json).expect("raw decode");
    let err = raw.try_decode().expect_err("should reject future version");
    assert!(
        matches!(
            &err,
            EnvelopeDecodeError::UnsupportedVersion { version, current }
                if *version == SCHEMA_VERSION + 1 && *current == SCHEMA_VERSION
        ),
        "expected UnsupportedVersion, got {err:?}"
    );
}

#[test]
fn raw_envelope_decode_preserves_unknown_variant_payload() {
    // A future event variant that this version of the library does
    // not know about. Raw decode succeeds (schema version is current)
    // but typed decode fails with UnknownEvent — the raw payload is
    // preserved so the caller can log, park, or replay the event.
    let unknown_json = r#"{
        "schema_version": 1,
        "event_id": "01966a00-0000-7000-8000-000000000001",
        "timestamp": "2025-06-15T12:00:00Z",
        "entity_ref": {"type": "dispatch", "id": "01966a00-0000-7000-8000-000000000001"},
        "payload": {
            "event_type": "lease_hibernated",
            "lease_id": "01966a00-0000-7000-8000-000000000001",
            "dispatch_id": "01966a00-0000-7000-8000-000000000001"
        }
    }"#;
    let raw: RawEventEnvelope = serde_json::from_str(unknown_json).expect("raw decode");
    let raw_payload_for_inspection = raw.payload.clone();
    let err = raw.try_decode().expect_err("should reject unknown variant");
    assert!(
        matches!(
            &err,
            EnvelopeDecodeError::UnknownEvent { payload, message }
                if *payload == raw_payload_for_inspection && !message.is_empty()
        ),
        "expected UnknownEvent, got {err:?}"
    );
}

#[test]
fn raw_envelope_carries_schema_version_for_inspection() {
    let raw_json = r#"{
        "schema_version": 1,
        "event_id": "01966a00-0000-7000-8000-000000000001",
        "timestamp": "2025-06-15T12:00:00Z",
        "entity_ref": {"type": "dispatch", "id": "01966a00-0000-7000-8000-000000000001"},
        "payload": {"event_type": "unknown"}
    }"#;
    let raw: RawEventEnvelope = serde_json::from_str(raw_json).expect("raw decode");
    // Consumer can inspect the version and decide what to do before
    // attempting a typed decode.
    assert_eq!(raw.schema_version, 1);
    assert!(raw.try_decode().is_err());
}

#[test]
fn raw_envelope_decode_rejects_entity_ref_payload_mismatch() {
    // entity_ref points at a different dispatch than the payload's
    // dispatch_id. This is a self-contradictory record and must be
    // rejected so routing code and projections cannot disagree.
    let mismatched_json = r#"{
        "schema_version": 1,
        "event_id": "01966a00-0000-7000-8000-000000000001",
        "timestamp": "2025-06-15T12:00:00Z",
        "entity_ref": {"type": "dispatch", "id": "019670ff-ffff-7fff-8fff-ffffffffffff"},
        "payload": {
            "event_type": "dispatch_started",
            "dispatch_id": "01966a00-0000-7000-8000-000000000001"
        }
    }"#;
    let raw: RawEventEnvelope = serde_json::from_str(mismatched_json).expect("raw decode");
    let err = raw.try_decode().expect_err("should reject mismatch");
    assert!(
        matches!(&err, EnvelopeDecodeError::EntityMismatch { .. }),
        "expected EntityMismatch, got {err:?}"
    );
}

#[test]
fn raw_envelope_decode_rejects_wrong_entity_kind() {
    // entity_ref is a Lease, but the payload is a dispatch-scoped event.
    // The root must be an EntityRef::Dispatch per current rules.
    let wrong_kind_json = r#"{
        "schema_version": 1,
        "event_id": "01966a00-0000-7000-8000-000000000001",
        "timestamp": "2025-06-15T12:00:00Z",
        "entity_ref": {"type": "lease", "id": "01966a00-0000-7000-8000-000000000001"},
        "payload": {
            "event_type": "dispatch_started",
            "dispatch_id": "01966a00-0000-7000-8000-000000000001"
        }
    }"#;
    let raw: RawEventEnvelope = serde_json::from_str(wrong_kind_json).expect("raw decode");
    let err = raw.try_decode().expect_err("should reject wrong kind");
    assert!(
        matches!(&err, EnvelopeDecodeError::EntityMismatch { .. }),
        "expected EntityMismatch, got {err:?}"
    );
}
