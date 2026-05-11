//! Tanren MCP (Model Context Protocol) server — runtime library.
//! Runtime moved out of `bin/tanren-mcp/src/main.rs` per thin-binary-crate:
//! this crate owns the rmcp tool surface, API-key middleware, and host-header
//! allowlist so the BDD harness can exercise it directly.
//! The MCP surface returns bearer-mode `SessionView` responses (no cookie jar).

mod auth;
mod organization_errors;

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
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountId, CheckOrganizationPermissionRequest,
    CreateOrganizationRequest, IdempotencyKey, ListOrganizationsRequest, MembershipId, OrgId,
    OrganizationName, OrganizationPermission, SessionToken, SignInRequest, SignUpRequest,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::{API_KEY_ENV, AuthConfig, require_api_key};
use crate::organization_errors::{map_organization_failure, organization_auth_required_failure};

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
    handlers: Handlers,
    store: Arc<Store>,
    /// Cached tool router built from the `#[rmcp::tool]` methods on this
    /// type. Read by the macro-generated `ServerHandler` impl below.
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
struct CreateOrganizationToolRequest {
    session_token: Option<SessionToken>,
    account_id: AccountId,
    name: OrganizationName,
    idempotency_key: Option<IdempotencyKey>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
struct ListOrganizationsToolRequest {
    session_token: Option<SessionToken>,
    account_id: AccountId,
    limit: Option<u64>,
    cursor: Option<MembershipId>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
struct CheckOrganizationPermissionToolRequest {
    session_token: Option<SessionToken>,
    account_id: AccountId,
    org_id: OrgId,
    permission: OrganizationPermission,
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

    #[rmcp::tool(
        name = "organization.create",
        description = "Create an organization for a signed-in account. Failure codes: auth_required, validation_failed, conflict, idempotency_conflict."
    )]
    async fn organization_create(
        &self,
        Parameters(request): Parameters<CreateOrganizationToolRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(session_token) = request.session_token else {
            return Ok(organization_auth_required_failure());
        };
        match self
            .handlers
            .create_organization(
                self.store.as_ref(),
                CreateOrganizationRequest::from_api(
                    session_token,
                    request.account_id,
                    tanren_contract::CreateOrganizationApiRequest::new(
                        request.name,
                        request.idempotency_key,
                    ),
                ),
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_organization_failure(&err)),
        }
    }

    #[rmcp::tool(
        name = "organization.list",
        description = "List organizations available to a signed-in account. Failure code: auth_required."
    )]
    async fn organization_list(
        &self,
        Parameters(request): Parameters<ListOrganizationsToolRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(session_token) = request.session_token else {
            return Ok(organization_auth_required_failure());
        };
        match self
            .handlers
            .list_organizations(
                self.store.as_ref(),
                ListOrganizationsRequest::from_api_query(
                    session_token,
                    request.account_id,
                    &tanren_contract::ListOrganizationsApiQuery {
                        limit: request.limit,
                        cursor: request.cursor,
                    },
                ),
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_organization_failure(&err)),
        }
    }

    #[rmcp::tool(
        name = "organization.check_permission",
        description = "Check an org admin permission for the signed-in account. Returns permission_denied when not granted."
    )]
    async fn organization_check_permission(
        &self,
        Parameters(request): Parameters<CheckOrganizationPermissionToolRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(session_token) = request.session_token else {
            return Ok(organization_auth_required_failure());
        };
        match self
            .handlers
            .check_organization_permission(
                self.store.as_ref(),
                CheckOrganizationPermissionRequest::from_api(
                    session_token,
                    request.account_id,
                    &tanren_contract::CheckOrganizationPermissionApiRequest::new(
                        request.org_id,
                        request.permission,
                    ),
                ),
            )
            .await
        {
            Ok(response) if response.allowed => Ok(success(&response)),
            Ok(_) => {
                let err = AppServiceError::Account(AccountFailureReason::PermissionDenied);
                Ok(map_organization_failure(&err))
            }
            Err(err) => Ok(map_organization_failure(&err)),
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
        AppServiceError::CreateOrganization(reason) => {
            (reason.code().to_owned(), reason.summary().to_owned())
        }
        AppServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message),
        AppServiceError::Store(err) => {
            tracing::error!(target: "tanren_mcp", error = %err, "store error");
            (
                "internal_error".to_owned(),
                "Tanren encountered an internal error.".to_owned(),
            )
        }
        _ => (
            "internal_error".to_owned(),
            "Tanren encountered an internal error.".to_owned(),
        ),
    };
    let body = json!({
        "code": code,
        "summary": summary,
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
) -> Result<(Router, CancellationToken)> {
    let auth_config = Arc::new(AuthConfig::from_bootstrap_key(api_key)?);
    let cancellation = CancellationToken::new();
    let router = build_router(auth_config, Handlers::new(), store, cancellation.clone());
    Ok((router, cancellation))
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
    let auth_config = Arc::new(AuthConfig::from_env().context("load MCP auth config")?);
    if !auth_config.is_configured() {
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
