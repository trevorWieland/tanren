use super::tests_support::seed_phase_event_line;
use super::*;

use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_store::{EventFilter, EventStore};

#[tokio::test]
async fn finalize_reverts_non_append_only_phase_events_edits() {
    let (service, spec_id) = mk_service().await;
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let phase_events = spec_folder.join("phase-events.jsonl");
    let seed_a = seed_phase_event_line(spec_id, "seed-a");
    let seed_b = seed_phase_event_line(spec_id, "seed-b");
    let baseline = format!("{seed_a}\n{seed_b}\n");
    std::fs::write(&phase_events, &baseline).expect("seed");

    let guard = enter_mutation_session(&spec_folder).expect("enter");
    let mutated = format!("{}\n{seed_b}\n", seed_phase_event_line(spec_id, "seed-z"));
    std::fs::write(&phase_events, &mutated).expect("mutate");
    let phase = PhaseId::try_new("do-task").expect("phase");

    finalize_mutation_session(&service, &phase, spec_id, &spec_folder, "session-6", guard)
        .await
        .expect("finalize");
    let on_disk = std::fs::read_to_string(&phase_events).expect("read phase-events");
    assert_eq!(on_disk, baseline, "non-append edits should be reverted");
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

#[test]
fn protected_artifacts_include_expanded_generated_set() {
    let root = tempfile::tempdir().expect("tempdir");
    let spec_folder = root.path();
    let protected = protected_artifacts(spec_folder);
    let paths = protected
        .iter()
        .map(|entry| {
            entry
                .path
                .file_name()
                .expect("filename")
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(paths.contains(&"audit.md".to_owned()));
    assert!(paths.contains(&"signposts.md".to_owned()));
    assert!(paths.contains(&".tanren-projection-checkpoint.json".to_owned()));
}
