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

mod server;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{
    CreateCredentialRequest, GetUserConfigRequest, RemoveCredentialRequest,
    RemoveUserConfigRequest, SetUserConfigRequest, UpdateCredentialRequest,
};
use tanren_identity_policy::{SessionToken, secret_serde};

pub use server::{build_router_with_store, serve};

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
    tool_router: ToolRouter<Self>,
}

impl std::fmt::Debug for TanrenMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TanrenMcp").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HealthResponse {
    status: String,
    version: String,
    contract_version: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SessionParams {
    session_token: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SetUserConfigParams {
    session_token: String,
    key: UserSettingKey,
    value: UserSettingValue,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct RemoveUserConfigParams {
    session_token: String,
    key: UserSettingKey,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CreateCredentialParams {
    session_token: String,
    kind: CredentialKind,
    name: String,
    description: Option<String>,
    provider: Option<String>,
    #[schemars(with = "String")]
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    value: SecretString,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct UpdateCredentialParams {
    session_token: String,
    id: CredentialId,
    name: Option<String>,
    description: Option<String>,
    #[schemars(with = "String")]
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    value: SecretString,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct RemoveCredentialParams {
    session_token: String,
    id: CredentialId,
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

    fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    async fn authenticate(
        &self,
        session_token: &str,
    ) -> Result<tanren_app_services::AuthenticatedActor, AppServiceError> {
        let token = SessionToken::from_secret(SecretString::from(session_token.to_owned()));
        self.handlers
            .resolve_actor(self.store.as_ref(), &token)
            .await
    }

    #[rmcp::tool(
        name = "account.create",
        description = "Create a new Tanren account via self-signup."
    )]
    async fn account_create(
        &self,
        Parameters(request): Parameters<tanren_contract::SignUpRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_up(self.store.as_ref(), request).await {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "account.sign_in",
        description = "Sign in to an existing Tanren account."
    )]
    async fn account_sign_in(
        &self,
        Parameters(request): Parameters<tanren_contract::SignInRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_in(self.store.as_ref(), request).await {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "account.accept_invitation",
        description = "Accept an organization invitation and create an account."
    )]
    async fn account_accept_invitation(
        &self,
        Parameters(request): Parameters<tanren_contract::AcceptInvitationRequest>,
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
        name = "user_config.list",
        description = "List all user-tier configuration entries. Returns metadata only."
    )]
    async fn user_config_list(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .list_user_config(self.store.as_ref(), &actor)
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "user_config.get",
        description = "Get a single user-tier configuration value."
    )]
    async fn user_config_get(
        &self,
        Parameters(params): Parameters<SetUserConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .get_user_config(
                self.store.as_ref(),
                &actor,
                GetUserConfigRequest { key: params.key },
            )
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "user_config.set",
        description = "Set (upsert) a user-tier configuration value."
    )]
    async fn user_config_set(
        &self,
        Parameters(params): Parameters<SetUserConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .set_user_config(
                self.store.as_ref(),
                &actor,
                SetUserConfigRequest {
                    key: params.key,
                    value: params.value,
                },
            )
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "user_config.remove",
        description = "Remove a user-tier configuration value."
    )]
    async fn user_config_remove(
        &self,
        Parameters(params): Parameters<RemoveUserConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .remove_user_config(
                self.store.as_ref(),
                &actor,
                RemoveUserConfigRequest { key: params.key },
            )
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "credential.list",
        description = "List credentials. Returns metadata only — stored values are never projected."
    )]
    async fn credential_list(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .list_credentials(self.store.as_ref(), &actor)
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "credential.add",
        description = "Add a new user-owned credential. The stored value is never returned."
    )]
    async fn credential_add(
        &self,
        Parameters(params): Parameters<CreateCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .create_credential(
                self.store.as_ref(),
                &actor,
                CreateCredentialRequest {
                    kind: params.kind,
                    name: params.name,
                    description: params.description,
                    provider: params.provider,
                    value: params.value,
                },
            )
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "credential.update",
        description = "Update an existing credential. The stored value is never returned."
    )]
    async fn credential_update(
        &self,
        Parameters(params): Parameters<UpdateCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .update_credential(
                self.store.as_ref(),
                &actor,
                UpdateCredentialRequest {
                    id: params.id,
                    name: params.name,
                    description: params.description,
                    value: params.value,
                },
            )
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "credential.remove",
        description = "Remove a user-owned credential."
    )]
    async fn credential_remove(
        &self,
        Parameters(params): Parameters<RemoveCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .remove_credential(
                self.store.as_ref(),
                &actor,
                RemoveCredentialRequest { id: params.id },
            )
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }
}

#[rmcp::tool_handler]
impl ServerHandler for TanrenMcp {
    fn get_info(&self) -> ServerInfo {
        let _ = self.router();
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Tanren control plane MCP server. Tools route through shared handlers; failures use the {code, summary} error taxonomy.".to_owned(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

fn success<T: Serialize>(value: &T) -> CallToolResult {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::success(vec![Content::text(text)])
}

fn map_failure(err: AppServiceError) -> CallToolResult {
    let (code, summary) = match err {
        AppServiceError::Account(reason) => (reason.code().to_owned(), reason.summary().to_owned()),
        AppServiceError::Configuration(reason) => {
            (reason.code().to_owned(), reason.summary().to_owned())
        }
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
    let body = json!({ "code": code, "summary": summary });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}
