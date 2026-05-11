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
use sea_orm::ConnectionTrait;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, ActiveProjectRequest, ActiveProjectView,
    ConnectProjectRepositoryRequest, ConnectProjectRepositoryResponse, CreateProjectRequest,
    CreateProjectResponse, ListVisibleProjectsRequest, ProjectCollectionView, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_provider_integrations::{FixtureSourceControlConfig, FixtureSourceControlProvider};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api::{code_to_reason, project_code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, ProjectHarness,
};

const TEST_API_KEY: &str = "bdd-test-key";

/// `@mcp` wire harness.
pub struct McpHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    server_uri: String,
    client: Option<RunningService<RoleClient, ClientInfo>>,
    session_credentials: HashMap<AccountId, SecretString>,
    last_actor_session_account_id: Option<AccountId>,
    fixture_source_control: FixtureSourceControlProvider,
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

        let fixture_source_control =
            FixtureSourceControlProvider::new(FixtureSourceControlConfig::default());
        let (router, cancellation) = tanren_mcp_app::build_router_with_store(
            store.clone(),
            SecretString::from(TEST_API_KEY.to_owned()),
            Arc::new(fixture_source_control.clone()),
        );

        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { cancellation.cancelled_owned().await })
                .await;
        });

        let server_uri = format!("http://{local_addr}/mcp");
        // Build the rmcp client transport with the bearer-token header.
        let client = Self::connect_client(&server_uri, TEST_API_KEY).await?;

        Ok(Self {
            store,
            db_path,
            server_uri,
            client: Some(client),
            session_credentials: HashMap::new(),
            last_actor_session_account_id: None,
            fixture_source_control,
            server: Some(server),
        })
    }

    async fn connect_client(
        server_uri: &str,
        bearer_token: &str,
    ) -> HarnessResult<RunningService<RoleClient, ClientInfo>> {
        let config = StreamableHttpClientTransportConfig::with_uri(server_uri.to_owned())
            .auth_header(bearer_token.to_owned());
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
        ClientInfo::default()
            .serve(transport)
            .await
            .map_err(|e| HarnessError::Transport(format!("rmcp serve: {e}")))
    }

    async fn invoke_tool(
        client: &RunningService<RoleClient, ClientInfo>,
        name: &'static str,
        body: Value,
    ) -> HarnessResult<Value> {
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

    async fn call_tool(&mut self, name: &'static str, body: Value) -> HarnessResult<Value> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HarnessError::Transport("rmcp client gone".to_owned()))?;
        Self::invoke_tool(client, name, body).await
    }

    async fn call_tool_with_bearer(
        &self,
        bearer_token: &str,
        name: &'static str,
        body: Value,
    ) -> HarnessResult<Value> {
        let client = Self::connect_client(&self.server_uri, bearer_token).await?;
        let payload = Self::invoke_tool(&client, name, body).await;
        drop(client);
        payload
    }

    async fn call_project_tool_as_actor(
        &mut self,
        actor_account_id: Option<AccountId>,
        name: &'static str,
        body: Value,
    ) -> HarnessResult<Value> {
        let maybe_secret = actor_account_id
            .and_then(|account_id| self.session_credentials.get(&account_id).cloned())
            .or_else(|| {
                self.last_actor_session_account_id
                    .and_then(|account_id| self.session_credentials.get(&account_id).cloned())
            });
        if let Some(secret) = maybe_secret {
            self.call_tool_with_bearer(secret.expose_secret(), name, body)
                .await
        } else {
            // Deliberately fall back to the bootstrap key so falsification
            // scenarios can assert that bootstrap credentials alone do not
            // authorize project tools.
            self.call_tool(name, body).await
        }
    }

    fn remember_session_credential(&mut self, session: &HarnessSession, payload: &Value) {
        let token = payload["session"]["token"]
            .as_str()
            .filter(|s| !s.is_empty());
        if let Some(token) = token {
            self.last_actor_session_account_id = Some(session.account_id);
            self.session_credentials
                .insert(session.account_id, SecretString::from(token.to_owned()));
        }
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
        self.remember_session_credential(&session, &payload);
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
        });
        let payload = self.call_tool("account.sign_in", body).await?;
        let session = decode_session(&payload)?;
        self.remember_session_credential(&session, &payload);
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
        self.remember_session_credential(&session, &payload);
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
impl ProjectHarness for McpHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        self.connect_project_repository_as_actor(req.owning_account_id, req)
            .await
    }

    async fn connect_project_repository_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        let body = serde_json::to_value(req)
            .map_err(|e| HarnessError::Transport(format!("encode request: {e}")))?;
        let payload = self
            .call_project_tool_as_actor(Some(actor_account_id), "project.connect_repository", body)
            .await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode project response: {e}")))
    }

    async fn list_visible_projects(
        &mut self,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        let actor_account_id = req.owning_account_id;
        let body = serde_json::to_value(req)
            .map_err(|e| HarnessError::Transport(format!("encode request: {e}")))?;
        let payload = self
            .call_project_tool_as_actor(Some(actor_account_id), "project.list_visible", body)
            .await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode project list response: {e}")))
    }

    async fn create_project(
        &mut self,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        let actor_account_id = req.owning_account_id;
        let body = serde_json::to_value(req)
            .map_err(|e| HarnessError::Transport(format!("encode request: {e}")))?;
        let payload = self
            .call_project_tool_as_actor(Some(actor_account_id), "project.create", body)
            .await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode create project response: {e}")))
    }

    async fn active_project(
        &mut self,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        let actor_account_id = req.owning_account_id;
        let body = serde_json::to_value(req)
            .map_err(|e| HarnessError::Transport(format!("encode request: {e}")))?;
        let payload = self
            .call_project_tool_as_actor(Some(actor_account_id), "project.active", body)
            .await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode active project response: {e}")))
    }

    async fn set_repository_access(
        &mut self,
        actor_account_id: AccountId,
        repository: tanren_identity_policy::RepositoryRef,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.fixture_source_control
            .set_repository_access(actor_account_id, repository, allowed);
        Ok(())
    }

    async fn set_designated_host_create_access(
        &mut self,
        actor_account_id: AccountId,
        host: tanren_identity_policy::DesignatedHost,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.fixture_source_control.set_host_reachable(&host, true);
        self.fixture_source_control
            .set_host_create_access(actor_account_id, &host, allowed);
        Ok(())
    }

    async fn repository_created_at_host(
        &self,
        host: &tanren_identity_policy::DesignatedHost,
        repository: &tanren_identity_policy::RepositoryRef,
    ) -> HarnessResult<bool> {
        Ok(self
            .fixture_source_control
            .repository_created_at_host(host, repository))
    }

    async fn source_control_call_counters(
        &mut self,
    ) -> HarnessResult<tanren_provider_integrations::SourceControlCallCounters> {
        Ok(self.fixture_source_control.call_counters())
    }

    async fn break_project_store_for_testing(&mut self) -> HarnessResult<()> {
        self.store
            .connection()
            .execute_unprepared("DROP TABLE IF EXISTS projects")
            .await
            .map_err(|e| HarnessError::Transport(format!("drop projects table: {e}")))?;
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
    } else if let Some(reason) = project_code_to_reason(&code) {
        HarnessError::Project(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}
