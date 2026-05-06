//! HTTP transport layer for the tanren-mcp server.
//!
//! Houses the axum router, API-key middleware, health endpoint,
//! host-header allowlist, and the `serve` entry point so that the
//! tool surface in `lib.rs` stays focused on MCP tool definitions.

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tanren_app_services::{Handlers, Store};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::TanrenMcp;

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";
const DATABASE_URL_ENV: &str = "DATABASE_URL";
const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";

/// Configuration for the tanren-mcp runtime. R-0001 sub-8 keeps it
/// env-driven; downstream PRs may swap in a typed config crate without
/// changing the [`serve`] signature.
#[derive(Debug, Default)]
pub struct Config;

impl Config {
    /// Construct the default config; bind address, allowed hosts, and
    /// API key continue to come from environment variables.
    #[must_use]
    pub const fn from_env() -> Self {
        Self
    }
}

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
    /// R-0008 will route through the real credential store. Wrapped in
    /// `SecretString` so accidental `Debug` / `Serialize` calls do not
    /// leak the credential.
    bootstrap_key: Option<secrecy::SecretString>,
}

impl AuthConfig {
    fn from_env() -> Self {
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

async fn require_api_key(
    axum::extract::State(config): axum::extract::State<Arc<AuthConfig>>,
    request: Request,
    next: Next,
) -> Response {
    // Operator-config check first: an unconfigured server is in an
    // outage state, not an auth-failure state.
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

fn build_router(
    auth_config: Arc<AuthConfig>,
    handlers: Handlers,
    store: Arc<Store>,
    cancellation: CancellationToken,
) -> Router {
    let config = streamable_http_config(cancellation);
    let mcp_service: StreamableHttpService<TanrenMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(TanrenMcp::new(handlers.clone(), store.clone())),
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
/// `TANREN_MCP_ALLOWED_HOSTS` env var.
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

/// Build the MCP axum router around a caller-supplied `Arc<Store>` and a
/// caller-supplied bootstrap API key. Intended for the BDD wire-harness
/// in `tanren-testkit`: the harness owns the database, seeds
/// invitations + reads events directly, and spawns this router on an
/// ephemeral port. Returns the router plus the `CancellationToken`
/// callers can flip to drive graceful shutdown of the rmcp streaming
/// service.
#[cfg(any(test, feature = "test-hooks"))]
pub fn build_router_with_store(
    store: Arc<Store>,
    api_key: secrecy::SecretString,
) -> (Router, CancellationToken) {
    let auth_config = Arc::new(AuthConfig {
        bootstrap_key: Some(api_key),
    });
    let cancellation = CancellationToken::new();
    let router = build_router(auth_config, Handlers::new(), store, cancellation.clone());
    (router, cancellation)
}

/// Serve the tanren-mcp surface to completion. Honours `SIGTERM`/`SIGINT`
/// for graceful shutdown.
///
/// # Errors
///
/// Returns an error if the database connection cannot be established,
/// the listener cannot bind, or `axum::serve` returns an error.
pub async fn serve(_config: Config) -> Result<()> {
    let bind = env::var(BIND_ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_owned());
    let auth_config = Arc::new(AuthConfig::from_env());
    if auth_config.bootstrap_key.is_none() {
        tracing::warn!(
            target: "tanren_mcp",
            env_var = API_KEY_ENV,
            "TANREN_MCP_API_KEY is not set — every /mcp request will be rejected with `unavailable` until a bootstrap key is provided."
        );
    }

    let database_url = env::var(DATABASE_URL_ENV).with_context(|| {
        format!("{DATABASE_URL_ENV} must be set so tanren-mcp can connect to the event store")
    })?;
    let store = Arc::new(
        Store::connect(&database_url)
            .await
            .with_context(|| format!("connect to store at {DATABASE_URL_ENV}"))?,
    );
    let handlers = Handlers::new();

    let cancellation = CancellationToken::new();
    let router = build_router(auth_config, handlers, store, cancellation.clone());

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
