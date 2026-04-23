use schemars::JsonSchema;
use schemars::schema_for;
use serde::de::DeserializeOwned;
use serde_json::json;
use tanren_contract::methodology::{
    AbandonTaskParams, AckResponse, AddDemoStepParams, AddFindingParams, AddFindingResponse,
    AddSignpostParams, AddSignpostResponse, AddSpecAcceptanceCriterionParams,
    AppendDemoResultParams, CompleteTaskParams, CreateIssueParams, CreateIssueResponse,
    CreateTaskParams, CreateTaskResponse, EscalateToBlockerParams, ListRelevantStandardsParams,
    ListRelevantStandardsResponse, ListTasksParams, ListTasksResponse, METHODOLOGY_SCHEMA_VERSION,
    MarkDemoStepSkipParams, MarkTaskGuardSatisfiedParams, PostReplyDirectiveParams,
    RecordAdherenceFindingParams, RecordNonNegotiableComplianceParams, RecordRubricScoreParams,
    RelevantStandard, ReportPhaseOutcomeParams, ResetTaskGuardsParams, ReviseTaskParams,
    SchemaVersion, SetSpecBaseBranchParams, SetSpecDemoEnvironmentParams,
    SetSpecDependenciesParams, SetSpecExpectationsParams, SetSpecImplementationPlanParams,
    SetSpecMotivationsParams, SetSpecNonNegotiablesParams, SetSpecPlannedBehaviorsParams,
    SetSpecProblemStatementParams, SetSpecRelevanceContextParams, SetSpecTitleParams,
    SpecStatusParams, SpecStatusResponse, StartTaskParams, UpdateSignpostStatusParams,
};

fn assert_schema_version_required<T: JsonSchema>() {
    let schema_json = serde_json::to_value(schema_for!(T)).expect("schema json");
    let required = schema_json
        .get("required")
        .and_then(serde_json::Value::as_array)
        .expect("schema required[]");
    let properties = schema_json
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("schema properties");
    assert!(
        required.iter().any(|v| v == "schema_version"),
        "schema_version must be required"
    );
    assert!(
        properties.contains_key("schema_version"),
        "schema_version must be declared"
    );
}

fn parse_fixture<T: DeserializeOwned>(fixture: serde_json::Value) -> T {
    serde_json::from_value(fixture).expect("fixture must deserialize")
}

fn assert_serialized_schema_version<T: serde::Serialize>(value: T) {
    let body = serde_json::to_value(value).expect("serialize");
    assert_eq!(
        body.get("schema_version")
            .and_then(serde_json::Value::as_str),
        Some(METHODOLOGY_SCHEMA_VERSION)
    );
}

#[test]
fn every_methodology_request_schema_requires_schema_version() {
    assert_schema_version_required::<CreateTaskParams>();
    assert_schema_version_required::<StartTaskParams>();
    assert_schema_version_required::<CompleteTaskParams>();
    assert_schema_version_required::<MarkTaskGuardSatisfiedParams>();
    assert_schema_version_required::<ResetTaskGuardsParams>();
    assert_schema_version_required::<ReviseTaskParams>();
    assert_schema_version_required::<AbandonTaskParams>();
    assert_schema_version_required::<ListTasksParams>();
    assert_schema_version_required::<AddFindingParams>();
    assert_schema_version_required::<RecordRubricScoreParams>();
    assert_schema_version_required::<RecordNonNegotiableComplianceParams>();
    assert_schema_version_required::<SetSpecTitleParams>();
    assert_schema_version_required::<SetSpecProblemStatementParams>();
    assert_schema_version_required::<SetSpecMotivationsParams>();
    assert_schema_version_required::<SetSpecExpectationsParams>();
    assert_schema_version_required::<SetSpecPlannedBehaviorsParams>();
    assert_schema_version_required::<SetSpecImplementationPlanParams>();
    assert_schema_version_required::<SetSpecNonNegotiablesParams>();
    assert_schema_version_required::<AddSpecAcceptanceCriterionParams>();
    assert_schema_version_required::<SetSpecDemoEnvironmentParams>();
    assert_schema_version_required::<SetSpecDependenciesParams>();
    assert_schema_version_required::<SetSpecBaseBranchParams>();
    assert_schema_version_required::<SetSpecRelevanceContextParams>();
    assert_schema_version_required::<SpecStatusParams>();
    assert_schema_version_required::<AddDemoStepParams>();
    assert_schema_version_required::<MarkDemoStepSkipParams>();
    assert_schema_version_required::<AppendDemoResultParams>();
    assert_schema_version_required::<AddSignpostParams>();
    assert_schema_version_required::<UpdateSignpostStatusParams>();
    assert_schema_version_required::<ReportPhaseOutcomeParams>();
    assert_schema_version_required::<EscalateToBlockerParams>();
    assert_schema_version_required::<PostReplyDirectiveParams>();
    assert_schema_version_required::<CreateIssueParams>();
    assert_schema_version_required::<ListRelevantStandardsParams>();
    assert_schema_version_required::<RecordAdherenceFindingParams>();
}

#[test]
fn every_methodology_response_schema_requires_schema_version() {
    assert_schema_version_required::<AckResponse>();
    assert_schema_version_required::<CreateTaskResponse>();
    assert_schema_version_required::<ListTasksResponse>();
    assert_schema_version_required::<AddFindingResponse>();
    assert_schema_version_required::<AddSignpostResponse>();
    assert_schema_version_required::<CreateIssueResponse>();
    assert_schema_version_required::<RelevantStandard>();
    assert_schema_version_required::<ListRelevantStandardsResponse>();
    assert_schema_version_required::<SpecStatusResponse>();
}

#[test]
fn every_methodology_request_payload_serializes_schema_version() {
    let schema_version = SchemaVersion::current();
    assert_task_and_quality_request_payload_schema_versions(&schema_version);
    assert_spec_and_demo_request_payload_schema_versions(&schema_version);
    assert_phase_issue_and_adherence_request_payload_schema_versions(&schema_version);
}

fn assert_task_and_quality_request_payload_schema_versions(schema_version: &SchemaVersion) {
    assert_serialized_schema_version(parse_fixture::<CreateTaskParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "title": "Task",
        "description": "Desc",
        "origin": {"kind": "user"},
        "acceptance_criteria": []
    })));
    assert_serialized_schema_version(parse_fixture::<StartTaskParams>(json!({
        "schema_version": schema_version.as_str(),
        "task_id": "00000000-0000-0000-0000-000000000011"
    })));
    assert_serialized_schema_version(parse_fixture::<CompleteTaskParams>(json!({
        "schema_version": schema_version.as_str(),
        "task_id": "00000000-0000-0000-0000-000000000011",
        "evidence_refs": []
    })));
    assert_serialized_schema_version(parse_fixture::<MarkTaskGuardSatisfiedParams>(json!({
        "schema_version": schema_version.as_str(),
        "task_id": "00000000-0000-0000-0000-000000000011",
        "guard": "audited"
    })));
    assert_serialized_schema_version(parse_fixture::<ResetTaskGuardsParams>(json!({
        "schema_version": schema_version.as_str(),
        "task_id": "00000000-0000-0000-0000-000000000011",
        "reason": "retry from investigate loop"
    })));
    assert_serialized_schema_version(parse_fixture::<ReviseTaskParams>(json!({
        "schema_version": schema_version.as_str(),
        "task_id": "00000000-0000-0000-0000-000000000011",
        "revised_description": "Updated",
        "revised_acceptance": [],
        "reason": "spec drift"
    })));
    assert_serialized_schema_version(parse_fixture::<AbandonTaskParams>(json!({
        "schema_version": schema_version.as_str(),
        "task_id": "00000000-0000-0000-0000-000000000011",
        "reason": "superseded",
        "disposition": "replacement",
        "replacements": ["00000000-0000-0000-0000-000000000012"]
    })));
    assert_serialized_schema_version(parse_fixture::<ListTasksParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001"
    })));
    assert_serialized_schema_version(parse_fixture::<AddFindingParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "severity": "fix_now",
        "title": "title",
        "description": "description",
        "source": {"kind": "triage"}
    })));
    assert_serialized_schema_version(parse_fixture::<RecordRubricScoreParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "scope": "task",
        "scope_target_id": "task-a",
        "pillar": "completeness",
        "score": 10,
        "target": 10,
        "passing": 7,
        "rationale": "ok",
        "supporting_finding_ids": ["00000000-0000-0000-0000-000000000021"]
    })));
    assert_serialized_schema_version(parse_fixture::<RecordNonNegotiableComplianceParams>(
        json!({
            "schema_version": schema_version.as_str(),
            "spec_id": "00000000-0000-0000-0000-000000000001",
            "scope": "spec",
            "name": "no-panic",
            "status": "pass",
            "rationale": "checked"
        }),
    ));
}

fn assert_spec_and_demo_request_payload_schema_versions(schema_version: &SchemaVersion) {
    assert_spec_frontmatter_request_payload_schema_versions(schema_version);
    assert_demo_request_payload_schema_versions(schema_version);
    assert_signpost_request_payload_schema_versions(schema_version);
}

fn assert_spec_frontmatter_request_payload_schema_versions(schema_version: &SchemaVersion) {
    assert_serialized_schema_version(parse_fixture::<SetSpecTitleParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "title": "New title"
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecProblemStatementParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "problem_statement": "Current behavior is inconsistent."
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecMotivationsParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "motivations": ["determinism", "auditability"]
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecExpectationsParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "expectations": ["plan stays current", "status is event-derived"]
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecPlannedBehaviorsParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "planned_behaviors": ["emit typed events", "materialize projections"]
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecImplementationPlanParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "implementation_plan": ["add schema", "wire projection", "verify ci"]
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecNonNegotiablesParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "items": ["no-panic"]
    })));
    assert_serialized_schema_version(parse_fixture::<AddSpecAcceptanceCriterionParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "criterion": {"id": "ac-1", "description": "desc", "measurable": "done"}
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecDemoEnvironmentParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "demo_environment": {
            "connections": [{"name": "api", "kind": "http", "probe": "GET /health"}]
        }
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecDependenciesParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "dependencies": {
            "depends_on_spec_ids": ["00000000-0000-0000-0000-000000000002"],
            "external_issue_refs": ["github#1"]
        }
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecBaseBranchParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "branch": "main"
    })));
    assert_serialized_schema_version(parse_fixture::<SetSpecRelevanceContextParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "relevance_context": {
            "touched_files": ["src/lib.rs"],
            "project_language": "rust",
            "tags": ["security"],
            "category": "backend"
        }
    })));
    assert_serialized_schema_version(parse_fixture::<SpecStatusParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001"
    })));
}

fn assert_demo_request_payload_schema_versions(schema_version: &SchemaVersion) {
    assert_serialized_schema_version(parse_fixture::<AddDemoStepParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "id": "step-1",
        "mode": "RUN",
        "description": "run",
        "expected_observable": "pass"
    })));
    assert_serialized_schema_version(parse_fixture::<MarkDemoStepSkipParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "step_id": "step-2",
        "reason": "n/a"
    })));
    assert_serialized_schema_version(parse_fixture::<AppendDemoResultParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "step_id": "step-1",
        "status": "pass",
        "observed": "ok"
    })));
}

fn assert_signpost_request_payload_schema_versions(schema_version: &SchemaVersion) {
    assert_serialized_schema_version(parse_fixture::<AddSignpostParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "task_id": "00000000-0000-0000-0000-000000000011",
        "status": "unresolved",
        "problem": "problem",
        "evidence": "evidence",
        "tried": ["attempt 1"],
        "files_affected": ["src/lib.rs"]
    })));
    assert_serialized_schema_version(parse_fixture::<UpdateSignpostStatusParams>(json!({
        "schema_version": schema_version.as_str(),
        "signpost_id": "00000000-0000-0000-0000-000000000031",
        "status": "resolved",
        "resolution": "done"
    })));
}

fn assert_phase_issue_and_adherence_request_payload_schema_versions(
    schema_version: &SchemaVersion,
) {
    assert_serialized_schema_version(parse_fixture::<ReportPhaseOutcomeParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "task_id": "00000000-0000-0000-0000-000000000011",
        "outcome": {"outcome": "complete", "summary": "ok"}
    })));
    assert_serialized_schema_version(parse_fixture::<EscalateToBlockerParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "reason": "blocked",
        "options": ["revise", "create"]
    })));
    assert_serialized_schema_version(parse_fixture::<PostReplyDirectiveParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "thread_ref": "review-thread-1",
        "body": "thanks",
        "disposition": "ack"
    })));
    assert_serialized_schema_version(parse_fixture::<CreateIssueParams>(json!({
        "schema_version": schema_version.as_str(),
        "origin_spec_id": "00000000-0000-0000-0000-000000000001",
        "title": "issue",
        "description": "desc",
        "suggested_spec_scope": "next lane",
        "priority": "high"
    })));
    assert_serialized_schema_version(parse_fixture::<ListRelevantStandardsParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "touched_files": ["src/lib.rs"],
        "project_language": "rust",
        "domains": ["backend"],
        "tags": ["security"],
        "category": "backend"
    })));
    assert_serialized_schema_version(parse_fixture::<RecordAdherenceFindingParams>(json!({
        "schema_version": schema_version.as_str(),
        "spec_id": "00000000-0000-0000-0000-000000000001",
        "standard": {"name": "no-unwrap-in-production", "category": "rust-error-handling"},
        "affected_files": ["src/lib.rs"],
        "line_numbers": [12],
        "severity": "fix_now",
        "rationale": "unwrap in production path"
    })));
}

#[test]
fn every_methodology_response_payload_serializes_schema_version() {
    let schema_version = SchemaVersion::current();
    assert_serialized_schema_version(parse_fixture::<AckResponse>(json!({
        "schema_version": schema_version.as_str()
    })));
    assert_serialized_schema_version(parse_fixture::<CreateTaskResponse>(json!({
        "schema_version": schema_version.as_str(),
        "task_id": "00000000-0000-0000-0000-000000000011"
    })));
    assert_serialized_schema_version(parse_fixture::<ListTasksResponse>(json!({
        "schema_version": schema_version.as_str(),
        "tasks": []
    })));
    assert_serialized_schema_version(parse_fixture::<AddFindingResponse>(json!({
        "schema_version": schema_version.as_str(),
        "finding_id": "00000000-0000-0000-0000-000000000021"
    })));
    assert_serialized_schema_version(parse_fixture::<AddSignpostResponse>(json!({
        "schema_version": schema_version.as_str(),
        "signpost_id": "00000000-0000-0000-0000-000000000031"
    })));
    assert_serialized_schema_version(parse_fixture::<CreateIssueResponse>(json!({
        "schema_version": schema_version.as_str(),
        "issue_id": "00000000-0000-0000-0000-000000000041",
        "reference": {
            "provider": "github",
            "number": 1,
            "url": "https://example.invalid/issues/1"
        }
    })));
    assert_serialized_schema_version(parse_fixture::<RelevantStandard>(json!({
        "schema_version": schema_version.as_str(),
        "standard": {
            "name": "no-unwrap-in-production",
            "category": "rust-error-handling",
            "applies_to": ["**/*.rs"],
            "applies_to_languages": ["rust"],
            "applies_to_domains": ["error-handling"],
            "importance": "critical",
            "body": "no unwrap"
        },
        "inclusion_reason": "matched applies_to"
    })));
    assert_serialized_schema_version(parse_fixture::<ListRelevantStandardsResponse>(json!({
        "schema_version": schema_version.as_str(),
        "standards": []
    })));
}
