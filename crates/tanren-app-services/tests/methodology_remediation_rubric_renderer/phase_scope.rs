use super::*;

#[tokio::test]
async fn rubric_score_rejects_phase_outside_audit() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let spec_id = runtime_spec_id(&svc);
    let create = svc
        .create_task(
            &scope,
            &phase("audit-task"),
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "T".into(),
                description: String::new(),
                acceptance_criteria: vec![],
                depends_on: vec![],
                parent_task_id: None,
                origin: TaskOrigin::ShapeSpec,
            },
        )
        .await
        .expect("create");
    let err = svc
        .record_rubric_score(
            &scope,
            &phase("do-task"),
            RecordRubricScoreParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                scope: PillarScope::Task,
                scope_target_id: Some(create.task_id.to_string()),
                pillar: PillarId::try_new("security").expect("pillar"),
                score: PillarScore::try_new(8).expect("score"),
                target: PillarScore::try_new(10).expect("target"),
                passing: PillarScore::try_new(7).expect("passing"),
                rationale: "reason".into(),
                supporting_finding_ids: vec![],
                idempotency_key: None,
            },
        )
        .await
        .expect_err("non-audit phase should fail");
    assert!(
        err.to_string().contains("/phase"),
        "expected /phase validation error, got {err}"
    );
}

#[tokio::test]
async fn non_negotiable_compliance_rejects_phase_outside_audit() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let spec_id = runtime_spec_id(&svc);
    let err = svc
        .record_non_negotiable_compliance(
            &scope,
            &phase("do-task"),
            RecordNonNegotiableComplianceParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                scope: PillarScope::Task,
                name: "must-pass".into(),
                status: ComplianceStatus::Pass,
                rationale: "ok".into(),
                idempotency_key: None,
            },
        )
        .await
        .expect_err("non-audit phase should fail");
    assert!(
        err.to_string().contains("/phase"),
        "expected /phase validation error, got {err}"
    );
}
