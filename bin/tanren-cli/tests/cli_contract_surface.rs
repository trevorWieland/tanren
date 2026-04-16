//! Additional CLI contract-surface coverage.

mod support;

use serde_json::Value;
use support::auth::{add_auth_args, auth_harness, auth_harness_with_org, cli, temp_db};
use uuid::Uuid;

#[test]
fn create_accepts_contract_surface_fields_and_get_reflects_them() {
    let (db_url, _dir) = temp_db();
    let org_id = Uuid::now_v7();
    let auth = auth_harness_with_org(org_id);
    support::auth::lint_anchor(&auth);
    let mut create_cmd = cli();
    create_cmd.args([
        "--database-url",
        &db_url,
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
        "--auth-mode",
        "oauth",
        "--gate-cmd",
        "cargo test",
        "--context",
        "audit this change",
        "--model",
        "claude-4",
        "--project-env",
        "API_URL=https://example.com",
        "--project-env",
        "BUILD_TAG=v1",
        "--required-secret",
        "OPENAI_API_KEY",
        "--required-secret",
        "GITHUB_TOKEN",
        "--preserve-on-failure",
    ]);
    add_auth_args(&mut create_cmd, &auth);
    let create_output = create_cmd.output().expect("execute");
    assert!(
        create_output.status.success(),
        "create should succeed. stderr: {}",
        String::from_utf8_lossy(&create_output.stderr)
    );
    let create_json: Value =
        serde_json::from_str(&String::from_utf8(create_output.stdout).expect("utf8"))
            .expect("json");
    let dispatch_id = create_json["dispatch_id"].as_str().expect("dispatch_id");

    let mut get_cmd = cli();
    get_cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "get",
        "--id",
        dispatch_id,
    ]);
    let read_auth = auth_harness_with_org(org_id);
    add_auth_args(&mut get_cmd, &read_auth);
    let get_output = get_cmd.output().expect("execute");
    assert!(get_output.status.success(), "get should succeed");
    let get_json: Value =
        serde_json::from_str(&String::from_utf8(get_output.stdout).expect("utf8")).expect("json");
    assert_eq!(get_json["auth_mode"], "oauth");
    assert_eq!(get_json["gate_cmd"], "cargo test");
    assert_eq!(get_json["context"], "audit this change");
    assert_eq!(get_json["model"], "claude-4");
    assert_eq!(get_json["preserve_on_failure"], true);
    assert_eq!(
        get_json["required_secrets"],
        serde_json::json!(["OPENAI_API_KEY", "GITHUB_TOKEN"])
    );
    assert_eq!(
        get_json["project_env_keys"],
        serde_json::json!(["API_URL", "BUILD_TAG"])
    );
}

#[test]
fn create_rejects_invalid_secret_names() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
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
        "--required-secret",
        "bad-secret",
    ]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "invalid secret names must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
}

#[test]
fn create_rejects_duplicate_project_env_keys() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
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
        "--project-env",
        "DUP=one",
        "--project-env",
        "DUP=two",
    ]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "duplicate keys must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
}

#[test]
fn list_rejects_invalid_cursor_token() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();

    let mut create_cmd = cli();
    create_cmd.args([
        "--database-url",
        &db_url,
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
    add_auth_args(&mut create_cmd, &auth);
    let create_output = create_cmd.output().expect("execute");
    assert!(
        create_output.status.success(),
        "bootstrap create should succeed"
    );

    let mut list_cmd = cli();
    list_cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "list",
        "--cursor",
        "v9|bad",
    ]);
    add_auth_args(&mut list_cmd, &auth);

    let output = list_cmd.output().expect("execute");
    assert!(!output.status.success(), "invalid cursor must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
}

#[test]
fn dispatch_create_missing_required_flag_yields_canonical_invalid_args_wire_shape() {
    // No `--project` → clap emits MissingRequiredArgument; the CLI
    // must map that to the stable `invalid_args` wire shape with an
    // allowlisted field and a machine-safe reason code. The raw clap
    // message (which varies by version) must not leak into the wire.
    let mut cmd = cli();
    cmd.args([
        "dispatch",
        "create",
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
    let output = cmd.output().expect("execute");
    assert!(
        !output.status.success(),
        "missing --project must fail. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert_eq!(v["message"], "invalid cli args");
    assert_eq!(v["details"]["type"], "invalid_args");
    assert_eq!(v["details"]["reason_code"], "missing_required_argument");
    // Field may or may not be populated depending on clap context
    // availability. If present, it must be the allowlisted
    // snake_case name — never raw user text.
    if let Some(field) = v["details"]["field"].as_str() {
        assert_eq!(field, "project");
    }
}

#[test]
fn dispatch_create_invalid_value_does_not_echo_raw_input_to_wire() {
    // Craft a phase value that looks like a secret. The mapper must
    // not echo the raw string into `message` or `details`.
    let mut cmd = cli();
    cmd.args([
        "dispatch",
        "create",
        "--project",
        "p",
        "--phase",
        "sk-secret-looking-value",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
    ]);
    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "invalid phase value must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert_eq!(v["message"], "invalid cli args");
    assert_eq!(v["details"]["type"], "invalid_args");
    assert_eq!(v["details"]["reason_code"], "invalid_value");
    assert!(
        !stderr.contains("sk-secret-looking-value"),
        "raw user value leaked into wire: {stderr}"
    );
}
