use std::env;
use std::sync::Arc;

use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::{AccountStore, Handlers, Store};
use tanren_identity_policy::{AccountId, SessionToken};
use tokio_util::sync::CancellationToken;

const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";
const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct HealthResponse {
    status: String,
    version: String,
    contract_version: u32,
}

pub(super) async fn health() -> Json<HealthResponse> {
    let report = Handlers::new().health(env!("CARGO_PKG_VERSION"));
    Json(HealthResponse {
        status: report.status.to_owned(),
        version: report.version.to_owned(),
        contract_version: report.contract_version.value(),
    })
}

#[derive(Debug, Clone)]
pub(super) struct AuthConfig {
    pub(super) bootstrap_key: Option<secrecy::SecretString>,
}

#[derive(Debug, Clone)]
pub(super) struct AuthState {
    pub(super) config: AuthConfig,
    pub(super) store: Arc<Store>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum AuthenticatedMcpCredential {
    BootstrapApiKey,
    Session { account_id: AccountId },
}

impl AuthConfig {
    pub(super) fn from_env() -> Self {
        let bootstrap_key = env::var(API_KEY_ENV)
            .ok()
            .filter(|s| !s.is_empty())
            .map(secrecy::SecretString::from);
        Self { bootstrap_key }
    }

    pub(crate) fn extract_credential(headers: &HeaderMap) -> Option<&str> {
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

pub(super) async fn require_api_key(
    axum::extract::State(state): axum::extract::State<Arc<AuthState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(presented) = AuthConfig::extract_credential(request.headers()) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(error_body(
                "auth_required",
                "Missing bearer credential. Set Authorization: Bearer <session-token>.",
            )),
        )
            .into_response();
    };

    if state
        .config
        .bootstrap_key
        .as_ref()
        .map(ExposeSecret::expose_secret)
        .is_some_and(|expected| constant_time_eq(presented.as_bytes(), expected.as_bytes()))
    {
        request
            .extensions_mut()
            .insert(AuthenticatedMcpCredential::BootstrapApiKey);
        return next.run(request).await;
    }

    let Ok(token) = SessionToken::parse(presented) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(error_body(
                "auth_required",
                "Missing, expired, or malformed MCP session credential.",
            )),
        )
            .into_response();
    };
    let now = Utc::now();
    match state.store.find_active_session(&token, now).await {
        Ok(Some(session)) => {
            request
                .extensions_mut()
                .insert(AuthenticatedMcpCredential::Session {
                    account_id: session.account_id,
                });
            next.run(request).await
        }
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(error_body(
                "auth_required",
                "Missing, expired, or unknown MCP session credential.",
            )),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(
                target: "tanren_mcp",
                error = %err,
                "resolve MCP bearer credential"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body(
                    "internal_error",
                    "Tanren encountered an internal error while resolving MCP credentials.",
                )),
            )
                .into_response()
        }
    }
}

pub(super) fn streamable_http_config(
    cancellation: CancellationToken,
) -> StreamableHttpServerConfig {
    let base = StreamableHttpServerConfig::default().with_cancellation_token(cancellation);
    let raw = env::var(ALLOWED_HOSTS_ENV).ok().filter(|s| !s.is_empty());
    let Some(value) = raw else {
        return base;
    };
    if value.trim() == "*" {
        tracing::warn!(
            target: "tanren_mcp",
            env_var = ALLOWED_HOSTS_ENV,
            "Host-header validation disabled by `*`; relying on credential auth as the sole gate."
        );
        return base.disable_allowed_hosts();
    }
    let mut hosts: Vec<String> = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];
    for host in value.split(',') {
        let trimmed = host.trim();
        if !trimmed.is_empty() {
            hosts.push(trimmed.to_owned());
        }
    }
    tracing::info!(
        target: "tanren_mcp",
        allowed_hosts = ?hosts,
        "Host-header validation extended via {ALLOWED_HOSTS_ENV}"
    );
    base.with_allowed_hosts(hosts)
}

fn error_body(code: &str, summary: &str) -> serde_json::Value {
    json!({
        "code": code,
        "summary": summary,
    })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
