use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use uuid::Uuid;

use super::*;

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

#[derive(Debug, Clone, Serialize)]
struct TestClaims {
    iss: String,
    aud: String,
    exp: i64,
    nbf: i64,
    iat: i64,
    jti: String,
    org_id: Uuid,
    user_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
struct MissingJtiClaims {
    iss: String,
    aud: String,
    exp: i64,
    nbf: i64,
    iat: i64,
    org_id: Uuid,
    user_id: Uuid,
}

fn verifier(max_ttl_secs: u64) -> ActorTokenVerifier {
    ActorTokenVerifier::from_public_key_pem(
        TEST_ED25519_PUBLIC_KEY_PEM,
        "tanren-tests",
        "tanren-cli",
        max_ttl_secs,
    )
    .expect("verifier")
}

fn base_claims(now_unix: i64) -> TestClaims {
    TestClaims {
        iss: "tanren-tests".to_owned(),
        aud: "tanren-cli".to_owned(),
        exp: now_unix + 60,
        nbf: now_unix - 30,
        iat: now_unix - 5,
        jti: Uuid::now_v7().to_string(),
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        team_id: None,
        api_key_id: None,
        project_id: None,
    }
}

fn sign<T: Serialize>(claims: &T, kid: Option<&str>) -> String {
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = kid.map(str::to_owned);
    encode(
        &header,
        claims,
        &EncodingKey::from_ed_pem(TEST_ED25519_PRIVATE_KEY_PEM.as_bytes()).expect("encoding key"),
    )
    .expect("token")
}

#[test]
fn verify_accepts_valid_token_and_materializes_replay_guard() {
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let token_ctx = verifier.verify(&token).expect("valid token");

    assert_eq!(
        token_ctx.context().actor().org_id.into_uuid(),
        claims.org_id
    );
    assert_eq!(
        token_ctx.context().actor().user_id.into_uuid(),
        claims.user_id
    );

    let replay_guard = token_ctx.replay_guard().to_store_replay_guard();
    assert_eq!(replay_guard.issuer, claims.iss);
    assert_eq!(replay_guard.audience, claims.aud);
    assert_eq!(replay_guard.jti, claims.jti);
    assert_eq!(replay_guard.iat_unix, claims.iat);
    assert_eq!(replay_guard.exp_unix, claims.exp);
}

#[test]
fn verify_accepts_token_without_kid_header() {
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let token = sign(&claims, None);

    let verifier = verifier(300);
    verifier
        .verify(&token)
        .expect("missing kid should still validate with static public key");
}

#[test]
fn verify_rejects_missing_jti_claim() {
    let now = Utc::now().timestamp();
    let claims = MissingJtiClaims {
        iss: "tanren-tests".to_owned(),
        aud: "tanren-cli".to_owned(),
        exp: now + 60,
        nbf: now - 30,
        iat: now - 5,
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
    };
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier.verify(&token).expect_err("missing jti must fail");

    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_rejects_token_ttl_above_configured_max() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.exp = claims.iat + 3_600;
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier.verify(&token).expect_err("ttl must fail");

    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_rejects_empty_issuer_claim() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.iss = "  ".to_owned();
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier.verify(&token).expect_err("empty iss must fail");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_rejects_oversized_issuer_claim() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.iss = "i".repeat(MAX_ISS_CLAIM_LEN + 1);
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier
        .verify(&token)
        .expect_err("oversized iss must fail");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_rejects_empty_audience_claim() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.aud = String::new();
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier.verify(&token).expect_err("empty aud must fail");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_rejects_oversized_audience_claim() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.aud = "a".repeat(MAX_AUD_CLAIM_LEN + 1);
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier
        .verify(&token)
        .expect_err("oversized aud must fail");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_rejects_empty_jti_claim() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.jti = "\n".to_owned();
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier.verify(&token).expect_err("empty jti must fail");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_rejects_oversized_jti_claim() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.jti = "j".repeat(MAX_JTI_CLAIM_LEN + 1);
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let err = verifier
        .verify(&token)
        .expect_err("oversized jti must fail");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}
