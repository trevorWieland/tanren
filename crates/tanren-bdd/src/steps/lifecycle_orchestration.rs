use std::fs;

use cucumber::{given, then, when};
use serde_json::json;

use super::lifecycle_support::{
    LIFECYCLE_SPEC_ID, add_audit_finding, complete_all_spec_checks, complete_phase,
    create_completed_lifecycle_task, create_implemented_lifecycle_task, lifecycle_evidence,
    lifecycle_task_id, list_open_fix_now, run_lifecycle_fail, run_lifecycle_json, run_lifecycle_ok,
    source_check, spec_status, start_audit_check, string_field,
};
use crate::world::BehaviorWorld;

#[when("a spec-scoped audit finding remains open")]
fn when_spec_scoped_audit_finding_remains_open(world: &mut BehaviorWorld) {
    complete_all_spec_checks(world);
    let finding_id = add_audit_finding(world, None, "audit-spec");
    world.lifecycle_finding_id = Some(finding_id);
}

#[when("a completed task later has a resolved audit finding")]
fn when_completed_task_later_has_resolved_audit_finding(world: &mut BehaviorWorld) {
    let task_id = create_completed_lifecycle_task(world);
    let finding_id = add_audit_finding(world, Some(&task_id), "audit-task");
    let scope = json!({"scope": "task", "task_id": task_id});
    let check_run_id = start_audit_check(world, "audit-task", &scope);
    world.lifecycle_task_id = Some(task_id.clone());
    world.lifecycle_finding_id = Some(finding_id.clone());
    world.lifecycle_check_run_id = Some(check_run_id);
    run_lifecycle_ok(
        world,
        "audit-task",
        &["finding", "resolve"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "finding_id": finding_id,
            "evidence": lifecycle_evidence(world, "terminal task recheck resolved blocker")
        }),
    );
    complete_phase(
        world,
        "audit-task",
        Some(&task_id),
        "terminal task audit recheck passed",
    );
}

#[when("an investigation link references missing attempt provenance")]
fn when_investigation_link_references_missing_attempt_provenance(world: &mut BehaviorWorld) {
    let finding_id = add_audit_finding(world, None, "audit-spec");
    run_lifecycle_fail(
        world,
        "investigate",
        &["investigation", "link-root-cause"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "attempt_id": "00000000-0000-0000-0000-00000000a001",
            "root_cause_id": "00000000-0000-0000-0000-00000000b001",
            "finding_id": finding_id,
            "source_check": source_check("audit-spec", &json!({"scope": "spec"}))
        }),
    );
}

#[when("task-scoped investigate tries to create a task")]
fn when_task_scoped_investigate_tries_to_create_task(world: &mut BehaviorWorld) {
    let task_id = create_implemented_lifecycle_task(world);
    run_lifecycle_fail(
        world,
        "investigate",
        &["task", "create"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "title": "Invalid remediation task",
            "description": "Task-scoped investigate must not create this.",
            "origin": {
                "kind": "investigation",
                "source_phase": "audit-task",
                "source_task": task_id,
                "loop_index": 1
            },
            "acceptance_criteria": []
        }),
    );
}

#[when("spec-scoped investigate creates a follow-up task")]
fn when_spec_scoped_investigate_creates_follow_up_task(world: &mut BehaviorWorld) {
    let finding_id = add_audit_finding(world, None, "audit-spec");
    let task_response = run_lifecycle_json(
        world,
        "investigate",
        &["task", "create"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "title": "Spec investigation follow-up",
            "description": "Address a genuinely spec-scoped audit finding.",
            "origin": {
                "kind": "spec_investigation",
                "source_phase": "audit-spec",
                "source_finding": finding_id
            },
            "acceptance_criteria": []
        }),
    );
    world.lifecycle_task_id = Some(string_field(&task_response, "task_id"));
    world.lifecycle_finding_id = Some(finding_id);
}

#[then("spec status routes task-scoped blockers to task checks")]
fn then_spec_status_routes_task_scoped_blockers_to_task_checks(world: &mut BehaviorWorld) {
    let status = spec_status(world);
    assert_eq!(status["ready_for_walk_spec"], false, "{status}");
    assert_eq!(status["next_transition"], "task_check_batch", "{status}");
    assert_eq!(status["next_task_id"], lifecycle_task_id(world), "{status}");
    assert!(
        status["pending_task_checks"]
            .as_array()
            .expect("pending task checks")
            .iter()
            .any(|guard| guard == "audited"),
        "{status}"
    );
}

#[then("spec status routes spec-scoped blockers to spec checks")]
fn then_spec_status_routes_spec_scoped_blockers_to_spec_checks(world: &mut BehaviorWorld) {
    let status = spec_status(world);
    assert_eq!(status["ready_for_walk_spec"], false, "{status}");
    assert_eq!(status["next_transition"], "spec_check_batch", "{status}");
    assert_eq!(status["next_task_id"], serde_json::Value::Null, "{status}");
    assert!(
        status["pending_spec_checks"]
            .as_array()
            .expect("pending spec checks")
            .iter()
            .any(|check| check == "audit_spec"),
        "{status}"
    );
}

#[then("audit-task can report complete for the completed task")]
fn then_audit_task_can_report_complete_for_completed_task(world: &mut BehaviorWorld) {
    let output = world.installer_output.as_ref().expect("audit-task output");
    assert_eq!(output.status, Some(0), "{}", output.stderr);
    let open = list_open_fix_now(world);
    assert_eq!(
        open["findings"].as_array().expect("findings array").len(),
        0
    );
}

#[then("investigation provenance linking is rejected")]
fn then_investigation_provenance_linking_is_rejected(world: &mut BehaviorWorld) {
    let output = world
        .installer_output
        .as_ref()
        .expect("investigation output");
    assert_ne!(output.status, Some(0));
    assert!(output.stderr.contains("attempt_id"), "{}", output.stderr);
}

#[then("task creation from task-scoped investigate is rejected")]
fn then_task_creation_from_task_scoped_investigate_is_rejected(world: &mut BehaviorWorld) {
    let output = world.installer_output.as_ref().expect("investigate output");
    assert_ne!(output.status, Some(0));
    assert!(output.stderr.contains("spec-scoped"), "{}", output.stderr);
}

#[then("the investigation follow-up task is created")]
fn then_investigation_follow_up_task_is_created(world: &mut BehaviorWorld) {
    let status = spec_status(world);
    assert_eq!(status["next_task_id"], lifecycle_task_id(world), "{status}");
    assert_eq!(status["next_transition"], "task_do", "{status}");
}

#[given("the phase0 orchestrator source")]
fn given_phase0_orchestrator_source(_world: &mut BehaviorWorld) {}

#[then("phase0 investigation envelopes include source findings and prior attempts")]
fn then_phase0_envelopes_include_source_findings_and_prior_attempts(_world: &mut BehaviorWorld) {
    let source =
        fs::read_to_string(BehaviorWorld::workspace_root().join("scripts/orchestration/phase0.sh"))
            .expect("read phase0 orchestrator");
    assert!(source.contains("open_finding_ids_json"), "{source}");
    assert!(source.contains("prior_attempts_json"), "{source}");
    assert!(source.contains("finding list"), "{source}");
    assert!(source.contains("investigation list-attempts"), "{source}");
    assert!(
        source.contains("source_finding_ids: $source_finding_ids"),
        "{source}"
    );
    assert!(
        source.contains("prior_attempts: $prior_attempts"),
        "{source}"
    );
    assert!(
        source.contains("pending_spec_checks[]?"),
        "phase0 spec batches must derive work from pending_spec_checks:\n{source}"
    );
    assert!(
        source.contains("phase0-${RUN_STAMP}-${CYCLE}-spec-gate-complete"),
        "phase0 spec-gate outcomes need fresh per-cycle idempotency:\n{source}"
    );
    assert!(
        !source.contains("run_hook \"run_demo_hook\""),
        "run-demo harness phase must not be followed by a duplicate hook:\n{source}"
    );
    assert!(
        !source.contains("run_hook \"audit_spec_hook\""),
        "audit-spec harness phase must not be followed by a duplicate hook:\n{source}"
    );
    assert!(
        !source.contains("run_hook \"adhere_spec_hook\""),
        "adhere-spec harness phase must not be followed by a duplicate hook:\n{source}"
    );
}
