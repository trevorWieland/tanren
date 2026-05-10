//! `@mcp` harness — spawns `tanren-mcp-app` on an ephemeral port and
//! drives the three account-flow tools through the rmcp
//! streamable-HTTP client.

use std::collections::HashMap;
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
    AcceptInvitationRequest, AccountFailureReason, AccountView, SignInRequest, SignUpRequest,
    SignedInAccountView, SwitchActiveAccountRequest,
};
use tanren_identity_policy::{AccountId, SessionToken};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api::{scenario_db_path, sqlite_url};
use super::api_codec::code_to_reason;
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, InvalidSessionKind,
};

const TEST_API_KEY: &str = "bdd-test-key";
const DEFAULT_WINDOW_KEY: &str = "_default";

/// `@mcp` wire harness.
pub struct McpHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    endpoint: String,
    clients: HashMap<String, RunningService<RoleClient, ClientInfo>>,
    active_account_by_window: HashMap<String, AccountId>,
    session_tokens_by_account: HashMap<AccountId, SessionToken>,
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

        let endpoint = format!("http://{local_addr}/mcp");
        let client = Self::connect_client(&endpoint).await?;
        let mut clients = HashMap::new();
        clients.insert(DEFAULT_WINDOW_KEY.to_owned(), client);

        Ok(Self {
            store,
            db_path,
            endpoint,
            clients,
            active_account_by_window: HashMap::new(),
            session_tokens_by_account: HashMap::new(),
            server: Some(server),
        })
    }

    async fn connect_client(
        endpoint: &str,
    ) -> HarnessResult<RunningService<RoleClient, ClientInfo>> {
        let config = StreamableHttpClientTransportConfig::with_uri(endpoint.to_owned())
            .auth_header(TEST_API_KEY.to_owned());
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
        ClientInfo::default()
            .serve(transport)
            .await
            .map_err(|e| HarnessError::Transport(format!("rmcp serve: {e}")))
    }

    async fn call_tool(&mut self, name: &'static str, body: Value) -> HarnessResult<Value> {
        self.call_tool_in_window(None, name, body).await
    }

    async fn call_tool_in_window(
        &mut self,
        _window_id: Option<&str>,
        name: &'static str,
        body: Value,
    ) -> HarnessResult<Value> {
        let client = self
            .clients
            .get(DEFAULT_WINDOW_KEY)
            .ok_or_else(|| HarnessError::Transport("rmcp client missing".to_owned()))?;
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
        for (_, client) in self.clients.drain() {
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
        let token = decode_session_token(&payload)?;
        self.session_tokens_by_account
            .insert(session.account_id, token);
        self.active_account_by_window
            .insert(DEFAULT_WINDOW_KEY.to_owned(), session.account_id);
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
        });
        let payload = self.call_tool("account.sign_in", body).await?;
        let session = decode_session(&payload)?;
        let token = decode_session_token(&payload)?;
        self.session_tokens_by_account
            .insert(session.account_id, token);
        self.active_account_by_window
            .insert(DEFAULT_WINDOW_KEY.to_owned(), session.account_id);
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
        let token = decode_session_token(&payload)?;
        self.session_tokens_by_account
            .insert(session.account_id, token);
        self.active_account_by_window
            .insert(DEFAULT_WINDOW_KEY.to_owned(), session.account_id);
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

    async fn list_active_accounts(&mut self) -> HarnessResult<Vec<SignedInAccountView>> {
        let payload = self
            .call_tool("account.list_active", serde_json::json!({}))
            .await?;
        serde_json::from_value(payload["accounts"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode active accounts: {e}")))
    }

    async fn switch_active_account(
        &mut self,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let body = serde_json::to_value(SwitchActiveAccountRequest { target_account_id })
            .map_err(|e| HarnessError::Transport(format!("encode switch request: {e}")))?;
        let payload = self.call_tool("account.switch_active", body).await?;
        let accounts: Vec<SignedInAccountView> =
            serde_json::from_value(payload["accounts"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode active accounts: {e}")))?;
        let active_account_id = active_account_id(&accounts)?;
        self.active_account_by_window
            .insert(DEFAULT_WINDOW_KEY.to_owned(), active_account_id);
        Ok(accounts)
    }

    async fn list_active_accounts_in_window(
        &mut self,
        window_id: &str,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let accounts = self.list_active_accounts().await?;
        let window_key = normalize_window_id(Some(window_id)).to_owned();
        let active = if let Some(account_id) = self.active_account_by_window.get(&window_key) {
            *account_id
        } else {
            active_account_id(&accounts)?
        };
        Ok(project_active_account(accounts, active))
    }

    async fn switch_active_account_in_window(
        &mut self,
        window_id: &str,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let accounts = self.list_active_accounts().await?;
        if !accounts
            .iter()
            .any(|entry| entry.account.id == target_account_id)
        {
            return Err(HarnessError::Account(
                AccountFailureReason::TargetAccountNotSignedIn,
                "target account is not signed in for this MCP harness session".to_owned(),
            ));
        }
        let window_key = normalize_window_id(Some(window_id)).to_owned();
        self.active_account_by_window
            .insert(window_key, target_account_id);
        Ok(project_active_account(accounts, target_account_id))
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }

    async fn invalidate_caller_session(&mut self, mode: InvalidSessionKind) -> HarnessResult<()> {
        let _ = mode;
        let client = Self::connect_client(&self.endpoint).await?;
        self.clients.clear();
        self.clients.insert(DEFAULT_WINDOW_KEY.to_owned(), client);
        self.active_account_by_window.clear();
        self.session_tokens_by_account.clear();
        Ok(())
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

fn decode_session_token(payload: &Value) -> HarnessResult<SessionToken> {
    let raw = payload["session"]["token"]
        .as_str()
        .ok_or_else(|| HarnessError::Transport("missing session.token".to_owned()))?;
    Ok(SessionToken::from_secret(SecretString::from(
        raw.to_owned(),
    )))
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

fn normalize_window_id(window_id: Option<&str>) -> &str {
    match window_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value,
        None => DEFAULT_WINDOW_KEY,
    }
}

fn active_account_id(accounts: &[SignedInAccountView]) -> HarnessResult<AccountId> {
    accounts
        .iter()
        .find(|entry| entry.is_active)
        .map(|entry| entry.account.id)
        .ok_or_else(|| HarnessError::Transport("missing active account".to_owned()))
}

fn project_active_account(
    accounts: Vec<SignedInAccountView>,
    active_account_id: AccountId,
) -> Vec<SignedInAccountView> {
    accounts
        .into_iter()
        .map(|mut entry| {
            entry.is_active = entry.account.id == active_account_id;
            entry
        })
        .collect()
}
