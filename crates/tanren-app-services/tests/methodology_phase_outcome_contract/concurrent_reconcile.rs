use std::sync::Arc;

use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::{MethodologyEvent, TaskStarted};
use tanren_domain::{EntityRef, EventId, SpecId, TaskId};
use tanren_store::methodology::AppendPhaseEventOutboxParams;

use super::{PhaseEventsRuntime, line_for_envelope, mk_service};

#[tokio::test]
async fn concurrent_reconcile_does_not_double_project_same_outbox_row() {
    let spec_id = SpecId::new();
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-outbox-race"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir spec folder");
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: spec_folder.clone(),
        agent_session_id: "runtime-session-race".into(),
    };
    let svc = Arc::new(mk_service(vec![], runtime).await);
    let phase_name = "shape-spec";
    let task_id = TaskId::new();
    let envelope = EventEnvelope {
        schema_version: tanren_domain::SCHEMA_VERSION,
        event_id: EventId::new(),
        timestamp: chrono::Utc::now(),
        entity_ref: EntityRef::Task(task_id),
        payload: DomainEvent::Methodology {
            event: MethodologyEvent::TaskStarted(TaskStarted { task_id, spec_id }),
        },
    };
    let line = line_for_envelope(&envelope, spec_id, phase_name, "runtime-session-race")
        .expect("project line");
    let line_json = serde_json::to_string(&line).expect("line json");
    svc.store()
        .append_methodology_event_with_outbox(
            &envelope,
            Some(AppendPhaseEventOutboxParams {
                spec_id,
                spec_folder: spec_folder.to_string_lossy().to_string(),
                line_json: line_json.clone(),
            }),
        )
        .await
        .expect("seed outbox");

    let svc_a = Arc::clone(&svc);
    let folder_a = spec_folder.clone();
    let svc_b = Arc::clone(&svc);
    let folder_b = spec_folder.clone();
    let (r1, r2) = tokio::join!(
        async move {
            svc_a
                .reconcile_phase_events_outbox_for_folder(&folder_a)
                .await
        },
        async move {
            svc_b
                .reconcile_phase_events_outbox_for_folder(&folder_b)
                .await
        }
    );
    let projected = r1.expect("first reconcile") + r2.expect("second reconcile");
    assert_eq!(projected, 1, "row must be projected exactly once");

    let content = std::fs::read_to_string(spec_folder.join("phase-events.jsonl")).expect("read");
    let non_empty = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert_eq!(non_empty, 1, "file must contain exactly one projected line");
}
