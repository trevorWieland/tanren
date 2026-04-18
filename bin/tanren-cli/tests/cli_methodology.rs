//! Integration tests for the `tanren methodology …` tool surface.
//!
//! Covers the happy path and a representative typed-error path for
//! every §3 and §6 tool group. Each test shells out to the compiled
//! CLI with `assert_cmd`, points at a fresh sqlite file, and
//! asserts:
//! - process exit code (0 / 4 / typed),
//! - stdout JSON shape for success cases,
//! - stderr JSON shape for typed `ToolError` cases.

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn mkdb() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("tanren.db");
    let url = format!("sqlite:{}?mode=rwc", db.display());
    // Migrate once up-front so the first methodology call doesn't
    // race with schema creation.
    Command::cargo_bin("tanren-cli")
        .expect("bin")
        .args(["--database-url", &url, "db", "migrate"])
        .assert()
        .success();
    (dir, url)
}

fn cli(url: &str) -> Command {
    let mut cmd = Command::cargo_bin("tanren-cli").expect("bin");
    cmd.args(["--database-url", url]);
    // Integration tests exercise the full tool surface without
    // supplying a phase banner, so opt into the audited admin fallback.
    // Production callers invoke the CLI with explicit
    // `TANREN_PHASE_CAPABILITIES` under orchestrator dispatch; default
    // is deny.
    cmd.env("TANREN_CAPABILITY_OVERRIDE", "admin");
    cmd
}

fn parse_stdout(out: &std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&text).expect("stdout is JSON")
}

fn parse_stderr(out: &std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&out.stderr);
    serde_json::from_str(&text).expect("stderr is JSON")
}

#[test]
fn task_create_then_list_round_trips() {
    let (_d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000011";

    let out = cli(&url)
        .args([
            "methodology",
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
    let list = arr.as_array().expect("list is array");
    assert_eq!(list.len(), 1, "should see the created task");
    assert_eq!(list[0]["title"].as_str(), Some("T"));
}

#[test]
fn validation_error_returns_exit_4_with_typed_field_path() {
    let (_d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000012";

    let out = cli(&url)
        .args([
            "methodology",
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
    let (_d, url) = mkdb();
    // Provide malformed JSON — the CLI should surface a
    // validation_failed ToolError pointing at the argument payload.
    let out = cli(&url)
        .args(["methodology", "task", "create", "--json", "not json"])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
}

#[test]
fn capability_enforcement_denies_when_env_scope_excludes_tool() {
    let (_d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000013";

    // Scope permits only task.read — `create_task` must be denied
    // with a typed CapabilityDenied error.
    let out = cli(&url)
        .env("TANREN_PHASE_CAPABILITIES", "task.read")
        .args([
            "methodology",
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
fn create_issue_returns_urn_no_placeholder_url() {
    let (_d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000014";
    let out = cli(&url)
        .args([
            "methodology",
            "--phase",
            "triage-audits",
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
    let list = arr.as_array().expect("standards is array");
    assert!(
        !list.is_empty(),
        "baseline standards registry must not be empty (F3 fix)"
    );
}

#[test]
fn abandon_rejects_empty_replacements_and_trivial_reason() {
    let (_d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000016";
    let create = cli(&url)
        .args([
            "methodology",
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
            "task",
            "abandon",
            "--json",
            &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"reason\":\"no\",\"replacements\":[]}}"),
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
