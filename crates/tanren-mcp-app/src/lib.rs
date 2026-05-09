//! Tanren MCP (Model Context Protocol) server — runtime library.
//!
//! R-0001 (sub-8) promotes the runtime out of `bin/tanren-mcp/src/main.rs`
//! per the thin-binary-crate profile. The binary shrinks to a wiring shell
//! that initializes tracing and calls [`serve`]; the rmcp tool surface,
//! API-key middleware, and host-header allowlist live here so the BDD
//! harness can exercise this code via the rmcp client crate without
//! spinning up a child process.
//!
//! The MCP surface continues to return bearer-mode `SessionView`
//! responses — there is no cookie jar between the rmcp client and server.

mod active_account;
mod auth;

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::middleware;
use axum::routing::get;
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tanren_app_services::{ActiveAccountContextError, AppServiceError, Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, ListActiveAccountsRequest, SignInRequest,
    SignUpRequest, SwitchActiveAccountRequest,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::active_account::{ActiveAccountSessionError, ActiveAccountSessionState};
use crate::auth::{API_KEY_ENV, AuthConfig, require_api_key};

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
const DATABASE_URL_ENV: &str = "DATABASE_URL";
/// Comma-separated extra hostnames / `host:port` authorities to add to
/// rmcp's `allowed_hosts` Host-header allowlist.
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

/// MCP tool surface. Holds the shared `Handlers` facade and a `Store`
/// handle; behaviour tools delegate through the facade so the api / mcp /
/// cli / tui surfaces all resolve to the same logic per the
/// equivalent-operations rule in
/// `docs/architecture/subsystems/interfaces.md`.
#[derive(Clone)]
pub(crate) struct TanrenMcp {
    clock: Clock,
    handlers: Handlers,
    store: Arc<Store>,
    active_accounts: Arc<ActiveAccountSessionState>,
    /// Cached tool router built from the `#[rmcp::tool]` methods on this
    /// type. Read by the macro-generated `ServerHandler` impl below.
    tool_router: ToolRouter<Self>,
}

impl std::fmt::Debug for TanrenMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TanrenMcp").finish_non_exhaustive()
    }
}

#[rmcp::tool_router]
impl TanrenMcp {
    fn new(clock: Clock, handlers: Handlers, store: Arc<Store>) -> Self {
        Self {
            clock,
            handlers,
            store,
            active_accounts: Arc::new(ActiveAccountSessionState::default()),
            tool_router: Self::tool_router(),
        }
    }

    /// Self-signup tool. Mirrors the api `POST /accounts` shape via
    /// `tanren_contract::SignUpRequest` / `SignUpResponse`.
    #[rmcp::tool(
        name = "account.create",
        description = "Create a new Tanren account via self-signup. Returns the new account view and an opaque session token. Failures use the shared {code, summary} taxonomy: duplicate_identifier, invalid_credential."
    )]
    async fn account_create(
        &self,
        Parameters(request): Parameters<SignUpRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_up(self.store.as_ref(), request).await {
            Ok(response) => {
                self.active_accounts
                    .note_signed_in(response.account.id, response.session.token.clone());
                Ok(success(&response))
            }
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Sign-in tool. Mirrors the api `POST /sessions` shape via
    /// `tanren_contract::SignInRequest` / `SignInResponse`.
    #[rmcp::tool(
        name = "account.sign_in",
        description = "Sign in to an existing Tanren account. Returns the account view and an opaque session token. Failure code: invalid_credential."
    )]
    async fn account_sign_in(
        &self,
        Parameters(request): Parameters<SignInRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_in(self.store.as_ref(), request).await {
            Ok(response) => {
                self.active_accounts
                    .note_signed_in(response.account.id, response.session.token.clone());
                Ok(success(&response))
            }
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Invitation-acceptance tool. Mirrors the api
    /// `POST /invitations/{token}/accept` shape via
    /// `tanren_contract::AcceptInvitationRequest` /
    /// `AcceptInvitationResponse`.
    #[rmcp::tool(
        name = "account.accept_invitation",
        description = "Accept an organization invitation and create a Tanren account in the inviting org. Failure codes: invitation_not_found, invitation_already_consumed, invitation_expired, invalid_credential."
    )]
    async fn account_accept_invitation(
        &self,
        Parameters(request): Parameters<AcceptInvitationRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .handlers
            .accept_invitation(self.store.as_ref(), request)
            .await
        {
            Ok(response) => {
                self.active_accounts
                    .note_signed_in(response.account.id, response.session.token.clone());
                Ok(success(&response))
            }
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Active-account listing tool. Uses the MCP session's signed-in set
    /// and active account context.
    #[rmcp::tool(
        name = "account.list_active",
        description = "List signed-in accounts for this MCP session and identify the active account. Failure code: invalid_credential when no signed-in account is present in this session."
    )]
    async fn account_list_active(&self) -> Result<CallToolResult, McpError> {
        let context = match self
            .active_accounts
            .context(self.store.as_ref(), &self.clock)
            .await
        {
            Ok(Some(context)) => context,
            Ok(None) => return Ok(missing_session_failure()),
            Err(ActiveAccountSessionError::Validation(err)) => {
                return Ok(active_context_failure(&err));
            }
            Err(ActiveAccountSessionError::Store(err)) => {
                return Ok(active_context_store_failure(&err));
            }
        };
        match self
            .handlers
            .list_active_accounts(
                self.store.as_ref(),
                &context,
                ListActiveAccountsRequest::default(),
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Active-account switch tool. The target must already be in the
    /// MCP session's signed-in set.
    #[rmcp::tool(
        name = "account.switch_active",
        description = "Switch the active account for this MCP session. Failure codes: invalid_credential when no signed-in account is present; target_account_not_signed_in when the target is outside the signed-in set."
    )]
    async fn account_switch_active(
        &self,
        Parameters(request): Parameters<SwitchActiveAccountRequest>,
    ) -> Result<CallToolResult, McpError> {
        let context = match self
            .active_accounts
            .context(self.store.as_ref(), &self.clock)
            .await
        {
            Ok(Some(context)) => context,
            Ok(None) => return Ok(missing_session_failure()),
            Err(ActiveAccountSessionError::Validation(err)) => {
                return Ok(active_context_failure(&err));
            }
            Err(ActiveAccountSessionError::Store(err)) => {
                return Ok(active_context_store_failure(&err));
            }
        };
        match self
            .handlers
            .switch_active_account(self.store.as_ref(), &context, request)
            .await
        {
            Ok(response) => {
                self.active_accounts.set_active(response.active_account_id);
                Ok(success(&response))
            }
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Borrow the cached `ToolRouter`. Exists so the dead-code lint can
    /// see the field as read even on rmcp macro versions whose
    /// `#[tool_handler]` expansion path does not access the field
    /// directly under the lint's heuristic. Production callers reach
    /// the router via the `ServerHandler` trait's `call_tool` /
    /// `list_tools` methods generated by `#[tool_handler]`, not this
    /// helper.
    fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }
}

#[rmcp::tool_handler]
impl ServerHandler for TanrenMcp {
    fn get_info(&self) -> ServerInfo {
        // Touch the cached router so the dead-code lint never flags
        // `tool_router` even on rmcp macro versions whose tool_handler
        // expansion path uses the static `Self::tool_router()` builder
        // rather than the cached field.
        let _ = self.router();
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Tanren control plane MCP server. Account-flow tools route through the same handlers the HTTP API uses; failure responses share the {code, summary} error taxonomy."
                .to_owned(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

/// Encode a successful handler response as a JSON-text `CallToolResult`.
fn success<T: Serialize>(value: &T) -> CallToolResult {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::success(vec![Content::text(text)])
}

/// Encode an [`AppServiceError`] as the shared `{code, summary}` error
/// body and surface it as an MCP tool failure result.
fn map_failure(err: AppServiceError) -> CallToolResult {
    let (code, summary) = match err {
        AppServiceError::Account(reason) => (reason.code().to_owned(), reason.summary().to_owned()),
        AppServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message),
        AppServiceError::Store(err) => (
            "internal_error".to_owned(),
            format!("Tanren encountered an internal error: {err}"),
        ),
        _ => (
            "internal_error".to_owned(),
            "Unknown app-service failure".to_owned(),
        ),
    };
    let body = json!({
        "code": code,
        "summary": summary,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}

fn missing_session_failure() -> CallToolResult {
    map_failure(AppServiceError::Account(
        AccountFailureReason::InvalidCredential,
    ))
}

fn active_context_failure(err: &ActiveAccountContextError) -> CallToolResult {
    let body = json!({
        "code": AccountFailureReason::ValidationFailed.code(),
        "summary": err.to_string(),
    });
    CallToolResult::error(vec![Content::text(body.to_string())])
}

fn active_context_store_failure(err: &str) -> CallToolResult {
    let body = json!({
        "code": "internal_error",
        "summary": format!("Tanren encountered an internal error: {err}"),
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
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

fn build_router(
    auth_config: Arc<AuthConfig>,
    clock: Clock,
    handlers: Handlers,
    store: Arc<Store>,
    cancellation: CancellationToken,
) -> Router {
    let config = streamable_http_config(cancellation);
    let mcp_service: StreamableHttpService<TanrenMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                Ok(TanrenMcp::new(
                    clock.clone(),
                    handlers.clone(),
                    store.clone(),
                ))
            },
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
    let clock = Clock::default();
    let router = build_router(
        auth_config,
        clock.clone(),
        Handlers::with_clock(clock),
        store,
        cancellation.clone(),
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
    let clock = Clock::default();
    let handlers = Handlers::with_clock(clock.clone());

    let cancellation = CancellationToken::new();
    let router = build_router(auth_config, clock, handlers, store, cancellation.clone());

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
