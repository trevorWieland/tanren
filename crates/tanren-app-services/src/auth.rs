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
use tanren_store::ReplayGuard as StoreReplayGuard;
use uuid::Uuid;

/// Default hard ceiling for accepted actor token lifetime (`exp - iat`).
pub const DEFAULT_ACTOR_TOKEN_MAX_TTL_SECS: u64 = 900;

/// Allowed positive clock skew for `iat` relative to local wall clock.
const DEFAULT_IAT_FUTURE_SKEW_SECS: i64 = 30;
const MAX_ISS_CLAIM_LEN: usize = 256;
const MAX_AUD_CLAIM_LEN: usize = 256;
const MAX_JTI_CLAIM_LEN: usize = 512;

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

/// Replay guard key materialized from a verified actor token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayGuard {
    issuer: String,
    audience: String,
    jti: String,
    iat_unix: i64,
    exp_unix: i64,
}

impl ReplayGuard {
    #[must_use]
    pub fn new(
        issuer: String,
        audience: String,
        jti: String,
        iat_unix: i64,
        exp_unix: i64,
    ) -> Self {
        Self {
            issuer,
            audience,
            jti,
            iat_unix,
            exp_unix,
        }
    }

    #[must_use]
    pub fn to_store_replay_guard(&self) -> StoreReplayGuard {
        StoreReplayGuard {
            issuer: self.issuer.clone(),
            audience: self.audience.clone(),
            jti: self.jti.clone(),
            iat_unix: self.iat_unix,
            exp_unix: self.exp_unix,
        }
    }
}

/// Verified token material including trusted request context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedActorToken {
    context: RequestContext,
    replay_guard: ReplayGuard,
}

impl VerifiedActorToken {
    /// Borrow the trusted request context extracted from claims.
    #[must_use]
    pub const fn context(&self) -> &RequestContext {
        &self.context
    }

    /// Borrow replay guard key material for mutating command paths.
    #[must_use]
    pub const fn replay_guard(&self) -> &ReplayGuard {
        &self.replay_guard
    }

    /// Consume into `(RequestContext, ReplayGuard)`.
    #[must_use]
    pub fn into_parts(self) -> (RequestContext, ReplayGuard) {
        (self.context, self.replay_guard)
    }
}

/// Typed token-verification failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TokenVerificationError {
    /// Token did not pass cryptographic/claim validation.
    #[error("token validation failed")]
    InvalidToken,
}

/// Auth failure taxonomy shared across verify/replay/backend boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailureKind {
    InvalidToken,
    ReplayRejected,
    BackendFailure,
}

impl TokenVerificationError {
    #[must_use]
    pub const fn kind(self) -> AuthFailureKind {
        match self {
            Self::InvalidToken => AuthFailureKind::InvalidToken,
        }
    }
}

impl From<TokenVerificationError> for ContractError {
    fn from(err: TokenVerificationError) -> Self {
        match err {
            TokenVerificationError::InvalidToken => token_validation_contract_error(),
        }
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

    /// Verify a signed actor token and return trusted context + replay guard.
    pub fn verify(&self, token: &str) -> Result<VerifiedActorToken, TokenVerificationError> {
        Self::validate_header(token)?;

        let claims = decode::<ActorTokenClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|err| {
                emit_auth_boundary_internal_error("invalid_actor_token", &err.to_string());
                token_validation_error()
            })?
            .claims;

        self.enforce_claim_sanity(&claims)?;

        let actor = ActorContext {
            org_id: OrgId::from_uuid(claims.org_id),
            user_id: UserId::from_uuid(claims.user_id),
            team_id: claims.team_id.map(TeamId::from_uuid),
            api_key_id: claims.api_key_id.map(ApiKeyId::from_uuid),
            project_id: claims.project_id.map(ProjectId::from_uuid),
        };
        let replay_guard =
            ReplayGuard::new(claims.iss, claims.aud, claims.jti, claims.iat, claims.exp);
        Ok(VerifiedActorToken {
            context: RequestContext::new(actor),
            replay_guard,
        })
    }

    fn validate_header(token: &str) -> Result<(), TokenVerificationError> {
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

    fn enforce_claim_sanity(
        &self,
        claims: &ActorTokenClaims,
    ) -> Result<(), TokenVerificationError> {
        validate_claim_string("iss", &claims.iss, MAX_ISS_CLAIM_LEN)?;
        validate_claim_string("aud", &claims.aud, MAX_AUD_CLAIM_LEN)?;
        validate_claim_string("jti", &claims.jti, MAX_JTI_CLAIM_LEN)?;

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

fn validate_claim_string(
    field: &str,
    value: &str,
    max_len: usize,
) -> Result<(), TokenVerificationError> {
    if value.trim().is_empty() {
        emit_auth_boundary_internal_error(
            "actor_token_claim_invalid",
            &format!("{field} must be non-empty"),
        );
        return Err(token_validation_error());
    }
    if value.len() > max_len {
        emit_auth_boundary_internal_error(
            "actor_token_claim_invalid",
            &format!("{field} exceeds max length {max_len}"),
        );
        return Err(token_validation_error());
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

fn token_validation_contract_error() -> ContractError {
    ContractError::InvalidField {
        field: "actor_token".to_owned(),
        reason: "token validation failed".to_owned(),
    }
}

fn token_validation_error() -> TokenVerificationError {
    TokenVerificationError::InvalidToken
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
