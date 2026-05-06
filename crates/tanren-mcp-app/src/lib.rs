mod server;
mod tools_support;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ServerCapabilities, ServerInfo};
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    CreateCredentialRequest, GetUserConfigRequest, RemoveCredentialRequest,
    RemoveUserConfigRequest, SetUserConfigRequest, UpdateCredentialRequest,
};
use tanren_identity_policy::SessionToken;

use tools_support::{
    CreateCredentialParams, EvaluateNotificationRouteParams, RemoveCredentialParams,
    RemoveUserConfigParams, SessionParams, SetNotificationPreferencesParams,
    SetOrganizationNotificationOverridesParams, SetUserConfigParams, UpdateCredentialParams,
    map_failure, success,
};

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

    #[rmcp::tool(
        name = "notification.list_preferences",
        description = "List all notification preferences for the authenticated user."
    )]
    async fn notification_list_preferences(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .list_notification_preferences(self.store.as_ref(), &actor)
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "notification.set_preferences",
        description = "Set (upsert) notification preferences for the authenticated user."
    )]
    async fn notification_set_preferences(
        &self,
        Parameters(params): Parameters<SetNotificationPreferencesParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .set_notification_preferences(self.store.as_ref(), &actor, params.request)
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "notification.set_org_overrides",
        description = "Set (upsert) organization-level notification overrides."
    )]
    async fn notification_set_org_overrides(
        &self,
        Parameters(params): Parameters<SetOrganizationNotificationOverridesParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .set_organization_notification_overrides(self.store.as_ref(), &actor, params.request)
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "notification.evaluate_route",
        description = "Evaluate which channels an event would be routed through."
    )]
    async fn notification_evaluate_route(
        &self,
        Parameters(params): Parameters<EvaluateNotificationRouteParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .evaluate_notification_route(self.store.as_ref(), &actor, params.request)
            .await
        {
            Ok(resp) => Ok(success(&resp)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    #[rmcp::tool(
        name = "notification.read_pending_snapshot",
        description = "Read the current pending routing snapshot for the authenticated user."
    )]
    async fn notification_read_pending_snapshot(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let actor = match self.authenticate(&params.session_token).await {
            Ok(a) => a,
            Err(err) => return Ok(map_failure(err)),
        };
        match self
            .handlers
            .read_pending_routing_snapshot(self.store.as_ref(), &actor)
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
