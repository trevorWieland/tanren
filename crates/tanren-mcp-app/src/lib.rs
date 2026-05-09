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
use rmcp::ServerHandler;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_configuration_secrets::{OwnerScope, UserSettingKey, UserSettingValue};
use tanren_contract::{
    AcceptInvitationRequest, CreateUserCredentialRequest, SignInRequest, SignUpRequest,
    UpdateUserCredentialRequest, UpsertUserSettingRequest,
};
use tanren_identity_policy::AccountId;
use uuid::Uuid;

pub(crate) const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8081";
pub(crate) const BIND_ADDRESS_ENV: &str = "TANREN_MCP_BIND";
pub(crate) const DATABASE_URL_ENV: &str = "DATABASE_URL";
/// Comma-separated extra hostnames / `host:port` authorities to add to
/// rmcp's `allowed_hosts` Host-header allowlist.
pub(crate) const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";

mod auth;
mod server;

#[cfg(any(test, feature = "test-hooks"))]
pub use crate::server::build_router_with_store;
pub use crate::server::serve;

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
        Parameters(request): Parameters<AccountScopeParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .list_user_settings(self.store.as_ref(), account_id, account_id)
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
        Parameters(request): Parameters<SetUserConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .upsert_user_setting(
                self.store.as_ref(),
                account_id,
                account_id,
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
        Parameters(request): Parameters<RemoveUserConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .remove_user_setting(self.store.as_ref(), account_id, account_id, request.key)
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
        Parameters(request): Parameters<AddCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .add_user_credential(
                self.store.as_ref(),
                account_id,
                CreateUserCredentialRequest {
                    kind: request.kind,
                    owner_scope: OwnerScope::User { account_id },
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
        Parameters(request): Parameters<UpdateCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .update_user_credential(
                self.store.as_ref(),
                account_id,
                &request.item_id,
                OwnerScope::User { account_id },
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
        Parameters(request): Parameters<AccountScopeParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .list_user_credentials(
                self.store.as_ref(),
                account_id,
                OwnerScope::User { account_id },
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
        Parameters(request): Parameters<RemoveCredentialParams>,
    ) -> Result<CallToolResult, McpError> {
        let account_id = match parse_account_id(&request.account_id) {
            Ok(value) => value,
            Err(summary) => return Ok(validation_failure(&summary)),
        };
        match self
            .handlers
            .remove_user_credential(
                self.store.as_ref(),
                account_id,
                &request.item_id,
                OwnerScope::User { account_id },
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct AccountScopeParams {
    account_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct SetUserConfigParams {
    account_id: String,
    key: UserSettingKey,
    value: UserSettingValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RemoveUserConfigParams {
    account_id: String,
    key: UserSettingKey,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct AddCredentialParams {
    account_id: String,
    kind: tanren_configuration_secrets::UserCredentialKind,
    #[serde(deserialize_with = "tanren_identity_policy::secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    value: secrecy::SecretString,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct UpdateCredentialParams {
    account_id: String,
    item_id: String,
    #[serde(deserialize_with = "tanren_identity_policy::secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    value: secrecy::SecretString,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RemoveCredentialParams {
    account_id: String,
    item_id: String,
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
        AppServiceError::Configuration(reason) => {
            (reason.code().to_owned(), reason.summary().to_owned())
        }
        AppServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message),
        AppServiceError::Store(_) => (
            "internal_error".to_owned(),
            "Tanren encountered an internal error.".to_owned(),
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

fn validation_failure(summary: &str) -> CallToolResult {
    let body = json!({
        "code": "validation_failed",
        "summary": summary,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}

fn parse_account_id(raw: &str) -> std::result::Result<AccountId, String> {
    let parsed = Uuid::parse_str(raw).map_err(|_| "account_id must be a valid uuid".to_owned())?;
    Ok(AccountId::new(parsed))
}
