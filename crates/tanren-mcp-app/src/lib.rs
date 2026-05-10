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

use anyhow::Result;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::{Extension, ToolCallContext};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, InitializeRequestParams, InitializeResult,
    ListToolsResult, PaginatedRequestParams, ServerInfo,
};
use rmcp::service::RequestContext;
use std::sync::Arc;
use tanren_app_services::{Handlers, Store};
use tanren_configuration_secrets::{CredentialSealingFailure, CredentialValueSealer};
use tanren_contract::{
    AcceptInvitationRequest, CreateUserCredentialRequest, ListUserCredentialsRequest,
    ListUserSettingsRequest, OwnerScope, SignInRequest, SignUpRequest, UpdateUserCredentialRequest,
    UpsertUserSettingRequest,
};

pub(crate) const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
pub(crate) const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
pub(crate) const DATABASE_URL_ENV: &str = "DATABASE_URL";
/// Comma-separated extra hostnames / `host:port` authorities to add to
/// rmcp's `allowed_hosts` Host-header allowlist.
pub(crate) const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";
/// Comma-separated explicit origins for MCP CORS.
pub(crate) const CORS_ORIGINS_ENV: &str = "TANREN_MCP_CORS_ORIGINS";

mod auth;
mod server;
mod tool_support;

use crate::auth::ActorCapabilityModel;
use crate::tool_support::{
    AccountScopeParams, AddCredentialParams, RemoveCredentialParams, RemoveUserConfigParams,
    SetUserConfigParams, UpdateCredentialParams, account_principal_from_parts,
    actor_capability_model_from_context, map_failure, parse_account_id, parse_user_credential_id,
    permission_denied_failure, server_info_for_capability_model, success, validation_failure,
};

#[cfg(any(test, feature = "test-hooks"))]
pub use crate::server::build_router_with_store;
pub use crate::server::serve;

/// Configuration for the tanren-mcp runtime. R-0001 sub-8 keeps it
/// env-driven; downstream PRs may swap in a typed config crate without
/// changing the [`serve`] signature.
#[derive(Debug, Clone)]
pub struct Config {
    /// Credential sealing adapter initialized once from environment config.
    pub credential_sealer: Result<CredentialValueSealer, CredentialSealingFailure>,
}

impl Config {
    /// Construct the default config; bind address, allowed hosts, and
    /// API key continue to come from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            credential_sealer: CredentialValueSealer::from_env(),
        }
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

impl std::fmt::Debug for TanrenMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TanrenMcp").finish_non_exhaustive()
    }
}

#[rmcp::tool_router]
impl TanrenMcp {
    pub(crate) fn new(handlers: Handlers, store: Arc<Store>) -> Self {
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

    /// List user-tier settings for one account.
    #[rmcp::tool(
        name = "config.user.list",
        description = "List authenticated user-tier settings for one account id. Returns metadata/value only (no credential secrets)."
    )]
    async fn config_user_list(
        &self,
        Extension(parts): Extension<axum::http::request::Parts>,
        Parameters(request): Parameters<AccountScopeParams>,
    ) -> Result<CallToolResult, McpError> {
        let authenticated_account_id = match account_principal_from_parts(&parts) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let requested_account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .list_user_settings_page(
                self.store.as_ref(),
                authenticated_account_id,
                requested_account_id,
                ListUserSettingsRequest {
                    limit: request.limit,
                    after: request.after,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Upsert one user-tier setting for one account.
    #[rmcp::tool(
        name = "config.user.set",
        description = "Set one authenticated user-tier setting for one account id."
    )]
    async fn config_user_set(
        &self,
        Extension(parts): Extension<axum::http::request::Parts>,
        Parameters(request): Parameters<SetUserConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let authenticated_account_id = match account_principal_from_parts(&parts) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let requested_account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .upsert_user_setting(
                self.store.as_ref(),
                authenticated_account_id,
                requested_account_id,
                UpsertUserSettingRequest {
                    key: request.key,
                    value: request.value,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Remove one user-tier setting for one account.
    #[rmcp::tool(
        name = "config.user.remove",
        description = "Remove one authenticated user-tier setting by key for one account id."
    )]
    async fn config_user_remove(
        &self,
        Extension(parts): Extension<axum::http::request::Parts>,
        Parameters(request): Parameters<RemoveUserConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let authenticated_account_id = match account_principal_from_parts(&parts) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let requested_account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .remove_user_setting(
                self.store.as_ref(),
                authenticated_account_id,
                requested_account_id,
                request.key,
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Add one user-owned credential.
    #[rmcp::tool(
        name = "credential.add",
        description = "Create one user-owned credential. Returns metadata only and never returns raw secret values."
    )]
    async fn credential_add(
        &self,
        Extension(parts): Extension<axum::http::request::Parts>,
        Parameters(request): Parameters<AddCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let authenticated_account_id = match account_principal_from_parts(&parts) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let requested_account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .add_user_credential(
                self.store.as_ref(),
                authenticated_account_id,
                CreateUserCredentialRequest {
                    kind: request.kind,
                    owner_scope: OwnerScope::User {
                        account_id: requested_account_id,
                    },
                    value: request.value,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Update one user-owned credential value.
    #[rmcp::tool(
        name = "credential.update",
        description = "Update one user-owned credential value by metadata id. Response remains metadata-only."
    )]
    async fn credential_update(
        &self,
        Extension(parts): Extension<axum::http::request::Parts>,
        Parameters(request): Parameters<UpdateCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let authenticated_account_id = match account_principal_from_parts(&parts) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let requested_account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        let item_id = match parse_user_credential_id(&request.item_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .update_user_credential(
                self.store.as_ref(),
                authenticated_account_id,
                item_id,
                OwnerScope::User {
                    account_id: requested_account_id,
                },
                UpdateUserCredentialRequest {
                    value: request.value,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// List user-owned credential metadata.
    #[rmcp::tool(
        name = "credential.list",
        description = "List user-owned credential metadata for one account id. Secret values are never returned."
    )]
    async fn credential_list(
        &self,
        Extension(parts): Extension<axum::http::request::Parts>,
        Parameters(request): Parameters<AccountScopeParams>,
    ) -> Result<CallToolResult, McpError> {
        let authenticated_account_id = match account_principal_from_parts(&parts) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let requested_account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .list_user_credentials_page(
                self.store.as_ref(),
                authenticated_account_id,
                OwnerScope::User {
                    account_id: requested_account_id,
                },
                ListUserCredentialsRequest {
                    limit: request.limit,
                    after: request.after,
                },
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Remove one user-owned credential by metadata id.
    #[rmcp::tool(
        name = "credential.remove",
        description = "Remove one user-owned credential by metadata id."
    )]
    async fn credential_remove(
        &self,
        Extension(parts): Extension<axum::http::request::Parts>,
        Parameters(request): Parameters<RemoveCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let authenticated_account_id = match account_principal_from_parts(&parts) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        let requested_account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        let item_id = match parse_user_credential_id(&request.item_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .remove_user_credential(
                self.store.as_ref(),
                authenticated_account_id,
                item_id,
                OwnerScope::User {
                    account_id: requested_account_id,
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
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let capability_model = actor_capability_model_from_context(&context)?;
        if !capability_model.allows_tool(request.name.as_ref()) {
            return Ok(permission_denied_failure(
                "Authenticated principal is not allowed to call this MCP tool.",
            ));
        }
        let tool_context = ToolCallContext::new(self, request, context);
        self.router().call(tool_context).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let capability_model = actor_capability_model_from_context(&context)?;
        let tools = self
            .router()
            .list_all()
            .into_iter()
            .filter(|tool| capability_model.allows_tool(tool.name.as_ref()))
            .collect();
        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request);
        }
        let capability_model = actor_capability_model_from_context(&context)?;
        Ok(server_info_for_capability_model(capability_model))
    }

    fn get_info(&self) -> ServerInfo {
        // Touch the cached router so the dead-code lint never flags
        // `tool_router` even on rmcp macro versions whose tool_handler
        // expansion path uses the static `Self::tool_router()` builder
        // rather than the cached field.
        let _ = self.router();
        server_info_for_capability_model(ActorCapabilityModel::bootstrap())
    }
}
