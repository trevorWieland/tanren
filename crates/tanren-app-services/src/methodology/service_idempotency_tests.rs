use std::sync::Arc;

use chrono::{Duration, Utc};
use tanren_contract::methodology::SchemaVersion;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::events::{FindingAdded, MethodologyEvent};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::{EntityKind, FindingId, NonEmptyString, SpecId};
use tanren_store::EventFilter;
use tanren_store::{EventStore, Store};

use super::MethodologyError;
use crate::methodology::service::{MethodologyService, PhaseEventsRuntime};

async fn mk_service() -> (Arc<Store>, MethodologyService, SpecId) {
    let store = Arc::new(
        Store::open_and_migrate("sqlite::memory:?cache=shared")
            .await
            .expect("open"),
    );
    let spec_id = SpecId::new();
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-methodology-idempotency-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "test-session".into(),
    };
    let service = MethodologyService::with_runtime(store.clone(), vec![], Some(runtime), vec![]);
    (store, service, spec_id)
}

fn idempotency_payload() -> serde_json::Value {
    serde_json::json!({
        "schema_version": SchemaVersion::current(),
        "kind": "partial-failure-check"
    })
}

async fn run_partial_failure(
    service: &MethodologyService,
    phase: &PhaseId,
    spec_id: SpecId,
    key: Option<String>,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, MethodologyError> {
    service
        .run_idempotent_mutation(
            "test_partial_failure",
            spec_id,
            key.clone(),
            payload,
            || async {
                let finding = Finding {
                    id: FindingId::new(),
                    spec_id,
                    severity: FindingSeverity::FixNow,
                    title: NonEmptyString::try_new("partial emit").expect("title"),
                    description: String::new(),
                    affected_files: vec!["src/lib.rs".into()],
                    line_numbers: vec![],
                    source: FindingSource::Audit {
                        phase: PhaseId::try_new("audit-task").expect("phase"),
                        pillar: None,
                    },
                    attached_task: None,
                    created_at: Utc::now(),
                };
                service
                    .emit(
                        phase,
                        MethodologyEvent::FindingAdded(FindingAdded {
                            finding: Box::new(finding),
                            idempotency_key: key.clone(),
                        }),
                    )
                    .await?;
                Err(MethodologyError::FieldValidation {
                    field_path: "/test_partial_failure".into(),
                    expected: "success".into(),
                    actual: "forced failure".into(),
                    remediation: "intentional partial failure".into(),
                })
            },
        )
        .await
}

fn canonical_payload_hash(payload: &serde_json::Value) -> String {
    let canonical = super::canonical_json(payload).expect("canonical json");
    super::sha256_hex(canonical.as_bytes())
}

#[tokio::test]
async fn idempotency_replays_same_error_after_partial_event_emit() {
    let (store, service, spec_id) = mk_service().await;
    let phase = PhaseId::try_new("audit-task").expect("phase");
    let payload = idempotency_payload();
    let key = Some("idempotency-partial-failure".to_owned());

    let first = run_partial_failure(&service, &phase, spec_id, key.clone(), &payload).await;
    assert!(matches!(
        first,
        Err(MethodologyError::FieldValidation { .. })
    ));

    let second = service
        .run_idempotent_mutation("test_partial_failure", spec_id, key, &payload, || async {
            Ok(serde_json::json!({"unexpected":"success"}))
        })
        .await;
    assert!(matches!(
        second,
        Err(MethodologyError::FieldValidation { .. })
    ));

    let events = store
        .query_events(&EventFilter {
            entity_kind: Some(EntityKind::Finding),
            spec_id: Some(spec_id),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query events");
    assert_eq!(
        events.events.len(),
        1,
        "partial failure replay must not append duplicate events"
    );

    let idem = store
        .get_methodology_idempotency(
            "test_partial_failure",
            &spec_id.to_string(),
            "idempotency-partial-failure",
        )
        .await
        .expect("load idempotency")
        .expect("entry exists");
    assert!(
        idem.response_json.is_some(),
        "failed attempts must store terminal idempotency outcomes"
    );

    let stored = store
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            spec_id: Some(spec_id),
            limit: 10,
            ..EventFilter::new()
        })
        .await
        .expect("query")
        .events;
    assert!(stored.iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::FindingAdded(_)
        }
    )));
}

#[tokio::test]
async fn active_reservation_conflicts_until_lease_expires() {
    let (store, service, spec_id) = mk_service().await;
    let phase = PhaseId::try_new("audit-task").expect("phase");
    let payload = idempotency_payload();
    let key = "idempotency-active-lease".to_owned();
    let request_hash = canonical_payload_hash(&payload);
    store
        .insert_methodology_idempotency_reservation(
            tanren_store::methodology::InsertMethodologyIdempotencyParams {
                tool: "test_partial_failure".into(),
                scope_key: spec_id.to_string(),
                idempotency_key: key.clone(),
                request_hash,
                request_hash_algo: super::REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1.into(),
                reservation_expires_at: Utc::now() + Duration::seconds(120),
            },
        )
        .await
        .expect("seed active reservation");

    let result = run_partial_failure(&service, &phase, spec_id, Some(key), &payload).await;
    assert!(matches!(
        result,
        Err(MethodologyError::Conflict { reason, .. })
            if reason.contains("unfinished prior attempt")
    ));
}

#[tokio::test]
async fn expired_reservation_is_reclaimed_and_finalized() {
    let (store, service, spec_id) = mk_service().await;
    let payload = idempotency_payload();
    let key = "idempotency-expired-lease".to_owned();
    let request_hash = canonical_payload_hash(&payload);
    store
        .insert_methodology_idempotency_reservation(
            tanren_store::methodology::InsertMethodologyIdempotencyParams {
                tool: "test_reclaim_success".into(),
                scope_key: spec_id.to_string(),
                idempotency_key: key.clone(),
                request_hash,
                request_hash_algo: super::REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1.into(),
                reservation_expires_at: Utc::now() - Duration::seconds(10),
            },
        )
        .await
        .expect("seed expired reservation");

    let response = service
        .run_idempotent_mutation(
            "test_reclaim_success",
            spec_id,
            Some(key.clone()),
            &payload,
            || async { Ok(serde_json::json!({"ok": true})) },
        )
        .await
        .expect("reclaimed response");
    assert_eq!(response["ok"], serde_json::json!(true));

    let entry = store
        .get_methodology_idempotency("test_reclaim_success", &spec_id.to_string(), &key)
        .await
        .expect("load")
        .expect("entry");
    assert!(
        entry.response_json.is_some(),
        "reclaimed reservation must be finalized"
    );
    assert!(
        entry.reservation_expires_at.is_none(),
        "finalized reservation must clear active lease"
    );
}
