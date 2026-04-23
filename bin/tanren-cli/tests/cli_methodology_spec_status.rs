//! Integration coverage for `tanren-cli methodology spec status`.

use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::Value;

fn mkdb() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("tanren.db");
    let url = format!("sqlite:{}?mode=rwc", db.display());
    Command::cargo_bin("tanren-cli")
        .expect("bin")
        .args(["--database-url", &url, "db", "migrate"])
        .assert()
        .success();
    (dir, url)
}

fn mk_spec_folder(dir: &tempfile::TempDir, spec_id: &str) -> String {
    let path = dir.path().join(format!("2026-01-01-0101-{spec_id}-test"));
    std::fs::create_dir_all(&path).expect("mkdir spec folder");
    path.to_string_lossy().to_string()
}

fn cli(url: &str) -> Command {
    let mut cmd = Command::cargo_bin("tanren-cli").expect("bin");
    cmd.args(["--database-url", url]);
    cmd.env(
        "TANREN_PHASE_CAPABILITIES",
        "task.create,task.start,task.complete,task.revise,task.abandon,task.read,finding.add,rubric.record,compliance.record,spec.frontmatter,demo.frontmatter,demo.results,signpost.add,signpost.update,phase.outcome,phase.escalate,issue.create,standard.read,adherence.record,feedback.reply",
    );
    cmd
}

fn parse_stdout(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).expect("stdout json")
}

fn spec_status(url: &str, spec: &str) -> Value {
    let out = cli(url)
        .args([
            "methodology",
            "--phase",
            "do-task",
            "spec",
            "status",
            "--json",
            &format!("{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\"}}"),
        ])
        .output()
        .expect("spec status");
    assert!(
        out.status.success(),
        "spec status failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    parse_stdout(&out)
}

fn create_user_task(url: &str, spec: &str, spec_folder: &str) -> String {
    let out = cli(url)
        .args([
            "methodology",
            "--phase",
            "shape-spec",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder,
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"title\":\"T\",\"description\":\"\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[]}}"
            ),
        ])
        .output()
        .expect("create_task");
    assert!(
        out.status.success(),
        "create_task failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    parse_stdout(&out)["task_id"]
        .as_str()
        .expect("task_id")
        .to_owned()
}

fn run_task_mutation(
    url: &str,
    spec: &str,
    spec_folder: &str,
    phase: &str,
    payload: &str,
    verb: &str,
) {
    let out = cli(url)
        .args([
            "methodology",
            "--phase",
            phase,
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder,
            "task",
            verb,
            "--json",
            payload,
        ])
        .output()
        .expect("task mutation");
    assert!(
        out.status.success(),
        "{phase} task {verb} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn report_phase_outcome(url: &str, spec: &str, spec_folder: &str, phase: &str, outcome_json: &str) {
    let out = cli(url)
        .args([
            "methodology",
            "--phase",
            phase,
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder,
            "phase",
            "outcome",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"outcome\":{outcome_json}}}"
            ),
        ])
        .output()
        .expect("phase outcome");
    assert!(
        out.status.success(),
        "{phase} phase outcome failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn assert_task_step_progression(url: &str, spec: &str, spec_folder: &str, task_id: &str) {
    let active = spec_status(url, spec);
    assert_eq!(active["next_action"].as_str(), Some("run_loop"));
    assert_eq!(active["next_task_id"].as_str(), Some(task_id));
    assert_eq!(active["next_step"].as_str(), Some("task_do_task"));

    run_task_mutation(
        url,
        spec,
        spec_folder,
        "do-task",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\"}}"),
        "start",
    );
    run_task_mutation(
        url,
        spec,
        spec_folder,
        "do-task",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"evidence_refs\":[]}}"),
        "complete",
    );

    let implemented = spec_status(url, spec);
    assert_eq!(implemented["next_step"].as_str(), Some("task_gate"));
    assert_eq!(
        implemented["pending_required_guards"]
            .as_array()
            .expect("guards")
            .iter()
            .map(|v| v.as_str().expect("str"))
            .collect::<Vec<_>>(),
        vec!["gate_checked", "audited", "adherent"]
    );

    run_task_mutation(
        url,
        spec,
        spec_folder,
        "do-task",
        &format!(
            "{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"guard\":\"gate_checked\"}}"
        ),
        "guard",
    );
    assert_eq!(
        spec_status(url, spec)["next_step"].as_str(),
        Some("task_audit")
    );

    run_task_mutation(
        url,
        spec,
        spec_folder,
        "audit-task",
        &format!(
            "{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"guard\":\"audited\"}}"
        ),
        "guard",
    );
    assert_eq!(
        spec_status(url, spec)["next_step"].as_str(),
        Some("task_adhere")
    );
}

#[test]
fn spec_status_routes_shape_blocker_walk_and_complete_states() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-0000000000aa";
    let spec_folder = mk_spec_folder(&d, spec);

    let missing = spec_status(&url, spec);
    assert_eq!(missing["spec_exists"].as_bool(), Some(false));
    assert_eq!(missing["next_action"].as_str(), Some("shape_spec_required"));
    assert!(missing.get("next_step").is_none());

    let task_id = create_user_task(&url, spec, &spec_folder);
    assert_task_step_progression(&url, spec, &spec_folder, &task_id);

    run_task_mutation(
        &url,
        spec,
        &spec_folder,
        "adhere-task",
        &format!(
            "{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"guard\":\"adherent\"}}"
        ),
        "guard",
    );

    let walk_ready = spec_status(&url, spec);
    assert_eq!(
        walk_ready["next_action"].as_str(),
        Some("walk_spec_required")
    );
    assert_eq!(walk_ready["ready_for_walk_spec"].as_bool(), Some(true));

    report_phase_outcome(
        &url,
        spec,
        &spec_folder,
        "investigate",
        "{\"outcome\":\"blocked\",\"reason\":{\"kind\":\"awaiting_human_input\",\"prompt\":\"reason: needs user\\noptions:\\n- retry\\n- defer\"},\"summary\":\"blocked\"}",
    );
    let blocked = spec_status(&url, spec);
    assert_eq!(
        blocked["next_action"].as_str(),
        Some("resolve_blockers_required")
    );
    assert_eq!(
        blocked["last_blocker_reason_kind"].as_str(),
        Some("awaiting_human_input")
    );
    assert_eq!(
        blocked["last_blocker_options"].as_array().map(Vec::len),
        Some(2)
    );

    report_phase_outcome(
        &url,
        spec,
        &spec_folder,
        "resolve-blockers",
        "{\"outcome\":\"complete\",\"summary\":\"resolved\"}",
    );
    report_phase_outcome(
        &url,
        spec,
        &spec_folder,
        "walk-spec",
        "{\"outcome\":\"complete\",\"summary\":\"walked\"}",
    );

    let done = spec_status(&url, spec);
    assert_eq!(done["next_action"].as_str(), Some("complete"));
}

#[test]
fn spec_status_routes_task_blocked_to_task_investigate() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-0000000000ab";
    let spec_folder = mk_spec_folder(&d, spec);
    let task_id = create_user_task(&url, spec, &spec_folder);

    run_task_mutation(
        &url,
        spec,
        &spec_folder,
        "do-task",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\"}}"),
        "start",
    );
    run_task_mutation(
        &url,
        spec,
        &spec_folder,
        "do-task",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"evidence_refs\":[]}}"),
        "complete",
    );

    report_phase_outcome(
        &url,
        spec,
        &spec_folder,
        "audit-task",
        "{\"outcome\":\"blocked\",\"reason\":{\"kind\":\"other\",\"detail\":\"fix now\"},\"summary\":\"audit blocked\"}",
    );

    let blocked = spec_status(&url, spec);
    assert_eq!(blocked["next_action"].as_str(), Some("run_loop"));
    assert_eq!(blocked["next_task_id"].as_str(), Some(task_id.as_str()));
    assert_eq!(blocked["next_step"].as_str(), Some("task_investigate"));
    assert_eq!(
        blocked["investigate_source_phase"].as_str(),
        Some("audit-task")
    );
    assert_eq!(
        blocked["investigate_source_outcome"].as_str(),
        Some("blocked")
    );
    assert_eq!(
        blocked["investigate_source_task_id"].as_str(),
        Some(task_id.as_str())
    );
    assert_eq!(blocked["blockers_active"].as_bool(), Some(false));
}

#[test]
fn spec_status_routes_task_investigate_completion_back_to_do_task() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-0000000000ac";
    let spec_folder = mk_spec_folder(&d, spec);
    let task_id = create_user_task(&url, spec, &spec_folder);

    run_task_mutation(
        &url,
        spec,
        &spec_folder,
        "do-task",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\"}}"),
        "start",
    );
    run_task_mutation(
        &url,
        spec,
        &spec_folder,
        "do-task",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"evidence_refs\":[]}}"),
        "complete",
    );
    run_task_mutation(
        &url,
        spec,
        &spec_folder,
        "do-task",
        &format!(
            "{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"guard\":\"gate_checked\"}}"
        ),
        "guard",
    );

    report_phase_outcome(
        &url,
        spec,
        &spec_folder,
        "audit-task",
        "{\"outcome\":\"blocked\",\"reason\":{\"kind\":\"other\",\"detail\":\"fix now\"},\"summary\":\"audit blocked\"}",
    );
    assert_eq!(
        spec_status(&url, spec)["next_step"].as_str(),
        Some("task_investigate")
    );

    report_phase_outcome(
        &url,
        spec,
        &spec_folder,
        "investigate",
        "{\"outcome\":\"complete\",\"summary\":\"investigate complete\"}",
    );

    let recovered = spec_status(&url, spec);
    assert_eq!(recovered["next_action"].as_str(), Some("run_loop"));
    assert_eq!(recovered["next_task_id"].as_str(), Some(task_id.as_str()));
    assert_eq!(recovered["next_step"].as_str(), Some("task_do_task"));
    assert_eq!(
        recovered["next_step_reason"].as_str(),
        Some("investigate completed for latest blocked outcome in audit-task; rerun do-task")
    );
}
