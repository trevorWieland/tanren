use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::sync::Arc;
use tanren_app_services::{AccountStore, Store};
use tanren_identity_policy::{AccountId, SessionToken};

const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";

#[derive(Debug, Clone)]
pub(crate) struct AuthConfig {
    /// Bootstrap API key. F-0002 sources this from `TANREN_MCP_API_KEY`;
    /// R-0008 will route through the real credential store. Wrapped in
    /// `SecretString` so accidental `Debug` / `Serialize` calls do not
    /// leak the credential.
    pub(crate) bootstrap_key: Option<secrecy::SecretString>,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthState {
    pub(crate) config: Arc<AuthConfig>,
    pub(crate) store: Arc<Store>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthenticatedPrincipal {
    pub(crate) account_id: AccountId,
}

impl AuthConfig {
    pub(crate) fn from_env() -> Self {
        let bootstrap_key = env::var(API_KEY_ENV)
            .ok()
            .filter(|s| !s.is_empty())
            .map(secrecy::SecretString::from);
        Self { bootstrap_key }
    }

    fn extract_credential(headers: &HeaderMap) -> Option<&str> {
        if let Some(value) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            && let Some(token) = value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        {
            return Some(token.trim());
        }
        if let Some(value) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
            return Some(value.trim());
        }
        None
    }
}

pub(crate) async fn require_api_key(
    State(state): State<Arc<AuthState>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Operator-config check first: an unconfigured server is in an
    // outage state, not an auth-failure state.
    let Some(expected) = state
        .config
        .bootstrap_key
        .as_ref()
        .map(secrecy::ExposeSecret::expose_secret)
    else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "unavailable",
                "MCP credential store is not configured. Set TANREN_MCP_API_KEY (bootstrap key) until R-0008 lands the real store.",
            )),
        )
            .into_response();
    };

    let Some(presented) = AuthConfig::extract_credential(request.headers()) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(error_body(
                "auth_required",
                "Missing Authorization: Bearer <api-key> or X-API-Key header.",
            )),
        )
            .into_response();
    };

    if credentials_match(presented, expected) {
        return next.run(request).await;
    }

    match resolve_principal(state.store.as_ref(), presented).await {
        Ok(Some(principal)) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        Ok(None) => (
            StatusCode::FORBIDDEN,
            Json(error_body(
                "permission_denied",
                "Presented credential is not authorized for this MCP service.",
            )),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(target: "tanren_mcp", error = %err, "session principal resolution");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body(
                    "internal_error",
                    "Tanren encountered an internal error.",
                )),
            )
                .into_response()
        }
    }
}

async fn resolve_principal(
    store: &Store,
    presented: &str,
) -> Result<Option<AuthenticatedPrincipal>, String> {
    let token = SessionToken::from_secret(secrecy::SecretString::from(presented.to_owned()));
    let session = AccountStore::find_active_session_by_token(store, &token, Utc::now())
        .await
        .map_err(|err| err.to_string())?;
    Ok(session.map(|record| AuthenticatedPrincipal {
        account_id: record.account_id,
    }))
}

fn credentials_match(presented: &str, expected: &str) -> bool {
    let presented_hash = Sha256::digest(presented.as_bytes());
    let expected_hash = Sha256::digest(expected.as_bytes());
    constant_time_eq(&presented_hash, &expected_hash)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        let left = a.get(i).copied().unwrap_or_default();
        let right = b.get(i).copied().unwrap_or_default();
        diff |= usize::from(left ^ right);
    }
    diff == 0
}

pub(crate) fn principal_from_request(
    parts: &axum::http::request::Parts,
) -> Option<AuthenticatedPrincipal> {
    parts.extensions.get::<AuthenticatedPrincipal>().copied()
}

/// Shared error response shape per
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy".
fn error_body(code: &str, summary: &str) -> serde_json::Value {
    json!({
        "code": code,
        "summary": summary,
    })
}
