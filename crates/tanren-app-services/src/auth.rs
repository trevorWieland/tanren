//! Trusted actor context extraction from signed bearer tokens.
//!
//! Interface layers provide a signed JWT. This module verifies the
//! signature and required claims, enforces replay protection, then
//! materializes a trusted [`RequestContext`] for service/orchestrator
//! policy checks.

use std::collections::HashSet;
use std::path::Path;

use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tanren_contract::ContractError;
use tanren_domain::{ActorContext, ApiKeyId, OrgId, ProjectId, TeamId, UserId};
use tanren_observability::emit_correlated_internal_error;
use tanren_store::{
    ConsumeActorTokenJtiParams, PurgeExpiredActorTokenJtisParams, TokenReplayStore,
};
use uuid::Uuid;

/// Default hard ceiling for accepted actor token lifetime (`exp - iat`).
pub const DEFAULT_ACTOR_TOKEN_MAX_TTL_SECS: u64 = 900;

/// Allowed positive clock skew for `iat` relative to local wall clock.
const DEFAULT_IAT_FUTURE_SKEW_SECS: i64 = 30;

/// Maximum replay rows to purge per verification call.
const REPLAY_PURGE_LIMIT: u64 = 128;

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
    max_token_ttl_secs: i64,
    iat_future_skew_secs: i64,
}

impl std::fmt::Debug for ActorTokenVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorTokenVerifier")
            .field("decoding_key", &"<redacted>")
            .field("validation", &self.validation)
            .field("max_token_ttl_secs", &self.max_token_ttl_secs)
            .field("iat_future_skew_secs", &self.iat_future_skew_secs)
            .finish()
    }
}

impl ActorTokenVerifier {
    /// Construct a verifier from an in-memory Ed25519 public key PEM document.
    pub fn from_public_key_pem(
        public_key_pem: &str,
        issuer: &str,
        audience: &str,
        max_token_ttl_secs: u64,
    ) -> Result<Self, ContractError> {
        let decoding_key = DecodingKey::from_ed_pem(public_key_pem.as_bytes()).map_err(|err| {
            emit_auth_boundary_internal_error("invalid_actor_public_key", &err.to_string());
            ContractError::InvalidField {
                field: "actor_public_key".to_owned(),
                reason: "invalid actor public key".to_owned(),
            }
        })?;

        Ok(Self {
            decoding_key,
            validation: build_validation(issuer, audience),
            max_token_ttl_secs: i64::try_from(max_token_ttl_secs).unwrap_or(i64::MAX),
            iat_future_skew_secs: DEFAULT_IAT_FUTURE_SKEW_SECS,
        })
    }

    /// Construct a verifier from a local Ed25519 public key PEM file.
    pub fn from_public_key_file(
        path: &Path,
        issuer: &str,
        audience: &str,
        max_token_ttl_secs: u64,
    ) -> Result<Self, ContractError> {
        let public_key_pem = std::fs::read_to_string(path).map_err(|err| {
            emit_auth_boundary_internal_error("invalid_actor_public_key", &err.to_string());
            ContractError::InvalidField {
                field: "actor_public_key".to_owned(),
                reason: "invalid actor public key".to_owned(),
            }
        })?;
        Self::from_public_key_pem(&public_key_pem, issuer, audience, max_token_ttl_secs)
    }

    /// Verify a signed actor token, enforce one-time replay consumption,
    /// and return trusted request context.
    pub async fn verify_and_consume<S>(
        &self,
        token: &str,
        replay_store: &S,
    ) -> Result<RequestContext, ContractError>
    where
        S: TokenReplayStore,
    {
        Self::validate_header(token)?;

        let claims = decode::<ActorTokenClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|err| {
                emit_auth_boundary_internal_error("invalid_actor_token", &err.to_string());
                token_validation_error()
            })?
            .claims;

        self.enforce_claim_sanity(&claims)?;

        let consumed = replay_store
            .consume_actor_token_jti(ConsumeActorTokenJtiParams {
                issuer: claims.iss.clone(),
                audience: claims.aud.clone(),
                jti: claims.jti.clone(),
                iat_unix: claims.iat,
                exp_unix: claims.exp,
                consumed_at: Utc::now(),
            })
            .await
            .map_err(|err| {
                emit_auth_boundary_internal_error(
                    "actor_token_replay_store_error",
                    &err.to_string(),
                );
                token_validation_error()
            })?;

        if !consumed {
            emit_auth_boundary_internal_error(
                "actor_token_replay_rejected",
                &format!("jti={}", claims.jti),
            );
            return Err(token_validation_error());
        }

        if let Err(err) = replay_store
            .purge_expired_actor_token_jtis(PurgeExpiredActorTokenJtisParams {
                expires_before_unix: Utc::now().timestamp(),
                limit: REPLAY_PURGE_LIMIT,
            })
            .await
        {
            emit_auth_boundary_internal_error("actor_token_replay_purge_error", &err.to_string());
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

    fn validate_header(token: &str) -> Result<(), ContractError> {
        let header = decode_header(token).map_err(|err| {
            emit_auth_boundary_internal_error("invalid_actor_token_header", &err.to_string());
            token_validation_error()
        })?;

        if header.alg != Algorithm::EdDSA {
            emit_auth_boundary_internal_error(
                "invalid_actor_token_algorithm",
                &format!("unexpected algorithm: {:?}", header.alg),
            );
            return Err(token_validation_error());
        }

        Ok(())
    }

    fn enforce_claim_sanity(&self, claims: &ActorTokenClaims) -> Result<(), ContractError> {
        let token_ttl = claims.exp.saturating_sub(claims.iat);
        if token_ttl <= 0 || token_ttl > self.max_token_ttl_secs {
            emit_auth_boundary_internal_error(
                "actor_token_ttl_violation",
                &format!(
                    "exp={}, iat={}, max_ttl={}",
                    claims.exp, claims.iat, self.max_token_ttl_secs
                ),
            );
            return Err(token_validation_error());
        }

        let now = Utc::now().timestamp();
        if claims.iat > now.saturating_add(self.iat_future_skew_secs) {
            emit_auth_boundary_internal_error(
                "actor_token_iat_future_violation",
                &format!(
                    "iat={}, now={}, skew={}",
                    claims.iat, now, self.iat_future_skew_secs
                ),
            );
            return Err(token_validation_error());
        }
        if claims.iat < claims.nbf.saturating_sub(self.iat_future_skew_secs) {
            emit_auth_boundary_internal_error(
                "actor_token_iat_nbf_violation",
                &format!(
                    "iat={}, nbf={}, skew={}",
                    claims.iat, claims.nbf, self.iat_future_skew_secs
                ),
            );
            return Err(token_validation_error());
        }

        if (claims.team_id.is_some() || claims.api_key_id.is_some()) && claims.project_id.is_none()
        {
            emit_auth_boundary_internal_error(
                "actor_token_scope_inconsistent",
                "team_id/api_key_id requires project_id",
            );
            return Err(token_validation_error());
        }

        Ok(())
    }
}

fn build_validation(issuer: &str, audience: &str) -> Validation {
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
        "iat".to_owned(),
        "jti".to_owned(),
    ]);
    validation
}

fn token_validation_error() -> ContractError {
    ContractError::InvalidField {
        field: "actor_token".to_owned(),
        reason: "token validation failed".to_owned(),
    }
}

fn emit_auth_boundary_internal_error(error_code: &str, raw_error: &str) {
    let correlation_id = Uuid::now_v7();
    let _ = emit_correlated_internal_error(
        "tanren_app_services",
        error_code,
        correlation_id,
        raw_error,
    );
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActorTokenClaims {
    iss: String,
    aud: String,
    exp: i64,
    nbf: i64,
    iat: i64,
    jti: String,
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
mod tests;
