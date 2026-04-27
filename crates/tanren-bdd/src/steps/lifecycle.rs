use std::fs;

use cucumber::{given, then, when};
use serde_json::json;
use tanren_contract::methodology::{
    AddFindingParams, CreateTaskParams, FindingLifecycleParams, RecordCheckResultParams,
    RecordInvestigationAttemptParams,
};
use tanren_testkit::temp_repo::TempRepo;

use super::lifecycle_support::{
    LIFECYCLE_SPEC_ID, OTHER_SPEC_ID, add_audit_finding, assert_success, block_phase,
    complete_all_spec_checks, complete_phase, create_completed_lifecycle_task,
    create_implemented_lifecycle_task, lifecycle_evidence, lifecycle_finding_id,
    lifecycle_spec_folder, lifecycle_task_id, list_open_fix_now, run_lifecycle_fail,
    run_lifecycle_json, run_lifecycle_ok, source_check, spec_status, start_audit_check,
    string_field,
};
use crate::world::BehaviorWorld;

#[given("an initialized finding lifecycle command repository")]
fn given_initialized_finding_lifecycle_command_repository(world: &mut BehaviorWorld) {
    world.installer_repo =
        Some(TempRepo::create("tanren-bdd-lifecycle").expect("create temporary repo"));
    world.installer_output = None;
    world.lifecycle_task_id = None;
    world.lifecycle_finding_id = None;
    world.lifecycle_check_run_id = None;
    world.lifecycle_attempt_id = None;
    world.lifecycle_root_cause_id = None;

    let repo_path = world.repo_path().to_path_buf();
    let spec_folder = repo_path.join("tanren/specs/finding-lifecycle");
    fs::create_dir_all(&spec_folder).expect("create lifecycle spec folder");
    let standards_root = repo_path.join("tanren/standards");
    fs::create_dir_all(&standards_root).expect("create standards root");
    fs::create_dir_all(standards_root.join("proof")).expect("create standard category");
    fs::write(
        standards_root.join("proof/lifecycle.md"),
        concat!(
            "---\n",
            "kind: standard\n",
            "name: lifecycle\n",
            "category: proof\n",
            "importance: high\n",
            "applies_to: []\n",
            "applies_to_languages: []\n",
            "applies_to_domains: []\n",
            "---\n\n",
            "# Lifecycle standard\n\n",
            "Remediation findings must be tracked.\n",
        ),
    )
    .expect("write lifecycle standard");
    let config_path = repo_path.join("tanren.yml");
    fs::write(
        &config_path,
        "methodology:\n  task_complete_requires: [audited, adherent]\n",
    )
    .expect("write lifecycle methodology config");
    let database_url = format!("sqlite:{}?mode=rwc", repo_path.join("tanren.db").display());
    world.lifecycle_database_url = Some(database_url.clone());
    world.lifecycle_spec_folder = Some(spec_folder);
    world.lifecycle_config_path = Some(config_path);

    world.run_cli(
        vec![
            "--database-url".into(),
            database_url,
            "db".into(),
            "migrate".into(),
        ],
        false,
    );
    assert_success(world);
}

#[when("a task-scoped audit finding is investigated")]
fn when_task_scoped_audit_finding_is_investigated(world: &mut BehaviorWorld) {
    let task_id = create_implemented_lifecycle_task(world);
    let finding_id = add_audit_finding(world, Some(&task_id), "audit-task");
    let task_scope = json!({"scope": "task", "task_id": task_id});
    let check_run_id = start_audit_check(world, "audit-task", &task_scope);
    let source_check = source_check("audit-task", &task_scope);
    block_phase(
        world,
        "audit-task",
        Some(&task_id),
        "task audit found a blocking issue",
    );
    let attempt_response = run_lifecycle_json(
        world,
        "investigate",
        &["investigation", "record-attempt"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "fingerprint": format!("audit-task:{task_id}"),
            "loop_index": 1,
            "source_check": source_check,
            "source_findings": [finding_id],
            "evidence_refs": ["investigation-report.json"],
            "root_causes": [{
                "description": "implementation missed the audit requirement",
                "confidence": "high",
                "category": "code_bug",
                "affected_files": ["src/lib.rs"]
            }]
        }),
    );
    let attempt_id = string_field(&attempt_response, "attempt_id");
    let root_cause_id = attempt_response["root_cause_ids"][0]
        .as_str()
        .expect("root_cause_id")
        .to_owned();
    run_lifecycle_ok(
        world,
        "investigate",
        &["investigation", "link-root-cause"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "attempt_id": attempt_id,
            "root_cause_id": root_cause_id,
            "finding_id": finding_id,
            "source_check": source_check
        }),
    );
    complete_phase(
        world,
        "investigate",
        Some(&task_id),
        "investigation recorded task repair context",
    );
    world.lifecycle_finding_id = Some(finding_id);
    world.lifecycle_check_run_id = Some(check_run_id);
    world.lifecycle_attempt_id = Some(attempt_id);
    world.lifecycle_root_cause_id = Some(root_cause_id);
    world.lifecycle_task_id = Some(task_id);
}

#[when("do-task records repair evidence for the same task")]
fn when_do_task_records_repair_evidence_for_same_task(world: &mut BehaviorWorld) {
    let task_id = lifecycle_task_id(world);
    run_lifecycle_ok(
        world,
        "do-task",
        &["task", "complete"],
        &json!({
            "schema_version": "1.0.0",
            "task_id": task_id,
            "evidence_refs": ["implementation.log"]
        }),
    );
}

#[when("a task-scoped audit finding is investigated twice")]
fn when_task_scoped_audit_finding_is_investigated_twice(world: &mut BehaviorWorld) {
    when_task_scoped_audit_finding_is_investigated(world);
    let task_id = lifecycle_task_id(world);
    let finding_id = lifecycle_finding_id(world);
    let task_scope = json!({"scope": "task", "task_id": task_id});
    let source_check = source_check("audit-task", &task_scope);
    let attempt_response = run_lifecycle_json(
        world,
        "investigate",
        &["investigation", "record-attempt"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "fingerprint": format!("audit-task:{task_id}"),
            "loop_index": 2,
            "source_check": source_check,
            "source_findings": [finding_id],
            "evidence_refs": ["investigation-report-2.json"],
            "root_causes": [{
                "description": "repair attempt still missed the audit requirement",
                "confidence": "medium",
                "category": "code_bug",
                "affected_files": ["src/lib.rs"]
            }]
        }),
    );
    world.lifecycle_attempt_id = Some(string_field(&attempt_response, "attempt_id"));
}

#[when("audit resolves the finding and completes the task check")]
fn when_audit_resolves_finding_and_completes_task_check(world: &mut BehaviorWorld) {
    let task_id = lifecycle_task_id(world);
    let finding_id = lifecycle_finding_id(world);
    run_lifecycle_ok(
        world,
        "audit-spec",
        &["finding", "resolve"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "finding_id": finding_id,
            "evidence": lifecycle_evidence(world, "audit independently verified the fix")
        }),
    );
    complete_phase(world, "audit-task", Some(&task_id), "audit task passed");
    complete_phase(
        world,
        "adhere-task",
        Some(&task_id),
        "adherence task passed",
    );
}

#[when("all spec checks report complete")]
fn when_all_spec_checks_report_complete(world: &mut BehaviorWorld) {
    complete_all_spec_checks(world);
}

#[when("all spec checks reported complete before a later task mutation")]
fn when_spec_checks_are_stale_after_task_mutation(world: &mut BehaviorWorld) {
    complete_all_spec_checks(world);
    let task_id = create_completed_lifecycle_task(world);
    world.lifecycle_task_id = Some(task_id);
}

#[when("a task-scoped audit finding remains open")]
fn when_task_scoped_audit_finding_remains_open(world: &mut BehaviorWorld) {
    let task_id = create_completed_lifecycle_task(world);
    complete_all_spec_checks(world);
    let finding_id = add_audit_finding(world, Some(&task_id), "audit-task");
    world.lifecycle_task_id = Some(task_id);
    world.lifecycle_finding_id = Some(finding_id);
}

#[then("audit-task complete is rejected while open blocking findings remain")]
fn then_audit_task_complete_is_rejected(world: &mut BehaviorWorld) {
    let task_id = lifecycle_task_id(world);
    run_lifecycle_fail(
        world,
        "audit-task",
        &["phase", "outcome"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "task_id": task_id,
            "outcome": {"outcome": "complete", "summary": "should be rejected"}
        }),
    );
    let output = world.installer_output.as_ref().expect("audit-task output");
    assert!(
        output.stderr.contains("open finding ids"),
        "{}",
        output.stderr
    );
}

#[then("spec status is not walk-ready because open blocking findings remain")]
fn then_spec_status_is_not_walk_ready(world: &mut BehaviorWorld) {
    let status = spec_status(world);
    assert_eq!(status["ready_for_walk_spec"], false);
    let open = list_open_fix_now(world);
    assert!(
        !open["findings"]
            .as_array()
            .expect("findings array")
            .is_empty(),
        "{open}"
    );
}

#[then("do-task is rejected when it tries to resolve the finding")]
fn then_do_task_is_rejected_when_resolving(world: &mut BehaviorWorld) {
    let finding_id = lifecycle_finding_id(world);
    run_lifecycle_fail(
        world,
        "do-task",
        &["finding", "resolve"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "finding_id": finding_id,
            "evidence": {
                "summary": "do-task attempted to close the finding",
                "evidence_refs": ["implementation.log"]
            }
        }),
    );
    let output = world.installer_output.as_ref().expect("do-task output");
    assert!(
        output.stderr.contains("capability_denied"),
        "{}",
        output.stderr
    );
}

#[then("finding list shows no open fix_now findings")]
fn then_finding_list_shows_no_open_fix_now_findings(world: &mut BehaviorWorld) {
    let open = list_open_fix_now(world);
    assert_eq!(
        open["findings"].as_array().expect("findings array").len(),
        0
    );
    let all = run_lifecycle_json(
        world,
        "audit-spec",
        &["finding", "list"],
        &json!({"schema_version": "1.0.0", "spec_id": LIFECYCLE_SPEC_ID}),
    );
    assert_eq!(all["findings"][0]["status"], "resolved");
}

#[then("investigation attempts list includes source finding history")]
fn then_investigation_attempts_list_includes_source_finding_history(world: &mut BehaviorWorld) {
    let attempt_id = world
        .lifecycle_attempt_id
        .as_ref()
        .expect("attempt id")
        .clone();
    let finding_id = lifecycle_finding_id(world);
    let attempts = run_lifecycle_json(
        world,
        "investigate",
        &["investigation", "list-attempts"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "fingerprint": format!("audit-task:{}", lifecycle_task_id(world)),
            "finding_id": finding_id
        }),
    );
    let attempts = attempts["attempts"].as_array().expect("attempts array");
    let attempt = attempts
        .iter()
        .find(|item| item["id"] == attempt_id)
        .expect("recorded attempt");
    assert!(
        attempt["source_findings"]
            .as_array()
            .expect("source findings")
            .iter()
            .any(|item| item == &json!(finding_id)),
        "{attempt}"
    );
}

#[then("spec status routes investigation recovery to the same task")]
fn then_spec_status_routes_investigation_recovery_to_same_task(world: &mut BehaviorWorld) {
    let status = spec_status(world);
    assert_eq!(status["ready_for_walk_spec"], false, "{status}");
    assert_eq!(status["next_transition"], "task_do", "{status}");
    assert_eq!(status["next_task_id"], lifecycle_task_id(world), "{status}");
    assert_eq!(
        status["investigate_source_task_id"],
        lifecycle_task_id(world),
        "{status}"
    );
}

#[then("investigation attempts list preserves both attempts")]
fn then_investigation_attempts_list_preserves_both_attempts(world: &mut BehaviorWorld) {
    let task_id = lifecycle_task_id(world);
    let finding_id = lifecycle_finding_id(world);
    let attempts = run_lifecycle_json(
        world,
        "investigate",
        &["investigation", "list-attempts"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "fingerprint": format!("audit-task:{task_id}"),
            "finding_id": finding_id
        }),
    );
    let attempts = attempts["attempts"].as_array().expect("attempts array");
    assert_eq!(attempts.len(), 2, "{attempts:?}");
    assert!(
        attempts.iter().any(|item| item["loop_index"] == 1),
        "{attempts:?}"
    );
    assert!(
        attempts.iter().any(|item| item["loop_index"] == 2),
        "{attempts:?}"
    );
}

#[then("spec status is walk-ready")]
fn then_spec_status_is_walk_ready(world: &mut BehaviorWorld) {
    let status = spec_status(world);
    assert_eq!(status["ready_for_walk_spec"], true, "{status}");
    assert_eq!(status["next_transition"], "walk_spec_required", "{status}");
    assert_eq!(
        status["last_blocker_phase"],
        serde_json::Value::Null,
        "{status}"
    );
    assert_eq!(
        status["last_blocker_summary"],
        serde_json::Value::Null,
        "{status}"
    );
    assert_eq!(
        status["last_blocker_reason"],
        serde_json::Value::Null,
        "{status}"
    );
    let pending = status["pending_spec_checks"].as_array().map_or(0, Vec::len);
    assert_eq!(pending, 0, "{status}");
}

#[then("spec status requires a fresh spec check batch")]
fn then_spec_status_requires_fresh_spec_check_batch(world: &mut BehaviorWorld) {
    let status = spec_status(world);
    assert_eq!(status["ready_for_walk_spec"], false, "{status}");
    assert_eq!(status["next_transition"], "spec_check_batch", "{status}");
    assert_eq!(status["next_task_id"], serde_json::Value::Null, "{status}");
    let pending = status["pending_spec_checks"]
        .as_array()
        .expect("pending spec checks");
    assert_eq!(pending.len(), 4, "{status}");
    for expected in ["spec_gate", "run_demo", "audit_spec", "adhere_spec"] {
        assert!(
            pending.iter().any(|check| check == expected),
            "missing {expected}: {status}"
        );
    }
}

#[then("audit artifacts count only open fix_now findings")]
fn then_audit_artifacts_count_only_open_fix_now_findings(world: &mut BehaviorWorld) {
    let audit = fs::read_to_string(lifecycle_spec_folder(world).join("audit.md"))
        .expect("read projected audit.md");
    assert!(audit.contains("fix_now_count: 0"), "{audit}");
    assert!(audit.contains("status `resolved`"), "{audit}");
}

#[given("malformed finding lifecycle and generic check payloads")]
fn given_malformed_lifecycle_payloads(_world: &mut BehaviorWorld) {}

#[then("lifecycle contracts reject unknown fields and invalid links")]
fn then_lifecycle_contracts_reject_malformed_payloads(_world: &mut BehaviorWorld) {
    let unknown_finding = serde_json::from_value::<AddFindingParams>(json!({
        "schema_version": "1.0.0",
        "spec_id": LIFECYCLE_SPEC_ID,
        "severity": "fix_now",
        "title": "bad payload",
        "description": "unknown fields are rejected",
        "source": {"kind": "audit", "phase": "audit-spec", "pillar": "correctness"},
        "unexpected": true
    }));
    let bad_lifecycle = serde_json::from_value::<FindingLifecycleParams>(json!({
        "schema_version": "1.0.0",
        "spec_id": LIFECYCLE_SPEC_ID,
        "finding_id": "not-a-uuid",
        "evidence": {"summary": "bad id"}
    }));
    let unknown_check = serde_json::from_value::<RecordCheckResultParams>(json!({
        "schema_version": "1.0.0",
        "check_run_id": "00000000-0000-0000-0000-000000000001",
        "spec_id": LIFECYCLE_SPEC_ID,
        "kind": {"kind": "audit"},
        "scope": {"scope": "spec"},
        "status": "fail",
        "summary": "bad check",
        "unexpected": true
    }));
    let removed_remediation_origin = serde_json::from_value::<CreateTaskParams>(json!({
        "schema_version": "1.0.0",
        "spec_id": LIFECYCLE_SPEC_ID,
        "title": "removed remediation task",
        "description": "remediation origin no longer exists",
        "origin": {
            "kind": "remediation",
            "source_check": source_check("audit-spec", &json!({"scope": "spec"})),
            "finding_ids": [OTHER_SPEC_ID],
            "root_cause_id": "00000000-0000-0000-0000-000000000002",
            "attempt_id": "00000000-0000-0000-0000-000000000003"
        },
        "acceptance_criteria": []
    }));
    let unknown_investigation = serde_json::from_value::<RecordInvestigationAttemptParams>(json!({
        "schema_version": "1.0.0",
        "spec_id": LIFECYCLE_SPEC_ID,
        "fingerprint": "bad",
        "loop_index": 1,
        "source_check": source_check("audit-spec", &json!({"scope": "spec"})),
        "unexpected": true
    }));

    assert!(unknown_finding.is_err());
    assert!(bad_lifecycle.is_err());
    assert!(unknown_check.is_err());
    assert!(removed_remediation_origin.is_err());
    assert!(unknown_investigation.is_err());
}
