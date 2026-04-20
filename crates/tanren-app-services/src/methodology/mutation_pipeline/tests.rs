use super::*;
use std::sync::Arc;

use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::{EntityRef, EventId, TaskId};
use tanren_store::methodology::AppendPhaseEventOutboxParams;
use tanren_store::{EventFilter, EventStore, Store};

async fn mk_service() -> (MethodologyService, SpecId) {
    let store = Store::open_and_migrate("sqlite::memory:")
        .await
        .expect("open");
    let spec_id = SpecId::new();
    let runtime = crate::methodology::service::PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-methodology-mutation-pipeline-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "test-session".into(),
    };
    (
        MethodologyService::with_runtime(Arc::new(store), vec![], Some(runtime), vec![]),
        spec_id,
    )
}

#[tokio::test]
async fn finalize_emits_unauthorized_edit_and_reverts_file() {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let plan = spec_folder.join("plan.md");
    std::fs::write(&plan, "original\n").expect("seed");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    #[cfg(unix)]
    std::fs::set_permissions(&plan, std::fs::Permissions::from_mode(0o644))
        .expect("unlock protected file to simulate unauthorized agent edit");
    std::fs::write(&plan, "mutated\n").expect("mutate");
    let phase = PhaseId::try_new("do-task").expect("phase");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-1", guard)
        .await
        .expect("finalize");

    let on_disk = std::fs::read_to_string(&plan).expect("read");
    assert_eq!(on_disk, "original\n", "postflight must revert edits");

    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.events.into_iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::UnauthorizedArtifactEdit(_)
        }
    )));
}

#[tokio::test]
async fn finalize_reverts_newly_created_protected_artifact() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    let created = spec_folder.join("plan.md");
    std::fs::write(&created, "created during session\n").expect("create");
    let phase = PhaseId::try_new("do-task").expect("phase");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-3", guard)
        .await
        .expect("finalize");

    assert!(
        !created.exists(),
        "postflight must remove newly created protected artifacts"
    );

    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.events.into_iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::UnauthorizedArtifactEdit(_)
        }
    )));
}

#[tokio::test]
async fn finalize_preserves_unmanifested_user_artifact() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    let created = spec_folder.join("tool-index.json");
    std::fs::write(&created, "{\"generated\":true}\n").expect("create");
    let phase = PhaseId::try_new("do-task").expect("phase");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-4", guard)
        .await
        .expect("finalize");

    assert!(
        created.exists(),
        "unmanifested files must not be removed by heuristic filename checks"
    );

    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(
        !events.events.into_iter().any(|env| matches!(
            env.payload,
            DomainEvent::Methodology {
                event: MethodologyEvent::UnauthorizedArtifactEdit(_)
            }
        )),
        "no unauthorized-artifact event expected for unmanifested files"
    );
}

#[tokio::test]
async fn finalize_reverts_newly_created_manifested_generated_artifact() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    std::fs::write(
        spec_folder.join(".tanren-generated-artifacts.json"),
        r#"{ "generated_artifacts": ["tool-index.json"] }"#,
    )
    .expect("write manifest");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    let created = spec_folder.join("tool-index.json");
    std::fs::write(&created, "{\"generated\":true}\n").expect("create");
    let phase = PhaseId::try_new("do-task").expect("phase");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-4b", guard)
        .await
        .expect("finalize");

    assert!(
        !created.exists(),
        "manifested generated artifact must be removed"
    );

    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.events.into_iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::UnauthorizedArtifactEdit(_)
        }
    )));
}

#[tokio::test]
async fn finalize_reverts_unprojected_phase_events_appends() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let phase_events = spec_folder.join("phase-events.jsonl");
    std::fs::write(&phase_events, "").expect("seed");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    std::fs::write(&phase_events, "{\"seed\":1}\n{\"next\":2}\n").expect("append");
    let phase = PhaseId::try_new("do-task").expect("phase");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-5", guard)
        .await
        .expect("finalize");
    let on_disk = std::fs::read_to_string(&phase_events).expect("read phase-events");
    assert_eq!(on_disk, "", "unprojected append must be reverted");
    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.events.into_iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::UnauthorizedArtifactEdit(_)
        }
    )));
}

#[tokio::test]
async fn finalize_reverts_fresh_phase_events_file_without_projected_append() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let phase_events = spec_folder.join("phase-events.jsonl");
    let phase = PhaseId::try_new("do-task").expect("phase");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    std::fs::write(&phase_events, "{\"seed\":1}\n").expect("write");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-5a", guard)
        .await
        .expect("finalize");
    assert!(
        !phase_events.exists(),
        "fresh append-only file without projected outbox lines must be removed"
    );
    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.events.into_iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::UnauthorizedArtifactEdit(_)
        }
    )));
}

#[tokio::test]
async fn finalize_allows_fresh_phase_events_file_when_fully_projected() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let phase_events = spec_folder.join("phase-events.jsonl");
    let phase = PhaseId::try_new("do-task").expect("phase");

    let guard = enter_mutation_session(&spec_folder).expect("enter");

    let envelope = EventEnvelope {
        schema_version: tanren_domain::SCHEMA_VERSION,
        event_id: EventId::new(),
        timestamp: Utc::now(),
        entity_ref: EntityRef::Task(TaskId::from_uuid(
            uuid::Uuid::parse_str("019da764-e015-7af1-9385-8a7b98995e90").expect("valid task uuid"),
        )),
        payload: DomainEvent::Methodology {
            event: MethodologyEvent::TaskStarted(tanren_domain::methodology::events::TaskStarted {
                task_id: TaskId::from_uuid(
                    uuid::Uuid::parse_str("019da764-e015-7af1-9385-8a7b98995e90")
                        .expect("valid task uuid"),
                ),
                spec_id,
            }),
        },
    };
    let projected_line =
        crate::methodology::line_for_envelope(&envelope, spec_id, phase.as_str(), "session-5b")
            .expect("projected line");
    let projected_json = serde_json::to_string(&projected_line).expect("line json");
    service
        .store()
        .append_methodology_event_with_outbox(
            &envelope,
            Some(AppendPhaseEventOutboxParams {
                spec_id,
                spec_folder: spec_folder.to_string_lossy().to_string(),
                line_json: projected_json.clone(),
            }),
        )
        .await
        .expect("append outbox row");
    service
        .store()
        .mark_phase_event_outbox_projected(envelope.event_id)
        .await
        .expect("mark projected");
    std::fs::write(&phase_events, format!("{projected_json}\n")).expect("write");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-5b", guard)
        .await
        .expect("finalize");
    let on_disk = std::fs::read_to_string(&phase_events).expect("read");
    assert_eq!(on_disk, format!("{projected_json}\n"));
    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(
        !events.events.into_iter().any(|env| matches!(
            env.payload,
            DomainEvent::Methodology {
                event: MethodologyEvent::UnauthorizedArtifactEdit(_)
            }
        )),
        "projected fresh creation should not emit unauthorized edit"
    );
}

#[tokio::test]
async fn finalize_allows_projected_phase_events_appends() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let phase_events = spec_folder.join("phase-events.jsonl");
    std::fs::write(&phase_events, "").expect("seed");
    let phase = PhaseId::try_new("do-task").expect("phase");

    let guard = enter_mutation_session(&spec_folder).expect("enter");

    let envelope = EventEnvelope {
        schema_version: tanren_domain::SCHEMA_VERSION,
        event_id: EventId::new(),
        timestamp: Utc::now(),
        entity_ref: EntityRef::Task(TaskId::from_uuid(
            uuid::Uuid::parse_str("019da764-e015-7af1-9385-8a7b98995e90").expect("valid task uuid"),
        )),
        payload: DomainEvent::Methodology {
            event: MethodologyEvent::TaskStarted(tanren_domain::methodology::events::TaskStarted {
                task_id: TaskId::from_uuid(
                    uuid::Uuid::parse_str("019da764-e015-7af1-9385-8a7b98995e90")
                        .expect("valid task uuid"),
                ),
                spec_id,
            }),
        },
    };
    let projected_line =
        crate::methodology::line_for_envelope(&envelope, spec_id, phase.as_str(), "session-5")
            .expect("projected line");
    let projected_json = serde_json::to_string(&projected_line).expect("line json");
    service
        .store()
        .append_methodology_event_with_outbox(
            &envelope,
            Some(AppendPhaseEventOutboxParams {
                spec_id,
                spec_folder: spec_folder.to_string_lossy().to_string(),
                line_json: projected_json.clone(),
            }),
        )
        .await
        .expect("append outbox row");
    service
        .store()
        .mark_phase_event_outbox_projected(envelope.event_id)
        .await
        .expect("mark projected");

    std::fs::write(&phase_events, format!("{projected_json}\n")).expect("append projected");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-5", guard)
        .await
        .expect("finalize");

    let on_disk = std::fs::read_to_string(&phase_events).expect("read phase-events");
    assert!(
        on_disk.contains(&projected_json),
        "projected append should be preserved"
    );

    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(
        !events.events.into_iter().any(|env| matches!(
            env.payload,
            DomainEvent::Methodology {
                event: MethodologyEvent::UnauthorizedArtifactEdit(_)
            }
        )),
        "authorized append should not emit unauthorized-artifact events"
    );
}

#[tokio::test]
async fn finalize_reverts_non_append_only_phase_events_edits() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let phase_events = spec_folder.join("phase-events.jsonl");
    std::fs::write(&phase_events, "{\"seed\":1}\n{\"next\":2}\n").expect("seed");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    std::fs::write(&phase_events, "{\"seed\":9}\n{\"next\":2}\n").expect("mutate");
    let phase = PhaseId::try_new("do-task").expect("phase");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-6", guard)
        .await
        .expect("finalize");
    let on_disk = std::fs::read_to_string(&phase_events).expect("read phase-events");
    assert_eq!(
        on_disk, "{\"seed\":1}\n{\"next\":2}\n",
        "non-append edits should be reverted"
    );
    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.events.into_iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::UnauthorizedArtifactEdit(_)
        }
    )));
}

#[tokio::test]
async fn malformed_evidence_emits_schema_error_and_fails_closed() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    std::fs::write(spec_folder.join("audit.md"), "not-frontmatter\n").expect("write");
    let phase = PhaseId::try_new("audit-task").expect("phase");

    let err = finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-2", None)
        .await
        .expect_err("malformed evidence must fail");
    assert!(matches!(err, MethodologyError::EvidenceSchema { .. }));

    let events = service
        .store()
        .query_events(&EventFilter {
            event_type: Some("methodology".into()),
            limit: 100,
            ..EventFilter::new()
        })
        .await
        .expect("query");
    assert!(events.events.into_iter().any(|env| matches!(
        env.payload,
        DomainEvent::Methodology {
            event: MethodologyEvent::EvidenceSchemaError(_)
        }
    )));
}
