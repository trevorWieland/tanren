//! Tanren MCP (Model Context Protocol) server — runtime library.
//!
//! Promoted from `bin/tanren-mcp/src/main.rs` per the thin-binary-crate
//! profile. The rmcp tool surface, API-key middleware, and host-header
//! allowlist live here so the BDD harness can exercise via rmcp without
//! spinning up a child process. No cookie jar between client and server.

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
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
use std::path::Path;
use std::sync::Arc;
use tanren_app_services::{AppServiceError, Handlers, Store, project_uninstall};
use tanren_contract::{
    AcceptInvitationRequest, SignInRequest, SignUpRequest, UninstallApplyRequest,
    UninstallPreviewRequest,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";
const DATABASE_URL_ENV: &str = "DATABASE_URL";
/// Comma-separated extra hostnames / `host:port` authorities to add to
/// rmcp's `allowed_hosts` Host-header allowlist.
const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";

/// Configuration for the tanren-mcp runtime (env-driven).
#[derive(Debug, Default)]
pub struct Config;

impl Config {
    /// Construct the default config from environment variables.
    #[must_use]
    pub const fn from_env() -> Self {
        Self
    }
}

/// Encode a serializable value as JSON and inject the
/// `hosted_account_unchanged: true` field before returning an MCP success.
fn success_with_hosted_note<T: Serialize>(value: &T) -> CallToolResult {
    let mut v = serde_json::to_value(value).unwrap_or_else(|_| json!({}));
    if let Some(obj) = v.as_object_mut() {
        obj.insert("hosted_account_unchanged".into(), json!(true));
    }
    let text = serde_json::to_string(&v).unwrap_or_else(|_| "{}".into());
    CallToolResult::success(vec![Content::text(text)])
}

/// MCP tool surface. Holds the shared `Handlers` facade and a `Store`
/// handle so api / mcp / cli / tui surfaces all resolve to the same logic
/// per the equivalent-operations rule in `docs/architecture/subsystems/interfaces.md`.
#[derive(Clone)]
pub(crate) struct TanrenMcp {
    handlers: Handlers,
    store: Arc<Store>,
    /// Cached tool router built from `#[rmcp::tool]` methods.
    tool_router: ToolRouter<Self>,
}

impl std::fmt::Debug for TanrenMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TanrenMcp").finish_non_exhaustive()
    }
}

#[rmcp::tool_router]
impl TanrenMcp {
    fn new(handlers: Handlers, store: Arc<Store>) -> Self {
        Self {
            handlers,
            store,
            tool_router: Self::tool_router(),
        }
    }

    /// Self-signup tool mirroring `POST /accounts`.
    #[rmcp::tool(
        name = "account.create",
        description = "Create a new Tanren account via self-signup. Returns the new account view and an opaque session token. Failures use the shared {code, summary} taxonomy: duplicate_identifier, invalid_credential."
    )]
    async fn account_create(
        &self,
        Parameters(request): Parameters<SignUpRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_up(self.store.as_ref(), request).await {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Sign-in tool mirroring `POST /sessions`.
    #[rmcp::tool(
        name = "account.sign_in",
        description = "Sign in to an existing Tanren account. Returns the account view and an opaque session token. Failure code: invalid_credential."
    )]
    async fn account_sign_in(
        &self,
        Parameters(request): Parameters<SignInRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_in(self.store.as_ref(), request).await {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Invitation-acceptance tool mirroring `POST /invitations/{token}/accept`.
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
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "project.uninstall_preview",
        description = "Preview which Tanren-generated assets would be removed from a repository. No files are deleted. Returns lists of files to remove (unchanged Tanren-generated) and files to preserve (user-owned or modified). Hosted account/project history is not changed by this operation."
    )]
    async fn project_uninstall_preview(
        &self,
        Parameters(request): Parameters<UninstallPreviewRequest>,
    ) -> Result<CallToolResult, McpError> {
        let repo = Path::new(&request.repo_path);
        match project_uninstall::preview(repo) {
            Ok(preview) => Ok(success_with_hosted_note(&preview)),
            Err(err) => Ok(map_uninstall_error(&err)),
        }
    }

    #[rmcp::tool(
        name = "project.uninstall_apply",
        description = "Apply uninstall: remove unchanged Tanren-generated assets from a repository. Requires explicit confirmation (confirm=true). User-owned and modified files are preserved. Hosted account/project history is not changed by this operation."
    )]
    async fn project_uninstall_apply(
        &self,
        Parameters(request): Parameters<UninstallApplyRequest>,
    ) -> Result<CallToolResult, McpError> {
        if !request.confirm {
            let body = json!({
                "code": "confirmation_required",
                "summary": "Uninstall apply requires explicit confirmation. Set 'confirm' to true to proceed. No files were modified."
            });
            let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
            return Ok(CallToolResult::error(vec![Content::text(text)]));
        }
        let repo = Path::new(&request.repo_path);
        match project_uninstall::apply(repo) {
            Ok(result) => Ok(success_with_hosted_note(&result)),
            Err(err) => Ok(map_uninstall_error(&err)),
        }
    }

    /// Borrow the cached `ToolRouter` (keeps dead-code lint happy).
    fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }
}

#[rmcp::tool_handler]
impl ServerHandler for TanrenMcp {
    fn get_info(&self) -> ServerInfo {
        // Touch the cached router to satisfy the dead-code lint.
        let _ = self.router();
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Tanren control plane MCP server. Account-flow tools route through the same handlers the HTTP API uses; project uninstall tools preview and remove Tanren-generated assets from a local repository while preserving user-owned files. Failure responses share the {code, summary} error taxonomy. Hosted account and project history are never changed by uninstall tools."
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

/// Encode an [`AppServiceError`] as an MCP tool failure.
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

fn map_uninstall_error(err: &project_uninstall::UninstallError) -> CallToolResult {
    let code = match &err {
        project_uninstall::UninstallError::NoManifest { .. } => "no_manifest",
        project_uninstall::UninstallError::ReadFailed { .. } => "read_failed",
        project_uninstall::UninstallError::ParseFailed { .. } => "parse_failed",
        project_uninstall::UninstallError::Io(_) => "io_error",
        _ => "internal_error",
    };
    let body = json!({
        "code": code,
        "summary": err.to_string(),
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

/// Shared `{code, summary}` error body.
fn error_body(code: &str, summary: &str) -> serde_json::Value {
    json!({
        "code": code,
        "summary": summary,
    })
}

#[derive(Debug, Clone)]
struct AuthConfig {
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
    // Unconfigured server is an outage, not an auth failure.
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

/// Build rmcp's `StreamableHttpServerConfig` honouring `TANREN_MCP_ALLOWED_HOSTS`.
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
            "Host-header validation disabled by `*`"
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
    tracing::info!(target: "tanren_mcp", allowed_hosts = ?hosts, "Host-header allowlist extended");
    base.with_allowed_hosts(hosts)
}

/// Build the MCP axum router with a caller-supplied `Arc<Store>` and API
/// key. For the BDD wire-harness: the harness owns the database and spawns
/// this router on an ephemeral port.
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

/// Serve the tanren-mcp surface to completion (honours `SIGTERM`/`SIGINT`).
///
/// # Errors
///
/// Returns an error if the database connection, bind, or serve fails.
pub async fn serve(_config: Config) -> Result<()> {
    let bind = env::var(BIND_ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_owned());
    let auth_config = Arc::new(AuthConfig::from_env());
    if auth_config.bootstrap_key.is_none() {
        tracing::warn!(target: "tanren_mcp", env_var = API_KEY_ENV, "TANREN_MCP_API_KEY not set");
    }

    let database_url =
        env::var(DATABASE_URL_ENV).with_context(|| format!("{DATABASE_URL_ENV} must be set"))?;
    let store = Arc::new(
        Store::connect(&database_url)
            .await
            .with_context(|| "connect to store")?,
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
