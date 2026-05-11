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
    AcceptInvitationRequest, AccountView, CreateUserCredentialRequest,
    CreateUserCredentialResponse, ListUserCredentialsResponse, ListUserSettingsResponse,
    RemoveUserCredentialResponse, SignInRequest, SignUpRequest, UpsertUserSettingRequest,
    UpsertUserSettingResponse,
};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api::{code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

const TEST_API_KEY: &str = "bdd-test-key";

/// `@mcp` wire harness.
pub struct McpHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    endpoint_url: String,
    client: Option<RunningService<RoleClient, ClientInfo>>,
    server: Option<JoinHandle<()>>,
    authenticated_account_id: Option<tanren_identity_policy::AccountId>,
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
        let endpoint_url = format!("http://{local_addr}/mcp");

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
        let client = connect_client(&endpoint_url, TEST_API_KEY.to_owned()).await?;

        Ok(Self {
            store,
            db_path,
            endpoint_url,
            client: Some(client),
            server: Some(server),
            authenticated_account_id: None,
        })
    }

    async fn rotate_auth(&mut self, bearer_token: String) -> HarnessResult<()> {
        if let Some(client) = self.client.take() {
            drop(client);
        }
        let client = connect_client(&self.endpoint_url, bearer_token).await?;
        self.client = Some(client);
        Ok(())
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
        let (session, session_token) = decode_session(&payload)?;
        self.rotate_auth(session_token).await?;
        self.authenticated_account_id = Some(session.account_id);
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
        });
        let payload = self.call_tool("account.sign_in", body).await?;
        let (session, session_token) = decode_session(&payload)?;
        self.rotate_auth(session_token).await?;
        self.authenticated_account_id = Some(session.account_id);
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
        let (session, session_token) = decode_session(&payload)?;
        self.rotate_auth(session_token).await?;
        self.authenticated_account_id = Some(session.account_id);
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

    async fn list_user_settings(
        &mut self,
        requested_account_id: tanren_identity_policy::AccountId,
    ) -> HarnessResult<ListUserSettingsResponse> {
        let body = serde_json::json!({
            "account_id": requested_account_id.to_string(),
        });
        let payload = self.call_tool("config.user.list", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode settings response: {e}")))
    }

    async fn upsert_user_setting(
        &mut self,
        requested_account_id: tanren_identity_policy::AccountId,
        request: UpsertUserSettingRequest,
    ) -> HarnessResult<UpsertUserSettingResponse> {
        let body = serde_json::json!({
            "account_id": requested_account_id.to_string(),
            "key": request.key,
            "value": request.value,
        });
        let payload = self.call_tool("config.user.set", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode upsert response: {e}")))
    }

    async fn list_user_credentials(
        &mut self,
        requested_account_id: tanren_identity_policy::AccountId,
    ) -> HarnessResult<ListUserCredentialsResponse> {
        let body = serde_json::json!({
            "account_id": requested_account_id.to_string(),
        });
        let payload = self.call_tool("credential.list", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode credential list response: {e}")))
    }

    async fn add_user_credential(
        &mut self,
        requested_account_id: tanren_identity_policy::AccountId,
        request: CreateUserCredentialRequest,
    ) -> HarnessResult<CreateUserCredentialResponse> {
        let body = serde_json::json!({
            "account_id": requested_account_id.to_string(),
            "kind": request.kind,
            "value": request.value.expose_secret(),
        });
        let payload = self.call_tool("credential.add", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode add credential response: {e}")))
    }

    async fn remove_user_credential(
        &mut self,
        requested_account_id: tanren_identity_policy::AccountId,
        item_id: &str,
    ) -> HarnessResult<RemoveUserCredentialResponse> {
        let body = serde_json::json!({
            "account_id": requested_account_id.to_string(),
            "item_id": item_id,
        });
        let payload = self.call_tool("credential.remove", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode remove credential response: {e}")))
    }

    async fn expire_session(&mut self) -> HarnessResult<()> {
        Err(HarnessError::Transport(
            "expire_session is not supported by this harness".to_owned(),
        ))
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

async fn connect_client(
    endpoint_url: &str,
    bearer_token: String,
) -> HarnessResult<RunningService<RoleClient, ClientInfo>> {
    let config = StreamableHttpClientTransportConfig::with_uri(endpoint_url.to_owned())
        .auth_header(bearer_token);
    let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
    ClientInfo::default()
        .serve(transport)
        .await
        .map_err(|e| HarnessError::Transport(format!("rmcp serve: {e}")))
}

fn decode_session(payload: &Value) -> HarnessResult<(HarnessSession, String)> {
    let account: AccountView = serde_json::from_value(payload["account"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
    let expires_at = payload["session"]["expires_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
    let token = payload["session"]["token"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| HarnessError::Transport("missing session.token".to_owned()))?
        .to_owned();
    Ok((
        HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: true,
        },
        token,
    ))
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
        HarnessError::FailureCode(code, summary)
    }
}
