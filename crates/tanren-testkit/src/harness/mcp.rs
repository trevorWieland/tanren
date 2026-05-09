//! `@mcp` harness — spawns `tanren-mcp-app` on an ephemeral port and
//! drives the three account-flow tools through the rmcp
//! streamable-HTTP client.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::RoleClient;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ClientInfo, Content, RawContent};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, ApplyRoleRequest, ApplyRoleResponse, CreateRoleRequest,
    CreateRoleResponse, DeleteRoleRequest, DeleteRoleResponse, EditRoleRequest, EditRoleResponse,
    PermissionCheckRequest, PermissionCheckResponse, PermissionGrantView, RoleTemplateView,
    SignInRequest, SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, NewRole, RoleStore};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api::{code_to_reason, role_code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessRoleTemplate, HarnessSession, RoleHarness, RoleHarnessError, RoleHarnessResult,
    seed_role_admin_grants,
};

const TEST_API_KEY: &str = "bdd-test-key";

/// `@mcp` wire harness.
pub struct McpHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    role_actor: Option<AccountId>,
    client: Option<RunningService<RoleClient, ClientInfo>>,
    server: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for McpHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpHarness")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl McpHarness {
    /// Spawn an ephemeral `tanren-mcp-app` and connect a client to it.
    ///
    /// # Errors
    ///
    /// Returns an error if the database, listener, server, or rmcp
    /// client handshake fails.
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("mcp");
        let db_url = sqlite_url(&db_path);
        let store = Store::connect(&db_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;
        let store = Arc::new(store);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| HarnessError::Transport(format!("bind listener: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| HarnessError::Transport(format!("local addr: {e}")))?;

        let (router, cancellation) = tanren_mcp_app::build_router_with_store(
            store.clone(),
            SecretString::from(TEST_API_KEY.to_owned()),
        );

        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { cancellation.cancelled_owned().await })
                .await;
        });

        // Build the rmcp client transport with the bearer-token header.
        let config =
            StreamableHttpClientTransportConfig::with_uri(format!("http://{local_addr}/mcp"))
                .auth_header(TEST_API_KEY.to_owned());
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
        let client = ClientInfo::default()
            .serve(transport)
            .await
            .map_err(|e| HarnessError::Transport(format!("rmcp serve: {e}")))?;

        Ok(Self {
            store,
            db_path,
            role_actor: None,
            client: Some(client),
            server: Some(server),
        })
    }

    async fn call_tool(&mut self, name: &'static str, body: Value) -> HarnessResult<Value> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HarnessError::Transport("rmcp client gone".to_owned()))?;
        let args: serde_json::Map<String, Value> = match body {
            Value::Object(map) => map,
            other => {
                return Err(HarnessError::Transport(format!(
                    "tool args must be a JSON object, got {other}"
                )));
            }
        };
        let result: CallToolResult = client
            .call_tool(CallToolRequestParams::new(name).with_arguments(args))
            .await
            .map_err(|e| HarnessError::Transport(format!("call_tool {name}: {e}")))?;
        let text = first_text(&result.content).ok_or_else(|| {
            HarnessError::Transport(format!("tool {name} returned no text content"))
        })?;
        let payload: Value = serde_json::from_str(text)
            .map_err(|e| HarnessError::Transport(format!("decode tool result: {e}")))?;
        if result.is_error == Some(true) {
            return Err(failure_from_payload(&payload));
        }
        Ok(payload)
    }

    async fn call_role_tool(
        &mut self,
        name: &'static str,
        body: Value,
    ) -> RoleHarnessResult<Value> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| RoleHarnessError::Transport("rmcp client gone".to_owned()))?;
        let args: serde_json::Map<String, Value> = match body {
            Value::Object(map) => map,
            other => {
                return Err(RoleHarnessError::Transport(format!(
                    "tool args must be a JSON object, got {other}"
                )));
            }
        };
        let result: CallToolResult = client
            .call_tool(CallToolRequestParams::new(name).with_arguments(args))
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("call_tool {name}: {e}")))?;
        let text = first_text(&result.content).ok_or_else(|| {
            RoleHarnessError::Transport(format!("tool {name} returned no text content"))
        })?;
        let payload: Value = serde_json::from_str(text)
            .map_err(|e| RoleHarnessError::Transport(format!("decode tool result: {e}")))?;
        if result.is_error == Some(true) {
            return Err(role_failure_from_payload(&payload));
        }
        Ok(payload)
    }
}

impl Drop for McpHarness {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            drop(client);
        }
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for McpHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Mcp
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
            "display_name": req.display_name,
        });
        let payload = self.call_tool("account.create", body).await?;
        let session = decode_session(&payload)?;
        self.role_actor = Some(session.account_id);
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
        });
        let payload = self.call_tool("account.sign_in", body).await?;
        let session = decode_session(&payload)?;
        self.role_actor = Some(session.account_id);
        Ok(session)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let body = serde_json::json!({
            "invitation_token": req.invitation_token.as_str(),
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
            "display_name": req.display_name,
        });
        let payload = self.call_tool("account.accept_invitation", body).await?;
        let session = decode_session(&payload)?;
        self.role_actor = Some(session.account_id);
        let joined_org = serde_json::from_value(payload["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
        Ok(HarnessAcceptance {
            session,
            joined_org,
        })
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.store
            .seed_invitation(NewInvitation {
                token: fixture.token,
                inviting_org_id: fixture.inviting_org,
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_invitation: {e}")))?;
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}

#[async_trait]
impl RoleHarness for McpHarness {
    async fn create_role(
        &mut self,
        req: CreateRoleRequest,
    ) -> RoleHarnessResult<CreateRoleResponse> {
        self.ensure_role_actor()?;
        let payload = self
            .call_role_tool("role.create", serde_json::json!(req))
            .await?;
        decode_role_payload(payload, "role.create")
    }

    async fn edit_role(&mut self, req: EditRoleRequest) -> RoleHarnessResult<EditRoleResponse> {
        self.ensure_role_actor()?;
        let payload = self
            .call_role_tool("role.edit", serde_json::json!(req))
            .await?;
        decode_role_payload(payload, "role.edit")
    }

    async fn delete_role(
        &mut self,
        req: DeleteRoleRequest,
    ) -> RoleHarnessResult<DeleteRoleResponse> {
        self.ensure_role_actor()?;
        let payload = self
            .call_role_tool("role.delete", serde_json::json!(req))
            .await?;
        decode_role_payload(payload, "role.delete")
    }

    async fn apply_role(&mut self, req: ApplyRoleRequest) -> RoleHarnessResult<ApplyRoleResponse> {
        self.ensure_role_actor()?;
        let payload = self
            .call_role_tool("role.apply", serde_json::json!(req))
            .await?;
        decode_role_payload(payload, "role.apply")
    }

    async fn check_permission(
        &mut self,
        req: PermissionCheckRequest,
    ) -> RoleHarnessResult<PermissionCheckResponse> {
        self.ensure_role_actor()?;
        let payload = self
            .call_role_tool("permission.check", serde_json::json!(req))
            .await?;
        decode_role_payload(payload, "permission.check")
    }

    async fn seed_role_template(&mut self, fixture: HarnessRoleTemplate) -> RoleHarnessResult<()> {
        self.store
            .create_role(NewRole {
                id: fixture.id,
                scope: fixture.scope,
                name: fixture.name,
                permissions: fixture.permissions,
                created_at: fixture.created_at,
                updated_at: fixture.updated_at,
            })
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("seed_role_template: {e}")))?;
        Ok(())
    }

    async fn seed_role_admin_for_authenticated_actor(
        &mut self,
        scope: tanren_identity_policy::RoleScope,
        permissions: Vec<tanren_identity_policy::PermissionName>,
    ) -> RoleHarnessResult<()> {
        let actor = self
            .role_actor
            .ok_or_else(|| RoleHarnessError::Transport("missing role actor".to_owned()))?;
        seed_role_admin_grants(self.store.as_ref(), actor, scope, permissions).await
    }

    async fn read_role_template(
        &self,
        role: tanren_identity_policy::ScopedRole,
    ) -> RoleHarnessResult<Option<RoleTemplateView>> {
        let maybe = self
            .store
            .find_role(role)
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("read_role_template: {e}")))?;
        Ok(maybe.map(role_template_view))
    }

    async fn read_direct_grants(
        &self,
        principal: tanren_identity_policy::PrincipalRef,
    ) -> RoleHarnessResult<Vec<PermissionGrantView>> {
        let grants = self
            .store
            .list_all_direct_grants(principal)
            .await
            .map_err(|e| RoleHarnessError::Transport(format!("read_direct_grants: {e}")))?;
        Ok(grants.into_iter().map(permission_grant_view).collect())
    }
}

impl McpHarness {
    fn ensure_role_actor(&self) -> RoleHarnessResult<AccountId> {
        self.role_actor
            .ok_or_else(|| RoleHarnessError::Transport("missing role actor".to_owned()))
    }
}

fn first_text(content: &[Content]) -> Option<&str> {
    for item in content {
        if let RawContent::Text(text) = &item.raw {
            return Some(text.text.as_str());
        }
    }
    None
}

fn decode_session(payload: &Value) -> HarnessResult<HarnessSession> {
    let account: AccountView = serde_json::from_value(payload["account"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
    let expires_at = payload["session"]["expires_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
    let token_present = payload["session"]["token"]
        .as_str()
        .is_some_and(|s| !s.is_empty());
    Ok(HarnessSession {
        account_id: account.id,
        account,
        expires_at,
        has_token: token_present,
    })
}

fn failure_from_payload(payload: &Value) -> HarnessError {
    let code = payload
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("transport_error")
        .to_owned();
    let summary = payload
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure")
        .to_owned();
    if let Some(reason) = code_to_reason(&code) {
        HarnessError::Account(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}

fn role_failure_from_payload(payload: &Value) -> RoleHarnessError {
    let code = payload
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("transport_error")
        .to_owned();
    let summary = payload
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure")
        .to_owned();
    if let Some(reason) = role_code_to_reason(&code) {
        RoleHarnessError::Role(reason, summary)
    } else {
        RoleHarnessError::Transport(format!("{code}: {summary}"))
    }
}

fn decode_role_payload<T: serde::de::DeserializeOwned>(
    payload: Value,
    tool_name: &str,
) -> RoleHarnessResult<T> {
    serde_json::from_value(payload).map_err(|e| {
        RoleHarnessError::Transport(format!("decode tool result for {tool_name}: {e}"))
    })
}

fn role_template_view(record: tanren_store::RoleRecord) -> RoleTemplateView {
    RoleTemplateView {
        id: record.id,
        scope: record.scope,
        name: record.name,
        permissions: record.permissions,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn permission_grant_view(record: tanren_store::PermissionGrantRecord) -> PermissionGrantView {
    PermissionGrantView {
        id: record.id,
        principal: record.principal,
        scope: record.scope,
        permission: record.permission,
        source: record.source,
        revocation: record.revocation,
        granted_at: record.granted_at,
    }
}
