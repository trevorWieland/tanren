//! Additional CLI contract-surface coverage.

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
}

fn cli() -> Command {
    Command::cargo_bin("tanren-cli").expect("binary should exist")
}

fn temp_db() -> (String, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    (url, dir)
}

fn auth_harness() -> AuthHarness {
    let issuer = "tanren-tests".to_owned();
    let audience = "tanren-cli".to_owned();
    let now = Utc::now().timestamp();
    let claims = ActorClaims {
        iss: issuer.clone(),
        aud: audience.clone(),
        exp: now + 600,
        nbf: now - 60,
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
    };

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

#[test]
fn create_accepts_contract_surface_fields_and_get_reflects_them() {
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
    add_auth_args(&mut get_cmd, &auth);
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
