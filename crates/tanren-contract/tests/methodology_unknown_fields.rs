use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tanren_contract::methodology as c;

const SPEC_A: &str = "00000000-0000-0000-0000-000000000001";
const SPEC_B: &str = "00000000-0000-0000-0000-000000000002";
const TASK_A: &str = "00000000-0000-0000-0000-000000000011";
const TASK_B: &str = "00000000-0000-0000-0000-000000000012";
const FINDING_A: &str = "00000000-0000-0000-0000-000000000021";
const SIGNPOST_A: &str = "00000000-0000-0000-0000-000000000031";

fn assert_unknown_field_rejected<T: DeserializeOwned>(tool: &str, mut args: Value) {
    let obj = args
        .as_object_mut()
        .expect("tool params fixture must be a JSON object");
    obj.insert("unexpected_field".into(), json!(true));
    let msg = match serde_json::from_value::<T>(args) {
        Ok(_) => "unexpectedly accepted unknown field".to_owned(),
        Err(err) => err.to_string(),
    };
    assert!(
        msg.contains("unknown field") && msg.contains("unexpected_field"),
        "{tool} should fail on unknown_field, got: {msg}"
    );
}

#[test]
fn task_and_scoring_params_reject_unknown_fields() {
    assert_unknown_field_rejected::<c::CreateTaskParams>(
        "create_task",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "title": "Task",
            "description": "Desc",
            "origin": {"kind": "user"},
            "acceptance_criteria": []
        }),
    );
    assert_unknown_field_rejected::<c::StartTaskParams>(
        "start_task",
        json!({"schema_version": "1.0.0", "task_id": TASK_A}),
    );
    assert_unknown_field_rejected::<c::CompleteTaskParams>(
        "complete_task",
        json!({"schema_version": "1.0.0", "task_id": TASK_A, "evidence_refs": ["audit.md#L12"]}),
    );
    assert_unknown_field_rejected::<c::MarkTaskGuardSatisfiedParams>(
        "mark_task_guard_satisfied",
        json!({"schema_version": "1.0.0", "task_id": TASK_A, "guard": "audited"}),
    );
    assert_unknown_field_rejected::<c::ResetTaskGuardsParams>(
        "reset_task_guards",
        json!({
            "schema_version": "1.0.0",
            "task_id": TASK_A,
            "reason": "retry loop reset"
        }),
    );
    assert_unknown_field_rejected::<c::ReviseTaskParams>(
        "revise_task",
        json!({
            "schema_version": "1.0.0",
            "task_id": TASK_A,
            "revised_description": "Updated",
            "revised_acceptance": [{"id": "ac-1", "description": "desc", "measurable": "done"}],
            "reason": "spec drift"
        }),
    );
    assert_unknown_field_rejected::<c::AbandonTaskParams>(
        "abandon_task",
        json!({
            "schema_version": "1.0.0",
            "task_id": TASK_A,
            "reason": "superseded",
            "disposition": "replacement",
            "replacements": [TASK_B]
        }),
    );
    assert_unknown_field_rejected::<c::ListTasksParams>(
        "list_tasks",
        json!({"schema_version": "1.0.0", "spec_id": SPEC_A}),
    );
    assert_unknown_field_rejected::<c::AddFindingParams>(
        "add_finding",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "severity": "fix_now",
            "title": "title",
            "description": "description",
            "source": {"kind": "triage"}
        }),
    );
    assert_unknown_field_rejected::<c::RecordRubricScoreParams>(
        "record_rubric_score",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "scope": "task",
            "scope_target_id": "task-a",
            "pillar": "completeness",
            "score": 10,
            "target": 10,
            "passing": 7,
            "rationale": "ok",
            "supporting_finding_ids": [FINDING_A]
        }),
    );
    assert_unknown_field_rejected::<c::RecordNonNegotiableComplianceParams>(
        "record_non_negotiable_compliance",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "scope": "spec",
            "name": "no-panic",
            "status": "pass",
            "rationale": "checked"
        }),
    );
}

#[test]
fn spec_and_demo_params_reject_unknown_fields() {
    spec_params_reject_unknown_fields();
    demo_params_reject_unknown_fields();
}

fn spec_params_reject_unknown_fields() {
    assert_unknown_field_rejected::<c::SetSpecTitleParams>(
        "set_spec_title",
        json!({"schema_version": "1.0.0", "spec_id": SPEC_A, "title": "New title"}),
    );
    assert_unknown_field_rejected::<c::SetSpecProblemStatementParams>(
        "set_spec_problem_statement",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "problem_statement": "Current behavior is inconsistent."
        }),
    );
    assert_unknown_field_rejected::<c::SetSpecMotivationsParams>(
        "set_spec_motivations",
        json!({"schema_version": "1.0.0", "spec_id": SPEC_A, "motivations": ["determinism"]}),
    );
    assert_unknown_field_rejected::<c::SetSpecExpectationsParams>(
        "set_spec_expectations",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "expectations": ["always generate plan.md"]
        }),
    );
    assert_unknown_field_rejected::<c::SetSpecPlannedBehaviorsParams>(
        "set_spec_planned_behaviors",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "planned_behaviors": ["emit typed events only"]
        }),
    );
    assert_unknown_field_rejected::<c::SetSpecImplementationPlanParams>(
        "set_spec_implementation_plan",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "implementation_plan": ["wire projector", "update docs"]
        }),
    );
    assert_unknown_field_rejected::<c::SetSpecNonNegotiablesParams>(
        "set_spec_non_negotiables",
        json!({"schema_version": "1.0.0", "spec_id": SPEC_A, "items": ["no-panic"]}),
    );
    assert_unknown_field_rejected::<c::AddSpecAcceptanceCriterionParams>(
        "add_spec_acceptance_criterion",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "criterion": {"id": "ac-1", "description": "desc", "measurable": "done"}
        }),
    );
    assert_unknown_field_rejected::<c::SetSpecDemoEnvironmentParams>(
        "set_spec_demo_environment",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "demo_environment": {
                "connections": [{"name": "api", "kind": "http", "probe": "GET /health"}]
            }
        }),
    );
    assert_unknown_field_rejected::<c::SetSpecDependenciesParams>(
        "set_spec_dependencies",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "dependencies": {"depends_on_spec_ids": [SPEC_B], "external_issue_refs": ["github#1"]}
        }),
    );
    assert_unknown_field_rejected::<c::SetSpecBaseBranchParams>(
        "set_spec_base_branch",
        json!({"schema_version": "1.0.0", "spec_id": SPEC_A, "branch": "main"}),
    );
    assert_unknown_field_rejected::<c::SetSpecRelevanceContextParams>(
        "set_spec_relevance_context",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "relevance_context": {
                "touched_files": ["src/lib.rs"],
                "project_language": "rust",
                "tags": ["security"],
                "category": "backend"
            }
        }),
    );
    assert_unknown_field_rejected::<c::SpecStatusParams>(
        "spec_status",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A
        }),
    );
}

fn demo_params_reject_unknown_fields() {
    assert_unknown_field_rejected::<c::AddDemoStepParams>(
        "add_demo_step",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "id": "step-1",
            "mode": "RUN",
            "description": "run",
            "expected_observable": "pass"
        }),
    );
    assert_unknown_field_rejected::<c::MarkDemoStepSkipParams>(
        "mark_demo_step_skip",
        json!({"schema_version": "1.0.0", "spec_id": SPEC_A, "step_id": "step-2", "reason": "n/a"}),
    );
    assert_unknown_field_rejected::<c::AppendDemoResultParams>(
        "append_demo_result",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "step_id": "step-1",
            "status": "pass",
            "observed": "ok"
        }),
    );
}

#[test]
fn signpost_phase_and_issue_params_reject_unknown_fields() {
    assert_unknown_field_rejected::<c::AddSignpostParams>(
        "add_signpost",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "task_id": TASK_A,
            "status": "unresolved",
            "problem": "problem",
            "evidence": "evidence",
            "tried": ["attempt 1"],
            "files_affected": ["src/lib.rs"]
        }),
    );
    assert_unknown_field_rejected::<c::UpdateSignpostStatusParams>(
        "update_signpost_status",
        json!({"schema_version": "1.0.0", "signpost_id": SIGNPOST_A, "status": "resolved", "resolution": "done"}),
    );
    assert_unknown_field_rejected::<c::ReportPhaseOutcomeParams>(
        "report_phase_outcome",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "task_id": TASK_A,
            "outcome": {"outcome": "complete", "summary": "ok"}
        }),
    );
    assert_unknown_field_rejected::<c::EscalateToBlockerParams>(
        "escalate_to_blocker",
        json!({"schema_version": "1.0.0", "spec_id": SPEC_A, "reason": "blocked", "options": ["revise", "create"]}),
    );
    assert_unknown_field_rejected::<c::PostReplyDirectiveParams>(
        "post_reply_directive",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "thread_ref": "review-thread-1",
            "body": "thanks",
            "disposition": "ack"
        }),
    );
    assert_unknown_field_rejected::<c::CreateIssueParams>(
        "create_issue",
        json!({
            "schema_version": "1.0.0",
            "origin_spec_id": SPEC_A,
            "title": "issue",
            "description": "desc",
            "suggested_spec_scope": "next lane",
            "priority": "high"
        }),
    );
}

#[test]
fn standards_and_adherence_params_reject_unknown_fields() {
    assert_unknown_field_rejected::<c::ListRelevantStandardsParams>(
        "list_relevant_standards",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "touched_files": ["src/lib.rs"],
            "project_language": "rust",
            "domains": ["backend"],
            "tags": ["security"],
            "category": "backend"
        }),
    );
    assert_unknown_field_rejected::<c::RecordAdherenceFindingParams>(
        "record_adherence_finding",
        json!({
            "schema_version": "1.0.0",
            "spec_id": SPEC_A,
            "standard": {"name": "no-unwrap", "category": "rust"},
            "affected_files": ["src/lib.rs"],
            "line_numbers": [12],
            "severity": "fix_now",
            "rationale": "unwrap in production path"
        }),
    );
}
