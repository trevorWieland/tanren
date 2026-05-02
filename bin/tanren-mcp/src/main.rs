//! Tanren MCP (Model Context Protocol) server.
//!
//! F-0002 corrects F-0001's stdio scaffolding: the architecture mandates
//! MCP Streamable HTTP with scoped credentials (see
//! `docs/architecture/subsystems/interfaces.md#mcp` and
//! `docs/architecture/technology.md`). This binary wraps an `rmcp`
//! `StreamableHttpService` in `axum`, gates `/mcp` behind an API-key
//! middleware that emits the shared
//! `auth_required` / `permission_denied` taxonomy from the interfaces
//! contract, and exposes the same `/health` shape as `tanren-api`.
//!
//! The bootstrap key is sourced from `TANREN_MCP_API_KEY`; the real
//! credential store lands with R-0008 (B-0048 / B-0125) and replaces
//! the env-sourced placeholder. The tool registry is empty — R-* slices
//! add `#[rmcp::tool]` routes that route through `tanren-app-services`.

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rmcp::ServerHandler;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tanren_app_services::Handlers;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";
/// Comma-separated extra hostnames / `host:port` authorities to add to
/// rmcp's `allowed_hosts` Host-header allowlist. rmcp's defaults
/// (`localhost`, `127.0.0.1`, `::1`) are kept; this env var appends to
/// them. Set to `*` to disable Host-header validation entirely (auth
/// remains the only gate). Operators deploying MCP behind a load
/// balancer or under a real hostname must set this — see
/// `docs/architecture/subsystems/interfaces.md` and
/// `docs/architecture/operations.md`.
const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";

/// Empty tool surface. Behavior tools land with R-* slices via
/// `#[rmcp::tool]` annotations on this type.
#[derive(Debug, Clone, Default)]
struct TanrenMcp;

impl ServerHandler for TanrenMcp {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
    contract_version: u32,
}

async fn health() -> Json<HealthResponse> {
    let report = Handlers::new().health(env!("CARGO_PKG_VERSION"));
    Json(HealthResponse {
        status: report.status.to_owned(),
        version: report.version.to_owned(),
        contract_version: report.contract_version.value(),
    })
}

/// Shared error response shape per
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy".
fn error_body(code: &str, summary: &str) -> serde_json::Value {
    json!({
        "code": code,
        "summary": summary,
    })
}

#[derive(Debug, Clone)]
struct AuthConfig {
    /// Bootstrap API key. F-0002 sources this from `TANREN_MCP_API_KEY`;
    /// R-0008 will route through the real credential store.
    bootstrap_key: Option<String>,
}

impl AuthConfig {
    fn from_env() -> Self {
        let bootstrap_key = env::var(API_KEY_ENV).ok().filter(|s| !s.is_empty());
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

async fn require_api_key(
    axum::extract::State(config): axum::extract::State<Arc<AuthConfig>>,
    request: Request,
    next: Next,
) -> Response {
    // Operator-config check first: an unconfigured server is in an
    // outage state, not an auth-failure state. Reporting 401 to an
    // unauthenticated probe in that case would misclassify the outage
    // as a client-credential failure and route operators away from
    // the actual cause.
    let Some(expected) = config.bootstrap_key.as_deref() else {
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

    if !constant_time_eq(presented.as_bytes(), expected.as_bytes()) {
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

fn build_router(auth_config: Arc<AuthConfig>, cancellation: CancellationToken) -> Router {
    let config = streamable_http_config(cancellation);
    let mcp_service: StreamableHttpService<TanrenMcp, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(TanrenMcp),
            Arc::new(LocalSessionManager::default()),
            config,
        );

    let mcp_with_auth = ServiceBuilder::new()
        .layer(middleware::from_fn_with_state(auth_config, require_api_key))
        .service(mcp_service);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .nest_service("/mcp", mcp_with_auth)
        .layer(cors)
}

/// Build rmcp's `StreamableHttpServerConfig` honouring the
/// `TANREN_MCP_ALLOWED_HOSTS` env var. rmcp ships loopback-only Host
/// validation by default for DNS-rebind protection; non-local
/// deployments must extend the allowlist (or set `*` to disable Host
/// validation entirely and rely solely on the API-key middleware).
fn streamable_http_config(cancellation: CancellationToken) -> StreamableHttpServerConfig {
    let base = StreamableHttpServerConfig::default().with_cancellation_token(cancellation);
    let raw = env::var(ALLOWED_HOSTS_ENV).ok().filter(|s| !s.is_empty());
    let Some(value) = raw else {
        return base;
    };
    if value.trim() == "*" {
        tracing::warn!(
            target: "tanren_mcp",
            env_var = ALLOWED_HOSTS_ENV,
            "Host-header validation disabled by `*`; relying on API-key auth as the sole gate."
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

#[tokio::main]
async fn main() -> Result<()> {
    tanren_observability::init().context("install tracing subscriber")?;

    let bind = env::var(BIND_ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_owned());
    let auth_config = Arc::new(AuthConfig::from_env());
    if auth_config.bootstrap_key.is_none() {
        tracing::warn!(
            target: "tanren_mcp",
            env_var = API_KEY_ENV,
            "TANREN_MCP_API_KEY is not set — every /mcp request will be rejected with `unavailable` until a bootstrap key is provided."
        );
    }

    let cancellation = CancellationToken::new();
    let router = build_router(auth_config, cancellation.clone());

    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("bind {bind}"))?;
    tracing::info!(target: "tanren_mcp", address = %bind, "tanren-mcp listening on streamable HTTP");

    let cancel = cancellation.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            cancel.cancel();
        })
        .await
        .context("axum serve")?;
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let sigterm = signal(SignalKind::terminate()).ok();
    if let Some(mut sigterm) = sigterm {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    } else {
        let _ = tokio::signal::ctrl_c().await;
    }
    tracing::info!(target: "tanren_mcp", "shutdown signal received");
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!(target: "tanren_mcp", "shutdown signal received");
}
