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

use anyhow::{Context, Result};
use axum::Router;
use axum::middleware;
use axum::routing::get;
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
};
use serde::Serialize;
use serde_json::json;
use std::env;
use std::sync::Arc;
use tanren_app_services::project::{
    ActiveProjectQuery, ConnectExistingRepositoryCommand, CreateNewProjectCommand,
    ListVisibleProjectsQuery,
};
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, ActiveProjectRequest, ConnectProjectRepositoryRequest,
    CreateProjectRequest, ListVisibleProjectsRequest, SignInRequest, SignUpRequest,
};
use tanren_provider_integrations::AllowAllSourceControlProvider;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

mod http;

use crate::http::{AuthConfig, health, require_api_key, streamable_http_config};

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";
const DATABASE_URL_ENV: &str = "DATABASE_URL";

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
    handlers: Handlers,
    store: Arc<Store>,
    source_control: Arc<AllowAllSourceControlProvider>,
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
    fn new(
        handlers: Handlers,
        store: Arc<Store>,
        source_control: Arc<AllowAllSourceControlProvider>,
    ) -> Self {
        Self {
            handlers,
            store,
            source_control,
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
            Ok(response) => Ok(success(&response)),
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
            Ok(response) => Ok(success(&response)),
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
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Connect an existing repository as a project.
    #[rmcp::tool(
        name = "project.connect_repository",
        description = "Connect an existing repository as a Tanren project. Failure codes: duplicate_repository, no_access, validation_failed, provider_failure."
    )]
    async fn project_connect_repository(
        &self,
        Parameters(request): Parameters<ConnectProjectRepositoryRequest>,
    ) -> Result<CallToolResult, McpError> {
        let actor_account_id = request.owning_account_id;
        match self
            .handlers
            .connect_project_repository(
                self.store.as_ref(),
                self.source_control.as_ref(),
                ConnectExistingRepositoryCommand {
                    actor_account_id,
                    request,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Create a project by creating a repository at a designated host first.
    #[rmcp::tool(
        name = "project.create",
        description = "Create a repository at a designated host and register it as a Tanren project. Failure codes: duplicate_repository, no_access, validation_failed, provider_failure."
    )]
    async fn project_create(
        &self,
        Parameters(request): Parameters<CreateProjectRequest>,
    ) -> Result<CallToolResult, McpError> {
        let actor_account_id = request.owning_account_id;
        match self
            .handlers
            .create_project(
                self.store.as_ref(),
                self.source_control.as_ref(),
                CreateNewProjectCommand {
                    actor_account_id,
                    request,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// List projects visible to the owning account.
    #[rmcp::tool(
        name = "project.list_visible",
        description = "List projects visible to an account, including active marker and empty counts."
    )]
    async fn project_list_visible(
        &self,
        Parameters(request): Parameters<ListVisibleProjectsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let actor_account_id = request.owning_account_id;
        match self
            .handlers
            .list_visible_projects(
                self.store.as_ref(),
                ListVisibleProjectsQuery {
                    actor_account_id,
                    request,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Read active-project metadata for an account.
    #[rmcp::tool(
        name = "project.active",
        description = "Read active-project metadata for an account."
    )]
    async fn project_active(
        &self,
        Parameters(request): Parameters<ActiveProjectRequest>,
    ) -> Result<CallToolResult, McpError> {
        let actor_account_id = request.owning_account_id;
        match self
            .handlers
            .active_project(
                self.store.as_ref(),
                ActiveProjectQuery {
                    actor_account_id,
                    request,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
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
    let (code, summary, status) = match err {
        AppServiceError::Account(reason) => (
            reason.code().to_owned(),
            reason.summary().to_owned(),
            reason.http_status(),
        ),
        AppServiceError::Project(reason) => (
            reason.code().to_owned(),
            reason.summary().to_owned(),
            reason.http_status(),
        ),
        AppServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message, 400),
        AppServiceError::Store(err) => (
            "internal_error".to_owned(),
            format!("Tanren encountered an internal error: {err}"),
            500,
        ),
        _ => (
            "internal_error".to_owned(),
            "Unknown app-service failure".to_owned(),
            500,
        ),
    };
    let body = json!({
        "code": code,
        "summary": summary,
        "status": status,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}

fn build_router(
    auth_config: Arc<AuthConfig>,
    handlers: Handlers,
    store: Arc<Store>,
    source_control: Arc<AllowAllSourceControlProvider>,
    cancellation: CancellationToken,
) -> Router {
    let config = streamable_http_config(cancellation);
    let mcp_service: StreamableHttpService<TanrenMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                Ok(TanrenMcp::new(
                    handlers.clone(),
                    store.clone(),
                    source_control.clone(),
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
    let router = build_router(
        auth_config,
        Handlers::new(),
        store,
        Arc::new(AllowAllSourceControlProvider),
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
    let handlers = Handlers::new();
    let source_control = Arc::new(AllowAllSourceControlProvider);

    let cancellation = CancellationToken::new();
    let router = build_router(
        auth_config,
        handlers,
        store,
        source_control,
        cancellation.clone(),
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

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!(target: "tanren_mcp", "shutdown signal received");
}
