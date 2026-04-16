use std::path::PathBuf;
use std::sync::OnceLock;

use assert_cmd::Command;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey, spki::der::pem::LineEnding};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand_core::OsRng;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

/// Lazily-generated Ed25519 keypair for this test process.
///
/// Replaces the previously-committed private-key PEM literals so no
/// secret-shaped material lands in source control (lane-0.4 audit
/// follow-up; `GitGuardian` incident 30350537). The keypair is only
/// ever used to sign and verify tokens within this test binary —
/// there is no production code path that consumes either half — and
/// each `cargo nextest run` invocation regenerates it.
fn test_keypair_pems() -> &'static (String, String) {
    static KEYS: OnceLock<(String, String)> = OnceLock::new();
    KEYS.get_or_init(|| {
        let signing_key = SigningKey::generate(&mut OsRng);
        let private_pem = signing_key
            .to_pkcs8_pem(LineEnding::LF)
            .expect("encode pkcs8 pem")
            .to_string();
        let public_pem = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("encode spki pem");
        (private_pem, public_pem)
    })
}

fn test_private_key_pem() -> &'static str {
    test_keypair_pems().0.as_str()
}

fn test_public_key_pem() -> &'static str {
    test_keypair_pems().1.as_str()
}

#[derive(Debug)]
pub(crate) struct AuthHarness {
    pub token: String,
    pub issuer: String,
    pub audience: String,
    pub actor_public_key_file: PathBuf,
    pub actor_token_file: PathBuf,
    _dir: tempfile::TempDir,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ActorClaims {
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub nbf: i64,
    pub iat: i64,
    pub jti: String,
    pub org_id: Uuid,
    pub user_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ActorClaimsMissingJti {
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub nbf: i64,
    pub iat: i64,
    pub org_id: Uuid,
    pub user_id: Uuid,
}

pub(crate) fn cli() -> Command {
    Command::cargo_bin("tanren-cli").expect("binary should exist")
}

pub(crate) fn temp_db() -> (String, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    migrate(&url);
    (url, dir)
}

pub(crate) fn migrate(db_url: &str) {
    let mut cmd = cli();
    cmd.args(["--database-url", db_url, "db", "migrate"]);
    let output = cmd.output().expect("migrate");
    assert!(
        output.status.success(),
        "migrate should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn base_claims() -> ActorClaims {
    let now = Utc::now().timestamp();
    ActorClaims {
        iss: "tanren-tests".to_owned(),
        aud: "tanren-cli".to_owned(),
        exp: now + 600,
        nbf: now - 30,
        iat: now - 5,
        jti: Uuid::now_v7().to_string(),
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    }
}

pub(crate) fn base_claims_with_org(org_id: Uuid) -> ActorClaims {
    let mut claims = base_claims();
    claims.org_id = org_id;
    claims
}

pub(crate) fn claims_missing_jti() -> ActorClaimsMissingJti {
    let now = Utc::now().timestamp();
    ActorClaimsMissingJti {
        iss: "tanren-tests".to_owned(),
        aud: "tanren-cli".to_owned(),
        exp: now + 600,
        nbf: now - 30,
        iat: now - 5,
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
    }
}

pub(crate) fn sign_with_kid<T: Serialize>(claims: &T, kid: Option<&str>) -> String {
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = kid.map(str::to_owned);
    encode(
        &header,
        claims,
        &EncodingKey::from_ed_pem(test_private_key_pem().as_bytes()).expect("encoding key"),
    )
    .expect("token")
}

pub(crate) fn auth_harness() -> AuthHarness {
    auth_harness_with_claims(&base_claims())
}

pub(crate) fn auth_harness_with_org(org_id: Uuid) -> AuthHarness {
    auth_harness_with_claims(&base_claims_with_org(org_id))
}

pub(crate) fn auth_harness_with_claims(claims: &ActorClaims) -> AuthHarness {
    let token = sign_with_kid(claims, Some("kid-1"));

    let dir = tempfile::tempdir().expect("temp dir");
    let actor_public_key_file = dir.path().join("actor-public-key.pem");
    std::fs::write(&actor_public_key_file, test_public_key_pem()).expect("write public key");
    let actor_token_file = dir.path().join("actor-token.jwt");
    std::fs::write(&actor_token_file, &token).expect("write token");

    AuthHarness {
        token,
        issuer: claims.iss.clone(),
        audience: claims.aud.clone(),
        actor_public_key_file,
        actor_token_file,
        _dir: dir,
    }
}

pub(crate) fn add_auth_args(cmd: &mut Command, auth: &AuthHarness) {
    add_auth_args_with_ttl(cmd, auth, 900);
}

pub(crate) fn add_auth_args_with_ttl(cmd: &mut Command, auth: &AuthHarness, ttl_secs: u64) {
    cmd.args([
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
        "--actor-token-max-ttl-secs",
        &ttl_secs.to_string(),
    ]);
}

pub(crate) fn assert_stderr_is_single_json(stderr: &str) -> Value {
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

pub(crate) fn lint_anchor(auth: &AuthHarness) {
    let _ = &auth.token;
    let _ = temp_db as fn() -> (String, tempfile::TempDir);
    let _ = migrate as fn(&str);
    let _ = auth_harness as fn() -> AuthHarness;
    let _ = base_claims_with_org as fn(Uuid) -> ActorClaims;
    let _ = claims_missing_jti as fn() -> ActorClaimsMissingJti;
    let _ = auth_harness_with_org as fn(Uuid) -> AuthHarness;
    let _ = add_auth_args as fn(&mut Command, &AuthHarness);
    let _ = add_auth_args_with_ttl as fn(&mut Command, &AuthHarness, u64);
    let _ = assert_stderr_is_single_json as fn(&str) -> Value;
}
