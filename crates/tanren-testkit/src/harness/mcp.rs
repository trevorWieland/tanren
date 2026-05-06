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
    AcceptInvitationRequest, AccountView, AttentionSpecView, ProjectScopedViews, ProjectView,
    SignInRequest, SignUpRequest, SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, NewProject, NewSpec, ProjectStore};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api_support::{code_to_project_reason, code_to_reason, scenario_db_path, sqlite_url};
use super::project::{HarnessProjectFixture, HarnessSpecFixture, ProjectHarness};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
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
}

#[async_trait]
impl ProjectHarness for McpHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Mcp
    }

    async fn seed_project(&mut self, fixture: HarnessProjectFixture) -> HarnessResult<ProjectId> {
        let id = fixture.id;
        self.store
            .seed_project(NewProject {
                id,
                account_id: fixture.account_id,
                name: fixture.name,
                state: fixture.state,
                created_at: fixture.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_project: {e}")))?;
        Ok(id)
    }

    async fn seed_spec(&mut self, fixture: HarnessSpecFixture) -> HarnessResult<SpecId> {
        let id = fixture.id;
        self.store
            .seed_spec(NewSpec {
                id,
                project_id: fixture.project_id,
                name: fixture.name,
                needs_attention: fixture.needs_attention,
                attention_reason: fixture.attention_reason,
                created_at: fixture.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
        Ok(id)
    }

    async fn seed_view_state(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        state: Value,
    ) -> HarnessResult<()> {
        self.store
            .write_view_state(account_id, project_id, state, chrono::Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_view_state: {e}")))?;
        Ok(())
    }

    async fn list_projects(&mut self, account_id: AccountId) -> HarnessResult<Vec<ProjectView>> {
        let body = serde_json::json!({
            "account_id": account_id.as_uuid().to_string(),
        });
        let payload = self.call_tool("project.list", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode project list: {e}")))
    }

    async fn switch_active_project(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
    ) -> HarnessResult<SwitchProjectResponse> {
        let body = serde_json::json!({
            "account_id": account_id.as_uuid().to_string(),
            "project_id": project_id.as_uuid().to_string(),
        });
        let payload = self.call_tool("project.switch_active", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode switch response: {e}")))
    }

    async fn attention_spec(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        spec_id: SpecId,
    ) -> HarnessResult<AttentionSpecView> {
        let body = serde_json::json!({
            "account_id": account_id.as_uuid().to_string(),
            "project_id": project_id.as_uuid().to_string(),
            "spec_id": spec_id.as_uuid().to_string(),
        });
        let payload = self.call_tool("project.attention_spec", body).await?;
        serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode attention spec: {e}")))
    }

    async fn project_scoped_views(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ProjectScopedViews> {
        let body = serde_json::json!({
            "account_id": account_id.as_uuid().to_string(),
        });
        let payload = self.call_tool("project.scoped_views", body).await?;
        let project_id: ProjectId = serde_json::from_value(payload["project_id"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode project_id: {e}")))?;
        let specs: Vec<SpecId> = serde_json::from_value(payload["specs"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode specs: {e}")))?;
        let loops: Vec<tanren_identity_policy::LoopId> =
            serde_json::from_value(payload["loops"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode loops: {e}")))?;
        let milestones: Vec<tanren_identity_policy::MilestoneId> =
            serde_json::from_value(payload["milestones"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode milestones: {e}")))?;
        Ok(ProjectScopedViews {
            project_id,
            specs,
            loops,
            milestones,
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
    } else if let Some(reason) = code_to_project_reason(&code) {
        HarnessError::Project(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}
