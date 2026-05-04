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
    AcceptInvitationRequest, AccountView, CreateOrganizationRequest, OrganizationFailureReason,
    OrganizationView, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{OrgAdminPermissions, SessionToken};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api_helpers::{code_to_reason, org_code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessOrgCreation, HarnessResult, HarnessSession,
};

const TEST_API_KEY: &str = "bdd-test-key";

/// `@mcp` wire harness.
pub struct McpHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    client: Option<RunningService<RoleClient, ClientInfo>>,
    server: Option<JoinHandle<()>>,
    session_token: Option<SecretString>,
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
            session_token: None,
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
        self.session_token = payload["session"]["token"]
            .as_str()
            .map(|s| SecretString::from(s.to_owned()));
        decode_session(&payload)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
        });
        let payload = self.call_tool("account.sign_in", body).await?;
        self.session_token = payload["session"]["token"]
            .as_str()
            .map(|s| SecretString::from(s.to_owned()));
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
        self.session_token = payload["session"]["token"]
            .as_str()
            .map(|s| SecretString::from(s.to_owned()));
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

    async fn create_organization(
        &mut self,
        request: CreateOrganizationRequest,
    ) -> HarnessResult<HarnessOrgCreation> {
        let token = self
            .session_token
            .as_ref()
            .map(|t| t.expose_secret().to_owned())
            .unwrap_or_default();
        let body = serde_json::json!({
            "session_token": token,
            "name": request.name.as_str(),
        });
        let payload = self.call_tool("create_organization", body).await?;
        let organization: OrganizationView =
            serde_json::from_value(payload["organization"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode organization: {e}")))?;
        let permissions: OrgAdminPermissions =
            serde_json::from_value(payload["membership_permissions"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode permissions: {e}")))?;
        Ok(HarnessOrgCreation {
            organization,
            permissions,
        })
    }

    async fn list_organizations(&mut self) -> HarnessResult<Vec<OrganizationView>> {
        let token = self
            .session_token
            .as_ref()
            .map(|t| t.expose_secret().to_owned())
            .unwrap_or_default();
        let body = serde_json::json!({
            "session_token": token,
        });
        let payload = self.call_tool("list_organizations", body).await?;
        let orgs: Vec<OrganizationView> = serde_json::from_value(payload["organizations"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode organizations: {e}")))?;
        Ok(orgs)
    }

    async fn admin_permissions_for_org(
        &mut self,
        org_id: tanren_identity_policy::OrgId,
    ) -> HarnessResult<OrgAdminPermissions> {
        let token_str = self.session_token.as_ref().ok_or_else(|| {
            HarnessError::Organization(
                OrganizationFailureReason::Unauthenticated,
                "no session token stored".to_owned(),
            )
        })?;
        let session_token = SessionToken::from_secret(token_str.clone());
        let session = self
            .store
            .find_session_by_token(&session_token)
            .await
            .map_err(|e| HarnessError::Transport(format!("find_session: {e}")))?
            .ok_or_else(|| {
                HarnessError::Organization(
                    OrganizationFailureReason::Unauthenticated,
                    "session not found".to_owned(),
                )
            })?;
        let membership =
            AccountStore::find_membership(self.store.as_ref(), session.account_id, org_id)
                .await
                .map_err(|e| HarnessError::Transport(format!("find_membership: {e}")))?;
        Ok(match membership {
            Some(m) => OrgAdminPermissions::from_bits(m.permissions),
            None => OrgAdminPermissions {
                invite: false,
                manage_access: false,
                configure: false,
                set_policy: false,
                delete: false,
            },
        })
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
    } else if let Some(reason) = org_code_to_reason(&code) {
        HarnessError::Organization(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}
