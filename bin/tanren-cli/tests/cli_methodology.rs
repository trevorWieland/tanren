//! Integration tests for the `tanren-cli methodology …` tool surface.

#[path = "support/methodology_test_support.rs"]
mod methodology_test_support;

use methodology_test_support::{
    cli, mk_spec_folder, mkdb, parse_stderr, parse_stdout, write_legacy_phase_events_file,
    write_phase_events_file,
};
use std::path::PathBuf;
use tanren_domain::SpecId;

#[test]
fn task_create_then_list_round_trips() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000011";
    let spec_folder = mk_spec_folder(&d, spec);

    let out = cli(&url)
        .args([
            "methodology",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"title\":\"T\",\"description\":\"\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[]}}"
            ),
        ])
        .output()
        .expect("cli");
    assert!(out.status.success(), "create_task should succeed");
    let v = parse_stdout(&out);
    assert!(
        v.get("task_id").is_some(),
        "create_task response must carry task_id"
    );

    let out = cli(&url)
        .args([
            "methodology",
            "task",
            "list",
            "--json",
            &format!("{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\"}}"),
        ])
        .output()
        .expect("cli");
    assert!(out.status.success(), "list_tasks should succeed");
    let arr = parse_stdout(&out);
    assert_eq!(arr["schema_version"].as_str(), Some("1.0.0"));
    let list = arr["tasks"].as_array().expect("list tasks is array");
    assert_eq!(list.len(), 1, "should see the created task");
    assert_eq!(list[0]["title"].as_str(), Some("T"));
}

#[test]
fn validation_error_returns_exit_4_with_typed_field_path() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000012";
    let spec_folder = mk_spec_folder(&d, spec);

    let out = cli(&url)
        .args([
            "methodology",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"title\":\"\",\"description\":\"\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[]}}"
            ),
        ])
        .output()
        .expect("cli");
    assert_eq!(
        out.status.code(),
        Some(4),
        "validation error → exit 4 per install-targets.md"
    );
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
    assert_eq!(err["field_path"].as_str(), Some("/title"));
}

#[test]
fn unknown_tool_json_returns_typed_validation() {
    let (d, url) = mkdb();
    let spec_folder = mk_spec_folder(&d, "00000000-0000-0000-0000-000000000020");
    // Provide malformed JSON — the CLI should surface a
    // validation_failed ToolError pointing at the argument payload.
    let out = cli(&url)
        .args([
            "methodology",
            "--spec-id",
            "00000000-0000-0000-0000-000000000020",
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "create",
            "--json",
            "not json",
        ])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
}

#[test]
fn capability_enforcement_denies_when_env_scope_excludes_tool() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000013";
    let spec_folder = mk_spec_folder(&d, spec);

    // Scope permits only task.read — `create_task` must be denied
    // with a typed CapabilityDenied error.
    let out = cli(&url)
        .env("TANREN_PHASE_CAPABILITIES", "task.read")
        .args([
            "methodology",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"title\":\"T\",\"description\":\"\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[]}}"
            ),
        ])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("capability_denied"));
    assert_eq!(err["capability"].as_str(), Some("task.create"));
}

#[test]
fn capability_scope_rejects_unknown_env_tags() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000113";
    let spec_folder = mk_spec_folder(&d, spec);
    let out = cli(&url)
        .env("TANREN_PHASE_CAPABILITIES", "task.read,unknown.tag")
        .args([
            "methodology",
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "list",
            "--json",
            &format!("{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\"}}"),
        ])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
    assert_eq!(
        err["field_path"].as_str(),
        Some("/TANREN_PHASE_CAPABILITIES")
    );
}

#[test]
fn adherence_rejects_non_contract_severity_at_boundary() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000213";
    let spec_folder = mk_spec_folder(&d, spec);
    let out = cli(&url)
        .args([
            "methodology",
            "--phase",
            "adhere-task",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "adherence",
            "add-finding",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"standard\":{{\"name\":\"no-unwrap-in-production\",\"category\":\"rust-error-handling\"}},\"affected_files\":[],\"line_numbers\":[],\"severity\":\"note\",\"rationale\":\"bad\"}}"
            ),
        ])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
}

#[test]
fn create_issue_returns_urn_no_placeholder_url() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000014";
    let spec_folder = mk_spec_folder(&d, spec);
    let out = cli(&url)
        .args([
            "methodology",
            "--phase",
            "triage-audits",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "issue",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"origin_spec_id\":\"{spec}\",\"title\":\"fix stale doc\",\"description\":\"\",\"suggested_spec_scope\":\"docs\",\"priority\":\"low\"}}"
            ),
        ])
        .output()
        .expect("cli");
    assert!(out.status.success(), "create_issue should succeed");
    let v = parse_stdout(&out);
    let url_str = v["reference"]["url"].as_str().unwrap_or_default();
    assert!(
        url_str.starts_with("urn:tanren:issue:"),
        "expected URN, got {url_str}"
    );
    assert!(
        !url_str.contains("example.invalid"),
        "placeholder URL must not appear"
    );
}

#[test]
fn replay_missing_file_returns_not_found() {
    let (_d, url) = mkdb();
    let empty = tempfile::tempdir().expect("tempdir");
    let out = cli(&url)
        .args([
            "methodology",
            "replay",
            empty.path().to_str().expect("path"),
        ])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("not_found"));
    assert_eq!(err["resource"].as_str(), Some("phase-events.jsonl"));
}

#[test]
fn ingest_phase_events_does_not_require_spec_folder() {
    let (d, url) = mkdb();
    let spec_id = SpecId::new();
    let file = write_phase_events_file(d.path(), spec_id);
    let out = cli(&url)
        .args([
            "methodology",
            "ingest-phase-events",
            file.to_str().expect("utf8"),
        ])
        .output()
        .expect("cli");
    assert!(
        out.status.success(),
        "ingest-phase-events should run without --spec-folder: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn replay_does_not_require_spec_folder() {
    let (d, url) = mkdb();
    let spec_id = SpecId::new();
    let spec_folder = mk_spec_folder(&d, &spec_id.to_string());
    let _path = write_phase_events_file(&spec_folder, spec_id);
    let out = cli(&url)
        .args(["methodology", "replay", spec_folder.to_str().expect("utf8")])
        .output()
        .expect("cli");
    assert!(
        out.status.success(),
        "replay should run without --spec-folder: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ingest_phase_events_strict_provenance_rejects_legacy_lines() {
    let (d, url) = mkdb();
    let spec_id = SpecId::new();
    let file = write_legacy_phase_events_file(d.path(), spec_id);
    let out = cli(&url)
        .args([
            "methodology",
            "ingest-phase-events",
            file.to_str().expect("utf8"),
        ])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(6));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
}

#[test]
fn replay_round_trips_real_generated_phase_events_file() {
    let (source_dir, source_url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000031";
    let spec_folder = mk_spec_folder(&source_dir, spec);

    let create = cli(&source_url)
        .args([
            "methodology",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"title\":\"Replay Me\",\"description\":\"\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[]}}"
            ),
        ])
        .output()
        .expect("create");
    assert!(
        create.status.success(),
        "seed create failed: {}",
        String::from_utf8_lossy(&create.stderr)
    );
    let phase_events = spec_folder.join("phase-events.jsonl");
    assert!(
        phase_events.exists(),
        "phase-events.jsonl must be generated"
    );

    let (_target_dir, target_url) = mkdb();
    let replay = cli(&target_url)
        .args(["methodology", "replay", spec_folder.to_str().expect("utf8")])
        .output()
        .expect("replay");
    assert!(
        replay.status.success(),
        "replay failed: {}",
        String::from_utf8_lossy(&replay.stderr)
    );

    let list = cli(&target_url)
        .args([
            "methodology",
            "task",
            "list",
            "--json",
            &format!("{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\"}}"),
        ])
        .output()
        .expect("list");
    assert!(list.status.success(), "list failed after replay");
    let tasks = parse_stdout(&list);
    assert_eq!(tasks["schema_version"].as_str(), Some("1.0.0"));
    let arr = tasks["tasks"].as_array().expect("tasks array");
    assert_eq!(arr.len(), 1, "replayed store must contain one task");
    assert_eq!(arr[0]["title"].as_str(), Some("Replay Me"));
}

#[test]
fn list_standards_returns_nonempty_baseline() {
    let (_d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000015";
    let out = cli(&url)
        .args([
            "methodology",
            "standard",
            "list",
            "--json",
            &format!("{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\"}}"),
        ])
        .output()
        .expect("cli");
    assert!(out.status.success());
    let arr = parse_stdout(&out);
    assert_eq!(arr["schema_version"].as_str(), Some("1.0.0"));
    let list = arr["standards"].as_array().expect("standards is array");
    assert!(
        !list.is_empty(),
        "baseline standards registry must not be empty (F3 fix)"
    );
}

#[test]
fn abandon_rejects_empty_replacements_and_trivial_reason() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000016";
    let spec_folder = mk_spec_folder(&d, spec);
    let create = cli(&url)
        .args([
            "methodology",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"title\":\"X\",\"description\":\"\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[]}}"
            ),
        ])
        .output()
        .expect("cli");
    let task_id = parse_stdout(&create)["task_id"]
        .as_str()
        .expect("task_id string")
        .to_owned();
    let out = cli(&url)
        .args([
            "methodology",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "abandon",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"reason\":\"no\",\"disposition\":\"replacement\",\"replacements\":[]}}"
            ),
        ])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
    assert_eq!(err["field_path"].as_str(), Some("/replacements"));
}

#[test]
fn cargo_bin_exists() {
    // Sanity: assert the binary we're shelling out to actually exists
    // so we never accidentally trust a stale cached output.
    let path: PathBuf = assert_cmd::cargo::cargo_bin("tanren-cli");
    assert!(path.exists(), "tanren bin at {path:?}");
}
