use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::world::BehaviorWorld;

pub(super) const LIFECYCLE_SPEC_ID: &str = "00000000-0000-0000-0000-000000008080";
pub(super) const OTHER_SPEC_ID: &str = "00000000-0000-0000-0000-000000008081";

static PHASE_OUTCOME_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) fn add_audit_finding(
    world: &mut BehaviorWorld,
    task_id: Option<&str>,
    phase: &str,
) -> String {
    let response = run_lifecycle_json(
        world,
        phase,
        &["finding", "add"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "severity": "fix_now",
            "title": "Blocking audit finding",
            "description": "The implementation does not satisfy the audit check.",
            "affected_files": ["src/lib.rs"],
            "line_numbers": [12],
            "source": {"kind": "audit", "phase": phase, "pillar": "correctness"},
            "attached_task": task_id
        }),
    );
    string_field(&response, "finding_id")
}

pub(super) fn start_audit_check(world: &mut BehaviorWorld, phase: &str, scope: &Value) -> String {
    let response = run_lifecycle_json(
        world,
        phase,
        &["check", "start"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "kind": {"kind": "audit"},
            "scope": scope.clone(),
            "fingerprint": "audit:finding-lifecycle"
        }),
    );
    string_field(&response, "check_run_id")
}

pub(super) fn complete_phase(
    world: &mut BehaviorWorld,
    phase: &str,
    task_id: Option<&str>,
    summary: &str,
) {
    let sequence = PHASE_OUTCOME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    run_lifecycle_ok(
        world,
        phase,
        &["phase", "outcome"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "task_id": task_id,
            "idempotency_key": format!("bdd-phase-outcome-{phase}-{sequence}"),
            "outcome": {"outcome": "complete", "summary": summary}
        }),
    );
}

pub(super) fn block_phase(
    world: &mut BehaviorWorld,
    phase: &str,
    task_id: Option<&str>,
    summary: &str,
) {
    let sequence = PHASE_OUTCOME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    run_lifecycle_ok(
        world,
        phase,
        &["phase", "outcome"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "task_id": task_id,
            "idempotency_key": format!("bdd-phase-outcome-{phase}-blocked-{sequence}"),
            "outcome": {
                "outcome": "blocked",
                "summary": summary,
                "reason": {
                    "kind": "other",
                    "detail": "task check failed"
                }
            }
        }),
    );
}

pub(super) fn complete_all_spec_checks(world: &mut BehaviorWorld) {
    complete_phase(world, "spec-gate", None, "spec gate passed");
    complete_phase(world, "run-demo", None, "demo passed");
    complete_phase(world, "audit-spec", None, "audit spec passed");
    complete_phase(world, "adhere-spec", None, "adherence spec passed");
}

pub(super) fn create_completed_lifecycle_task(world: &mut BehaviorWorld) -> String {
    let task_id = create_implemented_lifecycle_task(world);
    complete_phase(world, "audit-task", Some(&task_id), "audit task passed");
    complete_phase(
        world,
        "adhere-task",
        Some(&task_id),
        "adherence task passed",
    );
    task_id
}

pub(super) fn create_implemented_lifecycle_task(world: &mut BehaviorWorld) -> String {
    let task_response = run_lifecycle_json(
        world,
        "shape-spec",
        &["task", "create"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "title": "Task with lifecycle checks",
            "description": "A task that can be rechecked by lifecycle phases.",
            "origin": {"kind": "shape_spec"},
            "acceptance_criteria": []
        }),
    );
    let task_id = string_field(&task_response, "task_id");
    run_lifecycle_ok(
        world,
        "do-task",
        &["task", "start"],
        &json!({"schema_version": "1.0.0", "task_id": task_id}),
    );
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
    task_id
}

pub(super) fn lifecycle_evidence(world: &BehaviorWorld, summary: &str) -> Value {
    json!({
        "summary": summary,
        "evidence_refs": ["audit.log"],
        "check_run_id": world.lifecycle_check_run_id.as_ref().expect("check run id"),
        "check_kind": {"kind": "audit"}
    })
}

pub(super) fn source_check(phase: &str, scope: &Value) -> Value {
    json!({"phase": phase, "kind": {"kind": "audit"}, "scope": scope.clone()})
}

pub(super) fn list_open_fix_now(world: &mut BehaviorWorld) -> Value {
    run_lifecycle_json(
        world,
        "audit-spec",
        &["finding", "list"],
        &json!({
            "schema_version": "1.0.0",
            "spec_id": LIFECYCLE_SPEC_ID,
            "status": "open",
            "severity": "fix_now"
        }),
    )
}

pub(super) fn spec_status(world: &mut BehaviorWorld) -> Value {
    run_lifecycle_json(
        world,
        "audit-spec",
        &["spec", "status"],
        &json!({"schema_version": "1.0.0", "spec_id": LIFECYCLE_SPEC_ID}),
    )
}

pub(super) fn run_lifecycle_json(
    world: &mut BehaviorWorld,
    phase: &str,
    command: &[&str],
    payload: &Value,
) -> Value {
    run_lifecycle_ok(world, phase, command, payload);
    let output = world.installer_output.as_ref().expect("command output");
    serde_json::from_str(&output.stdout).expect("JSON command response")
}

pub(super) fn run_lifecycle_ok(
    world: &mut BehaviorWorld,
    phase: &str,
    command: &[&str],
    payload: &Value,
) {
    run_lifecycle_command(world, phase, command, payload);
    assert_success(world);
}

pub(super) fn run_lifecycle_fail(
    world: &mut BehaviorWorld,
    phase: &str,
    command: &[&str],
    payload: &Value,
) {
    run_lifecycle_command(world, phase, command, payload);
    let output = world.installer_output.as_ref();
    assert_ne!(
        output.and_then(|item| item.status),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        output.map_or("", |item| item.stdout.as_str()),
        output.map_or("", |item| item.stderr.as_str())
    );
}

fn run_lifecycle_command(
    world: &mut BehaviorWorld,
    phase: &str,
    command: &[&str],
    payload: &Value,
) {
    let mut args = vec![
        "--database-url".into(),
        world
            .lifecycle_database_url
            .as_ref()
            .expect("lifecycle database")
            .clone(),
        "methodology".into(),
        "--phase".into(),
        phase.into(),
        "--methodology-config".into(),
        path_string(world.lifecycle_config_path.as_ref().expect("config path")),
        "--spec-id".into(),
        LIFECYCLE_SPEC_ID.into(),
        "--spec-folder".into(),
        path_string(lifecycle_spec_folder(world)),
    ];
    args.extend(command.iter().map(|part| (*part).to_owned()));
    args.extend(["--json".into(), payload.to_string()]);
    world.run_cli(args, false);
}

pub(super) fn lifecycle_spec_folder(world: &BehaviorWorld) -> &Path {
    world
        .lifecycle_spec_folder
        .as_deref()
        .expect("lifecycle spec folder")
}

pub(super) fn lifecycle_task_id(world: &BehaviorWorld) -> String {
    world.lifecycle_task_id.as_ref().expect("task id").clone()
}

pub(super) fn lifecycle_finding_id(world: &BehaviorWorld) -> String {
    world
        .lifecycle_finding_id
        .as_ref()
        .expect("finding id")
        .clone()
}

pub(super) fn string_field(value: &Value, field: &str) -> String {
    value[field].as_str().expect("string field").to_owned()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn assert_success(world: &BehaviorWorld) {
    let output = world.installer_output.as_ref();
    assert_eq!(
        output.and_then(|item| item.status),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        output.map_or("", |item| item.stdout.as_str()),
        output.map_or("", |item| item.stderr.as_str())
    );
}
