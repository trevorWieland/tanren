//! Trusted actor context extraction from signed bearer tokens.
//!
//! Interface layers provide a signed JWT. This module verifies the
//! signature and required claims, then materializes a trusted
//! [`RequestContext`] for service/orchestrator policy checks.

use std::collections::HashSet;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;
use tanren_contract::ContractError;
use tanren_domain::{ActorContext, ApiKeyId, OrgId, ProjectId, TeamId, UserId};
use tanren_observability::emit_correlated_internal_error;
use uuid::Uuid;

/// Trusted request context for service and orchestrator entrypoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    actor: ActorContext,
}

impl RequestContext {
    /// Build a trusted request context from an already-trusted actor.
    #[must_use]
    pub const fn new(actor: ActorContext) -> Self {
        Self { actor }
    }

    /// Borrow the trusted actor context.
    #[must_use]
    pub const fn actor(&self) -> &ActorContext {
        &self.actor
    }
}

/// Verifies signed actor tokens and produces trusted request context.
#[derive(Clone)]
pub struct ActorTokenVerifier {
    decoding_key: DecodingKey,
    validation: Validation,
}

impl std::fmt::Debug for ActorTokenVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorTokenVerifier").finish_non_exhaustive()
    }
}

impl ActorTokenVerifier {
    /// Construct a verifier for Ed25519/EdDSA actor tokens.
    pub fn from_ed25519_pem(
        public_key_pem: &str,
        issuer: &str,
        audience: &str,
    ) -> Result<Self, ContractError> {
        let decoding_key = DecodingKey::from_ed_pem(public_key_pem.as_bytes()).map_err(|err| {
            ContractError::InvalidField {
                field: "actor_public_key".to_owned(),
                reason: format!("invalid Ed25519 public key PEM: {err}"),
            }
        })?;

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = 0;
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[audience]);
        validation.required_spec_claims = HashSet::from_iter([
            "exp".to_owned(),
            "nbf".to_owned(),
            "iss".to_owned(),
            "aud".to_owned(),
        ]);

        Ok(Self {
            decoding_key,
            validation,
        })
    }

    /// Verify a signed actor token and return trusted context.
    pub fn verify(&self, token: &str) -> Result<RequestContext, ContractError> {
        let claims = decode::<ActorTokenClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|err| {
                let correlation_id = Uuid::now_v7();
                if emit_correlated_internal_error(
                    "tanren_app_services",
                    "invalid_actor_token",
                    correlation_id,
                    &err.to_string(),
                )
                .is_err()
                {
                    // Fail closed at the wire boundary with generic auth errors.
                }
                ContractError::InvalidField {
                    field: "actor_token".to_owned(),
                    reason: "token validation failed".to_owned(),
                }
            })?
            .claims;

        if (claims.team_id.is_some() || claims.api_key_id.is_some()) && claims.project_id.is_none()
        {
            return Err(ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: "team_id/api_key_id claims require project_id claim".to_owned(),
            });
        }

        let actor = ActorContext {
            org_id: OrgId::from_uuid(claims.org_id),
            user_id: UserId::from_uuid(claims.user_id),
            team_id: claims.team_id.map(TeamId::from_uuid),
            api_key_id: claims.api_key_id.map(ApiKeyId::from_uuid),
            project_id: claims.project_id.map(ProjectId::from_uuid),
        };
        Ok(RequestContext::new(actor))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActorTokenClaims {
    #[serde(rename = "iss")]
    _iss: String,
    #[serde(rename = "aud")]
    _aud: String,
    #[serde(rename = "exp")]
    _exp: i64,
    #[serde(rename = "nbf")]
    _nbf: i64,
    org_id: Uuid,
    user_id: Uuid,
    #[serde(default)]
    team_id: Option<Uuid>,
    #[serde(default)]
    api_key_id: Option<Uuid>,
    #[serde(default)]
    project_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde::Serialize;

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
        org_id: Uuid,
        user_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        team_id: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_key_id: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        project_id: Option<Uuid>,
    }

    fn verifier() -> ActorTokenVerifier {
        ActorTokenVerifier::from_ed25519_pem(
            TEST_ED25519_PUBLIC_KEY_PEM,
            "tanren-tests",
            "tanren-cli",
        )
        .expect("verifier")
    }

    fn sign(claims: &impl Serialize) -> String {
        encode(
            &Header::new(Algorithm::EdDSA),
            claims,
            &EncodingKey::from_ed_pem(TEST_ED25519_PRIVATE_KEY_PEM.as_bytes())
                .expect("encoding key"),
        )
        .expect("token")
    }

    fn base_claims(now_unix: i64) -> TestClaims {
        TestClaims {
            iss: "tanren-tests".to_owned(),
            aud: "tanren-cli".to_owned(),
            exp: now_unix + 60,
            nbf: now_unix - 60,
            org_id: Uuid::now_v7(),
            user_id: Uuid::now_v7(),
            team_id: None,
            api_key_id: None,
            project_id: None,
        }
    }

    #[test]
    fn verify_accepts_valid_token() {
        let now = chrono::Utc::now().timestamp();
        let claims = base_claims(now);
        let token = sign(&claims);
        let ctx = verifier().verify(&token).expect("valid token");
        assert_eq!(ctx.actor().org_id.into_uuid(), claims.org_id);
        assert_eq!(ctx.actor().user_id.into_uuid(), claims.user_id);
    }

    #[test]
    fn verify_rejects_wrong_issuer() {
        let now = chrono::Utc::now().timestamp();
        let mut claims = base_claims(now);
        claims.iss = "wrong-issuer".to_owned();
        let token = sign(&claims);
        let err = verifier().verify(&token).expect_err("issuer mismatch");
        assert!(matches!(
            err,
            ContractError::InvalidField { ref field, .. } if field == "actor_token"
        ));
        if let ContractError::InvalidField { reason, .. } = err {
            assert_eq!(reason, "token validation failed");
            assert!(!reason.contains("issuer"));
            assert!(!reason.contains("audience"));
            assert!(!reason.contains("expired"));
        }
    }

    #[test]
    fn verify_rejects_wrong_audience() {
        let now = chrono::Utc::now().timestamp();
        let mut claims = base_claims(now);
        claims.aud = "wrong-audience".to_owned();
        let token = sign(&claims);
        let err = verifier().verify(&token).expect_err("audience mismatch");
        assert!(matches!(
            err,
            ContractError::InvalidField { ref field, .. } if field == "actor_token"
        ));
    }

    #[test]
    fn verify_rejects_expired_token() {
        let now = chrono::Utc::now().timestamp();
        let mut claims = base_claims(now);
        claims.exp = now - 1;
        let token = sign(&claims);
        let err = verifier().verify(&token).expect_err("expired");
        assert!(matches!(
            err,
            ContractError::InvalidField { ref field, .. } if field == "actor_token"
        ));
    }

    #[test]
    fn verify_rejects_not_before_in_future() {
        let now = chrono::Utc::now().timestamp();
        let mut claims = base_claims(now);
        claims.nbf = now + 120;
        let token = sign(&claims);
        let err = verifier().verify(&token).expect_err("nbf in future");
        assert!(matches!(
            err,
            ContractError::InvalidField { ref field, .. } if field == "actor_token"
        ));
    }

    #[test]
    fn verify_rejects_missing_scope_ids() {
        let now = chrono::Utc::now().timestamp();
        let claims = serde_json::json!({
            "iss": "tanren-tests",
            "aud": "tanren-cli",
            "exp": now + 60,
            "nbf": now - 60,
            "user_id": Uuid::now_v7(),
        });
        let token = sign(&claims);
        let err = verifier().verify(&token).expect_err("missing org_id");
        assert!(matches!(
            err,
            ContractError::InvalidField { ref field, .. } if field == "actor_token"
        ));
    }

    #[test]
    fn verify_rejects_scope_without_project() {
        let now = chrono::Utc::now().timestamp();
        let mut claims = base_claims(now);
        claims.team_id = Some(Uuid::now_v7());
        claims.project_id = None;
        let token = sign(&claims);
        let err = verifier().verify(&token).expect_err("scope invalid");
        assert!(matches!(
            err,
            ContractError::InvalidField { ref field, .. } if field == "actor_token"
        ));
    }
}
