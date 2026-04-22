use std::path::Path;

use chrono::Utc;
use tanren_domain::methodology::events::{MethodologyEvent, TaskCreated};
use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};
use tanren_domain::{EventId, NonEmptyString, SpecId, TaskId};

pub(super) fn seed_phase_event_line(spec_id: SpecId, title: &str) -> String {
    let task = Task {
        id: TaskId::new(),
        spec_id,
        title: NonEmptyString::try_new(title).expect("title"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::User,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let payload = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(task),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    serde_json::to_string(&serde_json::json!({
        "schema_version": "1.0.0",
        "event_id": EventId::new(),
        "spec_id": spec_id,
        "phase": "do-task",
        "agent_session_id": "session-1",
        "timestamp": Utc::now(),
        "origin_kind": "tool_primary",
        "tool": "create_task",
        "payload": payload,
    }))
    .expect("serialize seeded phase-events line")
}

pub(super) fn seed_phase_events_file(spec_folder: &Path, spec_id: SpecId) {
    let line = seed_phase_event_line(spec_id, "seed task");
    std::fs::write(spec_folder.join("phase-events.jsonl"), format!("{line}\n"))
        .expect("write seeded phase-events");
}
