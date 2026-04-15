//! CLI integration tests — invoke the actual `tanren-cli` binary.
//!
//! These tests verify clap parsing, composition-root wiring, stdout/stderr
//! JSON shape, security-bound actor token handling, and exit codes.

use std::path::PathBuf;

use assert_cmd::Command;
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

const TEST_ED25519_PRIVATE_KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIAPLmow/yTJDEVu9jxvrdcEK0yfRG0bAzr3hnOrtggLP
-----END PRIVATE KEY-----
";
const TEST_ED25519_PUBLIC_KEY_PEM: &str = "\
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEA7jO4B+xp2yKG7Rh2aMFdyIsqxEMq8jYMO7b7HEZ6vLs=
-----END PUBLIC KEY-----
";

#[derive(Debug)]
struct AuthHarness {
    issuer: String,
    audience: String,
    public_key_file: PathBuf,
    actor_token_file: PathBuf,
    _dir: tempfile::TempDir,
}

#[derive(Debug, Serialize)]
struct ActorClaims {
    iss: String,
    aud: String,
    exp: i64,
    nbf: i64,
    org_id: Uuid,
    user_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<Uuid>,
}

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

fn auth_harness() -> AuthHarness {
    auth_harness_with_claims(&ActorClaims {
        iss: "tanren-tests".to_owned(),
        aud: "tanren-cli".to_owned(),
        exp: Utc::now().timestamp() + 600,
        nbf: Utc::now().timestamp() - 60,
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    })
}

fn auth_harness_with_org(org_id: Uuid) -> AuthHarness {
    auth_harness_with_claims(&ActorClaims {
        iss: "tanren-tests".to_owned(),
        aud: "tanren-cli".to_owned(),
        exp: Utc::now().timestamp() + 600,
        nbf: Utc::now().timestamp() - 60,
        org_id,
        user_id: Uuid::now_v7(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    })
}

fn auth_harness_with_claims(claims: &ActorClaims) -> AuthHarness {
    let issuer = claims.iss.clone();
    let audience = claims.aud.clone();

    let token = encode(
        &Header::new(Algorithm::EdDSA),
        &claims,
        &EncodingKey::from_ed_pem(TEST_ED25519_PRIVATE_KEY_PEM.as_bytes()).expect("encoding key"),
    )
    .expect("token");

    let dir = tempfile::tempdir().expect("temp dir");
    let public_key_file = dir.path().join("actor-public.pem");
    std::fs::write(&public_key_file, TEST_ED25519_PUBLIC_KEY_PEM).expect("write key");
    let actor_token_file = dir.path().join("actor-token.jwt");
    std::fs::write(&actor_token_file, &token).expect("write token");

    AuthHarness {
        issuer,
        audience,
        public_key_file,
        actor_token_file,
        _dir: dir,
    }
}

fn add_auth_args(cmd: &mut Command, auth: &AuthHarness) {
    cmd.args([
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
    ]);
}

/// Invoke `dispatch create` with standard arguments and return stdout.
fn create_dispatch(db_url: &str, auth: &AuthHarness) -> String {
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

fn assert_stderr_is_single_json(stderr: &str) -> Value {
    let trimmed = stderr.trim();
    assert!(
        trimmed.starts_with('{') && trimmed.ends_with('}'),
        "stderr should contain exactly one JSON document: {stderr}"
    );
    let mut stream = serde_json::Deserializer::from_str(trimmed).into_iter::<Value>();
    let parsed = stream
        .next()
        .expect("expected one JSON value")
        .expect("stderr should be valid JSON");
    assert!(
        stream.next().is_none(),
        "stderr should contain only one JSON value"
    );
    parsed
}

#[test]
fn create_outputs_valid_json_and_exits_0() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
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
    let auth = auth_harness();

    let _ = create_dispatch(&db_url, &auth);

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
    add_auth_args(&mut cmd, &auth);

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

#[test]
fn cancel_nonexistent_dispatch_outputs_not_found_json_on_stderr() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "cancel",
        "--id",
        &Uuid::now_v7().to_string(),
    ]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "missing cancel should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "not_found");
}

#[test]
fn list_on_unmigrated_db_returns_schema_not_ready() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();

    let mut cmd = cli();
    cmd.args(["--database-url", &db_url, "dispatch", "list"]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(
        !output.status.success(),
        "list should fail without migrations"
    );
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "schema_not_ready");
}

#[test]
fn db_migrate_allows_read_commands_without_implicit_write() {
    let (db_url, _dir) = temp_db();
    let mut migrate = cli();
    migrate.args(["--database-url", &db_url, "db", "migrate"]);
    let migrate_output = migrate.output().expect("execute migrate");
    assert!(
        migrate_output.status.success(),
        "db migrate should succeed: {}",
        String::from_utf8_lossy(&migrate_output.stderr)
    );

    let auth = auth_harness();
    let mut list = cli();
    list.args(["--database-url", &db_url, "dispatch", "list"]);
    add_auth_args(&mut list, &auth);

    let output = list.output().expect("execute list");
    assert!(
        output.status.success(),
        "list should succeed after explicit migrate: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let v: Value = serde_json::from_str(&stdout).expect("json");
    assert!(v["dispatches"].is_array());
}

#[test]
fn full_lifecycle_create_get_list_cancel() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let stdout = create_dispatch(&db_url, &auth);
    let created: Value = serde_json::from_str(&stdout).expect("JSON");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    let mut get_cmd = cli();
    get_cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "get",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut get_cmd, &auth);
    let get_output = get_cmd.output().expect("execute get");
    assert!(get_output.status.success());

    let mut list_cmd = cli();
    list_cmd.args(["--database-url", &db_url, "dispatch", "list"]);
    add_auth_args(&mut list_cmd, &auth);
    let list_output = list_cmd.output().expect("execute list");
    assert!(list_output.status.success());

    let mut cancel_cmd = cli();
    cancel_cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "cancel",
        "--id",
        dispatch_id,
        "--reason",
        "test cancel",
    ]);
    add_auth_args(&mut cancel_cmd, &auth);
    let cancel_output = cancel_cmd.output().expect("execute cancel");
    assert!(
        cancel_output.status.success(),
        "cancel should succeed: {}",
        String::from_utf8_lossy(&cancel_output.stderr)
    );
}

#[test]
fn second_cancel_returns_invalid_transition_code() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let stdout = create_dispatch(&db_url, &auth);
    let created: Value = serde_json::from_str(&stdout).expect("json");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    let mut first = cli();
    first.args([
        "--database-url",
        &db_url,
        "dispatch",
        "cancel",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut first, &auth);
    let first_output = first.output().expect("execute first cancel");
    assert!(first_output.status.success(), "first cancel should succeed");

    let mut second = cli();
    second.args([
        "--database-url",
        &db_url,
        "dispatch",
        "cancel",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut second, &auth);
    let second_output = second.output().expect("execute second cancel");
    assert!(!second_output.status.success(), "second cancel should fail");
    let stderr = String::from_utf8(second_output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_transition");
}

#[test]
fn missing_actor_token_flags_fail_closed() {
    let (db_url, _dir) = temp_db();
    let output = cli()
        .args(["--database-url", &db_url, "dispatch", "list"])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("actor_token"));
}

#[test]
fn invalid_log_level_outputs_json_on_stderr() {
    let output = cli()
        .args(["--log-level", "[", "db", "migrate"])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "should fail with non-zero exit");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("stderr should be valid JSON");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("log_level"));
}

#[test]
fn store_internal_failure_with_trace_outputs_pure_json_on_stderr() {
    let auth = auth_harness();
    let state_dir = tempfile::tempdir().expect("state dir");
    let mut cmd = cli();
    cmd.env("XDG_STATE_HOME", state_dir.path());
    cmd.args([
        "--log-level",
        "trace",
        "--database-url",
        "sqlite:/dev/null/tanren.db?mode=rwc",
        "dispatch",
        "list",
    ]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "internal");
    let correlation_id = v["details"]["correlation_id"]
        .as_str()
        .expect("correlation_id");
    assert!(!correlation_id.is_empty());
    let sink_path = state_dir.path().join("tanren/internal-errors.jsonl");
    let sink = std::fs::read_to_string(&sink_path).expect("sink file");
    let line = sink.lines().next().expect("sink line");
    let event: Value = serde_json::from_str(line).expect("event json");
    assert_eq!(event["error_code"], "internal");
    assert_eq!(event["correlation_id"], correlation_id);
    assert_eq!(event["component"], "tanren_app_services");
}
