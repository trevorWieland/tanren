//! CLI integration tests — invoke the actual `tanren-cli` binary.
//!
//! These tests verify clap parsing, composition-root wiring, stdout/stderr
//! JSON shape, security-bound actor token handling, and exit codes.

mod support;

use serde_json::Value;
use support::auth::{
    add_auth_args, assert_stderr_is_single_json, auth_harness, auth_harness_with_org, cli, temp_db,
};
use uuid::Uuid;

/// Invoke `dispatch create` with standard arguments and return stdout.
fn create_dispatch(db_url: &str, auth: &support::auth::AuthHarness) -> String {
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        db_url,
        "dispatch",
        "create",
        "--project",
        "test-project",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
    ]);
    add_auth_args(&mut cmd, auth);

    let output = cmd.output().expect("execute");

    assert!(
        output.status.success(),
        "create should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8")
}

#[test]
fn create_outputs_valid_json_and_exits_0() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    support::auth::lint_anchor(&auth);
    let stdout = create_dispatch(&db_url, &auth);

    let v: Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(
        v["dispatch_id"].is_string(),
        "dispatch_id should be a string"
    );
    assert_eq!(v["status"], "pending");
    assert_eq!(v["mode"], "manual");
    assert_eq!(v["lane"], "impl");
    assert_eq!(v["project"], "test-project");
    assert_eq!(v["phase"], "do_task");
    assert_eq!(v["cli"], "claude");
    assert_eq!(v["auth_mode"], "api_key");
}

#[test]
fn get_nonexistent_outputs_not_found_json_on_stderr_and_exits_1() {
    let (db_url, _dir) = temp_db();
    let create_auth = auth_harness();

    let _ = create_dispatch(&db_url, &create_auth);
    let read_auth = auth_harness();

    let fake_id = Uuid::now_v7();
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "get",
        "--id",
        &fake_id.to_string(),
    ]);
    add_auth_args(&mut cmd, &read_auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail with non-zero exit");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("stderr should be valid JSON");
    assert_eq!(v["code"], "not_found");
}

#[test]
fn get_unauthorized_dispatch_is_hidden_as_not_found() {
    let (db_url, _dir) = temp_db();
    let create_auth = auth_harness_with_org(Uuid::now_v7());
    let stdout = create_dispatch(&db_url, &create_auth);
    let created: Value = serde_json::from_str(&stdout).expect("json");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    let read_auth = auth_harness_with_org(Uuid::now_v7());
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "get",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut cmd, &read_auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "unauthorized read should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "not_found");
}

#[test]
fn cancel_unauthorized_dispatch_is_hidden_as_not_found() {
    let (db_url, _dir) = temp_db();
    let create_auth = auth_harness_with_org(Uuid::now_v7());
    let stdout = create_dispatch(&db_url, &create_auth);
    let created: Value = serde_json::from_str(&stdout).expect("json");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    let cancel_auth = auth_harness_with_org(Uuid::now_v7());
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "cancel",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut cmd, &cancel_auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "unauthorized cancel should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "not_found");
}
