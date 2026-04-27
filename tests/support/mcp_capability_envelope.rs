use std::sync::OnceLock;

use chrono::Utc;
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey, spki::der::pem::LineEnding};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand_core::OsRng;
use serde::Serialize;
use uuid::Uuid;

const TEST_ISSUER: &str = "tanren-mcp-tests";
const TEST_AUDIENCE: &str = "tanren-mcp";

fn test_keypair_pems() -> &'static (String, String) {
    static KEYS: OnceLock<(String, String)> = OnceLock::new();
    KEYS.get_or_init(|| {
        let signing_key = SigningKey::generate(&mut OsRng);
        let private_pem = signing_key
            .to_pkcs8_pem(LineEnding::LF)
            .expect("encode private key")
            .to_string();
        let public_pem = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("encode public key");
        (private_pem, public_pem)
    })
}

pub(crate) fn test_capability_public_key_pem() -> &'static str {
    test_keypair_pems().1.as_str()
}

pub(crate) fn test_capability_issuer() -> &'static str {
    TEST_ISSUER
}

pub(crate) fn test_capability_audience() -> &'static str {
    TEST_AUDIENCE
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CapabilityEnvelopeClaimsFixture {
    pub(crate) iss: String,
    pub(crate) aud: String,
    pub(crate) exp: i64,
    pub(crate) nbf: i64,
    pub(crate) iat: i64,
    pub(crate) jti: String,
    pub(crate) phase: String,
    pub(crate) spec_id: Uuid,
    pub(crate) agent_session_id: String,
    pub(crate) capabilities: Vec<String>,
}

impl CapabilityEnvelopeClaimsFixture {
    pub(crate) fn valid(
        phase: &str,
        spec_id: Uuid,
        agent_session_id: &str,
        capabilities_csv: &str,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            iss: TEST_ISSUER.to_owned(),
            aud: TEST_AUDIENCE.to_owned(),
            exp: now + 600,
            nbf: now - 30,
            iat: now - 5,
            jti: Uuid::now_v7().to_string(),
            phase: phase.to_owned(),
            spec_id,
            agent_session_id: agent_session_id.to_owned(),
            capabilities: parse_capabilities_csv(capabilities_csv),
        }
    }
}

pub(crate) fn sign_capability_envelope(claims: &CapabilityEnvelopeClaimsFixture) -> String {
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = Some("mcp-test-key".to_owned());
    encode(
        &header,
        claims,
        &EncodingKey::from_ed_pem(test_keypair_pems().0.as_bytes()).expect("encoding key"),
    )
    .expect("encode capability token")
}

pub(crate) fn signed_capability_token(
    phase: &str,
    spec_id: Uuid,
    agent_session_id: &str,
    capabilities_csv: &str,
) -> String {
    let claims =
        CapabilityEnvelopeClaimsFixture::valid(phase, spec_id, agent_session_id, capabilities_csv);
    sign_capability_envelope(&claims)
}

fn parse_capabilities_csv(capabilities_csv: &str) -> Vec<String> {
    capabilities_csv
        .split(',')
        .map(str::trim)
        .filter(|capability| !capability.is_empty())
        .map(str::to_owned)
        .collect()
}
