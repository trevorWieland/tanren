use anyhow::Result;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::service::RequestContext;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tanren_app_services::{AppServiceError, Handlers, RoleServiceError, Store};
use tanren_contract::{
    AcceptInvitationRequest, ApplyRoleRequest, CreateRoleRequest, DeleteRoleRequest,
    EditRoleRequest, PermissionCheckRequest, RoleActor, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::AccountId;

use crate::{SessionActorStore, TanrenMcp};

const INTERNAL_ERROR_SUMMARY: &str = "Tanren encountered an internal error.";
const MCP_SESSION_ID_HEADER: &str = "mcp-session-id";

#[rmcp::tool_router]
impl TanrenMcp {
    pub(crate) fn new(handlers: Handlers, store: Arc<Store>) -> Self {
        Self {
            handlers,
            store,
            tool_router: Self::tool_router(),
        }
    }

    #[rmcp::tool(
        name = "account.create",
        description = "Create a new Tanren account via self-signup. Returns account plus session token. Failure codes: duplicate_identifier, invalid_credential, validation_failed."
    )]
    async fn account_create(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<SignUpRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_up(self.store.as_ref(), request).await {
            Ok(response) => {
                self.set_authenticated_actor(&request_context, response.account.id)
                    .await;
                success(&response)
            }
            Err(err) => map_account_failure(err),
        }
    }

    #[rmcp::tool(
        name = "account.sign_in",
        description = "Sign in to an existing Tanren account. Returns account plus session token. Failure codes: invalid_credential, validation_failed."
    )]
    async fn account_sign_in(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<SignInRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.handlers.sign_in(self.store.as_ref(), request).await {
            Ok(response) => {
                self.set_authenticated_actor(&request_context, response.account.id)
                    .await;
                success(&response)
            }
            Err(err) => map_account_failure(err),
        }
    }

    #[rmcp::tool(
        name = "account.accept_invitation",
        description = "Accept an organization invitation and create a Tanren account in the inviting org. Failure codes: invitation_not_found, invitation_already_consumed, invitation_expired, invalid_credential."
    )]
    async fn account_accept_invitation(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<AcceptInvitationRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .handlers
            .accept_invitation(self.store.as_ref(), request)
            .await
        {
            Ok(response) => {
                self.set_authenticated_actor(&request_context, response.account.id)
                    .await;
                success(&response)
            }
            Err(err) => map_account_failure(err),
        }
    }

    #[rmcp::tool(
        name = "role.create",
        description = "Create a role template. Failure codes: validation_failed, conflict, permission_denied."
    )]
    async fn role_create(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<CreateRoleRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(actor) = self.require_authenticated_actor(&request_context).await else {
            return unauthenticated_role_failure();
        };
        match self
            .handlers
            .create_role(self.store.as_ref(), actor, request)
            .await
        {
            Ok(response) => success(&response),
            Err(err) => map_role_failure(err),
        }
    }

    #[rmcp::tool(
        name = "role.edit",
        description = "Edit a role template. Failure codes: validation_failed, not_found, conflict, permission_denied."
    )]
    async fn role_edit(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<EditRoleRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(actor) = self.require_authenticated_actor(&request_context).await else {
            return unauthenticated_role_failure();
        };
        match self
            .handlers
            .edit_role(self.store.as_ref(), actor, request)
            .await
        {
            Ok(response) => success(&response),
            Err(err) => map_role_failure(err),
        }
    }

    #[rmcp::tool(
        name = "role.delete",
        description = "Delete a role template. Failure codes: validation_failed, not_found, permission_denied."
    )]
    async fn role_delete(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<DeleteRoleRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(actor) = self.require_authenticated_actor(&request_context).await else {
            return unauthenticated_role_failure();
        };
        match self
            .handlers
            .delete_role(self.store.as_ref(), actor, request)
            .await
        {
            Ok(response) => success(&response),
            Err(err) => map_role_failure(err),
        }
    }

    #[rmcp::tool(
        name = "role.apply",
        description = "Apply a role template to a principal. Failure codes: validation_failed, role_as_principal_rejected, not_found, permission_denied."
    )]
    async fn role_apply(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<ApplyRoleRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(actor) = self.require_authenticated_actor(&request_context).await else {
            return unauthenticated_role_failure();
        };
        match self
            .handlers
            .apply_role(self.store.as_ref(), actor, request)
            .await
        {
            Ok(response) => success(&response),
            Err(err) => map_role_failure(err),
        }
    }

    #[rmcp::tool(
        name = "permission.check",
        description = "Check whether a principal has a permission in a scope. Failure codes: validation_failed, role_as_principal_rejected, permission_denied."
    )]
    async fn permission_check(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(request): Parameters<PermissionCheckRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(actor) = self.require_authenticated_actor(&request_context).await else {
            return unauthenticated_role_failure();
        };
        match self
            .handlers
            .check_permission(self.store.as_ref(), actor, request)
            .await
        {
            Ok(response) => success(&response),
            Err(err) => map_role_failure(err),
        }
    }

    #[rmcp::tool(
        name = "role.capabilities",
        description = "Discover role administration capabilities for the authenticated MCP actor."
    )]
    async fn role_capabilities(
        &self,
        request_context: RequestContext<RoleServer>,
        Parameters(_): Parameters<RoleCapabilitiesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let Some(actor) = self.require_authenticated_actor(&request_context).await else {
            return unauthenticated_role_failure();
        };
        match self
            .handlers
            .role_admin_capabilities(self.store.as_ref(), actor)
            .await
        {
            Ok(response) => success(&response),
            Err(err) => map_role_failure(err),
        }
    }

    pub(crate) fn router(&self) -> &ToolRouter<Self> {
        &self.tool_router
    }

    async fn set_authenticated_actor(
        &self,
        request_context: &RequestContext<RoleServer>,
        account_id: AccountId,
    ) {
        let Some(session_id) = session_id(request_context) else {
            tracing::warn!(
                target: "tanren_mcp",
                "missing MCP session id while binding authenticated actor"
            );
            return;
        };
        let Some(store) = session_actor_store(request_context) else {
            tracing::error!(
                target: "tanren_mcp",
                "missing session actor store while binding authenticated actor"
            );
            return;
        };
        let mut actors = store.write().await;
        actors.insert(session_id, account_id);
    }

    async fn require_authenticated_actor(
        &self,
        request_context: &RequestContext<RoleServer>,
    ) -> Option<RoleActor> {
        let session_id = session_id(request_context)?;
        let store = session_actor_store(request_context)?;
        let actors = store.read().await;
        actors
            .get(&session_id)
            .copied()
            .map(|account_id| RoleActor { account_id })
    }
}

#[rmcp::tool_handler]
impl ServerHandler for TanrenMcp {
    fn get_info(&self) -> ServerInfo {
        let _ = self.router();
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Tanren control plane MCP server. Account and role tools route through the same handlers the HTTP API uses; failure responses share the {code, summary} error taxonomy."
                .to_owned(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

fn success<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    serde_json::to_string(value)
        .map(|text| CallToolResult::success(vec![Content::text(text)]))
        .map_err(|err| {
            tracing::error!(target: "tanren_mcp", error = %err, "encode tool success payload");
            internal_error_mcp_failure()
        })
}

fn map_account_failure(err: AppServiceError) -> Result<CallToolResult, McpError> {
    let (code, summary) = match err {
        AppServiceError::Account(reason) => (reason.code().to_owned(), reason.summary().to_owned()),
        AppServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message),
        AppServiceError::Store(err) => {
            tracing::error!(target: "tanren_mcp", error = %err, "store error");
            (
                "internal_error".to_owned(),
                INTERNAL_ERROR_SUMMARY.to_owned(),
            )
        }
        _ => (
            "internal_error".to_owned(),
            INTERNAL_ERROR_SUMMARY.to_owned(),
        ),
    };
    failure(&code, &summary)
}

fn map_role_failure(err: RoleServiceError) -> Result<CallToolResult, McpError> {
    let (code, summary) = match err {
        RoleServiceError::Role(reason) => (reason.code().to_owned(), reason.summary().to_owned()),
        RoleServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message),
        RoleServiceError::Store(err) => {
            tracing::error!(target: "tanren_mcp", error = %err, "store error");
            (
                "internal_error".to_owned(),
                INTERNAL_ERROR_SUMMARY.to_owned(),
            )
        }
        _ => (
            "internal_error".to_owned(),
            INTERNAL_ERROR_SUMMARY.to_owned(),
        ),
    };
    failure(&code, &summary)
}

fn failure(code: &str, summary: &str) -> Result<CallToolResult, McpError> {
    let body = json!({ "code": code, "summary": summary });
    serde_json::to_string(&body)
        .map(|text| CallToolResult::error(vec![Content::text(text)]))
        .map_err(|err| {
            tracing::error!(target: "tanren_mcp", error = %err, "encode tool error payload");
            internal_error_mcp_failure()
        })
}

fn unauthenticated_role_failure() -> Result<CallToolResult, McpError> {
    failure(
        "permission_denied",
        "Authentication is required for role operations. Call account.sign_in, account.create, or account.accept_invitation first in this MCP session.",
    )
}

fn internal_error_mcp_failure() -> McpError {
    McpError::internal_error(
        INTERNAL_ERROR_SUMMARY,
        Some(json!({
            "code": "internal_error",
            "summary": INTERNAL_ERROR_SUMMARY,
        })),
    )
}

fn session_id(request_context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = request_context
        .extensions
        .get::<axum::http::request::Parts>()?;
    let session_id = parts.headers.get(MCP_SESSION_ID_HEADER)?;
    let session_id = session_id.to_str().ok()?.trim();
    if session_id.is_empty() {
        return None;
    }
    Some(session_id.to_owned())
}

fn session_actor_store(request_context: &RequestContext<RoleServer>) -> Option<SessionActorStore> {
    let parts = request_context
        .extensions
        .get::<axum::http::request::Parts>()?;
    parts.extensions.get::<SessionActorStore>().cloned()
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct RoleCapabilitiesRequest {}
