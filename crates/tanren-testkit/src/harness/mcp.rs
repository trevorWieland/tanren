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
    AcceptInvitationRequest, AccountView, CreateOrgInvitationRequest, OrgInvitationView,
    SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, OrgId, OrganizationPermission,
};
use tanren_store::{AccountStore, CreateInvitation, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api::{code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessMembershipSeed, HarnessOrgInvitationSeed, HarnessResult, HarnessSession,
};

const TEST_API_KEY: &str = "bdd-test-key";

/// `@mcp` wire harness.
pub struct McpHarness {
    store: Arc<Store>,
    db_path: PathBuf,
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
        let payload: Value = serde_json::from_str(&text)
            .map_err(|e| HarnessError::Transport(format!("decode tool result: {e}")))?;
        if result.is_error == Some(true) {
            return Err(failure_from_payload(&payload));
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
        decode_session(&payload)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
        });
        let payload = self.call_tool("account.sign_in", body).await?;
        decode_session(&payload)
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

    async fn seed_org_invitation(
        &mut self,
        fixture: HarnessOrgInvitationSeed,
    ) -> HarnessResult<()> {
        self.store
            .seed_organization_invitation(CreateInvitation {
                token: fixture.token,
                inviting_org_id: fixture.org_id,
                recipient_identifier: fixture.recipient_identifier,
                granted_permissions: fixture.permissions,
                created_by_account_id: fixture.created_by,
                created_at: chrono::Utc::now(),
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_org_invitation: {e}")))?;
        Ok(())
    }

    async fn seed_membership(&mut self, fixture: HarnessMembershipSeed) -> HarnessResult<()> {
        self.store
            .insert_membership(
                fixture.account_id,
                fixture.org_id,
                fixture.permissions,
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_membership: {e}")))?;
        Ok(())
    }

    async fn create_org_invitation(
        &mut self,
        caller_account_id: AccountId,
        caller_org_context: Option<OrgId>,
        request: CreateOrgInvitationRequest,
    ) -> HarnessResult<OrgInvitationView> {
        let body = serde_json::json!({
            "caller_account_id": caller_account_id.as_uuid().to_string(),
            "org_id": request.org_id.as_uuid().to_string(),
            "recipient_identifier": request.recipient_identifier.as_str(),
            "permissions": request.permissions.iter().map(OrganizationPermission::as_str).collect::<Vec<_>>(),
            "expires_at": request.expires_at.to_rfc3339(),
        });
        let payload = self.call_tool("invitation.create", body).await?;
        let caller_org = caller_org_context.unwrap_or(request.org_id);
        decode_invitation_view(&payload, caller_org)
    }

    async fn list_org_invitations(
        &mut self,
        caller_account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<OrgInvitationView>> {
        let body = serde_json::json!({
            "caller_account_id": caller_account_id.as_uuid().to_string(),
            "org_id": org_id.as_uuid().to_string(),
        });
        let payload = self.call_tool("invitation.list_org", body).await?;
        decode_invitation_list(&payload)
    }

    async fn list_recipient_invitations(
        &mut self,
        identifier: &Identifier,
    ) -> HarnessResult<Vec<OrgInvitationView>> {
        let body = serde_json::json!({
            "recipient_identifier": identifier.as_str(),
        });
        let payload = self.call_tool("invitation.list_recipient", body).await?;
        decode_invitation_list(&payload)
    }

    async fn revoke_invitation(
        &mut self,
        caller_account_id: AccountId,
        caller_org_context: Option<OrgId>,
        org_id: OrgId,
        token: InvitationToken,
    ) -> HarnessResult<OrgInvitationView> {
        let body = serde_json::json!({
            "caller_account_id": caller_account_id.as_uuid().to_string(),
            "org_id": org_id.as_uuid().to_string(),
            "token": token.as_str(),
        });
        let payload = self.call_tool("invitation.revoke", body).await?;
        decode_invitation_view(&payload, caller_org_context.unwrap_or(org_id))
    }

    async fn find_membership_permissions(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<OrganizationPermission>> {
        AccountStore::find_organization_permissions(self.store.as_ref(), account_id, org_id)
            .await
            .map_err(|e| HarnessError::Transport(format!("find_membership_permissions: {e}")))
    }
}

fn first_text(content: &[Content]) -> Option<String> {
    for item in content {
        if let RawContent::Text(text) = &item.raw {
            return Some(text.text.clone());
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

fn decode_invitation_view(payload: &Value, _org_id: OrgId) -> HarnessResult<OrgInvitationView> {
    serde_json::from_value(payload.clone())
        .map_err(|e| HarnessError::Transport(format!("decode invitation view: {e}")))
}

fn decode_invitation_list(payload: &Value) -> HarnessResult<Vec<OrgInvitationView>> {
    let invitations = payload
        .get("invitations")
        .ok_or_else(|| HarnessError::Transport("missing invitations field".to_owned()))?;
    serde_json::from_value(invitations.clone())
        .map_err(|e| HarnessError::Transport(format!("decode invitations list: {e}")))
}
