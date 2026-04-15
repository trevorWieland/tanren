//! Auth token transport + clap display behavior coverage.

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
    token: String,
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
    let claims = ActorClaims {
        iss: issuer.clone(),
        aud: audience.clone(),
        exp: Utc::now().timestamp() + 600,
        nbf: Utc::now().timestamp() - 60,
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
        token,
        issuer,
        audience,
        public_key_file,
        actor_token_file,
        _dir: dir,
    }
}

fn migrate(db_url: &str) {
    let mut cmd = cli();
    cmd.args(["--database-url", db_url, "db", "migrate"]);
    let output = cmd.output().expect("migrate");
    assert!(
        output.status.success(),
        "migrate should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn actor_token_cli_arg_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "--actor-token",
            &auth.token,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(
        v["message"]
            .as_str()
            .expect("msg")
            .contains("--actor-token")
    );
}

#[test]
fn actor_token_can_be_read_from_stdin() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    migrate(&db_url);

    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "--actor-token-stdin",
        "--actor-public-key-file",
        auth.public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
        "dispatch",
        "list",
    ]);
    cmd.write_stdin(format!("{}\n", auth.token));
    let output = cmd.output().expect("execute");
    assert!(output.status.success(), "stdin token should authenticate");
}

#[test]
fn actor_token_can_be_read_from_env() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    migrate(&db_url);

    let output = cli()
        .env("TANREN_ACTOR_TOKEN", &auth.token)
        .args([
            "--database-url",
            &db_url,
            "--actor-public-key-file",
            auth.public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(output.status.success(), "env token should authenticate");
}

#[test]
fn token_source_conflict_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "--actor-token-stdin",
            "--actor-token-file",
            auth.actor_token_file.to_str().expect("utf8 path"),
            "--actor-public-key-file",
            auth.public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(
        !output.status.success(),
        "conflicting token sources must fail"
    );
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("token source"));
}

#[test]
fn token_source_conflict_env_plus_file_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let output = cli()
        .env("TANREN_ACTOR_TOKEN", &auth.token)
        .args([
            "--database-url",
            &db_url,
            "--actor-token-file",
            auth.actor_token_file.to_str().expect("utf8 path"),
            "--actor-public-key-file",
            auth.public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "env+file conflict must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("token source"));
}

#[test]
fn token_source_conflict_env_plus_stdin_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let mut cmd = cli();
    cmd.env("TANREN_ACTOR_TOKEN", &auth.token);
    cmd.args([
        "--database-url",
        &db_url,
        "--actor-token-stdin",
        "--actor-public-key-file",
        auth.public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
        "dispatch",
        "list",
    ]);
    cmd.write_stdin(format!("{}\n", auth.token));

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "env+stdin conflict must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("token source"));
}

#[test]
fn help_exits_successfully() {
    let output = cli().arg("--help").output().expect("execute");
    assert!(output.status.success(), "help should exit 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("Tanren"));
    assert!(stdout.contains("--database-url"));
}

#[test]
fn version_exits_successfully() {
    let output = cli().arg("--version").output().expect("execute");
    assert!(output.status.success(), "version should exit 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.starts_with("tanren "));
}
