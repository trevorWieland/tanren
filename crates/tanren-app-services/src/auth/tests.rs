use std::sync::OnceLock;

use chrono::Utc;
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey, spki::der::pem::LineEnding};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand_core::OsRng;
use serde::Serialize;
use uuid::Uuid;

use super::*;

/// Lazily-generated Ed25519 keypair for these tests.
///
/// Replaces the previously-committed private-key PEM literals so no
/// secret-shaped material lands in source control (lane-0.4 audit
/// follow-up; `GitGuardian` incident 30350537). The keypair is only
/// ever used to sign and verify tokens within this `#[cfg(test)]`
/// module.
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
    verifier_with_byte_ceiling(max_ttl_secs, DEFAULT_ACTOR_TOKEN_MAX_BYTES)
}

fn verifier_with_byte_ceiling(max_ttl_secs: u64, max_token_bytes: usize) -> ActorTokenVerifier {
    ActorTokenVerifier::from_public_key_pem(
        test_public_key_pem(),
        "tanren-tests",
        "tanren-cli",
        max_ttl_secs,
        max_token_bytes,
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
        &EncodingKey::from_ed_pem(test_private_key_pem().as_bytes()).expect("encoding key"),
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

#[test]
fn verify_rejects_token_exceeding_max_bytes_before_decode() {
    // Sign a legitimate token, then pad a bogus suffix so its byte
    // length exceeds the configured ceiling. The verifier must reject
    // on size before touching the JWT decoder, so padding the token
    // makes the signature invalid as a side-effect — but the failure
    // mode we assert is the size guard, not signature failure.
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let base_token = sign(&claims, Some("kid-1"));
    let tiny_limit = 128usize;
    assert!(
        base_token.len() > tiny_limit,
        "test token already oversized"
    );
    let verifier = verifier_with_byte_ceiling(300, tiny_limit);
    let err = verifier
        .verify(&base_token)
        .expect_err("oversized token must fail");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn verify_accepts_token_exactly_at_max_bytes_boundary() {
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let token = sign(&claims, Some("kid-1"));
    // Ceiling equal to token length must pass; ceiling one below must fail.
    let verifier_ok = verifier_with_byte_ceiling(300, token.len());
    verifier_ok.verify(&token).expect("boundary token accepted");

    let verifier_reject = verifier_with_byte_ceiling(300, token.len() - 1);
    let err = verifier_reject
        .verify(&token)
        .expect_err("one byte below ceiling must reject");
    assert_eq!(err.kind(), AuthFailureKind::InvalidToken);
}

#[test]
fn constructors_reject_zero_max_token_bytes() {
    let err = ActorTokenVerifier::from_public_key_pem(
        test_public_key_pem(),
        "tanren-tests",
        "tanren-cli",
        300,
        0,
    )
    .expect_err("zero max_token_bytes must fail");
    assert!(matches!(
        err,
        ContractError::InvalidField { ref field, .. }
            if field == "actor_token_max_bytes"
    ));
}
