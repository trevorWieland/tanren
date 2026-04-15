use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use tokio::sync::Mutex;

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

#[derive(Debug, Clone)]
struct ReplayStoreMock {
    consume_result: bool,
    consumed: Arc<Mutex<Vec<(String, String, String)>>>,
    purge_calls: Arc<Mutex<u64>>,
}

impl ReplayStoreMock {
    fn accepting() -> Self {
        Self {
            consume_result: true,
            consumed: Arc::new(Mutex::new(Vec::new())),
            purge_calls: Arc::new(Mutex::new(0)),
        }
    }

    fn rejecting_replay() -> Self {
        Self {
            consume_result: false,
            consumed: Arc::new(Mutex::new(Vec::new())),
            purge_calls: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl TokenReplayStore for ReplayStoreMock {
    async fn consume_actor_token_jti(
        &self,
        params: ConsumeActorTokenJtiParams,
    ) -> tanren_store::StoreResult<bool> {
        self.consumed
            .lock()
            .await
            .push((params.issuer, params.audience, params.jti));
        Ok(self.consume_result)
    }

    async fn purge_expired_actor_token_jtis(
        &self,
        _params: PurgeExpiredActorTokenJtisParams,
    ) -> tanren_store::StoreResult<u64> {
        let mut calls = self.purge_calls.lock().await;
        *calls += 1;
        Ok(0)
    }
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

#[tokio::test]
async fn verify_accepts_valid_token_and_consumes_replay_key() {
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let replay_store = ReplayStoreMock::accepting();

    let context = verifier
        .verify_and_consume(&token, &replay_store)
        .await
        .expect("valid token");

    assert_eq!(context.actor().org_id.into_uuid(), claims.org_id);
    assert_eq!(context.actor().user_id.into_uuid(), claims.user_id);

    let consumed = replay_store.consumed.lock().await;
    assert_eq!(consumed.len(), 1);
    assert_eq!(consumed[0].2, claims.jti);
}

#[tokio::test]
async fn verify_accepts_token_without_kid_header() {
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let token = sign(&claims, None);

    let verifier = verifier(300);
    let replay_store = ReplayStoreMock::accepting();

    verifier
        .verify_and_consume(&token, &replay_store)
        .await
        .expect("missing kid should still validate with static public key");
}

#[tokio::test]
async fn verify_rejects_missing_jti_claim() {
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
    let replay_store = ReplayStoreMock::accepting();

    let err = verifier
        .verify_and_consume(&token, &replay_store)
        .await
        .expect_err("missing jti must fail");

    assert!(matches!(
        err,
        ContractError::InvalidField { ref field, ref reason }
        if field == "actor_token" && reason == "token validation failed"
    ));
}

#[tokio::test]
async fn verify_rejects_replayed_token() {
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let replay_store = ReplayStoreMock::rejecting_replay();

    let err = verifier
        .verify_and_consume(&token, &replay_store)
        .await
        .expect_err("replay must fail");

    assert!(matches!(
        err,
        ContractError::InvalidField { ref field, ref reason }
        if field == "actor_token" && reason == "token validation failed"
    ));
}

#[tokio::test]
async fn verify_rejects_token_ttl_above_configured_max() {
    let now = Utc::now().timestamp();
    let mut claims = base_claims(now);
    claims.exp = claims.iat + 3_600;
    let token = sign(&claims, Some("kid-1"));

    let verifier = verifier(300);
    let replay_store = ReplayStoreMock::accepting();

    let err = verifier
        .verify_and_consume(&token, &replay_store)
        .await
        .expect_err("ttl must fail");

    assert!(matches!(
        err,
        ContractError::InvalidField { ref field, ref reason }
        if field == "actor_token" && reason == "token validation failed"
    ));
}
