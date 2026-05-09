use std::env;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use axum::Json;
use axum::Router;
use axum::http::{HeaderValue, Method, header};
use axum::middleware;
use axum::routing::get;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde::{Deserialize, Serialize};
use tanren_app_services::{Handlers, Store};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::{API_KEY_ENV, AuthConfig, AuthState, require_authenticated_principal};
use crate::{
    ALLOWED_HOSTS_ENV, BIND_ADDRESS_ENV, CORS_ORIGINS_ENV, Config, DATABASE_URL_ENV,
    DEFAULT_BIND_ADDRESS, TanrenMcp,
};

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

fn build_router(
    auth_state: AuthState,
    handlers: Handlers,
    store: Arc<Store>,
    cancellation: CancellationToken,
    cors: CorsMode,
    allowed_origins: &[String],
) -> Router {
    let config = streamable_http_config(cancellation, allowed_origins);
    let mcp_service: StreamableHttpService<TanrenMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(TanrenMcp::new(handlers.clone(), store.clone())),
            Arc::new(LocalSessionManager::default()),
            config,
        );

    let mcp_with_auth = ServiceBuilder::new()
        .layer(middleware::from_fn_with_state(
            auth_state,
            require_authenticated_principal,
        ))
        .service(mcp_service);

    Router::new()
        .route("/health", get(health))
        .nest_service("/mcp", mcp_with_auth)
        .layer(cors.into_layer())
}

/// Build rmcp's `StreamableHttpServerConfig` honouring the
/// `TANREN_MCP_ALLOWED_HOSTS` env var.
fn streamable_http_config(
    cancellation: CancellationToken,
    allowed_origins: &[String],
) -> StreamableHttpServerConfig {
    let mut base = StreamableHttpServerConfig::default().with_cancellation_token(cancellation);
    let raw = env::var(ALLOWED_HOSTS_ENV).ok().filter(|s| !s.is_empty());
    let Some(value) = raw else {
        if !allowed_origins.is_empty() {
            base = base.with_allowed_origins(allowed_origins.iter().cloned());
        }
        return base;
    };
    if value.trim() == "*" {
        tracing::warn!(
            target: "tanren_mcp",
            env_var = ALLOWED_HOSTS_ENV,
            "Host-header validation disabled by `*`; relying on API-key auth as the sole gate."
        );
        base = base.disable_allowed_hosts();
        if !allowed_origins.is_empty() {
            base = base.with_allowed_origins(allowed_origins.iter().cloned());
        }
        return base;
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
    base = base.with_allowed_hosts(hosts);
    if !allowed_origins.is_empty() {
        base = base.with_allowed_origins(allowed_origins.iter().cloned());
    }
    base
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
    let auth_state = AuthState {
        config: Arc::new(AuthConfig {
            bootstrap_key: Some(api_key),
        }),
        store: store.clone(),
    };
    let cancellation = CancellationToken::new();
    let router = build_router(
        auth_state,
        Handlers::new(),
        store,
        cancellation.clone(),
        CorsMode::TestHooksPermissive,
        &[],
    );
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
            "TANREN_MCP_API_KEY is not set — account bootstrap tools are disabled; only valid bearer session tokens can authenticate."
        );
    }
    let cors_allowlist = parse_required_cors_allowlist(env::var(CORS_ORIGINS_ENV).ok().as_deref())?;

    let database_url = env::var(DATABASE_URL_ENV).with_context(|| {
        format!("{DATABASE_URL_ENV} must be set so tanren-mcp can connect to the event store")
    })?;
    let store = Arc::new(
        Store::connect(&database_url)
            .await
            .with_context(|| format!("connect to store at {DATABASE_URL_ENV}"))?,
    );
    let handlers = Handlers::new();
    let auth_state = AuthState {
        config: auth_config,
        store: store.clone(),
    };

    let cancellation = CancellationToken::new();
    let router = build_router(
        auth_state,
        handlers,
        store,
        cancellation.clone(),
        CorsMode::Explicit(cors_allowlist.header_values.clone()),
        &cors_allowlist.raw_values,
    );

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

#[derive(Debug, Clone)]
struct CorsAllowlist {
    raw_values: Vec<String>,
    header_values: Vec<HeaderValue>,
}

#[derive(Debug, Clone)]
enum CorsMode {
    Explicit(Vec<HeaderValue>),
    TestHooksPermissive,
}

impl CorsMode {
    fn into_layer(self) -> CorsLayer {
        match self {
            Self::Explicit(origins) => CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
                .allow_headers([
                    header::ACCEPT,
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::HeaderName::from_static("last-event-id"),
                    header::HeaderName::from_static("mcp-protocol-version"),
                    header::HeaderName::from_static("mcp-session-id"),
                    header::HeaderName::from_static("x-api-key"),
                ]),
            Self::TestHooksPermissive => CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        }
    }
}

fn parse_required_cors_allowlist(raw: Option<&str>) -> Result<CorsAllowlist> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        bail!(
            "{CORS_ORIGINS_ENV} must be set to a comma-separated list of explicit origins for tanren-mcp (for example: https://agent.example.com)"
        );
    };

    let mut raw_values = Vec::new();
    let mut header_values = Vec::new();
    for token in raw.split(',') {
        let origin = token.trim();
        if origin.is_empty() {
            continue;
        }
        let value = HeaderValue::from_str(origin)
            .with_context(|| format!("parse CORS origin `{origin}` as HeaderValue"))?;
        raw_values.push(origin.to_owned());
        header_values.push(value);
    }
    if raw_values.is_empty() {
        bail!(
            "{CORS_ORIGINS_ENV} must include at least one explicit origin; wildcards are not accepted"
        );
    }
    Ok(CorsAllowlist {
        raw_values,
        header_values,
    })
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!(target: "tanren_mcp", "shutdown signal received");
}
