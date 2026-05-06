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

mod actor;
mod auth;
mod response;

use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::middleware;
use axum::routing::get;
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde::{Deserialize, Serialize};
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, ActiveProjectView, ConnectProjectRequest, CreateProjectRequest,
    ProjectFailureReason, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::OrgId;
use tanren_policy::{ActorContext, Decision, ScopeTarget, authorize_project_registration};
use tanren_provider_integrations::SourceControlProvider;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::AuthConfig;
use crate::response::{map_failure, success};

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";
const DATABASE_URL_ENV: &str = "DATABASE_URL";
const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";

#[derive(Debug, Default)]
pub struct Config;

impl Config {
    #[must_use]
    pub const fn from_env() -> Self {
        Self
    }
}

#[derive(Clone)]
pub(crate) struct TanrenMcp {
    handlers: Handlers,
    store: Arc<Store>,
    provider: Option<Arc<dyn SourceControlProvider>>,
    tool_router: ToolRouter<Self>,
    actor_context: Option<ActorContext>,
}

impl std::fmt::Debug for TanrenMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TanrenMcp").finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ProjectConnectParams {
    name: String,
    repository_url: String,
    #[serde(default)]
    org: Option<OrgId>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ProjectCreateParams {
    name: String,
    provider_host: String,
    #[serde(default)]
    org: Option<OrgId>,
}

#[rmcp::tool_router]
impl TanrenMcp {
    fn new(
        handlers: Handlers,
        store: Arc<Store>,
        provider: Option<Arc<dyn SourceControlProvider>>,
        actor_context: Option<ActorContext>,
    ) -> Self {
        Self {
            handlers,
            store,
            provider,
            actor_context,
            tool_router: Self::tool_router(),
        }
    }

    fn require_actor(&self) -> Result<&ActorContext, CallToolResult> {
        self.actor_context.as_ref().ok_or_else(|| {
            map_failure(AppServiceError::Project(ProjectFailureReason::AccessDenied))
        })
    }

    fn check_scope_policy(
        actor: &ActorContext,
        requested_org: Option<OrgId>,
    ) -> Result<(), CallToolResult> {
        let target = match requested_org {
            Some(org) => ScopeTarget::Org(org),
            None => ScopeTarget::Personal,
        };
        match authorize_project_registration(actor, &target) {
            Decision::Allow => Ok(()),
            Decision::Deny(_) => Err(map_failure(AppServiceError::Project(
                ProjectFailureReason::AccessDenied,
            ))),
        }
    }

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
        name = "project.connect",
        description = "Connect an existing repository as a new Tanren project (B-0025). Returns the project view. Failure codes: access_denied, duplicate_repository, validation_failed, provider_failure, provider_not_configured."
    )]
    async fn project_connect(
        &self,
        Parameters(params): Parameters<ProjectConnectParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.require_actor() {
            Ok(a) => a,
            Err(result) => return Ok(result),
        };
        if let Err(result) = Self::check_scope_policy(actor, params.org) {
            return Ok(result);
        }
        let Some(provider) = self.provider.as_deref() else {
            return Ok(map_failure(AppServiceError::Project(
                ProjectFailureReason::ProviderNotConfigured,
            )));
        };
        let request = ConnectProjectRequest {
            name: params.name,
            repository_url: params.repository_url,
            org: params.org,
        };
        match self
            .handlers
            .connect_project(self.store.as_ref(), provider, actor.account_id, request)
            .await
        {
            Ok(project) => Ok(success(&project)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "project.create",
        description = "Create a new Tanren project and its backing repository (B-0026). Returns the project view. Failure codes: access_denied, duplicate_repository, validation_failed, provider_failure, provider_not_configured."
    )]
    async fn project_create(
        &self,
        Parameters(params): Parameters<ProjectCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.require_actor() {
            Ok(a) => a,
            Err(result) => return Ok(result),
        };
        if let Err(result) = Self::check_scope_policy(actor, params.org) {
            return Ok(result);
        }
        let Some(provider) = self.provider.as_deref() else {
            return Ok(map_failure(AppServiceError::Project(
                ProjectFailureReason::ProviderNotConfigured,
            )));
        };
        let request = CreateProjectRequest {
            name: params.name,
            provider_host: params.provider_host,
            org: params.org,
        };
        match self
            .handlers
            .create_project(self.store.as_ref(), provider, actor.account_id, request)
            .await
        {
            Ok(project) => Ok(success(&project)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "project.active",
        description = "Read back the caller's currently active project. Returns the active project view or null when no project is active."
    )]
    async fn project_active(&self) -> Result<CallToolResult, McpError> {
        let actor = match self.require_actor() {
            Ok(a) => a,
            Err(result) => return Ok(result),
        };
        match self
            .handlers
            .active_project(self.store.as_ref(), actor.account_id)
            .await
        {
            Ok(Some(view)) => Ok(success(&view)),
            Ok(None) => Ok(success(&Option::<ActiveProjectView>::None)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }
}

#[rmcp::tool_handler]
impl ServerHandler for TanrenMcp {
    fn get_info(&self) -> ServerInfo {
        let _ = self.router();
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Tanren control plane MCP server. Account-flow and project-flow tools route through the same handlers the HTTP API uses; failure responses share the {code, summary} error taxonomy."
                .to_owned(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
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

fn build_router(
    auth_config: Arc<AuthConfig>,
    handlers: Handlers,
    store: Arc<Store>,
    cancellation: CancellationToken,
    provider: Option<Arc<dyn SourceControlProvider>>,
    actor_context: Option<ActorContext>,
) -> Router {
    let config = streamable_http_config(cancellation);
    let mcp_service: StreamableHttpService<TanrenMcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                Ok(TanrenMcp::new(
                    handlers.clone(),
                    store.clone(),
                    provider.clone(),
                    actor_context,
                ))
            },
            Arc::new(LocalSessionManager::default()),
            config,
        );

    let mcp_with_auth = ServiceBuilder::new()
        .layer(middleware::from_fn_with_state(
            auth_config,
            auth::require_api_key,
        ))
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

#[cfg(any(test, feature = "test-hooks"))]
pub fn build_router_with_store(
    store: Arc<Store>,
    api_key: secrecy::SecretString,
    provider: Option<Arc<dyn SourceControlProvider>>,
) -> (Router, CancellationToken) {
    build_router_with_actor(store, api_key, provider, None)
}

#[cfg(any(test, feature = "test-hooks"))]
pub fn build_router_with_actor(
    store: Arc<Store>,
    api_key: secrecy::SecretString,
    provider: Option<Arc<dyn SourceControlProvider>>,
    actor_context: Option<ActorContext>,
) -> (Router, CancellationToken) {
    let auth_config = Arc::new(AuthConfig {
        bootstrap_key: Some(api_key),
    });
    let cancellation = CancellationToken::new();
    let router = build_router(
        auth_config,
        Handlers::new(),
        store,
        cancellation.clone(),
        provider,
        actor_context,
    );
    (router, cancellation)
}

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

    let actor_context = actor::resolve_serve_actor_context(&database_url).await;

    let cancellation = CancellationToken::new();
    let router = build_router(
        auth_config,
        handlers,
        store,
        cancellation.clone(),
        None,
        actor_context,
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
