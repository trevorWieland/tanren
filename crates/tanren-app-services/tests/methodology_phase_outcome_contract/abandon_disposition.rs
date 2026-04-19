use super::*;

#[tokio::test]
async fn explicit_user_discard_requires_resolve_blockers_phase_and_provenance() {
    let spec_id = SpecId::new();
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-phase-outcome-explicit-discard-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "runtime-session-explicit-discard".into(),
    };
    let svc = mk_service(vec![], runtime).await;
    let task_id = create_implemented_task(&svc, spec_id, "do-task").await;
    let scope = scope(&[ToolCapability::TaskAbandon]);

    let missing_provenance = svc
        .abandon_task(
            &scope,
            &phase("resolve-blockers"),
            AbandonTaskParams {
                schema_version: SchemaVersion::current(),
                task_id,
                reason: "user discarded the path".into(),
                disposition: TaskAbandonDisposition::ExplicitUserDiscard,
                replacements: vec![],
                explicit_user_discard_provenance: None,
                idempotency_key: Some("discard-missing-provenance".into()),
            },
        )
        .await
        .expect_err("missing provenance must be rejected");
    assert!(matches!(
        missing_provenance,
        MethodologyError::FieldValidation { ref field_path, .. }
            if field_path == "/explicit_user_discard_provenance"
    ));

    let wrong_phase = svc
        .abandon_task(
            &scope,
            &phase("investigate"),
            AbandonTaskParams {
                schema_version: SchemaVersion::current(),
                task_id,
                reason: "user discarded the path".into(),
                disposition: TaskAbandonDisposition::ExplicitUserDiscard,
                replacements: vec![],
                explicit_user_discard_provenance: Some(
                    ExplicitUserDiscardProvenance::ResolveBlockers {
                        resolution_note: NonEmptyString::try_new("approved by user").expect("note"),
                    },
                ),
                idempotency_key: Some("discard-wrong-phase".into()),
            },
        )
        .await
        .expect_err("explicit discard outside resolve-blockers must be rejected");
    assert!(matches!(
        wrong_phase,
        MethodologyError::FieldValidation { ref field_path, .. }
            if field_path == "/disposition"
    ));
}

#[tokio::test]
async fn replacement_disposition_requires_replacement_task_ids() {
    let spec_id = SpecId::new();
    let runtime = PhaseEventsRuntime {
        spec_id,
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-phase-outcome-replacement-disposition-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "runtime-session-replacement-disposition".into(),
    };
    let svc = mk_service(vec![], runtime).await;
    let task_id = create_implemented_task(&svc, spec_id, "do-task").await;

    let err = svc
        .abandon_task(
            &scope(&[ToolCapability::TaskAbandon]),
            &phase("do-task"),
            AbandonTaskParams {
                schema_version: SchemaVersion::current(),
                task_id,
                reason: "superseded".into(),
                disposition: TaskAbandonDisposition::Replacement,
                replacements: vec![],
                explicit_user_discard_provenance: None,
                idempotency_key: Some("replacement-missing-target".into()),
            },
        )
        .await
        .expect_err("replacement disposition without replacements must fail");
    assert!(matches!(
        err,
        MethodologyError::FieldValidation { ref field_path, .. } if field_path == "/replacements"
    ));
}
