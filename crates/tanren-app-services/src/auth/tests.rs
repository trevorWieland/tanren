use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::Serialize;
use tokio::sync::Mutex;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

const TEST_ED25519_PRIVATE_KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIAPLmow/yTJDEVu9jxvrdcEK0yfRG0bAzr3hnOrtggLP
-----END PRIVATE KEY-----
";
const TEST_ED25519_PUBLIC_KEY_X: &str = "7jO4B-xp2yKG7Rh2aMFdyIsqxEMq8jYMO7b7HEZ6vLs";

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

fn jwks_json(kid: &str) -> String {
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

fn verifier(kid: &str, max_ttl_secs: u64) -> ActorTokenVerifier {
    ActorTokenVerifier::from_jwks_json(&jwks_json(kid), "tanren-tests", "tanren-cli", max_ttl_secs)
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

fn sign_with_kid<T: Serialize>(claims: &T, kid: Option<&str>) -> String {
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
    let token = sign_with_kid(&claims, Some("kid-1"));

    let mut verifier = verifier("kid-1", 300);
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
async fn verify_rejects_missing_kid_header() {
    let now = Utc::now().timestamp();
    let claims = base_claims(now);
    let token = sign_with_kid(&claims, None);

    let mut verifier = verifier("kid-1", 300);
    let replay_store = ReplayStoreMock::accepting();

    let err = verifier
        .verify_and_consume(&token, &replay_store)
        .await
        .expect_err("missing kid must fail");

    assert!(matches!(
        err,
        ContractError::InvalidField { ref field, ref reason }
        if field == "actor_token" && reason == "token validation failed"
    ));
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
    let token = sign_with_kid(&claims, Some("kid-1"));

    let mut verifier = verifier("kid-1", 300);
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
    let token = sign_with_kid(&claims, Some("kid-1"));

    let mut verifier = verifier("kid-1", 300);
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
    let token = sign_with_kid(&claims, Some("kid-1"));

    let mut verifier = verifier("kid-1", 300);
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

#[tokio::test]
async fn verifier_refreshes_remote_jwks_on_unknown_kid() {
    let server = MockServer::start().await;
    let path_name = "/.well-known/jwks.json";
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_for_mock = Arc::clone(&call_count);
    Mock::given(method("GET"))
        .and(path(path_name))
        .respond_with(move |_: &wiremock::Request| {
            if call_count_for_mock.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(200).set_body_string(jwks_json("kid-old"))
            } else {
                ResponseTemplate::new(200).set_body_string(jwks_json("kid-new"))
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let url = format!("{}{path_name}", server.uri());
    let mut verifier = ActorTokenVerifier::from_jwks_url(&url, "tanren-tests", "tanren-cli", 300)
        .await
        .expect("verifier");

    let token = sign_with_kid(&base_claims(Utc::now().timestamp()), Some("kid-new"));
    let replay_store = ReplayStoreMock::accepting();

    let context = verifier
        .verify_and_consume(&token, &replay_store)
        .await
        .expect("should refresh and validate");

    assert!(context.actor().org_id.into_uuid() != Uuid::nil());
}
