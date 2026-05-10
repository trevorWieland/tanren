use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub(crate) const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";

#[derive(Debug, Clone)]
pub(crate) struct AuthConfig {
    pub(crate) bootstrap_key: Option<secrecy::SecretString>,
}

impl AuthConfig {
    pub(crate) fn from_env() -> Result<Self> {
        let bootstrap_key = parse_bootstrap_key(std::env::var(API_KEY_ENV))?;
        Ok(Self { bootstrap_key })
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
    axum::extract::State(config): axum::extract::State<Arc<AuthConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = config
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

    if !constant_time_eq_digest(presented, expected) {
        return (
            StatusCode::FORBIDDEN,
            Json(error_body(
                "permission_denied",
                "Presented credential is not authorized for this MCP service.",
            )),
        )
            .into_response();
    }

    next.run(request).await
}

fn error_body(code: &str, summary: &str) -> serde_json::Value {
    json!({
        "code": code,
        "summary": summary,
    })
}

fn normalized_digest(value: &str) -> [u8; 32] {
    let normalized = value.trim();
    Sha256::digest(normalized.as_bytes()).into()
}

fn constant_time_eq_digest(presented: &str, expected: &str) -> bool {
    let presented_digest = normalized_digest(presented);
    let expected_digest = normalized_digest(expected);
    presented_digest.ct_eq(&expected_digest).into()
}

fn parse_bootstrap_key(
    raw: std::result::Result<String, std::env::VarError>,
) -> Result<Option<secrecy::SecretString>> {
    match raw {
        Ok(value) => {
            let normalized = value.trim();
            if normalized.is_empty() {
                return Err(anyhow!(
                    "{API_KEY_ENV} must not be empty or whitespace-only"
                ));
            }
            Ok(Some(secrecy::SecretString::from(normalized.to_owned())))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(anyhow!("read {API_KEY_ENV}: {err}")),
    }
}
