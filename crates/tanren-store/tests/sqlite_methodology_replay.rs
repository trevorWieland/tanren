#[path = "support/methodology_replay_support.rs"]
mod methodology_replay_support;

use chrono::Utc;
use serde_json::json;
use tanren_domain::methodology::event_tool::canonical_tool_for_event;
use tanren_domain::methodology::events::{
    FindingAdded, MethodologyEvent, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated,
    TaskGateChecked, TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin};
use tanren_domain::{EntityKind, FindingId, SpecId, TaskId};
use tanren_store::methodology::{
    ReplayError, ReplayOptions, ingest_phase_events, ingest_phase_events_with_options,
};
use tanren_store::{EventFilter, EventStore};

use self::methodology_replay_support::{
    fresh_store, line_json, line_json_with_attribution, seed_finding, seed_task, temp_path,
};

#[tokio::test]
async fn replay_rejects_tool_derived_without_causal_link() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let path = temp_path("replay-missing-caused-by");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json_with_attribution(
                spec_id,
                uuid::Uuid::now_v7(),
                &event,
                canonical_tool_for_event(&event),
                Some("tool_derived"),
                None
            )
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("missing caused_by for derived origin must fail");
    assert!(matches!(err, ReplayError::MissingCausedByToolCall { .. }));
}

#[tokio::test]
async fn replay_rejects_missing_origin_kind_by_default() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let path = temp_path("replay-missing-origin-kind");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string(&json!({
                "event_id": uuid::Uuid::now_v7(),
                "spec_id": spec_id,
                "phase": "do-task",
                "agent_session_id": "session-1",
                "timestamp": Utc::now(),
                "tool": canonical_tool_for_event(&event),
                "payload": event,
            }))
            .expect("serialize")
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("missing origin_kind must fail in strict mode");
    assert!(matches!(err, ReplayError::MissingOriginKind { .. }));
}

#[tokio::test]
async fn replay_legacy_mode_accepts_missing_origin_kind() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let path = temp_path("replay-legacy-origin-kind");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string(&json!({
                "event_id": uuid::Uuid::now_v7(),
                "spec_id": spec_id,
                "phase": "do-task",
                "agent_session_id": "session-1",
                "timestamp": Utc::now(),
                "tool": canonical_tool_for_event(&event),
                "payload": event,
            }))
            .expect("serialize")
        ),
    )
    .expect("write");

    let stats = ingest_phase_events_with_options(
        &store,
        &path,
        &[RequiredGuard::GateChecked],
        ReplayOptions {
            allow_legacy_provenance: true,
        },
    )
    .await
    .expect("legacy replay");
    assert_eq!(stats.events_appended, 1);
}

#[tokio::test]
async fn replay_rejects_system_origin_for_tool_event() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let path = temp_path("replay-origin-kind-mismatch");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json_with_attribution(
                spec_id,
                uuid::Uuid::now_v7(),
                &event,
                canonical_tool_for_event(&event),
                Some("system"),
                Some("call-1")
            )
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("system origin for non-system event must fail");
    assert!(matches!(err, ReplayError::OriginKindMismatch { .. }));
}

#[tokio::test]
async fn replay_accepts_tool_derived_with_causal_link() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let path = temp_path("replay-derived-with-caused-by");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json_with_attribution(
                spec_id,
                uuid::Uuid::now_v7(),
                &event,
                canonical_tool_for_event(&event),
                Some("tool_derived"),
                Some("call-1")
            )
        ),
    )
    .expect("write");

    let stats = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect("replay");
    assert_eq!(stats.events_appended, 1);
}

#[tokio::test]
async fn replay_ingests_canonical_phase_event_lines() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();

    let events = vec![
        MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(seed_task(spec_id, task_id)),
            origin: TaskOrigin::User,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskStarted(TaskStarted { task_id, spec_id }),
        MethodologyEvent::TaskImplemented(TaskImplemented {
            task_id,
            spec_id,
            evidence_refs: vec![],
        }),
        MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id,
            spec_id,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskAudited(TaskAudited {
            task_id,
            spec_id,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskAdherent(TaskAdherent {
            task_id,
            spec_id,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskCompleted(TaskCompleted { task_id, spec_id }),
    ];

    let path = temp_path("replay-canonical");
    let mut content = String::new();
    for event in &events {
        content.push_str(&line_json(
            spec_id,
            uuid::Uuid::now_v7(),
            event,
            canonical_tool_for_event(event),
        ));
        content.push('\n');
    }
    std::fs::write(&path, content).expect("write jsonl");

    let stats = ingest_phase_events(
        &store,
        &path,
        &[
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ],
    )
    .await
    .expect("ingest");

    assert_eq!(stats.events_appended, events.len());
    assert_eq!(stats.events_skipped_duplicate_event_id, 0);

    let queried = store
        .query_events(&EventFilter {
            entity_kind: Some(EntityKind::Task),
            event_type: Some("methodology".into()),
            spec_id: Some(spec_id),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert_eq!(queried.events.len(), events.len());
}

#[tokio::test]
async fn replay_rejects_tool_mismatch() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });

    let path = temp_path("replay-tool-mismatch");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json(spec_id, uuid::Uuid::now_v7(), &event, "start_task")
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("tool mismatch");
    assert!(matches!(err, ReplayError::ToolMismatch { .. }));
}

#[tokio::test]
async fn replay_rejects_invalid_sequence() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCompleted(TaskCompleted { task_id, spec_id });

    let path = temp_path("replay-invalid-seq");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &event,
                canonical_tool_for_event(&event),
            )
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("invalid sequence");
    assert!(matches!(err, ReplayError::MissingTaskCreate { .. }));
}

#[tokio::test]
async fn replay_dedupes_duplicate_event_ids() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let event = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let event_id = uuid::Uuid::now_v7();

    let line = line_json(spec_id, event_id, &event, canonical_tool_for_event(&event));
    let path = temp_path("replay-dedupe");
    std::fs::write(&path, format!("{line}\n{line}\n")).expect("write");

    let stats = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect("ingest");
    assert_eq!(stats.events_appended, 1);
    assert_eq!(stats.events_skipped_duplicate_event_id, 1);
}

#[tokio::test]
async fn replay_reports_malformed_line_with_raw_context() {
    let store = fresh_store().await;
    let path = temp_path("replay-malformed");
    std::fs::write(&path, "{not json}\n").expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("malformed");
    assert!(matches!(err, ReplayError::MalformedLine { .. }));
}

#[tokio::test]
async fn replay_preserves_line_number_and_raw_for_midstream_malformed_line() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let created = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let valid = line_json(
        spec_id,
        uuid::Uuid::now_v7(),
        &created,
        canonical_tool_for_event(&created),
    );
    let malformed = "{definitely-not-json}";
    let path = temp_path("replay-malformed-midstream");
    std::fs::write(&path, format!("{valid}\n{malformed}\n")).expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("malformed");
    assert!(
        matches!(err, ReplayError::MalformedLine { .. }),
        "expected malformed-line error"
    );
    if let ReplayError::MalformedLine { line, raw, .. } = err {
        assert_eq!(line, 2);
        assert_eq!(raw, malformed);
    }
    let queried = store
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            spec_id: Some(spec_id),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert_eq!(
        queried.events.len(),
        0,
        "transactional replay should not persist any lines from a malformed file"
    );
}

#[tokio::test]
async fn findings_by_ids_uses_sparse_lookup() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let id1 = FindingId::new();
    let id2 = FindingId::new();
    let mut finding_one = seed_finding(spec_id, task_id, id1, "one");
    finding_one.attached_task = None;
    let mut finding_two = seed_finding(spec_id, task_id, id2, "two");
    finding_two.attached_task = None;
    let f1 = MethodologyEvent::FindingAdded(FindingAdded {
        finding: Box::new(finding_one),
        idempotency_key: None,
    });
    let f2 = MethodologyEvent::FindingAdded(FindingAdded {
        finding: Box::new(finding_two),
        idempotency_key: None,
    });

    let path = temp_path("findings-by-ids");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            line_json(spec_id, uuid::Uuid::now_v7(), &f1, "add_finding"),
            line_json(spec_id, uuid::Uuid::now_v7(), &f2, "add_finding")
        ),
    )
    .expect("write");
    ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect("ingest");

    let one = tanren_store::methodology::projections::findings_by_ids(&store, spec_id, &[id2])
        .await
        .expect("lookup");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, id2);
}

#[tokio::test]
async fn findings_by_ids_chunks_large_id_lists() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let id = FindingId::new();
    let mut finding = seed_finding(spec_id, task_id, id, "chunked");
    finding.attached_task = None;
    let finding_event = MethodologyEvent::FindingAdded(FindingAdded {
        finding: Box::new(finding),
        idempotency_key: None,
    });
    let path = temp_path("findings-by-ids-large");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &finding_event,
                canonical_tool_for_event(&finding_event),
            )
        ),
    )
    .expect("write");
    ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect("ingest");

    let mut requested = Vec::new();
    for _ in 0..2_500 {
        requested.push(FindingId::new());
    }
    requested.push(id);

    let found =
        tanren_store::methodology::projections::findings_by_ids(&store, spec_id, &requested)
            .await
            .expect("chunked lookup");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, id);
}
