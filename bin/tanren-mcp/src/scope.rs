//! Signed capability-envelope verification for `tanren-mcp`.
//!
//! The MCP boundary requires a signed envelope and derives runtime
//! scope + phase + spec/session bindings from verified claims.

use anyhow::{Context as _, Result, anyhow, bail};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tanren_app_services::methodology::{
    CapabilityScope, PhaseId, SpecId, parse_scope_env_for_phase,
};
use uuid::Uuid;

const ENV_ENVELOPE_TOKEN: &str = "TANREN_MCP_CAPABILITY_ENVELOPE";
const ENV_PUBLIC_KEY_PEM: &str = "TANREN_MCP_CAPABILITY_PUBLIC_KEY_PEM";
const ENV_PUBLIC_KEY_FILE: &str = "TANREN_MCP_CAPABILITY_PUBLIC_KEY_FILE";
const ENV_TOKEN_ISSUER: &str = "TANREN_MCP_CAPABILITY_ISSUER";
const ENV_TOKEN_AUDIENCE: &str = "TANREN_MCP_CAPABILITY_AUDIENCE";
const ENV_TOKEN_MAX_TTL_SECS: &str = "TANREN_MCP_CAPABILITY_MAX_TTL_SECS";

const DEFAULT_CAPABILITY_TOKEN_MAX_TTL_SECS: u64 = 900;
const DEFAULT_CAPABILITY_TOKEN_MAX_BYTES: usize = 16 * 1024;
const DEFAULT_IAT_FUTURE_SKEW_SECS: i64 = 30;
const MAX_ISS_CLAIM_LEN: usize = 256;
const MAX_AUD_CLAIM_LEN: usize = 256;
const MAX_JTI_CLAIM_LEN: usize = 512;
const MAX_AGENT_SESSION_ID_LEN: usize = 120;

/// Verified capability envelope claims used by `tanren-mcp` startup.
#[derive(Debug, Clone)]
pub(crate) struct VerifiedCapabilityEnvelope {
    pub(crate) scope: CapabilityScope,
    pub(crate) phase: PhaseId,
    pub(crate) spec_id: SpecId,
    pub(crate) agent_session_id: String,
    pub(crate) replay_claims: CapabilityReplayClaims,
}

#[derive(Debug, Clone)]
pub(crate) struct CapabilityReplayClaims {
    pub(crate) issuer: String,
    pub(crate) audience: String,
    pub(crate) jti: String,
    pub(crate) iat_unix: i64,
    pub(crate) exp_unix: i64,
}

/// Parse + verify the signed capability envelope from environment.
///
/// # Errors
/// Returns an error when required env vars are missing, the envelope
/// fails cryptographic verification, or claims fail typed validation.
pub(crate) fn verify_from_env() -> Result<VerifiedCapabilityEnvelope> {
    let token = required_env_non_empty(ENV_ENVELOPE_TOKEN)?;
    if token.len() > DEFAULT_CAPABILITY_TOKEN_MAX_BYTES {
        bail!("{ENV_ENVELOPE_TOKEN} exceeds max bytes ({DEFAULT_CAPABILITY_TOKEN_MAX_BYTES})");
    }
    let public_key_pem = load_public_key_pem()?;
    let issuer = required_env_non_empty(ENV_TOKEN_ISSUER)?;
    let audience = required_env_non_empty(ENV_TOKEN_AUDIENCE)?;
    let max_ttl_secs = std::env::var(ENV_TOKEN_MAX_TTL_SECS)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            v.parse::<u64>().with_context(|| {
                format!("parsing {ENV_TOKEN_MAX_TTL_SECS} as positive integer seconds (got `{v}`)")
            })
        })
        .transpose()?
        .unwrap_or(DEFAULT_CAPABILITY_TOKEN_MAX_TTL_SECS);
    verify_signed_envelope(&token, &public_key_pem, &issuer, &audience, max_ttl_secs)
}

fn verify_signed_envelope(
    token: &str,
    public_key_pem: &str,
    issuer: &str,
    audience: &str,
    max_ttl_secs: u64,
) -> Result<VerifiedCapabilityEnvelope> {
    validate_header(token)?;
    let decoding_key = DecodingKey::from_ed_pem(public_key_pem.as_bytes())
        .context("decoding Ed25519 capability-envelope public key")?;
    let validation = build_validation(issuer, audience);
    let claims = decode::<CapabilityEnvelopeClaims>(token, &decoding_key, &validation)
        .context("verifying TANREN_MCP_CAPABILITY_ENVELOPE signature/claims")?
        .claims;
    enforce_claim_sanity(&claims, max_ttl_secs)?;

    let phase = PhaseId::try_new(claims.phase).context("parsing signed `phase` claim")?;
    let capabilities_csv = claims
        .capabilities
        .iter()
        .map(|cap| cap.trim())
        .collect::<Vec<_>>()
        .join(",");
    let scope = parse_scope_env_for_phase(&capabilities_csv, Some(&phase))
        .map_err(|err| anyhow!("invalid capabilities claim: {err}"))?;
    let spec_id = SpecId::from_uuid(claims.spec_id);

    Ok(VerifiedCapabilityEnvelope {
        scope,
        phase,
        spec_id,
        agent_session_id: claims.agent_session_id,
        replay_claims: CapabilityReplayClaims {
            issuer: claims.iss,
            audience: claims.aud,
            jti: claims.jti,
            iat_unix: claims.iat,
            exp_unix: claims.exp,
        },
    })
}

fn required_env_non_empty(name: &str) -> Result<String> {
    let raw = std::env::var(name).with_context(|| format!("missing required env var {name}"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("{name} must be non-empty");
    }
    Ok(trimmed.to_owned())
}

fn load_public_key_pem() -> Result<String> {
    if let Ok(pem) = std::env::var(ENV_PUBLIC_KEY_PEM)
        && !pem.trim().is_empty()
    {
        return Ok(pem);
    }
    if let Ok(path) = std::env::var(ENV_PUBLIC_KEY_FILE)
        && !path.trim().is_empty()
    {
        return std::fs::read_to_string(path.trim())
            .with_context(|| format!("reading {ENV_PUBLIC_KEY_FILE} path `{}`", path.trim()));
    }
    bail!(
        "missing capability-envelope public key; set {ENV_PUBLIC_KEY_PEM} or {ENV_PUBLIC_KEY_FILE}"
    )
}

fn validate_header(token: &str) -> Result<()> {
    let header = decode_header(token).context("decoding capability-envelope JWT header")?;
    if header.alg != Algorithm::EdDSA {
        bail!(
            "unsupported capability-envelope algorithm {:?}; expected EdDSA",
            header.alg
        );
    }
    Ok(())
}

fn build_validation(issuer: &str, audience: &str) -> Validation {
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = 0;
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    validation.required_spec_claims = std::collections::HashSet::from_iter([
        "exp".to_owned(),
        "nbf".to_owned(),
        "iss".to_owned(),
        "aud".to_owned(),
        "iat".to_owned(),
        "jti".to_owned(),
        "phase".to_owned(),
        "spec_id".to_owned(),
        "agent_session_id".to_owned(),
        "capabilities".to_owned(),
    ]);
    validation
}

fn enforce_claim_sanity(claims: &CapabilityEnvelopeClaims, max_ttl_secs: u64) -> Result<()> {
    validate_claim_string("iss", &claims.iss, MAX_ISS_CLAIM_LEN)?;
    validate_claim_string("aud", &claims.aud, MAX_AUD_CLAIM_LEN)?;
    validate_claim_string("jti", &claims.jti, MAX_JTI_CLAIM_LEN)?;
    validate_claim_string(
        "agent_session_id",
        &claims.agent_session_id,
        MAX_AGENT_SESSION_ID_LEN,
    )?;

    let max_ttl_secs = i64::try_from(max_ttl_secs).unwrap_or(i64::MAX);
    let token_ttl = claims.exp.saturating_sub(claims.iat);
    if token_ttl <= 0 || token_ttl > max_ttl_secs {
        bail!(
            "capability-envelope ttl out of bounds: exp={}, iat={}, max_ttl_secs={max_ttl_secs}",
            claims.exp,
            claims.iat
        );
    }

    let now = Utc::now().timestamp();
    if claims.iat > now.saturating_add(DEFAULT_IAT_FUTURE_SKEW_SECS) {
        bail!(
            "capability-envelope iat is too far in the future: iat={}, now={now}, skew={DEFAULT_IAT_FUTURE_SKEW_SECS}",
            claims.iat
        );
    }
    if claims.iat < claims.nbf.saturating_sub(DEFAULT_IAT_FUTURE_SKEW_SECS) {
        bail!(
            "capability-envelope iat is before nbf window: iat={}, nbf={}, skew={DEFAULT_IAT_FUTURE_SKEW_SECS}",
            claims.iat,
            claims.nbf
        );
    }

    for (idx, capability) in claims.capabilities.iter().enumerate() {
        if capability.trim().is_empty() {
            bail!("capabilities[{idx}] must be non-empty");
        }
    }
    Ok(())
}

fn validate_claim_string(field: &str, value: &str, max_len: usize) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{field} must be non-empty");
    }
    if value.len() > max_len {
        bail!("{field} exceeds max length {max_len}");
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityEnvelopeClaims {
    iss: String,
    aud: String,
    exp: i64,
    nbf: i64,
    iat: i64,
    jti: String,
    phase: String,
    spec_id: Uuid,
    agent_session_id: String,
    capabilities: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey, spki::der::pem::LineEnding};
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use rand_core::OsRng;
    use serde::Serialize;

    fn test_keypair_pems() -> &'static (String, String) {
        static KEYS: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
        KEYS.get_or_init(|| {
            let signing_key = SigningKey::generate(&mut OsRng);
            let private_pem = signing_key
                .to_pkcs8_pem(LineEnding::LF)
                .expect("private pem")
                .to_string();
            let public_pem = signing_key
                .verifying_key()
                .to_public_key_pem(LineEnding::LF)
                .expect("public pem");
            (private_pem, public_pem)
        })
    }

    fn sign_claims<T: Serialize>(claims: &T) -> String {
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some("mcp-test".to_owned());
        encode(
            &header,
            claims,
            &EncodingKey::from_ed_pem(test_keypair_pems().0.as_bytes()).expect("encoding key"),
        )
        .expect("token")
    }

    #[derive(Debug, Clone, Serialize)]
    struct TestClaims {
        iss: String,
        aud: String,
        exp: i64,
        nbf: i64,
        iat: i64,
        jti: String,
        phase: String,
        spec_id: Uuid,
        agent_session_id: String,
        capabilities: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct MissingJtiClaims {
        iss: String,
        aud: String,
        exp: i64,
        nbf: i64,
        iat: i64,
        phase: String,
        spec_id: Uuid,
        agent_session_id: String,
        capabilities: Vec<String>,
    }

    fn valid_claims() -> TestClaims {
        let now = Utc::now().timestamp();
        TestClaims {
            iss: "tanren-tests".to_owned(),
            aud: "tanren-mcp".to_owned(),
            exp: now + 600,
            nbf: now - 30,
            iat: now - 5,
            jti: Uuid::now_v7().to_string(),
            phase: "do-task".to_owned(),
            spec_id: Uuid::now_v7(),
            agent_session_id: "mcp-session".to_owned(),
            capabilities: vec!["task.read".to_owned(), "phase.outcome".to_owned()],
        }
    }

    #[test]
    fn verify_signed_envelope_accepts_valid_claims() {
        let claims = valid_claims();
        let token = sign_claims(&claims);
        let verified = verify_signed_envelope(
            &token,
            test_keypair_pems().1.as_str(),
            claims.iss.as_str(),
            claims.aud.as_str(),
            900,
        )
        .expect("verified");

        assert_eq!(verified.spec_id, SpecId::from_uuid(claims.spec_id));
        assert_eq!(verified.phase.as_str(), "do-task");
        assert_eq!(verified.agent_session_id, "mcp-session");
        assert!(
            verified
                .scope
                .allows(tanren_app_services::methodology::ToolCapability::TaskRead)
        );
    }

    #[test]
    fn verify_signed_envelope_rejects_unknown_capability_tag() {
        let mut claims = valid_claims();
        claims.capabilities.push("unknown.tag".to_owned());
        let token = sign_claims(&claims);

        let err = verify_signed_envelope(
            &token,
            test_keypair_pems().1.as_str(),
            claims.iss.as_str(),
            claims.aud.as_str(),
            900,
        )
        .expect_err("must fail");
        assert!(err.to_string().contains("invalid capabilities claim"));
    }

    #[test]
    fn verify_signed_envelope_rejects_ttl_above_limit() {
        let mut claims = valid_claims();
        claims.exp = claims.iat + 1_200;
        let token = sign_claims(&claims);

        let err = verify_signed_envelope(
            &token,
            test_keypair_pems().1.as_str(),
            claims.iss.as_str(),
            claims.aud.as_str(),
            900,
        )
        .expect_err("must fail");
        assert!(err.to_string().contains("ttl out of bounds"));
    }

    #[test]
    fn verify_signed_envelope_rejects_audience_mismatch() {
        let claims = valid_claims();
        let token = sign_claims(&claims);
        let err = verify_signed_envelope(
            &token,
            test_keypair_pems().1.as_str(),
            claims.iss.as_str(),
            "wrong-audience",
            900,
        )
        .expect_err("must fail");
        let lower = err.to_string().to_ascii_lowercase();
        assert!(
            lower.contains("aud")
                || lower.contains("claim")
                || lower.contains("verify")
                || lower.contains("validation")
        );
    }

    #[test]
    fn verify_signed_envelope_rejects_missing_required_claim() {
        let claims = valid_claims();
        let token = sign_claims(&MissingJtiClaims {
            iss: claims.iss,
            aud: claims.aud,
            exp: claims.exp,
            nbf: claims.nbf,
            iat: claims.iat,
            phase: claims.phase,
            spec_id: claims.spec_id,
            agent_session_id: claims.agent_session_id,
            capabilities: claims.capabilities,
        });
        let err = verify_signed_envelope(
            &token,
            test_keypair_pems().1.as_str(),
            "tanren-tests",
            "tanren-mcp",
            900,
        )
        .expect_err("must fail");
        assert!(
            !err.to_string().trim().is_empty(),
            "missing required claim must produce a non-empty validation error"
        );
    }
}
