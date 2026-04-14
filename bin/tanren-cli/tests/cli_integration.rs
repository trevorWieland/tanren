//! CLI integration tests — invoke the actual `tanren-cli` binary.
//!
//! These tests verify the real CLI: clap parsing, composition-root
//! wiring, stdout/stderr JSON shape, and exit codes. Each test gets
//! a fresh `SQLite` database via `tempfile`.

use assert_cmd::Command;
use serde_json::Value;
use uuid::Uuid;

/// Build a `Command` pointing at the `tanren-cli` binary.
fn cli() -> Command {
    Command::cargo_bin("tanren-cli").expect("binary should exist")
}

/// Create a temporary database URL that persists for the lifetime of the
/// returned `TempDir`.
fn temp_db() -> (String, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    (url, dir)
}

/// Invoke `dispatch create` with standard arguments and return stdout.
fn create_dispatch(db_url: &str) -> (String, Uuid, Uuid) {
    let org_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let output = cli()
        .args([
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
            "--org-id",
            &org_id.to_string(),
            "--user-id",
            &user_id.to_string(),
        ])
        .output()
        .expect("execute");

    assert!(
        output.status.success(),
        "create should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    (stdout, org_id, user_id)
}

// -- Create -----------------------------------------------------------------

#[test]
fn create_outputs_valid_json_and_exits_0() {
    let (db_url, _dir) = temp_db();
    let (stdout, _, _) = create_dispatch(&db_url);

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
}

// -- Get --------------------------------------------------------------------

#[test]
fn get_existing_dispatch_outputs_json_and_exits_0() {
    let (db_url, _dir) = temp_db();
    let (stdout, _, _) = create_dispatch(&db_url);
    let created: Value = serde_json::from_str(&stdout).expect("JSON");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "get",
            "--id",
            dispatch_id,
        ])
        .output()
        .expect("execute");

    assert!(output.status.success());
    let get_stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: Value = serde_json::from_str(&get_stdout).expect("valid JSON");
    assert_eq!(v["dispatch_id"], dispatch_id);
    assert_eq!(v["status"], "pending");
}

#[test]
fn get_nonexistent_outputs_not_found_json_on_stderr_and_exits_1() {
    let (db_url, _dir) = temp_db();
    // Create a dispatch first to ensure DB is migrated.
    let _ = create_dispatch(&db_url);

    let fake_id = Uuid::now_v7();
    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "get",
            "--id",
            &fake_id.to_string(),
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "should fail with non-zero exit");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("stderr should be valid JSON");
    assert_eq!(v["code"], "not_found");
}

// -- List -------------------------------------------------------------------

#[test]
fn list_empty_outputs_json_and_exits_0() {
    let (db_url, _dir) = temp_db();
    // Create+cancel to ensure DB is migrated, then list with a filter that
    // won't match.
    let _ = create_dispatch(&db_url);

    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "list",
            "--status",
            "running",
        ])
        .output()
        .expect("execute");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(v["dispatches"].is_array());
    assert_eq!(v["dispatches"].as_array().expect("array").len(), 0);
}

// -- Cancel -----------------------------------------------------------------

#[test]
fn cancel_dispatch_exits_0() {
    let (db_url, _dir) = temp_db();
    let (stdout, org_id, user_id) = create_dispatch(&db_url);
    let created: Value = serde_json::from_str(&stdout).expect("JSON");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "cancel",
            "--id",
            dispatch_id,
            "--org-id",
            &org_id.to_string(),
            "--user-id",
            &user_id.to_string(),
            "--reason",
            "test cancel",
        ])
        .output()
        .expect("execute");

    assert!(
        output.status.success(),
        "cancel should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// -- Validation error -------------------------------------------------------

#[test]
fn validation_error_outputs_json_on_stderr_and_exits_1() {
    let (db_url, _dir) = temp_db();
    // Ensure DB is migrated first.
    let _ = create_dispatch(&db_url);

    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "create",
            "--project",
            "",
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
            "--org-id",
            &Uuid::now_v7().to_string(),
            "--user-id",
            &Uuid::now_v7().to_string(),
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "should fail with non-zero exit");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("stderr should be valid JSON");
    assert_eq!(v["code"], "invalid_input");
}

// -- Startup failure --------------------------------------------------------

#[test]
fn startup_failure_outputs_json_on_stderr() {
    let output = cli()
        .args([
            "--database-url",
            "sqlite:/nonexistent/path/bad.db",
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "should fail with non-zero exit");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("stderr should be valid JSON");
    assert_eq!(v["code"], "internal");
}

// -- Full lifecycle ---------------------------------------------------------

#[test]
fn full_lifecycle_create_get_list_cancel() {
    let (db_url, _dir) = temp_db();
    let (stdout, org_id, user_id) = create_dispatch(&db_url);
    let created: Value = serde_json::from_str(&stdout).expect("JSON");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    // Get
    let get_output = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "get",
            "--id",
            dispatch_id,
        ])
        .output()
        .expect("execute");
    assert!(get_output.status.success());
    let get_json: Value =
        serde_json::from_str(&String::from_utf8(get_output.stdout).expect("utf8")).expect("JSON");
    assert_eq!(get_json["dispatch_id"], dispatch_id);
    assert_eq!(get_json["status"], "pending");

    // List
    let list_output = cli()
        .args(["--database-url", &db_url, "dispatch", "list"])
        .output()
        .expect("execute");
    assert!(list_output.status.success());
    let list_json: Value =
        serde_json::from_str(&String::from_utf8(list_output.stdout).expect("utf8")).expect("JSON");
    let dispatches = list_json["dispatches"].as_array().expect("array");
    assert!(
        dispatches.iter().any(|d| d["dispatch_id"] == dispatch_id),
        "list should contain the created dispatch"
    );

    // Cancel
    let cancel_output = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "cancel",
            "--id",
            dispatch_id,
            "--org-id",
            &org_id.to_string(),
            "--user-id",
            &user_id.to_string(),
        ])
        .output()
        .expect("execute");
    assert!(cancel_output.status.success());

    // Verify cancelled
    let get_cancelled = cli()
        .args([
            "--database-url",
            &db_url,
            "dispatch",
            "get",
            "--id",
            dispatch_id,
        ])
        .output()
        .expect("execute");
    assert!(get_cancelled.status.success());
    let cancelled_json: Value =
        serde_json::from_str(&String::from_utf8(get_cancelled.stdout).expect("utf8"))
            .expect("JSON");
    assert_eq!(cancelled_json["status"], "cancelled");
}
