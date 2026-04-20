use super::*;

fn make_folder(root: &std::path::Path, spec_id: SpecId, suffix: &str) -> std::path::PathBuf {
    root.join(format!("2026-01-01-0101-{spec_id}-{suffix}"))
}

async fn seed_task_started_outbox_rows(
    svc: &MethodologyService,
    spec_id: SpecId,
    phase_name: &str,
    folder_raw: &str,
    session: &str,
    row_count: usize,
) {
    for _ in 0..row_count {
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
        let line =
            line_for_envelope(&envelope, spec_id, phase_name, session).expect("project line");
        let line_json = serde_json::to_string(&line).expect("line json");
        svc.store()
            .append_methodology_event_with_outbox(
                &envelope,
                Some(AppendPhaseEventOutboxParams {
                    spec_id,
                    spec_folder: folder_raw.to_owned(),
                    line_json,
                }),
            )
            .await
            .expect("seed outbox row");
    }
}

fn non_empty_line_count(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path)
        .expect("phase-events file")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

#[tokio::test]
async fn reconcile_folder_query_is_scoped_and_paged_without_starvation() {
    let target_spec_id = SpecId::new();
    let backlog_spec_id = SpecId::new();
    let root = tempfile::tempdir().expect("tempdir");
    let target_folder = make_folder(root.path(), target_spec_id, "target-folder");
    let backlog_folder = make_folder(root.path(), backlog_spec_id, "backlog-folder");
    std::fs::create_dir_all(&target_folder).expect("mkdir target folder");
    std::fs::create_dir_all(&backlog_folder).expect("mkdir backlog folder");

    let runtime = PhaseEventsRuntime {
        spec_id: target_spec_id,
        spec_folder: target_folder.clone(),
        agent_session_id: "runtime-session-reconcile-starvation".into(),
    };
    let svc = mk_service(vec![], runtime).await;
    let phase_name = phase("shape-spec").as_str().to_owned();
    let target_folder_raw = target_folder.to_string_lossy().to_string();
    let backlog_folder_raw = backlog_folder.to_string_lossy().to_string();

    // Seed >10_000 rows in folder B first so non-scoped global scans
    // would never reach folder A in one reconcile pass.
    let backlog_row_count = 10_001usize;
    seed_task_started_outbox_rows(
        &svc,
        backlog_spec_id,
        phase_name.as_str(),
        backlog_folder_raw.as_str(),
        "seed-session-backlog",
        backlog_row_count,
    )
    .await;

    let target_row_count = 3usize;
    seed_task_started_outbox_rows(
        &svc,
        target_spec_id,
        phase_name.as_str(),
        target_folder_raw.as_str(),
        "seed-session-target",
        target_row_count,
    )
    .await;

    let projected = svc
        .reconcile_phase_events_outbox_for_folder(&target_folder)
        .await
        .expect("reconcile target folder");
    assert_eq!(
        projected, target_row_count as u64,
        "scoped reconcile must fully drain targeted folder rows"
    );

    let pending_target = svc
        .store()
        .load_pending_phase_event_outbox(Some(target_spec_id), 100)
        .await
        .expect("pending target spec");
    assert!(
        pending_target.is_empty(),
        "target-folder rows should be fully drained"
    );

    let pending_backlog = svc
        .store()
        .load_pending_phase_event_outbox(Some(backlog_spec_id), (backlog_row_count + 10) as u64)
        .await
        .expect("pending backlog spec");
    assert_eq!(
        pending_backlog.len(),
        backlog_row_count,
        "backlog rows must remain untouched by target-folder reconcile"
    );

    assert_eq!(
        non_empty_line_count(&target_folder.join("phase-events.jsonl")),
        target_row_count,
        "target-folder reconcile should write every targeted row exactly once"
    );
}
