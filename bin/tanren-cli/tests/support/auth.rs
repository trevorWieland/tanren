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
const TEST_ED25519_PUBLIC_KEY_X: &str = "7jO4B-xp2yKG7Rh2aMFdyIsqxEMq8jYMO7b7HEZ6vLs";
const DEFAULT_KID: &str = "kid-1";

#[derive(Debug)]
pub(crate) struct AuthHarness {
    pub token: String,
    pub issuer: String,
    pub audience: String,
    pub kid: String,
    pub jwks_file: PathBuf,
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
        &EncodingKey::from_ed_pem(TEST_ED25519_PRIVATE_KEY_PEM.as_bytes()).expect("encoding key"),
    )
    .expect("token")
}

pub(crate) fn jwks_json(kid: &str) -> String {
    serde_json::json!({
        "keys": [
            {
                "kty": "OKP",
                "crv": "Ed25519",
                "x": TEST_ED25519_PUBLIC_KEY_X,
                "kid": kid,
                "alg": "EdDSA",
                "use": "sig"
            }
        ]
    })
    .to_string()
}

pub(crate) fn auth_harness() -> AuthHarness {
    auth_harness_with_claims(&base_claims())
}

pub(crate) fn auth_harness_with_org(org_id: Uuid) -> AuthHarness {
    auth_harness_with_claims(&base_claims_with_org(org_id))
}

pub(crate) fn auth_harness_with_claims(claims: &ActorClaims) -> AuthHarness {
    auth_harness_with_claims_and_kid(claims, DEFAULT_KID)
}

pub(crate) fn auth_harness_with_claims_and_kid(claims: &ActorClaims, kid: &str) -> AuthHarness {
    let token = sign_with_kid(claims, Some(kid));

    let dir = tempfile::tempdir().expect("temp dir");
    let jwks_file = dir.path().join("actor-jwks.json");
    std::fs::write(&jwks_file, jwks_json(kid)).expect("write jwks");
    let actor_token_file = dir.path().join("actor-token.jwt");
    std::fs::write(&actor_token_file, &token).expect("write token");

    AuthHarness {
        token,
        issuer: claims.iss.clone(),
        audience: claims.aud.clone(),
        kid: kid.to_owned(),
        jwks_file,
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
        "--actor-jwks-file",
        auth.jwks_file.to_str().expect("utf8 path"),
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
        "--actor-token-max-ttl-secs",
        &ttl_secs.to_string(),
    ]);
}

pub(crate) fn add_auth_args_with_jwks_url(cmd: &mut Command, auth: &AuthHarness, jwks_url: &str) {
    cmd.args([
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-jwks-url",
        jwks_url,
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
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
    let _ = &auth.kid;
    let _ = migrate as fn(&str);
    let _ = base_claims_with_org as fn(Uuid) -> ActorClaims;
    let _ = claims_missing_jti as fn() -> ActorClaimsMissingJti;
    let _ = auth_harness_with_org as fn(Uuid) -> AuthHarness;
    let _ = add_auth_args as fn(&mut Command, &AuthHarness);
    let _ = add_auth_args_with_ttl as fn(&mut Command, &AuthHarness, u64);
    let _ = add_auth_args_with_jwks_url as fn(&mut Command, &AuthHarness, &str);
    let _ = assert_stderr_is_single_json as fn(&str) -> Value;
}
