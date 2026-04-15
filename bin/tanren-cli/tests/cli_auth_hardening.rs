//! CLI auth-boundary hardening tests.

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
    let claims = ActorClaims {
        iss: "tanren-tests".to_owned(),
        aud: "tanren-cli".to_owned(),
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
        audience: claims.aud,
        public_key_file,
        actor_token_file,
        _dir: dir,
    }
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
fn invalid_actor_token_error_is_generic_without_verification_details() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "list",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "wrong-issuer",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    let message = v["message"].as_str().expect("message");
    assert!(message.contains("token validation failed"));
    assert!(!message.contains("InvalidIssuer"));
    assert!(!message.contains("invalid issuer"));
    assert!(!message.contains("audience"));
    assert!(!message.contains("expired"));
    assert!(!message.contains("signature"));
}

#[test]
fn internal_failure_omits_correlation_id_when_sink_persist_fails() {
    let auth = auth_harness();
    let mut cmd = cli();
    cmd.env("XDG_STATE_HOME", "/dev/null");
    cmd.args([
        "--database-url",
        "sqlite:/dev/null/tanren.db?mode=rwc",
        "dispatch",
        "list",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "tanren-tests",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "internal");
    assert!(
        v.get("details")
            .and_then(|details| details.get("correlation_id"))
            .is_none(),
        "correlation_id must be omitted when sink persistence fails"
    );
}
