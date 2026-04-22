use super::*;
use std::num::NonZeroU32;
use tanren_domain::NonEmptyString;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::{TaskCreated, TaskStarted};
use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};
use tanren_domain::{EntityRef, EventId, TaskId};

fn task(spec: SpecId) -> Task {
    Task {
        id: TaskId::new(),
        spec_id: spec,
        title: NonEmptyString::try_new("t").expect("non-empty"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::ShapeSpec,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn project_filters_by_spec_id() {
    let spec_a = SpecId::new();
    let spec_b = SpecId::new();
    let task_a = task(spec_a);
    let task_b = task(spec_b);
    let env_a = EventEnvelope {
        schema_version: tanren_domain::SCHEMA_VERSION,
        event_id: EventId::new(),
        timestamp: Utc::now(),
        entity_ref: EntityRef::Task(task_a.id),
        payload: DomainEvent::Methodology {
            event: MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(task_a),
                origin: TaskOrigin::ShapeSpec,
                idempotency_key: None,
            }),
        },
    };
    let env_b = EventEnvelope {
        schema_version: tanren_domain::SCHEMA_VERSION,
        event_id: EventId::new(),
        timestamp: Utc::now(),
        entity_ref: EntityRef::Task(task_b.id),
        payload: DomainEvent::Methodology {
            event: MethodologyEvent::TaskStarted(TaskStarted {
                task_id: task_b.id,
                spec_id: spec_b,
            }),
        },
    };
    let lines = project_phase_events(&[env_a, env_b], spec_a, "do-task", "session-1");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spec_id, spec_a);
}

#[test]
fn render_jsonl_is_lf_terminated() {
    let spec = SpecId::new();
    let line = PhaseEventLine {
        schema_version: PHASE_EVENT_LINE_SCHEMA_VERSION.to_owned(),
        event_id: EventId::new(),
        spec_id: spec,
        phase: "do-task".into(),
        agent_session_id: "s1".into(),
        timestamp: Utc::now(),
        caused_by_tool_call_id: Some("call-1".into()),
        origin_kind: PhaseEventOriginKind::ToolPrimary,
        tool: "start_task".into(),
        payload: MethodologyEvent::TaskStarted(TaskStarted {
            task_id: TaskId::new(),
            spec_id: spec,
        }),
    };
    let text = render_jsonl(&[line.clone(), line]).expect("render");
    assert_eq!(text.lines().count(), 2);
    assert!(text.ends_with('\n'));
}

#[test]
fn append_jsonl_encoded_line_is_line_safe_under_concurrency() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("phase-events.jsonl");
    let workers = 8usize;
    let per_worker = 100usize;
    let mut handles = Vec::with_capacity(workers);
    for worker in 0..workers {
        let path = path.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..per_worker {
                let encoded = format!("{{\"worker\":{worker},\"seq\":{i}}}");
                append_jsonl_encoded_line(&path, &encoded).expect("append");
            }
        }));
    }
    for handle in handles {
        handle.join().expect("join");
    }
    let text = std::fs::read_to_string(&path).expect("read");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), workers * per_worker);
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).expect("valid json");
        assert!(parsed.get("worker").is_some());
        assert!(parsed.get("seq").is_some());
    }
}

#[test]
fn append_jsonl_encoded_line_dedup_skips_existing_event_id() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("phase-events.jsonl");
    let event_id = EventId::new();
    let encoded = format!("{{\"event_id\":\"{event_id}\",\"value\":1}}");
    let first = append_jsonl_encoded_line_if_missing_event_id(&path, &encoded, Some(event_id))
        .expect("first append");
    let second = append_jsonl_encoded_line_if_missing_event_id(&path, &encoded, Some(event_id))
        .expect("second append");
    assert!(first, "first append should write");
    assert!(!second, "second append should be skipped as duplicate");
    let raw = std::fs::read_to_string(path).expect("read");
    let lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1);
}

#[test]
fn dedupe_recovery_rebuilds_missing_marker_from_existing_jsonl() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("phase-events.jsonl");
    let event_id = EventId::new();
    let encoded = format!("{{\"event_id\":\"{event_id}\",\"value\":1}}");
    append_jsonl_encoded_line_if_missing_event_id(&path, &encoded, Some(event_id))
        .expect("initial append");
    let marker = phase_events_storage::event_id_marker_path(&path, event_id);
    assert!(marker.exists(), "marker must exist after first append");
    std::fs::remove_file(&marker).expect("remove marker");
    let appended = append_jsonl_encoded_line_if_missing_event_id(&path, &encoded, Some(event_id))
        .expect("second append");
    assert!(!appended, "second append should skip duplicate");
    assert!(
        marker.exists(),
        "duplicate-recovery path should rebuild missing marker"
    );
    let raw = std::fs::read_to_string(path).expect("read");
    let lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1, "recovery path must avoid duplicate write");
}

#[test]
fn fsync_batch_policy_persists_pending_counter() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("phase-events.jsonl");
    let policy = PhaseEventsAppendPolicy {
        fsync_every: NonZeroU32::new(3).expect("non-zero"),
    };
    append_jsonl_encoded_line_if_missing_event_id_with_policy(&path, "{\"a\":1}", None, policy)
        .expect("append 1");
    let state_path = phase_events_storage::phase_events_fsync_state_path(&path);
    let state1: PhaseEventsFsyncState =
        serde_json::from_str(&std::fs::read_to_string(&state_path).expect("state1"))
            .expect("parse state1");
    assert_eq!(state1.pending_since_last_sync, 1);

    append_jsonl_encoded_line_if_missing_event_id_with_policy(&path, "{\"a\":2}", None, policy)
        .expect("append 2");
    let state2: PhaseEventsFsyncState =
        serde_json::from_str(&std::fs::read_to_string(&state_path).expect("state2"))
            .expect("parse state2");
    assert_eq!(state2.pending_since_last_sync, 2);

    append_jsonl_encoded_line_if_missing_event_id_with_policy(&path, "{\"a\":3}", None, policy)
        .expect("append 3");
    let state3: PhaseEventsFsyncState =
        serde_json::from_str(&std::fs::read_to_string(&state_path).expect("state3"))
            .expect("parse state3");
    assert_eq!(state3.pending_since_last_sync, 0);
}

#[test]
fn compact_jsonl_event_log_removes_duplicates_and_blanks_and_rebuilds_index() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("phase-events.jsonl");
    let event_a = EventId::new();
    let event_b = EventId::new();
    let raw = format!(
        "\n{{\"event_id\":\"{event_a}\",\"n\":1}}\n\n{{\"event_id\":\"{event_a}\",\"n\":2}}\n{{\"event_id\":\"{event_b}\",\"n\":3}}\n"
    );
    std::fs::write(&path, raw).expect("seed");

    let report = compact_jsonl_event_log(&path).expect("compact");
    assert_eq!(report.total_lines_before, 5);
    assert_eq!(report.total_lines_after, 2);
    assert_eq!(report.duplicates_removed, 1);
    assert_eq!(report.empty_lines_removed, 2);
    assert!(report.rewrote_file);

    let on_disk = std::fs::read_to_string(&path).expect("read compacted");
    let lines: Vec<&str> = on_disk.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains(&event_a.to_string()));
    assert!(lines[1].contains(&event_b.to_string()));
    assert!(event_id_marker_exists(&path, event_a));
    assert!(event_id_marker_exists(&path, event_b));
}
