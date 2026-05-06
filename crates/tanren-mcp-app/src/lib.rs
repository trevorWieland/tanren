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

mod transport;

pub use transport::{Config, serve};

#[cfg(any(test, feature = "test-hooks"))]
pub use transport::build_router_with_store;

use chrono::{DateTime, Utc};
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, CreateOrgInvitationRequest, RevokeOrgInvitationRequest, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, OrgId, OrganizationPermission,
};

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

    /// Create an organization invitation. Mirrors the api
    /// `POST /organizations/{org_id}/invitations` shape.
    #[rmcp::tool(
        name = "invitation.create",
        description = "Create an organization invitation. The caller must be an org admin. Returns the new invitation view. Failure codes: permission_denied, personal_context."
    )]
    async fn invitation_create(
        &self,
        Parameters(params): Parameters<CreateInvitationParams>,
    ) -> Result<CallToolResult, McpError> {
        let org_id = params.org_id;
        let request = CreateOrgInvitationRequest {
            org_id,
            recipient_identifier: params.recipient_identifier,
            permissions: params.permissions,
            expires_at: params.expires_at,
        };
        match self
            .handlers
            .create_invitation(
                self.store.as_ref(),
                params.caller_account_id,
                Some(org_id),
                request,
            )
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// List all invitations for an organization (admin view). Mirrors
    /// the api `GET /organizations/{org_id}/invitations` shape.
    #[rmcp::tool(
        name = "invitation.list_org",
        description = "List all invitations for an organization. The caller must be an org admin. Failure codes: permission_denied."
    )]
    async fn invitation_list_org(
        &self,
        Parameters(params): Parameters<ListOrgInvitationsParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .handlers
            .list_org_invitations(self.store.as_ref(), params.caller_account_id, params.org_id)
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// List pending invitations addressed to a recipient identifier.
    /// Mirrors the api `GET /invitations?recipient_identifier=…` shape.
    #[rmcp::tool(
        name = "invitation.list_recipient",
        description = "List pending invitations addressed to a recipient identifier (email or other handle). Returns matching invitation views."
    )]
    async fn invitation_list_recipient(
        &self,
        Parameters(params): Parameters<ListRecipientInvitationsParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .handlers
            .list_recipient_invitations(self.store.as_ref(), &params.recipient_identifier)
            .await
        {
            Ok(response) => Ok(success(&response)),
            Err(err) => Ok(map_failure(err)),
        }
    }

    /// Revoke a pending organization invitation. Mirrors the api
    /// `POST /organizations/{org_id}/invitations/{token}/revoke` shape.
    #[rmcp::tool(
        name = "invitation.revoke",
        description = "Revoke a pending organization invitation. The caller must be an org admin. Failure codes: permission_denied, personal_context, invitation_not_found, invitation_already_consumed, invitation_revoked."
    )]
    async fn invitation_revoke(
        &self,
        Parameters(params): Parameters<RevokeInvitationParams>,
    ) -> Result<CallToolResult, McpError> {
        let org_id = params.org_id;
        let request = RevokeOrgInvitationRequest {
            org_id,
            token: params.token,
        };
        match self
            .handlers
            .revoke_invitation(
                self.store.as_ref(),
                params.caller_account_id,
                Some(org_id),
                request,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct CreateInvitationParams {
    caller_account_id: AccountId,
    org_id: OrgId,
    recipient_identifier: Identifier,
    permissions: Vec<OrganizationPermission>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ListOrgInvitationsParams {
    caller_account_id: AccountId,
    org_id: OrgId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ListRecipientInvitationsParams {
    recipient_identifier: Identifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RevokeInvitationParams {
    caller_account_id: AccountId,
    org_id: OrgId,
    token: InvitationToken,
}
