//! Tanren HTTP API server — runtime library.
//!
//! R-0001 (sub-8) promotes the runtime out of `bin/tanren-api/src/main.rs`
//! per the thin-binary-crate profile. The binary shrinks to a wiring
//! shell that initializes tracing and calls [`serve`]; everything else
//! lives here so the BDD harness can drive the same axum router via
//! `tower::ServiceExt::oneshot` or against an ephemeral-port server.
//!
//! Modules (all crate-private):
//!
//! - `routes` hosts the `#[utoipa::path]`-annotated handlers and the
//!   `ApiDoc` struct that the `OpenApi` derive walks.
//! - `cookies` hosts the tower-sessions store dispatch (sqlite vs
//!   postgres) and the `(account_id, expires_at)` write helper.
//! - `errors` hosts the shared `{code, summary}` failure body and the
//!   `AppServiceError` mapping.
//!
//! Key sub-8 changes over the historical hand-rolled `bin/tanren-api`:
//!
//! - **Configurable CORS.** The workspace clippy guard denies
//!   `tower_http::cors::Any`. The api-app reads
//!   `TANREN_API_CORS_ORIGINS` (comma-separated) into
//!   `cors_allow_origins: Vec<HeaderValue>`; default is
//!   `["http://localhost:3000"]` for dev, but production deploys must
//!   set the env var explicitly.
//! - **Cookie sessions via `tower-sessions`.** Successful sign-up,
//!   sign-in, and accept-invitation responses set
//!   `Set-Cookie: tanren_session=<id>; HttpOnly; Secure; SameSite=Strict;
//!   Path=/; Max-Age=2592000` and return
//!   `SessionEnvelope::Cookie { account_id, expires_at }` in the body
//!   (no token). The CLI/MCP/TUI surfaces still receive the bearer-flow
//!   `SessionView`.
//! - **utoipa `OpenAPI`.** Each handler carries `#[utoipa::path(...)]`;
//!   `ApiDoc` collects them. The hand-rolled `serde_json::json!({...})`
//!   document is gone.
//! - **Sign-out.** `POST /sessions/revoke` clears the cookie via
//!   `Session::flush` and returns 204.

mod cookies;
mod errors;
mod routes;
#[cfg(feature = "test-hooks")]
mod test_hooks;

use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::http::{HeaderValue, header};
use secrecy::SecretString;
use tanren_app_services::{Handlers, Store};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[cfg(any(test, feature = "test-hooks"))]
use crate::cookies::session_layer_with_secure;
use crate::cookies::{SessionLayerEnum, build_cookie_store, session_layer};
use crate::routes::build_router;

pub use crate::errors::AccountFailureBody;
pub use crate::routes::{
    AcceptInvitationBody, AcceptInvitationResponseCookie, HealthResponse, SignInResponseCookie,
    SignUpResponseCookie,
};

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8080";
const DEFAULT_DEV_ORIGIN: &str = "http://localhost:3000";
const BIND_ADDRESS_ENV: &str = "TANREN_API_BIND";
const DATABASE_URL_ENV: &str = "DATABASE_URL";
const CORS_ORIGINS_ENV: &str = "TANREN_API_CORS_ORIGINS";

/// Configuration for the tanren-api runtime.
#[derive(Debug, Clone)]
pub struct Config {
    /// `host:port` to bind the HTTP listener on.
    pub bind: String,
    /// Database URL passed to `Store::connect` (and to the
    /// tower-sessions store). The same database is used for both the
    /// bearer-flow `account_sessions` row-set and the cookie-flow
    /// `tower_sessions` row-set.
    pub database_url: SecretString,
    /// Allowed `Origin` values for the CORS layer. `tower_http::cors::Any`
    /// is denied workspace-wide — set this explicitly.
    pub cors_allow_origins: Vec<HeaderValue>,
}

impl Config {
    /// Read the canonical environment variables into a [`Config`].
    ///
    /// - `TANREN_API_BIND` defaults to `0.0.0.0:8080`.
    /// - `DATABASE_URL` is required.
    /// - `TANREN_API_CORS_ORIGINS` is comma-separated; defaults to
    ///   `http://localhost:3000` when unset.
    ///
    /// # Errors
    ///
    /// Returns an error when `DATABASE_URL` is unset or when any
    /// configured origin fails to parse as a `HeaderValue`.
    pub fn from_env() -> Result<Self> {
        let bind = env::var(BIND_ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_owned());
        let database_url = env::var(DATABASE_URL_ENV).with_context(|| {
            format!("{DATABASE_URL_ENV} must be set so tanren-api can connect to the event store")
        })?;
        let cors_allow_origins = parse_cors_origins(env::var(CORS_ORIGINS_ENV).ok().as_deref())?;
        Ok(Self {
            bind,
            database_url: SecretString::from(database_url),
            cors_allow_origins,
        })
    }
}

fn parse_cors_origins(raw: Option<&str>) -> Result<Vec<HeaderValue>> {
    let trimmed = raw.map_or("", str::trim);
    if trimmed.is_empty() {
        return Ok(vec![HeaderValue::from_static(DEFAULT_DEV_ORIGIN)]);
    }
    let mut out = Vec::new();
    for token in trimmed.split(',') {
        let origin = token.trim();
        if origin.is_empty() {
            continue;
        }
        let value = HeaderValue::from_str(origin)
            .with_context(|| format!("parse CORS origin `{origin}` as HeaderValue"))?;
        out.push(value);
    }
    if out.is_empty() {
        return Ok(vec![HeaderValue::from_static(DEFAULT_DEV_ORIGIN)]);
    }
    Ok(out)
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) handlers: Handlers,
    pub(crate) store: Arc<Store>,
}

/// Build the axum router and the `OpenAPI` document. Exposed for the BDD
/// harness; production callers should use [`serve`].
///
/// # Errors
///
/// Returns an error if the database connection cannot be established
/// or the tower-sessions migrations fail.
pub async fn build_app(config: &Config) -> Result<axum::Router> {
    use secrecy::ExposeSecret;
    let database_url = config.database_url.expose_secret();
    let store = Arc::new(
        Store::connect(database_url)
            .await
            .with_context(|| format!("connect to store at {DATABASE_URL_ENV}"))?,
    );
    let state = AppState {
        handlers: Handlers::new(),
        store: store.clone(),
    };

    let cookie_store = build_cookie_store(database_url).await?;
    let layer = session_layer(cookie_store);

    let cors = CorsLayer::new()
        .allow_origin(config.cors_allow_origins.clone())
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_credentials(true);

    let (router, api) = build_router(state).split_for_parts();

    let openapi_router: axum::Router = axum::Router::new().route(
        "/openapi.json",
        axum::routing::get(move || {
            let api = api.clone();
            async move { Json(api) }
        }),
    );

    let merged = router.merge(openapi_router);
    #[cfg(feature = "test-hooks")]
    let merged = merged.merge(test_hooks::router(store.clone()));
    let merged = merged.layer(cors);
    let with_sessions: axum::Router = match layer {
        SessionLayerEnum::Sqlite(l) => merged.layer(l),
        SessionLayerEnum::Postgres(l) => merged.layer(l),
    };
    Ok(with_sessions)
}

/// Serve the tanren-api surface to completion. Honours `SIGTERM`/`SIGINT`
/// for graceful shutdown.
///
/// # Errors
///
/// Returns an error if the database connection cannot be established,
/// the listener cannot bind, or `axum::serve` returns an error.
pub async fn serve(config: Config) -> Result<()> {
    let bind = config.bind.clone();
    let app = build_app(&config).await?;

    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("bind {bind}"))?;
    tracing::info!(target: "tanren_api", address = %bind, "tanren-api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await
        .context("axum serve")?;
    Ok(())
}

/// Build an axum app sharing a caller-supplied `Arc<Store>` and using a
/// caller-supplied database URL for the tower-sessions cookie backing
/// store. Intended for the BDD wire-harness in `tanren-testkit`: the
/// harness owns the `SQLite` file, seeds invitations + reads recent
/// events directly via the store, and spawns the api app on an
/// ephemeral port talking to the same file. The `secure_cookie` flag
/// must be `false` for plain-HTTP loopback test traffic.
///
/// # Errors
///
/// Returns an error if the cookie session-store migrations fail.
#[cfg(any(test, feature = "test-hooks"))]
pub async fn build_app_with_store(
    store: Arc<Store>,
    cookie_database_url: &str,
    cors_allow_origins: Vec<HeaderValue>,
    secure_cookie: bool,
) -> Result<axum::Router> {
    let state = AppState {
        handlers: Handlers::new(),
        store: store.clone(),
    };

    let cookie_store = build_cookie_store(cookie_database_url).await?;
    let layer = session_layer_with_secure(cookie_store, secure_cookie);

    let cors = CorsLayer::new()
        .allow_origin(cors_allow_origins)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_credentials(true);

    let (router, api) = build_router(state).split_for_parts();

    let openapi_router: axum::Router = axum::Router::new().route(
        "/openapi.json",
        axum::routing::get(move || {
            let api = api.clone();
            async move { Json(api) }
        }),
    );

    let merged = router
        .merge(openapi_router)
        .merge(test_hooks::router(store))
        .layer(cors);
    let with_sessions: axum::Router = match layer {
        SessionLayerEnum::Sqlite(l) => merged.layer(l),
        SessionLayerEnum::Postgres(l) => merged.layer(l),
    };
    Ok(with_sessions)
}

#[cfg(unix)]
async fn shutdown() {
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
    tracing::info!(target: "tanren_api", "shutdown signal received");
}

#[cfg(not(unix))]
async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!(target: "tanren_api", "shutdown signal received");
}
