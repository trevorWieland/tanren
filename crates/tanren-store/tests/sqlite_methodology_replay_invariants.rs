#[path = "support/methodology_replay_support.rs"]
mod methodology_replay_support;

use chrono::Utc;
use serde_json::json;
use tanren_domain::methodology::event_tool::canonical_tool_for_event;
use tanren_domain::methodology::events::{
    FindingAdded, MethodologyEvent, TaskAbandoned, TaskCreated,
};
use tanren_domain::methodology::task::{
    ExplicitUserDiscardProvenance, RequiredGuard, TaskAbandonDisposition, TaskOrigin,
};
use tanren_domain::{FindingId, NonEmptyString, SpecId, TaskId};
use tanren_store::methodology::{ReplayError, ingest_phase_events};

use self::methodology_replay_support::{
    fresh_store, line_json, seed_finding, seed_task, temp_path,
};

#[tokio::test]
async fn replay_rejects_explicit_discard_outside_resolve_blockers_phase() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let created = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let abandoned = MethodologyEvent::TaskAbandoned(TaskAbandoned {
        task_id,
        spec_id,
        reason: NonEmptyString::try_new("explicit discard approved").expect("reason"),
        disposition: TaskAbandonDisposition::ExplicitUserDiscard,
        replacements: vec![],
        explicit_user_discard_provenance: Some(ExplicitUserDiscardProvenance::ResolveBlockers {
            resolution_note: NonEmptyString::try_new("approved by user").expect("note"),
        }),
    });
    let path = temp_path("replay-explicit-discard-phase");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &created,
                canonical_tool_for_event(&created),
            ),
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &abandoned,
                canonical_tool_for_event(&abandoned),
            )
        ),
    )
    .expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("explicit_user_discard outside resolve-blockers must fail");
    assert!(matches!(
        err,
        ReplayError::FieldValidation { ref details } if details.field_path == "/disposition"
    ));
}

#[tokio::test]
async fn replay_rejects_explicit_discard_without_provenance() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let created = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let abandoned = MethodologyEvent::TaskAbandoned(TaskAbandoned {
        task_id,
        spec_id,
        reason: NonEmptyString::try_new("explicit discard approved").expect("reason"),
        disposition: TaskAbandonDisposition::ExplicitUserDiscard,
        replacements: vec![],
        explicit_user_discard_provenance: None,
    });
    let created_line = line_json(
        spec_id,
        uuid::Uuid::now_v7(),
        &created,
        canonical_tool_for_event(&created),
    );
    let abandon_line = serde_json::to_string(&json!({
        "event_id": uuid::Uuid::now_v7(),
        "spec_id": spec_id,
        "phase": "resolve-blockers",
        "agent_session_id": "session-1",
        "timestamp": Utc::now(),
        "origin_kind": "tool_primary",
        "tool": canonical_tool_for_event(&abandoned),
        "payload": abandoned,
    }))
    .expect("serialize");
    let path = temp_path("replay-explicit-discard-provenance");
    std::fs::write(&path, format!("{created_line}\n{abandon_line}\n")).expect("write");

    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("explicit_user_discard requires provenance");
    assert!(matches!(
        err,
        ReplayError::FieldValidation { ref details }
            if details.field_path == "/explicit_user_discard_provenance"
    ));
}

#[tokio::test]
async fn replay_rejects_replacement_disposition_without_replacements() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let created = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(seed_task(spec_id, task_id)),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let abandoned = MethodologyEvent::TaskAbandoned(TaskAbandoned {
        task_id,
        spec_id,
        reason: NonEmptyString::try_new("abandon").expect("reason"),
        disposition: TaskAbandonDisposition::Replacement,
        replacements: vec![],
        explicit_user_discard_provenance: None,
    });
    let path = temp_path("replay-replacement-empty");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &created,
                canonical_tool_for_event(&created),
            ),
            line_json(
                spec_id,
                uuid::Uuid::now_v7(),
                &abandoned,
                canonical_tool_for_event(&abandoned),
            )
        ),
    )
    .expect("write");
    let err = ingest_phase_events(&store, &path, &[RequiredGuard::GateChecked])
        .await
        .expect_err("replacement disposition must include replacements");
    assert!(matches!(
        err,
        ReplayError::FieldValidation { ref details } if details.field_path == "/replacements"
    ));
}

#[tokio::test]
async fn replay_rejects_finding_with_zero_line_number() {
    let store = fresh_store().await;
    let spec_id = SpecId::new();
    let task_id = TaskId::new();
    let mut finding = seed_finding(spec_id, task_id, FindingId::new(), "bad line");
    finding.attached_task = None;
    finding.line_numbers = vec![0];
    let event = MethodologyEvent::FindingAdded(FindingAdded {
        finding: Box::new(finding),
        idempotency_key: None,
    });
    let path = temp_path("replay-finding-line-zero");
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
        .expect_err("finding line number zero must fail");
    assert!(matches!(
        err,
        ReplayError::FieldValidation { ref details } if details.field_path == "/line_numbers/0"
    ));
}
