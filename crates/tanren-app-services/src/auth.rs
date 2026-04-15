//! Trusted actor context extraction from signed bearer tokens.
//!
//! Interface layers provide a signed JWT. This module verifies the
//! signature and required claims, enforces replay protection, then
//! materializes a trusted [`RequestContext`] for service/orchestrator
//! policy checks.

use std::collections::HashSet;
use std::path::Path;

use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
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

#[derive(Debug, Clone)]
enum JwksSource {
    Static,
    RemoteUrl {
        url: String,
        client: reqwest::Client,
    },
}

/// Verifies signed actor tokens and produces trusted request context.
#[derive(Debug, Clone)]
pub struct ActorTokenVerifier {
    jwks: JwkSet,
    source: JwksSource,
    validation: Validation,
    max_token_ttl_secs: i64,
    iat_future_skew_secs: i64,
}

impl ActorTokenVerifier {
    /// Construct a verifier from an in-memory JWKS JSON document.
    pub fn from_jwks_json(
        jwks_json: &str,
        issuer: &str,
        audience: &str,
        max_token_ttl_secs: u64,
    ) -> Result<Self, ContractError> {
        let jwks = parse_jwks_json(jwks_json)?;
        Ok(Self {
            jwks,
            source: JwksSource::Static,
            validation: build_validation(issuer, audience),
            max_token_ttl_secs: i64::try_from(max_token_ttl_secs).unwrap_or(i64::MAX),
            iat_future_skew_secs: DEFAULT_IAT_FUTURE_SKEW_SECS,
        })
    }

    /// Construct a verifier from a local JWKS file.
    pub fn from_jwks_file(
        path: &Path,
        issuer: &str,
        audience: &str,
        max_token_ttl_secs: u64,
    ) -> Result<Self, ContractError> {
        let jwks_json = std::fs::read_to_string(path).map_err(|err| {
            emit_auth_boundary_internal_error("invalid_actor_jwks", &err.to_string());
            ContractError::InvalidField {
                field: "actor_jwks".to_owned(),
                reason: "invalid actor jwks".to_owned(),
            }
        })?;
        Self::from_jwks_json(&jwks_json, issuer, audience, max_token_ttl_secs)
    }

    /// Construct a verifier from a remote JWKS URL.
    pub async fn from_jwks_url(
        url: &str,
        issuer: &str,
        audience: &str,
        max_token_ttl_secs: u64,
    ) -> Result<Self, ContractError> {
        let client = reqwest::Client::new();
        let jwks = fetch_jwks_from_url(&client, url).await?;
        Ok(Self {
            jwks,
            source: JwksSource::RemoteUrl {
                url: url.to_owned(),
                client,
            },
            validation: build_validation(issuer, audience),
            max_token_ttl_secs: i64::try_from(max_token_ttl_secs).unwrap_or(i64::MAX),
            iat_future_skew_secs: DEFAULT_IAT_FUTURE_SKEW_SECS,
        })
    }

    /// Verify a signed actor token, enforce one-time replay consumption,
    /// and return trusted request context.
    pub async fn verify_and_consume<S>(
        &mut self,
        token: &str,
        replay_store: &S,
    ) -> Result<RequestContext, ContractError>
    where
        S: TokenReplayStore,
    {
        let (kid, decoding_key) = self.resolve_decoding_key(token).await?;

        let claims = decode::<ActorTokenClaims>(token, &decoding_key, &self.validation)
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
                &format!("kid={kid}; jti={}", claims.jti),
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

    async fn resolve_decoding_key(
        &mut self,
        token: &str,
    ) -> Result<(String, DecodingKey), ContractError> {
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

        let Some(kid) = header.kid.filter(|kid| !kid.trim().is_empty()) else {
            emit_auth_boundary_internal_error(
                "missing_actor_token_kid",
                "token header missing kid",
            );
            return Err(token_validation_error());
        };

        if self.jwks.find(&kid).is_none() {
            self.refresh_jwks().await?;
        }

        let jwk = self.jwks.find(&kid).ok_or_else(|| {
            emit_auth_boundary_internal_error("unknown_actor_token_kid", &kid);
            token_validation_error()
        })?;

        let decoding_key = DecodingKey::from_jwk(jwk).map_err(|err| {
            emit_auth_boundary_internal_error("invalid_actor_jwk", &err.to_string());
            token_validation_error()
        })?;

        Ok((kid, decoding_key))
    }

    async fn refresh_jwks(&mut self) -> Result<(), ContractError> {
        let JwksSource::RemoteUrl { url, client } = &self.source else {
            return Ok(());
        };

        let refreshed = fetch_jwks_from_url(client, url).await.map_err(|err| {
            emit_auth_boundary_internal_error("actor_jwks_refresh_failed", &err.to_string());
            token_validation_error()
        })?;
        self.jwks = refreshed;
        Ok(())
    }
}

fn parse_jwks_json(jwks_json: &str) -> Result<JwkSet, ContractError> {
    serde_json::from_str::<JwkSet>(jwks_json).map_err(|err| {
        emit_auth_boundary_internal_error("invalid_actor_jwks", &err.to_string());
        ContractError::InvalidField {
            field: "actor_jwks".to_owned(),
            reason: "invalid actor jwks".to_owned(),
        }
    })
}

async fn fetch_jwks_from_url(client: &reqwest::Client, url: &str) -> Result<JwkSet, ContractError> {
    let response = client.get(url).send().await.map_err(|err| {
        emit_auth_boundary_internal_error("invalid_actor_jwks", &err.to_string());
        ContractError::InvalidField {
            field: "actor_jwks".to_owned(),
            reason: "invalid actor jwks".to_owned(),
        }
    })?;

    let response = response.error_for_status().map_err(|err| {
        emit_auth_boundary_internal_error("invalid_actor_jwks", &err.to_string());
        ContractError::InvalidField {
            field: "actor_jwks".to_owned(),
            reason: "invalid actor jwks".to_owned(),
        }
    })?;

    let body = response.text().await.map_err(|err| {
        emit_auth_boundary_internal_error("invalid_actor_jwks", &err.to_string());
        ContractError::InvalidField {
            field: "actor_jwks".to_owned(),
            reason: "invalid actor jwks".to_owned(),
        }
    })?;

    parse_jwks_json(&body)
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
